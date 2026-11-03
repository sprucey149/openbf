---
id: AIMV
title: Firing soldiers point the drawn barrel at the target: census moving/standing gun alignment and held-fire spins, trace the pose/facing/wrap cause and keep a mutation-checked referee
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer's 2026-10-22 report: firing while walking/strafing holds the gun sideways, some
shooters do not face their targets, and held fire sometimes spins a soldier through 360
degrees while bolts keep travelling correctly. A muzzle start alone does not establish aim.
The drawn barrel must point along the shot's aim, within the PS2's measured allowance.

## Evidence
User report in INBOX 2026-10-22 09:10, item 4. No black-box snapshot or baked assets are
analysis/re-soldier-anim-layers.md (ANIMA), ANIML card, BTURN and analysis/re-look-ramp.md.
Read a supplied snapshot first; identify BUILD commit and schema before judging it.

## What to do
1. On current main, reproduce in a separate headless snapshot directory (never overwrite
   The maintainer's last.json). Record firing ticks with face/aim/yaw, stance, target and trail.
2. Add a headless pose observer using the actual drawn weapon/barrel transform and the
   corresponding shot line, in the same frame and time. Census every fielded class,
   both games, bots/player, stand/walk/strafe/crouch. Print sample counts and uncovered
   rows. Separate intended weapon spread from mean gun aim; measure the PS2 allowance
   rather than choose a guessed angular threshold.
3. Enumerate firing yaw travel over 270 degrees in each rolling 1 s window (unwrap
   correctly), list every case and cause. Trace facing vs aim split, upper/lower layers,
   limits and +/-pi wrap before selecting the root fix; coordinate BTURN's presentation.
4. Fix the demonstrated cause and keep the observer as a meaningful regression referee.

## Acceptance
- Before/after table: class x game x controller x stance, firing samples, barrel-to-shot
  angle max/p95 and counts over the measured PS2 allowance. Every required row has
  samples or an explicit asset/stance reason; ordinary missing coverage is not a pass.
- Black-box build/seed/ticks for each reproduced symptom, and all >270 degrees/s firing
  episodes with cause. On a fixed target with held fire: zero unintended spins after fix.
- Scene tests run the real pose and fire paths in all four stances, with independent
  barrel and shot directions; each stays within the PS2 allowance. Mutations removing
  moving upper-body aim, using body yaw as aim, or breaking wrap each fail the referee.
- Full applicable CI, sim/sweep/arena/bake/GPU/golden gates; view changed frames beside
  the PS2 references before landing. No inferred pass from missing data.

## Notes
Phase measure until the symptom/census and measured allowance exist. Re-phase build
with exact files/declares once ownership is proven. This is one root-cause task, not
authorization to change goldens or tolerances. Current cloud prerequisites block it.
