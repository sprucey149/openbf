---
id: GRFG
title: The ordnance census as a gate: gates.py runs ordnance_census with the leak group and compares its footer with a gates.toml [ordnance] row (new 0) - the builder was refused by the oracle guard, the planner commits it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card GRFL landed the grenade fix, its test and the census (game/examples/ordnance_census.rs + ordnance_census_accepted.txt) on 296fa0c, but not the gate: the oracle guard refuses a builder's edit of the gate script and of the baseline file. Without the gate the 7,260 / new 0 footer is watched by nobody.

## Evidence
- engine/artifacts/lead_tmp/grfl/build/GATE_PROPOSAL.md: the g_ordnance function (never executed) and the [ordnance] row.
- Measured on 296fa0c's tree, three runs byte-identical, 30 s on 16 threads under the 8 G cap: ordnance: maps 32  floors 389228  gaps 6500  runs 1556912  through 7260  accepted 7260  new 0. The parent's ray through the same binary: through 127017 accepted 13407 new 113610.

## Note (cleanse 14, 2026-10-09) - land.py's fidelity re-baseline is wider than the declaration
For the planner, with this card's gate wiring (same files, same channel). `scripts/land.py` `rebaseline()` reads the card's
`fidelity=` tokens and calls `gates.fidelity_rebaseline(only=...)` with `only = None` when any token is `all` or starts with
`@`, else the set of metric names. So `fidelity=still_pct@bf1_kam1` rewrites still_pct's baseline on all 32 maps and the
aggregate, and `fidelity=@bf1_bes1` rewrites EVERY metric on every map - including each value that got worse inside its
tolerance on maps the card never named. Two such landings can each lose 0.9 of a tolerance and no gate row ever says so.
Read in the code (land.py:519-529, gates.py `fidelity_rebaseline`), not yet seen in a landing: no declaration is readable
until STATUS question 33 is settled. Proposed: re-baseline only the declared (metric, map) values plus the ones that got
better; add the case to `gates.py --selftest-fidelity`.

## Planner 2026-10-12 11:10
- **value 3 -> 4.** the maintainer approved this card by name on 2026-10-11 and `[approvals.GRFG] oracle = "baselines,guards"` stays open in gates.toml until it lands (the scoreboard's debt list); at 3 it would not be picked for weeks. One session. When it is done the planner removes the approval block. Undo: `loopctl.py set GRFG value=3`.

- Planner 2026-10-12 19:35: value 4 -> 5. The maintainer's approval of 2026-10-11 has stood unused for 10 landings (BSPN ...
  CLSE4) - the debt rule sends that to STATUS (question 41). At 5 the dry run gives it to lane 2 after TGAP.
  Undo: `loopctl.py set GRFG value=4`.

## Outcome (2026-10-16 03:09, lane 1, LANDED d21b411)
g_ordnance in gates.py (runs with the leak group, FAIL unless the footer ends `new 0`), gates.toml [ordnance] footer from main d1def96. Acceptance: branch PASS (new 0); main with GRFL's fix reverted FAIL (new 112300); land.undeclared dry run PASS -> ignored, FAIL -> refuses. Planner: remove [approvals.GRFG]. Report reports/20261016_0309_card_GRFG.md.

- 2026-10-22 06:50 (card TSLG, the planner's 10-09 ask): `ordnance_census` ORDN_AT on main 9193a4c at the maintainer's FIRST Kamino deck (-125.6, 70.2, 151.0): floors 1, gaps 0, runs 4, **through 0** (body 393 `kam_bldg_aaa_bridge5`, a mesh with the soldier and ordnance layers); his second deck (103.1, 52.4, 75.0): through 0 (body 352 `kam_bldg_aaa_bridge1`).
