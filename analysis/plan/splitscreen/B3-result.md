# B3 result, November 25, 2026 (branch `claude/split-b3`)

Battle hookup for seat profiles (`SPLITSCREEN.md` 18.2 B3), on private `main` at `fddea3b0b`.

## What landed

- `netplay::install_local_profile(walk, slot, &PlayerProfile)`: `install_profile` minus the cheats
  (aim assist, medal career, award bonuses, the profile and its points baseline in
  `walk.netplay`). `install_profile` is now "cheats, then `install_local_profile`", in the same
  order as before. `gate` is split into `gate_slot`, and `gate_locals` runs it for locals 1-3.
- `App::install_seat_profiles` (`main/windowed/split.rs`) runs once, right after `split::seat`, at
  both seating sites (`enter_battle` and the dev `walk_requested` path). For each local `k >= 1`
  whose seat holds a profile (`App.split_profiles[seat_number(k)]`), it installs that profile for
  `k`'s slot and sets `k`'s viewpoint. It then re-reads `split_feeds[k-1].set_opening_medals`
  under `with_local(k)`, and if the keyboard seat is not P1, gives the keyboard that seat's mouse
  settings and keys. The resume path (`sample_split_pads`) never installs anything. It only
  applies the seat's award-bonus preference again, and `restore_slot` gives back the career.
- Per tick, offline split only: each extra feed's `tick` points go to
  `score_medal_points(local_unit(k), ..)`, and `gate_locals` runs beside `set_player_gate` (P2-P4
  pay their own class costs). `pending_class_kills` is now part of `LocalView` and is swapped by
  `swap_local`. `track_class_kills` runs for each extra local, using its own kills from before
  the step.
- Careers are flushed per seat and every round is recorded per seat. The helpers are
  `WalkSim::flush_local_careers` and `record_local_battles` (`walk/locals.rs`) and `career.rs`'s
  `flush_split_careers` / `record_split_rounds`. They write the one `App.split_profiles` entry
  routed by `seat_number(k)`, never seat 0, and save it. They run at the frame flush and at all
  three `record_round` sites (`events.rs` outcome, `close.rs`, `leave_battle`). In
  `leave_battle`, the seats are snapshotted before `self.split.take()`. B2's `init_shell` already
  moves the profiles back.
- Per-seat input settings in battle: pad bindings, flight controls, pad look (`sample_split_pads`),
  viewpoint, prompt icons (`split_icons`), rumble strength and fire rumble (`ExtraRumble::tick`),
  and the keyboard player's mouse (`kbm.follow`) and keys (`refresh_context_bindings` through
  `App::keyboard_settings`). `leave_battle` gives the keyboard back to the host's profile.
- core: `SlotKeep` and `MedalCareer` derive `Debug` and `PartialEq`, and `SlotKeep::following(life)`
  was added. This is a test aid only, with no sim change.

## Tests (all filtered)

- `cargo test -p game --lib -- walk::locals netplay::`: 70 passed, 2 ignored. The new tests in
  `walk::locals` are:
  - `p2s_kills_class_kills_and_medals_land_in_p2s_profile_only`
  - `p2_pays_its_own_class_costs`
  - `each_locals_round_lands_in_its_own_seats_profile` (seats 0, 1, 2, in identity and swapped
    order)
  - `p2s_counters_add_up_across_two_rounds`
  - `p2s_medal_progress_and_bonus_survive_a_lost_pad`: after the resume, `keep_slot` equals the
    pre-suspend keep, and the bonus is still active.
  - `a_one_player_rounds_saved_profile_is_unchanged` (Save identity, see below)

  `a_lost_controller_leaves_a_bot_and_the_others_play_on` still passes.
- `cargo test -p game --bin game -- split:: front::profiles`: 12 passed. The new test is
  `split::tests::a_seat_profile_reaches_its_player_and_takes_its_round`.
- Gate: `cargo build --release -p game`, then `scripts/one_player_gate.py`. Every row matches,
  `lan_soak` included (4312 hash checks).
- Save identity: `a_one_player_rounds_saved_profile_is_unchanged` is a scripted one-player round
  (a kill, class kills flushed every 30 ticks, a medal, the round recorded) that hashes the saved
  RON entry. The pin `3505798455005844255` was produced by the same script on `fddea3b0b`'s code
  without the two new per-local calls, and this branch gives the same bytes.
- Clippy: no new warnings in touched files. rustfmt was run on the touched files only.

## Deviations

- Offline extra locals go into `walk.netplay.profiles` / `baseline`, as `install_profile` does,
  so the class gate is `netplay::gate_slot`. `TickState::new` seeds the other slots' medal
  counts only in a LAN round (`netplay.active`), so offline P1's feed does not print
  "Player 2 earned ..." lines. Per-view award lines (per-view `medals_seen`, `note_medals` in
  the split loop, profile names) are still B4's.
- The seat profiles live in B2's `Party` and `App.split_profiles`, not in `FrontShell.seat_profiles`.
  P1 is always recorded to `App.profile` directly (the host rule: local 0 is seat 0). The helpers
  never write seat 0.
- The resume re-applies only the award-bonus preference.

## Review fixes (same branch)

- Lost pad, round counters: `suspend_local_player` banks the unit's kills and deaths, the
  slot's gate points and the side in a per-local `RoundCarry` (in `LocalView`, swapped like
  `pending_class_kills`; the first player's lives on `WalkSim`). `record_battle` adds the
  banked kills and deaths. A player still away at the round's end is recorded from the bank
  alone, won or lost on the banked side, and never reads the unit index a bot may now hold.
  For a player who dropped during the round, `record_local_battles` takes the round's points
  from the class gate, not from the feed's row for the unit index. A one-player round has an
  empty bank and takes the old path. The save-identity pin is unchanged.
- Lost pad, class gate: `resume_local_player` clears the slot's points baseline (the new unit
  may sit on another index, whose round row is someone else's) and restores the banked points.
  `gate_slot` takes a `carried` term, which is 0 in LAN and the bank in `gate_locals`.
- Class costs follow mid-round unlocks: when `flush_local_careers` changes a seat's profile, it
  refreshes that slot's `netplay.profiles[slot].unlock_kills`
  (`PlayerProfile::unlock_kills_of`). P2 then unlocks on the next tick, as P1 does.
- Game Menu actions no longer reset the keyboard player's mouse to the host's settings:
  `apply_pause_action` follows `split::keyboard_settings_of` (the keyboard seat's profile, else
  the host's).
- Extra pads use their own profile's per-context layouts (`gamepad_layouts`, vehicle and
  flyer), cached per player in `PlayerInput` and rebuilt when their control context changes.
- A local with no seat profile (the developer flag) has its pending class kills cleared at each
  flush, so they no longer pile up.
- New tests: `walk::locals::a_lost_pad_keeps_p2s_round_counters_and_side` (away at the end and
  back again), `p2s_spawn_points_survive_a_rejoin_on_another_index`,
  `p2s_class_costs_follow_their_flushed_kills`, and
  `menu_actions::tests::a_game_menu_action_keeps_the_keyboard_players_mouse_settings`.
  Lib `walk::locals netplay::`: 73 passed, 2 ignored. Bin `split:: front::profiles
  menu_actions::`: 13 passed. The gate was run again after a release build: every row matches,
  `lan_soak` included (4312 hash checks).

## Left

- B4 (names and per-view award lines), B9 (per-view stats pages from these careers) and B10
  (Game Menu rows writing `App.split_profiles`).
- P2-P4's prompt icons still use the base pad table in vehicles; only the pad input follows
  the per-context layout.
- The maintainer's check with real pads: P2's medals and kills reach P2's profile in the
  Players screen's career line after a round, a pad drop included.
