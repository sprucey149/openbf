//! Route ownership is derived from live pilots, rather than cached counters.
//! The cursor is already hashed. This covers follower detach, pursuit, death,
//! dismount, human takeover and generation reuse without an orphan reservation.

use super::*;
use crate::unit::Controller;

fn pilot(sim: &Sim, index: u32, v: &Vehicle) -> bool {
    v.alive
        && v.kind == VehicleKind::Flyer
        && v.seats
            .first()
            .and_then(|s| s.occupant)
            .and_then(|h| sim.unit(h))
            .is_some_and(|u| {
                u.controller == Controller::Ai
                    && u.mount.is_some_and(|m| {
                        m.vehicle.index == index && m.vehicle.gen == v.gen && m.seat == 0
                    })
            })
}

pub const TRANSPORT_CLASS: u32 = 0xd32f_6312;

pub fn has_transport_paths(sim: &Sim) -> bool {
    sim.flyer_paths
        .iter()
        .any(|p| p.classes.iter().any(|(id, _)| *id == TRANSPORT_CLASS))
}

fn fallback_transport_classes(sim: &Sim, handle: VehicleHandle) -> Vec<u32> {
    let Some(v) = sim.vehicle(handle) else {
        return Vec::new();
    };
    let team = v.home_team.or(v.team);
    let mut classes: Vec<u32> = sim
        .alive_vehicles()
        .filter_map(|(_, other)| {
            if other.home_team == team || other.team == team {
                let p = sim.flyer_params.get(other.class as usize)?.as_ref()?;
                if p.follower.class_id != 0 && p.follower.class_id != TRANSPORT_CLASS {
                    return Some(p.follower.class_id);
                }
            }
            None
        })
        .collect();
    for pad in &sim.vehicle_pads {
        if pad.team == team {
            for c in pad.def.classes.iter().flatten() {
                let cls = c.spec.class;
                if let Some(Some(p)) = sim.flyer_params.get(cls as usize) {
                    if p.follower.class_id != 0 && p.follower.class_id != TRANSPORT_CLASS {
                        classes.push(p.follower.class_id);
                    }
                }
            }
        }
    }
    classes.sort_unstable();
    classes.dedup();
    classes
}

/// Discovery and joining differ for an unspecified follower class on a path
/// with explicit classes (B2 FUN_002f9740 vs FUN_002f9808).
pub(super) fn allows(
    sim: &Sim,
    handle: VehicleHandle,
    class: u32,
    cursor: PathCursor,
    joining: bool,
) -> bool {
    let Some(path) = sim.flyer_paths.get(usize::from(cursor.path)) else {
        return false;
    };
    if !path.enabled || path.nodes.len() < 2 {
        return false;
    }
    if let Some(instance) = path.enable_instance {
        if sim
            .building_turrets
            .iter()
            .any(|t| t.instance == instance && sim.vehicle(t.handle).is_none())
            || sim
                .stations
                .iter()
                .any(|s| s.instance == Some(instance) && s.destroyed)
        {
            return false;
        }
    }
    let mut total = 0;
    let mut same_class = 0;
    let mut direction = path.direction;
    for (index, v) in sim.vehicles.iter().enumerate() {
        if handle.index == index as u32 && handle.gen == v.gen {
            continue;
        }
        if !pilot(sim, index as u32, v) {
            continue;
        }
        let Some(c) = v.flyer.ai.cursor.filter(|c| c.path == cursor.path) else {
            continue;
        };
        total += 1;
        if sim
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .is_some_and(|p| p.follower.class_id == class)
        {
            same_class += 1;
        }
        if direction == 0 && path.single_direction {
            direction = if c.reverse { -1 } else { 1 };
        }
    }
    // FUN_002ff478 skips the capacity/class discovery call for class zero.
    if ((joining || class != 0) && total >= path.max_entities)
        || (direction > 0 && cursor.reverse)
        || (direction < 0 && !cursor.reverse)
    {
        return false;
    }
    if path.classes.is_empty() || (!joining && class == 0) {
        return true;
    }
    let fallback = (class == TRANSPORT_CLASS && !has_transport_paths(sim))
        .then(|| fallback_transport_classes(sim, handle));
    path.classes
        .iter()
        .find(|(id, _)| *id == class || fallback.as_ref().is_some_and(|f| f.contains(id)))
        .is_some_and(|(_, max)| same_class < *max)
}

/// Apply in unit slot order, rechecking a newly selected route against plans
/// already applied this tick. Two immutable thinks may discover the last slot.
pub(crate) fn commit(sim: &Sim, handle: VehicleHandle, mut ai: FlyerAi) -> FlyerAi {
    let Some(v) = sim.vehicle(handle) else {
        return FlyerAi::default();
    };
    if let Some(c) = ai.cursor {
        let same = v.flyer.ai.cursor.is_some_and(|old| old.path == c.path);
        let class = sim
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0, |p| p.follower.class_id);
        if !same && !allows(sim, handle, class, c, true) {
            ai.goal = None;
            if let Some(p) = sim
                .flyer_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
            {
                loiter(v, p, &mut ai);
            } else {
                ai.cursor = None;
                ai.curve = None;
                ai.t = 0.0;
            }
        }
    }
    ai
}

impl Sim {
    /// BF2 FUN_002f9f18: toggle every route with the effective name hash.
    /// Existing followers keep their path; the bit gates new admission.
    pub fn enable_flyer_path(&mut self, name: u32, enabled: bool) {
        for path in &mut self.flyer_paths {
            if path.name == name {
                path.enabled = enabled;
            }
        }
    }
}
