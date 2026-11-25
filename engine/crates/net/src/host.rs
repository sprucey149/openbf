//! The host session core: the lobby, the per-connection links, timeouts and
//! the packets each update sends. `Hello` checks, slots, kicks and leaves
//! live in `host/admission.rs`; round flow, the tick clock, bundles and hash
//! checks in `host/round.rs`.
//!
//! Rules:
//! - Clock-injected: every entry point takes or reuses the `now` of the last
//!   [`HostCore::handle_datagram`] / [`HostCore::update`]; nothing reads a
//!   clock. Peers and players live in ordered maps.
//! - Slot 0 is the host's own player. Its pads go through the same input
//!   queue as everyone's ([`HostCore::submit_input`]).
//! - The host machine may have more local players (splitscreen):
//!   [`HostCore::add_local`] gives each the lowest free slot (which may lie
//!   above guests' slots), with no connection ("host locals",
//!   [`HostCore::host_local`]), and returns the unique name it gave. They are in the
//!   roster and seated like slot 0, loaded from the start, never reserved,
//!   kicked, benched or timed out, and their pads go in through
//!   [`HostCore::submit_input_for`], numbered with slot 0's sequence. The
//!   host's locals are one machine for the grouped close and the late
//!   seating (`scheduler` rules), as a guest machine's are. Guests take the
//!   lowest slots left.
//! - A new connection id is accepted only from a packet that carries
//!   reliable unit 0 (a `Hello`), and only while fewer than 64 handshakes
//!   are open. Packets from a known id at another address are dropped.
//! - `Hello` is checked in order: protocol, build (exact), ban, name, load
//!   barrier (`Busy`), then a slot: the same session token reclaims its old
//!   slot (connected or reserved), otherwise the lowest free slot below
//!   `max_players` (`Full` when none). Mid-round joins are accepted.
//! - A machine may bring several local players (splitscreen): its `Hello`
//!   lists locals 1.. after its first player, and admission takes one slot
//!   per local at once or refuses the whole `Hello` (`Full` when they do not
//!   all fit). Each local names its stable seat (0 for the `Hello`'s own
//!   player) and its token is [`crate::local_token`]`(token, seat)`, so a
//!   rejoin reclaims every seat's own slot whatever the line-up; a reclaimed
//!   slot keeps its name only while its player still asks for that name. `Welcome` returns local 0's slot and
//!   the others in order; each slot gets its own `PlayerJoined`. The
//!   connection's round messages (`Loaded`, hash reports, `CaughtUp`,
//!   `Resync`) speak for all of its slots; `Ready` names its local. A leave,
//!   a timeout, a kick or a bench of any of its slots acts on the whole
//!   machine, with one `PlayerLeft` (and one `Trouble`) per slot; a kick of
//!   a reserved slot also frees the machine's other reserved slots.
//! - A machine counts for hash agreement and the fell-behind drop while any
//!   of its slots plays the round (its connection reports for all of them,
//!   keyed by local 0's slot). A machine seated only in part at the load
//!   barrier gets a `PeerCaughtUp` for each unseated slot once it is live,
//!   so the game adds them as it adds a late joiner.
//! - A local whose pad is lost (a guest's `LocalAway { local }`, or
//!   [`HostCore::set_local_away`] for the host's own, slot 0 included)
//!   leaves the round with a `RemoveHuman` (a bot holds the slot) and
//!   [`NetEvent::LocalAway`]; it is never seated while away (`add_human`
//!   refuses it, `load_round` leaves it out). Its machine plays on: it
//!   still counts for hashes while any other slot is seated, and its
//!   pacing comes from the first seated local. When the pad is back
//!   (`LocalBack`), a removal its away queued and not yet closed is
//!   cancelled; otherwise a slot waiting in the round's late list is
//!   released (as `LocalBack`, not `PeerCaughtUp`) once its machine is
//!   live, and a slot of a live machine (one on the running round's stream,
//!   so not benched) gets [`NetEvent::LocalBack`], on which the
//!   host game seats it with `add_human` (the queue lines up with its
//!   siblings, `scheduler` rules). A new link (a rejoin) starts not away.
//! - A machine's inputs share one sequence: `Inputs` feeds local 0's slot,
//!   `LocalInputs { idx }` local `idx`'s (an index with no slot is a bad
//!   chunk). Its status reports `input_through` as the slowest of its slots
//!   and the pacing figures of local 0's.
//! - A player who leaves or times out keeps slot and name for
//!   [`crate::RESERVE_FOR`] for the same token. A kick frees the slot.
//! - Silence: "trouble" after [`crate::TROUBLE_AFTER`], dropped after
//!   [`crate::DROP_AFTER`] ([`crate::DROP_AFTER_LOADING`] during the load
//!   barrier). Every peer hears from the host at least every
//!   [`crate::HEARTBEAT`].
//! - Lobby state is opaque, versioned and sent whole, at most every 100 ms.
//! - A peer on another protocol version gets a header-only answer (at most
//!   once a second per address) so it can say "version mismatch".
//! - Discovery: one `Found` per datagram however many `Find`s it carries,
//!   at most every [`FOUND_EVERY`] per address (a browser asks once a
//!   second), so the host is no reply amplifier. A `Find` from another
//!   protocol version (the frozen layout, `packet::frozen_find_nonce`) is
//!   answered the same way, so that browser can list this host as another
//!   version; it never opens a peer.

mod admission;
use admission::Hello;
pub use admission::{full_detail, parse_full_detail};
mod round;

use crate::bundle::TickBundle;
use crate::conn::{Conn, ConnStats};
use crate::msg::{Msg, RejectReason, RosterEntry};
use crate::packet::{
    bare_packet, parse_chunks, parse_header, raw_conn_id, Chunk, DropCounts, DropReason,
};
use crate::scheduler::SlotQueue;
use crate::{Micros, DROP_AFTER, DROP_AFTER_LOADING, HEARTBEAT, TROUBLE_AFTER};
use core::sim::PadState;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{IpAddr, SocketAddr};

pub use round::RoundPhase;

/// The default lobby maximum (humans, host included).
pub const DEFAULT_MAX_PLAYERS: u8 = 16;
/// The most humans a host takes: their pads alone always fit one bundle
/// (`bundle` rules), whatever they hold.
pub const MAX_PLAYERS: u8 = {
    let n = (crate::packet::BUNDLE_BUDGET - 2) / crate::bundle::MAX_PAD_BYTES;
    if n < crate::MAX_SLOTS {
        n as u8
    } else {
        crate::MAX_SLOTS as u8
    }
};
/// [`HostConfig::lag_comp_cap`]'s default: the sim's own cap.
pub const DEFAULT_LAG_COMP_CAP: u8 = core::sim::lag_comp::VIEW_LAG_CAP;
const MAX_HANDSHAKES: usize = 64;
const LOBBY_INTERVAL: Micros = Micros::from_millis(100);
const CLOSE_LINGER: Micros = Micros::from_secs(2);
/// The shortest gap between two discovery answers to one address.
pub const FOUND_EVERY: Micros = Micros::from_millis(250);
/// The largest discovery info blob.
pub const MAX_INFO: usize = 1000;

/// What the host is and who plays on it.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// The sim build identity; a client's must match exactly.
    pub build: String,
    /// The host player's name (slot 0).
    pub name: String,
    pub profile: Vec<u8>,
    /// Humans allowed, host included (1..=[`MAX_PLAYERS`]).
    pub max_players: u8,
    /// The input queue depth the scheduler aims for.
    pub queue_target: usize,
    /// Lag compensation (`LAN_MULTIPLAYER.md` §6): the most ticks a closed
    /// pad's `view_lag` may say its player was behind (8, about 250 ms); 0
    /// turns lag compensation off.
    pub lag_comp_cap: u8,
    /// Slow machines (`LAN_MULTIPLAYER.md` §8b): a guest playing the round
    /// whose reported tick falls more than this many ticks behind live is
    /// dropped with [`FELL_BEHIND_TEXT`] ([`DEFAULT_FELL_BEHIND_TICKS`], 10 s);
    /// 0 turns the drop off. A guest catching up (a join, a reconnect, a
    /// resync) is exempt until it reports `CaughtUp`.
    pub fell_behind_ticks: u32,
}

/// [`HostConfig::fell_behind_ticks`]' default: 10 s of ticks.
pub const DEFAULT_FELL_BEHIND_TICKS: u32 = 300;
/// The reject detail a guest dropped for falling behind gets.
pub const FELL_BEHIND_TEXT: &str = "Your computer could not keep up with this battle";

impl HostConfig {
    pub fn new(build: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            build: build.into(),
            name: name.into(),
            profile: Vec::new(),
            max_players: DEFAULT_MAX_PLAYERS,
            queue_target: 1,
            lag_comp_cap: DEFAULT_LAG_COMP_CAP,
            fell_behind_ticks: DEFAULT_FELL_BEHIND_TICKS,
        }
    }
}

/// Why a player left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LeaveReason {
    Left,
    Timeout,
    Kicked,
    /// The same token connected again; the old link was replaced.
    Replaced,
    /// Dropped for falling behind live ([`HostConfig::fell_behind_ticks`]).
    FellBehind,
}

/// What the host game hears from its network core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetEvent {
    PlayerJoined {
        slot: u8,
        /// Its machine (splitscreen, issue #194 B19): local 0's slot of the
        /// connection that holds it ([`HostCore::machine_slots`]'s first),
        /// `slot` itself for a one-player machine. The lobby groups and
        /// kicks by it.
        machine: u8,
        name: String,
        content: Vec<u8>,
        profile: Vec<u8>,
        reconnect: bool,
    },
    PlayerLeft {
        slot: u8,
        reason: LeaveReason,
    },
    /// A refused `Hello` (for the log).
    Refused {
        addr: SocketAddr,
        reason: RejectReason,
    },
    Trouble {
        slot: u8,
        on: bool,
    },
    PeerLoaded {
        slot: u8,
        round_id: u32,
        static_digest: u64,
        hash0: u64,
    },
    /// Every connected roster player reported `Loaded`.
    AllLoaded {
        round_id: u32,
    },
    /// The round's clock starts `lead_us` from now (tick 0 closes a tick
    /// after that): the host game leaves its loading screen then.
    RoundStarted {
        round_id: u32,
        lead_us: u32,
    },
    /// A closed tick; the host game steps it at once.
    Bundle(TickBundle),
    PeerCaughtUp {
        slot: u8,
        round_id: u32,
        tick: u32,
    },
    /// Guest `slot`'s hash at `tick` of round `round_id` differed from the
    /// host game's; the guest is told only by [`HostCore::tell_desync`].
    Desync {
        round_id: u32,
        slot: u8,
        tick: u32,
        host_hash: u64,
        peer_hash: u64,
    },
    /// Every guest live in round `round_id` (connected, in its bundles, not
    /// catching up) reported a hash for check `tick` equal to the host
    /// game's: everyone agrees on that tick's game (the host's self-check
    /// keeps it as its checkpoint).
    HashesAgreed {
        round_id: u32,
        tick: u32,
    },
    /// A guest's per-tick hash rows around its desync at `tick`
    /// ([`crate::msg::Msg::DesyncHashes`]), for the host's report.
    DesyncHashes {
        slot: u8,
        tick: u32,
        build: String,
        target: String,
        first_tick: u32,
        width: u8,
        values: Vec<u64>,
    },
    PeerReady {
        slot: u8,
        round_id: u32,
        profile: Vec<u8>,
    },
    /// Local player `slot`'s pad went away (a guest's `LocalAway` or
    /// [`HostCore::set_local_away`]): its slot leaves the round (a
    /// `RemoveHuman`, which the host core schedules itself) and is not
    /// seated again until it is back.
    LocalAway {
        slot: u8,
    },
    /// Local player `slot`'s pad is back while its machine plays round
    /// `round_id` and the slot is out of it: the host game seats it again
    /// with `add_human`, as after `PeerCaughtUp` (the returning queue lines
    /// up with its machine's other locals). A slot back before its seat
    /// was taken keeps it, and one whose machine is still catching up gets
    /// `PeerCaughtUp` instead; neither gets this event.
    LocalBack {
        slot: u8,
        round_id: u32,
        tick: u32,
    },
    Request {
        slot: u8,
        data: Vec<u8>,
    },
    Chat {
        slot: u8,
        text: String,
    },
    /// The answer to [`crate::HostCommand::AddLocal`] (splitscreen B19):
    /// the host machine's player in Players-screen seat `seat` got `slot`
    /// and the unique `name` [`HostCore::add_local`] gave it, or the
    /// reason it was refused: [`RejectReason::Full`] (no free slot),
    /// [`RejectReason::Busy`] (a round loading) or
    /// [`RejectReason::BadName`].
    LocalAdded {
        seat: u8,
        added: Result<(u8, String), RejectReason>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Handshake,
    /// Joined; the slot of its local 0 (the others are [`Peer::extra`]).
    Joined(u8),
    Closing(Micros),
}

struct Peer {
    conn: Conn,
    phase: Phase,
    /// The slots of the machine's locals 1.. (splitscreen), in order.
    extra: Vec<u8>,
    /// The catch-up stream this peer is on (0: none).
    stream: u32,
    stream_round: u32,
    stream_start: u32,
    /// The first live tick the peer does not hold yet.
    next_needed: u32,
    trouble: bool,
    /// The log length when this peer's stream last sent (a packet goes out
    /// for new ticks, otherwise only for an ack, a resend or a heartbeat).
    /// Reset with every new stream.
    sent_through: u32,
    /// The slot whose queue paces the machine's status, the offset added
    /// to its starvation count and the count last put in a status
    /// (`build_for`). With one local the slot never changes and the offset
    /// stays 0, so its status bytes are unchanged.
    pace_slot: Option<u8>,
    starved_offset: u32,
    starved_sent: u32,
}

pub(crate) struct Player {
    name: String,
    token: u64,
    profile: Vec<u8>,
    conn: Option<u32>,
    /// Its machine's `Hello` token (its own token is derived from it and
    /// its seat): a kick of a reserved slot takes the machine's other
    /// reserved slots with it.
    machine: u64,
    /// One of the host machine's own players (slot 0 and every
    /// [`HostCore::add_local`] slot): no connection, never reserved,
    /// kicked or timed out.
    host_local: bool,
    queue: SlotQueue,
    reserved_until: Option<Micros>,
    /// A human in the current round's bundles.
    active: bool,
    loaded: bool,
    /// The newest tick this player is known to have stepped: its hash
    /// reports, or the tick it became live (the fell-behind check).
    seen_tick: u32,
    /// Stepping a catch-up stream (a join, a reconnect, a resync) until
    /// its `CaughtUp`.
    catching_up: bool,
    /// Its pad is away (`LocalAway`, [`HostCore::set_local_away`]): out of
    /// the round and never seated until it is back.
    away: bool,
}

impl Player {
    /// The machine whose locals close and seat together (`scheduler`
    /// rules): a guest's connection, or the host's own machine (`u64::MAX`,
    /// above every connection id). `None` for a reserved slot.
    fn machine_key(&self) -> Option<u64> {
        match self.conn {
            Some(c) => Some(u64::from(c)),
            None => self.host_local.then_some(u64::MAX),
        }
    }
}

/// One connected player's link figures.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PeerStats {
    pub slot: u8,
    pub name: String,
    pub rtt_ms: f64,
    pub rttvar_ms: f64,
    pub silence_ms: f64,
    pub trouble: bool,
    pub queue_depth: u8,
    pub starved: u32,
    /// Slips merged away and the published slack (µs).
    pub slips: u32,
    pub slack_us: Option<i64>,
    pub link: ConnStats,
}

/// The host's net meter.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct HostStats {
    pub tick: u32,
    pub local_depth: u8,
    pub local_starved: u32,
    /// The host player's published slack (µs, `scheduler`).
    pub local_slack_us: Option<i64>,
    pub peers: Vec<PeerStats>,
    pub drops: DropCounts,
    /// Live packets whose next bundle did not fit (the `bundle` rules keep
    /// this 0; anything else is a host bug that stalls that guest).
    pub oversize_bundles: u64,
}

/// The host's protocol state machine.
pub struct HostCore {
    config: HostConfig,
    now: Micros,
    peers: BTreeMap<u32, Peer>,
    players: BTreeMap<u8, Player>,
    banned_tokens: BTreeSet<u64>,
    banned_ips: BTreeSet<IpAddr>,
    events: VecDeque<NetEvent>,
    outbox: Vec<(SocketAddr, Vec<u8>)>,
    version_replies: BTreeMap<SocketAddr, Micros>,
    found_replies: BTreeMap<SocketAddr, Micros>,
    lobby: Vec<u8>,
    lobby_version: u32,
    lobby_dirty: bool,
    lobby_sent: Micros,
    info: Vec<u8>,
    pub(crate) round: round::RoundState,
    pub drops: DropCounts,
    oversize_bundles: u64,
    /// The host's own pacing slot ([`HostCore::host_pace_slot`]) as of the
    /// last update, and the offset that keeps its starvation count one
    /// running count across a change (as a guest's status does).
    host_pace: u8,
    host_starved_offset: u32,
    host_starved_sent: u32,
}

impl HostCore {
    pub fn new(mut config: HostConfig) -> Self {
        config.max_players = config.max_players.clamp(1, MAX_PLAYERS);
        let mut players = BTreeMap::new();
        players.insert(
            0,
            Player {
                name: config.name.clone(),
                token: 0,
                profile: config.profile.clone(),
                conn: None,
                machine: 0,
                host_local: true,
                queue: SlotQueue::new(config.queue_target),
                reserved_until: None,
                active: false,
                loaded: true,
                seen_tick: 0,
                catching_up: false,
                away: false,
            },
        );
        Self {
            config,
            now: Micros::ZERO,
            peers: BTreeMap::new(),
            players,
            banned_tokens: BTreeSet::new(),
            banned_ips: BTreeSet::new(),
            events: VecDeque::new(),
            outbox: Vec::new(),
            version_replies: BTreeMap::new(),
            found_replies: BTreeMap::new(),
            lobby: Vec::new(),
            lobby_version: 0,
            lobby_dirty: false,
            lobby_sent: Micros::ZERO,
            info: Vec::new(),
            round: round::RoundState::default(),
            drops: DropCounts::default(),
            oversize_bundles: 0,
            host_pace: 0,
            host_starved_offset: 0,
            host_starved_sent: 0,
        }
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    /// The next event for the host game.
    pub fn poll_event(&mut self) -> Option<NetEvent> {
        self.events.pop_front()
    }

    /// The host player's input for its next input tick, with the confirmed
    /// tick the host was viewing (its sim's `tick`: the bundles it has
    /// stepped), stamped into `view_lag` at the close like a guest's.
    pub fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        self.submit_input_for(0, pad, view_tick);
    }

    /// [`HostCore::submit_input`] for one of the host's own players
    /// (module rules): `slot` 0 or a slot [`HostCore::add_local`] gave. The
    /// host game submits every one of its locals once per frame, a missing
    /// pad as a neutral one, so they all number the same sequences. A
    /// slot that is no host local is ignored.
    pub fn submit_input_for(&mut self, slot: u8, pad: PadState, view_tick: u32) {
        let pad = crate::bundle::strip_tracked_inputs(pad);
        let running = self.round.accepts_inputs();
        if let Some(p) = self.players.get_mut(&slot).filter(|p| p.host_local) {
            let queue = running && p.active;
            p.queue.push(pad, view_tick, self.now, queue);
        }
    }

    /// Adds one of the host machine's own extra players (splitscreen) as
    /// `name` with `profile` (module rules): the lowest free slot below
    /// `max_players` (any free slot, not "slots 1-3": guests may already
    /// hold the low ones, so callers ask [`HostCore::host_local`]), its
    /// name made unique like a guest's. Returns the slot and the name it
    /// was given, the one the roster and every guest show. Refused with
    /// [`RejectReason::Busy`] while a round is loading,
    /// [`RejectReason::BadName`] for a name no guest could join with, and
    /// [`RejectReason::Full`] when no slot is free. The slot is seated by the next
    /// `load_round` (or `add_human` in a running round, at once or later)
    /// like any other.
    ///
    /// Call it between frames, before any [`HostCore::submit_input_for`] of
    /// the frame: the new queue is numbered from slot 0's last sequence,
    /// so a call after some locals submitted this frame leaves it one
    /// sequence out of step for good.
    pub fn add_local(
        &mut self,
        name: &str,
        profile: Vec<u8>,
    ) -> Result<(u8, String), RejectReason> {
        // The order a guest machine's Hello is judged in (admission).
        if !crate::msg::valid_name(name) {
            return Err(RejectReason::BadName);
        }
        if self.round.is_loading() {
            return Err(RejectReason::Busy);
        }
        let slot = (1..self.config.max_players)
            .find(|s| !self.players.contains_key(s))
            .ok_or(RejectReason::Full)?;
        let name = crate::msg::unique_name(name, |n| {
            self.players
                .values()
                .any(|p| p.name.eq_ignore_ascii_case(n))
        });
        let head = &self.players[&0].queue;
        // Every local submits once a frame, so between frames they all
        // stand on the same sequence; a mismatch means a mid-frame call.
        debug_assert!(
            self.players
                .values()
                .filter(|p| p.host_local)
                .all(|p| p.queue.recv_through() == head.recv_through()),
            "add_local between submit_input_for calls of one frame"
        );
        let mut queue = SlotQueue::new(self.config.queue_target);
        // Numbered with the host's other locals (`scheduler` rules).
        queue.start_beside(head);
        self.players.insert(
            slot,
            Player {
                name: name.clone(),
                token: 0,
                profile,
                conn: None,
                machine: 0,
                host_local: true,
                queue,
                reserved_until: None,
                active: false,
                loaded: true,
                seen_tick: 0,
                catching_up: false,
                away: false,
            },
        );
        Ok((slot, name))
    }

    /// [`HostCore::add_local`] for a threaded session, whose caller cannot
    /// read a return value: the answer comes back as
    /// [`NetEvent::LocalAdded`] for `seat` (splitscreen B19).
    pub fn add_local_event(&mut self, seat: u8, name: &str, profile: Vec<u8>) {
        let added = self.add_local(name, profile);
        self.events.push_back(NetEvent::LocalAdded { seat, added });
    }

    /// Takes away a host's extra player from [`HostCore::add_local`]: its
    /// slot is freed and, in a round, a bot takes its place. False when
    /// `slot` is slot 0 or no host local.
    pub fn remove_local(&mut self, slot: u8) -> bool {
        if slot == 0 || !self.host_local(slot) {
            return false;
        }
        let p = self.players.remove(&slot).expect("host local");
        self.round.remove_human(slot, p.active);
        self.check_all_loaded();
        self.follow_host_pace();
        true
    }

    /// One of the host machine's own players' pad went away (`away`) or
    /// came back (module rules; slot 0 included). The slot leaves the round
    /// with a `RemoveHuman` and [`NetEvent::LocalAway`]; once back,
    /// [`NetEvent::LocalBack`] asks the host game to seat it again. False
    /// when `slot` is no host local.
    pub fn set_local_away(&mut self, slot: u8, away: bool) -> bool {
        if !self.host_local(slot) {
            return false;
        }
        self.mark_away(slot, away);
        true
    }

    /// Whether `slot`'s pad is away (`LocalAway`, [`HostCore::set_local_away`]).
    pub fn is_away(&self, slot: u8) -> bool {
        self.players.get(&slot).is_some_and(|p| p.away)
    }

    /// `slot`'s pad went away or came back (module rules).
    pub(crate) fn mark_away(&mut self, slot: u8, away: bool) {
        let Some(p) = self.players.get_mut(&slot) else {
            return;
        };
        if p.away == away {
            return;
        }
        p.away = away;
        let active = p.active;
        let (host_local, catching_up, conn) = (p.host_local, p.catching_up, p.conn);
        if away {
            self.round.remove_human(slot, active);
            self.events.push_back(NetEvent::LocalAway { slot });
            return;
        }
        // Live: the host's own, or a machine on this round's stream (a
        // benched machine keeps its link but gets no bundles).
        let live = host_local
            || (!catching_up
                && conn
                    .and_then(|id| self.peers.get(&id))
                    .is_some_and(|pr| pr.stream_round == self.round.id));
        // Back before the removal its own away queued closed: it keeps its
        // seat. An unseated slot's removal (the load's tick-0 one) stays.
        if active && self.round.cancel_remove(slot) {
            return;
        }
        if self.round.late_locals.contains(&slot) {
            // Released as a return (`LocalBack`), not a late joiner.
            if !self.round.returned.contains(&slot) {
                self.round.returned.push(slot);
            }
            self.release_late_locals();
            return;
        }
        if self.round.is_running() && !active && live {
            self.events.push_back(NetEvent::LocalBack {
                slot,
                round_id: self.round.id,
                tick: self.round.next_tick(),
            });
        }
    }

    /// The host machine's playing local whose queue paces it: slot 0, or
    /// while slot 0's pad is away the first seated host local (as a guest
    /// machine's status does).
    pub fn host_pace_slot(&self) -> u8 {
        if self.players.get(&0).is_some_and(|p| p.active) {
            return 0;
        }
        self.players
            .iter()
            .find(|(_, p)| p.host_local && p.active)
            .map_or(0, |(s, _)| *s)
    }

    /// The host's pacing figures for its own [`crate::InputPacer`]: depth
    /// and starvations of [`HostCore::host_pace_slot`], the count kept
    /// running across a change of slot. Slot 0's own while it plays.
    pub fn host_pacing(&self) -> (u8, u32) {
        match self.players.get(&self.host_pace) {
            Some(p) => (
                p.queue.depth,
                p.queue.starved.wrapping_add(self.host_starved_offset),
            ),
            None => (0, self.host_starved_sent),
        }
    }

    /// The host's published input slack ([`HostCore::host_pace_slot`]'s).
    pub fn host_slack_us(&self) -> Option<i64> {
        self.players
            .get(&self.host_pace)
            .and_then(|p| p.queue.slack_us())
    }

    /// Moves the host's pacing to [`HostCore::host_pace_slot`], keeping
    /// the starvation count running.
    fn follow_host_pace(&mut self) {
        let pace = self.host_pace_slot();
        if pace != self.host_pace {
            self.host_starved_offset = self
                .host_starved_sent
                .wrapping_sub(self.players[&pace].queue.starved);
            self.host_pace = pace;
        }
        self.host_starved_sent = self.host_pacing().1;
    }

    /// Whether `slot` is one of the host machine's own players: slot 0 and
    /// the slots of [`HostCore::add_local`].
    pub fn host_local(&self, slot: u8) -> bool {
        self.players.get(&slot).is_some_and(|p| p.host_local)
    }

    /// The host machine's own players' slots, slot 0 first.
    pub fn host_locals(&self) -> Vec<u8> {
        self.players
            .iter()
            .filter(|(_, p)| p.host_local)
            .map(|(s, _)| *s)
            .collect()
    }

    /// Sets the discovery reply blob (name, map, players, ...), truncated to
    /// [`MAX_INFO`] bytes.
    pub fn set_info(&mut self, info: Vec<u8>) {
        let mut info = info;
        info.truncate(MAX_INFO);
        self.info = info;
    }

    /// Replaces the lobby state; guests get it within 100 ms.
    pub fn set_lobby(&mut self, state: Vec<u8>) {
        self.lobby = state;
        self.lobby_version = self.lobby_version.wrapping_add(1);
        self.lobby_dirty = true;
    }

    /// Connected players by slot (host included), with names.
    pub fn players(&self) -> Vec<(u8, String)> {
        self.players
            .iter()
            .filter(|(_, p)| p.host_local || p.conn.is_some())
            .map(|(s, p)| (*s, p.name.clone()))
            .collect()
    }

    /// Sends an opaque message to one player, or to every guest.
    pub fn send_notice(&mut self, slot: Option<u8>, data: Vec<u8>) {
        let m = Msg::Notice(data).encode();
        for (s, id) in self.guest_conns() {
            if slot.is_none_or(|x| x == s) {
                if let Some(peer) = self.peers.get_mut(&id) {
                    peer.conn.send_reliable(&m);
                }
            }
        }
    }

    /// The host player's chat line, to every guest.
    pub fn chat(&mut self, text: &str) {
        self.relay_chat(0, text);
    }

    fn relay_chat(&mut self, from: u8, text: &str) {
        let m = Msg::Chat {
            slot: from,
            text: text.to_owned(),
        }
        .encode();
        for (s, id) in self.guest_conns() {
            if s != from {
                if let Some(peer) = self.peers.get_mut(&id) {
                    peer.conn.send_reliable(&m);
                }
            }
        }
    }

    /// Tells every guest the host is leaving; send one more update after it.
    pub fn close(&mut self) {
        let m = Msg::Leave.encode();
        for peer in self.peers.values_mut() {
            peer.conn.send_reliable(&m);
        }
    }

    fn guest_conns(&self) -> Vec<(u8, u32)> {
        self.peers
            .iter()
            .filter_map(|(id, p)| match p.phase {
                Phase::Joined(s) => Some((s, *id)),
                _ => None,
            })
            .collect()
    }

    /// Every slot connection `id` holds, local 0 first (none before it
    /// joined).
    fn conn_slots(&self, id: u32) -> Vec<u8> {
        match self.peers.get(&id) {
            Some(Peer {
                phase: Phase::Joined(s),
                extra,
                ..
            }) => std::iter::once(*s).chain(extra.iter().copied()).collect(),
            _ => Vec::new(),
        }
    }

    /// The slots of `slot`'s machine while it is connected (local 0 first),
    /// else `slot` alone.
    pub fn machine_slots(&self, slot: u8) -> Vec<u8> {
        let slots = self
            .players
            .get(&slot)
            .and_then(|p| p.conn)
            .map(|id| self.conn_slots(id))
            .unwrap_or_default();
        if slots.contains(&slot) {
            slots
        } else {
            vec![slot]
        }
    }

    pub(crate) fn send_to_slot(&mut self, slot: u8, msg: &Msg) {
        let id = self.players.get(&slot).and_then(|p| p.conn);
        if let Some(peer) = id.and_then(|id| self.peers.get_mut(&id)) {
            peer.conn.send_reliable(&msg.encode());
        }
    }

    /// Takes one datagram from the network.
    pub fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        self.now = now;
        let (h, body) = match parse_header(bytes) {
            Ok(x) => x,
            Err(d) => {
                self.drops.count(d);
                match (d, raw_conn_id(bytes)) {
                    // A browser on another version asks in the frozen
                    // layout: it hears this host's own `Found` and lists
                    // it as "different version". No peer opens for it.
                    (DropReason::Protocol(_), Some(0)) => {
                        if let Some(nonce) = crate::packet::frozen_find_nonce(bytes) {
                            self.answer_find(from, nonce);
                        }
                    }
                    (DropReason::Protocol(_), Some(id)) => self.answer_version(from, id),
                    _ => {}
                }
                return;
            }
        };
        let Ok(chunks) = parse_chunks(body) else {
            self.drops.malformed += 1;
            return;
        };
        if h.conn == 0 {
            let nonce = chunks.iter().find_map(|c| match c {
                Chunk::Find { nonce } => Some(*nonce),
                _ => None,
            });
            if let Some(nonce) = nonce {
                self.answer_find(from, nonce);
            }
            return;
        }
        if !self.peers.contains_key(&h.conn) {
            let opens = chunks
                .iter()
                .any(|c| matches!(c, Chunk::Reliable { unit: 0, .. }));
            let handshakes = self
                .peers
                .values()
                .filter(|p| p.phase == Phase::Handshake)
                .count();
            if !opens || handshakes >= MAX_HANDSHAKES {
                self.drops.unknown_conn += 1;
                return;
            }
            self.peers.insert(
                h.conn,
                Peer {
                    conn: Conn::new(h.conn, from, now),
                    phase: Phase::Handshake,
                    extra: Vec::new(),
                    stream: 0,
                    stream_round: 0,
                    stream_start: 0,
                    next_needed: 0,
                    trouble: false,
                    sent_through: 0,
                    pace_slot: None,
                    starved_offset: 0,
                    starved_sent: 0,
                },
            );
        }
        let peer = self.peers.get_mut(&h.conn).expect("peer present");
        if peer.conn.addr != from {
            self.drops.wrong_addr += 1;
            return;
        }
        let Some(rest) = peer.conn.receive(now, &h, chunks, bytes.len()) else {
            return;
        };
        let phase = peer.phase;
        for c in rest {
            match c {
                Chunk::ClientStatus {
                    stream,
                    next_needed,
                } => {
                    if let Some(peer) = self.peers.get_mut(&h.conn) {
                        if stream == peer.stream && next_needed > peer.next_needed {
                            peer.next_needed = next_needed;
                        }
                    }
                }
                Chunk::Inputs(body) => {
                    if let Phase::Joined(slot) = phase {
                        self.take_inputs(slot, body);
                    }
                }
                Chunk::LocalInputs { idx, data } => {
                    let slot = match phase {
                        Phase::Joined(_) => self
                            .peers
                            .get(&h.conn)
                            .and_then(|p| p.extra.get(usize::from(idx) - 1).copied()),
                        _ => None,
                    };
                    match slot {
                        Some(slot) => self.take_inputs(slot, data),
                        None => self.drops.bad_chunk += 1,
                    }
                }
                _ => self.drops.bad_chunk += 1,
            }
        }
        self.drain_messages(h.conn);
    }

    /// Discovery's answer, at most every [`FOUND_EVERY`] per address. Its
    /// first chunk is the `Found` in the frozen layout
    /// (`packet::frozen_found`), whatever version asked.
    fn answer_find(&mut self, from: SocketAddr, nonce: u64) {
        let now = self.now;
        let due = self
            .found_replies
            .get(&from)
            .is_none_or(|t| now.0 >= t.0 + FOUND_EVERY.0);
        if due {
            self.found_replies.insert(from, now);
            let mut pw = bare_packet(0);
            pw.found(nonce, &self.info);
            self.outbox.push((from, pw.finish()));
        }
    }

    fn answer_version(&mut self, from: SocketAddr, conn: u32) {
        let due = self
            .version_replies
            .get(&from)
            .is_none_or(|t| self.now.0 >= t.0 + 1_000_000);
        if due && conn != 0 {
            self.version_replies.insert(from, self.now);
            self.outbox.push((from, bare_packet(conn).finish()));
        }
    }

    fn take_inputs(&mut self, slot: u8, body: &[u8]) {
        let accepts = self.round.accepts_inputs();
        let now = self.now;
        let Some(p) = self.players.get_mut(&slot) else {
            return;
        };
        let q = &mut p.queue;
        match crate::packet::decode_inputs(body, |s| q.base(s)) {
            Ok(Some((base, pads))) => q.receive(base, &pads, accepts && p.active, now),
            Ok(None) => self.drops.bad_chunk += 1,
            Err(_) => self.drops.bad_chunk += 1,
        }
    }

    fn drain_messages(&mut self, id: u32) {
        loop {
            let Some(peer) = self.peers.get_mut(&id) else {
                return;
            };
            if peer.conn.broken {
                self.drop_peer(id, LeaveReason::Left);
                return;
            }
            let Some(bytes) = peer.conn.take_message() else {
                return;
            };
            let phase = peer.phase;
            let Ok(msg) = Msg::decode(&bytes) else {
                self.drops.bad_message += 1;
                continue;
            };
            match (phase, msg) {
                (
                    Phase::Handshake,
                    Msg::Hello {
                        protocol,
                        build,
                        name,
                        token,
                        content,
                        profile,
                        locals,
                    },
                ) => {
                    let hello = Hello {
                        protocol,
                        build,
                        name,
                        token,
                        content,
                        profile,
                        locals,
                    };
                    self.admit(id, hello);
                }
                (Phase::Joined(_), Msg::Leave) => {
                    self.drop_peer(id, LeaveReason::Left);
                    return;
                }
                (Phase::Joined(slot), Msg::Chat { text, .. }) => {
                    self.events.push_back(NetEvent::Chat {
                        slot,
                        text: text.clone(),
                    });
                    self.relay_chat(slot, &text);
                }
                (Phase::Joined(slot), Msg::Request(data)) => {
                    self.events.push_back(NetEvent::Request { slot, data });
                }
                (
                    Phase::Joined(_),
                    Msg::Ready {
                        round_id,
                        profile,
                        local,
                    },
                ) => match self.conn_slots(id).get(usize::from(local)) {
                    Some(&slot) => self.events.push_back(NetEvent::PeerReady {
                        slot,
                        round_id,
                        profile,
                    }),
                    None => self.drops.bad_message += 1,
                },
                (Phase::Joined(_), m @ (Msg::LocalAway { .. } | Msg::LocalBack { .. })) => {
                    let (Msg::LocalAway { local } | Msg::LocalBack { local }) = m else {
                        unreachable!()
                    };
                    let away = matches!(m, Msg::LocalAway { .. });
                    match self.conn_slots(id).get(usize::from(local)) {
                        Some(&slot) => {
                            self.mark_away(slot, away);
                        }
                        None => self.drops.bad_message += 1,
                    }
                }
                (Phase::Joined(slot), m) => {
                    if !self.round_message(slot, m) {
                        self.drops.bad_message += 1;
                    }
                }
                _ => self.drops.bad_message += 1,
            }
        }
    }

    /// Advances timers and the tick clock; returns the datagrams to send.
    pub fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        self.now = now;
        self.round_update();
        self.follow_host_pace();
        self.expire(now);
        let lobby = if self.lobby_dirty && now.0 >= self.lobby_sent.0 + LOBBY_INTERVAL.0 {
            self.lobby_dirty = false;
            self.lobby_sent = now;
            Some(
                Msg::Lobby {
                    version: self.lobby_version,
                    state: self.lobby.clone(),
                }
                .encode(),
            )
        } else {
            None
        };
        let mut out = std::mem::take(&mut self.outbox);
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            if let (Some(l), Some(peer)) = (&lobby, self.peers.get_mut(&id)) {
                if matches!(peer.phase, Phase::Joined(_)) {
                    peer.conn.send_reliable(l);
                }
            }
            self.build_for(id, now, &mut out);
        }
        out
    }

    fn expire(&mut self, now: Micros) {
        let limit = if self.round.is_loading() {
            DROP_AFTER_LOADING
        } else {
            DROP_AFTER
        };
        let ids: Vec<u32> = self.peers.keys().copied().collect();
        for id in ids {
            let peer = self.peers.get_mut(&id).expect("peer");
            let silence = peer.conn.silence(now);
            match peer.phase {
                Phase::Closing(until) => {
                    if now >= until || peer.conn.reliable_idle() {
                        self.peers.remove(&id);
                    }
                    continue;
                }
                Phase::Handshake if silence >= DROP_AFTER => {
                    self.peers.remove(&id);
                    continue;
                }
                Phase::Handshake => continue,
                Phase::Joined(_) => {
                    if silence >= limit {
                        self.drop_peer(id, LeaveReason::Timeout);
                        continue;
                    }
                    let trouble = silence >= TROUBLE_AFTER;
                    if trouble != peer.trouble {
                        peer.trouble = trouble;
                        for s in self.conn_slots(id) {
                            self.events.push_back(NetEvent::Trouble {
                                slot: s,
                                on: trouble,
                            });
                        }
                        if !trouble {
                            // Heard again: half the fell-behind limit to report.
                            let grace = self
                                .round
                                .next_tick()
                                .saturating_sub(self.config.fell_behind_ticks / 2);
                            for s in self.conn_slots(id) {
                                if let Some(p) = self.players.get_mut(&s) {
                                    p.seen_tick = p.seen_tick.max(grace);
                                }
                            }
                        }
                    }
                }
            }
        }
        self.players.retain(|_, p| {
            p.host_local || p.conn.is_some() || p.reserved_until.is_some_and(|t| now < t)
        });
        self.version_replies.retain(|_, t| now.0 < t.0 + 10_000_000);
        self.found_replies
            .retain(|_, t| now.0 < t.0 + FOUND_EVERY.0);
    }

    fn build_for(&mut self, id: u32, now: Micros, out: &mut Vec<(SocketAddr, Vec<u8>)>) {
        let log_len = self.round.log.len() as u32;
        let streaming = self.round.streams_to(self.peers[&id].stream_round);
        let peer = self.peers.get_mut(&id).expect("peer");
        let new_ticks = streaming && log_len > peer.sent_through && peer.stream != 0;
        if !new_ticks && !peer.conn.wants_send(now, HEARTBEAT) {
            return;
        }
        // The pacing figures are the first playing local's (the machine
        // paces all its locals under one clock; local 0 unless its pad is
        // away, whose queue no longer closes); `input_through` is the
        // slowest of its slots, so the client keeps every local's
        // unreceived inputs.
        let status = match peer.phase {
            Phase::Joined(slot) => self.players.get(&slot).map(|p| {
                let slots = peer
                    .extra
                    .iter()
                    .filter_map(|s| self.players.get(s).map(|x| (*s, x)));
                let through = slots
                    .clone()
                    .map(|(_, x)| x.queue.recv_through())
                    .fold(p.queue.recv_through(), u32::min);
                let (pace_slot, pace) = if p.active {
                    (slot, p)
                } else {
                    slots.clone().find(|(_, x)| x.active).unwrap_or((slot, p))
                };
                // The starvation count the client sees stays one running
                // count when pacing moves to another local (a jump or a
                // fall would read as a burst of starvations).
                if peer.pace_slot != Some(pace_slot) {
                    if peer.pace_slot.is_some() {
                        peer.starved_offset = peer.starved_sent.wrapping_sub(pace.queue.starved);
                    }
                    peer.pace_slot = Some(pace_slot);
                }
                let starved = pace.queue.starved.wrapping_add(peer.starved_offset);
                peer.starved_sent = starved;
                (through, pace.queue.depth, starved, pace.queue.slack_us())
            }),
            _ => None,
        };
        let log = &self.round.log;
        let (stream, start, next) = (peer.stream, peer.stream_start, peer.next_needed);
        let mut sent_through = peer.sent_through;
        let mut oversize = false;
        let packets = peer.conn.build(now, |pw| {
            if let Some((through, depth, starved, slack)) = status {
                pw.host_status(through, depth, starved, slack);
            }
            if streaming && stream != 0 && next < log_len {
                let base = (next > start).then(|| log[next as usize - 1].pads.as_slice());
                let budget = pw.room().saturating_sub(crate::packet::CHUNK_OVERHEAD);
                let (body, n) = crate::packet::encode_bundles(
                    stream,
                    next,
                    base,
                    &log[next as usize..],
                    budget,
                );
                if n > 0 {
                    pw.bundles(&body);
                } else {
                    oversize = true;
                }
            }
        });
        if streaming {
            sent_through = log_len;
        }
        peer.sent_through = sent_through;
        let addr = peer.conn.addr;
        self.oversize_bundles += u64::from(oversize);
        out.extend(packets.into_iter().map(|p| (addr, p)));
    }

    /// The net meter.
    pub fn stats(&self) -> HostStats {
        // Slot 0's, or while its pad is away the playing host local's.
        let (local_depth, local_starved) = self.host_pacing();
        HostStats {
            tick: self.round.next_tick(),
            local_depth,
            local_starved,
            local_slack_us: self.host_slack_us(),
            peers: self
                .peers
                .values()
                .filter_map(|peer| {
                    let Phase::Joined(slot) = peer.phase else {
                        return None;
                    };
                    let p = self.players.get(&slot)?;
                    Some(PeerStats {
                        slot,
                        name: p.name.clone(),
                        rtt_ms: peer.conn.rtt.srtt as f64 / 1e3,
                        rttvar_ms: peer.conn.rtt.rttvar as f64 / 1e3,
                        silence_ms: peer.conn.silence(self.now).as_millis_f64(),
                        trouble: peer.trouble,
                        queue_depth: p.queue.depth,
                        starved: p.queue.starved,
                        slips: p.queue.slips,
                        slack_us: p.queue.slack_us(),
                        link: peer.conn.stats,
                    })
                })
                .collect(),
            drops: self.drops,
            oversize_bundles: self.oversize_bundles,
        }
    }

    /// The queue depth (before the pop) at the last close for `slot`.
    pub fn queue_depth(&self, slot: u8) -> Option<u8> {
        self.players.get(&slot).map(|p| p.queue.depth)
    }

    /// `slot`'s pacing report (depth, starvations), as a client gets it; the
    /// host game feeds slot 0's to its own [`crate::InputPacer`].
    pub fn pacing(&self, slot: u8) -> Option<(u8, u32)> {
        self.players
            .get(&slot)
            .map(|p| (p.queue.depth, p.queue.starved))
    }

    /// `slot`'s published input slack in microseconds (`scheduler`), as a
    /// client gets it.
    pub fn slack_us(&self, slot: u8) -> Option<i64> {
        self.players.get(&slot).and_then(|p| p.queue.slack_us())
    }

    /// Whether `slot` is a human in the current round's bundles.
    pub fn is_active(&self, slot: u8) -> bool {
        self.players.get(&slot).is_some_and(|p| p.active)
    }

    pub(crate) fn roster(&self) -> Vec<RosterEntry> {
        self.players
            .iter()
            .filter(|(_, p)| p.host_local || p.conn.is_some())
            .map(|(s, p)| RosterEntry {
                slot: *s,
                name: p.name.clone(),
                profile: p.profile.clone(),
            })
            .collect()
    }
}
