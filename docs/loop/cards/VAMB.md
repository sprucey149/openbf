---
id: VAMB
title: BF1 vehicles (non-static models with a shdw volume) are lit by SkyInfo VehicleAmbientColor — the port lights them with AmbientColor; bake the key and give those draws a per-draw ambient (CAMB's LightPath::Character pattern)
milestone: M2
value: 3
phase: build
status: blocked
deps: CAMB
files: engine/crates/bake/src/lighting.rs, engine/crates/manifest/src/lib.rs, engine/crates/render/src/shader.wgsl, engine/crates/game/src/viewer/vehicles.rs
declares:
needs: restore BF1 raw shdw census inputs and paired vehicle PS2 reference before retry
effort:
branch:
resume: 20261025_0503 input hold 0/38; no source change; restore inputs and run census/vehicle comparison; see engine/artifacts/loop/reports/20261025_0503_card_VAMB.md
created: 2026-09-28
defer: post-alpha
tries: 1
---
## Why
BF1's light-set builder FUN_00493cf0 gives a NON-static model with a shadow volume (model +0xb8, the modl loader FUN_00497150 stores the shdw chunk there) … vehicles still take AmbientColor.
## Evidence
re-model-lighting §12; mungefs::sky parses VehicleAmbientColor but bake::lighting drops it. No PS2 frame yet (the GEO1 0x69c580 patch had no vehicle in view).
## Build
1. Bake vehicle_ambient beside character_ambient (defaults to AmbientColor when absent, as FUN_00493298 copies it); while in manifest/lib.rs fix character_ambient's doc ('Not consumed yet' is stale since CAMB). Rebake.
2. [H] Census the BF1 bake for which models carry shdw (vehicles by name); a LightPath::Vehicle (or a per-draw ambient) on those draws, BF1 only.
3. Confirm with a PS2 patched state with a vehicle in view (0x69c580 zeroed; recipe lead_tmp/re/folgl/ps2exp/mkstates.py).
## Acceptance
A BF1 vehicle within a few levels of a PS2 frame; world props and soldiers unmoved.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
