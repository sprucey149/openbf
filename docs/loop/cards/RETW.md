---
id: RETW
title: A lost bot walks back along a floor path, not in a straight line at the nearest hub: 34 'lost' deaths in 8 rounds stand on ground a walk leads back from (BF2 UTA1 21 under the y 14.4 landing, DEA1 7, GEO1 4, KAM1 2, MYG1 1, BF1 YAV2 1)
milestone: M3
value: 5
phase: build
status: done
deps: STLVL,OFFG,BOTD,MOVR
files: engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/mover.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/sim/bounds.rs
declares: sim; sweep; arena
needs:
effort:
branch: retw
resume: landed d5ae05f4 2026-10-15
created: 2026-10-02
alpha: yes
---

## Why
Card LVLF (2026-10-02) refereed every 'lost' death with the real soldier: after its landing 36 remain over 8 seeds x 32 maps, and 34 of them stand where a floor walk DOES lead back to a hub. The bot is given up after 45 s (`LOST_RECYCLE`) because its return walk (`Sim::return_point`) aims a straight line at the XZ-nearest hub, and that hub is on a deck straight overhead or behind a wall. A player sees a bot vanish on ordinary ground. UTA1 is a showcase map.

## Evidence (`lead_tmp/lvlf/README.md`, `classify_v6.txt`)
- BF2 UTA1 21: the ground floor (y 9.4) under and in front of the y 14.4 landing, x -143..-134, z 92..111. Bots enter at (-135.8, 9.4, 123) and walk in under the landing (STLVL's case by the look of it; their route was not logged); `return_point` then offers hubs 12 / 13 / 56 / 55, all at 14.4 overhead. The real soldier walks 37-59 m to hub 64 (floor 9.4) in 4-7 s.
- BF2 DEA1 7: (-172..-166, 48.0, -249), 4 m under hubs at 52.0; hub 41 (floor 46.5) is 27-30 m away on the floor flood, but the soldier driven along the flood's path fell (`walkto_main.txt`: to y 25.4 from both spots tried; in `classify` 5 of 7 end at a respawn point, 2 arrive after 23 s): look at this one by hand first.
- BF2 GEO1 4: (71, -72, 118) on a n.y 0.70 slope; 2 of 4 walk 16 m to hub 36, 2 stay stuck (a steep-face trap).
- BF2 KAM1 2: units that SPAWN at cp3's two pod nodes (-100.6 / -103.3, 61.3, 317.5), which the level test reads Off(8.3) (a hub 78 m away stopped 74 m short says 'over'); 41 of 46 such spawns walk out, 2 died at (-92, 61.8, 317.5).
- BF2 MYG1 1: (-25, 0.0, -44) under a y 4.0 walkway; 57 m flat to hub 114. BF1 YAV2 1: (25, -47.0, -36), 1 m under hub floors at -46.0, 2.9 m from ground the test calls on.
- Tried in LVLF and dropped (8-seed runs `lead_tmp/lvlf/v1`, `v2`; code `lead_tmp/lvlf/tools/bounds_v2_return_walk.rs.txt`): `return_point` preferring hubs a STRAIGHT floor walk from the feet reaches, without and with a chest-high wall ray. KAM1's pod deaths went 2 -> 39 and 34 (the straight line at the 'reachable' hub 55 runs into the pod's wall; main's hub 54 is the door), UTA1 21 -> 12 and 10, DEA1 7 -> 10 and 7. A straight line is not enough: the walk needs a path.

## Notes
- LVLF 2026-10-02: `resolve_nav_floors` still takes UTA1's bad spawn node (-46.0, 0.0, 225.2) as an anchor at the sinkhole's floor (-74.8), so the map's kill floor sits 15 m under THAT. Harmless today (the fall kills); fix it with the anchors if this card touches them.

## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

## BOTD (2026-10-03) - adopted as step 2 of the Recover ladder (bot-design.md 6.3); after MOVR
Acceptance unchanged. In BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: value 3 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set RETW value=3`.

- 2026-10-07 04:34 (GRAI landed 79f1890; `reports/20261007_0434_card_GRAI.md`): with the bots' grenades the 8-seed recycles rose 59 -> 82 at spots already listed - bf2_dag1 hub 2 (floor dy -16) 11 -> 17, bf2_pol1 hub 178 7 -> 14, bf1_end1 hub 10 1 -> 6 (`lead_tmp/grai/ab/grai/seeds.md` against `ab/base`). Not traced; a blast's push over an edge is a guess to check first.

- 2026-10-09 17:25 (card OBST2 landed 7dfb448; `reports/20261009_1725_card_OBST2.md`): **BF2 KAM1's pod pad got worse and is this
  card's first case now: 'lost' deaths there 6 -> 50 over 24 seeds (8 seeds 1 -> 18; 4 of 13 pad spawns die for 1 of 10).**
  Traced with the black box (`lead_tmp/obst2/work/bb3/`, units 25 and 26 of seed 1, tick 260): a pad spawn is LOST from its first
  second, walks at hub 54's centre (-71.7, 60.3, 318.9) and gets off the pad only by the lost bot's JUMP over the gap at x -93.
  Unit 26 met the kerb at z 318.3 (a wall: slide, fan, jump at -4 s, on the platform at -1 s); unit 25 met it at z 317.8, where the
  probe says `ledge STOP Drop at 1.00 m`, stood at 0.0 m/s through 11 "jump tries" and died after 45 s. Where it meets the kerb is
  set by the ten ticks of leg walking before the lost state is confirmed - main's corner search (wrong: it walked the leg through
  `barrier867`) happened to send them north-east. What to build here: a lost bot stopped at a LEDGE jumps as one stopped at a wall
  does (or tries the kerb 1 m to either side) - check `behave::lost_jump`'s condition on a `SteerPick::Ledge` first.
- 2026-10-15 card SPNC (census of every spawn node): **Kamino cp3's pod node 8 (-100.6, 62.0, 317.5) is still the worst
  node of the 32 maps on main fdebf98:** level `Off(8.33)` from its first tick, and 28 of its 64 lives in 16 rounds stood
  20 s+ within 1 m in their first minute (43.8 %; the map's other 72 nodes 0.4 %). Node 9 (-103.3, 61.4, 317.5), the same
  level reading, 0 of 35. Its first leg is fine (start hub 54 floor 60.3, next 55 reached by the floor walk): the level test
  is the cause, not the leg. Rows: `lead_tmp/spnc/main16/bq`, `lead_tmp/spnc/tools/nodelives.py`. The census prints every
  level-off node (3 on main: these two and Mygeeto cp4_con node 5, Off(1.18), whose bots walk off).
- 2026-10-15 14:51 (card OBSV, lane 2): **BF2 Geonosis, the cave mouth `geo_prop_cave1` (body 265) at (70.9, -72.1, 117.7)
  is the steadiest 'lost' spot on main b8a634d**: 21 lost deaths in 72 seeds (seeds 1-72 x 32 maps; OBSV's branch 30). One
  trace, the same on both builds (main seed 3 unit 31, branch seed 12 unit 21): leg 37 -> 36 (arc 43, `travel`, `steer fwd`)
  walks down to (71.9, -72.9, 115), is LOST (level 1.7-2.5) beside the cave's wall, and stands at (70.9, -72.1, 117.6) for
  45 s, return points turning between (84.5,-64.5,132.6), (57.9,-67.1,132.1), (81.3,-69.0,77.7), (92.0,-42.7,149.3) with
  `ledge STOP Drop` / `Wall body 265` and 10 jump tries, until LOST_RECYCLE. Snapshots: `lead_tmp/obsv/lost/bbm/bf2_geo1_s3.json`
  (main), `lost/bbb/bf2_geo1_s12.json` (branch); the sweeps `lead_tmp/obsv/ab/{a,c}_main2` (SWEEP_DEATHS=1).
