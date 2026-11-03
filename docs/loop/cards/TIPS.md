---
id: TIPS
title: BF2's tooltip bar (the bottom hint line: 'This CP is now controlled by your team.', 'Stand near an enemy CP to capture it.', enter/exit vehicle, build a turret ...): the table at 0x5ff4ac, each tip's trigger, timing and repeat rule
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
EVMSG's RE (`analysis/re-event-messages.md` §1, 2026-09-30): the only text BF2 shows when the player's team takes the
post he stands on is a TOOLTIP — "This CP is now controlled by your team." (Locl 0x64bcc6d2) in the bottom bar
(`player1tooltipsGroup` / `player1tooltipsTEXT`: MultiLineText, NumLines 3, DisplayTime default 2, frame mesh
`hud_tooltips_frame`, fade 0.25 / 0.5; events `player%d.tooltips` / `.disable`). The port shows no tooltip.

## Leads
- The table of 0x20-byte records at 0x5ff4ac ff. in SWBFRNT2.ELF: a Locl hash + six floats / flags per tip ("Activate
  boost", "Turrets can be built here with a fusion cutter.", "Stand near an enemy CP to capture it.", "This CP is now
  controlled by your team.", "Enter/Exit vehicle: {btnb}", "Enter/Exit turret: {btnb}", "Take off / land: {btna}" …) —
  dump it whole (`lead_tmp/re/evmsg/keys.py` resolves hashes), find its reader and each tip's trigger.
- BF1's twin: `game.tooltips.*` key strings (approach_cp "THIS IS A COMMAND POST", approach_vehicle, after_enter_vehicle,
  approach_turret, approach_unbuilt, approach_health_ammo, reinforcements, flyertakeoff, disablemsg "DISABLE TOOL TIPS IN
  OPTIONS"), `FUN_002db4c8`, `ScriptCB_NextToolTipState`.
- PS2 frames: `engine/artifacts/pcsx2/ref/evmsg/bf2_hot1_cp_tooltip_xshot.png`, `bf2_hot1_kill_line.png` (bottom bar).
- An options switch turns tips off on the PS2 — the port needs the same switch (FEEL_NOTES).

## Acceptance
- RE: every tip of the table with its trigger, show time, repeat rule and text ([V]/[O], verifier run).
- Built (after BHUD2 + EVMSG's MultiLineText): a scripted scene per tip kind in which the port shows the PS2's text in the
  bottom bar, onset within ±0.5 s of the trigger and the PS2's duration ±2 frames — a unit test per rule; a PS2 versus
  port pair of the bar within ±4 px (640×480).

## Notes
- 2026-10-20 (card PWST, `analysis/re-powerup-station.md` §5.2): the table's real base is 0x5ff3d0 (32 entries x 0x20,
  built by `FUN_00393fd0`; the text hash at +0x1c, so 0x5ff4ac above is entry 6's hash). Per entry (2.0, 0, 8.0, 40.0,
  5.0, 2.0, bytes 4, 0x80) [H: delay / show / cool-down / a 4-shows cap read by `FUN_00394500`]; `FUN_003943b0(mgr, i,
  player)` arms tip i only offline and while mgr+0x39e is set. Station tips: BF2 15 "This droid will heal you when you
  stand near it.", 16 "Stand near this droid to refresh ammo.", 17 "R5 droids will repair nearby vehicles." (armed by the
  station update for the local player inside the sphere, served or not); 5 "{useButton} change soldier class" (the
  soldier update's class-change bit, `FUN_00248c38` / `FUN_003b4668`). BF1: tooltip manager `DAT_0056449c`, key table
  `FUN_002db988` (case 10 `approach_health_ammo` "THIS IS A RE-SUPPLY DROID" when the player is served; 7 / 8 / 9
  `approach_unbuilt` / `_ammo` / `_health` for a broken station under the reticle, `FUN_002d4b50`).
