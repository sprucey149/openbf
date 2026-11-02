---
id: KSEAL
title: BF2 Kamino's ocean is lit by the sun on the PS2 (FFT normals in the packet): (8,19,19) at 314-393 m where the port's unlit unity draw reads (26-49, 38-57, 34-55) with white foam
milestone: M1
value: 5
phase: build
status: done
deps: KSEA
files: engine/crates/game/src/viewer/look.rs,engine/crates/render/src/shader.wgsl,engine/crates/render/src/lighting.rs,engine/crates/render/src/frame.rs,engine/crates/render/src/lib.rs
declares: goldens=maps/bf2_kam1.png,walkabout_stats.ron; parity=bf2_kam1
needs:
effort:
branch: kseal
resume: landed 0fb84669 2026-10-01
created: 2026-09-30
alpha: yes
---

## Why
BF2 Kamino's sea is dark and blue-shifted on the PS2 and bright with white foam in the port: the port draws the ocean
unlit at unity (`viewer::look::water_look`, PROVISIONAL [H] for BF2), the PS2 lights it. `analysis/re-kamino.md` §2.3.

## Evidence
- BF2's ocean vertex packer `FUN_00574a70` writes three NORMAL bytes per vertex and submits a normal stream
  (708056–708058, 708105) [V]; the colour word is half-white with the foam byte as alpha (708067).
- PS2, lightning frozen (`lead_tmp/kamr/ps2exp/out/s2_ctl0..2.png`): (8–9, 19, 19) at 314–393 m, (6,15,15) at 428 m,
  (5,13,13) at 521 m, where the port's bare base texture reads (21,34,29) — ratios (0.38–0.48, 0.56–0.59, 0.66–0.69)
  = the sun (65,85,105)/128 × ~0.8. With the lightning live the sea follows the lights ((2,9,7) at a flash's start).
- Port at the same pose, ocean stepped: (26–49, 38–57, 34–55) (`views/sea_compare.png`).
- BF1's ocean stays unity (PS2 (17,30,33) vs port (20,33,29), KOC) — do not touch it.
