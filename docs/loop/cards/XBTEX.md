---
id: XBTEX
title: The Xbox discs as texture donors: pack.py / rebake.sh find the Xbox BF1 and BF2 images in iso/ by their XDVDFS magic, a bake reader for the Xbox tex_ layout (DXT1 / DXT2-3 as is, the uncompressed formats unswizzled), and bake::tier takes the LARGEST copy per texture name across PS2 / Xbox / PC with the PS2 alpha mask
status: archived
issue: 99
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #99](https://github.com/sprucey149/openbf/issues/99).

## Why
The maintainer (INBOX 2026-10-21 19:30): "pack.py should take whatever versions the player owns and use the best textures".
First look and tools: `engine/artifacts/lead_tmp/xbtex/README.md` (keep the dir). Measured there: the best copy is per
texture, not per disc (BF2 GEO1: Xbox alone has the largest of 11 names, PC alone 6; BF2 KAS2: PC wins), and BF1 gains
most (no PC BF1; Xbox BF1 NAB2 has the largest copy of 35 textures).
The maintainer's rule (19:45): another version changes ONLY the look - never a number or shape the sim reads (collision, ODF
values, mission values, terrain heights, animation timing that moves a unit, hit spheres). The PS2 is the reference.

## Do
- An XDVDFS reader in Rust inside `bake` (like `extract-iso`; `tools/xiso.py` is the reference: partition at 0 trimmed or
  0x18300000 full), found by the magic, not the filename.
- `bake xbox-tex`: `tex_` = NAME / INFO (20 bytes: w, h, depth, mips, type, D3DFORMAT, body bytes) / one BODY. Formats
  0x0C DXT1, 0x0E DXT2/3 (not swizzled); 0x00 L8, 0x04 A4R4G4B4, 0x05 R5G6B5, 0x06 A8R8G8B8, 0x19 A8, 0x1A A8L8 (Morton
  swizzled). The format codes are to be verified by decoding and looking.
- `bake::tier`: the largest copy per name across PS2 / Xbox / PC; the PS2 alpha mask carried as for tier 2.
- `_bump` maps are not this card (a look card of their own if wanted).

## Acceptance
- A per-format decode test on one known texture each (a picture checked by eye + a hash).
- A census per map: textures taken from each source (counts) - BF2 GEO1 and BF1 NAB2 at least.
- A contact sheet of the 20 biggest changes for the maintainer. The sim hash and every non-golden gate unchanged (look only).

## Results (build, 2026-10-21, branch xbtex de77a64)
- Decode: `mungefs/tests/suite/xbox_tex.rs` pins one texture per format (0x00 0x04 0x05 0x06 0x0C 0x0E 0x19 0x1A) by
  hash after looking (`lead_tmp/xbtex/look/fmt_sheet.png`, `a4r4g4b4_water.png`); the non-square swizzle checked
  against the PS2's own NAB2 sky: mean |xbox - ps2| 3.72 of 255, re-swizzled control 40.61. Every 2D surface of both
  discs decodes (5,748; the 14 left are cube maps / one volume). A8 decodes black (D3D's rule): not a donor.
- Census (`lead_tmp/xbtex/census/census.txt`): BF2 GEO1 pc 6 -> 9 + xbox 8; BF1 NAB2 xbox 0 -> 18; maps total pc
  330 -> 335, xbox 131 (every BF1 map gains, 116 in all); sides BF1 0 -> 38, BF2 29 -> 63; common BF1 0 -> 8, BF2 12 -> 16.
- Found on the way: 90 of the 91 detail-compressed bases (PS2 64x64 base + 256x256 `_dtl`) have a donor that IS the
  whole painting (`look/dtl_vs_xbox.png`): taken at the companion's size or more, drawn without the companion
  (`game::assets::load_texture`; test `the_detail_companion_goes_with_the_ps2_base`, mutation-checked).
- Sheets: `census/top20.png` (all sources), `census/top20_xbox_maps.png` (Xbox map textures).
- Gates without the GPU (`lead_tmp/xbtex/gates2/SUMMARY.md`): sim, simhash, sweep, arena, leak, ordnance identical.
