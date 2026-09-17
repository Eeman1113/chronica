//! Climate: regional temperature and rainfall fields, seasons, snow. Replaces the prototype's
//! single global `sim.weather` + `sim.drought` booleans (AUDIT §6.12) with per-cell state driven
//! by deterministic moving weather fields. Drought is not a coin flip — it is what a dry stretch
//! of the rain field actually does to soil and rivers.

use crate::core::clock::DAYS_PER_YEAR;
use crate::sim::Sim;
use crate::terrain::Noise;

pub struct ClimateParams {
    pub equator_temp: f32,
    pub pole_drop: f32,
    pub seasonal_amp: f32,
    pub lapse_per_elev: f32,
    pub base_rain: f32,
}

impl Default for ClimateParams {
    fn default() -> Self {
        ClimateParams {
            equator_temp: 24.0,
            pole_drop: 30.0,
            seasonal_amp: 9.0,
            lapse_per_elev: 14.0,
            base_rain: 0.035,
        }
    }
}

/// Advance climate one day: write `temp`, `rain`, update `snow` (accumulate/melt).
pub fn tick(sim: &mut Sim) {
    let p = ClimateParams::default();
    let day = sim.clock.day;
    let phase = (day % DAYS_PER_YEAR) as f32 / DAYS_PER_YEAR as f32;
    // Seasonal curve: peak mid-summer (phase 0.375 in a spring-start calendar).
    let season = (std::f32::consts::TAU * (phase - 0.125)).sin();
    let n_weather = Noise::new(crate::core::rng::splitmix64(sim.cfg.seed ^ 0xC71A_7E00));
    let w = sim.grid.w as usize;
    let h = sim.grid.h as usize;
    let t_day = day as f32;

    for y in 0..h {
        // latitude: 0 at equator row (middle), 1 at poles (top/bottom edges)
        let lat = ((y as f32 / h as f32) - 0.5).abs() * 2.0;
        for x in 0..w {
            let i = y * w + x;
            let elev = sim.grid.elev[i].max(0.0);
            let temp = p.equator_temp - lat * p.pole_drop + season * p.seasonal_amp
                - elev * p.lapse_per_elev
                + (n_weather.at3(x as f32 / 40.0, y as f32 / 40.0, t_day / 15.0) - 0.5) * 6.0;
            sim.grid.temp[i] = temp;

            // Rainfall: moving humid fronts (3D noise over space+time), wetter near the sea
            // band and at altitude fronts. A dry stretch of this field IS a drought.
            let front = n_weather.fbm3(x as f32 / 28.0, y as f32 / 28.0, t_day / 11.0, 3);
            let humidity = 1.0 - lat * 0.35;
            let rain_amt = if front > 0.56 {
                (front - 0.56) * p.base_rain * humidity * 2.2
            } else {
                0.0
            };
            if sim.grid.ocean[i] {
                sim.grid.rain[i] = 0.0;
                continue;
            }
            if temp <= 0.0 {
                // snowfall accumulates as pack; no liquid rain
                sim.grid.snow[i] += rain_amt;
                sim.grid.rain[i] = 0.0;
            } else {
                sim.grid.rain[i] = rain_amt;
                // snowmelt: proportional to warmth, feeds the surface (spring floods)
                if sim.grid.snow[i] > 0.0 {
                    let melt = (temp * 0.004).min(sim.grid.snow[i]);
                    sim.grid.snow[i] -= melt;
                    sim.grid.rain[i] += melt;
                }
            }
        }
    }
}
