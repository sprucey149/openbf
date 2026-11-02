---
id: VDRAW
title: Parked vehicles drawn: every pad-spawned vehicle (VPAD) drawn rigid at its sim transform from VBAKE's side models (LOD0, cross-side textures), the pad yaw checked against a PCSX2 frame — every map's vehicles become visible
milestone: M2
value: 5
phase: build
status: done
deps:
files: game/src/viewer/vehicles.rs,game/src/viewer/scene.rs,game/src/vehicle_pads.rs,game/src/walk.rs,game/src/main/{battle,headless,windowed}.rs,game/tests/vehicle_pads.rs
declares:
needs:
branch: vdraw
resume: landed 9f60ffdf 2026-09-28
created: 2026-09-26
---

## Why
VPAD (896b924) spawns every map's vehicles on tick 1 (TAT1: 16), and VBAKE (0a81fc9) baked all 75 side classes, but
nothing draws them: the battlefield has no vehicles in it. Drawing the parked ones is M2's first thing a player sees,
and it needs neither HVRE nor HOVR. Split out of VSEAT (planner 2026-09-26), whose "skiff drawn at its transform" it
generalises.

## Evidence
- `core::vehicle_pad` / `Sim::tick_vehicle_pads` (spawn at the pad, yaw = atan2(r02, r00) PROVISIONAL, height
  `ground_under` PROVISIONAL); `game::vehicle_pads::install` returns `PadBuild::classes` (nothing keeps it yet).
- `side_bf{1,2}/vehicles/<side>/vehicles.ron` + `models/` (VBAKE, rigid in bind pose); map-local classes (TAT1 skiff,
  BES1 cloud car) in the map bake. 13 chunk/LOD models name a texture only another side LVL ships (VBAKE report,
  re-vehicles §10 item 3): the lookup spans every loaded side.
- PS2 frames with parked vehicles: search `engine/artifacts/pcsx2/ref/*/` (TAT1, HOT1, GEO1 spawn poses).
