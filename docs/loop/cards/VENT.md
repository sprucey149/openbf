---
id: VENT
title: The vehicle entity skeleton: `Sim.vehicles`, `VehicleHandle`, seats, `Unit.mount`, the hash append — PLAN's decided P7 shape, adjusted by VEH0
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The P7 entity shape is DECIDED (PLAN §DESIGN SPECS "P7 DESIGN — entity shape": a second entity kind, occupancy on both
sides, `Target` enum, hulls from `coll`/`prim`, seat routing by pad slot). Every later M2 card builds on it.

## Evidence
PLAN.md §DESIGN SPECS, the P7 bullet (fields, hash order, AI dispatch on `Unit.mount`). VEH0's RE may amend fields
(damage regions, seat kinds, spawn pads) — read `analysis/re-vehicles.md` first.

## VEH0 amendments (2026-09-25, `analysis/re-vehicles.md` §5-§7, §11)
- **Health:** ONE hull pool per vehicle (the PS2 has no damage regions — §6), so `health: Health` (HealthType::Vehicle)
  replaces PLAN's `Vec<RegionHealth>` and `Target::Vehicle(VehicleHandle)` carries no region.
- **Seats:** the PS2's ring order — the vehicle's own seat (Pilot, the BODY section), then one Gunner per TURRETn section
  in index order (≤ 8), then Passengers (≤ 4): `Seat { kind: Pilot | Gunner(u8 section) | Passenger, occupant }`; at most
  13 seats. A seat-only vehicle (a building turret, §8) has no Pilot seat and no movement.
- **Team:** from the spawn pad (§7), overridden by the pilot's team while occupied (as PLAN says); **no respawn field on
  the vehicle** — the pad owns respawn (VPAD).
- Kinds: `VehicleKind::{Hover, Walker, Flyer}` stays (treaded and commandhover are Hover; commandwalker is Walker plus a
  command post; carrier is Flyer plus cargo — §2, §4).
