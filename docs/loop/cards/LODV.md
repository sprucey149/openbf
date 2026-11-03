---
id: LODV
title: Vehicles take the far scene's model / LOWD switch: a vehicle past NearSceneRange[1] is drawn as its LOWD (67 vehicle LOWDs in side_bf1/2), as card LOD does for world instances
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card LOD switched world instances and path props only; vehicles (per-frame draws in viewer/vehicles.rs) still draw their model at every distance on far-scene maps - a named deviation in LOD's report. The PS2 far scene draws every LOWD-bearing geometry as its LOWD past NearSceneRange[1] (analysis/re-lod.md §7). Also a speed item for weak GPUs (card ISPD).

## Evidence
side_bf1/side_bf2 bakes hold 67 vehicle *LOWD.glb (e.g. cis_hover_aatLOWD, all_fly_xwingLOWD); render::LodSwitch + look::lod_bands are on main after LOD.

## Acceptance
A test: a vehicle draw list on a FarSceneRange>0 map carries the model with LodSide::Model and its LOWD with LodSide::Lowd (none on a FarSceneRange(0) map), mutation-checked; a pose_render of a parked vehicle at ~1.5x NearSceneRange[2] shows the LOWD; goldens predicted (likely none: no golden has a far vehicle).
