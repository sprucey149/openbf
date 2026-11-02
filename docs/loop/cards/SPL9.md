---
id: SPL9
title: game/src/assets.rs is 1,514 lines, over the 1,500 cap with no split card or waiver — split it (SPLIT_RECIPE) or waive it in scripts/size_waivers.txt
milestone: M0
value: 1
phase: cleanup
status: done
deps:
files: engine/crates/game/src/assets.rs
declares:
needs:
effort:
branch:
resume: MERGED into SPL8 (planner 2026-09-30 21:45): the game crate's two files over the cap, one behaviour-neutral session
created: 2026-09-29
---

## Why
Found by the scripts cleanse 2026-09-29 (card CLNS): `scripts/size_report.sh --check-map` lists game/src/assets.rs at 1,514 lines; no SPL card and no waiver name it (SPL7 has the four core/bake files, SPL8 viewer/scene.rs).
