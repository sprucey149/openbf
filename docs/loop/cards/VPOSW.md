---
id: VPOSW
title: A walker's posed nodes are drawn over its walk skin: the AT-ST's head (cube5) with its chin and ear guns, the AT-AT's head and cannons, BF2's one-man AT-ST aimer_y / aimer_x, the AT-TE's guns and main cannon, the spider walker's turrets - each aimer / turret node of the walker's sections as the PS2's node table holds it (analysis/re-vehicle-pose.md 6)
status: archived
issue: 67
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #67](https://github.com/sprucey149/openbf/issues/67).

## Why
RE done (`analysis/re-walker-nodes.md` [O], 2026-10-22, docs 8a0c08b). The port's walker skin poses only the walk clip
(`viewer/walker_pose.rs`), so on five of the six showcase maps a piloted walker never turns its head or guns. On the
PS2 every walker's aimer, turret and barrel node is the vehicle formula (`re-turret-draw.md` 2.2 / 2.3) built on its
parent's WALK-POSED matrix (the table entry): BF1 1,205 / 1,206 RAM rows to 1e-3, BF2 all within 0.04, parent at its
bind 0 of 1,332. What turns: the AT-ST's head cube5 (yaw +-90, pitch -5..20 deg of its bind under the posed neck)
with its chin pivot, barrels and ear guns; the spider's two yaw + pitch turrets; the AT-TE's main cannon (TURRET1,
yaw only, its barrel on the aim frame), four front and two side guns; the AT-AT's head (BF1: the BODY aimer's matrix
written over TURRET1's node, re-walker-nodes 4) and cannons; BF2's one-man AT-ST aimer_x only (aimer_y stays on the
walk pose).

## Build (re-walker-nodes 7)
- Class data: each walker section's aimer chain (node, parent, AimerYaw/PitchLimits, rates, BODY or TURRET base,
  barrels); VPOSE's build parses the same keys for hovers - hence `deps: VPOSE`.
- Pose, after `WalkerRig::palette`, parent-first: parent_now = `palette[p] * global_bind[p]` (or a chained node's
  written matrix); aimer `turret_pose::aimer_node(parent_now, local, dir_model)`, turret
  `turret_node(parent_now * local, yaw, pitch x PitchTurnFactor)`, barrels `barrel_node`; write back
  `palette[n] = node * global_bind[n]^-1` before `skin` (every drawn bone is in the table). Same in
  `WalkerRig::node_now` so riders hang from the posed node. Visual only: no sim hash.
- Direction: clamp in the frame bind(class) x parent_now x body (re-walker-nodes 5); want = toward the seat's aim
  point. PROVISIONAL: steady state first (no rate; the PS2's rates are 57.3 / 114.6 deg/s on every walker aimer).
  A never-aimed walker keeps the walk pose, so idle-walker goldens should not move.

## Evidence
- RAM rows for the test: `lead_tmp/vposw/walker_nodes_ps2.txt` (1,798 rows: parent entry, bind local, direction in
  model space, the PS2's entry; BF1 1,230 / 1,230 and BF2 492 / 568 at 1e-3 by an independent numpy pass, `mkrows.py`).
- Tools: `lead_tmp/vposw/` (wtable.py, wreplay.py, waimers.py, wbodyframe.py, atat_head.py, drawn_census.py).
- The root-bob note below is NOT DummyRoot (re-walker-nodes 0.7: bone 0's translation is 0 in all 348 RAM rows, every
  DummyRoot track is z-only); the bob is bone_root's own track. [H] the port's bone_root at the PS2's clip time: read
  the anim player E+0x1d70 first if the build's pictures show a height gap.

## Acceptance
- A test from `walker_nodes_ps2.txt` (copied to `game/tests/data/`): the port's posed node (aimer / turret / barrel law
  on the row's parent and local) = the PS2's entry within 0.01 deg and 1 mm on every BF1 row (BF2 within 0.04: the
  draw-time lag). Mutations that must fail it: the parent at its bind; the frame without the parent; BF1 AT-AT head
  by the turret law.
- An end-to-end test through `pose_meshes` / the palette: a piloted AT-ST aiming 60 deg left moves the cube5 vertices
  and leaves the legs; a never-aimed walker's palette equals the walk palette.
- Pictures: an AT-ST aiming sideways, PS2 (a stored state with a piloted AT-ST) beside the port.

## Original card text (2026-10-22)
### Why
On the PS2 a walker's aimers and MountedTurrets write their nodes into the walker's node table (BF1 E+0x1ec0, BF2
E+0x1d7c) and the walker's skinned draw reads that table per palette bone (`analysis/re-vehicle-pose.md` 6, card
VPOSE's RE). The port's walker skin (`viewer/walker_pose.rs`) poses the walk clip only, so on five of the six showcase
maps the AT-ST's head (`cube5`, 851 vertices: an own-weapon aimer chain `cube5` -> `aimer_mouthpivot`, chin barrels
`barrel_lgun/rgun`, ear guns `aimer_Leargun/Reargun`) never turns toward its aim; likewise the AT-AT's head
(`aimer_head`, BF2 HOT1), BF2's one-man AT-ST (`aimer_y` -> `aimer_x`, UTA1 MYG1), the AT-TE's guns and main cannon,
the spider walker's turrets (BF1 YAV2). Census: `lead_tmp/vpose/nodecensus_<g>.txt`.

### Evidence
- re-vehicle-pose 6 [V]: the walker draws refresh aimers and turrets into the table before the model (BF1 FUN_00217278
  B1:128485/128496/128593, BF2 FUN_0025b428 B2:176748/176954/176971 - turrets only while the fade alpha >= 0.8); a
  walker skips its WHOLE draw while a turret's viewer test fails (B1:128431-128439, B2:176563-176573).
- [H] the parent step: the aimer-node formula with the parent's BIND fails in RAM (spider `turret_y_2` 0/37, AT-TE
  `aimer_rgun_1` 0/19, the AT-TE's `aimer_maincannon_y` turret node 0/19) - the parents (bone_root, bone_head) are
  posed by the walk clip and are not in the table where `lead_tmp/vpose/replay_aimer_veh.py` looks. Read which matrix
  the walker's aimer / turret base uses before building.
- Moved from card VPOSE (VRIDE's build note, 2026-10-15): the posed WALKER skin is drawn without its clip's DummyRoot
  translation (`viewer::walker_pose`, the bake holds it at rest): BF2's one-man AT-ST `bone_root` sits 0.07-0.22 m
  higher than the PS2's RAM (port y 2.915-2.942 against 2.717 / 2.852 in `ref/states/bf2_uta1_cp3_con_cp1_postA_spawn.p2s`)
  and without its sideways sway (+-0.029 against 0.082 / -0.075). [H] the bob; measure the walker clip's DummyRoot
  track first (`lead_tmp/vpose/findump` prints every track of a clip; the walker bank's clip names were not found by
  `<model>_walk_*` guesses - list the bank with `lead_tmp/vpose/bankls`).

### Acceptance
- RE first (bounded): the walker aimer / turret base matrix and the parent step, replayed on RAM (`replay_aimer_veh.py`
  extended: the AT-ST's `cube5` and the AT-TE's guns fit within 1e-3 on every row of the stored states).
- Then: a test from those RAM rows (the port's posed node = the PS2's table entry within 0.01 deg); mutation: the walk
  clip's head alone fails; pictures: an AT-ST aiming sideways, PS2 beside the port.
## Playtest 2026-10-28 (the maintainer)
BF2 GEO1, the Republic AT-TE (vehicle 12, `rep_walk_atte`): "the geonosis clone cp vehicle is rendering incorrectly". The maintainer's
screenshot (local, `engine/artifacts/lead_tmp/playtest_20261028/geo1_atte_cannon.png`): the main mass-driver cannon floats
on its own up and to the left, clear of the walker's back; the gunner sits on an empty mount. Black box of the F9 (09:24,
build a43f94e5+dirty, seed 49): the AT-TE at (13.6, -63.4, 226.7), hp 10589 of 40000. Raise this card's value: the player
sees it at every Republic command walker.

## Local issue 67 implementation, 2026-10-29

See `analysis/walker-aim-67.md` for the demonstrated AT-TE double-applied mount offset, implementation, class coverage, tests, local captures, remaining original-game acceptance and distribution receipt. The walk-only overlay leaves zero-aim palettes unchanged; only the separately demonstrated AT-TE baked cannon offset is repaired. VPOSE's hover/flyer build remains separate, as does droidekas #65. This card is not marked complete without its missing original-game acceptance.

Close-range playtest follow-up: actual hull/surface overlap replaces the aggregate box's full-hide test, and a foot cannot hide an entire walker. Walker animation clocks/leg pose and gun aim now interpolate with vehicle motion; rider/muzzle attachments use the same drawn pose. Snapshot builds, reproduction and checks: `analysis/walker-aim-67.md`, close-range follow-up section.

Riding-control follow-up authorized by the maintainer: pilot mouse look passes the yaw window and the existing BF2 body follows at the authored rate; humans can take AI-held gun seats while the displaced bot stays aboard. Walker gunners now retain their original rate/limit law and rear-facing bind frame, including the AT-TE rear 120-240 degree yaw arc. The main gunner camera uses its mount translation. Research, capture limitations and regression/distribution results are in analysis/walker-aim-67.md. This expands the earlier visual-only scope for these demonstrated controls.

Presentation follow-up: the gun-seat camera now rides the posed animated mount (authored ThirdPersonFOV, TiltValue, TrackCenter/TrackOffset) with the HUD reticle on the aim ray; walker shots draw their authored tracers from the posed fire points at their SecondaryOrdnancePeriod interval and play positioned fire loops; an armed seat shows its own weapon rows and reticle. WarmUpSound stays unread because the sim has no spin-up phase. The branch is merged into local main as d8881a8 on top of droideka #65; walker and droideka tests and Clippy pass together. Evidence in analysis/walker-aim-67.md, presentation follow-up section.
