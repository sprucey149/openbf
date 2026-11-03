---
id: SSUB
title: Sound properties by the battle's sub-level: a planet's SOUND LVL defines one property once per sub-level (nab1cw, nab1gcw, nab2cw, nab2gcw) and the port keeps the file's LAST audible one for every map and era
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
VSND's replay referee (2026-10-03, analysis/re-vehicle-sounds.md 10b): NAB's fighter tank engine record has other
graphs in nab2gcw than in nab1cw, and the file's last definition gave 19,107 of 19,419 layer-ticks against 19,374
with the battle's own sub-level. The engine records are now picked by sub-level (`SoundLib::engine`,
`game::foley::sublevels`); the sound PROPERTIES are not: `game::sounds::merge_props` keeps the last audible
definition of a name (its own comment: 873 of TAT1's 2210 records share a hash, 10 differ; PROVISIONAL).

## Notes
- 2026-10-06 planner: **CLOSED as built by card SNDE** (e45d75a, `reports/20261006_1350_card_SNDE.md`). SNDE did this card's step 2 for another reason (the commander's era): `bake sounds` keeps each record's sub-level (`SoundProp::level`), `SoundLib` resolves a name in the battle's own sub-levels first (`foley::sublevels`), foley tables and engine records by the same list, and `merge_props`'s PROVISIONAL note now covers only two definitions INSIDE one sub-level. Its checks stand in for this card's test: `a_battle_plays_its_missions_own_sub_level` reads every name of the mission's sub-level from the disc LVL, 57 missions, 53,907 names, 0 of other audio (the SNDE report's numbers; I read `game/tests/sound_sublevels.rs` and `game/src/sounds.rs`, I did not run the test). **Not run by anyone: this card's census of names whose GAIN / PITCH / DISTANCES differ between sub-levels and the `SOUND_LOG` before / after on bf1_nab1.** The whole record is the sub-level's own now, so the rule is in; if a wrong gain is ever heard, `cargo run -p game --example audit_takes -- <mission>` is the tool. Undo: `loopctl.py set SSUB status=todo`.
