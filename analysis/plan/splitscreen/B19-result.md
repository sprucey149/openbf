# B19 result: the LAN lobby for several local players, November 25, 2026

Branch `claude/split-b19` from private main `3d5d43374`. ui `lobby.rs` and `shell_ps2.rs`, game
`windowed/lobby.rs`, `windowed/lan.rs`, `windowed/front.rs`, `netplay/` and `walk/locals.rs`,
and a small addition to `net` (SPLITSCREEN.md 18.2 B19, section 9).

## What landed

- **Machines in the roster.** `ui::lobby::RosterRow.machine` and `LobbyPlayer.machine` hold
  the slot of the machine's first player. It is 0 for the host's own players, and a guest
  machine's local 0 slot otherwise. `group_rows` puts the host's own players first, then each
  guest machine, with a machine's first player before its further ones. Slots never renumber,
  so the order is stable.
  - A further player is drawn indented as `  + Name`. Ping and addon show only on the
    machine's first row.
  - The machine's Kick button sits on its first row, and is highlighted while the pad is on
    any row of that machine.
- **Kick per machine.** `RosterNav` asks about, and kicks, the machine (by its first slot)
  from any of its rows. The help line names all its players ("Kick Han and Leia?").
  - Rows on the host's side (`machine == 0`) are never kicked. This replaces the old
    `slot != 0` checks.
  - `lobby_kick` refuses `Peer::host_local` and kicks the slot of the machine's first player.
    The net core then removes the whole connection. The line reads "Bo and Leia were removed
    from the game".
- **Ready per machine.** `Lobby.ready` and `LobbyPhase::Results.ready` now hold machines.
  - `PeerReady` from any slot marks `Peer::machine_of(slot)` ready.
  - The host's results check waits for every guest's `machine`.
  - Each row's ready mark is its machine's.
  - The status line counts machines.
  - `net_round_finished` inserts `machine_of(0)`.
- **Game `Peer`.**
  - `Guest.machine` comes from `NetEvent::PlayerJoined.machine`.
  - New: `local_slots()`, `host_local(slot)` (the host's slot 0 and its extra locals) and
    `machine_of(slot)`.
  - `NetEvent::LocalAdded` extends the extra slots and gives `Notice::LocalAdded`.
  - `PeerLoaded` checks the static digest only for a machine's first slot, so a machine with
    several players is counted once (the B14 follow-up).
- **From B18's review.**
  - The desync report carries `local_slots`, the machine's slot set. It is left out of the
    file for a machine of one player.
  - `WalkSim::release_local(slot)` and `Peer::sync_walk_locals(walk)` release the slots the
    peer let go and adopt the ones it gained. A debug assert checks that the walk's local
    slots equal the peer's.
  - The window runs this on the new `NetAction::LocalsChanged`, only while the walk's local 0
    is the peer's.
- **Seats into the session.**
  - The Splitscreen mode list's Multiplayer row stays dimmed behind
    `shell_ps2::SPLIT_LAN_LIVE` (false) until B20 gives each seat its own pad and view in a LAN
    round; B20 flips it. The path behind it is built: entered from there, the Multiplayer
    page's Back returns to the mode list (`Menus::multiplayer_split`).
  - Its Host Game and Join Game keep the Players screen's seats (`App.split_party`).
  - `App::lan_locals` turns each seated seat 1-3 with a Ready or lent profile into a
    `LanLocal` (seat, name, `PlayerProfile`).
  - A guest's `Hello.locals` carries them (`NetplaySession::join_locals`). Their slots are
    recorded from `Welcome.extra`.
  - The host sends `HostCommand::AddLocal` for each, as the session opens and before any
    input (`add_host_locals`). It keeps the slot and unique name from `LocalAdded`. If one is
    refused, the lobby says "<name> could not join this game" (the event does not carry
    whether the cause was no free slot, a round loading or a bad name). The unique name is
    copied into the local's profile too; a guest's locals take theirs from the roster
    (`learn_names`).
  - `seat_humans` seats the host's locals with their own profiles.
  - The roster lists them with `machine 0`.
  - The discovery player count includes them.
- **net (not in the plan's file list).**
  - `NetEvent::PlayerJoined.machine`.
  - `HostCommand::AddLocal { seat, name, profile }`, which calls
    `HostCore::add_local_event` and is answered by `NetEvent::LocalAdded { seat, added }`.
  - These are in-process only. There is no wire change, and PROTOCOL stays 12.
- **Fonts.** The roster's ready mark (U+2713) showed as a missing-glyph box in the menu face. It
  now reads "Name · Ready". The indent uses `+`, not a box-drawing glyph.

## Checks

- New tests, all passing:
  - `ui lobby::tests::roster_rows_group_by_machine_and_kick_a_whole_machine` (the plan's check).
    Grouped order: the host's own players first (including a host local in slot 3 that joined
    after the guests), then each machine with its first player first. The host's own player is
    never kicked. From Leia's row the question names "Han and Leia", and the second Accept
    returns machine 2. A machine that leaves while asked does not pass the question on.
  - `game windowed::lobby::tests::a_two_player_guest_machine_groups_readies_and_is_kicked_whole`,
    headless over UDP. Host Ann plus host local Rex; guest machine Bo plus Leia.
    - The host's roster and the guest's copy are grouped the same way.
    - The round's seats are Ann, Rex, Bo, Leia, and Rex keeps his own profile.
    - The guest machine's one Ready ends the results.
    - A kick of Rex does nothing.
    - From Leia's row the pad kicks machine Bo. The guest's session ends, and both of its
      players leave the host's roster.
  - `game netplay::tests::netplay_host_locals_are_the_hosts_machine_and_the_walk_follows_the_peer`.
    - `AddLocal` gives slot 1 "Host_1" and slot 2 "Leia". Both are `host_local` on machine 0.
    - The walk adopts both at the build.
    - `remove_local(1)` followed by `sync_walk_locals` releases slot 1 only.
- Updated: `shell_ps2::tests::the_players_screen_leads_to_the_split_mode_list` (Multiplayer is
  `SPLIT_LAN_LIVE`; the test forces it live and checks Back from it returns to the list). The roster fixtures now carry `machine` and the
  name Rex.
- Filtered runs:
  - `cargo test -p ui --lib -- roster_ the_players_screen multiplayer`: 7 passed.
    `-- lobby::`: 9 passed.
  - `cargo test -p net --lib -- locals_ session_ codec_`: 45 passed.
  - `cargo test -p game --lib -- netplay:: walk::locals`: 82 passed, 2 ignored. This includes
    B18's two-local lockstep test with the per-machine digest check.
  - `cargo test -p game --bin game -- windowed::lobby`: 7 passed, 1 ignored.
    `-- windowed::lan`: 3 passed.
- `cargo test -p game --test typography_captures -- --ignored` at 1280x720 with the
  `egui_roster` fixture extended to a host local and a guest machine of two. I looked at the PNG:
  the rows are grouped and indented, the Kick button sits on the machine's first row, and no
  glyph is missing.
- `cargo clippy -p net -p ui -p game --tests`: no new warnings in the touched files.
  `cargo fmt --check`: clean.
- `cargo build --release -p game`, then `scripts/one_player_gate.py`: every row matches. The
  `lan_soak` pins did not move (`ticks 1559, hash_checks 4312, static_digest dac14dbdede3d9be`).

## Deviations

- The plan names only ui `lobby.rs` and game `windowed/lobby.rs`. The host's threaded
  `NetSession` cannot return `add_local`'s slot and name, and the game had no machine identity.
  Both needed the in-process `net` additions above, plus `Peer` and `lan.rs` changes.
- The plan's "`ready.insert(0)` becomes a `host_local` check" is done by keying ready on the
  machine. The host's machine is 0, and one Ready answers for each guest machine.
- Host locals are added only as a session opens. A pad joining the host mid-session is not
  handled, and the Players screen still fixes the seats before Multiplayer.
- No split captures were taken: nothing in the battle view changed. The visible change is the
  lobby roster, which was checked through the typography capture.

## Left

- B20 LAN window: per-local pads into `frame_locals`, split views in a LAN round, and the
  lent-profile hand-off (`App.split_profiles`) for LAN locals. Also each local's own Ready
  profile at round end (`ClientCommand::ReadyLocal`), so a guest's further players keep their
  medals, and B17's game side. Until B20, a LAN round seats the extra locals and they stand
  idle (neutral pads), which is why the Multiplayer row stays dimmed until then.
- B20 also: a `HostCommand::RemoveLocal` answered by an event that shrinks `Peer.extra` and
  pushes `NetAction::LocalsChanged` (a host pad drop or seat leave mid-session). Until then
  the release side of `Peer::sync_walk_locals` runs only in its unit test. A refused seat
  should show its cell a notice rather than an idle view.
- Review fixes after landing: the row gate above, neutral refusal wording, unique names into
  profiles and guests' locals, `start_at`'s doc comment restored, the kick line's was/were
  counted from rows, and the unused `HostCommand::RemoveLocal` dropped.
- B21 polish items already landed; the maintainer's LAN run after B20.
