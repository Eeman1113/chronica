//! Terrain generation and geology. Ports the prototype's *concepts* (fbm elevation, radial
//! continent mask — `generateWorld` index.html:533) but places NO rivers, NO lakes, NO biomes:
//! water bodies emerge from rainfall + flow (water module, Test B); biomes are derived
//! observations of actual cover and climate.

use crate::core::rng::{splitmix64, Rng};
use crate::sim::Sim;

/// Seeded value noise with fbm, deterministic, dependency-free.
#[derive(Clone)]
pub struct Noise {
    seed: u64,
}

impl Noise {
    pub fn new(seed: u64) -> Noise {
        Noise { seed }
    }
    #[inline]
    fn lattice(&self, xi: i64, yi: i64, zi: i64) -> f32 {
        let h = splitmix64(
            self.seed
                ^ (xi as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (yi as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ (zi as u64).wrapping_mul(0x1656_67B1_9E37_79F9),
        );
        ((h >> 40) as f32) / ((1u64 << 24) as f32)
    }
    /// Trilinear-interpolated value noise, ~[0,1].
    pub fn at3(&self, x: f32, y: f32, z: f32) -> f32 {
        let (xi, yi, zi) = (x.floor() as i64, y.floor() as i64, z.floor() as i64);
        let (fx, fy, fz) = (x - xi as f32, y - yi as f32, z - zi as f32);
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy, sz) = (s(fx), s(fy), s(fz));
        let mut acc = 0.0;
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    let w = (if dx == 1 { sx } else { 1.0 - sx })
                        * (if dy == 1 { sy } else { 1.0 - sy })
                        * (if dz == 1 { sz } else { 1.0 - sz });
                    acc += w * self.lattice(xi + dx, yi + dy, zi + dz);
                }
            }
        }
        acc
    }
    pub fn at2(&self, x: f32, y: f32) -> f32 {
        self.at3(x, y, 0.0)
    }
    /// Fractal Brownian motion, `oct` octaves, ~[0,1].
    pub fn fbm2(&self, x: f32, y: f32, oct: u32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for _ in 0..oct {
            sum += amp * self.at2(x * freq, y * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
    pub fn fbm3(&self, x: f32, y: f32, z: f32, oct: u32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for _ in 0..oct {
            sum += amp * self.at3(x * freq, y * freq, z * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

/// Generate elevation, regolith, soil, initial groundwater, and fill the ocean basin with real
/// water. Nothing else: streams, lakes, wetlands all come from the water cycle.
pub fn generate(sim: &mut Sim) {
    let master = sim.cfg.seed;
    let w = sim.grid.w;
    let h = sim.grid.h;
    let n_elev = Noise::new(splitmix64(master ^ 0xA1));
    let n_warp = Noise::new(splitmix64(master ^ 0xD4));
    let n_soil = Noise::new(splitmix64(master ^ 0xB2));
    let mut _rng = Rng::domain(master, "world_gen");

    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let rad = cx.min(cy);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            // domain-warped position (prototype's distorted radial mask)
            let wx = x as f32 + (n_warp.at2(x as f32 / 60.0, y as f32 / 60.0) - 0.5) * 46.0;
            let wy = y as f32
                + (n_warp.at2(x as f32 / 60.0 + 99.0, y as f32 / 60.0) - 0.5) * 46.0;
            let d = (((wx - cx) / rad).powi(2) + ((wy - cy) / rad).powi(2)).sqrt();
            let mask = (1.0 - d).clamp(-0.6, 1.0); // continent falls into sea toward edges
            let e = n_elev.fbm2(x as f32 / 48.0, y as f32 / 48.0, 5);
            // elevation in world units; sea level = 0
            let elev = (e - 0.42 + mask * 0.35) * 1.6;
            sim.grid.elev[i] = elev;
            sim.grid.soil_depth[i] = (1.0 - elev.max(0.0) * 0.8).clamp(0.05, 1.0)
                * (0.4 + 0.6 * n_soil.at2(x as f32 / 24.0, y as f32 / 24.0));
            sim.grid.soil_n[i] = 0.4 + 0.4 * n_soil.at2(x as f32 / 16.0 + 50.0, y as f32 / 16.0);
        }
    }

    // Ocean: flood-fill from map edges through below-sea-level cells; fill with real water.
    let n = sim.grid.n();
    let mut ocean = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..w as i32 {
        for &y in &[0i32, h as i32 - 1] {
            let i = (y as u32 * w + x as u32) as usize;
            if sim.grid.elev[i] < 0.0 {
                stack.push(i);
            }
        }
    }
    for y in 0..h as i32 {
        for &x in &[0i32, w as i32 - 1] {
            let i = (y as u32 * w + x as u32) as usize;
            if sim.grid.elev[i] < 0.0 {
                stack.push(i);
            }
        }
    }
    while let Some(i) = stack.pop() {
        if ocean[i] {
            continue;
        }
        ocean[i] = true;
        let (x, y) = sim.grid.xy(i);
        for (dx, dy) in crate::world::grid::DX4.iter().zip(crate::world::grid::DY4.iter()) {
            if let Some(j) = sim.grid.idx(x + dx, y + dy) {
                if !ocean[j] && sim.grid.elev[j] < 0.0 {
                    stack.push(j);
                }
            }
        }
    }
    for i in 0..n {
        sim.grid.ocean[i] = ocean[i];
        if ocean[i] {
            sim.grid.surface[i] = -sim.grid.elev[i]; // the sea is standing water, depth = basin depth
        }
        // initial groundwater: a modest store everywhere on land
        if !ocean[i] {
            sim.grid.ground[i] = ground_capacity(&sim.grid, i) * 0.35;
        }
    }

    // Drainage: geologic time has already carved valleys — fill *shallow* noise depressions via
    // priority flood so land generally drains to the sea, while depressions deeper than
    // DEEP_BASIN are kept: they are real basins where lakes can form by actually filling with
    // water and spilling (still never "placed" as lakes).
    const DEEP_BASIN: f32 = 0.30;
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    #[inline]
    fn key(v: f32) -> u32 {
        (v + 16.0).to_bits() // positive-shifted f32 bits are order-preserving
    }
    let mut heap: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
    let mut visited = vec![false; n];
    for i in 0..n {
        let (x, y) = sim.grid.xy(i);
        let edge = x == 0 || y == 0 || x == w as i32 - 1 || y == h as i32 - 1;
        if ocean[i] || edge {
            visited[i] = true;
            heap.push(Reverse((key(sim.grid.elev[i]), i as u32)));
        }
    }
    let mut eps_count: u64 = 0;
    while let Some(Reverse((k, iu))) = heap.pop() {
        let i = iu as usize;
        let level = f32::from_bits(k) - 16.0;
        let (x, y) = sim.grid.xy(i);
        for kk in 0..8 {
            let (nx, ny) = (
                x + crate::world::grid::DX8[kk],
                y + crate::world::grid::DY8[kk],
            );
            if let Some(j) = sim.grid.idx(nx, ny) {
                if visited[j] || ocean[j] {
                    continue;
                }
                visited[j] = true;
                let spill = level.max(sim.grid.elev[j]);
                let fill = spill - sim.grid.elev[j];
                if fill > 0.0 && fill < DEEP_BASIN {
                    eps_count += 1;
                    sim.grid.elev[j] = spill + eps_count as f32 * 1e-5;
                }
                heap.push(Reverse((key(sim.grid.elev[j].max(spill)), j as u32)));
            }
        }
    }
}

/// Groundwater capacity of a cell — a function of its real soil depth.
#[inline]
pub fn ground_capacity(grid: &crate::world::Grid, i: usize) -> f32 {
    0.2 + grid.soil_depth[i] * 0.8
}
