---
id: FARS
title: The far scene on the 13 maps with a drawn LOWR sheet: cull the heightfield (base + TALPH layers) at NearSceneRange[2] and draw the sheet past it, ruled map by map with PCSX2 pairs - KAS1's ~95%-holed sheet waits on FARC/REFL, POL1's dark shape, the sheet's live DATE mask and VU cull
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Past `NearSceneRange[2]` the PS2 draws the `LOWR` sheet, never the heightfield (`re-lowr.md` §0.2 [V]). The port draws the
whole heightfield to 2000 m on every map, because `OPENBF_FAR_CULL` / `OPENBF_LOWRES` are off. REDB's sweep measured the far
heightfield in view (vantage / ground px): BF1 BES2 59k/0, GEO1 168k/0, HOT1 154k/3.4k, KAS1 146k/124, KAS2 306k/33k,
NAB1 310k/70, NAB2 20k/0, YAV2 44k/0, tat1 269k/13k; BF2 GEO1 149k/54k, HOT1 168k/3.3k, KAS2 159k/0, POL1 59k/0.
The maintainer wants this ruled map by map with PCSX2 pairs, never one global switch.

## Evidence
- `analysis/re-fog-amount.md` §6 (`lead_tmp/re/redb/sweep_table_t1.tsv`; `sweep_{vantage,ground}_asis_vs_sheet.png`, viewed).
- The sheet's LIVE GS words (§2, verifier-found): TEST 0x5c10b / 0x5c000 with **DATE = DATM = 1**, so the sheet draws only
  where the far target's alpha bit was set by the sky pass. The u64 0x800800 is VU cull enable plus side flip, so the sheet
  is one-sided (`re-vu1-cull.md`).
- Problems seen in the sweep:
  - BF1 KAS1's sheet is ~95 % holes (CLN5), so it shows black clear where the PS2 shows its far water and the mirrored
    shells (FARC, REFL);
  - BF2 POL1 shows a dark shape by the landing pad.

## Acceptance
- Per map (the 13 above): the heightfield (base + TALPH layers) is culled at NSR[2] and the sheet is drawn past it
  (`lowres_range`), or a recorded reason to keep the heightfield (e.g. KAS1 until FARC/REFL). Each is decided against that
  map's PCSX2 pairs.
- The sheet's DATE mask and one-sidedness are either modelled or shown not to matter on the shipped maps.
- Every moved golden is declared and viewed.
- FARC keeps the 5 sheetless maps with far heightfield in view: BF1 END1, RHN1, RHN2, YAV1; tat2.

## Notes
Created 2026-09-29 by REDB (lane 2): split out of REDB when its RE showed the red was the fog arithmetic, not the cull.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 4 → 3 until the alpha (restore from `value_pre_alpha`).

## Note (Claude, 2026-10-27, from the maintainer's INBOX note)
Choosing how the near scene meets the far image: a per-pixel dither + discard fade may be expensive on a weak GPU, and a
hard seam may look wrong. Measure both (fps and a view sheet) on the Iris Xe proxy before choosing.
