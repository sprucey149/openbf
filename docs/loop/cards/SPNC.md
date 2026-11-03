---
id: SPNC
title: Every spawn node on the 32 maps against the route it starts on: Dagobah's node at (-20.6, 4.0, -45.1) starts a bot on a leg whose hub is 8 m below it (card DAGS) - list every node whose first leg's hub the floor walked from the node does not reach, per map and post, so the first-leg rule is checked everywhere and not found one playtest at a time
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card DAGS found one spawn node whose first leg cannot be walked. The PS2's spawn draw (card BSPN) gives every post an equal
share, so a bad node in a small post now takes a whole post's share of a team's spawns: the same fault on another map would
show the same way. The maintainer, 2026-10-11: bots must not stand for minutes after a respawn, on any map.

## Evidence
- Card DAGS's trace (`lead_tmp/dags/`), card BSPN's refused landing (`loop/reports/20261011_1643_land_BSPN.md`).

## Notes
- Depends on DAGS so it measures the rule that card lands; the BEFORE column can be taken from main at once.
- 2026-10-12 card MOUTH (measure): **a spot for the census, BF1 Bespin Platforms (-21.1, 98.7, -187.6)** (unit 8's spawn,
  all 8 seeds): `stop wall 0.00 bes1_bldg_main02` on its first frame, first leg `2->6` walked along the wall, then it
  never gets through the door at hub 8 (-25.8, -170.8). Trace: `lead_tmp/mouth/snap_bes1/bf1_bes1_s60_t1800.json` unit 8;
  the note on RLOOP has the loop.
- 2026-10-12 14:01 card RLOOP (build): **a spot for the census, BF2 Kamino (-97.8, 61.3, 317.5), every round.** The bot spawned there (unit 25, generation 1, at 1.3 s in 16 of 16 seeds on main 4ca302f; other lives from the node later in the round) is LOST from its first tick (`level 7.9`, leg `54 -> 55`), walks 5 m toward its return point, stops at a ledge (`steer ledge STOP Drop at 1.00 m`) at (-92.8, 61.8, 317.8), stands 44 s and is given up as lost: 33 of Kamino's 36 standing spells in 16 seeds, and most of its 31 `lost` deaths. Trace: `lead_tmp/rloop/snap/main_4ca302f/bf2_kam1_s1_t900_u25.txt`; rows `lead_tmp/rloop/sw/main16_loops.txt`. RLOOP's rule (not landed) only shortens the stand to about 22 s; the cause is the node or the level test there. Kamino is a showcase map.

- Planner 2026-10-12 14:22: **value 4 -> 5.** Two nodes are already known to strand a bot EVERY round, one on a map the maintainer
  plays: BF2 Kamino (-97.8, 61.3, 317.5) (16 of 16 seeds; 33 of Kamino's 36 standing spells and most of its 31 `lost`
  deaths in 16 rounds) and BF1 Bespin Platforms (-21.1, 98.7, -187.6) (8 of 8 seeds; 351 of that map's 566 stalls). DAGS
  is on main (4ca302f), so the "after" column is today's main. Undo: `loopctl.py set SPNC value=4`.
- From DAGS's report, for the census: the HUNT destination in `route::retarget_phase_a` still asks the bare XZ locate
  (its twin in `retarget_hunt` has the storey rule); switch `DAGS=2` showed no effect on Dagobah and was not run on all maps.
- 2026-10-15 (build, lane 2): **the census is built - `game/examples/spawn_node_census.rs` (branch `spnc` 9f688b1, every
  gate PASS; land.py refused on a dirty main, see `resume:`).** 1,455 nodes on 32 maps in 7 s. The first leg is asked from
  today's start hub (AFTER) and from the bare locate before DAGS (BEFORE) through the real `plan_leg`
  (`core::ai::first_leg`). A hub counts as reached by the straight floor walk (`walk_floor`) OR by a soldier's floor
  flood whose walk is at most 2x the distance + 10 m (`Sim::floor_flood`). The card's literal test (walk_floor alone)
  failed 20 nodes after the rule, and 0 of them stranded bots (main fdebf98, 16 rounds, `lead_tmp/spnc/main16`): a trunk
  or a log on the straight line. A flood with no length limit passed DAGS's own node, so the limit stays in.
  **BEFORE 9 failing, AFTER 6.** The rule fixed Dagobah cp4 node 2 (DAGS's: 10.1 % of lives with a 20 s+ stay -> 0 %,
  DAGS's own A/B samples), Utapau con_cp7 node 2 and Tatooine 2 cp7 node 3 (neither stranded bots before it). The 6
  left are BF1/BF2 Endor's bunker deck (cp5 nodes 6/7/8 at y 9.5 over hubs at y -3) and BF1 Endor cp6 node 7. Their
  bots walk down the deck's ramp (0-11 % of lives still at the node 12 s on, median lives 33-66 s): accepted, each one
  on `spawn_node_census_accepted.txt` with its numbers. No node the PS2 also strands, so no capture is owed.
  - Not seen by a first-leg census, by design: Kamino cp3 node 8 (level Off(8.3) from tick 1; 28 of 64 lives in a 20 s+
    stay; note on RETW) and Bespin Platforms cp9 node 7 (-18.5, 99.0, -187.2) (leg 2 -> 6 reached on the floor, a wall
    at 0.4 m: the door, cards MOUTH / MOUTH2). The census prints the level test and the first wall per node for these.
  - The gate line is card SPNCG (value 3).
