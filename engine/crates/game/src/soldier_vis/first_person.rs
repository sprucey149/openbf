//! First-person soldier visual state, model loading, and draw lists.
//!
//! Renders the player's FPM arms and 1st-person weapon model (`re-first-person-arms.md`).

use super::{fire_point, intern_name, SoldierVisuals, UnitVisual, WeaponAsset};
use crate::assets::{load_skinned_glb, SkinnedAsset};
use crate::first_person::{
    bone_global, clip_name, frame_scale, model_matrix, run_rate, Chooser, Fire, Input, Motion,
    Playback, CAMERA_BONE, WEAPON_BONE,
};
use crate::walk::{TeamTemplate, WalkSim};
use glam::{Mat4, Vec3};
use render::{Camera, Renderer, SkinnedDrawCmd, SkinnedMeshHandle, TextureHandle};
use std::collections::HashMap;
use std::path::Path;

pub struct FpArms {
    pub asset: SkinnedAsset,
    pub meshes: Vec<(SkinnedMeshHandle, Option<TextureHandle>, render::DrawLayer)>,
    pub camera: usize,
    pub weapon: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct FpClass {
    pub arms: Option<usize>,
    pub primary: (Option<usize>, usize),
    pub held: Vec<(u8, Option<usize>, usize)>,
}

impl FpClass {
    pub fn weapon_of(&self, slot: u8) -> (Option<usize>, usize) {
        self.held
            .iter()
            .find(|(s, _, _)| *s == slot)
            .map_or(self.primary, |(_, g, r)| (*g, *r))
    }
}

struct FpState {
    unit: (usize, u32),
    arms: usize,
    chooser: Chooser,
    playback: Playback,
    shot_seen: Option<u64>,
    palette: Vec<Mat4>,
}

#[derive(Default)]
pub struct FirstPersonVisuals {
    pub arms: Vec<FpArms>,
    pub weapons: Vec<WeaponAsset>,
    pub classes: [Vec<FpClass>; 2],
    state: Option<FpState>,
}

impl FirstPersonVisuals {
    pub fn load(
        mut r: Option<&mut Renderer>,
        assets_dir: &Path,
        game: &str,
        teams: &[TeamTemplate; 2],
        cache: &mut HashMap<String, Option<TextureHandle>>,
    ) -> Self {
        let side = assets_dir.join(format!("side_{game}"));
        let mut models = Vec::new();
        let mut guns = Vec::new();
        let mut classes: [Vec<FpClass>; 2] = Default::default();
        for (team, t) in teams.iter().enumerate() {
            for c in &t.classes {
                let primary = &c.first_person_weapon;
                classes[team].push(FpClass {
                    arms: intern_name(&mut models, c.first_person.as_deref()),
                    primary: (
                        intern_name(&mut guns, primary.geometry.as_deref()),
                        primary.row,
                    ),
                    held: c
                        .first_person_held
                        .iter()
                        .map(|(s, w)| (*s, intern_name(&mut guns, w.geometry.as_deref()), w.row))
                        .collect(),
                });
            }
        }
        let mut arms = Vec::new();
        let mut loaded: Vec<Option<usize>> = Vec::new();
        for model in &models {
            loaded.push(
                Self::load_arms(r.as_deref_mut(), &side, model, cache).map(|a| {
                    arms.push(a);
                    arms.len() - 1
                }),
            );
        }
        for row in classes.iter_mut() {
            for c in row.iter_mut() {
                c.arms = c.arms.and_then(|k| loaded[k]);
            }
        }
        let weapons = guns
            .iter()
            .map(|g| SoldierVisuals::load_weapon(r.as_deref_mut(), assets_dir, game, g, cache))
            .collect();
        Self {
            arms,
            weapons,
            classes,
            state: None,
        }
    }

    fn load_arms(
        r: Option<&mut Renderer>,
        side: &Path,
        model: &str,
        cache: &mut HashMap<String, Option<TextureHandle>>,
    ) -> Option<FpArms> {
        let glb = manifest::fpm::glb_path(side, model);
        let asset = match load_skinned_glb(&glb) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("first-person arms {model}: {e:#}");
                return None;
            }
        };
        let Some(camera) = asset.skeleton.index_of(CAMERA_BONE) else {
            eprintln!("first-person arms {model}: no {CAMERA_BONE}");
            return None;
        };
        let mut meshes = Vec::new();
        if let Some(r) = r {
            let parts = match crate::viewer::upload_skinned(r, &asset.mesh) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("first-person arms {model}: {e:#}");
                    return None;
                }
            };
            meshes = parts
                .into_iter()
                .map(|(mesh, mat, layer)| {
                    (
                        mesh,
                        SoldierVisuals::texture(r, side, mat, [0.0; 2], cache),
                        layer,
                    )
                })
                .collect();
        }
        let weapon = asset.skeleton.index_of(WEAPON_BONE);
        println!(
            "first-person arms {model}: {} bones, {} clips",
            asset.skeleton.len(),
            asset.clips.len()
        );
        Some(FpArms {
            asset,
            meshes,
            camera,
            weapon,
        })
    }

    fn class(&self, team: u8, class: u8) -> Option<&FpClass> {
        self.classes.get(usize::from(team))?.get(usize::from(class))
    }

    /// Advance first-person arm animation state machine for the player.
    pub fn update(&mut self, walk: &WalkSim, dt: f32, bf2: bool, units: &[UnitVisual]) {
        let i = walk.player.index as usize;
        let Some(u) = walk
            .sim
            .unit(walk.player)
            .filter(|_| walk.shows_first_person_arms())
        else {
            self.state = None;
            return;
        };
        let Some(class) = self.class(u.team, u.spec.class).cloned() else {
            self.state = None;
            return;
        };
        let Some(arms) = class.arms else {
            self.state = None;
            return;
        };
        let hand = super::in_hand(&walk.sim.weapon_classes, u.active_weapon());
        let (_, row) = class.weapon_of(hand.slot);
        let shot_tick = units.get(i).and_then(|v| v.shot_tick);
        let fresh = self
            .state
            .as_ref()
            .is_none_or(|s| s.unit != (i, u.gen) || s.arms != arms);
        if fresh {
            let mut chooser = Chooser::default();
            chooser.spawned();
            self.state = Some(FpState {
                unit: (i, u.gen),
                arms,
                chooser,
                playback: Playback::default(),
                shot_seen: shot_tick,
                palette: Vec::new(),
            });
        }
        let state = self.state.as_mut().expect("first-person state");
        let shot = shot_tick.is_some() && shot_tick != state.shot_seen;
        state.shot_seen = shot_tick;
        let (_, _, weapon_state) = u.active_weapon();
        let input = Input {
            weapon: hand.class,
            row,
            fire: if shot {
                Fire::Shooting
            } else if weapon_state.reloading() {
                Fire::Reloading
            } else {
                Fire::Idle
            },
            shot,
            motion: Motion::of(u.soldier.stance),
            speed2: u.soldier.vel.length_squared(),
        };
        let a = &self.arms[arms];
        let clips = &a.asset.clips;
        let finished = state.playback.finished(clips);
        let blending = state.playback.blend > 0.0;
        if let Some(play) = state.chooser.step(&input, finished, blending, dt) {
            let name = clip_name(bf2, play.row, play.column);
            let clip = clips.iter().position(|c| c.name.eq_ignore_ascii_case(name));
            state.playback.play(clip, play.looping, play.snap);
        }
        let rate = run_rate(state.chooser.column, input.speed2);
        let skel = &a.asset.skeleton;
        let pose = state.playback.advance(skel, clips, rate, dt);
        state.palette = skel.palette(&pose.locals());
    }

    /// The player's first-person weapon fire point in world coordinates.
    pub fn muzzle(&self, walk: &WalkSim, bf2: bool) -> Option<(usize, Vec3)> {
        let i = walk.player.index as usize;
        let u = walk.sim.unit(walk.player)?;
        if !walk.shows_first_person_arms() {
            return None;
        }
        let state = self.state.as_ref()?;
        let a = self.arms.get(state.arms)?;
        let bone = a.weapon?;
        let class = self.class(u.team, u.spec.class)?;
        let hand = super::in_hand(&walk.sim.weapon_classes, u.active_weapon());
        let (gun, _) = class.weapon_of(hand.slot);
        let weapon = gun.and_then(|g| self.weapons.get(g))?;
        let camera = walk.camera();
        let eye = camera.view().inverse();
        let model = model_matrix(
            eye,
            bone_global(&a.asset.skeleton, &state.palette, a.camera),
            bf2,
            frame_scale(camera.fov_y, bf2),
        );
        let at: Mat4 = model * bone_global(&a.asset.skeleton, &state.palette, bone);
        let name = walk
            .sim
            .weapon_classes
            .get(hand.class)
            .and_then(|w| w.flash.as_ref())
            .map_or(core::flash::DEFAULT_FIRE_POINT, |f| f.fire_point.as_str());
        Some((i, fire_point(at, &weapon.nodes, name)))
    }
}

pub fn shows_arms(u: &core::unit::Unit) -> bool {
    u.first_person_view() && u.zoom.is_none()
}

impl SoldierVisuals {
    pub fn first_person_draws(
        &self,
        walk: &WalkSim,
        camera: &Camera,
    ) -> (Vec<SkinnedDrawCmd>, Vec<render::DrawCmd>) {
        let mut skinned = Vec::new();
        let mut draws = Vec::new();
        let Some(u) = walk
            .sim
            .unit(walk.player)
            .filter(|_| walk.shows_first_person_arms())
        else {
            return (skinned, draws);
        };
        let Some(state) = self.fp.state.as_ref() else {
            return (skinned, draws);
        };
        let Some(a) = self.fp.arms.get(state.arms) else {
            return (skinned, draws);
        };
        let Some(class) = self.fp.class(u.team, u.spec.class) else {
            return (skinned, draws);
        };
        let hand = self.hand_of(u, &walk.sim.weapon_classes);
        let (gun, _) = class.weapon_of(hand.slot);
        let skel = &a.asset.skeleton;
        let eye = camera.view().inverse();
        let bf2 = self.bf2;
        let model = model_matrix(
            eye,
            bone_global(skel, &state.palette, a.camera),
            bf2,
            frame_scale(camera.fov_y, bf2),
        );
        for (mesh, tex, layer) in &a.meshes {
            skinned.push(SkinnedDrawCmd {
                mesh: *mesh,
                texture: *tex,
                color: [1.0; 4],
                model,
                palette: state.palette.clone(),
                layer: *layer,
                light: render::LightPath::Character,
                lit: Some(crate::point_light::soldier_sphere(
                    model.transform_point3(Vec3::ZERO),
                )),
            });
        }
        if let (Some(bone), Some(weapon)) = (a.weapon, gun.and_then(|g| self.fp.weapons.get(g))) {
            let at: Mat4 = model * bone_global(skel, &state.palette, bone);
            if !hand.hidden {
                weapon.part_draws(at, &mut draws);
            }
        }
        (skinned, draws)
    }
}
