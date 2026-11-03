---
id: VBIND
title: The vehicle bake keeps a non-bone-space model's BNAM segments as stored, so their moving parts sit at the model origin: BF2 imp_fly_trooptrans's landing gear and wings, all_hover_hovernaut's turret barrels, all_fly_kwing's cannons, cis_hover_mtt's guns, imp_fly_tiebomber's bomb bay, rep_hover_gian_speeder's turret, all_hover_skiff's guns and rudders, bes_fly_cloudcar's gear doors, all_weap_inf_autoturret's guns
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
TBIND gave the map bake bake::bind_pose: a bone_space 0 model's BNAM segment is stored in its bone's frame and goes through that bone's bind matrix (PROVISIONAL [D], the PS2's HOT1 mortar frame). The vehicle bake (vehicle.rs model_space) still returns a non-bone-space model as stored, so every vehicle whose parts hang off a non-identity bone draws them at the origin.

## Evidence (lead_tmp/tbind/bnscan.py over SIDE/*.LVL; glbbounds.py on side_bf2/vehicles)
- side_bf1 all_hover_hovernaut: the barrel primitives span z 0..0.93 and 0..3.11 at the origin; its bones are 4-11 m out (turret_front_barrel (0, 5.55, 1.07)).
- side_bf2 imp_fly_trooptrans: the landing-gear primitives sit at the origin (-0.39..0.4); bone_l_landinggear2 is at (4.02, -0.39, 0).
- The same pattern holds for all_fly_kwing, cis_hover_mtt, imp_fly_tiebomber and all_weap_inf_autoturret (ALL.LVL), and for the map-local copies TAT1 all_hover_skiff and NAB2 rep_hover_gian_speeder.

## Landed 2026-09-30 (881c108; report reports/20260930_2225_resume_VBIND.md)
- Shared bake after the landing: hovernaut front barrel y 5.41..5.69 (z 1.07..4.18), trooptrans gear foot x 3.02..3.81, y -2.2..-1.74; Hailfire wheels x ±2.53..4.65, y 0.06..7.87. 11 side GLBs DIFF, every other file SAME; no golden moved.
- Not touched, correctly: all_hover_skiff / bes_fly_cloudcar are map-baked (TBIND already poses them); rep_hover_gian_speeder and all_weap_inf_autoturret have no baked model.
