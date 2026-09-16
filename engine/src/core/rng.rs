//! Deterministic counter-based RNG streams (docs/DETERMINISM.md).
//!
//! Every stream is keyed by (master seed, domain tag, entity id) and owns a draw counter that is
//! part of saved state. A draw is a pure function of (key, counter), so entity A's randomness
//! never depends on whether entity B drew first — the property that makes deterministic
//! parallelism possible. No global RNG exists anywhere in the engine.

use serde::{Deserialize, Serialize};

#[inline]
pub fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline]
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rng {
    key: u64,
    ctr: u64,
}

impl Rng {
    /// Named stream for a whole domain (e.g. "climate").
    pub fn domain(master: u64, domain: &str) -> Rng {
        Rng { key: splitmix64(master ^ fnv1a(domain)), ctr: 0 }
    }
    /// Per-entity stream: domain plus the entity's permanent id.
    pub fn entity(master: u64, domain: &str, id: u64) -> Rng {
        Rng { key: splitmix64(master ^ fnv1a(domain) ^ splitmix64(id)), ctr: 0 }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.ctr = self.ctr.wrapping_add(1);
        splitmix64(self.key ^ self.ctr.wrapping_mul(0xA076_1D64_78BD_642F))
    }
    /// Uniform in [0,1).
    #[inline]
    pub fn f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }
    #[inline]
    pub fn f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) / ((1u64 << 53) as f64)
    }
    /// Uniform integer in [lo, hi] inclusive.
    #[inline]
    pub fn range_i(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(hi >= lo);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }
    #[inline]
    pub fn range_f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }
    /// Bernoulli draw. Callers must satisfy the "causality bar" (docs/SIMULATION_MODEL.md):
    /// probability may only perturb the outcome of a situation whose reasons exist in state.
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    #[inline]
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> Option<&'a T> {
        if xs.is_empty() { None } else { Some(&xs[(self.next_u64() % xs.len() as u64) as usize]) }
    }
    #[inline]
    pub fn pick_index(&mut self, len: usize) -> usize {
        debug_assert!(len > 0);
        (self.next_u64() % len as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streams_are_independent_and_reproducible() {
        let mut a1 = Rng::entity(42, "animals", 7);
        let mut b = Rng::entity(42, "animals", 8);
        let s1: Vec<u64> = (0..8).map(|_| a1.next_u64()).collect();
        let _junk: Vec<u64> = (0..1000).map(|_| b.next_u64()).collect();
        let mut a2 = Rng::entity(42, "animals", 7);
        let s2: Vec<u64> = (0..8).map(|_| a2.next_u64()).collect();
        assert_eq!(s1, s2, "stream unaffected by other streams and reproducible");
    }
    #[test]
    fn uniformity_smoke() {
        let mut r = Rng::domain(1, "t");
        let mean: f64 = (0..10_000).map(|_| r.f64()).sum::<f64>() / 10_000.0;
        assert!((mean - 0.5).abs() < 0.02);
    }
}
