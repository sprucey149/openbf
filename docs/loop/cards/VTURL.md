---
id: VTURL
title: A building turret's turn law: a full-circle turret stops at 180 degrees and its gun swings round the long way (the look is clamped at the seam, the aimer walks back through 0 - seen now that card VTURD draws the gun), and the PS2's stick-to-rate law with the aimer's limits in the turret's aim frame replaces the port's PROVISIONAL seat look (re-turret-draw 2.2, 2.3)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card VTURD (2026-10-13) draws a building turret turned to its seat's `GunState`. That makes the port's PROVISIONAL
seat look (`core/src/vehicle/gun.rs`) visible, and it is not the PS2's:
- **A full-circle turret stops at 180 degrees and its gun swings round the long way.** Traced on the branch (the sim is
  main's): BF2 Hoth's dish turret (`YawLimits -180 180`), the player seated, right stick held from tick 100 - look
  -76.4 deg at tick 120, -152.8 at 140, then HELD at -180.0 from tick 160 on, while the aimer goes -129.9 (tick 160),
  -53.5 (180), +22.9 (200): its wanted yaw flips to +180 and it walks back through 0. The bolts already went that way
  on main; now the drawn gun does too. Frames: `lead_tmp/vturd/ride/hot_sheet.png` (ticks 140-200: the dish turns to
  face the camera). On the PS2 a turret whose limits span 270 degrees or more is treated as turning full circle
  (`analysis/re-turret-draw.md` 3.3) and the default limits are read as "no limit, wrap" (2.1).
- **The turn law** (re-turret-draw 2.2, 2.3 [O]): the stick drives a RATE (`-stick x MaxTurnSpeed / MaxPitchSpeed`,
  slewed by `TurnFilter` / `PitchFilter`: BF1 exponential, BF2 linear) inside `YawLimits` / `PitchLimits`; a separate
  aimer chases the aim POINT at `AimerYawRate` / `AimerPitchRate` inside `AimerYawLimits` / `AimerPitchLimits`, which
  are relative to the turret's AIM FRAME (the dish and the anti-aircraft gun author `AimerYawLimits 0 0`: the gun
  cannot yaw off the turret). The port moves the look at the aimer's rates inside the seat's limits and keeps the
  aimer's angles in the model frame.

## Evidence
`analysis/re-turret-draw.md` 2.2, 2.3; the PS2's rows `lead_tmp/vturd/ps2/*_rows.bin` (yaw, pitch, the stick's rate
and the aimer's own yaw / pitch per game update: a ready referee for the law); the trace above
(`game/tests/turret_pose.rs`'s world, pads `ly` full + `rx` full).
