---
id: HERO
title: Heroes and lightsabers (M4, PLAN §Heroes): playable BF2 heroes and BF1 AI heroes on all content, killable, BF2's hero health pool from the disc with conquest time/damage drain and kill-heal — the saber combat, the hero spawn/possession flow
status: archived
issue: 81
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #81](https://github.com/sprucey149/openbf/issues/81).

## Current health decision — 2026-10-30

Regular BF2 conquest now keeps the original time drain, damage and kill healing in the saber-shaped health pool. This supersedes the earlier no-drain policy; zero drain remains configurable. The broader BF1 and other-mode acceptance below is unchanged.

## Why
The maintainer (INBOX 2026-09-28 (d)): heroes and lightsabers have no card. PLAN §Heroes: playable BF2 heroes and AI BF1 heroes on
all content; always killable; BF2's hero health pool from the disc. The 2026-10-30 decision restores time/damage drain and kill healing for regular BF2 conquest.

## Acceptance
- RE ([O]): hero spawn/possession flow (BF2), NPC hero behaviour (BF1), the disc's hero health and kill-heal values,
  saber combat (melee, block, throw). Split into build cards. M4: not before M1/M2/M3.

## Evidence leads to reconcile

These are historical pointers, not fresh primary verification. UAUD1/UAUD2's class defaults and empty hero
populations do not establish live hero health, kill-heal or spawn rules. Their September statements about the
port's single weapon predate its current slot handling; recheck through the actual roster before planning a repair.
`analysis/re-powerups.md` §3.5 names an `AcceptHero` removal site at 0x18d0bc: trace its caller, replacement unit
and score/drop suppression alongside ANNC's `FUN_003e6cf8` manager. §5.3 and `analysis/re-powerup-station.md` §4
describe a hero-specific health refusal; an ordinary pickup/station heal is not evidence of the kill-heal rule.
`analysis/re-spawn-flow.md` §1's hero-class replacement branch is another possession/return-flow lead.
None of these pointers supplies the still-owed saber melee/block/throw derivation.

## Notes
- Created 2026-09-28 by the planner.
- UAUD1 (2026-09-29, analysis/unit-audit-bf1.md §2): every BF1 IA mission calls `SetHeroClass` per team - `all_inf_lukeskywalker` (the
  snow maps: `all_inf_lukeskywalkersnow`, geometry `all_inf_pilot`), `imp_inf_darthvader`, `rep_inf_macewindu`,
  `cis_inf_countdooku`: MaxHealth and AddHealth 1e6, NoEnterVehicles 1, one melee weapon (`*_weap_lightsaber`,
  AnimationBank `lightsabre`, not in COMMON.LVL), capes as `AnimationAddon`. None was live in the 22 BF1 save states
  (RAM census) although their classes are loaded. The mission RONs do not carry the hero class.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §2): the 33 BF2 conquest scripts load 18 hero classes via `SetHeroClass` (per-script pairs
  in the §2 table); none was live in the 19 ground save states. 16 of them write their weapons as WeaponName1..4
  (a saber and two force powers, or 3-4 gadgets), which the port's `props_all` reads as one weapon. `com_jedi_default`:
  MaxHealth 1200, MaxSpeed 9, JumpHeight 3.5 (jump velocity 11.2), its own ControlSpeed rows (sprint 2.5, jet 1.5)
  and a camera 5.2 m back; `com_hero_default` (Fett, Han, Leia, Chewbacca): JumpHeight 2.5. Root scales: Yoda 0.343,
  Vader 1.25, the Emperor and Chewbacca 1.15, Leia 0.95.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).

- USND RE 2026-10-13 (`analysis/re-unit-chatter.md` 2.8, 3.5, 3.7): a hero's saber hum and Vader's breath are the
  soldier's `LowHealthSound` with `LowHealthThreshold` 1.1 / 1.0 (BF1: `com_inf_saber_ambient` on four Jedi,
  `imp_inf_vo_darth_breath`; asked again whenever the unit's own handle is idle, for as long as he lives - card
  UTALK builds the rule for the natives). BF2 heroes carry `VOSound` lines (`hero_<name>_KillingSpree4`,
  `..._AcquiredTarget`): the event rule is card UVO2's.

- ANNC RE 2026-09-30 (`analysis/re-announcer.md` §2.6, [D]/[H]): BF2 hero voice and music. Class properties: `SoldierMusic` (0xf5b46be4, class+0x1264, e.g. `all_hero_Luke_lp`), 0xe4dee4ee (class+0x1258, `hero_<n>_spawn`), 0xd3a93472 / 0xef9e29f7 (class+0x125c / +0x1260, `hero_<n>_exhausted`), class+0x1254 (a fourth line); parser `FUN_00252bb0`. The hero HUD resolves them per team (`FUN_00383208` B2 381077-381083); the hero manager `FUN_003e6cf8` plays them through `FUN_00382580` / `FUN_003825c8` (spawn line at the hero + a HUD message) / `FUN_00382880` (exhausted). Gates `ScriptCB_EnableHeroVO` / `EnableHeroMusic` (`DAT_005fd083` / `DAT_005fd082`, default 1, read in the soldier spawn `FUN_002360c8`; only `tat2g_eli` clears them). When each fires is NOT traced. They are 2-D lines on `global_vo_slow`, i.e. the same voice queue as the commander's (ANNC builds that queue).

## Note (Claude, 2026-10-27, from the maintainer's INBOX note): PS2 behaviour to measure on PCSX2
Questions, not facts: does an AI hero ever appear in BF2 single player (sample at least 240 s); what a dead hero does
(animation, fade, any pickup dropped); the "Play as X?" prompt's layout, countdown and buttons; the unlock points, offer
time and respawn time; whether the PS2 drains hero health over time, and how fast. A drain would be a fact about the PS2
only: the 2026-10-30 decision now restores the verified drain for regular BF2 conquest. Combo data -> COMBO; blade, block, throw -> SABR.
