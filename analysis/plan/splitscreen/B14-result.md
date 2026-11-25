# B14 result: LAN admission for several local players, November 25, 2026

Branch `claude/split-b14` from private main `af4cc0787`. Net crate only (SPLITSCREEN.md 18.2 B14,
section 9).

## What landed

- `PROTOCOL` 11 to 12 (`net/src/packet.rs`, history line added).
- `Msg::Hello.locals: Vec<LocalHello { seat, name, profile }>` for locals 1.. (at most 3,
  `MAX_LOCALS` = 4), in rising seat order (seats 1-3; decode refuses seat 0, a repeat or a
  fall); a one-player `Hello` is its old fields plus one zero count byte. `ClientCore::new` keeps
  at most 3 (debug assert), so a client never sends a `Hello` the host cannot read.
- `Msg::Welcome.extra: Vec<u8>`: the slots of locals 1.., in `Hello` order. `ClientEvent::Welcome`
  carries it; `ClientCore::slots()` and `local_slot(idx)` read it. `ClientState::Connected { slot }`
  is unchanged (local 0's slot).
- Derived tokens: `net::local_token(token, seat)` (seat 0 is the token itself, others a SplitMix64
  mix), stored per slot, so a ban, a reclaim and the reserve all work per slot with no new map.
  Keyed by seat, not by place in the `Hello`, so a rejoin with a different line-up reclaims each
  seat's own slot. A reclaimed slot keeps its old name only while its player still asks for it
  (the same name or its `_N` form); otherwise the new name is made unique.
- `pick_slots(token, count)`: each local's reserved or connected slot for its derived token, the
  rest the lowest free ones; all or none (`Full` when they do not all fit, nothing taken). Every
  local's name is checked (`BadName` for any bad one) and made unique in join order.
- One `PlayerJoined` per slot (the machine's content manifest on each), one `PlayerLeft` per slot
  on a leave, a timeout, a kick or a replace, and one `Trouble` per slot. On a replace, the slots
  the new `Hello` takes back leave as `Replaced`; the ones it releases leave as `Left` (the game
  shows `Replaced` as "reconnected"). A kick of a reserved slot also frees the machine's other
  reserved slots (`Player.machine`, the `Hello` token).
- `Msg::Ready.local`, `ClientCore::ready_local`, `ClientCommand::ReadyLocal`; the host emits
  `PeerReady` for that local's slot (an index with no slot is dropped as a bad message).
  `ready()` keeps meaning local 0.
- Host internals: `Peer.extra`, `Player.local`, `HostCore::machine_slots(slot)` (public, for the
  B19 lobby). The connection's `Loaded`, hash reports, `CaughtUp` and `Resync` speak for all its
  slots (`loaded`, `seen_tick`, `catching_up`; `PeerLoaded` and `PeerCaughtUp` per slot so the
  game's `AddHuman` runs for each). Hash agreement and the fell-behind check count each machine once,
  keyed by local 0's slot (its reports' key), while any of its slots is seated and none is
  catching up, whichever local that is. A machine seated only in part at the load barrier streams
  as seated and gets `PeerCaughtUp` for each unseated slot once it is live (`late_locals`), so
  the game adds them as late joiners. Kick, bench and drop act on the whole machine. A rejoin with fewer locals than
  before releases (reserves) the old link's slots it does not take back.

## Checks

- `cargo test -p net --lib`: 60 passed, 0 failed (34 of them in the filtered first run:
  `locals_`, `codec_`, `session_`). New: `locals_two_local_join_gets_two_slots` (2 slots, unique
  names, Ready per local, both seated, one machine hash agrees the tick),
  `locals_full_when_not_all_fit` (`Full` with 1 free and no slot taken; `BadName` on any local),
  `locals_rejoin_reclaims_the_same_slots_and_a_drop_leaves_both` (timeout drops both with
  `RemoveHuman`, the same token reclaims the same 2 as reconnects with their names, a kick of
  local 1 takes the machine), `locals_rejoin_with_fewer_locals_releases_the_rest`,
  `codec_hello_locals_bounded_and_tokens_derived` (1-local `Hello` round-trips with a zero count, 4
  extra locals or 4 extra slots fail the decode, derived tokens distinct). `codec_messages_round_trip`
  gained 2- and 3-local `Hello`, `Welcome` with extras and `Ready` local 2.
- `cargo clippy -p net --tests`: no warnings in `net`.
- `cargo build --release -p game` (the game compiles unchanged against the new API).
- One-player gate with `lan_soak`: every row matches; the soak pins did not move
  (`ticks 1559, hash_checks 4312, static_digest dac14dbdede3d9be`), so no re-pin and no reason
  line in the gate script.

Review fixes (second commit): seat-keyed tokens and name rule, any-seated-slot machine
representative, `late_locals`, per-slot `Trouble`, `Left` for released slots, reserved-sibling
kick, client-side locals clamp. New tests: `locals_rejoin_follows_seats_and_names`,
`locals_kick_of_reserved_slot_frees_the_machine`,
`locals_partly_seated_machine_counts_and_joins_late`; `codec_hello_locals_bounded_and_tokens_derived`
now also refuses bad seat lists. `cargo test -p net --lib`: 63 passed. `cargo clippy -p net
--tests`: no warnings in `net`. One-player gate after `cargo build --release -p game`: every row
matches, `lan_soak` pins unchanged (`ticks 1559, hash_checks 4312, static_digest
dac14dbdede3d9be`).

## Deviations

- The plan says `Phase::Joined` would hold the slot list; the code keeps `Joined(local 0's slot)`
  and puts the others in `Peer.extra`, so every existing one-slot path (inputs, chat, stats,
  desync) is untouched until B15.
- `PeerLoaded` and `PeerCaughtUp` are emitted per slot (not named in the plan): the game adds each
  mid-round joiner on `PeerCaughtUp`, so a machine's extra slots need one each.
- Extra slots get no input until B15 (`LocalInputs`); in a round they close with the queue's empty
  pad. No game code sends extra locals yet (B18-B20), so nothing in play changes.

## Known follow-ups

- `PeerLoaded` goes out once per slot with the machine's one digest and `hash0`, so the game's
  `check_digest` would count a mismatching machine once per local and queue N copies in
  `loaded_digests`. Latent until extra locals play; B18 dedupes by machine
  (`HostCore::machine_slots`).
- `Trouble` now names every slot of the silent machine; B18/B19 show it per player.

## Left

B15 inputs, B16 host's own locals, B17 away and back, B18 game netplay, B19 lobby, B20 LAN window,
then B21 polish. Not in this packet: the maintainer's November 25 asks (the game's HUD font for
the name tags and the per-cell round timer, and "Paused by <player name>") are now packet B13b in
SPLITSCREEN.md 18.2, with their checks.
