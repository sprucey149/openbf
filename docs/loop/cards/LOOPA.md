---
id: LOOPA
title: gates.py / land.py stop at once with ADAPTER LOST when the GPU the goldens were rendered on is absent (split from LOOPT: the oracle guard refuses a builder's edit to gates.py / land.py without [approvals.LOOPA] oracle = "guards")
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
LOOPT part 2. 2026-10-15 18:26 the NVIDIA GPU fell off the bus; wgpu took the Intel iGPU and SKIPC's and VRIDE's gates moved 89 goldens each (33 declared + 56 the diff cannot reach); each session spent turns proving it was the adapter. LOOPT's build session (2026-10-15 20:00) was refused by the oracle guard on its first edit to scripts/gates.py ("guards" class: gates.py, land.py, oracle_guard.py). Not worked around.

## Needs
`[approvals.LOOPA]` `since = "2026-10-15"` `oracle = "guards"` in engine/artifacts/loop/gates.toml (the planner or the maintainer adds it). Then this is a ~1 hour build.
