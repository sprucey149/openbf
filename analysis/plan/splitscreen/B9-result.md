# B9 result, November 25, 2026 (branch `claude/split-b9`)

Scoreboard and stats pages per view (`SPLITSCREEN.md` 18.2 B9), on private `main` at `6dc1093d2`.

## What landed

- ui `scoreboard::draw_scoreboard_in(ctx, rect, salt, b)`: the board inside `rect`, sized as a
  window of that size would be (units and fonts scaled by `menu_height(rect) / menu_height(window)`),
  on its own layer per salt. `draw_scoreboard` is now `draw_scoreboard_in(ctx, content_rect, 0, b)`;
  with the whole window the scale is exactly 1 and salt 0 keeps the `"scoreboard"` Id.
- Per-seat scoreboard hold: `SplitState.score_holds`, one `Hold` per local player, stepped by
  `App::step_split_score_holds` from that player's own pad (`split.pads[i]`: Select and Switch
  Weapon) and the Tab key only for the keyboard player. A frame whose pads were not sampled
  (paused, round end, popup) releases every hold. In split, each shown board is built with
  `scoreboard_model` under `with_local(i)` with that view's feed and drawn in that view's cell
  (`draw_scoreboard_in`, salt `i + 1`), under the single view's rules (no campaign, no menu, not
  on that view's spawn pages or round end, not away). The full-window board is not drawn in split.
- Per-view stats pages: `split::split_stats_pages(walk, split_feeds, split_profiles, seats)`
  builds each further local player's pages under `with_local(i)` from their own feed (names,
  round, opening medals set by B3) and their own seat profile's career (this round already in it
  from `record_split_rounds`). An away player gets `None`. They are made in the same frame as
  P1's (`wants_stats`), kept in `SplitState.stats`, and swapped into that view's
  `RoundEndModel.stats` in place of the shared P1 pages.
- Per-pad page cursors: `App::seat_menu_edges` reads each seated player's own pad (D-pad, left
  stick, South, East (and North once the pages are up) as Back, West as Square, shoulders) with per-player held bits
  (`SplitState.menu_prev`), plus the keyboard's menu keys for the keyboard player only. In split,
  P1's shared `RoundEnd` steps on P1's own presses (no longer on whichever pad was active), and
  `RoundEnd::step_views` steps every further player's pages on their own pad. `step_pages` is the
  one page-step-and-sound routine, shared by `RoundEnd::step` (unchanged behaviour).

## Tests (filtered)

- `cargo test -p ui --lib -- scoreboard::`: 7 passed. New
  `a_split_views_board_stays_in_its_cell` (whole window with salt 0 equals `draw_scoreboard`;
  boards in side-by-side, top-bottom and grid cells of 1920x1080 keep every text and frame inside
  the cell, the viewer's row drawn).
- Single-view identity of the board: a temporary probe hashing every shape of
  `draw_scoreboard` for 32-row boards at 640x480, 1280x720, 1280x1024 and 3840x2160, ping on and
  off, gave `b0fd8a7e4f47a5b9` on `main`'s `scoreboard.rs` and on this branch's (probe not kept).
- `cargo test -p game --lib -- round_end:: round_stats::`: 14 passed. New
  `round_end::flow_tests::each_split_players_pages_follow_their_own_pad` (P2's cross moves only
  P2's pages, P1's down moves only P1's, an away view takes nothing, P2's presses hold off the
  30 s idle quit, P2's Done leaves for everyone). Existing round_end and round_stats tests
  unchanged.
- `cargo test -p game --bin game -- split::`: 13 passed. New:
  - `each_views_stats_pages_are_its_own_players` (the B9 check): view 1's pages mark "Han" (P2)
    as me with P2's career (42 kills against P1's 7); view 1's "medals this match" holds War Hero
    and every count is 1, counted from P2's slot against P2's opening counts (P2 opened with one
    of each); view 0 marks "Luke" with no medal; view 0 lists P2 as "Han" and view 1 lists P1 as
    "Luke"; no row reads "Player N"; an away P2 gets no pages.
  - `each_players_board_is_held_on_their_own_pad`.
- Captures: `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 1 passed.
  New scenes `two_stats_pages`, `two_stats_pages_top_bottom` and `four_stats_pages` at 3840x2160,
  1920x1080 and 3839x2372 (`engine/artifacts/split_captures/`). Looked at: each cell shows its own
  page (Team Stats with its own row highlighted, or "Personal Stats (Han)" / "(Leia)" with that
  player's medal), its own pad's prompt icons, footers at the cell's bottom.
- Gate: `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches,
  `lan_soak` ok (4312 hash checks). (A later one-line draw change, `max` to `fmax` for clippy,
  came after the gate; it is in the egui pass only.)
- Clippy (`-p game --bin game --tests`): no new warnings in touched lines. rustfmt on touched files.

## Deviations

- The per-view pages and holds live in `SplitState` (`stats`, `score_holds`, `menu_prev`), not
  in `App.split_stats`: the split state is made per battle, so nothing stale carries into the
  next map.
- Leaving the stats: the plan keeps `walk.round_end` shared and gives each cell its own cursor,
  but does not say who leaves. Decided here: a Done from any present player leaves the stats for
  everyone (as today any pad's Done did), and any player's press resets the 30 s idle quit. P1's
  own pad (and the keyboard, when P1 holds it) drives P1's pages; it is no longer whichever pad
  was used last. The end movie can be skipped from any player's pad.
- The round's result banner stays the first player's (all local players share a side today).
- The scoreboard in a split view takes the LAN roster as the single view's does (none today:
  split is offline only), so B14+ LAN local players keep the ping column and roster names.

## Review fixes

- A player away when the pages are made gets theirs once back while the pages are up:
  `split::fill_split_stats_pages` runs every frame the pages are up and makes only the missing
  ones of present players, keeping made pages and their cursors (`split_stats_pages` wraps it).
  `each_views_stats_pages_are_its_own_players` now also suspends P2, fills (still none),
  resumes P2, fills (P2's own pages), and checks a moved cursor survives the next fill.
- North is Back on a split pad only once the stats pages are up, as in `App::menu_edges`; it no
  longer skips the end movie in split (South and East still do, as in single).
- `seat_menu_edges` runs every split frame (its result used only at the round's end), so a
  button held as the round ends is not read as a fresh press on its first frame.
- `draw_scoreboard_in` takes the splitscreen HUD size factor; a split cell's board shrinks with
  that cell's HUD. The single view passes 1, so its board is unchanged.
  `a_split_views_board_stays_in_its_cell` covers every cell at factor 1 and 0.7.
- Rejected: a capture of the wiring (real walk pages, egui board in a cell). The capture harness
  paints only the HUD drawer (no egui pass), and `split_stats_pages` lives in the binary, out of
  reach of `tests/`. The wiring is covered by the bin test on a real two-local walk, the cell
  geometry by the ui test; the held board in a 2P and 4P cell stays on the maintainer's check.
- Kept: each further view's pages are cloned into its HUD model every stats frame (as P1's are
  by `hud_model`); at most three small page sets a frame, a cost and not a fault.
- Gate after the fixes: release build, `one_player_gate.py` every row matches, `lan_soak` ok
  (4312 hash checks). `scoreboard::` 7 passed, `split::` 13 passed.

## Left

- B10 (Game Menu rows edit the opener's profile), then the first playtest build.
- The maintainer's check with real pads: P2 holds their board in their own cell (2P and 4P, and
  with a smaller Splitscreen HUD size); at round end each player walks their own pages and P2's
  Career Stats show P2's profile; a pad unplugged before the stats and plugged back during them
  gets its own pages.
