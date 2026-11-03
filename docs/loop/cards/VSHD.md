---
id: VSHD
title: Vehicle shadow volumes, the data: parse shdw (NAME INFO SHDV SHDI half-edge meshes) and bake each BF1 vehicle's pieces into its manifest - BF1 darkens x0.625 inside sun-extruded volumes, BF2 never draws them (re-vehicle-shadow.md); VSHD2 draws
status: archived
issue: 66
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #66](https://github.com/sprucey149/openbf/issues/66).

## Why (rewritten by the RE pass, 2026-10-16 - `analysis/re-vehicle-shadow.md`)
BF1 darkens the frame under every vehicle that carries a `shdw` volume (27 models: the flyers, hovers and walkers of all
four sides): a stencil-style count of the sun-extruded volumes, then x 0.625 wherever the count is non-zero. BF2 builds
the same volumes and never draws them (its draw slot is an empty function) - no BF2 work. This card is the DATA half:
parse `shdw` and bake each BF1 vehicle's pieces; card **VSHD2** draws them.

## Evidence (re-vehicle-shadow.md 1, 6, 7)
- `shdw` = NAME (the node the piece follows) INFO (vertex count, SHDI short count) SHDV (xyz f32) SHDI (4 u16 per
  half-edge: vertex, next, twin = h ^ 1, 0xFFFF). `lead_tmp/vshd/tools/shdw_check.py` reads all 404 disc chunks with
  no error (BF1 240 / 73 names, BF2 164 / 41).
- BF2 bakes nothing (re-vehicle-shadow 6: empty slot 0x593090 in ELF and RAM).

## What to build
- `mungefs::shdw`: parse a `shdw` (NAME, INFO, SHDV, SHDI) into {node name, positions, half-edges {vertex, next, twin}},
  rejecting a broken mesh (index out of range, twin(twin(h)) != h, a face loop that revisits).
- `bake vehicle`: for each BF1 vehicle geometry whose top-LOD `modl` holds `shdw` children, write the pieces into the
  vehicle's manifest entry (`ManifestVehicle::shadow`, serde default, empty when none) - every piece of the model, in
  chunk order (the PS2 draws them all).

## Acceptance
- A mungefs test that parses every disc `shdw` chunk (240 BF1 + 164 BF2) and asserts the half-edge invariants of
  re-vehicle-shadow 1 (twin = h ^ 1, twin(twin) = h, vertex(next(h)) = vertex(twin(h)), closed loops) - SKIP without
  the disc, run on this machine; and a count check against `shdw_check.py`'s totals (404, 73 + 41 names).
- After the side rebake: `all_fly_xwing`'s manifest entry holds its 9 pieces (dummyroot, xwing_b_gear2/3,
  xwing_bl_hatch3/4, xwing_br_hatch3/4, xwing_f_gear2/3 - the census, `lead_tmp/fxc/census.tsv`) and no BF2 manifest has a `shadow` field.

## Original text (FXC, 2026-10-16)
## Why
Filed by FXC's census (2026-10-16, `analysis/re-chunk-census.md` 2-3). Both PS2 games draw a **shadow volume** for each
model that carries a `shdw` chunk (BF1 27 models, nearly every vehicle; BF2 11: walkers / flyers), rebuilt along a light
vector every draw: the ground under and beside a vehicle goes dark. The port parses no `shdw` and draws no shadow at all.
On the showcase battles: BF1 NAB1 (AAT, STAP, fighter tank; GCW AT-ST, bike, X-wing), YAV2 (spider walker, AT-ST, fighter
tank), BF2 HOT1 (AT-ST snow). PLAN principle 5 wants modern shadows by default with a strict-faithful mode; this card builds
the faithful vehicle shadow (the look the strict mode needs and the baseline any modern shadow is judged against).

## Evidence
- PCSX2 knock-out (BF1 NAB1, a parked X-wing): the gate DAT_00522ed0 written 1 / 0 / 1 / 0 - with it on, 9.5 % of the
  frame (a band of ground in front of the X-wing) is darker in both "on" grabs than in both "off" grabs, on / off brightness
  0.63 (`lead_tmp/fxc/ps2/xw_g*_v*.png`, `xw_dark_overlay.png`, `knock.py`); reproduced by the workflow's measure agent.
- BF1: the gate's ELF default is 1 and nothing writes it (dead setter at 0x4ae850) - always on. Draw path FUN_00496248 ->
  FUN_004ad760 (per piece: FUN_004ae680 matrix + light vector, FUN_004ad180 rebuild) -> vtable +0xc -> FUN_004adbc8.
- BF2: FUN_0057cf60 -> FUN_00592c38 under the byte DAT_0060c66e = `FUN_00563f78() < 2` [H: one local view] after the
  Shadow effect's set-up; `Effect("Shadow") Intensity` per map (UTA1 0.2, TAT2 0.35, FEL1 0.5; default 0.5, obj+0x28).
- Draw rule: model +0xb8 set and `(flags & 0x230000) == 0x10000`; a `near` flag = depth < 30 m + radius (BF2 subtracts the
  camera's +0x130).

## What to find (phase re, bounded)
- FUN_004ad180 / FUN_00592668: how the volume is built (silhouette edges of SHDV/SHDI extruded along the vector? how far?)
  and what FUN_0049b8a8 returns (the sun direction? from SunInfo).
- FUN_004adbc8 / FUN_0059xxxx draw: how the GS darkens (destination-alpha passes? a screen quad?), the shade's colour and
  amount (BF1 SunInfo ShadowColor? BF2 Intensity at obj+0x28 - its reader), what `near` changes.
- Then build: mungefs `shdw` parser (SHDV / SHDI), bake it beside the model, a stencil shadow-volume pass in render for
  vehicle draws (wgpu has a stencil buffer: the PS2's result, not its GS trick), BF2's per-map intensity, off in split screen.

## Acceptance
The NAB1 X-wing pose rendered by the port (VREST's `--cam` recipe, `lead_tmp/vrest/build/ps2/nab1_xwing_pose.json`) beside
the PS2 pair: the port's shadow band, measured the same way as `lead_tmp/fxc/ps2` (pixels darker with shadows on than off),
covers 9.5 % +- 3 points of the frame with on / off brightness 0.63 +- 0.08; plus a mungefs test that parses every disc
`shdw` (240 BF1 + 164 BF2 chunks) without error.
