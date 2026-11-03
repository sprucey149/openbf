---
id: WCLMB
title: Does a PS2 AT-ST step onto HOT1's cp3 bunker? WALK's leg rays (terrain + buildings within LegRayHitLength above the feet, mask 0x90) lift the port's AT-ST ~7 m onto hoth_bldg_bunker bunker1 in two ticks; check on PCSX2 (BF2 HOT1, walk an AT-ST into the bunker) and, if the PS2 blocks it, find what does (the contact pass's place in the frame, C7's push-out, the layer the bunker is in)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
game/examples/walker_drive bf2_hot1 imp_walk_atst (WALK, 2026-09-29): turning right from the pad, the AT-ST steps +3.2 m then +4.4 m onto hoth_bldg_bunker bunker1 (cp3, instance at -214, -2.7, 192), walks on its roof and drops off. Each lift is under LegRayHitLength 5 m, which re-walker §6 allows; the order of the contacts against the legs is [H] (§2), and moving the contacts first did not change it.

## Evidence
WALKER_TRACE=1 walker_drive bf2_hot1 imp_walk_atst: y -7.82 → -4.61 → -0.32 → +0.67 between x -204.2 and -208 (terrain -7.6); body 199 of the collision world (Layers 3, aabb -226..-201 × 173..210, top y 1.05).

## Acceptance
A PCSX2 run (capture job): an AT-ST driven into the cp3 bunker's side on BF2 HOT1; record whether it climbs. If not, a card for the mechanism.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

## Note from WALKB1's RE (2026-10-07)
- BF1's foot-on-a-building rule differs from BF2's (`analysis/re-walker.md` §16): BF1 supports a foot only on a face
  with n.y >= 0.8 and then ALSO applies the generic push-out; BF2 supports from n.y >= 0.2 and returns without one.
  The port runs BF2's on both games' maps.
- Card WCON moves the walker's contact pass between the fall and the legs (the PS2's order). Re-run this card's
  bunker step-up after it lands.
