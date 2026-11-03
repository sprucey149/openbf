---
id: DEKA
title: The droideka as a fielded CIS class: BF1 fields 2-4 per Clone Wars battle and the player can pick it; the port's roster rejects it (BASE walkerdroid) - the walker/ball entity, its repeater and shield, its AI
status: archived
issue: 85
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #85](https://github.com/sprucey149/openbf/issues/85).

## Why
UAUD1 (analysis/unit-audit-bf1.md §2, §3): `cis_inf_droideka` is in `AddUnitClass` on all 13 BF1 Clone Wars missions (2-4 each), the
PS2 has 1-4 live droidekas (vtable 0x530138, max health 450) in every one of the 20 CW save states
(`engine/artifacts/lead_tmp/unit-audit/ramcensus_bf1.txt`), and the class-select screen offers DROIDEKA - Repeating
Blasters, Shield Emitter (`lead_tmp/unit-audit/pcsx2/cis_classes_sheet.png`; played as the player:
`pcsx2/deka_sheet.png`). The port drops it: `ClassRoster::build` rejects a BASE chain that does not end at `soldier`
(game/src/roster.rs 354), so every CW battle has no droidekas.

## Evidence
- Disc (SIDE/CIS.LVL sublevel `cis_inf_droideka`): MaxSpeed 5.0, MaxStrafeSpeed 3.0, MaxTurnSpeed 1.25, Acceleraton
  20, MaxHealth 450, WeaponName `cis_weap_inf_repeater` + `cis_weap_walk_droideka_shield` (BASE shield), ~70 unnamed
  props (ball / deploy), EngineSound `cis_walk_droideka_engine_parameterized`, its own camera section, ExplosionName
  `com_inf_droid_exp`; model `cis_walk_droideka` with its own `zaa_` bank.
- `analysis/re-droideka.md`: the ball form and jump, both games (PLAN §Movement & feel: BF2's ball jump everywhere).

## Notes
- BF2's CW missions name it too (UAUD2). PLAN's "the BUILD stays HELD (P7-class)" predates M2 being active - the
  planner lifts or keeps the hold.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §2, §3.8): BF2's `cis_inf_droideka` (BASE walkerdroid, MaxHealth 550, repeater +
  shield emitter, anim banks `cis_walk_droideka` 12 clips + `droidekafp`) is the CIS special slot of all 16 BF2 CW
  conquest scripts, and the port rejects it on all 15 CW RONs. The PS2 fields 1-2 in every one of the 17 CW save
  states (30 records, 20 alive; the soldier layout: class at entity+0x8 / +0x4a4, health at +0x170). For the player it
  is locked until 12 points (PointsToUnlock -> UNLK).
- DSHN audit (2026-09-29) [D]: BF1's shield bubble probably draws with the `specular` lookup table through its OWN
  records, not the model segments' Cs·Ad + Cd. The attribution rests on `FUN_0037bb18`'s vtable 0x549428 sitting among
  the 'Shield' / ShieldEffectClass RTTI strings; the draw is not traced to a live shield. `FUN_00389338` (its own
  'specular' string at 0x549db0) sets:
  - 0x648af0 = 0x148 (Cs·As + Cd), TEST 0x51011, PRIM 0x50;
  - 0x648b00 = 0x100048 (Cs·As + Cd);
  - 0x648b10 = 0x4000000068 (Cs·0.5 + Cd);
  - 0x648b20 = 0x8000000168 (Cs + Cd).
  They are read by `FUN_003879a8` / `FUN_00388ee8` (callers `FUN_0037bb18` / `FUN_0037c268`, vtable 0x549428). Read
  them when the shield's look is built (`lead_tmp/audit-dshn/`).

## Result (RE, 2026-10-03, lane 3) - closed, split into DEKU / DEKS / DEKD / DEKF
`analysis/re-droideka-unit.md` [O] (commit 28f1f90; verifier 37 claims: 28 SUPPORTED, 9 PARTLY folded in, 0 NOT).
- Measured on the PS2 (live, game-clock timed): the ball moves at TWICE its stored velocity in both games (ratio
  1.999; top 12.96 m/s BF1, 11.99 m/s BF2; bots cruise 10.2 / 11.9 m/s; free fall 39.2 m/s^2) - `re-droideka.md`'s
  6.5 / 6.0 m/s corrected by a banner; fold 0.50 s, unfold 1.67 s in both; shield -100.0 /s up, +300.0 /s down.
- Shield: 3000 points, a press toggles, down while rolling; a 1.25 m sphere that takes a bolt's damage and passes
  none on; a body hit skips it; BF2's full shield does not drain until hit.
- AI: rolls everywhere, deploys only for a fight (and the stuck walk); shield up only in the attack state when down
  and over half; sight starts a fight only within 10 m; stand and shoot. RAM: 218 of 218 ball rows order "roll".
- Walk: BF1 root motion x 2 (2.26 m/s), no strafe; BF2 2.5 m/s with strafe. Collision bodies of both discs.
- Build order: DEKU (the unit in core, not fielded) -> DEKS (shield) -> DEKD (bake + draw + sounds) -> DEKF (roster +
  bots; deps SPL11, the planner may lift it). The PLAN hold ("P7-class") is the planner's: these cards are M1, alpha.
- Open [H]: explosion vs a raised shield, the two barrels' alternation, BF1's pad button for the fold.
