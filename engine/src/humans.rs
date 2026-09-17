//! Individual humans (directive §4.4, Stage 4). Every person: body, needs, personality, skills,
//! individually-held knowledge, memories (uncapped, decaying), typed relationships formed only
//! through real encounters, possessions, home, goals with recorded rationale, and a biography
//! that IS their event history. The decision model extends the animal brain's shape: needs +
//! percepts + memory + relationships + personality → action + rationale.

use crate::core::ids::{AnimalId, BuildingId, EntityRef, EventId, PersonId};
use crate::core::jobs::par_map;
use crate::core::rng::Rng;
use crate::history::{BondContext, Cause, DeathCause, EventKind, StateKind, StateRef};
use crate::objects::{Building, BuildingKind, Objects};
use crate::sim::Sim;
use crate::species::{ANIMALS, PLANTS};
use crate::world::SpatialIndex;
use serde::{Deserialize, Serialize};

pub const N_SKILLS: usize = 8;
pub const SK_FORAGE: usize = 0;
pub const SK_HUNT: usize = 1;
pub const SK_FARM: usize = 2;
pub const SK_WOODCUT: usize = 3;
pub const SK_BUILD: usize = 4;
pub const SK_CRAFT: usize = 5;
pub const SK_SPEAK: usize = 6;
pub const SK_FIGHT: usize = 7;

/// Techniques: individually held knowledge (Test E). Never a global tech level.
pub const N_TECH: usize = 6;
pub const TECH_FARMING: u8 = 0;
pub const TECH_POTTERY: u8 = 1;
pub const TECH_SMELTING: u8 = 2;
pub const TECH_WRITING: u8 = 3;
pub const TECH_WEAVING: u8 = 4;
pub const TECH_MASONRY: u8 = 5;
pub const TECH_NAMES: [&str; N_TECH] =
    ["farming", "pottery", "smelting", "writing", "weaving", "masonry"];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MemoryKind {
    SawDie { who: EntityRef, cause: DeathCause },
    KinDied { who: PersonId },
    WentHungry,
    FledFire,
    FledBeast { beast: AnimalId },
    WasTaught { tech: u8, by: PersonId },
    Taught { tech: u8, to: PersonId },
    Married { to: PersonId },
    ChildBorn { child: PersonId },
    ToldTale { about: EventId, by: PersonId }, // inherited/retold memory, weight-distorted
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    pub day: u64,
    pub kind: MemoryKind,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelKind {
    Friend,
    Spouse,
    ParentOf,
    ChildOf,
    Rival,
    Teacher,
    Student,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rel {
    pub other: PersonId,
    pub kind: RelKind,
    pub strength: f32,
}

/// Recorded rationale for the day's chosen goal (the human `why`).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct HumanRationale {
    pub eat: f32,
    pub drink: f32,
    pub rest: f32,
    pub flee: f32,
    pub gather: f32,
    pub hunt: f32,
    pub build: f32,
    pub social: f32,
    pub farm: f32,
    pub chosen: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum HumanAction {
    EatStored,
    Drink { at: (i32, i32) },
    Rest,
    Flee { from: (i32, i32) },
    Gather,                    // forage edible plants here / move to better forage
    MoveTo { to: (i32, i32) }, // generic approach
    Hunt { target: u32 },
    ChopWood,                  // fell a tree here, carry logs
    Build { kind: BuildingKind },
    Deposit,                   // store carried food at home
    Socialize { with: u32 },   // talk: bonds, news, beliefs, teaching
    Court { with: u32 },
    TendFarm,                  // farming technique: plant/tend wheat on this cell
    Idle,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Human {
    pub name: String,
    pub culture: u8,
    pub sex: u8, // 0 f, 1 m
    pub born: i64,
    pub frailty_d: u64, // constitution drawn at birth; death near it, modulated by health
    pub x: i32,
    pub y: i32,
    // body
    pub health: f32,
    pub nutrition: f32, // long-term reserve 0..1
    pub hunger: f32,
    pub thirst: f32,
    pub fatigue: f32,
    // psyche
    pub traits: [f32; 4], // ambition, courage, warmth, curiosity
    pub piety: f32,
    pub fear: f32,
    pub grief: f32,
    pub memories: Vec<Memory>,
    pub beliefs: Vec<(u32, f32)>, // belief id, strength (clusters derived in Stage 5)
    // competence
    pub skills: [f32; N_SKILLS],
    pub techs: Vec<u8>, // techniques actually known
    // social
    pub rels: Vec<Rel>,
    pub mother: PersonId,
    pub father: PersonId,
    pub spouse: PersonId,
    pub pregnant_by: PersonId,
    pub due_day: u64,
    // economy
    pub carried_food: f32,
    pub carried_water: f32, // waterskin: carrying is the oldest human technology
    pub carried_wood: f32,
    pub home: BuildingId,
    // mind state
    pub rationale: HumanRationale,
    pub current: HumanAction,
    pub camp: (i32, i32), // band/home anchor: where this person considers "ours"
    pub water_memory: Option<(i32, i32)>,
    pub days_starving: u16,
    pub days_thirsty: u16,
    pub rng: Rng,
    pub alive: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Humans {
    pub list: Vec<Human>,
    #[serde(skip)]
    pub index: SpatialIndex,
}

impl Humans {
    pub fn id_of(idx: usize) -> PersonId {
        PersonId::from_index(idx)
    }
    pub fn get(&self, id: PersonId) -> Option<&Human> {
        self.list.get(id.index())
    }
    pub fn rel_strength(&self, a: usize, b: PersonId) -> f32 {
        self.list[a]
            .rels
            .iter()
            .filter(|r| r.other == b)
            .map(|r| r.strength)
            .fold(0.0, f32::max)
    }
    pub fn rebuild_index(&mut self, w: u32, h: u32) {
        if self.index.is_empty_dims() {
            self.index = SpatialIndex::new(w, h);
        }
        self.index.clear();
        for (i, h_) in self.list.iter().enumerate() {
            if h_.alive {
                self.index.insert(i as u32, h_.x, h_.y);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        master: u64,
        name: String,
        culture: u8,
        x: i32,
        y: i32,
        born: i64,
        mother: PersonId,
        father: PersonId,
    ) -> usize {
        let idx = self.list.len();
        let mut rng = Rng::entity(master, "humans", idx as u64);
        let frailty_d = (360.0 * rng.range_f(52.0, 88.0)) as u64;
        let traits = [
            rng.f32(),
            rng.f32(),
            rng.f32(),
            rng.f32(),
        ];
        let sex = if rng.chance(0.5) { 0 } else { 1 };
        self.list.push(Human {
            name,
            culture,
            sex,
            born,
            frailty_d,
            x,
            y,
            health: 1.0,
            nutrition: 0.7,
            hunger: 0.3,
            thirst: 0.3,
            fatigue: 0.0,
            traits,
            piety: rng.f32(),
            fear: 0.0,
            grief: 0.0,
            memories: Vec::new(),
            beliefs: Vec::new(),
            skills: [0.05; N_SKILLS],
            techs: Vec::new(),
            rels: Vec::new(),
            mother,
            father,
            spouse: PersonId::NONE,
            pregnant_by: PersonId::NONE,
            due_day: 0,
            carried_food: 0.5,
            carried_water: 4.0,
            carried_wood: 0.0,
            home: BuildingId::NONE,
            rationale: HumanRationale::default(),
            current: HumanAction::Idle,
            camp: (x, y),
            water_memory: None,
            days_starving: 0,
            days_thirsty: 0,
            rng,
            alive: true,
        });
        idx
    }
}

/// Simple deterministic name generator (culture-flavored syllables).
pub fn make_name(rng: &mut Rng, culture: u8) -> String {
    const ON: [&[&str]; 3] = [
        &["ka", "ta", "ma", "ra", "sa", "na", "ha"],
        &["bel", "dor", "gan", "mir", "tal", "ver"],
        &["ash", "el", "ir", "or", "un", "yl"],
    ];
    let bank = ON[(culture as usize) % ON.len()];
    let n = 2 + (rng.next_u64() % 2) as usize;
    let mut s = String::new();
    for _ in 0..n {
        s.push_str(bank[(rng.next_u64() % bank.len() as u64) as usize]);
    }
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// Seed founding bands near fresh water with decent forage — people, not settlements
/// (settlements are emergent clusters, Stage 5).
pub fn generate(sim: &mut Sim) {
    let mut rng = Rng::domain(sim.cfg.seed, "humans_gen");
    let mut band_sites: Vec<usize> = Vec::new();
    let n = sim.grid.n();
    let mut tries = 0;
    while band_sites.len() < 5 && tries < 8000 {
        tries += 1;
        let i = (rng.next_u64() % n as u64) as usize;
        if sim.grid.ocean[i] {
            continue;
        }
        let (x, y) = sim.grid.xy(i);
        // real site quality: fresh water within 4, forage within 2
        let mut water = false;
        'w: for dy in -4i32..=4 {
            for dx in -4i32..=4 {
                if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                    if sim.grid.is_river(j) || sim.grid.is_lake(j) {
                        water = true;
                        break 'w;
                    }
                }
            }
        }
        if !water || sim.grid.veg_cover[i] < 0.2 {
            continue;
        }
        if band_sites.iter().any(|&s| {
            let (sx, sy) = sim.grid.xy(s);
            (sx - x).abs().max((sy - y).abs()) < 24
        }) {
            continue;
        }
        band_sites.push(i);
    }
    for (bi, &site) in band_sites.iter().enumerate() {
        let (x, y) = sim.grid.xy(site);
        // the band's water source: nearest fresh cell to the site (they settled here for it)
        let mut band_water = None;
        'bw: for r in 0..=6i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                        if sim.grid.is_river(j) || sim.grid.is_lake(j) {
                            band_water = Some((x + dx, y + dy));
                            break 'bw;
                        }
                    }
                }
            }
        }
        let culture = bi as u8;
        let band_n = 9 + (rng.next_u64() % 4) as i64;
        let first_idx = sim.humans.list.len();
        for _ in 0..band_n {
            let back = rng.range_i(16 * 360, 40 * 360);
            let name = make_name(&mut rng, culture);
            let dx = rng.range_i(-2, 2) as i32;
            let dy = rng.range_i(-2, 2) as i32;
            let ni = sim.humans.spawn(
                sim.cfg.seed,
                name,
                culture,
                x + dx,
                y + dy,
                -back,
                PersonId::NONE,
                PersonId::NONE,
            );
            sim.humans.list[ni].water_memory = band_water;
            // the founding generation carries its own knowledge: a couple of farmers per band
            // (and one potter), from which techniques spread — or die — by real teaching
            let k = ni - first_idx;
            if k < 2 {
                sim.humans.list[ni].techs.push(TECH_FARMING);
            } else if k == 2 {
                sim.humans.list[ni].techs.push(TECH_POTTERY);
            }
        }
        // a band has lived together: its members start with real (pre-history) bonds
        let last_idx = sim.humans.list.len();
        for a in first_idx..last_idx {
            for b in (a + 1)..last_idx {
                let bid = Humans::id_of(b);
                let aid = Humans::id_of(a);
                sim.humans.list[a].rels.push(Rel { other: bid, kind: RelKind::Friend, strength: 0.4 });
                sim.humans.list[b].rels.push(Rel { other: aid, kind: RelKind::Friend, strength: 0.4 });
            }
        }
    }
}

pub struct HumanPercepts {
    pub threat: Option<(u32, i32, i32)>, // dangerous animal idx + pos
    pub game: Option<u32>,               // huntable animal
    pub forage_here: f32,
    pub forage_near: Option<(i32, i32)>,
    pub forage_near_val: f32,
    pub water_near: Option<(i32, i32)>,
    pub people_near: Vec<u32>, // deterministic order
    pub tree_here: bool,
    pub fire_near: bool,
    pub granary_near: Option<(u32, f32)>, // building idx with food, its stock
}

fn perceive(sim: &Sim, hi: usize) -> HumanPercepts {
    let h = &sim.humans.list[hi];
    let grid = &sim.grid;
    let mut p = HumanPercepts {
        threat: None,
        game: None,
        forage_here: 0.0,
        forage_near: None,
        forage_near_val: 0.0,
        water_near: None,
        people_near: Vec::new(),
        tree_here: false,
        fire_near: false,
        granary_near: None,
    };
    const R: i32 = 8;
    // animals: threats and game
    let mut best_threat = i32::MAX;
    let mut best_game = i32::MAX;
    sim.animals.index.for_each_near(h.x, h.y, R, |ai| {
        let a = &sim.animals.list[ai as usize];
        if !a.alive {
            return;
        }
        let d = (a.x - h.x).abs().max((a.y - h.y).abs());
        if d > R {
            return;
        }
        let sp = &ANIMALS[a.species as usize];
        let dangerous = !sp.prey.is_empty() && sp.mass > 30.0; // carnivores/omnivores of size
        if dangerous && d < best_threat {
            best_threat = d;
            p.threat = Some((ai, a.x, a.y));
        }
        if !dangerous && d < best_game {
            best_game = d;
            p.game = Some(ai);
        }
    });
    // people
    sim.humans.index.for_each_near(h.x, h.y, R, |oi| {
        if oi as usize != hi && sim.humans.list[oi as usize].alive {
            let o = &sim.humans.list[oi as usize];
            if (o.x - h.x).abs().max((o.y - h.y).abs()) <= R {
                p.people_near.push(oi);
            }
        }
    });
    // forage + trees here
    if let Some(i) = grid.idx(h.x, h.y) {
        if let Some(pids) = sim.plants.by_cell.get(i) {
            for &pi in pids {
                let pl = &sim.plants.list[pi as usize];
                if !pl.alive {
                    continue;
                }
                let sp = &PLANTS[pl.species as usize];
                let ed = if sp.kind == crate::species::PlantKind::Tree {
                    0.08
                } else {
                    sp.edible
                };
                p.forage_here += pl.biomass * ed;
                if sp.kind == crate::species::PlantKind::Tree && pl.biomass > 1.0 {
                    p.tree_here = true;
                }
            }
        }
        p.fire_near = grid.burning[i] > 0;
    }
    // water: nearest fresh cell by expanding rings (radius 8)
    'wring: for r in 0..=8i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if let Some(j) = grid.idx(h.x + dx, h.y + dy) {
                    if grid.is_fresh_water(j) {
                        p.water_near = Some((h.x + dx, h.y + dy));
                        break 'wring;
                    }
                }
            }
        }
    }
    // nearby better forage + fire
    let mut best_val = 0.0;
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            if let Some(j) = grid.idx(h.x + dx, h.y + dy) {
                if grid.burning[j] > 0 && dx.abs() <= 2 && dy.abs() <= 2 {
                    p.fire_near = true;
                }
                if let Some(pids) = sim.plants.by_cell.get(j) {
                    let mut v = 0.0;
                    for &pi in pids {
                        let pl = &sim.plants.list[pi as usize];
                        if pl.alive {
                            let sp = &PLANTS[pl.species as usize];
                            let ed = if sp.kind == crate::species::PlantKind::Tree {
                                0.08
                            } else {
                                sp.edible
                            };
                            v += pl.biomass * ed;
                        }
                    }
                    if v > best_val + 0.05 {
                        best_val = v;
                        p.forage_near = Some((h.x + dx, h.y + dy));
                        p.forage_near_val = v;
                    }
                }
            }
        }
    }
    if p.water_near.is_none() {
        p.water_near = h.water_memory;
    }
    // the band's stores: nearest hut with food within reach (forager sharing commons —
    // formal ownership/households arrive with Stage 5 institutions)
    let mut best_d = i32::MAX;
    for (bi, b) in sim.objects.buildings.iter().enumerate() {
        if !b.exists || b.food_store < 0.3 {
            continue;
        }
        let (bx, by) = grid.xy(b.cell as usize);
        let d = (bx - h.x).abs().max((by - h.y).abs());
        if d <= 10 && d < best_d {
            best_d = d;
            p.granary_near = Some((bi as u32, b.food_store));
        }
    }
    p
}

/// The human decision model: same shape as the animal brain, plus stores, homes, social goals.
fn decide(sim: &Sim, hi: usize, p: &HumanPercepts) -> (HumanAction, HumanRationale) {
    let h = &sim.humans.list[hi];
    let mut r = HumanRationale::default();
    let day = sim.clock.day;
    let age_y = (day as i64 - h.born) as f32 / 360.0;
    let adult = age_y >= 14.0;

    let home_store = h.home.some().map(|b| sim.objects.buildings[b.index()].food_store).unwrap_or(0.0)
        + p.granary_near.map(|(_, s2)| s2).unwrap_or(0.0);

    if let Some((_, tx, ty)) = p.threat {
        let d = (tx - h.x).abs().max((ty - h.y).abs());
        r.flee = ((8 - d).max(0) as f32 / 8.0) * (2.5 - h.traits[1] * 1.2) + h.fear;
    }
    if p.fire_near {
        r.flee = r.flee.max(3.0);
    }
    r.drink = h.thirst * h.thirst * 1.9;
    // eat: from carried/stored food, else gather
    let food_avail = h.carried_food + home_store;
    r.eat = if food_avail > 0.1 { h.hunger * 1.4 } else { 0.0 };
    // gathering is only worth what the land actually offers where you can see
    let forage_seen = (p.forage_here + p.forage_near_val * 0.6).min(2.0);
    let starving = h.days_starving >= 4;
    r.gather = h.hunger.max(0.35) * (0.1 + forage_seen)
        * if food_avail < 1.5 { 1.2 } else { 0.35 }
        * if starving { 1.5 } else { 1.0 };
    if adult && p.game.is_some() {
        r.hunt = h.hunger.max(0.3) * (0.5 + h.skills[SK_HUNT] * 1.2) * (0.6 + h.traits[1] * 0.6)
            * if food_avail < 1.5 { 1.0 } else { 0.3 }
            * if h.hunger > 1.0 { 1.6 } else { 1.0 };
    }
    // shelter: homeless adults want a hut; homeowners keep stores stocked
    if adult && h.home.is_none() && food_avail > 1.0 && h.hunger < 0.8 && h.nutrition > 0.45 {
        r.build = 0.7 + h.traits[0] * 0.4;
    }
    // farming: knowers of the technique tend plots when hungry season looms
    if adult && h.techs.contains(&TECH_FARMING) {
        r.farm = 0.8 + h.hunger * 0.5 + if food_avail < 2.0 { 0.4 } else { 0.0 };
    }
    // social pull: bonds, courtship, teaching — stronger for warm personalities
    if !p.people_near.is_empty() {
        r.social = 0.25 + h.traits[2] * 0.5 + (h.grief * 0.3);
        if adult && h.spouse.is_none() {
            r.social += 0.35;
        }
        if h.hunger > 1.0 {
            r.social *= 0.3; // an empty stomach ends the conversation
        }
    }
    r.rest = h.fatigue * 1.3;

    let table = [
        (r.flee, 0u8),
        (r.drink, 1),
        (r.eat, 2),
        (r.gather, 3),
        (r.hunt, 4),
        (r.build, 5),
        (r.social, 6),
        (r.farm, 7),
        (r.rest, 8),
    ];
    let mut best = (0.2f32, 9u8); // idle threshold
    for e in table.iter() {
        if e.0 > best.0 {
            best = *e;
        }
    }
    r.chosen = best.1;

    // chore override: a full pack goes to the granary before anything optional
    if h.carried_food > 1.5 && !h.home.is_none() && r.flee < 0.5 && h.hunger < 0.9 {
        return (HumanAction::Deposit, r);
    }
    let act = match best.1 {
        0 => {
            let from = p.threat.map(|(_, x, y)| (x, y)).unwrap_or((h.x, h.y));
            HumanAction::Flee { from }
        }
        1 => HumanAction::Drink { at: p.water_near.unwrap_or((h.x, h.y)) },
        2 => HumanAction::EatStored,
        3 => {
            if starving && forage_seen < 0.15 {
                // the land here is spent: explore — but people follow water, never leave it
                let hd = crate::core::rng::splitmix64(day ^ (h.born as u64).wrapping_mul(31));
                let dirs: [(i32, i32); 8] =
                    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
                let d = dirs[(hd % 8) as usize];
                let (ax, ay) = p.water_near.unwrap_or((h.x, h.y));
                HumanAction::MoveTo { to: (ax + d.0 * 5, ay + d.1 * 5) }
            } else if p.forage_here > 0.05 && p.forage_here * 1.3 >= p.forage_near_val {
                HumanAction::Gather
            } else if let Some(to) = p.forage_near {
                HumanAction::MoveTo { to }
            } else {
                HumanAction::Gather
            }
        }
        4 => HumanAction::Hunt { target: p.game.unwrap() },
        5 => {
            // building needs wood: chop if carrying too little and a tree is here
            if h.carried_wood >= 8.0 {
                HumanAction::Build { kind: BuildingKind::Hut }
            } else if p.tree_here {
                HumanAction::ChopWood
            } else {
                // find a tree: move toward best forage as proxy for woods
                match p.forage_near {
                    Some(to) => HumanAction::MoveTo { to },
                    None => HumanAction::Idle,
                }
            }
        }
        6 => {
            // pick the most-bonded nearby person (deterministic: first max)
            let mut best_o = None;
            let mut best_s = -1.0f32;
            for &oi in &p.people_near {
                let s = sim.humans.rel_strength(hi, Humans::id_of(oi as usize));
                if s > best_s {
                    best_s = s;
                    best_o = Some(oi);
                }
            }
            match best_o {
                Some(oi) => {
                    let o = &sim.humans.list[oi as usize];
                    let o_age = (day as i64 - o.born) as f32 / 360.0;
                    if adult
                        && h.spouse.is_none()
                        && o.spouse.is_none()
                        && o.sex != h.sex
                        && o_age >= 14.0
                    {
                        HumanAction::Court { with: oi }
                    } else {
                        HumanAction::Socialize { with: oi }
                    }
                }
                None => HumanAction::Idle,
            }
        }
        7 => HumanAction::TendFarm,
        8 => HumanAction::Rest,
        _ => {
            // idle: drift home or deposit surplus
            if h.carried_food > 1.2 && !h.home.is_none() {
                HumanAction::Deposit
            } else {
                HumanAction::Idle
            }
        }
    };
    (act, r)
}

pub fn remember(h: &mut Human, day: u64, kind: MemoryKind, weight: f32) {
    h.memories.push(Memory { day, kind, weight });
}

fn add_rel(sim: &mut Sim, a: usize, b: usize, kind: RelKind, ds: f32) {
    let bid = Humans::id_of(b);
    let h = &mut sim.humans.list[a];
    if let Some(r) = h.rels.iter_mut().find(|r| r.other == bid && r.kind == kind) {
        r.strength = (r.strength + ds).min(3.0);
    } else {
        h.rels.push(Rel { other: bid, kind, strength: ds });
    }
}

fn move_human(sim: &mut Sim, hi: usize, dir: (i32, i32), steps: i32) {
    for _ in 0..steps {
        let (nx, ny) = {
            let h = &sim.humans.list[hi];
            (h.x + dir.0, h.y + dir.1)
        };
        match sim.grid.idx(nx, ny) {
            Some(j) if !sim.grid.ocean[j] => {
                let h = &mut sim.humans.list[hi];
                h.x = nx;
                h.y = ny;
            }
            _ => break,
        }
    }
}

/// Step toward a destination, stopping ON it (never overshooting past it).
fn move_toward_h(sim: &mut Sim, hi: usize, to: (i32, i32), steps: i32) {
    for _ in 0..steps {
        let (dir, arrived) = {
            let h = &sim.humans.list[hi];
            (((to.0 - h.x).signum(), (to.1 - h.y).signum()), h.x == to.0 && h.y == to.1)
        };
        if arrived {
            break;
        }
        let (nx, ny) = {
            let h = &sim.humans.list[hi];
            (h.x + dir.0, h.y + dir.1)
        };
        match sim.grid.idx(nx, ny) {
            Some(j) if !sim.grid.ocean[j] => {
                let h = &mut sim.humans.list[hi];
                h.x = nx;
                h.y = ny;
            }
            _ => break,
        }
    }
}

pub fn tick(sim: &mut Sim) {
    let day = sim.clock.day;
    sim.humans.rebuild_index(sim.grid.w, sim.grid.h);

    // ---------- perceive + decide (parallel, read-only) ----------
    let n = sim.humans.list.len();
    let sim_ref: &Sim = sim;
    let plans: Vec<Option<(HumanAction, HumanRationale, Option<(i32, i32)>)>> =
        par_map(n, |hi| {
            let h = &sim_ref.humans.list[hi];
            if !h.alive {
                return None;
            }
            let p = perceive(sim_ref, hi);
            let water_seen = p.water_near;
            let (act, rat) = decide(sim_ref, hi, &p);
            Some((act, rat, water_seen))
        });

    // ---------- act (ordered) ----------
    let mut deaths: Vec<(usize, DeathCause, Vec<Cause>)> = Vec::new();
    for hi in 0..n {
        let Some((act, rat, water_seen)) = plans[hi].clone() else { continue };
        if !sim.humans.list[hi].alive {
            continue;
        }
        {
            let h = &mut sim.humans.list[hi];
            h.rationale = rat;
            h.current = act;
            if let Some(w) = water_seen {
                h.water_memory = Some(w);
            }
            if h.days_starving >= 3 && h.home.is_none() {
                // hungry bands move camp with their feet
                h.camp = (h.x, h.y);
            }
        }
        apply_action(sim, hi, act, &mut deaths);
    }

    // ---------- physiology (ordered) ----------
    let mut births: Vec<(usize, PersonId)> = Vec::new();
    for hi in 0..n {
        if !sim.humans.list[hi].alive {
            continue;
        }
        let (starved, parched, aged, birth) = {
            let h = &mut sim.humans.list[hi];
            let age_y = (day as i64 - h.born) as f32 / 360.0;
            let active = 0.8 + 0.4 * (age_y / 60.0).min(1.0);
            h.hunger = (h.hunger + 0.07 * active).min(2.0);
            h.thirst = (h.thirst + 0.14).min(2.0);
            if h.thirst > 0.3 && h.carried_water > 0.4 {
                h.carried_water -= 0.5;
                h.thirst = 0.0;
                h.days_thirsty = 0;
            }
            h.fatigue = (h.fatigue - 0.15).max(0.0);
            h.fear *= 0.85;
            h.grief *= 0.995;
            if h.hunger >= 1.0 {
                h.days_starving += 1;
                h.nutrition -= 0.015;
                if h.days_starving == 10 {
                    // hunger becomes a memory that shapes later decisions
                    remember(h, day, MemoryKind::WentHungry, 0.8);
                }
            } else {
                if h.days_starving > 0 {
                    h.days_starving -= 1;
                }
                if h.hunger < 0.6 {
                    h.nutrition = (h.nutrition + 0.008).min(1.0);
                }
            }
            if h.thirst >= 1.0 {
                h.days_thirsty += 1;
            } else {
                h.days_thirsty = 0;
            }
            h.health = (h.health
                + if h.nutrition > 0.4 { 0.005 } else { -0.01 })
            .clamp(0.0, 1.0);
            let age_d = (day as i64 - h.born) as u64;
            // frailty: death near constitution limit, hastened by poor health
            let aged = age_d > h.frailty_d + ((h.health - 0.5) * 3600.0) as i64 as u64;
            let starved = h.nutrition <= 0.02 || h.days_starving as f32 > 40.0;
            let parched = h.days_thirsty > 8;
            let birth = !h.pregnant_by.is_none() && day >= h.due_day;
            (starved, parched, aged, birth)
        };
        if aged {
            deaths.push((hi, DeathCause::Age, vec![]));
            continue;
        }
        if starved {
            let v = sim.humans.list[hi].nutrition;
            deaths.push((
                hi,
                DeathCause::Starvation,
                vec![Cause::State(StateRef { what: StateKind::Hunger, value: v })],
            ));
            continue;
        }
        if parched {
            deaths.push((
                hi,
                DeathCause::Thirst,
                vec![Cause::State(StateRef { what: StateKind::Thirst, value: 1.0 })],
            ));
            continue;
        }
        if birth {
            let father = sim.humans.list[hi].pregnant_by;
            births.push((hi, father));
        }
    }

    for (mi, father) in births {
        let (x, y, culture, mname) = {
            let m = &mut sim.humans.list[mi];
            m.pregnant_by = PersonId::NONE;
            (m.x, m.y, m.culture, m.name.clone())
        };
        let name = {
            let m = &mut sim.humans.list[mi];
            make_name(&mut m.rng, culture)
        };
        let ci = sim.humans.spawn(
            sim.cfg.seed,
            name,
            culture,
            x,
            y,
            day as i64,
            Humans::id_of(mi),
            father,
        );
        let cid = Humans::id_of(ci);
        add_rel(sim, mi, ci, RelKind::ParentOf, 2.0);
        add_rel(sim, ci, mi, RelKind::ChildOf, 2.0);
        if !father.is_none() {
            add_rel(sim, father.index(), ci, RelKind::ParentOf, 2.0);
            add_rel(sim, ci, father.index(), RelKind::ChildOf, 2.0);
        }
        {
            let m = &mut sim.humans.list[mi];
            remember(m, day, MemoryKind::ChildBorn { child: cid }, 1.5);
        }
        sim.history.push(
            day,
            EntityRef::Person(cid),
            EventKind::Born {
                mother: EntityRef::Person(Humans::id_of(mi)),
                father: EntityRef::Person(father),
            },
            sim.grid.idx(x, y).map(|i| i as u32),
            vec![],
        );
        let _ = mname;
    }

    // family life: spouses who are actually together may conceive
    for hi in 0..n {
        let (spouse, eligible) = {
            let h = &sim.humans.list[hi];
            if !h.alive || h.sex != 0 || !h.pregnant_by.is_none() {
                (PersonId::NONE, false)
            } else {
                let age_y = (day as i64 - h.born) as f32 / 360.0;
                (h.spouse, (14.0..46.0).contains(&age_y) && h.nutrition > 0.3)
            }
        };
        if !eligible {
            continue;
        }
        let Some(sp) = spouse.some() else { continue };
        let si = sp.index();
        if si >= sim.humans.list.len() || !sim.humans.list[si].alive {
            continue;
        }
        let together = {
            let h = &sim.humans.list[hi];
            let o = &sim.humans.list[si];
            (o.x - h.x).abs().max((o.y - h.y).abs()) <= 1
        };
        if together {
            let conceive = {
                let h = &mut sim.humans.list[hi];
                h.rng.chance(0.05)
            };
            if conceive {
                let h = &mut sim.humans.list[hi];
                h.pregnant_by = sp;
                h.due_day = day + 270;
            }
        }
    }

    for (hi, cause, causes) in deaths {
        kill_human(sim, hi, cause, causes);
    }

    // memory decay: yearly pass, weakest fade but nothing is deleted from history
    if day % 360 == 200 {
        for h in sim.humans.list.iter_mut() {
            if !h.alive {
                continue;
            }
            for m in h.memories.iter_mut() {
                m.weight *= 0.96;
            }
        }
    }
}

fn apply_action(
    sim: &mut Sim,
    hi: usize,
    act: HumanAction,
    deaths: &mut Vec<(usize, DeathCause, Vec<Cause>)>,
) {
    let day = sim.clock.day;
    match act {
        HumanAction::Flee { from } => {
            let dir = {
                let h = &sim.humans.list[hi];
                ((h.x - from.0).signum(), (h.y - from.1).signum())
            };
            move_human(sim, hi, dir, 3);
            let h = &mut sim.humans.list[hi];
            h.fear = (h.fear + 0.4).min(2.0);
            h.fatigue = (h.fatigue + 0.3).min(1.5);
        }
        HumanAction::Drink { at } => {
            // no known water? head downhill — water lives in valleys
            let no_target = {
                let h = &sim.humans.list[hi];
                at == (h.x, h.y)
            };
            if no_target {
                let (hx, hy) = {
                    let h = &sim.humans.list[hi];
                    (h.x, h.y)
                };
                let mut best = (hx, hy);
                let mut best_e = f32::MAX;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if let Some(j) = sim.grid.idx(hx + dx * 3, hy + dy * 3) {
                            if !sim.grid.ocean[j] && sim.grid.elev[j] < best_e {
                                best_e = sim.grid.elev[j];
                                best = (hx + dx * 3, hy + dy * 3);
                            }
                        }
                    }
                }
                move_toward_h(sim, hi, best, 2);
                let now_wet = {
                    let h = &sim.humans.list[hi];
                    sim.grid.idx(h.x, h.y).map(|i| sim.grid.is_fresh_water(i)).unwrap_or(false)
                };
                if now_wet {
                    let h = &mut sim.humans.list[hi];
                    h.thirst = 0.0;
                    h.days_thirsty = 0;
                    h.carried_water = 6.0;
                }
                return;
            }
            let on_water = {
                let h = &sim.humans.list[hi];
                sim.grid
                    .idx(h.x, h.y)
                    .map(|i| sim.grid.is_fresh_water(i))
                    .unwrap_or(false)
            };
            if on_water {
                let h = &mut sim.humans.list[hi];
                h.thirst = 0.0;
                h.days_thirsty = 0;
                h.carried_water = 6.0;
            } else {
                move_toward_h(sim, hi, at, 2);
                let h = &sim.humans.list[hi];
                let (arrived_dry, hx, hy) = {
                    let h = &sim.humans.list[hi];
                    let wet = sim
                        .grid
                        .idx(h.x, h.y)
                        .map(|i| sim.grid.is_fresh_water(i))
                        .unwrap_or(false);
                    ((h.x, h.y) == at && !wet, h.x, h.y)
                };
                let _ = (hx, hy);
                if arrived_dry {
                    // the remembered water is gone: forget it and search fresh tomorrow
                    let h = &mut sim.humans.list[hi];
                    if h.water_memory == Some(at) {
                        h.water_memory = None;
                    }
                } else if sim
                    .grid
                    .idx(sim.humans.list[hi].x, sim.humans.list[hi].y)
                    .map(|i| sim.grid.is_fresh_water(i))
                    .unwrap_or(false)
                {
                    let h = &mut sim.humans.list[hi];
                    h.thirst = 0.0;
                    h.days_thirsty = 0;
                    h.carried_water = 6.0;
                }
            }
        }
        HumanAction::EatStored => {
            let need = {
                let h = &sim.humans.list[hi];
                h.hunger * 0.9
            };
            let mut ate = 0.0;
            {
                let h = &mut sim.humans.list[hi];
                let from_hand = h.carried_food.min(need);
                h.carried_food -= from_hand;
                ate += from_hand;
            }
            if ate < need {
                let home = sim.humans.list[hi].home;
                if let Some(b) = home.some() {
                    let bld = &mut sim.objects.buildings[b.index()];
                    let take = bld.food_store.min(need - ate);
                    bld.food_store -= take;
                    ate += take;
                }
            }
            if ate < need * 0.5 {
                // the band shares: walk to the nearest stocked hut and eat from its store
                let target = {
                    let h = &sim.humans.list[hi];
                    let mut best: Option<(usize, i32)> = None;
                    for (bi, b) in sim.objects.buildings.iter().enumerate() {
                        if !b.exists || b.food_store < 0.3 {
                            continue;
                        }
                        let (bx, by) = sim.grid.xy(b.cell as usize);
                        let d = (bx - h.x).abs().max((by - h.y).abs());
                        if d <= 10 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                            best = Some((bi, d));
                        }
                    }
                    best
                };
                if let Some((bi, d)) = target {
                    if d <= 1 {
                        let bld = &mut sim.objects.buildings[bi];
                        let take = bld.food_store.min(need - ate);
                        bld.food_store -= take;
                        ate += take;
                    } else {
                        let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
                        move_toward_h(sim, hi, (bx, by), 2);
                    }
                }
            }
            let h = &mut sim.humans.list[hi];
            h.hunger = (h.hunger - ate).max(0.0);
        }
        HumanAction::Gather => {
            let cell = {
                let h = &sim.humans.list[hi];
                sim.grid.idx(h.x, h.y)
            };
            let Some(cell) = cell else { return };
            let skill = sim.humans.list[hi].skills[SK_FORAGE];
            let want = 2.0 + skill * 1.0; // food-value target for a day's gathering (real surplus)
            let Some(pids) = sim.plants.by_cell.get(cell).cloned() else { return };
            let mut got = 0.0f32;
            let mut kills: Vec<usize> = Vec::new();
            let mut need = want;
            let autumn = matches!(sim.clock.season(), crate::core::clock::Season::Autumn);
            for &pi in pids.iter() {
                if need <= 0.0 {
                    break;
                }
                let pl = &mut sim.plants.list[pi as usize];
                if !pl.alive {
                    continue;
                }
                let spp = &PLANTS[pl.species as usize];
                // humans forage everything: greens and seeds from herbs, mast (acorns, nuts,
                // bark foods) from trees — trees yield less per biomass but hold far more
                let ed = if spp.kind == crate::species::PlantKind::Tree {
                    if autumn { 0.30 } else { 0.08 }
                } else {
                    spp.edible
                };
                if ed < 0.05 {
                    continue;
                }
                let frac = if spp.kind == crate::species::PlantKind::Tree { 0.15 } else { 0.7 };
                // gather until the day's food-value target is met (need is food, not biomass)
                let take = (need / ed).min(pl.biomass * frac);
                pl.biomass -= take;
                got += take * ed;
                need -= take * ed;
                if pl.biomass < 0.01 {
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
            // roots, tubers, bark-foods: dug from the real litter/detritus layer — the
            // winter staple. Consumes actual cell organic matter.
            if need > 0.0 {
                let lit = sim.grid.litter[cell];
                let dig = (need / 0.25).min(lit * 0.4);
                if dig > 0.0 {
                    sim.grid.litter[cell] -= dig;
                    got += dig * 0.25;
                }
            }
            let h = &mut sim.humans.list[hi];
            h.carried_food += got;
            h.skills[SK_FORAGE] = (h.skills[SK_FORAGE] + 0.002).min(1.0);
            h.fatigue = (h.fatigue + 0.15).min(1.5);
            // eat from hand immediately if hungry
            if h.hunger > 0.5 {
                let bite = h.carried_food.min(h.hunger * 0.8);
                h.carried_food -= bite;
                h.hunger = (h.hunger - bite).max(0.0);
            }
        }
        HumanAction::MoveTo { to } => {
            move_toward_h(sim, hi, to, 2);
            // arrived somewhere green? gather before the day ends
            let here = {
                let h = &sim.humans.list[hi];
                sim.grid.idx(h.x, h.y)
            };
            if let Some(cell) = here {
                let forage: f32 = sim
                    .plants
                    .by_cell
                    .get(cell)
                    .map(|v| {
                        v.iter()
                            .map(|&pi| {
                                let pl = &sim.plants.list[pi as usize];
                                if pl.alive {
                                    pl.biomass * PLANTS[pl.species as usize].edible
                                } else {
                                    0.0
                                }
                            })
                            .sum()
                    })
                    .unwrap_or(0.0);
                if forage > 0.05 && sim.humans.list[hi].hunger > 0.4 {
                    apply_action(sim, hi, HumanAction::Gather, &mut Vec::new());
                }
            }
        }
        HumanAction::Hunt { target } => {
            hunt_animal(sim, hi, target as usize);
        }
        HumanAction::ChopWood => {
            let cell = {
                let h = &sim.humans.list[hi];
                sim.grid.idx(h.x, h.y)
            };
            let Some(cell) = cell else { return };
            let Some(pids) = sim.plants.by_cell.get(cell).cloned() else { return };
            // fell the biggest tree here
            let mut best: Option<(usize, f32)> = None;
            for &pi in pids.iter() {
                let pl = &sim.plants.list[pi as usize];
                if pl.alive
                    && PLANTS[pl.species as usize].kind == crate::species::PlantKind::Tree
                    && pl.biomass > 1.0
                {
                    if best.map(|(_, b)| pl.biomass > b).unwrap_or(true) {
                        best = Some((pi as usize, pl.biomass));
                    }
                }
            }
            if let Some((pi, biomass)) = best {
                let fell_ev = sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(hi)),
                    EventKind::FelledTree {
                        plant: EntityRef::Plant(crate::vegetation::Plants::id_of(pi)),
                    },
                    Some(cell as u32),
                    vec![],
                );
                crate::vegetation::kill_plant(
                    sim,
                    pi,
                    DeathCause::Felled,
                    vec![Cause::Event(fell_ev)],
                );
                let h = &mut sim.humans.list[hi];
                h.carried_wood += biomass.min(6.0);
                h.skills[SK_WOODCUT] = (h.skills[SK_WOODCUT] + 0.003).min(1.0);
                h.fatigue = (h.fatigue + 0.3).min(1.5);
            }
        }
        HumanAction::Build { kind } => {
            let (cell, ok) = {
                let h = &sim.humans.list[hi];
                let cell = sim.grid.idx(h.x, h.y);
                (cell, h.carried_wood >= 8.0)
            };
            let Some(cell) = cell else { return };
            if !ok || sim.objects.building_at(cell as u32).is_some() {
                return;
            }
            let bidx = sim.objects.buildings.len();
            sim.objects.buildings.push(Building {
                kind,
                cell: cell as u32,
                built_day: day,
                builder: Humans::id_of(hi),
                wood_used: 8.0,
                condition: 1.0,
                food_store: 0.0,
                burned: false,
                exists: true,
            });
            let bid = Objects::building_id(bidx);
            {
                let h = &mut sim.humans.list[hi];
                h.carried_wood -= 8.0;
                h.skills[SK_BUILD] = (h.skills[SK_BUILD] + 0.01).min(1.0);
                if h.home.is_none() {
                    h.home = bid;
                }
            }
            sim.history.push(
                day,
                EntityRef::Person(Humans::id_of(hi)),
                EventKind::BuiltBuilding { building: EntityRef::Building(bid) },
                Some(cell as u32),
                vec![],
            );
            // spouse moves in
            let spouse = sim.humans.list[hi].spouse;
            if let Some(s) = spouse.some() {
                let sh = &mut sim.humans.list[s.index()];
                if sh.home.is_none() {
                    sh.home = bid;
                }
            }
        }
        HumanAction::Deposit => {
            let home = sim.humans.list[hi].home;
            if let Some(b) = home.some() {
                let (hx, hy) = sim.grid.xy(sim.objects.buildings[b.index()].cell as usize);
                let at_home = {
                    let h = &sim.humans.list[hi];
                    (h.x - hx).abs().max((h.y - hy).abs()) <= 1
                };
                if !at_home {
                    move_toward_h(sim, hi, (hx, hy), 2);
                    return;
                }
                let (amt, cell) = {
                    let h = &mut sim.humans.list[hi];
                    let amt = h.carried_food - 0.5;
                    if amt <= 0.0 {
                        return;
                    }
                    h.carried_food -= amt;
                    (amt, sim.objects.buildings[b.index()].cell)
                };
                sim.objects.buildings[b.index()].food_store += amt;
                if amt > 1.5 {
                    sim.history.push(
                        day,
                        EntityRef::Person(Humans::id_of(hi)),
                        EventKind::StoredFood { amount: amt },
                        Some(cell),
                        vec![],
                    );
                }
            } else {
                return;
            }
        }
        HumanAction::Socialize { with } => {
            socialize(sim, hi, with as usize);
        }
        HumanAction::Court { with } => {
            court(sim, hi, with as usize);
        }
        HumanAction::TendFarm => {
            tend_farm(sim, hi);
        }
        HumanAction::Rest => {
            let h = &mut sim.humans.list[hi];
            h.fatigue = (h.fatigue - 0.5).max(0.0);
        }
        HumanAction::Idle => {
            // drift back toward home/camp — the gravity that keeps bands and villages together
            let to = {
                let h = &sim.humans.list[hi];
                match h.home.some() {
                    Some(b) => sim.grid.xy(sim.objects.buildings[b.index()].cell as usize),
                    None => h.camp,
                }
            };
            move_toward_h(sim, hi, to, 1);
        }
    }
    let _ = deaths;
}

/// Hunting: approach the actually-perceived animal; a struggle at adjacency, better odds with
/// skill and courage; wounded hunters and failed hunts are real outcomes.
fn hunt_animal(sim: &mut Sim, hi: usize, ai: usize) {
    if ai >= sim.animals.list.len() || !sim.animals.list[ai].alive {
        return;
    }
    let (dir, dist) = {
        let h = &sim.humans.list[hi];
        let a = &sim.animals.list[ai];
        (((a.x - h.x).signum(), (a.y - h.y).signum()), (a.x - h.x).abs().max((a.y - h.y).abs()))
    };
    if dist > 1 {
        let to = {
            let a = &sim.animals.list[ai];
            (a.x, a.y)
        };
        move_toward_h(sim, hi, to, 2);
    }
    let _ = dir;
    let dist_now = {
        let h = &sim.humans.list[hi];
        let a = &sim.animals.list[ai];
        (a.x - h.x).abs().max((a.y - h.y).abs())
    };
    if dist_now > 1 {
        return;
    }
    let (p_kill, meat) = {
        let h = &sim.humans.list[hi];
        let a = &sim.animals.list[ai];
        let sp = &ANIMALS[a.species as usize];
        let att = 30.0 * (0.5 + h.skills[SK_HUNT] * 1.2) * (0.6 + h.traits[1] * 0.5);
        let def = sp.mass * a.condition * 0.6 + sp.speed as f32 * 5.0;
        (((att / (att + def)) * 0.8).clamp(0.05, 0.85), sp.mass * (0.4 + a.condition * 0.4))
    };
    let roll = sim.humans.list[hi].rng.chance(p_kill);
    if roll {
        let cell = {
            let a = &sim.animals.list[ai];
            sim.grid.idx(a.x, a.y).unwrap() as u32
        };
        let kill_ev = sim.history.push(
            sim.clock.day,
            EntityRef::Animal(crate::animals::Animals::id_of(ai)),
            EventKind::Killed { by: EntityRef::Person(Humans::id_of(hi)) },
            Some(cell),
            vec![Cause::State(StateRef {
                what: StateKind::Hunger,
                value: sim.humans.list[hi].hunger,
            })],
        );
        crate::animals::kill_animal(
            sim,
            ai,
            DeathCause::Predation,
            vec![Cause::Event(kill_ev)],
            false,
        );
        let h = &mut sim.humans.list[hi];
        h.carried_food += meat * 0.12; // dressed yield in food units
        h.skills[SK_HUNT] = (h.skills[SK_HUNT] + 0.006).min(1.0);
        h.fatigue = (h.fatigue + 0.3).min(1.5);
    } else {
        let h = &mut sim.humans.list[hi];
        h.fatigue = (h.fatigue + 0.35).min(1.5);
        h.skills[SK_HUNT] = (h.skills[SK_HUNT] + 0.002).min(1.0);
    }
}

/// Talking: bonds strengthen from the real meeting; techniques are taught along strong bonds;
/// memories can be retold (the raw material of legend and grievance).
fn socialize(sim: &mut Sim, hi: usize, oi: usize) {
    if oi >= sim.humans.list.len() || !sim.humans.list[oi].alive {
        return;
    }
    let day = sim.clock.day;
    let dist = {
        let h = &sim.humans.list[hi];
        let o = &sim.humans.list[oi];
        (o.x - h.x).abs().max((o.y - h.y).abs())
    };
    if dist > 1 {
        let to = {
            let o = &sim.humans.list[oi];
            (o.x, o.y)
        };
        move_toward_h(sim, hi, to, 2);
        return;
    }
    let had_bond = sim.humans.rel_strength(hi, Humans::id_of(oi)) > 0.0;
    add_rel(sim, hi, oi, RelKind::Friend, 0.08);
    add_rel(sim, oi, hi, RelKind::Friend, 0.08);
    if !had_bond {
        sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::BondFormed {
                with: EntityRef::Person(Humans::id_of(oi)),
                context: BondContext::Neighbors,
            },
            None,
            vec![],
        );
    }
    {
        let h = &mut sim.humans.list[hi];
        h.skills[SK_SPEAK] = (h.skills[SK_SPEAK] + 0.001).min(1.0);
    }
    // beliefs travel inside real conversations
    crate::society::share_belief(sim, hi, oi);
    crate::society::share_belief(sim, oi, hi);
    // teaching: a technique passes along a real bond from one head to another
    let bond = sim.humans.rel_strength(hi, Humans::id_of(oi));
    if bond > 0.4 {
        let teachable: Vec<u8> = {
            let h = &sim.humans.list[hi];
            let o = &sim.humans.list[oi];
            h.techs.iter().copied().filter(|t| !o.techs.contains(t)).collect()
        };
        if let Some(&tech) = teachable.first() {
            let will = {
                let h = &mut sim.humans.list[hi];
                h.rng.chance(0.15 + h.traits[2] * 0.2)
            };
            if will {
                sim.humans.list[oi].techs.push(tech);
                let t_ev = sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(hi)),
                    EventKind::TaughtSkill {
                        student: EntityRef::Person(Humans::id_of(oi)),
                        skill: tech,
                    },
                    None,
                    vec![],
                );
                sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(oi)),
                    EventKind::LearnedSkill { skill: tech, teacher: EntityRef::Person(Humans::id_of(hi)) },
                    None,
                    vec![Cause::Event(t_ev)],
                );
                let hid = Humans::id_of(hi);
                let oid = Humans::id_of(oi);
                remember(&mut sim.humans.list[hi], day, MemoryKind::Taught { tech, to: oid }, 0.6);
                remember(
                    &mut sim.humans.list[oi],
                    day,
                    MemoryKind::WasTaught { tech, by: hid },
                    1.0,
                );
                add_rel(sim, hi, oi, RelKind::Teacher, 0.3);
                add_rel(sim, oi, hi, RelKind::Student, 0.3);
            }
        }
    }
}

fn court(sim: &mut Sim, hi: usize, oi: usize) {
    if oi >= sim.humans.list.len() || !sim.humans.list[oi].alive {
        return;
    }
    let day = sim.clock.day;
    let dist = {
        let h = &sim.humans.list[hi];
        let o = &sim.humans.list[oi];
        (o.x - h.x).abs().max((o.y - h.y).abs())
    };
    if dist > 1 {
        let to = {
            let o = &sim.humans.list[oi];
            (o.x, o.y)
        };
        move_toward_h(sim, hi, to, 2);
        return;
    }
    // strengthen the bond through the real meeting; marriage when it is strong and both free
    add_rel(sim, hi, oi, RelKind::Friend, 0.15);
    add_rel(sim, oi, hi, RelKind::Friend, 0.12);
    let bond = sim.humans.rel_strength(hi, Humans::id_of(oi));
    let both_free = sim.humans.list[hi].spouse.is_none() && sim.humans.list[oi].spouse.is_none();
    if both_free && bond > 0.7 {
        let hid = Humans::id_of(hi);
        let oid = Humans::id_of(oi);
        sim.humans.list[hi].spouse = oid;
        sim.humans.list[oi].spouse = hid;
        add_rel(sim, hi, oi, RelKind::Spouse, 2.0);
        add_rel(sim, oi, hi, RelKind::Spouse, 2.0);
        let shared_camp = sim.humans.list[hi].camp;
        sim.humans.list[oi].camp = shared_camp;
        remember(&mut sim.humans.list[hi], day, MemoryKind::Married { to: oid }, 1.5);
        remember(&mut sim.humans.list[oi], day, MemoryKind::Married { to: hid }, 1.5);
        sim.history.push(
            day,
            EntityRef::Person(hid),
            EventKind::Married { with: EntityRef::Person(oid) },
            None,
            vec![],
        );
    }
    // conception between spouses who are actually together
    let married = sim.humans.list[hi].spouse == Humans::id_of(oi);
    if married {
        let (fi, mi) = if sim.humans.list[hi].sex == 0 { (hi, oi) } else { (oi, hi) };
        let can = {
            let f = &sim.humans.list[fi];
            let age_y = (day as i64 - f.born) as f32 / 360.0;
            f.sex == 0
                && f.pregnant_by.is_none()
                && (14.0..46.0).contains(&age_y)
                && f.nutrition > 0.35
        };
        if can {
            let conceive = {
                let f = &mut sim.humans.list[fi];
                f.rng.chance(0.10)
            };
            if conceive {
                let sire = Humans::id_of(mi);
                let f = &mut sim.humans.list[fi];
                f.pregnant_by = sire;
                f.due_day = day + 270;
            }
        }
    }
}

/// Farming (requires the technique): convert a nearby cell into a wheat plot by sowing real
/// crop plants; tended plots feed households and are how agriculture spreads on the map.
fn tend_farm(sim: &mut Sim, hi: usize) {
    let day = sim.clock.day;
    let cell = {
        let h = &sim.humans.list[hi];
        sim.grid.idx(h.x, h.y)
    };
    let Some(cell) = cell else { return };
    // harvest ripe wheat here, else sow
    let pids = sim.plants.by_cell.get(cell).cloned().unwrap_or_default();
    let mut harvested = 0.0f32;
    let mut kills = Vec::new();
    let mut wheat_here = 0;
    for &pi in &pids {
        let pl = &sim.plants.list[pi as usize];
        if !pl.alive || pl.species != crate::species::SP_WHEAT {
            continue;
        }
        wheat_here += 1;
        if pl.biomass > 0.35 {
            harvested += pl.biomass * PLANTS[pl.species as usize].edible;
            kills.push(pi as usize);
        }
    }
    for pi in kills {
        crate::vegetation::kill_plant(sim, pi, DeathCause::Felled, vec![]);
    }
    if harvested > 0.0 {
        let h = &mut sim.humans.list[hi];
        h.carried_food += harvested;
        h.skills[SK_FARM] = (h.skills[SK_FARM] + 0.004).min(1.0);
        sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::HarvestedFood { amount: harvested },
            Some(cell as u32),
            vec![],
        );
        return;
    }
    if wheat_here < 6 {
        // sow: seeds become real crop plants
        let n_sow = 3;
        for _ in 0..n_sow {
            sim.plants.spawn(sim.cfg.seed, crate::species::SP_WHEAT, cell as u32, day as i64, 0.05);
        }
        let h = &mut sim.humans.list[hi];
        h.skills[SK_FARM] = (h.skills[SK_FARM] + 0.002).min(1.0);
        h.fatigue = (h.fatigue + 0.2).min(1.5);
    }
}

pub fn kill_human(sim: &mut Sim, hi: usize, cause: DeathCause, causes: Vec<Cause>) {
    let (cell, name) = {
        let h = &mut sim.humans.list[hi];
        if !h.alive {
            return;
        }
        h.alive = false;
        (sim.grid.idx(h.x, h.y).map(|i| i as u32), h.name.clone())
    };
    let _ = name;
    let hid = Humans::id_of(hi);
    let death_ev = sim.history.push(
        sim.clock.day,
        EntityRef::Person(hid),
        EventKind::Died { cause },
        cell,
        causes,
    );
    // grief and memory ripple through real kin and bonds; widow unlink
    let day = sim.clock.day;
    let (spouse, kin): (PersonId, Vec<usize>) = {
        let h = &sim.humans.list[hi];
        let mut kin: Vec<usize> = h
            .rels
            .iter()
            .filter(|r| {
                matches!(r.kind, RelKind::Spouse | RelKind::ParentOf | RelKind::ChildOf)
                    || (r.kind == RelKind::Friend && r.strength > 0.8)
            })
            .map(|r| r.other.index())
            .collect();
        kin.sort_unstable();
        kin.dedup();
        (h.spouse, kin)
    };
    for ki in kin {
        if ki < sim.humans.list.len() && sim.humans.list[ki].alive {
            let k = &mut sim.humans.list[ki];
            k.grief = (k.grief + 0.8).min(2.0);
            remember(k, day, MemoryKind::KinDied { who: hid }, 1.4);
        }
    }
    if let Some(s) = spouse.some() {
        if s.index() < sim.humans.list.len() {
            sim.humans.list[s.index()].spouse = PersonId::NONE;
        }
    }
    let _ = death_ev;
}

/// Derived observation.
pub fn population(sim: &Sim) -> usize {
    sim.humans.list.iter().filter(|h| h.alive).count()
}
