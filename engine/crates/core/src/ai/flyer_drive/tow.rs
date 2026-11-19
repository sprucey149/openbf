//! Openbf tow crew coordination. measure: a stock Hoth bot pilot's approach,
//! turn direction and altitude with a human in the rear seat. The normal
//! flight integrator and the authored cable contacts resolve the maneuver.

use super::{clear_chord, ground, stick_to, Body, FlyerAi, FlyerParams, Sim, Vec3, Vehicle};
use crate::collision::HullFilter;
use crate::fmath::FloatExt;
use crate::unit::UnitHandle;
use crate::vehicle::{flyer::TowRun, VehicleHandle};

/// A crew looks for an exposed walker within this many cable lengths.
const SEEK_LENGTHS: f32 = 4.0;
/// Stand off far enough for the rear arc while leaving room in the cable.
const TURN_RADIUS_SHARE: f32 = 0.42;
const TURN_LOOK_AHEAD: f32 = 0.6;
const PASS_HEIGHT_SHARE: f32 = 0.16;
/// The inbound arc and outbound pass leave the target inside the gun's rear arc.
const APPROACH_RADIUS_SHARE: f32 = 0.31;
const RETRY_RADIUS_SHARE: f32 = 0.70;

pub(super) fn pilot(
    sim: &Sim,
    me: UnitHandle,
    craft: &Vehicle,
    source: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
) -> Option<[f32; 4]> {
    let planned = plan(sim, me, craft, source, p, ai);
    if planned.is_none() {
        ai.tow_run = None;
    }
    planned
}

fn plan(
    sim: &Sim,
    me: UnitHandle,
    craft: &Vehicle,
    source: VehicleHandle,
    p: &FlyerParams,
    ai: &mut FlyerAi,
) -> Option<[f32; 4]> {
    let frames = sim.tow_frames.as_ref()?;
    let crew = craft
        .seats
        .iter()
        .filter_map(|s| s.occupant)
        .filter(|&u| u != me);
    let cable = crew
        .filter_map(|u| sim.seat_look(u))
        .flat_map(|sight| sight.gun.weapons)
        .find_map(|w| sim.weapon_classes.get(w.class)?.ordnance.tow)?;
    let team = sim.unit(me)?.team;
    let latched = sim.live_tow_target(source);
    let available = |(target, walker): (VehicleHandle, &Vehicle)| {
        let parts = frames.primitives(walker);
        let own = HullFilter::Except([Some(source), Some(target)]);
        parts.iter().any(|part| {
            part.leg.is_some()
                && crate::perception::los_past(sim, craft.pos, part.frame.w_axis.truncate(), own)
        })
    };
    let target = latched
        .or_else(|| {
            ai.tow_run
                .and_then(|run| sim.vehicle(run.target).map(|v| (run.target, v)))
                .filter(|&(target, v)| {
                    v.health.alive()
                        && v.team.or(v.home_team).is_some_and(|t| sim.hostile(team, t))
                        && v.pos.distance(craft.pos) <= cable.initial_length * SEEK_LENGTHS
                        && available((target, v))
                })
                .map(|(target, _)| target)
        })
        .or_else(|| {
            sim.tow_walkers(team, craft.pos, cable.initial_length * SEEK_LENGTHS)
                .filter(|&(target, v)| available((target, v)))
                .min_by(|(_, a), (_, b)| {
                    a.pos
                        .distance_squared(craft.pos)
                        .total_cmp(&b.pos.distance_squared(craft.pos))
                })
                .map(|(target, _)| target)
        })?;
    let walker = sim.vehicle(target)?;
    let forward = craft.rot * Vec3::Z;
    let offset = (craft.pos - walker.pos).with_y(0.0);
    let distance = offset.length();
    let radial = offset.try_normalize()?;
    let tangent = Vec3::Y.cross(radial);
    let previous = ai.tow_run.filter(|run| run.target == target);
    let clockwise = previous.map_or_else(|| forward.dot(tangent) >= 0.0, |run| run.clockwise);
    let tangent = if clockwise { tangent } else { -tangent };
    let length = cable.initial_length.fmin(cable.length);
    let radius = length * TURN_RADIUS_SHARE;
    let approach = length * APPROACH_RADIUS_SHARE;
    let retry = length * RETRY_RADIUS_SHARE;
    let opening_shot = latched.is_none()
        && if previous.is_some_and(|run| run.opening_shot) {
            distance < retry
        } else {
            distance < radius
        };
    let (sin, cos) = crate::fmath::sin_cos(TURN_LOOK_AHEAD);
    let goal = if opening_shot {
        // The BF2 rear mount reaches 115–245 degrees. Turning outward
        // gives it a shot before the pilot commits to the tangential wrap.
        craft.pos + (radial * cos + tangent * sin) * radius
    } else {
        walker.pos
            + (radial * cos + tangent * sin) * if latched.is_some() { radius } else { approach }
    };
    let [floor, ceiling] = sim.flight_limits.ai;
    let over_ground = cable.initial_length * PASS_HEIGHT_SHARE;
    let height = ground(sim, goal.x, goal.z, floor) + over_ground;
    let goal = goal.with_y(height.fmin(ceiling));
    if !clear_chord(sim, craft.pos, goal, source) {
        return None;
    }
    let body = Body {
        pos: craft.pos,
        left: craft.rot * Vec3::X,
        fwd: forward,
        vel: craft.vel,
        aim_origin: craft.pos,
        aim_dir: crate::vehicle::flyer::untilted(craft) * Vec3::Z,
    };
    let (yaw, pitch) = stick_to(p, &body, goal);
    let throttle = if latched.is_some() || distance <= 2.0 * radius {
        -1.0
    } else {
        0.0
    };
    ai.tow_run = Some(TowRun {
        target,
        clockwise,
        opening_shot,
    });
    Some([throttle, 0.0, yaw, pitch])
}
