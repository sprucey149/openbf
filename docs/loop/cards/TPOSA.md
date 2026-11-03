---
id: TPOSA
title: Positions slice 1, split from TPOS to land alone: the position table built at map load (ai/tactic.rs: Cover / Snipe hints, barrier-edge points, hub points; LOW / CORNER cover over 16 directions; an 8 m grid) - DERIVED, not hashed, nothing reads it yet (TPOS's Tactician, TURR's seats, SNPR's snipe hints build on it)
status: archived
issue: 11
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #11](https://github.com/sprucey149/openbf/issues/11).

## Why
Card TPOS (value 5) is bigger than one session: its Tactician (slice 2, branch tpos c22d913) is not at its bars yet (B12 1.0-6.4 % against 20 %), while its item 1 - the table at map load - is done, behaviour-neutral and wanted on main by TPOS, TURR (seat positions) and SNPR (snipe hints). Split 2026-10-16 by the TPOS session; commit 1fcf7f6 cherry-picked.

## Notes
- TPOS keeps the rest (the Tactician, reservations, the cycle, Capture / Guard / Overwatch, the deletions) and rebases onto this.
