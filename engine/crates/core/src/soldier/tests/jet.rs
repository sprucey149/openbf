//! Card JETP: the jets on BF2's rules, against `analysis/re-jetpack.md`'s
//! numbers (§6 [D], dt = 1/30). The classes are the disc's (§2.2).
use super::*;

#[test]
fn second_jump_captures_direction_and_native_restart_gate() {
    let c = jedi();
    for (stick, direction) in [
        ((0.0, 0.0), 0),
        ((1.0, 0.0), 1),
        ((-1.0, 0.0), 2),
        ((0.0, -1.0), 3),
        ((0.0, 1.0), 4),
    ] {
        let mut s = Soldier::new(Vec3::ZERO);
        assert_eq!(s.jump_animation, None);
        s.stance = Stance::Jump;
        assert!(s.jet_on(&c, stick));
        assert_eq!(s.jump_animation, Some(direction));
        let first = s.jump_animation_serial;
        s.energy = c.energy_bar;
        assert!(s.jet_on(&c, stick));
        assert_eq!(s.jump_animation_serial, first + u32::from(direction < 2));
        s.energy = c.energy_bar;
        s.time_in_stance = 0.6;
        assert!(s.jet_on(&c, stick));
        assert_eq!(
            s.jump_animation_serial,
            first + u32::from(direction < 2) + 1
        );
    }
}

#[test]
fn wrapped_jump_serial_still_hashes_the_direction_used_by_restart_gating() {
    let bytes = |s: &Soldier| {
        let mut bytes = Vec::new();
        s.hash_into(&mut |part: &[u8]| bytes.extend_from_slice(part));
        bytes
    };
    let mut s = Soldier::new(Vec3::ZERO);
    let plain = bytes(&s);
    s.jump_animation_serial = 0;
    s.jump_animation = Some(2);
    let backward = bytes(&s);
    s.jump_animation = Some(3);
    assert_ne!(bytes(&s), backward);
    assert_ne!(backward, plain);
}

const FWD: i16 = 32767;

/// BF2 `imp_inf_dark_trooper` (the rebel trial class's run and jump).
fn dark_trooper() -> SoldierConstants {
    SoldierConstants {
        jet_jump: 21.0,
        jet_push: 0.0,
        jet_acceleration: 10.0,
        jet_fuel_cost: 0.0,
        jet_fuel_initial: 0.5,
        jet_fuel_recharge: 0.075,
        jet_fuel_min_border: 0.49,
        ..trial_class(GroundRules::Bf2)
    }
}

/// BF2 `rep_inf_ep3_jettrooper`: MaxSpeed 7.0, MaxStrafeSpeed 5.25. These
/// are the disc's fuel numbers, which the PS2 capture replays; the game
/// flies every jet trooper on `game::roster::JET_TROOPER_FUEL_*`.
fn jet_trooper() -> SoldierConstants {
    SoldierConstants {
        jet_jump: 5.0,
        jet_push: 8.0,
        jet_acceleration: 30.0,
        jet_hover: true,
        jet_fuel_cost: 0.16,
        jet_fuel_initial: 0.25,
        jet_fuel_recharge: 0.12,
        jet_fuel_min_border: 0.24,
        jet_control: [1.5, 1.25, 1.25],
        ..trial_class(GroundRules::Bf2)
    }
}

/// BF1 `rep_inf_jet_trooper` on BF2's rules: its own numbers (MaxSpeed 6.5,
/// MaxStrafeSpeed 4.5, initial / recharge / border 0.25 / 0.1 / 0.24), the
/// net cost 0.2 - 0.1 = 0.1 (`game::roster::JET_TROOPER_FUEL_COST`, every
/// jet trooper's), and the BF2 twin's JetAcceleration 30 and jet row.
fn bf1_jet_trooper() -> SoldierConstants {
    SoldierConstants {
        jet_jump: 5.0,
        jet_push: 8.0,
        jet_acceleration: 30.0,
        jet_hover: true,
        jet_fuel_cost: 0.1,
        jet_fuel_initial: 0.25,
        jet_fuel_recharge: 0.1,
        jet_fuel_min_border: 0.24,
        jet_control: [1.5, 1.25, 1.25],
        ..trial_class(GroundRules::Bf1)
    }
}

/// A BF2 Jedi (`com_jedi_default` + Luke / Obi-Wan): JumpHeight 3.5,
/// JetJump 10, no fuel, 40 energy a jump.
fn jedi() -> SoldierConstants {
    SoldierConstants {
        jump_vy: (2.0 * GRAVITY * 3.5f32).sqrt(),
        jet_jump: 10.0,
        jet_push: 0.0,
        jet_acceleration: 10.0,
        jet_fuel_cost: 0.0,
        jet_fuel_initial: 0.0,
        jet_fuel_recharge: 0.0,
        jet_fuel_min_border: 0.0,
        jet_energy_drain: 40.0,
        jet_control: [1.5, 1.25, 1.25],
        energy_cost_jump: 0.0,
        saber: true,
        ..trial_class(GroundRules::Bf2)
    }
}

/// Vader: hover, a full tank to start, 30 energy (MaxSpeed 6.5 / 6.5).
fn vader() -> SoldierConstants {
    SoldierConstants {
        max_speed: 6.5,
        max_strafe_speed: 6.5,
        jet_jump: 7.0,
        jet_push: 7.0,
        jet_acceleration: 20.0,
        jet_hover: true,
        jet_fuel_cost: 0.3,
        jet_fuel_initial: 0.25,
        jet_fuel_recharge: 0.9,
        jet_fuel_min_border: 1.0,
        jet_energy_drain: 30.0,
        jet_control: [1.5, 1.25, 1.25],
        energy_cost_jump: 0.0,
        saber: true,
        ..trial_class(GroundRules::Bf2)
    }
}

fn pad(ly: i16, lx: i16, jump: bool) -> PadState {
    PadState {
        ly,
        lx,
        buttons: if jump { buttons::JUMP } else { 0 },
        ..Default::default()
    }
}

/// One tick over a floor at `floor` (BF2's rules), as a human.
fn tick_over(s: &mut Soldier, p: &PadState, c: &SoldierConstants, floor: f32, env: AirEnv) {
    let from = s.pos.y;
    s.step_looking(DT, p, c, GroundRules::Bf2, 1.0, env);
    s.resolve_ground(DT, floor, Vec3::Y, from, GroundRules::Bf2);
}

fn human() -> AirEnv {
    AirEnv::default()
}

/// Tap, release, tap: a standing jump, then the jet on the jump's second
/// update in the air with the stick `stick` (over a floor at `floor` once
/// he has left the one at 0).
fn jet_from_a_jump(c: &SoldierConstants, stick: (i16, i16), floor: f32) -> Soldier {
    let mut s = Soldier::new(Vec3::ZERO);
    s.fuel = 1.0;
    tick_over(&mut s, &pad(0, 0, true), c, 0.0, human());
    assert_eq!(s.stance, Stance::Jump);
    tick_over(&mut s, &pad(0, 0, false), c, floor, human());
    tick_over(&mut s, &pad(stick.0, stick.1, true), c, floor, human());
    s
}

#[test]
fn the_kick_points_by_the_stick_exactly() {
    let c = dark_trooper();
    // (fwd, strafe) -> (forward, right, up) shares of JetJump (§6).
    for ((f, r), want) in [
        ((0.0, 0.0), (0.0, 0.0, 1.0)),
        ((1.0, 0.0), (0.673, 0.0, 0.740)),
        ((-1.0, 0.0), (-0.0905, 0.0, 0.9959)),
        ((0.0, 1.0), (0.0, 0.0905, 0.9959)),
    ] {
        let mut s = Soldier::new(Vec3::new(0.0, 10.0, 0.0));
        s.stance = Stance::Fall;
        s.grounded = false;
        s.fuel = 1.0;
        assert!(s.jet_on(&c, (f, r)));
        // yaw 0: forward is -Z, right +X.
        let got = (-s.vel.z / 21.0, s.vel.x / 21.0, s.vel.y / 21.0);
        assert!(
            (got.0 - want.0).abs() < 1e-3
                && (got.1 - want.1).abs() < 1e-3
                && (got.2 - want.2).abs() < 1e-3,
            "stick ({f}, {r}): {got:?} vs {want:?}"
        );
        assert_eq!(s.stance, Stance::Jet);
        assert_eq!(s.fuel, 0.5, "the start's cost");
    }
}

#[test]
fn a_start_needs_the_border_and_an_undepleted_bar() {
    let c = dark_trooper();
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Fall;
    s.fuel = 0.4899;
    assert!(!s.jet_on(&c, (0.0, 0.0)), "under the border");
    s.fuel = 0.49;
    assert!(s.jet_on(&c, (0.0, 0.0)), "equal passes");
    assert_eq!(s.fuel, 0.0, "floored at 0");
    // A cost of 0 passes a depleted bar (FUN_001e2010); a cost does not.
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Fall;
    s.fuel = 1.0;
    s.energy_depleted = true;
    assert!(s.jet_on(&c, (0.0, 0.0)));
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Fall;
    s.energy_depleted = true;
    assert!(!s.jet_on(&jedi(), (0.0, 0.0)), "a depleted bar refuses");
    // A class with no jet keys never starts one: the default border 0.1
    // over a tank that never fills.
    let plain = trial_class(GroundRules::Bf2);
    let mut s = Soldier::new(Vec3::ZERO);
    for _ in 0..300 {
        tick_over(&mut s, &PadState::default(), &plain, 0.0, human());
    }
    assert_eq!(s.fuel, 0.0);
    s.stance = Stance::Fall;
    assert!(!s.jet_on(&plain, (0.0, 0.0)));
}

/// §6 "BF2 dark trooper": from a jump's second update with the stick
/// forward, vy ~ 8.0 - 0.6 + 15.5 = 22.9 m/s and an apex of about 14.6 m
/// over that point before the drag.
#[test]
fn the_dark_troopers_leap_rises_as_the_formula_says() {
    let c = dark_trooper();
    let mut s = jet_from_a_jump(&c, (FWD, 0), -1000.0);
    assert_eq!(s.stance, Stance::Jet);
    assert!((s.vel.y - 22.9).abs() < 0.05, "vy {}", s.vel.y);
    // The kick's 14.13 and the update's air stick (2.1 m/s^2).
    assert!((-s.vel.z - 14.2).abs() < 0.1, "forward {}", -s.vel.z);
    let from = s.pos.y;
    // §4.1 replayed in f64 outside the runtime: JetPush 0, the drag
    // exp(-0.012 |v| dt), gravity 18, the move.
    let (mut v, mut y, mut want) = (s.vel.as_dvec3(), 0.0f64, 0.0f64);
    let dt = f64::from(DT);
    let mut apex = from;
    for _ in 0..90 {
        tick_over(&mut s, &pad(0, 0, false), &c, -1000.0, human());
        apex = apex.max(s.pos.y);
        v *= f64::from(fmath::exp((-0.012 * v.length() * dt) as f32));
        v.y -= 18.0 * dt;
        y += v.y * dt;
        want = want.max(y);
    }
    let rise = apex - from;
    // 22.9^2 / 36 = 14.57 m without the drag (§6); the drag, about 0.3 / s
    // at 27 m/s, takes a fifth of it.
    assert!(
        (f64::from(rise) - want).abs() < 0.01,
        "apex {rise} vs {want}"
    );
    assert!(rise > 11.0 && rise < 14.57, "apex {rise}");
    assert_eq!(s.stance, Stance::Jet, "the jump-jet lasts: no fuel cost");
}

/// §6: fuel 1.0 -> 0.5 -> 0.0, two leaps from full; then 0.49 / 0.075 =
/// 6.5 s to the next.
#[test]
fn the_dark_trooper_leaps_twice_from_full_then_waits_six_and_a_half_seconds() {
    let c = dark_trooper();
    let mut s = Soldier::new(Vec3::new(0.0, 100.0, 0.0));
    s.stance = Stance::Fall;
    s.grounded = false;
    s.fuel = 1.0;
    assert!(s.jet_on(&c, (0.0, 0.0)));
    s.stance = Stance::Fall;
    assert!(s.jet_on(&c, (0.0, 0.0)));
    s.stance = Stance::Fall;
    assert!(!s.jet_on(&c, (0.0, 0.0)), "the third is refused");
    // On the floor, standing: the refill to the border.
    let mut s = Soldier::new(Vec3::ZERO);
    let mut ticks = 0;
    while s.fuel < c.jet_fuel_min_border {
        tick_over(&mut s, &PadState::default(), &c, 0.0, human());
        ticks += 1;
    }
    assert!((195..=197).contains(&ticks), "{ticks} updates");
    // One leap from the spawn's tank.
    assert_eq!(Soldier::spawn_fuel(&c), JET_SPAWN_FUEL);
}

/// A jet's tap ends it into the fall; no recharge while jetting (BF2, on
/// every map).
#[test]
fn a_third_tap_ends_the_jet_and_the_tank_waits() {
    let c = jet_trooper();
    let mut s = jet_from_a_jump(&c, (0, 0), -1000.0);
    assert_eq!(s.stance, Stance::Hover);
    let f0 = s.fuel;
    for _ in 0..30 {
        tick_over(&mut s, &pad(0, 0, false), &c, -1000.0, human());
    }
    assert!((s.fuel - (f0 - 0.16)).abs() < 1e-4, "drains, no recharge");
    let (pos, vel) = (s.pos, s.vel);
    tick_over(&mut s, &pad(0, 0, true), &c, -1000.0, human());
    assert_eq!(s.stance, Stance::Fall);
    assert_eq!(
        (s.pos, s.vel),
        (pos, vel),
        "the ending update does not move"
    );
    let f1 = s.fuel;
    tick_over(&mut s, &pad(0, 0, false), &c, -1000.0, human());
    assert!(s.fuel > f1, "the fall recharges");
    assert!(s.vel.y < vel.y, "and gravity is back");
}

/// A bot's pad taps nothing: the PS2's bots call the jet themselves
/// (card JETP2).
#[test]
fn a_bots_taps_start_no_jet() {
    let c = jet_trooper();
    let bot = AirEnv {
        human: false,
        ..AirEnv::default()
    };
    let mut s = Soldier::new(Vec3::ZERO);
    s.fuel = 1.0;
    for p in [pad(0, 0, true), pad(0, 0, false), pad(0, 0, true)] {
        tick_over(&mut s, &p, &c, -1000.0, bot);
    }
    assert_eq!(s.stance, Stance::Jump);
    assert_eq!(s.fuel, 1.0);
}

/// Hovering at pitch `p` with the stick `stick` for `ticks` updates.
fn hover(c: &SoldierConstants, p: f32, stick: (i16, i16), ticks: usize) -> Soldier {
    let mut s = jet_from_a_jump(c, (0, 0), -1000.0);
    assert_eq!(s.stance, Stance::Hover);
    s.pitch = p;
    s.vel = Vec3::ZERO;
    for _ in 0..ticks {
        tick_over(&mut s, &pad(stick.0, stick.1, false), c, -1000.0, human());
        s.fuel = 1.0; // the speed rows, not the tank
    }
    s
}

/// The aim pitch whose flight axis is level: sin(p) = -0.08 (4.6 deg down).
fn level_pitch() -> f32 {
    -fmath::asin(HOVER_LIFT)
}

/// §6 "BF2 jet trooper": 10.5 m/s along the aim (7 x 1.5), strafing 6.6
/// (5.25 x 1.25); level flight 4.6 deg down; level aim climbs 1.66 m/s.
/// Backing is §4.2's formula, MaxStrafeSpeed x the row's FORWARD factor,
/// 5.25 x 1.5 = 7.9 (§6's "6.6" takes the strafe factor; the port follows
/// the formula and its instructions).
#[test]
fn the_jet_troopers_hover_speeds_are_the_jet_rows() {
    let c = jet_trooper();
    assert!((level_pitch().to_degrees() + 4.59).abs() < 0.01);
    let s = hover(&c, level_pitch(), (FWD, 0), 60);
    assert!((-s.vel.z - 10.5).abs() < 1e-3, "forward {:?}", s.vel);
    assert!(s.vel.y.abs() < 1e-3, "level flight {}", s.vel.y);
    let s = hover(&c, level_pitch(), (-FWD, 0), 60);
    assert!((s.vel.z - 7.875).abs() < 1e-3, "backing {:?}", s.vel);
    let s = hover(&c, level_pitch(), (0, FWD), 60);
    assert!((s.vel.x - 6.5625).abs() < 1e-3, "strafe {:?}", s.vel);
    let s = hover(&c, 0.0, (FWD, 0), 60);
    assert!(
        (s.vel.y - 1.66).abs() < 0.01,
        "level aim climbs {}",
        s.vel.y
    );
    // The approach moves at most JetAcceleration x dt = 1 m/s an update
    // along the axis (from rest: 1.0, then 1.0 more less the fading).
    let s = hover(&c, level_pitch(), (FWD, 0), 1);
    assert!((-s.vel.z - 1.0).abs() < 1e-4, "{:?}", s.vel);
}

/// With the stick released the speed off the flight axis and the right
/// axis fades by e^(-2t); no gravity.
#[test]
fn a_released_hover_fades_by_e_to_the_minus_two_t() {
    let c = jet_trooper();
    let mut s = hover(&c, level_pitch(), (0, 0), 0);
    s.vel = Vec3::new(0.0, 5.0, 0.0);
    for _ in 0..30 {
        tick_over(&mut s, &PadState::default(), &c, -1000.0, human());
    }
    let want = 5.0 * fmath::exp(-2.0);
    assert!((s.vel.y - want).abs() < 1e-3, "{} vs {want}", s.vel.y);
}

/// §6: the start costs 0.25 and 0.16 a second burns the rest, 4.7 s from
/// full; empty to full again in 8.3 s.
#[test]
fn the_jet_trooper_flies_four_point_seven_seconds_and_refills_in_eight_point_three() {
    let c = jet_trooper();
    let mut s = jet_from_a_jump(&c, (0, 0), -1000.0);
    assert!((s.fuel - 0.75).abs() < 0.01, "{}", s.fuel);
    let mut ticks = 0;
    while s.stance == Stance::Hover {
        tick_over(&mut s, &pad(FWD, 0, false), &c, -1000.0, human());
        ticks += 1;
    }
    let secs = ticks as f32 * DT;
    assert!((secs - 4.69).abs() < 0.1, "{secs} s of flight");
    assert_eq!(s.stance, Stance::Fall, "empty: he falls");
    assert_eq!(s.fuel, 0.0);
    let mut s = Soldier::new(Vec3::ZERO);
    let mut ticks = 0;
    while s.fuel < 1.0 {
        tick_over(&mut s, &PadState::default(), &c, 0.0, human());
        ticks += 1;
    }
    assert!((ticks as f32 * DT - 8.33).abs() < 0.05, "{ticks} updates");
}

/// BF1's jet trooper on BF2's rules: BF2's mover with the borrowed row on
/// its own speeds (forward 6.5 x 1.5 = 9.75, strafe 4.5 x 1.25 = 5.625).
/// No recharge in flight, but at BF1's net cost, so it flies as long as on
/// BF1: (1 - 0.25) / 0.1 = 7.5 s from full (playtest 2026-11-05 B4).
#[test]
fn the_bf1_jet_trooper_flies_as_long_as_on_bf1() {
    let c = bf1_jet_trooper();
    let s = hover(&c, level_pitch(), (FWD, 0), 60);
    assert!((-s.vel.z - 9.75).abs() < 1e-3, "forward {:?}", s.vel);
    assert!(s.vel.y.abs() < 1e-3);
    let s = hover(&c, level_pitch(), (0, FWD), 60);
    assert!((s.vel.x - 5.625).abs() < 1e-3, "strafe {:?}", s.vel);
    let mut s = jet_from_a_jump(&c, (0, 0), -1000.0);
    let mut ticks = 0;
    while s.stance == Stance::Hover {
        tick_over(&mut s, &PadState::default(), &c, -1000.0, human());
        ticks += 1;
    }
    assert!((ticks as f32 * DT - 7.5).abs() < 0.1, "{ticks} updates");
}

/// The jet row's turn (1.25) for a human; a bot turns at 1.0.
#[test]
fn a_human_turns_by_the_jet_row_in_the_air() {
    let c = jet_trooper();
    let turn = PadState {
        rx: FWD,
        ..Default::default()
    };
    let mut yaw = Vec::new();
    for human in [true, false] {
        let mut s = hover(&c, level_pitch(), (0, 0), 0);
        let y0 = s.yaw;
        let env = AirEnv {
            human,
            ..AirEnv::default()
        };
        tick_over(&mut s, &turn, &c, -1000.0, env);
        yaw.push(y0 - s.yaw);
    }
    let one = c.max_turn_speed * DT;
    assert!((yaw[0] - 1.25 * one).abs() < 1e-5, "{yaw:?}");
    assert!((yaw[1] - one).abs() < 1e-5, "{yaw:?}");
}

/// A jet lands on the floor: stand, with the light lock (0.3 s).
#[test]
fn a_jet_lands_into_a_stand_with_the_light_lock() {
    let c = jet_trooper();
    let mut s = jet_from_a_jump(&c, (0, 0), 0.0);
    assert_eq!(s.stance, Stance::Hover);
    s.pitch = -0.8;
    let mut ticks = 0;
    while s.stance == Stance::Hover && ticks < 300 {
        tick_over(&mut s, &pad(FWD, 0, false), &c, 0.0, human());
        ticks += 1;
    }
    assert_eq!(s.stance, Stance::Stand, "landed after {ticks}");
    assert!((s.lock_timer - c.soft_lock).abs() < 1e-6);
    // The dark trooper's leap lands the same way.
    let c = dark_trooper();
    let mut s = jet_from_a_jump(&c, (0, 0), 0.0);
    assert_eq!(s.stance, Stance::Jet);
    let mut ticks = 0;
    while s.stance == Stance::Jet && ticks < 300 {
        tick_over(&mut s, &PadState::default(), &c, 0.0, human());
        ticks += 1;
    }
    assert_eq!(s.stance, Stance::Stand);
    assert!((s.lock_timer - c.soft_lock).abs() < 1e-6);
}

/// The ceiling (§5.3): within 5 m of it the feet are pulled toward 5 m
/// under it. A hover climbing at level aim under a 20 m ceiling settles
/// near 15 m; a jump is pulled down too.
#[test]
fn the_fly_height_ceiling_pulls_down_within_five_metres() {
    let mut s = Soldier::new(Vec3::new(0.0, 17.0, 0.0));
    s.fly_ceiling(DT, 20.0);
    assert!((s.vel.y - (3.0 - 5.0) * DT * 10.0).abs() < 1e-6);
    let mut s = Soldier::new(Vec3::new(0.0, 14.0, 0.0));
    s.fly_ceiling(DT, 20.0);
    assert_eq!(s.vel.y, 0.0, "outside the band");

    let c = jet_trooper();
    let env = AirEnv {
        ceiling: 20.0,
        human: true,
        bot_jet: None,
    };
    let mut s = hover(&c, 0.0, (FWD, 0), 0);
    s.pos.y = 10.0;
    for _ in 0..600 {
        tick_over(&mut s, &pad(FWD, 0, false), &c, -1000.0, env);
        s.fuel = 1.0;
    }
    assert!(s.pos.y > 14.5 && s.pos.y < 15.5, "settles at {}", s.pos.y);
    // A jump from a floor 1 m under the ceiling's band peaks lower.
    let mut free = Soldier::new(Vec3::new(0.0, 14.0, 0.0));
    let mut capped = free.clone();
    let (mut a, mut b) = (14.0f32, 14.0f32);
    for t in 0..40 {
        let p = pad(0, 0, t == 0);
        tick_over(&mut free, &p, &c, 14.0, human());
        tick_over(&mut capped, &p, &c, 14.0, env);
        a = a.max(free.pos.y);
        b = b.max(capped.pos.y);
    }
    assert!(b < a - 0.05, "capped apex {b} vs {a}");
}

/// §3.3: a sabre class that does not hover double-jumps: vy = 1.5 x the
/// jump speed (16.8 m/s for JumpHeight 3.5) and the stick's run speed,
/// paid with 40 energy; a depleted bar refuses.
#[test]
fn the_jedi_double_jump_rises_at_sixteen_point_eight() {
    let c = jedi();
    let mut s = jet_from_a_jump(&c, (FWD, 0), -1000.0);
    assert_eq!(s.stance, Stance::Jump);
    assert!((s.vel.y - 16.84).abs() < 0.01, "vy {}", s.vel.y);
    assert!((-s.vel.z - 7.0).abs() < 1e-4 && s.vel.x.abs() < 1e-4);
    assert!((s.energy - 60.0).abs() < 0.5, "energy {}", s.energy);
    assert_eq!(s.fuel, 1.0, "no fuel spent");
    // A third tap jumps again while the bar holds (§3.3 [D]).
    tick_over(&mut s, &pad(0, 0, false), &c, -1000.0, human());
    tick_over(&mut s, &pad(0, 0, true), &c, -1000.0, human());
    assert!((s.vel.y - 16.84).abs() < 0.01, "again {}", s.vel.y);
    assert!(s.energy < 25.0);
    // Falling fast, the jump's speed adds instead: vy + kick < J / 2.
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Fall;
    s.vel.y = -20.0;
    assert!(s.jet_on(&c, (0.0, 0.0)));
    let j = c.jump_vy;
    assert!((s.vel.y - (-20.0 + 10.0 + j)).abs() < 1e-4, "{}", s.vel.y);
    // No energy, no jump.
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Fall;
    s.energy_depleted = true;
    assert!(!s.jet_on(&c, (0.0, 0.0)));
    assert_eq!(s.stance, Stance::Fall);
}

/// Vader / the Emperor: a full tank to start (spawn 0.5 waits 0.56 s), 30
/// energy, a 2.5 s hover; a tap ends it and the sabre's re-try is refused.
#[test]
fn vaders_short_hover() {
    let c = vader();
    let mut s = Soldier::new(Vec3::ZERO);
    s.fuel = Soldier::spawn_fuel(&c);
    let mut ticks = 0;
    while s.fuel < 1.0 {
        tick_over(&mut s, &PadState::default(), &c, 0.0, human());
        ticks += 1;
    }
    assert!((ticks as f32 * DT - 0.56).abs() < 0.04, "{ticks}");
    let mut s = jet_from_a_jump(&c, (0, 0), -1000.0);
    assert_eq!(s.stance, Stance::Hover);
    assert!((s.energy - 70.0).abs() < 0.5, "energy {}", s.energy);
    let e = s.energy;
    let mut ticks = 0;
    while s.stance == Stance::Hover {
        tick_over(&mut s, &PadState::default(), &c, -1000.0, human());
        ticks += 1;
    }
    assert!((ticks as f32 * DT - 2.5).abs() < 0.1, "{ticks}");
    assert_eq!(s.energy, e, "the bar holds while he hovers");
    // A tap mid-hover: Fall, and the re-try is refused (tank under 1.0).
    let mut s = jet_from_a_jump(&c, (0, 0), -1000.0);
    tick_over(&mut s, &pad(0, 0, false), &c, -1000.0, human());
    tick_over(&mut s, &pad(0, 0, true), &c, -1000.0, human());
    assert_eq!(s.stance, Stance::Fall);
}

/// The fuel and the jet state are in the hash; a soldier with no fuel
/// hashes as before the field existed.
#[test]
fn fuel_and_the_jet_state_are_hashed() {
    let hash = |s: &Soldier| {
        let mut bytes = Vec::new();
        s.hash_into(&mut |b: &[u8]| bytes.extend_from_slice(b));
        bytes
    };
    let a = Soldier::new(Vec3::ZERO);
    let bytes = hash(&a);
    assert!(!bytes.windows(12).any(|w| w == b"SOLDIER_FUEL"));
    let mut b = a.clone();
    b.fuel = 0.5;
    assert_ne!(hash(&b), bytes);
    let mut c = b.clone();
    c.stance = Stance::Hover;
    assert_ne!(hash(&c), hash(&b));
}

#[test]
fn control_rows_parse() {
    assert_eq!(
        SoldierConstants::control_row("jet    1.50 1.25 1.25"),
        Some(("jet", [1.5, 1.25, 1.25]))
    );
    assert_eq!(SoldierConstants::control_row("jet 1.5"), None);
}

/// The BF2 jet trooper's hover, measured live on the PS2 (one row per game update,
/// `analysis/re-jetpack.md` §16): feet, velocity and fuel at updates since the jet
/// started. Tap, 12 updates, tap; the stick goes on 14 updates into the hover.
#[test]
fn the_hover_replays_the_ps2_capture() {
    let c = jet_trooper();
    let run = |ly: i16, lx: i16, pitch: f32| {
        let mut s = Soldier::new(Vec3::ZERO);
        s.fuel = 1.0;
        s.pitch = pitch;
        tick_over(&mut s, &pad(0, 0, true), &c, 0.0, human());
        for _ in 0..11 {
            tick_over(&mut s, &pad(0, 0, false), &c, -100.0, human());
        }
        tick_over(&mut s, &pad(0, 0, true), &c, -100.0, human());
        assert_eq!(s.stance, Stance::Hover);
        let start = s.pos.y;
        let mut rows = vec![(s.vel, s.pos.y - start, s.fuel)];
        for k in 1..=90 {
            let (y, x) = if k >= 14 { (ly, lx) } else { (0, 0) };
            tick_over(&mut s, &pad(y, x, false), &c, -100.0, human());
            rows.push((s.vel, s.pos.y - start, s.fuel));
        }
        rows
    };
    let near = |got: f32, want: f32, what: &str| {
        assert!((got - want).abs() <= 0.02, "{what}: {got}, PS2 {want}");
    };
    // Stick released at spawn pitch: up, axis y, the rise, and the fuel.
    let none = run(0, 0, -0.07);
    for (k, vy, rise, fuel) in [
        (0, 6.257, 0.0, 0.75),
        (5, 4.48, 0.86, 0.723),
        (10, 3.21, 1.47, 0.697),
        (20, 1.65, 2.23, 0.643),
        (45, 0.31, 2.87, 0.51),
        (90, 0.02, 3.01, 0.27),
    ] {
        near(none[k].0.y, vy, "vy");
        near(none[k].1, rise, "rise");
        near(none[k].2, fuel, "fuel");
    }
    // Held stick: the speeds it settles at. Forward and back settle the issue's old
    // question (7.875 m/s, not 6.6): MaxSpeed and MaxStrafeSpeed by the jet row.
    near(run(FWD, 0, -0.07)[60].0.z, -10.5, "forward");
    near(run(-FWD, 0, -0.07)[60].0.z, 7.88, "back");
    near(run(0, -FWD, -0.07)[60].0.x, -6.56, "strafe");
    let up = run(FWD, 0, 0.429);
    near(up[60].0.y, 7.8, "climb");
    near(up[60].0.z, -7.03, "climb forward");
}
