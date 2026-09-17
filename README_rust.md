# Chronica — the Rust engine

> *Do not simulate the story. Simulate the world that could produce the story.*

This is the native rewrite of the [Chronica prototype](README.md) (`index.html`) as a
deterministic, fully emergent world engine in Rust. **Everything that exists is an individually
simulated entity** — every cell, every river's water, every blade of grass, every hare, every
person — and history is the causal, queryable consequence of their interactions. There is no
story director, no scheduled drama, no "important NPC" tier, and nothing is ever pruned.

Built in eight audited stages (reports: [`docs/STAGE_REPORTS.md`](docs/STAGE_REPORTS.md); design
docs: [`docs/`](docs/)). **15/15 tests green**, including byte-exact determinism across thread
counts and save/load.

---

## The world

*A founding summer (seed 2024, year 0). Green is real vegetation cover — tens of thousands of
individual plants; blue is water that actually flowed there; white is snowpack; yellow dots are
people; tan dots are herds.*

![World map, year 0](screenshots_rust/y0_map.png)

*The same world three years on. Nobody placed a single river or lake — rain fell on generated
terrain, flowed downhill, filled basins, spilled, and carved the drainage you see. Winters bury
the high latitudes; spring melt floods the valleys.*

![World map, year 3](screenshots_rust/y3_map.png)

### Hydrology lens

*Groundwater fill (dark → bright) with derived river channels (light blue) and lakes. Aquifers
drain downslope by hydraulic head; baseflow keeps valley streams alive between rains; plants
transpire from this same store — drink the valley dry and the forest feels it.*

![Hydrology lens](screenshots_rust/y0_water.png)

### Vegetation lens

*Every pixel's brightness is the summed biomass of the individual plants actually rooted there —
trees (green) versus grasses and reeds (yellow-green). Forest cover is `derive(trees)`, never a
density field.*

![Vegetation lens](screenshots_rust/y0_veg.png)

*A six-year-old world (seed 7): fire scars, regrowth, migrating herds.*

![Seed 7, year 6](screenshots_rust/s7y6_map.png)

---

## History is derived, not written

All player-facing text is generated from structured causal events. A person's biography is
literally a query over what happened to them:

```
$ world_inspector world.crn person 3
Sana — culture 0, dead
--- biography (derived from events) ---
  Year 0: Sana felled broadleaf oak
  Year 0: Sana raised a dwelling
  Year 0: Sana laid up 2.1 in store
  Year 1: Sana laid up 1.9 in store
  Year 2: Sana and Mirvertal became close
  Year 2: Sana first spoke of The Way of Tarama
  Year 2: Sana died of hunger
```

Sana's belief was born from her own grief memories (the origination event cites them); had she
lived, it would have spread only through real conversations. In this world another faith fared
better:

```
$ world_inspector world.crn beliefs
The Way of Belverbel: 6 faithful
```

And every "why?" walks the causal graph down to physical state:

```
$ world_inspector world.crn why 46550
Year 3: a hare died of hunger
  ← because Hunger was 2.00
```

---

## What is actually simulated

| Layer | The truth underneath |
|---|---|
| **Water** | Rain fields → surface flow by hydraulic head (3 sub-steps/day) → infiltration → groundwater with downslope drainage → springs, evaporation, erosion. Rivers/lakes are *observations* (`is_river`/`is_lake`), never placed. |
| **Plants** | Each an entity: own RNG stream, lifespan drawn at germination, growth from its cell's real water/light/nutrients, transpiration back into the aquifer, light competition, seeding real neighbor cells, winter dormancy, death always with a cause event. |
| **Animals** | Each an individual: needs, condition, inherited genes, generation count, perception via spatial index only, one shared utility brain with **recorded rationale**, pursuit predation (dice only resolve a struggle that really closed distance), courtship, gestation with recorded sire, corpses that rot and feed the soil. |
| **People** | Everything above plus: nutrition, personality, uncapped decaying memories, typed relationships formed only by real encounters, marriage → conception only when spouses are actually together, grief through real kin bonds, gathering/hunting/woodcutting/building/storing/sharing/farming that consume real world state, and techniques held in individual heads — taught along bonds, extinct if the last knower dies untaught. |
| **Society** | Settlements *recognized* from real building clusters; beliefs originating in real memories and spreading in real conversations; raids from cited hunger or kin-death grievances — some seeds stay entirely peaceful. |
| **History** | Append-only causal event graph, never capped, never pruned; consequences are a derived reverse index; text generated on demand. |

## Guarantees (tested, not aspirational)

- `seed → world` is **byte-identical** across reruns, across 1 vs N threads, and across
  save→load→continue (`engine/tests/determinism.rs`).
- Emergence suite: **A** (forest stats = count of tree entities; felling reduces cover *because*
  trees were removed), **B** (rivers/lakes from rain alone), **C** (settlements from real
  clusters), **D** (conflict possible, never scheduled — verified peaceful seeds exist),
  **E** (knowledge spreads by teaching / dies with its last knower), **F** (why-chains reach
  physical state), plus predator/prey causality.
- Forbidden-pattern grep (`if year ==`, scheduled drama, event quotas): clean.

## Run it

```sh
export PATH="$HOME/.cargo/bin:$PATH"   # rustup toolchain (needs rustc 1.88+)

cargo run --release -p chronica-renderer            # live map, click-to-inspect, chronicle
cargo run --release -p sim_runner -- --seed 7 --days 3600 --stats-every 360 --save world.crn
cargo run --release -p world_inspector -- world.crn                 # chronicle voice
cargo run --release -p world_inspector -- world.crn why <event_id>  # causal chain
cargo run --release -p world_inspector -- world.crn person <idx>    # derived biography
cargo run --release -p replay -- verify world.crn                   # bit-identical replay
cargo run --release -p snapshot -- --seed 2024 --days 1230 --out shot  # headless map PNGs (PPM)
cargo test --release --workspace                                    # the full suite
```

**Performance** (Apple Silicon laptop): 275 sim-days/s at 192×128 with ~40k plants, hundreds of
animals, full hydrology and fire; complete world saves are ~10 MB; details in
[`docs/PERFORMANCE.md`](docs/PERFORMANCE.md).

## Honest state of the world

The engine is complete through all eight stages, but two things are open (tracked in
[`CLAUDE.md`](CLAUDE.md) and the Stage 4 report):

1. **Human lineages don't yet persist for centuries.** Founding bands live, build, marry, teach,
   believe, and raise children — but most seeds decline within a decade (winter/spring food
   gaps, local over-foraging). Every collapse is diagnosable from the engine's own event log,
   which is the system working as designed. Candidate fixes are documented.
2. **Markets, currencies, and armies-at-scale** aren't built yet; goods are honest stores and
   conflict is band-scale feuding. Nothing pretends otherwise.

## Repository map

```
engine/            the simulation (core, world, terrain, climate, water, vegetation, fire,
                   species, brains, animals, humans, objects, society, history, inspection,
                   persistence, sim)
renderer/          egui app — reads the inspection API, owns no state
tools/             sim_runner · world_inspector · replay · snapshot
engine/tests/      determinism + emergence suite (Tests A–F, predator/prey)
docs/              AUDIT (the prototype, every cheat mapped) · ARCHITECTURE · SIMULATION_MODEL ·
                   ENTITY_MODEL · EVENT_MODEL · DETERMINISM · SAVE_FORMAT · PERFORMANCE ·
                   DECISIONS · STAGE_REPORTS
index.html         the original browser prototype (behavioral reference, unchanged)
```
