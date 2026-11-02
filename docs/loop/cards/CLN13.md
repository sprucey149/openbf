---
id: CLN13
title: Cleanse 18 (bake): hygiene, comments-are-claims, ledger and tests over the bake diff 920544c..ce8975d (25 commits, ~3,100 lines: anim_sets, cloth, turret_rig, soldier, sounds, entity_light, hud, map, main), CRATES.md counts
milestone: M0
value: 3
phase: cleanup
status: done
deps:
files: engine/CRATES.md, engine/crates/bake/src/anim_sets.rs, engine/crates/bake/src/anim_sets/bf2.rs, engine/crates/bake/src/hud.rs, engine/crates/bake/src/main.rs, engine/crates/bake/src/ogg.rs, engine/crates/bake/src/soldier.rs, engine/crates/bake/src/turret_rig.rs, engine/crates/bake/src/water_glb.rs, engine/crates/mungefs/src/soundbank.rs, engine/crates/mungefs/src/water.rs, scripts/cleanse_delete.sh, analysis/re-soldier-anim-layers.md, analysis/re-water.md
declares:
needs:
effort:
branch: cln13
resume: landed 7752d00 2026-10-19
created: 2026-10-19
owner: CLEANSE 67775
---
## Why
Cleanse 18, area bake (diff 920544c..ce8975d). Findings and their end states are in the report.

## Evidence
`engine/artifacts/lead_tmp/cln13/`.
