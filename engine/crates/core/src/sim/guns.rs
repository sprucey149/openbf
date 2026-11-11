//! Seat guns (card VTUR, `analysis/re-vehicles.md` §5, §9 [O]; the model is
//! `crate::vehicle::gun`): a rider's look, the camera ray, the aimer and
//! the seat's weapons fired through the soldier's weapon code
//! (`WeaponState::update`, `Bolt::spawn`), and the seat switch.
//!
//! - **The gun** (§9): the look moves with the right stick; a 512 m ray
//!   from the eye along the look finds the aim point; the aimer turns
//!   toward it at its rates, inside its limits; the bolt leaves the fire
//!   point along the aimer. A building turret's look and aimer follow the
//!   PS2's MountedTurret law (card VTURL, `gun::Turn`, `Aimer::chase`).
//!
//! A bot rider aims and fires through the same pad (card VTAI): its think
//! (`ai::gunner`) moves the look stick toward its perceived target and
//! presses FIRE, and the shot is held to the on-foot fairness rule here,
//! where it is made. A vehicle's gun states are made when anyone first
//! rides one of its armed seats.

mod seat_switch;

use super::input::{buttons, PadState};
use super::state::{Event, Sim};
use crate::collision::{HullFilter, Layers};
use crate::projectile::{Bolt, Shooter};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::gun::{self, GunSetup, GunState, AIM_RAY};
use crate::vehicle::VehicleHandle;
use crate::vehicle::{SeatKind, VehicleKind};
use crate::weapon::{shot_direction, FireInput, WeaponState};
use crate::TICK_DT;
use glam::{Quat, Vec3};

/// Where a rider's seat gun looks and points now (card VTAI): read by a bot's
/// perception and aim. Before the vehicle is armed the look and the aim are
/// the aimer's rest.
#[derive(Clone, Debug)]
pub struct SeatLook {
    pub gun: GunSetup,
    /// The camera's (yaw, pitch), model frame.
    pub look: (f32, f32),
    /// The aimer's (yaw, pitch), model frame.
    pub aim: (f32, f32),
    /// The vehicle's rotation.
    pub rot: Quat,
    /// The eye the camera ray leaves, world ([`GunSetup::ray_eye`]).
    pub eye: Vec3,
    /// A building turret's zoomed eye, world (card VTURZ, re-turret-draw 4
    /// [O]): `EyePointOffset` on the AIM frame (the look's yaw AND pitch,
    /// [`gun::frame`]) from the pivot - where the PS2's zoomed camera
    /// stands; [`SeatLook::eye`] while the seat is zoomed.
    pub scope_eye: Vec3,
    /// The look's direction, world.
    pub view: Vec3,
    /// The fire point, world.
    pub muzzle: Vec3,
    /// The barrel's direction, world.
    pub barrel: Vec3,
}

impl Sim {
    /// `u`'s seat gun as it stands, or `None` for a seat with no gun.
    pub fn seat_look(&self, u: UnitHandle) -> Option<SeatLook> {
        let m = self.unit(u)?.mount?;
        let gun = self.seat_gun_setup(u)?;
        let v = self.vehicle(m.vehicle)?;
        let (look, _, aim) = gun.rest();
        let (look, aim, zoomed) = v
            .guns
            .get(usize::from(m.seat))
            .and_then(Option::as_ref)
            .map_or((look, aim, false), |g| (g.look, g.aim, g.zoom.is_some()));
        let (pos, rot) = (v.pos, v.rot);
        let eye = pos + rot * gun.ray_eye(look, zoomed);
        let scope_eye = pos + rot * gun.scope_eye(look);
        Some(SeatLook {
            look,
            aim,
            rot,
            eye,
            scope_eye,
            view: (rot * gun::dir(look.0, look.1)).normalize(),
            muzzle: pos + rot * gun.muzzle(aim),
            barrel: (rot * gun::dir(aim.0, aim.1)).normalize(),
            gun,
        })
    }

    /// A building turret's zoom (card VTURZ, `analysis/re-turret-draw.md` 4
    /// [O]): a human gunner's press of ZOOM steps the turret's zoom flag
    /// through the first weapon's toggle (B1:137361-137368, B2:189166-189183:
    /// the weapon has `ZoomMax` > 1; the soldier's toggle B1 `FUN_00283c80`,
    /// `WeaponClass::zoom_press`): on at `ZoomMin`, then on to `ZoomMax` when
    /// the two differ and `ZoomRate` is 0, else off. BF2's turret also runs
    /// the continuous zoom when `ZoomRate` != 0 (B2 `FUN_003258f8` called at
    /// 189182 with T+0x7c, the controllable's (T+0x10) +0x6c: the throttle
    /// slot, re-hover 5) - zoom += `throttle` x zoom x `ZoomRate` x dt inside
    /// [`ZoomMin`, `ZoomMax`]; BF1's turret calls only the toggle (its three
    /// callers of `FUN_00283c18` are the soldier and two vehicle updates). A
    /// seat whose weapon cannot zoom holds none. The port's seat fires its
    /// first weapon on FIRE and has no weapon switch, so the PS2's switch
    /// cancel is the seat switch's pilot change (`Sim::unseat_unit`). Only
    /// a building turret (a [`gun::Turn`]) zooms: a vehicle's turret seat is
    /// not read. PROVISIONAL (not read): the look's turn is not divided by
    /// the zoom's divisor as a soldier's is.
    pub(super) fn tick_seat_zoom(&mut self, i: usize, press: bool, throttle: f32) {
        if !matches!(self.units[i].controller, Controller::Player { .. }) {
            return;
        }
        let h = self.handle(i);
        let (Some(m), Some(g)) = (self.units[i].mount, self.seat_gun_setup(h)) else {
            return;
        };
        let bf2 = self.ground_rules == crate::soldier::GroundRules::Bf2;
        let wc = g
            .weapons
            .first()
            .map(|w| &self.weapon_classes[w.class])
            .filter(|wc| g.turn.is_some() && wc.zooms());
        let Some(Some(st)) = self.vehicles[m.vehicle.index as usize]
            .guns
            .get_mut(usize::from(m.seat))
        else {
            return;
        };
        let Some(wc) = wc else {
            st.zoom = None;
            return;
        };
        if press {
            let step = st.zoom.map(|z| u8::from(z != wc.zoom_min));
            st.zoom = wc.zoom_press(step).map(|s| wc.zoom_at(s).0);
        }
        if let Some(z) = st.zoom.as_mut().filter(|_| bf2 && wc.zoom_rate != 0.0) {
            *z = (*z + throttle * *z * wc.zoom_rate * TICK_DT).clamp(wc.zoom_min, wc.zoom_max);
        }
    }

    /// `u`'s seat zoom as the camera takes it (card VTURZ): the turret's
    /// zoom value; `step` 0 at `ZoomMin`, else 1, and the divisor that
    /// step's (the seat's turn does not read it); `scope` is set (the view is
    /// the eye point's, along the look).
    pub fn seat_zoom(&self, u: UnitHandle) -> Option<crate::unit::Zoom> {
        let m = self.unit(u)?.mount?;
        let value = self
            .vehicle(m.vehicle)?
            .guns
            .get(usize::from(m.seat))?
            .as_ref()?
            .zoom?;
        let g = self.seat_gun_setup(u)?;
        let wc = &self.weapon_classes[g.weapons.first()?.class];
        let step = u8::from(value != wc.zoom_min);
        Some(crate::unit::Zoom {
            step,
            value,
            divisor: wc.zoom_at(step).1,
            scope: true,
        })
    }

    /// The gun setup of `u`'s seat, if it has one.
    pub(super) fn seat_gun_setup(&self, u: UnitHandle) -> Option<GunSetup> {
        let m = self.unit(u)?.mount?;
        let v = self.vehicle(m.vehicle)?;
        let setup = self.seat_setup.get(v.class as usize)?.as_ref()?;
        setup.guns.get(usize::from(m.seat))?.clone()
    }

    /// Make every seat's gun state of vehicle `vi` from its class's setup
    /// (each gun at rest, each weapon full), once.
    pub fn arm(&mut self, vi: usize) {
        if !self.vehicles[vi].guns.is_empty() {
            return;
        }
        let class = self.vehicles[vi].class as usize;
        let Some(setup) = self.seat_setup.get(class).and_then(|s| s.as_ref()) else {
            return;
        };
        let n = self.vehicles[vi].seats.len();
        let guns = (0..n)
            .map(|s| {
                let g = setup.guns.get(s)?.as_ref()?;
                let (look, rel, aim) = g.rest();
                Some(GunState {
                    look,
                    aim,
                    rate: (0.0, 0.0),
                    rel,
                    weapons: g
                        .weapons
                        .iter()
                        .map(|w| {
                            let class = &self.weapon_classes[w.class];
                            if w.infinite {
                                WeaponState::with_clips(class, true, 0)
                            } else {
                                WeaponState::new(class, true, w.reserve_clips)
                            }
                        })
                        .collect(),
                    zoom: None,
                })
            })
            .collect::<Vec<_>>();
        if guns.iter().any(Option::is_some) {
            self.vehicles[vi].guns = guns;
        }
    }

    /// The first surface or soldier along the camera ray, else its end —
    /// past the seat's own vehicle's hull (card VHULL) and a building
    /// turret's own building (card TURR).
    fn aim_point(&self, eye: Vec3, dir: Vec3, riders: &[UnitHandle], own: VehicleHandle) -> Vec3 {
        let mut t = AIM_RAY;
        if let Some(h) = self.terrain_ray(eye, dir, t, 0.5) {
            t = t.min(h);
        }
        // A building turret's ray passes its own building (the PS2's
        // excludes the owner entity, `ray_past`).
        let building = self.vehicle(own).and_then(|v| v.instance);
        if let Some(h) = self.collision.ray_past(
            eye,
            dir,
            t,
            Layers::ORDNANCE,
            HullFilter::except(Some(own)),
            [building, None],
        ) {
            t = t.min(h.t);
        }
        for (h, u) in self.alive_units() {
            if riders.contains(&h) {
                continue;
            }
            if let Some(hit) = u.capsule().ray_hit(eye, dir, t) {
                t = t.min(hit);
            }
        }
        eye + dir * t
    }

    /// Step 4, for a rider: move its seat's look and aimer and fire the
    /// seat's weapons (FIRE the first, SECONDARY_FIRE the second). A bot's
    /// pad is its think's (`ai::gunner`).
    pub(super) fn seat_gun(&mut self, i: usize, p: &PadState) {
        let ai = match self.units[i].controller {
            Controller::Player { .. } => false,
            Controller::Ai => true,
            _ => return,
        };
        let h = self.handle(i);
        let Some(m) = self.units[i].mount else { return };
        let Some(g) = self.seat_gun_setup(h) else {
            return;
        };
        let bot_aim = (ai && g.turn.is_some())
            .then(|| {
                self.units[i]
                    .ai
                    .ride
                    .and_then(|r| r.man)
                    .and_then(|m| m.aim)
            })
            .flatten();
        self.drive_seat_gun(m, g, p, Some(i), bot_aim);
    }

    /// Shared mounted weapon path: an occupied seat or a self-piloted building.
    /// An autonomous gun has no soldier and credits no fabricated player.
    pub(super) fn drive_seat_gun(
        &mut self,
        m: crate::vehicle::Mount,
        g: GunSetup,
        p: &PadState,
        driver: Option<usize>,
        bot_aim: Option<Vec3>,
    ) {
        let h = driver.map_or(
            UnitHandle {
                index: u32::MAX,
                // A distinct non-unit source per building, so simultaneous
                // self guns do not collapse into one fire sound.
                gen: m.vehicle.index,
            },
            |i| self.handle(i),
        );
        let ai = driver.is_some_and(|i| self.units[i].controller == Controller::Ai);
        let vi = m.vehicle.index as usize;
        self.arm(vi);
        let seat = usize::from(m.seat);
        let dt = TICK_DT;
        let (pos, rot) = (self.vehicles[vi].pos, self.vehicles[vi].rot);
        let riders: Vec<UnitHandle> = self.vehicles[vi]
            .seats
            .iter()
            .filter_map(|s| s.occupant)
            .collect();
        // Out of its slot while it is driven, and back at the end.
        let Some(mut st) = self.vehicles[vi].guns.get_mut(seat).and_then(Option::take) else {
            return;
        };

        // The look: a building turret's by its turn law (card VTURL);
        // otherwise the stick at the aimer's rates, inside its limits. A
        // walker's pilot looks along the walker's own aim (card WALK: the
        // controllable's E+0x50c / +0x510, re-walker §3 I7-I15), which its
        // integrator moved this tick; the aimer then follows it.
        let a = g.aimer;
        let rx = PadState::axis(p.rx);
        let ry = PadState::axis(p.ry);
        let walker = g.pilot && self.vehicles[vi].kind == VehicleKind::Walker;
        let flyer = g.pilot && self.vehicles[vi].kind == VehicleKind::Flyer;
        if let Some(t) = g.turn {
            (st.look, st.rate) = t.step(st.look, st.rate, (rx, ry), dt);
        } else if walker {
            let w = &self.vehicles[vi].walker;
            st.look = (w.aim_yaw, w.aim_pitch);
        } else if flyer {
            // The pilot looks along the untilted craft frame, not a second
            // independently accumulated pitch (B1 FUN_001dbbd8, B2 FUN_00209758).
            st.look = (0.0, self.vehicles[vi].flyer.tilt_pitch);
        } else {
            st.look.0 = if g.pilot {
                0.0
            } else {
                (st.look.0 - rx * a.yaw_rate * dt).clamp(a.yaw[0], a.yaw[1])
            };
            st.look.1 = (st.look.1 + ry * a.pitch_rate * dt).clamp(a.pitch[0], a.pitch[1]);
        }
        if p.has_look() && !walker && !flyer {
            st.look = super::look::seat_look(&g, st.look, p.look(), st.zoom);
        }

        // The camera ray and the aimer (zoomed, from the zoomed eye). A bot
        // in a building turret hands its aimer its own aim point instead
        // (`ai::ManState::aim`, the PS2's AI seat).
        let target = bot_aim.unwrap_or_else(|| {
            let eye = pos + rot * g.ray_eye(st.look, st.zoom.is_some());
            self.aim_point(
                eye,
                rot * gun::dir(st.look.0, st.look.1),
                &riders,
                m.vehicle,
            )
        });
        let pivot = pos + rot * g.pivot;
        if g.turn.is_some() {
            // The PS2's aimer, in the turret's aim frame or the model's. It
            // lays the BARREL's line on the aim point, not the pivot's: in
            // BF2 Mygeeto's recoilless turret (barrels 3.4 m over the
            // pivot) the settled aim point is 0.004 m off the line through
            // the two barrels' midpoint along the aimer's direction and 3.5 m
            // off the pivot's (PS2 RAM, 342 rows, card TURR,
            // `lead_tmp/turr/aim/ps2/barrel_line.py`). The port fires from
            // one fire point, so it aims that one's line (half the barrel
            // spacing, 0.36 m there, off the PS2's centre line).
            //
            // The line is measured from its point nearest the pivot, not
            // from the muzzle (playtest 2026-10-29, BF2 Geonosis: "jumpy and
            // jittery" while walking round a manned cannon). Both name the
            // same line, so the settled aim is the same; but the muzzle
            // swings with the aim, and a muzzle far ahead of the pivot
            // (the cannon's 2.5 m) moved the asked direction against the
            // turn: from 5 m the aimer flipped about 4 deg every tick.
            let off = g.muzzle(st.aim) - g.pivot;
            let d = gun::dir(st.aim.0, st.aim.1);
            let from = pos + rot * (g.pivot + off - d * off.dot(d));
            let w = (rot.inverse() * (target - from)).normalize_or_zero();
            let w = if g.framed {
                let [x, y, z] = gun::frame(st.look);
                Vec3::new(w.dot(x), w.dot(y), w.dot(z))
            } else if g.aimer_bind != Quat::IDENTITY {
                g.aimer_bind.inverse() * w
            } else {
                w
            };
            st.rel = a.chase(st.rel, w, dt);
            st.aim = g.aim_of(st.look, st.rel);
        } else {
            let mut want = gun::angles(rot.inverse() * (target - pivot));
            let hold_yaw = g.pilot && !walker;
            if hold_yaw {
                want.0 = 0.0;
            }
            st.aim = a.turn(st.aim, want, dt);
            if hold_yaw {
                st.aim.0 = 0.0;
            }
            st.rel = st.aim;
        }

        // The weapons.
        let origin = pos + rot * g.muzzle(st.aim);
        let fwd = (rot * gun::dir(st.aim.0, st.aim.1)).normalize();
        let lock_origin = pos + rot * g.ray_eye(st.look, st.zoom.is_some());
        let lock_dir = (rot * gun::dir(st.look.0, st.look.1)).normalize_or_zero();
        let right = fwd.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        let up = right.cross(fwd).normalize();
        let is_player = driver.is_some_and(|i| self.units[i].is_player);
        let team = driver.map_or(self.vehicles[vi].team.unwrap_or(u8::MAX), |i| {
            self.units[i].team
        });
        let (infinite_cheat, no_reload_cheat) =
            driver.map_or((false, false), |i| self.ammo_cheats(i));
        // On foot's D9 spread multiplier and the fairness record (step 4).
        let ai_spread_mult = driver
            .filter(|_| ai)
            .map(|i| self.skill_of(&self.units[i]).spread_mult);
        let free_burst = g.pilot && self.vehicles[vi].flyer.ai.fire_held;
        let armed_target = g
            .pilot
            .then_some(self.vehicles[vi].flyer.ai.armed_target)
            .flatten();
        let channel = (ai
            && g.pilot
            && self.vehicles[vi].kind == VehicleKind::Flyer
            && self
                .flyer_params
                .get(self.vehicles[vi].class as usize)
                .and_then(Option::as_ref)
                .is_some_and(|p| p.bf2))
        .then_some(usize::from(self.vehicles[vi].flyer.ai.channel));
        let armed_station = g.pilot && self.vehicles[vi].flyer.ai.armed_station.is_some();
        let perceived = !ai || free_burst || armed_target.is_some() || armed_station || {
            let st = &self.units[driver.unwrap()].ai;
            st.target
                .is_some_and(|t| st.contacts.iter().flatten().any(|c| c.handle == t))
        };
        // Both PS2 flyer updates clear pilot AND mounted-turret triggers
        // outside Flying; BF2 also clears them in a landing region
        // (B1:86443–86482, B2:118975–119007).
        let vehicle = &self.vehicles[vi];
        let flight_blocked = vehicle.kind == VehicleKind::Flyer
            && (vehicle.flyer.phase != crate::vehicle::flyer::FlightPhase::Flying
                || (self
                    .flyer_params
                    .get(vehicle.class as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|p| p.bf2)
                    && self.flyer_region_coverage(vehicle).0));
        let mut shots = Vec::new();
        for (k, w) in g.weapons.iter().enumerate() {
            let held = match (channel, k) {
                (Some(c), _) => c == k && p.pressed(buttons::FIRE),
                (None, 0) => p.pressed(buttons::FIRE),
                (None, 1) => p.pressed(buttons::SECONDARY_FIRE),
                _ => false,
            };
            let wc = &self.weapon_classes[w.class];
            let ws = &mut st.weapons[k];
            // A seat stands still: the class's standing, unmoving spread.
            ws.set_posture(wc, 0, 0.0, false);
            if let Some(m) = ai_spread_mult {
                ws.spread_mult = m;
            }
            let player_guided = is_player && wc.guided.is_some();
            let guided_up = player_guided
                && driver.is_some_and(|i| {
                    let owner = self.handle(i);
                    self.guided.iter().any(|r| r.owner == owner)
                });
            let guided_release = player_guided && ws.remote_press == 2 && held;
            if !held {
                ws.remote_press = 0;
            }
            let input = FireInput {
                held: held && !flight_blocked && !guided_up && !guided_release,
                blocked: flight_blocked,
                infinite_ammo: infinite_cheat,
                no_reload: no_reload_cheat,
                ai_dispersion: None,
            };
            shots.clear();
            let was_reloading = ws.reloading();
            ws.update(dt, wc, input, &mut self.lcg, &mut shots);
            let reload_began = !was_reloading && ws.reloading();
            if shots.is_empty() && ws.salvo_counter < 1 && k < super::lock_on::LOCK_CHANNELS {
                if let Some(i) = driver {
                    if let crate::unit::Controller::Player { slot } = self.units[i].controller {
                        if let Some(lock) = self.lock_on.get_mut(slot as usize) {
                            lock.salvo_target[k] = None;
                        }
                    }
                }
            }
            if let Some(i) = driver.filter(|_| reload_began) {
                let unit = self.handle(i);
                self.events.push(Event::Reload {
                    unit,
                    weapon_class: w.class as u32,
                });
            }
            if shots.is_empty() {
                continue;
            }
            if let Some(i) = driver {
                self.units[i].last_shot_tick = self.tick;
            }
            let who = Shooter {
                owner: h,
                team,
                weapon_class: w.class,
                is_player,
            };
            let first_shot = st.weapons[k].shots_fired.saturating_sub(shots.len() as u64);
            for (shot_index, spec) in shots.iter().enumerate() {
                let origin = if g.pilot && self.vehicles[vi].kind == VehicleKind::Flyer {
                    w.fire_points
                        .get(
                            ((first_shot + shot_index as u64) % w.fire_points.len().max(1) as u64)
                                as usize,
                        )
                        .map_or(origin, |point| pos + rot * *point)
                } else {
                    origin
                };
                // The round's speed and life at its charge (the Hoth dish
                // turret's charge points, re-weapon-charge §4).
                let blend = self.weapon_classes[w.class].ordnance.charged(spec.charge);
                let (speed, life) = (blend.velocity, blend.life_span);
                let d = shot_direction(
                    &mut self.lcg,
                    fwd,
                    right,
                    up,
                    spec.pitch_spread,
                    spec.yaw_spread,
                    spec.tilt,
                );
                if ai {
                    self.nav_stats.ai_shots += 1;
                    self.nav_stats.seat_shots += 1;
                    // Design decision 2's rule, as on foot: a pressed shot
                    // goes only at a contact this bot's perception holds.
                    if held {
                        debug_assert!(
                            perceived,
                            "AI rider {driver:?} fired at an unperceived target at tick {}",
                            self.tick
                        );
                        if !perceived {
                            self.nav_stats.unperceived_shots += 1;
                        }
                    } else {
                        self.nav_stats.carry_shots += 1;
                    }
                }
                if let Some(class) = self.weapon_classes[w.class].ordnance.tow {
                    self.fire_tow(m, who, class, origin, d, speed, life);
                    continue;
                }
                let bolt = Bolt::fire(
                    origin,
                    d,
                    speed,
                    life,
                    who,
                    self.heightfield.as_ref(),
                    self.water_plane(),
                    &self.collision,
                    HullFilter::except(Some(m.vehicle)),
                )
                .shaped(&self.weapon_classes[w.class].ordnance);
                if is_player && self.weapon_classes[w.class].guided.is_some() {
                    if let Some(i) = driver {
                        self.launch_seat_guided(i, w.class, m.vehicle, m.seat, origin, d);
                    }
                    continue;
                }
                let mut bolt = bolt;
                bolt.charge = spec.charge;
                // A seat's missile (card UORD §4): the cannon-class wrist
                // rockets pass their held target at every shot
                // (`re-ordnance-kinds.md` §5), so the vehicle gun's own
                // `ai.target` / crewman's target is its lock, with no dwell.
                if let Some(mc) = self.weapon_classes[w.class].ordnance.missile {
                    let ord = self.weapon_classes[w.class].ordnance.clone();
                    let target = self
                        .units
                        .get(h.index as usize)
                        .filter(|u| u.gen == h.gen)
                        .and_then(|u| u.ai.target)
                        .and_then(|f| f.unit())
                        .map(crate::missile::MissileTarget::Unit);
                    let locked = driver
                        .and_then(|i| self.seat_lock_target(i, w.class, lock_origin, lock_dir));
                    let armed = armed_target.map(crate::missile::MissileTarget::Vehicle);
                    let target = locked.or(armed).or(target);
                    bolt.as_missile(&ord, &mc, target, &mut self.lcg);
                }
                self.note_bolt_shot(&mut bolt, origin, d);
                self.launch_round(bolt, false);
                if self.weapon_classes[w.class].secondary.is_some() {
                    let round = st.weapons[k].count_secondary_round();
                    self.fire_secondary(who, w.class, round, origin, d, spec.charge);
                }
            }
            if st.weapons[k].salvo_counter < 1 && k < super::lock_on::LOCK_CHANNELS {
                if let Some(i) = driver {
                    if let crate::unit::Controller::Player { slot } = self.units[i].controller {
                        if let Some(lock) = self.lock_on.get_mut(slot as usize) {
                            lock.salvo_target[k] = None;
                        }
                    }
                }
            }
        }
        self.vehicles[vi].guns[seat] = Some(st);
    }
}

#[cfg(test)]
mod tests;
