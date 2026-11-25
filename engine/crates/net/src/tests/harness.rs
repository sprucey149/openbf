//! A host and clients on one [`LinkSim`], stepped 1 ms at a time in virtual
//! time. Each player runs a frame loop with an [`InputPacer`] fed by the
//! host's pacing reports, like the game will.

use crate::linksim::Lcg;
use crate::*;
use core::sim::{buttons, PadState};
use std::net::SocketAddr;

pub const BUILD: &str = "test-build";

pub fn addr(i: u16) -> SocketAddr {
    SocketAddr::from((
        [10, 0, 0, (i + 1) as u8],
        if i == 0 { DEFAULT_PORT } else { 50_000 + i },
    ))
}

/// A simulated player's frame loop.
pub struct Player {
    pub pacer: InputPacer,
    pub frame: Micros,
    pub next_frame: Micros,
    /// Local clock rate (1.0 = the host's).
    pub drift: f64,
    pub rng: Lcg,
    pub fire: bool,
    pub playing: bool,
    /// Every pad field random over its whole range (the largest bundles).
    pub wild: bool,
    pub submitted: Vec<PadState>,
}

impl Player {
    pub fn new(frame_ms: f64, drift: f64, seed: u64) -> Self {
        Self {
            pacer: InputPacer::new(),
            frame: Micros((frame_ms * 1000.0) as u64),
            next_frame: Micros::ZERO,
            drift,
            rng: Lcg(seed),
            fire: false,
            playing: false,
            wild: false,
            submitted: Vec::new(),
        }
    }

    /// Runs due frames; returns the input ticks to submit.
    fn frames(&mut self, now: Micros, pacing: (u8, u32), slack: Option<i64>) -> Vec<PadState> {
        self.pacer.slack(slack);
        let mut out = Vec::new();
        while self.playing && now >= self.next_frame {
            self.next_frame = self.next_frame + self.frame;
            if self.rng.unit() < 0.15 {
                self.fire = !self.fire;
            }
            let r = &mut self.rng;
            let pad = if self.wild {
                PadState {
                    lx: r.next_u64() as i16,
                    ly: r.next_u64() as i16,
                    rx: r.next_u64() as i16,
                    ry: r.next_u64() as i16,
                    buttons: r.next_u64() as u32,
                    mx: r.next_u64() as i32,
                    my: r.next_u64() as i32,
                    fire_lag: r.next_u64() as u8,
                    pick: Some(core::sim::ScreenPick::Walker {
                        index: r.next_u64() as u32,
                        gen: r.next_u64() as u32,
                    }),
                    view_lag: 0,
                    tracked_shots: [None; 2],
                    tracked_combat: None,
                }
            } else {
                PadState {
                    mx: self.rng.upto(80) as i32 - 40,
                    my: self.rng.upto(20) as i32 - 10,
                    lx: (self.rng.upto(2000) as i16) - 1000,
                    buttons: if self.fire { buttons::FIRE } else { 0 },
                    ..PadState::default()
                }
            };
            self.pacer.feedback(pacing.0, pacing.1);
            out.extend(self.pacer.frame(self.frame.as_secs_f64() * self.drift, pad));
        }
        out
    }
}

pub struct Client {
    pub core: ClientCore,
    pub addr: SocketAddr,
    pub player: Player,
    pub events: Vec<ClientEvent>,
    pub bundles: Vec<TickBundle>,
    /// When each of `bundles` arrived.
    pub bundle_at: Vec<Micros>,
    pub alive: bool,
    /// Answer `LoadRound` with `loaded` at once.
    pub auto_load: bool,
    /// When the last `Start` arrived.
    pub start_at: Option<Micros>,
    /// FNV-1a over every datagram this client sent, in order.
    pub sent_hash: u64,
    /// Extra local players on this machine: each submits a pad of its own
    /// (from `extra_rng`) beside every pad of `player`, under one sequence.
    pub extra_locals: usize,
    pub extra_rng: Lcg,
    /// What each extra local submitted.
    pub extra_submitted: Vec<Vec<PadState>>,
}

/// How [`Net`] hands the host's players' pads to its core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostSubmit {
    /// `submit_input_for` per slot, between updates.
    Direct,
    /// One [`HostCommand::SubmitInputsFor`] per frame, as `NetSession` sends.
    Session,
    /// One [`HostCommand::SubmitInputFor`] per slot with the thread's update
    /// run between slot 0's and the rest (the old per-slot stream).
    TornSession,
}

pub struct Net {
    pub sim: LinkSim,
    pub now: Micros,
    pub host: HostCore,
    pub host_player: Player,
    /// The host's extra local players ([`HostCore::add_local`] slots): each
    /// submits a pad of its own (from `host_extra_rng`) beside every pad of
    /// `host_player`.
    pub host_extra: Vec<u8>,
    pub host_extra_rng: Lcg,
    /// What each of `host_extra` submitted.
    pub host_extra_submitted: Vec<Vec<PadState>>,
    /// How the host's players' pads go in: straight to the core, or as the
    /// threaded session applies its commands.
    pub host_submit: HostSubmit,
    host_torn_sends: Vec<(SocketAddr, Vec<u8>)>,
    /// Ticks the `TornSession` update closed between two slots' commands.
    pub host_torn_closes: u32,
    pub clients: Vec<Client>,
    pub host_bundles: Vec<TickBundle>,
    /// When each of `host_bundles` closed.
    pub host_bundle_at: Vec<Micros>,
    /// With `record_sends`: for every host datagram carrying bundles, the
    /// host's next tick after the update that sent it, and its destination.
    pub bundle_sends: Vec<(u32, SocketAddr)>,
    pub record_sends: bool,
    pub host_events: Vec<(Micros, NetEvent)>,
    /// Per slot, the queue depth at each closed tick.
    pub depths: Vec<Vec<u8>>,
    /// FNV-1a over every datagram the host sent, in order.
    pub host_sent_hash: u64,
}

impl Net {
    pub fn new(seed: u64, link: LinkParams, config: HostConfig) -> Self {
        let mut sim = LinkSim::new(seed);
        sim.set_default(link);
        sim.add_endpoint(addr(0));
        Self {
            sim,
            now: Micros::from_millis(1),
            host: HostCore::new(config),
            host_player: Player::new(6.9, 1.0, seed ^ 77),
            host_extra: Vec::new(),
            host_extra_rng: Lcg(0x4057 ^ seed),
            host_extra_submitted: Vec::new(),
            host_submit: HostSubmit::Direct,
            host_torn_sends: Vec::new(),
            host_torn_closes: 0,
            clients: Vec::new(),
            host_bundles: Vec::new(),
            host_bundle_at: Vec::new(),
            bundle_sends: Vec::new(),
            record_sends: false,
            host_events: Vec::new(),
            depths: vec![Vec::new(); MAX_SLOTS],
            host_sent_hash: 0xcbf2_9ce4_8422_2325,
        }
    }

    /// The players here report no hashes, so the fell-behind drop is off
    /// (its own test turns it on).
    pub fn host_config() -> HostConfig {
        HostConfig {
            fell_behind_ticks: 0,
            ..HostConfig::new(BUILD, "Host")
        }
    }

    /// Adds a client at address index `idx`; returns its index in `clients`.
    pub fn join(&mut self, idx: u16, name: &str, token: u64, player: Player) -> usize {
        self.join_with(idx, ClientConfig::new(BUILD, name, token), player)
    }

    pub fn join_with(&mut self, idx: u16, mut config: ClientConfig, player: Player) -> usize {
        let a = addr(idx);
        self.sim.add_endpoint(a);
        config.conn_id = 1000 + u32::from(idx) * 7 + self.clients.len() as u32;
        self.clients.push(Client {
            core: ClientCore::new(config, addr(0), self.now),
            addr: a,
            player,
            events: Vec::new(),
            bundles: Vec::new(),
            bundle_at: Vec::new(),
            alive: true,
            auto_load: true,
            start_at: None,
            sent_hash: 0xcbf2_9ce4_8422_2325,
            extra_locals: 0,
            extra_rng: Lcg(0x5eed ^ u64::from(idx)),
            extra_submitted: Vec::new(),
        });
        self.clients.len() - 1
    }

    pub fn step(&mut self) {
        let now = self.now;
        self.sim.advance(now);
        while let Some((from, b)) = self.sim.recv(addr(0)) {
            self.host.handle_datagram(now, from, &b);
        }
        for c in self.clients.iter_mut().filter(|c| c.alive) {
            while let Some((from, b)) = self.sim.recv(c.addr) {
                c.core.handle_datagram(now, from, &b);
            }
        }
        // The host player's frames go in before the tick clock runs.
        // Slot 0's pacing, or the playing host local's while its pad is away.
        let pacing = self.host.host_pacing();
        // The host plays each bundle as it closes: it views the newest.
        let view = self.host_bundles.len() as u32;
        let slack = self.host.host_slack_us();
        for p in self.host_player.frames(now, pacing, slack) {
            self.host_player.submitted.push(p);
            self.host_extra_submitted
                .resize(self.host_extra.len(), Vec::new());
            let mut row = vec![(0, p)];
            for (k, slot) in self.host_extra.iter().enumerate() {
                let q = random_pad(&mut self.host_extra_rng);
                row.push((*slot, q));
                self.host_extra_submitted[k].push(q);
            }
            match self.host_submit {
                HostSubmit::Direct => {
                    for (slot, pad) in row {
                        self.host.submit_input_for(slot, pad, view);
                    }
                }
                HostSubmit::Session => SessionCore::apply(
                    &mut self.host,
                    HostCommand::SubmitInputsFor {
                        pads: row,
                        view_tick: view,
                    },
                ),
                HostSubmit::TornSession => {
                    // The thread woke between two slots' commands and ran
                    // its update at the tick deadline.
                    for (i, (slot, pad)) in row.into_iter().enumerate() {
                        if i == 1 {
                            let t = self.host.next_tick();
                            self.host_torn_sends.extend(self.host.update(now));
                            self.host_torn_closes += self.host.next_tick() - t;
                        }
                        SessionCore::apply(
                            &mut self.host,
                            HostCommand::SubmitInputFor {
                                slot,
                                pad,
                                view_tick: view,
                            },
                        );
                    }
                }
            }
        }
        let before = self.host.next_tick();
        let mut sends = std::mem::take(&mut self.host_torn_sends);
        sends.extend(self.host.update(now));
        for (to, b) in sends {
            if self.record_sends && carries_bundles(&b) {
                self.bundle_sends.push((self.host.next_tick(), to));
            }
            for byte in &b {
                self.host_sent_hash =
                    (self.host_sent_hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
            }
            self.sim.send(now, addr(0), to, &b);
        }
        while let Some(e) = self.host.poll_event() {
            match e {
                NetEvent::Bundle(b) => {
                    self.host_bundles.push(b);
                    self.host_bundle_at.push(now);
                }
                NetEvent::AllLoaded { .. } => {
                    self.host_events.push((now, e));
                    self.host.start_round();
                }
                NetEvent::RoundStarted { .. } => {
                    self.host_player.playing = true;
                    self.host_events.push((now, e));
                }
                // As the host game does: a returning pad is seated again.
                NetEvent::LocalBack { slot, .. } => {
                    self.host_events.push((now, e));
                    self.host.add_human(slot, TEAM_AUTO);
                }
                e => self.host_events.push((now, e)),
            }
        }
        if self.host.next_tick() != before {
            for (slot, d) in self.depths.iter_mut().enumerate() {
                if self.host.is_active(slot as u8) {
                    d.push(self.host.queue_depth(slot as u8).unwrap_or(0));
                }
            }
        }
        for c in self.clients.iter_mut().filter(|c| c.alive) {
            let pads = c.player.frames(now, c.core.pacing(), c.core.slack_us());
            // The client views the bundles it has received.
            let view = c.bundles.len() as u32;
            for p in pads {
                if c.extra_locals == 0 {
                    if c.core.submit_input(p, view).is_some() {
                        c.player.submitted.push(p);
                    }
                    continue;
                }
                let mut all = vec![p];
                for _ in 0..c.extra_locals {
                    all.push(random_pad(&mut c.extra_rng));
                }
                if c.core.submit_inputs(&all, view).is_some() {
                    c.player.submitted.push(p);
                    c.extra_submitted.resize(c.extra_locals, Vec::new());
                    for (k, q) in all[1..].iter().enumerate() {
                        c.extra_submitted[k].push(*q);
                    }
                }
            }
            for (to, b) in c.core.update(now) {
                for byte in &b {
                    c.sent_hash = (c.sent_hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
                }
                self.sim.send(now, c.addr, to, &b);
            }
            while let Some(e) = c.core.poll_event() {
                match e {
                    ClientEvent::Bundle(b) => {
                        let mine = c.core.slot().is_some_and(|s| {
                            b.commands.iter().any(|cmd| {
                                matches!(cmd, SessionCommand::AddHuman { slot, .. } if *slot == s)
                            })
                        });
                        if mine {
                            c.player.playing = true;
                        }
                        c.bundles.push(b);
                        c.bundle_at.push(now);
                    }
                    ClientEvent::LoadRound {
                        round_id, joining, ..
                    } => {
                        if c.auto_load && !joining {
                            c.core.loaded(round_id, 7, 9);
                        }
                        c.bundles.clear();
                        c.bundle_at.clear();
                        c.events.push(e);
                    }
                    ClientEvent::Start { .. } => {
                        c.player.playing = true;
                        c.start_at = Some(now);
                        c.events.push(e);
                    }
                    e => c.events.push(e),
                }
            }
        }
        self.now = self.now + Micros::from_millis(1);
    }

    pub fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms {
            self.step();
        }
    }

    /// Steps until `f` holds; panics after `limit_ms`.
    pub fn run_until(&mut self, limit_ms: u64, mut f: impl FnMut(&Net) -> bool) {
        for _ in 0..limit_ms {
            if f(self) {
                return;
            }
            self.step();
        }
        assert!(f(self), "condition not reached in {limit_ms} ms");
    }

    /// Every client welcomed.
    pub fn connect_all(&mut self) {
        self.run_until(5_000, |n| n.clients.iter().all(|c| c.core.slot().is_some()));
    }

    /// Loads and starts a round seating every connected player (the harness
    /// starts it on `AllLoaded`).
    pub fn start_round(&mut self) {
        let humans: Vec<u8> = self.host.players().iter().map(|p| p.0).collect();
        self.host.load_round(b"setup".to_vec(), &humans);
        self.run_until(5_000, |n| {
            n.host_events
                .iter()
                .any(|(_, e)| matches!(e, NetEvent::RoundStarted { .. }))
        });
    }

    pub fn host_event(&self, f: impl Fn(&NetEvent) -> bool) -> Option<(Micros, NetEvent)> {
        self.host_events.iter().find(|(_, e)| f(e)).cloned()
    }
}

/// An extra local's pad: mouse, stick and a FIRE press now and then.
pub fn random_pad(r: &mut Lcg) -> PadState {
    PadState {
        mx: r.upto(60) as i32 - 30,
        my: r.upto(30) as i32 - 15,
        lx: (r.upto(2000) as i16) - 1000,
        buttons: if r.unit() < 0.2 { buttons::FIRE } else { 0 },
        ..PadState::default()
    }
}

/// Whether a datagram carries a bundles chunk.
pub fn carries_bundles(bytes: &[u8]) -> bool {
    let Ok((_, body)) = crate::packet::parse_header(bytes) else {
        return false;
    };
    crate::packet::parse_chunks(body).is_ok_and(|cs| {
        cs.iter()
            .any(|c| matches!(c, crate::packet::Chunk::Bundles(_)))
    })
}

/// Rising edges of `bit` through a pad sequence.
pub fn edges<'a>(pads: impl Iterator<Item = &'a PadState>, bit: u32) -> usize {
    let mut prev = 0;
    pads.filter(|p| {
        let e = p.buttons & bit != 0 && prev & bit == 0;
        prev = p.buttons;
        e
    })
    .count()
}

/// `slot`'s pads through a bundle sequence.
pub fn slot_pads(bundles: &[TickBundle], slot: u8) -> Vec<PadState> {
    bundles
        .iter()
        .filter_map(|b| b.pad(slot).copied())
        .collect()
}
