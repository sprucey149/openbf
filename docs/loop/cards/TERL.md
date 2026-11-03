---
id: TERL
title: The terrain's PS2 lighting: BF2 draws its lightmap with no ambient and no sun (both Hoths clip in the port); read the terrain VU path (form, scale) and BF1's terrain, then build
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The terrain is most of every outdoor frame, and the port lights it as LUMI x (ambient + sun.N.L). On the PS2, BF2's
terrain base pass carries NO VU lighting. So the port clips both Hoths to white (F27; HOLD 11(a)) and over-brightens
FEL1 / END1 / KAS2's ground. This is queue LITE's §5 (`analysis/re-model-lighting.md`).

## RE result (2026-09-25, lane 2 — `analysis/re-terrain-lighting.md`, [O], verifier 22/1/0; main 73184fa)
- **Both games draw the terrain UNLIT.** Each layer takes VU1's unlit path with the vertex colour from `COLR` (LUMI is
  only a point lookup):
  - BF1: c5 << 3, so the pixel is tex × c5/16 (up to 1.94, clamped at 255);
  - BF2: c5 << 2 × HDR, so the pixel is tex × c5/32 (HDR is neutral).
- **Measured:**
  - BF1 HOT1: the snow stays at 197.9 with the sun, the ambient or both zeroed.
  - BF2 FEL1: the ground ignores its non-Static sun and doubles at HDR 1.0.
  - The port instrument at BF2 NAB2's single-layer floor: PS2 (48,38,31) against the rule's (48,37,29).
  - Hoth: 243 → 223 against the PS2's 210. The rest is the unported layer blend [H] (TALPH / KTEX).
- **Out of this build:**
  - the DTLX detail pass (card TDTL);
  - BF2's runtime omni and projector passes (card TDYN);
  - the layers (TALPH / KTEX);
  - BF2's near-scene fog (FOGN).
- Sheets and tables are under `engine/artifacts/lead_tmp/re/terl/`: `sheets/`, `means.txt`, `masked.txt`, `ps2exp/`.

## Evidence
- **[V code, verifier-corrected]** FUN_00571bd0 builds the terrain's light set prelit (FUN_00599040(box, 4, 1, 1, 0),
  706665). FUN_0056f208 never uploads it and never sends an ambient. It reads only the non-Static point lights and the
  +0x94 light, for extra additive passes.
- **[V measured]** BF2 HOT1: the near snow does not move when the sun or the ambient is zeroed in RAM (217.8 / 207.8 in
  every variant), while every model darkens (`lead_tmp/re/lite/ps2exp/frames/bf2_hot1__*.png`).
- **[D]** The port with the terrain lit by its lightmap alone: 242 -> 229 (PS2 214), clipping 33 % -> 11 % (PS2 0.2 %)
  (`lead_tmp/re/lite/sheets/lite_bf2_hot1_v2.png`).

## Notes
- FEL1's ground boxes are foliage-covered (F22). The foliage responds to the non-Static sun and to the ambient, and it is
  FOLG's path.
