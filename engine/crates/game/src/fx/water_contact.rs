use super::{emitter::Frame, Fx};
use crate::{classdb::ClassDb, soldier_vis::WaterFeet, walk::WalkSim};
use core::fmath::FloatExt;
use glam::{Mat3, Vec3};
use std::collections::HashMap;

#[derive(Default, Debug)]
struct Look {
    splash: Option<String>,
    foot: Option<String>,
    wake: Option<String>,
    min_foot_height: f32,
}

#[derive(Clone, Copy, Default, Debug)]
struct Foot {
    wet: bool,
    height: Option<f32>,
    descending: bool,
}

impl Foot {
    fn step(&mut self, height: f32, minimum: f32) -> bool {
        let previous = self.height.replace(height).unwrap_or(height);
        if previous - height > 0.01 {
            self.descending = true;
        } else if self.descending && height - previous > 0.01 {
            self.descending = false;
            return previous < minimum;
        }
        false
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Track {
    gen: u32,
    live: bool,
    feet: [Foot; 2],
}

#[derive(Default, Debug)]
pub(super) struct State {
    looks: HashMap<String, Look>,
    tracks: Vec<Track>,
    ripple_clock: f32,
    splash_clock: f32,
    tick: Option<u64>,
    feet_tick: Option<u64>,
}

impl State {
    pub fn load(&mut self, db: &ClassDb) {
        for class in db.names() {
            self.looks.insert(
                class.to_owned(),
                Look {
                    splash: db.prop(class, "WaterSplashEffect").map(str::to_owned),
                    foot: db.prop(class, "FootWaterSplashEffect").map(str::to_owned),
                    wake: db.prop(class, "WakeWaterSplashEffect").map(str::to_owned),
                    min_foot_height: db.prop_f32(class, "MinFootHeight").unwrap_or(0.17),
                },
            );
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.ripple_clock = 0.0;
        self.splash_clock = 0.0;
        self.tick = None;
        self.feet_tick = None;
    }
}

fn wades(u: &core::unit::Unit) -> bool {
    u.alive && u.mount.is_none() && u.droideka.is_none()
}

impl State {
    fn look(&self, walk: &WalkSim, u: &core::unit::Unit) -> Option<&Look> {
        self.looks
            .get(&walk.team_template(u.team).class(u.spec.class).name)
    }
}

impl Fx {
    pub(super) fn soldier_water(&mut self, walk: &WalkSim) {
        if self.water_contact.tick == Some(walk.sim.tick) {
            return;
        }
        self.water_contact.tick = Some(walk.sim.tick);
        for u in &walk.sim.units {
            if !wades(u) {
                continue;
            }
            let s = &u.soldier;
            if !walk.sim.water_under(s.pos).is_some_and(|h| h > s.pos.y) {
                continue;
            }
            self.water_contact.ripple_clock += core::TICK_DT;
            let speed_squared = s.vel.length_squared();
            let period = 1.0 / (0.3 * speed_squared + 4.0);
            if self.water_contact.ripple_clock > period {
                self.water_contact.ripple_clock =
                    (self.water_contact.ripple_clock - period).fmin(period);
                if speed_squared > 0.0 {
                    let at = s.pos + s.vel * 0.03;
                    if let Some(height) = walk.sim.still_water_under(s.pos) {
                        self.water.spawn_wake(Vec3::new(at.x, height, at.z), s.vel);
                    }
                }
            }
            self.water_contact.splash_clock += core::TICK_DT;
            if s.vel.y < -5.0 && self.water_contact.splash_clock > 1.0 {
                self.water_contact.splash_clock = 0.0;
                let name = self
                    .water_contact
                    .look(walk, u)
                    .and_then(|l| l.splash.clone());
                if let Some(name) = name {
                    if let Some(height) = walk.sim.still_water_under(s.pos) {
                        self.spawn_floating(&name, s.pos, height);
                    }
                }
            }
        }
    }

    pub(super) fn soldier_water_feet(&mut self, walk: &WalkSim, poses: &[Option<WaterFeet>]) {
        if self.water_contact.feet_tick == Some(walk.sim.tick) {
            return;
        }
        self.water_contact.feet_tick = Some(walk.sim.tick);
        self.water_contact
            .tracks
            .resize(walk.sim.units.len(), Track::default());
        let mut starts = Vec::new();
        for (i, u) in walk.sim.units.iter().enumerate() {
            let t = &mut self.water_contact.tracks[i];
            let pose = poses.get(i).copied().flatten().filter(|_| wades(u));
            let Some(pose) = pose else {
                t.live = false;
                continue;
            };
            if !t.live || t.gen != u.gen {
                *t = Track {
                    gen: u.gen,
                    live: true,
                    ..Track::default()
                };
            }
            let class = &walk.team_template(u.team).class(u.spec.class).name;
            let Some(look) = self.water_contact.looks.get(class) else {
                continue;
            };
            let moving = u.soldier.vel.length_squared() > 0.0;
            for j in 0..2 {
                let at = pose.world[j];
                let foot = &mut t.feet[j];
                let wet = walk.sim.water_under(at).is_some_and(|h| at.y < h);
                if wet && !foot.wet && moving {
                    if let Some(name) = &look.foot {
                        starts.push((name.clone(), Frame::at(at)));
                    }
                }
                foot.wet = wet;
                let step = foot.step(pose.height[j], look.min_foot_height);
                let wake_depth = walk
                    .sim
                    .water_under(u.soldier.pos)
                    .is_some_and(|h| u.soldier.pos.y < h && h < at.y + 0.5);
                if step && wake_depth && u.soldier.grounded && u.soldier.vel.length_squared() > 4.5
                {
                    if let Some(name) = &look.wake {
                        let forward =
                            Vec3::new(u.soldier.vel.x, 0.0, u.soldier.vel.z).normalize_or_zero();
                        if forward != Vec3::ZERO {
                            starts.push((
                                name.clone(),
                                Frame {
                                    rot: Mat3::from_cols(Vec3::Y.cross(forward), Vec3::Y, forward),
                                    pos: u.soldier.pos,
                                    vel: Vec3::ZERO,
                                },
                            ));
                        }
                    }
                }
            }
        }
        for (name, frame) in starts {
            if let Some(level) = walk.sim.still_water_under(frame.pos) {
                if self.spawn_floating(&name, frame.pos, level) {
                    self.live.last_mut().unwrap().frame.rot = frame.rot;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{
        emitter::{Era, Lcg},
        ripple, EffectDef, Library,
    };
    use std::sync::Arc;

    fn fixture() -> (WalkSim, Fx) {
        let mut walk = WalkSim::new(1, Vec3::ZERO, None);
        walk.sim.units.truncate(1);
        walk.sim.units[0].alive = true;
        walk.sim.units[0].soldier.grounded = true;
        walk.sim.water_level = Some(0.5);
        let mut lib = Library::from_sets(Era::Bf2, Vec::new(), &mut Lcg(1));
        for name in ["fall", "foot", "wake"] {
            let hash = crate::classdb::fnv1a_lowercase(name);
            lib.effects.insert(
                hash,
                Arc::new(EffectDef::new(
                    hash,
                    Some(name.into()),
                    Vec::new(),
                    Era::Bf2,
                    &mut Lcg(1),
                )),
            );
        }
        let mut fx = Fx::with_library(lib);
        fx.water = ripple::Water::new(false, Some(ripple::kas1()));
        let u = &walk.sim.units[0];
        let class = walk.team_template(u.team).class(u.spec.class).name.clone();
        fx.water_contact.looks.insert(
            class,
            Look {
                splash: Some("fall".into()),
                foot: Some("foot".into()),
                wake: Some("wake".into()),
                min_foot_height: 0.17,
            },
        );
        (walk, fx)
    }

    fn tick(walk: &mut WalkSim, fx: &mut Fx) {
        walk.sim.tick += 1;
        fx.ingest(walk);
    }

    #[test]
    fn moving_waders_leave_rings_on_the_still_surface_once_per_tick() {
        let (mut walk, mut fx) = fixture();
        walk.sim.units[0].soldier.vel = Vec3::X * 7.0;
        tick(&mut walk, &mut fx);
        assert!(fx.ripples().is_empty());
        tick(&mut walk, &mut fx);
        assert_eq!(fx.ripples().len(), 1);
        let r = fx.ripples()[0];
        assert_eq!((r.life, r.size), (2.0, 0.25));
        assert!((r.at - Vec3::new(0.21, 0.5, 0.0)).length() < 1e-6);
        fx.ingest(&walk);
        assert_eq!(fx.ripples().len(), 1);
        walk.sim.units[0].soldier.vel = Vec3::ZERO;
        for _ in 0..30 {
            tick(&mut walk, &mut fx);
        }
        assert_eq!(fx.ripples().len(), 1);
    }

    #[test]
    fn dry_cells_above_water_and_dead_soldiers_do_not_leave_rings() {
        let (mut walk, mut fx) = fixture();
        walk.sim.units[0].soldier.vel = Vec3::X * 7.0;
        walk.sim.water_mask = Some(core::water::WetMask::new(2, 1.0, [1.0; 2], &[0; 4]).unwrap());
        for _ in 0..10 {
            tick(&mut walk, &mut fx);
        }
        walk.sim.water_mask = None;
        walk.sim.units[0].soldier.pos.y = 0.5;
        for _ in 0..10 {
            tick(&mut walk, &mut fx);
        }
        walk.sim.units[0].soldier.pos.y = 0.0;
        walk.sim.units[0].alive = false;
        for _ in 0..10 {
            tick(&mut walk, &mut fx);
        }
        assert!(fx.ripples().is_empty());
        assert!(fx.live.is_empty());
    }

    #[test]
    fn waders_share_the_ps2_ripple_and_splash_clocks() {
        let (mut walk, mut fx) = fixture();
        let mut second = walk.sim.units[0].spec.build(0, &walk.sim.weapon_classes);
        second.soldier.pos = Vec3::X;
        second.alive = true;
        walk.sim.units.push(second);
        for u in &mut walk.sim.units {
            u.soldier.vel = Vec3::Z * 7.0;
        }
        tick(&mut walk, &mut fx);
        assert_eq!(fx.ripples().len(), 1);
        assert!((fx.ripples()[0].at.x - 1.0).abs() < 1e-6);
        for _ in 0..16 {
            tick(&mut walk, &mut fx);
        }
        for u in &mut walk.sim.units {
            u.soldier.vel = Vec3::NEG_Y * 6.0;
        }
        tick(&mut walk, &mut fx);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].name(), Some("fall"));
    }

    #[test]
    fn falling_splash_uses_the_class_and_the_shared_one_second_clock() {
        let (mut walk, mut fx) = fixture();
        for _ in 0..31 {
            tick(&mut walk, &mut fx);
        }
        walk.sim.units[0].soldier.vel = Vec3::NEG_Y * 6.0;
        tick(&mut walk, &mut fx);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].name(), Some("fall"));
        assert_eq!(fx.live[0].frame.pos, Vec3::new(0.0, 0.52, 0.0));
        for _ in 0..20 {
            tick(&mut walk, &mut fx);
        }
        assert_eq!(fx.live.len(), 1);
        fx.clear();
        assert!(fx.ripples().is_empty());
        assert!(fx.water_contact.tick.is_none());
    }

    #[test]
    fn feet_splash_on_entry_and_wake_when_a_low_foot_lifts() {
        let (mut walk, mut fx) = fixture();
        walk.sim.units[0].soldier.vel = Vec3::NEG_Z * 7.0;
        let mut pose = WaterFeet {
            world: [Vec3::new(-0.1, 0.1, 0.0), Vec3::new(0.1, 0.6, 0.0)],
            height: [0.1, 0.6],
        };
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[0].name(), Some("foot"));
        assert_eq!(fx.live[0].frame.pos, Vec3::new(-0.1, 0.52, 0.0));
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert_eq!(fx.live.len(), 1);
        walk.sim.tick += 1;
        pose.height[0] = 0.05;
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        walk.sim.tick += 1;
        pose.height[0] = 0.1;
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert_eq!(fx.live.len(), 2);
        assert_eq!(fx.live[1].name(), Some("wake"));
        assert_eq!(fx.live[1].frame.rot.z_axis, Vec3::NEG_Z);
        walk.sim.tick += 1;
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert_eq!(fx.live.len(), 2);
    }

    #[test]
    fn stationary_feet_and_a_new_life_do_not_reuse_a_foot_lift() {
        let (mut walk, mut fx) = fixture();
        let mut pose = WaterFeet {
            world: [Vec3::ZERO; 2],
            height: [0.1; 2],
        };
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert!(fx.live.is_empty());
        walk.sim.tick += 1;
        pose.height = [0.05; 2];
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        walk.sim.units[0].gen += 1;
        walk.sim.units[0].soldier.vel = Vec3::X * 7.0;
        walk.sim.tick += 1;
        pose.height = [0.1; 2];
        fx.soldier_water_feet(&walk, &[Some(pose)]);
        assert_eq!(fx.live.len(), 2);
        assert!(fx.live.iter().all(|e| e.name() == Some("foot")));
    }
}
