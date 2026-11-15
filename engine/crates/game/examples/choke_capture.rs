//! Offscreen stills of a Force Choke victim (issue #173): one hero holds choke
//! on one stationary victim of a chosen class, and the victim and caster are
//! drawn side-on (and each up close) through the production frame path (`Fx::advance_frame`).
//!
//! Usage: choke_capture <baked-root> <map> <out-dir> <hero-class> <victim-game>
//! <victim-side> <victim-class> [native]
//!
//! `native` loads the victim from `side_<game>/locals` (Gungan, Ewok, Jawa,
//! Tusken, Geonosian). `prone` keeps the victim prone until the choke
//! catches him; `air` keeps him 3 m up in the hover jet until it does
//! (issue #173's stance rules; the stills follow his height). The victim stands 6 m in front of the caster on flat
//! diagnostic ground with 3000 HP; the caster has infinite stamina. Stills are
//! taken at choke ages near 0.3, 1.0 and 2.0 s, and 0.1 / 0.4 / 0.8 s after the
//! release. `<victim-class>.json` holds, per frame, the choke age and the drawn body's
//! lowest and highest vertex above the victim's feet (the lift).
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)
use anyhow::{anyhow, Result};
use core::sim::{buttons, PadState};
use game::{class_picker::ClassPicker, soldier_vis::SoldierVisuals, walk::WalkSim};
use glam::Vec3;
use render::{Camera, Renderer};
use std::path::Path;

const FPS: u32 = 60;
/// Ticks the trigger is held (from tick 6): 0.5 s wind-up plus 2.5 s hold.
const HOLD: std::ops::Range<u64> = 6..96;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let [_, root, map, out, hero, vgame, vside, vclass, options @ ..] = args.as_slice() else {
        anyhow::bail!(
            "usage: choke_capture <baked-root> <map> <out-dir> <hero-class> <victim-game> <victim-side> <victim-class> [native] [prone|air]"
        );
    };
    let native = options.iter().any(|s| s == "native");
    let prone = options.iter().any(|s| s == "prone");
    let air = options.iter().any(|s| s == "air");
    let root = Path::new(root);
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let (scene, _) = game::assets::load_battle_scene(&root.join(map), "con")?;
    let mut walk = WalkSim::new(198, Vec3::ZERO, None);
    walk.sim.ground_rules = if scene.manifest.source_game == "bf1" {
        core::soldier::GroundRules::Bf1
    } else {
        core::soldier::GroundRules::Bf2
    };
    walk.blend.on = true;
    walk.sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let db = game::roster::load_classdb(&root.join("classes"), "bf1", &["all"])?;
    let base = game::roster::ClassRoster::build(&db, "all_inf_soldier", 0, None)?
        .register(&mut walk.sim, &mut Vec::new());
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let mut picker = ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    let choice = picker
        .choices
        .iter()
        .position(|c| c.class.class == *hero)
        .ok_or_else(|| anyhow!("missing {hero}"))?;
    let vdb = if native {
        // A native side's classes: its own RON and the game's shared one.
        let mut db = game::classdb::ClassDb::default();
        for f in [
            format!("{vgame}_{vside}.ron"),
            if vgame == "bf2" {
                "bf2_ingame.ron".to_owned()
            } else {
                "bf1_common.ron".to_owned()
            },
        ] {
            let p = root.join("classes").join(f);
            if p.exists() {
                db.load(&p)?;
            }
        }
        db
    } else {
        game::roster::load_classdb(&root.join("classes"), vgame, &[vside.as_str()])?
    };
    let victim = game::roster::ClassRoster::build(&vdb, vclass, 0, None)?;
    let mut visual = game::walk::ClassVisual::from_roster(&victim);
    visual.from_bf2_side = vgame == "bf2" && scene.manifest.source_game != "bf2";
    let family = visual.anim.family.clone();
    if native {
        let mut template = walk.teams[1].clone();
        template.faction = vside.clone();
        template.geometry = victim.geometry.clone();
        template.classes = vec![visual];
        walk.native_teams.insert(1, template);
    } else {
        walk.teams[1].classes = vec![visual];
    }
    let mut renderer = Renderer::new_headless(640, 480).map_err(|e| anyhow!("{e}"))?;
    let mut vis = SoldierVisuals::for_walk(&mut renderer, root, &scene, &walk);
    assert!(picker.select(&mut walk, choice));
    for request in &picker.pending_assets {
        vis.append_test_class(&mut renderer, root, &walk, request);
    }
    let mut fx = game::fx::Fx::for_walk(root, &scene.dir, "bf2", &walk);
    fx.extend_test_classes(root, &scene.dir, "bf2", &walk);
    fx.preload(&mut renderer);

    let h = walk.player;
    walk.sim.cheats.infinite_stamina = true;
    let slot = walk
        .sim
        .unit(h)
        .unwrap()
        .held
        .iter()
        .position(|w| walk.sim.weapon_classes[w.class].name == "com_weap_inf_force_choke")
        .ok_or_else(|| anyhow!("{hero} holds no com_weap_inf_force_choke"))?;
    {
        let u = walk.sim.unit_mut(h).unwrap();
        u.cur1 = Some(slot as u8);
        for (i, held) in u.held.iter_mut().enumerate() {
            held.state.active = i == slot;
        }
        u.soldier = core::soldier::Soldier::new(Vec3::ZERO);
        u.soldier.energy = u.consts.energy_bar;
        u.weapon.melee = None;
    }
    let mut spec = core::unit::UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -6.0));
    spec.max_health = 3000.0;
    let target = walk
        .sim
        .spawn(&spec)
        .ok_or_else(|| anyhow!("victim did not spawn"))?;
    let ti = target.index as usize;

    // Choke ages and post-release times to keep a still at (seconds).
    let mut wanted_age = vec![0.3f32, 1.0, 2.0];
    let mut wanted_after = vec![0.1f32, 0.4, 0.8];
    let mut released: Option<f32> = None;
    let mut rows = Vec::new();
    let start = walk.sim.tick;
    let mut frame = 0u32;
    let dt = 1.0 / FPS as f32;
    while walk.sim.tick < start + 150 {
        let tick = walk.sim.tick - start;
        let pad = PadState {
            buttons: if HOLD.contains(&tick) {
                buttons::SECONDARY_FIRE
            } else {
                0
            },
            ..Default::default()
        };
        let was_choked = walk.sim.units[ti].choked.is_some();
        // Until the choke first catches him, keep the asked pose.
        if !was_choked && released.is_none() && (prone || air) {
            let v = &mut walk.sim.units[ti].soldier;
            if prone {
                v.stance = core::soldier::Stance::Prone;
            } else {
                v.stance = core::soldier::Stance::Hover;
                v.grounded = false;
                v.pos = Vec3::new(0.0, 3.0, -6.0);
                v.vel = Vec3::ZERO;
                v.fuel = 1.0;
            }
        }
        walk.advance_with(dt, &[pad], |w| {
            vis.note_shots(w);
            fx.ingest(w);
        });
        fx.advance_frame(Some(&mut vis), &mut walk, &pad, dt);
        walk.blend_in();
        let v = &walk.sim.units[ti];
        let age = v.choked.map(|c| c.age);
        if was_choked && age.is_none() && released.is_none() {
            released = Some(0.0);
        } else if let Some(r) = &mut released {
            *r += dt;
        }
        let feet = v.soldier.pos;
        let mesh = vis.drawn_body(&walk, ti).map(|(tris, _)| {
            let ys = tris.iter().flatten().map(|p| p.y - feet.y);
            let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
            (lo, hi)
        });
        let caster = walk.sim.unit(h).unwrap();
        rows.push(serde_json::json!({
            "frame": frame, "tick": walk.sim.tick, "choke_age": age, "after_release": released,
            "victim_stance": format!("{:?}", v.soldier.stance), "victim_feet_y": feet.y,
            "mesh_low": mesh.map(|m| m.0), "mesh_high": mesh.map(|m| m.1),
            "victim_body": format!("{:?}", vis.units[ti].animator.body()),
            "caster_state": format!("{:?}", caster.held.get(slot).map(|w| w.state.state)),
        }));
        let mut shots: Vec<String> = Vec::new();
        if let Some(a) = age {
            if let Some(k) = wanted_age.iter().position(|w| a >= *w) {
                shots.push(format!("age{:.1}", wanted_age.remove(k)));
            }
        }
        if let Some(r) = released {
            if let Some(k) = wanted_after.iter().position(|w| r >= *w) {
                shots.push(format!("release{:.1}", wanted_after.remove(k)));
            }
        }
        if tick == 3 {
            shots.push("before".into());
        }
        let up = Vec3::Y * walk.sim.units[ti].soldier.pos.y;
        for shot in shots {
            for (view, eye, focus) in [
                (
                    "side",
                    Vec3::new(8.5, 1.3, -3.0) + up,
                    Vec3::new(0.0, 1.0, -3.0) + up,
                ),
                (
                    "close",
                    Vec3::new(3.2, 1.2, -6.0) + up,
                    Vec3::new(0.0, 1.1, -6.0) + up,
                ),
                ("caster", Vec3::new(2.4, 1.4, 0.4), Vec3::new(0.0, 1.1, 0.0)),
            ] {
                let d = focus - eye;
                let camera = Camera {
                    position: eye,
                    yaw: (-d.x).atan2(-d.z),
                    pitch: d.y.atan2(Vec3::new(d.x, 0.0, d.z).length()),
                    ..Default::default()
                };
                let body = vis.draws(&walk);
                let weapons = vis.weapon_draws(&walk);
                fx.submit_with(&mut renderer, &camera, vis.saber_draws(&walk));
                renderer
                    .render_frame(&camera, &weapons, &body, [0.13, 0.15, 0.18, 1.0], None)
                    .map_err(|e| anyhow!("{e}"))?;
                let (rgba, w, hgt) = renderer.capture_rgba().map_err(|e| anyhow!("{e}"))?;
                std::fs::write(
                    out.join(format!("{vclass}_{shot}_{view}.png")),
                    render::encode_png_rgb(&rgba, w, hgt).map_err(|e| anyhow!("{e}"))?,
                )?;
            }
        }
        walk.blend_out();
        frame += 1;
    }
    std::fs::write(
        out.join(format!("{vclass}.json")),
        serde_json::to_vec_pretty(&serde_json::json!({
            "build": env!("OPENBF_BUILD_COMMIT"), "dirty": env!("OPENBF_BUILD_DIRTY"),
            "map": map, "hero": hero, "victim": vclass, "victim_game": vgame, "victim_side": vside,
            "family": family, "native": native, "prone": prone, "air": air, "fps": FPS,
            "intervention": "flat diagnostic ground; infinite caster stamina; stationary 3000 HP victim 6 m ahead",
            "frames": rows,
        }))?,
    )?;
    Ok(())
}
