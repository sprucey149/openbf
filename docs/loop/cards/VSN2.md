---
id: VSN2
title: Vehicle sounds 2: walker footsteps and the leg hydraulics (per foot, by the PS2's height rule), turret whir (two looping voices by turn rate), the boarding sound (TurretActivateSound, BF2 also on leaving), the collision sound, the kaadu's and tauntaun's cries on mount / dismount - the PS2 starts 269 walker steps in 105 s on Utapau, the port none
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card VSND builds the seat guns and the engine loop; these are the other vehicle sounds the PS2 makes
(`analysis/audio-census.md` 2-3: NAB1 88 kaadu steps + 34 turret / boarding / collision starts in 105 s, UTA1 269
walker steps + 52). The port plays none. RE: `analysis/re-vehicle-sounds.md` sections 7-9 (Docs 3d2fa0f).

## Evidence
- **Steps** (doc 7): every non-idle update, per foot prim of the class's foot list: h = the prim's underside minus
  the walker's own y. Type 0 (AT-ST, AT-RT, kaadu, tauntaun): a step when h comes down through `low + 0.05` (BF1) /
  `low + StompThreshold` (BF2, 0.15; tauntaun 0.5), low = the lowest h that foot has had. Type 1 (spider, AT-AT,
  AT-TE): a latch on the descent per update (0.1 m). `FootstepSound<i>` is a one-shot at (foot x, walker y, foot z);
  i = the foot's index in the list - BF1 the `TerrainCollision` lines (kaadu / tauntaun: 1 left, 2 right; 0 is the
  body), BF2 the `TerrainLeft` / `TerrainRight` lines. `HydraulicSound` (`atst_leg_up`): once per upswing above
  `HydraulicSoundHeight` (AT-ST 0.2), at the foot. PS2 rates: one AT-RT's steps 0.33 s (54 of 123) or 0.5 s (28)
  apart, median 0.334; kaadu 0.33 s (45 of 59).
- **Turret whir** (doc 8.2): per turret seat two looping voices (`TurretYawSound`, `TurretPitchSound`): started
  when the axis angle changed this update (BF1 at 0.01 rad resolution, BF2 0.001); r = |rate| / max rate; pitch =
  ((1 - r) x `Turret*SoundPitch` + r) x the property's pitch, gain = r x the property's gain; unchanged angle ->
  gain x0.6 per update, stop below 0.0001. `Turret*SoundPitch` 0.7 on every BF1 class (BF2 0.7 / 0.6), default 0.8.
- **Boarding** (doc 8.3-8.4): `TurretActivateSound` (`vehicle_equip`) one-shot at the vehicle when a `TURRETn`
  seat (or a building turret) gets an occupant, AI too; `TurretDeactivateSound` when it empties (BF2 classes only).
  The driver's own seat has no such key. PS2 log: `vehicle_equip` 0.03 s after the player takes a gun turret.
- **Collision** (doc 9): `VehicleCollisionSound` one-shot at the vehicle - hover: where collision damage is dealt;
  walker: a closing contact with another vehicle. The sound's `PlayInterval` 0.1 s limits it.
- **Creature cries** (doc 3.3, 3.5, 4): `TurnOnSound` when the pilot seat is taken, `TurnOffSound` `TurnOffTime`
  seconds (3.0 default) after it empties - the kaadu (`kaadu`), the tauntaun (`tauntaunstart` / `tauntaunstop`).
  The same turn-on / turn-off as VSND's engine, for a class with no `EngineSound`.

## Do
1. First: where the port has a walker's foot heights. If the sim does not pose the legs, take them from the
   baked walk clips (the foot node's height over the clip; `re-walker.md`) - a per-clip table of step and upswing
   phases made at load, checked against the PS2's 0.33 / 0.5 s intervals.
2. Steps + hydraulics, the whir (needs VSND's loop handle), activate / deactivate on seat changes, the collision
   one-shot on the hover's collision damage and the walker's vehicle contact, the creatures' cries.
3. `SOUND_LOG` rows for each.

## Acceptance
- `SOUND_LOG` on bf2_uta1 (headless replay, AI AT-RTs walking): `com_weap_layered_at_step` gaps between one
  walker's consecutive steps - median in 0.25-0.55 s (PS2 0.334) and steps per walker-second within 2x of the
  PS2's; `atst_leg_up` starts / step starts within 2x of the PS2's 104 / 136.
- bf1_nab1: `kaadustepleft` and `kaadustepright` alternate; gap 0.33 s +- 0.1.
- `vehicle_equip` starts = turret-seat boardings of the run (BF2: + leavings); none for a driver's seat.
- Unit tests of the whir with the PS2's constants: full rate -> gain = property gain, pitch = base; rest -> x0.6 per
  update, stopped below 0.0001; `Turret*SoundPitch` 0.7 -> pitch 0.7 at r = 0. Mutation-check one.
- Collision: one sound per hover collision-damage event, never two within 0.1 s.

## Notes
- Not here: the jet pack's and the droideka's engine and their `TurnOnSound` / `TurnOffSound` (note on USND); the
  droideka's three leg sounds (doc 7.6); `TurretAmbientSound` (4 BF1 classes, `droid_fx7_hum` - AMBM's prop loops);
  flyers.
- Not settled (doc 13): whether the AT-TE's rear feet are silent on the PS2 (it authors slots 0, 1, 4, 5); which
  seats set a turret object's pilot beyond the gun turret seen live. A PS2 check first if either matters.

- Note from VSND's build (2026-10-03, landed 79eac42): the pieces this card reuses are in. `audio::Voice` is the looping voice with settable gain / rate / pan (stops when dropped; `Audio::play_voice`) - the turret whir's two voices and card WHUM's hum use it; `SoundLib::loop_voices` shows the pattern (a voice per key, started in range, set over one tick, dropped when not listed). `game::engine_sound::EngineSounds::tick` already knows each vehicle's turn-on and turn-off edges (`Instance::on` / `turning_off`): `TurnOnSound` / `TurnOffSound` hang there. `SOUND_LOG` has `engine` / `engines` rows; `lead_tmp/vsnd/port/engine_referee.py <csv>` reads them. Sub-levels: a record's definition differs between a planet's sub-levels (NAB's fighter tank) - look anything new up through `game::foley::sublevels`, and bake it per sub-level (`mungefs::sound_props::find_named_sublevels`).
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
