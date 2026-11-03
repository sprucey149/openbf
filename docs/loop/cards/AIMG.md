---
id: AIMG
title: No sim referee exercises the aim magnet: aim_demo fires 120 ticks with enemies=0 (magnet -1 every tick), so AIMF's acquisition moved no gate — add an aim replay with enemies in and out of the cone, a friend on the crosshair, and a wall
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AIMF (73632e1) rewrote the soldier acquisition; the gates showed sim/simhash/sweep/arena identical because no referee log puts a target in front of a player (aim_demo motion.csv: magnet column -1 on all 120 ticks, enemies 0). Only the unit tests in `core::sim::input` guard it.

## Do / Acceptance
A recorded or scripted headless aim replay (dummy rig: enemy just off the ray, friend on the crosshair, enemy past LockOnRange + r) whose motion.csv magnet column changes; hash it in sim_gates.sh (gpu_tests/sim_gates edits may be classifier-denied -> For the maintainer). Declares: sim (new baseline).
