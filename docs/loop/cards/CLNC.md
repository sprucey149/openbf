---
id: CLNC
title: Cleanse core (2026-09-30): the core crate since the last core cleanse (68b5fbd), read hardest on the 1,657 lines since d748ac2 (STHOP ground rules, steep probe, flash/impact/laser); stale comments, dead code, ledger labels, CRATES counts, size waivers
milestone: M0
value: 2
phase: cleanup
status: done
deps:
files: engine/crates/core,engine/CRATES.md,scripts/size_waivers.txt,scripts/land.py,analysis/re-soldier-ground.md
declares:
needs:
effort:
branch: clnc
resume: landed de2541b8 2026-09-30
created: 2026-09-30
---
## Why
The cleanse rotation's `core` area (LAST_CLEANSE). Scope: core since the last core cleanse (68b5fbd, 2026-09-17), the critical read on the diff since d748ac2.
