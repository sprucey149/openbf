---
id: VTURZ
title: The building turret's two views as the PS2's (the maintainer's answer to STATUS 47, INBOX 2026-10-13): third person with the camera turning with the gun, and R3 for the zoomed scope view at the eye point along the aim in which his own turret building is not drawn (other viewports still draw it); no gunner's-eye view. Split from VTURX, whose wreck half stays there
milestone: M2
value: 5
phase: build
status: done
deps: VTURD
files: engine/crates/game/src/viewer/scene.rs,engine/crates/game/src/viewer/turret_pose.rs,engine/crates/game/src/walk.rs,engine/crates/core/src/sim/guns.rs,engine/crates/core/src/sim/seats.rs,engine/crates/core/src/sim/vehicles.rs,engine/crates/core/src/vehicle/gun.rs,engine/crates/game/src/vehicle_class.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/main/headless.rs,engine/crates/game/examples/pose_render.rs,engine/crates/game/tests/turret_views.rs
declares:
needs:
effort:
branch: vturz
resume: landed 24931419 2026-10-16
created: 2026-10-15
alpha: yes
---

## Why
The maintainer's answer to STATUS question 47 (INBOX 2026-10-13): "build what the PS2 does" - in a building turret he sits in
third person with the camera turning with the gun, and R3 gives the zoomed scope view in which his own turret is not
drawn. No gunner's-eye view (his earlier "first and third person" wording is dropped). VTURD (landed 1cf5c8b) made the
turret turn; this card is the camera. Value 5, alpha: his ask, seen in the first seconds in any turret.

## Evidence
- `analysis/re-turret-draw.md` sections 4-5 [O] (VTURD's RE, 2026-10-13): a building turret is always third person;
  R3 on a turret weapon with `ZoomMax` > 1 puts the camera at the eye point along the aim and the WHOLE building is left
  out of the gunner's own viewport (other viewports still draw it; the gunner stays in the scene list). A weapon
  switch or a pilot change ends the zoom. Not read: what draws the scope frame (the same in both games; not the class's
  `FirstPerson` model) - read it or take the soldier zoom's frame as a PROVISIONAL stand-in.
- PS2 frames: `engine/artifacts/lead_tmp/vturd/sheet_ps2_cases.png` (BF1 Naboo gun turret and BF2 Mygeeto gun, zoomed:
  a scope frame, no turret).
- The port has a soldier zoom (`core/src/unit.rs` `camera_section(.., zoomed)`, `weapon.rs` `zoom_max`); whether a
  SEAT has one is not checked - look first in `game/src/viewer/look.rs` and `sim/input.rs`.
- Check first, before building anything for the third-person half: does the port's turret camera already turn with
  the gun since VTURD? If yes, that half is a test only.

## Notes
- VTURL (the turn law at 180 degrees) is in the same viewer files; whichever lands second rebases.
- The wreck half stays on VTURX (deps TBLD).
- 2026-10-16 LANDED 2493141 (report `reports/20261016_0050_card_VTURZ.md`). Third person already turned with the gun since
  VTURD (a test only). Built: the turret's zoom (R3 toggle; BF2 left-stick zoom by ZoomRate; a pilot change ends it), the
  zoomed camera at `EyePointOffset` on the aim frame (0.1 mm from the PS2's camera on 8 RAM rows) with the class's
  FirstPersonFOV (12.98 deg on Mygeeto; the PS2 frame fits 13.0-13.5), the zoomed viewer's building left out of his own
  view (`SceneGpu::view_draws`). RE `analysis/re-turret-draw.md` 4.1. Not built: the scope frame (noted on ZSCP); the
  zoom's turn divisor on a turret (unread). Pairs: `lead_tmp/vturz/pairs/sheet.png`.
