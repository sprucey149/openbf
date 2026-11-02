---
id: VTURD
title: A building turret turns with its aim (the maintainer's INBOX 2026-10-12 18:55, ask 1a): the TurretNodeName node turns with the seat's look, the AimerNodeName node looks along the aimer's own direction and the barrels slide back at a shot, for the player's turret and the bots', in his third-person seat view and from outside - drawn as rigid per-bone parts moved the way doors are (RE done 2026-10-13, analysis/re-turret-draw.md); the gunner is card VTURG, the wreck and the zoom card VTURX
milestone: M2
value: 5
phase: build
status: done
deps: VTURB
files: engine/crates/bake/src/map.rs,engine/crates/bake/src/bind_pose.rs,engine/crates/bake/src/hardpoints.rs,engine/crates/bake/src/turret_rig.rs,engine/crates/manifest/src/lib.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/game/src/viewer/turret_pose.rs,engine/crates/game/src/vehicle_class.rs,engine/crates/game/src/main/windowed.rs,engine/crates/game/src/main/headless.rs,engine/crates/game/examples/pose_render.rs,engine/crates/game/tests/turret_pose.rs
declares: rebake=tat2,maps,common
needs:
effort:
branch: vturd
resume: landed 1cf5c8be 2026-10-13
created: 2026-09-29
alpha: yes
---
## Why
VTURB keeps the world's static building model for a turret (no viewer geometry), so the gun never visibly turns when a player aims it, and a dead turret (once VHULL lets bolts hit hulls) stays intact.

## Evidence
Classes author TurretNodeName / AimerNodeName / BarrelNodeName (BF1 com_weap_gunturret main_cylinder / turret_main; YAV1 tower cube_y / gun / barrel; HOT1 turret_y / turret_x / lgun). uta1_bldg_turret authors DestroyedGeometryName uta1_bldg_turret_destroyed. RE needed: how the PS2 poses those nodes (the aimer B1 FUN_0026e260 writes angles; which node takes yaw vs pitch) and what a dead armed building does (removed like a vehicle §6, or its destroyed geometry like a building).

## Note (PXWA, 2026-09-29, lane 2)
BF1 BES1's building turrets stand barrel-level on the PS2 and in the port's bind pose (fins up):
`engine/artifacts/pcsx2/ref/bf1_bes1/walk/pairs/cp3_cp18__sheet.png` b270, `cp4_cp17__sheet.png` b180.
BF1 HOT1's tower turrets (`hoth_bldg_hoth_turret` / `_dish_turret`) carry their gun heads on the PS2; the port draws
the base only: `engine/artifacts/pcsx2/ref/bf1_hot1/walk/pairs/cp2_cp6__sheet.png` b180/b270.

## Note (VTUR2, 2026-09-30, lane 1) — why the Hoth tower's head is missing
The head is not missing; it is drawn at the tower's FOOT. `hoth_bldg_hoth_turret` (modl INFO bone_space 0) holds four
segments. The pillar spans y 0.01–3.17; the other three (the head, x/z ±1.56 and y −0.43..0.57; a yoke at y 0..0.77;
the barrel along z 0.46–2.36 at y≈0) are stored relative to their bone, not in model space. Its skel puts `turret_y`
at y 3.57 (the map manifest's hardpoints). The map bake writes them as stored, so they sit at ground level. The bounds
match in both eras' bakes: `assets_baked/bf1_hot1/models/hoth_bldg_hoth_turret.glb` on main, and
`lead_tmp/vtur2/bake/bf2_hot1/models/` (TUR.LVL's copy). BF2 sheet: `lead_tmp/vtur2/pose/sheet_hot1.png` (PS2 |
main | VTUR2): the towers stand where the PS2 draws them; the head is a dark lump at the base.
So `crate::vehicle`'s "a BNAM only says which bone would move them" does not hold for this model. Placing a
hard-skinned segment at its bone's bind pose in the map bake is the first step of this card's draw, and it fixes the
look even before the gun turns. BF2 HOT1's 12 conquest laser mortars now show it too (other TUR models not
checked), so a value of 4 is suggested.

## Note (AUDIT VTUR2, 2026-09-30) — the bind-pose step is its own card, TBIND (value 4, build)
Every TUR model VTUR2 pulls is affected, not only the Hoth mortar:
- bone_space 0 with bone-relative segments: the YAV1 watchtower's gun sits at the foot (bones 14-15.5 m up); the TAT2
  barge turret 0.3 m low.
- bone_space 1 (the map bake never runs `to_model_space`): HOT1's dish ~1 m low, frame-checked; also the BF2
  `com_weap_gunturret`, `kam_prop_antiaircraft` and `geo_bldg_geoturret` (BF1 GEO1 too).
This card keeps the aim-driven node draw and the destroyed geometry.

## Note from TBLD's RE (2026-10-03, `analysis/re-build-zones.md` 1)
- A destroyed armed building is not removed on the PS2: it goes to state 1 and draws its `DestroyedGeometryName`
  model with that model's collision (HOT1's turrets, `tur_bldg_tower`, the chaingun roof / tripod ...), or to its
  build pad when it has an unbuilt model and no destroyed one. TBLD's build keeps the state; drawing the destroyed
  model is left for this card.

## The maintainer, 2026-10-12 18:55 (INBOX, ask 1) - planner 2026-10-12 19:15: value 3 -> 5, widened
His words (via chat): while he sits in a turret the turret and its user should rotate with the aim; today the turret
is a static image. Wanted, all three:
- (a) **The turret turns.** The player's own turret turns with his aim in first AND third person; a bot's turret turns
  where the bot aims.
- (b) **An OPEN turret shows its occupant**: the seated soldier model in the seat's pose, his team and class skin,
  while someone is in the seat (player or bot); gone when the seat is empty.
- (c) **NOT for the turrets you enter**: an enclosed seat (Hoth's white turrets) draws no occupant, as the PS2.
Card TURR (bots manning turrets) stays as it is - this card is the visual half, and does not wait for it: the player
in the seat is enough to referee (a) - (c).

### RE to do first (phase re; the answers go to `analysis/`, [O] with the verifier)
1. Which node takes yaw and which pitch, and the limits, per turret class (the aimer B1 FUN_0026e260 writes the
   angles; `TurretNodeName` / `AimerNodeName` / `BarrelNodeName` above), with BF2's twin opened.
2. Open or enclosed: which class key or seat field decides that the occupant is drawn - a census over EVERY armed
   building class on both discs (program-side count, not a reading), with the six showcase maps' classes named.
3. The occupant's pose: which animation the seat asks for per turret class (the vehicle seats' `PilotAnimation` /
   pose keys are the first suspect [H]) and where the soldier is attached (which hardpoint, does he turn with the yaw
   node).
4. A PCSX2 frame of each case for the build's pairs (one open turret occupied and turned, one enclosed occupied).

### Acceptance (replaces the line above for the build)
- A picture pair, PS2 beside the port, per case, in the card's report: an open turret empty / occupied / turned (yaw
  at least 60 degrees off its rest, pitch off level), an enclosed turret occupied. Parity run on the closest PS2
  capture: not worse.
- A test: the drawn aimer node's yaw and pitch equal the seat's GunState within 1 degree, for the player's seat and a
  bot's (mutation: draw it at rest -> the test fails).
- A test through the BAKED class db: the occupant is drawn if and only if the seat is taken and the class is open by
  the RE's rule; the list of open / enclosed classes printed per map (the census line of the report).
- First person: a picture from the seat at two aims 90 degrees apart shows the barrel in the same place on screen and
  the world turned.
- The destroyed state (the older half of this card): a turret whose pool is empty draws `DestroyedGeometryName`,
  one picture - or its own card, see `resume:`.

## RE done (2026-10-13, lane 2) - `analysis/re-turret-draw.md` [O], docs 1aa04dc; the card is split in three
Report `loop/reports/20261013_0524_card_VTURD.md`. PS2 reference frames: `lead_tmp/vturd/sheet_ps2_cases.png` (15
frames), `lead_tmp/vturd/ps2/sheet_hot1_bots2_dish_zoom.png` (a bot gunner on the dish turret, from outside).
- **This card = the turn** (asks a). The `TurretNodeName` node takes the yaw (x YawTurnFactor) and the pitch x
  PitchTurnFactor - 0.0 on every soldier-seat turret but BF1's Bespin anti-aircraft gun, so it turns and never
  tilts. The `AimerNodeName` node looks along the AIMER's direction (the port's `GunState::aim`), not the camera's:
  on the PS2 the two are 2 to 21 degrees apart when the target is near. A barrel is its parent's matrix slid back
  by `state x BarrelRecoil`. Nothing else moves. Every turret model splits exactly into rigid per-bone parts (0
  triangles across bones but for the fambaa and one space turret), so the parts are moved like door leaves.
- **Card VTURG = the gunner** (asks b, c): drawn when the seat has `PilotAnimation` / `Pilot9Pose`, hidden
  otherwise; BF1 6 classes drawn / 1 hidden / 2 with no soldier, BF2 15 / 7 / 47. On the showcase maps: BF1's gun
  turret (NAB1, TAT1, YAV2) and BF2 Hoth's dish turret and Mygeeto's anti-aircraft gun show him; Hoth's laser tower
  hides him; Utapau's turrets and the chainguns fire by themselves.
- **Card VTURX = the wreck and the zoom** (value 3): a dead turret draws `DestroyedGeometryName` rigid; zoomed, the
  gunner's own view leaves the whole building out. The state machine is card TBLD's.
- **"First AND third person" (ask a) on the PS2**: a building turret has no first-person view. The seat camera is
  third person and rides the turret's aim frame, so the gun stays where it is on screen and the world turns
  (`sheet_ps2_cases.png` row 3: two barrels fixed through 30 degrees of pitch). The other view is the zoom (R3): a
  scope frame, no turret drawn. This card's "first person" picture is therefore the seat camera at two aims 90
  degrees apart; the zoom is VTURX.

### Acceptance for THIS card (the turn; the occupant's lines above belong to VTURG, the destroyed line to VTURX)
- A test: the node matrices from a fixture of the PS2's own rows (`lead_tmp/vturd/ps2/nab1_gty`, `myg1_lg`,
  `hot1_dish`: yaw, pitch, the aimer's direction -> the turret node, the aimer node, a barrel) equal the PS2's to
  1e-3 on every row kept; `lead_tmp/vturd/replay_nodes.py` / `replay_aimer.py` are the transcription and print the
  alternatives that must fail (full pitch on the turret node, the aimer node as the camera's aim).
- A test: the drawn aimer node's yaw and pitch equal the seat's `GunState::aim` within 1 degree and the turret
  node's yaw the `look` yaw, for the player's seat and a bot's (mutation: drawn at rest -> fails).
- A census line: per map, the turret instances whose every drawn part is bound to a rig node (the bake's own
  count), and 0 triangles across bones on the six showcase maps.
- Pictures, PS2 beside the port: an open turret empty, turned 60 degrees or more with the pitch off level (BF1 Naboo
  gun turret, BF2 Mygeeto anti-aircraft gun), an enclosed one turned (BF2 Hoth laser tower); the seat camera at two
  aims 90 degrees apart with the barrel in the same place. Parity on the closest PS2 capture not worse; at rest no
  golden moves.

### Not this card (for the planner)
- The PS2's turret TURN LAW differs from the port's PROVISIONAL seat look (`core/src/vehicle/gun.rs`): the stick
  drives a rate (`-stick x MaxTurnSpeed / MaxPitchSpeed`, slewed by `TurnFilter` / `PitchFilter`: BF1 exponential,
  BF2 linear) inside `YawLimits` / `PitchLimits`, and a separate aimer chases the aim point at `AimerYawRate` /
  `AimerPitchRate` inside its own limits (`re-turret-draw.md` 2.2, 2.3). No card yet.
- The same node posing applies to every vehicle's turret sections (the port draws vehicles rigid). No card yet.

## Built and landed (2026-10-13, lane 1) - 1cf5c8b, report `loop/reports/20261013_0926_card_VTURD.md`
The turn is drawn: `bake/src/turret_rig.rs` (parts per node + the rig), `game/src/viewer/turret_pose.rs` (the three
node matrices from `GunState`). PS2 rows: turret node 611 of 613, aimer node 231 of 231, barrels 228 of 228; the drawn
aimer and turret node within 1 degree of the seat's gun state on 4 classes (mutation fails); goldens and parity
unchanged. Pairs `lead_tmp/vturd/pairs/`. Seen in the real game and filed as card VTURL: a full-circle turret's look
stops at 180 degrees and the gun swings back the long way (the sim's PROVISIONAL seat look).
