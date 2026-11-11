use crate::walk::WalkSim;
use core::sim::Event;
use core::unit::UnitHandle;
use core::vehicle::VehicleKind;
use glam::Mat4;

#[derive(Clone, Debug)]
pub struct Style {
    pub tracer: Option<core::laser::LaserLook>,
    pub fire_sound: Option<String>,
    pub frequency: u64,
}

pub fn ordinal(walk: &WalkSim, unit: UnitHandle, class: usize, event: usize) -> u64 {
    let count = walk
        .sim
        .unit(unit)
        .and_then(|u| u.mount)
        .and_then(|m| {
            let v = walk.sim.vehicle(m.vehicle)?;
            let setup = walk
                .sim
                .seat_setup
                .get(v.class as usize)?
                .as_ref()?
                .guns
                .get(m.seat as usize)?
                .as_ref()?;
            let slot = setup.weapons.iter().position(|w| w.class == class)?;
            Some(
                v.guns
                    .get(m.seat as usize)?
                    .as_ref()?
                    .weapons
                    .get(slot)?
                    .shots_fired,
            )
        })
        .unwrap_or(1);
    let after=walk.sim.events.iter().skip(event+1).filter(|e|matches!(e,Event::Shot {unit:u,weapon_class,..} if *u==unit && *weapon_class as usize==class)).count() as u64;
    count.saturating_sub(after + 1)
}

pub fn style(walk: &WalkSim, class: usize) -> Option<&Style> {
    walk.walker_shot_styles.get(&class)
}

pub fn visible(walk: &WalkSim, class: usize, shot: u64) -> bool {
    style(walk, class).is_none_or(|s| s.tracer.is_none() || shot.is_multiple_of(s.frequency.max(1)))
}

pub fn frame(walk: &WalkSim, unit: UnitHandle, class: usize, shot: u64) -> Option<Mat4> {
    let m = walk.sim.unit(unit)?.mount?;
    let v = walk.sim.vehicle(m.vehicle)?;
    if v.kind != VehicleKind::Walker {
        return None;
    }
    let seat = walk
        .vehicle_seats
        .get(v.class as usize)?
        .get(m.seat as usize)?;
    let name = &walk.sim.weapon_classes.get(class)?.name;
    let weapon = seat
        .weapons
        .iter()
        .position(|w| w.class.eq_ignore_ascii_case(name))?;
    let points: Vec<_> = seat
        .aimers
        .iter()
        .filter(|a| a.weapon == weapon)
        .flat_map(|a| &a.fire_points)
        .collect();
    let name = points
        .get(shot as usize % points.len().max(1))
        .copied()
        .or(seat.fire_point.as_ref())?;
    let rig = walk.walker_rigs.get(v.class as usize)?.as_ref()?;
    let scale = walk
        .sim
        .walker_params
        .get(v.class as usize)?
        .as_ref()?
        .root_scale;
    Some(Mat4::from_rotation_translation(v.rot, v.pos) * rig.node_now(v, scale, name)?)
}

pub fn loops(walk: &WalkSim) -> Vec<(UnitHandle, usize, &str, glam::Vec3)> {
    walk.sim
        .alive_units()
        .flat_map(|(h, u)| {
            let Some(m) = u.mount else {
                return Vec::new();
            };
            let Some(v) = walk
                .sim
                .vehicle(m.vehicle)
                .filter(|v| matches!(v.kind, VehicleKind::Walker | VehicleKind::Flyer))
            else {
                return Vec::new();
            };
            let Some(gun) = walk
                .sim
                .seat_setup
                .get(v.class as usize)
                .and_then(Option::as_ref)
                .and_then(|s| s.guns.get(m.seat as usize))
                .and_then(Option::as_ref)
            else {
                return Vec::new();
            };
            let Some(state) = v.guns.get(m.seat as usize).and_then(Option::as_ref) else {
                return Vec::new();
            };
            gun.weapons
                .iter()
                .zip(&state.weapons)
                .filter_map(|(w, s)| {
                    let wc = walk.sim.weapon_classes.get(w.class)?;
                    let live = s.fire_cmd > 0
                        && s.shots_fired > 0
                        && !s.overheated
                        && s.reload_timer <= 0.0
                        && s.active;
                    let name = style(walk, w.class)
                        .and_then(|s| s.fire_sound.as_deref())
                        .or(wc.fire_loop_sound.as_deref())
                        .filter(|_| live)?;
                    let pos = frame(walk, h, w.class, s.shots_fired.saturating_sub(1))
                        .map_or(v.pos, |m| m.w_axis.truncate());
                    Some((h, w.class, name, pos))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
