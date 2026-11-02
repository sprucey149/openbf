---
id: TALPH
title: Terrain texture LAYERS: the port draws only LTEX[0]; the PS2 blends every painted layer by ALPH's 4-bit over-alphas — BF2 KAS2's sand beaches, BF1 KAS1's grass, KAS2's pale slopes (KTEX merged)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/terrain.rs,engine/crates/bake/src/terrain_glb.rs,engine/crates/bake/src/map.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/assets.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/render/src/draw.rs,engine/crates/render/src/shader.wgsl,engine/CRATES.md
declares: goldens=maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_hot1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_dag1.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_geo1.png,maps/bf2_hot1.png,maps/bf2_kas2.png,maps/bf2_nab2.png,maps/bf2_pol1.png,maps/bf2_yav1.png,maps/tat1.png,maps/tat2.png,tat1_overview.png,tat1_spawn.png,tat1_hud.png; rebake=tat1,tat2,maps
needs:
branch: talph
resume: LANDED 23238e1 (2026-09-28 lane 1, report reports/20260928_1320_land_TALPH.md): 28 goldens viewed and committed; PS2-pose checks improved on RHN1/2, HOT1 and BF2 KAS2 shore.
created: 2026-09-25
---

## Planner re-scope (2026-09-26) — value 2 → 5, KTEX merged in
**Why now:** the maintainer (INBOX 2026-09-25) asked whether BF2 Kashyyyk's sand beaches are the terrain layers. They are [D]:
the bake keeps ONE layer (`mungefs/src/terrain.rs` "LTEX[0] as its single base layer"; `terrain_kas2.glb` has one
material, `kas2_main_1`), and the planner's census of `PCHS -> PTCH -> INFO` masks (script below;
`reports/20260926_1100_planner.md`) shows what the port throws away:
- **BF2 KAS2:** LTEX = [kas2_main_1 (mean RGB 79,80,63 — grass/dirt), kas2_main_3 (103,92,70 — sandy brown),
  **kas2_main_2 (135,131,120 — pale sand)**]; layer masks: layer 0 in 174 of 256 patches, layer 1 in 160, **layer 2
  (the pale sand) in 90**. Script: `engine/artifacts/lead_tmp/planner/layer_census_20260926.py`.
- **BF1 KAS1:** LTEX[0] = kas1_main_2 (140,133,114 — the grey sand the port draws everywhere, F17), then kas1_main_3,
  **kas1_main_1 (66,88,58 — green)** in 51 of 73 live patches, kas1_main_4, kas1_main_7 (both green).
- **BF1 KAS2:** kas2_main_1 (56,60,36 — the port's dark grass) in 555 of 621 patches; kas2_main_2 (124,110,88 — pale
  sand, F6's slope behind Dock Two) in 60; kas2_cobbles_02 in 18; kas2_main_3 in 102.
So F6, F17, most of F14 / F23's ground colour, the maintainer's beaches and TERL's Hoth residual are ONE missing system.
Walk-through rule (the maintainer): the layers are decoration on the same heightfield — no collision change.

## Why (original)
The terrain's authored alpha (texture-layer blend weights or transparency) is unread; wherever the disc uses it the port's
ground differs. May bear on KTEX (KAS2's pale sand slope).

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 10(d); `analysis/a2-bake-formats.md` (terrain).
- **NOTE 2026-09-25 (TERL RE, `analysis/re-terrain-lighting.md` §6 [O]).** `ALPH` holds the LAYER WEIGHTS, and the ELF's
  path is now known:
  - The PCHS parser FUN_0056bbf0 stores ALPH's per-patch pointer at patch entry +0 (703310 / 703323). It is an RLE
    stream walked for (grid+1)·(grid/2+1)·layers bytes.
  - FUN_00572c98 decompresses it (FUN_0056e898). FUN_0056c9d8 reads each vertex's weight as a 4-bit nibble, 2 per byte.
  - DAT_0060c1a0 (BF1 DAT_00522a20, same 16 bytes) maps the nibble to the GS vertex alpha:
    {0, 9, 18, 27, 36, 45, 54, 64, 73, 82, 91, 100, 109, 118, 128, 0}. A triangle is skipped when any weight is 0xf or
    all three are 0.
  - A single-layer patch uses 0xee (opaque). Layers are blended with ALPHA 0x44.
  - A patch with a `PTCH` replacement (entry +0x10) takes its own ALPH / COLR (FUN_0056dcd8; *(decompiler detail omitted)* / *(decompiler detail omitted)*).
- **Why it matters:** the Hoth residual after TERL is this blend. At both Hoth spawns the patches are masks 0x5 / 0x7;
  hoth_main_2 has mean 177 and hoth_main_4 214, against hoth_main_1's 225.
