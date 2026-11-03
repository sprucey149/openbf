---
id: ELFB
title: BF1's alpha test keeps the failed texels' colour (Normal 0x02: TEST 0x5100b, AFAIL FB_ONLY — colour written, no Z): KAS2's platform-hut bamboo shows clear-black gaps and GEO1's Techno Union wreck lattice see-through holes where the PS2 draws them solid dark (0x07 words the port MASKs)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
ELFR moved the 0x1 words to OPAQUE; the two remaining PXWA defects are 0x07 words: KAS2 `kas2_bldg_hut_walls` 0x7 (the bamboo
curtain on `kas2_bldg_platform_roof` — exact (0,0,0) over 1.2–2.4 % of `bf1_kas2/walk cp1_cp5`/`cp2_cp4`, unchanged by ELFR, the
PS2 (43,34,30)) and GEO1 `geo_bldg_wrecked_02` / `geo_bldg_rafters` 0x7 on `geo_bldg_technounion_hull` (`bf1_geo1/walk cp3_cp10`).
re-material-flags §2 [V 594351]: BF1 Normal mode 1 → TEST 0x5100b = ATE GEQUAL AREF 0x60, **AFAIL FB_ONLY** — a failed texel
still writes its RGB, only the Z write is dropped (BF2 uses KEEP). So what shows through a gap depends on draw order:
anything drawn LATER behind it overwrites it, anything drawn EARLIER (terrain?) is covered by the texel's colour.
## RE (bounded)
The BF1 bucket order for opaque vs tested draws (`FUN_0049e260` / `FUN_0049e2c0`, §2) against the terrain, sky and water passes;
then a port model (e.g. the failed texels drawn colour-only, depth-tested, no depth write, right after the opaque pass) and a
PCSX2 check on the leaves/bamboo 0x07 cases the census calls MASK-and-agreeing (a tree's failed texels over the sky: the PS2
must show the sky there, or the order says otherwise). Watch the leaves: ELFR's census showed no 0x07 word changed.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.
