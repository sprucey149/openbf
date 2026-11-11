use super::*;
use crate::ai::brain::intent::compose_goal;
use crate::ai::state::HoldState;
use crate::sim::Station;

/// The pre-carve goal, kind and arrival matches in their original order.
fn old_chain(
    c: &Cands<'_>,
    pos: Vec3,
    route: Option<(bool, Vec3)>,
    cmd: bool,
    take: bool,
) -> (Goal, StepKind, u8, Option<(Vec3, f32)>, Option<Vec3>) {
    let goal = match c.boarding {
        Some(_) if route.is_some_and(|(last, _)| !last) => c.base_goal,
        _ => c.boarding.or(c.base_goal),
    };
    let goal = match c.ring {
        Some(p) if route.is_none_or(|(last, _)| last) => Some(p),
        _ => goal,
    };
    let at_ring = c.ring.filter(|p| goal == Some(*p));
    let goal = match c.spot {
        Some(p) if route.is_none_or(|(last, _)| last) => Some(p),
        _ => goal,
    };
    let at_spot = c.spot.filter(|p| goal == Some(*p));
    let goal = match c.fixing {
        Some(p) if route.is_none_or(|(last, _)| last) => Some(p),
        _ => goal,
    };
    let at_fix = c.fixing.filter(|p| goal == Some(*p));
    let goal = match c.mend {
        Some(s) => (!s.reaches(pos)).then_some(s.pos),
        None => goal,
    };
    let kind = match () {
        _ if c.ret.is_some() => 1,
        _ if c.flee.is_some() => 2,
        _ if c.item.is_some() => 6,
        _ if c.holding.is_some() => 3,
        _ if c.boarding.is_some() && goal == c.boarding => 4,
        _ if c.mend.is_some() => 7,
        _ => 5,
    };
    let (owner, arrive) = match kind {
        1 => (StepKind::Return, None),
        2 => (StepKind::Flee, None),
        3 => (StepKind::Hold, c.holding.map(|h| (h.goal, HOLD_ARRIVE))),
        4 => (StepKind::Board, None),
        6 => (StepKind::Pickup, c.item.map(|p| (p, 0.2))),
        7 => (StepKind::Mend, None),
        5 if c.fixing.is_some() => (StepKind::Fix, at_fix.map(|p| (p, HOLD_ARRIVE))),
        5 if c.ring.is_some() && take => (StepKind::RingTake, at_ring.map(|p| (p, HOLD_ARRIVE))),
        5 if c.ring.is_some() => (StepKind::Ring, at_ring.map(|p| (p, RING_ARRIVE))),
        5 if c.spot.is_some() => (StepKind::Spot, at_spot.map(|p| (p, RING_ARRIVE))),
        5 if c.push.is_some() && goal == c.push => {
            (StepKind::Push, c.push.map(|p| (p, CORNER_ARRIVE)))
        }
        5 => (
            StepKind::Leg,
            match (route, cmd) {
                (_, true) => None,
                (Some((last, waypoint)), false) => last.then_some((waypoint, CORNER_ARRIVE)),
                (None, false) => goal.map(|g| (g, CORNER_ARRIVE)),
            },
        ),
        _ => unreachable!(),
    };
    let immediate = c
        .ret
        .or(c.flee)
        .or(c.item)
        .or(c.holding.map(|h| h.goal))
        .or(goal);
    (
        Goal {
            approach: goal,
            at_ring,
            at_spot,
            at_fix,
        },
        owner,
        kind,
        arrive,
        immediate,
    )
}

#[test]
fn scores_match_the_chain() {
    let pos = Vec3::ZERO;
    let shared = Vec3::new(1.0, 2.0, 3.0);
    let waypoint = Vec3::Z * 20.0;
    let approaching = Station::new(0, Vec3::Z * 30.0, 1.0);
    let in_reach = Station::new(0, pos, 1.0);
    for present in 0_u16..(1 << 9) {
        for coincide in 0_u16..(1 << 5) {
            for route in [None, Some((true, waypoint)), Some((false, waypoint))] {
                for flags in 0..8 {
                    let cmd = flags & 1 != 0;
                    let take = flags & 2 != 0;
                    let base_goal = (flags & 4 != 0).then_some(shared);
                    for mend in [None, Some(&approaching), Some(&in_reach)] {
                        // Distinct points have the same XZ and different Y.
                        let point = |bit: u16| {
                            (present & (1 << bit) != 0).then(|| {
                                if coincide & (1 << bit) != 0 {
                                    shared
                                } else {
                                    shared + Vec3::Y * f32::from(bit + 1)
                                }
                            })
                        };
                        let cands = Cands {
                            last_leg: route.is_none_or(|(last, _)| last),
                            base_goal,
                            boarding: point(0),
                            ring: point(1),
                            spot: point(2),
                            fixing: point(3),
                            push: point(4),
                            ret: point(5),
                            flee: point(6),
                            item: point(7),
                            holding: point(8).map(|goal| HoldState {
                                goal,
                                ..HoldState::default()
                            }),
                            mend,
                            ..Cands::default()
                        };
                        let goal = compose_goal(&cands, pos);
                        let step = select(&cands, goal, take);
                        let arrive = step.arrive(
                            &cands,
                            goal,
                            route.and_then(|(last, p)| last.then_some(p)),
                            route.is_none(),
                            cmd,
                        );
                        assert_eq!(
                            (goal, step, step.move_kind() as u8, arrive, goal.immediate(&cands)),
                            old_chain(&cands, pos, route, cmd, take),
                            "present {present:09b}, coincide {coincide:05b}, route {route:?}, flags {flags}",
                        );
                        assert_eq!(step.after_aim(None), step);
                        assert_eq!(step.after_aim(Some(pos)), StepKind::Melee);
                        assert_eq!(StepKind::Melee.move_kind() as u8, 5);
                    }
                }
            }
        }
    }
}
