---
id: SNDB
title: The black box records the sounds: the last 60 s of sound starts (name, take, gain, pitch, distance) in the snapshot and the digest, with a line per overloaded tick - so the maintainer's next 'terrible noise' (the rocket fired at him, not reproduced by SNDX) is named by F9
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-04: "The rocket launcher when I was playing as the republic and the droid would shoot me with a rocket would do this sound. It was a terrible noise." Card SNDX found and fixed the shotgun's cause (8 stacked fire sounds) and could NOT reproduce the rocket's: nobody can name a sound the maintainer heard, because the black box holds every unit's state and no sound.

## Evidence (card SNDX, report `reports/20261006_2030_card_SNDX.md`)
- The rocket's own sounds start once each in a headless BF2 Kamino battle (`SOUND_LOG`): `cis_weap_inf_rocket_launcher_fire` (take 643bbc70, 20,012 Hz, gain 0.7) and `com_weap_rocket_small_exp` (6 takes, full level inside 40 m). Their baked takes are equal to ffmpeg's decode of the disc bytes, frame for frame.
- The replay's player never stood within 130 m of a rocket, so "what else starts at the ear in that second" is not measured.
- The PS2 plays a looping, doppler-shifted hum on a rocket in flight (`com_weap_inf_ord_hum_rocket`, re-weapon-sounds.md 5); the port plays none (card WHUM). So the port's rocket is missing a sound, not adding one.
- Mix level in that battle after SNDX: peak 1.52 x full scale, 0.023 % of the frames over it (`scripts/tools/mix.py`) - no headroom problem at 130-250 m; unmeasured at the ear.

## Notes
Filed 2026-10-06 by the SNDX session. Tools: `engine/artifacts/lead_tmp/sndx/` (bank_census.py, vs_ffmpeg.py, mix.py), `scripts/sound_bake_check.py`.
A quick check the maintainer can do without any of this: `ffplay engine/assets_baked/bf2_kam1/sounds/59951779/643bbc70.wav` is the CIS rocket's fire take exactly as the disc holds it.
- 2026-10-07 planner: **value 4 -> 5.** the maintainer's 'static' report (SNDX, value 5) is half open: the shotgun is fixed (2b27875), the rocket's noise was not reproduced in a headless battle. This card is the only way the next time he hears it becomes a named sound, and it is small; it should be on main before his next playtest. Undo: `loopctl.py set SNDB value=4`.
