---
id: FOFG
title: The sweep's off-graph share (fidelity offgraph_pct) counts every alive unit where it sits, a turret gunner at its seat and a pilot in its vehicle: count units on foot only and print the seated share beside it - card TURR's KAM1 +8.65 was 7.28 seated gunners
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The fidelity gate's `offgraph_pct` (scripts/gates.py: the `N` locus share of the sweep's `shares` line, written by
`Shares::observe` in `game/examples/round_to_outcome.rs`) samples every alive unit once a second at its position, riders at
their seat. It was built (cards RCYC / OFFG) to catch soldiers who walk off the nav graph. A seated gunner on a building
turret, or a pilot driving across open ground, is not that - but it counts. Card TURR (bots man turrets, branch `turr`) was
refused on it: offgraph_pct@bf2_kam1 4.41 -> 13.06 and the all-map value 6.25 -> 8.21, of which 7.28 points on KAM1 are
gunners in their seats (`lead_tmp/turr/offg`, report `reports/20261016_0621_card_TURR.md`). Any vehicle or turret card
meets the same wall. Planner call 2026-10-16b (reversible): the share measures units ON FOOT; the seated share is printed
beside it so nothing is hidden.

## Evidence
- `lead_tmp/turr/offg/`: TURR's scratch foot-only counter (bin `rto_foot2` = branch + counter); its all-units value
  reproduces the gate's 13.06 exactly; seated 7.28, on foot 5.78 (boarding 0.88, within 12 m of a turret 1.50, elsewhere 3.40).
- gates.toml `[fidelity.metrics.offgraph_pct]` tol_map_abs 8.5, tol_agg_abs 0.6, dir down.

## Notes
Unblocks TURR (deps). Not oracle-guarded: round_to_outcome.rs is the sweep binary, not gates.py.
