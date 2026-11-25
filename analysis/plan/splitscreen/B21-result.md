# B21 Polish: result (splitscreen, issue #194)

Date: November 25, 2026. Base: private main `34e29f56b`. Branch `claude/split-b21`.
Scope: the six items of `SPLITSCREEN.md` 18.2 B21, each with its named test. The fonts and
"Paused by <name>" asks landed earlier in B22 (`962cb2822`), so they are not in this packet.

## What landed

| Item | Code | Test |
|---|---|---|
| Vehicle and flyer cameras per view | none needed: `render_blend::view_camera` already runs each view's own camera state under `with_local` | `render_blend::tests::view_cameras_follow_p2s_vehicle_and_flyer` |
| Space map HUD per view | the split loop's per-view HUD build moved out of `events.rs` into `split::further_view_models` (same logic) so the test runs the window's own code | `split::tests::a_split_space_round_has_a_hud_per_view_and_no_medal_careers` |
| Class picker per view | none needed | `split::tests::p2s_class_row_moves_on_p2s_pad_only` |
| Death cam per view | none needed | `split::tests::p2s_death_shows_in_view_1_only` |
| Suspend releases P2's guided missile or remote droid | `WalkSim::suspend_local_player` now clears that player's `cam_guided` at once; the sim already lets the rocket or droid go on the next step (its owner unit is gone) | `walk::locals::tests::a_lost_pad_lets_go_of_p2s_rocket_or_droid_only` |
| Suspend and the opener's pause menu | the lost/returned pad handling moved out of `App::seat_split_pads` into the generic `split::apply_presence` (same logic) so it can be tested with `Seats<u8>`; `App::paused_by_line` now takes the menu's driver and names it once the menu has passed on (review fix) | `split::tests::a_lost_pad_under_p2s_menu_passes_it_on_then_suspends_p2_alone` |

What the tests check:
- **Vehicles.** P2 boards a speeder. View 1's camera stays within 20 m of it and follows it when
  it moves 40 m. P2 then boards a flyer, and P2 alone has `cam_flyer`. P1's view camera
  matches a twin round where P2 never boards, in both position and forward.
- **Space.** P1 gets the host career and P2 a seat profile with medals, but neither slot gets a
  medal career, and none appears after a step. Every view gets a HUD model showing its own
  player's health. P2 then sits in a damaged starfighter: view 1's HUD shows the ship's health
  (40/100) and view 0 shows none.
- **Class picker.** Both players are on the class screen. P2's pad holds "next class" through
  `SplitState::finish_pads`. P2's class row moves one step and P1's stays put, in both the spawn
  page models and `spawn_choice`.
- **Death camera.** P2 dies. View 1 gets P2's death camera, a different camera from the on-foot
  one, then P2's spawn screens. View 0 shows no death camera and no spawn page the whole time,
  and P1 keeps walking.
- **Guided rocket and droid.** P1 and P2 each guide a rocket, or P2 flies a droid. P2's pad
  drops. P2's guided view is gone at once and the rocket or droid is released on the next step.
  P1's rocket and guided view are unchanged.
- **Pause menu.** P2 opens the menu and P2's pad drops. The menu passes to P1, its title goes
  from "Paused by Leia" to "Paused by Han", and P2 stays seated while the game is paused. Once the game resumes, P2 alone is suspended and P1 walks on.
  When the pad comes back, P2 returns behind the spawn screens.

## Tests run

- `cargo test -p game --lib -- view_cameras_follow a_lost_pad_lets_go`: 2 passed.
- `cargo test -p game --lib -- walk::locals render_blend`: 34 passed.
- `cargo test -p game --bin game -- split::`: 21 passed (the 4 new ones and the 17 already there),
  again after the review fixes (pause title, starfighter HUD).
- `cargo build --release -p game` and `scripts/one_player_gate.py`: every row matches
  (bf1_hot1, bf2_uta1, bf2_spa1, lan_soak ok).
- `cargo clippy -p game --lib --bin game --tests`: no new warnings in the touched files.
  The dependency crates fail `-D warnings` on this toolchain (audio, mungefs, render). That was
  already the case on main.

## Deviations

- **Pause menu.** I kept today's rule. The Game Menu is never closed by a suspend, because a pad
  is never suspended while the menu is up: `seat_split_pads` runs only on sampled frames and at
  the round's end. A lost pad under the menu passes the menu on through `menu_driver`, and that
  player is suspended once the game resumes. The menu keeps the opener's rows (B10), but its
  title names whoever drives it now, as `SPLITSCREEN.md` §4 and B13b ask ("Paused by" names the
  new owner). B22 had left the opener's name there; review caught it and B21 fixes it. The
  test pins the hand-off, the title and the suspension's result. It does not drive the window's
  frame: that `apply_presence` never runs under the menu rests on `events.rs` skipping the
  pad-sampling branches while paused or with a popup up, and the test's doc comment says so.
- **Four of the six items needed no code change.** The per-view state was already right. Those
  items got their tests, and two pieces of window code were moved into testable functions with
  no change to what they do.
- **No captures.** The only visible change is the pause title's name, which the split capture
  set does not cover, so no split captures were rendered.

## Left

- LAN with several local players: B17 to B20 have not landed on main (B14 to B16 have). The
  batch is not ready to commit or to build for a playtest until they land; after that, re-run
  this packet's lost-pad and menu tests on top of them.
- Two pads returning under new ids at once still go to the lowest waiting seats. This is the
  documented limit and is unchanged.
