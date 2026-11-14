# LAN multiplayer design (issue #168)

Status: accepted design, November 13, 2026. The maintainer started M7 (PLAN.md).
This file settles the choice PLAN left open between lockstep and client-server.
`net/src/lib.rs` is the implementation. Each module's `//!` header repeats the
rules it owns.

## 1. Decision: host-ordered deterministic lockstep

Every machine, the host included, runs the same `Sim` from the same per-tick
input bundles. The host orders the inputs and never waits for a slow client.
Nothing else is sent: bots, bolts, vehicles and posts cost no bandwidth.

Why this fits openbf:

- `core` is already deterministic and cross-OS by contract: `core::fmath` on
  `libm`, parry `enhanced-determinism`, seeded RNGs, fixed 30 Hz ticks,
  `Sim::step(&[PadState])` indexed by player slot and a whole-state
  `Sim::hash`. `PadState::pick` was already documented for "a lockstep peer".
- The renderer, animator, audio, HUD, black box and event sounds all read the
  `Sim` directly. With snapshot replication every one of them would need a
  replicated proxy world. With lockstep they are unchanged; a peer only sets
  which slot is "me".
- Battlefront is bot-heavy. Lockstep sends about 25 bytes per human per tick,
  and bots cost nothing. Snapshots would cost 30-60 KB/s per client.
- Measured step cost is 1-3 ms p99 on all 34 maps at 16-32 bots per side
  (#110 card), against a 33 ms budget. Every machine already runs this sim in
  single player, so a human replacing a bot costs nothing extra.

Rejected for now:

- **Full rollback (GGPO-style).** Steps spike to about 10 ms, and with 8-16
  humans someone changes input almost every tick. Rollback would multiply sim
  CPU on every machine and needs presentation de-duplication.
- **Snapshot client-server.** It needs a serializer for about 60 types and a
  proxy presentation layer. The original game did this and its community
  patches spent years fixing prediction drift.

Upgrade path: make `Sim` `Clone` (about 8 mechanical derives, see #168 card)
and add local-player movement prediction over a cloned sim. Rollback becomes
possible once static map data moves behind an `Arc`.

## 2. Topology and threads

- **Star topology.** Clients talk only to the host. Peers never connect to each
  other, so a VPN only needs each guest to reach the host.
- **One network thread per session** owns the UDP socket, the clock and all
  protocol state. The game thread talks to it through channels. A frame hitch,
  a blocking load or an unfocused window never stops acknowledgements,
  heartbeats or the host's tick clock.
- **The host's network thread is the tick server.** It closes tick bundles at
  30 Hz on its own clock. The host's game consumes them like a client with zero
  latency. This also makes a later dedicated relay a small change.
- The host's game does not get special treatment from the scheduler. Its pads go
  through the same per-slot queue as a remote client's.

## 3. Transport

- Plain `std::net::UdpSocket`, non-blocking. No async runtime and no new
  networking crate.
- Every datagram is at most **1200 bytes**, so it fits Tailscale's 1280 MTU,
  WireGuard and PPPoE.
- Packet header:
  - magic `OBF1`
  - protocol version (u16)
  - connection id (u32, random per connection)
  - sequence (u16)
  - ack (u16) plus a 32-bit ack bitfield
- Unknown magic, wrong version, an unknown connection id or a malformed body is
  dropped silently and counted. Every length and enum is range-checked before
  use.
- **Unreliable redundant streams** carry the hot path:
  - Inputs: every packet carries all unacknowledged inputs, delta-coded.
  - Tick bundles: every packet carries all unacknowledged bundles.
  - There are no resend timers. Loss is covered by the next packet.
- **A reliable ordered channel** carries everything else (handshake, lobby
  state, round setup, catch-up logs, commands from the host game, chat). It
  fragments into the 1200-byte limit and resends after RTT + 4·RTTVAR (minimum
  50 ms). Messages are acknowledged through the packet acks.
- **RTT** uses EWMA mean (1/8) and deviation (1/4) from acked send times.
- The port is UDP **37450** by default and configurable. Hosts bind
  `0.0.0.0:37450`, and clients bind an ephemeral port.
- **Discovery:**
  - Clients send `Find` to the limited broadcast and to the /24 directed
    broadcast of the interface the default route leaves by (the standard
    library lists no interfaces, and no new crate is taken for it; a wrong
    mask only reaches nobody). Hosts answer `Found` with name, map,
    players, max players, protocol version and build id (`HostInfo`); Join
    Game lists a host on another build dimmed with why.
  - **Join by address** (IP or host name, including Tailscale MagicDNS) is the
    primary path, because Tailscale and WireGuard carry no broadcast.
  - Recent hosts are remembered (the profile's last 8, newest first).
- **Timeouts:**
  - A connection is "having trouble" after 1.5 s of silence; the HUD meter
    shows it.
  - It is dropped after 12 s, or after 60 s during a load barrier.
  - Peers send a heartbeat at least every 100 ms.
- **The test transport** (`LinkSim`) is in-memory with seeded latency, jitter,
  loss, duplication and reordering. A `Clock` trait lets protocol tests run in
  virtual time.

## 4. Session protocol

### 4.1 Join handshake

The client sends `Hello` with:

- `protocol` (u16)
- `build` (the sim build identity, §7)
- player name
- session token (random u64, reused on reconnect)
- a content manifest: playable map keys, games present, and whether the Kit
  Fisto and Asajj Ventress addon is installed
- input kind (pad or mouse; diagnostic only)
- profile data that feeds the sim: medal counts, unlock state, aim-assist
  settings, look sensitivity

The host replies with `Welcome` (slot, current lobby state) or `Reject`. The
reject reasons are:

- `Version` (protocol or build differ; the message says which)
- `Full`
- `Busy` (round loading)
- `Banned` (kicked this session)
- `BadName`

Each is shown as plain text (`netplay::lobby::reject_text`); `Version`
names both builds: "The host runs a different openbf build (host: X, you:
Y)". The player's name is Options → Game Options → Player name (the
profile's name; 1-24 characters, not only spaces).

### 4.2 Lobby

The host owns `LobbyState`, the shared setup the guests see:

- the screen and cursor the host is on
- the Instant Action playlist (by **map key**, never a local index), modes and
  era
- session options
- the cheat policy
- the roster: name, slot, team preference, ping, ready, and whether each player
  has the addon

It is versioned and sent as full state over the reliable channel whenever it
changes, throttled to 10 Hz. Guests draw the host's setup screens read-only
from it, marked "The host is choosing…".

The host's map and option lists are the **intersection** of every connected
player's content:

- A map is offered only when every player has its key. The map's files are
  not compared here: a baked `manifest.ron` is mostly presentation data that
  differs between PS2, PC and Xbox copies (and, before the bake wrote LF
  everywhere, between Windows and other bakes). Whether copies simulate the
  same is the load barrier's static digest (§4.3).
- The addon heroes option is forced off and greyed unless everyone has the
  addon.
- If a player who lacks a playlisted map or the addon joins, that entry is
  removed or the option is switched off, with a lobby message.

How it is played (`game/src/main/windowed/lobby.rs`):

- Main → Multiplayer: LAN Host Game, LAN Join Game, and Online dimmed with
  "Online play needs a dedicated server — not available yet".
- Host Game: max players 2-16 (default 8), the UDP port, the guests'
  cheat policy (Off, or an allowed list with a row per cheat) and lag
  compensation on or off; then the host's own Instant Action screen is the
  lobby, with a roster panel (name, ping, addon, team, ready) and Kick.
  Every human starts at the side select ("Auto"); a per-player team
  preference is not offered yet.
- Join Game: the hosts found on the LAN, Join by address (IP or host name,
  optional `:port`, resolved off the frame loop) and the recent hosts.
- Guests draw the host's screen from `LobbyState` with "The host is
  choosing…"; their only input is Back (leave). The map list is the host's
  by key, built from the guest's own catalog in the host's order, so every
  index and cursor lines up.

### 4.3 Round start and the load barrier

1. The host sends `LoadRound` with:
   - `round_id`
   - the full `BattleSetup` inputs: map, mode, armies, era, seed, difficulty,
     every session option, cheat policy and starting cheats
   - the human roster, each human's slot, team and profile data
   - the playlist position
2. Each peer loads, builds the Sim and replies `Loaded`. The reply carries:
   - `round_id`
   - the **static gameplay digest** of the built sim: a hash over class
     tables, collision, nav and map-wide constants, everything the sim reads
     that `Sim::hash` does not cover
   - `Sim::hash` at tick 0
3. A peer whose digests differ from the host's is moved to the lobby with
   "Your copy of <map> plays differently from the host's (<group>)". The map
   stays playable for everyone else.
   - The host benches it (`HostCore::bench`: a bot takes its slot, no more
     bundles go to it, the connection stays, the next round seats it) and
     sends its static digest groups; the guest names the first group whose
     digest differs from its own build's.
   - A guest's `Loaded` that comes before the host's own build is compared
     when the host's build is done.
   - A mid-round joiner without the map in play is benched the same way
     ("You do not have <map>; you join the next round").
4. The barrier times out at 60 s. Late peers become mid-round joiners.
5. `Start` carries the host's tick-0 time, and bundles start flowing.

### 4.4 Ticks, inputs and commands

- Bundle `T` holds, for every active slot, the `PadState` used by step `T`. It
  also holds an optional list of **session commands**, applied in order before
  step `T` on every peer.
- Commands are the only way anything outside the pads changes the sim during a
  round:
  - `AddHuman { slot, team, profile }`
  - `RemoveHuman { slot }`: a bot takes over at the human's position. This
    reuses `Sim::vacate_player_slot` + `spawn_ai`.
  - `SetSlotCheats { slot, cheats }`
  - `SetCheatPolicy { allowed }`
  - `EndRound`
  - room for later commands
- The pause-menu settings that changed the sim in single player (AI cap,
  reinforcements, assist, career awards) are host-only commands in multiplayer.
  They are not offered to guests.
- **Inputs are never dropped.**
  - A client sends one input per input tick with a sequence number. The host
    keeps a short FIFO per slot.
  - When the host closes tick `T` and a slot's queue is empty, it repeats that
    slot's last input. Mouse deltas are zeroed, once-bits cleared, and
    `pick` and `fire_lag` cleared.
  - A late input slips to the next open tick.
  - If a queue grows beyond its target plus 3, the two oldest inputs are merged
    (mouse deltas summed, once-bits OR'd, latest levels kept).
  - A FIFO keeps any standing queue it is given, and each standing input is a
    tick of delay on every input after it. So a close right after a starved
    one that finds two or more inputs (the late one and the on-time one)
    merges them: one late input never leaves a tick of delay behind.
- **Client pacing** works by time dilation, at most ±4% and never by jumps:
  - Each host status tells the client its queue depth, its starvation count
    and its **slack**: how early the latest of its inputs arrived before its
    close over the last 15 closes (negative when one was late).
  - The client's input clock rate follows the slack proportionally,
    `-(slack - 2 ms) / 2 s`, so its latest input lands just before the
    close. An occasional starved tick is accepted (it repeats the last
    input) rather than a deeper queue.
  - Measured in `--net-soak` (November 13, 2026, `bf2_kas2`, 6 peers): a
    guest's confirmed input latency is RTT + 29 ms at 15 ± 3 ms one way and
    1% loss, RTT + 48 ms at 40 ± 10 ms and 3% (`bf1_tat2`, 4 peers), RTT +
    58 ms at 60 ± 15 ms and 5%; most of what is past one tick is the
    uniform jitter itself. The host's own: 21 ms.
- **Client playback** keeps a jitter buffer of received bundles. Its target is
  the measured arrival jitter in ticks: 0 on a clean LAN, more on a jittery
  VPN. It is raised quickly and lowered slowly, because steady delay feels
  better than delay that jumps around. The buffer's level (bundles waiting
  minus the time owed toward the next step, smoothed over 0.5 s) steers a
  ±5% dilation so a bundle waits about the target.
  - More than 6 ticks behind: it catches up at up to 4 steps per frame.
  - Starved: it holds, and the render blend keeps the last tick drawn.
- **The host's game** plays its own bundles as they close (zero network delay).
- **Pending inputs.** The host status also carries the last closed tick and
  the newest of the client's input sequences it held. A client counts its
  inputs sent and not yet in a stepped tick by those reports (between
  reports, one input a tick), so the inputs in flight when its slot joined
  count too. The look preview and the prediction (§6) run over them.

### 4.4b No pausing during a round (maintainer, November 13, 2026)

- Nobody can pause a multiplayer round, **the host included**. The battle only
  stops between matches.
- The pause button opens the pause menu as an overlay:
  - That player's own soldier gets a neutral pad and stands where he is,
    vulnerable.
  - The battle keeps running for everyone, including on the player's own
    screen.
- Menu actions that change the battle become commands (host) or requests
  (guests), applied on a future tick.
- Nothing local may stop or slow the shared clock. The `slow` cheat word, which
  changes time, is unavailable in multiplayer.

### 4.5 Joining mid-round and reconnecting

1. The host sends `LoadRound` plus the round's **bundle log so far** (zstd,
   reliable channel). New bundles keep streaming.
2. The joiner builds the round from setup and fast-forwards through the log on
   its game thread, at up to 4,000 steps per frame batch, with a progress bar.
   - A 10-minute round is 18,000 ticks, about 10-20 s.
   - There is no state serialization. Joining reuses the same deterministic
     path as replays.
3. When the joiner is within 2 ticks of live, it reports `CaughtUp`.
4. The host then schedules `AddHuman` for that slot. The joiner enters the side,
   post and class screens, and the battle keeps its population by shedding a
   bot.
5. **Reconnect** is the same path. The session token keeps the player's slot
   and name for 90 s.

### 4.6 Leaving

- A `Leave` message or a 12 s timeout schedules `RemoveHuman`.
- A bot replaces the player where they stood, so the team keeps its strength.
- If the **host** leaves, the round ends for everyone with the scoreboard and a
  "The host left the game" message. Host migration is not planned.

### 4.7 Desync detection

- Every 30 ticks each client sends `(tick, Sim::hash)`. The host compares it
  with its own.
- On a mismatch:
  - Both sides write a black box desync record with the SHASH group hashes at
    that tick and the last 300 bundles.
  - The client is told and **rejoins automatically** through §4.5.
  - A second desync within 5 minutes moves that player to the lobby with a
    message naming the first differing group.
- The host is the reference and never desyncs.
- Desync reports (done November 13, 2026; `game::netplay::desync`):
  - While a round runs, each peer keeps every stepped tick's row (its
    `Sim::hash` and the five SHASH groups units, rng, score, posts and timers
    over exact bits) for the last 512 ticks, and the round's bundle log from
    tick 0. A row costs about 60 µs a tick (bf1_tat2, release).
  - `Desync` carries the host's hash, build and OS/CPU. The guest answers
    with its rows for the 60 ticks up to the mismatch (`DesyncHashes`,
    protocol 3), writes its report and rejoins. The host writes its report
    with both sides' rows (or without the guest's after 5 s).
  - Files: `<black box folder>/desync/desync-<map>-<seed>-r<round>-s<slot>-t<tick>-<host|guest>.json`
    with both hashes, rows, builds and OS/CPU, the static digest by group,
    the round setup and the bundle log.
  - `game --desync-report HOST.json [GUEST.json]` prints the first tick and
    group where the rows differ, and re-simulates the round from the setup
    and bundles to say which side this build agrees with.

### 4.8 Round end and the playlist

The flow follows FEATURE_SET "Post-match results":

- Every peer sees the same team board, then its own personal and career pages.
- A client that has confirmed its last page shows "Waiting for other players"
  and sends `Ready`.
- The host continues the playlist when every connected player is ready, or
  after 60 s.
- A player who disconnects leaves the ready check.
- As built: the host closes the round's bundles (`EndRound`) as soon as
  its round is decided, and the 60 s count from then; the host continues
  only once it has left its own pages too. A guest waiting in the lobby
  (benched, or joined between rounds) is ready at once. After the
  playlist's last row everyone returns to the host's setup screen.

## 5. Cheats

- In multiplayer, cheats are **per player slot**. In single player, slot 0 is
  the old global set, so single-player behaviour and hashes do not change.
- The host's lobby sets the **cheat policy**: Off, or a list of allowed cheats.
- A guest's pause-menu Cheats page and Morse codes **request** a change:
  - The host's network thread applies it only when the cheat is allowed.
  - It is applied as a `SetSlotCheats` command on a future tick.
- The host can switch any player's cheats, or everyone's, during a round.
- Turning a cheat's permission off clears it for everyone on the next tick.
- Game-only presentation words (`hud`, `80s`, `at`, `alias`) stay local when
  they do not change the sim. `slow` changes time for everyone, so it is
  unavailable in multiplayer (§4.4b).

## 5b. Awards, names, teams and the scoreboard (maintainer, November 13, 2026)

**Award policy.** The host chooses whether awards are allowed, the same way as
cheats.

- With awards off, nobody's earned awards apply: no award weapons and no medal
  effects.
- With awards on, each player's own earned awards apply.
- The host can switch the policy during a round with a `GameCommand`, applied
  on a future tick.
- Difficulty tuning always assumes awards off (they are a bonus on top).

**Player names.**

- A player's name is their profile name.
- Profile selection at launch is separate work for the profile feature. Until
  then the name is a setting.
- In a session, the host makes names unique in join order: a second "the maintainer"
  becomes "playtest_1", a third "playtest_2".
- On a single machine, two local profiles can never have the same name; that
  is the profile feature's rule.

**Names over heads.**

- In multiplayer, and later splitscreen, a human player's name is drawn above
  their head.
- It shows only with direct line of sight: the same visibility ray the game
  uses for other sight checks, never through walls or terrain. It also fades
  with distance.
- Bots show no name.

**Teams.** As now, every player picks a side on the side screen at the start
of a round. There is nothing more for now.

**In-battle scoreboard.**

- It is shown while **Select / View** is held on a controller, or **Tab** on a
  keyboard. It does not show while Select is part of the vehicle seat-switch
  chord.
- It lists both teams' players and bots with their points, kills and deaths,
  plus a ping column for humans in multiplayer.
- It reads the same team-stats data as the round-end Team Stats page.

**Chat.** There is no in-game chat. Players use their own voice chat
(Discord).

## 6. Latency hiding: local prediction from the first release

The maintainer chose (November 13, 2026) to ship local prediction in the
first testable version rather than wait. Prediction is presentation only:
the confirmed lockstep sim stays the single source of truth, and nothing
predicted ever feeds back into it.

- **Look.** The camera turns by the confirmed aim plus every pending input's
  look (mouse deltas summed, stick turns per tick) through `Sim::look_preview`.
  When the inputs land the sim aim equals what the player saw.
- **Own movement ("latency state").** Each peer keeps a *predicted* copy of the
  sim:
  - Every time a confirmed tick lands, the peer clones the confirmed sim,
    which is cheap because static map data is shared behind an `Arc`.
  - It then steps the copy forward over the local player's pending inputs.
    Other humans repeat their last input, and bots repeat their last pad with
    AI thinking skipped (`Sim::step_predicted`).
  - The local soldier or vehicle, the camera, the reticle and the local
    player's own muzzle flash and fire sound come from the predicted copy.
  - Everything else (other players, bots, bolts, damage, posts, the HUD
    numbers and all other sounds) comes from the confirmed sim, so it is
    never wrong and never plays twice.
  - Prediction usually agrees with the confirmed result. When it does not
    (the player was knocked back, blocked or killed), the drawn pose eases to
    the corrected one over 0.25 s instead of snapping.
- **Shots** are decided by the confirmed sim, so every machine agrees on hits.
  The local player's own shot feedback (flash, sound, recoil) plays at once
  from the prediction and is de-duplicated against the confirmed fire event
  of the same unit, weapon and shot count.
- **Lag compensation: what you saw is what you hit.** Without it, a shooter
  would have to lead moving targets by their own delay.
  - Each input carries the confirmed tick its player was looking at. The host
    stamps `view_lag = T - view_tick` (capped at 8 ticks; always on and not a player-facing option, by the
    maintainer's choice on November 13, 2026; `--lan-host --no-lag-comp` turns it
    off for testing only) into the
    pad in the bundle, so it is deterministic.
  - The sim keeps a short ring of every unit's and vehicle's past hit volumes.
    It is derived from hashed past state and identical on every peer, so it is
    not hashed itself.
  - A shot whose pad has `view_lag > 0` sweeps its bolt's first
    `view_lag × speed × tick` metres against the rewound volumes. The bolt then
    continues from the advanced point at the present tick.
  - Every flown round gets the same (November 13, 2026, `core::sim::lag_comp`):
    straight bolts and rockets; shells, which fall and meet the world along
    each rewound tick's chord, and whose blast goes off at the rewound
    impact; unguided missiles; fat rays, which pass through each unit's
    rewound point; and tracers, which move on with their round. Beams sweep
    their ray against the volumes `view_lag` ticks back.
  - A homing missile keeps its lock path. It steers at its target's present
    point every tick, so its shooter never leads it, and the lock was earned
    against the moving target over the dwell time. Rewinding it would aim it
    at where the target was. Player-steered guided rockets also keep theirs.
  - Single player and `view_lag = 0` take exactly the old path, so no pin
    moves.
  - The cost: a victim can occasionally be hit just after stepping behind
    cover, by up to the shooter's delay. Most shooters accept this trade, and
    the cap bounds it.
- **Budget guard.**
  - The prediction depth is the number of pending inputs: about RTT/33 ms + 1
    ticks.
  - It is capped at 8 ticks (about 250 ms) and at a CPU budget of 6 ms per
    frame.
  - Past either cap, the peer predicts fewer ticks; past the hard limit it falls
    back to plain delay. The meter shows which mode is active.
  - On a LAN the depth is 1-2 ticks, so prediction costs about one extra step
    per tick.
  - Measured (November 13, 2026, `--net-soak --predict`, `bf2_kas2`, 6
    peers, 33 bots a side): at 120 ms RTT guests predict 5-6 ticks (p95
    6-8), 0.7-0.9 ms a frame on average, p95 about 2 ms, worst 4.4 ms; at
    30 ms RTT 2 ticks and 0.3-0.4 ms. Every hash still matched.
- **How it is drawn** (`game::netplay::predict`, `game::netplay::view`):
  - The predicted local pose is the last two predicted ticks blended at the
    input clock's phase (the predicted ticks advance with the inputs sent).
    It replaces the confirmed one only while both are the same life in the
    same seat; a boarding or a death shows from the confirmed sim.
  - Own shots are numbered per unit and weapon class. A predicted shot whose
    number was not fed back yet plays its sound, muzzle flash and
    first-person fire at once; the confirmed shot with that number is muted.
- **Expected feel.**

  | Connection | RTT | Look and own movement | Hits and others' actions |
  |---|---|---|---|
  | LAN | about 1 ms | immediate | 1-2 ticks |
  | Same-region VPN | 20-40 ms | immediate | about 70-110 ms |
  | Cross-continent | 150 ms | immediate | about 220 ms |

  Hits are judged against what the shooter saw (lag compensation above). Only
  the hit marker and damage numbers appear one round trip later.

  The meter shows all of it.

## 7. Compatibility: any OS, any CPU, any disc

**One math path everywhere.** Linux, Windows and macOS players, on Intel/AMD
or Apple Silicon/ARM, must simulate bit-for-bit the same.

- `core::fmath` already routes transcendentals through the pure-Rust `libm`,
  and parry is forced onto it.
- The remaining hazard is glam's vector code. Its SSE2 and NEON paths sum
  dot products in different orders. The sim therefore uses glam's
  **`scalar-math`** path on every target, so every CPU executes the same
  IEEE operations in the same order. If that moves a pinned hash, the re-pin is
  a maintainer decision.
  - Done 2026-11-13 (approved with its re-pin): `engine/Cargo.toml` builds glam
    with `scalar-math`, and `core::fmath` refuses to compile without it. New
    pins: `--sim-hash` 100 `71dabcfadf689bff`, 600 `abcec91efdcf4c93`; the
    all-proxy skirmish `7efc85cee4eb5f2a` / `8350aa51d0876ce8`; every round
    golden re-blessed. XDET then agrees on Linux, Windows and aarch64.
- `f32::max` and `f32::min` (and `f64`'s) are the third hazard. LLVM
  lowers them per CPU, and the CPUs disagree on a tie between `+0.0` and
  `-0.0` and on which NaN comes back: x86's `maxss` gives the second
  operand, AArch64's `fmaxnm` puts `-0.0` below `+0.0`. A weapon's spread
  decays with `(spread - dt * rate).max(floor)` and came out `-0.0` on x86
  and `+0.0` on aarch64, so real maps' state hashes parted within seconds
  (ticks 452 to 717) while the play looked the same.
  - Done 2026-11-13, no re-pin: `core::fmath::max`/`min` and
    `FloatExt::fmax`/`fmin` give x86's exact answer as plain comparisons
    (measured against `f32::max`/`min` in debug and release over every pair
    of signed zeros, infinities, subnormals and NaN payloads). Every float
    max and min in `core` and `game` uses them; clippy's `disallowed-methods`
    (`core/clippy.toml`, `game/clippy.toml`) refuses `f32`/`f64` `max`/`min`
    outside test code. `f32::clamp` and glam's scalar vector
    `min`/`max`/`clamp` are comparisons already and stay.
  - Hashes and the static digest feed any NaN as `f32::NAN`'s bits
    (`FloatExt::hash_bits`): a NaN made at run time is `0xffc00000` on x86
    and `0x7fc00000` on AArch64.
  - Evidence: no x86 pin, round golden or real-map trace moved. `game
    --sim-trace 3600 --state-hashes` agrees per tick on x86 Linux, aarch64
    (qemu) and Windows (Wine) on bf1_tat2, bf2_kas2, bf2_spa1 and bf2_myg1,
    and on x86 and aarch64 on every other baked map. XDET passes with the
    new known answers.
- No FMA contraction, no `target-cpu=native` and no fast-math are allowed in
  release builds.
- **XDET**, the cross-platform determinism check, runs the core's determinism
  battery on three targets and compares hash streams:
  - x86_64 Linux natively
  - x86_64 Windows under Wine (Proton)
  - aarch64 Linux under qemu, which stands in for Apple Silicon: the risk is
    the CPU, not the OS
- A real Mac build must be compiled on a Mac, because linking needs Apple's
  SDK. `cargo check --target aarch64-apple-darwin` catches compile errors here.

**Build identity.**

- `build` = the source commit plus a hash of uncommitted changes. Peers must
  match exactly, because different sources desync.
- The same commit gives the same `build` on every OS: untracked OS and editor
  litter (`.DS_Store`, `Thumbs.db`, swap files) is not a change, line endings
  are read as LF, and `.gitattributes` keeps the sources LF on every checkout
  so `core.autocrlf` cannot make a clean tree look dirty.
- The OS and CPU are reported for diagnostics. They are not a reason to refuse.

**Content identity.**

- At join, only the map keys are compared: everyone must have the map.
- At load, the static digest of the built `Sim` is checked (§4.3). That makes
  mixed PS2, PC and Xbox copies acceptable exactly when they simulate the same.
- The per-tick hash check (§4.7) is the backstop.

**Gameplay environment switches** (`OPENBF_DOORS_SOLID`, `OPENBF_PLNP_MISSION`,
`OPENBF_AUTOPILOT`, …) are ignored in multiplayer, or sent by the host as part
of the setup.

## 8. Player cap

The default lobby maximum is **16 humans**. The protocol and slot tables support
32. Bots fill each side to the map's normal size, and a joining human takes a
bot's place.

What limits it:

- Not bandwidth: about 8 KB/s down per client and about 150 KB/s up for the host
  at 16 players.
- Not the host's CPU: every machine runs the same sim, and humans replace bots.
- The real limits:
  - The **slowest machine** must run the sim at 30 Hz plus render.
  - The **worst link** sets only that player's own delay, not everyone's.
  - **Join catch-up** time grows with round length.

## 8b. Slow machines

A slow PC never slows anyone else down: the host never waits for anyone. The
responses, in order of preference:

1. **Sim first.** Each frame runs the due ticks before drawing. Rendering may
   drop frames, but the sim keeps 30 Hz. A tick costs 1-3 ms, so this holds even
   on weak CPUs.
   - The windowed loop polls the session, steps the due bundles
     (`LocalView::frame`, then the tick hook for each), and only then poses
     and draws the frame (`main/windowed/events.rs`).
   - The step gets the frame's whole wall time, up to 1 s. Other per-frame
     work keeps its 0.1 s cap. So a late frame steps every bundle that came due
     while it was late before anything draws.
   - The input clock is the one exception. It advances at most
     `peer::MAX_INPUT_FRAME_S` (0.1 s) per frame. Otherwise a stall's time
     would become a burst of repeated inputs, standing in the host's queue as
     delay on every later tick. The host already repeated the inputs it
     missed.
   - A guest catching up steps its frame's own whole ticks plus 4
     (`net::pacer::Playback`), so a slow frame never falls further behind.
   - A frame never skips its drawing. Its sim work already comes first, and a
     catch-up does not draw between its steps, so a skipped draw would save
     one frame's draw time and leave the screen frozen.
     measure: the per-frame draw cost on the weakest playtest PC before
     revisiting.
2. **Automatic quality (the "fuzzy map" fallback).** A governor watches frame
   times.
   - When the 95th-percentile frame time stays over budget for 3 s, it first
     lowers the render resolution scale, then steps down the graphics tier
     (shadows, draw distance, LOD bias, particle and foliage density).
   - After 10 s of headroom it steps back up, one step at a time with
     hysteresis, so quality never flickers.
   - It is on by default (Options → Graphics → Automatic quality). Off keeps
     the player's choice: Low power resolution's own controller, or full
     size. While it is on, Low power resolution is greyed. It changes nothing
     the sim reads, so it is safe in multiplayer. It plugs into the graphics
     tiers (#93/#128) as they land.
   - Built November 13, 2026 (`game::quality_governor`):
     - Over budget means the p95 is above 1.2 × budget. The budget is the
       display's period, at most 60 frames a second.
     - Headroom means the p95 is at most 1.1 × budget. Between the two,
       nothing changes.
     - It steps down once per full 3 s window of new frames.
     - The scales are 100, 85, 70, 60 and 50%. The 3D world draws into a
       smaller target; the HUD and menus stay at the window's size.
     - Below the scales come the `TierHook` steps. There are none until the
       tiers land.
     - A step up is a trial. A trial that goes over within 6 s steps back
       down and waits 20 s before the next try. Each further failure in a
       row doubles the wait, up to 160 s.
     - A CPU-bound frame steps nothing. That is a frame whose own work is
       over budget, measured without its wait for the window's image. A
       smaller picture would only blur it, as on a host whose sim and bots
       fill the frame. The note then says "CPU-bound".
       - The rule applies once the platform has shown that it waits in the
         acquire. Where it never does, every slow frame counts as the GPU's.
     - The first 4 s of a battle are ignored, and so are a joiner's replay
       frames.
     - The fps counter and the net meter show the scale ("res 85%").
3. **Cheaper sims for everyone.**
   - The AI think pass is read-only on the sim with a later write-back. It runs
     on worker threads in fixed partitions, so the result is identical for any
     thread count.
   - Loading builds the round on worker threads (§10.5).
     - The loading worker parses the scene, then builds the setup and the walk
       (`main/windowed/round_prep.rs`).
     - Beside the walk it loads the sound library. It then decodes the
       battle's takes on up to 4 threads, alongside the CPU-only resources:
       foley, engine sounds, ambience, the feed, the effects library and the
       weather.
     - The event thread keeps the GPU uploads and the single-player installs.
       It keeps animating the loading screen meanwhile.
     - The tick-0 `Sim::hash` equals the serial build's.
4. **Last resort.** A player whose machine falls more than 10 s behind live
   (the sim itself cannot keep 30 Hz) is moved back to the lobby with
   "Your computer could not keep up with this battle". A bot takes their place.
   The host can change the 10 s threshold or turn the drop off
   (`HostConfig::fell_behind_ticks`, 0 = off). "Behind" is measured from the
   guest's hash reports; a guest catching up (a join, a reconnect, a resync,
   a late load) and a link already "having trouble" are left alone.

## 9. Diagnostics

- **Net meter** (toggle in Options → Graphics → Network meter, or with F7):
  - ping
  - input delay in ticks and milliseconds
  - jitter buffer
  - loss
  - resends
  - bytes/s
  - the last hash check
  - the prediction's mode, depth, cost and corrections, and (on the host)
    every guest's ping and queue
- The black box records the meter every 0.5 s (schema 17, `net`).
- `game --net-soak` runs several peers in one process over `LinkSim` on a real
  map, with joins, leaves, cheat changes, loss and jitter. It checks that every
  peer's `Sim::hash` matches on every tick.
- `--net-host` and `--net-join ADDR` run the same soak across machines, headless.

## 10. Work breakdown

1. `net` crate: transport, reliability, sessions, scheduler, `LinkSim`, unit
   tests in virtual time.
2. `core`: per-slot cheats, per-slot profile inputs (medal careers, assist,
   look sensitivity), a dynamic `target_hints`, a multi-human audit of
   `players.first()` and `[0]`, `add_human` and `remove_human` with a bot
   taking over. No single-player hash changes.
3. `game` netplay:
   - `WalkSim::local_slot`
   - command application
   - the net driver in the windowed loop (pacing, playback, look preview over
     pending inputs, non-blocking pause)
   - the `--net-soak` harness
4. `game` and `ui` interface (built November 13, 2026, branch
   `claude/lan-windowed`; not yet played on two machines):
   - Multiplayer → LAN → Host or Join
   - the lobby mirror
   - the join-by-address box and recent hosts
   - host cheat policy and in-round cheat controls
   - the results ready check
   - the net meter
5. Loading on worker threads: build the round's sim (`make_walk`) and load sounds
   off the event thread, so loading screens stay animated and heartbeats keep
   flowing. Built November 13, 2026 (§8b.3).
6. Prediction (§6):
   - `Sim: Clone`, with static map data behind an `Arc`
   - `Sim::step_predicted`
   - the predicted view in the game, with error easing, own-shot feedback
     de-duplication and the budget guard
   - lag compensation (`view_lag`, the hit-volume ring and the bolt sweep)
7. XDET and scalar math (§7).
8. Slow machines (§8b):
   - the quality governor (built November 13, 2026; graphics tiers plug in
     through `TierHook`)
   - deterministic parallel AI think
   - the fell-behind drop

## 11. Later work (not in the first release)

- **Online play through a dedicated relay server.**
  - The host's network thread is already the tick server.
  - A relay would run that same code headless, with NAT traversal and
    public addressing.
- **Galactic Conquest in multiplayer.** Turn order and the galaxy map would be
  driven by the host, as the setup screens are.
- **Splitscreen players in a LAN game.** Up to two local players would share
  one connection, each in their own slot.
- **Host migration.** It needs state snapshots (below) and a re-handshake to
  the new host.
- **Fast joins from state snapshots.** A late joiner would load a recent
  serialized `Sim` instead of replaying the round from tick 0. That needs serde
  on the dynamic sim state and a re-derivation of the static data from the
  bake. It is also the base for host migration and instant desync recovery.
- **Remembered host settings** and "play again with this group".
- **Spectating** while waiting for the next round when joining near the end of
  one.
- **In-game text chat or callouts.** Voice over Discord covers it for now.
- **Team switching between rounds and auto-balance.**
