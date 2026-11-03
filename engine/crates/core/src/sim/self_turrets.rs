//! BF2 placed `PilotType=self` seats (issue #91). No soldier is invented:
//! the building owns the controller and the ordinary mounted gun owns shots.
//! PS2 reference: re-turret-ai §§2–3, 6–7; 100 m view, range + 10 m fight,
//! 22.5 degree fire cone, idle yaw ±30 degrees every 18 thinks. The 0.5 s
//! think cadence is provisional port policy; bursts share the PS2 heat and
//! skill laws with soldier gunners.
use super::{buttons, PadState, Sim};
use crate::autoturret::{Aim, AIM_CONE, FIGHT_MARGIN, LOOK_EVERY, LOOK_SPREAD, THINK, VIEW};
use crate::collision::HullFilter;
use crate::fmath;
use crate::vehicle::{gun, Mount, VehicleHandle};
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct SelfPilot {
    pub seat: u8,
    pub target: Option<Aim>,
    pub fighting: bool,
    pub next_think: u64,
    pub thinks: u32,
    pub look: f32,
    pub shots: u32,
    pub burst: u8,
    pub pause_until: u64,
}

impl SelfPilot {
    pub fn new(seat: u8) -> Self {
        Self {
            seat,
            target: None,
            fighting: false,
            next_think: 0,
            thinks: 0,
            look: 0.0,
            shots: 0,
            burst: 0,
            pause_until: 0,
        }
    }

    pub(super) fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        eat(b"SELF");
        eat(&[self.seat, u8::from(self.fighting), self.burst]);
        match self.target {
            None => eat(&[0]),
            Some(Aim::Unit(h)) => {
                eat(&[1]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
            Some(Aim::Vehicle(h)) => {
                eat(&[2]);
                eat(&h.index.to_le_bytes());
                eat(&h.gen.to_le_bytes());
            }
        }
        eat(&self.next_think.to_le_bytes());
        eat(&self.thinks.to_le_bytes());
        eat(&self.look.to_bits().to_le_bytes());
        eat(&self.shots.to_le_bytes());
        eat(&self.pause_until.to_le_bytes());
    }
}

impl Sim {
    /// Enable only the authored self seats of this placed building.
    pub fn set_building_self_pilots(&mut self, instance: u32, seats: &[u8]) {
        if let Some(t) = self
            .building_turrets
            .iter_mut()
            .find(|t| t.instance == instance)
        {
            t.self_pilots = seats.iter().copied().map(SelfPilot::new).collect();
        }
    }

    pub(super) fn tick_self_turrets(&mut self) {
        for i in 0..self.building_turrets.len() {
            let t = &self.building_turrets[i];
            if t.down.is_some() || t.self_pilots.is_empty() {
                continue;
            }
            let h = t.handle;
            let mut pilots = std::mem::take(&mut self.building_turrets[i].self_pilots);
            for p in &mut pilots {
                self.self_turret(h, p);
            }
            self.building_turrets[i].self_pilots = pilots;
        }
    }

    /// The aim point on `a`, if it is a live enemy in `view` metres with a clear
    /// line. The line is the costly part, so distance comes first.
    fn self_target(
        &self,
        h: VehicleHandle,
        a: Aim,
        team: u8,
        from: Vec3,
        view: f32,
    ) -> Option<Vec3> {
        let own = self.vehicle(h)?;
        let (at, target_hull, target_building) = match a {
            Aim::Unit(u) => {
                if crate::ai::dfocus::just_spawned(self, u) {
                    return None;
                }
                let u = self.unit(u).filter(|u| {
                    u.alive && self.hostile(team, u.team) && u.mount.is_none() && u.cloak.visible()
                })?;
                (u.capsule().center(), None, None)
            }
            Aim::Vehicle(vh) => {
                let v = self
                    .vehicle(vh)
                    .filter(|v| v.alive && v.team.is_some_and(|t| t != team))?;
                (self.vehicle_blast_sphere(v).0, Some(vh), v.instance)
            }
        };
        if at.distance(from) > view {
            return None;
        }
        crate::perception::los_seat(
            self,
            from,
            at,
            HullFilter::Except([Some(h), target_hull]),
            [own.instance, target_building],
        )
        .then_some(at)
    }

    fn self_turret(&mut self, h: VehicleHandle, p: &mut SelfPilot) {
        let Some(v) = self.vehicle(h).filter(|v| v.alive) else {
            return;
        };
        let Some(team) = v.team else { return };
        let seat = usize::from(p.seat);
        if v.seats.get(seat).is_none_or(|s| s.occupant.is_some()) {
            return;
        }
        let Some(g) = self
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.guns.get(seat))
            .cloned()
            .flatten()
        else {
            return;
        };
        let Some(w) = g.weapons.first() else { return };
        let class = w.class;
        let (pos, rot) = (v.pos, v.rot);
        self.arm(h.index as usize);
        let Some(st) = self
            .vehicle(h)
            .and_then(|v| v.guns.get(seat))
            .and_then(Option::as_ref)
        else {
            return;
        };
        let (look, aim) = (st.look, st.aim);
        let heat = st.weapons.first().map_or(0.0, |w| w.heat);
        let spread = st.weapons.first().map_or(0.0, |w| {
            let wc = &self.weapon_classes[class];
            w.yaw_spread(wc).max(w.pitch_spread(wc))
        });
        let unlimited = w.infinite || self.weapon_classes[class].infinite_ammo();
        let eye = pos + rot * g.muzzle(aim);
        let wc = &self.weapon_classes[class];
        let [min, _, max] = wc.ai_range;
        if self.tick >= p.next_think {
            p.next_think = self.tick + (THINK * crate::TICK_HZ as f32) as u64;
            p.thinks += 1;
            let held = p
                .target
                .and_then(|a| self.self_target(h, a, team, eye, f32::INFINITY));
            if !(p.fighting && held.is_some_and(|at| at.distance(eye) <= max + FIGHT_MARGIN)) {
                let units = self
                    .alive_units()
                    .filter(|(_, u)| wc.ai_target_kind & (1 << u.kind()) != 0)
                    .map(|(h, _)| Aim::Unit(h));
                let vehicles = self
                    .alive_vehicles()
                    .filter(|(_, v)| wc.ai_target_kind & (1 << v.health.health_type as u32) != 0)
                    .map(|(h, _)| Aim::Vehicle(h));
                let found = units
                    .chain(vehicles)
                    .filter_map(|a| {
                        self.self_target(h, a, team, eye, VIEW)
                            .map(|at| (a, at.distance(eye)))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                p.target = found.map(|(a, _)| a);
                p.fighting = found.is_some_and(|(_, d)| (min..=max).contains(&d));
            }
            if p.target.is_none() && p.thinks % LOOK_EVERY == 1 {
                p.look = self.lcg.uniform(-LOOK_SPREAD, LOOK_SPREAD);
            }
        }
        let at = p
            .target
            .and_then(|a| self.self_target(h, a, team, eye, f32::INFINITY));
        let want = at.map_or((p.look, 0.0), |at| gun::angles(rot.inverse() * (at - eye)));
        let mut pad = PadState::default();
        if let Some(t) = g.turn {
            let (s, c) = fmath::sin_cos(gun::wrap(want.0 - look.0));
            let yaw = if c < 0.0 { s.signum() } else { s };
            pad.rx = PadState::quantize(-crate::ai::hover_drive::shaped(
                yaw,
                4.5 - t.yaw_speed.clamp(1.0, 3.0),
            ));
            pad.ry = PadState::quantize(crate::ai::hover_drive::shaped(
                fmath::sin_cos(want.1 - look.1).0,
                5.0 - t.pitch_speed.clamp(1.0, 3.0),
            ));
        }
        let wc = &self.weapon_classes[class];
        let fwd = rot * gun::dir(aim.0, aim.1);
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let skill = self.difficulty_of(team).skill_at(self.bot_skill(team));
        let burst = crate::ai::burst_len(
            self.ground_rules,
            wc,
            crate::unit::UnitHandle {
                index: h.index,
                gen: h.gen,
            },
            self.tick,
            &skill,
        );
        let delay = crate::ai::pause_ticks(self.ground_rules, wc, &skill);
        if p.burst > 0 && crate::ai::heat_spent(wc, heat) {
            p.burst = 0;
            p.shots = 0;
            p.pause_until = self.tick + delay;
        }
        if p.burst == 0 && self.tick >= p.pause_until && crate::ai::heat_fits(wc, burst, heat) {
            p.burst = burst;
            p.shots = 0;
        }
        let fire = p.burst > 0
            && p.fighting
            && self.tick >= p.pause_until
            && at.is_some_and(|at| {
                let d = at.distance(eye);
                (min..=max).contains(&d)
                    && (at - eye).normalize_or_zero().dot(fwd) >= fmath::sin_cos(AIM_CONE).1
                    && !crate::ai::item_line_blocked(
                        self,
                        team,
                        eye,
                        (fwd, right, right.cross(fwd)),
                        at,
                        spread,
                    )
            });
        // The shared weapon update handles cooldown, salvos, heat and ammo.
        // Single-trigger guns receive edges. Unlimited seat weapons burst on
        // heat, never a shot count (PS2 FUN_002bac98, re-soldier-ai §23.2).
        if fire && (!wc.trigger_single || self.tick.is_multiple_of(2)) {
            pad.buttons |= buttons::FIRE;
        }
        let before = self.events.len();
        self.drive_seat_gun(
            Mount {
                vehicle: h,
                seat: p.seat,
            },
            g,
            &pad,
            None,
            at,
        );
        if !unlimited {
            p.shots += self.events[before..]
                .iter()
                .filter(|e| matches!(e, super::Event::Shot { .. }))
                .count() as u32;
        }
        if !unlimited && p.shots >= u32::from(p.burst.max(1)) {
            p.burst = 0;
            p.shots = 0;
            p.pause_until = self.tick + delay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{Body, Shape};
    use crate::sim::{Event, TurretFall};
    use crate::unit::{UnitHandle, UnitSpec};
    use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon, Turn};
    use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    use crate::weapon::{HealthType, WeaponClass};
    use glam::{Mat4, Quat};

    fn world() -> (Sim, UnitHandle) {
        let mut s = Sim::empty(42);
        s.heightfield = Some(crate::sim::tests::flat_ground());
        let mut wc = WeaponClass::bf1_rifle();
        wc.ai_range = [0.0, 30.0, 80.0];
        wc.ordnance.max_damage = 0.0;
        wc.trigger_single = false;
        s.weapon_classes[0] = wc;
        s.seat_setup = vec![Some(SeatSetup {
            radius: 1.0,
            boardable: vec![false],
            body_active: vec![],
            exit_drop: 15.0,
            ai_size: 1,
            guns: vec![Some(GunSetup {
                weapons: vec![GunWeapon {
                    class: 0,
                    infinite: true,
                    reserve_clips: 0,
                }],
                fire_point: Vec3::Y,
                pivot: Vec3::Y,
                eye: Vec3::ZERO,
                aimer: Aimer::default(),
                pilot: false,
                turn: Some(Turn::from_class(
                    None,
                    Some([-45.0, 45.0]),
                    (2.0, 0.5),
                    (10.0, 10.0),
                    gun::Slew::Linear,
                )),
                framed: true,
                aimer_bind: Quat::IDENTITY,
                gunner: None,
            })],
        })];
        s.add_building_turret(
            VehicleSpec {
                kind: VehicleKind::Turret,
                class: 0,
                home_team: Some(0),
                neutral_when_empty: false,
                spawn_pad: None,
                pos: Vec3::ZERO,
                yaw: 0.0,
                max_health: 50.0,
                health_type: HealthType::Building,
                seats: vec![SeatKind::Gunner(0)],
            },
            Quat::IDENTITY,
            None,
            123,
            (TurretFall::Hidden, vec![]),
        );
        s.set_building_self_pilots(123, &[0]);
        let u = s
            .spawn(&UnitSpec::player(1, Vec3::new(0.0, 0.0, 60.0)))
            .unwrap();
        (s, u)
    }

    fn shots(s: &mut Sim, ticks: usize) -> usize {
        (0..ticks)
            .map(|_| {
                s.step(&[PadState::default()]);
                s.events
                    .iter()
                    .filter(|e| matches!(e, Event::Shot { unit, .. } if unit.index == u32::MAX))
                    .count()
            })
            .sum()
    }

    #[test]
    fn self_seat_fires_without_soldier_tracks_outside_range_and_releases_dead_target() {
        let (mut s, u) = world();
        let n = shots(&mut s, 180);
        assert!(
            n > 2,
            "shots={n}, pilot={:?}, unit={:?}, gun={:?}",
            s.building_turrets[0].self_pilots,
            s.unit(u).map(|u| (u.alive, u.soldier.pos, u.kind())),
            s.vehicle(s.building_turrets[0].handle)
                .map(|v| (&v.guns, v.team))
        );
        assert!(s.vehicle(s.building_turrets[0].handle).unwrap().seats[0]
            .occupant
            .is_none());
        s.unit_mut(u).unwrap().soldier.pos.z = 95.0;
        // A salvo committed at the preceding tick boundary may still leave;
        // releasing a cannon does not cancel its already-started cycle.
        assert!(shots(&mut s, 2) <= 1);
        assert_eq!(shots(&mut s, 88), 0);
        assert_eq!(
            s.building_turrets[0].self_pilots[0].target,
            Some(Aim::Unit(u))
        );
        s.unit_mut(u).unwrap().alive = false;
        assert_eq!(shots(&mut s, 30), 0);
        assert_eq!(s.building_turrets[0].self_pilots[0].target, None);
    }

    #[test]
    fn self_seat_respects_team_occlusion_death_and_hashes_controller() {
        let (mut s, u) = world();
        s.unit_mut(u).unwrap().team = 0;
        assert_eq!(shots(&mut s, 60), 0);
        s.unit_mut(u).unwrap().team = 1;
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 2.0, 30.0)),
            Shape::Box {
                half: Vec3::new(5.0, 3.0, 1.0),
            },
        ));
        s.collision.rebuild_broadphase();
        assert_eq!(shots(&mut s, 60), 0);
        let before = s.hash();
        s.building_turrets[0].self_pilots[0].pause_until += 1;
        assert_ne!(s.hash(), before);
        let vh = s.building_turrets[0].handle;
        s.vehicle_mut(vh).unwrap().alive = false;
        assert_eq!(shots(&mut s, 60), 0);
    }
}
