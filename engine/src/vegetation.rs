//! Individual plants (directive §2.2, Test A). Every plant is an entity with its own state and
//! RNG stream; it grows from its cell's real water/nutrients/light, competes for light with the
//! plants actually in its cell, transpires real groundwater (closing the hydrology loop), seeds
//! real neighbor cells, and dies of an actual cause with an event. Forest cover, biome labels,
//! and fuel are DERIVED (`Grid::veg_cover`, `fuel_at`). This module replaces the prototype's
//! `world.veg/vsp/vgen` density fields (AUDIT §6.4).

use crate::core::ids::{EntityRef, PlantId};
use crate::core::jobs::par_map;
use crate::core::rng::Rng;
use crate::history::{Cause, DeathCause, EventKind, StateKind, StateRef};
use crate::sim::Sim;
use crate::species::{PlantKind, PLANTS, SP_GRASS, SP_OAK, SP_PINE, SP_REED, SP_SCRUB};
use crate::terrain::{ground_capacity, Noise};
use serde::{Deserialize, Serialize};

/// Evaluation cadence: each plant is evaluated every N days (staggered by id), with dt scaling.
/// Cadence changes *when*, never *what* (directive §2.10).
pub const PLANT_STRIDE: u64 = 7;
/// Soft cap on total biomass per cell — competition for space/light, not a population command.
pub const CELL_CAPACITY: f32 = 4.0;

#[derive(Clone, Serialize, Deserialize)]
pub struct Plant {
    pub species: u8,
    pub cell: u32,
    pub born: i64, // may be negative: the initial cohort predates day 0
    pub biomass: f32,
    pub health: f32,   // 1 healthy .. 0 dead
    pub max_age_d: u64,// drawn once at germination from own stream
    pub rng: Rng,
    pub alive: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Plants {
    pub list: Vec<Plant>,
    // derived, rebuilt: total biomass and tree-shade per cell
    #[serde(skip)]
    pub cell_biomass: Vec<f32>,
    #[serde(skip)]
    pub cell_shade: Vec<f32>,
    /// Living-plant indices per cell (deterministic ascending order); rebuilt with aggregates.
    #[serde(skip)]
    pub by_cell: Vec<Vec<u32>>,
}

impl Plants {
    pub fn id_of(idx: usize) -> PlantId {
        PlantId::from_index(idx)
    }
    pub fn spawn(&mut self, master: u64, species: u8, cell: u32, born: i64, initial_biomass: f32) -> usize {
        let idx = self.list.len();
        let mut rng = Rng::entity(master, "plants", idx as u64);
        let sp = &PLANTS[species as usize];
        let max_age_d = (sp.max_age_y * 360.0 * rng.range_f(0.6, 1.4)) as u64;
        self.list.push(Plant {
            species,
            cell,
            born,
            biomass: initial_biomass,
            health: 1.0,
            max_age_d,
            rng,
            alive: true,
        });
        idx
    }
    pub fn rebuild_cell_aggregates(&mut self, n_cells: usize) {
        self.cell_biomass.clear();
        self.cell_biomass.resize(n_cells, 0.0);
        self.cell_shade.clear();
        self.cell_shade.resize(n_cells, 0.0);
        if self.by_cell.len() != n_cells {
            self.by_cell = vec![Vec::new(); n_cells];
        } else {
            for v in self.by_cell.iter_mut() {
                v.clear();
            }
        }
        for (pi, p) in self.list.iter().enumerate() {
            if !p.alive {
                continue;
            }
            let sp = &PLANTS[p.species as usize];
            self.cell_biomass[p.cell as usize] += p.biomass;
            self.cell_shade[p.cell as usize] += p.biomass * sp.shade_power / sp.max_biomass;
            self.by_cell[p.cell as usize].push(pi as u32);
        }
    }
}

/// Suitability 0..1 of (temperature, moisture) for a species. The wet side of the moisture
/// curve is forgiving (waterlogging only bites near saturation); the dry side is not.
#[inline]
fn suitability(sp: &crate::species::PlantSpecies, t: f32, m: f32) -> f32 {
    let st = (1.0 - ((t - sp.t_opt) / sp.t_tol).powi(2)).max(0.0);
    let mut dev = (m - sp.m_opt) / sp.m_tol;
    if dev > 0.0 {
        dev *= 0.6;
    }
    let sm = (1.0 - dev * dev).max(0.0);
    st * sm
}

/// Soil moisture fraction a plant experiences (see `Grid::plant_moisture`).
#[inline]
pub fn cell_moisture(sim: &Sim, i: usize) -> f32 {
    sim.grid.plant_moisture(i)
}

/// Seed the initial plant communities from climate suitability — individuals from day zero.
pub fn generate(sim: &mut Sim) {
    let noise = Noise::new(crate::core::rng::splitmix64(sim.cfg.seed ^ 0x7E9E7A17));
    let w = sim.grid.w;
    let h = sim.grid.h;
    let mut rng = Rng::domain(sim.cfg.seed, "vegetation_gen");
    for y in 0..h as i32 {
        let lat = ((y as f32 / h as f32) - 0.5).abs() * 2.0;
        let mean_t = 24.0 - lat * 30.0;
        for x in 0..w as i32 {
            let i = (y as u32 * w + x as u32) as usize;
            if sim.grid.ocean[i] {
                continue;
            }
            let t = mean_t - sim.grid.elev[i].max(0.0) * 14.0;
            let m0 = 0.3 + 0.5 * noise.at2(x as f32 / 20.0, y as f32 / 20.0);
            // pick candidate species by suitability; density from a community-noise field
            let dens = noise.at2(x as f32 / 9.0 + 400.0, y as f32 / 9.0);
            for &spi in &[SP_OAK, SP_PINE, SP_SCRUB, SP_GRASS, SP_REED] {
                let sp = &PLANTS[spi as usize];
                if spi == SP_REED && sim.grid.ground[i] < ground_capacity(&sim.grid, i) * 0.5 {
                    continue;
                }
                let su = suitability(sp, t, m0);
                if su < 0.25 {
                    continue;
                }
                let want = match sp.kind {
                    PlantKind::Tree => (su * dens * 2.2) as i64,
                    PlantKind::Shrub => (su * dens * 1.5) as i64,
                    _ => (su * 1.2) as i64,
                };
                let frac = match sp.kind {
                    PlantKind::Tree => su * dens * 2.2,
                    PlantKind::Shrub => su * dens * 1.5,
                    _ => su * 1.2,
                } - want as f32;
                let count = want + if rng.chance(frac) { 1 } else { 0 };
                for _ in 0..count {
                    let b0 = sp.max_biomass * rng.range_f(0.3, 0.9);
                    let back = rng.range_i(0, (sp.max_age_y * 360.0 * 0.5) as i64);
                    sim.plants.spawn(sim.cfg.seed, spi, i as u32, -back, b0);
                }
            }
        }
    }
    sim.plants.rebuild_cell_aggregates(sim.grid.n());
    refresh_cover(sim);
}

struct PlantPlan {
    d_biomass: f32,
    transpire: f32,
    consume_n: f32,
    died: Option<DeathCause>,
    d_health: f32,
    seeds: Vec<(u32, u8)>,
    rng_after: Rng,
}

/// Daily vegetation phase: evaluate the 1/7 stripe of plants due today.
pub fn tick(sim: &mut Sim) {
    let day = sim.clock.day;
    let n_cells = sim.grid.n();
    sim.plants.rebuild_cell_aggregates(n_cells);

    let dt = PLANT_STRIDE as f32;
    let grid = &sim.grid;
    let plants = &sim.plants;
    let master = sim.cfg.seed;

    let plans: Vec<Option<PlantPlan>> = par_map(plants.list.len(), |pi| {
        let p = &plants.list[pi];
        if !p.alive || (pi as u64 + day) % PLANT_STRIDE != 0 {
            return None;
        }
        let sp = &PLANTS[p.species as usize];
        let i = p.cell as usize;
        let mut rng = p.rng.clone();
        let t = grid.temp[i];
        let m = grid.plant_moisture(i);
        let su = suitability(sp, t, m);
        // light: shade from other plants in the cell (own contribution removed)
        let own_shade = p.biomass * sp.shade_power / sp.max_biomass;
        let shade = (plants.cell_shade[i] - own_shade).max(0.0);
        let light = (1.0 - shade * (1.0 - sp.shade_tol)).clamp(0.0, 1.0);
        // crowding: cell biomass vs capacity
        let crowd = (1.0 - plants.cell_biomass[i] / CELL_CAPACITY).clamp(0.0, 1.0);

        let mut plan = PlantPlan {
            d_biomass: 0.0,
            transpire: 0.0,
            consume_n: 0.0,
            died: None,
            d_health: 0.0,
            seeds: Vec::new(),
            rng_after: p.rng.clone(),
        };

        let age = day as i64 - p.born;
        // death by age: real per-individual lifespan drawn at germination
        if age > p.max_age_d as i64 {
            plan.died = Some(DeathCause::Age);
            plan.rng_after = rng;
            return Some(plan);
        }
        // cold: perennials go DORMANT (mild decay); only hard frost beyond the species'
        // envelope, or winter on annuals, does real damage
        let cold = t < sp.t_opt - sp.t_tol;
        let hard_frost = t < sp.t_opt - 2.2 * sp.t_tol;
        if hard_frost {
            plan.d_health -= 0.12 * dt / 7.0;
        } else if cold && sp.kind == PlantKind::Crop {
            plan.d_health -= 0.10 * dt / 7.0;
        } else if cold {
            plan.d_health -= 0.015 * dt / 7.0; // dormancy cost
        }
        // growth or stress (dormant plants neither grow nor take water stress)
        let nutrients = grid.soil_n[i].clamp(0.0, 1.5);
        let grow_su = su * light * (0.5 + 0.5 * nutrients.min(1.0));
        if grow_su > 0.15 {
            let g = sp.growth * dt * grow_su * crowd * p.biomass.max(0.02)
                * (1.0 - p.biomass / sp.max_biomass).max(0.0);
            plan.d_biomass = g;
            plan.transpire = 0.00015 * dt * p.biomass.min(2.0) * su;
            plan.consume_n = g * 0.01;
            plan.d_health += 0.06 * dt / 7.0; // recovery when conditions are good
        } else if !cold {
            // stressed while active: water/heat/light are the binding constraints
            plan.d_health -= 0.08 * dt / 7.0;
            if m < sp.m_opt - sp.m_tol {
                plan.d_health -= 0.06 * dt / 7.0;
            }
        }
        let health = (p.health + plan.d_health).clamp(0.0, 1.2);
        // Perennial dieback-dormancy: stressed plants brown out and shrink before they die.
        // Death happens when truly hopeless: desiccation, frost beyond the envelope, or having
        // withered away (biomass gone) — matching real perennials instead of annual churn.
        let hopeless = m < 0.05 || hard_frost || p.biomass < 0.015;
        if health <= 0.0 && !hopeless {
            plan.d_health = 0.05 - p.health; // hold at dormant floor
            plan.d_biomass = -p.biomass * 0.05; // wither slowly
            plan.rng_after = rng;
            return Some(plan);
        }
        if health <= 0.0 {
            // attribute death to the weakest of the real constraints
            let m_factor = (1.0 - ((m - sp.m_opt) / sp.m_tol).powi(2)).max(0.0);
            let t_factor = (1.0 - ((t - sp.t_opt) / sp.t_tol).powi(2)).max(0.0);
            plan.died = Some(if m_factor <= t_factor && m_factor <= light {
                if m > sp.m_opt {
                    DeathCause::Drowning // waterlogged, not parched
                } else {
                    DeathCause::Drought
                }
            } else if t_factor <= light {
                DeathCause::Frost
            } else {
                DeathCause::Shade
            });
            plan.rng_after = rng;
            return Some(plan);
        }
        // seeding: mature, healthy plants put seeds into real nearby cells
        if p.biomass >= sp.max_biomass * sp.maturity && health > 0.5 && su > 0.3 {
            if rng.chance(0.06 * dt / 7.0 * su) {
                let (x, y) = ((p.cell % grid.w) as i32, (p.cell / grid.w) as i32);
                let dx = rng.range_i(-(sp.seed_range as i64), sp.seed_range as i64) as i32;
                let dy = rng.range_i(-(sp.seed_range as i64), sp.seed_range as i64) as i32;
                if let Some(j) = {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= grid.w as i32 || ny >= grid.h as i32 {
                        None
                    } else {
                        Some((ny as u32 * grid.w + nx as u32) as usize)
                    }
                } {
                    // a seed only germinates where conditions actually support the species
                    // trees need open ground; grass/reeds can establish as understory
                    let room = match sp.kind {
                        PlantKind::Grass | PlantKind::Reed => {
                            plants.cell_biomass[j] < CELL_CAPACITY
                        }
                        _ => plants.cell_biomass[j] < CELL_CAPACITY * 0.5,
                    };
                    if !grid.ocean[j] && room {
                        // a seedling must survive both today's conditions and its first winter:
                        // estimate the site's winter minimum from latitude/altitude
                        let lat = (((j as u32 / grid.w) as f32 / grid.h as f32) - 0.5).abs() * 2.0;
                        let winter_min_est =
                            24.0 - lat * 30.0 - grid.elev[j].max(0.0) * 14.0 - 9.0 - 5.0;
                        let winter_ok = winter_min_est > sp.t_opt - 2.2 * sp.t_tol;
                        if winter_ok
                            && suitability(sp, grid.temp[j], grid.plant_moisture(j)) > 0.3
                        {
                            plan.seeds.push((j as u32, p.species));
                        }
                    }
                }
            }
        }
        plan.rng_after = rng;
        Some(plan)
    });

    // ordered apply
    let _ = master;
    let mut new_seeds: Vec<(u32, u8)> = Vec::new();
    for (pi, plan) in plans.into_iter().enumerate() {
        let Some(plan) = plan else { continue };
        let (biomass, cell, species) = {
            let p = &mut sim.plants.list[pi];
            p.rng = plan.rng_after;
            p.health = (p.health + plan.d_health).clamp(0.0, 1.2);
            p.biomass = (p.biomass + plan.d_biomass).min(PLANTS[p.species as usize].max_biomass);
            (p.biomass, p.cell, p.species)
        };
        let i = cell as usize;
        sim.grid.ground[i] = (sim.grid.ground[i] - plan.transpire).max(0.0);
        sim.grid.soil_n[i] = (sim.grid.soil_n[i] - plan.consume_n).max(0.0);
        if let Some(cause) = plan.died {
            kill_plant(sim, pi, cause, vec![cause_state_for(sim, i, cause)]);
            let _ = (biomass, species);
            continue;
        }
        new_seeds.extend(plan.seeds);
    }
    for (cell, species) in new_seeds {
        let day = sim.clock.day as i64;
        sim.plants.spawn(sim.cfg.seed, species, cell, day, 0.02);
    }

    // slow nutrient cycle: litter decomposes into soil
    if day % 7 == 3 {
        for i in 0..n_cells {
            let l = sim.grid.litter[i];
            if l > 0.0 {
                let dec = l * 0.02;
                sim.grid.litter[i] -= dec;
                sim.grid.soil_n[i] = (sim.grid.soil_n[i] + dec * 0.5).min(1.5);
            }
        }
    }
    if day % PLANT_STRIDE == 1 {
        refresh_cover(sim);
    }
}

fn cause_state_for(sim: &Sim, i: usize, cause: DeathCause) -> Cause {
    match cause {
        DeathCause::Drought => Cause::State(StateRef {
            what: StateKind::SoilMoisture,
            value: cell_moisture(sim, i),
        }),
        DeathCause::Frost => {
            Cause::State(StateRef { what: StateKind::Temperature, value: sim.grid.temp[i] })
        }
        _ => Cause::State(StateRef { what: StateKind::Fuel, value: sim.plants.cell_biomass[i] }),
    }
}

/// Kill a plant with a cause; biomass becomes litter; a PlantDied event is recorded.
pub fn kill_plant(sim: &mut Sim, pi: usize, cause: DeathCause, causes: Vec<Cause>) {
    let (cell, biomass) = {
        let p = &mut sim.plants.list[pi];
        if !p.alive {
            return;
        }
        p.alive = false;
        (p.cell, p.biomass)
    };
    sim.grid.litter[cell as usize] += biomass;
    sim.history.push(
        sim.clock.day,
        EntityRef::Plant(Plants::id_of(pi)),
        EventKind::PlantDied { cause },
        Some(cell),
        causes,
    );
}

/// Refresh the derived cover observation (`population = count(entities)` in map form).
pub fn refresh_cover(sim: &mut Sim) {
    let n = sim.grid.n();
    if sim.grid.veg_cover.len() != n {
        sim.grid.veg_cover = vec![0.0; n];
    }
    for c in sim.grid.veg_cover.iter_mut() {
        *c = 0.0;
    }
    for p in &sim.plants.list {
        if p.alive {
            let sp = &PLANTS[p.species as usize];
            sim.grid.veg_cover[p.cell as usize] += p.biomass / sp.max_biomass;
        }
    }
}

/// Derived statistics for tests/inspection: (live plants, live trees, tree cover fraction of land).
pub fn forest_stats(sim: &Sim) -> (usize, usize, f32) {
    let mut live = 0;
    let mut trees = 0;
    let mut tree_cells = std::collections::HashSet::new();
    for p in &sim.plants.list {
        if !p.alive {
            continue;
        }
        live += 1;
        if PLANTS[p.species as usize].kind == PlantKind::Tree && p.biomass > 0.5 {
            trees += 1;
            tree_cells.insert(p.cell);
        }
    }
    let land = (0..sim.grid.n()).filter(|&i| !sim.grid.ocean[i]).count();
    (live, trees, tree_cells.len() as f32 / land.max(1) as f32)
}

/// Fuel available to fire in a cell: real litter plus dry standing biomass.
#[inline]
pub fn fuel_at(sim: &Sim, i: usize) -> f32 {
    sim.grid.litter[i] + sim.plants.cell_biomass.get(i).copied().unwrap_or(0.0) * 0.5
}
