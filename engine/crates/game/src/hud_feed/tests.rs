#[path = "cutter_tests.rs"]
mod cutter_tests;
#[path = "fisto_tests.rs"]
mod fisto_tests;
#[path = "item_layer_tests.rs"]
mod item_layer_tests;

use super::*;

#[test]
#[ignore = "requires GPU and tutorial bake"]
fn tutorial_followup_campaign_map_renders() {
    let root = crate::assets::baked_root();
    let mut scene = crate::assets::load_scene(&root.join("bf2_geo1")).unwrap();
    scene.apply_mode_layer("con");
    let page = ui::hud_draw::campaign::Model {
        selected: 7,
        objectives: (0..8)
            .map(|i| ui::hud_draw::campaign::Objective {
                text: format!("Objective {}: follow the marked target", i + 1),
                active: i == 7,
            })
            .collect(),
        footer: [
            "Objective Details {usebutton}",
            "Hints {lockTargetButton}",
            "Return {Map}",
        ]
        .map(str::to_owned),
    };
    let out = root.parent().unwrap().join("artifacts/tutorial-ui");
    std::fs::create_dir_all(&out).unwrap();
    for (name, icons) in [
        ("ps2", ui::input_icons::Icons::default()),
        (
            "keyboard",
            ui::input_icons::Icons::of(ui::input_icons::IconSet::KeyboardMouse),
        ),
    ] {
        let mut r = render::Renderer::new_headless(1280, 720).unwrap();
        let mut view = HudView::for_renderer(&root, &r).unwrap();
        view.set_icons(icons);
        view.hud.map = Some(crate::minimap::still(&scene, glam::Vec3::ZERO, 0.0));
        let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
        let unit_sheet = db
            .prop("rep_inf_ep2_rifleman", "MapTexture")
            .unwrap()
            .to_owned();
        let map = view.hud.map.as_mut().unwrap();
        map.units.push(ui::hud_map::MapIcon {
            pos: map.centre,
            texture: unit_sheet.clone(),
            scale: 2.0,
            color: ui::hud_map::FRIEND,
            ..Default::default()
        });
        // The production feed registers this map-specific texture directory.
        view.texture_dirs
            .insert(0, map.sheet_dir.as_ref().unwrap().clone());
        let frame = ui::hud_draw::campaign::draw(&view.hud, &view.art, &page, [1280.0, 720.0]);
        view.install_frame(frame);
        assert!(
            view.drawn_sheets().any(|s| s == unit_sheet),
            "large map lost its live units"
        );
        assert!(
            view.drawn_sheets().any(|s| s.contains("geo1_map")),
            "missing large map"
        );
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.15, 0.12, 0.1, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .unwrap();
        assert!(
            view.unresolved_sheets().next().is_none(),
            "missing campaign map art"
        );
        let (rgba, w, h) = r.capture_rgba().unwrap();
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(out.join(format!("campaign-map-{name}.png")))
            .unwrap();
    }
}

#[test]
#[ignore = "requires a GPU adapter and the BF2 HUD bake"]
fn tutorial_prompts_render_for_each_device() {
    use ui::input_icons::{IconSet, Icons};
    let assets = crate::assets::baked_root();
    let size = [1280, 720];
    let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
    let out = assets.parent().unwrap().join("artifacts/tutorial-ui");
    std::fs::create_dir_all(&out).unwrap();
    for (name, set) in [
        ("ps2", IconSet::Ds2),
        ("xbox", IconSet::XboxOne),
        ("keyboard", IconSet::KeyboardMouse),
    ] {
        let mut view = HudView::for_renderer(&assets, &r).unwrap();
        view.set_icons(Icons::of(set));
        view.send(
            "objectivePopup",
            ui::hud_tree::EventValue::Text(
                "{btnr1} Fire   {btnl1} Grenade\n{btna} Jump   {btnb} Use".into(),
            ),
        );
        view.send("player1.objectivesUpdated", ui::hud_tree::EventValue::None);
        view.hud.advance(0.5);
        let frame = ui::hud_draw::draw(&view.hud, &view.art, size.map(|v| v as f32));
        view.install_frame(frame);
        let expected = if set == IconSet::KeyboardMouse {
            "icon_kb_enter"
        } else {
            ui::input_icons::pad_sheet(set, ui::input_icons::PadButton::L2)
        };
        assert!(
            view.drawn_sheets().any(|s| s == expected),
            "missing {expected}"
        );
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.15, 0.12, 0.1, 1.0],
            Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
        )
        .unwrap();
        assert!(
            view.unresolved_sheets().next().is_none(),
            "missing tutorial art"
        );
        let (rgba, w, h) = r.capture_rgba().unwrap();
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(out.join(format!("prompts-{name}.png")))
            .unwrap();
    }
}
use manifest::hud::{HudDistanceFont, HudDistanceGlyph};

#[test]
fn menu_family_shares_pages_keeps_symbol_fallback_and_rejects_partial_bakes() {
    use manifest::hud::{HudFont, HudGlyph, MENU_ATLAS, MENU_FONTS};
    let dir = std::env::temp_dir().join(format!("openbf-hud-menu-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let page = format!("{MENU_ATLAS}.sdf0");
    std::fs::write(
        dir.join(format!("{page}.ktx2")),
        bake::ktx2w::write_rgba8(1024, 1024, &[vec![128; 1024 * 1024 * 4]]),
    )
    .unwrap();
    let glyph = |code, advance| HudGlyph {
        code,
        advance,
        page: 0,
        x0: 0,
        x1: 5,
        y0: 0,
        y1: 8,
        uv: [0.0, 0.1, 0.0, 0.1],
    };
    let mut legacy = HudArt::default();
    for (name, role) in MENU_FONTS {
        let font = HudFont {
            name: name.into(),
            line_height: 22,
            pages: vec![page.clone()],
            glyphs: vec![glyph(65, 7)],
        };
        std::fs::write(
            dir.join(format!("{name}.ron")),
            ron::to_string(&font).unwrap(),
        )
        .unwrap();
        let sdf = HudDistanceFont {
            pages: vec![page.clone()],
            glyphs: vec![HudDistanceGlyph {
                code: 65,
                page: 0,
                rect: [0.0, 0.0, 5.0, 8.0],
                uv: [0.0, 0.0, 0.1, 0.1],
            }],
        };
        std::fs::write(
            dir.join(format!("{name}.sdf.ron")),
            ron::to_string(&sdf).unwrap(),
        )
        .unwrap();
        legacy.fonts.insert(
            role.into(),
            HudFont {
                name: role.into(),
                pages: vec!["legacy".into()],
                glyphs: vec![glyph(65, 99), glyph(0x2605, 11)],
                ..font
            },
        );
    }
    let mut art = legacy.clone();
    let mut pages = HashMap::new();
    add_menu_fonts(&mut art, &mut pages, &dir);
    assert_eq!(pages.len(), 1, "three roles share one decoded page");
    for (_, role) in MENU_FONTS {
        let menu = art.menu_font(role).unwrap();
        assert_eq!(
            menu.glyphs.iter().find(|g| g.code == 65).unwrap().advance,
            7
        );
        let symbol = menu.glyphs.iter().find(|g| g.code == 0x2605).unwrap();
        assert_eq!((symbol.page, symbol.advance), (1, 11));
        assert_eq!(art.fonts[role], legacy.fonts[role]);
    }
    std::fs::remove_file(dir.join("menu_gamefont_tiny.sdf.ron")).unwrap();
    let mut art = legacy.clone();
    let mut pages = HashMap::new();
    add_menu_fonts(&mut art, &mut pages, &dir);
    assert_eq!(art.fonts, legacy.fonts);
    assert!(pages.is_empty());
    assert!(art.distance_fonts.is_empty());
    assert_eq!(
        art.menu_font("gamefont_large").unwrap().name,
        "gamefont_large"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_distance_page_falls_back_to_bitmap_without_dropping_usable_glyphs() {
    let dir = std::env::temp_dir().join(format!("openbf-hud-distance-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let name = "testfont";
    let metadata = HudDistanceFont {
        pages: vec!["testfont.sdf0".into(), "testfont.sdf1".into()],
        glyphs: [65, 66]
            .into_iter()
            .enumerate()
            .map(|(page, code)| HudDistanceGlyph {
                code,
                page: page as u8,
                rect: [0.0, 0.0, 2.0, 4.0],
                uv: [0.0, 0.0, 0.25, 0.5],
            })
            .collect(),
    };
    let metadata_path = dir.join("testfont.sdf.ron");
    std::fs::write(&metadata_path, ron::to_string(&metadata).unwrap()).unwrap();
    std::fs::write(dir.join("testfont.sdf0.ktx2"), b"incomplete texture").unwrap();
    let ktx = bake::ktx2w::write_rgba8(1024, 1024, &[vec![255; 1024 * 1024 * 4]]);
    std::fs::write(dir.join("testfont.sdf1.ktx2"), ktx).unwrap();
    let mut art = HudArt::default();
    let mut pages = HashMap::new();
    add_distance_font(&mut art, &mut pages, &dir, name);
    assert_eq!(pages.len(), 1);
    assert_eq!(
        art.distance_fonts[name].glyphs,
        [metadata.glyphs[1].clone()]
    );
    std::fs::write(metadata_path, b"incomplete manifest").unwrap();
    let mut art = HudArt::default();
    add_distance_font(&mut art, &mut pages, &dir, name);
    assert!(art.distance_fonts.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

/// On foot, a launcher's dwell fills the reticle ring (BF2's state-3 heat),
/// so the lock box's fill survives the gauge's later send.
#[test]
fn a_launchers_dwell_fills_the_ring_on_foot() {
    let mut w = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
    let foe = w
        .sim
        .spawn(&core::unit::UnitSpec::trooper(
            1,
            glam::Vec3::new(0.0, 0.0, -30.0),
        ))
        .expect("foe");
    let mut c = core::weapon::WeaponClass::bf1_rifle();
    c.ordnance.kind = core::weapon::OrdKind::Missile;
    c.ordnance.missile = Some(core::missile::MissileClass::default());
    c.lock_time = 1.0;
    c.target_kind = 1;
    let wc = w.sim.add_weapon_class(c);
    let i = w.player.index as usize;
    w.sim.units[i].weapon_class = wc;
    assert_eq!(weapon_gauge(&w).ring, 0.0);
    w.sim.units[i].weapon.lock_target = Some(core::missile::MissileTarget::Unit(foe));
    w.sim.units[i].weapon.lock_held = 0.25;
    assert_eq!(weapon_gauge(&w).ring, 0.25);
}

#[test]
#[ignore = "requires a GPU adapter and the BF2 HUD bake"]
fn profile_picker_renders_native_menu_pages() {
    use ui::shell_ps2::{
        profiles::{Page, Picker, Target},
        ShellMenus,
    };

    let assets = crate::assets::baked_root();
    let mut menus = ShellMenus::new(&|_, fallback| fallback.to_owned());
    menus.covered = true;
    let mut picker = Picker::new(
        vec![(0, "Player 1".into()), (1, "Rebel Pilot".into())],
        Some(1),
    );
    let out = std::env::var_os("OPENBF_PROFILE_CAPTURE").map(std::path::PathBuf::from);

    for size in [[640, 480], [1280, 720], [640, 800]] {
        let mut r = render::Renderer::new_headless(size[0], size[1]).unwrap();
        let mut view = HudView::for_renderer(&assets, &r).unwrap();
        for (label, page, focus) in [
            ("list", Page::List, Target::Profile(1)),
            (
                "name",
                Page::Name {
                    text: "Rebel Pilot".into(),
                    symbols: false,
                    suggested: false,
                },
                Target::Key(0),
            ),
            (
                "difficulty",
                Page::Difficulty("Rebel Pilot".into()),
                Target::Difficulty(1),
            ),
            ("delete", Page::Delete(1), Target::No),
        ] {
            picker.page = page;
            picker.set_focus(focus);
            view.update_shell_profiles(&menus, false, &picker, size);
            assert!(
                view.drawn_sheets().count() >= 4,
                "{label} did not draw menu art"
            );
            r.render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, target, size| view.paint(dev, q, enc, target, size)),
            )
            .unwrap();
            let (rgba, w, h) = r.capture_rgba().unwrap();
            assert!(
                rgba.chunks_exact(4).filter(|p| p[0] > 120).count() > 700,
                "{label} was blank"
            );
            if let Some(dir) = &out {
                image::RgbaImage::from_raw(w, h, rgba)
                    .unwrap()
                    .save(dir.join(format!("profile-{label}-{w}x{h}.png")))
                    .unwrap();
            }
        }
    }
}

/// Issue #173: each view's reticle colour is its own slot's pick. A LAN
/// guest (slot 1) with nothing under his aim shows no relation while the
/// host's (slot 0) aim is on an enemy, and the host's view keeps its red.
#[test]
fn reticle_relation_reads_the_views_own_slot() {
    use core::sim::AimTarget;
    use core::unit::UnitHandle;
    let mut sim = core::sim::Sim::empty(1);
    let enemy = AimTarget::Unit(UnitHandle { index: 3, gen: 1 });
    sim.reticle_pick = vec![Some((enemy, -1)), None];
    assert_eq!(reticle_relation(&sim, 0), -1, "the host sees his enemy");
    assert_eq!(reticle_relation(&sim, 1), 0, "the guest has no pick");
    sim.reticle_pick[1] = Some((enemy, 1));
    assert_eq!(reticle_relation(&sim, 1), 1);
    assert_eq!(reticle_relation(&sim, 5), 0, "a slot past the table");
}
