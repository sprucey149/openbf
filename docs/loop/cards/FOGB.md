---
id: FOGB
title: BF1's near models took no fog in the KAS2 patched-state control - identify the missing gate; planar fog depth is already implemented by REDB
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
`analysis/re-fog.md` §0.3 / §2.2 read BF1's segment draw `FUN_004d0108` as fogging every opaque model segment in BOTH
scenes (no layer test), and the port fogs every draw from its near distance. The PS2 itself disagrees on BF1 KAS2. With the
fog ramp patched to 0..200 m in EE RAM:
- the bamboo stalks (10–40 m, f = 0.05–0.2) changed by exactly (0,0,0);
- the soldier's box changed by −0.13 (noise);
- only the far-water band fogged.

On BF1 maps with short ramps (END1/YAV1 90..350, BES1 80..430, BES2 80.., HOT1 100..1000), the port may be fogging near
models the PS2 leaves clear. The original report also identified radial rather than planar fog depth in the port.
REDB subsequently implemented planar depth; the source reconciliation below records that part's no-change decision.

## Evidence
- `analysis/re-fog.md` §9.3 ([O]).
- Frames: `engine/artifacts/lead_tmp/re/kas2-fog/ps2exp/frames/bf1_kas2_v1sea__{ctl,r200,r400,r800}.png`.
- States: `ps2exp/mkstates.py` (keeps the recipe; the .p2s copies were deleted, regenerate them from
  `pcsx2/ref/states/bf1_kas2_cp0_cp2_beachhead_v1_sea.p2s`).
- RAM matrix: 0x6ea8b0 in that state.

## Acceptance
- Name the gate that skips near models in BF1: the opacity test (*(decompiler detail omitted)*, `seg+0x2c`, 0x400000), a near-scene
  rule, the fog matrix per scene, or the VU program. Use RE plus one more patched-state pair on a map whose near props
  are plainly opaque (e.g. BF1 HOT1 or END1, whose short ramps make the test easy).
- Then a build card (or a recorded no-change) for the port's BF1 near-model fog, and for radial vs planar distance.
- BF2's twin question is card FOGN.

## Notes
Created 2026-09-28 by KASF (lane 2). FOGN (value 3) is the BF2 half: far-scene-only fog. Consider merging if the answer is
"far scene only" in both games.

## Note (REDB, 2026-09-29, lane 2)
REDB settles HOW MUCH fog (planar view depth, the disc's `fogspread` table, a lerp on the stored bytes —
`analysis/re-fog-amount.md` §8). This card keeps WHICH draws are fogged in BF1's near scene. The segment draw's fog pass binds
the same texture (B1 594618).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

## Current source reconciliation (2026-10-25)
The radial-to-planar request needs no further implementation: REDB records landing 002c99e3, and current
`render/src/shader.wgsl` assigns `fog_depth = clip_pos.w` in `vs_main` (252), the specular helper (900), and
`vs_skinned` (992). `shade` passes that depth to `apply_fog` (434), whose `fog_amount` uses it (581–583).
`render/src/grass.wgsl` does the same at 45/77. `render/src/lighting.rs` documents planar metres at 14–18.
The radial `view_dist` remains in culling, fades and the water crossover; its presence does not mean fog amount
uses radial distance. This is source reconciliation, not a fresh PS2 measurement or current pixel-parity pass.

The near-model question stays open: `re-fog` §0.3/§2.2's reading and §9.3's patched-state observation remain
contested, and `viewer/look.rs`'s BF1 near-scene-fog comment (600–602) does not settle them. Do not disable BF1
near fog globally from the bamboo/soldier observation alone. Keep opacity, scene/matrix and VU explanations
open until the original control and a plainly opaque near-prop pair distinguish them. FOGN keeps BF2's scope.
