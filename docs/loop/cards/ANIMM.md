---
id: ANIMM
title: The legs match the ground: the PS2's movement clip choice (walk / run thresholds and BF2's speed mode), the playback rate that keeps feet from sliding, the strafe twist of the lower body, the turn on the spot
milestone: M1
value: 5
phase: build
status: done
deps: ANIML
files: engine/crates/game/src/animator.rs,engine/crates/game/src/soldier_vis.rs
declares:
needs:
effort:
branch: animm
resume: landed 8e8d33c2 2026-10-15
created: 2026-10-05
alpha: yes
domain: render
---
## Why
game/src/animator.rs switches walk to run at a provisional 4.5 m/s, plays every clip at a constant rate (feet slide
at any speed but the clip's own), has no strafing pose (a side-stepping soldier runs straight ahead) and no turn clip.

## Evidence (analysis/re-soldier-anim-layers.md 3.5, 7; analysis/re-foley.md 4.4; [O])
- Neither game ships standing strafe clips. A soldier moving sideways plays the forward or backward clip with his
  legs turned toward the move: BF1 forward when atan2(left speed, forward speed) is inside +-1.2566 rad (+-1.8850
  while a forward clip already plays; -1.8064 .. +1.1781 when alert), else backward; the leg angle is limited to
  +-1.2566 (alert: -0.7854 .. +0.5498) and eased 25 % per update (BF2: min(1, 7.5 dt)); the DummyRoot pose is
  rotated by it about the up axis and the aim pose (ANIMA) turns the torso back.
- Walk or run: BF1 forward runs above 5.5 m/s (5.0 to stay), speeds divided by the skeleton scale; backward is always
  the run clip where one exists (stand). BF2: the soldier's speed mode (run above 0.78 x MaxSpeed, 0.71 to stay), and
  run where the walk clip is missing (stand_walkbackward is).
- Rate: dt x (0.95 v / v_clip + 0.05), v_clip the clip's root speed (BF1 RAM: walkforward 3.687, runforward 6.338,
  walkbackward -1.245, runbackward -3.724, crouch walk 2.075 / -2.050 m/s).
- Idle under 0.3 m/s (BF2: 0.2 once moving). Turn on the spot: the twist follows the facing; past +0.3927 rad the
  turnright clip plays, below -0.7069 (BF1 crouch -1.1781; prone +-0.4712) turnleft.
- The FOLY build (footsteps) keeps its own clip id and phase: make it read the animator's, or show both agree.

## Note from ANIML (landed 644695a, 2026-10-05)
- `animator::body` is the PROVISIONAL movement choice (RUN_SPEED 4.5, idle under 0.3 m/s, forward by the facing) and
  `Move::ids` maps it to each game's ids; `Player::advance` plays at the plain dt (the rate rule goes there);
  `Rig::body_slots` falls back to the other gait, then idle, for an empty slot. The census (`ANIM_CENSUS=1` on
  `round_to_outcome`) prints the upper-over-lower clip pairs.

## Note from ANIMA (landed 87046b1, 2026-10-06)
- `AnimInput::aim.0` (the aim's yaw from the legs) is fed 0 by `soldier_vis`: with the leg twist, feed the twist's
  remainder; `animator::aim` already limits it to 0.6283 rad and shares it over the spine / head.
- Seen, not chased: from behind, the port's super battle droid's muzzle flash sits over the middle of the head
  while firing (`lead_tmp/anima/port_sbd_behind.png`); the PS2's is right of the head, at the raised right wrist
  (`lead_tmp/anima/ps2/sheet_cis.png`). Compare the fire point with the droid's forearm before calling it matched.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
- 2026-10-12 lane 2 (card ROLLS, RE `analysis/re-roll-picture.md`): the PS2's ROLL turns the body with the SAME field
  as the walking soldier's leg twist (BF1 animator+0x8c, BF2 +0x20cc: eased 0.25 an update, then `DummyRoot` turned
  about the up axis). ROLLS (value 5, same two files) builds `Animator::twist` with the easing and the root turn and
  feeds it the travel angle only while the body is `Special::Roll`, 0 otherwise; this card then supplies the walking
  target and need not build the mechanism. Live rows worth knowing here: a strafing soldier holds about +-71 degrees
  of twist on both games (BF1 alert left strafe: -71 on the backward clip; BF2: +71 on the forward clip), and after
  he stops BF2's turn-on-the-spot clip takes the twist to 0 at 9.6 degrees an update
  (`lead_tmp/rolls/ps2/analyse.py out bf1|bf2 roll_left --rows`, the rows before and after the roll).

## Landed 8e8d33c (2026-10-15, lane 1)
- `animator::choose` (BF1 FUN_0022e990; BF2 FUN_0024f350 + FUN_0027ed18, re-soldier-anim-layers 7.1), the rate
  `move_rate`, `Animator::turn_on_spot`, the twist under the aim. Acceptance 1: `the_chooser_is_the_ps2s` (5
  mutations each fail a test). Acceptance 2: `soldier_vis::tests::the_feet_keep_to_the_ground` (table in the report,
  `lead_tmp/animm/feet_table.txt`). Acceptance 3: FEEL_NOTES line.
- NOT done here, filed: foley still uses its own strafe-clip chooser (card FOLYM). [H] left: BF2's alert branch through
  a weapon's vtable+0x5c (an unclamped twist); the skeleton scale (taken as 1).
- Not looked at in a picture: no golden shows a moving soldier (0 goldens moved). A PCSX2 strafe pair (ps2-roll strip
  kit) would be the visual check.
