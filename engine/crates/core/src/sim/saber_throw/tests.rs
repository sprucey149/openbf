use super::*;
use crate::hero_weapon::{Blade, BladeSegment, SaberClass};
use crate::sim::tests::flat_ground;
use crate::unit::{SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::{OrdnanceClass, WeaponClass};

fn scene() -> (Sim, UnitHandle, UnitHandle) {
    let mut sim = Sim::empty(73);
    sim.heightfield = Some(flat_ground());
    let edge = BladeSegment {
        start: Vec3::ZERO,
        end: Vec3::Z,
    };
    let primary = WeaponClass {
        saber: Some(Box::new(SaberClass {
            combo: manifest::combo::ComboDef {
                name_hash: 1,
                fields: vec![],
            },
            blades: vec![Blade {
                bone: "hp_weapons".into(),
                node: "hp_fire".into(),
                length: 1.0,
                width: 0.1,
                texture: "blade".into(),
                trail_color: None,
            }],
            throw_edges: vec![edge],
            deflect_edges: vec![edge],
            weapon_bind: Mat4::from_translation(Vec3::Y * 1.15),
            throw_bindings: vec![crate::hero_weapon::SaberThrowBinding {
                weapon: "throw".into(),
                animation: "stand_throw".into(),
                seconds: 0.28,
                attachment: Mat4::from_translation(Vec3::Y * 1.15),
            }],
            clips: vec![],
            state_seconds: vec![],
            damage: 40.0,
            hit_sound: None,
            deflect_sound: None,
            hit_effect: None,
            deflect_effect: None,
        })),
        ..Default::default()
    };
    let thrown = WeaponClass {
        name: "throw".into(),
        saber_throw: Some(SaberThrow {
            energy: 60.0,
            velocity: 30.0,
            outbound_seconds: 0.1,
            return_seconds: 0.1,
            damage: 40.0,
            width: 0.75,
            length: 1.2,
            width_from_edge: false,
            length_from_edge: true,
            rotation_rate: 0.0,
        }),
        ordnance: OrdnanceClass {
            max_damage: 40.0,
            ..Default::default()
        },
        ..Default::default()
    };
    sim.weapon_classes = vec![primary, thrown].into();
    let owner = sim
        .spawn(&UnitSpec {
            slots: vec![SlotSpec {
                slot: 1,
                channel: 1,
                weapon_class: 1,
                clips: 0,
            }]
            .into(),
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    let victim = sim.spawn(&UnitSpec::trooper(1, Vec3::NEG_Z * 1.5)).unwrap();
    (sim, owner, victim)
}

#[test]
fn programmatic_throw_above_the_contact_budget_retires_without_target_queries() {
    let (mut sim, owner, victim) = scene();
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    assert!(sim.units[owner.index as usize].held[0]
        .state
        .saber_flight
        .is_some());
    // A regressed implementation does only 65 passes, rather than hanging the test.
    sim.weapon_classes[1].saber_throw.as_mut().unwrap().velocity = 960.0;
    let health = sim.unit(victim).unwrap().health.cur;
    sim.tick_saber_throws();
    assert!(sim.units[owner.index as usize].held[0]
        .state
        .saber_flight
        .is_none());
    assert_eq!(sim.unit(victim).unwrap().health.cur, health);
}

#[test]
fn thrown_saber_pays_once_uses_primary_attachment_and_releases_on_catch() {
    let (mut sim, owner, _) = scene();
    let i = owner.index as usize;
    sim.launch_saber_throw(i, Some(0), 1);
    assert_eq!(sim.unit(owner).unwrap().soldier.energy, 40.0);
    let flight = sim.unit(owner).unwrap().held[0]
        .state
        .saber_flight
        .as_ref()
        .unwrap();
    assert_eq!(flight.pos, Vec3::Y * 1.15);
    assert_eq!(flight.blade_class, 0);
    assert!(flight.transform().is_finite());
    sim.launch_saber_throw(i, Some(0), 1);
    assert_eq!(
        sim.unit(owner).unwrap().soldier.energy,
        40.0,
        "a flying saber cannot be duplicated"
    );
    for _ in 0..10 {
        sim.tick_saber_throws();
    }
    assert!(sim.unit(owner).unwrap().held[0]
        .state
        .saber_flight
        .is_none());
}

#[test]
fn constructor_cached_throw_pose_ignores_live_animation_but_tracks_owner_frame() {
    let (mut sim, owner, _) = scene();
    let i = owner.index as usize;
    let cached = sim.saber_attachment(i, 0, 1).unwrap();
    let saber = sim.weapon_classes[0].saber.as_mut().unwrap();
    saber.clips.push(crate::hero_weapon::SaberClip {
        name: "stand_idle".into(),
        root_speed: [0.0; 2],
        root_frames: vec![],
        weapon_frames: vec![Mat4::from_translation(Vec3::splat(100.0))],
        frames: vec![],
    });
    assert_eq!(sim.saber_attachment(i, 0, 1), Some(cached));
    sim.launch_saber_throw(i, Some(0), 1);
    assert_eq!(
        sim.units[i].held[0]
            .state
            .saber_flight
            .as_ref()
            .unwrap()
            .pos,
        cached.w_axis.truncate()
    );
    let unit = sim.unit_mut(owner).unwrap();
    unit.soldier.pos = Vec3::new(4.0, 2.0, -3.0);
    unit.soldier.yaw = 0.7;
    unit.soldier.time_in_stance = 20.0;
    let moved = sim.saber_attachment(i, 0, 1).unwrap();
    assert!(moved
        .w_axis
        .truncate()
        .abs_diff_eq(Vec3::new(4.0, 3.15, -3.0), 1e-6));
    assert_eq!(sim.units[i].soldier.energy, 40.0);
    sim.weapon_classes[0]
        .saber
        .as_mut()
        .unwrap()
        .throw_bindings
        .clear();
    assert!(sim.saber_attachment(i, 0, 1).is_none());
}

#[test]
fn a_struck_victim_takes_one_hit_even_when_secondary_selection_changes() {
    let (mut sim, owner, victim) = scene();
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    // The weapon state continues ticking while another channel is selected.
    sim.unit_mut(owner).unwrap().cur1 = None;
    let before = sim.unit(victim).unwrap().health.cur;
    for _ in 0..10 {
        sim.tick_saber_throws();
    }
    assert_eq!(
        sim.unit(victim).unwrap().health.cur,
        before - 80.0,
        "once a leg, out and back; no per-substep repeat"
    );
}

#[test]
fn a_flying_saber_keeps_its_damage_window_after_ordinary_owner_death() {
    let (mut sim, owner, victim) = scene();
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    let before = sim.unit(victim).unwrap().health.cur;
    let mut damage = sim.weapon_classes[0].ordnance.damage_desc(0.0, false);
    damage.damage = 400.0;
    sim.hero_damage(victim.index as usize, owner, damage, Vec3::Z);
    assert!(!sim.units[owner.index as usize].alive);
    assert_eq!(sim.units[owner.index as usize].gen, owner.gen);
    sim.tick_saber_throws();
    assert!(
        sim.units[owner.index as usize].held[0]
            .state
            .saber_flight
            .is_some(),
        "health death preserves the owner entity and flight"
    );
    for _ in 1..10 {
        sim.tick_saber_throws();
    }
    assert_eq!(sim.unit(victim).unwrap().health.cur, before - 80.0);
    assert!(sim.units[owner.index as usize].held[0]
        .state
        .saber_flight
        .is_none());
}

#[test]
fn a_flying_saber_returns_to_the_current_hidden_seat_frame() {
    let (mut sim, owner, _) = scene();
    let i = owner.index as usize;
    sim.launch_saber_throw(i, Some(0), 1);
    let vehicle = crate::sim::hero_bodies::seat_fixture(&mut sim, owner, false);
    crate::sim::hero_bodies::pose_fixture(&mut sim);
    let vehicle = sim.vehicle_mut(vehicle).unwrap();
    vehicle.pos = Vec3::new(3.0, 5.0, 7.0);
    vehicle.rot = glam::Quat::from_rotation_z(0.6);
    let expected = Mat4::from_rotation_translation(vehicle.rot, vehicle.pos)
        * Mat4::from_translation(Vec3::Y * 1.15);
    assert!(!sim.hero_body_active(i));
    assert_eq!(sim.saber_attachment(i, 0, 1), Some(expected));
    assert!(sim.units[i].held[0].state.saber_flight.is_some());
    for _ in 0..10 {
        sim.tick_saber_throws();
    }
    assert!(sim.units[i].held[0].state.saber_flight.is_none());
}

#[test]
fn despawning_retires_flights_while_health_death_preserves_them() {
    for spawn_screen in [false, true] {
        let (mut sim, owner, victim) = scene();
        let i = owner.index as usize;
        let before = sim.unit(victim).unwrap().health.cur;
        sim.launch_saber_throw(i, Some(0), 1);
        if spawn_screen {
            assert!(sim.open_spawn_screens(0, false));
        } else {
            assert!(sim.vacate_player_slot(0).is_some());
        }
        assert_eq!(sim.units[i].gen, owner.gen);
        assert!(
            sim.units[i].held[0].state.saber_flight.is_none(),
            "an explicit Despawn removes the entity without a generation change"
        );
        for _ in 0..10 {
            sim.tick_saber_throws();
        }
        assert_eq!(sim.unit(victim).unwrap().health.cur, before);
    }
}

#[test]
fn a_throw_without_primary_edge_data_cannot_spend_energy_or_create_a_fake_blade() {
    let (mut sim, owner, _) = scene();
    sim.weapon_classes[0]
        .saber
        .as_mut()
        .unwrap()
        .throw_edges
        .clear();
    let before = sim.unit(owner).unwrap().soldier.energy;
    sim.launch_saber_throw(owner.index as usize, Some(0), 1);
    assert_eq!(sim.unit(owner).unwrap().soldier.energy, before);
    assert!(sim.unit(owner).unwrap().held[0]
        .state
        .saber_flight
        .is_none());
}

mod collision;
mod reference;
mod targets;

#[test]
fn a_thrown_saber_can_damage_only_the_exposed_seated_body() {
    for exposed in [false, true] {
        let (mut sim, owner, target) = scene();
        let vehicle = crate::sim::hero_bodies::seat_fixture(&mut sim, target, exposed);
        crate::sim::hero_bodies::pose_fixture(&mut sim);
        sim.unit_mut(target).unwrap().soldier.pos = Vec3::splat(100.0);
        let before = sim.unit(target).unwrap().health.cur;
        sim.launch_saber_throw(owner.index as usize, Some(0), 1);
        for _ in 0..10 {
            sim.tick_saber_throws();
        }
        let target = sim.unit(target).unwrap();
        assert_eq!(target.health.cur, before - if exposed { 80.0 } else { 0.0 });
        assert_eq!(target.mount.unwrap().vehicle, vehicle);
    }
}

#[test]
fn off_field_bot_recycles_retire_their_independent_flight() {
    for cause in [
        crate::sim::DeathCause::Lost,
        crate::sim::DeathCause::Stalled,
    ] {
        let (mut sim, owner, _) = scene();
        sim.launch_saber_throw(owner.index as usize, Some(0), 1);
        sim.kill_environment(owner, cause);
        assert_eq!(sim.units[owner.index as usize].gen, owner.gen);
        assert!(sim.units[owner.index as usize].held[0]
            .state
            .saber_flight
            .is_none());
    }
}

/// Playtest 2026-10-30: "There should be an animation for the throw and
/// catch of the lightsaber." The catch is the melee program's CATCH_SABRE
/// state, entered by the melee update when the saber comes back with its
/// owner standing or crouched (FUN_0033cda8 B2:333228-333249; the state is
/// the combo's +0x14 slot, whose name hash 0x8718d358 is CATCH_SABRE under
/// the PS2's hash, FUN_00539308: `c | 0x20` per byte). While the saber
/// flies the program holds IDLE: FIRE starts no attack (B2:333218-333226).
#[test]
fn a_caught_saber_enters_catch_sabre_and_a_flying_one_holds_idle() {
    use crate::sim::{buttons, PadState};
    use manifest::combo::{ComboField as F, ComboKeyword as K, ComboValue as V};
    let f = |keyword, args: Vec<V>, children: Vec<F>| F {
        keyword,
        args,
        scoped: !children.is_empty(),
        children,
    };
    let t = |s: &str| V::Text(s.into());
    let (mut sim, owner, victim) = scene();
    sim.units[victim.index as usize].soldier.pos = Vec3::X * 50.0;
    let combo = manifest::combo::ComboDef {
        name_hash: 1,
        fields: vec![
            f(
                K::State,
                vec![t("CATCH_SABRE")],
                vec![
                    f(K::Animation, vec![t("stand_catch")], vec![]),
                    f(K::Duration, vec![V::Number(0.5)], vec![]),
                ],
            ),
            f(
                K::State,
                vec![t("IDLE")],
                vec![
                    f(K::Duration, vec![V::Number(0.0)], vec![]),
                    f(
                        K::Transition,
                        vec![t("ATTACK1")],
                        vec![f(
                            K::If,
                            vec![],
                            vec![
                                f(K::Break, vec![], vec![]),
                                f(K::Button, vec![t("Fire"), t("Press")], vec![]),
                            ],
                        )],
                    ),
                ],
            ),
            f(
                K::State,
                vec![t("ATTACK1")],
                vec![f(K::Duration, vec![V::Number(0.5)], vec![])],
            ),
        ],
    };
    sim.weapon_classes[0].saber.as_mut().unwrap().combo = combo.clone();
    sim.weapon_classes[0].saber.as_mut().unwrap().state_seconds = vec![0.5, 0.0, 0.5];
    let i = owner.index as usize;
    sim.step(&[PadState::default()]);
    sim.launch_saber_throw(i, Some(0), 1);
    let state = |sim: &Sim| sim.units[i].weapon.melee.as_ref().unwrap().runtime.clone();
    // Flying: FIRE does not leave IDLE.
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    assert!(
        sim.units[i].held[0].state.saber_flight.is_some(),
        "still flying"
    );
    assert!(
        state(&sim).in_state(&combo, "IDLE"),
        "no attack while it flies"
    );
    let mut caught = false;
    for _ in 0..30 {
        sim.step(&[PadState::default()]);
        if state(&sim).in_state(&combo, "CATCH_SABRE") {
            caught = true;
            break;
        }
    }
    assert!(caught, "the catch state plays");
    assert!(sim.units[i].held[0].state.saber_flight.is_none());
    assert_eq!(
        sim.units[i]
            .weapon
            .melee
            .as_ref()
            .unwrap()
            .animation
            .as_deref(),
        Some("stand_catch")
    );
}
