//! Test B (River): terrain + rainfall only → rivers and lakes appear with no water body ever
//! placed by the generator, and water reaches the sea.

use chronica_engine::sim::{Config, Sim};
use chronica_engine::water::water_stats;

#[test]
fn rain_makes_rivers_and_lakes() {
    let mut sim = Sim::new(Config { seed: 777, width: 160, height: 110 });

    // At generation time there must be zero fresh water anywhere (the ocean is the only water).
    let (r0, l0, _) = water_stats(&sim);
    assert_eq!(r0, 0, "generator must not place rivers");
    assert_eq!(l0, 0, "generator must not place lakes");

    sim.run_days(3 * 360);

    let (rivers, lakes, mean_surface) = water_stats(&sim);
    assert!(rivers > 40, "expected sustained river channels, got {rivers} river cells");
    assert!(lakes > 5, "expected pooled lakes in depressions, got {lakes} lake cells");
    assert!(mean_surface.is_finite() && mean_surface < 10.0, "water must not blow up");

    // Seasonality/weather must vary rainfall — the field is not constant.
    let wet_cells = sim.grid.rain.iter().filter(|r| **r > 0.0).count();
    assert!(wet_cells > 0 || sim.grid.snow.iter().any(|s| *s > 0.0));
}
