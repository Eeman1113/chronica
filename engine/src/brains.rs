//! One decision architecture for all agents (directive §4.5), parameterized by species and
//! individual. Inputs: needs + what was actually perceived + memory + genes. Output: an action
//! plus a recorded rationale (the score table), so "why did X do Y?" is always answerable from
//! state. Animals use it directly; the human brain (Stage 4) extends the same shape with
//! memory/relationships/goals.

use serde::{Deserialize, Serialize};

/// What an animal actually perceived this tick (filled from the spatial index — never omniscient).
#[derive(Clone, Copy, Default)]
pub struct Percepts {
    pub threat_idx: Option<u32>,   // nearest predator (arena index)
    pub threat_dist: i32,
    pub prey_idx: Option<u32>,     // nearest huntable animal
    pub prey_dist: i32,
    pub forage_here: f32,          // edible plant biomass in current cell
    pub forage_near: Option<(i32, i32)>, // best forage cell seen within perception
    pub forage_near_val: f32,      // its edible biomass
    pub water_near: Option<(i32, i32)>,  // nearest water seen (or remembered)
    pub mate_idx: Option<u32>,
    pub mate_dist: i32,
    pub herd_center: Option<(i32, i32)>,
    pub carrion_near: Option<u32>, // corpse index
    pub fire_near: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Flee { from: (i32, i32) },
    Drink { at: (i32, i32) },
    Graze,                      // eat plants in current cell
    MoveToForage { to: (i32, i32) },
    Hunt { target: u32 },
    Scavenge { corpse: u32 },
    Court { mate: u32 },
    JoinHerd { to: (i32, i32) },
    Rest,
    Roam,
}

/// The recorded rationale: the utility table the decision argmaxed over. Serialized with the
/// animal so the inspector can show "what it weighed" (prototype's `a.think`, made structural).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Rationale {
    pub flee: f32,
    pub drink: f32,
    pub eat: f32,
    pub hunt: f32,
    pub mate: f32,
    pub herd: f32,
    pub rest: f32,
    pub chosen: u8,
}

pub struct AnimalMindInput {
    pub hunger: f32,
    pub thirst: f32,
    pub fatigue: f32,
    pub fear: f32,
    pub genes: [f32; 4], // wariness, appetite, boldness, herd-instinct
    pub is_carnivore: bool,
    pub is_herbivore: bool,
    pub mature: bool,
    pub pregnant: bool,
}

/// The shared utility policy. Pure function: same inputs → same decision.
pub fn decide(inp: &AnimalMindInput, p: &Percepts, my_pos: (i32, i32)) -> (Action, Rationale) {
    let mut r = Rationale::default();

    // threat response — perceived predators and fire dominate
    if let Some(_t) = p.threat_idx {
        let close = (8.0 - p.threat_dist as f32).max(0.0) / 8.0;
        r.flee = close * 3.0 * inp.genes[0] + inp.fear;
    }
    if p.fire_near {
        r.flee = r.flee.max(3.5);
    }
    r.drink = inp.thirst * inp.thirst * 1.8;
    if p.water_near.is_none() {
        r.drink *= 0.5; // no water perceived or remembered: urge tempered by ignorance
    }
    if inp.is_herbivore {
        let food = p.forage_here.max(p.forage_near_val * 0.8).max(0.02);
        r.eat = (inp.hunger.powf(1.3) * inp.genes[1] * (0.4 + food.min(1.5))).min(1.3);
    }
    if inp.is_carnivore {
        if let Some(_) = p.prey_idx {
            let close = (10.0 - p.prey_dist as f32).max(0.0) / 10.0;
            r.hunt = inp.hunger.powf(1.2) * inp.genes[1] * (0.6 + close) * (0.5 + inp.genes[2]);
        }
        if p.carrion_near.is_some() {
            r.eat = r.eat.max(inp.hunger * 1.1);
        }
    }
    if inp.mature && !inp.pregnant && p.mate_idx.is_some() && inp.hunger < 1.0 && r.flee < 0.5 {
        r.mate = 1.0 + (1.0 - inp.hunger) * 0.6;
    }
    if let Some(_) = p.herd_center {
        // company matters less on an empty stomach
        r.herd = inp.genes[3] * 0.5 * (1.0 - inp.hunger * 0.6).max(0.0);
    }
    r.rest = inp.fatigue * 1.2;
    let roam = 0.15 + inp.genes[2] * 0.1;

    // argmax with fixed priority order for exact determinism on ties
    let table = [
        (r.flee, 0u8),
        (r.drink, 1),
        (r.eat, 2),
        (r.hunt, 3),
        (r.mate, 4),
        (r.herd, 5),
        (r.rest, 6),
        (roam, 7),
    ];
    let mut best = table[7];
    for e in table.iter() {
        if e.0 > best.0 {
            best = *e;
        }
    }
    r.chosen = best.1;

    let act = match best.1 {
        0 => {
            // run from the threat's position (or just away if it vanished)
            Action::Flee { from: my_pos }
        }
        1 => Action::Drink { at: p.water_near.unwrap_or(my_pos) },
        2 => {
            if inp.is_carnivore {
                if let Some(c) = p.carrion_near {
                    Action::Scavenge { corpse: c }
                } else {
                    Action::Roam
                }
            } else if let Some(to) = p.forage_near {
                // move when the grass really is greener elsewhere
                if p.forage_near_val > p.forage_here * 2.0 && p.forage_here < 0.3 {
                    Action::MoveToForage { to }
                } else if p.forage_here > 0.03 {
                    Action::Graze
                } else {
                    Action::MoveToForage { to }
                }
            } else if p.forage_here > 0.03 {
                Action::Graze
            } else {
                Action::Roam
            }
        }
        3 => Action::Hunt { target: p.prey_idx.unwrap() },
        4 => Action::Court { mate: p.mate_idx.unwrap() },
        5 => Action::JoinHerd { to: p.herd_center.unwrap() },
        6 => Action::Rest,
        _ => Action::Roam,
    };
    (act, r)
}
