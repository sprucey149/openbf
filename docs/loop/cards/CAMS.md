---
id: CAMS
title: The soldier chase camera per class and game: the port frames every soldier with one BF2 stance table (walk.rs camera_section: stand 3.2 m back, crouch 3.0); BF1's classes author stand 3.5 / crouch 3.5 (+0.15 up) / prone 3.0 and the PS2 sits exactly 3.50 m behind the track centre in 19 of 22 BF1 states - read each class's CAMERASECTION (Wookiee smuggler 2.25 / 4.0, jet trooper tilt 7.5, droideka, locals)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
UAUD1 (analysis/unit-audit-bf1.md §3.6). The port's soldier chase camera is one stance table, `camera_section` in
game/src/walk.rs 41-49: stand (centre 1.8, up 0, back 3.2), crouch (1.3, 0.15, 3.0), prone (0.5, 0, 3.0), sprint
(1.8, 0, 4.0). Its doc says these are BF2's `com_inf_default` values, and it serves both games. BF1's own classes
author a different table, and the PS2 uses it: in 19 of the 22 BF1 save states the camera eye is exactly 3.50 m
behind the player's track centre (feet + 1.8 m), along the view's back axis, at pitch -9.01 deg. The other 3 are
shorter (2.99, 3.22, 3.35 m), still on the view axis; [H] pulled in by geometry behind the player. The port frames every BF1 soldier 0.3 m too
close standing and 0.5 m too close crouched.

## Evidence
- Measured: `engine/artifacts/lead_tmp/unit-audit/camdist.py` -> `camdist_bf1.txt`.
- Disc: `census_ia.md`, each class's CAMERASECTION blocks. 33 of the 52 BF1 IA classes share one set:
  - STAND: eye and centre 1.8, TrackOffset (0, 0, 3.5), TiltValue 5;
  - STANDZOOM: (0.4, 0.05, 2.8), tilt 3.5;
  - CROUCH: 1.3, (0, 0.15, 3.5); CROUCHZOOM: (0.4, 0.2, 2.8);
  - PRONE: 0.5, (0, 0, 3.0); PRONEZOOM: (0.4, 0.2, 2.8).
- 8 other sets:
  - Wookiee smuggler (+ snow): STAND centre 2.25, offset (0, 0, 4.0);
  - jet trooper and the Wookiee / Gungan locals: tilt 7.5;
  - droideka: (0, 0.5, 4.5);
  - Mace Windu and the Ewok / Tusken / Jawa locals: eye 2.0, offset (0, -0.25, 3.5), tilt 10;
  - the Geonosian, the snow stormtrooper, the jungle vanguard and the probe droid each have one.

## Acceptance
- The chase camera reads the player's class CAMERASECTION per stance (zoom sections when zoomed). BF2 reads it
  through the BASE chain (`com_inf_default`); check that BF2's result equals today's constants, or declare the move.
- One BF1 frame, viewed beside a PCSX2 state from `pcsx2/ref/states/` at the same pose: the eye 3.5 m back.
- A FEEL_NOTES line. Any moved golden declared and viewed; expected none, since goldens use fixed cameras (check).

## Notes
- TiltValue on the soldier camera is untraced ([H]: a pitch-down in degrees, as VSEAT applies it to seats).
  Keep it out unless the build RE's it or a PCSX2 pose pins it.
- CFOVM owns the FOV per mode; this card owns the distance and height.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.6): on BF2 the port's table IS `com_inf_default`'s, so 44 of the 49 fielded BF2
  classes match. The exceptions: the Wookiee (+ snow) stand centre 2.25 / offset 4.0, crouch 3.5, sprint 4.5, tilt 5;
  the MagnaGuard stand 4.0, crouch 3.5, sprint 5.0; the jet troopers stand 3.5, crouch 3.5, tilt 7.5. BF2's
  `com_inf_default` TiltValue is 10 (BF1 5).

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — the chase camera frames every shot of the video. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

- Note (cleanse CLNG, 2026-10-02): what the PS2's chase camera does on CONTACT with the world (pull in or slide, the
  spring rate, how it recovers, which `coll` chunks it tests) is not in `analysis/`; `game/src/camera_collide.rs` is a
  plain pull-in labelled PROVISIONAL and names this card as the owner of the question. Fold the measurement into this
  card's RE (a PCSX2 RAM hold against a wall: `ps2-camera-anywhere-by-ram-hold`), or split it off if it grows.

- Note from AIMA's RE (2026-10-02, `analysis/re-aim-assist.md` section 10): the soldier camera is read now.
  - **TiltValue is a pitch-down of the camera against the AIM** (BF1 5 deg, BF2 10 deg; zoom sections 3.5; BF2 prone 5;
    x 0.4 in split screen), and the aim ray and the reticle hang on it. AIMA's build brings the tilt and a per-game
    stance table (BF1 stand 3.5 m - this card's finding - crouch (0, 0.15, 3.5), prone 3.0; BF2 as today).
  - What stays here: the per-class exceptions (Wookiee, droideka, jet trooper, locals - read each class's
    CameraSection through the BASE chain); the zoom sections (TrackOffset (0.4, 0.05 / 0.2, 2.8): over the right
    shoulder); BF1's `split*` sections; BF2's blend between two sections during a stance change (B2 FUN_001cf738).
  - **The PS2's collision rule** (section 10.6, B1 FUN_001b58f0 [V]) against `game/src/camera_collide.rs`: five rays from
    feet + (0, 1.5, 0) to the eye (the centre and the near plane's four corners), buildings and terrain, ordnance
    bodies; the smallest fraction is taken at once, a larger one is approached at 0.75 per second and only past a 0.15
    band; the eye is origin + (eye - origin) * fraction.
  - Smoothing: AimTension (BF1 30, BF2 none) on the orientation, MoveTension 15 per axis on the eye, both
    exp(-tension * dt); the camera snaps when the track centre jumps more than 80 m.

## Note from ANIMA (2026-10-06)
- At about 30 deg of up-pitch the PS2's BF1 chase camera keeps the soldier mid-frame; the port's has him at the
  bottom edge (`engine/artifacts/lead_tmp/anima/pair_aim_up.png`, row 1 against rows 2-3; PS2 pose in
  `lead_tmp/anima/ps2/mid_idle.json`). The port's pitch there was set by eye, so register the pitch first.
- 2026-10-07 planner, from GRNP's report: **the PS2's chase camera looks 10.65 deg below the aim, the port's 10.0** (measured on BF2 Mos Eisley beside 15 grenade throws, `lead_tmp/grnp/ps2/out/`). Not carded on its own; check it with this card's stance tables.
- 2026-10-07 (card ZOOM): two camera items for this card. (1) The zoom out: the PS2 eases the eye back by exp(-15 dt) (the stance section's MoveTension; BF1's tilt by AimTension 30), the port snaps (`analysis/re-zoom.md` section 5 [M]). (2) Camera collision differs at a wall: on BF1 NAB2 with the soldier at (-45.34, -50.04, -9.40), yaw 152.45 deg, the PS2 pulled its unzoomed camera in 1.19 m (eye 2.26 m back, 2.19 m up, against the ideal 3.457 / 2.348) and the port's stayed at the ideal place; zoomed, both stand at the ZOOM section. Frames: `engine/artifacts/lead_tmp/zoom/pair/zoom_pair_bf1_nab2.png` (left PS2, middle the port's walk replay), rows `lead_tmp/zoom/ps2/pair/ps2_bf1_nab2_rows.json`.

## Maintainer decision (playtest 2026-10-28, branch camfix)
The default chase camera is now BF2's on every map, BF1 included (`core::unit::camera_section`, `walk::chase_fov_y`; the PS2 table stays as `ps2_camera_section`), a named deviation: "standardize the bf2 camera angles and zoom and everything for default. All maps should appear the same way". Matching BF1's own stance table or 37.69 deg FOV is no longer a goal of this card; the per-class sections would sit on top of BF2's default. Camera collision is ours too: `analysis/camera-occlusion.md`.
