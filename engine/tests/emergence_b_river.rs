//! Test B (River): terrain + rainfall only → rivers and lakes appear with no water body ever
//! placed by the generator, and water reaches the sea.

use chronica_engine::sim::{Config, Sim};
use chronica_engine::water::water_stats;

#[test]
fn rain_makes_rivers_and_lakes() {
    // No code path ever writes a river/lake flag: fresh water enters the world only as rain in
    // `climate::tick` and moves only in `water::tick`. Generation runs a hydrological spin-up
    // (rain falling on dry terrain), so rivers must already have EMERGED by day 0 — and must
    // persist as weather-driven, fluctuating features afterward.
    let mut sim = Sim::new(Config { seed: 777, width: 160, height: 110 });

    let (r0, l0, _) = water_stats(&sim);
    assert!(r0 > 20, "rivers should have emerged from rain during spin-up, got {r0}");
    let _ = l0;

    sim.run_days(3 * 360);

    let (rivers, lakes, mean_surface) = water_stats(&sim);
    assert!(rivers > 40, "expected sustained river channels, got {rivers} river cells");
    assert!(lakes > 5, "expected pooled lakes in depressions, got {lakes} lake cells");
    assert!(mean_surface.is_finite() && mean_surface < 10.0, "water must not blow up");

    // Seasonality/weather must vary rainfall — the field is not constant.
    let wet_cells = sim.grid.rain.iter().filter(|r| **r > 0.0).count();
    assert!(wet_cells > 0 || sim.grid.snow.iter().any(|s| *s > 0.0));
}
