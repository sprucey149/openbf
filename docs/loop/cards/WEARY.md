---
id: WEARY
title: SSD wear: fewer, smaller test binaries (63 integration-test files under engine/crates/*/tests/ each link the whole engine into their own binary; game has 37) and stale lane targets
milestone: M3
value: 4
phase: build
status: done
deps:
files: engine/crates/game/tests, engine/crates/render/tests, engine/crates/bake/tests, engine/crates/mungefs/tests, engine/CRATES.md, scripts/loopctl.py
declares: none (behaviour-neutral: test files moved into one binary per crate; loopctl's target sweep)
needs:
effort:
branch: weary
resume:
created: 2026-10-15
first: 2
---
## Why
The maintainer, 2026-10-15: SSD wear is a big deal (see card WEARX for the numbers: ~78 GB/h written under the loop, cargo output nearly all of it). Every file under crates/*/tests/*.rs is its own test binary that links the crate and every dependency: game 37, bake 9, render 10, mungefs 6, core 1. debug/deps is 71 GB of the 121 GB debug target. Already done (46c6555): line-tables-only debug info, dev incremental off - which shrinks each binary; this card shrinks the count.

## Merged: WEART (planner 2026-10-16 11:09: the same work - WEART was filed by WEARC's session a day after this card)
Its measurements and instrument are the starting point here; its gates.py clause stands: if the golden_* test binaries move, the
gates.py edit (GOLDEN_TESTS / core_link_check) needs `[approvals.WEARY] oracle = "guards"` from the maintainer - otherwise leave the golden_*
files where they are. `scripts/gates.py` is NOT in this card's files (it would hold WEARX, first 1, out).

### WEART Why
Card WEARC (landed eb331e7, 2026-10-16) stopped ci linking examples: one warm ci.sh after a core edit 1.76 -> 1.14 GB written (cgroup io.stat, report reports/20261016_0014_card_WEARC.md). What remains is mostly test-binary links: 77 test executables, 5.92 GB apparent in debug/deps after one ci run (largest: game lib tests 153 MB, golden_hud 132 MB, golden_spawn 131 MB, skinned_glb 130 MB, battle_replay 130 MB).
### WEART Do
1. Measure on today's main with WEARC's instrument (lead_tmp/wearc/measure.sh: a systemd-run scope, io.stat wbytes after sync -f).
2. Fold game/tests/*.rs into one integration binary (tests/all/main.rs with mod lines, the common Rust pattern) or a few grouped ones; keep the golden tests in binaries gates.py finds by name (gates.py GOLDEN_TESTS globs debug/deps/<name>-*): if they move, the gates.py edit needs the maintainer's [approvals.WEART] oracle = guards - otherwise leave the golden_* files as they are.
### WEART Acceptance
- ci.sh: the same pass / fail / skipped counts as main (1553 / 0 / 46 on eb331e7), pasted.
- Bytes written per warm ci.sh run (after a core edit) before and after, and the debug/deps executable total, in the report.
- gates.py --auto on the branch: gpu (goldens) PASS - it still finds every golden test binary.
