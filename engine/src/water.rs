//! The water cycle: rain → surface water → downhill flow → pooling/evaporation/infiltration →
//! groundwater → springs. Rivers and lakes are DERIVED from this state (`Grid::is_river/is_lake`)
//! — nothing ever places them (Test B). Erosion moves real sediment.
//!
//! Determinism under threads: flows are computed in parallel from an immutable snapshot of the
//! previous surface state; application happens single-threaded in index order (jobs.rs contract).

use crate::core::jobs::par_map;
use crate::sim::Sim;
use crate::terrain::ground_capacity;
use crate::world::grid::{DX8, DY8};

const FLOW_RATE: f32 = 0.65;     // fraction of head difference that moves per day
const INFILTRATION: f32 = 0.020; // surface → groundwater per day
const EVAP_BASE: f32 = 0.0016;   // per degree-day evaporation from surface water
const SOIL_EVAP: f32 = 0.0016;   // groundwater loss to evapotranspiration
const SPRING_RATE: f32 = 0.010;  // groundwater overflow → surface (springs/baseflow)
const EROSION_K: f32 = 0.00030;  // sediment eroded per unit flow
const FLOW_EMA: f32 = 0.06;      // how fast the derived-flow observation tracks reality

pub fn tick(sim: &mut Sim) {
    let n = sim.grid.n();
    let w = sim.grid.w as i32;
    let h = sim.grid.h as i32;

    // 1) Rain lands on the surface; infiltration into groundwater; evaporation.
    for i in 0..n {
        if sim.grid.ocean[i] {
            continue;
        }
        sim.grid.surface[i] += sim.grid.rain[i];
        // infiltration limited by remaining capacity
        let cap = ground_capacity(&sim.grid, i);
        let room = (cap - sim.grid.ground[i]).max(0.0);
        let inf = sim.grid.surface[i].min(INFILTRATION).min(room);
        sim.grid.surface[i] -= inf;
        sim.grid.ground[i] += inf;
        // evaporation from open water, stronger when hot
        let t = sim.grid.temp[i].max(0.0);
        let evap = (EVAP_BASE * t).min(sim.grid.surface[i]);
        sim.grid.surface[i] -= evap;
        // evapotranspiration draws down groundwater
        sim.grid.ground[i] = (sim.grid.ground[i] - SOIL_EVAP * t * 0.35).max(0.0);
        // springs: over-capacity groundwater surfaces
        if sim.grid.ground[i] > cap {
            let over = (sim.grid.ground[i] - cap).min(SPRING_RATE + (sim.grid.ground[i] - cap) * 0.5);
            sim.grid.ground[i] -= over;
            sim.grid.surface[i] += over;
        }
    }

    // 2) Flow: each wet land cell sends water toward its lowest neighbor by hydraulic head
    //    (elevation + water depth). Parallel plan from snapshot, ordered apply.
    let elev = &sim.grid.elev;
    let surf_snapshot: Vec<f32> = sim.grid.surface.clone();
    let ocean = &sim.grid.ocean;
    let grid_w = sim.grid.w;
    let plans: Vec<(u32, f32)> = par_map(n, |i| {
        if ocean[i] || surf_snapshot[i] <= 0.0005 {
            return (u32::MAX, 0.0);
        }
        let x = (i as u32 % grid_w) as i32;
        let y = (i as u32 / grid_w) as i32;
        let head_i = elev[i] + surf_snapshot[i];
        let mut best_j = u32::MAX;
        let mut best_drop = 0.0f32;
        for k in 0..8 {
            let (nx, ny) = (x + DX8[k], y + DY8[k]);
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            let j = (ny as u32 * grid_w + nx as u32) as usize;
            let head_j = if ocean[j] { 0.0 } else { elev[j] + surf_snapshot[j] };
            let drop = head_i - head_j;
            if drop > best_drop {
                best_drop = drop;
                best_j = j as u32;
            }
        }
        if best_j == u32::MAX {
            return (u32::MAX, 0.0);
        }
        // move up to half the head difference (stability), capped by available water
        let amount = (best_drop * 0.5 * FLOW_RATE).min(surf_snapshot[i]);
        (best_j, amount)
    });
    for i in 0..n {
        let (j, amt) = plans[i];
        if j == u32::MAX || amt <= 0.0 {
            // decay the flow observation where nothing moves
            sim.grid.flow[i] *= 1.0 - FLOW_EMA;
            continue;
        }
        sim.grid.surface[i] -= amt;
        if !sim.grid.ocean[j as usize] {
            sim.grid.surface[j as usize] += amt;
        } // water reaching the ocean joins the sea (its level is the boundary condition)
        sim.grid.flow[i] = sim.grid.flow[i] * (1.0 - FLOW_EMA) + amt * FLOW_EMA;

        // 3) Erosion: flowing water carries soil downhill.
        let er = (EROSION_K * amt).min(sim.grid.soil_depth[i] * 0.001);
        if er > 0.0 {
            sim.grid.elev[i] -= er;
            sim.grid.soil_depth[i] = (sim.grid.soil_depth[i] - er).max(0.01);
            let jj = j as usize;
            if !sim.grid.ocean[jj] {
                sim.grid.sediment[jj] += er;
                // deposition where flow is weak
                if sim.grid.flow[jj] < 0.005 {
                    let dep = sim.grid.sediment[jj] * 0.1;
                    sim.grid.sediment[jj] -= dep;
                    sim.grid.elev[jj] += dep;
                    sim.grid.soil_depth[jj] += dep;
                }
            }
        }
    }
}

/// Derived observation: fraction of land cells that are river or lake (for stats/tests).
pub fn water_stats(sim: &Sim) -> (usize, usize, f32) {
    let mut rivers = 0;
    let mut lakes = 0;
    let mut land = 0usize;
    let mut total_surface = 0.0;
    for i in 0..sim.grid.n() {
        if sim.grid.ocean[i] {
            continue;
        }
        land += 1;
        total_surface += sim.grid.surface[i];
        if sim.grid.is_river(i) {
            rivers += 1;
        } else if sim.grid.is_lake(i) {
            lakes += 1;
        }
    }
    (rivers, lakes, total_surface / land.max(1) as f32)
}
