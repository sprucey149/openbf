//! Focused checks of the netplay DTOs, commands and one in-process lockstep
//! round on an asset-free fixture battle.

use super::desync;
use super::virtual_net::{Builder, VirtualNet};
use super::*;
use core::sim::PadState;
use net::{LinkParams, Micros};

fn seat(slot: u8) -> HumanSeat {
    HumanSeat {
        slot,
        name: format!("P{slot}"),
        team: None,
        cheats: 0,
        profile: PlayerProfile {
            name: format!("P{slot}"),
            unlock_kills: [slot.into(), 50, 100, 0, 7, 0, 0, 1],
            ..PlayerProfile::default()
        },
    }
}

pub(super) fn setup() -> RoundSetup {
    let mut s = RoundSetup::instant_action(
        "bf2_kas2",
        "con",
        ["rep".into(), "cis".into()],
        &ui::shell::SessionOptions::default(),
        &crate::profile::Profile::default(),
    );
    s.cheat_policy = 1 << cheat_index("stamina").unwrap();
    s.humans.push(seat(0));
    s
}

/// The setup and a profile survive the wire byte for byte; the largest
/// profile fits an `AddHuman` command.
#[test]
fn netplay_setup_and_profile_round_trip() {
    let mut s = setup();
    s.humans[0].profile.assist.intensity = 0.7;
    s.humans[0].profile.medals = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    s.humans.push(HumanSeat {
        team: Some(1),
        cheats: ALL_CHEATS,
        ..seat(5)
    });
    let back = RoundSetup::decode(&s.encode()).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.encode(), s.encode());
    let mut big = PlayerProfile {
        name: "W".repeat(net::msg::MAX_NAME_CHARS),
        medals: [u32::MAX; 9],
        unlock_kills: [u32::MAX; 8],
        ..PlayerProfile::default()
    };
    big.assist.intensity = 0.123_456_79;
    let bytes = big.encode();
    assert!(
        bytes.len() <= net::bundle::MAX_COMMAND_BYTES,
        "{}",
        bytes.len()
    );
    assert_eq!(PlayerProfile::decode_or_default(&bytes), big);
    assert_eq!(
        PlayerProfile::decode_or_default(b"junk"),
        PlayerProfile::default()
    );
    let mut bad = s.clone();
    bad.humans[0].slot = 40;
    assert!(RoundSetup::decode(&bad.encode()).is_err());
}

/// The host offers maps everyone has with equal digests, games everyone
/// has, and the addon only when all have it.
#[test]
fn netplay_content_intersection() {
    let m = |maps: &[&str], games: &[&str], addon: bool| ContentManifest {
        maps: maps.iter().map(|k| (*k).to_owned()).collect(),
        games: games.iter().map(|g| (*g).to_owned()).collect(),
        addon_heroes: addon,
        ..ContentManifest::default()
    };
    let a = m(&["bf1_tat2", "bf2_kas2", "bf2_cor1"], &["bf1", "bf2"], true);
    let b = m(&["bf2_kas2", "bf2_cor1"], &["bf2"], true);
    let c = m(&["bf2_kas2", "bf1_tat2"], &["bf1", "bf2"], false);
    let i = ContentManifest::intersect([&a, &b, &c]);
    assert_eq!(i.maps.iter().collect::<Vec<_>>(), ["bf2_kas2"]);
    assert_eq!(i.games.iter().collect::<Vec<_>>(), ["bf2"]);
    assert!(!i.addon_heroes);
    assert!(ContentManifest::intersect([&a, &a]).addon_heroes);
    assert_eq!(ContentManifest::intersect([&a]), a);
    assert_eq!(ContentManifest::intersect([]), ContentManifest::default());
    assert_eq!(ContentManifest::decode(&a.encode()), Some(a));
    assert!(!build_id().is_empty());
}

/// The pre-join check asks only whether everyone has the map: two copies of
/// the same map whose `manifest.ron` differs (a Windows bake's CRLF line
/// endings, a PC copy's extra textures) scan to the same manifest, so mixed
/// copies keep every map in common. `Sim::static_digest` at load decides
/// whether they simulate the same.
#[test]
fn netplay_content_scan_ignores_manifest_bytes() {
    let root = std::env::temp_dir().join(format!("openbf_content_scan_{}", std::process::id()));
    let write = |copy: &str, key: &str, text: &str| {
        let dir = root.join(copy).join(key);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.ron"), text).unwrap();
    };
    let lf =
        "(\n    map: \"kas2\",\n    source_game: \"BF2\",\n    textures: [\"kas2_main\"],\n)\n";
    let crlf = lf.replace('\n', "\r\n");
    let pc = lf.replace("[\"kas2_main\"]", "[\"kas2_main\", \"kas2_sky_clouds\"]");
    write("linux", "bf2_kas2", lf);
    write("windows", "bf2_kas2", &crlf);
    write("pc", "bf2_kas2", &pc);
    let scan = |copy: &str| ContentManifest::scan(&root.join(copy));
    let (linux, windows, pc) = (scan("linux"), scan("windows"), scan("pc"));
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(linux.maps.iter().collect::<Vec<_>>(), ["bf2_kas2"]);
    assert_eq!(linux.games.iter().collect::<Vec<_>>(), ["bf2"]);
    assert_eq!(linux, windows);
    assert_eq!(linux, pc);
    let common = ContentManifest::intersect([&linux, &windows, &pc]);
    assert_eq!(common.maps.iter().collect::<Vec<_>>(), ["bf2_kas2"]);
}

/// Guests ask only for their own slot and only for allowed cheats; off is
/// always allowed; junk is refused.
#[test]
fn netplay_requests_are_checked_against_the_policy() {
    let stamina = cheat_index("stamina").unwrap();
    let classes = cheat_index("classes").unwrap();
    let policy = 1 << stamina;
    let ok = validate_request(
        3,
        &GameRequest::CheatToggle {
            id: stamina,
            on: true,
        },
        policy,
    );
    assert_eq!(
        ok,
        Ok(GameCommand::SetSlotCheat {
            slot: 3,
            id: stamina,
            on: true
        })
    );
    let no = validate_request(
        3,
        &GameRequest::CheatToggle {
            id: classes,
            on: true,
        },
        policy,
    );
    assert_eq!(no, Err(Refusal::NotAllowed));
    let off = validate_request(
        3,
        &GameRequest::CheatToggle {
            id: classes,
            on: false,
        },
        policy,
    );
    assert_eq!(
        off,
        Ok(GameCommand::SetSlotCheat {
            slot: 3,
            id: classes,
            on: false
        })
    );
    let bad = validate_request(
        3,
        &GameRequest::CheatToggle {
            id: CHEAT_COUNT,
            on: true,
        },
        ALL_CHEATS,
    );
    assert_eq!(bad, Err(Refusal::UnknownCheat));
    assert_eq!(
        validate_request_bytes(1, b"{", policy),
        Err(Refusal::Malformed)
    );
    let req = GameRequest::CheatToggle {
        id: stamina,
        on: true,
    };
    assert_eq!(
        validate_request_bytes(1, &req.encode(), policy).map(|_| ()),
        Ok(())
    );
    for bits in [0, 1, 0b1010, ALL_CHEATS] {
        assert_eq!(cheat_bits(&cheats_from_bits(bits)), bits);
    }
    let cmd = GameCommand::SetCheatPolicy { allowed: 5 };
    assert_eq!(GameCommand::decode(&cmd.encode()), Some(cmd));
    assert_eq!(GameCommand::decode(b"nope"), None);
}

pub(super) fn fixture(setup: &RoundSetup, local: u8) -> anyhow::Result<crate::walk::WalkSim> {
    let mut walk = crate::walk::WalkSim::new(setup.seed, glam::Vec3::ZERO, None);
    walk.spawn_dummy_targets();
    walk.local_slot = local;
    install_humans(&mut walk, setup)?;
    Ok(walk)
}

/// Cheat commands per slot and for everyone; a narrower policy clears what
/// it no longer allows.
#[test]
fn netplay_cheat_commands_apply_per_slot() {
    let mut s = setup();
    s.humans.push(HumanSeat {
        cheats: 1 << cheat_index("ammo").unwrap(),
        ..seat(2)
    });
    let mut walk = fixture(&s, 0).unwrap();
    let bits = |w: &crate::walk::WalkSim, slot| cheat_bits(&w.sim.cheats_for_slot(slot));
    assert_eq!(bits(&walk, 2), 1 << cheat_index("ammo").unwrap());
    assert_eq!(bits(&walk, 0), 0);
    apply_game(
        &mut walk,
        &GameCommand::SetSlotCheats {
            slot: ALL_SLOTS,
            bits: 0b111,
        },
    );
    assert_eq!((bits(&walk, 0), bits(&walk, 2)), (0b111, 0b111));
    apply_game(
        &mut walk,
        &GameCommand::SetSlotCheat {
            slot: 2,
            id: 1,
            on: false,
        },
    );
    assert_eq!((bits(&walk, 0), bits(&walk, 2)), (0b111, 0b101));
    apply_game(&mut walk, &GameCommand::SetCheatPolicy { allowed: 0b100 });
    assert_eq!((bits(&walk, 0), bits(&walk, 2)), (0b100, 0b100));
    assert_eq!(walk.netplay.cheat_policy, 0b100);
}

/// The award policy (LAN §5b): Off in the setup leaves every slot without
/// its earned awards; the host's command turns them back to each player's
/// own preference, on every slot alike.
#[test]
fn netplay_award_policy_applies_to_every_slot() {
    let mut s = setup();
    s.awards = false;
    s.humans.push(seat(2));
    let back = RoundSetup::decode(&s.encode()).unwrap();
    assert!(!back.awards);
    let mut walk = fixture(&s, 0).unwrap();
    let on = |w: &crate::walk::WalkSim| [0, 2].map(|slot| w.sim.slot_career_award_bonuses(slot));
    assert!(
        walk.sim.slot_medal_counts(2).is_some(),
        "slot 2 has a career"
    );
    assert_eq!(on(&walk), [false, false]);
    apply_game(&mut walk, &GameCommand::SetAwardPolicy { on: true });
    assert_eq!(on(&walk), [true, true]);
    // A player's own Off holds through the policy's changes.
    apply_game(
        &mut walk,
        &GameCommand::SetCareerAwards { slot: 2, on: false },
    );
    assert_eq!(on(&walk), [true, false]);
    apply_game(&mut walk, &GameCommand::SetAwardPolicy { on: false });
    assert_eq!(on(&walk), [false, false]);
    apply_game(
        &mut walk,
        &GameCommand::SetCareerAwards { slot: 0, on: true },
    );
    assert_eq!(on(&walk), [false, false], "the policy wins while Off");
    apply_game(&mut walk, &GameCommand::SetAwardPolicy { on: true });
    assert_eq!(on(&walk), [true, false]);
    let cmd = GameCommand::SetAwardPolicy { on: false };
    assert_eq!(GameCommand::decode(&cmd.encode()), Some(cmd));
    // A setup without the field (a desync report from before the award
    // policy) is refused, not guessed.
    let mut json: serde_json::Value = serde_json::from_slice(&setup().encode()).unwrap();
    json.as_object_mut().unwrap().remove("awards");
    assert!(RoundSetup::decode(&serde_json::to_vec(&json).unwrap()).is_err());
}

use super::apply::apply_game;

/// Two peers in virtual time on the fixture battle: a guest joins mid-round
/// (catch-up from the log, `AddHuman`), plays, then leaves (`RemoveHuman`);
/// both peers' hashes agree on every tick both stepped.
#[test]
fn netplay_two_peers_join_and_leave_in_lockstep() {
    let link = LinkParams {
        latency: Micros::from_millis(20),
        jitter: Micros::from_millis(5),
        loss: 0.03,
        duplicate: 0.01,
        reorder: 0.01,
    };
    let builder: Builder = Box::new(|s: &RoundSetup, local: u8, _node: usize| {
        Ok((
            fixture(s, local)?,
            Some(crate::event_feed::EventFeed::default()),
        ))
    });
    let mut v = VirtualNet::new(7, link, "test-build", seat(0).profile.encode(), builder);
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 60));
    let g = v.join("Guest", 42, seat(1).profile.encode(), 15.0);
    assert!(v.run_until(15_000, |v| v.host_tick() >= 200));
    v.nodes[g].link.client_mut().unwrap().leave();
    assert!(v.run_until(15_000, |v| v.host_tick() >= 300));
    let host = &v.nodes[0];
    let guest = &v.nodes[g];
    let slot = guest.peer.local_slot;
    assert_eq!(slot, 1);
    let joined = host.applied.iter().find_map(|(t, a)| match a {
        Applied::Joined { slot: 1, ok } => Some((*t, *ok)),
        _ => None,
    });
    let (join_tick, ok) = joined.expect("the guest was added");
    assert!(ok && join_tick > 60, "{join_tick}");
    assert!(host
        .applied
        .iter()
        .any(|(t, a)| matches!(a, Applied::Left { slot: 1, .. }) && *t > join_tick));
    assert_eq!(guest.catch_up.len(), 1);
    assert!(guest.peer.stats.catch_up_ticks >= 60);
    assert!(
        guest.peer.next_tick() >= join_tick + 50,
        "{}",
        guest.peer.next_tick()
    );
    assert_eq!(guest.builds[0].0, host.builds[0].0, "static digests");
    assert!(
        v.checker.checks >= u64::from(guest.peer.next_tick()) - 5,
        "{}",
        v.checker.checks
    );
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
    assert_eq!(host.peer.stats.desyncs + guest.peer.stats.desyncs, 0);
    assert!(!guest.latencies_ms.is_empty());
    let w = host.walk.as_ref().unwrap();
    assert!(
        w.sim.players.get(1).copied().flatten().is_none(),
        "slot 1 left"
    );
    assert!(!w.netplay.profiles.contains_key(&1));
}

/// The windowed peer's per-frame step on a host (a bare `HostCore` in
/// virtual time): frames step the round, the prediction runs over the
/// inputs pending, and drawing (blend, folded look, prediction swapped in
/// and out) never changes the confirmed sim's hash.
#[test]
fn netplay_local_view_frames_draw_without_touching_the_sim() {
    let mut core = net::HostCore::new(net::HostConfig::new("test-build", "Host"));
    let mut peer = Peer::host();
    let bytes = peer.host_load(&mut core, &setup());
    let mut walk = fixture(&RoundSetup::decode(&bytes).unwrap(), 0).unwrap();
    peer.round_built(&mut core, &mut walk);
    let mut view = LocalView::new();
    let mut now = Micros::from_millis(1);
    let (mut swapped, mut muted, mut fed, mut ticks) = (0, 0, 0, 0);
    for frame in 0..240u32 {
        core.update(now);
        peer.poll(&mut core);
        let pad = PadState {
            ly: PadState::quantize(1.0),
            rx: PadState::quantize(0.3),
            buttons: if frame % 40 < 20 {
                core::sim::buttons::FIRE
            } else {
                0
            },
            ..PadState::default()
        };
        let report = view.frame(
            &mut peer,
            &mut core,
            1.0 / 60.0,
            pad,
            &mut walk,
            None,
            |_, _, _, m| {
                muted += usize::from(m);
            },
        );
        ticks += report.ticks;
        fed += view.shots().len();
        let hash = walk.sim.hash();
        view.draw_in(&mut walk);
        swapped += usize::from(view.predictor.swapped());
        view.draw_out(&mut walk);
        assert_eq!(
            walk.sim.hash(),
            hash,
            "frame {frame}: drawing reached the sim"
        );
        assert!((0.0..=1.0).contains(&view.fraction()));
        now = now + Micros(16_667);
    }
    assert_eq!(peer.mode(), Mode::Playing);
    assert!(ticks >= 110, "{ticks} ticks");
    assert!(swapped > 0, "the prediction was drawn");
    assert!(fed > 0 && muted > 0, "fed {fed}, muted {muted}");
}

/// Desync reports (LAN §4.7): a guest's sim nudged 1 cm makes the host's
/// check fail; both sides write a report, the host's with the guest's rows;
/// the rows' first difference is the `units` group at the nudged tick, and
/// a re-simulation from the report's setup and bundles agrees with the
/// host, not the nudged guest.
#[test]
fn netplay_desync_writes_both_reports_naming_the_group() {
    let link = LinkParams {
        latency: Micros::from_millis(20),
        jitter: Micros::from_millis(5),
        loss: 0.02,
        duplicate: 0.0,
        reorder: 0.0,
    };
    let builder: Builder = Box::new(|s: &RoundSetup, local: u8, _node: usize| {
        Ok((
            fixture(s, local)?,
            Some(crate::event_feed::EventFeed::default()),
        ))
    });
    let dir = std::env::temp_dir().join(format!("openbf-desync-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut v = VirtualNet::new(9, link, "test-build", seat(0).profile.encode(), builder);
    v.nodes[0].peer.report_dir = Some(dir.clone());
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let g = v.join("Guest", 43, seat(1).profile.encode(), 15.0);
    v.nodes[g].peer.report_dir = Some(dir.clone());
    assert!(
        v.run_until(15_000, |v| v.nodes[g].peer.mode() == Mode::Playing
            && v.nodes[g].peer.next_tick() >= 100)
    );
    let nudged = v.nodes[g].peer.next_tick();
    let w = v.nodes[g].walk.as_mut().unwrap();
    let u = w.sim.units.iter_mut().find(|u| u.alive).unwrap();
    u.soldier.pos.x += 0.01;
    assert!(
        v.run_until(20_000, |v| v.nodes[0].peer.desync_reports.len() == 1
            && v.nodes[g].peer.desync_reports.len() == 1)
    );
    let host = DesyncReport::read(&v.nodes[0].peer.desync_reports[0]).unwrap();
    let guest = DesyncReport::read(&v.nodes[g].peer.desync_reports[0]).unwrap();
    assert_eq!((host.side.as_str(), guest.side.as_str()), ("host", "guest"));
    assert_eq!((host.slot, guest.slot, host.tick), (1, 1, guest.tick));
    assert_eq!(host.session, guest.session);
    assert!(host.tick >= nudged && host.tick < nudged + 30 + HASH_EVERY);
    assert_ne!(host.host.hash, host.guest.hash);
    assert_eq!(host.guest.hash, guest.guest.hash);
    assert_eq!(host.host.hash, guest.host.hash);
    assert_eq!(host.guest.target, net::local_target());
    assert_eq!(
        host.guest.rows, guest.guest.rows,
        "the guest's rows reached the host"
    );
    assert_eq!(host.host.rows.len(), desync::REPORT_ROWS as usize);
    assert!(!host.static_groups.is_empty() && !host.bundles.is_empty());
    let (hr, gr) = (
        desync::parse_rows(&host.host.rows).unwrap(),
        desync::parse_rows(&host.guest.rows).unwrap(),
    );
    let (tick, cols) = desync::first_difference(&hr, &gr).expect("the rows differ");
    assert_eq!(tick, nudged, "{cols:?}");
    assert!(cols.contains(&"units"), "{cols:?}");
    // Re-simulated from the guest's own file, this build agrees with the
    // host throughout and parts from the nudged guest at the nudge.
    let setup = guest.round_setup().unwrap();
    let mut walk = fixture(&setup, guest.local_slot).unwrap();
    let mut feed = crate::event_feed::EventFeed::default();
    let (stepped, with_host, with_guest) =
        desync::resimulate(&guest, &hr, &gr, &mut walk, Some(&mut feed)).unwrap();
    assert_eq!(stepped, host.tick + 1);
    assert!(
        with_host.compared > 0 && with_host.differs.is_none(),
        "{with_host:?}"
    );
    assert_eq!(with_guest.differs.map(|d| d.0), Some(nudged));
    let _ = std::fs::remove_dir_all(&dir);
}

fn logged(v: &VirtualNet, node: usize, what: &str) -> bool {
    v.nodes[node].log.iter().any(|(_, l)| l.contains(what))
}

fn clean_link() -> LinkParams {
    LinkParams {
        latency: Micros::from_millis(10),
        jitter: Micros::from_millis(2),
        loss: 0.0,
        duplicate: 0.0,
        reorder: 0.0,
    }
}

fn fixture_builder() -> Builder {
    Box::new(|s: &RoundSetup, local: u8, _node: usize| {
        Ok((
            fixture(s, local)?,
            Some(crate::event_feed::EventFeed::default()),
        ))
    })
}

/// LAN §4.7: a guest's first desync rejoins; its second within
/// `DESYNC_WINDOW_S` does not ask for the round again: it leaves for the
/// lobby and tells the host, whose notice names the groups that parted.
#[test]
fn netplay_a_second_desync_sends_the_guest_to_the_lobby() {
    let mut v = VirtualNet::new(
        11,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        fixture_builder(),
    );
    v.nodes[0].peer.report_dir = None;
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let g = v.join("Guest", 44, seat(1).profile.encode(), 15.0);
    v.nodes[g].peer.report_dir = None;
    let nudge = |v: &mut VirtualNet| {
        let w = v.nodes[g].walk.as_mut().unwrap();
        let u = w.sim.units.iter_mut().find(|u| u.alive).unwrap();
        u.soldier.pos.x += 0.01;
    };
    let playing = |v: &VirtualNet| {
        v.nodes[g].peer.mode() == Mode::Playing && v.nodes[g].peer.next_tick() >= 100
    };
    assert!(v.run_until(15_000, playing));
    nudge(&mut v);
    // The first: a rejoin (a second build, caught up and playing again).
    assert!(v.run_until(20_000, |v| v.nodes[g].builds.len() == 2
        && v.nodes[g].peer.stats.desyncs == 1
        && playing(v)));
    assert!(!logged(&v, g, "DesyncAgain"));
    // The host checked itself first: its own game had not parted.
    assert!(v.nodes[0].peer.stats.host_checks >= 1);
    assert_eq!(v.nodes[0].peer.stats.host_divergences, 0);
    let builds = v.nodes[g].builds.len();
    nudge(&mut v);
    assert!(v.run_until(20_000, |v| logged(v, 0, "LeftRound")));
    assert!(logged(&v, g, "DesyncAgain"), "{:?}", v.nodes[g].log);
    assert_eq!(v.nodes[g].peer.mode(), Mode::Idle);
    let left = v.nodes[0]
        .log
        .iter()
        .find(|(_, l)| l.contains("LeftRound"))
        .unwrap();
    assert!(
        left.1.contains("slot: 1") && left.1.contains("Desynced") && left.1.contains("units"),
        "{left:?}"
    );
    // No third replay of the round.
    v.run_until(3_000, |_| false);
    assert_eq!(v.nodes[g].builds.len(), builds, "no resync asked");
}

/// Issue #168, first VPN playtest: the 5-minute window of LAN §4.7 is per
/// round. A desync late in one round and one early in the next are one
/// each: the guest rejoins both times instead of going to the lobby. The
/// next round loads while the old one runs, so the old round's hash
/// reports, bundles and builds still on their way must not reach it.
#[test]
fn netplay_a_desync_in_each_round_rejoins_both_times() {
    let mut v = VirtualNet::new(
        14,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        fixture_builder(),
    );
    v.nodes[0].peer.report_dir = None;
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let g = v.join("Guest", 48, seat(1).profile.encode(), 15.0);
    v.nodes[g].peer.report_dir = None;
    let nudge = |v: &mut VirtualNet| {
        let w = v.nodes[g].walk.as_mut().unwrap();
        let u = w.sim.units.iter_mut().find(|u| u.alive).unwrap();
        u.soldier.pos.x += 0.01;
    };
    let playing = |v: &VirtualNet, from: u32| {
        v.nodes[g].peer.mode() == Mode::Playing && v.nodes[g].peer.next_tick() >= from
    };
    assert!(v.run_until(15_000, |v| playing(v, 100)));
    nudge(&mut v);
    assert!(v.run_until(20_000, |v| v.nodes[g].peer.stats.desyncs == 1
        && v.nodes[g].builds.len() == 2
        && playing(v, 100)));
    // The next round, the guest seated from its start.
    let mut next = setup();
    next.seed += 1;
    next.humans.push(seat(v.nodes[g].peer.local_slot));
    v.host_load(&next);
    assert!(
        v.run_until(20_000, |v| v.nodes[g].builds.len() == 3 && playing(v, 60)),
        "{:?}\n{:?}",
        v.nodes[g].log,
        v.nodes[0].log
    );
    assert_eq!(v.nodes[g].peer.stats.desyncs, 1, "{:?}", v.nodes[g].log);
    assert!(!logged(&v, 0, "DigestMismatch"), "{:?}", v.nodes[0].log);
    nudge(&mut v);
    assert!(
        v.run_until(20_000, |v| v.nodes[g].peer.stats.desyncs == 2
            && v.nodes[g].builds.len() == 4
            && playing(v, 60)),
        "{:?}",
        v.nodes[g].log
    );
    assert!(!logged(&v, g, "DesyncAgain"), "{:?}", v.nodes[g].log);
    assert!(!logged(&v, 0, "LeftRound"));
}

/// Nudges the host's own sim (as a drawing write once did).
fn nudge_host(v: &mut VirtualNet) {
    let w = v.nodes[0].walk.as_mut().unwrap();
    let u = w.sim.units.iter_mut().find(|u| u.alive).unwrap();
    u.soldier.pos.x += 0.01;
}

/// A host and `guests` guests playing past tick 120.
fn playing_round(seed: u64, guests: usize) -> (VirtualNet, Vec<usize>) {
    let mut v = VirtualNet::new(
        seed,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        fixture_builder(),
    );
    v.nodes[0].peer.report_dir = None;
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let gs: Vec<usize> = (0..guests)
        .map(|i| {
            let g = v.join(
                &format!("G{i}"),
                60 + i as u64,
                seat(i as u8 + 1).profile.encode(),
                15.0 + 2.0 * i as f64,
            );
            v.nodes[g].peer.report_dir = None;
            g
        })
        .collect();
    assert!(v.run_until(20_000, |v| gs.iter().all(|g| {
        v.nodes[*g].peer.mode() == Mode::Playing && v.nodes[*g].peer.next_tick() >= 120
    })));
    (v, gs)
}

/// After the host corrected itself: no guest was told of a desync, none
/// rebuilt or left, every guest heard it once, and no hash differed after
/// the correction.
fn assert_host_corrected(v: &VirtualNet, gs: &[usize], corrections: u32) {
    let host = &v.nodes[0].peer;
    assert_eq!(host.corrections(), corrections, "{:?}", v.nodes[0].log);
    assert_eq!(host.stats.host_divergences, corrections);
    assert_eq!(host.stats.desyncs, 0, "{:?}", v.nodes[0].log);
    assert!(logged(v, 0, "HostDiverged"), "{:?}", v.nodes[0].log);
    assert!(!logged(v, 0, "LeftRound"));
    for g in gs {
        let n = &v.nodes[*g];
        assert_eq!(n.peer.stats.desyncs, 0, "{:?}", n.log);
        assert_eq!(n.builds.len(), 1, "no rejoin: {:?}", n.log);
        let heard = n
            .log
            .iter()
            .filter(|(_, l)| l.contains("HostDiverged"))
            .count();
        assert_eq!(heard, corrections as usize, "{:?}", n.log);
        assert!(!logged(v, *g, "DesyncAgain"), "{:?}", n.log);
        assert_eq!(n.peer.mode(), Mode::Playing);
    }
    let last = v
        .checker
        .last_mismatch_tick
        .expect("the host's game parted");
    assert!(
        last < host.trusted_from(),
        "a mismatch at {last} after the correction at {}",
        host.trusted_from()
    );
}

/// Issue #168, first VPN playtest: when the host's own game parts, it
/// replays its round from the log, finds the replay disagrees with its live
/// game, and takes the replay's place. The guests play on: nobody rejoins
/// or goes to the lobby, and every guest is told once.
#[test]
fn netplay_a_host_that_parts_corrects_itself_and_the_guests_play_on() {
    let (mut v, gs) = playing_round(15, 2);
    nudge_host(&mut v);
    assert!(
        v.run_until(20_000, |v| v.nodes[0].peer.corrections() == 1),
        "{:?}",
        v.nodes[0].log
    );
    // Well past the next checks.
    let t = v.host_tick();
    assert!(v.run_until(20_000, |v| gs
        .iter()
        .all(|g| v.nodes[*g].peer.next_tick() >= t + 6 * HASH_EVERY)));
    assert_host_corrected(&v, &gs, 1);
    assert_eq!(v.nodes[0].peer.stats.host_checks, 1, "one check, no loop");
}

/// The same with one guest, the host's replay on a worker thread as in the
/// window: nothing needs two guests to agree.
#[test]
fn netplay_a_one_guest_host_corrects_itself_on_a_worker() {
    let (mut v, gs) = playing_round(16, 1);
    v.set_host_rebuild(Some(Rebuild::Thread(std::sync::Arc::new(
        |s: &RoundSetup, local: u8, _: &super::selfcheck::Stop| {
            Ok((
                fixture(s, local)?,
                Some(crate::event_feed::EventFeed::default()),
            ))
        },
    ))));
    nudge_host(&mut v);
    // Virtual time waits for the worker's real time.
    let wait = |v: &VirtualNet| {
        if v.nodes[0].peer.checking() {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    };
    assert!(
        v.run_until(20_000, |v| {
            wait(v);
            v.nodes[0].peer.corrections() == 1
        }),
        "{:?}",
        v.nodes[0].log
    );
    let t = v.host_tick();
    assert!(v.run_until(20_000, |v| v.nodes[gs[0]].peer.next_tick()
        >= t + 6 * HASH_EVERY));
    assert_host_corrected(&v, &gs, 1);
}

/// Second review of issue #168: a host's game that parts late in a long
/// round is checked from its latest checkpoint (a copy of its game where
/// every guest agreed), on the worker, in a few seconds of ticks instead of
/// a replay of the whole round. The builder is never called.
#[test]
fn netplay_a_late_host_parting_is_checked_from_a_checkpoint_on_a_worker() {
    let (mut v, gs) = playing_round(18, 1);
    let late = 20_100;
    assert!(v.run_until(late as u64 * 40, |v| v.host_tick() >= late));
    let cp = v.nodes[0].peer.checkpoint_tick().expect("a checkpoint");
    assert!(
        cp + 2 * peer::CHECKPOINT_EVERY >= late,
        "checkpoint at {cp}"
    );
    v.set_host_rebuild(Some(Rebuild::Thread(std::sync::Arc::new(
        |_: &RoundSetup, _: u8, _: &super::selfcheck::Stop| {
            anyhow::bail!("built: the checkpoint was not used")
        },
    ))));
    nudge_host(&mut v);
    let wall = std::time::Instant::now();
    let wait = |v: &VirtualNet| {
        if v.nodes[0].peer.checking() {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    };
    assert!(
        v.run_until(20_000, |v| {
            wait(v);
            v.nodes[0].peer.corrections() == 1
        }),
        "{:?}",
        v.nodes[0].log
    );
    let secs = wall.elapsed().as_secs_f64();
    let st = &v.nodes[0].peer.stats;
    assert_eq!(st.host_check_failures, 0, "{:?}", v.nodes[0].log);
    assert!(st.last_check_from > cp && st.last_check_from >= late - 2 * peer::CHECKPOINT_EVERY);
    assert!(
        st.last_check_ticks < peer::CHECK_BASE_TICKS,
        "the check took {} ticks",
        st.last_check_ticks
    );
    assert!(secs < 10.0, "the check took {secs:.1} s");
    let t = v.host_tick();
    assert!(v.run_until(20_000, |v| v.nodes[gs[0]].peer.next_tick()
        >= t + 6 * HASH_EVERY));
    assert_host_corrected(&v, &gs, 1);
}

/// Third review of issue #168: a checkpoint is proven by `Sim::hash` only.
/// Here the host's game parts in what the hash does not cover but the sim
/// reads (the round's time limit, read by the round's rules; the feed's
/// round points the class gate reads are another) before a checkpoint, so
/// the checkpoint carries the parting and the replay from it agrees with
/// the parted host. The host confirms from the setup before telling
/// anyone, and corrects itself: the guest is not blamed.
#[test]
fn netplay_a_host_parting_outside_the_hash_before_a_checkpoint_is_corrected() {
    let (mut v, gs) = playing_round(20, 1);
    let every = peer::CHECKPOINT_EVERY;
    assert!(v.run_until(40_000, |v| v.host_tick() >= every + 100));
    let c1 = 2 * every;
    assert!(v.host_tick() < c1, "the next candidate still ahead");
    // Ends the host's round (`Sim::outcome`, hashed) at tick c1 + 150:
    // after the candidate at c1 is agreed on, before the next.
    let limit = (c1 + 150) / core::TICK_HZ;
    v.nodes[0].walk.as_mut().unwrap().sim.time_limit = Some(limit);
    let hash = |v: &VirtualNet| v.nodes[0].walk.as_ref().unwrap().sim.hash();
    let before = hash(&v);
    v.nodes[0].walk.as_mut().unwrap().sim.time_limit = None;
    assert_eq!(hash(&v), before, "the time limit is outside the hash");
    v.nodes[0].walk.as_mut().unwrap().sim.time_limit = Some(limit);
    assert!(
        v.run_until(40_000, |v| v.nodes[0].peer.corrections() == 1
            || v.nodes[0].peer.stats.desyncs > 0),
        "{:?}",
        v.nodes[0].log
    );
    let st = &v.nodes[0].peer.stats;
    assert_eq!(st.desyncs, 0, "the guest was blamed: {:?}", v.nodes[0].log);
    assert!(st.checkpoints >= 2, "the candidate at {c1} was agreed on");
    assert_eq!(st.host_confirm_checks, 1, "{:?}", v.nodes[0].log);
    assert_eq!(st.host_checks, 2, "the checkpoint's, then the setup's");
    assert_eq!(st.last_check_from, 0);
    let t = v.host_tick();
    assert!(v.run_until(20_000, |v| v.nodes[gs[0]].peer.next_tick()
        >= t + 6 * HASH_EVERY));
    assert_host_corrected(&v, &gs, 1);
    assert_eq!(v.nodes[0].walk.as_ref().unwrap().sim.time_limit, None);
}

/// Second review of issue #168: a self-check that gives no verdict (here
/// its build fails, before any checkpoint) falls back to the rule before
/// self-checks: the guest that differed is told and rejoins, and the round
/// goes on for everyone.
#[test]
fn netplay_a_failed_self_check_tells_the_guest_and_the_round_goes_on() {
    let (mut v, gs) = playing_round(19, 1);
    let g = gs[0];
    v.set_host_rebuild(Some(Rebuild::Inline(Box::new(|_: &RoundSetup, _: u8| {
        anyhow::bail!("no build here")
    }))));
    assert_eq!(v.nodes[0].peer.checkpoint_tick(), None);
    let w = v.nodes[g].walk.as_mut().unwrap();
    let u = w.sim.units.iter_mut().find(|u| u.alive).unwrap();
    u.soldier.pos.x += 0.01;
    assert!(
        v.run_until(20_000, |v| v.nodes[g].builds.len() == 2
            && v.nodes[g].peer.mode() == Mode::Playing),
        "{:?}\n{:?}",
        v.nodes[0].log,
        v.nodes[g].log
    );
    let host = &v.nodes[0].peer;
    assert_eq!(host.stats.host_check_failures, 1, "{:?}", v.nodes[0].log);
    assert_eq!(host.stats.host_divergences, 0);
    assert_eq!(host.stats.desyncs, 1);
    assert!(logged(&v, 0, "SelfCheckFailed"), "{:?}", v.nodes[0].log);
    assert_eq!(v.nodes[g].peer.stats.desyncs, 1);
    let t = v.host_tick();
    assert!(v.run_until(20_000, |v| v.nodes[g].peer.next_tick()
        >= t + 4 * HASH_EVERY));
    for n in &v.nodes {
        assert_eq!(n.peer.mode(), Mode::Playing, "{}: {:?}", n.name, n.log);
        assert!(!n.log.iter().any(|(_, l)| l.contains("RoundAbandoned")));
    }
}

/// A host whose game parts a second time in one round ends the round for
/// everyone, once, instead of correcting again and again; nobody rejoins.
#[test]
fn netplay_a_host_that_parts_twice_ends_the_round() {
    let (mut v, gs) = playing_round(17, 2);
    nudge_host(&mut v);
    assert!(v.run_until(20_000, |v| v.nodes[0].peer.corrections() == 1));
    let t = v.host_tick();
    assert!(v.run_until(10_000, |v| v.host_tick() >= t + 2 * HASH_EVERY));
    nudge_host(&mut v);
    assert!(
        v.run_until(20_000, |v| v
            .nodes
            .iter()
            .all(|n| n.peer.mode() == Mode::Ended)),
        "{:?}\n{:?}",
        v.nodes[0].log,
        v.nodes[gs[0]].log
    );
    for n in &v.nodes {
        let ended = n
            .log
            .iter()
            .filter(|(_, l)| l.contains("RoundAbandoned"))
            .count();
        assert_eq!(ended, 1, "{}: {:?}", n.name, n.log);
        assert_eq!(n.peer.stats.desyncs, 0, "{}: {:?}", n.name, n.log);
        assert_eq!(n.builds.len(), 1, "{}: no rejoin", n.name);
        assert!(!n.log.iter().any(|(_, l)| l.contains("DesyncAgain")));
    }
    assert_eq!(v.nodes[0].peer.corrections(), 2);
}

/// LAN §4.3/§4.5: a mid-round joiner's build is checked like a round-start
/// guest's `Loaded`: equal static digests but a different tick-0 sim is a
/// `DigestMismatch { start: true }`; an equal build raises none.
#[test]
fn netplay_a_joiner_whose_opening_state_differs_is_caught() {
    let builder: Builder = Box::new(|s: &RoundSetup, local: u8, node: usize| {
        let mut walk = fixture(s, local)?;
        if node == 2 {
            let u = walk.sim.units.iter_mut().find(|u| u.alive).unwrap();
            u.soldier.pos.x += 0.01;
        }
        Ok((walk, Some(crate::event_feed::EventFeed::default())))
    });
    let mut v = VirtualNet::new(
        12,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        builder,
    );
    v.nodes[0].peer.report_dir = None;
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let same = v.join("Same", 45, seat(1).profile.encode(), 15.0);
    v.nodes[same].peer.report_dir = None;
    assert!(v.run_until(15_000, |v| v.nodes[same].peer.mode() == Mode::Playing));
    assert!(!logged(&v, 0, "DigestMismatch"), "{:?}", v.nodes[0].log);
    let odd = v.join("Odd", 46, seat(2).profile.encode(), 15.0);
    v.nodes[odd].peer.report_dir = None;
    assert!(v.run_until(15_000, |v| logged(v, 0, "DigestMismatch")));
    let line = &v.nodes[0]
        .log
        .iter()
        .find(|(_, l)| l.contains("DigestMismatch"))
        .unwrap()
        .1;
    assert!(
        line.contains("slot: 2") && line.contains("start: true"),
        "{line}"
    );
    assert_eq!(v.nodes[0].peer.stats.digest_mismatches, 1);
}

/// A guest that cannot build the round tells the host
/// (`Peer::leave_round_for_lobby`); the host's bench releases its load
/// barrier at once instead of after `net::BARRIER_TIMEOUT`.
#[test]
fn netplay_a_guest_that_cannot_build_does_not_hold_the_barrier() {
    let builder: Builder = Box::new(|s: &RoundSetup, local: u8, node: usize| {
        anyhow::ensure!(node != 1, "no such map here");
        Ok((
            fixture(s, local)?,
            Some(crate::event_feed::EventFeed::default()),
        ))
    });
    let mut v = VirtualNet::new(
        13,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        builder,
    );
    v.nodes[0].peer.report_dir = None;
    let g = v.join("Guest", 47, seat(1).profile.encode(), 15.0);
    assert!(v.run_until(5_000, |v| logged(v, g, "Welcome")));
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| logged(v, g, "build failed")));
    let n = &mut v.nodes[g];
    n.peer
        .leave_round_for_lobby(&mut n.link, LeftWhy::LoadFailed);
    assert_eq!(v.nodes[g].peer.mode(), Mode::Idle);
    assert!(v.run_until(2_000, |v| logged(v, 0, "LeftRound")));
    assert!(logged(&v, 0, "LoadFailed"));
    // What the window's host does with it.
    Link::host(&mut v.nodes[0].link, net::HostCommand::Bench(1));
    let t = v.now;
    assert!(v.run_until(2_000, |v| v.host_tick() > 0));
    assert!(v.now.saturating_sub(t) < Micros::from_millis(2_000));
    assert_eq!(v.nodes[0].peer.mode(), Mode::Playing);
}

/// A round-start network: the host and guests with their own loading
/// times (virtual ms) and frame intervals, every guest seated.
fn loading_round(seed: u64, host_ms: u64, guests: &[(u64, f64)]) -> (VirtualNet, Vec<usize>) {
    let mut v = VirtualNet::new(
        seed,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        fixture_builder(),
    );
    v.nodes[0].peer.report_dir = None;
    let gs: Vec<usize> = guests
        .iter()
        .enumerate()
        .map(|(i, (load, frame))| {
            let g = v.join(
                &format!("G{i}"),
                70 + i as u64,
                seat(i as u8 + 1).profile.encode(),
                *frame,
            );
            v.nodes[g].peer.report_dir = None;
            v.nodes[g].load_ms = *load;
            g
        })
        .collect();
    // The first guest is far away (a VPN hop): its `Start` is late by 80 ms.
    v.sim.set_both(
        super::virtual_net::addr(0),
        v.nodes[gs[0]].addr,
        LinkParams {
            latency: Micros::from_millis(80),
            ..clean_link()
        },
    );
    assert!(v.run_until(5_000, |v| gs.iter().all(|g| logged(v, *g, "Welcome"))));
    // Their round trips settle on heartbeats.
    v.run_until(2_000, |_| false);
    let mut s = setup();
    for g in &gs {
        s.humans.push(seat(v.nodes[*g].peer.local_slot));
    }
    v.nodes[0].load_ms = host_ms;
    v.host_load(&s);
    (v, gs)
}

/// Issue #168, the first VPN playtest: every peer leaves its loading
/// screen at one moment. Four peers whose loading takes 0.6 to 4 s, one of
/// them 80 ms from the host: none
/// plays before its own loading is over or before the slowest one's, every
/// one plays from tick 0 in step, and they all begin within two frames.
#[test]
fn netplay_peers_with_different_loads_start_together() {
    let (mut v, gs) = loading_round(31, 2_500, &[(600, 15.0), (4_000, 16.7), (1_500, 7.0)]);
    let launched = v.now;
    let all = |v: &VirtualNet| v.nodes.iter().all(|n| n.first_step.is_some());
    assert!(v.run_until(10_000, all), "every peer plays");
    assert!(v.run_until(3_000, |v| v.nodes.iter().all(|n| n.peer.next_tick() >= 60)));
    let slowest = v.nodes.iter().filter_map(|n| n.loaded_at).max().unwrap();
    assert!(slowest >= launched + Micros::from_millis(4_000));
    let playing: Vec<Micros> = v.nodes.iter().map(|n| n.playing_at.unwrap()).collect();
    for n in &v.nodes {
        let at = n.playing_at.unwrap();
        assert!(
            at >= launched + Micros::from_millis(n.load_ms) && at >= slowest,
            "{} played at {at:?} (launched {launched:?}, its load {} ms, the slowest {slowest:?})",
            n.name,
            n.load_ms
        );
        assert_eq!(n.first_step.unwrap().0, 0, "{} plays from tick 0", n.name);
        assert!(n.build_error.is_none());
    }
    let spread = playing
        .iter()
        .max()
        .unwrap()
        .saturating_sub(*playing.iter().min().unwrap());
    assert!(
        spread <= Micros::from_millis(34),
        "the peers began {} ms apart: {playing:?}",
        spread.as_millis_f64()
    );
    assert_eq!(gs.len(), 3);
    assert!(v.checker.checks > 0);
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
}

/// A seated guest whose loading outlasts the load barrier: the others start
/// together at the barrier's timeout, and it enters when it is built,
/// behind (from tick 0, in step) without holding them.
#[test]
fn netplay_a_guest_stuck_loading_enters_behind_without_holding_the_start() {
    let barrier = net::BARRIER_TIMEOUT.0 / 1000;
    let (mut v, gs) = loading_round(32, 1_000, &[(800, 15.0), (barrier + 4_000, 16.7)]);
    let (ok, stuck) = (gs[0], gs[1]);
    let launched = v.now;
    assert!(
        v.run_until(barrier + 2_000, |v| v.nodes[0].first_step.is_some()
            && v.nodes[ok].first_step.is_some())
    );
    let host_at = v.nodes[0].playing_at.unwrap();
    assert!(host_at >= launched + Micros::from_millis(barrier - 50));
    let ok_at = v.nodes[ok].playing_at.unwrap();
    assert!(host_at.max(ok_at).saturating_sub(host_at.min(ok_at)) <= Micros::from_millis(34));
    assert!(v.nodes[stuck].walk.is_none(), "still loading");
    assert!(v.run_until(8_000, |v| v.nodes[stuck].first_step.is_some()));
    let n = &v.nodes[stuck];
    assert!(n.playing_at.unwrap() >= launched + Micros::from_millis(n.load_ms));
    assert_eq!(n.first_step.unwrap().0, 0);
    assert!(v.run_until(10_000, |v| v.nodes[stuck].peer.next_tick() + 10
        >= v.host_tick()));
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
}

/// The host skips the loading screen for everyone (issue #168): four
/// peers whose presentation is 4 s and whose builds alone take 0.3 to
/// 1.5 s (one 80 ms away). The host skips at 200 ms: every peer ends its
/// presentation, none plays before its own build or the slowest one's,
/// all well before the full presentation, together and from tick 0.
#[test]
fn netplay_a_host_skip_ends_every_loading_screen_together() {
    let (mut v, gs) = loading_round(33, 4_000, &[(4_000, 15.0), (4_000, 16.7), (4_000, 7.0)]);
    for (i, floor) in [300, 600, 1_500, 900].into_iter().enumerate() {
        v.nodes[i].build_floor_ms = Some(floor);
    }
    let launched = v.now;
    assert!(!v.run_until(200, |_| false));
    assert!(v.host_skip(), "the host's skip goes out");
    assert!(!v.host_skip(), "once a round");
    let all = |v: &VirtualNet| v.nodes.iter().all(|n| n.first_step.is_some());
    assert!(v.run_until(10_000, all), "every peer plays");
    let slowest = v.nodes.iter().filter_map(|n| n.loaded_at).max().unwrap();
    assert!(slowest >= launched + Micros::from_millis(1_500));
    let playing: Vec<Micros> = v.nodes.iter().map(|n| n.playing_at.unwrap()).collect();
    for n in &v.nodes {
        let at = n.playing_at.unwrap();
        let floor = Micros::from_millis(n.build_floor_ms.unwrap());
        assert!(n.peer.loading_skipped(), "{} heard the skip", n.name);
        assert!(
            at >= launched + floor && at >= slowest,
            "{} played at {at:?} (launched {launched:?}, its build {floor:?}, the slowest {slowest:?})",
            n.name
        );
        assert!(
            at < launched + Micros::from_millis(3_000),
            "{} played at {at:?}: the skip ended its 4 s presentation",
            n.name
        );
        assert_eq!(n.first_step.unwrap().0, 0, "{} plays from tick 0", n.name);
        assert!(n.build_error.is_none());
    }
    let spread = playing
        .iter()
        .max()
        .unwrap()
        .saturating_sub(*playing.iter().min().unwrap());
    assert!(
        spread <= Micros::from_millis(34),
        "the peers began {} ms apart: {playing:?}",
        spread.as_millis_f64()
    );
    assert_eq!(gs.len(), 3);
    assert!(v.run_until(3_000, |v| v.nodes.iter().all(|n| n.peer.next_tick() >= 60)));
    assert!(v.checker.checks > 0);
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
}

/// A guest cannot skip a LAN loading screen: its skip sends nothing, and
/// every peer plays its whole presentation.
#[test]
fn netplay_a_guest_cannot_skip_the_loading_screen() {
    let (mut v, gs) = loading_round(34, 1_500, &[(2_000, 15.0), (1_000, 16.7)]);
    for n in &mut v.nodes {
        n.build_floor_ms = Some(100);
    }
    let launched = v.now;
    assert!(!v.run_until(200, |_| false));
    for g in &gs {
        let n = &mut v.nodes[*g];
        assert!(!n.peer.skip_loading(&mut n.link), "{} cannot skip", n.name);
    }
    let all = |v: &VirtualNet| v.nodes.iter().all(|n| n.first_step.is_some());
    assert!(v.run_until(10_000, all), "every peer plays");
    for n in &v.nodes {
        assert!(!n.peer.loading_skipped(), "{} was not skipped", n.name);
        let at = n.playing_at.unwrap();
        assert!(
            at >= launched + Micros::from_millis(2_000),
            "{} played at {at:?}, before the slowest presentation",
            n.name
        );
        assert_eq!(n.first_step.unwrap().0, 0);
    }
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
}

/// A skip that names another round (the one before, or one not loaded) is
/// stale: no guest ends its presentation. The current round's is heard
/// (the control).
#[test]
fn netplay_a_skip_of_another_round_is_ignored() {
    let (mut v, gs) = loading_round(35, 500, &[(500, 15.0), (500, 16.7)]);
    let all = |v: &VirtualNet| v.nodes.iter().all(|n| n.first_step.is_some());
    assert!(v.run_until(5_000, all));
    let old = v.nodes[0].peer.round_id();
    // A skip after `Start` (the round already running) is ignored too.
    let started_skip = net::HostCommand::Notice {
        slot: None,
        data: super::peer::PeerNotice::SkipLoading { round_id: old }.encode(),
    };
    Link::host(&mut v.nodes[0].link, started_skip);
    assert!(!v.run_until(300, |v| gs
        .iter()
        .any(|g| v.nodes[*g].peer.loading_skipped())));
    let mut s = setup();
    for g in &gs {
        s.humans.push(seat(v.nodes[*g].peer.local_slot));
    }
    for n in &mut v.nodes {
        n.load_ms = 3_000;
        n.build_floor_ms = Some(100);
    }
    v.host_load(&s);
    let new = v.nodes[0].peer.round_id();
    assert_ne!(old, new);
    let loading = |v: &VirtualNet| gs.iter().all(|g| v.nodes[*g].peer.round_id() == new);
    assert!(v.run_until(1_000, loading), "the guests load the new round");
    // The checker compares ticks, not rounds: no peer steps the old one
    // now, so the new round's start over.
    v.checker = Default::default();
    let skip = |round_id| net::HostCommand::Notice {
        slot: None,
        data: super::peer::PeerNotice::SkipLoading { round_id }.encode(),
    };
    for round_id in [old, new.wrapping_add(1), 0] {
        Link::host(&mut v.nodes[0].link, skip(round_id));
    }
    assert!(!v.run_until(300, |v| gs
        .iter()
        .any(|g| v.nodes[*g].peer.loading_skipped())));
    for g in &gs {
        assert!(v.nodes[*g].loading(), "still in its presentation");
    }
    // The control: this round's skip ends theirs.
    Link::host(&mut v.nodes[0].link, skip(new));
    assert!(v.run_until(300, |v| gs
        .iter()
        .all(|g| v.nodes[*g].peer.loading_skipped())));
    assert!(v.run_until(5_000, all));
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
}

/// One peer of [`netplay_udp_peers_with_different_loads_start_together`].
struct UdpPeer<L> {
    peer: Peer,
    link: L,
    load: std::time::Duration,
    due: Option<(std::time::Instant, RoundSetup)>,
    walk: Option<crate::walk::WalkSim>,
    loaded: Option<std::time::Instant>,
    playing: Option<std::time::Instant>,
    first: Option<u32>,
    last: std::time::Instant,
}

impl<L: Link> UdpPeer<L> {
    fn new(peer: Peer, link: L, load_ms: u64) -> Self {
        Self {
            peer,
            link,
            load: std::time::Duration::from_millis(load_ms),
            due: None,
            walk: None,
            loaded: None,
            playing: None,
            first: None,
            last: std::time::Instant::now(),
        }
    }

    /// Its loading screen: the build `load` after the `LoadRound`.
    fn load(&mut self, setup: RoundSetup) {
        self.due = Some((std::time::Instant::now() + self.load, setup));
        self.walk = None;
    }

    /// One frame on the real clock.
    fn frame(&mut self) {
        let now = std::time::Instant::now();
        for n in self.peer.poll(&mut self.link) {
            if let Notice::Load { setup, .. } = n {
                self.load(*setup);
            }
        }
        if self.due.as_ref().is_some_and(|(at, _)| now >= *at) {
            let (_, setup) = self.due.take().unwrap();
            let mut walk = fixture(&setup, self.peer.local_slot).expect("fixture");
            self.peer.round_built(&mut self.link, &mut walk);
            self.loaded = Some(now);
            self.walk = Some(walk);
        }
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        if let Some(walk) = self.walk.as_mut() {
            let pad = super::virtual_net::scripted_pad(self.peer.local_slot, walk.sim.tick);
            let first = &mut self.first;
            let report = self
                .peer
                .frame(&mut self.link, dt, pad, walk, None, |_, b, _| {
                    first.get_or_insert(b.tick);
                });
            if report.mode == Mode::Playing {
                self.playing.get_or_insert(now);
            }
        }
    }
}

/// Issue #168 over real UDP on this machine (`net::NetSession`, its own
/// threads and the real clock): a host and two guests whose loading takes
/// 0.3, 1.2 and 0.6 s. None plays before the slowest is built, every one
/// plays from tick 0 without a desync, and they all begin within a few
/// frames of each other.
#[test]
fn netplay_udp_peers_with_different_loads_start_together() {
    use net::{ClientConfig, HostConfig, NetSession, Transport, UdpTransport};
    use std::time::{Duration, Instant};
    let host_t = UdpTransport::bind("127.0.0.1:0").expect("bind");
    let at = host_t.local_addr().unwrap();
    let mut config = HostConfig::new("test-build", "Host");
    config.profile = seat(0).profile.encode();
    let mut host = UdpPeer::new(Peer::host(), NetSession::host(config, host_t), 300);
    let mut guests: Vec<_> = [1_200u64, 600]
        .iter()
        .enumerate()
        .map(|(i, load)| {
            let mut c = ClientConfig::new("test-build", format!("G{i}"), 80 + i as u64);
            c.profile = seat(i as u8 + 1).profile.encode();
            let t = UdpTransport::bind("127.0.0.1:0").expect("bind");
            UdpPeer::new(Peer::client(), NetSession::join(c, at, t), *load)
        })
        .collect();
    type Host = UdpPeer<NetSession<net::HostCore>>;
    type Guest = UdpPeer<NetSession<net::ClientCore>>;
    fn frames(
        host: &mut Host,
        guests: &mut [Guest],
        until: &dyn Fn(&Host, &[Guest]) -> bool,
        secs: u64,
    ) {
        let t = Instant::now();
        while !until(host, guests) {
            assert!(
                t.elapsed() < Duration::from_secs(secs),
                "not reached in {secs} s"
            );
            host.frame();
            for g in guests.iter_mut() {
                g.frame();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    frames(
        &mut host,
        &mut guests,
        &|h, gs| h.peer.guests().len() == 2 && gs.iter().all(|g| g.peer.local_slot != 0),
        5,
    );
    // Their round trips settle on heartbeats.
    let settle = Instant::now() + Duration::from_millis(600);
    frames(&mut host, &mut guests, &|_, _| Instant::now() >= settle, 5);
    let mut setup = setup();
    for g in &guests {
        setup.humans.push(seat(g.peer.local_slot));
    }
    let bytes = host.peer.host_load(&mut host.link, &setup);
    host.load(RoundSetup::decode(&bytes).unwrap());
    let launched = Instant::now();
    frames(
        &mut host,
        &mut guests,
        &|h, gs| h.peer.next_tick() >= 60 && gs.iter().all(|g| g.peer.next_tick() >= 60),
        15,
    );
    let loaded = std::iter::once(host.loaded)
        .chain(guests.iter().map(|g| g.loaded))
        .map(Option::unwrap)
        .max()
        .unwrap();
    assert!(loaded >= launched + Duration::from_millis(1_200));
    let playing: Vec<Instant> = std::iter::once(host.playing)
        .chain(guests.iter().map(|g| g.playing))
        .map(Option::unwrap)
        .collect();
    assert!(
        playing.iter().all(|p| *p >= loaded),
        "nobody began before the slowest was built"
    );
    let spread = *playing.iter().max().unwrap() - *playing.iter().min().unwrap();
    println!("udp start spread {:.1} ms", spread.as_secs_f64() * 1e3);
    assert!(
        spread <= Duration::from_millis(50),
        "the peers began {:.1} ms apart",
        spread.as_secs_f64() * 1e3
    );
    assert_eq!(host.first, Some(0));
    for g in &guests {
        assert_eq!(g.first, Some(0));
        assert_eq!(
            (g.peer.stats.desyncs, g.peer.stats.digest_mismatches),
            (0, 0)
        );
    }
    assert_eq!(host.peer.stats.digest_mismatches, 0);
    host.link.shutdown();
    for g in guests {
        g.link.shutdown();
    }
}

/// The fourth LAN playtest: a charge weapon's charge on the HUD ring, on
/// the host and on a guest, with prediction and drawing on as the window
/// runs them. Every class's weapon is made a 2.5 s charge weapon (the BF2
/// arc caster's MaxPressedTime) on every peer alike; the scripted pads hold
/// FIRE for 1.5 s at a time. Each peer's HUD gauge for its own soldier is
/// read from its confirmed sim after each frame. The window builds the HUD
/// model between `draw_in` and `draw_out`, but the predicted pose it swaps in
/// writes only the pose (`UnitPose::write_to`), never the weapon, so that is
/// the state the HUD reads. A guest's charge starts a round trip after its
/// press, as every input of its own does.
#[test]
fn netplay_a_held_charge_fills_each_peers_own_ring() {
    let builder: Builder = Box::new(|s: &RoundSetup, local: u8, _node: usize| {
        // The fixture with a command post per side, so both humans spawn.
        let mut walk = super::smooth_tests::fixture(s, local)?;
        for c in &mut walk.sim.weapon_classes {
            c.max_pressed_time = 2.5;
        }
        Ok((walk, Some(crate::event_feed::EventFeed::default())))
    });
    let mut v = VirtualNet::new(
        11,
        clean_link(),
        "test-build",
        seat(0).profile.encode(),
        builder,
    );
    v.predict = true;
    v.draw = true;
    v.nodes[0].peer.report_dir = None;
    v.host_load(&setup());
    assert!(v.run_until(5_000, |v| v.host_tick() >= 30));
    let g = v.join("G", 61, seat(1).profile.encode(), 16.7);
    v.nodes[g].peer.report_dir = None;
    assert!(v.run_until(20_000, |v| v.nodes[g].peer.mode() == Mode::Playing));
    let mut peak = [0.0f32; 2];
    let mut held_frames = [0u32; 2];
    v.run_until(20_000, |v| {
        for (k, n) in [0, g].into_iter().enumerate() {
            let node = &v.nodes[n];
            let Some(walk) = node.walk.as_ref() else {
                continue;
            };
            let me = &walk.sim.units[walk.player.index as usize];
            if me.alive && me.weapon.fire_cmd > 0 {
                held_frames[k] += 1;
            }
            let gauge = crate::hud_feed::weapon_gauge(walk);
            peak[k] = peak[k].max(gauge.charge.unwrap_or(0.0));
        }
        false
    });
    println!("held frames {held_frames:?}, peak charge {peak:?}");
    assert!(held_frames.iter().all(|h| *h > 0), "{held_frames:?}");
    assert_eq!(v.checker.mismatch_count, 0, "{:?}", v.checker.mismatches);
    assert!(peak[0] > 0.3, "the host's ring: {peak:?}");
    assert!(peak[1] > 0.3, "the guest's ring: {peak:?}");
}

// The build id's rules (game/build.rs), with their own tests.
#[path = "../../build_rules.rs"]
mod build_rules;
