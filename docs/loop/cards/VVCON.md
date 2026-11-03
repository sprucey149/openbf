---
id: VVCON
title: Vehicles against vehicles: the PS2 pairs vehicle hulls with each other (BF1's per-tick pass moves the vehicle list against layers 1|4, re-collision-layers 4) and a hull impact deals damage credited to the OTHER body's driver (B1 FUN_001ee3e0, re-vehicle-death 10.1 item 9); the port's vehicle contacts see only the map (HullFilter::Static, collision/world.rs: 'which the port does not model yet') - measure how often two port vehicles overlap, then RE the PS2's response and build it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The port's hovers and walkers take contacts only against the map's bodies (`HullFilter::Static` in `core/src/collision/world.rs`,
whose comment says the PS2 pairs vehicles with each other and the port does not model it). On the PS2 two vehicles collide
(re-collision-layers 4: the vehicle list is moved against layers 1 | 4) and the hull impact damage carries the other body's
controller as the attacker (B1 FUN_001ee3e0 B1:99986-100017, verified by card RIDK's workflow, claim C6): a ram by a manned
vehicle is a kill credit for its driver. Filed by card RIDK (RE, 2026-10-16); not measured on the port yet.

## Evidence
- `analysis/re-vehicle-death.md` 10.1 item 9 (RIDK) and `analysis/re-collision-layers.md` 4 (the pass list).
- The port: `collision/world.rs` HullFilter doc comment; `Sim::impact_vehicle` is fed only by `hover::step` against the
  heightfield and the world (`vehicles.rs` tick_hovers).

## What to do
1. Measure on main (8 seeds x 32 maps): unit-seconds with two live vehicles' hull spheres interpenetrating, per map and by
   vehicle class, and how many of those are moving (> 2 m/s) - is it a visible gap or a rare one?
2. If it matters: RE the PS2's vehicle-vehicle response (the narrowphase pair, the hover's contact FUN_001ee708 with another
   hull, the damage factor from the other body), one PCSX2 trial driving a hover into a parked one.
3. Build the contact and the damage with the other driver credited.

## Acceptance
- Step 1's census table (per map, moving vs parked) in this card, with the script on disk.
- If built: a test - two hovers driven into each other do not pass through; the impact damage credits the other driver;
  8 seeds x 32 maps main beside the branch (recycled, noprog, vehicle deaths by cause).
