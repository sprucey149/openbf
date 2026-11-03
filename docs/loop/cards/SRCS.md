---
id: SRCS
title: pack.py says what it found: the sources in iso/ per game (PS2 / Xbox / PC / mods) and which source each texture family came from, in its summary and a file beside the bake
status: archived
issue: 99
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #99](https://github.com/sprucey149/openbf/issues/99).

## Why
INBOX 2026-10-21 19:30: with XBTEX the bake mixes donors; the player should see what pack.py found and used. Aspyr's
Classic Collection gets a reader only when the maintainer owns it (not yet).

## Acceptance
- pack.py's summary on this machine lists the PS2, Xbox and PC sources per game, and a per-map table (textures from each
  source) is written beside the bake; a test over a fixture iso/ dir (one of each, one unknown file named as unknown).
