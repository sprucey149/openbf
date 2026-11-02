---
id: ANIMS
title: Bake the PS2's soldier animation sets: per (skeleton, weapon set) the upper-body and lower-body clip of every animation id with frames, loop, root speed and blend time - BF1's nine AnimationBank sets and fallback table, BF2's (family, class) sets and the disc's sanm chunk
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/anim.rs,engine/crates/mungefs/src/sanm.rs,engine/crates/bake/src/soldier.rs,engine/crates/bake/src/anim_sets.rs,engine/CRATES.md
declares: rebake=side
needs:
effort:
branch: anims
resume: landed d582ff52 2026-10-05
created: 2026-10-05
alpha: yes
domain: render
---
## Why
The maintainer, 2026-10-04: "shooting, walking, and standing animation is not the exact same for every unit. It varies depending on
weapon they're holding, so like where it sits on their shoulder ... the sbd doesn't hold a gun so it sticks its arm
straight out and fires from its hand, not holding some invisible gun." The port bakes one list of 34 human_rifle clips
into every soldier (bake/src/soldier.rs:156 SOLDIER_CLIPS), so a launcher trooper, a pistol holder, an engineer and
every droid stand, walk and fire as riflemen. This card bakes what the PS2 plays; card ANIML then draws it.

## Evidence (analysis/re-soldier-anim-layers.md 2, 5, 6, 8, [O]; verifier 41 / 10 / 0)
- BF1: 9 weapon sets by the weapon class's AnimationBank (rifle, bazooka, pistol, grenade, tool, lightsabre, sbdroid,
  bdroid, cannon); six tables (base, special, per-weapon movement, per-weapon special, alert, upper actions) built by
  FUN_0022d828 with the fallback table {bazooka->rifle, pistol->tool, grenade->tool, tool->pistol, sabre->tool,
  sbdroid->tool, bdroid->rifle}. The six human_animation banks hold 208 clips, the tables use 151.
- BF2: a set per (skeleton family, weapon animation class): 151 {upper, lower} pairs, names
  <family>_<class>_<animation>[_upper|_lower|_full], filled along the parent chains (classes rifle <- bazooka, tool;
  tool <- pistol, melee; families under human; CustomAnimationBank / ComboAnimationBank rows). Blend times, Loop,
  AimType, SyncType, BlendTimeTo: the sanm chunk of INGAME.LVL (192 Animation scopes).
- The two resolvers are re-implemented and checked against PS2 memory: engine/artifacts/lead_tmp/anim/sets_bf1.py
  (979 of 979 slots) and sets_bf2.py (665,176 of 665,176 slots over 176 battle states, 38 sets); sanm.py decodes the
  chunk (258 of 258 live records equal it). Port those two files; do not re-derive the rules.
