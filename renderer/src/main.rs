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
                    if let Ok(seed) = self.seed_input.parse::<u64>() {
                        *self = App::new(seed);
                    }
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
                ui.heading("Settlements");
                for (name, (x, y)) in chronica_engine::society::living_settlements(&self.sim) {
                    ui.label(format!("{name} at ({x},{y})"));
                }
                ui.heading("Beliefs");
                for (name, cnt) in chronica_engine::society::belief_clusters(&self.sim) {
                    ui.label(format!("{name}: {cnt} faithful"));
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
            ui.heading("Chronicle");
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                let n = self.sim.history.events.len();
                for ev in self.sim.history.events[n.saturating_sub(40)..].iter() {
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
            painter.image(tex.id(), rect, uv, egui::Color32::WHITE);

            let visible = |x: i32, y: i32| -> bool {
                (x as f32) > self.cam.x - half.x - 2.0
                    && (x as f32) < self.cam.x + half.x + 2.0
                    && (y as f32) > self.cam.y - half.y - 2.0
                    && (y as f32) < self.cam.y + half.y + 2.0
            };
            let close_up = self.zoom >= 14.0;

            // buildings
            for b in self.sim.objects.buildings.iter().filter(|b| b.exists) {
                let (x, y) = self.sim.grid.xy(b.cell as usize);
                if !visible(x, y) {
                    continue;
                }
                let p = to_screen(x as f32 + 0.5, y as f32 + 0.5);
                painter.rect_filled(
                    egui::Rect::from_center_size(
                        p,
                        egui::vec2(self.zoom * 0.85, self.zoom * 0.85),
                    ),
                    2.0,
                    egui::Color32::from_rgb(160, 120, 70),
                );
                if close_up && b.food_store > 0.5 {
                    painter.text(
                        p,
                        egui::Align2::CENTER_CENTER,
                        format!("{:.0}", b.food_store),
                        egui::FontId::proportional(self.zoom * 0.45),
                        egui::Color32::from_rgb(255, 235, 180),
                    );
                }
            }
            // animals: colored dots; at close-up, species initial + what they're doing
            for (ai, a) in self.sim.animals.list.iter().enumerate().filter(|(_, a)| a.alive) {
                if !visible(a.x, a.y) {
                    continue;
                }
                let sp = &chronica_engine::species::ANIMALS[a.species as usize];
                let p = to_screen(a.x as f32 + 0.5, a.y as f32 + 0.5);
                let col = if sp.prey.is_empty() {
                    egui::Color32::from_rgb(205, 175, 132)
                } else {
                    egui::Color32::from_rgb(225, 80, 80)
                };
                painter.circle_filled(p, (self.zoom * 0.28).max(1.5), col);
                if close_up {
                    let doing = match a.rationale.chosen {
                        0 => "fleeing!",
                        1 => "→ water",
                        2 => "grazing",
                        3 => "hunting!",
                        4 => "courting",
                        5 => "w/ herd",
                        6 => "resting",
                        _ => "roaming",
                    };
                    painter.text(
                        p + egui::vec2(0.0, -self.zoom * 0.45),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{} {}", sp.name, doing),
                        egui::FontId::proportional((self.zoom * 0.38).min(13.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210),
                    );
                }
                let _ = ai;
            }
            // people: yellow dots; at close-up, name + current action
            for h in self.sim.humans.list.iter().filter(|h| h.alive) {
                if !visible(h.x, h.y) {
                    continue;
                }
                let p = to_screen(h.x as f32 + 0.5, h.y as f32 + 0.5);
                painter.circle_filled(
                    p,
                    (self.zoom * 0.36).max(2.0),
                    egui::Color32::from_rgb(252, 240, 110),
                );
                if close_up {
                    let doing = match h.current {
                        chronica_engine::humans::HumanAction::Gather => "gathering",
                        chronica_engine::humans::HumanAction::Drink { .. } => "→ water",
                        chronica_engine::humans::HumanAction::EatStored => "eating",
                        chronica_engine::humans::HumanAction::Hunt { .. } => "hunting!",
                        chronica_engine::humans::HumanAction::ChopWood => "chopping",
                        chronica_engine::humans::HumanAction::Build { .. } => "building",
                        chronica_engine::humans::HumanAction::Deposit => "storing",
                        chronica_engine::humans::HumanAction::Socialize { .. } => "talking",
                        chronica_engine::humans::HumanAction::Court { .. } => "courting",
                        chronica_engine::humans::HumanAction::TendFarm => "farming",
                        chronica_engine::humans::HumanAction::Rest => "resting",
                        chronica_engine::humans::HumanAction::Flee { .. } => "fleeing!",
                        chronica_engine::humans::HumanAction::MoveTo { .. } => "walking",
                        chronica_engine::humans::HumanAction::Idle => "idling",
                    };
                    painter.text(
                        p + egui::vec2(0.0, -self.zoom * 0.5),
                        egui::Align2::CENTER_BOTTOM,
                        format!("{} — {}", h.name, doing),
                        egui::FontId::proportional((self.zoom * 0.42).min(14.0)),
                        egui::Color32::WHITE,
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
