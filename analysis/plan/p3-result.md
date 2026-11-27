# P3 result: retire brain-shadow and no-caller sim functions (2026-11-27)

## What changed
- Removed the `brain-shadow` feature: `core/src/ai/brain/shadow.rs` (56 lines),
  `collision::cost::preserve`, the `shadow` module and `verify_shadow` re-export,
  the feature lines in `core/Cargo.toml` and `game/Cargo.toml`, and the CRATES.md row.
- `sim/step.rs`: dropped the two shadow-gated statements and the `cfg(not)` attribute.
  `live_ai_elapsed` is still taken right after `unit_grid.clear()`, so `ai_wall_ms`
  times the same span as before.
- Deleted functions with no callers: `Sim::spawn_native_ambush`, `goals::defend_hint`,
  `Flag::is_at_home`, `CampaignProgress::save_names`, and the `legacy_think` re-export
  (`ai.rs`, `ai/brain.rs`). The proxy pin doc now says "the legacy think".
- Kept: the route-fixtures feature and gate, the game dev-dependency, `legacy.rs`,
  `save_list` and other save code.
- Size: 1143 -> 1142 core/game src files, 602182 -> 602032 lines (-150).

## Before removal
`cargo test -p core --features brain-shadow`: 2551 passed, 24 failed, 3 ignored.
18 failures were the shadow check itself (`brain pad at ...` mismatches in supply,
fix, man, ride, stall, perception, remotes, state, step tests): the shadow had drifted
from the live brain and no longer agreed with it. The rest were the fmath referees
(`front.rs` trailing cfg(test), clippy max/min) and `mesh_ray` cached-bounds, which
fail without the feature as well.

## Checks
- `cargo test -p core --lib` filtered: brain 36, native_teams 14, ctf 13, goals 3, all pass.
- `cargo test -p game profile`: 1 + 53 + 15 + 1 pass (3 ignored).
- Proxy pin `an_all_proxy_skirmish_keeps_its_hash` passes. Space pin
  `a_space_round_replays_to_the_same_hash` passes.
- `scripts/sim_gates.sh`: all green (72 ticks 0e8ffee82cf9a7a2, walk c906917527627f71,
  aim ff3c44a84452fd1c, 600 ticks abcec91efdcf4c93).
- `scripts/one_player_gate.py`: every row matches.
- `size_report.sh --check-map`: exits 1 on main and here, for the same reasons that were
  already there (42 files not in the map, 3 missing named paths). The stale count drops 695 -> 694.
- Clippy: `cargo clippy -p core -p game --all-targets -- -D warnings` does not pass on
  main with the installed clippy 0.1.100. It stops on new lints in audio and render
  (let_unit_value, chunks_exact). With `--no-deps`, core and game give 191 distinct
  warnings, the same set on main and on this branch. This branch adds none.

## Deviations
- The clippy acceptance was checked as "no new warnings vs main". The clean `-D warnings`
  run cannot pass until the toolchain lint backlog is fixed.
  This substitute needs the maintainer's acceptance before landing; otherwise rerun the
  full `-D warnings` check once the lint-backlog packet has landed.

## Left
- The clippy 0.1.100 lint backlog across the workspace (separate packet).
