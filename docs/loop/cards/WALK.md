---
id: WALK
title: The walker, built: BF2's walker integrator (re-walker §2-§9) — the clip clock and root motion, the camera-relative turn, the legs, the probes and contacts — AT-ST first; the rigid model moves with it until WSKN
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The AT-ST and 14 other walker-family classes spawn on pads (VPAD) but cannot move: the port has no walker integrator.
BF2 walker handling (PLAN's ground lock) is now transcribed statement by statement.

## Evidence
analysis/re-walker.md ([O]; verifier 44 SUPPORTED / 4 PARTLY folded / 0 NOT; live PS2 clip clock x0.9000 vs x0.9021):
§2 frame order, §3 input, §4 locomotion (clip clock, turn tree, clip state machine, root motion), §6 legs, §8 wall
probes, §9 contact, §11 class conversions + ctor defaults + the disc census, §14 build notes and test numbers.

## Notes
- The AI steering (FUN_002a54d8) is not transcribed: bots keep the port's driver path (VAI / VTAI) writing
  throttle / strafe / turn into the walker input.
- PROVISIONAL (re-walker §14.5): the contact pass's place in the frame, C7's push-out distance, the stomp damage,
  bone 0x16e27226.
- Walkers with two or more leg pairs (spider, AT-AT, AT-TE) need the pair order (front/back) from their WalkerLegPair
  lines for §6's pitch sign; the AT-ST has one pair.- **Audit (VWLK, 2026-09-29, PARTLY; banner in re-walker.md, 3e4c742):** everything this card builds for BF2's walkers
  held, and a live BF2 HOT1 AT-ST confirmed it: per-tick yaw steps of exactly 2·0.5/30 rad, 5.02-5.12 m/s at full
  stick. Two corrections for BF1 maps:
  - §13's BF1 speeds are wrong. BF1 covers z·30/N, so its tauntaun ran ~19.8 m/s (live 18.1), not 21.75. Write the
    FEEL_NOTES line from card WALKB1, not from §13's old table.
  - BF1-disc classes run through BF2's turn tree with the ctor flags and 0/0 YawLimits. That behaviour is untraced
    (WALKB1). BF1 NAB1's kaadu is a fielded walker too (card KAAD).
  Build the BF2 AT-ST first, as planned; WALKB1 decides the BF1 classes.
