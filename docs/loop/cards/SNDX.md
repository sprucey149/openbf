---
id: SNDX
title: Some sounds play as static (the maintainer's playtest 2026-10-04): the rocket launcher fired at the player on a Republic-vs-CIS map and BF2's shotgun sound like TV noise, 'a terrible noise' - find which baked sounds are wrong (format / rate / channel / loop point of the decode), compare each with the PS2's own output, fix the bake
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-04: "Some sounds at least in some maps are terrible static sounds that sound like an old static tv ... The rocket launcher when I was playing as the republic and the droid would shoot me with a rocket would do this sound. It was a terrible noise. Same with the bf2 shotgun noise - not quite right".

## Evidence
- Maps played that day: BF2 KAM1 (Republic) and BF1 KAS2 (Empire). The rocket is the CIS launcher's (fire and / or the ordnance's own sound); the shotgun is BF2's.
- No sound check since FOLY has been heard by anyone (STATUS): every one is a name and a time in a log, so a wrongly decoded sample would pass.
- A GUESS, not checked: those samples are baked with the wrong format, rate, channel count or loop point.

## Notes
Filed 2026-10-04 by Claude in chat from the maintainer's playtest (build 1bde70c, main 4e51e84). The maintainer's own words are quoted; nothing here is measured unless it says so.

## Result (2026-10-06, landed 2b27875; report `reports/20261006_2030_card_SNDX.md`)
- The card's guess is wrong: the bake is clean. `scripts/sound_bake_check.py`: 9,160 baked takes of 34 map directories against ffmpeg's decode of the disc bytes - rate and frame count equal on all, 9,035 equal frame for frame, 6 takes (125 files) differ only after a saturated frame; no file has over 1 % of its frames at full scale. Disc census: 73 banks, 14,521 real samples, 12,247 aliases, no bad block header, every alias has its target's rate and size.
- **Shotgun: found and fixed.** The port started `FireSound` per bolt (8 a pull); the PS2 starts it once per pull (re-weapon-sounds.md 7.7, verifier 10 of 10). One blast at the ear: 0.98 x full scale as 1 start, 5.1 x with 30 % of the frames clipped as 8. BF2 Kamino replay: 576 shotgun fire starts -> 72, sim hash unchanged. Also BF1's shotgun (5), bowcaster (7), the SBD's tri-shot (3) and every twin cannon without a `SalvoDelay`.
- **Rocket: NOT reproduced.** Its fire sound and its explosion start once each; their takes are the disc's. Card SNDB (the black box records the sounds) is the instrument for it.
- 2026-10-07 (SNDB session): **the mix-level line above was rendered at 60 ticks a second; the sim runs 30** (`core::TICK_HZ`), so starts were twice as dense as real. `kam1_fix.csv` at 30: peak 1.19 x full scale (not 1.52), 0.0010 % of frames over it (not 0.023 %). The conclusion stands. `scripts/tools/mix.py` is fixed and reads black box snapshots (card SNDB); the old one is `mix_pre_sndb.py`.
