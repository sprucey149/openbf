---
id: PPRS
title: Per-player pressure: in split screen and LAN each player picks their own level of how hard the enemy's bots press THEM (how many may fire at them at once, the human-target preference), separate from the session's difficulty
milestone: M3
value: 3
phase: build
status: todo
deps: TCMP
files:
declares:
needs: maintainer - later scope, PLAN.md: Instant Action only for now
effort:
branch:
resume:
created: 2026-10-05
---

## Why
The maintainer, 2026-10-05, on Claude's suggestion that friends of different skill each pick their own: "per player pressure in split screen or lan multiplayer is a good idea".

## Evidence
- PLAN's layer 2 (per-human pressure): the human-target preference x1.0 / 1.0 / 1.3 / 1.6 by tier, and BOTD section 7's "bots that may fire at one human at once" 2 / 4 / no limit.

## Acceptance
- Each player has a pressure setting of their own (saved per profile, default = the session's tier) that sets those two things for bots targeting THAT player; the session's difficulty sets everything else.
- A sim test with two humans at different settings: the bots' fire at each follows its own setting.

## Notes
Filed 2026-10-05 by Claude in chat from a design talk with the maintainer. His words are quoted; no number here is measured.
