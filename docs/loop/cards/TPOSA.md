---
id: TPOSA
title: Positions slice 1, split from TPOS to land alone: the position table built at map load (ai/tactic.rs: Cover / Snipe hints, barrier-edge points, hub points; LOW / CORNER cover over 16 directions; an 8 m grid) - DERIVED, not hashed, nothing reads it yet (TPOS's Tactician, TURR's seats, SNPR's snipe hints build on it)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/tactic.rs, engine/crates/core/src/ai.rs, engine/crates/core/src/sim/state.rs, engine/crates/game/src/walk.rs, engine/crates/game/examples/round_to_outcome.rs, engine/crates/game/tests/nav_walls.rs, engine/CRATES.md
declares:
needs:
effort:
branch: tposa
resume: landed 85834ec8 2026-10-16; independent audit INCOMPLETE / UNAUDITED 2026-10-25; restore held primary inputs and verified history before retry; report reports/20261025_2207_audit_TPOSA.md
domain: ai
created: 2026-10-16
alpha: yes
owner: TPOS 8632
---
## Why
Card TPOS (value 5) is bigger than one session: its Tactician (slice 2, branch tpos c22d913) is not at its bars yet (B12 1.0-6.4 % against 20 %), while its item 1 - the table at map load - is done, behaviour-neutral and wanted on main by TPOS, TURR (seat positions) and SNPR (snipe hints). Split 2026-10-16 by the TPOS session; commit 1fcf7f6 cherry-picked.

## Notes
- TPOS keeps the rest (the Tactician, reservations, the cycle, Capture / Guard / Overwatch, the deletions) and rebases onto this.
