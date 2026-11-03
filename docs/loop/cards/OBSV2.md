---
id: OBSV2
title: A walk point that lies in a map body is moved to where a soldier can stand (a detour corner, a hub's centre, a leg line's start) - the cure of com_weap_gunturret's contact time (688 s -> 13-26 s per 256 rounds: 677 s of it is ONE turret on BF1 Endor, in whose middle a detour corner of the leg from hub 18 to hub 0 lies): built behind a switch and measured by card OBSV, held back because the bots it frees walk on into Endor's hollow tree (card ETREE)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
Split off card OBSV (2026-10-12): OBSV's bar 'com_weap_gunturret's contact time halved' is not a footprint question.
Traced (`lead_tmp/obsv/bb/end1_s2_u30.txt`, `bb/end1_s7_u24.txt`): the bot's aim point is a DETOUR CORNER at
(198.41, -31.45), 0.36 m from the turret's middle (198.6, -31.8); it gets to 1.04 m of it (the corner radius is 1.0 m)
and walks round the turret - going round, so not stuck - until its allowance is spent. 90 % of the turret's contact
time has an aim point no soldier can stand on (27 % of the time at empty vehicles, 18-20 % at small and mid bodies).

## What is built (not landed): `lead_tmp/obsv/obsv_scratch_knobs_k9.patch` on main 17ae28e, switch OBSV=4
`route::stand_point`: a point the wall pass moves a soldier from (over 0.05 m) is moved out by up to six passes + 0.3 m,
kept only where a soldier then stands on a floor within a step of the point's; applied to a hub's centre in
`hub_point` / `centre_leg`, to a leg line's start, and to a detour corner on the unit's own level (asked a step over
his feet: from higher the floor under a corner in a body is the body's roof - the first form put the corner on the
turret's top and changed nothing).

## Measured (lead_tmp/obsv/census/k7_4, k7_5; ab/t_*)
- 8 seeds x 32 maps x 10 min: com_weap_gunturret 688 -> 26 s (alone), 13 s (with OBSV's rule); small bodies 2,272 -> 1,695 s.
- BF1 Endor recycles, seeds 9-80: 37 -> 82 (alone), 112 (with OBSV's rule and its arrival half) - the hollow tree
  (`dth/e4_*.err`: 18 of 36 at (168, 0) for 5 of 15 on main). Mos Eisley 168 -> 177.
- Stalled 10 s windows over all maps 1,549 -> 1,700 / 1,731 (8 seeds; like variants ran 1,483 - 1,737: not resolved).

## Acceptance
- After card ETREE: the switch on, 24 seeds x 32 maps paired by seed - rounds, recycles, lost and floor deaths not worse;
  BF1 Endor's recycles over seeds 9-80 not over main's; com_weap_gunturret's contact time at most half of main's
  (`lead_tmp/obsv/tools/census.sh` + `classes.py`).
- A test per walk point (corner, hub centre, line start) that fails under its mutation, one of them a corner under a
  body's roof.
- 2026-10-12 17:05 planner: OBSV now waits for HJAM, so this card's chain is HJAM -> OBSV, ETREE -> OBSV2. Value left at 4.
