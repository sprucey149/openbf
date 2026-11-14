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
//! - A guest told of a desync asks for the round again (it rejoins). A
//!   second desync within [`DESYNC_WINDOW_S`] of play (LAN §4.7) does not:
//!   the guest leaves the round for the lobby ([`Notice::DesyncAgain`]) and
//!   tells the host ([`GameRequest::LeftRound`]), whose
//!   [`Notice::LeftRound`] names the first SHASH groups that differed.
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
/// sends it to the lobby instead of rejoining again (LAN §4.7).
pub const DESYNC_WINDOW_S: f64 = 300.0;

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
    Started,
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
}

/// The peer's counters, for the net meter and the soak's summary.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct PeerStats {
    pub ticks_stepped: u64,
    pub hash_reports: u64,
    pub desyncs: u32,
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
    /// A guest: when ([`Peer::clock_s`]) it last desynced and rejoined.
    last_desync_s: Option<f64>,
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

/// A host report waiting for its guest's rows.
struct Pending {
    report: DesyncReport,
    waited: u32,
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
            static_digest: None,
            catch_up_budget: CATCH_UP_BUDGET,
            cheat_policy: 0,
            active: false,
            catch_up_frames: 0,
            guests: BTreeMap::new(),
            hash0: None,
            loaded_digests: Vec::new(),
            clock_s: 0.0,
            last_desync_s: None,
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
        self.active = false;
        self.static_digest = None;
        self.hash0 = None;
        self.catch_up_frames = 0;
        self.loaded_digests.clear();
    }

    /// The host starts a round: every guest gets `LoadRound`. Returns the
    /// bytes the host must build its own round from (decode them).
    pub fn host_load<L: Link>(&mut self, link: &mut L, setup: &RoundSetup) -> Vec<u8> {
        let bytes = setup.encode();
        self.reset_round();
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
            NetEvent::RoundStarted { round_id } => {
                self.round_id = round_id;
                self.started = true;
                if self.built {
                    self.mode = Mode::Playing;
                }
                out.push(Notice::Started);
            }
            NetEvent::Bundle(b) => self.queue.push_back(b),
            NetEvent::PeerCaughtUp { slot, tick, .. } => {
                self.reported.remove(&slot);
                link.host(HostCommand::AddHuman {
                    slot,
                    team: TEAM_AUTO,
                });
                out.push(Notice::GuestCaughtUp { slot, tick });
            }
            NetEvent::Desync {
                slot,
                tick,
                host_hash,
                peer_hash,
            } => {
                self.stats.desyncs += 1;
                out.push(Notice::Desync { slot, tick });
                if self.reported.insert(slot) {
                    let mut report = self.report(slot, tick, "host");
                    report.host.hash = format!("{host_hash:016x}");
                    report.host.rows = self.rows_before(tick);
                    report.guest.hash = format!("{peer_hash:016x}");
                    report.guest.build = self.build_of_guest();
                    self.awaiting.insert(slot, Pending { report, waited: 0 });
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
                if self
                    .awaiting
                    .get(&slot)
                    .is_some_and(|p| p.report.tick == tick)
                {
                    let mut p = self.awaiting.remove(&slot).expect("pending");
                    let g = &mut p.report.guest;
                    (g.build, g.target) = (build, target);
                    if usize::from(width) == desync::COLUMNS.len() {
                        g.rows = values
                            .chunks_exact(desync::COLUMNS.len())
                            .enumerate()
                            .map(|(i, v)| {
                                let row: desync::Row = v.try_into().expect("width 6");
                                desync::format_row(first_tick + i as u32, &row)
                            })
                            .collect();
                    }
                    self.write_report(p.report, out);
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
            ClientEvent::Start { .. } => {
                self.started = true;
                if self.mode == Mode::Waiting {
                    self.mode = Mode::Playing;
                }
                out.push(Notice::Started);
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
                let rows = self.ring.before(tick);
                if let Some(first) = rows.first().map(|r| r.0) {
                    let values = rows.iter().flat_map(|r| r.1).collect();
                    link.desync_hashes(tick, first, desync::COLUMNS.len() as u8, values);
                }
                let mut report = self.report(self.local_slot, tick, "guest");
                report.host.hash = format!("{host_hash:016x}");
                (report.host.build, report.host.target) = (host_build, host_target);
                report.guest.hash = self
                    .ring
                    .get(tick)
                    .map_or_else(String::new, |r| format!("{:016x}", r[0]));
                report.guest.rows = self.rows_before(tick);
                self.write_report(report, out);
                self.stats.desyncs += 1;
                self.queue.clear();
                let again = self
                    .last_desync_s
                    .is_some_and(|t| self.clock_s - t < DESYNC_WINDOW_S);
                if again {
                    // The same copy parts again: the lobby, not a third
                    // replay of the round (LAN §4.7).
                    self.last_desync_s = None;
                    self.leave_round_for_lobby(link, LeftWhy::Desynced);
                    out.push(Notice::DesyncAgain { tick });
                } else {
                    self.last_desync_s = Some(self.clock_s);
                    let slot = self.local_slot;
                    out.push(Notice::Desync { slot, tick });
                    self.mode = Mode::Idle;
                    link.request_resync();
                }
            }
            ClientEvent::Notice(b) => out.push(Notice::Message(b)),
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
            self.mode = if self.started {
                Mode::Playing
            } else {
                Mode::Waiting
            };
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
            self.mode = if self.started {
                Mode::Playing
            } else {
                Mode::Waiting
            };
        }
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

    /// One rendered frame of `dt` seconds with the local pad `pad`: inputs
    /// out, due bundles stepped. `each` runs after every step with the
    /// bundle stepped and the feed (scored for the tick): event sounds,
    /// effects, the black box.
    pub fn frame<L: Link>(
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
                    (p.waited >= desync::GUEST_ROWS_WAIT).then_some(*s)
                })
                .collect();
            for s in waited {
                let p = self.awaiting.remove(&s).expect("pending");
                let mut later = std::mem::take(&mut self.later);
                self.write_report(p.report, &mut later);
                self.later = later;
            }
        }
        report.behind = self.queue.len();
        report.mode = self.mode;
        report.blend = self.playback.blend();
        report
    }

    fn step_one<L: Link>(
        &mut self,
        link: &mut L,
        walk: &mut WalkSim,
        feed: Option<&mut EventFeed>,
        b: &TickBundle,
        each: &mut impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>),
        report: &mut FrameReport,
    ) {
        if b.tick != self.next_tick {
            self.stats.out_of_order += 1;
        }
        let applied = super::step(walk, b, feed, |w, f| each(w, b, f));
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
            self.write_report(p.report, &mut later);
        }
        self.later = later;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
