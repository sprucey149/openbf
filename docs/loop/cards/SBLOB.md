---
id: SBLOB
title: Soldier / pickup drop-shadow blob (player_shadow, RedShadowBlob): both PS2 games bind a blob texture in the soldier's and the pickup's draw; the port draws no shadow under a unit - RE the blob and measure it with a PCSX2 knock-out
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by VSHD's RE (2026-10-16, `analysis/re-vehicle-shadow.md` 6): besides the vehicle volumes, both PS2 games load a
`player_shadow` texture for a blob drawn under units - class names `RedShadowBlob`, `DropShadowRenderable` in both ELFs.
The port draws no shadow of any kind under a soldier (`grep` of render / game: only a PROVISIONAL comment in
`game/src/viewer/pickups.rs`). If every soldier on the PS2 stands on a dark blob, every battle frame shows the gap.
Not measured yet: whether the blob is visible and how dark - so value 3 until the knock-out says.

## Evidence
- B1: the string `player_shadow` (0x532938) bound in FUN_002013a0 (113457, 114119 - the soldier's live draw per
  re-lod / USKN) and (0x5318d8) in FUN_001f3cb0 (103907, the pickup draw: a 5 m ray down, re-powerups 4).
- B2: FUN_00239bb0 (the soldier's per-frame draw, re-side-alpha 5) binds it at 155383 and draws it under a rule not yet read
  (v = +0x34 of an entry looked up by hash 0x8afdf5e4: nothing at v <= 0.3, factor v - 0.3 up to 1 at 1.3; the sun
  vector `DAT_0060c6e0 + 0x50`; 155386-155420); also FUN_00225860
  (pickup), FUN_001ed920 and FUN_002310f0 (classes not named).

## What to find (phase re, bounded)
- The blob's quad (size from the unit's radius?), placement (ray down, offset along the sun), alpha / fade by height,
  colour, blend and bucket; which units get one (soldiers, droidekas, pickups; the player?) in both games.
- A PCSX2 knock-out: point the blob's draw or its texture binding at nothing for one grab (RULES "empty its manager's
  list") on a BF1 and a BF2 standing soldier.

## Acceptance
`analysis/re-<topic>.md` with the rule and the knock pairs (BF1 + BF2): the blob's pixel share under one soldier and its
on / off brightness ratio, measured; then a build card or a close (if invisible).
