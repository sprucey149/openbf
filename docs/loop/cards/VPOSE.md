---
id: VPOSE
title: A hover's own posed nodes are drawn: its TURRET sections' turret / aimer / barrel nodes and its own weapons' aimer nodes by the PS2's node laws (card VTURD's), and its FinAnimation nine-pose from the drive's (a, b, c) - fighter tank skis and winglets, bike vanes and body pitch, BARC lean, skiff rudders - with riders on the posed nodes and HoverNine starting at p = y = 0 (analysis/re-vehicle-pose.md)
status: archived
issue: 67
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #67](https://github.com/sprucey149/openbf/issues/67).

## Why
The port draws every hover rigid in its bind pose (`game/src/viewer/vehicles.rs`). On the PS2 (RE done 2026-10-22,
`analysis/re-vehicle-pose.md` [O]) a hover's node table is posed by three writers every update: each TURRET
section's MountedTurret (its `TurretNodeName` node turned by the gunner's yaw, its aimer and barrel nodes along the
aimer), the aimers of the vehicle's OWN weapons (the fighter tank's side guns, the AAT's side guns, the bike's gun,
the BARC's front guns, the combat speeders' guns), and the class's `FinAnimation` nine-pose (the fighter tank's skis
and winglets, the bike's handlebars / foils / flaps / pedals and its whole body's 5 deg pitch, the BARC's body lean,
the skiff's rudders). On BF1 NAB1 / TAT1 / YAV2 and BF2 UTA1 / MYG1 a fighter tank fires while its turret looks
forward, an AAT's main gun never turns, a bike's vanes never move, and card VRIDE's gunner and BARC rider hang from
nodes drawn at rest. Also a landed bug: `turret_pose::HoverNine::START` is (-1, -1); the PS2 zeroes p and y
(B1:96395-96396, B2:130826-130827; 786 of 786 never-driven hovers in RAM), so the port's rider eases out of a
full-reverse, full-left pose for ~0.5 s after boarding a fresh hover.

## Evidence
- `analysis/re-vehicle-pose.md` 2 (the table: layout, writers, order, no reset), 3 (turret + aimer nodes: 337 / 337
  hover turret rows and every BF1 aimer row in RAM fit re-turret-draw 2.2 / 2.3), 4 (FinAnimation: setter, bank rig,
  constructor, drive, slerp sampler + slerp blend, the LATER duplicate track wins, compose; 297 / 297 hover tables
  replayed to 0.001 deg), 5 (the BARC rider leans with boneroot), 7 (the build spec).
- Tools: `lead_tmp/vpose/` - `ps2table.py` (a save state's vehicle node tables), `replay_fin.py`, `replay_turret.py`,
  `replay_aimer_veh.py`, `findump/` + `zafdump/` (a clip's tracks, a bank's rig), `clips/*.json` (the five fin clips +
  rigs, both games), `nodecensus_<g>.txt` (every vehicle class's posed nodes and triangles across bones).
- RE-phase card text (kept): "a vehicle's TURRET section is the same MountedTurret as a building turret ... a hover's
  drive also poses the hover's own model by its class's FinAnimation clip ... card VRIDE's gunner hangs from that
  turret node and stays at its rest until this lands." The walker DummyRoot note moved to card VPOSW.

## Build (re-vehicle-pose 7)
1. Bake (`rebake=side`): per hover class with a posed node or a FinAnimation - its node rig (each skel node's bind
   matrix and parent, as `ManifestTurretRig`), its model drawn per node (one-bone skin as the walker skin, or per-node
   parts: 0 triangles span bones on every hover but BF1's combat speeder (14) and the BARC (16)), and its FinAnimation
   clip + rig (every frame of every rig bone, the later duplicate track kept, the rig's parents and rests).
2. Pose, visual only (no sim hash): all nodes at bind; the fin rig from `HoverNine`'s (a, b, c) while piloted (else
   the last pose; never driven = frame 4 = bind); each TURRET seat's turret node from its `look` (yaw, pitch x
   `PitchTurnFactor`, through the parent's posed matrix), aimer node(s) from its `aim`, barrels (`barrel_node`); each
   own-weapon aimer node from the pilot seat's aim (PROVISIONAL per-aimer direction: from the node's pivot to the
   seat's aim point inside the aimer's limits; the PS2 runs one aimer per weapon; note the tank's 2-link chains
   `lgun_x` pitch-only -> `lgun_y` yaw-only).
3. Riders (`soldier_vis::rider_now`) hang from the POSED parent (BARC boneroot, tank turret_y), same tick.
4. `HoverNine::START` = (0, 0); fix its doc comment.

## Acceptance
- A test from the PS2's own table rows: the port's fin pose at the RAM's (a, b, c) equals the PS2's table entry for
  every fin bone of a BF2 ridden bike (`lead_tmp/vpose/ps2/bf2_b2bike_3_left.p2s_table.json`, a 3.882 b 0.882 c 0.236)
  and the Mygeeto fighter tank (`bf2_myg1_cp2_cp1_con_post1_spawn.p2s_table.json`, a 4.218 b 1.218 c 0.901) within
  0.01 deg / 1 mm; mutations that must fail: nearest frame, nlerp, the FIRST duplicate track, a rigid hover.
- A test: the fighter tank's `turret_y` at the RAM's yaw (-0.0584) puts the gunner's hardpoint at (0.0089, 0.6581,
  -3.0100) in the tank frame within 1 cm; the BARC rider at (0, 0.0001, 0.0100) with up (0, 1, -0.010) from boneroot's
  posed matrix (`bf2_kas2_cw_rep_spawnmap.p2s`); `HoverNine::START` = (0, 0).
- Pictures: a ridden fighter tank turned and a ridden bike leaning, PS2 beside the port (the PS2 frames: any state of
  `lead_tmp/vride/ps2/` or a fresh `ride_b2.py` capture).
