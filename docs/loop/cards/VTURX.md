---
id: VTURX
title: A destroyed building turret draws its wreck, and a zoomed gunner does not see his own turret: in build state 1 the building is its DestroyedGeometryName model drawn rigid at the instance (none of the 49 destroyed models on the discs holds a posed node), and while the gunner is zoomed (R3) the whole building is left out of his own view, as the PS2 (RE done 2026-10-13, analysis/re-turret-draw.md 4 and 5)
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The older half of card VTURD ("the turret swaps to DestroyedGeometryName when its pool empties") and the PS2's one
other turret view, split off by VTURD's RE (2026-10-13, `analysis/re-turret-draw.md` 4 and 5 [O]) so the maintainer's ask
(the turn, the gunner) is not held up. Value 3: neither is the ask; both are seen within a few minutes on Hoth and
Mygeeto, where turrets fall in every battle.

## Evidence
- **The wreck.** A dead armed building goes to state 1 and swaps to its `DestroyedGeometryName` model with that
  model's collision (state machine: `analysis/re-build-zones.md` 1, card TBLD); its turret's tasks are removed, so
  the angles freeze, and the draw does not look at the state - it poses whatever model is current. None of the 49
  destroyed models on the discs (BF1 4 classes, BF2 45) holds a turret, aimer or barrel node, so the wreck is one
  rigid model at the instance. PS2 frames: `lead_tmp/vturd/ps2/myg1_destroyed_a.png` (BF2 Mygeeto's anti-aircraft
  gun, state 1, health 0 of 4000: a low black wreck), `lead_tmp/vturd/ps2/hot1_bots_lasermortar_turet4_a0.png` (a
  Hoth laser tower fallen in a battle: an open broken shell under an empty health bar).
  `antiaircraft_chunk1` is one name for two different models on the two discs.
- **The zoom.** A building turret is always third person; R3 on a turret weapon with `ZoomMax` > 1 puts the camera
  at the eye point along the aim and the WHOLE building is left out of the gunner's own viewport (other viewports
  still draw it; the gunner himself stays in the scene list). A weapon switch or a pilot change ends it. Frames:
  `lead_tmp/vturd/sheet_ps2_cases.png` (BF1 Naboo gun turret and BF2 Mygeeto gun, zoomed: a scope frame, no turret).
  Not read: what draws the scope frame (the same on both; not the class's `FirstPerson` model).

## Acceptance
- A test: a turret building in the destroyed state draws its class's `DestroyedGeometryName` model and none of the
  live model's parts, and draws the live model again when rebuilt; through the baked class db: the count of classes
  with a destroyed model per game = 4 (BF1) and 45 (BF2), each model found in the bake. One picture beside each of
  the two PS2 frames.
- A test: with the gunner zoomed the turret building's draws are absent from his own view and present in another
  view; unzoomed they are present. One picture pair beside the PS2's zoomed frame.

## Notes
- Needs card TBLD for the state (the port's turret today is removed like a vehicle when its pool empties) - or, if
  TBLD is still open, the minimal state "pool empty = destroyed" in `core/src/sim/vehicles.rs` with TBLD's fields.
- If the port's seat has no zoom yet (card ZOOM was the soldier's), build the destroyed half and file the zoom as a
  note on the card that brings a seat zoom.- 2026-10-15 planner: **the zoom half moved to card VTURZ** (value 5, alpha: the maintainer's answer to STATUS 47 asks for it); this card is the
  wreck alone. Its zoom acceptance line and the zoom part of `resume:` are VTURZ's now. Undo: `lead_tmp/planner_1015/VTURX.before.md`.
