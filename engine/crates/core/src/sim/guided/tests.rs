use super::*;
use crate::guided::{GuidedClass, GuidedWeapon};
use crate::unit::UnitSpec;
use crate::weapon::{ExplosionClass, PropSource, WeaponClass};

struct Props;
impl PropSource for Props {
    fn prop(&self, _: &str) -> Option<&str> {
        None
    }
    fn prop_hash(&self, _: u32) -> Option<&str> {
        None
    }
}

fn scene(award: bool) -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(71);
    sim.heightfield = Some(crate::sim::tests::flat_ground());
    let mut class = GuidedClass::from_props(&Props);
    award_flight(&mut class.flight);
    class.explosion = Some(ExplosionClass {
        damage: 500.0,
        damage_inner: 1.0,
        damage_outer: 5.0,
        ..Default::default()
    });
    class.noncritical = Some(ExplosionClass {
        damage: 50.0,
        damage_inner: 1.0,
        damage_outer: 5.0,
        ..Default::default()
    });
    let weapon = WeaponClass {
        rounds_per_clip: 1,
        initial_salvo_delay: 0.3,
        trigger_single: true,
        guided: Some(Box::new(GuidedWeapon {
            class,
            spawn_offset: Vec3::new(0.0, 1.0, 5.0),
            requires_award: award,
        })),
        ..Default::default()
    };
    let index = sim.add_weapon_class(weapon);
    let mut spec = UnitSpec::player(0, Vec3::ZERO);
    spec.weapon_class = index;
    spec.total_clips = 3;
    let h = sim.spawn(&spec).unwrap();
    sim.unit_mut(h).unwrap().soldier.pitch = 0.0;
    (sim, h)
}

fn award_flight(f: &mut crate::vehicle::flyer::FlyerParams) {
    f.acceleration = 80.0;
    f.min_speed = 30.0;
    f.mid_speed = 30.0;
    f.max_speed = 30.0;
    f.boost_speed = 80.0;
    f.boost_acceleration = 50.0;
    f.pitch_rate = 1.2;
    f.turn_rate = 1.2;
    f.roll_rate = 3.15;
    f.level_filter = 1.5;
    f.energy_max = 200.0;
    f.energy_auto_restore = -5.0;
    f.energy_boost_drain = 20.0;
}

fn step(sim: &mut Sim, buttons: u32) {
    sim.step(&[PadState {
        buttons,
        ..Default::default()
    }]);
}

fn launch(sim: &mut Sim, owner: UnitHandle) {
    step(sim, buttons::FIRE);
    for _ in 0..30 {
        if sim.controlled_guided(owner).is_some() {
            return;
        }
        step(sim, 0);
    }
    panic!("rocket did not launch");
}

#[test]
fn award_gate_consumption_and_flight_route_controls() {
    let (mut sim, owner) = scene(true);
    step(&mut sim, buttons::FIRE);
    assert!(sim.guided.is_empty());
    assert_eq!(sim.unit(owner).unwrap().weapon.clip, 1);
    sim.cheats.all_awards = true;
    step(&mut sim, 0);
    launch(&mut sim, owner);
    let before = sim.unit(owner).unwrap().soldier.pos;
    let rocket = sim.guided[0].pos;
    for _ in 0..15 {
        sim.step(&[PadState {
            ly: i16::MAX,
            rx: i16::MAX,
            ..Default::default()
        }]);
    }
    assert!(sim.unit(owner).unwrap().soldier.pos.distance(before) < 0.01);
    assert!(sim.guided[0].pos.distance(rocket) > 10.0);
    assert!(sim.guided[0].pos.x > 1.0);
    assert_eq!(sim.guided.len(), 1);
    assert!(sim.guided[0].energy() < 200.0);
    assert!(sim.droids.is_empty() && sim.bolts.is_empty());
    let rounds = sim
        .unit(owner)
        .unwrap()
        .weapon
        .rounds(&sim.weapon_classes[sim.unit(owner).unwrap().weapon_class]);
    assert!(rounds.unwrap() < 3);
}

#[test]
fn hand_back_detonates_and_a_held_fire_cannot_relaunch() {
    let (mut sim, owner) = scene(false);
    launch(&mut sim, owner);
    step(&mut sim, buttons::ACTION | buttons::FIRE);
    assert!(sim.guided.is_empty());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::Returned,
            ..
        }
    )));
    for _ in 0..40 {
        step(&mut sim, buttons::FIRE);
    }
    assert!(sim.guided.is_empty());
    step(&mut sim, 0);
    launch(&mut sim, owner);
    sim.kill_environment(owner, crate::sim::DeathCause::Suicide);
    step(&mut sim, 0);
    assert!(sim.guided.is_empty());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::OwnerGone,
            ..
        }
    )));
}

#[test]
fn launch_sweep_hits_a_wall_before_the_authored_spawn_offset() {
    let (mut sim, owner) = scene(false);
    sim.collision.push_body(crate::collision::Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 2.0, -2.75)),
        crate::collision::Shape::Box {
            half: Vec3::new(2.0, 2.0, 0.25),
        },
    ));
    step(&mut sim, buttons::FIRE);
    let mut impact = None;
    for _ in 0..60 {
        impact = impact.or_else(|| {
            sim.events.iter().find_map(|e| match e {
                Event::GuidedGone { pos, .. } => Some(*pos),
                _ => None,
            })
        });
        step(&mut sim, 0);
    }
    let at = impact.expect("spawn must not cross the wall");
    assert!(at.z >= -3.0 && at.z <= -2.5);
    assert!(sim.guided.is_empty());
    assert!(sim.units[owner.index as usize].weapon.shots_fired > 0);
}

#[test]
fn collision_damages_with_owner_credit_and_expiry_returns_control() {
    let (mut sim, owner) = scene(false);
    let target = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    launch(&mut sim, owner);
    let mut killed = None;
    for tick in 0..30 {
        step(&mut sim, 0);
        if sim.events.iter().any(|e| matches!(e, Event::Kill { shooter, victim } if *shooter == owner && *victim == target)) {
            killed.get_or_insert(tick);
        }
    }
    assert!(killed.is_some());
    assert!(sim.guided[0].energy() <= 0.0);
    assert!(sim.controlled_guided(owner).is_some());
    for _ in 0..40 {
        step(&mut sim, 0);
    }
    assert!(sim.controlled_guided(owner).is_none());
    launch(&mut sim, owner);
    sim.guided[0].flyer.energy = 0.0;
    step(&mut sim, 0);
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::Expired,
            ..
        }
    )));
    let held = sim.guided[0].pos;
    assert!(sim.controlled_guided(owner).is_some());
    assert!(sim.flown_guided(owner).is_none());
    let hold = (1.0 * crate::TICK_HZ as f32) as usize;
    for _ in 0..hold - 2 {
        step(&mut sim, 0);
        assert_eq!(sim.guided.len(), 1);
        assert_eq!(sim.guided[0].pos, held);
        assert!(!sim
            .events
            .iter()
            .any(|e| matches!(e, Event::GuidedGone { .. })));
    }
    for _ in 0..3 {
        step(&mut sim, 0);
    }
    assert!(sim.guided.is_empty());
    assert!(sim.controlled_guided(owner).is_none());
}

#[test]
fn contact_damage_scales_from_max_to_boost_speed_and_spends_the_rocket() {
    let (mut sim, owner) = scene(false);
    let c = sim.weapon_classes[sim.unit(owner).unwrap().weapon_class]
        .guided
        .as_ref()
        .unwrap()
        .class
        .clone();
    assert_eq!(c.contact_damage, [500.0, 2000.0]);
    let target = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    sim.unit_mut(target).unwrap().health.max = 10_000.0;
    sim.unit_mut(target).unwrap().health.cur = 10_000.0;
    launch(&mut sim, owner);
    let mid = (c.flight.max_speed + c.flight.boost_speed) * 0.5;
    let mut r = sim.guided[0].clone();
    r.vel = r.rot * Vec3::Z * mid;
    sim.guided_strike(&mut r, Some(crate::projectile::Target::Unit(target)));
    assert_eq!(r.energy(), 0.0);
    let left = sim.unit(target).unwrap().health.cur;
    assert!(
        (10_000.0 - left - 1250.0).abs() < 1.0,
        "dealt {}",
        10_000.0 - left
    );
    sim.guided[0] = r;
    step(&mut sim, 0);
    assert!(sim
        .events
        .iter()
        .any(|e| matches!(e, Event::GuidedGone { .. })));
    assert_eq!(sim.guided.len(), 1);
    assert!(!sim.guided[0].live());
}

#[test]
fn expiry_at_boost_speed_is_critical() {
    let (mut sim, owner) = scene(false);
    launch(&mut sim, owner);
    let r = &mut sim.guided[0];
    r.flyer.energy = 0.0;
    r.vel = r.rot * Vec3::Z * 64.0;
    step(&mut sim, 0);
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::CriticalExpiry,
            ..
        }
    )));
}

#[test]
fn a_launch_flies_at_min_speed_nose_first() {
    let (mut sim, owner) = scene(false);
    let wc = sim.unit(owner).unwrap().weapon_class;
    sim.weapon_classes[wc]
        .guided
        .as_mut()
        .unwrap()
        .class
        .flight
        .mid_speed = 50.0;
    launch(&mut sim, owner);
    let r = &sim.guided[0];
    assert_eq!(r.flyer.phase, crate::vehicle::flyer::FlightPhase::Flying);
    assert!(r.vel.length() >= 30.0 && r.vel.length() <= 30.0 + 80.0 * crate::TICK_DT + 0.01);
    assert!(r.forward().dot(Vec3::NEG_Z) > 0.99);
    assert!((r.drawn_rotation() * Vec3::Z).dot(Vec3::NEG_Z) > 0.99);
}

#[test]
fn bot_steers_a_rocket_and_all_state_replays_from_recorded_pads() {
    let (mut a, owner) = scene(false);
    let (mut b, _) = scene(false);
    let pads: Vec<_> = (0..180)
        .map(|tick| PadState {
            buttons: if tick == 0 {
                buttons::FIRE
            } else if tick == 100 {
                buttons::ACTION
            } else {
                0
            },
            rx: if tick > 20 { 10000 } else { 0 },
            my: if tick > 50 { 300 } else { 0 },
            ..Default::default()
        })
        .collect();
    let recorded = ron::to_string(&pads).unwrap();
    let replay: Vec<PadState> = ron::from_str(&recorded).unwrap();
    for (p, q) in pads.iter().zip(replay) {
        a.step(&[*p]);
        b.step(&[q]);
        assert_eq!(a.hash(), b.hash());
    }
    launch(&mut a, owner);
    let target = a
        .spawn(&UnitSpec::trooper(1, Vec3::new(20.0, 0.0, -30.0)))
        .unwrap();
    a.unit_mut(owner).unwrap().controller = Controller::Ai;
    a.unit_mut(owner).unwrap().ai.target = Some(target.into());
    a.guided[0].rot = GuidedRocket::body(Vec3::NEG_Z);
    a.guided[0].flyer.tilt_pitch = 0.0;
    a.guided[0].flyer.tilt_roll = 0.0;
    assert!(a.bot_guided_pad(0).unwrap().rx > 0);
    let hash = a.hash();
    a.guided[0].prev_buttons ^= buttons::ACTION;
    assert_ne!(hash, a.hash());
}

#[test]
fn rockets_take_droid_scaled_damage_and_boost_spends_energy() {
    let (mut sim, owner) = scene(false);
    launch(&mut sim, owner);
    let energy = sim.guided[0].energy();
    for _ in 0..30 {
        sim.step(&[PadState {
            buttons: buttons::SPRINT,
            ly: i16::MAX,
            ..Default::default()
        }]);
    }
    assert!(sim.guided[0].vel.length() > 75.0);
    assert!((energy - sim.guided[0].energy() - 20.0).abs() < 0.2);
    let rocket = sim.guided[0].clone();
    let enemy = sim
        .spawn(&UnitSpec::trooper(1, rocket.pos + Vec3::X * 10.0))
        .unwrap();
    let mut bolt = crate::projectile::Bolt::fire(
        rocket.pos + Vec3::X,
        -Vec3::X,
        60.0,
        1.0,
        crate::projectile::Shooter {
            owner: enemy,
            team: 1,
            weapon_class: 0,
            is_player: false,
        },
        sim.heightfield.as_ref(),
        None,
        &sim.collision,
        HullFilter::All,
    );
    bolt.hit = crate::projectile::Hit::Target(crate::projectile::Target::Guided(rocket.id));
    bolt.end = rocket.pos;
    sim.resolve_bolt(&bolt);
    assert!(sim.guided[0].health <= 0.0);
    step(&mut sim, 0);
    assert!(sim.guided.is_empty());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::Destroyed,
            ..
        }
    )));
}

#[test]
fn a_bot_launches_and_guides_at_a_seen_enemy() {
    let (mut sim, owner) = scene(false);
    let class = sim.unit(owner).unwrap().weapon_class;
    sim.unit_mut(owner).unwrap().controller = Controller::Ai;
    sim.spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -25.0)))
        .unwrap();
    let mut launched = false;
    let mut hit = false;
    for _ in 0..300 {
        sim.step(&[]);
        launched |= sim.events.iter().any(|e| matches!(e, Event::Shot { unit, weapon_class, .. } if *unit == owner && *weapon_class as usize == class));
        hit |= sim
            .events
            .iter()
            .any(|e| matches!(e, Event::GuidedGone { .. }));
        if launched && hit {
            break;
        }
    }
    assert!(launched && hit, "launched={launched}, hit={hit}");
}

#[test]
fn forced_weapon_changes_return_control_and_rockets_contact_deployed_turrets() {
    let (mut sim, owner) = scene(false);
    launch(&mut sim, owner);
    let r = sim.guided[0].clone();
    let class = crate::autoturret::TurretClass::new(
        &crate::remote::tests::Props(crate::autoturret::tests::BF2_TURRET),
        WeaponClass::bf1_rifle(),
    );
    let mut turret = crate::autoturret::Turret::new(1, owner, 0, 0, &class, r.pos, Vec3::ZERO, 0.0);
    turret.landed = true;
    sim.turrets.push(turret);
    assert!(sim.guided_contact(&r, r.pos + Vec3::Z * 2.0).is_some());
    sim.turrets.clear();
    let other = sim.add_weapon_class(WeaponClass::bf1_rifle());
    sim.unit_mut(owner).unwrap().weapon_class = other;
    step(&mut sim, 0);
    assert!(sim.guided.is_empty());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::GuidedGone {
            end: GuidedEnd::Returned,
            ..
        }
    )));
}

/// PR #61 review fix 4: a bot's rocket with no target in sight hands control
/// back after `BOT_BLIND`, not at the end of its energy.
#[test]
fn a_bots_rocket_with_no_target_returns_after_three_seconds() {
    let (mut sim, owner) = scene(false);
    launch(&mut sim, owner);
    sim.unit_mut(owner).unwrap().controller = Controller::Ai;
    let ticks = (crate::guided::BOT_BLIND * crate::TICK_HZ as f32).round() as usize;
    let mut returned_at = None;
    for t in 1..=ticks + 5 {
        sim.step(&[]);
        let back = sim.events.iter().any(|e| {
            matches!(
                e,
                Event::GuidedGone {
                    end: GuidedEnd::Returned,
                    ..
                }
            )
        });
        if back {
            returned_at = Some(t);
            break;
        }
    }
    let t = returned_at.expect("the rocket never returned control");
    assert!(
        t.abs_diff(ticks) <= 2,
        "returned after {t} ticks, not {ticks}"
    );
    assert!(sim.guided.is_empty());
    assert!(sim.controlled_guided(owner).is_none());
}

#[test]
fn a_seat_guided_rocket_steers_pitches_and_rolls_with_flyer_sticks() {
    let mut class = GuidedClass::from_props(&Props);
    award_flight(&mut class.flight);
    let rocket = GuidedRocket {
        id: 1,
        owner: UnitHandle { index: 0, gen: 0 },
        team: 0,
        weapon_class: 0,
        seat: Some((crate::vehicle::VehicleHandle { index: 0, gen: 0 }, 1)),
        pos: Vec3::new(0.0, 100.0, 0.0),
        vel: Vec3::NEG_Z * 30.0,
        rot: GuidedRocket::body(Vec3::NEG_Z),
        flyer: GuidedRocket::flying(&class),
        health: 1.0,
        age: 0.0,
        prev_buttons: 0,
        pending_end: None,
        spent: None,
        blind: 0.0,
    };
    let env = crate::vehicle::flyer::FlyerEnvironment {
        min_height: -1000.0,
        max_height: 1000.0,
        player_aboard: true,
        ..Default::default()
    };
    let flown = |pad: PadState| {
        let mut r = rocket.clone();
        let (input, _) = crate::sim::seats::driver_input(&pad);
        for _ in 0..10 {
            r.fly(&class, input, env, crate::TICK_DT);
        }
        r
    };
    let still = flown(PadState::default());
    assert!(still.forward().dot(Vec3::NEG_Z) > 0.999);
    let pitched = flown(PadState {
        ry: i16::MAX,
        ..Default::default()
    });
    assert!(pitched.forward().y.abs() > 0.1);
    let turned = flown(PadState {
        rx: i16::MAX,
        ..Default::default()
    });
    assert!(turned.forward().x.abs() > 0.1);
    let rolled = flown(PadState {
        lx: i16::MAX,
        ..Default::default()
    });
    assert!((rolled.rot * Vec3::Y).x.abs() > 0.1);
}
