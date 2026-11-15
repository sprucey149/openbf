//! [`Peer`]: one machine's lockstep driver, the same for the windowed loop
//! and the headless soaks (LAN §4.3-§4.7).
//!
//! Per frame the caller does three thin things:
//! 1. [`Peer::poll`] drains the session's events. A [`Notice::Load`] asks it
//!    to build the round's `WalkSim` from the [`RoundSetup`] (setting
//!    [`WalkSim::local_slot`] is [`Peer::round_built`]'s job).
//! 2. [`Peer::round_built`] once the walk is built: the host waits for every
//!    guest's `Loaded` and its own build before `Start`; a round-start guest
//!    reports `Loaded` (static digest + tick-0 hash); a joiner starts
//!    catching up.
//! 3. [`Peer::frame`] each rendered frame: the frame's pad goes through the
//!    [`InputPacer`] to the session, then due bundles are stepped
//!    ([`super::step`]).
//!
//! Rules:
//! - The host plays its own bundles as they close: every queued bundle each
//!   frame. A guest's [`Playback`] keeps the jitter buffer. A joiner steps
//!   at most [`Peer::catch_up_budget`] bundles a frame until the round log
//!   is stepped and at most 2 bundles wait, then reports `CaughtUp`.
//! - After a step whose tick is a multiple of [`HASH_EVERY`], the peer
//!   reports `Sim::hash` (the host's core compares).
//! - The host's game policy lives here too: start when everyone loaded,
//!   `AddHuman` (auto team) on a guest's `CaughtUp`, guests' requests
//!   checked against the cheat policy ([`super::validate_request_bytes`]).
//! - A hash mismatch is first the host's question (issue #168): did its
//!   own game part? With a [`Peer::rebuild`] it replays its round to the
//!   mismatch tick ([`super::selfcheck`], one check at a time, off the
//!   frame on a worker thread in the window): from its latest checkpoint,
//!   or from the setup and the whole bundle log without one.
//!   - Checkpoints: every [`CHECKPOINT_EVERY`] ticks while guests are
//!     connected the host keeps a copy of its game as a candidate; when
//!     every live guest's hash agreed on that tick (`HashesAgreed`) the
//!     candidate becomes the checkpoint. At most one of each is held.
//!     Tick 0 is a candidate too (its checkpoint spares a check the map
//!     load), so a check from the setup is mostly the confirming one below.
//!     A mismatch at tick T drops a candidate of tick T or later (it may
//!     be the parted game's), and none is taken or promoted while a check
//!     runs.
//!   - At the mismatch the host takes its own rows for the reports and asks
//!     the guest for its rows ([`PeerNotice::SendRows`]): a slow check must
//!     not find them gone from either ring. The guest keeps a copy of what
//!     it sent for its own report.
//!   - Guests that differ while it runs wait for its verdict; a guest that
//!     is rejoining is not checked again until it has caught up.
//!   - A check that has no verdict by [`CHECK_BASE_TICKS`] (plus
//!     [`CHECK_BUILD_TICKS`] when it builds the round) plus its replay span
//!     over [`CHECK_REPLAY_SPEED`], or that fails (a build error, a broken
//!     log, a lost worker), falls back to the old rule: every guest that
//!     differed is told, with the normal strike rule.
//!   - The replay agrees with the host: each guest that differed at that
//!     tick parted. It is told ([`HostCommand::TellDesync`]), once. A
//!     checkpoint's agreement is proven by `Sim::hash` only, and the copy
//!     also holds what the hash does not cover but the sim reads (the
//!     feed's round points the class gate turns into `Sim::spawn_points`):
//!     a host that parted there before the checkpoint replays its own
//!     parting. So a replay from a checkpoint that agrees is confirmed
//!     first: the checkpoint and the candidate are dropped and the check
//!     runs again from the setup (once: that one starts at tick 0), and
//!     only its verdict is acted on.
//!   - It does not: the host's game parted. The replay, stepped on to the
//!     host's tick, takes the live game's place ([`WalkSim::adopt_round`],
//!     [`FrameReport::corrected`]); the host writes a report
//!     (`host_diverged_at`), raises [`Notice::HostDiverged`] and tells every
//!     guest once ([`PeerNotice::HostCorrected`]). No guest is told of a
//!     desync, except one whose hash also differs from the replay's. Hash
//!     checks of the ticks before the correction are not compared again.
//!   - A second host correction in one round ends the round for everyone
//!     (`EndRound`, [`Notice::RoundAbandoned`],
//!     [`PeerNotice::RoundAbandoned`]) instead of going on out of step; the
//!     window takes everyone to the round's results.
//!   - A check whose round is over (the peer no longer playing) is dropped,
//!     and a mismatch then is not checked or told.
//!   - Without a rebuild every mismatch is the guest's (the old rule).
//! - A guest told of a desync asks for the round again (it rejoins). A
//!   second desync within [`DESYNC_WINDOW_S`] of play in the same round
//!   (LAN §4.7) does not: the guest leaves the round for the lobby
//!   ([`Notice::DesyncAgain`]) and tells the host
//!   ([`GameRequest::LeftRound`]), whose [`Notice::LeftRound`] names the
//!   first SHASH groups that differed. A new round starts the count over.
//! - One start moment (protocol 8): `Start` names how long until tick 0
//!   ([`Notice::Started`]'s `begin_in_s`: the host's lead, a guest's less
//!   its half round trip). A built peer waits ([`Mode::Waiting`]) that
//!   long of its frames' time, or until its caller ends the wait at its own
//!   clock ([`Peer::begin_play`], the window at the end of its loading
//!   screen), and only then sends inputs and steps bundles. A peer built
//!   after the moment (the barrier's timeout started without it) plays at
//!   once, behind.
//! - Another round's messages are stale: the host takes its new round's id
//!   at [`Peer::host_load`] (a `Built` or a desync of the old round is
//!   dropped) and steps no bundle before the new round starts.
//! - A guest's static digest or tick-0 hash that differs from the host's is
//!   a [`Notice::DigestMismatch`], also when it arrives before the host's
//!   own build (then it is compared at [`Peer::round_built`]). A round-start
//!   guest's come in `Loaded`; a mid-round joiner's in
//!   [`GameRequest::Built`] (its catch-up must not look live).
//! - A guest that cannot build its round says so
//!   ([`Peer::leave_round_for_lobby`]), so the host's barrier stops waiting.
//! - Pending inputs (sent, not yet in a stepped bundle; the look preview and
//!   the prediction run over them) are numbered as the session numbers them
//!   (1 = the first submitted). Each stepped bundle names the newest of the
//!   local slot's inputs the host consumed through its tick
//!   ([`TickBundle::consumed`]): those through it are dropped, exactly, on
//!   the host and on a guest alike, whatever the host's queue starved,
//!   slipped or merged; the inputs in flight when the slot joined go with
//!   the first input consumed after them.
//! - Sim first (LAN §8b.1): the frame time `dt` is the frame's whole wall
//!   time (up to 1 s), so a late frame steps every bundle that came due
//!   while it was late before anything draws. Only the input clock is held
//!   to [`MAX_INPUT_FRAME_S`] a frame: a stall's time is not turned into a
//!   burst of repeated inputs, which would stand in the host's queue as
//!   input delay for every later tick.
//! - Desync reports ([`super::desync`]): every stepped tick's hash row goes
//!   into a ring and every bundle into the round's log; on a desync both
//!   sides write a report into [`Peer::report_dir`] (the guest sends its
//!   rows to the host first), announced by [`Notice::DesyncReport`].
//! - Nothing here reads a clock: frame times are the caller's.

use super::desync::{self, BundleLog, DesyncReport, HashRing, Side};
pub use super::link::{Link, LinkEvent};
use super::{
    validate_request_bytes, Applied, GameCommand, GameRequest, LeftWhy, Refusal, RoundSetup,
};
use crate::event_feed::EventFeed;
use crate::walk::WalkSim;
use core::fmath::FloatExt;
use core::sim::PadState;
use net::client::DisconnectReason;
use net::{
    ClientEvent, HostCommand, InputPacer, LeaveReason, NetEvent, Playback, RejectReason,
    TickBundle, TEAM_AUTO,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

/// Ticks between hash reports.
pub const HASH_EVERY: u32 = 30;
/// A joiner's default steps per frame while catching up.
pub const CATCH_UP_BUDGET: usize = 4000;
/// The most input-clock time one frame advances (module rules): a longer
/// frame is a stall, and the host already repeated the inputs it missed.
pub const MAX_INPUT_FRAME_S: f64 = 0.1;
/// Local inputs kept for look preview and prediction.
const PENDING_KEPT: usize = 64;
/// A guest's second desync this soon after its first (seconds of play)
/// in the same round sends it to the lobby instead of rejoining again
/// (LAN §4.7).
pub const DESYNC_WINDOW_S: f64 = 300.0;
/// Host corrections in one round that end it (module rules).
pub const MAX_HOST_CORRECTIONS: u32 = 2;
/// Ticks a self-check may take before the host gives up on it (module
/// rules): this much (10 s) always,
pub const CHECK_BASE_TICKS: u32 = 300;
/// this much more (30 s) when it builds the round (a map load in the
/// window),
pub const CHECK_BUILD_TICKS: u32 = 900;
/// and the ticks it replays over this: a worker steps a round many times
/// faster than play (about 0.5 ms a tick against 33 ms, measured on the
/// bf2 maps), so 4 leaves wide room.
pub const CHECK_REPLAY_SPEED: u32 = 4;
/// Ticks between the host's checkpoint candidates (module rules; 10 s, a
/// multiple of [`HASH_EVERY`]).
pub const CHECKPOINT_EVERY: u32 = 300;

/// The peers' own messages on the host's notice channel (never a
/// [`super::GameNotice`]: the window does not see them).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PeerNotice {
    /// The host's game parted at check `tick` of round `round_id` and was
    /// corrected from the round's log (module rules).
    HostCorrected { round_id: u32, tick: u32 },
    /// The host ended round `round_id`: it could not keep its game in step.
    RoundAbandoned { round_id: u32 },
    /// The host asks for this guest's rows at check `tick` of round
    /// `round_id` (`Msg::DesyncHashes`), before it knows whose game parted.
    SendRows { round_id: u32, tick: u32 },
}

impl PeerNotice {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("notice encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// A guest's desync that counts toward [`DESYNC_WINDOW_S`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct Strike {
    round_id: u32,
    tick: u32,
    /// [`Peer::clock_s`] then.
    at_s: f64,
}

/// Where the peer is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub enum Mode {
    /// No round.
    #[default]
    Idle,
    /// A `LoadRound` waits for the caller's build.
    Loading,
    /// Built; waiting for the round to start.
    Waiting,
    /// A joiner stepping the round log.
    CatchingUp,
    Playing,
    /// `EndRound` stepped.
    Ended,
    Disconnected,
}

/// Something the caller (the HUD, a harness) may want to show or act on.
#[derive(Clone, Debug)]
pub enum Notice {
    Welcome {
        slot: u8,
    },
    Rejected {
        reason: RejectReason,
        detail: String,
    },
    /// Build this round, then call [`Peer::round_built`].
    Load {
        round_id: u32,
        joining: bool,
        setup: Box<RoundSetup>,
    },
    /// The host sent a setup this build cannot read.
    BadSetup(String),
    /// The round starts: tick 0 begins in `begin_in_s` (module rules).
    Started {
        begin_in_s: f64,
    },
    /// A guest finished its catch-up; the host scheduled its `AddHuman`.
    GuestCaughtUp {
        slot: u8,
        tick: u32,
    },
    PlayerJoined {
        slot: u8,
        name: String,
        reconnect: bool,
    },
    PlayerLeft {
        slot: u8,
        reason: LeaveReason,
    },
    /// A guest's copy builds a different round ("plays differently"):
    /// its static digest, or (`start`) its tick-0 `Sim::hash` with equal
    /// static digests.
    DigestMismatch {
        slot: u8,
        theirs: u64,
        ours: u64,
        start: bool,
    },
    Desync {
        slot: u8,
        tick: u32,
    },
    /// This guest's second desync within [`DESYNC_WINDOW_S`]: it left the
    /// round for the lobby (and told the host).
    DesyncAgain {
        tick: u32,
    },
    /// The host's own game parted at check `tick` and was corrected from
    /// the round's log (module rules). On the host `slots` are the guests
    /// whose hash agreed with the replay's; a guest's is empty.
    HostDiverged {
        tick: u32,
        slots: Vec<u8>,
    },
    /// The host ended the round (module rules): its game parted a second
    /// time (`detail`, the host's; a guest's is empty).
    RoundAbandoned {
        tick: u32,
        detail: String,
    },
    /// The host's self-check at `tick` gave no verdict (module rules): the
    /// guests that differed were told, as without one.
    SelfCheckFailed {
        tick: u32,
        detail: String,
    },
    /// The host: guest `slot` left the round for the lobby on its own;
    /// `groups`: the SHASH groups of its last desync, when compared.
    LeftRound {
        slot: u8,
        why: LeftWhy,
        groups: Vec<&'static str>,
    },
    /// A desync report was written there.
    DesyncReport(PathBuf),
    /// A desync report could not be written.
    DesyncReportFailed(String),
    Refused {
        slot: u8,
        why: Refusal,
    },
    Lobby(Vec<u8>),
    Message(Vec<u8>),
    /// A guest confirmed its last results page (LAN §4.8).
    PeerReady {
        slot: u8,
    },
    Chat {
        slot: u8,
        text: String,
    },
    Disconnected(DisconnectReason),
}

/// What one [`Peer::frame`] did.
#[derive(Clone, Debug, Default)]
pub struct FrameReport {
    pub ticks: u32,
    /// Bundles still queued.
    pub behind: usize,
    pub mode: Mode,
    /// The render blend's fraction toward the next tick.
    pub blend: f64,
    /// The commands stepped, by tick.
    pub applied: Vec<(u32, Applied)>,
    /// This frame reported `CaughtUp` at this tick.
    pub caught_up: Option<u32>,
    /// The input ticks this frame sent, oldest first.
    pub submitted: Vec<PadState>,
    /// The host: its game was replaced by its replay this frame (the
    /// check tick); the caller's views of the old one start over.
    pub corrected: Option<u32>,
}

/// The peer's counters, for the net meter and the soak's summary.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct PeerStats {
    pub ticks_stepped: u64,
    pub hash_reports: u64,
    pub desyncs: u32,
    /// The host: self-checks run, and those that found its own game
    /// parted (and corrected it).
    pub host_checks: u32,
    pub host_divergences: u32,
    /// The host: checks with no verdict (module rules).
    pub host_check_failures: u32,
    /// The host: checks from a checkpoint that agreed, run again from the
    /// setup to confirm (module rules; counted in `host_checks` too).
    pub host_confirm_checks: u32,
    /// The host's last check: the tick its replay started from (0: no
    /// checkpoint), its check tick, and the host ticks it took.
    pub last_check_from: u32,
    pub last_check_tick: u32,
    pub last_check_ticks: u32,
    /// The host: checkpoints taken (agreed on).
    pub checkpoints: u32,
    pub digest_mismatches: u32,
    pub refused: u32,
    pub inputs_sent: u64,
    /// Bundles whose tick was not the one expected (a bug if nonzero).
    pub out_of_order: u32,
    /// The last joiner catch-up: ticks stepped and frames taken.
    pub catch_up_ticks: u32,
    pub catch_up_frames: u32,
    /// Desync reports written.
    pub desync_reports: u32,
}

/// A guest as the host knows it.
#[derive(Clone, Debug)]
pub struct Guest {
    pub name: String,
    pub profile: super::PlayerProfile,
    /// `None` when its `Hello` carried no readable manifest.
    pub content: Option<super::ContentManifest>,
}

/// One machine's lockstep driver (module header).
pub struct Peer {
    host: bool,
    pub local_slot: u8,
    mode: Mode,
    pacer: InputPacer,
    playback: Playback,
    queue: VecDeque<TickBundle>,
    next_tick: u32,
    pending: VecDeque<PadState>,
    /// Each pending input's sequence.
    pending_seq: VecDeque<u32>,
    /// Inputs submitted this session.
    sent: u32,
    round_id: u32,
    joining: bool,
    log_len: u32,
    built: bool,
    all_loaded: bool,
    started: bool,
    /// The start's wait still to go, in frame seconds (module rules).
    start_wait: Option<f64>,
    /// Our round's static digest, once built.
    pub static_digest: Option<u64>,
    /// Steps per frame while catching up.
    pub catch_up_budget: usize,
    /// The host's cheat policy for guests' requests.
    pub cheat_policy: u32,
    /// The local slot is a human in the round's bundles.
    active: bool,
    catch_up_frames: u32,
    guests: BTreeMap<u8, Guest>,
    /// Our round's tick-0 `Sim::hash`, once built.
    hash0: Option<u64>,
    /// Guests' static digests and tick-0 hashes that came before the
    /// host's own build.
    loaded_digests: Vec<(u8, u64, u64)>,
    /// Seconds of frames this session (the caller's `dt`s).
    clock_s: f64,
    /// A guest: its last desync that counts (module rules).
    last_desync: Option<Strike>,
    /// The host: how it replays its round for a self-check (module rules);
    /// `None` takes every mismatch for the guest's.
    pub rebuild: Option<super::Rebuild>,
    /// The host: the self-check in flight.
    check: Option<super::selfcheck::Check>,
    /// The host: its latest checkpoint, and the candidate waiting for the
    /// guests' agreement (module rules).
    checkpoint: Option<Box<super::selfcheck::Checkpoint>>,
    candidate: Option<Box<super::selfcheck::Checkpoint>>,
    /// The host: guests' rows that came before their report was made, by
    /// slot.
    early_rows: BTreeMap<u8, GuestRows>,
    /// A guest: what it last sent for [`PeerNotice::SendRows`], for its
    /// own report.
    sent_rows: Option<SentRows>,
    /// The host: hash checks before this tick are from before a correction.
    trust_from: u32,
    /// The host: corrections this round.
    corrections: u32,
    /// The host: this round is being ended (module rules).
    abandoned: bool,
    /// The host: each guest's last desync's differing groups.
    desync_groups: BTreeMap<u8, Vec<&'static str>>,
    /// Notices raised outside [`Peer::poll`], given out by the next one.
    deferred: Vec<Notice>,
    pub stats: PeerStats,
    /// Where desync reports go; `None` writes none (module rules).
    pub report_dir: Option<PathBuf>,
    /// The reports written, oldest first.
    pub desync_reports: Vec<PathBuf>,
    ring: HashRing,
    log: BundleLog,
    setup: Option<RoundSetup>,
    static_groups: Vec<(&'static str, u64)>,
    /// The host's reports waiting for their guest's rows, by slot.
    awaiting: BTreeMap<u8, Pending>,
    /// Guests already reported, until they have caught up again.
    reported: BTreeSet<u8>,
    /// Notices raised outside [`Peer::poll`], given out by the next one.
    later: Vec<Notice>,
}

/// A guest's rows at `tick` (`Msg::DesyncHashes`), as report text.
struct GuestRows {
    tick: u32,
    build: String,
    target: String,
    rows: Vec<String>,
}

/// What a guest sent the host for [`PeerNotice::SendRows`].
struct SentRows {
    round_id: u32,
    tick: u32,
    /// Its hash at `tick`, hex, and its rows, as report text.
    hash: String,
    rows: Vec<String>,
}

/// A host report waiting for its guest's rows, and one hash check more.
struct Pending {
    report: DesyncReport,
    waited: u32,
    /// The guest's rows came.
    rows: bool,
}

impl Peer {
    /// A host's peer (slot 0).
    pub fn host() -> Self {
        Self::new(true, 0)
    }

    /// A guest's peer; its slot comes with `Welcome`.
    pub fn client() -> Self {
        Self::new(false, 0)
    }

    fn new(host: bool, local_slot: u8) -> Self {
        Self {
            host,
            local_slot,
            mode: Mode::Idle,
            pacer: InputPacer::new(),
            playback: Playback::new(),
            queue: VecDeque::new(),
            next_tick: 0,
            pending: VecDeque::new(),
            pending_seq: VecDeque::new(),
            sent: 0,
            round_id: 0,
            joining: false,
            log_len: 0,
            built: false,
            all_loaded: false,
            started: false,
            start_wait: None,
            static_digest: None,
            catch_up_budget: CATCH_UP_BUDGET,
            cheat_policy: 0,
            active: false,
            catch_up_frames: 0,
            guests: BTreeMap::new(),
            hash0: None,
            loaded_digests: Vec::new(),
            clock_s: 0.0,
            last_desync: None,
            rebuild: None,
            check: None,
            checkpoint: None,
            candidate: None,
            early_rows: BTreeMap::new(),
            sent_rows: None,
            trust_from: 0,
            corrections: 0,
            abandoned: false,
            desync_groups: BTreeMap::new(),
            deferred: Vec::new(),
            stats: PeerStats::default(),
            report_dir: Some(desync::default_dir()),
            desync_reports: Vec::new(),
            ring: HashRing::default(),
            log: BundleLog::default(),
            setup: None,
            static_groups: Vec::new(),
            awaiting: BTreeMap::new(),
            reported: BTreeSet::new(),
            later: Vec::new(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn is_host(&self) -> bool {
        self.host
    }

    /// Bundles received and not yet stepped.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The next tick this peer steps.
    pub fn next_tick(&self) -> u32 {
        self.next_tick
    }

    /// Local inputs sent and not yet stepped, oldest first (none while the
    /// local slot is not in the round).
    pub fn pending_inputs(&self) -> &VecDeque<PadState> {
        static NONE: VecDeque<PadState> = VecDeque::new();
        if self.active {
            &self.pending
        } else {
            &NONE
        }
    }

    /// The input pacer (its dilation for the meter).
    pub fn pacer(&self) -> &InputPacer {
        &self.pacer
    }

    /// The playback controller (its target for the meter).
    pub fn playback(&self) -> &Playback {
        &self.playback
    }

    /// The round a guest was last asked to load (its `Ready` names it).
    pub fn round_id(&self) -> u32 {
        self.round_id
    }

    /// Leaves the round in play without leaving the session (the lobby:
    /// a benched guest, the results): nothing more is stepped until the
    /// next `LoadRound`.
    pub fn leave_round(&mut self) {
        self.reset_round();
        self.mode = Mode::Idle;
    }

    /// A guest leaves the round it was given for the lobby on its own
    /// (`why`): the host is told, so it benches this slot at once instead
    /// of holding its load barrier or a seat for it. A guest with no round
    /// (or the host) only leaves locally.
    pub fn leave_round_for_lobby<L: Link>(&mut self, link: &mut L, why: LeftWhy) {
        if !self.host && self.round_id != 0 && self.mode != Mode::Idle {
            let round_id = self.round_id;
            self.request(link, &GameRequest::LeftRound { round_id, why });
        }
        self.leave_round();
    }

    /// Seconds of frames this session ([`DESYNC_WINDOW_S`]'s clock).
    pub fn clock_s(&self) -> f64 {
        self.clock_s
    }
    /// The guests connected to the host, by slot.
    pub fn guests(&self) -> &BTreeMap<u8, Guest> {
        &self.guests
    }

    fn reset_round(&mut self) {
        self.flush_pending();
        self.reported.clear();
        self.ring.clear();
        self.log.clear();
        self.static_groups.clear();
        self.queue.clear();
        self.pending.clear();
        self.pending_seq.clear();
        self.next_tick = 0;
        self.playback = Playback::new();
        self.built = false;
        self.all_loaded = false;
        self.started = false;
        self.start_wait = None;
        self.active = false;
        self.static_digest = None;
        self.hash0 = None;
        self.catch_up_frames = 0;
        self.loaded_digests.clear();
        self.check = None;
        self.checkpoint = None;
        self.candidate = None;
        self.early_rows.clear();
        self.sent_rows = None;
        self.trust_from = 0;
        self.corrections = 0;
        self.abandoned = false;
    }

    /// The host: its corrections this round (module rules).
    pub fn corrections(&self) -> u32 {
        self.corrections
    }

    /// The host: the first tick after its last correction (0 without one).
    pub fn trusted_from(&self) -> u32 {
        self.trust_from
    }

    /// The host: a self-check is running.
    pub fn checking(&self) -> bool {
        self.check.is_some()
    }

    /// The host: the tick of its latest checkpoint (module rules).
    pub fn checkpoint_tick(&self) -> Option<u32> {
        self.checkpoint.as_ref().map(|c| c.tick)
    }

    /// The host starts a round: every guest gets `LoadRound`. Returns the
    /// bytes the host must build its own round from (decode them).
    pub fn host_load<L: Link>(&mut self, link: &mut L, setup: &RoundSetup) -> Vec<u8> {
        let bytes = setup.encode();
        self.reset_round();
        // `HostCore::load_round`'s next id: the old round's messages still
        // on their way are stale from here on (module rules).
        self.round_id = self.round_id.wrapping_add(1).max(1);
        self.setup = Some(setup.clone());
        self.cheat_policy = setup.cheat_policy;
        self.joining = false;
        self.mode = Mode::Loading;
        // The setup's seats are the round's humans (net seats no one else).
        let humans = setup.humans.iter().map(|h| h.slot).collect();
        link.host(HostCommand::LoadRound {
            setup: bytes.clone(),
            humans,
        });
        bytes
    }

    /// The host issues a game command (every peer applies it on one tick).
    pub fn host_command<L: Link>(&mut self, link: &mut L, cmd: &GameCommand) {
        if let GameCommand::SetCheatPolicy { allowed } = cmd {
            self.cheat_policy = *allowed;
        }
        link.host(HostCommand::IssueCommand(cmd.encode()));
    }

    /// A guest asks the host for something.
    pub fn request<L: Link>(&mut self, link: &mut L, req: &super::GameRequest) {
        link.request(req.encode());
    }

    /// Drains the session's events.
    pub fn poll<L: Link>(&mut self, link: &mut L) -> Vec<Notice> {
        let mut out = std::mem::take(&mut self.deferred);
        out.append(&mut self.later);
        while let Some(e) = link.poll() {
            match e {
                LinkEvent::Host(e) => self.host_event(link, e, &mut out),
                LinkEvent::Client(e) => self.client_event(link, e, &mut out),
            }
        }
        out
    }

    fn maybe_start<L: Link>(&mut self, link: &mut L) {
        if self.host && self.built && self.all_loaded && self.mode == Mode::Waiting {
            self.all_loaded = false;
            link.host(HostCommand::StartRound);
        }
    }

    fn host_event<L: Link>(&mut self, link: &mut L, e: NetEvent, out: &mut Vec<Notice>) {
        match e {
            NetEvent::PlayerJoined {
                slot,
                name,
                reconnect,
                content,
                profile,
            } => {
                let guest = Guest {
                    name: name.clone(),
                    profile: super::PlayerProfile::decode_or_default(&profile),
                    content: super::ContentManifest::decode(&content),
                };
                self.guests.insert(slot, guest);
                out.push(Notice::PlayerJoined {
                    slot,
                    name,
                    reconnect,
                });
            }
            NetEvent::PlayerLeft { slot, reason } => {
                if let Some(p) = self.awaiting.remove(&slot) {
                    self.write_report(p.report, out);
                }
                self.reported.remove(&slot);
                self.desync_groups.remove(&slot);
                self.early_rows.remove(&slot);
                self.guests.remove(&slot);
                out.push(Notice::PlayerLeft { slot, reason });
            }
            NetEvent::PeerLoaded {
                slot,
                static_digest,
                hash0,
                ..
            } => self.check_digest(slot, static_digest, hash0, out),
            NetEvent::AllLoaded { round_id } => {
                self.round_id = round_id;
                self.all_loaded = true;
                self.maybe_start(link);
            }
            NetEvent::RoundStarted { round_id, lead_us } => {
                self.round_id = round_id;
                let begin_in_s = self.start(lead_us);
                out.push(Notice::Started { begin_in_s });
            }
            // Before `RoundStarted` a bundle is the old round's (module rules).
            NetEvent::Bundle(b) => {
                if self.started {
                    self.queue.push_back(b);
                }
            }
            NetEvent::PeerCaughtUp { slot, tick, .. } => {
                self.reported.remove(&slot);
                link.host(HostCommand::AddHuman {
                    slot,
                    team: TEAM_AUTO,
                });
                out.push(Notice::GuestCaughtUp { slot, tick });
            }
            NetEvent::Desync {
                round_id,
                slot,
                tick,
                host_hash,
                peer_hash,
            } => self.on_mismatch(link, round_id, slot, tick, host_hash, peer_hash, out),
            NetEvent::HashesAgreed { round_id, tick } => {
                // Agreement on a later tick proves the candidate's too;
                // not while a check runs (module rules).
                let ok = round_id == self.round_id && !self.abandoned && self.check.is_none();
                if let Some(c) = self
                    .candidate
                    .take_if(|c| ok && c.tick <= tick && c.tick >= self.trust_from)
                {
                    self.checkpoint = Some(c);
                    self.stats.checkpoints += 1;
                }
            }
            NetEvent::DesyncHashes {
                slot,
                tick,
                build,
                target,
                first_tick,
                width,
                values,
            } => {
                // The groups that differed, while this peer still holds
                // its rows of those ticks (a later report keeps the last).
                if usize::from(width) == desync::COLUMNS.len() {
                    let theirs: Vec<(u32, desync::Row)> = values
                        .chunks_exact(desync::COLUMNS.len())
                        .enumerate()
                        .map(|(i, v)| (first_tick + i as u32, v.try_into().expect("width 6")))
                        .collect();
                    if let Some((_, cols)) =
                        desync::first_difference(&self.ring.before(tick), &theirs)
                    {
                        let groups: Vec<&'static str> =
                            cols.into_iter().filter(|c| *c != "hash").collect();
                        if !groups.is_empty() {
                            self.desync_groups.insert(slot, groups);
                        }
                    }
                }
                let rows: Vec<String> = if usize::from(width) == desync::COLUMNS.len() {
                    values
                        .chunks_exact(desync::COLUMNS.len())
                        .enumerate()
                        .map(|(i, v)| {
                            let row: desync::Row = v.try_into().expect("width 6");
                            desync::format_row(first_tick + i as u32, &row)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                if let Some(p) = self
                    .awaiting
                    .get_mut(&slot)
                    .filter(|p| p.report.tick == tick && !p.rows)
                {
                    p.rows = true;
                    let g = &mut p.report.guest;
                    (g.build, g.target) = (build, target);
                    if !rows.is_empty() {
                        g.rows = rows;
                    }
                    if p.waited >= HASH_EVERY {
                        let p = self.awaiting.remove(&slot).expect("pending");
                        self.finish_report(p, out);
                    }
                } else if !self.awaiting.contains_key(&slot) {
                    // Asked for at the mismatch: its report is not made yet.
                    self.early_rows.insert(
                        slot,
                        GuestRows {
                            tick,
                            build,
                            target,
                            rows,
                        },
                    );
                }
            }
            NetEvent::Request { slot, data } => match GameRequest::decode(&data) {
                Some(GameRequest::Built {
                    round_id,
                    static_digest,
                    hash0,
                }) => {
                    if round_id == self.round_id {
                        self.check_digest(slot, static_digest, hash0, out);
                    }
                }
                Some(GameRequest::LeftRound { round_id, why }) => {
                    // An older round's is stale (round ids only grow).
                    if round_id >= self.round_id {
                        let groups = self.desync_groups.remove(&slot).unwrap_or_default();
                        out.push(Notice::LeftRound { slot, why, groups });
                    }
                }
                _ => match validate_request_bytes(slot, &data, self.cheat_policy) {
                    Ok(cmd) => link.host(HostCommand::IssueCommand(cmd.encode())),
                    Err(why) => {
                        self.stats.refused += 1;
                        out.push(Notice::Refused { slot, why });
                    }
                },
            },
            NetEvent::Chat { slot, text } => out.push(Notice::Chat { slot, text }),
            NetEvent::PeerReady { slot, .. } => out.push(Notice::PeerReady { slot }),
            NetEvent::Refused { .. } | NetEvent::Trouble { .. } => {}
        }
    }

    fn client_event<L: Link>(&mut self, link: &mut L, e: ClientEvent, out: &mut Vec<Notice>) {
        match e {
            ClientEvent::Welcome { slot, .. } => {
                self.local_slot = slot;
                out.push(Notice::Welcome { slot });
            }
            ClientEvent::Rejected { reason, detail } => {
                self.mode = Mode::Disconnected;
                out.push(Notice::Rejected { reason, detail });
            }
            ClientEvent::Lobby { state, .. } => out.push(Notice::Lobby(state)),
            ClientEvent::LoadRound {
                round_id,
                joining,
                log_len,
                setup,
                ..
            } => {
                self.reset_round();
                self.setup = None;
                self.round_id = round_id;
                self.joining = joining;
                self.log_len = log_len;
                match RoundSetup::decode(&setup) {
                    Ok(s) => {
                        self.setup = Some(s.clone());
                        self.cheat_policy = s.cheat_policy;
                        self.mode = Mode::Loading;
                        out.push(Notice::Load {
                            round_id,
                            joining,
                            setup: Box::new(s),
                        });
                    }
                    Err(e) => {
                        self.mode = Mode::Idle;
                        out.push(Notice::BadSetup(format!("{e:#}")));
                    }
                }
            }
            ClientEvent::Start { begin_in_us, .. } => {
                let begin_in_s = self.start(begin_in_us);
                out.push(Notice::Started { begin_in_s });
            }
            ClientEvent::Bundle(b) => self.queue.push_back(b),
            ClientEvent::Desync {
                tick,
                host_hash,
                host_build,
                host_target,
            } => {
                // Already asking for the round again: a later check of the
                // same divergence.
                if !matches!(self.mode, Mode::Playing | Mode::CatchingUp | Mode::Ended) {
                    return;
                }
                let round_id = self.round_id;
                let sent = self
                    .sent_rows
                    .take()
                    .filter(|s| s.round_id == round_id && s.tick == tick);
                let mut report = self.report(self.local_slot, tick, "guest");
                report.host.hash = format!("{host_hash:016x}");
                (report.host.build, report.host.target) = (host_build, host_target);
                if let Some(sent) = sent {
                    // The host has them already; the ring may not.
                    (report.guest.hash, report.guest.rows) = (sent.hash, sent.rows);
                } else {
                    self.send_rows(link, tick);
                    report.guest.hash = self
                        .ring
                        .get(tick)
                        .map_or_else(String::new, |r| format!("{:016x}", r[0]));
                    report.guest.rows = self.rows_before(tick);
                }
                self.write_report(report, out);
                self.stats.desyncs += 1;
                self.queue.clear();
                let again = self.last_desync.is_some_and(|d| {
                    d.round_id == round_id && self.clock_s - d.at_s < DESYNC_WINDOW_S
                });
                if again {
                    // The same copy parts again: the lobby, not a third
                    // replay of the round (LAN §4.7).
                    self.last_desync = None;
                    self.leave_round_for_lobby(link, LeftWhy::Desynced);
                    out.push(Notice::DesyncAgain { tick });
                } else {
                    self.last_desync = Some(Strike {
                        round_id,
                        tick,
                        at_s: self.clock_s,
                    });
                    let slot = self.local_slot;
                    out.push(Notice::Desync { slot, tick });
                    self.mode = Mode::Idle;
                    link.request_resync();
                }
            }
            ClientEvent::Notice(b) => match PeerNotice::decode(&b) {
                Some(PeerNotice::HostCorrected { round_id, tick }) => {
                    if round_id == self.round_id {
                        out.push(Notice::HostDiverged {
                            tick,
                            slots: Vec::new(),
                        });
                    }
                }
                Some(PeerNotice::RoundAbandoned { round_id }) => {
                    if round_id == self.round_id {
                        out.push(Notice::RoundAbandoned {
                            tick: self.next_tick,
                            detail: String::new(),
                        });
                    }
                }
                Some(PeerNotice::SendRows { round_id, tick }) => {
                    if round_id == self.round_id && self.ring.get(tick).is_some() {
                        self.send_rows(link, tick);
                        self.sent_rows = Some(SentRows {
                            round_id,
                            tick,
                            hash: self
                                .ring
                                .get(tick)
                                .map_or_else(String::new, |r| format!("{:016x}", r[0])),
                            rows: self.rows_before(tick),
                        });
                    }
                }
                None => out.push(Notice::Message(b)),
            },
            ClientEvent::Chat { slot, text } => out.push(Notice::Chat { slot, text }),
            ClientEvent::Trouble(_) => {}
            ClientEvent::Disconnected(r) => {
                self.mode = Mode::Disconnected;
                out.push(Notice::Disconnected(r));
            }
        }
    }

    /// The caller built the round `Notice::Load` (or [`Peer::host_load`])
    /// asked for.
    pub fn round_built<L: Link>(&mut self, link: &mut L, walk: &mut WalkSim) {
        walk.set_local_slot(self.local_slot);
        let digest = walk.sim.static_digest();
        let hash0 = walk.sim.hash();
        self.static_groups = walk.sim.static_digest_groups();
        self.static_digest = Some(digest);
        self.hash0 = Some(hash0);
        self.built = true;
        self.next_tick = 0;
        if self.host {
            self.check_early_digests();
            self.mode = self.built_mode();
            self.maybe_start(link);
        } else if self.joining {
            let round_id = self.round_id;
            self.request(
                link,
                &GameRequest::Built {
                    round_id,
                    static_digest: digest,
                    hash0,
                },
            );
            self.mode = Mode::CatchingUp;
            self.catch_up_frames = 0;
        } else {
            link.loaded(self.round_id, digest, hash0);
            self.mode = self.built_mode();
        }
    }

    /// `Start` heard, its tick 0 `us` away: the wait begins (module rules).
    fn start(&mut self, us: u32) -> f64 {
        self.started = true;
        let begin_in_s = f64::from(us) / 1e6;
        self.start_wait = (us > 0).then_some(begin_in_s);
        if self.mode == Mode::Waiting {
            self.mode = self.built_mode();
        }
        begin_in_s
    }

    /// A built round-start peer's mode: playing once started and its wait
    /// is over.
    fn built_mode(&self) -> Mode {
        if self.started && self.start_wait.is_none() {
            Mode::Playing
        } else {
            Mode::Waiting
        }
    }

    /// The caller's own clock reached the start (module rules): the wait
    /// ends and a built peer plays. Before `Start` it does nothing.
    pub fn begin_play(&mut self) {
        if !self.started {
            return;
        }
        self.start_wait = None;
        if self.mode == Mode::Waiting {
            self.mode = Mode::Playing;
        }
    }

    /// The round being built or played is a mid-round join or a rejoin (no
    /// `Start` follows).
    pub fn joining(&self) -> bool {
        self.joining
    }

    /// The host's own build is done: the guests' digests that came first
    /// are compared now (the next [`Peer::poll`] gives the mismatches).
    fn check_early_digests(&mut self) {
        let mut out = Vec::new();
        for (slot, theirs, hash0) in std::mem::take(&mut self.loaded_digests) {
            self.check_digest(slot, theirs, hash0, &mut out);
        }
        self.deferred.append(&mut out);
    }

    /// The host compares a guest's static digest and tick-0 hash with its
    /// own build's (LAN §4.3), or keeps them until it has one.
    fn check_digest(&mut self, slot: u8, theirs: u64, hash0: u64, out: &mut Vec<Notice>) {
        let (Some(ours), Some(our0)) = (self.static_digest, self.hash0) else {
            self.loaded_digests.push((slot, theirs, hash0));
            return;
        };
        let mismatch = if ours != theirs {
            Some((theirs, ours, false))
        } else {
            (our0 != hash0).then_some((hash0, our0, true))
        };
        if let Some((theirs, ours, start)) = mismatch {
            self.stats.digest_mismatches += 1;
            out.push(Notice::DigestMismatch {
                slot,
                theirs,
                ours,
                start,
            });
        }
    }

    /// One rendered frame of `dt` seconds with the local pad `pad`: the
    /// host's self-check verdict ([`Peer::poll_self_check`]), then inputs
    /// out and due bundles stepped ([`Peer::frame_steps`]). `each` runs
    /// after every step with the bundle stepped and the feed (scored for
    /// the tick): event sounds, effects, the black box.
    pub fn frame<L: Link>(
        &mut self,
        link: &mut L,
        dt: f64,
        pad: PadState,
        walk: &mut WalkSim,
        mut feed: Option<&mut EventFeed>,
        each: impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>),
    ) -> FrameReport {
        let corrected = self.poll_self_check(link, walk, feed.as_deref_mut());
        let mut report = self.frame_steps(link, dt, pad, walk, feed, each);
        report.corrected = corrected;
        report
    }

    /// [`Peer::frame`] without its self-check poll, for a caller that polls
    /// it itself first ([`super::LocalView`]): one poll a frame, so the
    /// caller sees every correction.
    pub(super) fn frame_steps<L: Link>(
        &mut self,
        link: &mut L,
        dt: f64,
        pad: PadState,
        walk: &mut WalkSim,
        mut feed: Option<&mut EventFeed>,
        mut each: impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>),
    ) -> FrameReport {
        let mut report = FrameReport::default();
        self.clock_s += dt;
        if self.mode == Mode::Waiting && self.started {
            let left = self.start_wait.map_or(0.0, |w| w - dt);
            if left <= 0.0 {
                self.begin_play();
            } else {
                self.start_wait = Some(left);
            }
        }
        if self.mode == Mode::Playing {
            let (depth, starved) = link.pacing();
            self.pacer.feedback(depth, starved);
            self.pacer.slack(link.slack_us());
            let view_tick = u32::try_from(walk.sim.tick).unwrap_or(u32::MAX);
            for p in self.pacer.frame(dt.fmin(MAX_INPUT_FRAME_S), pad) {
                link.submit_input(p, view_tick);
                report.submitted.push(p);
                self.stats.inputs_sent += 1;
                self.sent += 1;
                self.pending.push_back(p);
                self.pending_seq.push_back(self.sent);
                if self.pending.len() > PENDING_KEPT {
                    self.pending.pop_front();
                    self.pending_seq.pop_front();
                }
            }
        }
        let steps = match self.mode {
            Mode::Playing if self.host => self.queue.len(),
            Mode::Playing => self
                .playback
                .frame(dt, self.queue.len(), link.jitter_ticks()),
            Mode::CatchingUp => {
                self.catch_up_frames += 1;
                self.queue.len().min(self.catch_up_budget.max(1))
            }
            _ => 0,
        };
        for _ in 0..steps {
            let Some(b) = self.queue.pop_front() else {
                break;
            };
            self.step_one(link, walk, feed.as_deref_mut(), &b, &mut each, &mut report);
            if let Some(c) = self.check.as_mut() {
                c.forward(&b);
            }
            if self.mode == Mode::Ended {
                break;
            }
        }
        if self.mode == Mode::CatchingUp && self.next_tick >= self.log_len && self.queue.len() <= 2
        {
            let tick = self.next_tick.saturating_sub(1);
            link.caught_up(tick);
            self.stats.catch_up_ticks = self.next_tick;
            self.stats.catch_up_frames = self.catch_up_frames;
            self.mode = Mode::Playing;
            report.caught_up = Some(tick);
        }
        if !self.awaiting.is_empty() && report.ticks > 0 {
            let waited: Vec<u8> = self
                .awaiting
                .iter_mut()
                .filter_map(|(s, p)| {
                    p.waited += report.ticks;
                    (p.waited >= desync::GUEST_ROWS_WAIT || (p.rows && p.waited >= HASH_EVERY))
                        .then_some(*s)
                })
                .collect();
            for s in waited {
                let p = self.awaiting.remove(&s).expect("pending");
                let mut later = std::mem::take(&mut self.later);
                self.finish_report(p, &mut later);
                self.later = later;
            }
        }
        report.behind = self.queue.len();
        report.mode = self.mode;
        report.blend = self.playback.blend();
        report
    }

    #[allow(clippy::too_many_arguments)]
    fn step_one<L: Link>(
        &mut self,
        link: &mut L,
        walk: &mut WalkSim,
        mut feed: Option<&mut EventFeed>,
        b: &TickBundle,
        each: &mut impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>),
        report: &mut FrameReport,
    ) {
        if b.tick != self.next_tick {
            self.stats.out_of_order += 1;
        }
        let applied = super::step(walk, b, feed.as_deref_mut(), |w, f| each(w, b, f));
        self.log.push(b);
        // A joiner's catch-up keeps only the rows the ring would hold.
        let row = (self.mode != Mode::CatchingUp
            || b.tick as usize + desync::RING_TICKS >= self.log_len as usize)
            .then(|| desync::row_of(&walk.sim));
        if let Some(row) = row {
            self.ring.push(b.tick, row);
        }
        match b.consumed(self.local_slot) {
            Some(s) => {
                self.active = true;
                while self.pending_seq.front().is_some_and(|q| *q <= s) {
                    self.pending_seq.pop_front();
                    self.pending.pop_front();
                }
            }
            None => self.active = false,
        }
        self.next_tick = b.tick + 1;
        self.stats.ticks_stepped += 1;
        report.ticks += 1;
        if b.tick.is_multiple_of(HASH_EVERY) {
            link.report_hash(b.tick, row.map_or_else(|| walk.sim.hash(), |r| r[0]));
            self.stats.hash_reports += 1;
        }
        // A checkpoint candidate (module rules): only where a check could
        // use one, and never from a game the host is checking.
        if self.host
            && b.tick.is_multiple_of(CHECKPOINT_EVERY)
            && self.rebuild.is_some()
            && self.check.is_none()
            && !self.guests.is_empty()
            && !self.abandoned
        {
            self.candidate = Some(Box::new(super::selfcheck::Checkpoint::of(
                b.tick,
                walk,
                feed.as_deref(),
                &self.ring,
            )));
        }
        if walk.netplay.ended {
            self.mode = Mode::Ended;
        }
        report
            .applied
            .extend(applied.into_iter().map(|a| (b.tick, a)));
    }
}

impl Peer {
    /// A report of a desync of guest `slot` at `tick` with this peer's own
    /// round data; the caller fills the sides' hashes and rows.
    fn report(&self, slot: u8, tick: u32, side: &str) -> DesyncReport {
        let me = Side {
            build: super::build_id(),
            target: super::target(),
            ..Side::default()
        };
        let (host, guest) = if self.host {
            (me, Side::default())
        } else {
            (Side::default(), me)
        };
        let (session, setup, summary) = match &self.setup {
            Some(s) => (
                desync::session_name(s, self.round_id),
                desync::hex(&s.encode()),
                format!("{} {} seed {}", s.map, s.mode, s.seed),
            ),
            None => (
                format!("unknown-r{}", self.round_id),
                String::new(),
                String::new(),
            ),
        };
        DesyncReport {
            format: desync::FORMAT.to_owned(),
            side: side.to_owned(),
            session,
            round_id: self.round_id,
            slot,
            local_slot: self.local_slot,
            tick,
            columns: desync::COLUMNS.iter().map(|c| (*c).to_owned()).collect(),
            host,
            guest,
            static_digest: self
                .static_digest
                .map_or_else(String::new, |d| format!("{d:016x}")),
            static_groups: self
                .static_groups
                .iter()
                .map(|(n, d)| ((*n).to_owned(), format!("{d:016x}")))
                .collect(),
            setup,
            setup_summary: summary,
            bundles: self.log.chunks(),
            bundles_broken: self.log.broken,
            host_diverged_at: None,
            agreeing_guests: Vec::new(),
        }
    }

    /// This peer's rows for a report at `tick`, as text.
    fn rows_before(&self, tick: u32) -> Vec<String> {
        self.ring
            .before(tick)
            .iter()
            .map(|(t, r)| desync::format_row(*t, r))
            .collect()
    }

    /// A guest sends the host its rows at `tick` (`Msg::DesyncHashes`).
    fn send_rows<L: Link>(&self, link: &mut L, tick: u32) {
        let rows = self.ring.before(tick);
        if let Some(first) = rows.first().map(|r| r.0) {
            let values = rows.iter().flat_map(|r| r.1).collect();
            link.desync_hashes(tick, first, desync::COLUMNS.len() as u8, values);
        }
    }

    /// A guest's build: equal to the host's, or `Hello` refused it.
    fn build_of_guest(&self) -> String {
        super::build_id()
    }

    fn write_report(&mut self, report: DesyncReport, out: &mut Vec<Notice>) {
        let Some(dir) = self.report_dir.clone() else {
            return;
        };
        match report.write(&dir) {
            Ok(path) => {
                self.stats.desync_reports += 1;
                self.desync_reports.push(path.clone());
                out.push(Notice::DesyncReport(path));
            }
            Err(e) => out.push(Notice::DesyncReportFailed(format!("{e:#}"))),
        }
    }

    /// Writes every host report still waiting, without its guest's rows.
    fn flush_pending(&mut self) {
        let pending = std::mem::take(&mut self.awaiting);
        let mut later = std::mem::take(&mut self.later);
        for (_, p) in pending {
            self.finish_report(p, &mut later);
        }
        self.later = later;
    }

    /// The host: guest `slot`'s hash at `tick` of round `round_id` was
    /// `peer_hash`, its own `host_hash` (module rules).
    #[allow(clippy::too_many_arguments)]
    fn on_mismatch<L: Link>(
        &mut self,
        link: &mut L,
        round_id: u32,
        slot: u8,
        tick: u32,
        host_hash: u64,
        peer_hash: u64,
        out: &mut Vec<Notice>,
    ) {
        if round_id != self.round_id
            || self.mode != Mode::Playing
            || tick < self.trust_from
            || self.abandoned
            || self.reported.contains(&slot)
        {
            return;
        }
        // A candidate of the tick or later may be the parted game's.
        if self.candidate.as_ref().is_some_and(|c| c.tick >= tick) {
            self.candidate = None;
        }
        if self
            .check
            .as_ref()
            .is_some_and(|c| c.differs.contains_key(&slot))
        {
            return;
        }
        let differ = super::selfcheck::Differ {
            tick,
            host_hash,
            peer_hash,
            host_rows: self.rows_before(tick),
        };
        if self.rebuild.is_some() {
            // Its rows now, before a slow check loses them from its ring.
            link.host(HostCommand::Notice {
                slot: Some(slot),
                data: PeerNotice::SendRows { round_id, tick }.encode(),
            });
        }
        if let Some(c) = self.check.as_mut() {
            c.differs.insert(slot, differ);
            return;
        }
        // From the latest checkpoint before the tick, else from the setup.
        let ready = self.rebuild.is_some() && self.log.len() > tick && !self.log.broken;
        let start = match self.checkpoint.as_ref().filter(|c| c.tick < tick) {
            _ if !ready => None,
            Some(c) => Some(super::selfcheck::Start::Checkpoint(Box::new(c.copy()))),
            None => self.setup.clone().map(super::selfcheck::Start::Setup),
        };
        let Some(start) = start else {
            // Nothing to check against: the guest's (the old rule).
            let rows = Some(differ.host_rows);
            self.blame(link, slot, tick, host_hash, peer_hash, rows, out);
            return;
        };
        self.start_check(start, slot, differ);
    }

    /// The host: a check of guest `slot`'s difference `first` from `start`
    /// (module rules), its deadline from now.
    fn start_check(
        &mut self,
        start: super::selfcheck::Start,
        slot: u8,
        first: super::selfcheck::Differ,
    ) {
        let rebuild = self.rebuild.as_mut().expect("a rebuild");
        let from = start.first_tick();
        let span = self.next_tick.saturating_sub(from);
        let build = match start {
            super::selfcheck::Start::Setup(_) => CHECK_BUILD_TICKS,
            super::selfcheck::Start::Checkpoint(_) => 0,
        };
        let deadline = self.next_tick + CHECK_BASE_TICKS + build + span / CHECK_REPLAY_SPEED;
        self.stats.host_checks += 1;
        let log = self.log.since(from);
        let check = super::selfcheck::Check::start(
            rebuild,
            start,
            self.local_slot,
            log,
            slot,
            first,
            self.next_tick,
            deadline,
        );
        self.check = Some(check);
    }

    /// The host: guest `slot`'s game parted at `tick`. It is told, and the
    /// host's report waits for its rows (once until it has caught up).
    /// `host_rows`: the host's rows taken at the mismatch (`None`: from its
    /// ring now).
    #[allow(clippy::too_many_arguments)]
    fn blame<L: Link>(
        &mut self,
        link: &mut L,
        slot: u8,
        tick: u32,
        host_hash: u64,
        peer_hash: u64,
        host_rows: Option<Vec<String>>,
        out: &mut Vec<Notice>,
    ) {
        if !self.reported.insert(slot) {
            return;
        }
        self.stats.desyncs += 1;
        out.push(Notice::Desync { slot, tick });
        link.host(HostCommand::TellDesync {
            slot,
            tick,
            host_hash,
        });
        // An earlier report still waiting for one more check is written,
        // not replaced.
        if let Some(p) = self.awaiting.remove(&slot) {
            self.finish_report(p, out);
        }
        let mut report = self.report(slot, tick, "host");
        report.host.hash = format!("{host_hash:016x}");
        report.host.rows = host_rows.unwrap_or_else(|| self.rows_before(tick));
        report.guest.hash = format!("{peer_hash:016x}");
        report.guest.build = self.build_of_guest();
        let mut rows = false;
        if let Some(g) = self.early_rows.remove(&slot).filter(|g| g.tick == tick) {
            rows = true;
            (report.guest.build, report.guest.target) = (g.build, g.target);
            if !g.rows.is_empty() {
                report.guest.rows = g.rows;
            }
        }
        self.awaiting.insert(
            slot,
            Pending {
                report,
                waited: 0,
                rows,
            },
        );
    }

    /// The host: runs the self-check's verdict, once it has one (module
    /// rules). Returns the check tick when its game was replaced by the
    /// replay ([`FrameReport::corrected`]). [`Peer::frame`] calls it
    /// first; a caller that must know before the frame's steps (the
    /// prediction, [`super::LocalView`]) calls it itself, then
    /// [`Peer::frame_steps`]. A guest has nothing to poll.
    pub fn poll_self_check<L: Link>(
        &mut self,
        link: &mut L,
        walk: &mut WalkSim,
        feed: Option<&mut EventFeed>,
    ) -> Option<u32> {
        if !self.host {
            return None;
        }
        if self.mode != Mode::Playing {
            // The round is over: nothing to correct.
            self.check = None;
            return None;
        }
        let c = self.check.as_mut()?;
        let verdict = match c.poll() {
            Some(v) => v,
            None if self.next_tick > c.deadline => Err(anyhow::anyhow!(
                "no verdict after {} ticks",
                self.next_tick - c.started_at
            )),
            None => return None,
        };
        let mut check = self.check.take().expect("a check");
        self.stats.last_check_from = check.from;
        self.stats.last_check_tick = check.tick;
        self.stats.last_check_ticks = self.next_tick - check.started_at;
        let mut out = std::mem::take(&mut self.later);
        let at = check.tick;
        let differs = std::mem::take(&mut check.differs);
        let mut corrected = None;
        let verdict = match verdict {
            Ok(super::selfcheck::Verdict::Parted { hash, mut replayed }) => check
                .catch_up(&mut replayed, self.next_tick)
                .map(|()| super::selfcheck::Verdict::Parted { hash, replayed }),
            v => v,
        };
        match verdict {
            Err(e) => {
                // No verdict: every guest that differed is told (the old
                // rule), the round goes on.
                self.stats.host_check_failures += 1;
                out.push(Notice::SelfCheckFailed {
                    tick: at,
                    detail: format!("{e:#}"),
                });
                for (slot, d) in differs {
                    let rows = Some(d.host_rows);
                    self.blame(link, slot, d.tick, d.host_hash, d.peer_hash, rows, &mut out);
                }
            }
            Ok(super::selfcheck::Verdict::Agrees { .. }) if check.from > 0 => {
                // Proven by `Sim::hash` only (module rules): confirmed from
                // the setup before anyone is told. Without a way to build
                // it (no setup, a broken log) the checkpoint's verdict
                // stands, and the checkpoint goes either way.
                self.checkpoint = None;
                self.candidate = None;
                let ready = self.rebuild.is_some() && !self.log.broken && self.log.len() > at;
                let first = differs.iter().find(|(_, d)| d.tick == at).map(|(s, _)| *s);
                match (self.setup.clone().filter(|_| ready), first) {
                    (Some(setup), Some(slot)) => {
                        let mut rest = differs;
                        let d = rest.remove(&slot).expect("the first");
                        self.stats.host_confirm_checks += 1;
                        self.start_check(super::selfcheck::Start::Setup(setup), slot, d);
                        if let Some(c) = self.check.as_mut() {
                            c.differs.extend(rest);
                        }
                    }
                    _ => {
                        for (slot, d) in differs.into_iter().filter(|(_, d)| d.tick == at) {
                            let rows = Some(d.host_rows);
                            self.blame(link, slot, at, d.host_hash, d.peer_hash, rows, &mut out);
                        }
                    }
                }
            }
            Ok(super::selfcheck::Verdict::Agrees { .. }) => {
                // Those that differed only later wait for their next check.
                for (slot, d) in differs.into_iter().filter(|(_, d)| d.tick == at) {
                    let rows = Some(d.host_rows);
                    self.blame(link, slot, at, d.host_hash, d.peer_hash, rows, &mut out);
                }
            }
            Ok(super::selfcheck::Verdict::Parted { hash, replayed }) => {
                let at_check: Vec<(u8, super::selfcheck::Differ)> =
                    differs.into_iter().filter(|(_, d)| d.tick == at).collect();
                let agree: Vec<u8> = at_check
                    .iter()
                    .filter(|(_, d)| d.peer_hash == hash)
                    .map(|(s, _)| *s)
                    .collect();
                // The report holds the live game's rows of the mismatch.
                if let Some((slot, d)) = at_check.first() {
                    let mut r = self.report(*slot, at, "host");
                    r.host.hash = format!("{:016x}", check.live);
                    r.host.rows = d.host_rows.clone();
                    r.guest.hash = format!("{:016x}", d.peer_hash);
                    r.guest.build = self.build_of_guest();
                    if let Some(g) = self.early_rows.get(slot).filter(|g| g.tick == at) {
                        (r.guest.build, r.guest.target) = (g.build.clone(), g.target.clone());
                        r.guest.rows = g.rows.clone();
                    }
                    r.host_diverged_at = Some(at);
                    r.agreeing_guests = agree.clone();
                    self.write_report(r, &mut out);
                }
                let replayed = *replayed;
                walk.adopt_round(replayed.walk);
                if let (Some(f), Some(nf)) = (feed, replayed.feed) {
                    f.adopt(nf);
                }
                self.ring = replayed.ring;
                // The candidate was the old game's; the checkpoint stays
                // (it is from before the parting).
                self.candidate = None;
                self.trust_from = self.next_tick;
                self.corrections += 1;
                self.stats.host_divergences += 1;
                corrected = Some(at);
                out.push(Notice::HostDiverged {
                    tick: at,
                    slots: agree.clone(),
                });
                let round_id = self.round_id;
                link.host(HostCommand::Notice {
                    slot: None,
                    data: PeerNotice::HostCorrected { round_id, tick: at }.encode(),
                });
                for s in agree {
                    self.early_rows.remove(&s);
                }
                // A guest that differs from the replay too parted itself.
                for (slot, d) in at_check.into_iter().filter(|(_, d)| d.peer_hash != hash) {
                    self.blame(link, slot, at, hash, d.peer_hash, None, &mut out);
                }
                if self.corrections >= MAX_HOST_CORRECTIONS {
                    let detail = format!("the host's game parted {} times", self.corrections);
                    self.abandon(link, at, detail, &mut out);
                }
            }
        }
        self.later = out;
        corrected
    }

    /// The host ends the round for everyone (module rules): every peer
    /// steps `EndRound` and raises [`Notice::RoundAbandoned`] with
    /// `detail` (the host's) at its next poll.
    pub fn abandon_round<L: Link>(&mut self, link: &mut L, detail: String) {
        let mut out = std::mem::take(&mut self.later);
        self.abandon(link, self.next_tick, detail, &mut out);
        self.later = out;
    }

    fn abandon<L: Link>(&mut self, link: &mut L, tick: u32, detail: String, out: &mut Vec<Notice>) {
        if self.abandoned || !self.host {
            return;
        }
        self.abandoned = true;
        self.check = None;
        self.candidate = None;
        link.host(HostCommand::EndRound);
        let round_id = self.round_id;
        link.host(HostCommand::Notice {
            slot: None,
            data: PeerNotice::RoundAbandoned { round_id }.encode(),
        });
        out.push(Notice::RoundAbandoned { tick, detail });
    }

    /// Writes a host report.
    fn finish_report(&mut self, p: Pending, out: &mut Vec<Notice>) {
        self.write_report(p.report, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #168, first VPN playtest: a guest's strikes (LAN §4.7) count
    /// per round, and the host's word that its own game was corrected is a
    /// line for this round only, never a strike.
    #[test]
    fn a_guests_strikes_are_per_round_and_the_hosts_correction_is_no_strike() {
        let host: std::net::SocketAddr = ([10, 0, 0, 1], net::DEFAULT_PORT).into();
        let mut core =
            net::ClientCore::new(net::ClientConfig::new("b", "G", 1), host, net::Micros::ZERO);
        let mut peer = Peer::client();
        peer.report_dir = None;
        let setup = RoundSetup::instant_action(
            "bf2_kas2",
            "con",
            ["rep".into(), "cis".into()],
            &ui::shell::SessionOptions::default(),
            &crate::profile::Profile::default(),
        )
        .encode();
        let mut walk = WalkSim::new(1, glam::Vec3::ZERO, None);
        let mut load = |peer: &mut Peer, core: &mut net::ClientCore, round_id, joining| {
            let mut out = Vec::new();
            peer.client_event(
                core,
                ClientEvent::LoadRound {
                    round_id,
                    joining,
                    log_len: 0,
                    setup: setup.clone(),
                    roster: Vec::new(),
                },
                &mut out,
            );
            peer.round_built(core, &mut walk);
            if !joining {
                peer.client_event(
                    core,
                    ClientEvent::Start {
                        round_id,
                        begin_in_us: 0,
                    },
                    &mut out,
                );
            }
            assert!(matches!(peer.mode(), Mode::Playing | Mode::CatchingUp));
        };
        let event = |peer: &mut Peer, core: &mut net::ClientCore, e| {
            let mut out = Vec::new();
            peer.client_event(core, e, &mut out);
            out.into_iter()
                .filter(|n| !matches!(n, Notice::DesyncReport(_)))
                .collect::<Vec<_>>()
        };
        let desync = |tick| ClientEvent::Desync {
            tick,
            host_hash: 1,
            host_build: "b".into(),
            host_target: String::new(),
        };
        let corrected = |round_id, tick| {
            ClientEvent::Notice(PeerNotice::HostCorrected { round_id, tick }.encode())
        };
        let ended =
            |round_id| ClientEvent::Notice(PeerNotice::RoundAbandoned { round_id }.encode());
        // The first notice of each event, by name.
        let first = |n: Vec<Notice>| {
            n.first()
                .map_or_else(|| "none".to_owned(), |n| format!("{n:?}"))
        };
        let rejoin = "Desync { slot: 0, tick: ";
        // Round 1: one desync late in the round, a rejoin.
        load(&mut peer, &mut core, 1, false);
        assert!(first(event(&mut peer, &mut core, desync(3000))).starts_with(rejoin));
        // Round 2: one early in it is the round's first, a rejoin.
        load(&mut peer, &mut core, 2, false);
        assert!(first(event(&mut peer, &mut core, desync(30))).starts_with(rejoin));
        // A second in the same round: the lobby.
        load(&mut peer, &mut core, 2, true);
        assert_eq!(
            first(event(&mut peer, &mut core, desync(60))),
            "DesyncAgain { tick: 60 }"
        );
        // Round 3: the host corrected its own game; a line, no strike.
        load(&mut peer, &mut core, 3, false);
        assert_eq!(
            first(event(&mut peer, &mut core, corrected(3, 30))),
            "HostDiverged { tick: 30, slots: [] }"
        );
        assert_eq!(first(event(&mut peer, &mut core, corrected(2, 60))), "none");
        assert_eq!(first(event(&mut peer, &mut core, ended(2))), "none");
        assert!(first(event(&mut peer, &mut core, desync(90))).starts_with(rejoin));
        load(&mut peer, &mut core, 3, true);
        assert_eq!(
            first(event(&mut peer, &mut core, desync(120))),
            "DesyncAgain { tick: 120 }"
        );
        load(&mut peer, &mut core, 4, false);
        assert!(first(event(&mut peer, &mut core, ended(4))).starts_with("RoundAbandoned"));
    }

    /// Third review of issue #168: a mismatch at tick T drops a checkpoint
    /// candidate of tick T or later (it may be the parted game's) and keeps
    /// an older one; no candidate is promoted while a check runs; and a
    /// mismatch once the round is over is neither checked nor told.
    #[test]
    fn a_mismatch_drops_a_later_candidate_and_a_check_holds_promotion() {
        let mut core = net::HostCore::new(net::HostConfig::new("b", "Host"));
        let walk = WalkSim::new(1, glam::Vec3::ZERO, None);
        let candidate = |tick| {
            Some(Box::new(super::super::selfcheck::Checkpoint::of(
                tick,
                &walk,
                None,
                &HashRing::default(),
            )))
        };
        let desync = |tick| NetEvent::Desync {
            round_id: 1,
            slot: 1,
            tick,
            host_hash: 1,
            peer_hash: 2,
        };
        let mut peer = Peer::host();
        peer.report_dir = None;
        (peer.round_id, peer.mode) = (1, Mode::Playing);
        let mut out = Vec::new();
        // Without a rebuild every mismatch is the guest's: told at once.
        peer.candidate = candidate(300);
        peer.host_event(&mut core, desync(300), &mut out);
        assert!(peer.candidate.is_none(), "a candidate of the tick goes");
        assert_eq!(peer.stats.desyncs, 1);
        peer.reported.clear();
        peer.candidate = candidate(300);
        peer.host_event(&mut core, desync(330), &mut out);
        assert_eq!(peer.candidate.as_ref().map(|c| c.tick), Some(300));
        // A check in flight (its build fails; its verdict not yet polled).
        peer.reported.clear();
        peer.rebuild = Some(super::super::Rebuild::Inline(Box::new(
            |_: &RoundSetup, _: u8| anyhow::bail!("no build here"),
        )));
        peer.setup = Some(super::super::tests::setup());
        for tick in 0..=400 {
            peer.log.push(&TickBundle {
                tick,
                ..TickBundle::default()
            });
        }
        peer.host_event(&mut core, desync(360), &mut out);
        assert!(peer.checking());
        peer.host_event(
            &mut core,
            NetEvent::HashesAgreed {
                round_id: 1,
                tick: 390,
            },
            &mut out,
        );
        assert!(peer.checkpoint.is_none(), "no promotion while checking");
        assert_eq!(peer.candidate.as_ref().map(|c| c.tick), Some(300));
        // The round over: nothing checked, nobody told.
        peer.check = None;
        peer.reported.clear();
        peer.mode = Mode::Ended;
        let desyncs = peer.stats.desyncs;
        out.clear();
        peer.host_event(&mut core, desync(390), &mut out);
        assert!(!peer.checking());
        assert_eq!(peer.stats.desyncs, desyncs);
        assert!(out.is_empty(), "{out:?}");
    }

    /// A guest's `Loaded` before the host's own build is still checked: the
    /// mismatch comes with the first poll after the host built its round.
    #[test]
    fn a_digest_that_arrives_before_the_hosts_build_is_still_checked() {
        let mut core = net::HostCore::new(net::HostConfig::new("b", "Host"));
        let mut peer = Peer::host();
        let mut out = Vec::new();
        for (slot, digest) in [(1, 5), (2, 7)] {
            peer.host_event(
                &mut core,
                NetEvent::PeerLoaded {
                    slot,
                    round_id: 1,
                    static_digest: digest,
                    hash0: 0,
                },
                &mut out,
            );
        }
        assert!(out.is_empty(), "nothing to compare with yet");
        (peer.static_digest, peer.hash0) = (Some(7), Some(0));
        peer.check_early_digests();
        let n = peer.poll(&mut core);
        assert!(
            matches!(
                n.as_slice(),
                [Notice::DigestMismatch {
                    slot: 1,
                    theirs: 5,
                    ours: 7,
                    start: false,
                }]
            ),
            "{n:?}"
        );
        assert_eq!(peer.stats.digest_mismatches, 1);
        assert!(peer.poll(&mut core).is_empty(), "said once");
    }
}
