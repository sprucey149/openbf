---
id: SEGU
title: Untextured and dangling segments: what the PS2 draws for a `segm` with empty `TNAM` slots (75 stubs on 8 maps) or a texture name with no `tex_` chunk
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 18: `yav_prop_ivy`, `Cor1_bldg_flowers`, `noIcon`; the port skips them (PROVISIONAL). Pulling `yav_prop_ivy` from
the PC install is a classic-plus call (the maintainer). Overlaps LEAF / SKIPC — read theirs first.

## Acceptance
What the PS2 does with a missing texture ([O]: skip, white, or a default), checked against a PCSX2 pair; a build card.

## Notes
- 2026-09-29 (PXWB, lane 2; FINDINGS F55) [H]: BF2 DEA1's control room at cp5 — the PS2's consoles carry screens, keypads and a red panel, the port draws flat grey wedges: `pcsx2/ref/bf2_dea1/walk/pairs/cp2_cp5__sheet.png` b090 (180,200,470,360) PS2 (57,42,52) sd 33 vs port (32,38,44) sd 7; b000 (0,190,150,300) sd 20 vs 3. No console instance within 14 m of cp5 in the manifest, so they are building segments; DEA1 has 11 untextured-on-disc segments (walkabout INDEX).

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
