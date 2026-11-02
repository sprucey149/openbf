use crate::sim::tests::flat_ground;
use crate::sim::{Event, Sim};
use crate::unit::{UnitHandle, UnitSpec};
use crate::weapon::{BurnClass, EmitterOrdnance, OrdnanceClass, WeaponClass, DEG};
use crate::TICK_DT;
use glam::Vec3;

fn flamethrower(bf2: bool, time: f32, rate: f32) -> EmitterOrdnance {
    EmitterOrdnance {
        cone_length: 10.0,
        cone_angle: 45.0 * DEG,
        max_targets: 10,
        jump_deduction: 0.0,
        damage_threshold: 0.0,
        max_jump_distance: 0.0,
        first_radius: 2.0,
        push: 0.0,
        lightning: None,
        bolts: 1,
        affect_friends: false,
        affect_enemies: true,
        hit_all: true,
        radius: 0.0,
        effect: Some("com_sfx_weap_flamethrower_ord".into()),
        burn: BurnClass {
            time,
            rate,
            effects: vec![("bone_ribcage".into(), "com_sfx_inf_damage_fire".into())],
        },
        bf2,
    }
}

fn add(s: &mut Sim, em: EmitterOrdnance) -> usize {
    s.add_weapon_class(WeaponClass {
        emitter: Some(em),
        ordnance: OrdnanceClass {
            max_damage: 15.0,
            min_damage: Some(5.0),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn scene() -> (Sim, UnitHandle, UnitHandle) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    s.units[shooter.index as usize].soldier.yaw = -std::f32::consts::FRAC_PI_2;
    let target = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(3.0, 0.0, 0.0)))
        .unwrap();
    (s, shooter, target)
}

fn health(s: &Sim, h: UnitHandle) -> f32 {
    s.unit(h).map_or(0.0, |u| u.health.cur)
}

#[test]
fn the_hero_flamethrower_burns_fifty_a_second_for_five_seconds() {
    let (mut s, shooter, target) = scene();
    let class = add(&mut s, flamethrower(true, 5.0, 50.0));
    s.fire_emitter(shooter.index as usize, class, 1.0);
    let after_hit = health(&s, target);
    assert_eq!(after_hit, 285.0);
    let burn = s.unit(target).unwrap().burn.unwrap();
    assert_eq!(
        (burn.left, burn.damage_per_second, burn.owner),
        (5.0, 50.0, shooter)
    );
    for _ in 0..30 {
        s.tick_burns(TICK_DT);
    }
    assert!((after_hit - health(&s, target) - 50.0).abs() < 1e-3);
    for _ in 0..200 {
        s.tick_burns(TICK_DT);
    }
    assert!(s.unit(target).unwrap().burn.is_none());
    let burnt = after_hit - health(&s, target);
    assert!((burnt - 250.0).abs() < 50.0 * TICK_DT + 1e-3, "{burnt}");
}

#[test]
fn a_new_hit_keeps_the_longer_timer_and_takes_the_new_rate() {
    let (mut s, shooter, target) = scene();
    let long = add(&mut s, flamethrower(true, 5.0, 50.0));
    let short = add(&mut s, flamethrower(true, 1.5, 0.0));
    s.fire_emitter(shooter.index as usize, long, 1.0);
    for _ in 0..30 {
        s.tick_burns(TICK_DT);
    }
    s.fire_emitter(shooter.index as usize, short, 1.0);
    let burn = s.unit(target).unwrap().burn.unwrap();
    assert!((burn.left - 4.0).abs() < 1e-4);
    assert_eq!(burn.damage_per_second, 0.0);
    assert_eq!(burn.weapon_class, short);
    let before = health(&s, target);
    s.tick_burns(TICK_DT);
    assert_eq!(health(&s, target), before, "rate 0 burns without damage");
}

#[test]
fn the_incinerator_marks_without_damage_and_runs_out() {
    let (mut s, shooter, target) = scene();
    let class = add(&mut s, flamethrower(true, 1.5, 0.0));
    s.fire_emitter(shooter.index as usize, class, 1.0);
    let after_hit = health(&s, target);
    for _ in 0..44 {
        s.tick_burns(TICK_DT);
    }
    assert!(s.unit(target).unwrap().burn.is_some());
    s.tick_burns(TICK_DT);
    s.tick_burns(TICK_DT);
    assert!(s.unit(target).unwrap().burn.is_none());
    assert_eq!(health(&s, target), after_hit);
}

#[test]
fn bf1_rules_and_a_class_without_burn_set_none() {
    let (mut s, shooter, target) = scene();
    let bf1 = add(&mut s, flamethrower(false, 5.0, 50.0));
    let none = add(&mut s, flamethrower(true, 0.0, 50.0));
    s.fire_emitter(shooter.index as usize, bf1, 1.0);
    s.fire_emitter(shooter.index as usize, none, 1.0);
    assert!(s.unit(target).unwrap().burn.is_none());
}

#[test]
fn a_burn_kill_is_credited_to_the_shooter() {
    let (mut s, shooter, target) = scene();
    let class = add(&mut s, flamethrower(true, 5.0, 50.0));
    s.units[target.index as usize].health.cur = 20.0;
    s.fire_emitter(shooter.index as usize, class, 1.0);
    s.events.clear();
    for _ in 0..30 {
        s.tick_burns(TICK_DT);
    }
    assert!(!s.units[target.index as usize].alive);
    assert!(s.units[target.index as usize].burn.is_none());
    assert!(s.events.iter().any(|e| matches!(
        e,
        Event::Kill { shooter: k, victim } if *k == shooter && *victim == target
    )));
    assert!(!s.events.iter().any(|e| matches!(e, Event::Hit { .. })));
}

#[test]
fn a_burn_reaches_the_hash_only_while_it_runs() {
    let (mut s, shooter, target) = scene();
    let class = add(&mut s, flamethrower(true, 0.1, 0.0));
    s.fire_emitter(shooter.index as usize, class, 1.0);
    let burning = s.hash();
    let burn = s.units[target.index as usize].burn.take();
    let without = s.hash();
    assert_ne!(burning, without);
    s.units[target.index as usize].burn = burn;
    for _ in 0..4 {
        s.tick_burns(TICK_DT);
    }
    assert!(s.units[target.index as usize].burn.is_none());
    assert_eq!(s.hash(), without);
}

#[test]
fn a_bots_cone_spares_a_human_teammate_and_still_draws_its_arc() {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    let bot = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    s.units[bot.index as usize].soldier.yaw = -std::f32::consts::FRAC_PI_2;
    let human = s
        .spawn(&UnitSpec::player(0, Vec3::new(3.0, 0.0, 0.0)))
        .unwrap();
    let before = health(&s, human);
    let class = add(
        &mut s,
        EmitterOrdnance {
            affect_friends: true,
            ..flamethrower(true, 5.0, 50.0)
        },
    );
    s.fire_emitter(bot.index as usize, class, 1.0);
    assert_eq!(health(&s, human), before);
    assert!(s.unit(human).unwrap().burn.is_none());
    assert!(s.events.iter().any(|e| matches!(e, Event::Emitter { .. })));
}

#[test]
fn an_emitter_uses_shield_then_health_and_never_ignites_a_droideka() {
    for (raised, points, scale, expected_health, expected_shield) in [
        (false, 100.0, 1.0, 285.0, 100.0),
        (true, 100.0, 2.0, 300.0, 70.0),
        (true, 10.0, 2.0, 290.0, 0.0),
        (true, 100.0, 0.0, 285.0, 100.0),
    ] {
        let mut s = Sim::empty(3);
        let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        let target = s
            .spawn(&UnitSpec {
                droideka: Some(std::sync::Arc::new(Default::default())),
                ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -3.0))
            })
            .unwrap();
        let shield = &mut s
            .unit_mut(target)
            .unwrap()
            .droideka
            .as_mut()
            .unwrap()
            .shield;
        shield.on = raised;
        shield.points = points;
        let class = add(&mut s, flamethrower(true, 5.0, 50.0));
        s.weapon_classes[class].ordnance.shield_scale = scale;
        s.fire_emitter(shooter.index as usize, class, 1.0);
        let u = s.unit(target).unwrap();
        assert_eq!(
            u.health.cur, expected_health,
            "raised={raised}, points={points}, scale={scale}"
        );
        assert_eq!(u.droideka.as_ref().unwrap().shield.points, expected_shield);
        assert!(u.burn.is_none());
        assert_eq!(
            u.health.shield, 0.0,
            "borrowed shield cannot be charged twice"
        );
    }
}
