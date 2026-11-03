---
id: GLOW
title: Self-lit surfaces glow on the PS2 and are dark in the port: BF2 Utapau's window panels, Coruscant's skyline windows, Mustafar's lava
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance
(Rewritten by the planner 2026-09-30; the original entry and the notes below keep the detail.)
- **Why:** BF2 lamps, window panels, light strips and lava glow (with a halo) on the PS2 and are dark in the port, on
  every BF2 map with lit interiors. The showcase maps UTA1 and MYG1 are among them.
- **Evidence:** the LITE RE note below gives the mechanism, [O] in `analysis/re-model-lighting.md` §1/§2. BF2 draws
  the scene at 0.5 and doubles it back. An MTRL 0x14 segment skips the 0.5 (a 2× glow), and a segment without 0x01 is
  unlit. The halo is the HDR bloom (GlowThreshold, NumBloomPasses). The pairs are in the notes (F28, F48, F55, F58,
  F61, F64).
- **Acceptance (measurable):**
  - RE: which bit or channel marks a surface self-lit or unlit on each game, twin-checked; and what the bloom does
    (threshold, passes, radius), because the modern port may bloom at full resolution (COMPARISON_RULES).
  - Pixel test on the named boxes, re-rendered: UTA1 cp3_con_cp1 b090 corner (0,0,120,80) within ~20 levels of the
    PS2's (130,132,138); POL1 cp0_cp2con b000 (260,0,480,50) toward (252,252,251); MUS1 cp0_cp5 b180 lava toward
    (153,89,33).
  - The goldens that move are declared, and PARITY on the BF2 maps does not regress.

## Original QUEUE.md entry (verbatim, 2026-09-25)
GLOW. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 3 (FINDINGS F28); **[RE] FIRST** (job D, at max); FREE; overlaps LITE (lighting) and R1U (the glow-mask alpha) — read both first] **Self-lit surfaces glow on the PS2 and are dark in the port: BF2 Utapau's window panels, Coruscant's skyline windows, Mustafar's lava** [RE] — evidence `engine/artifacts/pcsx2/ref/bf2_uta1/pairs/cp3_con_cp1_postA_v3.png` (panels (50,10,160,150) PS2 (61,51,35) spread 34.3 vs port (48,43,38) 10.3), `…/bf2_cor1/pairs/cp1_cp2_postA_v1.png` (towers (130,20,240,130): brightest 213 and 1.8 % ≥ 120 on the PS2 vs 73 and 0 %), `…/bf2_mus1/pairs/cp1_cp1_postA_v{1,2,3}.png` (bright-lava pixels R ≥ 180, G ≥ 90, B ≤ 90: 1190/1094/1069 vs 0/3/0); LOW: BF1 BES2 `cp0_cp4_postA_v3_up` ceiling. `analysis/re-material-flags.md` §"COUNTED 2026-09-24" already reads some materials' alpha as "a glow mask, not a cutout". RE (§5): which material bit or texture channel marks a surface self-lit / additive / unlit on each game (the model material word, the texture's alpha as an emissive mask, a separate glow pass?) — with the twin check (BF1 vs BF2) and the flag default check. Build: the port honours it in the world shader. GATE: the three pairs read lit where the PS2 is (the boxes above within ~20 levels); goldens that move are declared after the RE; sim hashes unmoved. Files: `render/src/` (world shader + material flags), maybe `bake/src/` (the flag) — shares `render/src/pipelines.rs`/the world shader with BFC and LITE: build when neither is IN PROGRESS.

## Notes
- 2026-09-25 (LITE RE, `analysis/re-model-lighting.md` §1/§2): BF2 draws the whole scene at 0.5 (`Effect("HDR")`, DAT_007c5754) and its composite doubles the frame back (ALPHA 0xff00000029). A segment whose MTRL has 0x14 skips the 0.5 (FUN_0058ed70 723501), so it comes out DOUBLED: a glow. Segments without MTRL 0x01 are drawn unlit (VU1 0x2a3), e.g. COR1's light strips `f=0x1010210`. The port lights both. The HDR bloom (GlowThreshold CLUT, NumBloomPasses) adds the halo.
- 2026-09-29 (PXWB, lane 2; FINDINGS F48): BF2 lamp halos and light strips — UTA1's ceiling lamps carry white glow halos on the PS2 (`pcsx2/ref/bf2_uta1/walk/pairs/cp3_con_cp1__sheet.png` b090 corner (0,0,120,80) (130,132,138) vs (64,66,65)); GEO1's bunker wall strips and ceiling lamps glow white-yellow on the PS2, thin orange lines in the port (`bf2_geo1/walk/pairs/cp4_cp4__sheet.png`, `cp1_cp7__sheet.png`); TAN1's window console wrapped in a blue glow (`bf2_tan1/walk/pairs/cp2_cp7con__sheet.png` b090). The lamps' LIGHT (pools on the floor) is BPNT's.
- 2026-09-29 (PXWB part 2, FINDINGS F55): FEL1's glowing pod-plant stacks are bright yellow-white with a halo on the PS2, pale translucent blue-white in the port (`pcsx2/ref/bf2_fel1/walk/pairs/cp4_cp5_1__sheet.png` b090 (40,60,160,200) (154,160,97) vs (139,145,93)); the post lamp's halo (`cp2_cp2_1__sheet.png` b090 (40,40,120,120) (227,216,163) vs (151,153,82)); DEA1's window rims glow with a halo (`bf2_dea1/walk/pairs/cp0_cp1__sheet.png` b180).
- 2026-09-29 (PXWB part 3, FINDINGS F58): MUS1's lava glows on the PS2 (`pcsx2/ref/bf2_mus1/walk/pairs/cp0_cp5__sheet.png` b180 lava sea (0,320,640,440) PS2 (153,89,33) vs port (72,32,16); `cp1_cp1__sheet.png` b090 the lavafall yellow-white vs a dark-blotched texture); MUS1's holo table / screens / blue floor lamps (`cp2_cp3__sheet.png`, `cp3_cp2__sheet.png`); MYG1's corridor strips blaze white with a bloom (`bf2_myg1/walk/pairs/cp4_cp5_con__sheet.png`); KAM1's lamp halos (`bf2_kam1/walk/pairs/cp2_cp3__sheet.png` b180/b270).
- 2026-09-29 (PXWB, FINDINGS F61): BF2 NAB2's lit windows glow with a halo on the PS2 (`pcsx2/ref/bf2_nab2/walk/pairs/cp0_cp1__sheet.png` b090, `cp4_cp6__sheet.png` b180, `cp1_cp3__sheet.png` b270).
- 2026-09-29 (PXWB part 4, FINDINGS F64): BF2 POL1's ceiling skylight blazes white with a halo spilling over the ceiling on the PS2, a flat pale panel in the port (`pcsx2/ref/bf2_pol1/walk/pairs/cp0_cp2con__sheet.png` b000 (260,0,480,50) PS2 (252,252,251) vs port (184,189,181); the top band (0,0,640,120) (164,163,163) vs (111,114,109); b180 alike) and its corridor strips carry halos (`cp3_cp4con__sheet.png` b090); BF2 COR1's light columns glow with a big white halo (`bf2_cor1/walk/pairs/cp4_cp4__sheet.png` b090 (180,100,330,260) (103,104,90) vs (90,95,79), b000/b180), its terrace lamp tops glow (`cp1_cp2__sheet.png` b000: max luminance 201 / 222 with 8 / 139 px >= 200 on the PS2 vs 110 / 124 and none in the port) and a control-room screen flares (`cp5_cp6__sheet.png` b000 (0,30,60,110) (48,75,101) vs (35,51,71)).

## RE result (2026-09-30, lane 3; `analysis/re-glow.md`, [O] UNAUDITED; report `reports/20260930_1100_card_GLOW.md`)
- **The bit:** BF2 MTRL **0x10** without 0x04, on a map whose `Effect("HDR")` is enabled (13 of the port's 17 BF2 maps):
  ALPHA 8 (Cs + Cs·As/128) with colour 0xff404040 → ×(1 + As/128), ≈ ×2.98 for texture alpha 255; the texture's alpha is
  the glow mask. 245 such segments, all unlit. **0x14** skips the HDR ×0.5 (×2 on screen). BF1: no per-segment glow.
- **The bloom:** luma (Rec.601) of the half-scale frame ≥ GlowThreshold·255 → 4× down → NumBloomPasses×2 five-tap passes
  (weight int(int(MaxTotalWeight·127)·0.2)/128 each) → added; σ ≈ 7.8 % of the screen height at 5 passes. Per-map values
  in §2.1 (UTA1 3 passes / 1.0, HOT1 threshold 0.7, TAN1 0.9).
- **Measured on the PS2** (SW, VRAM): MYG1 strips match the ×(1 + As/128) model per pixel; the bloom cascade reproduces
  the PS2's bloom within ~10 %; glow off → no bloom on MYG1 (frame luma 99.7 → 82.8). HW Basic (the library) agrees
  within ~2 levels, so the named-box acceptance on the library frames stands.
- **Build notes:** keep glow textures' alpha in the bake (`alpha.rs` strips it for all-OPAQUE textures); do the glow
  multiply in display (gamma) units; the glow drops the instance colour; a 0x10|0x02 segment tests at disc alpha ≥ 98;
  the PS2 turns HDR off in split screen — keep it on in the port (reversible; FEEL/For the maintainer). The bloom may be split into
  its own card if the build runs long.
- **Also found:** both games' DomeInfo **Filter** (every BF1 map, BF2 TAT2) — card **BFLT**; it explains TATL.
