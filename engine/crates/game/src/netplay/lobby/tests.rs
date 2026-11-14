//! The lobby's data (issue #168).

use super::*;
use ui::shell::{IaTexts, SessionOptions, SidePick};

/// A catalog with `baked` maps baked, each with conquest missions for both
/// eras.
fn catalog(baked: &[&str]) -> Vec<MapEntry> {
    let bakes: Vec<(String, String)> = baked
        .iter()
        .map(|k| ((*k).to_owned(), k[..3].to_owned()))
        .collect();
    let missions = |_: &str, mode: Mode| -> Vec<[String; 2]> {
        if mode == Mode::Con {
            vec![["cis".into(), "rep".into()], ["imp".into(), "all".into()]]
        } else {
            Vec::new()
        }
    };
    ui::shell::catalog(&|_, _| None, &bakes, &missions)
}

fn ia(maps: Vec<MapEntry>) -> InstantAction {
    let options = SessionOptions {
        dlc_heroes_installed: true,
        dlc_heroes: true,
        ..Default::default()
    };
    InstantAction::new(maps, IaTexts::new(&|_, e| e.to_owned()), options)
}

fn index(ia: &InstantAction, key: &str) -> usize {
    ia.maps.iter().position(|m| m.key == key).unwrap()
}

fn manifest(keys: &[&str], addon: bool) -> ContentManifest {
    ContentManifest {
        maps: keys.iter().map(|k| (*k).to_owned()).collect(),
        games: ["bf2".to_owned()].into(),
        addon_heroes: addon,
    }
}

fn add(ia: &mut InstantAction, key: &str) {
    let i = index(ia, key);
    let before = ia.playlist.len();
    ia.add(
        MapPick::One(i),
        ModePick::One(Mode::Con),
        SidePick::One(Side::Era(ERAS[0])),
    );
    assert!(ia.playlist.len() > before, "{key} adds a row");
}

/// The host's screen round-trips through JSON, and a guest whose own
/// catalog lists the maps in another order (and has one more) shows the same
/// maps, playlist, picks and cursors, mapped by key.
#[test]
fn lobby_state_round_trips_and_maps_the_playlist_by_key() {
    let all = ["bf2_cor1", "bf2_kas2", "bf2_nab2"];
    let full = catalog(&all);
    let mut host = ia(full.clone());
    restrict(&mut host, &full, &manifest(&all[..], true), true);
    add(&mut host, "bf2_nab2");
    add(&mut host, "bf2_cor1");
    host.picked_map = MapPick::One(index(&host, "bf2_kas2"));
    host.state = State::Playlist;
    host.play_list.at = 2;
    host.page = Page::Overview(1);
    host.options.con_mult = 250;
    let mut s = LobbyState::from_ia(&host);
    s.phase = LobbyPhase::Results { ready: vec![0, 2] };
    s.cheat_policy = 0b11;
    s.roster.push(LobbyPlayer {
        slot: 2,
        name: "Ann".into(),
        ping_ms: Some(31),
        addon: false,
        team: None,
    });
    let back = LobbyState::decode(&s.encode()).expect("decodes");
    assert_eq!(back, s);
    assert_eq!(
        s.playlist
            .iter()
            .map(|e| e.map.as_str())
            .collect::<Vec<_>>(),
        ["bf2_nab2", "bf2_cor1"]
    );
    // The guest's catalog: another order and a map the host does not offer.
    let mut theirs = catalog(&["bf2_nab2", "bf2_hot1", "bf2_cor1", "bf2_kas2"]);
    theirs.reverse();
    let mut guest = ia(theirs.clone());
    assert!(back.apply_to(&mut guest, &theirs).is_empty());
    let keys = |ia: &InstantAction| ia.maps.iter().map(|m| m.key.clone()).collect::<Vec<_>>();
    assert_eq!(keys(&guest), keys(&host));
    assert_eq!(playlist_keys(&guest), playlist_keys(&host));
    assert_eq!(guest.picked_map, host.picked_map);
    assert_eq!(
        (guest.state, guest.play_list, guest.page),
        (State::Playlist, host.play_list, Page::Overview(1))
    );
    assert_eq!(guest.options.con_mult, 250);
    // A guest missing a playlisted map drops its row and names the key.
    let short = catalog(&["bf2_kas2", "bf2_nab2"]);
    let mut g2 = ia(short.clone());
    assert_eq!(back.apply_to(&mut g2, &short), ["bf2_cor1"]);
    assert_eq!(
        playlist_keys(&g2)
            .iter()
            .map(|e| e.map.as_str())
            .collect::<Vec<_>>(),
        ["bf2_nab2"]
    );
}

/// The host's list is the intersection: a map a new player lacks leaves the
/// list and the playlist with a lobby line; the addon heroes go off and
/// grey unless everyone has them; an unchanged intersection changes nothing.
#[test]
fn restrict_keeps_the_intersection_and_says_what_went() {
    let all = ["bf2_cor1", "bf2_kas2", "bf2_nab2"];
    let full = catalog(&all);
    let mut host = ia(full.clone());
    assert!(restrict(&mut host, &full, &manifest(&all, true), true).is_empty());
    assert_eq!(host.maps.len(), 3, "only baked maps everyone has");
    add(&mut host, "bf2_kas2");
    add(&mut host, "bf2_nab2");
    assert!(host.options.dlc_heroes);
    let lines = restrict(
        &mut host,
        &full,
        &manifest(&["bf2_cor1", "bf2_nab2"], false),
        true,
    );
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("removed from the playlist"), "{lines:?}");
    assert!(lines[1].contains("addon heroes"), "{lines:?}");
    assert_eq!(
        host.maps.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(),
        ["bf2_cor1", "bf2_nab2"].map(|k| full.iter().find(|m| m.key == k).unwrap().key.as_str())
    );
    assert_eq!(
        playlist_keys(&host)
            .iter()
            .map(|e| e.map.as_str())
            .collect::<Vec<_>>(),
        ["bf2_nab2"]
    );
    assert!(!host.options.dlc_heroes && !host.options.dlc_heroes_installed);
    // The playlist's indices point at the new list.
    assert_eq!(host.maps[host.playlist[0].map].key, "bf2_nab2");
    assert!(restrict(
        &mut host,
        &full,
        &manifest(&["bf2_cor1", "bf2_nab2"], false),
        true
    )
    .is_empty());
    // Everyone has the addon again: offered (greyed no more), still off.
    restrict(
        &mut host,
        &full,
        &manifest(&["bf2_cor1", "bf2_nab2"], true),
        true,
    );
    assert!(host.options.dlc_heroes_installed && !host.options.dlc_heroes);
}

/// Every refusal reads as plain text; the build refusal names both builds.
#[test]
fn reject_reasons_read_as_plain_text() {
    use net::RejectReason as R;
    assert_eq!(
        reject_text(R::Version, "build abc123, host def456+9f", "abc123"),
        "The host runs a different openbf build (host: def456+9f, you: abc123)"
    );
    assert_eq!(
        reject_text(
            R::Version,
            &format!("protocol {}, host 2", net::PROTOCOL),
            "x"
        ),
        format!(
            "The host runs a different openbf build (host: protocol 2, you: protocol {})",
            net::PROTOCOL
        )
    );
    assert!(reject_text(R::Version, "", "x").contains("host: unknown, you: x"));
    assert_eq!(reject_text(R::Full, "", "x"), "The game is full");
    assert!(reject_text(R::Busy, "", "x").contains("loading"));
    assert!(reject_text(R::Banned, "", "x").contains("removed"));
    assert!(reject_text(R::BadName, "", "x").contains("player name"));
    assert_eq!(
        reject_text(R::Kicked, net::host::FELL_BEHIND_TEXT, "x"),
        net::host::FELL_BEHIND_TEXT
    );
    assert!(reject_text(R::Kicked, "", "x").contains("removed"));
}

/// Recent hosts: newest first, once each, at most eight; the name falls
/// back when the profile's is not a valid one.
#[test]
fn recent_hosts_and_the_player_name() {
    let mut r = Vec::new();
    for i in 0..10 {
        remember_host(&mut r, &format!("10.0.0.{i}"));
    }
    remember_host(&mut r, " 10.0.0.5 ");
    remember_host(&mut r, "");
    assert_eq!(r.len(), RECENT_HOSTS);
    assert_eq!(r[0], "10.0.0.5");
    assert_eq!(r[1], "10.0.0.9");
    assert_eq!(r.iter().filter(|a| *a == "10.0.0.5").count(), 1);
    assert_eq!(player_name("  Ann "), "Ann");
    assert_eq!(player_name(&"x".repeat(40)).chars().count(), 24);
    let fallback = player_name("   ");
    assert!(net::msg::valid_name(&fallback), "{fallback:?}");
}

/// The mismatch text names the first group whose digest differs.
#[test]
fn digest_mismatch_names_the_first_differing_group() {
    let host = vec![
        ("classes".to_owned(), 1),
        ("collision".to_owned(), 2),
        ("nav".to_owned(), 3),
    ];
    let ours = [("classes", 1), ("collision", 9), ("nav", 4)];
    assert_eq!(
        digest_mismatch_text("Kashyyyk", &host, &ours),
        "Your copy of Kashyyyk plays differently from the host's (collision)"
    );
    let info = HostInfo {
        name: "Ann".into(),
        players: 8,
        max_players: 8,
        protocol: net::PROTOCOL,
        build: "b".into(),
        ..Default::default()
    };
    assert_eq!(HostInfo::decode(&info.encode()), Some(info.clone()));
    assert_eq!(info.refusal("b").as_deref(), Some("full"));
    assert_eq!(info.refusal("c").as_deref(), Some("another openbf build"));
    let n = GameNotice::DigestMismatch {
        map: "Kashyyyk".into(),
        groups: host,
    };
    assert_eq!(GameNotice::decode(&n.encode()), Some(n));
}

/// A row the roster cannot all play now is skipped at its turn (a guest
/// without it joined after Launch), with a line; the addon heroes go off
/// for the rest of the playlist; the queue's end is `None`.
#[test]
fn next_playable_row_skips_what_a_later_joiner_lacks() {
    let row = |key: &str| ui::shell::Launch {
        key: key.into(),
        game: ui::shell::Game::Bf2,
        mode: Mode::Con,
        armies: ["rep".into(), "cis".into()],
        label: key.to_uppercase(),
    };
    let mut playlist: std::collections::VecDeque<_> =
        [row("bf2_kas2"), row("bf2_nab2"), row("bf2_cor1")].into();
    let mut session = SessionOptions {
        dlc_heroes_installed: true,
        dlc_heroes: true,
        ..Default::default()
    };
    let everyone = manifest(&["bf2_kas2", "bf2_nab2", "bf2_cor1"], true);
    let mut lines = Vec::new();
    let first = next_playable_row(&mut playlist, &mut session, &everyone, true, &mut lines);
    assert_eq!(first.map(|r| r.key).as_deref(), Some("bf2_kas2"));
    assert!(lines.is_empty() && session.dlc_heroes, "{lines:?}");
    // A guest with only bf2_cor1 and no addon joined during the first row.
    let common = ContentManifest::intersect([&everyone, &manifest(&["bf2_cor1"], false)]);
    let next = next_playable_row(&mut playlist, &mut session, &common, true, &mut lines);
    assert_eq!(next.map(|r| r.key).as_deref(), Some("bf2_cor1"));
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("addon heroes"), "{lines:?}");
    assert!(lines[1].starts_with("BF2_NAB2 was removed"), "{lines:?}");
    assert!(!session.dlc_heroes && !session.dlc_heroes_installed);
    lines.clear();
    assert!(next_playable_row(&mut playlist, &mut session, &common, true, &mut lines).is_none());
    assert!(lines.is_empty());
}

/// One token for the whole process: every join reuses it (a rejoin
/// reclaims its slot and name).
#[test]
fn the_session_token_is_one_per_process() {
    assert_eq!(session_token(), session_token());
    assert_ne!(session_token(), 0);
}
