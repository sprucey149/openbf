//! The LAN pages' models (issue #168).

use super::*;

fn press(f: impl Fn(&mut MenuInput)) -> MenuInput {
    let mut i = MenuInput::default();
    f(&mut i);
    i
}

const CHEATS: [(&str, &str); 3] = [
    ("invincible", "Invincibility"),
    ("ammo", "Infinite ammo"),
    ("stamina", "Infinite stamina"),
];

/// Host Game's defaults (8 players, the default port, cheats Off, lag
/// compensation on); max players holds to 2..=16; the per-cheat rows show
/// only under the Allowed policy and make its bits.
#[test]
fn host_setup_defaults_ranges_and_cheat_policy() {
    let mut h = HostSetup::new(&CHEATS, 37450);
    let labels: Vec<String> = h.rows().into_iter().map(|r| r.label).collect();
    assert_eq!(
        labels,
        [
            "Max players",
            "UDP port",
            "Cheats for guests",
            "Awards",
            "Start Hosting",
            "Back"
        ]
    );
    assert_eq!(
        (h.max_players, h.port.as_str(), h.lag_comp),
        (8, "37450", true)
    );
    assert_eq!(h.cheat_policy(), 0);
    for _ in 0..20 {
        h.step(press(|i| i.right = true));
    }
    assert_eq!(h.max_players, 16);
    assert!(!h.focus_text);
    h.step(press(|i| i.down = true));
    assert!(h.focus_text, "arriving on the port row focuses its box");
    h.focus_text = false;
    h.step(press(|i| i.up = true));
    assert!(!h.focus_text);
    for _ in 0..20 {
        h.step(press(|i| i.left = true));
    }
    assert_eq!(h.max_players, 2);
    // Cheats for guests: Allowed adds one row per cheat, all allowed.
    h.cursor = 2;
    h.step(press(|i| i.confirm = true));
    assert!(h.cheats_allowed);
    assert_eq!(h.rows().len(), 6 + CHEATS.len());
    assert_eq!(h.rows()[4].label, "   Infinite ammo");
    assert_eq!(h.cheat_policy(), 0b111);
    h.step(press(|i| i.down = true));
    h.step(press(|i| i.down = true));
    h.step(press(|i| i.confirm = true));
    assert_eq!(h.cheat_policy(), 0b101, "ammo switched off");
    assert_eq!(h.rows()[4].value.as_deref(), Some("Off"));
    let clicks = h.take_clicks();
    assert!(clicks.contains(&MenuSound::Select) && clicks.contains(&MenuSound::Accept));
}

/// Start Hosting checks the port; Back (row or key) leaves.
#[test]
fn host_setup_start_checks_the_port() {
    let mut h = HostSetup::new(&CHEATS, 37450);
    let start = h
        .row_ids()
        .iter()
        .position(|r| *r == HostRow::Start)
        .unwrap();
    h.cursor = 1;
    *h.text_mut().expect("the port row is a text box") = "70000".into();
    h.cursor = start;
    assert!(h.text_mut().is_none());
    assert_eq!(h.step(press(|i| i.confirm = true)), None);
    assert!(h.message.as_deref().unwrap().contains("1 to 65535"));
    assert_eq!(h.take_clicks(), [MenuSound::Error]);
    h.port = "40000".into();
    assert!(
        !h.rows().iter().any(|r| r.label.contains("Lag")),
        "lag compensation is always on and not offered"
    );
    assert!(h.awards, "awards default On, as in single player");
    h.cursor = start - 1;
    assert_eq!(h.rows()[h.cursor].label, "Awards");
    h.step(press(|i| i.right = true));
    assert!(!h.awards);
    h.cursor = start;
    assert_eq!(
        h.step(press(|i| i.confirm = true)),
        Some(HostCommand::Start {
            max_players: 8,
            port: 40000,
            cheat_policy: 0,
            lag_comp: true,
            awards: false
        })
    );
    assert_eq!(h.step(press(|i| i.back = true)), Some(HostCommand::Back));
}

fn found(addr: &str, refusal: Option<&str>) -> FoundRow {
    FoundRow {
        addr: addr.into(),
        name: "Host".into(),
        map: "Kashyyyk".into(),
        players: "2/8".into(),
        ping_ms: 3,
        refusal: refusal.map(Into::into),
    }
}

/// Join Game: the found hosts, the address box and the recent hosts; a
/// host on another build is dimmed and skipped; Connect carries the
/// address; an empty box is refused with the hint.
#[test]
fn join_page_rows_connect_and_refusals() {
    let mut j = JoinPage::new(vec!["10.0.0.9:37450".into()]);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Address);
    assert!(j.focus_text, "the address box has the keyboard at once");
    assert_eq!(j.step(press(|i| i.confirm = true)), None);
    assert!(j.status.as_deref().unwrap().contains("address"));
    *j.text_mut().unwrap() = " studio-pc:40000 ".into();
    assert_eq!(
        j.step(press(|i| i.confirm = true)),
        Some(JoinCommand::Connect("studio-pc:40000".into()))
    );
    j.set_hosts(vec![
        found("192.168.1.5:37450", None),
        found("192.168.1.6:37450", Some("another openbf build")),
    ]);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Address, "focus kept");
    let rows = j.rows();
    assert!(rows[0].enabled && !rows[1].enabled);
    assert!(rows[1].label.ends_with("(another openbf build)"));
    j.step(press(|i| i.up = true));
    assert_eq!(
        j.row_ids()[j.cursor],
        JoinRow::Host(0),
        "the dimmed host is skipped"
    );
    // A host appearing before it keeps the focus on the same address.
    j.set_hosts(vec![
        found("192.168.1.2:37450", None),
        found("192.168.1.5:37450", None),
    ]);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Host(1));
    assert_eq!(
        j.step(press(|i| i.confirm = true)),
        Some(JoinCommand::Connect("192.168.1.5:37450".into()))
    );
    j.step(press(|i| i.down = true));
    j.step(press(|i| i.down = true));
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Test);
    assert_eq!(j.rows()[j.cursor].label, "Test connection");
    assert_eq!(
        j.step(press(|i| i.confirm = true)),
        Some(JoinCommand::Test("studio-pc:40000".into())),
        "Test connection asks the typed address"
    );
    j.step(press(|i| i.down = true));
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Recent(0));
    assert_eq!(
        j.step(press(|i| i.confirm = true)),
        Some(JoinCommand::Connect("10.0.0.9:37450".into()))
    );
    assert_eq!(j.step(press(|i| i.back = true)), Some(JoinCommand::Back));
}
