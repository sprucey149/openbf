---
id: CLNT
title: Cleanse scripts (2026-09-30): the loop tooling changed since CLNS (90c5dc9) — land.py/gates.py detach+resume (LDET), loopctl (LRUL/LDOC/LANE3), parity.py (PARITY); stale docs, script bugs, disk hygiene, CRATES counts
milestone: M0
value: 2
phase: cleanup
status: done
deps:
files: scripts/land.py,scripts/gates.py,scripts/loopctl.py,scripts/parity.py,engine/CRATES.md
declares:
needs:
branch: clnt
resume: landed d748ac2c 2026-09-30
created: 2026-09-30
---
## Why
The cleanse rotation's `scripts/artifacts tooling` area (LAST_CLEANSE), scoped to the 1,610 lines changed in scripts/ since the last scripts cleanse CLNS (90c5dc9).
