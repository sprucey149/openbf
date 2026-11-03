---
id: VHUD
title: No HUD in a turret or vehicle seat: the weapon readout, the reticle and the target box are built only on foot (`on_foot` in game/src/main/battle.rs hud_model); RE and draw the PS2's seat HUD (the seat weapon's ammo/heat, its reticle, the vehicle's health) for both games
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer's playtest 2026-10-28: "turrets and vehicles don't have the hud".

## Evidence
- `hud_model`: `let on_foot = me.alive && me.mount.is_none();` gates `weapon`, `reticle` and `target` (battle.rs 432-499 at 665865d8).
- Read first: `re-heat-hud.md`, `re-turret-draw.md`, `re-zoom.md` §turret scope (card ZSCP).

## Acceptance
- An RE note (both games): which HUD elements a pilot / gunner / building-turret gunner sees and what each reads.
- Built: the seat HUD shows them; a PCSX2 pair per seat kind (hover, walker, building turret).
