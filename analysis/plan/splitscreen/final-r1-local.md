# Splitscreen final review, round 1, local findings

## November 25, 2026

Branch `claude/split-final-r1-local`, built on private `main` at `0f6a28b32` (issue #194).
One builder, nine findings from the final splitscreen review (area: local). Each finding below
says what was done and which named test covers it.

### forbidden-name-in-split-tests and maintainer-name-in-player-names-tests (blockers)
- Every string-literal fixture naming the maintainer in Rust is renamed to `Rex` (case kept:
  `Rex`, `REX`, `rex_1`), with the expected values changed to match (`free("REX") == "REX_2"`,
  `"Rex killed Kai"`, and so on). Files: `game/src/player_names.rs`, `game/src/round_stats.rs`,
  `game/src/event_feed/tests.rs`, `game/src/netplay/reach.rs`, `game/tests/ui_sharp.rs`,
  `game/tests/typography_captures.rs`, `net/src/tests/codec.rs`, `net/src/tests/session.rs`,
  `ui/src/scoreboard.rs`. The two findings are one change.
- New commit check: `scripts/export_public.py` refuses a private name inside a Rust string
  literal (`code_name_problems`, run on every `.rs` before the scrub). The scrub rewrites prose,
  but inside a literal the rewrite would change what the code does, so a literal must be clean
  at the source. `land.py` already runs `export_public.py --check` on every landing. Comments
  stay with the scrub, which already rewrites them.
- Tests: `scripts/tests/test_export_public.py::CodeNameTests` (refusal cases, prose in comments
  allowed, and the whole tree has no hits). Also `player_names` tests,
  `round_stats`, `event_feed::tests`, `netplay::reach`, `net` `codec` and `session`, and
  `ui` `scoreboard`.

### same-model-pads-swap-seats (major)
- The platform gives nothing that tells two pads of one model apart, so the fix detects the
  ambiguity instead of guessing. A returning pad whose model matches the lost pads of two or
  more waiting seats is seated nowhere.
  - `input::owners::PadOwners::alike_waiting` lists those seats.
  - `PadOwners::reclaim_seat` seats a pad on the seat its holder picks.
- In battle, `game::local_seats::Seats::offer_alike` returns `Offered::Choose(SeatChoice)` for
  such a pad. `Seats::take_choice` seats it, and `Seats::still_waiting` drops seats that were
  filled meanwhile. A pad of a model that only one waiting seat lost still goes back by itself.
- The window (`windowed/split.rs`, `seat_split_pads`) keeps one open `PadPick` at a time.
  - Its pad's d-pad (or left stick) left and right move the pick, and its A (South) takes the
    seat. Each counts on a fresh press, so the A that opened the question does not answer it.
  - Each candidate's lost-controller notice changes its last line
    (`game::split_ui::pick_line`, in the pad's own button names). The picked player's cell says
    "Press A to play as Han". The others say "D-pad left or right to play as Chewie".
  - The model is the gilrs uuid, compared between the returning pad and each lost id.
- `Seats::offer_with` no longer treats a waiting seat's old id as that seat's pad being back
  when another player now holds that id. Without this, the second identical pad could not
  return to the seat that was left.
- Players screen: `Party::join_pad_alike` and `PartyFrame::models` (filled in
  `front/players.rs` from gilrs). A pad that is ambiguous in the same way takes its seat
  through the "somebody else's pad" path, so its player picks their profile again. Nobody ends
  up playing on someone else's pick.
- Tests: `input` `owners::tests::identical_lost_pads_are_ambiguous_and_the_pick_seats_the_pad`;
  `game` `local_seats::tests::identical_pads_coming_back_ask_whose_they_are` and
  `an_identical_pad_on_the_players_screen_asks_for_its_profile`;
  `split_ui::tests::the_pick_line_names_the_pads_button_and_the_player`; and the window's
  `windowed::split::tests::a_pad_pick_takes_only_the_seat_its_holder_picks`.
- Capture: `pad_pick_four` (split_capture `split_name_tags_capture`).

### offscreen-markers-clamp-to-band (minor)
- New `ui::hud_draw::Frame::project_edge`, used for off-screen objective, CTF and flag markers
  and for the off-screen lock marker. It keeps the marker's direction from the centre and its
  edge share, and applies them to the whole cell instead of the 4:3 band.
  - In a tall cell, a marker above or below now reaches 0.9 of the way to the cell's top or
    bottom edge.
  - Where the projection is the whole screen (the single view, wide cells), it returns
    `Frame::project` unchanged. The `frame_identity` digests still pass.
- Test: `ui` `hud_draw::frame_tests::an_off_screen_marker_clamps_to_the_cell`.

### ctf-flag-tint-view0-team-in-all-cells (major)
- `windowed::split::set_ctf_viewer` sets the flags' viewer team from the player the walk views.
  View 0 (`events.rs`) calls it, and so does each further view's build (`build_view`), before
  its flags draw. The one `VehicleVisuals` serves every cell, so each cell now colours its own
  side's flag as the friend's.
- New `VehicleVisuals::ctf_flag_tints` and `FlagVisuals::viewer_tints` read the tint the draw
  uses.
- Test: `windowed::split::tests::each_view_colours_the_ctf_flags_for_its_own_side`. It runs
  two-flag CTF with P1 on team 0 and P2 on team 1. View 0 shows [friend, enemy], view 1 shows
  [enemy, friend], and view 0 is right again on the next frame.

### split-rumble-not-reset-between-rounds (major)
- `App.split_rumble` (P2-P4's fire gate tick and last mount) is cleared in `leave_battle`, next
  to P1's reset. It is also cleared when a battle's resources are installed
  (`battle_resources_for`, where the split feeds and previews are rebuilt). This covers playlist
  rounds, Restart Mission and new matches.
- Test: `windowed::tests::leave_battle_clears_pause_state_and_sounds` (updated).

### split-view-toggle-not-saved-menu-stale (major)
- A split player's in-battle view toggle now also writes their own profile and saves it
  (`windowed::split::keep_viewpoint`, through `menu_actions::opener_profile`). That is the
  host's profile for local 0 and their seat's profile otherwise. A seat with no profile keeps
  the toggle for the round only, and never writes the host's.
- The Game Menu's Viewpoint row in split reads the opener's live `PlayerInput` viewpoint, at
  both model builds in `events.rs`. It always shows the view the player is in.
- Test: `windowed::menu_actions::tests::a_pad_view_toggle_keeps_the_players_own_profile_in_step`.

### split-timer-sits-on-bf2-reticle (major)
- The per-cell round timer moves into the reinforcement counts' row, left of the counts.
  - `ui::hud::SPLIT_TIMER_TOP` is now 0.067, centred on `player1teams` (0.073-0.151 of the box).
  - New `SPLIT_COUNTS_HALF` is 0.38: the counts' half width of 0.31, plus room for a fourth
    digit.
  - `SPLIT_FEED_RIGHT` is gone.
- `game::name_tags::cell_timer_at` places the timer the same way in every cell shape. Its
  bottom is at 0.157 of the box. The drawn BF2 reticle's top is at about 0.24 and BF1's is
  lower, so the timer is well clear of it. It also stays off the minimap in the top-right
  corner of tall cells, which ruled out the right side of the counts.
- Tests, both in `split_hud_edges`:
  - `the_split_timer_sits_in_the_counts_row`.
  - `the_split_timer_clears_the_hud`, which replaces `the_split_timer_clears_the_feed`. It now
    checks the reticle where the game draws it: the root's size centred on (0.5,
    `ps2_reticle_y`) in the cell's band, for BF1 and BF2. It also checks `player1flags` and
    720p cells. The old timer position fails this check.
  - `name_tags::tests::cell_text_follows_the_cells_hud_box` is updated.

### three-four-options-help-crowds-footer (minor)
- The 3-4 player screen-row help is now `ui::menu::SPLIT_SCREEN_HELP`: "Left/Right moves this
  player's screen; whoever was there swaps with them."
- Test: `ui` `pause_draw::help_fit_tests::split_screen_help_fits_two_lines`. It wraps the help
  with the bake-matched `gamefont_small` fixture at the narrowest stack width that
  `draw_stack` uses. It allows 2 lines at most at 16:9, 4K and 16:10, and 1 line for two
  seats. It also confirms that the old text took more than 2 lines.

### Checks run
- Focused tests:
  - `input` `owners`.
  - `game --lib`: `local_seats`, `split_ui`, `player_names`, `round_stats`,
    `event_feed::tests`, `netplay::reach`, `netplay::tests`.
  - `game --bin game`: `windowed::split`, `windowed::menu_actions`, `windowed::front`,
    `windowed::tests::leave`.
  - `net`: `codec`, `session`.
  - `ui --lib`: `split_screen_help_fits_two_lines`, `frame_tests`, `frame_identity`,
    `scoreboard`.
  - `scripts/tests/test_export_public.py -k CodeName`.
  - `ui_sharp` and `typography_captures` compile.
  - All of these pass.
- `split_hud_edges --ignored` (with `HUD_ASSETS`): 5 of 5 pass.
- One-player gate: every row matches. It checked bf1_hot1, bf2_uta1, bf2_spa1 and the LAN soak,
  with the release build of this branch.
- Split captures (`split_capture --ignored`): 3 of 3 pass. I looked at four captures:
  - `b8_tags_top_bottom` and `b8_tags_side_by_side` at 1920x1080, and `b22_tags_four_hud_full`
    at 1280x720: the timer sits left of the counts, in their row, well above the reticle, in
    every cell.
  - `b7_three_options_page` at 1920x1080: the help is two lines, clear of the footer.
  - `pad_pick_four`: Han's cell says "Press A to play as Han", and Chewie's says "D-pad left or
    right to play as Chewie".
- No capture shows an off-screen marker in a tall cell. That change is covered by its unit test
  only.
