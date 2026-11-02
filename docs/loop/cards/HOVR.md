---
id: HOVR
title: core::vehicle::hover — the BF2 hover integrator (spring points, lift, gravity, velocity-matching thrust, filtered yaw, world contact) on every hover/treaded class; the TAT1 skiff on a scripted pad
milestone: M2
value: 5
phase: build
status: done
deps: VENT,HVRE,VCLS
files: core/src/vehicle/hover.rs,core/src/vehicle.rs,core/src/sim/vehicles.rs,core/src/sim/step.rs,core/src/sim/state.rs,core/src/collision/walls.rs,core/src/fmath.rs,game/src/vehicle_class.rs,game/src/vehicle_pads.rs,game/examples/hover_drive.rs
declares: sweep
needs:
branch: hovr
resume: LANDED dae70c5 on 2026-09-28 (report reports/20260928_0923_land_HOVR.md). Follow-ups: HOVX (hull prims, water float, boost, bike jump), HCPW (contact sidedness / cp4 wedge).
created: 2026-09-25
---

## Why
The first vehicle physics: HVRE's transcription of the BF2 hover integrator as `core` code, deterministic, on every
hover/treaded class (PLAN locks BF2 ground handling; BF1-disc classes lose only their stick-based turn blend and the
strafe→turn fold — FEEL_NOTES line).

## Evidence
`analysis/re-hover.md` (HVRE) and `analysis/re-vehicles.md` §3.
