# Splitscreen final review, round 2: local game fixes

## 2026-11-25

Builder: one pass over the five confirmed local-game findings. Each now has a named test. The
one-player gate (`scripts/one_player_gate.py`) passes on the release build: every row matches,
and the LAN soak is ok. No split capture was re-run, because nothing a capture draws changed.
The pick line's text now follows the waiting player's icon choice, and `split_capture` builds
that line itself.

### split-round-end-pads-never-seated-offline (major)

- **Cause:** offline, `round_end_held()` put every round-end frame into the `popup_up` arm, so
  the round-end arm that seats split pads could never run.
- **Fix, branch order:** the per-frame pad branches are now chosen by
  `split::frame_pads(paused, lan_split, popup_up, round_over) -> FramePads`, and `events.rs`
  matches on the result. The order is: LAN menu, Game Menu, round end, popup, play. The round
  end now comes ahead of the popups. Its pad is still neutral and its taps are still dropped,
  so the one-player path behaves the same.
- **Fix, pages of an away player:** a pad lost during the stats left that player's pages in
  `split.stats`, so `RoundEnd::step_views` kept waiting for them. `fill_split_stats_pages` now
  drops an away player's pages, as `SplitState::stats` documents (`None` while away). They get
  fresh pages when they come back.
- **Test:** `windowed::split::tests::an_offline_round_end_still_seats_and_drops_split_pads`.
  - P2 drops before the end. The offline outcome holds the game, and the frame still picks
    `FramePads::RoundEnd`.
  - The stats come up with no pages for P2. P2's pad returns, P2 is seated and gets their own
    pages.
  - P1 leaves and their cell shows "Waiting for Leia". P2's pad goes again: P2 is away, loses
    their pages, is not waited for, and the stats end without the idle timeout.
- **Test limit:** the test drives the seats through `Seats<u8>` and `apply_presence`, because
  a gilrs `GamepadId` cannot be built in a test.

### split-controls-editor-and-pick-line-host-icons (minor)

- **Controls editor:** `controls_model` now names pad buttons through `App::editor_icon_set`
  (`split::seat_icon_set`). It uses the editing seat's own `input_icons` and that player's pad
  family; with no pad yet it falls back to the device the split prompts use. Out of a split
  battle it returns `None`, and the editor keeps the host's choice on the device in use, as
  before.
- **Pick line:** `pick_line` takes the host and seat profiles and uses the waiting player's
  own `seat_settings(..).input_icons`. The button names still come from the returning pad's
  family.
- **Test:** `windowed::split::tests::the_editor_and_the_pick_line_use_the_players_own_icons`.
  P2 has PlayStation 4 icons forced and the host is on Auto, with Xbox pads. P2's editor and
  P2's pick line show PlayStation names, P1's show Xbox names, and an empty seat gives none.

### walk-test-ears-held-view-stale (minor)

- **Fix:** `walk::tests::a_ground_rider_lost_with_the_hull_holds_the_seat_view_past_four_seconds`
  now expects the ears 2 m behind the held view along its back axis, which is where
  `event_sounds::listener` puts them. The held view itself is still asserted unchanged.
- **Result:** the filtered lib set from the finding (split, local, ..., profile) now passes:
  305 passed, 0 failed.

### players-screen-second-identical-pad-trusted (major)

- **Remember the ambiguity:** `Party` keeps an `unsure` flag per seat. When a pad returns while
  two or more waiting seats lost pads of its model, every seat in that group is marked. Any pad
  that later takes a marked seat asks for its profile, even when only one seat still waits.
- **Clearing the flag:** taking the seat clears it, and so do emptying the seat and
  `set_owners`.
- **Release the greyed id:** `held_ids` skips a marked waiting seat's guest pick. The player
  choosing on the first seat back may be that seat's own player, so their profile is not greyed
  out.
- **No duplicate profiles:** `can_start` now refuses when two seated seats are Ready on the
  same profile. Without this, a still-waiting marked seat and the seat that took its profile
  could both start on it.
- **Test:** `local_seats::tests::an_identical_pad_on_the_players_screen_asks_for_its_profile`
  now goes past the first return:
  1. P3's seat is marked, and Cy is free on P2's seat's picker.
  2. Cy picks Cy there, and start is refused while both seats hold Cy.
  3. The second pad comes back on P3's old id with only P3 waiting. It still asks, and Cy is
     greyed there.
  4. Bo picks Bo and the screen can start.

### pick-ignores-second-identical-pad-silently (minor)

- **Any identical pad answers:** `PadPick` now keeps every pad that may answer, each with its
  own held buttons. When an identical pad comes back while a question is open, its question
  shares waiting players with the open one, so `PadPick::join` adds it. Its d-pad moves the
  shared pick, and its A takes the seat shown for itself, never for the opener.
- **What happens after a take:** the seated pad stops answering. If two or more seats still
  wait, the question stays open for the pads left.
- **One question per model:** `SplitState::pick` became `picks: Vec<PadPick>`. A pad of
  another model opens its own question over its own waiting players, and is no longer ignored
  until the first question closes. `pick_line` finds the question that covers each waiting
  cell.
- **Pads that go or are seated:** `PadPick::retain` drops them from a question, and the lines
  then name the model of the next pad.
- **Tests:** `windowed::split::tests::a_second_identical_pad_answers_the_open_pick_too` (new),
  and `a_pad_pick_takes_only_the_seat_its_holder_picks` (updated for the per-pad `step`).

### Checks run

- `cargo test -p game --bin game -- split:: controls pick round_end stats lobby front windowed::tests`
- `cargo test -p game --lib -- split local name_tag player_name event_feed round_end virtual_net netplay away presence lan seat view_camera b21 profile`
  (305 passed)
- `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches.
