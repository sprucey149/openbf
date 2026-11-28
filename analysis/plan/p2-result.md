# P2 result: duplicate offsets table and dead bake/asset helpers (2026-11-27)

## What changed
- `bake/src/map/render_offsets_data3.rs` (814 lines) deleted. A one-off test
  `assert_eq!(data2::MOVES_42, data3::MOVES_43)` passed first (1 passed), and the
  file bodies differ only in the static's name. The PC2 Felucia bladder recipe now
  points at `data2::MOVES_42`. The table header now says that identical fitted move
  sets are shared across fingerprints. The one-off test was removed with data3.
- `engine/CRATES.md`: the data3 row is gone, the data2 row now covers the Xbox, PC1
  and PC2 Felucia bladder, the table count is 430, and the shell.rs row names
  `bake_shell_with_campaign`.
- Dead Rust removed: `bake::shell::bake_shell`, `pak::exe_archive`,
  `extensions::mods::{dedup_sorted, normalize_override_map_key, collect_map_modes_summary}`,
  `CatalogLvl..::has_tag` (the file is `extensions/src/mods/catalog.rs`),
  `mungefs::lua50::Proto::{string_constants, collect_strings}`. No imports were left unused.
- Dead Python removed: `_ron_stage_kind` (gen_missions.py), `placed_draw_surfaces`
  (render_contact_proxy.py), `segment_distance2` (render_collision_check.py),
  `function_at` (pcref.py).
- Doc fixes: the `resolve_file` doc in pak/src/overlay.rs is back on its function
  (it had been sitting on `Resolved`). The `le` module doc in mungefs/src/lib.rs is
  reworded so it no longer describes history.
- `bake_versions.lock`: 13 groups recorded neutral, because none of their output changes.
- Net: 12 lines added, 944 removed.

## Checks
- `cargo test -p bake`: 416 passed (lib), 14 ignored, plus 2, 3 and 29 in the other targets. All green.
- `cargo test -p pak -p extensions -p mungefs`: pak 22, extensions 31, mungefs 209 (3 ignored) + suite 42. All green.
- py_compile on the 4 scripts: ok. pytest scripts/tests/test_render_collision_check.py and scripts/test_pcref.py: 99 passed.
- `scripts/bake_versions.py check`: pass, after `neutral` for the 13 groups.
- `scripts/one_player_gate.py`: every row matches (release game build, lan_soak ok).

## Deviations
- `cargo clippy -p bake -p pak -p extensions -p mungefs --all-targets -- -D warnings`
  fails on main too. rustc/clippy 1.100.0 adds lints (manual chunks_exact → as_chunks,
  explicit deref, derivable impl, is_multiple_of, if_same_then_else, sort_by_key) that
  fire 42 times in code this packet does not touch. Run without `-D`, main and this
  branch report the same 42 warning sites, so the packet adds no new warning. They are
  not fixed here: fixing them touches about 30 bake files outside scope.
- `scripts/size_report.sh --check-map` exits 1 on main and on this branch with the
  same 45 NOT IN MAP / NO SUCH PATH entries, none of them in files this packet touches.
  The removed data3 row leaves no gap of its own.
- No files from the PERF chain were touched.

## Left
- A toolchain-lint cleanup packet for rustc 1.100 (the 42 sites above), and the 45 CRATES.md map gaps.
- The bake gate (gates.py `bake`) still has to confirm the 13 neutral groups at landing.
