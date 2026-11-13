//! Tier-2 visual goldens of the BF2 PS2 shell painted by the HUD pass,
//! headless: the front screens (card BSHLA) and the Instant Action screen
//! with its Session Options (`ui::shell_ia`).
//!   cargo test -p game --test golden_shell -- --ignored
//!   UPDATE_GOLDENS=1 cargo test -p game --test golden_shell -- --ignored

mod common;

/// The BF2 PS2 shell's front screens (card BSHLA) as the HUD pass paints
/// them: each over a fixed frame of its movie (`shell_main` segment 0 frame
/// 300 for the title and the main menu, `shell_sub_left`'s first segment
/// frame 150 for Singleplayer and Options), the rows all revealed, the
/// prompt at its brightest. Needs the BF2 common bake with the shell.
/// `SHELL_PS2_OUT=<dir>`: also each at 640 x 480, the PS2 frames' size.
fn front_screens() -> Vec<(&'static str, ui::shell_ps2::ShellMenus)> {
    use ui::menu::MenuInput;
    use ui::shell_ps2::{Page, ShellMenus};
    let texts = game::event_feed::Texts::load(&game::assets::baked_root(), "bf2");
    let text = |key: &str, english: &str| {
        texts
            .bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    };
    let base = ShellMenus::new(&text);
    let settle = |mut m: ShellMenus, page: Page| {
        m.page = page;
        // Five seconds on the page (every row shown), the prompt's clock at
        // the top of its swing.
        m.step(MenuInput::default(), false, 0.0);
        m.on_page = 5.0;
        m.clock = 0.0;
        m
    };
    vec![
        ("title", settle(base.clone(), Page::Title)),
        ("main", settle(base.clone(), Page::Main)),
        ("singleplayer", settle(base.clone(), Page::Singleplayer)),
        ("options", settle(base, Page::Options)),
    ]
}

fn front_screen(m: &ui::shell_ps2::ShellMenus, size: [u32; 2]) -> (Vec<u8>, u32, u32) {
    let assets = game::assets::baked_root();
    let movie = game::shell_movie::ShellMovie::load(&assets).expect("the shell bake (bake common)");
    let (seg, frame) = if m.movie() == ui::shell_ps2::MOVIE_MAIN {
        (0, 300)
    } else {
        (0, 150)
    };
    let (rgba, w, h) = movie.frame_now(m.movie(), seg, frame).expect("movie frame");
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let mut view =
        game::hud_feed::HudView::for_renderer(&assets, &r).expect("the BF2 HUD bake (bake common)");
    view.set_movie_frame(rgba, w, h);
    view.update_shell(m, true, size);
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.0, 0.0, 0.0, 1.0],
        Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
    )
    .expect("render");
    r.capture_rgba().expect("capture")
}

#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn ps2_front_screens_match_golden() {
    common::force_ps2_tier();
    game::shell_movie::load_index(&game::assets::baked_root())
        .expect("no valid shell index in the BF2 common bake (rebake common)");
    let ps2 = std::env::var_os("SHELL_PS2_OUT").map(std::path::PathBuf::from);
    let mut failures = Vec::new();
    for (name, m) in front_screens() {
        let (rgba, w, h) = front_screen(&m, [1280, 720]);
        let golden = common::goldens_dir().join(format!("shell_{name}.png"));
        if let Some(f) = common::check_golden(&golden, &rgba, w, h) {
            failures.push(f);
        }
        if let Some(dir) = &ps2 {
            let (rgba, w, h) = front_screen(&m, [640, 480]);
            image::RgbaImage::from_raw(w, h, rgba)
                .expect("frame")
                .save(dir.join(format!("shell_{name}_640.png")))
                .expect("write");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The help lines draw each icon set through the real pass: only the icons
/// change from the PS2's own, each is textured (its art rendered, not the
/// untextured quad a missing sheet leaves), and the rest of the frame stands.
/// `SHELL_PS2_OUT=<dir>`: also each frame.
#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake"]
fn help_lines_draw_every_icon_set() {
    use ui::input_icons::{IconSet, Icons, ALL_SETS};
    common::force_ps2_tier();
    let assets = game::assets::baked_root();
    game::shell_movie::load_index(&assets).expect("the shell index");
    let m = front_screens()
        .into_iter()
        .find(|(name, _)| *name == "singleplayer")
        .expect("the Singleplayer page")
        .1;
    let size = [1280, 720];
    let frame = |icons: Icons| {
        let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
        let mut view = game::hud_feed::HudView::for_renderer(&assets, &r).expect("the HUD bake");
        view.set_icons(icons);
        view.update_shell(&m, false, size);
        let drawn: Vec<String> = view.drawn_sheets().map(str::to_owned).collect();
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.2, 0.2, 0.2, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .expect("render");
        let (rgba, w, _) = r.capture_rgba().expect("capture");
        (rgba, w as usize, drawn)
    };
    let (base, width, _) = frame(Icons::default());
    let out = std::env::var_os("SHELL_PS2_OUT").map(std::path::PathBuf::from);
    for set in ALL_SETS {
        let icons = Icons::of(set);
        let (rgba, _, drawn) = frame(icons);
        let accept = icons.sheet(ui::input_icons::Prompt::Accept);
        assert!(drawn.iter().any(|s| s == accept), "{set:?}: {drawn:?}");
        // The two help lines stand at the screen's bottom corners.
        let (mut inside, mut outside) = (0, 0);
        // The colours each corner changed to: art has edges and shading, the
        // untextured quad of a missing sheet is one flat colour.
        let mut colours = [std::collections::HashSet::new(), Default::default()];
        for (i, (a, b)) in rgba.chunks_exact(4).zip(base.chunks_exact(4)).enumerate() {
            if a != b {
                let (x, y) = (i % width, i / width);
                let corner =
                    y > size[1] as usize * 3 / 4 && (x < 200 || x > size[0] as usize - 200);
                if corner {
                    inside += 1;
                    colours[usize::from(x >= size[0] as usize / 2)]
                        .insert(<[u8; 4]>::try_from(a).unwrap());
                } else {
                    outside += 1;
                }
            }
        }
        if set != IconSet::Ds2 {
            assert!(inside > 50, "{set:?}: the icons did not change ({inside})");
            for (side, seen) in ["left", "right"].iter().zip(&colours) {
                assert!(
                    seen.len() > 8,
                    "{set:?}: the {side} icon is {} flat colour(s): untextured",
                    seen.len()
                );
            }
        } else {
            assert_eq!(inside, 0, "the PS2's own art is the base");
        }
        assert_eq!(outside, 0, "{set:?}: pixels moved away from the help lines");
        if let Some(dir) = &out {
            image::RgbaImage::from_raw(size[0], size[1], rgba)
                .expect("frame")
                .save(dir.join(format!("icons_{set:?}.png")))
                .expect("write");
        }
    }
}

/// The Instant Action screen's states and Session Options' pages
/// (`analysis/re-bf2-menus.md` §1-§2), each the PCSX2 reference frame's
/// (`bshl/ref_frames/<name>.png`) counterpart where there is one.
fn ia_screens() -> Vec<(&'static str, ui::shell::InstantAction)> {
    use ui::shell::{Cmd, Era, IaInput, MapPick, Mode, ModePick, Page, Side, SidePick, State};
    let assets = game::assets::baked_root();
    // `engine/missions` (the bake may be a link elsewhere).
    let missions = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    // Normal, the port's default tier: the AI Difficulty row reads
    // "< Normal >", as the PS2's own frame does.
    let base = game::shell_ia::instant_action(&assets, &missions, core::skill::Difficulty::Normal);
    let at = |ia: &ui::shell::InstantAction, label: &str| {
        ia.maps.iter().position(|m| m.label == label).expect(label)
    };
    let cmd = |ia: &mut ui::shell::InstantAction, c: Cmd| {
        ia.command = ui::shell::COMMANDS.iter().position(|x| *x == c).unwrap();
    };
    let mut out = Vec::new();
    out.push(("ia_missionselect", base.clone()));
    let mut m = base.clone();
    m.state = State::Map;
    out.push(("ia_planet_list", m.clone()));
    let hoth = at(&m, "Hoth (bf2)");
    m.map_list.at = hoth;
    m.map_list.top = hoth + 1 - ui::shell::LIST_ROWS;
    out.push(("ia_planet_hoth", m.clone()));
    m.picked_map = MapPick::One(hoth);
    m.state = State::Mode;
    out.push(("ia_mode", m.clone()));
    m.picked_mode = ModePick::One(Mode::Con);
    m.state = State::Era;
    out.push(("ia_era", m.clone()));
    m.add(
        MapPick::One(hoth),
        ModePick::One(Mode::Con),
        SidePick::One(Side::Era(Era::Gcw)),
    );
    m.state = State::Command;
    cmd(&mut m, Cmd::Launch);
    out.push(("ia_playlist_launch", m.clone()));
    let mut p = m.clone();
    cmd(&mut p, Cmd::Remove);
    p.state = State::Playlist;
    p.play_list.at = 1;
    out.push(("ia_playlist", p));
    let mut full = m.clone();
    full.add(MapPick::All, ModePick::All, SidePick::AllEras);
    full.add(MapPick::All, ModePick::All, SidePick::AllEras);
    full.add(MapPick::All, ModePick::All, SidePick::AllEras);
    out.push(("ia_full", full));
    let mut o = m;
    cmd(&mut o, Cmd::Options);
    out.push(("ia_options_focus", o.clone()));
    o.page = Page::Overview(0);
    out.push(("session_options", o.clone()));
    o.step(
        IaInput {
            menu: ui::menu::MenuInput {
                confirm: true,
                ..Default::default()
            },
            ..Default::default()
        },
        0.0,
    );
    out.push(("options_global", o.clone()));
    o.page = Page::Overview(2);
    o.step(
        IaInput {
            menu: ui::menu::MenuInput {
                confirm: true,
                ..Default::default()
            },
            ..Default::default()
        },
        0.0,
    );
    out.push(("options_conquest", o));
    out
}

fn ia_screen(ia: &ui::shell::InstantAction, size: [u32; 2]) -> (Vec<u8>, u32, u32) {
    use ui::shell_ps2::{Page, ShellMenus};
    let assets = game::assets::baked_root();
    let mut menus = ShellMenus::new(&|_, e| e.to_owned());
    menus.page = Page::Singleplayer;
    menus.covered = true;
    let movie = ia.page != ui::shell::Page::Missions;
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let mut view =
        game::hud_feed::HudView::for_renderer(&assets, &r).expect("the BF2 HUD bake (bake common)");
    if movie {
        let shell = game::shell_movie::ShellMovie::load(&assets).expect("the shell bake");
        let (rgba, w, h) = shell
            .frame_now(ui::shell_ps2::MOVIE_SUB, 0, 150)
            .expect("movie frame");
        view.set_movie_frame(rgba, w, h);
    }
    view.update_instant(&menus, ia, movie, size);
    r.render_with_overlay(
        &render::Camera::default(),
        &[],
        [0.0, 0.0, 0.0, 1.0],
        Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
    )
    .expect("render");
    r.capture_rgba().expect("capture")
}

/// `IA_OUT=<dir>`: also each at 640 x 480, the PS2 frames' size.
#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake with the IA sheets"]
fn ia_screens_match_golden() {
    common::force_ps2_tier();
    let out = std::env::var_os("IA_OUT").map(std::path::PathBuf::from);
    let mut failures = Vec::new();
    for (name, ia) in ia_screens() {
        let (rgba, w, h) = ia_screen(&ia, [1280, 720]);
        let golden = common::goldens_dir().join(format!("shell_{name}.png"));
        if let Some(f) = common::check_golden(&golden, &rgba, w, h) {
            failures.push(f);
        }
        if let Some(dir) = &out {
            let (rgba, w, h) = ia_screen(&ia, [640, 480]);
            image::RgbaImage::from_raw(w, h, rgba)
                .expect("frame")
                .save(dir.join(format!("{name}_640.png")))
                .expect("write");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The Hero page shows a scroll arrow only when there are more options.
#[test]
#[ignore = "requires a GPU adapter + the BF2 common bake with the IA sheets"]
fn the_hero_options_page_shows_the_scroll_arrow_when_needed() {
    common::force_ps2_tier();
    let assets = game::assets::baked_root();
    let missions = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let base = game::shell_ia::instant_action(&assets, &missions, core::skill::Difficulty::Normal);
    let page = |heroes: bool, cursor: usize| {
        let mut ia = base.clone();
        ia.options.heroes = heroes;
        ia.options.dlc_heroes_installed = false;
        ia.options.dlc_heroes = false;
        ia.page = ui::shell::Page::Group(
            ui::shell::Group::Hero,
            ui::shell::ListCursor { at: 0, top: cursor },
        );
        let (rgba, w, h) = ia_screen(&ia, [1280, 720]);
        if let Some(dir) = std::env::var_os("IA_OUT").map(std::path::PathBuf::from) {
            image::RgbaImage::from_raw(w, h, rgba.clone())
                .expect("frame")
                .save(dir.join(format!("hero_{heroes}_{cursor}.png")))
                .expect("write");
        }
        rgba
    };
    let last_page = base
        .options
        .rows(ui::shell::Group::Hero)
        .len()
        .saturating_sub(ui::shell::OPTION_ROWS);
    let (short, long, scrolled) = (page(false, 0), page(true, 0), page(true, last_page));
    let bottom_edge = |rgba: &[u8]| -> Vec<u8> {
        // Across the middle of the window's bottom border, where the arrow's ink falls.
        (440..840)
            .flat_map(|x| (440..462).map(move |y| (x, y)))
            .flat_map(|(x, y): (usize, usize)| rgba[(y * 1280 + x) * 4..][..3].to_vec())
            .collect()
    };
    assert_eq!(
        bottom_edge(&short),
        bottom_edge(&scrolled),
        "no lower arrow at the end of either list"
    );
    assert_ne!(bottom_edge(&short), bottom_edge(&long), "the arrow below");
}
