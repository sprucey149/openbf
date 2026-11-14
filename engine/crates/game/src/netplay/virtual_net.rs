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
//! - Input latency is measured with markers: every [`MARK_EVERY`]th frame's
//!   pad carries a marker id 1..=3 in the [`MARK`] bits (button bits the sim
//!   ignores). A marker's change survives the pacer and the host's queue
//!   (merges keep edges, repeats hold levels), so when a peer steps a change
//!   to id `v` in its own slot's pads it is the latest marker `v` it sent.
//!   Latency is that send time to the step, in virtual ms. A marker the host
//!   dropped (sent before the slot joined) is skipped, never mismatched.
//! - Prediction (opt-in, [`VirtualNet::predict`]): every node also runs a
//!   [`Predictor`] over its pending inputs each frame, as the window does,
//!   recording its CPU cost, depth, mode and corrections. It only reads the
//!   node's sim; the hash checks would catch a write.

use super::{Link, LinkEvent, Notice, Peer, RoundSetup};
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
}

/// Checks every step's hash against the first one seen for its tick.
#[derive(Debug, Default)]
pub struct HashChecker {
    reference: BTreeMap<u32, u64>,
    pub checks: u64,
    /// (node, tick, reference, got), the first 32.
    pub mismatches: Vec<(usize, u32, u64, u64)>,
    pub mismatch_count: u64,
}

impl HashChecker {
    pub fn check(&mut self, node: usize, tick: u32, hash: u64) {
        match self.reference.get(&tick) {
            Some(r) => {
                self.checks += 1;
                if *r != hash {
                    self.mismatch_count += 1;
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
    /// [`VirtualNet::predict`]: the node's predictor and, per frame that
    /// predicted, its CPU ms and depth.
    pub predictor: Option<super::Predictor>,
    pub predict_ms: Vec<f64>,
    pub predict_depth: Vec<u32>,
    /// Frames with prediction off although inputs were pending.
    pub predict_off: u64,
}

impl Node {
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
            connected_at: Micros::ZERO,
            load_wall: None,
            bytes_out: 0,
            bytes_in: 0,
            log: Vec::new(),
            applied: Vec::new(),
            build_error: None,
            predictor: None,
            predict_ms: Vec::new(),
            predict_depth: Vec::new(),
            predict_off: 0,
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
    builder: Builder,
    conn_ids: u32,
    /// Every node predicts (module rules).
    pub predict: bool,
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
        let host = Node::new(
            "Host",
            0,
            Peer::host(),
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
        }
    }

    /// The host's core.
    pub fn host(&self) -> &HostCore {
        self.nodes[0].link.host().expect("node 0 hosts")
    }

    /// The host's next tick to close.
    pub fn host_tick(&self) -> u32 {
        self.host().next_tick()
    }

    fn client_core(&mut self, name: &str, token: u64, profile: Vec<u8>) -> ClientCore {
        let mut config = ClientConfig::new(self.build.clone(), name, token);
        config.profile = profile;
        self.conn_ids += 1;
        config.conn_id = self.conn_ids;
        ClientCore::new(config, addr(0), self.now)
    }

    /// A guest connects; returns its node index.
    pub fn join(&mut self, name: &str, token: u64, profile: Vec<u8>, frame_ms: f64) -> usize {
        let i = self.nodes.len();
        let a = addr(i);
        self.sim.add_endpoint(a);
        let core = self.client_core(name, token, profile);
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
        self.nodes.push(node);
        i
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
        let core = self.client_core(&name, token, profile);
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
        self.build_node(0, &setup, false);
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
        match (self.builder)(setup, local, i) {
            Ok((walk, feed)) => {
                let n = &mut self.nodes[i];
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
            let notices = n.peer.poll(&mut n.link);
            for notice in notices {
                if let Notice::Load { setup, joining, .. } = &notice {
                    let (setup, joining) = (setup.as_ref().clone(), *joining);
                    self.nodes[i]
                        .log
                        .push((now, format!("load (joining {joining})")));
                    self.build_node(i, &setup, joining);
                    continue;
                }
                self.nodes[i].log.push((now, format!("{notice:?}")));
            }
            let n = &mut self.nodes[i];
            let Some(walk) = n.walk.as_mut() else {
                continue;
            };
            let mut pad = scripted_pad(n.peer.local_slot, walk.sim.tick);
            if n.frames.is_multiple_of(MARK_EVERY) {
                pad.buttons |= (n.marks_made % 3 + 1) << 30;
                n.marks_made += 1;
            }
            let slot = n.peer.local_slot;
            let dt = n.frame.as_secs_f64();
            if self.predict && n.predictor.is_none() {
                n.predictor = Some(super::Predictor::new());
            }
            let (checker, markers, last_mark, lat, predictor) = (
                &mut self.checker,
                &mut n.markers,
                &mut n.last_mark,
                &mut n.latencies_ms,
                &mut n.predictor,
            );
            let report = n.peer.frame(
                &mut n.link,
                dt,
                pad,
                walk,
                n.feed.as_mut(),
                |w, b: &TickBundle, _| {
                    checker.check(i, b.tick, w.sim.hash());
                    if let Some(p) = predictor.as_mut() {
                        p.note_bundle(b, &w.sim, slot);
                        p.confirm_shots(&w.sim, w.player);
                    }
                    let mark = b.pad(slot).map_or(0, mark_of);
                    if mark != 0 && mark != *last_mark {
                        if let Some(k) = markers.iter().rposition(|(v, _)| *v == mark) {
                            lat.push(now.saturating_sub(markers[k].1).as_millis_f64());
                            markers.drain(..=k);
                        }
                    }
                    *last_mark = mark;
                },
            );
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
            if let Some(p) = n.predictor.as_mut() {
                let pending: Vec<PadState> = n.peer.pending_inputs().iter().copied().collect();
                p.update(&walk.sim, slot, &pending, dt as f32);
                if p.depth() > 0 {
                    n.predict_ms.push(p.stats.cost_ms);
                    n.predict_depth.push(p.depth() as u32);
                } else if !pending.is_empty() {
                    n.predict_off += 1;
                }
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
