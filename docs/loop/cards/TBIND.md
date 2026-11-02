---
id: TBIND
title: TUR turret models draw their moving parts at the bone's bind pose: the map bake keeps bone-space vertices (no to_model_space) and bone-relative segments as stored — BF2 HOT1's 12 laser-mortar heads sit at the tower foot, its 12 dishes ~1 m low, YAV1's 7 watchtower guns at the foot (bones 14-15.5 m up), and every bone-space TUR model (KAM1's 21 gun turrets, KAS2/MYG1's 13 antiaircraft, GEO1's 3; BF1 GEO1's 10 geoturrets) in bone space
milestone: M2
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/map.rs,engine/crates/bake/src/glb.rs
declares: goldens=maps/bf2_hot1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_myg1.png,maps/bf2_geo1.png,maps/bf1_geo1.png,maps/tat2.png,maps/bf2_yav1.png,maps/bf1_bes1.png,maps/bf1_end1.png,maps/bf1_hot1.png,maps/bf1_yav1.png,tat1_overview.png; rebake=maps,tat1,tat2,common
needs:
branch: tbind
resume: landed 2d0eda2d 2026-09-30; report reports/20260930_1755_card_TBIND.md
created: 2026-09-30
alpha: yes
domain: render
---

## Why
VTUR2 (a1bf3e8) pulls BF2's SIDE/TUR.LVL models into the map bakes. The map bake writes every segment's vertices as
stored. It never calls `mungefs::model::to_model_space`; only the vehicle bake (vehicle.rs:178) and the soldier bake
(soldier.rs:93) do, and the verifier checked this. So the moving parts of all 71 spawned BF2 conquest turrets sit where
their bone-relative coordinates put them, not where the PS2 draws them. HOT1 and MYG1 are showcase maps.

## Evidence (AUDIT VTUR2 2026-09-30, `lead_tmp/audit-vtur2/`: `skel.py`, `glbbounds.py`, `bonespace.py`, `pose/`)
- bone_space 0 models with bone-relative segments (baked GLB y range vs the BNAM bone's bind height):
  - `hoth_bldg_hoth_turret`: head, yoke and barrel at y −0.43..0.57, bone `turret_y` at 3.57. HOT1 has 12.
  - `yav_bldg_watchtower`: gun and barrel at y −0.26..1.34, bones `cube_y` 14.10, `gun` 15.44, `barrel` 15.50. YAV1 has 7.
  - `tat_bldg_barge_turret`: `turret_01` at 0.30. TAT2 has 3.
- bone_space 1 models, their vertices in bone space (y dips below the ground):
  - `com_weap_gunturret` (BF2 TUR copy; BF1 COMMON's is not bone-space): 21 on KAM1, plus the buildables once TBLD
    builds them.
  - `hoth_bldg_dish_turret`: HOT1 has 12.
  - `kam_prop_antiaircraft`: KAS2 5, MYG1 8.
  - `geo_bldg_geoturret`: BF2 GEO1 3, BF1 GEO1 10. The BF1 GEO1 case predates VTUR2.
- PS2 | port pairs at the PS2's own poses (`pose/*__pair.png`, audit_pose = pose_render + common merged):
  - HOT1 `cp1_cp6_postA_spawn`: the PS2's laser mortar wears its wide head on top; the port's pillar has none.
  - HOT1 `cp1_cp6_postA_v1` (`pose/crop_hot1_dish.png`): the PS2's dish stands on a pedestal; the port's sits ~1 m
    lower, on the snow.
- Placed bone-space models on the ground maps are essentially all turrets. BF1: GEO1's geoturret and END1's treesmash.
  BF2: the TUR models, plus TAT3's torturedroid (`bonespace.py`). Space maps have more.

## Notes
- VTURD (turn the gun where it aims, destroyed geometry) builds on this; this card is its bind-pose first step.

- Planner 2026-09-30 17:30: Raised 4 → 5: the M2 lane's top visible gap on two showcase maps (HOT1's mortar heads at the tower foot, MYG1's antiaircraft) and a small bake fix; M2 had no value-5 card after WSKN.
