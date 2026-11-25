# B15 result: LAN inputs for several local players, November 25, 2026

Branch `claude/split-b15` from private main `650d6979a`. Net crate only (SPLITSCREEN.md 18.2 B15,
section 9).

## What landed

- `Chunk::LocalInputs { idx, data }` (chunk kind 9, `PacketWriter::local_inputs`): the inputs body
  of extra local `idx` (1..), prefixed by the index byte. Local 0 keeps the plain `Inputs` chunk,
  so a one-local client's datagrams are byte-identical. `idx` 0 fails the parse. PROTOCOL stays
  12 (B14 bumped it; nothing was published in between), with its history line extended.
- `ClientCore::submit_inputs(&[PadState], view_tick)`: one sequence for all the machine's locals
  (a missing pad is neutral). `submit_input` is now `submit_inputs(&[pad])`, so the game is
  unchanged. `ClientCore::locals()`. Each packet gives every local an equal share of the room and
  the same count (it re-encodes the longer bodies to the shortest), so all of a machine's slots
  receive the same sequences in the same datagram. With one local, the budget is the old one.
  Per-local delta bases (`acked_pads`) advance together on the host's `input_through`.
- Host routing: `LocalInputs { idx }` goes to `Peer.extra[idx - 1]`. An index with no slot, or a
  connection still in its handshake, counts as a bad chunk.
- `HostStatus.input_through` is the minimum of `recv_through` over the connection's slots. Depth,
  starvations and slack are the first active slot's (local 0 unless its pad is away, when its
  queue no longer closes), one pacer per machine.
- Grouped close (`SlotQueue::close_group`, `same_queued`). `close_tick` closes a connection's
  active slots together while their queues hold the same sequences (the whole queued list, not
  only count and ends). A slip merge or length merge
  happens at a pair only when every queue can merge it. Without this, the plan's acceptance
  could not hold: merges depend on pad content (a pair whose merge would lose a FIRE edge is
  skipped), so two locals' queues consumed different sequences (tick 1: 3 against 7). A lone
  queue still uses `close`, which `pop_close` / `try_merge` / `apply_merge` split out with no
  change in behaviour.
- Late seating (`SlotQueue::seat_beside`). An `AddHuman` for a slot whose connection has a
  sibling that already stepped this tick lines it up with that sibling: it takes the sibling's
  consumed sequence (its own pad at that sequence, a repeat when the sibling starved) and queues
  its own received inputs (kept in `recent` while it was not seated) for the sibling's queued
  sequences, folding a merged run as a forced merge. From the next tick the two close as a
  group. If its `recent` no longer covers them it falls back to the old empty start.

## Checks

- `cargo test -p net --lib`: 67 passed, 0 failed. New tests:
  - `inputs_one_local_datagrams_unchanged`: FNV hashes of every datagram from two one-local
    clients and from the host, over 600 ticks at 3 % loss. Pinned on the pre-change build and
    unchanged after.
  - `inputs_two_locals_consumed_at_the_same_sequences`: linksim 40 ± 10 ms, loss 0.03, 2,000
    ticks, a 2-local machine beside a 1-local one. Both slots' mouse sums and FIRE edges are
    conserved, the two slots' pads differ, `consumed(2) == consumed(3)` on every tick, there are
    no bad chunks, and nothing is left unacknowledged.
  - `inputs_unknown_local_is_a_bad_chunk`.
  - `scheduler_group_merges_only_where_all_can`.
  - `inputs_late_seated_local_consumes_with_its_sibling`: local 1 is taken out and seated again
    6 times mid-round over the lossy link (the sibling held 1-2 queued inputs at each seating);
    `consumed(1) == consumed(2)` on every tick both are seated. Without `seat_beside` it fails
    (tick 663: 674 against 675).
  - `scheduler_seat_beside_lines_up_with_the_sibling`, `scheduler_same_queued_compares_every_sequence`.
  - `scheduler_four_locals_depth_against_a_lone_queue`: 4 locals with independent FIRE traffic,
    6,000 ticks of bursty arrivals. Group mean depth 3.36 (max 5, 878 merges) against a lone
    queue's 3.35 (max 5, 879 merges); no forced merges.
  - The harness gained per-client and host sent-byte hashes and extra-local pads
    (`extra_locals`, `extra_submitted`).
- `cargo clippy -p net --tests`: no warnings in `net`.
- `cargo build --release -p game`, then the one-player gate: every row matches. The `lan_soak`
  pins did not move (`ticks 1559, hash_checks 4312, static_digest dac14dbdede3d9be`).

## Deviations

- The grouped close is new and not in the plan. The acceptance needs it (see above). It also
  keeps the game's per-local pending queues the same length, which B18 needs (section 9 r3).
- A slot seated later than its siblings is lined up with them (`seat_beside`, above). B17's
  `LocalBack` must seat through the same `AddHuman` path (or call `seat_beside`) so a returning
  pad keeps the alignment.
- Pacing figures are one slot's, not combined (the first active one). Every local's inputs arrive
  in the same datagram, so their depths are equal while grouped.
- `submit_inputs` ignores pads past the machine's locals, as documented (the debug assert that
  contradicted the doc is gone).

## Left

- B16: the host's own locals.
- B17: local away and back.
- B18: game netplay. It has to call `submit_inputs` with every local's pad and read
  `consumed(slot)` per local.
- B19: lobby.
- B20: LAN window.
- B21: polish.
- B13b: the HUD font for the name tags and the timer, and "Paused by <name>". It is its own
  packet, not this one, and the maintainer tested for it: it should land, with its 2/3/4-cell
  captures looked at and the Paused-by test, before the next playtest build.
