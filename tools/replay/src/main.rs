//! Replay & diff: re-run a save's seed/config from scratch to the same day and compare, or
//! diff two saves event-by-event, reporting the first divergence.

use chronica_engine::persistence;
use chronica_engine::sim::Sim;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("verify") => {
            let save = PathBuf::from(args.next().expect("save path"));
            let loaded = persistence::load_from_file(&save).expect("load");
            let mut fresh = Sim::new(loaded.cfg.clone());
            fresh.run_days(loaded.clock.day);
            if fresh.state_hash() == loaded.state_hash() {
                println!("OK: replay reproduces the save bit-identically");
            } else {
                println!("DIVERGED: replay hash != save hash");
                first_event_divergence(&fresh, &loaded);
                std::process::exit(1);
            }
        }
        Some("diff") => {
            let a = persistence::load_from_file(&PathBuf::from(args.next().unwrap())).unwrap();
            let b = persistence::load_from_file(&PathBuf::from(args.next().unwrap())).unwrap();
            first_event_divergence(&a, &b);
        }
        _ => {
            eprintln!("usage: replay verify SAVE | replay diff SAVE_A SAVE_B");
            std::process::exit(2);
        }
    }
}

fn first_event_divergence(a: &Sim, b: &Sim) {
    let n = a.history.events.len().min(b.history.events.len());
    for i in 0..n {
        let (ea, eb) = (&a.history.events[i], &b.history.events[i]);
        if ea.kind != eb.kind || ea.day != eb.day || ea.subject != eb.subject {
            println!("first divergence at event #{i}:");
            println!("  A: day {} {:?} {:?}", ea.day, ea.subject, ea.kind);
            println!("  B: day {} {:?} {:?}", eb.day, eb.subject, eb.kind);
            return;
        }
    }
    if a.history.events.len() != b.history.events.len() {
        println!(
            "event logs identical for {n} events, lengths differ: {} vs {}",
            a.history.events.len(),
            b.history.events.len()
        );
    } else {
        println!("event logs identical ({n} events)");
    }
}
