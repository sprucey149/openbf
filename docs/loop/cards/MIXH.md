---
id: MIXH
title: The port's mix has no headroom because its per-ear level law is hotter than the PS2's: the PS2 splits a 3-D voice linearly between the ears (L + R = gain x level; centred = 0.5 per ear, the port's kira pan gives 1.0, +6 dB) and plays BF2's effects bus at 0.63 - give the port the PS2's per-ear law and bus gains, then UCRY's fight logs must read per-ear clipping <= 0.03 % (RE analysis/re-mix-headroom.md)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/audio/src/lib.rs,engine/crates/game/src/sounds.rs,engine/crates/game/src/event_sounds.rs,engine/crates/manifest/src/lib.rs,engine/crates/bake/src/sounds.rs,engine/crates/mungefs/src/sound_props.rs
declares: rebake=sounds
needs:
effort:
branch: mixh
resume: LANDED a91d252 2026-10-16 03:35 (report reports/20261016_0327_card_MIXH.md). Next: UCRY re-measures its mixer clause on main (mix.py --law new --limit).
created: 2026-10-16
alpha: yes
---

## Why
RE done 2026-10-16 (`analysis/re-mix-headroom.md`, d1def96): the PS2 has no limiter or count-driven duck; its mix rarely
clips because each 3-D voice is quieter PER EAR than the port's. The PS2 splits a 3-D voice linearly, L + R = gain x
level (B1 FUN_0044cb78 / B2 FUN_00540b98, mode 3), so a centred sound is 0.5 per ear; kira's sqrt 2 constant-power pan
gives it 1.0 (+6 dB), a side sound 1.30 vs 1.0 (+2.3 dB). BF2's effects bus plays at 0.63 (slider 7/10 x `main` 0.9),
which the port does not apply (+4 dB more). PS2 front-line fights recorded off PCSX2's SPU2 output: 0-4 clamp runs of
0.2 ms in 4 minutes. The PS2's own starts, rendered through the port's law, clip like the port.

## Evidence
- `analysis/re-mix-headroom.md` 0 and 1.5. Kit `engine/artifacts/lead_tmp/mixh/`: `tools/levelcensus.py` (per-voice
  law in RAM, 739/888 BF1 and 549/659 BF2 within 1 %), `tools/ps2render.py` (PS2 starts under each law), `tools/plateaus.py`
  + `ps2/f_*.raw` (the PS2 captures), `ps2/run_audio.sh` (PCSX2 output capture recipe), `tools/buses.py` (disc bus tree).

## Notes
- Build spec (RE 2): kira carries the PS2's per-ear pair exactly: volume = g l sqrt(p^2 + (1-p)^2) / sqrt 2, panning =
  2m - 1, m = p^2 / (p^2 + (1-p)^2). Replace `game::sounds::pan_for`'s x 0.7 and every 3-D play site (one-shots `mix`,
  `loop_voices`). Bake each property's `Bus` (hash) into `SoundProp` (rebake=sounds); runtime bus table per game from
  RE 1.4 claim 10-11 (BF1: soundfx 1.0, voiceover 0.64, ambience 0.6; BF2: soundfx 0.63, battlechatter 0.9, voiceover
  0.63, ambience 0.378) - apply it to what the port plays from those buses; music keeps card MUSV's level.
- Decision to record in FEEL_NOTES: BF2's slider values (effects 7/10) are read from the capture memory card, factory
  default unread [H]; take them as the default effects level (reversible: one constant).
- Not here: the `voiceover` duck of `main` (card VODK); the 39-voice cap (silences only the quietest).
- After landing: re-measure UCRY's mixer acceptance on main (UCRY is blocked on this card).

## Why (conversion text, kept)
Card UCRY's mixer check (2026-10-16, `engine/artifacts/lead_tmp/ucry/census/mix/summary.txt`): the port's event
sounds rendered by `scripts/tools/mix.py` from headless SOUND_LOG take rows, the player held beside the team-mate
nearest an enemy every 20 s (a scratch `SOUND_LOG_FOLLOW` patch, `lead_tmp/ucry/census/run.sh`), the cry takes
REMOVED (`census/nocry.py`; UCRY does not touch the sim, so this is main's own sound for that battle):
peak UTA1 s4 3.52 x full scale (0.40 % of frames over), UTA1 s3 2.22 (0.05 %), NAB1 s1 1.80 (0.03 %), TAT1 s2 2.23
(0.15 %), MYG1 s2 2.44 (0.43 %); with UCRY's cries 3.42 / 2.42 / 2.07 / 2.55 / 2.52 (0.57 / 0.09 / 0.07 / 0.40 /
0.56 %). Parked at the spawn post the mix is under full scale on UTA1 / MYG1 (0.92-0.98) and NAB1's windowed
snapshot (null ALSA device) peaks 0.96 without cries, 1.02 for one frame with them; TAT1's post reads 2.35 with or
without. The worst ticks are pellet impacts: 14 `imp_weap_ord_exp` starts in one tick (a shotgun's pellets each
start the impact, card SNDX collapsed only the fire sound). The maintainer hears this as crackle (SNDX's "static").
