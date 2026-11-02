---
id: BSHLA
title: The BF2 PS2 shell's title and stack menus (card BSHL's RE): bake the shell art, the movie's menu loops and the shell music; Press START, Main Menu (Singleplayer, Multiplayer dimmed + skipped by focus, Splitscreen dimmed, Options), Singleplayer and Options as the PS2's vertical-button stacks over the shell_main / shell_sub_left movie, pixel-matched to the PCSX2 frames
milestone: M5
value: 5
phase: build
status: done
deps: BSHL
files: engine/crates/bake/src/main.rs, engine/crates/bake/src/lib.rs, engine/crates/bake/src/shell.rs, engine/crates/ui/src/shell.rs, engine/crates/ui/src/menu.rs, engine/crates/game/src/assets.rs, engine/crates/game/src/main/windowed.rs, scripts/rebake.sh
declares: goldens=shell_title.png,shell_main.png,shell_singleplayer.png,shell_options.png; rebake=common
needs:
effort:
branch: bshla
resume: landed f5c94429 2026-10-21
created: 2026-10-20
first: 6
---
## Why
The maintainer (INBOX 2026-10-19 (a), BSHL `first: 6`): the BF2 PS2 shell is the community's top ask; today's shell
(`goldens/shell.png`) is one egui list. `analysis/re-bf2-shell.md` (BSHL) has the PS2's screens, flow, palette, stack
geometry and movie. This card builds the front of it: Press START, Main Menu, Singleplayer, Options. The Instant Action
screen is BSHLB; the BF2 loading screen LDBF2.

## What (re-bf2-shell §2-§4, §9.1-5, 7)
- Bake (new, `rebake=common`): SHELL.LVL's `logo`; COMMON.LVL's `bf2_buttons_*` (8 pieces), `btna`, `btnb`,
  `opaque_black`; the movie's menu loops (SHELL.MVS segments 0-1 = `shell_main`, 2-9 = `shell_sub_left` in the mcfg
  order 2,3,4,5,8,6,7,9; no audio in them) and the stream `shell_music` / `shell_soundtrack` of SOUND/SHELL.LVL.
  Engineering is ours: the movie decoder (an MPEG-2 decoder at run time, or a bake-time transcode to a frame pack the
  game decodes on a worker thread) - record the call; if it needs an external tool at bake time, say so for PACK.
- Screens, each a PS2 vertical-button stack (§3: `interface_util`'s PS2 palette and pads, `gamefont_large`, rows
  font height + 6 with gutter 4, title bar pieces / mid pieces / bottom pieces, wrap-around links, the 0.1 s per row
  reveal; the help line Back (Triangle) / Select (Cross) 15 units above the safe bottom):
  - Title: `logo` at the top (y -38), `shell_main`, "Press START Button To Begin" pulsing alpha 0.4..1 at 0.5/s;
    Start (pad) or Enter (keyboard) advances. Legal / logo screens and the profile flow: skipped (§9.1, §9.3).
  - Main Menu (logo + stack, container at 0.6 of the height + 10): Singleplayer; **Multiplayer in its PS2 slot,
    `enabled: false` in data, drawn dimmed (label grey 110), skipped by focus** (§3.3 - the PS2's own disabled style;
    FEEL_NOTES line); Splitscreen dimmed (until SSCR); Options. Down from Singleplayer lands on Options.
  - Singleplayer (left-justified, `shell_sub_left`): Instant Action live -> today's IA setup until BSHLB lands; Rise of
    the Empire and Galactic Conquest dimmed; Training and Space Overview hidden.
  - Options (left-justified, `shell_sub_left`): the port's pages behind the PS2 stack (Game / Audio / Controls /
    Video); Online, View Trailer, Credits hidden.
- Widescreen: the movie pillarboxed (4:3, letterboxed art inside the 448 lines), the stacks at the centre (Hor+), text
  and pieces scaled by display height / 448.

## Notes
- Sounds: UISN owns the clicks; the template's defaults are accept `shell_menu_accept`, back `shell_menu_cancel`, exit
  `shell_menu_exit`, focus move `shell_select_change` (re-bf2-shell §3.2).
- ERAM's two notes (BSHL card) go to BSHLB (the IA screen).
- The attract movie (30 s idle on the title, segments 32-33) is optional here; build it if the decoder makes it free.
- 2026-10-20 (the maintainer): every menu and screen takes the left stick AND the D-pad, the same way. The shared input is in: `input::stick_arms`, ORed into `windowed.rs` `menu_edges` (the shell, the pause menu, the screens built on `MenuInput`) and into the sim's SPAWN_POST_* bits. New screens read `menu_edges`, never a button directly; a test per screen that stick and D-pad give the same move.
