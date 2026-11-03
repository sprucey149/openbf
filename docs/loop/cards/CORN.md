---
id: CORN
title: Aim points in walls: the corner search walks a leg THROUGH a barrier it ignores (BF2 Tantive IV's leg 20 > 79: 36 of the worst door's 45 stalls, a corner inside the wall beside the pod room door), and a hub point or corner can lie behind a door's jamb - find the rule that cures them without the all-maps cost the first prototype had
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Filed by card DOORS's measure pass (2026-10-12). After MOUTH2 and DOORS a door's LEAF no longer turns bots away; what
is left near doors is the point a bot walks at lying IN A WALL. On BF2 Tantive IV - the worst door map, 52 door
stalls per 20 door bot-minutes against Kamino's 1 - bots at hub 20 turning west walk at a path corner inside the wall
beside the pod room door, meet the corridor, commit a side and go 22 m round through the door. It is what the maintainer's
"they turned and walked into the wall behind" looks like on the maps he has not played yet.

## Evidence (main 4dbb460; kit `engine/artifacts/lead_tmp/doors/`, README there)
- **Tantive IV leg 20 > 79, traced** (`tools/urows.py` on `snap/bf2_tan1_main/bf2_tan1_s64_t1800.json` unit 25, t
  11.5-22; `tools/meet.py`: 22 of the map's 70 "going round" door stalls in 8 seeds walk at this one corner). The
  straight leg crosses `barrier219` (a 4 x 1.6 m barrier askew in the corridor at (-282.7, 149.8)). The bot's point of
  hub 79, (-298.89, 148.96), is 0.90 m outside `barrier33` (the wall north of the corridor) - inside the search's
  radius (`DETOUR_RADIUS` = the capsule's 0.5 + 0.5 = 1.0 m), so `NavGraph::leg_ignores` drops barrier33 for the
  whole leg, and the chain round barrier219 comes back as (-278.90, 151.68), (-282.84, 154.05): both INSIDE
  barrier33. Printed by the scratch switch `DOORS_DET=-277,148,3` (`scratch_knobs.patch`): 2 of 4 searches from hub
  20 answer so (goal points 0.90 and 0.40 m outside the barrier); a goal 1.27 m outside gets "no chain" and the bot
  walks straight. The PS2 tests containment with the unit radius alone (`ai.rs`, `DETOUR_RADIUS`'s note): by that
  test the 0.90 m goal is not ignored and the 0.40 m one still is.
- **Leg 24 > 25** at `rpodroom2`: corners (-294.4, 175.9) and (-298.7, 175.9) against `tan4_bldg_pod_b` (13 round
  stalls) - not traced further.
- **Aim points behind a jamb** (a hub point or a corner outside a door's opening; the maintainer's "centre line" ask, DOORS
  rule (a), not built): Tantive IV's tech-room doors (legs 101 > 102, 96 > 21, 12 > 13, 14 > 15: about 12 door stalls
  in 8 seeds), Kamino `kam_bldg_podroom_door24` (leg 88 > 87, the bot's point of hub 87 4.0 m off the centre of a
  7.5 m door; 2 stalls in 26 near visits), the Death Star's leg 4 > 1 corner (-56.1, -77.8), 6.9 m off the centre of
  a 10 m blast door (4 of the 5 leaf meetings that still end badly after DOORS, one bot).
- **A first prototype is measured and NOT good enough** (`DOORS=2` in `bin/rto_k2`: a corner inside the plain
  rectangle of a barrier the leg ignores is no node): Tantive IV door stalls 192 -> 140, its round class 70 -> 32,
  the map's P-stalls 1000 -> 913 (8 seeds x 3 min) - but all 32 maps move, and over 8 seeds x 2 min of snapshots
  S-stalls 2566 -> 2581 (paired t +0.15), P 8977 -> 8854 (t -1.19), Utapau's S 391 -> 471, Yavin 4's 70 -> 91,
  Endor's 139 -> 106, Kashyyyk's 82 -> 61; recycles 39 -> 43 over 8 sweep seeds (Tatooine 8 -> 14, Kashyyyk 9 -> 4):
  `out/ab_k2.txt`, `sw/a_0` against `sw/a_2`.

## What to do (measure first)
1. Census the corner search's own output on all 32 maps (RULES: census the PLAN at its point of use): of the
   crossing legs, how many get a corner inside ANY barrier's plain rectangle, how many of those inside one the leg
   ignores, per map; and how many legs ignore a barrier only because of the 0.5 m the port adds to the PS2's radius.
2. Variants behind switches in one binary (null = main byte for byte): (i) the prototype; (ii) containment for
   `leg_ignores` with the capsule radius alone (the PS2's test), crossings as now; (iii) an ignored barrier is
   ignored only for the segment that ends at the point it holds; (iv) for a leg that crosses a door's opening, the
   aim point clamped into the opening (DOORS rule (a)) - judged by Tantive IV's tech-room doors.
3. Trace Utapau and Yavin 4 under the winner before believing a gain (the prototype's rise there is unexplained).

## Acceptance
- Tantive IV: door stalls per 20 door bot-minutes (`lead_tmp/doors/tools/census.py`, 8 seeds x 3 min; main 4dbb460
  60.7, bc10b82 51.3) at most 35, and the leg 20 > 79 corner cluster (`tools/meet.py`) gone.
- All 32 maps, `mouth/tools/ab.py` over 8 seeds: S-stalls not up (paired t under +1), no map's S up by over 20 %
  without a trace; 24 seeds x 32 maps paired: rounds 768 / 768, recycles, lost and floor deaths not worse; the maintainer's
  Kamino scene not worse (run it on the FIRST variant that shows a gain).
- A test per rule from the Tantive IV barriers' own numbers (barrier33, barrier219, the two end points), each
  mutation-checked.

## Measured 2026-10-22 (lane 2, main 9193a4c; kit `engine/artifacts/lead_tmp/corn/`, README there)
Switch binary `CORN=<bits>` (null = main hash for hash on 4 maps; `tools/apply_knobs.py` + bit 16).
- **Census of the search's own output** (`CORN_CENSUS=1`, 32 maps x seeds 1-2, `out/census_main0.txt`): 1,035 of
  83,076 chains (1.2 %) put 1,254 corners inside a barrier's bare rectangle, EVERY one in a barrier the leg ignores;
  506 of those chains ignore it only by the port's extra 0.5 m. Worst: Kamino 179, Tantive IV 150 (barrier33's pair
  at (-283, 154) / (-279, 152): 89 + 89), Mygeeto 77, Tatooine 2 77, Utapau 74. Depth inside the barrier
  (`out/wall_depths.txt`): 363 within 0.5 m of its edge (Kamino's 171 at 0.1-0.25 m), 891 deeper. 572 sit in a
  barrier holding the start, 682 the goal.
- **Variants**, each: Tantive door stalls / 20 door bot-min (today's main 37.4); S-stalls pooled over 3 blocks x 8
  seeds x 2 snapshots (main 7,217; paired t over 96 map-blocks); 24 x 32 sweep: rounds, recycles (main 768, 88).

  | variant | Tantive | S | t | rounds | recycles | scene >6 rev |
  |---|---|---|---|---|---|---|
  | (i) bare rectangle of an ignored barrier: no node | 29.5 | 6,744 | -4.0 | 767 | 98 | 2 / 387 |
  | (ii) containment at the capsule's 0.5 m | - | - | - | - | - | - (no-way searches 4,832 -> 30,839: dropped) |
  | (iii) ignored only on the segment at its end | 25.8 | 6,696 | -3.4 | 768 | 118 (t +1.7) | 1 / 424 |
  | (iii) + that segment tested at the end's own clearance | 27.4 | 6,472 | -4.7 | 767 | 126 (t +2.9) | 1 / 378 |
  | **(vi) deeper than CORNER_CLEARANCE inside an ignored barrier: no node** | **33.5** | **6,834** | **-3.7** | **768** | **96 (t +0.9)** | **0 / 421** |
  Main's scene 0 / 419. (vi): S down in every block (t -2.1 / -2.2 / -2.2), P 26,164 -> 25,518; lost 6 -> 3, floor
  52 -> 50, falls 18 -> 20 (t +0.4); leg 20 > 79's and leg 24 > 25's corner clusters 0 stalls (`meet.py`).
- **Why (vi), not the prototype (i):** (i)'s Tatooine 1 rise (S 49 -> 62) is unit 23 at t 35.5 s in 13 of 24 seeds
  (the opening) on leg 16 > 17: main walks its corner (-258.3, 149.0), 0.08 m inside barrier1466, straight up the
  ramp; (i) refuses it and the search goes round the other way, 8 s stalled. A barrier's rectangle near its edge
  can cover floor the arc uses; Tantive's corners are 0.65 and 4.19 m inside barrier33.
- **Traced rises under (vi):** Felucia S 38 -> 52 = one life (seed 70 unit 30, 8 stalls at (103, -71), legs 3 <-> 1;
  main's bots stall there too, seeds 70 / 71 / 83); BF2 Kashyyyk recycles 15 -> 25 = main's ledge trap at (46, 27.8,
  -108.6) (leg 48 > 47, 11.5 m over hub 48's floor; its searches answer "none" on both builds; unit-s there 360 ->
  470, lives 187 -> 188) + 5 singles. Utapau (iii)+shrink: main's pen at (-187, 9.4, 133) (main block b 85 unit-s).
- (iv) the door-opening clamp: NOT measured this pass. What is left at Tantive's doors after (vi) is hub points
  behind the tech-room jambs (legs 101 > 102, 12 > 13, 96 > 21) - DOORS rule (a)'s case.

## Built 2026-10-22 08:27 (lane 2; branch `corn` fd24062 on main 8a3680b)
Rule (vi) in `DetourSearch::expand` (the search keeps the leg's ignored barriers; the goal is never refused). The
branch binary = the switch binary's `CORN=16` hash for hash on 6 maps. Tests from the bakes' rows, each
mutation-checked (each mutation fails its own test only): `a_corner_deep_in_a_barrier_the_leg_ignores_is_no_node`
(Tantive's corridor: barrier33, 219, 192, 42; rule off -> main's corners back), `a_corner_at_the_edge_of_an_ignored
_barrier_is_kept` (Tatooine's ramp; margin 0 -> the long way round), `a_goal_deep_in_an_ignored_barrier_is_still_
reached` (goal exemption off -> None). ci EXIT=0 (1835 / 0 / 53 = 1832 + 3). Gates (GPU-free groups, the RTX is lost;
`lead_tmp/corn/gates2`): sim PASS (motion identical), simhash PASS, sweep MOVED (declared), fidelity PASS (561
values, 1 better), arena_flat PASS; arena maps + leak NOT run (STOP at 08:33, stopped by pid).

## Notes
- 2026-10-12 DOORS: the kit's `tools/classify.py` splits a door's stalls by cause; 48 % are bots standing in a
  firefight (`hold` 2 / `fight` within a second) - count them out before reading a door's stall number.
- `core/src/nav.rs` is also OBSV2's / TGAP's / FLNK's neighbourhood (`route.rs` calls the search): check what is in
  flight before building.
