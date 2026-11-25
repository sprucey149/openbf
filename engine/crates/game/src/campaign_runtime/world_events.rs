//! Fixed-tick environment actions for Instant Action. Terrain and outcomes
//! remain owned by the native battle mode.

use core::sim::{Event, Sim};
use glam::Mat4;
use manifest::world_events::{Action, Anchor, WorldEvents};

use super::{CampaignRuntime, World};
use crate::{assets::Scene, campaign_script::Value, classdb::fnv1a_lowercase, layout::MapLayout};

#[derive(Clone)]
pub struct WorldEventRuntime {
    world: World,
    transforms: Vec<Mat4>,
    handlers: Vec<(String, Vec<Action>)>,
    pending: Vec<(u64, Vec<Action>)>,
    pub(crate) effects: Vec<(String, Mat4)>,
}

impl WorldEventRuntime {
    pub fn new(
        events: WorldEvents,
        scene: &Scene,
        layout: &MapLayout,
        post_keys: &[String],
        sim: &Sim,
    ) -> Self {
        let mut rt = CampaignRuntime::new(
            Default::default(),
            layout,
            &scene.manifest,
            post_keys,
            Vec::new(),
            sim,
        );
        rt.load_world_anims(&scene.dir, &scene.manifest, sim);
        rt.w.post_loaded = true;
        Self {
            world: rt.w,
            transforms: scene
                .manifest
                .instances
                .iter()
                .map(|i| crate::viewer::instance_matrix(i.rotation, i.translation))
                .collect(),
            handlers: events.object_destroyed,
            pending: vec![(sim.tick, events.start)],
            effects: Vec::new(),
        }
    }

    pub fn animated_instances(&self) -> impl Iterator<Item = u32> + '_ {
        self.world.anims.animated_instances()
    }

    pub fn instance_poses(&self) -> &std::collections::BTreeMap<u32, Mat4> {
        self.world.anims.instance_poses()
    }

    pub fn step(&mut self, sim: &mut Sim) {
        self.effects.clear();
        let destroyed: Vec<u16> = sim
            .events
            .iter()
            .filter_map(|event| match *event {
                Event::StationDestroyed { station, .. } => Some(station),
                _ => None,
            })
            .collect();
        for station in destroyed {
            if let Some(name) = self.world.station_names.get(usize::from(station)) {
                let ready: Vec<_> = self
                    .handlers
                    .iter()
                    .filter(|(object, _)| object.eq_ignore_ascii_case(name))
                    .map(|(_, actions)| actions.clone())
                    .collect();
                for actions in ready {
                    self.run(actions, sim);
                }
            }
        }
        let ready: Vec<_> = self
            .pending
            .extract_if(.., |(at, _)| *at <= sim.tick)
            .map(|(_, actions)| actions)
            .collect();
        for actions in ready {
            self.run(actions, sim);
        }
        self.world.anims.step(sim, core::TICK_DT);
    }

    fn run(&mut self, actions: Vec<Action>, sim: &mut Sim) {
        for action in actions {
            match action {
                Action::PlayAnimation(name) => self.world.anims.play(&name),
                Action::PauseAnimation(name) => self.world.anims.pause(&name),
                Action::KillObject(name) => self.world.kill_object(sim, &Value::str(&name), true),
                Action::Barrier(name, enabled) => {
                    if let Some(nav) = sim.nav.as_mut() {
                        nav.set_barriers(fnv1a_lowercase(&name), enabled);
                    }
                }
                Action::Arc(name, block) => {
                    if let Some(nav) = sim.nav.as_mut() {
                        nav.block_arcs(&[fnv1a_lowercase(&name)], block);
                    }
                }
                Action::Effect(effect, anchor) => {
                    let transform = match anchor {
                        Anchor::Object(name) => self
                            .world
                            .instances
                            .iter()
                            .position(|(object, _, _)| object.eq_ignore_ascii_case(&name))
                            .map(|index| {
                                self.instance_poses()
                                    .get(&(index as u32))
                                    .copied()
                                    .unwrap_or(self.transforms[index])
                            }),
                        Anchor::PathPoint(path, index) => self
                            .world
                            .path(&path)
                            .and_then(|i| self.world.paths[i].1.get(index))
                            .map(|&(pos, yaw)| {
                                Mat4::from_rotation_translation(
                                    glam::Quat::from_rotation_y(yaw),
                                    pos,
                                )
                            }),
                    };
                    if let Some(transform) = transform {
                        self.effects.push((effect, transform));
                    }
                }
                Action::After(seconds, actions)
                    if self.pending.len() < 2048 && (0.0..=3600.0).contains(&seconds) =>
                {
                    let duration = seconds * core::TICK_HZ as f32;
                    let ticks = duration as u64;
                    let ticks = ticks + u64::from((ticks as f32) < duration);
                    self.pending
                        .push((sim.tick.saturating_add(ticks.max(1)), actions));
                }
                Action::After(..) => {}
            }
        }
    }
}
