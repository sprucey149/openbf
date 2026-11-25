//! The items on the field drawn (card PWUP, `analysis/re-powerups.md` §4):
//! every `Sim::pickups` entry as its class's model (`GeometryName`, baked in
//! the common layer: `common_<game>/models/`), upright at the item, facing
//! the heading it was dropped with. BF1 stops drawing an item when 1 s of
//! life is left (hidden for its last 2 s, still there); BF2 draws it to the
//! end. PROVISIONAL: BF2's 1 s fade and the `player_shadow` blob under the
//! item are not drawn.
//!
//! Battle classes can be preloaded before the first frame; a class not
//! preloaded uploads lazily when its item first lies on the field. A model
//! that is not found is reported once and draws nothing.

use super::scene::draw_in;
use super::upload::{upload_submeshes, upload_texture_file};
use crate::assets::load_glb;
use anyhow::Result;
use core::fmath::FloatExt;
use glam::{Mat4, Quat, Vec3};
use render::{DrawCmd, DrawLayer, LightPath, MeshHandle, Renderer, TextureHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// BF1 draws an item only while its life is over this (s; B1 `FUN_001f3cb0`).
pub const BF1_HIDE_LIFE: f32 = 1.0;

/// One drawn submesh of an item model.
#[derive(Clone)]
struct Part {
    mesh: MeshHandle,
    texture: Option<TextureHandle>,
    layer: DrawLayer,
    light: LightPath,
    additive: bool,
    floor_lift: f32,
    /// The model's sphere in its own space: every model draw gathers the
    /// local lights for it (`re-point-lights.md` §5), the item's own
    /// `LightOdf` omni among them (§4).
    sphere: Option<render::LitSphere>,
}

impl Part {
    fn new(
        cpu: &crate::assets::CpuMesh,
        k: usize,
        mesh: MeshHandle,
        texture: Option<TextureHandle>,
        layer: DrawLayer,
        additive: bool,
    ) -> Self {
        let min_y = cpu
            .positions
            .iter()
            .map(|p| p[1])
            .reduce(FloatExt::fmin)
            .unwrap_or(0.0);
        let floor_lift = (-min_y - core::sim::ctf::GROUND_FLAG_CLEARANCE).fmax(0.0);
        Self {
            mesh,
            texture,
            layer,
            light: cpu.light_path(k),
            additive,
            floor_lift,
            sphere: crate::point_light::model_sphere(cpu, Mat4::IDENTITY),
        }
    }

    fn lit(&self, m: Mat4) -> Option<render::LitSphere> {
        self.sphere.map(|s| render::LitSphere {
            centre: m.transform_point3(s.centre),
            radius: s.radius * m.transform_vector3(Vec3::X).length(),
        })
    }
}

/// One banner of a flag model: its `<geometry>.cloths.ron` cloth (baked by
/// `bake::cloth::item_cloths`), the CLTH texture and its triangle list.
struct FlagCloth {
    def: crate::cloth::Definition,
    texture: Option<TextureHandle>,
    indices: Vec<u32>,
}

/// A live flag's banner state: the geometry it was started for (loose or
/// carried), one state per cloth.
struct FlagClothLive {
    geometry: String,
    states: Vec<crate::cloth::State>,
}

/// The cloth frame's only "bone": the flag model's root, at identity (the
/// bake folds `Bind(PRNT)` into the frame).
const FLAG_ROOT: [Mat4; 1] = [Mat4::IDENTITY];

#[derive(Default)]
pub struct FlagVisuals {
    loose: Vec<Part>,
    carried: Vec<Part>,
    ground_loose: Vec<Part>,
    ground_carried: Vec<Part>,
    team_ground: [Option<(Vec<Part>, Vec<Part>)>; 2],
    /// The geometry names behind `ground_loose` / `ground_carried` and each
    /// `team_ground` pair, which key `cloths`.
    ground_geometry: [Option<String>; 2],
    team_geometry: [Option<(String, String)>; 2],
    cloths: HashMap<String, Vec<std::sync::Arc<FlagCloth>>>,
    cloth_live: Vec<Option<FlagClothLive>>,
    cloth_meshes: HashMap<(usize, String, usize), MeshHandle>,
    clock: Option<f32>,
    /// The ground flag class's `DroppedColorize` / `CarriedColorize`
    /// (`com_item_flag`: both `1`), indexed by carried.
    colorize: [bool; 2],
    viewer_team: Option<u8>,
    bf1: bool,
    cull: Option<f32>,
    home_rotation: Quat,
    campaign: HashMap<String, (Vec<Part>, Vec<Part>)>,
    glows: HashMap<String, FlagGlow>,
    /// The map's `Effect("HDR")` is on, so a lens flare lands ×2
    /// (`glow_draws`).
    hdr: bool,
    /// Each unit's drawn `bone_ribcage` ([`Self::set_ribcage`]).
    ribcage: Option<Vec<Option<Mat4>>>,
    /// Each CTF flag's placed class ([`Self::set_ctf_classes`]).
    ctf_classes: Vec<Option<String>>,
}

struct FlagGlow {
    color: [u8; 4],
    intensity: f32,
    radius: f32,
    distance: f32,
    omni: f32,
}

/// The flag draw's colour (`FUN_0035e720`, B2:343900-344050; PC 0x57eff0):
/// with the class's `DroppedColorize` (not carried) or `CarriedColorize`
/// (carried) set, the model and its cloths take the viewer's team-table
/// colour for the flag's team (PS2 table +100 + 4 x team, PC +0x68), as a
/// linear tint of the display bytes (`SceneGpu::marker_tint`'s rule). The
/// one-flag neutral flag is team 0, the table's (192, 192, 192); a 2-flag
/// flag is the viewer's own (friend) or the other team's (enemy). Its
/// alpha, table alpha x fade x min(timer + 1, 1), stays 1: the port resets
/// a flag at timer 0.
pub(crate) fn flag_tint(
    mode: core::sim::CtfMode,
    flag_team: u8,
    viewer_team: Option<u8>,
    bf1: bool,
) -> Option<[f32; 3]> {
    use crate::point_light::Relation;
    let rel = match mode {
        core::sim::CtfMode::OneFlag => Relation::Neutral,
        core::sim::CtfMode::TwoFlag if viewer_team? == flag_team => Relation::Friend,
        core::sim::CtfMode::TwoFlag => Relation::Enemy,
    };
    Some(crate::holo::relation_rgb(rel, bf1).map(|x| render::srgb_decode(f32::from(x) / 255.0)))
}

/// Where a flag draws, and whether it is carried. A loose flag stands at
/// its position. A flag carried on foot is the carrier's posed bone: the
/// soldier draw `FUN_00239bb0` (B2:154800-154846) sets the flag's matrix to
/// the bone the flag's +0x480 CRC names (class +0x3fc, prop 0x826bffda,
/// default 0xe5b51072 = crc32_lower("bone_ribcage")) and ignores
/// `CarriedOffset`; the loose draw `FUN_0035e6f8` runs only without a
/// carrier, so a carrier that is not drawn shows no flag. `ribcage` is
/// `SoldierVisuals::jet_frames` (that bone, by unit index); with none (no
/// soldier models) the flag keeps the offset pose.
/// OURS: a mounted carrier keeps `CarriedOffset` on the vehicle; the vehicle
/// draw `FUN_001f3f58` (B2:105217-105274) poses it on the vehicle's
/// flag +0x484 bone (default 0x698aed8a = "bone_spine3") and draws nothing
/// without one. replace: that bone lookup.
fn flag_pose(
    sim: &core::sim::Sim,
    flag: &core::sim::Flag,
    home_rotation: Quat,
    ribcage: Option<&[Option<Mat4>]>,
) -> Option<(bool, Mat4)> {
    match flag.status {
        core::sim::FlagStatus::AtHome | core::sim::FlagStatus::Dropped { .. } => Some((
            false,
            Mat4::from_rotation_translation(home_rotation, flag.current_pos),
        )),
        core::sim::FlagStatus::Carried { carrier } => {
            let unit = sim.unit(carrier)?;
            let (pos, rot) = match unit.mount {
                Some(mount) => {
                    let vehicle = sim.vehicle(mount.vehicle)?;
                    (vehicle.pos, vehicle.rot)
                }
                None => {
                    if let Some(frames) = ribcage {
                        return Some((true, (*frames.get(carrier.index as usize)?)?));
                    }
                    (unit.soldier.pos, Quat::from_rotation_y(unit.soldier.yaw))
                }
            };
            Some((
                true,
                Mat4::from_rotation_translation(rot, pos + rot * flag.carried_offset),
            ))
        }
    }
}

impl FlagVisuals {
    pub fn load(r: &mut Renderer, scene: &crate::assets::Scene, cull: Option<f32>) -> Self {
        let mut out = Self {
            cull,
            hdr: scene.manifest.hdr.is_some(),
            bf1: scene.manifest.source_game.eq_ignore_ascii_case("bf1"),
            ..Self::default()
        };
        if let Some(instance) = scene
            .manifest
            .instances
            .iter()
            .find(|i| i.class.eq_ignore_ascii_case("com_icon_spaceflag"))
        {
            out.home_rotation = Quat::from_mat4(&super::instance_matrix(
                instance.rotation,
                instance.translation,
            ));
        }
        let mut db = crate::classdb::ClassDb::default();
        let common_dir = scene
            .dir
            .parent()
            .filter(|p| pak::is_dir(p.join(format!("common_{}", scene.manifest.source_game))))
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("assets_baked"));
        let _ = db.load_map(&scene.dir);
        let _ = db.load(
            &common_dir
                .join("classes")
                .join(format!("{}_ingame.ron", scene.manifest.source_game)),
        );
        let set = |key: &str| db.prop_f32("com_item_flag", key).is_some_and(|v| v != 0.0);
        out.colorize = [set("DroppedColorize"), set("CarriedColorize")];
        for class in db
            .names()
            .filter(|name| db.engine_base(name) == Some("flag"))
        {
            if let Some(light) = db.prop(class, "LightOdf") {
                let color: Vec<u8> = db
                    .prop(light, "Color")
                    .unwrap_or("255 255 255 255")
                    .split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect();
                if color.len() == 4 {
                    out.glows.insert(
                        class.to_owned(),
                        FlagGlow {
                            color: [color[0], color[1], color[2], color[3]],
                            intensity: db.prop_f32(light, "FlareIntensity").unwrap_or(0.0),
                            // EntityLight defaults, re-point-lights.md §10.
                            radius: db.prop_f32(light, "ConeInitialWidth").unwrap_or(1.0) * 0.5,
                            distance: db.prop_f32(light, "DrawDistance").unwrap_or(200.0),
                            omni: db.prop_f32(light, "OmniRadius").unwrap_or(0.0),
                        },
                    );
                }
            }
            out.campaign.insert(
                class.to_owned(),
                (
                    load_flag_parts(r, scene, db.prop(class, "GeometryName")),
                    load_flag_parts(r, scene, db.prop(class, "CarriedGeometryName")),
                ),
            );
        }
        // Generic ground flag models from scene or common_<game> (`com_icon_neutral_flag`,
        // faction icon flags, or `com_item_flag` class) so non-space CTF draws
        // actual flags without requiring a placed space-flag instance.
        for name in [
            "com_icon_neutral_flag",
            "com_icon_imperial_flag",
            "com_icon_alliance_flag",
            "com_icon_republic_flag",
            "com_icon_cis_flag",
            "com_icon_flag",
        ] {
            if out.ground_loose.is_empty() {
                out.ground_loose = load_flag_parts(r, scene, Some(name));
                if !out.ground_loose.is_empty() {
                    out.ground_geometry[0] = Some(out.note_cloths(r, scene, name));
                }
            }
            if out.ground_carried.is_empty() {
                let carried = format!("{name}_carried");
                out.ground_carried = load_flag_parts(r, scene, Some(&carried));
                if !out.ground_carried.is_empty() {
                    out.ground_geometry[1] = Some(out.note_cloths(r, scene, &carried));
                }
            }
        }
        if out.ground_loose.is_empty() {
            if let Some((l, c)) = out.campaign.get("com_item_flag") {
                out.ground_loose = l.clone();
                if out.ground_carried.is_empty() {
                    out.ground_carried = c.clone();
                }
            }
        }
        if out.ground_carried.is_empty() {
            out.ground_carried = out.ground_loose.clone();
            out.ground_geometry[1] = out.ground_geometry[0].clone();
        }
        for (team_idx, candidates) in [
            (
                0usize,
                [
                    "com_icon_alliance_flag",
                    "com_icon_republic_flag",
                    "com_icon_neutral_flag",
                ],
            ),
            (
                1usize,
                [
                    "com_icon_imperial_flag",
                    "com_icon_cis_flag",
                    "com_icon_neutral_flag",
                ],
            ),
        ] {
            for name in candidates {
                let loose = load_flag_parts(r, scene, Some(name));
                if !loose.is_empty() {
                    let carried_name = format!("{name}_carried");
                    let c = load_flag_parts(r, scene, Some(&carried_name));
                    let (carried, carried_name) = if c.is_empty() {
                        (loose.clone(), name.to_owned())
                    } else {
                        (c, carried_name)
                    };
                    let names = (
                        out.note_cloths(r, scene, name),
                        out.note_cloths(r, scene, &carried_name),
                    );
                    out.team_ground[team_idx] = Some((loose, carried));
                    out.team_geometry[team_idx] = Some(names);
                    break;
                }
            }
        }
        let additive_bit = crate::fx::draw::additive_bit(
            if scene.manifest.source_game.eq_ignore_ascii_case("bf1") {
                crate::fx::emitter::Era::Bf1
            } else {
                crate::fx::emitter::Era::Bf2
            },
        );
        for (name, carried) in [
            ("com_icon_spaceflag", false),
            ("com_icon_spaceflag_carried", true),
        ] {
            let Some(cpu) = scene.meshes.get(name) else {
                continue;
            };
            let Ok(meshes) = upload_submeshes(r, cpu) else {
                continue;
            };
            let mut parts = Vec::new();
            for (k, (mesh, mat, layer, flags)) in meshes.into_iter().enumerate() {
                let texture = mat.and_then(|name| {
                    scene
                        .textures
                        .get(&name.to_ascii_lowercase())
                        .and_then(|t| super::upload::upload_cpu_texture(r, t, name).ok())
                });
                if texture.is_some() {
                    parts.push(Part::new(
                        cpu,
                        k,
                        mesh,
                        texture,
                        layer,
                        layer == DrawLayer::Blend && flags & additive_bit != 0,
                    ));
                }
            }
            if carried {
                out.carried = parts;
            } else {
                out.loose = parts;
            }
        }
        out
    }

    /// The viewer's sim team, for the flags' relation colour ([`flag_tint`]).
    pub fn set_viewer_team(&mut self, team: Option<u8>) {
        self.viewer_team = team;
    }

    /// Each CTF flag's relation colour for the viewer set last ([`flag_tint`]), by flag: what a
    /// colourised ground flag draws in. Empty without CTF.
    pub fn viewer_tints(&self, sim: &core::sim::Sim) -> Vec<Option<[f32; 3]>> {
        sim.ctf.as_ref().map_or_else(Vec::new, |ctf| {
            ctf.flags
                .iter()
                .map(|f| flag_tint(ctf.mode, f.team, self.viewer_team, self.bf1))
                .collect()
        })
    }

    /// Each unit's drawn `bone_ribcage` by unit index
    /// (`SoldierVisuals::jet_frames`), where a flag carried on foot draws
    /// ([`flag_pose`]); `None` keeps the `CarriedOffset` pose.
    pub fn set_ribcage(&mut self, frames: Option<Vec<Option<Mat4>>>) {
        self.ribcage = frames;
    }

    /// Every live CTF flag's model, and its banners stepped to `sim`'s time
    /// (`share` into the next tick). The cloth steps by the frame's time like
    /// a soldier's (`SoldierVisuals::step_cloths`), in the flag's frame, and
    /// restarts when the flag swaps between its loose and carried model.
    pub fn draws(&mut self, r: &mut Renderer, sim: &core::sim::Sim, share: f32) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        let Some(ctf) = &sim.ctf else {
            self.cloth_live.clear();
            self.clock = None;
            return out;
        };
        let now = (sim.tick as f32 + share) * core::TICK_DT;
        let dt = self.clock.map_or(0.0, |t| now - t);
        self.clock = Some(now);
        let is_space = ctf.is_space();
        self.cloth_live.resize_with(ctf.flags.len(), || None);
        for (index, flag) in ctf.flags.iter().enumerate() {
            let Some((carried, m)) =
                flag_pose(sim, flag, self.home_rotation, self.ribcage.as_deref())
            else {
                self.cloth_live[index] = None;
                continue;
            };
            let team = usize::from(flag.team);
            let (parts, geometry): (&[Part], Option<&String>) = if is_space {
                if carried {
                    (&self.carried, None)
                } else {
                    (&self.loose, None)
                }
            } else if let Some((tl, tc)) = self.team_ground.get(team).and_then(|o| o.as_ref()) {
                let names = self.team_geometry.get(team).and_then(|o| o.as_ref());
                if carried {
                    (tc, names.map(|n| &n.1))
                } else {
                    (tl, names.map(|n| &n.0))
                }
            } else if carried {
                (&self.ground_carried, self.ground_geometry[1].as_ref())
            } else {
                (&self.ground_loose, self.ground_geometry[0].as_ref())
            };
            let tint = (!is_space && self.colorize[usize::from(carried)])
                .then(|| flag_tint(ctf.mode, flag.team, self.viewer_team, self.bf1))
                .flatten();
            let tinted = |mut draw: DrawCmd| {
                if let Some([r, g, b]) = tint {
                    draw.color = [r, g, b, draw.color[3]];
                }
                draw
            };
            // `FUN_0035e720`'s colorize branch: a model with cloths draws its
            // own mesh in `DAT_0076c200` (ff ff ff ff) at the team alpha and
            // only the cloths take the relation colour.
            let banners =
                geometry.is_some_and(|g| self.cloths.get(g).is_some_and(|c| !c.is_empty()));
            let mut lift = 0.0f32;
            for part in parts {
                let pose = if !is_space && !carried && part.floor_lift > 0.0 {
                    lift = lift.fmax(part.floor_lift);
                    Mat4::from_translation(Vec3::Y * part.floor_lift) * m
                } else {
                    m
                };
                let mut draw = match part.texture {
                    Some(texture) => draw_in(part.layer, part.mesh, texture, pose),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], pose),
                }
                .with_cull(self.cull)
                .with_light(part.light)
                .with_lit(part.lit(pose));
                draw.additive = part.additive;
                out.push(if banners { draw } else { tinted(draw) });
            }
            let pose = Mat4::from_translation(Vec3::Y * lift) * m;
            let light = parts.first().map_or(LightPath::Lit, |p| p.light);
            let lit = parts.first().and_then(|p| p.lit(pose));
            let Some(geometry) = geometry.cloned() else {
                self.cloth_live[index] = None;
                continue;
            };
            let defs = self.cloths.get(&geometry).cloned().unwrap_or_default();
            if defs.is_empty() {
                self.cloth_live[index] = None;
                continue;
            }
            let slot = &mut self.cloth_live[index];
            if slot.as_ref().is_none_or(|l| l.geometry != geometry) {
                *slot = Some(FlagClothLive {
                    states: defs
                        .iter()
                        .map(|c| {
                            let seed = c.def.raw.name.bytes().fold(index as u32, |h, b| {
                                (h ^ u32::from(b)).wrapping_mul(0x1000193)
                            });
                            crate::cloth::State::new(&c.def, pose, &FLAG_ROOT, seed)
                        })
                        .collect(),
                    geometry: geometry.clone(),
                });
            }
            let Some(live) = slot.as_mut() else { continue };
            for (k, (c, state)) in defs.iter().zip(&mut live.states).enumerate() {
                state.step(&c.def, pose, &FLAG_ROOT, dt);
                let vertices = state.vertices(&c.def, &FLAG_ROOT);
                let key = (index, geometry.clone(), k);
                let mesh = match crate::cloth::upload(
                    r,
                    self.cloth_meshes.get(&key).copied(),
                    &vertices,
                    &c.indices,
                ) {
                    Ok(mesh) => mesh,
                    Err(e) => {
                        eprintln!("flag cloth upload: {e}");
                        continue;
                    }
                };
                self.cloth_meshes.insert(key, mesh);
                let draw = match c.texture {
                    Some(texture) => DrawCmd::textured(mesh, texture, pose),
                    None => DrawCmd::flat(mesh, [0.6, 0.6, 0.6, 1.0], pose),
                }
                .with_cull(self.cull)
                .with_light(light)
                .with_lit(lit);
                out.push(tinted(draw));
            }
        }
        out
    }

    /// Load `geometry`'s banners once, beside its GLB in the scene or the
    /// common bake (`manifest::cloth::item_path`), and return the key they
    /// sit under. A model with no companion has none; a broken one is
    /// reported and draws its pole alone.
    fn note_cloths(
        &mut self,
        r: &mut Renderer,
        scene: &crate::assets::Scene,
        geometry: &str,
    ) -> String {
        let key = geometry.to_ascii_lowercase();
        if !self.cloths.contains_key(&key) {
            let loaded = load_flag_cloths(r, scene, geometry);
            self.cloths.insert(key.clone(), loaded);
        }
        key
    }

    pub fn set_team_flag_models(
        &mut self,
        r: &mut Renderer,
        scene: &crate::assets::Scene,
        models: &[(Option<String>, Option<String>)],
    ) {
        for (idx, (loose_name, carried_name)) in models.iter().take(2).enumerate() {
            let loose = load_flag_parts(r, scene, loose_name.as_deref());
            let Some(loose_name) = loose_name.as_deref().filter(|_| !loose.is_empty()) else {
                continue;
            };
            let carried = load_flag_parts(r, scene, carried_name.as_deref());
            let (carried, carried_name) = match carried_name.as_deref() {
                Some(name) if !carried.is_empty() => (carried, name),
                _ => (loose.clone(), loose_name),
            };
            let names = (
                self.note_cloths(r, scene, loose_name),
                self.note_cloths(r, scene, carried_name),
            );
            self.team_ground[idx] = Some((loose, carried));
            self.team_geometry[idx] = Some(names);
        }
    }

    /// Each live flag's index in `sim.ctf` and its class: a campaign's
    /// script flags, else the CTF round's placed flags
    /// ([`Self::set_ctf_classes`]). Every flag class carries its
    /// `LightOdf` (EntityItem ctor `FUN_00224fb8`, re-point-lights.md §4:
    /// every CTF flag's is `flaglight`). A carried flag's light is off: the
    /// pickup `FUN_0035d050` (B2:343074-343077) switches the flag's
    /// EntityLight (+0x2b0) off through `FUN_00228570` (active byte +0x2a0
    /// cleared, `FUN_004519b8` takes it off the light manager's list), and
    /// the drop `FUN_0035d2f8` (B2:343271-343278) and the reset home
    /// `FUN_0035d7c8` (B2:343449-343454) switch it on again through
    /// `FUN_00228530` (`FUN_00451948` files it back). Its flare is that
    /// light's, so it goes with it.
    fn glow_flags<'a>(&'a self, walk: &'a crate::walk::WalkSim) -> Vec<(usize, &'a str)> {
        let Some(ctf) = &walk.sim.ctf else {
            return Vec::new();
        };
        let loose = |i: usize| ctf.flags.get(i).is_some_and(|f| f.carrier().is_none());
        let flags: Vec<(usize, &str)> = match &walk.campaign {
            Some(campaign) => campaign.flag_objects().collect(),
            None => self
                .ctf_classes
                .iter()
                .enumerate()
                .filter_map(|(i, c)| Some((i, c.as_deref()?)))
                .collect(),
        };
        flags.into_iter().filter(|&(i, _)| loose(i)).collect()
    }

    /// Each sim CTF flag's placed class, in `sim.ctf`'s flag order.
    pub fn set_ctf_classes(&mut self, classes: Vec<Option<String>>) {
        self.ctf_classes = classes;
    }

    /// The item light's lens flare (re-point-lights.md §10). The light ctor
    /// `FUN_00227150` bakes the flare's alpha byte as `A × FlareIntensity`
    /// (B2 139719–139724; FlareIntensity clamped to 0..1 by the setter,
    /// 140625–140632) and keeps `DrawDistance²` for the cull. The manager
    /// `FUN_00466d08` fades the star's centre alpha by
    /// `(1 − d²/D²) / 0.19` past `0.81·D²` (515485–515488); the ring keeps
    /// the full byte (`FUN_00468238` → `FUN_00468a28`, 515993–516002).
    /// `flaglight`'s 225 255 255 64 × 0.5 gives byte 32, GS alpha 16.
    ///
    /// On an HDR map the flare's bytes skip the HDR scale (no
    /// `FUN_0057fd50` read in `FUN_00466d08` / `FUN_00468238`) while the
    /// scene draws at ×0.5 and the bloom composite (pass priority 0x271a,
    /// B2 504385, after the managers) multiplies by 255/128
    /// (re-glow.md §2.2), so the flare lands ×2; the port's frame is not
    /// half-scale, so the alpha byte doubles here. [D] measured: the PS2
    /// tutorial frame lifts the holocron's surroundings by about +50 luma;
    /// this model predicts about +56 at the centre. PROVISIONAL: that the
    /// LensFlare stage draws before the composite is inferred from the
    /// pass priority, not read off the frame loop. trace: the caller of
    /// `FUN_00466ce8`.
    fn glow_draws(
        &self,
        walk: &crate::walk::WalkSim,
        camera: &render::Camera,
    ) -> Vec<render::ParticleDraw> {
        let Some(ctf) = &walk.sim.ctf else {
            return Vec::new();
        };
        self.glow_flags(walk)
            .into_iter()
            .filter_map(|(index, class)| {
                let def = self.glows.get(class)?;
                let flag = ctf.flags.get(index)?;
                let (_, matrix) =
                    flag_pose(&walk.sim, flag, Quat::IDENTITY, self.ribcage.as_deref())?;
                let pos = matrix.w_axis.truncate();
                let d2 = pos.distance_squared(camera.position);
                let far = def.distance * def.distance;
                if d2 >= far || def.intensity <= 0.0 {
                    return None;
                }
                let alpha =
                    f32::from(def.color[3]) * core::fmath::FloatExt::fmin(def.intensity, 1.0);
                let alpha =
                    core::fmath::FloatExt::fmin(alpha * if self.hdr { 2.0 } else { 1.0 }, 255.0)
                        as u8;
                let fade = if d2 > far * 0.80999994 {
                    (1.0 - d2 / far) * 5.2631564
                } else {
                    1.0
                };
                crate::holo::lens_flare(
                    pos,
                    def.radius,
                    alpha,
                    fade,
                    [def.color[0], def.color[1], def.color[2]],
                    camera,
                )
            })
            .collect()
    }
    /// The item's `LightOdf` EntityLight as a light (EntityItem ctor
    /// `FUN_00224fb8`, re-point-lights.md §4): a non-Static omni of the
    /// light class's Color and OmniRadius — `flaglight`'s 225 255 255, r 10
    /// — that lights the models and the terrain around the flag (or holocron).
    /// At the item's origin: the ctor places the light at the item's +0x150
    /// with an identity rotation (B2:138182-138196), and the item's matrix
    /// setters `FUN_00225de0` / `FUN_00225f88` move it to the new matrix's
    /// translation (`FUN_00452588`, B2:138995-138999).
    fn item_lights(&self, walk: &crate::walk::WalkSim) -> Vec<(Vec3, [u8; 3], f32)> {
        let Some(ctf) = &walk.sim.ctf else {
            return Vec::new();
        };
        self.glow_flags(walk)
            .into_iter()
            .filter_map(|(index, class)| {
                let def = self.glows.get(class).filter(|d| d.omni > 0.0)?;
                let flag = ctf.flags.get(index)?;
                let (_, matrix) =
                    flag_pose(&walk.sim, flag, Quat::IDENTITY, self.ribcage.as_deref())?;
                Some((
                    matrix.w_axis.truncate(),
                    [def.color[0], def.color[1], def.color[2]],
                    def.omni,
                ))
            })
            .collect()
    }
    pub fn campaign_draws(
        &self,
        sim: &core::sim::Sim,
        campaign: &crate::campaign_runtime::CampaignRuntime,
    ) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        let Some(ctf) = &sim.ctf else { return out };
        for (index, class) in campaign.flag_objects() {
            let Some(flag) = ctf.flags.get(index) else {
                continue;
            };
            let Some((loose, carried_parts)) = self.campaign.get(class) else {
                continue;
            };
            let Some((carried, matrix)) =
                flag_pose(sim, flag, Quat::IDENTITY, self.ribcage.as_deref())
            else {
                continue;
            };
            let parts = if carried { carried_parts } else { loose };
            for part in parts {
                let mut draw = match part.texture {
                    Some(texture) => draw_in(part.layer, part.mesh, texture, matrix),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], matrix),
                }
                .with_cull(self.cull)
                .with_light(part.light)
                .with_lit(part.lit(matrix));
                draw.additive = part.additive;
                out.push(draw);
            }
        }
        out
    }
}

impl super::vehicles::VehicleVisuals {
    pub fn campaign_glows(
        &self,
        walk: &crate::walk::WalkSim,
        camera: &render::Camera,
    ) -> Vec<render::ParticleDraw> {
        self.flags.glow_draws(walk, camera)
    }
    pub fn campaign_lights(&self, walk: &crate::walk::WalkSim) -> Vec<(Vec3, [u8; 3], f32)> {
        self.flags.item_lights(walk)
    }
    pub fn campaign_flags(&self, walk: &crate::walk::WalkSim) -> Vec<DrawCmd> {
        walk.campaign
            .as_ref()
            .map(|c| self.flags.campaign_draws(&walk.sim, c))
            .unwrap_or_default()
    }
    pub fn configure_ctf_flags(
        &mut self,
        r: &mut Renderer,
        scene: &crate::assets::Scene,
        specs: &[(Option<String>, Option<String>)],
    ) {
        self.flags.set_team_flag_models(r, scene, specs);
    }

    /// Each CTF flag's placed class, whose `LightOdf` glows on it
    /// (`campaign_glows`, `campaign_lights`).
    pub fn set_ctf_flag_classes(&mut self, classes: Vec<Option<String>>) {
        self.flags.set_ctf_classes(classes);
    }
}

/// `geometry`'s banners from the first of the scene and the common bake that
/// holds its companion, each with its CLTH texture.
fn load_flag_cloths(
    r: &mut Renderer,
    scene: &crate::assets::Scene,
    geometry: &str,
) -> Vec<std::sync::Arc<FlagCloth>> {
    let common = common_dir(scene);
    let Some((dir, path)) = [scene.dir.as_path(), common.as_path()]
        .into_iter()
        .map(|dir| (dir, manifest::cloth::item_path(dir, geometry)))
        .find(|(_, p)| pak::exists(p))
    else {
        return Vec::new();
    };
    let read = || -> Result<Vec<manifest::cloth::Cloth>> {
        Ok(ron::from_str(&pak::read_to_string(&path)?)?)
    };
    let raw = match read() {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!(
                "{}: {e:#}; the flag draws without its banner",
                path.display()
            );
            return Vec::new();
        }
    };
    raw.into_iter()
        .filter_map(|c| {
            let texture = c.texture.clone().and_then(|name| {
                scene
                    .textures
                    .get(&name.to_ascii_lowercase())
                    .and_then(|t| super::upload::upload_cpu_texture(r, t, &name).ok())
                    .or_else(|| {
                        upload_texture_file(r, &dir.join("textures"), &name)
                            .ok()
                            .flatten()
                    })
            });
            let indices = c.triangles.iter().flatten().copied().collect();
            let skel = crate::skin::Skeleton::new(
                vec![c.parent.clone()],
                vec![None],
                vec![Mat4::IDENTITY],
            );
            match crate::cloth::Definition::new(c, &skel) {
                Ok(def) => Some(std::sync::Arc::new(FlagCloth {
                    def,
                    texture,
                    indices,
                })),
                Err(e) => {
                    eprintln!("{}: {e:#}; the flag draws without it", path.display());
                    None
                }
            }
        })
        .collect()
}

fn common_dir(scene: &crate::assets::Scene) -> PathBuf {
    let common_name = format!("common_{}", scene.manifest.source_game);
    scene
        .dir
        .parent()
        .map(|root| root.join(&common_name))
        .filter(|cd| pak::is_dir(cd))
        .unwrap_or_else(|| PathBuf::from("assets_baked").join(&common_name))
}

/// Script-created flags need no placed instance. Their class selects both
/// models, including the tutorial holocron in the common bake.
fn load_flag_parts(
    r: &mut Renderer,
    scene: &crate::assets::Scene,
    geometry: Option<&str>,
) -> Vec<Part> {
    let Some(geometry) = geometry else {
        return Vec::new();
    };
    let common = common_dir(scene);
    let loaded = [scene.dir.as_path(), common.as_path()]
        .into_iter()
        .find_map(|dir| load_glb(&dir.join("models").join(format!("{geometry}.glb"))).ok());
    let Some(cpu) = scene.meshes.get(geometry).or(loaded.as_ref()) else {
        eprintln!("campaign flag model missing: {geometry}");
        return Vec::new();
    };
    let Ok(meshes) = upload_submeshes(r, cpu) else {
        return Vec::new();
    };
    let additive_bit = crate::fx::draw::additive_bit(crate::fx::emitter::Era::Bf2);
    meshes
        .into_iter()
        .enumerate()
        .map(|(k, (mesh, material, layer, flags))| {
            let texture = material.and_then(|name| {
                scene
                    .textures
                    .get(&name.to_ascii_lowercase())
                    .and_then(|t| super::upload::upload_cpu_texture(r, t, name).ok())
                    .or_else(|| {
                        [scene.dir.as_path(), common.as_path()]
                            .into_iter()
                            .find_map(|dir| {
                                upload_texture_file(r, &dir.join("textures"), name)
                                    .ok()
                                    .flatten()
                            })
                    })
            });
            Part::new(
                cpu,
                k,
                mesh,
                texture,
                layer,
                layer == DrawLayer::Blend && flags & additive_bit != 0,
            )
        })
        .collect()
}

/// The item models uploaded so far, by geometry name.
#[derive(Default)]
pub struct PickupVisuals {
    /// `common_<game>`, the map's game's; `None` draws nothing.
    dir: Option<PathBuf>,
    bf1: bool,
    models: HashMap<String, Vec<Part>>,
    textures: HashMap<String, Option<TextureHandle>>,
    cull: Option<f32>,
}

/// Is an item with `life` seconds left drawn (§4)?
pub fn item_drawn(bf1: bool, life: f32) -> bool {
    !bf1 || life > BF1_HIDE_LIFE
}

impl PickupVisuals {
    pub fn new(assets_dir: &Path, game: &str, cull: Option<f32>) -> Self {
        Self {
            dir: Some(assets_dir.join(format!("common_{game}"))),
            bf1: game.eq_ignore_ascii_case("bf1"),
            cull,
            ..Self::default()
        }
    }

    fn upload(&mut self, r: &mut Renderer, geometry: &str) -> Result<Vec<Part>> {
        let dir = self.dir.clone().unwrap_or_default();
        let cpu = load_glb(&dir.join("models").join(format!("{geometry}.glb")))?;
        let mut parts = Vec::new();
        for (k, (mesh, mat, layer, _)) in upload_submeshes(r, &cpu)?.into_iter().enumerate() {
            let texture = match mat.map(str::to_ascii_lowercase) {
                Some(name) => match self.textures.get(&name) {
                    Some(&t) => t,
                    None => {
                        let t = upload_texture_file(r, &dir.join("textures"), &name)?;
                        self.textures.insert(name, t);
                        t
                    }
                },
                None => None,
            };
            if texture.is_none() && super::route_untextured(mat) != super::Untextured::LookupFailed
            {
                continue;
            }
            parts.push(Part::new(&cpu, k, mesh, texture, layer, false));
        }
        Ok(parts)
    }

    /// Upload every item class `sim` can drop (card HITCH): at the battle's
    /// load, so the first powerup a death leaves reads no file in a frame.
    pub fn preload(&mut self, r: &mut Renderer, sim: &core::sim::Sim) {
        if self.dir.is_none() {
            return;
        }
        for c in &sim.item_classes {
            if c.geometry.is_empty() || self.models.contains_key(&c.geometry) {
                continue;
            }
            let parts = self.upload(r, &c.geometry).unwrap_or_else(|e| {
                eprintln!("pickups: {}: {e:#}", c.geometry);
                Vec::new()
            });
            self.models.insert(c.geometry.clone(), parts);
        }
    }

    /// The draws of every item of `sim` that is drawn now.
    pub fn draws(&mut self, r: &mut Renderer, sim: &core::sim::Sim) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        if self.dir.is_none() {
            return out;
        }
        for p in &sim.pickups {
            let Some(c) = sim.item_classes.get(usize::from(p.class)) else {
                continue;
            };
            if c.geometry.is_empty() || !item_drawn(self.bf1, p.life) {
                continue;
            }
            if !self.models.contains_key(&c.geometry) {
                let parts = self.upload(r, &c.geometry).unwrap_or_else(|e| {
                    eprintln!("pickups: {}: {e:#}", c.geometry);
                    Vec::new()
                });
                self.models.insert(c.geometry.clone(), parts);
            }
            let m = Mat4::from_rotation_translation(Quat::from_rotation_y(p.yaw), p.pos);
            for part in &self.models[&c.geometry] {
                let base = match part.texture {
                    Some(t) => draw_in(part.layer, part.mesh, t, m),
                    None => DrawCmd::flat(part.mesh, [0.6, 0.6, 0.6, 1.0], m),
                };
                out.push(
                    base.with_cull(self.cull)
                        .with_light(part.light)
                        .with_lit(part.lit(m)),
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GPU and BF2 common/Geonosis bakes"]
    fn tutorial_holocron_uploads_without_a_placed_space_flag() {
        let root = crate::assets::baked_root();
        let scene = crate::assets::load_scene(&root.join("bf2_geo1")).unwrap();
        let mut r = Renderer::new_headless(320, 240).unwrap();
        let art = FlagVisuals::load(&mut r, &scene, None);
        let (loose, carried) = art.campaign.get("com_item_holocron").expect("flag class");
        assert!(!loose.is_empty(), "invisible loose holocron");
        assert!(!carried.is_empty(), "invisible carried holocron");
        assert!(loose.iter().chain(carried).all(|p| p.texture.is_some()));
        let missions = root.parent().unwrap().join("missions");
        let mission = crate::mission::MissionDef::load_campaign(&missions, "geo1c_c").unwrap();
        let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
        let mut sim = core::sim::Sim::empty(1);
        sim.ctf = Some(core::sim::CtfState::new_ground_one_flag(
            1,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
        ));
        let mut campaign = crate::campaign_runtime::CampaignRuntime::new(
            mission.campaign.unwrap(),
            &layout,
            &scene.manifest,
            &[],
            Vec::new(),
            &sim,
        );
        campaign.w.flags.push(crate::campaign_runtime::FlagEnt {
            name: "tutorial_holocron".into(),
            class: "com_item_holocron".into(),
            ctf: 0,
            dead: false,
        });
        let draws = art.campaign_draws(&sim, &campaign);
        assert!(!draws.is_empty(), "live campaign flag produced no draws");
        let camera = render::Camera {
            position: glam::Vec3::new(0.0, 0.5, 3.0),
            ..Default::default()
        };
        r.render(&camera, &draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (rgba, w, h) = r.capture_rgba().unwrap();
        assert!(
            rgba.chunks_exact(4)
                .filter(|p| p[..3].iter().any(|c| *c > 20))
                .count()
                > 100,
            "holocron rendered blank"
        );
        let out = root.parent().unwrap().join("artifacts/tutorial-ui");
        std::fs::create_dir_all(&out).unwrap();
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(out.join("holocron.png"))
            .unwrap();
        let mut walk = crate::walk::WalkSim::new(1, glam::Vec3::ZERO, None);
        walk.sim = sim;
        walk.campaign = Some(campaign);
        let glows = art.glow_draws(&walk, &camera);
        assert_eq!(glows.len(), 1, "missing LightOdf lens flare");
        r.set_particles(glows);
        r.render(&camera, &draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        let (lit, w, h) = r.capture_rgba().unwrap();
        image::RgbaImage::from_raw(w, h, lit)
            .unwrap()
            .save(out.join("holocron-glow.png"))
            .unwrap();
        let mut campaign = walk.campaign.take().unwrap();
        let sim = walk.sim;
        campaign.w.flags[0].dead = true;
        assert!(art.campaign_draws(&sim, &campaign).is_empty());
    }

    /// The 2-flag banners take the viewer's relation colour, the one-flag
    /// neutral flag the table's neutral grey; with no viewer a 2-flag flag
    /// is left untinted.
    #[test]
    fn flag_tint_follows_the_viewer_relation() {
        use core::sim::CtfMode::{OneFlag, TwoFlag};
        let lin = |rgb: [u8; 3]| rgb.map(|x| render::srgb_decode(f32::from(x) / 255.0));
        assert_eq!(
            flag_tint(TwoFlag, 1, Some(1), false),
            Some(lin([1, 86, 213]))
        );
        assert_eq!(
            flag_tint(TwoFlag, 0, Some(1), false),
            Some(lin([200, 100, 100]))
        );
        assert_eq!(
            flag_tint(OneFlag, 0, Some(1), false),
            Some(lin([192, 192, 192]))
        );
        assert_eq!(
            flag_tint(OneFlag, 0, None, false),
            Some(lin([192, 192, 192]))
        );
        assert_eq!(flag_tint(TwoFlag, 0, None, false), None);
    }

    /// BF1 hides an item for its last 2 s (life 1 -> -1); BF2 never.
    #[test]
    fn bf1_hides_the_item_for_its_last_two_seconds() {
        assert!(item_drawn(true, 30.0));
        assert!(item_drawn(true, 1.01));
        assert!(!item_drawn(true, 1.0));
        assert!(!item_drawn(true, -0.5));
        assert!(item_drawn(false, 1.0));
        assert!(item_drawn(false, -0.9));
    }

    /// On foot the carried flag is the carrier's drawn `bone_ribcage`
    /// (`FUN_00239bb0`), not `CarriedOffset`; an undrawn carrier shows none.
    #[test]
    fn foot_carried_flag_rides_the_carrier_ribcage() {
        let mut sim = core::sim::Sim::empty(1);
        let mut flag = core::sim::Flag::new_team_flag(0, glam::Vec3::ZERO, 15.0);
        let unit = sim
            .spawn(&core::unit::UnitSpec::trooper(
                1,
                glam::Vec3::new(4.0, 0.0, 2.0),
            ))
            .unwrap();
        flag.status = core::sim::FlagStatus::Carried { carrier: unit };
        let bone = Mat4::from_rotation_translation(
            Quat::from_rotation_z(1.2),
            glam::Vec3::new(4.0, 1.3, 2.1),
        );
        let mut frames = vec![None; sim.units.len()];
        frames[unit.index as usize] = Some(bone);
        assert_eq!(
            flag_pose(&sim, &flag, Quat::IDENTITY, Some(&frames)),
            Some((true, bone))
        );
        frames[unit.index as usize] = None;
        assert_eq!(flag_pose(&sim, &flag, Quat::IDENTITY, Some(&frames)), None);
        let (_, offset) = flag_pose(&sim, &flag, Quat::IDENTITY, None).unwrap();
        let soldier = &sim.unit(unit).unwrap().soldier;
        let expect = soldier.pos + Quat::from_rotation_y(soldier.yaw) * flag.carried_offset;
        assert!((offset.w_axis.truncate() - expect).length() < 1e-5);
    }

    #[test]
    fn space_flag_uses_live_state_and_carrier_pose() {
        let mut sim = core::sim::Sim::empty(1);
        let mut flag = core::sim::Flag::new_space_neutral(glam::Vec3::new(1.0, 2.0, 3.0));
        let (carried, home) = flag_pose(&sim, &flag, Quat::IDENTITY, None).unwrap();
        assert!(!carried);
        assert_eq!(home.w_axis.truncate(), flag.home_pos);
        flag.current_pos = glam::Vec3::new(20.0, 30.0, 40.0);
        flag.status = core::sim::FlagStatus::Dropped {
            timer: 5.0,
            pickup_cooldown: 0.0,
        };
        let (carried, dropped) = flag_pose(&sim, &flag, Quat::IDENTITY, None).unwrap();
        assert!(!carried);
        assert_eq!(dropped.w_axis.truncate(), flag.current_pos);
        let unit = sim
            .spawn(&core::unit::UnitSpec::trooper(0, glam::Vec3::ZERO))
            .unwrap();
        let flyer = sim.spawn_vehicle(&core::vehicle::VehicleSpec {
            kind: core::vehicle::VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: glam::Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        });
        assert!(sim.enter_vehicle(unit, flyer, 0));
        flag.status = core::sim::FlagStatus::Carried { carrier: unit };
        let pos = glam::Vec3::new(-8.0, 5.0, 7.0);
        let rot = Quat::from_rotation_z(0.5) * Quat::from_rotation_y(1.0);
        let vehicle = sim.vehicle_mut(flyer).unwrap();
        (vehicle.pos, vehicle.rot) = (pos, rot);
        let (carried, pose) = flag_pose(&sim, &flag, Quat::IDENTITY, None).unwrap();
        assert!(carried);
        assert_eq!(pose.w_axis.truncate(), pos + rot * flag.carried_offset);
        assert!((pose.transform_vector3(glam::Vec3::Z) - rot * glam::Vec3::Z).length() < 0.001);
    }

    /// Regression test for Mustafar (`mus1c_c`) orbital strike beacon:
    /// `mus_flag_bomb`'s loose model `mus1_icon_bomb` references textures
    /// (`tan4_prop_console`, `tan4_prop_console_fx`) that stock `MUS1.LVL`
    /// omits. `FlagVisuals::campaign_draws` must still emit flat-fallback draws
    /// instead of dropping the model while placed.
    #[test]
    #[ignore = "requires GPU and BF2 Mustafar bake"]
    fn mus1_beacon_draws_when_loose_and_when_carried() {
        let root = crate::assets::baked_root();
        let map = root.join("bf2_mus1");
        if !map.join("manifest.ron").exists() {
            return;
        }
        let scene = crate::assets::load_scene(&map).unwrap();
        let mut r = Renderer::new_headless(64, 64).unwrap();
        let art = FlagVisuals::load(&mut r, &scene, None);
        let (loose, carried) = art
            .campaign
            .get("mus_flag_bomb")
            .expect("mus_flag_bomb class loaded");
        assert!(!loose.is_empty(), "mus1_icon_bomb loose parts loaded");
        assert!(!carried.is_empty(), "mus_icon_bomb_carried parts loaded");
        let missions = root.parent().unwrap().join("missions");
        let mission = crate::mission::MissionDef::load_campaign(&missions, "mus1c_c").unwrap();
        let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
        let mut sim = core::sim::Sim::empty(1);
        let unit = sim
            .spawn(&core::unit::UnitSpec::trooper(0, glam::Vec3::ZERO))
            .unwrap();
        sim.ctf = Some(core::sim::CtfState::new_ground_one_flag(
            1,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
        ));
        let mut campaign = crate::campaign_runtime::CampaignRuntime::new(
            mission.campaign.unwrap(),
            &layout,
            &scene.manifest,
            &[],
            Vec::new(),
            &sim,
        );
        campaign.w.flags.push(crate::campaign_runtime::FlagEnt {
            name: "bomb2".into(),
            class: "mus_flag_bomb".into(),
            ctf: 0,
            dead: false,
        });
        let loose_draws = art.campaign_draws(&sim, &campaign);
        assert_eq!(
            loose_draws.len(),
            loose.len(),
            "loose beacon must draw even when its textures are absent from MUS1.LVL"
        );
        sim.ctf.as_mut().unwrap().flags[0].status =
            core::sim::FlagStatus::Carried { carrier: unit };
        let carried_draws = art.campaign_draws(&sim, &campaign);
        assert_eq!(
            carried_draws.len(),
            carried.len(),
            "carried beacon must draw"
        );
    }
}
