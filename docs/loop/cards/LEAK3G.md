---
id: LEAK3G
title: The leak gate judges each map x class (LEAK3's gate half): leak_moved_rows.txt, a declared leak=<map>:<class> scope, a leak_rise FAIL for every other rise - draft written and tested, needs the guards approval to apply
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
LEAK3's AUDIT (reports/20260930_0930_audit_LEAK.md): a declared `leak` re-baselines every count and nothing lists which map x class rose (JWAL +36 counts, WPSH 11, WTUN 41 unreported). LEAK3 part 1 (census pins, `new` can fire) landed without this half because scripts/gates.py and land.py are oracle-guarded for a builder (the hook refused the edit, 2026-10-22).

## Evidence
Draft + test: engine/artifacts/lead_tmp/leak3/README.md and gate_draft/leak_judge.py (leak_counts / leak_allowed / leak_judge). On the audit's regression censuses (lead_tmp/audit-leak/census_r_*.txt) with leak=bf1_kas2:tunnel declared: cyl FAILS 32 maps, sphere 3 (bf2_fel1 bf2_uta1 tat2), box 32, tallbox 14; base vs base 0 rows; allow=all 0 undeclared.

## Acceptance
- gates.py's leak group writes leak_moved_rows.txt and prints the rises; a rise the card's `leak=<map>:<class>,...` (`*` either side, `all`) does not name is a leak_rise FAIL; bare `leak` = falls only.
- land.py parses `leak=<tokens>` and passes `--leak-allow`; its --self-test covers the parse and leak_judge on synthetic outputs.
- Re-run the four audit fixtures through the wired gates.py functions: the numbers above.

## Notes
needs the planner (or the maintainer) to add `[approvals.LEAK3G] oracle = "guards"` in gates.toml before a builder can take it.
