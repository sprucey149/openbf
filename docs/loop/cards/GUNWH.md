---
id: GUNWH
title: BF2's AI driver waits for the player: a new pilot of a hover / walker holds its wait up to 15 s from the start while the human player is on foot on its team within 12 m, or within 150 m facing the vehicle (30 deg) - even with every seat full (analysis/re-driver-wait.md 5, B2 FUN_002cd928 / FUN_002cd718); the port's BF2 drivers drive off
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
On the PS2, BF2's AI driver waits for the human player when the player is near or looking at the vehicle (re-driver-wait.md 5: the hold is tested before the seat count, up to 15 s from the wait's start; +0x130 >= 0 is the player alone in all 179 BF2 save states). The port's BF2 drivers never wait for the player. Player-visible on every BF2 map with hovers or walkers.

## Evidence
- analysis/re-driver-wait.md 5 (B2 FUN_002cd610 ctor: +0x64 = start + 15.0; FUN_002cd928 update; FUN_002cd718 the scan: on foot, same team as the driver, d <= 150 m and (d < 12 m or the player's forward within 30 deg of the vehicle)); event 0x27 at 7-10 s has no line (re-unit-chatter 4).
- Builds on GUNWT step A (the wait itself).

## Acceptance
- Scene (BF2 rules, a 2-seat hover): a bot takes the pilot seat with the player on foot 8 m off: the hover stands 15 s (+-1 think) and leaves at once when the player gets in; the player 30 m off facing away: it leaves after the 4 s window; facing it from 100 m: it holds to 15 s. Mutation-checked.
- The sweep's boarding / stall counts on BF2 maps sorted NOISE / EXPOSED / CAUSED; a headless round's player is parked (the census must not change where the player stands far from vehicles).
- BF1 untouched (a BF1 driver does not wait for the player: re-driver-wait.md A5).
