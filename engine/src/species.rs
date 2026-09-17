//! Species parameter tables (plants and animals). Parameters, not behavior: all behavior lives
//! in vegetation/animals/brains and is identical for every individual (directive §2.10).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlantKind {
    Grass,
    Shrub,
    Tree,
    Reed,
    Crop,
}

pub struct PlantSpecies {
    pub name: &'static str,
    pub kind: PlantKind,
    pub t_opt: f32,      // optimal temperature
    pub t_tol: f32,      // temperature tolerance width
    pub m_opt: f32,      // optimal soil-moisture fraction 0..1
    pub m_tol: f32,
    pub growth: f32,     // relative daily growth rate
    pub max_biomass: f32,
    pub fire_res: f32,   // 0..1
    pub shade_power: f32,// how much light it takes from shorter plants
    pub shade_tol: f32,  // how much shading it tolerates
    pub seed_range: i32, // dispersal radius (cells)
    pub maturity: f32,   // biomass fraction at which it can seed
    pub max_age_y: f32,  // typical lifespan in years (per-individual draw around this)
    pub edible: f32,     // forage value per biomass for herbivores
}

pub const PLANTS: &[PlantSpecies] = &[
    PlantSpecies { name: "Meadow Grass", kind: PlantKind::Grass, t_opt: 15.0, t_tol: 18.0, m_opt: 0.45, m_tol: 0.40, growth: 0.22, max_biomass: 0.35, fire_res: 0.05, shade_power: 0.05, shade_tol: 0.35, seed_range: 2, maturity: 0.35, max_age_y: 6.0, edible: 1.0 },
    PlantSpecies { name: "Dry Scrub", kind: PlantKind::Shrub, t_opt: 21.0, t_tol: 14.0, m_opt: 0.25, m_tol: 0.25, growth: 0.035, max_biomass: 0.8, fire_res: 0.10, shade_power: 0.20, shade_tol: 0.30, seed_range: 3, maturity: 0.40, max_age_y: 25.0, edible: 0.35 },
    PlantSpecies { name: "Broadleaf Oak", kind: PlantKind::Tree, t_opt: 13.0, t_tol: 12.0, m_opt: 0.55, m_tol: 0.32, growth: 0.016, max_biomass: 6.0, fire_res: 0.25, shade_power: 0.9, shade_tol: 0.25, seed_range: 4, maturity: 0.30, max_age_y: 180.0, edible: 0.05 },
    PlantSpecies { name: "Northern Pine", kind: PlantKind::Tree, t_opt: 4.0, t_tol: 14.0, m_opt: 0.45, m_tol: 0.32, growth: 0.013, max_biomass: 5.0, fire_res: 0.15, shade_power: 0.8, shade_tol: 0.45, seed_range: 4, maturity: 0.30, max_age_y: 220.0, edible: 0.03 },
    PlantSpecies { name: "River Reed", kind: PlantKind::Reed, t_opt: 17.0, t_tol: 14.0, m_opt: 0.9, m_tol: 0.30, growth: 0.10, max_biomass: 0.5, fire_res: 0.05, shade_power: 0.1, shade_tol: 0.5, seed_range: 2, maturity: 0.4, max_age_y: 8.0, edible: 0.6 },
    PlantSpecies { name: "Emmer Wheat", kind: PlantKind::Crop, t_opt: 18.0, t_tol: 10.0, m_opt: 0.5, m_tol: 0.25, growth: 0.12, max_biomass: 0.6, fire_res: 0.02, shade_power: 0.05, shade_tol: 0.2, seed_range: 1, maturity: 0.6, max_age_y: 1.0, edible: 1.4 },
];

pub const SP_GRASS: u8 = 0;
pub const SP_SCRUB: u8 = 1;
pub const SP_OAK: u8 = 2;
pub const SP_PINE: u8 = 3;
pub const SP_REED: u8 = 4;
pub const SP_WHEAT: u8 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Diet {
    Herbivore,
    Carnivore,
    Omnivore,
}

pub struct AnimalSpecies {
    pub name: &'static str,
    pub diet: Diet,
    pub mass: f32,          // adult mass (meat value, fight weight)
    pub speed: i32,         // cells per day at full burst
    pub perception: i32,    // sense radius (cells)
    pub lifespan_y: f32,    // mean lifespan; individual value drawn at birth
    pub maturity_y: f32,
    pub gestation_d: u64,
    pub litter: (i64, i64),
    pub hunger_rate: f32,   // per day
    pub herd: f32,          // 0..1 herding tendency baseline
    pub prey: &'static [u8],// indices into ANIMALS this species hunts
    pub domesticable: bool,
    pub t_min: f32,         // habitable temperature band (annual mean)
    pub t_max: f32,
}

pub const ANIMALS: &[AnimalSpecies] = &[
    AnimalSpecies { name: "Hare", diet: Diet::Herbivore, mass: 3.0, speed: 4, perception: 5, lifespan_y: 6.0, maturity_y: 0.6, gestation_d: 32, litter: (2, 5), hunger_rate: 0.30, herd: 0.1, prey: &[], domesticable: false, t_min: -12.0, t_max: 30.0 },
    AnimalSpecies { name: "Deer", diet: Diet::Herbivore, mass: 60.0, speed: 5, perception: 7, lifespan_y: 14.0, maturity_y: 1.5, gestation_d: 200, litter: (1, 2), hunger_rate: 0.16, herd: 0.7, prey: &[], domesticable: false, t_min: -15.0, t_max: 28.0 },
    AnimalSpecies { name: "Boar", diet: Diet::Omnivore, mass: 80.0, speed: 4, perception: 6, lifespan_y: 12.0, maturity_y: 1.2, gestation_d: 115, litter: (2, 6), hunger_rate: 0.18, herd: 0.4, prey: &[], domesticable: true, t_min: -8.0, t_max: 30.0 },
    AnimalSpecies { name: "Wolf", diet: Diet::Carnivore, mass: 40.0, speed: 6, perception: 9, lifespan_y: 10.0, maturity_y: 1.8, gestation_d: 63, litter: (2, 5), hunger_rate: 0.14, herd: 0.6, prey: &[0, 1, 2, 5], domesticable: false, t_min: -25.0, t_max: 26.0 },
    AnimalSpecies { name: "Bear", diet: Diet::Omnivore, mass: 220.0, speed: 4, perception: 8, lifespan_y: 22.0, maturity_y: 3.5, gestation_d: 220, litter: (1, 3), hunger_rate: 0.12, herd: 0.0, prey: &[0, 1, 2, 5], domesticable: false, t_min: -20.0, t_max: 24.0 },
    AnimalSpecies { name: "Wild Sheep", diet: Diet::Herbivore, mass: 45.0, speed: 4, perception: 6, lifespan_y: 11.0, maturity_y: 1.0, gestation_d: 150, litter: (1, 3), hunger_rate: 0.17, herd: 0.8, prey: &[], domesticable: true, t_min: -18.0, t_max: 26.0 },
    AnimalSpecies { name: "Wild Horse", diet: Diet::Herbivore, mass: 300.0, speed: 8, perception: 8, lifespan_y: 24.0, maturity_y: 3.0, gestation_d: 340, litter: (1, 1), hunger_rate: 0.15, herd: 0.7, prey: &[], domesticable: true, t_min: -20.0, t_max: 28.0 },
    AnimalSpecies { name: "Aurochs", diet: Diet::Herbivore, mass: 500.0, speed: 4, perception: 6, lifespan_y: 18.0, maturity_y: 2.5, gestation_d: 280, litter: (1, 1), hunger_rate: 0.20, herd: 0.7, prey: &[], domesticable: true, t_min: -15.0, t_max: 28.0 },
];

pub const A_HARE: u8 = 0;
pub const A_DEER: u8 = 1;
pub const A_BOAR: u8 = 2;
pub const A_WOLF: u8 = 3;
pub const A_BEAR: u8 = 4;
pub const A_SHEEP: u8 = 5;
pub const A_HORSE: u8 = 6;
pub const A_AUROCHS: u8 = 7;
