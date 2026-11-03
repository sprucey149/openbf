---
id: BHUD2
title: The BF2 HUD, part 2 — draw it: a HUD interpreter over BHUD's baked tree (Group, Bitmap, BarBitmap, BarSegmented, Text, Model3D; fader, colour pulse, z-order, Shadow) with the PS2's own screen mapping and Hor+ anchors, fed from the sim — health, stamina, weapons + ammo (BF1 IconTexture fallback), reinforcements, the reticle; retires the egui placeholder
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
analysis/re-hud.md §2–§4 (card BHUD's RE): the PS2 draws every HUD element from the hud_ tree under one camera; the port's
HUD (engine/goldens/tat1_hud.png) is an egui placeholder in neither BF2's layout nor its art.

## Note from BHUD's build (2026-09-30)
BHUD baked the layouts (`common_bf2/hud/<layout>.ron`, index `hud/index.ron`), the fonts (`common_bf2/fonts/`) and ten
COMMON/CORE sheets into `common_bf2/textures/` (schema `manifest::hud`). It did NOT bake the four side-LVL HUD meshes
(`hud_cis_trishot` CIS, `hud_wampa` SNW, `hud_stealth` ALL, `hud_imp_flamethrower` IMP): they belong with the side
weapons — add them to rebake.sh's side group (`bake weapon <SIDE>.LVL hud_… side_bf2`) when this card draws weapon icons
(declare `rebake=side`). Records keep their names case-blind (`HudNode.name`; unnamed = `#` + 8 hex digits).

- Planner 2026-09-30 17:30: Two new consumers: SQMK's one-line order text ("Follow me — 3 units", drawn in the placeholder box now) moves into the PS2 HUD message style here; EVMSG (M3 5) replaces the placeholder kill/capture feed with BHUD2 elements once its RE is done (its build deps BHUD2).
- EVMSG RE 2026-09-30 (`analysis/re-event-messages.md`): the interpreter needs a MultiLineText (queue / admit / expire by DisplayTime / fade / scroll, EventText + EventColor; RE §2 has the state machine and the PS2 timings) — used by `player1systemMessages` and `player1tooltipsTEXT`; build it here or leave it to EVMSG (whichever lands first). The `player1stats` group (points + kills pop-up) is fed by EVMSG.

- Planner 2026-09-30 21:45: value 4 -> 5. It gates EVMSG (5), BMAP (4) and the restyle of SQMK's order text, and the placeholder HUD is on the alpha checklist (PLAN: "the real BF2 HUD").
