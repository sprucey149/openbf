---
id: PRIMB
title: Collision prims of non-door geometries rest in their PRNT bone's frame too: census every prim whose bone binds off the identity, then put them through the bind matrix (DEA1's trash compactor walls collide back to back while drawn 12-20 m apart)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
STDOR (2026-10-01) showed a door's PRNT prims are stored in their bone's frame, as BNAM segments are (TBIND), and put DOOR geometries' prims through the bind matrix (`BindPose::rest_prims`). Every other geometry's prims are still read as model space (`mungefs::coll`'s PROVISIONAL), so a prop whose bones bind off the identity collides where it is not drawn.

## Evidence
- TBIND's note on card BNR: `dea1_prop_trashcompactor` draws its walls at z +-5.8..9.8 through their bones, and its prims share the segments' frame (`p_cube4` under `bone_wall1a` at z 1.28) - the collision walls stand back to back in the middle.
- `analysis/re-bf2-doors.md` section 10 (the rule and the PS2 pairs). `bake::walker` already takes its prims at the bind pose (`prim_points`).

## Acceptance
- A census over every map and side LVL (a test or an example): prims whose PRNT bone binds off the identity, per class - the count listed before, and 0 left reading model space after (or each exception named with its PS2 evidence).
- DEA1's compactor: each wall prim's centre within 0.1 m of its drawn segment's centre (a data-gated test).
- Scratch bake diff lists only the classes the census named. Leak census new 0. Sweep rows of maps without such a class unchanged.

defer: post-alpha (DEA1 is not a showcase map). Vehicles and turrets (side bakes) are in scope of the census; fix them only with a PS2 check per class.
