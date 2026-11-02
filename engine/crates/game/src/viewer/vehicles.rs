//! The battle's vehicles drawn (card VDRAW): every live `Sim::vehicles` entry
//! as its class's LOD0 model, rigid, at the sim's transform (`pos`, `rot` —
//! the PS2's entity matrix, the same basis a world instance's XFRM gives
//! [`super::instance_matrix`]), through the world path: the GLB material's
//! pass (ALPHA RULE v2), its lighting path and its specular second pass.
//!
//! A class's model is looked up by its `GeometryName`
//! (`crate::vehicle_pads::PadBuild::geometry`) in the map's own bake first —
//! map-local classes, the TAT1 skiff and the BES1 cloud car — then in every
//! side's `side_<game>/vehicles/<side>/` (card VBAKE). Its textures span the
//! same places, the model's own side first: 13 VBAKE models name a texture
//! only another side's LVL ships (re-vehicles §10 item 3).
//!
//! A walker whose side bake wrote its skin (card WSKN) is drawn POSED: its
//! model's vertices are skinned on the CPU each frame by
//! [`super::walker_pose::WalkerRig`] from the integrator's anim state into
//! one dynamic mesh per submesh per live walker, drawn at the body's
//! transform (bone 0 carries the legs' turn). Any other walker draws rigid
//! along its legs' heading (card WALK).

use super::look::far_cull;
use super::scene::{draw_in, spec_cull};
use super::upload::{upload_cpu_texture, upload_submeshes, upload_texture_file};
use super::walker_pose::{skin, WalkerRig};
use crate::assets::{load_glb, load_glb_joints, load_skinned_glb, CpuMesh, Scene};
use anyhow::Result;
use glam::Mat4;
use render::{
    DrawCmd, DrawLayer, LightPath, MeshData, MeshHandle, Renderer, TextureHandle, Vertex,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One drawn submesh of a vehicle model.
struct Part {
    mesh: MeshHandle,
    /// `None` = untextured through [`super::route_untextured`]'s
    /// `LookupFailed` arm (flat grey, as the world draws it).
    texture: Option<TextureHandle>,
    layer: DrawLayer,
    /// The specular second pass's (colour, power), resolved under the map's sun.
    spec: Option<([f32; 3], u32)>,
    light: LightPath,
    /// A skinned walker's submesh (card WSKN): the model's vertices it uses
    /// and its triangles over them.
    skin: Option<(Vec<u32>, Vec<u32>)>,
}

/// A walker class's skin (card WSKN): the rig and the model's bind-pose
/// vertices with each one's bone.
struct Skin {
    rig: WalkerRig,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[u8; 4]>,
    joints: Vec<[u8; 4]>,
}

/// One class's uploaded model.
pub(super) struct ClassVis {
    parts: Vec<Part>,
    skin: Option<Skin>,
    shadows: Vec<(render::ShadowMesh, Option<usize>)>,
}

impl ClassVis {
    pub(super) fn rig(&self) -> Option<&WalkerRig> {
        self.skin.as_ref().map(|s| &s.rig)
    }

    pub(super) fn draws(
        &self,
        out: &mut Vec<DrawCmd>,
        meshes: Option<&[MeshHandle]>,
        m: Mat4,
        cull: (Option<f32>, Option<f32>),
        fade: Option<render::Fade>,
    ) {
        for (k, p) in self.parts.iter().enumerate() {
            let mesh = meshes.map_or(p.mesh, |h| h[k]);
            if let (Some(t), Some((rgb, power))) = (p.texture, p.spec) {
                out.push(
                    DrawCmd::specular(mesh, t, rgb, power, m)
                        .with_cull(cull.1)
                        .with_light(p.light)
                        .with_fade(fade),
                );
            }
            let base = match p.texture {
                Some(t) => draw_in(p.layer, mesh, t, m),
                None => DrawCmd::flat(mesh, [0.6, 0.6, 0.6, 1.0], m),
            };
            out.push(base.with_cull(cull.0).with_light(p.light).with_fade(fade));
        }
    }
}

/// Every vehicle class's uploaded model, by `Vehicle::class`.
#[derive(Default)]
pub struct VehicleVisuals {
    classes: Vec<Option<ClassVis>>,
    bf1_shadows: bool,
    shadows_on: bool,
    shadow_triangles: Vec<[f32; 3]>,
    /// Per `Sim::vehicles` slot: the class its dynamic meshes were made for
    /// and one mesh per part (card WSKN).
    posed: std::collections::HashMap<usize, (u32, Vec<MeshHandle>)>,
    /// The world models' far cull and the specular pass's (`upload_scene`'s).
    cull: Option<f32>,
    spec_cull: Option<f32>,
    /// The items on the field (card PWUP): the battle's other dynamic
    /// models, drawn with the vehicles so every draw site has them.
    items: super::pickups::PickupVisuals,
}

/// One side's baked vehicles: its directory and geometry → LOD0 model.
struct Side {
    dir: PathBuf,
    models: HashMap<String, String>,
    /// Every baked texture (lowercase).
    textures: HashSet<String>,
    /// LOD0 model (lowercase) → the walker record that names it as its
    /// skin (card WSKN).
    walkers: HashMap<String, manifest::ManifestWalker>,
    walker_seats: HashMap<String, Vec<crate::vehicle_class::SeatSpec>>,
}

/// Every side's baked vehicles of one game (`side_<game>/vehicles/*/`), in
/// name order.
pub struct VehicleSides(Vec<Side>);

/// Where a vehicle model was found: the map's own bake (its `models` key)
/// or side `i` of [`VehicleSides`] (the GLB stem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    Map(String),
    Side(usize, String),
}

/// Where a vehicle texture was found: the map's own textures, or side `i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TexSource {
    Map,
    Side(usize),
}

fn visibility_opacity(walker: bool, opacity: f32) -> f32 {
    if walker {
        opacity.max(crate::camera_collide::SEE_THROUGH_OPACITY)
    } else {
        opacity
    }
}

fn lod0(g: &manifest::ManifestGeometry) -> &str {
    g.lods
        .iter()
        .find(|(tier, _)| tier.eq_ignore_ascii_case("LOD0"))
        .map_or(g.model.as_str(), |(_, m)| m.as_str())
}

impl VehicleSides {
    /// Every `side_<game>/vehicles/<side>/vehicles.ron` under `assets_dir`
    /// (a side that fails to parse is reported and skipped).
    pub fn load(assets_dir: &Path, game: &str) -> Self {
        let root = assets_dir.join(format!("side_{game}")).join("vehicles");
        let mut dirs: Vec<PathBuf> = pak::read_dir(&root)
            .into_iter()
            .flatten()
            .filter(|p| pak::is_file(p.join("vehicles.ron")))
            .collect();
        dirs.sort();
        Self(
            dirs.into_iter()
                .filter_map(|dir| {
                    let text = pak::read_to_string(dir.join("vehicles.ron")).ok()?;
                    match ron::from_str::<manifest::VehicleManifest>(&text) {
                        Ok(m) => {
                            let mut db = crate::classdb::ClassDb::default();
                            let _ = db.load(
                                &assets_dir
                                    .join("classes")
                                    .join(format!("{game}_{}.ron", m.source)),
                            );
                            let walker_seats = m
                                .vehicles
                                .iter()
                                .filter_map(|v| {
                                    Some((
                                        v.walker.as_ref()?.skin.as_ref()?.to_ascii_lowercase(),
                                        crate::vehicle_class::seats(&db, &v.class.name),
                                    ))
                                })
                                .collect();
                            Some(Side {
                                walker_seats,
                                models: m
                                    .geometries
                                    .iter()
                                    .map(|g| (g.name.to_ascii_lowercase(), lod0(g).to_owned()))
                                    .collect(),
                                textures: m
                                    .textures
                                    .iter()
                                    .map(|t| t.to_ascii_lowercase())
                                    .collect(),
                                walkers: m
                                    .vehicles
                                    .iter()
                                    .filter_map(|v| v.walker.as_ref())
                                    .filter_map(|w| {
                                        Some((w.skin.as_ref()?.to_ascii_lowercase(), w.clone()))
                                    })
                                    .collect(),
                                dir,
                            })
                        }
                        Err(e) => {
                            eprintln!("vehicles: {}: {e}", dir.display());
                            None
                        }
                    }
                })
                .collect(),
        )
    }

    /// How many sides were found.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Side `i`'s directory.
    pub fn dir(&self, i: usize) -> &Path {
        &self.0[i].dir
    }
}

/// A `GeometryName`'s LOD0 model: the map's own bake first (its
/// `geometries` row, else the name itself, among its `models`), then the
/// first side that bakes that geometry.
pub fn find_model(
    m: &manifest::Manifest,
    sides: &VehicleSides,
    geometry: &str,
) -> Option<ModelSource> {
    let key = geometry.trim().to_ascii_lowercase();
    let want = m
        .geometries
        .iter()
        .find(|g| g.name.eq_ignore_ascii_case(&key))
        .map_or(key.as_str(), lod0);
    if let Some(k) = m.models.iter().find(|k| k.eq_ignore_ascii_case(want)) {
        return Some(ModelSource::Map(k.clone()));
    }
    sides
        .0
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.models.get(&key).map(|m| ModelSource::Side(i, m.clone())))
}

/// A texture by name for a model from `home` (`None` = the map): the
/// model's own side, the map, then the other sides in order.
pub fn find_texture(
    m: &manifest::Manifest,
    sides: &VehicleSides,
    home: Option<usize>,
    name: &str,
) -> Option<TexSource> {
    let name = name.to_ascii_lowercase();
    let in_side = |i: usize| sides.0[i].textures.contains(&name);
    if let Some(h) = home.filter(|&h| in_side(h)) {
        return Some(TexSource::Side(h));
    }
    if m.textures.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
        return Some(TexSource::Map);
    }
    (0..sides.0.len())
        .find(|&i| Some(i) != home && in_side(i))
        .map(TexSource::Side)
}

impl VehicleVisuals {
    /// A measuring switch (`--bench-toggle shadows`): a BF2 map stays without.
    pub fn set_shadows(&mut self, on: bool) {
        self.shadows_on = on;
    }

    /// World-space shadow triangles the last [`Self::draws`] submitted.
    pub fn shadow_triangles(&self) -> usize {
        self.shadow_triangles.len() / 3
    }

    /// Upload each class's model; `geometry[class]` is its `GeometryName`.
    /// Call after [`super::upload_scene`] (the renderer then holds the
    /// map's sun, which a specular pass is resolved under). A class whose
    /// model is found nowhere is reported and draws nothing.
    pub fn load(
        r: &mut Renderer,
        scene: &Scene,
        assets_dir: &Path,
        geometry: &[Option<String>],
    ) -> Self {
        let game = scene.manifest.source_game.to_ascii_lowercase();
        let sides = VehicleSides::load(assets_dir, &game);
        let cull = far_cull(&scene.manifest.lighting).map(|c| c.world);
        let mut out = Self {
            classes: Vec::new(),
            bf1_shadows: game == "bf1",
            shadows_on: true,
            shadow_triangles: Vec::new(),
            posed: Default::default(),
            cull,
            spec_cull: spec_cull(scene, cull),
            items: super::pickups::PickupVisuals::new(assets_dir, &game, cull),
        };
        let mut textures: HashMap<(Option<TexSource>, String), Option<TextureHandle>> =
            HashMap::new();
        for g in geometry {
            let parts =
                g.as_deref()
                    .and_then(|g| match upload_class(r, scene, &sides, &mut textures, g) {
                        Ok(Some(p)) => Some(p),
                        Ok(None) => {
                            eprintln!("vehicles: no model for geometry {g}");
                            None
                        }
                        Err(e) => {
                            eprintln!("vehicles: {g}: {e:#}");
                            None
                        }
                    });
            out.classes.push(parts);
        }
        out
    }

    /// Upload what `sim`'s battle can show later (card HITCH): the item
    /// models ([`super::pickups::PickupVisuals::preload`]).
    pub fn preload(&mut self, r: &mut Renderer, sim: &core::sim::Sim) {
        self.items.preload(r, sim);
    }

    /// The draws of every live vehicle of `sim`, at its sim transform; a
    /// skinned walker's meshes are posed first (card WSKN). A vehicle in
    /// `faded` is drawn see-through at its opacity (the chase camera's,
    /// `crate::camera_collide::see_through_vehicles`).
    pub fn draws(
        &mut self,
        r: &mut Renderer,
        sim: &core::sim::Sim,
        faded: &[(core::vehicle::VehicleHandle, f32)],
    ) -> Vec<DrawCmd> {
        let mut out = Vec::new();
        self.shadow_triangles.clear();
        for (slot, v) in sim.vehicles.iter().enumerate().filter(|(_, v)| v.alive) {
            let Some(Some(class)) = self.classes.get_mut(v.class as usize) else {
                continue;
            };
            let walker = v.kind == core::vehicle::VehicleKind::Walker;
            let fade = faded
                .iter()
                .find(|(h, _)| h.index as usize == slot && h.gen == v.gen)
                .map(|&(_, o)| render::Fade::see_through(visibility_opacity(walker, o)));
            let palette = class.skin.as_ref().filter(|_| walker).map(|sk| {
                let scale = sim
                    .walker_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .map_or(1.0, |p| p.root_scale);
                sk.rig.aimed_palette(v, scale)
            });
            let posed = match (&class.skin, &palette) {
                (Some(skin), Some(palette)) => {
                    match pose_skinned(r, &mut self.posed, slot, v.class, class, skin, palette) {
                        Ok(m) => Some(m),
                        Err(e) => {
                            eprintln!("vehicles: posing walker {slot}: {e:#}");
                            None
                        }
                    }
                }
                _ => None,
            };
            // A posed walker's bone 0 carries the legs' turn (§10); a rigid
            // one turns its whole model with its legs (card WALK).
            let rot = if walker && posed.is_none() {
                core::vehicle::walker::legs_rot(v)
            } else {
                v.rot
            };
            let m = Mat4::from_rotation_translation(rot, v.pos);
            if self.bf1_shadows
                && self.shadows_on
                && v.kind != core::vehicle::VehicleKind::Turret
                && !class.shadows.is_empty()
            {
                let sun = r.lighting.sun_dir.normalize_or_zero();
                let length = if v.kind == core::vehicle::VehicleKind::Flyer {
                    500.0
                } else {
                    150.0
                };
                let sphere = sim
                    .blast_setup
                    .get(v.class as usize)
                    .and_then(Option::as_ref);
                let radius = sphere.map_or(0.0, |s| s.sphere_radius);
                let centre = sphere.map_or(v.pos, |s| m.transform_point3(s.sphere_centre));
                let origin = if walker {
                    let leg_radius = sim
                        .walker_params
                        .get(v.class as usize)
                        .and_then(Option::as_ref)
                        .and_then(|p| p.building.iter().find(|b| b.foot))
                        .map_or(radius, |b| b.radius);
                    let d = if sun.y < -0.5 {
                        leg_radius / (2.0 * sun.y.abs())
                    } else {
                        leg_radius
                    };
                    centre - sun * d + glam::Vec3::Y * (radius - d)
                } else {
                    centre + glam::Vec3::Y * radius
                };
                let length = sim
                    .heightfield
                    .as_ref()
                    .and_then(|h| {
                        core::collision::ray_terrain_ps2(
                            h,
                            &sim.collision.cutters,
                            origin,
                            origin + sun * length,
                        )
                    })
                    .map_or(length, |f| length * f);
                for (shadow, bone) in &mut class.shadows {
                    let node = palette
                        .as_ref()
                        .filter(|_| posed.is_some())
                        .and_then(|p| bone.and_then(|b| p.get(b)))
                        .copied()
                        .unwrap_or(Mat4::IDENTITY);
                    shadow.append_volume(m * node, sun * length, &mut self.shadow_triangles);
                }
            }
            class.draws(
                &mut out,
                posed.as_deref(),
                m,
                (self.cull, self.spec_cull),
                fade,
            );
        }
        r.set_shadow_triangles(&self.shadow_triangles);
        out.extend(self.items.draws(r, sim));
        out
    }
}

pub(super) fn pose_class<K: Copy + PartialEq>(
    r: &mut Renderer,
    posed: &mut HashMap<usize, (K, Vec<MeshHandle>)>,
    slot: usize,
    key: K,
    class: &ClassVis,
    palette: &[Mat4],
) -> Result<Vec<MeshHandle>> {
    let Some(sk) = &class.skin else {
        anyhow::bail!("not skinned");
    };
    pose_skinned(r, posed, slot, key, class, sk, palette)
}

fn pose_skinned<K: Copy + PartialEq>(
    r: &mut Renderer,
    posed: &mut HashMap<usize, (K, Vec<MeshHandle>)>,
    slot: usize,
    key: K,
    class: &ClassVis,
    sk: &Skin,
    palette: &[Mat4],
) -> Result<Vec<MeshHandle>> {
    let (pos, nrm) = skin(palette, &sk.positions, &sk.normals, &sk.joints);
    let fresh = posed.get(&slot).is_none_or(|(c, _)| *c != key);
    let mut handles = Vec::with_capacity(class.parts.len());
    for (k, p) in class.parts.iter().enumerate() {
        let Some((verts, indices)) = &p.skin else {
            handles.push(p.mesh);
            continue;
        };
        let at = |i: &u32| *i as usize;
        if fresh {
            let positions: Vec<[f32; 3]> = verts.iter().map(|i| pos[at(i)]).collect();
            let normals: Vec<[f32; 3]> = verts.iter().map(|i| nrm[at(i)]).collect();
            let uvs: Vec<[f32; 2]> = verts.iter().map(|i| sk.uvs[at(i)]).collect();
            let colors: Vec<[u8; 4]> = verts.iter().map(|i| sk.colors[at(i)]).collect();
            let h = r
                .add_dynamic_mesh(&MeshData {
                    positions: &positions,
                    normals: &normals,
                    uvs: &uvs,
                    colors: Some(&colors),
                    indices,
                })
                .map_err(|e| anyhow::anyhow!("part {k}: {e}"))?;
            handles.push(h);
        } else {
            let h = posed[&slot].1[k];
            let vs: Vec<Vertex> = verts
                .iter()
                .map(|i| Vertex {
                    position: pos[at(i)],
                    normal: nrm[at(i)],
                    uv: sk.uvs[at(i)],
                    color: sk.colors[at(i)],
                })
                .collect();
            r.update_vertices(h, &vs)
                .map_err(|e| anyhow::anyhow!("part {k}: {e}"))?;
            handles.push(h);
        }
    }
    if fresh {
        posed.insert(slot, (key, handles.clone()));
    }
    Ok(handles)
}

/// Each vehicle class's walker rig (card WSKN), by `Vehicle::class`, loaded
/// without a renderer as [`VehicleVisuals::load`] finds it - `None` for a
/// class drawn rigid. A rider on an animated node hangs from it (card
/// VRIDE, `crate::soldier_vis`).
pub fn walker_rigs(
    manifest: &manifest::Manifest,
    assets_dir: &Path,
    geometry: &[Option<String>],
) -> Vec<Option<std::sync::Arc<WalkerRig>>> {
    let sides = VehicleSides::load(assets_dir, &manifest.source_game.to_ascii_lowercase());
    geometry
        .iter()
        .map(|g| {
            let Some(ModelSource::Side(i, model)) = find_model(manifest, &sides, g.as_deref()?)
            else {
                return None;
            };
            let w = sides.0[i].walkers.get(&model.to_ascii_lowercase())?;
            let path = sides.dir(i).join("walkers").join(format!("{model}.glb"));
            let a = load_skinned_glb(&path).ok()?;
            Some(std::sync::Arc::new(
                WalkerRig::new(a.skeleton, a.clips, w).with_seats(
                    sides.0[i]
                        .walker_seats
                        .get(&model.to_ascii_lowercase())
                        .cloned()
                        .unwrap_or_default(),
                ),
            ))
        })
        .collect()
}

/// The top of each class's model, for its camera's pull point
/// ([`crate::walk::vehicle_pull_height`]), parallel to `geometry`: the vis box's top,
/// or on a bake older than it the highest vertex, which can stand several metres
/// lower. `None`: a class with no model.
pub fn model_tops(
    manifest: &manifest::Manifest,
    map_dir: &Path,
    assets_dir: &Path,
    geometry: &[Option<String>],
) -> Vec<Option<f32>> {
    let sides = VehicleSides::load(assets_dir, &manifest.source_game.to_ascii_lowercase());
    geometry
        .iter()
        .map(|g| {
            let (dir, model) = match find_model(manifest, &sides, g.as_deref()?)? {
                ModelSource::Map(model) => (map_dir, model),
                ModelSource::Side(i, model) => (sides.dir(i), model),
            };
            let mesh = load_glb(&dir.join("models").join(format!("{model}.glb"))).ok()?;
            mesh.vis_top
                .or_else(|| mesh.positions.iter().map(|p| p[1]).reduce(f32::max))
        })
        .collect()
}

/// Find `geometry`'s LOD0 ([`find_model`]) and upload it.
fn upload_class(
    r: &mut Renderer,
    scene: &Scene,
    sides: &VehicleSides,
    textures: &mut HashMap<(Option<TexSource>, String), Option<TextureHandle>>,
    geometry: &str,
) -> Result<Option<ClassVis>> {
    let owned: CpuMesh;
    let mut joints = Vec::new();
    let mut rig = None;
    let (cpu, home) = match find_model(&scene.manifest, sides, geometry) {
        None => return Ok(None),
        Some(ModelSource::Map(k)) => (&scene.meshes[&k], None),
        Some(ModelSource::Side(i, model)) => {
            let dir = sides.dir(i);
            let (mesh, j) = load_glb_joints(&dir.join("models").join(format!("{model}.glb")))?;
            (owned, joints) = (mesh, j);
            // Card WSKN: a walker model with its bones and a baked skin.
            let walker = sides.0[i].walkers.get(&model.to_ascii_lowercase());
            if let (Some(w), false) = (walker, joints.is_empty()) {
                let path = dir.join("walkers").join(format!("{model}.glb"));
                match load_skinned_glb(&path) {
                    Ok(a) => {
                        rig = Some(
                            WalkerRig::new(a.skeleton, a.clips, w).with_seats(
                                sides.0[i]
                                    .walker_seats
                                    .get(&model.to_ascii_lowercase())
                                    .cloned()
                                    .unwrap_or_default(),
                            ),
                        )
                    }
                    Err(e) => eprintln!("vehicles: {}: {e:#} — drawn rigid", path.display()),
                }
            }
            (&owned, Some(i))
        }
    };
    let shadow_path = match find_model(&scene.manifest, sides, geometry) {
        Some(ModelSource::Map(model)) => {
            Some(scene.dir.join("shadows").join(format!("{model}.ron")))
        }
        Some(ModelSource::Side(i, model)) => {
            Some(sides.dir(i).join("shadows").join(format!("{model}.ron")))
        }
        None => None,
    };
    let mut visual = upload_parts(r, scene, sides, textures, cpu, home, rig, joints)?;
    if scene.manifest.source_game.eq_ignore_ascii_case("bf1") {
        if let Some(path) = shadow_path.filter(|p| pak::is_file(p)) {
            let load = || -> Result<Vec<(render::ShadowMesh, Option<usize>)>> {
                let pieces: Vec<manifest::shadow::ShadowPiece> =
                    ron::from_str(&pak::read_to_string(&path)?)?;
                pieces
                    .into_iter()
                    .map(|piece| {
                        let mesh = render::ShadowMesh::new(&piece.positions, &piece.faces)
                            .map_err(anyhow::Error::msg)?;
                        let bone = visual
                            .skin
                            .as_ref()
                            .and_then(|s| s.rig.skeleton.index_of(&piece.node));
                        Ok((mesh, bone))
                    })
                    .collect()
            };
            match load() {
                Ok(shadows) => visual.shadows = shadows,
                Err(e) => eprintln!(
                    "vehicle shadows: {}: {e:#} — drawing the vehicle without its shadow",
                    path.display()
                ),
            }
        }
    }
    Ok(Some(visual))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn upload_parts(
    r: &mut Renderer,
    scene: &Scene,
    sides: &VehicleSides,
    textures: &mut HashMap<(Option<TexSource>, String), Option<TextureHandle>>,
    cpu: &CpuMesh,
    home: Option<usize>,
    rig: Option<WalkerRig>,
    joints: Vec<[u8; 4]>,
) -> Result<ClassVis> {
    let mut parts = Vec::new();
    for (k, (mesh, mat, layer, _)) in upload_submeshes(r, cpu)?.into_iter().enumerate() {
        let texture = match mat {
            Some(name) => texture(r, scene, sides, home, textures, name)?,
            None => None,
        };
        if texture.is_none() && super::route_untextured(mat) != super::Untextured::LookupFailed {
            continue;
        }
        parts.push(Part {
            mesh,
            texture,
            layer,
            spec: cpu
                .spec_pass(k)
                .map(|p| (p.rgb_under(r.lighting.sun_color), p.power)),
            light: cpu.light_path(k),
            skin: rig.as_ref().map(|_| {
                let (start, count) = (cpu.submeshes[k].0 as usize, cpu.submeshes[k].1 as usize);
                compact(&cpu.indices[start..start + count], cpu.positions.len())
            }),
        });
    }
    let skin = rig.map(|rig| Skin {
        rig,
        positions: cpu.positions.clone(),
        normals: cpu.normals.clone(),
        uvs: cpu.uvs.clone(),
        colors: cpu.colors.clone(),
        joints,
    });
    Ok(ClassVis {
        parts,
        skin,
        shadows: Vec::new(),
    })
}

/// One submesh's vertices (first-use order) and its triangles over them.
fn compact(idx: &[u32], vertices: usize) -> (Vec<u32>, Vec<u32>) {
    let mut slot = vec![u32::MAX; vertices];
    let mut verts = Vec::new();
    let tris = idx
        .iter()
        .map(|&i| {
            let s = &mut slot[i as usize];
            if *s == u32::MAX {
                *s = verts.len() as u32;
                verts.push(i);
            }
            *s
        })
        .collect();
    (verts, tris)
}

/// Upload (once; misses cached too) a texture [`find_texture`] finds.
fn texture(
    r: &mut Renderer,
    scene: &Scene,
    sides: &VehicleSides,
    home: Option<usize>,
    cache: &mut HashMap<(Option<TexSource>, String), Option<TextureHandle>>,
    name: &str,
) -> Result<Option<TextureHandle>> {
    let name = name.to_ascii_lowercase();
    let source = find_texture(&scene.manifest, sides, home, &name);
    let key = (source, name.clone());
    if let Some(&t) = cache.get(&key) {
        return Ok(t);
    }
    let found = match source {
        None => None,
        Some(TexSource::Map) => match scene.textures.get(&name) {
            Some(t) => Some(upload_cpu_texture(r, t, &name)?),
            None => None,
        },
        Some(TexSource::Side(i)) => upload_texture_file(r, &sides.dir(i).join("textures"), &name)?,
    };
    cache.insert(key, found);
    Ok(found)
}

#[cfg(test)]
mod texture_cache_tests {
    use super::*;

    #[test]
    fn resolved_source_keeps_same_named_home_textures_and_misses_distinct() {
        let m: manifest::Manifest = ron::from_str(r#"(
            map: "t", source_game: "bf2", terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: ["shared"], classes: [], geometries: [], regions: [], instances: []
        )"#).unwrap();
        let side = |name: &str| Side {
            dir: name.into(),
            models: HashMap::new(),
            walkers: HashMap::new(),
            walker_seats: HashMap::new(),
            textures: ["shared".to_owned()].into_iter().collect(),
        };
        let sides = VehicleSides(vec![side("a"), side("b")]);
        for order in [[0, 1], [1, 0]] {
            let mut cache = HashMap::new();
            for home in order {
                let source = find_texture(&m, &sides, Some(home), "SHARED");
                assert_eq!(source, Some(TexSource::Side(home)));
                let key = (source, "shared".to_owned());
                cache.entry(key.clone()).or_insert(home);
                assert_eq!(cache[&key], home);
            }
            assert_eq!(cache.len(), 2);
            assert_eq!(
                find_texture(&m, &sides, None, "shared"),
                Some(TexSource::Map)
            );
        }
        assert_eq!(find_texture(&m, &sides, Some(0), "missing"), None);
    }
}

#[cfg(test)]
mod close_walker_tests {
    #[test]
    fn a_camera_inside_one_foot_never_hides_the_whole_walker() {
        assert_eq!(
            super::visibility_opacity(true, 0.0),
            crate::camera_collide::SEE_THROUGH_OPACITY
        );
        assert_eq!(super::visibility_opacity(true, 1.0), 1.0);
        assert_eq!(super::visibility_opacity(false, 0.0), 0.0);
    }
}

#[cfg(test)]
mod shadow_tests {
    use super::*;

    /// Requires the isolated shadow bake overlay described in the issue-66 report.
    #[test]
    #[ignore = "requires BF1 NAB1 assets and an isolated shadow sidecar overlay"]
    fn real_xwing_shadows_are_submitted_only_on_bf1() {
        use core::vehicle::{VehicleKind, VehicleSpec};
        use glam::Vec3;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let assets = root.join("engine/artifacts/lead_tmp/shadows/assets");
        let (mut scene, _) =
            crate::assets::load_battle_scene(&root.join("engine/assets_baked/bf1_nab1"), "con")
                .unwrap();
        let mut r = Renderer::new_headless(256, 192).unwrap();
        let mut sim = core::sim::Sim::empty(1);
        sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: Vec3::new(0., 3., 0.),
            yaw: 0.,
            max_health: 1000.,
            health_type: core::weapon::HealthType::Vehicle,
            seats: Vec::new(),
        });
        let initial_hash = sim.hash();
        let geometry = [Some("all_fly_xwing".into())];
        for game in ["bf1", "bf2"] {
            scene.manifest.source_game = game.into();
            let mut vis = VehicleVisuals::load(&mut r, &scene, &assets, &geometry);
            let class = vis.classes[0]
                .as_ref()
                .expect("real X-wing model must load");
            assert!(!class.parts.is_empty());
            assert_eq!(class.shadows.len(), if game == "bf1" { 9 } else { 0 });
            let draws = vis.draws(&mut r, &sim, &[]);
            assert!(!draws.is_empty());
            assert_eq!(vis.shadow_triangles.is_empty(), game == "bf2");
            let plane = r
                .add_mesh(&MeshData {
                    positions: &[
                        [-60., 0., -60.],
                        [60., 0., -60.],
                        [60., 0., 60.],
                        [-60., 0., 60.],
                    ],
                    normals: &[[0., 1., 0.]; 4],
                    uvs: &[[0.; 2]; 4],
                    colors: None,
                    indices: &[0, 2, 1, 0, 3, 2],
                })
                .unwrap();
            let mut draws = draws;
            draws.push(
                DrawCmd::flat(plane, [0.5, 0.5, 0.5, 1.], Mat4::IDENTITY)
                    .with_light(LightPath::Unlit),
            );
            let camera = crate::viewer::look_at(Vec3::new(18., 18., 25.), Vec3::new(0., 0., 0.));
            r.render(&camera, &draws, [0.1, 0.1, 0.1, 1.]).unwrap();
            let (with, w, h) = r.capture_rgba().unwrap();
            r.set_shadow_triangles(&[]);
            r.render(&camera, &draws, [0.1, 0.1, 0.1, 1.]).unwrap();
            let (without, _, _) = r.capture_rgba().unwrap();
            let changed = with
                .chunks_exact(4)
                .zip(without.chunks_exact(4))
                .filter(|(a, b)| a[..3] != b[..3])
                .count();
            let ground = with
                .chunks_exact(4)
                .zip(without.chunks_exact(4))
                .filter(|(a, b)| {
                    a[..3] != b[..3] && b[0] == b[1] && b[1] == b[2] && (185..=190).contains(&b[0])
                })
                .count();
            println!("{game}: real X-wing changed {changed} pixels, {ground} on the plane");
            if game == "bf1" {
                assert!(
                    ground > 100,
                    "real caster must shade the plane, got {ground}"
                );
            } else {
                assert_eq!(changed, 0);
            }
            let out = root.join("engine/artifacts/lead_tmp/shadows");
            image::save_buffer(
                out.join(format!("{game}-xwing.png")),
                &with,
                w,
                h,
                image::ColorType::Rgba8,
            )
            .unwrap();
            assert_eq!(
                sim.hash(),
                initial_hash,
                "rendering must not mutate simulation"
            );
        }
    }
}
