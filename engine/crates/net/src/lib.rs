//! LAN multiplayer transport and sessions: host-ordered deterministic
//! lockstep (`analysis/plan/LAN_MULTIPLAYER.md`, issue #168).
//!
//! Every peer runs the same `Sim` from the same per-tick [`TickBundle`]s. The
//! host's network thread closes a bundle every 1/30 s from each human slot's
//! input queue and streams it to every client; nothing else about the battle
//! is sent. This crate is generic over game payloads: round setup, lobby
//! state, content manifests, profiles and game commands are opaque bytes.
//!
//! Layout (each module's header holds the rules it owns):
//! - [`wire`], [`packet`], [`bundle`], [`msg`]: encodings, range-checked.
//! - [`conn`]: sequence/acks, RTT and the reliable ordered channel.
//! - [`scheduler`]: the host's per-slot input FIFO.
//! - [`host`], [`client`]: the clock-injected session state machines
//!   ([`HostCore`], [`ClientCore`]); all time is a [`Micros`] the caller
//!   passes in, so tests run in virtual time.
//! - [`discovery`]: `Find`/`Found` and [`LanBrowser`].
//! - [`transport`], [`linksim`]: real UDP and the seeded in-memory network.
//! - [`session`]: [`NetSession`], one network thread per session.
//! - [`pacer`]: game-thread [`InputPacer`] and [`Playback`].
//!
//! Crate rules:
//! - No async runtime, no new networking crate: `std::net::UdpSocket`.
//! - The cores never read a clock and keep peers in ordered maps, so a
//!   virtual-time run is reproducible from its seeds.
//! - Every datagram is at most [`packet::MAX_DATAGRAM`] bytes; anything
//!   malformed is dropped and counted, never trusted.

pub mod bundle;
pub mod client;
pub mod conn;
pub mod discovery;
pub mod host;
pub mod linksim;
pub mod msg;
pub mod pacer;
pub mod packet;
pub mod scheduler;
pub mod session;
pub mod transport;
pub mod wire;

#[cfg(test)]
mod tests;

pub use bundle::{SessionCommand, TickBundle, MAX_SLOTS, TEAM_AUTO};
pub use client::{ClientConfig, ClientCore, ClientEvent, ClientState, ClientStats};
pub use discovery::{FoundHost, LanBrowser};
pub use host::{
    HostConfig, HostCore, HostStats, LeaveReason, NetEvent, PeerStats, DEFAULT_LAG_COMP_CAP,
};
pub use linksim::{LinkParams, LinkSim, LinkSimTransport};
pub use msg::{RejectReason, RosterEntry};
pub use pacer::{InputPacer, Playback};
pub use packet::{DEFAULT_PORT, MAX_DATAGRAM, PROTOCOL};
pub use session::{ClientCommand, HostCommand, NetSession, SessionCore};
pub use transport::{Transport, UdpTransport};

use std::ops::{Add, Sub};

/// The sim's tick rate.
pub const TICK_HZ: f64 = 30.0;
/// Peers send at least this often.
pub const HEARTBEAT: Micros = Micros::from_millis(100);
/// Silence after which a connection is "having trouble".
pub const TROUBLE_AFTER: Micros = Micros::from_millis(1500);
/// Silence after which a connection is dropped.
pub const DROP_AFTER: Micros = Micros::from_secs(12);
/// Silence allowed during a load barrier.
pub const DROP_AFTER_LOADING: Micros = Micros::from_secs(60);
/// How long the load barrier waits before starting without late peers.
pub const BARRIER_TIMEOUT: Micros = Micros::from_secs(60);
/// How long a departed player's slot and name stay reserved for their token.
pub const RESERVE_FOR: Micros = Micros::from_secs(90);

/// A point in time, in microseconds since a session-chosen origin. The
/// cores take it as an argument; only [`Clock`] implementations read time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Micros(pub u64);

impl Micros {
    pub const ZERO: Micros = Micros(0);

    pub const fn from_millis(ms: u64) -> Self {
        Micros(ms * 1000)
    }

    pub const fn from_secs(s: u64) -> Self {
        Micros(s * 1_000_000)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e6
    }

    pub fn as_millis_f64(self) -> f64 {
        self.0 as f64 / 1e3
    }

    pub fn saturating_sub(self, o: Micros) -> Micros {
        Micros(self.0.saturating_sub(o.0))
    }

    /// The start of tick `t` for a clock whose tick 0 starts at `self`.
    pub fn tick_time(self, t: u32) -> Micros {
        Micros(self.0 + u64::from(t) * 1_000_000 / 30)
    }
}

impl Add for Micros {
    type Output = Micros;
    fn add(self, o: Micros) -> Micros {
        Micros(self.0 + o.0)
    }
}

impl Sub for Micros {
    type Output = Micros;
    fn sub(self, o: Micros) -> Micros {
        Micros(self.0.saturating_sub(o.0))
    }
}

/// A source of [`Micros`].
pub trait Clock {
    fn now(&self) -> Micros;
}

/// A clock shared between threads (a session thread and its transport).
pub type SharedClock = std::sync::Arc<dyn Clock + Send + Sync>;

/// The real monotonic clock, from its creation.
#[derive(Clone, Copy, Debug)]
pub struct RealClock {
    origin: std::time::Instant,
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl RealClock {
    pub fn new() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

impl Clock for RealClock {
    fn now(&self) -> Micros {
        Micros(self.origin.elapsed().as_micros() as u64)
    }
}

/// This machine's OS and CPU (`linux-x86_64`), for diagnostics only: a
/// desync report names both peers' (LAN §7: never a reason to refuse).
pub fn local_target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// A fresh random 64-bit value for connection ids, session tokens and nonces (not
/// for anything the sim reads).
pub fn fresh_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        h.write_u128(d.as_nanos());
    }
    h.write_u32(std::process::id());
    h.finish()
}

/// A fresh nonzero connection id.
pub fn fresh_conn_id() -> u32 {
    loop {
        let v = fresh_u64() as u32;
        if v != 0 {
            return v;
        }
    }
}
