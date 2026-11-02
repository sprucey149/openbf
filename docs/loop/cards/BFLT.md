---
id: BFLT
title: The DomeInfo Filter: every BF1 map (and BF2 TAT2) brightens its 3D frame on the PS2 by Intensity/128 of (frame - Threshold) — TAT1 +25 luma — and the port has no such pass; it is the cause of TATL (TAT1 -12.8 -> -0.1 median dluma when applied)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/sky.rs,engine/crates/bake/src/lighting.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/viewer/look.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/render/src/frame.rs,engine/crates/render/src/pipelines.rs,engine/crates/render/src/lib.rs,engine/crates/render/src/filter.rs,engine/crates/render/src/filter.wgsl
declares: rebake=maps; goldens=maps/bf1_bes1.png,maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/tat1.png,maps/tat2.png,tat1_spawn.png,tat1_overview.png,tat1_hud.png,walkabout_stats.ron; parity=bf1_kam1,bf1_rhn1
needs:
branch: bflt
resume: LANDED 7dfe311 (report reports/20260930_1245_card_BFLT.md)
created: 2026-09-30
alpha: yes
---

## Why
Every BF1 map authors `DomeInfo { Filter(1) Threshold(T) Intensity(I) }` and the PS2 runs it over the 3D frame before
the HUD: a soft lift of everything brighter than T. On the threshold-50 maps (TAT1, KAM1, KAS2, NAB1, NAB2) it brightens the
whole frame by ~20 %; the port has no such pass, which is why showcase TAT1 reads 12–15 luma darker than the PS2 (card TATL).
BF2 TAT2 (the port's `tat2`) authors it too (threshold 150).

## Evidence
- `analysis/re-glow.md` §3 ([O], verifier SUPPORTED/PARTLY): keys (B1 FUN_002e29e0, BF2 FUN_003bcd50), the pass (B1
  FUN_0049fe60, BF2 FUN_00585890): downsample 2:1, subtract T, a sub-pixel blur, add I/128 of it; per-map values §3.2.
- On the PS2 (SW, patched … KAS2 72.1 / 65.7; BF2 TAT2 154.9 /
  150.5; the model out = min(255, v + I/128·up2(max(0, box2(v) − T))) predicts the on-frame from the off-frame within
  mean |d| 1.6–3.9 (`lead_tmp/re/glow/ps2exp/simfilter.py`, sheets `ps2exp/sheets/bf1_tat1__nofilter.png`).
- Applied offline to the port's 09-29 walk frames: TAT1 median dluma −12.8 → −0.1 (24 pairs); KAM1 −6.8 → +1.3; NAB2
  −5.9 → −0.6; KAS2 −7.7 → −3.7; threshold-150 maps move ≤ 3; NAB1 −7.3 → +10.5 and BES2 −1.0 → +8.8 overshoot (their
  bright areas are already too bright in the port — re-measure after landing and card the cause if it stays).

## Notes
- Softness (every map, a 47 % blur-lerp) is SD softening — not part of this card (COMPARISON_RULES).
- The PS2 truncates the blur offsets to 1/16-px integers (sub-pixel): the port's low-pass only needs to match a 2×2 box
  at 256×224 scaled to the output.
