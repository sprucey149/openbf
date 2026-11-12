//! The class screen's real soldiers against its panel: every class of both
//! teams in front, still and through a slide, at each aspect and under both
//! cameras, drawn alone on black; no lit pixel may reach the panel's left edge
//! or the window's. BF1 and BF2 rosters, droidekas and heroes included.
//!
//! `capture_the_class_screen` paints the real page over the mission's first
//! camera shot at each aspect, for a person to look at:
//!   SPAWN_CAPTURE_OUT=<dir> cargo test -p game --test suite capture_the_class_screen -- --ignored

use game::spawn_preview::Preview;
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;
use render::{Camera, Renderer};
use std::path::Path;
use ui::spawn_draw::SpawnPage;

fn fixture(root: &Path, map: &str) -> (WalkSim, game::assets::Scene, game::mission::MissionDef) {
    let space = map.starts_with("bf2_spa");
    let mode = if space { "ass" } else { "con" };
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), mode).unwrap();
    let missions = root.parent().unwrap().join("missions");
    let mission = [format!("{map}_{mode}_cw.ron"), format!("{map}_{mode}.ron")]
        .into_iter()
        .map(|f| missions.join(f))
        .find(|p| p.exists())
        .and_then(|p| game::mission::MissionDef::load(&p).ok())
        .unwrap();
    let (mut rosters, _) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        &map[..3],
        [
            mission.teams[0].faction.as_str(),
            mission.teams[1].faction.as_str(),
        ],
        Some(&mission),
    )
    .unwrap();
    game::droideka::install(&mut rosters, &scene.dir, &map[..3]);
    let mut heroes = [None; 2];
    for (t, roster) in rosters.iter_mut().enumerate() {
        heroes[t] = game::heroes::append(roster, &root.join("classes"), Some(&mission), t);
    }
    let mut sim = core::sim::Sim::empty(42);
    let mut names = Vec::new();
    let mut team = 0;
    let teams = rosters.map(|roster| {
        let classes: Vec<_> = roster
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        let visuals = roster
            .classes
            .iter()
            .map(ClassVisual::from_roster)
            .collect();
        let template = TeamTemplate::from_roster(&roster, &classes[0], visuals);
        sim.classes[team] = classes;
        team += 1;
        template
    });
    game::heroes::install(&mut sim, heroes);
    let walk = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, None);
    (walk, scene, mission)
}

#[test]
#[ignore = "owned Space Felucia models and headless GPU"]
fn republic_preview_is_stable_at_the_space_felucia_camera() {
    let root = std::env::var_os("SPAWN_PREVIEW_ASSETS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(game::assets::baked_root);
    let (walk, scene, mission) = fixture(&root, "bf2_spa7");
    let mut r = Renderer::new_headless(1920, 1126).unwrap();
    r.cull = true;
    let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
    assert_eq!(walk.teams[0].faction, "rep");
    assert_eq!(walk.teams[0].classes.len(), 2);
    let camera = mission.camera_shots()[3].camera();
    let centered = Camera {
        position: Vec3::ZERO,
        ..camera
    };
    for class in 0..2 {
        let page = page(0, class, 2);
        let mut render = |camera: &Camera| {
            let (body, weapons) = Preview::default().draws_live(
                &mut r,
                &vis,
                Some(&page),
                camera,
                0.0,
                |_, _, _, _, _| None,
            );
            assert!(!body.is_empty() && !weapons.is_empty());
            r.render_frame(camera, &weapons, &body, [0.0, 0.0, 0.0, 1.0], None)
                .unwrap();
            r.capture_rgba().unwrap().0
        };
        let near = render(&centered);
        let far = render(&camera);
        assert!(near.chunks_exact(4).filter(|p| p[..3] != [0; 3]).count() > 1000);
        let changed = near
            .chunks_exact(4)
            .zip(far.chunks_exact(4))
            .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(a, b)| a.abs_diff(*b) > 8))
            .count();
        println!("Republic class {class}: {changed} changed pixels from camera translation");
        assert_eq!(changed, 0, "the same close model and pose should be stable");
    }
}

fn page(team: usize, class: usize, n: usize) -> SpawnPage {
    SpawnPage::Class {
        title: String::new(),
        lines: Vec::new(),
        back: String::new(),
        accept: String::new(),
        team: team as u8,
        class,
        classes: (0..n as u8).collect(),
        refused: 0,
    }
}

#[test]
#[ignore = "requires a GPU adapter and the baked maps; writes PNGs and compares nothing"]
fn capture_the_class_screen() {
    let out = std::path::PathBuf::from(std::env::var_os("SPAWN_CAPTURE_OUT").expect("the out dir"));
    let root = game::assets::baked_root();
    for (map, team) in [
        ("bf1_bes1", 0),
        ("bf1_bes1", 1),
        ("bf2_kam1", 1),
        ("bf2_nab2", 0),
    ] {
        let (walk, scene, mission) = fixture(&root, map);
        for (w, h) in [(640, 480), (1280, 720), (1574, 805), (1680, 720)] {
            let mut r = Renderer::new_headless(w, h).unwrap();
            let gpu = game::viewer::upload_scene(&mut r, &scene).unwrap();
            let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
            let mut dekas = game::viewer::DroidekaVisuals::load(&mut r, &scene, &root, &walk);
            let mut hud = game::hud_feed::HudView::for_renderer(&root, &r).unwrap();
            let cam = mission.camera_shots().first().unwrap().camera();
            let n = walk.sim.classes[team].len();
            let refused = (0..n.min(32))
                .filter(|&c| walk.sim.class_refusal(0, team as u8, c).is_some())
                .fold(0u32, |m, c| m | 1 << c);
            let p = SpawnPage::Class {
                title: walk.teams[team].classes[0].name.clone(),
                lines: vec!["Unit Count: 6".into(), String::new(), "Equipment:".into()],
                back: "Back".into(),
                accept: "Select Character".into(),
                team: team as u8,
                class: 0,
                classes: (0..n as u8).collect(),
                refused,
            };
            let (body, models) = Preview::default().draws_live(
                &mut r,
                &vis,
                Some(&p),
                &cam,
                0.0,
                |r, t, c, model, dt| {
                    let d = walk
                        .sim
                        .classes
                        .get(usize::from(t))?
                        .get(usize::from(c))?
                        .droideka
                        .as_deref()?;
                    Some(dekas.preview_draws(r, t, c, d, model, dt))
                },
            );
            let mut draws = gpu.draws.clone();
            draws.extend(models);
            let model = ui::hud::HudModel {
                dead: true,
                spawn: Some(p),
                ..Default::default()
            };
            hud.update(&model, 0.0, [w, h]);
            r.render_frame(
                &cam,
                &draws,
                &body,
                gpu.clear,
                Some(&mut |d, q, e, v, s| hud.paint(d, q, e, v, s)),
            )
            .unwrap();
            let (rgba, cw, ch) = r.capture_rgba().unwrap();
            let file = out.join(format!("{map}_t{team}_{w}x{h}.png"));
            std::fs::write(file, render::encode_png(&rgba, cw, ch).unwrap()).unwrap();
        }
    }
}

/// A class screen opens on every death: its outlines are measured at load, not
/// on its first frame (27 to 71 ms a team when they were, on the baked maps).
#[test]
#[ignore = "requires a GPU adapter and the baked BF2 maps"]
fn the_class_screen_does_not_measure_on_opening() {
    let root = game::assets::baked_root();
    let (walk, scene, _) = fixture(&root, "bf2_kas2");
    let mut r = Renderer::new_headless(640, 360).unwrap();
    let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
    for team in 0..2u8 {
        let start = std::time::Instant::now();
        let some = (0..walk.sim.classes[usize::from(team)].len() as u8)
            .filter(|&c| vis.still_outline(team, c).is_some())
            .count();
        assert!(some > 0, "team {team} has soldiers");
        let took = start.elapsed().as_secs_f32() * 1e3;
        assert!(took < 5.0, "team {team}: {took} ms");
    }
}

/// The lit pixels' least and most x.
fn lit_span(rgba: &[u8], w: u32) -> Option<(u32, u32)> {
    let mut out: Option<(u32, u32)> = None;
    for (i, px) in rgba.chunks_exact(4).enumerate() {
        if px[0] | px[1] | px[2] > 4 {
            let x = i as u32 % w;
            out = Some(out.map_or((x, x), |(a, b)| (a.min(x), b.max(x))));
        }
    }
    out
}

#[test]
#[ignore = "requires a GPU adapter and the baked BF1/BF2 maps"]
fn the_real_line_up_stays_clear_of_the_panel() {
    let root = game::assets::baked_root();
    let shot = 2.0 * core::fmath::atan2(0.34131, 1.0);
    let mut worst = 0.0f32;
    for map in ["bf1_bes1", "bf1_nab1", "bf2_kam1", "bf2_nab2", "bf2_kas2"] {
        let (walk, scene, _) = fixture(&root, map);
        for (w, h) in [(480, 360), (640, 360), (720, 368), (840, 360)] {
            let mut r = Renderer::new_headless(w, h).unwrap();
            let vis = game::soldier_vis::SoldierVisuals::for_walk(&mut r, &root, &scene, &walk);
            let mut dekas = game::viewer::DroidekaVisuals::load(&mut r, &scene, &root, &walk);
            let panel = ui::spawn_draw::window_point(
                [ui::spawn_draw::CLASS_BOX[0], 0.0],
                [w as f32, h as f32],
            )[0];
            for fov_y in [shot, walk.fov_y()] {
                let cam = Camera {
                    fov_y,
                    ..Default::default()
                };
                for team in 0..2 {
                    let n = walk.sim.classes[team].len();
                    for pick in 0..n {
                        let mut preview = Preview::default();
                        // Still, then a quarter and a half of the way to the next.
                        for (class, dt) in [
                            (pick, 0.0),
                            ((pick + 1) % n, 0.083),
                            ((pick + 1) % n, 0.083),
                        ] {
                            let p = page(team, class, n);
                            let (body, models) = preview.draws_live(
                                &mut r,
                                &vis,
                                Some(&p),
                                &cam,
                                dt,
                                |r, t, c, model, dt| {
                                    let d = walk
                                        .sim
                                        .classes
                                        .get(usize::from(t))?
                                        .get(usize::from(c))?
                                        .droideka
                                        .as_deref()?;
                                    Some(dekas.preview_draws(r, t, c, d, model, dt))
                                },
                            );
                            r.render_frame(&cam, &models, &body, [0.0, 0.0, 0.0, 1.0], None)
                                .unwrap();
                            let (rgba, _, _) = r.capture_rgba().unwrap();
                            let (lo, hi) = lit_span(&rgba, w).expect("soldiers drawn");
                            let name = &walk.teams[team].classes[pick].name;
                            assert!(
                                (hi as f32) < panel,
                                "{map} {w}x{h} fov {fov_y} {name}: lit to {hi}, panel at {panel}"
                            );
                            assert!(lo > 0, "{map} {w}x{h} fov {fov_y} {name}: off the left");
                            worst = worst.max(hi as f32 / panel);
                        }
                    }
                }
            }
        }
    }
    println!("nearest the panel: {worst:.3} of its left edge");
}
