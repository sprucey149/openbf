---
id: FOLYM
title: Footsteps follow the legs: foley's walk / run choice reads the animator's movement clip (card ANIMM's chooser), not its own strafe-clip model
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
game/src/foley.rs (card FOLY) picks walk or run steps with its own model: strafe clips for a standing soldier (left always run, right walk under 5.0 / 5.5), the larger axis wins. re-soldier-anim-layers.md 3.5's erratum: a standing or crouching soldier has NO strafe clips; card ANIMM's `animator::choose` plays the forward / backward clip with the legs twisted (alert window -103.5 .. +67.5, else +-72 / +-108 while a forward clip plays). So the steps and the drawn legs disagree: a non-alert BF1 right strafe at 4.5 m/s draws runbackward but foley plays walk steps; a strafe's stride is foley's PROVISIONAL 1.3-1.55 m, not the played clip's.

## Evidence
- analysis/re-soldier-anim-layers.md 3.5 (erratum for re-foley 4.4), 7; analysis/re-foley.md 4.4, 8.2.
- ANIMM (branch animm, 8e8d33c): `animator::choose(game, vel, strafe, alert, max_speed, &mut Legs)` is a pure function; the alert is the animator's timer (`aim::Aim`), which foley does not have.

## Acceptance
1. Foley's step clip = the drawn lower clip's walk / run on every drawn soldier update of a headless round (ANIM_CENSUS + foley side by side: 0 disagreements on 4 maps x 2 seeds), or foley reads `Animator::body()` directly where the animator runs.
2. The stride per clip = half the clip's root travel per cycle at the played rate (the bake's root speed x cycle length); the live 8.2 spacings within 0.2 m.
3. A test: BF1 alert left strafe 4.4 -> run steps, right 4.5 -> walk steps; not alert, right 4.5 -> run steps.
