---
id: RKHULT
title: RKHUL's tests pin neither the prim rotation's ORDER (mutation passes all 161, moves 162 of 921 side prims) nor a chunk / non-bone-space prim (6 shipped prims); TAT1's map-local skiff keeps 5 prims at the identity rule (fins 4.0 m off); two stale comments
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT RKHUL (2026-10-22). The landing's rule is right (the PS2's XFRM x bone order, B2/B1 FUN_00160160; the bake equals the drawn skin GLB's bind x raw XFRM on 283 walker prims, worst 3.3e-6), but its tests pin less than the rule:
- Mutation M4 (rotation composed in the order bind x XFRM instead of XFRM x bind, `bake::bind_pose::prims_at_bind`) passes all 161 bake tests and changes the rotation of 162 of 921 baked side prims (59 by more than 0.5 in a matrix entry; BF2 CIS p_cannonR, IMP p_wing_R, p_-so_cube8L/R, GUN p_tail, the AT-ST p_r_thigh). The walker test checks translations only; the door test's prims have identity rotations, where the order cannot show.
- Mutation N6 (`vehicle::Index::rest_prims` composes only when the geometry's top LOD is bone-space) passes all 161 and reverts 6 shipped prims on chunk geometries (BF1 CIS / IMP, BF2 IMP vehicles.ron): no test covers a chunk geometry's or a non-bone-space vehicle's prims.
- The map bake keeps the identity rule for a MAP's own vehicle class: only TAT1's `tat_hover_skiff` (TAT1.LVL / TAT1H.LVL; fielded on pads cp4_vehiclespawn_10 and cp7_vehiclespawn_8) has prims on non-identity bones - p_fin1 / p_fin2 (bone_rudderL / R) 4.02 / 4.06 m from their bone spots, each on the side opposite its stored x; p_gun1..3 1.68 / 1.84 / 3.37 m. Its hull bodies (p_soldiercube, p_vehiclesphere*) are on dummyroot (identity) and untouched. The BF1 NAB2_H gian speeder has two more but is never fielded. TCOL's build extends the map bake's prim rule past doors; check whether that covers this class.
- Stale text: `game/src/vehicle_class.rs:366-376` still says hull_parts takes PRNT as identity; `bake/src/vehicle.rs` says the spider's stored prims "all lay within 2.9 m of the origin" (its four foot spheres are stored 5.57 m out); the census count is 53 classes (two independent censuses), not 52.
Equivalent survivors (no shipped change, no test needed): skel lookup geometry-first, exact-case PRNT name, root-bone skip, geometry-only skel, LOD name passed.
## Evidence
engine/artifacts/lead_tmp/audit-rkhul/: logs/mut_*.log, tools/mut_chain.py (M0-M12 in one lock hold), tools/mut_bake.py + bake_M4_rotation_order_swapped/, wf/C4-new-mutations/ (N1-N6, bake_N6_*), logs/census_side.txt + census_maps.txt + maplocal_prims.txt (tools/bake_census.rs), wf/C5-maplocal/, wf/C1-python-census/.
## Acceptance
A vehicle bake test with a prim whose stored rotation is not the identity on a rotated bone (one of the M4 list) and a chunk-geometry prim: `python3 engine/artifacts/lead_tmp/audit-rkhul/tools/mut_chain.py M4_rotation_order_swapped` and the N6 mutation (wf/C4-new-mutations) each FAIL a test; the tat1 bake's skiff p_fin1 / p_fin2 at x -1.77 / +1.77, z -4.10 (bind) if the map rule is changed; the two comments true.
