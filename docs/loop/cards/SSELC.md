---
id: SSELC
title: The PS2 death camera (BF2 FUN_001bc188): phase 1 eases in ~0.5 s to a view of the body from 6.22 m (the line from body + 0.8 m through a point fixed at the death 1 m back / 1.5 m up, 5 m past it); after an ENEMY kill, from 1.5 s, phase 2 moves onto the body and looks at the killer, following him; 6.0 m casts against the world; no FOV change; the spawn map opens over phase 1 again
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
What the PS2 shows between a death and the spawn screen (planner 2026-10-19 on card SSEL; the maintainer's "turn to the killer?").
The port keeps the live chase camera on a dead player. The PS2 has a dedicated death camera, read in code and measured
live: `analysis/re-spawn-flow.md` §0 item 8 and §6.5 (second pass, 2026-10-20).

## Evidence (`analysis/re-spawn-flow.md` §6.5)
- Start: when the dead player's member lets go of the unit, the soldier update hands the camera manager the unit and
  the killer (`E+0x190`, the killer's member record; 0 for a hero class and after a suicide); the death camera starts
  from the player camera's current matrix (`FUN_001ba658` -> `FUN_001bc5f8`).
- Set-up: look-at = body position + (0, 0.8, 0) (refreshed every update); from = body position at the death + (0, 1, 0)
  + (the camera's back direction made horizontal and unit, y = 0.5) (FIXED at the death).
- Update (timer from 0): t < 1.0 s: weight t/1.0, from -> look-at; 1.0..1.5 s: weight 1; > 1.5 s with a killer: weight
  min(1, (t - 1.5) x 0.5), from = the body point, look at the killer's vehicle or else his unit at its +0xd0 (his feet +
  1.70 m). Two 6.0 m casts from `from` +- 0.75 m up, backward along the view; eye = from - dir x (6 x the smaller
  fraction - 1). Each update the camera moves by the weight toward that pose (rotation slerp, eye lerp).
- No FOV call: the screen keeps the last FOV (65 deg; 55.39 deg = tan_v 0.40237 after an enemy kill, one player-camera
  update on the corpse).
- Measured (Kamino 16 AI a side, Hoth): hold distance 6.221 / 6.219 m (code: 6.2207); the ease's remaining fraction
  0.967 / 0.812 / 0.360 / 0.047 = the product of (1 - k/30); phase 2 on the first update after 1.5 s in 7 of 7 enemy
  kills with rows; the view points within 0.1-4.0 deg of a living enemy 2.5 s after the death in 7 of 8; the turn in
  phase 1 (10-177 deg) comes from the body being thrown past the fixed point (0.3-1.8 m); suicides and an explosion
  kill: no phase 2.
- At the spawn screen (4.07 s): the unit's destructor re-starts the camera with no unit and no killer from the player
  camera's stale matrix: phase 1 replays (with the same turn), never phase 2.
- The original first-pass table (suicides, kept): 3.10 m / 2.57 m / -14.0 deg at 0, 3.72 / 2.98 / -19.5 at +0.13 s,
  4.68 / 3.64 / -27.1 at +0.27 s, 5.14 / 4.08 / -32.1 at +0.43 s, ~5.2 / ~4.1 / -32..-34 from +0.6 s.

## Do
- The killer: the port's `Event::Kill { shooter, victim }` (`core/src/sim/state.rs`) already names him; a shooter equal
  to the victim (a self-kill), a kill with no shooter unit (an explosion credited to no one, a fall) and a hero class
  give no killer (re-spawn-flow §6.5). `game`: a `DeathCamera` beside the chase camera with the set-up and update above, the casts
  through the collision world (the port's ray / sphere cast), driven from the death until the spawn screen; then replay
  phase 1 from the pre-death pose under the spawn screen (SSELB dims it).
- Constants 1.0 / 1.5 / 6.0 / 0.8 / 0.75 / 0.5 are ELF words (re-spawn-flow §6.5); name them so.

## Acceptance (RUN and pasted)
1. A camera test in `game` (headless, a scripted death with no killer): the eye's distance to body + 0.8 m after 0.6 s
   = 6.2207 m +- 0.01 with nothing behind; the remaining fraction after 2 / 4 / 8 updates 0.967 / 0.812 / 0.360 +- 0.01
   (30 Hz; the port's tick rate converts the weight by time, say how).
2. The same with a killer: no change before 1.5 s; from 1.5 s the eye ends within 1.2 m of the body point and the
   view's forward within 2 deg of the killer's head point by 2.3 s; a killer that moves is followed.
3. A suicide and a hero-class death: phase 1 only.
4. A golden or a picture pair of the death view (Hoth) beside `lead_tmp/ssel/t2_hot1_death/death_sheet.png` and a
   phase-2 frame beside `lead_tmp/ssel/t9_kam1_ai16_kill/fr/sheet_d1.png`.

## Notes
- CFOVM's spawn / death camera FOV question is answered: the death camera sets no FOV (re-spawn-flow §6.5); the
  spawn screens at the level start use the camera shots' 48 deg (§1.1); the FEL1 0.40237 state is the one-update ease.
- Open [H] (re-spawn-flow §11): who writes `E+0x190`; what the `+0xb8` test excludes (likely a self-kill); why the one
  explosion kill kept no killer.
