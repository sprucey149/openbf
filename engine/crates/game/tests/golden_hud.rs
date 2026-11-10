//! Tier-2 visual golden: the HUD — BF2's own tree (`game::hud_feed`), with
//! two event lines in its message box, the statistics pop-up after a kill,
//! an enemy's name and health bar at the top (card HTGT),
//! BF1's squad order cross half a second after FOLLOW ME took two units
//! (card SQHUD), and the egui lines over it — painted over the TAT1 GOLDEN_CAM frame
//! with a fixed HudModel, perceptual-diffed against goldens/tat1_hud.png;
//! and the hit-direction chevrons (card HATK) against goldens/hud_ambush.png.
//!
//! Ignored by default — needs a GPU adapter AND baked assets_baked/tat1:
//!   cargo test -p game --test golden_hud -- --ignored
//! Regenerate deliberately (review the PNG first):
//!   UPDATE_GOLDENS=1 cargo test -p game --test golden_hud -- --ignored

mod common;

use game::viewer::{camera_from, upload_scene, GOLDEN_CAM};

fn model() -> ui::hud::HudModel {
    ui::hud::HudModel {
        ctf: false,
        team_points: None,
        lock: None,
        hints: Vec::new(),
        seat_health: None,
        incoming_missile: None,
        space: None,
        damage_flash: 0.0,
        gauge: Default::default(),
        leaving: None,
        team_names: ["all".into(), "imp".into()],
        tickets: [247, 239],
        bleed: [0.0; 2],
        hp: 220.0,
        hp_max: 300.0,
        energy: 64.0,
        energy_max: 100.0,
        jet: None,
        hero: false,
        hero_offer: None,
        weapon: Some(ui::hud::HudWeapon {
            key: "all_weap_inf_rifle".into(),
            class: String::new(),
            icon: None,
            name: String::new(),
            total: Some(281),
            clip_fraction: 31.0 / 50.0,
            bf2: true,
        }),
        weapon2: None,
        // An enemy at 60 % under the reticle: the target readout (card HTGT).
        reticle: Some(-1),
        target: Some(ui::hud::HudTarget {
            id: 1,
            name: "Stormtrooper".into(),
            health_fraction: 0.6,
            relation: -1,
            team0: false,
            shield: None,
        }),
        reticle_y: game::walk::ps2_reticle_y("bf1"),
        dead: false,
        respawn_in: None,
        outcome: None,
        countdown: None,
        round_end: None,
        time_left: None,
        messages: Vec::new(),
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

/// FOLLOW ME, two followers, half a second after the press.
fn cross() -> ui::hud_squad::SquadCross {
    ui::hud_squad::SquadCross {
        labels: ["MOVE OUT", "HOLD POSITION", "FOLLOW ME", "AT EASE"].map(str::to_owned),
        arm: ui::hud_squad::RIGHT,
        count: Some(2),
        timer: ui::hud_squad::SquadCross::timer_at(0.5).expect("shown"),
    }
}

/// The event lines of the golden: an enemy's capture, the player's death as
/// the PS2 printed it on HOT1 (`engine/artifacts/pcsx2/ref/evmsg/`).
fn messages() -> Vec<ui::hud::HudMessage> {
    use ui::hud::message_color::{ENEMY, OWN};
    [
        ("Empire captured the Homestead", ENEMY),
        ("Ben Pulastra killed Player 1", OWN),
    ]
    .into_iter()
    .map(|(text, color)| ui::hud::HudMessage {
        text: text.into(),
        color,
    })
    .collect()
}

#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/tat1"]
fn tat1_hud_matches_golden() {
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("tat1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake TAT1 first (see test header) — golden test can't skip silently");
    }
    let scene = game::assets::load_scene(&baked).expect("scene");
    let mut r = render::Renderer::new_headless(1280, 720).expect("headless renderer");
    let gpu = upload_scene(&mut r, &scene).expect("upload");
    let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), render::HEADLESS_FORMAT);
    // The minimap as the golden camera sees the map: its posts as authored.
    let cam = camera_from(GOLDEN_CAM);
    let m = ui::hud::HudModel {
        map: Some(game::minimap::still(&scene, cam.position, cam.yaw)),
        squad: Some(cross()),
        ..model()
    };
    let mut hud = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r)
        .expect("the BF2 HUD bake (bake common)");
    // The first update settles the fades (the statistics pop-up of the
    // battle's start is gone again); then the lines arrive and a kill and
    // its point are scored, and the next frames admit the lines one by one.
    hud.update(&m, 0.0, [1280, 720]);
    let scored = ui::hud::HudModel {
        messages: messages(),
        stats: Some(ui::hud::HudStats {
            points: 1,
            kills: 1,
            ..Default::default()
        }),
        ..m.clone()
    };
    hud.update(&scored, 0.0, [1280, 720]);
    let m = ui::hud::HudModel {
        messages: Vec::new(),
        ..scored
    };
    // Three lines: the capture breaks at the box's edge.
    for _ in 0..3 {
        hud.update(&m, 0.6, [1280, 720]);
    }
    let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| ui::hud::draw_hud(ctx, &m));
    r.render_with_overlay(
        &camera_from(GOLDEN_CAM),
        &gpu.draws,
        gpu.clear,
        Some(&mut |dev, q, enc, view, size| {
            hud.paint(dev, q, enc, view, size);
            overlay.paint(&paint, dev, q, enc, view, size)
        }),
    )
    .expect("render");
    let (rgba, w, h) = r.capture_rgba().expect("capture");

    common::assert_golden(&common::goldens_dir().join("tat1_hud.png"), &rgba, w, h);
}

/// Card HATK: the hit-direction chevrons over the HUD (`ui::hud_ambush`,
/// `analysis/re-hud-attacker.md`) - the right and bottom ones half a second
/// into their fade, the left and top ones just lit - perceptual-diffed
/// against goldens/hud_ambush.png. View it beside the PS2's frames,
/// `engine/artifacts/lead_tmp/hatk/shots/edges.png`.
#[test]
#[ignore = "requires a GPU adapter + baked assets_baked/tat1"]
fn tat1_hud_ambush_matches_golden() {
    use ui::hud_ambush::Edge;
    common::force_ps2_tier();
    let baked = game::assets::baked_root().join("tat1");
    if !baked.join("manifest.ron").exists() {
        panic!("bake TAT1 first (see test header) — golden test can't skip silently");
    }
    let scene = game::assets::load_scene(&baked).expect("scene");
    let mut r = render::Renderer::new_headless(1280, 720).expect("headless renderer");
    let gpu = upload_scene(&mut r, &scene).expect("upload");
    let m = model();
    let mut hud = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r)
        .expect("the BF2 HUD bake (bake common)");
    // The first update settles the fades; then two hits, half a second,
    // two more.
    hud.update(&m, 0.0, [1280, 720]);
    let hit = |edges: &[Edge]| ui::hud::HudModel {
        ambush: edges.to_vec(),
        ..m.clone()
    };
    hud.update(&hit(&[Edge::Right, Edge::Bottom]), 0.0, [1280, 720]);
    hud.update(&m, 0.5, [1280, 720]);
    hud.update(&hit(&[Edge::Left, Edge::Top]), 0.0, [1280, 720]);
    r.render_with_overlay(
        &camera_from(GOLDEN_CAM),
        &gpu.draws,
        gpu.clear,
        Some(&mut |dev, q, enc, view, size| hud.paint(dev, q, enc, view, size)),
    )
    .expect("render");
    let (rgba, w, h) = r.capture_rgba().expect("capture");

    common::assert_golden(&common::goldens_dir().join("hud_ambush.png"), &rgba, w, h);
}

/// The player flying a remote droid, on a BF1 and a BF2 map, in the
/// droid's third-person chase camera with the models drawn (the droid, the
/// standing owner, a team mate's and an enemy's droid and their owners) and
/// the HUD: BF2's blaster and destruct meshes in the weapon rows on both
/// maps, the droid's time left in the stamina bar, and the minimap centred
/// on the droid with the droid icons and the owner's body. Not a golden: it
/// writes `engine/artifacts/lead_tmp/droidcam/<map>.png` to look at.
#[test]
#[ignore = "requires a GPU adapter + the baked maps"]
fn droid_hud_picture() {
    use core::sim::{buttons, PadState};
    use core::unit::Controller;
    use glam::Vec3;
    common::force_ps2_tier();
    let root = game::assets::baked_root();
    let out = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/lead_tmp/droidcam"
    ));
    std::fs::create_dir_all(&out).unwrap();
    for (map, game, class, sides) in [
        ("bf2_nab2", "bf2", "rep_inf_ep3_officer", ["rep", "cis"]),
        ("bf1_tat2", "bf1", "all_inf_marksman", ["all", "imp"]),
    ] {
        let mut scene = game::assets::load_scene(&root.join(map)).expect("scene");
        scene.apply_mode_layer("con");
        let db = game::roster::load_classdb(&root.join("classes"), game, &sides).unwrap();
        let roster = game::roster::ClassRoster::build(&db, class, 1, None).unwrap();
        let mut sim = core::sim::Sim::empty(42);
        scene.install_collision(&mut sim);
        let uc = roster.register(&mut sim, &mut Vec::new());
        let mut visual = game::walk::ClassVisual::from_roster(&roster);
        visual.held_hud = roster
            .held
            .iter()
            .map(|h| (h.slot, h.hud.clone()))
            .collect();
        let faction = game::roster::FactionRoster {
            faction: sides[0].into(),
            classes: vec![roster.clone()],
        };
        let t = game::walk::TeamTemplate::from_roster(&faction, &uc, vec![visual]);
        let mut enemy = t.clone();
        enemy.faction = sides[1].into();
        let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
        let at = layout.cps.first().map_or(Vec3::ZERO, |c| c.pos) + Vec3::new(6.0, 0.0, 6.0);
        let mut walk = game::walk::WalkSim::with_teams(
            sim,
            scene.heightfield(),
            Vec3::new(at.x, 0.0, at.z),
            [t.clone(), enemy.clone()],
            None,
        );
        walk.add_posts(&layout);
        // The player launches his droid and flies it forward a second.
        let owner = walk.player;
        let slot = roster
            .held
            .iter()
            .find(|h| h.weapon.remote.is_some())
            .unwrap()
            .slot;
        let u = &mut walk.sim.units[owner.index as usize];
        for h in &mut u.held {
            h.state.active = h.slot == slot;
        }
        u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
        let pad = |buttons: u32, ly: i16| PadState {
            buttons,
            ly,
            ..Default::default()
        };
        for _ in 0..30 {
            walk.sim.step(&[pad(buttons::SECONDARY_FIRE, 0)]);
        }
        for _ in 0..30 {
            walk.sim.step(&[pad(0, i16::MAX / 2)]);
        }
        let mine = walk.sim.controlled_droid(owner).expect("flying").clone();
        // A team mate's and an enemy's droid near it, for the colours.
        let remote = walk.sim.weapon_classes[mine.weapon_class]
            .remote
            .clone()
            .unwrap();
        for (k, team, off) in [
            (0u64, 0u8, Vec3::new(-8.0, 0.0, -6.0)),
            (1, 1, Vec3::new(7.0, 0.0, -9.0)),
        ] {
            let tpl = if team == 0 { &t } else { &enemy };
            let bot = walk
                .sim
                .spawn(&tpl.spec(
                    team,
                    mine.pos + off + Vec3::new(0.0, 0.0, 3.0),
                    0.0,
                    Controller::Ai,
                    0,
                ))
                .unwrap();
            let d = core::remote::Droid::new(
                1000 + k,
                bot,
                team,
                mine.weapon_class,
                &remote,
                mine.pos + off,
                Vec3::ZERO,
                0.7,
                false,
            );
            walk.sim.droids.push(d);
        }
        let statics =
            game::minimap::MapStatic::load(&scene, &root.join("classes"), &walk, &[], None);
        let [weapon, weapon2] = game::hud_feed::weapon_rows(&walk);
        let class = walk.sim.droid_class(&mine).unwrap();
        let m = ui::hud::HudModel {
            team_names: [sides[0].into(), sides[1].into()],
            tickets: [150, 150],
            hp: mine.health,
            hp_max: class.max_health,
            energy: mine.time_left(),
            energy_max: core::remote::LIFETIME,
            weapon,
            weapon2,
            reticle: Some(0),
            reticle_y: walk.reticle_y(),
            stats: Some(ui::hud::HudStats::default()),
            map: Some(statics.model(&walk)),
            ..model()
        };
        let m = ui::hud::HudModel { target: None, ..m };
        let mut r = render::Renderer::new_headless(1280, 720).expect("headless renderer");
        let gpu = upload_scene(&mut r, &scene).expect("upload");
        let mut hud = game::hud_feed::HudView::for_renderer(&root, &r).expect("the BF2 HUD bake");
        hud.update(&m, 0.0, [1280, 720]);
        hud.update(&m, 1.0, [1280, 720]);
        for name in ["player1weapon1mesh", "player1weapon2mesh", "player1map"] {
            println!("{map} {name}: {:?}", hud.bounds(name));
        }
        // The models: the standing owner, the bots and every droid's own.
        let mut vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
        for _ in 0..10 {
            vis.update(&walk, core::TICK_DT);
        }
        let mut draws = gpu.draws.clone();
        let props = vis.weapon_draws(&walk);
        // The flown droid's own model draws, in front of the camera and on
        // screen (in first person it could have sat behind the near plane).
        let cam = walk.camera();
        let at = |m: &glam::Mat4| m.transform_point3(Vec3::ZERO);
        let own: Vec<_> = props
            .iter()
            .filter(|d| at(&d.model).distance(mine.pos) < 1e-3)
            .collect();
        assert!(!own.is_empty(), "{map}: the flown droid's model draws");
        let c = cam.view_proj(1280.0 / 720.0) * (mine.pos + Vec3::Y * 0.15).extend(1.0);
        let (x, y) = (c.x / c.w, c.y / c.w);
        assert!(
            c.w > 1.0 && x.abs() < 0.2 && (-0.9..0.0).contains(&y),
            "{map}: ({x}, {y})"
        );
        draws.extend(props);
        let skinned = vis.draws(&walk);
        println!(
            "{map}: camera {:?} droid {:?} ({:.2} m), droid on screen at ({x:.2}, {y:.2})",
            cam.position,
            mine.pos,
            cam.position.distance(mine.pos)
        );
        r.render_frame(
            &cam,
            &draws,
            &skinned,
            gpu.clear,
            Some(&mut |dev, q, enc, view, size| hud.paint(dev, q, enc, view, size)),
        )
        .expect("render");
        let (rgba, w, h) = r.capture_rgba().expect("capture");
        let png = render::encode_png_rgb(&rgba, w, h).expect("png");
        std::fs::write(out.join(format!("{map}.png")), png).unwrap();
    }
}

/// Issue #72: render the real BF2 jet bar offscreen; no golden is changed.
#[test]
#[ignore = "requires GPU and the baked BF2 HUD"]
fn jet_fuel_bar_draws_and_disables_offscreen() {
    common::force_ps2_tier();
    let mut r = render::Renderer::new_headless(640, 480).unwrap();
    let mut hud = game::hud_feed::HudView::for_renderer(&game::assets::baked_root(), &r).unwrap();
    let mut m = ui::hud::HudModel {
        jet: Some((1.0, 0.24)),
        ..model()
    };
    let out = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../artifacts/lead_tmp/sprint_jet"
    ));
    std::fs::create_dir_all(out).unwrap();
    hud.update(
        &ui::hud::HudModel {
            jet: None,
            ..m.clone()
        },
        0.0,
        [640, 480],
    );
    let mut shots = Vec::new();
    for (label, jet) in [
        ("full", Some((1.0, 0.24))),
        ("low", Some((0.12, 0.24))),
        ("off", None),
    ] {
        m.jet = jet;
        hud.update(&m, 0.0, [640, 480]);
        for _ in 0..if jet.is_some() { 2 } else { 20 } {
            hud.update(&m, 0.1, [640, 480]);
        }
        eprintln!(
            "{label} fill {:?} threshold {:?}",
            hud.bounds("player1jetpack_fill"),
            hud.bounds("player1jetpack_threshold")
        );
        r.render_with_overlay(
            &camera_from(GOLDEN_CAM),
            &[],
            [0.03, 0.03, 0.03, 1.0],
            Some(&mut |dev, q, enc, view, size| hud.paint(dev, q, enc, view, size)),
        )
        .unwrap();
        let (rgba, w, h) = r.capture_rgba().unwrap();
        std::fs::write(
            out.join(format!("jet_{label}.png")),
            render::encode_png(&rgba, w, h).unwrap(),
        )
        .unwrap();
        shots.push(rgba);
    }
    let changed = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
    };
    assert!(
        changed(&shots[0], &shots[1]) > 100,
        "full and low fuel must draw differently"
    );
    assert!(
        changed(&shots[1], &shots[2]) > 100,
        "disable must remove the bar"
    );
}
