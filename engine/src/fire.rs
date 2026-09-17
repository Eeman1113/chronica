//! Fire on real fuel. Lightning strikes where storm cells actually are; ignition needs actual
//! dry fuel (litter + standing biomass); spread consumes real plants (each killed plant gets a
//! death event caused by the fire); rain and fuel exhaustion end fires. No global spread cap —
//! cost is bounded by fuel, which is bounded by growth (AUDIT §6.11 replacement).

use crate::core::ids::EntityRef;
use crate::core::rng::Rng;
use crate::history::{Cause, DeathCause, EventKind, StateKind, StateRef};
use crate::sim::Sim;
use crate::terrain::Noise;
use crate::vegetation::fuel_at;
use crate::world::grid::{DX8, DY8};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct FireState {
    /// Active fires: cell → (remaining burn days, origin event).
    pub burning: Vec<(u32, u8, crate::core::ids::EventId)>,
    pub rng: Rng,
}

impl FireState {
    pub fn new(master: u64) -> FireState {
        FireState { burning: Vec::new(), rng: Rng::domain(master, "fire") }
    }
}

pub fn tick(sim: &mut Sim) {
    let day = sim.clock.day;
    let noise = Noise::new(crate::core::rng::splitmix64(sim.cfg.seed ^ 0xC71A_7E00));

    // --- Lightning: strikes where the storm field is intense right now ---
    // The same weather field the climate module rains from; a strike is weather uncertainty
    // localized inside a real storm — the causality bar is satisfied by the storm's existence.
    let mut strikes: Vec<usize> = Vec::new();
    {
        let w = sim.grid.w as usize;
        let h = sim.grid.h as usize;
        let mut rng = sim.fire.rng.clone();
        // sample a bounded number of candidate cells deterministically
        for _ in 0..6 {
            let x = rng.pick_index(w);
            let y = rng.pick_index(h);
            let i = y * w + x;
            let front = noise.fbm3(x as f32 / 28.0, y as f32 / 28.0, day as f32 / 11.0, 3);
            if front > 0.66 && !sim.grid.ocean[i] {
                strikes.push(i);
            }
        }
        sim.fire.rng = rng;
    }
    for i in strikes {
        let strike_ev = sim.history.push(
            day,
            EntityRef::Cell(i as u32),
            EventKind::LightningStrike,
            Some(i as u32),
            vec![],
        );
        // ignition requires real dry fuel and no rain falling on it
        let fuel = fuel_at(sim, i);
        let dry = sim.grid.rain[i] <= 0.0 && sim.grid.surface[i] < 0.01 && sim.grid.temp[i] > 2.0;
        if fuel > 0.6 && dry && sim.grid.burning[i] == 0 {
            let ev = sim.history.push(
                day,
                EntityRef::Cell(i as u32),
                EventKind::FireIgnited,
                Some(i as u32),
                vec![
                    Cause::Event(strike_ev),
                    Cause::State(StateRef { what: StateKind::Fuel, value: fuel }),
                ],
            );
            let dur = (2.0 + fuel.min(8.0)) as u8;
            sim.grid.burning[i] = dur;
            sim.fire.burning.push((i as u32, dur, ev));
        }
    }

    if sim.fire.burning.is_empty() {
        return;
    }

    // --- Burn & spread ---
    let mut next: Vec<(u32, u8, crate::core::ids::EventId)> = Vec::new();
    let mut to_kill: Vec<(usize, crate::core::ids::EventId)> = Vec::new();
    let mut spread_to: Vec<(usize, crate::core::ids::EventId)> = Vec::new();
    let mut rng = sim.fire.rng.clone();
    for bi in 0..sim.fire.burning.len() {
        let (cell, mut left, origin) = sim.fire.burning[bi];
        let i = cell as usize;
        // rain puts fires out
        if sim.grid.rain[i] > 0.005 {
            sim.grid.burning[i] = 0;
            sim.history.push(day, EntityRef::Cell(cell), EventKind::FireDied, Some(cell), vec![
                Cause::Event(origin),
                Cause::State(StateRef { what: StateKind::Rainfall, value: sim.grid.rain[i] }),
            ]);
            continue;
        }
        // consume litter, mark scar
        sim.grid.litter[i] *= 0.5;
        sim.grid.burn_scar[i] = 1.0;
        // burn the plants that are actually here (their deaths caused by this fire)
        to_kill.push((i, origin));
        // spread to neighbors with enough dry fuel; the wind carries the fire downwind
        let (wx, wy) = crate::climate::wind_today(sim.cfg.seed, day);
        let (x, y) = sim.grid.xy(i);
        for k in 0..8 {
            if let Some(j) = sim.grid.idx(x + DX8[k], y + DY8[k]) {
                if sim.grid.burning[j] > 0 || sim.grid.ocean[j] {
                    continue;
                }
                let fuel_j = fuel_at(sim, j);
                let dry_j = sim.grid.rain[j] <= 0.0 && sim.grid.surface[j] < 0.01;
                // alignment of this direction with today's wind: downwind ×3, upwind ×0.25
                let dl = ((DX8[k] as f32 * wx + DY8[k] as f32 * wy)
                    / (DX8[k] as f32).hypot(DY8[k] as f32))
                    .clamp(-1.0, 1.0);
                let wind_mul = if dl > 0.3 { 3.0 } else if dl < -0.3 { 0.25 } else { 1.0 };
                if fuel_j > 0.5 && dry_j && rng.chance((fuel_j * 0.06 * wind_mul).min(0.6)) {
                    spread_to.push((j, origin));
                }
            }
        }
        left -= 1;
        if left == 0 || fuel_at(sim, i) < 0.1 {
            sim.grid.burning[i] = 0;
            sim.history.push(day, EntityRef::Cell(cell), EventKind::FireDied, Some(cell), vec![
                Cause::Event(origin),
            ]);
        } else {
            sim.grid.burning[i] = left;
            next.push((cell, left, origin));
        }
    }
    sim.fire.rng = rng;

    // kill burning plants (events point at the fire origin)
    for (cell, origin) in to_kill {
        let victims: Vec<usize> = sim
            .plants
            .by_cell
            .get(cell)
            .map(|v| v.iter().map(|&x| x as usize).collect())
            .unwrap_or_default();
        for pi in victims {
            let fire_res = crate::species::PLANTS[sim.plants.list[pi].species as usize].fire_res;
            // big trees with fire-resistant bark can survive a ground fire pass
            if sim.plants.list[pi].biomass > 3.0 && fire_res > 0.2 {
                sim.plants.list[pi].health -= 0.4;
                if sim.plants.list[pi].health > 0.0 {
                    continue;
                }
            }
            crate::vegetation::kill_plant(sim, pi, DeathCause::Fire, vec![Cause::Event(origin)]);
        }
    }
    let mut spread_count = 0u32;
    for (j, origin) in spread_to {
        if sim.grid.burning[j] == 0 {
            let fuel = fuel_at(sim, j);
            let dur = (2.0 + fuel.min(8.0)) as u8;
            sim.grid.burning[j] = dur;
            next.push((j as u32, dur, origin));
            spread_count += 1;
        }
    }
    if spread_count > 0 {
        // one aggregate spread event per day per world keeps event volume sane while every
        // plant death above still carries its own caused event
        let origin = next.last().unwrap().2;
        sim.history.push(
            day,
            EntityRef::Cell(next.last().unwrap().0),
            EventKind::FireSpread { cells: spread_count },
            None,
            vec![Cause::Event(origin)],
        );
    }
    sim.fire.burning = next;

    // scars heal slowly (decades)
    if day % 90 == 17 {
        for s in sim.grid.burn_scar.iter_mut() {
            *s = (*s - 0.02).max(0.0);
        }
    }
}
