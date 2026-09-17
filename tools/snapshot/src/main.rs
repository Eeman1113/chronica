//! Headless map snapshots: renders true world state to PPM images (convert to PNG with sips).
//! Read-only over the same state the renderer reads. Usage:
//!   snapshot --seed S --days D --out prefix [--scale N]
//! Writes: prefix_map.ppm (terrain/water/veg/fire/entities), prefix_water.ppm (hydrology lens),
//!         prefix_veg.ppm (vegetation lens)

use chronica_engine::sim::{Config, Sim};
use std::io::Write;

fn write_ppm(path: &str, w: usize, h: usize, px: &[(u8, u8, u8)], scale: usize) {
    let mut f = std::fs::File::create(path).expect("create ppm");
    write!(f, "P6\n{} {}\n255\n", w * scale, h * scale).unwrap();
    let mut buf = Vec::with_capacity(w * h * scale * scale * 3);
    for y in 0..h * scale {
        for x in 0..w * scale {
            let (r, g, b) = px[(y / scale) * w + (x / scale)];
            buf.extend_from_slice(&[r, g, b]);
        }
    }
    f.write_all(&buf).unwrap();
}

fn main() {
    let mut seed = 2024u64;
    let mut days = 0u64;
    let mut out = String::from("snap");
    let mut scale = 6usize;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().expect("value");
        match k.as_str() {
            "--seed" => seed = v().parse().unwrap(),
            "--days" => days = v().parse().unwrap(),
            "--out" => out = v(),
            "--scale" => scale = v().parse().unwrap(),
            _ => {}
        }
    }
    let mut sim = Sim::new(Config { seed, width: 192, height: 128 });
    sim.run_days(days);
    let g = &sim.grid;
    let (w, h) = (g.w as usize, g.h as usize);

    // --- main map: terrain, derived water, vegetation, fire, entities ---
    let mut px = vec![(0u8, 0u8, 0u8); w * h];
    for i in 0..w * h {
        px[i] = if g.ocean[i] {
            (16, 32, 64)
        } else if g.is_river(i) || g.is_lake(i) {
            (52, 96, 168)
        } else if g.burning[i] > 0 {
            (235, 96, 32)
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
            px[i] = if sp.prey.is_empty() { (205, 175, 132) } else { (225, 80, 80) };
        }
    }
    for hu in sim.humans.list.iter().filter(|h| h.alive) {
        if let Some(i) = g.idx(hu.x, hu.y) {
            px[i] = (252, 240, 110);
        }
    }
    write_ppm(&format!("{out}_map.ppm"), w, h, &px, scale);

    // --- hydrology lens: groundwater fill + surface water + flow ---
    let mut px2 = vec![(0u8, 0u8, 0u8); w * h];
    for i in 0..w * h {
        px2[i] = if g.ocean[i] {
            (10, 18, 40)
        } else if g.is_river(i) {
            (120, 190, 255)
        } else if g.is_lake(i) {
            (70, 130, 220)
        } else {
            let fill = (g.ground[i] / (0.2 + g.soil_depth[i] * 0.8)).clamp(0.0, 1.0);
            ((20.0 + 30.0 * fill) as u8, (24.0 + 60.0 * fill) as u8, (36.0 + 160.0 * fill) as u8)
        };
    }
    write_ppm(&format!("{out}_water.ppm"), w, h, &px2, scale);

    // --- vegetation lens: per-cell biomass of real plants, trees emphasized ---
    let mut tree_b = vec![0.0f32; w * h];
    let mut herb_b = vec![0.0f32; w * h];
    for p in sim.plants.list.iter().filter(|p| p.alive) {
        let sp = &chronica_engine::species::PLANTS[p.species as usize];
        if sp.kind == chronica_engine::species::PlantKind::Tree {
            tree_b[p.cell as usize] += p.biomass;
        } else {
            herb_b[p.cell as usize] += p.biomass;
        }
    }
    let mut px3 = vec![(0u8, 0u8, 0u8); w * h];
    for i in 0..w * h {
        px3[i] = if g.ocean[i] {
            (12, 20, 44)
        } else {
            let t = (tree_b[i] / 8.0).min(1.0);
            let hb = (herb_b[i] / 1.5).min(1.0);
            ((30.0 + hb * 130.0) as u8, (40.0 + t * 150.0 + hb * 40.0) as u8, (28.0 + t * 40.0) as u8)
        };
    }
    write_ppm(&format!("{out}_veg.ppm"), w, h, &px3, scale);

    eprintln!(
        "snapshotted seed {seed} at day {days}: plants={} animals={} people={} events={}",
        sim.plants.list.iter().filter(|p| p.alive).count(),
        sim.animals.list.iter().filter(|a| a.alive).count(),
        chronica_engine::humans::population(&sim),
        sim.history.events.len()
    );
}
