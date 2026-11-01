//! Card JETP2: a soldier in a jet stance carries his class's `JetEffect`;
//! it stops when the jet ends or he dies, and a new jet spawns a new one.
use super::*;
use core::soldier::Stance;
use core::unit::UnitSpec;

fn library(game: &str, faction: &str) -> Option<Library> {
    let root = crate::assets::baked_root();
    let common = root.join(format!("common_{game}"));
    if !common.join("effects.ron").exists() {
        eprintln!("SKIP: {} is not baked", common.display());
        return None;
    }
    Some(Library::load(&root, &common, game, &[faction]))
}

#[test]
fn a_jet_carries_its_effect_on_the_back_and_stops_it_at_the_end() {
    for (game, faction, name) in [
        ("bf2", "rep", "rep_sfx_jetpack"),
        ("bf2", "imp", "imp_sfx_jetpack_darktrooper"),
        ("bf1", "rep", "jetpack"),
        ("bf1", "imp", "darktrooper_jetpack"),
    ] {
        let Some(lib) = library(game, faction) else {
            continue;
        };
        assert!(lib.get(name).is_some(), "{game} {name} baked");
        let mut fx = Fx::with_library(lib);
        let mut sim = Sim::empty(2);
        let h = sim
            .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::new(4.0, 10.0, -2.0)))
            .unwrap();
        let look = |_: &core::unit::Unit| Some(name);
        let set = |sim: &mut Sim, stance: Stance| {
            sim.units[h.index as usize].soldier.stance = stance;
        };
        // On foot: nothing.
        fx.follow_jets(&sim, look);
        assert!(fx.jets().is_empty(), "{game} {name}");
        for jet in [Stance::Hover, Stance::Jet] {
            set(&mut sim, jet);
            fx.follow_jets(&sim, look);
            assert_eq!(fx.jets(), [h], "{game} {name} {jet:?}");
            // One effect per soldier, however long he jets; it rides him.
            sim.units[h.index as usize].soldier.pos.x += 3.0;
            fx.follow_jets(&sim, look);
            fx.update(0.3);
            assert_eq!(fx.jets(), [h]);
            let e = fx.live().iter().find(|e| e.jet == Some(h)).unwrap();
            let u = &sim.units[h.index as usize].soldier;
            let want = crate::fx::jet::jet_frame(u.pos, u.yaw, u.vel);
            assert!(e.frame.pos.abs_diff_eq(want.pos, 1e-5));
            assert!(
                (e.frame.pos.y - u.pos.y - 1.481).abs() < 1e-4,
                "on the back"
            );
            // The frame's x runs up the spine: the exhaust (-x) goes down.
            assert!(e.frame.rot.x_axis.abs_diff_eq(Vec3::Y, 1e-5));
            assert!(e.emitted().iter().sum::<u32>() > 0, "{name} emits");
            assert!(!e.is_stopped());
            // The jet ends (a tap, empty fuel, a landing): stopped.
            set(&mut sim, Stance::Fall);
            fx.follow_jets(&sim, look);
            assert!(fx.jets().is_empty());
            assert!(fx
                .live()
                .iter()
                .any(|e| e.name() == Some(name) && e.is_stopped()));
        }
        // A death in the jet stops it too.
        set(&mut sim, Stance::Hover);
        fx.follow_jets(&sim, look);
        assert_eq!(fx.jets(), [h]);
        sim.units[h.index as usize].alive = false;
        fx.follow_jets(&sim, look);
        assert!(fx.jets().is_empty());
        // Its particles live out, then it is gone.
        fx.update(5.0);
        assert!(fx.live().iter().all(|e| e.name() != Some(name)), "{name}");
    }
}

#[test]
fn a_class_without_a_jet_effect_spawns_nothing() {
    let Some(lib) = library("bf2", "rep") else {
        return;
    };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let h = sim
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    sim.units[h.index as usize].soldier.stance = Stance::Hover;
    fx.follow_jets(&sim, |_| None);
    assert!(fx.is_empty());
    fx.follow_jets(&sim, |_| Some("no_such_effect"));
    assert!(fx.is_empty());
}

/// The drawn bone moves a running jet effect at once and places the next.
#[test]
fn the_drawn_bone_carries_the_effect() {
    let Some(lib) = library("bf2", "rep") else {
        return;
    };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let h = sim
        .spawn(&UnitSpec::player_in_slot(0, 0, Vec3::ZERO))
        .unwrap();
    sim.units[h.index as usize].soldier.stance = Stance::Hover;
    // A quarter turn about z: x up.
    let bone = Mat4::from_cols(
        Vec3::Y.extend(0.0),
        Vec3::NEG_X.extend(0.0),
        Vec3::Z.extend(0.0),
        Vec3::new(1.0, 2.0, 3.0).extend(1.0),
    );
    let mut frames = vec![None; sim.units.len()];
    frames[h.index as usize] = Some(bone);
    fx.set_jet_frames(frames.clone());
    fx.follow_jets(&sim, |_| Some("rep_sfx_jetpack"));
    let e = fx.live().iter().find(|e| e.jet == Some(h)).unwrap();
    assert!(e.frame.pos.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-5));
    assert!(e.frame.rot.x_axis.abs_diff_eq(Vec3::Y, 1e-5));
    frames[h.index as usize] = Some(Mat4::from_translation(Vec3::X * 9.0));
    fx.set_jet_frames(frames);
    let e = fx.live().iter().find(|e| e.jet == Some(h)).unwrap();
    assert!(e.frame.pos.abs_diff_eq(Vec3::X * 9.0, 1e-5));
}

/// A moving jet trooper drawn at 60 Hz between 30 Hz ticks (card RINT): each
/// frame his jet effect sits on the `bone_ribcage` that frame draws (the
/// blended pose), not where the last tick left him.
#[test]
fn a_moving_jets_effect_rides_the_drawn_body_every_frame() {
    use crate::soldier_vis::SoldierVisuals;
    use crate::walk::{ClassVisual, TeamTemplate, WalkSim};
    use core::sim::PadState;
    let root = crate::assets::baked_root();
    let geometry = "rep_inf_jettrooper";
    if !root
        .join(format!("side_bf2/soldiers/{geometry}.glb"))
        .exists()
    {
        eprintln!("SKIP: no bf2 {geometry} bake");
        return;
    }
    let Some(lib) = library("bf2", "rep") else {
        return;
    };
    let mut t = TeamTemplate::default_trooper();
    t.classes = vec![ClassVisual {
        name: "jet".into(),
        label: "jet".into(),
        geometry: Some(geometry.into()),
        ..Default::default()
    }];
    let mut vis = SoldierVisuals::load_cpu(&root, "bf2", &[t.clone(), t]);
    let mut fx = Fx::with_library(lib);
    let ground = core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    };
    let mut w = WalkSim::new(1, Vec3::new(5.0, 20.0, -3.0), Some(ground));
    w.blend.on = true;
    let p = w.player.index as usize;
    {
        let u = &mut w.sim.units[p];
        u.consts.max_speed = 7.0;
        u.consts.jet_push = 8.0;
        u.consts.jet_acceleration = 30.0;
        u.consts.jet_hover = true;
        u.consts.jet_fuel_cost = 0.0;
        u.consts.jet_control = [1.5, 1.25, 1.25];
        // Off the ground (the spawn put him on it), hovering.
        u.soldier.pos.y = 5.0;
        u.soldier.grounded = false;
        u.soldier.stance = Stance::Hover;
        u.soldier.fuel = 1.0;
    }
    // Forward and turning: the body moves and turns between ticks.
    let pad = PadState {
        ly: i16::MAX,
        rx: i16::MAX / 3,
        ..Default::default()
    };
    let dt = 1.0 / 60.0;
    let (mut worst, mut checked) = (0.0f32, 0);
    let start = w.sim.units[p].soldier.pos;
    for frame in 0..60 {
        // The windowed game's tick hook (`main/windowed.rs`).
        w.advance_with(dt, std::slice::from_ref(&pad), |walk| {
            fx.follow_jets(&walk.sim, |_| Some("rep_sfx_jetpack"));
            vis.note_shots(walk);
        });
        fx.advance_frame(Some(&mut vis), &mut w, &pad, dt);
        // What the frame draws: its blended pose, as the draw lists read it.
        crate::render_blend::blend_in_looking(&mut w, &pad);
        let drawn = vis.jet_frames(&w)[p];
        w.blend_out();
        let (Some(m), Some(e)) = (drawn, fx.live().iter().find(|e| e.jet.is_some())) else {
            continue;
        };
        assert!(w.sim.units[p].soldier.stance.jetting(), "frame {frame}");
        if frame < 10 {
            continue;
        }
        let off = (e.frame.pos - m.w_axis.truncate()).length();
        worst = worst.max(off);
        checked += 1;
    }
    let moved = (w.sim.units[p].soldier.pos - start).length();
    println!("moved {moved:.2} m; worst emitter offset {worst:.4} m over {checked} frames");
    assert!(moved > 2.0, "he must move: {moved}");
    assert!(checked >= 40, "{checked}");
    assert!(
        worst < 1e-4,
        "the jet effect is {worst:.3} m off the drawn body"
    );
}
