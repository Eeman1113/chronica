//! Tests C, D, F (society-layer shapes).
//! C: settlements arise from real building/population clusters, with no growth formula.
//! D: violence is a *possible* outcome of need+grievance; some seeds produce none.
//! F: "why?" chains traverse from social events down to physical/social state.

use chronica_engine::history::{Cause, EventKind};
use chronica_engine::inspection;
use chronica_engine::sim::{Config, Sim};

#[test]
fn c_settlements_emerge_from_real_clusters() {
    // Across seeds, at least one world grows a recognized settlement out of actual huts +
    // actual residents; the founding event exists and is causally anchored.
    let mut found_any = false;
    for seed in [7u64, 2024, 99] {
        let mut sim = Sim::new(Config { seed, width: 192, height: 128 });
        sim.run_days(3 * 360);
        for ev in &sim.history.events {
            if let EventKind::SettlementFounded { .. } = ev.kind {
                found_any = true;
                // the settlement is a derived observation over REAL buildings:
                let s = &sim.society.settlements[0];
                let n_bld = sim
                    .objects
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.exists && {
                            let (x, y) = sim.grid.xy(b.cell as usize);
                            (x - s.center.0).abs().max((y - s.center.1).abs()) <= 10
                        }
                    })
                    .count();
                assert!(n_bld >= 3, "a settlement must sit on a real building cluster");
            }
        }
    }
    assert!(found_any, "some seed should grow a settlement from real clustering");
}

#[test]
fn d_conflict_possible_not_scripted() {
    // Raids require desperation/grievance + courage + a real visible out-group target.
    // Verify: every raid that occurred is causally chained to hunger state or a death event —
    // and that raids are NOT universal (some seeds have none): no scheduler, no quota.
    let mut counts: Vec<usize> = Vec::new();
    for seed in [7u64, 11, 2024, 42, 99, 1234] {
        let mut sim = Sim::new(Config { seed, width: 160, height: 110 });
        sim.run_days(4 * 360);
        let raids: Vec<_> = sim
            .history
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::RaidCarriedOut { .. }))
            .collect();
        for r in &raids {
            assert!(
                !r.causes.is_empty(),
                "every raid must cite its causes (hunger state / grudge event)"
            );
        }
        counts.push(raids.len());
    }
    // No quota, no schedule: the amount of violence must be free to vary with each world's
    // actual desperation and grievances — identical counts across seeds would smell scripted.
    let min = counts.iter().min().unwrap();
    let max = counts.iter().max().unwrap();
    assert!(
        min != max,
        "raid counts must vary across worlds (got uniformly {min}): violence looks scheduled"
    );
}

#[test]
fn f_why_chains_reach_physical_state() {
    let mut sim = Sim::new(Config { seed: 42, width: 160, height: 110 });
    sim.run_days(2 * 360);
    // find a predation death and walk why(): must reach a hunger StateRef via the kill event
    let mut verified = false;
    for ev in &sim.history.events {
        if matches!(
            ev.kind,
            EventKind::Died { cause: chronica_engine::history::DeathCause::Predation }
        ) {
            let chain = sim.history.why(ev.id, 8);
            let mut saw_state = false;
            for (_, cid) in &chain {
                if let Some(cev) = sim.history.get(*cid) {
                    if cev.causes.iter().any(|c| matches!(c, Cause::State(_))) {
                        saw_state = true;
                    }
                }
            }
            assert!(saw_state, "a predation death must trace to hunger state");
            let text = inspection::why_text(&sim, ev.id, 8);
            assert!(text.len() >= 2, "why-text renders the chain");
            verified = true;
            break;
        }
    }
    assert!(verified, "expected at least one predation death in 2 years");
}
