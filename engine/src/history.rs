//! Append-only causal event store (docs/EVENT_MODEL.md).
//! Events are structured data; text is generated on demand (inspection module).
//! Never pruned, never capped. Consequences are a derived reverse index, not a stored field.

use crate::core::ids::{EntityRef, EventId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum StateKind {
    SoilMoisture,
    Groundwater,
    SurfaceWater,
    Rainfall,
    Temperature,
    Fuel,
    Hunger,
    Thirst,
    Grievance,
    FoodStore,
    Morale,
    Belief,
    Wealth,
    Health,
}

/// A causal pointer at physical/social state: (cell or entity implied by the event), field, value.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct StateRef {
    pub what: StateKind,
    pub value: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Cause {
    Event(EventId),
    State(StateRef),
}

/// Kind + payload. Extended stage by stage; every meaningful state change gets one.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EventKind {
    // world / physical
    WorldGenerated,
    LightningStrike,
    FireIgnited,
    FireSpread { cells: u32 },
    FireDied,
    Flood { cells: u32 },
    DroughtSeason { region_dryness: f32 },
    // plants
    PlantDied { cause: DeathCause },
    // animals & humans share death/birth kinds where sensible
    Born { mother: EntityRef, father: EntityRef },
    Died { cause: DeathCause },
    Killed { by: EntityRef },
    Ate { what: EntityRef },
    Mated { with: EntityRef },
    // humans
    Married { with: EntityRef },
    BondFormed { with: EntityRef, context: BondContext },
    TaughtSkill { student: EntityRef, skill: u8 },
    LearnedSkill { skill: u8, teacher: EntityRef },
    DiscoveredSkill { skill: u8 },
    BuiltBuilding { building: EntityRef },
    CraftedItem { item: EntityRef },
    HarvestedFood { amount: f32 },
    FelledTree { plant: EntityRef },
    StoredFood { amount: f32 },
    WentHungry { days: u32 },
    AdoptedBelief { belief: EntityRef, from: EntityRef },
    OriginatedBelief { belief: EntityRef },
    // society
    SettlementFounded { settlement: EntityRef },
    SettlementAbandoned { settlement: EntityRef },
    JoinedSettlement { settlement: EntityRef },
    LeftSettlement { settlement: EntityRef },
    FactionFormed { faction: EntityRef },
    BecameLeader { faction: EntityRef },
    TradeExecuted { good: u8, qty: f32, price: f32, with: EntityRef },
    GrievanceHeld { against: EntityRef, weight: f32 },
    // conflict
    WarDeclared { against: EntityRef },
    BattleFought { attacker: EntityRef, defender: EntityRef, att_losses: u32, def_losses: u32 },
    PeaceMade { with: EntityRef },
    Migrated { from: EntityRef, to_cell: u32 },
    RaidCarriedOut { against: EntityRef },
    TamedAnimal { animal: EntityRef },
    SlaughteredAnimal { animal: EntityRef },
    BuildingLost { building: EntityRef, to_flood: bool },
    CaughtSickness { from: EntityRef, pathogen: u8 },
    RecoveredFromSickness { pathogen: u8 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum DeathCause {
    Age,
    Starvation,
    Thirst,
    Disease,
    Fire,
    Drowning,
    Predation,
    Battle,
    Murder,
    Exposure,
    Drought,
    Shade,     // outcompeted for light
    Frost,
    Felled,    // cut by a person
    Browsed,   // eaten by a herbivore
    Childbirth,
    Slaughtered, // livestock, by its keeper
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum BondContext {
    CoWork,
    Household,
    Comrades,
    Neighbors,
    Kin,
    Trade,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub day: u64,
    /// Primary participant ("whose event this is").
    pub subject: EntityRef,
    pub kind: EventKind,
    pub loc: Option<u32>,
    pub causes: Vec<Cause>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct History {
    pub events: Vec<Event>,
    // Derived indices — rebuilt on load (DECISIONS D-009), never serialized.
    #[serde(skip)]
    pub by_entity: HashMap<EntityRef, Vec<EventId>>,
    #[serde(skip)]
    pub children: HashMap<EventId, Vec<EventId>>,
}

impl History {
    pub fn push(
        &mut self,
        day: u64,
        subject: EntityRef,
        kind: EventKind,
        loc: Option<u32>,
        causes: Vec<Cause>,
    ) -> EventId {
        let id = EventId::from_index(self.events.len());
        for c in &causes {
            if let Cause::Event(pid) = c {
                self.children.entry(*pid).or_default().push(id);
            }
        }
        self.by_entity.entry(subject).or_default().push(id);
        self.events.push(Event { id, day, subject, kind, loc, causes });
        id
    }

    #[inline]
    pub fn get(&self, id: EventId) -> Option<&Event> {
        self.events.get(id.index())
    }
    pub fn of_entity(&self, e: EntityRef) -> &[EventId] {
        self.by_entity.get(&e).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn consequences(&self, id: EventId) -> &[EventId] {
        self.children.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn rebuild_after_load(&mut self) {
        self.by_entity.clear();
        self.children.clear();
        for ev in &self.events {
            self.by_entity.entry(ev.subject).or_default().push(ev.id);
            for c in &ev.causes {
                if let Cause::Event(pid) = c {
                    self.children.entry(*pid).or_default().push(ev.id);
                }
            }
        }
    }

    /// Walk causes depth-first to a bounded depth, returning the chain (for "why?" queries).
    pub fn why(&self, id: EventId, max_depth: usize) -> Vec<(usize, EventId)> {
        let mut out = Vec::new();
        let mut stack = vec![(0usize, id)];
        while let Some((d, e)) = stack.pop() {
            out.push((d, e));
            if d + 1 <= max_depth {
                if let Some(ev) = self.get(e) {
                    for c in ev.causes.iter().rev() {
                        if let Cause::Event(p) = c {
                            stack.push((d + 1, *p));
                        }
                    }
                }
            }
        }
        out
    }
}
