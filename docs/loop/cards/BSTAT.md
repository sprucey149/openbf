---
id: BSTAT
title: Bots' battle stats: RE what the PS2 keeps per bot during a round (kills, deaths, points, captures) for the end-of-round stats screens, and whether it is thrown away after the screens; build the port's per-bot tallies to match
status: archived
issue: 50
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #50](https://github.com/sprucey149/openbf/issues/50).

## Why
The maintainer 2026-10-28: "We also need to track the stats of bots, because we have the stats screen now ... Check how the originals track bot stats though, I think it all just gets thrown away after the stats screens are finished being looked at." the maintainer is building the post-battle stats screens (issue #50).

## Evidence
- Read first: `re-round-end.md`, `re-spawn-flow.md` (member records, stride 0x1a4, `DAT_00661330`; stats at member+0x1fc per §4.3), `re-event-messages.md`.
- The port today: `Unit::kills` / `deaths` per unit slot; the career counts only the human (game/src/walk.rs `track_class_kills`).

## Acceptance
- RE note: per-member stats fields, who writes them (kill, death, capture, points), what the stats screens read, and when they are cleared.
- Built: per-bot tallies the stats screens read, cleared as the PS2 clears them.
