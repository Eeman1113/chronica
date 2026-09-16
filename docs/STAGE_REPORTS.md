# STAGE_REPORTS.md — reports at each stage gate (directive §7)

Autonomous overnight run authorized by the user 2026-09-16 ("go fully without halt, no need to
ask, finish all phases"). Reports are recorded here instead of stopping at each gate.

---

## STAGE 1 — Core architecture

**Done:** Cargo workspace (`engine` + `tools/{sim_runner,world_inspector,replay}`); `core/ids`
(typed 64-bit never-reused IDs, `EntityRef`), `core/rng` (counter-based per-entity streams —
draws are pure functions of (master, domain, id, counter), so entity randomness is independent
of update order), `core/clock` (day tick, 360-day calendar, D-007), `core/jobs` (deterministic
parallel map: plan in parallel from snapshot, apply in index order), `world/grid` (SoA cells,
every field addressable), `world/spatial` (uniform-bucket index, deterministic visit order),
`history` (append-only causal event store, consequences as derived reverse index, never capped),
`persistence` (binary versioned complete save; skipped indices rebuilt on load, D-009),
`sim` (fixed phase order), `sim_runner` CLI (`--seed/--days/--save/--load/--threads/--hash`),
`replay verify|diff`, `world_inspector` (events + why-chains).

**Verified by:** `cargo test --workspace --release` — `rerun_is_byte_identical`,
`different_seeds_differ`, `save_load_continue_equals_uninterrupted`, `thread_count_invariance`
(1 vs 8 threads), RNG stream tests: **all pass**. `replay verify` on a 720-day save: OK.

**Rules audit:** serde HashMap ordering would have silently broken byte-identity — banned from
serialized state (D-009). No global RNG exists; `Rng::chance` documents the causality bar at the
call-site level.

**Decisions:** D-007 (day tick), D-008 (single engine crate + modules), D-009 (no HashMap in
saved state).

**Next:** Stage 2 completion; then biology.

---

## STAGE 2 — Physical world

**Done:** `terrain` (fbm + domain-warped continent mask ported conceptually from prototype
`generateWorld`; ocean = edge-connected sub-sea-level basin filled with real standing water; NO
rivers/lakes/biomes placed); `climate` (per-cell temperature by latitude/season/altitude, moving
rainfall fronts via 3D noise over space+time, snowpack accumulation and melt — replaces the
prototype's single global weather/drought singletons); `water` (rain → surface volumes →
head-driven downhill flow (parallel plan from snapshot, ordered apply) → infiltration →
groundwater → springs; evaporation/evapotranspiration; erosion moving real sediment; **rivers
and lakes are derived observations** `is_river`/`is_lake` over surface+flow state).

**Verified by:** Emergence Test B (`emergence_b_river.rs`): at generation, 0 river cells and 0
lake cells exist; after 3 simulated years, sustained channels (>40 river cells) and pooled lakes
(>5 cells) exist; water balance is bounded. Determinism suite still green with the water phase
running in parallel. Throughput: 820 days/s at 192×128 (dev-scale) in release.

**Rules audit:** first water balance blew up (rain ≫ sinks) — the emergence test caught it; fixed
by physical rebalance, not by clamping water or special-casing. Fire/burn state fields exist on
the grid, but ignition+spread land in Stage 3 *with real fuel* (individual plants + litter) —
implementing fire before fuel exists would have meant burning a biome label, which is exactly the
aggregate the rules forbid.

**Open questions:** hydrology wetness (lake fraction grows through year 2) — revisit when
vegetation transpiration draws on groundwater in Stage 3.

**Next:** Stage 3 — individual plants (Test A), individual animals, food web, fire on real fuel.
