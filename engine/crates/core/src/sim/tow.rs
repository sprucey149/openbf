//! BF2 OrdnanceTowCable flight and wrapping (FUN_00323178). The head
//! can latch onto an authored tow primitive. The live span is a cubic
//! Hermite curve swept in ten chords, measured along those same chords.

use super::{Event, Sim};
use crate::collision::{ray_terrain_ps2, HullFilter, Layers};
use crate::fmath::FloatExt;
use crate::projectile::Shooter;
use crate::vehicle::gun;
use crate::vehicle::tow::{TowClass, TowPrimitive};
use crate::vehicle::{Mount, Vehicle, VehicleHandle};
use glam::Vec3;

const CHORDS: usize = 10;
const WRAPS: usize = 13;
const GUN_TANGENT_LEN: f32 = 15.0;

/// A local wrap point with its tangent in world coordinates.
#[derive(Clone, Copy, Debug)]
struct Anchor {
    prim: usize,
    at: Vec3,
    // The live tangent is the newest record's unflattened +0x80 vector.
    tangent: Vec3,
}

#[derive(Clone, Debug)]
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
    Detached {
        target: VehicleHandle,
        anchors: Vec<Anchor>,
    },
}

#[derive(Clone, Debug)]
pub struct TowCable {
    mount: Mount,
    shooter: Shooter,
    class: TowClass,
    flight: Flight,
    /// Current sampled world-space polyline for rendering.
    pub points: Vec<Vec3>,
}

impl TowCable {
    fn tripped(&self) -> bool {
        matches!(self.flight, Flight::Detached { .. })
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
                    eat(&x.hash_bits().to_le_bytes());
                }
            }
            Flight::Latched { target, anchors } | Flight::Detached { target, anchors } => {
                let count = anchors.len() as u8;
                // The high bit distinguishes the full live phase from the old count tags.
                let tag = if anchors.len() == WRAPS && !self.tripped() {
                    count | 0x80
                } else {
                    count
                };
                eat(&[tag]);
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
                        eat(&x.hash_bits().to_le_bytes());
                    }
                }
            }
        }
    }
}

fn span(at: Vec3, tangent: Vec3, muzzle: Vec3, aim: Vec3) -> [Vec3; CHORDS + 1] {
    let end_tangent = aim * -GUN_TANGENT_LEN;
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

    /// A dying walker's posed collision ([`crate::vehicle::walker::WalkerDeath`]): the death
    /// function moves its collider to the building list and FUN_00261208
    /// keeps posing it until entry 4 (`analysis/re-tow-cable.md`). Where the
    /// tow frames pose a class, those bodies stand in for its rigid hull
    /// ([`Sim::dying_walker_bodies`]); any other class keeps the hull.
    pub(super) fn sync_dying_walkers(&mut self) {
        self.collision.clear_falling_walkers();
        if self.tow_frames.is_none() {
            return;
        }
        let bodies: Vec<_> = self
            .vehicles
            .iter()
            .filter_map(|v| self.dying_walker_bodies(v))
            .collect();
        for b in bodies {
            self.collision.add_falling_walker(b);
        }
    }

    /// The posed bodies of a dead walker, when the tow frames have any.
    pub(super) fn dying_walker_bodies(&self, v: &Vehicle) -> Option<Vec<crate::collision::Body>> {
        if !v.alive || v.kind != crate::vehicle::VehicleKind::Walker || !v.walker.dead {
            return None;
        }
        let bodies = self.tow_frames.as_ref()?.fallen_bodies(v);
        (!bodies.is_empty()).then_some(bodies)
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
        if cable.tripped()
            || matches!(&cable.flight, Flight::Latched { anchors, .. } if anchors.len() == WRAPS)
        {
            return self.step_full_tow(cable, dt);
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
                let len = vel.length() * dt.fmin(*life);
                let dir = vel.normalize_or_zero();
                let end = from + dir * len;
                let mut stop = self
                    .collision
                    .ray_with(from, dir, len, Layers::ORDNANCE, HullFilter::Static)
                    .map_or(len, |hit| hit.t);
                if let Some(hf) = &self.heightfield {
                    if let Some(t) = ray_terrain_ps2(hf, &self.collision.cutters, from, end) {
                        stop = stop.fmin(t * len);
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
                let space = self.space_round();
                let active = advance_latch(
                    &parts,
                    anchors,
                    (muzzle, aim),
                    cable.class,
                    dt,
                    &mut cable.points,
                    space,
                );
                if anchors.len() < WRAPS {
                    return active;
                }
                if space {
                    cable.flight = Flight::Detached {
                        target: *target,
                        anchors: std::mem::take(anchors),
                    };
                } else {
                    cable.points.clear();
                }
                true
            }
            Flight::Detached { .. } => unreachable!("detached wraps update before the live span"),
        }
    }

    fn step_full_tow(&mut self, cable: &mut TowCable, dt: f32) -> bool {
        let (target, anchors, detached) = match &mut cable.flight {
            Flight::Latched { target, anchors } => (*target, anchors, false),
            Flight::Detached { target, anchors } => (*target, anchors, true),
            Flight::Flying { .. } => unreachable!("full wraps have a target"),
        };
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
        let Some(vehicle) = self.vehicle(cable.mount.vehicle) else {
            return false;
        };
        // B2 FUN_00323178:316106–316139 and B1 FUN_002814f8:206776–206808
        // clear the link one update after the last insert, then return.
        if !detached {
            settle_full_wraps(anchors, &parts, dt, &mut cable.points);
            cable.flight = Flight::Detached {
                target,
                anchors: std::mem::take(anchors),
            };
            return true;
        }
        let shooter = cable.shooter;
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
        settle_full_wraps(anchors, &parts, dt, &mut cable.points);
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
        // A walker the wraps killed is dying (`Sim::destroy_vehicle`): the
        // link ends there.
        self.vehicle(target).is_some_and(|v| v.health.alive())
    }
}

fn settle_full_wraps(
    anchors: &mut [Anchor],
    parts: &[TowPrimitive],
    dt: f32,
    points: &mut Vec<Vec3>,
) {
    relax_anchors(anchors, parts, dt);
    points.clear();
    points.extend(
        anchors
            .iter()
            .map(|a| parts[a.prim].frame.transform_point3(a.at)),
    );
}

fn relax_anchors(anchors: &mut [Anchor], parts: &[TowPrimitive], dt: f32) {
    let newest = anchors.len().saturating_sub(1);
    for (index, anchor) in anchors.iter_mut().enumerate().skip(1) {
        let offset = parts[anchor.prim].offset;
        let desired = Vec3::new(offset.x, anchor.at.y, offset.z);
        anchor.at = towards(anchor.at, desired, dt * 30.0);
        // B2 FUN_003248d8 only relaxes older +0x80 copies.
        if index < newest {
            anchor.tangent = towards(anchor.tangent, Vec3::ZERO, dt * 30.0);
        }
    }
}

fn advance_latch(
    parts: &[TowPrimitive],
    anchors: &mut Vec<Anchor>,
    muzzle_and_aim: (Vec3, Vec3),
    class: TowClass,
    dt: f32,
    out: &mut Vec<Vec3>,
    space: bool,
) -> bool {
    let (muzzle, aim) = muzzle_and_aim;
    if anchors.iter().any(|a| a.prim >= parts.len()) {
        return false;
    }
    relax_anchors(anchors, parts, dt);
    let current = anchors.last().expect("latched cables have an anchor");
    let last = &parts[current.prim];
    let start = last.frame.transform_point3(current.at);
    let start_tangent = current.tangent;
    let points = span(start, start_tangent, muzzle, aim);
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
    for (chord, w) in points.windows(2).enumerate() {
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
                // B2 FUN_00323178:316443–316497 stores half the world
                // Hermite derivative at the hit's parameter along this chord.
                let fraction = t / len;
                let parameter = (chord as f32 + fraction) / CHORDS as f32;
                let displacement = muzzle - start;
                let end_tangent = aim * -GUN_TANGENT_LEN;
                let cubic = start_tangent + end_tangent - displacement * 2.0;
                let quadratic = displacement * 3.0 - start_tangent * 2.0 - end_tangent;
                let derivative =
                    (cubic * (3.0 * parameter) + quadratic * 2.0) * parameter + start_tangent;
                let inverse = parts[prim].frame.inverse();
                // Both originals also scale the unit chord direction by the hit fraction
                // for the stored point (B2:316442–316459, B1:206692–206721).
                let advance = if space { t } else { fraction };
                anchors.push(Anchor {
                    prim,
                    at: inverse.transform_point3(w[0] + dir * advance),
                    tangent: derivative * 0.5,
                });
                if anchors.len() == WRAPS {
                    // FUN_00322890 and FUN_00280c20 omit the full live span.
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
        tip.tangent,
        muzzle,
        aim,
    ));
    true
}

#[cfg(test)]
mod tests;
