//! BF2 OrdnanceTowCable flight and wrapping (FUN_00323178). The head
//! can latch onto an authored tow primitive. The live span is a cubic
//! Hermite curve swept in ten chords, measured along those same chords.

use super::{Event, Sim};
use crate::collision::{ray_terrain_ps2, HullFilter, Layers};
use crate::projectile::Shooter;
use crate::vehicle::gun;
use crate::vehicle::tow::{TowClass, TowPrimitive};
use crate::vehicle::{walker, Mount, Vehicle, VehicleHandle};
use glam::Vec3;

const CHORDS: usize = 10;
const WRAPS: usize = 13;

/// A wrap point in its primitive's local coordinates.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    prim: usize,
    at: Vec3,
    tangent: Vec3,
}

#[derive(Debug)]
enum Flight {
    Flying {
        pos: Vec3,
        vel: Vec3,
        life: f32,
    },
    Latched {
        target: VehicleHandle,
        anchors: Vec<Anchor>,
    },
}

#[derive(Debug)]
pub struct TowCable {
    mount: Mount,
    shooter: Shooter,
    class: TowClass,
    flight: Flight,
    /// Current sampled world-space polyline for rendering.
    pub points: Vec<Vec3>,
}

/// A fallen walker plays the authored death clip after its hull and crew
/// leave the live vehicle set. Its posed collision is queried as a building.
pub struct TowFall {
    vehicle: Vehicle,
    started: u64,
    duration: f32,
}

impl TowFall {
    fn age(&self, tick: u64) -> f32 {
        tick.saturating_sub(self.started) as f32 * crate::TICK_DT
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"TOW_FALL");
        eat(&self.started.to_le_bytes());
        eat(&self.duration.to_bits().to_le_bytes());
        self.vehicle.hash_into(eat);
    }

    pub fn pose(&self, tick: u64) -> Vehicle {
        let mut vehicle = self.vehicle.clone();
        vehicle.walker.anim = walker::ANIM_DEATH;
        vehicle.walker.clip_time = self.age(tick).min(self.duration);
        vehicle.walker.layer_blend.fill(0.0);
        vehicle
    }
}

impl TowCable {
    fn tripped(&self) -> bool {
        matches!(&self.flight, Flight::Latched { anchors, .. } if anchors.len() == WRAPS)
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"TOW");
        for h in [
            self.mount.vehicle.index,
            self.mount.vehicle.gen,
            self.shooter.owner.index,
            self.shooter.owner.gen,
        ] {
            eat(&h.to_le_bytes());
        }
        eat(&[
            self.mount.seat,
            self.shooter.team,
            u8::from(self.shooter.is_player),
        ]);
        eat(&(self.shooter.weapon_class as u64).to_le_bytes());
        match &self.flight {
            Flight::Flying { pos, vel, life } => {
                eat(&[0]);
                for x in pos
                    .to_array()
                    .into_iter()
                    .chain(vel.to_array())
                    .chain([*life])
                {
                    eat(&x.to_bits().to_le_bytes());
                }
            }
            Flight::Latched { target, anchors } => {
                eat(&[anchors.len() as u8]);
                eat(&target.index.to_le_bytes());
                eat(&target.gen.to_le_bytes());
                for anchor in anchors {
                    eat(&(anchor.prim as u32).to_le_bytes());
                    for x in anchor
                        .at
                        .to_array()
                        .into_iter()
                        .chain(anchor.tangent.to_array())
                    {
                        eat(&x.to_bits().to_le_bytes());
                    }
                }
            }
        }
    }
}

fn span(at: Vec3, tangent: Vec3, muzzle: Vec3, aim: Vec3) -> [Vec3; CHORDS + 1] {
    let end_tangent = aim * -15.0;
    std::array::from_fn(|i| {
        let t = i as f32 / CHORDS as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        at * (2.0 * t3 - 3.0 * t2 + 1.0)
            + tangent * (t3 - 2.0 * t2 + t)
            + muzzle * (-2.0 * t3 + 3.0 * t2)
            + end_tangent * (t3 - t2)
    })
}

fn towards(value: Vec3, goal: Vec3, distance: f32) -> Vec3 {
    let delta = goal - value;
    let len = delta.length();
    if len <= distance {
        goal
    } else {
        value + delta * (distance / len)
    }
}

/// FUN_002669f8: the current leg is ignored. A neighboring previous leg
/// also prohibits the opposite leg. A first wrap may take any other leg.
fn wrap_order(hit: u8, current: u8, previous: Option<u8>) -> i8 {
    if hit == current {
        return 0;
    }
    if previous == Some(hit)
        || (previous.is_some_and(|leg| leg % 2 != current % 2) && hit == (current + 2) % 4)
    {
        -1
    } else {
        1
    }
}

fn nearest(prims: &[TowPrimitive], from: Vec3, dir: Vec3, len: f32) -> Option<(usize, f32)> {
    prims
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.ray_hit(from, dir, len).map(|t| (i, t)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

impl Sim {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fire_tow(
        &mut self,
        mount: Mount,
        shooter: Shooter,
        class: TowClass,
        origin: Vec3,
        dir: Vec3,
        speed: f32,
        life: f32,
    ) {
        self.events.push(Event::Shot {
            unit: shooter.owner,
            weapon_class: shooter.weapon_class as u32,
            origin,
            dir,
        });
        // The PS2 stores the active link on the firing controllable.
        if self
            .tow_cables
            .iter()
            .any(|c| c.mount.vehicle == mount.vehicle && !c.tripped())
        {
            return;
        }
        self.tow_cables.push(TowCable {
            mount,
            shooter,
            class,
            flight: Flight::Flying {
                pos: origin,
                vel: dir * speed,
                life,
            },
            points: Vec::new(),
        });
    }

    fn tow_muzzle(&self, mount: Mount) -> Option<(Vec3, Vec3)> {
        let vehicle = self.vehicle(mount.vehicle)?;
        let seat = usize::from(mount.seat);
        let gun = self
            .seat_setup
            .get(vehicle.class as usize)?
            .as_ref()?
            .guns
            .get(seat)?
            .as_ref()?;
        let (yaw, pitch) = vehicle.guns.get(seat)?.as_ref()?.aim;
        Some((
            vehicle.pos + vehicle.rot * gun.muzzle((yaw, pitch)),
            vehicle.rot * gun::dir(yaw, pitch),
        ))
    }

    pub(super) fn sync_tow_falls(&mut self) {
        if self.tow_falls.is_empty() {
            return;
        }
        self.tow_falls
            .retain(|fall| fall.age(self.tick) < fall.duration);
        self.collision.clear_falling_walkers();
        if let Some(frames) = &self.tow_frames {
            for fall in &self.tow_falls {
                self.collision
                    .add_falling_walker(frames.fallen_bodies(&fall.pose(self.tick)));
            }
        }
    }

    pub(super) fn fly_tow_cables(&mut self, dt: f32) {
        if self.tow_cables.is_empty() {
            return;
        }
        let mut cables = std::mem::take(&mut self.tow_cables);
        cables.retain_mut(|c| self.fly_tow(c, dt));
        self.tow_cables = cables;
    }

    fn fly_tow(&mut self, cable: &mut TowCable, dt: f32) -> bool {
        if let Flight::Latched { target, anchors } = &mut cable.flight {
            if anchors.len() == WRAPS {
                return self.step_tripped_tow(
                    *target,
                    anchors,
                    cable.mount.vehicle,
                    cable.shooter,
                    dt,
                    &mut cable.points,
                );
            }
        }
        if self.unit(cable.shooter.owner).and_then(|u| u.mount) != Some(cable.mount) {
            return false;
        }
        let Some((muzzle, aim)) = self.tow_muzzle(cable.mount) else {
            return false;
        };
        let limit = cable.class.initial_length;
        match &mut cable.flight {
            Flight::Flying { pos, vel, life } => {
                let from = *pos;
                let len = vel.length() * dt.min(*life);
                let dir = vel.normalize_or_zero();
                let end = from + dir * len;
                let mut stop = self
                    .collision
                    .ray_with(from, dir, len, Layers::ORDNANCE, HullFilter::Static)
                    .map_or(len, |hit| hit.t);
                if let Some(hf) = &self.heightfield {
                    if let Some(t) = ray_terrain_ps2(hf, &self.collision.cutters, from, end) {
                        stop = stop.min(t * len);
                    }
                }
                let mut latch = None;
                if let Some(frames) = &self.tow_frames {
                    for (i, v) in self.vehicles.iter().enumerate().filter(|(_, v)| v.alive) {
                        let h = VehicleHandle {
                            index: i as u32,
                            gen: v.gen,
                        };
                        if h == cable.mount.vehicle {
                            continue;
                        }
                        let parts = frames.primitives(v);
                        if parts.is_empty() {
                            if let Some(hit) = self
                                .collision
                                .ray_with(from, dir, stop, Layers::ORDNANCE, HullFilter::Only(h))
                                .filter(|hit| hit.t < stop)
                            {
                                stop = hit.t;
                                latch = None;
                            }
                            continue;
                        }
                        if let Some((prim, t)) =
                            nearest(&parts, from, dir, stop).filter(|(_, t)| *t < stop)
                        {
                            stop = t;
                            let p = &parts[prim];
                            let local = p.frame.inverse();
                            latch = p.leg.map(|_| {
                                (
                                    h,
                                    Anchor {
                                        prim,
                                        at: local.transform_point3(from + dir * t),
                                        tangent: Vec3::ZERO,
                                    },
                                )
                            });
                        }
                    }
                }
                if let Some((target, anchor)) =
                    latch.filter(|_| (from + dir * stop).distance(muzzle) <= limit)
                {
                    cable.points = span(from + dir * stop, Vec3::ZERO, muzzle, aim).into();
                    cable.flight = Flight::Latched {
                        target,
                        anchors: vec![anchor],
                    };
                    return true;
                }
                *pos = end;
                *life -= dt;
                if *life <= 0.0 || stop < len || end.distance(muzzle) > limit {
                    return false;
                }
                cable.points = span(end, Vec3::ZERO, muzzle, aim).into();
                true
            }
            Flight::Latched { target, anchors } => {
                let Some(parts) = self
                    .vehicle(*target)
                    .and_then(|v| Some(self.tow_frames.as_ref()?.primitives(v)))
                else {
                    return false;
                };
                advance_latch(
                    &parts,
                    anchors,
                    muzzle,
                    aim,
                    cable.class,
                    dt,
                    &mut cable.points,
                ) || anchors.len() == WRAPS
            }
        }
    }

    // FUN_00323178 state 3: the live link is gone but the wraps continue
    // contacting the walker. The native call iterates the source seats,
    // overriding friendly fire to 1 while it sends the ordnance packet.
    fn step_tripped_tow(
        &mut self,
        target: VehicleHandle,
        anchors: &mut [Anchor],
        source: VehicleHandle,
        shooter: Shooter,
        dt: f32,
        points: &mut Vec<Vec3>,
    ) -> bool {
        let Some(walker) = self.vehicle(target) else {
            return false;
        };
        // FUN_00323178 B2:315990 reads target +0x174. FUN_00267bf0
        // B2:185259 loads it from class +0x2ec, the MaxHealth setter
        // (FUN_0026a340 B2:186807-186808, hash 0x19971f1b).
        let max_health = walker.health.max;
        let Some(frames) = &self.tow_frames else {
            return false;
        };
        let parts = frames.primitives(walker);
        if anchors.iter().any(|a| a.prim >= parts.len()) {
            return false;
        }
        let Some(vehicle) = self.vehicle(source) else {
            return false;
        };
        let bf2 = self.ground_rules == crate::soldier::GroundRules::Bf2;
        let seats: Vec<_> = if bf2 {
            vehicle
                .seats
                .iter()
                .filter_map(|seat| seat.occupant)
                .filter(|owner| self.unit(*owner).is_some())
                .collect()
        } else {
            // BF1 FUN_002814f8 delivers the ordnance's own descriptor once.
            vec![shooter.owner]
        };
        if seats.is_empty() {
            return false;
        }
        relax_anchors(anchors, &parts, dt);
        points.clear();
        points.extend(
            anchors
                .iter()
                .map(|a| parts[a.prim].frame.transform_point3(a.at)),
        );
        self.melee_lcg_active = true;
        // This is the same native stream as the saber's: 2 states per
        // uniform draw, choosing among the records preceding the final one.
        let index = (self.melee_lcg.unit() * (anchors.len() - 1) as f32) as usize;
        let a = anchors[index];
        let p = &parts[a.prim];
        let dir = p.frame.transform_vector3(a.at).normalize_or_zero();
        let mut desc = self.weapon_classes[shooter.weapon_class]
            .ordnance
            .damage_desc_charged(0.0, false, 1.0);
        // PS2 B2 FUN_00323178:315990 and B1 FUN_002814f8:206359:
        // tripped wraps replace only the packet base, retaining its scales.
        desc.damage = max_health;
        for owner in seats {
            // BF2 state 3 forces a friendly-fire scale of one.
            let team = (!bf2).then_some(shooter.team);
            self.damage_vehicle_by(target, &desc, 1.0, Some(owner), team, Some(dir));
        }
        let alive = self.vehicle(target).is_some();
        if !alive {
            self.record_tow_fall(target);
        }
        alive
    }

    fn record_tow_fall(&mut self, target: VehicleHandle) {
        let Some(vehicle) = self.vehicles.get(target.index as usize).filter(|v| {
            v.gen == target.gen && !v.alive && v.kind == crate::vehicle::VehicleKind::Walker
        }) else {
            return;
        };
        let Some(clip) = self
            .walker_params
            .get(vehicle.class as usize)
            .and_then(Option::as_ref)
            .and_then(|p| p.clips.get(usize::from(walker::ANIM_DEATH)))
            .and_then(Option::as_ref)
            .filter(|c| c.duration > 0.0)
        else {
            return;
        };
        let fall = TowFall {
            vehicle: vehicle.clone(),
            started: self.tick,
            duration: clip.duration,
        };
        if let Some(frames) = &self.tow_frames {
            self.collision
                .add_falling_walker(frames.fallen_bodies(&fall.pose(self.tick)));
        }
        self.tow_falls.push(fall);
    }
}

fn relax_anchors(anchors: &mut [Anchor], parts: &[TowPrimitive], dt: f32) {
    for anchor in anchors.iter_mut().skip(1) {
        let offset = parts[anchor.prim].offset;
        let desired = Vec3::new(offset.x, anchor.at.y, offset.z);
        anchor.at = towards(anchor.at, desired, dt * 30.0);
        anchor.tangent = towards(anchor.tangent, Vec3::ZERO, dt * 30.0);
    }
}

fn advance_latch(
    parts: &[TowPrimitive],
    anchors: &mut Vec<Anchor>,
    muzzle: Vec3,
    aim: Vec3,
    class: TowClass,
    dt: f32,
    out: &mut Vec<Vec3>,
) -> bool {
    if anchors.iter().any(|a| a.prim >= parts.len()) {
        return false;
    }
    relax_anchors(anchors, parts, dt);
    let current = anchors.last().expect("latched cables have an anchor");
    let last = &parts[current.prim];
    let points = span(
        last.frame.transform_point3(current.at),
        last.frame.transform_vector3(current.tangent),
        muzzle,
        aim,
    );
    let length = points.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>();
    let limit = if anchors.len() == 1 {
        class.initial_length
    } else {
        class.length
    };
    if length > limit {
        return false;
    }
    // FUN_003223d0 B2:314962,315027 clears records then inserts the
    // initial contact. FUN_00323178 B2:315850-315858 counts occupied
    // records (including that contact), before inserting the next at 316497.
    // Its >2 gate (316196-316197) therefore matches anchors.len() > 2.
    let previous = if anchors.len() > 2 {
        parts[anchors[anchors.len() - 2].prim].leg
    } else {
        None
    };
    let leg = last.leg.expect("latched leg");
    for w in points.windows(2) {
        let delta = w[1] - w[0];
        let len = delta.length();
        if len == 0.0 {
            continue;
        }
        let dir = delta / len;
        let Some((prim, t)) = nearest(parts, w[0], dir, len) else {
            continue;
        };
        let Some(next) = parts[prim].leg else {
            return false;
        };
        match wrap_order(next, leg, previous) {
            -1 => return false,
            0 => continue,
            _ => {
                let inverse = parts[prim].frame.inverse();
                anchors.push(Anchor {
                    prim,
                    at: inverse.transform_point3(w[0] + dir * t),
                    tangent: inverse.transform_vector3(dir),
                });
                if anchors.len() == WRAPS {
                    // FUN_00323178 enters state 3 and clears
                    // the firing vehicle's live cable link.
                    // Detached wraps begin the damage path on the next update.
                    return false;
                }
                break;
            }
        }
    }
    out.clear();
    out.extend(
        anchors
            .iter()
            .take(anchors.len() - 1)
            .map(|a| parts[a.prim].frame.transform_point3(a.at)),
    );
    let tip = anchors.last().expect("latched cable");
    let p = &parts[tip.prim];
    out.extend(span(
        p.frame.transform_point3(tip.at),
        p.frame.transform_vector3(tip.tangent),
        muzzle,
        aim,
    ));
    true
}

#[cfg(test)]
mod tests;
