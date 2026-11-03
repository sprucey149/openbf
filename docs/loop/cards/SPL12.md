---
id: SPL12
title: Split: core/src/sim/step.rs (1,767 lines, 990 of them its inline tests from line 775) - move the test module to core/src/sim/step/tests.rs; then core/src/sim/spawn.rs (1,561)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Cleanse 14 (2026-10-09): both files are over the 1,500-line cap with neither a split card nor a line in scripts/size_waivers.txt. step.rs grew 1,385 -> 1,767 through card ZOOM (its tests). engine/artifacts/briefs/SPLIT_RECIPE.md: one file, one session, a pure move.

## Acceptance
- `scripts/size_report.sh`: neither file in the over-1,500 list; `--check-map` exit 0 with the new files in engine/CRATES.md.
- Every gate PASS, nothing declared; the sim hashes and the test count (gates.toml [ci] passed) unchanged.
