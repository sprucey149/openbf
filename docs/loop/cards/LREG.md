---
id: LREG
title: Light regions: both games dim the sun and blend the ambient of non-static models inside region volumes (BF2 FUN_0059b2f8, BF1 FUN_00493cf0's DAT_0069c548) — the port has none (BF2 RAM: MUS1 13, UTA1 9, POL1 7, TAN1 6 entries)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Both games dim NON-static models inside light regions:
- They scale the sun, and on BF2 Light2 too, by a region factor, and blend the ambient toward the region's own.
- BF2: `FUN_00597e98` → `FUN_0059b2f8` (B2 731344–), the list headed by `DAT_007cd51c`, weights by `FUN_005adc48`,
  entry +0x60 / +0x64 factors and +0xb8.. / +0xc8.. ambient adds.
- BF1: `FUN_00493cf0` 556043–556085, the list at `DAT_0069c548`, region +0x70 factor and +0x74 tint.
- Static draws skip them: terrain foliage, and BF2 ODF `Lighting="static"` classes (`analysis/re-foliage.md` §16).

The port has no regions, so a model under an indoor or shadowed region is lit as if it stood in the open.

## Evidence
Live list entries, walked as the ELF walks them (card FOLGL's scripts, `lead_tmp/re/folgl/`):
- BF2: MUS1 13, UTA1 9, POL1 7, TAN1 6, DEA1 5, GEO1 3, COR1 2, NAB2 1–3 (by pose), DAG1 1, KAM1 1; END1, FEL1,
  HOT1, KAS2, MYG1 and YAV1 0 at the spawn.
- BF1: KAM1 1 (factor 0.0, tint e6e3da), RHN1 1 (0.0, 000000), RHN2 1 (0.5, 0000007f). The other BF1 states have none.

## RE
- Where the regions come from on the disc. The BF2 list writers are `FUN_0059b088` / `b138` / `b1e8` / `b980`; BF1's
  list is `FUN_00493ad0`'s.
- `FUN_005adc48`'s weight: the volume's shape and falloff.
- What each entry field means.
- Which BF2 draws are non-static: the ODF `Lighting` default per class (`FUN_00214600` 127897–127911,
  `FUN_00239bb0` 153340–153341).
- One PS2 patched state with the list emptied, on MUS1 or UTA1, for the size of the effect.

## Acceptance
`analysis/re-light-regions.md` [O] with the verifier, and a build card if the effect shows in a PCSX2 pair.
## Note (PXWA, 2026-09-29, lane 2): BF1 GEO1's cave is 2-3x brighter and redder on the PS2
The PCSX2 walkabout at GEO1 cp6 (the cave by the arena; 1-unit battle, the player held at the post): the rock walls are
PS2 (54,16,8) vs port (18,9,3) over (200,100,640,300) at b270, (46,22,17) vs (19,10,3) at b000 —
`engine/artifacts/pcsx2/ref/bf1_geo1/walk/pairs/cp0_cp6__sheet.png`. GEO1's baked light is a grey sun 0.188 and an
ambient (0.188,0.130,0.072); the PS2's extra red reads like a region or local light inside the cave (or LSUM's sum rule
on a dim scene). The state `ref/states/bf1_geo1_walk_cp0_cp6_b000.p2s` is kept for a patched-state test.

## Notes
- 2026-09-29 (PXWB part 3, FINDINGS F58) [H]: BF2 KAM1's interiors take a blue (`pcsx2/ref/bf2_kam1/walk/pairs/cp0_cp1__sheet.png` b000/b090) and a pink-red (`cp3_cp2__sheet.png` b000/b180/b270) cast on the near consoles on the PS2, neutral grey in the port — a region or a local light (BPNT).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

## Note (2026-09-30, BPNT RE, `analysis/re-point-lights.md` §5 [V] / §12 [H])
- Besides the ambient regions, BF2 authors **region-bound directional lights**: a `lght` type-1 Light with `Region`
  (0x031afe7d → +0x60, resolved to +0x64). Examples: TAN1's blue grate / white tunnel lights (6), DEA1's 30+ Overbright
  directionals, FEL1's 6.
- FUN_00597e98 gives a model inside such a region the light, weighted by FUN_005adc48. The set holds at most 2
  directionals in all, Light1 / Light2 first.
- The weight function and which models sit in the regions are this card's to read.
