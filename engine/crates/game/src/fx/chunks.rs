//! Destruction chunks (`re-vehicle-death.md` §6, `re-droid-death.md` §2, `re-chunks.md`):
//! rigid bodies on the chunk model's one collision prim, with their trail, smoke and
//! terrain effects. [ours] Contacts come from the terrain and the map's bodies only (no
//! vehicle hulls, no soldiers), and chunks never damage units. A chunk model with no baked
//! prim is a sphere around its mesh bounds: the PS2's own rule for a model without one.
use super::emitter::Frame;
use super::{Era, FxAssets, Lcg, Library, LiveEffect};
use crate::{classdb::ClassDb, walk::WalkSim};
use core::collision::{HullFilter, Layers, Shape};
use core::sim::{Event, Sim};
use glam::{Mat3, Mat4, Quat, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

const MASS: f32 = 40.0;
const SPACE_LIFE: f32 = 8.0;
const CLUSTERS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Prim {
    Sphere(f32),
    Cylinder { radius: f32, height: f32 },
    Box(Vec3),
}

impl Prim {
    fn of(shape: &Shape) -> Option<Prim> {
        Some(match *shape {
            Shape::Sphere { radius } => Prim::Sphere(radius),
            Shape::Cylinder {
                radius,
                half_height,
            } => Prim::Cylinder {
                radius,
                height: half_height * 2.0,
            },
            Shape::Box { half } => Prim::Box(half),
            Shape::Mesh(_) => return None,
        })
    }

    fn inertia(self) -> Vec3 {
        match self {
            Prim::Box(h) => {
                let s = h * h;
                Vec3::new(s.y + s.z, s.x + s.z, s.y + s.x) * 0.33333
            }
            Prim::Sphere(r) => Vec3::splat(r * r * 0.4),
            Prim::Cylinder { radius, height } => {
                let side = radius * radius * 0.25 + height * height * 0.08333;
                Vec3::new(side, radius * radius * 0.5, side)
            }
        }
    }

    fn bound(self) -> f32 {
        match self {
            Prim::Sphere(r) => r,
            Prim::Box(h) => h.length(),
            Prim::Cylinder { radius, height } => (radius * radius + height * height * 0.25).sqrt(),
        }
    }

    fn support(self, rot: Quat, dir: Vec3) -> Vec3 {
        match self {
            Prim::Sphere(r) => dir.normalize_or_zero() * r,
            Prim::Box(h) => [Vec3::X * h.x, Vec3::Y * h.y, Vec3::Z * h.z]
                .into_iter()
                .map(|a| rot * a)
                .map(|a| if a.dot(dir) < 0.0 { -a } else { a })
                .sum(),
            Prim::Cylinder { radius, height } => {
                let axis = rot * Vec3::Y;
                let along = axis.dot(dir);
                let cap = if along < 0.0 { -axis } else { axis } * (height * 0.5);
                cap + (dir - axis * along).normalize_or_zero() * radius
            }
        }
    }

    fn shape(self) -> Shape {
        match self {
            Prim::Sphere(radius) => Shape::Sphere { radius },
            Prim::Cylinder { radius, height } => Shape::Cylinder {
                radius,
                half_height: height * 0.5,
            },
            Prim::Box(half) => Shape::Box { half },
        }
    }
}

#[derive(Clone, Debug)]
struct Spec {
    model: String,
    prim: Prim,
    local: Mat4,
    speed: f32,
    inherit: f32,
    up: f32,
    bounce: f32,
    stick: f32,
    linear_damping: f32,
    angular_damping: f32,
    gravity: Vec3,
    omega: Vec3,
    lifetime: f32,
    mode: u8,
    budget: u8,
    terrain_effect: Option<String>,
    trails: Vec<(String, Option<Mat4>)>,
    smoke: Vec<(String, Mat4)>,
    node: Option<Mat4>,
    node_name: Option<String>,
}

impl Spec {
    fn inverse_inertia(&self) -> Vec3 {
        (self.prim.inertia() * MASS).map(|i| if i > 0.0 { 1.0 / i } else { 1.0 })
    }
}

struct Hit {
    point: Vec3,
    normal: Vec3,
    depth: f32,
    terrain: bool,
}

struct Piece {
    spec: Arc<Spec>,
    pos: Vec3,
    vel: Vec3,
    rot: Quat,
    momentum: Vec3,
    omega: Vec3,
    moving: bool,
    count: u8,
    budget: u8,
    touched: bool,
    timer: f32,
    space: bool,
    trails: Vec<(LiveEffect, Option<Mat4>)>,
    smoke: Vec<LiveEffect>,
}

impl Piece {
    fn inverse_inertia(&self) -> Mat3 {
        let r = Mat3::from_quat(self.rot);
        r * Mat3::from_diagonal(self.spec.inverse_inertia()) * r.transpose()
    }

    fn body(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rot, self.pos)
    }

    fn model(&self) -> Mat4 {
        self.body() * self.spec.local.inverse()
    }

    fn frame(m: Mat4, vel: Vec3) -> Frame {
        Frame {
            rot: Mat3::from_mat4(m),
            pos: m.w_axis.truncate(),
            vel,
        }
    }

    /// `[ours]` BF1's rest life on both games (the maintainer 2026-10-29:
    /// BF2's 8 s left pieces lying too long).
    fn life(&self) -> f32 {
        if self.budget == 0xff {
            return 30.0;
        }
        if self.spec.lifetime >= 0.0 {
            return self.spec.lifetime;
        }
        match self.spec.mode {
            2 | 4 => 53.0,
            _ if self.space => SPACE_LIFE,
            _ => 3.0,
        }
    }

    fn opacity(&self) -> f32 {
        ((self.life() - self.timer) * 0.5).clamp(0.0, 1.0)
    }

    /// `rays` is what the settle and below-terrain rays see: map bodies on
    /// the ground (PS2 masks 0x90 and 0x10 hold no vehicle), every hull in
    /// space.
    fn step(
        &mut self,
        sim: &Sim,
        dt: f32,
        bf1: bool,
        rays: HullFilter,
        bursts: &mut Vec<Vec3>,
    ) -> bool {
        self.touched = false;
        if !self.moving || sim.heightfield.is_none() || self.budget == 0xff {
            let life = self.life();
            if self.timer < life {
                self.timer += dt;
                if self.timer >= life {
                    return false;
                }
            }
            if !self.moving {
                return true;
            }
        }
        let s = self.spec.clone();
        let gravity = if bf1 {
            Vec3::new(0.0, -9.8, 0.0)
        } else {
            s.gravity
        };
        let force = gravity * MASS - self.vel * s.linear_damping;
        if s.mode != 1 && s.mode != 4 {
            self.momentum -= self.momentum * s.angular_damping * dt;
            self.omega = self.inverse_inertia() * self.momentum;
        }
        let turn = self.omega * dt;
        if turn.length() > 0.0001 {
            self.rot = (Quat::from_scaled_axis(turn) * self.rot).normalize();
        }
        self.vel += force * (1.0 / MASS) * dt;
        self.pos += self.vel * dt;
        if self.count > self.budget {
            let reach = s.prim.bound() * 1.001;
            let limit = if s.mode == 4 { 0.3 } else { 0.75 };
            let near = floor_fraction(sim, self.pos, reach, rays).is_some_and(|f| f < limit);
            if near || s.mode == 2 || u32::from(self.count) > u32::from(self.budget) + 5 {
                self.moving = false;
                return true;
            }
        }
        if let Some(hf) = &sim.heightfield {
            if self.pos.y + s.prim.bound() < hf.height_at(self.pos.x, self.pos.z)
                && sim.collision.ray_down_with(self.pos, 256.0, rays).is_none()
            {
                return false;
            }
        }
        for hit in contacts(sim, s.prim, self.pos, self.rot) {
            self.hit(&hit, sim, bursts);
        }
        true
    }

    fn hit(&mut self, c: &Hit, sim: &Sim, bursts: &mut Vec<Vec3>) {
        if !self.touched {
            self.touched = true;
            let s = self.spec.clone();
            let n = c.normal;
            let r = c.point - self.pos;
            let slide = self.vel + self.omega.cross(r);
            let vn = slide.dot(n);
            let t = -(slide - n * vn).normalize_or_zero();
            if vn < 0.0 {
                if c.terrain && self.count >= self.budget {
                    if self.count == self.budget {
                        let y = sim
                            .heightfield
                            .as_ref()
                            .map_or(-1000.0, |hf| hf.height_at(c.point.x, c.point.z));
                        bursts.push(Vec3::new(c.point.x, y, c.point.z));
                        self.count = self.count.wrapping_add(1);
                    }
                    return;
                }
                let inv = self.inverse_inertia();
                let k = 1.0 / MASS + (inv * r.cross(n)).cross(r).dot(n);
                let j = (s.bounce * -vn - vn) / k;
                self.vel += n * j / MASS;
                self.momentum += r.cross(n * j);
                self.omega = inv * self.momentum;
                let vt = (self.vel + self.omega.cross(r)).dot(t);
                if vt < 0.0 {
                    let k = 1.0 / MASS + (inv * r.cross(t)).cross(r).dot(t);
                    self.momentum += r.cross(t * (-vt * s.stick / k));
                    self.omega = inv * self.momentum;
                }
            }
        }
        self.count = self.count.wrapping_add(1);
        self.pos += c.normal * c.depth;
    }

    fn trail_frame(&self, node: Option<Mat4>) -> Frame {
        match node {
            Some(n) => Piece::frame(self.model() * n, self.vel),
            None => Piece::frame(self.body(), self.vel),
        }
    }

    fn follow(&mut self) {
        let frames: Vec<Frame> = self
            .trails
            .iter()
            .map(|(_, n)| self.trail_frame(*n))
            .collect();
        for ((e, _), frame) in self.trails.iter_mut().zip(frames) {
            e.frame = frame;
        }
    }

    fn settle(&mut self, lib: Option<&Library>, rng: &mut Lcg) {
        let Some(lib) = lib else {
            return;
        };
        let model = self.model();
        for (name, node) in &self.spec.smoke {
            if let Some(def) = lib.get(name) {
                let frame = Piece::frame(model * *node, Vec3::ZERO);
                self.smoke.push(LiveEffect::new(def.clone(), frame, rng));
            }
        }
    }

    fn stop(self, out: &mut Vec<LiveEffect>) {
        for mut e in self.trails.into_iter().map(|(e, _)| e).chain(self.smoke) {
            e.stop();
            out.push(e);
        }
    }
}

fn floor_fraction(sim: &Sim, from: Vec3, reach: f32, rays: HullFilter) -> Option<f32> {
    let terrain = sim.heightfield.as_ref().and_then(|hf| {
        let h = hf.height_at(from.x, from.z);
        let gap = from.y - h;
        (gap >= 0.0 && gap <= reach && !sim.collision.is_cut(Vec3::new(from.x, h, from.z)))
            .then_some(gap / reach)
    });
    let body = sim
        .collision
        .ray_down_with(from, reach, rays)
        .map(|(y, _)| (from.y - y) / reach);
    match (terrain, body) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn contacts(sim: &Sim, prim: Prim, pos: Vec3, rot: Quat) -> Vec<Hit> {
    let mut out = Vec::new();
    if let Some(hf) = &sim.heightfield {
        let n = Vec3::from(hf.normal_at(pos.x, pos.z));
        let p = pos + prim.support(rot, -n);
        let h = hf.height_at(p.x, p.z);
        let depth = (h - p.y) * n.y;
        if depth > 0.0 && !sim.collision.is_cut(Vec3::new(p.x, h, p.z)) {
            out.push(Hit {
                point: p,
                normal: n,
                depth,
                terrain: true,
            });
        }
    }
    if sim.collision.bodies_and_hulls().next().is_some() {
        let mut found = Vec::new();
        match prim {
            Prim::Sphere(r) => sim.collision.sphere_contacts_filtered(
                pos,
                r,
                Layers::SOLDIER | Layers::VEHICLE,
                HullFilter::All,
                &mut found,
            ),
            _ => sim.collision.shape_contacts_filtered(
                &prim.shape(),
                Mat4::from_rotation_translation(rot, pos),
                Layers::SOLDIER | Layers::VEHICLE,
                HullFilter::All,
                &mut found,
            ),
        }
        out.extend(found.into_iter().map(|(depth, normal)| Hit {
            point: pos + prim.support(rot, -normal),
            normal,
            depth,
            terrain: false,
        }));
    }
    out
}

#[derive(Default)]
struct SideData {
    prims: HashMap<String, (Prim, Mat4)>,
    nodes: HashMap<String, Vec<manifest::ManifestHardpoint>>,
    classes: HashMap<String, Vec<manifest::ManifestHardpoint>>,
}

impl SideData {
    fn load(dirs: &[std::path::PathBuf]) -> SideData {
        let mut out = SideData::default();
        for file in dirs
            .iter()
            .filter_map(|d| d.parent())
            .map(|d| d.join("manifest.ron"))
            .filter(|p| pak::is_file(p))
        {
            let Ok(text) = pak::read_to_string(&file) else {
                continue;
            };
            let Ok(map) = ron::from_str::<manifest::Manifest>(&text) else {
                eprintln!("chunks: {} unreadable", file.display());
                continue;
            };
            Self::record_prims(&mut out.prims, &map.collision, false);
        }
        for file in dirs
            .iter()
            .filter_map(|d| d.parent())
            .map(|d| d.join("vehicles.ron"))
            .filter(|p| pak::is_file(p))
        {
            let Ok(text) = pak::read_to_string(&file) else {
                continue;
            };
            let Ok(vm) = ron::from_str::<manifest::VehicleManifest>(&text) else {
                eprintln!("chunks: {} unreadable", file.display());
                continue;
            };
            Self::record_prims(&mut out.prims, &vm.collision, false);
            for g in vm.geometries.iter().filter(|g| !g.hardpoints.is_empty()) {
                out.nodes
                    .entry(g.name.to_ascii_lowercase())
                    .or_insert_with(|| g.hardpoints.clone());
            }
            for v in vm
                .vehicles
                .iter()
                .filter(|v| !v.class.hardpoints.is_empty())
            {
                out.classes
                    .entry(v.class.name.to_ascii_lowercase())
                    .or_insert_with(|| v.class.hardpoints.clone());
            }
        }
        for file in dirs
            .iter()
            .filter_map(|d| d.parent())
            .map(|d| d.join("droid_chunks.ron"))
            .filter(|p| pak::is_file(p))
        {
            match pak::read_to_string(&file)
                .map_err(anyhow::Error::from)
                .and_then(|t| {
                    ron::from_str::<manifest::DroidChunkManifest>(&t).map_err(anyhow::Error::from)
                }) {
                Ok(meta) => {
                    Self::record_prims(&mut out.prims, &meta.collision, true);
                    out.nodes.extend(
                        meta.nodes
                            .into_iter()
                            .map(|(n, nodes)| (n.to_ascii_lowercase(), nodes)),
                    );
                }
                Err(e) => eprintln!("chunks {}: {e:#}", file.display()),
            }
        }
        out
    }

    /// Record each geometry's one-prim collision. A droid's bake replaces an
    /// earlier record (`replace`); a map's or a vehicle's keeps the first.
    fn record_prims(
        out: &mut HashMap<String, (Prim, Mat4)>,
        collision: &[manifest::ManifestCollision],
        replace: bool,
    ) {
        for c in collision {
            let [p] = &c.prims[..] else {
                continue;
            };
            let Some(prim) = crate::assets::prim_shape(p).as_ref().and_then(Prim::of) else {
                continue;
            };
            let entry = (
                prim,
                crate::viewer::instance_matrix(p.rotation, p.translation),
            );
            if replace {
                out.insert(c.geometry.to_ascii_lowercase(), entry);
            } else {
                out.entry(c.geometry.to_ascii_lowercase()).or_insert(entry);
            }
        }
    }
}

fn node_matrix(nodes: &[manifest::ManifestHardpoint], name: &str) -> Option<Mat4> {
    let n = nodes
        .iter()
        .find(|n| n.name.eq_ignore_ascii_case(name.trim()))?;
    let ax = n
        .axes
        .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    Some(Mat4::from_cols(
        Vec3::from(ax[0]).extend(0.0),
        Vec3::from(ax[1]).extend(0.0),
        Vec3::from(ax[2]).extend(0.0),
        Vec3::from(n.pos).extend(1.0),
    ))
}

fn floats(v: &str) -> Vec<f32> {
    v.split_whitespace()
        .map_while(|t| t.parse::<f32>().ok())
        .collect()
}

#[derive(Default)]
pub struct Chunks {
    defs: HashMap<String, Vec<Arc<Spec>>>,
    live: VecDeque<Vec<Piece>>,
    loose: Vec<LiveEffect>,
    rng: Option<Lcg>,
    bf1: bool,
    posed_nodes: HashMap<u32, Vec<(String, Mat4)>>,
    lit: HashSet<String>,
    /// Space rules: explosion-class pieces stand in for a class without its
    /// own, and the pieces' floor rays see vehicle hulls. Ground modes keep
    /// the class's own pieces and the map's bodies.
    space: bool,
}

impl Chunks {
    pub fn extend(
        &mut self,
        db: &ClassDb,
        names: &[String],
        dirs: &[std::path::PathBuf],
        era: Era,
    ) {
        for (name, pieces) in Self::load_for(db, names, dirs, era, self.space).defs {
            self.defs.entry(name).or_insert(pieces);
        }
    }

    /// The pieces under space rules ([`Chunks::load_for`]).
    pub fn load(db: &ClassDb, names: &[String], dirs: &[std::path::PathBuf], era: Era) -> Self {
        Self::load_for(db, names, dirs, era, true)
    }

    /// The pieces of `names`. Under ground rules (`space` false) a class
    /// throws only its own pieces, one per slot.
    pub fn load_for(
        db: &ClassDb,
        names: &[String],
        dirs: &[std::path::PathBuf],
        era: Era,
        space: bool,
    ) -> Self {
        use crate::classdb::fnv1a_lowercase as hash;
        let mut out = Self {
            bf1: era == Era::Bf1,
            space,
            ..Self::default()
        };
        let side = SideData::load(dirs);
        let mut bounds = HashMap::new();
        for name in names {
            let mut specs = Vec::new();
            let vehicle_nodes = side
                .classes
                .get(&name.to_ascii_lowercase())
                .or_else(|| {
                    db.prop(name, "GeometryName")
                        .and_then(|g| side.nodes.get(&g.to_ascii_lowercase()))
                })
                .map_or(&[][..], Vec::as_slice);
            let mut slots = chunk_properties(db.chain(name));
            let mut exp_name = None;
            if space
                && !slots.iter().any(|s| {
                    s.props
                        .get(&hash("ChunkGeometryName"))
                        .is_some_and(|m| !m.trim().is_empty())
                })
            {
                if let Some(exp) = crate::vehicle_class::explosion_name(db, name) {
                    slots = chunk_properties(db.chain(&exp));
                    exp_name = Some(exp);
                }
            }
            for slot in &slots {
                let props = &slot.props;
                let Some(model) = props.get(&hash("ChunkGeometryName")) else {
                    continue;
                };
                if model.trim().is_empty() {
                    continue;
                }
                let text = |key: &str| props.get(&hash(key)).map(String::as_str);
                let number = |key: &str| text(key).and_then(crate::vehicle_class::float);
                let vector = |key: &str| {
                    text(key)
                        .map(floats)
                        .filter(|v| v.len() >= 3)
                        .map(|v| Vec3::new(v[0], v[1], v[2]))
                };
                let key = model.to_ascii_lowercase();
                let (prim, local) = match side.prims.get(&key) {
                    Some(p) => *p,
                    None => {
                        let bound = bounds.entry(key.clone()).or_insert_with(|| {
                            let file = dirs
                                .iter()
                                .map(|d| d.join(format!("{model}.glb")))
                                .find(|p| pak::is_file(p))?;
                            let mesh = crate::assets::load_glb(&file).ok()?;
                            let (mut lo, mut hi) =
                                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                            for &v in &mesh.positions {
                                lo = lo.min(Vec3::from(v));
                                hi = hi.max(Vec3::from(v));
                            }
                            (!mesh.positions.is_empty())
                                .then_some(((lo + hi) * 0.5, (hi - lo).length() * 0.5))
                        });
                        let (centre, radius) = match *bound {
                            Some((c, r)) => (c, r),
                            None => {
                                if dirs.is_empty() {
                                    (Vec3::ZERO, 0.5)
                                } else {
                                    eprintln!("chunk model {model}: missing bake, no piece drawn");
                                    continue;
                                }
                            }
                        };
                        (Prim::Sphere(radius), Mat4::from_translation(centre))
                    }
                };
                let chunk_nodes = side.nodes.get(&key).map_or(&[][..], Vec::as_slice);
                let mode = match text("ChunkPhysics").map(|v| v.trim().to_ascii_lowercase()) {
                    None => 0,
                    Some(v) => match v.as_str() {
                        "full" => 0,
                        "static" => 2,
                        "leaf" => 3,
                        "cockpit" => 4,
                        _ => 1,
                    },
                };
                let ps2 = |v: Vec3| Vec3::new(v.x, v.y, -v.z);
                let omega = vector("ChunkOmega").unwrap_or(Vec3::ZERO);
                let spec = Arc::new(Spec {
                    model: model.to_owned(),
                    prim,
                    local,
                    speed: number("ChunkSpeed").unwrap_or(-1.0),
                    inherit: number("ChunkVelocityFactor").unwrap_or(1.0),
                    up: number("ChunkUpFactor").unwrap_or(0.25),
                    bounce: number("ChunkBounciness").unwrap_or(0.6),
                    stick: number("ChunkStickiness").unwrap_or(0.7),
                    linear_damping: number("ChunkLinearDamping").unwrap_or(0.001),
                    angular_damping: number("ChunkAngularDamping").unwrap_or(0.004),
                    gravity: vector("ChunkGravity").map_or(Vec3::new(0.0, -9.8, 0.0), ps2),
                    omega,
                    lifetime: number("ChunkLifetime").unwrap_or(-1.0),
                    mode,
                    budget: text("ChunkTerrainCollisions")
                        .and_then(|v| v.trim().parse::<i64>().ok())
                        .map_or(0, |v| v as u8),
                    terrain_effect: text("ChunkTerrainEffect").map(|v| v.trim().to_owned()),
                    trails: slot
                        .trails
                        .iter()
                        .map(|(e, n)| {
                            (
                                e.trim().to_owned(),
                                n.as_deref().and_then(|n| node_matrix(chunk_nodes, n)),
                            )
                        })
                        .collect(),
                    smoke: slot
                        .smoke
                        .iter()
                        .map(|(e, n)| {
                            (
                                e.trim().to_owned(),
                                n.as_deref()
                                    .and_then(|n| node_matrix(chunk_nodes, n))
                                    .unwrap_or(Mat4::IDENTITY),
                            )
                        })
                        .collect(),
                    node: text("ChunkNodeName").and_then(|n| node_matrix(vehicle_nodes, n)),
                    node_name: text("ChunkNodeName").map(|n| n.trim().to_owned()),
                });
                // NumChunks belongs to explosion classes, whose own law
                // (`re-vehicle-death.md` §6.1) is not ported; the ground
                // throws each slot once.
                let count = text("NumChunks")
                    .filter(|_| space)
                    .or_else(|| {
                        (slots.len() == 1)
                            .then(|| db.prop(name, "NumChunks"))
                            .flatten()
                    })
                    .or_else(|| {
                        (slots.len() == 1)
                            .then(|| exp_name.as_deref().and_then(|e| db.prop(e, "NumChunks")))
                            .flatten()
                    })
                    .filter(|_| space)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .map_or(1, |n| n.clamp(1, 10));
                for _ in 0..count {
                    specs.push(Arc::clone(&spec));
                }
            }
            if !specs.is_empty() {
                out.defs.insert(name.clone(), specs);
            }
        }
        out
    }

    pub fn pose_bursts(&mut self, walk: &WalkSim, vis: &crate::soldier_vis::SoldierVisuals) {
        self.posed_nodes.clear();
        for e in &walk.sim.events {
            if let Event::DroidBurst { unit, .. } = e {
                self.posed_nodes
                    .insert(unit.index, vis.chunk_nodes(walk, *unit));
            }
        }
    }

    pub fn tick(&mut self, walk: &WalkSim, lib: Option<&Library>) {
        let dt = core::TICK_DT;
        let rng = self.rng.get_or_insert(Lcg(0xdeb1_5eed));
        let mut bursts = Vec::new();
        let rays = if self.space {
            HullFilter::All
        } else {
            HullFilter::Static
        };
        for cluster in &mut self.live {
            for p in std::mem::take(cluster) {
                let mut p = p;
                let moving = p.moving;
                let mut here = Vec::new();
                if p.step(&walk.sim, dt, self.bf1, rays, &mut here) {
                    if moving && !p.moving {
                        p.settle(lib, rng);
                    }
                    p.follow();
                    bursts.extend(here.into_iter().map(|at| (p.spec.clone(), at)));
                    cluster.push(p);
                } else {
                    p.stop(&mut self.loose);
                }
            }
        }
        self.live.retain(|c| !c.is_empty());
        if let Some(lib) = lib {
            for (spec, at) in bursts {
                if let Some(def) = spec.terrain_effect.as_deref().and_then(|n| lib.get(n)) {
                    self.loose
                        .push(LiveEffect::new(def.clone(), Frame::at(at), rng));
                }
            }
        }
        for event in &walk.sim.events {
            match event {
                Event::DroidBurst {
                    unit,
                    team,
                    class,
                    pos: _,
                    scale,
                } => {
                    let name = &walk.teams[usize::from(*team).min(1)].class(*class).name;
                    let u = &walk.sim.units[unit.index as usize];
                    let nodes = self.posed_nodes.remove(&unit.index);
                    self.launch_posed(
                        name,
                        u.soldier.pos,
                        u.soldier.vel,
                        Quat::from_rotation_y(u.soldier.yaw + std::f32::consts::PI),
                        1.0,
                        *scale,
                        lib,
                        nodes.as_deref(),
                    );
                }
                Event::VehicleDestroyed {
                    vehicle,
                    class,
                    pos,
                    vel,
                    ..
                } => {
                    if let Some(name) = walk.vehicle_classes.get(*class as usize) {
                        let rot = walk
                            .sim
                            .vehicles
                            .get(vehicle.index as usize)
                            .map(|v| v.rot)
                            .filter(|r| r.is_finite() && *r != Quat::IDENTITY)
                            .or_else(|| {
                                walk.sim
                                    .building_turrets
                                    .iter()
                                    .find(|t| t.handle == *vehicle)
                                    .map(|t| t.rot)
                            })
                            .unwrap_or(Quat::IDENTITY);
                        let radius = walk
                            .sim
                            .seat_setup
                            .get(*class as usize)
                            .and_then(Option::as_ref)
                            .map(|s| s.radius)
                            .filter(|&r| r > 0.0)
                            .or_else(|| {
                                walk.sim
                                    .blast_setup
                                    .get(*class as usize)
                                    .and_then(Option::as_ref)
                                    .map(|b| b.sphere_radius)
                                    .filter(|&r| r > 0.0)
                            })
                            .or_else(|| {
                                walk.sim
                                    .building_turrets
                                    .iter()
                                    .find(|t| t.handle == *vehicle)
                                    .and_then(|t| t.aim_sphere.map(|(_, r)| r))
                                    .filter(|&r| r > 0.0)
                            })
                            .unwrap_or(1.0);
                        self.launch(name, *pos, *vel, rot, radius, 1.0, lib);
                    }
                }
                Event::StationDestroyed {
                    station,
                    class,
                    pos,
                    ..
                } => {
                    if let Some(sc) = walk.sim.station_classes.get(usize::from(*class)) {
                        let st = walk.sim.stations.get(usize::from(*station));
                        let rot = st.map_or(Quat::IDENTITY, |s| {
                            Quat::from_mat4(&s.transform).normalize()
                        });
                        let radius = st.map_or(1.0, |s| {
                            if s.sphere_radius > 0.0 {
                                s.sphere_radius
                            } else if s.radius > 0.0 {
                                s.radius
                            } else {
                                1.0
                            }
                        });
                        self.launch(&sc.name, *pos, Vec3::ZERO, rot, radius, 1.0, lib);
                    }
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn launch(
        &mut self,
        name: &str,
        pos: Vec3,
        velocity: Vec3,
        rot: Quat,
        radius: f32,
        scale: f32,
        lib: Option<&Library>,
    ) {
        self.launch_posed(name, pos, velocity, rot, radius, scale, lib, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn launch_posed(
        &mut self,
        name: &str,
        pos: Vec3,
        velocity: Vec3,
        rot: Quat,
        radius: f32,
        scale: f32,
        lib: Option<&Library>,
        nodes: Option<&[(String, Mat4)]>,
    ) {
        let Some(specs) = self.defs.get(name) else {
            return;
        };
        let start = Mat4::from_rotation_translation(rot, pos);
        let bodies: Vec<Mat4> = specs
            .iter()
            .map(|s| {
                let posed = s.node_name.as_deref().and_then(|name| {
                    // Only pieces with baked launch frames opt into posed bones.
                    // Native battle-droid pieces keep their existing origin.
                    s.node?;
                    nodes?
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(name))
                        .map(|(_, m)| *m)
                });
                start * posed.or(s.node).unwrap_or(Mat4::IDENTITY) * s.local
            })
            .collect();
        let centre = bodies.iter().map(|b| b.w_axis.truncate()).sum::<Vec3>() / specs.len() as f32;
        let rng = self.rng.get_or_insert(Lcg(0xdeb1_5eed));
        if self.live.len() == CLUSTERS {
            if let Some(old) = self.live.pop_front() {
                for p in old {
                    p.stop(&mut self.loose);
                }
            }
        }
        let mut cluster = Vec::with_capacity(specs.len());
        for (spec, body) in specs.iter().zip(bodies) {
            let at = body.w_axis.truncate();
            let d = at - centre;
            let dir = (d + Vec3::Y * (spec.up * d.length()))
                .try_normalize()
                .unwrap_or(Vec3::Y);
            let vel = launch_velocity(
                velocity,
                dir,
                d.length_squared(),
                radius,
                spec.speed,
                spec.inherit,
                spec.mode == 3,
                scale,
                rng,
            );
            let q = Quat::from_mat3(&Mat3::from_mat4(body)).normalize();
            let r = Mat3::from_quat(q);
            let world = r * Mat3::from_diagonal(spec.prim.inertia() * MASS) * r.transpose();
            let still = spec.mode == 2;
            let omega = r * spec.omega;
            let mut piece = Piece {
                spec: spec.clone(),
                pos: at,
                vel,
                rot: q,
                momentum: world * omega,
                omega,
                moving: !still,
                count: u8::from(still),
                budget: if still { 0 } else { spec.budget },
                touched: false,
                timer: 0.0,
                space: self.space,
                trails: Vec::new(),
                smoke: Vec::new(),
            };
            if let Some(lib) = lib {
                for (name, node) in &spec.trails {
                    if let Some(def) = lib.get(name) {
                        let frame = piece.trail_frame(*node);
                        piece
                            .trails
                            .push((LiveEffect::new(def.clone(), frame, rng), *node));
                    }
                }
            }
            cluster.push(piece);
        }
        self.live.push_back(cluster);
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.live.iter_mut().flatten() {
            for e in p.trails.iter_mut().map(|(e, _)| e).chain(&mut p.smoke) {
                e.advance(dt);
            }
        }
        for e in &mut self.loose {
            e.advance(dt);
        }
        self.loose.retain(|e| !e.is_dead());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draws(
        &self,
        era: Era,
        eye: Vec3,
        forward: Vec3,
        zoom: f32,
        bias: f32,
        assets: &mut dyn FxAssets,
        out: &mut Vec<render::ParticleDraw>,
    ) {
        for p in self.live.iter().flatten() {
            let name = p.spec.model.to_ascii_lowercase();
            if self.lit.contains(&name) {
                continue;
            }
            if let Some(model) = assets.model(&name) {
                let alpha = (p.opacity() * 255.0) as u8;
                model.draws(p.model(), [255, 255, 255, alpha], p.pos, false, era, out);
            }
        }
        let effects = self
            .live
            .iter()
            .flatten()
            .flat_map(|p| p.trails.iter().map(|(e, _)| e).chain(&p.smoke))
            .chain(&self.loose);
        for e in effects {
            e.draws_with_bias(eye, forward, zoom, bias, assets, out);
        }
    }

    pub fn models(&self) -> impl Iterator<Item = &str> {
        self.defs.values().flatten().map(|s| s.model.as_str())
    }

    pub fn set_lit(&mut self, names: impl IntoIterator<Item = String>) {
        self.lit.extend(names);
    }

    pub fn lit_poses(&self) -> impl Iterator<Item = (&str, Mat4, f32)> {
        self.live
            .iter()
            .flatten()
            .filter(|p| self.lit.contains(&p.spec.model.to_ascii_lowercase()))
            .map(|p| (p.spec.model.as_str(), p.model(), p.opacity()))
    }

    pub fn len(&self) -> usize {
        self.live.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.loose.clear();
        self.posed_nodes.clear();
    }

    pub fn piece_states(&self) -> Vec<(Vec3, Vec3)> {
        self.live.iter().flatten().map(|p| (p.pos, p.vel)).collect()
    }

    pub fn set_piece_vel(&mut self, cluster: usize, piece: usize, vel: Vec3) {
        if let Some(c) = self.live.get_mut(cluster) {
            if let Some(p) = c.get_mut(piece) {
                p.vel = vel;
            }
        }
    }
}

#[derive(Clone, Default)]
struct Slot {
    props: HashMap<u32, String>,
    trails: Vec<(String, Option<String>)>,
    smoke: Vec<(String, Option<String>)>,
    trail_count: usize,
    smoke_count: usize,
}

impl Slot {
    fn take(&mut self, key: u32, value: &str) {
        use crate::classdb::fnv1a_lowercase as hash;
        let put = |list: &mut Vec<(String, Option<String>)>, at: usize| {
            if at < list.len() {
                list[at] = (value.to_owned(), None);
            } else {
                list.push((value.to_owned(), None));
            }
        };
        if key == hash("ChunkGeometryName") {
            self.trail_count = 0;
            self.smoke_count = 0;
            self.props.insert(key, value.to_owned());
        } else if key == hash("ChunkTrailEffect") {
            if self.trail_count < 3 {
                put(&mut self.trails, self.trail_count);
                self.trail_count += 1;
            }
        } else if key == hash("ChunkSmokeEffect") {
            if self.smoke_count < 3 {
                put(&mut self.smoke, self.smoke_count);
                self.smoke_count += 1;
            }
        } else if key == hash("ChunkTrailNodeName") {
            if let Some(t) = self
                .trail_count
                .checked_sub(1)
                .and_then(|i| self.trails.get_mut(i))
            {
                t.1 = Some(value.to_owned());
            }
        } else if key == hash("ChunkSmokeNodeName") {
            if let Some(s) = self
                .smoke_count
                .checked_sub(1)
                .and_then(|i| self.smoke.get_mut(i))
            {
                s.1 = Some(value.to_owned());
            }
        } else {
            self.props.insert(key, value.to_owned());
        }
    }
}

fn chunk_properties(chain: Vec<&crate::classdb::ClassDef>) -> Vec<Slot> {
    use crate::classdb::fnv1a_lowercase as hash;
    let keys: Vec<_> = [
        "ChunkGeometryName",
        "ChunkSpeed",
        "ChunkVelocityFactor",
        "ChunkUpFactor",
        "ChunkBounciness",
        "ChunkStickiness",
        "ChunkLifetime",
        "ChunkPhysics",
        "ChunkTerrainCollisions",
        "ChunkTerrainEffect",
        "ChunkOmega",
        "ChunkGravity",
        "ChunkLinearDamping",
        "ChunkAngularDamping",
        "ChunkTrailEffect",
        "ChunkTrailNodeName",
        "ChunkSmokeEffect",
        "ChunkSmokeNodeName",
        "ChunkNodeName",
        "NumChunks",
    ]
    .map(hash)
    .into();
    let mut slots: std::collections::BTreeMap<String, Slot> = Default::default();
    let mut defaults = Slot::default();
    for def in chain.into_iter().rev() {
        let mut slot = None;
        let mut implicit = 0;
        for (key, value) in &def.props {
            if *key == hash("CHUNKSECTION") {
                slot = Some(value.to_ascii_lowercase());
            } else if keys.contains(key) {
                if *key == hash("ChunkGeometryName") && slot.is_none() {
                    slot = Some(format!("implicit{implicit}"));
                    implicit += 1;
                }
                if let Some(slot) = &slot {
                    slots
                        .entry(slot.clone())
                        .or_insert_with(|| defaults.clone())
                        .take(*key, value);
                } else {
                    defaults.take(*key, value);
                }
            }
        }
    }
    slots.into_values().collect()
}

#[allow(clippy::too_many_arguments)]
fn launch_velocity(
    parent: Vec3,
    dir: Vec3,
    distance2: f32,
    radius: f32,
    speed: f32,
    inherit: f32,
    leaf: bool,
    scale: f32,
    rng: &mut Lcg,
) -> Vec3 {
    let mut v = parent * inherit;
    if speed >= 0.0 {
        v += dir * speed;
    } else {
        if distance2 > 1.0 {
            v += dir * ((15.0 + 15.0 * rng.float()) / radius.max(0.001));
        }
        if leaf {
            v *= 0.25;
        }
        if v.length() < 5.0 {
            v = dir * (4.0 + 2.0 * rng.float());
            if v.y < 2.0 {
                v.y = 2.0 + rng.float();
            }
        }
    }
    v * scale
}

#[cfg(test)]
mod tests;
