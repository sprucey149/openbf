//! The LAN pages' models and headless Host geometry (issues #168, #181).

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

/// Inspect egui's painted glyph geometry, including its actual clipping rect.
#[test]
fn host_layout_keeps_heading_addresses_and_selected_rows_in_bounds() {
    for size in [
        [1920.0, 1080.0],
        [1280.0, 720.0],
        [800.0, 600.0],
        [640.0, 480.0],
    ] {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size.into());
        let safe = screen.shrink(size[1] * 0.05);
        let mut h = HostSetup::new(&CHEATS, 37450);
        h.cheats = [
            "invincible",
            "ammo",
            "clip",
            "stamina",
            "classes",
            "awards",
            "damage",
            "guardian",
            "endurance",
            "technician",
        ]
        .into_iter()
        .map(|id| (id, crate::menu::cheat_text(id).unwrap().0, true))
        .collect();
        let lines = vec![
            (
                "LAN Address: 192.168.255.254:65535".into(),
                Color32::LIGHT_BLUE,
            ),
            (
                "VPN Address: 100.127.255.254:65535".into(),
                Color32::LIGHT_BLUE,
            ),
        ];
        for expanded in [false, true] {
            h.cheats_allowed = expanded;
            for cursor in 0..h.rows().len() {
                h.cursor = cursor;
                let rows = h.rows();
                let mut output = egui::FullOutput::default();
                for _ in 0..4 {
                    output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(screen),
                            ..Default::default()
                        },
                        |ctx| {
                            draw_host_page(ctx, &rows, cursor, h.text_mut(), false, &lines);
                        },
                    );
                }
                for want in [
                    "LAN: Host Game",
                    lines[0].0.as_str(),
                    lines[1].0.as_str(),
                    rows[cursor].label.trim(),
                ] {
                    let shape = output
                        .shapes
                        .iter()
                        .find_map(|c| match &c.shape {
                            egui::Shape::Text(t) if t.galley.text().trim() == want => {
                                Some((c.clip_rect, t))
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing {want}"));
                    let bounds = shape.1.visual_bounding_rect();
                    assert!(
                        safe.contains_rect(bounds),
                        "{size:?} {want}: {bounds:?}, safe {safe:?}"
                    );
                    assert!(
                        shape.0.contains_rect(bounds),
                        "{size:?} {want}: clipped {bounds:?} by {:?}",
                        shape.0
                    );
                }
            }
        }
    }
}

fn host_frame(
    ctx: &egui::Context,
    h: &mut HostSetup,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, PageDraw) {
    let rows = h.rows();
    let cursor = h.cursor;
    let focus = std::mem::take(&mut h.focus_text);
    let mut draw = PageDraw::default();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(640.0, 480.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            draw = draw_host_page(ctx, &rows, cursor, h.text_mut(), focus, &[]);
        },
    );
    (output, draw)
}

#[test]
fn host_layout_port_focus_editing_submission_and_mouse_access() {
    let ctx = egui::Context::default();
    let mut h = HostSetup::new(&CHEATS, 37450);
    h.cheats_allowed = true;
    for _ in 0..4 {
        host_frame(&ctx, &mut h, vec![]);
    }
    let (output, _) = host_frame(&ctx, &mut h, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "UDP Port" => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
        .unwrap();
    host_frame(
        &ctx,
        &mut h,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    let (_, draw) = host_frame(
        &ctx,
        &mut h,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(
        draw.clicked,
        Some(1),
        "an unfocused port row can be selected with the mouse"
    );
    // The same menu edge used by both keys and the pad arrives on Port.
    h.step(press(|i| i.down = true));
    let (_, draw) = host_frame(&ctx, &mut h, vec![]);
    assert!(draw.text_focus);
    h.port.clear();
    let (_, draw) = host_frame(&ctx, &mut h, vec![egui::Event::Text("65535".into())]);
    assert_eq!(h.port, "65535");
    assert!(draw.text_focus);
    let (_, draw) = host_frame(
        &ctx,
        &mut h,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(draw.submitted && !draw.text_focus);
    for _ in 0..2 {
        host_frame(&ctx, &mut h, vec![]);
    }
    h.step(press(|i| i.up = true));
    h.step(press(|i| i.down = true));
    assert!(host_frame(&ctx, &mut h, vec![]).1.text_focus);
    let (_, draw) = host_frame(
        &ctx,
        &mut h,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        !draw.text_focus && !draw.submitted,
        "Escape returns the menu's input"
    );
}

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
            "Max Players",
            "UDP Port",
            "Cheats for Guests",
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
    assert_eq!(h.rows()[4].label, "   Infinite Ammo");
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
    assert_eq!(j.rows()[j.cursor].label, "Test Connection");
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

fn roster() -> Vec<RosterRow> {
    let row = |slot: u8, name: &str| RosterRow {
        slot,
        name: name.into(),
        ping_ms: (slot != 0).then_some(40),
        addon: false,
        team: "Auto".into(),
        ready: false,
    };
    vec![row(0, "the maintainer"), row(1, "Kai"), row(2, "Sam")]
}

/// Issue #168, the first VPN playtest: the host's player list with a pad.
/// Square opens it (and moves into it), Up / Down move and wrap, Accept on
/// the host's own row does nothing, Accept on a guest asks and a second
/// Accept kicks him; Back cancels the question, then closes the list, as
/// Square does; a closed list opens again with Square.
#[test]
fn roster_list_opens_moves_kicks_and_closes_with_a_pad() {
    let rows = roster();
    let mut nav = RosterNav::default();
    let none = MenuInput::default();
    assert!(!nav.active() && !nav.hidden, "shown, not holding the input");
    // Moves do not reach a list the pad is not in.
    assert_eq!(
        nav.input(false, press(|i| i.down = true), &rows, true),
        None
    );
    assert_eq!(nav.focus, None);
    // Square: into the list, on the first row.
    assert_eq!(nav.input(true, none, &rows, true), None);
    assert!(nav.active());
    assert_eq!(nav.focus, Some(0));
    // Accept on the host's own row: nothing.
    assert_eq!(
        nav.input(false, press(|i| i.confirm = true), &rows, true),
        None
    );
    assert_eq!(nav.confirm, None);
    // Up wraps to the last guest; Down back to the first row, then Kai.
    nav.input(false, press(|i| i.up = true), &rows, true);
    assert_eq!(nav.focus, Some(2));
    nav.input(false, press(|i| i.down = true), &rows, true);
    nav.input(false, press(|i| i.down = true), &rows, true);
    assert_eq!(nav.focus, Some(1));
    // Accept asks; Back cancels the question and stays in the list.
    let accept = press(|i| i.confirm = true);
    assert_eq!(nav.input(false, accept, &rows, true), None);
    assert_eq!(nav.confirm, Some(1), "asks for Kai");
    nav.input(false, press(|i| i.back = true), &rows, true);
    assert!(nav.active() && nav.confirm.is_none());
    // Accept twice: Kai is kicked.
    assert_eq!(nav.input(false, accept, &rows, true), None);
    assert_eq!(nav.input(false, accept, &rows, true), Some(1));
    // The question belongs to the guest it was asked for: Kai leaves while
    // asked, Sam is now the row under the pad, and Accept only asks again.
    assert_eq!(nav.input(false, accept, &rows, true), None, "asks for Kai");
    let kai_gone = vec![rows[0].clone(), rows[2].clone()];
    assert_eq!(
        nav.input(false, accept, &kai_gone, true),
        None,
        "Sam is not kicked unasked"
    );
    assert_eq!(
        RosterPrompts::default().line(&nav, &kai_gone, true),
        "Kick Sam? : kick \u{b7} : cancel",
        "the question is Sam's now"
    );
    assert_eq!(nav.input(false, accept, &kai_gone, true), Some(2));
    // Sam leaves while asked, the pad falls back to the maintainer: no question.
    nav.input(false, accept, &kai_gone, true);
    nav.input(false, none, &kai_gone[..1], true);
    assert_eq!(nav.confirm, None, "disarmed on the maintainer");
    assert_eq!(
        RosterPrompts::default().line(&nav, &kai_gone[..1], true),
        "Up / Down: move \u{b7} : close"
    );
    // A guest's list never kicks.
    let mut guest = RosterNav {
        focus: Some(1),
        ..RosterNav::default()
    };
    assert_eq!(guest.input(false, accept, &rows, false), None);
    assert_eq!(guest.input(false, accept, &rows, false), None);
    // Back closes the list and gives the input back; Square opens it again.
    nav.input(false, press(|i| i.back = true), &rows, true);
    assert!(!nav.active() && nav.hidden);
    nav.input(true, none, &rows, true);
    assert!(nav.active() && !nav.hidden);
    // Square closes it as well.
    nav.input(true, none, &rows, true);
    assert!(!nav.active() && nav.hidden);
    // A list whose rows went away lets go of the input.
    let mut nav = RosterNav {
        focus: Some(2),
        ..RosterNav::default()
    };
    nav.input(false, none, &rows[..1], true);
    assert_eq!(nav.focus, Some(0), "held to the last row");
    nav.input(false, none, &[], true);
    assert!(!nav.active());
}

/// The roster's help line names the set's own buttons: Square / X / Space
/// to open, Accept, and the lobby's Back (Circle / B / Backspace).
#[test]
fn roster_prompts_name_the_buttons_in_use() {
    use crate::input_icons::{IconSet, Icons};
    let rows = roster();
    let ds = RosterPrompts::of(&Icons::of(IconSet::Ds4));
    let xbox = RosterPrompts::of(&Icons::of(IconSet::XboxOne));
    let keys = RosterPrompts::of(&Icons::of(IconSet::KeyboardMouse));
    let shut = RosterNav::default();
    assert_eq!(ds.line(&shut, &rows, true), "Square: players list");
    assert_eq!(xbox.line(&shut, &rows, true), "X: players list");
    assert_eq!(keys.line(&shut, &rows, true), "Space: players list");
    let on_kai = RosterNav {
        focus: Some(1),
        ..RosterNav::default()
    };
    assert_eq!(
        xbox.line(&on_kai, &rows, true),
        "Up / Down: move \u{b7} A: kick \u{b7} B: close"
    );
    let asking = RosterNav {
        confirm: Some(1),
        ..on_kai
    };
    assert_eq!(
        ds.line(&asking, &rows, true),
        "Kick Kai? Cross: kick \u{b7} Circle: cancel"
    );
    assert_eq!(
        keys.line(&on_kai, &rows, false),
        "Up / Down: move \u{b7} Backspace: close"
    );
}
