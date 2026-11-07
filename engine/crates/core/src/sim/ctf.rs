//! Deterministic opt-in CTF and Space One-Flag simulation rules.
//!
//! Evidence is the PS2 BF2 executable listing (`SWBFRNT2.ELF-decomp.c` `FUN_0035d050`,
//! `FUN_0035d2f8`, `FUN_0035d7c8`, `FUN_0035e058`, `FUN_0035e370`), mission scripts
//! (`objectivectf.luac`, `objectiveoneflagctf.luac`, `spa1g_1flag.luac`), shell options
//! (`ifs_instant_options.luac`), and baked levels (`SPA1.LVL`..`SPA9.LVL`, `INGAME.LVL`).
//!
//! In One-Flag CTF (used on all six space maps `spa1`..`spa9` and nine ground maps):
//! - A single neutral flag spawns at `home_pos`.
//! - Either team can pick it up (on foot or while piloting a flyer in space).
//! - When carried, the flag tracks the carrier (or their vehicle when mounted).
//! - Carrier scores by delivering the flag to the enemy team's capture region.
//! - First team to reach `score_limit` (default 5) wins the round.
//!
//! In Two-Flag CTF (used on nine ground maps):
//! - Each team has a home flag at their base.
//! - Enemy flag carried to home capture region scores 1 point.
//! - Friendly unit touching dropped team flag immediately returns it to home (Touch Return).

use glam::Vec3;

use super::state::{Event, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::flyer::FlightPhase;

/// Default score limit in session options (SWBFRNT2.ELF FUN_0019e020 line 43828).
pub const DEFAULT_CTF_SCORE_LIMIT: u32 = 5;

/// Dropped space flag return timeout in seconds (com_icon_spaceflag ODF Lifespan).
pub const SPACE_FLAG_RETURN_TIMEOUT: f32 = 15.0;

/// Dropped ground flag return timeout in seconds (SWBFRNT2.ELF FUN_0035d2f8 line 343281: 60.0f).
pub const GROUND_FLAG_RETURN_TIMEOUT: f32 = 60.0;

/// Re-pickup delay after drop in seconds (SWBFRNT2.ELF FUN_0035e370 line 343802: 0.5s).
pub const FLAG_REPICKUP_DELAY: f32 = 0.5;

/// Soldier interaction pickup radius in metres (SWBFRNT2.ELF FUN_0035e370 trigger).
pub const SOLDIER_PICKUP_RADIUS: f32 = 2.5;

/// Flyer interaction pickup sphere radius in metres (SPA1.LVL com_icon_spaceflag prim).
pub const FLYER_PICKUP_RADIUS: f32 = 20.0;

/// Flag attachment offset when carried in space (com_icon_spaceflag ODF CarriedOffset).
pub const SPACE_CARRIED_OFFSET: Vec3 = Vec3::new(0.0, 1.5, -1.5);

/// Flag attachment offset when carried on foot (com_item_flag ODF CarriedOffset).
pub const GROUND_CARRIED_OFFSET: Vec3 = Vec3::new(0.0, 1.5, -0.5);

/// Default capture region trigger radius in metres.
/// PROVISIONAL: trace: capture zone volume / measure: space hangar trigger sphere / replace: authored regn extent
pub const DEFAULT_CAPTURE_REGION_RADIUS: f32 = 25.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtfMode {
    /// 1-flag CTF: neutral flag carried to enemy capture zone.
    OneFlag,
    /// 2-flag CTF: enemy flag carried to own capture zone.
    TwoFlag,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlagStatus {
    /// At designated home spawn position.
    AtHome,
    /// Carried by a soldier or pilot.
    Carried { carrier: UnitHandle },
    /// Dropped in space or on ground.
    Dropped {
        timer: f32,
        pickup_cooldown: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flag {
    /// Team affiliation: 0 for neutral flag in 1-flag, or 0 (team 1 / att) and 1 (team 2 / def) in 2-flag.
    pub team: u8,
    pub home_pos: Vec3,
    pub current_pos: Vec3,
    pub status: FlagStatus,
    pub soldier_pickup_radius: f32,
    pub flyer_pickup_radius: f32,
    pub drop_timeout: f32,
    pub carried_offset: Vec3,
}

impl Flag {
    pub fn new_space_neutral(home_pos: Vec3) -> Self {
        Self {
            team: 0,
            home_pos,
            current_pos: home_pos,
            status: FlagStatus::AtHome,
            soldier_pickup_radius: SOLDIER_PICKUP_RADIUS,
            flyer_pickup_radius: FLYER_PICKUP_RADIUS,
            drop_timeout: SPACE_FLAG_RETURN_TIMEOUT,
            carried_offset: SPACE_CARRIED_OFFSET,
        }
    }

    pub fn new_ground_neutral(home_pos: Vec3) -> Self {
        Self {
            team: 0,
            home_pos,
            current_pos: home_pos,
            status: FlagStatus::AtHome,
            soldier_pickup_radius: SOLDIER_PICKUP_RADIUS,
            flyer_pickup_radius: FLYER_PICKUP_RADIUS,
            drop_timeout: GROUND_FLAG_RETURN_TIMEOUT,
            carried_offset: GROUND_CARRIED_OFFSET,
        }
    }

    pub fn new_team_flag(team: u8, home_pos: Vec3, drop_timeout: f32) -> Self {
        Self {
            team,
            home_pos,
            current_pos: home_pos,
            status: FlagStatus::AtHome,
            soldier_pickup_radius: SOLDIER_PICKUP_RADIUS,
            flyer_pickup_radius: FLYER_PICKUP_RADIUS,
            drop_timeout,
            carried_offset: GROUND_CARRIED_OFFSET,
        }
    }

    pub fn is_at_home(&self) -> bool {
        matches!(self.status, FlagStatus::AtHome)
    }

    pub fn carrier(&self) -> Option<UnitHandle> {
        match self.status {
            FlagStatus::Carried { carrier } => Some(carrier),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureRegion {
    pub team: u8,
    pub pos: Vec3,
    pub radius: f32,
}

impl CaptureRegion {
    pub fn new(team: u8, pos: Vec3, radius: f32) -> Self {
        Self { team, pos, radius }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        (self.pos - p).length_squared() <= self.radius * self.radius
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CtfState {
    pub mode: CtfMode,
    pub score_limit: u32,
    pub team_points: [u32; 2],
    pub flags: Vec<Flag>,
    /// Scoring zones for each team: capture_regions[t] is the region team `t` must enter to score.
    pub capture_regions: [CaptureRegion; 2],
}

impl CtfState {
    /// Constructs a Space One-Flag state with neutral flag in space and team hangar capture goals.
    pub fn new_space_one_flag(
        score_limit: u32,
        home_pos: Vec3,
        team0_goal: Vec3,
        team1_goal: Vec3,
    ) -> Self {
        Self {
            mode: CtfMode::OneFlag,
            score_limit,
            team_points: [0, 0],
            flags: vec![Flag::new_space_neutral(home_pos)],
            capture_regions: [
                CaptureRegion::new(0, team0_goal, DEFAULT_CAPTURE_REGION_RADIUS),
                CaptureRegion::new(1, team1_goal, DEFAULT_CAPTURE_REGION_RADIUS),
            ],
        }
    }

    /// Constructs a Ground One-Flag state.
    pub fn new_ground_one_flag(
        score_limit: u32,
        home_pos: Vec3,
        team0_goal: Vec3,
        team1_goal: Vec3,
    ) -> Self {
        Self {
            mode: CtfMode::OneFlag,
            score_limit,
            team_points: [0, 0],
            flags: vec![Flag::new_ground_neutral(home_pos)],
            capture_regions: [
                CaptureRegion::new(0, team0_goal, DEFAULT_CAPTURE_REGION_RADIUS),
                CaptureRegion::new(1, team1_goal, DEFAULT_CAPTURE_REGION_RADIUS),
            ],
        }
    }

    /// Constructs a Two-Flag CTF state.
    pub fn new_two_flag(
        score_limit: u32,
        team0_home: Vec3,
        team0_goal: Vec3,
        team1_home: Vec3,
        team1_goal: Vec3,
    ) -> Self {
        Self {
            mode: CtfMode::TwoFlag,
            score_limit,
            team_points: [0, 0],
            flags: vec![
                Flag::new_team_flag(0, team0_home, GROUND_FLAG_RETURN_TIMEOUT),
                Flag::new_team_flag(1, team1_home, GROUND_FLAG_RETURN_TIMEOUT),
            ],
            capture_regions: [
                CaptureRegion::new(0, team0_goal, DEFAULT_CAPTURE_REGION_RADIUS),
                CaptureRegion::new(1, team1_goal, DEFAULT_CAPTURE_REGION_RADIUS),
            ],
        }
    }

    /// Bit-exact hashing of CTF simulation state.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(&[match self.mode {
            CtfMode::OneFlag => 1,
            CtfMode::TwoFlag => 2,
        }]);
        eat(&self.score_limit.to_le_bytes());
        for p in self.team_points {
            eat(&p.to_le_bytes());
        }
        eat(&(self.flags.len() as u32).to_le_bytes());
        for f in &self.flags {
            eat(&[f.team]);
            for coord in f.current_pos.to_array() {
                eat(&coord.to_bits().to_le_bytes());
            }
            match f.status {
                FlagStatus::AtHome => eat(&[0]),
                FlagStatus::Carried { carrier } => {
                    eat(&[1]);
                    eat(&carrier.index.to_le_bytes());
                    eat(&carrier.gen.to_le_bytes());
                }
                FlagStatus::Dropped {
                    timer,
                    pickup_cooldown,
                } => {
                    eat(&[2]);
                    eat(&timer.to_bits().to_le_bytes());
                    eat(&pickup_cooldown.to_bits().to_le_bytes());
                }
            }
        }
    }
}

impl Sim {
    /// Opt-in installation of CTF state onto the deterministic Sim.
    pub fn install_ctf(&mut self, ctf: CtfState) {
        self.ctf = Some(ctf);
        self.battle.unlimited_reinforcements = true;
    }

    /// Ticks the CTF / One-Flag simulation rules for fixed step dt.
    pub(super) fn tick_ctf(&mut self, dt: f32) {
        let Some(mut ctf) = self.ctf.take() else {
            return;
        };

        let num_flags = ctf.flags.len();
        for flag_idx in 0..num_flags {
            let mut flag = ctf.flags[flag_idx].clone();

            match flag.status {
                FlagStatus::AtHome => {
                    flag.current_pos = flag.home_pos;
                    let only_enemy = matches!(ctf.mode, CtfMode::TwoFlag);
                    if let Some(carrier) = self.ctf_find_pickup(&flag, only_enemy) {
                        flag.status = FlagStatus::Carried { carrier };
                        let (pos, _) = self.ctf_carrier_pos(carrier, &flag);
                        flag.current_pos = pos;
                        let team = self.unit(carrier).map_or(0, |u| u.team);
                        self.events.push(Event::FlagPickedUp {
                            flag: flag_idx as u8,
                            unit: carrier,
                            team,
                        });
                    }
                }
                FlagStatus::Carried { carrier } => {
                    let (carrier_pos, drop) = self.ctf_carrier_pos(carrier, &flag);

                    if drop {
                        flag.current_pos = carrier_pos;
                        flag.status = FlagStatus::Dropped {
                            timer: flag.drop_timeout,
                            pickup_cooldown: FLAG_REPICKUP_DELAY,
                        };
                        let team = self
                            .units
                            .get(carrier.index as usize)
                            .filter(|u| u.gen == carrier.gen)
                            .map_or(0, |u| u.team);
                        self.events.push(Event::FlagDropped {
                            flag: flag_idx as u8,
                            team,
                            pos: carrier_pos,
                        });
                    } else {
                        flag.current_pos = carrier_pos;
                        let carrier_team = self.unit(carrier).map_or(0, |u| u.team as usize);

                        let captured = match ctf.mode {
                            CtfMode::OneFlag => {
                                if carrier_team < ctf.capture_regions.len() {
                                    ctf.capture_regions[carrier_team].contains(flag.current_pos)
                                } else {
                                    false
                                }
                            }
                            CtfMode::TwoFlag => {
                                let enemy_flag = flag.team != carrier_team as u8;
                                enemy_flag
                                    && carrier_team < ctf.capture_regions.len()
                                    && ctf.capture_regions[carrier_team].contains(flag.current_pos)
                            }
                        };

                        if captured {
                            let scoring_team = carrier_team as u8;
                            ctf.team_points[carrier_team] += 1;
                            self.events.push(Event::FlagCaptured {
                                flag: flag_idx as u8,
                                team: scoring_team,
                            });

                            flag.status = FlagStatus::AtHome;
                            flag.current_pos = flag.home_pos;

                            if ctf.team_points[carrier_team] >= ctf.score_limit
                                && self.outcome.is_none()
                            {
                                self.outcome = Some(scoring_team);
                                self.events.push(Event::TeamDefeated {
                                    team: 1 - scoring_team,
                                });
                            }
                        }
                    }
                }
                FlagStatus::Dropped {
                    mut timer,
                    mut pickup_cooldown,
                } => {
                    pickup_cooldown = (pickup_cooldown - dt).max(0.0);
                    timer -= dt;

                    let out_of_bounds = self.kill_floor.is_some_and(|kf| flag.current_pos.y < kf)
                        || self
                            .death_regions
                            .iter()
                            .any(|r| r.contains(flag.current_pos));

                    if out_of_bounds || timer <= 0.0 {
                        flag.status = FlagStatus::AtHome;
                        flag.current_pos = flag.home_pos;
                        self.events.push(Event::FlagReset {
                            flag: flag_idx as u8,
                        });
                    } else {
                        flag.status = FlagStatus::Dropped {
                            timer,
                            pickup_cooldown,
                        };

                        if pickup_cooldown <= 0.0 {
                            if let Some(touching_unit) = self.ctf_find_pickup(&flag, false) {
                                let unit_team =
                                    self.unit(touching_unit).map_or(0, |u| u.team);

                                match ctf.mode {
                                    CtfMode::OneFlag => {
                                        flag.status = FlagStatus::Carried {
                                            carrier: touching_unit,
                                        };
                                        let (pos, _) = self.ctf_carrier_pos(touching_unit, &flag);
                                        flag.current_pos = pos;
                                        self.events.push(Event::FlagPickedUp {
                                            flag: flag_idx as u8,
                                            unit: touching_unit,
                                            team: unit_team,
                                        });
                                    }
                                    CtfMode::TwoFlag => {
                                        if unit_team == flag.team {
                                            flag.status = FlagStatus::AtHome;
                                            flag.current_pos = flag.home_pos;
                                            self.events.push(Event::FlagReturned {
                                                flag: flag_idx as u8,
                                                team: unit_team,
                                            });
                                        } else {
                                            flag.status = FlagStatus::Carried {
                                                carrier: touching_unit,
                                            };
                                            let (pos, _) = self.ctf_carrier_pos(touching_unit, &flag);
                                            flag.current_pos = pos;
                                            self.events.push(Event::FlagPickedUp {
                                                flag: flag_idx as u8,
                                                unit: touching_unit,
                                                team: unit_team,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ctf.flags[flag_idx] = flag;
        }

        self.ctf = Some(ctf);
    }

    fn ctf_carrier_pos(&self, carrier: UnitHandle, flag: &Flag) -> (Vec3, bool) {
        if let Some(u) = self
            .units
            .get(carrier.index as usize)
            .filter(|u| u.gen == carrier.gen)
        {
            if let Some(m) = u.mount {
                if let Some(v) = self.vehicles.get(m.vehicle.index as usize) {
                    let dead = !u.alive
                        || v.health.cur <= 0.0
                        || (v.kind == crate::vehicle::VehicleKind::Flyer
                            && v.flyer.phase == FlightPhase::Crashed);
                    if dead {
                        (v.pos, true)
                    } else {
                        (v.pos + flag.carried_offset, false)
                    }
                } else {
                    (u.soldier.pos, true)
                }
            } else if !u.alive {
                (u.soldier.pos, true)
            } else {
                (u.soldier.pos + flag.carried_offset, false)
            }
        } else {
            (flag.current_pos, true)
        }
    }

    fn ctf_find_pickup(&self, flag: &Flag, only_enemy: bool) -> Option<UnitHandle> {
        let (s_rad_sq, f_rad_sq) = (
            flag.soldier_pickup_radius * flag.soldier_pickup_radius,
            flag.flyer_pickup_radius * flag.flyer_pickup_radius,
        );

        for (idx, u) in self.units.iter().enumerate() {
            if !u.alive || (only_enemy && u.team == flag.team) {
                continue;
            }
            let handle = UnitHandle {
                index: idx as u32,
                gen: u.gen,
            };

            if let Some(m) = u.mount {
                if let Some(v) = self.vehicles.get(m.vehicle.index as usize) {
                    if v.health.cur <= 0.0 {
                        continue;
                    }
                    let is_pilot = v.seats.first().and_then(|s| s.occupant) == Some(handle);
                    if is_pilot && (v.pos - flag.current_pos).length_squared() <= f_rad_sq {
                        return Some(handle);
                    }
                }
            } else if (u.soldier.pos - flag.current_pos).length_squared() <= s_rad_sq {
                return Some(handle);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::UnitSpec;
    use crate::vehicle::flyer::FlyerParams;
    use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;

    fn setup_sim() -> Sim {
        let mut s = Sim::empty(42);
        s.flyer_params = vec![Some(FlyerParams::default())];
        s.seat_setup = vec![Some(SeatSetup {
            radius: 5.0,
            body_active: vec![],
            boardable: vec![true],
            exit_drop: 10.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::ZERO,
                pivot: Vec3::ZERO,
                eye: Vec3::ZERO,
                aimer: Aimer::default(),
                pilot: true,
                turn: None,
                framed: false,
                aimer_bind: glam::Quat::IDENTITY,
                gunner: None,
            })],
        })];
        s
    }

    fn spawn_soldier(sim: &mut Sim, team: u8, pos: Vec3) -> UnitHandle {
        let u = sim.spawn(&UnitSpec::trooper(team, pos)).unwrap();
        sim.unit_mut(u).unwrap().soldier.pos = pos;
        u
    }

    #[test]
    fn test_ctf_hash_invariance_when_none() {
        let s1 = Sim::empty(12345);
        let s2 = Sim::empty(12345);
        assert_eq!(s1.hash(), s2.hash());
        assert!(s1.ctf.is_none());
    }

    #[test]
    fn test_ctf_space_pickup_soldier_foot() {
        let mut sim = setup_sim();
        let home = Vec3::new(10.0, 0.0, 0.0);
        let ctf = CtfState::new_space_one_flag(3, home, Vec3::new(-100.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0));
        sim.install_ctf(ctf);

        // Unit 0 at (10.0, 2.0, 0.0): distance 2.0m <= 2.5m pickup radius
        let u0 = spawn_soldier(&mut sim, 0, Vec3::new(10.0, 2.0, 0.0));
        // Unit 1 at (10.0, 5.0, 0.0): distance 5.0m > 2.5m pickup radius
        let _u1 = spawn_soldier(&mut sim, 1, Vec3::new(10.0, 5.0, 0.0));

        sim.tick_ctf(0.1);
        let ctf_state = sim.ctf.as_ref().unwrap();
        assert_eq!(ctf_state.flags[0].status, FlagStatus::Carried { carrier: u0 });
        assert_eq!(ctf_state.flags[0].carrier(), Some(u0));
    }

    #[test]
    fn test_ctf_space_pickup_flyer() {
        let mut sim = setup_sim();
        let home = Vec3::new(0.0, 0.0, 0.0);
        let ctf = CtfState::new_space_one_flag(5, home, Vec3::new(-500.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 0.0));
        sim.install_ctf(ctf);

        // Spawn a pilot unit and flyer at distance 15m (within 20m flyer pickup radius)
        let flyer_pos = Vec3::new(15.0, 0.0, 0.0);
        let v = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: flyer_pos,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let pilot = spawn_soldier(&mut sim, 0, flyer_pos);
        assert!(sim.enter_vehicle(pilot, v, 0));

        sim.tick_ctf(0.1);
        let ctf_state = sim.ctf.as_ref().unwrap();
        assert_eq!(ctf_state.flags[0].status, FlagStatus::Carried { carrier: pilot });
        // Current position of flag tracks vehicle pos + offset
        assert_eq!(ctf_state.flags[0].current_pos, flyer_pos + SPACE_CARRIED_OFFSET);
    }

    #[test]
    fn test_ctf_capture_scoring_and_win() {
        let mut sim = setup_sim();
        let home = Vec3::new(0.0, 0.0, 0.0);
        let team0_goal = Vec3::new(100.0, 0.0, 0.0);
        let team1_goal = Vec3::new(-100.0, 0.0, 0.0);
        let ctf = CtfState::new_space_one_flag(2, home, team0_goal, team1_goal);
        sim.install_ctf(ctf);

        let u0 = spawn_soldier(&mut sim, 0, home);
        sim.tick_ctf(0.1);
        assert_eq!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Carried { carrier: u0 });

        // Move unit 0 into team 0 capture region
        sim.unit_mut(u0).unwrap().soldier.pos = team0_goal;
        sim.tick_ctf(0.1);

        // Score should be 1, flag reset to home
        let ctf_state = sim.ctf.as_ref().unwrap();
        assert_eq!(ctf_state.team_points[0], 1);
        assert_eq!(ctf_state.flags[0].status, FlagStatus::AtHome);
        assert_eq!(ctf_state.flags[0].current_pos, home);
        assert!(sim.outcome.is_none());

        // Second capture to reach score_limit 2
        sim.unit_mut(u0).unwrap().soldier.pos = home;
        sim.tick_ctf(0.1);
        assert_eq!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Carried { carrier: u0 });

        sim.unit_mut(u0).unwrap().soldier.pos = team0_goal;
        sim.tick_ctf(0.1);

        assert_eq!(sim.ctf.as_ref().unwrap().team_points[0], 2);
        assert_eq!(sim.outcome, Some(0)); // Team 0 wins
        assert!(sim.events.iter().any(|e| matches!(e, Event::TeamDefeated { team: 1 })));
    }

    #[test]
    fn test_ctf_dropped_timeout_and_cooldown() {
        let mut sim = setup_sim();
        let home = Vec3::new(0.0, 0.0, 0.0);
        let ctf = CtfState::new_space_one_flag(3, home, Vec3::new(100.0, 0.0, 0.0), Vec3::new(-100.0, 0.0, 0.0));
        sim.install_ctf(ctf);

        let u0 = spawn_soldier(&mut sim, 0, home);
        sim.tick_ctf(0.1);
        assert!(sim.ctf.as_ref().unwrap().flags[0].carrier().is_some());

        // Kill u0 so flag is dropped
        sim.unit_mut(u0).unwrap().alive = false;
        sim.tick_ctf(0.1);

        let ctf_state = sim.ctf.as_ref().unwrap();
        match ctf_state.flags[0].status {
            FlagStatus::Dropped { timer, pickup_cooldown } => {
                assert!(timer <= SPACE_FLAG_RETURN_TIMEOUT);
                assert!(pickup_cooldown > 0.0);
            }
            _ => panic!("Expected dropped flag"),
        }

        // Another unit at the dropped location should NOT pick it up while cooldown > 0
        let u1 = spawn_soldier(&mut sim, 1, home);
        sim.tick_ctf(0.1);
        assert!(matches!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Dropped { .. }));

        // Advance past cooldown (0.5s total)
        sim.tick_ctf(0.4);
        // Now u1 should pick it up
        assert_eq!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Carried { carrier: u1 });

        // Kill u1 and let full 15s timeout elapse
        sim.unit_mut(u1).unwrap().alive = false;
        sim.tick_ctf(0.1); // Flag dropped
        sim.tick_ctf(15.5); // Advance past 15s timeout

        let ctf_state = sim.ctf.as_ref().unwrap();
        assert_eq!(ctf_state.flags[0].status, FlagStatus::AtHome);
        assert_eq!(ctf_state.flags[0].current_pos, home);
        assert!(sim.events.iter().any(|e| matches!(e, Event::FlagReset { flag: 0 })));
    }

    #[test]
    fn test_ctf_two_flag_touch_return() {
        let mut sim = setup_sim();
        let team0_home = Vec3::new(-50.0, 0.0, 0.0);
        let team1_home = Vec3::new(50.0, 0.0, 0.0);
        let ctf = CtfState::new_two_flag(3, team0_home, team1_home, team1_home, team0_home);
        sim.install_ctf(ctf);

        // Enemy u1 (team 1) picks up Team 0 flag (flag 0)
        let u1 = spawn_soldier(&mut sim, 1, team0_home);
        sim.tick_ctf(0.1);
        assert_eq!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Carried { carrier: u1 });

        // u1 dies at (0, 0, 0)
        sim.unit_mut(u1).unwrap().soldier.pos = Vec3::ZERO;
        sim.unit_mut(u1).unwrap().alive = false;
        sim.tick_ctf(0.1);
        assert!(matches!(sim.ctf.as_ref().unwrap().flags[0].status, FlagStatus::Dropped { .. }));

        // Advance cooldown past 0.5s
        sim.tick_ctf(0.5);

        // Friendly u0 (team 0) touches dropped team 0 flag -> Touch Return
        let _u0 = spawn_soldier(&mut sim, 0, Vec3::ZERO);
        sim.tick_ctf(0.1);

        let ctf_state = sim.ctf.as_ref().unwrap();
        assert_eq!(ctf_state.flags[0].status, FlagStatus::AtHome);
        assert_eq!(ctf_state.flags[0].current_pos, team0_home);
        assert!(sim.events.iter().any(|e| matches!(e, Event::FlagReturned { flag: 0, team: 0 })));
    }
}
