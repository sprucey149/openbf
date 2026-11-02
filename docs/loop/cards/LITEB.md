---
id: LITEB
title: Light static models the PS2's way: the disc's per-vertex COL0 (multiply; 0x200 adds), prelit models (no ambient, no Static lights), BF2 Light2, BF2 sun colour sRGB->linear
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/glb.rs,engine/crates/bake/src/lighting.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/assets.rs,engine/crates/game/src/viewer/look.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/render/src/shader.wgsl,engine/crates/render/src/lighting.rs,engine/crates/render/src/lib.rs
declares: goldens=maps/bf1_bes1.png,maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_cor1.png,maps/bf2_dag1.png,maps/bf2_dea1.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_geo1.png,maps/bf2_hot1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_mus1.png,maps/bf2_myg1.png,maps/bf2_nab2.png,maps/bf2_pol1.png,maps/bf2_tan1.png,maps/bf2_uta1.png,maps/bf2_yav1.png,maps/tat1.png,maps/tat2.png,tat1_overview.png,tat1_spawn.png,tat1_hud.png; rebake=tat1,tat2,maps
branch: liteb
resume: landed d8e30ba; report reports/20260925_1858_build_LITEB.md
created: 2026-09-25
---

## Why
The port renders BF2's static scenery 1.3-2.6x brighter than the PS2 (F20, F27) because it drops the disc's per-vertex
lighting and the PS2's light rules. This is queue LITE's answer, `analysis/re-model-lighting.md` (§0, §2, §3, §9) [O].

## Evidence
All from `analysis/re-model-lighting.md`:
- **PS2, VU1** (BF2 0x1e6 / 0x1bb / 0x521 / 0x537, handler 0x56b; BF1 twins 0x51b / 0x531).
  - lit colour = mat x (lerp(Bottom, Top, (1 + n.y)/2) + sum C_i x max(0, N.L_i)), in GS units (Top/255, Light.Color 1.0 = 1.0);
  - then COL0/128 x that (a segment with COL0);
  - or + mat x COL0/128 (a 0x200 "vc-lit" segment; BF1 adds COL0 without mat).
- **PS2, EE.** A model with ANY 0x200 segment is prelit (FUN_0057de00, model +0x48 & 1). Its light set uploads a black
  ambient and skips every light whose Static bit (word 0 bit 8) is set (FUN_00597e98 / FUN_0057fd90). Normal models get
  Top/Bottom + Light1 + Light2 (Light2 only while `GlobalLights.Light2` names a type-1 light; COR1, END1, MUS1, NAB2, UTA1).
- **Measured, the port with these rules at the PS2's poses** (scratch instrument, `engine/artifacts/lead_tmp/re/lite/`,
  §8), luma:
  - BF2 MYG1 floor 52 -> 31 (PS2 28);
  - BF2 TAN1 frame 137 -> 109 (PS2 103), its 12-13 % white clipping -> 0 %;
  - BF2 COR1 floor 92 -> 43 with Light2 (PS2 46).
  - Sheets: `lead_tmp/re/lite/sheets/lite_bf2_{cor1_spawn,myg1_spawn,tan1_v3}.png`.

## Notes
- Fold in the per-segment "unlit" case here only if it is small; GLOW owns the 0x10 glow segments (they skip BF2's HDR 0.5
  and come out doubled, §1).
- BF2's HDR scale itself is neutral (the composite doubles it back) — do not port a 0.5.
