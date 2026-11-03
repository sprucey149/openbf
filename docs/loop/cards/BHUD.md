---
id: BHUD
title: The BF2 HUD, part 1 — bake it: the five INGAME.LVL hud_ layouts as a named element tree, the four CORE.LVL bitmap fonts, and the sheets/meshes the port lacks (analysis/re-hud.md); BHUD2 draws it, BMAP the minimap
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why (RE done 2026-09-30, `analysis/re-hud.md`)
The whole BF2 HUD is DATA on the disc: five `hud_` config chunks in BF2 INGAME.LVL (`hudtransforms`, `1playerhud`,
`2playerhud`, `3playerhud`, `4playerhud`; the `mungefs::config` DATA/SCOP format; 145 of 156 property hashes named in
`engine/artifacts/lead_tmp/re/bhud/names.txt`, dumper `huddump.py`). Fonts: four `font` chunks in BF2 CORE.LVL. The port
bakes the INGAME sheets and meshes already, but not the layout, the fonts, the five BF2 COMMON.LVL sheets
(`hud_icon_flash`, `border_3_pieces`, `opaque_black`, `btn_directional_pad*`, `stats_kills`), CORE's `white`, or the
side-LVL `hud_*` meshes (`hud_cis_trishot` CIS, `hud_wampa` SNW, `hud_stealth` ALL, `hud_imp_flamethrower` IMP). This card
bakes them; BHUD2 draws them; BMAP does the minimap and world markers.

## Original acceptance (2026-09-28)
- RE: where BF2's HUD art and layout live on the disc (textures, fonts, the layout constants), per element (health,
  ammo, reticle — RETC, minimap, tickets, CP bar). → done, `analysis/re-hud.md`.
- Build cards: bake the art (BF2 first, BF1 icon fallback), draw it anchored per COMPARISON_RULES (no stretching).
  → this card (bake), BHUD2 (draw), BMAP (minimap + markers).

## Notes
- Created 2026-09-28 by the planner. RETC (the reticle) is one element; do it first or fold it in.
