//! Query any entity / event / causal chain from a save, in chronicle voice.
//! Usage:
//!   world_inspector SAVE                      — latest chronicle lines
//!   world_inspector SAVE why EVENT_ID         — causal chain to physical/social state
//!   world_inspector SAVE person INDEX         — a person's full derived biography
//!   world_inspector SAVE settlements|beliefs  — derived observations

use chronica_engine::core::ids::{EntityRef, EventId, PersonId};
use chronica_engine::inspection;
use chronica_engine::persistence;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let save = PathBuf::from(args.next().expect("usage: world_inspector SAVE [cmd ...]"));
    let sim = persistence::load_from_file(&save).expect("load save");
    match args.next().as_deref() {
        Some("why") => {
            let id: u64 = args.next().expect("event id").parse().unwrap();
            for line in inspection::why_text(&sim, EventId(id), 16) {
                println!("{line}");
            }
        }
        Some("person") => {
            let idx: usize = args.next().expect("person index").parse().unwrap();
            let h = &sim.humans.list[idx];
            println!(
                "{} — culture {}, born day {}, {}",
                h.name,
                h.culture,
                h.born,
                if h.alive { "living" } else { "dead" }
            );
            println!("skills: forage {:.2} hunt {:.2} farm {:.2}", h.skills[0], h.skills[1], h.skills[2]);
            println!("techniques: {:?}", h.techs);
            println!("memories: {}", h.memories.len());
            println!("--- biography (derived from events) ---");
            for line in inspection::biography(&sim, EntityRef::Person(PersonId::from_index(idx))) {
                println!("  {line}");
            }
        }
        Some("settlements") => {
            for (name, (x, y)) in chronica_engine::society::living_settlements(&sim) {
                println!("{name} at ({x},{y})");
            }
        }
        Some("beliefs") => {
            for (name, n) in chronica_engine::society::belief_clusters(&sim) {
                println!("{name}: {n} faithful");
            }
        }
        _ => {
            println!(
                "world at {}, {} events, {} people alive",
                sim.clock.date_string(),
                sim.history.events.len(),
                chronica_engine::humans::population(&sim)
            );
            for ev in sim.history.events.iter().rev().take(30).collect::<Vec<_>>().iter().rev() {
                println!("#{}  {}", ev.id.0, inspection::describe(&sim, ev));
            }
        }
    }
}
