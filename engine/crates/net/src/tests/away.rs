//! A local player's lost pad in a LAN round (PROTOCOL 12, splitscreen
//! packet B17): `LocalAway` takes only that local's slot out of the round,
//! `LocalBack` seats it again in step with its machine's other locals, and
//! every peer keeps stepping the host's exact bundles.

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

fn duo_config(token: u64) -> ClientConfig {
    let mut c = ClientConfig::new(BUILD, "Left", token);
    c.locals = vec![LocalHello {
        seat: 1,
        name: "Right".to_owned(),
        profile: Vec::new(),
    }];
    c
}

/// Every session command of the round, with its tick.
fn commands(host: &[TickBundle]) -> Vec<(u32, SessionCommand)> {
    host.iter()
        .flat_map(|b| b.commands.iter().map(move |c| (b.tick, c.clone())))
        .collect()
}

fn mouse_x(v: &[core::sim::PadState]) -> i64 {
    v.iter().map(|p| i64::from(p.mx)).sum()
}

/// Every client stepped a prefix of the host's own log (the game's hashes
/// follow from the bundles), and a long one.
fn same_bundles(net: &Net, at_least: usize) {
    for c in &net.clients {
        assert_eq!(c.bundles[..], net.host_bundles[..c.bundles.len()]);
        assert!(c.bundles.len() >= at_least, "{} bundles", c.bundles.len());
    }
}

fn stop_and_drain(net: &mut Net) {
    net.host_player.playing = false;
    for c in &mut net.clients {
        c.player.playing = false;
    }
    net.run_ms(3_000);
}

/// The plan's check: a guest's second local loses its pad and gets it
/// back. Only its slot leaves and returns (one `RemoveHuman`, one
/// `AddHuman`, nothing else), the other slots play every tick with every
/// input they sent, the returning slot consumes the same sequence as its
/// sibling on every tick, and both guests step the host's exact bundles.
#[test]
fn away_guest_local_goes_and_returns() {
    let mut net = Net::new(41, lossy(), Net::host_config());
    let solo = net.join(1, "Solo", 101, Player::new(16.7, 1.0004, 6));
    let duo = net.join_with(2, duo_config(102), Player::new(7.0, 0.9996, 7));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[solo].core.slots(), vec![1]);
    assert_eq!(net.clients[duo].core.slots(), vec![2, 3]);
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 300);

    net.clients[duo].core.set_local_away(1, true);
    assert!(net.clients[duo].core.local_away(1));
    net.run_until(5_000, |n| !n.host.is_active(3));
    assert!(net.host.is_away(3) && !net.host.is_away(2));
    assert!(net
        .host_event(|e| matches!(e, NetEvent::LocalAway { slot: 3 }))
        .is_some());
    // Never seated while away.
    assert!(!net.host.add_human(3, TEAM_AUTO));
    let gone = net.host.next_tick() + 300;
    net.run_until(60_000, |n| n.host.next_tick() >= gone);

    net.clients[duo].core.set_local_away(1, false);
    net.run_until(5_000, |n| n.host.is_active(3));
    assert!(net
        .host_event(|e| matches!(e, NetEvent::LocalBack { slot: 3, .. }))
        .is_some());
    let end = net.host.next_tick() + 400;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    stop_and_drain(&mut net);

    let host = &net.host_bundles;
    let cmds = commands(host);
    assert_eq!(cmds.len(), 2, "{cmds:?}");
    let (out_at, back_at) = match (&cmds[0], &cmds[1]) {
        (
            (a, SessionCommand::RemoveHuman { slot: 3 }),
            (b, SessionCommand::AddHuman { slot: 3, .. }),
        ) => (*a, *b),
        _ => panic!("{cmds:?}"),
    };
    assert!(back_at > out_at + 250, "{out_at} {back_at}");
    for b in host {
        for s in [0, 1, 2] {
            assert!(b.pad(s).is_some(), "slot {s} missing at tick {}", b.tick);
        }
        let away = (out_at..back_at).contains(&b.tick);
        assert_eq!(b.pad(3).is_some(), !away, "slot 3 at tick {}", b.tick);
    }
    let mut both = 0;
    for t in host {
        if let (Some(ca), Some(cb)) = (t.consumed(2), t.consumed(3)) {
            assert_eq!(ca, cb, "tick {} consumed {ca} vs {cb}", t.tick);
            both += 1;
        }
    }
    assert!(both > 600, "{both} ticks with both locals seated");
    // The other players lost nothing: every mouse step they sent landed.
    let d = &net.clients[duo];
    assert_eq!(mouse_x(&slot_pads(host, 2)), mouse_x(&d.player.submitted));
    let s = &net.clients[solo];
    assert_eq!(mouse_x(&slot_pads(host, 1)), mouse_x(&s.player.submitted));
    same_bundles(&net, 900);
    let st = net.host.stats();
    assert_eq!(
        (st.drops.malformed, st.drops.bad_chunk, st.drops.bad_message),
        (0, 0, 0)
    );
}

/// Steps `net` until the host closed `tick`, watching the starvation count
/// the machine `c` hears: it must only ever grow, and never by a burst (a
/// change of pacing slot must not read as starvations).
fn run_watching(net: &mut Net, c: usize, tick: u32, last: &mut u32) {
    net.run_until(120_000, |n| {
        let now = n.clients[c].core.pacing().1;
        assert!(
            now >= *last && now - *last <= 3,
            "client starvation count {} -> {now}",
            *last
        );
        *last = now;
        n.host.next_tick() >= tick
    });
}

/// With a guest's local 0 away, the machine's status paces from local 1
/// (the first seated slot, B15). Over 1000 ticks local 1's queue stays as
/// deep as it was with both seated (pacing from local 0's idle queue would
/// let it grow to about 3) and starves at about a one-local machine's rate
/// (5-9 a 1000 ticks seated in a group, 9-13 alone, 7-10 for a one-local
/// machine on this link, seeds 43-50); the count the client hears runs on
/// through both changes of pacing slot; and local 0 comes back in step.
#[test]
fn away_local_zero_paces_from_its_sibling() {
    let mut net = Net::new(43, lossy(), Net::host_config());
    let duo = net.join_with(1, duo_config(201), Player::new(7.0, 0.9996, 9));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    let (a, b) = (1u8, 2u8);
    assert_eq!(net.clients[duo].core.slots(), vec![a, b]);
    net.start_round();
    let starved = |n: &Net| n.host.pacing(b).map_or(0, |p| p.1);
    let mut heard = 0;
    run_watching(&mut net, duo, 400, &mut heard);
    let (s0, d0) = (starved(&net), net.depths[usize::from(b)].len());
    run_watching(&mut net, duo, 1_400, &mut heard);
    let (s1, d1) = (starved(&net), net.depths[usize::from(b)].len());

    net.clients[duo].core.set_local_away(0, true);
    let out_at = net.host.next_tick();
    run_watching(&mut net, duo, out_at + 1_000, &mut heard);
    assert!(!net.host.is_active(a));
    let (s3, d3) = (starved(&net), net.depths[usize::from(b)].len());
    let s2 = s1;
    let depth = &net.depths[usize::from(b)];
    let mean = |r: std::ops::Range<usize>| {
        depth[r.clone()].iter().map(|d| f64::from(*d)).sum::<f64>() / r.len() as f64
    };
    let (seated, alone) = (mean(d0..d1), mean(d1 + 30..d3));
    println!(
        "slot {b}: both seated +{} starvations, depth {seated:.2}; \
         local 0 away +{} starvations, depth {alone:.2}",
        s1 - s0,
        s3 - s2
    );
    assert!(alone <= seated + 0.5, "depth {seated:.2} -> {alone:.2}");
    assert!(s3 - s2 <= 2 * (s1 - s0) + 4, "{s0} {s1} {s3}");
    let window = net
        .host_bundles
        .iter()
        .filter(|t| t.tick > out_at + 30 && t.tick < out_at + 1_000);
    assert!(window.clone().all(|t| t.pad(a).is_none()));
    let mut prev = 0;
    let mut stalls = 0;
    for t in window {
        let c = t.consumed(b).expect("local 1 seated");
        stalls += usize::from(c == prev);
        prev = c;
    }
    assert!(
        stalls <= 5 + (s3 - s2) as usize,
        "{stalls} ticks without input"
    );

    net.clients[duo].core.set_local_away(0, false);
    let back = net.host.next_tick();
    run_watching(&mut net, duo, back + 400, &mut heard);
    assert!(net.host.is_active(a));
    stop_and_drain(&mut net);
    for t in &net.host_bundles {
        if let (Some(ca), Some(cb)) = (t.consumed(a), t.consumed(b)) {
            assert_eq!(ca, cb, "tick {} consumed {ca} vs {cb}", t.tick);
        }
    }
    same_bundles(&net, 2_000);
}

/// The host's own slot 0 loses its pad: a `RemoveHuman` for slot 0, the
/// host's other local paces the host (`host_pace_slot`, the meter's
/// figures) and plays on, and slot 0 comes back in step with it. A slot
/// that is no host local is refused.
#[test]
fn away_host_slot_zero_leaves_and_returns() {
    let mut net = Net::new(47, lossy(), Net::host_config());
    assert_eq!(
        net.host.add_local("Host", Vec::new()),
        Ok((1, "Host_1".to_owned()))
    );
    net.host_extra = vec![1];
    let guest = net.join(1, "Solo", 301, Player::new(10.0, 1.0, 3));
    net.connect_all();
    assert_eq!(net.clients[guest].core.slots(), vec![2]);
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 300);
    assert_eq!(net.host.host_pace_slot(), 0);

    assert!(!net.host.set_local_away(2, true), "a guest's slot");
    assert!(net.host.set_local_away(0, true));
    net.run_until(5_000, |n| !n.host.is_active(0));
    assert_eq!(net.host.host_pace_slot(), 1);
    let gone = net.host.next_tick() + 200;
    net.run_until(60_000, |n| n.host.next_tick() >= gone);
    // The meter (and the host's own pacer) reads slot 1's queue now.
    let st = net.host.stats();
    assert_eq!(st.local_depth, net.host.pacing(1).unwrap().0);
    assert_eq!(net.host.host_pacing(), (st.local_depth, st.local_starved));

    assert!(net.host.set_local_away(0, false));
    net.run_until(5_000, |n| n.host.is_active(0));
    assert_eq!(net.host.host_pace_slot(), 0);
    let end = net.host.next_tick() + 300;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    stop_and_drain(&mut net);

    let host = &net.host_bundles;
    let cmds = commands(host);
    assert!(
        matches!(
            cmds.as_slice(),
            [
                (_, SessionCommand::RemoveHuman { slot: 0 }),
                (_, SessionCommand::AddHuman { slot: 0, .. })
            ]
        ),
        "{cmds:?}"
    );
    assert!(host
        .iter()
        .all(|b| b.pad(1).is_some() && b.pad(2).is_some()));
    assert!(host.iter().any(|b| b.pad(0).is_none()));
    let mut both = 0;
    for t in host {
        if let (Some(c0), Some(c1)) = (t.consumed(0), t.consumed(1)) {
            assert_eq!(c0, c1, "tick {} consumed {c0} vs {c1}", t.tick);
            both += 1;
        }
    }
    assert!(both > 400, "{both}");
    assert_eq!(
        mouse_x(&slot_pads(host, 1)),
        mouse_x(&net.host_extra_submitted[0])
    );
    same_bundles(&net, 700);
}

/// The edges: a pad lost before the round loads keeps its slot out from
/// tick 0 and is released (`LocalBack`, a return) when it is back; a pad back
/// before its removal closed keeps its seat with no command at all; a
/// local index the machine does not have is a bad message.
#[test]
fn away_before_the_round_and_back_in_time() {
    let mut net = Net::new(53, lossy(), Net::host_config());
    let duo = net.join_with(1, duo_config(401), Player::new(7.0, 0.9996, 5));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![1, 2]);
    let solo = net.join(2, "Solo", 402, Player::new(10.0, 1.0, 4));
    net.connect_all();
    assert_eq!(net.clients[solo].core.slots(), vec![3]);

    // Lost in the lobby: the seat list names slot 2 but it stays out.
    net.clients[duo].core.set_local_away(1, true);
    net.run_until(5_000, |n| n.host.is_away(2));
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 200);
    assert!(!net.host.is_active(2));
    assert!(!net.host.add_human(2, TEAM_AUTO));
    assert!(net
        .host_event(|e| matches!(
            e,
            NetEvent::PeerCaughtUp { slot: 2, .. } | NetEvent::LocalBack { slot: 2, .. }
        ))
        .is_none());
    net.clients[duo].core.set_local_away(1, false);
    // A return, not a late joiner (the harness seats it as the game does).
    net.run_until(5_000, |n| n.host.is_active(2));
    assert!(net
        .host_event(|e| matches!(e, NetEvent::LocalBack { slot: 2, .. }))
        .is_some());
    assert!(net
        .host_event(|e| matches!(e, NetEvent::PeerCaughtUp { slot: 2, .. }))
        .is_none());
    let backs_before = net
        .host_events
        .iter()
        .filter(|(_, e)| matches!(e, NetEvent::LocalBack { .. }))
        .count();

    // Gone and back within one frame: the removal is cancelled.
    let before = commands(&net.host_bundles).len();
    net.clients[duo].core.set_local_away(1, true);
    net.clients[duo].core.set_local_away(1, false);
    let end = net.host.next_tick() + 300;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    assert!(net.host.is_active(2) && !net.host.is_away(2));
    assert_eq!(commands(&net.host_bundles).len(), before);
    let backs = net
        .host_events
        .iter()
        .filter(|(_, e)| matches!(e, NetEvent::LocalBack { .. }))
        .count();
    assert_eq!(backs, backs_before);

    // A one-local machine has no local 2.
    let bad = net.host.stats().drops.bad_message;
    net.clients[solo].core.set_local_away(2, true);
    net.run_until(5_000, |n| n.host.stats().drops.bad_message > bad);
    assert!(!net.host.is_away(3) && !net.host.is_away(2));

    stop_and_drain(&mut net);
    let host = &net.host_bundles;
    let cmds = commands(host);
    assert!(
        matches!(
            cmds.as_slice(),
            [
                (0, SessionCommand::RemoveHuman { slot: 2 }),
                (_, SessionCommand::AddHuman { slot: 2, .. })
            ]
        ),
        "{cmds:?}"
    );
    for t in host {
        if let (Some(ca), Some(cb)) = (t.consumed(1), t.consumed(2)) {
            assert_eq!(ca, cb, "tick {} consumed {ca} vs {cb}", t.tick);
        }
    }
    same_bundles(&net, 400);
}

/// A pad lost in the lobby and back while the round still loads: the
/// load's tick-0 `RemoveHuman` stays (the slot was never seated), and the
/// slot is seated once the round runs through one `LocalBack` and one
/// `AddHuman`, with every guest stepping the host's bundles.
#[test]
fn away_in_the_lobby_back_while_loading() {
    let mut net = Net::new(59, lossy(), Net::host_config());
    let duo = net.join_with(1, duo_config(501), Player::new(7.0, 0.9996, 8));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![1, 2]);
    net.clients[duo].core.set_local_away(1, true);
    net.run_until(5_000, |n| n.host.is_away(2));

    let humans: Vec<u8> = net.host.players().iter().map(|p| p.0).collect();
    net.host.load_round(b"setup".to_vec(), &humans);
    // Back on the loading screen, before tick 0 closes.
    net.clients[duo].core.set_local_away(1, false);
    net.run_until(5_000, |n| !n.host.is_away(2));
    assert!(
        matches!(
            net.host.round_phase(),
            crate::host::RoundPhase::Loading { .. }
        ),
        "{:?}",
        net.host.round_phase()
    );
    net.run_until(10_000, |n| n.host.is_active(2));
    let end = net.host.next_tick() + 300;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    stop_and_drain(&mut net);

    let cmds = commands(&net.host_bundles);
    assert!(
        matches!(
            cmds.as_slice(),
            [
                (0, SessionCommand::RemoveHuman { slot: 2 }),
                (_, SessionCommand::AddHuman { slot: 2, .. })
            ]
        ),
        "{cmds:?}"
    );
    let ev = |f: fn(&NetEvent) -> bool| net.host_events.iter().filter(|(_, e)| f(e)).count();
    assert_eq!(ev(|e| matches!(e, NetEvent::LocalBack { slot: 2, .. })), 1);
    assert_eq!(
        ev(|e| matches!(e, NetEvent::PeerCaughtUp { slot: 2, .. })),
        0
    );
    same_bundles(&net, 300);
}

/// A benched machine keeps its link but plays no more of the round: its
/// local's pad going and coming back gives no `LocalBack`, and the host
/// refuses to seat that local (no `AddHuman`).
#[test]
fn away_back_on_a_benched_machine_is_not_seated() {
    let mut net = Net::new(61, lossy(), Net::host_config());
    let duo = net.join_with(1, duo_config(601), Player::new(7.0, 0.9996, 2));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![1, 2]);
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 100);
    net.clients[duo].core.set_local_away(1, true);
    net.run_until(5_000, |n| n.host.is_away(2));
    assert!(net.host.bench(1));
    net.clients[duo].core.set_local_away(1, false);
    net.run_until(5_000, |n| !n.host.is_away(2));
    let end = net.host.next_tick() + 200;
    net.run_until(60_000, |n| n.host.next_tick() >= end);
    assert!(net
        .host_event(|e| matches!(e, NetEvent::LocalBack { .. }))
        .is_none());
    assert!(!net.host.add_human(2, TEAM_AUTO));
    assert!(!net.host.add_human(1, TEAM_AUTO));
    assert!(!net.host.is_active(1) && !net.host.is_active(2));
    assert!(!commands(&net.host_bundles)
        .iter()
        .any(|(_, c)| matches!(c, SessionCommand::AddHuman { .. })));
}

/// Finding lan-addhuman-stale-session-profile: the profile a slot's later
/// `AddHuman` carries is the one the game last set for it
/// (`HostCore::set_profile`, at each round's load), not the one its
/// `Hello` brought: a guest's second local loses its pad and comes back
/// on its newer profile. An unknown slot is refused.
#[test]
fn away_return_carries_the_profile_set_at_load() {
    let mut net = Net::new(43, lossy(), Net::host_config());
    let duo = net.join_with(2, duo_config(102), Player::new(7.0, 0.9996, 7));
    net.clients[duo].extra_locals = 1;
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![1, 2]);
    assert!(net.host.set_profile(2, b"round 2".to_vec()));
    assert!(!net.host.set_profile(9, b"nobody".to_vec()));
    net.start_round();
    net.run_until(60_000, |n| n.host.next_tick() >= 200);
    net.clients[duo].core.set_local_away(1, true);
    net.run_until(5_000, |n| !n.host.is_active(2));
    net.clients[duo].core.set_local_away(1, false);
    net.run_until(5_000, |n| n.host.is_active(2));
    stop_and_drain(&mut net);
    let back: Vec<Vec<u8>> = commands(&net.host_bundles)
        .into_iter()
        .filter_map(|(_, c)| match c {
            SessionCommand::AddHuman {
                slot: 2, profile, ..
            } => Some(profile),
            _ => None,
        })
        .collect();
    assert_eq!(back, vec![b"round 2".to_vec()]);
}
