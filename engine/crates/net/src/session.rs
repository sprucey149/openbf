//! [`NetSession`]: one network thread per session, owning the transport, the
//! clock and the protocol core; the game thread talks to it over channels.
//!
//! Rules:
//! - The thread polls about every millisecond: commands from the game, then
//!   every waiting datagram, then the core's update (which on the host closes
//!   due ticks), then events out. A frame hitch, a blocking load or an
//!   unfocused window on the game thread never stops acks, heartbeats or the
//!   host's tick clock.
//! - Commands and events are plain `mpsc` channels; nothing blocks the game
//!   thread. Stats are published to a shared slot every 50 ms.
//! - Shutting down (an explicit [`NetSession::shutdown`] or a drop) calls the
//!   core's `close` (the `Leave` message) and keeps the loop running up to
//!   200 ms to deliver it.

use crate::client::{ClientConfig, ClientCore, ClientEvent, ClientStats};
use crate::host::{HostConfig, HostCore, HostStats, NetEvent};
use crate::transport::Transport;
use crate::{Micros, RealClock, SharedClock};
use core::sim::PadState;
use std::net::SocketAddr;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const POLL: Duration = Duration::from_millis(1);
const STATS_EVERY: Micros = Micros::from_millis(50);
const CLOSE_FLUSH: Micros = Micros::from_millis(200);

/// A protocol core a [`NetSession`] can drive.
pub trait SessionCore: Send + 'static {
    type Command: Send + 'static;
    type Event: Send + 'static;
    type Stats: Clone + Default + Send + 'static;
    fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]);
    fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)>;
    fn apply(&mut self, cmd: Self::Command);
    fn poll_event(&mut self) -> Option<Self::Event>;
    fn stats(&self) -> Self::Stats;
    fn close(&mut self);
}

/// What the host game asks of its network thread.
#[derive(Clone, Debug)]
pub enum HostCommand {
    /// The host player's pad and the confirmed tick it was viewing.
    SubmitInput(PadState, u32),
    /// [`HostCore::submit_input_for`]: one of the host's own players' pad
    /// (`slot` 0 or a host local) and the confirmed tick it was viewing.
    SubmitInputFor {
        slot: u8,
        pad: PadState,
        view_tick: u32,
    },
    /// Every host local's pad of one frame (`(slot, pad)`, slot 0 first)
    /// and the confirmed tick they were viewing, applied in one go: the
    /// network thread can close a tick between two commands, so one
    /// command per slot could split a frame's pads across two ticks and
    /// put the locals' queues out of step (`HostCore::submit_input_for`).
    SubmitInputsFor {
        pads: Vec<(u8, PadState)>,
        view_tick: u32,
    },
    SetLobby(Vec<u8>),
    SetInfo(Vec<u8>),
    /// [`HostCore::load_round`]: the opaque setup and the slots it seats.
    LoadRound {
        setup: Vec<u8>,
        humans: Vec<u8>,
    },
    StartRound,
    EndRound,
    /// [`HostCore::set_profile`]: the profile `slot`'s later `AddHuman`
    /// carries.
    SetProfile {
        slot: u8,
        profile: Vec<u8>,
    },
    AddHuman {
        slot: u8,
        team: u8,
    },
    IssueCommand(Vec<u8>),
    ReportHash {
        tick: u32,
        hash: u64,
    },
    /// [`HostCore::tell_desync`].
    TellDesync {
        slot: u8,
        tick: u32,
        host_hash: u64,
    },
    Kick {
        slot: u8,
        ban: bool,
    },
    /// [`HostCore::bench`]: out of the round, still connected.
    Bench(u8),
    Notice {
        slot: Option<u8>,
        data: Vec<u8>,
    },
    Chat(String),
    /// [`HostCore::set_local_away`]: a host local's pad went or came back.
    SetLocalAway {
        slot: u8,
        away: bool,
    },
    /// [`HostCore::add_local_event`]: a further player of the host's own
    /// machine (Players-screen `seat`), answered by
    /// [`crate::NetEvent::LocalAdded`]. Send it between frames, before the
    /// frame's inputs (splitscreen B19).
    AddLocal {
        seat: u8,
        name: String,
        profile: Vec<u8>,
    },
}

/// What the client game asks of its network thread.
#[derive(Clone, Debug)]
pub enum ClientCommand {
    /// The pad and the confirmed tick its player was viewing.
    SubmitInput(PadState, u32),
    /// [`ClientCore::submit_inputs`]: every local player's pad under one
    /// sequence, and the confirmed tick they were viewing.
    SubmitInputs(Vec<PadState>, u32),
    Loaded {
        round_id: u32,
        static_digest: u64,
        hash0: u64,
    },
    CaughtUp(u32),
    ReportHash {
        tick: u32,
        hash: u64,
    },
    /// Our hash rows for the host's desync report
    /// ([`ClientCore::desync_hashes`]).
    DesyncHashes {
        tick: u32,
        first_tick: u32,
        width: u8,
        values: Vec<u64>,
    },
    RequestResync,
    Ready {
        round_id: u32,
        profile: Vec<u8>,
    },
    /// [`ClientCore::ready_local`]: another local player is ready.
    ReadyLocal {
        local: u8,
        round_id: u32,
        profile: Vec<u8>,
    },
    Request(Vec<u8>),
    Chat(String),
    Leave,
    /// [`ClientCore::set_local_away`]: a local's pad went or came back.
    SetLocalAway {
        local: u8,
        away: bool,
    },
}

impl SessionCore for HostCore {
    type Command = HostCommand;
    type Event = NetEvent;
    type Stats = HostStats;

    fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        HostCore::handle_datagram(self, now, from, bytes);
    }

    fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        HostCore::update(self, now)
    }

    fn apply(&mut self, cmd: HostCommand) {
        match cmd {
            HostCommand::SubmitInput(p, view) => self.submit_input(p, view),
            HostCommand::SubmitInputFor {
                slot,
                pad,
                view_tick,
            } => self.submit_input_for(slot, pad, view_tick),
            HostCommand::SubmitInputsFor { pads, view_tick } => {
                for (slot, pad) in pads {
                    self.submit_input_for(slot, pad, view_tick);
                }
            }
            HostCommand::SetLobby(s) => self.set_lobby(s),
            HostCommand::SetInfo(i) => self.set_info(i),
            HostCommand::LoadRound { setup, humans } => {
                self.load_round(setup, &humans);
            }
            HostCommand::StartRound => self.start_round(),
            HostCommand::EndRound => self.end_round(),
            HostCommand::SetProfile { slot, profile } => {
                self.set_profile(slot, profile);
            }
            HostCommand::AddHuman { slot, team } => {
                self.add_human(slot, team);
            }
            HostCommand::IssueCommand(c) => {
                self.issue_command(c);
            }
            HostCommand::ReportHash { tick, hash } => self.report_hash(tick, hash),
            HostCommand::TellDesync {
                slot,
                tick,
                host_hash,
            } => self.tell_desync(slot, tick, host_hash),
            HostCommand::Kick { slot, ban } => self.kick(slot, ban),
            HostCommand::Bench(slot) => {
                self.bench(slot);
            }
            HostCommand::Notice { slot, data } => self.send_notice(slot, data),
            HostCommand::Chat(t) => self.chat(&t),
            HostCommand::SetLocalAway { slot, away } => {
                self.set_local_away(slot, away);
            }
            HostCommand::AddLocal {
                seat,
                name,
                profile,
            } => self.add_local_event(seat, &name, profile),
        }
    }

    fn poll_event(&mut self) -> Option<NetEvent> {
        HostCore::poll_event(self)
    }

    fn stats(&self) -> HostStats {
        HostCore::stats(self)
    }

    fn close(&mut self) {
        HostCore::close(self);
    }
}

impl SessionCore for ClientCore {
    type Command = ClientCommand;
    type Event = ClientEvent;
    type Stats = ClientStats;

    fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        ClientCore::handle_datagram(self, now, from, bytes);
    }

    fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        ClientCore::update(self, now)
    }

    fn apply(&mut self, cmd: ClientCommand) {
        match cmd {
            ClientCommand::SubmitInput(p, view) => {
                self.submit_input(p, view);
            }
            ClientCommand::SubmitInputs(pads, view) => {
                self.submit_inputs(&pads, view);
            }
            ClientCommand::Loaded {
                round_id,
                static_digest,
                hash0,
            } => self.loaded(round_id, static_digest, hash0),
            ClientCommand::CaughtUp(t) => self.caught_up(t),
            ClientCommand::ReportHash { tick, hash } => self.report_hash(tick, hash),
            ClientCommand::DesyncHashes {
                tick,
                first_tick,
                width,
                values,
            } => self.desync_hashes(tick, first_tick, width, values),
            ClientCommand::RequestResync => self.request_resync(),
            ClientCommand::Ready { round_id, profile } => self.ready(round_id, profile),
            ClientCommand::ReadyLocal {
                local,
                round_id,
                profile,
            } => self.ready_local(local, round_id, profile),
            ClientCommand::Request(d) => self.request(d),
            ClientCommand::Chat(t) => self.chat(&t),
            ClientCommand::Leave => self.leave(),
            ClientCommand::SetLocalAway { local, away } => self.set_local_away(local, away),
        }
    }

    fn poll_event(&mut self) -> Option<ClientEvent> {
        ClientCore::poll_event(self)
    }

    fn stats(&self) -> ClientStats {
        ClientCore::stats(self)
    }

    fn close(&mut self) {
        if self.slot().is_some() {
            self.leave();
        }
    }
}

enum Ctl<T> {
    Cmd(T),
    Shutdown,
}

/// A running session: the network thread and its channels.
pub struct NetSession<C: SessionCore> {
    tx: Sender<Ctl<C::Command>>,
    rx: Receiver<C::Event>,
    stats: Arc<Mutex<C::Stats>>,
    thread: Option<JoinHandle<()>>,
}

impl NetSession<HostCore> {
    /// Hosts on `transport` with the real clock.
    pub fn host(config: HostConfig, transport: impl Transport + Send + 'static) -> Self {
        Self::spawn(HostCore::new(config), transport, Arc::new(RealClock::new()))
    }
}

impl NetSession<ClientCore> {
    /// Joins `host` on `transport` with the real clock.
    pub fn join(
        config: ClientConfig,
        host: SocketAddr,
        transport: impl Transport + Send + 'static,
    ) -> Self {
        let clock: SharedClock = Arc::new(RealClock::new());
        let core = ClientCore::new(config, host, clock.now());
        Self::spawn(core, transport, clock)
    }
}

impl<C: SessionCore> NetSession<C> {
    /// Runs `core` on its own thread. `clock` must be the one `core` was
    /// created against.
    pub fn spawn(core: C, transport: impl Transport + Send + 'static, clock: SharedClock) -> Self {
        let (tx, crx) = channel();
        let (etx, rx) = channel();
        let stats = Arc::new(Mutex::new(C::Stats::default()));
        let shared = Arc::clone(&stats);
        let thread = std::thread::Builder::new()
            .name("openbf-net".into())
            .spawn(move || run(core, transport, clock, crx, etx, shared))
            .expect("spawn network thread");
        Self {
            tx,
            rx,
            stats,
            thread: Some(thread),
        }
    }

    /// Queues a command for the network thread.
    pub fn send(&self, cmd: C::Command) {
        let _ = self.tx.send(Ctl::Cmd(cmd));
    }

    /// The next event, if one waits.
    pub fn try_event(&self) -> Option<C::Event> {
        self.rx.try_recv().ok()
    }

    /// Every waiting event.
    pub fn drain_events(&self) -> Vec<C::Event> {
        self.rx.try_iter().collect()
    }

    /// The latest published stats (at most 50 ms old).
    pub fn stats(&self) -> C::Stats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Says goodbye and stops the thread.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        let _ = self.tx.send(Ctl::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl<C: SessionCore> Drop for NetSession<C> {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run<C: SessionCore>(
    mut core: C,
    mut transport: impl Transport,
    clock: SharedClock,
    commands: Receiver<Ctl<C::Command>>,
    events: Sender<C::Event>,
    stats: Arc<Mutex<C::Stats>>,
) {
    let mut buf = vec![0u8; crate::MAX_DATAGRAM + 1];
    let mut closing: Option<Micros> = None;
    let mut last_stats = Micros::ZERO;
    loop {
        let now = clock.now();
        if closing.is_none() {
            loop {
                match commands.try_recv() {
                    Ok(Ctl::Cmd(c)) => core.apply(c),
                    Ok(Ctl::Shutdown) | Err(TryRecvError::Disconnected) => {
                        core.close();
                        closing = Some(now + CLOSE_FLUSH);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
        while let Ok(Some((n, from))) = transport.recv(&mut buf) {
            core.handle_datagram(now, from, &buf[..n]);
        }
        for (to, bytes) in core.update(now) {
            let _ = transport.send(to, &bytes);
        }
        while let Some(e) = core.poll_event() {
            let _ = events.send(e);
        }
        if now.0 >= last_stats.0 + STATS_EVERY.0 {
            last_stats = now;
            if let Ok(mut s) = stats.lock() {
                *s = core.stats();
            }
        }
        if closing.is_some_and(|t| now >= t) {
            return;
        }
        std::thread::sleep(POLL);
    }
}
