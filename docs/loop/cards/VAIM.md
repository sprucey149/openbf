---
id: VAIM
title: A target in a vehicle or a turret is aimed at with the VEHICLE's error block (the PS2: the controller hangs off the vehicle - size factor 2.0-4.2 for a turret, 3.9-12.5 a walker, 7.6 and more a flyer, the vehicle's velocity and top speed, no stance factor); the port takes the seated soldier's size and a velocity of zero, so its bots aim 2 to 25 times tighter at 9-10 % of their targets (audit of CLSE2, re-soldier-ai 21.3)
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why
Card CLSE2 made a BF2 bot's aim error "the TARGET's own block, with its model's size". For a target that sits in a
vehicle or mans a turret the PS2's block is the VEHICLE's, and it is several times larger; the port keeps the seated
soldier's. So the port's bots aim much tighter than the PS2's at riders, gunners and pilots - 9-10 % of all the
time a bot spends aiming on the 17 BF2-rule maps. Before CLSE2 the size was 1.0 for everyone; with it a rider's is
his soldier's 0.48-1.02, a step further from the PS2's 2 to 12.

## Evidence (`analysis/re-soldier-ai.md` 21.3; `engine/artifacts/lead_tmp/audit-clse2/wf/K5/`, `ps2/`)
- On the PS2 the controller (whose first 0x80 bytes are the block) hangs off the controllable the unit drives: the
  block's owner word is the vehicle's controllable on 256 of 267 mounted bots in 49 library states and the remote
  droid's on 11, the soldier's own on 0; live on Kamino's turrets 1,813 of 1,851 thinks. A passenger's or gunner's
  seat resolves to the parent vehicle.
- The size factor s (2 x `FUN_002a76b8`, 18.3's kinds), read at rest in RAM (`wf/K5/tab.txt`): `tur_bldg_laser` and
  `tur_weap_built_gunturret` 2.009, `tur_bldg_tower` 2.353, `tur_bldg_recoilless_lg` 4.157; `rep_walk_oneman_atst`
  4.000 (the one class on the disc that AUTHORS the size, key 0x4217f84f = 2.0), `imp_walk_atst_jungle` 3.863,
  `cis_walk_spider` 8.599, `imp_walk_atat` 12.460; flyers 4 x the cached size (snowspeeder 7.628, tri-fighter
  9.749); hovers and treads near rest 1.57 (speeder bike) to 4.12 (snail tank), which fit 0.4 x hypot(class
  +0xbe4, +0xbe8) [H]; a remote droid 0.173. Foot soldiers 0.48-1.02.
- The velocity at the block (t+0x10) is the vehicle's (up to 94 m/s for a flyer, 28 a hover) and the top speed its
  own: a speeder bike at 28 m/s reads s x m = 2.498 = 1.561 x 1.6. The stance slot of a vehicle's controllable
  returns the constant 1.0. An EntityFlyer also halves the decay and takes the cap blend(4.0, 1.2) (18.1, 18.2).
- The port (main bc10b82): `seen_aim` reads `e.consts.aim_size`, `stance_factor(e.soldier.stance)` and
  `e.soldier.vel` over the soldier's MaxSpeed whatever `e.mount` is; `Sim::ride` zeroes a rider's `soldier.vel`
  every tick, so his block never grows and sits at the floor; a rider keeps the stance word he boarded with (189 of
  101,532 mounted rows not standing).

## What to do
1. RE, bounded (the verifier workflow): the vehicle's slots +0x4c (velocity) and +0x68 (top speed) per kind -
   walker, hover, flyer, turret - which class fields they read; the hover's +0xbe4 / +0xbe8 (which keys) and
   18.2's x 0.4 (slot +0xe0 = 7) and x 2 (= 2) by class; whether a human's vehicle follows the same path (the
   PlayerController's block). The agent's table is the check: predict every row of `wf/K5/tab.txt` from the class
   data.
2. Build: a mounted unit's block thinks with its vehicle's velocity and top speed; `seen_aim` at a mounted target
   takes the vehicle class's size factor (a table per vehicle class as `roster::AIM_SIZE`, held to the baked models
   by a test, or the class key), stance 1.0. The flyer terms when a flyer is flown.

## Acceptance
- The size factor of every vehicle and turret class the port fields within 2 % of the PS2's RAM value
  (`wf/K5/tab.txt`; a class with no RAM row is listed as such), by a test.
- Tests through the sim: a rider's block grows when his vehicle changes velocity (0.455 x the change over the
  vehicle's top speed) and stays at the floor on a straight run; a bot aims at a rider with the vehicle's size and
  no stance factor; each fails under its mutation.
- Measured, main against the build, 8 seeds x the BF2-rule maps with vehicles or turrets (Hoth, Geonosis, Kamino,
  Felucia, Kashyyyk, Mygeeto, Endor, Tatooine): hits per bolt and damage per aimer-second at MOUNTED targets by
  vehicle class, before and after; rounds finished; kills a minute; vehicles destroyed a round. No bin of the
  foot-target tables moves past |t| 2.4.
## Note from VTURD's RE (2026-10-13, lane 2, `analysis/re-turret-draw.md` 2.3)
- Where a BOT gunner's aim enters a turret's gun: the seat's AIMER (its own task: B1 `FUN_0026d748`, B2 the
  headerless update at 0x3021f8) steers at the turret's aim point, and for a non-human occupant the wanted
  direction is first replaced by the output of an AI function - B1 `FUN_00236700(dt, out, unit, aimer, want)`
  (B1:191960-191968), B2 `FUN_002a83e0` (MIPS 0x302660-0x302694) - before the rate-limited step. Neither was read
  by VTURD; it is the hook to open for "how exactly a seated bot aims".
