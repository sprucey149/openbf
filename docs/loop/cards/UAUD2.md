---
id: UAUD2
title: Unit fidelity audit, BF2: every unit class on every BF2 map — weapon slots and what each draws, items/grenades, sounds, movement constants and animations, which map/era loads it — against the mission Lua + side LVL and a PCSX2 look; gaps become cards
milestone: M1
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
branch:
resume:
created: 2026-09-28
---

## Why
The maintainer (INBOX 2026-09-28, DECISION 3): audit every unit class on every map of both games; value 5, M1. The port models one
primary per class and its gallery guessed weapons (UGAL); nobody has checked the rosters against the disc class by class.

## Evidence
- Per class, check against the disc (the map's mission Lua + the side LVL) and a PCSX2 look:
  - every weapon slot and what it draws;
  - items and grenades;
  - sounds;
  - movement constants and animations;
  - which map / game / era loads which class.
- Known so far: BF2 SBD lists four weapons (`wrist_blaster`, `wrist_trishot`, `wrist_rocket`, `award_wrist_blaster`); the
  roster models one (UGAL, GREN).
- The BF2 hunt rosters and heroes belong to TEAMS / HERO; this card audits the classes the conquest maps load.

## Notes
- Created 2026-09-28 by the planner. Build a census script (props per class from the side LVLs) and leave it in
  `engine/artifacts/lead_tmp/unit-audit/` so the other game's card reuses it. UGAL fixes the gallery first.
- DONE 2026-09-29 (lane 2, report `reports/20260929_0645_re_UAUD2.md`): `analysis/unit-audit-bf2.md` [O] committed as 2d215df
  (+ RE_LOG). 33 `*_con` scripts, 76 classes, verdict per column. New cards CSPD, UNLK, B2JP; BF2 notes on UWPN, USKN,
  DEKA, GREN, PWUP, ANIM, USND, JETP, UORD, CAMS, HERO, MSND, TEAMS, SPSEL, SIMK, UGAL. Tools: `lead_tmp/unit-audit/*2.py`.
