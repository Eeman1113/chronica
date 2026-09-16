# DETERMINISM.md — reproducibility contract and RNG scheme

## Contract

```
seed + engine_version + config  →  identical world state and identical event log
                                    after any number of ticks,
                                    across thread counts 1..N,
                                    across save→load→continue,
                                    across replay.
```

This is a *tested invariant* (Emergence Test G), not an aspiration. Any PR that breaks byte-identity of the reference-run hash fails CI.

## What the prototype does (for reference)

- One `mulberry32` generator `R`, reseeded per world from `hashStr(seedStr)` (`index.html:214–215, 535`).
- Auxiliary seeded generators for cultures (`:226`) and name banks (`:381`, `seed^0x5EEDFACE`).
- Everything in the simulation draws from the single global stream `R` in *iteration order* — meaning any reordering of updates changes all downstream draws. This is why the prototype can never be parallelized safely and why the engine does not inherit this scheme.
- `Math.random` appears only in renderer/UI code (weather particles, screen shake, UI jitter; clusters near `:6107–6127` and `:8103–8450`). That separation is preserved: non-engine RNG is allowed in the renderer only.

## Engine RNG scheme

**Master seed** → 64-bit; hashed with engine version salt so a version bump intentionally decouples worlds (engine_version participates in the contract, so same-version reproduction is exact and cross-version reproduction is explicitly not promised).

**Named streams**, each an independent counter-based PRNG (Philox- or SplitMix-family; counter-based chosen so a stream can be split without shared mutable state):

| Stream | Keying |
|---|---|
| `world_gen` | master seed |
| `world_gen.names.<culture>` | master ⊕ hash(culture id) |
| `climate` | master ⊕ domain tag, advanced per tick by fixed schedule |
| `water` | per chunk: master ⊕ chunk coord |
| `vegetation` | per plant: master ⊕ PlantId |
| `animals` | per animal: master ⊕ AnimalId |
| `humans` | per person: master ⊕ PersonId |
| `economy` | per region/market: master ⊕ RegionId |
| `battle` | per engagement: master ⊕ EventId of the engagement-start event |

Rules:

1. **Per-entity keying, not per-system order.** Entity A's draws never depend on whether entity B updated first. This is what makes deterministic parallelism possible: partition entities across threads however you like; each entity's stream is a pure function of (master seed, entity id, entity's own draw counter).
2. **Draw counters are entity state** and are saved. Save→load→continue draws the same numbers as an uninterrupted run.
3. **No global RNG anywhere in engine crates.** Constructing a thread-local or global generator is forbidden by lint/code review; every draw site receives a stream handle explicitly.
4. **ID allocation is deterministic**: IDs are allocated from per-phase counters merged in fixed phase order, never from thread arrival order.
5. **Iteration order is defined**: any container whose iteration feeds simulation must have deterministic order (dense arrays by index, ordered maps, or explicit sort by ID before iteration). Hash maps with randomized or pointer-dependent order never feed the sim.
6. **Floating point**: same-arch determinism is the baseline guarantee (no `-ffast-math`/fastfloat reassociation; no FMA-vs-non-FMA divergence — either fixed by build flags or by using integer/fixed-point math in accumulation-sensitive paths like water flow and economy totals). Cross-platform bit-identity is a stretch goal, documented per release; reductions across threads always occur in fixed (index) order regardless.
7. **Time never comes from the wall clock.** The sim clock is the only time source in engine code.

## Parallelism model (summary; details in ARCHITECTURE.md)

Every tick is a fixed sequence of **phases**. Within a phase, systems do staged read/write: read the previous-phase snapshot, emit commands/deltas into per-thread buffers. Buffers are merged in deterministic order (by partition index, then by entity ID) before commit. Thread count changes partitioning of *work*, never ordering of *effects*.

## Verification

- `tests/determinism/`: same seed twice → byte-identical save; 1 thread vs N threads → identical; save at day D, load, run to 2D vs uninterrupted 2D → identical; replay of the event log reproduces the derived-statistics timeline.
- Reference-run hashes for a fixed seed set are committed; CI compares.
- `tools/replay` diffs two runs event-by-event and reports the first divergent event with both causal chains.
