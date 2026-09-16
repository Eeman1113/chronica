# ENTITY_MODEL.md — what exists and what state it carries

> Stage 0 status: normative field model derived from the prototype inventory (AUDIT.md §2) plus
> directive §2.2/§4.4. Exact types/layouts are fixed per stage; this defines *what must exist*.
> Everything has a permanent typed 64-bit ID; hot data is columnar; cold data in side tables;
> dead/destroyed entities keep full state in the historical registry (EVENT_MODEL.md).

## Cell (every tile is addressable state)
elevation · soil {depth, nutrients, type} · moisture & groundwater level · temperature (derived
from climate fields + altitude, cached) · biome label (derived from actual cover/water — never
authoritative) · surface material · fertility (derived) · surface-water link (WaterVolumeId or
none) · river/lake/ocean membership (derived from water state) · erosion/sediment state · snow/
ice depth · mud · fire state {burning, fuel, burn scar age} · vegetation occupancy (PlantIds via
spatial index; cover% derived) · structure link (BuildingId/none) · road/trail/tunnel object link
· resource deposits {kind, quantity remaining} · contamination · litter/detritus · claim/territory
(derived from settlement/faction assertions, kept as cache with invalidation).
Prototype ancestors: `world.*` arrays incl. `veg/vsp/litter/soilN/seedB/vgen/blight/cut/road`
(AUDIT §2) — the vegetation and road layers become derived or object-linked.

## Water volume
id · kind (surface/ground/channel segment) · volume · temperature · velocity/flow direction ·
sediment load · quality/contamination · source/destination links. Rivers, lakes, oceans are
*named derived observations* over connected volumes (with persistent identity for history: "the
river that dried" remains queryable).

## Plant (individual)
id · species · position (cell) · age · growth stage/biomass · health · water need/stress ·
nutrient state · root extent · reproductive state (flowering, seed load) · seeds produced/
dispersed (events) · damage (browsing, fire, felling) · disease instance links · genetics (per
individual, replacing per-cell `vgen`). Death always carries a cause (drought, fire, disease,
herbivory, felling, age, competition, frost). Forest cover, biome labels, seed banks, timber
stock = `derive(plants)`.

## Animal (individual)
id · species · sex · birth/death time · position · body {health, injuries, condition} · needs
{hunger, thirst, fatigue} · disease instances · genetics (heritable vector; prototype's 4-gene
scheme generalizes) · generation · perception state (what it has actually sensed) · memory
(water sites, dangers, den, herd mates — replacing `wx/wy/noWater/_tnm` ad-hoc fields) ·
emotional state (fear/arousal) · goals & current activity + recorded rationale · territory ·
herd/pack membership & social rank · mate/offspring links · ownership (owner PersonId/
SettlementId; domestication state; replacing `dom`) · man-kill / kill history = derived from
events (no `kills/manKills/legend` counters as authority) · reproductive state (cycle, pregnancy
with sire identity — replacing the fortnightly world pulse).

## Human (individual) — superset of prototype `newPerson` + directive §4.4
identity {id, name parts, culture} · genetics · sex · birth/death time · body {health, nutrition,
hunger, thirst, fatigue, injuries, disease instances, pregnancy} · aging state · personality
(prototype's amb/brav/pie/cha generalize to a trait vector) · emotional state · skills (practiced
levels, not booleans) · knowledge set (techniques, facts, places, people — individually held,
teachable, forgettable) · beliefs (weighted, with provenance — the religion substrate) · memories
(uncapped, weighted, decaying by rule, inheritable as distorted retellings — port of `mem[]` +
`inheritMemories`) · relationships (typed edges with strength/history: parent, child, sibling,
spouse, friend, enemy, rival, comrade, creditor/debtor, teacher/student, employer/employee,
leader/follower, witness, killer/victim — port of `frs[]`+kin, uncapped) · occupation & work
state · income/wealth (conserved ledger) · possessions (real ItemIds) · home (BuildingId) &
tenancy · location & movement state · current task/goal stack + recorded rationale · fears/
ambitions/preferences · social status & renown (derived from events, replacing `dist/notable`) ·
faction/settlement membership · belief-cluster affiliation (derived) · culture & languages known ·
biography = derived event query (no `bio[]` text array).

## Object (persistent items)
id · kind (tool, weapon, clothing, book, artifact, furniture, cart, container, coin lot where
practical) · material(s) · quality/condition/wear · created-by/when/where · owner chain (current +
history via events) · location (carried/in building/on ground) · uses (events) · destruction
{time, cause}. Relics are ordinary objects whose *derived* significance is high (provenance =
their event history; replaces `relic.log[]`).

## Building / infrastructure
id · kind (house, farm plot, market, temple, keep, wall segment, gate {open state, since},
store, fence, grave/monument, bridge, road segment, tunnel segment, canal segment, ship) ·
position/footprint · materials & construction progress (real inputs consumed) · condition/decay ·
builder(s) · owner/controller · contents (ItemIds, stored lots) · capacity. Roads/bridges are
objects created by decisions and wear (usage counters may *inform* decisions but the road is a
thing, not a threshold — replaces `world.road>=ROAD_T`).

## Goods
Lots with {good kind, quantity, quality, provenance (producer, origin, time), spoilage state,
location (building/carrier)}. Settlement `inv` Float32 arrays are replaced; prices are per-market
observations from real bids/scarcity.

## Group entities (all derived-membership, persistent identity)
**Household** (co-residents, shared stores) · **Settlement** (emergent cluster: founded-by,
buildings, institutions, derived population/professions; keeps prototype's pens/walls/gates/
graveyards/hunt orders as institutional decisions backed by real objects and ledgers of real
events — `threat` grievance ledger becomes derived from attack events) · **Institution** (market,
temple, watch, guild: members, rules, treasury) · **Faction** (leadership, law, treasury ledger,
diplomacy from interaction history, wars with real causes) · **Belief cluster** (derived over
belief graph; founder/teachers real) · **Culture/Language** (phonology + drift through real
contact — port of `contactYearly` mechanics onto individual speech contacts) · **Army** (roster
of PersonIds, supplies as real lots, morale, formation) · **Currency** (issuer, metal, purity,
minting ledger conserved against real metal).

## Ecosystem-level records
Corpses (individual, decay staged, scavenging bites, fertilization — port, uncapped) · disease
instances (pathogen kind, host, transmission chain — replaces settlement `plague` flags) · fires
(per-cell state + ignition event) · weather (regional fields, not a global singleton).

## Storage notes
Hot columns grouped by system access; IDs never reused; entity counts at reference scale
(PERFORMANCE.md): ~10⁴ humans, ~10⁴–10⁵ animals, ~10⁶ plants, ~10⁶–10⁷ cells. Plant hot state
must fit ~16B/individual (species, cell, age, biomass, health, flags) with cold detail in side
tables — this is the binding layout constraint identified in AUDIT §9.2.
