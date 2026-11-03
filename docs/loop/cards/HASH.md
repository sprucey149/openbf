---
id: HASH
title: `Sim::hash` omits `assist` and most of `Bolt` — a hash-contract change (a declared sim re-baseline)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 7 (B1). `--sim-hash 100` is pinned by a test in `core/src/sim/state.rs`.

## Acceptance
The missing fields hashed; the pinned hash and `gates.toml` sim rows re-baselined, declared `sim`; replays still identical run to run.

## 2026-10-24 source checkpoint (pending acceptance)

All Bolt fields are now serialized; SHASH's already-present positive assist timer
hashing is retained. Four HASH tests independently check 16 bolt scalars, 9 hit
states, null/count controls, predictive-hit damage, 601-state repeated runs and
assist duration/slot/null behavior. Core844 PASS; three deliberate omissions or
aliases fail their intended assertions. Fmt, core/test and game-bin Clippy, game
debug build PASS. Parent/candidate scripted aim non-hash columns identical at all
72 ticks, hash changes on 69; two candidate CSVs identical, final
`b5a2e27d28863d2f` (parent `75f43c4b00858a11`). Fixture100/600 and trace unchanged.
These asset-free checks do not earn acceptance: real walk/aim logs, TAT1 bake,
font, adapter and both configured baseline trees are absent. No pins, gates.toml,
goldens or thresholds changed. `declares: sim` remains the intended re-baseline;
