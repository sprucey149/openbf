---
id: VDMG
title: Vehicle damage and death: one hull pool through VehicleScale, collision damage, the damage-effect bands, death = the _exp class + chunks in the same tick (no wreck)
milestone: M2
value: 4
phase: build
status: done
deps: VENT,VPRE
files: engine/crates/core/src/vehicle.rs,engine/crates/core/src/sim/vehicles.rs,engine/crates/core/src/sim/combat.rs,engine/crates/core/src/sim/state.rs,engine/crates/game/src/vehicle_class.rs
declares:
needs:
branch: vdmg
resume: landed e7ed805 2026-09-26; the blast + chunks are VDTH; hulls join the bolt ray with HOVR/VSEAT; report reports/20260926_1225_build_VDMG.md
created: 2026-09-25
---

## Why
Vehicles must take damage, burn and blow up like the PS2's.

## Evidence
`analysis/re-vehicles.md` §6 [O]: HealthType vehicle (every vehicle ctor) → the ordnance's VehicleScale; ONE hull pool;
up to 10 visual damage slots (DamageStart/StopPercent, DamageEffect, DamageAttachPoint, scale, inherit velocity —
effect while health is between Stop and Start); at 0 health the `ExplosionName` class detonates and the chunks fly in
the same tick, no wreck; the hover's impact damage (§3.2 item 10); no self-repair on the discs. [H] there: occupants
killed or ejected, chunk lifetime — resolve or take a PROVISIONAL default and say so.
