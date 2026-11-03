---
id: TLUMT
title: The port's terrain lightmap (COLR) is transposed: the PS2 reads LUMI x-major (idx = iz + ix·n), the port puts colr[z·n+x] on vertex (x, z)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

- **Why:** found by FOLG's build (lane 1, 2026-09-26). The PS2's LUMI lookup (B1 FUN_00473d28 537200, B2 FUN_0055f1d0 695200) indexes the raw buffer `iz + ix·gridSize` — x-major. The port's COLR decode matches raw LUMI index for index (mungefs::terrain header, 0.31/255) and terrain_glb gives vertex (x, z) = index z·n + x the colour colr[z·n + x], i.e. the value the PS2 reads at (ix = z, iz = x): the lightmap is mirrored about the diagonal.
- **Evidence:** (1) `lead_tmp/re/folg/lumicheck.py`: the foliage tints the PS2 computed from LUMI reproduce exactly x-major on 211/211 BF1 instances, z-major 71/211 (max error 74 levels). (2) `lead_tmp/re/folg/lumihill.py`: the best Pearson r between LUMI and a hillshade of the baked heights (72 azimuths × 4 elevations) is TAT1 0.307 x-major vs 0.059 as the port reads it, BF2 NAB2 0.383 vs 0.030, BF2 FEL1 0.090 vs 0.024, END1 0.082 vs 0.060 (RHN2 flat, 0.027 vs 0.041). bake::foliage already samples LUMI x-major.
- **Build:** transpose the lightmap where it is decoded (unpack_colr and lumi, or where terrain_glb reads them) so COLR/LUMI index [x][z]; re-check tat1_lvl's COLR↔LUMI test (it compares the two in storage order, so it stays); confirm on a PCSX2 pair of a terrain with strong baked shadows (TAT1 or BF2 NAB2) before landing; audit TERL's goldens under the fix.
- **Acceptance:** the hillshade r moves to the x-major values above; a PCSX2 pair shows the terrain's dark patches where the PS2 has them; only terrain pixels move.
