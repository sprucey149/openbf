---
id: CLN10
title: Small leftovers: the `missions_root()` / `extracted_root()` literals, the era/sound/HUD-icon fallbacks, the TAT1 `--spawn` pin (delete `HISTORIC_WALK_START_MAP`)
milestone: M0
value: 1
phase: cleanup
status: done
deps:
files: engine/crates/game/src/walk.rs, engine/crates/game/src/main.rs, engine/crates/game/examples/arena.rs, engine/crates/ui/src/hud.rs, engine/crates/ui/src/hud_draw.rs, scripts/sim_gates.sh, engine/CRATES.md
declares:
needs:
effort:
branch:
resume:
created: 2026-09-25
tries: 1
---

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 9. `game/src/walk.rs:198` `HISTORIC_WALK_START_MAP`; pinning `--spawn -140,230` needs an edit to
`scripts/sim_gates.sh` — gate-script edits have been classifier-denied before (gpu_tests.sh); if denied, leave that half
for the maintainer in the report.

## Note (cleanse CLNR, 2026-10-02) - three more small leftovers, outside the render area
- `ui/src/hud.rs` `TEAM_COLORS` (used by the egui lines and `spawn.rs`) has no doc: say where the two colours come from, or label them PROVISIONAL.
- `ui/src/hud_draw.rs` `anchor_shift`: the 1/3 and 2/3 thresholds are tagged `[ours]` only; PROVISIONAL is the ledger keyword.
- `game/examples/arena.rs` (`SWEEP_SLOT0`'s doc) cites "PLAN §WHERE THINGS STAND", a heading PLAN.md no longer has.

## Source cleanup (2026-10-24)
- Both recorded TAT1 replays explicitly pass `--spawn -140,230`; removed the
  map-name exception in `WalkSim::rescue_spawn`. Unpinned TAT1 starts now follow
  the ordinary friendly-post rule. Grounded TAT1 and groundless/no-post controls
  are separate tests; no simulator rule or hash serialization changed.
- Labelled egui team colours and Hor+ thirds PROVISIONAL; repaired arena's
  historical table pointer. Updated the two changed module-map line counts.
- `rg` over current engine/scripts Rust/Python sources finds no `missions_root`
  or `extracted_root` symbol. No remaining helper body to consolidate.
- Retained current fallback rules: mission::pick_index selects the default for
  cross-era pairs (ERAM); sounds::merge_props documents per-level duplicates
  and cross-era lookup (SNDE/ANNC2); main/battle::squad_icons already labels
  BF1 icons on BF2 maps and their placement PROVISIONAL (SQMK). No new PS2 claim.
