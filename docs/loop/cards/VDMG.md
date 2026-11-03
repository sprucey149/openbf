---
id: VDMG
title: Vehicle damage and death: one hull pool through VehicleScale, collision damage, the damage-effect bands, death = the _exp class + chunks in the same tick (no wreck)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Vehicles must take damage, burn and blow up like the PS2's.

## Evidence
`analysis/re-vehicles.md` §6 [O]: HealthType vehicle (every vehicle ctor) → the ordnance's VehicleScale; ONE hull pool;
up to 10 visual damage slots (DamageStart/StopPercent, DamageEffect, DamageAttachPoint, scale, inherit velocity —
effect while health is between Stop and Start); at 0 health the `ExplosionName` class detonates and the chunks fly in
the same tick, no wreck; the hover's impact damage (§3.2 item 10); no self-repair on the discs. [H] there: occupants
killed or ejected, chunk lifetime — resolve or take a PROVISIONAL default and say so.
