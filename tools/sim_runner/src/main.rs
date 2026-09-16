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
    for d in 0..args.days {
        sim.tick();
        if args.stats_every > 0 && (d + 1) % args.stats_every == 0 {
            print_stats(&sim);
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
}

fn print_stats(sim: &Sim) {
    let (rivers, lakes, mean_surface) = chronica_engine::water::water_stats(sim);
    eprintln!(
        "[{}] events={} river_cells={} lake_cells={} mean_surface={:.4}",
        sim.clock.date_string(),
        sim.history.events.len(),
        rivers,
        lakes,
        mean_surface,
    );
}
