---
id: WSKN
title: The walker drawn alive: the skinned walker GLB (VBAKE left walkers rigid) and its clips played from WALK's anim state — the idle/leftup/rightup layers, the legs' planted turn steps, the pitch/roll bone
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Walkers draw rigid in their skel bind pose (VBAKE). After WALK an AT-ST would slide across the ground without
stepping: its feet never plant and its turn steps are invisible.

## Evidence
- analysis/re-walker.md §5: the 11-entry clip table, the three layers (base, _leftup, _rightup) and the transition
  table DAT_005fd100.
- §6: each leg pair's leftup/rightup blend P+0x120.
- §10: the leg offset is a yaw applied to bone 0, so the model stays planted while the body frame turns. The
  pitch/roll quaternions go on bone 0x16e27226.
- The VBAKE note (card VWLK, bake/src/vehicle.rs module doc): each walker model has a same-named skel.
  rep_walk_atte carries MTRL 0x07 / 0x10003 (the 0x02 alpha test) that bake::soldier::segment_blends refuses.
