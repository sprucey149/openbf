---
id: HOVX
title: Hover, BF2's boost + energy (re-hover §5.1 I5, §6.3, §6.7) and the speeder-bike jump (§6.9) — the hull bodies split to HHUL, the water float to HWAT
milestone: M2
value: 4
phase: build
status: done
deps: HOVR,HCON
files: core/src/vehicle/hover.rs,core/src/vehicle/hover/tests.rs,core/src/sim/seats.rs,game/src/vehicle_class.rs,game/tests/vehicle_class.rs,game/examples/hover_drive.rs,CRATES.md
declares: sweep
needs:
branch: hovx
resume: landed 24016cab 2026-09-28
created: 2026-09-28
---
## Why
HOVR landed only the spring points meeting the world (exact for the TAT1 skiff, which lists no hull prim). Tanks, bikes and the command hovers need their hull spheres, the water float, boost and the bike jump before VSEAT's driving feels right on every map.
## Evidence
analysis/re-hover.md [O] §4 S3, §6.3, §6.9, §7 K6-K9; HOVR report engine/artifacts/loop/reports/20260928_0923_land_HOVR.md.
