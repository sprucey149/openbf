---
id: ZOOM
title: Zoom does nothing: the ZOOM bit (R3 since 2026-10-04) has no consumer in the sim or the camera - the PS2's zoom per weapon (ZoomMin / ZoomMax / ZoomRate, the zoomed turn rate, first person on the sniper), read and built
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-04: zoom is the right stick's click. The binding is in (commit 6300e19), but `buttons::ZOOM` has no consumer: nothing in `core::sim` or the camera reads it (grep, 2026-10-04).

## Evidence
- `core/src/aim_assist.rs` already divides the turn rate by a zoom factor when one is given; nothing sets it from the pad.
- RE DONE 2026-10-06: `analysis/re-zoom.md` ([O]; verifier 48 of 52 SUPPORTED, 4 PARTLY folded in; live PS2 rows `engine/artifacts/lead_tmp/zoom/ps2/out/`). What the PS2 does, both games:
  - R3 is a press toggle, a human's soldier only: off -> `ZoomMin` -> `ZoomMax` (only when it differs and `ZoomRate` is 0) -> off. Rifles and pistols 2.5 / 2.5, launchers and shotguns 2.0, repeater 3.0, sniper rifles 2 / 8 (BF1 Empire / Republic 6 / 6); no zoom key = no zoom (grenades, tools); BF2's lightsaber never.
  - Cancelled by: a reload of the primary, a switch of the weapon in hand, the secondary's throw, a roll, being thrown, death, a seat change; BF2 also a sprint. Kept through walking (same speed), a jump, a crouch, firing.
  - Camera: the projection scale S eases to the zoom, S <- z + (S - z) exp(-25 dt); vertical tan = tan(FOV / 2) x 0.7666 / S with FOV 48 deg in BF1 (zoomed or not) and 65 -> 70 deg in BF2 (eased the same): rifle 37.69 -> 15.55 deg (BF1), 52.06 -> 24.24 deg (BF2). Third person takes the stance's ZOOM section at once: TrackOffset (0.4, 0.05 stand / 0.2 crouch / 0.2 prone, 2.8), tilt 3.5 deg. A `ZoomFirstPerson` weapon takes the scope view: the eye at the fire point less 0.3 m along the aim (BF2 0.6 m), no tilt, own body not drawn.
  - Turn: yaw and pitch rates and the aim assist's rate divided by the weapon's turn divisor (= the zoom value on every disc weapon).
  - Spread: unchanged on disc data (the `*AimSpread` column is 0.0 like the still column); pass the flag anyway.

## Notes
Filed 2026-10-04 by Claude in chat from the maintainer's playtest (build 1bde70c, main 4e51e84). The maintainer's own words are quoted; nothing here is measured unless it says so.
- 2026-10-05 planner: value 4 -> 5: a bound button (R3, the maintainer's own binding of 2026-10-04) that does nothing; a sniper cannot zoom. Undo: `loopctl.py set ZOOM value=4`.
- 2026-10-07 LANDED 08d6343 (report `reports/20261007_0314_card_ZOOM.md`): acceptance 1-5 run and pasted there (tests `the_baked_weapons_zoom_as_the_ps2s_do`, `the_zoom_press_steps_the_weapon_in_hand`, `the_zoom_ends_on_its_cancels_and_no_others`, `the_zoomed_turn_rates_are_divided_by_the_divisor`, `the_zoomed_camera_is_the_ps2s`, `the_zoom_eases_the_projection_as_the_ps2_does`; pair `engine/artifacts/lead_tmp/zoom/pair/zoom_pair_bf1_nab2.png`). Not built: ZoomRate's continuous zoom (turrets), the eased zoom out (CAMS), the scope overlay and sounds (ZSCP).
