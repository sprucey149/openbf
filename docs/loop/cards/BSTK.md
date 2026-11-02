---
id: BSTK
title: The stuck measure skips a bot flying its remote droid - its body stands on purpose, so stuck_max counts only real failures to move
milestone: M3
value: 3
phase: done
status: done
deps:
files: engine/crates/core/src/ai/mover.rs; engine/crates/game/examples/round_to_outcome.rs
declares: sim
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
The remote droid (public pull request #57) lets a bot fly its droid: the droid takes the owner's pad
(`Sim::droid_pads`), so the body stands still on purpose. The bot's think still ran its walk and deflected the
stick, and the mover's progress window saw a moving stick and no progress: every window failed. `Route::stuck_run`
grew, `nav_stats.stuck_max` and the sweep's `stuck_max` with it, and the landing of #57 moved the sweep's median
120 -> 360 ticks for that reason alone (the fidelity `stuck_max_med` of `scripts/sweep_seeds.py --botq` reads the
same number). The failed windows also re-planned and side-stepped a body that could not move, and counted on the
last resort's watch (card RCYC/RLOOP), which gives up a bot that does not move.

## Evidence
Sweep `round_to_outcome -- 60000 42` with `STUCK_CENSUS=200`, `lead_tmp/botre/runs/BSTK/` (before = main 54524b09):

| | before | after |
|---|---|---|
| finished | 34/34 | 34/34 |
| stuck_max median | 360 | 180 |
| stuck_max sum | 12840 | 5640 |
| maps at 360 or more | 32 of 34 | 6 of 34 |

Per map (before -> after): bes2 360->60, kam1 360->60, kas1 480->120, kas2 480->60, rhn2 360->60, bf2 tan1
480->120, mus1 360->60, ... ; bes1, nab2, bf2_end1 stay 360 (the same unit at the same tick as before: a foot bot
early in the round, a real stand). The census now says what the record setter was doing: 25 maps `foot`, 9
`seated` (all exactly 180 = `DRIVE_STUCK_BAIL`: a driver whose vehicle got nowhere and who got out, a real
failure), none `droid`.

Other deliberate holds, checked in the code (none needed a change):
- a manned turret or vehicle seat: a seated bot's think is its seat's (`ride::drive`) and never reaches the mover;
  only a DRIVER's route window reports a stuck run (hover / walker drive), which is a real failure;
- a bot holding a position (a fight position, a defence spot held, a sniper's spot, a squad Hold): the mover's
  holder arm already makes the window wait;
- a bot that has arrived (post, defence spot, sniper spot) stands with no stick deflection, so no window runs;
  a boarder and a thrower are excluded too (`!mv.boarding`; the throw zeroes the stick).

## Change
`mover::run`: a bot whose droid is under its control (`Sim::controlled_droid`) runs no progress window and is not
counted on the last resort's watch. The census line names the record setter's state (`foot` / `seated` /
`droid`).

## Open
Public #58 (bots stay in droids too long) is not touched here.
