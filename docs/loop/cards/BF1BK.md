---
id: BF1BK
title: Two ground movers to the PS2's, one build in soldier.rs: (a) BF1 soldiers walk BACKWARD at MaxSpeed - PS2 -6.50 m/s standing and -4.55 crouched (BF1 Theed, live), the port -4.5 and -3.15, it applies BF2's MaxStrafeSpeed backpedal cap to both games; (b) BF2's roll is the drag mover (FUN_00248560, 1.4 m/s2 of stick, no target) - stick held it keeps 10.5 m/s over 8.78 m (port slows to 9.29, 8.21 m) and a sideways stick bends it 0.38 m (port 0.08)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A BF1 soldier on the PS2 walks backward as fast as forward; the port's walks back at the strafe speed, 31 % slower, on
every BF1 map and for every class (the player's retreat, a bot backing off). Found by AUDIT AIRC (2026-10-02) while
checking AIRC's "0.2 x MaxSpeed forward AND back" in the air: the same holds on the ground.

## Evidence
- Code [V]: B1 `FUN_002098b8` B1:119236-119243 (ELF 0x209f98-0x20a000) takes class+0x1834 (MaxSpeed) for a forward
  target of EITHER sign; class+0x1838 (MaxStrafeSpeed) is the side axis only. The stand case hands it the raw stick
  (`FUN_00205cc8` B1:117386), crouch x 0.7 / x 0.6, prone x 0.3 / x 0.2 (`analysis/re-stance-roll.md` §3.3, §4.3b).
  BF2's `fwd > 0 ? MaxSpeed : MaxStrafeSpeed` (B2 `FUN_00240bf8` case 0) is BF2's own rule.
- PS2 live [V-PS2] (BF1 Theed, the default trooper, MaxSpeed 6.5 / MaxStrafeSpeed 4.5;
  `lead_tmp/audit-airc/ps2/out/bf1_ground_{back,fwd,left,right}.csv`, `bf1_back_jump.csv` rows 14-18 crouched):
  stick back **-6.5004 m/s**, forward 6.5004, strafe 4.5007; crouched back **-4.550**, crouched forward 4.5504.
  The ramp from rest is 2.806, 3.929, 5.051, 6.174, 6.500 m/s (84 x dt, then 84 x dt less the brake 70 x 0.6 x 1.2 x dt).
- The port (scratch probe `core/examples/audit_airc.rs`, BF1 rules, same class): stick back **-4.500**, crouched
  **-3.150**; forward 6.500 / 4.550. `core/src/soldier.rs` `fwd_cap`: "Backpedal is capped by MaxStrafeSpeed in both
  games" - true of BF2 only.

## Acceptance
- `GroundRules::Bf1`: the backward cap is `max_speed x ControlSpeed[stance].fwd`; `GroundRules::Bf2` unchanged.
- A unit test with the PS2's class (6.5 / 4.5, Acceleraton 70): steady speeds stand -6.50 and crouch -4.55 within
  0.01 m/s under BF1's rules; under BF2's rules the same class still walks back at -4.50 (MaxStrafeSpeed). The
  module header and the comment at `fwd_cap` say which game the asymmetry belongs to.
- 8 seeds against a null variant on the BF1 maps (bots that back up move faster); declared moves viewed.

## Notes
- Not in this card: BF1's ramp (x 1.2 acceleration with a separate brake - the port reaches 6.5 in 3 ticks, the PS2 in
  5), listed PROVISIONAL in `GroundRules`' doc comment.
- A BF1 backward JUMP adds -2.0 m/s on top (card JARC).

## Part (b) - BF2's roll mover (card ROLLM, merged here by the planner 2026-10-02 09:30)
Same file, same declares, same audit, PS2 rows already taken: one build pays the setup and the gates once. ROLLM's
text, verbatim:

**Why.** AIRC fixed the jump row of ControlSpeed (0.03 is an acceleration factor, not a speed). The roll row (0.02) has
the same meaning and the port still reads it as a target speed: a roll with the stick held slows down where the PS2's
holds its speed, and the stick cannot bend a roll. Found by AUDIT AIRC (2026-10-02).

**Evidence.**
- Code [V]: B2 `FUN_00240bf8` `case 5:` (B2:160662-160680) calls `FUN_00248560(dt, turn, stickFwd x class[0x790] x
  class[0x748], -stickStrafe x class[0x7b0] x class[0x748], E)` - the drag exp(-0.012 |v| dt) plus the stick ADDED as
  an acceleration (0.02 x 70 = 1.4 m/s2), no target (`analysis/re-stance-roll.md` section 4.3b).
- PS2 live [V-PS2] (BF2 Coruscant, rebel soldier, MaxSpeed 7.0, RollSpeedFactor 1.5, 26 roll updates;
  `lead_tmp/audit-airc/ps2/roll_cmp.py out bf2|bf2_s2`): the roll rows replay through that mover within 1.3e-6 m/s
  (through a target approach: 4.7e-2).

  | stick during the roll | PS2 speed, travel | port |
  |---|---|---|
  | held forward | 10.50 -> 10.56 m/s, 8.781 m | 10.45 -> 9.29 m/s, 8.206 m |
  | released | -> 9.62 m/s, 8.405 m | -> 9.29, 8.206 m |
  | back | -> 8.67 m/s, 8.034 m | -> 9.29, 8.206 m |
  | sideways | +0.99 m/s aside, 0.383 m | 0.075 m |
- The port: `core/src/soldier.rs` `Stance::Roll` goes through the ground branch (`step_to` toward stick x cap x 0.02
  at Acceleration x 0.02), no drag.

**Acceptance (b), beside part (a)'s above.**
- Under `GroundRules::Bf2` the roll's velocity is BF2's mover: `v *= exp(-AIR_DRAG |v| dt)`, then
  `+= stick x CONTROL_SPEED[5] x acceleration x dt` on each axis; gravity and the floor as today.
- A unit test with the PS2's class: after 26 roll ticks the speed is 10.56 (held), 9.62 (released), 8.67 (back) within
  0.05 m/s and the travel 8.78 / 8.41 / 8.03 m within 0.05 m; a sideways stick moves him 0.38 m aside within 0.03 m.
- BF1 maps: the port keeps BF2's roll on both games (PLAN, locked) - say in the test which rule BF1 maps get.
- Each of the two tests is seen to fail with its rule switched back (paste both).

**Order and scope.** Two commits, (a) then (b), each with ci green; one gates run and one landing for both. If (b)
moves the 8-seed sweep against the null variant by more than (a) alone did, land (a) and park (b) with the table.
JARC (value 5) edits the same lines of `soldier.rs` and goes first by value; build this on what it leaves.
Since AIRC the jump row's columns 1-2 multiply Acceleration: a hero's `jump 0.1 0.1` (CSPD's evidence) is 7 m/s2.
Undo the merge: `loopctl.py set ROLLM status=todo resume=` and cut this section.

## JARC note (2026-10-03) — one more number this card owns
JARC's take-off runs the ground mover first, so BF1's stick + jump from a stand is (the first ground update) + 2.0 m/s:
the port 70/30 + 2.0 = 4.33 m/s and 4.12 m, the PS2 84/30 + 2.0 = 4.810 m/s and 4.59 m
(`lead_tmp/audit-airc/ps2/out/bf1_stand_fwd*.csv`). The difference is this card's BF1 acceleration (Acceleraton x 1.2,
the ramp 2.806, 3.929, 5.051, 6.174, 6.500). When it lands, `core::soldier`'s
`stick_and_jump_together_from_a_stand_is_a_forward_leap` takes the PS2's own BF1 numbers (4.810, 4.59) and
`a_running_jump_takes_off_by_each_games_rule` its -8.50 backward take-off (the port's backpedal is -4.5, so -6.5 today).

## AUDIT JARC note (2026-10-03)
Eleven more BF1 rows (Theed; `lead_tmp/audit-jarc/ps2/out/bf1_*.csv`, `tools/port_cmp2.py ps2/out bf1`) say the same
and add the diagonal: stick and jump from a stand 4.803 m/s and 4.58 m (port 4.333 / 4.12); with the stick diagonal
the first ground update is 2.803 m/s on EACH axis (84 x dt; port 2.333); a diagonal run is (4.596, 3.182) m/s - the
unit circle of (6.5, 4.5) - and its backward take-off -6.596. BF1's own jump rule is exact in the port.

## Note from JGATE (2026-10-05, 0cbe7b7)
- `core/src/soldier.rs`'s tests are in `core/src/soldier/tests.rs` now (add it to `files:`). BF1 has a stand timer
  field, `Soldier::stand_age`; `air_trial` pre-ages it (BF1's jump waits 0.5 s in the stance).

## Note from ROLLS (2026-10-12, RE `analysis/re-roll-picture.md` 5)
- `core/src/soldier.rs`'s doc comment on `Soldier::roll_predicate` says "any direction except the straight-back +-30
  deg cone". The code is the PS2's (`FUN_00247b88`) and is right; the sentence is not: the refused cone is 60 degrees
  either side of straight back (a roll goes anywhere within 120 degrees of ahead; live on BF2: the stick straight
  back crouches, at 135 degrees it stood the crouched soldier up). Fix the comment when this card is in the file.
- Seen in `FUN_002471b0` while reading the roll (B2 162242-162262), not measured: the roll's speed-up to
  RollSpeedFactor x MaxSpeed adds to the FORWARD component only when it is positive, and to the side component
  always - a back-diagonal roll (allowed up to 120 degrees) would be boosted sideways only. The port scales both.
