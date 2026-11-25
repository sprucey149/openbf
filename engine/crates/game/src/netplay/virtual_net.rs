//! Several peers in one process on a [`LinkSim`], in virtual time (LAN §9):
//! the `--net-soak` harness and the in-process tests. Every peer is a real
//! [`Peer`] over a bare [`HostCore`] or [`ClientCore`], with its own
//! `WalkSim` built by the caller's builder from the round setup it got.
//!
//! Rules:
//! - Time advances 1 ms per [`VirtualNet::step`]; each node renders frames at
//!   its own interval. Per millisecond: deliveries, then each node's due
//!   frames (host first), then each core's update. Seeds fix everything, so a
//!   run repeats exactly.
//! - Every step of every peer checks `Sim::hash` against the first hash any
//!   peer produced for that tick ([`HashChecker`]); a rejoined peer's replay
//!   is checked too.
//! - The host checks itself on a hash mismatch (`super::selfcheck`) with
//!   the same builder, run inline so a run repeats; a test may give it a
//!   worker thread instead ([`VirtualNet::set_host_rebuild`]).
//! - Input latency is measured with markers: every [`MARK_EVERY`]th frame's
//!   pad carries a marker id 1..=3 in the [`MARK`] bits (button bits the sim
//!   ignores). A marker's change survives the pacer and the host's queue
//!   (merges keep edges, repeats hold levels), so when a peer steps a change
//!   to id `v` in its own slot's pads it is the latest marker `v` it sent.
//!   Latency is that send time to the step, in virtual ms. A marker the host
//!   dropped (sent before the slot joined) is skipped, never mismatched.
//! - Prediction (opt-in, [`VirtualNet::predict`]): every node runs its
//!   frames through a [`LocalView`] as the window does (the [`Predictor`]
//!   over its pending inputs), recording its CPU cost, depth, mode and
//!   corrections. It only reads the node's sim; the hash checks would catch
//!   a write.
//! - Drawing (opt-in, [`VirtualNet::draw`], with prediction): after each
//!   frame every node draws as the window does - the render blend on,
//!   [`LocalView::draw_in`] then [`LocalView::draw_out`] - and its
//!   `Sim::hash` must be the same after as before ([`DrawCheck`]).
//! - Loading time (opt-in, [`Node::load_ms`]): a node builds its round that
//!   many virtual ms after its `LoadRound` (its loading screen: the
//!   presentation and the build), so a test can give every peer its own.
//!   [`Node::loaded_at`], [`Node::playing_at`] and [`Node::first_step`]
//!   record when it built, when it first played and its first stepped tick.
//!   With [`Node::build_floor_ms`] the build alone takes that long: once the
//!   host skips the loading screen ([`VirtualNet::host_skip`],
//!   [`Peer::loading_skipped`]) the node builds at its floor (or at once if
//!   past it) instead of at the end of `load_ms`.
//! - Heroes (opt-in, [`VirtualNet::heroes`]): the scripted pads also accept
//!   hero offers and hold block now and then ([`scripted_hero_pad`]), so a
//!   human plays a saber hero whose swings and blocks are predicted and
//!   drawn ([`Node::hero_frames`], [`Node::melee_frames`]).
//! - Several local players (issue #194 B18, [`VirtualNet::join_locals`]): a
//!   guest machine's further locals play their own scripted pads, all sent
//!   under one sequence ([`Peer::frame_locals`]); with prediction every
//!   local slot is predicted. [`Node::local_pending`] records, per frame,
//!   each local's pending queue length.

use super::{Link, LinkEvent, LocalView, Notice, Peer, RoundSetup};
use crate::event_feed::EventFeed;
use crate::walk::WalkSim;
use core::sim::PadState;
use net::{
    ClientConfig, ClientCore, HostCommand, HostConfig, HostCore, LinkParams, LinkSim, Micros,
    TickBundle,
};
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

/// The marker bits (unused by the sim's buttons).
pub const MARK: u32 = 3 << 30;

fn mark_of(p: &PadState) -> u32 {
    (p.buttons & MARK) >> 30
}
/// Frames between markers.
pub const MARK_EVERY: u64 = 9;

/// A scripted pad for player slot `slot` at sim tick `tick` (the windowed
/// autopilot's pattern, phased and turned per slot): walks, wanders, fires
/// in bursts, presses the spawn screens' accept and now and then use.
pub fn scripted_pad(slot: u8, tick: u64) -> PadState {
    use core::sim::buttons as b;
    let k = f32::from(slot);
    let t = tick as f32 * core::TICK_DT + k * 1.3;
    let mut buttons = 0;
    if t % (4.0 + k * 0.5) < 1.5 {
        buttons |= b::FIRE;
    }
    if t % 0.5 < 0.25 {
        buttons |= b::SPAWN_ACCEPT;
    }
    if t % (7.0 + k) < 0.2 {
        buttons |= b::ACTION;
    }
    let turn =
        0.35 * core::fmath::sin_cos(t * (0.4 + 0.07 * k)).0 + 0.1 * core::fmath::sin_cos(t * 1.7).0;
    PadState {
        lx: PadState::quantize(0.3 * core::fmath::sin_cos(t * 0.23 + k).0),
        ly: PadState::quantize(1.0),
        rx: PadState::quantize(turn),
        buttons,
        ..PadState::default()
    }
}

/// [`scripted_pad`] for a hero soak ([`VirtualNet::heroes`]): it also
/// taps Accept on a hero offer every second and holds block (Reload) for
/// a second now and then, between the fire bursts.
pub fn scripted_hero_pad(slot: u8, tick: u64) -> PadState {
    use core::sim::buttons as b;
    let mut pad = scripted_pad(slot, tick);
    let k = f32::from(slot);
    let t = tick as f32 * core::TICK_DT + k * 1.3;
    if t % 1.0 < 0.5 {
        pad.buttons |= b::HERO_ACCEPT;
    }
    let block = t % (5.0 + k * 0.3);
    if (2.0..3.0).contains(&block) {
        pad.buttons = (pad.buttons & !b::FIRE) | b::RELOAD;
    }
    pad
}

/// Builds a peer's round for its local slot: the walk (already
/// `install_humans`-ed) and its event feed. The last argument is the node
/// index (0 = the host), so peers can load different copies of the game.
pub type Builder =
    Box<dyn FnMut(&RoundSetup, u8, usize) -> anyhow::Result<(WalkSim, Option<EventFeed>)>>;

/// The session core under a node.
pub enum NodeLink {
    Host(Box<HostCore>),
    Client(Box<ClientCore>),
}

impl NodeLink {
    fn handle(&mut self, now: Micros, from: SocketAddr, b: &[u8]) {
        match self {
            NodeLink::Host(h) => h.handle_datagram(now, from, b),
            NodeLink::Client(c) => c.handle_datagram(now, from, b),
        }
    }

    fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        match self {
            NodeLink::Host(h) => h.update(now),
            NodeLink::Client(c) => c.update(now),
        }
    }

    pub fn host(&self) -> Option<&HostCore> {
        match self {
            NodeLink::Host(h) => Some(h),
            NodeLink::Client(_) => None,
        }
    }

    pub fn client(&self) -> Option<&ClientCore> {
        match self {
            NodeLink::Client(c) => Some(c),
            NodeLink::Host(_) => None,
        }
    }

    pub fn client_mut(&mut self) -> Option<&mut ClientCore> {
        match self {
            NodeLink::Client(c) => Some(c),
            NodeLink::Host(_) => None,
        }
    }
}

macro_rules! both {
    ($s:expr, $x:ident => $e:expr) => {
        match $s {
            NodeLink::Host($x) => $e,
            NodeLink::Client($x) => $e,
        }
    };
}

impl Link for NodeLink {
    fn is_host(&self) -> bool {
        matches!(self, NodeLink::Host(_))
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        both!(self, x => x.as_mut().poll())
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        both!(self, x => Link::submit_input(x.as_mut(), pad, view_tick))
    }
    fn submit_inputs(&mut self, pads: &[(u8, PadState)], view_tick: u32) {
        both!(self, x => Link::submit_inputs(x.as_mut(), pads, view_tick))
    }
    fn pacing(&self) -> (u8, u32) {
        both!(self, x => Link::pacing(x.as_ref()))
    }
    fn jitter_ticks(&self) -> f64 {
        both!(self, x => Link::jitter_ticks(x.as_ref()))
    }
    fn slack_us(&self) -> Option<i64> {
        both!(self, x => Link::slack_us(x.as_ref()))
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        both!(self, x => Link::report_hash(x.as_mut(), tick, hash))
    }
    fn loaded(&mut self, r: u32, d: u64, h: u64) {
        both!(self, x => Link::loaded(x.as_mut(), r, d, h))
    }
    fn caught_up(&mut self, tick: u32) {
        both!(self, x => Link::caught_up(x.as_mut(), tick))
    }
    fn request(&mut self, data: Vec<u8>) {
        both!(self, x => Link::request(x.as_mut(), data))
    }
    fn request_resync(&mut self) {
        both!(self, x => Link::request_resync(x.as_mut()))
    }
    fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>) {
        both!(self, x => Link::desync_hashes(x.as_mut(), tick, first_tick, width, values))
    }
    fn host(&mut self, cmd: HostCommand) {
        both!(self, x => Link::host(x.as_mut(), cmd))
    }
    fn set_local_away(&mut self, local: u8, slot: u8, away: bool) {
        both!(self, x => Link::set_local_away(x.as_mut(), local, slot, away))
    }
}

/// Checks every step's hash against the first one seen for its tick.
#[derive(Debug, Default)]
pub struct HashChecker {
    reference: BTreeMap<u32, u64>,
    pub checks: u64,
    /// (node, tick, reference, got), the first 32.
    pub mismatches: Vec<(usize, u32, u64, u64)>,
    pub mismatch_count: u64,
    /// The latest tick that mismatched.
    pub last_mismatch_tick: Option<u32>,
}

impl HashChecker {
    pub fn check(&mut self, node: usize, tick: u32, hash: u64) {
        match self.reference.get(&tick) {
            Some(r) => {
                self.checks += 1;
                if *r != hash {
                    self.mismatch_count += 1;
                    self.last_mismatch_tick =
                        Some(self.last_mismatch_tick.map_or(tick, |t| t.max(tick)));
                    if self.mismatches.len() < 32 {
                        self.mismatches.push((node, tick, *r, hash));
                    }
                }
            }
            None => {
                self.reference.insert(tick, hash);
            }
        }
    }
}

/// [`VirtualNet::draw`]: frames drawn and those whose drawing changed the
/// confirmed sim's hash.
#[derive(Debug, Default)]
pub struct DrawCheck {
    pub frames: u64,
    /// Frames that drew the predicted local pose.
    pub swapped: u64,
    pub changed: u64,
    /// (node, tick), the first 32 that changed.
    pub first_changed: Vec<(usize, u64)>,
}

/// One peer.
pub struct Node {
    pub name: String,
    pub token: u64,
    pub peer: Peer,
    pub link: NodeLink,
    pub addr: SocketAddr,
    pub walk: Option<WalkSim>,
    pub feed: Option<EventFeed>,
    pub frame: Micros,
    pub next_frame: Micros,
    frames: u64,
    markers: VecDeque<(u32, Micros)>,
    marks_made: u32,
    last_mark: u32,
    last_sent_mark: u32,
    /// Marker send → own step, in virtual ms.
    pub latencies_ms: Vec<f64>,
    /// Static digest and tick-0 hash of each build.
    pub builds: Vec<(u64, u64)>,
    /// Each catch-up: virtual time the joiner's `LoadRound` arrived, when it
    /// caught up, and the wall ms its steps took.
    pub catch_up: Vec<(Micros, Option<Micros>, f64)>,
    /// Wall ms of each round build.
    pub build_ms: Vec<f64>,
    /// Virtual ms from a `LoadRound` to its build (module rules).
    pub load_ms: u64,
    /// The build alone within [`Node::load_ms`], in virtual ms: a host skip
    /// ends the loading no earlier (module rules). `None`: a skip does not
    /// shorten it.
    pub build_floor_ms: Option<u64>,
    /// A build waiting out [`Node::load_ms`]: when, the setup, joining,
    /// and when its `LoadRound` came.
    pending_build: Option<(Micros, RoundSetup, bool, Micros)>,
    /// The last round's build, first playing frame and first stepped tick
    /// (with its virtual time).
    pub loaded_at: Option<Micros>,
    pub playing_at: Option<Micros>,
    pub first_step: Option<(u32, Micros)>,
    /// Virtual time this node (re)connected.
    pub connected_at: Micros,
    load_wall: Option<std::time::Instant>,
    pub bytes_out: u64,
    pub bytes_in: u64,
    /// Notices, as text with their virtual time.
    pub log: Vec<(Micros, String)>,
    /// What every stepped command did, by tick.
    pub applied: Vec<(u32, super::Applied)>,
    pub build_error: Option<String>,
    /// [`VirtualNet::predict`]: the node's drawn view (its predictor) and,
    /// per frame that predicted, its CPU ms and depth.
    pub view: Option<LocalView>,
    pub predict_ms: Vec<f64>,
    pub predict_depth: Vec<u32>,
    /// Frames with prediction off although inputs were pending.
    pub predict_off: u64,
    /// Frames whose local unit was a hero, and those with its melee pose
    /// (a swing or a block) on.
    pub hero_frames: u64,
    pub melee_frames: u64,
    /// Per frame with further locals: each local's pending inputs, local 0
    /// first, while every local slot is seated (module rules).
    pub local_pending: Vec<Vec<usize>>,
    /// Frames each local (local 0 first) had predicted ticks drawn ahead.
    pub predicted_locals: Vec<u64>,
    /// The further locals' `Hello` entries, kept for a reconnect.
    locals: Vec<net::LocalHello>,
}

impl Node {
    /// A build is waiting out its loading time.
    pub fn loading(&self) -> bool {
        self.pending_build.is_some()
    }

    fn new(
        name: &str,
        token: u64,
        peer: Peer,
        link: NodeLink,
        addr: SocketAddr,
        frame_ms: f64,
    ) -> Self {
        Self {
            name: name.to_owned(),
            token,
            peer,
            link,
            addr,
            walk: None,
            feed: None,
            frame: Micros((frame_ms * 1000.0) as u64),
            next_frame: Micros::ZERO,
            frames: 0,
            markers: VecDeque::new(),
            marks_made: 0,
            last_mark: 0,
            last_sent_mark: 0,
            latencies_ms: Vec::new(),
            builds: Vec::new(),
            catch_up: Vec::new(),
            build_ms: Vec::new(),
            load_ms: 0,
            build_floor_ms: None,
            pending_build: None,
            loaded_at: None,
            playing_at: None,
            first_step: None,
            connected_at: Micros::ZERO,
            load_wall: None,
            bytes_out: 0,
            bytes_in: 0,
            log: Vec::new(),
            applied: Vec::new(),
            build_error: None,
            view: None,
            predict_ms: Vec::new(),
            predict_depth: Vec::new(),
            predict_off: 0,
            hero_frames: 0,
            melee_frames: 0,
            local_pending: Vec::new(),
            predicted_locals: Vec::new(),
            locals: Vec::new(),
        }
    }
}

/// The virtual network and its peers.
pub struct VirtualNet {
    pub sim: LinkSim,
    pub now: Micros,
    pub nodes: Vec<Node>,
    pub checker: HashChecker,
    build: String,
    builder: std::rc::Rc<std::cell::RefCell<Builder>>,
    conn_ids: u32,
    /// Every node predicts (module rules).
    pub predict: bool,
    /// Every predicting node draws each frame (module rules).
    pub draw: bool,
    pub draws: DrawCheck,
    /// The scripted pads accept heroes and block (module rules).
    pub heroes: bool,
}

/// Address of node `i` (0 = the host, on the default port).
pub fn addr(i: usize) -> SocketAddr {
    SocketAddr::from((
        [10, 0, 0, (i + 1) as u8],
        if i == 0 {
            net::DEFAULT_PORT
        } else {
            50_000 + i as u16
        },
    ))
}

impl VirtualNet {
    /// A host on a fresh network; `host_profile` is its `PlayerProfile` bytes.
    pub fn new(
        seed: u64,
        link: LinkParams,
        build: &str,
        host_profile: Vec<u8>,
        builder: Builder,
    ) -> Self {
        let mut sim = LinkSim::new(seed);
        sim.set_default(link);
        sim.add_endpoint(addr(0));
        let mut config = HostConfig::new(build, "Host");
        config.profile = host_profile;
        let builder = std::rc::Rc::new(std::cell::RefCell::new(builder));
        let mut peer = Peer::host();
        let shared = std::rc::Rc::clone(&builder);
        peer.rebuild = Some(super::Rebuild::Inline(Box::new(move |s, local| {
            (shared.borrow_mut())(s, local, 0)
        })));
        let host = Node::new(
            "Host",
            0,
            peer,
            NodeLink::Host(Box::new(HostCore::new(config))),
            addr(0),
            16.7,
        );
        Self {
            sim,
            now: Micros::from_millis(1),
            nodes: vec![host],
            checker: HashChecker::default(),
            build: build.to_owned(),
            builder,
            conn_ids: 1000,
            predict: false,
            draw: false,
            draws: DrawCheck::default(),
            heroes: false,
        }
    }

    /// How the host replays its round for a self-check (module rules).
    pub fn set_host_rebuild(&mut self, rebuild: Option<super::Rebuild>) {
        self.nodes[0].peer.rebuild = rebuild;
    }

    /// The host's core.
    pub fn host(&self) -> &HostCore {
        self.nodes[0].link.host().expect("node 0 hosts")
    }

    /// The host's next tick to close.
    pub fn host_tick(&self) -> u32 {
        self.host().next_tick()
    }

    fn client_core(
        &mut self,
        name: &str,
        token: u64,
        profile: Vec<u8>,
        locals: Vec<net::LocalHello>,
    ) -> ClientCore {
        let mut config = ClientConfig::new(self.build.clone(), name, token);
        config.profile = profile;
        config.locals = locals;
        self.conn_ids += 1;
        config.conn_id = self.conn_ids;
        ClientCore::new(config, addr(0), self.now)
    }

    /// A guest connects; returns its node index.
    pub fn join(&mut self, name: &str, token: u64, profile: Vec<u8>, frame_ms: f64) -> usize {
        self.join_locals(name, token, profile, frame_ms, Vec::new())
    }

    /// A guest machine with further local players (`locals`, seats 1..)
    /// connects; returns its node index (module rules).
    pub fn join_locals(
        &mut self,
        name: &str,
        token: u64,
        profile: Vec<u8>,
        frame_ms: f64,
        locals: Vec<net::LocalHello>,
    ) -> usize {
        let i = self.nodes.len();
        let a = addr(i);
        self.sim.add_endpoint(a);
        let core = self.client_core(name, token, profile, locals.clone());
        let mut node = Node::new(
            name,
            token,
            Peer::client(),
            NodeLink::Client(Box::new(core)),
            a,
            frame_ms,
        );
        node.next_frame = self.now;
        node.connected_at = self.now;
        node.locals = locals;
        self.nodes.push(node);
        i
    }

    /// Node `i`'s local player `local` lost its pad (`away`) or got it back,
    /// as the window's lost-pad path asks it ([`Peer::set_local_away`]).
    pub fn set_local_away(&mut self, i: usize, local: usize, away: bool) {
        let n = &mut self.nodes[i];
        n.peer.set_local_away(&mut n.link, local, away);
    }

    /// Cut node `i` off the network (or restore it).
    pub fn cut(&mut self, i: usize, cut: bool) {
        self.sim.cut(self.nodes[i].addr, cut);
    }

    /// Node `i` reconnects with its token: a new connection, a new peer and
    /// no round until the host's `LoadRound` (the rejoin path).
    pub fn reconnect(&mut self, i: usize, profile: Vec<u8>) {
        self.cut(i, false);
        let (name, token) = (self.nodes[i].name.clone(), self.nodes[i].token);
        let locals = self.nodes[i].locals.clone();
        let core = self.client_core(&name, token, profile, locals);
        let node = &mut self.nodes[i];
        node.link = NodeLink::Client(Box::new(core));
        node.peer = Peer::client();
        node.walk = None;
        node.feed = None;
        node.markers.clear();
        node.last_mark = 0;
        node.last_sent_mark = 0;
        node.next_frame = self.now;
        node.connected_at = self.now;
    }

    /// The host starts a round with `setup`.
    pub fn host_load(&mut self, setup: &RoundSetup) {
        let n = &mut self.nodes[0];
        let bytes = n.peer.host_load(&mut n.link, setup);
        let setup = RoundSetup::decode(&bytes).expect("own setup decodes");
        self.load_node(0, setup, false);
    }

    /// A `LoadRound` (or the host's own): built now, or after the node's
    /// [`Node::load_ms`].
    fn load_node(&mut self, i: usize, setup: RoundSetup, joining: bool) {
        let n = &mut self.nodes[i];
        (n.loaded_at, n.playing_at, n.first_step) = (None, None, None);
        if n.load_ms == 0 {
            self.build_node(i, &setup, joining);
        } else {
            // Its loading screen: no frame of the old round meanwhile.
            n.walk = None;
            let due = self.now + Micros::from_millis(n.load_ms);
            n.pending_build = Some((due, setup, joining, self.now));
        }
    }

    /// The host skips its round start's loading screen for everyone
    /// ([`Peer::skip_loading`]). Returns whether the skip went out.
    pub fn host_skip(&mut self) -> bool {
        let n = &mut self.nodes[0];
        n.peer.skip_loading(&mut n.link)
    }

    /// The host issues a game command.
    pub fn host_command(&mut self, cmd: &super::GameCommand) {
        let n = &mut self.nodes[0];
        n.peer.host_command(&mut n.link, cmd);
    }

    /// Guest `i` asks the host.
    pub fn request(&mut self, i: usize, req: &super::GameRequest) {
        let n = &mut self.nodes[i];
        n.peer.request(&mut n.link, req);
    }

    fn build_node(&mut self, i: usize, setup: &RoundSetup, joining: bool) {
        let started = std::time::Instant::now();
        let now = self.now;
        let local = self.nodes[i].peer.local_slot;
        let built = (self.builder.borrow_mut())(setup, local, i);
        match built {
            Ok((walk, feed)) => {
                let n = &mut self.nodes[i];
                n.loaded_at = Some(now);
                n.build_ms.push(started.elapsed().as_secs_f64() * 1e3);
                n.walk = Some(walk);
                n.feed = feed;
                let walk = n.walk.as_mut().expect("built");
                n.peer.round_built(&mut n.link, walk);
                n.builds.push((walk.sim.static_digest(), walk.sim.hash()));
                if joining {
                    n.catch_up.push((now, None, 0.0));
                    n.load_wall = Some(std::time::Instant::now());
                }
            }
            Err(e) => {
                let n = &mut self.nodes[i];
                n.build_error = Some(format!("{e:#}"));
                n.log.push((now, format!("build failed: {e:#}")));
            }
        }
    }

    /// One virtual millisecond.
    pub fn step(&mut self) {
        let now = self.now;
        self.sim.advance(now);
        for n in &mut self.nodes {
            while let Some((from, b)) = self.sim.recv(n.addr) {
                n.bytes_in += b.len() as u64;
                n.link.handle(now, from, &b);
            }
        }
        for i in 0..self.nodes.len() {
            self.frames(i);
            let n = &mut self.nodes[i];
            for (to, b) in n.link.update(now) {
                n.bytes_out += b.len() as u64;
                self.sim.send(now, n.addr, to, &b);
            }
        }
        self.now = self.now + Micros::from_millis(1);
    }

    fn frames(&mut self, i: usize) {
        let now = self.now;
        while now >= self.nodes[i].next_frame {
            let n = &mut self.nodes[i];
            n.next_frame = n.next_frame + n.frame;
            n.frames += 1;
            // The host skipped: the build is due at its floor.
            if let (Some(floor), Some((at, .., from))) =
                (n.build_floor_ms, n.pending_build.as_mut())
            {
                if n.peer.loading_skipped() {
                    *at = (*at).min(*from + Micros::from_millis(floor));
                }
            }
            if n.pending_build.as_ref().is_some_and(|(at, ..)| now >= *at) {
                let (_, setup, joining, _) = n.pending_build.take().expect("due build");
                self.build_node(i, &setup, joining);
            }
            let n = &mut self.nodes[i];
            let notices = n.peer.poll(&mut n.link);
            for notice in notices {
                if let Notice::Load { setup, joining, .. } = &notice {
                    let (setup, joining) = (setup.as_ref().clone(), *joining);
                    self.nodes[i]
                        .log
                        .push((now, format!("load (joining {joining})")));
                    self.load_node(i, setup, joining);
                    continue;
                }
                self.nodes[i].log.push((now, format!("{notice:?}")));
            }
            let n = &mut self.nodes[i];
            let Some(walk) = n.walk.as_mut() else {
                continue;
            };
            let mut pad = if self.heroes {
                scripted_hero_pad(n.peer.local_slot, walk.sim.tick)
            } else {
                scripted_pad(n.peer.local_slot, walk.sim.tick)
            };
            if n.frames.is_multiple_of(MARK_EVERY) {
                pad.buttons |= (n.marks_made % 3 + 1) << 30;
                n.marks_made += 1;
            }
            let slot = n.peer.local_slot;
            // The further locals' own scripted pads (module rules).
            let mut pads = vec![pad];
            for q in n.peer.extra_locals() {
                pads.push(if self.heroes {
                    scripted_hero_pad(q.slot, walk.sim.tick)
                } else {
                    scripted_pad(q.slot, walk.sim.tick)
                });
            }
            let dt = n.frame.as_secs_f64();
            if self.predict && n.view.is_none() {
                n.view = Some(LocalView::new());
            }
            let (checker, markers, last_mark, lat, first_step) = (
                &mut self.checker,
                &mut n.markers,
                &mut n.last_mark,
                &mut n.latencies_ms,
                &mut n.first_step,
            );
            let mut on_tick = |w: &mut WalkSim, b: &TickBundle| {
                first_step.get_or_insert((b.tick, now));
                checker.check(i, b.tick, w.sim.hash());
                let mark = b.pad(slot).map_or(0, mark_of);
                if mark != 0 && mark != *last_mark {
                    if let Some(k) = markers.iter().rposition(|(v, _)| *v == mark) {
                        lat.push(now.saturating_sub(markers[k].1).as_millis_f64());
                        markers.drain(..=k);
                    }
                }
                *last_mark = mark;
            };
            // With prediction the window's own per-frame step: the frame,
            // then the prediction over the inputs still pending.
            let report = match n.view.as_mut() {
                Some(view) => view.frame_locals(
                    &mut n.peer,
                    &mut n.link,
                    dt as f32,
                    &pads,
                    walk,
                    n.feed.as_mut(),
                    |w, b, _, _| on_tick(w, b),
                ),
                None => {
                    n.peer
                        .frame_locals(&mut n.link, dt, &pads, walk, n.feed.as_mut(), |w, b, _| {
                            on_tick(w, b)
                        })
                }
            };
            if !n.peer.extra_locals().is_empty()
                && (0..n.peer.local_count()).all(|k| {
                    n.peer.local_slot_of(k).is_some_and(|s| {
                        walk.sim
                            .players
                            .get(usize::from(s))
                            .is_some_and(Option::is_some)
                    })
                })
            {
                n.local_pending.push(
                    (0..n.peer.local_count())
                        .map(|k| n.peer.pending_inputs_of(k).len())
                        .collect(),
                );
            }
            // A marker held across several sent ticks is one rising edge.
            for p in &report.submitted {
                let m = mark_of(p);
                if m != 0 && m != n.last_sent_mark {
                    n.markers.push_back((m, now));
                    if n.markers.len() > 64 {
                        n.markers.pop_front();
                    }
                }
                n.last_sent_mark = m;
            }
            n.applied.extend(report.applied);
            if report.mode == super::Mode::Playing {
                n.playing_at.get_or_insert(now);
            }
            if let Some(view) = n.view.as_mut() {
                let p = &view.predictor;
                let locals = n.peer.local_count();
                n.predicted_locals
                    .resize(locals.max(n.predicted_locals.len()), 0);
                for (k, frames) in n.predicted_locals.iter_mut().take(locals).enumerate() {
                    *frames += u64::from(p.poses_of(k).len() > 1);
                }
                if p.depth() > 0 {
                    n.predict_ms.push(p.stats.cost_ms);
                    n.predict_depth.push(p.depth() as u32);
                } else if !n.peer.pending_inputs().is_empty() {
                    n.predict_off += 1;
                }
                if self.draw {
                    walk.blend.on = true;
                    let before = walk.sim.hash();
                    view.draw_in(walk);
                    self.draws.frames += 1;
                    self.draws.swapped += u64::from(view.predictor.swapped());
                    view.draw_out(walk);
                    if walk.sim.hash() != before {
                        self.draws.changed += 1;
                        if self.draws.first_changed.len() < 32 {
                            self.draws.first_changed.push((i, walk.sim.tick));
                        }
                    }
                }
            }
            if let Some(u) = walk
                .sim
                .players
                .get(usize::from(slot))
                .copied()
                .flatten()
                .filter(|h| walk.sim.is_hero(*h))
                .and_then(|h| walk.sim.unit(h))
            {
                n.hero_frames += 1;
                let melee = u.weapon.melee.as_ref().and_then(|m| m.pose());
                n.melee_frames += u64::from(melee.is_some());
            }
            if let Some(t) = report.caught_up {
                let wall = n
                    .load_wall
                    .take()
                    .map_or(0.0, |w| w.elapsed().as_secs_f64() * 1e3);
                if let Some(c) = n.catch_up.last_mut() {
                    c.1 = Some(now);
                    c.2 = wall;
                }
                n.log.push((now, format!("caught up at tick {t}")));
            }
        }
    }

    /// Steps until `f` holds or `limit_ms` pass; whether it held.
    pub fn run_until(&mut self, limit_ms: u64, mut f: impl FnMut(&VirtualNet) -> bool) -> bool {
        for _ in 0..limit_ms {
            if f(self) {
                return true;
            }
            self.step();
        }
        f(self)
    }
}
