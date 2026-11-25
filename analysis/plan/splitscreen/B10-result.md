# B10 result, November 25, 2026 (branch `claude/split-b10`)

Game Menu rows edit the opener's profile (`SPLITSCREEN.md` 18.2 B10, 18.1 item 6), on private
`main` at `4c3866983`.

## What landed

- ui `menu.rs`: `RowScope { Player, Host }`, `row_scope(Row)` (every settings row; `None` for
  Resume, Respawn, Quit, popups, page Back, the class picker and the Game / Audio / Graphics
  category rows) and `action_scope(MenuAction)` with the same split. `MenuModel.shared_rows`:
  when the opener is not P1, every host row's help ends with `SHARED_HELP` ("Shared: this
  changes P1's profile for every player."). `row_help` wraps the old help (`own_row_help`); with
  `shared_rows` false it returns exactly what it did.
- game `pause.rs`: `apply_opener_action(walk, a, paused, menu, opener)`. With `opener` 0 it is
  `apply_menu_action`. For a player after the first, `apply_slot_setting` (under
  `with_local(opener)`) sets their own slot: assist and pad look go through `set_slot_assist`,
  award bonuses through `set_slot_career_award_bonuses`, and the slot's installed
  `netplay.profiles` entry is kept in step. The session's `assist_intensity` /
  `pad_look_sensitivity` (P1's) are never touched.
- `walk/locals.rs`: `fov_scale` is part of `LocalView` and is swapped by `swap_local`. A new local
  starts with the first player's FOV, and `install_seat_profiles` gives each seat its profile's
  FOV. So the FOV row, `view_camera` and `fit_fov_y` all read the opener's FOV under
  `with_local`.
- `menu_actions.rs`: `persist_setting` is split into `persist_player` (the 11 per-player actions,
  into the opener's profile) and `persist_host` (the 14 match-wide ones, into `App.profile`).
  - `acting_local(owner, a)`: a host-row action acts as local 0, whoever pressed it. Every
    other action acts as the opener, and so does ResetGraphics (see the review fixes below).
  - `opener_profile`: `App.profile` for local 0, else `App.split_profiles[seat_number(opener)]`,
    the same entry that `record_round` and the career flush write. A seat with no profile (the
    developer flag) saves nothing: only assist, FOV and viewpoint change, for the round; its
    other per-player rows keep the host's values.
  - `apply_pause_action` takes `&mut split_profiles`, `(seats, inputs)` and `opener`. In split, a
    viewpoint change (the player's own, or a host graphics reset) is set on that player's
    `PlayerInput` at once.
  - `opener_model` fills the per-player model values (assist from the slot, FOV from the
    `LocalView`, mouse, pad look, rumble, fire rumble, career awards, flight, invert, icons,
    viewpoint, name) from the opener's seat profile and sets `shared_rows`.
  - `preview_rumble` buzzes the opener's own pad at their own strength
    (`rumble::preview_pad`).
- `events.rs`: both Game Menu models (before and after the actions) get `opener_model`. The pad
  path and the pointer path both run actions through `acting_local` / `with_local(opener)`.
  Before this, the pointer path always acted as P1, so a keyboard player at local 1 clicking
  Respawn killed P1's soldier. `EditBindings` opens the controls editor on the opener's profile.
- Controls editor (`front/controls.rs`, `front.rs`): `Controls.seat` (`for_seat`). Back saves the
  layouts into that seat's profile and marks that player's pad table stale
  (`PlayerInput::relayout`, rebuilt on the next sample). In split, a pad capture reads the
  pad of the player whose profile the page edits (`App::controls_pad`).

## Review fixes

- Local 0's profile seat: `split::profile_seat(seats, i)` is 0 for local 0 whatever its
  Players-screen seat, else `seat_number(i)`. `seat_settings`, `opener_profile`, the Controls
  editor's seat and its relayout lookup all use it. Before, a match with seat 0 empty gave local
  0 seat 1, so P1's binding edit was saved into seat 1's profile and never applied.
- `split::controls_for` opens the editor (both the pad and the click paths). A seat with no
  profile gets a feed line (`NO_PROFILE_BINDINGS`) instead of an editor whose edits were dropped.
- Capture pad: `App::controls_pad` takes the pad of the editor's own seat, not the menu's current
  driver. With no pad (keyboard, or pad gone) the capture cancels instead of reading another
  player's pad.
- ResetGraphics from P2/P3/P4 acts as the opener: the host's graphics fields reset, the host's
  FOV and view stay, and the opener's own FOV (`LocalView`), profile FOV/viewpoint and stamped
  viewpoint reset. Its help line is `RESET_SHARED_HELP`.
- Viewpoint on a seat with no profile toggles that player's own `PlayerInput`, no longer copies
  P1's.
- The egui fallback menu (no pause art) drops its clicks unless the keyboard drives the menu, as
  the native pause path does.
- New tests: `the_first_player_keeps_the_hosts_profile_in_any_seat`,
  `p2s_reset_graphics_resets_their_own_fov_and_view`, `a_seat_with_no_profile_toggles_its_own_view`.
  `cargo test -p game --bin game -- menu_actions:: split:: front::`: 41 passed, 2 ignored;
  `cargo test -p ui --lib -- menu::`: 51 passed. Gate re-run after a release build: every row
  matches, `lan_soak` included (4312 hash checks).

## Tests (filtered)

- `cargo test -p ui --lib -- menu::`: 51 passed. New:
  `split_menu_tests::every_settings_row_and_its_actions_share_one_scope`. It covers every row
  on the Game, Audio, Graphics, Keyboard/Mouse, Gamepad and Splitscreen pages: each has a scope,
  each action it makes (press, Left, Right) has that same scope, and "Shared" appears only on host
  rows and only with `shared_rows`.
- `cargo test -p game --lib -- pause:: walk::locals`: 26 passed. New:
  `pause::tests::p2s_assist_is_their_slots_and_p1s_the_sessions`.
  `walk::locals::tests::a_one_player_rounds_saved_profile_is_unchanged` (B3's save pin) still
  passes.
- `cargo test -p game --bin game -- menu_actions:: split:: front::`: 38 passed, 2 ignored. The
  new `menu_actions` tests are:
  - `p2s_invert_writes_p2s_profile_only_and_the_round_keeps_it`: the change lands in the
    `split_profiles[1]` entry, and `record_split_rounds` on that entry keeps it.
  - `p2s_assist_sets_their_slot_and_leaves_the_session`: `slot_assist(1)` is set and
    `sim.assist_intensity` is unchanged; pad look and award bonuses too.
  - `a_host_row_from_p2_writes_p1s_profile`: difficulty, effects volume and name tags go to the
    host's profile and P2's is untouched; P2's model shows P2's values and is shared.
  - `p2s_fov_and_viewpoint_are_their_own`.
  - `host_rows_act_as_the_first_player`.
- Save identity of the menu path: a temporary probe (not kept) ran 28 actions, each twice. It
  applied every persisted action plus Resume, a cheat and a split row through `main`'s
  `persist_setting` and through the new `apply_pause_action` with opener 0. The host profile's
  serialized settings, the session options, the walk's FOV and its assist were equal after every
  step.
- Gate: `cargo build --release -p game`, then `scripts/one_player_gate.py`. Every row matches,
  `lan_soak` included (4312 hash checks).

## Deviations

- The plan names `pause::` / `menu::` tests. The three acceptance checks need `App.split_profiles`
  and the windowed `apply_pause_action`, so they live in the bin's `menu_actions::tests`.
  `pause::` and `menu::` each get one test of their own part.
- `row_scope` lives in ui (`RowScope`), beside `Row`. B1's `game::profile::SETTING_SCOPES` still
  lists the `Settings` fields with the same split; the ui test checks rows against actions, not
  against that list.
- ResetGraphics stays a host row (graphics) in `row_scope`, but it acts as the opener: from
  P2's menu it resets the host's graphics and P2's own FOV and view.
- Pad look for P2 was already read live from the seat profile by the pad sampler (B3). B10 also
  sets the slot's `AssistProfile.pad_look_sensitivity`, which the aim assist reads.
- The LAN path (`netplay.pause_action`) is unchanged (one local per machine until B20).
- In split, P1's Game Menu viewpoint toggle now also updates P1's stamped `PlayerInput`.
  Before, it changed only the profile until the next battle.

## Left

- The controls editor's key names use the host's icon choice (`controls_model` reads
  `App.profile.settings.input_icons`); the footer icons are already the opener's.
- The maintainer's check with real pads (the first playtest build follows B10): P2 opens the
  Game Menu, changes pad look sensitivity (or flight controls), FOV, assist and a binding; each
  one takes effect on P2's view only, and a host row shows "Shared". Invert Mouse Y affects only
  the mouse, so check it only with P2 on the keyboard and mouse.
- A seat with no profile (developer flag) still shows the host's values in rows like rumble and
  invert, which do nothing for it; only the Controls editor is refused.
