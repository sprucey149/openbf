---
id: KRMP
title: Kamino's inclined ramps between the round platforms are walkable but their walkway is not drawn (the maintainer, kam1_play_11 / kam1_play_7): the rails and lamp housings draw, the floor between them does not — several on BF2 KAM1, check BF1's; KAMR did not find them
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/bake/src/map.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/game/examples/floor_census.rs
declares:
needs:
effort:
branch:
resume: closed: fixed in ec47e0b (door indices after the ocean splice); lead_tmp/kamdoor/README.md
created: 2026-09-30
alpha: yes
---
## Why
The maintainer's INBOX 2026-09-30 18:45: KAMR's "invisible floors" are **the inclined floating ramps that connect the circular
platforms**. He can walk on them, but their surface is not drawn; there are several on the map. KAMR (closed 19:35)
answered a different defect under that name (GRND: the floor slits in the cloning rooms) and its report lists the maintainer's
spots as NOT FOUND — so the ramps have no card. Kamino is not a showcase map, but the maintainer plays it and asked for its
bring-up as alpha (KAMR, value 5).

## Evidence
- `engine/artifacts/lead_tmp/playtest_0930/kam1_play_11.png` (the PORT, 2570×1481; viewed by the planner): the player stands
  on a deck edge looking down toward a round platform with a command post. Rail strips with lit lamp slots and two lamp
  housings draw; between the rails, where the walkway should be, the frame shows the platform hull below.
- `kam1_play_7.png`: "above my head, a little to the right" — a short spur out of a round platform with a turret on it;
  the rails render (about x 1030–1510, y 340–375 of the 2000-px view), the floor under them does not. (Positions from
  KAMR's card; the maintainer's reading.)
- KAMR's floor census (`analysis/re-kamino.md` §4.1; tool `lead_tmp/kamr/tools/floor_census.rs`, memory
  `floor-census-and-ground-probes`): BF2 KAM1's class A (walkable, no drawn triangle within 0.3 m) is 105,082 m², named as
  the two hidden discs, eleven platform cores and "the tops of the 2.2 m wall boxes along bridges and ramps". The ramp
  WALKWAYS are not named in A. So either [H1] a drawn triangle lies under the ramp's collision and the renderer shows
  nothing there (a blend / alpha-test material with zero alpha, a one-sided face seen from its back, a segment the draw
  list drops), or [H2] the census's flood never reached the ramps and their mesh is missing from the bake (a dropped
  segment or model, a LOD / `lowd` split). Neither is checked.
- No PS2 frame of a ramp is cited anywhere: the 24 stored walk pairs stand at the posts.

## Notes
- Planner 2026-09-30 21:45: filed from the maintainer's 18:45 note. Phase `build` (a port defect hunt, the PS2's behaviour is not
  in question); if step 3 lands on an open rule card (ADDM / ELFR2 / BFC), build it here for Kamino's case and leave a
  note there.
- The grey ellipse of `kam1_play_7` (KAMR: not found) may be the same family (a surface seen from below with its top
  missing): check it while standing there.

- Fable 2026-10-01 (the maintainer's chat session): ONE cause for KRMP + KGRY + KPOS, fixed in ec47e0b - the ocean step splices the draw list and the door poser kept its upload indices, so `pose_doors` moved ramps, decks and props to the doors' places (`lead_tmp/kamdoor/README.md`, `analysis/re-kamino.md` section 9). Nothing was wrong in the bake, the materials or the instance transforms; no headless path steps the water and poses doors together, which is why KAMR saw no misplaced object.
