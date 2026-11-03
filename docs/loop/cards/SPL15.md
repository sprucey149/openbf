---
id: SPL15
title: Split new size debt one file per session — ai/ride.rs 1729 and hud_tree.rs 1826
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / evidence
`scripts/size_report.sh --check-map` on bb974e5 lists ai/ride.rs at 1,729 and
hud_tree.rs at 1,826 lines, above the 1,500 cap. Neither has a SPL card or
waiver. The former combines claim/admission, pickup/wait and test wiring;
the latter combines element/fader state, tree interpretation and inline tests.
Their headers and existing card evidence retain the behavioral contracts.

## Scope
One file per session, each a pure move by `engine/artifacts/briefs/SPLIT_RECIPE.md`.
Inspect callers and existing boarding-card ownership before choosing a seam;
keep related helpers together where visibility widening would force reflow.
Preserve issuer/vehicle generations, claim priority, wait/call clocks, command
retention, allocation policy, hash bytes and every regression/referee.
No behavioral repair, test pruning, retuning or new abstraction in this card.
For HUD preserve operations, event/fade clocks, PS2 layout values, transforms,
mesh lookup and draw order. Retain BHUD/BMAP/CPBAR/EVMSG reference coverage.

## Acceptance
1. The original and each resulting source file are at most 1,500 lines, with
   coherent responsibilities; if no recipe-compliant seam exists, record the
   demonstrated reason rather than claiming a completed split.
2. Exact-parent canonical PURE MOVE exits 0 after formatting, generator
   regeneration reproduces the candidate, and an edited moved line fails the
   proof. Review every new-only line and visibility change. Preserve public
   APIs, test identities/counts and all hash/trace pins.
3. Update module headers and CRATES rows; map/strict checks exit 0. Original
   full CI, sim/sweep/arena/leak and applicable content/behavior/fidelity gates
   pass with nothing declared; no approvals, thresholds or goldens change.
   HUD also requires unchanged approved HUD/spawn/map captures and applicable
   GPU/golden/parity gates. Each slice retains its full original gate set.
