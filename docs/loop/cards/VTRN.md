---
id: VTRN
title: Vehicles turn too slowly with the controller (the maintainer 10-21): the PS2's turn rate per vehicle class at full stick (deg / s, its response curve and dead zone) against the port's on the same stick input, hover / speeder / walker / flyer, both games - fix where the port is slower
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
The maintainer (INBOX 2026-10-21 17:50 item 5): vehicles turn more slowly than in the original games. Check that the stick is not
read through the soldier's look curve or a dead zone twice, and that the vehicle's own turn law (HVRE, VWLK, the air
integrator) is fed the full stick.

## Do
1. The PS2's numbers: a scripted full-stick turn for N s per class (PAD), degrees from RAM, plus half stick for the curve.
2. The port's: the same input through the real input path (controller -> the vehicle), headless.
3. Fix where the port differs, per class.

## Acceptance
- A table per class: PS2 deg / s at full and half stick, the port's before and after; every class within 5 % of the PS2
  or a named reason. A test per vehicle family: full stick for 1 s turns the PS2's degrees.

## Notes
- 2026-10-22, card LOOKR's RE (`analysis/re-look-ramp.md`): **first suspect, from the code.** The port's full-stick timer
  for a human driver (`core/src/sim/seats.rs` `driver_input`: `full = turn^2 + pitch^2 >= 0.9`) reads the look stick
  AFTER `input::LOOK_SCALE` 0.80 (`input/src/lib.rs` 374-375), so a pure right-stick turn is at most 0.80 and 0.64 < 0.9:
  the timer only runs on a diagonal, and a human hover / walker keeps turning at its PitchRate start (`hover.rs`
  `yaw_target`, `walker.rs` `input`: PitchRate + (rate - PitchRate) x timer). The maintainer's five snapshots: the largest |rx| the
  sim received is 26213 = 0.80 x 32767. The PS2 tests the +-1-clamped stick before any shaping (re-hover 5.2). LOOKR's
  build carries the raw full test for the soldier and is specified to feed `driver_input` the same one; measure here
  before and after it.
