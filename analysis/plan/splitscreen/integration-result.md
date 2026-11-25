# Integration findings result, November 25, 2026

Branch `claude/split-bint`, built on private `main` at `d7a4865a0` (after B10). Input: the
integration tester's 23 findings on the whole splitscreen stack (issue #194). Each finding was
checked against the code before it was fixed. Five majors and eleven minors were fixed; the rest
were rejected or deferred, with the reason given below.

## Fixed

### Majors

- **Round timer over the event feed (side by side).**
  - In a cell narrower than 4:3 the HUD box is the full cell width. The feed
    (`player1systemMessages`) then ends at 0.78 of the box height from the left edge, past the
    centre.
  - `game::name_tags::cell_timer_at` now keeps the timer centred unless the feed reaches under
    it. When it does, the timer moves right to just past the feed. The new constants
    `ui::hud::SPLIT_FEED_RIGHT`, `SPLIT_TIMER_GAP` and `SPLIT_TIMER_SIZE` set the feed edge, the
    gap and the timer's maximum box.
  - `draw_hud_in` takes `timer_at: [f32; 2]` (an offset from the top centre). `draw_hud` passes
    `[0, 12]`, so single player is unchanged.
  - New `ui::hud_draw::root_span` (the x and y extent of a root).
  - New test `split_hud_edges::the_split_timer_clears_the_feed`. For every cell shape (2 side by
    side, 2 stacked, quads, thirds; 1080p, 3840x2400 and 1600x1200) at HUD sizes 1.0, 0.85 and
    0.7, it checks that the timer box stays in its cell and covers none of these: the feed, the
    counts, the target name or bar, the reticle, the objective timer, the victory timer, or the
    minimap. It also checks the feed edge constant against the real layout.
- **An away player never came back during the round end.**
  - Seating and presence were split out of `sample_split_pads` into `App::seat_split_pads`. That
    covers pump, gone, offer, and suspend/resume.
  - The frame now also calls it while `walk.sim.outcome` is set, and refreshes `view_away`.
  - A pad that returns during the stats is reseated and resumed, and `fill_split_stats_pages`
    then makes that player's pages.
  - No new unit test covers the frame call site: it needs gilrs ids, which a test cannot build.
    The walk-level path (suspend, resume, fill) is already covered by
    `each_views_stats_pages_are_its_own_players`.
- **P1's lost-pad carry.**
  - New `WalkSim::first_gate_points`: P1's gate adds the carried points over a per-rejoin
    baseline, as `gate_slot` does for the other players. While P1 is away, the gate holds the
    points P1 had at the drop.
  - New `WalkSim::carried_round_points` is now shared by `record_local_battles` and by P1's
    `career::record_round`.
  - While P1 is away in split, `tick_hook` still ticks the feed and `heroes::scores`, but credits
    no medal points to P1's handle.
  - Test: `p1s_points_survive_a_rejoin_on_another_index`.
- **One player's Done ended the stats for everyone.**
  - New `RoundEnd::hold_for_players(n)`, called when the split pages are made. From then on, a
    Done closes only that player's pages, and their pages take no more input.
  - The stats end when every player present (P1 included, unless P1 is away) has left, or when
    the idle timer runs out.
  - `step_views` takes `first_away`.
  - Cells that are done show a band: "Waiting for Han, Leia" (`split_ui::draw_waiting_line`,
    `App::split_waiting`).
  - Tests: the updated `each_split_players_pages_follow_their_own_pad` and the new
    `split_stats_end_when_every_player_has_left`.
- **No way out of the Players screen.**
  - P1's B on their own list while others are seated now leaves the whole screen. P1's B on the
    Ready card first goes to the picker, so that is two presses on one pad. The no-catalog case
    is handled the same way.
  - Esc from a keyboard nobody is playing on also leaves.
  - Either way, every seat is emptied and the screen returns to Main.
  - B on an unseated pad still does nothing while anyone is seated.
  - Open regions show "Esc: main menu" under "Enter: keyboard and mouse".
  - Tests: `seat_zeros_back_while_others_sit_leaves_the_screen` and
    `an_unbound_esc_with_pad_seats_seated_leaves_the_screen`. These replace the two tests that
    pinned the old refusal.

### Minors

- **Captures.**
  - The away cell now draws the real notice: `lost_lines` and `draw_lost_notice` moved into the
    lib as `game::split_ui`, and the window re-exports them.
  - Feed lines use profile names through `PlayerNames`: "Han killed Rebel Soldier", "Rebel
    Soldier killed Leia".
- **Lost notice.** It has a third line: "Reconnect and press any button".
- **Unused function.** `split_stats_pages` is now `#[cfg(test)]`, so the release bin no longer
  warns.
- **Game Menu opener vs driver.**
  - New `App::pause_opened_by` keeps the opener. Per-player rows, Respawn, `opener_model`,
    `acting_local` and `controls_for` all use the opener.
  - The prompt icons and pad routing follow the current driver (`pause_owner`).
- **Esc with nobody on the keyboard.** The menu still opens as seat 1, which plan section 15
  allows, but it now says "Paused (keyboard)".
- **Unseated keyboard.** Space continues when `can_start()` holds, so the keyboard does not have
  to sit down to press Continue. Test: `an_unseated_keyboards_space_continues`.
- **Create name.** "Create" suggests the seat's own number first ("Player 3" for P3).
- **FPS counter.** In split it follows the FPS Counter option. The views/render note shows only
  when the counter is on.
- **Seat failure.**
  - A split battle that can seat only one player now says so in the feed: "Splitscreen: only one
    player could be seated on this map".
  - The stderr lines are reworded for the Players-screen path.
- **Battle reclaim.**
  - New `Seats::offer_with` and `PadOwners::reclaim_where`. A pad that is not a waiting seat's
    own does not take a seat whose own pad is plugged back in and has not pressed yet.
  - Limit: two pads that both come back under new ids still go to the lowest waiting seats in
    turn, because nothing tells them apart. A spare pad can still take a seat whose pad is
    still gone. Test: `another_pad_takes_no_seat_whose_own_pad_is_back`.
- **Stale module doc.** The `split.rs` module doc now describes B8's per-cell timer, banner and
  tags.

## Rejected or deferred

- **'Paused by P2' / 'P1 Screen' labels.** Maintainer's call, as the finding says. Left as seat
  numbers.
- **Tag and timer font.** The timer uses the same egui `ITEM` style as the single-player round
  timer, and the tags use the same font as LAN's. Moving both to the native HUD font is a change
  across all modes, not a splitscreen fix.
- **Stats chevrons touching the Awards tab; tiny keyboard glyphs.** The chevrons follow BF2's own
  layout (`stats_draw.rs`: `DOWN_WEDGE` on the box's bottom border, the tab 2 px below) and look
  the same in single player. The keyboard footer glyphs are the shared glyph set. Neither is
  split-specific.
- **Join prompt says 'A' on DualShock.** Deferred. The open region has no seat device. A fix
  needs the active pad's set passed into `PlayersView` and an inline icon in centred body text.
- **Players regions jump from halves to 2x2; title not drawn.** Deferred. The halves layout is
  the plan's deliberate choice, and changing it needs a visual pass on the shell captures.
- **Skip the sky for an away view.** Not done. It is harmless (the finding says so) and touches
  the shared render path.

## Tests

- `cargo test -p game --lib -- local_seats walk::locals round_end name_tags split_ui player_names split_layout`:
  76 passed.
- `cargo test -p game --bin game -- split front menu_actions lan career`: 52 passed, 3 ignored.
- `cargo test -p game --test split_hud_edges -- --ignored`: 3 passed (including the new
  clearance test).
- `cargo test -p input -- owners`: 9 passed.
- `cargo test -p ui --lib -- hud:: hud_draw shell_ps2`: 74 passed.
- One-player gate (release build of this branch): every row matches.
  - bf1_hot1 `f2d22884a7116994`, bf2_uta1 `7152c06c6607554e`, bf2_spa1 `064e247849c498f3`.
  - lan_soak ok, 1559 ticks, 4312 hash checks.
- `split_capture --ignored --test-threads=1`: 2 passed. I looked at these PNGs:
  - b8_tags_side_by_side: the timer is right of the feed.
  - b8_tags_three_swapped: the timer is clear of the feed and the reticle.
  - four_one_away: the notice with three lines.
- rustfmt on the touched files: clean. Unrelated reformatting it made was undone.
- Not mine, seen on wider filters:
  - `spawn_page::tests::heroes::*` (2 failures) and
    `ui typography::menu_modules_take_text_styles_from_typography` (flags
    `hud_frame_identity.rs`). None of these files are touched here; they were not checked on
    `main`.
  - The `game --tests` suite target (`tests/suite/*.rs`) does not compile on `main`: the
    `frame(.., walk.player, ..)` calls predate an `Option<UnitHandle>` signature change.
