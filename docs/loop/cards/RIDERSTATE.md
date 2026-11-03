---
id: RIDERSTATE
title: Vehicle rider lifecycle consistency under death, exit and slot reuse
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Vehicle occupancy and unit handles must remain internally consistent through lifecycle transitions. Existing synthetic simulation fixtures make these invariants executable without interpreting new original-game behavior.

## Acceptance
Inspect existing rider lifecycle code and tests. Exercise death while seated, explicit exit, vehicle removal, and unit/vehicle slot reuse using the existing fixture helpers. Verify stale handles never identify replacement entities and seat/unit ownership agrees after the supported transitions. Derive expected behavior from existing documented engine invariants, not guessed PS2 constants.

If an actual defect is found, preserve a minimal failing-before regression, fix its cause and show that regression and relevant core tests pass. Do not add tests that merely repeat implementation or invent a bug. If no defect or meaningful coverage gap is found, record the finite audit once and close this audit scope with evidence; do not keep reopening it. Existing content-dependent gameplay acceptance remains separate and unearned.
