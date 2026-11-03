---
id: FIDSP
title: Fidelity gate counts RLOOP's standing loops: a new metric loop20 (spells of 20 s+ inside a 2 m box in recover / travel, scripts/loop_spells.py over the botq samples the fidelity run already writes, summed per map, direction down) in gates.py's FID_BOTQ set, with its tolerances set from main's two seed blocks
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
RLOOP (landed 70bfa91 + a059815 + 70b9c47) took the standing loops from 117 to 27 spells of 20 s+ (16 seeds x 32 maps), the ones over 30 s 81 -> 2. Nothing in the gates watches that number: a later bot card could bring the two-minute stands back and pass every gate (`stuck_max_med` is a median of a different instrument). RLOOP's report asked the planner to wire `scripts/loop_spells.py` into the fidelity numbers; that is a gates.py change, so it is this card. Bots are the maintainer's top concern (root CLAUDE.md).

## Do
- gates.py: a fidelity metric `loop20` (count of loop spells per map, sum over seeds as `recycled` / `noprog` are, FID_SUM) and optionally `loop30`, computed by importing / running loop_spells.py over the fidelity run's botq dir; `dir = "down"`.
- The tolerance from main's own spread: run main's two seed blocks (1-8, 9-16) and three more 8-seed blocks; per-map tolerance = the spread, agg tolerance from the six block totals. Say in the card what the numbers were; the planner writes the baseline rows (the builder cannot edit gates.toml - give the rows in the report).

## Acceptance
- On main (today's): `gates.py --groups fidelity` lists `loop20` per map and agg, equal to `loop_spells.py` run by hand on the same botq dir (paste both).
- A mutation: RLOOP's `PIN_FAILS` set to 200 on a scratch tree -> the gate reports `loop20@agg` worse (paste the line).
- The six block totals and the tolerance chosen in the report.
## Cloud checkpoint 2026-10-24 (FIDSP build)
Uncommitted source and permanent fixture checks in branch fidsp at
`engine/artifacts/lead_tmp/worktrees/fidsp`; recovery patch/receipts/next prompt in `lead_tmp/fidsp/`.
Python fidelity/gate self-tests, seven boundary tests, hand-CLI fixture parity and sum mutation pass.
CI exit 101: missing common-bake gamefont_large and suitable graphics adapter.
Real current-main parity, PIN_FAILS=200 mutation, six block totals and measured tolerances remain owed;
no baseline rows proposed, no gates.toml changes, no commit/landing. See `reports/20261024_0946_card_FIDSP.md`.

## GitHub checkpoint recovery (2026-10-24)
