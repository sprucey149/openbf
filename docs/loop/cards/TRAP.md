---
id: TRAP
title: Endor's log trap (end_weap_treesmash, base trap, both games): collision primitives parented to a bone are baked bone-relative and land at the instance origin; place them by the bone's rest transform, then RE and build the trap itself
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card VDROP (2026-10-03) found the two log cylinders of end_weap_treesmash (p_log / p_log1, parents bone_l_tree /
bone_r_tree, radius 0.5, length 8) lying crossed on the ground at the instance, on the speeder bikes' path: 160 of 171
unowned rider losses in 8 BF2 END1 rounds were there. VDROP gives a class of base `trap` no body at all
(`Scene::collision_parts`, PROVISIONAL). The logs belong up in the trees (the bind pose puts the trees +-11 m out,
`bake/src/bind_pose.rs`), and the trap itself (trigger, reset, com_holo_trap, a build zone) is not built.

## Evidence
- `analysis/re-hover-ai.md` 12b; `lead_tmp/vdrop/loss_law1_s*.txt` (every loss with its spot);
  `assets_baked/bf2_end1/manifest.ron` (the geometry's two prims; the two instances at (-75.2, -3.5, 78.8) and
  (-89.2, -3.5, 145.2)).
- PS2: bikes pass 0.3-0.8 m from both origins at 28 m/s (BF2, 37 passes in 5 min) and 0.44 m at 39.5 m/s (BF1):
  `lead_tmp/hais/live_end1.csv`, `live_b1end1.csv`.
- 64 collision prims in 9 maps have a bone parent (most are doors, placed by the door rig); the others to check:
  all_hover_skiff's rudders, bes2_carbon_forklift, geo_bldg_technounion's tanks, mus1's windows and shield.

## Acceptance
1. RE (bounded): what the PS2's trap is on both discs (when the logs fall, what they hit, the build zone), and where
   the two cylinders are at rest - a PCSX2 frame of each trap and a RAM read of its bodies.
2. The bake places a bone-parented prim by its bone's rest transform; `leak_census` new 0; the two logs' world
   positions printed beside the PS2's.
3. 8 seeds BF2 END1 and BF1 END1 with the bodies back: riders lost with no killer stays under 1 a round on BF2 END1
   (`lead_tmp/vdrop/hov.py`).

## Planner 2026-10-03 07:15 - `defer: post-alpha`
The backlog cap: Endor is not one of the six showcase maps, the card is value 3 and blocks no alpha card. What is on
main (VDROP's "a class of base `trap` gets no collision body", PROVISIONAL) keeps the bikes passing as on the PS2;
what is missing is the logs' real place and the trap itself. Reverse: `loopctl.py set TRAP defer=`.
