//! Simulation clock. Base tick = 1 day (DECISIONS D-007); calendar matches the prototype:
//! 360-day year, four 90-day seasons, twelve 30-day months. The clock is the only time source
//! in engine code — wall time never appears.

use serde::{Deserialize, Serialize};

pub const DAYS_PER_YEAR: u64 = 360;
pub const DAYS_PER_SEASON: u64 = 90;
pub const DAYS_PER_MONTH: u64 = 30;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Clock {
    pub day: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Clock {
    #[inline]
    pub fn year(&self) -> u64 {
        self.day / DAYS_PER_YEAR
    }
    #[inline]
    pub fn day_of_year(&self) -> u64 {
        self.day % DAYS_PER_YEAR
    }
    #[inline]
    pub fn season(&self) -> Season {
        match self.day_of_year() / DAYS_PER_SEASON {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
    /// 0..1 phase through the year, for smooth seasonal curves.
    #[inline]
    pub fn year_phase(&self) -> f32 {
        self.day_of_year() as f32 / DAYS_PER_YEAR as f32
    }
    #[inline]
    pub fn month(&self) -> u64 {
        self.day_of_year() / DAYS_PER_MONTH
    }
    pub fn date_string(&self) -> String {
        let s = match self.season() {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        };
        format!("Year {}, {} (day {})", self.year(), s, self.day_of_year())
    }
}
