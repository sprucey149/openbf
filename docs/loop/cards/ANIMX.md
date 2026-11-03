---
id: ANIMX
title: Soldier animation, what is still unread: the first-person rig and banks, the low-detail animator beyond the 10 full ones, seat and turret poses (clips named by CRC), the thrown / tumble / death picks, BF2's combo (melee) sets
status: archived
issue: 69
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #69](https://github.com/sprucey149/openbf/issues/69).

## Why
Card ANIM answered the layers, the weapon sets and the action layer (analysis/re-soldier-anim-layers.md). Its section
10 lists what was not opened; none of it blocks the build cards ANIMS, ANIML, ANIMA, ANIMM.

## Evidence
- BF1 low-detail animator FUN_0022fd28 (one player, bank human_animation_lowrez); BF2 FUN_0029e548 / FUN_0029e8e0
  (shared players, humanlz); which soldiers are drawn at the detail level that takes one of the 10 animators.
- First person: banks human_animation_fp / humanfp, sbdroidfp, droidekafp (8-joint rig, re-anim-keys.md 2.1).
- BF1 states 0xc / 0xd and BF2 states 0xe / 0x11 / 0x12: a clip named by CRC (seats); 45 BF1 clips are in no table.
- A second BF1 driver of the animator (FUN_001be1e8, FUN_001be8b8).
- BF2 combo sets (sabre) are not compared by sets_bf2.py.
- KNOCK's RE (2026-10-07, `analysis/re-knockback.md` §2 "Clips") read BF1's thrown / tumble / death picks in
  FUN_0022a0a8: thrown = `thrown_flyingfront / back / left / right` by the velocity against the body, tumble =
  `thrown_tumblefront / back`, the get-up state = `thrown_bounce..soft` / `thrown_land..soft`, dead after one of
  those = `thrown_restfrontsoft / backsoft`, else `stand_death_forward / backward / left / right` by the hit; the
  1.93 s lock is `stand_getupfront`'s length. Still open here: frames (no picture pair), BF2's twin of the map, the
  spin value stored with the thrown clip, BF2's flinch (E+0x4e4, the `stand_hit*` ids 0x22-0x25 are a guess).

## Acceptance
One analysis note per question taken, [O] claims with the five checks and a verifier; a RAM count like ANIM's
(clips on the players of every posed soldier) where a table is claimed.

## Planner note
Planner 2026-10-21 20:50: the thrown pick for a grenade throw is GTHRW's (the maintainer 10-21); the sprint fallback is SPRCL's.
