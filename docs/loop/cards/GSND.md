---
id: GSND
title: Game audio findings of cleanse 16: a hit plays the shooter's CURRENT weapon's collision sound (not the weapon that hit), a commander line's pitch from a different definition than its take, play_take ignores the slow cheat's rate, a hover engine's acceleration filter runs while off, foley's two era tables
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Cleanse 16 (card CLN11) read the game crate's audio code (diff 306c4f0..988b229). Findings, each from a reviewer's read; the lead
re-read only what is marked [lead]. Each is a hypothesis to trace on today's main before building:
1. MED, read: `event_sounds.rs` ~165-173 plays `wc_of(shooter).ordnance.collision_sound` for `Event::Hit`, which carries no weapon
   class (`core/src/sim/state.rs` ~98-104). A seat gunner's hits play his RIFLE's collision sound; a grenade / sticky blast pushes
   one Hit per victim (`core/src/sim/blast.rs` ~165), each playing whatever the thrower holds now, once per victim.
   Fix: put the weapon class on `Event::Hit` (or take it from the `Impact` beside it). Moves the sim's event struct: check simhash.
2. LOW: `sounds.rs` `play_voice` (~957-972) takes take, gain, length from `voice_take` (`find_named`, honours `foreign` sub-levels)
   but the pitch from `self.find(hash)`, which does not: an era-mix battle (`kas2gcw` Republic) can play a take at another
   definition's pitch, and the length handed to the voice queue then disagrees. Whether real pitches differ is unmeasured.
3. LOW: `sounds.rs` `play_take` (~978-1004, the positional squad answers) ignores `rate_mul` (the slow cheat) that `mix` applies.
4. LOW: `engine_sound.rs` ~209-223: a hover's filtered acceleration is updated while the engine is off, against a frozen
   `previous`, and not reset at turn-on: a shoved empty hover hands its next pilot a stale acceleration. Read the PS2 first.
5. LOW: `foley.rs` `sublevels` (~164-185) and `foreign_armies` (~195-221) each carry a map-prefix strip and a faction-to-era table;
   one shared helper stops them drifting.

## Acceptance
- 1: a test that a seat gunner's hit and a grenade's blast play the ordnance that hit (fails on main); simhash moves only if the
  event is hashed (declare it then).
- 2-5: a test each, or the finding closed with the PS2 evidence that it is right as it is. Every other gate PASS.

## Notes

## Merged: SPWS (planner 2026-10-16 11:09, batch rule: same files, both value 3 M1 audio; one session, one gates run)
Title: A battle's first tick starts 39 sounds at once (every soldier's equip sound and first step) - measure what the PS2 starts at a spawn and gate the port's. Its acceptance is part of this card's.

### SPWS Why
Found by card SNDB's first real snapshot (2026-10-07): the black box's only OVERLOAD line in 59 s of BF2 Kamino was tick 1.
Every unit spawns on the same tick, so every unit's equip sound and first footstep start on it. The maintainer hears it as a burst when a battle begins.

### SPWS Evidence
- `engine/artifacts/lead_tmp/sndb/win/bb/prev1.json` (build b870afc +dirty = branch sndb), digest `lead_tmp/sndb/win/digest.txt`:
  `OVERLOAD -58.97s tick 1: 39 starts, sum 13.58: com_weap_inf_equip_med x11 (4.10), com_weap_inf_equip_sm x13 (4.92), com_weap_inf_equip_lg x6 (1.80), rep_inf_trooper_walk_metal_left x9 (2.76) - nearest 2 m`
- `python3 scripts/tools/mix.py <that file> engine/assets_baked`: peak 1.59 x full scale, 84 frames over it, all in second 0 (`lead_tmp/sndb/win/prev1_mix.wav`).
- Unknown: whether the PS2 plays an equip sound on a spawn at all (it may only on a weapon SWITCH), and whether a soldier that has not moved steps.

### SPWS Acceptance
- The PS2's rule cited from `analysis/` or measured (ps2live sound-start sampler, memory note topic-ps2-live-samplers): which sounds a spawn starts.
- The port's tick 1 on the same scene (`win_run.sh` with a null ALSA device, see report `20261007_0205_card_SNDB.md`): no OVERLOAD line at the first tick unless the PS2 starts the same sounds; the count before / after in the card.- 2026-10-07 (SNDB session): the digest's one-take column for that tick: `one take at most x13 (sum 4.92)` - the 13 `com_weap_inf_equip_sm` starts are ONE take file, so they add up sample for sample (each start's own pitch draw aside), the same mechanism as SNDX's 8 stacked shotgun fires.
