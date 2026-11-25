//! Splitscreen admission (PROTOCOL 12): a machine with several local
//! players takes one slot each, all or none, reclaims them together, and
//! leaves together.

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

/// A machine whose first player is `name` and whose extra locals are
/// `extra`.
fn machine(name: &str, token: u64, extra: &[&str]) -> ClientConfig {
    let mut c = ClientConfig::new(BUILD, name, token);
    c.profile = vec![1];
    c.locals = extra
        .iter()
        .enumerate()
        .map(|(i, n)| LocalHello {
            seat: 1 + i as u8,
            name: (*n).to_owned(),
            profile: vec![2 + i as u8],
        })
        .collect();
    c
}

fn joined(net: &Net) -> Vec<(u8, String, Vec<u8>, bool)> {
    net.host_events
        .iter()
        .filter_map(|(_, e)| match e {
            NetEvent::PlayerJoined {
                slot,
                name,
                profile,
                reconnect,
                ..
            } => Some((*slot, name.clone(), profile.clone(), *reconnect)),
            _ => None,
        })
        .collect()
}

fn left(net: &Net) -> Vec<(u8, LeaveReason)> {
    net.host_events
        .iter()
        .filter_map(|(_, e)| match e {
            NetEvent::PlayerLeft { slot, reason } => Some((*slot, *reason)),
            _ => None,
        })
        .collect()
}

fn rejected(c: &Client) -> Option<RejectReason> {
    c.events.iter().find_map(|e| match e {
        ClientEvent::Rejected { reason, .. } => Some(*reason),
        _ => None,
    })
}

#[test]
fn locals_two_local_join_gets_two_slots() {
    let mut net = Net::new(21, lan(), Net::host_config());
    let solo = net.join(1, "Solo", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    let duo = net.join_with(2, machine("Left", 2, &["Solo"]), Player::new(10.0, 1.0, 2));
    net.connect_all();
    let c = &net.clients[duo].core;
    assert_eq!(c.slots(), vec![2, 3]);
    assert_eq!(
        (c.local_slot(0), c.local_slot(1), c.local_slot(2)),
        (Some(2), Some(3), None)
    );
    assert!(net.clients[duo].events.iter().any(|e| matches!(
        e,
        ClientEvent::Welcome { slot: 2, extra, .. } if *extra == [3]
    )));
    // One PlayerJoined per slot; names stay unique across machines.
    let j = joined(&net);
    assert_eq!(
        j,
        vec![
            (1, "Solo".into(), vec![], false),
            (2, "Left".into(), vec![1], false),
            (3, "Solo_1".into(), vec![2], false),
        ]
    );
    let players = net.host.players();
    assert_eq!(
        players.iter().map(|p| p.0).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(net.host.machine_slots(3), vec![2, 3]);
    assert_eq!(net.host.machine_slots(1), vec![1]);
    // The one-player guest still holds one slot and an empty extra list.
    assert_eq!(net.clients[solo].core.slots(), vec![1]);
    assert!(net.clients[solo].events.iter().any(|e| matches!(
        e,
        ClientEvent::Welcome { slot: 1, extra, .. } if extra.is_empty()
    )));

    // Ready per local: local 1's Ready is heard for slot 3.
    net.clients[duo].core.ready_local(1, 0, vec![9]);
    net.clients[duo].core.ready(0, vec![8]);
    net.clients[duo].core.ready_local(3, 0, vec![7]);
    net.run_ms(300);
    let ready: Vec<(u8, Vec<u8>)> = net
        .host_events
        .iter()
        .filter_map(|(_, e)| match e {
            NetEvent::PeerReady { slot, profile, .. } => Some((*slot, profile.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(ready, vec![(3, vec![9]), (2, vec![8])]);

    // Both slots play in the round: the machine's one `Loaded` and hash
    // report speak for both, so the round starts and agrees.
    net.start_round();
    net.run_until(10_000, |n| n.host.next_tick() >= 40);
    assert!(net.host.is_active(2) && net.host.is_active(3));
    assert!(net.host_bundles.last().unwrap().pad(3).is_some());
    net.host.report_hash(30, 0xAB);
    net.clients[solo].core.report_hash(30, 0xAB);
    net.clients[duo].core.report_hash(30, 0xAB);
    net.run_ms(300);
    assert!(net
        .host_event(|e| matches!(e, NetEvent::HashesAgreed { tick: 30, .. }))
        .is_some());
}

#[test]
fn locals_full_when_not_all_fit() {
    let mut cfg = Net::host_config();
    cfg.max_players = 3;
    let mut net = Net::new(22, lan(), cfg);
    net.join(1, "A", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    // One slot free: a two-player machine is refused whole.
    let duo = net.join_with(2, machine("B", 2, &["C"]), Player::new(10.0, 1.0, 2));
    net.run_ms(1_000);
    assert_eq!(rejected(&net.clients[duo]), Some(RejectReason::Full));
    // The refusal says how many would fit (finding lan-full-reject-with-locals).
    let detail = net.clients[duo].events.iter().find_map(|e| match e {
        ClientEvent::Rejected { detail, .. } => Some(detail.clone()),
        _ => None,
    });
    assert_eq!(detail.as_deref(), Some("room 1 of 2"));
    assert_eq!(crate::host::parse_full_detail("room 1 of 2"), Some((1, 2)));
    assert_eq!(crate::host::parse_full_detail(""), None);
    assert_eq!(
        joined(&net).len(),
        1,
        "no slot taken by the refused machine"
    );
    assert_eq!(net.host.players().len(), 2);
    // The free slot is still there for one player.
    let solo = net.join(3, "D", 3, Player::new(10.0, 1.0, 3));
    net.run_until(5_000, |n| n.clients[solo].core.slot().is_some());
    assert_eq!(net.clients[solo].core.slot(), Some(2));
    // A bad name on any local refuses the machine.
    let bad = net.join_with(4, machine("E", 4, &[""]), Player::new(10.0, 1.0, 4));
    net.run_ms(1_000);
    assert_eq!(rejected(&net.clients[bad]), Some(RejectReason::BadName));
}

#[test]
fn locals_rejoin_reclaims_the_same_slots_and_a_drop_leaves_both() {
    let mut net = Net::new(23, lan(), Net::host_config());
    let stay = net.join(1, "Stays", 1, Player::new(10.0, 1.0, 1));
    let duo = net.join_with(2, machine("P1", 42, &["P2"]), Player::new(10.0, 1.0, 2));
    net.connect_all();
    let slots = net.clients[duo].core.slots();
    assert_eq!(slots.len(), 2);
    net.start_round();
    net.run_ms(1_000);
    // The machine's link dies: both its slots leave, both humans go.
    net.sim.cut(addr(2), true);
    net.clients[duo].alive = false;
    net.run_until(20_000, |n| left(n).len() >= 2);
    assert_eq!(
        left(&net),
        slots
            .iter()
            .map(|s| (*s, LeaveReason::Timeout))
            .collect::<Vec<_>>()
    );
    net.run_ms(100);
    for s in &slots {
        assert!(net.host_bundles.iter().any(|b| b
            .commands
            .contains(&SessionCommand::RemoveHuman { slot: *s })));
        assert!(!net.host.is_active(*s));
    }
    assert!(net.host.players().iter().all(|p| !slots.contains(&p.0)));

    // A newcomer does not take the reserved slots; the same token gets
    // both back, as reconnects, with their names.
    let new = net.join(3, "New", 7, Player::new(10.0, 1.0, 3));
    net.sim.cut(addr(2), false);
    let back = net.join_with(4, machine("P1", 42, &["P2"]), Player::new(10.0, 1.0, 4));
    net.run_until(5_000, |n| {
        n.clients[new].core.slot().is_some() && n.clients[back].core.slot().is_some()
    });
    assert_eq!(net.clients[back].core.slots(), slots);
    assert!(!slots.contains(&net.clients[new].core.slot().unwrap()));
    let again: Vec<_> = joined(&net)
        .into_iter()
        .filter(|j| slots.contains(&j.0) && j.3)
        .map(|j| (j.0, j.1))
        .collect();
    assert_eq!(
        again,
        vec![(slots[0], "P1".into()), (slots[1], "P2".into())]
    );

    // A kick of local 1's slot takes the whole machine.
    let before = left(&net).len();
    net.host.kick(slots[1], false);
    net.run_ms(500);
    let kicked: Vec<_> = left(&net)[before..].to_vec();
    assert_eq!(
        kicked,
        vec![
            (slots[0], LeaveReason::Kicked),
            (slots[1], LeaveReason::Kicked)
        ]
    );
    assert_eq!(rejected(&net.clients[back]), Some(RejectReason::Kicked));
    // The other guest was never touched.
    assert!(net.clients[stay].core.slot().is_some());
}

/// A rejoin with fewer locals than before replaces the old link: the slot
/// it does not take back is released (reserved), not left connected to a
/// dead link.
#[test]
fn locals_rejoin_with_fewer_locals_releases_the_rest() {
    let mut net = Net::new(24, lan(), Net::host_config());
    let duo = net.join_with(1, machine("P1", 9, &["P2"]), Player::new(10.0, 1.0, 1));
    net.connect_all();
    let slots = net.clients[duo].core.slots();
    let solo = net.join_with(2, machine("P1", 9, &[]), Player::new(10.0, 1.0, 2));
    net.run_until(5_000, |n| n.clients[solo].core.slot().is_some());
    assert_eq!(net.clients[solo].core.slots(), vec![slots[0]]);
    // The reclaimed slot reconnects; the released one left.
    assert_eq!(
        left(&net),
        vec![
            (slots[0], LeaveReason::Replaced),
            (slots[1], LeaveReason::Left)
        ]
    );
    assert_eq!(
        net.host.players().iter().map(|p| p.0).collect::<Vec<_>>(),
        vec![0, slots[0]]
    );
}

/// A machine's extra locals with their seats and names.
fn seated(name: &str, token: u64, extra: &[(u8, &str)]) -> ClientConfig {
    let mut c = ClientConfig::new(BUILD, name, token);
    c.locals = extra
        .iter()
        .map(|(seat, n)| LocalHello {
            seat: *seat,
            name: (*n).to_owned(),
            profile: vec![*seat],
        })
        .collect();
    c
}

/// Rejoin tokens follow the seat, not the place in the `Hello`: when seat 1
/// leaves the line-up, seat 2 reclaims its own slot with its own name, and
/// a new player in a reclaimed seat gets their own name.
#[test]
fn locals_rejoin_follows_seats_and_names() {
    let mut net = Net::new(25, lan(), Net::host_config());
    let trio = net.join_with(
        1,
        seated("Han", 77, &[(1, "Leia"), (2, "Chewie")]),
        Player::new(10.0, 1.0, 1),
    );
    net.connect_all();
    let slots = net.clients[trio].core.slots();
    assert_eq!(slots.len(), 3);
    // Seat 1 leaves; seat 2 is now the Hello's first extra local, and a
    // new player sits in seat 3.
    let back = net.join_with(
        2,
        seated("Han", 77, &[(2, "Chewie"), (3, "Lando")]),
        Player::new(10.0, 1.0, 2),
    );
    net.run_until(5_000, |n| n.clients[back].core.slot().is_some());
    let now = net.clients[back].core.slots();
    assert_eq!(&now[..2], &[slots[0], slots[2]], "seat 2 keeps its slot");
    assert!(!slots.contains(&now[2]));
    let rejoined: Vec<_> = joined(&net)[3..]
        .iter()
        .map(|j| (j.0, j.1.clone(), j.3))
        .collect();
    assert_eq!(
        rejoined,
        vec![
            (slots[0], "Han".into(), true),
            (slots[2], "Chewie".into(), true),
            (now[2], "Lando".into(), false),
        ]
    );
    // Seat 1's slot was released (it left), the others reconnected.
    assert!(left(&net).contains(&(slots[1], LeaveReason::Left)));
    assert!(left(&net).contains(&(slots[2], LeaveReason::Replaced)));

    // Someone else in a reclaimed seat: the slot comes back under the new
    // player's own name, not the old one.
    let swap = net.join_with(
        3,
        seated("Han", 77, &[(2, "Wedge"), (3, "Lando")]),
        Player::new(10.0, 1.0, 3),
    );
    net.run_until(5_000, |n| n.clients[swap].core.slot().is_some());
    let last = joined(&net);
    let seat2 = last.iter().rev().find(|j| j.0 == slots[2]).unwrap();
    assert_eq!((seat2.1.as_str(), seat2.3), ("Wedge", true));
}

/// A kick of a reserved slot frees its machine's other reserved slots too.
#[test]
fn locals_kick_of_reserved_slot_frees_the_machine() {
    let mut net = Net::new(26, lan(), Net::host_config());
    let duo = net.join_with(1, machine("P1", 5, &["P2"]), Player::new(10.0, 1.0, 1));
    net.connect_all();
    let slots = net.clients[duo].core.slots();
    net.clients[duo].core.leave();
    net.run_until(5_000, |n| left(n).len() >= 2);
    let before = left(&net).len();
    net.host.kick(slots[1], false);
    net.run_ms(100);
    assert_eq!(
        left(&net)[before..],
        [
            (slots[0], LeaveReason::Kicked),
            (slots[1], LeaveReason::Kicked)
        ]
    );
    // Nothing left to reclaim: the same token joins as new players.
    let back = net.join_with(2, machine("P1", 5, &["P2"]), Player::new(10.0, 1.0, 2));
    net.run_until(5_000, |n| n.clients[back].core.slot().is_some());
    assert!(joined(&net)[2..].iter().all(|j| !j.3));
}

/// A machine seated only in part (local 0 out, local 1 in): it still counts
/// for hash agreement, and its unseated local gets `PeerCaughtUp` once the
/// round runs so the game can add it.
#[test]
fn locals_partly_seated_machine_counts_and_joins_late() {
    let mut net = Net::new(27, lan(), Net::host_config());
    let duo = net.join_with(1, machine("P1", 8, &["P2"]), Player::new(10.0, 1.0, 1));
    net.connect_all();
    let slots = net.clients[duo].core.slots();
    net.host.load_round(b"setup".to_vec(), &[0, slots[1]]);
    net.run_until(5_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::RoundStarted { .. }))
            .is_some()
    });
    net.run_until(5_000, |n| {
        n.host_event(|e| matches!(e, NetEvent::PeerCaughtUp { slot, .. } if *slot == slots[0]))
            .is_some()
    });
    assert!(net
        .host_event(|e| matches!(e, NetEvent::PeerCaughtUp { slot, .. } if *slot == slots[0]))
        .is_some());
    assert!(net
        .host_event(|e| matches!(e, NetEvent::PeerCaughtUp { slot, .. } if *slot == slots[1]))
        .is_none());
    assert!(net.host.add_human(slots[0], TEAM_AUTO));
    net.run_until(10_000, |n| n.host.next_tick() >= 40);
    assert!(net.host.is_active(slots[0]) && net.host.is_active(slots[1]));

    // Agreement waits for the machine's report, keyed by local 0's slot.
    net.host.report_hash(30, 0xAB);
    net.run_ms(300);
    assert!(net
        .host_event(|e| matches!(e, NetEvent::HashesAgreed { tick: 30, .. }))
        .is_none());
    net.clients[duo].core.report_hash(30, 0xAB);
    net.run_ms(300);
    assert!(net
        .host_event(|e| matches!(e, NetEvent::HashesAgreed { tick: 30, .. }))
        .is_some());
}

/// B16: the host's own extra local takes slot 1, so a guest joining after
/// it gets slot 2; every bundle carries slot 1's pads, closed with slot 0's
/// on the same sequences, and the guest steps the same bundles.
#[test]
fn locals_host_local_takes_a_slot_and_its_pads_reach_the_bundles() {
    let mut net = Net::new(31, lan(), Net::host_config());
    assert_eq!(
        net.host.add_local("Host", vec![5]),
        Ok((1, "Host_1".to_owned()))
    );
    net.host_extra = vec![1];
    let guest = net.join(1, "Solo", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    assert_eq!(net.clients[guest].core.slots(), vec![2]);
    assert_eq!(
        net.host.players(),
        vec![
            (0, "Host".to_owned()),
            (1, "Host_1".to_owned()),
            (2, "Solo".to_owned())
        ]
    );
    assert_eq!(net.host.host_locals(), vec![0, 1]);
    assert!(net.host.host_local(1) && !net.host.host_local(2));
    // A guest machine with two locals takes the next two.
    let duo = net.join_with(2, machine("Left", 2, &["Right"]), Player::new(10.0, 1.0, 2));
    net.connect_all();
    assert_eq!(net.clients[duo].core.slots(), vec![3, 4]);
    // The host's locals are never kicked, benched or reclaimed by a guest.
    net.host.kick(1, true);
    assert!(!net.host.bench(1));
    assert!(net.host.host_local(1));

    net.start_round();
    let roster = net.clients[guest].events.iter().find_map(|e| match e {
        ClientEvent::LoadRound { roster, .. } => {
            Some(roster.iter().map(|r| r.slot).collect::<Vec<_>>())
        }
        _ => None,
    });
    assert_eq!(roster, Some(vec![0, 1, 2, 3, 4]));
    net.run_until(60_000, |n| n.host.next_tick() >= 600);
    net.host_player.playing = false;
    for c in &mut net.clients {
        c.player.playing = false;
    }
    net.run_ms(2_000);

    let host = &net.host_bundles;
    assert!(
        host.iter().all(|b| b.pad(1).is_some()),
        "slot 1 in every bundle"
    );
    let g = &net.clients[guest];
    assert_eq!(g.bundles[..], host[..g.bundles.len()]);
    assert!(g.bundles.len() > 500);
    let sent = &net.host_extra_submitted[0];
    let got = slot_pads(host, 1);
    let sum = |v: &[core::sim::PadState]| {
        v.iter().fold((0i64, 0i64), |s, p| {
            (s.0 + i64::from(p.mx), s.1 + i64::from(p.lx))
        })
    };
    // Every pad the local queued while it played reached the bundles (the
    // host submits before the round starts too; those are not queued).
    let played: Vec<_> = sent[sent.len() - got.len().min(sent.len())..].to_vec();
    assert_eq!(sum(&got).0, sum(&played).0, "mouse x");
    assert_eq!(
        edges(got.iter(), core::sim::buttons::FIRE),
        edges(played.iter(), core::sim::buttons::FIRE),
        "FIRE presses"
    );
    assert_ne!(slot_pads(host, 0), got);
    let mut live = 0;
    for t in host {
        assert_eq!(t.consumed(0), t.consumed(1), "tick {}", t.tick);
        live += usize::from(t.consumed(1).is_some_and(|s| s > 0));
    }
    assert!(live > 500, "{live} ticks consumed input");
}

/// B16: a host local added and taken away; one added mid-round is seated
/// with `add_human` and lines up with slot 0.
#[test]
fn locals_host_local_added_mid_round_lines_up_and_leaves() {
    let mut net = Net::new(32, lan(), Net::host_config());
    net.join(1, "Solo", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    net.start_round();
    net.run_until(30_000, |n| n.host.next_tick() >= 100);
    let (slot, _) = net.host.add_local("Right", vec![]).expect("a free slot");
    assert_eq!(slot, 2);
    net.host_extra = vec![slot];
    net.run_ms(200);
    assert!(net.host.add_human(slot, 1));
    net.run_until(30_000, |n| n.host.is_active(slot));
    let seated = net.host.next_tick();
    net.run_until(30_000, |n| n.host.next_tick() >= seated + 300);
    for t in &net.host_bundles[seated as usize..] {
        assert_eq!(t.consumed(0), t.consumed(slot), "tick {}", t.tick);
    }
    assert!(slot_pads(&net.host_bundles[seated as usize..], slot).len() >= 300);

    // Taken away: a bot takes its place and the slot is free again.
    assert!(net.host.remove_local(slot));
    assert!(!net.host.remove_local(0));
    let gone = net.host.next_tick();
    net.run_until(30_000, |n| n.host.next_tick() >= gone + 10);
    assert!(net.host_bundles[gone as usize..].iter().any(|b| b
        .commands
        .iter()
        .any(|c| matches!(c, SessionCommand::RemoveHuman { slot: s } if *s == slot))));
    assert!(net.host_bundles.last().unwrap().pad(slot).is_none());
    assert_eq!(
        net.host.players().iter().map(|p| p.0).collect::<Vec<_>>(),
        [0, 1]
    );
}

/// B16: a host local seated in the same frame it is added (a pad joining
/// mid-round) lines up with slot 0 from its first tick.
#[test]
fn locals_host_local_seated_at_once_lines_up() {
    let mut net = Net::new(33, lan(), Net::host_config());
    net.join(1, "Solo", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    net.start_round();
    net.run_until(30_000, |n| n.host.next_tick() >= 100);
    let (slot, _) = net.host.add_local("Right", vec![]).expect("a free slot");
    net.host_extra = vec![slot];
    assert!(net.host.add_human(slot, 1));
    net.run_until(30_000, |n| n.host.is_active(slot));
    let seated = net.host.next_tick();
    net.run_until(30_000, |n| n.host.next_tick() >= seated + 300);
    let ticks = &net.host_bundles[seated as usize - 1..];
    for t in ticks {
        assert_eq!(t.consumed(0), t.consumed(slot), "tick {}", t.tick);
    }
    assert!(
        ticks
            .iter()
            .filter(|t| t.consumed(slot).is_some_and(|s| s > 0))
            .count()
            > 250
    );
}

/// A refused host local says why, the reasons a guest machine's Hello
/// gets: a bad name, a round loading, no free slot.
#[test]
fn locals_host_local_refusal_names_its_reason() {
    let config = HostConfig {
        max_players: 2,
        ..Net::host_config()
    };
    let mut net = Net::new(36, lan(), config);
    assert_eq!(net.host.add_local("", vec![]), Err(RejectReason::BadName));
    net.host.load_round(b"setup".to_vec(), &[0]);
    assert_eq!(net.host.add_local("Leia", vec![]), Err(RejectReason::Busy));
    net.run_until(5_000, |n| {
        n.host_events
            .iter()
            .any(|(_, e)| matches!(e, NetEvent::RoundStarted { .. }))
    });
    assert_eq!(
        net.host.add_local("Leia", vec![]),
        Ok((1, "Leia".to_owned()))
    );
    assert_eq!(net.host.add_local("Lando", vec![]), Err(RejectReason::Full));
}

/// B16: a host local whose name a guest already holds gets the same unique
/// name from `add_local` as every guest sees in the roster.
#[test]
fn locals_host_local_name_matches_the_roster() {
    let mut net = Net::new(34, lan(), Net::host_config());
    let guest = net.join(1, "Han", 1, Player::new(10.0, 1.0, 1));
    net.connect_all();
    let (slot, name) = net.host.add_local("han", vec![]).expect("a free slot");
    assert_eq!((slot, name.as_str()), (2, "han_1"));
    net.host_extra = vec![slot];
    net.start_round();
    let roster = net.clients[guest].events.iter().find_map(|e| match e {
        ClientEvent::LoadRound { roster, .. } => roster
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.name.clone()),
        _ => None,
    });
    assert_eq!(roster.as_deref(), Some(name.as_str()));
}

/// B16: `add_local` between two locals' submits of one frame is caught.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "add_local between submit_input_for calls")]
fn locals_host_local_added_mid_frame_is_caught() {
    let mut net = Net::new(35, lan(), Net::host_config());
    net.host.add_local("Left", vec![]).expect("a free slot");
    net.host.submit_input(core::sim::PadState::default(), 0);
    let _ = net.host.add_local("Right", vec![]);
}

/// Two host locals' frames through the threaded session's commands, with
/// frame hitches that drain the queues: the row of one frame is one
/// command, so a tick closing on the thread never falls between its slots
/// and both consume the same sequence on every tick. The old per-slot
/// stream (`TornSession`: the thread's update run between slot 0's
/// command and slot 1's) starves slot 1 at such a close and parts them.
#[test]
fn locals_host_session_submits_a_frame_in_one_command() {
    let run = |submit: HostSubmit| {
        let mut net = Net::new(34, lan(), Net::host_config());
        net.host_submit = submit;
        let (slot, _) = net.host.add_local("Right", vec![]).expect("a free slot");
        net.host_extra = vec![slot];
        net.join(1, "Solo", 1, Player::new(10.0, 1.0, 1));
        net.connect_all();
        net.start_round();
        // Frame hitches: the host's frames stop, its queues drain, and
        // the frame after a hitch sends several rows at once.
        while net.host.next_tick() < 1500 {
            net.run_ms(400);
            net.host_player.next_frame = Micros(net.now.0 + 60_000);
        }
        let ticks = &net.host_bundles[50..];
        let parted = ticks
            .iter()
            .filter(|t| t.consumed(0) != t.consumed(slot))
            .count();
        let live = ticks
            .iter()
            .filter(|t| t.consumed(slot).is_some_and(|s| s > 0))
            .count();
        (parted, live, net.host_torn_closes)
    };
    let (parted, live, _) = run(HostSubmit::Session);
    assert_eq!(parted, 0, "one command keeps the locals together");
    assert!(live > 1300, "{live} ticks consumed input");
    // Counterfactual: 20 closes fell between the slots' commands and 12
    // ticks parted when this was written.
    let (torn, _, closes) = run(HostSubmit::TornSession);
    assert!(closes > 0, "a close fell between two slots' commands");
    assert!(torn > 0, "the per-slot stream parts the locals");
}
