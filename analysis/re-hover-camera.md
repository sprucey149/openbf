# OPUS RE (2026-11-12, vehicle parity: the IFT-X) - [O] UNAUDITED
# RE: a hover's seat cameras - the pilot's chase camera and look, the gunner's turret camera, the seat FOV

**The questions.** On BF2 Kashyyyk the port's fighter tank (`rep_hover_fightertank`, the IFT-X) is seen from a
different place than the PS2's, cannot look down, does not raise its guns when the pilot looks up, and its gunner's
camera sits inside the model. (1) Where the PS2 puts a hover pilot's camera and what it looks along. (2) How the
pilot's look and his guns' aim differ. (3) Where a hover gunner's camera is. (4) Which FOV a seat draws with.

**Method.** The decomp (B2 = the BF2 disc's, B1 = BF1's) read for the hover's camera update and its FOV, then two PS2
save states the user made on Kashyyyk (slot 8: riding the IFT-X as pilot; slot 9: in its turret seat), read from
their `eeMemory.bin` without touching the running emulator: the hover entity, its class's sections, the global
camera matrix and the projection. Every number below reproduces those states to the printed digits.

## 1. The entity and the camera in RAM [L]
- The IFT-X entity E = 0x1d171a0 (vtable 0x61cc70) in both states: rows right / up / forward / position at E+0x120,
  driver E+0x330, class E+0x4f4, the tilt E+0x544 / E+0x548, the pilot's aim yaw E+0x554 and pitch E+0x558, the eye
  E+0x340, the aim direction E+0x350, the nine-pose numbers E+0x1cb0.
- The camera: the global matrix at 0x700c30 (copy at 0x17399b0), rows right, up, back, position; the projection at
  0x17399b0+0x80 with x scale 2.2460 and y scale 2.9299: tan(vfov / 2) = 0.3413, a vertical FOV of 37.7 degrees =
  `2 atan(tan 24 x 0.7666)`, the class's `ThirdPersonFOV` 48 (the default) as a horizontal FOV on the 4:3 screen.
  The soldier chase FOV the port drew every seat with (52 degrees vertical) is not a seat's.

## 2. The pilot [O] [L]
- [O] The hover camera, B2 `FUN_00220d68` (twin B1 `FUN_001b58f0`, B1:100734): its frame is E's rows with the drive's
  roll and pitch tilt taken back out (E+0x544 / E+0x548); it calls the shared chase law `FUN_001cef68` with the BODY
  section (K+0xc00) and the aim direction E+0x350. The zoom branch is the first-person eye at E+0x340. The lift by
  `SuspensionNodeName` (K+0xe2c, the hailfire only) is not ported.
- [L] Slot 8: the camera at (0, 2.075, -11.02) in the hover's untilted frame, looking 7 degrees below the level aim.
  That is the chase law with center `TrackCenter` (0, 2, -5) in the hover's frame, `TrackOffset` (0, 0.5, 6) in the
  view's frame (x right, y up, z back), the view the aim direction tilted down by `TiltValue` 7.
- [O] The look, re-hover 5.1: the stick adds `pit x PitchRate x 30 x dt^2` to the aim pitch (I8, `rate` radians a
  second at 30 Hz); each update takes the body's own pitch change back out, `aimPitch -= oldUp . E'.fwd`
  (B2:131896-131935, I10), so the aim stays put in the world while the body pitches; then it clamps to
  `PitchLimits` (I11; the IFT-X -8..25, yaw 0 / 0).
- [L] Slot 8's aim pitch 0.1934 rad equals the body's pitch-up there: the aim is level in the world, as I10 keeps it.
- [O] The guns: each weapon's aimer chases the aim point within its own `AimerPitchLimits` / `AimerYawLimits`
  (`lgun_x` pitch -30..90, `lgun_y` yaw -30..15, likewise `rgun_*`), not within the look's `PitchLimits`; the port
  had one limit for both, so the guns could not follow the look up.
- [O] The FOV, B2 `FUN_00221388`: with vF = velocity . E's forward row, when vF > `ForwardSpeed`, `BoostSpeed` >
  `ForwardSpeed` and `BoostFOV` (K+0xf74) != 0, the FOV is `lerp(ThirdPersonFOV, BoostFOV, clamp((vF - FS) / (BS -
  FS)))`; else `ThirdPersonFOV`. The IFT-X: FS 9, BS 20, BoostFOV 60.

## 3. The gunner [O] [L]
- [O] Every TURRET section, on a building or a vehicle, is a MountedTurret; its camera (B2:190496) is the same chase
  law `FUN_001cef68` on the turret's aim frame T+0x1c0 with its aim direction T+0x1e0.
- [L] Slot 9: the camera at (0, 2.889, -5.858) in the hover's frame, looking 2.8 degrees below the body. That is the
  `turret_y` node at (0, 1.7932, -2.8576), `TrackCenter` (0, 1, -1) on the aim frame, `TrackOffset` (0, 0, 2),
  `TiltValue` 3; the same 37.7 degree FOV.
- [L] The gunner is drawn at `hp_active` (0, 0.658, -3.010) under the posed `turret_y` (re-vehicle-pose 3,
  re-vehicle-riders 3).

## 4. For the port
- `game/src/walk/flyer_camera.rs`: a hover pilot uses the flyer chase law with `FlyerSection::hover` (BF2 tilt from
  `TiltValue`, no slow / fast offsets, the BoostFOV fraction), on `hover::untilted_rot`, along the pilot's aim; a
  hover gunner seat is a MountedTurret (`flyer_gunner`, `vehicle_pads::turret_law` for every gunner seat).
- `game/src/walk/walker_view.rs` `seat_fovs`: every seat draws with its section's `ThirdPersonFOV` /
  `FirstPersonFOV`, not the soldier's.
- The pilot's aim ray leaves the hover's eye E+0x340, its matrix on the BODY section's `EyePointOffset` from the
  model's origin (B2:132251, `FUN_001ce880`); the port had measured it from the gun's pivot, the IFT-X's right
  cannon (`vehicle_pads` sets `GunSetup::eye` from the origin for a hover pilot; zoomed, the scope eye still turns
  with the look).
- `core/src/sim/vehicles.rs` `hold_pilot_aim` (I10, I11) and `core/src/vehicle/gun.rs` `GunSetup::weapon_aimer`: the
  pilot's look keeps `PitchLimits`, his guns turn within the first aimer's limits.
- Card VPOSE's hover posing (`viewer/walker_pose.rs` `hover_palette` / `hover_nodes`, `turret_pose::HoverClocks`):
  the fins from the nine-pose numbers, the turret and aimer nodes from the seats, riders on the posed nodes.
- A gunner's full-circle yaw number is the gun's own slewed rate over its turn speed (re-turret-draw 3.3), carried
  through the frame blend with the look (`render_blend::VehiclePose::looks`); a seat without a rate, and an
  aimer-node hang's one-update lag, read one tick back along the drawn frames (`soldier_vis::SeatClock`). Sampling
  either once per tick made the gunner lean in 30 Hz steps under a smooth turret (`examples/seat_jitter.rs`). The PS2
  capture's lean restarting whenever the stick stops is not reproduced.

## 5. Still [H] / not read
- The 16:9 port widens only the horizontal FOV; the vertical one is the PS2's.
- The hailfire's `SuspensionNodeName` lift; the zoom (first-person) branch is not checked in RAM.
- One aim per seat: the PS2 runs one aimer per weapon from its own pivot (re-vehicle-pose 7.2, PROVISIONAL).
- The beam's `BeamGlowWidth` fall-through (section 6) and BF2's glow blend mode 0 are read from the decomp only, not
  the instruction bytes (no extracted ELF in this checkout).

## 6. The gunner's beam: an OrdnanceBeam draws its own ribbons [O]
The IFT-X turret's `rep_weap_hover_fightertank_laser` fires `com_weap_veh_beam_ord` (label `beam`), not a bolt. The port
drew it with the bolt law (tapered core w -> w/2, additive glow 2w -> 1.6w) and padded its length; the PS2 draws a beam
with its own code, and spawns its impact effect itself.
- [V] Setter, B2 `FUN_00312838` (B2:291437-291560): `LaserWidth` (0x6f179ab8) -> C+0xc0 and **C+0xcc = 2.5 x it**;
  `LaserTexture` -> C+0xc4; `BeamColor` (0xa3093acd) -> C+0xc8 (core colour); `LaserGlowColor` -> C+0xd4;
  `BeamGlowWidth` (0x398b9ad9) -> C+0xcc, and in the decomp the same case then falls into the `BeamGlowTexture`
  (0xc6bfc336) case, naming the glow texture C+0xd0 after its own value [not asm-checked; no class on either disc
  authors it]; `Range` (0xfadc0cd2) -> C+0xbc; `PassThrough`-style flag 0x604e2441 -> C+0xd8 bit 0; `ImpactEffect`
  (0x11727a92) -> C+0xdc (an effect lookup); everything else -> the ordnance base `FUN_00310620`, never the bullet
  setter (no light, no lengths, no per-surface impacts: the base reads no `ImpactEffectWater`). Ctor `FUN_003126c0` (B2:291369-291392):
  Range 150, the colours the static white, C+0xd0 = `lightglow` (0x716bd81a).
- [V] B1 `FUN_00275bd0` (B1:196986-197090): `LaserWidth` +0x88, `LaserTexture` +0x8c, `LaserGlowColor` +0x90,
  `ImpactEffect` +0x94, `Range` +0x84; no BeamColor / glow keys. Ctor `FUN_00275ad0`: glow colour white.
- [V] Draw, B2 `FUN_00312438` (B2:291272-291366): H = inst+0x50 (the muzzle), P = H + dir (inst+0x100) x len
  (inst+0x110); core: texture C+0xc4 with `FUN_00594270(tex, 0)`, colour C+0xc8, ribbon modes 4 / 5 both at size C+0xc0;
  glow: texture C+0xd0 with `FUN_00594270(tex, 0)`, colour C+0xd4, both at C+0xcc. B1 `FUN_002758e0`
  (B1:196842-196931): the same from inst+0xc0 / +0xd0 / +0xe0, the core in the static white with mode 0, the glow
  `lightglow` with `FUN_004afac0(lightglow, 1)` at 2.5 x LaserWidth. So: **no taper, no fade, no length padding**; the
  glow 2.5 w wide, ADDITIVE on BF1 and NORMAL-blended on BF2 (blend mode 0 is the bolt core's, 1 the bolt glow's:
  re-bolt 3.1/3.3). The IFT-X beam: w 0.4, glow 1.0 m, (92, 136, 250, 100) - a wide, solid blue band.
- [V] Impact, B2 `FUN_00311d00` (B2:291073-291081), B1 (B1:196819-196831): on each update, at the end of each ray
  segment that struck something, the class's `ImpactEffect` (`com_sfx_weap_beam_exp`) is spawned. The port's loader
  gave beams no impact effect.
- Build: `core::laser::LaserLook::replay_beam` / `BeamLook`, `game::bolt_vis::beam_draw`, `render::BeamRibbons`,
  `core::impact::ImpactLook::beam`.
