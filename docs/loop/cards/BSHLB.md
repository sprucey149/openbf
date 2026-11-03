---
id: BSHLB
title: The BF2 PS2 Instant Action screen and Session Options (card BSHL's RE): the iface_instant_action backdrop (top 448 rows), Planet / Mode / Era lists, the LAUNCH / ADD / REMOVE / OPTIONS stack, the info box and the Session Options stacks, with the port's maps, modes and era pairings - replacing the egui setup list
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BSHL's second screen: the PS2 BF2 Instant Action screen (`ifs_missionselect`) and Session Options replace today's
egui setup list (`ui/src/shell.rs`, nine rows). `analysis/re-bf2-shell.md` §5, §9.6.

## What
- Bake (`rebake=common`): SHELL.LVL's `iface_instant_action` (512x512; its TOP 448 rows fill the 512x448 screen,
  uv v 0..0.875 - §3.4, measured on the frame), the mode icons if used, COMMON.LVL's window / list pieces the lists
  use (`NewButtonWindow`: title bar + `border_3_pieces` style - read `ifelem_buttonwindow` for the piece names).
- The screen: no movie; Planet -> Mode -> Era lists in the left window, the Playlist on the right, the command stack
  LAUNCH / ADD / REMOVE / OPTIONS in the centre housing (`gamefont_tiny`, square buttons, `yTop` -130, width cap 80),
  the info box along the bottom (`gamefont_tiny`, focused-text blue 138,160,244). Lists `safeW x 0.375 - 35` wide,
  rows `font height + 2`, rows shown `clamp(floor((safeH - 240) / (row + spacing)), 4, 16)` (9 on the PS2), the Era
  list half (§5, `ifs_missionselect` main.f34). Focus starts on ADD.
- Content (the port's features, §9.6): the port's maps in the Planet list by their names (BF1 and BF2), the modes the
  port runs, the two eras plus the cross-era pairing as an Era entry that opens a faction-pair list; ERAM's notes
  (army rows "Your army / Enemy"; the mode label shows the mission really loaded).
- OPTIONS -> Session Options stack (Global, Conquest; the others hidden until the port runs those modes): Global =
  AI Units Per Team, AI Difficulty Level; Conquest = AI Units Per Team, Reinforcements, Timer - the `ShellModel`
  values the port already has. LAUNCH starts the playlist's first entry with them.

## Acceptance (each RUN and pasted)
1. Pixel geometry against `ref_frames/ia_planet_list.png` / `ia_playlist_launch.png` at 640x480: the backdrop panel's
   top and bottom edges at y 64.3 / 413.6 px (+/- 1), the Planet and Playlist windows' outer rects within 2 px of the
   PS2's (measure them first and paste the numbers), 9 planet rows visible.
2. A scripted input test (headless): ADD -> Hoth -> Conquest -> an era -> the playlist holds Hoth -> LAUNCH returns the
   action that starts hot1 conquest with the Session Options' AI count, reinforcements and timer.
3. The ERAM notes: a test that picking Empire first makes "Your army" the Empire.
4. Goldens of the IA screen and Session Options VIEWED beside the PCSX2 frames before land.py commits them.

## Notes
- deps BSHLA (the stack widget, the bake path, the shell state machine).
