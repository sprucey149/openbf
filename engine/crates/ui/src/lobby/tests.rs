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
        mode: String::new(),
        players: "2/8".into(),
        ping_ms: 3,
        refusal: refusal.map(Into::into),
    }
}

/// Join Game: the found hosts, the address box and the recent games; a
/// host on another version is dimmed and skipped; Connect carries the
/// address; an empty box is refused with the hint.
#[test]
fn join_page_rows_connect_and_refusals() {
    let mut j = JoinPage::new(Vec::new());
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Address);
    assert!(j.focus_text, "the address box has the keyboard at once");
    assert_eq!(j.step(press(|i| i.confirm = true), false), None);
    assert!(j.status.as_deref().unwrap().contains("address"));
    *j.text_mut().unwrap() = " studio-pc:40000 ".into();
    assert_eq!(
        j.step(press(|i| i.confirm = true), false),
        Some(JoinCommand::Connect("studio-pc:40000".into()))
    );
    j.set_hosts(
        vec![
            found("192.168.1.5:37450", None),
            found("192.168.1.6:37450", Some("different version")),
        ],
        Vec::new(),
    );
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Address, "focus kept");
    let rows = j.rows();
    assert!(rows[0].heading && !rows[0].enabled);
    assert_eq!(rows[0].label, "Games on this network");
    assert!(rows[1].enabled && !rows[2].enabled);
    assert!(rows[2].label.ends_with("(different version)"));
    j.step(press(|i| i.up = true), false);
    assert_eq!(
        j.row_ids()[j.cursor],
        JoinRow::Host(0),
        "the dimmed host is skipped"
    );
    // A host appearing before it keeps the focus on the same address.
    j.set_hosts(
        vec![
            found("192.168.1.2:37450", None),
            found("192.168.1.5:37450", None),
        ],
        Vec::new(),
    );
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Host(1));
    assert_eq!(
        j.step(press(|i| i.confirm = true), false),
        Some(JoinCommand::Connect("192.168.1.5:37450".into()))
    );
    j.step(press(|i| i.down = true), false);
    j.step(press(|i| i.down = true), false);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Test);
    assert_eq!(j.rows()[j.cursor].label, "Test Connection");
    assert_eq!(
        j.step(press(|i| i.confirm = true), false),
        Some(JoinCommand::Test("studio-pc:40000".into())),
        "Test connection asks the typed address"
    );
    assert_eq!(
        j.step(press(|_| {}), true),
        None,
        "Forget only on a recent game"
    );
    assert_eq!(
        j.step(press(|i| i.back = true), false),
        Some(JoinCommand::Back)
    );
}

/// Recent games: above the address box, labelled with the host's answer
/// while it hosts and "not hosting now" (still pressable) when not; the
/// cursor starts on the first game and jumps to one that answers until the
/// player presses something; Square forgets the one under the cursor.
#[test]
fn join_page_recent_games_rows_cursor_and_forget() {
    let recent = vec![
        RecentRow::new("ann-pc", "Ann", "2 days ago"),
        RecentRow::new("100.64.0.9", "", ""),
    ];
    let mut j = JoinPage::new(recent);
    assert_eq!(
        j.row_ids(),
        [
            JoinRow::Heading(JoinSection::Recent),
            JoinRow::Recent(0),
            JoinRow::Recent(1),
            JoinRow::Address,
            JoinRow::Test,
            JoinRow::Back
        ]
    );
    assert_eq!(
        j.row_ids()[j.cursor],
        JoinRow::Recent(0),
        "starts on a game"
    );
    assert!(!j.focus_text);
    let rows = j.rows();
    assert_eq!(rows[0].label, "Recent games");
    assert_eq!(rows[1].label, "Ann (not hosting now, joined 2 days ago)");
    assert_eq!(rows[1].value.as_deref(), Some("ann-pc"));
    assert!(rows[1].enabled, "an offline recent game stays pressable");
    assert_eq!(rows[2].label, "100.64.0.9 (not hosting now)");
    // The second answers: the cursor, untouched, moves to it.
    let live = FoundRow {
        mode: "Conquest".into(),
        ..found("100.64.0.9:37450", None)
    };
    j.set_hosts(
        vec![found("192.168.1.5:37450", None)],
        vec![None, Some(live.clone())],
    );
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Recent(1));
    let rows = j.rows();
    assert_eq!(rows[0].label, "Games on this network");
    assert_eq!(rows[4].label, "Host \u{2014} Kashyyyk, Conquest");
    assert_eq!(rows[4].value.as_deref(), Some("2/8  3 ms"));
    assert_eq!(
        j.step(press(|i| i.confirm = true), false),
        Some(JoinCommand::Connect("100.64.0.9".into())),
        "a recent game joins by the address it was joined at"
    );
    // Touched: a later list change keeps the focus on its row.
    j.set_hosts(Vec::new(), vec![Some(live.clone()), Some(live)]);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Recent(1));
    assert_eq!(j.step(press(|_| {}), true), Some(JoinCommand::Forget(1)));
    j.set_recent(vec![RecentRow::new("ann-pc", "Ann", "2 days ago")]);
    assert_eq!(
        j.row_ids()[j.cursor],
        JoinRow::Address,
        "the next row after the forgotten one"
    );
    j.step(press(|i| i.up = true), false);
    assert_eq!(j.row_ids()[j.cursor], JoinRow::Recent(0));
    assert_eq!(
        j.step(press(|i| i.confirm = true), false),
        Some(JoinCommand::Connect("ann-pc".into()))
    );
}

fn roster() -> Vec<RosterRow> {
    let row = |slot: u8, name: &str| RosterRow {
        slot,
        machine: slot,
        name: name.into(),
        ping_ms: (slot != 0).then_some(40),
        addon: false,
        team: "Auto".into(),
        ready: false,
    };
    vec![row(0, "Rex"), row(1, "Kai"), row(2, "Sam")]
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
    // Sam leaves while asked, the pad falls back to Rex: no question.
    nav.input(false, accept, &kai_gone, true);
    nav.input(false, none, &kai_gone[..1], true);
    assert_eq!(nav.confirm, None, "disarmed on Rex");
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

/// Splitscreen B19: a roster with several players on one machine. The host
/// (slot 0) has a second player who took slot 3 after two guest machines
/// joined: Kai alone (slot 1), and Han with Leia (slots 2 and 4, Leia
/// seated later). The rows group by machine, the host's own first, each
/// machine's first player before its further ones; the host's players are
/// never kicked, and a kick from any row of Han's machine asks for and
/// removes the whole machine.
#[test]
fn roster_rows_group_by_machine_and_kick_a_whole_machine() {
    let row = |slot: u8, machine: u8, name: &str| RosterRow {
        slot,
        machine,
        name: name.into(),
        ping_ms: (machine != 0).then_some(40),
        addon: false,
        team: "Auto".into(),
        ready: false,
    };
    let rows = group_rows(vec![
        row(0, 0, "Rex"),
        row(1, 1, "Kai"),
        row(2, 2, "Han"),
        row(3, 0, "Rex_1"),
        row(4, 2, "Leia"),
    ]);
    let order: Vec<(u8, &str)> = rows.iter().map(|r| (r.slot, r.name.as_str())).collect();
    assert_eq!(
        order,
        [
            (0, "Rex"),
            (3, "Rex_1"),
            (1, "Kai"),
            (2, "Han"),
            (4, "Leia")
        ],
        "the host's own players first, then each machine, its first player first"
    );
    assert!(rows[0].host_side() && rows[1].host_side() && !rows[2].host_side());
    assert!(rows[1].follows() && rows[4].follows() && !rows[3].follows());
    assert_eq!(machine_names(&rows, 2), "Han and Leia");
    assert_eq!(machine_names(&rows, 1), "Kai");
    let accept = press(|i| i.confirm = true);
    let none = MenuInput::default();
    let mut nav = RosterNav::default();
    nav.input(true, none, &rows, true);
    // The host's second player is the host's: no kick.
    nav.input(false, press(|i| i.down = true), &rows, true);
    assert_eq!(nav.focus, Some(1));
    assert_eq!(nav.input(false, accept, &rows, true), None);
    assert_eq!(nav.confirm, None, "the host's own player is never kicked");
    assert_eq!(
        RosterPrompts::default().line(&nav, &rows, true),
        "Up / Down: move \u{b7} : close"
    );
    // Leia's row asks for her whole machine.
    nav.focus = Some(4);
    assert_eq!(nav.input(false, accept, &rows, true), None);
    assert_eq!(nav.confirm, Some(2), "asks for Han's machine");
    assert_eq!(
        RosterPrompts::default().line(&nav, &rows, true),
        "Kick Han and Leia? : kick \u{b7} : cancel"
    );
    // Moving to Han's row (the same machine) drops the question as any move
    // does; asked again there, the second Accept kicks machine 2.
    nav.input(false, press(|i| i.up = true), &rows, true);
    assert_eq!((nav.focus, nav.confirm), (Some(3), None));
    assert_eq!(nav.input(false, accept, &rows, true), None);
    assert_eq!(nav.input(false, accept, &rows, true), Some(2));
    // Kai alone is his own machine.
    nav.focus = Some(2);
    nav.input(false, accept, &rows, true);
    assert_eq!(nav.input(false, accept, &rows, true), Some(1));
    // The machine leaves while asked: the question is not handed on.
    nav.focus = Some(3);
    nav.input(false, accept, &rows, true);
    let gone = group_rows(vec![row(0, 0, "Rex"), row(3, 0, "Rex_1"), row(1, 1, "Kai")]);
    nav.focus = Some(2);
    assert_eq!(
        nav.input(false, accept, &gone, true),
        None,
        "Kai not kicked unasked"
    );
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
