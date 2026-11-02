---
id: VSEAT
title: Enter, drive, exit — the first drivable vehicle: ACTION + the PS2's aim-cone entry, the driver seat's pad drives the skiff, the vehicle camera, the skiff drawn at its transform, exit at the PS2's 9 points (TAT1)
milestone: M2
value: 5
phase: build
status: done
deps: HOVR,VTEAM,VDRAW,HCON
files: engine/crates/core/src/sim/seats.rs,engine/crates/core/src/sim/seats/tests.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/vehicle.rs,engine/crates/core/src/vehicle/hover.rs,engine/crates/game/src/walk.rs,engine/crates/game/src/ride.rs,engine/crates/game/src/vehicle_pads.rs,engine/crates/game/src/vehicle_class.rs,engine/crates/game/src/main/headless.rs,engine/crates/game/examples/vehicle_ride.rs,engine/CRATES.md
declares:
needs:
branch: vseat
resume: landed 2c5521fb 2026-09-28
created: 2026-09-25
---

## Why
M2's first player-visible result: walk up to a vehicle, get in, drive it, get out.

## Evidence
`analysis/re-vehicles.md` §5: the use input gathers objects within the soldier's radius + 0.5 and takes the best score
t + 16·perp² inside the aim cone (0 ≤ t ≤ 5.0 + target radius, perp² ≤ (radius + t)²); the first free PilotType-2 seat
in ring order (the driver first); instant enter and exit; exit placement = 9 candidates at the hull radius (behind,
left, right, diagonals, front, centre), each with a clear path, a 5 m ray up and a 10 m (BF1) / 15 m (BF2) ground ray.
§3.2 item 9: throttle / strafe / turn inputs.
