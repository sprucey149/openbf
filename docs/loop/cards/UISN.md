---
id: UISN
title: Interface sounds: the spawn screen's five SetSoundEffect slots (class change, post change, accept, back) and the pause menu's open / move / accept / back clicks, 2-D
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Every menu move in the port is silent. On the PS2 the spawn screen and the pause menu click on every move, accept
and back. RE: `analysis/re-ambience-music.md` section 6 (8a35a92).

## Evidence
- Spawn screen (the engine's state machine; slots set by the mission script's `SetSoundEffect`, the same five pairs
  in every script of both discs): class change (stick left / right beyond 0.8, re-armed under 0.2, or d-pad; not
  while the carousel slides) -> `shell_select_unit`; spawn point change (only when the selected post changed) ->
  `shell_select_change`; accept of the team, of a selected spawn point and of the class (the spawn) ->
  `shell_menu_enter`; back -> `shell_menu_exit` (BF1 from the unit and ready states; BF2 3 -> 2, 4 -> 5, 5 -> 3,
  and a refused class plays it too). BF1's team toggle also plays `shell_select_unit`.
- Pause menu (Lua): opening -> `shell_menu_accept`; every cursor move -> `shell_select_change` (auto-repeat 0.65 s,
  then 0.15 s); accept on any button -> `shell_menu_accept`; back (closes) -> `shell_menu_exit`; the yes / no
  popup: accept, then accept (yes) or `shell_menu_cancel` (no).
- All 2-D, bus `shellfx` (gain 0.4). `shell_menu_enter` inherits `shell_menu_accept` (pitch 1.1);
  `shell_select_unit` is a record of the map's SOUND LVL (pitch 0.3 +- 0.1, gain 0.3 BF1 / 0.4 BF2).

## Do
The mission RON carries the `SetSoundEffect` pairs (`scripts/gen_missions.py`); the spawn screen and the pause
menu emit UI sound events; 2-D playback through the CORE / map sound library.

## Acceptance
- A scripted input test on the spawn screen and the pause menu (headless): the counts of `shell_select_unit`,
  `shell_select_change`, `shell_menu_enter`, `shell_menu_accept`, `shell_menu_exit` starts in SOUND_LOG equal the
  script's class changes, post / cursor changes, accepts and backs; a spawn-point move that does not change the
  selected post makes no sound.
- The two zoom names (`binocularzoomin` / `binocularzoomout`) on scope in / out, if the port has the scope.
- Mutation check: map `SpawnDisplayBack` to the accept sound and see the test fail.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
- 2026-10-20 (card BSHL's RE, `analysis/re-bf2-shell.md` §3.2): the PS2 shell's own clicks for the menus BSHLA builds -
  the shell-screen template's defaults are accept `shell_menu_accept`, back `shell_menu_cancel`, exit `shell_menu_exit`,
  error `shell_menu_error`, enter none; every focus move in a button stack plays `shell_select_change` (also when the
  walk wraps back to the same button). One sound table for the shell and the pause menu.
