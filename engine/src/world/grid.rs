//! The world grid: every cell is individually addressable state (directive §2.2).
//! Layout is SoA — one dense column per field, indexed by `y*w + x`.
//! Water bodies (rivers/lakes/oceans) are NEVER flagged by a generator: they are derived
//! observations over where water actually is (`surface`, `flow`, `ocean` from elevation vs sea
//! level at gen time only for the initial ocean fill — the sea is real standing water).

use serde::{Deserialize, Serialize};

pub const CHUNK: u32 = 32;

#[derive(Clone, Serialize, Deserialize)]
pub struct Grid {
    pub w: u32,
    pub h: u32,
    // -- terrain --
    pub elev: Vec<f32>,      // elevation; sea level = 0.0
    pub soil_depth: Vec<f32>,// erodible regolith depth
    pub soil_n: Vec<f32>,    // nutrients 0..~1.5
    pub litter: Vec<f32>,    // dead organic matter awaiting decomposition
    pub sediment: Vec<f32>,  // suspended/deposited sediment budget
    // -- water --
    pub surface: Vec<f32>,   // standing/flowing surface water depth
    pub ground: Vec<f32>,    // groundwater store 0..capacity
    pub flow: Vec<f32>,      // EMA of recent outflow volume (derived-river evidence)
    pub ocean: Vec<bool>,    // connected-to-edge below-sea-level basin, filled at gen with real water
    // -- climate-written --
    pub temp: Vec<f32>,      // today's surface temperature (deg C)
    pub rain: Vec<f32>,      // today's rainfall depth
    pub snow: Vec<f32>,      // snowpack depth
    // -- disturbance --
    pub burning: Vec<u8>,    // remaining burn ticks (0 = not burning)
    pub burn_scar: Vec<f32>, // 0..1 recent-burn fraction, heals over years
    pub contamination: Vec<f32>,
    // -- occupancy caches (rebuilt, not serialized) --
    #[serde(skip)]
    pub veg_cover: Vec<f32>, // derived: total plant cover per cell (observation cache)
}

impl Grid {
    pub fn new(w: u32, h: u32) -> Grid {
        let n = (w * h) as usize;
        Grid {
            w,
            h,
            elev: vec![0.0; n],
            soil_depth: vec![0.5; n],
            soil_n: vec![0.5; n],
            litter: vec![0.0; n],
            sediment: vec![0.0; n],
            surface: vec![0.0; n],
            ground: vec![0.0; n],
            flow: vec![0.0; n],
            ocean: vec![false; n],
            temp: vec![10.0; n],
            rain: vec![0.0; n],
            snow: vec![0.0; n],
            burning: vec![0; n],
            burn_scar: vec![0.0; n],
            contamination: vec![0.0; n],
            veg_cover: vec![0.0; n],
        }
    }

    #[inline]
    pub fn n(&self) -> usize {
        (self.w * self.h) as usize
    }
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            None
        } else {
            Some((y as u32 * self.w + x as u32) as usize)
        }
    }
    #[inline]
    pub fn xy(&self, i: usize) -> (i32, i32) {
        ((i as u32 % self.w) as i32, (i as u32 / self.w) as i32)
    }
    /// Rebuild skipped/derived columns after deserialization.
    pub fn rebuild_after_load(&mut self) {
        if self.veg_cover.len() != self.n() {
            self.veg_cover = vec![0.0; self.n()];
        }
    }
    /// A cell currently holding meaningful fresh water (derived observation).
    #[inline]
    pub fn is_fresh_water(&self, i: usize) -> bool {
        !self.ocean[i] && self.surface[i] > 0.05
    }
    /// Derived "river" observation: sustained flow through a wet cell.
    #[inline]
    pub fn is_river(&self, i: usize) -> bool {
        !self.ocean[i] && self.surface[i] > 0.01 && self.flow[i] > 0.02
    }
    /// Derived "lake" observation: standing water with little throughflow.
    #[inline]
    pub fn is_lake(&self, i: usize) -> bool {
        !self.ocean[i] && self.surface[i] > 0.12 && self.flow[i] <= 0.02
    }
}

pub const DX8: [i32; 8] = [1, -1, 0, 0, 1, 1, -1, -1];
pub const DY8: [i32; 8] = [0, 0, 1, -1, 1, -1, 1, -1];
pub const DX4: [i32; 4] = [1, -1, 0, 0];
pub const DY4: [i32; 4] = [0, 0, 1, -1];
