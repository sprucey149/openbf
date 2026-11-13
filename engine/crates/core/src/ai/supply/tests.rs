use crate::heightfield::Heightfield;
use crate::sim::{Event, ItemClass, PadState, Sim};
use crate::soldier::GroundRules;
use crate::unit::{Controller, SlotSpec, UnitHandle, UnitSpec};
use crate::weapon::{Dispenser, Thrown, WeaponClass};
use glam::Vec3;

fn world(bf2: bool) -> (Sim, UnitHandle, usize) {
    let mut s = Sim::empty(19);
    s.ground_rules = if bf2 {
        GroundRules::Bf2
    } else {
        GroundRules::Bf1
    };
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let pack = s.add_weapon_class(WeaponClass {
        name: "supply".into(),
        rounds_per_clip: 1,
        reload_time: 1.0,
        trigger_single: true,
        shot_delay: [1.0; 2],
        initial_salvo_delay: if bf2 { 0.5 } else { 0.0 },
        thrown: Some(Thrown {
            shot_elevate: 0.0,
            hide_on_fire: true,
            hand: bf2.then_some(Dispenser::BF2_HAND),
            native_thermal_hand: false,
        }),
        dispenser: Some(Dispenser {
            item: ItemClass::basic("dual", 100.0, 2),
            velocity: 10.0,
            strength: [0.3, 0.4],
            max_items: 4,
        }),
        ..Default::default()
    });
    let bot = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            slots: vec![SlotSpec {
                slot: 3,
                channel: 1,
                weapon_class: pack,
                clips: 5,
            }]
            .into(),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    (s, bot, pack)
}

#[test]
fn needy_self_receives_health_and_ammo_from_one_pack() {
    for bf2 in [false, true] {
        let (mut s, bot, _) = world(bf2);
        let u = s.unit_mut(bot).unwrap();
        u.health.cur = 50.0;
        u.weapon.spare = 0;
        let primary = u.weapon_class;
        let mut drops = 0;
        let mut gains = 0;
        for _ in 0..120 {
            s.step(&[]);
            drops += s
                .events
                .iter()
                .filter(|e| matches!(e, Event::ItemDropped { .. }))
                .count();
            gains += s.events.iter().filter(|e| matches!(e, Event::ItemTaken { unit, health: true, ammo: true, .. } if *unit == bot)).count();
        }
        assert_eq!(drops, 1, "bf2={bf2}");
        assert_eq!(gains, 1, "bf2={bf2}");
        let u = s.unit(bot).unwrap();
        assert_eq!(u.health.cur, 150.0);
        assert!(u.weapon.spare > 0);
        assert_eq!(u.weapon_class, primary);
        assert!(u.soldier.pitch.abs() < 0.04);
        assert_eq!(
            u.held[0].state.rounds(&s.weapon_classes[u.held[0].class]),
            Some(4)
        );
    }
}

fn drops(s: &Sim) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e, Event::ItemDropped { .. }))
        .count()
}

#[test]
fn nearby_allies_including_humans_receive_supplies_without_changing_the_primary() {
    for bf2 in [false, true] {
        for human in [false, true] {
            let (mut s, bot, _) = world(bf2);
            // An AI ally holds position; no objective or combat pulls it away.
            let ally = s
                .spawn(&UnitSpec {
                    controller: if human {
                        Controller::Player { slot: 0 }
                    } else {
                        Controller::Ai
                    },
                    ..UnitSpec::trooper(0, Vec3::NEG_Z * 2.0)
                })
                .unwrap();
            let u = s.unit_mut(ally).unwrap();
            u.health.cur = 50.0;
            u.weapon.spare = 0;
            let primary = s.unit(bot).unwrap().weapon_class;
            let mut count = 0;
            let mut taken = 0;
            for _ in 0..120 {
                s.step(&[PadState::default()]);
                count += drops(&s);
                taken += s.events.iter().filter(|e| matches!(e, Event::ItemTaken { unit, health: true, ammo: true, .. } if *unit == ally)).count();
            }
            assert_eq!(count, 1, "bf2={bf2} human={human}");
            assert_eq!(taken, 1, "bf2={bf2} human={human}");
            assert_eq!(s.unit(bot).unwrap().weapon_class, primary);
        }
    }
}

#[test]
fn full_units_enemy_needs_and_empty_packs_do_not_deploy() {
    for case in 0..4 {
        let (mut s, bot, _) = world(true);
        match case {
            0 => {}
            1 => {
                // Out of sight behind the bot; its need alone cannot trigger aid.
                let enemy = s.spawn(&UnitSpec::player(1, Vec3::Z * 2.0)).unwrap();
                s.unit_mut(enemy).unwrap().health.cur = 1.0;
            }
            2 => {
                let u = s.unit_mut(bot).unwrap();
                u.health.cur = 1.0;
                u.held[0].state.clip = 0;
                u.held[0].state.spare = 0;
            }
            _ => {
                // Packs cannot replenish themselves, so a used pack alone
                // is not an ammunition need.
                s.unit_mut(bot).unwrap().held[0].state.spare = 0;
            }
        }
        for _ in 0..180 {
            s.step(&[PadState::default()]);
            assert_eq!(drops(&s), 0, "case {case}");
        }
    }
}

#[test]
fn existing_pickups_suppress_each_needed_resource_independently() {
    for (health, ammo, should_drop) in [
        (100.0, 2, false),
        (100.0, 0, true),
        (0.0, 2, true),
        (0.0, 0, true),
    ] {
        let (mut s, bot, _) = world(true);
        let u = s.unit_mut(bot).unwrap();
        u.health.cur = 50.0;
        u.weapon.spare = 0;
        let item = s.add_item_class(ItemClass::basic("nearby", health, ammo));
        s.pickups.push(crate::sim::Pickup {
            source: None,
            class: item,
            pos: Vec3::X * 4.0,
            vel: Vec3::ZERO,
            yaw: 0.0,
            life: 30.0,
            landed: true,
        });
        let mut st = crate::ai::AiState::default();
        assert_eq!(
            super::plan(&s, bot, &mut st, true, &PadState::default()).is_some(),
            should_drop
        );
        if !should_drop {
            assert!(st.supply.drop.is_none());
            s.pickups.clear();
            s.tick = 149;
            assert!(super::plan(&s, bot, &mut st, true, &PadState::default()).is_none());
            s.tick = 150;
            assert!(super::plan(&s, bot, &mut st, true, &PadState::default()).is_some());
        }
    }
}

#[test]
fn cooldown_blocks_repeated_supply_and_retries_after_five_seconds() {
    let (mut s, bot, _) = world(true);
    s.unit_mut(bot).unwrap().health.cur = 1.0;
    let mut press = None;
    let mut count = 0;
    for _ in 0..260 {
        s.step(&[]);
        count += drops(&s);
        let tick = s.tick;
        let u = s.unit_mut(bot).unwrap();
        if u.ai.supply.drop.is_some_and(|d| d.pressed) && press.is_none() {
            press = Some(tick - 1);
        }
        // Keep a real need after every pickup and remove existing supply to
        // isolate the scheduling gate from item suppression.
        u.health.cur = 1.0;
        s.pickups.clear();
        if let Some(at) = press {
            if s.tick <= at + 150 {
                assert!(count <= 1, "repeat before cooldown at {}", s.tick);
            }
        }
    }
    assert!(press.is_some());
    assert_eq!(count, 2);
}

#[test]
fn secondary_selection_does_not_fire_grenades_or_spend_primary_ammo() {
    let (mut s, bot, pack) = world(true);
    let grenade = s.add_weapon_class(WeaponClass {
        rounds_per_clip: 1,
        ..Default::default()
    });
    let spec = SlotSpec {
        slot: 2,
        channel: 1,
        weapon_class: grenade,
        clips: 2,
    };
    let mut bot_spec = s.unit(bot).unwrap().spec.clone();
    bot_spec.slots = vec![
        spec,
        SlotSpec {
            slot: 3,
            channel: 1,
            weapon_class: pack,
            clips: 5,
        },
    ]
    .into();
    // New life with the grenade selected first, then the support pack.
    let mut s2 = Sim::empty(19);
    s2.ground_rules = s.ground_rules;
    s2.heightfield = s.heightfield.take();
    s2.weapon_classes.clear();
    for wc in &s.weapon_classes {
        s2.add_weapon_class(wc.clone());
    }
    let bot = s2.spawn(&bot_spec).unwrap();
    let u = s2.unit_mut(bot).unwrap();
    assert_eq!(u.held[u.cur1.unwrap() as usize].slot, 2);
    u.health.cur = 50.0;
    let primary = (u.weapon_class, u.weapon.clip, u.weapon.spare);
    let mut count = 0;
    for _ in 0..120 {
        s2.step(&[]);
        count += drops(&s2);
    }
    assert_eq!(count, 1);
    let u = s2.unit(bot).unwrap();
    assert_eq!((u.weapon_class, u.weapon.clip, u.weapon.spare), primary);
    assert_eq!(
        u.held
            .iter()
            .find(|h| h.slot == 2)
            .unwrap()
            .state
            .rounds(&s2.weapon_classes[grenade]),
        Some(2)
    );
}

#[test]
fn combat_and_recent_hits_keep_the_primary_and_cancel_uncommitted_drops() {
    let (mut s, bot, _) = world(true);
    s.unit_mut(bot).unwrap().health.cur = 50.0;
    let mut st = crate::ai::AiState::default();
    assert!(super::plan(&s, bot, &mut st, true, &PadState::default()).is_some());
    let enemy = s.spawn(&UnitSpec::player(1, Vec3::NEG_Z * 10.0)).unwrap();
    s.unit_mut(enemy).unwrap().health.cur = 30000.0;
    st.target = Some(enemy.into());
    assert!(super::plan(&s, bot, &mut st, false, &PadState::default()).is_none());
    assert!(st.supply.drop.is_none());
    // Actual combat still fires ordinary shots; no support drops are made.
    let mut shots = 0;
    for _ in 0..90 {
        s.step(&[PadState::default()]);
        assert_eq!(drops(&s), 0);
        shots += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == bot))
            .count();
    }
    assert!(shots > 0);
    st = crate::ai::AiState::default();
    let tick = s.tick;
    s.unit_mut(bot).unwrap().last_hit = Some((enemy, tick));
    assert!(super::plan(&s, bot, &mut st, true, &PadState::default()).is_none());
}

#[test]
fn deployment_is_deterministic_and_all_pending_state_is_hashed() {
    let run = || {
        let (mut s, bot, _) = world(true);
        s.unit_mut(bot).unwrap().health.cur = 50.0;
        (0..180)
            .map(|_| {
                s.step(&[]);
                s.hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
    let (mut s, bot, _) = world(true);
    s.unit_mut(bot).unwrap().health.cur = 50.0;
    let mut st = crate::ai::AiState::default();
    super::plan(&s, bot, &mut st, true, &PadState::default());
    let bytes = |st: &crate::ai::AiState| {
        let mut bytes = Vec::new();
        st.hash_into(&mut |b| bytes.extend_from_slice(b));
        bytes
    };
    let base = bytes(&st);
    for case in 0..6 {
        let mut other = st;
        match case {
            0 => other.supply.next_scan += 1,
            1 => other.supply.drop.as_mut().unwrap().target.index += 1,
            2 => other.supply.drop.as_mut().unwrap().target.gen += 1,
            3 => other.supply.drop.as_mut().unwrap().slot += 1,
            4 => other.supply.drop.as_mut().unwrap().until += 1,
            _ => other.supply.drop.as_mut().unwrap().pressed = true,
        }
        assert_ne!(base, bytes(&other), "case {case}");
    }
}

#[test]
fn walls_dead_allies_and_urgent_moves_do_not_receive_a_drop() {
    use crate::collision::{Body, Shape};
    for case in 0..5 {
        let (mut s, bot, _) = world(true);
        let ally = s.spawn(&UnitSpec::player(0, Vec3::NEG_Z * 2.0)).unwrap();
        s.unit_mut(ally).unwrap().health.cur = 1.0;
        let mut st = crate::ai::AiState::default();
        match case {
            0 => {
                s.collision.push_body(Body::new(
                    glam::Mat4::from_translation(Vec3::new(0.0, 1.0, -1.0)),
                    Shape::Box {
                        half: Vec3::new(3.0, 2.0, 0.1),
                    },
                ));
            }
            1 => s.unit_mut(ally).unwrap().alive = false,
            2 => st.step = crate::ai::StepKind::Flee,
            3 => st.step = crate::ai::StepKind::Return,
            _ => s.unit_mut(bot).unwrap().soldier.grounded = false,
        }
        assert!(
            super::plan(&s, bot, &mut st, true, &PadState::default()).is_none(),
            "case {case}"
        );
        assert!(st.supply.drop.is_none());
    }
}

#[test]
fn an_unavailable_recipient_ends_the_pending_plan_without_spending_ammo() {
    let (mut s, bot, _) = world(true);
    let ally = s.spawn(&UnitSpec::player(0, Vec3::NEG_Z * 2.0)).unwrap();
    s.unit_mut(ally).unwrap().health.cur = 1.0;
    let mut st = crate::ai::AiState::default();
    assert!(super::plan(&s, bot, &mut st, true, &PadState::default()).is_some());
    assert_eq!(st.supply.drop.unwrap().target, ally);
    s.unit_mut(ally).unwrap().alive = false;
    assert!(super::plan(&s, bot, &mut st, false, &PadState::default()).is_none());
    assert!(st.supply.drop.is_none());
    assert_eq!(s.unit(bot).unwrap().held[0].state.clip, 1);
}

#[test]
fn space_proxy_mounted_and_human_suppliers_are_unchanged() {
    for case in 0..4 {
        let (mut s, bot, _) = world(true);
        s.unit_mut(bot).unwrap().health.cur = 1.0;
        match case {
            0 => s.space_assault = Some(crate::sim::SpaceAssault::new(180)),
            1 => {
                s.unit_mut(bot).unwrap().proxy_human =
                    Some(crate::ai::proxy::Strength::Good.profile())
            }
            2 => {
                s.unit_mut(bot).unwrap().mount = Some(crate::vehicle::Mount {
                    vehicle: crate::vehicle::VehicleHandle { index: 0, gen: 0 },
                    seat: 0,
                })
            }
            _ => s.unit_mut(bot).unwrap().controller = Controller::Player { slot: 0 },
        }
        let mut st = crate::ai::AiState::default();
        assert!(
            super::plan(&s, bot, &mut st, true, &PadState::default()).is_none(),
            "case {case}"
        );
        assert_eq!(st.supply, super::SupplyState::default());
    }
}

#[test]
fn two_support_bots_share_one_recipient_without_duplicate_packs() {
    let (mut s, first, pack) = world(true);
    let mut spec = s.unit(first).unwrap().spec.clone();
    spec.pos = Vec3::X * 0.8;
    let second = s.spawn(&spec).unwrap();
    assert!(s.unit(second).unwrap().held.iter().any(|w| w.class == pack));
    let ally = s.spawn(&UnitSpec::player(0, Vec3::NEG_Z * 2.0)).unwrap();
    s.unit_mut(ally).unwrap().health.cur = 50.0;
    let mut count = 0;
    for _ in 0..120 {
        s.step(&[PadState::default()]);
        count += drops(&s);
    }
    assert_eq!(count, 1);
    assert_eq!(s.unit(ally).unwrap().health.cur, 150.0);
}

#[test]
fn health_only_and_ammo_only_needs_each_trigger_a_pack() {
    for health in [false, true] {
        let (mut s, bot, _) = world(true);
        if health {
            s.unit_mut(bot).unwrap().health.cur = 50.0;
        } else {
            s.unit_mut(bot).unwrap().weapon.spare = 0;
        }
        let mut count = 0;
        let mut gain = 0;
        for _ in 0..120 {
            s.step(&[]);
            count += drops(&s);
            gain += s.events.iter().filter(|e| matches!(e, Event::ItemTaken { unit, health: h, ammo: a, .. } if *unit == bot && *h == health && *a != health)).count();
        }
        assert_eq!(count, 1);
        assert_eq!(gain, 1);
    }
}

#[test]
fn combat_during_a_latched_release_finishes_once_then_resumes_fighting() {
    let (mut s, bot, _) = world(true);
    s.unit_mut(bot).unwrap().health.cur = 50.0;
    let mut latched = false;
    for _ in 0..50 {
        s.step(&[]);
        if s.unit(bot).unwrap().held[0].state.state == crate::weapon::FireState::Firing {
            latched = true;
            break;
        }
    }
    assert!(
        latched,
        "the BF2 delayed release must actually be under way"
    );
    let enemy = s.spawn(&UnitSpec::player(1, Vec3::NEG_Z * 10.0)).unwrap();
    s.unit_mut(enemy).unwrap().health.cur = 30000.0;
    let mut count = 0;
    let mut shots = 0;
    for _ in 0..90 {
        s.step(&[PadState::default()]);
        count += drops(&s);
        shots += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, weapon_class: 0, .. } if *unit == bot))
            .count();
    }
    assert_eq!(count, 1);
    assert!(shots > 0);
    assert!(s.unit(bot).unwrap().ai.supply.drop.is_none());
}

#[test]
fn failed_pool_spawns_refund_the_bot_and_do_not_spam() {
    let (mut s, bot, _) = world(true);
    s.item_pool = 0;
    s.unit_mut(bot).unwrap().health.cur = 50.0;
    for _ in 0..180 {
        s.step(&[]);
        assert_eq!(drops(&s), 0);
    }
    let u = s.unit(bot).unwrap();
    let w = &u.held[0];
    assert_eq!(w.state.rounds(&s.weapon_classes[w.class]), Some(5));
    assert!(u.ai.supply.next_scan > 150);
}

#[test]
fn an_older_owned_pack_behind_a_wall_does_not_hide_the_new_self_drop() {
    let (mut s, bot, pack) = world(true);
    s.unit_mut(bot).unwrap().health.cur = 50.0;
    s.pickups.push(crate::sim::Pickup {
        source: Some((bot, pack)),
        class: 0,
        pos: Vec3::Z * 4.0,
        vel: Vec3::ZERO,
        yaw: 0.0,
        life: 30.0,
        landed: true,
    });
    s.collision.push_body(crate::collision::Body::new(
        glam::Mat4::from_translation(Vec3::new(0.0, 1.0, 2.0)),
        crate::collision::Shape::Box {
            half: Vec3::new(3.0, 2.0, 0.1),
        },
    ));
    let mut count = 0;
    let mut gains = 0;
    for _ in 0..120 {
        s.step(&[]);
        count += drops(&s);
        gains += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::ItemTaken { unit, health: true, .. } if *unit == bot))
            .count();
    }
    assert_eq!(count, 1);
    assert_eq!(gains, 1);
    assert_eq!(s.pickups.len(), 1, "only the older blocked pack remains");
}
