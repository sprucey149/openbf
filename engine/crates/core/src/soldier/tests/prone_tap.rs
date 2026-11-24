//! `[ours]` The prone double tap (the maintainer, 2026-10-28, both games): with the
//! stick idle a stand crouches on a tap, a crouch goes prone on a second tap
//! within [`PRONE_DOUBLE_TAP`] of the first and stands up after it, a prone
//! stands up on a tap; JUMP stands a crouch or a prone up without a jump.
use super::*;

fn tap() -> PadState {
    PadState {
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    }
}

fn jump() -> PadState {
    PadState {
        buttons: buttons::JUMP,
        ..Default::default()
    }
}

/// A soldier crouched by a tap this very tick (`time_in_stance` 0).
fn crouched(c: &SoldierConstants) -> Soldier {
    let mut s = Soldier::new(Vec3::ZERO);
    run(&mut s, &PadState::default(), c, 20); // past STANCE_TAP_WAIT
    tick(&mut s, &tap(), c);
    assert_eq!(s.stance, Stance::Crouch);
    assert_eq!(s.time_in_stance, 0.0);
    s
}

/// The crouch's second tap lands `n` ticks after the first.
fn second_tap_after(n: usize) -> Soldier {
    let c = SoldierConstants::default();
    let mut s = crouched(&c);
    run(&mut s, &PadState::default(), &c, n - 1);
    tick(&mut s, &tap(), &c);
    s
}

#[test]
fn one_tap_crouches_a_stand() {
    let c = SoldierConstants::default();
    let s = crouched(&c);
    assert!(!s.lock_hard && s.lock_timer == 0.0, "crouch is instant");
}

#[test]
fn a_second_tap_inside_the_window_goes_prone() {
    let c = SoldierConstants::default();
    // The quickest double tap a pad can make: press, release, press.
    let s = second_tap_after(2);
    assert_eq!(s.stance, Stance::Prone);
    assert!(s.lock_hard && (s.lock_timer - c.prone_down_lock).abs() < 1e-6);
    // 0.9 s.
    assert_eq!(second_tap_after(27).stance, Stance::Prone);
}

#[test]
fn a_crouch_tap_after_the_window_stands_up() {
    // 1.1 s.
    let s = second_tap_after(33);
    assert_eq!(s.stance, Stance::Stand);
    assert!(
        !s.lock_hard && s.lock_timer == 0.0,
        "crouch -> stand is instant"
    );
}

#[test]
fn a_prone_tap_stands_up_through_the_get_up_lock() {
    let c = SoldierConstants::default();
    let mut s = second_tap_after(2);
    assert_eq!(s.stance, Stance::Prone);
    // Refused inside the lay-down lock.
    run(&mut s, &PadState::default(), &c, 20);
    tick(&mut s, &tap(), &c);
    assert_eq!(s.stance, Stance::Prone);
    run(&mut s, &PadState::default(), &c, 30);
    tick(&mut s, &tap(), &c);
    assert_eq!(
        s.stance,
        Stance::Stand,
        "no longer prone -> stand -> crouch"
    );
    assert!(s.lock_hard && (s.lock_timer - c.prone_up_lock).abs() < 1e-6);
}

#[test]
fn jump_stands_a_crouch_up_without_a_jump() {
    let c = SoldierConstants::default();
    let mut s = crouched(&c);
    run(&mut s, &PadState::default(), &c, 20);
    tick(&mut s, &jump(), &c);
    assert_eq!(s.stance, Stance::Stand);
    assert!(s.grounded && s.vel.y == 0.0 && s.lock_timer == 0.0);
    assert_eq!(s.energy, c.energy_bar);
}

#[test]
fn jump_stands_a_prone_up_through_the_get_up_lock_without_a_jump() {
    let c = SoldierConstants::default();
    let mut s = second_tap_after(2);
    // Refused inside the lay-down lock.
    run(&mut s, &PadState::default(), &c, 20);
    tick(&mut s, &jump(), &c);
    assert_eq!(s.stance, Stance::Prone);
    run(&mut s, &PadState::default(), &c, 30);
    tick(&mut s, &jump(), &c);
    assert_eq!(s.stance, Stance::Stand);
    assert!(s.lock_hard && (s.lock_timer - c.prone_up_lock).abs() < 1e-6);
    assert!(s.grounded && s.vel.y == 0.0);
    // The button held on through the get-up does not launch one either.
    for _ in 0..60 {
        tick(&mut s, &jump(), &c);
        assert_eq!(s.stance, Stance::Stand);
        assert!(s.grounded && s.vel.y == 0.0);
    }
    assert_eq!(s.energy, c.energy_bar, "standing up costs nothing");
}

#[test]
fn the_roll_is_unchanged_from_a_crouch() {
    let c = SoldierConstants::default();
    let mut s = crouched(&c);
    run(&mut s, &PadState::default(), &c, 2);
    let side = PadState {
        lx: 32767,
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    };
    tick(&mut s, &side, &c);
    assert_eq!(
        s.stance,
        Stance::Roll,
        "moving = roll, inside the window too"
    );
}

#[test]
fn the_bots_mirror_matches_the_step() {
    let c = SoldierConstants::default();
    let mut s = crouched(&c);
    // The press that crouched it is still held: no edge.
    assert_eq!(s.stance_tap_after(DT), None);
    tick(&mut s, &PadState::default(), &c);
    assert_eq!(s.stance_tap_after(DT), Some(Stance::Prone));
    assert!(!s.jump_stands_after(DT), "inside the rise wait");
    for n in 2..60 {
        let want = s.stance_tap_after(DT);
        let mut t = s.clone();
        tick(&mut t, &tap(), &c);
        assert_eq!(Some(t.stance), want, "tick {n}");
        let rises = s.jump_stands_after(DT);
        let mut j = s.clone();
        tick(&mut j, &jump(), &c);
        assert_eq!(j.stance == Stance::Stand, rises, "tick {n}");
        tick(&mut s, &PadState::default(), &c);
    }
}

#[test]
fn prone_off_never_enters_prone_and_updates_static_digest() {
    let c = SoldierConstants::default();
    let mut s = crouched(&c);
    run(&mut s, &PadState::default(), &c, 4);
    s.step_looking_with_prone(
        DT,
        &tap(),
        &c,
        GroundRules::Bf2,
        1.0,
        AirEnv::default(),
        false,
    );
    assert_eq!(
        s.stance,
        Stance::Stand,
        "double-tapping crouch with prone disabled stands up instead of going prone"
    );

    let sim_default = crate::sim::Sim::empty(7);
    let mut sim_bf2_only = crate::sim::Sim::empty(7);
    sim_bf2_only.prone_off = true;
    assert_ne!(
        sim_default.static_digest(),
        sim_bf2_only.static_digest(),
        "prone_off must participate in the LAN static digest"
    );
}
