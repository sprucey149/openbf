---
id: ANIMA
title: The soldier turns his gun to where he aims: the alert timer and its clip rows, the torso aim pose (spine and ribcage) with the PS2's limits, the head-only look when not alert
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
With the two layers of ANIML a soldier still fires level whatever the pitch of his aim, and has one idle. On the PS2 a
soldier who fires, charges or holds a target is ALERT for 2.0-2.5 s: he plays the alert clip rows and his torso is
turned onto the aim line; otherwise only his neck and head look that way.

## Evidence (analysis/re-soldier-anim-layers.md 3.4, 6.4, 7, [O])
- BF1: timer on the animator, armed to 2.5 s (2.0 while still running) by a shot, a charge or a live weapon target,
  not for the tool and sabre sets; alert rows ALERT[weapon][action] (rifle: idle_emote and runforward only); a 0.4 s
  layer ramp from 0.2 at each change; aim weight 0 -> 1 over the first 0.5 s and 1 -> 0 over 0.5 s after it runs out.
- BF2: timer on the soldier, 2.5 s, zero for a class without the alert flag (tool, melee); the standalert_ /
  crouchalert_ blocks; aim type from the upper clip's AimType (sanm): Head for plain stand / crouch clips, Torso for
  alert clips and weapon actions, faded over the clip's BlendInTime.
- Aim pose: yaw limited to +-0.6283 rad from the leg twist, pitch +-1.0996 (prone: no downward pitch). BF1 eases 15 %
  per update, BF2 by min(1, 4.5 dt). Torso: BF1 rotates spine a / spine b / ribcage by 1/8, 4/8, 8/8 of the angle
  (absolute, children re-derived from the ribcage); BF2 by 0.125 / 0.375 / 0.5 (cumulative). Head: BF1 neck 2/8,
  head 5/8; BF2 neck 0.25, head 0.375.

## Note from ANIML (landed 644695a, 2026-10-05)
- The two players are in `game/src/animator.rs`: `Rig::body_slots` is where the alert rows go (BF1 `alert[weapon][id]`,
  BF2 block + 3); `Animator::step` owns the action and would own the alert timer; `soldier_vis` builds `AnimInput`.
- Open from ANIML's acceptance 3: the super battle droid's idle pair is NOT a same-angle comparison
  (`engine/artifacts/lead_tmp/animl/pair_sbd_idle.png`: PS2 from behind with the forearms at waist height, the port's
  unit sheet from the side with them raised). Take a PCSX2 frame of an idle SBD from the side, or pose the port from
  behind, and compare the forearm height before calling the droid's carry matched; an alert row may be the difference.
- 2026-10-05 planner: value 4 -> 5: the maintainer's second ask on ANIM (2026-10-04, 'the sbd ... sticks its arm straight out and fires from its hand') is not yet shown against a same-angle PS2 frame (the note above from ANIML); that check is the first step of this card. Undo: `loopctl.py set ANIMA value=4`.
