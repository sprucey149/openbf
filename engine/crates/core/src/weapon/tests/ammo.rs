use super::*;
use crate::unit::{SlotSpec, UnitSpec};

#[test]
fn primary_and_held_slots_count_the_loaded_clip_in_the_issue() {
    let c = WeaponClass::bf1_rifle();
    for human in [false, true] {
        for clips in [0, 1, 3, 6] {
            let mut spec = if human {
                UnitSpec::player(0, Vec3::ZERO)
            } else {
                UnitSpec::trooper(0, Vec3::ZERO)
            };
            spec.total_clips = clips;
            spec.slots = vec![SlotSpec {
                slot: 1,
                channel: 0,
                weapon_class: 0,
                clips,
            }]
            .into();
            let u = spec.build(0, std::slice::from_ref(&c));
            let held = &u.held[0].state;
            let total = (clips != 0).then_some(clips * 50);
            assert_eq!(u.weapon.rounds(&c), total, "human {human}, clips {clips}");
            assert_eq!(u.weapon.issued(&c), total);
            assert_eq!(u.weapon.clip, held.clip);
            assert_eq!(u.weapon.spare, held.spare);
        }
    }
}

#[test]
fn an_empty_single_shot_reloads_after_its_fire_cycle_without_another_press() {
    let c = WeaponClass {
        trigger_single: true,
        rounds_per_clip: 1,
        shot_delay: [0.5; 2],
        reload_time: 1.0,
        ..WeaponClass::bf1_rifle()
    };
    let mut w = WeaponState::with_clips(&c, true, 3);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    w.update(DT30, &c, fi(true, false), &mut lcg, &mut shots);
    assert_eq!(shots.len(), 1);
    for _ in 0..10 {
        w.update(DT30, &c, fi(false, false), &mut lcg, &mut shots);
        assert!(!w.reloading(), "the firing delay must finish first");
    }
    for _ in 0..8 {
        w.update(DT30, &c, fi(false, false), &mut lcg, &mut shots);
    }
    assert!(w.reloading(), "release must not prevent an empty reload");
    assert_eq!(w.clip, 0);
    for _ in 0..32 {
        w.update(DT30, &c, fi(false, false), &mut lcg, &mut shots);
    }
    assert!(!w.reloading());
    assert_eq!((w.clip, w.spare), (1, 1));
    assert_eq!(shots.len(), 1);
}

#[test]
fn an_empty_selected_idle_weapon_reloads_without_input() {
    let c = WeaponClass::bf1_rifle();
    for active in [false, true] {
        let mut w = WeaponState::with_clips(&c, true, 2);
        w.clip = 0;
        w.active = active;
        w.update(
            DT30,
            &c,
            fi(false, false),
            &mut GameLcg::new(1),
            &mut Vec::new(),
        );
        assert_eq!(w.reloading(), active);
    }
}

#[test]
fn a_partial_reload_returns_loaded_rounds_to_the_reserve() {
    let c = WeaponClass::bf1_rifle();
    let mut w = WeaponState::with_clips(&c, true, 3);
    w.clip = 17;
    let total = w.rounds(&c);
    assert!(w.start_reload(&c));
    assert_eq!((w.clip, w.spare), (0, 117));
    assert_eq!(w.rounds(&c), total);
    run(&mut w, &c, DT30, 47, false);
    assert!(!w.reloading());
    assert_eq!((w.clip, w.spare), (50, 67));
    assert_eq!(w.rounds(&c), total);
}

#[test]
fn a_five_clip_rifle_fires_275_rounds_and_then_requires_resupply() {
    let c = WeaponClass {
        rounds_per_clip: 55,
        ..WeaponClass::bf1_rifle()
    };
    let spec = UnitSpec {
        total_clips: 5,
        ..UnitSpec::player(0, Vec3::ZERO)
    };
    let mut w = spec.build(0, std::slice::from_ref(&c)).weapon;
    assert_eq!((w.clip, w.spare), (55, 220));
    let shots = run(&mut w, &c, DT30, 30 * 100, true);
    assert_eq!(shots.len(), 275);
    assert_eq!(w.rounds(&c), Some(0));
    assert!(w.out_of_ammo(&c));
    assert!(!w.start_reload(&c));
    assert!(run(&mut w, &c, DT30, 30 * 5, true).is_empty());
}

/// Issue #78: the ring shows a charge only while the weapon can charge. A reload
/// keeps the port's state Idle, where the PS2 has left its charging state 3.
#[test]
fn a_reloading_charge_weapon_reports_no_charge() {
    let c = WeaponClass {
        max_pressed_time: 1.0,
        rounds_per_clip: 1,
        reload_time: 2.0,
        ..WeaponClass::bf1_rifle()
    };
    let mut w = WeaponState::with_clips(&c, true, 3);
    w.clip = 0;
    let (mut lcg, mut shots) = (GameLcg::new(1), Vec::new());
    assert!(w.start_reload(&c));
    for _ in 0..15 {
        w.update(DT30, &c, fi(true, false), &mut lcg, &mut shots);
    }
    assert!(w.reloading());
    assert_eq!(w.charging(&c), None);
}
