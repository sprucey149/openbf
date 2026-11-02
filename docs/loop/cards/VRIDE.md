---
id: VRIDE
title: A vehicle's riders are drawn in their seats, as the PS2 draws them: a hover pilot, a speeder bike's rider, a walker's exposed pilot, a gunner on an open vehicle turret - the seat's pose key (PilotAnimation / Pilot9Pose / AnimatedPilotPosition), hung at its PilotPosition on the vehicle's posed node, in the seat clip, hidden on BF2 IsPilotExposed = 0; the port draws no rider at all (card VTURG built it for building turrets only)
milestone: M2
value: 5
phase: build
status: done
deps: VTURG
files: engine/crates/bake/src/soldier.rs,engine/crates/bake/src/hardpoints.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/vehicle_class.rs,engine/crates/game/src/soldier_vis.rs,engine/crates/game/src/viewer/turret_pose.rs,engine/crates/game/src/viewer/walker_pose.rs
declares: rebake=side,maps,tat1
needs:
effort:
branch: vride
resume: landed b054f579 2026-10-15
created: 2026-10-15
alpha: yes
---
## Build done (2026-10-15 lane 2, branch vride cba07db + 41782eb) - waiting only for the dGPU to land
Acceptance, each run (`cargo test -p game --test vehicle_rider` on the scratch overlay bake):
- Census: every baked vehicle seat's verdict = the PS2 census (BF1 72 seats: pilots 13 drawn / 24 hidden / 5 none,
  gunners 12 / 18; BF2 106: pilots 5 / 41 / 10, gunners 6 / 31 / 13; only BF1's gian speeder and BF2's kaadu are in
  no baked db). Found on the way: a walker TURRET section's seat keys fell onto the walker's pilot (the AT-TE pilot
  read DRAWN) - the pose keys now stay (B2:191723-191731); the rest is card WSEAT.
- Placement vs RAM, within 1 cm: bike riders (0, 0.0002, 0) both games; Jedi starfighter pilot (0, 0.0580, -1.1617) up
  (0, 0.9296, -0.3685); fighter-tank gunners (0, 0.6581, -3.0103) both games; STAP (0, 0.360, 0); one-man AT-ST rider
  on the walker skin's bone_root at the PS2's walking tilt (0, 0.991, 0.136).
- Nine-pose replay: BF1 244 / BF2 258 of 276 consecutive updates within 2e-4; a/b/c on 282 of 282 rows.
- Mutations, each fails one test: hide every rider; drop the hardpoint rotation; animated rider at the bind; BF2
  0.5 -> 1.0.
- Pictures: `lead_tmp/vride/pics/b1bike_pair.png` (PS2 beside port at the PS2's camera and bike matrix: rest matches;
  the turning frame's RAM camera lags the bike), `b1bike_left_pair.png` (the lean against its 0-input control).
- PS2 kaadu frame (`lead_tmp/vride/ps2/kaadu_pair01.png`): the rider sits on the saddle - the constant DummyRoot of
  `human_ride` is not drawn (re-vehicle-riders 4.3 now [L]); the port's bake rule is the PS2's.

## Why
The PS2 shows the riders of its vehicles: the speeder bike's rider, the hover tanks' exposed pilots, the AT-ST's? (to read), gunners on open vehicle turrets. The port hides every rider (`game/src/soldier_vis.rs` `seat_now`: building turrets only), so a moving bike has nobody on it - seen in a minute of play. Split from card VTURG (landed bc822ba), whose seat draw is the same PS2 function (analysis/re-turret-draw.md 3: BF1 FUN_0020c880, BF2 FUN_0024de60; the vehicles' callers are 7 [H]).

## RE done (2026-10-15, lane 2): `analysis/re-vehicle-riders.md` [O] (commit efa2d56), the build spec is its section 6
- One seat function places every rider (B1 FUN_0023e028, B2 FUN_002f74c0), called by each vehicle kind's own update
  with the vehicle's world matrix O and its pose table: `L x E x O` (L = the PilotPosition hardpoint's bind relative to
  its parent, E = the parent's matrix now); `AnimatedPilotPosition` = the named node's own matrix now (`E x O`): the
  tauntaun's / kaadu's / BF2 one-man AT-ST's animated bone. Hidden: no pose key, BF2 IsPilotExposed = 0.
- Census (lead_tmp/vride/vehcensus_<g>.txt, hangs_<g>.txt): drawn = speeder bikes (speederbike_9pose at hp_active, the
  bike's origin), STAP, BARC (BF2), tauntaun, kaadu, one-man AT-ST (BF2), fighter-tank GUNNERS (minigun_9pose under
  turret_y), the AT-TE main gunner, the skiff, BF1's combat speeder / landspeeder / gian speeder / fighters' pilots; the
  hover tanks' PILOTS, AT-STs, AATs are hidden (no pose key). "A hover tank's pilot" in the old acceptance = its gunner.
- A hover's nine-pose rider: p (throttle) and y (turn) smoothed at rate 5, a = y + 4, b = y + (p > 0 ? 1 : 7), c = |p|
  (BF1 clamp(trn - str) after the strafe-into-turn remix when the class has no StrafeSpeed, BF2 clamp(trn + 0.5 str)).
  Other seats play their clip at 30 fps, looping (the port's `Animator::seat` already does).
- PS2 frames of the bike rider, both games: lead_tmp/vride/ps2/b1bike_sheet.png, b2bike_sheet.png (+ the 12 states).
- New card VPOSE (value 4): the vehicle's own posed nodes (turret nodes, FinAnimation); until it lands a turret-node
  gunner hangs at the rest pose, as the rigid gun.

## Evidence
- re-turret-draw.md 3.1 / 3.3 [O]: the seat slot draws the soldier when the seat has a pose key; BF2 hides on IsPilotExposed = 0, authored on flyers' turret seats only (gunships, the snowspeeder's gunner, the cloud car); census of vehicle classes in lead_tmp/vturd/census2_bf1_veh.txt / census2_bf2_veh.txt.
- Clips: lead_tmp/vturd/clipcensus.txt lists every vehicle seat clip of both discs (human_ride 61 frames, human_speederbike_9pose, human_stap_ride, human_drivesnowspeeder, human_barc_9pose, human_drive_1manatst, ...): `bake::soldier::SEAT_CLIPS` holds only the turrets' three.
- Not read: which node a vehicle seat hangs from (a vehicle has no TurretNodeName pose in the port: it draws rigid), `AnimatedPilotPosition`, what a played clip (human_ride, 61 frames) is timed by.
