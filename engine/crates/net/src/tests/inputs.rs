//! Input streams of a machine with several local players (PROTOCOL 12,
//! splitscreen packet B15): one sequence for all locals, routed by the host
//! to each local's slot, acknowledged through the slowest of them.

use super::harness::*;
use crate::*;

fn lossy() -> LinkParams {
    LinkParams {
        latency: Micros::from_millis(40),
        jitter: Micros::from_millis(10),
        loss: 0.03,
        duplicate: 0.0,
        reorder: 0.0,
    }
}

/// A one-local client's datagrams are byte for byte what they were before
/// several locals could share a machine (hash pinned on the build before
/// `LocalInputs`).
#[test]
fn inputs_one_local_datagrams_unchanged() {
    let mut net = Net::new(5, lossy(), Net::host_config());
    net.join(1, "Solo", 101, Player::new(7.0, 1.0, 6));
    net.join(2, "Other", 102, Player::new(16.7, 1.0004, 7));
    net.connect_all();
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 600);
    let mut hashes: Vec<u64> = net.clients.iter().map(|c| c.sent_hash).collect();
    hashes.push(net.host_sent_hash);
    println!("one-local datagram hashes {hashes:x?}");
    assert_eq!(
        hashes,
        vec![
            0x9a89_c5fa_efd3_90c5,
            0x4c13_b833_482b_b6cc,
            0xa817_94fa_32a5_0ce7
        ]
    );
}

/// A two-local machine: every input of both locals reaches its own slot
/// over a 3 % lossy link, and the host consumes both at the same sequence
/// on every tick (they share one sequence and one datagram).
#[test]
fn inputs_two_locals_consumed_at_the_same_sequences() {
    let mut net = Net::new(13, lossy(), Net::host_config());
    net.join(1, "Solo", 101, Player::new(16.7, 1.0004, 6));
    let mut c = ClientConfig::new(BUILD, "Left", 102);
    c.locals = vec![LocalHello {
        seat: 1,
        name: "Right".to_owned(),
        profile: Vec::new(),
    }];
    let duo = net.join_with(2, c, Player::new(7.0, 0.9996, 7));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![2, 3]);
    net.start_round();
    net.run_until(120_000, |n| n.host.next_tick() >= 2_000);
    net.host_player.playing = false;
    for c in &mut net.clients {
        c.player.playing = false;
    }
    net.run_ms(3_000);

    let host = &net.host_bundles;
    let duo = &net.clients[duo];
    assert_eq!(duo.bundles[..], host[..duo.bundles.len()]);
    let (a, b) = (2u8, 3u8);
    let sent = [&duo.player.submitted, &duo.extra_submitted[0]];
    assert_eq!(sent[0].len(), sent[1].len());
    assert!(sent[0].len() > 1_500, "sent {}", sent[0].len());
    for (slot, sent) in [(a, sent[0]), (b, sent[1])] {
        let got = slot_pads(host, slot);
        let sum = |v: &[core::sim::PadState]| -> (i64, i64, i64) {
            v.iter().fold((0, 0, 0), |s, p| {
                (
                    s.0 + i64::from(p.mx),
                    s.1 + i64::from(p.my),
                    s.2 + i64::from(p.lx),
                )
            })
        };
        assert_eq!(sum(&got).0, sum(sent).0, "slot {slot} mouse x");
        assert_eq!(sum(&got).1, sum(sent).1, "slot {slot} mouse y");
        assert_eq!(
            edges(got.iter(), core::sim::buttons::FIRE),
            edges(sent.iter(), core::sim::buttons::FIRE),
            "slot {slot} FIRE presses"
        );
    }
    // The two locals' pads differ, so a mix-up would show above.
    assert_ne!(slot_pads(host, a), slot_pads(host, b));
    let mut live = 0;
    for t in host {
        let (ca, cb) = (t.consumed(a), t.consumed(b));
        assert_eq!(ca, cb, "tick {} consumed {ca:?} vs {cb:?}", t.tick);
        live += usize::from(ca.is_some_and(|s| s > 0));
    }
    assert!(live > 1_500, "{live} ticks consumed input");
    let st = net.host.stats();
    assert_eq!((st.drops.malformed, st.drops.bad_chunk), (0, 0));
    assert_eq!(duo.core.stats().unacked_inputs, 0);
}

/// A local seated after its sibling (here: taken out and seated again
/// mid-round, as a lost pad and its return will be) lines up with the
/// sibling on the seating tick: both consume the same sequence on every
/// tick after it, though the sibling held queued inputs then.
#[test]
fn inputs_late_seated_local_consumes_with_its_sibling() {
    let mut net = Net::new(19, lossy(), Net::host_config());
    let mut c = ClientConfig::new(BUILD, "Left", 102);
    c.locals = vec![LocalHello {
        seat: 1,
        name: "Right".to_owned(),
        profile: Vec::new(),
    }];
    let duo = net.join_with(1, c, Player::new(7.0, 0.9996, 7));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    let (a, b) = (1u8, 2u8);
    assert_eq!(net.clients[duo].core.slots(), vec![a, b]);
    net.start_round();
    let mut reseated = Vec::new();
    for round in 0..6u32 {
        let at = net.host.next_tick() + 150 + 37 * round;
        net.run_until(60_000, |n| n.host.next_tick() >= at);
        net.host.unseat(b);
        let back = net.host.next_tick() + 20 + 13 * round;
        net.run_until(60_000, |n| n.host.next_tick() >= back);
        assert!(net.host.add_human(b, 0));
        let depth = net.host.queue_depth(a).unwrap_or(0);
        net.run_until(60_000, |n| n.host.is_active(b));
        reseated.push((net.host.next_tick(), depth));
    }
    let end = net.host.next_tick() + 300;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    println!("reseated at (tick, sibling depth) {reseated:?}");
    assert!(
        reseated.iter().any(|(_, d)| *d > 0),
        "the sibling never held queued inputs at a seating"
    );
    let mut both = 0;
    for t in &net.host_bundles {
        if let (Some(ca), Some(cb)) = (t.consumed(a), t.consumed(b)) {
            assert_eq!(ca, cb, "tick {} consumed {ca} vs {cb}", t.tick);
            both += 1;
        }
    }
    assert!(both > 1_000, "{both} ticks with both seated");
    let st = net.host.stats();
    assert_eq!((st.drops.malformed, st.drops.bad_chunk), (0, 0));
}

/// A `LocalInputs` for a local the machine does not have is a bad chunk;
/// nothing reaches any slot.
#[test]
fn inputs_unknown_local_is_a_bad_chunk() {
    let mut net = Net::new(17, lossy(), Net::host_config());
    let solo = net.join(1, "Solo", 101, Player::new(7.0, 1.0, 6));
    net.connect_all();
    let conn = net.clients[solo].core.config().conn_id;
    let h = crate::packet::Header {
        protocol: crate::packet::PROTOCOL,
        conn,
        seq: 900,
        ack: 0,
        ack_bits: 0,
    };
    let mut pw = crate::packet::PacketWriter::new(&h);
    let pad = core::sim::PadState::default();
    let (body, _) = crate::packet::encode_inputs(0, &pad, &[(pad, 0)], 200);
    pw.local_inputs(1, &body);
    let before = net.host.stats().drops.bad_chunk;
    let now = net.now;
    net.host.handle_datagram(now, addr(1), &pw.finish());
    assert_eq!(net.host.stats().drops.bad_chunk, before + 1);
}
