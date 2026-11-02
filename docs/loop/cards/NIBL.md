---
id: NIBL
title: The port's 4-bit textures are nibble-swapped: mungefs::texture::unpack_4bit takes the high nibble first, the PS2 (GS PSMT4) the low — every 4-bit texture in the bake has each pair of horizontal texels swapped (HUD icons zig-zagged; re-bolt §7): swap, rebake, view every golden
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/texture.rs
declares: goldens=any; rebake=tat1,tat2,maps,common,side
needs:
branch: nibl
resume:
created: 2026-09-30
alpha: yes
---
## Why
Found by FXB's RE (`analysis/re-bolt.md` §7). `unpack_4bit` pushes `b >> 4` then `b & 0x0F`; on the PS2 a 4-bit
(PSMT4) texture keeps the LEFT texel in the LOW nibble. Every 4-bit texture the port bakes therefore has each
horizontal texel pair swapped: HUD icons are zig-zagged and every 4-bit world/unit texture carries a 1-texel
horizontal jitter (the classic tier; the PC-texture tier 2 is unaffected).

## Evidence
`engine/artifacts/lead_tmp/fxb/nibble.py`: swapping every horizontal texel pair of the BAKED mip 0 makes 191 of 205
BF1 COMMON 4-bit textures (94 of 104 BF2 INGAME) markedly smoother (roughness ratio median 0.58 / 0.56) and none
rougher; the controls — 8-bit (35 + 25) and 32-bit (5) textures — get rougher (median 1.53 / 1.43), 0 smoother.
Visual: `engine/artifacts/lead_tmp/fxb/nibble_sheet.png` (hud_target_flag_onscreen, hud_player_captured_flag, the
laser textures and lightglow: bake vs pairs swapped).
