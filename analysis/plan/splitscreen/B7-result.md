# B7 result, November 25, 2026 (branch `claude/split-b7`)

Splitscreen options (`SPLITSCREEN.md` 18.2 B7), on private `main` at `1cb86b183`.

## What landed

- `game::split_layout::arranged(players, w, h, two, cells, seat_of)`: the cells by local player.
  1 player gets the whole window. With 2, the two seats are ranked by their `cells` values and the
  lower one gets the left (or top) cell. With 3 or 4, the 2x2 grid is permuted by
  `cells[seat_of(i)]`, so with 3 the blank can be any cell. An order that is not a permutation
  falls back to `cells`.
- `split_layout::move_seat(cells, seated, seat, right)`: the "P# Screen" step. With 2 seated, any
  step swaps the two seated seats' values. With 3 or 4, the seat moves to the next (or previous)
  grid place and swaps with whoever, or the blank, held it.
- `App::split_cells` now calls `arranged` through a new `split::split_cells_for` with the host
  profile's `settings.splitscreen` (normalised) and `seats.seat_number`. It reads them every frame,
  so a change re-lays out on the next frame. `SplitState.layout` is gone, so the profile is the
  only source. `hud_k` already came from the profile (B6), so the HUD size row takes effect.
- ui: `SettingsPage::Splitscreen` (the last variant, titled "Splitscreen Options") and
  `MenuModel.split: Option<SplitMenu>` (`top_and_bottom`, `seated`, `cells`, `name_tags`,
  `hud_size`). Options appends the page row only when `split` is set, and it goes last, so no
  other row moves. The rows are "Two Players" (Side by Side / Top and Bottom), one "P# Screen"
  per seated seat, "Name Tags" and "HUD Size" (Full / Smaller / Smallest). The P# value shows
  Left/Right or Top/Bottom with 2 seated, else the grid place. With 3-4 seated, Two Players is
  locked (greyed, and it gives its reason as help). Left/Right and Cross step every row. The new
  actions are `SetSplitTwoPlayer`, `MoveSplitScreen { seat, right }`, `SetSplitNameTags` and
  `SetSplitHudSize`.
- Game wiring: `menu_actions::{apply_split_setting, split_menu, seated_of}`. In battle,
  `apply_pause_action` applies the actions to the host profile (`App.profile`) and saves it. The
  pause model gets `split` only when `split_on`. In the shell, the settings page gets `split` for
  the Splitscreen page, and `apply_shell_setting` applies the actions with the Players screen's
  seated seats.
- Shell: a new "Splitscreen Options" row (`RowId::SplitscreenOptions`, our key
  `openbf.split.options`) at the bottom of the SplitModes list. It opens
  `ShellCommand::Settings(Splitscreen)`, and Back returns to the list. RotE and GC stay greyed.
- `ClosePage` fix (`game::pause`): the parent row is looked up in a model with `split` set, so
  Back from the Splitscreen page lands on its own Options row. The page is last, so the other
  pages' Back rows are unchanged.
- `split_capture.rs`: `Scene` gains `order` and `hud_k`, and `arranged` lays out the views. There
  are six new `b7_*` scenes.

## Tests run

- `cargo test -p game --lib -- split_layout:: pause::`: 27 passed. New tests:
  `the_identity_order_equals_cells` (n 0-4, both splits, five windows),
  `every_order_is_a_bijection_onto_equal_cells` (24 orders x every seat set x n 2-4, plus 3P with
  the blank top-left), `a_move_swaps_exactly_two_seats`,
  `in_two_player_every_step_on_either_row_swaps_the_two_screens` (every seat pair x every order,
  so whatever the absent seats hold), and
  `pause::tests::back_from_the_splitscreen_page_returns_to_its_options_row`.
- `cargo test -p ui --lib -- menu:: shell_ps2::`: 81 passed. The new `menu::split_menu_tests` has
  5 tests: rows only with `split`; Back from Splitscreen goes to its Options row; Two Players
  locked at 3-4; the P# rows' steps and labels; name tags and HUD size. The SplitModes shell test
  now checks the new row and its command.
- `cargo test -p game --bin game -- menu_actions:: split:: front::`: 28 passed, 2 ignored. That
  includes the new
  `the_splitscreen_page_edits_the_hosts_options_and_the_cells_follow` (seats P1 and P3 with a hole,
  a swap, top and bottom, HUD size, name tags).
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 1 passed. This needed
  `engine/assets_baked` and `engine/missions` symlinked into the worktree; they are untracked and
  were not committed. I looked at these at 1920x1080:
  - `b7_two_swapped`: P1 is on the right.
  - `b7_two_top_bottom_swapped`: P1 is at the bottom.
  - `b7_three_blank_top_left`: the blank cell is top-left, and P1 is top-right.
  - `b7_two_hud_smallest`: the HUD is at 0.7.
  - `b7_two_options_page` and `b7_three_options_page`: in the 3P page, Two Players is greyed and
    the P# rows read Top Right / Bottom Left / Bottom Right.
- One-player gate (release build of this branch): every row matches. `lan_soak` is ok with 4312
  hash checks.

## Deviations

- The "P# Screen" action carries only `{ seat, right }`. The game works out the seated seats
  (`seated_of(seats)` in battle, the party's owners in the shell) and calls
  `split_layout::move_seat`, so the rule is tested in `split_layout::`, as the plan asks.
- The shell sets `MenuModel.split` only on the Splitscreen page. The shell's Options is a
  `shell_ps2` stack, not `PausePage::Options`, so the page is reached only through the SplitModes
  row.
- Back to the SplitModes list from a page it opened (Instant Action, the Splitscreen settings)
  keeps the focus on the row that opened it (`ShellMenus::uncover`); entering the list from the
  Players screen still puts it on Instant Action.
- The in-battle redraw after a pad action rebuilds `split` from the profile, so a Splitscreen row
  shows its new value on the same frame.
- The 3-4 player "P# Screen" help says Left/Right step through the four places in reading order.
- The Game Menu's Splitscreen rows still edit the host profile for any opener; B10's `row_scope`
  and "Shared" help must cover all four (Two Players, P# Screen, Name Tags, HUD Size).
- Nothing reads Name Tags yet; B8 draws the tags. The Game Menu rows still all edit the host's
  profile. Row scopes and "Shared" help are B10's.

## Left

- B8 reads `settings.splitscreen.name_tags` and places tags by `cell_of`, that is, the
  `arranged` cells.
- B10 adds `row_scope`, so a non-host opener sees "Shared" on these host rows.
- The maintainer should look at the `b7_*` captures and try the page with real pads.
