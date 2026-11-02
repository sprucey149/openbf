---
id: UGAL
title: The unit gallery gives every soldier its side's rifle (gallery::discover_units' naming guess); use the roster's weapon per class (WeaponName -> GeometryName, with inheritance) — BF2 cis_inf_sbdroid holds a rifle it does not have; view and re-bless every moved unit golden
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/gallery.rs,engine/crates/game/src/roster.rs,engine/crates/game/src/classdb.rs,engine/crates/game/tests/roster.rs,engine/CRATES.md
declares: goldens=units/bf1_all_inf_marksperson.png,units/bf1_all_inf_marksperson_snow.png,units/bf1_all_inf_pilot.png,units/bf1_all_inf_trooperfleet.png,units/bf1_all_inf_wookie.png,units/bf1_cis_inf_bdroid.png,units/bf1_cis_inf_sbdroid.png,units/bf1_imp_inf_atatpilot.png,units/bf1_imp_inf_atstpilot.png,units/bf1_imp_inf_pilot.png,units/bf1_imp_inf_scout.png,units/bf1_imp_inf_shocktrooper.png,units/bf1_imp_inf_snowtrooper.png,units/bf1_rep_inf_arctrooper.png,units/bf1_rep_inf_jettrooper.png,units/bf2_all_inf_bothanspy.png,units/bf2_all_inf_engineer.png,units/bf2_all_inf_marksperson.png,units/bf2_all_inf_markspersonsnow.png,units/bf2_all_inf_vanguard.png,units/bf2_all_inf_wookie.png,units/bf2_cis_inf_bdroid.png,units/bf2_cis_inf_sbdroid.png,units/bf2_imp_inf_atatcommander.png,units/bf2_imp_inf_scout.png,units/bf2_imp_inf_shocktrooper.png,units/bf2_rep_inf_arctrooper.png,units/bf2_rep_inf_clonecommander.png,units/bf2_rep_inf_ep3heavytrooper.png,units/bf2_rep_inf_ep3jettrooper.png,units/bf2_rep_inf_ep3sniper.png,units/bf2_rep_inf_feluciatrooper.png,units/bf2_rep_inf_jettrooper.png
needs:
branch: ugal
resume: landed 781acbd0 2026-09-29
created: 2026-09-28
---

## Why
The maintainer (INBOX 2026-09-28, the unit audit's first defect): `engine/goldens/units/bf2_cis_inf_sbdroid.png` shows the super
battle droid holding a rifle; it fires from its wrists. The cause is the UNIT GALLERY, not the game.

## Evidence
- `game::gallery::discover_units` (`engine/crates/game/src/gallery.rs` ~881-892, read by the planner): every soldier gets
  "the side's rifle, by the usual naming" — the first `<side>_weap_*rifle*.glb`.
- The game's own path (`roster.rs` ~393): class `WeaponName` → that weapon's `GeometryName`, with base inheritance. BF2
  `cis_inf_rifleman` (geometry `cis_inf_sbdroid`) → `cis_weap_inf_wrist_blaster` → no `GeometryName` in its chain, so the
  battle correctly draws no gun (the maintainer's reading; reproduce it).
- 52 unit goldens in `engine/goldens/units/`; every class whose weapon is not its side's rifle (droideka, magnaguard,
  snipers, engineers, heroes, …) is likely wrong the same way.

## Notes
- Created 2026-09-28 by the planner. Secondary slots (the SBD lists 4 weapons) are UAUD2 / GREN, not this card.
- UAUD1 (2026-09-29): the BATTLE has its own weapon bug - UWPN (a class sharing a body model draws the first class's
  gun). BF1's `cis_inf_sbdroid` golden holds `cis_weap_inf_sniperrifle` (its wrist blaster has no geometry).
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.2): the battle-side twin on BF2 is UWPN's 8 classes (the CIS engineer and sniper hold
  the assault droid's launcher on every CW map).
