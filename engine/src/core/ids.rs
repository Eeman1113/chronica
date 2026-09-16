//! Typed, permanent, never-reused 64-bit entity IDs (docs/ARCHITECTURE.md "IDs & storage").
//! Entities live forever in their arenas (dead ones flagged, never removed), so the arena index
//! is stable and the ID doubles as `index+1` into the arena (0 = "none" sentinel is avoided by
//! starting allocation at 1 and storing index = id-1). Allocation is per-kind and deterministic.

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);
        impl $name {
            pub const NONE: $name = $name(0);
            #[inline]
            pub fn from_index(i: usize) -> Self {
                $name(i as u64 + 1)
            }
            #[inline]
            pub fn index(self) -> usize {
                debug_assert!(self.0 != 0, "NONE id dereferenced");
                (self.0 - 1) as usize
            }
            #[inline]
            pub fn is_none(self) -> bool {
                self.0 == 0
            }
            #[inline]
            pub fn some(self) -> Option<Self> {
                if self.0 == 0 { None } else { Some(self) }
            }
        }
    };
}

id_type!(PersonId);
id_type!(AnimalId);
id_type!(PlantId);
id_type!(SettlementId);
id_type!(FactionId);
id_type!(BuildingId);
id_type!(ItemId);
id_type!(ArmyId);
id_type!(EventId);
id_type!(BeliefId);
id_type!(CultureId);
id_type!(DiseaseId);

/// A reference to anything that can appear in history. Cells are referenced by grid index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum EntityRef {
    Person(PersonId),
    Animal(AnimalId),
    Plant(PlantId),
    Settlement(SettlementId),
    Faction(FactionId),
    Building(BuildingId),
    Item(ItemId),
    Army(ArmyId),
    Belief(BeliefId),
    Culture(CultureId),
    Disease(DiseaseId),
    Cell(u32),
}
