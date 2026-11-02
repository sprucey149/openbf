---
id: VAI
title: Vehicle AI: bots board (the PS2's enter request), drive with RE_LOG's vehicle nav constants, fire the seat weapons, dismount
milestone: M2
value: 5
phase: build
status: done
deps: VSEAT,VPAD
files: engine/crates/core/src/ai/ride.rs,engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/route.rs,engine/crates/core/src/ai/state.rs,engine/crates/core/src/sim/seats.rs,engine/crates/core/src/vehicle.rs,engine/crates/game/src/vehicle_class.rs
declares: sweep
needs:
branch: vai
resume: landed f56e5ed9 2026-09-29
created: 2026-09-25
---

## Why
M2 is done only when AI drives every vehicle class a map ships.

## Evidence
`analysis/re-vehicles.md` §5 (AI boarding: B1 FUN_00244848 → FUN_0023c418 queues an enter request consumed through the
soldier's Enter slot; BF2 AI commands 0x13/0x14 in FUN_0027bae0 / FUN_0027caf0), §10 (RE_LOG's vehicle nav constants:
hover arrival radius ForwardSpeed × 0.5, others 4.0 m; look-ahead |velocity| for hovers, 20 m for walkers; flyers 3-D;
AISizeType masks), §9 (the AI's Target filter setter FUN_0023d1f8). Design: PLAN P7 — `ai::think` dispatches on
`Unit.mount`; boarding decisions belong to openbf's own AI (PLAN §AI).
