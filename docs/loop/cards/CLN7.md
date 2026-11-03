---
id: CLN7
title: Two game-side referees that do not test what they say
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
CLN7. [TODO — ADDED 2026-09-23 (lane 2) by the cleanse's test-prune pass; touches a GOLDEN REFEREE, so it is a queue entry rather than a cleanse fold-in; `game/src/viewer/look.rs`'s test module (`viewer.rs`'s until SPL1) is shared with OCF / 9 / W3 / 7 — take it while none of them is live] **Two game-side referees that do not test what they say** — (a) `game/tests/golden_tat1.rs` renders `load_scene` + `upload_scene` only, while `engine/goldens/tat1_overview.png` is written by the game binary's own capture path (`scripts/goldens.sh` → `run_headless` → `load_and_upload`, which merges `common_bf1` — the ≥ 12 drawable common instances `tests/load_scene.rs` counts), so the referee compares two different scenes and spends part of its tolerance on the difference; give the test the binary's scene (`apply_mode_layer("con")` + the `common_bf1` merge) and read the diff with `--nocapture` before and after — NO golden may move. (b) `viewer/look.rs`'s `no_shipped_map_fogs_its_water_at_a_step` (`viewer::look::tests` since SPL1) compares a table of literals with literals (fog near 400/1000/3000 vs 90/100/150) and reads no bake, so a rebake that moved them could not fail it; move the check into `game/tests/water_look.rs`'s per-map loop (`lighting.fog.near >= look.fog_begin` wherever `fog_begin` is `Some`) and delete the literal table. GATE: `./ci.sh` 0; `scripts/gpu_tests.sh` 0 with `git status --short engine/goldens` EMPTY; the new water_look assertion shown failing once on a doctored value (paste), then passing. Files: `engine/crates/game/tests/golden_tat1.rs`, `engine/crates/game/src/viewer/look.rs` (test module only), `engine/crates/game/tests/water_look.rs`.

- **Acceptance (planner 2026-09-30 08:30, from the entry's GATE):** (a) golden_tat1's per-pixel diff vs `goldens/tat1_overview.png` printed with `--nocapture` before and after, the after diff smaller (same scene as the binary); (b) the new water_look assertion shown failing once on a doctored fog value (paste), then passing, and the literal table gone; `./ci.sh` 0; gpu gate with `git status --short engine/goldens` empty; nothing declared.
