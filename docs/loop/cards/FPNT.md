---
id: FPNT
title: Two small weapon rules to the PS2's, one session and one gates run (FPNT + AMMO): (1) every soldier's shot leaves the PS2's fire point - a stance table shared by both games (stand 1.58 m up, 0.06 m right, 0.24 m ahead at the spawn pitch; crouch 1.15 m; prone 0.40 m) in place of the port's 75 % of the capsule, bots included, and every soldier spawns at pitch -0.07 rad; (2) a primary weapon's reserve is WeaponAmmo N = one clip loaded + N-1 in reserve, below 1 = endless
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The port fires every bolt from feet + 75 % of the capsule height (`core::unit::muzzle_height`, PROVISIONAL: 1.425 m
standing) along the aim. On the PS2 the fire point is a table by stance, the same numbers in both games, used by
players and bots alike (`analysis/re-aim-assist.md` section 10.2 step 2 [O]): F = M*T0 + d*T2.z + left*T2.x +
(d x left)*T2.y with T0 = (0.06, 1.7, 0) / (-0.1, 1.2, 0) / (0.06, 0.6, 0.4) and T2 = (-0.12, -0.10, 0.25) /
(-0.12, -0.05, 0.25) / (-0.12, -0.20, 0.15) for stand / crouch / prone (x along the soldier's LEFT, y up, z forward).
A bot's bolt leaves that point along its own tracked aim (no tilt, no ray). Card AIMA gives the human player this
point with the camera rule; this card gives it to everyone and so moves every sweep and arena hash.
The PS2 also starts every soldier at pitch -0.07 rad (B1 FUN_001ff1e8, B2 FUN_00234a68); the port at 0.

## Evidence
- `analysis/re-aim-assist.md` section 10.2 (the table, B1:124337-124365 / B2:172990-173007) and 10.3 (the spawn pitch).
- PS2 RAM, 132 save states: the aimer's position is 1.5828 m above the feet, 0.060 m right and 0.2424 m ahead at
  pitch -4.01 deg, to 0.5 mm (`engine/artifacts/lead_tmp/aima/model_bf1.py`, `model_bf2.py`).

## Do
1. `Unit::muzzle()` (the bolt's origin) = the table point for the unit's stance and aim; keep a separate eye for
   perception and the seats if they read `muzzle()` today (say which callers moved).
2. The spawn pitch -0.07 rad for every soldier.

## Acceptance
- A core test: F at stand / crouch / prone at pitch -0.07 rad equals the PS2's numbers (stand 1.5828 m up, 0.060 m
  right, 0.2424 m ahead; crouch and prone by the same formula) within 1 mm.
- The sweep referee: `scripts/sweep_seeds.py` on main first, then on the branch - finished rounds per map not lower by
  more than the 8-seed noise (+-3), and the leak census per map x class not higher; paste both tables.
- The arenas' kill / time-to-kill rows before and after (a bolt 0.16 m higher changes hits on low cover): state the
  change and that it is the PS2's.

## Planner 2026-10-03 03:35 - AMMO merged in (the batch rule: two value-3 builds, both `sim; sweep; arena`, both in `core/src/unit.rs`)
**Two commits, one gates run.** Commit 1 is this card's fire point (Do 1-2 above). Commit 2 is AMMO:

### Part 2 - the primary's reserve (card AMMO, 2026-10-02, verbatim)
Card GREN (d4886ba) built the class slots with the PS2's ammo rule (`analysis/re-grenades.md` 2.5 [V, M]: B1
FUN_002838d8 - reserve clips = class ammo - 1, endless when the class value is below 1; RAM: a BF1 stormtrooper's
rifle ammo 5 = reserve 4.0 + clip 1.0, its pistol endless) for every HELD slot (`WeaponState::with_clips`) and left
the weapon in the hands on the old rule (`WeaponState::new`: N spare clips, 0 = none).
Build: `UnitSpec::build` uses `WeaponState::with_clips` for the primary too (`spare_clips` becomes the class's
clips); drop the PROVISIONAL note on `with_clips`.
Acceptance (run and paste):
- A test over the baked class db: all_inf_soldier spawns with clip 50 + reserve 250 (HUD 300), imp_inf_storm_trooper
  50 + 200; a class whose primary has WeaponAmmo 0 has an endless reserve (its HUD count is the infinite sign).
- PWUP (afb7926) caps a pickup's ammo at "the reserve the weapon spawned with" (PROVISIONAL; the PS2's cap is max
  clips less the loaded one): with this rule the two are the same number - say so and drop that PROVISIONAL, or name
  the difference.
- The sweep: finished 32 / 32 kept; `sweep_seeds.py` on main, after commit 1 and after commit 2, so each part's move
  is its own row (bots that run dry sooner; a bolt 0.16 m higher).
Undo the merge: `loopctl.py set AMMO status=todo resume=` and cut this section.

## Planner refill boundary (2026-10-24 13:06 UTC)
Part 2 owns `core/src/sim/pickups.rs::reserve_cap`: its primary branch currently
uses `spec.spare_clips`, while held slots subtract one. Changing only the spawn
constructor would let a pickup or station refill the primary above the promised
spawn reserve. Include this file in the build and exercise the real give_ammo path.
Additional acceptance: after spending ammo and repeatedly receiving ammo, the two
finite primary examples remain capped at 250 and 200 reserve respectively; a
WeaponAmmo 0 primary remains endless; the held-slot cap stays unchanged. Mutating
the primary cap back to N clips must fail a refill check. Existing baked-class,
HUD, per-part sweep and arena acceptance stays required. This is current source
inspection, not new PS2 remeasurement; restore the worker's inputs first.
