//! The simulation root. One mode, full fidelity, headless (directive §2.1, §2.11).
//! `tick()` runs the fixed phase order from docs/SIMULATION_MODEL.md; systems are added to the
//! phase body as their stage lands — nothing here pretends to run before it exists.

use crate::core::clock::Clock;
use crate::history::History;
use crate::world::Grid;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub seed: u64,
    pub width: u32,
    pub height: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { seed: 1, width: 192, height: 128 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Sim {
    pub cfg: Config,
    pub clock: Clock,
    pub grid: Grid,
    pub history: History,
    pub plants: crate::vegetation::Plants,
    pub animals: crate::animals::Animals,
    pub fire: crate::fire::FireState,
    pub humans: crate::humans::Humans,
    pub objects: crate::objects::Objects,
}

impl Sim {
    /// Build a world from a seed. Stage 1: allocates the grid only (flat world, no systems).
    /// Terrain/climate/water generation arrives in Stage 2 and lives in their modules.
    pub fn new(cfg: Config) -> Sim {
        let grid = Grid::new(cfg.width, cfg.height);
        let fire = crate::fire::FireState::new(cfg.seed);
        let mut sim = Sim {
            cfg,
            clock: Clock::default(),
            grid,
            history: History::default(),
            plants: Default::default(),
            animals: Default::default(),
            fire,
            humans: Default::default(),
            objects: Default::default(),
        };
        sim.generate();
        sim
    }

    /// World generation dispatcher — grows per stage.
    fn generate(&mut self) {
        crate::terrain::generate(self);
        // Hydrological spin-up: run climate+water alone until rivers flow, then rewind the
        // clock. Life is seeded into a world whose water cycle is already real — no water body
        // is ever placed, we simply let the rain fall before anyone is born to see it.
        for _ in 0..240 {
            self.clock.day += 1;
            crate::climate::tick(self);
            crate::water::tick(self);
        }
        self.clock.day = 0;
        crate::vegetation::generate(self);
        crate::animals::generate(self);
        crate::humans::generate(self);
        self.history.push(
            0,
            crate::core::ids::EntityRef::Cell(0),
            crate::history::EventKind::WorldGenerated,
            None,
            vec![],
        );
    }

    /// One tick = one day (DECISIONS D-007). Fixed phase order.
    pub fn tick(&mut self) {
        self.clock.day += 1;
        // Phase 1: climate
        crate::climate::tick(self);
        // Phase 2: water
        crate::water::tick(self);
        // Phase 3: fire & hazards (burns real fuel)
        crate::fire::tick(self);
        // Phase 4: vegetation (individual plants)
        crate::vegetation::tick(self);
        // Phases 5–8: animal perception → cognition → action → physiology
        crate::animals::tick(self);
        // Phases 5–8 for people: perception → cognition → action → physiology
        crate::humans::tick(self);
        // Phases 9+ (society/economy) land with their stages.
    }

    pub fn run_days(&mut self, days: u64) {
        for _ in 0..days {
            self.tick();
        }
    }

    /// Canonical serialized form (also the save payload). Determinism tests hash this.
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("serialize sim")
    }

    pub fn state_hash(&self) -> u64 {
        // FNV-1a over the canonical bytes: stable, dependency-free.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.to_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        h
    }
}
