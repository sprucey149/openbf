---
id: VPRE
title: P7's behaviour-neutral prerequisites: `CollisionWorld.bodies` private (with a real staleness stamp), and `Hit::Unit` becomes `Hit::Target(Target::Unit)`
milestone: M2
value: 5
phase: cleanup
status: done
deps:
files: engine/crates/core/src/collision/world.rs,engine/crates/core/src/projectile.rs
declares:
needs:
branch: vpre
resume:
created: 2026-09-25
---

## Why
PLAN §DESIGN SPECS "P7 DESIGN — entity shape" routes every hit through `enum Target { Unit, Vehicle }` and puts vehicle hulls
in `CollisionWorld`. Two pieces need no RE and change no behaviour, so they can go before VEH0's answer and keep M2 moving.

## Evidence
- `core/src/collision/world.rs:286` `pub bodies: Vec<Body>` while the broadphase's staleness stamp is only `bodies.len()`:
  an in-place edit keeps a stale grid (analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 1; cleanse 2026-09-17 C1).
- `Hit::Unit` has 4 uses in `crates/` (grep 2026-09-25).
