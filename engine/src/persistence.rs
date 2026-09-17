//! Binary, versioned save/load (docs/SAVE_FORMAT.md). The save is the *complete* state — loading
//! resumes bit-identically (verified by tests/determinism). No JSON, no pruning, no quantization.

use crate::sim::Sim;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

pub const MAGIC: u32 = 0x43524f4e; // "CRON"
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SaveFile {
    magic: u32,
    schema: u32,
    sim: Sim,
}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Codec(String),
    BadMagic,
    BadVersion(u32),
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

pub fn save_to_file(sim: &Sim, path: &Path) -> Result<(), SaveError> {
    let f = SaveFile { magic: MAGIC, schema: SCHEMA_VERSION, sim: sim.clone() };
    let bytes = bincode::serialize(&f).map_err(|e| SaveError::Codec(e.to_string()))?;
    let mut out = std::fs::File::create(path)?;
    out.write_all(&bytes)?;
    Ok(())
}

pub fn load_from_file(path: &Path) -> Result<Sim, SaveError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    let f: SaveFile =
        bincode::deserialize(&bytes).map_err(|e| SaveError::Codec(e.to_string()))?;
    if f.magic != MAGIC {
        return Err(SaveError::BadMagic);
    }
    if f.schema != SCHEMA_VERSION {
        return Err(SaveError::BadVersion(f.schema));
    }
    let mut sim = f.sim;
    rebuild_after_load(&mut sim);
    Ok(sim)
}

/// Rebuild every #[serde(skip)] derived index (DECISIONS D-009).
pub fn rebuild_after_load(sim: &mut Sim) {
    sim.grid.rebuild_after_load();
    sim.history.rebuild_after_load();
    sim.plants.rebuild_cell_aggregates(sim.grid.n());
    crate::vegetation::refresh_cover(sim);
    sim.animals.rebuild_index(sim.grid.w, sim.grid.h);
}
