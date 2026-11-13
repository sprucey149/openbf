//! Render interpolation (card RINT): the sim steps at 30 Hz, the windowed
//! game draws at the display's rate, and a frame between two ticks draws the
//! world blended between them - `lerp(prev, cur, accum / TICK_DT)`, where
//! `accum` is what [`WalkSim::advance_with`]'s accumulator holds after the
//! frame's ticks. Without it a 60 / 144 Hz frame repeats the last tick's
//! picture and motion steps.
//!
//! What is blended (the drawn state the frame's builders read): each unit's
//! feet, yaw and pitch (the chase camera hangs off them, so the camera blends
//! too), each vehicle's place and turn, its guns' looks (and their aimers) and a walker's
//! aim, and the camera's zoom ease (`zoom_s`, `zoom_fov_mix`) and its collision's
//! stored pull-in (`cam_pull`), and modelled projectiles
//! (position and flight direction, matched by producer identity). Not blended: the
//! door / station animations (they step with
//! their ticks), ribbon bolts (their own blend, `BoltVis::draws`), effects and
//! the soldiers' animation clocks (already advanced by the frame's dt).
//!
//! A unit or vehicle snaps - drawn where the tick left it, no blend - when
//! its slot's generation or alive flag changed (a respawn, a death, a new
//! vehicle), a unit's seat changed (boarding, leaving), or it moved farther
//! in the tick than [`UNIT_SNAP_M`] / [`VEHICLE_SNAP_M`] (a teleport).
//!
//! The blend is presentation only. [`Blend::apply`] writes the blended pose
//! into the sim for the frame's draw lists and [`Blend::restore`] writes the
//! tick's own values back, bit for bit, before anything else reads the sim
//! (the tick, the black box, a menu action): no tick ever sees a blended
//! value, so the sim hash is the same with or without it.

#[cfg(test)]
mod projectile_tests;
mod walker;
pub use walker::WalkerPose;

use crate::walk::WalkSim;
use core::sim::{LookPreview, PadState};
use glam::{Quat, Vec3};

/// A unit that moved farther than this in one tick is drawn where it is
/// (150 m/s: no soldier runs or flies that fast; a respawn or a rescue
/// spawn does). PROVISIONAL.
pub const UNIT_SNAP_M: f32 = 5.0;
/// The same for a vehicle (300 m/s). PROVISIONAL.
pub const VEHICLE_SNAP_M: f32 = 10.0;
const WALKER_HEADING_BLEND: f32 = 0.1;
const WALKER_HEADING_SNAP_RAD: f32 = 0.2;

/// One unit's drawn pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitPose {
    pub gen: u32,
    pub alive: bool,
    /// The vehicle and seat it rides (`Unit::mount`), packed; None on foot.
    pub seat: Option<(u32, u8)>,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub melee: Option<(u32, bool, f32)>,
    pub choked: Option<f32>,
}

/// One vehicle's drawn pose.
#[derive(Debug, Clone, PartialEq)]
pub struct VehiclePose {
    pub gen: u32,
    pub alive: bool,
    pub pos: Vec3,
    pub rot: Quat,
    /// `GunState::look`, `GunState::aim` and `GunState::rate` per seat
    /// (None = an unarmed seat): the aimer is what a building turret's gun
    /// draws along, the rate what its gunner's nine-pose leans by.
    pub looks: Vec<Option<[(f32, f32); 3]>>,
    /// `WalkerState::aim_yaw` / `aim_pitch`.
    pub aim: (f32, f32),
    pub walker: Option<WalkerPose>,
    pub flyer_lift: Option<f32>,
}

/// One remote droid's or auto-turret's drawn pose, matched across ticks by its id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemPose {
    pub id: u64,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub up: Vec3,
}

/// A modelled projectile's presentation pose; its slot is only used to write
/// the captured frame back. Matching across ticks uses its launch identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectilePose {
    index: usize,
    key: crate::bolt_vis::Key,
    age: f32,
    pos: Vec3,
    vel: Vec3,
}

/// Everything the blend reads and writes, as one tick left it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pose {
    pub units: Vec<UnitPose>,
    pub vehicles: Vec<VehiclePose>,
    pub droids: Vec<ItemPose>,
    pub turrets: Vec<ItemPose>,
    pub guided: Vec<(u64, Vec3, Quat, [f32; 2])>,
    pub projectiles: Vec<ProjectilePose>,
    pub zoom_s: f32,
    pub zoom_fov_mix: f32,
    pub cam_place: Option<(f32, Vec3)>,
    pub cam_pull: f32,
    pub cam_flyer: Option<crate::walk::FlyerCam>,
    pub cam_gunner: Option<crate::walk::GunnerCam>,
    pub cam_exit: Option<Vec3>,
    pub cam_handover: Option<crate::walk::Handover>,
    pub cam_guided: Option<crate::walk::GuidedCam>,
    /// The death camera's timer, eye and turn.
    pub cam_death: Option<(f32, Vec3, Quat)>,
}

impl Pose {
    /// The walk's drawn state now.
    pub fn capture(walk: &WalkSim) -> Self {
        let mut p = Pose::default();
        p.capture_into(walk);
        p
    }

    /// [`Pose::capture`] into this one's buffers (no allocation when warm,
    /// but a vehicle's looks).
    pub fn capture_into(&mut self, walk: &WalkSim) {
        self.units.clear();
        self.units.extend(walk.sim.units.iter().map(|u| UnitPose {
            gen: u.gen,
            alive: u.alive,
            seat: u.mount.map(|m| (m.vehicle.index, m.seat)),
            pos: u.soldier.pos,
            yaw: u.soldier.yaw,
            pitch: u.soldier.pitch,
            choked: u.choked.map(|c| c.age),
            melee: u.weapon.melee.as_ref().and_then(|m| {
                m.pose().map(|(name, time, _)| {
                    (
                        core::hero_weapon::sound_hash(name),
                        m.block_animation.is_some(),
                        time,
                    )
                })
            }),
        }));
        self.vehicles.clear();
        self.vehicles.extend(walk.sim.vehicles.iter().map(|v| {
            VehiclePose {
                gen: v.gen,
                alive: v.alive,
                pos: v.pos,
                rot: v.rot,
                looks: v
                    .guns
                    .iter()
                    .map(|g| g.as_ref().map(|g| [g.look, g.aim, g.rate]))
                    .collect(),
                aim: (v.walker.aim_yaw, v.walker.aim_pitch),
                flyer_lift: (v.kind == core::vehicle::VehicleKind::Flyer).then_some(v.flyer.lift),
                walker: (v.kind == core::vehicle::VehicleKind::Walker).then(|| {
                    WalkerPose::capture(
                        v,
                        walk.sim
                            .walker_params
                            .get(v.class as usize)
                            .and_then(Option::as_ref),
                    )
                }),
            }
        }));
        self.droids.clear();
        self.droids.extend(walk.sim.droids.iter().map(|d| ItemPose {
            id: d.id,
            pos: d.pos,
            yaw: d.yaw,
            pitch: d.pitch,
            up: d.up,
        }));
        self.turrets.clear();
        self.turrets
            .extend(walk.sim.turrets.iter().map(|t| ItemPose {
                id: t.id,
                pos: t.pos,
                yaw: t.yaw,
                pitch: t.pitch,
                up: t.up,
            }));
        self.guided.clear();
        self.guided.extend(
            walk.sim
                .guided
                .iter()
                .map(|r| (r.id, r.pos, r.rot, [r.flyer.tilt_pitch, r.flyer.tilt_roll])),
        );
        self.projectiles.clear();
        self.projectiles.extend(
            walk.sim
                .bolts
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    walk.sim
                        .weapon_classes
                        .get(b.weapon_class)
                        .is_some_and(|w| {
                            w.ordnance.geometry.is_some()
                                || (w.ordnance.laser.is_none()
                                    && w.ordnance
                                        .impact
                                        .as_ref()
                                        .and_then(|l| l.trail.as_deref())
                                        .is_some())
                        })
                })
                .map(|(index, b)| ProjectilePose {
                    index,
                    key: crate::bolt_vis::key(b),
                    age: b.age,
                    pos: b.pos,
                    vel: b.vel,
                }),
        );
        self.zoom_s = walk.zoom_s;
        self.zoom_fov_mix = walk.zoom_fov_mix;
        self.cam_place = walk.cam_place;
        self.cam_pull = walk.cam_pull;
        self.cam_flyer = walk.cam_flyer;
        self.cam_gunner = walk.cam_gunner;
        self.cam_exit = walk.cam_exit;
        self.cam_handover = walk.cam_handover;
        self.cam_guided = walk.cam_guided;
        self.cam_death = walk.death_cam.map(|d| (d.timer, d.eye, d.rot));
    }

    /// Write this pose into the walk (slots past either length untouched).
    pub fn write(&self, walk: &mut WalkSim) {
        for (u, p) in walk.sim.units.iter_mut().zip(&self.units) {
            u.soldier.pos = p.pos;
            u.soldier.yaw = p.yaw;
            u.soldier.pitch = p.pitch;
            if let (Some(choked), Some(age)) = (&mut u.choked, p.choked) {
                choked.age = age;
            }
            if let (Some(m), Some((_, block, time))) = (&mut u.weapon.melee, p.melee) {
                if block {
                    m.block_animation_time = time;
                } else {
                    m.runtime.time = time;
                }
            }
        }
        for (v, p) in walk.sim.vehicles.iter_mut().zip(&self.vehicles) {
            v.pos = p.pos;
            v.rot = p.rot;
            for (g, l) in v.guns.iter_mut().zip(&p.looks) {
                if let (Some(g), Some([l, a, r])) = (g.as_mut(), l) {
                    (g.look, g.aim, g.rate) = (*l, *a, *r);
                }
            }
            (v.walker.aim_yaw, v.walker.aim_pitch) = p.aim;
            if let Some(lift) = p.flyer_lift {
                v.flyer.lift = lift;
            }
            if let Some(p) = &p.walker {
                p.write(v);
            }
        }
        for (d, p) in walk.sim.droids.iter_mut().zip(&self.droids) {
            if d.id == p.id {
                (d.pos, d.yaw, d.pitch, d.up) = (p.pos, p.yaw, p.pitch, p.up);
            }
        }
        for (t, p) in walk.sim.turrets.iter_mut().zip(&self.turrets) {
            if t.id == p.id {
                (t.pos, t.yaw, t.pitch, t.up) = (p.pos, p.yaw, p.pitch, p.up);
            }
        }
        for (r, p) in walk.sim.guided.iter_mut().zip(&self.guided) {
            if r.id == p.0 {
                (r.pos, r.rot) = (p.1, p.2);
                [r.flyer.tilt_pitch, r.flyer.tilt_roll] = p.3;
            }
        }
        for p in &self.projectiles {
            if let Some(b) = walk.sim.bolts.get_mut(p.index) {
                (b.pos, b.vel) = (p.pos, p.vel);
            }
        }
        walk.zoom_s = self.zoom_s;
        walk.zoom_fov_mix = self.zoom_fov_mix;
        walk.cam_place = self.cam_place;
        walk.cam_pull = self.cam_pull;
        walk.cam_flyer = self.cam_flyer;
        walk.cam_gunner = self.cam_gunner;
        walk.cam_exit = self.cam_exit;
        walk.cam_handover = self.cam_handover;
        walk.cam_guided = self.cam_guided;
        if let (Some(d), Some((_, eye, rot))) = (walk.death_cam.as_mut(), self.cam_death) {
            (d.eye, d.rot) = (eye, rot);
        }
    }
}

fn blend_items(prev: &[ItemPose], out: &mut [ItemPose], t: f32) {
    for o in out.iter_mut() {
        let Some(p) = prev.iter().find(|p| p.id == o.id) else {
            continue;
        };
        if p.pos.distance_squared(o.pos) > UNIT_SNAP_M * UNIT_SNAP_M {
            continue;
        }
        o.pos = mix3(p.pos, o.pos, t);
        o.yaw = lerp_angle(p.yaw, o.yaw, t);
        o.pitch = mix(p.pitch, o.pitch, t);
        o.up = mix3(p.up, o.up, t).normalize_or(o.up);
    }
}

/// `a` toward `b` by `t`; exactly `b` when the two are equal (a still
/// turret's place is matched by equality: `soldier_vis` seat hangs).
fn mix(a: f32, b: f32, t: f32) -> f32 {
    match a == b {
        true => b,
        false => a + (b - a) * t,
    }
}

fn mix3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(mix(a.x, b.x, t), mix(a.y, b.y, t), mix(a.z, b.z, t))
}

/// `a` toward `b` by `t` the short way round (radians, either sign);
/// exactly `b` when the two are equal.
pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    if a == b {
        return b;
    }
    let tau = std::f32::consts::TAU;
    let mut d = (b - a) % tau;
    if d > std::f32::consts::PI {
        d -= tau;
    } else if d < -std::f32::consts::PI {
        d += tau;
    }
    a + d * t
}

fn lerp_look(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (lerp_angle(a.0, b.0, t), mix(a.1, b.1, t))
}

/// The pose drawn at `t` (0 = `prev`, 1 = `cur`) of the way from one tick to
/// the next; each unit or vehicle that snaps is `cur`'s.
pub fn blend(prev: &Pose, cur: &Pose, t: f32) -> Pose {
    let mut out = cur.clone();
    for (o, p) in out.units.iter_mut().zip(&prev.units) {
        let c = *o;
        let same = p.gen == c.gen && p.alive == c.alive && p.seat == c.seat;
        if !same || p.pos.distance_squared(c.pos) > UNIT_SNAP_M * UNIT_SNAP_M {
            continue;
        }
        o.pos = mix3(p.pos, c.pos, t);
        o.yaw = lerp_angle(p.yaw, c.yaw, t);
        o.pitch = mix(p.pitch, c.pitch, t);
        if let Some(age) = &mut o.choked {
            let start = p
                .choked
                .filter(|old| old <= age)
                .unwrap_or((*age - core::TICK_DT).max(0.0));
            *age = mix(start, *age, t);
        }
        if let Some((key, block, time)) = &mut o.melee {
            let start = p
                .melee
                .filter(|old| old.0 == *key && old.1 == *block && old.2 <= *time)
                .map_or((*time - core::TICK_DT).max(0.0), |old| old.2);
            *time = mix(start, *time, t);
        }
    }
    for (o, p) in out.vehicles.iter_mut().zip(&prev.vehicles) {
        if !vehicle_blendable(p, o) {
            continue;
        }
        o.pos = mix3(p.pos, o.pos, t);
        if p.rot != o.rot {
            o.rot = p.rot.slerp(o.rot, t);
        }
        for (l, pl) in o.looks.iter_mut().zip(&p.looks) {
            if let (Some(l), Some(pl)) = (l.as_mut(), pl) {
                *l = [
                    lerp_look(pl[0], l[0], t),
                    lerp_look(pl[1], l[1], t),
                    (mix(pl[2].0, l[2].0, t), mix(pl[2].1, l[2].1, t)),
                ];
            }
        }
        o.aim = lerp_look(p.aim, o.aim, t);
        if let (Some(current), Some(previous)) = (&mut o.flyer_lift, p.flyer_lift) {
            *current = mix(previous, *current, t);
        }
        if let (Some(o), Some(p)) = (o.walker.as_mut(), p.walker.as_ref()) {
            o.blend(p, t);
        }
    }
    blend_items(&prev.droids, &mut out.droids, t);
    blend_items(&prev.turrets, &mut out.turrets, t);
    for r in &mut out.guided {
        if let Some(p) = prev.guided.iter().find(|p| p.0 == r.0) {
            r.1 = mix3(p.1, r.1, t);
            r.2 = p.2.lerp(r.2, t).normalize();
            r.3 = [mix(p.3[0], r.3[0], t), mix(p.3[1], r.3[1], t)];
        }
    }
    let mut previous: Vec<_> = prev.projectiles.iter().collect();
    for o in &mut out.projectiles {
        let Some(i) = previous.iter().position(|p| {
            let same = if o.key.5.is_some() {
                // A shell's changing velocity is part of the fallback key;
                // native producer provenance survives it and slot compaction.
                (p.key.0, p.key.1, p.key.2, p.key.5) == (o.key.0, o.key.1, o.key.2, o.key.5)
            } else {
                p.key == o.key
            };
            same && (p.age + core::TICK_DT - o.age).abs() < 1e-5
        }) else {
            continue;
        };
        let p = previous.remove(i);
        o.pos = mix3(p.pos, o.pos, t);
        o.vel = mix3(p.vel, o.vel, t);
    }
    out.zoom_s = mix(prev.zoom_s, cur.zoom_s, t);
    out.zoom_fov_mix = mix(prev.zoom_fov_mix, cur.zoom_fov_mix, t);
    out.cam_pull = mix(prev.cam_pull, cur.cam_pull, t);
    if let (Some(p), Some(c)) = (prev.cam_guided, cur.cam_guided) {
        if p.rocket == c.rocket {
            out.cam_guided = Some(crate::walk::GuidedCam {
                eye: mix3(p.eye, c.eye, t),
                rot: p.rot.lerp(c.rot, t).normalize(),
                fov: mix(p.fov, c.fov, t),
                ..c
            });
        }
    }
    if let (Some(p), Some(c)) = (prev.cam_flyer, cur.cam_flyer) {
        if p.vehicle == c.vehicle {
            out.cam_flyer = Some(crate::walk::FlyerCam {
                eye: mix3(p.eye, c.eye, t),
                rot: p.rot.lerp(c.rot, t),
                fov: mix(p.fov, c.fov, t),
                ..c
            });
        }
    }
    if let (Some(p), Some(c)) = (prev.cam_gunner, cur.cam_gunner) {
        if (p.vehicle, p.seat) == (c.vehicle, c.seat) {
            out.cam_gunner = Some(crate::walk::GunnerCam {
                eye: mix3(p.eye, c.eye, t),
                rot: crate::sim_pose::quat_slerp(p.rot, c.rot, t),
                ..c
            });
        }
    }
    if let (Some(p), Some(c)) = (prev.cam_exit, cur.cam_exit) {
        out.cam_exit = Some(mix3(p, c, t));
    }
    if let (Some(p), Some(c)) = (prev.cam_handover, cur.cam_handover) {
        out.cam_handover = Some(crate::walk::Handover {
            position: mix3(p.position, c.position, t),
            rot: crate::sim_pose::quat_slerp(p.rot, c.rot, t),
            ..c
        });
    }
    // The same run of the death camera (a replay restarts its timer: snap).
    if let (Some(p), Some(c)) = (prev.cam_death, cur.cam_death) {
        if c.0 >= p.0 {
            out.cam_death = Some((
                c.0,
                mix3(p.1, c.1, t),
                crate::sim_pose::quat_slerp(p.2, c.2, t),
            ));
        }
    }
    if let (Some(p), Some(c)) = (prev.cam_place, cur.cam_place) {
        out.cam_place = Some((mix(p.0, c.0, t), mix3(p.1, c.1, t)));
    }
    out
}

/// The blend's state: the pose before the last tick, and the tick's own
/// values while a blended pose is written in.
#[derive(Debug, Default)]
pub struct Blend {
    /// The pose before the newest tick (None before the first tick).
    prev: Option<Pose>,
    /// Spare buffer for the next `prev`.
    spare: Pose,
    /// The tick's own pose while [`Blend::apply`] has a blend written in.
    saved: Option<Pose>,
    /// The share of the way from the last tick's start to its end the
    /// written blend stands at (None: no blend is written in).
    shown: Option<f32>,
    /// Whether it blends: off by default (headless runs draw ticks), on in
    /// the windowed game unless `OPENBF_RENDER_BLEND=0` ([`Blend::from_env`]).
    pub on: bool,
    /// The share of the next tick's stick turn the last frame drew (None:
    /// no stick preview drew).
    drawn: Option<f32>,
    /// FIRE was down in the last stamped pad.
    fire_down: bool,
    /// The fire lag of a press no tick has taken yet.
    lag: u8,
    walker_heading: Option<(core::vehicle::VehicleHandle, Quat)>,
}

impl Blend {
    /// The windowed game's blend: on unless `OPENBF_RENDER_BLEND=0`.
    pub fn from_env() -> Self {
        Blend {
            on: std::env::var("OPENBF_RENDER_BLEND").map_or(true, |v| v != "0"),
            ..Default::default()
        }
    }

    /// Before a tick steps: keep the pose it starts from.
    pub fn before_tick(&mut self, walk: &WalkSim) {
        debug_assert!(self.saved.is_none(), "a tick stepped under a blend");
        if !self.on {
            return;
        }
        self.spare.capture_into(walk);
        let rider = stabilized_walker(walk);
        self.walker_heading = rider.and_then(|h| {
            let v = walk.sim.vehicle(h)?;
            let filtered = self
                .walker_heading
                .filter(|(old, _)| *old == h)
                .map_or(v.rot, |(_, q)| filtered_walker_heading(q, v.rot));
            self.spare.vehicles.get_mut(h.index as usize)?.rot = filtered;
            Some((h, filtered))
        });
        match self.prev.as_mut() {
            Some(p) => std::mem::swap(p, &mut self.spare),
            None => self.prev = Some(std::mem::take(&mut self.spare)),
        }
    }

    /// A new battle (or the walk was swapped): nothing to blend from.
    pub fn reset(&mut self) {
        debug_assert!(self.saved.is_none());
        self.prev = None;
        self.walker_heading = None;
        (self.drawn, self.fire_down, self.lag) = (None, false, 0);
    }

    /// The frame's pad with the fire lag of a FIRE press: the share of the
    /// turn the reticle was drawn at when it was pressed (the last frame's),
    /// kept until a tick takes it ([`Blend::consumed`]).
    pub fn stamp(&mut self, pad: PadState) -> PadState {
        let down = pad.pressed(core::sim::buttons::FIRE);
        if down && !self.fire_down {
            self.lag = self.drawn.map_or(0, PadState::lag_of);
        }
        self.fire_down = down;
        PadState {
            fire_lag: self.lag,
            ..pad
        }
    }

    /// After the frame's ticks: a stepped tick took the press.
    pub fn consumed(&mut self, ticks: u32) {
        if ticks > 0 {
            self.lag = 0;
        }
    }

    /// Write the pose `t` of the way from the last tick's start to its end
    /// into the walk, keeping the tick's own; [`Blend::restore`] must follow
    /// before anything but drawing reads the walk. Returns whether it wrote.
    pub fn apply(&mut self, walk: &mut WalkSim, t: f32) -> bool {
        if self.saved.is_some() || !self.on {
            return false;
        }
        let Some(prev) = self.prev.as_ref() else {
            return false;
        };
        let cur = Pose::capture(walk);
        let t = t.clamp(0.0, 1.0);
        let mut shown = blend(prev, &cur, t);
        if let Some((h, rot)) = self
            .walker_heading
            .filter(|(h, _)| stabilized_walker(walk) == Some(*h))
        {
            let i = h.index as usize;
            if let (Some(p), Some(c), Some(out)) = (
                prev.vehicles.get(i),
                cur.vehicles.get(i),
                shown.vehicles.get_mut(i),
            ) {
                if vehicle_blendable(p, c) {
                    out.rot = p.rot.slerp(filtered_walker_heading(rot, c.rot), t);
                }
            }
        }
        shown.write(walk);
        self.saved = Some(cur);
        self.shown = Some(t);
        true
    }

    /// Write the tick's own pose back (a no-op when none is blended in).
    pub fn restore(&mut self, walk: &mut WalkSim) {
        self.shown = None;
        if let Some(cur) = self.saved.take() {
            cur.write(walk);
        }
    }

    /// Whether a blended pose is written in now.
    pub fn applied(&self) -> bool {
        self.saved.is_some()
    }

    /// The sim time the walk's pose stands at now, in ticks: the last
    /// tick's own `tick`, or `tick - 1 + t` while a blend `t` of the way into
    /// it is written in.
    pub fn shown_at(&self, tick: u64) -> f64 {
        tick as f64 - self.shown.map_or(0.0, |t| 1.0 - f64::from(t))
    }

    pub fn preview(&mut self, walk: &mut WalkSim, look: LookPreview) {
        if self.saved.is_none() {
            self.saved = Some(Pose::capture(walk));
        }
        match look {
            LookPreview::Foot { unit, yaw, pitch } => {
                if let Some(u) = walk.sim.units.get_mut(unit) {
                    (u.soldier.yaw, u.soldier.pitch) = (yaw, pitch);
                }
            }
            LookPreview::Droid { droid, yaw, pitch } => {
                if let Some(d) = walk.sim.droids.get_mut(droid) {
                    (d.yaw, d.pitch) = (yaw, pitch);
                }
            }
            LookPreview::Seat {
                vehicle,
                seat,
                mut look,
                walker,
            } => {
                let tick = self.saved.as_ref().and_then(|p| p.vehicles.get(vehicle));
                if let (true, Some(t), Some(v)) = (walker, tick, walk.sim.vehicles.get(vehicle)) {
                    let f = (v.rot.inverse() * t.rot) * Vec3::Z;
                    look.0 += core::fmath::atan2(f.x, f.z);
                }
                if let Some(v) = walk.sim.vehicles.get_mut(vehicle) {
                    if let Some(Some(g)) = v.guns.get_mut(seat) {
                        g.look = look;
                    }
                    if walker {
                        (v.walker.aim_yaw, v.walker.aim_pitch) = look;
                    }
                }
            }
        }
    }
}

fn vehicle_blendable(previous: &VehiclePose, current: &VehiclePose) -> bool {
    previous.gen == current.gen
        && previous.alive == current.alive
        && previous.looks.len() == current.looks.len()
        && previous.pos.distance_squared(current.pos) <= VEHICLE_SNAP_M * VEHICLE_SNAP_M
}

fn filtered_walker_heading(previous: Quat, current: Quat) -> Quat {
    if previous.angle_between(current) >= WALKER_HEADING_SNAP_RAD {
        current
    } else {
        previous.slerp(current, WALKER_HEADING_BLEND).normalize()
    }
}

fn stabilized_walker(walk: &WalkSim) -> Option<core::vehicle::VehicleHandle> {
    let mount = walk.sim.unit(walk.player)?.mount?;
    let vehicle = walk.sim.vehicle(mount.vehicle)?;
    if vehicle.kind != core::vehicle::VehicleKind::Walker || mount.seat == 0 {
        return None;
    }
    let driver = vehicle
        .seats
        .first()?
        .occupant
        .and_then(|h| walk.sim.unit(h))?;
    (driver.controller == core::unit::Controller::Ai).then_some(mount.vehicle)
}

/// The frame's blend with the player's look preview drawn in. The mouse's
/// preview is drawn whole. A controller's on foot is drawn the frame's share
/// of the way into the next tick's turn (all of it with the blend off), so
/// the aim moves smoothly; [`Blend::stamp`] fires a press there.
/// The share of the way from the last tick to this one a frame draws at:
/// [`fraction`] with the blend on, 1 (the tick itself) with it off.
pub fn share(walk: &WalkSim) -> f32 {
    if walk.blend.on {
        fraction(walk)
    } else {
        1.0
    }
}

pub fn blend_in_looking(walk: &mut WalkSim, look: &PadState) {
    let mut preview = walk.sim.look_preview(walk.player, look);
    let share = share(walk);
    walk.blend.drawn = None;
    let stick = !look.has_look() && !look.mouse_aim();
    if let (true, Some(LookPreview::Foot { unit, yaw, pitch })) = (stick, preview.as_mut()) {
        if let Some(u) = walk.sim.units.get(*unit) {
            let (y0, p0) = (u.soldier.yaw, u.soldier.pitch);
            *yaw = y0 + (*yaw - y0) * share;
            *pitch = p0 + (*pitch - p0) * share;
            walk.blend.drawn = Some(share);
        }
    }
    walk.blend_in();
    if let Some(p) = preview {
        let mut blend = std::mem::take(&mut walk.blend);
        blend.preview(walk, p);
        walk.blend = blend;
    }
}

/// How far the frame is between the last tick and the next (0..1): the
/// accumulator's left-over over one tick.
pub fn fraction(walk: &WalkSim) -> f32 {
    (walk.accum() / core::TICK_DT).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walker_heading_eases_small_changes_and_snaps_large_ones() {
        let start = Quat::IDENTITY;
        let small = Quat::from_rotation_y(0.1);
        let eased = filtered_walker_heading(start, small);
        assert!((start.angle_between(eased) - 0.01).abs() < 0.0001);
        let teleport = Quat::from_rotation_y(0.3);
        assert_eq!(filtered_walker_heading(start, teleport), teleport);
    }

    fn walk() -> WalkSim {
        let mut w = WalkSim::new(1, Vec3::ZERO, None);
        w.blend.on = true;
        w
    }

    fn me(w: &mut WalkSim) -> &mut core::soldier::Soldier {
        let i = w.player.index as usize;
        &mut w.sim.units[i].soldier
    }

    #[test]
    fn saber_clock_blends_at_display_rates_and_restores_the_simulation() {
        for fps in [60, 75, 120] {
            let mut walk = walk();
            let index = walk.player.index as usize;
            walk.sim.units[index].weapon.melee = Some(core::hero_weapon::MeleeState {
                animation: Some("stand_attack1a".into()),
                ..Default::default()
            });
            walk.sim.units[index].choked = Some(core::hero_weapon::Choked {
                left: 5.0,
                age: 0.0,
            });
            let mut prev = Pose::capture(&walk);
            walk.sim.units[index].choked.as_mut().unwrap().age = core::TICK_DT;
            walk.sim.units[index]
                .weapon
                .melee
                .as_mut()
                .unwrap()
                .runtime
                .time = core::TICK_DT;
            let cur = Pose::capture(&walk);
            let original_hash = walk.sim.hash();
            for frame in 0..fps / 30 {
                let fraction = frame as f32 * 30.0 / fps as f32;
                let shown = blend(&prev, &cur, fraction);
                shown.write(&mut walk);
                let time = walk.sim.units[index]
                    .weapon
                    .melee
                    .as_ref()
                    .unwrap()
                    .pose()
                    .unwrap()
                    .1;
                assert!((time - fraction * core::TICK_DT).abs() < 1e-7);
                assert!((walk.sim.units[index].choked.unwrap().age - time).abs() < 1e-7);
                cur.write(&mut walk);
                assert_eq!(walk.sim.hash(), original_hash);
            }
            prev.units[0].gen += 1;
            assert_eq!(blend(&prev, &cur, 0.5).units[0].melee, cur.units[0].melee);
        }
    }

    struct NoProps;

    impl core::weapon::PropSource for NoProps {
        fn prop(&self, _: &str) -> Option<&str> {
            None
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }

    /// Half a tick on, a unit is drawn at the midpoint of its two ticks and
    /// turned halfway (the short way across +-pi); the tick's own pose comes
    /// back bit for bit.
    #[test]
    fn half_a_tick_draws_the_midpoint() {
        let mut w = walk();
        me(&mut w).pos = Vec3::new(1.0, 0.0, 2.0);
        me(&mut w).yaw = 3.0;
        w.cam_place = Some((1.8, Vec3::new(0.0, 0.0, 3.2)));
        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        me(&mut w).pos = Vec3::new(2.0, 0.5, 4.0);
        me(&mut w).yaw = -3.0;
        w.cam_place = Some((1.8, Vec3::new(0.0, 0.0, 4.0)));
        assert_eq!(b.shown_at(9), 9.0);
        assert!(b.apply(&mut w, 0.5));
        assert_eq!(b.shown_at(9), 8.5);
        assert!(me(&mut w).pos.abs_diff_eq(Vec3::new(1.5, 0.25, 3.0), 1e-6));
        assert_eq!(w.cam_place, Some((1.8, Vec3::new(0.0, 0.0, 3.6))));
        let half = std::f32::consts::PI;
        assert!(
            (me(&mut w).yaw.abs() - half).abs() < 1e-4,
            "{}",
            me(&mut w).yaw
        );
        b.restore(&mut w);
        assert_eq!(me(&mut w).pos, Vec3::new(2.0, 0.5, 4.0));
        assert_eq!(me(&mut w).yaw, -3.0);
        assert_eq!(w.cam_place, Some((1.8, Vec3::new(0.0, 0.0, 4.0))));
        assert!(!b.applied());
        assert_eq!(b.shown_at(9), 9.0);
    }

    #[test]
    fn a_droid_is_drawn_between_its_ticks_and_turned_by_the_preview() {
        let mut w = walk();
        let droid = core::remote::DroidClass::from_props(&NoProps, false);
        let remote = core::remote::RemoteWeapon::new(
            &NoProps,
            droid,
            core::weapon::WeaponClass::bf1_rifle(),
        );
        let d = core::remote::Droid::new(
            7,
            w.player,
            0,
            0,
            &remote,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::ZERO,
            0.5,
            true,
        );
        w.sim.droids.push(d);
        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        let d = &mut w.sim.droids[0];
        d.pos = Vec3::new(1.0, 2.0, -1.0);
        d.yaw = 0.7;
        d.pitch = 0.2;
        assert!(b.apply(&mut w, 0.5));
        let d = &w.sim.droids[0];
        assert!(d.pos.abs_diff_eq(Vec3::new(0.5, 2.0, -0.5), 1e-6));
        assert!((d.yaw - 0.6).abs() < 1e-6 && (d.pitch - 0.1).abs() < 1e-6);
        b.preview(
            &mut w,
            LookPreview::Droid {
                droid: 0,
                yaw: 0.9,
                pitch: 0.3,
            },
        );
        assert_eq!((w.sim.droids[0].yaw, w.sim.droids[0].pitch), (0.9, 0.3));
        b.restore(&mut w);
        let d = &w.sim.droids[0];
        assert_eq!(
            (d.pos, d.yaw, d.pitch),
            (Vec3::new(1.0, 2.0, -1.0), 0.7, 0.2)
        );
    }

    #[test]
    fn a_turret_is_drawn_between_its_ticks() {
        let mut w = walk();
        let class =
            core::autoturret::TurretClass::new(&NoProps, core::weapon::WeaponClass::bf1_rifle());
        let t = core::autoturret::Turret::new(
            3,
            w.player,
            0,
            0,
            &class,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::ZERO,
            0.5,
        );
        w.sim.turrets.push(t);
        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        let t = &mut w.sim.turrets[0];
        let from = t.pos;
        t.pos += Vec3::new(1.0, -1.0, 0.0);
        t.yaw = 0.7;
        t.pitch = 0.2;
        assert!(b.apply(&mut w, 0.5));
        let t = &w.sim.turrets[0];
        assert!(t.pos.abs_diff_eq(from + Vec3::new(0.5, -0.5, 0.0), 1e-6));
        assert!((t.yaw - 0.6).abs() < 1e-6 && (t.pitch - 0.1).abs() < 1e-6);
        b.restore(&mut w);
        let t = &w.sim.turrets[0];
        assert_eq!(
            (t.pos, t.yaw, t.pitch),
            (from + Vec3::new(1.0, -1.0, 0.0), 0.7, 0.2)
        );
    }

    #[test]
    fn flyer_lift_is_drawn_between_ticks() {
        let pose = |lift| Pose {
            vehicles: vec![VehiclePose {
                gen: 1,
                alive: true,
                pos: Vec3::ZERO,
                rot: Quat::IDENTITY,
                looks: Vec::new(),
                aim: (0.0, 0.0),
                walker: None,
                flyer_lift: Some(lift),
            }],
            ..Pose::default()
        };
        let mid = blend(&pose(0.2), &pose(0.8), 0.5);
        assert!((mid.vehicles[0].flyer_lift.unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_guns_aimer_and_rate_are_drawn_between_its_ticks() {
        let pose = |look: f32, aim: f32, rate: f32| Pose {
            vehicles: vec![VehiclePose {
                gen: 1,
                alive: true,
                pos: Vec3::ZERO,
                rot: Quat::IDENTITY,
                looks: vec![None, Some([(look, 0.0), (aim, 0.2), (rate, 0.0)])],
                aim: (0.0, 0.0),
                walker: None,
                flyer_lift: None,
            }],
            ..Pose::default()
        };
        let mid = blend(&pose(0.0, 0.1, -2.0), &pose(0.2, 0.3, 1.0), 0.5);
        let [l, a, r] = mid.vehicles[0].looks[1].unwrap();
        assert!((r.0 + 0.5).abs() < 1e-6, "{r:?}");
        assert!((l.0 - 0.1).abs() < 1e-6, "{l:?}");
        assert!(
            (a.0 - 0.2).abs() < 1e-6 && (a.1 - 0.2).abs() < 1e-6,
            "{a:?}"
        );
        assert_eq!(mid.vehicles[0].looks[0], None);
    }

    /// A respawned unit (a new generation in its slot) and one that moved
    /// farther than a run in the tick are drawn where the tick left them.
    #[test]
    fn a_respawn_or_a_teleport_snaps() {
        let mut w = walk();
        let i = w.player.index as usize;
        me(&mut w).pos = Vec3::ZERO;
        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        w.sim.units[i].gen += 1;
        me(&mut w).pos = Vec3::new(1.0, 0.0, 0.0);
        b.apply(&mut w, 0.5);
        assert_eq!(me(&mut w).pos, Vec3::new(1.0, 0.0, 0.0), "respawn");
        b.restore(&mut w);

        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        me(&mut w).pos = Vec3::new(1.0 + UNIT_SNAP_M + 0.5, 0.0, 0.0);
        b.apply(&mut w, 0.5);
        assert_eq!(me(&mut w).pos.x, 1.0 + UNIT_SNAP_M + 0.5, "teleport");
        b.restore(&mut w);
    }

    #[test]
    fn the_pending_mouse_look_is_drawn_where_the_tick_lands() {
        for on in [true, false] {
            let mut w = walk();
            w.blend.on = on;
            w.advance(1.0 / 30.0, &[PadState::default()]);
            let look = PadState {
                mx: PadState::quantize_look(0.2),
                my: PadState::quantize_look(-0.05),
                buttons: core::sim::buttons::MOUSE_AIM,
                ..PadState::default()
            };
            let tick = (me(&mut w).yaw, me(&mut w).pitch);
            blend_in_looking(&mut w, &look);
            let drawn = (me(&mut w).yaw, me(&mut w).pitch);
            let cam = w.camera();
            w.blend_out();
            assert_eq!((me(&mut w).yaw, me(&mut w).pitch), tick, "restored");
            assert_ne!(drawn, tick);
            assert_eq!(cam.yaw, drawn.0);
            w.step_tick(&[look]);
            assert_eq!((me(&mut w).yaw, me(&mut w).pitch), drawn, "blend {on}");
        }
    }

    #[test]
    fn a_walkers_mouse_aim_keeps_its_heading_while_the_legs_turn_the_hull() {
        use core::vehicle::{gun::dir, hover::quat_y, SeatKind, VehicleKind, VehicleSpec};
        let mut w = walk();
        let i = w
            .sim
            .spawn_vehicle(&VehicleSpec {
                kind: VehicleKind::Walker,
                class: 0,
                home_team: None,
                neutral_when_empty: false,
                spawn_pad: None,
                pos: Vec3::ZERO,
                yaw: 0.0,
                max_health: 1.0,
                health_type: core::weapon::HealthType::Vehicle,
                seats: vec![SeatKind::Pilot],
            })
            .index as usize;
        w.sim.vehicles[i].walker.aim_yaw = 0.3;
        let mut b = Blend {
            on: true,
            ..Default::default()
        };
        b.before_tick(&w);
        let v = &mut w.sim.vehicles[i];
        v.rot = (v.rot * quat_y(-0.1)).normalize();
        v.walker.aim_yaw += 0.1;
        let aim = v.walker.aim_yaw;
        let heading = v.rot * dir(aim, 0.0);
        let tick_rot = v.rot;
        assert!(b.apply(&mut w, 0.25));
        b.preview(
            &mut w,
            LookPreview::Seat {
                vehicle: i,
                seat: 0,
                look: (aim, 0.0),
                walker: true,
            },
        );
        let v = &w.sim.vehicles[i];
        assert!(v.rot.angle_between(tick_rot) > 0.05);
        let drawn = v.rot * dir(v.walker.aim_yaw, 0.0);
        assert!(drawn.angle_between(heading) < 1e-5, "{drawn} {heading}");
        b.restore(&mut w);
        assert_eq!(w.sim.vehicles[i].walker.aim_yaw, aim);
    }

    /// [ours] 2026-10-29: a held right stick draws the aim smoothly - each
    /// frame the share of the next tick's turn it is into that tick - not a
    /// step once a tick. 12 frames a tick over 3 ticks: the drawn yaw only
    /// turns one way and no frame turns it over 1.5x the mean.
    #[test]
    fn a_held_stick_draws_the_aim_smoothly() {
        let pad = PadState {
            rx: PadState::quantize(0.5),
            ..Default::default()
        };
        let mut w = walk();
        w.advance(core::TICK_DT, &[PadState::default()]);
        let dt = core::TICK_DT / 12.0;
        let mut drawn = Vec::new();
        for _ in 0..36 {
            w.advance(dt, &[pad]);
            blend_in_looking(&mut w, &pad);
            let yaw = me(&mut w).yaw;
            assert_eq!(w.camera().yaw, yaw, "the camera follows the aim");
            drawn.push(yaw);
            w.blend_out();
        }
        let steps: Vec<f32> = drawn.windows(2).map(|p| p[0] - p[1]).collect();
        let mean = (drawn[0] - drawn[drawn.len() - 1]) / steps.len() as f32;
        assert!(mean > 0.0, "the stick turned the aim");
        for (k, d) in steps.iter().enumerate() {
            assert!(*d >= 0.0, "frame {k} turned back: {d}");
            assert!(*d <= 1.5 * mean, "frame {k} stepped {d} (mean {mean})");
        }
    }

    /// A FIRE press takes the share of the turn the last frame drew, once:
    /// held, it latches nothing new; a stepped tick clears it; a mouse aim
    /// draws no share.
    #[test]
    fn a_press_takes_the_share_drawn_before_it() {
        use core::sim::buttons::FIRE;
        let stick = PadState {
            rx: PadState::quantize(0.5),
            ..Default::default()
        };
        let fire = PadState {
            buttons: FIRE,
            ..stick
        };
        let mut w = walk();
        w.advance(core::TICK_DT * 1.25, &[PadState::default()]);
        blend_in_looking(&mut w, &stick);
        w.blend_out();
        let mut b = std::mem::take(&mut w.blend);
        assert_eq!(b.stamp(fire).fire_lag, PadState::lag_of(0.25));
        assert_eq!(b.stamp(fire).fire_lag, PadState::lag_of(0.25), "kept");
        b.consumed(0);
        assert_eq!(b.stamp(fire).fire_lag, PadState::lag_of(0.25), "no tick");
        b.consumed(1);
        assert_eq!(b.stamp(fire).fire_lag, 0, "taken");
        assert_eq!(b.stamp(stick).fire_lag, 0);
        w.blend = b;
        let mouse = PadState {
            mx: PadState::quantize_look(0.1),
            buttons: core::sim::buttons::MOUSE_AIM,
            ..PadState::default()
        };
        blend_in_looking(&mut w, &mouse);
        w.blend_out();
        let fired = PadState {
            buttons: FIRE | core::sim::buttons::MOUSE_AIM,
            ..mouse
        };
        assert_eq!(w.blend.stamp(fired).fire_lag, 0, "the mouse");
    }

    /// The sim never reads the blend: a walk drawn blended at 144 Hz (the
    /// pose written in and back every frame) steps to the same sim hash as
    /// one that never blends, and its frames do move between ticks.
    #[test]
    fn the_blend_never_reaches_the_sim() {
        let pad = PadState {
            ly: -20000,
            rx: 9000,
            ..Default::default()
        };
        let mut plain = WalkSim::new(1, Vec3::ZERO, None);
        let mut drawn = walk();
        let dt = 1.0 / 144.0;
        let mut moved_between_ticks = 0;
        let mut last = None;
        for _ in 0..288 {
            plain.advance(dt, &[pad]);
            let ticks = drawn.advance(dt, &[pad]);
            drawn.blend_in();
            let cam = drawn.camera().position;
            if ticks == 0 && last.is_some_and(|l: Vec3| l != cam) {
                moved_between_ticks += 1;
            }
            last = Some(cam);
            drawn.blend_out();
        }
        assert_eq!(plain.sim.hash(), drawn.sim.hash());
        assert_eq!(plain.camera().position, drawn.camera().position);
        assert!(moved_between_ticks > 100, "{moved_between_ticks}");
    }
}
