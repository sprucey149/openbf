---
id: UTALK
title: BF1's AI squad talk and the natives' idle voice (the maintainer's INBOX 2026-10-12 18:55, ask 3; slice of USND): an AI in a vehicle seat calls for a gunner and the bots that board answer yes, a trooper says follow / move out at its agent's two points (re-squad-orders 5), and a class whose LowHealthThreshold is over 1 - the Gungans, the Ewoks, the Jawa - says its LowHealthSound for as long as it lives; on the PS2 the yes answers are 12 of the Dune Sea's 32 voice lines a minute and the Gungans' takes 16 of Naboo's; RE done (analysis/re-unit-chatter.md 2.8, 5)
milestone: M3
value: 5
phase: build
status: done
deps: USND,UCALL
files: engine/crates/core/src/squad.rs,engine/crates/core/src/ai/command.rs,engine/crates/core/src/ai/ride.rs,engine/crates/game/src/announce.rs,engine/crates/game/src/event_sounds.rs,engine/crates/game/src/sounds.rs
declares: none (sound events only: no sim rule reads them, no hashed state; tests added)
needs:
effort:
branch: utalk
resume: LANDED e22e702 (report reports/20261016_1325_card_UTALK.md). Lines on the port's events; the PS2's wait-in-seat + boarding is card GUNWT; the Gungans' voice waits on the locals team (MSND note)
created: 2026-10-13
alpha: yes
---
## Why
The maintainer's INBOX 2026-10-12 18:55, ask 3 (umbrella card USND, slice 3 of his order): the AI's own squad talk and the
answers to it, and with it the natives' constant voice. On the PS2's BF1 these are most of the voice slot: on the
Dune Sea the "yes" answer alone is 12 of 32 lines a minute, on Naboo the Gungans' takes are 16 of 32
(`analysis/re-unit-chatter.md` 2.8, 5; `analysis/re-squad-orders.md` 5 `[O]`).

## Evidence
- **The AI's orders** (`re-squad-orders.md` 5): a trooper whose first path leg fails says `ai_<s>_..._tac_follow`
  (chance 0.3); a trooper agent leaving one branch says `..._moveout` (1.0); an AI waiting in a vehicle seat says
  `..._needgunner` (0.3) and the AI on foot within 30 m board and answer `ai_<s>_..._tac_yes`. Each `ai_` command
  property has PlayInterval 20 +- 5 s; the "yes" has none (its takes are locked 1 s, `all` / `imp` 2 s).
- **The idle voice**: `LowHealthSound` is asked whenever the soldier's own handle is idle while he lives and his
  health is at or under MaxHealth x `LowHealthThreshold` (default 0.3). Every class that authors the sound sets the
  threshold to 1.1 (Vader 1.0), so it runs for as long as the unit lives: the three Gungans
  (`gungan_chatter_heartbeat`, PlayProbability 0.2, seven takes locked 1 s, on the voice stream), three Ewoks
  (`end_inf_ewok_breathing`), the Jawa; the saber hums and Vader's breath are card HERO's.
- **PS2 rates** (`lead_tmp/usnd/live_bf1.txt`, per minute, post / fight): gate passes `ai_` follow NAB1 4.0 / 4.75,
  TAT1 4.5 / 4.5; need-gunner 2.75 / 3.75, 4.25 / 3.75; move out 0.75 / 0, 1.0 / 0.75. Lines: follow 3.0-5.75,
  need-gunner 1.0-3.5, yes NAB1 2.0 / 1.0, TAT1 12.0 / 12.25; Gungan takes NAB1 16.0 / 18.5. The pending queue
  stands at its cap of 4.

## Notes
- The AI's orders are behaviour as well as sound (bots board the caller's vehicle): `re-squad-orders.md` 4.4, 5 and
  9b ("Not built: the AI's own orders"). If the boarding half is too big for one session, land the lines on the
  port's existing seat events and file the boarding as its own card.
- BF2 has the same low-health rule for seven classes (Ewoks, acklay, wampa, Jawa, Vader): same code path.
- 2026-10-16 planner (from UCALL's report `20261016_0730_card_UCALL.md`): "grenade!" is short on NAB1 - 1.63 a minute against the PS2's
  4.00 (TAT1 2.22 / 3.25); the port asks only 4.0 flee requests a minute while the PS2's two properties sit near their 20 s ceiling.
  Before this card's census: count the bots' grenade throws that land within 7.5 m of another AI soldier on both (likely the throw
  rate, not the voice gate). `acquired` / `heard` are within 30 % already. Census kit: `lead_tmp/ucall/census/` (run.sh + tab.py).

## Result (2026-10-16, landed e22e702)
- Tests: core `a_seated_ai_calls_for_a_gunner_and_the_ai_near_it_answer`, `a_trooper_says_follow_as_it_sets_out_and_move_out_as_it_arrives`;
  game `a_low_health_voice_asks_again_only_when_its_line_has_ended`, suite `every_fielded_bf1_class_talks_as_its_sides_ai`,
  `the_gungans_low_health_voice_runs_while_they_live`. Mutations 14 of 14 KILLED (`lead_tmp/utalk/mut/mut.txt`).
- Census (`lead_tmp/utalk/census/tab_utalk.txt`, 8 seeds x 240 s, player parked, CW): gate passes / min NAB1 follow 5.59 (PS2 4.0),
  move out 1.84 (0.75), need-gunner 1.50 (2.75); TAT1 5.38 (4.5), 1.56 (1.0), 1.00 (4.25). Yes lines NAB1 0.84 (2.0), TAT1 0.97 (12.0)
  - NOT within 30 %: the PS2's AI driver waits in its seat re-calling and its answerers board (card GUNWT). Voice slot in all
  13.3 / 12.0 lines a minute (PS2 31.75 / 32.25): NAB1's 16 Gungan lines need the locals team (MSND).
- Mixer (`lead_tmp/utalk/win/`, TAT1 CW windowed 680 s, null ALSA): 21 talk starts in 6 snapshots; 1 OVERLOAD tick names a follow
  line (0.50 of 1.95; 1.45 without it); mix_ogg peaks identical with and without the talk lines in all six (`mix.txt`).
