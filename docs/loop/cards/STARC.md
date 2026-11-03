---
id: STARC
title: Keep bots on the arc: walk back to the arc line before cutting to the next hub (POL1 radar door, GEO1 bunker), and a last leg that starts where the capture centre is in reach (NAB1 ruins column, MYG1 spiral ruin)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Build session 2026-10-02 - what the radar door really was (read this first; the original text is below)
- **Not a routing defect.** The bots at POL1 (18, 25, 84) stood in a pocket closed by a collision plane: the wall
  1.5 m behind `pol1_prop_door3` (`pol1_bldg_radar_main01`) has a doorway, and the bake filled it. On the disc the wall
  is five leaves that are a SIGNED fan of a rectangle with a hole; the port fanned each leaf two-sided.
- **PS2:** 23 soldiers crossed that wall's plane in 420 s, every one inside the doorway (`lead_tmp/starc/README.md`).
- **Built** (branch `starc`): `mungefs::coll_fan` - a coplanar group round a non-convex leaf whose fans overlap is baked
  as the region it sums to. One baked file changes: `bf2_pol1/collision/pol1_bldg_radar_main01.glb`.
- **Acceptance, run (9 seeds x 60,000 ticks on POL1, one binary, shared bake -> repaired wall):** recycles 41 -> 5, at
  the radar door 32 -> 0; doorway crossings 7 -> 198 and 0 through the wall; round length (sum) 189,870 -> 108,180.
  Tests: `coll_fan::tests` (the wall's disc vertices: doorway open, wall there once, 540.8 m2), `the_discs_signed_fans_are_repaired`.
- **What this card no longer owns:** the last-leg half is LLEG's (NAB1 ruins column, MYG1 spiral ruin, GEO1 `last->9`,
  `last->43`); the other rows of the RCYC table below were not looked at here and are not arc-line cases by this
  session's evidence - `walk_point` / `approach_leg` (KSTK) already walk the arc's line.
- **Left open, as notes:** BF2 MUS1 `mus1_bldg_world_1`'s ten-sided ground leaf (not flat, outline crosses itself; its fan
  covers 30,816 m2 against a signed 28,504) is NOT repaired and not understood. The PS2's ray-vs-leaf test is one-sided
  with a convex edge test (B2 `FUN_00165070`, read, not verified); the port's meshes are two-sided everywhere.

## Why
STALL C5 + C6 (`analysis/re-round-stalls.md` §2).

**C5.**
- POL1 (18.5, 25.1, 80.0), 9/9 seeds, 0.97 M unit-ticks: units leave hub 3 (radius 1.6) 2–3 m off arc 0's line and
  walk straight at hub 0, 66 m away. They slide on the frame post beside `pol1_prop_door3`, 3.5 m from its 3 m trigger,
  so the door stays shut. On the PS2, 17 soldiers pass there in 420 s, none staying longer than 16 s.
- GEO1 hub 9: units on `geo_bldg_bunker_1`'s floor walk straight at outside hubs through its walls.

**C6.** An off-graph unit's route is only the last leg, straight at the capture centre:
- NAB1 (a showcase map), (−106, −108.5, 62.9), 7 seeds, into `nab1_prop_ruins_column2`;
- MYG1 seed 6, 54,140 ticks, into `myg1_bldg_med_spiral_ruin2`.

## Note from the maintainer's "rounds that do not end" pass (2026-10-01, main 4603e9e, `lead_tmp/endings/README.md`) — `first: 6`
- POL1's radar door (18, 25, 84) is still the top walking-in-place spot on seed 1: 456 unit-seconds of team 1
  (`samples/bf2_pol1_s1.csv`). POL1 finishes 8/8 now, so it costs time, not rounds.
- KSTK (f293fd2) already added `route::walk_point` (off the arc's line by over 1 m, aim 5 m ahead on the line) and
  `route::approach_leg`. OFFG (`first: 3`) takes the off-graph (locus Nearest) straight legs. Measure what is left after
  OFFG before building; if OFFG's rule covers C6, say so here and keep only what remains.

## Note from RCYC (2026-10-01) - the recycle spots this card owns, and an acceptance line
RCYC's last resort (branch `rcyc`: a bot that wants to walk and makes no net progress for 120 s is recycled) was used 496 times over 8 seeds x 32 maps x 120,000 ticks; 79 spots were recycled on 2 or more seeds. `lead_tmp/rcyc/owners.py` gives this card 41 of them (186 recycles) by a HEURISTIC (on the graph at the hub's own height: a last leg that never reaches its point, or no progress along the arc) - look at each spot before trusting it. Re-list: `sweep_seeds.py --seeds 1-8 --ticks 120000`, then seeds.md's `## Recycles`; the run behind this table is `lead_tmp/rcyc/sweep120k_d`.

**Acceptance, added: 0 recycles at these spots (seeds.md `recycled` column and spot list) on this card's maps.**

| map | spot (x, y, z) | recycles | seeds | leg | locus | floor dy | steer | basis |
|---|---|---|---|---|---|---|---|---|
| bf1_bes1 | (-18, 108, -240) | 2 | 2 | 25->2 dest 122 [table] x1 | A25 x2 | +0.0 | fwd x1 | a last leg that does not reach its point |
| bf1_bes1 | (54, 111, -240) | 2 | 2 | last->96 dest 96 [table] x2 | H96 x2 | -2.1 | slide x2 | a last leg that does not reach its point |
| bf1_end1 | (-18, 0, 54) | 9 | 5 | last->7 dest 7 [straight] x6 | N78 x9 | -0.0 | slide x9 | a last leg that does not reach its point |
| bf1_end1 | (-12, 0, 54) | 5 | 5 | last->7 dest 7 [straight] x2 | N78 x5 | -0.0 | slide x5 | a last leg that does not reach its point |
| bf1_end1 | (168, -6, -36) | 4 | 4 | last->0 dest 0 [table] x4 | H0 x4 | -0.4 | slide x4 | a last leg that does not reach its point |
| bf1_end1 | (168, -6, -42) | 3 | 3 | last->0 dest 0 [table] x3 | H0 x2 | -0.5 | slide x2 | a last leg that does not reach its point |
| bf1_end1 | (-180, -3, 324) | 2 | 2 | last->4 dest 0 [table] x1 | A4 x2 | -1.0 | slide x2 | a last leg that does not reach its point |
| bf1_geo1 | (66, -63, 102) | 5 | 4 | last->43 dest 43 [table] x3 | N43 x5 | -12.7 | slide x5 | a last leg that does not reach its point |
| bf1_geo1 | (66, -63, 108) | 4 | 4 | last->43 dest 33 [table] x2 | N43 x4 | -12.7 | slide x4 | a last leg that does not reach its point |
| bf1_kas1 | (78, -30, -60) | 8 | 6 | last->35 dest 35 [table] x8 | N35 x8 | -23.5 | slide x5 | a last leg that does not reach its point |
| bf1_nab1 | (-108, -108, 60) | 20 | 7 | last->2 dest 2 [table] x10 | N2 x20 | -0.8 | slide x14 | a last leg that does not reach its point |
| bf1_nab1 | (-108, -108, 66) | 11 | 6 | last->2 dest 2 [table] x9 | N2 x11 | -0.8 | slide x9 | a last leg that does not reach its point |
| bf1_yav1 | (84, -48, -252) | 3 | 3 | 1->17 dest 17 [table] x2 | H1 x3 | -0.5 | back x2 | on the arc at its height, no progress |
| bf1_yav1 | (102, -60, -198) | 3 | 3 | 17->18 dest 18 [table] x2 | H17 x3 | +0.0 | slide x3 | on the arc at its height, no progress |
| bf1_yav2 | (-24, -39, -138) | 2 | 2 | 58->55 dest 0 [table] x1 | H55 x2 | +0.0 | fwd x2 | on the arc at its height, no progress |
| bf1_yav2 | (24, -39, -156) | 2 | 2 | 57->97 dest 64 [table] x2 | A97 x1 | +0.0 | fwd x2 | on the arc at its height, no progress |
| bf1_yav2 | (24, -39, -138) | 2 | 2 | 59->57 dest 14 [table] x1 | H57 x2 | +0.0 | fwd x2 | on the arc at its height, no progress |
| bf2_dea1 | (-162, 51, -144) | 7 | 5 | last->106 dest 76 [table] x4 | N106 x7 | -20.9 | slide x7 | a last leg that does not reach its point |
| bf2_end1 | (-18, 0, 54) | 3 | 3 | last->2 dest 2 [straight] x2 | N78 x3 | -0.0 | slide x3 | a last leg that does not reach its point |
| bf2_end1 | (168, -6, -42) | 3 | 3 | last->0 dest 0 [table] x3 | H0 x2 | -0.5 | slide x2 | a last leg that does not reach its point |
| bf2_end1 | (-90, 9, 300) | 2 | 2 | 72->2 dest 17 [table] x1 | N72 x2 | +11.4 | slide x2 | a last leg that does not reach its point |
| bf2_geo1 | (60, -63, 288) | 4 | 3 | last->18 dest 18 [table] x4 | H18 x4 | -0.3 | slide x4 | a last leg that does not reach its point |
| bf2_geo1 | (66, -63, 288) | 2 | 2 | last->18 dest 18 [table] x2 | H18 x2 | -0.3 | slide x2 | a last leg that does not reach its point |
| bf2_kam1 | (-42, 60, 6) | 4 | 4 | last->50 dest 89 [table] x2 | A50 x4 | -1.8 | slide x4 | a last leg that does not reach its point |
| bf2_kam1 | (42, 48, 24) | 4 | 3 | 38->39 dest 2 [table] x1 | N38 x4 | -2.3 | slide x3 | a last leg that does not reach its point |
| bf2_kam1 | (-90, 69, 198) | 2 | 2 | last->65 dest 100 [table] x1 | N65 x2 | -1.3 | slide x2 | a last leg that does not reach its point |
| bf2_kam1 | (-66, 60, 276) | 2 | 2 | 53->55 dest 2 [table] x1 | N53 x2 | -1.2 | fan +90 x1 | a last leg that does not reach its point |
| bf2_kam1 | (-60, 60, 330) | 2 | 2 | last->54 dest 100 [table] x1 | N54 x2 | -0.5 | fwd x1 | a last leg that does not reach its point |
| bf2_kam1 | (-36, 60, 102) | 2 | 2 | 45->46 dest 86 [table] x1 | N45 x2 | -0.0 | back x1 | a last leg that does not reach its point |
| bf2_kam1 | (42, 51, 12) | 2 | 2 | 38->39 dest 55 [table] x1 | A38 x1 | +0.1 | slide x2 | on the arc at its height, no progress |
| bf2_kam1 | (42, 51, 18) | 2 | 2 | 38->39 dest 89 [table] x1 | N38 x2 | -1.5 | slide x2 | a last leg that does not reach its point |
| bf2_kam1 | (84, 51, 78) | 2 | 2 | 41->14 dest 89 [table] x1 | N41 x2 | -0.6 | slide x2 | a last leg that does not reach its point |
| bf2_kas2 | (90, 18, -114) | 11 | 7 | 28->52 dest 52 [table] x4 | A28 x11 | +1.8 | slide x10 | on the arc at its height, no progress |
| bf2_kas2 | (144, 9, -48) | 3 | 3 | 18->3 dest 2 [table] x2 | A18 x3 | -2.7 | slide x2 | on the arc at its height, no progress |
| bf2_myg1 | (-66, 3, 252) | 3 | 2 | last->95 dest 95 [straight] x3 | H32 x3 | +0.0 | slide x3 | a last leg that does not reach its point |
| bf2_myg1 | (-66, 3, 258) | 2 | 2 | last->15 dest 15 [straight] x1 | H32 x2 | +0.0 | fwd x1 | a last leg that does not reach its point |
| bf2_pol1 | (18, 24, 84) | 20 | 8 | 3->21 dest 7 [table] x13 | A3 x12 | -11.9 | slide x15 | STARC's radar door |
| bf2_pol1 | (18, 24, 78) | 3 | 3 | 3->0 dest 146 [table] x1 | A3 x2 | -11.9 | slide x2 | STARC's radar door |
| tat1 | (-264, 12, 48) | 10 | 4 | last->104 dest 104 [table] x10 | H104 x10 | +2.9 | slide x10 | a last leg that does not reach its point |
| tat1 | (-60, 15, 54) | 2 | 2 | none x2 | A96 x1 | +0.1 | slide x2 | on the arc at its height, no progress |
| tat1 | (-6, 6, 138) | 2 | 2 | none x2 | A33 x1 | -1.7 | slide x2 | on the arc at its height, no progress |

## Note from OFFG (2026-10-02) - what OFFG's ramp fix left, for this card
- OFFG found Kamino's stall was the steering calling a bridge's 32 degree end ramp a wall (`steer::ramp_clear` now);
  BF2 KAM1 recycles 35 -> 7 over 8 seeds. `UNIT_SAMPLES` now has each unit's leg, and
  `lead_tmp/offg/tools/legs.py <csv>...` ranks the legs bots give up before their hub - use it on POL1 / GEO1 first.
- KAM1 legs still given up after the fix (8 seeds, `lead_tmp/offg/r1/samples`): 44>46 at (-12, 60, 84) 734 unit-s in
  one cell; 46>42 / 46>45 / 46>44 around (-15, 61, 102); 121>122 at (30, 61, 132) 317 unit-s; 100>103 and the last leg
  to hub 100 (cp0's defend spot, (-184..-198, 68, 30..36): hub 100's own line has no floor, `floor_dy -128`, 3 of the 7
  recycles left); 88>89 at (114, 52, 144) 2 recycles.
- The RCYC spots OFFG's card listed on OTHER maps are not ramps and did not move (8 seeds, base -> fix, noise level):
  BF1 BES1 (-12, 108, -240) 1 -> 4, BES2 (-246, -33, 90) 0 -> 3, END1 (168, -3, 0) 3 -> 7, KAS1 (30, -27, -210) 7 -> 4,
  BF2 DEA1 (-162, 51, -150 / -162) 8 -> 3, TAT2 (126, 0, 60) 2 -> 6, (102 / 96, 0, -12) 5 -> 7. They are this card's
  family (a leg that cannot be walked as a straight line); BF2 GEO1's are all `last->9` under cp1, 10-13 m below it.

## Note from STLVL (2026-10-02) - the last-leg half has its own card now
- Card **LLEG** (value 5, `deps: STARC` so the maintainer's order holds) takes "a last leg that starts where the capture centre
  is in reach": STLVL's census found the same shape as the largest recycle source left - BF2 GEO1's raised cp6 (one
  ramp) and walled cp7 (one doorway), 60-110 recycles per 8 seeds and the map's only hung rounds. This card keeps the
  arc-line half (POL1 radar door: 24-37 recycles per 8 seeds at (18, 24, 78..84), legs 3->21 / 3->0, after STLVL).
- STLVL changed `route::start_hub` (a soldier routes from a hub on its own storey, `level_hub`) and the stuck
  window's re-plan now calls it: re-read both before editing.
- New referees: `recycle_lv` lines and `FLOOR_GRID` in `round_to_outcome` (its header), `lead_tmp/stlvl/tools/lv.py
  <sweep dir> [map]` lists every recycle spot against the leg hub's resolved floor.
