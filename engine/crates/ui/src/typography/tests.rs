use super::*;
use crate::menu::{self, MenuModel, PausePage, SettingsPage};
use manifest::hud::HudGlyph;

/// A font whose every printable glyph advances `advance`.
fn font(line_height: u8, advance: u8) -> HudFont {
    HudFont {
        name: "t".into(),
        line_height,
        pages: vec!["p".into()],
        glyphs: (32u16..127)
            .map(|code| HudGlyph {
                code,
                page: 0,
                advance,
                x0: 0,
                x1: advance,
                y0: 0,
                y1: line_height,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect(),
    }
}

#[test]
fn the_roles_keep_the_shell_stacks_look() {
    // Title and rows: the large set at its own line height, 1.087 across
    // (`re-bf2-shell.md` §3.2); help and paragraphs the small set at 1.11;
    // the dense lists the tiny set at 1.10, in capitals.
    assert_eq!(
        (TITLE.face, TITLE.size, TITLE.stretch),
        (Face::Large, 22.0, 1.087)
    );
    assert_eq!(ITEM, TITLE);
    assert_eq!(
        (HELP.face, HELP.size, HELP.stretch),
        (Face::Small, 18.0, 1.11)
    );
    assert_eq!(BODY, HELP);
    assert_eq!(
        (LIST.face, LIST.size, LIST.case),
        (Face::Tiny, 15.0, Case::Upper)
    );
    for (name, s) in ROLES {
        assert_eq!(s.tracking, 0.0, "{name}: the face's own spacing");
        assert!(s.line >= s.size, "{name}: lines do not overlap");
    }
}

#[test]
fn roles_of_one_face_share_their_stretch() {
    // egui installs one font per metric set with its stretch baked in.
    for face in Face::ALL {
        let stretches: Vec<f32> = ROLES
            .iter()
            .filter(|(_, s)| s.face == face)
            .map(|(_, s)| s.stretch)
            .collect();
        assert!(stretches.windows(2).all(|w| w[0] == w[1]), "{face:?}");
        assert!(!stretches.is_empty(), "{face:?} has a role");
    }
}

#[test]
fn a_role_draws_at_its_size_on_any_metric_set() {
    // The menu face and the disc's fallback share line heights; a role's
    // scale makes the line `size` units either way.
    for lh in [22, 18, 15, 30] {
        let f = font(lh, 10);
        let [kx, ky] = ITEM.scale(&f);
        assert!((ky * f32::from(lh) - ITEM.size).abs() < 1e-4);
        assert!((kx / ky - ITEM.stretch).abs() < 1e-5);
    }
    let f = font(22, 10);
    assert!((width(&f, &ITEM, "Back") - 4.0 * 10.0 * 1.087).abs() < 1e-3);
    // Tracking adds after every glyph.
    let tracked = TextStyle {
        tracking: 1.0,
        ..ITEM
    };
    assert!((width(&f, &tracked, "Back") - width(&f, &ITEM, "Back") - 4.0).abs() < 1e-3);
}

#[test]
fn a_long_text_ends_in_an_ellipsis_at_the_same_size() {
    let f = font(22, 10);
    let room = width(&f, &ITEM, "Guardian Medal");
    assert_eq!(ellipsize(&f, &ITEM, "Guardian", room), "Guardian");
    let cut = ellipsize(&f, &ITEM, "Guardian: Damage Taken Halved", room);
    assert!(cut.ends_with(ELLIPSIS), "{cut}");
    assert!(width(&f, &ITEM, &cut) <= room);
    assert!(cut.len() > ELLIPSIS.len() + 4, "keeps what fits: {cut}");
}

#[test]
fn the_case_rule_is_applied_in_one_place() {
    assert_eq!(LIST.case.apply("Hoth: Echo Base"), "HOTH: ECHO BASE");
    assert_eq!(ITEM.case.apply("Hoth: Echo Base"), "Hoth: Echo Base");
    let f = font(15, 8);
    assert_eq!(width(&f, &LIST, "ab"), width(&f, &LIST, "AB"));
}

/// Title Case as the disc's texts write it: every word capitalised but the
/// small joining words inside a label; numbers, symbols and units as they
/// are.
fn title_case(text: &str) -> Result<(), String> {
    const SMALL: [&str; 12] = [
        "a", "an", "and", "as", "at", "by", "for", "in", "of", "on", "or", "to",
    ];
    for (i, word) in text.split_whitespace().enumerate() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric());
        let Some(first) = w.chars().next() else {
            continue;
        };
        if !first.is_alphabetic() || (i > 0 && SMALL.contains(&w)) {
            continue;
        }
        if !first.is_uppercase() {
            return Err(format!("{text:?}: {word:?}"));
        }
    }
    Ok(())
}

fn model() -> MenuModel {
    MenuModel {
        page: Some(PausePage::Main),
        respawn_label: Some("Respawn".into()),
        restart_available: true,
        class_picker_available: true,
        team_size_pct: 100,
        reinforcements_pct: 100,
        fov_scale: 1.0,
        base_fov_deg: 60.0,
        difficulty: 2,
        input_icons_label: "Auto",
        player_name: "player".into(),
        cheats: CHEAT_IDS
            .iter()
            .map(|id| (*id, "raw label", false))
            .collect(),
        ..Default::default()
    }
}

/// `core::sim::Cheats::entries`' ids (the ui crate does not see core).
const CHEAT_IDS: [&str; 10] = [
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
];

#[test]
fn our_labels_are_title_case() {
    let mut pages = vec![PausePage::Main, PausePage::Options, PausePage::Cheats];
    pages.extend(
        [
            SettingsPage::Game,
            SettingsPage::Audio,
            SettingsPage::Graphics,
            SettingsPage::Controls,
            SettingsPage::KeyboardMouse,
            SettingsPage::Gamepad,
        ]
        .map(PausePage::Settings),
    );
    let mut bad = Vec::new();
    for page in pages {
        for on in [false, true] {
            let mut m = model();
            m.page = Some(page);
            m.network_meter = on;
            m.auto_quality = on;
            m.invert_y = on;
            m.rumble_on_fire = on;
            m.career_awards_disabled = on;
            m.cheats = m.cheats.iter().map(|(id, l, _)| (*id, *l, on)).collect();
            if let Err(e) = title_case(menu::menu_title(&m)) {
                bad.push(e);
            }
            for row in menu::rows(&m) {
                let (name, value) = menu::row_text(&m, row);
                // The player's own name is as typed.
                if name != "Player Name" {
                    bad.extend(title_case(&name).err());
                    bad.extend(value.as_deref().and_then(|v| title_case(v).err()));
                }
            }
        }
    }
    for id in CHEAT_IDS {
        let (label, help) = menu::cheat_text(id).unwrap_or_else(|| panic!("{id} has a label"));
        bad.extend(title_case(label).err());
        assert!(help.ends_with('.'), "{id}: a sentence");
    }
    let class = MenuModel {
        class_choices: Some(Default::default()),
        ..model()
    };
    bad.extend(title_case(menu::menu_title(&class)).err());
    assert!(bad.is_empty(), "not Title Case: {bad:#?}");
    // Toggles say On / Off, one way everywhere.
    assert_eq!((menu::on_off(true), menu::on_off(false)), ("On", "Off"));
}

/// Where a menu module may not style its own text.
const FORBIDDEN: [(&str, &str); 14] = [
    (".color(Color32::", "a text colour (a tint role)"),
    (".color(egui::Color32::", "a text colour (a tint role)"),
    (".size(", "an egui text size"),
    ("FontId::proportional(", "an egui font size"),
    ("FontId::monospace(", "an egui font size"),
    ("FontId::new(", "an egui font size"),
    (".monospace()", "an egui face"),
    (".heading(", "an egui heading size"),
    (".strong()", "an egui weight"),
    ("to_uppercase()", "a case rule"),
    ("_STRETCH", "a stretch"),
    ("\"gamefont_", "a font name"),
    ("\"menu_gamefont_", "a font name"),
    ("Run {", "a hand-made text run"),
];

/// The HUD (BF2's tree and the code-drawn pieces beside it) keeps the
/// layouts' own fonts; the renderer and this module define text. The campaign
/// objective browser is a code-drawn HUD piece on the same whole-map pass;
/// `hud.rs` holds the layouts' own font-name data, not menu text styling.
const EXEMPT: [&str; 17] = [
    "typography.rs",
    "typography/",
    "hud.rs",
    "hud_draw.rs",
    "hud_text.rs",
    "hud_tree.rs",
    "hud_tree/",
    "hud_map.rs",
    "campaign_map.rs",
    "hud_squad.rs",
    "hud_scope.rs",
    "hud_space.rs",
    "hud_capture.rs",
    "hud_ambush.rs",
    "hud_flag.rs",
    "tests.rs",
    "_tests.rs",
];

/// `text` with every `#[cfg(test)]` item blanked, newlines kept: the item
/// after the attribute (and any further attributes) up to its `;` or its
/// closing brace. Code after a test item is still checked.
fn strip_tests(text: &str) -> String {
    const ATTR: &str = "#[cfg(test)]";
    let mut out: Vec<char> = text.chars().collect();
    let chars = out.clone();
    let blank = |out: &mut Vec<char>, from: usize, to: usize| {
        for c in &mut out[from..to] {
            if *c != '\n' {
                *c = ' ';
            }
        }
    };
    let mut i = 0;
    while i + ATTR.len() <= chars.len() {
        if chars[i..i + ATTR.len()].iter().copied().ne(ATTR.chars()) {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + ATTR.len();
        // Further attributes and blank space before the item.
        loop {
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j + 1 < chars.len() && chars[j] == '#' && chars[j + 1] == '[' {
                while j < chars.len() && chars[j] != ']' {
                    j += 1;
                }
                j += 1;
            } else {
                break;
            }
        }
        let (mut depth, mut opened) = (0i32, false);
        while j < chars.len() {
            match chars[j] {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                ';' if !opened => {
                    j += 1;
                    break;
                }
                _ => {}
            }
            j += 1;
            if opened && depth <= 0 {
                break;
            }
        }
        blank(&mut out, start, j.min(chars.len()));
        i = j;
    }
    out.into_iter().collect()
}

/// An all-capitals role ([`Case::Upper`]) handed to egui by its font alone
/// skips its case rule: it must go through [`egui_text`] or apply
/// `.case.apply(` beside the font.
fn upper_roles_without_case(body: &str, rel: &str) -> Vec<String> {
    let upper: Vec<&str> = ROLES
        .iter()
        .filter(|(_, s)| s.case == Case::Upper)
        .map(|(n, _)| *n)
        .collect();
    let mut bad = Vec::new();
    for (i, _) in body.match_indices("egui_font(") {
        let call: String = body[i..].chars().take_while(|c| *c != ')').collect();
        if !upper.iter().any(|r| call.contains(r)) {
            continue;
        }
        let near = |a: usize, b: usize| body.get(a..b.min(body.len())).unwrap_or("");
        let around = near(i.saturating_sub(400), i + 400);
        if !around.contains(".case.apply(") {
            let line = body[..i].lines().count().max(1);
            bad.push(format!(
                "{rel}:{line}: an upper-case role without its case rule"
            ));
        }
    }
    bad
}

fn scan(dir: &std::path::Path, root: &std::path::Path, bad: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            scan(&path, root, bad);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if EXEMPT.iter().any(|e| rel.ends_with(e) || rel.contains(e)) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // Unit tests inside a module build their own fixtures: blank each
        // test item, keep the rest of the file (and its line numbers).
        let body = strip_tests(&text);
        bad.extend(upper_roles_without_case(&body, &rel));
        for (n, line) in body.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            // A reasoned exception says why on its line.
            if line.contains("// typography: ") {
                continue;
            }
            for (pat, what) in FORBIDDEN {
                // `.size(` on a size vector (`r.size()`, `Vec2`) is layout,
                // not text: only a literal or a font size counts.
                let hit = if pat == ".size(" {
                    // `r.size()` is a rectangle's; any argument is a text
                    // size, which a role defines.
                    code.match_indices(".size(")
                        .any(|(i, _)| code[i + 6..].chars().next().is_some_and(|c| c != ')'))
                } else if pat == "Run {" {
                    code.match_indices(pat).any(|(i, _)| {
                        !code[..i]
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_')
                    })
                } else {
                    code.contains(pat)
                };
                if hit {
                    bad.push(format!("{rel}:{}: {what}: {}", n + 1, line.trim()));
                }
            }
        }
    }
}

#[test]
fn menu_modules_take_text_styles_from_typography() {
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let windowed = ui.join("../../game/src/main/windowed");
    let mut bad = Vec::new();
    scan(&ui, &ui, &mut bad);
    assert!(windowed.is_dir(), "{}", windowed.display());
    scan(&windowed, &windowed, &mut bad);
    assert!(
        bad.is_empty(),
        "text styled outside ui::typography:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_guard_sees_a_hard_coded_size() {
    let dir = std::env::temp_dir().join(format!("typography_guard_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("menu_page.rs"),
        "fn f(ui: &mut egui::Ui) { ui.label(egui::RichText::new(\"x\").size(18.0)); }\n",
    )
    .unwrap();
    let mut bad = Vec::new();
    scan(&dir, &dir, &mut bad);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(bad.len(), 1, "{bad:?}");
}

#[test]
fn the_guard_checks_code_after_a_test_item() {
    let dir = std::env::temp_dir().join(format!("typography_guard_tail_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("menu_page.rs"),
        "#[cfg(test)] mod t;\n\
         #[cfg(test)]\n\
         mod tests {\n    fn f() { let _ = egui::RichText::new(\"x\").size(1.0); }\n}\n\
         fn g(ui: &mut egui::Ui) { ui.label(egui::RichText::new(\"x\").size(SIZE)); }\n\
         fn h(ui: &mut egui::Ui) { ui.label(egui::RichText::new(\"x\").color(egui::Color32::RED)); }\n\
         fn k(ctx: &egui::Context) { let _ = egui_font(ctx, &LIST); }\n",
    )
    .unwrap();
    let mut bad = Vec::new();
    scan(&dir, &dir, &mut bad);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(bad.len(), 3, "{bad:#?}");
    assert!(bad
        .iter()
        .any(|b| b.contains(":6:") && b.contains("text size")));
    assert!(bad
        .iter()
        .any(|b| b.contains(":7:") && b.contains("colour")));
    assert!(bad
        .iter()
        .any(|b| b.contains(":8:") && b.contains("case rule")));
}

#[test]
fn readouts_are_small_in_the_one_face() {
    assert_eq!(READOUT.face, BODY.face);
    assert_eq!(READOUT.stretch, BODY.stretch);
    const { assert!(READOUT.size < BODY.size * 0.5) };
    assert_eq!(READOUT.case, Case::AsWritten);
}
