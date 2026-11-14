//! Handshakes, joins, leaves, reconnects and desync reports.

use super::harness::*;
use crate::*;

fn lan() -> LinkParams {
    LinkParams {
        latency: Micros::from_millis(5),
        jitter: Micros::from_millis(2),
        loss: 0.02,
        duplicate: 0.01,
        reorder: 0.01,
    }
}

fn rejected(c: &Client) -> Option<RejectReason> {
    c.events.iter().find_map(|e| match e {
        ClientEvent::Rejected { reason, .. } => Some(*reason),
        _ => None,
    })
}

#[test]
fn session_handshake_accepts_and_rejects() {
    let mut cfg = Net::host_config();
    cfg.max_players = 3;
    let mut net = Net::new(1, lan(), cfg);
    net.host.set_lobby(b"lobby-v1".to_vec());
    let a = net.join(1, "Alice", 1, Player::new(10.0, 1.0, 1));
    let mut wrong = ClientConfig::new("other-build", "Bob", 2);
    wrong.content = b"maps".to_vec();
    let b = net.join_with(2, wrong, Player::new(10.0, 1.0, 2));
    let c = net.join(3, "", 3, Player::new(10.0, 1.0, 3));
    let d = net.join(4, "Bad\u{7}Name", 4, Player::new(10.0, 1.0, 4));
    let e = net.join(5, "Erin", 5, Player::new(10.0, 1.0, 5));
    net.run_ms(1_000);
    let f = net.join(6, "Frank", 6, Player::new(10.0, 1.0, 6));
    net.run_ms(1_000);

    // Slots go lowest-free in Hello arrival order.
    let mut slots = [net.clients[a].core.slot(), net.clients[e].core.slot()];
    slots.sort();
    assert_eq!(slots, [Some(1), Some(2)]);
    assert!(net.clients[a]
        .events
        .iter()
        .any(|ev| matches!(ev, ClientEvent::Lobby { state, .. } if state == b"lobby-v1")));
    assert_eq!(rejected(&net.clients[b]), Some(RejectReason::Version));
    let detail = net.clients[b].events.iter().find_map(|ev| match ev {
        ClientEvent::Rejected { detail, .. } => Some(detail.clone()),
        _ => None,
    });
    assert!(detail.unwrap().contains("other-build"));
    assert_eq!(rejected(&net.clients[c]), Some(RejectReason::BadName));
    assert_eq!(rejected(&net.clients[d]), Some(RejectReason::BadName));
    assert_eq!(rejected(&net.clients[f]), Some(RejectReason::Full));
    let joined: Vec<u8> = net
        .host_events
        .iter()
        .filter_map(|(_, ev)| match ev {
            NetEvent::PlayerJoined { slot, .. } => Some(*slot),
            _ => None,
        })
        .collect();
    assert_eq!(joined.len(), 2);
    // Lobby changes reach guests whole, throttled.
    net.host.set_lobby(b"lobby-v2".to_vec());
    net.host.set_lobby(b"lobby-v3".to_vec());
    net.run_ms(500);
    let last = net.clients[a].events.iter().rev().find_map(|ev| match ev {
        ClientEvent::Lobby { state, .. } => Some(state.clone()),
        _ => None,
    });
    assert_eq!(last.unwrap(), b"lobby-v3");
}

#[test]
fn session_other_protocol_reports_version() {
    let mut net = Net::new(2, lan(), Net::host_config());
    // A peer on another protocol: the host answers with a bare header.
    let mut pkt = crate::packet::bare_packet(77).finish();
    pkt[4] = 9;
    net.sim.add_endpoint(addr(9));
    net.sim.send(net.now, addr(9), addr(0), &pkt);
    net.run_ms(50);
    let (_, reply) = net.sim.recv(addr(9)).expect("version answer");
    // A client from that version sees a protocol mismatch and stops.
    let mut c = ClientCore::new(
        ClientConfig {
            conn_id: 77,
            ..ClientConfig::new(BUILD, "Old", 1)
        },
        addr(0),
        Micros::ZERO,
    );
    let mut theirs = reply.clone();
    theirs[4] = crate::packet::PROTOCOL.wrapping_add(1) as u8;
    c.handle_datagram(Micros(10), addr(0), &theirs);
    assert_eq!(c.state(), ClientState::Rejected(RejectReason::Version));
    assert_eq!(net.host.stats().drops.wrong_protocol, 1);
}

#[test]
fn session_mid_round_join_gets_log_then_live_bundles() {
    let mut net = Net::new(3, lan(), Net::host_config());
    net.join(1, "A", 1, Player::new(8.0, 1.0, 1));
    net.join(2, "B", 2, Player::new(16.7, 1.0, 2));
    net.connect_all();
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 900);
    let j = net.join(3, "Late", 3, Player::new(9.0, 1.0, 3));
    net.run_until(10_000, |n| n.clients[j].core.slot().is_some());
    let log_len = net.clients[j]
        .events
        .iter()
        .find_map(|e| match e {
            ClientEvent::LoadRound {
                joining: true,
                log_len,
                ..
            } => Some(*log_len),
            _ => None,
        })
        .unwrap_or(0);
    // The LoadRound may still be in flight; wait for it and the whole log.
    net.run_until(10_000, |n| {
        n.clients[j].bundles.len() + 2 >= n.host_bundles.len() && n.clients[j].bundles.len() > 900
    });
    let log_len = log_len.max(
        net.clients[j]
            .events
            .iter()
            .find_map(|e| match e {
                ClientEvent::LoadRound { log_len, .. } => Some(*log_len),
                _ => None,
            })
            .unwrap(),
    );
    assert!(log_len >= 900);
    let slot = net.clients[j].core.slot().unwrap();
    let have = net.clients[j].bundles.len() as u32;
    net.clients[j].core.caught_up(have);
    net.run_until(5_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::PeerCaughtUp { .. }))
            .is_some()
    });
    assert!(!net.host.is_active(slot));
    assert!(net.host.add_human(slot, TEAM_AUTO));
    net.run_ms(3_000);
    let host = &net.host_bundles;
    let mine = &net.clients[j].bundles;
    assert!(mine.len() + 3 >= host.len());
    assert_eq!(mine[..], host[..mine.len()]);
    let added = host
        .iter()
        .position(|b| {
            b.commands
                .iter()
                .any(|c| matches!(c, SessionCommand::AddHuman { slot: s, .. } if *s == slot))
        })
        .expect("AddHuman issued");
    assert!(host[..added].iter().all(|b| b.pad(slot).is_none()));
    assert!(host[added..].iter().all(|b| b.pad(slot).is_some()));
    assert!(net.clients[j].player.submitted.len() > 60);
}

#[test]
fn session_timeout_removes_human_then_token_reclaims_slot() {
    let mut net = Net::new(4, lan(), Net::host_config());
    let a = net.join(1, "Stays", 1, Player::new(10.0, 1.0, 1));
    let b = net.join(2, "Drops", 42, Player::new(10.0, 1.0, 2));
    net.connect_all();
    net.start_round();
    net.run_ms(2_000);
    let slot = net.clients[b].core.slot().unwrap();
    let cut_at = net.now;
    net.sim.cut(addr(2), true);
    net.clients[b].alive = false;
    net.run_until(20_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::PlayerLeft { .. }))
            .is_some()
    });
    let (at, ev) = net
        .host_event(|e| matches!(e, NetEvent::PlayerLeft { .. }))
        .unwrap();
    assert_eq!(
        ev,
        NetEvent::PlayerLeft {
            slot,
            reason: LeaveReason::Timeout
        }
    );
    let (trouble_at, _) = net
        .host_event(|e| *e == NetEvent::Trouble { slot, on: true })
        .expect("trouble before the drop");
    let trouble_after = (trouble_at - cut_at).as_secs_f64();
    assert!(
        (1.4..=1.6).contains(&trouble_after),
        "trouble after {trouble_after} s"
    );
    let waited = (at - cut_at).as_secs_f64();
    assert!((11.9..=13.0).contains(&waited), "dropped after {waited} s");
    net.run_ms(100);
    let removed = net
        .host_bundles
        .iter()
        .position(|b| b.commands.contains(&SessionCommand::RemoveHuman { slot }))
        .expect("RemoveHuman issued");
    assert!(net.host_bundles[removed].pad(slot).is_none());
    assert!(net.host_bundles[removed - 1].pad(slot).is_some());
    // The other client was never dropped and still agrees.
    let c = &net.clients[a];
    assert!(c.bundles.len() + 3 >= net.host_bundles.len());
    assert_eq!(c.bundles[..], net.host_bundles[..c.bundles.len()]);

    // A newcomer does not get the reserved slot; the same token does.
    let n = net.join(3, "New", 7, Player::new(10.0, 1.0, 3));
    net.sim.cut(addr(2), false);
    let back = net.join(4, "Drops", 42, Player::new(10.0, 1.0, 4));
    net.run_until(5_000, |n2| {
        n2.clients[n].core.slot().is_some() && n2.clients[back].core.slot().is_some()
    });
    assert_eq!(net.clients[back].core.slot(), Some(slot));
    assert_ne!(net.clients[n].core.slot(), Some(slot));
    assert!(net
        .host_event(
            |e| matches!(e, NetEvent::PlayerJoined { slot: s, reconnect: true, .. } if *s == slot)
        )
        .is_some());
}

#[test]
fn session_hash_mismatch_reports_desync_and_resync_resends_round() {
    let mut net = Net::new(5, lan(), Net::host_config());
    let a = net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    net.start_round();
    net.run_until(10_000, |n| n.host.next_tick() >= 70);
    net.host.report_hash(30, 0xAAAA);
    net.clients[a].core.report_hash(30, 0xAAAA);
    net.clients[a].core.report_hash(60, 0xBBBB);
    net.run_ms(300);
    assert!(net
        .host_event(|e| matches!(e, NetEvent::Desync { .. }))
        .is_none());
    net.host.report_hash(60, 0xCCCC);
    net.run_ms(300);
    let slot = net.clients[a].core.slot().unwrap();
    assert!(net
        .host_event(|e| *e
            == NetEvent::Desync {
                slot,
                tick: 60,
                host_hash: 0xCCCC,
                peer_hash: 0xBBBB
            })
        .is_some());
    assert!(net.clients[a].events.iter().any(|e| matches!(
        e,
        ClientEvent::Desync {
            tick: 60,
            host_hash: 0xCCCC,
            ..
        }
    )));
    // The guest's rows reach the host game, range-checked on the way.
    net.clients[a]
        .core
        .desync_hashes(60, 31, 2, (0..61).collect());
    net.run_ms(300);
    let rows = net
        .host_event(|e| matches!(e, NetEvent::DesyncHashes { .. }))
        .map(|(_, e)| e);
    match rows {
        Some(NetEvent::DesyncHashes {
            slot: s,
            tick: 60,
            first_tick: 31,
            width: 2,
            values,
            ..
        }) => {
            assert_eq!(s, slot);
            assert_eq!(values, (0..60).collect::<Vec<u64>>(), "whole rows only");
        }
        other => panic!("{other:?}"),
    }
    net.clients[a].core.request_resync();
    net.run_ms(2_000);
    let loads = net.clients[a]
        .events
        .iter()
        .filter(|e| matches!(e, ClientEvent::LoadRound { joining: true, .. }))
        .count();
    assert_eq!(loads, 1);
    let mine = &net.clients[a].bundles;
    assert!(mine.len() + 3 >= net.host_bundles.len());
    assert_eq!(mine[..], net.host_bundles[..mine.len()]);
}

#[test]
fn session_leave_and_kick() {
    let mut net = Net::new(6, lan(), Net::host_config());
    let a = net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    let b = net.join(2, "B", 2, Player::new(10.0, 1.0, 2));
    net.connect_all();
    let (sa, sb) = (
        net.clients[a].core.slot().unwrap(),
        net.clients[b].core.slot().unwrap(),
    );
    net.clients[a].core.leave();
    net.host.kick(sb, true);
    net.run_ms(500);
    assert!(net
        .host_event(|e| *e
            == NetEvent::PlayerLeft {
                slot: sa,
                reason: LeaveReason::Left
            })
        .is_some());
    assert_eq!(rejected(&net.clients[b]), Some(RejectReason::Banned));
    // The banned token cannot come back.
    let again = net.join(3, "B", 2, Player::new(10.0, 1.0, 3));
    net.run_ms(500);
    assert_eq!(rejected(&net.clients[again]), Some(RejectReason::Banned));
}

#[test]
fn session_discovery_finds_host_with_rtt_and_info() {
    let mut net = Net::new(7, lan(), Net::host_config());
    net.host.set_info(b"Host on Hoth, 1/16".to_vec());
    let me = addr(8);
    net.sim.add_endpoint(me);
    let mut browser = LanBrowser::new(Vec::new());
    for _ in 0..1_500 {
        for (to, b) in browser.update(net.now) {
            net.sim.send(net.now, me, to, &b);
        }
        net.step();
        while let Some((from, b)) = net.sim.recv(me) {
            browser.handle_datagram(net.now, from, &b);
        }
    }
    let hosts = browser.hosts();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].addr, addr(0));
    assert_eq!(hosts[0].info, b"Host on Hoth, 1/16");
    let rtt = hosts[0].rtt.as_millis_f64();
    assert!((4.0..=20.0).contains(&rtt), "rtt {rtt} ms");
}

/// Slow machines (LAN §8b): a guest whose reported tick falls more than the
/// limit behind live is kicked with the fell-behind text; one that keeps
/// reporting stays, and a limit of 0 turns the drop off.
#[test]
fn session_drops_a_guest_that_falls_behind() {
    for limit in [90, 0] {
        let mut cfg = Net::host_config();
        cfg.fell_behind_ticks = limit;
        let mut net = Net::new(8, lan(), cfg);
        let keeps = net.join(1, "Keeps", 1, Player::new(10.0, 1.0, 1));
        let slow = net.join(2, "Slow", 2, Player::new(10.0, 1.0, 2));
        net.connect_all();
        let slow_slot = net.clients[slow].core.slot().unwrap();
        net.start_round();
        let mut reported = 0;
        while net.host.next_tick() < 400 {
            net.step();
            let t = net.host.next_tick().saturating_sub(2) / 30 * 30;
            if t > reported {
                reported = t;
                net.host.report_hash(t, 7);
                net.clients[keeps].core.report_hash(t, 7);
                // The slow guest stops at tick 60.
                if t <= 60 {
                    net.clients[slow].core.report_hash(t, 7);
                }
            }
        }
        net.run_ms(300);
        let dropped = net.host_event(|e| {
            *e == NetEvent::PlayerLeft {
                slot: slow_slot,
                reason: LeaveReason::FellBehind,
            }
        });
        let kept = net.host_event(|e| {
            matches!(e, NetEvent::PlayerLeft { slot, .. }
                if Some(*slot) == net.clients[keeps].core.slot())
        });
        assert!(kept.is_none(), "limit {limit}: the reporting guest stays");
        if limit == 0 {
            assert!(dropped.is_none(), "the drop is off");
            continue;
        }
        assert!(dropped.is_some(), "the slow guest was dropped");
        // Dropped once the clock passed 60 + limit + one report interval.
        let removed = net
            .host_bundles
            .iter()
            .position(|b| {
                b.commands
                    .contains(&SessionCommand::RemoveHuman { slot: slow_slot })
            })
            .expect("a bot took the slow guest's place");
        assert!(
            removed as u32 > 60 + limit + 30,
            "removed at tick {removed}"
        );
        assert!(
            (removed as u32) < 60 + limit + 30 + 10,
            "removed at tick {removed}"
        );
        let detail = net.clients[slow].events.iter().find_map(|e| match e {
            ClientEvent::Rejected {
                reason: RejectReason::Kicked,
                detail,
            } => Some(detail.clone()),
            _ => None,
        });
        assert_eq!(detail.as_deref(), Some(crate::host::FELL_BEHIND_TEXT));
    }
}

/// Benching (LAN §4.3, a copy that plays differently): the guest leaves the
/// round's bundles (a `RemoveHuman`) and gets no more of them, keeps its
/// connection, and the next round seats it again.
#[test]
fn session_bench_keeps_the_guest_out_of_the_round_until_the_next() {
    let mut net = Net::new(9, lan(), Net::host_config());
    let a = net.join(1, "Stays", 1, Player::new(10.0, 1.0, 1));
    let b = net.join(2, "Benched", 2, Player::new(10.0, 1.0, 2));
    net.connect_all();
    let sb = net.clients[b].core.slot().unwrap();
    net.start_round();
    net.run_until(10_000, |n| n.host.next_tick() >= 60);
    assert!(net.host.bench(sb));
    assert!(!net.host.bench(0), "the host is never benched");
    net.run_until(10_000, |n| n.host.next_tick() >= 200);
    let removed = net
        .host_bundles
        .iter()
        .position(|bd| {
            bd.commands
                .iter()
                .any(|c| matches!(c, SessionCommand::RemoveHuman { slot } if *slot == sb))
        })
        .expect("RemoveHuman issued");
    assert!(net.host_bundles[removed..]
        .iter()
        .all(|bd| bd.pad(sb).is_none()));
    let got = net.clients[b].bundles.len();
    assert!(
        got <= removed + 40,
        "bundles kept flowing: {got} > {removed}"
    );
    assert!(net.clients[a].bundles.len() + 3 >= net.host_bundles.len());
    assert_eq!(net.clients[b].core.slot(), Some(sb));
    // The next round seats it from tick 0.
    net.host.end_round();
    net.run_ms(200);
    net.host_events.clear();
    net.start_round();
    net.run_until(10_000, |n| n.host.next_tick() >= 30);
    assert!(net.host.is_active(sb));
    assert!(net.host_bundles.last().unwrap().pad(sb).is_some());
}

#[test]
fn session_names_are_unique_in_join_order() {
    let mut net = Net::new(11, lan(), Net::host_config());
    let a = net.join(1, "host", 1, Player::new(10.0, 1.0, 1));
    net.run_until(5_000, |n| n.clients[a].core.slot().is_some());
    let b = net.join(2, "HOST", 2, Player::new(10.0, 1.0, 2));
    net.run_until(5_000, |n| n.clients[b].core.slot().is_some());
    let c = net.join(3, "the maintainer", 3, Player::new(10.0, 1.0, 3));
    net.run_until(5_000, |n| n.clients[c].core.slot().is_some());
    let names = |n: &Net| -> Vec<String> { n.host.players().into_iter().map(|p| p.1).collect() };
    assert_eq!(names(&net), ["Host", "host_1", "HOST_2", "the maintainer"]);
    // The same token again keeps its session name, whatever it sends.
    let back = net.join(4, "Host", 2, Player::new(10.0, 1.0, 4));
    net.run_until(5_000, |n| n.clients[back].core.slot().is_some());
    assert_eq!(names(&net), ["Host", "host_1", "HOST_2", "the maintainer"]);
    let joined: Vec<String> = net
        .host_events
        .iter()
        .filter_map(|(_, ev)| match ev {
            NetEvent::PlayerJoined { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(joined, ["host_1", "HOST_2", "the maintainer", "HOST_2"]);
}

/// Round 2 shorter than round 1 still gets a bundle datagram to every guest
/// on every tick (the per-peer "already sent" mark starts over per stream).
#[test]
fn session_second_round_streams_a_bundle_on_every_tick() {
    let mut net = Net::new(
        12,
        LinkParams::ideal(Micros::from_millis(5)),
        Net::host_config(),
    );
    net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    net.join(2, "B", 2, Player::new(16.7, 1.0, 2));
    net.connect_all();
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 600);
    net.host.end_round();
    net.run_ms(500);
    net.host_events.clear();
    net.record_sends = true;
    net.start_round();
    net.run_until(20_000, |n| n.host.next_tick() >= 150);
    let guests: Vec<_> = net.clients.iter().map(|c| c.addr).collect();
    for to in guests {
        let missing: Vec<u32> = (1..=150)
            .filter(|t| !net.bundle_sends.contains(&(*t, to)))
            .collect();
        assert!(
            missing.is_empty(),
            "{to}: no bundle sent after ticks {missing:?}"
        );
    }
}

/// A mid-round join's log transfer over a 10 Mbit/s host uplink: the
/// reliable flight is bounded, so the other guest's live bundles keep
/// arriving within a few milliseconds of the link delay.
#[test]
fn session_mid_round_join_keeps_other_guests_bundles_flowing() {
    let link = LinkParams {
        jitter: Micros::from_millis(1),
        ..LinkParams::ideal(Micros::from_millis(5))
    };
    let mut net = Net::new(13, link, Net::host_config());
    net.sim.set_uplink(addr(0), 1_250_000);
    let a = net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    net.start_round();
    // About 360 KB of incompressible log: one 600-byte command a tick.
    let mut r = crate::linksim::Lcg(3);
    for _ in 0..600 {
        let cmd = (0..600).map(|_| r.next_u64() as u8).collect();
        assert!(net.host.issue_command(cmd));
    }
    net.run_until(60_000, |n| n.host.next_tick() >= 700);
    let join_at = net.host.next_tick() as usize;
    let j = net.join(2, "Late", 2, Player::new(10.0, 1.0, 2));
    net.run_until(30_000, |n| n.clients[j].bundles.len() >= join_at);
    let end = net.host.next_tick() as usize;
    let c = &net.clients[a];
    let worst = (join_at..end)
        .map(|t| (c.bundle_at[t] - net.host_bundle_at[t]).as_millis_f64())
        .fold(0.0, f64::max);
    assert!(
        worst < 50.0,
        "a live bundle waited {worst} ms behind the log"
    );
}

/// Discovery answers one `Find` per datagram, at most every 250 ms per
/// address (no reflection amplifier).
#[test]
fn session_discovery_answers_once_per_datagram_and_rate_limits() {
    let mut host = HostCore::new(Net::host_config());
    host.set_info(vec![1; 100]);
    let from = addr(9);
    let flood = || {
        let mut pw = crate::packet::bare_packet(0);
        for i in 0..50 {
            pw.find(i);
        }
        pw.finish()
    };
    let replies = |host: &mut HostCore, at: Micros| {
        host.handle_datagram(at, from, &flood());
        host.update(at).iter().filter(|(to, _)| *to == from).count()
    };
    assert_eq!(replies(&mut host, Micros::from_millis(1)), 1);
    assert_eq!(replies(&mut host, Micros::from_millis(100)), 0);
    assert_eq!(replies(&mut host, Micros::from_millis(300)), 1);
}

/// Sixteen humans with the widest pads and a queue of large commands: every
/// bundle stays within one datagram against either base, reaches every
/// guest, and the commands spread over the ticks.
#[test]
fn session_bundles_fit_a_datagram_with_full_pads_and_big_commands() {
    use crate::packet::BUNDLE_BUDGET;
    let mut net = Net::new(
        15,
        LinkParams::ideal(Micros::from_millis(2)),
        Net::host_config(),
    );
    net.host_player.wild = true;
    for i in 1..16u16 {
        let mut p = Player::new(10.0, 1.0, u64::from(i));
        p.wild = true;
        net.join(i, &format!("P{i}"), u64::from(i), p);
    }
    net.connect_all();
    net.start_round();
    for k in 0..90u8 {
        assert!(net.host.issue_command(vec![k; 300]));
    }
    net.run_until(30_000, |n| n.host.next_tick() >= 300);
    net.run_ms(500);
    assert_eq!(net.host.stats().oversize_bundles, 0);
    let host = &net.host_bundles;
    for (i, b) in host.iter().enumerate() {
        let prev = if i > 0 {
            host[i - 1].pads.as_slice()
        } else {
            &[]
        };
        for base in [&[][..], prev] {
            let mut w = crate::wire::Writer::new();
            b.write(&mut w, base);
            assert!(w.len() <= BUNDLE_BUDGET, "tick {i}: {} bytes", w.len());
        }
    }
    assert!(host[100].pads.len() == 16);
    let delivered: usize = host.iter().map(|b| b.commands.len()).sum();
    assert_eq!(delivered, 90);
    for c in &net.clients {
        assert!(
            c.bundles.len() + 6 >= host.len(),
            "{} of {}",
            c.bundles.len(),
            host.len()
        );
        assert_eq!(c.bundles[..], host[..c.bundles.len()]);
    }
}

/// The host game's seat list decides the round's humans: a guest it missed
/// (joined while the round was set up) joins once the round runs, a seat
/// whose guest is gone goes to a bot in tick 0, and a missed guest benched
/// during the load stays out.
#[test]
fn session_seat_list_decides_the_rounds_humans() {
    let mut net = Net::new(14, lan(), Net::host_config());
    let a = net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    let b = net.join(2, "B", 2, Player::new(10.0, 1.0, 2));
    let c = net.join(3, "C", 3, Player::new(10.0, 1.0, 3));
    net.connect_all();
    let slot = |n: &Net, i: usize| n.clients[i].core.slot().unwrap();
    let (sa, sb, sc) = (slot(&net, a), slot(&net, b), slot(&net, c));
    net.host.load_round(b"setup".to_vec(), &[0, sa, 9]);
    assert!(net.host.is_active(sa));
    assert!(!net.host.is_active(sb) && !net.host.is_active(sc));
    assert!(net.host.bench(sc));
    net.run_until(5_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::RoundStarted { .. }))
            .is_some()
    });
    net.run_until(5_000, |n| n.host.next_tick() >= 30);
    let first = &net.host_bundles[0];
    assert!(first
        .commands
        .contains(&SessionCommand::RemoveHuman { slot: 9 }));
    let seated: Vec<u8> = first.pads.iter().map(|p| p.0).collect();
    assert_eq!(seated, [0, sa]);
    let loads = |n: &Net, i: usize| -> Vec<bool> {
        n.clients[i]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::LoadRound { joining, .. } => Some(*joining),
                _ => None,
            })
            .collect()
    };
    assert_eq!(loads(&net, a), [false]);
    net.run_until(5_000, |n| loads(n, b) == [true]);
    net.run_ms(1_000);
    assert!(loads(&net, c).is_empty(), "the benched guest stays out");
    let have = net.clients[b].bundles.len() as u32;
    assert!(have + 3 >= net.host.next_tick());
    net.clients[b].core.caught_up(have);
    net.run_until(5_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::PeerCaughtUp { slot, .. } if *slot == sb))
            .is_some()
    });
    assert!(net.host.add_human(sb, TEAM_AUTO));
    net.run_ms(500);
    assert!(net.host_bundles.last().unwrap().pad(sb).is_some());
    let mine = &net.clients[b].bundles;
    assert_eq!(mine[..], net.host_bundles[..mine.len()]);
}
