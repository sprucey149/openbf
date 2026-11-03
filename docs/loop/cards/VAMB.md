---
id: VAMB
title: BF1 vehicles (non-static models with a shdw volume) are lit by SkyInfo VehicleAmbientColor — the port lights them with AmbientColor; bake the key and give those draws a per-draw ambient (CAMB's LightPath::Character pattern)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

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
