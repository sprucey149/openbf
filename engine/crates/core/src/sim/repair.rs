//! The fusion cutter in the sim: target choice, energy-gated pulses, station
//! rebuilding and fallen building turrets. Follows `analysis/re-build-zones.md`
//! (card TBLD).

use super::input::buttons;
use super::{Event, Sim};
use crate::collision::HullFilter;
use crate::projectile::Target;
use crate::unit::{Controller, Unit};
use crate::vehicle::{VehicleHandle, VehicleKind, SLICE_RESERVE};
use crate::weapon::WeaponClass;
use glam::Vec3;

impl Sim {
    pub(super) fn tick_repair_weapon(
        &mut self,
        i: usize,
        held_slot: Option<usize>,
        held: bool,
        blocked: bool,
        dt: f32,
    ) -> bool {
        let u = &self.units[i];
        let class = held_slot.map_or(u.weapon_class, |k| u.held[k].class);
        let wc = &self.weapon_classes[class];
        if wc.repair.is_none() {
            return false;
        }
        let target = self.repair_target(i, class);
        // The cutter's empty click: the PS2's repair weapon runs the base
        // update (B1 `FUN_002928b8` -> `FUN_00284318`), so its overheated
        // state clicks a press with or without a target; the port keeps its
        // heat on the soldier's energy, so a press the energy cannot pay is
        // the refused one.
        // PROVISIONAL [D], measure: a PS2 log of a human's press on a cutter
        // past its heat, in both games.
        let button = if held_slot.is_some() {
            buttons::SECONDARY_FIRE
        } else {
            buttons::FIRE
        };
        let cost = wc.heat_per_shot * u.consts.energy_bar;
        let click = held
            && !blocked
            && u.is_player
            && u.soldier.energy < cost
            && !self.cheats.infinite_stamina
            && self.pressed_now(i, button);
        let u = &mut self.units[i];
        let state = match held_slot {
            Some(k) => &mut u.held[k].state,
            None => &mut u.weapon,
        };
        if state.repair_tick(wc, held && target.is_some(), blocked, dt) {
            if let Some(target) = target {
                self.fire_repair(i, class, target);
            }
        }
        if click {
            self.push_fire_empty(i, class);
        }
        true
    }

    fn repair_candidate(
        &self,
        u: &Unit,
        wc: &WeaponClass,
        sphere: (Vec3, f32),
        owner: (Option<u8>, Option<u32>, HullFilter),
    ) -> Option<f32> {
        let (centre, radius) = sphere;
        let (team, instance, hulls) = owner;
        let relation = match team {
            Some(t) if t == u.team => 2,
            Some(_) => 0,
            None => 1,
        };
        if wc.target_team & (1 << relation) == 0 {
            return None;
        }
        let from = u.muzzle();
        let delta = centre - from;
        let dist = delta.length();
        if dist > wc.lock_on_range + radius || dist <= 0.0 {
            return None;
        }
        let dir = delta / dist;
        let score = super::input::cone_score(
            radius,
            wc.lock_on_angle,
            wc.lock_on_range,
            from,
            u.aim_frame().0,
            centre,
        );
        if score < 0.0 {
            return None;
        }
        // The LOS ray stops r short of the centre, on the shooter's side
        // (`re-aim-assist.md` 3.3, 3.6), the same ray as the magnet's: a pad
        // sunk under the terrain, Kashyyyk's `com_weap_gunturret10`, still
        // takes the cutter from the ground above it.
        let end = centre - dir * radius;
        if !crate::perception::los_seat(self, from, end, hulls, [instance, None]) {
            return None;
        }
        Some(score)
    }

    /// What unit `i`'s cutter in hand would take now (either channel's
    /// current weapon); `None` with no cutter up. The HUD's wreck readout.
    pub fn cutter_target(&self, i: usize) -> Option<Target> {
        let u = self.units.get(i)?;
        let tool = |c: usize| {
            self.weapon_classes
                .get(c)
                .is_some_and(|w| w.repair.is_some())
        };
        let class = if tool(u.weapon_class) {
            u.weapon_class
        } else {
            let c = u.held.get(usize::from(u.cur1?))?.class;
            tool(c).then_some(c)?
        };
        self.repair_target(i, class)
    }

    fn repair_target(&self, i: usize, class: usize) -> Option<Target> {
        let u = &self.units[i];
        let wc = &self.weapon_classes[class];
        let repair = wc.repair?;
        let mut best: Option<(f32, Target)> = None;
        for (si, s) in self.stations.iter().enumerate() {
            let kind = s.repair_kind() as usize;
            if repair.health[kind] <= 0.0 || s.health.cur >= s.health.max {
                continue;
            }
            if let Some(score) = self.repair_candidate(
                u,
                wc,
                (s.sphere_centre, s.sphere_radius),
                (s.team, s.instance, HullFilter::All),
            ) {
                if best.is_none_or(|(old, _)| score < old) {
                    best = Some((score, Target::Station(si as u16)));
                }
            }
        }
        // A standing auto-turret takes the shot its health type does (the
        // amount table is by the target's type, B2 `FUN_00346fa0` 339872, and
        // this class's is Building), a friend's only: it always has a team.
        for t in self
            .turrets
            .iter()
            .filter(|t| t.alive() && t.team == u.team)
        {
            let Some(c) = self.turret_class(t) else {
                continue;
            };
            if repair.health[c.health_type as usize] <= 0.0 || t.health >= c.body.max_health {
                continue;
            }
            let sphere = (t.centre(), crate::autoturret::RADIUS);
            if let Some(score) =
                self.repair_candidate(u, wc, sphere, (Some(t.team), None, HullFilter::All))
            {
                if best.is_none_or(|(old, _)| score < old) {
                    best = Some((score, Target::Turret(t.id)));
                }
            }
        }
        for (vh, v) in self.alive_vehicles() {
            // Issue #35: a player's cutter slices an enemy vehicle with riders.
            let enemy = v.team.is_some_and(|t| t != u.team);
            if enemy {
                if u.controller == Controller::Ai || !self.sliceable(vh) {
                    continue;
                }
            } else if repair.health[v.health.health_type as usize] <= 0.0
                || v.health.cur >= v.health.max
            {
                continue;
            }
            let Some((centre, radius)) = self.vehicle_sphere(vh) else {
                continue;
            };
            // A slice passes the team mask on both games (BF2's rule on BF1).
            let team = if enemy { Some(u.team) } else { v.team };
            if let Some(score) = self.repair_candidate(
                u,
                wc,
                (centre, radius),
                (team, v.instance, HullFilter::except(Some(vh))),
            ) {
                if best.is_none_or(|(old, _)| score < old) {
                    best = Some((score, Target::Vehicle(vh)));
                }
            }
        }
        // A fallen building turret (card TBLD): its wreck or pad takes the
        // rebuild / build amount (`analysis/re-build-zones.md` 1-2). It is
        // team 0 on the PS2: neutral to every cutter.
        for (bi, t) in self.building_turrets.iter().enumerate() {
            if t.down.is_none() || repair.health[t.repair_kind() as usize] <= 0.0 {
                continue;
            }
            if let Some(score) =
                self.repair_candidate(u, wc, t.sphere(), (None, Some(t.instance), HullFilter::All))
            {
                if best.is_none_or(|(old, _)| score < old) {
                    best = Some((score, Target::Building(bi as u16)));
                }
            }
        }
        best.map(|(_, target)| target)
    }

    fn fire_repair(&mut self, i: usize, class: usize, target: Target) {
        let repair = self.weapon_classes[class].repair.unwrap();
        let cost = self.weapon_classes[class].heat_per_shot * self.units[i].consts.energy_bar;
        if !(self.units[i].is_player && self.cheats.infinite_stamina) {
            if self.units[i].soldier.energy < cost {
                return;
            }
            self.units[i].soldier.energy -= cost;
        }
        let mut sliced = false;
        let (pos, amount) = match target {
            Target::Station(si) => {
                let s = &self.stations[usize::from(si)];
                let kind = s.repair_kind() as usize;
                let before = s.health.cur;
                let pos = s.sphere_centre;
                self.repair_station(si, repair.health[kind]);
                (pos, self.stations[usize::from(si)].health.cur - before)
            }
            Target::Vehicle(vh) => {
                let team = self.units[i].team;
                if self
                    .vehicle(vh)
                    .is_some_and(|v| v.team.is_some_and(|t| t != team))
                {
                    let pos = self.vehicle(vh).unwrap().pos;
                    let step = self.weapon_classes[class].shot_delay[0];
                    self.slice_vehicle(i, vh, step);
                    sliced = true;
                    (pos, 0.0)
                } else {
                    let Some(v) = self.vehicle_mut(vh) else {
                        return;
                    };
                    let before = v.health.cur;
                    v.health.cur = (v.health.cur + repair.health[v.health.health_type as usize])
                        .min(v.health.max);
                    (v.pos, v.health.cur - before)
                }
            }
            Target::Building(bi) => {
                let Some(t) = self.building_turrets.get(usize::from(bi)) else {
                    return;
                };
                let (pos, amount) = (t.sphere().0, repair.health[t.repair_kind() as usize]);
                (pos, self.repair_building(bi, amount))
            }
            Target::Turret(id) => {
                let Some(k) = self.turrets.iter().position(|t| t.id == id) else {
                    return;
                };
                let Some(c) = self.turret_class(&self.turrets[k]) else {
                    return;
                };
                let (gain, max) = (repair.health[c.health_type as usize], c.body.max_health);
                let t = &mut self.turrets[k];
                let before = t.health;
                t.health = (t.health + gain).min(max);
                (t.centre(), t.health - before)
            }
            Target::Unit(_)
            | Target::Mine(_)
            | Target::Droid(_)
            | Target::Guided(_)
            | Target::Shield(_) => return,
        };
        if amount > 0.0 || sliced {
            self.events.push(Event::Repair {
                unit: self.handle(i),
                weapon_class: class as u32,
                target,
                pos,
                amount,
            });
        }
    }

    /// A vehicle's cutter sphere in the world: its aim sphere, else its
    /// class's blast sphere (`Vehicle::aim_sphere`), else its origin.
    pub fn vehicle_sphere(&self, vh: VehicleHandle) -> Option<(Vec3, f32)> {
        let v = self.vehicle(vh)?;
        let (c, r) = v.aim_sphere.unwrap_or_else(|| {
            self.blast_setup
                .get(v.class as usize)
                .and_then(|b| b.as_ref())
                .map_or((Vec3::ZERO, 0.0), |b| (b.sphere_centre, b.sphere_radius))
        });
        Some((v.pos + v.rot * c, r))
    }

    /// Issue #35: the class's slice time, s; 0 = never sliced.
    pub fn slice_time_of(&self, vh: VehicleHandle) -> f32 {
        self.vehicle(vh)
            .and_then(|v| self.slice_time.get(v.class as usize))
            .copied()
            .unwrap_or(0.0)
    }

    /// Can a cutter slice `vh`: not a turret, a class with a slice time,
    /// and a soldier aboard (B2 `FUN_00346fa0`, re-build-zones §11).
    pub fn sliceable(&self, vh: VehicleHandle) -> bool {
        self.vehicle(vh).is_some_and(|v| {
            v.kind != VehicleKind::Turret
                && self.slice_time_of(vh) > 0.0
                && v.seats.iter().any(|s| {
                    s.occupant
                        .and_then(|h| self.unit(h))
                        .is_some_and(|o| o.alive)
                })
        })
    }

    /// Issue #35: one cutter shot on an enemy vehicle adds the cutter's
    /// ShotDelay to its meter; past the class's slice time every rider is
    /// put out and only the slicer may board for [`SLICE_RESERVE`].
    fn slice_vehicle(&mut self, i: usize, vh: VehicleHandle, step: f32) {
        let time = self.slice_time_of(vh);
        let Some(v) = self.vehicle_mut(vh) else {
            return;
        };
        v.slice += step;
        if v.slice <= time {
            return;
        }
        v.slice = 0.0;
        let riders: Vec<_> = v.seats.iter().filter_map(|s| s.occupant).collect();
        for r in riders {
            self.exit_vehicle(r);
        }
        let until = self.tick + (SLICE_RESERVE * crate::TICK_HZ as f32) as u64;
        let slicer = self.handle(i);
        if let Some(v) = self.vehicle_mut(vh) {
            v.slice_hold = Some((slicer, until));
        }
    }

    pub fn repair_station(&mut self, station: u16, amount: f32) {
        let Some(s) = self.stations.get_mut(usize::from(station)) else {
            return;
        };
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        s.health.cur = (s.health.cur + amount).min(s.health.max);
        if !s.destroyed || s.health.cur < s.health.max {
            return;
        }
        s.destroyed = false;
        s.serving = false;
        let pose = s.pose();
        let c = &self.station_classes[usize::from(s.class)];
        if let Some([x, y, z, r]) = c.sphere {
            s.sphere_centre = pose.transform_point3(Vec3::new(x, y, z));
            s.sphere_radius = r;
        }
        if let Some(instance) = s.instance {
            let parts = self
                .station_live_collision
                .get(usize::from(station))
                .map_or(&[][..], Vec::as_slice);
            self.collision.replace_instance(instance, parts, pose);
            self.collision.bind_station(instance, station);
            let bodies = if s.serves() {
                self.collision.prepare_rotation(instance, pose)
            } else {
                Vec::new()
            };
            self.station_bodies
                .resize_with(self.stations.len(), Vec::new);
            self.station_bodies[usize::from(station)] = bodies;
        }
    }
}

#[cfg(test)]
mod tests;
