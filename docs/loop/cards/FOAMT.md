---
id: FOAMT
title: The foam layer's `FoamTile(5,5)`: `mungefs::fx` parses it, the manifest drops it, so KAM1's foam keeps the base's Tile 4
status: archived
issue: 18
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #18](https://github.com/sprucey149/openbf/issues/18).

## Why
The foam copy of BF2 Kamino's sea tiles at the wrong rate.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §What next 2(c) (from OCF's landing 2026-09-23); `game/src/water_anim.rs:133`; `manifest::ManifestWaterConfig` lacks it.

## Acceptance
RE the foam texture matrix in the ocean VU path (`analysis/re-water.md`), then a build: manifest field, [REBAKE] `maps`,
the KAM1 goldens quantified and viewed.

## Note (2026-09-28, KOC's RE, lane 2)
BF1 KAM1's ocean is about to be on screen as well. The PS2 never draws Kamino's y = 0 terrain (`TerrainEnable 0`,
`analysis/re-terrain-enable.md`), and card KOC's build removes the port's plate.
- With the plate dropped, the port's ocean at F9's poses is within 2-7 levels of the PS2's mean.
- Its foam is fainter and finer: per-channel spread 4-10 against the PS2's 6-16, with a visible regular tiling. At
  `v4_down`, box (250,40,630,200): port (20,33,29) sd 6.0, PS2 (17,30,33) sd 16.2.
- Numbers and sheet: `engine/artifacts/lead_tmp/re/koc/port/{means_port_vs_ps2.txt,sheet_port_vs_ps2.png}`.
- For the planner: this raises FOAMT's visible cost once KOC lands (both KAM1s).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
