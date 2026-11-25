# Splitscreen final review, round 3: local game fixes

## 2026-11-25

Builder: one pass over the four confirmed local-game findings, on main at df8c74f10. Each now
has a named test. The one-player gate (`scripts/one_player_gate.py`) passes on the release
build: every row matches, and the LAN soak is ok. The split captures and the Players screen
capture were re-run and checked by eye.

### esc-menu-p1-away-passes-to-p2-keyboard-locked-out (major)

- **Opener:** offline, with nobody on the keyboard, Esc now opens the Game Menu as the lowest
  seat whose own pad is connected. If no pad is connected, it opens as the lowest seat that is
  not away. Only when every seat is away does it fall back to seat 0
  (`split::free_keyboard_opener`, used by `App::pause_opener`). An away P1 is no longer the
  opener while P2 is still here. LAN keeps its own rule (`lan_free_keyboard_opens`).
- **Driving:** `App::pause_keyboard_drives` is now true for any menu the free keyboard opened
  (`pause_opened_by == (_, true)`), whoever drives it by pad. If the pad side passes on
  (`menu_driver`), the keyboard can still move through the menu and close it with Esc.
- **Title:** `App::paused_by_line` checks for the free keyboard before it checks whether the
  driver differs. A keyboard menu keeps "Paused (keyboard)" and never names a pad player who
  did not open it. Pad menus still name the new driver after a hand-off (B13b).
- **Test:** `windowed::split::tests::esc_with_p1_away_opens_a_keyboard_menu_the_keyboard_keeps`
  (walk with P1 suspended, no keyboard seat). It covers the opener being P2, the opener rule's
  fallbacks, the probe's path (opened as P1, P1's pad gone, owner passes to P2: the keyboard
  still drives and the line stays "Paused (keyboard)"), and a pad player's menu that the
  keyboard does not drive.

### pick-line-spells-button-as-text (minor)

- The battle cell's lost-controller notice is no longer drawn with egui. It is drawn in the
  cell's own HUD pass (`SplitUi::lost`, `HudView::update_lost`,
  `ui::hud_draw::draw_lost_cell`, `Ctx::lost_cell` in `ui::shell_ps2::players`), in the menu
  face, so it can draw button glyphs as the spawn and menu footers do.
- `split_ui::pick_line` now returns a glyph sheet and a label. The pick shows the pad's A
  glyph with "Play as Han"; the other waiting cell shows the pad's d-pad left/right glyph with
  "Play as Chewie". Both follow the waiting player's own icon choice, as in round 2.
- The line the player acts on now uses the ITEM role (as large as the name's TITLE) in full
  white. "Reconnect and press any button" uses the same style, with no glyph.
- `lost_lines` became `lost_notice` (a `LostCell`), and the egui `draw_lost_notice` is gone.
  `split_capture` now builds the notices as `events.rs` does, and `pad_pick_four` makes both
  waiting cells (P2, P4) away views. Every view, away ones included, now paints its own cell.
- **Tests:** `split_ui::tests::the_pick_line_names_the_pads_button_and_the_player`,
  `ui shell_ps2::tests::the_lost_cell_draws_its_buttons_glyph_and_a_bright_acting_line`,
  `windowed::split::tests::the_editor_and_the_pick_line_use_the_players_own_icons`,
  `windowed::split::tests::the_notice_names_the_player`.
- **Captures (looked at):** `pad_pick_four_1920x1080.png` shows the green A glyph with "Play as
  Han" and the d-pad glyph with "Play as Chewie", in white, larger than before.
  `four_one_away_{1920x1080,3840x2160,3839x2372}.png` show the notice centred in its cell, with
  the acting line in white.

### players-continue-live-but-refused-duplicate-profile (minor)

- `Party::twice` (the later seat of each pair) now drives `Party::can_start`, and
  `Party::shares_profile` marks every card in a pair. `fill_players_view` passes the party's
  verdict in `PlayersView::blocked`, and `PlayersView::can_start` honours it. The Continue
  bars, their pointer hit and the "A Continue" help therefore stay dim whenever the party would
  refuse.
- Each sharing card now shows "Same profile twice" (warning tint) and "One player changes
  profile" in place of "Ready". The longer "Two players are on <name>" was cut off by the
  card's width in the capture, so it was not used.
- **Tests:** `local_seats::tests::an_identical_pad_on_the_players_screen_asks_for_its_profile`
  (extended: `twice() == [2]` and `shares_profile`), and
  `ui shell_ps2::tests::a_blocked_party_never_shows_continue_live`.
- **Capture (looked at):** `ui_capture capture_players`, `players_twice_*` (new): P2 and P3 are
  on Cy, both cards say so, and no Continue is live.

### players-screen-lost-seat-cannot-be-freed (minor)

- Holding Select (Share, Create, View or Back) for `REMOVE_HOLD` (1 s, OURS) on any seated pad
  empties the lowest guest seat whose pad is gone. Holding Tab on the keyboard does the same,
  whether the keyboard is seated or free (`PartyPress::remove`, `Party::removable`,
  `Party::remove_lost`, `RemoveHold`). Only that seat changes: nobody is renumbered, and the
  other seats keep their picks.
- Seat 0 (the host) is never emptied this way while others sit, because the host rule says
  seat 0 is never a hole. Another pad's A or Start can still take it.
- An unseated pad's hold does nothing. The hold fires once and must be released before it can
  fire again.
- A removable lost card now shows "[Select glyph] Hold to remove" under "Come back". The glyph
  comes from the seated pads' set when they are all one family. Otherwise the card reads "Hold
  Select or Tab to remove".
- **Tests:** `local_seats::tests::a_held_select_empties_a_lost_guest_seat_only`,
  `local_seats::tests::a_remove_hold_fires_once_per_full_hold`, and
  `ui shell_ps2::tests::a_removable_lost_seat_shows_the_select_glyph`.
- **Capture (looked at):** `players_4_*` shows P3's lost card with "Come back" and, under it,
  the Select glyph with "Hold to remove".

### Checks run

- `cargo test -p game --bin game -- split:: front:: menu_actions players paused_by esc_with_p1_away menu`: all passed.
- `cargo test -p game --lib -- local_seats split_ui`: 37 passed.
- `cargo test -p ui --lib -- shell_ps2 players lost_cell`: all passed.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 3 passed.
- `UI_CAPTURE_OUT=... cargo test -p game --test ui_capture capture_players -- --ignored`: passed.
- `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches, and
  the LAN soak is ok.

### Left

- A lost seat 0 still cannot be emptied while others sit (host rule). With no spare pad, the
  players back the screen out and rejoin. Changing this would mean re-deciding the host rule.
- The in-battle lost cell's texts are English literals, as they were before.
