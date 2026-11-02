---
id: USND
title: Unit chatter, all of it (the maintainer's INBOX 2026-10-12 18:55, ask 3; the umbrella card): every line the PS2's units say - the combat calls (acquired, hide, approach, flee, pre-damage, heard), hurt and death cries, the AI's own squad talk and order answers, low health, post and kill calls, the praise of the player - in both games, in the right faction's voice, with the PS2's triggers, cooldowns and radius; on the disc and in the bake, never played by the port. RE census of every unit-voice event first, then build in slices, each refereed by lines per minute against a PCSX2 capture; chatter must not clip the mixer
milestone: M3
value: 5
phase: re
status: done
deps:
files:
declares:
needs:
effort:
branch:
resume: RE done 2026-10-13: analysis/re-unit-chatter.md (docs 4d57f9c), report engine/artifacts/loop/reports/20261013_0815_card_USND.md. The build is four cards: UCALL (BF1 combat calls + the take lock), UCRY (hurt and death, both games), UTALK (BF1 AI squad talk + the natives' idle voice), UVO2 (BF2 VOSound)
created: 2026-09-29
alpha: yes
---

## Why
UAUD1 (analysis/unit-audit-bf1.md §3.5). Every BF1 soldier class authors these sounds and the engine reads them (HurtSound / DeathSound in
B1 `FUN_00221e48` 135984 / 136071; the chatter keys in `FUN_0020f2e0`, e.g. 123048; LowHealthSound 123549). The
names are already in the per-map sound bakes (BF1 KAS2 `sounds/names.ron` holds `all_inf_com_chatter_wound`,
`droid_death`, `all_weap_inf_rifle_mvt_jump`, `rep_weap_jetpack_turnon`, ...). The port plays only the primary's fire
and reload sounds, the ordnance impact, door sounds and the faction's squad-order chatter
(game/src/main/windowed.rs 18-113).

## Notes
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.5): BF2 moved the unit voice into `VOSound = <sound> <event>` lines: 162-211 per class
  for 41 of 49 fielded classes (32 events: SC_* squad commands + responses, *VO requests, AcquiredTarget, Grenade,
  MissileIncoming, HeadshotKill, KillingSpree4/8, FriendlyFire ...); the Wookiees 20, the Imperial officer 44, the
  Bothan spies / SBD / MagnaGuard none. Plus HurtSound / DeathSound (`droid_death` for the CIS) and the unnamed
  0x7241c3bb (`repmalechoke` ...). The port's squad chatter is silent on BF2 maps by design (windowed.rs 72-110); the
  BF2 command voice is the class's `SC_*` lines.

## The maintainer, 2026-10-12 18:55 (INBOX, ask 3) - planner 2026-10-12 19:15: value 4 -> 5, this card is the UMBRELLA
His wish (via chat): make the game lively - every line the units say to each other on the PS2, both games, the right
faction's voice, the PS2's triggers, cooldowns and radius. Not only the events in the old title.

### RE (this card's own phase; [O] with the verifier)
A census of EVERY unit-voice event on both discs, program-side (count the keys over every fielded class; a reading
is not a census):
- BF1: the soldier class's sound keys (`HurtSound`, `DeathSound`, `LowHealthSound`, the chatter keys of
  B1 `FUN_0020f2e0`, the `ai_<side>_..._tac_*` lines) - which code event asks for each, with what probability.
- BF2: the 32 `VOSound = <sound> <event>` events (`SC_*` commands and responses, the `*VO` requests,
  `AcquiredTarget`, `Grenade`, `MissileIncoming`, `HeadshotKill`, `KillingSpree4/8`, `FriendlyFire` ...) plus
  `HurtSound` / `DeathSound` and the unnamed 0x7241c3bb - the trigger of each event in the ELF.
- Per event, one row: the trigger (function and line), who speaks (class / faction voice - the Wookiee answers in
  `wok_inf_...`), probability, cooldown (`PlayInterval`, the per-take lock), radius and template (sample bank 3-D
  one-shot or the voice stream's queue, re-announcer 1), and whether it is the praise of card PRAISE.
- The PS2's baseline, live: `lead_tmp/sndc/snd_live.py` / `lead_tmp/annc/annc_live.py` on two maps a game (BF1 NAB1
  and BF2 UTA1 have earlier counts above; add one each), 3 minutes or more: lines per minute per event.
What is already read and need not be redone: the notes above (ANNC's queue, SQACK's squad-order voice, SNDC's
counts, FOLY's dead movement keys - still out of scope -, VSND's jet pack / droideka component).

### Then the build, in slices (each its own card, `deps: USND`, value 5, alpha; the RE session files them)
The maintainer's order: (1) the combat calls, (2) hurt and death, (3) orders and the AI's squad talk, (4) the rest (low health,
post and kill calls, the praise - card PRAISE is that part). A fact for whoever orders the work: hurt / death are
sample-bank 3-D one-shots that need no voice queue and are the most frequent unplayed sound near the player (NAB1
154 starts in 105 s) - the cheapest slice by far.

### Acceptance (each slice; the RE's is the census table, its verifier and the PS2 baseline file)
- A census line: the port's lines per minute per event on the capture's map (the SOUND_LOG patch or the black box's
  SOUNDS rows, 8 seeds) against the PS2 capture's - within 30 % per event that the PS2 plays at least 5 times.
- The right voice: a test through the BAKED class db that each fielded class's event resolves to its own faction's
  property (the Wookiee's, the droids' `droid_death`), both games.
- Cooldown and radius: a test per rule (a second ask inside the interval is silent; a speaker beyond the template's
  far distance is not started), mutation-checked.
- **The mixer (card SNDB's overload rule):** a 10-minute round with a sound device and the slice on writes NO
  `OVERLOAD` line that the slice's lines cause (the black box's digest), and `scripts/tools/mix.py` on its snapshot
  peaks under full scale. Chatter on the voice stream goes through `core::announcer::VoiceQueue::ask`.

## RE done (2026-10-13, lane 2) - `analysis/re-unit-chatter.md` `[O]`, docs 4d57f9c
- The census table is the document's section 0 (BF1: 9 rows; BF2: the 40-event table of 3.5); the verifier
  workflow's 40 claims and 53 verdicts are `engine/artifacts/lead_tmp/usnd/wf/claims.md` / `VERDICTS.md`; the PS2
  baseline is `lead_tmp/usnd/live_bf1.txt` / `live_bf2.txt` (two maps a game, the player at his post and in the
  fight, 4 minutes each).
- The premise of this card's title was wrong on three of the six "combat calls": hide, approach and pre-damage are
  never asked for on the PS2. And BF2's voice is built round the PLAYER (a speaker within 50 m of him), so its
  baseline has to be taken with the player in the fight.
- Build cards (each `deps: USND`, value 5, alpha): **UCALL** (BF1 combat calls + the take lock in the bake),
  **UCRY** (hurt and death, both games - eligible now, shares no bake change), **UTALK** (the AI's squad talk and
  the natives' idle voice; after UCALL), **UVO2** (BF2 `VOSound`; after UCALL). PRAISE is MERGED into UVO2.
