//! Lag compensation: a lagged shot meets the target where its shooter saw
//! him; no lag is the old path; a bot never has one.

use super::*;
use crate::collision::{Body, CollisionWorld, Layers, Shape};
use crate::sim::tests::{ai, flat_ground};
use crate::sim::{buttons, Event};
use crate::unit::{UnitHandle, UnitSpec};
use glam::{Mat4, Vec3};

/// Where the target strafes: 10 m in front of the shooter.
const RANGE: f32 = 10.0;

/// The shooter (slot 0) at the origin facing -Z with a 2000 m/s round (it
/// crosses the 10 m in its first tick), and a human target (slot 1) 10 m
/// ahead strafing right. Aim assist off: the aim is the test's.
fn range() -> (Sim, UnitHandle, UnitHandle) {
    let mut s = Sim::empty(3);
    s.heightfield = Some(flat_ground());
    s.assist_armed = false;
    s.assist_intensity = 0.0;
    s.weapon_classes[0].ordnance.velocity = 2000.0;
    let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let target = s
        .spawn(&UnitSpec::player_in_slot(
            1,
            1,
            Vec3::new(-2.0, 0.0, -RANGE),
        ))
        .unwrap();
    (s, shooter, target)
}

fn strafe() -> PadState {
    PadState {
        lx: i16::MAX,
        ..Default::default()
    }
}

/// Turn the shooter until his round leaves for `at` (his shot converges
/// on what his aim ray meets, so the aim is solved, not set).
fn aim_at(s: &mut Sim, shooter: UnitHandle, at: Vec3) {
    let i = shooter.index as usize;
    for _ in 0..40 {
        let (f, (fwd, _, _)) = s.human_shot(i);
        let want = (at - f).normalize();
        let angles = |d: Vec3| (crate::fmath::atan2(-d.x, -d.z), crate::fmath::asin(d.y));
        let ((wy, wp), (fy, fp)) = (angles(want), angles(fwd));
        let so = &mut s.units[i].soldier;
        so.yaw += wy - fy;
        so.pitch += wp - fp;
    }
    let (f, (fwd, _, _)) = s.human_shot(i);
    let miss = (at - f).cross(fwd).length();
    assert!(miss < 0.01, "aim solved to {miss} m");
}

/// Strafe the target for 40 ticks keeping his chest point per tick, aim at
/// where he was 3 ticks ago, fire with `view_lag`; did the shot hit him?
/// With `wall`, a wall goes up between the shooter and where the target
/// runs to, clear of where he was.
fn lagged_shot(view_lag: u8, wall: bool) -> (bool, Sim, Vec3) {
    let (mut s, shooter, target) = range();
    let mut seen = Vec::new();
    for _ in 0..40 {
        s.step(&[PadState::default(), strafe()]);
        seen.push(s.unit(target).unwrap().aim_point());
    }
    let then = seen[seen.len() - 4];
    let now = s.unit(target).unwrap().aim_point();
    // He runs: 3 ticks are well past a torso's half width.
    assert!(
        (now - then).length() > 0.5,
        "moved {}",
        (now - then).length()
    );
    if wall {
        // 1.5 m in front of him, from 0.2 m right of the line to where he
        // was (at that depth) to 3 m on.
        let z = then.z + 1.5;
        let left = then.x * z / then.z + 0.2;
        let body = Body::new(
            Mat4::from_translation(Vec3::new(left + 1.5, 1.5, z)),
            Shape::Box {
                half: Vec3::new(1.5, 1.5, 0.25),
            },
        );
        s.collision = CollisionWorld::new(vec![body], Vec::new());
    }
    aim_at(&mut s, shooter, then);
    let fire = PadState {
        buttons: buttons::FIRE,
        view_lag,
        ..Default::default()
    };
    let mut hit = false;
    for k in 0..4 {
        let pads = [if k == 0 { fire } else { PadState::default() }, strafe()];
        s.step(&pads);
        hit |= s
            .events
            .iter()
            .any(|e| matches!(e, Event::Hit { victim, .. } if *victim == target));
    }
    (hit, s, now)
}

#[test]
fn a_lagged_shot_hits_the_strafing_target_where_its_shooter_saw_him() {
    let (hit, s, _) = lagged_shot(3, false);
    assert!(hit, "view_lag 3 meets him where he was 3 ticks ago");
    assert_eq!(s.hit_history.len(), usize::from(VIEW_LAG_CAP));
    // The same shot with no lag is the old path: he has moved on.
    let (hit, _, _) = lagged_shot(0, false);
    assert!(!hit, "view_lag 0 misses");
}

#[test]
fn a_lagged_shot_still_hits_a_target_that_has_stepped_behind_a_wall() {
    // The documented trade-off: inside the window a target can be hit just
    // after he reached cover.
    let (hit, s, now) = lagged_shot(3, true);
    let eye = s.units[0].soldier.pos + Vec3::Y * 1.6;
    let to = now - eye;
    let blocked = s
        .collision
        .ray_with(
            eye,
            to.normalize(),
            to.length(),
            Layers::ORDNANCE,
            crate::collision::HullFilter::All,
        )
        .is_some();
    assert!(blocked, "the wall stands between the shooter and him now");
    assert!(hit, "but he was in the open when the shooter saw him");
}

#[test]
fn a_bot_never_has_a_view_lag_and_a_lag_is_capped() {
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    let me = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let bot = s.spawn(&ai(1, Vec3::new(0.0, 0.0, -30.0), 0.0)).unwrap();
    let pad = |view_lag| PadState {
        view_lag,
        ..Default::default()
    };
    s.step(&[pad(3)]);
    assert_eq!((s.view_lag_of(me.index), s.view_lag_of(bot.index)), (3, 0));
    s.step(&[pad(200)]);
    assert_eq!(s.view_lag_of(me.index), VIEW_LAG_CAP);
    // The bot's own pads (its thinks, and what a prediction replays) carry
    // none.
    assert!(s.ai_pads.iter().all(|p| p.view_lag == 0));
    s.step_predicted(&[pad(5)]);
    assert_eq!((s.view_lag_of(me.index), s.view_lag_of(bot.index)), (5, 0));
}

#[test]
fn no_view_lag_keeps_the_hash_of_the_old_path() {
    // `view_lag` 0 is the default pad: the pinned hashes (`state/tests.rs`,
    // `trace/tests.rs`) run through it. Here: a lag on a pad that fires
    // nothing changes nothing either, since only a shot reads it.
    let run = |view_lag| {
        let (mut s, _, _) = range();
        for _ in 0..30 {
            s.step(&[
                PadState {
                    view_lag,
                    ..Default::default()
                },
                strafe(),
            ]);
        }
        s.hash()
    };
    assert_eq!(run(0), run(6));
}

#[test]
fn a_lagged_shot_meets_a_moving_hull_where_it_stood() {
    use crate::collision::HullPart;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let shoot = |view_lag| {
        let mut s = Sim::empty(6);
        s.heightfield = Some(flat_ground());
        s.assist_armed = false;
        s.assist_intensity = 0.0;
        s.weapon_classes[0].ordnance.velocity = 2000.0;
        // A 2 m wide box hull on a vehicle that slides 0.6 m a tick
        // across the shooter's line (moved between steps: its class has no
        // drive, so the step leaves it where it is put).
        s.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
            Mat4::IDENTITY,
            Shape::Box {
                half: Vec3::new(1.0, 1.0, 1.5),
            },
            Layers::ALL,
        )]))];
        let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        let v = s.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(-12.0, 1.5, -15.0),
            yaw: 0.0,
            max_health: 4000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let i = v.index as usize;
        let mut seen = Vec::new();
        for _ in 0..20 {
            s.vehicles[i].pos.x += 0.6;
            s.step(&[PadState::default()]);
            seen.push(s.vehicles[i].pos);
        }
        // Where it stood 3 ticks back; now it is 1.8 m on, and the next
        // step takes it 0.6 m more: past its own half width.
        let then = seen[seen.len() - 4];
        aim_at(&mut s, shooter, then);
        s.vehicles[i].pos.x += 0.6;
        s.step(&[PadState {
            buttons: buttons::FIRE,
            view_lag,
            ..Default::default()
        }]);
        // (A rifle round does this hull no damage; the hit is the event.)
        s.events
            .iter()
            .any(|e| matches!(e, Event::VehicleHit { vehicle, .. } if *vehicle == v))
    };
    assert!(shoot(3), "view_lag 3 meets the hull where it stood");
    assert!(!shoot(0), "view_lag 0 misses it");
}

/// [`lagged_shot`] with the range's weapon reshaped by `shape` (no wall):
/// whether the target was hit, every event of the 4 ticks from the press,
/// and where he was 3 ticks before it.
fn lagged_round(
    view_lag: u8,
    shape: impl Fn(&mut crate::weapon::OrdnanceClass),
) -> (bool, Vec<Event>, Vec3) {
    let (mut s, shooter, target) = range();
    shape(&mut s.weapon_classes[0].ordnance);
    let mut seen = Vec::new();
    for _ in 0..40 {
        s.step(&[PadState::default(), strafe()]);
        seen.push(s.unit(target).unwrap().aim_point());
    }
    let then = seen[seen.len() - 4];
    aim_at(&mut s, shooter, then);
    let fire = PadState {
        buttons: buttons::FIRE,
        view_lag,
        ..Default::default()
    };
    let mut events = Vec::new();
    for k in 0..4 {
        let pads = [if k == 0 { fire } else { PadState::default() }, strafe()];
        s.step(&pads);
        events.extend(s.events.iter().cloned());
    }
    let hit = events
        .iter()
        .any(|e| matches!(e, Event::Hit { victim, .. } if *victim == target));
    (hit, events, then)
}

/// An unguided rocket (a `missile` round fired with no lock) at 300 m/s.
fn rocket(o: &mut crate::weapon::OrdnanceClass) {
    o.kind = crate::weapon::OrdKind::Missile;
    o.velocity = 300.0;
    o.life_span = 2.0;
    o.missile = Some(crate::missile::MissileClass::default());
}

/// A shell at 300 m/s with a blast: it reaches the 10 m line in its first
/// tick and drops about 1 cm on the way.
fn shell(o: &mut crate::weapon::OrdnanceClass) {
    o.kind = crate::weapon::OrdKind::Shell;
    o.gravity = 1.75;
    o.velocity = 300.0;
    o.life_span = 2.0;
    o.explosion = Some(crate::weapon::ExplosionClass {
        damage: 50.0,
        damage_inner: 1.0,
        damage_outer: 3.0,
        ..Default::default()
    });
}

#[test]
fn a_lagged_rocket_hits_the_strafing_target_where_its_shooter_saw_him() {
    let (hit, events, _) = lagged_round(3, rocket);
    assert!(hit, "view_lag 3 meets him where he was: {events:?}");
    let (hit, _, _) = lagged_round(0, rocket);
    assert!(!hit, "view_lag 0 misses: he has moved on");
}

#[test]
fn a_lagged_shells_blast_goes_off_at_the_rewound_impact() {
    let impacts = |events: &[Event]| -> Vec<Vec3> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Impact { pos, .. } => Some(*pos),
                _ => None,
            })
            .collect()
    };
    let (hit, events, then) = lagged_round(3, shell);
    assert!(hit, "the shell meets him where the shooter saw him");
    let at = impacts(&events);
    assert_eq!(at.len(), 1, "{at:?}");
    // The impact (and the blast's centre, its end) is on his body where he
    // stood 3 ticks back, not where he is now.
    let off = Vec3::new(at[0].x - then.x, 0.0, at[0].z - then.z).length();
    assert!(off < 0.6, "the impact is {off} m from where he was");
    let (hit, events, _) = lagged_round(0, shell);
    assert!(!hit, "view_lag 0 misses");
    assert!(
        impacts(&events)
            .iter()
            .all(|p| Vec3::new(p.x - then.x, 0.0, p.z - then.z).length() > 1.0),
        "and bursts nowhere near where he was"
    );
}

#[test]
fn a_bots_shell_is_never_fast_forwarded() {
    use crate::projectile::{Bolt, Shooter};
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    shell(&mut s.weapon_classes[0].ordnance);
    let me = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let bot = s.spawn(&ai(1, Vec3::new(30.0, 0.0, -30.0), 0.0)).unwrap();
    for _ in 0..10 {
        s.step(&[PadState::default()]);
    }
    let round = |s: &Sim, owner: UnitHandle| {
        Bolt::fire(
            s.unit(owner).unwrap().soldier.pos + Vec3::Y,
            Vec3::new(0.0, 0.3, -1.0).normalize(),
            100.0,
            2.0,
            Shooter {
                owner,
                team: s.unit(owner).unwrap().team,
                weapon_class: 0,
                is_player: owner == me,
            },
            s.heightfield.as_ref(),
            s.water_plane(),
            &s.collision,
            crate::collision::HullFilter::All,
        )
        .shaped(&s.weapon_classes[0].ordnance)
    };
    // Both pads say 5 ticks of lag; only the player's counts.
    let lagged = PadState {
        view_lag: 5,
        ..Default::default()
    };
    s.take_view_lags(&[lagged, lagged]);
    let (mine, theirs) = (round(&s, me), round(&s, bot));
    let first = s.bolts.len();
    s.bolts.extend([mine.clone(), theirs.clone()]);
    s.compensate_new_bolts(first, crate::TICK_DT);
    let (m, t) = (&s.bolts[first], &s.bolts[first + 1]);
    assert!(
        m.age > mine.age && m.pos != mine.pos,
        "the player's shell flew its lag"
    );
    assert!(m.vel.y < mine.vel.y, "and fell on the way");
    assert_eq!(
        (t.pos, t.vel, t.age),
        (theirs.pos, theirs.vel, theirs.age),
        "the bot's did not"
    );
}

#[test]
fn a_lagged_homing_missile_keeps_its_lock_path() {
    use crate::missile::{Missile, MissileTarget};
    use crate::projectile::{Bolt, Shooter};
    let mut s = Sim::empty(4);
    s.heightfield = Some(flat_ground());
    rocket(&mut s.weapon_classes[0].ordnance);
    let me = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let them = s
        .spawn(&UnitSpec::player_in_slot(1, 1, Vec3::new(0.0, 0.0, -40.0)))
        .unwrap();
    for _ in 0..10 {
        s.step(&[PadState::default(), PadState::default()]);
    }
    let mut b = Bolt::fire(
        Vec3::Y,
        -Vec3::Z,
        300.0,
        2.0,
        Shooter {
            owner: me,
            team: 0,
            weapon_class: 0,
            is_player: true,
        },
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        crate::collision::HullFilter::All,
    );
    let mut lcg = crate::lcg::GameLcg::new(1);
    b.missile = Some(Missile::launch(
        &crate::missile::MissileClass::default(),
        -Vec3::Z,
        300.0,
        Some(MissileTarget::Unit(them)),
        &mut lcg,
    ));
    s.take_view_lags(&[
        PadState {
            view_lag: 5,
            ..Default::default()
        },
        PadState::default(),
    ]);
    let first = s.bolts.len();
    s.bolts.push(b.clone());
    s.compensate_new_bolts(first, crate::TICK_DT);
    assert_eq!(s.bolts[first].pos, b.pos, "a locked round is not rewound");
}

#[test]
fn a_lagged_tracer_moves_its_lag_on_with_its_round() {
    // A tracer (no damage, no blast) strikes nothing; a lagged shot's
    // tracer moves the lag on like its round, so it is drawn beside it.
    use crate::projectile::{Bolt, Shooter};
    let (mut s, shooter, _) = range();
    s.weapon_classes[0].ordnance.max_damage = 0.0;
    for _ in 0..10 {
        s.step(&[PadState::default(), strafe()]);
    }
    let b = Bolt::fire(
        Vec3::Y,
        -Vec3::X,
        100.0,
        1.0,
        Shooter {
            owner: shooter,
            team: 0,
            weapon_class: 0,
            is_player: true,
        },
        s.heightfield.as_ref(),
        s.water_plane(),
        &s.collision,
        crate::collision::HullFilter::All,
    );
    s.take_view_lags(&[
        PadState {
            view_lag: 3,
            ..Default::default()
        },
        PadState::default(),
    ]);
    let first = s.bolts.len();
    s.bolts.push(b.clone());
    s.compensate_new_bolts(first, crate::TICK_DT);
    let moved = s.bolts[first].pos - b.pos;
    assert!(
        (moved - b.vel * 3.0 * crate::TICK_DT).length() < 1e-3,
        "{moved}"
    );
    assert_eq!(s.bolts[first].hit, b.hit, "it struck nothing on the way");
}

#[test]
fn a_lagged_fat_ray_passes_through_where_its_shooter_saw_him() {
    // A fat ray tests each body where it is, without a bolt's window lag
    // (`sweep_fat_rays`), so its rewound pass meets him a tick on from
    // where the aim was solved: the radius covers that tick, not the three
    // he has run since.
    let fat = |o: &mut crate::weapon::OrdnanceClass| {
        o.velocity = 300.0;
        o.life_span = 0.25;
        o.fat_ray = Some(crate::weapon::FatRayClass {
            max_damage: 100.0,
            min_damage: 10.0,
            damage_deduction: 5.0,
            radius: 0.8,
        });
    };
    let (hit, events, _) = lagged_round(3, fat);
    assert!(
        hit,
        "view_lag 3 passes through him where he was: {events:?}"
    );
    let (hit, _, _) = lagged_round(0, fat);
    assert!(!hit, "view_lag 0 passes where he no longer is");
}
