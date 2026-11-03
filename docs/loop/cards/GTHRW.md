---
id: GTHRW
title: The grenade throw is shown: the PS2's throw clip per class and game, played on the soldier in third person when he throws (the throw itself exists, card GREN; first person waits on card FPV)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 17:50 item 1): the grenade throw animation is not shown. RE done 2026-10-22
(`analysis/re-grenade-throw-anim.md`, docs f44cdcf): the port's animator starts an action only on a `Shot` event; a
throw raises `Throw`, at the release, so no clip plays. The PS2 starts the throw clip at the PRESS:
- BF1: the in-hand weapon becomes the grenade (set 3) and every fielded throw (strength 1.0, state 3) plays
  `human_grenade_stand_shoot3` (51 frames) on the upper player (the legs too while he stands); the grenade leaves at
  clip time 0.267 s; the action ends with the clip or when the channel returns to 0; the hands are empty.
- BF2: `OffhandWeapon` keeps the PRIMARY's set; a thermal plays that set's `shoot_secondary2` (rifle:
  `human_rifle_stand_shoot_secondary2_full`, 41 frames; bazooka / tool / pistol primaries their own upper clip); the
  legs only if he stood still at the start; release at clip time 0.50 s; the rifle stays in the right hand.
Measured live in both games (BF1 3 of 3 throws, BF2 2 of 2). First person needs a first-person view the port does
not have: card FPV.

## Acceptance
- A test (game/src/animator/tests.rs) that a throw starting in the press update sets the throw clip on the upper
  layer for its length: BF1 `grenade_stand_shoot3` on set 3 (legs joined standing, not running); BF2
  `shoot_secondary2` on the primary's set (legs only when standing at the start); mutation-checked (start it at the
  release instead -> fail).
  AUDIT 2026-10-22 (re-grenade-throw-anim §10.2): the legs rules are per game and the test needs a case for each -
  BF1 per update (movement id <= 2: legs leave when he moves, REJOIN when he stops; crouched still also joins); BF2
  only from the action's first update with movement id 0 (crouched still over 1 s = id 2 keeps the kneel; crouched
  under 1 s takes the stand clip), leaving for the walk / run when he moves, never rejoining. Mutations the PS2 rows
  reject: legs latched at the start, BF2's crouch ignored, BF2 rejoin on stopping (`lead_tmp/audit-gthrw/wf/M1-*`,
  `M2-*` replay the PS2 rows).
- A census over headless rounds (`ANIM_CENSUS=1`, BF1 NAB1 + GEO1, BF2 MYG1 + GEO1, seed 1): of every `Event::Throw`,
  the share whose thrower's animator holds the throw clip on the upper player at the release - 100 %; and no BF1
  throw followed by a rifle reload clip (the PS2's one-update reload blip, re-grenade-throw-anim 1.7).
  AUDIT 2026-10-22 (§10.1): on the PS2 the BF1 reload clip DOES play while L1 stays held past the fire state's end
  (or is pressed again during the reload) - to the release + 1 update, at most 2.5 s; BF2 never. Either port that
  (channel 1 kept while the throw button is held, state != empty) with a test, or record it as a named deviation in
  FEEL_NOTES; the census bar then reads "no reload clip after a throw whose button is up by the fire state's end".
- A picture strip per game of a player's throw in third person (pose_render or the windowed game under Xvfb) beside
  `engine/artifacts/lead_tmp/gthrw/ps2/bf1_throw_sheet.png` / `bf2_throw_sheet.png`, viewed and pasted in the report.
- The BF1-map timing call (re-grenade-throw-anim 7.5: the port releases at 0.51 s, BF1's clip at 0.267 s) recorded in
  FEEL_NOTES. AUDIT 2026-10-22 (§10.3): the upper player's own phase at the release is 9/51 = 0.300 s (BF1) and
  16/40 = 0.533 s (BF2), one update after the doc's 0.267 / 0.50; which one the PS2 draws is unread - use the phase
  and say so, or settle it with one live frame.
- AUDIT 2026-10-22 port note (N9): today `step_on_foot` takes the set from the HELD thermal while channel 1 is active
  (soldier_vis.rs:1144-1157), so a BF2 rocketeer throwing would get 'rifle', not his bazooka set
  (`human_bazooka_stand_shoot_secondary2`): the build must take channel 0's weapon's set for an offhand thrown weapon.

## Original (2026-10-21)
- RE: the clip the PS2 picks for a throw per weapon / class (both games), the layer and the time the grenade leaves the
  hand; then built: a picture strip per game of a throw (third person, and first person), the PS2's beside it; a test
  that a throw sets the throw clip on the upper layer for its duration.
