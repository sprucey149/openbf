---
id: DLCH
title: The Xbox DLC heroes (Asajj Ventress, Kit Fisto) from a DLC folder the player owns, in PS2 or PC format (a modded disc is refused) - local use only, never in git, using their supplied melee programs
status: archived
issue: 99
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #99](https://github.com/sprucey149/openbf/issues/99).

## Why
INBOX 2026-10-21 19:30. `SWBF2_mod.iso` holds them in PS2 format under `DATA/_LVL_PS2/addon/223/SIDE/dlc.lvl`;
`iso/ADDON_XBOX/DLC_XBOX/data/_LVL_PC/SIDE/dlc.lvl` holds a PC-format copy. Both are third-party repacks: local use only.
The maintainer: no melee combo needed. The DLC maps are BF1 maps the port already has. Waits on heroes existing (HERO).

## Acceptance
- Both heroes listed by the class reader from the PS2-format addon (class names, weapons, health), then playable on one
  map with HERO's rules; a test that the port runs unchanged when the addon is absent.

## Implemented sprint slice, 2026-11-01

Selective content detection and bake support both sources, importing only the
two heroes and their dependencies. The input scan now turns them on when their
folder is in `iso/`, and the pack and rebake carry them. On every BF2 mission that
fields a Republic or CIS hero they take that slot under the existing hero rules;
Alliance and Empire keep their own. The supplied melee programs reuse the existing
implementation; no bespoke combo system was added. Credits, player directions and
interfaces are in `scripts/DLC.md`.

Scoped headless checks pass: both classes and source health/weapons load,
offers/acceptance/health drain and actual melee kills work; absent-addon hashes
match the former path; PS2/PC physical setup and combat hashes match exactly.
Retail assets remain byte-identical. Their effects and Fisto's bubble sounds are
baked, and his tentacles sway. Parked for original-game/maintainer play
acceptance, not an implementation hold. The PC folder holds no hero voices, so
they have none. No broader hero issue is closed by this slice.
