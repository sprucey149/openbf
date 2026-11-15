//! BF2 transport approach and cargo deployment at a hostile hangar.

use crate::fmath::FloatExt;
use super::{
    chords_clear, hull_radius, FlightPhase, FlyerAi, Sim, Vec3, Vehicle, VehicleHandle, VehicleKind,
};
use crate::region::Region;

/// OURS: port turnaround interval. measure: original time from touchdown to departure.
pub(super) const WAIT: u64 = 10 * crate::TICK_HZ as u64;
/// OURS: door approach and exit clearance in hull radii beyond the volume.
/// replace: original spline approach and departure clearance.
const APPROACH_CLEARANCE_RADII: f32 = 4.0;

/// Use this planner where the world declares a transport follower route.
pub(crate) fn uses_authored_transport(sim: &Sim, v: &Vehicle) -> bool {
    v.kind == VehicleKind::Flyer
        && sim
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .is_some_and(|p| p.bf2 && p.follower.class_id == super::TRANSPORT_CLASS)
        && sim.round_rules().space_deathmatch()
        && super::has_transport_paths(sim)
}

fn destination_regions<'a>(sim: &'a Sim, v: &Vehicle) -> impl Iterator<Item = &'a Region> {
    let team = v.team.or(v.home_team);
    let no_pad = v.spawn_pad.is_none();
    let returning = v.flyer.ai.transport_returning;
    sim.landing_regions.iter().filter(move |region| {
        if let Some(post) = sim.cps.iter().min_by(|a, b| {
            a.def
                .pos
                .distance_squared(region.center)
                .total_cmp(&b.def.pos.distance_squared(region.center))
        }) {
            return team.zip(post.owner).is_some_and(|(a, b)| {
                if returning {
                    a == b
                } else {
                    sim.hostile(a, b)
                }
            });
        }
        no_pad
    })
}

pub(crate) fn at_destination(sim: &Sim, v: &Vehicle) -> bool {
    uses_authored_transport(sim, v) && destination_regions(sim, v).any(|r| r.contains(v.pos))
}

/// The PS2 fly agent lines up with a landing hint. The stock space maps
/// instead provide landing volumes. Use the clear face of the destination
/// volume as the intermediate point and the center as the touchdown target.
/// replace: match the original follower's approach in a stock space hangar.
pub(super) fn approach(sim: &Sim, v: &Vehicle, handle: VehicleHandle, ai: &mut FlyerAi) -> bool {
    if !uses_authored_transport(sim, v)
        || v.flyer.phase != FlightPhase::Flying
        || ai.transport_depart_at.is_some()
    {
        return false;
    }
    let forward = v.rot * Vec3::Z;
    let radius = hull_radius(sim, v);
    if let Some(goal) = ai.transport_goal.take() {
        if leg(sim, v, handle, ai, goal, goal + (goal - v.pos), radius) {
            ai.transport_goal = Some(goal);
            return true;
        }
    }
    // The two hangars are separated by open space. A transport can leave an
    // authored route once it has a clear line to the opposite hangar.
    for region in destination_regions(sim, v) {
        let center = region.center;
        if (center - v.pos).dot(forward) <= 0.0 {
            continue;
        }
        let width = region.size.x.fmax(region.size.z);
        // Carry a straight approach through the doorway before descending.
        let points = [region.basis.x_axis, region.basis.z_axis];
        let mut entries = [points[0], -points[0], points[1], -points[1]]
            .map(|dir| center + dir * (width + APPROACH_CLEARANCE_RADII * radius));
        entries.sort_by(|a, b| {
            v.pos
                .distance_squared(*a)
                .total_cmp(&v.pos.distance_squared(*b))
        });
        for entry in entries {
            if (entry - v.pos).dot(forward) <= 0.0
                || !chords_clear(sim, &[entry, center], radius, handle)
            {
                continue;
            }
            if leg(sim, v, handle, ai, entry, center, radius) {
                ai.transport_goal = Some(center);
                return true;
            }
        }
    }
    false
}

/// Carry the original roof escape forward before the ordinary route join.
/// After a drop, use the hangar opening even if the ship landed nose-in.
/// replace: original transport spline departure and turning profile.
pub(super) fn departure(sim: &Sim, v: &Vehicle, handle: VehicleHandle, ai: &mut FlyerAi) -> bool {
    let Some(region) = sim.landing_regions.iter().find(|r| r.contains(v.pos)) else {
        return false;
    };
    let radius = hull_radius(sim, v);
    let width = region.size.x.fmax(region.size.z);
    let center = Vec3::new(region.center.x, v.pos.y, region.center.z);
    let [x, z] = [region.basis.x_axis, region.basis.z_axis];
    for axis in [x, -x, z, -z] {
        let end = center + axis * (width + APPROACH_CLEARANCE_RADII * radius);
        if !chords_clear(sim, &[v.pos, end], radius, handle) {
            continue;
        }
        // Straight translation through the opening. The spline aligns the
        // hull along the escape chord as it begins moving.
        let delta = end - v.pos;
        ai.curve = Some([v.pos - delta, v.pos, end, end + delta]);
        ai.cursor = None;
        ai.goal = None;
        ai.t = 0.0;
        return true;
    }
    false
}

fn leg(
    sim: &Sim,
    v: &Vehicle,
    handle: VehicleHandle,
    ai: &mut FlyerAi,
    end: Vec3,
    after: Vec3,
    radius: f32,
) -> bool {
    let forward = v.rot * Vec3::Z;
    let behind = (end - v.pos).length().fmin(super::CONNECT_BEHIND);
    let curve = [v.pos - forward * behind, v.pos, end, after];
    let points: [Vec3; super::HOP_SAMPLES + 1] = std::array::from_fn(|k| {
        crate::vehicle::flyer::curve_at(&curve, k as f32 / super::HOP_SAMPLES as f32).0
    });
    if !chords_clear(sim, &points, radius, handle) {
        return false;
    }
    ai.curve = Some(curve);
    ai.cursor = None;
    ai.goal = None;
    ai.t = 0.0;
    true
}
