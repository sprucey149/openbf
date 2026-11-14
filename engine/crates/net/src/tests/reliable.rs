//! The reliable ordered channel over a hostile link.

use crate::conn::Conn;
use crate::linksim::{Lcg, LinkParams, LinkSim};
use crate::packet::{parse_chunks, parse_header};
use crate::{Micros, HEARTBEAT};
use std::net::SocketAddr;

fn pump(sim: &mut LinkSim, now: Micros, me: SocketAddr, conn: &mut Conn) -> Vec<Vec<u8>> {
    while let Some((_, b)) = sim.recv(me) {
        let (h, body) = parse_header(&b).unwrap();
        let chunks = parse_chunks(body).unwrap();
        conn.receive(now, &h, chunks, b.len());
    }
    let mut got = Vec::new();
    while let Some(m) = conn.take_message() {
        got.push(m);
    }
    if conn.wants_send(now, HEARTBEAT) {
        for p in conn.build(now, |_| {}) {
            assert!(p.len() <= crate::MAX_DATAGRAM);
            sim.send(now, me, conn.addr, &p);
        }
    }
    got
}

#[test]
fn reliable_delivers_in_order_under_loss_dup_and_reorder() {
    let a: SocketAddr = "10.0.0.1:1".parse().unwrap();
    let b: SocketAddr = "10.0.0.2:2".parse().unwrap();
    let mut sim = LinkSim::new(9);
    sim.set_default(LinkParams {
        latency: Micros::from_millis(20),
        jitter: Micros::from_millis(15),
        loss: 0.2,
        duplicate: 0.1,
        reorder: 0.2,
    });
    sim.add_endpoint(a);
    sim.add_endpoint(b);
    let mut ca = Conn::new(9, b, Micros::ZERO);
    let mut cb = Conn::new(9, a, Micros::ZERO);
    let mut r = Lcg(5);
    let sent: Vec<Vec<u8>> = (0..300)
        .map(|i| {
            let n = if i % 25 == 0 {
                5_000
            } else {
                r.upto(300) as usize
            };
            (0..n).map(|k| (k as u64 ^ i) as u8).collect()
        })
        .collect();
    let back: Vec<Vec<u8>> = (0..50).map(|i| vec![i as u8; 10]).collect();
    for m in &sent {
        ca.send_reliable(m);
    }
    for m in &back {
        cb.send_reliable(m);
    }
    let (mut at_b, mut at_a) = (Vec::new(), Vec::new());
    let mut now = Micros::ZERO;
    for _ in 0..60_000 {
        sim.advance(now);
        at_a.extend(pump(&mut sim, now, a, &mut ca));
        at_b.extend(pump(&mut sim, now, b, &mut cb));
        if at_b.len() == sent.len() && at_a.len() == back.len() && ca.reliable_idle() {
            break;
        }
        now = now + Micros::from_millis(1);
    }
    assert_eq!(at_b, sent);
    assert_eq!(at_a, back);
    assert!(ca.reliable_idle());
    assert!(ca.stats.resends > 0);
    assert!(cb.stats.duplicates > 0);
    // RTT settles near the 40 ms round trip.
    let rtt = ca.rtt.srtt as f64 / 1e3;
    assert!((30.0..=90.0).contains(&rtt), "rtt {rtt} ms");
}

/// A large message never puts more than [`crate::conn::MAX_IN_FLIGHT`] units
/// in flight, and unacked units back off instead of resending every RTO.
#[test]
fn reliable_flight_is_bounded_and_resends_back_off() {
    let peer: SocketAddr = "10.0.0.2:2".parse().unwrap();
    let mut c = Conn::new(9, peer, Micros::ZERO);
    c.send_reliable(&vec![7u8; 300_000]);
    let mut units = std::collections::BTreeSet::new();
    let mut chunks = 0usize;
    let mut now = Micros::ZERO;
    let send = |c: &mut Conn, now: Micros, units: &mut std::collections::BTreeSet<u32>| {
        let mut n = 0;
        if c.wants_send(now, HEARTBEAT) {
            for p in c.build(now, |_| {}) {
                let (_, body) = parse_header(&p).unwrap();
                for ch in parse_chunks(body).unwrap() {
                    if let crate::packet::Chunk::Reliable { unit, .. } = ch {
                        units.insert(unit);
                        n += 1;
                    }
                }
            }
        }
        n
    };
    for _ in 0..40 {
        chunks += send(&mut c, now, &mut units);
        now = now + Micros::from_millis(1);
    }
    assert_eq!(units.len(), crate::conn::MAX_IN_FLIGHT, "first burst");
    // Nothing is ever acked (a dead peer): 5 s of resends stay modest.
    for _ in 40..5_000 {
        chunks += send(&mut c, now, &mut units);
        now = now + Micros::from_millis(1);
    }
    assert_eq!(units.len(), crate::conn::MAX_IN_FLIGHT);
    assert!(
        chunks <= crate::conn::MAX_IN_FLIGHT * 8,
        "{chunks} reliable chunks in 5 s"
    );
}
