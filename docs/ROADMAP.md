# ROADMAP — what Chronica needs next

Synthesis of three sources: the engine's own diagnostics (what actually blocks emergence), a
fresh emergence-design brainstorm, and a line-by-line mining of the browser prototype for
portable features. Deduplicated and ordered by what unlocks what. Items marked **[port]** come
from the prototype (with its cheats replaced per AUDIT.md §6); items marked **[new]** go beyond
it.

---

## Phase 1 — "Civilization takes root" (the survival bottleneck)
*Goal: 5/5 seeds reach year 60 with growing villages. Everything else compounds on this.*

1. **Winter physics: food preservation + fuel & warmth** [new] — stored lots carry real
   moisture/smoke state; drying racks and smoking over real fires slow spoilage; huts have
   interior warmth fed by a woodpile. Winter becomes a *preparation* problem with two resource
   loops (wood ↔ food ↔ labor), and every saved or starved winter has a why-chain.
2. **Fishing + aquatic life** [port: fisheries/salmon] — fish shoals as real entities in the
   rivers we already grew; salmon runs in autumn; fishing as a learnable technique. The
   historical winter-gap closer and settlement magnet.
3. **Memory → schemas (learned foresight)** [new] — repeated congruent memories ("hungry every
   early spring") consolidate into expectations that bias decisions *ahead* of the event.
   People who suffered springs stockpile before spring. Solves demography through learning,
   not parameter tuning; every anticipatory act cites the memories that taught it.
4. **Real pathfinding (A\*)** [port] — people and animals currently walk straight lines and
   stall against lakes. Silent starvation cause; prerequisite for trade and armies.
5. **Agriculture economics audit** — instrument harvest-per-farmer-year vs. winter need; fix
   the pipeline until a farm demonstrably carries a family.
6. **Granary pests** [new] — rats that eat unsealed stores; countered by pots (pottery matters)
   and raised floors. Storage loss with causes and countermeasures.

## Phase 2 — Visible world & husbandry (prototype parity, mostly quick)

7. **Minimap** [port] — whole-world overview, viewport rect, click-to-jump. Hours of work,
   transforms navigation.
8. **Follow-cam** [port] — track a person/animal through their day (strictly camera; cadence
   stays observation-blind).
9. **Search + jump** [port] — over the *entire un-pruned* population, living and dead.
10. **Family tree UI** [port] — pure traversal of Born/Married events; dynasties become legible.
11. **Event cards + breadcrumb trail** [port] — click chronicle → cause/consequence navigation;
    the UI that sells the why-chain engine.
12. **Fences, pens, gates** [port] — containment for the herds we can already tame; staged
    construction from real wood; fence-leaps from real animal state, gates closed by a real
    keeper (forgetting emerges from that person's day).
13. **Town walls** [port] — masons laying real stone courses; defense vs. beasts and raiders.
14. **Weather/season rendering + scars** [port] — draw the rain/snow we already simulate; the
    scar glyph language (stumps, lakebeds, ruins, grazed-bare) so the map shows its wounds.
15. **Timelapse scrubber** [port] — yearly territory/settlement snapshots, replayable; "what
    happened while I was away," tiered to disk, never capped.
16. **Stats & charts panel** [port] — population/wildlife/forest time-series, uncapped.
17. **Graveyards & burial** [port] — grief-driven burial; graves weather only when no one
    living remembers the occupant (the prototype's most poetic compliant mechanic).
18. **Toasts** [port] — transient notices for notable events at speed.

## Phase 3 — Economy & material truth

19. **Item lots with provenance** [new+port] — the honestly-absent acceptance-checklist item:
    discrete goods carrying maker, material origin, transfer chain. Heirlooms, theft that
    matters, "the axe whose flint came from the far outcrop."
20. **Toolmaking chains, quality, wear** [new] — knapping/hafting; tools speed work, break
    mid-use, carry their maker's renown. Corpse materials (bone/hide/sinew) feed in.
21. **Barter from marginal utility** [new] — exchanges proposed during real encounters, valued
    by each side's need; *prices are a derived statistic over actual trades*. No market phase.
22. **Reciprocity ledger** [new] — every share/gift/rescue is a debt-memory; generosity accrues
    deference. One memory type unlocks status, feasting, big-man leadership.
23. **Skill decay & specialization** [new] — comparative advantage → division of labor →
    interdependence that barter needs. Professions become *derived labels* [port, de-cheated].
24. **Transport & pack animals** [new] — carry capacity binds; tamed beasts carry; caravans
    become physical consequences of geometry [port: caravans, after information honesty].
25. **Deliberate fire as land management** [new] — burning as technique; flush regrowth;
    occasional catastrophic escapes.

## Phase 4 — Society, politics, conflict

26. **Derived leadership & succession crises** [new+port] — influence computed from the
    deference/bond graph; the prototype's claim/support succession scoring ports nearly as-is;
    death of the influential + ambition = crisis, no dice.
27. **Norms from sanction patterns** [new] — witnessed violations → gossip → shared sanction
    clusters ARE the law, derived like religions.
28. **Lineage identity & exogamy** [new] — kin recognition, marriage between bands weaving
    alliance networks that inhibit or redirect feuds.
29. **Plots & conspiracies** [port, de-cheated] — recruitment along real bonds; stress from
    real misfortune density.
30. **Scouting & believed strength** [new] — raid decisions weigh *believed* defender strength
    from real (possibly stale) scouting; deterrence and disastrous miscalculation.
31. **Blood-price peacemaking** [new] — feuds end by compensation when both sides' real state
    prefers it; the payment is real goods in the event log.
32. **Captives & exile** [new] — raiders take people; losers of dominance contests found new
    camps. Culture transmission across enemy lines; every daughter-village has a founder with
    a grievance biography.
33. **Factions / diplomacy / armies / war** [port, the big one] — muster with the prototype's
    real grain-logistics gate (ports verbatim), supply attrition, battles resolved from
    morale+formation+terrain (never `chance(0.25)`), sieges where defenders are who's actually
    present, dice-free steward secession (ports verbatim), conquest/raze with real victims.
34. **Migration bands** [port] — group migration to *known* (heard-of) destinations, causally
    chained departures. **Imperfect information / news** [port] underneath it: stable beliefs
    about distant places updated by real reports.

## Phase 5 — Disease, deep ecology, deep world

35. **Pathogen instances** [port-mandated replacement] — reservoir species, contact-chain
    transmission over the co-location/trade/herding graphs, per-person immunity. Every plague
    a traceable story ("caught it from her brother, who met a trader from…").
36. **Migratory herds + territory memory** [new] — seasonal routes as emergent behavior; "the
    crossing" as teachable, losable hunting knowledge; dangerous *places*.
37. **Ice** [new] — frozen crossings, ice fishing, thaw drownings. **Slope microclimate** [new]
    — sun-traps explain "why they settled here." **Rock shelters/caves** [new] — free early
    shelter, contested with denning predators.
38. **Point minerals** [port: ore/flint/clay] — depletable outcrops driving travel, trade,
    conflict. **Canals** [port] — dug channels routing *real* water (our water sim makes the
    prototype's cosmetic canals genuinely functional). **Tunnels** [port, de-cheated].
39. **Crop & livestock selection** [new] — heritable yield traits under repeated human
    harvest-and-sow: domestication emerging over centuries with a traceable lineage.
40. **Beaver-analog ecosystem engineers** [new] — real dams into the water sim; four-system
    why-chains ("the field flooded because the dam because the beavers because the aspens").
41. **Multi-year climate oscillations, soil exhaustion & fallowing, river meandering** [new] —
    the decade drought everyone remembers; Malthus on real soil; the river that left the town.

## Phase 6 — Meaning & the historian's payoff

42. **Tales as mutating artifacts** [new, flagship] — retellings drift by teller personality;
    the inspector can *diff a legend against the true events*. Watching a hunting accident
    become a hero-myth over 80 years is the killer demo of a no-scripting causal engine.
43. **Man-eater reputation & organized culls** [port, de-cheated] — threat ledgers, real bait
    corpses, watches; "legend" as derived reputation from witnessed kills. Any number of named
    terrors; no singleton, no stat buffs.
44. **Taboos with teeth** [new] — beliefs constrain behavior; sometimes maladaptively (a
    village starving beside food it won't touch — with the why-chain to prove it).
45. **Emergent place-names** [new] — locations named after high-weight memories ("Bear Ford"),
    spreading like beliefs; chronicle text starts reading like history.
46. **Feast gatherings** [new] — surplus + schemas + social pull synchronize multi-band
    feasts: redistribution, courtship, teaching, belief-spread in one emergent institution.
47. **Temples & priests** [port] — belief clusters get buildings, gatherings, mourning.
48. **Writing changes history** [port: discovery graph + books, de-cheated] — before writing,
    only memories and drifted tales survive; after, witnessed events persist verbatim. Books
    written by literate people about what they witnessed; lootable, inheritable.
49. **Relics** [port, uncapped] — exceptional items whose renown is derived from their real
    provenance. **Eras** [port, display-only derivation]. **In-world chroniclers** [new] —
    biased, incomplete in-world histories, inspectable against the truth.
50. **Derived Legends Mode** [new] — notability from event-graph centrality; era detection from
    event-density shifts; the discovery surface for everything above.
51. **Descendant tracer** [new] — the acceptance-test tool: pick any dead nobody, walk their
    consequences (children, students, tales, feuds) centuries forward.
52. **Counterfactual probe** [new] — deterministic engine superpower: re-run from a save with
    one perturbation, diff the histories. "What if she had lived?" — answered literally.
53. **Avatar mode** [port] — walk the world as a real person entity via an explicit command
    channel (also the debugging surface), every intervention recorded as events.

## Engine underneath it all
54. Event-store disk tiering + per-entity dormancy cadence (PERFORMANCE.md hot spots).
55. Smooth movement interpolation; renderer lenses for invisible state (belief spread,
    grievance heat, reciprocity flows).
56. Bigger worlds (384×256+) once throughput allows; decide closed-world vs modeled-outside
    before ships (AUDIT §9.4).

## Explicitly never
Great-beast singleton, mode flags, caps/prunes/throttles, observation-dependent cadence,
random-pool victim picks, scheduled anything. (AUDIT §6 list stands.)
