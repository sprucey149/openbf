---
id: TBRD
title: Bots board a building turret only when they are right at it: today a bot boards from the turret's radius + 5 m + 2 m slack and appears in the seat from several metres away - bring that down to walking up to it
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/ride.rs, engine/crates/core/src/ai/man.rs
declares: sweep
needs:
effort:
branch: tbrd
resume: landed 9f19c5a1 2026-10-20; audit incomplete 2026-10-24, see reports/20261024_2340_audit_TBRD.md and private continuation
domain: vehicle
created: 2026-10-20
first: 7
---

## Why
(the maintainer, 2026-10-20, playing) Some bots are about 10 ft from a turret and 'teleport' into it. Asked: lower the maximum distance to something reasonable.
Cause (read in the code): a bot is seated the moment it is within `ai::ride::board_reach` = the vehicle's seat radius + `ENTER_REACH` (5 m, the PS2's reach past the target's radius, FUN_00241b98 0x40a00000) + `BOARD_SLACK` (2 m, card WAI2's allowance so a bot walking at an AT-AT's origin could board it - it applies to EVERY vehicle, building turrets too). So a bot boards from radius + 7 m. The PS2 itself seats a bot as soon as it is inside radius + 5 m (re-soldier-ai.md 7.5: UnitBoardAgent waits to be inside the vehicle's reach, no walk test), so the PS2 'teleports' too, from a shorter distance; the port's extra 2 m is ours.

## Evidence
`core/src/ai/ride.rs` `board_reach`; `core/src/sim/seats.rs` ENTER_REACH / the player's aim-ray entry test; `core/src/ai.rs` BOARD_SLACK (card WAI2); `analysis/re-soldier-ai.md` 7.5; `core/src/ai/man.rs` (card TURR: bots man building turrets).

## Notes

- 2026-10-20 (build, lane 1): the card's premise was off - the PS2's board agent is NOT radius + 5 m: B2 FUN_002c42d8 enters at the two body spheres + 1.5 m (lui 0x3fc0 at 0x2c4564), B1 FUN_00244848 + 0.5 m (lui 0x3f00 at 0x244a4c), 3-D (re-soldier-ai 7.2 / 7.5). Built for building turrets; walkers / hovers keep WAI2's. Measured (`engine/artifacts/tbrd/README.md`, 8 seeds x 32 maps): turret boards xz median 9.92 -> 4.26 m (BF1) / 9.90 -> 5.29 (BF2); boards from over 2 m above / below the gun BF1 BES2 190/320 -> 0, RHN1 191 -> 0, NAB1 212 -> 0. BF2 KAM1 / MYG1 / KAS2 boarded 394/104/27 -> 391/98/35. OURS (D19): a bot under / over a turret (inside its XZ reach, centre > 2 m off) gives the claim up at once. Landed 9f19c5a.
