//! Tier-2 visual golden: the spawn screen painted over the TAT1 GOLDEN_CAM
//! frame with a fixed SpawnModel (the HUD underneath — BF2's tree for a dead
//! player, with the line that says who killed him — and the death banner
//! suppressed, exactly as the game does it).
//!
//! Ignored by default — needs a GPU adapter AND baked assets_baked/tat1:
//!   cargo test -p game --test golden_spawn -- --ignored
//! Regenerate deliberately (review the PNG first):
//!   UPDATE_GOLDENS=1 cargo test -p game --test golden_spawn -- --ignored

mod common;

use game::viewer::{camera_from, upload_scene, GOLDEN_CAM};

fn hud() -> ui::hud::HudModel {
    ui::hud::HudModel {
        team_points: None,
        capital_ships: None,
        damage_flash: 0.0,
        gauge: Default::default(),
        leaving: None,
        team_names: ["all".into(), "imp".into()],
        tickets: [147, 152],
        bleed: [0.0; 2],
        hp: 0.0,
        hp_max: 300.0,
        hero: false,
        hero_offer: None,
        energy: 100.0,
        energy_max: 100.0,
        jet: None,
        weapon: None,
        weapon2: None,
        reticle: None,
        target: None,
        reticle_y: 0.5,
        // The spawn screen owns the death message while it is up.
        dead: false,
        respawn_in: None,
        outcome: None,
        countdown: None,
        round_end: None,
        time_left: None,
        messages: vec![ui::hud::HudMessage {
            text: "DX 178 killed Player 1".into(),
            color: ui::hud::message_color::OWN,
        }],
        stats: Some(ui::hud::HudStats::default()),
        player_team: 0,
        map: None,
        squad: None,
        capture: None,
        ambush: Vec::new(),
        hit_critical: None,
        hit_body: None,
        spawn: None,
        scope: None,
    }
}

fn spawn() -> ui::spawn::SpawnModel {
    ui::spawn::SpawnModel {
        team_names: ["all".into(), "imp".into()],
        player_team: 0,
        tickets: [147, 152],
        posts: vec![
            ui::spawn::SpawnPost {
                name: "cp1".into(),
                contested: false,
                distance: 42.0,
            },
            ui::spawn::SpawnPost {
                name: "cp3".into(),
                contested: true,
                distance: 118.0,
            },
        ],
        selected: Some(1),
        // The class row: the BF1 Alliance list a real TAT1 round fields, by
        // the disc's names (card SPSEL, `EventFeed::class_names`), the Pilot
        // chosen; the post line is the sim's spawn-map pick.
        classes: vec![
            "Rebel Soldier".into(),
            "Rebel Vanguard".into(),
            "Rebel Pilot".into(),
            "Rebel Marksman".into(),
            "Wookiee Smuggler".into(),
        ],
        selected_class: 2,
        countdown: Some(3.4),
        screen: None,
        page: None,
        icons: Default::default(),
    }
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/tat1"]
fn tat1_spawn_screen_matches_golden() {
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("tat1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake TAT1 first (see test header) — golden test can't skip silently");
    }
    let scene = game::assets::load_scene(&baked).expect("scene");
    let mut r = render::Renderer::new_headless(1280, 720).expect("headless renderer");
    let gpu = upload_scene(&mut r, &scene).expect("upload");
    let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
    let (h, s) = (hud(), spawn());
    // BF2's tree under the screen, as the game paints it for a dead player:
    // the reinforcement counts and the line that says who killed him.
    let mut tree = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r)
        .expect("the BF2 HUD bake (bake common)");
    let dead = ui::hud::HudModel {
        dead: true,
        ..h.clone()
    };
    tree.update(&dead, 0.0, [1280, 720]);
    let dead = ui::hud::HudModel {
        messages: Vec::new(),
        ..dead
    };
    tree.update(&dead, 0.6, [1280, 720]);
    let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
        ui::hud::draw_hud(ctx, &h);
        ui::spawn::draw_spawn(ctx, &s);
    });
    r.render_with_overlay(
        &camera_from(GOLDEN_CAM),
        &gpu.draws,
        gpu.clear,
        Some(&mut |dev, q, enc, view, size| {
            tree.paint(dev, q, enc, view, size);
            overlay.paint(&paint, dev, q, enc, view, size)
        }),
    )
    .expect("render");
    let (rgba, w, h) = r.capture_rgba().expect("capture");

    common::assert_golden(&common::goldens_dir().join("tat1_spawn.png"), &rgba, w, h);
}

/// The three PS2 spawn screens (card SSELB) as the HUD pass paints them on
/// Hoth's level start: each over the mission's camera shot its screen shows
/// on the PS2 (side 0, map 1, class 2: `lead_tmp/ssel/t1_hot1`), dimmed.
fn hot1_pages() -> [(&'static str, usize, ui::spawn_draw::SpawnPage); 3] {
    use ui::spawn_draw::SpawnPage;
    [
        (
            "side",
            0,
            SpawnPage::Side {
                title: "Select a team".into(),
                rows: vec!["Empire".into(), "Rebels".into(), "Auto Assign".into()],
                selected: 0,
                select: "Select".into(),
            },
        ),
        (
            "map",
            1,
            SpawnPage::Map {
                objective: "Capture all the Command Posts".into(),
                legend: [
                    "Command Post".into(),
                    "Change Command Post".into(),
                    "{btna} Select Command Post".into(),
                ],
            },
        ),
        (
            "class",
            2,
            SpawnPage::Class {
                title: "Snowtrooper".into(),
                lines: vec![
                    "Unit Count: 7".into(),
                    String::new(),
                    "Equipment:".into(),
                    "Blaster Rifle".into(),
                    "Blaster Pistol".into(),
                    "Thermal Detonator".into(),
                ],
                back: "Back".into(),
                accept: "Select Character".into(),
                team: 0,
                class: 0,
                classes: (0..6).collect(),
                // The battle's start: no points, so the officer (8) and
                // the dark trooper (12) are refused, black at places 0 / 1
                // as on the PS2's frame (re-spawn-flow §4.3).
                refused: 0b11_0000,
            },
        ),
    ]
}

/// Hoth's two armies as the spawn screens draw them: the Empire's six
/// classes in the mission's order (`missions/bf2_hot1_con.ron`), each with
/// its class's model and its primary's (the Rebels' are not shown).
fn hot1_teams() -> [game::walk::TeamTemplate; 2] {
    let mut empire = game::walk::TeamTemplate::default_trooper();
    let base = empire.classes[0].clone();
    empire.classes = [
        ("imp_inf_snowtrooper", "imp_weap_inf_rifle"),
        ("imp_inf_snowtrooper", "imp_weap_inf_launcher"),
        ("imp_inf_atatpilot", "imp_weap_inf_rifle"),
        ("imp_inf_snowtrooper", "imp_weap_inf_sniperrifle"),
        ("imp_inf_atatcommander", "imp_weap_inf_pistol"),
        ("imp_inf_darktrooper", "imp_weap_inf_rifle"),
    ]
    .into_iter()
    .map(|(body, gun)| game::walk::ClassVisual {
        geometry: Some(body.into()),
        weapon_geometry: Some(gun.into()),
        anim: game::animator::ClassAnim {
            family: "human".into(),
            root_scale: 1.0,
            banks: Vec::new(),
            custom: Default::default(),
        },
        ..base.clone()
    })
    .collect();
    [empire, game::walk::TeamTemplate::default_trooper()]
}

/// Paint `page` over hot1's camera shot `shot` at `size`, with its soldier
/// (card SSELB slice 3).
fn hot1_screen(
    scene: &game::assets::Scene,
    posts: &game::assets::Scene,
    shots: &[game::spawn_backdrop::CameraShot],
    shot: usize,
    page: &ui::spawn_draw::SpawnPage,
    size: [u32; 2],
) -> (Vec<u8>, u32, u32) {
    let mut r = render::Renderer::new_headless(size[0], size[1]).expect("headless renderer");
    let gpu = upload_scene(&mut r, scene).expect("upload");
    let mut tree = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r)
        .expect("the BF2 HUD bake (bake common)");
    // The map page's Spawn view: Hoth's posts, the ring on the second.
    // The owners as the PS2's frame has them after Auto Assign (the layer
    // starts every post on team 2; the mission hands them out at run
    // time): cp6 ("1") and cp3 ("2") the viewer's, in layer order cp5, cp6,
    // cp3, cp4, cp7.
    let map = matches!(page, ui::spawn_draw::SpawnPage::Map { .. }).then(|| {
        let mut map = game::minimap::spawn_still(posts, Some(1));
        let (f, e) = (ui::hud_map::FRIEND, ui::hud_map::ENEMY);
        for (p, c) in map.posts.iter_mut().zip([e, f, f, e, e]) {
            p.color = c;
        }
        map
    });
    let model = ui::hud::HudModel {
        dead: true,
        messages: Vec::new(),
        spawn: Some(page.clone()),
        map,
        ..hud()
    };
    tree.update(&model, 0.0, size);
    let cam = shots[shot].camera();
    let mut draws = gpu.draws.clone();
    let mut skinned = Vec::new();
    if game::spawn_preview::stands(page) {
        let vis = game::soldier_vis::SoldierVisuals::load(
            &mut r,
            &game::assets::baked_root(),
            "bf2",
            &hot1_teams(),
        );
        let (body, gun) =
            game::spawn_preview::Preview::default().draws(&vis, Some(page), &cam, 0.0);
        assert!(
            !body.is_empty() && !gun.is_empty(),
            "the Snowtrooper is baked"
        );
        skinned = body;
        draws.extend(gun);
    }
    r.render_frame(
        &cam,
        &draws,
        &skinned,
        gpu.clear,
        Some(&mut |dev, q, enc, view, size| {
            tree.paint(dev, q, enc, view, size);
        }),
    )
    .expect("render");
    r.capture_rgba().expect("capture")
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/bf2_hot1"]
fn hot1_spawn_screens_match_golden() {
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("bf2_hot1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake HOT1 first (see test header) - golden test can't skip silently");
    }
    let mission = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../missions/bf2_hot1_con.ron"
    );
    let shots = game::mission::MissionDef::load(std::path::Path::new(mission))
        .expect("hot1's mission")
        .camera_shots();
    let scene = game::assets::load_scene(&baked).expect("scene");
    // The map page's posts are the conquest layer's (the battle merges it;
    // the backdrop renders the base world, as before).
    let mut posts = game::assets::load_scene(&baked).expect("scene");
    posts.apply_mode_layer("con");
    // `SPAWN_PS2_OUT=<dir>`: also the 640 x 480 frames that lie over the
    // PS2's own (`lead_tmp/ssel/t1_hot1`, the 512 x 448 screen at 4:3).
    let ps2 = std::env::var_os("SPAWN_PS2_OUT").map(std::path::PathBuf::from);
    let mut failures = Vec::new();
    for (name, shot, page) in hot1_pages() {
        let (rgba, w, h) = hot1_screen(&scene, &posts, &shots, shot, &page, [1280, 720]);
        let golden = common::goldens_dir().join(format!("hot1_spawn_{name}.png"));
        if let Some(f) = common::check_golden(&golden, &rgba, w, h) {
            failures.push(f);
        }
        if let Some(dir) = &ps2 {
            let (rgba, w, h) = hot1_screen(&scene, &posts, &shots, shot, &page, [640, 480]);
            image::RgbaImage::from_raw(w, h, rgba)
                .expect("frame")
                .save(dir.join(format!("hot1_{name}_640.png")))
                .expect("write");
            // The side select on Auto Assign (no soldier): the frame his
            // pixels are told apart on, as the PS2's `s2_auto` is for its.
            if let ui::spawn_draw::SpawnPage::Side {
                title,
                rows,
                select,
                ..
            } = &page
            {
                let auto = ui::spawn_draw::SpawnPage::Side {
                    title: title.clone(),
                    rows: rows.clone(),
                    selected: 2,
                    select: select.clone(),
                };
                let (rgba, w, h) = hot1_screen(&scene, &posts, &shots, shot, &auto, [640, 480]);
                image::RgbaImage::from_raw(w, h, rgba)
                    .expect("frame")
                    .save(dir.join("hot1_side_auto_640.png"))
                    .expect("write");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
