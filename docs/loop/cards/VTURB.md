---
id: VTURB
title: Building turrets as seat-only vehicles: every armedbuilding instance (BF1 com_weap_gunturret, BF2 tur_* and the armedbuildingdynamic turrets) spawned from the map's instances as a VehicleKind::Turret with its TURRETn gunner seats and guns (VTUR's machinery), enterable, never moving, its hull pool; drawn at its instance
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's 09-28 coverage note: NAB1 ships com_weap_gunturret instances and PLAN §Vehicles item (5) wants every building turret usable. Split from VTUR (2026-09-29) to keep that landing one commit-sized change.

## Evidence
analysis/re-vehicles.md §8: armedbuilding uses the same TURRET/TURRET1..8 sections, MountedTurret section and setter (B1 FUN_001c4db8 / FUN_00225f08; B2 FUN_001e8860 / FUN_00270770); no pilot seat; kind 8 = 'turret' to the enter search (B1:119710). [H]: the armed building's entity init (seat ring, +0xac bit 3) was not opened. VTUR landed the seat guns (core::vehicle::gun, core::sim::guns), the fire points (bake::hardpoints — already baked for armedbuilding classes in maps and common) and game::vehicle_pads::seat_gun.
