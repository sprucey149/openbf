//! Third-person camera collision: keep the chase camera out of world
//! geometry by shortening the track when something sits between the player
//! and the camera.
//!
//! The PS2's rule (`analysis/re-aim-assist.md` §10.6 step 4, B1
//! `FUN_001b58f0` B1:59447-59649, BF2 `FUN_001cef68`): five parallel rays —
//! the centre and the near plane's four corners — go from a fixed point on
//! the soldier to the wanted eye, against the building and terrain layers;
//! the smallest hit fraction is the track's. A smaller fraction than last
//! frame's is taken at once, a larger one only when it is 1.0 or more than
//! 0.15 above, and then approached at 0.75 per second (B1 `FUN_0019fd70`,
//! [`ease_pull`]). The eye is the origin plus the track times that fraction.
//!
//! Ours (PROVISIONAL, playtest 2026-10-28: the camera inside a health droid,
//! then "the camera goes to the floor looking at my feet" healing beside
//! one; `analysis/camera-occlusion.md`):
//! - Only BIG world geometry pulls the camera in: a map instance whose
//!   model is more than [`PROP_MAX_FOOTPRINT`] across. Its drawn triangles
//!   (not the `coll` bodies; terrain is `WalkSim::settle_camera`'s) are cast
//!   as a sphere of [`CAMERA_RADIUS`] swept along the track, exact against
//!   each triangle — the PS2's five rays carry its near plane's 0.1 m
//!   rectangle, the port keeps a wider margin. An instance AABB is only the
//!   broad phase: Mos Eisley's buildings are concave, and their bounding
//!   boxes swallow whole streets.
//! - SMALL props (health / ammo droids, gun emplacements, columns, lamps)
//!   and every vehicle never pull the camera: while one stands in the
//!   camera's sight of the player, or holds the camera, it is drawn
//!   see-through ([`CameraBlockers::see_through`], [`see_through_vehicles`]:
//!   an ordered dither at [`SEE_THROUGH_OPACITY`], hidden outright with the
//!   camera inside it). They stay solid for the sim — this module is visual
//!   only. A droid (`powerupstation`) is tested at the pose it is drawn at.
//!   Soldiers neither pull nor fade.
//! - A soldier's track runs from the PS2's own origin, 1.5 m above his feet
//!   (`walk::CAMERA_PULL_Y`), to the ideal camera; the pulled-in camera
//!   keeps the ideal's view (its pitch and yaw, the look along his aim), so
//!   it never drops to the floor or turns to his feet. When it comes into
//!   him, his body is hidden ([`BODY_HIDE_DIST`]) — the PS2 ghosts him there
//!   (`re-muzzle-flash.md` §7) — and the view is his own, along the same aim.

use crate::assets::Scene;
use core::collision::{CollisionWorld, HullFilter, Layers};
use core::vehicle::VehicleHandle;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

/// Camera sphere radius (m): how far from every drawn surface the camera
/// stays. PROVISIONAL (ours), wider than the near plane's corners (0.1 m
/// away, about 0.12 m from the eye at the PS2's FOV), so the near plane never
/// cuts into what the camera backed up against.
pub const CAMERA_RADIUS: f32 = 0.35;
/// The player's body is hidden while the drawn camera is closer than this to
/// the point it is pulled in toward (m). PROVISIONAL (ours): the PS2 ghosts
/// the trooper instead (`re-muzzle-flash.md` §7). At 1.2 m the 52 deg view
/// spans 1.2 m top to bottom, too little to frame a 2 m soldier: his body
/// would only wall the view off, so it goes and the view is his aim's.
pub const BODY_HIDE_DIST: f32 = 1.2;
/// The rate a pulled-in camera returns at (track fraction per second) and
/// the rise it waits for (B1 `FUN_0019fd70`: 0.75 and 0.15).
pub const PULL_RETURN_RATE: f32 = 0.75;
pub const PULL_RETURN_STEP: f32 = 0.15;
/// A map instance whose model is at most this wide (m, the larger of its two
/// horizontal extents at its instance scale) is a PROP: it never pulls the
/// camera in, it is drawn see-through. PROVISIONAL (ours): it takes the
/// droids (1.4-1.9 m), the gun emplacements (2.4-3.1 m), columns, lamps and
/// small trees, and leaves walls, doors, buildings and rocks (4.5 m up)
/// solid (measured on BF1 NAB1 / YAV2, BF2 KAS2 / NAB2).
pub const PROP_MAX_FOOTPRINT: f32 = 4.0;
/// A see-through prop's opacity while it stands between the camera and the
/// player (an ordered dither, `render::Fade::see_through`). PROVISIONAL.
pub const SEE_THROUGH_OPACITY: f32 = 0.3;
/// The sight lines' thickness (m): a sphere this size swept from the camera
/// toward the player finds the props in the way.
pub const SIGHT_RADIUS: f32 = 0.15;
/// The sight lines stop this short of the player's points (m): a prop past
/// them, or touching one, is not in the way.
pub const SIGHT_END_GAP: f32 = 0.3;

/// The PS2's stored track fraction one update on (B1 `FUN_0019fd70`): a
/// smaller `raw` at once; a larger one only when it is 1.0 or more than
/// [`PULL_RETURN_STEP`] above `stored`, approached by
/// [`PULL_RETURN_RATE`]` * dt`.
pub fn ease_pull(stored: f32, raw: f32, dt: f32) -> f32 {
    if raw < stored {
        raw
    } else if raw == 1.0 || stored + PULL_RETURN_STEP < raw {
        raw.min(stored + dt * PULL_RETURN_RATE)
    } else {
        stored
    }
}

/// What one frame's chase camera sees through: map props by manifest index
/// and vehicles, each with its opacity (0 = hidden). The renderer draws them
/// so (`viewer::SceneGpu::see_through`, `viewer::vehicles` `draws`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SeeThrough {
    pub props: Vec<(u32, f32)>,
    pub vehicles: Vec<(VehicleHandle, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    fn empty() -> Aabb {
        Aabb {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        }
    }

    fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    fn grown(&self, r: f32) -> Aabb {
        Aabb {
            min: self.min - Vec3::splat(r),
            max: self.max + Vec3::splat(r),
        }
    }

    fn contains(&self, p: Vec3) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }

    /// World AABB of `local` transformed by `m` (bounds of the 8 corners).
    pub fn transformed(local: Aabb, m: Mat4) -> Aabb {
        let mut out = Aabb::empty();
        for i in 0..8 {
            out.add(m.transform_point3(Vec3::new(
                if i & 1 == 0 { local.min.x } else { local.max.x },
                if i & 2 == 0 { local.min.y } else { local.max.y },
                if i & 4 == 0 { local.min.z } else { local.max.z },
            )));
        }
        out
    }

    /// Does the segment `from -> from + dir*len` reach this box grown by
    /// `radius`? Broad phase only — a conservative yes/no.
    fn hits(&self, from: Vec3, dir: Vec3, len: f32, radius: f32) -> bool {
        let (lo, hi) = (
            self.min - Vec3::splat(radius),
            self.max + Vec3::splat(radius),
        );
        let (mut t0, mut t1) = (0.0f32, len);
        for a in 0..3 {
            let (o, d) = (from[a], dir[a]);
            if d.abs() < 1e-6 {
                if o < lo[a] || o > hi[a] {
                    return false;
                }
                continue;
            }
            let (mut n, mut f) = ((lo[a] - o) / d, (hi[a] - o) / d);
            if n > f {
                std::mem::swap(&mut n, &mut f);
            }
            t0 = t0.max(n);
            t1 = t1.min(f);
            if t0 > t1 {
                return false;
            }
        }
        true
    }
}

/// Model-space triangle soup, shared by every instance of one model.
#[derive(Debug)]
struct TriMesh {
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    local: Aabb,
    /// Bounds of consecutive batches, shared by all instances of the model.
    /// Triangle order stays unchanged (including equal-distance contacts).
    chunks: Vec<Aabb>,
}

const TRIANGLES_PER_CHUNK: usize = 16;

fn triangle_chunks(positions: &[Vec3], indices: &[u32]) -> Vec<Aabb> {
    indices
        .chunks(3 * TRIANGLES_PER_CHUNK)
        .map(|chunk| {
            let mut bounds = Aabb::empty();
            for &i in chunk {
                let p = positions[i as usize];
                if !p.is_finite() {
                    return Aabb {
                        min: Vec3::splat(f32::NEG_INFINITY),
                        max: Vec3::splat(f32::INFINITY),
                    };
                }
                bounds.add(p);
            }
            bounds
        })
        .collect()
}

/// The point of triangle `abc` nearest `p` (Ericson, Real-Time Collision
/// Detection 5.1.5).
pub fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Entry time of the ray `o + d t` (`d` unit) into the sphere (`c`, `r`),
/// within `0..=max`; the ray starts outside it.
fn ray_sphere(o: Vec3, d: Vec3, max: f32, c: Vec3, r: f32) -> Option<f32> {
    let oc = o - c;
    let b = d.dot(oc);
    let disc = b * b - (oc.length_squared() - r * r);
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (0.0..=max).contains(&t).then_some(t)
}

/// Entry time of the ray into the cylinder of radius `r` about segment
/// `p0 p1`, between its ends (the end spheres are [`ray_sphere`]'s).
fn ray_edge(o: Vec3, d: Vec3, max: f32, p0: Vec3, p1: Vec3, r: f32) -> Option<f32> {
    let e = p1 - p0;
    let ee = e.length_squared();
    if ee < 1e-12 {
        return None;
    }
    let m = o - p0;
    let dp = d - e * (d.dot(e) / ee);
    let mp = m - e * (m.dot(e) / ee);
    let a = dp.length_squared();
    if a < 1e-12 {
        return None;
    }
    let b = dp.dot(mp);
    let disc = b * b - a * (mp.length_squared() - r * r);
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    if !(0.0..=max).contains(&t) {
        return None;
    }
    let u = (m + d * t).dot(e) / ee;
    (0.0..=1.0).contains(&u).then_some(t)
}

/// How far (within `max`) a sphere of radius `r` at `o` moves along `d`
/// (unit length) before it touches triangle `abc`: the first entry into the
/// triangle grown by `r` (its two face planes, three edge cylinders, three
/// corner spheres). Two-sided, as PS2 map models are not reliably wound
/// outward. A sphere already touching the triangle stops at 0 only when it
/// moves toward it, so a soldier with his back to a wall can still swing the
/// camera along it.
pub fn sweep_sphere_tri(
    o: Vec3,
    d: Vec3,
    max: f32,
    r: f32,
    a: Vec3,
    b: Vec3,
    c: Vec3,
) -> Option<f32> {
    let near = closest_on_triangle(o, a, b, c) - o;
    if near.length_squared() < r * r {
        return (d.dot(near) > 0.0).then_some(0.0);
    }
    let mut best: Option<f32> = None;
    let mut take = |t: Option<f32>| {
        if let Some(t) = t {
            if best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
    };
    let n = (b - a).cross(c - a);
    if let Some(n) = n.try_normalize() {
        let dist = (o - a).dot(n);
        let n = if dist < 0.0 { -n } else { n };
        let closing = -d.dot(n);
        if closing > 1e-8 {
            let t = (dist.abs() - r) / closing;
            if (0.0..=max).contains(&t) {
                let p = o + d * t - n * r;
                if closest_on_triangle(p, a, b, c).distance_squared(p) < 1e-8 {
                    take(Some(t));
                }
            }
        }
    }
    for (p0, p1) in [(a, b), (b, c), (c, a)] {
        take(ray_edge(o, d, max, p0, p1, r));
    }
    for v in [a, b, c] {
        take(ray_sphere(o, d, max, v, r));
    }
    best
}

#[derive(Debug)]
struct Blocker {
    /// The instance's world box; a droid's is the box of every turn about
    /// its +Y.
    world: Aabb,
    /// World -> model, so one segment test serves every instance of a model.
    inv: Mat4,
    /// The model's units per world metre (1 for a rigid instance): the
    /// sphere's radius in model space.
    scale: f32,
    mesh: Arc<TriMesh>,
    /// The manifest instance (a building turret's camera passes its own).
    instance: u32,
    /// A droid: drawn at the pose `CameraBlockers::fraction`'s `live` gives.
    moves: bool,
    /// At most [`PROP_MAX_FOOTPRINT`] across: drawn see-through, never pulls.
    prop: bool,
}

/// World blockers for the chase camera. Visual only — never read by the sim,
/// so it cannot affect determinism.
#[derive(Debug, Default)]
pub struct CameraBlockers {
    blockers: Vec<Blocker>,
}

impl CameraBlockers {
    /// One blocker per drawable instance in the scene (terrain excluded —
    /// `camera()` already floors against the heightfield).
    pub fn from_scene(scene: &Scene) -> Self {
        let chunked = std::env::var("OPENBF_CAMERA_CHUNKS").as_deref() != Ok("0");
        let mut meshes: HashMap<String, Arc<TriMesh>> = HashMap::new();
        let mut blockers = Vec::new();
        for (ii, inst) in scene.manifest.instances.iter().enumerate() {
            let Some(model) = scene.resolve_model(&inst.class) else {
                continue;
            };
            if !meshes.contains_key(model) {
                let cpu = &scene.meshes[model];
                let positions: Vec<Vec3> = cpu.positions.iter().copied().map(Vec3::from).collect();
                let mut local = Aabb::empty();
                for p in &positions {
                    local.add(*p);
                }
                // Keep only triangles whose indices land in `positions`: the
                // GLB's indices are never range-checked on load, and
                // `fraction` indexes with them (the collision and render
                // paths filter the same way).
                let indices: Vec<u32> = cpu
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter(|tri| tri.iter().all(|&i| (i as usize) < positions.len()))
                    .flatten()
                    .copied()
                    .collect();
                let chunks = if chunked {
                    triangle_chunks(&positions, &indices)
                } else {
                    Vec::new()
                };
                meshes.insert(
                    model.to_owned(),
                    Arc::new(TriMesh {
                        positions,
                        indices,
                        local,
                        chunks,
                    }),
                );
            }
            let mesh = meshes[model].clone();
            if mesh.local.is_empty() || mesh.indices.len() < 3 {
                continue;
            }
            let m = crate::viewer::instance_matrix(inst.rotation, inst.translation);
            let moves = scene.resolve_class(&inst.class).is_some_and(|c| {
                c.base
                    .eq_ignore_ascii_case(crate::viewer::stations::STATION_BASE)
            });
            blockers.push(Blocker::new(mesh, m, ii as u32, moves));
        }
        Self { blockers }
    }

    pub fn is_empty(&self) -> bool {
        self.blockers.is_empty()
    }

    /// Pull `desired` toward `target` until the camera clears the scene's
    /// static geometry by `CAMERA_RADIUS`, props included: the gallery's
    /// framing, which draws nothing see-through.
    pub fn pull_in(&self, target: Vec3, desired: Vec3) -> Vec3 {
        target + (desired - target) * self.fraction_of(target, desired, None, |_| None, true)
    }

    /// The fraction (0..=1) of the track `target -> desired` a sphere of
    /// [`CAMERA_RADIUS`] travels before it touches a big model's drawn
    /// triangle (props never pull, [`Self::see_through`]). `skip` is passed
    /// through (a building turret's own building); `live` gives a droid's
    /// drawn pose by its manifest instance (None: its placed one).
    pub fn fraction(
        &self,
        target: Vec3,
        desired: Vec3,
        skip: Option<u32>,
        live: impl Fn(u32) -> Option<Mat4>,
    ) -> f32 {
        self.fraction_of(target, desired, skip, live, false)
    }

    fn fraction_of(
        &self,
        target: Vec3,
        desired: Vec3,
        skip: Option<u32>,
        live: impl Fn(u32) -> Option<Mat4>,
        props: bool,
    ) -> f32 {
        let delta = desired - target;
        let len = delta.length();
        if len < 1e-4 {
            return 1.0;
        }
        let dir = delta / len;
        let mut t = len;
        for b in &self.blockers {
            if (b.prop && !props)
                || Some(b.instance) == skip
                || !b.world.hits(target, dir, t, CAMERA_RADIUS)
            {
                continue;
            }
            if let Some(hit) = b.sweep(b.pose_inv(&live), target, dir, t, CAMERA_RADIUS) {
                t = t.min(hit);
            }
        }
        t / len
    }

    /// The props to draw see-through for a camera at `eye` filming the
    /// points `sights` (the player's eye and body), by manifest index: at 0
    /// (hidden) when the camera is inside one's box grown by
    /// [`CAMERA_RADIUS`], else at [`SEE_THROUGH_OPACITY`] when a sphere of
    /// [`SIGHT_RADIUS`] swept from the camera to within [`SIGHT_END_GAP`] of
    /// a sight point touches it. `skip` and `live` as [`Self::fraction`].
    pub fn see_through(
        &self,
        eye: Vec3,
        sights: &[Vec3],
        skip: Option<u32>,
        live: impl Fn(u32) -> Option<Mat4>,
    ) -> Vec<(u32, f32)> {
        let mut out = Vec::new();
        for b in self.blockers.iter().filter(|b| b.prop) {
            if Some(b.instance) == skip {
                continue;
            }
            let pose = b.moves.then(|| live(b.instance)).flatten();
            let near = pose.map_or(b.world, |m| Aabb::transformed(b.mesh.local, m));
            if near.grown(CAMERA_RADIUS).contains(eye) {
                out.push((b.instance, 0.0));
                continue;
            }
            let inv = pose.map_or(b.inv, |m| m.inverse());
            let blocks = sights.iter().any(|&s| {
                let dist = s.distance(eye);
                let len = dist - SIGHT_END_GAP;
                if len <= 0.0 {
                    return false;
                }
                let dir = (s - eye) / dist;
                b.world.hits(eye, dir, len, SIGHT_RADIUS)
                    && b.sweep(inv, eye, dir, len, SIGHT_RADIUS).is_some()
            });
            if blocks {
                out.push((b.instance, SEE_THROUGH_OPACITY));
            }
        }
        out
    }

    /// Is manifest instance `ii` a see-through prop (`None`: no blocker)?
    pub fn is_prop(&self, ii: u32) -> Option<bool> {
        self.blockers
            .iter()
            .find(|b| b.instance == ii)
            .map(|b| b.prop)
    }
}

impl Blocker {
    /// World -> model at the pose `live` gives a droid, else the placed one.
    fn pose_inv(&self, live: &impl Fn(u32) -> Option<Mat4>) -> Mat4 {
        match self.moves.then(|| live(self.instance)).flatten() {
            Some(pose) => pose.inverse(),
            None => self.inv,
        }
    }

    /// How far (m, within `len`) a sphere of `radius` at `from` moves along
    /// `dir` (unit) before it touches this model, placed by `inv`
    /// (world -> model).
    fn sweep(&self, inv: Mat4, from: Vec3, dir: Vec3, len: f32, radius: f32) -> Option<f32> {
        // Empty only in diagnostic reference mode: the original complete scan.
        if self.mesh.chunks.is_empty() {
            self.sweep_filtered::<false>(inv, from, dir, len, radius)
        } else {
            self.sweep_filtered::<true>(inv, from, dir, len, radius)
        }
    }

    fn sweep_filtered<const FILTER: bool>(
        &self,
        inv: Mat4,
        from: Vec3,
        dir: Vec3,
        len: f32,
        radius: f32,
    ) -> Option<f32> {
        let o = inv.transform_point3(from);
        let d = inv.transform_vector3(dir);
        // Model units per metre along the track (1 for a rigid instance).
        let k = d.length();
        if k < 1e-6 {
            return None;
        }
        let (d, r) = (d / k, radius * self.scale);
        let end = o + d * (len * k);
        // A broad phase only: include the triangle routine's 0.1 mm face
        // tolerance and ample roundoff room for its quadratic edge/vertex
        // tests. Non-finite tracks take the original complete scan.
        let magnitude = o.abs().max(end.abs()).max_element();
        let padding = r + 0.001 + (64.0 * f32::EPSILON).sqrt() * magnitude;
        let lo = o.min(end) - Vec3::splat(padding);
        let hi = o.max(end) + Vec3::splat(padding);
        let finite = o.is_finite() && end.is_finite() && padding.is_finite();
        let mut best: Option<f32> = None;
        for (i, chunk) in self
            .mesh
            .indices
            .chunks(3 * TRIANGLES_PER_CHUNK)
            .enumerate()
        {
            if FILTER && finite {
                let bounds = self.mesh.chunks[i];
                if hi.cmplt(bounds.min).any() || lo.cmpgt(bounds.max).any() {
                    continue;
                }
            }
            for tri in chunk.as_chunks::<3>().0 {
                let [a, b, c] = tri.map(|i| self.mesh.positions[i as usize]);
                let reach = best.map_or(len, |t| t) * k;
                if let Some(hit) = sweep_sphere_tri(o, d, reach, r, a, b, c) {
                    best = Some(hit / k);
                }
            }
        }
        best
    }

    fn new(mesh: Arc<TriMesh>, m: Mat4, instance: u32, moves: bool) -> Blocker {
        let world = if moves {
            // Every turn about the model's +Y: the widest reach from its axis
            // in x and z.
            let reach = mesh
                .positions
                .iter()
                .map(|p| (p.x * p.x + p.z * p.z).sqrt())
                .fold(0.0f32, f32::max);
            let spun = Aabb {
                min: Vec3::new(-reach, mesh.local.min.y, -reach),
                max: Vec3::new(reach, mesh.local.max.y, reach),
            };
            Aabb::transformed(spun, m)
        } else {
            Aabb::transformed(mesh.local, m)
        };
        let inv = m.inverse();
        // The largest model-space stretch of a world metre: a sphere that
        // size in the model covers the world sphere under any scale.
        let scale = [Vec3::X, Vec3::Y, Vec3::Z]
            .map(|v| inv.transform_vector3(v).length())
            .into_iter()
            .fold(0.0f32, f32::max);
        // The model's horizontal extents at the instance's scale.
        let ext = mesh.local.max - mesh.local.min;
        let footprint =
            (ext.x * m.x_axis.truncate().length()).max(ext.z * m.z_axis.truncate().length());
        Blocker {
            world,
            inv,
            scale,
            mesh,
            instance,
            moves,
            prop: footprint <= PROP_MAX_FOOTPRINT,
        }
    }
}

fn camera_meets_hull(body: &core::collision::Body, eye: Vec3) -> bool {
    use core::collision::Shape;
    let p = body.to_local.transform_point3(eye);
    let radius = CAMERA_RADIUS
        * [Vec3::X, Vec3::Y, Vec3::Z]
            .map(|a| body.to_local.transform_vector3(a).length())
            .into_iter()
            .fold(0.0f32, f32::max);
    match &body.shape {
        Shape::Sphere { radius: r } => p.length_squared() <= ((r + radius) * (r + radius)),
        Shape::Cylinder {
            radius: r,
            half_height,
        } => {
            let radial = ((p.x * p.x + p.z * p.z).sqrt() - r).max(0.0);
            let vertical = (p.y.abs() - half_height).max(0.0);
            radial * radial + vertical * vertical <= radius * radius
        }
        Shape::Box { half } => {
            (p.abs() - *half).max(Vec3::ZERO).length_squared() <= radius * radius
        }
        Shape::Mesh(mesh) => mesh.indices().iter().any(|t| {
            let [a, b, c] = t.map(|i| {
                let p = mesh.vertices()[i as usize];
                Vec3::new(p.x, p.y, p.z)
            });
            closest_on_triangle(p, a, b, c).distance_squared(p) <= radius * radius
        }),
    }
}

/// The vehicles to draw see-through for a camera at `eye` filming `sights`
/// (ours: no vehicle pulls the camera in): [`CameraBlockers::see_through`]'s
/// rule against each vehicle's collision hull, `own` (the rider's) excepted.
pub fn see_through_vehicles(
    world: &CollisionWorld,
    vehicles: impl Iterator<Item = VehicleHandle>,
    eye: Vec3,
    sights: &[Vec3],
    own: Option<VehicleHandle>,
) -> Vec<(VehicleHandle, f32)> {
    let mut out = Vec::new();
    for h in vehicles.filter(|h| Some(*h) != own) {
        let bodies = world.hull(h);
        if bodies.is_empty() {
            continue;
        }
        let mut near = Aabb::empty();
        for b in bodies {
            near.add(b.aabb.min);
            near.add(b.aabb.max);
        }
        if near.grown(CAMERA_RADIUS).contains(eye)
            && bodies.iter().any(|b| camera_meets_hull(b, eye))
        {
            out.push((h, 0.0));
            continue;
        }
        let blocks = sights.iter().any(|&s| {
            let dist = s.distance(eye);
            let len = dist - SIGHT_END_GAP;
            if len <= 0.0 {
                return false;
            }
            let dir = (s - eye) / dist;
            near.hits(eye, dir, len, SIGHT_RADIUS)
                && world
                    .ball_cast(
                        eye,
                        dir,
                        len,
                        SIGHT_RADIUS,
                        Layers::ORDNANCE,
                        HullFilter::Only(h),
                    )
                    .is_some()
        });
        if blocks {
            out.push((h, SEE_THROUGH_OPACITY));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(positions: Vec<Vec3>, indices: Vec<u32>) -> Arc<TriMesh> {
        let mut local = Aabb::empty();
        for p in &positions {
            local.add(*p);
        }
        let chunks = triangle_chunks(&positions, &indices);
        Arc::new(TriMesh {
            positions,
            indices,
            local,
            chunks,
        })
    }

    fn one(mesh: Arc<TriMesh>, m: Mat4, moves: bool) -> CameraBlockers {
        CameraBlockers {
            blockers: vec![Blocker::new(mesh, m, 7, moves)],
        }
    }

    /// A 10x10 m wall slab in the z = [-2.5, -2.0] plane, at identity.
    fn slab() -> Arc<TriMesh> {
        let quads: [[Vec3; 4]; 2] = [
            [
                Vec3::new(-5.0, -5.0, -2.0),
                Vec3::new(5.0, -5.0, -2.0),
                Vec3::new(5.0, 5.0, -2.0),
                Vec3::new(-5.0, 5.0, -2.0),
            ],
            [
                Vec3::new(-5.0, -5.0, -2.5),
                Vec3::new(5.0, -5.0, -2.5),
                Vec3::new(5.0, 5.0, -2.5),
                Vec3::new(-5.0, 5.0, -2.5),
            ],
        ];
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for q in quads {
            let b = positions.len() as u32;
            positions.extend_from_slice(&q);
            indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        mesh(positions, indices)
    }

    fn wall() -> CameraBlockers {
        one(slab(), Mat4::IDENTITY, false)
    }

    /// A 0.1 m square post from y -5 to 5 at (x, z), four sides.
    fn post(x: f32, z: f32) -> Arc<TriMesh> {
        let h = 0.05;
        let c = [(-h, -h), (h, -h), (h, h), (-h, h)];
        let mut positions = Vec::new();
        for (dx, dz) in c {
            positions.push(Vec3::new(x + dx, -5.0, z + dz));
            positions.push(Vec3::new(x + dx, 5.0, z + dz));
        }
        let mut indices = Vec::new();
        for i in 0..4u32 {
            let (a, b) = (2 * i, 2 * ((i + 1) % 4));
            indices.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
        mesh(positions, indices)
    }

    #[test]
    fn camera_stops_short_of_a_wall() {
        let got = wall().pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0));
        assert!(
            (got.z - (-2.0 + CAMERA_RADIUS)).abs() < 1e-3,
            "stopped at {got:?}"
        );
    }

    #[test]
    fn clear_line_keeps_the_full_track_distance() {
        assert_eq!(
            wall().pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, 4.0)),
            Vec3::new(0.0, 0.0, 4.0)
        );
    }

    #[test]
    fn a_miss_to_the_side_does_not_pull_in() {
        assert_eq!(
            wall().pull_in(Vec3::new(20.0, 0.0, 0.0), Vec3::new(20.0, 0.0, -4.0)),
            Vec3::new(20.0, 0.0, -4.0)
        );
    }

    /// The reason the triangle pass exists: a concave model's AABB covers
    /// open ground, and a box-only test collapsed the camera there. With the
    /// triangle pass, a segment through the gap keeps its full track.
    #[test]
    fn concave_model_does_not_block_through_its_opening() {
        let mut m = mesh(
            vec![
                Vec3::new(4.0, -5.0, -2.0),
                Vec3::new(5.0, -5.0, -2.0),
                Vec3::new(5.0, 5.0, -2.0),
            ],
            vec![0, 1, 2],
        );
        // Keep the slab's whole box: the AABB spans the origin's track.
        Arc::get_mut(&mut m).unwrap().local = Aabb {
            min: Vec3::new(-5.0, -5.0, -2.5),
            max: Vec3::new(5.0, 5.0, -2.0),
        };
        assert_eq!(
            one(m, Mat4::IDENTITY, false).pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0)),
            Vec3::new(0.0, 0.0, -4.0),
            "the AABB is hit but no triangle is"
        );
    }

    /// The droid bug: something right behind the player used to leave the
    /// camera at a 25 % floor of its track, inside it. Now the camera comes
    /// all the way in (the body is hidden there, `BODY_HIDE_DIST`).
    #[test]
    fn a_blocker_right_behind_the_target_takes_the_whole_track() {
        // The player 0.2 m in front of the slab's near face, the camera
        // wanted 4 m behind him through it.
        let got = wall().pull_in(Vec3::new(0.0, 0.0, -1.8), Vec3::new(0.0, 0.0, -5.8));
        assert_eq!(got, Vec3::new(0.0, 0.0, -1.8));
    }

    /// Touching a wall at the start blocks only a camera that moves into
    /// it: a soldier with his back to a wall can swing the camera along it.
    #[test]
    fn a_start_against_a_wall_blocks_only_toward_it() {
        let start = Vec3::new(0.0, 0.0, -1.8);
        let along = Vec3::new(3.0, 0.0, -1.8);
        assert_eq!(wall().pull_in(start, along), along);
        let away = Vec3::new(0.0, 0.0, 2.0);
        assert_eq!(wall().pull_in(start, away), away);
    }

    /// A thin post 0.2 m beside the track: a ray along the centre misses it,
    /// the swept sphere stops at its radius from it.
    #[test]
    fn a_post_beside_the_track_stops_the_sphere() {
        let b = Blocker::new(post(0.2, -3.0), Mat4::IDENTITY, 7, false);
        let dir = Vec3::NEG_Z;
        let t = b
            .sweep(b.inv, Vec3::ZERO, dir, 6.0, CAMERA_RADIUS)
            .expect("the sphere meets the post");
        let got = dir * t;
        assert!(got.z > -3.0, "{got:?}");
        let clear = post_clearance(&post(0.2, -3.0), got);
        assert!((clear - CAMERA_RADIUS).abs() < 1e-3, "{clear} {got:?}");
    }

    /// The playtest's ask (2026-10-28): a small prop between the camera and
    /// the player never pulls the camera in; it is drawn see-through, and
    /// hidden outright with the camera inside it. A prop beside the sight
    /// line, or past the player, stays whole.
    #[test]
    fn a_small_prop_is_see_through_and_never_pulls() {
        let b = one(post(0.2, -3.0), Mat4::IDENTITY, false);
        assert_eq!(b.is_prop(7), Some(true));
        assert_eq!(wall().is_prop(7), Some(false));
        let (player, cam) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -6.0));
        let none = |_| None;
        assert_eq!(
            b.fraction(player, cam, None, none),
            1.0,
            "a prop never pulls"
        );
        // The gallery's framing still stops at it.
        assert!(b.pull_in(player, cam).z > -3.0);
        assert_eq!(
            b.see_through(cam, &[player], None, none),
            vec![(7, SEE_THROUGH_OPACITY)]
        );
        // The camera inside the post's box: hidden.
        let inside = Vec3::new(0.2, 1.0, -3.0);
        assert_eq!(b.see_through(inside, &[player], None, none), vec![(7, 0.0)]);
        // Beside the sight line: whole.
        let aside = Vec3::new(3.0, 0.0, -6.0);
        assert!(b
            .see_through(aside, &[Vec3::new(3.0, 0.0, 0.0)], None, none)
            .is_empty());
        // Past the player (0.2 m beyond his sight point): whole.
        let lean = one(post(0.0, -0.2), Mat4::IDENTITY, false);
        assert!(lean
            .see_through(Vec3::new(0.0, 0.0, 6.0), &[player], None, none)
            .is_empty());
        // A skipped instance (a gunner's own building) is left alone.
        assert!(b.see_through(cam, &[player], Some(7), none).is_empty());
    }

    fn post_clearance(m: &TriMesh, p: Vec3) -> f32 {
        m.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize]);
                closest_on_triangle(p, a, b, c).distance(p)
            })
            .fold(f32::MAX, f32::min)
    }

    #[test]
    fn rotated_instance_transforms_the_segment() {
        // Same slab, yawed 90 deg: it now blocks along -x instead of -z.
        let b = one(
            slab(),
            Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
            false,
        );
        let got = b.pull_in(Vec3::ZERO, Vec3::new(-4.0, 0.0, 0.0));
        assert!((got.x - (-2.0 + CAMERA_RADIUS)).abs() < 1e-3, "{got:?}");
        assert_eq!(
            b.pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -4.0)),
            Vec3::new(0.0, 0.0, -4.0),
            "no longer blocks the old direction"
        );
    }

    /// A droid is cast where it is drawn: its live pose, not its placed one.
    #[test]
    fn a_turning_blocker_is_cast_at_its_live_pose() {
        let b = one(slab(), Mat4::IDENTITY, true);
        let turned = Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let live = |i: u32| (i == 7).then_some(turned);
        let to_x = Vec3::new(-4.0, 0.0, 0.0);
        let f = b.fraction(Vec3::ZERO, to_x, None, live);
        assert!((f * 4.0 - (2.0 - CAMERA_RADIUS)).abs() < 1e-3, "{f}");
        let to_z = Vec3::new(0.0, 0.0, -4.0);
        assert_eq!(b.fraction(Vec3::ZERO, to_z, None, live), 1.0);
        // No live pose: the placed one.
        assert!(b.fraction(Vec3::ZERO, to_z, None, |_| None) < 1.0);
    }

    #[test]
    fn a_skipped_instance_does_not_block() {
        let to = Vec3::new(0.0, 0.0, -4.0);
        assert_eq!(wall().fraction(Vec3::ZERO, to, Some(7), |_| None), 1.0);
        assert!(wall().fraction(Vec3::ZERO, to, Some(8), |_| None) < 1.0);
    }

    /// A model scaled by its instance: the sphere keeps its world radius.
    #[test]
    fn a_scaled_instance_keeps_the_world_radius() {
        let b = one(slab(), Mat4::from_scale(Vec3::splat(2.0)), false);
        let got = b.pull_in(Vec3::ZERO, Vec3::new(0.0, 0.0, -8.0));
        assert!((got.z - (-4.0 + CAMERA_RADIUS)).abs() < 1e-3, "{got:?}");
    }

    /// B1 `FUN_0019fd70`: in at once, out slowly and only past the step.
    #[test]
    fn the_pull_eases_out_as_the_ps2_does() {
        assert_eq!(ease_pull(1.0, 0.3, 1.0 / 30.0), 0.3);
        let dt = 1.0 / 30.0;
        assert_eq!(ease_pull(0.3, 0.4, dt), 0.3, "a rise under 0.15 waits");
        assert!((ease_pull(0.3, 0.5, dt) - (0.3 + 0.025)).abs() < 1e-6);
        assert!((ease_pull(0.3, 1.0, dt) - (0.3 + 0.025)).abs() < 1e-6);
        assert_eq!(ease_pull(0.99, 1.0, dt), 1.0, "never past the raw");
        let mut f = 0.0;
        let mut n = 0;
        while f < 1.0 {
            f = ease_pull(f, 1.0, dt);
            n += 1;
        }
        assert!(
            (40..=41).contains(&n),
            "4/3 s back from fully in: {n} ticks"
        );
    }

    #[test]
    fn transformed_box_bounds_all_corners() {
        let local = Aabb {
            min: Vec3::new(-1.0, 0.0, -1.0),
            max: Vec3::new(1.0, 2.0, 1.0),
        };
        let w = Aabb::transformed(local, Mat4::from_rotation_y(std::f32::consts::FRAC_PI_4));
        assert!((w.max.x - 2f32.sqrt()).abs() < 1e-5, "{w:?}");
        assert!((w.max.y - 2.0).abs() < 1e-5, "{w:?}");
    }

    #[test]
    fn chunked_sweeps_match_the_complete_triangle_scan_bit_for_bit() {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for x in -12..=12 {
            for z in -12..=12 {
                let at = positions.len() as u32;
                let (x, z) = (x as f32 * 2.0, z as f32 * 2.0);
                positions.extend([
                    Vec3::new(x, 0.0, z),
                    Vec3::new(x + 1.8, 0.0, z),
                    Vec3::new(x, 0.2, z + 1.8),
                ]);
                indices.extend([at, at + 1, at + 2]);
            }
        }
        let mesh = mesh(positions, indices);
        assert_eq!(mesh.chunks.len(), 40); // includes the final short batch
        let mut hits = 0;
        let mut misses = 0;
        for pose in [
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(170.0, -3.0, -250.0))
                * Mat4::from_rotation_y(0.71)
                * Mat4::from_scale(Vec3::new(0.7, 1.3, 1.8)),
        ] {
            let b = Blocker::new(mesh.clone(), pose, 7, true);
            for x in -10..=10 {
                for y in [-0.35, 0.0, 0.15, 0.35, 1.0, 3.0] {
                    let from = pose.transform_point3(Vec3::new(x as f32 * 2.0, y, -8.0));
                    for delta in [
                        Vec3::new(0.0, -2.0, 0.0),
                        Vec3::new(0.35, -0.1, 8.0),
                        Vec3::new(-3.0, 0.0, 8.0),
                    ] {
                        let end = pose.transform_point3(Vec3::new(x as f32 * 2.0, y, -8.0) + delta);
                        let len = end.distance(from);
                        let dir = (end - from) / len;
                        for radius in [SIGHT_RADIUS, CAMERA_RADIUS] {
                            let want = b.sweep_filtered::<false>(b.inv, from, dir, len, radius);
                            let got = b.sweep(b.inv, from, dir, len, radius);
                            assert_eq!(
                                got.map(f32::to_bits),
                                want.map(f32::to_bits),
                                "{from} {dir} {len} {radius}"
                            );
                            if got.is_some() {
                                hits += 1;
                            } else {
                                misses += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(hits > 100 && misses > 100, "{hits} hits / {misses} misses");
    }
}
