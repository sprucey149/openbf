# Final round 1, LAN findings: November 25, 2026

Branch `claude/split-final-r1-lan` from private main `0f6a28b32`. One builder, area `lan`.
Each finding below names what changed and the test that pins it.

## Findings and what was done

- **lan-split-paused-by-while-running, lan-paused-by-while-running, lan-split-menu-says-paused**
  (one defect, reported three times). In a LAN split round, the Game Menu said "Paused by
  <name>" while the round and the other cells ran on.
  - `App::paused_by_line` now checks `self.netplay`. In a LAN round it returns
    `game::split_ui::lan_menu_line`: "Han's menu (battle runs on)", or
    "Keyboard menu (battle runs on)" when the free keyboard opened it.
  - Offline split is unchanged ("Paused by <name>", "Paused (keyboard)").
  - Test: `windowed::split::tests::paused_by_names_the_opener` has a new LAN case. It holds a
    real hosted `NetplaySession` on a free port, asserts the new line, and asserts that the
    line never contains "Paused".
  - Capture: `b20_lan_menu_in_cell_1920x1080.png` now draws the line from the same function.
    I looked at it. "Han's menu (battle runs on)" sits over the right cell's menu in the HUD
    font and fits the cell. The left cell's HUD stays up.
- **lan-free-keyboard-esc-covers-p1.** In a pad-only LAN split round, Esc on the empty
  keyboard opened the menu over P1, who played blind under it.
  - `App::pause_opener` now refuses that Esc while any seated player's pad is connected
    (`split::lan_free_keyboard_opens`). I chose this over a whole-window overlay. The round
    runs on under a LAN menu, so any menu the free keyboard opens covers someone who is
    playing. Every seated player can open their own menu with Start.
  - Once every seated pad is gone, the keyboard is the only way out. Esc then opens the menu
    as before, with nobody left to cover.
  - That menu closes as soon as any seated pad comes back (`split::lan_menu_closes`), so it
    never stays over P1 once P1 is playing again.
  - Offline split is unchanged (it really pauses).
  - Test: `windowed::split::tests::a_lan_menu_closes_when_its_openers_pad_goes_and_is_never_passed_on`
    (the free-keyboard part).
- **lan-menu-handoff-freezes-other-player.** In a LAN round, the opener's lost pad passed the
  menu to another local player. That player's soldier then stood frozen in the live round,
  while the menu stayed drawn in the away opener's cell.
  - The per-frame hand-off moved out of the gilrs block in `windowed.rs` into
    `App::settle_split_menu`. The block now records each seat's connection, then calls it.
  - Offline, the method still passes the menu on (`menu_driver`, unchanged). Under
    `netplay`, it closes the menu instead (`lan_menu_closes`). The opener is already marked
    away by `lan_presence`, and a bot stands in.
  - An open Controls page closes with the menu, unsaved, as Back would leave it.
  - A keyboard player's menu never closes because of a pad.
  - Test: the same new test. Offline, a drop passes the menu to P1. In LAN the same drop
    closes it, and `lan_menu_view` is then `None`.
  - Test: `a_lost_pad_under_p2s_menu_passes_it_on_then_suspends_p2_alone` (offline) still
    passes.
- **lan-addhuman-stale-session-profile.** A rejoining or late-seated player got the profile
  the net core had held since the session opened. Fixed in two layers.
  - Net core: new `HostCore::set_profile` and `HostCommand::SetProfile`. Each time a round
    loads, `Peer::host_load` sends every setup seat's profile to the core before `LoadRound`.
    This covers a guest's last `Ready`, the host's locals refreshed from their seat profiles,
    and slot 0's current profile. A later `AddHuman` (a late local after `PeerCaughtUp`, a pad
    back) then carries the latest profile, not the `Hello` one.
  - Game side: `KeptSlot` now also keeps the `PlayerProfile` the slot was playing on. A
    `TEAM_KEPT` return installs that profile, not the command's bytes, so class unlocks and a
    mid-round Career Awards switch survive. It is deterministic, because every peer keeps the
    same profile from the bundles.
  - `SetCareerAwards` also updates the kept profile of a slot that is away. The award switch
    is applied again after `restore_slot`, because the kept career carries the switch as it
    was at removal. A switch made while the pad was gone, or a host policy change, therefore
    holds.
  - Tests: `netplay::tests::netplay_a_kept_return_keeps_its_unlocks_and_awards_switch`
    switches Awards off before and during the absence, then returns with stale bytes. Unlocks
    and the switch hold in the profile and the sim, and `TEAM_AUTO` still takes the
    command's profile.
  - Test: `net` `tests::away::away_return_carries_the_profile_set_at_load` checks that
    `set_profile` before the round is the profile the `LocalBack` `AddHuman` carries, and
    that an unknown slot is refused.
- **lan-full-reject-with-locals.** A machine with several players was refused with only "The
  game is full".
  - The host's `Full` refusal now carries `room R of W` (`net::host::full_detail`). R counts
    the machine's own reclaimable slots plus the free slots.
  - `reject_text` says "The game has room for 1 more player; 2 are on this computer. Join
    with fewer players", and keeps "The game is full" when nothing or everything would fit.
  - Tests: `net` `tests::locals::locals_full_when_not_all_fit` checks the detail is
    `room 1 of 2`. `netplay::lobby::tests::reject_reasons_read_as_plain_text` checks the text.
- **no-real-udp-lan-round-with-locals (blocker).** `--net-host` and `--net-join` now take
  `--locals N` (1 to 4) and `--away S`.
  - A guest's `Hello` carries `LocalHello` seats 1 to N-1. The host adds its own locals with
    `HostCommand::AddLocal` and seats them from `LocalAdded`.
  - Every local plays its own scripted pad through `frame_locals`.
  - `--away S` takes the machine's last local's pad away S seconds into the round, for
    10 s (`Peer::set_local_away`). It comes back through `LocalBack`, then `TEAM_KEPT`.
  - The host now ends the round for everyone at its outcome (`EndRound`), or at
    `--seconds`, and lingers 3 s so the guests hash-check the last ticks.
  - The summary JSON lists each local's slot and whether it is seated, the away phase, and
    the outcome.
  - Real runs over 127.0.0.1 with separate processes are under "Runs" below.

## Checks

- `cargo test -p net`: 85 passed.
- `cargo test -p game --lib -- netplay::`: 72 passed, 2 ignored.
- `cargo test -p game --bin game -- windowed::split net_real windowed::lan windowed::lobby`:
  all passed.
- `python3 scripts/one_player_gate.py --bin .../release/game --assets .../assets_baked`:
  "every row matches", with a LAN soak of 1559 ticks and 4312 hash checks.
- `cargo test -p game --test split_capture -- --ignored --test-threads=1`: 3 passed. I looked
  at `b20_lan_menu_in_cell`.

## Runs

These runs used the release `game` from this branch, as separate host and guest processes
over real UDP on 127.0.0.1. The map was bf1_tat2 Conquest, and each round ran to its outcome.

- **2 + 2: two players on the host and two on the guest.**
  - Commands: `--net-host --port 47831 --players 4 --locals 2 --seconds 1500 --away 60` and
    `--net-join 127.0.0.1:47831 --locals 2 --seconds 1500 --away 120`.
  - Each machine's second local lost its pad: the host's slot 1 at 60 s, the guest's slot 3
    at 120 s. Each came back 10 s later.
  - The round was decided at tick 17241 (outcome 0).
  - Host: `PASS: 575 hash checks, 0 desyncs`, locals `[[0,true],[1,true]]`.
  - Guest: `PASS: 575 hash checks, 0 desyncs`, locals `[[2,true],[3,true]]`.
  - The static digest was `a1a97c1f29b6788a` on both, with 0 digest mismatches.
- **2 + 1: the maintainer's case.** One player was on the host and two on the guest.
  - Commands: `--net-host --port 47832 --players 3 --seconds 1500` and
    `--net-join 127.0.0.1:47832 --locals 2 --seconds 1500 --away 90`.
  - The guest's slot 2 lost its pad at 90 s and came back 10 s later.
  - The round was decided at tick 16838 (outcome 1).
  - Host: `PASS: 562 hash checks, 0 desyncs`. Guest: `PASS: 562 hash checks, 0 desyncs`,
    locals `[[1,true],[2,true]]`.
  - The static digest was `40fdf33cb1f1d990` on both.
- An earlier 45 s smoke run (2 + 2, guest away at 10 s) passed with 46 and 45 hash checks
  and 0 desyncs.

## Left

- The "free keyboard" menu that opens once every pad is gone is pinned by
  `settle_split_menu` and `lan_free_keyboard_opens`. The window's gilrs path that feeds them
  is not driven by a test, because tests have no pads. A playtest on hardware should check
  this: in a pad-only LAN split round, Esc opens nothing; with every pad unplugged, Esc opens
  the menu; plugging a pad back in closes it.
- These runs were on one machine (loopback). A two-machine LAN or VPN run with `--locals`
  is still to do with the next playtest build.
