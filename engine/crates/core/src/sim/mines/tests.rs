use super::*;
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, PadState};
use crate::unit::{SlotSpec, UnitSpec};
use crate::weapon::{Dispenser, ExplosionClass, PropSource, WeaponClass};
use crate::TICK_DT;

struct Props;
impl PropSource for Props {
    fn prop(&self, _: &str) -> Option<&str> {
        None
    }
    fn prop_hash(&self, _: u32) -> Option<&str> {
        None
    }
}

fn setup(bf2: bool, remote: bool, life: f32) -> (Sim, usize, usize) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(flat_ground());
    let mut item = crate::mine::MineClass::from_props(&Props, bf2);
    item.life_span = life;
    item.trigger_radius = if remote || life < 60.0 { 0.0 } else { 1.0 };
    item.remote = remote;
    let cause = if life < 60.0 {
        Detonation::Expire
    } else {
        Detonation::Trigger
    };
    item.explosions[cause as usize] = Some(ExplosionClass {
        damage: if remote { 3000.0 } else { 1750.0 },
        damage_inner: if remote { 0.5 } else { 4.5 },
        damage_outer: if remote { 3.5 } else { 6.5 },
        ..Default::default()
    });
    let mut wc = WeaponClass::bf1_rifle();
    wc.mine = Some(Dispenser::new(&mut wc, &Props, bf2, item));
    wc.rounds_per_clip = 1;
    wc.max_pressed_time = if remote { 0.5 } else { 0.0 };
    wc.mine.as_mut().unwrap().max_items = if remote { 1 } else { 4 };
    wc.mine.as_mut().unwrap().strength = [0.2, 0.4];
    let weapon = sim.add_weapon_class(wc);
    let mut spec = UnitSpec::player_in_slot(0, 0, Vec3::ZERO);
    spec.slots = vec![SlotSpec {
        slot: 2,
        channel: 1,
        weapon_class: weapon,
        clips: 1,
    }]
    .into();
    let owner = sim.spawn(&spec).unwrap().index as usize;
    (sim, owner, weapon)
}

fn fire(sim: &mut Sim, pressed: bool) {
    sim.step(&[PadState {
        buttons: if pressed { buttons::SECONDARY_FIRE } else { 0 },
        ..Default::default()
    }]);
}

#[test]
fn mines_land_arm_and_replace_the_oldest_per_owner() {
    for bf2 in [false, true] {
        let (mut sim, owner, weapon) = setup(bf2, false, f32::MAX);
        sim.place_mine(owner, weapon, 0.0);
        for _ in 0..120 {
            sim.tick_mines(TICK_DT);
        }
        let first = &sim.mines[0];
        assert!(
            first.flight.armed && !first.flight.moving,
            "bf2={bf2}: {:?}",
            first.flight
        );
        assert!(first.flight.pos.z.abs() < 2.0, "{:?}", first.flight.pos);
        assert!(first.flight.pos.y.abs() < 0.03);
        let first_id = first.id;
        for _ in 0..4 {
            sim.place_mine(owner, weapon, 0.0);
        }
        assert_eq!(sim.mines.len(), 4);
        assert!(sim.mines.iter().all(|m| m.id != first_id));
        let other = sim.spawn(&UnitSpec::trooper(0, Vec3::X * 30.0)).unwrap();
        sim.place_mine(other.index as usize, weapon, 0.0);
        assert_eq!(sim.mines.len(), 5);
    }
}

#[test]
fn mine_waits_for_closest_approach_and_ignores_friends() {
    for team in [0, 1] {
        let (mut sim, owner, weapon) = setup(false, false, f32::MAX);
        sim.place_mine(owner, weapon, 0.0);
        sim.mines[0].flight.pos = Vec3::ZERO;
        sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
        let target = sim
            .spawn(&UnitSpec::trooper(team, Vec3::new(1.2, 0.0, -0.05)))
            .unwrap();
        sim.unit_mut(target).unwrap().soldier.vel = Vec3::Z;
        sim.tick_mines(TICK_DT);
        assert_eq!(sim.mines.len(), 1);
        sim.unit_mut(target).unwrap().soldier.pos.z = 0.05;
        sim.tick_mines(TICK_DT);
        assert_eq!(sim.mines.is_empty(), team == 1);
    }
}

#[test]
fn standing_enemy_triggers_and_bf1_blast_kills_at_four_metres() {
    let (mut sim, owner, weapon) = setup(false, false, f32::MAX);
    sim.place_mine(owner, weapon, 0.0);
    sim.mines[0].flight.pos = Vec3::ZERO;
    sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
    sim.spawn(&UnitSpec::trooper(1, Vec3::ZERO));
    let victim = sim.spawn(&UnitSpec::trooper(1, Vec3::X * 4.0)).unwrap();
    sim.tick_mines(TICK_DT);
    assert!(sim.mines.is_empty());
    assert!(sim.unit(victim).is_none());
}

/// Review fix 1 (PR #61): a mine laid in a previous life credits its owner's
/// current life in the same slot.
#[test]
fn a_mine_laid_before_a_respawn_credits_the_owners_new_life() {
    let (mut sim, owner, weapon) = setup(false, false, f32::MAX);
    sim.place_mine(owner, weapon, 0.0);
    sim.mines[0].flight.pos = Vec3::ZERO;
    sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
    sim.units[owner].gen += 1;
    sim.units[owner].soldier.pos = Vec3::X * 40.0;
    let now = sim.handle(owner);
    let victim = sim.spawn(&UnitSpec::trooper(1, Vec3::ZERO)).unwrap();
    sim.tick_mines(TICK_DT);
    assert!(sim.unit(victim).is_none());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::Kill { shooter, victim: v } if *shooter == now && *v == victim
    )));
    assert_eq!(sim.units[owner].kills, 1);
}

#[test]
fn timebomb_life_starts_at_placement_even_in_flight() {
    for (bf2, life, ticks) in [(false, 10.0, 300), (true, 5.0, 150)] {
        let (mut sim, owner, weapon) = setup(bf2, false, life);
        sim.place_mine(owner, weapon, 1.0);
        sim.mines[0].flight.pos.y = 1000.0;
        for _ in 0..ticks - 1 {
            sim.tick_mines(TICK_DT);
        }
        assert_eq!(sim.mines.len(), 1);
        sim.tick_mines(TICK_DT);
        assert!(sim.mines.is_empty());
        assert!(sim.events.iter().any(|e| matches!(
            e,
            Event::MineBlast {
                cause: Detonation::Expire,
                ..
            }
        )));
    }
}

#[test]
fn detpack_fresh_press_works_with_empty_ammo_and_does_not_rethrow_while_held() {
    let (mut sim, owner, weapon) = setup(true, true, 60.0);
    for _ in 0..16 {
        fire(&mut sim, true);
    }
    for _ in 0..25 {
        fire(&mut sim, false);
    }
    assert_eq!(sim.mines.len(), 1);
    assert!(sim.units[owner].held[0]
        .state
        .out_of_ammo(&sim.weapon_classes[weapon]));
    let pos = sim.mines[0].flight.pos;
    let enemy = sim.spawn(&UnitSpec::trooper(1, pos - Vec3::Y)).unwrap();
    fire(&mut sim, true);
    assert!(sim.mines.is_empty());
    assert!(sim.unit(enemy).is_none());
    assert!(sim.events.iter().any(|e| matches!(
        e,
        Event::MineBlast {
            cause: Detonation::Trigger,
            ..
        }
    )));
    for _ in 0..60 {
        fire(&mut sim, true);
    }
    fire(&mut sim, false);
    for _ in 0..25 {
        fire(&mut sim, false);
    }
    assert!(sim.mines.is_empty());
}

#[test]
fn shooting_a_detpack_destroys_it_without_trigger_damage() {
    let (mut sim, owner, weapon) = setup(true, true, 60.0);
    sim.place_mine(owner, weapon, 0.0);
    sim.mines[0].flight.pos = Vec3::new(
        0.0,
        crate::unit::muzzle_height(crate::soldier::Stance::Stand),
        -2.0,
    );
    sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
    sim.mines[0].health = 20.0;
    let other = sim
        .spawn(&UnitSpec::player_in_slot(1, 1, Vec3::new(0.0, 0.0, -4.0)))
        .unwrap();
    sim.unit_mut(other).unwrap().soldier.yaw = std::f32::consts::PI;
    let hp = sim.units[owner].health.cur;
    sim.step(&[
        PadState::default(),
        PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        },
    ]);
    for _ in 0..5 {
        sim.step(&[PadState::default(), PadState::default()]);
    }
    assert!(sim.mines.is_empty());
    assert_eq!(sim.units[owner].health.cur, hp);
}

#[test]
fn mine_hash_covers_life_health_trigger_and_attachment() {
    let (mut sim, owner, weapon) = setup(true, true, 60.0);
    sim.place_mine(owner, weapon, 0.0);
    let original = sim.hash();
    sim.mines[0].triggered = true;
    assert_ne!(sim.hash(), original);
    sim.mines[0].triggered = false;
    sim.mines[0].health -= 1.0;
    assert_ne!(sim.hash(), original);
}

#[test]
fn bf2_contact_ignores_an_engineer_and_triggers_on_an_approaching_enemy() {
    let (mut sim, owner, weapon) = setup(true, false, 60.0);
    sim.weapon_classes[weapon]
        .mine
        .as_mut()
        .unwrap()
        .item
        .contact_trigger = true;
    sim.place_mine(owner, weapon, 0.0);
    sim.mines[0].flight.pos = Vec3::ZERO;
    sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
    let enemy = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 0.1)))
        .unwrap();
    sim.unit_mut(enemy).unwrap().consts.immune_to_mines = true;
    sim.unit_mut(enemy).unwrap().soldier.vel = Vec3::NEG_Z;
    sim.tick_mines(TICK_DT);
    assert_eq!(sim.mines.len(), 1);
    sim.unit_mut(enemy).unwrap().consts.immune_to_mines = false;
    sim.tick_mines(TICK_DT);
    assert!(sim.mines.is_empty());
}

#[test]
fn a_friendly_suppression_radius_clears_the_proximity_trigger() {
    let (mut sim, owner, weapon) = setup(false, false, f32::MAX);
    sim.weapon_classes[weapon]
        .mine
        .as_mut()
        .unwrap()
        .item
        .friend_radius = 2.5;
    sim.place_mine(owner, weapon, 0.0);
    sim.mines[0].flight.pos = Vec3::ZERO;
    sim.mines[0].anchor(Vec3::Y, Anchor::World, None);
    sim.spawn(&UnitSpec::trooper(1, Vec3::X));
    sim.tick_mines(TICK_DT);
    assert_eq!(sim.mines.len(), 1);
    sim.units[owner].soldier.pos = Vec3::X * 20.0;
    sim.tick_mines(TICK_DT);
    assert!(sim.mines.is_empty());
}

#[test]
fn detonating_with_ammunition_does_not_turn_a_held_button_into_a_new_throw() {
    let (mut sim, _, _) = setup(true, true, 60.0);
    sim.cheats.infinite_ammo = true;
    sim.cheats.invincible = true;
    for _ in 0..16 {
        fire(&mut sim, true);
    }
    for _ in 0..25 {
        fire(&mut sim, false);
    }
    assert_eq!(sim.mines.len(), 1);
    for _ in 0..60 {
        fire(&mut sim, true);
    }
    fire(&mut sim, false);
    for _ in 0..25 {
        fire(&mut sim, false);
    }
    assert!(sim.mines.is_empty());
    for _ in 0..16 {
        fire(&mut sim, true);
    }
    for _ in 0..25 {
        fire(&mut sim, false);
    }
    assert_eq!(sim.mines.len(), 1);
}

#[test]
fn bf2_pack_sticks_to_a_hull_and_follows_its_position_and_rotation() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    for bf2 in [false, true] {
        let (mut sim, owner, weapon) = setup(bf2, true, 60.0);
        let vehicle = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 1.0, -1.5),
            yaw: 0.0,
            max_health: 900.0,
            health_type: crate::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let mut body = Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -1.5)),
            Shape::Box {
                half: Vec3::new(0.5, 1.0, 0.1),
            },
        );
        body.owner = Some(vehicle);
        sim.collision = CollisionWorld::new(vec![body], Vec::new());
        sim.place_mine(owner, weapon, 0.0);
        sim.mines[0].flight.pos = Vec3::Y;
        sim.mines[0].flight.vel = Vec3::NEG_Z * 30.0;
        sim.tick_mines(TICK_DT);
        sim.tick_mines(TICK_DT);
        if !bf2 {
            assert!(sim.mines[0].flight.moving);
            assert!(sim.mines[0].flight.pos.z < -1.5);
            continue;
        }
        let Some(Anchor::Vehicle { handle, local }) = sim.mines[0].flight.anchor else {
            panic!("pack did not stick");
        };
        assert_eq!(handle, vehicle);
        let rotation = sim.mines[0].anchor_rotation;
        let v = sim.vehicle_mut(vehicle).unwrap();
        v.pos.x += 3.0;
        v.rot = crate::vehicle::hover::quat_y(1.0);
        let expected = (v.pos + v.rot * local, v.rot * rotation);
        sim.tick_mines(TICK_DT);
        assert_eq!(sim.mines[0].flight.pos, expected.0);
        assert_eq!(sim.mines[0].rotation, expected.1);
    }
}

#[test]
fn a_capsule_axis_whose_bounds_cross_by_rounding_does_not_panic() {
    // The crash's numbers (a prone soldier beside a BF2 mine): lo above hi
    // by 4e-6, where `f32::clamp` panics.
    let (lo, hi) = (-62.712_704_f32, -62.712_708_f32);
    assert!(lo > hi);
    let y = clamp_axis(-60.0, lo, hi);
    assert!(y >= hi && y <= lo);
    assert_eq!(clamp_axis(5.0, 0.0, 1.0), 1.0);
}

#[test]
fn fc_mine_release_speed_bounds_preserve_endpoint_hashes() {
    for bf2 in [false, true] {
        for (raw, endpoint) in [(-1.0, 0.0), (4.0, 1.0)] {
            let (mut actual, owner, weapon) = setup(bf2, true, f32::MAX);
            let (mut control, ci, cw) = setup(bf2, true, f32::MAX);
            actual.place_mine(owner, weapon, raw);
            control.place_mine(ci, cw, endpoint);
            assert_eq!(actual.mines.len(), 1);
            assert_eq!(actual.mines[0].flight.vel, control.mines[0].flight.vel);
            assert_eq!(actual.hash(), control.hash());
        }
    }
}
