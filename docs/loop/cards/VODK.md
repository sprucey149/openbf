---
id: VODK
title: Voice-over ducks the battle: while a commander / VO line plays the PS2's voiceover bus attenuates main (BF1 to 0.5, BF2 to 0.65, 0.5 s fade in and out) - the port plays its lines over an undimmed battle
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
analysis/re-mix-headroom.md 1.4 claim 10 (card MIXH, 2026-10-16): each game's CORE.LVL holds ONE bus with AttenuateBus - voiceover attenuates main x 0.5 (BF1) / 0.65 (BF2), AttenuationLeadIn / LeadOut 0.5 s (lead_tmp/mixh/tools/buses.py; key hash 0x247db064 3 times in 323 LVLs). RAM: BF1 lead_tmp/mixh/ps2/shot2.p2s holds main at 0.5 with every effect voice's gain halved (voice +0x44 = +0xf4 x 0.5); 5 of 137 BF1 states hold main below 1.0 (0.5, 0.533, 0.818: fades). The port's commander lines (sounds::play_voice, windowed.rs) and unit chatter play over the battle at its full level, so they are harder to hear than on the PS2.

## Evidence
analysis/re-mix-headroom.md 1.4 claims 10-12; lead_tmp/mixh/tools/duckcount.py; the forum archive's mod-tools note (community_mods/gametoast_lua/doc/Archives/Gametoast.com/forum_data_part_3.json, 'AttenuateBus'): 'whenever in the stock game voice over announcements are played, the rest of the game's audio is faded until the voice over is finished'.

## Acceptance
- A test: while a voiceover-bus line plays, every main-bus sound (effects, music, ambience, chatter on BF2's main) is handed the mixer at x 0.5 (BF1) / x 0.65 (BF2), reached over 0.5 s after the line starts and back to 1.0 over 0.5 s after it ends; the VO line itself unchanged. Mutation-checked.
- A PS2 check: one RAM read of the main bus's final gain (lead_tmp/mixh/tools/duckcount.py's bus walk) during a commander line on each game, against the port's value at the same moment of a replay.

## Notes
Which properties are on the voiceover bus comes from the bus MIXH bakes (BF1 1,090 properties, BF2 6,679). Read which bus BF2's unit chatter uses (battlechatter, under main: ducked too) before building.
