//! Stage 3 done-when: a predator/prey world shows population dynamics arising from individual
//! encounters, and every death has a cause event.

use chronica_engine::history::{Cause, EventKind};
use chronica_engine::sim::{Config, Sim};

#[test]
fn population_dynamics_from_individual_encounters() {
    let mut sim = Sim::new(Config { seed: 42, width: 192, height: 128 });
    sim.run_days(15 * 360);

    let births = sim
        .history
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Born { .. }))
        .count();
    assert!(births > 100, "animals must reproduce through real courtship, got {births} births");

    let kills: Vec<_> = sim
        .history
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Killed { .. }))
        .collect();
    assert!(!kills.is_empty(), "predation must occur through real pursuit");

    // every dead animal has exactly one Died event, and predation deaths chain to their kill
    let dead = sim.animals.list.iter().filter(|a| !a.alive).count();
    let died_events: Vec<_> = sim
        .history
        .events
        .iter()
        .filter(|e| {
            matches!(e.kind, EventKind::Died { .. })
                && matches!(e.subject, chronica_engine::core::ids::EntityRef::Animal(_))
        })
        .collect();
    assert_eq!(dead, died_events.len(), "every animal death is recorded");
    let mut chained = 0;
    for ev in &died_events {
        if matches!(
            ev.kind,
            EventKind::Died { cause: chronica_engine::history::DeathCause::Predation }
        ) {
            assert!(
                ev.causes.iter().any(|c| matches!(c, Cause::Event(_))),
                "predation death must cite the kill event"
            );
            chained += 1;
        }
    }
    assert!(chained > 0, "some predation deaths expected");

    // births reference real parents that exist
    for ev in sim.history.events.iter() {
        if let EventKind::Born { mother, .. } = ev.kind {
            if let chronica_engine::core::ids::EntityRef::Animal(m) = mother {
                assert!(m.index() < sim.animals.list.len(), "mother must be a real animal");
            }
        }
    }
}
