# PERFORMANCE.md — performance philosophy, budgets, and (eventually) measured numbers

> Stage 0 status: philosophy and budgets only. Measured numbers land in Stage 8; interim
> measurements are appended per stage as systems come online.

## The one rule

**Optimize the engine, never the truth** (§2.12 of the rebuild directive). Allowed levers: memory
layout, cache locality, SoA/columnar storage, archetype grouping, stable handles, spatial indexing,
batching, SIMD, job scheduling, memory pools, compact IDs, compression, hierarchical pathfinding,
flow fields, cached paths, event-storage design, tiered persistence, and *evaluation cadence*
(when an entity is re-evaluated, chosen deterministically). Forbidden levers: fewer entities,
less state, simpler rules for "unimportant"/distant/unobserved entities, pruning history,
inventing off-screen outcomes.

The prototype demonstrates the forbidden levers precisely (see AUDIT.md §F): it stops moving
people past 1,800 population (`index.html:4001`), prunes souls past 22,000 (`:4133`), caps events
at 5,200 (`:499`), caps biographies (`:529`) and memories (`:2875`). Every one of those is a
performance decision expressed as a fidelity decision. The engine expresses the same budget
pressure as layout/scheduling decisions instead.

## Cadence is the legitimate lever

`§2.10`: systems register at tick/hour/day/season/year resolution, and an individual entity may be
evaluated less often **when nothing about its situation changed** — dormancy is "unchanged and
fully persisted", never "collapsed into a statistic". Examples of legitimate cadence:

- A tree with stable water/light/nutrient inputs re-evaluates seasonally, not daily; any change to
  its cell (fire, drought crossing a threshold, felling, herbivory) dirties it back to fine cadence.
- A sleeping person ticks at hour granularity; a person mid-conversation or mid-combat at full tick.
- Chunk-level dirty tracking gates water/erosion/vegetation passes: a chunk with no state change
  and no incoming flux is skipped *as computation* while remaining fully resident *as state*.

Cadence assignment must itself be deterministic (a function of state, never of wall-clock or load).

## Budgets (targets to design against, not promises)

| Scale | Humans | Animals | Plants | Cells | Target |
|---|---|---|---|---|---|
| Dev world | 1k | 5k | 100k | 1M | real-time ×100+ on a laptop |
| Reference | 10k | 50k | 1M | 4M | real-time ×20+ |
| Stress | 100k | 500k | 10M | 16M | real-time ×1 or better |
| Ceiling probe | 1M | — | — | — | measured, documented, not promised |

Memory envelope: hot per-entity data is columnar; budget ≈ low hundreds of bytes of hot state per
human, tens per animal, ≤16 per plant, ≤64 per cell, with cold data (memories, relationships,
biography back-references) in side tables and history tiered to disk. History storage grows
without bound by design — the answer is compact encoding + tiering (see SAVE_FORMAT.md), never caps.

## What gets measured (Stage 8, plus per-stage spot checks)

- ticks/sec at each scale row; wall-clock per sim-year
- thread scaling 1/2/4/8/16 (with determinism cross-check at each count)
- RAM by subsystem (columnar stores, side tables, spatial index, event store, path caches)
- allocations/tick (target: zero steady-state allocation in hot loops)
- cache miss rates on the hot loops (perf/instruments)
- save size, save/load time, incremental-save delta size
- event-store growth per sim-year at each scale
- pathfinding: requests/sec, cache hit rate, invalidation storms under terrain change
- brain cost: decisions/sec, and the tail (worst single entity per tick)

All numbers are recorded here with hardware, seed, config, and engine version, so regressions are
attributable. A benchmark harness lives in `tools/profiler` and runs headless via `sim_runner`.

## Optimization discipline

1. Measure before optimizing; keep the profile in the PR.
2. An optimization PR may not change: entity counts, entity state schema, decision rules, event
   emission, or any test-world outcome hash. Emergence tests + determinism tests are the guard.
3. Prefer making the common case cheap (layout, batching) over making the rare case absent
   (which is usually a truth violation in disguise).
