use super::*;

/// Issue #41: a soldier's bolt prims, standing at the origin facing -Z
/// (yaw 0): a level shot at head, chest and shin height meets the head,
/// the torso and the body; one 0.6 m to the side misses; from above, the
/// head; prone, the head lies 0.75 m ahead of the feet; a bigger
/// `CollisionRootScale` reaches further.
#[test]
fn a_soldiers_bolt_prims_are_the_ps2s() {
    let at = |stance, scale| BoltPrims {
        pos: Vec3::ZERO,
        yaw: 0.0,
        stance,
        scale,
    };
    let stand = at(Stance::Stand, 1.0);
    // From 20 m to his right, level, toward him.
    let shot = |p: &BoltPrims, y: f32, z: f32| {
        p.hit(Vec3::new(20.0, y, z), Vec3::NEG_X, 40.0)
            .map(|(_, zone)| zone)
    };
    assert_eq!(shot(&stand, 1.65, -0.16), Some(HitZone::Head));
    assert_eq!(shot(&stand, 1.15, 0.0), Some(HitZone::Torso));
    assert_eq!(shot(&stand, 0.3, 0.0), Some(HitZone::Body));
    assert_eq!(shot(&stand, 1.0, 0.6), None, "beside him");
    assert_eq!(shot(&stand, 2.0, -0.16), None, "over his head");
    // The near face: the head's sphere 0.17 m from its centre.
    let (t, _) = stand
        .hit(Vec3::new(20.0, 1.65, -0.16), Vec3::NEG_X, 40.0)
        .unwrap();
    assert!((t - (20.0 - 0.17)).abs() < 1e-4, "{t}");
    // From straight above: the head first.
    let down = stand.hit(Vec3::new(0.0, 10.0, -0.16), Vec3::NEG_Y, 20.0);
    assert_eq!(down.map(|(_, z)| z), Some(HitZone::Head));
    // Prone: the head 0.28 m up and 0.75 m ahead (-Z); the body behind.
    let prone = at(Stance::Prone, 1.0);
    assert_eq!(shot(&prone, 0.28, -0.75), Some(HitZone::Head));
    assert_eq!(shot(&prone, 0.2, 0.2), Some(HitZone::Body));
    assert_eq!(shot(&prone, 1.0, 0.0), None, "over a prone soldier");
    // Turned to face +X, the prone head lies at +X.
    let turned = BoltPrims {
        yaw: -std::f32::consts::FRAC_PI_2,
        ..prone
    };
    let from_side = turned.hit(Vec3::new(0.75, 0.28, 20.0), Vec3::NEG_Z, 40.0);
    assert_eq!(from_side.map(|(_, z)| z), Some(HitZone::Head));
    // The wookiee's 1.15: his head at 1.9 m.
    assert_eq!(
        shot(&at(Stance::Stand, 1.15), 1.9, -0.16),
        Some(HitZone::Head)
    );
    assert_eq!(shot(&stand, 1.9, -0.16), None);
}

const DT: f32 = AimBlock::PERIOD as f32 * crate::TICK_DT;
/// Skill 17's decay (m/s) and floor (m), a soldier's MaxSpeed.
const DECAY: f32 = 0.942_105_3;
const FLOOR: f32 = 0.589_473_6;
const TOP: f32 = 6.0;

fn think(b: &mut AimBlock, vel: Vec3) {
    b.think(DT, vel, TOP, DECAY, FLOOR, Vec3::ZERO);
}

/// Card CLSE2 (`analysis/re-soldier-ai.md` 18.1): a unit's error decays
/// to the floor of who shoots at it and its budget fills at HALF the
/// decay, at most 5.
#[test]
fn a_units_error_decays_to_the_floor_and_saves_a_budget() {
    let mut b = AimBlock::default();
    think(&mut b, Vec3::ZERO);
    assert_eq!(
        b.err, FLOOR,
        "from the constructor's 0 straight to the floor"
    );
    assert!((b.budget - 0.5 * DT * DECAY).abs() < 1e-6);
    b.err = 2.0;
    for _ in 0..6 {
        think(&mut b, Vec3::ZERO);
    }
    assert!(
        (b.err - (2.0 - DECAY)).abs() < 1e-4,
        "a second's decay: {}",
        b.err
    );
    assert!((b.budget - 0.5 * 7.0 * DT * DECAY).abs() < 1e-5);
    for _ in 0..600 {
        think(&mut b, Vec3::ZERO);
    }
    assert_eq!((b.err, b.budget), (FLOOR, AimBlock::BUDGET_MAX));
}

/// The error grows with the unit's CHANGE of velocity between two thinks
/// - a start 0.455, a reversal at full speed 0.91, a straight run nothing
/// - and only as far as the budget pays.
#[test]
fn a_units_error_grows_with_its_change_of_velocity_out_of_the_budget() {
    let run = Vec3::new(TOP, 0.0, 0.0);
    let rich = || AimBlock {
        err: FLOOR,
        budget: 3.0,
        ..AimBlock::default()
    };
    // A start from rest: + 0.455, less the think's own decay.
    let mut b = rich();
    think(&mut b, run);
    assert!((b.err - (FLOOR + 0.455)).abs() < 1e-5, "{}", b.err);
    assert!((b.budget - (3.0 + 0.5 * DT * DECAY - 0.455)).abs() < 1e-5);
    // Running straight on adds nothing: it decays.
    let was = b.err;
    think(&mut b, run);
    assert!((b.err - (was - DT * DECAY)).abs() < 1e-5, "{}", b.err);
    // A reversal at full speed: + 0.91. Height counts (3-D).
    let was = b.err;
    think(&mut b, -run);
    assert!(
        (b.err - (was - DT * DECAY + 0.91)).abs() < 1e-5,
        "{}",
        b.err
    );
    let mut up = rich();
    think(&mut up, Vec3::new(0.0, TOP, 0.0));
    assert!((up.err - (FLOOR + 0.455)).abs() < 1e-5);
    // Never over 2 m.
    for k in 0..40 {
        b.budget = 3.0;
        think(&mut b, if k % 2 == 0 { run } else { -run });
    }
    assert_eq!(b.err, crate::ai::ERR_MAX);
    // An empty budget pays only what the think itself saved.
    let mut poor = AimBlock {
        err: FLOOR,
        ..AimBlock::default()
    };
    think(&mut poor, run);
    assert!(
        (poor.err - (FLOOR + 0.5 * DT * DECAY)).abs() < 1e-5,
        "{}",
        poor.err
    );
    assert_eq!(poor.budget, 0.0);
}

/// The direction tumbles: a unit vector that turns 2 x dt radians a
/// think about a walking axis - it is not drawn anew.
#[test]
fn a_units_error_direction_tumbles() {
    let angle = |a: Vec3, b: Vec3| fmath::atan2(a.cross(b).length(), a.dot(b));
    let mut b = AimBlock::default();
    assert_eq!(b.dir, Vec3::ZERO);
    let mut last = Vec3::Y;
    let (mut most, mut least, mut far) = (0.0f32, 9.0f32, 0.0f32);
    for k in 0..600u64 {
        let draw = |s: u32| 2.0 * crate::ai::unit_unit(3, k, 1, s) - 1.0;
        b.think(
            DT,
            Vec3::ZERO,
            TOP,
            DECAY,
            FLOOR,
            Vec3::new(draw(1), draw(2), draw(3)),
        );
        assert!((b.dir.length() - 1.0).abs() < 1e-4, "unit length at {k}");
        assert!((b.axis.length() - 1.0).abs() < 1e-4);
        let turned = angle(b.dir, last);
        most = most.max(turned);
        least = least.min(turned);
        far = far.max(angle(b.dir, Vec3::Y));
        last = b.dir;
    }
    assert!(most <= 2.0 * DT + 1e-3, "at most 2 x dt a think: {most}");
    assert!(
        most > 1.5 * DT,
        "and that much about an axis across it: {most}"
    );
    assert!(least < DT, "less about an axis along it: {least}");
    assert!(
        far > 2.0,
        "over a minute and a half it goes all the way round: {far}"
    );
}

/// The legacy twelve fields each move the block hash. Current `dir` is
/// derived from `quat`; bot-only history is tested separately below.
#[test]
fn the_aim_block_is_hashed_field_by_field() {
    let hash = |b: &AimBlock| {
        let mut out = Vec::new();
        b.hash_into(&mut |x: &[u8]| out.extend_from_slice(x));
        out
    };
    let base = AimBlock::default();
    let edits: [fn(&mut AimBlock); 12] = [
        |b| b.err = 1.0,
        |b| b.budget = 1.0,
        |b| b.vel.x = 1.0,
        |b| b.vel.y = 1.0,
        |b| b.vel.z = 1.0,
        |b| b.axis.x = 1.0,
        |b| b.axis.y = 1.0,
        |b| b.axis.z = 1.0,
        |b| b.quat.x = 1.0,
        |b| b.quat.y = 1.0,
        |b| b.quat.z = 1.0,
        |b| b.quat.w = 2.0,
    ];
    let mut seen = vec![hash(&base)];
    for e in edits {
        let mut b = base;
        e(&mut b);
        let h = hash(&b);
        assert!(!seen.contains(&h));
        seen.push(h);
    }
}
mod audit;
