//! Persistent objects: buildings and items (directive §2.2 "important objects"). Every building
//! and tool has identity, maker, materials, condition, and history through events. Stores hold
//! real food mass contributed by real gathering acts.

use crate::core::ids::{BuildingId, ItemId, PersonId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingKind {
    Hut,      // shelter for a household
    Granary,  // shared food store
    Hall,     // gathering place (institutions later)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Building {
    pub kind: BuildingKind,
    pub cell: u32,
    pub built_day: u64,
    pub builder: PersonId,
    pub wood_used: f32,   // real logs from real felled trees
    pub progress: f32,    // 0..1 — a house is raised over days of real work
    pub condition: f32,   // decays; repaired with more wood
    pub food_store: f32,  // food mass actually deposited
    pub burned: bool,
    pub exists: bool,
}

impl Building {
    /// A building shelters only when finished and sound.
    #[inline]
    pub fn standing(&self) -> bool {
        self.exists && self.progress >= 1.0 && self.condition > 0.3
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    StoneAxe,
    Spear,
    Bow,
    Pot,
    Cloth,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    pub made_day: u64,
    pub maker: PersonId,
    pub owner: PersonId, // NONE => lying at `cell`
    pub cell: u32,
    pub condition: f32,
    pub exists: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Objects {
    pub buildings: Vec<Building>,
    pub items: Vec<Item>,
}

impl Objects {
    pub fn building_id(idx: usize) -> BuildingId {
        BuildingId::from_index(idx)
    }
    pub fn item_id(idx: usize) -> ItemId {
        ItemId::from_index(idx)
    }
    /// Buildings on/near a cell (linear scan is fine at current scales; indexed in Stage 8).
    pub fn building_at(&self, cell: u32) -> Option<usize> {
        self.buildings.iter().position(|b| b.exists && b.cell == cell)
    }
}
