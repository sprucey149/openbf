---
id: HAIS
title: Hover bots steer by the PS2's law: the shared FUN_002a54d8 with a strafe speed, FUN_002a45a0's squared turn curve, and FUN_00220528's own goal handling (the port's steer_to is OURS)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The port's hover driver (ai::ride::steer_to) is OURS / PROVISIONAL: a proportional turn and a throttle cone. The PS2's
hover AI FUN_00220528 calls the same steering FUN_002a54d8 the walker uses, with K+0x670 (ForwardSpeed), K+0x678
(a strafe speed), K+0x680 / K+0x688 (B2:135769), and its turn stick comes from FUN_002a45a0's hover path:
u = clamp(-l.x x (4.5 - clamp(rate, 1, 3)), +-1), turn = u|u|. Card WAI transcribed and verified those two functions;
what is left is FUN_00220528's own part (it edits the goal and halves the throttle on some condition, B2:135614-135760).

## Evidence
analysis/re-walker-ai.md sections 3-4 ([O]). lead_tmp/wai/wai_sample.py + wai_check.py replay a transcription against
live PS2 RAM tick by tick (walkers: 7504 of 7504 ticks); the same sampler takes a hover with its vtable pair.

## Closed 2026-10-03 (lane 2, report `reports/20261003_0245_card_HAIS.md`)
`analysis/re-hover-ai.md` (docs commit b073094; verifier 49 SUPPORTED / 0 / 0). Acceptance as run:
- FUN_00220528 transcribed (probe, MovingTurnOnly floor, unstick manoeuvre, x0.6, bail-out) and its BF1 twin
  FUN_001eedc0 opened AND replayed.
- Replay, driving ticks equal on throttle, strafe and turn: BF2 Endor 22617 / 22621 (99.98 %), BF2 Kashyyyk 15815 /
  15832 (99.89 %), BF2 Geonosis 6491 / 6492 (99.98 %), BF1 Endor 24089 / 24222 (99.45 %).
- PS2 speeder bikes lost with a rider, 5.00 min of battle: BF2 END1 4 (0 crashes), BF1 END1 16 (15 crashes).
- The build is VDROP's; its acceptance is re-based on these numbers.
