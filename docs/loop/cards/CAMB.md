---
id: CAMB
title: BF1 soldiers are lit by SkyInfo CharacterAmbientColor, not AmbientColor — the port uses AmbientColor for every draw (9 of 14 BF1 maps differ: GEO1 d19c49 vs 78654c, HOT1 a6a9ad vs ffffff)
milestone: M1
value: 4
phase: build
status: done
deps:
files: engine/crates/render/src/{shader.wgsl,draw.rs,frame.rs,lib.rs,lighting.rs}, engine/crates/game/src/{soldier_vis.rs,viewer/look.rs}
declares: goldens=maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_yav1.png,maps/tat1.png
needs:
branch: camb
resume: landed 86d23b5; vehicles → VAMB
created: 2026-09-28
---
## Why
BF1's model draw gives each kind of draw its own ambient (`analysis/re-model-lighting.md` §12, [O], card FOLGL):
- `FUN_00496248` → `FUN_00493cf0` gives a NON-static model `DAT_0069c578` = SkyInfo `CharacterAmbientColor`.
- A non-static model with a `shdw` volume (model +0xb8) takes `DAT_0069c580` = `VehicleAmbientColor` instead.
- Static draws (world props, foliage) take `AmbientColor`.

The port lights every draw, soldiers included, with `AmbientColor`. 11 of 14 BF1 maps author `CharacterAmbientColor`,
and on 9 it differs from `AmbientColor`. RAM values:
- GEO1: d19c49 against 78654c, so the PS2 soldier is lit brighter and more orange.
- HOT1: a6a9ad against ffffff, so the port's soldier is too bright.
- END1: 8c8c78 against 646e64.

## Evidence
PS2 patched states, BF1 GEO1 spawn (`engine/artifacts/lead_tmp/re/folgl/ps2exp/sheet_geo1.png`, the recipe in
`ps2exp/mkstates.py`; two controls agree):
- 0x69c578's RGB zeroed turns the SOLDIER black and leaves the world as it is.
- 0x69c570 zeroed darkens the rock spires and the wing and leaves the soldier.
- 0x69c580 zeroed changes nothing in view (no vehicle nearby).
