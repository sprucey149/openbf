---
id: LODP
title: Card LOD's far switch costs 4 ms a frame on BF2 KAM1 at 2880x1920 on the Intel iGPU (94 -> 68 fps, models 7.9 -> 11.9 ms; OPENBF_LOD=0 restores it) - find what the switch draws extra and keep the PS2's look at the old cost
milestone: M0
value: 5
phase: build
status: done
deps:
files: engine/crates/render/src/draw.rs, engine/crates/render/src/frame.rs, engine/crates/render/src/frustum.rs
declares: goldens=maps/bf1_yav2.png,maps/bf2_hot1.png,maps/tat1.png
needs:
effort:
branch: lodp
resume:
created: 2026-10-20
alpha: yes
---
## Why
Card ISPD's profiler (OPENBF_GPU_TIMING=1, engine/artifacts/ispd/README.md) on main 4914148 = LOD dde30a1 + the profiler, the windowed game in kwin --virtual on OPENBF_ADAPTER=igpu, player idle at his spawn, 90 s (engine/artifacts/ispd/win1.sh):
- bf2_kam1 2880x1920: pre-LOD (36667e3) 98 fps, models 7.57 ms, 309 calls; LOD on 68 fps, models 11.85 ms, 350 calls; OPENBF_LOD=0 94 fps, models 7.89 ms, 309 calls.
- bf2_geo1 2880x1920: models 1.45 / 1.81 / 1.50 ms, 324 / 345 / 324 calls.
Rows: engine/artifacts/ispd/win/bf2_{kam1,geo1}_2880x1920_{idle,lod,lodoff}/frames.csv. The switch exists to draw LESS far away; here it draws more.

## Hypotheses (none tested)
1. The 41 extra draw calls are LOWD meshes now drawn where the model alone was drawn before, both rasterised across the dithered band (each pixel keeps one, both pay the raster and the fragment cost up to the discard).
2. A LOWD that covers a large part of the screen (a far shell) drawn with no depth benefit.
(The far_cull discard predates LOD, so early-depth loss alone is not new.)
