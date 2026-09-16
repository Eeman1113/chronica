# SIMULATION_MODEL.md — what runs, when, and why

> Stage 0 status: target model with prototype cross-references. Phase order becomes normative in
> Stage 1; per-system details are refined in Stages 2–6.

## Time

`tick` is the atomic step (sub-day; exact subdivision fixed in Stage 1 — the prototype's atom is
the day, which is too coarse for perception/combat/fire and forces it to resolve encounters with
single dice rolls). Aggregation boundaries: hour → day → month (30d) → season → year (360d) →
decade → century. The prototype's calendar (12×30-day months, `DAYS_Y=360`) is kept.

**No pre-run.** The prototype fast-forwards `histYears` of history at boot behind a progress bar
(`index.html:8270–8290`) using the same loop. The engine starts at year 0 and history is lived at
full fidelity; "an old world" is just a world you ran longer (headless, via `sim_runner`).
**No speed-dependent fidelity.** The prototype throttles thinking at high speed (`:4940`,
brains every 12th day) and drops backlogged ticks in the UI (`:8308`). Engine speed changes how
many ticks execute per wall-second, never what a tick does.

## Phase order (within a tick; fixed, documented, deterministic)

1. **Climate** — regional temperature, rainfall, wind, season effects (climate/)
2. **Water** — precipitation → surface volumes, flow, infiltration, groundwater, evaporation,
   flooding, erosion/sediment transport (water/, terrain/)
3. **Fire & physical hazards** — ignition from lightning/human causes, spread over real fuel
   (plants, buildings), burn state, smoke visibility inputs (world/, vegetation/, objects/)
4. **Vegetation** — per-plant growth, water/nutrient uptake (writes back to cell soil), seed
   dispersal, competition, disease, death-with-cause (vegetation/)
5. **Perception** — sensory queries for all agents due this tick: vision/hearing/smell against
   the spatial index; writes percepts, never world state (brains/)
6. **Cognition** — every due agent's brain: needs + percepts + memory + relationships +
   personality + goals → chosen action + recorded rationale (brains/)
7. **Action resolution** — movement (pathfinding/), work, eating/drinking, combat, speech acts
   (information transfer), building, trading; conflicts resolved by deterministic ordered commit
8. **Physiology** — hunger, thirst, fatigue, pregnancy, aging, injury, disease progression,
   death-with-cause (humans/, animals/)
9. **Society & economy** (staggered cadences) — household formation, institution upkeep,
   production/consumption of real lots, spoilage, transport arrival, price formation from actual
   bids/scarcity, belief transmission decay/reinforcement, grievance accounting (society/,
   economy/, politics/)
10. **History commit** — event segments append, indices update, derived-stat caches refresh
11. **Scheduler maintenance** — dirty propagation, cadence reassignment, spatial index compaction

Each phase reads the committed state of prior phases and stages its writes (ARCHITECTURE.md).

## Cadence

Registered frequencies per system (tick/hour/day/…) plus per-entity stretching: an entity whose
inputs are unchanged since last evaluation is re-queued at a longer horizon and woken early by
dirty events on its cell/chunk/relationships (a fire, a price move it would perceive, an attack).
Wake conditions are part of each system's spec — an entity may never be *unwakeable* by something
it could perceive. The prototype's 21-day striped vegetation pass (`VEG_STRIDE`, `:1074`) is the
degenerate ancestor of this: stripes by index, not by whether anything changed.

## The causality bar (what "no naked dice" means concretely)

Randomness may resolve *uncertainty inside a causal situation* whose inputs exist in state — never
substitute for the situation. The standing test for every roll in the engine:

> Could the inspector answer "why did this happen / why here / why them / why now" from state and
> events, with the roll only answering "and it could have gone the other way"?

Prototype examples on each side (full list in AUDIT.md §F):

- **Passes the bar** (port the shape): grievance-driven wars with real casus belli (`:5686`),
  steward defection from ground-down loyalty (`:5696`), births gated by grain-days and housing
  (`:5345`), army supply attrition (`:4966–4985`), bait as a real corpse, grazing eating real
  vegetation.
- **Fails the bar** (replace): prophet by `chance(pop/6000)` + piety max (`:5700`); peace by
  `chance(0.15)` (`:5214`); battle routs by `chance(0.25)` (`:5035`); raze-vs-conquer coin flip
  (`:5065`); one-plague-per-year worldwide `break` (`:5776`); global drought `chance(0.07)`;
  religion spread by distance roll (`:5723`); friendship by `pick(pool)` (`:1973–1976`);
  fortnightly species-level reproduction pulse with random parents (`:4263`); kill-at-adjacency
  `chance(0.35)` (`:4690`); daily-rerolled lifespan (`:4192`); tunnel collapse killing a random
  crew member (`:3757`).

Replacement pattern in every case: put the *state* that the probability was standing in for into
the world (beliefs held by individuals; morale and position in battles; pathogen instances with
transmission on real contact; courtship between co-located animals; structural integrity of a
tunnel section), and let the roll — if one remains — perturb only the outcome of a real encounter.

## System-by-system port map (prototype → engine)

| Prototype (function, line) | Engine home | Port notes |
|---|---|---|
| worldgen `generateWorld` (533) | world/, terrain/ | keep noise-based elevation/moisture concepts; rivers/lakes become consequences of the water phase, not generator output |
| `vegDaily`/`vegMonthly` (1074/1194) density fields | vegetation/ | per-cell densities → individual plants; biome relabeling from real cover |
| animals `updateAnimals` (4187) | animals/, brains/ | keep perception/pursuit/territory/herds; replace pulse-reproduction, dice kills, rerolled lifespans |
| people `updatePeople` (3504) + trips | humans/, brains/ | trips become real movement for everyone always (no 1800-pop LOD, `:4001`) |
| settlements `updateSettlements` (3404) + monthlies | society/, economy/, objects/ | stock scalars → item lots; walls/gates/pens/tunnels stay physical (already good) |
| economy/caravans/currency | economy/ | prices from actual bids/scarcity; goods with provenance |
| factions/diplomacy/succession | politics/ | keep grievance ledger direction; relations from interaction history, not `ri(-15,15)` init |
| war `updateArmies`/`fieldBattle` (4966/5035) | military/ | soldiers are the people they name; battles resolve from formations/morale/terrain over ticks |
| religion (5700) | society/ | belief clusters: individuals hold/transmit/lose beliefs; "a religion" is a derived cluster with real founders because they really taught |
| beast/legend (`sim.beast`, 1713) | derived | reputation computed from kill events + witnesses' memories; no singleton, no flag, no cap of one |
| memories/`memCap` (2875) | brains/, history/ | uncapped, weighted, decay-by-rule; inherited grudge distortion kept as a *transmission* mechanic |
| `logEvent` (480) | history/ | see EVENT_MODEL.md |
| pathfinding A* | pathfinding/ | hierarchical + flow fields + invalidation |
| save v2 JSON (8130) | persistence/ | see SAVE_FORMAT.md |
| Observatory panels (6000–8100) | inspection/ + renderer/ | panel contents define the minimum inspection API |
