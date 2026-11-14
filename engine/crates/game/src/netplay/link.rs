//! [`Link`]: what [`super::Peer`] needs of a session, over a bare
//! [`HostCore`] / [`ClientCore`] in virtual time or a [`NetSession`] on its
//! network thread. A guest-only call is a no-op on the host and the other
//! way round.

use core::sim::PadState;
use net::{
    ClientCommand, ClientCore, ClientEvent, HostCommand, HostCore, NetEvent, NetSession,
    SessionCore,
};

/// A session event, from either side.
#[derive(Clone, Debug)]
pub enum LinkEvent {
    Host(NetEvent),
    Client(ClientEvent),
}

/// What [`Peer`] needs of a session: a bare core in virtual time, or a
/// [`NetSession`] on its network thread.
pub trait Link {
    fn is_host(&self) -> bool;
    fn poll(&mut self) -> Option<LinkEvent>;
    /// `view_tick`: the confirmed tick the player was looking at (the
    /// local sim's tick), which the host turns into `PadState::view_lag`.
    fn submit_input(&mut self, pad: PadState, view_tick: u32);
    /// This peer's queue depth and starvation count, from the host.
    fn pacing(&self) -> (u8, u32);
    /// This peer's input slack the host published (µs, `net::scheduler`).
    fn slack_us(&self) -> Option<i64>;
    /// Live-bundle arrival jitter in ticks (0 on the host).
    fn jitter_ticks(&self) -> f64;
    fn report_hash(&mut self, tick: u32, hash: u64);
    /// A guest's `Loaded` (a no-op on the host).
    fn loaded(&mut self, round_id: u32, static_digest: u64, hash0: u64);
    /// A guest's `CaughtUp` (a no-op on the host).
    fn caught_up(&mut self, tick: u32);
    /// A guest's request (a no-op on the host).
    fn request(&mut self, data: Vec<u8>);
    fn request_resync(&mut self);
    /// A guest's hash rows for the host's desync report (a no-op on the
    /// host; `net::ClientCore::desync_hashes`).
    fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>);
    /// A host command (a no-op on a guest).
    fn host(&mut self, cmd: HostCommand);
}

impl Link for HostCore {
    fn is_host(&self) -> bool {
        true
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        self.poll_event().map(LinkEvent::Host)
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        HostCore::submit_input(self, pad, view_tick);
    }
    fn pacing(&self) -> (u8, u32) {
        HostCore::pacing(self, 0).unwrap_or((0, 0))
    }
    fn slack_us(&self) -> Option<i64> {
        HostCore::slack_us(self, 0)
    }
    fn jitter_ticks(&self) -> f64 {
        0.0
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        HostCore::report_hash(self, tick, hash);
    }
    fn loaded(&mut self, _: u32, _: u64, _: u64) {}
    fn caught_up(&mut self, _: u32) {}
    fn request(&mut self, _: Vec<u8>) {}
    fn request_resync(&mut self) {}
    fn desync_hashes(&mut self, _: u32, _: u32, _: u8, _: Vec<u64>) {}
    fn host(&mut self, cmd: HostCommand) {
        SessionCore::apply(self, cmd);
    }
}

impl Link for ClientCore {
    fn is_host(&self) -> bool {
        false
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        self.poll_event().map(LinkEvent::Client)
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        ClientCore::submit_input(self, pad, view_tick);
    }
    fn pacing(&self) -> (u8, u32) {
        ClientCore::pacing(self)
    }
    fn slack_us(&self) -> Option<i64> {
        ClientCore::slack_us(self)
    }
    fn jitter_ticks(&self) -> f64 {
        ClientCore::jitter_ticks(self)
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        ClientCore::report_hash(self, tick, hash);
    }
    fn loaded(&mut self, round_id: u32, static_digest: u64, hash0: u64) {
        ClientCore::loaded(self, round_id, static_digest, hash0);
    }
    fn caught_up(&mut self, tick: u32) {
        ClientCore::caught_up(self, tick);
    }
    fn request(&mut self, data: Vec<u8>) {
        ClientCore::request(self, data);
    }
    fn request_resync(&mut self) {
        ClientCore::request_resync(self);
    }
    fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>) {
        ClientCore::desync_hashes(self, tick, first_tick, width, values);
    }
    fn host(&mut self, _: HostCommand) {}
}

impl Link for NetSession<HostCore> {
    fn is_host(&self) -> bool {
        true
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        self.try_event().map(LinkEvent::Host)
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        self.send(HostCommand::SubmitInput(pad, view_tick));
    }
    fn pacing(&self) -> (u8, u32) {
        let s = self.stats();
        (s.local_depth, s.local_starved)
    }
    fn slack_us(&self) -> Option<i64> {
        self.stats().local_slack_us
    }
    fn jitter_ticks(&self) -> f64 {
        0.0
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        self.send(HostCommand::ReportHash { tick, hash });
    }
    fn loaded(&mut self, _: u32, _: u64, _: u64) {}
    fn caught_up(&mut self, _: u32) {}
    fn request(&mut self, _: Vec<u8>) {}
    fn request_resync(&mut self) {}
    fn desync_hashes(&mut self, _: u32, _: u32, _: u8, _: Vec<u64>) {}
    fn host(&mut self, cmd: HostCommand) {
        self.send(cmd);
    }
}

impl Link for NetSession<ClientCore> {
    fn is_host(&self) -> bool {
        false
    }
    fn poll(&mut self) -> Option<LinkEvent> {
        self.try_event().map(LinkEvent::Client)
    }
    fn submit_input(&mut self, pad: PadState, view_tick: u32) {
        self.send(ClientCommand::SubmitInput(pad, view_tick));
    }
    fn pacing(&self) -> (u8, u32) {
        let s = self.stats();
        (s.queue_depth, s.starved)
    }
    fn slack_us(&self) -> Option<i64> {
        self.stats().slack_us
    }
    fn jitter_ticks(&self) -> f64 {
        self.stats().jitter_ticks
    }
    fn report_hash(&mut self, tick: u32, hash: u64) {
        self.send(ClientCommand::ReportHash { tick, hash });
    }
    fn loaded(&mut self, round_id: u32, static_digest: u64, hash0: u64) {
        self.send(ClientCommand::Loaded {
            round_id,
            static_digest,
            hash0,
        });
    }
    fn caught_up(&mut self, tick: u32) {
        self.send(ClientCommand::CaughtUp(tick));
    }
    fn request(&mut self, data: Vec<u8>) {
        self.send(ClientCommand::Request(data));
    }
    fn request_resync(&mut self) {
        self.send(ClientCommand::RequestResync);
    }
    fn desync_hashes(&mut self, tick: u32, first_tick: u32, width: u8, values: Vec<u64>) {
        self.send(ClientCommand::DesyncHashes {
            tick,
            first_tick,
            width,
            values,
        });
    }
    fn host(&mut self, _: HostCommand) {}
}
