---
id: HTRN
title: BF1 Hoth: rebels pace against the trench props east of cp4 and are recycled (hub 4, legs 4->39 and back) - 56 recycles in 32 rounds after BSPN, 17 before it at the same cells
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
**Correction, same day:** the counts in the title are of parent ff68507. On BSPN's branch rebased over FGHTA (b75d36b) the gate's seeds 1-8 give BF1 Hoth 3 recycles (`lead_tmp/bspn/land/branch/fidelity`). The traced stall below is real; its rate today has to be re-measured first (32 seeds on main).

Card BSPN's sweep (the PS2's spawn draw) took BF1 Hoth's recycles from 6 to 37 over seeds 1-8 and from 11 to 19 over seeds 9-32; the spots are the ones the parent already recycled at, so the spawn rule changed the traffic, not the defect.

## Evidence
- `lead_tmp/bspn/sweep_fin/seeds.md` (recycles table, bf1_hot1 rows): team 1, cells x -294..-228, y -9, z -48..6, locus A4 / H4, legs `4->39 dest 40`, `4->6 dest 27`, `last->4`; steer fwd / slide; floor dy -1.0..-3.6.
- Snapshot `lead_tmp/bspn/bb/bf1_hot1_s2.json` (seed 2, recycle of unit 24 at tick 22633; build ff68507 +dirty = branch bspn): eleven rebels with a blocked-steering run of 3.5-18.5 s, every one on body 8 `hoth_prop_trench_turrets trenchturrets @ (-256.0,-8.1,-16.0)` or body 290 `hoth_prop_trench_short trenchshort @ (-212.0,-8.1,-27.0)`; unit 24 walked 41.6 m for 17.2 m net with 2 reversals, aiming at the leg's point (-117.1, 5.4, -2.0) 118 m off.
- Not classified yet: whether the leg's straight line crosses the trench wall (a route defect: hub 4's point or the 4->39 arc) or the way round exists and the steering loses it.

## Acceptance
- The stall is classified from a traced case (route through a wall / steering / pen), with the PS2's hub 4 and its connection read from the disc beside the port's.
- After the fix: BF1 Hoth recycles over 32 seeds (`sweep_seeds.py --seeds 1-32 --maps bf1_hot1`) at or under 5, from 56; the 8-seed sweep's other maps unchanged or declared.

- 2026-10-09 12:45 planner. `defer: post-alpha`: BF1 Hoth is not a showcase map (BF2 Hoth is), value 3, and the rise that filed it was gone after the rebase over FGHTA (the BSPN report). Undo: `loopctl.py set HTRN defer=`.
