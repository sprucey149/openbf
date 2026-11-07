//! Flyer/world integration. Flight equations are in vehicle::flyer; this
//! adapter supplies terrain/building rays without inventing a floor in space.

use super::Sim;
use crate::collision::{HullFilter, Layers};
use crate::vehicle::flyer::{self, FlightPhase, FlyerEnvironment, LandingSurface};
use crate::vehicle::{SeatKind, VehicleHandle, VehicleKind};
use glam::Vec3;

const PILOT_REPAIR_MIN_HEALTH: f32 = 1400.0;

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
            Layers::VEHICLE,
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
        self.fly_flyers(dt);
        self.tick_flyer_edges(dt);
    }

    /// Is `h` counting down outside the boundary: its flown droid, or the flyer it
    /// rides? (The HUD's line and the commander's, `re-boundary.md` 5.)
    pub fn is_leaving(&self, h: crate::unit::UnitHandle) -> bool {
        self.controlled_droid(h)
            .is_some_and(|d| d.leaving.is_some())
            || self
                .unit(h)
                .and_then(|u| u.mount)
                .and_then(|m| self.vehicle(m.vehicle))
                .is_some_and(|v| v.flyer.leaving.is_some())
    }

    /// The map's edge for a flyer (`re-boundary.md` T1, T2, T3b): one with a crew,
    /// outside the 2-D boundary, starts the countdown (BF1 10 s, BF2 6.5 s) and
    /// spends this update's `dt` of it, and is destroyed with its crew the update it
    /// goes below 0; back inside clears it. BF2 counts a human's craft above or below
    /// `MaxPlayerFlyHeight` / `MinPlayerFlyHeight` as outside too, in the flying state
    /// only: gunships park over the ceiling on bf2_uta1, bf2_myg1 and bf2_kam1. The cheat spares a
    /// human, as it does on foot. PROVISIONAL: the PS2 keeps a timer per crew member;
    /// here the craft holds one. Soldiers on foot are not tested yet (card BNDY).
    fn tick_flyer_edges(&mut self, dt: f32) {
        let time = crate::boundary::leaving_time(self.ground_rules);
        let mut gone = Vec::new();
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive || v.kind != VehicleKind::Flyer {
                continue;
            }
            let [floor, ceiling] = self.flight_limits.player;
            let band = v.input.human
                && v.flyer.phase == FlightPhase::Flying
                && self
                    .flyer_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|p| p.bf2)
                && (v.pos.y < floor || v.pos.y > ceiling);
            let outside = !v.is_empty() && (band || self.boundary.outside(v.pos.x, v.pos.z));
            let human = v.input.human;
            let handle = VehicleHandle {
                index: i as u32,
                gen: v.gen,
            };
            let left = outside.then(|| v.flyer.leaving.unwrap_or(time) - dt);
            self.vehicles[i].flyer.leaving = left;
            if left.is_some_and(|t| t < 0.0) && !(human && self.cheats.invincible) {
                gone.push(handle);
            }
        }
        for h in gone {
            self.destroy_vehicle(h, None);
        }
    }

    fn fly_flyers(&mut self, dt: f32) {
        for i in 0..self.vehicles.len() {
            let v = &self.vehicles[i];
            if !v.alive || v.kind != VehicleKind::Flyer {
                continue;
            }
            let handle = VehicleHandle {
                index: i as u32,
                gen: v.gen,
            };
            if v.flyer.phase == FlightPhase::Crashed {
                let killer = v.flyer.damager;
                self.destroy_vehicle(handle, killer);
                continue;
            }
            if v.flyer.ai.wedges >= crate::ai::flyer_drive::WEDGED_GIVEUP
                && v.flyer.phase != FlightPhase::Landed
            {
                self.destroy_vehicle(handle, None);
                continue;
            }
            self.pilot_repair(i, dt);
            self.start_side_roll(i);
            let v = &self.vehicles[i];
            let crashing = v.flyer.phase == FlightPhase::Falling;
            let crash_random = if crashing {
                [
                    self.lcg.unit(),
                    self.lcg.unit(),
                    self.lcg.unit(),
                    self.lcg.unit(),
                ]
            } else {
                [0.0; 4]
            };
            let v = &self.vehicles[i];
            let player_aboard = v.seats.iter().any(|s| {
                s.occupant
                    .and_then(|o| self.unit(o))
                    .is_some_and(|u| u.is_human())
            });
            let Some(p) = self
                .flyer_params
                .get(v.class as usize)
                .and_then(Option::as_ref)
            else {
                continue;
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
                player_aboard,
                crash_random,
            };
            let row = super::flyer_contact::FlyerRow::of(p);
            if p.bf2
                && !v.input.human
                && v.flyer.phase == FlightPhase::Flying
                && v.flyer.ai.curve.is_some()
            {
                let time = self.tick as f32 * dt;
                let ground_map = self.ground_flyer_map;
                let p = p.clone();
                flyer::spline_step(&mut self.vehicles[i], &p, ground_map, time, dt);
                self.flyer_contact(i, handle, from, row);
                continue;
            }
            flyer::step(&mut self.vehicles[i], p, env, dt);
            self.flyer_contact(i, handle, from, row);
        }
    }

    fn start_side_roll(&mut self, i: usize) {
        let v = &mut self.vehicles[i];
        let Some(side) = v.flyer.ai.trick.take() else {
            return;
        };
        let Some(p) = self
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
        else {
            return;
        };
        let was = v.flyer.in_roll;
        flyer::side_roll(&mut v.flyer, p, side);
        if !was && v.flyer.in_roll {
            if let Some(h) = v.seats.iter().find_map(|s| s.occupant) {
                let floor = self
                    .unit(h)
                    .map_or(0.0, |u| self.enemy_skill_of(u.team).err_floor);
                if let Some(u) = self.units.get_mut(h.index as usize) {
                    u.aim.trick(floor);
                }
            }
        }
    }

    fn pilot_repair(&mut self, i: usize, dt: f32) {
        let v = &self.vehicles[i];
        let Some(p) = self
            .flyer_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
        else {
            return;
        };
        if !p.bf2
            || v.health.max <= PILOT_REPAIR_MIN_HEALTH
            || v.health.cur <= 0.0
            || v.health.cur >= v.health.max
            || v.flyer.phase == FlightPhase::Falling
        {
            return;
        }
        let piloted = v
            .seats
            .first()
            .filter(|s| s.kind == SeatKind::Pilot)
            .and_then(|s| s.occupant)
            .and_then(|o| self.unit(o))
            .is_some_and(|u| u.spec.consts.pilot_skill);
        if !piloted {
            return;
        }
        let rate = v.health.max * p.pilot_repair_scale;
        let health = &mut self.vehicles[i].health;
        health.cur = (health.cur + rate * dt).min(health.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{buttons, Event, PadState};
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

    /// BF1 Bespin's kill floor is 24 m under its decks and its mission lets a
    /// pilot fly to y -200. He flies under it and lives; the craft he leaves
    /// there is falling, and the floor takes it.
    #[test]
    fn a_crewed_flyer_flies_under_the_kill_floor_and_an_empty_one_does_not() {
        let (mut sim, h) = world();
        sim.kill_floor = Some(50.0);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        let v = sim.vehicle_mut(h).unwrap();
        v.pos.y = 30.0;
        v.flyer.phase = FlightPhase::Flying;
        for _ in 0..10 {
            sim.step(&[PadState::default()]);
        }
        assert!(sim.vehicle(h).is_some(), "the crewed craft is flying");
        assert!(sim.unit(pilot).is_some_and(|u| u.alive), "its pilot lives");
        sim.unseat_unit(pilot);
        sim.step(&[PadState::default()]);
        assert!(sim.vehicle(h).is_none(), "the empty craft is falling");
        assert!(sim.unit(pilot).is_none_or(|u| !u.alive), "and he is out");
    }

    /// A gunner left alone is not flying the craft: with no ground under it (BF1
    /// Bespin) it falls until the floor takes it, or its pad makes no other.
    #[test]
    fn a_craft_with_only_a_gunner_falls_to_the_kill_floor() {
        let (mut sim, h) = world();
        sim.heightfield = None;
        sim.kill_floor = Some(50.0);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        let v = sim.vehicle_mut(h).unwrap();
        v.seats.push(crate::vehicle::Seat {
            kind: crate::vehicle::SeatKind::Gunner(0),
            occupant: None,
        });
        v.pos.y = 30.0;
        v.flyer.phase = FlightPhase::Flying;
        let gunner = sim
            .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 30.0, 0.0)))
            .unwrap();
        assert!(sim.seat_unit(gunner, h, 1));
        for _ in 0..30 {
            sim.step(&[PadState::default()]);
        }
        assert!(sim.vehicle(h).is_none(), "nobody is flying it");
    }

    fn bf2_world(has_chunks: bool) -> (Sim, VehicleHandle) {
        let (mut sim, h) = world();
        let p = sim.flyer_params[0].as_mut().unwrap();
        p.bf2 = true;
        p.has_chunks = has_chunks;
        p.trick_roll_speed = 6.0;
        p.level_filter = 1.5;
        let v = sim.vehicle_mut(h).unwrap();
        v.pos = Vec3::new(0.0, 80.0, 0.0);
        v.vel = Vec3::new(0.0, 0.0, 50.0);
        v.flyer.lift = 1.0;
        v.flyer.initialized = true;
        v.flyer.phase = FlightPhase::Flying;
        (sim, h)
    }

    fn lethal() -> crate::weapon::DamageDesc {
        crate::vehicle::collision_desc(1.0e6)
    }

    #[test]
    fn a_bf2_ship_out_of_hull_tumbles_then_blows_up_and_pays_its_killer() {
        let (mut sim, h) = bf2_world(true);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        let killer = sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(50.0, 1.0, 0.0)))
            .unwrap();
        sim.damage_vehicle(h, &lethal(), 1.0, Some((killer, 1)));
        let v = sim.vehicle(h).expect("it does not vanish at zero hull");
        assert_eq!(v.flyer.phase, FlightPhase::Falling);
        assert!(!v.health.alive());
        assert!(
            (0.2..2.2).contains(&v.flyer.crash_timer),
            "{}",
            v.flyer.crash_timer
        );
        assert!(
            sim.unit(pilot).is_some_and(|u| u.alive),
            "aboard until the end"
        );
        let forward = v.rot * Vec3::Z;
        let mut spun = 0.0f32;
        let mut seen = None;
        for t in 0..(3 * crate::TICK_HZ) {
            sim.step(&[PadState::default()]);
            for e in &sim.events {
                if let Event::VehicleDestroyed {
                    vehicle, killer, ..
                } = e
                {
                    assert_eq!(*vehicle, h);
                    seen = Some((t, *killer));
                }
            }
            if let Some(v) = sim.vehicle(h) {
                spun = spun.max(1.0 - (v.rot * Vec3::Z).dot(forward));
            }
            if seen.is_some() {
                break;
            }
        }
        let (t, paid) = seen.expect("it blows up on its timer");
        assert!(t as f32 * crate::TICK_DT <= 2.3, "{t}");
        assert_eq!(paid, Some(killer));
        assert!(spun > 0.0, "it tumbles");
        assert!(
            sim.unit(pilot).is_none_or(|u| !u.alive),
            "the crew goes with it"
        );
    }

    #[test]
    fn a_bf2_ship_without_chunks_waits_for_contact() {
        let (mut sim, h) = bf2_world(false);
        sim.damage_vehicle(h, &lethal(), 1.0, None);
        assert_eq!(sim.vehicle(h).unwrap().flyer.crash_timer, 600.0);
    }

    #[test]
    fn a_landed_bf2_ship_blows_up_where_it_stands() {
        let (mut sim, h) = bf2_world(true);
        sim.vehicle_mut(h).unwrap().flyer.phase = FlightPhase::Landed;
        sim.damage_vehicle(h, &lethal(), 1.0, None);
        assert!(sim.vehicle(h).is_none());
    }

    #[test]
    fn a_bf2_ship_left_empty_blows_up_half_a_second_later() {
        let (mut sim, h) = bf2_world(true);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        sim.step(&[PadState::default()]);
        let v = sim.vehicle(h).unwrap();
        assert_eq!(v.flyer.phase, FlightPhase::Falling);
        assert!(!v.health.alive());
        for _ in 0..(crate::TICK_HZ * 3 / 4) {
            sim.step(&[PadState::default()]);
        }
        assert!(sim.vehicle(h).is_none());
    }

    #[test]
    fn never_crash_keeps_an_empty_bf2_ship_flying() {
        let (mut sim, h) = bf2_world(true);
        sim.flyer_params[0].as_mut().unwrap().never_crash_when_unpiloted = true;
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        for _ in 0..crate::TICK_HZ {
            sim.step(&[PadState::default()]);
        }
        let v = sim.vehicle(h).unwrap();
        assert!(v.health.alive());
        assert_eq!(v.flyer.phase, FlightPhase::Flying);
    }

    #[test]
    fn a_crashing_ship_keeps_rolling_past_the_wrap() {
        let (mut sim, h) = bf2_world(true);
        sim.damage_vehicle(h, &lethal(), 1.0, None);
        sim.vehicle_mut(h).unwrap().flyer.crash_timer = 100.0;
        let mut wrapped = false;
        let mut last = 0.0f32;
        for _ in 0..(2 * crate::TICK_HZ) {
            sim.step(&[PadState::default()]);
            let Some(v) = sim.vehicle(h) else {
                break;
            };
            wrapped |= (v.flyer.crash_roll - last).abs() > std::f32::consts::PI;
            last = v.flyer.crash_roll;
            assert!(v.flyer.crash_roll.abs() <= std::f32::consts::PI);
        }
        assert!(wrapped, "6 rad/s turns past pi inside two seconds");
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
    fn an_empty_craft_is_destroyed_where_it_comes_down() {
        let (mut sim, h) = abandoned();
        for _ in 0..(60 * crate::TICK_HZ) {
            sim.step(&[PadState::default()]);
            match sim.vehicle(h).filter(|v| v.alive) {
                Some(v) => assert!(v.pos.y >= 0.0, "tick {}: {:?}", sim.tick, v.pos),
                None => return,
            }
        }
        panic!("still falling after a minute");
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

    fn over_the_line(bf2: bool) -> (Sim, VehicleHandle) {
        let (mut sim, h) = world();
        sim.flyer_params[0].as_mut().unwrap().bf2 = bf2;
        sim.ground_rules = if bf2 {
            crate::soldier::GroundRules::Bf2
        } else {
            crate::soldier::GroundRules::Bf1
        };
        sim.boundary
            .add_shape(&[[-50.0, -50.0], [-50.0, 50.0], [50.0, 50.0], [50.0, -50.0]]);
        sim.vehicle_mut(h).unwrap().pos.x = 80.0;
        (sim, h)
    }

    /// The whole countdown from outside: BF1 10 s, BF2 6.5 s, and its crew goes with it.
    /// The PS2 took 300 and 196 updates (`re-boundary.md` M2).
    #[test]
    fn a_flyer_outside_the_boundary_is_destroyed_at_the_games_time() {
        for (bf2, seconds) in [(false, 10.0), (true, 6.5)] {
            let (mut sim, h) = over_the_line(bf2);
            let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
            let ticks = (seconds * crate::TICK_HZ as f32) as u64;
            for t in 0..ticks - 1 {
                sim.step(&[PadState::default()]);
                assert!(sim.vehicle(h).is_some(), "bf2 {bf2} tick {t}");
                assert!(sim.is_leaving(pilot), "bf2 {bf2} tick {t}");
            }
            sim.step(&[PadState::default()]);
            sim.step(&[PadState::default()]);
            assert!(sim.vehicle(h).is_none(), "bf2 {bf2}");
            assert!(sim.unit(pilot).is_none_or(|u| !u.alive));
        }
    }

    #[test]
    fn coming_back_inside_starts_the_next_countdown_from_full() {
        let (mut sim, h) = over_the_line(true);
        for _ in 0..90 {
            sim.step(&[PadState::default()]);
        }
        let left = sim.vehicle(h).unwrap().flyer.leaving.unwrap();
        assert!(left < 4.0);
        sim.vehicle_mut(h).unwrap().pos.x = 0.0;
        sim.step(&[PadState::default()]);
        assert_eq!(sim.vehicle(h).unwrap().flyer.leaving, None);
        sim.vehicle_mut(h).unwrap().pos.x = 80.0;
        sim.step(&[PadState::default()]);
        assert!(sim.vehicle(h).unwrap().flyer.leaving.unwrap() > 6.4);
    }

    #[test]
    fn an_empty_flyer_is_not_tested_and_the_cheat_spares_a_human() {
        let (mut sim, h) = over_the_line(true);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unseat_unit(pilot);
        sim.step(&[PadState::default()]);
        assert_eq!(sim.vehicle(h).unwrap().flyer.leaving, None);

        let (mut sim, h) = over_the_line(true);
        sim.cheats.invincible = true;
        for _ in 0..300 {
            sim.step(&[PadState::default()]);
        }
        assert!(sim.vehicle(h).is_some());
    }

    /// BF2's height band counts as outside for a human's craft in flight, inside the
    /// 2-D shape.
    #[test]
    fn bf2s_ceiling_counts_a_humans_craft_as_outside() {
        for (bf2, counted) in [(true, true), (false, false)] {
            let (mut sim, h) = over_the_line(bf2);
            sim.flight_limits.player = [-200.0, 20.0];
            let v = sim.vehicle_mut(h).unwrap();
            v.pos = Vec3::new(0.0, 40.0, 0.0);
            v.flyer.phase = FlightPhase::Flying;
            sim.step(&[PadState::default()]);
            let v = sim.vehicle(h).unwrap();
            assert_eq!(v.flyer.leaving.is_some(), counted, "bf2 {bf2}");
        }
    }

    /// bf2_uta1's gunships park at y 83 under a 29.5 ceiling. The PS2 sets its height
    /// flag in the flying state's case only (B2:118249, B2:118385), so boarding one is
    /// not a countdown: a human who sits in it on the pad keeps his craft.
    #[test]
    fn a_craft_parked_over_bf2s_ceiling_is_not_counting_down() {
        let (mut sim, h) = over_the_line(true);
        sim.flight_limits.player = [-200.0, 29.5];
        sim.vehicle_mut(h).unwrap().pos = Vec3::new(0.0, 83.0, 0.0);
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        for t in 0..300 {
            sim.step(&[PadState::default()]);
            assert_eq!(sim.vehicle(h).unwrap().flyer.leaving, None, "tick {t}");
            assert!(!sim.is_leaving(pilot), "tick {t}");
        }
        assert_eq!(sim.vehicle(h).unwrap().flyer.phase, FlightPhase::Landed);
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
    /// A craft whose model starts over its origin (BF1's TIE bomber, -0.0115) lifts
    /// off and touches down again, and so does one under the sweep's 1 cm (the TIE
    /// fighter's 0.006). Before this the first never rose and neither could land.
    #[test]
    fn a_craft_with_almost_no_landed_height_takes_off_and_lands() {
        for landed in [-0.0115, 0.006] {
            let (mut sim, h) = world();
            sim.flyer_params[0].as_mut().unwrap().landed_height = landed;
            let v = sim.vehicle_mut(h).unwrap();
            v.pos.y = crate::vehicle::flyer::rest_height(landed);
            v.flyer.phase = FlightPhase::Landed;
            sim.step(&[PadState::default()]);
            sim.step(&[PadState {
                buttons: buttons::JUMP,
                ..PadState::default()
            }]);
            for _ in 0..120 {
                sim.step(&[PadState::default()]);
            }
            let v = sim.vehicle(h).unwrap();
            assert_eq!(v.flyer.phase, FlightPhase::Flying, "landed {landed}");
            assert!(v.pos.y > 2.0, "landed {landed}: y {}", v.pos.y);
            sim.step(&[PadState {
                buttons: buttons::JUMP,
                ..PadState::default()
            }]);
            for _ in 0..1800 {
                sim.step(&[PadState::default()]);
                if sim.vehicle(h).unwrap().flyer.phase == FlightPhase::Landed {
                    break;
                }
            }
            let v = sim.vehicle(h).unwrap();
            assert_eq!(
                v.flyer.phase,
                FlightPhase::Landed,
                "landed {landed} at {:?}",
                v.pos
            );
        }
    }

    fn repair_world(max: f32, skill: bool) -> (Sim, usize) {
        let (mut sim, h) = world();
        sim.flyer_params[0].as_mut().unwrap().bf2 = true;
        sim.flyer_params[0].as_mut().unwrap().pilot_repair_scale = 0.005;
        let pilot = sim.vehicle(h).unwrap().seats[0].occupant.unwrap();
        sim.unit_mut(pilot).unwrap().spec.consts.pilot_skill = skill;
        let v = sim.vehicle_mut(h).unwrap();
        v.health.max = max;
        v.health.cur = max / 2.0;
        (sim, h.index as usize)
    }

    #[test]
    fn a_pilot_mends_a_big_ship_at_the_class_rate() {
        let (mut sim, i) = repair_world(2000.0, true);
        sim.pilot_repair(i, 1.0);
        assert_eq!(sim.vehicles[i].health.cur, 1010.0);
    }

    #[test]
    fn a_soldier_or_a_small_ship_gets_no_pilot_repair() {
        let (mut sim, i) = repair_world(2000.0, false);
        sim.pilot_repair(i, 1.0);
        assert_eq!(sim.vehicles[i].health.cur, 1000.0);
        let (mut sim, i) = repair_world(PILOT_REPAIR_MIN_HEALTH, true);
        sim.pilot_repair(i, 1.0);
        assert_eq!(sim.vehicles[i].health.cur, PILOT_REPAIR_MIN_HEALTH / 2.0);
    }
}
