---
id: ANIML
title: A soldier's upper body follows the weapon in his hands and keeps shooting while he walks: two clip players and the PS2's bone masks in game::animator, the set of the weapon in hand, the shoot / charge / reload layer over the movement clip
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-04: "When walking and shooting, the units need to do the shooting motion while still walking forwards.
Right now when walking it doesn't change animation to shooting." and "the sbd doesn't hold a gun so it sticks its arm
straight out and fires from its hand, not holding some invisible gun." game/src/animator.rs plays ONE clip on the
whole body (select, lines 81-86: shoot and reload only when the soldier stands still) from the rifle list.

## Evidence (analysis/re-soldier-anim-layers.md 3, 4, 7, [O]; verifier 41 / 10 / 0)
- Two clip players into one kept pose. LOWER (hips, legs): the rifle's movement clip whatever the weapon (BF2: the
  set's lower slot). UPPER (BF1 13 bones: spine a / b, ribcage, neck, head, both clavicles, upper arms, forearms,
  hands; BF2: every bone but root, pelvis, legs): the weapon set's version of the movement clip, or its shoot /
  charge / reload clip while one runs. Standing still (movement id 0-2) the action plays on the legs too (BF1: the
  same clip, blend + 0.5 s; BF2: the action's lower slot, only if he stood still when it started).
- The action: every shot restarts the shoot clip (blend 0.01 s); reload 0.5 s (BF2: rate-fitted to the weapon's
  reload time); it ends when its clip has run. No action clip in a jump, fall, roll, landing (BF1) or sprint (BF2);
  it is drawn over the jet hover (both) and BF2's soft landing. BF1's grenade: shoot / shoot2 / shoot3 by the hold
  fraction (<= 0.4, <= 0.7, above).
- Blend: BF1 maps pull the kept pose toward the clip by t / T each update (overwrite at 1; a bone the clip has no
  track for takes its rest pose on overwrite and keeps its own on a blend); BF2 maps crossfade linearly over T =
  the old clip's BlendTimeTo for the new one, else min(new BlendInTime, old BlendOutTime).
- PS2 memory: rifle_stand_shoot over rifle_stand_runforward_alert, pistol_stand_shoot over rifle_stand_runbackward,
  human_rifle_stand_shoot_full over human_rifle_standalert_runbackward; 35 idle super battle droids on
  sbdroid_stand_idle_emote over rifle_stand_idle_emote (engine/artifacts/lead_tmp/anim/ram_animators_bf1.csv,
  ram_animators_bf2.csv). PS2 frame of the droid's carry: engine/artifacts/lead_tmp/unit-audit/pcsx2/10_sbd_idle_crop.png.

## Note from ANIMS (landed d582ff5, 2026-10-05)
- The data is baked: `engine/assets_baked/side_bf{1,2}/soldiers/<model>.sets.ron` = `bake::anim_sets::SoldierSets`
  (move the type to `manifest` so `game` can read it): `upper_bones` / `lower_bones` by name; `clips` (name = the GLB
  animation's name, frames, BF1 loop + blend); `root_speed` by clip name [x, z] m/s; `bf1` = the six tables as clip
  indices (`base[stance][action]`, `special`, `w[stance][weapon][action]`, `alert[weapon][action]`,
  `u[stance][weapon][upper]`, `s[weapon][special]`, weapons in `bake::anim_sets::bf1::WEAPONS` order - a weapon
  class's `AnimationBank` picks the set, `bf1::weapon_set`); `bf2` = per family, weapon class -> 268 slots
  (`2 * pair + layer`, pairs as `bf2::base_name`), with `bf2_animations` (the `sanm` loop / blend in / out / aim /
  sync / BlendTimeTo per pair and layer).
- BF2 clip names carry `_upper` / `_lower` / `_full`; look clips up with `animator::find_clip`, never `c.name ==`.
- Before landing a change the bake or the clip lookup: overlay the scratch bake in the worktree and run
  `goldens.sh --check` (the branch gpu gate renders the old shared bake).
