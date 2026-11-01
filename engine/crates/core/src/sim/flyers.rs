//! Flyer/world integration. Flight equations are in vehicle::flyer; this
//! adapter supplies terrain/building rays without inventing a floor in space.

use super::Sim;
use crate::collision::{HullFilter, Layers};
use crate::vehicle::flyer::{self, FlightPhase, FlyerEnvironment, LandingSurface};
use crate::vehicle::{VehicleHandle, VehicleKind};
use glam::Vec3;

/// Mission inputs, separate from the soldier ceiling. Both bands default to
/// −200..200 on PS2; the mission's four Set*FlyHeight calls replace them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightLimits {
    pub player: [f32; 2],
    pub ai: [f32; 2],
}

impl Default for FlightLimits {
    fn default() -> Self {
        Self {
            player: [-200.0, 200.0],
            ai: [-200.0, 200.0],
        }
    }
}

impl Sim {
    fn flyer_ground(&self, origin: Vec3, handle: VehicleHandle) -> Option<LandingSurface> {
        let mut surface = self.heightfield.as_ref().and_then(|hf| {
            let height = hf.height_at(origin.x, origin.z);
            ((0.0..=1024.0).contains(&(origin.y - height))
                && !self.collision.is_cut(Vec3::new(origin.x, height, origin.z)))
            .then(|| LandingSurface {
                height,
                normal: hf.normal_at(origin.x, origin.z).into(),
                can_land: true,
            })
        });
        if let Some(hit) = self.collision.ray_with(
            origin,
            Vec3::NEG_Y,
            1024.0,
            Layers::SOLDIER,
            HullFilter::except(Some(handle)),
        ) {
            let height = origin.y - hit.t;
            if surface.is_none_or(|s| height > s.height) {
                surface = Some(LandingSurface {
                    height,
                    normal: hit.normal,
                    can_land: true,
                });
            }
        }
        surface
    }

    pub(super) fn tick_flyers(&mut self, dt: f32) {
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive || v.kind != VehicleKind::Flyer {
                continue;
            }
            let Some(p) = self
                .flyer_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let handle = VehicleHandle {
                index: i as u32,
                gen: v.gen,
            };
            let from = v.pos;
            let [min_height, max_height] = if v.input.human {
                self.flight_limits.player
            } else {
                self.flight_limits.ai
            };
            let env = FlyerEnvironment {
                ground: self.flyer_ground(from, handle),
                min_height,
                max_height,
                // PROVISIONAL: origin membership; PS2 tests the flyer's sphere.
                in_landing_region: self.landing_regions.iter().any(|r| r.contains(from)),
                require_landing_region: !self.landing_regions.is_empty(),
            };
            let (threshold, scale) = (p.collision_threshold, p.collision_scale);
            // BF2's ordinary AI flight writes the pose along its curve in
            // the flying state (B2:117417). OURS: without the original's
            // avoidance and hull response it takes no contact here.
            // A human who took the seat flies it, whatever curve was left.
            if p.bf2
                && !v.input.human
                && v.flyer.phase == FlightPhase::Flying
                && v.flyer.ai.curve.is_some()
            {
                let time = self.tick as f32 * dt;
                let ground_map = self.ground_flyer_map;
                let p = p.clone();
                flyer::spline_step(&mut self.vehicles[i], &p, ground_map, time, dt);
                continue;
            }
            flyer::step(&mut self.vehicles[i], p, env, dt);
            self.flyer_contact(i, handle, from, threshold, scale);
        }
    }

    /// PROVISIONAL continuous centre sweep. Keeps a fast fighter from passing
    /// through a wall in one update; the original hull contact/torque solver
    /// remains unconfirmed. This is not claimed as PS2 collision parity.
    fn flyer_contact(
        &mut self,
        i: usize,
        handle: VehicleHandle,
        from: Vec3,
        threshold: [f32; 3],
        scale: [f32; 3],
    ) {
        let to = self.vehicles[i].pos;
        let delta = to - from;
        let distance = delta.length();
        if distance < 1e-6 {
            return;
        }
        let direction = delta / distance;
        let world = self
            .collision
            .ray_with(
                from,
                direction,
                distance,
                Layers::SOLDIER,
                HullFilter::except(Some(handle)),
            )
            .map(|h| (h.t, h.normal));
        let terrain = self.terrain_ray(from, direction, distance, 0.5).map(|t| {
            let p = from + direction * t;
            let n = self
                .heightfield
                .as_ref()
                .map_or(Vec3::Y, |hf| hf.normal_at(p.x, p.z).into());
            (t, n)
        });
        let hit = match (world, terrain) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (a, b) => a.or(b),
        };
        let Some((t, normal)) = hit else {
            return;
        };
        // OURS: what is left of the move slides along the surface, once, so
        // a craft in a hangar is not pinned to the first ceiling it touches.
        let at = from + direction * (t - 0.01).max(0.0);
        let rest = direction * (distance - t).max(0.0);
        let along = rest - normal * rest.dot(normal);
        let slid = along.length();
        let clear = slid > 1e-4
            && self
                .collision
                .ray_with(
                    at,
                    along / slid,
                    slid,
                    Layers::SOLDIER,
                    HullFilter::except(Some(handle)),
                )
                .is_none()
            && self.terrain_ray(at, along / slid, slid, 0.5).is_none();
        let v = &self.vehicles[i];
        let closing = (-v.vel.dot(normal)).max(0.0);
        let up = normal.dot(v.rot * Vec3::Y);
        let forward = normal.dot(v.rot * Vec3::Z);
        let at = self.over_sheet(from, at);
        let rest_at = if clear {
            self.over_sheet(from, at + along)
        } else {
            at
        };
        let v = &mut self.vehicles[i];
        v.pos = rest_at;
        v.vel += normal * closing;
        // OURS: an empty craft that came down rests where it hit. The PS2's
        // falling contact is not read, and its cruise term would otherwise
        // skate it along the ground at MidSpeed for the rest of the round.
        if v.flyer.phase == FlightPhase::Falling && v.is_empty() && normal.y > 0.7 {
            v.flyer.phase = FlightPhase::Landed;
            v.vel = Vec3::ZERO;
            v.flyer.omega = Vec3::ZERO;
        }
        self.impact_vehicle(handle, closing, up, forward, threshold, scale, 1.0);
    }

    /// A move that began over the terrain ends over it, where the sheet is not
    /// cut. `terrain_ray` brackets a crossing to half a 0.5 m step, so a shallow
    /// approach can be placed past it, and the sheet is two-sided: from below it
    /// no longer stops anything, and the craft sinks away under the map.
    fn over_sheet(&self, from: Vec3, p: Vec3) -> Vec3 {
        let Some(hf) = self.heightfield.as_ref() else {
            return p;
        };
        let height = hf.height_at(p.x, p.z);
        if from.y < hf.height_at(from.x, from.z)
            || p.y >= height
            || self.collision.is_cut(Vec3::new(p.x, height, p.z))
        {
            return p;
        }
        Vec3::new(p.x, height + 0.01, p.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{buttons, PadState};
    use crate::unit::UnitSpec;
    use crate::vehicle::flyer::FlyerParams;
    use crate::vehicle::{SeatKind, VehicleSpec};
    use crate::weapon::HealthType;

    fn world() -> (Sim, VehicleHandle) {
        let mut sim = Sim::empty(42);
        sim.heightfield = Some(crate::sim::tests::flat_ground());
        sim.flyer_params = vec![Some(FlyerParams {
            landed_height: 1.0,
            takeoff_height: 2.5,
            takeoff_time: 1.0,
            acceleration: 30.0,
            min_speed: 20.0,
            mid_speed: 50.0,
            max_speed: 82.0,
            ..FlyerParams::default()
        })];
        let pos = Vec3::new(0.0, 1.0, 0.0);
        let h = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: true,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let pilot = sim.spawn(&UnitSpec::player(0, pos)).unwrap();
        assert!(sim.seat_unit(pilot, h, 0));
        (sim, h)
    }

    #[test]
    fn player_pad_takes_off_and_replays_through_the_normal_tick() {
        let (mut a, h) = world();
        let (mut b, _) = world();
        for t in 0..300 {
            let pad = PadState {
                buttons: if t < 90 { buttons::JUMP } else { 0 },
                ly: if t > 150 { i16::MAX } else { 0 },
                ..PadState::default()
            };
            a.step(&[pad]);
            b.step(&[pad]);
            assert_eq!(a.hash(), b.hash(), "tick {t}");
        }
        let v = a.vehicle(h).unwrap();
        assert_eq!(v.flyer.phase, FlightPhase::Flying);
        assert!(v.pos.y > 3.5 && v.pos.z > 100.0);
        assert!((v.vel.length() - 82.0).abs() < 0.01);
        let pilot = v.seats[0].occupant.unwrap();
        assert_eq!(a.unit(pilot).unwrap().soldier.pos, v.pos);
    }

    /// 12 m up at cruise with its pilot gone: the long shallow glide a craft
    /// makes after its pilot leaves.
    fn abandoned() -> (Sim, VehicleHandle) {
        let (mut sim, h) = world();
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        let v = sim.vehicle_mut(h).unwrap();
        v.pos = Vec3::new(0.0, 12.0, 0.0);
        v.flyer.phase = FlightPhase::Falling;
        v.vel = Vec3::new(0.0, 0.0, 50.0);
        (sim, h)
    }

    #[test]
    fn a_craft_grazing_the_terrain_does_not_slip_under_the_sheet() {
        let (mut sim, h) = abandoned();
        for _ in 0..(60 * crate::TICK_HZ) {
            sim.step(&[PadState::default()]);
            let v = sim.vehicle(h).unwrap();
            assert!(v.pos.y >= 0.0, "tick {}: {:?}", sim.tick, v.pos);
        }
    }

    #[test]
    fn a_shallow_flying_descent_stays_over_the_terrain() {
        let (mut sim, h) = world();
        for t in 0..150 {
            sim.step(&[PadState {
                buttons: if t == 0 { buttons::JUMP } else { 0 },
                ..PadState::default()
            }]);
        }
        let v = sim.vehicle_mut(h).unwrap();
        assert_eq!(v.flyer.phase, FlightPhase::Flying);
        // 1 degree down, a hand above the ground: 2.7 m a tick, 5 cm down.
        v.rot = crate::vehicle::hover::quat_x(0.0175);
        v.pos = Vec3::new(0.0, 0.25, 0.0);
        v.vel = v.rot * Vec3::Z * 82.0;
        for _ in 0..30 {
            sim.step(&[PadState::default()]);
            let v = sim.vehicle(h).unwrap();
            assert!(v.pos.y >= 0.0, "tick {}: {:?}", sim.tick, v.pos);
        }
    }

    #[test]
    fn an_empty_craft_comes_to_rest_where_it_comes_down_and_can_be_flown_again() {
        let (mut sim, h) = abandoned();
        let mut rested = None;
        for _ in 0..(60 * crate::TICK_HZ) {
            sim.step(&[PadState::default()]);
            let v = sim.vehicle(h).unwrap();
            if v.flyer.phase == FlightPhase::Landed {
                rested = Some(v.pos);
                break;
            }
        }
        let at = rested.expect("still falling after a minute");
        for _ in 0..30 {
            sim.step(&[PadState::default()]);
        }
        let v = sim.vehicle(h).unwrap();
        assert_eq!(v.pos, at);
        assert_eq!(v.vel, Vec3::ZERO);
        let pilot = sim.spawn(&UnitSpec::player(0, at)).unwrap();
        assert!(sim.seat_unit(pilot, h, 0));
        sim.step(&[PadState {
            buttons: buttons::JUMP,
            ..PadState::default()
        }]);
        assert_eq!(sim.vehicle(h).unwrap().flyer.phase, FlightPhase::Takeoff);
    }

    #[test]
    fn a_mission_with_its_floor_over_its_ceiling_does_not_panic() {
        let (mut sim, h) = world();
        sim.flyer_params[0].as_mut().unwrap().bf2 = true;
        sim.flight_limits.player = [120.0, 40.0];
        for t in 0..150 {
            sim.step(&[PadState {
                buttons: if t == 0 { buttons::JUMP } else { 0 },
                ..PadState::default()
            }]);
        }
        assert!(sim.vehicle(h).unwrap().pos.y <= 40.2);
    }

    #[test]
    fn bf2_canenter_refuses_a_flyer_in_flight_only() {
        for (rules, phase, open) in [
            (crate::soldier::GroundRules::Bf2, FlightPhase::Flying, false),
            (crate::soldier::GroundRules::Bf2, FlightPhase::Landed, true),
            (crate::soldier::GroundRules::Bf1, FlightPhase::Flying, true),
        ] {
            let (mut sim, h) = world();
            let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
            sim.unseat_unit(pilot);
            let other = sim
                .spawn(&UnitSpec::player(0, Vec3::new(1.0, 1.0, 0.0)))
                .unwrap();
            sim.seat_setup = vec![Some(crate::vehicle::SeatSetup {
                radius: 3.0,
                boardable: vec![true],
                body_active: vec![true],
                exit_drop: 15.0,
                ai_size: 0x20,
                guns: Vec::new(),
            })];
            sim.ground_rules = rules;
            sim.vehicle_mut(h).unwrap().flyer.phase = phase;
            assert_eq!(
                sim.enter_seat(other, h).is_some(),
                open,
                "{rules:?} {phase:?}"
            );
        }
    }

    #[test]
    fn mouse_pitch_and_unseating_reach_the_flight_state() {
        let (mut sim, h) = world();
        for t in 0..180 {
            sim.step(&[PadState {
                buttons: if t == 0 { buttons::JUMP } else { 0 },
                ..PadState::default()
            }]);
        }
        let before = sim.vehicle(h).unwrap().rot;
        sim.step(&[PadState {
            my: PadState::quantize_look(0.03),
            ..PadState::default()
        }]);
        let v = sim.vehicle(h).unwrap();
        assert!(v.input.pitch.abs() > 0.9);
        assert!(v.rot.angle_between(before) > 0.0001);
        let pilot = v.seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        sim.step(&[PadState::default()]);
        let v = sim.vehicle(h).unwrap();
        assert_eq!(v.flyer.phase, FlightPhase::Falling);
        assert_eq!(v.input, crate::vehicle::hover::HoverInput::default());
    }
}
