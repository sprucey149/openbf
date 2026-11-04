use super::*;
use crate::unit::{Controller, SlotSpec};
use crate::weapon::{Dispenser, Thrown};

fn setup(bf2: bool) -> (Sim, UnitHandle, usize) {
    let mut s = sim(bf2);
    let w = s.add_weapon_class(WeaponClass {
        name: "pack_dispenser".into(),
        rounds_per_clip: 1,
        reload_time: 1.0,
        trigger_single: true,
        shot_delay: [0.0; 2],
        initial_salvo_delay: if bf2 { 0.5 } else { 0.0 },
        thrown: Some(Thrown {
            shot_elevate: 0.0,
            hide_on_fire: true,
            hand: bf2.then_some(Dispenser::BF2_HAND),
        }),
        dispenser: Some(Dispenser {
            item: items(bf2)[usize::from(DUAL)].clone(),
            velocity: 10.0,
            strength: [0.3, 0.4],
            max_items: 4,
        }),
        ..Default::default()
    });
    let h = s
        .spawn(&UnitSpec {
            controller: Controller::Player { slot: 0 },
            slots: vec![SlotSpec {
                slot: 3,
                channel: 1,
                weapon_class: w,
                clips: 5,
            }]
            .into(),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    (s, h, w)
}

#[test]
fn five_presses_spend_five_packs_and_replace_only_the_owners_oldest() {
    for bf2 in [false, true] {
        let (mut s, h, _) = setup(bf2);
        s.item_pool = 4;
        let mut drops = Vec::new();
        for tick in 0usize..720 {
            let p = PadState {
                buttons: if tick % 120 == 0 {
                    crate::sim::buttons::SECONDARY_FIRE
                } else {
                    0
                },
                ..Default::default()
            };
            s.step(&[p]);
            if s.events
                .iter()
                .any(|e| matches!(e, Event::ItemDropped { .. }))
            {
                drops.push(tick);
            }
        }
        assert_eq!(drops.len(), 5);
        for (n, tick) in drops.iter().enumerate() {
            assert!(
                (*tick as i32
                    - (n * 120 + if bf2 { crate::TICK_HZ as usize / 2 } else { 0 }) as i32)
                    .abs()
                    <= 1
            );
        }
        assert_eq!(s.pickups.len(), 4);
        assert_eq!(
            s.unit(h).unwrap().held[0]
                .state
                .rounds(&s.weapon_classes[s.unit(h).unwrap().held[0].class]),
            Some(0)
        );
        let oldest_life = 30.0 - (720 - drops[1]) as f32 * DT;
        assert!(s.pickups.iter().all(|p| p.life >= oldest_life - 0.01));
    }
}

#[test]
fn bf2_packs_leave_the_hand_in_the_soldiers_frame_and_bf1_keeps_its_origin() {
    for bf2 in [false, true] {
        for (yaw, forward) in [
            (0.0, Vec3::NEG_Z),
            (std::f32::consts::FRAC_PI_2, Vec3::NEG_X),
            (std::f32::consts::PI, Vec3::Z),
        ] {
            let (mut s, h, weapon) = setup(bf2);
            let u = &mut s.units[h.index as usize];
            u.soldier.pos = Vec3::new(7.0, 3.0, -4.0);
            u.soldier.yaw = yaw;
            u.soldier.pitch = 0.8;
            let expected = if bf2 {
                u.soldier.pos + Vec3::Y * Dispenser::BF2_HAND.0 + forward * Dispenser::BF2_HAND.1
            } else {
                u.muzzle()
            };
            assert!(s.dispense_pickup(h.index as usize, weapon, 1.0));
            assert!((s.pickups[0].pos - expected).length() < 1e-6);
        }
    }
}

#[test]
fn held_input_and_presses_during_reload_cannot_duplicate_a_pack() {
    for bf2 in [false, true] {
        let (mut s, _, _) = setup(bf2);
        let fire = PadState {
            buttons: crate::sim::buttons::SECONDARY_FIRE,
            ..Default::default()
        };
        for _ in 0..90 {
            s.step(&[fire]);
        }
        assert_eq!(s.pickups.len(), 1);
        s.step(&[PadState::default()]);
        s.step(&[fire]);
        s.step(&[PadState::default()]);
        s.step(&[fire]);
        step(&mut s, 90);
        assert_eq!(s.pickups.len(), 2);
    }
}

#[test]
fn cap_is_per_owner_and_weapon_and_flight_inherits_velocity() {
    let (mut s, h, w) = setup(false);
    let other = s.spawn(&UnitSpec::trooper(0, Vec3::X * 10.0)).unwrap();
    for n in 0..4 {
        s.units[h.index as usize].soldier.pos.x = n as f32;
        s.dispense_pickup(h.index as usize, w, 1.0);
    }
    let oldest = s.pickups[0].pos;
    s.dispense_pickup(other.index as usize, w, 1.0);
    let mut alternate = s.weapon_classes[w].clone();
    alternate.name = "other_dispenser".into();
    let alt = s.add_weapon_class(alternate);
    s.dispense_pickup(h.index as usize, alt, 1.0);
    s.units[h.index as usize].soldier.vel = Vec3::new(1.0, 2.0, 3.0);
    s.dispense_pickup(h.index as usize, w, 0.5);
    assert_eq!(s.pickups.len(), 6);
    assert!(!s
        .pickups
        .iter()
        .any(|p| p.pos == oldest && p.source == Some((h, w))));
    assert!((s.pickups.last().unwrap().vel - Vec3::new(1.0, 2.0, -0.5)).length() < 1e-6);
    let before = s.hash();
    s.pickups.last_mut().unwrap().source = Some((other, w));
    assert_ne!(s.hash(), before);
}

#[test]
fn full_pool_and_expiry_use_the_existing_pickup_lifecycle() {
    for bf2 in [false, true] {
        let (mut s, h, w) = setup(bf2);
        s.item_pool = 1;
        s.dispense_pickup(h.index as usize, w, 1.0);
        let first = s.pickups[0];
        s.dispense_pickup(h.index as usize, w, 1.0);
        assert_eq!(s.pickups, vec![first]);
        s.pickups[0].life = 0.0;
        step(&mut s, 61);
        assert!(s.pickups.is_empty());
    }
}

#[test]
fn a_failed_spawn_refunds_the_firing_slot_when_both_channels_hold_the_same_class() {
    for bf2 in [false, true] {
        for primary in [false, true] {
            let (mut s, h, w) = setup(bf2);
            let u = &mut s.units[h.index as usize];
            u.weapon_class = w;
            u.weapon = u.held[0].state.clone();
            let rounds = |s: &Sim, primary: bool| {
                let u = s.unit(h).unwrap();
                let state = if primary { &u.weapon } else { &u.held[0].state };
                state.rounds(&s.weapon_classes[w])
            };
            s.item_pool = 0;
            let fire = PadState {
                buttons: if primary {
                    crate::sim::buttons::FIRE
                } else {
                    crate::sim::buttons::SECONDARY_FIRE
                },
                ..Default::default()
            };
            s.step(&[fire]);
            assert!(!s.events.iter().any(|e| matches!(e, Event::Reload { .. })));
            for _ in 0..65 {
                step(&mut s, 1);
                assert!(!s.events.iter().any(|e| matches!(e, Event::Reload { .. })));
            }
            assert!(s.pickups.is_empty());
            assert_eq!(rounds(&s, primary), Some(5));
            assert_eq!(rounds(&s, !primary), Some(5));
            s.item_pool = ITEM_POOL_DEFAULT;
            s.step(&[fire]);
            step(&mut s, 65);
            assert_eq!(s.pickups.len(), 1);
            assert_eq!(rounds(&s, primary), Some(4));
            assert_eq!(rounds(&s, !primary), Some(5));
        }
    }
}

#[test]
fn pickups_and_stations_cannot_refill_primary_or_held_dispensers() {
    let (mut s, h, w) = setup(false);
    let u = &mut s.units[h.index as usize];
    u.weapon_class = w;
    u.weapon = WeaponState::with_clips(&s.weapon_classes[w], true, 5);
    u.weapon.spare = 0;
    u.held[0].state.spare = 0;
    assert!(!give_ammo(&s.weapon_classes, u, 2));
    assert_eq!(u.weapon.spare, 0);
    assert_eq!(u.held[0].state.spare, 0);
    let item = s.item_classes[usize::from(DUAL)].clone();
    assert!(!s.item_needed(s.unit(h).unwrap(), &item));
}

#[test]
fn fc_pack_release_speed_bounds_preserve_endpoint_hashes() {
    for bf2 in [false, true] {
        for (raw, endpoint) in [(-1.0, 0.0), (4.0, 1.0)] {
            let (mut actual, h, weapon) = setup(bf2);
            let (mut control, ch, cw) = setup(bf2);
            assert!(actual.dispense_pickup(h.index as usize, weapon, raw));
            assert!(control.dispense_pickup(ch.index as usize, cw, endpoint));
            assert_eq!(actual.pickups.len(), 1);
            assert_eq!(actual.pickups[0].vel, control.pickups[0].vel);
            assert_eq!(actual.hash(), control.hash());
        }
    }
}
