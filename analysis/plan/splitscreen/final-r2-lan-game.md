# Splitscreen final review, round 2: LAN game fixes

## 2026-11-25

Branch `claude/split-final-r2-lan-game`, from main `fe88b0398`.

### refused-host-local-vague-notice (minor)

When the host core refused one of the host's own further players, the lobby said only
"<name> could not join this game". It did not give a reason.

- `HostCore::add_local` now returns `Result<(u8, String), RejectReason>`. It checks in the
  same order as a guest machine's Hello: `BadName`, then `Busy` (a round loading), then
  `Full` (no free slot). `NetEvent::LocalAdded` and `Notice::LocalAdded` carry that result.
- `netplay::lobby::local_refusal_text` gives the lobby line in `reject_text`'s words. For
  example: "Leia could not join: the game is full", and "Leia could not join: the host is
  loading a battle; try again in a moment". The lobby in `windowed/lan.rs` uses it. The
  seat is still dropped from the session's locals, so no cell is left waiting.
- Tests: `net` `locals_host_local_refusal_names_its_reason`, which checks all three
  reasons from the core, and `game` `refused_host_local_line_names_the_reason`. The
  existing `netplay_host_locals_are_the_hosts_machine_and_the_walk_follows_the_peer`,
  `locals_host_local_takes_a_slot_and_its_pads_reach_the_bundles` and
  `away_host_slot_zero_leaves_and_returns` were updated to expect `Ok`.

### lan-split-cheats-act-on-p1-slot (major)

In a LAN round, P2-P4's Cheats page and Morse words always read and changed P1's slot.

- `GameRequest::CheatToggle` now carries the target `slot`. `validate_request` takes the
  asking machine's slots and refuses any other slot with the new `Refusal::NotYours`. The
  host peer builds that list with the new `Peer::machine_slots`: a guest machine's local 0
  plus the guests whose `machine` it is, or the host's own `local_slots`. So a guest machine
  can change its further players' cheats, but no one else's.
- `NetplaySession::cheat_entries`, `toggle_cheat` and `morse_word` now take the opener's
  slot. The host's first player keeps the target picker and the awards row. Any other
  player sees their own slot's cheats. A guest's list is filtered by the host's policy;
  the host machine's further players get every cheat. A toggle or a word goes to that slot.
- `events.rs` passes `menu_actions::opener_slot(walk, menu_owner)` for the model's cheats
  and for words. Pause actions in a LAN round use the new `menu_actions::lan_acting_local`:
  ToggleCheat acts as the opener, and every other action is still decided by
  `acting_local`. Offline cheats are still session-wide and unchanged.
- Help: the new `MenuModel::own_cheats` is set in a LAN round. With it, the Cheats rows say
  `OWN_CHEATS_HELP` ("Yours: this changes only your own soldier's cheats.") instead of
  `SHARED_HELP`. Other host rows still say they are shared.
- Tests:
  - `netplay_a_guest_further_players_cheat_toggle_changes_only_their_slot`, on the virtual
    net: a guest machine with 2 players; local 1's toggle changes only its slot on the host
    and guest walks, and a request for slot 0 is refused.
  - `netplay_requests_are_checked_against_the_policy`, rewritten: the own-machine slot, the
    further player's slot, `NotYours`, the policy, and malformed requests.
  - `lan_a_further_players_cheats_page_is_their_own_slot`: on the host, P1 gets the target
    page and a further player gets their own rows.
  - `lan_cheat_rows_act_as_the_opener`.
  - `ui` `lan_cheat_rows_say_they_are_the_openers_own`.

### b20-lan-menu-capture-shows-restart-row (minor)

- The `lan_menu` fixture in `tests/split_capture.rs` now sets `restart_available: false`.
  A LAN round has no `App::current`. It also sets `own_cheats: true`, as the LAN menu does.
- Recaptured. `b20_lan_menu_in_cell_1920x1080.png` was looked at: Han's menu (battle runs
  on) shows Resume Game / Options / Respawn / Quit, with no Restart Mission row.

### Checks

- `cargo test -p net locals_host` (7), `cargo test -p net away_host` (1),
  `cargo test -p ui --lib split_menu_tests` (8), `cargo test -p game --lib netplay::`
  (74 passed, 2 ignored), and `cargo test -p game --bin game -- lan_ host_rows_act_as` (16).
  All passed.
- `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches,
  including lan_soak ok.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1` with absolute
  `HUD_ASSETS`: 3 passed.

Left: none.
