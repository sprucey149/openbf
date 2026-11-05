//! The geometry the world is built from: the soldier [`Capsule`] (its ray
//! test and the stepped shape the wall pass asks about), the [`Aabb`] every
//! broad phase uses, a body's [`Shape`] and the shared mesh builder
//! [`trimesh`], the [`Layers`] mask saying which queries a [`Body`] answers,
//! and the terrain [`Cutter`]. The layer-mask tests live here too: each of
//! the four world queries obeys the mask.

use super::{vector, Isometry, TriMesh, STEP_HEIGHT};
use crate::vehicle::VehicleHandle;
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use std::sync::Arc;

mod mesh_ray;

/// Vertical capsule: segment from `base + r·Y` to `base + (h − r)·Y`, radius `r`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub base: Vec3,
    pub height: f32,
    pub radius: f32,
}

impl Capsule {
    pub(super) fn segment(&self) -> (Vec3, Vec3) {
        let r = self.radius.min(self.height * 0.5);
        (
            self.base + Vec3::Y * r,
            self.base + Vec3::Y * (self.height - r),
        )
    }

    pub fn center(&self) -> Vec3 {
        self.base + Vec3::Y * (self.height * 0.5)
    }

    /// This capsule with its bottom [`STEP_HEIGHT`] cut off — the shape
    /// everything that asks "is there a WALL here?" uses, as opposed to the
    /// full capsule bolts, line of sight, unit separation and aim keep.
    ///
    /// Without it every kerb, ramp lip and stair tread reads as a wall: the
    /// walkability filter calls any face steeper than 60 deg one, and
    /// [`crate::sim::Sim::ground_under`] already lifts a soldier over anything
    /// up to `STEP_HEIGHT` (its probe starts there). Three rules have to agree
    /// about what a step is — the ground query, the AI's forward probe and the
    /// wall pass — and the wall pass was the odd one out: BF2 POL1 units 3 and
    /// 4 stood on a ramp for 33,840 ticks at `steer fwd` with `push 0.301`
    /// every tick, walking 0.3 m into a kerb between 0.2 and 0.5 m high and
    /// being pushed 0.3 m back out of it, while the lifted probe swept clear
    /// in all twelve directions.
    ///
    /// The lift is `STEP_HEIGHT.min(height * 0.5)`, so a prone capsule (0.55 m)
    /// keeps half of itself rather than inverting, and the height loses the
    /// same amount: the top does not move.
    ///
    /// On the PROBE the same lift was measured as a small effect when A.1
    /// added it (BF2 TAN1's blocked-forward count 656,798 → 650,987
    /// unit-ticks) — but that pair was taken against the MIRRORED wall pass
    /// (PLAN Log 2026-09-02). Re-measured at HEAD in the honest world (A.4
    /// re-read, 2026-09-03), the probe lift moves TAN1 the OTHER way
    /// (1,462,664 unit-ticks without it → 1,534,202 with it, both runs the
    /// full 60,000 ticks), and it is kept on OUTCOMES instead: without the
    /// lift the sweep drops 28 → 27 of 32 (BF2 GEO1 stops finishing), the
    /// median `stuck_max` rises 720 → 960 and the maps over 10k go 2 → 3.
    /// On the wall PASS the lift is not small either way, because the pass
    /// is what moves the unit.
    ///
    /// PROVISIONAL (ours). The PS2's step/slope rule is untraced — HOLD 13,
    /// which is why this changes WHICH CAPSULE is asked and not `STEP_HEIGHT`.
    pub fn stepped(&self) -> Capsule {
        let lift = STEP_HEIGHT.min(self.height * 0.5);
        Capsule {
            base: self.base + Vec3::Y * lift,
            height: self.height - lift,
            radius: self.radius,
        }
    }

    /// Distance along the ray (`dir` unit length) to the first hit within
    /// `max_t`, or None. Standard ray-vs-capsule: infinite cylinder test
    /// clipped to the segment, then the two end spheres.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let (a, b) = self.segment();
        let r = self.radius;
        let ab = b - a;
        let ao = origin - a;
        let ab2 = ab.length_squared();
        let mut best: Option<f32> = None;
        let mut consider = |t: f32| {
            if t >= 0.0 && t <= max_t && best.is_none_or(|bt| t < bt) {
                best = Some(t);
            }
        };
        // Cylinder body: components perpendicular to the axis.
        if ab2 > 0.0 {
            let d_perp = dir - ab * (dir.dot(ab) / ab2);
            let o_perp = ao - ab * (ao.dot(ab) / ab2);
            let qa = d_perp.length_squared();
            let qb = 2.0 * d_perp.dot(o_perp);
            let qc = o_perp.length_squared() - r * r;
            if qa > 1e-12 {
                let disc = qb * qb - 4.0 * qa * qc;
                if disc >= 0.0 {
                    let s = disc.sqrt();
                    for t in [(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)] {
                        let p = origin + dir * t;
                        let u = (p - a).dot(ab) / ab2;
                        if (0.0..=1.0).contains(&u) {
                            consider(t);
                        }
                    }
                }
            } else if qc <= 0.0 {
                // Ray parallel to the axis and inside the cylinder: hits the
                // caps below; nothing to add here.
            }
        }
        // End spheres.
        for c in [a, b] {
            let oc = origin - c;
            let qb = 2.0 * dir.dot(oc);
            let qc = oc.length_squared() - r * r;
            let disc = qb * qb - 4.0 * qc;
            if disc >= 0.0 {
                let s = disc.sqrt();
                consider((-qb - s) * 0.5);
                consider((-qb + s) * 0.5);
            }
        }
        best
    }

    /// World AABB of the capsule: radius in XZ, `base.y ..= base.y + height`.
    pub fn aabb(&self) -> Aabb {
        Aabb {
            min: self.base - Vec3::new(self.radius, 0.0, self.radius),
            max: self.base + Vec3::new(self.radius, self.height, self.radius),
        }
    }

    /// This capsule as a parry shape plus the isometry that places it: parry's
    /// `Capsule::new_y` is centred on the origin with its segment ±half_height
    /// along Y, so the placement is the segment midpoint (`base + h/2·Y`).
    pub(super) fn to_parry(self) -> (parry3d::shape::Capsule, Isometry) {
        let r = self.radius.min(self.height * 0.5);
        let half = (self.height - 2.0 * r) * 0.5;
        (
            parry3d::shape::Capsule::new_y(half.max(0.0), r),
            Isometry::translation(self.base.x, self.base.y + self.height * 0.5, self.base.z),
        )
    }
}

/// A rigid `Mat4` as a parry isometry. See the module header for why the
/// instance matrices may be treated as rigid.
pub(super) fn isometry_of(m: Mat4) -> Isometry {
    let b = Mat3::from_mat4(m);
    let rot = Quat::from_mat3(&Mat3::from_cols(
        b.x_axis.normalize_or(Vec3::X),
        b.y_axis.normalize_or(Vec3::Y),
        b.z_axis.normalize_or(Vec3::Z),
    ))
    .normalize();
    let t = m.w_axis.truncate();
    Isometry::from_parts(
        parry3d::na::Translation3::new(t.x, t.y, t.z),
        parry3d::na::UnitQuaternion::from_quaternion(parry3d::na::Quaternion::new(
            rot.w, rot.x, rot.y, rot.z,
        )),
    )
}

/// Axis-aligned box, used as the world's broad phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn empty() -> Aabb {
        Aabb {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        }
    }

    pub fn add(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
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

    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
            && self.min.z <= o.max.z
            && self.max.z >= o.min.z
    }

    /// Does the segment `from -> from + dir*len` reach this box? Slab test,
    /// broad phase only — a conservative yes.
    pub fn hits_ray(&self, from: Vec3, dir: Vec3, len: f32) -> bool {
        let (mut t0, mut t1) = (0.0f32, len);
        for a in 0..3 {
            let (o, d) = (from[a], dir[a]);
            if d.abs() < 1e-6 {
                if o < self.min[a] || o > self.max[a] {
                    return false;
                }
                continue;
            }
            let (mut n, mut f) = ((self.min[a] - o) / d, (self.max[a] - o) / d);
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

/// One body's geometry, in its own (model) space. Meshes are built once per
/// geometry and shared by `Arc` across every instance of it.
#[derive(Clone)]
pub enum Shape {
    Mesh(Arc<TriMesh>),
    Sphere {
        radius: f32,
    },
    /// Y-aligned, centred on the origin — parry's convention and the PS2's.
    Cylinder {
        radius: f32,
        half_height: f32,
    },
    Box {
        half: Vec3,
    },
}

impl Shape {
    fn local_aabb(&self) -> Aabb {
        match self {
            Shape::Mesh(m) => {
                let a = m.local_aabb();
                Aabb {
                    min: Vec3::new(a.mins.x, a.mins.y, a.mins.z),
                    max: Vec3::new(a.maxs.x, a.maxs.y, a.maxs.z),
                }
            }
            Shape::Sphere { radius } => Aabb {
                min: Vec3::splat(-radius),
                max: Vec3::splat(*radius),
            },
            Shape::Cylinder {
                radius,
                half_height,
            } => Aabb {
                min: Vec3::new(-radius, -half_height, -radius),
                max: Vec3::new(*radius, *half_height, *radius),
            },
            Shape::Box { half } => Aabb {
                min: -*half,
                max: *half,
            },
        }
    }

    /// Run `f` with this shape as a parry `Shape` trait object.
    pub(super) fn with_parry<R>(&self, f: &mut dyn FnMut(&dyn parry3d::shape::Shape) -> R) -> R {
        match self {
            Shape::Mesh(m) => f(m.as_ref()),
            Shape::Sphere { radius } => f(&parry3d::shape::Ball::new(*radius)),
            Shape::Cylinder {
                radius,
                half_height,
            } => f(&parry3d::shape::Cylinder::new(*half_height, *radius)),
            Shape::Box { half } => f(&parry3d::shape::Cuboid::new(vector(*half))),
        }
    }

    pub(crate) fn ray_distance(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
        let ray = parry3d::query::Ray::new(super::point(origin), vector(dir));
        self.cast_local(&ray, max_t).map(|hit| hit.time_of_impact)
    }

    /// Ray cast in SHAPE space. The ray parameter is preserved by the affine
    /// world→shape map, so the returned `time_of_impact` is a world distance.
    pub(super) fn cast_local(
        &self,
        ray: &parry3d::query::Ray,
        max_len: f32,
    ) -> Option<parry3d::query::RayIntersection> {
        use parry3d::query::RayCast;
        match self {
            Shape::Mesh(m) => mesh_ray::cast(m, ray, max_len),
            Shape::Sphere { radius } => parry3d::shape::Ball::new(*radius)
                .cast_local_ray_and_get_normal(ray, max_len, false),
            Shape::Cylinder {
                radius,
                half_height,
            } => ray_cylinder(ray, *half_height, *radius, max_len),
            Shape::Box { half } => parry3d::shape::Cuboid::new(vector(*half))
                .cast_local_ray_and_get_normal(ray, max_len, false),
        }
    }
}

/// A ray against a capped cylinder about its own Y axis, in closed form: the
/// overlap of the ray's stretch between the two cap planes with its stretch
/// inside the infinite cylinder. parry casts a ray at a `Cylinder` through
/// its support map (GJK, an iteration per query), and every ground probe
/// over a round deck pays for it. NOT solid, as parry's with `solid = false`:
/// from inside, the hit is the surface the ray leaves through, with the
/// normal facing back in.
fn ray_cylinder(
    ray: &parry3d::query::Ray,
    half_height: f32,
    radius: f32,
    max_len: f32,
) -> Option<parry3d::query::RayIntersection> {
    use parry3d::na::Vector3;
    let (o, d) = (ray.origin, ray.dir);
    // The caps' slab.
    let (mut y0, mut y1) = (f32::NEG_INFINITY, f32::INFINITY);
    if d.y.abs() < 1e-9 {
        if o.y.abs() > half_height {
            return None;
        }
    } else {
        y0 = (-half_height - o.y) / d.y;
        y1 = (half_height - o.y) / d.y;
        if y0 > y1 {
            std::mem::swap(&mut y0, &mut y1);
        }
    }
    // The side.
    let (mut s0, mut s1) = (f32::NEG_INFINITY, f32::INFINITY);
    let a = d.x * d.x + d.z * d.z;
    let c = o.x * o.x + o.z * o.z - radius * radius;
    if a < 1e-12 {
        if c > 0.0 {
            return None;
        }
    } else {
        let b = o.x * d.x + o.z * d.z;
        let disc = b * b - a * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        s0 = (-b - sq) / a;
        s1 = (-b + sq) / a;
    }
    let (t_in, t_out) = (y0.max(s0), y1.min(s1));
    if t_in > t_out || t_out < 0.0 {
        return None;
    }
    // The outward normal where the ray crosses the surface at `t`: the side's
    // when the side is what it crosses there.
    let normal = |t: f32, side: bool| {
        if side {
            let p = o + d * t;
            Vector3::new(p.x, 0.0, p.z) / radius
        } else {
            Vector3::new(0.0, (o.y + d.y * t).signum(), 0.0)
        }
    };
    let (t, n) = if t_in > 0.0 {
        (t_in, normal(t_in, s0 >= y0))
    } else {
        (t_out, -normal(t_out, s1 <= y1))
    };
    (t <= max_len)
        .then(|| parry3d::query::RayIntersection::new(t, n, parry3d::shape::FeatureId::Unknown))
}

/// Build a shared collision [`TriMesh`] from a baked triangle soup, or `None`
/// for degenerate input (fewer than 3 indices, no vertices, or a mesh parry
/// rejects). Keeps `parry3d` an implementation detail of this crate.
pub fn trimesh(positions: &[[f32; 3]], indices: &[u32]) -> Option<Arc<TriMesh>> {
    if positions.is_empty() || indices.len() < 3 {
        return None;
    }
    let points: Vec<parry3d::na::Point3<f32>> = positions
        .iter()
        .map(|p| parry3d::na::Point3::new(p[0], p[1], p[2]))
        .collect();
    let tris: Vec<[u32; 3]> = indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|t| t.iter().all(|&i| (i as usize) < positions.len()))
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    if tris.is_empty() {
        return None;
    }
    TriMesh::new(points, tris).ok().map(Arc::new)
}

/// Which QUERIES a body answers — the port's two collision layers so far
/// (`analysis/re-collision-layers.md` §4: a soldier's walk / ground / wall
/// queries use the model's SOLDIER subset, a bolt / reticle / LOS ray its
/// ORDNANCE subset). Bit 2 is RESERVED for the vehicle subset (P7) and unused.
///
/// The bit VALUES are ours, not the PS2's layer ids (RE §5 tabulates those:
/// BF1 soldier 2 / ordnance 0x200, BF2 soldier 2 / ordnance 0x400) — this is a
/// per-body membership mask, not the engine's per-model subset bitmap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layers(pub u8);

impl Layers {
    /// In no subset — a body the world does not hold at all.
    pub const NONE: Layers = Layers(0);
    /// The soldier subset: `ray_down`, `push_out`, `wall_sweep`.
    pub const SOLDIER: Layers = Layers(1);
    /// The ordnance subset: bolts, the reticle, AI line of sight.
    pub const ORDNANCE: Layers = Layers(2);
    /// The vehicle subset: what a flyer's contact and its pilot's rays test.
    pub const VEHICLE: Layers = Layers(4);
    /// The two layers that exist, and what `Body::new` gives a body. The PS2
    /// acquisition ray tests both (flag 0, RE §4); the port currently uses
    /// ORDNANCE through `perception::los_seat` (module header discrepancy).
    pub const ALL: Layers = Layers(1 | 2);

    /// Does this body answer a query for `q`? Any bit in common is enough.
    pub fn contains(self, q: Layers) -> bool {
        self.0 & q.0 != 0
    }
}

impl std::ops::BitOr for Layers {
    type Output = Layers;
    fn bitor(self, rhs: Layers) -> Layers {
        Layers(self.0 | rhs.0)
    }
}

/// One placed piece of world geometry.
pub struct Body {
    /// World-space bounds — the broad phase.
    pub aabb: Aabb,
    /// World → shape space.
    pub to_local: Mat4,
    /// Shape space → world.
    pub to_world: Mat4,
    pub shape: Shape,
    /// `to_world` as a rigid isometry, for parry's `contact`.
    pub(super) iso: Isometry,
    /// Which queries this body answers. See [`Layers`].
    pub layers: Layers,
    /// The vehicle whose hull this body is (card VHULL), `None` for the
    /// map's own bodies. A query may skip an owner's bodies
    /// ([`super::HullFilter`]) and a ray reports the owner it hit.
    pub owner: Option<VehicleHandle>,
    /// The map instance this body is of (`game::assets::Scene`'s body
    /// order), `None` for a hull or a fixture's body. A building turret's
    /// camera ray skips its own building's (card TURR,
    /// [`super::CollisionWorld::ray_past`]).
    pub instance: Option<u32>,
    /// Card VMESH: a mesh whose every edge is shared by exactly two of its
    /// triangles (vertices welded at a millimetre) - a WATERTIGHT solid
    /// such as BF1 Endor's cp2 boulder, which nobody can walk into. Rooms,
    /// tunnels, caves and the hollow trunks all have open edges. `false`
    /// for primitives.
    pub closed: bool,
    /// A hull body's damage factor for a direct hit (its class's
    /// `HitLocation` for this prim: the vehicles' x4 critical prims, BF1's
    /// two x0 ones; issue #41, `analysis/re-ai-difficulty.md` 9.2); 1.0 for
    /// every other body.
    pub hit_mult: f32,
}

impl Body {
    /// Native blade query: primitive bounds expand by width and the ray
    /// moves back by width without extending its length (B2 0016a710).
    /// Mesh primitives take the unexpanded ray (00160d60 -> 00166328).
    pub fn blade_contact(&self, start: Vec3, end: Vec3, radius: f32) -> bool {
        self.blade_contact_at(start, end, radius).is_some()
    }

    /// The contact's distance from `start` along the blade, as
    /// [`crate::unit::BoltPrims::melee_contact`] measures it.
    pub fn blade_contact_at(&self, start: Vec3, end: Vec3, radius: f32) -> Option<f32> {
        let delta = end - start;
        let length = delta.length();
        if !length.is_finite() || length <= 0.0 || !radius.is_finite() || radius < 0.0 {
            return None;
        }
        let dir = self.to_local.transform_vector3(delta / length);
        let mut from = self.to_local.transform_point3(start);
        if let Shape::Mesh(_) = &self.shape {
            let ray = parry3d::query::Ray::new(super::point(from), vector(dir));
            return self
                .shape
                .cast_local(&ray, length)
                .map(|hit| hit.time_of_impact)
                .filter(|&t| t < length);
        }
        from -= dir * radius;
        let contact = match &self.shape {
            Shape::Sphere { radius: r } => {
                crate::unit::ray_sphere(from, dir, Vec3::ZERO, r + radius)
            }
            Shape::Cylinder {
                radius: r,
                half_height,
            } => crate::unit::ray_upright_cylinder(
                from,
                dir,
                Vec3::ZERO,
                r + radius,
                half_height + radius,
            ),
            Shape::Box { half } => crate::unit::ray_yawed_box(
                from,
                dir,
                Vec3::ZERO,
                Vec3::X,
                Vec3::Z,
                *half + Vec3::splat(radius),
            ),
            Shape::Mesh(_) => unreachable!(),
        };
        contact.filter(|&t| t == 0.0 || t < length)
    }

    /// A body in EVERY layer — the PS2's default for a body no collision
    /// property names (RE §0.3), and the semantics every fixture in this
    /// module was written under.
    pub fn new(to_world: Mat4, shape: Shape) -> Body {
        Body::with_layers(to_world, shape, Layers::ALL | Layers::VEHICLE)
    }

    /// A body in exactly `layers` — what the runtime builds
    /// (`game::assets::Scene::collision_world`) once the class rule has said
    /// which subsets the body is in.
    pub fn with_layers(to_world: Mat4, shape: Shape, layers: Layers) -> Body {
        let closed = shape.closed();
        Body::placed(to_world, shape, layers, closed)
    }

    /// [`Body::with_layers`] with `closed` already known (card HULLC): a
    /// mover or a hull part re-placed every tick keeps its shape, so the
    /// mesh weld is not run again.
    pub(super) fn placed(to_world: Mat4, shape: Shape, layers: Layers, closed: bool) -> Body {
        Body {
            aabb: Aabb::transformed(shape.local_aabb(), to_world),
            to_local: to_world.inverse(),
            iso: isometry_of(to_world),
            to_world,
            closed,
            shape,
            layers,
            owner: None,
            instance: None,
            hit_mult: 1.0,
        }
    }
}

impl Shape {
    /// [`Body::closed`] for this shape: a watertight mesh; never a primitive.
    fn closed(&self) -> bool {
        match self {
            Shape::Mesh(m) => watertight(m),
            _ => false,
        }
    }
}

/// Every edge of `m` shared by exactly two triangles, its vertices welded at
/// a millimetre (the bake repeats a vertex per face); a mesh with no
/// triangles is not.
fn watertight(m: &TriMesh) -> bool {
    use std::collections::HashMap;
    let mut ids: HashMap<[i64; 3], u32> = HashMap::new();
    let vid: Vec<u32> = m
        .vertices()
        .iter()
        .map(|p| {
            let k = [p.x, p.y, p.z].map(|c| (f64::from(c) * 1000.0).round() as i64);
            let n = ids.len() as u32;
            *ids.entry(k).or_insert(n)
        })
        .collect();
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    for t in m.indices() {
        let v = t.map(|i| vid[i as usize]);
        for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            if a != b {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
    }
    !edges.is_empty() && edges.values().all(|&n| n == 2)
}

impl Body {
    /// Card OBSV: the body's extent on the ground in the frame `origin` +
    /// `axes` (XZ, unit, at right angles) — the least and the greatest
    /// coordinate of its shape's own box along each axis. Exact for a box;
    /// the box about a cylinder, a sphere and a mesh.
    pub fn ground_extent(
        &self,
        origin: glam::Vec2,
        axes: [glam::Vec2; 2],
    ) -> (glam::Vec2, glam::Vec2) {
        let a = self.shape.local_aabb();
        let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { a.min.x } else { a.max.x },
                if k & 2 == 0 { a.min.y } else { a.max.y },
                if k & 4 == 0 { a.min.z } else { a.max.z },
            );
            let w = self.to_world.transform_point3(c);
            let d = glam::Vec2::new(w.x, w.z) - origin;
            let q = glam::Vec2::new(d.dot(axes[0]), d.dot(axes[1]));
            lo = lo.min(q);
            hi = hi.max(q);
        }
        (lo, hi)
    }
}

/// One body of a vehicle CLASS's hull (card VHULL), in the model's space:
/// its shape, where it sits in the model, and the subsets the class's
/// collision lists put it in. `CollisionWorld::set_hull` places a live
/// vehicle's parts at its transform.
#[derive(Clone)]
pub struct HullPart {
    /// Shape space → model space.
    pub local: Mat4,
    pub shape: Shape,
    pub layers: Layers,
    /// [`Body::closed`] of `shape`, worked out once by [`HullPart::new`]
    /// (card HULLC: `set_hull` places every part every tick).
    pub(super) closed: bool,
    /// [`Body::hit_mult`] of the body placed from this part.
    pub hit_mult: f32,
}

impl HullPart {
    pub fn new(local: Mat4, shape: Shape, layers: Layers) -> HullPart {
        HullPart {
            local,
            closed: shape.closed(),
            shape,
            layers,
            hit_mult: 1.0,
        }
    }

    /// This part with a direct hit's damage factor (see [`Body::hit_mult`]).
    pub fn with_hit_mult(self, hit_mult: f32) -> HullPart {
        HullPart { hit_mult, ..self }
    }

    /// The same part: the same placement bit for bit, layers and shape (a
    /// mesh by identity - the hull holding it keeps it alive, so its address
    /// cannot be reused).
    pub(super) fn same_as(&self, o: &HullPart) -> bool {
        let bits = |m: &Mat4| m.to_cols_array().map(f32::to_bits);
        bits(&self.local) == bits(&o.local)
            && self.layers == o.layers
            && self.hit_mult.to_bits() == o.hit_mult.to_bits()
            && match (&self.shape, &o.shape) {
                (Shape::Mesh(a), Shape::Mesh(b)) => Arc::ptr_eq(a, b),
                (Shape::Sphere { radius: a }, Shape::Sphere { radius: b }) => {
                    a.to_bits() == b.to_bits()
                }
                (
                    Shape::Cylinder {
                        radius: a,
                        half_height: c,
                    },
                    Shape::Cylinder {
                        radius: b,
                        half_height: d,
                    },
                ) => a.to_bits() == b.to_bits() && c.to_bits() == d.to_bits(),
                (Shape::Box { half: a }, Shape::Box { half: b }) => {
                    a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)
                }
                _ => false,
            }
    }
}

/// A terrain cutter: a convex volume the terrain is punched out of. See
/// `mungefs::terrain::Cutter`.
#[derive(Default)]
pub struct Cutter {
    /// `(radius, cx, cy, cz)` bounding sphere from the `CUTR INFO` — a cheap
    /// reject before the planes. All zeroes = no bound (test the planes).
    pub sphere: [f32; 4],
    pub planes: Vec<Vec4>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::tests::{box_mesh, floor_quad, mesh_body, slab};
    use crate::collision::CollisionWorld;

    /// Card HULLC: a hull part's and a mover's `closed` is worked out once,
    /// where the shape is made, and is what [`watertight`] says of the same
    /// mesh - on the part, on the hull bodies `set_hull` places every tick,
    /// and on a body `move_body` re-places.
    #[test]
    fn closed_is_computed_once_and_matches_watertight() {
        let owner = crate::vehicle::VehicleHandle { index: 0, gen: 0 };
        let half = Vec3::new(1.0, 1.5, 2.0);
        let shapes = [false, true].map(|open| box_mesh(Vec3::ZERO, half, open).shape);
        let want: Vec<bool> = shapes
            .iter()
            .map(|s| match s {
                Shape::Mesh(m) => watertight(m),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(want, [true, false], "the fixture: one sealed box, one open");
        let parts: Vec<HullPart> = shapes
            .iter()
            .map(|s| HullPart::new(Mat4::from_translation(Vec3::Y), s.clone(), Layers::ALL))
            .chain([HullPart::new(
                Mat4::IDENTITY,
                Shape::Box { half },
                Layers::ALL,
            )])
            .collect();
        let want = [want[0], want[1], false];
        assert_eq!(parts.iter().map(|p| p.closed).collect::<Vec<_>>(), want);
        let mut w = CollisionWorld::new(
            shapes
                .iter()
                .map(|s| Body::new(Mat4::IDENTITY, s.clone()))
                .collect(),
            Vec::new(),
        );
        for pose in [
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(5.0, 0.0, -3.0)),
        ] {
            w.set_hull(owner, &parts, pose);
            let got: Vec<bool> = w.hull(owner).iter().map(|b| b.closed).collect();
            assert_eq!(got, want, "set_hull at {pose:?}");
        }
        for (i, &closed) in want.iter().take(2).enumerate() {
            w.move_body(i, Mat4::from_translation(Vec3::X * 3.0));
            assert_eq!(w.bodies()[i].closed, closed, "move_body {i}");
        }
    }

    /// Card HULLC: `set_hull` keeps a hull placed by the same vehicle from
    /// the same parts at the same pose, and re-places it on any change - a
    /// new pose, a part's layers, an equal mesh that is another `Arc`, a new
    /// generation in the slot.
    #[test]
    fn set_hull_keeps_only_an_unchanged_hull() {
        let owner = crate::vehicle::VehicleHandle { index: 2, gen: 0 };
        let half = Vec3::new(1.0, 1.5, 2.0);
        let mesh = box_mesh(Vec3::ZERO, half, false).shape;
        let part = |shape: Shape, layers| HullPart::new(Mat4::IDENTITY, shape, layers);
        let parts = vec![part(mesh.clone(), Layers::ALL)];
        let mut w = CollisionWorld::new(Vec::new(), Vec::new());
        let at = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        let probe = |w: &CollisionWorld, o| {
            w.hull(o)
                .iter()
                .map(|b| (b.aabb.min.x, b.layers, b.closed))
                .collect::<Vec<_>>()
        };
        w.set_hull(owner, &parts, at(0.0));
        assert_eq!(probe(&w, owner), [(-1.0, Layers::ALL, true)]);
        w.set_hull(owner, &parts, at(0.0));
        assert_eq!(probe(&w, owner), [(-1.0, Layers::ALL, true)], "kept");
        w.set_hull(owner, &parts, at(4.0));
        assert_eq!(probe(&w, owner), [(3.0, Layers::ALL, true)], "a new pose");
        let ordnance = vec![part(mesh.clone(), Layers::ORDNANCE)];
        w.set_hull(owner, &ordnance, at(4.0));
        assert_eq!(probe(&w, owner), [(3.0, Layers::ORDNANCE, true)], "layers");
        let open = vec![part(
            box_mesh(Vec3::ZERO, half, true).shape,
            Layers::ORDNANCE,
        )];
        w.set_hull(owner, &open, at(4.0));
        assert_eq!(
            probe(&w, owner),
            [(3.0, Layers::ORDNANCE, false)],
            "another mesh"
        );
        let next = crate::vehicle::VehicleHandle { index: 2, gen: 1 };
        w.set_hull(next, &open, at(4.0));
        assert!(
            w.hull(owner).is_empty(),
            "the old generation's hull is gone"
        );
        assert_eq!(
            probe(&w, next),
            [(3.0, Layers::ORDNANCE, false)],
            "a new generation"
        );
        // An equal mesh in a new `Arc` is another mesh: placed again (its
        // `closed` would be the same; the hull now holds the new `Arc`).
        let Shape::Mesh(m) = &open[0].shape else {
            unreachable!()
        };
        let copy = vec![part(Shape::Mesh(Arc::new((**m).clone())), Layers::ORDNANCE)];
        w.set_hull(next, &copy, at(4.0));
        let Shape::Mesh(held) = &w.hull(next)[0].shape else {
            unreachable!()
        };
        let Shape::Mesh(c) = &copy[0].shape else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(held, c), "an equal mesh in a new Arc");
        w.set_hull(next, &[], at(4.0));
        assert!(w.hull(next).is_empty(), "no part, no hull");
    }

    /// [`Capsule::stepped`] is exactly the capsule the AI's forward probe used
    /// to build for itself (`ai::probe_capsule`, deleted in A.4 when the wall
    /// pass started asking for the same shape): lift = `STEP_HEIGHT` capped at
    /// half the height, height down by the same, radius and top unmoved.
    #[test]
    fn stepped_capsule_matches_the_probe() {
        for (h, lift) in [(1.9f32, 0.6f32), (1.6, 0.6), (0.55, 0.275)] {
            let c = Capsule {
                base: Vec3::new(1.0, 2.0, 3.0),
                height: h,
                radius: 0.5,
            };
            let s = c.stepped();
            assert_eq!(s.base, c.base + Vec3::Y * lift, "height {h}");
            assert!((s.height - (h - lift)).abs() < 1e-6, "height {h}: {s:?}");
            assert_eq!(s.radius, c.radius);
            assert!(
                (s.base.y + s.height - (c.base.y + c.height)).abs() < 1e-6,
                "the top does not move: {s:?}"
            );
            // The lift is exactly what `probe_capsule` computed.
            assert!((lift - STEP_HEIGHT.min(h * 0.5)).abs() < 1e-6);
        }
    }

    /// The closed-form cylinder ray answers as parry's support-map cast does:
    /// rays at points inside the cylinder from outside it (a cap or the side
    /// is met, the normal faces out), rays from inside (the surface left
    /// through, the normal facing back in), rays that pass it by, and the
    /// reach.
    #[test]
    fn the_closed_form_cylinder_ray_is_parrys() {
        use crate::collision::point;
        use parry3d::query::{Ray, RayCast};
        let (hh, r) = (1.5f32, 4.0f32);
        let cyl = parry3d::shape::Cylinder::new(hh, r);
        let seed = std::cell::Cell::new(0x1234_5678u32);
        let rnd = || {
            seed.set(
                seed.get()
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223),
            );
            (seed.get() >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        // A point inside the cylinder, `k` of the way out to its surface.
        let inside = |k: f32| {
            let a = rnd() * std::f32::consts::PI;
            let rad = (rnd() * 0.5 + 0.5) * r * k;
            let (sa, ca) = crate::fmath::sin_cos(a);
            point(Vec3::new(ca * rad, rnd() * hh * k, sa * rad))
        };
        let (mut out_hits, mut in_hits, mut caps, mut sides, mut loose) = (0, 0, 0, 0, 0);
        for i in 0..4000 {
            let target = inside(0.9);
            // From outside (far or near, any side) or from inside.
            let from_in = i % 4 == 3;
            let o = if from_in {
                inside(0.9)
            } else {
                let mut d = vector(Vec3::new(rnd(), rnd(), rnd()));
                if d.norm() < 0.1 {
                    d.x = 1.0;
                }
                target + d.normalize() * (2.0 * r + hh + 10.0 * (rnd() + 1.0))
            };
            let d = target - o;
            if d.norm() < 0.2 {
                continue;
            }
            let ray = Ray::new(o, d.normalize());
            let want = cyl
                .cast_local_ray_and_get_normal(&ray, 100.0, false)
                .expect("parry: a ray at the cylinder's inside meets it");
            let got = ray_cylinder(&ray, hh, r, 100.0).expect("a ray at its inside meets it");
            let (wt, gt) = (want.time_of_impact, got.time_of_impact);
            // (parry's iteration stops a few mm early on a grazing hit at
            // the rim; the closed form's point is on the surface, below.)
            assert!((wt - gt).abs() < 2e-2, "{i}: t {gt} for {wt} ({ray:?})");
            loose += u32::from((wt - gt).abs() > 2e-3);
            let hit = ray.point_at(gt);
            let rad = (hit.x * hit.x + hit.z * hit.z).sqrt();
            let on_side = (rad - r).abs() < 1e-3;
            let on_cap = (hit.y.abs() - hh).abs() < 1e-3;
            assert!(on_side || on_cap, "{i}: {hit:?} is not on the surface");
            // Off the rim, where two faces meet (and parry's normal is a
            // blend of them), the normal is one face's.
            if rad < r - 0.05 || hit.y.abs() < hh - 0.05 {
                assert!(
                    got.normal.dot(&want.normal) > 0.999,
                    "{i}: normal {:?} for {:?}",
                    got.normal,
                    want.normal
                );
                // It faces the ray both ways: out of the cylinder from
                // outside, back into it from inside.
                assert!(got.normal.dot(&ray.dir) < 0.0, "{i}: facing");
                let out = got.normal.dot(&hit.coords) > 0.0;
                assert_eq!(out, !from_in, "{i}: outward");
                caps += u32::from(on_cap);
                sides += u32::from(on_side);
            }
            if from_in {
                in_hits += 1;
            } else {
                out_hits += 1;
            }
            // Short of the surface the ray has no hit.
            assert!(ray_cylinder(&ray, hh, r, gt - 0.01).is_none(), "{i}: reach");
            // The same ray turned round from outside goes away from it.
            if !from_in {
                let away = Ray::new(o, -ray.dir);
                assert!(ray_cylinder(&away, hh, r, 100.0).is_none(), "{i}: away");
                assert!(cyl
                    .cast_local_ray_and_get_normal(&away, 100.0, false)
                    .is_none());
            }
        }
        assert!(out_hits > 2000 && in_hits > 800, "{out_hits} {in_hits}");
        assert!(caps > 500 && sides > 500, "{caps} caps, {sides} sides");
        assert!(loose < 20, "{loose} hits over 2 mm from parry's");
        // Straight down beside it, and down its axis from above and inside.
        let down = |x: f32, y: f32| Ray::new(point(Vec3::new(x, y, 0.0)), vector(Vec3::NEG_Y));
        assert!(ray_cylinder(&down(r + 0.01, 10.0), hh, r, 100.0).is_none());
        let top = ray_cylinder(&down(1.0, 10.0), hh, r, 100.0).expect("the top");
        assert!((top.time_of_impact - (10.0 - hh)).abs() < 1e-6 && top.normal.y == 1.0);
        let under = ray_cylinder(&down(1.0, 0.0), hh, r, 100.0).expect("the underside");
        assert!((under.time_of_impact - hh).abs() < 1e-6 && under.normal.y == 1.0);
    }

    /// Card OBSV (the audit of card OBST2's landing): the closed-form
    /// cylinder ray against an ORACLE that is neither implementation — it
    /// knows only whether a point is in the cylinder, and walks the ray in
    /// 1 mm steps (f64) to the first point where that changes, then bisects.
    /// Rays that miss, that graze the side and the rim, that start inside,
    /// and short ones (a tick's move) — where the test above only asks rays
    /// aimed at the inside, and takes parry's word (2 cm off at a rim, and a
    /// miss or a false hit in 5 of 40,000 short rays, measured).
    #[test]
    fn the_closed_form_cylinder_ray_meets_what_an_inside_test_says() {
        use crate::collision::point;
        use parry3d::query::Ray;
        let seed = std::cell::Cell::new(0x0b57_2026u64);
        let unit = || {
            seed.set(
                seed.get()
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407),
            );
            (seed.get() >> 11) as f64 / (1u64 << 53) as f64
        };
        let signed = || unit() * 2.0 - 1.0;
        let inside =
            |p: [f64; 3], hh: f64, r: f64| p[1].abs() <= hh && p[0] * p[0] + p[2] * p[2] <= r * r;
        // The first t in [0, len] where the ray's point is on the other side
        // of the surface than its origin, and whether that stretch is under
        // 5 mm long (a graze the 1 mm walk may step over: not judged).
        let oracle = |o: [f64; 3], d: [f64; 3], len: f64, hh: f64, r: f64| {
            let at = |t: f64| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
            let from = inside(o, hh, r);
            let n = (len / 1e-3).ceil() as usize;
            let mut prev = 0.0;
            for i in 1..=n {
                let t = (i as f64 * 1e-3).min(len);
                if inside(at(t), hh, r) != from {
                    let (mut a, mut b) = (prev, t);
                    for _ in 0..40 {
                        let m = 0.5 * (a + b);
                        if inside(at(m), hh, r) != from {
                            b = m;
                        } else {
                            a = m;
                        }
                    }
                    let thin = (1..=5).any(|k| inside(at(b + f64::from(k) * 1e-3), hh, r) == from);
                    return (Some(b), thin);
                }
                prev = t;
            }
            (None, false)
        };
        // [rays, the oracle's hits, its misses, from inside, not judged]
        let mut seen = [[0u32; 5]; 5];
        for (kind, row) in seen.iter_mut().enumerate() {
            while row[0] < 1500 {
                let r = 0.2 + unit() * unit() * 8.0;
                let hh = 0.1 + unit() * unit() * 6.0;
                let ext = r.max(hh) * 1.5 + 2.0;
                let mut o = [signed() * ext, signed() * ext, signed() * ext];
                let mut d = [signed(), signed(), signed()];
                let mut len = 1.0 + unit() * 11.0;
                let turn = unit() * std::f64::consts::TAU;
                match kind {
                    // Straight down: a ground probe.
                    1 => {
                        d = [0.0, -1.0, 0.0];
                        o = [
                            signed() * (r + 0.5),
                            hh + signed() * 3.0,
                            signed() * (r + 0.5),
                        ];
                        len = 0.5 + unit() * 8.0;
                    }
                    // Level: a sweep's, a parity ray's.
                    2 => {
                        d = [turn.cos(), 0.0, turn.sin()];
                        o[1] = signed() * (hh + 0.5);
                    }
                    // Short, from within 0.3 m of the surface: a tick's move.
                    3 => {
                        len = 0.01 + unit() * 0.4;
                        o = [
                            turn.cos() * (r + signed() * 0.3),
                            signed() * (hh + 0.3),
                            turn.sin() * (r + signed() * 0.3),
                        ];
                    }
                    // A graze: level, passing the axis within 5 cm of the
                    // radius, inside it or outside.
                    4 => {
                        let off = r + signed() * 0.05;
                        d = [turn.cos(), 0.0, turn.sin()];
                        let back = r + 1.0 + unit() * 3.0;
                        o = [
                            -turn.sin() * off - d[0] * back,
                            signed() * hh * 0.9,
                            turn.cos() * off - d[2] * back,
                        ];
                        len = 2.0 * back + 2.0;
                    }
                    // Any ray, at a point in or just outside the cylinder.
                    _ => {
                        let to = [signed() * r * 1.3, signed() * hh * 1.3, signed() * r * 1.3];
                        // (One in four from a point in it.)
                        if row[0] % 4 == 3 {
                            let k = 0.7 * unit();
                            o = [turn.cos() * r * k, signed() * hh * 0.9, turn.sin() * r * k];
                        }
                        d = [to[0] - o[0], to[1] - o[1], to[2] - o[2]];
                    }
                }
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if l < 0.1 {
                    continue;
                }
                d = [d[0] / l, d[1] / l, d[2] / l];
                // (The ray the f32 code sees is the one the oracle walks.)
                let o32 = Vec3::new(o[0] as f32, o[1] as f32, o[2] as f32);
                let d32 = Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32);
                let (o, d) = (
                    [f64::from(o32.x), f64::from(o32.y), f64::from(o32.z)],
                    [f64::from(d32.x), f64::from(d32.y), f64::from(d32.z)],
                );
                let (hh32, r32, len32) = (hh as f32, r as f32, len as f32);
                let (hh, r, len) = (f64::from(hh32), f64::from(r32), f64::from(len32));
                let (want, thin) = oracle(o, d, len, hh, r);
                row[0] += 1;
                let ray = Ray::new(point(o32), vector(d32));
                let got = ray_cylinder(&ray, hh32, r32, len32);
                // A graze under 5 mm, and a hit within 2 mm of the ray's
                // end, are the walk's to miss: counted, not judged.
                let at_end = |t: f64| (t - len).abs() < 2e-3;
                if thin
                    || want.is_some_and(at_end)
                    || got.is_some_and(|g| at_end(f64::from(g.time_of_impact)))
                {
                    row[4] += 1;
                    continue;
                }
                let what = format!("kind {kind} o {o:?} d {d:?} len {len} hh {hh} r {r}");
                match (want, got) {
                    (None, None) => row[2] += 1,
                    (Some(t), Some(g)) => {
                        let gt = f64::from(g.time_of_impact);
                        assert!((gt - t).abs() < 2e-3, "t {gt} for {t}: {what}");
                        // The normal faces the ray, and is the surface's
                        // there: the side's is level and radial, a cap's is
                        // +-Y.
                        let n = Vec3::new(g.normal.x, g.normal.y, g.normal.z);
                        assert!(n.dot(d32) <= 1e-4, "the normal {n} faces away: {what}");
                        assert!((n.length() - 1.0).abs() < 1e-3, "{n}: {what}");
                        let hit = o32 + d32 * g.time_of_impact;
                        let rad = (hit.x * hit.x + hit.z * hit.z).sqrt();
                        if n.y == 0.0 {
                            assert!((rad - r32).abs() < 2e-3, "side: {hit} {what}");
                            let out = Vec3::new(hit.x, 0.0, hit.z) / rad;
                            assert!(n.dot(out).abs() > 0.999, "radial: {n} {what}");
                        } else {
                            assert!(n.y.abs() == 1.0, "a cap's normal: {n} {what}");
                            assert!((hit.y.abs() - hh32).abs() < 2e-3, "cap: {hit} {what}");
                        }
                        row[1] += 1;
                        row[3] += u32::from(inside(o, hh, r));
                    }
                    (w, g) => panic!(
                        "the oracle {w:?}, the ray {:?}: {what}",
                        g.map(|g| g.time_of_impact)
                    ),
                }
            }
        }
        // Every kind has hits and misses to judge, and starts inside.
        for (kind, row) in seen.iter().enumerate() {
            eprintln!("kind {kind}: {row:?}");
            assert!(row[1] >= 150 && row[2] >= 150, "kind {kind}: {row:?}");
            assert!(
                row[3] >= 20 || kind == 4,
                "kind {kind}: {row:?} from inside"
            );
            assert!(row[4] * 4 < row[0], "kind {kind}: {row:?} not judged");
        }
    }

    /// [`slab`], in exactly `layers` instead of every layer.
    fn slab_in(z: f32, layers: Layers) -> Body {
        Body::with_layers(
            Mat4::from_translation(Vec3::new(0.0, 0.0, z)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
            layers,
        )
    }

    /// A capsule fixture for the layer tests: 1.9 m tall, 0.5 m radius.
    fn cap_at(p: Vec3) -> Capsule {
        Capsule {
            base: p,
            height: 1.9,
            radius: 0.5,
        }
    }

    /// A world of one slab and one floor mesh, both in exactly `layers` — the
    /// runtime's ordnance-only / soldier-only bodies in miniature.
    fn masked_world(layers: Layers) -> CollisionWorld {
        let mut floor = mesh_body(&floor_quad(-5.0, 10.0));
        floor.layers = layers;
        CollisionWorld::new(vec![slab_in(-20.0, layers), floor], Vec::new())
    }

    /// The BF2 `p_-o-…` prims and BF1 HOT1's `p_ordinance*`
    /// (analysis/re-collision-layers.md §2, §4): in the model's ORDNANCE
    /// subset and NOT its soldier one, so a bolt and a line of sight stop at
    /// them while a soldier walks through — every soldier query must be blind
    /// to them.
    #[test]
    fn an_ordnance_only_body_is_invisible_to_the_soldier_and_hit_by_the_bolt() {
        let world = masked_world(Layers::ORDNANCE);
        // Inside the slab: no push at all.
        assert_eq!(
            world.push_out(&cap_at(Vec3::new(0.0, 0.0, -19.7))),
            Vec3::ZERO
        );
        // Walking straight into it: no wall.
        assert!(world
            .wall_sweep(&cap_at(Vec3::new(0.0, 0.0, -15.0)), Vec3::NEG_Z, 10.0)
            .is_none());
        // Standing over the floor: no ground.
        assert!(world.ray_down(Vec3::new(0.0, 10.0, 0.0), 20.0).is_none());
        // The bolt / LOS ray DOES see the slab, at its near face.
        let (t, _) = world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ORDNANCE)
            .expect("the ordnance ray hits it");
        assert!((t - 19.5).abs() < 1e-4, "{t}");
        // ... and the same ray asked for the soldier subset does not.
        assert!(world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::SOLDIER)
            .is_none());
    }

    /// The converse, and the bigger half of the change on the discs: BF2
    /// common's `com_inv_col_*` invisible walls and BF1 END1's ewok fences
    /// carry `OrdnanceCollision = none` (RE §2, §8), so they block a soldier
    /// and a bolt flies through them.
    #[test]
    fn a_soldier_only_body_blocks_the_soldier_and_not_the_bolt() {
        let world = masked_world(Layers::SOLDIER);
        let push = world.push_out(&cap_at(Vec3::new(0.0, 0.0, -19.7)));
        assert!(push.length() > 1e-4, "pushed out of the slab: {push}");
        assert_eq!(push.y, 0.0, "the push is horizontal");
        let (t, n) = world
            .wall_sweep(&cap_at(Vec3::new(0.0, 0.0, -15.0)), Vec3::NEG_Z, 10.0)
            .expect("the wall stops the soldier");
        assert!(
            (t - 4.0).abs() < 1e-3,
            "near face at 19.5 m, radius 0.5: {t}"
        );
        assert!((n - Vec3::Z).length() < 1e-3, "{n}");
        let (y, _) = world
            .ray_down(Vec3::new(0.0, 10.0, 0.0), 20.0)
            .expect("the floor is ground");
        assert!((y + 5.0).abs() < 1e-5, "{y}");
        // The bolt and the line of sight go straight through.
        assert!(world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ORDNANCE)
            .is_none());
        assert!(world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::SOLDIER)
            .is_some());
    }

    /// The tie rule of `ray_takes_the_nearest_body_and_the_first_on_an_exact_tie`
    /// runs over the MASKED bodies only: a body the mask excludes cannot win a
    /// tie it is not in. Same two shapes, same bit-exact `t = 16.0`.
    #[test]
    fn ray_ties_are_broken_among_the_masked_bodies_only() {
        let world = CollisionWorld::new(
            vec![
                // First, and soldier-only: it wins the tie for the soldier ray
                // and is invisible to the ordnance one.
                Body::with_layers(
                    Mat4::from_translation(Vec3::new(0.0, 3.0, -20.0)),
                    Shape::Sphere { radius: 5.0 },
                    Layers::SOLDIER,
                ),
                slab_in(-16.5, Layers::ORDNANCE),
            ],
            Vec::new(),
        );
        let (t, n) = world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ORDNANCE)
            .expect("hit");
        assert_eq!(t, 16.0, "the tie is exact");
        assert!(
            (n - Vec3::Z).length() < 1e-5,
            "the SECOND body wins: the first is not in the ordnance subset: {n}"
        );
        let (t, n) = world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::SOLDIER)
            .expect("hit");
        assert_eq!(t, 16.0);
        assert!(
            (n - Vec3::new(0.0, -0.6, 0.8)).length() < 1e-5,
            "the ball wins the soldier ray: {n}"
        );
    }

    /// `Body::new` is in EVERY layer — the PS2's default for a body no
    /// collision property names (RE §0.3), and the contract every fixture in
    /// this module is written under.
    #[test]
    fn body_new_is_in_every_layer() {
        assert_eq!(slab(0.0).layers, Layers::ALL | Layers::VEHICLE);
        assert!(Layers::ALL.contains(Layers::SOLDIER));
        assert!(Layers::ALL.contains(Layers::ORDNANCE));
        assert!(!Layers::NONE.contains(Layers::SOLDIER));
        assert!(!Layers::NONE.contains(Layers::ORDNANCE));
        assert!(slab(0.0).layers.contains(Layers::VEHICLE));
        assert!(!Layers::ALL.contains(Layers::VEHICLE));
        assert_eq!(Layers::SOLDIER | Layers::ORDNANCE, Layers::ALL);
    }

    #[test]
    fn ray_capsule_hits_body_and_caps_and_misses() {
        let c = Capsule {
            base: Vec3::new(0.0, 0.0, -20.0),
            height: 1.9,
            radius: 0.5,
        };
        // Straight at chest height along -Z from the origin: hit at 19.5 m.
        let t = c
            .ray_hit(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z, 100.0)
            .unwrap();
        assert!((t - 19.5).abs() < 1e-4, "{t}");
        // Grazing past the side by 0.6 m: miss.
        assert!(c
            .ray_hit(Vec3::new(0.6, 1.0, 0.0), Vec3::NEG_Z, 100.0)
            .is_none());
        // Top cap from above.
        let t = c
            .ray_hit(Vec3::new(0.0, 10.0, -20.0), Vec3::NEG_Y, 100.0)
            .unwrap();
        assert!((t - (10.0 - 1.9)).abs() < 1e-4, "{t}");
        // Beyond max_t: none.
        assert!(c
            .ray_hit(Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z, 10.0)
            .is_none());
        // Behind the ray: none.
        assert!(c
            .ray_hit(Vec3::new(0.0, 1.0, -40.0), Vec3::NEG_Z, 100.0)
            .is_none());
    }
    #[test]
    fn blade_primitive_width_moves_the_ray_and_preserves_native_length() {
        let body = Body::new(Mat4::IDENTITY, Shape::Sphere { radius: 1.0 });
        let start = Vec3::new(0.0, 0.0, -2.0);
        assert!(!body.blade_contact(start, start + Vec3::Z * 0.85, 0.25));
        assert!(body.blade_contact(start, start + Vec3::Z * 1.01, 0.25));
        assert!(body.blade_contact(Vec3::ZERO, Vec3::Z * 0.01, 0.25));
    }

    #[test]
    fn blade_queries_expand_box_corners_and_cylinder_flat_caps() {
        for shape in [
            Shape::Box { half: Vec3::ONE },
            Shape::Cylinder {
                radius: 1.0,
                half_height: 1.0,
            },
        ] {
            let matrix = Mat4::from_translation(Vec3::new(2.0, 3.0, -4.0))
                * Mat4::from_rotation_y(0.3)
                * Mat4::from_rotation_x(-0.5);
            let body = Body::new(matrix, shape);
            let a = matrix.transform_point3(Vec3::new(1.15, 1.15, -3.0));
            let b = matrix.transform_point3(Vec3::new(1.15, 1.15, 3.0));
            // The native expanded bounds include this square corner. A
            // rounded Minkowski capsule is too narrow at the diagonal.
            assert!(body.blade_contact(a, b, 0.2));
            let a = matrix.transform_point3(Vec3::new(1.21, 1.15, -3.0));
            let b = matrix.transform_point3(Vec3::new(1.21, 1.15, 3.0));
            assert!(!body.blade_contact(a, b, 0.2));
        }
    }

    #[test]
    fn blade_mesh_branch_ignores_width_as_the_native_mesh_vtable_does() {
        let mesh = trimesh(
            &[[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 2],
        )
        .unwrap();
        let body = Body::new(Mat4::IDENTITY, Shape::Mesh(mesh));
        assert!(body.blade_contact(Vec3::NEG_Z, Vec3::Z, 0.5));
        assert!(!body.blade_contact(Vec3::new(0.8, 0.0, -1.0), Vec3::new(0.8, 0.0, 1.0), 0.5));
    }
}
