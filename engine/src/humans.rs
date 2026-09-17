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
    WasHelped { by: PersonId },   // the debt of a shared meal, remembered
    Helped { who: PersonId },     // generosity, remembered by the giver too
    StolenFrom { thief: PersonId }, // a grievance with a name on it
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
    pub preserve: f32,
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
    PreserveFood,              // smoke/dry the fresh stores over the hearth
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
    pub lineage: u32, // the mother-line: kin recognition beyond the nuclear family
    pub mother: PersonId,
    pub father: PersonId,
    pub spouse: PersonId,
    pub pregnant_by: PersonId,
    pub due_day: u64,
    pub last_birth_day: i64,  // for the postpartum recovery window (-1 = never)
    pub pregnancies: u16,     // how many she has carried
    pub carrying_twins: bool,
    // economy
    pub carried_food: f32,
    pub seed_genes: Option<[f32; 2]>, // the strain you saved seed from — selection in action
    pub carried_water: f32, // waterskin: carrying is the oldest human technology
    pub carried_wood: f32,
    pub home: BuildingId,
    // mind state
    pub rationale: HumanRationale,
    pub current: HumanAction,
    pub camp: (i32, i32), // band/home anchor: where this person considers "ours"
    pub area_yield: f32,  // rolling memory of what this ground has been giving
    pub water_memory: Option<(i32, i32)>,
    pub infections: Vec<(u8, u64, crate::core::ids::EventId)>, // pathogen, day caught, case event
    pub immune: Vec<u8>,
    /// What this head knows about the land: (kind 0=forage 1=water 2=danger, x, y, day learned).
    /// Spread person-to-person in conversation; stale knowledge misleads.
    pub known_spots: Vec<(u8, i32, i32, u64)>,
    /// Seasonal hunger, remembered: EMA of hungry days per season — the raw material of foresight.
    pub season_hunger: [f32; 4],
    pub vengeance: Option<crate::core::ids::AnimalId>, // the beast that took your kin
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
            lineage: 0,
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
            last_birth_day: -1,
            pregnancies: 0,
            carrying_twins: false,
            carried_food: 0.5,
            seed_genes: None,
            carried_water: 4.0,
            carried_wood: 0.0,
            home: BuildingId::NONE,
            rationale: HumanRationale::default(),
            current: HumanAction::Idle,
            camp: (x, y),
            area_yield: 1.0,
            water_memory: None,
            infections: Vec::new(),
            immune: Vec::new(),
            known_spots: Vec::new(),
            season_hunger: [0.0; 4],
            vengeance: None,
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
        let band_n = 17 + (rng.next_u64() % 6) as i64;
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
            sim.humans.list[ni].lineage = ni as u32; // a founder begins a mother-line
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
    if p.water_near.is_none() {
        p.water_near = h
            .known_spots
            .iter()
            .filter(|(k, _, _, _)| *k == 1)
            .max_by_key(|(_, _, _, learned)| *learned)
            .map(|(_, x, y, _)| (*x, *y));
    }
    // the band's stores: nearest hut with food within reach (forager sharing commons —
    // formal ownership/households arrive with Stage 5 institutions)
    let mut best_d = i32::MAX;
    for (bi, b) in sim.objects.buildings.iter().enumerate() {
        if !b.standing() || b.food_store < 0.3 {
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
    // the debt of blood: if the beast that took your kin is within sight, nothing else matters
    if let Some(beast) = h.vengeance {
        let bi = beast.index();
        if bi < sim.animals.list.len() {
            let b = &sim.animals.list[bi];
            if b.alive && (b.x - h.x).abs().max((b.y - h.y).abs()) <= 9 {
                r.hunt = r.hunt.max(2.2 + h.traits[1]);
            } else if !b.alive {
                // the debt is paid or the beast is gone
            }
        }
    }
    // shelter: homeless adults want a hut; homeowners keep stores stocked
    let my_house_unfinished = h
        .home
        .some()
        .map(|b| {
            let bl = &sim.objects.buildings[b.index()];
            bl.exists && bl.progress < 1.0
        })
        .unwrap_or(false);
    // a village project you'd benefit from: any unfinished granary or palisade near your home
    let community_site = sim.objects.buildings.iter().position(|b| {
        b.exists
            && b.progress < 1.0
            && matches!(
                b.kind,
                crate::objects::BuildingKind::Granary | crate::objects::BuildingKind::Palisade
            )
            && {
                let (bx, by) = sim.grid.xy(b.cell as usize);
                (bx - h.x).abs().max((by - h.y).abs()) <= 10
            }
    });
    if adult && food_avail > 1.0 && h.hunger < 0.8 {
        let season = day % 360 / 90;
        let winter_presses = if season == 2 { 0.5 } else { 0.0 }; // autumn urgency
        if my_house_unfinished {
            r.build = 1.0 + winter_presses;
        } else if community_site.is_some() {
            // many hands: the village raises what no single roof could
            r.build = 0.8 + h.traits[2] * 0.3 + winter_presses;
        } else if h.home.is_none() && h.nutrition > 0.45 {
            r.build = 0.7 + h.traits[0] * 0.4 + winter_presses;
        }
    }
    // farming: knowers of the technique work plots by their homes — sowing in the growing
    // seasons, harvesting when the wheat actually stands ripe. The consequence chain is real:
    // no sowing → no stand → no harvest → hungry winter.
    if adult && h.techs.contains(&TECH_FARMING) && !h.home.is_none() {
        let season = day % 360 / 90; // 0 spring 1 summer 2 autumn 3 winter
        r.farm = match season {
            0 | 1 => 1.0 + h.hunger * 0.3,
            2 => 1.3, // harvest presses
            _ => 0.0, // fields sleep in winter
        };
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
    // foresight, learned the hard way: if the coming season has starved you before, the
    // memory of it drives the gathering and the racks NOW (schemas from lived seasons)
    let season_now = (day % 360 / 90) as usize;
    let next_season = (season_now + 1) % 4;
    let dread = h.season_hunger[next_season] + h.season_hunger[season_now] * 0.5;
    if dread > 0.15 {
        r.gather *= 1.0 + dread.min(1.0);
    }
    // the smell of frost: fresh stores spoil, smoked ones keep — autumn is for the racks
    if adult {
        if let Some(b) = h.home.some() {
            let bl = &sim.objects.buildings[b.index()];
            if bl.standing() && bl.food_store > 1.5 && bl.firewood > 0.5 {
                let season = day % 360 / 90;
                r.preserve = 0.4
                    + bl.food_store * 0.08
                    + if season == 2 { 0.6 } else { 0.0 }
                    + dread.min(1.0) * 0.5;
            }
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
        (r.preserve, 9),
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
                // the land here is spent: go where you KNOW food was, or where someone told
                // you it was — and only failing all knowledge, explore along the water
                let known = h
                    .known_spots
                    .iter()
                    .filter(|(k, x, y, learned)| {
                        *k == 0
                            && day.saturating_sub(*learned) < 720
                            && (*x - h.x).abs().max((*y - h.y).abs()) > 4
                    })
                    .max_by_key(|(_, _, _, learned)| *learned)
                    .map(|(_, x, y, _)| (*x, *y));
                if let Some(to) = known {
                    HumanAction::MoveTo { to }
                } else {
                    let hd = crate::core::rng::splitmix64(day ^ (h.born as u64).wrapping_mul(31));
                    let dirs: [(i32, i32); 8] =
                        [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
                    let d = dirs[(hd % 8) as usize];
                    let (ax, ay) = p.water_near.unwrap_or((h.x, h.y));
                    HumanAction::MoveTo { to: (ax + d.0 * 5, ay + d.1 * 5) }
                }
            } else if p.forage_here > 0.05 && p.forage_here * 1.3 >= p.forage_near_val {
                HumanAction::Gather
            } else if let Some(to) = p.forage_near {
                HumanAction::MoveTo { to }
            } else {
                HumanAction::Gather
            }
        }
        4 => {
            // the sworn quarry first — the beast your kin died to
            let vengeance_target = h.vengeance.and_then(|b| {
                let bi = b.index();
                sim.animals.list.get(bi).and_then(|a| {
                    if a.alive && (a.x - h.x).abs().max((a.y - h.y).abs()) <= 9 {
                        Some(bi as u32)
                    } else {
                        None
                    }
                })
            });
            match vengeance_target.or(p.game) {
                Some(t) => HumanAction::Hunt { target: t },
                None => HumanAction::Idle,
            }
        }
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
                        && o.lineage != h.lineage
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
        9 => HumanAction::PreserveFood,
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

/// A woman's chance of conceiving on a given day she is with her partner — the product of a
/// real age-fertility curve, her nutrition and health, and the postpartum recovery window.
/// Zero outside the fertile years; peaks in the twenties; most days do NOT end in conception.
pub fn female_fertility(age_y: f32, nutrition: f32, health: f32, days_since_birth: i64) -> f32 {
    let age_f = if age_y < 15.0 {
        0.0
    } else if age_y < 21.0 {
        (age_y - 15.0) / 6.0 // ramp to full over the late teens
    } else if age_y < 33.0 {
        1.0 // peak years
    } else if age_y < 41.0 {
        1.0 - (age_y - 33.0) / 8.0 * 0.7 // gentle decline
    } else if age_y < 49.0 {
        0.3 - (age_y - 41.0) / 8.0 * 0.3 // rare, then gone
    } else {
        0.0
    };
    if age_f <= 0.0 {
        return 0.0;
    }
    let body = ((nutrition - 0.35) / 0.5).clamp(0.0, 1.0) * (0.4 + 0.6 * health.clamp(0.0, 1.0));
    // the body needs time between children; fertility returns over ~a year of nursing
    let post = if days_since_birth < 0 {
        1.0
    } else {
        ((days_since_birth as f32 - 120.0) / 300.0).clamp(0.0, 1.0)
    };
    age_f * body * post
}

pub fn remember(h: &mut Human, day: u64, kind: MemoryKind, weight: f32) {
    h.memories.push(Memory { day, kind, weight });
}

/// Keep a bounded working set of place-knowledge: newest of each kind wins ties by distance.
pub fn remember_spot(h: &mut Human, kind: u8, x: i32, y: i32, day: u64) {
    if let Some(e) = h
        .known_spots
        .iter_mut()
        .find(|(k, sx, sy, _)| *k == kind && (*sx - x).abs().max((*sy - y).abs()) <= 3)
    {
        e.3 = day;
        return;
    }
    h.known_spots.push((kind, x, y, day));
    if h.known_spots.len() > 24 {
        // the oldest knowledge fades from the working mind (history keeps everything; heads don't)
        let oldest = h
            .known_spots
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.3)
            .map(|(i, _)| i)
            .unwrap();
        h.known_spots.remove(oldest);
    }
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

fn wear_ground(sim: &mut Sim, hi: usize) {
    let (x, y) = {
        let h = &sim.humans.list[hi];
        (h.x, h.y)
    };
    if let Some(j) = sim.grid.idx(x, y) {
        // a footstep crushes the grass a little; ten thousand make a road
        sim.grid.wear[j] = (sim.grid.wear[j] + 0.015).min(1.0);
    }
}

fn move_human(sim: &mut Sim, hi: usize, dir: (i32, i32), steps: i32) {
    for _ in 0..steps {
        let (nx, ny) = {
            let h = &sim.humans.list[hi];
            (h.x + dir.0, h.y + dir.1)
        };
        match sim.grid.idx(nx, ny) {
            Some(j)
                if !sim.grid.ocean[j]
                    && (sim.grid.surface[j] <= 0.6 || sim.grid.ice_bears(j)) =>
            {
                let deep = sim.grid.surface[j] > 0.2 && !sim.grid.ice_bears(j);
                let h = &mut sim.humans.list[hi];
                h.x = nx;
                h.y = ny;
                if deep {
                    // fording costs the day and the legs
                    h.fatigue = (h.fatigue + 0.15).min(1.5);
                    break;
                }
            }
            _ => break,
        }
    }
}

/// Fresh water on your cell or any neighboring cell is drinkable — you kneel at the bank.
fn near_fresh(sim: &Sim, x: i32, y: i32) -> bool {
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

/// Step toward a destination, stopping ON it (never overshooting past it).
/// Beaten paths carry you further: an extra step when the ground underfoot is a road.
fn move_toward_h(sim: &mut Sim, hi: usize, to: (i32, i32), steps: i32) {
    let on_path = {
        let h = &sim.humans.list[hi];
        sim.grid.idx(h.x, h.y).map(|j| sim.grid.is_path(j)).unwrap_or(false)
    };
    let steps = if on_path { steps + 1 } else { steps };
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
            Some(j)
                if !sim.grid.ocean[j]
                    && (sim.grid.surface[j] <= 0.6 || sim.grid.ice_bears(j)) =>
            {
                let deep = sim.grid.surface[j] > 0.2 && !sim.grid.ice_bears(j);
                let h = &mut sim.humans.list[hi];
                h.x = nx;
                h.y = ny;
                if deep {
                    h.fatigue = (h.fatigue + 0.15).min(1.5);
                    break;
                }
            }
            _ => break,
        }
        wear_ground(sim, hi);
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
            // foresight, not crisis: when the ground has gone thin for a homeless forager,
            // strike camp BEFORE starving — the seasonal round of real foragers
            if h.home.is_none() && h.area_yield < 0.25 && h.days_starving == 0 {
                let hd = crate::core::rng::splitmix64(
                    (day / 30) ^ (h.born as u64).wrapping_mul(131),
                );
                let dirs: [(i32, i32); 8] =
                    [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
                let d = dirs[(hd % 8) as usize];
                let (ax, ay) = h.water_memory.unwrap_or((h.x, h.y));
                h.camp = (ax + d.0 * 10, ay + d.1 * 10);
                h.area_yield = 0.8; // hope, to be tested against the new ground
            }
        }
        apply_action(sim, hi, act, &mut deaths);
    }

    // ---------- floods: people wade out; dwellings the water takes are lost ----------
    for hi in 0..n {
        let (x, y, alive) = {
            let h = &sim.humans.list[hi];
            (h.x, h.y, h.alive)
        };
        if !alive {
            continue;
        }
        if let Some(here) = sim.grid.idx(x, y) {
            if !sim.grid.ocean[here] && sim.grid.surface[here] > 0.6 {
                let mut best: Option<((i32, i32), f32)> = None;
                for r in 1..=5i32 {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dy.abs() != r {
                                continue;
                            }
                            if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                                if !sim.grid.ocean[j] && sim.grid.surface[j] < 0.5 {
                                    let d = sim.grid.surface[j];
                                    if best.map(|(_, bd)| d < bd).unwrap_or(true) {
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
                    let h = &mut sim.humans.list[hi];
                    h.x = to.0;
                    h.y = to.1;
                    h.fatigue = (h.fatigue + 0.4).min(1.5);
                    h.fear = (h.fear + 0.5).min(2.0);
                    if h.camp == (x, y) {
                        h.camp = to;
                    }
                }
            }
        }
    }
    if day % 10 == 4 {
        for bi in 0..sim.objects.buildings.len() {
            let (lost, builder, cell) = {
                let b = &sim.objects.buildings[bi];
                if !b.exists {
                    continue;
                }
                (
                    sim.grid.surface[b.cell as usize] > 0.5,
                    b.builder,
                    b.cell,
                )
            };
            if lost {
                let salvage = {
                    let b = &mut sim.objects.buildings[bi];
                    b.exists = false;
                    b.food_store * 0.6 // most of the store is carried to safety
                };
                if let Some(o) = builder.some() {
                    if o.index() < sim.humans.list.len() && sim.humans.list[o.index()].alive {
                        sim.humans.list[o.index()].carried_food += salvage;
                    }
                }
                let bid = crate::objects::Objects::building_id(bi);
                sim.history.push(
                    day,
                    EntityRef::Person(builder),
                    EventKind::BuildingLost {
                        building: EntityRef::Building(bid),
                        to_flood: true,
                    },
                    Some(cell),
                    vec![],
                );
                // whoever called it home is homeless now
                for h in sim.humans.list.iter_mut() {
                    if h.home == bid {
                        h.home = crate::core::ids::BuildingId::NONE;
                    }
                }
            }
        }
    }

    // ---------- herding: feeding tames; a keeper's beasts feed the lean months ----------
    for hi in 0..n {
        let (adult, food, pos, hungry, own_store_low) = {
            let h = &sim.humans.list[hi];
            if !h.alive {
                continue;
            }
            let age_y = (day as i64 - h.born) as f32 / 360.0;
            let store = h
                .home
                .some()
                .map(|b| sim.objects.buildings[b.index()].food_store)
                .unwrap_or(0.0);
            (
                age_y >= 14.0,
                h.carried_food,
                (h.x, h.y),
                h.hunger > 1.0,
                store < 2.0,
            )
        };
        if !adult {
            continue;
        }
        // find an adjacent domesticable animal (deterministic first match)
        let mut adj: Option<usize> = None;
        sim.animals.index.for_each_near(pos.0, pos.1, 1, |ai| {
            if adj.is_some() {
                return;
            }
            let a = &sim.animals.list[ai as usize];
            if a.alive
                && crate::species::ANIMALS[a.species as usize].domesticable
                && (a.x - pos.0).abs().max((a.y - pos.1).abs()) <= 1
            {
                adj = Some(ai as usize);
            }
        });
        let Some(ai) = adj else { continue };
        let owner_id = Humans::id_of(hi);
        let is_mine = sim.animals.list[ai].tamed_by == owner_id;
        let is_wild = sim.animals.list[ai].tamed_by.is_none();
        let warm = sim.humans.list[hi].traits[2] > 0.45;
        if is_wild && food > 1.5 && warm {
            // a patient hand and a shared meal: taming is earned day by day
            let warmth = sim.humans.list[hi].traits[2];
            {
                let h = &mut sim.humans.list[hi];
                h.carried_food -= 0.10;
            }
            let a = &mut sim.animals.list[ai];
            a.hunger = (a.hunger - 0.4).max(0.0);
            a.tame_prog += 0.6 + warmth * 0.6;
            let _ = &a;
            zoonotic_exposure(sim, hi, ai);
            let a = &mut sim.animals.list[ai];
            let _ = a;
            if a.tame_prog >= 6.0 {
                a.tamed_by = owner_id;
                let aid = crate::animals::Animals::id_of(ai);
                sim.history.push(
                    day,
                    EntityRef::Person(owner_id),
                    EventKind::TamedAnimal { animal: EntityRef::Animal(aid) },
                    sim.grid.idx(pos.0, pos.1).map(|i| i as u32),
                    vec![],
                );
            }
        } else if is_mine && hungry && own_store_low {
            // winter's arithmetic: the herd is the larder
            let aid = crate::animals::Animals::id_of(ai);
            let meat = crate::species::ANIMALS[sim.animals.list[ai].species as usize].mass
                * (0.3 + sim.animals.list[ai].condition * 0.4);
            let ev = sim.history.push(
                day,
                EntityRef::Person(owner_id),
                EventKind::SlaughteredAnimal { animal: EntityRef::Animal(aid) },
                sim.grid.idx(pos.0, pos.1).map(|i| i as u32),
                vec![Cause::State(StateRef {
                    what: StateKind::Hunger,
                    value: sim.humans.list[hi].hunger,
                })],
            );
            crate::animals::kill_animal(
                sim,
                ai,
                crate::history::DeathCause::Slaughtered,
                vec![Cause::Event(ev)],
                false,
            );
            let h = &mut sim.humans.list[hi];
            h.carried_food += meat * 0.12;
            h.hunger = (h.hunger - 1.0).max(0.0);
        }
    }

    // ---------- the village takes shape: shared stores from surplus, walls from fear ----------
    if day % 30 == 21 {
        for hi in 0..n {
            let (alive, adult, pos, wood, warm) = {
                let h = &sim.humans.list[hi];
                let age_y = (day as i64 - h.born) as f32 / 360.0;
                (h.alive, age_y >= 16.0, (h.x, h.y), h.carried_wood, h.traits[2] > 0.4)
            };
            if !alive || !adult {
                continue;
            }
            // how many finished homes stand near here? (a hamlet in being)
            let mut homes = 0;
            let mut has_granary = false;
            let mut palisades = 0;
            for b in sim.objects.buildings.iter() {
                if !b.exists {
                    continue;
                }
                let (bx, by) = sim.grid.xy(b.cell as usize);
                if (bx - pos.0).abs().max((by - pos.1).abs()) <= 8 {
                    match b.kind {
                        crate::objects::BuildingKind::Hut if b.progress >= 1.0 => homes += 1,
                        crate::objects::BuildingKind::Granary => has_granary = true,
                        crate::objects::BuildingKind::Palisade => palisades += 1,
                        _ => {}
                    }
                }
            }
            if homes < 3 {
                continue;
            }
            // a common store rises where a hamlet stands and hands are willing
            if !has_granary && wood >= 4.0 && warm {
                if let Some(cell) = sim.grid.idx(pos.0, pos.1) {
                    if sim.objects.building_at(cell as u32).is_none()
                        && sim.grid.surface[cell] < 0.1
                    {
                        sim.objects.buildings.push(Building {
                            kind: crate::objects::BuildingKind::Granary,
                            cell: cell as u32,
                            built_day: day,
                            builder: Humans::id_of(hi),
                            wood_used: 4.0,
                            progress: 0.1,
                            condition: 1.0,
                            food_store: 0.0,
                            preserved_store: 0.0,
                            firewood: 0.0,
                            burned: false,
                            exists: true,
                        });
                        let h = &mut sim.humans.list[hi];
                        h.carried_wood -= 4.0;
                        continue;
                    }
                }
            }
            // walls rise from remembered violence: raids, beasts, thefts carried in THIS head
            let fear_of_others: f32 = sim.humans.list[hi]
                .memories
                .iter()
                .filter(|m| {
                    matches!(
                        m.kind,
                        MemoryKind::StolenFrom { .. }
                            | MemoryKind::KinDied { .. }
                            | MemoryKind::FledBeast { .. }
                    )
                })
                .map(|m| m.weight)
                .sum();
            if fear_of_others > 2.0 && palisades < 12 && wood >= 3.0 {
                // one segment at a time, on the hamlet's rim, nearest to this builder
                let mut best: Option<(usize, i32)> = None;
                for r in 4..=6i32 {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dy.abs() != r {
                                continue;
                            }
                            let Some(cell) = sim.grid.idx(pos.0 + dx, pos.1 + dy) else {
                                continue;
                            };
                            if sim.grid.ocean[cell]
                                || sim.grid.surface[cell] > 0.1
                                || sim.objects.building_at(cell as u32).is_some()
                            {
                                continue;
                            }
                            let d = dx.abs().max(dy.abs());
                            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                best = Some((cell, d));
                            }
                        }
                    }
                    if best.is_some() {
                        break;
                    }
                }
                if let Some((cell, _)) = best {
                    sim.objects.buildings.push(Building {
                        kind: crate::objects::BuildingKind::Palisade,
                        cell: cell as u32,
                        built_day: day,
                        builder: Humans::id_of(hi),
                        wood_used: 3.0,
                        progress: 0.15,
                        condition: 1.0,
                        food_store: 0.0,
                        preserved_store: 0.0,
                        firewood: 0.0,
                        burned: false,
                        exists: true,
                    });
                    let h = &mut sim.humans.list[hi];
                    h.carried_wood -= 3.0;
                }
            }
        }
    }

    // ---------- the moral economy: shared meals bind, stolen ones burn ----------
    for hi in 0..n {
        let (alive, adult, food, pos, lineage_id) = {
            let h = &sim.humans.list[hi];
            let age_y = (day as i64 - h.born) as f32 / 360.0;
            (h.alive, age_y >= 14.0, h.carried_food, (h.x, h.y), Humans::id_of(hi))
        };
        if !alive || !adult {
            continue;
        }
        // GENEROSITY: food in hand, a hungry neighbor in sight who is not your own child
        if food > 2.5 {
            let mut needy: Option<usize> = None;
            sim.humans.index.for_each_near(pos.0, pos.1, 2, |oi| {
                if needy.is_some() || oi as usize == hi {
                    return;
                }
                let o = &sim.humans.list[oi as usize];
                if o.alive && o.hunger > 1.0 && o.mother != lineage_id && o.father != lineage_id {
                    needy = Some(oi as usize);
                }
            });
            if let Some(oi) = needy {
                let give = 0.8f32;
                sim.humans.list[hi].carried_food -= give;
                {
                    let o = &mut sim.humans.list[oi];
                    o.hunger = (o.hunger - give).max(0.0);
                }
                let giver_id = Humans::id_of(hi);
                let taker_id = Humans::id_of(oi);
                remember(&mut sim.humans.list[oi], day, MemoryKind::WasHelped { by: giver_id }, 1.0);
                remember(&mut sim.humans.list[hi], day, MemoryKind::Helped { who: taker_id }, 0.6);
                add_rel(sim, oi, hi, RelKind::Friend, 0.25);
                add_rel(sim, hi, oi, RelKind::Friend, 0.1);
            }
        }
        // THEFT: starving, bold, and a stranger's stocked granary within reach
        let desperate = {
            let h = &sim.humans.list[hi];
            h.days_starving >= 3 && h.hunger > 1.1 && h.traits[1] > 0.55
        };
        if desperate {
            let target = sim.objects.buildings.iter().position(|b| {
                b.standing() && (b.food_store + b.preserved_store) > 1.0 && {
                    let (bx, by) = sim.grid.xy(b.cell as usize);
                    (bx - pos.0).abs().max((by - pos.1).abs()) <= 1
                        && sim.humans.rel_strength(hi, b.builder) < 0.2
                        && b.builder != Humans::id_of(hi)
                }
            });
            if let Some(bi) = target {
                let owner = sim.objects.buildings[bi].builder;
                let take = {
                    let b = &mut sim.objects.buildings[bi];
                    let t = (b.food_store + b.preserved_store).min(2.0);
                    let from_fresh = b.food_store.min(t);
                    b.food_store -= from_fresh;
                    b.preserved_store -= t - from_fresh;
                    t
                };
                {
                    let h = &mut sim.humans.list[hi];
                    h.carried_food += take;
                    h.hunger = (h.hunger - take.min(1.0)).max(0.0);
                }
                let thief_id = Humans::id_of(hi);
                let hunger_now = sim.humans.list[hi].hunger;
                sim.history.push(
                    day,
                    EntityRef::Person(thief_id),
                    EventKind::RaidCarriedOut { against: EntityRef::Person(owner) },
                    sim.grid.idx(pos.0, pos.1).map(|i| i as u32),
                    vec![Cause::State(StateRef { what: StateKind::Hunger, value: hunger_now })],
                );
                // whoever saw it — and the owner, when the loss is discovered — will not forget
                let mut witnesses: Vec<usize> = Vec::new();
                sim.humans.index.for_each_near(pos.0, pos.1, 4, |oi| {
                    if oi as usize != hi && sim.humans.list[oi as usize].alive {
                        witnesses.push(oi as usize);
                    }
                });
                for wi in witnesses {
                    remember(
                        &mut sim.humans.list[wi],
                        day,
                        MemoryKind::StolenFrom { thief: thief_id },
                        1.2,
                    );
                }
                if let Some(ow) = owner.some() {
                    if ow.index() < sim.humans.list.len() && sim.humans.list[ow.index()].alive {
                        remember(
                            &mut sim.humans.list[ow.index()],
                            day,
                            MemoryKind::StolenFrom { thief: thief_id },
                            1.5,
                        );
                    }
                }
            }
        }
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
            let season_now = (day % 360 / 90) as usize;
            if h.hunger >= 1.0 {
                h.season_hunger[season_now] =
                    (h.season_hunger[season_now] * 0.995 + 0.02).min(1.5);
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
            // sickness runs its course in the body it inhabits
            let mut recovered: Vec<u8> = Vec::new();
            let mut fatal: Option<(u8, crate::core::ids::EventId)> = None;
            for k in 0..h.infections.len() {
                let (pg, since, case_ev) = h.infections[k];
                let p = &crate::pathogens::PATHOGENS[pg as usize];
                let t = day.saturating_sub(since);
                if t < p.incubation_d {
                    continue;
                }
                if t > p.incubation_d + p.illness_d {
                    recovered.push(pg);
                    continue;
                }
                h.fatigue = (h.fatigue + 0.25).min(1.5);
                h.hunger = (h.hunger + 0.03).min(2.0);
                let risk = p.daily_lethality * (1.6 - h.health).max(0.2);
                if h.rng.chance(risk) {
                    fatal = Some((pg, case_ev));
                }
            }
            if let Some((_pg, case_ev)) = fatal {
                h.health = 0.0;
                let _ = case_ev;
            }
            for pg in &recovered {
                h.infections.retain(|(g, _, _)| g != pg);
                if !h.immune.contains(pg) {
                    h.immune.push(*pg);
                }
            }
            let _ = &recovered;
            // winter nights: walls are the difference between cold and dying of it
            let t_here = sim
                .grid
                .idx(h.x, h.y)
                .map(|i| sim.grid.temp[i])
                .unwrap_or(0.0);
            if t_here < -4.0 {
                let mut shelter = 0u8; // 0 open sky, 1 cold walls, 2 hearth-warmed
                for b in sim.objects.buildings.iter() {
                    if b.standing() {
                        let (bx, by) = sim.grid.xy(b.cell as usize);
                        if (bx - h.x).abs().max((by - h.y).abs()) <= 1 {
                            shelter = if b.firewood > 0.0 { 2 } else { shelter.max(1) };
                            if shelter == 2 {
                                break;
                            }
                        }
                    }
                }
                match shelter {
                    2 => {}
                    1 => {
                        h.health -= 0.006 * (-t_here / 10.0).min(2.0);
                    }
                    _ => {
                        h.health -= 0.012 * (-t_here / 10.0).min(2.0);
                        h.fatigue = (h.fatigue + 0.1).min(1.5);
                    }
                }
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
        if sim.humans.list[hi].health <= 0.0 {
            let sick_case = sim.humans.list[hi]
                .infections
                .iter()
                .find(|(pg, since, _)| {
                    day.saturating_sub(*since)
                        >= crate::pathogens::PATHOGENS[*pg as usize].incubation_d
                })
                .map(|(_, _, ev)| *ev);
            if let Some(case_ev) = sick_case {
                deaths.push((hi, DeathCause::Disease, vec![Cause::Event(case_ev)]));
            } else {
                let t = sim
                    .grid
                    .idx(sim.humans.list[hi].x, sim.humans.list[hi].y)
                    .map(|i| sim.grid.temp[i])
                    .unwrap_or(0.0);
                deaths.push((
                    hi,
                    DeathCause::Exposure,
                    vec![Cause::State(StateRef { what: StateKind::Temperature, value: t })],
                ));
            }
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
        let (x, y, culture, mname, twins, age_y, mhealth) = {
            let m = &mut sim.humans.list[mi];
            m.pregnant_by = PersonId::NONE;
            let t = m.carrying_twins;
            m.carrying_twins = false;
            m.last_birth_day = day as i64;
            m.pregnancies = m.pregnancies.saturating_add(1);
            let age_y = (day as i64 - m.born) as f32 / 360.0;
            (m.x, m.y, m.culture, m.name.clone(), t, age_y, m.health)
        };
        // the oldest danger: childbirth. Worse for the very young, the older mother, and twins.
        let birth_risk = (0.012
            + if age_y < 18.0 { 0.02 } else { 0.0 }
            + if age_y > 38.0 { 0.03 } else { 0.0 }
            + if twins { 0.02 } else { 0.0 })
            * (1.6 - mhealth).clamp(0.4, 1.6);
        let mother_dies = {
            let m = &mut sim.humans.list[mi];
            m.rng.chance(birth_risk)
        };
        let n_babies = if twins { 2 } else { 1 };
        for _ in 0..n_babies {
            // a stillbirth is a real, cited outcome, not an error
            let stillborn = {
                let m = &mut sim.humans.list[mi];
                m.rng.chance(0.03 + (0.5 - mhealth).max(0.0) * 0.1)
            };
            if stillborn {
                sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(mi)),
                    EventKind::Stillbirth,
                    sim.grid.idx(x, y).map(|i| i as u32),
                    vec![],
                );
                continue;
            }
            birth_one(sim, mi, father, x, y, culture);
        }
        if mother_dies {
            kill_human(sim, mi, DeathCause::Childbirth, vec![]);
        }
        let _ = mname;
    }


    // family life: parents provide — a child's camp is its mother's, and a parent who is
    // actually beside a hungry child shares from the hand. Orphans face the world alone:
    // a real consequence of every parental death.
    for hi in 0..n {
        let (mother, is_child) = {
            let h = &sim.humans.list[hi];
            if !h.alive {
                continue;
            }
            let age_y = (day as i64 - h.born) as f32 / 360.0;
            (h.mother, age_y < 14.0)
        };
        if !is_child {
            continue;
        }
        let Some(m) = mother.some() else { continue };
        let mi = m.index();
        if mi >= sim.humans.list.len() || !sim.humans.list[mi].alive {
            continue;
        }
        let (mcamp, mpos) = {
            let mo = &sim.humans.list[mi];
            (mo.camp, (mo.x, mo.y))
        };
        {
            let c = &mut sim.humans.list[hi];
            c.camp = mcamp;
        }
        let (near, child_hungry) = {
            let c = &sim.humans.list[hi];
            (
                (c.x - mpos.0).abs().max((c.y - mpos.1).abs()) <= 3,
                c.hunger > 0.7,
            )
        };
        if near && child_hungry {
            let give = {
                let mo = &mut sim.humans.list[mi];
                let g = (mo.carried_food - 0.3).max(0.0).min(0.8);
                mo.carried_food -= g;
                g
            };
            if give > 0.0 {
                let c = &mut sim.humans.list[hi];
                c.hunger = (c.hunger - give).max(0.0);
            } else {
                // nothing in hand: the mother draws on the granary for her child
                let home = sim.humans.list[mi].home;
                if let Some(b) = home.some() {
                    let bld = &mut sim.objects.buildings[b.index()];
                    let g = bld.food_store.min(0.8);
                    bld.food_store -= g;
                    let c = &mut sim.humans.list[hi];
                    c.hunger = (c.hunger - g).max(0.0);
                }
            }
        }
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
            let (p, twins) = {
                let h = &sim.humans.list[hi];
                let age_y = (day as i64 - h.born) as f32 / 360.0;
                let fert =
                    female_fertility(age_y, h.nutrition, h.health, if h.last_birth_day < 0 { -1 } else { day as i64 - h.last_birth_day });
                (0.035 * fert, age_y) // base daily chance × fertility; most days: nothing
            };
            let _ = twins;
            let conceive = {
                let h = &mut sim.humans.list[hi];
                h.rng.chance(p)
            };
            if conceive {
                let twin = {
                    let h = &mut sim.humans.list[hi];
                    h.rng.chance(0.018)
                };
                let (fx, fy) = {
                    let h = &mut sim.humans.list[hi];
                    h.pregnant_by = sp;
                    // a real, slightly variable nine months
                    h.due_day = day + h.rng.range_i(262, 284) as u64;
                    h.carrying_twins = twin;
                    (h.x, h.y)
                };
                sim.history.push(
                    day,
                    EntityRef::Person(Humans::id_of(hi)),
                    EventKind::Conceived { with: EntityRef::Person(sp) },
                    sim.grid.idx(fx, fy).map(|i| i as u32),
                    vec![],
                );
            }
        }
    }

    // pregnancy is carried in the body, and the body can fail it: starvation, sickness, or a
    // hard fall ends a pregnancy before its time — each loss cited to its real cause
    for hi in 0..n {
        let (pregnant, cause) = {
            let h = &sim.humans.list[hi];
            if !h.alive || h.pregnant_by.is_none() {
                (false, DeathCause::Age)
            } else if h.nutrition < 0.15 {
                (true, DeathCause::Starvation)
            } else if h
                .infections
                .iter()
                .any(|(pg, since, _)| day.saturating_sub(*since) >= crate::pathogens::PATHOGENS[*pg as usize].incubation_d)
            {
                (true, DeathCause::Disease)
            } else {
                (false, DeathCause::Age)
            }
        };
        if !pregnant {
            continue;
        }
        let risk = if cause == DeathCause::Starvation { 0.02 } else { 0.006 };
        let lost = {
            let h = &mut sim.humans.list[hi];
            h.rng.chance(risk)
        };
        if lost {
            let hx = sim.humans.list[hi].x;
            let hy = sim.humans.list[hi].y;
            {
                let h = &mut sim.humans.list[hi];
                h.pregnant_by = PersonId::NONE;
                h.carrying_twins = false;
                h.grief = (h.grief + 0.6).min(2.0);
            }
            sim.history.push(
                day,
                EntityRef::Person(Humans::id_of(hi)),
                EventKind::Miscarried { cause },
                sim.grid.idx(hx, hy).map(|i| i as u32),
                vec![],
            );
        }
    }

    for (hi, cause, causes) in deaths {
        kill_human(sim, hi, cause, causes);
    }

    // houses weather; unrepaired they fall to ruin
    if day % 360 == 150 {
        for bi in 0..sim.objects.buildings.len() {
            let (fell, builder, cell) = {
                let b = &mut sim.objects.buildings[bi];
                if !b.exists || b.progress < 1.0 {
                    continue;
                }
                b.condition -= 0.06;
                (b.condition <= 0.3, b.builder, b.cell)
            };
            if fell {
                sim.objects.buildings[bi].exists = false;
                let bid = Objects::building_id(bi);
                sim.history.push(
                    day,
                    EntityRef::Person(builder),
                    EventKind::BuildingLost {
                        building: EntityRef::Building(bid),
                        to_flood: false,
                    },
                    Some(cell),
                    vec![],
                );
                for h in sim.humans.list.iter_mut() {
                    if h.home == bid {
                        h.home = crate::core::ids::BuildingId::NONE;
                    }
                }
            }
        }
    }
    // repair: a homeowner at home with logs shores the walls up
    for hi in 0..n {
        let (home, wood, pos) = {
            let h = &sim.humans.list[hi];
            if !h.alive {
                continue;
            }
            (h.home, h.carried_wood, (h.x, h.y))
        };
        let Some(b) = home.some() else { continue };
        let bi = b.index();
        let needs = {
            let bl = &sim.objects.buildings[bi];
            bl.exists && bl.progress >= 1.0 && bl.condition < 0.8
        };
        let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
        let at_home = (bx - pos.0).abs().max((by - pos.1).abs()) <= 1;
        if at_home && wood >= 1.0 {
            if needs {
                sim.humans.list[hi].carried_wood -= 1.0;
                let bl = &mut sim.objects.buildings[bi];
                bl.condition = (bl.condition + 0.25).min(1.0);
                bl.wood_used += 1.0;
            } else {
                // the woodpile: warmth for the cold months
                let bl = &mut sim.objects.buildings[bi];
                if bl.firewood < 12.0 {
                    let put = (wood - 0.5).min(3.0).max(0.0);
                    sim.humans.list[hi].carried_wood -= put;
                    bl.firewood += put;
                }
            }
        }
    }

    // ---------- rot: fresh food spoils by real temperature; smoke slows it; hearths burn ----------
    {
        let mut cell_temps: Vec<(usize, f32)> = Vec::new();
        for (bi, b) in sim.objects.buildings.iter().enumerate() {
            if b.exists && (b.food_store > 0.0 || b.preserved_store > 0.0 || b.firewood > 0.0) {
                cell_temps.push((bi, sim.grid.temp[b.cell as usize]));
            }
        }
        for (bi, t) in cell_temps {
            let b = &mut sim.objects.buildings[bi];
            let rate = if t > 15.0 {
                0.012
            } else if t > 5.0 {
                0.006
            } else if t > 0.0 {
                0.003
            } else {
                0.001
            };
            b.food_store *= 1.0 - rate;
            b.preserved_store *= 1.0 - rate * 0.12;
            // the hearth burns through cold days whether or not anyone watches it
            if t < 5.0 && b.firewood > 0.0 && b.standing() {
                b.firewood = (b.firewood - 0.25).max(0.0);
            }
        }
        for h in sim.humans.list.iter_mut() {
            if h.alive && h.carried_food > 0.0 {
                let t = 10.0; // what you carry rides against your body
                let _ = t;
                h.carried_food *= 1.0 - 0.008;
            }
        }
    }

    // the grass reclaims unwalked paths
    if day % 360 == 77 {
        for w in sim.grid.wear.iter_mut() {
            *w *= 0.55;
        }
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
            remember_spot(h, 2, from.0, from.1, day); // that place is marked in the mind
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
                    near_fresh(sim, h.x, h.y)
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
                near_fresh(sim, h.x, h.y)
            };
            if on_water {
                let h = &mut sim.humans.list[hi];
                h.thirst = 0.0;
                h.days_thirsty = 0;
                h.carried_water = 6.0;
                let (hx2, hy2) = (h.x, h.y);
                remember_spot(h, 1, hx2, hy2, day);
            } else {
                move_toward_h(sim, hi, at, 2);
                let h = &sim.humans.list[hi];
                let (arrived_dry, hx, hy) = {
                    let h = &sim.humans.list[hi];
                    let wet = near_fresh(sim, h.x, h.y);
                    let stalled = (h.x - at.0).abs().max((h.y - at.1).abs()) <= 1;
                    (stalled && !wet, h.x, h.y)
                };
                let _ = (hx, hy);
                if arrived_dry {
                    // the remembered water is gone: forget it and search fresh tomorrow
                    let h = &mut sim.humans.list[hi];
                    if h.water_memory == Some(at) {
                        h.water_memory = None;
                    }
                } else if {
                    let h = &sim.humans.list[hi];
                    near_fresh(sim, h.x, h.y)
                } {
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
                    if ate < need {
                        let take2 = bld.preserved_store.min(need - ate);
                        bld.preserved_store -= take2;
                        ate += take2;
                    }
                }
            }
            if ate < need * 0.5 {
                // the band shares: walk to the nearest stocked hut and eat from its store
                let target = {
                    let h = &sim.humans.list[hi];
                    let mut best: Option<(usize, i32)> = None;
                    for (bi, b) in sim.objects.buildings.iter().enumerate() {
                        if !b.standing() || b.food_store < 0.3 {
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
            h.carried_food = (h.carried_food + got).min(7.0); // a basket holds what it holds
            h.area_yield = h.area_yield * 0.92 + got * 0.08; // the land remembers being picked
            if got > 1.0 {
                remember_spot(h, 0, h.x, h.y, day);
            }
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
                h.carried_wood = (h.carried_wood + biomass.min(6.0)).min(12.0);
                h.skills[SK_WOODCUT] = (h.skills[SK_WOODCUT] + 0.003).min(1.0);
                h.fatigue = (h.fatigue + 0.3).min(1.5);
            }
        }
        HumanAction::Build { kind } => {
            // the village project first: shared walls and shared stores outrank private comfort
            let community = sim.objects.buildings.iter().position(|b| {
                b.exists
                    && b.progress < 1.0
                    && matches!(
                        b.kind,
                        crate::objects::BuildingKind::Granary
                            | crate::objects::BuildingKind::Palisade
                    )
                    && {
                        let h = &sim.humans.list[hi];
                        let (bx, by) = sim.grid.xy(b.cell as usize);
                        (bx - h.x).abs().max((by - h.y).abs()) <= 10
                    }
            });
            if let Some(bi) = community {
                let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
                let at_site = {
                    let h = &sim.humans.list[hi];
                    (h.x - bx).abs().max((h.y - by).abs()) <= 1
                };
                if !at_site {
                    move_toward_h(sim, hi, (bx, by), 2);
                    return;
                }
                // your logs and your labor go into the common work
                let skill = sim.humans.list[hi].skills[SK_BUILD];
                let wood_give = sim.humans.list[hi].carried_wood.min(2.0);
                {
                    let h = &mut sim.humans.list[hi];
                    h.carried_wood -= wood_give;
                    h.fatigue = (h.fatigue + 0.25).min(1.5);
                    h.skills[SK_BUILD] = (h.skills[SK_BUILD] + 0.006).min(1.0);
                }
                let b = &mut sim.objects.buildings[bi];
                b.wood_used += wood_give;
                b.progress = (b.progress
                    + 0.06
                    + skill * 0.06
                    + if wood_give > 0.5 { 0.06 } else { 0.0 })
                .min(1.0);
                if b.progress >= 1.0 {
                    let cell = b.cell;
                    let bid = Objects::building_id(bi);
                    sim.history.push(
                        day,
                        EntityRef::Person(Humans::id_of(hi)),
                        EventKind::BuiltBuilding { building: EntityRef::Building(bid) },
                        Some(cell),
                        vec![],
                    );
                }
                return;
            }
            // is my own house already rising? then today's work goes into it
            let my_site = {
                let h = &sim.humans.list[hi];
                h.home.some().and_then(|b| {
                    let bl = &sim.objects.buildings[b.index()];
                    if bl.exists && bl.progress < 1.0 {
                        Some(b.index())
                    } else {
                        None
                    }
                })
            };
            if let Some(bi) = my_site {
                let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
                let at_site = {
                    let h = &sim.humans.list[hi];
                    (h.x - bx).abs().max((h.y - by).abs()) <= 1
                };
                if !at_site {
                    move_toward_h(sim, hi, (bx, by), 2);
                    return;
                }
                let skill = sim.humans.list[hi].skills[SK_BUILD];
                let b = &mut sim.objects.buildings[bi];
                b.progress = (b.progress + 0.15 + skill * 0.1).min(1.0);
                let done = b.progress >= 1.0;
                let cell = b.cell;
                {
                    let h = &mut sim.humans.list[hi];
                    h.fatigue = (h.fatigue + 0.25).min(1.5);
                    h.skills[SK_BUILD] = (h.skills[SK_BUILD] + 0.006).min(1.0);
                }
                if done {
                    let built_ev = sim.history.push(
                        day,
                        EntityRef::Person(Humans::id_of(hi)),
                        EventKind::BuiltBuilding {
                            building: EntityRef::Building(Objects::building_id(bi)),
                        },
                        Some(cell),
                        vec![],
                    );
                    // the finished house claims its ground: whatever grew here is cleared
                    let pids: Vec<usize> = sim
                        .plants
                        .by_cell
                        .get(cell as usize)
                        .map(|v| v.iter().map(|&x| x as usize).collect())
                        .unwrap_or_default();
                    for pi in pids {
                        crate::vegetation::kill_plant(
                            sim,
                            pi,
                            DeathCause::Felled,
                            vec![Cause::Event(built_ev)],
                        );
                    }
                }
                return;
            }
            // claim a standing empty house before raising a new one
            let empty_house = {
                let h = &sim.humans.list[hi];
                let mut found = None;
                for (bi, b) in sim.objects.buildings.iter().enumerate() {
                    if !b.standing() {
                        continue;
                    }
                    let (bx, by) = sim.grid.xy(b.cell as usize);
                    if (bx - h.x).abs().max((by - h.y).abs()) > 6 {
                        continue;
                    }
                    let bid = Objects::building_id(bi);
                    if !sim.humans.list.iter().any(|o| o.alive && o.home == bid) {
                        found = Some(bi);
                        break;
                    }
                }
                found
            };
            if let Some(bi) = empty_house {
                let bid = Objects::building_id(bi);
                let h = &mut sim.humans.list[hi];
                h.home = bid;
                return;
            }
            // found a new frame with real timber
            let (cell, ok) = {
                let h = &sim.humans.list[hi];
                let cell = sim.grid.idx(h.x, h.y);
                (cell, h.carried_wood >= 8.0)
            };
            let Some(cell) = cell else { return };
            if !ok
                || sim.objects.building_at(cell as u32).is_some()
                || sim.grid.surface[cell] > 0.1
            {
                return;
            }
            let bidx = sim.objects.buildings.len();
            sim.objects.buildings.push(Building {
                kind,
                cell: cell as u32,
                built_day: day,
                builder: Humans::id_of(hi),
                wood_used: 8.0,
                progress: 0.15,
                condition: 1.0,
                food_store: 0.0,
                preserved_store: 0.0,
                firewood: 0.0,
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
            // prefer the village storehouse when one stands nearer than your own roof
            let granary = {
                let h = &sim.humans.list[hi];
                sim.objects
                    .buildings
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| {
                        b.standing()
                            && matches!(b.kind, crate::objects::BuildingKind::Granary)
                    })
                    .map(|(bi, b)| {
                        let (bx, by) = sim.grid.xy(b.cell as usize);
                        (bi, (bx - h.x).abs().max((by - h.y).abs()))
                    })
                    .filter(|(_, d)| *d <= 8)
                    .min_by_key(|(_, d)| *d)
                    .map(|(bi, _)| bi)
            };
            if let Some(bi) = granary {
                let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
                let at_g = {
                    let h = &sim.humans.list[hi];
                    (h.x - bx).abs().max((h.y - by).abs()) <= 1
                };
                if !at_g {
                    move_toward_h(sim, hi, (bx, by), 2);
                    return;
                }
                let amt = {
                    let h = &mut sim.humans.list[hi];
                    let a = (h.carried_food - 0.5).max(0.0);
                    h.carried_food -= a;
                    a
                };
                if amt > 0.0 {
                    sim.objects.buildings[bi].food_store += amt;
                }
                return;
            }
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
        HumanAction::PreserveFood => {
            let Some(b) = sim.humans.list[hi].home.some() else { return };
            let bi = b.index();
            let (bx, by) = sim.grid.xy(sim.objects.buildings[bi].cell as usize);
            let at_home = {
                let h = &sim.humans.list[hi];
                (h.x - bx).abs().max((h.y - by).abs()) <= 1
            };
            if !at_home {
                move_toward_h(sim, hi, (bx, by), 2);
                return;
            }
            let bl = &mut sim.objects.buildings[bi];
            if bl.firewood < 0.3 || bl.food_store <= 0.0 {
                return;
            }
            let batch = bl.food_store.min(2.5);
            bl.food_store -= batch;
            bl.preserved_store += batch * 0.85; // the smoke takes its tithe
            bl.firewood -= 0.3;
            let h = &mut sim.humans.list[hi];
            h.fatigue = (h.fatigue + 0.2).min(1.5);
            h.skills[SK_CRAFT] = (h.skills[SK_CRAFT] + 0.003).min(1.0);
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
    let shared_faith = {
        let a = &sim.humans.list[hi];
        let b = &sim.humans.list[oi];
        a.beliefs.iter().any(|(ba, _)| b.beliefs.iter().any(|(bb, _)| ba == bb))
    };
    let warmth_gain = if shared_faith { 0.14 } else { 0.08 };
    add_rel(sim, hi, oi, RelKind::Friend, warmth_gain);
    add_rel(sim, oi, hi, RelKind::Friend, warmth_gain);
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
    // and so, invisibly, does sickness
    transmit_between(sim, hi, oi);
    transmit_between(sim, oi, hi);
    // what one head knows of the land, another can learn over talk
    {
        let day2 = sim.clock.day;
        let picks: Vec<(u8, i32, i32, u64)> = {
            let h = &mut sim.humans.list[hi];
            if h.known_spots.is_empty() {
                vec![]
            } else {
                let i1 = h.rng.pick_index(h.known_spots.len());
                vec![h.known_spots[i1]]
            }
        };
        for (k, x, y, learned) in picks {
            let o = &mut sim.humans.list[oi];
            remember_spot(o, k, x, y, learned.min(day2)); // secondhand, and already aging
        }
    }
    // and the past is retold — reshaped by the teller and the listener both
    {
        let day2 = sim.clock.day;
        let tale: Option<(MemoryKind, f32)> = {
            let h = &sim.humans.list[hi];
            h.memories
                .iter()
                .filter(|m| {
                    matches!(
                        m.kind,
                        MemoryKind::KinDied { .. }
                            | MemoryKind::FledBeast { .. }
                            | MemoryKind::WentHungry
                    ) && m.weight > 0.6
                })
                .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap())
                .map(|m| (m.kind, m.weight))
        };
        if let Some((kind, w)) = tale {
            let already = sim.humans.list[oi].memories.iter().any(|m| m.kind == kind);
            if !already {
                let (fearful, warmth) = {
                    let o = &sim.humans.list[oi];
                    (o.fear + o.traits[3], o.traits[2])
                };
                // the listener's mind does the distorting: fear amplifies, warmth softens
                let w2 = (w * 0.7 * (1.0 + fearful * 0.3 - warmth * 0.15)).clamp(0.1, 2.5);
                let o = &mut sim.humans.list[oi];
                remember(o, day2, kind, w2);
                // vengeance is contagious: a retold beast becomes the clan's enemy
                if let MemoryKind::FledBeast { beast } = kind {
                    if o.vengeance.is_none() && w2 > 1.2 {
                        o.vengeance = Some(beast);
                    }
                }
            }
        }
    }
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
    // the farm is the ring of land around the farmer's own house
    let Some(home) = sim.humans.list[hi].home.some() else { return };
    let (hx, hy) = sim.grid.xy(sim.objects.buildings[home.index()].cell as usize);
    let at_farm = {
        let h = &sim.humans.list[hi];
        (h.x - hx).abs().max((h.y - hy).abs()) <= 2
    };
    if !at_farm {
        move_toward_h(sim, hi, (hx, hy), 2);
        return;
    }
    let season = day % 360 / 90;
    // 1) harvest any ripe wheat standing on the plot ring — straight into hand, then granary
    let mut harvested = 0.0f32;
    let mut kills: Vec<usize> = Vec::new();
    let mut best_seed: Option<(f32, [f32; 2])> = None;
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            let Some(cell) = sim.grid.idx(hx + dx, hy + dy) else { continue };
            let Some(pids) = sim.plants.by_cell.get(cell) else { continue };
            for &pi in pids {
                let pl = &sim.plants.list[pi as usize];
                if pl.alive && pl.species == crate::species::SP_WHEAT && pl.biomass > 0.30 {
                    harvested += pl.biomass * PLANTS[pl.species as usize].edible;
                    if best_seed.map(|(b, _)| pl.biomass > b).unwrap_or(true) {
                        best_seed = Some((pl.biomass, pl.genes));
                    }
                    kills.push(pi as usize);
                }
            }
        }
    }
    if let Some((_, g)) = best_seed {
        // the farmer keeps seed from the finest heads — that choice IS domestication
        sim.humans.list[hi].seed_genes = Some(g);
    }
    let harvest_ev = if harvested > 0.0 {
        Some(sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::HarvestedFood { amount: harvested },
            sim.grid.idx(hx, hy).map(|i| i as u32),
            vec![],
        ))
    } else {
        None
    };
    for pi in kills {
        crate::vegetation::kill_plant(
            sim,
            pi,
            DeathCause::Felled,
            harvest_ev.map(|e| vec![Cause::Event(e)]).unwrap_or_default(),
        );
    }
    if harvested > 0.0 {
        {
            let h = &mut sim.humans.list[hi];
            h.carried_food += harvested;
            h.skills[SK_FARM] = (h.skills[SK_FARM] + 0.004).min(1.0);
        }
        // the harvest goes to the granary — this is what winter is survived on
        let overflow = {
            let h = &mut sim.humans.list[hi];
            let keep = 1.0f32;
            let over = (h.carried_food - keep).max(0.0);
            h.carried_food -= over;
            over
        };
        sim.objects.buildings[home.index()].food_store += overflow;
        return;
    }
    // 2) sowing season: put seed into the emptiest plot cell (real plants, real land use)
    if season == 0 || season == 1 {
        let mut best: Option<(usize, usize)> = None; // (cell, wheat count)
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                if dx == 0 && dy == 0 {
                    continue; // not under the house
                }
                let Some(cell) = sim.grid.idx(hx + dx, hy + dy) else { continue };
                if sim.grid.ocean[cell] || sim.grid.surface[cell] > 0.05 {
                    continue;
                }
                let wheat = sim
                    .plants
                    .by_cell
                    .get(cell)
                    .map(|v| {
                        v.iter()
                            .filter(|&&pi| {
                                let pl = &sim.plants.list[pi as usize];
                                pl.alive && pl.species == crate::species::SP_WHEAT
                            })
                            .count()
                    })
                    .unwrap_or(0);
                if wheat < 5 && best.map(|(_, w)| wheat < w).unwrap_or(true) {
                    best = Some((cell, wheat));
                }
            }
        }
        if let Some((cell, _)) = best {
            let sow_genes = sim.humans.list[hi].seed_genes;
            for _ in 0..4 {
                sim.plants.spawn_with_genes(
                    sim.cfg.seed,
                    crate::species::SP_WHEAT,
                    cell as u32,
                    day as i64,
                    0.06,
                    sow_genes,
                );
            }
            let h = &mut sim.humans.list[hi];
            h.skills[SK_FARM] = (h.skills[SK_FARM] + 0.002).min(1.0);
            h.fatigue = (h.fatigue + 0.2).min(1.5);
        }
    }
}

/// Contagion along a real human contact: the sick one passes it on.
pub fn transmit_between(sim: &mut Sim, from: usize, to: usize) {
    let day = sim.clock.day;
    let contagious: Vec<u8> = sim.humans.list[from]
        .infections
        .iter()
        .filter(|(pg, since, _)| {
            day.saturating_sub(*since)
                >= crate::pathogens::PATHOGENS[*pg as usize].incubation_d / 2
        })
        .map(|(pg, _, _)| *pg)
        .collect();
    for pg in contagious {
        let already = {
            let t = &sim.humans.list[to];
            t.immune.contains(&pg) || t.infections.iter().any(|(g, _, _)| *g == pg)
        };
        if already {
            continue;
        }
        let p = &crate::pathogens::PATHOGENS[pg as usize];
        let catches = {
            let t = &mut sim.humans.list[to];
            t.rng.chance(p.transmissibility)
        };
        if catches {
            let src_case = sim.humans.list[from]
                .infections
                .iter()
                .find(|(g, _, _)| *g == pg)
                .map(|(_, _, ev)| *ev);
            let mut causes = Vec::new();
            if let Some(e) = src_case {
                causes.push(Cause::Event(e));
            }
            let ev = sim.history.push(
                day,
                EntityRef::Person(Humans::id_of(to)),
                EventKind::CaughtSickness {
                    from: EntityRef::Person(Humans::id_of(from)),
                    pathogen: pg,
                },
                None,
                causes,
            );
            sim.humans.list[to].infections.push((pg, day, ev));
        }
    }
}

/// Zoonosis: butchering or tending an infected beast can pass its sickness to the hand.
pub fn zoonotic_exposure(sim: &mut Sim, hi: usize, ai: usize) {
    let Some((pg, _since)) = sim.animals.list[ai].infection else { return };
    let already = {
        let t = &sim.humans.list[hi];
        t.immune.contains(&pg) || t.infections.iter().any(|(g, _, _)| *g == pg)
    };
    if already {
        return;
    }
    let p = &crate::pathogens::PATHOGENS[pg as usize];
    let catches = {
        let t = &mut sim.humans.list[hi];
        t.rng.chance(p.transmissibility * 2.0)
    };
    if catches {
        let day = sim.clock.day;
        let ev = sim.history.push(
            day,
            EntityRef::Person(Humans::id_of(hi)),
            EventKind::CaughtSickness {
                from: EntityRef::Animal(crate::animals::Animals::id_of(ai)),
                pathogen: pg,
            },
            None,
            vec![],
        );
        sim.humans.list[hi].infections.push((pg, day, ev));
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
    // did a beast do this? the kin will know it by name
    let beast_killer: Option<crate::core::ids::AnimalId> = sim
        .history
        .of_entity(EntityRef::Person(hid))
        .iter()
        .rev()
        .filter_map(|id| sim.history.get(*id))
        .find_map(|ev| match ev.kind {
            EventKind::Killed { by: EntityRef::Animal(a) } => Some(a),
            _ => None,
        });
    for ki in kin {
        if ki < sim.humans.list.len() && sim.humans.list[ki].alive {
            let k = &mut sim.humans.list[ki];
            k.grief = (k.grief + 0.8).min(2.0);
            remember(k, day, MemoryKind::KinDied { who: hid }, 1.4);
            if let Some(b) = beast_killer {
                k.vengeance = Some(b);
                remember(k, day, MemoryKind::FledBeast { beast: b }, 1.6);
            }
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

/// Bring one child into the world from a known mother and father.
fn birth_one(sim: &mut Sim, mi: usize, father: PersonId, x: i32, y: i32, culture: u8) {
    let day = sim.clock.day;
{
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
        {
            let ml = sim.humans.list[mi].lineage;
            sim.humans.list[ci].lineage = ml;
        }
        add_rel(sim, mi, ci, RelKind::ParentOf, 2.0);
        add_rel(sim, ci, mi, RelKind::ChildOf, 2.0);
        {
            let mhome = sim.humans.list[mi].home;
            let c = &mut sim.humans.list[ci];
            c.home = mhome; // born under the mother's roof
        }
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
}
}
