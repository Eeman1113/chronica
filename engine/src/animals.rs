//! Individual animals (directive §2.2/§5-Stage-3). Every animal: own state, own RNG stream, own
//! lifespan drawn at birth (fixing the prototype's daily lifespan reroll, AUDIT §6.9), needs,
//! perception through the spatial index (never omniscient), the shared utility brain with
//! recorded rationale, real pursuit predation (a kill requires having actually closed the
//! distance; dice only decide the struggle's outcome), courtship on real co-location, gestation,
//! birth with gene inheritance, and death always with a cause event. Population counts are
//! derived. Reproduction has no world pulse and no species cap: it is bounded by food, predation,
//! and mate-finding (replacing AUDIT §6.9's fortnightly pulse + `sp.cap`).

use crate::brains::{decide, Action, AnimalMindInput, Percepts, Rationale};
use crate::core::ids::{AnimalId, EntityRef};
use crate::core::jobs::par_map;
use crate::core::rng::Rng;
use crate::history::{Cause, DeathCause, EventKind, StateKind, StateRef};
use crate::sim::Sim;
use crate::species::{Diet, ANIMALS, PLANTS};
use crate::world::SpatialIndex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Animal {
    pub species: u8,
    pub sex: u8, // 0 f, 1 m
    pub born: i64, // may be negative: seeded adults were born before day 0
    pub lifespan_d: u64, // drawn once at birth from own stream
    pub x: i32,
    pub y: i32,
    pub condition: f32, // 0..1 body condition
    pub hunger: f32,
    pub thirst: f32,
    pub fatigue: f32,
    pub fear: f32,
    pub genes: [f32; 4],
    pub generation: u32,
    pub mother: AnimalId,
    pub father: AnimalId,
    pub pregnant_by: AnimalId, // NONE if not pregnant
    pub due_day: u64,
    pub water_memory: Option<(i32, i32)>,
    pub forage_memory: Option<(i32, i32)>, // remembered good grazing
    pub danger_memory: Option<(i32, i32)>, // remembered attack site
    pub infection: Option<(u8, u64)>,      // pathogen, day caught
    pub tamed_by: crate::core::ids::PersonId, // NONE = wild
    pub tame_prog: f32,
    pub days_starving: u16,
    pub days_thirsty: u16,
    pub man_kills: u16,       // people this beast has taken — the road to a name
    pub rationale: Rationale, // last decision's recorded reasoning
    pub rng: Rng,
    pub alive: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Corpse {
    pub species: u8,
    pub cell: u32,
    pub day: u64,
    pub mass: f32,
    pub of_animal: AnimalId,
    pub bait: bool,                       // staked by people to draw a predator
    pub staker: crate::core::ids::PersonId,
    pub rot: f32,                         // decay 0..1: fresh -> ripe -> rotten -> bones
    pub gone: bool,
}

/// A corpse's visible decay stage (prototype's fresh/ripe/rotten/bones), from its rot.
#[inline]
pub fn corpse_stage(rot: f32) -> u8 {
    if rot < 0.15 { 0 } else if rot < 0.45 { 1 } else if rot < 0.82 { 2 } else { 3 }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Animals {
    pub list: Vec<Animal>,
    pub corpses: Vec<Corpse>,
    /// Man-eaters earn names: arena index -> the name a terrified people gave it.
    pub names: std::collections::BTreeMap<u32, String>,
    #[serde(skip)]
    pub index: SpatialIndex,
}

impl Animals {
    pub fn id_of(idx: usize) -> AnimalId {
        AnimalId::from_index(idx)
    }
    pub fn spawn(
        &mut self,
        master: u64,
        species: u8,
        x: i32,
        y: i32,
        born: i64,
        mother: AnimalId,
        father: AnimalId,
        inherit: Option<([f32; 4], u32)>,
    ) -> usize {
        let idx = self.list.len();
        let mut rng = Rng::entity(master, "animals", idx as u64);
        let sp = &ANIMALS[species as usize];
        let lifespan_d = (sp.lifespan_y * 360.0 * rng.range_f(0.65, 1.35)) as u64;
        let (genes, generation) = match inherit {
            Some((mut g, gen)) => {
                for gi in g.iter_mut() {
                    *gi = (*gi * rng.range_f(0.95, 1.05)).clamp(0.3, 2.5);
                }
                (g, gen + 1)
            }
            None => {
                let base = [1.0, 1.0, 1.0, sp.herd];
                (base.map(|v: f32| (v * rng.range_f(0.85, 1.15)).clamp(0.3, 2.5)), 0)
            }
        };
        let sex = if rng.chance(0.5) { 0 } else { 1 };
        self.list.push(Animal {
            species,
            sex,
            born,
            lifespan_d,
            x,
            y,
            condition: 1.0,
            hunger: 0.2,
            thirst: 0.2,
            fatigue: 0.0,
            fear: 0.0,
            genes,
            generation,
            mother,
            father,
            pregnant_by: AnimalId::NONE,
            due_day: 0,
            water_memory: None,
            forage_memory: None,
            danger_memory: None,
            infection: None,
            tamed_by: crate::core::ids::PersonId::NONE,
            tame_prog: 0.0,
            days_starving: 0,
            days_thirsty: 0,
            man_kills: 0,
            rationale: Rationale::default(),
            rng,
            alive: true,
        });
        idx
    }
    pub fn rebuild_index(&mut self, w: u32, h: u32) {
        if self.index.is_empty_dims() {
            self.index = SpatialIndex::new(w, h);
        }
        self.index.clear();
        for (i, a) in self.list.iter().enumerate() {
            if a.alive {
                self.index.insert(i as u32, a.x, a.y);
            }
        }
    }
}

/// Habitat-driven initial populations: animals seeded where their climate band and (for
/// herbivores) real forage exist. Counts scale with land area, not authored world caps.
pub fn generate(sim: &mut Sim) {
    let mut rng = Rng::domain(sim.cfg.seed, "animals_gen");
    let land: Vec<usize> = (0..sim.grid.n()).filter(|&i| !sim.grid.ocean[i]).collect();
    let per_species_density: &[(u8, f32)] = &[
        (crate::species::A_HARE, 0.010),
        (crate::species::A_DEER, 0.004),
        (crate::species::A_BOAR, 0.002),
        (crate::species::A_WOLF, 0.0005),
        (crate::species::A_BEAR, 0.0003),
        (crate::species::A_SHEEP, 0.002),
        (crate::species::A_HORSE, 0.001),
        (crate::species::A_AUROCHS, 0.001),
    ];
    // trout fill the rivers and lakes the rain has made
    {
        let mut placed = 0;
        for i in 0..sim.grid.n() {
            if sim.grid.is_river(i) || sim.grid.is_lake(i) {
                if rng.chance(0.5) {
                    let (x, y) = sim.grid.xy(i);
                    let back = rng.range_i(0, (7.0f32 * 360.0 * 0.5) as i64);
                    sim.animals.spawn(
                        sim.cfg.seed,
                        crate::species::A_FISH,
                        x,
                        y,
                        -back,
                        AnimalId::NONE,
                        AnimalId::NONE,
                        None,
                    );
                    placed += 1;
                    if placed > 900 {
                        break;
                    }
                }
            }
        }
    }
    let h = sim.grid.h as f32;
    for &(spi, dens) in per_species_density {
        let sp = &ANIMALS[spi as usize];
        let want = (land.len() as f32 * dens) as usize;
        let mut placed = 0;
        let mut tries = 0;
        while placed < want && tries < want * 30 {
            tries += 1;
            let i = land[rng.pick_index(land.len())];
            let (x, y) = sim.grid.xy(i);
            let lat = ((y as f32 / h) - 0.5).abs() * 2.0;
            let mean_t = 24.0 - lat * 30.0 - sim.grid.elev[i].max(0.0) * 14.0;
            if mean_t < sp.t_min || mean_t > sp.t_max {
                continue;
            }
            if sp.diet != Diet::Carnivore && sim.grid.veg_cover[i] < 0.15 {
                continue;
            }
            let back = rng.range_i(0, (sp.lifespan_y * 360.0 * 0.5) as i64);
            sim.animals.spawn(sim.cfg.seed, spi, x, y, -back, AnimalId::NONE, AnimalId::NONE, None);
            placed += 1;
        }
    }
}

/// Is `hunter_sp` a predator of `prey_sp`?
#[inline]
fn preys_on(hunter_sp: u8, prey_sp: u8) -> bool {
    ANIMALS[hunter_sp as usize].prey.contains(&prey_sp)
}

struct AnimalPlan {
    action: Action,
    rationale: Rationale,
    percept_threat: Option<u32>,
    water_seen: Option<(i32, i32)>,
}

pub fn tick(sim: &mut Sim) {
    let day = sim.clock.day;
    sim.animals.rebuild_index(sim.grid.w, sim.grid.h);

    // ---------- perception + cognition (parallel, read-only) ----------
    let animals = &sim.animals;
    let grid = &sim.grid;
    let plants = &sim.plants;
    let sim_humans_index = &sim.humans.index;
    let sim_humans_list = &sim.humans.list;
    let plans: Vec<Option<AnimalPlan>> = par_map(animals.list.len(), |ai| {
        let a = &animals.list[ai];
        if !a.alive {
            return None;
        }
        let sp = &ANIMALS[a.species as usize];
        let mut p = Percepts::default();
        p.threat_dist = i32::MAX;
        p.prey_dist = i32::MAX;
        p.mate_dist = i32::MAX;
        p.human_prey_dist = i32::MAX;
        let r = sp.perception;
        let mut herd_sum = (0i64, 0i64, 0i64);
        animals.index.for_each_near(a.x, a.y, r, |oi| {
            if oi as usize == ai {
                return;
            }
            let o = &animals.list[oi as usize];
            if !o.alive {
                return;
            }
            let d = (o.x - a.x).abs().max((o.y - a.y).abs());
            if d > r {
                return;
            }
            if preys_on(o.species, a.species) && d < p.threat_dist {
                p.threat_dist = d;
                p.threat_idx = Some(oi);
            }
            if preys_on(a.species, o.species) && d < p.prey_dist {
                p.prey_dist = d;
                p.prey_idx = Some(oi);
            }
            if o.species == a.species {
                if o.sex != a.sex
                    && d < p.mate_dist
                    && (day as i64 - o.born) as f32 / 360.0 >= sp.maturity_y
                    && o.pregnant_by.is_none()
                {
                    p.mate_dist = d;
                    p.mate_idx = Some(oi);
                }
                herd_sum.0 += o.x as i64;
                herd_sum.1 += o.y as i64;
                herd_sum.2 += 1;
            }
        });
        if herd_sum.2 >= 2 {
            p.herd_center =
                Some(((herd_sum.0 / herd_sum.2) as i32, (herd_sum.1 / herd_sum.2) as i32));
        }
        // a tamed beast stays with its keeper: the herd is wherever they are
        if let Some(owner) = a.tamed_by.some() {
            if let Some(o) = sim_humans_list.get(owner.index()) {
                if o.alive {
                    p.herd_center = Some(o.camp);
                }
            }
        }
        // a desperate predator sees people too
        if sp.diet != Diet::Herbivore && sp.mass > 30.0 && a.hunger > 1.0 {
            let humans = &sim_humans_index;
            humans.for_each_near(a.x, a.y, r, |hi2| {
                let hq = &sim_humans_list[hi2 as usize];
                if !hq.alive {
                    return;
                }
                let d = (hq.x - a.x).abs().max((hq.y - a.y).abs());
                if d <= r && d < p.human_prey_dist {
                    p.human_prey_dist = d;
                    p.human_prey = Some(hi2);
                }
            });
        }
        // forage: edible biomass here + best nearby cell scanned in a small ring
        let edible_local = |cell: usize| -> f32 {
            if grid.surface[cell] > 0.12 {
                return plants
                    .by_cell
                    .get(cell)
                    .map(|v| {
                        v.iter()
                            .map(|&pi| {
                                let pl = &plants.list[pi as usize];
                                if pl.species == crate::species::SP_REED {
                                    pl.biomass * PLANTS[pl.species as usize].edible
                                } else {
                                    0.0
                                }
                            })
                            .sum()
                    })
                    .unwrap_or(0.0);
            }
            plants
                .by_cell
                .get(cell)
                .map(|v| {
                    v.iter()
                        .map(|&pi| {
                            let pl = &plants.list[pi as usize];
                            pl.biomass * PLANTS[pl.species as usize].edible
                        })
                        .sum()
                })
                .unwrap_or(0.0)
        };
        let here = grid.idx(a.x, a.y).unwrap();
        p.forage_here = edible_local(here);
        if sp.diet != Diet::Carnivore {
            let fr = sp.perception;
            let mut best = 0.0;
            for dy in -fr..=fr {
                for dx in -fr..=fr {
                    if let Some(j) = grid.idx(a.x + dx, a.y + dy) {
                        if grid.ocean[j] {
                            continue;
                        }
                        let e = edible_local(j);
                        if e > best + 0.05 {
                            best = e;
                            p.forage_near = Some((a.x + dx, a.y + dy));
                            p.forage_near_val = e;
                        }
                    }
                }
            }
        }
        // water: current perception ring, else memory
        'water: for rr in 0..=4i32 {
            for dy in -rr..=rr {
                for dx in -rr..=rr {
                    if dx.abs() != rr && dy.abs() != rr {
                        continue;
                    }
                    if let Some(j) = grid.idx(a.x + dx, a.y + dy) {
                        if grid.is_fresh_water(j) {
                            p.water_near = Some((a.x + dx, a.y + dy));
                            break 'water;
                        }
                    }
                }
            }
        }
        let water_seen = p.water_near;
        if p.water_near.is_none() {
            p.water_near = a.water_memory;
        }
        if p.forage_near.is_none() && a.hunger > 0.6 {
            if let Some(m) = a.forage_memory {
                // nothing in sight: head for the ground that fed you before
                if (m.0 - a.x).abs().max((m.1 - a.y).abs()) > 2 {
                    p.forage_near = Some(m);
                    p.forage_near_val = 0.2;
                }
            }
        }
        // carrion for meat-eaters
        if sp.diet != Diet::Herbivore {
            for (ci, c) in animals.corpses.iter().enumerate() {
                if c.gone || c.mass < 1.0 {
                    continue;
                }
                let (cx, cy) = grid.xy(c.cell as usize);
                if (cx - a.x).abs().max((cy - a.y).abs()) <= r {
                    p.carrion_near = Some(ci as u32);
                    break;
                }
            }
        }
        p.fire_near = {
            let mut f = false;
            'fire: for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    if let Some(j) = grid.idx(a.x + dx, a.y + dy) {
                        if grid.burning[j] > 0 {
                            f = true;
                            break 'fire;
                        }
                    }
                }
            }
            f
        };

        let age_y = (day as i64 - a.born) as f32 / 360.0;
        let inp = AnimalMindInput {
            hunger: a.hunger,
            thirst: a.thirst,
            fatigue: a.fatigue,
            fear: a.fear,
            genes: a.genes,
            is_carnivore: sp.diet != Diet::Herbivore,
            is_herbivore: sp.diet != Diet::Carnivore,
            is_aquatic: sp.aquatic,
            mature: age_y >= sp.maturity_y,
            pregnant: !a.pregnant_by.is_none(),
        };
        let (action, rationale) = decide(&inp, &p, (a.x, a.y));
        Some(AnimalPlan { action, rationale, percept_threat: p.threat_idx, water_seen })
    });

    // ---------- action resolution (ordered, mutating) ----------
    let mut deaths: Vec<(usize, DeathCause, Vec<Cause>)> = Vec::new();
    let mut births: Vec<(u8, i32, i32, AnimalId, AnimalId, [f32; 4], u32)> = Vec::new();
    for ai in 0..sim.animals.list.len() {
        let Some(plan) = &plans[ai] else { continue };
        if !sim.animals.list[ai].alive {
            continue;
        }
        let sp = &ANIMALS[sim.animals.list[ai].species as usize];
        sim.animals.list[ai].rationale = plan.rationale;
        if let Some(w) = plan.water_seen {
            sim.animals.list[ai].water_memory = Some(w);
        }
        match plan.action {
            Action::Flee { .. } => {
                let (tx, ty) = if let Some(t) = plan.percept_threat {
                    let t = &sim.animals.list[t as usize];
                    (t.x, t.y)
                } else {
                    (sim.animals.list[ai].x, sim.animals.list[ai].y)
                };
                let a = &sim.animals.list[ai];
                let dir = ((a.x - tx).signum(), (a.y - ty).signum());
                move_animal(sim, ai, dir, sp.speed + 1);
                let a = &mut sim.animals.list[ai];
                a.fatigue = (a.fatigue + 0.25).min(1.5);
                a.fear = (a.fear * 0.8 + 0.4).min(2.0);
            }
            Action::Drink { at } => {
                let a = &sim.animals.list[ai];
                if (a.x, a.y) == at || grid_fresh(sim, a.x, a.y) {
                    let a = &mut sim.animals.list[ai];
                    a.thirst = 0.0;
                    a.days_thirsty = 0;
                } else {
                    move_animal_toward(sim, ai, at, sp.speed);
                    let a2 = &sim.animals.list[ai];
                    if grid_fresh(sim, a2.x, a2.y) {
                        let a2 = &mut sim.animals.list[ai];
                        a2.thirst = 0.0;
                        a2.days_thirsty = 0;
                    }
                }
            }
            Action::Graze => {
                graze(sim, ai);
            }
            Action::MoveToForage { to } => {
                move_animal_toward(sim, ai, to, sp.speed.min(3));
                graze(sim, ai);
            }
            Action::Hunt { target } => {
                hunt(sim, ai, target as usize, &mut deaths);
            }
            Action::HuntHuman { target } => {
                hunt_human(sim, ai, target as usize);
            }
            Action::Scavenge { corpse } => {
                scavenge(sim, ai, corpse as usize);
            }
            Action::Court { mate } => {
                court(sim, ai, mate as usize);
            }
            Action::JoinHerd { to } => {
                move_animal_toward(sim, ai, to, 1);
            }
            Action::Rest => {
                let a = &mut sim.animals.list[ai];
                a.fatigue = (a.fatigue - 0.4).max(0.0);
            }
            Action::Roam => {
                let dir = {
                    let a = &mut sim.animals.list[ai];
                    ((a.rng.range_i(-1, 1)) as i32, (a.rng.range_i(-1, 1)) as i32)
                };
                move_animal(sim, ai, dir, sp.speed.min(3));
            }
        }
    }

    // ---------- aquatic physiology: water is breath; stranding kills ----------
    for ai in 0..sim.animals.list.len() {
        let (aquatic, x, y, alive) = {
            let a = &sim.animals.list[ai];
            (
                ANIMALS[a.species as usize].aquatic,
                a.x,
                a.y,
                a.alive,
            )
        };
        if !alive || !aquatic {
            continue;
        }
        let here = sim.grid.idx(x, y);
        let habitable_here = here.map(|j| wet_enough(sim, j)).unwrap_or(false);
        // stranded by the falling water? make for the nearest true river or lake
        if !habitable_here {
            let mut best: Option<((i32, i32), f32)> = None;
            for r in 1..=4i32 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs() != r && dy.abs() != r {
                            continue;
                        }
                        if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                            if wet_enough(sim, j) {
                                let d = sim.grid.surface[j];
                                if best.map(|(_, bd)| d > bd).unwrap_or(true) {
                                    best = Some(((x + dx, y + dy), d));
                                }
                            }
                        }
                    }
                }
                if best.is_some() {
                    break;
                }
            }
            if let Some((to, _)) = best {
                let a = &mut sim.animals.list[ai];
                a.x = to.0;
                a.y = to.1;
            }
        }
        let wet = sim
            .grid
            .idx(sim.animals.list[ai].x, sim.animals.list[ai].y)
            .map(|j| wet_enough(sim, j))
            .unwrap_or(false);
        {
            let a = &mut sim.animals.list[ai];
            a.thirst = 0.0;
            if wet {
                a.days_thirsty = 0;
            } else {
                a.days_thirsty += 1;
            }
        }
        if sim.animals.list[ai].days_thirsty > 1 {
            kill_animal(
                sim,
                ai,
                DeathCause::Exposure,
                vec![Cause::State(StateRef { what: StateKind::SurfaceWater, value: 0.0 })],
                true,
            );
        }
    }

    // ---------- physiology (ordered) ----------
    for ai in 0..sim.animals.list.len() {
        if !sim.animals.list[ai].alive {
            continue;
        }
        // a flood under your feet is an emergency: wade toward the nearest dry ground
        if !ANIMALS[sim.animals.list[ai].species as usize].aquatic {
            let (x, y) = {
                let a = &sim.animals.list[ai];
                (a.x, a.y)
            };
            if let Some(here) = sim.grid.idx(x, y) {
                if sim.grid.ocean[here] || sim.grid.surface[here] > 0.12 {
                    let mut best: Option<((i32, i32), f32)> = None;
                    for r in 1..=4i32 {
                        for dy in -r..=r {
                            for dx in -r..=r {
                                if dx.abs() != r && dy.abs() != r {
                                    continue;
                                }
                                if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                                    if !sim.grid.ocean[j] {
                                        let d = sim.grid.surface[j];
                                        if d <= 0.12
                                            && best.map(|(_, bd)| d < bd).unwrap_or(true)
                                        {
                                            best = Some(((x + dx, y + dy), d));
                                        }
                                    }
                                }
                            }
                        }
                        if best.is_some() {
                            break;
                        }
                    }
                    if let Some((to, _)) = best {
                        // wading ignores the depth rule — that's what escaping a flood is
                        let a = &mut sim.animals.list[ai];
                        a.x = to.0;
                        a.y = to.1;
                        a.fatigue = (a.fatigue + 0.3).min(1.5);
                    }
                }
            }
        } else {
            // aquatic: keep thirst zeroed in the shared block below
        }
        let sp = &ANIMALS[sim.animals.list[ai].species as usize];
        let (starved, parched, aged, froze, birth) = {
            let a = &mut sim.animals.list[ai];
            a.hunger = (a.hunger + sp.hunger_rate * 0.7).min(2.0);
            a.thirst = (a.thirst + 0.14).min(2.0);
            a.fatigue = (a.fatigue - 0.1).max(0.0);
            a.fear *= 0.85;
            if a.hunger >= 1.0 {
                a.days_starving += 1;
                a.condition -= 0.02;
            } else if a.days_starving > 0 {
                a.days_starving -= 1;
                a.condition = (a.condition + 0.01).min(1.0);
            }
            if a.thirst >= 1.0 {
                a.days_thirsty += 1;
            } else {
                a.days_thirsty = 0;
            }
            let starved = a.days_starving as f32 > 24.0 + sp.mass.sqrt();
            let parched = a.days_thirsty > 8;
            let aged = (day as i64 - a.born) > a.lifespan_d as i64;
            // a starving mother's body sheds the pregnancy before it can kill her
            if !a.pregnant_by.is_none() && a.condition < 0.2 && a.rng.chance(0.03) {
                a.pregnant_by = AnimalId::NONE;
            }
            let birth = !a.pregnant_by.is_none() && day >= a.due_day;
            (starved, parched, aged, false, birth)
        };
        let _ = froze;
        if aged {
            deaths.push((ai, DeathCause::Age, vec![]));
            continue;
        }
        if starved {
            let h = sim.animals.list[ai].hunger;
            deaths.push((
                ai,
                DeathCause::Starvation,
                vec![Cause::State(StateRef { what: StateKind::Hunger, value: h })],
            ));
            continue;
        }
        if parched {
            let t = sim.animals.list[ai].thirst;
            deaths.push((
                ai,
                DeathCause::Thirst,
                vec![Cause::State(StateRef { what: StateKind::Thirst, value: t })],
            ));
            continue;
        }
        if birth {
            let (spi, x, y, mid, fid, genes, generation, litter) = {
                let a = &mut sim.animals.list[ai];
                let father = a.pregnant_by;
                a.pregnant_by = AnimalId::NONE;
                let sp = &ANIMALS[a.species as usize];
                let litter = a.rng.range_i(sp.litter.0, sp.litter.1);
                (
                    a.species,
                    a.x,
                    a.y,
                    Animals::id_of(ai),
                    father,
                    a.genes,
                    a.generation,
                    litter,
                )
            };
            for _ in 0..litter {
                births.push((spi, x, y, mid, fid, genes, generation));
            }
        }
    }

    // ---------- sickness among beasts: born of crowding, passed nose to nose ----------
    if day % 5 == 2 {
        let n_a = sim.animals.list.len();
        let mut new_cases: Vec<(usize, u8, Option<usize>)> = Vec::new();
        for ai in 0..n_a {
            let (alive, x, y, sp, infected) = {
                let a = &sim.animals.list[ai];
                (a.alive, a.x, a.y, a.species, a.infection)
            };
            if !alive {
                continue;
            }
            // count the actual crowd around this animal
            let mut crowd = 0;
            let mut sick_neighbor: Option<usize> = None;
            sim.animals.index.for_each_near(x, y, 2, |oi| {
                let o = &sim.animals.list[oi as usize];
                if o.alive && oi as usize != ai {
                    crowd += 1;
                    if o.infection.is_some() && o.species == sp {
                        sick_neighbor = Some(oi as usize);
                    }
                }
            });
            if infected.is_none() {
                if let Some(src) = sick_neighbor {
                    let pg = sim.animals.list[src].infection.unwrap().0;
                    let catches = {
                        let a = &mut sim.animals.list[ai];
                        a.rng.chance(0.15)
                    };
                    if catches {
                        new_cases.push((ai, pg, Some(src)));
                    }
                } else if crowd >= 7 {
                    // dense herds breed the murrain; marshes breed the fever — the crowding
                    // and the wet ground are the cited causes, never a calendar
                    let wet = sim
                        .grid
                        .idx(x, y)
                        .map(|i| sim.grid.surface[i] > 0.02)
                        .unwrap_or(false);
                    let pg = if wet { 0u8 } else { 1u8 };
                    let sparked = {
                        let a = &mut sim.animals.list[ai];
                        a.rng.chance(0.004)
                    };
                    if sparked {
                        new_cases.push((ai, pg, None));
                    }
                }
            } else if let Some((_pg, since)) = infected {
                if day.saturating_sub(since) > 30 {
                    sim.animals.list[ai].infection = None; // recovered
                } else {
                    let a = &mut sim.animals.list[ai];
                    a.condition = (a.condition - 0.01).max(0.1);
                }
            }
        }
        for (ai, pg, _src) in new_cases {
            let d = sim.clock.day;
            sim.animals.list[ai].infection = Some((pg, d));
        }
    }

    // deaths → events + corpses (skip corpse if body was consumed in a hunt — hunt handles it)
    for (ai, cause, causes) in deaths {
        kill_animal(sim, ai, cause, causes, true);
    }
    // births → real newborns with real parents
    for (spi, x, y, mid, fid, genes, generation) in births {
        let idx =
            sim.animals.spawn(sim.cfg.seed, spi, x, y, day as i64, mid, fid, Some((genes, generation)));
        sim.history.push(
            day,
            EntityRef::Animal(Animals::id_of(idx)),
            EventKind::Born { mother: EntityRef::Animal(mid), father: EntityRef::Animal(fid) },
            sim.grid.idx(x, y).map(|i| i as u32),
            vec![],
        );
    }

    // corpses decay by real weather: warmth quickens rot, deep cold nearly halts it; every
    // stage returns a little of the flesh to the soil, and bones finally weather away
    {
        let mut fert: Vec<(usize, f32)> = Vec::new();
        for c in sim.animals.corpses.iter_mut() {
            if c.gone {
                continue;
            }
            let t = sim.grid.temp[c.cell as usize];
            let season_rate = if t < -2.0 {
                0.006 // frozen: a body can last a whole winter
            } else if t < 8.0 {
                0.020
            } else if t < 18.0 {
                0.035
            } else {
                0.055 // summer heat: gone in a few weeks
            };
            let before = corpse_stage(c.rot);
            c.rot = (c.rot + season_rate).min(1.2);
            let after = corpse_stage(c.rot);
            // each time it passes into a riper stage, some mass soaks into the ground
            if after > before {
                let give = (c.mass * 0.03).min(0.25);
                fert.push((c.cell as usize, give));
                c.mass = (c.mass - give).max(0.0);
            }
            if c.rot >= 1.2 || c.mass <= 0.2 {
                fert.push((c.cell as usize, (c.mass * 0.05).min(0.3)));
                c.gone = true;
            }
        }
        for (cell, n) in fert {
            sim.grid.soil_n[cell] = (sim.grid.soil_n[cell] + n).min(1.5);
        }
    }
    if day % 30 == 11 {
        sim.animals.corpses.retain(|c| !c.gone);
    }
}

#[inline]
fn grid_fresh(sim: &Sim, x: i32, y: i32) -> bool {
    // the bank counts: an animal drinks from the water's edge
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                if sim.grid.is_fresh_water(j) {
                    return true;
                }
            }
        }
    }
    false
}

#[allow(dead_code)]
fn edible_at_g(sim: &Sim, cell: usize) -> f32 {
    if sim.grid.surface[cell] > 0.12 {
        // drowned ground: only reeds stand above the water
        return sim
            .plants
            .by_cell
            .get(cell)
            .map(|v| {
                v.iter()
                    .map(|&pi| {
                        let p = &sim.plants.list[pi as usize];
                        if p.species == crate::species::SP_REED {
                            p.biomass * PLANTS[p.species as usize].edible
                        } else {
                            0.0
                        }
                    })
                    .sum()
            })
            .unwrap_or(0.0);
    }
    sim.plants
        .by_cell
        .get(cell)
        .map(|v| {
            v.iter()
                .map(|&pi| {
                    let p = &sim.plants.list[pi as usize];
                    p.biomass * PLANTS[p.species as usize].edible
                })
                .sum()
        })
        .unwrap_or(0.0)
}

/// Deep water is no place for a land animal (floods are real and must be fled).
#[inline]
fn wet_enough(sim: &Sim, j: usize) -> bool {
    // a fish can swim in a river, a lake, or a genuine flood — but NOT a thin meltwater film
    // over the forest floor. When the flood drains below this depth the fish is stranded.
    !sim.grid.ocean[j]
        && (sim.grid.is_river(j) || sim.grid.is_lake(j) || sim.grid.surface[j] > 0.05)
}

#[inline]
fn too_deep(sim: &Sim, j: usize) -> bool {
    if sim.grid.ice_bears(j) {
        return false; // winter roads
    }
    sim.grid.ocean[j] || sim.grid.surface[j] > 0.12
}

/// Step toward a destination, stopping ON it.
fn move_animal_toward(sim: &mut Sim, ai: usize, to: (i32, i32), steps: i32) {
    let aquatic = ANIMALS[sim.animals.list[ai].species as usize].aquatic;
    for _ in 0..steps {
        let (dir, arrived) = {
            let a = &sim.animals.list[ai];
            (((to.0 - a.x).signum(), (to.1 - a.y).signum()), a.x == to.0 && a.y == to.1)
        };
        if arrived {
            break;
        }
        let (nx, ny) = {
            let a = &sim.animals.list[ai];
            (a.x + dir.0, a.y + dir.1)
        };
        match sim.grid.idx(nx, ny) {
            Some(j) if (aquatic && wet_enough(sim, j)) || (!aquatic && !too_deep(sim, j)) => {
                let a = &mut sim.animals.list[ai];
                a.x = nx;
                a.y = ny;
            }
            // a land animal can swim a river or flood to cross it — tiring, one stroke per step
            Some(j) if !aquatic && sim.grid.swimmable(j) => {
                let a = &mut sim.animals.list[ai];
                a.x = nx;
                a.y = ny;
                a.fatigue = (a.fatigue + 0.3).min(1.5);
                break;
            }
            _ => break,
        }
    }
}

fn move_animal(sim: &mut Sim, ai: usize, dir: (i32, i32), steps: i32) {
    let aquatic = ANIMALS[sim.animals.list[ai].species as usize].aquatic;
    for _ in 0..steps {
        let (nx, ny) = {
            let a = &sim.animals.list[ai];
            (a.x + dir.0, a.y + dir.1)
        };
        match sim.grid.idx(nx, ny) {
            Some(j) if (aquatic && wet_enough(sim, j)) || (!aquatic && !too_deep(sim, j)) => {
                let a = &mut sim.animals.list[ai];
                a.x = nx;
                a.y = ny;
            }
            Some(j) if !aquatic && sim.grid.swimmable(j) => {
                let a = &mut sim.animals.list[ai];
                a.x = nx;
                a.y = ny;
                a.fatigue = (a.fatigue + 0.3).min(1.5);
                break;
            }
            _ => break,
        }
    }
}

/// Herbivore browsing: eats real plant biomass in its cell, largest edible plant first. A plant
/// browsed to nothing dies with cause Browsed.
fn graze(sim: &mut Sim, ai: usize) {
    let (cell, mut need) = {
        let a = &sim.animals.list[ai];
        let sp = &ANIMALS[a.species as usize];
        (
            sim.grid.idx(a.x, a.y).unwrap(),
            (a.hunger.max(0.3) * sp.mass * 0.006).max(0.05),
        )
    };
    let flooded = sim.grid.surface[cell] > 0.12;
    let Some(pids) = sim.plants.by_cell.get(cell).cloned() else { return };
    let mut eaten = 0.0f32;
    let mut kills: Vec<usize> = Vec::new();
    for &pi in pids.iter() {
        if need <= 0.0 {
            break;
        }
        let p = &mut sim.plants.list[pi as usize];
        if !p.alive {
            continue;
        }
        if flooded && p.species != crate::species::SP_REED {
            continue; // you cannot graze under water
        }
        let ed = PLANTS[p.species as usize].edible;
        if ed < 0.2 {
            continue;
        }
        let bite = need.min(p.biomass * 0.8);
        p.biomass -= bite;
        eaten += bite * ed;
        need -= bite;
        if p.biomass < 0.01 {
            kills.push(pi as usize);
        }
    }
    for pi in kills {
        crate::vegetation::kill_plant(
            sim,
            pi,
            DeathCause::Browsed,
            vec![Cause::State(StateRef { what: StateKind::Hunger, value: 1.0 })],
        );
    }
    // bottom-feeders: aquatic grazers also sift real detritus from the bed
    if ANIMALS[sim.animals.list[ai].species as usize].aquatic && need > 0.0 {
        let lit = sim.grid.litter[cell];
        let sift = (need * 2.0).min(lit * 0.3);
        if sift > 0.0 {
            sim.grid.litter[cell] -= sift;
            eaten += sift * 0.5;
        }
    }
    let a = &mut sim.animals.list[ai];
    let sp = &ANIMALS[a.species as usize];
    if eaten > sp.mass * 0.002 {
        a.forage_memory = Some((a.x, a.y)); // this ground fed me
    }
    a.hunger = (a.hunger - eaten / (sp.mass * 0.003).max(0.05)).max(0.0);
}

/// Pursuit predation. The hunter moves toward its actually-perceived target; only if it closes
/// to adjacency does a struggle happen, resolved by real body state with the hunter's own dice.
fn hunt(sim: &mut Sim, ai: usize, ti: usize, deaths: &mut Vec<(usize, DeathCause, Vec<Cause>)>) {
    if ti >= sim.animals.list.len() || !sim.animals.list[ti].alive {
        return;
    }
    let (dir, dist) = {
        let a = &sim.animals.list[ai];
        let t = &sim.animals.list[ti];
        (
            ((t.x - a.x).signum(), (t.y - a.y).signum()),
            (t.x - a.x).abs().max((t.y - a.y).abs()),
        )
    };
    let speed = ANIMALS[sim.animals.list[ai].species as usize].speed;
    if dist > 1 {
        let to = {
            let t = &sim.animals.list[ti];
            (t.x, t.y)
        };
        move_animal_toward(sim, ai, to, speed);
        let _ = dir;
        let a = &mut sim.animals.list[ai];
        a.fatigue = (a.fatigue + 0.2).min(1.5);
    }
    let dist_now = {
        let a = &sim.animals.list[ai];
        let t = &sim.animals.list[ti];
        (t.x - a.x).abs().max((t.y - a.y).abs())
    };
    if dist_now > 1 {
        return; // did not close — no kill without a real approach
    }
    // struggle: power from mass, condition, boldness; prey escape from speed and wariness
    let (p_kill, hunter_id) = {
        let a = &sim.animals.list[ai];
        let t = &sim.animals.list[ti];
        let asp = &ANIMALS[a.species as usize];
        let tsp = &ANIMALS[t.species as usize];
        let att = asp.mass * a.condition * (0.6 + a.genes[2] * 0.5);
        let def = tsp.mass * t.condition * (0.5 + t.genes[0] * 0.4)
            + tsp.speed as f32 * 6.0 * (1.0 - t.fatigue.min(1.0) * 0.5);
        (((att / (att + def)) * 0.42).clamp(0.03, 0.7), Animals::id_of(ai))
    };
    let roll = sim.animals.list[ai].rng.chance(p_kill);
    if roll {
        // record the kill; body is partly eaten now, the rest is a real corpse
        let (t_species, t_cell, t_mass) = {
            let t = &sim.animals.list[ti];
            let m = ANIMALS[t.species as usize].mass * (0.5 + t.condition * 0.5);
            (t.species, sim.grid.idx(t.x, t.y).unwrap() as u32, m)
        };
        let kill_ev = sim.history.push(
            sim.clock.day,
            EntityRef::Animal(Animals::id_of(ti)),
            EventKind::Killed { by: EntityRef::Animal(hunter_id) },
            Some(t_cell),
            vec![Cause::State(StateRef {
                what: StateKind::Hunger,
                value: sim.animals.list[ai].hunger,
            })],
        );
        deaths.push((ti, DeathCause::Predation, vec![Cause::Event(kill_ev)]));
        let eaten = {
            let a = &mut sim.animals.list[ai];
            let asp = &ANIMALS[a.species as usize];
            // a predator gorges: even small prey is worth days (real canid ecology)
            let meal = (asp.mass * 0.35).min(t_mass * 0.7);
            a.hunger = (a.hunger - meal / (asp.mass * 0.18)).max(0.0);
            a.condition = (a.condition + 0.05).min(1.0);
            meal
        };
        sim.animals.corpses.push(Corpse {
            species: t_species,
            cell: t_cell,
            day: sim.clock.day,
            mass: t_mass - eaten,
            of_animal: Animals::id_of(ti),
            bait: false,
            staker: crate::core::ids::PersonId::NONE,
            rot: 0.0,
            gone: false,
        });
        sim.history.push(
            sim.clock.day,
            EntityRef::Animal(hunter_id),
            EventKind::Ate { what: EntityRef::Animal(Animals::id_of(ti)) },
            Some(t_cell),
            vec![Cause::Event(kill_ev)],
        );
    } else {
        // prey breaks away, shaken — and it will remember this place
        let t = &mut sim.animals.list[ti];
        t.fear = (t.fear + 0.8).min(2.0);
        t.danger_memory = Some((t.x, t.y));
        let a = &mut sim.animals.list[ai];
        a.fatigue = (a.fatigue + 0.3).min(1.5);
    }
}

fn scavenge(sim: &mut Sim, ai: usize, ci: usize) {
    if ci >= sim.animals.corpses.len() || sim.animals.corpses[ci].gone {
        return;
    }
    let (cx, cy) = {
        let c = &sim.animals.corpses[ci];
        sim.grid.xy(c.cell as usize)
    };
    let dist = {
        let a = &sim.animals.list[ai];
        (cx - a.x).abs().max((cy - a.y).abs())
    };
    if dist > 0 {
        let speed = ANIMALS[sim.animals.list[ai].species as usize].speed;
        move_animal_toward(sim, ai, (cx, cy), speed.min(3));
        return;
    }
    let bite = {
        let a = &sim.animals.list[ai];
        (ANIMALS[a.species as usize].mass * 0.2).max(1.0)
    };
    let c = &mut sim.animals.corpses[ci];
    let meal = bite.min(c.mass);
    c.mass -= meal;
    let a = &mut sim.animals.list[ai];
    let asp = &ANIMALS[a.species as usize];
    a.hunger = (a.hunger - meal / (asp.mass * 0.3)).max(0.0);
}

fn court(sim: &mut Sim, ai: usize, mi: usize) {
    if mi >= sim.animals.list.len() || !sim.animals.list[mi].alive {
        return;
    }
    let dist = {
        let a = &sim.animals.list[ai];
        let m = &sim.animals.list[mi];
        (m.x - a.x).abs().max((m.y - a.y).abs())
    };
    if dist > 1 {
        let (to, sp) = {
            let a = &sim.animals.list[ai];
            let m = &sim.animals.list[mi];
            ((m.x, m.y), ANIMALS[a.species as usize].speed.min(2))
        };
        move_animal_toward(sim, ai, to, sp);
        return;
    }
    // conception: the female carries; sire recorded (real parentage, Test genetics)
    let day = sim.clock.day;
    let (fi, mi2) = if sim.animals.list[ai].sex == 0 { (ai, mi) } else { (mi, ai) };
    // fertility, not certainty: only a mature, well-conditioned female conceives, and not always
    let (ok, p) = {
        let f = &sim.animals.list[fi];
        let sp = &ANIMALS[f.species as usize];
        let age_y = (day as i64 - f.born) as f32 / 360.0;
        let ready = f.sex == 0
            && f.pregnant_by.is_none()
            && age_y >= sp.maturity_y
            && f.hunger < 1.0
            && f.condition > 0.45;
        // small-fast breeders conceive readily; large slow ones seldom
        let base = if sp.gestation_d < 60 { 0.4 } else if sp.gestation_d < 200 { 0.22 } else { 0.12 };
        (ready, base * f.condition)
    };
    let conceives = ok && sim.animals.list[fi].rng.chance(p);
    if conceives {
        let sire = Animals::id_of(mi2);
        let gest = {
            let f = &mut sim.animals.list[fi];
            let g = ANIMALS[f.species as usize].gestation_d;
            // a touch of natural variation
            (g as f32 * f.rng.range_f(0.94, 1.06)) as u64
        };
        let f = &mut sim.animals.list[fi];
        f.pregnant_by = sire;
        f.due_day = day + gest;
        sim.history.push(
            day,
            EntityRef::Animal(Animals::id_of(fi)),
            EventKind::Mated { with: EntityRef::Animal(sire) },
            None,
            vec![],
        );
    }
}

/// A predator stalks a person. The approach is real; the struggle weighs the beast's body
/// against human courage, skill, and the crowd — and people can kill their attacker. Witnesses
/// carry the memory: the raw material of man-eater legends and culls.
fn hunt_human(sim: &mut Sim, ai: usize, hi: usize) {
    if hi >= sim.humans.list.len() || !sim.humans.list[hi].alive {
        return;
    }
    let (dist, to) = {
        let a = &sim.animals.list[ai];
        let h = &sim.humans.list[hi];
        ((h.x - a.x).abs().max((h.y - a.y).abs()), (h.x, h.y))
    };
    let speed = ANIMALS[sim.animals.list[ai].species as usize].speed;
    if dist > 1 {
        move_animal_toward(sim, ai, to, speed);
        let a = &mut sim.animals.list[ai];
        a.fatigue = (a.fatigue + 0.2).min(1.5);
    }
    let dist_now = {
        let a = &sim.animals.list[ai];
        let h = &sim.humans.list[hi];
        (h.x - a.x).abs().max((h.y - a.y).abs())
    };
    if dist_now > 1 {
        return;
    }
    // the crowd matters: nearby adults raise the defense
    let (hx, hy) = to;
    let mut crowd = 0;
    for o in sim.humans.list.iter() {
        if o.alive && (o.x - hx).abs().max((o.y - hy).abs()) <= 2 {
            crowd += 1;
        }
    }
    // a door to bar changes everything
    let behind_walls = sim.objects.buildings.iter().any(|b| {
        b.standing() && {
            let (bx, by) = sim.grid.xy(b.cell as usize);
            (bx - hx).abs().max((by - hy).abs()) <= 1
        }
    });
    let (p_kill, beast_id) = {
        let a = &sim.animals.list[ai];
        let h = &sim.humans.list[hi];
        let asp = &ANIMALS[a.species as usize];
        let att = asp.mass * a.condition * (0.5 + a.genes[2] * 0.5);
        let mut def = 30.0 * (0.5 + h.traits[1] * 0.6 + h.skills[crate::humans::SK_FIGHT] * 0.8)
            + crowd as f32 * 9.0;
        if behind_walls {
            def *= 2.5;
        }
        (((att / (att + def)) * 0.45).clamp(0.03, 0.6), Animals::id_of(ai))
    };
    let roll = sim.animals.list[ai].rng.chance(p_kill);
    let day = sim.clock.day;
    let cell = sim.grid.idx(hx, hy).map(|i| i as u32);
    if roll {
        let hunger_now = sim.animals.list[ai].hunger;
        let kill_ev = sim.history.push(
            day,
            EntityRef::Person(crate::humans::Humans::id_of(hi)),
            EventKind::Killed { by: EntityRef::Animal(beast_id) },
            cell,
            vec![Cause::State(StateRef { what: StateKind::Hunger, value: hunger_now })],
        );
        crate::humans::kill_human(sim, hi, DeathCause::Predation, vec![Cause::Event(kill_ev)]);
        {
            let a = &mut sim.animals.list[ai];
            let asp = &ANIMALS[a.species as usize];
            a.hunger = (a.hunger - 60.0 / (asp.mass * 0.18)).max(0.0);
            a.condition = (a.condition + 0.05).min(1.0);
            a.man_kills = a.man_kills.saturating_add(1);
        }
        // a beast that has taken more than one soul earns a name in frightened mouths
        let (mk, sp_i) = {
            let a = &sim.animals.list[ai];
            (a.man_kills, a.species)
        };
        let arena = ai as u32;
        if mk >= 2 && !sim.animals.names.contains_key(&arena) {
            let mut rng = crate::core::rng::Rng::entity(sim.cfg.seed, "beast_names", arena as u64);
            let nm = crate::humans::make_name(&mut rng, (sp_i % 3) as u8);
            sim.animals.names.insert(arena, nm);
            sim.history.push(
                day,
                EntityRef::Animal(beast_id),
                EventKind::PredatorNamed { predator: EntityRef::Animal(beast_id) },
                cell,
                vec![Cause::Event(kill_ev)],
            );
        }
        // witnesses will not forget the beast
        for oi in 0..sim.humans.list.len() {
            let near = {
                let o = &sim.humans.list[oi];
                o.alive && (o.x - hx).abs().max((o.y - hy).abs()) <= 6
            };
            if near {
                let o = &mut sim.humans.list[oi];
                o.fear = (o.fear + 1.2).min(2.0);
                crate::humans::remember(
                    o,
                    day,
                    crate::humans::MemoryKind::FledBeast { beast: beast_id },
                    1.6,
                );
            }
        }
    } else {
        // driven off — and a brave defender may wound or kill the beast
        {
            let a = &mut sim.animals.list[ai];
            a.fear = (a.fear + 0.8).min(2.0);
            a.fatigue = (a.fatigue + 0.4).min(1.5);
        }
        let fight = {
            let h = &mut sim.humans.list[hi];
            h.fear = (h.fear + 0.8).min(2.0);
            h.skills[crate::humans::SK_FIGHT] =
                (h.skills[crate::humans::SK_FIGHT] + 0.01).min(1.0);
            let pw = 0.15 + h.traits[1] * 0.3 + h.skills[crate::humans::SK_FIGHT] * 0.3;
            h.rng.chance(pw)
        };
        if fight {
            let kill_ev = sim.history.push(
                day,
                EntityRef::Animal(beast_id),
                EventKind::Killed { by: EntityRef::Person(crate::humans::Humans::id_of(hi)) },
                cell,
                vec![],
            );
            kill_animal(sim, ai, DeathCause::Battle, vec![Cause::Event(kill_ev)], true);
        }
    }
}

/// Kill an animal with a cause event; optionally leave a corpse.
pub fn kill_animal(
    sim: &mut Sim,
    ai: usize,
    cause: DeathCause,
    causes: Vec<Cause>,
    leave_corpse: bool,
) {
    let (cell, species, mass_left) = {
        let a = &mut sim.animals.list[ai];
        if !a.alive {
            return;
        }
        a.alive = false;
        let cell = sim.grid.idx(a.x, a.y).unwrap() as u32;
        let m = ANIMALS[a.species as usize].mass * (0.3 + a.condition * 0.5);
        (cell, a.species, m)
    };
    sim.history.push(
        sim.clock.day,
        EntityRef::Animal(Animals::id_of(ai)),
        EventKind::Died { cause },
        Some(cell),
        causes,
    );
    if leave_corpse && cause != DeathCause::Predation {
        sim.animals.corpses.push(Corpse {
            species,
            cell,
            day: sim.clock.day,
            mass: mass_left,
            of_animal: Animals::id_of(ai),
            bait: false,
            staker: crate::core::ids::PersonId::NONE,
            rot: 0.0,
            gone: false,
        });
    }
}

/// Derived populations per species (observation, never authority).
pub fn population_by_species(sim: &Sim) -> Vec<(String, usize)> {
    let mut counts = vec![0usize; ANIMALS.len()];
    for a in &sim.animals.list {
        if a.alive {
            counts[a.species as usize] += 1;
        }
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(i, c)| (ANIMALS[i].name.to_string(), c))
        .collect()
}
