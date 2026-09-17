//! Read-only inspection API (directive §4.8). The ONLY interface for renderer and tools.
//! Player-facing text is GENERATED here from structured events — never stored (EVENT_MODEL.md).

use crate::core::ids::{EntityRef, EventId};
use crate::history::{Cause, DeathCause, Event, EventKind};
use crate::sim::Sim;

pub fn name_of(sim: &Sim, e: EntityRef) -> String {
    match e {
        EntityRef::Person(p) => sim
            .humans
            .get(p)
            .map(|h| h.name.clone())
            .unwrap_or_else(|| format!("person #{}", p.0)),
        EntityRef::Animal(a) => sim
            .animals
            .list
            .get(a.index())
            .map(|x| {
                format!("a {}", crate::species::ANIMALS[x.species as usize].name.to_lowercase())
            })
            .unwrap_or_else(|| format!("animal #{}", a.0)),
        EntityRef::Plant(p) => sim
            .plants
            .list
            .get(p.index())
            .map(|x| crate::species::PLANTS[x.species as usize].name.to_lowercase())
            .unwrap_or_else(|| format!("plant #{}", p.0)),
        EntityRef::Settlement(s) => sim
            .society
            .settlements
            .get(s.index())
            .map(|x| x.name.clone())
            .unwrap_or_else(|| format!("settlement #{}", s.0)),
        EntityRef::Belief(b) => sim
            .society
            .beliefs
            .get(b.index())
            .map(|x| x.name.clone())
            .unwrap_or_else(|| format!("belief #{}", b.0)),
        EntityRef::Building(b) => format!("a building (#{})", b.0),
        EntityRef::Item(i) => format!("an item (#{})", i.0),
        EntityRef::Army(a) => format!("a warband (#{})", a.0),
        EntityRef::Faction(f) => format!("a people (#{})", f.0),
        EntityRef::Culture(c) => format!("culture #{}", c.0),
        EntityRef::Disease(d) => format!("a sickness (#{})", d.0),
        EntityRef::Cell(c) => {
            let (x, y) = sim.grid.xy(c as usize);
            format!("the land at ({x},{y})")
        }
    }
}

fn cause_name(c: DeathCause) -> &'static str {
    match c {
        DeathCause::Age => "of old age",
        DeathCause::Starvation => "of hunger",
        DeathCause::Thirst => "of thirst",
        DeathCause::Disease => "of sickness",
        DeathCause::Fire => "in fire",
        DeathCause::Drowning => "by drowning",
        DeathCause::Predation => "to a predator",
        DeathCause::Battle => "in battle",
        DeathCause::Murder => "by another's hand",
        DeathCause::Exposure => "of exposure",
        DeathCause::Drought => "of drought",
        DeathCause::Shade => "starved of light",
        DeathCause::Frost => "of frost",
        DeathCause::Felled => "to the axe",
        DeathCause::Browsed => "eaten to the root",
        DeathCause::Childbirth => "in childbirth",
        DeathCause::Slaughtered => "under the keeper's knife",
    }
}

/// Chronicle voice: one line of text for any event, generated on demand.
pub fn describe(sim: &Sim, ev: &Event) -> String {
    let s = name_of(sim, ev.subject);
    let year = ev.day / 360;
    let body = match &ev.kind {
        EventKind::WorldGenerated => "the world took form".to_string(),
        EventKind::LightningStrike => format!("lightning struck {s}"),
        EventKind::FireIgnited => format!("a fire took hold at {s}"),
        EventKind::FireSpread { cells } => format!("the fire spread across {cells} stretches"),
        EventKind::FireDied => format!("the fire at {s} burned out"),
        EventKind::Flood { cells } => format!("floodwater covered {cells} stretches"),
        EventKind::DroughtSeason { .. } => "the rains failed".to_string(),
        EventKind::PlantDied { cause } => format!("a {s} died {}", cause_name(*cause)),
        EventKind::Born { mother, .. } => {
            format!("{s} was born to {}", name_of(sim, *mother))
        }
        EventKind::Died { cause } => format!("{s} died {}", cause_name(*cause)),
        EventKind::Killed { by } => format!("{s} was slain by {}", name_of(sim, *by)),
        EventKind::Ate { what } => format!("{s} fed on {}", name_of(sim, *what)),
        EventKind::Mated { with } => format!("{s} paired with {}", name_of(sim, *with)),
        EventKind::Married { with } => format!("{s} married {}", name_of(sim, *with)),
        EventKind::BondFormed { with, .. } => {
            format!("{s} and {} became close", name_of(sim, *with))
        }
        EventKind::TaughtSkill { student, skill } => format!(
            "{s} taught {} the craft of {}",
            name_of(sim, *student),
            crate::humans::TECH_NAMES[*skill as usize]
        ),
        EventKind::LearnedSkill { skill, teacher } => format!(
            "{s} learned {} from {}",
            crate::humans::TECH_NAMES[*skill as usize],
            name_of(sim, *teacher)
        ),
        EventKind::DiscoveredSkill { skill } => format!(
            "{s} worked out the craft of {}",
            crate::humans::TECH_NAMES[*skill as usize]
        ),
        EventKind::BuiltBuilding { .. } => format!("{s} raised a dwelling"),
        EventKind::CraftedItem { .. } => format!("{s} crafted a piece of work"),
        EventKind::HarvestedFood { amount } => {
            format!("{s} brought in a harvest of {amount:.1}")
        }
        EventKind::FelledTree { plant } => format!("{s} felled {}", name_of(sim, *plant)),
        EventKind::StoredFood { amount } => format!("{s} laid up {amount:.1} in store"),
        EventKind::WentHungry { days } => format!("{s} went hungry for {days} days"),
        EventKind::AdoptedBelief { belief, from } => format!(
            "{s} took up {} from {}",
            name_of(sim, *belief),
            name_of(sim, *from)
        ),
        EventKind::OriginatedBelief { belief } =>

            format!("{s} first spoke of {}", name_of(sim, *belief)),
        EventKind::SettlementFounded { settlement } => {
            format!("the settlement of {} took root", name_of(sim, *settlement))
        }
        EventKind::SettlementAbandoned { settlement } => {
            format!("{} fell silent and empty", name_of(sim, *settlement))
        }
        EventKind::JoinedSettlement { settlement } => {
            format!("{s} settled in {}", name_of(sim, *settlement))
        }
        EventKind::LeftSettlement { settlement } => {
            format!("{s} left {}", name_of(sim, *settlement))
        }
        EventKind::FactionFormed { .. } => format!("{s} banded together as a people"),
        EventKind::BecameLeader { faction } => {
            format!("{s} came to lead {}", name_of(sim, *faction))
        }
        EventKind::TradeExecuted { qty, with, .. } => {
            format!("{s} traded {qty:.0} goods with {}", name_of(sim, *with))
        }
        EventKind::GrievanceHeld { against, .. } => {
            format!("{s} held a grievance against {}", name_of(sim, *against))
        }
        EventKind::WarDeclared { against } => {
            format!("{s} made war on {}", name_of(sim, *against))
        }
        EventKind::BattleFought { attacker, defender, att_losses, def_losses } => format!(
            "{} met {} in battle: {} and {} fell",
            name_of(sim, *attacker),
            name_of(sim, *defender),
            att_losses,
            def_losses
        ),
        EventKind::PeaceMade { with } => format!("{s} made peace with {}", name_of(sim, *with)),
        EventKind::Migrated { to_cell, .. } => {
            let (x, y) = sim.grid.xy(*to_cell as usize);
            format!("{s} moved away toward ({x},{y})")
        }
        EventKind::RaidCarriedOut { against } => {
            format!("{s} raided the stores of {}", name_of(sim, *against))
        }
        EventKind::TamedAnimal { animal } => {
            format!("{s} tamed {}", name_of(sim, *animal))
        }
        EventKind::SlaughteredAnimal { animal } => {
            format!("{s} slaughtered {}", name_of(sim, *animal))
        }
        EventKind::CaughtSickness { from, pathogen } => format!(
            "{s} caught the {} from {}",
            crate::pathogens::PATHOGENS[*pathogen as usize].name,
            name_of(sim, *from)
        ),
        EventKind::RecoveredFromSickness { pathogen } => format!(
            "{s} shook off the {}",
            crate::pathogens::PATHOGENS[*pathogen as usize].name
        ),
        EventKind::BuildingLost { to_flood, .. } => {
            if *to_flood {
                format!("the waters took {s}'s dwelling")
            } else {
                format!("{s}'s dwelling fell to ruin")
            }
        }
    };
    format!("Year {year}: {body}")
}

/// The "why" chain as text: walk causes to physical/social state (Test F shape).
pub fn why_text(sim: &Sim, id: EventId, max_depth: usize) -> Vec<String> {
    let mut out = Vec::new();
    for (depth, ev_id) in sim.history.why(id, max_depth) {
        let Some(ev) = sim.history.get(ev_id) else { continue };
        let indent = "  ".repeat(depth);
        out.push(format!("{indent}{}", describe(sim, ev)));
        // terminal state causes
        for c in &ev.causes {
            if let Cause::State(sr) = c {
                out.push(format!(
                    "{indent}  ← because {:?} was {:.2}",
                    sr.what, sr.value
                ));
            }
        }
    }
    out
}

/// Full biography of a person, derived purely from events (no stored text).
pub fn biography(sim: &Sim, e: EntityRef) -> Vec<String> {
    sim.history
        .of_entity(e)
        .iter()
        .filter_map(|id| sim.history.get(*id))
        .map(|ev| describe(sim, ev))
        .collect()
}
