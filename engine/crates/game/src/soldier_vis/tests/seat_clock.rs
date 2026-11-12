use super::*;

fn held(x: f32) -> Mat4 {
    Mat4::from_translation(Vec3::new(x, 0.0, 0.0))
}

#[test]
fn a_seat_reads_one_tick_back_along_its_drawn_frames() {
    let mut slot = None;
    let mut then = None;
    for k in 0..=12 {
        let at = 10.0 + f64::from(k) * 0.25;
        then = Some(SeatClock::advance(
            &mut slot,
            at,
            0.1 * at as f32,
            held(at as f32),
        ));
    }
    let then = then.unwrap();
    assert!((then.turned - 0.1).abs() < 1e-5, "{then:?}");
    assert!(
        then.held
            .w_axis
            .truncate()
            .abs_diff_eq(Vec3::new(12.0, 0.0, 0.0), 1e-4),
        "{then:?}"
    );
}

#[test]
fn frames_between_ticks_turn_the_lean_evenly() {
    let mut slot = None;
    let mut turns = Vec::new();
    for at in [5.0, 5.4, 5.8, 6.0, 6.3, 6.7, 7.0, 7.45, 7.9, 8.2] {
        let c = SeatClock::advance(&mut slot, at, 0.2 * at as f32, held(0.0));
        turns.push(c.turned);
    }
    for t in &turns[3..] {
        assert!((t - 0.2).abs() < 1e-5, "{turns:?}");
    }
}

#[test]
fn a_new_run_clears_what_the_seat_saw() {
    let mut slot = None;
    SeatClock::advance(&mut slot, 40.0, 1.0, held(4.0));
    let c = SeatClock::advance(&mut slot, 3.0, -1.0, held(-2.0));
    assert_eq!(c.turned, 0.0);
    assert_eq!(c.held, held(-2.0));
}

#[test]
fn a_rated_gun_leans_by_its_own_rate() {
    let then = SeatThen {
        turned: 0.0,
        held: Mat4::IDENTITY,
    };
    assert_eq!(turn_number(Some(-1.0), then, 2.0), -0.5);
    assert_eq!(turn_number(Some(9.0), then, 2.0), 1.0);
    let turned = SeatThen {
        turned: core::TICK_DT,
        ..then
    };
    assert!((turn_number(None, turned, 2.0) - 0.5).abs() < 1e-6);
}
