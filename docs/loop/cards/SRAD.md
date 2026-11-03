---
id: SRAD
title: Soldier collision to the PS2's (audit of JWAL, 2026-09-29): two 0.57 m spheres 0.15 m FORWARD of the feet (centres 0.57 and s*1.9-0.42), walls at n.y <= 0.6 (and upper-sphere n.y > 0) clip the velocity against the HORIZONTAL normal but push along the FULL normal (y - 0.02), pushes projected off the tick's earlier normals — the port's 0.5 capsule, WALKABLE_MIN_NY 0.5 and horizontal-only push differ; measure the PS2 step height on PCSX2 before touching STEP_HEIGHT
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why (rewritten by the JWAL audit, 2026-09-29, `reports/20260929_2240_audit_JWAL.md`)
The audit read the PS2 soldier's collision in both ELFs, and one Opus verifier re-read it (9/11 SUPPORTED; V3's push
direction was corrected and re-checked in the instructions). The port differs on every point. That changes every wall
standoff, door and corridor fit, sloped wall and step — the "plays right" of M3.
- **Shape** [V]: B1 `FUN_0020e2e0`, B2 twin `FUN_002517e8`.
  - The terrain / building / vehicle layers (0x40 / 8 / 4) collide with two spheres, `coll_body_lower` and
    `coll_body_upper`.
  - Radius 0.57 = the per-stance table 0x533090 (0.72) − 0.15 (ELF 0x20e468–0x20e494).
  - Both sit 0.15 m FORWARD of the feet along the soldier's facing.
  - Centres 0.57 and s·1.9 − 0.42 above the feet (crouch s·1.6 − 0.42; s = class+0x1e60).
  - Prone uses its own 'medium' spheres and branch. Soldier-vs-soldier uses `main_body` (a cylinder r 0.57, h 1.65).
  - So the PS2 stops 0.72 m from a wall he faces (PCSX2: 0.720), 0.57 side-on, and 0.42 backing in [D].
  - The port: one 0.5 m capsule (`unit::SOLDIER_RADIUS`, PROVISIONAL), no offset.
- **Response** [V]: B1 `FUN_00202f38`, B2 twin `FUN_0023cfd0`.
  - WALLS are a lower-sphere contact with n.y ≤ 0.6 (B1 `DAT_0051788c`, B2 literal) and an upper-sphere contact with
    n.y > 0.
  - For a wall, the VELOCITY is clipped against the horizontal normal (v −= n_h·(v·n_h) when negative). Then |v.x| and
    |v.z| under 0.1 snap to 0.
  - The POSITION is pushed along the FULL normal × depth, with 0.02 taken off an upward component (floored at 0). A
    downward normal is flattened.
  - GROUND is a lower-sphere contact with n.y > 0.6: it clips the velocity against the full normal, sets grounded and
    pushes along the normal.
  - Every push is projected off the tick's earlier contact normals (`FUN_00170268`). A projected depth over 2 m that
    also exceeds the contact's own depth is dropped.
  - The port: `WALKABLE_MIN_NY` 0.5 (PROVISIONAL); a horizontal-only push (walls.rs `push_out_with`); JWAL's velocity
    clip, without the 0.1 snap.
- **Sweep** [V]: the PS2 tests sphere-vs-MESH discretely.
  - `FUN_00160e60` finds a contact, runs the handler, then re-queries from the pushed centre; no previous position is
    read.
  - C+0x198 (the collision object's +0xa0, its previous position) feeds only the primitive-pair collide
    `FUN_001605d0`.
  - The comments in walls.rs that call the PS2 "swept" (WPSH's `push_out_soldier_from`) are wrong for meshes.
- **Step height** [H]: not read and not measured. A lower-sphere GROUND contact only exists at contact heights under
  0.57·(1 − 0.6) = 0.23 m above the feet. A wall contact's full-normal push can still creep a soldier up. The port's
  `STEP_HEIGHT` 0.6 is PROVISIONAL.

## Acceptance (audit)
1. **PCSX2 first:** walk/run a soldier in each game, and also sprint in BF2, at kerbs or primitives of known height (about 0.2,
   0.3, 0.45 and 0.6 m; find them in the bake). Record which are climbed. Also back him into a wall to check the 0.42 m
   standoff, and push him along a sloped wall.
   BF1's original game has no sprint (`re-stance-roll.md` 1 / 4.2b); compare the port's BF1-content sprint
   separately under its approved modern movement rules. Do not invent a BF1 PS2 sprint trial or undo SPUP.
2. **Build, gated:**
   - the two-sphere shape (or an exact equivalent) with the 0.15 m forward offset;
   - the 0.6 wall/ground split;
   - the horizontal velocity clip plus the 0.1 snap;
   - the full-normal push with the 0.02 lift skin, and the per-tick projection;
   - the measured step (keep JWAL's ledge rule if the shape change still needs it).
   Declare sim / sweep / arena / leak. The leak census and JWAL's City Hall test must stay at 0 new sites. Fix the
   walls.rs comments above.

## Original (2026-09-29, from JWAL's RE)
JWAL's PCSX2 trials (`analysis/re-soldier-walls.md` §5.1) ran a BF1 stormtrooper into City Hall's r 9.25 lobes.
- He stops 0.720 m from the surface head-on (east and north walls) and 0.751 on the west lobe.
- He stops 0.625 m in an oblique slide.
- Near the jump's apex he creeps 0.05 m closer, then back out.

The port's soldier is a 0.5 m capsule (`SOLDIER_RADIUS`, PROVISIONAL), so a port soldier hugs walls about 0.2 m
closer than the PS2's. The RE_LOG's open item "the soldier collision-bounds numbers behind `FUN_00235ad0` (the
census assumed r = 0.35 m)" is the same question.

## Evidence
- `lead_tmp/jwal/ps2exp/out/results.json`, the per-trial CSVs.
- [H] The collision test is swept from C+0x198 (§5.3).
- [H] The apex creep fits a sphere of about 0.72 m whose centre is about 0.6 m above the feet.

## Acceptance
- RE the BF1 (and BF2) soldier's collision shape against primitives and meshes: radius, the vertical extent, and
  the swept test. Cite the functions and run the re_check discipline.
- Decide the port's radius. It changes every wall standoff, door and corridor fit, so gate it
  (declare sim / sweep / arena / leak).

## Notes
- Pairs with WPSH (the swept test) and JWAL (the ledge rule's capsule).

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — no soldier out of bounds; every wall standoff. Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

- Planner 2026-09-30 21:45: `deps` + GSKIP. This card's diff lies under `engine/crates/core/`, which the selective gates skip for goldens, yet core feeds golden pixels (the gallery vantage's `ray_down` on COR1 / DEA1 / MYG1 / TAN1 / UTA1 / BF2 KAM1, the heightfield under grass and foliage, the walker pose). GSKIP corrects the rule first; `land.py` has no flag to force every group.
- GSKIP landed bc6a047 (2026-10-01): `gates.py --auto` now runs every golden group for a `core/src/collision` diff; `--golden-groups all` is no longer needed here.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
