//! Query any entity / event / causal chain from a save. Grows with the inspection API.
//! Stage 1: events and why-chains only (the only entities that exist yet).

use chronica_engine::core::ids::EventId;
use chronica_engine::persistence;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let save = PathBuf::from(args.next().expect("usage: world_inspector SAVE [why EVENT_ID]"));
    let sim = persistence::load_from_file(&save).expect("load save");
    match args.next().as_deref() {
        Some("why") => {
            let id: u64 = args.next().expect("event id").parse().unwrap();
            for (depth, ev_id) in sim.history.why(EventId(id), 12) {
                let ev = sim.history.get(ev_id).unwrap();
                println!(
                    "{}{:?} (day {}, subject {:?})",
                    "  ".repeat(depth),
                    ev.kind,
                    ev.day,
                    ev.subject
                );
            }
        }
        _ => {
            println!("world at {}, {} events", sim.clock.date_string(), sim.history.events.len());
            for ev in sim.history.events.iter().rev().take(20) {
                println!("#{} day {} {:?} subject {:?}", ev.id.0, ev.day, ev.kind, ev.subject);
            }
        }
    }
}
