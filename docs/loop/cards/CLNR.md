---
id: CLNR
title: Cleanse render (2026-10-02): the render crate since its last cleanse (de7f771, 2026-09-24), read hardest on its ~6,000 changed lines (frame, shader.wgsl, draw, pipelines, grass, leaf, bolt, particles, hud, filter, point_light, terrain_light, specular); stale comments, dead code, ledger labels, tests, CRATES counts
milestone: M0
value: 2
phase: cleanup
status: done
deps:
files: engine/crates/render,engine/CRATES.md,scripts/size_waivers.txt
declares:
needs:
effort:
branch: clnr
resume: landed b1cffcc9 2026-10-02
created: 2026-10-02
---
## Why
The cleanse rotation's `render` area (LAST_CLEANSE). Scope: `engine/crates/render` since its last cleanse (de7f771, 2026-09-24): 37 commits.

## Evidence

## Notes
