//! Deterministic parallelism helpers (docs/DETERMINISM.md "Parallelism model").
//!
//! The only sanctioned pattern: map over an index range in parallel producing per-item results
//! (each item touching only its own state + read-only shared state), then apply the collected
//! results **in index order** on one thread. Order of effects is therefore independent of thread
//! count. Rayon preserves output ordering for indexed collects, which is what we rely on.

use rayon::prelude::*;

/// Parallel map preserving index order. `f` must not mutate shared state.
pub fn par_map<T, F>(n: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync + Send,
{
    (0..n).into_par_iter().map(f).collect()
}

/// Parallel map over chunks of a slice, preserving order; returns one result per chunk.
pub fn par_map_chunks<'a, S, T, F>(items: &'a [S], chunk: usize, f: F) -> Vec<T>
where
    S: Sync,
    T: Send,
    F: Fn(usize, &'a [S]) -> T + Sync + Send,
{
    items
        .par_chunks(chunk)
        .enumerate()
        .map(|(i, c)| f(i, c))
        .collect()
}
