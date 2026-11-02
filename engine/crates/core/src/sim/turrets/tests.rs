use super::*;
use crate::autoturret::tests::BF2_TURRET;
use crate::remote::tests::Props;
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, PadState};
use crate::unit::{Controller, SlotSpec, UnitSpec};
use crate::weapon::{Dispenser, ExplosionClass, WeaponClass};
use crate::TICK_DT;

fn setup() -> (Sim, UnitHandle, usize) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(flat_ground());
    let mut gun = WeaponClass::bf1_rifle();
    gun.name = "com_weap_inf_dropturret_gun".into();
    gun.rounds_per_clip = 0;
    gun.shot_delay = [1.7; 2];
    gun.salvo_count = 4;
    gun.salvo_delay = [0.15, 0.3];
    gun.ai_range = [0.0, 15.0, 40.0];
    gun.ordnance.velocity = 100.0;
    gun.ordnance.life_span = 0.8;
    gun.ordnance.max_damage = 100.0;
    let mut c = TurretClass::new(&Props(BF2_TURRET), gun);
    c.explosion = Some(ExplosionClass {
        damage: 50.0,
        damage_inner: 4.0,
        damage_outer: 4.0,
        ..Default::default()
    });
    let mut w = WeaponClass::bf1_rifle();
    w.name = "rep_weap_inf_autoturret".into();
    w.rounds_per_clip = 1;
    let src = Props(&[
        ("MaxItems", "1"),
        ("MinStrength", "0.3"),
        ("MaxStrength", "0.4"),
        ("ShotDelay", "1.5"),
    ]);
    w.turret = Some(Box::new(Dispenser::new(&mut w, &src, true, c)));
    let weapon = sim.add_weapon_class(w);
    let mut spec = UnitSpec::player_in_slot(0, 0, Vec3::new(5.0, 0.0, 5.0));
    spec.slots = vec![SlotSpec {
        slot: 4,
        channel: 1,
        weapon_class: weapon,
        clips: 2,
    }]
    .into();
    let owner = sim.spawn(&spec).unwrap();
    (sim, owner, weapon)
}

fn gun(sim: &Sim, weapon: usize) -> usize {
    sim.weapon_classes[weapon]
        .turret
        .as_ref()
        .unwrap()
        .item
        .gun_index
}

fn press(sim: &mut Sim, buttons: u32) {
    sim.step(&[PadState {
        buttons,
        ..Default::default()
    }]);
}

fn run(sim: &mut Sim, secs: f32) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..(secs / TICK_DT).round() as usize {
        sim.step(&[PadState::default()]);
        events.extend(sim.events.iter().cloned());
    }
    events
}

fn deploy(sim: &mut Sim) {
    press(sim, buttons::SECONDARY_FIRE);
    for _ in 0..120 {
        if !sim.turrets.is_empty() {
            run(sim, 3.0);
            assert!(sim.turrets[0].landed, "{:?}", sim.turrets[0].pos);
            return;
        }
        press(sim, 0);
    }
    panic!("no turret placed");
}

fn gone(events: &[Event]) -> Vec<TurretEnd> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::TurretGone { end, .. } => Some(*end),
            _ => None,
        })
        .collect()
}

fn shots(events: &[Event], gun: usize) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Shot { weapon_class, .. } if *weapon_class as usize == gun))
        .count()
}

fn enemy_ahead(sim: &mut Sim, metres: f32) -> UnitHandle {
    let at = sim.turrets[0].pos.with_y(0.0) - Vec3::Z * metres;
    sim.spawn(&UnitSpec::trooper(1, at)).unwrap()
}

#[test]
fn the_secondary_fire_throws_one_that_lands_upright_on_the_ground() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let t = &sim.turrets[0];
    assert_eq!((t.owner, t.team, t.weapon_class), (owner, 0, weapon));
    assert!(t.pos.y.abs() < 0.05, "{:?}", t.pos);
    assert!(t.up.abs_diff_eq(Vec3::Y, 1e-4));
    assert!(t.pos.z < 5.0, "thrown ahead: {:?}", t.pos);
    assert_eq!(t.health, 50.0);
    assert_eq!(sim.turret_of(owner, weapon).map(|t| t.id), Some(1));
}

#[test]
fn it_shoots_an_enemy_in_range_dead_for_its_owner() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let enemy = enemy_ahead(&mut sim, 20.0);
    let events = run(&mut sim, 15.0);
    assert!(shots(&events, gun(&sim, weapon)) >= 4);
    let kills: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Kill { shooter, victim } => Some((*shooter, *victim)),
            _ => None,
        })
        .collect();
    assert_eq!(kills, [(owner, enemy)]);
}

/// Card TKFIX (ours): the turret holds its fire while a friend of its team
/// stands on the line to its target, and shoots once he has left it. Its
/// kills are its owner's, so a shot through the friend was a team kill.
#[test]
fn it_holds_fire_through_a_friend_on_its_line() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let enemy = enemy_ahead(&mut sim, 20.0);
    let mid = sim.turrets[0].pos.with_y(0.0) - Vec3::Z * 8.0;
    let friend = sim.spawn(&UnitSpec::trooper(0, mid)).unwrap();
    let events = run(&mut sim, 10.0);
    let g = gun(&sim, weapon);
    assert_eq!(shots(&events, g), 0, "a shot with the friend on its line");
    assert!(sim
        .unit(friend)
        .is_some_and(|u| u.health.cur == u.health.max));
    assert!(sim.turrets[0].target.is_some(), "it tracks the enemy");
    sim.unit_mut(friend).unwrap().soldier.pos = mid + Vec3::X * 6.0;
    let events = run(&mut sim, 15.0);
    assert!(shots(&events, g) >= 4, "fires once the line is clear");
    assert!(events.iter().any(
        |e| matches!(e, Event::Kill { shooter, victim } if *shooter == owner && *victim == enemy)
    ));
    assert!(sim
        .unit(friend)
        .is_some_and(|u| u.health.cur == u.health.max));
}

/// Review fix 1: a turret thrown in a previous life credits its owner's
/// current life (the same unit slot), not the stale handle.
#[test]
fn a_kill_after_the_owner_respawned_credits_his_new_life() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let i = owner.index as usize;
    // A new life in the same slot: what a respawn or a class change does.
    sim.units[i].gen += 1;
    let now = sim.handle(i);
    assert_ne!(now, owner);
    let enemy = enemy_ahead(&mut sim, 20.0);
    let events = run(&mut sim, 15.0);
    assert!(shots(&events, gun(&sim, weapon)) >= 4);
    let kills: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Kill { shooter, victim } => Some((*shooter, *victim)),
            _ => None,
        })
        .collect();
    assert_eq!(kills, [(now, enemy)]);
    assert_eq!(sim.units[i].kills, 1);
    // The turret is still the old life's own: its bolts do not strike it.
    assert_eq!(sim.turrets.len(), 1);
    assert_eq!(sim.turrets[0].health, 50.0);
}

/// Review fix 1: a slot that now plays for the other team is not credited.
#[test]
fn an_item_of_a_slot_that_changed_team_credits_nobody() {
    let (mut sim, owner, _) = setup();
    let i = owner.index as usize;
    assert_eq!(sim.item_owner(owner, 0), owner);
    sim.units[i].gen += 1;
    assert_eq!(sim.item_owner(owner, 0), sim.handle(i));
    sim.units[i].team = 1;
    assert_eq!(sim.item_owner(owner, 0), owner);
}

#[test]
fn its_bolts_leave_its_own_gun_far_from_its_owner() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    sim.unit_mut(owner).unwrap().soldier.pos = Vec3::new(30.0, 0.0, 30.0);
    enemy_ahead(&mut sim, 20.0);
    let gun = gun(&sim, weapon);
    let events = run(&mut sim, 5.0);
    let origins: Vec<Vec3> = events
        .iter()
        .filter_map(|e| match e {
            Event::Shot {
                unit,
                weapon_class,
                origin,
                ..
            } if *weapon_class as usize == gun => Some((*unit, *origin)),
            _ => None,
        })
        .map(|(unit, origin)| {
            assert_eq!(unit, owner);
            origin
        })
        .collect();
    assert!(!origins.is_empty());
    let centre = sim.turrets[0].centre();
    let hand = sim.unit(owner).unwrap().muzzle();
    for o in origins {
        assert!(o.distance(centre) < 0.5, "{o} vs turret {centre}");
        assert!(o.distance(hand) > 20.0, "{o} at the owner's gun");
    }
    assert!(sim.is_item_gun(gun) && !sim.is_item_gun(weapon));
}

#[test]
fn it_tracks_but_holds_fire_beyond_the_gun_range() {
    let (mut sim, _, weapon) = setup();
    deploy(&mut sim);
    let enemy = enemy_ahead(&mut sim, 60.0);
    let events = run(&mut sim, 5.0);
    assert_eq!(shots(&events, gun(&sim, weapon)), 0);
    assert_eq!(sim.turrets[0].target, Some(Aim::Unit(enemy)));
    assert!(!sim.turrets[0].fighting);
    sim.unit_mut(enemy).unwrap().soldier.pos.z += 25.0;
    let events = run(&mut sim, 5.0);
    assert!(shots(&events, gun(&sim, weapon)) > 0);
}

#[test]
fn blasts_and_bolts_hurt_it_by_the_building_scale_until_it_explodes() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let centre = sim.turrets[0].centre();
    let mut no_buildings = ExplosionClass {
        damage: 1000.0,
        damage_inner: 2.0,
        damage_outer: 2.0,
        ..Default::default()
    };
    no_buildings.scales[HealthType::Building as usize] = 0.0;
    sim.explode(&no_buildings, centre, None, Some(1));
    assert_eq!(sim.turrets[0].health, 50.0);
    let mut rifle = WeaponClass::bf1_rifle();
    rifle.ordnance.max_damage = 100.0;
    rifle.ordnance.scales[HealthType::Building as usize] = 0.1;
    let rifle = sim.add_weapon_class(rifle);
    let who = Shooter {
        owner: UnitHandle { index: 9, gen: 0 },
        team: 1,
        weapon_class: rifle,
        is_player: false,
    };
    let from = centre - Vec3::Z * 10.0;
    let aim = crate::remote::look_frame(0.0, 0.0);
    let aim = (-aim.0, -aim.1, aim.2);
    sim.free_shots(
        who,
        from,
        aim,
        &[crate::weapon::ShotSpec {
            tilt: (0.0, 0.0),
            charge: 0.0,
            stage: 0,
            pitch_spread: 0.0,
            yaw_spread: 0.0,
        }],
    );
    run(&mut sim, 0.5);
    assert!(
        (sim.turrets[0].health - 40.0).abs() < 1e-3,
        "{}",
        sim.turrets[0].health
    );
    let blast = ExplosionClass {
        damage: 100.0,
        damage_inner: 2.0,
        damage_outer: 2.0,
        ..Default::default()
    };
    sim.explode(&blast, centre, None, Some(1));
    let events = run(&mut sim, TICK_DT);
    assert_eq!(gone(&events), [TurretEnd::Destroyed]);
    assert!(sim.turret_of(owner, weapon).is_none());
}

#[test]
fn it_expires_after_its_lifetime_and_a_second_throw_replaces_the_first() {
    let (mut sim, owner, weapon) = setup();
    deploy(&mut sim);
    let first = sim.turrets[0].id;
    sim.place_turret(owner.index as usize, weapon, 0.0);
    assert_eq!(sim.turrets.len(), 1);
    assert_ne!(sim.turrets[0].id, first);
    assert_eq!(gone(&sim.events), [TurretEnd::Replaced]);
    let age = sim.turrets[0].age;
    let events = run(&mut sim, 59.9 - age);
    assert!(gone(&events).is_empty());
    let events = run(&mut sim, 0.2);
    assert_eq!(gone(&events), [TurretEnd::Expired]);
    assert!(sim.turrets.is_empty());
}

fn enemies_at(sim: &mut Sim, centre: Vec3, n: usize) -> Vec<UnitHandle> {
    (0..n)
        .map(|k| {
            let mut spec = UnitSpec::trooper(1, centre + Vec3::new(k as f32 * 1.5 - 3.0, 0.0, 0.0));
            spec.max_health = 1.0e9;
            sim.spawn(&spec).unwrap()
        })
        .collect()
}

#[test]
fn a_bot_throws_one_only_at_a_soldier_in_a_cluster_of_his_side() {
    let (mut sim, owner, _) = setup();
    sim.unit_mut(owner).unwrap().controller = Controller::Ai;
    let centre = Vec3::new(5.0, 0.0, -25.0);
    let enemies = enemies_at(&mut sim, centre, 1);
    sim.unit_mut(owner).unwrap().ai.target = Some(enemies[0].into());
    assert_eq!(crate::ai::ability_input(&sim, owner), None);
    enemies_at(&mut sim, centre + Vec3::Z, 4);
    assert!(crate::ai::ability_input(&sim, owner).is_some());
    let friend = sim
        .spawn(&UnitSpec::trooper(0, centre + Vec3::X * 6.0))
        .unwrap();
    assert_eq!(crate::ai::ability_input(&sim, owner), None);
    sim.unit_mut(friend).unwrap().alive = false;
    assert!(crate::ai::ability_input(&sim, owner).is_some());
}

fn long_fight(cluster: usize, clips: u32, secs: f32) -> (Vec<f32>, usize, Vec<TurretEnd>) {
    let (mut sim, owner, weapon) = setup();
    let state = crate::weapon::WeaponState::with_clips(&sim.weapon_classes[weapon], false, clips);
    let u = sim.unit_mut(owner).unwrap();
    u.controller = Controller::Ai;
    u.held[0].state = state;
    enemies_at(&mut sim, Vec3::new(5.0, 0.0, -25.0), cluster);
    let (mut throws, mut most, mut ends) = (Vec::new(), 0, Vec::new());
    for _ in 0..(secs / TICK_DT).round() as usize {
        sim.step(&[PadState::default()]);
        let t = sim.tick as f32 * TICK_DT;
        throws.extend(sim.events.iter().filter_map(|e| match e {
            Event::Throw {
                unit, weapon_class, ..
            } if *unit == owner && *weapon_class as usize == weapon => Some(t),
            _ => None,
        }));
        ends.extend(gone(&sim.events));
        most = most.max(sim.turrets.iter().filter(|t| t.owner == owner).count());
    }
    (throws, most, ends)
}

#[test]
fn over_a_long_fight_a_bot_keeps_max_items_alive_and_throws_once_a_round() {
    let (throws, most, ends) = long_fight(5, 3, 150.0);
    assert_eq!(throws.len(), 3, "{throws:?}");
    assert_eq!(most, 1);
    assert_eq!(
        ends.iter().filter(|e| **e == TurretEnd::Replaced).count(),
        2,
        "{ends:?}"
    );
    for w in throws.windows(2) {
        assert!(w[1] - w[0] >= 2.0 - 1e-3, "{throws:?}");
    }
    let (throws, most, _) = long_fight(1, 3, 60.0);
    assert_eq!((throws.len(), most), (0, 0), "{throws:?}");
}

#[test]
fn turrets_are_hashed_and_replay_exactly() {
    let play = || {
        let (mut sim, _, _) = setup();
        deploy(&mut sim);
        enemy_ahead(&mut sim, 20.0);
        run(&mut sim, 4.0);
        sim
    };
    let (mut a, b) = (play(), play());
    assert_eq!(a.hash(), b.hash());
    a.turrets[0].yaw += 0.01;
    assert_ne!(a.hash(), b.hash());
}

/// PR #61 review fix 5: a landed turret whose support goes (here the ground
/// drops away under it, as a vehicle driving off does) falls and lands again.
#[test]
fn a_turret_whose_support_goes_falls_and_lands_again() {
    let (mut sim, _, _) = setup();
    deploy(&mut sim);
    assert!(sim.turrets[0].landed);
    run(&mut sim, 1.0);
    assert!(sim.turrets[0].landed, "still supported");
    if let Some(hf) = sim.heightfield.as_mut() {
        hf.heights.iter_mut().for_each(|h| *h = -5.0);
    }
    sim.step(&[PadState::default()]);
    assert!(!sim.turrets[0].landed, "no support: falling");
    run(&mut sim, 3.0);
    let t = &sim.turrets[0];
    assert!(t.landed, "{:?}", t.pos);
    assert!((t.pos.y + 5.0).abs() < 0.05, "{:?}", t.pos);
}

/// Playtest 2026-10-29 (Kamino): the turret killed an enemy building
/// turret's gunner, then kept shooting the empty, now neutral turret.
/// A vehicle is a target only while its live team is hostile; a held one
/// is dropped as soon as it is not.
#[test]
fn it_drops_a_building_turret_whose_gunner_it_killed() {
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let (mut sim, _, weapon) = setup();
    sim.respawn_delay = 1000.0;
    deploy(&mut sim);
    let at = sim.turrets[0].pos.with_y(0.0) - Vec3::Z * 20.0;
    let v = sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: at,
        yaw: 0.0,
        max_health: 100_000.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    sim.vehicle_mut(v).unwrap().aim_sphere = Some((Vec3::Y * 1.5, 1.0));
    let gunner = sim.spawn(&UnitSpec::trooper(1, at + Vec3::X)).unwrap();
    assert!(sim.seat_unit(gunner, v, 0));
    let g = gun(&sim, weapon);
    // The disc's autoturret guns take vehicles (`AITarget` mask).
    sim.weapon_classes[g].ai_target_kind |= 1 << HealthType::Vehicle as u32;
    let events = run(&mut sim, 5.0);
    assert_eq!(sim.turrets[0].target, Some(Aim::Vehicle(v)));
    assert!(shots(&events, g) >= 4, "it fights the crewed turret");

    sim.kill_environment(gunner, crate::sim::DeathCause::Fall);
    assert_eq!(sim.vehicle_team(v), None, "empty: neutral");
    run(&mut sim, 0.5);
    assert_eq!(sim.turrets[0].target, None, "the empty turret is dropped");
    // A salvo already started may finish (4 rounds, up to 0.3 s apart).
    run(&mut sim, 1.0);
    let events = run(&mut sim, 5.0);
    assert_eq!(shots(&events, g), 0, "no shot at the empty turret");
}

#[test]
fn portable_shell_blocks_standing_soldier_but_not_a_prone_soldier() {
    let (mut sim, owner, weapon) = setup();
    let class = sim.weapon_classes[weapon]
        .turret
        .as_ref()
        .unwrap()
        .item
        .clone();
    let mut t = Turret::new(
        500,
        owner,
        0,
        weapon,
        &class,
        Vec3::Y * crate::autoturret::CENTRE,
        Vec3::ZERO,
        0.0,
    );
    t.land(Vec3::ZERO, Vec3::Y);
    sim.turrets.push(t);
    let i = owner.index as usize;
    let start = Vec3::new(0.0, 0.0, 2.0);
    sim.units[i].soldier.pos = start;
    let from = sim.units[i].capsule();
    sim.units[i].soldier.pos = -Vec3::Z * 2.0;
    sim.units[i].soldier.vel = -Vec3::Z * 10.0;
    sim.push_turrets(i, from);
    assert!(
        sim.units[i].soldier.pos.z > 0.25,
        "swept contact does not tunnel"
    );
    assert_eq!(sim.units[i].soldier.vel.z, 0.0);
    sim.units[i].soldier.stance = crate::soldier::Stance::Prone;
    sim.units[i].soldier.pos = start;
    let from = sim.units[i].capsule();
    sim.units[i].soldier.pos = -Vec3::Z * 2.0;
    sim.push_turrets(i, from);
    assert!(
        (sim.units[i].soldier.pos.z + 2.0).abs() < 1e-5,
        "prone fits under shell"
    );
    sim.units[i].soldier.stance = crate::soldier::Stance::Stand;
    sim.units[i].soldier.pos = start;
    let from = sim.units[i].capsule();
    sim.units[i].soldier.pos = -Vec3::Z * 2.0;
    sim.turrets[0].health = 0.0;
    sim.push_turrets(i, from);
    assert_eq!(
        sim.units[i].soldier.pos.z, -2.0,
        "destroyed shell does not block"
    );
}

#[test]
fn player_walk_is_stopped_by_portable_shell_until_it_dies() {
    let (mut sim, owner, weapon) = setup();
    // Isolate collision retirement from the death explosion's knockback.
    sim.weapon_classes[weapon]
        .turret
        .as_mut()
        .unwrap()
        .item
        .explosion = None;
    let class = sim.weapon_classes[weapon]
        .turret
        .as_ref()
        .unwrap()
        .item
        .clone();
    let mut turret = Turret::new(
        500,
        owner,
        0,
        weapon,
        &class,
        Vec3::Y * crate::autoturret::CENTRE,
        Vec3::ZERO,
        0.0,
    );
    turret.land(Vec3::ZERO, Vec3::Y);
    sim.turrets.push(turret);
    sim.unit_mut(owner).unwrap().soldier.pos = Vec3::Z * 2.0;
    let pad = PadState {
        ly: PadState::quantize(1.0),
        ..Default::default()
    };
    for _ in 0..crate::TICK_HZ {
        sim.step(&[pad]);
    }
    assert!(
        (0.25..1.0).contains(&sim.unit(owner).unwrap().soldier.pos.z),
        "walk reached shell and wall pass stopped it: {:?}",
        sim.unit(owner).unwrap().soldier.pos
    );
    sim.turrets[0].health = 0.0;
    for _ in 0..crate::TICK_HZ {
        sim.step(&[pad]);
    }
    assert!(
        sim.unit(owner).unwrap().soldier.pos.z < -0.5,
        "dead sphere does not linger"
    );
}
