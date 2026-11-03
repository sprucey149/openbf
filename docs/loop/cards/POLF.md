---
id: POLF
title: BF2 Polis Massa: PS2 soldiers stand 0.57 m above the port's terrain on 95 % of 15,719 terrain samples (on it on 4 %; BF2 HOT1 / GEO1 and BF1 HOT1 stand on it) - find what they stand on: a floor the bake drops, a terrain offset, or the PS2's own contact
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On BF2 Polis Massa the PS2's soldiers stand about 0.57 m ABOVE the port's floor wherever that floor is the terrain:
the port's soldiers there would walk half a metre under whatever the PS2's stand on. Found by AUDIT BOWL (2026-10-02)
while running the port's ground under the PS2's own soldier positions; not followed there (outside the target).

## Evidence (`engine/artifacts/lead_tmp/audit-bowl/`; PS2 feet from RAM: `lead_tmp/stall/ps2/bf2_pol1_soldiers.csv`,
`lead_tmp/cont/ps2/bf2_pol1_members.csv`, 91,119 samples of 16 AI a side, the player parked)
- `pts_ps2_layers/bf2_pol1.csv` (the port's floor, terrain and the first soldier- / ordnance-layer face under each PS2
  point): 15,719 samples are over uncut terrain with no model body under them. PS2 feet minus the port's terrain:
  **0.0 m on 610 (4 %), 0.4-0.8 m on 14,882 (95 %), peak 0.6** (0.5: 4,001; 0.6: 9,179). Flat ground (n.y 1.0 on 86 %).
  Cells (24 m): (48, -120) 2,790, (48, -96) 1,727, (-48, 0) 1,536, (-72, -120) 989. E.g. PS2 (-72.400, 21.432, -103.047),
  port terrain 20.900; PS2 (63.930, 10.950, -112.570), terrain 10.320.
- The same reader on other terrain maps (`pts_ps2_terrain/`, PS2 samples from `lead_tmp/wai2/ps2/*_sold.csv`): BF2
  HOT1 23,407 of 30,378 at 0.0, BF2 GEO1 30,262 of 36,085, BF1 HOT1 10,751 of 11,064. So the terrain contact is right
  in general and POL1 is the exception.
- 0.57 m is the PS2 foot sphere's radius (`analysis/re-soldier-ground.md` section 1.2) [H: a coincidence or the cause].
- Not explained by a second layer: no ordnance-layer body under those points either.
- Known about POL1: its terrain lies at 22-49 m over floors at 13-33 m and all 189 hubs have y = 0 (RE_LOG, "Hub y is
  the editor's pick height").

## What to do (measure)
1. One spot, e.g. (-72.4, 21.43, -103.0): the disc's terrain height there against the bake's
   (`analysis/re-terrain-heights.md`; BF2 `FUN_00473db0`), and every collision leaf within 1.5 m in the disc
   (`lead_tmp/starc/tools/colldump.py`, `collcensus.py`).
2. A PCSX2 frame of a soldier standing there (`ref/states/bf2_pol1_*`; `ps2-camera-anywhere-by-ram-hold`) beside the
   port's at the same pose: is the port's soldier sunk into what is drawn?
3. Say which it is — a model floor the bake drops, a terrain offset, or the PS2's own contact — and re-phase to build.

## Acceptance
- The cause named with a disc or RAM citation, and one number for the build: the share of the 15,719 PS2 terrain
  samples within 0.15 m of the port's floor (today 4 %; the build's bar 95 %).

## Notes
- Also seen, smaller: on BF2 KAM1 849 of 93,903 PS2 samples (0.9 %) stand 0.35-1.0 m over the port's floor, on inclined
  floors (n.y 0.667-0.876) with a model body under them, by 0.36-0.44 m; on KAM1's hull slope the offset is 0.10 m (the
  sphere's geometry). `pts_ps2_layers/bf2_kam1.csv`.
- Re-make `pts_ps2_layers`: `tools/patch_main_obs.py` + the `sol_y` / `ord_y` lines (README "Builds"), then
  `AUDIT_POINTS=ps2_pts.csv AUDIT_POINTS_OUT=<dir> round_to_outcome 0 1 bf2_pol1`.
