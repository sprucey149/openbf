---
id: BTURN
title: BF2 bot body turn that looks natural: measure saturation and time-to-face, choose a rate between 5 and 20 rad/s, ease drawn yaw and respect torso aim limits; BF1 stays at 1x (the maintainer 2026-10-22)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card LOOKR's RE (`analysis/re-look-ramp.md` 2 and 4, [V], two blind verifiers): a BF2 bot's soldier turns at
**4 x MaxTurnSpeed** - `FUN_00238068` returns class +0x754 x 4.0 when E+0x330 < 0 (B2:151844-151851; the 4.0 is
0x3c014080 at 0x23807c), the yaw rule `FUN_00247fa8` uses it flat for a bot (B2:162912-162913), and the AI's stick
writer `FUN_00249f80` hands the same value to its converter `FUN_002a54d8` (B2:164694-164697). For a 5.0 class that is
20 rad/s (1146 deg/s) where the port's bots turn at 5.0 (`core/src/ai/gunner_foot.rs` 233: rx = -e_yaw / (max_turn_speed
x dt), clamped). BF1's bots are flat MaxTurnSpeed (`FUN_002098b8` B1:119340-119341, the port's rule already). A bot that
turns 4x faster reacts to a flanker in a quarter of the time: the maintainer's top concern is the bots.

## The maintainer's decision (2026-10-22)
Match the 4x PS2 rate only as far as it LOOKS right. The sim aim may turn quickly; the
drawn soldier must not snap. Keep BF1 at 1x; choose a BF2 body rate between the current
5 and the measured PS2 20 rad/s for a 5 rad/s class, and say which. Ease the drawn body
yaw and apply ANIMA torso/aim limits. This supersedes the old mandatory 4x build/bar.

## Do
1. Measure saturation (|rx|=1) and target-pick-to-fire-cone time per BF2 map on current main.
2. Trial 10 rad/s (2x class rate) as a PROVISIONAL midpoint, then compare bounded alternatives
   inside 5..20 with the same replay; choose by measured barrel alignment and visible turn.
   Share AIMV's observer rather than inventing another facing frame. A separate fast sim-aim
   rate is permitted; record both rates and the presentation easing law/parameters.
3. Use shortest-arc yaw interpolation, preserve ANIMA torso limits, and compare stand,
   walk, strafe and crouch on BF1 and BF2. No switch from turn tuning may conceal AIMV's cause.

## Acceptance
- Per-map saturation and time-to-face table, plus selected BF2 body rate and exact easing
  parameters justified by a fixed replay's before/after frames. BF1's rate remains 1x.
- Mutation-checked scene: chosen turn time (pi / chosen rate plus measured tracker lag),
  BF1 unchanged, wrap at +/-pi takes the shortest arc, and the drawn yaw has no discontinuity.
- AIMV barrel-to-shot limit passes in each firing stance, no spurious firing spin over
  270 degrees/s on a fixed target; distinguish necessary target tracking from wrap faults.
- 8 seeds x 32 maps, the maintainer's Kamino scene, full applicable gates; classify every refusal.
