---
id: UTAX
title: BF2 Utapau (showcase): the Rebels are trapped at their own base - bots attacking the far posts take a STRAIGHT fallback leg from hub 61/64 to hub 65, stop at a ledge drop at (-150, 14.4, 143), recover back to hubs 12/13 and repeat (111 unit-s a round, Imperials 7); Rebel first capture 101 s vs 28 s, posts held 1.4 vs 3.9, the Imperials win 16 of 16 - both PS2 rounds went to the Rebels (48-2, 28-0)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card WINS (2026-10-22): on main 3ae13d3 the Imperials win BF2 Utapau (a showcase map) 16 of 16 seeds in the fidelity
baseline and 8 of 8 in WINS's re-run; kills are even (78 / 76 per round), the Rebels lose ~73 tickets a round to the bleed
because they hold 1.4 posts on average against 3.9. On the PS2 the Rebels won both bots-only rounds (48-2 and 28-0, `lead_tmp/wins/ps2/bf2_uta1_r{1,2}_cps.jsonl`), holding
3.7-3.8 posts on average against 2.1, and owned their two nearest neutral posts (CON_CP7, con_CP1a) by the first sample
(3 Imperial deaths in); the port's Rebels take CON_CP7 at 40-101 s. The Rebels are slow out of their base: 50 m from their spawn takes p90 60 s (Imperials 24 s) and 30
Rebel lives of 60 s+ never get 50 m out (Imperials 9); their first capture comes at 101 s median (Imperials 28 s).

## Evidence (one traced case, main 3ae13d3 + a read-only probe)
- Black box `engine/artifacts/lead_tmp/wins/bb/t1500/bf2_uta1_s1.json` (BLACKBOX_AT=1500, seed 1), `--unit 2`: Rebel
  engineer #2, attack cp1 (CON_CP5, dest hub 65), walks legs 12->61 (table), then `leg -1->65 arc -1 s` (a STRAIGHT
  fallback, 200 m) from hub 61/64 and stops at `STOP Drop at 1.00 m` at (-150, 14.4, 143), slides (ROUND side +-1),
  recovers back to hubs 12 / 13 and repeats - from 9 s to 132 s of its life (107 slide rows at the spot). Units 3, 11, 12,
  7, 8 do the same in that seed.
- Census (8 seeds, `lead_tmp/wins/run1/bq/`, `lead_tmp/wins/census_slide.txt`): Rebels slide at the cell (-150, 140) for
  111 unit-seconds a round, Imperials 7; Rebels slide 7.3 % of their rows, Imperials 2.6 %. ROUND_VERBOSE at 100 s: nav
  `fallbacks straight 23`, `level_rejected legs 63 (unrouted 23)`; 10 of 16 Rebels within 50 m of their home post.
- A second case: `bb/bf2_uta1_s1.json` (tick 750) `--unit 9`: legs 25->26 / 26->35 slide along `uta1_bldg_macy_collision1`
  for ~9 s on the upper ledge above CON_CP7.

## Do
Find why hub 61 (or 64) has no table route to hub 65 (and the other far posts) and the unit walks straight off the ledge
edge: a level-rejected leg, a missing arc, a drop arc the soldier filter removes. Fix it at the root (route or graph),
not by a per-map patch.

## Acceptance
- A test on the UTA1 graph: a route from hub 61 to hub 65 exists for a soldier and its first leg is not a straight fallback
  (mutation-checked).
- 8-seed census on the branch: Rebel slide at cell (-150, 140) <= 15 unit-s / round (main 111); Rebel p90 time to 50 m
  from spawn <= 30 s (main 60 s); report the win share (main 0-16) and posts held (main 1.4 / 3.9) - the PS2 had the
  Rebels on 3-4 posts.
## Result (2026-10-22 01:15, lane 1, branch utax 733bf61 on main 3ae13d3) - built, PARKED on the RTX
Cause: arc 79 (hub 61 -> 64) is the authored (table) way off the Rebel terrace to con_cp5 / the far posts; hub 61's
floor is 14.4, 64's 9.4 - a 5 m drop past the floor walk's 4 m follow-down (`TRACE_DROP` = `ai::LEDGE_DROP`), so
`arc_walkable` failed both ways, BNDS marked it dead, every retry (table, floor BFS, any other arc) failed and the leg fell
to the straight line. The steering lets the drop go once the leg aims at 64 (its ledge is measured from the waypoint).
Fix (`sim/bounds.rs`): when the level walk fails and the far hub is more than a ledge lower, the arc is walked again
following the floor down to `ARC_DROP` 8 m (17 m/s landing, under the 25 m/s lethal cut). One arc direction moves on 32
maps (nav_walls' pinned table: uta1 8 -> 7, the rest unchanged); 64 -> 61 stays shut.
- Test `nav_walls::utapaus_terrace_drop_is_walked_down_to_the_far_posts` (61 -> 64 open, 64 -> 61 shut, first leg 61 ->
  65 = Table via arc 79 to 64); mutation ARC_DROP = 4.0 fails it ("61 -> 64 is walked down").
- Census, 24 seeds each, main + WINS probe vs branch + probe (`lead_tmp/utax/census/{main,br}`, `census.py`, `wins_cp.py`):
  wins main 0-24 -> branch 14-10 (Rebels); Rebel slide at cell (-150, 140) 80.9 -> 1.0 unit-s/round (bar <= 15); Rebel
  p90 to 50 m 69.0 -> 27.2 s (bar <= 30); posts held 1.3 / 4.0 -> 2.6 / 2.8; first capture Rebels 170 -> 28 s. Imperial
  60 s+ lives that never go 50 m 12 -> 36: guard / fight at their own posts (`never.py`: guard 5814, fight 4297 rows,
  recover 57) - the Rebels reach them now.
- Gates GPU-free (`lead_tmp/utax/gates2/SUMMARY.md`): sim, simhash, arena, leak, ordnance, protect, map PASS; fidelity PASS
  (better: recycled all maps 46 -> 33); sweep MOVED on bf2_uta1 only (declared); ci 1833/0/53 (+1 test). `--auto`
  (`gates/`) refuses: gpu ADAPTER LOST.
