---
id: FOL2
title: Foley, the rest: the surface from the prop under a soldier's feet (metal / stone / wood groups), which soldiers the PS2 gives an animator (it stepped 15 others on UTA1 where the port's on-screen rule steps 392), BodyFall
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
FOLY (d51ff74, 2026-10-02) plays steps and stance sounds from the baked table with two surfaces only (terrain,
water). Left open, in order of what a player hears:
1. **Prop surfaces** (re-foley 3): a soldier on a prop takes its class's `FoleyFXGroup` (baked class prop hash
   2714356677: metal 416, stone 383, wood 58 map classes). Today a metal walkway on Naboo or Utapau plays the terrain
   row; a sub-level with no terrain group (bf2 kam1cw, bf1 bes1 / kam1) starts on its first group (correct there:
   every floor is metal). Needs: the body under the feet (a `ray_down` that returns the body index; the instance of
   each body is `Scene::collision_parts`' third value, as `collision_body_labels` uses it) -> class -> group.
   PS2 RAM on UTA1: 2 of 35 soldiers on `metal_foley` (re-foley 8.3).
2. **Who steps** (re-foley 4.3): the PS2 steps a soldier that holds an animator - 4 of 44 on NAB1, 1 of 35 on UTA1 in
   the two states; the animator is taken in the draw (B1 113774-113777) and dropped when another soldier takes it
   (116789-116799). The port's rule is 'on screen within 50 m' (`game::foley::WHO`). FOLY's referee
   (`lead_tmp/foly/pad_foly.ron`, 105 s, player at his spawn): others' steps 2 (NAB1) / 392 (UTA1) / 177 (KAM1)
   on screen, 147 / 2480 with everyone within 50 m; the PS2's own runs 107 / 15. Bounded RE: how many animators
   there are and who gets one (nearest? on screen?), then match it.
3. **BodyFall** (re-foley 4.5): a thrown soldier touching the ground after 0.3 s - needs the thrown state (grenade
   knock-down); the slot is baked.
4. Smaller: the strides of the non-forward clips are the PS2's measured spacing, not the clips' root travel
   (`STRIDE_BACKWARD` 1.65, `STRIDE_SIDE_RUN` 1.55, `STRIDE_SIDE_WALK` 1.3: read the root travel of
   runbackward / the strafe clips as re-anim-keys 5 did for forward); the clip's direction is 'the larger axis'.

## Acceptance (run and paste)
- Unit test (mutation-checked): a soldier standing on a body whose class has `FoleyFXGroup` metal_foley plays the
  metal row, back on the terrain the terrain row.
- Referee (`SOUND_LOG=<csv> game --map assets_baked/bf2_uta1 --mode con --mission missions/bf2_uta1_con_cw.ron
  --cheat invincible --headless --walk --walk-replay lead_tmp/foly/pad_foly.ron`, `lead_tmp/foly/referee/summary.py`):
  a walk across a UTA1 metal platform logs `cis_inf_droid_run_metal_*`; the others' step count over the 105 s
  against the PS2's 15 (UTA1) / 107 (NAB1) with the animator rule, before and after.
