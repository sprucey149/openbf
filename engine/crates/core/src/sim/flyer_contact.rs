use super::Sim;
use crate::collision::{bounding_radius, Body, HullFilter, HullPart, Layers, Shape};
use crate::vehicle::flyer::{FlightPhase, FlyerParams};
use crate::vehicle::{collision_desc, impact_damage, VehicleHandle, VehicleKind};
use glam::{Mat3, Mat4, Quat, Vec3};

const SNAP_DISTANCE: f32 = 100.0;
const CONTACTS_PER_STEP: usize = 8;
const FLYER_VS_FLYER_DAMAGE: f32 = 0.2;
const NOSE_DAMAGE: f32 = 0.5;
const OTHER_DAMAGE_LIMIT: f32 = 0.5;
const RESTITUTION: f32 = -1.4;
const FRICTION: f32 = 0.125;
const JOLT_PER_SPEED: f32 = 0.025;
const JOLT_BASE: f32 = 0.01;
const JOLT_TURN: f32 = 0.3;
const CEILING_NORMAL: f32 = -0.7;
const TOUCHDOWN_NORMAL: f32 = 0.7;
const TOUCHDOWN_UP: f32 = 0.9375;
const TOUCHDOWN_LIFT: f32 = 0.0001;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FlyerRow {
    pub threshold: [f32; 3],
    pub scale: [f32; 3],
    pub bf2: bool,
    pub command: bool,
}

impl FlyerRow {
    pub(super) fn of(p: &FlyerParams) -> Self {
        FlyerRow {
            threshold: p.collision_threshold,
            scale: p.collision_scale,
            bf2: p.bf2,
            command: p.command_flyer,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FlyerHit {
    normal: Vec3,
    depth: f32,
    other: Option<VehicleHandle>,
}

#[derive(Clone, Copy, Debug)]
struct OtherHull {
    handle: VehicleHandle,
    flyer: bool,
    flying: bool,
    vel: Vec3,
    max_health: f32,
}

impl Sim {
    pub(super) fn flyer_contact(
        &mut self,
        i: usize,
        handle: VehicleHandle,
        from: Vec3,
        row: FlyerRow,
    ) {
        if self.vehicles[i].flyer.phase == FlightPhase::Landed {
            return;
        }
        let moved = self.vehicles[i].pos - from;
        let distance = moved.length();
        if distance > SNAP_DISTANCE {
            return;
        }
        let parts = self
            .hull_parts
            .get(self.vehicles[i].class as usize)
            .and_then(Clone::clone);
        let parts: &[HullPart] = parts.as_deref().unwrap_or(&[]);
        let radius = bounding_radius(parts);
        let steps = if radius > 0.0 {
            (distance / radius + 1.0) as u32
        } else {
            1
        };
        let step = moved / steps as f32;
        self.vehicles[i].pos = from;
        for _ in 0..steps {
            self.vehicles[i].pos += step;
            for _ in 0..CONTACTS_PER_STEP {
                let Some(hit) = self.flyer_touching(i, handle, parts, from) else {
                    break;
                };
                if !self.flyer_hit(i, handle, hit, row) {
                    return;
                }
                if self.vehicles[i].flyer.phase == FlightPhase::Landed {
                    return;
                }
            }
        }
    }

    fn flyer_touching(
        &self,
        i: usize,
        handle: VehicleHandle,
        parts: &[HullPart],
        from: Vec3,
    ) -> Option<FlyerHit> {
        let v = &self.vehicles[i];
        let pose = Mat4::from_rotation_translation(v.rot, v.pos);
        let admits = |b: &Body| self.flyer_meets(b);
        let mut best: Option<FlyerHit> = None;
        let mut has_prim = false;
        for part in parts {
            if !part.flyer_part || matches!(part.shape, Shape::Mesh(_)) {
                continue;
            }
            has_prim = true;
            let found = self.collision.deepest_contact(
                part,
                pose,
                HullFilter::except(Some(handle)),
                admits,
            );
            if let Some(c) = found {
                if best.is_none_or(|b| c.depth > b.depth) {
                    best = Some(FlyerHit {
                        normal: c.normal,
                        depth: c.depth,
                        other: c.owner,
                    });
                }
            }
        }
        if !has_prim {
            let radius = bounding_radius(parts);
            if radius > 0.0 {
                let fallback =
                    HullPart::new(Mat4::IDENTITY, Shape::Sphere { radius }, Layers::FLYER)
                        .with_flyer_subsets(true, true);
                if let Some(c) = self.collision.deepest_contact(
                    &fallback,
                    pose,
                    HullFilter::except(Some(handle)),
                    admits,
                ) {
                    if best.is_none_or(|b| c.depth > b.depth) {
                        best = Some(FlyerHit {
                            normal: c.normal,
                            depth: c.depth,
                            other: c.owner,
                        });
                    }
                }
            }
        }
        if let Some(ground) = self.flyer_on_terrain(parts, pose, from) {
            if best.is_none_or(|b| ground.depth > b.depth) {
                best = Some(ground);
            }
        }
        best
    }

    fn flyer_meets(&self, body: &Body) -> bool {
        let building = body
            .owner
            .and_then(|h| self.vehicle(h))
            .is_some_and(|o| matches!(o.kind, VehicleKind::Flyer | VehicleKind::Walker));
        if building {
            body.flyer_part
        } else {
            body.layers.contains(Layers::FLYER)
        }
    }

    fn flyer_on_terrain(&self, parts: &[HullPart], pose: Mat4, from: Vec3) -> Option<FlyerHit> {
        let hf = self.heightfield.as_ref()?;
        if from.y < hf.height_at(from.x, from.z) {
            return None;
        }
        let center = pose.w_axis.truncate();
        let terrain = parts.iter().filter(|p| p.terrain_part);
        let lows = terrain.filter_map(|p| p.lowest_point(pose));
        let mut best: Option<(f32, Vec3)> = None;
        let mut any = false;
        for low in lows {
            any = true;
            let depth = hf.height_at(low.x, low.z) - low.y;
            if best.is_none_or(|b| depth > b.0) {
                best = Some((depth, low));
            }
        }
        if !any {
            let r = bounding_radius(parts);
            best = Some((
                hf.height_at(center.x, center.z) - (center.y - r),
                center - Vec3::Y * r,
            ));
        }
        let (depth, low) = best?;
        if depth <= 0.0 {
            return None;
        }
        let height = hf.height_at(low.x, low.z);
        if self.collision.is_cut(Vec3::new(low.x, height, low.z)) {
            return None;
        }
        Some(FlyerHit {
            normal: hf.normal_at(low.x, low.z).into(),
            depth,
            other: None,
        })
    }

    fn flyer_hit(&mut self, i: usize, handle: VehicleHandle, hit: FlyerHit, row: FlyerRow) -> bool {
        let n = hit.normal;
        let other = hit.other.and_then(|h| {
            let o = self.vehicle(h)?;
            let flyer = o.kind == VehicleKind::Flyer;
            Some(OtherHull {
                handle: h,
                flyer,
                flying: flyer && o.flyer.phase != FlightPhase::Landed,
                vel: o.vel,
                max_health: o.health.max,
            })
        });
        if self.vehicles[i].flyer.phase == FlightPhase::Takeoff {
            if other.is_some_and(|o| o.flying) {
                return true;
            }
            // BF2: a craft taking off destroys a non-flying object it touches.
            // Space modes only until the rule is checked against Conquest
            // hangars, where it would destroy parked tanks and their riders.
            let space = self.space_round();
            if let Some(o) = other.filter(|o| !o.flying && space) {
                self.destroy_vehicle(o.handle, None);
            }
            if n.y < CEILING_NORMAL {
                self.vehicles[i].flyer.phase = FlightPhase::Landing;
                self.vehicles[i].pos += n * hit.depth;
                return true;
            }
        }
        let v = &self.vehicles[i];
        let (phase, rot, max_health) = (v.flyer.phase, v.rot, v.health.max);
        let (up, forward) = (rot * Vec3::Y, rot * Vec3::Z);
        let other_vel = other.map_or(Vec3::ZERO, |o| o.vel);
        let relative = v.vel - other_vel;
        let along = relative.dot(n);
        if along >= 0.0 {
            self.vehicles[i].pos += n * hit.depth;
            return true;
        }
        let mut damage = impact_damage(along, n.dot(up), n.dot(forward), row.threshold, row.scale);
        if row.bf2 {
            if phase == FlightPhase::Flying {
                damage += (-n).dot(forward).max(0.0) * NOSE_DAMAGE * max_health;
            }
            if let Some(o) = other {
                if o.flyer && !row.command {
                    damage = FLYER_VS_FLYER_DAMAGE * max_health;
                } else {
                    damage = damage.min(o.max_health * OTHER_DAMAGE_LIMIT);
                }
            }
        }
        let will_die = damage >= self.vehicles[i].health.cur && self.vehicles[i].health.cur > 0.0;
        let was_falling = matches!(phase, FlightPhase::Falling | FlightPhase::Crashed)
            || self.vehicles[i].flyer.phase == FlightPhase::Falling;
        let lethal = will_die || was_falling;

        if damage > 0.0 && self.vehicles[i].health.cur > 0.0 {
            self.damage_vehicle(handle, &collision_desc(damage), 1.0, None);
            if let Some(o) = other.filter(|o| o.flyer && !o.flying) {
                let other_damage = FLYER_VS_FLYER_DAMAGE * o.max_health;
                self.damage_vehicle(o.handle, &collision_desc(other_damage), 1.0, None);
            }
        }

        if lethal {
            self.events.push(crate::sim::Event::FlyerImpact {
                vehicle: handle,
                pos: self.vehicles[i].pos,
                normal: n,
                speed: -along,
                damage,
                lethal: true,
            });
            self.vehicles[i].pos += n * hit.depth;
            self.vehicles[i].flyer.phase = FlightPhase::Crashed;
            if self.vehicle(handle).is_some_and(|v| v.alive) {
                let killer = self.vehicles[i].flyer.damager;
                self.destroy_vehicle(handle, killer);
            }
            return false;
        }

        self.events.push(crate::sim::Event::FlyerImpact {
            vehicle: handle,
            pos: self.vehicles[i].pos,
            normal: n,
            speed: -along,
            damage,
            lethal: false,
        });

        let mut impulse = RESTITUTION * along;
        if other.is_some_and(|o| o.flying) {
            impulse *= 0.5;
        }
        let sliding = relative - n * along;
        let speed = sliding.length();
        let kept = if speed > 0.0 {
            1.0 - (impulse * FRICTION / speed).min(1.0)
        } else {
            1.0
        };
        let v = &mut self.vehicles[i];
        v.vel = sliding * kept + n * (impulse + along) + other_vel;
        if phase == FlightPhase::Landing {
            if n.y > TOUCHDOWN_NORMAL && n.dot(up) > TOUCHDOWN_UP && v.flyer.lift < TOUCHDOWN_LIFT {
                v.flyer.phase = FlightPhase::Landed;
            }
            v.pos += n * hit.depth;
            return true;
        }
        v.rot = jolted(v.rot, v.vel, n);
        v.pos += n * hit.depth;
        true
    }
}

fn jolted(rot: Quat, vel: Vec3, n: Vec3) -> Quat {
    let (up, forward) = (rot * Vec3::Y, rot * Vec3::Z);
    let turn = (vel.length() * JOLT_PER_SPEED + JOLT_BASE).min(1.0);
    let mut aim = forward * (1.0 - turn * JOLT_TURN) + n * (turn * JOLT_TURN);
    if vel != Vec3::ZERO {
        aim += vel.normalize() * JOLT_TURN;
    }
    let forward = aim.normalize_or(forward);
    let right = up.cross(forward).normalize_or(rot * Vec3::X);
    let up = forward.cross(right);
    Quat::from_mat3(&Mat3::from_cols(right, up, forward)).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, CollisionWorld};
    use crate::unit::UnitSpec;
    use crate::vehicle::{SeatKind, VehicleSpec};
    use crate::weapon::HealthType;
    use glam::Vec3;
    use std::sync::Arc;

    const HEALTH: f32 = 1500.0;

    fn box_at(center: Vec3, half: Vec3) -> Body {
        let mut b = Body::new(Mat4::from_translation(center), Shape::Box { half });
        b.layers = Layers::FLYER | Layers::SOLDIER;
        b
    }

    fn flying(world: Vec<Body>, bf2: bool) -> (Sim, VehicleHandle) {
        let mut sim = Sim::empty(42);
        sim.collision = CollisionWorld::new(world, Vec::new());
        sim.flyer_params = vec![Some(FlyerParams {
            bf2,
            landed_height: 1.0,
            takeoff_height: 2.5,
            takeoff_time: 1.0,
            acceleration: 30.0,
            min_speed: 20.0,
            mid_speed: 80.0,
            max_speed: 105.0,
            boost_speed: 170.0,
            collision_threshold: [5.0, 2.5, 0.0],
            collision_scale: [1.5, 0.0, 0.0],
            ..FlyerParams::default()
        })];
        sim.hull_parts = vec![Some(Arc::from(vec![HullPart::new(
            Mat4::IDENTITY,
            Shape::Sphere { radius: 4.0 },
            Layers::VEHICLE,
        )
        .with_flyer_subsets(true, true)]))];
        let h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: HEALTH,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let pilot = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
        assert!(sim.seat_unit(pilot, h, 0));
        let v = sim.vehicle_mut(h).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.lift = 1.0;
        (sim, h)
    }

    fn fly(sim: &mut Sim, h: VehicleHandle, pos: Vec3, vel: Vec3) {
        let v = sim.vehicle_mut(h).unwrap();
        let from = pos - vel * crate::TICK_DT;
        v.pos = pos;
        v.vel = vel;
        let row = FlyerRow::of(sim.flyer_params[0].as_ref().unwrap());
        let i = h.index as usize;
        sim.flyer_contact(i, h, from, row);
    }

    fn wall() -> Vec<Body> {
        vec![box_at(
            Vec3::new(0.0, 0.0, 50.0),
            Vec3::new(40.0, 40.0, 1.0),
        )]
    }

    fn floor() -> Vec<Body> {
        vec![box_at(
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(500.0, 1.0, 500.0),
        )]
    }

    #[test]
    fn nose_first_into_a_wall_at_speed_is_the_end_of_the_ship() {
        let (mut sim, h) = flying(wall(), true);
        fly(&mut sim, h, Vec3::new(0.0, 0.0, 47.0), Vec3::Z * 80.0);
        assert!(sim.vehicle(h).is_none_or(|v| !v.alive));
    }

    #[test]
    fn a_grazing_floor_hit_pushes_up_rebounds_and_damages() {
        let (mut sim, h) = flying(floor(), true);
        let vel = Vec3::new(0.0, -10.0, 80.0);
        let rot = Quat::from_rotation_arc(Vec3::Z, vel.normalize());
        sim.vehicle_mut(h).unwrap().rot = rot;
        fly(&mut sim, h, Vec3::Y * 3.5, vel);
        let v = sim.vehicle(h).unwrap();
        assert!(v.alive);
        assert!((v.pos.y - 4.0).abs() < 1e-3, "{}", v.pos.y);
        assert!((v.vel.y - 4.0).abs() < 1e-3, "{}", v.vel.y);
        assert!(v.vel.z < 80.0 && v.vel.z > 70.0, "{}", v.vel.z);
        let nose = (rot * Vec3::Z).dot(Vec3::NEG_Y);
        let speed = 10.0 - (5.0 + 2.5 * (rot * Vec3::Y).y);
        let want = HEALTH - (speed * speed * 1.5 + nose * 0.5 * HEALTH);
        assert!((v.health.cur - want).abs() < 1.0, "{}", v.health.cur);
    }

    #[test]
    fn a_contact_turns_the_nose_away() {
        let (mut sim, h) = flying(floor(), true);
        let vel = Vec3::new(0.0, -10.0, 80.0);
        sim.vehicle_mut(h).unwrap().rot = Quat::from_rotation_arc(Vec3::Z, vel.normalize());
        let before = (sim.vehicle(h).unwrap().rot * Vec3::Z).y;
        fly(&mut sim, h, Vec3::Y * 3.5, vel);
        let after = (sim.vehicle(h).unwrap().rot * Vec3::Z).y;
        assert!(after > before, "{before} {after}");
    }

    #[test]
    fn moving_away_from_a_surface_is_pushed_out_without_damage() {
        let (mut sim, h) = flying(floor(), true);
        fly(&mut sim, h, Vec3::Y * 3.5, Vec3::new(0.0, 5.0, 80.0));
        let v = sim.vehicle(h).unwrap();
        assert_eq!(v.health.cur, HEALTH);
        assert!((v.pos.y - 4.0).abs() < 1e-3);
        assert_eq!(v.vel.y, 5.0);
    }

    #[test]
    fn a_ship_passes_a_body_outside_its_flyer_subset() {
        let mut field = box_at(Vec3::new(0.0, 0.0, 50.0), Vec3::new(40.0, 40.0, 1.0));
        field.layers = Layers::SOLDIER;
        let (mut sim, h) = flying(vec![field], true);
        fly(&mut sim, h, Vec3::new(0.0, 0.0, 49.0), Vec3::Z * 80.0);
        let v = sim.vehicle(h).unwrap();
        assert_eq!(v.health.cur, HEALTH);
        assert_eq!(v.vel, Vec3::Z * 80.0);
    }

    #[test]
    fn a_ceiling_on_takeoff_starts_a_landing() {
        let ceiling = box_at(Vec3::new(0.0, 10.0, 0.0), Vec3::new(50.0, 1.0, 50.0));
        let (mut sim, h) = flying(vec![ceiling], true);
        sim.vehicle_mut(h).unwrap().flyer.phase = FlightPhase::Takeoff;
        fly(&mut sim, h, Vec3::Y * 6.0, Vec3::Y * 3.0);
        assert_eq!(sim.vehicle(h).unwrap().flyer.phase, FlightPhase::Landing);
    }

    #[test]
    fn a_landing_touches_down_only_once_the_lift_is_gone() {
        let (mut sim, h) = flying(floor(), true);
        let v = sim.vehicle_mut(h).unwrap();
        v.flyer.phase = FlightPhase::Landing;
        v.flyer.lift = 0.5;
        fly(&mut sim, h, Vec3::Y * 3.5, Vec3::NEG_Y * 3.0);
        assert_eq!(sim.vehicle(h).unwrap().flyer.phase, FlightPhase::Landing);
        sim.vehicle_mut(h).unwrap().flyer.lift = 0.0;
        fly(&mut sim, h, Vec3::Y * 3.5, Vec3::NEG_Y * 3.0);
        assert_eq!(sim.vehicle(h).unwrap().flyer.phase, FlightPhase::Landed);
    }

    #[test]
    fn a_falling_craft_is_destroyed_by_what_it_hits() {
        let (mut sim, h) = flying(floor(), true);
        sim.vehicle_mut(h).unwrap().flyer.phase = FlightPhase::Falling;
        fly(&mut sim, h, Vec3::Y * 3.5, Vec3::NEG_Y * 2.0);
        assert!(sim.vehicle(h).is_none_or(|v| !v.alive));
    }

    #[test]
    fn a_fast_ship_cannot_step_through_a_thin_wall() {
        let (mut sim, h) = flying(wall(), true);
        fly(&mut sim, h, Vec3::new(0.0, 0.0, 52.0), Vec3::Z * 170.0);
        assert!(sim.vehicle(h).is_none_or(|v| !v.alive));
    }

    #[test]
    fn a_jump_longer_than_the_step_limit_takes_no_contact() {
        let (mut sim, h) = flying(wall(), true);
        let row = FlyerRow::of(sim.flyer_params[0].as_ref().unwrap());
        let v = sim.vehicle_mut(h).unwrap();
        v.pos = Vec3::new(0.0, 0.0, 200.0);
        v.vel = Vec3::Z * 80.0;
        sim.flyer_contact(h.index as usize, h, Vec3::ZERO, row);
        assert_eq!(sim.vehicle(h).unwrap().health.cur, HEALTH);
    }

    #[test]
    fn bf1_has_no_nose_term() {
        let (mut sim, h) = flying(floor(), false);
        let vel = Vec3::new(0.0, -6.0, 80.0);
        sim.vehicle_mut(h).unwrap().rot = Quat::from_rotation_arc(Vec3::Z, vel.normalize());
        fly(&mut sim, h, Vec3::Y * 3.5, vel);
        assert_eq!(sim.vehicle(h).unwrap().health.cur, HEALTH);
    }

    #[test]
    fn terrain_stops_a_ship_at_its_lowest_part() {
        let (mut sim, h) = flying(Vec::new(), true);
        sim.heightfield = Some(crate::sim::tests::flat_ground());
        let ground = sim.heightfield.as_ref().unwrap().height_at(0.0, 0.0);
        fly(
            &mut sim,
            h,
            Vec3::new(0.0, ground + 3.5, 0.0),
            Vec3::new(0.0, 5.0, 80.0),
        );
        let v = sim.vehicle(h).unwrap();
        assert!((v.pos.y - (ground + 4.0)).abs() < 1e-3, "{}", v.pos.y);
    }

    #[test]
    fn parked_flyer_takes_no_damage_when_other_moves_away() {
        let (mut sim, h) = flying(Vec::new(), true);
        let parked_h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 0.0, 5.0),
            yaw: 0.0,
            max_health: HEALTH,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let parts = sim.hull_parts[0].as_ref().unwrap().clone();
        sim.collision.set_hull(
            parked_h,
            &parts,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0)),
        );

        fly(
            &mut sim,
            h,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -10.0),
        );
        assert_eq!(sim.vehicle(parked_h).unwrap().health.cur, HEALTH);
        assert_eq!(sim.vehicle(h).unwrap().health.cur, HEALTH);
    }

    #[test]
    fn ramming_parked_flyer_at_speed_damages_parked_flyer() {
        let (mut sim, h) = flying(Vec::new(), true);
        let parked_h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0.0, 0.0, 5.0),
            yaw: 0.0,
            max_health: HEALTH,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let parts = sim.hull_parts[0].as_ref().unwrap().clone();
        sim.collision.set_hull(
            parked_h,
            &parts,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0)),
        );

        fly(
            &mut sim,
            h,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 80.0),
        );
        assert!(sim.vehicle(parked_h).unwrap().health.cur < HEALTH);
    }
}
