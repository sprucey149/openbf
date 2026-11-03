---
id: PRAISE
title: Teammates' voices on the player's kill streaks ("glad you're on our side!"): trigger, radius, speaker, cooldown, lines per faction
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX 2026-09-30 11:40: nearby teammates say things like "glad you're on our side!" after the player's kill
streak, in both PS2 games. Filed so it is not lost; belongs with USND's unit voice work.

## What to find
Which events trigger it (streak counts, who is near, which class speaks, cooldowns) and the lines per faction.

## Planner 2026-10-12 19:15 (the maintainer's INBOX 18:55, ask 3: "unit chatter, all of it ... the player-praise lines")
No longer post-alpha: value 3 -> 4, `alpha: yes`, `deps: USND`. USND is the umbrella and its RE census reads this
card's trigger with every other unit-voice event; this card is then the praise part of USND's fourth build slice
(the RE session re-phases it to `build` with the trigger it found, or closes it MERGED into that slice).
Undo: `loopctl.py set PRAISE value=3 deps= alpha= defer=post-alpha`.

## MERGED into UVO2 (USND's RE, 2026-10-13, `analysis/re-unit-chatter.md` 3.3-3.5 `[O]`)
The praise is BF2 `VOSound` events, read with every other unit-voice event; card UVO2 builds them:
- **Trigger**: only a HUMAN's kills. `HeadshotKill` (event 9): on his kill of an enemy [H: the headshot test itself
  was not found on the lines read]; `KillingSpree4` / `KillingSpree8` (0xc / 0xd): his kills each within 4.0 s of
  the last, at the 4th and the 8th (12 and 16 fire too and no class has a line for them).
- **Speaker**: HeadshotKill names none, so the player's nearest team-mate within 50 m (`VORadius`, default, authored
  by no class) says it, 3-D through the voice queue; the spree events name the unit of the stats record's character
  [H: read as the killer himself - then his own soldier says it at once, 2-D; the heroes' `hero_<name>_KillingSpree4`
  lines fit that].
- **Cooldown**: the line property's own gate, 40 +- 20 s (`cis`, `imp`, `rep`), 80 +- 40 s (`all`), 30 +- 5 (`wok`).
- **Lines**: 189 HeadshotKill, 214 / 190 KillingSpree4 / 8 `VOSound` chunks on the disc; per class in
  `engine/artifacts/lead_tmp/usnd/bf2_classes.tsv`.
- **BF1: none found.** The soldier class has no such key (11 sound keys + the 30 squad-order keys censused), and
  live, BF1 Naboo with the PLAYER fighting (`lead_tmp/usnd/ps2/run_kill.sh`: ps2live's playing bot, 15 kills and 48
  hits in 150 s, the longest run of kills inside 4 s steps was 2): 99 voice lines, every one a usual call (acquired,
  flee, heard, the AI's squad talk, the Gungans) or a commander line - nothing follows a kill
  (`lead_tmp/usnd/ps2/k1_nab1.csv`, `k1_nab1_fight.csv`). The maintainer's "in both PS2 games" (INBOX 2026-09-30) is not
  borne out for BF1 by one map and 15 kills; a BF1 line said only after a longer streak is not excluded.
- BF2 not measured live: no capture's player killed. A BF2 run of `run_kill.sh`'s kind needs a BF2 playing bot
  (ps2live's is BF1 only); it would settle the two [H]s - `usnd_live.py` names the property and its gate.

## MERGED note (card UVO2, 2026-10-16)
UVO2 built BF2's KillingSpree4 / 8 lines (a human's kills each within 4 s, said by his own unit, 2-D; `game/src/vo.rs` `Vo::tick`). HeadshotKill (event 9) is NOT built: the port reports no head hit (card UVO4 item 4). BF1's side of "praise" is untouched.
