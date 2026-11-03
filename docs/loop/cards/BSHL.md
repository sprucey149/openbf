---
id: BSHL
title: The BF2 shell and menus' real look (PLAN §Presentation): today's shell (goldens/shell.png) is a working placeholder — BF2's shell art, flow and fonts, content adapted to the port's features
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
(2026-10-19, the maintainer, first: 6, no longer post-alpha: the community's top ask) The PS2 BF2 shell, captured from the PS2 DISC and PCSX2 - the PS2 front-end menus differ from the PC game's, so PC-derived layouts are leads only. The in-game unit-select screen (rotating trooper line-up) is probably close to the same on both and is SSEL's. Menu art, fonts, screen order, layout constants: read the disc's own compiled shell scripts (BF2's SHELL.LVL / SHELLPS2.LVL) first, then capture each screen in PCSX2 and match it pixel-measured (RULES: visual claims are pixel-measured).

The maintainer (INBOX 2026-09-28 (d)); PLAN §Presentation: BF2's shell and menus. Today's shell (`engine/goldens/shell.png`) is a
working placeholder. The content may change to fit the port's features; the look and flow stay BF2's.

## Notes
- Created 2026-09-28 by the planner.

## RE done (2026-10-20, lane 1, second session)
`analysis/re-bf2-shell.md` (767318c): flow, the one stack style (palette, pads, measured geometry), the disabled
button (grey 110, focus skips it - the port's Multiplayer takes it, so the 2026-09-30 note's choice is "focus skips
it"), the SHELL.MVS movie, the 512x448 screen, the IA screen, the BF2 loading screen (tips 1..54). Build cards:
BSHLA (value 5), BSHLB (value 4, deps BSHLA), LDBF2 (value 4). ERAM's two notes moved to BSHLB. The loading screen's
tips / order check (planner 10-19) is LDBF2's.
