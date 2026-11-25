# B16 result: the host's own local players, November 25, 2026

Branch `claude/split-b16` from private main `fe02c11c3`. Net crate only (SPLITSCREEN.md 18.2 B16,
section 9).

## What landed

- `Player.host_local`: slot 0 and each extra player of the host machine. Such a slot has no
  connection. It is in the roster and in `players()`, seated by `load_round` like slot 0, and
  counted as loaded from the start. It is never reserved, expired, kicked, benched or reclaimed by a
  guest token. Every `slot == 0` / `slot != 0` check in `host.rs`, `host/round.rs` and
  `host/admission.rs` that meant "the host's own player" now reads `host_local`.
- `HostCore::add_local(name, profile) -> Option<(u8, String)>` gives the lowest free slot below
  `max_players` and returns the name it was given. Its name is made unique like a guest's
  (`Host` -> `Host_1`), and that returned name is the one in the roster every guest sees. It is
  refused while a round loads, and refused for a bad name. The new queue starts beside slot 0
  (`SlotQueue::start_beside`): numbered from slot 0's last sequence, with every sequence slot 0
  still remembers recorded as a neutral pad, so `add_human` straight after `add_local` (same
  frame) seats it in step with slot 0. `add_local` must be called between frames, before any
  `submit_input_for` of the frame; a debug assert catches a call between two locals' submits. `HostCore::remove_local(slot)` frees the slot, and in a round
  `RemoveHuman` hands it to a bot. `HostCore::host_local(slot)` and `host_locals()` are for B19's
  lobby, which turns its `slot != 0` checks into `host_local` checks.
- `HostCore::submit_input_for(slot, pad, view_tick)`. `submit_input` is now
  `submit_input_for(0, ..)`. A slot that is not a host local is ignored. The host game submits each
  of its locals once per frame, so they all number the same sequences.
- The host's locals count as one machine (`Player::machine_key`: the connection id, or
  `u64::MAX` for the host). That is the same rule as a guest machine's: a grouped close while
  their queues hold the same sequences, and `seat_beside` for one seated late. To make
  `seat_beside` work, `SlotQueue::push` now keeps `recent` the way `receive` does. Slot 0 never
  decodes inputs, so this adds memory and changes no output.
- The test harness gained `host_extra` slots, which submit `random_pad`s beside the host
  player's. `random_pad` was factored out of the guest extra-local code and draws in the same
  order, so B15's pinned hashes are unchanged.

## Checks

- `cargo test -p net --lib`: 73 passed, 0 failed. The filtered `locals_` run before it: 12
  passed. New tests:
  - `locals_host_local_takes_a_slot_and_its_pads_reach_the_bundles` (the acceptance check): host
    plus 1 local plus a guest gives the guest slot 2. A 2-local guest machine then gets 3 and 4.
    `players()` and the `LoadRound` roster are `0..=4`. `kick(1, true)` and `bench(1)` do
    nothing. Every bundle carries slot 1's pad. The guest steps the host's exact bundles. Slot 1's
    mouse sum and FIRE presses match what it submitted. `consumed(0) == consumed(1)` on every
    tick, with over 500 ticks consuming input.
  - `locals_host_local_added_mid_round_lines_up_and_leaves`: `add_local` in a running round,
    then `add_human`. After seating, `consumed(0) == consumed(slot)` for 300 ticks.
    `remove_local` sends `RemoveHuman` and frees the slot. `remove_local(0)` is refused. The
    local is added after a guest joined, so it takes slot 2, above the guest.
  - Review fixes: `locals_host_local_seated_at_once_lines_up` (`add_local` then `add_human` with
    no gap: `consumed(0) == consumed(slot)` from the seating tick for 300 ticks; it fails without
    the neutral-pad seeding), `locals_host_local_name_matches_the_roster` (a guest named `Han`
    exists; `add_local("han")` returns `han_1`, the `LoadRound` roster name), and
    `locals_host_local_added_mid_frame_is_caught` (debug assert).
  - `inputs_one_local_datagrams_unchanged`, B15's FNV pin of every datagram, still passes.
- `cargo clippy -p net --tests`: no warnings in `net`.
- `cargo build --release -p game`, then the one-player gate: every row matches. The `lan_soak` pins
  did not move (`ticks 1559, hash_checks 4312, static_digest dac14dbdede3d9be`).

## Deviations

- The plan names only `add_local`, `submit_input_for` and `host_local`. I also added
  `remove_local` and `host_locals()`. The lobby (B19) needs a way to drop a host seat, and
  `remove_local` gives a running round its `RemoveHuman`.
- `add_local` emits no `PlayerJoined`. The host game makes the call itself and gets back the slot
  and the unique name it was given; B18/B19 must use that returned name (not the profile name it
  passed in) so feed lines and name tags match the guests'.
- Plan section 9 said the host's extra locals are slots 1-3. They take the lowest free slot
  instead, so a host pad that joins after guests sits above them (host 0, guest 1, host local 2).
  Slots never renumber. B18/B19 ask `host_local(slot)` and never assume slots 1-3; SPLITSCREEN.md
  section 9 now says so.
- Grouping the host's locals as one machine, and `push` keeping `recent`, are not in the plan.
  B18's equal per-local pending queues and a returning host pad (B17) both need them.

## Left

B17 local away and back, B18 game netplay, B19 lobby, B20 LAN window, B21 polish. B13b (the HUD
font for the name tags and the round timer, and "Paused by <player name>") is its own packet and
is still open.
