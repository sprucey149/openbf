# B18 result: game netplay for several local players, November 25, 2026

Branch `claude/split-b18` from private main `928626ca1`. Game `netplay/` and `walk/`, plus two
small additions to `net` (SPLITSCREEN.md 18.2 B18, section 9).

## What landed

- `Peer` (`netplay/peer.rs`): each further local player is a `LocalQueue` with its slot, its own
  pending inputs and sequences, and its own `InputPacer`. That pacer is a `follower()` of local
  0's pacer, so it runs on local 0's clock. Every frame, every pacer gets the same `dt`, the same
  host report and the same slack, so all of them send the same number of input ticks. Each tick
  goes out for every local under one sequence through the new `Link::submit_inputs`. Each stepped
  bundle's `consumed(slot)` trims each local's own queue.
  - A guest gets its extra slots from `Welcome.extra`. The host's caller passes the slots
    `HostCore::add_local` returned to `Peer::set_extra_slots`.
  - `round_built` adopts every extra slot into the walk.
  - New readers: `frame_locals(pads)`, `extra_locals`, `local_count`, `local_slot_of`,
    `pending_inputs_of`, `pacer_of`. `frame(pad)` is `frame_locals(&[pad])`, and with one local it
    makes exactly the calls it made before.
- `Link::submit_inputs(&[(slot, pad)], view_tick)` is implemented for `HostCore`
  (`submit_input_for` per slot), `ClientCore` (`submit_inputs`), both `NetSession`s, the
  window's `NetLink` and the harness's `NodeLink`. The host's `NetSession` sends a frame's row as
  one `HostCommand::SubmitInputsFor`: its thread can close a tick between two commands, and one
  command per slot then starved the later slots at that close (review fix).
- Predictor (`netplay/predict.rs`): the per-slot state is now a `Track` (poses, ease, shot
  ledger, `through`, confirmed poses, what is swapped in, shots). Local 0's track is the old
  state.
  - `update_locals` steps the one predicted copy with every local slot playing its own pending
    input. The other humans repeat their last bundle pad, so no local slot is predicted from a
    stale pad.
  - Each local gets its own poses, shots and correction.
  - New: `set_extra_slots`, `poses_of`, `shots_of`, `confirm_shots_of`, `swap_in_local`,
    `depth_of`, `index_of_slot`. `swap_out` puts back every track, the last one first.
  - Uneven queues (a late seat): the copy goes as deep as the longest queue, but each track's
    poses and shots stop at its own inputs (`depth_of(i)`), and a queue that grows into ticks the
    copy stepped with a repeated pad re-predicts. The meter's correction figures are local 0's.
  - `update` is `update_locals(.., &[])` and gives the same result as before.
- `LocalView` (`netplay/view.rs`): `frame_locals(pads)` keeps look pads per local. Its `each`
  gets a muted bitmask, with bit `k` for walk local `k`. `frame` keeps its bool. Also new:
  `draw_in_local(walk)` (used inside `with_local(k)`), `look_pads_of_slot` and `shots_of_slot`.
  - Per-local parts are found by slot, never by position: the walk's locals only grow, while
    `Peer::set_extra_slots` may drop or reorder. A walk local whose slot the peer no longer
    holds gets no prediction, muted bit or look pads, so it never takes another seat's.
  - Each local's look fold starts after its own predicted inputs (`depth_of`), not after the
    deepest local's.
- Walk (`walk/locals.rs`, `walk.rs`, `netplay/apply.rs`):
  - `WalkSim::adopt_local(slot)` seats nothing; the bundles' `AddHuman` seats the slot.
  - `follow_locals` runs on `AddHuman` and in `adopt_round`, which now re-follows every local.
  - `is_waiting(i)` (adopted, not seated yet) is split from away. `is_suspended` still means
    "no unit", and its doc says so.
  - A waiting local's per-player tick does not run.
  - Under `netplay.active`, `step_tick` no longer adds the further locals' own RESPAWN presses
    and leaves them set. The window takes one with `WalkSim::take_respawn_of(i)` and puts RESPAWN
    in that local's submitted pad (B20 wires it).
- `net`, not in the plan's file list:
  - `InputPacer::follower()`.
  - `HostCommand::SubmitInputFor`, `HostCommand::SubmitInputsFor` (one frame's row, atomic) and
    `ClientCommand::SubmitInputs` for the threaded sessions.
  - No wire change, and PROTOCOL stays 12.
- Harness (`netplay/virtual_net.rs`):
  - `VirtualNet::join_locals` adds a guest with extra locals; each local plays its own scripted
    pad.
  - Per-frame records: `Node::local_pending` (queue lengths) and `Node::predicted_locals`.

## Checks

- New tests, all passing:
  - `netplay::tests::netplay_a_guest_with_two_local_players_plays_in_lockstep`: the plan's
    check. Host with 1 player plus a guest with 2 locals, 20 ± 5 ms link, 3 % loss, prediction
    and drawing on, 600 ticks after seating.
    - 633 hash checks and 0 mismatches.
    - Both guest locals spawn and move (76 and 54 samples, about 170 m each).
    - The guest walk follows each local to its slot's unit.
    - Late seat: local 1's pad goes away at about tick 250 (`RemoveHuman`) and comes back at 400
      (`AddHuman`). The two pending queues are equal on all 999 frames with both seated,
      including the 400+ frames after the late seat (depth up to 5).
    - Both locals predicted (1,334 and 999 frames). Drawing never changed the hash.
  - `netplay::tests::netplay_a_host_local_added_mid_round_keeps_its_queue_beside_local_zero`:
    the host adds its own local mid-round on uneven frames.
    - Waiting, not away, until seated.
    - Equal pending queues on every frame after seating.
    - Both slots in the bundles, and the second local's own pad moves it.
    - Counterfactual: a fresh pacer in place of `follower()` fails at once ("local 1 off local
      0's clock").
  - `netplay::predict::tests::prediction_of_two_local_slots_matches_the_confirmed_steps`.
    - Both slots predicted 4 deep, each local's poses equal to stepping the real sim with both
      pads.
    - Both swapped in at once and out with the hash unchanged.
    - `update` equals `update_locals(.., &[])`.
- Review fixes, new tests:
  - `net` `locals_host_session_submits_a_frame_in_one_command`: two host locals through the
    session's commands, with 60 ms frame hitches. One `SubmitInputsFor` per frame gives 0 parted
    ticks in 1,450. Counterfactual: the old per-slot stream, with the thread's update between slot
    0's command and slot 1's, gave 20 closes in that gap and 12 parted ticks. Without hitches the
    host's buffer hides it (up to 60 torn closes, 0 parted).
  - `netplay::predict::tests::prediction_of_uneven_local_queues_stops_each_at_its_own_inputs`.
  - `walk::locals::tests::a_lan_round_keeps_a_local_respawn_for_the_pad`.
- Filtered runs:
  - `cargo test -p game --lib -- netplay:: walk::locals`: 80 passed, 2 ignored.
  - `cargo test -p net --lib -- session_ inputs_ locals_ away_`: 49 passed.
    `inputs_one_local_datagrams_unchanged` is among them.
- `cargo clippy -p net -p game --lib --tests`: no new warnings in the touched files.
- `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches, before
  and after the review fixes. The `lan_soak` pins did not move (`ticks 1559, hash_checks 4312, static_digest dac14dbdede3d9be`).

## Deviations

- The `net` additions above are needed because the window's threaded sessions have no other path
  for several locals' inputs.
- "RESPAWN in the pad, `pads_with_local_respawns` refused under `netplay.active`": done for the
  further locals. The first player's `request_respawn` path is unchanged, because the LAN window
  already puts RESPAWN in the submitted pad (`lan.rs` 877).
- `is_suspended` keeps its meaning (no unit). `is_waiting` is new, so the plan's split is
  `is_waiting` against `is_suspended && !is_waiting` (away).
- Not done here:
  - The desync report still names one `local_slot` (section 9 r3 says "the slot set"). B19
    carries `(0..local_count()).filter_map(local_slot_of)` into it (on B19's checklist).
  - The walk cannot release an adopted local. When `set_extra_slots` drops one, the view already
    skips that walk local (keyed by slot), but B19 must add `WalkSim::release_local(slot)` and
    call it there (on B19's checklist).
  - The B14 follow-up still counts a several-local machine's `PeerLoaded` digest once per slot.
    Both need the machine's identity on the game side, which B19 (rows grouped by machine) adds.
  - The extra locals' muted bits and `shots_of` are produced but not yet wired to sound: the
    window's LAN branch is B20.
- The late seat in the acceptance test uses the net-level `set_local_away`. The game's lost-pad
  routing under `netplay.active` is B20's, as B17 recorded.

## Left

- B19 lobby, using the host's `add_local` with `Peer::set_extra_slots` and `adopt_local`.
- B20 LAN window: per-local pads into `frame_locals`, `draw_in_local` per cell, split feeds,
  muted bits and `shots_of` to sound, B17's game side.
- B21 polish.
