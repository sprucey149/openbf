---
id: GUIDED
title: Fly the award rocket launcher and return control to its soldier
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
The ordinary BF2 award rocket launchers were discarded as medal equipment. The maintainer assigned guided-rocket behavior while explicitly keeping medal progression separate; All awards supplies access. BF1's locally available ordinary class data has no equivalent. Portable auto-turrets and heroes remain outside this card's ownership.

## Evidence
analysis/re-guided-rockets.md: inherited disc census, flyer/remote executable branches, authored movement, shared control routing and distinct critical/noncritical explosion rules. No interactive or live PS2 validation was performed.

## Acceptance
Available local implementation checks: award gating and all 14 inherited launchers; player launch/control/return, energy/boost, collision/damage/owner credit, death and forced-switch cleanup, bot launch/steering, serialized-pad replay hashes, camera preview/interpolation restoration, map-local census, existing equipment and renderer regressions. Results are in docs/loop/reports/20261028_GUIDED.md.

Original-game acceptance remains open: pad mapping and launcher/camera offset frame, lateral/roll/high-pitch flyer fidelity, collision dimensions and crash/control-loss timing. The runtime does not load the original player-boundary/fly-height input; the guided boundary grace rule still needs that map integration. This card is parked, not complete.

## Notes

2026-10-28 maintainer playtest: guided rockets work, but turning is jittery. Snapshot build 577d409, BF2 COR1 seed 132, about 75 FPS with stable frame times. The mouse preview replaced interpolated rocket rotation with a full extra tick of filtered steering, including zero-motion mouse-aim frames. Remove that preview override and retain display-rate pose/camera interpolation. Regression covers 60/75/144 Hz, pending and zero mouse deltas, controller input and bit-identical simulation after drawing; see the report's follow-up.
