//! Headless runner: `sim_runner --seed S --days D [--width W --height H] [--save PATH]
//! [--load PATH] [--threads N] [--hash] [--stats-every DAYS]`

use chronica_engine::persistence;
use chronica_engine::sim::{Config, Sim};
use std::path::PathBuf;

struct Args {
    seed: u64,
    days: u64,
    width: u32,
    height: u32,
    save: Option<PathBuf>,
    load: Option<PathBuf>,
    threads: Option<usize>,
    hash: bool,
    stats_every: u64,
}

fn parse_args() -> Args {
    let mut a = Args {
        seed: 1,
        days: 360,
        width: 192,
        height: 128,
        save: None,
        load: None,
        threads: None,
        hash: false,
        stats_every: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut val = || it.next().expect("missing value for flag");
        match k.as_str() {
            "--seed" => a.seed = val().parse().unwrap(),
            "--days" => a.days = val().parse().unwrap(),
            "--width" => a.width = val().parse().unwrap(),
            "--height" => a.height = val().parse().unwrap(),
            "--save" => a.save = Some(PathBuf::from(val())),
            "--load" => a.load = Some(PathBuf::from(val())),
            "--threads" => a.threads = Some(val().parse().unwrap()),
            "--hash" => a.hash = true,
            "--stats-every" => a.stats_every = val().parse().unwrap(),
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    a
}

fn main() {
    let args = parse_args();
    if let Some(t) = args.threads {
        rayon::ThreadPoolBuilder::new().num_threads(t).build_global().unwrap();
    }
    let mut sim = if let Some(p) = &args.load {
        persistence::load_from_file(p).expect("load save")
    } else {
        Sim::new(Config { seed: args.seed, width: args.width, height: args.height })
    };
    let t0 = std::time::Instant::now();
    let start_day = sim.clock.day;
    let probe = std::env::var("CHRONICA_PROBE").ok().and_then(|s| s.parse::<usize>().ok());
    for d in 0..args.days {
        sim.tick();
        if std::env::var("CHRONICA_PROBE_H").is_ok() {
            let probe_i: usize = std::env::var("CHRONICA_PROBE_H").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            if let Some((i, h)) = sim.humans.list.iter().enumerate().filter(|(i2, h)| h.alive && *i2 >= probe_i).next() {
                eprintln!(
                    "day {} #{} pos=({},{}) hun={:.2} thi={:.2} nut={:.2} sd={} food={:.2} act={:?} ch={}",
                    sim.clock.day, i, h.x, h.y, h.hunger, h.thirst, h.nutrition,
                    h.days_starving, h.carried_food, h.current, h.rationale.chosen
                );
            }
        }
        if let Some(pi) = probe {
            if let Some(a) = sim.animals.list.get(pi) {
                if a.alive && d < 60 {
                    eprintln!(
                        "day {} sp={} pos=({},{}) hun={:.2} thi={:.2} fat={:.2} cond={:.2} starve_d={} rat={:?}",
                        sim.clock.day,
                        chronica_engine::species::ANIMALS[a.species as usize].name,
                        a.x, a.y, a.hunger, a.thirst, a.fatigue, a.condition,
                        a.days_starving, a.rationale
                    );
                }
            }
        }
        if args.stats_every > 0 && (d + 1) % args.stats_every == 0 {
            print_stats(&sim);
            if std::env::var("CHRONICA_ENV").is_ok() {
                env_stats(&sim);
            }
        }
    }
    let dt = t0.elapsed();
    let days_run = sim.clock.day - start_day;
    eprintln!(
        "ran {} days in {:.2}s ({:.0} days/s) — {}",
        days_run,
        dt.as_secs_f64(),
        days_run as f64 / dt.as_secs_f64().max(1e-9),
        sim.clock.date_string()
    );
    print_stats(&sim);
    if let Some(p) = &args.save {
        persistence::save_to_file(&sim, p).expect("save");
        eprintln!("saved to {}", p.display());
    }
    if args.hash {
        println!("{:016x}", sim.state_hash());
    }
    if std::env::var("CHRONICA_CAUSES").is_ok() {
        use chronica_engine::history::EventKind;
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for ev in &sim.history.events {
            let is_person = matches!(ev.subject, chronica_engine::core::ids::EntityRef::Person(_));
            let k = match &ev.kind {
                EventKind::PlantDied { cause } => format!("PlantDied:{cause:?}"),
                EventKind::Died { cause } if is_person => format!("HumanDied:{cause:?}"),
                EventKind::Died { cause } => format!("Died:{cause:?}"),
                other if is_person => format!("H:{}", kind_name(other)),
                other => format!("{}", kind_name(other)),
            };
            *counts.entry(k).or_default() += 1;
        }
        for (k, c) in counts {
            eprintln!("  {k}: {c}");
        }
    }
}

#[allow(dead_code)]
fn env_stats(sim: &Sim) {
    let mut ms: Vec<f32> = Vec::new();
    let mut ts: Vec<f32> = Vec::new();
    for i in 0..sim.grid.n() {
        if sim.grid.ocean[i] {
            continue;
        }
        ms.push(chronica_engine::vegetation::cell_moisture(sim, i));
        ts.push(sim.grid.temp[i]);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |v: &Vec<f32>, p: f64| v[(v.len() as f64 * p) as usize];
    eprintln!(
        "  moisture p5={:.2} p25={:.2} p50={:.2} p75={:.2} p95={:.2} | temp p5={:.1} p25={:.1} p50={:.1} p75={:.1} p95={:.1}",
        pct(&ms, 0.05), pct(&ms, 0.25), pct(&ms, 0.5), pct(&ms, 0.75), pct(&ms, 0.95),
        pct(&ts, 0.05), pct(&ts, 0.25), pct(&ts, 0.5), pct(&ts, 0.75), pct(&ts, 0.95)
    );
}

fn kind_name(k: &chronica_engine::history::EventKind) -> &'static str {
    use chronica_engine::history::EventKind::*;
    match k {
        WorldGenerated => "WorldGenerated",
        LightningStrike => "LightningStrike",
        FireIgnited => "FireIgnited",
        FireSpread { .. } => "FireSpread",
        FireDied => "FireDied",
        Born { .. } => "Born",
        Killed { .. } => "Killed",
        Ate { .. } => "Ate",
        Mated { .. } => "Mated",
        _ => "Other",
    }
}

fn print_stats(sim: &Sim) {
    let (rivers, lakes, mean_surface) = chronica_engine::water::water_stats(sim);
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let pops = chronica_engine::animals::population_by_species(sim);
    let pop_str: Vec<String> =
        pops.iter().filter(|(_, c)| *c > 0).map(|(n, c)| format!("{n}:{c}")).collect();
    eprintln!(
        "[{}] events={} rivers={} lakes={} surf={:.3} plants={} trees={} cover={:.2} people={} | {}",
        sim.clock.date_string(),
        sim.history.events.len(),
        rivers,
        lakes,
        mean_surface,
        live_plants,
        trees,
        cover,
        chronica_engine::humans::population(sim),
        pop_str.join(" "),
    );
}
