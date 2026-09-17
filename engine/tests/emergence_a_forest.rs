//! Test A (Forest): forest statistics are derived from tree entities; removing trees reduces
//! cover *because* they were removed, each with a caused death event.

use chronica_engine::history::{Cause, DeathCause, EventKind};
use chronica_engine::sim::{Config, Sim};
use chronica_engine::species::{PlantKind, PLANTS};
use chronica_engine::vegetation::{forest_stats, kill_plant, refresh_cover};

#[test]
fn forest_cover_is_derived_and_felling_reduces_it() {
    let mut sim = Sim::new(Config { seed: 777, width: 192, height: 128 });

    let (live0, trees0, cover0) = forest_stats(&sim);
    assert!(trees0 >= 10_000, "world should start with ≥10k trees, got {trees0}");
    assert!(live0 > trees0);

    // derivation check: the stat equals a manual count over entities
    let manual: usize = sim
        .plants
        .list
        .iter()
        .filter(|p| {
            p.alive && PLANTS[p.species as usize].kind == PlantKind::Tree && p.biomass > 0.5
        })
        .count();
    assert_eq!(trees0, manual, "forest stat must be count(tree entities)");

    sim.run_days(360);
    let (_, trees1, cover1) = forest_stats(&sim);
    assert!(trees1 > 5_000, "forest must persist through a year, got {trees1}");

    // fell 60% of living trees, each a caused event
    let victims: Vec<usize> = sim
        .plants
        .list
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            p.alive && PLANTS[p.species as usize].kind == PlantKind::Tree && p.biomass > 0.5
        })
        .map(|(i, _)| i)
        .collect();
    let n_fell = victims.len() * 6 / 10;
    for &pi in victims.iter().take(n_fell) {
        kill_plant(&mut sim, pi, DeathCause::Felled, vec![]);
    }
    refresh_cover(&mut sim);

    let (_, trees2, cover2) = forest_stats(&sim);
    assert_eq!(trees2, trees1 - n_fell, "tree count drops exactly by the trees removed");
    assert!(
        cover2 < cover1 * 0.6,
        "cover must fall because trees were removed: {cover1} -> {cover2}"
    );
    let _ = cover0;

    // every felled tree has a PlantDied{Felled} event
    let felled_events = sim
        .history
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::PlantDied { cause: DeathCause::Felled }))
        .count();
    assert_eq!(felled_events, n_fell);

    // and in general: every dead plant has a death event with its cause
    let dead_plants = sim.plants.list.iter().filter(|p| !p.alive).count();
    let plant_death_events = sim
        .history
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::PlantDied { .. }))
        .count();
    assert_eq!(dead_plants, plant_death_events, "every plant death is recorded with a cause");
}

#[test]
fn plant_deaths_carry_causal_state() {
    let mut sim = Sim::new(Config { seed: 31, width: 96, height: 64 });
    sim.run_days(2 * 360);
    // at least some natural deaths occurred, and non-age deaths carry a StateRef or Event cause
    let mut checked = 0;
    for ev in &sim.history.events {
        if let EventKind::PlantDied { cause } = ev.kind {
            if cause != DeathCause::Age {
                assert!(
                    !ev.causes.is_empty(),
                    "non-age plant death must point at causal state/events: {cause:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "expected some environmental plant deaths in 2 years");
    let _ = Cause::Event(chronica_engine::core::ids::EventId(1));
}
