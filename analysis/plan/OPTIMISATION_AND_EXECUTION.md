<!-- Moved verbatim from PLAN.md (original lines shown in the pointer table) on 2026-10-08. PLAN.md old sections: Optimisation backlog (603-700), Reference (753-767), PART B header (768-774), Tech stack (799-825), Workspace layout (826-848), Testing & fidelity ladder (860-884), Fidelity method (885-897), Community research (898-904), Kickoff decisions (915-932), Log (933-934). -->

## Optimisation backlog (added 2026-08-30 — the maintainer's direction)

Full numbers, method and reasoning: `engine/artifacts/perf/README.md` (local,
gitignored) and `analysis/community-projects.md`. Kept here because PLAN is what
survives; the artifact is not committed.

**Scope correction.** The 2026-08-30 baseline measured the PS2-AUTHENTIC
workload (16 AI/team, 720p, PS2 textures) and concluded nothing needed
optimising. The maintainer's direction is to SPEND that headroom: more AI units than the
PS2 held, higher resolution, the Harrisonfog HD set, and parallelism. Each moves
the workload off the axis that was measured, so the baseline is now a
REGRESSION REFERENCE and the open question is how cost SCALES.

**MEASURED 2026-08-31 (Opus session 3).** `--ai-cap` landed (it was the
blocker), plus `--bench-player-cam`, `--no-cull`, a units-alive count and a
DRAW-CALL count. TAT2, 720p, population held stable:

| units alive | tick mean | tick p99 | frame dGPU | frame iGPU |
|---|---|---|---|---|
| 17 | 0.003 ms | 0.011 | 0.25 ms | 2.54 ms |
| 65 | 0.015 | 0.032 | 0.35 | 3.26 |
| 194 | 0.069 | 0.176 | 0.64 | 3.75 |
| 380 | 0.193 | 0.535 | 1.09 | 4.89 |
| 563 | 0.389 | 1.067 | 1.25 | 6.32 |
| 740 | — | — | — | 6.98 |

- **M1 answered: the sim scales at about n^1.4, not n².** The pairwise loops
  early-out cheaply. At 563 alive the tick is 1.2 % of the 33.3 ms budget.
- **M2 answered:** frame cost is linear in skinned units (~1.7 µs/unit dGPU,
  ~6 µs/unit iGPU) on top of a per-map static cost.
- **M4 answered with the real shader:** 720p→4K at 201 units is 0.63→0.96 ms
  (dGPU) and 3.86→5.24 ms (iGPU) — nine times the pixels for ~1.4x the time,
  so still NOT fill-bound.
- M3 (CPU vs GPU split, needs timestamp queries) and M5 (HD texture memory)
  are still open.
- **Verdict: "more AI than the PS2 held" is already affordable**, at 4K, on
  the integrated GPU. A3 (spatial grid) and A4 (deterministic parallelism)
  have NO measured need and are insurance, not work.

**CPU / sim — gates more AI units** (all verified in code)
- A1 `separate_units` is full O(n²) pairwise + 2 heightfield samples per overlap
  (`core/src/sim.rs:502`). 32 units = 496 pairs, 200 = 19,900, 500 = 124,750.
  First wall.
- A2 AI target selection is O(n²) (`core/src/ai.rs:112`), compounding with A1.
- A3 A uniform spatial grid collapses both to ~O(n). Highest-value sim change;
  stays hash-stable if rebuilt per tick in index order.
- A4 **Nothing in the workspace uses threads or rayon** — the engine is entirely
  single-threaded. Gains are real but `core` is replay/hash-deterministic, so
  this needs DETERMINISTIC parallelism (fixed partitions, no order-dependent
  float accumulation). A `par_iter()` sprinkle would silently break replays and
  netcode. Design task, not a refactor.
- A5 Per-tick allocations: negligible at 32 units, unmeasured at 200+.
- A6 `Scene::resolve_model` linear-scans classes+geometries per instance
  (`game/src/assets.rs:268`) — load-time only, but O(instances × classes).

**GPU / render — gates resolution and HD textures** (all verified in code)
- B1 **DONE 2026-08-31.** Frustum culling: 78–94 % of static draws culled from
  the player's eye on the big maps, frame −14 to −17 % on the iGPU, all 42
  goldens byte-identical. Note the number that actually matters is DRAW CALLS,
  not draw-list length: END1's 2,645 draws already collapsed into 227
  instanced calls before culling.
- B2 **Baked LOD chains are ignored** — `ManifestGeometry.lods` exists but
  `resolve_model` returns one model. The discs ship LODs and we discard them.
  STILL OPEN; session 4 did not get to it.
- B3 One draw call + a 96-joint palette per skinned unit (`PALETTE_STRIDE`,
  `palette_capacity` starts at 16). At 200 units: 200 draws, 19,200 matrices/frame.
- B4 **DONE 2026-08-31 (session 4).** Terrain is uploaded as 32-cell chunks,
  each with its own AABB, so culling reaches it: −17 to −22 % frame time on
  the big maps, all 42 goldens byte-identical. It needed a prior fix — every
  submesh used to inherit the whole model's bounds — and it must happen at
  UPLOAD, not in the bake, or `Scene::heightfield` silently gets a garbage
  ground plane. See the Log.
- B5 BC7/BC5 block compression. Not needed at PS2 sizes (10.6–27.1 MB/map);
  needed on the classic-plus path (~16x the bytes → ~250 MB/map). Belongs to
  HOLD FOR FABLE #8; this arithmetic is the input to that decision.
- B6 Depth prepass — only once shading is expensive (after M4).
- B7 **RETIRED 2026-08-31 (session 4) — measured, no need.** Load is
  21.3–38.8 ms per map even after the PC tier, the SIDE tier and full mip
  chains grew texture bytes twice. Roughly a frame and a bit to open a map.

**Explicitly NOT on the list:** draw-call batching by material (already done —
opaque draws sort by (mesh, texture) into instanced runs; the blend pass is
intentionally unbatched because compositing is order-dependent), and optimising
the sim at 32 units (0.031 ms p99 — there is nothing there).

**Sequence, revised on the 2026-08-31 measurements.** `--ai-cap` + M1/M2/M4 and
B1 frustum culling are DONE. What is left, in value order:
1. **B5 texture memory / block compression** — the one number that is actually
   a problem. PS2 art is 10.6–27.1 MB/map; PC-stock-where-larger takes that to
   19–61 MB and the Harrisonfog set to ~250 MB. Belongs with HOLD #8.
2. ~~**B4 split the terrain mesh**~~ — DONE 2026-08-31, −17 to −22 %.
3. **B2 baked LOD chains** — still free data we discard, but the discs' LODs
   buy vertex work we are not short of; take it for memory, not frame time.
   **The next optimisation item, and the only one left with any value.**
4. **B3 skinned batching / B6 depth prepass** — only if a future workload
   moves the knee. B3's cost is now measured (~1.7 µs/unit dGPU).
5. **A3/A4** — no measured need. Do not spend determinism risk on them.



## Reference: what we hold (2026-08-29/30)
- Decompiled corpora: `analysis/SWBFRNT2.ELF-decomp.c` (21MB, 17,002 fns),
  `analysis/SWBFRONT.ELF-decomp.c` (16MB, 15,755 fns); censuses + string-xref maps.
- Named functions: 769 BF2 + 483 BF1 from in-binary Lua binding tables, applied in
  Ghidra projects (`ghidra-proj/{bf1,bf2}`).
- Anchors (BF2): lua_State `DAT_005faf7c`; snprintf `FUN_0051fbf8`; memset `FUN_0051d8e8`;
  memcpy `FUN_0051d734`; rand `FUN_005220a0`; Lua allocator `FUN_001a6148`; pool
  create/destroy `FUN_0059d568/0059d610`; anim-set builder `FUN_00282f78`; version beacon
  `FUN_003f7c68`; team-relations write in `SetTeamAsFriend` (+0x84); startup `FUN_003c1c38`.
- Toolchain: Sony ee-gcc, C++ w/ exceptions, eabi64, VU overlays. BF1: Lua 4.0.1,
  zlib 1.1.4, GSM 06.10. BF2: Lua 5.0.2, zlib 1.2.2. EENet.
- Community leverage: LibSWBF2, swbf-unmunge, SWBF2Phoenix (reference), exp111 notes.
- Tooling: Ghidra 12.1.3 + emotionengine-reloaded + JDK 21 in `tools/`; scripts in
  `scripts/`. Discs read-only at `<disc images>`.



# PART B — AUTONOMOUS EXECUTION GUIDE

Older working notes. `AGENTS.md` and `PLAN.md` hold the current rules.



## Tech stack (locked; versions chosen to reuse the maintainer's existing wgpu code)
- **Rust** stable (1.97+). Cargo workspace.
- **Render:** `wgpu 27` + `winit 0.30`. Adapt camera/mesh/pipeline/gltf-load code from
  The maintainer's own projects (`an earlier project's source` 52k LOC, `an earlier project's source`
  17k LOC) — same wgpu/winit generation. Cite what was adapted in code + RE_LOG.
- **Math:** `glam` (match the 3D projects). **Bytemuck**, **pollster**.
- **UI:** `egui 0.33` + `egui-wgpu`/`egui-winit`; **`egui_kittest`** for headless UI tests
  and `png` golden images — both patterns already used in `an earlier project's source`.
- **Input:** `gilrs` (gamepad; controller-only per decision #28).
- **Audio:** `kira` (positional + one-shot mixer), default; revisit at the audio phase.
- **Data:** `serde` + `ron` manifests; `gltf` (bake output), `image`/`png`, `ktx2`.
- **Collision geometry:** `parry3d` — `engine/Cargo.toml` asks for `"0.22"`
  and `Cargo.lock` holds **0.22.1**, which is load-bearing: that version's
  `contact_composite_shape_shape` passes `pose12.inverse()` where
  `contact_with_shape` wants `pose12` (a MIRRORED pose; upstream master is
  fixed). `core::collision::walls::mesh_contact` works around it per triangle and a
  regression test on an asymmetric body detects an upstream fix. A parry bump
  can therefore move every sim hash and is a lead-run, gated change — never a
  drive-by `cargo update` (moved here from HANDOFF 2026-09-08, cleanse C2).
  parry3d is for raycasts/overlap QUERIES ONLY. Character movement
  feel is bespoke, transcribed from the decomp — NEVER a generic physics engine for feel.
- **ECS:** ~~`hecs` (lightweight)~~ — **RESOLVED 2026-09-01: no ECS.** The sim is dense
  `Vec<Unit>` slots addressed by generation-checked handles (`core::unit::UnitHandle`),
  which is what the determinism hash and the replay tests index; `hecs` was declared in
  `core/Cargo.toml` and never used, and is removed. NOT Bevy — we keep the maintainer's reusable
  wgpu renderer and full control of the deterministic sim.



## Workspace layout
`engine/CRATES.md` is the live module map, and the crates are `mungefs`, `bake`, `manifest`, `core`, `render`, `audio`, `input`, `ui`, `net` and `game`.

## Testing & fidelity ladder (how the agent validates its own work)
- **Tier 0 — every commit:** `cargo build`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
- **Tier 1 — every commit (`cargo test`, headless, deterministic):**
  - Physics/math: port constants+algorithm from decomp, assert Rust output matches
    reference values traced from the decomp (see Fidelity Method). Epsilon-compare.
  - Determinism: same seed + same input log ⇒ identical `--sim-hash`. Guards netcode too.
  - Bake round-trip: parse known LVL chunks, assert mesh/texture counts + checksums.
  - Data: every RON manifest parses + schema-validates.
- **Tier 2 — visual goldens (headless):** render offscreen to PNG, perceptual-diff against
  committed `goldens/`. The agent READS the PNG itself to sanity-check colors/layout/pose.
  UI screens driven by `egui_kittest`. New/changed visuals update goldens deliberately.
- **Tier 3 — audio:** `ffmpeg` extracts loudness/spectrogram/duration from baked audio;
  assert similarity to source + exact durations. (Agent compares numbers, not ears.)
- **Tier 4 — human feel checkpoints (milestone boundaries only):** agent produces a
  runnable build + artifacts (headless-captured gameplay video, motion plots, side-by-side
  vs PCSX2 where set up) and asks the maintainer to judge feel. The only tier needing a human.
- **The 300-frame median protocol — any performance comparison** (moved here
  from HANDOFF 2026-09-08, cleanse C2; it lived only in a superseded session
  digest). A short bench run on this machine's iGPU is worth about **±0.4 ms**,
  so a comparison is the **median of five 300-frame runs**, A and B interleaved.
  Session 4's first terrain-split numbers came from 120–150-frame cold runs and
  were wrong: the protocol reversed one result from −29 % to 0 %. A single cold
  run is not evidence, and a bimodal config (TAT2 ai=64 on this machine) is
  reported with all five samples, not a mean.



## Fidelity method (RE → Rust), for every faithful subsystem
1. Locate functions in `analysis/*-decomp.c` + `analysis/*-bindings.txt` (grep the corpora;
   spawn an RE subagent for big subsystems).
2. Extract EXACT constants + the algorithm's structure (integrator, order of ops, fixed
   vs float). PS2 gameplay often uses specific tick-scaled constants — keep them exact.
3. Port to Rust; keep original constants in data with a comment citing the source address.
4. Write a Tier-1 reference-trace test (expected values from the decomp; spot-check against
   PCSX2 per decision Q1).
5. Record in a note `analysis/re-<topic>.md`: subsystem, source function addresses, findings,
   open questions. Record a keep/drop/merge decision in `analysis/plan/FEATURE_SET.md`.
**Solve it right:** when community mods don't reveal a mechanic and fidelity matters, RE the
decomp to find the real functions and constants. Hard, but it makes the product correct.



## Community research (honor the maintainer's no-bloat rule)
Study, don't blindly copy (watch licenses): LibSWBF2, swbf-unmunge, SWBF2Phoenix, exp111's
notes, and PC-port mods (ModDB, Steam Workshop, GameFront, Nexus, r/SWBF wikis) —
especially graphics mods (classic-plus look) and Galactic Conquest mods (R1). **Adopt an
idea only if it clearly improves gameplay; default to faithful.** Log both adopted ideas
and rejected-as-bloat items in `analysis/community-projects.md` so decisions are traceable.



## KICKOFF DECISIONS (RESOLVED 2026-08-30)
- **Q1 Fidelity oracle → STAGED, first tier now LIVE (2026-09-23).** Decomp-first stays
  the default (transcribe exact math, Tier-1 reference-trace tests). The PCSX2 trace rig
  once staged for "later" is stood up for its first tier — `engine/artifacts/pcsx2/pcsx2_capture.sh`
  boots a disc headless and screenshots it — and **the loop may use it itself**, for any
  subsystem where the decomp is ambiguous: the maintainer lifted the old "only the maintainer can supply a
  PCSX2 capture" restriction the same day (`OPUS_LOOP_PROMPT.md` §5/§7). Reaching a
  specific map point rather than whatever a fresh boot reaches by itself is still open —
  not `pcsx2-gsrunner` (it replays GS dumps, it does not boot a fresh ISO); the likely
  next step is a save-state library plus `-statefile`.
- **Q2 Autonomy → RUN UNTIL BLOCKED.** See Escalation policy. Max autonomy; interrupt only
  for unresolvable design forks, hard blockers, or guardrail questions.
- **Q3 Build order → FINISH RE AUDITS FIRST.** Complete A2–A6 before engine gameplay code
  (P0 gates on it). Workspace scaffolding during P0 is fine.
- **Q4 Git.** Decided since: the source is public, and everything sensitive stays gitignored. Publication follows `AGENTS.md`.
- (Codename: working name "**openbf**"; the maintainer may rename.)
