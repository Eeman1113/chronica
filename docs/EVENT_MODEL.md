# EVENT_MODEL.md — the causal history record

## What the prototype has (reference)

`logEvent(type, text, x, y, ids, meta)` at `index.html:480` produces:

```
{ id (evSeq), d (sim day), type, text, x, y,
  ids: {p|sett|fac|...},          // entity references
  actor, victim,                  // optional person IDs
  why: {named numeric reasons},   // the true inputs behind a decision
  causeId,                        // parent event (or causeTxt free text)
  cons: [child event ids]         // capped at 8!
}
```

This is genuinely a causal record — `why` carries real numbers, `causeId/cons` form a graph, and
the UI walks it. But: consequences are capped at 8 per event (`:493`), the whole log is capped at
5,200/20,000 events with "minor" (life/misc) events pruned first (`:499–508`), player text is the
*primary* record (`text` is composed at emission), and saves truncate to the last 3,000/12,000
events (`:8173`). Biographies (`bio[]`, capped 44/160 entries, `:529`) duplicate history as text
strings per person instead of referencing events.

## Engine model

**An event is structured data; text is a view.**

```
Event {
  id:            EventId (u64, monotonic, never reused)
  time:          SimTime (tick precision)
  kind:          EventKind (typed enum: Birth, Death, Kill, Marriage, Taught, Traded, Built,
                 Burned, Flooded, Migrated, Founded, Fought, Deposed, Adopted-Belief, …)
  location:      CellId | None
  participants:  [(EntityRef, Role)]     // Role: actor, victim, witness, object, beneficiary…
                 // EntityRef is any typed ID: person, animal, plant, item, building, settlement…
  causes:        [EventId | StateRef]    // parents; StateRef points at world state
                 // (e.g. cell moisture below threshold) for physical causation
  rationale:     RationaleRef | None     // for agent decisions: the brain's recorded inputs —
                 // the engine's replacement for the prototype's `why{}` (see brains/)
  magnitude:     kind-specific payload (casualties, quantity, price, …)
}
consequences:    derived reverse index of `causes` — never stored as a capped forward list
```

Rules:

1. **Append-only, never pruned** (§2.5, §2.6). Events accumulate into sealed immutable segments
   that tier to disk (SAVE_FORMAT.md). RAM holds recent segments + indices.
2. **Every meaningful state change emits an event**, from every system — physical (fire spread,
   flood, tree death with cause) as well as social. "Meaningful" is defined per system in
   SIMULATION_MODEL.md; the test is Emergence Test F: any observed change must be explainable.
3. **Causal edges bottom out in state.** "Why?" must terminate at physical/social facts:
   war ← grievances ← famine memories ← harvest events ← soil-moisture StateRefs ← rainfall
   events. A StateRef captures (entity/cell, field, value, time) compactly at emission.
4. **Consequences are an index, not a field** — unbounded, queryable, no cap-8.
5. **Text is generated on demand** from `kind` + participants + magnitude via the language/culture
   layer (chronicle voice is a renderer concern). No player-facing string is authoritative.
6. **Biography = query**: `events where participants contains person P` ordered by time. No
   per-person text arrays, no caps. Genealogy = view over Birth events + relationship records.
7. **Indices**: by-entity, by-location (chunk), by-time, by-kind, plus the causal graph both ways.
   Built incrementally as segments seal; persisted with the save.
8. **Significance is computed, not declared** — e.g. size of the downstream consequence cone,
   distinctiveness, participant renown. Used only for display ranking, never for retention.

## Historical-entity registry

Dead people, extinct species, razed buildings, dissolved factions, dry rivers, abandoned
settlements, destroyed items: their full final state moves to the registry keyed by permanent ID,
remains reachable from every event that references them, and is served by the inspection API
identically to living entities (plus death/destruction metadata). The prototype's lossy `souls`
map (pruned >22,000, biographies kept only for notables, `:4130–4136`) is replaced by this
registry; "notable" disappears as a storage class and survives only as a computed display rank.
