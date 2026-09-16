# DECISIONS.md — append-only architectural decision log

Format: `D-NNN (date) — decision — reason`. Never edit or delete an entry; supersede with a new one.

---

**D-001 (2026-09-16) — Implementation language: UNRESOLVED, escalated to user at the Stage 0 gate.**
The rebuild directive contradicts itself: §0 sets `IMPLEMENTATION_LANGUAGE = Rust` with `RENDERER_STACK = Rust + wgpu + egui`, while the appended second half of the same directive repeatedly mandates "a native C++ simulation engine." Stage 0 (audit + docs) is language-neutral, so the audit proceeded. All Stage 0 docs are written language-neutrally; the words "the engine language" mean whichever is chosen. This is exactly the "expensive to reverse AND not settled by the rules" case in which stopping to ask is required (§3.3). Recommendation recorded here for the record: **Rust** — memory safety without GC matters enormously for a 100k+-entity, heavily multithreaded, deterministic simulation (data races are *compile errors* in Rust, and §2.9 determinism-under-threads is the single hardest requirement in either language); cargo gives workspace/crate structure, testing, and benchmarking out of the box matching the §4.1 layout; wgpu+egui satisfies the renderer stack natively. C++ offers no capability Rust lacks for this workload.

**D-002 (2026-09-16) — Behavioral reference file is `index.html`.**
The directive names `index(2).html`; no such file exists in the repo. `index.html` and `chronica.html` are byte-identical (both 445,497 bytes, 8,452 lines). `index.html` is treated as the authoritative behavioral reference; `chronica.html` is ignored as a duplicate.

**D-003 (2026-09-16) — Audit methodology.**
The 8,452-line prototype was audited by six parallel readers covering contiguous, slightly overlapping line ranges (1–1080, 1050–2330, 2300–3580, 3550–5010, 4990–5960, 5900–8452), each producing structured notes (function inventory, entity fields, event mechanisms, relationships, persistence, cheats, renderer-mutations, RNG usage). Every cheat site named in the directive (§2.4/§5-Stage-0) was additionally verified first-hand by the lead: event cap (line 499), bio cap (529), memory cap (2875), souls prune (4133), walk-LOD (4001), prophet threshold (5700), `befriend`/`proQuarterlySocial` (1958/1964), `sim.beast` singleton (1713–1772), save-time truncation (8139, 8173), mode wiring (8206, 8249–8262). Findings are synthesized in `docs/AUDIT.md`; the raw per-range notes live outside the repo (session scratchpad) since `AUDIT.md` is the durable record.

**D-004 (2026-09-16) — Docs are written before engine code, per Stage 0.**
`ARCHITECTURE.md`, `SIMULATION_MODEL.md`, `ENTITY_MODEL.md`, `EVENT_MODEL.md`, `DETERMINISM.md`, `SAVE_FORMAT.md`, `PERFORMANCE.md` describe the *target* engine, not the prototype. Where they cite prototype line numbers they refer to `index.html` at commit `c253cd5`. They will be revised as stages land; `AUDIT.md` is frozen once Stage 0 closes (it describes a fixed artifact).

**D-005 (2026-09-16) — The prototype's own emergent-direction refactors are kept as design seeds, not as code.**
The prototype is *mid-migration* toward the rules: rebellion is already grievance-driven ("rebellion by dice is gone in every mode", line 5696), pro-mode has plots recruited person-by-person, and `proQuarterlySocial` already restricts friendship pools to co-location/co-profession/shared institutions. These are treated as statements of intent that the engine completes (full causality), not as implementations to port.
