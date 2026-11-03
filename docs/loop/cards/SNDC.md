---
id: SNDC
title: Gameplay audio census: which sounds the PS2 plays in a battle (weapons, impacts, explosions, vehicles, foley, voice, music, ambience, UI) against what the port plays — one table per game on a showcase map, a card per gap
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's INBOX note 2026-09-29 22:40, item 1: M-ALPHA must "sound like a game". Only SNDP (value 1, sample edge cases)
and USND (unit voice) cover audio today. The port plays the primary's fire and reload sounds, the ordnance impact, door
sounds and the faction's squad chatter (USND's reading of `game/src/main/windowed.rs`). Nobody has listed what else the
PS2 plays.

## Result (2026-10-02, lane 3) - DONE
`analysis/audio-census.md`. PS2: two 105 game-second battles sampled live with a new RAM sampler
(`lead_tmp/sndc/snd_live.py`: every sound start by name from the 78 one-shot voices, the 39 handle voices and the 6
streams) plus a pause-menu / respawn sample each; port: a SOUND_LOG of the headless battle (scratch patch). 24 event
rows per game: the port plays 5 (BF1) / 4 of 21 heard (BF2). Second finding: the bake resolves `Inherit` only inside
one SOUND LVL, the templates are in CORE.LVL -> every port sound is gain 1.0 and silent beyond 50 m (disc: weapons
5 - 400 m). Cards: SNDT (M1 5, build), FOLY (M1 5, re), VSND (M2 4, re), WSND (M1 4, re), AMBM (M1 4, re); notes on
USND, GREN, PWUP. Report `reports/20261002_1250_card_SNDC.md`.
