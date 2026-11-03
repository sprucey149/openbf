---
id: VHULL
title: Parked hulls are solid: a spawned vehicle's coll/prim bodies join CollisionWorld at its transform — soldiers stop at it, bolts hit it (VDMG's hull pool goes live), AI rays see it; a declared sim/sweep move
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
After VDRAW, a parked vehicle is visible but soldiers walk through it and bolts fly through it; VDMG's hull damage
path is reached only by tests. On the PS2 a vehicle is a solid body that bolts hit.

## Evidence
VDMG report (`reports/20260926_1225_build_VDMG.md`): "Hulls join the bolt ray when HOVR/VSEAT put them in the
collision world". VBAKE bakes each class's `coll` mesh + `prim` bodies (3 undecoded prim kinds dropped: both AT-TEs, BF2
oneman AT-ST). `core/src/collision/world.rs` (`move_body` already moves door bodies).

## Notes
- Planner 2026-09-29 19:05: value 4 → 5. M2 had no value-5 card after WALK. PLAN's M2 is ground vehicles first, and soldiers and bolts passing through every parked vehicle is the most visible ground-vehicle gap left; it also takes VDMG's hull pool live. Reverse: set value=4.
