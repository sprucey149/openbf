# The chase camera near walls, props and droids — design (ours, 2026-10-28)

The playtest of 2026-10-28 reported two camera problems:

1. "Standing near these things to heal up the camera would do weird things like go to the floor looking at my feet
   [...] I couldn't heal next to a droid and stand guard because I couldn't see anything [...] Could the camera go
   through walls or something and when on the wrong side of walls or obstacles or droids or something they become
   invisible? What do modern games do with the issue?"
2. "The bf1 default camera seems to be cutting off the players units feet a little. [...] Let's standardize the bf2
   camera angles and zoom and everything for default. All maps should appear the same way."

This note is the design the port took for both (branch `camfix`), the reasons, and what stays provisional. The PS2's
own camera rule is `re-aim-assist.md` §10.1 / §10.6; everything below marked **ours** is a named deviation from it.

## 1. What third-person games do

A third-person camera sits on a boom ("spring arm") from a pivot on the character to the wanted camera place. The
common pieces, in engines and shipped games since the PS3 era:

- **Pull in only against world geometry.** The boom is swept with a small sphere from the pivot toward the wanted
  place, and the camera stops short of the first blocking surface (Unreal's spring arm: a 12 cm probe on the camera
  collision channel; Unity Cinemachine's collider "pull camera forward"). Characters, vehicles and small dynamic
  props are usually on a channel the camera ignores, so they never shove the camera about.
- **The pivot is at the head or shoulder, not the feet,** and the pulled-in camera keeps its look direction. A camera
  pulled toward the feet ends up looking at the floor. Cinemachine has "preserve camera height" and a minimum
  distance from the target for the same reason.
- **What stands between the camera and the character becomes see-through.** Instead of pulling in, small occluders
  (props, foliage, other characters, vehicles) are faded: a screen-door dither (Fortnite, Assassin's Creed, The
  Witcher 3), a silhouette through walls (Super Mario 3D World and many action games), or hidden outright when the
  camera is inside them. Cinemachine's collider has an "ignore tag" and "transparent layers" for this.
- **The character fades when the camera comes very close.** Below about a metre the body only blocks the view, so
  games fade or hide it (Gears of War, Red Dead Redemption 2, Fortnite) and the view keeps the aim.
- **Ease out slowly, snap in at once,** so a pillar passing by does not make the camera jump back and forth. The PS2
  does exactly this (B1 `FUN_0019fd70`, `camera_collide::ease_pull`).

## 2. The port's rule (`game::camera_collide`, `WalkSim::settle_camera`, `WalkSim::camera_see_through`)

- **Big world geometry pulls the camera in.** A map instance whose model is more than `PROP_MAX_FOOTPRINT` = 4 m
  across (the larger of its two horizontal extents at its instance scale) is swept with a 0.35 m sphere along the
  track, as before (walls, buildings, rocks, doors, the terrain sheet). Measured on BF1 NAB1 / YAV2 and BF2 KAS2 /
  NAB2: walls (4.5 m and up), doors, buildings, rocks over 4 m and bunkers stay solid. A door is tested where it is
  drawn: the submeshes its clip moves ride the sim door's `open`, so an open doorway is clear and a shut door still pulls
  (BF2 Death Star, 2026-11-01: testing the shut model put the camera in the trooper in every doorway).
- **Small props and every vehicle never pull it.** Health / ammo / vehicle droids (1.4-1.9 m), gun emplacements
  (2.4-3.1 m), columns, lamps, small trees and every vehicle hull are drawn see-through while they stand in the
  camera's sight of the player: a sphere of 0.15 m swept from the camera toward his eye point and his middle (1.0 m),
  stopping 0.3 m short of them. In the way, a 30 % ordered dither (`render::Fade::see_through`, the foliage's
  screen-door in the same pass: no blend sort, no new pipeline); with the camera inside its box, hidden. They stay
  solid for the sim (collision, shots, the aim ray): only the draw changes. A droid is tested at the pose it is drawn
  at; a gunner's own building and a rider's own vehicle are left alone. Soldiers neither pull nor fade.
- **A wall keeps the view.** The soldier's track runs from the PS2's own collision origin, 1.5 m above the feet in
  every stance (the section's fixed point (0, 1.5, 0), §10.1 / §10.6 step 4), to the ideal camera. The pulled-in
  camera keeps the ideal's pitch and yaw (the look along the aim, where the reticle points) and never goes below that
  height, so it cannot drop to the floor or turn to the feet. Before, the track ran to a 1.0 m body point (issue
  #36), and a health droid behind the player pulled the camera down there.
- **Very close, the body goes.** Within `BODY_HIDE_DIST` = 1.2 m of the pull point the player's body is hidden and
  the view is his aim's (at 1.2 m the 52° view spans 1.2 m top to bottom: a 2 m soldier cannot be framed there).
  The PS2 ghosts the trooper instead (`re-muzzle-flash.md` §7).
- Unchanged: the gallery sheets' fixed framing (`CameraBlockers::pull_in` still stops at props: it draws nothing
  see-through), the PS2's in-at-once / out-at-0.75-per-second easing, the terrain sheet (pulled short of it, kept 0.3 m
  off it on the side the camera is on), the droid camera's own target. A seat camera's track runs from the section's point on
  its frame, as on the PS2: 0.9 of the model's vis box top over a hover or flyer, 0.8 over a walker (written when the
  class reads `GeometryName`), and the fixed (0, 1.5, 0) for a gunner's own turret and a building turret
  (`walk::vehicle_pull_height`, issue #71). [ours] A pilot's point stops short of any wall or terrain between it and the fixed point, so a tall model's point cannot
  put the camera past a hangar's sheet or a wall beside a banked flyer (`WalkSim::pull_point_on_own_side`). The droid camera's own target is still the port's: the PS2's droid section
  has (0, 0.15, 0), B1 `FUN_001cb208`, B2 `FUN_001f0cf8`.

The healing case: the player stands with his back against a health droid, the camera behind him in or behind the
droid. The camera stays at its normal place (3.2 m back, 2.57 m up at the spawn pitch), looking where he aims, and
the droid is dithered (between) or hidden (camera inside it).

## 3. Why BF1 maps framed the soldier differently

The PS2 uses each game's own soldier camera (`re-aim-assist.md` §10.1, `analysis/re-zoom.md` §5):

| | TrackOffset (stand) | TiltValue | vertical FOV (third person) |
|---|---|---|---|
| BF1 | (0, 0, 3.5) | 5° | 37.69° (`ThirdPersonFOV` 48 × 0.7666) |
| BF2 | (0, 0, 3.2) | 10° | 52.06° (65 × 0.7666) |

At the spawn pitch (−4.01°) BF1's camera looks 9.01° down from 2.35 m up, 3.46 m back: the frame's bottom edge is
9.01 + 18.85 = 27.9° under the horizon while the feet are 34.2° under it, so everything below about 0.5 m (the
shins and boots) is cut off. BF2's looks 14.01° down from 2.57 m, 3.10 m back: bottom edge 40.0°, feet 39.7°, just
inside. The port keeps the PS2's vertical FOV and widens the horizontal with the window (Hor+), so the narrow BF1
FOV showed as a zoomed-in, feet-cut framing.

**Ours:** every map plays BF2's default chase camera — `core::unit::camera_section` is BF2's table for every
stance (stand, crouch, prone, sprint and the zoom sections, which were already alike), `walk::chase_fov_y` /
`chase_fovs` give BF2's 52.06° and its 65° / 70° zoom FOVs. The PS2 records stay as `ps2_camera_section`,
`ps2_fov_y` and `ps2_soldier_fovs`. The tilt moves the aim ray and the reticle with it (BF1 maps now draw the
reticle on BF2's line 153.35 of 480, and the aim origin is 2.354 m up), as on BF2. The seat cameras take the chase
FOV (card VSEAT), so BF1's vehicles widen with it; their placement is each seat's own and did not show the problem.
Not changed: the scope view (`eye_back`: BF1 0.3 m, BF2 0.6 m), the per-class camera sections of card CAMS
(not read by the port), split screen's 0.4 tilt factor.

## 4. Provisional numbers and checks

PROVISIONAL (ours): `PROP_MAX_FOOTPRINT` 4 m, `SEE_THROUGH_OPACITY` 0.3, `SIGHT_RADIUS` 0.15 m, `SIGHT_END_GAP`
0.3 m, `BODY_HIDE_DIST` 1.2 m. The 1.5 m pull point is the PS2's value [V].

Tests: `game/tests/suite/camera_clip.rs` (the healing droid on BF1 Yavin 4: Arena, 1057 placements in the droid's
way and 188 with the camera inside it; walls that pull in keeping pitch and height; a gun emplacement and a vehicle
seen through and never pulling; no jitter), `game/src/camera_collide.rs` unit tests, `walk::tests::
bf1_and_bf2_maps_frame_the_soldier_alike`. Stills: `cargo run -p game --example camera_stills -- <dir> [prefix]`
(before / after pairs were rendered for the branch's review under `engine/artifacts/lead_tmp/camfix/`).
