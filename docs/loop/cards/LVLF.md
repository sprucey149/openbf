---
id: LVLF
title: Bots recycled as 'lost' on open terrain: BF2 UTA1 71, BF1 HOT1 33, BF2 KAM1 23 'lost' deaths over 8 seeds - say for each whether the unit was really off the level or Sim::level_at read terrain (Hoth's trenches, Utapau's sinkhole floors) as off, and fix the test before STLVL / OFFG / STEND lean on it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-10-01 14:40, item 3 (repeated 17:15, not carded until now): `Sim::level_at` gives false positives under terrain (Hoth's trenches), and it should be right before any rule leans on it harder. Three of the maintainer's `first:` cards do lean on it: OFFG's rule (step 3 names `Sim::level_at`), STLVL (level-aware arrival) and STEND (the endgame hunt that was tried in BNDS and dropped because 'the hunters end up off the level': 155 lost deaths on Hoth, `lead_tmp/fable2/README.md` Task 3). A bot that level_at calls off for 45 s is killed with its ticket (BNDS, `core/src/ai/brain.rs` `st.lost`), so a false positive is a bot the player sees vanish on open ground. UTA1 and HOT1 are showcase maps.

## Evidence
- main ce66fe2, 8 seeds x 32 maps x 120,000 ticks (`lead_tmp/rcyc/sweep120k_main/seeds.md`, world-deaths column, read by the planner 2026-10-01, not re-run): `lost` deaths bf2_uta1 71, bf1_hot1 33, bf2_kam1 23 (+10 `under`), bf2_dea1 4, bf2_hot1 3, bf2_geo1 2, bf2_myg1 1; every other map 0.
- `core/src/sim/bounds.rs` header: 'The terrain is always on it' - so 33 lost deaths on BF1 Hoth, a terrain map, are either a hole in that sentence or units inside / under buildings. Not looked at yet.
- Not known: where each of these units stood. That is step 1.

## Notes
- Planner 2026-10-01: value 5 and `alpha: yes` because UTA1 and HOT1 are showcase maps and three `first:` cards use the test, so the answer is worth most BEFORE they build. It cannot be numbered into the maintainer's order (his to change); it runs beside it on the other lane.
- `files:` is `bounds.rs` only, on purpose: the `first:` cards all list `ai/brain.rs` and OFFG lists `round_to_outcome.rs`, and a shared file would keep one of the maintainer's cards waiting. Keep the step-1 observer a scratch example outside the tree (or env-gated lines that leave stdout identical), and if `round_to_outcome.rs` must change, land that as a last small commit once no `first:` card is in progress.
## Note from BOWL (2026-10-02) - one of the false positives, placed (`lead_tmp/bowl/README.md`, `m3/`)
- **BF2 Hoth's trench at (31..48, -2, -7)** is a floor 2.0 m under the snow, inside terrain cutters, between barriers;
  hub 29's defend spot (waypoint (32, -1, -6)) is in it. Main reads its floor at x 32-39 as `Off(2.01..2.08)`: the
  nearest walk stops more than `LEVEL_SLACK` away and the next arrives 1-8 cm over `BOUND_DROP`. Profile it with
  `LEVEL_PROFILE=24,-7,50,-7,-0.6,1 STUCK_CENSUS=1 round_to_outcome 1 3 bf2_hot1` (new with BOWL; from y 6 the ray
  finds the cut snow at 0.0, so ask from inside).
- BOWL added `trench = heightfield.is_some() && collision.is_cut(feet)` to `level_at` (a point in a terrain cut keeps
  the full `BOUND_DROP` past a barrier). The same flag is the natural place for this card's fix: a point in a cut and
  within a trench's depth of the heightfield over it is on the level.
- Placed with `SWEEP_DEATHS=2` on seeds 4-5 (main and BOWL, same cells): BF2 UTA1 at (-60, -74, 230..240), the
  sinkhole floor - really off; BF1 HOT1 at (-180, -10, 160..170), (-140, 10, 90..110), (-260, -10, -20..-10) - not
  looked at. After BOWL, 8 seeds x 60,000 ticks: UTA1 80 (main 71), BF1 HOT1 33 (33), BF2 HOT1 1 (1), KAM1 2 (11).

## Result (2026-10-02, landed 22eddb0; report `reports/20261002_0330_card_LVLF.md`, `lead_tmp/lvlf/README.md`)
- Main, 8 seeds: 129 lost deaths = 61 units SPAWNED under UTA1's landing (one node 1.01 m under its floor), 34 in
  Hoth's trenches (the level test: 1,085 of BF1 Hoth's 4,067 trench cells read off), 34 on ground a walk leads back
  from (the return walk's straight line fails). No bot walked off the level to where no walk leads back.
- Fixed: a trench is on the level; a spawn node under its floor spawns on it. 129 -> 36 (on the landing's base
  143 -> 41), both Hoths 0, rounds 256 / 256, no map's finish count drops.
- NOT met: 34 FALSE lost deaths remain (UTA1 21, DEA1 7, GEO1 2, KAM1 2, MYG1 1, YAV2 1) -> card RETW.
