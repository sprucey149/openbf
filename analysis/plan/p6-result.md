# P6 result: one fill-quad helper (2026-11-27)

## What changed
- `Ctx::fill(elem, s: &Screen, rect, rgba)` in `engine/crates/ui/src/hud_draw.rs`: a flat fill with the
  white texture, identity transform, no shadow, additive blend or hit box. It calls `Ctx::quad`
  with the same values, so vertices, indices and batches come out the same and in the same order.
- The 8 hand-written white-quad calls now use it: hud_draw.rs (banner fade, damage flash),
  stats_draw.rs (full-window black at the page start, `stats_black`), hud_scope.rs (scope bars),
  spawn_draw.rs (`spawn_veil`), pause_draw.rs (popup prompt backing, still in `menu_screen(size)`),
  shell_ps2/players.rs (`lost_cell`). Each site keeps its own Screen and element index.
- Unused `Mat4`/`Quad` imports dropped in pause_draw.rs and stats_draw.rs.
- Net: 6 files, +41 / -97 lines (about -56; the helper itself is 18 lines).

## Checks
- `cargo test -p ui --no-fail-fast`: 303 passed, 2 failed, both known and not from this packet
  (shell_ia `every_shell_help_line_fits_the_help_box`, typography
  `menu_modules_take_text_styles_from_typography`).
- Split captures (`cargo test -p game --test split_capture -- --ignored`, 3 tests, 68 PNGs including
  four_paused_by_leia, four_one_away, four_stats_pages, spawn pages): every PNG byte-identical
  (sha256) to the same captures taken on main db4a2d1f1 before the change. Looked at the pause and
  4-view lost-cell captures.
- `scripts/one_player_gate.py` on a release build: every row matches.
- `cargo clippy -p ui --all-targets -- -D warnings`: no findings in the changed code. The command
  still fails on lints already on main, from the newer clippy (campaign_saves.rs 769-775,
  shell_ps2/tests.rs 311 and 1062, the `chunks_exact` test line in stats_draw.rs). Those lines are
  untouched here.

## Deviations
- About -56 lines rather than the estimated -90: the helper takes 18 lines, and four sites still
  build their unit Screen inline (the packet keeps a Screen per site, no shared constant).
- Pre-existing clippy failures above are left for the cleanup packets.

## Left
- glyph_of, the single lookup in Ctx::run, text wrap and child sort wait for perf-plan.md.
