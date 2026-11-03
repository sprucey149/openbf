---
id: MUSV
title: The battle's music proven in the window, and a music volume: the windowed music frame runs without a sound device so win_run.sh logs the same 'music ... start' lines as the headless round (AMBM's windowed wiring is exercised by no check), and one music gain in the pause menu's options (the PS2's ingamemusic bus)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
AMBM (1bde70c) gave the battle its music, and no check has run the path the maintainer will use. The rules, the records, the
timing and the decoder are tested, and a headless round logs the changes (`MUSIC_LOG`), but `main/windowed.rs` starts
a take only behind an open sound device, and the windowed game under Xvfb opens none (`win_run.sh` runs it with an
empty ALSA config; `Audio::new()` returns `None`). So if the windowed wiring is wrong, the first to find out is the maintainer,
by hearing nothing. Second: the music has no level of its own. The PS2 has an `ingamemusic` bus beside the effects.
Filed by the planner from INBOX 2026-10-03 07:10 (report `reports/20261003_0710_card_AMBM.md`, "Not checked").

## Notes
- Mutation-check the windowed check once: break the windowed start (skip the manager's update) and see the log lose
  its lines.
- UISN (interface sounds) also edits `main/windowed.rs`; the picker will not run the two side by side.
## Landed 2026-10-06 (9f2aa09) - report `reports/20261006_1200_card_MUSV.md`
- Window log = headless log in Musics, takes and order (BF1 Naboo, seed 1); volume 0 (env and profile) prints no line; mutation run prints none.
- NOT met as written: the 0.1 s time match (window 16.3 s, headless 12.6 s for `middle`) - the window's battle has an idle human in a slot, so its reinforcements fall on other ticks.
- Item 3 (BF1 state rule live on the PS2) not done; `state_bf1` stays PROVISIONAL and no open card holds it.
