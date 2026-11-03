---
id: FWDC
title: BF1 Geonosis cp2's building is missing: rep_bldg_forwardcenter (BASE commandpost, model + skel + collision in side REP.LVL, loaded by the conquest script's ReadDataFile) is drawn and solid on the PS2 and has no model and no body in the port - bake the side-LVL buildings a mission names and merge them into the battle's scene (analysis/re-skipped-classes.md 5)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BF1 Geonosis cp2 (the Republic's forward command center) is a large hull with a tower and a ramp on the PS2 and open desert in
the port, in a battle too; bots and the player walk through where it stands. Found by card SKIPC's RE (2026-10-15),
`analysis/re-skipped-classes.md` section 5 [O]: the class row is in `assets_baked/classes/bf1_rep.ron` (BASE commandpost,
GeometryName rep_bldg_forwardcenter, DestroyedGeometryName rep_bldg_forwardcenter_dest, MaxHealth 30000, explosion _exp,
prims p_vehiclecylinder1/2 + CollisionMesh); its skel / modl / gmod / coll / prim are in SIDE/REP.LVL (both discs); the
conquest mission geo1r loads it with ReadDataFile("SIDE\\rep.lvl", "rep_bldg_forwardcenter", ...); no bake path takes a
side-LVL commandpost class, so it is the one placed instance on the port's 32 conquest layers a battle cannot find.

## Evidence
- `engine/artifacts/lead_tmp/re/skipc/ps2/geo1_forwardcenter_pairs.png` (PS2 | port, two stands 20 m north and west of it;
  poses `ps2/bf1_geo1/fcN_b000.json`, `fcW_b270.json`).
- The census: `engine/artifacts/lead_tmp/re/skipc/census_main_efa2d56.txt` (the only NOCLASS row in the battle column).

## Acceptance
- The census example (`lead_tmp/re/skipc/skipc_census.rs`) prints no NOCLASS row in the battle column on any map.
- A battle frame (`game --headless --cam ... --capture-frame`) at the two stored PS2 poses shows the hull and tower in the
  PS2 frame's boxes.
- A test: a soldier walked at the hull is stopped by it (the class's collision as the sim builds it).
- BF1 GEO1 over 8 seeds, main against the branch: stalls and recycles within 10 m of cp2 printed (the building is new
  geometry at a post the bots capture). Open first: is it destructible on the PS2 (MaxHealth 30000, a _dest model)?
