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

---

## STAGE 3 — Biology

**Done:** `species` (parameter tables only — behavior is shared); `vegetation` — every plant an
individual entity (own RNG stream, per-individual lifespan drawn at germination, growth from its
cell's real water/light/nutrients, transpiration feeding back into the aquifer, light competition
in-cell, seeding into real neighbor cells gated by germination + first-winter survivability,
perennial dieback-dormancy, death always with cause + causal StateRef/Event); `fire` — lightning
in real storm cells, ignition/spread on real fuel (litter + standing biomass), burned plants die
with events caused by the fire, rain extinguishes, no spread cap; `animals` — every animal an
individual (needs, condition, per-individual lifespan, genes inherited with mutation, generation
tracking, water memory), perception strictly through the spatial index, shared utility brain with
**recorded rationale** (`brains::decide` — same architecture humans will extend), pursuit
predation (kills require actually closing distance; dice only resolve the struggle), courtship on
co-location, gestation with recorded sire, corpses that rot/are scavenged/fertilize soil.
Water/climate refined: head-driven groundwater drainage (baseflow rivers), bank recharge, spring
venting, shallow-depression drainage at gen (deep basins remain honest lake basins), 3 flow
sub-steps/day.

**Verified by:** Test A (`emergence_a_forest`): ≥10k trees, stat == count(entities), felling 60%
drops count exactly and cover proportionally, every plant death has a cause event — pass.
Predator/prey (`emergence_predator_prey`): >100 real births, real kills, every animal death
recorded, predation deaths chain to their kill events — pass. Test B + full determinism suite
still green. 40-year world: plants ~220k stable, trees ~15k stable, cover ~0.67, rivers/lakes
persistent, 7.6k animal births vs 8k deaths (cycling populations).

**Rules audit:** Ecology was tuned exclusively through physical/biological parameters (rain scale,
evapotranspiration, aquifer physics, trophic densities, forage behavior) — never by exempting
entities, faking counts, or scripting outcomes. Every mass die-off during tuning was diagnosed
via the event log's cause distribution — the causality system already earns its keep.
The directive's Stage 3 bar is met; long-run (>50y) megafauna persistence continues to be
observed and tuned as later stages add humans (hunting pressure changes the balance anyway).

**Decisions:** none new (parameters, not architecture).

**Next:** Stage 4 — humans on the same brain architecture, with memory, relationships, knowledge,
items; Test E.
