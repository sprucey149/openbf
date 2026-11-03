---
id: CSPD
title: BF2 per-stance ControlSpeed: every BF2 team class inherits com_inf_default's sprint / jump / roll turn factor 0.35, the port's CONTROL_SPEED is the BF2 constructor's defaults (0.2 / 0.5 / 0.6) - read ControlSpeed through the class chain on BF2 maps (heroes' and jet troopers' rows too), keep BF1's table
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
UAUD2 (analysis/unit-audit-bf2.md §3.6). A BF2 soldier on the PS2 turns at 0.35 x MaxTurnSpeed while sprinting, jumping and
rolling; the port uses 0.2, 0.5 and 0.6 on every BF2 map. The values are measured (disc + RAM); how the difference
feels in play is not.

## Evidence
- Setter: B2 FUN_00252bb0 172005-172053 parses `ControlSpeed = <stance> f f f` ('%s %f %f %f', ELF 0x61e8b8); stance index
  by fnv(name): stand 0, crouch 1, prone 2, sprint 3, jump 4, roll 5, jet 6, tumble 7; columns at +0x77c / +0x79c / +0x7bc.
- Constructor FUN_00250488 column 3: 1.0, 1.0, 0.5, 0.2, 0.5, 0.6, 1.0, 0.1 (169403-169418) = the port's CONTROL_SPEED
  (core/src/soldier.rs 28-37); the port multiplies column 3 into yaw and pitch rates (279-286).
- INGAME.LVL com_inf_default: sprint 1.50 0.50 0.35, jump 0.03 0.03 0.35, roll 0.02 0.02 0.35; a derived class copies it
  (FUN_00250dc8 169847-169849). RAM (ramclass2.py, 23 BF2 states): 570 class objects carry 0.35; the ctor rows remain only on
  the engine base soldier and the Ewoks. Verifier V5 SUPPORTED.
- Other authored rows: com_jedi_default sprint 2.5 0.5 0.5, jet 1.5 1.25 1.25, jump 0.1 0.1 0.6; com_hero_default jet 2.0 1.25
  1.25, jump 0.1 0.1 0.4; the jet troopers' jet 1.5 1.25 1.25; Yoda sprint 3.5, Vader 5.0, Grievous 3.0 0.2 0.2.

## Acceptance
- SoldierConstants (or the unit's class) carries the 8x3 table from the class chain on BF2 maps; BF1 keeps today's table
  (BF1 authors no ControlSpeed, UAUD1 census). Cite the PS2 reader of +0x7bc for the turn use before changing its meaning.
- A unit test pins the BF2 rifleman's sprint row (1.5, 0.5, 0.35) from the baked class RON; FEEL_NOTES line; declared sweep
  moves viewed.

## Notes
- The column-1 sprint factor 1.5 already matches; only column 3 of sprint / jump / roll differs for the team classes.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).
- **AUDIT AIRC, 2026-10-02:** since AIRC (63975ed) the jump row's columns 1-2 multiply ACCELERATION (B2 `FUN_00248560`),
  not a speed: a hero's `jump 0.1 0.1` is 7 m/s2 of air steering against the team classes' 2.1. The roll row is the
  same kind of number on the PS2 (card ROLLM). Live RAM again: cs_turn 0.35 for sprint / jump / roll on BF2 Coruscant's
  rebel soldier (`lead_tmp/audit-airc/ps2/out/bf2_events.json`).
- 2026-10-04 (the maintainer's playtest, via Claude): the SPRINT turn factor is the maintainer's 0.6 on every map since commit below (`core::soldier::CONTROL_SPEED`, FEEL_NOTES) - a named deviation; this card keeps it and reads the PS2's jump / roll factors and the per-class rows only.

## Note (AUDIT re-sprint-clip, 2026-10-22, lane 1)
- The sprint row's FIRST factor is the sprint speed multiplier: the PS2 sprints at ControlSpeed[sprint].fwd x MaxSpeed
  per class (B2 FUN_00240bf8 case 3, B2:160477-160481; ctor 1.5). Infantry author 1.50, heroes more: com_jedi_default
  2.5, cis_hero_grievous 3.0, rep_hero_yoda 3.5, imp_hero_darthvader 5.0 (agent N6, `lead_tmp/audit-sprcl/repro/N6/`);
  the port's fixed CONTROL_SPEED[3][0] = 1.5 sprints every saber hero 1.7-3.3x too slowly once heroes are fielded.
