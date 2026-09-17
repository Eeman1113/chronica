//! Pathogens as entities-in-hosts (the mandated §6.9 replacement for scheduled plagues).
//! A sickness ORIGINATES only where its causes are: dense animal crowds (zoonosis) — cited as
//! state. It moves only along real contacts: butchering a kill, tending a tamed beast, sharing a
//! conversation or a roof. Immunity is individually remembered. Every case cites its source.

pub struct Pathogen {
    pub name: &'static str,
    pub incubation_d: u64,
    pub illness_d: u64,
    pub daily_lethality: f32, // scaled by (1 - health)
    pub transmissibility: f32,
}

pub const PATHOGENS: &[Pathogen] = &[
    Pathogen { name: "marsh fever", incubation_d: 4, illness_d: 18, daily_lethality: 0.012, transmissibility: 0.10 },
    Pathogen { name: "murrain", incubation_d: 6, illness_d: 24, daily_lethality: 0.020, transmissibility: 0.07 },
];
