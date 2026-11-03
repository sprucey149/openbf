---
id: TSTX
title: The test suite's cost: 1,168 tests run on every landing (ci.sh: fmt, clippy, build, cargo test --workspace) in ~7.5 min, but 2 test binaries are 64 % of the test time (announcer_ps2 93 s, battle_replay 65 s) - run the binaries in parallel, tier the slow ones by what they depend on, and list tests that no longer earn their place
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-02: "1166 tests are a lot. Do they run every time code changes are made? Similar to goldens, should this be modified so not all tests run every time - only tests that would realistically be changed? As this project gets bigger, are there tests that really aren't necessary anymore and we could delete?"

## Facts found (Claude, 2026-10-02, JITR's gate run `lead_tmp/jitr/gates/ci.txt`, main 045a98b)
- Every landing runs `engine/ci.sh` whole: fmt check, clippy -D warnings, build, `cargo test --workspace`. JITR's ci took 17:09:55 -> 17:17:30 (7.5 min, ONE ci at a time across lanes under `ci.lock`). Goldens already skip by `goldens/deps.toml` (GSKIP); the unit and integration tests never skip.
- 62 test binaries, 245 s of test run time, run ONE AFTER ANOTHER by `cargo test`. `tests/announcer_ps2.rs` 92.8 s (4 tests), `tests/battle_replay.rs` 64.8 s (1 test), `tests/all_maps_load.rs` 17.6 s, `event_messages` 13.3 s, `global_sound` 8.4 s. The ~1,100 unit tests together take under 20 s - their NUMBER is not the cost.
- The 43 ignored tests are the GPU tests (`requires a GPU adapter`), run by the gpu gate on the NVIDIA box - not dead.

## Result (Claude chat session, 2026-10-02, branch tstx 5731440; evidence `lead_tmp/tstx/`)
- Same tree, warm target, under `ci.lock`: `cargo test --workspace` 267 s; `cargo nextest run` 53 s (wall = the slowest test,
  battle_replay 51 s); + `[profile.dev.package."*"]` and `[profile.dev.package.core]` opt-level 2: 12 s (slowest test
  7.5 s, mungefs tat2_envmap_is_a_cube_map). Rebuild after touching core 19 -> 13 s, after touching game 18 -> 12 s; the
  one-off dependency rebuild ~90 s. Also tried game at opt-level 1: no gain, slower rebuilds (29 s) - not taken.
  A whole warm `ci.sh` (fmt, clippy, build, tests, doctests): 12-13 s (3 runs). Counts 1167 / 0 / 43 identical both ways
  (`t2_times.txt`, `t3_times.txt`, `t4_times.txt`).
- Step 3 (tier the heavy tests by dependency) NOT built: with the step at 12 s there is nothing left to save, and a skip
  rule is one more thing that can fall behind the code.
- Step 4: inventory `lead_tmp/tstx/test_inventory.md` (a subagent's read of every test, then each deletion checked by
  hand). Removed 5 (commit message lists them). Kept, against the inventory: battle_replay (the only determinism check
  WITH a human pad in it; 4 s now), bake map.rs a_class_carries_its_collision_lines_in_prop_order (tests the bake's
  class row, not the parser), mungefs fx_lvl bit_three_cells_match_the_patch_count (12 maps, not 3), all_maps_load
  every_map_clears_to_black (the CLRB regression guard), sim/state.rs's 100-tick hash pin (deliberate: "a lead decision").
  104 tests pin today's bot rules: they go with BOTD's redesign, not now.
- Test defects the inventory found: card TSTF.

- Land session 2026-10-02 21:30 (lane 3): `needs:` cleared - the maintainer's chat session (pid 235607) is gone and left `resume: land it` after 30 min of "MAIN is held". First phase 1 refused: bake DIFF=1, `side_bf2/vehicles/rep/collision/rep_fly_vwing.glb` (342 index bytes). Not this branch: main's own `engine/target/release/bake` (built 15:30) writes the branch's bytes (md5 db81a314, `scripts/scratch_bake.sh side` -> `lead_tmp/tstx/vwing_scratch_bake`); the shared file is from 2026-09-30, older than STARC (c9d2776, a face with a hole is baked as the face), which changed `mungefs/coll.rs` and declared `rebake=maps` only. Declared `rebake=side` here so the landing refreshes the stale file; no baked map names the vwing.
