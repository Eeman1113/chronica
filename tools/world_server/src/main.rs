//! The eternal world: a headless Chronica sim that runs forever and serves a live view over
//! HTTP. The sim thread owns the world (ticking at a configured pace, autosaving so restarts
//! resume the SAME world); the server thread serves the latest rendered map + chronicle.
//!
//!   world_server [--seed N] [--port N] [--save PATH] [--days-per-min N] [--width W --height H]

use chronica_engine::inspection;
use chronica_engine::persistence;
use chronica_engine::sim::{Config, Sim};
use std::sync::{Arc, RwLock};

struct View {
    png: Vec<u8>,
    html: String,
    state: String,
}

/// The seed chain: each world's seed is derived from the last — unique, never repeating,
/// reproducible. World N's seed is splitmix^N(genesis).
fn seed_for_epoch(genesis: u64, epoch: u64) -> u64 {
    let mut s = genesis;
    for _ in 0..epoch {
        s = chronica_engine::core::rng::splitmix64(s ^ 0xE7E5_11FE_57A1_D00D);
    }
    s
}


/// The live-stream client: a canvas renderer over /state.json — wheel-zoom at the cursor,
/// drag-pan, glyph view when close, all camera state preserved across the 5-second live poll.
const LIVE_HTML: &str = include_str!("viewer.html");
const FAVICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#241a14"/><rect x="1.5" y="1.5" width="29" height="29" rx="6" fill="none" stroke="#4a3d2a" stroke-width="1"/><text x="16" y="23" font-size="21" text-anchor="middle" fill="#d9b464" font-family="Georgia, serif">◈</text></svg>"##;
fn main() {
    let mut seed = 1u64;
    let mut port = 80u16;
    let mut save = String::from("/var/lib/chronica/world.crn");
    let mut days_per_min = 0u64; // 0 = as fast as the engine runs; the DVR decouples watching
    let mut width = 192u32;
    let mut height = 128u32;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().expect("value");
        match k.as_str() {
            "--seed" => seed = v().parse().unwrap(),
            "--port" => port = v().parse().unwrap(),
            "--save" => save = v(),
            "--days-per-min" => days_per_min = v().parse().unwrap(),
            "--width" => width = v().parse().unwrap(),
            "--height" => height = v().parse().unwrap(),
            _ => {}
        }
    }
    let data_dir = std::path::Path::new(&save).parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&data_dir).ok();
    let archive_dir = data_dir.join("archive");
    std::fs::create_dir_all(&archive_dir).ok();
    let registry_path = data_dir.join("worlds.log");
    let genesis = seed;

    // which epoch are we in? the registry of dead worlds remembers
    let mut epoch: u64 = std::fs::read_to_string(&registry_path)
        .map(|t| t.lines().count() as u64)
        .unwrap_or(0);

    // resume the same world if it exists — the whole point is continuity
    let save_path = std::path::PathBuf::from(&save);
    let mut sim = if save_path.exists() {
        eprintln!("resuming world (epoch {epoch}) from {save}");
        persistence::load_from_file(&save_path).expect("load save")
    } else {
        let s = seed_for_epoch(genesis, epoch);
        eprintln!("creating world {} (seed {s})", epoch + 1);
        Sim::new(Config { seed: s, width, height })
    };
    // when did the last creature die? (u64::MAX = life persists)
    let mut doomsday: u64 = u64::MAX;

    let view = Arc::new(RwLock::new(View { png: Vec::new(), html: String::new(), state: String::new() }));
    {
        let mut v = view.write().unwrap();
        *v = render_view(&sim);
    }
    // the current world's full binary (save + entire event history), refreshed each autosave
    let save_bytes: Arc<RwLock<Vec<u8>>> = Arc::new(RwLock::new(sim.to_bytes()));
    let cur_seed: Arc<RwLock<u64>> = Arc::new(RwLock::new(sim.cfg.seed));
    // the DVR: a rolling buffer of world-frames the client can replay at its own pace
    use std::collections::VecDeque;
    const FRAME_STRIDE: u64 = 2; // capture a frame every N sim-days
    const MAX_FRAMES: usize = 2400; // ~4800 sim-days of rewind (~13 years)
    let frames: Arc<RwLock<VecDeque<(u64, String)>>> = Arc::new(RwLock::new(VecDeque::new()));
    frames.write().unwrap().push_back((sim.clock.day, render_frame(&sim)));
    let meta: Arc<RwLock<String>> = Arc::new(RwLock::new(render_meta(&sim, sim.clock.day, sim.clock.day)));

    // ---- HTTP server thread ----
    let server_view = Arc::clone(&view);
    let server_save = Arc::clone(&save_bytes);
    let server_seed = Arc::clone(&cur_seed);
    let server_frames = Arc::clone(&frames);
    let server_meta = Arc::clone(&meta);
    std::thread::spawn(move || {
        let server = tiny_http::Server::http(("0.0.0.0", port)).expect("bind http");
        eprintln!("serving on port {port}");
        for req in server.incoming_requests() {
            let url = req.url().to_string();
            let v = server_view.read().unwrap();
            let resp = if url.starts_with("/archive/") {
                // the museum of dead worlds
                let name = url.trim_start_matches("/archive/");
                let safe = !name.contains("..") && !name.contains('/');
                let path = std::path::Path::new("/var/lib/chronica/archive").join(name);
                if safe && path.exists() {
                    let bytes = std::fs::read(&path).unwrap_or_default();
                    let ct: &[u8] = if name.ends_with(".png") {
                        b"image/png"
                    } else if name.ends_with(".html") {
                        b"text/html; charset=utf-8"
                    } else {
                        b"application/octet-stream"
                    };
                    let mut r = tiny_http::Response::from_data(bytes).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], ct).unwrap(),
                    );
                    if name.ends_with(".crn") {
                        r.add_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Disposition"[..],
                                format!("attachment; filename=\"{name}\"").as_bytes(),
                            )
                            .unwrap(),
                        );
                    }
                    r
                } else {
                    tiny_http::Response::from_data(b"gone".to_vec())
                }
            } else if url.starts_with("/download/current") {
                // the living world, whole: state + full causal history, one file
                let bytes = server_save.read().unwrap().clone();
                let seed = *server_seed.read().unwrap();
                let fname = format!("chronica_world_seed_{seed:016x}.crn");
                tiny_http::Response::from_data(bytes)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/octet-stream"[..],
                        )
                        .unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Disposition"[..],
                            format!("attachment; filename=\"{fname}\"").as_bytes(),
                        )
                        .unwrap(),
                    )
            } else if url.starts_with("/favicon") {
                tiny_http::Response::from_data(FAVICON.as_bytes().to_vec()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/svg+xml"[..])
                        .unwrap(),
                )
            } else if url.starts_with("/frame") {
                // the nearest recorded frame to ?day=D (default: newest)
                let want: Option<u64> = url
                    .split_once("day=")
                    .and_then(|(_, r)| r.split(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|d| d.parse().ok());
                let fr = server_frames.read().unwrap();
                let body = if let Some(d) = want {
                    fr.iter()
                        .min_by_key(|(fd, _)| (*fd as i64 - d as i64).abs())
                        .map(|(_, j)| j.clone())
                } else {
                    fr.back().map(|(_, j)| j.clone())
                }
                .unwrap_or_else(|| "{}".to_string());
                tiny_http::Response::from_data(body.into_bytes()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                )
            } else if url.starts_with("/live.json") {
                tiny_http::Response::from_data(server_meta.read().unwrap().clone().into_bytes())
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                            .unwrap(),
                    )
            } else if url.starts_with("/classic") {
                tiny_http::Response::from_data(v.html.clone().into_bytes()).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap(),
                )
            } else if url.starts_with("/map.png") {
                tiny_http::Response::from_data(v.png.clone()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
                        .unwrap(),
                )
            } else {
                tiny_http::Response::from_data(LIVE_HTML.as_bytes().to_vec()).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap(),
                )
            };
            let _ = req.respond(resp);
        }
    });

    // ---- the worlds, forever: each runs until a century after its last creature ----
    // days_per_min == 0 → run as fast as the engine can; the DVR decouples watching from living
    let tick_sleep = if days_per_min == 0 {
        std::time::Duration::ZERO
    } else {
        std::time::Duration::from_millis(60_000 / days_per_min.max(1))
    };
    let mut last_frame_day = sim.clock.day;
    let mut last_save = std::time::Instant::now();
    let mut last_meta = std::time::Instant::now();
    loop {
        let t0 = std::time::Instant::now();
        sim.tick();

        // capture a frame into the DVR buffer every few sim-days
        if sim.clock.day.saturating_sub(last_frame_day) >= FRAME_STRIDE {
            last_frame_day = sim.clock.day;
            let f = render_frame(&sim);
            let mut fr = frames.write().unwrap();
            fr.push_back((sim.clock.day, f));
            while fr.len() > MAX_FRAMES {
                fr.pop_front();
            }
        }
        // refresh the live sidebar a few times a second (real time), not per sim-tick
        if last_meta.elapsed() >= std::time::Duration::from_millis(500) {
            last_meta = std::time::Instant::now();
            let oldest = frames.read().unwrap().front().map(|(d, _)| *d).unwrap_or(sim.clock.day);
            *meta.write().unwrap() = render_meta(&sim, oldest, sim.clock.day);
        }

        // watch for the death of the last creature (plants alone don't count)
        let creatures = chronica_engine::humans::population(&sim)
            + sim.animals.list.iter().filter(|a| a.alive).count();
        if creatures == 0 {
            if doomsday == u64::MAX {
                doomsday = sim.clock.day;
                eprintln!(
                    "[{}] the last creature has died — the plants inherit the world for 10 years",
                    sim.clock.date_string()
                );
            }
        } else {
            doomsday = u64::MAX;
        }

        // a century of silence, then the archive and a new genesis
        if doomsday != u64::MAX && sim.clock.day >= doomsday + 10 * 360 {
            let world_no = epoch + 1;
            let this_seed = sim.cfg.seed;
            let year = sim.clock.day / 360;
            eprintln!("world {world_no} ends in year {year}; archiving…");
            let base = format!("world_{world_no:03}_seed_{this_seed:016x}_year_{year}");
            let _ = persistence::save_to_file(&sim, &archive_dir.join(format!("{base}.crn")));
            let _ = std::fs::write(archive_dir.join(format!("{base}.png")), render_png(&sim));
            let _ = std::fs::write(archive_dir.join(format!("{base}.html")), render_html(&sim));
            // one line per world, forever
            let line = format!(
                "{world_no}	{this_seed:016x}	{year}	{}	{}
",
                sim.history.events.len(),
                base
            );
            use std::io::Write as _;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&registry_path)
            {
                let _ = f.write_all(line.as_bytes());
            }
            // keep every world's portrait, final page, and registry line forever; but the
            // heavy full-save .crn we retain only for the most recent worlds (disk is finite)
            if let Ok(rd) = std::fs::read_dir(&archive_dir) {
                let mut crns: Vec<(std::time::SystemTime, std::path::PathBuf)> = rd
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|x| x == "crn").unwrap_or(false))
                    .filter_map(|e| e.metadata().ok().and_then(|m| m.modified().ok()).map(|t| (t, e.path())))
                    .collect();
                crns.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
                for (_, path) in crns.into_iter().skip(60) {
                    let _ = std::fs::remove_file(path);
                }
            }
            let _ = std::fs::remove_file(&save_path);
            epoch += 1;
            let s = seed_for_epoch(genesis, epoch);
            eprintln!("world {} begins (seed {s})", epoch + 1);
            sim = Sim::new(Config { seed: s, width, height });
            doomsday = u64::MAX;
            *save_bytes.write().unwrap() = sim.to_bytes();
            *cur_seed.write().unwrap() = sim.cfg.seed;
            {
                let mut fr = frames.write().unwrap();
                fr.clear();
                fr.push_back((sim.clock.day, render_frame(&sim)));
            }
            last_frame_day = sim.clock.day;
            *meta.write().unwrap() = render_meta(&sim, sim.clock.day, sim.clock.day);
            *view.write().unwrap() = render_view(&sim);
            continue;
        }
        // autosave by real time (the sim may be doing thousands of days a second)
        if last_save.elapsed() >= std::time::Duration::from_secs(20) {
            last_save = std::time::Instant::now();
            let tmp = save_path.with_extension("crn.tmp");
            if persistence::save_to_file(&sim, &tmp).is_ok() {
                let _ = std::fs::rename(&tmp, &save_path);
                *save_bytes.write().unwrap() = sim.to_bytes();
                eprintln!("[{}] autosaved", sim.clock.date_string());
            }
        }
        if tick_sleep > std::time::Duration::ZERO {
            let spent = t0.elapsed();
            if spent < tick_sleep {
                std::thread::sleep(tick_sleep - spent);
            }
        }
    }
}

fn render_view(sim: &Sim) -> View {
    View { png: render_png(sim), html: render_html(sim), state: String::new() }
}

/// One cell, classified for the client renderer (mirror of the desktop glyph logic).
fn classify_cell(sim: &Sim, i: usize) -> (u8, u8) {
    let g = &sim.grid;
    let shade = ((chronica_engine::core::rng::splitmix64(i as u64) >> 32) % 4) as u8;
    if g.ocean[i] {
        return (if g.elev[i] < -0.45 { 1 } else { 0 }, shade);
    }
    if g.ice_bears(i) {
        return (2, shade);
    }
    if g.burning[i] > 0 {
        return (3, shade);
    }
    if g.surface[i] > 0.12 {
        return (4, shade); // flood/lake
    }
    if g.is_river(i) {
        return (5, shade);
    }
    if g.snow[i] > 0.02 {
        return (6, shade);
    }
    if g.elev[i] > 1.02 {
        return (7, shade);
    }
    if g.elev[i] > 0.82 {
        return (8, shade);
    }
    if g.crop_cover.get(i).copied().unwrap_or(0.0) > 0.05 {
        return (9, shade); // field
    }
    if g.is_path(i) {
        return (10, shade);
    }
    if g.burn_scar[i] > 0.3 {
        return (11, shade);
    }
    // dominant plant
    let (mut oak, mut pine, mut reed, mut shrub, mut grass) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if let Some(pids) = sim.plants.by_cell.get(i) {
        for &pi in pids {
            let pl = &sim.plants.list[pi as usize];
            if !pl.alive {
                continue;
            }
            match pl.species {
                chronica_engine::species::SP_OAK => oak += pl.biomass,
                chronica_engine::species::SP_PINE => pine += pl.biomass,
                chronica_engine::species::SP_REED => reed += pl.biomass,
                chronica_engine::species::SP_SCRUB => shrub += pl.biomass,
                chronica_engine::species::SP_WHEAT => {}
                _ => grass += pl.biomass,
            }
        }
    }
    if oak > 1.2 || pine > 1.2 {
        return (if pine > oak { 13 } else { 12 }, shade);
    }
    if oak + pine > 0.25 {
        return (14, shade);
    }
    if reed > 0.15 {
        return (15, shade);
    }
    if shrub > 0.25 {
        return (16, shade);
    }
    if grass > 0.5 {
        return (17, shade);
    }
    if grass > 0.10 {
        return (18, shade);
    }
    (19, shade)
}

fn render_frame(sim: &Sim) -> String {
    use base64::Engine as _;
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let g = &sim.grid;
    let n = g.n();
    let mut cells = Vec::with_capacity(n);
    for i in 0..n {
        let (c, sh) = classify_cell(sim, i);
        cells.push(c | (sh << 6));
    }
    let cells_b64 = base64::engine::general_purpose::STANDARD.encode(&cells);

    let day = sim.clock.day;
    let people: Vec<serde_json::Value> = sim
        .humans
        .list
        .iter()
        .filter(|h| h.alive)
        .map(|h| {
            let doing = match h.current {
                chronica_engine::humans::HumanAction::Gather => "gathering",
                chronica_engine::humans::HumanAction::Drink { .. } => "going to water",
                chronica_engine::humans::HumanAction::EatStored => "eating",
                chronica_engine::humans::HumanAction::Hunt { .. } => "hunting!",
                chronica_engine::humans::HumanAction::ChopWood => "chopping wood",
                chronica_engine::humans::HumanAction::Build { .. } => "building",
                chronica_engine::humans::HumanAction::Deposit => "storing food",
                chronica_engine::humans::HumanAction::Socialize { .. } => "talking",
                chronica_engine::humans::HumanAction::Court { .. } => "courting",
                chronica_engine::humans::HumanAction::TendFarm => "farming",
                chronica_engine::humans::HumanAction::PreserveFood => "smoking food",
                chronica_engine::humans::HumanAction::Fish { .. } => "fishing",
                chronica_engine::humans::HumanAction::StakeBait => "setting bait",
                chronica_engine::humans::HumanAction::LieInWait => "lying in wait",
                chronica_engine::humans::HumanAction::Rest => "resting",
                chronica_engine::humans::HumanAction::Flee { .. } => "fleeing!",
                chronica_engine::humans::HumanAction::MoveTo { .. } => "walking",
                chronica_engine::humans::HumanAction::Idle => "idling",
            };
            let child = (day as i64 - h.born) < 14 * 360;
            serde_json::json!({"x":h.x,"y":h.y,"n":h.name,"c":h.culture,"a":doing,"k":child})
        })
        .collect();
    let animals: Vec<serde_json::Value> = sim
        .animals
        .list
        .iter()
        .filter(|a| a.alive)
        .map(|a| {
            let doing = match a.rationale.chosen {
                0 => "fleeing!",
                1 => "to water",
                2 => "grazing",
                3 => "hunting!",
                4 => "courting",
                5 => "with herd",
                6 => "resting",
                _ => "roaming",
            };
            serde_json::json!({"x":a.x,"y":a.y,"s":a.species,"a":doing,"t":!a.tamed_by.is_none()})
        })
        .collect();
    let buildings: Vec<serde_json::Value> = sim
        .objects
        .buildings
        .iter()
        .filter(|b| b.exists || b.progress >= 1.0)
        .map(|b| {
            let (x, y) = g.xy(b.cell as usize);
            let k = match b.kind {
                chronica_engine::objects::BuildingKind::Hut => 0,
                chronica_engine::objects::BuildingKind::Granary => 1,
                chronica_engine::objects::BuildingKind::Hall => 2,
                chronica_engine::objects::BuildingKind::Palisade => 3,
            };
            serde_json::json!({"x":x,"y":y,"k":k,"p":(b.progress*100.0) as u32,
                "r":b.exists,"s":(b.food_store+b.preserved_store) as u32})
        })
        .collect();
    let corpses: Vec<serde_json::Value> = sim
        .animals
        .corpses
        .iter()
        .filter(|c| !c.gone)
        .map(|c| {
            let (x, y) = g.xy(c.cell as usize);
            serde_json::json!({"x":x,"y":y,"s":c.species,
                "g":chronica_engine::animals::corpse_stage(c.rot),"b":c.bait})
        })
        .collect();
    let setts: Vec<serde_json::Value> = chronica_engine::society::living_settlements(sim)
        .into_iter()
        .map(|(nm, (x, y))| serde_json::json!({"n":nm,"x":x,"y":y}))
        .collect();
    // recent marks
    let mut marks = Vec::new();
    for ev in sim.history.events.iter().rev().take(600) {
        if day.saturating_sub(ev.day) > 3 {
            break;
        }
        let Some(loc) = ev.loc else { continue };
        let (x, y) = g.xy(loc as usize);
        let t = match &ev.kind {
            EK::Killed { .. } => 0,
            EK::RaidCarriedOut { .. } => 1,
            EK::Born { .. } => 2,
            EK::HarvestedFood { .. } => 3,
            EK::FelledTree { .. } => 4,
            _ => continue,
        };
        marks.push(serde_json::json!({"x":x,"y":y,"t":t}));
    }
    // stats + chronicle + faiths + past worlds (client renders panels)
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = Vec::new();
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push(inspection::describe(sim, ev));
            if chron.len() >= 30 {
                break;
            }
        }
    }
    let mut species_counts = Vec::new();
    for (nm, c) in chronica_engine::animals::population_by_species(sim) {
        species_counts.push(serde_json::json!({"n":nm,"c":c}));
    }
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let faiths: Vec<serde_json::Value> = chronica_engine::society::belief_clusters(sim)
        .into_iter()
        .map(|(nm, c)| serde_json::json!({"n":nm,"c":c}))
        .collect();
    let past: Vec<serde_json::Value> = std::fs::read_to_string("/var/lib/chronica/worlds.log")
        .unwrap_or_default()
        .lines()
        .rev()
        .take(50)
        .filter_map(|l| {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 5 {
                Some(serde_json::json!({"no":p[0],"seed":p[1],"year":p[2],"events":p[3],"base":p[4]}))
            } else {
                None
            }
        })
        .collect();

    serde_json::json!({
        "w": g.w, "h": g.h, "day": day, "date": sim.clock.date_string(),
        "cells": cells_b64,
        "people": people, "animals": animals, "buildings": buildings,
        "setts": setts, "marks": marks, "corpses": corpses
    })
    .to_string()
}

/// The live sidebar + DVR buffer range: what the world is *now*, plus how far back you can rewind.
fn render_meta(sim: &Sim, oldest: u64, newest: u64) -> String {
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = Vec::new();
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push(inspection::describe(sim, ev));
            if chron.len() >= 30 {
                break;
            }
        }
    }
    let mut species_counts = Vec::new();
    for (nm, c) in chronica_engine::animals::population_by_species(sim) {
        species_counts.push(serde_json::json!({"n":nm,"c":c}));
    }
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let faiths: Vec<serde_json::Value> = chronica_engine::society::belief_clusters(sim)
        .into_iter()
        .map(|(nm, c)| serde_json::json!({"n":nm,"c":c}))
        .collect();
    let past: Vec<serde_json::Value> = std::fs::read_to_string("/var/lib/chronica/worlds.log")
        .unwrap_or_default()
        .lines()
        .rev()
        .take(50)
        .filter_map(|l| {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 5 {
                Some(serde_json::json!({"no":p[0],"seed":p[1],"year":p[2],"events":p[3],"base":p[4]}))
            } else {
                None
            }
        })
        .collect();
    serde_json::json!({
        "date": sim.clock.date_string(), "liveDay": sim.clock.day,
        "oldestDay": oldest, "newestDay": newest,
        "stats": {"people": chronica_engine::humans::population(sim),
                   "kids": sim.humans.list.iter().filter(|h| h.alive && (sim.clock.day as i64 - h.born) < 14*360).count(),
                   "species": species_counts, "plants": live_plants,
                   "trees": trees, "cover": cover, "events": sim.history.events.len()},
        "faiths": faiths, "past": past, "chron": chron
    })
    .to_string()
}

fn render_png(sim: &Sim) -> Vec<u8> {
    let g = &sim.grid;
    let (w, h) = (g.w as usize, g.h as usize);
    let scale = 5usize;
    let mut px = vec![(0u8, 0u8, 0u8); w * h];
    for i in 0..w * h {
        px[i] = if g.ocean[i] {
            (16, 32, 64)
        } else if g.ice_bears(i) {
            (185, 210, 235)
        } else if g.is_river(i) || g.is_lake(i) {
            (52, 96, 168)
        } else if g.burning[i] > 0 {
            (235, 96, 32)
        } else if g.crop_cover.get(i).copied().unwrap_or(0.0) > 0.15 {
            (212, 178, 70)
        } else if g.is_path(i) {
            (150, 128, 96)
        } else if g.snow[i] > 0.02 {
            (222, 226, 234)
        } else if g.burn_scar[i] > 0.3 {
            (70, 60, 52)
        } else {
            let elev = g.elev[i].clamp(0.0, 1.2);
            let cover = g.veg_cover.get(i).copied().unwrap_or(0.0).min(2.5) / 2.5;
            (
                (96.0 + elev * 90.0 - cover * 60.0).clamp(0.0, 255.0) as u8,
                (92.0 + cover * 110.0 + elev * 20.0).clamp(0.0, 255.0) as u8,
                (58.0 + elev * 40.0 - cover * 30.0).clamp(0.0, 255.0) as u8,
            )
        };
    }
    for b in sim.objects.buildings.iter().filter(|b| b.exists) {
        px[b.cell as usize] = (168, 124, 70);
    }
    for a in sim.animals.list.iter().filter(|a| a.alive) {
        if let Some(i) = g.idx(a.x, a.y) {
            let sp = &chronica_engine::species::ANIMALS[a.species as usize];
            if !sp.aquatic {
                px[i] = if sp.prey.is_empty() { (205, 175, 132) } else { (225, 80, 80) };
            }
        }
    }
    for hu in sim.humans.list.iter().filter(|h| h.alive) {
        if let Some(i) = g.idx(hu.x, hu.y) {
            px[i] = (252, 240, 110);
        }
    }
    // upscale + encode png
    let (ow, oh) = (w * scale, h * scale);
    let mut rgb = Vec::with_capacity(ow * oh * 3);
    for y in 0..oh {
        for x in 0..ow {
            let (r, gg, b) = px[(y / scale) * w + (x / scale)];
            rgb.extend_from_slice(&[r, gg, b]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, ow as u32, oh as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&rgb).unwrap();
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

fn past_worlds_html() -> String {
    let Ok(t) = std::fs::read_to_string("/var/lib/chronica/worlds.log") else {
        return String::new();
    };
    let mut out = String::new();
    for line in t.lines().rev().take(50) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 5 {
            out.push_str(&format!(
                "<li>World {} — seed <code>{}</code> — ended year {} — {} events — <a href=\"/archive/{}.png\">portrait</a> · <a href=\"/archive/{}.html\">final page</a></li>",
                parts[0], parts[1], parts[2], parts[3], parts[4], parts[4]
            ));
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("<h2>Worlds that were</h2><ul>{out}</ul>")
    }
}

fn render_html(sim: &Sim) -> String {
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let people = chronica_engine::humans::population(sim);
    let children = sim
        .humans
        .list
        .iter()
        .filter(|h| h.alive && (sim.clock.day as i64 - h.born) < 14 * 360)
        .count();
    let mut pops = String::new();
    for (name, c) in chronica_engine::animals::population_by_species(sim) {
        if c > 0 {
            pops.push_str(&format!("<li>{name}: {c}</li>"));
        }
    }
    let extinct: Vec<String> = chronica_engine::animals::population_by_species(sim)
        .into_iter()
        .filter(|(_, c)| *c == 0)
        .map(|(n, _)| n)
        .collect();
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let mut setts = String::new();
    for (name, (x, y)) in chronica_engine::society::living_settlements(sim) {
        setts.push_str(&format!("<li>{} at ({x},{y})</li>", esc(&name)));
    }
    let mut faiths = String::new();
    let mut forgotten = 0;
    for (name, c) in chronica_engine::society::belief_clusters(sim) {
        if c > 0 {
            faiths.push_str(&format!("<li>{}: {c} faithful</li>", esc(&name)));
        } else {
            forgotten += 1;
        }
    }
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = String::new();
    let mut shown = 0;
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push_str(&format!("<li>{}</li>", esc(&inspection::describe(sim, ev))));
            shown += 1;
            if shown >= 30 {
                break;
            }
        }
    }
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="refresh" content="6">
<title>Chronica — {date}</title>
<style>
body{{background:#14100e;color:#cfc4a6;font-family:'Iowan Old Style',Georgia,serif;margin:0;padding:24px;display:flex;gap:28px;flex-wrap:wrap}}
h1{{font-size:22px;color:#e8dcbc;margin:0 0 4px}} h2{{font-size:15px;color:#b9a2d6;margin:16px 0 4px}}
.map img{{image-rendering:pixelated;border:1px solid #3a2f26;max-width:min(960px,95vw)}}
ul{{margin:4px 0;padding-left:18px;font-size:13px;line-height:1.5}}
.small{{color:#8d8065;font-size:12px}}
.col{{min-width:260px;max-width:420px}}
.chron li{{color:#a8b6a0}}
</style></head><body>
<div class="map">
<h1>CHRONICA — the eternal world</h1>
<div class="small">{date} · this world has been alive since its seed · page refreshes itself</div>
<a href="/map.png"><img src="/map.png?t={day}" alt="the world"></a>
<div class="small">yellow people · tan herds · red predators · gold fields · brown roads &amp; homes · white ice</div>
</div>
<div class="col">
<h2>The living</h2>
<ul><li><b>People: {people}</b> (of whom {children} children)</li>{pops}
<li>Plants: {live_plants} ({trees} grown trees, {coverpct:.0}% forest)</li></ul>
{extinct_html}
<h2>Settlements</h2><ul>{setts_html}</ul>
<h2>Faiths</h2><ul>{faiths}{forgotten_html}</ul>
</div>
<div class="col chron">
<h2>The chronicle (latest notable)</h2><ul>{chron}</ul>
<div class="small">events recorded since the world began: {events}</div>
{past}
</div>
</body></html>"#,
        date = sim.clock.date_string(),
        day = sim.clock.day,
        people = people,
        children = children,
        pops = pops,
        live_plants = live_plants,
        trees = trees,
        coverpct = cover * 100.0,
        extinct_html = if extinct.is_empty() {
            String::new()
        } else {
            format!(
                "<div class=\"small\">gone from the world: {}</div>",
                esc(&extinct.join(", "))
            )
        },
        setts_html = if setts.is_empty() {
            "<li class=\"small\">no settlement yet bears a name</li>".to_string()
        } else {
            setts
        },
        faiths = faiths,
        forgotten_html = if forgotten > 0 {
            format!(
                "<li class=\"small\">…and {forgotten} faiths whose last believer is gone</li>"
            )
        } else {
            String::new()
        },
        chron = chron,
        events = sim.history.events.len(),
        past = past_worlds_html(),
    )
}
