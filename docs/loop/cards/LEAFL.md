---
id: LEAFL
title: BF2 leaves still sum their light in linear space: grass.wgsl's leaf path adds ambient + sun (+ light2) linear and multiplies unclamped, while LSUM moved every Lit model to one sum in GS units clamped at 255/128 - a leaf under an ambient and a sun of one size reads ~1.4x darker than a Lit model beside it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Cleanse 17 (CLN12) reviewer finding, verified by reading: `render/src/grass.wgsl` vs_grass (the `in.params.x > 0.5` leaf branch, ~line 48-58) sets `out.light = ambient + sun (+ light2)` from the LINEAR globals and fs_grass does `rgb = rgb * in.light` (~line 75), no clamp. Card LSUM (`analysis/re-kamino-decks.md` §2 [O]) made the world shader sum in GS (display) units: `gs_lit(texel, ambient_g + sun_g + light2_g)` with each colour `linear_to_srgb`-ed before its weight and the sum clamped at GS_MAX (shader.wgsl ~455-486, 533). The leaf comment says it lights "per vertex as the VU", so the leaf path should take the same sum; before LSUM both shared the error, now they disagree. PROVISIONAL path (leaf.rs).

## Evidence
shader.wgsl `gs_lit` / `shade_unfogged`; grass.wgsl vs_grass / fs_grass; `render::lighting::light_sum` (the CPU twin).

## Acceptance
- A GPU referee beside `light_path::the_lit_sum_is_made_in_display_units`: a leaf and a Lit quad with the same normal under ambient = sun (KAM1-like, 100 / 105) render the same pixel within 2 levels (fails on main: ~1.4x apart).
- Goldens declared for the BF2 leaf maps that move; view them.
- Every other gate PASS.
