# ARCHITECTURE.md — target engine architecture

> Stage 0 status: structure and contracts. Refined as stages land. Language: see DECISIONS D-001
> (Rust vs C++ escalated at the Stage 0 gate); everything below is valid in either.

## Principle

One simulation mode. Everything individually represented, persistently simulated, causally
connected. Derived statistics are observations. The renderer owns no state. Deterministic under
threads and save/load. Optimize layout and scheduling, never truth. (Full rules: rebuild
directive §2; condensed in CLAUDE.md.)

## Workspace layout

```
chronica/
  crates|modules/
    core/          typed 64-bit IDs (never reused), generational handles, deterministic RNG
                   streams (DETERMINISM.md), sim clock, phase scheduler, job system, error types
    world/         chunked grid, per-cell state, chunk dirty tracking, spatial index, regions
    terrain/       elevation, soil, materials, erosion, sediment
    water/         water volumes, flow, groundwater, evaporation, floods; rivers/lakes/oceans as
                   *derived* observations of where water actually is
    climate/       temperature, rainfall, wind, seasons, weather as regional (not global) state
    vegetation/    individual plants: growth, competition, dispersal, fire response, death-by-cause
    animals/       individual animals: needs, perception, behavior, food web, herds, disease
    humans/        individual people: bodies, needs, aging, skills, knowledge, inventory
    brains/        one decision architecture for all agents (perception → memory → needs/goals →
                   action + recorded rationale), parameterized by species/individual
    society/       households, settlements-as-emergent-clusters, institutions, culture, language,
                   belief clusters (religion = derived cluster, not founded-by-fiat)
    economy/       goods as items/lots, production, consumption, spoilage, transport, prices,
                   currencies
    politics/      factions, leadership, law, diplomacy, grievances
    military/      armies of real people, logistics, movement, battles, morale
    objects/       persistent items, buildings, roads, bridges, tunnels, canals, ships, relics
    pathfinding/   hierarchical regional graph + local A*/flow fields, cached paths, invalidation
    history/       append-only event store, causal graph, genealogy views, query API, text gen
    persistence/   binary versioned chunked incremental save/load, snapshots, replay (SAVE_FORMAT.md)
    inspection/    read-only typed API over all of the above — the ONLY interface for renderer/tools
  renderer/        map + entity views + inspection panels (reads inspection API only)
  tools/           sim_runner (headless CLI), world_inspector, replay, profiler, js_diff
  tests/           unit, determinism, emergence (Tests A–H), stress
  docs/            this set
```

## Core contracts

**IDs & storage.** Permanent typed IDs (`PersonId`, `PlantId`, `CellId`, `EventId`, …), 64-bit,
never reused; generational handles index hot storage. Hot per-entity data lives in dense columnar
arrays grouped by access pattern (positions, needs, health…); cold/variable data (memories,
relationships, inventories) in ID-keyed side tables. Death moves a person from hot storage to the
historical registry — same ID, nothing lost (§2.6). Aggregates (`population`, `forest_cover`,
prices, faction strength) are computed views with explicit, tested cache-invalidation.

**Clock & scheduler.** tick → hour → day → month → season → year → decade → century. Systems
register (frequency, phase); phase order is fixed and documented in SIMULATION_MODEL.md.
Per-entity cadence may stretch when the entity's situation is unchanged (dirty-tracking decides,
deterministically); cadence changes *when* an entity is evaluated, never *what* it is.

**Job system.** Each phase partitions work deterministically (by chunk / ID range), runs batches
across threads, threads emit commands into per-thread buffers, buffers merge in fixed order
(partition index, then entity ID) before commit. Thread count is invisible to outcomes.

**Event store.** Append-only, causal edges parent→child, entity back-references, indexed
by-entity/-location/-time/-type; segments seal and tier to disk; never capped (EVENT_MODEL.md).

**Inspection API.** Read-only and complete: any cell, water volume, plant, animal, person (alive
or dead), item, building, settlement, faction, belief cluster, event, family, or causal chain by
ID; plus spatial and derived-statistic queries. Renderer and all tools consume only this. Nothing
in it can construct or mutate entities — "observed" and "unobserved" regions are architecturally
indistinguishable (§2.3, Test H).

**Headless first.** `sim_runner --seed S --days D` is the primary artifact from Stage 1 onward;
the renderer is a feature-gated consumer added in Stage 7.

## What deliberately does NOT exist

- Mode flags (`pro`/`best`/`brains`/`histYears` pre-run) — one mode; worlds start at year 0 and
  history is lived, not pre-rolled at boot.
- Aggregate substitutes: per-cell vegetation density fields (`veg/vsp/vgen`), settlement stock
  scalars standing in for goods, formulaic garrisons, population counters that aren't `count()`.
- Story hooks: date-scheduled events, boredom-driven drama, probability-gated history
  (prophet-by-threshold, coin-flip peace/routs), singletons (`sim.beast`).
- Pruning: souls caps, event caps, biography caps, memory caps, lossy saves.
- Any renderer write-path into simulation state.

Each of these maps to a specific prototype site and an emergent replacement in AUDIT.md §F.
