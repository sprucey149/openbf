---
id: VTURG
title: An open building turret shows its gunner (the maintainer's INBOX 2026-10-12 18:55, ask 1b / 1c): a soldier seated in a turret whose seat has a pose key (PilotAnimation / Pilot9Pose) is drawn in his own team and class skin at the seat's hardpoint carried by the posed turret or aimer node, in the seat clip human_<value> (nine frames blended by the turret's yaw and pitch fractions, or the one-frame drive pose), without his hand weapon; a seat with no pose key - Hoth's laser towers - draws nobody, as the PS2 (RE done 2026-10-13, analysis/re-turret-draw.md 3)
milestone: M2
value: 5
phase: build
status: done
deps: VTURD
files: engine/crates/bake/src/soldier.rs,engine/crates/game/src/animator.rs,engine/crates/game/src/soldier_vis.rs,engine/crates/game/src/vehicle_class.rs,engine/crates/game/src/viewer/turret_pose.rs,engine/crates/game/src/main/headless.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/examples/pose_render.rs,engine/crates/game/tests/turret_gunner.rs
declares: rebake=side
needs:
effort:
branch: vturg
resume: landed bc822ba2 2026-10-15
created: 2026-10-13
alpha: yes
---
## Why
The maintainer's INBOX 2026-10-12 18:55, ask 1 (b) and (c): "an OPEN turret shows its occupant: the seated soldier model in the
seat's pose, his team and class skin, while someone is in the seat (player or bot); gone when the seat is empty" and
"NOT for the turrets you enter: an enclosed seat (Hoth's white turrets) draws no occupant, as the PS2". The port
draws no rider at all (`game/src/soldier_vis.rs`: "A rider is hidden"). Split from card VTURD by its RE
(2026-10-13, `analysis/re-turret-draw.md` 3 [O]).

## Evidence (the PS2's rule, read in both games and seen live)
- **Drawn or hidden**: BF1 draws the seated soldier when the seat has a pose key (`PilotAnimation` or `Pilot9Pose`)
  and takes him out of the scene list otherwise; BF2 the same and also hides him on `IsPilotExposed = 0` (authored
  on flyers' turret seats only - gunships, the snowspeeder's gunner, the cloud car). Census over every
  armed-building class of both discs (`lead_tmp/vturd/turcensus2.py`): BF1 6 drawn / 1 hidden / 2 no soldier, BF2
  15 / 7 / 47; the hidden one on a showcase map is Hoth's laser tower (`hoth_bldg_hoth_turret`,
  `tur_bldg_hoth_lasermortar`). In PS2 RAM the player's soldier is in the scene list (state 0x11) in Hoth's dish
  turret and out of it, with no body, in the laser tower.
- **Where**: `soldier = L x E x O` - the `PilotPosition` hardpoint's bind matrix relative to its PARENT node, that
  node's CURRENT matrix (VTURD's pose), the building's matrix. Parent = the turret node on the gun turrets, the dish
  and the towers (he swings with the yaw, stays upright); = the aimer node on the anti-aircraft guns (he swings and
  tilts with the barrels). Live: his position in the node's frame is the hardpoint's bind offset to 4 decimals
  (BF1 gun turret (0, -0.338, -0.152), BF2 dish (-0.003, -0.350, -1.880)).
- **Pose**: one clip `human_<value>` (BF1 bank `human_animation_1` in COMMON.LVL, BF2 `human_4` in INGAME.LVL;
  `lead_tmp/vturd/clipcensus.py`): `human_drive` is 1 frame; `human_minigun_9pose` and `human_dishcannon_9pose`
  have 9 frames and the pose is frame `a` blended toward frame `b` by `c`, with `a = yf + 4`,
  `b = yf + (pf > 0 ? 7 : 1)`, `c = |pf|` - pf the pitch's place in `PitchLimits` (-1 .. 1; the PS2's turret pitch
  is positive DOWN), yf the yaw's place in `YawLimits`, or the turn stick when the turret turns full circle.
- **No hand weapon** while seated; drawn in every view, his own included.
- Pictures: `lead_tmp/vturd/ps2/sheet_hot1_bots2_dish_zoom.png` (a bot at the dish turret, from outside: standing
  behind the dish, hands on the grips), `lead_tmp/vturd/sheet_ps2_cases.png` (the gunner's back in the Naboo gun
  turret's own view; the laser tower with nobody).

## Notes
- The vehicle seats use the same function on the PS2 (hover pilots, bike riders, walker pilots: `PilotAnimation`
  / `Pilot9Pose` / `AnimatedPilotPosition`); this card is the building turrets, the vehicles a later card.
- Not read on the PS2: which way the 9-pose rows look is derived from the numbers (frames 6-8 = the pitch at its
  upper limit = nose down); view the baked clip's nine frames before trusting it.
## Note from VTURD's build (2026-10-13, landed 1cf5c8b)
- The posed node matrices are there to hang the gunner on: `game::viewer::turret_pose::pose(rig, spec, look, aim,
  recoil)` gives every node's model-space matrix (the rig is `ManifestClass::turret_rig`, bind matrices and parents
  included, so `L` = `bind(parent)^-1 x bind(PilotPosition)`), and `SceneGpu::turrets` joins a sim turret to its
  building by the instance's translation. The PS2 uses the aimer node of the update BEFORE for an aimer-node seat.
- The seat look still stops at 180 degrees (card VTURL): a gunner on a full-circle turret will show it too.
- `pose_render` needs `POSE_COMMON=1` to draw BF1's gun turret (it is a COMMON-LVL class); pair tool
  `lead_tmp/vturd/tools/mk_pairs.py` (the port from a PS2 capture's own camera and turret angles).
