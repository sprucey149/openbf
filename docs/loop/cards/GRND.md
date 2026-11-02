---
id: GRND
title: The ground query is one ray: a soldier standing on a slit between collision floor pieces (KAM1's cloning rooms) falls 7.9 m onto the hidden soldier-only disc, and a steep face over a walkable one hides it - the PS2 soldier stands (drop test 69.272 vs 69.273)
milestone: M3
value: 5
phase: build
status: done
deps: STHOP,GSKIP
files: engine/crates/core/src/collision/rays.rs,engine/crates/core/src/sim/state.rs,engine/crates/game/examples/floor_census.rs
declares: sim; sweep; arena
needs:
effort:
branch:
resume: closed: built in 3c7b00c (foot ring + ray past a steep face); lead_tmp/grnd-fable/README.md. Left for a new card: the census as examples/floor_census.rs, the other 30 maps, the shallow 0.3-0.6 m holes
created: 2026-09-30
alpha: yes
---

## Why
The maintainer's "invisible floors/platforms" on Kamino. The hidden collision under the map is the disc's own (a soldier-only
disc r 102 m 7.9 m under each cloning facility, a core cylinder inside each landing pad); what puts a soldier ON it is
the port's ground query. `analysis/re-kamino.md` §4 ([O]).

## Evidence
- PS2 drop test (`lead_tmp/kamr/ps2exp/kamr_ps2.py drop`): released on the slit at (−197.35, 58.17) in BF2 KAM1's
  central chamber the soldier settles at 69.272 (control spot 69.273). The port's `ground_under` there is 61.409 — the
  hidden disc: a walking soldier is airborne one tick, a standing or stopping one (and a bot that halts) drops 7.9 m.
- `Sim::ground_under_with` casts ONE ray at the feet's (x, z); `CollisionWorld::ray_down_with` takes one hit per body and
  drops the body when that face is steeper than walkable — a 61° bevel 0.18 m over a walkable face at the pads' rim
  (21.65, −51.83) sends the answer to the core 9.4 m below.
- The floor census (`lead_tmp/kamr/tools/floor_census.rs`, scratch): BF2 KAM1 at 0.5 m — 4,033 drawn-floor points with
  no collision next to reachable floor (1,008 m²: the cloning-room slits, the pads' rims), 544 ground-query holes;
  BF1 KAM1 24,344 / 313. All 32 maps at 1 m: `lead_tmp/kamr/census/all_maps_cell1.md`.
- Not defects: the rail (0.57 m) IS walked over on the PS2 (feet 78.42 → 79.11, `out/rail_0.csv`), and the hull is
  then walked GLUED at 34° — STHOP's reference trace.

## Notes
- Census leads NOT followed (other maps): BF2 MUS1 — `mus1_bldg_conference`'s collision floor 0.5–0.8 m over the drawn
  `mus1_bldg_world_1` surface on 15,160 m²; BF1/BF2 END1 — ~709,000 m² of reachable heightfield outside the drawn
  terrain; BF2 MYG1 platform cores; BF2 UTA1 `p_floor` boxes. Look before carding.

## Note from STHOP's build (2026-09-30, lane 1): the 0.5-0.6 band, and what the ground query now is
- STHOP landed the contact under the NEW feet (`Sim::ground_under_n(feet, rising, from_y)`: the ray starts at
  max(feet + step, the feet's height before the move)) and `Soldier::resolve_ground` (n.y > 0.6, 2 cm, approach < 0.1).
  GRND's "one ground rule" builds on that pair, not on the old `ground_under` clamp.
- **A face with 0.5 <= n.y <= 0.6 is neither a wall nor a floor in the port.** The wall pass and the sweep call a wall
  only under n.y 0.5 (`WALKABLE_MIN_NY`); the soldier stands only over 0.6 (the PS2's limit). STHOP holds the feet out
  of such a face (he slides, airborne), stands him when wedged (`WEDGE_MAX_DY`, PROVISIONAL) and makes the AI steer
  round it (`steer::steep_ahead`).
- **Tried and dropped:** `WALKABLE_MIN_NY` 0.5 -> 0.6 for the whole wall pass (the PS2's wall limit). 8 seeds x 32 maps:
  220/256 rounds (main 228), 25 maps >= 7/8 (KAS2 3/8, UTA1 5/8, YAV1 5/8) - `lead_tmp/sthop/sweep6_ny06`. The port's
  routes cross such faces (roots, buttresses); the PS2's 0.57 m sphere foot probably rolls over what the port's point
  foot meets as a wall. Do it with the sphere foot, not before.
- Stuck spots found on the way (steep faces on a route): TAT1 (-137, 17.3, 95.5) a 55 deg ramp 2 m high into a pocket;
  GEO1 (-72, -62.8, 268.2) a buttress of n.y 0.52-0.60.

- Planner 2026-09-30 21:45: `deps` + GSKIP. This card's diff lies under `engine/crates/core/`, which the selective gates skip for goldens, yet core feeds golden pixels (the gallery vantage's `ray_down` on COR1 / DEA1 / MYG1 / TAN1 / UTA1 / BF2 KAM1, the heightfield under grass and foliage, the walker pose). GSKIP corrects the rule first; `land.py` has no flag to force every group.

- Fable 2026-10-01: BUILT, 3c7b00c. bf2_kam1 holes 544 -> 139 (none of the 25 largest clusters deeper than 0.47 m; were 98 points at -9.7 m), bf1_kam1 313 -> 304 (shallow). Sweep 8 seeds: rounds 226 -> 227, KAM1 region deaths 367 -> 267. The acceptance's `holes = 0` is NOT met (shallow rim and seam steps remain) and the census example is not committed; `lead_tmp/grnd-fable/README.md`. The leak census's sites fell 66901 -> 13030 with it (new 0), baseline `loop_baselines/20261001_1017_FABLE`.
- GSKIP landed bc6a047 (2026-10-01): `gates.py --auto` now runs every golden group for a `core/src/collision` diff; `--golden-groups all` is no longer needed here.
