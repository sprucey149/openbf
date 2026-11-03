//! Reference vectors from re-stance-roll.md §5 (BF2-model subset).
use super::*;

mod jet;
mod juni;
mod motion;
mod prone_tap;

const DT: f32 = 1.0 / 30.0;

fn full_fwd() -> PadState {
    PadState {
        ly: 32767,
        ..Default::default()
    }
}

fn run(s: &mut Soldier, pad: &PadState, c: &SoldierConstants, ticks: usize) {
    for _ in 0..ticks {
        tick(s, pad, c);
    }
}

/// One tick on a level floor at y = 0, BF2's rules.
fn tick(s: &mut Soldier, pad: &PadState, c: &SoldierConstants) {
    s.step(DT, pad, c, GroundRules::Bf2);
    s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
}

#[test]
fn v2_stance_speed_caps() {
    let c = SoldierConstants::default();
    for (stance, want) in [
        (Stance::Stand, 9.0),
        (Stance::Crouch, 6.3),
        (Stance::Prone, 2.7),
    ] {
        let mut s = Soldier::new(Vec3::ZERO);
        s.stance = stance;
        run(&mut s, &full_fwd(), &c, 120);
        let sp = Vec2::new(s.vel.x, s.vel.z).length();
        assert!((sp - want).abs() < 1e-3, "{stance:?}: {sp} != {want}");
        assert_eq!(s.stance, stance, "no spurious transition");
    }
    // Backpedal cap = MaxStrafeSpeed (7.0) at full reverse, stand.
    let mut s = Soldier::new(Vec3::ZERO);
    let back = PadState {
        ly: -32767,
        ..Default::default()
    };
    run(&mut s, &back, &c, 120);
    assert!((Vec2::new(s.vel.x, s.vel.z).length() - 7.0).abs() < 1e-3);
}

#[test]
fn v5_roll_impulse_and_energy() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    // Moving at 13.5 m/s forward (sprint speed), roll button + stick fwd.
    s.vel = Vec3::new(0.0, 0.0, -13.5);
    let pad = PadState {
        ly: 32767,
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    };
    tick(&mut s, &pad, &c);
    assert_eq!(s.stance, Stance::Roll);
    assert!((s.energy - 90.0).abs() < 1.0, "energy {}", s.energy);
    // Impulse to exactly 1.7 * 9 = 15.3 on the entry tick (before the
    // near-zero roll steering meaningfully changes it).
    let sp = Vec2::new(s.vel.x, s.vel.z).length();
    assert!((sp - 15.3).abs() < 0.1, "roll speed {sp}");

    // From standstill: no impulse (speed <= 0.1 gate), still rolls.
    let mut s2 = Soldier::new(Vec3::ZERO);
    tick(&mut s2, &pad, &c);
    assert_eq!(s2.stance, Stance::Roll);
    assert!(Vec2::new(s2.vel.x, s2.vel.z).length() < 1.0);

    // Roll exits to stand with the soft lock.
    let mut ticks = 0;
    while s.stance == Stance::Roll && ticks < 60 {
        tick(&mut s, &PadState::default(), &c);
        ticks += 1;
    }
    assert_eq!(s.stance, Stance::Stand);
    assert!(s.lock_timer > 0.0, "soft lock armed");
}

#[test]
fn v6_sprint_ramp_and_hysteresis() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    let sprint = PadState {
        ly: 32767,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    // Run up to MaxSpeed first (sprint needs 0.9*MaxSpeed floor).
    run(&mut s, &sprint, &c, 100);
    assert_eq!(s.stance, Stance::Sprint);
    let sp = Vec2::new(s.vel.x, s.vel.z).length();
    assert!((sp - 13.5).abs() < 1e-2, "sprint cap {sp} != 13.5");
    assert!(s.energy < 100.0, "sprint drains energy");

    // Draining the pool empty ends the sprint and latches depleted.
    let mut s3 = s.clone();
    run(&mut s3, &sprint, &c, 200);
    assert_ne!(s3.stance, Stance::Sprint, "empty pool ends sprint");
    assert!(s3.energy_depleted || s3.energy >= ENERGY_RECOVER_THRESHOLD);

    // Strafe past the +-60 deg keep-cone -> drops to stand.
    let veer = PadState {
        ly: 16000,
        lx: 32767,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    tick(&mut s, &veer, &c);
    assert_eq!(s.stance, Stance::Stand);
}

#[test]
fn v7_contextual_button_cycles_or_rolls() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    let press = PadState {
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    };
    let release = PadState::default();
    // Stationary presses: stand -> crouch, a second tap inside the double
    // tap's 1 s -> prone, a prone tap -> stand (`prone_tap.rs`).
    let cycle = |s: &mut Soldier, wait: usize| {
        run(s, &release, &c, wait);
        tick(s, &press, &c);
    };
    cycle(&mut s, 20); // past the 0.5 s repeat gate
    assert_eq!(s.stance, Stance::Crouch);
    cycle(&mut s, 20); // 0.7 s after the first tap
    assert_eq!(s.stance, Stance::Prone);
    // Prone entry carries BF1's 1.333 s hard lock: movement is frozen
    // and the next cycle press is refused until it expires.
    assert!(s.lock_hard && s.lock_timer > 1.0);
    let mut frozen = s.clone();
    run(&mut frozen, &full_fwd(), &c, 10);
    assert_eq!(frozen.vel.length(), 0.0, "hard lock freezes movement");
    cycle(&mut s, 20); // still locked -> refused
    assert_eq!(s.stance, Stance::Prone);
    cycle(&mut s, 45); // lock expired (total > 40 frames)
    assert_eq!(s.stance, Stance::Stand);
    assert!(s.lock_hard && s.lock_timer > 1.4, "getup lock 1.5 s");

    // Moving press = roll, and straight-back is excluded.
    let back = PadState {
        ly: -32767,
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    };
    run(&mut s, &release, &c, 20);
    tick(&mut s, &back, &c);
    assert_ne!(s.stance, Stance::Roll, "straight-back cone excluded");
}

#[test]
fn jump_ballistics() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    let jump = PadState {
        buttons: buttons::JUMP,
        ..Default::default()
    };
    // The take-off update: the jump's velocity, the feet still down.
    tick(&mut s, &jump, &c);
    assert_eq!(s.stance, Stance::Jump);
    assert!(s.vel.y == 8.0 && s.pos.y == 0.0 && !s.grounded);
    assert!((s.energy - 90.0).abs() < 1.0);
    // The first moving update: BF2's drag and no gravity; then gravity.
    let drag = fmath::exp(-AIR_DRAG * 8.0 * DT);
    tick(&mut s, &PadState::default(), &c);
    assert!((s.vel.y - 8.0 * drag).abs() < 1e-5, "vy {}", s.vel.y);
    assert!((s.pos.y - 8.0 * drag * DT).abs() < 1e-5);
    tick(&mut s, &PadState::default(), &c);
    assert!(s.vel.y < 8.0 * drag - GRAVITY * DT + 1e-4);
    for _ in 0..45 {
        tick(&mut s, &PadState::default(), &c);
    }
    assert_eq!(s.stance, Stance::Stand, "landed");
    assert_eq!(s.pos.y, 0.0);
}

/// A plane through the origin falling toward -Z (the way a soldier at
/// yaw 0 walks) at `deg`: its normal and its height under a point.
fn slope(deg: f32) -> (Vec3, impl Fn(Vec3) -> f32) {
    let (sn, cs) = deg.to_radians().sin_cos();
    (Vec3::new(0.0, cs, -sn), move |p: Vec3| p.z * sn / cs)
}

fn tick_on(
    s: &mut Soldier,
    pad: &PadState,
    c: &SoldierConstants,
    rules: GroundRules,
    n: Vec3,
    floor: &impl Fn(Vec3) -> f32,
) {
    let from_y = s.pos.y;
    s.step(DT, pad, c, rules);
    let y = floor(s.pos);
    s.resolve_ground(DT, y, n, from_y, rules);
}

/// Card STHOP: the KAM1 ramp (34.3 deg). The PS2's player is grounded on
/// every planar update at MaxSpeed ALONG the plane, with a real vy
/// (re-soldier-ground.md §2.1); the old level-velocity step left the
/// floor on the second tick.
#[test]
fn a_soldier_walks_down_a_ramp_along_its_plane() {
    let c = SoldierConstants::default();
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let (n, floor) = slope(34.3);
        let mut s = Soldier::new(Vec3::ZERO);
        s.ground_n = n; // standing on it, as the last contact left him
        for t in 0..90 {
            tick_on(&mut s, &full_fwd(), &c, rules, n, &floor);
            assert!(s.grounded, "{rules:?}: airborne at tick {t}");
            assert_eq!(s.stance, Stance::Stand);
            assert_eq!(s.pos.y, floor(s.pos), "the feet are on the floor");
        }
        assert!(s.vel.dot(n).abs() < 1e-3, "the velocity lies in the plane");
        // BF2: MaxSpeed along the plane. BF1: the stick x |N.y|.
        let want = match rules {
            GroundRules::Bf2 => 9.0,
            GroundRules::Bf1 => 9.0 * n.y,
        };
        assert!((s.vel.length() - want).abs() < 1e-2, "{rules:?}: {}", s.vel);
        assert!(s.vel.y < -4.0, "walking down has a real vy: {}", s.vel.y);
    }
}

/// BF2 FUN_002480a0: an uphill target is scaled by 1 - (uphill part)^2 /
/// |target|^2 = cos^2 on a straight climb (PCSX2: 4.87 m/s up 34 deg at
/// MaxSpeed 7, §2.2). BF1 scales by |N.y| either way.
#[test]
fn an_uphill_walk_is_slowed_by_the_games_rule() {
    let c = SoldierConstants::default();
    let (n, floor) = slope(34.0);
    let back = Vec3::new(0.0, n.y, -n.z); // the same slope, rising toward -Z
    let up_floor = |p: Vec3| -floor(p);
    for (rules, want) in [
        (GroundRules::Bf2, 9.0 * n.y * n.y),
        (GroundRules::Bf1, 9.0 * n.y),
    ] {
        let mut s = Soldier::new(Vec3::ZERO);
        for _ in 0..90 {
            tick_on(&mut s, &full_fwd(), &c, rules, back, &up_floor);
        }
        assert!(s.grounded && s.vel.y > 0.0);
        assert!((s.vel.length() - want).abs() < 1e-2, "{rules:?}: {}", s.vel);
    }
}

/// The contact rule (BF1 FUN_00202f38 / BF2 FUN_0023cfd0): 2 cm, n.y > 0.6,
/// approach < 0.1.
#[test]
fn floor_contact_needs_two_centimetres_a_walkable_normal_and_no_departure() {
    let r = GroundRules::Bf2;
    let at = |y: f32, vel: Vec3, n: Vec3| {
        let mut s = Soldier::new(Vec3::new(0.0, y, 0.0));
        s.vel = vel;
        s.resolve_ground(DT, 0.0, n, y + 1.0, r);
        s
    };
    // A fold that falls away 1.5 cm keeps the contact and sets the feet down.
    let s = at(0.015, Vec3::ZERO, Vec3::Y);
    assert!(s.grounded && s.pos.y == 0.0 && s.air_time == 0.0);
    // 5 cm does not: airborne, N = up, the feet where they were.
    let s = at(0.05, Vec3::ZERO, Vec3::Y);
    assert!(!s.grounded && s.pos.y == 0.05 && s.ground_n == Vec3::Y);
    assert!((s.air_time - DT).abs() < 1e-7);
    // Leaving the floor at 0.1 m/s or more is no contact (a jump's first tick).
    assert!(!at(0.0, Vec3::new(0.0, 0.2, 0.0), Vec3::Y).grounded);
    // A landing takes the into-floor velocity and keeps the rest.
    let s = at(-0.1, Vec3::new(3.0, -9.0, 0.0), Vec3::Y);
    assert!(s.grounded && s.vel == Vec3::new(3.0, 0.0, 0.0) && s.pos.y == 0.0);
    // n.y 0.6 (53.13 deg) is not a floor: he is held out of it, airborne.
    let steep = Vec3::new(0.8, 0.6, 0.0);
    let s = at(-0.1, Vec3::new(0.0, -5.0, 0.0), steep);
    assert!(!s.grounded && s.pos.y == 0.0);
    assert!(
        s.vel.dot(steep).abs() < 1e-6,
        "he slides along it: {}",
        s.vel
    );
    // ... unless it no longer lets him slide (his feet did not move in
    // height this tick): wedged, he stands, level.
    let mut s = Soldier::new(Vec3::ZERO);
    s.vel = Vec3::new(0.9, -1.34, 0.57);
    s.grounded = false;
    s.resolve_ground(DT, 0.0, steep, 0.0, r);
    assert!(s.grounded && s.ground_n == Vec3::Y && s.vel.y == 0.0);
    assert!(at(0.0, Vec3::ZERO, Vec3::new(0.78, 0.626, 0.0).normalize()).grounded);
}

/// Stand falls by AIRTIME, not height: BF2 after 1.0 s (PCSX2: the update
/// after the timer reads 1.000, §2.3); BF1 after 0.7 s, or 1.0 s for a
/// stance at least 5 s old. Crouch stands up after 0.3 s.
#[test]
fn stand_falls_after_the_games_airtime() {
    let c = SoldierConstants::default();
    let ticks_to_fall = |rules: GroundRules, age_ticks: usize, stance: Stance| {
        let mut s = Soldier::new(Vec3::ZERO);
        s.stance = stance;
        for _ in 0..age_ticks {
            s.step(DT, &PadState::default(), &c, rules);
            s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
        }
        for t in 1..=60 {
            s.step(DT, &PadState::default(), &c, rules);
            s.resolve_ground(DT, crate::collision::NO_GROUND, Vec3::Y, 0.0, rules);
            if s.stance != stance {
                return (t, s.stance);
            }
        }
        (0, s.stance)
    };
    // air_time > limit on the first tick past it (f32 sums of 1/30).
    let (t, st) = ticks_to_fall(GroundRules::Bf2, 0, Stance::Stand);
    assert!(
        (30..=31).contains(&t) && st == Stance::Fall,
        "BF2: {t} {st:?}"
    );
    let (t, st) = ticks_to_fall(GroundRules::Bf1, 0, Stance::Stand);
    assert!(
        (21..=22).contains(&t) && st == Stance::Fall,
        "BF1 young: {t} {st:?}"
    );
    let (t, st) = ticks_to_fall(GroundRules::Bf1, 160, Stance::Stand);
    assert!(
        (30..=31).contains(&t) && st == Stance::Fall,
        "BF1 old: {t} {st:?}"
    );
    let (t, st) = ticks_to_fall(GroundRules::Bf2, 0, Stance::Crouch);
    assert!(
        (9..=10).contains(&t) && st == Stance::Stand,
        "crouch: {t} {st:?}"
    );
}

/// Airborne in stand, BF2 keeps the full ground acceleration and BF1 a
/// fifth of it (FUN_002098b8's x0.2); gravity is -18 in both.
#[test]
fn air_control_in_stand_is_full_in_bf2_and_a_fifth_in_bf1() {
    let c = SoldierConstants::default();
    for (rules, want) in [(GroundRules::Bf2, 70.0 * DT), (GroundRules::Bf1, 14.0 * DT)] {
        let mut s = Soldier::new(Vec3::new(0.0, 10.0, 0.0));
        s.grounded = false;
        s.step(DT, &full_fwd(), &c, rules);
        assert!((-s.vel.z - want).abs() < 1e-4, "{rules:?}: {}", s.vel);
        assert!((s.vel.y + GRAVITY * DT).abs() < 1e-5);
    }
}

/// The steep-floor limits: BF2 x0.1 under N.y 0.7, where the slope's
/// gravity out-pulls the stick; BF1 x0.01 under 0.75 with its 0.4 *
/// Acceleraton pull. Either way a soldier pushing UP a 50 deg floor
/// (n.y 0.64, still a floor) from a stand slides down it. Card SKIRT: first
/// the flattest normal's full-hold tick and the lift-offs at the top of each
/// climb (BF2's air control is full) carry him up a little - the PS2's
/// soldier hangs 0.3-0.7 m up its 48 deg band from a stand at the foot
/// (re-soldier-ground.md §7.1) - and no further than a metre.
#[test]
fn a_floor_steeper_than_the_limit_slides_the_soldier_down() {
    let c = SoldierConstants::default();
    let (n, floor) = slope(50.0);
    let back = Vec3::new(0.0, n.y, -n.z);
    let up_floor = |p: Vec3| -floor(p);
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        let mut s = Soldier::new(Vec3::ZERO);
        let mut high = 0.0f32;
        for _ in 0..240 {
            tick_on(&mut s, &full_fwd(), &c, rules, back, &up_floor);
            high = high.max(s.pos.y);
        }
        assert!(s.pos.y < -0.5, "{rules:?}: he slid to y {}", s.pos.y);
        assert!(high < 1.0, "{rules:?}: he got {high} m up");
    }
}

/// One jump on a level floor, as the PS2 trials ran it (`lead_tmp/airc/ps2`,
/// `lead_tmp/audit-airc/ps2`): `run` ticks of the `ground` stick, JUMP
/// pressed with that stick still on the pad, then `air` held to the
/// landing.
struct Trial {
    /// (forward, side, up) velocity right after the take-off update.
    take_off: Vec3,
    /// ...and after every update that moved him in the air (index 0 = the
    /// PS2's update +1).
    vels: Vec<Vec3>,
    apex: f32,
    /// (forward, side) travel from the take-off update's START to the
    /// landing: it includes that update's ground move.
    travel: Vec2,
}

fn air_trial(
    rules: GroundRules,
    c: &SoldierConstants,
    (run, ground): (usize, i16),
    air: (i16, i16),
) -> Trial {
    let mut s = Soldier::new(Vec3::ZERO);
    s.stand_age = 1.0; // match the historical numeric trial setup
    let go = |s: &mut Soldier, pad: &PadState| {
        s.step(DT, pad, c, rules);
        s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
    };
    let ground = PadState {
        ly: ground,
        ..Default::default()
    };
    for _ in 0..run {
        go(&mut s, &ground);
    }
    let from = s.pos;
    go(
        &mut s,
        &PadState {
            buttons: buttons::JUMP,
            ..ground
        },
    );
    assert_eq!(s.stance, Stance::Jump);
    assert!(!s.grounded && s.pos.y == 0.0);
    // -Z is forward and +X right at yaw 0.
    let fsu = |s: &Soldier| Vec3::new(-s.vel.z, s.vel.x, s.vel.y);
    let take_off = fsu(&s);
    let pad = PadState {
        ly: air.0,
        lx: air.1,
        ..Default::default()
    };
    let (mut vels, mut apex) = (Vec::new(), 0.0f32);
    while !s.grounded && vels.len() < 60 {
        go(&mut s, &pad);
        apex = apex.max(s.pos.y);
        // The landing update moves him too (its last piece of the fall).
        vels.push(fsu(&s));
    }
    Trial {
        take_off,
        vels,
        apex,
        travel: Vec2::new(from.z - s.pos.z, s.pos.x - from.x),
    }
}

/// The classes the PS2 trials ran: BF2's rebel soldier (MaxSpeed 7.0 /
/// 5.25, JumpHeight 1.78) and BF1's (6.5 / 4.5, jump velocity 8.0).
fn trial_class(rules: GroundRules) -> SoldierConstants {
    match rules {
        GroundRules::Bf2 => SoldierConstants {
            max_speed: 7.0,
            max_strafe_speed: 5.25,
            jump_vy: 8.005,
            ..SoldierConstants::default()
        },
        GroundRules::Bf1 => SoldierConstants {
            max_speed: 6.5,
            max_strafe_speed: 4.5,
            ..SoldierConstants::default()
        },
    }
}

const BOTH: [GroundRules; 2] = [GroundRules::Bf2, GroundRules::Bf1];
const FWD: i16 = 32767;

/// JARC / JUNI: BF2 numeric apex and first-air witnesses with matched BF2
/// class constants under both map rules. Original BF1 rows: re-stance-roll.md §4.3b.
#[test]
fn a_jump_rises_as_high_and_as_long_as_the_ps2s() {
    for (rules, apex, vy) in [
        (GroundRules::Bf2, 1.861, [8.0050, 7.9794, 7.3540]),
        (GroundRules::Bf1, 1.861, [8.0050, 7.9794, 7.3540]),
    ] {
        let t = air_trial(rules, &trial_class(GroundRules::Bf2), (0, 0), (0, 0));
        assert_eq!(t.vels.len(), 28, "{rules:?}: moving updates");
        assert!((t.apex - apex).abs() < 0.02, "{rules:?}: apex {}", t.apex);
        let got = [t.take_off.z, t.vels[0].z, t.vels[1].z];
        for (g, w) in got.iter().zip(vy) {
            assert!((g - w).abs() < 2e-3, "{rules:?}: vy {got:?}");
        }
        assert!(t.travel.length() < 1e-5, "no stick, no drift");
    }
}

/// AIRC / JUNI: BF2 numeric standing-air witnesses under both map rules,
/// matched BF2 class. Original per-game rows: re-stance-roll.md §4.3a.
#[test]
fn a_standing_jump_steers_by_the_stick_as_the_ps2_does() {
    // (rules, update -> PS2 forward speed, PS2 travel, PS2 strafe speed at update 20, strafe travel)
    let cases = [
        (
            GroundRules::Bf2,
            [(1, 0.0700), (5, 0.3481), (14, 0.9719), (20, 1.3863)],
            0.935,
            1.3864,
            0.935,
        ),
        (
            GroundRules::Bf1,
            [(1, 0.0700), (5, 0.3481), (14, 0.9719), (20, 1.3863)],
            0.935,
            1.3864,
            0.935,
        ),
    ];
    for (rules, speeds, travel, strafe_v, strafe_travel) in cases {
        let c = trial_class(GroundRules::Bf2);
        let t = air_trial(rules, &c, (0, 0), (FWD, 0));
        for (k, want) in speeds {
            let got = t.vels[k - 1].x;
            assert!(
                (got - want).abs() < 0.004,
                "{rules:?} update {k}: {got} vs {want}"
            );
        }
        let (d, v) = (t.travel, &t.vels);
        assert!((d.x - travel).abs() < 0.03, "{rules:?}: {} m", d.x);
        assert!(d.y.abs() < 1e-4);
        // Back is forward mirrored under the merged air law.
        let b = air_trial(rules, &c, (0, 0), (-FWD, 0));
        assert!((b.vels[19].x + v[19].x).abs() < 1e-4 && (b.travel.x + d.x).abs() < 1e-4);
        // The same air law acts on the side axis.
        let s = air_trial(rules, &c, (0, 0), (0, -FWD));
        assert!(
            (s.vels[19].y + strafe_v).abs() < 0.004,
            "{rules:?}: {}",
            s.vels[19].y
        );
        assert!(
            (-s.travel.y - strafe_travel).abs() < 0.03,
            "{rules:?}: {}",
            s.travel.y
        );
        assert!(s.vels[19].x.abs() < 1e-4);
    }
}

/// JARC / JUNI: BF2 numeric same-update stick and jump witness on both
/// map rules with matched BF2 constants; ground movement precedes take-off.
#[test]
fn stick_and_jump_together_from_a_stand_is_a_forward_leap() {
    for (rules, v0, travel) in [
        (GroundRules::Bf2, 3.045, 3.75),
        (GroundRules::Bf1, 3.045, 3.75),
    ] {
        let t = air_trial(rules, &trial_class(GroundRules::Bf2), (0, FWD), (FWD, 0));
        let got = t.take_off.x;
        assert!((got - v0).abs() < 0.02, "{rules:?}: take-off {got} m/s");
        let d = t.travel.x;
        assert!((d - travel).abs() < 0.1, "{rules:?}: {d} m vs {travel}");
    }
}

/// JARC / JUNI: matched BF2 class running take-off on both map rules.
/// Original BF1 additive planar boost remains documented in re-stance-roll.md §4.3b.
#[test]
fn a_running_jump_takes_off_by_the_merged_rule() {
    for rules in BOTH {
        let c = trial_class(GroundRules::Bf2);
        let v = |stick| air_trial(rules, &c, (21, stick), (0, 0)).take_off;
        assert!((v(FWD).x - 9.10).abs() < 0.01);
        assert!((v(-FWD).x + 5.25).abs() < 0.01);
    }
}

/// Card JARC 5: BF2 ADDS the jump velocity (PS2: a jump on the update the
/// feet came down at vy -1.0695 took off at 6.9355,
/// `bf2_run_release_2.csv` rows 18-19), BF1 sets it. In the port a
/// grounded soldier has a vy walking a ramp: down the 34.3 deg KAM1 ramp
/// BF2's jump is weaker by it; BF1's was not. The port adds on BF1 maps too
/// (named deviation, the maintainer 2026-10-25: one jump rule for the merged game).
#[test]
fn the_jump_adds_to_the_vertical_speed_in_both_games() {
    let c = trial_class(GroundRules::Bf2);
    let mut s = Soldier::new(Vec3::ZERO);
    s.vel = Vec3::new(0.0, -1.0695, 0.0);
    s.take_off(&c);
    assert!((s.vel.y - 6.9355).abs() < 1e-4);
    for rules in BOTH {
        let c = trial_class(GroundRules::Bf2);
        let (n, floor) = slope(34.3);
        let mut s = Soldier::new(Vec3::ZERO);
        s.ground_n = n;
        for _ in 0..20 {
            tick_on(&mut s, &full_fwd(), &c, rules, n, &floor);
        }
        let down = s.vel.y;
        assert!(s.grounded && down < -1.0, "{rules:?}: vy {down}");
        let pad = PadState {
            buttons: buttons::JUMP,
            ..full_fwd()
        };
        tick_on(&mut s, &pad, &c, rules, n, &floor);
        assert_eq!(s.stance, Stance::Jump);
        assert!(s.vel.y < c.jump_vy - 1.0, "{rules:?}: vy {}", s.vel.y);
    }
}

/// AIRC / JUNI: retain BF2 numeric running-air witnesses on both map rules
/// with matched BF2 constants; independent multi-class rows live in juni.rs.
#[test]
fn a_running_jump_keeps_its_speed_and_the_stick_bends_it() {
    // (rules, the air stick (fwd, side), PS2 speed at update 20, PS2 travel)
    let cases = [
        (GroundRules::Bf2, (0, 0), 8.4160, 7.740),
        (GroundRules::Bf2, (FWD, 0), 9.7259, 8.571),
        (GroundRules::Bf2, (-FWD, 0), 7.1000, 6.906),
        (GroundRules::Bf2, (0, -FWD), 8.4149, 7.544),
        (GroundRules::Bf1, (0, 0), 8.4160, 7.740),
        (GroundRules::Bf1, (FWD, 0), 9.7259, 8.571),
        (GroundRules::Bf1, (-FWD, 0), 7.1000, 6.906),
        (GroundRules::Bf1, (0, -FWD), 8.4149, 7.544),
    ];
    for (rules, air, speed, travel) in cases {
        let c = trial_class(GroundRules::Bf2);
        let t = air_trial(rules, &c, (21, FWD), air);
        assert!(
            (t.vels[19].x - speed).abs() < 0.02,
            "{rules:?} {air:?}: {} m/s vs {speed}",
            t.vels[19].x
        );
        // The PS2's travel is from its take-off row, a ground move later
        // than the trial's start.
        let d = t.travel.x - c.max_speed * DT;
        assert!(
            (d - travel).abs() < 0.03 * travel + 0.05,
            "{rules:?} {air:?}: {d} m vs {travel}"
        );
    }
}

const JUMP: PadState = PadState {
    buttons: buttons::JUMP,
    lx: 0,
    ly: 0,
    rx: 0,
    ry: 0,
    mx: 0,
    my: 0,
    fire_lag: 0,
    pick: None,
};

/// One tick with the floor at y = 0, or with none under the feet.
fn go(s: &mut Soldier, pad: &PadState, c: &SoldierConstants, rules: GroundRules, floor: bool) {
    let y = if floor {
        0.0
    } else {
        crate::collision::NO_GROUND
    };
    s.step(DT, pad, c, rules);
    s.resolve_ground(DT, y, Vec3::Y, 0.0, rules);
}

/// A soldier who walked `age` ticks on the floor (`stance`: stand, or a
/// sprint once at speed), then JUMP pressed on the `off`-th update that
/// began off the floor (0 = on it). Returns him and the vy that update
/// began with.
fn press_off_the_floor(rules: GroundRules, sprint: bool, age: usize, off: usize) -> (Soldier, f32) {
    let c = trial_class(rules);
    let walk = PadState {
        ly: FWD,
        buttons: if sprint { buttons::SPRINT } else { 0 },
        ..Default::default()
    };
    let mut s = Soldier::new(Vec3::ZERO);
    for _ in 0..age {
        go(&mut s, &walk, &c, rules, true);
    }
    let want = if sprint {
        Stance::Sprint
    } else {
        Stance::Stand
    };
    assert_eq!(s.stance, want);
    for _ in 0..off {
        go(&mut s, &walk, &c, rules, false);
        assert!(!s.grounded && s.stance == want);
    }
    let vy = s.vel.y;
    let press = PadState {
        buttons: walk.buttons | buttons::JUMP,
        ..walk
    };
    go(&mut s, &press, &c, rules, off == 0);
    (s, vy)
}

/// JGATE / JUNI: both maps use the BF2 0.3 s late-press window and
/// vertical addition. BF1 original age restrictions remain in re-stance-roll.md §4.3c.
#[test]
fn a_press_within_three_tenths_of_losing_the_floor_still_jumps() {
    for sprint in [false, true] {
        for off in 0..=12 {
            let (s, vy) = press_off_the_floor(GroundRules::Bf2, sprint, 40, off);
            let took = s.stance == Stance::Jump;
            assert_eq!(took, off <= 9, "BF2 sprint {sprint}: off {off}");
            if took && off > 0 {
                let want = vy - GRAVITY * DT + 8.005;
                assert!((s.vel.y - want).abs() < 1e-4, "off {off}: vy {}", s.vel.y);
            }
        }
    }
    // The PS2's row (`bf2_b_sprint_rel.csv` 29-30): 7.527 = 0.122 - 0.6 + 8.005.
    let c = trial_class(GroundRules::Bf2);
    let mut s = Soldier::new(Vec3::new(0.0, 0.3, 0.0));
    (s.grounded, s.air_time, s.vel.y) = (false, DT, 0.122);
    go(&mut s, &JUMP, &c, GroundRules::Bf2, false);
    assert!((s.vel.y - 7.527).abs() < 1e-3, "vy {}", s.vel.y);
    // ...and the next update is an ordinary airborne one: gravity.
    let vy = s.vel.y;
    go(&mut s, &PadState::default(), &c, GroundRules::Bf2, false);
    assert!(s.vel.y < vy - 0.5);

    // BF1 now uses the same window for young and old stances.
    for off in 0..=12 {
        let (old, vy) = press_off_the_floor(GroundRules::Bf1, false, 160, off);
        assert_eq!(old.stance == Stance::Jump, off <= 9, "BF1 old: off {off}");
        // The take-off adds (the merged game's rule, as BF2): off the floor
        // the jump rides on the fall already under way.
        if old.stance == Stance::Jump && off == 0 {
            assert_eq!(old.vel.y, 8.0);
        } else if old.stance == Stance::Jump {
            let want = vy - GRAVITY * DT + 8.0;
            assert!(
                (old.vel.y - want).abs() < 1e-4,
                "BF1 off {off}: vy {}",
                old.vel.y
            );
        }
        let (young, _) = press_off_the_floor(GroundRules::Bf1, false, 40, off);
        assert_eq!(young.stance == Stance::Jump, off <= 9, "BF1 young: {off}");
    }
    // A young stance that left the floor and touched down is at 5 s.
    let c = trial_class(GroundRules::Bf1);
    let mut s = Soldier::new(Vec3::ZERO);
    let idle = PadState::default();
    for floor in [true, false, false, true, false, false] {
        go(&mut s, &idle, &c, GroundRules::Bf1, floor);
    }
    go(&mut s, &JUMP, &c, GroundRules::Bf1, false);
    assert_eq!(s.stance, Stance::Jump);
}

/// JGATE / JUNI: crouch still rises after its 0.5 s gate, then a fresh
/// press jumps on every map. The shared landing lock remains; the former
/// BF1 additional stand delay is documented in re-stance-roll.md §4.3c.
#[test]
fn crouch_jump_rises_then_the_next_press_jumps_on_every_map() {
    let idle = PadState::default();
    for rules in BOTH {
        let c = trial_class(rules);
        let mut s = Soldier::new(Vec3::ZERO);
        s.stance = Stance::Crouch;
        for _ in 0..6 {
            go(&mut s, &idle, &c, rules, true);
        }
        // 0.23 s into the crouch: nothing, stick or not.
        let lean = PadState { ly: FWD, ..JUMP };
        go(&mut s, &lean, &c, rules, true);
        assert!(s.stance == Stance::Crouch && s.vel.y == 0.0 && s.grounded);
        for _ in 0..10 {
            go(&mut s, &idle, &c, rules, true);
        }
        go(&mut s, &lean, &c, rules, true);
        assert_eq!(s.stance, Stance::Stand, "{rules:?}");
        assert!(s.vel.y == 0.0 && s.grounded && s.lock_timer == 0.0);
        assert_eq!(s.energy, c.energy_bar, "standing up costs nothing");
        // JUNI: the next press, 0.1 s on, jumps on both maps.
        for _ in 0..2 {
            go(&mut s, &idle, &c, rules, true);
        }
        go(&mut s, &JUMP, &c, rules, true);
        assert_eq!(s.stance, Stance::Jump, "{rules:?}");
    }
    // Both maps accept a new press after the shared landing lock.
    for rules in BOTH {
        let c = trial_class(rules);
        let mut s = Soldier::new(Vec3::ZERO);
        s.stand_age = 1.0;
        go(&mut s, &JUMP, &c, rules, true);
        while s.stance != Stance::Stand {
            go(&mut s, &idle, &c, rules, true);
        }
        for _ in 0..11 {
            go(&mut s, &idle, &c, rules, true);
        }
        go(&mut s, &JUMP, &c, rules, true);
        assert_eq!(s.stance, Stance::Jump, "{rules:?} at 0.4 s");
    }
}

/// Card JGATE 4, BF2 with the stick diagonal and the jump on one update
/// from a stand (PS2 `bf2_stand_diagl_pre.csv`: 3.035 m/s forward, 2.335
/// aside): the forward part x JumpForwardSpeedFactor, the side part x
/// JumpStrafeSpeedFactor.
#[test]
fn bf2s_jump_boosts_the_forward_part_and_not_the_side_part() {
    let c = trial_class(GroundRules::Bf2);
    let mut s = Soldier::new(Vec3::ZERO);
    let pad = PadState {
        ly: FWD,
        lx: -FWD,
        ..JUMP
    };
    go(&mut s, &pad, &c, GroundRules::Bf2, true);
    assert_eq!(s.stance, Stance::Jump);
    assert!((-s.vel.z - 3.035).abs() < 0.005, "forward {}", -s.vel.z);
    assert!((-s.vel.x - 2.335).abs() < 0.005, "aside {}", -s.vel.x);
    // The class's own side factor is read.
    let c2 = SoldierConstants {
        jump_strafe_factor: 0.5,
        ..c
    };
    let mut s = Soldier::new(Vec3::ZERO);
    go(&mut s, &pad, &c2, GroundRules::Bf2, true);
    assert!((-s.vel.x - 2.335 * 0.5).abs() < 0.005, "aside {}", -s.vel.x);
}

struct Props(&'static [(&'static str, &'static str)]);
impl crate::weapon::PropSource for Props {
    fn prop(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
    fn prop_hash(&self, _: u32) -> Option<&str> {
        None
    }
}

/// Card JGATE 5: BF2's JumpHeight is stored as the velocity
/// sqrtf((h + h) x 18) (FUN_00252bb0 B2:170972-170976); a class without
/// the key keeps the constructor's 8.0 (BF1).
#[test]
fn jump_height_is_read_as_a_take_off_velocity() {
    let vy = |props| SoldierConstants::from_props(&Props(props)).jump_vy;
    assert!((vy(&[("JumpHeight", "1.78")]) - 8.005).abs() < 1e-3);
    assert!((vy(&[("JumpHeight", "3.5")]) - 11.225).abs() < 1e-3);
    assert_eq!(vy(&[("MaxSpeed", "7.0")]), 8.0);
}

/// Card JGATE: the jump's energy take refuses only a depleted bar and
/// overdraws a short one (BF2 FUN_001e2010); a sprint jump pays the
/// dearer cost (FUN_00247528).
#[test]
fn a_jump_overdraws_the_energy_bar_and_a_depleted_one_refuses() {
    let c = SoldierConstants::default();
    let mut s = Soldier::new(Vec3::ZERO);
    s.energy = 4.0;
    tick(&mut s, &JUMP, &c);
    assert_eq!(s.stance, Stance::Jump);
    assert!(s.energy == 0.0 && s.energy_depleted);
    let mut s = Soldier::new(Vec3::ZERO);
    (s.energy, s.energy_depleted) = (20.0, true);
    tick(&mut s, &JUMP, &c);
    assert!(s.stance == Stance::Stand && s.grounded);
    // At a sprint's speed the jump costs EnergyCostJumpSprint.
    let (s, _) = press_off_the_floor(GroundRules::Bf2, true, 60, 0);
    let mut walked = Soldier::new(Vec3::ZERO);
    let sprint = PadState {
        ly: FWD,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    let c = trial_class(GroundRules::Bf2);
    run(&mut walked, &sprint, &c, 61);
    let paid = walked.energy - s.energy;
    assert!((paid - c.energy_cost_jump_sprint).abs() < 1e-3, "{paid}");
}

#[test]
fn roll_predicate_matches_fun_00247b88() {
    // stick >= 0.9 required
    assert!(!Soldier::roll_predicate(0.5, 0.5));
    assert!(Soldier::roll_predicate(1.0, 0.0)); // forward
    assert!(Soldier::roll_predicate(0.0, 1.0)); // pure strafe
    assert!(!Soldier::roll_predicate(-1.0, 0.0)); // straight back
                                                  // back-left outside the +-30 deg cone: fwd=-0.5, strafe=-0.9:
                                                  // |strafe| 0.9 > 0.5*1.732 = 0.866 -> allowed
    assert!(Soldier::roll_predicate(-0.5, -0.9));
    // inside the cone: fwd=-0.9, strafe=0.3 -> blocked
    assert!(!Soldier::roll_predicate(-0.9, 0.3));
}

/// Card SPUP: stick forward + SPRINT up a slope. The port's start test is
/// taken against what the slope allows (ours; the PS2 refuses over 15.1
/// deg), so the sprint is on within 1 s at 10, 20 and 30 deg in both games'
/// rules.
#[test]
fn a_sprint_starts_while_running_uphill() {
    let c = SoldierConstants::default();
    let pad = PadState {
        ly: 32767,
        buttons: buttons::SPRINT,
        ..Default::default()
    };
    for rules in [GroundRules::Bf2, GroundRules::Bf1] {
        for deg in [0.0, 10.0, 20.0, 30.0] {
            let (n, floor) = slope(deg);
            let mut s = Soldier::new(Vec3::ZERO);
            s.yaw = std::f32::consts::PI; // +Z: up the plane
            s.grounded = true;
            s.ground_n = n;
            let at = (0..30).find(|_| {
                tick_on(&mut s, &pad, &c, rules, n, &floor);
                s.stance == Stance::Sprint
            });
            assert!(at.is_some(), "{rules:?} {deg} deg: no sprint in 1 s");
            assert!(s.pos.y >= 0.0 && s.vel.z > 0.0, "{rules:?} {deg}: uphill");
        }
    }
}

/// Card SPUP, the PS2's own numbers (BF2, one row per update, Hoth and
/// Kamino; lead_tmp/spup/ps2/out): the facing's share of a full-stick run up
/// a floor of the given angle, over MaxSpeed. It is cos^3, so the PS2's
/// `0.9 x MaxSpeed <` holds to 15.1 deg; the port's run is the same run.
#[test]
fn the_uphill_run_is_the_ps2s() {
    let c = SoldierConstants::default();
    let pad = PadState {
        ly: 32767,
        ..Default::default()
    };
    for (deg, ps2) in [
        (9.79, 0.957),
        (14.77, 0.904),
        (16.12, 0.887),
        (24.87, 0.747),
    ] {
        let (n, floor) = slope(deg);
        let mut s = Soldier::new(Vec3::ZERO);
        s.yaw = std::f32::consts::PI;
        s.grounded = true;
        s.ground_n = n;
        for _ in 0..27 {
            tick_on(&mut s, &pad, &c, GroundRules::Bf2, n, &floor);
        }
        let share = s.vel.z / c.max_speed;
        assert!((share - ps2).abs() < 0.002, "{deg} deg: {share} != {ps2}");
    }
}

/// Card SKIRT: BF2 Kamino's pod skirt, north side (re-soldier-ground.md §7):
/// a level floor that meets a band of the face's own normal (n.y 0.6665) at
/// z = -1, rising toward -Z. `(height, normal)` under a point.
fn skirt_band(ny: f32) -> (Vec3, impl Fn(Vec3) -> (f32, Vec3)) {
    let h = Vec3::new(-0.1487, 0.0, 0.7305).normalize() * (1.0 - ny * ny).sqrt();
    let n = Vec3::new(h.x, ny, h.z);
    (n, move |p: Vec3| {
        let y = -(n.x * p.x + n.z * (p.z + 1.0)) / n.y;
        if y > 0.0 {
            (y, n)
        } else {
            (0.0, Vec3::Y)
        }
    })
}

fn tick_band(
    s: &mut Soldier,
    c: &SoldierConstants,
    rules: GroundRules,
    floor: &impl Fn(Vec3) -> (f32, Vec3),
) -> Vec3 {
    let from_y = s.pos.y;
    s.step(DT, &full_fwd(), c, rules);
    let (y, n) = floor(s.pos);
    s.resolve_ground(DT, y, n, from_y, rules);
    n
}

/// Rule 1, the flattest normal (BF2 B2:156159-156166): on the tick the run
/// meets the band, N stays the level floor's, so that tick's integrator runs
/// at full hold and the next contact projects 6.29 m/s onto the face, where
/// it would be 4.09 (the PS2's `a_n172`: (-0.760, 3.408, -3.264) with N up,
/// then 6.29 on the face). One tick only.
#[test]
fn meeting_a_steeper_floor_keeps_the_flatter_normal_for_one_tick() {
    let c = trial_class(GroundRules::Bf2);
    let (n, floor) = skirt_band(0.6665);
    let mut s = Soldier::new(Vec3::new(0.0, 0.0, 3.0));
    let met = (0..60)
        .find(|_| tick_band(&mut s, &c, GroundRules::Bf2, &floor) != Vec3::Y)
        .expect("the run meets the band");
    assert!(met > 3, "a run-up first: {met}");
    assert!(
        s.grounded && s.kept_flat && s.ground_n == Vec3::Y,
        "{:?}",
        s.ground_n
    );
    assert!(
        (s.vel.length() - 4.78).abs() < 0.02,
        "projected onto the face: {}",
        s.vel
    );
    tick_band(&mut s, &c, GroundRules::Bf2, &floor);
    assert!(s.grounded && !s.kept_flat && s.ground_n == n);
    let v = s.vel.length();
    assert!(
        (v - 6.29).abs() < 0.05,
        "the full-hold tick's speed onto the face: {v}"
    );
}

/// Rule 3, the dead zone (BF1 B1:114883-114888, BF2 B2:156366-156373): after
/// a floor contact each horizontal axis under 0.1 m/s is zeroed; v.y is not.
#[test]
fn a_floor_contact_zeroes_a_horizontal_speed_under_a_tenth() {
    for rules in BOTH {
        let mut s = Soldier::new(Vec3::ZERO);
        s.vel = Vec3::new(0.05, 0.03, -0.09);
        s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
        assert!(
            s.grounded && s.vel == Vec3::new(0.0, 0.03, 0.0),
            "{rules:?}: {}",
            s.vel
        );
        let mut s = Soldier::new(Vec3::ZERO);
        s.vel = Vec3::new(-0.1, 0.0, 0.15);
        s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
        assert_eq!(s.vel, Vec3::new(-0.1, 0.0, 0.15), "{rules:?}");
    }
}

/// Rule 4, the overlap (BF2 B2:156394-156397, BF1 B1:114900-114902): at the
/// top of a climb the dead zone leaves (0, +v.y, 0), out of a sloped face; the
/// overlap a touch-down left dies away (x n.y^2 a tick, less the move off the
/// face) and on the tick after the horizontal speed falls under 0.1 the
/// floor is gone: airborne, N up. Only under the climb limit - BF2 0.7, BF1
/// 0.75, so a 0.72 face sheds a BF1 soldier and holds a BF2 one.
#[test]
fn the_floor_is_lost_at_the_top_of_a_climb_only_under_the_climb_limit() {
    let lost_at = |rules: GroundRules, ny: f32| {
        let (n, floor) = skirt_band(ny);
        let mut s = Soldier::new(Vec3::new(0.0, 0.0, -2.0));
        s.pos.y = floor(s.pos).0;
        s.ground_n = n;
        s.floor_overlap = FLOOR_CONTACT_GAP * n.y; // as a touch-down left it
        s.vel = Vec3::new(0.05, 0.12, -0.06);
        for t in 0..10 {
            let from_y = s.pos.y;
            s.pos += s.vel * DT;
            let (y, fnrm) = floor(s.pos);
            s.resolve_ground(DT, y, fnrm, from_y, rules);
            if t == 0 {
                assert!(s.grounded, "{rules:?} {ny}: held on the first tick");
                assert_eq!((s.vel.x, s.vel.z), (0.0, 0.0), "the dead zone");
            }
            if !s.grounded {
                assert_eq!(s.ground_n, Vec3::Y);
                return Some(t);
            }
        }
        None
    };
    for rules in BOTH {
        assert_eq!(
            lost_at(rules, 0.6665),
            Some(1),
            "{rules:?}: under both limits"
        );
        assert_eq!(lost_at(rules, 0.8), None, "{rules:?}: over both limits");
    }
    assert!(
        lost_at(GroundRules::Bf1, 0.72).is_some(),
        "under BF1's 0.75"
    );
    assert_eq!(lost_at(GroundRules::Bf2, 0.72), None, "over BF2's 0.7");
}

// ── Card KNOCK: the throw, the tumble, the get-up and the lock ─────────────
// PS2 reference: `lead_tmp/knock/out/bf1_hot1_knock.json` trial 2, the player
// P (re-knockback.md §2 / §6): feet (171.183, -6.920, 50.595), the thermal
// detonator's blast at (169.182, -6.800, 50.428), Push 10, PushRadius 4.5.

/// One tick on a level floor at y = 0, BF1's rules.
fn tick1(s: &mut Soldier, pad: &PadState, c: &SoldierConstants) {
    s.step(DT, pad, c, GroundRules::Bf1);
    s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf1);
}

/// P's push direction: normalize(the standing sphere centre - blast) +
/// (0, 0.6, 0).
fn p_dir() -> Vec3 {
    let d = Vec3::new(171.183, -6.92 + 1.0, 50.595) - Vec3::new(169.182, -6.8, 50.428);
    d.normalize() + Vec3::Y * KNOCK_LIFT
}

#[test]
fn knock_bf1_the_ps2_players_impulse() {
    let mut s = Soldier::new(Vec3::ZERO);
    s.knock(p_dir(), 10.0, GroundRules::Bf1, 0.0);
    let want = Vec3::new(9.128, 10.013, 0.761);
    assert!((s.vel - want).abs().max_element() < 0.02, "{:?}", s.vel);
    assert_eq!(s.stance, Stance::Thrown);
    // Under 2 m/s of lift and of ground speed: the velocity only.
    let mut s = Soldier::new(Vec3::ZERO);
    s.knock(Vec3::new(1.0, 0.0, 0.0), 1.5, GroundRules::Bf1, 0.0);
    assert_eq!((s.stance, s.vel), (Stance::Stand, Vec3::new(1.5, 0.0, 0.0)));
    // Along the ground over 2 m/s: straight to the tumble.
    s.knock(Vec3::new(1.0, 0.0, 0.0), 1.0, GroundRules::Bf1, 0.0);
    assert_eq!(s.stance, Stance::Tumble);
    // A corpse keeps its state and takes the velocity (FUN_00209758).
    let mut s = Soldier::new(Vec3::ZERO);
    s.stance = Stance::Dead;
    s.knock(p_dir(), 10.0, GroundRules::Bf1, 0.0);
    assert_eq!(s.stance, Stance::Dead);
    assert!(s.vel.y > 10.0);
}

/// §2 [M], update by update: the push's tick sets the velocity; the next
/// moves him with no gravity (the floor bit is still set); then g 13.5 with
/// the horizontal 9.16 m/s unchanged; on the ground at update 46, tumbling
/// at 47; the tumble at 8.4 m/s^2 to under 4 m/s, the get-up at 16.8 to
/// rest, then the lock (GETUP_LOCK) with no move and no fire; at rest 18.7 m
/// from the start 2.4 s after the blast.
#[test]
fn knock_bf1_the_ps2_players_flight() {
    let c = SoldierConstants::default();
    let idle = PadState::default();
    let mut s = Soldier::new(Vec3::ZERO);
    s.knock(p_dir(), 10.0, GroundRules::Bf1, 0.0);
    let (vy0, h0) = (s.vel.y, Vec2::new(s.vel.x, s.vel.z).length());
    tick1(&mut s, &idle, &c);
    assert!((s.pos.y - vy0 * DT).abs() < 1e-5, "no gravity on update 1");
    assert_eq!(s.vel.y, vy0);
    assert!(!s.grounded);
    let mut n = 1;
    while !s.grounded {
        let vy = s.vel.y;
        tick1(&mut s, &idle, &c);
        n += 1;
        if !s.grounded {
            assert!((vy - s.vel.y - 13.5 * DT).abs() < 1e-4, "g 13.5");
        }
        assert!((Vec2::new(s.vel.x, s.vel.z).length() - h0).abs() < 1e-4);
        assert!(s.no_fire());
        assert!(n < 60);
    }
    assert_eq!(n, 46, "on the ground at update 46");
    assert_eq!(s.stance, Stance::Thrown);
    tick1(&mut s, &idle, &c);
    assert_eq!(s.stance, Stance::Tumble, "tumbling at update 47");
    assert!((Vec2::new(s.vel.x, s.vel.z).length() - h0).abs() < 1e-4);
    let mut t = 47;
    let mut last = h0;
    while s.stance == Stance::Tumble {
        tick1(&mut s, &idle, &c);
        t += 1;
        let sp = Vec2::new(s.vel.x, s.vel.z).length();
        if s.stance == Stance::Tumble {
            assert!(sp >= 4.0);
        }
        last = sp;
    }
    assert_eq!(s.stance, Stance::GetUp);
    assert!(last < 4.0 && last > 3.4, "{last}");
    while s.stance == Stance::GetUp {
        tick1(&mut s, &idle, &c);
        t += 1;
    }
    assert_eq!(s.stance, Stance::Stand);
    assert!((s.lock_timer - GETUP_LOCK).abs() < 1e-6);
    let secs = t as f32 * DT;
    let dist = Vec2::new(s.pos.x, s.pos.z).length();
    assert!((secs - 2.4).abs() <= 0.1, "{secs} s");
    assert!((dist - 18.7).abs() <= 0.3, "{dist} m");
    // Locked: the stick moves nothing, the trigger is held off.
    let at = s.pos;
    for _ in 0..51 {
        tick1(&mut s, &full_fwd(), &c);
        assert!(s.no_fire());
    }
    assert!((s.pos - at).length() < 1e-4, "no move while locked");
    // It runs out on update 52, when the stand-up stops moving (ours; the
    // PS2 holds 58, 1.935 s).
    tick1(&mut s, &full_fwd(), &c);
    assert!(!s.no_fire(), "free on update 52");
    assert!(s.vel.length() > 0.5, "free after the lock");
}

/// The landing's draw (B1:117586-117607): between 3 and 6 m/s a throw
/// tumbles only when the draw is over 0.3, else it gets up.
#[test]
fn knock_landing_speed_and_draw() {
    let c = SoldierConstants::default();
    for (draw, want) in [(0.9, Stance::Tumble), (0.1, Stance::GetUp)] {
        let mut s = Soldier::new(Vec3::ZERO);
        s.knock(Vec3::new(0.78, 0.6, 0.0), 5.0, GroundRules::Bf1, draw);
        assert_eq!(s.stance, Stance::Thrown);
        while s.stance == Stance::Thrown {
            tick1(&mut s, &PadState::default(), &c);
        }
        assert_eq!(s.stance, want, "draw {draw}");
    }
}
