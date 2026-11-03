---
id: FOLGC
title: Foliage collision: the prp_ path props (tree-billboard lines, DAG1/YAV2/FEL1 hellbushes) are ordinary props with their ODF's collision, and the 0x26edc40c meshes (BF2 FEL1 pitcher bush, GEO1 flat rocks) are solid — the port draws both and collides with neither
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

- **Why:** re-foliage §9/§11: path props are created as entity instances of their class (their ODF's model, collision, leafpatch attachments); a Mesh entry with 0x26edc40c builds a CollisionMesh at every placed spot, at load, map-wide. FOLG (2026-09-26) bakes both (Manifest.foliage.path_props; ManifestFoliageMesh.collision + foliage.bin) but keeps them out of the collision world so the sim gates did not move.
- **Build:** Scene::collision_world adds the path props as bodies (after the manifest instances, so the existing order is unchanged) and the collision-flagged foliage models (their class geometry's coll mesh at the instance matrix, game::viewer::foliage::matrix — move it where the sim can reach it). Declare the sweep/arena moves on the maps that carry them (BF1 END1 KAS2 NAB2 YAV1 YAV2, BF2 DAG1 END1 FEL1 NAB2 YAV1 GEO1). The maintainer's rule: no collision unless the PS2 gives it one — these are the ones it gives.
- **Acceptance:** a unit stops at a KAS2 treebill and at a FEL1 pitcher bush; walks through the ferns.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — soldiers walking through trees (YAV2). Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Note (VREST RE, 2026-10-07): a pad's spawn ray stands on the solid foliage rocks
`analysis/re-vehicle-rest.md` §4: BF1 GEO1's gunship at pad `laat3` stands 0.249 m higher on the PS2 than terrain +
LandedHeight, because a `PropCollisionObject` (one of the `geo_prop_flatrocks` this card makes solid; box (58.887,
-63.005, 273.348)..(66.151, -62.390, 280.100)) lies under the pad and the pad's ray (terrain + the building layer) hits
it. When the rocks join the collision world, the pad spawn's ground probe must see them; the referee is
`lead_tmp/vrest/join.py` (the Flyer row: `laat3` goes from +0.249 to 0).
