---
id: VBAKE
title: Bake the SIDE vehicle models, chunks and collision (speeder bikes, tanks, AT-STs, fighters) — rigid hovers/flyers first, then the skinned walkers' blend-bit pass
milestone: M2
value: 4
phase: build
status: done
deps:
files: engine/crates/bake/src/vehicle.rs,engine/crates/bake/src/map.rs,engine/crates/manifest/src/lib.rs,scripts/rebake.sh,engine/crates/game/examples/vehicle_gallery.rs
declares: rebake=side
needs:
branch: vbake
resume: landed 0a81fc97 2026-09-26
created: 2026-09-25
---

## Why
Only map-local vehicle models are baked; every SIDE vehicle (speeder bikes, fighter tanks, AAT, STAP, AT-ST, X-wing, …
— 51 of the 53 classes) has no model, chunks or hull in `assets_baked`.

## Evidence
`analysis/re-vehicles.md` §1 (the classes per map), §6 (CHUNKSECTION: up to 9 chunk models per class), §10 item 3 (the
side bake writes soldiers and weapons only; skinned walkers meet `bake::soldier::segment_blends`' refusal of the 0x02 bit
and of a 0x04 word with 0x08/0x40/0x80 — `rep_walk_atte` carries 0x02, PLAN HOLD 1 note).
