---
id: ROLLM
title: BF2's roll is moved by the drag mover (FUN_00248560, ControlSpeed row 5 = 1.4 m/s2 of stick, no target): stick held it keeps 10.5 m/s and covers 8.78 m (port slows to 9.29, 8.21 m), the stick sideways bends it 0.38 m (port 0.08) - the port still reads the roll row as a target speed
milestone: M3
value: 3
phase: build
status: done
deps:
files: engine/crates/core/src/soldier.rs
declares: sim; sweep; arena; leak
needs:
effort:
branch:
resume: MERGED into BF1BK (planner 2026-10-02 09:30): same file core/src/soldier.rs, same declares (sim; sweep; arena; leak), both from the AIRC audit with PS2 rows in lead_tmp/audit-airc - BF1BK carries this card's text and acceptance, one build pays the gates once
created: 2026-10-02
domain: physics
---

## Why
AIRC fixed the jump row of ControlSpeed (0.03 is an acceleration factor, not a speed). The roll row (0.02) has the
same meaning and the port still reads it as a target speed: a roll with the stick held slows down where the PS2's
holds its speed, and the stick cannot bend a roll. Found by AUDIT AIRC (2026-10-02).

## Evidence
- Code [V]: B2 `FUN_00240bf8` `case 5:` (B2:160662-160680) calls `FUN_00248560(dt, turn, stickFwd x class[0x790] x
  class[0x748], -stickStrafe x class[0x7b0] x class[0x748], E)` - the drag exp(-0.012 |v| dt) plus the stick ADDED as
  an acceleration (0.02 x 70 = 1.4 m/s2), no target (`analysis/re-stance-roll.md` §4.3b).
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

## Notes
- Shares `core/src/soldier.rs` with CSPD (the per-class ControlSpeed table) and JARC: the planner may batch.
- Since AIRC the jump row's columns 1-2 multiply Acceleration: a hero's `jump 0.1 0.1` (CSPD's evidence) is 7 m/s2.
