use super::*;

/// Issue #41: a soldier's bolt prims, standing at the origin facing -Z
/// (yaw 0): a level shot at head, chest and shin height meets the head,
/// the torso and the body; one 0.6 m to the side misses; from above, the
/// head; prone, the head lies 0.75 m ahead of the feet; a bigger
/// `CollisionRootScale` reaches further.
#[test]
fn a_soldiers_bolt_prims_are_the_ps2s() {
    let at = |posture, scale| BoltPrims::new(Vec3::ZERO, 0.0, posture, scale, None);
    let stand = at(Posture::Stand, 1.0);
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
    let prone = at(Posture::Prone, 1.0);
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
        shot(&at(Posture::Stand, 1.15), 1.9, -0.16),
        Some(HitZone::Head)
    );
    assert_eq!(shot(&stand, 1.9, -0.16), None);
}

/// Issue #182: a seated rider's prims ([`BoltPrims::posed`]) sit on his
/// drawn head and torso: a side shot at each meets that zone, the torso
/// sphere stops short of a head set close over the chest (so the head is
/// still a head hit), and the body cylinder stops at the torso centre.
#[test]
fn a_seated_riders_prims_sit_on_his_drawn_head_and_torso() {
    let head = Vec3::new(0.0, 1.825, 0.1);
    let torso = Vec3::new(0.0, 1.3, 0.0);
    let p = BoltPrims {
        posed: Some([head, torso]),
        ..BoltPrims::new(Vec3::ZERO, 0.0, Posture::Stand, 1.0, None)
    };
    // His facing (+z in his frame) is world -Z at yaw 0.
    let shot = |p: &BoltPrims, y: f32, z: f32| {
        p.hit(Vec3::new(20.0, y, z), Vec3::NEG_X, 40.0)
            .map(|(_, zone)| zone)
    };
    assert_eq!(shot(&p, 1.825, -0.1), Some(HitZone::Head));
    assert_eq!(shot(&p, 1.3, 0.0), Some(HitZone::Torso));
    assert_eq!(shot(&p, 0.3, 0.0), Some(HitZone::Body));
    assert_eq!(shot(&p, 1.65, 0.6), None, "beside him");
    // Down the spine: the head first.
    let down = p.hit(Vec3::new(0.0, 10.0, -0.1), Vec3::NEG_Y, 20.0);
    assert_eq!(down.map(|(_, z)| z), Some(HitZone::Head));
    // The torso stops 0.17 m (the head's radius) short of the head's
    // centre, 0.53 m off (a trooper's 0.4 m would reach into the head);
    // the body reaches half the torso centre's height.
    let (r, hh) = p.posed_sizes(head, torso);
    assert!((r - (head.distance(torso) - 0.17)).abs() < 1e-6, "{r}");
    assert!(r < 0.4);
    assert!((hh - 0.65).abs() < 1e-6, "{hh}");
    // Head far over the chest: the full 0.4 m.
    let (r, _) = p.posed_sizes(Vec3::new(0.0, 2.0, 0.0), torso);
    assert!((r - 0.4).abs() < 1e-6, "{r}");
    // A head 0.3 m over the chest: the torso stops 0.17 m short of it.
    let close = Vec3::new(0.0, 1.6, 0.0);
    let (r, _) = p.posed_sizes(close, torso);
    assert!((r - (0.3 - 0.17)).abs() < 1e-5, "{r}");
    let q = BoltPrims {
        posed: Some([close, torso]),
        ..p
    };
    assert_eq!(shot(&q, 1.6, 0.0), Some(HitZone::Head));
    // Over the body cylinder's top (the torso centre's height).
    assert_eq!(shot(&q, 1.35, 0.0), Some(HitZone::Torso));
}

/// `game::roster::STANCE_BODY`'s rows: BF2's clone trooper, Vader, and
/// BF1's Ewok.
fn row(p: [[f32; 3]; 5]) -> StanceBody {
    let v = |i: usize| Vec3::from_array(p[i]);
    StanceBody {
        head: [v(0), v(1), v(2)],
        torso: [v(3), v(4)],
    }
}
const TROOPER: [[f32; 3]; 5] = [
    [-0.024, 1.673, 0.131],
    [-0.066, 1.261, 0.207],
    [-0.086, 0.313, 0.699],
    [0.112, 1.252, -0.154],
    [0.092, 0.914, -0.161],
];
const VADER: [[f32; 3]; 5] = [
    [-0.037, 1.942, 0.129],
    [-0.077, 1.436, 0.215],
    [-0.098, 0.344, 0.721],
    [0.128, 1.444, -0.181],
    [0.108, 1.023, -0.199],
];
const EWOK_BF1: [[f32; 3]; 5] = [
    [-0.070, 1.021, 0.131],
    [-0.128, 0.765, 0.184],
    [-0.071, 0.221, 0.579],
    [0.108, 0.723, -0.144],
    [0.107, 0.581, -0.142],
];

/// A level shot from 20 m to his right (he faces -Z at yaw 0, so his
/// frame's z is world -z) at height `y` and his-frame depth `z`.
fn side_shot(p: &BoltPrims, y: f32, z: f32) -> Option<HitZone> {
    p.hit(Vec3::new(20.0, y, -z), Vec3::NEG_X, 40.0)
        .map(|(_, zone)| zone)
}

/// Playtest 2026-11-22 ("hit boxes in crouch or prone seemed messed up"):
/// a crouched trooper is hit on his drawn crouched head and chest, and a
/// shot at standing head height or just over his head misses him.
#[test]
fn a_crouched_soldier_is_hit_where_he_crouches() {
    let body = row(TROOPER);
    let p = BoltPrims::new(Vec3::ZERO, 0.0, Posture::Crouch, 1.0, Some(&body));
    assert_eq!(side_shot(&p, 1.261, 0.207), Some(HitZone::Head));
    assert_eq!(side_shot(&p, 0.914, -0.161), Some(HitZone::Torso));
    assert_eq!(side_shot(&p, 0.3, 0.0), Some(HitZone::Body));
    assert_eq!(side_shot(&p, 1.673, 0.131), None, "standing head height");
    assert_eq!(side_shot(&p, 1.5, 0.1), None, "over his head");
    // From above: his head first.
    let down = p.hit(Vec3::new(-0.066, 10.0, -0.207), Vec3::NEG_Y, 20.0);
    assert_eq!(down.map(|(_, z)| z), Some(HitZone::Head));
}

/// The same playtest: a prone trooper's head lies at the front end, 0.7 m
/// ahead of his feet's point and 0.31 m up; his body is a box along his
/// facing from behind his feet to his shoulders. Over his body, beyond his
/// feet or beside him a shot misses; turned, it all turns with him.
#[test]
fn a_prone_soldier_is_hit_on_his_head_and_along_his_body() {
    let body = row(TROOPER);
    let p = BoltPrims::new(Vec3::ZERO, 0.0, Posture::Prone, 1.0, Some(&body));
    assert_eq!(side_shot(&p, 0.313, 0.699), Some(HitZone::Head));
    assert_eq!(side_shot(&p, 0.2, -0.25), Some(HitZone::Body), "his hips");
    assert_eq!(side_shot(&p, 0.2, -1.0), Some(HitZone::Body), "his legs");
    assert_eq!(
        side_shot(&p, 0.2, 0.5),
        Some(HitZone::Body),
        "his shoulders"
    );
    assert_eq!(side_shot(&p, 0.2, -1.2), None, "past his feet");
    assert_eq!(side_shot(&p, 0.6, -0.25), None, "over his back");
    assert_eq!(side_shot(&p, 1.15, 0.0), None, "standing chest height");
    assert_eq!(side_shot(&p, 0.313, 0.95), None, "past his head");
    // A shot along his length from in front meets the head before the body.
    let front = p.hit(Vec3::new(-0.086, 0.313, -20.0), Vec3::Z, 40.0);
    assert_eq!(front.map(|(_, z)| z), Some(HitZone::Head));
    // From above onto his back, and onto his head.
    let back = p.hit(Vec3::new(0.0, 10.0, 0.4), Vec3::NEG_Y, 20.0);
    assert_eq!(back.map(|(_, z)| z), Some(HitZone::Body));
    let head = p.hit(Vec3::new(-0.086, 10.0, -0.699), Vec3::NEG_Y, 20.0);
    assert_eq!(head.map(|(_, z)| z), Some(HitZone::Head));
    // Facing +X, lying along X: his head at x +0.7.
    let turned = BoltPrims::new(
        Vec3::ZERO,
        -std::f32::consts::FRAC_PI_2,
        Posture::Prone,
        1.0,
        Some(&body),
    );
    let from_side = turned.hit(Vec3::new(0.699, 0.313, 20.0), Vec3::NEG_Z, 40.0);
    assert_eq!(from_side.map(|(_, z)| z), Some(HitZone::Head));
    let legs = turned.hit(Vec3::new(-1.0, 0.2, 20.0), Vec3::NEG_Z, 40.0);
    assert_eq!(legs.map(|(_, z)| z), Some(HitZone::Body));
    assert!(turned
        .hit(Vec3::new(-1.2, 0.2, 20.0), Vec3::NEG_Z, 40.0)
        .is_none());
}

/// The same playtest: a class's height is his model's. Vader (drawn head
/// 1.94 m, no `CollisionRootScale`) is hit in the head at 1.94 m, where a
/// trooper is not; an Ewok (1.02 m, no `CollisionRootScale` either) is
/// missed at 1.4 m, which met a trooper-sized Ewok before; crouched and
/// prone they keep their own sizes.
#[test]
fn a_tall_and_a_short_class_are_hit_at_their_own_heights() {
    let at = |rows: [[f32; 3]; 5], posture| {
        BoltPrims::new(Vec3::ZERO, 0.0, posture, 1.0, Some(&row(rows)))
    };
    let (vader, trooper, ewok) = (
        at(VADER, Posture::Stand),
        at(TROOPER, Posture::Stand),
        at(EWOK_BF1, Posture::Stand),
    );
    assert!((vader.scale - 1.942 / 1.673).abs() < 1e-6);
    assert_eq!(side_shot(&vader, 1.942, 0.129), Some(HitZone::Head));
    assert_eq!(side_shot(&trooper, 1.942, 0.129), None);
    assert_eq!(side_shot(&trooper, 1.673, 0.131), Some(HitZone::Head));
    assert_eq!(side_shot(&ewok, 1.021, 0.131), Some(HitZone::Head));
    assert_eq!(side_shot(&ewok, 1.4, 0.0), None, "over the Ewok");
    let old_ewok = BoltPrims::new(Vec3::ZERO, 0.0, Posture::Stand, 1.0, None);
    assert!(side_shot(&old_ewok, 1.4, 0.0).is_some());
    assert_eq!(side_shot(&ewok, 0.723, -0.144), Some(HitZone::Torso));
    // Crouched: Vader's head at 1.44 m, over the trooper's crouch.
    let (vader_c, trooper_c) = (at(VADER, Posture::Crouch), at(TROOPER, Posture::Crouch));
    assert_eq!(side_shot(&vader_c, 1.436, 0.215), Some(HitZone::Head));
    assert_eq!(side_shot(&trooper_c, 1.5, 0.215), None);
    assert_eq!(side_shot(&vader_c, 1.5, 0.215), Some(HitZone::Head));
    // Prone: the Ewok is shorter along the ground too.
    let (ewok_p, trooper_p) = (at(EWOK_BF1, Posture::Prone), at(TROOPER, Posture::Prone));
    assert_eq!(side_shot(&ewok_p, 0.221, 0.579), Some(HitZone::Head));
    assert_eq!(side_shot(&trooper_p, 0.2, -1.0), Some(HitZone::Body));
    assert_eq!(side_shot(&ewok_p, 0.2, -1.0), None, "past the Ewok's feet");
    assert_eq!(side_shot(&ewok_p, 0.3, 0.0), None, "over the Ewok's back");
    // No row: the posture lists at `CollisionRootScale`, as before.
    let listed = BoltPrims::new(Vec3::ZERO, 0.0, Posture::Prone, 1.15, None);
    assert_eq!(listed.head, Vec3::new(0.0, 0.28 * 1.15, 0.75 * 1.15));
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
