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
    // An older setup without the field reads as On.
    let mut json: serde_json::Value = serde_json::from_slice(&setup().encode()).unwrap();
    json.as_object_mut().unwrap().remove("awards");
    assert!(
        RoundSetup::decode(&serde_json::to_vec(&json).unwrap())
            .unwrap()
            .awards
    );
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

// The build id's rules (game/build.rs), with their own tests.
#[path = "../../build_rules.rs"]
mod build_rules;
