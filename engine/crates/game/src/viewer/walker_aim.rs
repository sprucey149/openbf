use super::turret_pose::{aimer_node, barrel_node, turret_node};
use crate::skin::Skeleton;
use crate::vehicle_class::{SeatKind, SeatSpec};
use core::vehicle::Vehicle;
use glam::{Mat4, Vec3};

#[derive(Clone)]
enum Writer {
    Turret {
        seat: usize,
        pitch_factor: f32,
    },
    Aimer {
        seat: usize,
        yaw: [f32; 2],
        pitch: [f32; 2],
    },
    Barrel {
        recoil: f32,
        kicks: Vec<RingPoint>,
    },
}

/// A barrel's place in a seat weapon's fire ring: point `point` of `len`.
#[derive(Clone, Copy)]
struct RingPoint {
    seat: usize,
    weapon: usize,
    point: u64,
    len: u64,
}

/// A barrel's recoil state now (`super::turret_pose::recoil_state`) from
/// its seat, weapon, fire-ring point and ring length.
pub type Kick<'a> = &'a dyn Fn(usize, usize, u64, u64) -> f32;

/// Every barrel at rest.
pub fn at_rest(_: usize, _: usize, _: u64, _: u64) -> f32 {
    0.0
}

/// The draw's memory of each vehicle weapon's recent rounds: the fire-ring
/// turn each left on and its tick, kept while a barrel it kicked still
/// slides (`super::turret_pose::RECOIL_DECAY`). The PS2 keeps the state on
/// the aimer's barrel record (B1:192018-192023, B2 MIPS 0x3027f0-0x30282c);
/// nothing the sim reads moves with it, so the port keeps it here.
#[derive(Default)]
pub struct BarrelKicks {
    weapons: std::collections::HashMap<(usize, u32, usize, usize), Rounds>,
}

struct Rounds {
    shots: u64,
    recent: Vec<(u64, u64)>,
    seen: u64,
}

impl BarrelKicks {
    /// Note the rounds every live vehicle's seat weapons fired since the
    /// last step. A tick's earlier rounds take the turns before its last
    /// one (`WeaponState::last_turn`), exact while they pass no turn.
    pub fn step(&mut self, sim: &core::sim::Sim) {
        let keep = (1.0 / super::turret_pose::RECOIL_DECAY / core::TICK_DT).ceil() as u64;
        for (slot, v) in sim.vehicles.iter().enumerate().filter(|(_, v)| v.alive) {
            for (seat, g) in v.guns.iter().enumerate() {
                let Some(g) = g else { continue };
                for (weapon, w) in g.weapons.iter().enumerate() {
                    let r = self
                        .weapons
                        .entry((slot, v.gen, seat, weapon))
                        .or_insert(Rounds {
                            shots: w.shots_fired,
                            recent: Vec::new(),
                            seen: sim.tick,
                        });
                    if w.shots_fired < r.shots {
                        r.shots = w.shots_fired;
                        r.recent.clear();
                    }
                    let new = (w.shots_fired - r.shots).min(8);
                    for k in (0..new).rev() {
                        r.recent.push((w.last_turn.saturating_sub(k), sim.tick));
                    }
                    r.shots = w.shots_fired;
                    r.seen = sim.tick;
                    r.recent.retain(|&(_, t)| sim.tick.saturating_sub(t) < keep);
                }
            }
        }
        self.weapons.retain(|_, r| r.seen == sim.tick);
    }

    /// The recoil state of point `point` of `len` of vehicle `slot`'s
    /// (generation `gen`) seat weapon: its latest round's, 0 with none.
    pub fn state(
        &self,
        tick: u64,
        slot: usize,
        gen: u32,
        seat: usize,
        weapon: usize,
        point: u64,
        len: u64,
    ) -> f32 {
        self.weapons
            .get(&(slot, gen, seat, weapon))
            .and_then(|r| {
                r.recent
                    .iter()
                    .rev()
                    .find(|&&(turn, _)| turn % len.max(1) == point)
            })
            .map_or(0.0, |&(_, t)| {
                super::turret_pose::recoil_state(tick.saturating_sub(t) as f32 * core::TICK_DT)
            })
    }
}

#[derive(Default)]
pub struct WalkerAim {
    writers: Vec<Option<Writer>>,
    seats: Vec<SeatSpec>,
    turret: Vec<Option<usize>>,
}

impl WalkerAim {
    pub fn new(skel: &Skeleton, seats: Vec<SeatSpec>) -> Self {
        let mut writers = vec![None; skel.len()];
        let turret: Vec<_> = seats
            .iter()
            .map(|s| {
                matches!(s.kind, SeatKind::Gunner(_))
                    .then(|| s.turret_node.as_deref().and_then(|n| skel.index_of(n)))
                    .flatten()
            })
            .collect();
        for (seat, spec) in seats.iter().enumerate() {
            if let Some(n) = turret[seat] {
                writers[n] = Some(Writer::Turret {
                    seat,
                    pitch_factor: spec.pitch_turn_factor.unwrap_or(1.0),
                });
            }
        }
        for (seat, spec) in seats.iter().enumerate().rev() {
            for a in &spec.aimers {
                if let Some(n) = skel.index_of(&a.node) {
                    writers[n] = Some(Writer::Aimer {
                        seat,
                        yaw: a.yaw.map(f32::to_radians),
                        pitch: a.pitch.map(f32::to_radians),
                    });
                }
                for &(ref name, recoil) in &a.barrels {
                    if let Some(n) = skel.index_of(name) {
                        writers[n] = Some(Writer::Barrel {
                            recoil,
                            kicks: Vec::new(),
                        });
                    }
                }
            }
        }
        for (seat, spec) in seats.iter().enumerate() {
            for weapon in 0..spec.weapons.len() {
                let ring = kick_ring(skel, spec, weapon);
                let len = ring.len() as u64;
                for (point, n) in ring.into_iter().enumerate() {
                    if let Some(Some(Writer::Barrel { kicks, .. })) = n.map(|n| &mut writers[n]) {
                        kicks.push(RingPoint {
                            seat,
                            weapon,
                            point: point as u64,
                            len,
                        });
                    }
                }
            }
        }
        Self {
            writers,
            seats,
            turret,
        }
    }

    pub fn pose(&self, skel: &Skeleton, walk: &[Mat4], v: &Vehicle) -> Vec<Mat4> {
        self.pose_kicked(skel, walk, v, &at_rest)
    }

    /// [`Self::pose`] with each barrel slid back by its recoil state.
    pub fn pose_kicked(
        &self,
        skel: &Skeleton,
        walk: &[Mat4],
        v: &Vehicle,
        kick: Kick,
    ) -> Vec<Mat4> {
        let mut now = vec![None; skel.len()];
        for n in 0..skel.len() {
            self.resolve(n, skel, walk, v, kick, &mut now);
        }
        now.into_iter().map(Option::unwrap).collect()
    }

    fn resolve(
        &self,
        n: usize,
        skel: &Skeleton,
        walk: &[Mat4],
        v: &Vehicle,
        kick: Kick,
        now: &mut [Option<Mat4>],
    ) -> Mat4 {
        if let Some(m) = now[n] {
            return m;
        }
        let parent = skel.parents[n];
        let p = parent.map_or(Mat4::IDENTITY, |p| {
            self.resolve(p, skel, walk, v, kick, now)
        });
        let old_p = parent.map_or(Mat4::IDENTITY, |p| walk[p]);
        let inherited = p != old_p;
        let carried = p * skel.local_bind[n];
        let mut m = if inherited {
            p * old_p.inverse() * walk[n]
        } else {
            walk[n]
        };
        match self.writers.get(n).and_then(Option::as_ref) {
            Some(Writer::Turret { seat, pitch_factor }) => {
                if let Some(g) = v
                    .guns
                    .get(*seat)
                    .and_then(Option::as_ref)
                    .filter(|g| g.look != (0.0, 0.0) || g.aim != (0.0, 0.0))
                {
                    let look = limited_look(&self.seats[*seat], g.look);
                    m = turret_node(&carried, look.0, -look.1 * pitch_factor);
                }
            }
            Some(Writer::Aimer { seat, yaw, pitch }) => {
                if let Some(g) = v
                    .guns
                    .get(*seat)
                    .and_then(Option::as_ref)
                    .filter(|g| g.look != (0.0, 0.0) || g.aim != (0.0, 0.0))
                {
                    let spec = &self.seats[*seat];
                    let pivot = self.turret[*seat]
                        .or_else(|| spec.aimers.first().and_then(|a| skel.index_of(&a.node)))
                        .map_or(Vec3::ZERO, |k| skel.global_bind[k].w_axis.truncate());
                    let eye = pivot + Vec3::from(spec.eye_point_offset.unwrap_or([0.0; 3]));
                    let target = eye
                        + core::vehicle::gun::dir(g.aim.0, g.aim.1) * core::vehicle::gun::AIM_RAY;
                    let frame = self.turret[*seat].map_or(carried, |t| {
                        let tp = skel.parents[t].map_or(Mat4::IDENTITY, |k| {
                            self.resolve(k, skel, walk, v, kick, now)
                        });
                        let look = limited_look(spec, g.look);
                        turret_node(&(tp * skel.local_bind[t]), look.0, -look.1)
                    });
                    let d = frame
                        .inverse()
                        .transform_vector3(target - carried.w_axis.truncate())
                        .normalize_or_zero();
                    let y = clamp_yaw(d.x.atan2(d.z), *yaw);
                    let x = d.y.clamp(-1.0, 1.0).asin().clamp(pitch[0], pitch[1]);
                    let dir = frame
                        .transform_vector3(core::vehicle::gun::dir(y, x))
                        .normalize_or_zero();
                    m = aimer_node(&p, &skel.local_bind[n], dir);
                }
            }
            Some(Writer::Barrel { recoil, kicks }) => {
                let state = kicks
                    .iter()
                    .map(|k| kick(k.seat, k.weapon, k.point, k.len))
                    .fold(0.0, f32::max);
                let slide = if state > 0.0 { state * recoil } else { 0.0 };
                if inherited || slide > 0.0 {
                    m = barrel_node(&p, skel.local_bind[n].w_axis.truncate(), slide);
                }
            }
            _ => {}
        }
        now[n] = Some(m);
        m
    }
}

/// The barrel node of each point of seat weapon `weapon`'s fire ring, in
/// the ring's order (`crate::vehicle_pads` builds the ring the same way):
/// every firing aimer's barrel slots that name a fire point, or the aimer's
/// own node when it has none. A shot sets its barrel's state to 1 and steps
/// the ring (B1:191494, B2:293613-293632). An aimer authored before any
/// WeaponName is the first weapon's: both setters write the aimer of the
/// current WEAPONSECTION, which starts at 0 (B1:138820, B2:191043).
fn kick_ring(skel: &Skeleton, spec: &SeatSpec, weapon: usize) -> Vec<Option<usize>> {
    let mut ring = Vec::new();
    for a in spec
        .aimers
        .iter()
        .filter(|a| !a.no_fire && (a.weapon == weapon || (weapon == 0 && a.weapon == usize::MAX)))
    {
        let slots: Vec<_> = a
            .slots
            .iter()
            .filter(|s| {
                s.fire_point
                    .as_deref()
                    .is_some_and(|f| skel.index_of(f).is_some())
            })
            .collect();
        if slots.is_empty() {
            if a.slots.iter().all(|s| s.fire_point.is_none()) && skel.index_of(&a.node).is_some() {
                ring.push(None);
            }
            continue;
        }
        ring.extend(
            slots
                .iter()
                .map(|s| s.node.as_deref().and_then(|b| skel.index_of(b))),
        );
    }
    ring
}

fn clamp_yaw(mut angle: f32, limits: [f32; 2]) -> f32 {
    if limits[1] > std::f32::consts::PI && angle < 0.0 {
        angle += std::f32::consts::TAU;
    }
    angle.clamp(limits[0], limits[1])
}

fn limited_look(spec: &SeatSpec, look: (f32, f32)) -> (f32, f32) {
    let yaw = spec
        .yaw_limits
        .filter(|l| l[1] - l[0] < 360.0)
        .map_or(look.0, |l| clamp_yaw(look.0, l.map(f32::to_radians)));
    let pitch = spec
        .pitch_limits
        .filter(|l| l[1] - l[0] < 360.0)
        .map_or(look.1, |l| {
            look.1.clamp(-l[1].to_radians(), -l[0].to_radians())
        });
    (yaw, pitch)
}

#[cfg(test)]
mod tests {
    use super::BarrelKicks;
    use core::vehicle::gun::GunState;
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use core::weapon::{WeaponClass, WeaponState};

    fn sim() -> core::sim::Sim {
        let mut s = core::sim::Sim::empty(1);
        let mut v = VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: glam::Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        }
        .build(1);
        v.guns = vec![
            None,
            Some(GunState {
                look: (0.0, 0.0),
                aim: (0.0, 0.0),
                rate: (0.0, 0.0),
                rel: (0.0, 0.0),
                weapons: vec![WeaponState::new(&WeaponClass::default(), true, 0)],
                zoom: None,
            }),
        ];
        s.vehicles.push(v);
        s
    }

    fn fire(s: &mut core::sim::Sim, rounds: u64) {
        let w = &mut s.vehicles[0].guns[1].as_mut().unwrap().weapons[0];
        w.shots_fired += rounds;
        w.last_turn = w.turns() - 1;
    }

    #[test]
    fn each_round_kicks_its_ring_barrel_for_a_quarter_second() {
        let mut s = sim();
        let gen = s.vehicles[0].gen;
        let mut k = BarrelKicks::default();
        k.step(&s);
        assert_eq!(k.state(s.tick, 0, gen, 1, 0, 0, 2), 0.0);
        fire(&mut s, 2);
        k.step(&s);
        assert_eq!(k.state(s.tick, 0, gen, 1, 0, 0, 2), 1.0);
        assert_eq!(k.state(s.tick, 0, gen, 1, 0, 1, 2), 1.0);
        s.tick += 3;
        fire(&mut s, 1);
        k.step(&s);
        let since = 3.0 * core::TICK_DT;
        assert_eq!(k.state(s.tick, 0, gen, 1, 0, 0, 2), 1.0);
        assert_eq!(
            k.state(s.tick, 0, gen, 1, 0, 1, 2),
            super::super::turret_pose::recoil_state(since)
        );
        s.tick += 60;
        k.step(&s);
        assert_eq!(k.state(s.tick, 0, gen, 1, 0, 0, 2), 0.0);
        s.vehicles[0].gen += 1;
        fire(&mut s, 1);
        k.step(&s);
        assert_eq!(k.state(s.tick, 0, gen + 1, 1, 0, 0, 1), 0.0);
    }
}
