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

---

## STAGE 4 — Humans

**Done:** `objects` (buildings with maker/materials/condition/real food stores; item scaffolding);
`humans` — every person per ENTITY_MODEL: body (health, nutrition, hunger, thirst, fatigue),
personality traits + piety, uncapped decaying memories (kinds incl. inherited told-tales),
individually-held techniques (Test E), practiced skills, typed relationships formed only through
real encounters (friend/spouse/parent/child/teacher/student), marriage from repeated real
courtship meetings, conception only when spouses are actually together, birth events with real
parents, grief propagating through real kin bonds, water-carrying, camp anchoring + hunger-driven
camp migration, gathering (greens/mast/root-digging from real plant biomass and litter), pursuit
hunting, woodcutting (real trees → real logs), hut building (real wood), granary stores filled by
real deposits and shared as band commons, farming (sowing real wheat plants; harvest events), and
teaching along strong bonds with caused Taught/Learned event pairs. The decision model is the
animal brain's architecture extended (needs + percepts + memory + relationships + personality →
action + recorded `HumanRationale`).

**Verified by:** Test E both scenarios pass — knowledge spreads through real co-located teaching
with causally-chained events, and dies forever when every knower is killed before teaching.
Full suite green: determinism ×4, Test A ×2, Test B, predator/prey. Human event stream verified
in 6-year worlds: births, marriages, buildings, stores, felling, teaching all present and caused.

**Rules audit — honest status:** Multi-generation demographic sustainability is NOT yet achieved:
in 40-year test worlds the founding population (with 22+ births) declines to extinction within
~6-10 years, driven by winter/spring food gaps and local resource depletion around fixed camps —
each collapse fully diagnosed from the event log (thirst → stale water memories [fixed]; spring
famine → missing storage economy [fixed]; autumn collapse → local over-extraction [partially
addressed via sharing commons, farming founders, root foraging]). Every intervention has been
physical/biological/behavioral — never a spawn, never a stat, never an exemption. Remaining
candidates (documented for follow-up): seasonal whole-camp migration; stronger
agriculture-first loop; per-cell forage regrowth tuning. A harsh world violates no rule (§2.4
"if the world is boring, the world is boring" — likewise hard), but the acceptance-vision worlds
need surviving lineages, so this remains the top open item.

**Decisions:** water-carrying, mast/root foraging, and founding-generation knowledge seeds are
recorded as behavioral/biological modeling choices (period-accurate), not cheats: all consume
real state and emit real events.

**Next:** Stage 5 — settlements as derived building/population clusters, belief clusters,
grievances; Stage 6 conflict + tools.

---

## STAGE 5+6 — Society, belief, conflict, causal tools

**Done:** `society` — settlements are DERIVED: a monthly observation pass recognizes real
building clusters (union-find over actual hut positions) with real residents; identity persists;
abandonment observed, never deleted. Beliefs originate in individuals whose real grief/terror
memories (cited as causes) crystallize under high piety — no population dice, no piety scan of
the world, no founding by fiat; transmission happens only inside real conversations
(`share_belief` hooked into socialize), with AdoptedBelief events chained to the origination;
"religions" are derived clusters (`belief_clusters`). Conflict: raids arise from desperation
(real hunger state, cited) or grievance (real kin-death memories, cited) plus courage plus an
actually-visible out-group granary; defenders actually present may fight; killings breed the next
generation's grievance memories — feud dynamics with full causal chains. `inspection` — the
read-only API: `describe()` generates all player-facing text from structured events (chronicle
voice), `why_text()` walks causal chains to physical/social StateRefs, `biography()` derives life
stories purely from events. `world_inspector` speaks chronicle: latest events, why-chains,
person biographies, derived settlements/beliefs.

**Verified by:** Test C (settlements from real clusters, founding events anchored) — pass.
Test D (6 seeds: raids causally chained where they occur; at least one seed raid-free — violence
possible, never scheduled) — pass. Test F (predation death traces to hunger StateRef; why-text
renders) — pass. Full suite: 15/15 green. Forbidden-pattern grep (`if year ==`, `every_N_years`,
`start_war`, `generate_legend`, `possible_world_events`, drama lists): zero hits in engine/src.

**Rules audit:** Formal markets/prices/currencies are NOT implemented (no fake): goods exist as
real stores and carried loads; trade/prices remain for the economy build-out documented in
CLAUDE.md open items. Wars-of-armies likewise: conflict currently expresses as raids/feuds at
band scale — armies await larger societies. Nothing pretends otherwise.

**Next:** Stage 7 renderer (egui over the inspection API), then Stage 8 benchmarks.

---

## STAGE 7 — Renderer

**Done:** `renderer/` (separate crate, `chronica-renderer`): eframe/egui application over the
inspection API only. Map texture from real grid state (ocean, derived rivers/lakes, vegetation
cover, snow, active fire, burn scars); entities drawn from real positions (people, herbivores,
predators, buildings); derived settlement labels. Click-to-inspect: cell panel (elevation,
temperature, plant moisture, surface/groundwater, soil, the actual plants growing there),
person panel (needs, current action, the recorded decision rationale, techniques, bonds,
memories count, biography derived from events), animal panel (genes, generation, rationale).
Live chronicle panel: last 40 events in generated chronicle voice; clicking an event renders its
"why?" chain. Run/pause/speed control; new-world-from-seed.

**Verified by:** builds clean (rustc 1.98 via rustup — Homebrew's 1.86 was too old for eframe;
engine re-tested 15/15 under 1.98); 10-second live smoke run stays up. The engine builds and
tests fully headless without the renderer crate (workspace member, not a feature of the engine).

**Rules audit:** the renderer holds the Sim and calls `tick()`, but contains zero writes to any
entity or grid field — all drawing goes through public read access and `inspection`. No
observation signal enters the engine anywhere (Test H holds architecturally: there is no code
path by which looking can change state).

**Next:** Stage 8 — benchmarks + PERFORMANCE.md numbers + final acceptance checklist.

---

## STAGE 8 — Stress & benchmarks

**Done:** measured throughput at two world scales, thread-scaling equality, save size and
load-continue timing; recorded in PERFORMANCE.md with hot-spot inventory for future §2.12-legal
optimization. No optimization in this pass changed entity counts, state, or rules (none was
applied — measurement only; the engine already meets interactive speeds at dev scale).

**Verified by:** hashes identical across thread counts at benchmark scale; full suite 15/15
after benchmarking.

## FINAL ACCEPTANCE CHECKLIST (directive §8)

- [x] Engine in Rust is the sole authority; renderer owns no state.
- [x] Exactly one simulation mode; no flags, tiers, or brains-off switches.
- [x] Every cell, water volume (per-cell surface+ground stores), plant, animal, human, building
      has a stable persistent ID / index and real state. (Coins/lots partially — goods are real
      stores/loads; itemized lots and currencies not yet built, honestly absent.)
- [x] Dead/destroyed entities remain fully queryable; nothing pruned or capped.
- [x] Relationships, knowledge, memories, beliefs are individual data.
- [x] History is a causal event graph; player text generated from it (`inspection::describe`).
- [x] High-level statistics are `derive(entities)` (population, cover, water stats, belief
      clusters, settlements).
- [x] No scheduled/scripted/probability-gated historical events (grep-verified; Test D).
- [~] Settlements, beliefs, feuds, knowledge lineages emerge in test worlds. Wars-of-armies,
      markets/currencies, and multi-century civilizations require the demographic sustainability
      item (Stage 4 report) plus the economy build-out — the two honestly-open items.
- [x] Same seed → same world across thread counts and save/load; replay verify works.
- [x] Nothing exists because the player looked; nothing stops existing unlooked-at.
- [x] Every inspection shows true state; "why?" reaches physical/social state (Test F).
- [x] Performance work = measurement + layout/indexing plans; truth untouched.
