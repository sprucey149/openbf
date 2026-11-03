---
id: VSND
title: Vehicle sounds 1: a seat gun's shot plays the GUN's FireSound (today the rider's rifle), and the engine loop - the soundparameterized record baked, 1-4 looping layers per piloted vehicle with gain and pitch from the PS2's graphs (speed, acceleration, health, turn-on / turn-off time)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
SNDC (`analysis/audio-census.md`): the port plays no vehicle sound at all. PS2, 105 s: NAB1 970 vehicle / turret
weapon starts, 338 audible engine-layer starts, 88 kaadu steps, boarding and collision; UTA1 172 weapon starts, 232
engine layers (AT-RT), 269 walker steps (`com_weap_layered_at_step`, `atst_leg_up`), turret whir.

## Evidence
- Engines: `EngineSound = <x>_engine_parameterized` (20 classes BF1, 25 BF2) is a record of kind 0x12EBD555 that
  names a `soundlayered` of 2-4 looping properties (AAT: 0x842C3A54, 0xE0539C6C, 0x5ACF7D70; templates 25 / 45 -
  450 m). On the PS2 they play in the 39 handle voices only (B1 `*(0x5220c4)`, B2 `*(0x60b654)`, stride 0x150) -
  `snd_live.py state lead_tmp/sqveh/nab1_cis_aat_pilot.p2s --game bf1 --planet NAB` shows the player's AAT's three
  layers. How the parameter (speed? throttle?) drives pitch and the layer mix is not read [H].
- Weapons: `cis_weap_hover_aat_cannon_fire` 434 starts, `com_weap_turret_fire` 364, `imp_weap_hover_fightertank_*`,
  the beam's `FireLoopSound`. The port's log has no vehicle weapon name in 120 s (376 `vehicle_hit` events).
- Other keys: `FootstepSound0-5` (walkers, kaadu), `TurretYawSound` / `TurretPitchSound` (`turret_whir_*_lp`),
  `TurretActivateSound` / 0x1ADA38C1 (`vehicle_equip`, heard on boarding), `VehicleCollisionSound`, 0x069134E5
  (`atst_leg_up`), 0xEA603E52 / 0x042DABAE (droideka fold / unfold, jet pack on / off).

## Do (re, then build)
RE the parameterized record (B1 loader `FUN_003579a0` / create `FUN_00356bd8` for kind 0x12EBD555) and what feeds it;
which update starts walker footsteps. Build: the vehicle weapons' `FireSound` first (known behaviour, no RE), then
engines as looping voices that follow the vehicle, steps, turret whir, boarding, collision.
