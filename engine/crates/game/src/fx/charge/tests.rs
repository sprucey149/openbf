use super::*;
use crate::fx::Library;
use core::fmath::FloatExt;
use core::unit::UnitSpec;
use core::weapon::{FireState, WeaponClass, WeaponState};

fn library() -> Option<Library> {
    let root = crate::assets::baked_root();
    let common = root.join("common_bf2");
    if !common.join("effects.ron").exists() {
        eprintln!("SKIP: {} is not baked", common.display());
        return None;
    }
    Some(Library::load(&root, &common, "bf2", &[]))
}

/// The BF2 arc caster's charge keys: 5 rounds, 2.5 s to a full charge.
fn arc_caster(effect: Option<&str>) -> WeaponClass {
    WeaponClass {
        rounds_per_clip: 5,
        max_pressed_time: 2.5,
        charge_up_effect: effect.map(str::to_owned),
        ..Default::default()
    }
}

/// Hold the trigger `held` seconds into a charge (0 = not pressed).
fn hold(sim: &mut Sim, h: UnitHandle, class: usize, held: f32) {
    let c = sim.weapon_classes[class].clone();
    let u = &mut sim.units[h.index as usize];
    u.weapon_class = class;
    u.weapon = WeaponState::new(&c, true, 2);
    u.weapon.state = FireState::Idle;
    u.weapon.active = true;
    u.weapon.fire_cmd = if held > 0.0 {
        1 + (held * 30.0) as i32
    } else {
        0
    };
    u.weapon.refire_time = c.max_pressed_time;
    u.weapon.refire_accum = held.fmin(c.max_pressed_time);
}

#[test]
fn a_held_charge_glows_at_the_gun_brightening_to_full_and_stops_on_release() {
    let Some(lib) = library() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let class = sim.add_weapon_class(arc_caster(Some("com_sfx_chargeup")));
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();

    hold(&mut sim, h, class, 0.0);
    fx.ingest_sim(&sim);
    assert!(fx.charge_glows().is_empty(), "not pressed");

    hold(&mut sim, h, class, 1.25);
    fx.ingest_sim(&sim);
    let glows = fx.charge_glows();
    assert_eq!(glows.len(), 1, "one glow while charging");
    assert_eq!(glows[0].0, h);
    assert!(
        (glows[0].1 - 0.5).abs() < 1e-6,
        "half bright: {}",
        glows[0].1
    );
    assert_eq!(
        glows[0].2,
        sim.unit(h).unwrap().muzzle(),
        "at the fire point"
    );
    assert_eq!(
        fx.live.iter().find(|e| e.charge.is_some()).unwrap().name(),
        Some("com_sfx_chargeup")
    );

    // Fully charged and still held: the same glow, at full.
    hold(&mut sim, h, class, 4.0);
    fx.ingest_sim(&sim);
    let glows = fx.charge_glows();
    assert_eq!(glows.len(), 1, "not spawned again");
    assert_eq!(glows[0].1, 1.0, "full charge, full glow");

    // Drawn where the soldier's weapon tip is drawn.
    let tip = Vec3::new(0.3, 1.4, -0.6);
    let mut points = vec![None; sim.units.len()];
    points[h.index as usize] = Some(tip);
    fx.set_repair_origins(&points);
    assert_eq!(fx.charge_glows()[0].2, tip);

    // Released: gone at once.
    hold(&mut sim, h, class, 0.0);
    fx.ingest_sim(&sim);
    assert!(fx.charge_glows().is_empty(), "released");
}

#[test]
fn the_glow_scales_every_draw_by_the_charge() {
    let Some(lib) = library() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let class = sim.add_weapon_class(arc_caster(Some("com_sfx_chargeup")));
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    hold(&mut sim, h, class, 2.5);
    fx.ingest_sim(&sim);
    fx.update(0.3);
    let eye = sim.unit(h).unwrap().muzzle() + Vec3::new(0.0, 0.0, 3.0);
    let draw = |fx: &Fx| {
        let mut out = Vec::new();
        let e = fx.live.iter().find(|e| e.charge.is_some()).unwrap();
        e.draws(eye, Vec3::NEG_Z, &mut |_| None, &mut out);
        out
    };
    let full = draw(&fx);
    assert!(!full.is_empty(), "the full glow draws");
    fx.live
        .iter_mut()
        .find(|e| e.charge.is_some())
        .unwrap()
        .glow = 0.5;
    let half = draw(&fx);
    assert_eq!(full.len(), half.len());
    for (a, b) in full.iter().zip(&half) {
        for k in 0..4 {
            assert_eq!(b.color[k], (f32::from(a.color[k]) * 0.5) as u8);
        }
    }
}

#[test]
fn no_glow_when_the_class_names_no_effect() {
    let Some(lib) = library() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let bare = sim.add_weapon_class(arc_caster(None));
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    hold(&mut sim, h, bare, 2.5);
    fx.ingest_sim(&sim);
    assert!(fx.charge_glows().is_empty(), "the class names no effect");
}

#[test]
fn a_bf1_map_draws_its_own_charge_up_for_the_bf2_arc_caster() {
    let root = crate::assets::baked_root();
    let common = root.join("common_bf1");
    if !common.join("effects.ron").exists() {
        eprintln!("SKIP: {} is not baked", common.display());
        return;
    }
    let mut fx = Fx::with_library(Library::load(&root, &common, "bf1", &[]));
    let mut sim = Sim::empty(1);
    let class = sim.add_weapon_class(arc_caster(Some("com_sfx_chargeup")));
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    hold(&mut sim, h, class, 2.5);
    fx.ingest_sim(&sim);
    let e = fx.live.iter().find(|e| e.charge.is_some());
    assert_eq!(e.and_then(|e| e.name()), Some("chargeup"));
}
