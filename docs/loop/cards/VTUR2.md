---
id: VTUR2
title: BF2 building turrets: bake SIDE/TUR.LVL's classes (tur_bldg_laser, _tower, _chaingun_*, _recoilless_*, _hoth_*, _beam, _mortar, tur_weap_built_gunturret — 333 instances on BF2 maps) with their models and skel nodes, so vehicle_pads::add_turrets spawns them as VTURB turrets
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
VTURB (2026-09-29) made every armed-building instance a seat-only vehicle, but BF2's building turrets resolve to no class: `tur_*` classes live in `extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/TUR.LVL`, which no class bake reads (`assets_baked/classes/bf2_*.ron` = all, cis, imp, ingame, rep). Instance counts (BF2 manifests): tur_bldg_laser 122, _tower 35, _chaingun_tripod 31, _recoilless_lg 26, _chaingun_roof 25, _hoth_dishturret 24, tur_weap_built_gunturret 27, _hoth_lasermortar 18, _beam 8, others ≤ 5.

## Evidence
re-vehicles §8 (BF2 armedbuilding FUN_001e8860 → FUN_001e8948 → FUN_00270770; portableturret / defensegridturret are armedbuildingdynamic). BuildingSection hash in B2 FUN_001e8948 B2:97129. Report `engine/artifacts/loop/reports/20260929_*_card_VTURB.md`.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F52): the PS2 DRAWS these — BF2 HOT1's defence towers stand beside cp6 on the PS2 and are absent from the port's frame (`pcsx2/ref/bf2_hot1/walk/pairs/cp1_cp6__sheet.png` b180: `tur_bldg_hoth_lasermortar` turet11, 21.9 m at yaw 155; b270: turet12, 22.1 m at yaw 282; `cp2_cp3__sheet.png` b270 more on the plain). The bake has no model for any `tur_bldg_*` instance, so today they are invisible as well as unboardable.
