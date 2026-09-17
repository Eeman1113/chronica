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
}

fn main() {
    let mut seed = 1u64;
    let mut port = 80u16;
    let mut save = String::from("/var/lib/chronica/world.crn");
    let mut days_per_min = 30u64; // ~43 sim-years per real year: a decade of watching ≈ 430 years
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
    std::fs::create_dir_all(std::path::Path::new(&save).parent().unwrap()).ok();

    // resume the same world if it exists — the whole point is continuity
    let save_path = std::path::PathBuf::from(&save);
    let mut sim = if save_path.exists() {
        eprintln!("resuming world from {save}");
        persistence::load_from_file(&save_path).expect("load save")
    } else {
        eprintln!("creating world (seed {seed})");
        Sim::new(Config { seed, width, height })
    };

    let view = Arc::new(RwLock::new(View { png: Vec::new(), html: String::new() }));
    {
        let mut v = view.write().unwrap();
        *v = render_view(&sim);
    }

    // ---- HTTP server thread ----
    let server_view = Arc::clone(&view);
    std::thread::spawn(move || {
        let server = tiny_http::Server::http(("0.0.0.0", port)).expect("bind http");
        eprintln!("serving on port {port}");
        for req in server.incoming_requests() {
            let url = req.url().to_string();
            let v = server_view.read().unwrap();
            let resp = if url.starts_with("/map.png") {
                tiny_http::Response::from_data(v.png.clone()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
                        .unwrap(),
                )
            } else {
                tiny_http::Response::from_data(v.html.clone().into_bytes()).with_header(
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

    // ---- the world, forever ----
    let tick_sleep = std::time::Duration::from_millis(60_000 / days_per_min.max(1));
    let mut since_render = 0u64;
    let mut since_save = 0u64;
    loop {
        let t0 = std::time::Instant::now();
        sim.tick();
        since_render += 1;
        since_save += 1;
        if since_render >= 3 {
            since_render = 0;
            let new_view = render_view(&sim);
            *view.write().unwrap() = new_view;
        }
        if since_save >= 180 {
            since_save = 0;
            // alternate files so a crash mid-write can never eat the world
            let tmp = save_path.with_extension("crn.tmp");
            if persistence::save_to_file(&sim, &tmp).is_ok() {
                let _ = std::fs::rename(&tmp, &save_path);
                eprintln!("[{}] autosaved", sim.clock.date_string());
            }
        }
        let spent = t0.elapsed();
        if spent < tick_sleep {
            std::thread::sleep(tick_sleep - spent);
        }
    }
}

fn render_view(sim: &Sim) -> View {
    View { png: render_png(sim), html: render_html(sim) }
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
    )
}
