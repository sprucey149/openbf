use super::*;
use crate::collision::HullFilter;
use crate::hero_weapon::{Blade, BladeSegment, MeleeState, SaberClass, SaberClip};
use crate::projectile::{Bolt, Hit, Shooter, Target};
use crate::sim::tests::flat_ground;
use crate::unit::{UnitHandle, UnitSpec};
use crate::weapon::{ExplosionClass, WeaponClass};
use manifest::combo::{ComboDef, ComboField as F, ComboKeyword as K, ComboValue as V};

impl Sim {
    fn saber_hits(&mut self) {
        self.saber_hits_with(&[], &[]);
    }
}

pub(super) fn field(keyword: K, args: Vec<V>, children: Vec<F>) -> F {
    F {
        keyword,
        args,
        scoped: !children.is_empty(),
        children,
    }
}
pub(super) fn text(s: &str) -> V {
    V::Text(s.into())
}
pub(super) fn program(attack: Vec<F>) -> ComboDef {
    ComboDef {
        name_hash: 1,
        fields: vec![
            field(
                K::State,
                vec![text("IDLE")],
                vec![
                    field(K::Duration, vec![V::Number(0.0)], vec![]),
                    field(
                        K::Transition,
                        vec![text("ATTACK")],
                        vec![field(
                            K::If,
                            vec![],
                            vec![
                                field(K::Break, vec![], vec![]),
                                field(K::Button, vec![text("Fire"), text("Press")], vec![]),
                            ],
                        )],
                    ),
                ],
            ),
            field(K::State, vec![text("ATTACK")], attack),
        ],
    }
}
pub(super) fn saber(def: ComboDef) -> WeaponClass {
    let segment = BladeSegment {
        start: Vec3::new(0.0, 0.9, 0.0),
        end: Vec3::new(0.0, 0.9, 1.2),
    };
    WeaponClass {
        saber: Some(Box::new(SaberClass {
            combo: def,
            blades: vec![Blade {
                node: "edge".into(),
                bone: "hand".into(),
                length: 1.2,
                width: 0.1,
                texture: "blade".into(),
                trail_color: None,
            }],
            clips: vec![SaberClip {
                name: "swing".into(),
                root_speed: [0.0; 2],
                root_frames: vec![Vec3::ZERO; 31],
                weapon_frames: Vec::new(),
                frames: vec![vec![segment]; 31],
            }],
            state_seconds: vec![0.0, 1.0],
            hand_edges: Vec::new(),
            throw_edges: Vec::new(),
            deflect_edges: Vec::new(),
            weapon_bind: glam::Mat4::IDENTITY,
            throw_bindings: Vec::new(),
            damage: 100.0,
            hit_sound: None,
            deflect_sound: None,
            hit_effect: None,
            deflect_effect: None,
        })),
        ..WeaponClass::default()
    }
}
fn scene() -> (Sim, UnitHandle, UnitHandle) {
    let mut sim = Sim::empty(71);
    sim.heightfield = Some(flat_ground());
    sim.ground_rules = crate::soldier::GroundRules::Bf2;
    sim.weapon_classes = vec![saber(program(vec![])), WeaponClass::bf1_rifle()].into();
    let defender = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let shooter = sim
        .spawn(&UnitSpec {
            weapon_class: 1,
            ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -5.0))
        })
        .unwrap();
    let u = sim.unit_mut(defender).unwrap();
    u.soldier.energy = 100.0;
    u.weapon.melee = Some(MeleeState {
        deflect: Some(combo::Deflect {
            min_angle: -110.0,
            max_angle: 110.0,
            energy_cost: 2.5,
        }),
        ..Default::default()
    });
    (sim, defender, shooter)
}

#[test]
fn tracked_saber_hits_once_per_stroke_and_rearms_on_the_backswing() {
    use crate::sim::{TrackedCombat, TrackedHand};
    use glam::Quat;
    let (mut sim, wielder, target) = scene();
    let class = sim.weapon_classes[0].saber.as_mut().unwrap();
    class.damage = 0.0;
    class.combo = program(vec![field(
        K::Attack,
        vec![],
        vec![field(K::Damage, vec![V::Number(90.0)], vec![])],
    )]);
    class.blades[0].bone = "hp_weapons".into();
    class.hand_edges.push(BladeSegment {
        start: Vec3::ZERO,
        end: Vec3::Z * 1.2,
    });
    sim.unit_mut(target).unwrap().soldier.pos = Vec3::new(0.0, 0.0, 0.9);
    let health = sim.unit(target).unwrap().health.cur;
    let sample = |sim: &mut Sim, x, motion_epoch| {
        let mut pads = vec![PadState::default(); sim.units.len()];
        pads[wielder.index as usize].tracked_combat = Some(TrackedCombat {
            hands: [
                None,
                Some(TrackedHand::new(Vec3::new(x, 0.9, 0.0), Quat::IDENTITY)),
            ],
            motion_epoch,
            grenade: None,
            ..TrackedCombat::default()
        });
        sim.events.clear();
        sim.saber_hits_with(&pads, &[]);
        sim.tick += 1;
        sim.unit(target).unwrap().health.cur
    };
    assert_eq!(sample(&mut sim, -0.6, 1), health);
    assert_eq!(
        sample(&mut sim, 0.6, 2),
        health,
        "a snap turn is not a swing"
    );
    let hit = sample(&mut sim, -0.25, 2);
    assert_eq!(
        hit,
        health - 90.0,
        "the blade crosses the target and deals the authored attack damage"
    );
    assert!(sim.events.iter().any(
        |event| matches!(event, crate::sim::Event::SaberHandContact {
        attacker, hand: 1, blocked: false, ..
    } if *attacker == wielder)
    ));
    assert_eq!(sample(&mut sim, -0.8, 2), hit, "same stroke");
    assert!(!sim
        .events
        .iter()
        .any(|event| matches!(event, crate::sim::Event::SaberHandContact { .. })));
    for _ in 0..10 {
        assert_eq!(sample(&mut sim, -0.8, 2), hit);
    }
    let hit = sample(&mut sim, 0.8, 2);
    assert_eq!(hit, health - 180.0);
    for _ in 0..11 {
        assert_eq!(sample(&mut sim, 0.8, 2), hit);
    }
    let hit = sample(&mut sim, -0.8, 2);
    assert_eq!(hit, health - 270.0);
    for _ in 0..11 {
        assert_eq!(sample(&mut sim, -0.8, 2), hit);
    }

    let defender = sim.unit_mut(target).unwrap();
    defender.weapon_class = 0;
    defender.soldier.energy = 100.0;
    defender.weapon.melee = Some(MeleeState {
        deflect: Some(combo::Deflect {
            min_angle: -110.0,
            max_angle: 110.0,
            energy_cost: 2.5,
        }),
        ..Default::default()
    });
    assert_eq!(sample(&mut sim, 0.8, 2), hit, "the enemy blocked the cut");
    assert!(sim.events.iter().any(
        |event| matches!(event, crate::sim::Event::SaberHandContact {
        attacker, hand: 1, blocked: true, ..
    } if *attacker == wielder)
    ));
}

#[test]
fn tracked_saber_keeps_the_rotating_tip_and_a_between_tick_wrist_cut() {
    use crate::sim::{TrackedCombat, TrackedHand};
    use glam::Quat;
    let (mut sim, wielder, target) = scene();
    let class = sim.weapon_classes[0].saber.as_mut().unwrap();
    class.blades[0].bone = "hp_weapons".into();
    class.blades[0].width = 0.04;
    class.hand_edges.push(BladeSegment {
        start: Vec3::ZERO,
        end: Vec3::Z * 1.2,
    });
    sim.unit_mut(target).unwrap().soldier.pos = Vec3::Z * 1.5;
    let health = sim.unit(target).unwrap().health.cur;
    let pose = |yaw| TrackedHand::new(Vec3::Y * 1.64, Quat::from_rotation_y(yaw));
    let start = pose(-1.4);
    let end = pose(1.4);
    let mut pads = [PadState::default(); 2];
    let i = wielder.index as usize;
    pads[i].tracked_combat = Some(TrackedCombat {
        hands: [None, Some(start)],
        ..TrackedCombat::default()
    });
    sim.saber_hits_with(&pads, &[]);
    sim.tick += 1;
    assert_eq!(sim.unit(target).unwrap().health.cur, health);

    // Both fixed-tick samples are at the same pose. The intermediate OpenXR
    // sample records the excursion, including the rotation through the tip.
    pads[i].tracked_combat.as_mut().unwrap().waypoints[1] = Some(end);
    sim.saber_hits_with(&pads, &[]);
    assert_eq!(sim.unit(target).unwrap().health.cur, health - 100.0);
    sim.tick += 1;
    assert!(pads[i].has_once());
    pads[i].clear_once();
    sim.saber_hits_with(&pads, &[]);
    assert_eq!(sim.unit(target).unwrap().health.cur, health - 100.0);
}

#[test]
fn tracked_saber_hits_moving_enemies_beside_the_player() {
    use crate::sim::{TrackedCombat, TrackedHand};
    use glam::Quat;

    for side in [-1.0, 1.0] {
        let (mut sim, wielder, target) = scene();
        let class = sim.weapon_classes[0].saber.as_mut().unwrap();
        class.blades[0].bone = "hp_weapons".into();
        class.hand_edges.push(BladeSegment {
            start: Vec3::ZERO,
            end: Vec3::Z * 1.2,
        });
        let rotation = Quat::from_rotation_y(side * std::f32::consts::FRAC_PI_2);
        let pad = |z| PadState {
            tracked_combat: Some(TrackedCombat {
                hands: [
                    None,
                    Some(TrackedHand::new(Vec3::new(side * 0.35, 1.15, z), rotation)),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let victim = sim.unit_mut(target).unwrap();
        victim.soldier.pos = Vec3::new(side * 1.2, 0.0, 0.29);
        let health = victim.health.cur;
        sim.saber_hits_with(&[pad(0.0)], &[]);
        sim.tick += 1;
        let pads = [pad(-0.18)];
        let starts = sim.saber_target_starts(&pads);
        sim.unit_mut(target).unwrap().soldier.pos.z = 0.59;
        let mut without_history = sim.clone();
        without_history.saber_hits_with(&pads, &[]);
        assert_eq!(without_history.unit(target).unwrap().health.cur, health);
        sim.saber_hits_with(&pads, &starts);
        assert_eq!(
            sim.unit(target).unwrap().health.cur,
            health - 100.0,
            "side {side}"
        );
        assert_eq!(sim.unit(wielder).unwrap().soldier.yaw, 0.0);
    }
}

#[test]
fn tracked_saber_cut_from_the_side_is_outside_a_forward_guard() {
    use crate::sim::{TrackedCombat, TrackedHand};
    use glam::Quat;

    for side in [-1.0, 1.0] {
        let (mut sim, _, target) = scene();
        let class = sim.weapon_classes[0].saber.as_mut().unwrap();
        class.blades[0].bone = "hp_weapons".into();
        class.hand_edges.push(BladeSegment {
            start: Vec3::ZERO,
            end: Vec3::Z * 1.2,
        });
        let defender = sim.unit_mut(target).unwrap();
        defender.weapon_class = 0;
        defender.soldier.pos = Vec3::X * (side * 1.2);
        defender.soldier.yaw = std::f32::consts::PI;
        defender.soldier.energy = 100.0;
        defender.weapon.melee = Some(MeleeState {
            deflect: Some(combo::Deflect {
                min_angle: -30.0,
                max_angle: 30.0,
                energy_cost: 2.5,
            }),
            ..Default::default()
        });
        let health = defender.health.cur;
        for z in [0.6, -0.6] {
            let palm = Vec3::new(side * 0.25, 1.15, z);
            let rotation = Quat::from_rotation_y(side * std::f32::consts::FRAC_PI_2);
            sim.saber_hits_with(
                &[PadState {
                    tracked_combat: Some(TrackedCombat {
                        hands: [None, Some(TrackedHand::new(palm, rotation))],
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                &[],
            );
            sim.tick += 1;
        }
        let defender = sim.unit(target).unwrap();
        assert_eq!(defender.health.cur, health - 100.0, "side {side}");
        assert_eq!(defender.soldier.energy, 100.0);
    }
}
fn tracked_duel() -> (Sim, UnitHandle, UnitHandle) {
    let (mut sim, wielder, target) = scene();
    let class = sim.weapon_classes[0].saber.as_mut().unwrap();
    class.blades[0].bone = "hp_weapons".into();
    class.hand_edges.push(BladeSegment {
        start: Vec3::ZERO,
        end: Vec3::Z * 1.2,
    });
    sim.unit_mut(target).unwrap().soldier.pos = Vec3::Z;
    (sim, wielder, target)
}

fn sample_tracked_duel(sim: &mut Sim, at: Vec3, rotation: glam::Quat) {
    use crate::sim::{TrackedCombat, TrackedHand};

    sim.saber_hits_with(
        &[PadState {
            tracked_combat: Some(TrackedCombat {
                hands: [None, Some(TrackedHand::new(at, rotation))],
                ..TrackedCombat::default()
            }),
            ..PadState::default()
        }],
        &[],
    );
    sim.tick += 1;
}

#[test]
fn tracked_saber_can_return_a_cut_during_a_standing_flinch() {
    let (mut sim, wielder, target) = tracked_duel();
    let left = Vec3::new(-0.7, 1.15, 0.0);
    let right = Vec3::new(0.7, 1.15, 0.0);
    let straight = glam::Quat::IDENTITY;
    sample_tracked_duel(&mut sim, left, straight);
    sample_tracked_duel(&mut sim, right, straight);
    let health = sim.unit(target).unwrap().health.cur;
    assert_eq!(health, 200.0, "the opening cut landed");
    for _ in 0..11 {
        sample_tracked_duel(&mut sim, right, straight);
    }

    let mut recovering = sim.clone();
    let recovery = &mut recovering.unit_mut(wielder).unwrap().soldier;
    recovery.fire_lock = true;
    recovery.lock_timer = crate::soldier::GETUP_LOCK;
    let counter = |sim: &mut Sim| {
        sim.unit_mut(wielder).unwrap().soldier.grounded = true;
        sim.hero_knock(wielder, Vec3::NEG_Z, 3.0, target);
        assert!(sim.unit(wielder).unwrap().soldier.no_fire());
    };
    counter(&mut recovering);
    sample_tracked_duel(&mut recovering, left, straight);
    assert_eq!(recovering.unit(target).unwrap().health.cur, health);

    counter(&mut sim);
    assert!(!sim.unit(wielder).unwrap().soldier.stance.knocked());
    let mut choked = sim.clone();
    choked
        .unit_mut(wielder)
        .unwrap()
        .soldier
        .choke_release_lock();
    sample_tracked_duel(&mut choked, left, straight);
    assert_eq!(choked.unit(target).unwrap().health.cur, health);

    let mut down = sim.clone();
    down.unit_mut(wielder).unwrap().soldier.stance = crate::soldier::Stance::Tumble;
    sample_tracked_duel(&mut down, left, straight);
    assert_eq!(down.unit(target).unwrap().health.cur, health);

    sample_tracked_duel(&mut sim, left, straight);
    assert_eq!(sim.unit(target).unwrap().health.cur, health - 100.0);
}

#[test]
fn tracked_saber_rearms_on_a_rounded_return() {
    let (mut sim, _, target) = tracked_duel();
    let health = sim.unit(target).unwrap().health.cur;
    // Two passes through the defender, separated by a continuous rounded
    // return. Every adjacent motion pair is less than 90 degrees apart.
    for eighth in -2..=18 {
        let yaw = eighth as f32 * std::f32::consts::FRAC_PI_8;
        let at = Vec3::Y * 1.15 + glam::Quat::from_rotation_y(yaw) * Vec3::Z * 0.95;
        sample_tracked_duel(&mut sim, at, glam::Quat::IDENTITY);
        if eighth == 2 {
            assert_eq!(
                sim.unit(target).map(|unit| unit.health.cur),
                Some(health - 100.0)
            );
        }
    }
    assert_eq!(
        sim.unit(target).map(|unit| unit.health.cur),
        Some(health - 200.0)
    );
}

#[test]
fn tracked_saber_keeps_both_edges_in_one_stroke() {
    let (mut sim, _, target) = tracked_duel();
    let class = sim.weapon_classes[0].saber.as_mut().unwrap();
    class.blades.push(class.blades[0].clone());
    class.hand_edges.push(BladeSegment {
        start: Vec3::ZERO,
        end: -Vec3::Z * 1.2,
    });
    let defender = sim.unit_mut(target).unwrap();
    defender.soldier.pos = -Vec3::Z;
    let health = defender.health.cur;
    // The rear edge lands the cut while both edges continue rotating.
    for yaw in [-0.7, -0.4, -0.2, 0.0, 0.2, 0.4, 0.7] {
        sample_tracked_duel(&mut sim, Vec3::Y * 1.15, glam::Quat::from_rotation_y(yaw));
    }
    assert_eq!(
        sim.unit(target).map(|unit| unit.health.cur),
        Some(health - 100.0)
    );
}

#[test]
fn tracked_saber_thrust_cooldowns_are_independent_per_hand_and_target() {
    use crate::sim::{TrackedCombat, TrackedHand};

    let (mut sim, _, target) = tracked_duel();
    let class = sim.weapon_classes[0].saber.as_mut().unwrap();
    let mut offhand = class.blades[0].clone();
    offhand.bone = "bone_l_hand".into();
    class.blades.push(offhand);
    class.hand_edges.push(class.hand_edges[0]);
    let other = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.2, 0.0, 1.0)))
        .unwrap();
    for handle in [target, other] {
        let health = &mut sim.unit_mut(handle).unwrap().health;
        health.max = 500.0;
        health.cur = health.max;
    }
    let sample = |sim: &mut Sim, z: [f32; 2], motion_epoch| {
        let hands = z.map(|z| {
            Some(TrackedHand::new(
                Vec3::new(0.0, 1.15, z),
                glam::Quat::IDENTITY,
            ))
        });
        sim.events.clear();
        sim.saber_hits_with(
            &[PadState {
                tracked_combat: Some(TrackedCombat {
                    hands,
                    motion_epoch,
                    ..TrackedCombat::default()
                }),
                ..PadState::default()
            }],
            &[],
        );
        sim.tick += 1;
        let health = [target, other].map(|handle| sim.unit(handle).unwrap().health.cur);
        let hits = sim
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sim::Event::SaberHandContact {
                    hand,
                    blocked: false,
                    ..
                } => Some(*hand),
                _ => None,
            })
            .collect::<Vec<_>>();
        (health, hits)
    };
    assert_eq!(sample(&mut sim, [-1.0; 2], 0), ([500.0; 2], vec![]));
    assert_eq!(sample(&mut sim, [0.0, -1.0], 0), ([400.0; 2], vec![0; 2]));
    assert_eq!(sample(&mut sim, [-1.0, 0.0], 0), ([300.0; 2], vec![1; 2]));
    for attempt in 2..12 {
        let z = if attempt % 2 == 0 { 0.0 } else { -1.0 };
        assert_eq!(
            sample(&mut sim, [z; 2], 1),
            ([300.0; 2], vec![]),
            "tick {attempt} after left hit"
        );
    }
    assert_eq!(sample(&mut sim, [0.0; 2], 1), ([200.0; 2], vec![0; 2]));
    assert_eq!(sample(&mut sim, [-1.0; 2], 1), ([100.0; 2], vec![1; 2]));
}

fn impact(sim: &Sim, defender: UnitHandle, owner: UnitHandle, direction: Vec3) -> Bolt {
    let mut bolt = Bolt::fire(
        sim.unit(defender).unwrap().muzzle(),
        direction,
        80.0,
        2.0,
        Shooter {
            owner,
            team: 1,
            weapon_class: 1,
            is_player: false,
        },
        None,
        None,
        &sim.collision,
        HullFilter::All,
    );
    bolt.age = 0.6;
    bolt.hit = Hit::Target(Target::Unit(defender));
    bolt
}

#[test]
fn block_resolves_before_damage_and_redirects_toward_the_defenders_aim() {
    let (mut sim, defender, owner) = scene();
    sim.unit_mut(defender).unwrap().soldier.pitch = 0.4;
    let health = sim.unit(defender).unwrap().health.cur;
    let bolt = impact(&sim, defender, owner, Vec3::Z);
    sim.resolve_bolt(&bolt);
    let u = sim.unit(defender).unwrap();
    assert_eq!(u.health.cur, health);
    assert_eq!(u.soldier.energy, 97.5);
    let reflected = sim
        .bolts
        .last()
        .expect("block must create the redirected projectile");
    assert_eq!((reflected.owner, reflected.team), (defender, 0));
    assert_eq!((reflected.age, reflected.charge), (0.0, 1.0));
    assert!(
        reflected.vel.y > 5.0,
        "aimed up, not a mirror about a horizontal normal: {:?}",
        reflected.vel
    );
    assert!(reflected.vel.z < 0.0);
    assert!(!sim
        .events
        .iter()
        .any(|e| matches!(e, super::super::Event::Hit { .. })));
}

#[test]
fn rear_and_depleted_impacts_are_not_blocked_and_dead_heroes_cannot_block() {
    for (direction, energy, depleted) in [(Vec3::NEG_Z, 100.0, false), (Vec3::Z, 2.49, true)] {
        let (mut sim, defender, owner) = scene();
        sim.unit_mut(defender).unwrap().soldier.energy = energy;
        sim.unit_mut(defender).unwrap().soldier.energy_depleted = depleted;
        let before = sim.unit(defender).unwrap().health.cur;
        let bolt = impact(&sim, defender, owner, direction);
        sim.resolve_bolt(&bolt);
        assert!(sim.unit(defender).unwrap().health.cur < before);
        assert!(sim.bolts.is_empty());
        assert_eq!(sim.unit(defender).unwrap().soldier.energy, energy);
    }
    let (mut sim, defender, _) = scene();
    sim.unit_mut(defender).unwrap().alive = false;
    assert!(!sim.saber_deflect(defender, Vec3::Z, Vec3::ZERO));
}

#[test]
fn a_combo_blast_damages_once_on_state_entry_through_the_normal_blast_path() {
    let (mut sim, player, target) = scene();
    let blast = ExplosionClass {
        damage: 40.0,
        damage_inner: 8.0,
        damage_outer: 8.0,
        ..Default::default()
    };
    let mut weapon = saber(program(vec![
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::PlayExplosion, vec![], vec![]),
    ]));
    weapon.ordnance.explosion = Some(blast);
    sim.weapon_classes[0] = weapon;
    sim.unit_mut(player).unwrap().weapon.melee = None;
    let before = sim.unit(target).unwrap().health.cur;
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    assert_eq!(sim.unit(target).unwrap().health.cur, before - 40.0);
    assert_eq!(
        sim.events
            .iter()
            .filter(|e| matches!(e, super::super::Event::WeaponBlast { .. }))
            .count(),
        1
    );
    sim.step(&[PadState::default()]);
    assert_eq!(sim.unit(target).unwrap().health.cur, before - 40.0);
    assert!(!sim
        .events
        .iter()
        .any(|e| matches!(e, super::super::Event::WeaponBlast { .. })));
}

#[test]
fn each_swing_hits_a_body_once_and_an_active_block_prevents_that_damage() {
    for blocked in [false, true] {
        let (mut sim, player, target) = scene();
        sim.weapon_classes[0] = saber(program(vec![
            field(K::Animation, vec![text("swing")], vec![]),
            field(
                K::Attack,
                vec![],
                vec![
                    field(
                        K::DamageTime,
                        vec![V::Number(0.0), V::Number(30.0), text("Frames")],
                        vec![],
                    ),
                    field(K::Damage, vec![V::Number(100.0)], vec![]),
                ],
            ),
        ]));
        sim.unit_mut(player).unwrap().weapon.melee = None;
        let victim = sim.unit_mut(target).unwrap();
        victim.soldier.pos = Vec3::new(0.0, 0.0, -1.0);
        victim.soldier.yaw = std::f32::consts::PI;
        if blocked {
            victim.weapon_class = 0;
            victim.weapon.melee = Some(MeleeState {
                deflect: Some(combo::Deflect {
                    min_angle: -110.0,
                    max_angle: 110.0,
                    energy_cost: 2.5,
                }),
                ..Default::default()
            });
        }
        let before = sim.unit(target).unwrap().health.cur;
        let mut pads = [PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }];
        // Run the production evaluator and sweep. The idle target's manually
        // installed block is retained by calling the two owned phases here.
        sim.saber_inputs(&mut pads);
        sim.saber_hits();
        sim.saber_hits();
        assert_eq!(
            sim.unit(target).unwrap().health.cur,
            before - if blocked { 0.0 } else { 100.0 },
            "blocked={blocked}"
        );
    }
}

#[test]
fn a_partial_bar_can_pay_one_last_block_and_zero_cost_bypasses_depletion() {
    let (mut sim, defender, owner) = scene();
    let before = sim.unit(defender).unwrap().health.cur;
    sim.unit_mut(defender).unwrap().soldier.energy = 1.0;
    let bolt = impact(&sim, defender, owner, Vec3::Z);
    sim.resolve_bolt(&bolt);
    let u = sim.unit(defender).unwrap();
    assert_eq!(u.health.cur, before);
    assert_eq!(u.soldier.energy, 0.0);
    assert!(u.soldier.energy_depleted);
    sim.resolve_bolt(&bolt);
    assert!(sim.unit(defender).unwrap().health.cur < before);
    let u = sim.unit_mut(defender).unwrap();
    u.weapon
        .melee
        .as_mut()
        .unwrap()
        .deflect
        .as_mut()
        .unwrap()
        .energy_cost = 0.0;
    assert!(sim.saber_deflect(defender, Vec3::Z, Vec3::ZERO));
    assert!(sim.unit(defender).unwrap().soldier.energy_depleted);
}

mod alignment;
mod contact;
mod deflect;
mod riders;
mod targets;
mod turning;

#[test]
fn a_deflected_blade_contact_consumes_the_victim_cache_before_damage() {
    let (mut sim, defender, attacker) = scene();
    let a = sim.unit_mut(attacker).unwrap();
    a.weapon_class = 0;
    a.soldier.pos = Vec3::NEG_Z;
    a.soldier.yaw = std::f32::consts::PI;
    let attack = crate::combo::AttackWindow {
        index: 0,
        id: 3,
        record: 12,
        cache_end: 1.0,
        edge: 0,
        damage: Some(100.0),
        length: Some(1.2),
        length_from_edge: false,
        width: Some(0.1),
        width_from_edge: false,
        push: 0.0,
        play_explosion: false,
    };
    a.weapon.melee = Some(MeleeState {
        animation: Some("swing".into()),
        attacks: vec![attack],
        previous_position: a.soldier.pos,
        previous_yaw: a.soldier.yaw,
        ..Default::default()
    });
    let before = sim.unit(defender).unwrap().health.cur;
    sim.saber_hits();
    assert_eq!(sim.unit(defender).unwrap().health.cur, before);
    assert_eq!(
        sim.unit(defender).unwrap().soldier.energy,
        97.5,
        "real deflect, not a missed blade"
    );
    assert!(sim
        .unit(attacker)
        .unwrap()
        .weapon
        .melee
        .as_ref()
        .unwrap()
        .already_hit(&attack, Target::Unit(defender)));
    sim.unit_mut(defender)
        .unwrap()
        .weapon
        .melee
        .as_mut()
        .unwrap()
        .deflect = None;
    sim.saber_hits();
    assert_eq!(
        sim.unit(defender).unwrap().health.cur,
        before,
        "same hit window cannot retry after block"
    );
}

mod interruption;

/// Playtest 2026-10-30 (Mace on Coruscant, F9 `prev1.json` ticks
/// 3194-3240): the right stick held for 1.5 s of swings turned nothing - the
/// combo's "InputLock All" holds Turn - while the window drew the stick's
/// turn between ticks and snapped back at every tick: the shaking camera.
/// The preview draws what the tick does.
/// [ours] (the maintainer, 2026-10-30): a human turns through every combo
/// state, so his preview draws the tick's whole turn, locked or not (no
/// jitter: what is drawn is what the tick takes). A bot draws nothing.
#[test]
fn the_look_preview_draws_the_turn_a_human_takes_through_a_locked_combo_state() {
    for lock in [true, false] {
        let mut attack = vec![
            field(K::Animation, vec![text("swing")], vec![]),
            field(K::Duration, vec![V::Number(1.0)], vec![]),
        ];
        attack.push(if lock {
            field(K::InputLock, vec![text("All"), text("!Thrust")], vec![])
        } else {
            field(K::TurnFactor, vec![V::Number(0.5)], vec![])
        });
        let (mut sim, player, _) = scene();
        sim.weapon_classes[0] = saber(program(attack));
        sim.unit_mut(player).unwrap().weapon.melee = None;
        sim.step(&[PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
        sim.step(&[PadState::default()]);
        let start = sim.unit(player).unwrap().soldier.yaw;
        for pad in [
            PadState {
                rx: 20000,
                ..Default::default()
            },
            PadState {
                mx: 3000,
                buttons: buttons::MOUSE_AIM,
                ..Default::default()
            },
        ] {
            for _ in 0..5 {
                let preview = sim.look_preview(player, &pad);
                sim.step(&[pad]);
                let after = sim.unit(player).unwrap().soldier.yaw;
                match preview {
                    Some(crate::sim::LookPreview::Foot { yaw, .. }) => {
                        assert_eq!(yaw, after, "lock {lock}: the drawn turn is the tick's")
                    }
                    other => panic!("lock {lock}: {other:?}"),
                }
            }
        }
        assert_ne!(sim.unit(player).unwrap().soldier.yaw, start, "lock {lock}");
    }
}

#[test]
fn a_reflected_missile_retains_its_native_flight_state_without_a_lock() {
    let (mut sim, defender, owner) = scene();
    let ord = &mut sim.weapon_classes[1].ordnance;
    ord.kind = crate::weapon::OrdKind::Missile;
    ord.missile = Some(crate::missile::MissileClass::default());
    let bolt = impact(&sim, defender, owner, Vec3::Z);
    sim.resolve_bolt(&bolt);
    let reflected = sim.bolts.last().unwrap();
    assert_eq!(reflected.owner, defender);
    assert!(reflected.missile.is_some());
    assert!(reflected.missile.unwrap().target.is_none());
    assert!(sim.beams.is_empty());
}

#[test]
fn a_held_beam_pays_the_native_deflect_cost_each_contact_tick() {
    let (mut sim, defender, owner) = scene();
    sim.weapon_classes[1].ordnance.kind = crate::weapon::OrdKind::Beam;
    sim.weapon_classes[1].ordnance.beam = Some(crate::weapon::BeamClass {
        range: 20.0,
        life: 0.15,
        pierce: false,
        impact: None,
    });
    let bolt = impact(&sim, defender, owner, Vec3::Z);
    let health = sim.unit(defender).unwrap().health.cur;
    for _ in 0..3 {
        sim.resolve_bolt(&bolt);
    }
    assert_eq!(sim.unit(defender).unwrap().health.cur, health);
    assert_eq!(sim.unit(defender).unwrap().soldier.energy, 92.5);
    assert!(sim.beams.is_empty());
}

#[test]
fn a_fat_ray_block_spares_the_defender_but_the_ray_reaches_the_next_body() {
    let (mut sim, defender, owner) = scene();
    sim.weapon_classes[1].ordnance.fat_ray = Some(crate::weapon::FatRayClass {
        max_damage: 100.0,
        min_damage: 10.0,
        damage_deduction: 5.0,
        radius: 1.0,
    });
    let behind = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 1.0)))
        .unwrap();
    let before = sim.unit(defender).unwrap().health.cur;
    let mut ray = impact(&sim, defender, owner, Vec3::Z);
    ray.pos = Vec3::new(0.0, 1.4, -2.0);
    ray.vel = Vec3::Z * 100.0;
    ray.hit = Hit::None;
    sim.bolts.push(ray);
    sim.sweep_fat_rays(crate::TICK_DT);
    assert_eq!(sim.unit(defender).unwrap().health.cur, before);
    assert_eq!(sim.unit(defender).unwrap().soldier.energy, 97.5);
    assert_eq!(sim.unit(behind).unwrap().health.cur, 205.0);
    assert!(
        sim.bolts.len() > 1,
        "accepted block creates reflected ordnance"
    );
}

/// A saber block ends a PassThrough beam: the defender takes nothing and
/// the soldier behind him is not reached.
#[test]
fn a_saber_block_ends_a_pass_through_beam() {
    let (mut sim, defender, owner) = scene();
    sim.weapon_classes[1].ordnance.kind = crate::weapon::OrdKind::Beam;
    sim.weapon_classes[1].ordnance.beam = Some(crate::weapon::BeamClass {
        range: 50.0,
        life: 0.15,
        pierce: true,
        impact: None,
    });
    let behind = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 5.0)))
        .unwrap();
    let (health, behind_health) = (
        sim.unit(defender).unwrap().health.cur,
        sim.unit(behind).unwrap().health.cur,
    );
    let from = sim.unit(owner).unwrap().muzzle();
    let round = Bolt::fire(
        from,
        Vec3::Z,
        50.0,
        1.0,
        Shooter {
            owner,
            team: 1,
            weapon_class: 1,
            is_player: false,
        },
        None,
        None,
        &sim.collision,
        HullFilter::All,
    );
    sim.launch_round(round, false);
    sim.fly_bolts(crate::TICK_DT);
    assert_eq!(sim.unit(defender).unwrap().health.cur, health);
    assert!(
        sim.unit(defender).unwrap().soldier.energy < 100.0,
        "blocked"
    );
    assert_eq!(sim.unit(behind).unwrap().health.cur, behind_health);
    let len = sim.beams[0].len;
    assert!(len < 5.0, "drawn to the defender: {len}");
}
