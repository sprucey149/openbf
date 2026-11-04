//! Issue #103: release strength stays within the authored item endpoints.
use super::*;

#[test]
fn fc_dispenser_held_retrigger_bounds_item_speed() {
    let c = WeaponClass {
        max_pressed_time: 0.5,
        shot_delay: [2.0; 2],
        trigger_single: true,
        ..Default::default()
    };
    let d = Dispenser {
        item: (),
        velocity: 10.0,
        strength: [0.3, 1.0],
        max_items: 0,
    };
    let mut w = WeaponState::new(&c, true, 0);
    let mut lcg = GameLcg::new(7);
    let mut shots = Vec::new();
    for _ in 0..15 {
        w.update(DT30, &c, fi(true, true), &mut lcg, &mut shots);
    }
    w.update(DT30, &c, fi(false, true), &mut lcg, &mut shots);
    assert_eq!(shots.len(), 1);
    assert_eq!(d.speed(shots[0].charge), 10.0);
    shots.clear();
    // Press while the first firing cycle is still running, then keep it held
    // after the cycle returns to idle. No fresh idle press resets its limit.
    for _ in 0..90 {
        w.update(DT30, &c, fi(true, true), &mut lcg, &mut shots);
    }
    assert_eq!(w.state, FireState::Idle);
    w.update(DT30, &c, fi(false, true), &mut lcg, &mut shots);
    assert_eq!(shots.len(), 1);
    assert!(
        shots[0].charge > 1.0,
        "witness the raw state-machine overflow"
    );
    assert_eq!(d.speed(shots[0].charge), 10.0);
}

#[test]
fn fc_dispenser_switch_and_unusual_charge_durations() {
    for max_pressed_time in [0.0, -0.5, f32::MIN_POSITIVE, 0.017, 0.37, 0.5] {
        let c = WeaponClass {
            max_pressed_time,
            shot_delay: [2.0; 2],
            trigger_single: true,
            ..Default::default()
        };
        let d = Dispenser {
            item: (),
            velocity: 10.0,
            strength: [0.3, 1.0],
            max_items: 0,
        };
        let mut w = WeaponState::new(&c, true, 0);
        let mut lcg = GameLcg::new(7);
        let mut shots = Vec::new();
        for tick in 0..300 {
            // Holster mid-charge, leave it updating, then select it again.
            w.active = !(5..20).contains(&tick);
            let held = tick < 25 || (26..200).contains(&tick);
            w.update(DT30, &c, fi(held, true), &mut lcg, &mut shots);
        }
        assert!(!shots.is_empty(), "duration {max_pressed_time}");
        for shot in shots {
            assert!(
                (3.0..=10.0).contains(&d.speed(shot.charge)),
                "duration {max_pressed_time}, raw charge {}",
                shot.charge
            );
        }
    }
    // Lower-bound protection and the in-range interpolation control.
    let d = Dispenser {
        item: (),
        velocity: 10.0,
        strength: [0.3, 1.0],
        max_items: 0,
    };
    assert_eq!(d.speed(-1.0), 3.0);
    assert_eq!(d.speed(0.5), 6.5);
}
