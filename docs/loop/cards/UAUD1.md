---
id: UAUD1
title: Unit fidelity audit, BF1: every unit class on every BF1 map — weapon slots and what each draws, items/grenades, sounds, movement constants and animations, which map/era loads it — against the mission Lua + side LVL and a PCSX2 look; gaps become cards
status: archived
---

Historical technical record. Recorded state before archival was `done`.

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
- BF1 sides on the disc: `ALL CIS DES EWK GAR GEO GUN IMP REP WOK` (`extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/`).
  The special sides (GUN, EWK, WOK, GEO, DES, GAR) belong to TEAMS; this card audits the classes the IA maps load.

## Notes
- Created 2026-09-28 by the planner. Build a census script (props per class from the side LVLs) and leave it in
  `engine/artifacts/lead_tmp/unit-audit/` so the other game's card reuses it. UGAL fixes the gallery first.

## Outcome (2026-09-29, two sessions; commit 144b3df)
- `analysis/unit-audit-bf1.md` ([O], RE_LOG entry): 52 classes × 7 columns, a verdict each.
- The first session ended on an API error after building the census and filing the cards. The second re-derived the
  numbers, corrected six statements, ran a second verifier (8 SUPPORTED, 2 PARTLY, all folded) and measured one new
  gap.
- MATCH:
  - rosters: 26/26 RONs = the bytecode;
  - movement constants: 34 classes, 0 differences;
  - body models.
- New cards: DEKA, UWPN, USKN, USND, JETP, UORD, B1ME (first session) and CAMS (second session: the chase camera sits
  3.50 m back on the PS2, 3.2 in the port).
- Notes added to GREN, PWUP, ANIM, HERO, MSND, TEAMS, SPSEL, CFOVM, KNOCK and UGAL. USKN's slot question is answered
  (BF1 `FUN_001be1e8` + the models' MNAM chunks).
- UAUD2 reuses `engine/artifacts/lead_tmp/unit-audit/` (README).
