---
id: SHASH
title: A per-tick sim-state hash stream: the port writes one canonical record per tick (unit positions and health, the game LCG state, team scores and tickets, post owners, round timers) as a trace file, and a headless replay of a recorded pad log (InputLog) reproduces it byte for byte - the port half of the PS2 state-diff oracle (research rec. 6)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
`research/decomp_landscape_20261008/README.md` rec. 6: every decomp project that works has an objective oracle the agent cannot argue with; ours is behavioural and today only prose plus aggregate sweeps. The port already has two coarse determinism hashes (`game --sim-hash N` -> `hash100` / `hash600` in `gates.toml [sim]`, one number after N ticks) which say "something moved" but not which tick or which field. A per-tick stream of per-FIELD hashes is the port half of a first-divergence differ (FDIFF) against the PS2 (PSHASH). It is also a better "did this landing move the sim" gate than one end-of-run number.

## Evidence
- `core::sim::input::InputLog` records `PadState` per tick per slot (`slot_count`, `frames()`) and `Sim::step(&[PadState])` (`core/src/sim/step.rs:73`) is the one tick entry: a replay already exists, the trace hooks into it.
- `core::lcg::GameLcg::state()` (`core/src/lcg.rs:75`) already exposes the shared LCG state (the PS2's: BF1 `DAT_0066eed0`, BF2 `DAT_0073d870`, `analysis/re-blaster-damage.md` section 3).
- The existing gate: `scripts/gates.py` `g_sim` runs `scripts/sim_gates.sh` and `game --sim-hash 100`; the new stream must not change those numbers (it only reads).

## Design (decide the open ones in the card; do not widen)
- **Record** per tick t: `t`, then FOUR group hashes + a short field list so a differ can say which field: `units` (per unit by stable id: x,y,z quantized to 1/16 m, health quantized to 1/4, team, alive), `rng` (the LCG state, raw u32), `score` (tickets per team, kills, casualties), `posts` (owner team + capture progress quantized per post), `timers` (round clock, respawn timers). Hash = FNV-1a 64 over little-endian fixed-width fields, canonical order (unit id ascending).
- **Quantization is the contract**: the PS2 side (PSHASH) can only match to what a float read gives; the quantum is a constant in ONE place (`trace.rs`) that FDIFF reads from the trace header (so the differ can compare two traces with different quanta only via the coarser one).
- **Trace file** = a text header (`# openbf-trace 1`, map, seed, quanta, tick rate, field list) + one line per tick `t unitsH rngH scoreH postsH timersH`; optional `--trace-full` adds the raw tuples per tick for the differ to print the offending field's two values (cap the file; tests run 600 ticks).
- **CLI**: `game --sim-trace <out> <ticks> [--input <inputlog>] [--seed S] [--map M]`; replay of the same InputLog + seed twice gives identical files.

## Notes
Filed 2026-10-08 from the research note. Group order and the quanta are decided HERE; SHSRE/PSHASH/FDIFF conform. Does not need PCSX2 at all - ships and is useful (landing bisect) by itself.

## Cloud build checkpoint (2026-10-24)

Incomplete: isolated uncommitted candidate on `shash` at
`engine/artifacts/lead_tmp/worktrees/shash`; continuation/patch/logs in
`engine/artifacts/lead_tmp/shash_cloud/`. Original Acceptance remains owed.

Format decisions: FIVE groups, matching the five named groups/columns above
(the word FOUR was a counting error); dead slots included, unit identity is
slot plus generation; posts use registration order; whole tickets and member
kills/deaths per team; capture meters 1/256 s, timer floats 1/1024 s; exact
elapsed/respawn tick integers; nearest rounding with ties away from zero;
non-finite/out-of-range values error. Full tuples are decimal encoded integers
with a 64 MiB error cap. Quanta come from trace.rs and are emitted in the header.

Implemented fixture/map CLI and observational writer. The 600-tick fixture
InputLog replays are byte-identical; a single ly high-byte change at frame 300
first differs at record 300. A grounded core fixture's jump-byte variant first
differs at 301. Independent decoder checks all 3,000 full-row group hashes per
trace and rejects a corrupted LCG tuple. Existing 100/600 hashes unchanged.

Still required: real-map replay and timing within 2% with restored
`engine/assets_baked/tat1` and `engine/artifacts/p3_walk/{walk_demo,aim_demo}.ron`,
full CI and auto/landing gates with restored gate inputs. No commit, push,
landing or completed-card claim. Exact checks and evidence are in the report.

## GitHub checkpoint recovery (2026-10-24)
