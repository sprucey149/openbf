---
id: HVYB
title: The PS2's explosive claim on missiles too (BF2, re-soldier-ai 24.5): one rocket / grenade user per team at a soldier, and at a hover / walker / flyer one per hull position (1 + turrets + passenger slots); the claim keyed by the vehicle, not the rider
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
Card HVYA step 4 read BF2's `FUN_002ab430` (`analysis/re-soldier-ai.md` 24.5 [O], docs 68afc2c): the chooser skips a
GRENADE or MISSILE weapon when the shooter's team already holds as many kind-4 claims on the target as its capacity - 1 for
a soldier / droid / building, 1 + mounted turrets + passenger slots for a hover / walker / flyer (entity+0x250). The port
claims thrown weapons only, one per team on any target (keyed by the target UNIT - for a vehicle the rider, so pilot and
gunner of one tank are two targets), and never a missile: every launcher of a team may fire at one tank at once, and two
rocket troopers at one soldier crowd.
## Evidence
- 24.5: capacity functions by class (`FUN_00249f30` soldier 1, `FUN_0026bb30` 1, hover `FUN_002219f0` / walker
  `FUN_0025b278` / flyer `FUN_0020ea80` = entity+0x250), the count built in each ctor (hover 130734 / 130751 / 130785).
- Port: `arsenal::think` `barred` = any team mate's live `ArmState::claim` on `target`; `claim` is set only when
  the pick is thrown.
- OPEN [H]: the claim's lifetime (`FUN_002e03e0` stamps `FUN_004240f8`; expiry `FUN_002dfec0` unread) and BF1's twin
  (`FUN_002387b0`'s callees) - read both before building (one fn + callers pass each).
## Acceptance
1. Read the expiry and BF1's twin into 24.5 (re_check + one verifier).
2. A test: three rocket troopers of one team, a tank with 2 positions at 40 m - at most 2 have the launcher picked at any
   tick; a soldier crowd - at most 1. Mutation-check (capacity 99 -> the test fails).
3. `HEAVY_CENSUS` (heavy shots at vehicles per round, all maps, 8 seeds) before / after on main's binary and the branch;
   gates show only the declared sim / sweep moves (sorted NOISE / EXPOSED / CAUSED if a fidelity metric refuses).
