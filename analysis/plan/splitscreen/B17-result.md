# B17 result: a local player's pad away and back in a LAN round, November 25, 2026

Branch `claude/split-b17` from private main `ad1825e86`. Net crate, plus two small game-side
hookups (SPLITSCREEN.md 18.2 B17, section 9).

## What landed

- `Msg::LocalAway { local }` (tag 19) and `Msg::LocalBack { local }` (tag 20). A local index of
  `MAX_LOCALS` or more fails the decode. PROTOCOL stays 12, because nothing was published since
  B14. Its history line now names the two messages.
- `ClientCore::set_local_away(local, away)` and `local_away(local)`. Only changes are sent. A
  pad lost before the `Welcome` is kept and sent with it. Added `ClientCommand::SetLocalAway`.
- `HostCore::set_local_away(slot, away)` handles the host's own locals, slot 0 included. It
  returns false for a slot that is no host local. Added `HostCommand::SetLocalAway` and
  `HostCore::is_away(slot)`.
- Host rules (`host.rs` module docs):
  - A lost pad sets `Player.away`. It schedules a `RemoveHuman`, so a bot holds the slot, and
    emits `NetEvent::LocalAway { slot }`. The machine plays on.
  - While a slot is away, `add_human` refuses it. `load_round` leaves it unseated, and it gets
    a tick-0 `RemoveHuman` like any absent seat. It waits in `late_locals`.
  - On `LocalBack`, a removal its own away queued that has not closed yet is cancelled, so the
    slot keeps its seat and no command goes out (only while the slot is seated: an unseated
    slot's tick-0 `RemoveHuman` from the load stays). A slot waiting in `late_locals` is
    released, once its machine is live in the running round, as `NetEvent::LocalBack` (a
    return), not `PeerCaughtUp`, so the game shows no "entered the battle" line for it.
    Otherwise, if the machine is live in the running round (on the round's stream, so a benched
    machine is not), the host emits
    `NetEvent::LocalBack { slot, round_id, tick }` and the host game seats the slot with
    `add_human`. That `AddHuman` goes through B15's `seat_beside`, so the returning queue lines
    up with its siblings.
  - A rejoin (a new link) starts with no slot away. The client tells the host again (B20
    must replay `set_local_away` on any new link; written into its Check).
  - `add_human` refuses a guest's slot whose machine is not on this round's stream (benched).
- Pacing:
  - A guest machine's status already paced from its first seated local (B15). Its starvation
    count now stays one running count when the pacing slot changes. Before, a return to
    local 0 dropped the client's count from 21 to 9, and the pacer read that as a burst. Uses
    `Peer.pace_slot`, `starved_offset` and `starved_sent`.
  - The host's own pacing works the same way through `HostCore::host_pace_slot()`,
    `host_pacing()` and `host_slack_us()`. The meter's `local_*` fields and the game's
    in-process `HostCore` link use these. While slot 0 plays they are slot 0's own figures.
- Game: on `NetEvent::LocalBack` for the current round, `netplay::Peer` sends
  `HostCommand::AddHuman { slot, team: TEAM_AUTO }`, as it does after `PeerCaughtUp`.
  `LocalAway` needs nothing from it. `link.rs` reads the host's pacing through `host_pacing()`.

## Checks

- New tests in `net/src/tests/away.rs`, run with `cargo test -p net --lib away_`: 6 passed.
  - `away_guest_local_goes_and_returns` is the plan's check. A guest's local 2 goes and
    returns on a 40 ± 10 ms link with 3 % loss:
    - The round's only commands are one `RemoveHuman { 3 }` and one `AddHuman { 3 }`.
    - Slots 0-2 are in every bundle, and slot 3 is missing exactly between the two commands.
    - `consumed(2) == consumed(3)` on every tick both are seated (over 600 ticks).
    - Every mouse step from the other players landed.
    - Both guests' bundle logs equal the host's.
    - No bad chunks or bad messages.
  - `away_local_zero_paces_from_its_sibling`: local 0 is away for 1,000 ticks.
    - Local 1's queue depth stays where it was with both seated (1.33 against 1.26).
    - Local 1 starved 12 times in 1,000 ticks, against 6 with both seated and 7-10 for a
      one-local machine on this link (seeds 43-50 measured: seated in a group 5-9, alone
      9-13). The plan's "no growth" holds as "the rate of a lone queue". The grouped close
      merges less, so a group starves slightly less.
    - The client's starvation count only grows, never by more than 3 in a step, through both
      changes of pacing slot.
    - Local 0 returns in step with local 1.
    - Both counterfactuals were run. Pacing from the idle local 0 fails the test (depth 1.33
      to 2.97). Without the running-count offset it also fails (count 21 to 9).
  - `away_host_slot_zero_leaves_and_returns`: the host's slot-0 `RemoveHuman`. Host plus one
    host local plus a guest.
    - The only commands are `RemoveHuman { 0 }` and then `AddHuman { 0 }`.
    - Slots 1 and 2 are in every bundle.
    - `host_pace_slot()` moves to 1 and back, and the meter and pacer read slot 1 meanwhile.
    - `consumed(0) == consumed(1)` after the return, and slot 1's input is intact.
    - The guest's bundle log equals the host's.
    - `set_local_away` on a guest's slot is refused.
  - `away_before_the_round_and_back_in_time`:
    - A pad lost in the lobby leaves its slot out from tick 0 (`RemoveHuman` in bundle 0),
      with no `PeerCaughtUp` or `LocalBack` while away and `add_human` refused. When the pad is
      back the slot is released as `LocalBack` (no `PeerCaughtUp`) and seats in step with its
      sibling.
    - Gone and back within one frame sends no command and no `LocalBack`.
    - A one-local machine's `LocalAway { 2 }` is a bad message.
  - `away_in_the_lobby_back_while_loading` (review fix): pad lost in the lobby, back during
    Loading before tick 0. Bundle 0 keeps `RemoveHuman { 2 }`, then one `LocalBack` and one
    `AddHuman { 2 }` seat it; no `PeerCaughtUp`; bundle logs equal. Fails without the fix
    (the tick-0 removal was cancelled and the slot never seated).
  - `away_back_on_a_benched_machine_is_not_seated` (review fix): a benched machine's local
    going and coming back gives no `LocalBack`, and `add_human` refuses its slots.
- `codec_messages_round_trip` round-trips both new messages, and local index 4 is refused.
- Filtered net runs: `codec_` 8, `inputs_` 6 (`inputs_one_local_datagrams_unchanged`, B15's
  pinned datagram hashes, still passes), `locals_` 15, `session_` 22, `soak_` 3,
  `scheduler_` 12. All passed.
- `cargo clippy -p net --tests`: no warnings in `net`.
- `cargo test -p game --lib netplay::`: 62 passed, 2 ignored (rerun after the review fixes).
- `cargo build --release -p game`, then the one-player gate: every row matches. The
  `lan_soak` pins did not move (`ticks 1559, hash_checks 4312, static_digest
  dac14dbdede3d9be`).

## Deviations

- The plan says `LocalBack` "reseats through `AddHuman`". The host core does not schedule that
  `AddHuman` itself. It emits `NetEvent::LocalBack` and the host game calls `add_human`, as it
  already does after `PeerCaughtUp`, so the team stays the game's choice. The game currently
  sends `TEAM_AUTO`; the sim's join picks the side with fewer humans, which is not always the
  player's old side (a tie goes to team 0). The removal also runs `leave_human`, so the round's
  medals and award progress are lost. Not reachable yet (nothing in the game sends
  `SetLocalAway`, and split is refused in a LAN round), so B20's Check now names keeping the
  old team and the round career through a deterministic suspend/resume, with a test.
- "Hashes match" in the net crate means every guest's bundle log equals the host's, because
  the net harness runs no sim. The game-side check is not built yet: a game `virtual_net`
  test where a guest's local 2 unplugs and replugs with no `suspend_local_player`,
  `resume_local_player` or `install_*` call, and hashes match. The game has no multi-local
  netplay until B18 (per-local pending, `adopt_local`), and split is not allowed in a LAN round
  until B20. That test and the `split.rs` lost-pad path under `netplay.active` (only
  `set_local_away`) go to B20, which the plan already pairs with B17 for the game side.
- Not in the plan, but the acceptance needed them:
  - The running starvation count across a change of pacing slot, for guests and for the host.
  - `host_pace_slot`, `host_pacing` and `host_slack_us` for the host's own slot 0 being away.
  - A one-local machine's status bytes and the host's one-player figures are unchanged (pinned
    datagram test, gate).

## Left

- B18: game netplay for several locals.
- B19: lobby.
- B20: LAN window, including the game side of B17 above.
- B21: polish.
- B13b: the HUD font for name tags and the round timer, and "Paused by <player name>". It is
  still its own packet. The maintainer asked for it on November 25, and it lands before the
  next playtest build.

## Review fixes

- Fixed: a pad back during Loading no longer cancels the load's tick-0 `RemoveHuman` of an
  unseated slot (`if active && cancel_remove`); the slot waits in `late_locals` and is seated
  once the round runs.
- Fixed: a benched machine is not live, so its local's return emits no `LocalBack`, and
  `add_human` refuses a guest slot whose machine is off the round's stream.
- Fixed: a `late_locals` slot that was away is released as `LocalBack`, so returns never print
  "entered the battle" (`RoundState.returned`).
- Recorded for B20 (SPLITSCREEN.md 18.2 B20 Check): the game `virtual_net` away/back hash test
  and the `split.rs` routing; same team and round career on return; replaying lost pads on a
  new link. Split stays refused in LAN rounds until then.
- Rejected: resetting the pacing offsets in `load_round`. `SlotQueue::clear` does not reset
  `starved`, so the count and its offset run on across rounds, and a pacing slot change at a
  round start goes through the same running-count step as any other; nothing jumps.
- After the fixes: `away_` 6, `codec_` 8, `inputs_` 6, `locals_` 15, `session_` 22, game
  `netplay::` 62 passed; release build and the one-player gate: every row matches, `lan_soak`
  pins unchanged.
