//! LAN discovery: `Find` / `Found` on connection id 0 at the host port.
//!
//! Rules:
//! - A [`LanBrowser`] sends `Find` once a second to each address the caller
//!   gives (by default the limited broadcast `255.255.255.255:37450`; the
//!   caller adds directed broadcasts and remembered hosts). Each `Find`
//!   carries a nonce; the `Found` echoing it gives that host's round trip.
//! - The host answers every `Find` with its game-set info blob (at most
//!   [`crate::host::MAX_INFO`] bytes): name, map, players and so on, opaque
//!   here.
//! - A host not heard for [`FORGET_AFTER`] leaves the list. Join by address
//!   is the primary path (tunnels carry no broadcast); discovery is a
//!   convenience. Over a tunnel the caller asks its remembered hosts by
//!   unicast ([`LanBrowser::set_targets`], which keeps the hosts heard).
//! - A host on another protocol version answers in discovery's frozen
//!   layout (`packet::frozen_found`): it is listed with its own
//!   [`FoundHost::protocol`], so the caller can show it as another
//!   version. A host whose build predates the frozen answer never replies
//!   to another version's `Find` and stays unlisted.

use crate::packet::{
    bare_packet, frozen_found, parse_chunks, parse_header, Chunk, DropReason, DEFAULT_PORT,
};
use crate::transport::Transport;
use crate::Micros;
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// How often the browser asks.
pub const FIND_EVERY: Micros = Micros::from_secs(1);
/// A host silent this long is forgotten.
pub const FORGET_AFTER: Micros = Micros::from_secs(5);

/// A host that answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundHost {
    pub addr: SocketAddr,
    pub info: Vec<u8>,
    pub rtt: Micros,
    pub last_seen: Micros,
    /// The protocol its answer came in: [`crate::PROTOCOL`], or another
    /// version's (it cannot be joined).
    pub protocol: u16,
}

/// Finds hosts on the local network.
#[derive(Debug)]
pub struct LanBrowser {
    targets: Vec<SocketAddr>,
    last_find: Option<Micros>,
    next_nonce: u64,
    sent: BTreeMap<u64, Micros>,
    hosts: BTreeMap<SocketAddr, FoundHost>,
    now: Micros,
}

impl LanBrowser {
    /// The limited broadcast on the default port.
    pub fn default_targets() -> Vec<SocketAddr> {
        vec![SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::BROADCAST,
            DEFAULT_PORT,
        ))]
    }

    /// A browser asking `targets` (empty: [`LanBrowser::default_targets`]).
    pub fn new(targets: Vec<SocketAddr>) -> Self {
        let targets = if targets.is_empty() {
            Self::default_targets()
        } else {
            targets
        };
        Self {
            targets,
            last_find: None,
            next_nonce: crate::fresh_u64() | 1,
            sent: BTreeMap::new(),
            hosts: BTreeMap::new(),
            now: Micros::ZERO,
        }
    }

    /// Asks `targets` from the next `Find` on (empty:
    /// [`LanBrowser::default_targets`]); the hosts already heard stay.
    pub fn set_targets(&mut self, targets: Vec<SocketAddr>) {
        self.targets = if targets.is_empty() {
            Self::default_targets()
        } else {
            targets
        };
    }

    /// The addresses each `Find` goes to.
    pub fn targets(&self) -> &[SocketAddr] {
        &self.targets
    }

    /// Hosts heard within [`FORGET_AFTER`], by address.
    pub fn hosts(&self) -> Vec<FoundHost> {
        self.hosts
            .values()
            .filter(|h| self.now.0 < h.last_seen.0 + FORGET_AFTER.0)
            .cloned()
            .collect()
    }

    /// The `Find` datagrams due now.
    pub fn update(&mut self, now: Micros) -> Vec<(SocketAddr, Vec<u8>)> {
        self.now = now;
        if self.last_find.is_some_and(|t| now.0 < t.0 + FIND_EVERY.0) {
            return Vec::new();
        }
        self.last_find = Some(now);
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(2);
        self.sent.insert(nonce, now);
        while self.sent.len() > 8 {
            self.sent.pop_first();
        }
        let mut pw = bare_packet(0);
        pw.find(nonce);
        let bytes = pw.finish();
        self.hosts
            .retain(|_, h| now.0 < h.last_seen.0 + FORGET_AFTER.0);
        self.targets.iter().map(|t| (*t, bytes.clone())).collect()
    }

    /// Takes a datagram from the browser's socket.
    pub fn handle_datagram(&mut self, now: Micros, from: SocketAddr, bytes: &[u8]) {
        self.now = now;
        let (h, body) = match parse_header(bytes) {
            Ok(x) => x,
            Err(DropReason::Protocol(_)) => {
                if let Some((protocol, nonce, info)) = frozen_found(bytes) {
                    self.heard(now, from, nonce, info, protocol);
                }
                return;
            }
            Err(_) => return,
        };
        if h.conn != 0 {
            return;
        }
        let Ok(chunks) = parse_chunks(body) else {
            return;
        };
        for c in chunks {
            if let Chunk::Found { nonce, info } = c {
                self.heard(now, from, nonce, info, h.protocol);
            }
        }
    }

    /// A `Found` for one of our nonces.
    fn heard(&mut self, now: Micros, from: SocketAddr, nonce: u64, info: &[u8], protocol: u16) {
        let Some(sent) = self.sent.get(&nonce) else {
            return;
        };
        let rtt = now.saturating_sub(*sent);
        self.hosts.insert(
            from,
            FoundHost {
                addr: from,
                info: info.to_vec(),
                rtt,
                last_seen: now,
                protocol,
            },
        );
    }

    /// Sends due `Find`s and takes every waiting reply through `t`.
    pub fn pump(&mut self, now: Micros, t: &mut dyn Transport) {
        for (to, b) in self.update(now) {
            let _ = t.send(to, &b);
        }
        let mut buf = [0u8; crate::MAX_DATAGRAM + 1];
        while let Ok(Some((n, from))) = t.recv(&mut buf) {
            self.handle_datagram(now, from, &buf[..n]);
        }
    }
}
