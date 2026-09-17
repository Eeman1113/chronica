//! Test E (Knowledge): knowledge spreads, persists, or vanishes based on who actually learned
//! it. Techniques live in individual heads; transmission requires real co-located teaching along
//! real bonds; killing every knower before they teach extinguishes the technique forever.

use chronica_engine::history::EventKind;
use chronica_engine::humans::{kill_human, TECH_FARMING, TECH_POTTERY};
use chronica_engine::history::DeathCause;
use chronica_engine::sim::{Config, Sim};

fn count_knowers(sim: &Sim, tech: u8) -> usize {
    sim.humans.list.iter().filter(|h| h.alive && h.techs.contains(&tech)).count()
}

fn teaching_events(sim: &Sim, tech: u8) -> usize {
    sim.history
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::TaughtSkill { skill, .. } if skill == tech))
        .count()
}

#[test]
fn knowledge_spreads_through_real_teaching() {
    let mut sim = Sim::new(Config { seed: 2024, width: 192, height: 128 });
    let initial_farmers = count_knowers(&sim, TECH_FARMING);
    assert!(initial_farmers > 0, "founding bands carry farmers");
    sim.run_days(4 * 360);
    let taught = teaching_events(&sim, TECH_FARMING) + teaching_events(&sim, TECH_POTTERY);
    assert!(
        taught > 0,
        "techniques must have passed head-to-head through real co-located teaching"
    );
    // every learning event chains causally to its teaching event
    for ev in &sim.history.events {
        if let EventKind::LearnedSkill { .. } = ev.kind {
            assert!(!ev.causes.is_empty(), "learning must cite the teaching that caused it");
        }
    }
}

#[test]
fn knowledge_dies_with_its_last_knower() {
    let mut sim = Sim::new(Config { seed: 2024, width: 192, height: 128 });
    // kill every farmer on day one, before anyone could be taught
    let farmers: Vec<usize> = sim
        .humans
        .list
        .iter()
        .enumerate()
        .filter(|(_, h)| h.alive && h.techs.contains(&TECH_FARMING))
        .map(|(i, _)| i)
        .collect();
    assert!(!farmers.is_empty());
    for fi in farmers {
        kill_human(&mut sim, fi, DeathCause::Murder, vec![]);
    }
    sim.run_days(4 * 360);
    assert_eq!(
        count_knowers(&sim, TECH_FARMING),
        0,
        "with every knower dead before teaching, the technique must be extinct"
    );
    assert_eq!(
        teaching_events(&sim, TECH_FARMING),
        0,
        "no farming could ever have been taught"
    );
    // while pottery — whose knower lived — may have spread in the same world
}
