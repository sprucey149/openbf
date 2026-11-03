---
id: SPL14
title: Split the two files still over the 1,500 cap: core/src/ai/state.rs 1,734 and game/src/walk.rs 1,669 - a pure move by SPLIT_RECIPE
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Cleanse 19 (2026-10-20, card CLN14): `scripts/size_report.sh --strict` exits 1 on main 7ba685f for two files without a waiver. `core/src/sim/spawn.rs` (2,758) is SPL12's; CLN14 adds its waiver line. `game/src/soldier_vis.rs` (2,071; grew through USKN's skeletons and skins) had no card; CLN14 adds a waiver line naming this card. Four more files are over the 1,500 cap with neither a card nor a waiver: core/src/ai/state.rs 1,787, game/src/walk.rs 1,664, bake/src/soldier.rs 1,637, core/src/sim/bounds.rs 1,501. engine/artifacts/briefs/SPLIT_RECIPE.md: one file, one session, a pure move; soldier_vis.rs first.

## Acceptance
- `scripts/size_report.sh`: soldier_vis.rs not in the over-1,500 list, and its waiver line deleted; `--check-map` exit 0 with the new files in engine/CRATES.md; `--strict` exit 0.
- Every gate PASS, nothing declared; goldens unchanged and the test count (gates.toml [ci] passed) unchanged.

## Cleanse note (2026-10-29)
`wc -l`: core/src/ai/state.rs 1,734, game/src/walk.rs 1,669 (both over the cap). Done or at it: game/src/soldier_vis.rs 667, bake/src/soldier.rs 1,244, core/src/sim/bounds.rs 1,500 (at the cap, not over).
