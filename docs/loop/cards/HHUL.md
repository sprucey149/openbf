---
id: HHUL
title: Hover hull bodies meet the world: which of a hover model's bodies the PS2 tests against terrain and buildings (the class's TerrainCollision/BuildingCollision subsets, the coll mesh, the spring spheres) → HoverParams::hull, then K7-K9 on them
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## RE result (2026-09-30, lane 3; analysis/re-hover.md §13 [O], verifier 15/2/0 of 17, all folded in)
- **Only the spring spheres meet the terrain and buildings** on every shipped hover, in both eras. The coll mesh and
  every other primitive meet neither.
  - The one addition: the BF2 `imp_hover_fightertank` / `rep_hover_fightertank` box `p_cube1` also meets
    **buildings**.
  - `p_cube1`: half-extents (1.511, 0.491, 0.956), at (0, 0.511, 1.559), identity rotation, parent `bone_root` at the
    origin.
- **Why.** Every `AddSpringBody` / `SetSpringBody` line calls the `BuildingCollision` and `TerrainCollision` handlers
  with the spring's name. Those overwrite the subset with the names since `GeometryName`.
  - A mesh body's terrain and pair slots return 0, so the mesh is inert on the moving side anyway.
  - PS2 RAM: 18/18 live hover classes hold exactly these subsets.
- **So the card's premise was only half right.** Springs stopping a hover nose-first at a wall IS the PS2: K5 lets a
  spring sphere sink BodySpringLength into a wall, then pushes it out. The port already does that.
- **What the build changes:**
  1. `p_cube1` on two classes. Its hull flags come from replaying the class's ordered PROP list (§13.2).
  2. The springs' terrain contact becomes the PS2's sphere rule (§13.5): one height sample under the lowest point,
     0 ≤ d = h − c.y + r ≤ 2r, depth d·n.y, the cut tested at the surface point. It is shallower than the port's plane
     rule by r(1 − n.y) on slopes.
- **The world side** (each building's VEHICLE subset, not the soldier subset) went to new card **VSUB**.

## Landed (2026-10-03, 80a8edc; report `reports/20261003_0625_card_HHUL.md`)
- Unit tests: §13.5 by hand (30°, r 2: 0.268 shallower); the replay over 12 BF1 + 10 BF2 classes (`p_cube1`, building
  only, on the two BF2 fighter tanks); the tank stops at a thin post with the post 2.515 m ahead of its origin.
- PS2 RAM: quiet parked hovers inside the bob band 40 → 47 of 47 (mean |dy| 2.6 → 1.3 cm). Pad census 0 of 216.
- Sweep: the 14 hover maps moved, 32/32 finish; 6-seed A/B: hover crashes 169 → 167.
- Left: a hull box / cylinder against the terrain (no class needs it); the vehicle subset (VSUB).

## Why (original, 2026-09-28)
Split from HOVX (2026-09-28). HoverParams::hull is empty: only the spring spheres meet the world, so a tank or bike nose-first into a wall is stopped only by its springs (K5). The contact response for hull bodies (K7-K9) is already in core::vehicle::hover::contacts; what is missing is WHICH bodies.
## Evidence
re-collision-layers §4-§6: the terrain object FUN_00212530 uses the moving object's TerrainCollision subset (… vehicles (layer 4) are moved against buildings (8) + terrain (FUN_00163008). With no property line, EVERY body (mesh + prims) is in the terrain and building subsets. Side census (baked vehicles.ron, 2026-09-28): no shipped hover authors TerrainCollision; only the BF2 fighter tanks author BuildingCollision = p_cube1; BF2 all_hover_combatspeeder, snailtank and barcspeeder author no collision lines at all. HOVX card note: FUN_00169668 (B2:7937-8054) pushes out fully only for other-class 0x10/0x80.
## Questions (RE, bounded)
1. Does FUN_00212530 (BF1) / its BF2 twin test the coll MESH body against the heightfield, or only prims (sphere/cylinder/box)? If only prims, which prim kinds?
2. Are the AddSpringBody spheres bodies in the model's subsets (they are FUN_00168cd0 lookups, re-hover §4 S1) — do they meet terrain through the subset as well as the spring hook?
3. The prims' shapes for the port: spheres exact; cylinders / boxes as what?
