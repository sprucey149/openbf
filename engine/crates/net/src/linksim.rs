//! An in-memory network for tests and the soak harness.
//!
//! Rules:
//! - Deterministic: one seeded LCG decides every loss, delay, duplicate and
//!   reorder, in send order; the same seed and the same sends give the same
//!   deliveries. Time is whatever `now` the caller passes.
//! - Each directed link has [`LinkParams`]: one-way latency, uniform jitter
//!   (±), loss, duplication and reorder probabilities. Jitter alone already
//!   reorders; `reorder` adds an extra delay of up to one latency.
//! - A datagram to `255.255.255.255:<port>` reaches every endpoint on that
//!   port except the sender. A datagram to an unknown address vanishes.
//! - [`LinkSim::cut`] partitions an endpoint: everything to or from it is
//!   lost until it is restored (for timeout tests).
//! - [`LinkSim::set_uplink`] gives an endpoint a bottleneck uplink: its
//!   datagrams leave one after another at that byte rate, in send order,
//!   before the link's latency (a home upload queue; unset is unlimited).

use crate::transport::Transport;
use crate::Micros;
use crate::SharedClock;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// One direction of a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkParams {
    pub latency: Micros,
    pub jitter: Micros,
    pub loss: f64,
    pub duplicate: f64,
    pub reorder: f64,
}

impl Default for LinkParams {
    fn default() -> Self {
        Self::ideal(Micros::from_millis(1))
    }
}

impl LinkParams {
    /// A clean link with a fixed latency.
    pub fn ideal(latency: Micros) -> Self {
        Self {
            latency,
            jitter: Micros::ZERO,
            loss: 0.0,
            duplicate: 0.0,
            reorder: 0.0,
        }
    }
}

/// The small LCG behind every random choice (Knuth's MMIX constants).
#[derive(Clone, Copy, Debug)]
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 ^ (self.0 >> 29)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in [0, n].
    pub fn upto(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % (n + 1)
        }
    }
}

type Flight = Reverse<(Micros, u64, SocketAddr, SocketAddr, Vec<u8>)>;

/// The simulated network.
pub struct LinkSim {
    rng: Lcg,
    default: LinkParams,
    links: BTreeMap<(SocketAddr, SocketAddr), LinkParams>,
    in_flight: BinaryHeap<Flight>,
    inboxes: BTreeMap<SocketAddr, VecDeque<(SocketAddr, Vec<u8>)>>,
    cut: BTreeSet<SocketAddr>,
    /// Bottleneck uplinks: bytes per second and when the queue drains.
    uplinks: BTreeMap<SocketAddr, (u64, Micros)>,
    order: u64,
    now: Micros,
    pub sent: u64,
    pub lost: u64,
    pub delivered: u64,
}

impl LinkSim {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Lcg(seed ^ 0x9e37_79b9_7f4a_7c15),
            default: LinkParams::default(),
            links: BTreeMap::new(),
            in_flight: BinaryHeap::new(),
            inboxes: BTreeMap::new(),
            cut: BTreeSet::new(),
            uplinks: BTreeMap::new(),
            order: 0,
            now: Micros::ZERO,
            sent: 0,
            lost: 0,
            delivered: 0,
        }
    }

    pub fn now(&self) -> Micros {
        self.now
    }

    /// Parameters for every link without its own.
    pub fn set_default(&mut self, p: LinkParams) {
        self.default = p;
    }

    /// Parameters for `from → to`.
    pub fn set_link(&mut self, from: SocketAddr, to: SocketAddr, p: LinkParams) {
        self.links.insert((from, to), p);
    }

    /// Parameters for both directions between `a` and `b`.
    pub fn set_both(&mut self, a: SocketAddr, b: SocketAddr, p: LinkParams) {
        self.set_link(a, b, p);
        self.set_link(b, a, p);
    }

    /// Registers an address that can receive.
    pub fn add_endpoint(&mut self, addr: SocketAddr) {
        self.inboxes.entry(addr).or_default();
    }

    /// Cuts (or restores) an endpoint off the network.
    pub fn cut(&mut self, addr: SocketAddr, cut: bool) {
        if cut {
            self.cut.insert(addr);
        } else {
            self.cut.remove(&addr);
        }
    }

    /// Limits `addr`'s uplink to `bytes_per_sec` (module rules).
    pub fn set_uplink(&mut self, addr: SocketAddr, bytes_per_sec: u64) {
        self.uplinks
            .insert(addr, (bytes_per_sec.max(1), Micros::ZERO));
    }

    /// Sends a datagram at `now`.
    pub fn send(&mut self, now: Micros, from: SocketAddr, to: SocketAddr, bytes: &[u8]) {
        let broadcast = matches!(to, SocketAddr::V4(v) if v.ip().is_broadcast());
        if broadcast {
            let targets: Vec<SocketAddr> = self
                .inboxes
                .keys()
                .filter(|a| a.port() == to.port() && **a != from)
                .copied()
                .collect();
            for t in targets {
                self.send_one(now, from, t, bytes);
            }
        } else {
            self.send_one(now, from, to, bytes);
        }
    }

    fn send_one(&mut self, now: Micros, from: SocketAddr, to: SocketAddr, bytes: &[u8]) {
        self.sent += 1;
        let p = self.links.get(&(from, to)).copied().unwrap_or(self.default);
        // The uplink queue: lost datagrams still used the bandwidth.
        let now = match self.uplinks.get_mut(&from) {
            Some((rate, free_at)) => {
                let wire = bytes.len() as u64 * 1_000_000 / *rate;
                *free_at = Micros(now.0.max(free_at.0) + wire);
                *free_at
            }
            None => now,
        };
        if self.rng.unit() < p.loss {
            self.lost += 1;
            return;
        }
        let copies = if self.rng.unit() < p.duplicate { 2 } else { 1 };
        for _ in 0..copies {
            let j = self.rng.upto(2 * p.jitter.0) as i64 - p.jitter.0 as i64;
            let mut delay = (p.latency.0 as i64 + j).max(0) as u64;
            if self.rng.unit() < p.reorder {
                delay += self.rng.upto(p.latency.0);
            }
            self.order += 1;
            self.in_flight.push(Reverse((
                Micros(now.0 + delay),
                self.order,
                from,
                to,
                bytes.to_vec(),
            )));
        }
    }

    /// Delivers everything due by `now` into the inboxes.
    pub fn advance(&mut self, now: Micros) {
        self.now = self.now.max(now);
        while let Some(Reverse((at, ..))) = self.in_flight.peek() {
            if *at > self.now {
                break;
            }
            let Reverse((_, _, from, to, bytes)) = self.in_flight.pop().expect("peeked");
            if self.cut.contains(&from) || self.cut.contains(&to) {
                self.lost += 1;
                continue;
            }
            if let Some(q) = self.inboxes.get_mut(&to) {
                self.delivered += 1;
                q.push_back((from, bytes));
            }
        }
    }

    /// The next datagram delivered to `at`.
    pub fn recv(&mut self, at: SocketAddr) -> Option<(SocketAddr, Vec<u8>)> {
        self.inboxes.get_mut(&at)?.pop_front()
    }
}

/// A [`Transport`] onto a shared [`LinkSim`], timed by a shared clock (for
/// running [`crate::NetSession`]s in one process).
pub struct LinkSimTransport {
    sim: Arc<Mutex<LinkSim>>,
    addr: SocketAddr,
    clock: SharedClock,
}

impl LinkSimTransport {
    pub fn new(sim: Arc<Mutex<LinkSim>>, addr: SocketAddr, clock: SharedClock) -> Self {
        sim.lock().expect("linksim lock").add_endpoint(addr);
        Self { sim, addr, clock }
    }
}

impl Transport for LinkSimTransport {
    fn send(&mut self, to: SocketAddr, bytes: &[u8]) -> io::Result<()> {
        let now = self.clock.now();
        self.sim
            .lock()
            .map_err(|_| io::Error::other("linksim poisoned"))?
            .send(now, self.addr, to, bytes);
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        let now = self.clock.now();
        let mut sim = self
            .sim
            .lock()
            .map_err(|_| io::Error::other("linksim poisoned"))?;
        sim.advance(now);
        Ok(sim.recv(self.addr).map(|(from, b)| {
            let n = b.len().min(buf.len());
            buf[..n].copy_from_slice(&b[..n]);
            (n, from)
        }))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.addr)
    }
}
