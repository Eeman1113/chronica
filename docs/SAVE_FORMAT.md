# SAVE_FORMAT.md — persistence design

> Stage 0 status: design contract. The concrete byte layout is specified when Stage 1 implements
> the skeleton and is versioned from day one.

## Requirements (from §2.6, §2.8, §4.7)

1. **Complete**: a save captures *all* authoritative state — every cell, water volume, plant,
   animal, human (living and dead), object, building, settlement, faction, belief cluster,
   relationship, memory, knowledge item, brain state, goal, event, causal edge, RNG stream
   counters, scheduler state (cadence assignments, dirty sets, phase position), and the clock.
2. **Bit-resumable**: load → continue ≡ uninterrupted run (tested, see DETERMINISM.md).
3. **Never lossy**: no "keep only notables", no event truncation. The prototype's save is the
   anti-pattern being replaced: it keeps at most 4,000 non-notable souls (`index.html:8139`) and
   the last 3,000 events (12,000 in best mode, `:8173`) — a lossy save that reconstructs an
   approximation. Ours resumes the same world.
4. **Binary, versioned, chunked, incremental**: no generic JSON for world state.

## Architecture

**Container**: a save is a directory (or single-file archive) of sections:

```
save/
  MANIFEST            engine_version, schema_version, master seed, config, clock, section index, checksums
  world/chunks/*      per-chunk columnar cell state (terrain, water, vegetation-cell links, structures)
  entities/<kind>/*   columnar hot state per entity kind, ID-ordered; side tables (memories,
                      relationships, inventories) as length-prefixed ID-keyed records
  history/segments/*  append-only event segments (see EVENT_MODEL.md), immutable once sealed
  rng/                per-stream counters not derivable from entity state
  sched/              cadence assignments, dirty sets, phase cursor
```

**Encoding**: little-endian, columnar within sections (mirrors in-memory SoA so saving is mostly
sequential memcpy), varint/delta encoding for IDs and timestamps, general-purpose compression
(zstd-class) per section. Field presence is schema-driven, not self-describing per record — the
schema lives in the manifest under `schema_version`.

**Incremental saves**: chunk- and segment-granular. A snapshot writes only dirty chunks/segments
plus a new manifest referencing unchanged sections from the parent snapshot (content-addressed or
generation-tagged). History segments are immutable once sealed, so they are written exactly once —
this is what makes "never prune" affordable: old history costs disk, not RAM or save time.

**Tiering**: the engine may keep sealed history segments and cold side-table pages on disk during
*runtime* as well, memory-mapped or paged in on query. Persistence and runtime cold storage share
one format so "load" of cold data is a no-op.

**Schema versioning**: `schema_version` bumps on any layout change; a loader supports reading
N-1 with an explicit migration pass (which re-verifies determinism from the migrated state going
forward; cross-version bit-continuation is not promised — see DETERMINISM.md).

**Replay**: a replay file = manifest + config + the input timeline (none in pure-observer worlds)
+ periodic state checksums. Replaying re-runs the sim; checksums verify agreement. `tools/replay`
diffs two runs at first divergence.

## Explicit non-goals

- Human-readable saves (use `tools/world_inspector` against a save instead).
- Backward-compatible saves across many engine versions (the world is seed-reproducible; long-term
  archival is the replay file: seed + version + config).
