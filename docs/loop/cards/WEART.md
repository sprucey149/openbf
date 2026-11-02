---
id: WEART
title: SSD wear, test binaries (after WEARC): game's 37 integration test files each link a whole-engine test binary (~130 MB each; 77 test executables = 5.9 GB in target/debug/deps), so every ci run after a core edit still writes ~1.1 GB - merge them into one or a few test binaries
milestone: M0
value: 4
phase: build
status: done
deps:
files: engine/crates/game/tests,scripts/gates.py
declares:
needs:
effort:
branch:
resume: 2026-10-16 planner: MERGED into WEARY (duplicate; the maintainer's first: 2); text under WEARY's '## Merged: WEART'.
created: 2026-10-16
alpha: yes
---
## Why
Card WEARC (landed eb331e7, 2026-10-16) stopped ci linking examples: one warm ci.sh after a core edit 1.76 -> 1.14 GB written (cgroup io.stat, report reports/20261016_0014_card_WEARC.md). What remains is mostly test-binary links: 77 test executables, 5.92 GB apparent in debug/deps after one ci run (largest: game lib tests 153 MB, golden_hud 132 MB, golden_spawn 131 MB, skinned_glb 130 MB, battle_replay 130 MB).
