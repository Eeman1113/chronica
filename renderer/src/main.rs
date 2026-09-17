//! Chronica renderer (Stage 7): egui/eframe over the read-only inspection API.
//! Owns NO simulation state and cannot create/mutate entities (directive §2.11): it holds the
//! Sim, calls `tick()` on it, and *reads* everything it draws through `inspection`. Nothing is
//! created because you look; nothing stops because you look away (Test H is architectural: there
//! is no observation input anywhere in the engine).

use chronica_engine::core::ids::EntityRef;
use chronica_engine::inspection;
use chronica_engine::sim::{Config, Sim};
use eframe::egui;

struct App {
    sim: Sim,
    running: bool,
    speed: f32, // days per frame (fractional = slow motion)
    day_accum: f32,
    tex: Option<egui::TextureHandle>,
    cam: egui::Vec2, // world-space center (cells)
    zoom: f32,       // pixels per cell
    selected_cell: Option<(i32, i32)>,
    selected_person: Option<usize>,
    selected_animal: Option<usize>,
    why_lines: Vec<String>,
    seed_input: String,
    notable_only: bool,
}

impl App {
    fn new(seed: u64) -> App {
        App {
            sim: Sim::new(Config { seed, width: 192, height: 128 }),
            running: true,
            speed: 1.0,
            day_accum: 0.0,
            tex: None,
            cam: egui::vec2(96.0, 64.0),
            zoom: 7.0,
            selected_cell: None,
            selected_person: None,
            selected_animal: None,
            why_lines: Vec::new(),
            seed_input: seed.to_string(),
            notable_only: true,
        }
    }

    fn map_image(&self) -> egui::ColorImage {
        let g = &self.sim.grid;
        let (w, h) = (g.w as usize, g.h as usize);
        let mut px = vec![egui::Color32::BLACK; w * h];
        for i in 0..w * h {
            let c = if g.ocean[i] {
                egui::Color32::from_rgb(18, 34, 66)
            } else if g.is_river(i) || g.is_lake(i) {
                egui::Color32::from_rgb(52, 90, 160)
            } else if g.burning[i] > 0 {
                egui::Color32::from_rgb(230, 92, 30)
            } else if g.crop_cover.get(i).copied().unwrap_or(0.0) > 0.15 {
                // fields read from orbit: the land people remade
                let season = self.sim.clock.season();
                match season {
                    chronica_engine::core::clock::Season::Winter => {
                        egui::Color32::from_rgb(150, 132, 96)
                    }
                    chronica_engine::core::clock::Season::Autumn => {
                        egui::Color32::from_rgb(212, 178, 70)
                    }
                    _ => egui::Color32::from_rgb(178, 168, 74),
                }
            } else if g.is_path(i) {
                egui::Color32::from_rgb(150, 128, 96) // roads walked into being
            } else {
                let elev = g.elev[i].clamp(0.0, 1.2);
                let cover = g.veg_cover.get(i).copied().unwrap_or(0.0).min(2.5) / 2.5;
                let snow = g.snow[i] > 0.02;
                if snow {
                    egui::Color32::from_rgb(225, 228, 235)
                } else {
                    let r = 96.0 + elev * 90.0 - cover * 60.0;
                    let gr = 92.0 + cover * 110.0 + elev * 20.0;
                    let b = 58.0 + elev * 40.0 - cover * 30.0;
                    egui::Color32::from_rgb(
                        r.clamp(0.0, 255.0) as u8,
                        gr.clamp(0.0, 255.0) as u8,
                        b.clamp(0.0, 255.0) as u8,
                    )
                }
            };
            px[i] = c;
        }
        // burn scars
        for i in 0..w * h {
            if g.burn_scar[i] > 0.3 && !g.ocean[i] && g.burning[i] == 0 {
                px[i] = egui::Color32::from_rgb(70, 60, 52);
            }
        }
        egui::ColorImage {
            size: [w, h],
            pixels: px,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.running {
            self.day_accum += self.speed;
            while self.day_accum >= 1.0 {
                self.sim.tick();
                self.day_accum -= 1.0;
            }
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("CHRONICA");
                ui.label(self.sim.clock.date_string());
                if ui.button(if self.running { "⏸ pause" } else { "▶ run" }).clicked() {
                    self.running = !self.running;
                }
                ui.add(
                    egui::Slider::new(&mut self.speed, 0.05..=30.0)
                        .logarithmic(true)
                        .text("days/frame"),
                );
                ui.label("(scroll = zoom, drag = pan)");
                ui.separator();
                ui.label(format!(
                    "people {}  events {}",
                    chronica_engine::humans::population(&self.sim),
                    self.sim.history.events.len()
                ));
                ui.separator();
                ui.label("seed:");
                ui.text_edit_singleline(&mut self.seed_input);
                if ui.button("new world").clicked() {
                    // any text works as a seed: numbers parse, words hash
                    let t = self.seed_input.trim();
                    let seed = t.parse::<u64>().unwrap_or_else(|_| {
                        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
                        for b in t.as_bytes() {
                            h ^= *b as u64;
                            h = h.wrapping_mul(0x0000_0100_0000_01B3);
                        }
                        h
                    });
                    // world generation takes a few seconds; make that visible
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                        "Chronica — generating world…".into(),
                    ));
                    *self = App::new(seed);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Title("Chronica".into()));
                }
            });
        });

        egui::SidePanel::right("inspect").min_width(380.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some((x, y)) = self.selected_cell {
                    ui.heading(format!("Land at ({x},{y})"));
                    if let Some(i) = self.sim.grid.idx(x, y) {
                        let g = &self.sim.grid;
                        ui.label(format!(
                            "elev {:.2}  temp {:.1}°  moisture {:.2}",
                            g.elev[i],
                            g.temp[i],
                            g.plant_moisture(i)
                        ));
                        ui.label(format!(
                            "surface water {:.3}  groundwater {:.2}  snow {:.2}",
                            g.surface[i], g.ground[i], g.snow[i]
                        ));
                        ui.label(format!(
                            "soil N {:.2}  litter {:.2}  burn scar {:.2}",
                            g.soil_n[i], g.litter[i], g.burn_scar[i]
                        ));
                        if let Some(bi) = self.sim.objects.building_at(i as u32) {
                            let b = &self.sim.objects.buildings[bi];
                            ui.separator();
                            let kind = match b.kind {
                                chronica_engine::objects::BuildingKind::Hut => "Dwelling",
                                chronica_engine::objects::BuildingKind::Granary => {
                                    "Common storehouse"
                                }
                                chronica_engine::objects::BuildingKind::Palisade => {
                                    "Palisade"
                                }
                                chronica_engine::objects::BuildingKind::Hall => "Hall",
                            };
                            let state = if b.progress < 1.0 {
                                format!("under construction ({:.0}%)", b.progress * 100.0)
                            } else {
                                format!("condition {:.0}%", b.condition * 100.0)
                            };
                            ui.heading(format!("{kind} — {state}"));
                            ui.label(format!(
                                "built by {}",
                                chronica_engine::inspection::name_of(
                                    &self.sim,
                                    chronica_engine::core::ids::EntityRef::Person(b.builder)
                                )
                            ));
                            ui.label(format!(
                                "stores: {:.1} fresh, {:.1} smoked  ·  woodpile {:.1}  ·  timber used {:.0}",
                                b.food_store, b.preserved_store, b.firewood, b.wood_used
                            ));
                        }
                        let status = if g.ocean[i] {
                            "open sea"
                        } else if g.is_river(i) {
                            "a river runs here"
                        } else if g.is_lake(i) {
                            "still water"
                        } else {
                            "dry land"
                        };
                        ui.label(status);
                        // plants actually here
                        if let Some(pids) = self.sim.plants.by_cell.get(i) {
                            ui.separator();
                            ui.label(format!("{} plants growing here:", pids.len()));
                            for &pi in pids.iter().take(12) {
                                let p = &self.sim.plants.list[pi as usize];
                                ui.label(format!(
                                    "  {} — biomass {:.2}, health {:.2}",
                                    chronica_engine::species::PLANTS[p.species as usize].name,
                                    p.biomass,
                                    p.health
                                ));
                            }
                        }
                    }
                    ui.separator();
                }
                if let Some(pi) = self.selected_person {
                    let h = &self.sim.humans.list[pi];
                    ui.heading(format!("{}{}", h.name, if h.alive { "" } else { " (dead)" }));
                    ui.label(format!(
                        "culture {}  age {:.0}y  {}",
                        h.culture,
                        (self.sim.clock.day as i64 - h.born) as f32 / 360.0,
                        if h.sex == 0 { "woman" } else { "man" }
                    ));
                    ui.label(format!(
                        "hunger {:.2}  thirst {:.2}  nutrition {:.2}  health {:.2}",
                        h.hunger, h.thirst, h.nutrition, h.health
                    ));
                    ui.label(format!("doing: {:?}", h.current));
                    ui.label(format!(
                        "weighed: eat {:.2} drink {:.2} gather {:.2} hunt {:.2} build {:.2} social {:.2} farm {:.2}",
                        h.rationale.eat,
                        h.rationale.drink,
                        h.rationale.gather,
                        h.rationale.hunt,
                        h.rationale.build,
                        h.rationale.social,
                        h.rationale.farm
                    ));
                    ui.label(format!("techniques: {:?}", h.techs));
                    ui.label(format!("bonds: {}", h.rels.len()));
                    ui.label(format!("memories: {}", h.memories.len()));
                    ui.separator();
                    ui.label("Biography (derived from events):");
                    for line in inspection::biography(
                        &self.sim,
                        EntityRef::Person(
                            chronica_engine::humans::Humans::id_of(pi),
                        ),
                    )
                    .iter()
                    .rev()
                    .take(15)
                    {
                        ui.label(format!("  {line}"));
                    }
                    ui.separator();
                }
                if let Some(ai) = self.selected_animal {
                    let a = &self.sim.animals.list[ai];
                    let sp = &chronica_engine::species::ANIMALS[a.species as usize];
                    ui.heading(format!("{}{}", sp.name, if a.alive { "" } else { " (dead)" }));
                    ui.label(format!(
                        "age {:.1}y  gen {}  hunger {:.2} thirst {:.2} fear {:.2}",
                        (self.sim.clock.day as i64 - a.born) as f32 / 360.0,
                        a.generation,
                        a.hunger,
                        a.thirst,
                        a.fear
                    ));
                    ui.label(format!(
                        "genes: wariness {:.2} appetite {:.2} boldness {:.2} herd {:.2}",
                        a.genes[0], a.genes[1], a.genes[2], a.genes[3]
                    ));
                    ui.label(format!(
                        "weighed: flee {:.2} drink {:.2} eat {:.2} hunt {:.2} mate {:.2}",
                        a.rationale.flee,
                        a.rationale.drink,
                        a.rationale.eat,
                        a.rationale.hunt,
                        a.rationale.mate
                    ));
                    ui.separator();
                }
                ui.heading("Population");
                let people_alive = chronica_engine::humans::population(&self.sim);
                let children = self
                    .sim
                    .humans
                    .list
                    .iter()
                    .filter(|h| {
                        h.alive && (self.sim.clock.day as i64 - h.born) < 14 * 360
                    })
                    .count();
                ui.label(format!("People: {people_alive}  (of whom {children} children)"));
                let mut tame_counts = [0usize; 8];
                for a in self.sim.animals.list.iter() {
                    if a.alive && !a.tamed_by.is_none() {
                        tame_counts[a.species as usize] += 1;
                    }
                }
                for (i, (name, count)) in
                    chronica_engine::animals::population_by_species(&self.sim)
                        .iter()
                        .enumerate()
                {
                    if *count > 0 {
                        if tame_counts[i] > 0 {
                            ui.label(format!(
                                "{name}: {count}  ({} tame)",
                                tame_counts[i]
                            ));
                        } else {
                            ui.label(format!("{name}: {count}"));
                        }
                    }
                }
                let extinct: Vec<&str> =
                    chronica_engine::animals::population_by_species(&self.sim)
                        .iter()
                        .filter(|(_, c)| *c == 0)
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|n| Box::leak(n.to_string().into_boxed_str()) as &str)
                        .collect();
                if !extinct.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("extinct: {}", extinct.join(", ")))
                            .color(egui::Color32::from_rgb(150, 110, 110)),
                    );
                }
                let (live_plants, trees, cover) =
                    chronica_engine::vegetation::forest_stats(&self.sim);
                ui.label(format!(
                    "Plants: {live_plants}  ({trees} grown trees, {:.0}% forest)",
                    cover * 100.0
                ));
                let corpses = self
                    .sim
                    .animals
                    .corpses
                    .iter()
                    .filter(|c| !c.gone)
                    .count();
                if corpses > 0 {
                    ui.label(format!("Carcasses on the ground: {corpses}"));
                }
                ui.separator();
                ui.heading("Settlements");
                for (name, (x, y)) in chronica_engine::society::living_settlements(&self.sim) {
                    ui.label(format!("{name} at ({x},{y})"));
                }
                ui.heading("Beliefs");
                let clusters = chronica_engine::society::belief_clusters(&self.sim);
                let mut forgotten = 0;
                for (name, cnt) in &clusters {
                    if *cnt > 0 {
                        ui.label(format!("{name}: {cnt} faithful"));
                    } else {
                        forgotten += 1;
                    }
                }
                if forgotten > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "…and {forgotten} faiths whose last believer is gone"
                        ))
                        .color(egui::Color32::from_rgb(140, 130, 150)),
                    );
                }
                if !self.why_lines.is_empty() {
                    ui.separator();
                    ui.heading("Why?");
                    for l in &self.why_lines {
                        ui.label(l);
                    }
                }
            });
        });

        egui::TopBottomPanel::bottom("chronicle").min_height(140.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Chronicle");
                ui.checkbox(&mut self.notable_only, "notable only");
            });
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                use chronica_engine::core::ids::EntityRef as ER;
                use chronica_engine::history::EventKind as EK;
                let notable = |ev: &chronica_engine::history::Event| -> bool {
                    let human = matches!(ev.subject, ER::Person(_));
                    match &ev.kind {
                        EK::PlantDied { .. } => false,
                        EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => {
                            human
                        }
                        EK::Killed { by } => human || matches!(by, ER::Person(_)),
                        EK::LightningStrike | EK::FireDied => false,
                        _ => true,
                    }
                };
                let mut shown = 0;
                let mut lines: Vec<&chronica_engine::history::Event> = Vec::new();
                for ev in self.sim.history.events.iter().rev() {
                    if !self.notable_only || notable(ev) {
                        lines.push(ev);
                        shown += 1;
                        if shown >= 40 {
                            break;
                        }
                    }
                }
                for ev in lines.iter().rev() {
                    let line = inspection::describe(&self.sim, ev);
                    if ui.link(line).clicked() {
                        self.why_lines = inspection::why_text(&self.sim, ev.id, 10);
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let img = self.map_image();
            let tex = self.tex.get_or_insert_with(|| {
                ui.ctx().load_texture("map", img.clone(), egui::TextureOptions::NEAREST)
            });
            tex.set(img, egui::TextureOptions::NEAREST);

            let avail = ui.available_size();
            let (rect, resp) =
                ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
            let ww = self.sim.grid.w as f32;
            let wh = self.sim.grid.h as f32;

            // camera controls: scroll zooms about the cursor, drag pans
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(ptr) = resp.hover_pos() {
                    let world_before = self.cam
                        + (ptr - rect.center()) / self.zoom;
                    self.zoom = (self.zoom * (1.0 + scroll * 0.0015)).clamp(3.0, 64.0);
                    let world_after = self.cam + (ptr - rect.center()) / self.zoom;
                    self.cam += world_before - world_after;
                }
            }
            if resp.dragged() {
                self.cam -= resp.drag_delta() / self.zoom;
            }
            self.cam.x = self.cam.x.clamp(0.0, ww);
            self.cam.y = self.cam.y.clamp(0.0, wh);

            let to_screen = |wx: f32, wy: f32| -> egui::Pos2 {
                rect.center() + (egui::vec2(wx, wy) - self.cam) * self.zoom
            };
            // visible world window as UV into the map texture
            let half = rect.size() / (2.0 * self.zoom);
            let uv = egui::Rect::from_min_max(
                egui::pos2(
                    (self.cam.x - half.x) / ww,
                    (self.cam.y - half.y) / wh,
                ),
                egui::pos2(
                    (self.cam.x + half.x) / ww,
                    (self.cam.y + half.y) / wh,
                ),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(8, 12, 24));

            let visible = |x: i32, y: i32| -> bool {
                (x as f32) > self.cam.x - half.x - 2.0
                    && (x as f32) < self.cam.x + half.x + 2.0
                    && (y as f32) > self.cam.y - half.y - 2.0
                    && (y as f32) < self.cam.y + half.y + 2.0
            };
            let ascii = self.zoom >= 12.0; // close-up: the world in glyphs, prototype-style
            let labels = self.zoom >= 22.0;

            if !ascii {
                painter.image(tex.id(), rect, uv, egui::Color32::WHITE);
            } else {
                // ---------- ASCII world ----------
                let x0 = (self.cam.x - half.x).floor() as i32 - 1;
                let x1 = (self.cam.x + half.x).ceil() as i32 + 1;
                let y0 = (self.cam.y - half.y).floor() as i32 - 1;
                let y1 = (self.cam.y + half.y).ceil() as i32 + 1;
                let font = egui::FontId::monospace(self.zoom * 0.95);
                let g = &self.sim.grid;
                for cy in y0..=y1 {
                    for cx in x0..=x1 {
                        let Some(i) = g.idx(cx, cy) else { continue };
                        let center = to_screen(cx as f32 + 0.5, cy as f32 + 0.5);
                        // choose glyph + colors from TRUE state
                        // deterministic per-cell variant for glyph texture (prototype style)
                        let vh = chronica_engine::core::rng::splitmix64(i as u64);
                        let v = (vh % 4) as usize;
                        let season = self.sim.clock.season();
                        use chronica_engine::core::clock::Season;
                        let lat = ((cy as f32 / g.h as f32) - 0.5).abs() * 2.0;
                        let (ch, fg, bg): (&str, egui::Color32, egui::Color32) = if g.ocean[i] {
                            if g.elev[i] < -0.45 {
                                (["≈", "~", "≈", "≈"][v], egui::Color32::from_rgb(40, 70, 130), egui::Color32::from_rgb(10, 18, 40))
                            } else {
                                (["~", "≈", "~", "≈"][v], egui::Color32::from_rgb(70, 110, 180), egui::Color32::from_rgb(16, 30, 60))
                            }
                        } else if g.ice_bears(i) {
                            (["═", "─", "═", "═"][v], egui::Color32::from_rgb(200, 225, 245), egui::Color32::from_rgb(90, 120, 150))
                        } else if g.surface[i] > 0.12 {
                            if g.is_river(i) {
                                (["≈", "~", "≈", "~"][v], egui::Color32::from_rgb(130, 185, 245), egui::Color32::from_rgb(26, 46, 90))
                            } else {
                                (["~", "≈", "≈", "~"][v], egui::Color32::from_rgb(115, 165, 235), egui::Color32::from_rgb(24, 44, 88))
                            }
                        } else if g.is_river(i) {
                            (["≈", "~", "~", "≈"][v], egui::Color32::from_rgb(130, 185, 245), egui::Color32::from_rgb(26, 46, 90))
                        } else if g.burning[i] > 0 {
                            (["^", "▲", "*", "^"][v], egui::Color32::from_rgb(255, 170, 60), egui::Color32::from_rgb(120, 40, 10))
                        } else if g.snow[i] > 0.02 {
                            (["·", "∙", ".", "∙"][v], egui::Color32::from_rgb(244, 246, 252), egui::Color32::from_rgb(150, 156, 170))
                        } else if g.elev[i] > 1.02 {
                            (["▲", "△", "▲", "▲"][v], egui::Color32::from_rgb(240, 240, 245), egui::Color32::from_rgb(70, 66, 78))
                        } else if g.elev[i] > 0.82 {
                            (["▒", "▲", "▒", "▒"][v], egui::Color32::from_rgb(150, 138, 155), egui::Color32::from_rgb(52, 48, 58))
                        } else if g.is_path(i)
                            && g.crop_cover.get(i).copied().unwrap_or(0.0) < 0.05
                        {
                            ("∙", egui::Color32::from_rgb(170, 150, 120), egui::Color32::from_rgb(52, 44, 34))
                        } else if g.burn_scar[i] > 0.3 {
                            (["\"", "·", "τ", "·"][v], egui::Color32::from_rgb(105, 100, 95), egui::Color32::from_rgb(36, 34, 32))
                        } else {
                            // living ground: what actually grows here decides the glyph
                            let mut oak_b = 0.0f32;
                            let mut pine_b = 0.0f32;
                            let mut reed_b = 0.0f32;
                            let mut crop_b = 0.0f32;
                            let mut shrub_b = 0.0f32;
                            let mut grass_b = 0.0f32;
                            if let Some(pids) = self.sim.plants.by_cell.get(i) {
                                for &pi in pids {
                                    let pl = &self.sim.plants.list[pi as usize];
                                    if !pl.alive {
                                        continue;
                                    }
                                    match pl.species {
                                        chronica_engine::species::SP_OAK => oak_b += pl.biomass,
                                        chronica_engine::species::SP_PINE => pine_b += pl.biomass,
                                        chronica_engine::species::SP_REED => reed_b += pl.biomass,
                                        chronica_engine::species::SP_WHEAT => crop_b += pl.biomass,
                                        chronica_engine::species::SP_SCRUB => shrub_b += pl.biomass,
                                        _ => grass_b += pl.biomass,
                                    }
                                }
                            }
                            if crop_b > 0.05 {
                                // field through the year: plowed / growing / harvest / stubble
                                let (fc, fch) = match season {
                                    Season::Spring => (egui::Color32::from_rgb(150, 120, 70), "="),
                                    Season::Summer => (egui::Color32::from_rgb(190, 175, 70), "≡"),
                                    Season::Autumn => (egui::Color32::from_rgb(230, 195, 80), "≡"),
                                    Season::Winter => (egui::Color32::from_rgb(140, 125, 95), "∙"),
                                };
                                (fch, fc, egui::Color32::from_rgb(56, 44, 22))
                            } else if oak_b > 1.2 || pine_b > 1.2 {
                                if pine_b > oak_b {
                                    // taiga conifers hold their grey-green all year
                                    (["↑", "Λ", "↑", "↑"][v], egui::Color32::from_rgb(105, 135, 110), egui::Color32::from_rgb(24, 34, 28))
                                } else {
                                    // broadleaf forest turns with the seasons
                                    let fc = match season {
                                        Season::Autumn => egui::Color32::from_rgb(215, 140, 55),
                                        Season::Winter => egui::Color32::from_rgb(130, 125, 115),
                                        _ => egui::Color32::from_rgb(115, 150, 75),
                                    };
                                    (["♣", "♠", "♠", "♣"][v], fc, egui::Color32::from_rgb(26, 38, 24))
                                }
                            } else if oak_b + pine_b > 0.25 {
                                ("τ", egui::Color32::from_rgb(120, 150, 95), egui::Color32::from_rgb(30, 38, 26))
                            } else if reed_b > 0.15 {
                                (["\"", "~", "τ", "\""][v], egui::Color32::from_rgb(60, 120, 80), egui::Color32::from_rgb(20, 40, 34))
                            } else if shrub_b > 0.25 {
                                (["\"", ";", "·", ";"][v], egui::Color32::from_rgb(140, 145, 80), egui::Color32::from_rgb(36, 36, 24))
                            } else if grass_b > 0.10 {
                                let gc = if grass_b > 0.5 {
                                    egui::Color32::from_rgb(125, 165, 80)
                                } else {
                                    egui::Color32::from_rgb(110, 140, 70)
                                };
                                (["·", ".", ",", "'"][v], gc, egui::Color32::from_rgb(30, 40, 25))
                            } else if g.plant_moisture(i) < 0.18 && g.temp[i] > 18.0 {
                                // desert: hot and truly dry
                                (["·", "~", ".", "·"][v], egui::Color32::from_rgb(200, 175, 130), egui::Color32::from_rgb(60, 50, 34))
                            } else if lat > 0.72 {
                                // tundra: cold bare ground
                                (["·", ",", ".", "·"][v], egui::Color32::from_rgb(165, 175, 160), egui::Color32::from_rgb(48, 52, 48))
                            } else {
                                (["·", ".", ".", "·"][v], egui::Color32::from_rgb(130, 112, 82), egui::Color32::from_rgb(38, 32, 22))
                            }
                        };
                        let cell_rect = egui::Rect::from_center_size(
                            center,
                            egui::vec2(self.zoom + 1.0, self.zoom + 1.0),
                        );
                        painter.rect_filled(cell_rect, 0.0, bg);
                        painter.text(center, egui::Align2::CENTER_CENTER, ch, font.clone(), fg);
                    }
                }
            }

            // ---------- buildings ----------
            for b in self.sim.objects.buildings.iter() {
                if !b.exists && b.progress < 1.0 {
                    continue; // never finished, never a ruin
                }
                let (x, y) = self.sim.grid.xy(b.cell as usize);
                if !visible(x, y) {
                    continue;
                }
                let p = to_screen(x as f32 + 0.5, y as f32 + 0.5);
                if ascii {
                    use chronica_engine::objects::BuildingKind as BK;
                    let (glyph, col) = if !b.exists {
                        // per the key: ruins are grey and crumble
                        ("□", egui::Color32::from_rgb(130, 130, 130))
                    } else if b.progress < 1.0 {
                        ("□", egui::Color32::from_rgb(200, 170, 110))
                    } else {
                        match b.kind {
                            BK::Granary => ("▦", egui::Color32::from_rgb(230, 200, 120)),
                            BK::Palisade => ("#", egui::Color32::from_rgb(190, 165, 120)),
                            BK::Hall => ("†", egui::Color32::from_rgb(210, 190, 160)),
                            BK::Hut => ("⌂", egui::Color32::from_rgb(210, 160, 90)),
                        }
                    };
                    painter.text(
                        p,
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        egui::FontId::monospace(self.zoom * 0.95),
                        col,
                    );
                } else if b.exists {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p, egui::vec2(self.zoom * 0.85, self.zoom * 0.85)),
                        2.0,
                        egui::Color32::from_rgb(160, 120, 70),
                    );
                }
            }
            // ---------- animals: DF letters ----------
            for a in self.sim.animals.list.iter().filter(|a| a.alive) {
                if !visible(a.x, a.y) {
                    continue;
                }
                let sp = &chronica_engine::species::ANIMALS[a.species as usize];
                let p = to_screen(a.x as f32 + 0.5, a.y as f32 + 0.5);
                if ascii {
                    let (ch, col) = match a.species {
                        chronica_engine::species::A_HARE => ("r", egui::Color32::from_rgb(210, 190, 150)),
                        chronica_engine::species::A_DEER => ("d", egui::Color32::from_rgb(200, 160, 110)),
                        chronica_engine::species::A_BOAR => ("b", egui::Color32::from_rgb(150, 110, 80)),
                        chronica_engine::species::A_WOLF => ("w", egui::Color32::from_rgb(235, 90, 90)),
                        chronica_engine::species::A_BEAR => ("B", egui::Color32::from_rgb(230, 110, 60)),
                        chronica_engine::species::A_SHEEP => ("m", egui::Color32::from_rgb(230, 225, 210)),
                        chronica_engine::species::A_HORSE => ("h", egui::Color32::from_rgb(190, 150, 100)),
                        chronica_engine::species::A_FISH => ("\u{223e}", egui::Color32::from_rgb(130, 185, 215)),
                        _ => ("c", egui::Color32::from_rgb(170, 130, 90)),
                    };
                    painter.text(
                        p,
                        egui::Align2::CENTER_CENTER,
                        ch,
                        egui::FontId::monospace(self.zoom * 0.95),
                        col,
                    );
                } else {
                    let col = if sp.prey.is_empty() {
                        egui::Color32::from_rgb(205, 175, 132)
                    } else {
                        egui::Color32::from_rgb(225, 80, 80)
                    };
                    painter.circle_filled(p, (self.zoom * 0.28).max(1.5), col);
                }
                if labels {
                    let doing = match a.rationale.chosen {
                        0 => "fleeing!",
                        1 => "→water",
                        2 => "grazing",
                        3 => "hunting!",
                        4 => "courting",
                        5 => "w/herd",
                        6 => "resting",
                        _ => "roaming",
                    };
                    painter.text(
                        p + egui::vec2(0.0, -self.zoom * 0.55),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{} {}", sp.name, doing),
                        egui::FontId::proportional((self.zoom * 0.32).min(13.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                    );
                }
            }
            // ---------- carcasses: grey letters fading as they rot ----------
            if ascii {
                for c in self.sim.animals.corpses.iter().filter(|c| !c.gone) {
                    let (x, y) = self.sim.grid.xy(c.cell as usize);
                    if !visible(x, y) {
                        continue;
                    }
                    let ch = match c.species {
                        chronica_engine::species::A_HARE => "r",
                        chronica_engine::species::A_DEER => "d",
                        chronica_engine::species::A_BOAR => "b",
                        chronica_engine::species::A_WOLF => "w",
                        chronica_engine::species::A_BEAR => "B",
                        chronica_engine::species::A_SHEEP => "m",
                        chronica_engine::species::A_HORSE => "h",
                        _ => "c",
                    };
                    let rot = ((self.sim.clock.day - c.day) as f32 / 30.0).clamp(0.0, 1.0);
                    let grey = (150.0 - rot * 90.0) as u8;
                    let pos = to_screen(x as f32 + 0.5, y as f32 + 0.5);
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        ch,
                        egui::FontId::monospace(self.zoom * 0.85),
                        egui::Color32::from_rgb(grey, grey, grey),
                    );
                    // the strike through the carcass
                    painter.line_segment(
                        [
                            pos + egui::vec2(-self.zoom * 0.3, 0.0),
                            pos + egui::vec2(self.zoom * 0.3, 0.0),
                        ],
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(grey, grey, grey)),
                    );
                }
            }
            // ---------- people: ☺ villagers, • children ----------
            for h in self.sim.humans.list.iter().filter(|h| h.alive) {
                if !visible(h.x, h.y) {
                    continue;
                }
                let p = to_screen(h.x as f32 + 0.5, h.y as f32 + 0.5);
                if ascii {
                    let cul_col = [
                        egui::Color32::from_rgb(255, 240, 120),
                        egui::Color32::from_rgb(120, 220, 255),
                        egui::Color32::from_rgb(255, 150, 220),
                        egui::Color32::from_rgb(160, 255, 160),
                        egui::Color32::from_rgb(255, 180, 120),
                    ][(h.culture as usize) % 5];
                    let age_y = (self.sim.clock.day as i64 - h.born) as f32 / 360.0;
                    let glyph = if age_y < 14.0 { "•" } else { "☺" };
                    painter.text(
                        p,
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        egui::FontId::monospace(self.zoom * 0.95),
                        cul_col,
                    );
                } else {
                    painter.circle_filled(
                        p,
                        (self.zoom * 0.36).max(2.0),
                        egui::Color32::from_rgb(252, 240, 110),
                    );
                }
                if labels {
                    let doing = match h.current {
                        chronica_engine::humans::HumanAction::Gather => "gathering",
                        chronica_engine::humans::HumanAction::Drink { .. } => "→water",
                        chronica_engine::humans::HumanAction::EatStored => "eating",
                        chronica_engine::humans::HumanAction::Hunt { .. } => "hunting!",
                        chronica_engine::humans::HumanAction::ChopWood => "chopping",
                        chronica_engine::humans::HumanAction::Build { .. } => "building",
                        chronica_engine::humans::HumanAction::Deposit => "storing",
                        chronica_engine::humans::HumanAction::Socialize { .. } => "talking",
                        chronica_engine::humans::HumanAction::Court { .. } => "courting",
                        chronica_engine::humans::HumanAction::TendFarm => "farming",
                        chronica_engine::humans::HumanAction::PreserveFood => "smoking food",
                        chronica_engine::humans::HumanAction::Fish { .. } => "fishing",
                        chronica_engine::humans::HumanAction::Rest => "resting",
                        chronica_engine::humans::HumanAction::Flee { .. } => "fleeing!",
                        chronica_engine::humans::HumanAction::MoveTo { .. } => "walking",
                        chronica_engine::humans::HumanAction::Idle => "idling",
                    };
                    painter.text(
                        p + egui::vec2(0.0, -self.zoom * 0.55),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{} — {}", h.name, doing),
                        egui::FontId::proportional((self.zoom * 0.36).min(14.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            // ---------- recent happenings, marked where they happened ----------
            let now = self.sim.clock.day;
            for ev in self.sim.history.events.iter().rev().take(600) {
                if now.saturating_sub(ev.day) > 3 {
                    break;
                }
                let Some(loc) = ev.loc else { continue };
                let (x, y) = self.sim.grid.xy(loc as usize);
                if !visible(x, y) {
                    continue;
                }
                let p = to_screen(x as f32 + 0.5, y as f32 + 0.5);
                use chronica_engine::history::EventKind as EK;
                let mark = match &ev.kind {
                    EK::Killed { .. } => Some(("✕", egui::Color32::from_rgb(255, 60, 60))),
                    EK::RaidCarriedOut { .. } => Some(("‼", egui::Color32::from_rgb(255, 120, 40))),
                    EK::Born { .. } => Some(("+", egui::Color32::from_rgb(255, 160, 220))),
                    EK::HarvestedFood { .. } => Some(("$", egui::Color32::from_rgb(240, 210, 90))),
                    EK::FelledTree { .. } => Some(("/", egui::Color32::from_rgb(200, 170, 120))),
                    _ => None,
                };
                if let Some((ch, col)) = mark {
                    painter.text(
                        p + egui::vec2(self.zoom * 0.3, -self.zoom * 0.3),
                        egui::Align2::CENTER_CENTER,
                        ch,
                        egui::FontId::monospace((self.zoom * 0.8).max(10.0)),
                        col,
                    );
                }
            }
            // settlement names
            for (name, (x, y)) in chronica_engine::society::living_settlements(&self.sim) {
                if !visible(x, y) {
                    continue;
                }
                painter.text(
                    to_screen(x as f32, y as f32 - 2.0),
                    egui::Align2::CENTER_BOTTOM,
                    name,
                    egui::FontId::proportional(14.0),
                    egui::Color32::WHITE,
                );
            }

            // click (not drag) selects
            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let world = self.cam + (pos - rect.center()) / self.zoom;
                    let cx = world.x.floor() as i32;
                    let cy = world.y.floor() as i32;
                    self.selected_cell = Some((cx, cy));
                    self.selected_person = self
                        .sim
                        .humans
                        .list
                        .iter()
                        .enumerate()
                        .filter(|(_, h)| h.alive)
                        .map(|(i, h)| (i, (h.x - cx).abs().max((h.y - cy).abs())))
                        .filter(|(_, d)| *d <= 2)
                        .min_by_key(|(_, d)| *d)
                        .map(|(i, _)| i);
                    self.selected_animal = self
                        .sim
                        .animals
                        .list
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.alive)
                        .map(|(i, a)| (i, (a.x - cx).abs().max((a.y - cy).abs())))
                        .filter(|(_, d)| *d <= 2)
                        .min_by_key(|(_, d)| *d)
                        .map(|(i, _)| i);
                }
            }
        });
    }
}

fn main() -> eframe::Result {
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2024u64);
    eframe::run_native(
        "Chronica",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
            ..Default::default()
        },
        Box::new(move |_cc| Ok(Box::new(App::new(seed)))),
    )
}
