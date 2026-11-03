---
id: SSELB
title: The PS2 BF2 spawn screens drawn: side select (Select a team: the two teams' names + Auto Assign, the highlighted team's first class standing in the backdrop), spawn map (BMAP2's view + legend and objective lines), class select (the 3-D line-up, the panel: name, Unit Count, Equipment, the locked texts); backdrops = the mission's AddCameraShot views (baked) at the level start, the death view dimmed 0.51 at a respawn
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The spawn screens as the PS2's BF2 draws them (the maintainer 2026-10-19; `analysis/re-spawn-flow.md` §0-§4, §6, [O]). Today
`ui/src/spawn.rs` is an egui panel over the battle (a post list and a class row, dimmed by the panel). The PS2: three
full screens over a 3-D backdrop - the side select, the spawn map, the class select with a 3-D line-up - see
`engine/artifacts/lead_tmp/ssel/ps2_flow_sheet.png` (BF2 Hoth, every screen, captured live).

## Evidence (`analysis/re-spawn-flow.md`)
- **Side select** (§2): the Lua screen "Select a team" (`ifs.sideselect.chooseteam`), a vertical list: team 1's name,
  team 2's name (the team objects' names: "Empire" / "Rebels"), "Auto Assign" (`ifs.sideselect.autoassign`); black +
  dimmed backdrop flags; no Back help text. The HIGHLIGHTED team's first class stands in the camera shot as a 3-D soldier
  (none on Auto Assign).
- **Spawn map** (§3): the map element is card BMAP2's Spawn view; top right the mission objective ("CAPTURE ALL THE
  COMMAND POSTS"); bottom left three legend lines: post icon "COMMAND POST", D-pad "CHANGE COMMAND POST", X "SELECT
  COMMAND POST"; no Back line.
- **Class select** (§4): every class of the team as a 3-D soldier in a line-up in front of the camera shot, the selected
  one in front (a carousel slide); the panel (`FUN_00396430`): title = the class name; "Unit Count: N" or "Unit Count: N
  Max: M"; "Equipment:" and one line per weapon; a refused class "Get N more points to unlock!" / "Too many units
  already!"; help "Back" (Triangle) left, "Select Character" (X) right. Left / Right cycle with wrap (Left = next).
- **Backdrops** (§1.1): at the level start each screen change cuts to another of the mission's `AddCameraShot` views
  (48 deg horizontal, tan_v 0.34131), picked by `FUN_001b9d00` (n < 3: next; else a two-step LCG draw, never the
  current one). Hoth's eyes at side / map / class = hot1g_con's shots 0, 1, 2 exactly (`lead_tmp/ssel/t1_hot1`). At a
  respawn the backdrop is the death camera's view (card SSELC), dimmed to ~0.51 of its brightness within 0.3 s (§6.3).
- `AddCameraShot` is not baked today (`scripts/gen_missions.py` drops it); every BF2 conquest mission calls it 3-22
  times (first 16 kept), BF1's 44 of 44 mission files too.

## Do
- Bake: `gen_missions.py` keeps the `AddCameraShot` calls (quaternion x, y, z, w + position; first 16) in the mission RON;
  `mission.rs` reads them.
- Camera: on the side / map / class screens at the level start the view is the current shot (48 deg); the shot pick of
  `FUN_001b9d00` on each screen change (PROVISIONAL until a test pins the draw against a PS2 row).
- Screens: the three layouts in the BF2 HUD style (the port's HUD fonts and widgets), text from the disc's locl keys;
  the side preview and the class line-up from the class models already loaded for the spawn screen (`WalkSim`'s class
  models). The line-up's slide timing and model offsets are NOT measured (re-spawn-flow §11): measure them from a PCSX2
  strip first (`lead_tmp/ssel/ssel_live.py`, a class-select run with Left / Right) and pin them in the card.
- Respawn backdrop: the death camera's view (SSELC), dimmed to 0.51.

## Acceptance (each RUN and pasted)
1. A bake test: hot1's mission RON carries hot1g_con's AddCameraShot list (count and the first shot's numbers).
2. A pose check: the port's eye at the side select on Hoth equals shot 0 and the PS2's measured eye in
   `lead_tmp/ssel/t1_hot1/s0_team1` (pose.py) within 0.05 m; FOV tan_v 0.34131.
3. A unit test of the shot draw (`FUN_001b9d00`): n < 3 steps by one; n >= 3 never returns the current shot.
4. Goldens of each screen (side, map, class, the respawn map over the dimmed death view), VIEWED beside
   `ps2_flow_sheet.png` before land.py commits them.

## Notes
- deps: SSELA (the stages the screens show). BMAP2 owns the map element's Spawn view and its PostSelect ring; this card
  draws the rest of the screen around it. UISN owns the sounds; EVMSG the "PLAYER 1 DIED" line.
- PLAN's shell is BF2's: BF1 maps get the same flow (re-spawn-flow §8, a reversible call).
- 2026-10-20 (card BSHL's RE, `analysis/re-bf2-shell.md` §2): the shell's LAUNCH ends in `ifs_instant_side`, which stops
  the shell movie, sets the two team names and enters the mission; the mission's side select is this card's. The BF2
  loading screen between them is LDBF2.
- 2026-10-20 (the maintainer): every menu and screen takes the left stick AND the D-pad, the same way. The shared input is in: `input::stick_arms`, ORed into `windowed.rs` `menu_edges` (the shell, the pause menu, the screens built on `MenuInput`) and into the sim's SPAWN_POST_* bits. New screens read `menu_edges`, never a button directly; a test per screen that stick and D-pad give the same move.
