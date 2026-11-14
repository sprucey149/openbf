//! The world's point and ray queries: is a point cut out of the terrain
//! ([`is_cut_by`], [`CollisionWorld::is_cut`]), the ground ray
//! ([`CollisionWorld::ray_down`]), the general world ray bolts and line of
//! sight take ([`CollisionWorld::ray`]), the marched terrain ray
//! ([`ray_heightfield`]) and the flat-fire lead solve ([`lead_point`]).

use super::shape::{Aabb, Body, Cutter, Layers};
use super::world::{CollisionWorld, HullFilter};
use super::{point, vector, WALKABLE_MIN_NY};
use crate::fmath::FloatExt;
use crate::heightfield::{HeightProbe, Heightfield};
use crate::vehicle::VehicleHandle;
use glam::Vec3;

/// PROVISIONAL (ours): how many faces of one body the down ray may pass
/// before it gives the body up, how far under a steep face it restarts, and
/// how far under the FIRST steep face a floor still counts — a bevel over a
/// deck, not whatever face lies deep inside a solid (its underside reads as
/// walkable to a two-sided ray).
const RAY_DOWN_PASSES: usize = 4;
const RAY_DOWN_SKIP: f32 = 0.01;
const RAY_DOWN_SEEK: f32 = 0.5;
/// How far past the best hit a body is still read (m), for the rounding between
/// a box in world space, its mesh's own boxes and a hit in the body's space.
const BOX_MARGIN: f32 = 0.01;

/// Is `p` inside any of `cutters`? A point is inside one iff EVERY plane
/// evaluates strictly < 0 (analysis/re-terrain-heights.md: the PS2 height query
/// returns −300.0 for such a point). Free function so the terrain ray can take
/// a bare cutter slice.
///
/// PROVISIONAL (ours): reading the `CUTR INFO` sphere as a CONSERVATIVE bound,
/// so a point outside it can skip the planes. The sphere itself is measured
/// (`mungefs::terrain::Cutter`); that it always encloses the plane volume is
/// our inference from the two pinned cutters, not something the PS2 code was
/// read for.
pub fn is_cut_by(cutters: &[Cutter], p: Vec3) -> bool {
    let q = p.extend(1.0);
    cutters.iter().any(|c| {
        if c.planes.is_empty() {
            return false;
        }
        // The `CUTR INFO` bounding sphere, when there is one: a conservative
        // bound, so outside it is a definite miss and the planes (up to 18 of
        // them) are never touched. r = 0 means the bake carried none.
        let r = c.sphere[0];
        if r > 0.0
            && (p - Vec3::new(c.sphere[1], c.sphere[2], c.sphere[3])).length_squared() > r * r
        {
            return false;
        }
        c.planes.iter().all(|pl| pl.dot(q) < 0.0)
    })
}

impl CollisionWorld {
    /// Is `p` inside any cutter? See [`is_cut_by`].
    pub fn is_cut(&self, p: Vec3) -> bool {
        is_cut_by(&self.cutters, p)
    }

    /// First (highest) WALKABLE surface strictly below `origin` along −Y within
    /// `max_len`: (hit y, world normal). Two-sided — PS2 collision winding does
    /// not tell facing — so a hit counts only if `|normal.y| >= WALKABLE_MIN_NY`.
    /// Bodies are visited in ascending index order (the broad phase offers
    /// the candidates in it) and ties go to the first body.
    ///
    /// A SOLDIER query: bodies outside [`Layers::SOLDIER`] are skipped (RE §4).
    /// Every vehicle hull is ground too (a soldier stands on a parked one).
    pub fn ray_down(&self, origin: Vec3, max_len: f32) -> Option<(f32, Vec3)> {
        self.ray_down_with(origin, max_len, HullFilter::All)
    }

    /// [`Self::ray_down`] seeing only the hulls `hulls` admits.
    pub fn ray_down_with(
        &self,
        origin: Vec3,
        max_len: f32,
        hulls: HullFilter,
    ) -> Option<(f32, Vec3)> {
        self.ray_down_owned(origin, max_len, hulls)
            .map(|(y, n, _)| (y, n))
    }

    /// [`Self::ray_down_with`] with the vehicle whose hull the hit is on
    /// (`None` for a map body).
    pub fn ray_down_owned(
        &self,
        origin: Vec3,
        max_len: f32,
        hulls: HullFilter,
    ) -> Option<(f32, Vec3, Option<VehicleHandle>)> {
        if max_len <= 0.0 {
            return None;
        }
        super::cost::query();
        let mut best: Option<(f32, Vec3, Option<VehicleHandle>)> = None;
        // The ray goes straight down, so its XZ footprint is one point.
        self.visit_xz(origin.x, origin.x, origin.z, origin.z, hulls, |body| {
            if !body.layers.contains(Layers::SOLDIER) {
                return;
            }
            // Only a hit nearer than the best can replace it, so look no
            // farther, less a margin: a hit a rounding nearer than the best
            // can lie in a box that rounds to just past it (BF2 Tatooine 3's
            // hub 54, one bit of a floor).
            let limit = best.map_or(max_len, |b| (b.0 + BOX_MARGIN).fmin(max_len));
            let a = &body.aabb;
            if origin.x < a.min.x
                || origin.x > a.max.x
                || origin.z < a.min.z
                || origin.z > a.max.z
                || !a.hits_ray(origin, Vec3::NEG_Y, limit)
            {
                return;
            }
            let d = body.to_local.transform_vector3(Vec3::NEG_Y);
            // A face too steep to stand on does not end the search in this
            // body: the ray goes on from just under it, so a floor below a
            // bevel is still found (KAM1's pad rims: a 61° bevel 0.18 m over
            // the deck sent the answer to the core 9.4 m down — card GRND,
            // re-kamino.md §4.3). Capped: a ray inside a wall's thickness
            // would otherwise step down it face by face.
            let mut from = 0.0f32;
            let mut reach = limit;
            for pass in 0..RAY_DOWN_PASSES {
                let o = body.to_local.transform_point3(origin - Vec3::Y * from);
                let ray = parry3d::query::Ray::new(point(o), vector(d));
                let Some(hit) = counted_cast(body, &ray, reach - from, origin) else {
                    return;
                };
                let t = from + hit.time_of_impact;
                if pass == 0 {
                    reach = reach.fmin(t + RAY_DOWN_SEEK);
                }
                let n = body
                    .to_world
                    .transform_vector3(Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z))
                    .normalize_or_zero();
                if n.y.abs() >= WALKABLE_MIN_NY {
                    if best.is_none_or(|(b, _, _)| t < b) {
                        best = Some((t, n, body.owner));
                    }
                    return;
                }
                from = t + RAY_DOWN_SKIP;
                if from >= reach {
                    return;
                }
            }
        });
        best.map(|(t, n, owner)| (origin.y - t, n, owner))
    }

    /// First hit along `dir` (UNIT length) within `max_t`: `(t, world normal)`.
    ///
    /// The general world ray, and the counterpart of [`Self::ray_down`]: no
    /// walkability filter, so a WALL is a hit — that is the whole point. Bolts
    /// ([`crate::projectile::Bolt::spawn`]) and AI line of sight
    /// ([`crate::ai`]) use it to stop at the map's `coll` meshes and `prim`
    /// primitives; the ground query uses `ray_down`.
    ///
    /// `layers` is the subset to test. On the PS2 the rays that pass the
    /// trailing flag — the bolt (`FUN_002764C0`), the soldier reticle
    /// (`FUN_00204748`) and the AI line of sight (`FUN_00162E88`) — test only
    /// the target model's ORDNANCE subset (`analysis/re-collision-layers.md`
    /// §4), so those callers pass [`Layers::ORDNANCE`]. Acquisition and the
    /// reticle pick in `Sim::acquire_magnet` currently use
    /// `perception::los_seat`, also with [`Layers::ORDNANCE`]. This leaves
    /// a mask discrepancy with the PS2 acquisition ray, which passes flag 0
    /// and tests every body ([`Layers::ALL`]).
    ///
    /// Determinism: bodies are visited in ascending `bodies` order with a
    /// strict `<` on `t`, so the FIRST body wins a tie (module header).
    /// The normal is only built for the body that wins.
    ///
    /// The ray is two-sided and NOT solid (`solid = false`, as `ray_down`):
    /// an origin inside a body is not an instant hit at `t = 0`, it hits the
    /// surface it is about to leave through.
    ///
    /// Broad phase, in cost order: the grid (`world::BodyGrid`) offers the bodies
    /// filed under the segment's XZ footprint instead of all of them — on a
    /// 1,578-body map a 30 m line of sight used to read every one of those
    /// ~200 B structs, and the PLAN Log (2026-09-01, session 7) measured the
    /// tail of the AI think tick as exactly that memory traffic — then the
    /// SEGMENT's own AABB against the body's (`push_out`'s box-vs-box test,
    /// six compares), then the slab test. All three are conservative — a
    /// segment that leaves its own bounding box does not exist, and the grid
    /// hands over a superset — so none of them can change the answer.
    ///
    /// Every vehicle hull is in it (card VHULL); [`Self::ray_with`] chooses.
    pub fn ray(&self, origin: Vec3, dir: Vec3, max_t: f32, layers: Layers) -> Option<(f32, Vec3)> {
        self.ray_with(origin, dir, max_t, layers, HullFilter::All)
            .map(|h| (h.t, h.normal))
    }

    /// [`Self::ray`] seeing only the hulls `hulls` admits, and saying whose
    /// hull it hit ([`RayHit::owner`]).
    pub fn ray_with(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_t: f32,
        layers: Layers,
        hulls: HullFilter,
    ) -> Option<RayHit> {
        self.ray_past(origin, dir, max_t, layers, hulls, [None; 2])
    }

    /// [`Self::ray_with`] that also passes through the bodies of the map
    /// instances in `skip` ([`Body::instance`]): a building turret's camera
    /// ray past its own building, as the PS2's excludes the turret's owner
    /// entity (B1 `FUN_00223850` B1:138064-138065, BF2 `FUN_0026cea0`
    /// B2:189959-189960; the hit filter `FUN_00162b60` passes a body whose
    /// owner is on the list, B1:2474-2480).
    pub fn ray_past(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_t: f32,
        layers: Layers,
        hulls: HullFilter,
        skip: [Option<u32>; 2],
    ) -> Option<RayHit> {
        self.ray_past_owners(origin, dir, max_t, layers, hulls, skip, |_| false)
    }

    /// [`Self::ray_past`] that also passes through every hull whose owner
    /// `passed` names: a piercing beam's ray past the vehicles it has
    /// already struck this tick (`Sim::pierce_beam`).
    #[allow(clippy::too_many_arguments)]
    pub fn ray_past_owners(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_t: f32,
        layers: Layers,
        hulls: HullFilter,
        skip: [Option<u32>; 2],
        passed: impl Fn(VehicleHandle) -> bool,
    ) -> Option<RayHit> {
        if max_t <= 0.0 {
            return None;
        }
        super::cost::query();
        let mut seg = Aabb::empty();
        seg.add(origin);
        seg.add(origin + dir * max_t);
        let mut best: Option<(f32, &Body, Vec3)> = None;
        self.visit_segment(origin, dir, max_t, hulls, |body| {
            if !body.layers.contains(layers)
                || (body.instance.is_some() && skip.contains(&body.instance))
                || body.owner.is_some_and(&passed)
            {
                return;
            }
            // Only a nearer hit can replace the best, so look no farther,
            // less a margin ([`Self::ray_down_with`]).
            let reach = best.map_or(max_t, |b| (b.0 + BOX_MARGIN).fmin(max_t));
            if !body.aabb.overlaps(&seg) || !body.aabb.hits_ray(origin, dir, reach) {
                return;
            }
            let o = body.to_local.transform_point3(origin);
            let d = body.to_local.transform_vector3(dir);
            let ray = parry3d::query::Ray::new(point(o), vector(d));
            let Some(hit) = counted_cast(body, &ray, reach, origin) else {
                return;
            };
            if best.is_none_or(|(t, _, _)| hit.time_of_impact < t) {
                best = Some((
                    hit.time_of_impact,
                    body,
                    Vec3::new(hit.normal.x, hit.normal.y, hit.normal.z),
                ));
            }
        });
        best.map(|(t, body, n)| RayHit {
            t,
            normal: body.to_world.transform_vector3(n).normalize_or_zero(),
            owner: body.owner,
            instance: body.instance,
            hit_mult: body.hit_mult,
        })
    }
}

impl CollisionWorld {
    /// Whether [`Self::ray_past`] would hit anything: the same bodies and the
    /// same casts, but the first hit ends it. A line of sight has no use for
    /// which body is nearest, and a blocked one is the common case.
    pub fn ray_blocked(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_t: f32,
        layers: Layers,
        hulls: HullFilter,
        skip: [Option<u32>; 2],
    ) -> bool {
        if max_t <= 0.0 {
            return false;
        }
        super::cost::query();
        let mut seg = Aabb::empty();
        seg.add(origin);
        seg.add(origin + dir * max_t);
        let mut blocked = false;
        self.visit_segment(origin, dir, max_t, hulls, |body| {
            if blocked
                || !body.layers.contains(layers)
                || (body.instance.is_some() && skip.contains(&body.instance))
                || !body.aabb.overlaps(&seg)
                || !body.aabb.hits_ray(origin, dir, max_t)
            {
                return;
            }
            let o = body.to_local.transform_point3(origin);
            let d = body.to_local.transform_vector3(dir);
            let ray = parry3d::query::Ray::new(point(o), vector(d));
            blocked = counted_cast(body, &ray, max_t, origin).is_some();
        });
        blocked
    }

    /// PROVISIONAL (ours; the chase camera, `game::camera_collide`): how far
    /// a ball of `radius` moves from `origin` along `dir` (UNIT length)
    /// before it touches a body in `layers` that `hulls` admits, within
    /// `max_t` (parry's linear shape cast, so the time of impact is a
    /// distance in metres). A ball already touching a body is stopped by it
    /// only when it moves deeper (`stop_at_penetration: false`). Visual
    /// only: the sim never asks it.
    pub fn ball_cast(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_t: f32,
        radius: f32,
        layers: Layers,
        hulls: HullFilter,
    ) -> Option<f32> {
        if max_t <= 0.0 {
            return None;
        }
        let mut seg = Aabb::empty();
        seg.add(origin);
        seg.add(origin + dir * max_t);
        seg.min -= Vec3::splat(radius);
        seg.max += Vec3::splat(radius);
        let ball = parry3d::shape::Ball::new(radius);
        let iso = super::Isometry::translation(origin.x, origin.y, origin.z);
        let options = parry3d::query::ShapeCastOptions {
            max_time_of_impact: max_t,
            target_distance: 0.0,
            stop_at_penetration: false,
            compute_impact_geometry_on_penetration: false,
        };
        let mut best: Option<f32> = None;
        self.visit_xz(seg.min.x, seg.max.x, seg.min.z, seg.max.z, hulls, |body| {
            if !body.layers.contains(layers) || !body.aabb.overlaps(&seg) {
                return;
            }
            let hit = body.shape.with_parry(&mut |s| {
                parry3d::query::cast_shapes(
                    &iso,
                    &vector(dir),
                    &ball,
                    &body.iso,
                    &vector(Vec3::ZERO),
                    s,
                    options,
                )
                .ok()
                .flatten()
            });
            if let Some(h) = hit {
                if best.is_none_or(|b| h.time_of_impact < b) {
                    best = Some(h.time_of_impact);
                }
            }
        });
        best
    }
}

/// The directions [`CollisionWorld::enclosing`] counts crossings along:
/// three near straight up, then eight sideways, each off the axes so a ray
/// does not run down a shared edge (two faces met at one point read as
/// two); two of the three up rays must agree (a trunk's apex vertex).
const ENCLOSE_UP: [Vec3; 3] = [
    Vec3::new(0.0031, 1.0, 0.0017),
    Vec3::new(0.0613, 1.0, -0.0371),
    Vec3::new(-0.0437, 1.0, 0.0589),
];
const ENCLOSE_SIDE: [Vec3; 8] = [
    Vec3::new(1.0, 0.0023, 0.0109),
    Vec3::new(0.7121, 0.0017, 0.7019),
    Vec3::new(0.0127, 0.0029, 1.0),
    Vec3::new(-0.6987, 0.0013, 0.7153),
    Vec3::new(-1.0, 0.0019, -0.0093),
    Vec3::new(-0.7089, 0.0031, -0.7051),
    Vec3::new(-0.0113, 0.0011, -1.0),
    Vec3::new(0.7057, 0.0021, -0.7083),
];
/// How many of the eight sideways rays must cross an odd number of times: a
/// ray through a vertex or along a seam may miscount (Endor's hollow trunk
/// reads one such ray at its axis), a soldier in a nook of single faces with
/// a roof is open on two or more.
const ENCLOSE_SIDE_MIN: usize = 7;

/// Most faces of one body a crossing count walks through before it calls
/// the ray open (a guard; Endor's boulder is crossed once from inside).
const ENCLOSE_PASSES: usize = 16;

impl CollisionWorld {
    /// The SOLDIER-layer mesh body `p` is shut inside, if any (card VMESH;
    /// ours, the PS2 has no such query): two of three rays up and seven of
    /// eight sideways from `p` each cross that one body's faces an ODD
    /// number of times. A closed prop (BF1 Endor's cp2 boulder) answers on every ray;
    /// a hollow trunk open below (Endor's `end_prop_tree04`) answers too,
    /// as only the down ray would see its open end; a roof over a soldier
    /// (one ray of nine) and a room whose walls have two faces (even
    /// sideways) do not. Winding is not read: PS2 collision faces do not
    /// tell their side (see [`Self::ray_down`]). The body's index in
    /// [`Self::bodies`], or `None`; vehicle hulls are not asked.
    pub fn enclosing(&self, p: Vec3) -> Option<usize> {
        let mut found: Option<*const Body> = None;
        self.visit_xz(p.x, p.x, p.z, p.z, HullFilter::Static, |body| {
            let inside = body.aabb.min.cmple(p).all() && p.cmple(body.aabb.max).all();
            if found.is_some()
                || !body.layers.contains(Layers::SOLDIER)
                || !matches!(body.shape, super::shape::Shape::Mesh(_))
                || !inside
            {
                return;
            }
            let reach = (body.aabb.max - body.aabb.min).length() + 1.0;
            let odd = |d: &Vec3| crossings(body, p, d.normalize(), reach) % 2 == 1;
            if at_least(&ENCLOSE_UP, 2, odd) && at_least(&ENCLOSE_SIDE, ENCLOSE_SIDE_MIN, odd) {
                found = Some(body as *const Body);
            }
        });
        let hit = found?;
        self.bodies().iter().position(|b| std::ptr::eq(b, hit))
    }
}

/// Do `min` or more of `dirs` pass `test`? Stops at the first ray that settles it.
fn at_least(dirs: &[Vec3], min: usize, test: impl Fn(&Vec3) -> bool) -> bool {
    let mut passed = 0;
    for (k, d) in dirs.iter().enumerate() {
        if test(d) {
            passed += 1;
        }
        if passed >= min {
            return true;
        }
        if passed + (dirs.len() - k - 1) < min {
            return false;
        }
    }
    false
}

/// A cast against `body`, with the slow-frame log told whose it was.
fn counted_cast(
    body: &Body,
    ray: &parry3d::query::Ray,
    reach: f32,
    origin: Vec3,
) -> Option<parry3d::query::RayIntersection> {
    let hit = body.shape.cast_local(ray, reach);
    super::cost::mesh_cast_of(body.instance, origin.to_array(), reach);
    hit
}

/// How many of `body`'s faces the segment `p + dir·[0, reach]` passes
/// through, walking from one hit to the next (a guard of
/// [`ENCLOSE_PASSES`]; past it the count is 0, an open answer).
fn crossings(body: &Body, p: Vec3, dir: Vec3, reach: f32) -> usize {
    let mut from = 0.0f32;
    for n in 0..ENCLOSE_PASSES {
        let o = body.to_local.transform_point3(p + dir * from);
        let d = body.to_local.transform_vector3(dir);
        let ray = parry3d::query::Ray::new(point(o), vector(d));
        match counted_cast(body, &ray, reach - from, p) {
            Some(h) => from += h.time_of_impact + 1e-3,
            None => return n,
        }
        if from >= reach {
            return n + 1;
        }
    }
    0
}

/// What [`CollisionWorld::ray_with`] hit: the distance, the world normal and
/// the vehicle whose hull it is (`None` for the map's own bodies).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub t: f32,
    pub normal: Vec3,
    pub owner: Option<VehicleHandle>,
    pub instance: Option<u32>,
    /// The body's [`super::Body::hit_mult`].
    pub hit_mult: f32,
}

/// March a ray against the heightfield and return the distance to the first
/// CROSSING of the surface, or None within `max_t`. `step` in meters (bolt v0:
/// 0.5 m).
///
/// The terrain is a SHEET, not a solid: a hit is the first sample pair whose
/// above/below state FLIPS, in EITHER direction, so a ray that starts below the
/// surface is not an immediate hit — it hits when it comes back out (and a ray
/// that stays below never does). Without that, the 29 tat1 units standing 2.7 m
/// under the heightfield INSIDE cp2's building cutter could neither see each
/// other nor fire, and neither could anything under BF2 Polis Massa's surface.
///
/// A crossing whose (bisected) point is inside a terrain cutter is IGNORED —
/// the sheet does not exist there, which is the same rule the height query
/// follows (analysis/re-terrain-heights.md, FUN_00473db0 returns its −300.0
/// sentinel for a cut point) — and the march continues past it.
///
/// PROVISIONAL (ours): the crossing semantics and the cutter skip. The PS2's
/// own terrain ray is untraced.
pub fn ray_heightfield(
    hf: &Heightfield,
    cutters: &[Cutter],
    origin: Vec3,
    dir: Vec3,
    max_t: f32,
    step: f32,
) -> Option<f32> {
    let mut t = 0.0f32;
    let mut prev_t = 0.0f32;
    let mut prev_above = origin.y - hf.height_at(origin.x, origin.z) >= 0.0;
    // Every error of computing a marched point, with room to spare.
    let slack = 0.05 + 1e-5 * (origin.abs().element_sum() + max_t);
    let mut probe = HeightProbe::new(hf);
    while t < max_t {
        t = (t + step).fmin(max_t);
        let p = origin + dir * t;
        let (above, run) = probe.side(p, dir, slack);
        if above != prev_above {
            // One bisection for a tighter crossing point: if the midpoint is
            // still on the near side the crossing is in [mid, t], else in
            // [prev_t, mid].
            let mid = 0.5 * (prev_t + t);
            let pm = origin + dir * mid;
            let mid_above = pm.y - hf.height_at(pm.x, pm.z) >= 0.0;
            let hit = if mid_above == prev_above {
                0.5 * (mid + t)
            } else {
                mid
            };
            if !is_cut_by(cutters, origin + dir * hit) {
                return Some(hit);
            }
            // Cut here: no sheet to cross, keep marching.
        }
        prev_t = t;
        prev_above = above;
        // The samples the probe vouches for keep this side: step past them.
        let end = t + run;
        while t < max_t && (t + step).fmin(max_t) <= end {
            t = (t + step).fmin(max_t);
            prev_t = t;
        }
        if t >= max_t {
            break;
        }
    }
    None
}

/// How near the terrain a midpoint of [`ray_terrain_ps2`]'s bisection has to
/// lie to be the hit (m), and the longest level run it bisects in one piece.
const TERRAIN_RAY_TOL: f32 = 0.1;
const TERRAIN_RAY_PIECE: f32 = 3.0;
/// Extra height samples allowed for a long ray's first bounds check.
const TERRAIN_RAY_WIDE_VERTICES: u32 = 192;

/// BF2's own terrain segment test (card GRNP, `analysis/re-grenades.md`
/// §13.2 [O]; B2 `FUN_00256b58` -> `FUN_0055fb38` -> `FUN_0055f940` ->
/// `FUN_0055f818`): the fraction of `a -> b` at which the segment meets the
/// terrain, `None` when it does not. It is NOT the crossing: a segment that
/// ends under the terrain is halved until a midpoint lies within
/// [`TERRAIN_RAY_TOL`] of the terrain's height, and that midpoint is the hit
/// — up to 0.1 m over the ground or under it. Only a vertical segment gets
/// the exact crossing. A segment that starts under the terrain hits nothing
/// in its first [`TERRAIN_RAY_PIECE`]. The thrown grenade's sweep takes it
/// (its bounce heights and rest are this ray's); bolts and the camera keep
/// [`ray_heightfield`]'s crossing.
///
/// A hit inside a terrain cutter is refused, as the PS2's hole test
/// (`FUN_0055fdd0`) refuses one — PROVISIONAL: the port's cutters stand in
/// for the PS2's holes.
pub fn ray_terrain_ps2(hf: &Heightfield, cutters: &[Cutter], a: Vec3, b: Vec3) -> Option<f32> {
    let delta = b - a;
    if !delta.is_finite() {
        return None;
    }
    let under = |p: Vec3| p.y < hf.height_at(p.x, p.z);
    let f = if delta.x.abs() < 0.001 && delta.z.abs() < 0.001 {
        if b.y >= a.y || under(a) {
            return None;
        }
        (hf.height_at(a.x, a.z) - a.y) / delta.y
    } else {
        let mut clear = !under(a);
        if delta.x.abs().fmax(delta.z.abs()) > TERRAIN_RAY_PIECE * 32.0
            && terrain_segment_stays_on_side(hf, a, b, delta, clear)
        {
            return None;
        }
        terrain_march(hf, a, b, &mut clear)
    };
    (f < 1.0 && !is_cut_by(cutters, a + delta * f)).then_some(f)
}

#[cold]
fn terrain_segment_stays_on_side(
    hf: &Heightfield,
    a: Vec3,
    b: Vec3,
    delta: Vec3,
    clear: bool,
) -> bool {
    let span = delta.x.abs().fmax(delta.z.abs());
    // Spend at most one vertex read for four ordinary pieces, once.
    let budget = ((span / (TERRAIN_RAY_PIECE * 4.0)) as u32).min(TERRAIN_RAY_WIDE_VERTICES);
    hf.segment_wide_bounds(a, b, budget)
        .is_some_and(|(low, high)| a.y.fmin(b.y) >= high || (!clear && a.y.fmax(b.y) < low))
}

/// Keep the original rounding on normal segments. Halve before adding if the sum overflows.
fn terrain_midpoint(a: Vec3, b: Vec3) -> Vec3 {
    let mid = (a + b) * 0.5;
    if mid.is_finite() {
        mid
    } else {
        a * 0.5 + b * 0.5
    }
}

/// `FUN_0055f940`: pieces of at most [`TERRAIN_RAY_PIECE`] level metres,
/// nearest first; a piece whose end is under the terrain is bisected when
/// the ray has been over the terrain (`clear`, shared along the ray).
fn terrain_march(hf: &Heightfield, a: Vec3, b: Vec3, clear: &mut bool) -> f32 {
    let d = a - b;
    let mid = (d.x * d.x + d.z * d.z > TERRAIN_RAY_PIECE * TERRAIN_RAY_PIECE)
        .then(|| terrain_midpoint(a, b));
    if let Some(mid) = mid.filter(|&mid| mid != a && mid != b) {
        // Both ends clamp to the same sample, so all recursive midpoints
        // read its height too. Skip pieces whose endpoints keep the same
        // side and cannot change the search state or produce a hit.
        if hf.grid_point(a.x, a.z) == hf.grid_point(b.x, b.z) {
            let height = hf.height_at(a.x, a.z);
            let a_under = a.y < height;
            let b_under = b.y < height;
            if !a_under && !b_under {
                *clear = true;
                return 1.0;
            }
            if a_under && b_under && !*clear {
                return 1.0;
            }
        }
        // A long segment can have a moving sample even when its other
        // axis is clamped to the terrain's edge. For a small rectangle of
        // cells, every midpoint stays inside its conservative height bounds.
        if d.x.abs().fmax(d.z.abs()) > TERRAIN_RAY_PIECE * 2.0 {
            if let Some((low, high)) = hf.segment_cell_bounds(a, b) {
                if a.y.fmin(b.y) >= high {
                    *clear = true;
                    return 1.0;
                }
                if !*clear && a.y.fmax(b.y) < low {
                    return 1.0;
                }
            }
        }
        let f = terrain_march(hf, a, mid, clear);
        if f < 1.0 {
            return f * 0.5;
        }
        return terrain_march(hf, mid, b, clear) * 0.5 + 0.5;
    }
    // A segment with no representable split still carries its height bracket.
    if b.y < hf.height_at(b.x, b.z) {
        if *clear {
            return terrain_bisect(hf, a, b);
        }
    } else {
        *clear = true;
    }
    1.0
}

/// `FUN_0055f818`: halve `lo -> hi` until the two ends are within the
/// tolerance of each other or the midpoint is within it of the terrain.
fn terrain_bisect(hf: &Heightfield, mut lo: Vec3, mut hi: Vec3) -> f32 {
    let (mut f, mut h) = (0.0f32, 0.5f32);
    while (lo - hi).length_squared() > TERRAIN_RAY_TOL * TERRAIN_RAY_TOL {
        let mid = terrain_midpoint(lo, hi);
        if mid == lo || mid == hi {
            break;
        }
        let e = hf.height_at(mid.x, mid.z) - mid.y;
        if e > TERRAIN_RAY_TOL {
            hi = mid;
        } else if e >= -TERRAIN_RAY_TOL {
            break;
        } else {
            f += h;
            lo = mid;
        }
        h *= 0.5;
    }
    f + h
}

/// Flat-fire intercept (BF1 FUN_0026d3f0 without the gravity-drop branch —
/// bolts have no gravity): solve |d + v·t| = s·t for the earliest t > 0 and
/// return the point `d + v·t` (relative to the shooter). `d` = target −
/// shooter, `v` = target velocity, `s` = ordnance speed. None if the target
/// outruns the bolt.
pub fn lead_point(d: Vec3, v: Vec3, s: f32) -> Option<Vec3> {
    let a = v.length_squared() - s * s;
    let b = 2.0 * d.dot(v);
    let c = d.length_squared();
    let t = if a.abs() < 1e-6 {
        if b.abs() < 1e-9 {
            return None;
        }
        -c / b
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let t1 = (-b - sq) / (2.0 * a);
        let t2 = (-b + sq) / (2.0 * a);
        match (t1 > 0.0, t2 > 0.0) {
            (true, true) => t1.fmin(t2),
            (true, false) => t1,
            (false, true) => t2,
            _ => return None,
        }
    };
    (t > 0.0).then(|| d + v * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::tests::{floor_quad, mesh_body, ramp, slab, world_from_triangles};
    use crate::collision::{Shape, NO_GROUND};
    use crate::sim::Sim;
    use glam::{Mat4, Vec4};

    fn flat_hf(y: f32) -> Heightfield {
        Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![y; 4],
        }
    }

    fn sim_with(world: CollisionWorld, terrain_y: Option<f32>) -> Sim {
        let mut s = Sim::empty(1);
        s.heightfield = terrain_y.map(flat_hf);
        s.collision = world;
        s
    }

    #[test]
    fn retail_terrain_ray_skips_long_runs_clamped_to_the_same_height() {
        let hf = flat_hf(0.0);
        let a = Vec3::new(-100_000_000.0, 1.0, 0.0);
        let b = Vec3::new(100_000_000.0, 1.0, 0.0);
        assert_eq!(ray_terrain_ps2(&hf, &[], a, b), None);
        assert_eq!(
            ray_terrain_ps2(&hf, &[], a.with_y(-1.0), b.with_y(-1.0)),
            None
        );

        // First crossing is outside the grid, at the same clamped sample.
        // The regular nearest-first split still supplies its fraction.
        let b = b.with_y(-3.0);
        let f = ray_terrain_ps2(&hf, &[], a, b).unwrap();
        assert_eq!(f, 0.25);
    }

    #[test]
    fn retail_terrain_ray_preserves_a_varying_clamped_cell() {
        let hf = Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 0.0,
            offset_z: 0.0,
            heights: vec![0.0, 10.0, 1.0, 11.0],
        };
        let a = Vec3::new(200.0, 1.0, -100_000.0);
        let b = Vec3::new(3800.0, 1.0, -2.0);
        let hit = ray_terrain_ps2(&hf, &[], a, b).expect("edge height rises across the ray");
        assert!((0.05..0.06).contains(&hit), "{hit}");
        assert_eq!(
            ray_terrain_ps2(&hf, &[], a.with_y(12.0), b.with_y(12.0)),
            None
        );
        assert_eq!(
            ray_terrain_ps2(&hf, &[], a.with_y(-1.0), b.with_y(-1.0)),
            None
        );
    }

    #[test]
    fn retail_terrain_ray_keeps_the_first_crossing_across_two_cells() {
        let hf = Heightfield {
            size: 3,
            unit: 4000.0,
            offset_x: 0.0,
            offset_z: 0.0,
            heights: vec![0.0, 8.0, 0.0, 0.0, 8.0, 0.0, 0.0, 8.0, 0.0],
        };
        let a = Vec3::new(100.0, 1.0, -100_000.0);
        let b = Vec3::new(7900.0, 1.0, -2.0);
        let hit = ray_terrain_ps2(&hf, &[], a, b).expect("the middle vertex rises above the ray");
        assert!((0.04..0.06).contains(&hit), "{hit}");
        assert_eq!(
            ray_terrain_ps2(&hf, &[], a.with_y(9.0), b.with_y(9.0)),
            None
        );
    }

    #[test]
    fn retail_terrain_ray_matches_the_march_across_many_cells() {
        let hf = Heightfield {
            size: 17,
            unit: 4000.0,
            offset_x: 0.0,
            offset_z: 0.0,
            heights: (0..17 * 17).map(|i| (i * 17 % 43) as f32).collect(),
        };
        let mut hits = 0;
        let mut skips = 0;
        for z in [-80.0, 200.0, 19_000.0] {
            for (start_y, end_y) in [(50.0, 60.0), (50.0, -5.0), (-5.0, 50.0), (-5.0, -3.0)] {
                let from = Vec3::new(-1000.0, start_y, z);
                let to = Vec3::new(63_990.0, end_y, z + 4000.0);
                for (a, b) in [(from, to), (to, from)] {
                    let mut clear = a.y >= hf.height_at(a.x, a.z);
                    skips += usize::from(terrain_segment_stays_on_side(&hf, a, b, b - a, clear));
                    let old = terrain_march(&hf, a, b, &mut clear);
                    let want = (old < 1.0).then(|| old.to_bits());
                    let got = ray_terrain_ps2(&hf, &[], a, b).map(f32::to_bits);
                    assert_eq!(got, want, "{a} -> {b}");
                    hits += usize::from(got.is_some());
                }
            }
        }
        assert_eq!(skips, 12);
        assert!(hits > 0);
    }

    #[test]
    fn retail_terrain_ray_rejects_nonfinite_endpoints() {
        let hf = flat_hf(0.0);
        let finite = Vec3::new(0.0, 1.0, 0.0);
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let invalid = Vec3::new(invalid, -1.0, 0.0);
            assert_eq!(ray_terrain_ps2(&hf, &[], finite, invalid), None);
            assert_eq!(ray_terrain_ps2(&hf, &[], invalid, finite), None);
        }
    }

    #[test]
    fn retail_terrain_ray_finishes_when_the_midpoint_rounds_to_an_end() {
        let hf = flat_hf(0.0);
        let from = Vec3::new(134_217_728.0, 1.0, 0.0);
        let to = Vec3::new(134_217_744.0, 1.0, 0.0);
        assert_eq!((from + to) * 0.5, from);
        assert_eq!(ray_terrain_ps2(&hf, &[], from, to), None);
        assert_eq!(ray_terrain_ps2(&hf, &[], to, from), None);
    }

    #[test]
    fn retail_terrain_ray_finishes_a_nonrepresentable_height_bracket() {
        let from = Vec3::new(134_217_728.0, 1.0, 0.0);
        let to = Vec3::new(134_217_744.0, 1.0, 0.0);
        let hf = Heightfield {
            size: 2,
            unit: 16.0,
            offset_x: -from.x,
            offset_z: 0.0,
            heights: vec![0.0, 2.0, 0.0, 2.0],
        };
        assert_eq!(hf.height_at(from.x, 0.0), 0.0);
        assert_eq!(hf.height_at(to.x, 0.0), 2.0);
        assert_eq!(ray_terrain_ps2(&hf, &[], from, to), Some(0.5));
    }

    #[test]
    fn retail_terrain_ray_finishes_with_overflowing_midpoint_sum() {
        let hf = flat_hf(0.0);
        let a = Vec3::new(f32::from_bits(f32::MAX.to_bits() - 1), 1.0, 0.0);
        let b = Vec3::new(f32::MAX, 1.0, 0.0);
        assert!(!(a + b).is_finite());
        assert_eq!(ray_terrain_ps2(&hf, &[], a, b), None);
        assert_eq!(ray_terrain_ps2(&hf, &[], b, a), None);
        let top = Vec3::Y * f32::MAX;
        assert_eq!(ray_terrain_ps2(&hf, &[], top, -top), None);
    }

    #[test]
    fn retail_terrain_ray_preserves_a_crossing_and_a_clear_ray() {
        let hf = flat_hf(0.0);
        let from = Vec3::new(-5.0, 1.0, 0.0);
        let across = Vec3::new(5.0, -1.0, 0.0);
        assert_eq!(ray_terrain_ps2(&hf, &[], from, across), Some(0.53125));
        let cut = Cutter {
            sphere: [1.0, 0.3125, -0.0625, 0.0],
            planes: vec![Vec4::new(1.0, 0.0, 0.0, -1.0)],
        };
        assert_eq!(ray_terrain_ps2(&hf, &[cut], from, across), None);
        assert_eq!(
            ray_terrain_ps2(&hf, &[], from, Vec3::Y + Vec3::X * 5.0),
            None
        );
        assert_eq!(ray_terrain_ps2(&hf, &[], Vec3::Y, Vec3::NEG_Y), Some(0.5));
    }

    #[test]
    fn ground_prefers_platform_below_probe_over_terrain() {
        let s = sim_with(world_from_triangles(floor_quad(5.0, 5.0)), Some(0.0));
        assert_eq!(s.ground_under(Vec3::new(0.0, 5.2, 0.0), false), 5.0);
        // The platform is above the probe: the terrain wins.
        assert_eq!(s.ground_under(Vec3::new(0.0, 0.0, 0.0), false), 0.0);
    }

    #[test]
    fn ground_ignores_platform_more_than_step_above_feet() {
        let s = sim_with(world_from_triangles(floor_quad(5.0, 5.0)), Some(0.0));
        // 0.7 m below a 0.6 m step: out of reach.
        assert_eq!(s.ground_under(Vec3::new(0.0, 4.3, 0.0), false), 0.0);
        assert_eq!(s.ground_under(Vec3::new(0.0, 4.5, 0.0), false), 5.0);
    }

    #[test]
    fn rising_probe_does_not_mantle() {
        let s = sim_with(world_from_triangles(floor_quad(5.0, 5.0)), Some(0.0));
        assert_eq!(s.ground_under(Vec3::new(0.0, 4.5, 0.0), true), 0.0);
    }

    #[test]
    fn cut_terrain_yields_model_floor_or_sentinel() {
        // A cutter box around the origin, y in (−5, 12).
        let cutter = Cutter {
            planes: vec![
                Vec4::new(1.0, 0.0, 0.0, -5.0),
                Vec4::new(-1.0, 0.0, 0.0, -5.0),
                Vec4::new(0.0, 1.0, 0.0, -12.0),
                Vec4::new(0.0, -1.0, 0.0, -5.0),
                Vec4::new(0.0, 0.0, 1.0, -5.0),
                Vec4::new(0.0, 0.0, -1.0, -5.0),
            ],
            ..Default::default()
        };
        let mut world = world_from_triangles(floor_quad(2.0, 4.0));
        world.cutters.push(cutter);
        let s = sim_with(world, Some(10.0));
        assert!(s.collision.is_cut(Vec3::new(0.0, 2.1, 0.0)));
        assert_eq!(s.ground_under(Vec3::new(0.0, 2.1, 0.0), false), 2.0);
        // Above the cutter's cap the terrain is intact again.
        assert!(!s.collision.is_cut(Vec3::new(0.0, 20.0, 0.0)));
        assert_eq!(s.ground_under(Vec3::new(0.0, 20.0, 0.0), false), 10.0);

        // With no model floor inside the cut there is no ground at all.
        let mut bare = CollisionWorld::default();
        bare.cutters.push(Cutter {
            planes: s.collision.cutters[0].planes.clone(),
            ..Default::default()
        });
        let s = sim_with(bare, Some(10.0));
        assert_eq!(s.ground_under(Vec3::new(0.0, 2.1, 0.0), false), NO_GROUND);
    }

    /// PROVISIONAL `TERRAIN_BURY`: the heightfield is a one-sided sheet, so a
    /// unit far enough UNDER an uncut one is underground and the model floor
    /// beneath it is the ground (BF2 Polis Massa's under-surface base).
    #[test]
    fn unit_under_an_uncut_heightfield_stands_on_the_model_floor() {
        let s = sim_with(world_from_triangles(floor_quad(10.0, 5.0)), Some(40.0));
        // 30 m under the surface, standing on the box: the terrain is absent.
        // (The probe is 330 m long, so the mesh hit is only float-close.)
        let g = s.ground_under(Vec3::new(0.0, 10.2, 0.0), false);
        assert!((g - 10.0).abs() < 1e-4, "{g}");
        // 1 m under the surface: inside the bury depth, so the terrain still
        // wins outright and snaps the unit up onto it.
        assert_eq!(s.ground_under(Vec3::new(0.0, 39.0, 0.0), false), 40.0);
        // 2.5 m under: past the bury depth — falls through to the box.
        let g = s.ground_under(Vec3::new(0.0, 37.5, 0.0), false);
        assert!((g - 10.0).abs() < 1e-4, "{g}");
        // ... and with nothing under it, to NO_GROUND.
        let bare = sim_with(CollisionWorld::default(), Some(40.0));
        assert_eq!(
            bare.ground_under(Vec3::new(0.0, 37.5, 0.0), false),
            NO_GROUND
        );
        // The surface itself is unaffected.
        assert_eq!(bare.ground_under(Vec3::new(0.0, 40.0, 0.0), false), 40.0);
    }

    #[test]
    fn prim_cylinder_top_and_box_top_are_ground() {
        // The Kamino platform floor shape: cyl (r 5, FULL height 8) at y = −4,
        // so its top is exactly y = 0.
        let cyl = Body::new(
            Mat4::from_translation(Vec3::new(0.0, -4.0, 0.0)),
            Shape::Cylinder {
                radius: 5.0,
                half_height: 4.0,
            },
        );
        let s = sim_with(CollisionWorld::new(vec![cyl], Vec::new()), Some(-10.0));
        // parry's cylinder cast lands on the cap to within 6e-8, not bit-exact.
        let g = s.ground_under(Vec3::new(0.0, 0.3, 0.0), false);
        assert!(g.abs() < 1e-5, "cylinder top is y = 0: {g}");

        let bx = Body::new(
            Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0)),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 2.0),
            },
        );
        let s = sim_with(CollisionWorld::new(vec![bx], Vec::new()), Some(-10.0));
        let g = s.ground_under(Vec3::new(0.0, 4.2, 0.0), false);
        assert!((g - 4.0).abs() < 1e-5, "box top is y = 4: {g}");
    }

    /// Card GRND: two deck pieces at y = 10 over a hidden disc at y = 2, as
    /// KAM1's cloning rooms are built. A slit narrower than the foot is
    /// stood on (the ring finds the pieces, lowered by the sphere's sag at
    /// that radius); a gap wider than it is a hole; a piece answers exactly.
    #[test]
    fn the_foot_bridges_a_slit_between_floor_pieces() {
        let world = |gap: f32| {
            let piece = |x: f32| {
                Body::new(
                    Mat4::from_translation(Vec3::new(x, 9.5, 0.0)),
                    Shape::Box {
                        half: Vec3::new(2.0, 0.5, 5.0),
                    },
                )
            };
            let disc = Body::new(
                Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
                Shape::Cylinder {
                    radius: 20.0,
                    half_height: 1.0,
                },
            );
            let x = 2.0 + gap * 0.5;
            sim_with(
                CollisionWorld::new(vec![piece(-x), piece(x), disc], Vec::new()),
                None,
            )
        };
        let s = world(0.4);
        let g = s.ground_under(Vec3::new(0.0, 10.0, 0.0), false);
        assert!((9.9..10.0).contains(&g), "stands on the slit: {g}");
        let on = s.ground_under(Vec3::new(-2.0, 10.0, 0.0), false);
        assert!((on - 10.0).abs() < 1e-4, "a piece answers exactly: {on}");
        let s = world(1.0);
        let g = s.ground_under(Vec3::new(0.0, 10.0, 0.0), false);
        assert!((g - 2.0).abs() < 1e-4, "a 1 m gap is a hole: {g}");
    }

    /// Card RAIL: a rail like BF2 Kamino's rim — a 52° bevel (n.y 0.615: a
    /// floor, but one the stick has no hold on) rising 0.5 m to a flat top.
    /// The foot rides the top's edge: within the ring's reach of it the floor
    /// is the top, less the sphere's sag, with the top's normal; lower on the
    /// bevel it is the bevel. A soldier walking at 5 m/s goes over it, which
    /// the point foot did not allow under 7.
    #[test]
    fn the_foot_rides_the_top_of_a_short_steep_bevel() {
        let q = |x0: f32, y0: f32, x1: f32, y1: f32| {
            let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
            vec![
                [p(x0, y0, -5.0), p(x1, y1, -5.0), p(x1, y1, 5.0)],
                [p(x0, y0, -5.0), p(x1, y1, 5.0), p(x0, y0, 5.0)],
            ]
        };
        let mut tris = q(-6.0, 0.0, 1.0, 0.0);
        tris.extend(q(1.0, 0.0, 1.39, 0.5));
        tris.extend(q(1.39, 0.5, 2.5, 0.5));
        tris.extend(q(2.5, 0.0, 8.0, 0.0));
        let mut s = sim_with(world_from_triangles(tris), None);
        let (y, n) = s.ground_under_n(Vec3::new(1.05, 0.06, 0.0), false, 0.06);
        assert!(
            y < 0.1 && n.y < 0.7,
            "low on the bevel: the bevel ({y}, {n})"
        );
        let (y, n) = s.ground_under_n(Vec3::new(1.2, 0.25, 0.0), false, 0.25);
        assert!((y - 0.415).abs() < 0.01, "the top, less the sag: {y}");
        assert!(n.y > 0.99, "with the top's normal: {n}");

        use crate::sim::PadState;
        use crate::unit::UnitSpec;
        let h = s
            .spawn(&UnitSpec {
                yaw: -std::f32::consts::FRAC_PI_2, // facing +X
                ..UnitSpec::player(0, Vec3::new(-3.0, 0.0, 0.0))
            })
            .expect("spawned");
        s.unit_mut(h).expect("alive").consts.max_speed = 5.0;
        let pad = PadState {
            ly: i16::MAX,
            ..PadState::default()
        };
        for _ in 0..90 {
            s.step(&[pad]);
        }
        let x = s.unit(h).expect("alive").soldier.pos.x;
        assert!(x > 3.0, "over the rail at 5 m/s: x {x}");
    }

    /// Card GRND: a face too steep to stand on does not hide the floor under
    /// it in the same body (KAM1's pad rims).
    #[test]
    fn a_floor_under_a_steep_face_of_the_same_body_is_found() {
        let mut tris = ramp(70.0, 5.3);
        tris.extend(floor_quad(5.0, 5.0));
        let s = sim_with(world_from_triangles(tris), Some(0.0));
        let g = s.ground_under(Vec3::new(0.0, 5.5, 0.0), false);
        assert!((g - 5.0).abs() < 1e-4, "{g}");
    }

    #[test]
    fn steep_faces_are_not_ground() {
        // 70° is steeper than the 60° walkable limit: fall through to terrain.
        let s = sim_with(world_from_triangles(ramp(70.0, 2.0)), Some(0.0));
        assert_eq!(s.ground_under(Vec3::new(0.0, 2.1, 0.0), false), 0.0);
        // 30° is walkable, and at x = 0 the plane is at y = 2.
        let s = sim_with(world_from_triangles(ramp(30.0, 2.0)), Some(0.0));
        let g = s.ground_under(Vec3::new(0.0, 2.1, 0.0), false);
        assert!((g - 2.0).abs() < 1e-4, "{g}");
    }

    #[test]
    fn ray_down_takes_the_highest_of_two_stacked_floors() {
        let world = CollisionWorld::new(
            vec![
                mesh_body(&floor_quad(2.0, 5.0)),
                mesh_body(&floor_quad(5.0, 5.0)),
            ],
            Vec::new(),
        );
        let (y, n) = world
            .ray_down(Vec3::new(0.0, 10.0, 0.0), 20.0)
            .expect("hit");
        assert!((y - 5.0).abs() < 1e-5, "{y}");
        assert!(n.y.abs() > 0.99, "{n}");
        // Nothing below the ray's reach.
        assert!(world.ray_down(Vec3::new(0.0, 10.0, 0.0), 1.0).is_none());
        assert!(world.ray_down(Vec3::new(100.0, 10.0, 0.0), 20.0).is_none());
    }

    /// The general world ray, and what separates it from `ray_down`: NO
    /// walkability filter, so a vertical wall is a hit. Plus the `max_t` and
    /// direction bounds.
    ///
    /// The mask is `Layers::ALL` throughout: these fixtures are `Body::new`
    /// bodies (every layer, the PS2 default) and the subject is the ray's
    /// GEOMETRY, not its layer filter — `ALL` is also the mask the aim-assist
    /// ray will pass (RE §4). The filter itself is the three tests below.
    #[test]
    fn ray_stops_at_a_wall_and_respects_max_t() {
        let world = CollisionWorld::new(vec![slab(-20.0)], Vec::new());
        let (t, n) = world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ALL)
            .expect("hit");
        assert!((t - 19.5).abs() < 1e-4, "near face at 19.5 m: {t}");
        assert!((n - Vec3::Z).length() < 1e-4, "{n}");
        // The surface `ray_down` would have thrown away as too steep — which
        // is exactly the surface a bolt and a line of sight must see.
        assert!(n.y.abs() < WALKABLE_MIN_NY, "{n}");
        // Past `max_t`, and a zero-length ray: nothing.
        assert!(world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 19.4, Layers::ALL)
            .is_none());
        assert!(world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 0.0, Layers::ALL)
            .is_none());
        // Pointing away, and missing sideways: nothing.
        assert!(world.ray(Vec3::ZERO, Vec3::Z, 100.0, Layers::ALL).is_none());
        assert!(world
            .ray(Vec3::new(20.0, 0.0, 0.0), Vec3::NEG_Z, 100.0, Layers::ALL)
            .is_none());
    }

    /// The nearest body wins on DISTANCE (the far one is listed first here),
    /// and an exact tie goes to the FIRST body — the strict `<` of the module
    /// header's determinism rule.
    #[test]
    fn ray_takes_the_nearest_body_and_the_first_on_an_exact_tie() {
        // Two OVERLAPPING slabs — z in (−20.9, −19.9) and (−20.5, −19.5) —
        // with the NEARER one second in `bodies`.
        let world = CollisionWorld::new(vec![slab(-20.4), slab(-20.0)], Vec::new());
        let (t, _) = world
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ALL)
            .expect("hit");
        assert!((t - 19.5).abs() < 1e-4, "nearest, not first: {t}");

        // The tie, made OBSERVABLE. Two shapes chosen so both t values are
        // BIT-exactly 16.0 while their normals differ:
        //   ball  r 5 at (0, 3, −20): |(0,−3,20)| along −Z gives
        //         t = 20 − sqrt(25 − 9) = 16, normal (0, −3, 4)/5;
        //   slab  near face at z = −16:                 t = 16, normal +Z.
        // Whichever is FIRST in `bodies` is the normal that comes back.
        let ball = || {
            Body::new(
                Mat4::from_translation(Vec3::new(0.0, 3.0, -20.0)),
                Shape::Sphere { radius: 5.0 },
            )
        };
        let pair = |a: Body, b: Body| CollisionWorld::new(vec![a, b], Vec::new());
        let (t0, n0) = pair(ball(), slab(-16.5))
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ALL)
            .expect("hit");
        let (t1, n1) = pair(slab(-16.5), ball())
            .ray(Vec3::ZERO, Vec3::NEG_Z, 100.0, Layers::ALL)
            .expect("hit");
        assert_eq!(t0, 16.0, "the tie is exact");
        assert_eq!(t1, 16.0, "the tie is exact");
        assert!(
            (n0 - Vec3::new(0.0, -0.6, 0.8)).length() < 1e-5,
            "first body (the ball) wins: {n0}"
        );
        assert!(
            (n1 - Vec3::Z).length() < 1e-5,
            "first body (the slab) wins: {n1}"
        );
    }

    fn slope() -> Heightfield {
        // 5x5 grid, unit 10, world x/z in [-20, 20]: flat for z <= 0, then a
        // 45-degree ramp (height = z) toward +z.
        Heightfield {
            size: 5,
            unit: 10.0,
            offset_x: 20.0,
            offset_z: 20.0,
            heights: (0..25)
                .map(|i| {
                    let z = (i / 5) as f32 * 10.0 - 20.0;
                    z.max(0.0)
                })
                .collect(),
        }
    }

    #[test]
    fn ray_heightfield_finds_the_wall_and_clears_flat_ground() {
        let hf = slope();
        // Horizontal bolt at y = 5 from z = -10 toward +z meets the ramp
        // where height = 5, i.e. z = 5 → t = 15 (planar ramp: exact lerp).
        let t = ray_heightfield(&hf, &[], Vec3::new(0.0, 5.0, -10.0), Vec3::Z, 100.0, 0.5).unwrap();
        assert!((t - 15.0).abs() < 0.3, "{t}");
        // Flat part, going -z: no hit.
        assert!(
            ray_heightfield(&hf, &[], Vec3::new(0.0, 1.0, 0.0), Vec3::NEG_Z, 15.0, 0.5).is_none()
        );
    }

    /// The terrain is a sheet you collide with by CROSSING it: a ray that
    /// starts under the surface is not an immediate hit, and one that stays
    /// under it never hits at all. (Before this, every unit standing under a
    /// heightfield was blind and its bolts died at t = 0.)
    #[test]
    fn ray_from_below_the_surface_is_not_an_immediate_hit() {
        let hf = slope();
        // 1 m under the flat part, running along it into the flat: never
        // crosses back out.
        assert!(
            ray_heightfield(&hf, &[], Vec3::new(0.0, -1.0, 0.0), Vec3::NEG_Z, 10.0, 0.5).is_none()
        );
        // Under the flat part, climbing steeply: crosses out at y = 0, i.e.
        // t = sqrt(2) ≈ 1.414 along a 45° up-ray.
        let d = Vec3::new(0.0, 1.0, -1.0).normalize();
        let t = ray_heightfield(&hf, &[], Vec3::new(0.0, -1.0, 0.0), d, 10.0, 0.1)
            .expect("crosses out");
        assert!((t - 2f32.sqrt()).abs() < 0.2, "{t}");
    }

    /// A crossing inside a cutter is not a crossing — there is no sheet there.
    #[test]
    fn crossing_inside_a_cutter_is_ignored() {
        let hf = slope();
        // Everything with |x| < 30, |z| < 30 is cut away.
        let cutter = Cutter {
            planes: vec![
                Vec4::new(1.0, 0.0, 0.0, -30.0),
                Vec4::new(-1.0, 0.0, 0.0, -30.0),
                Vec4::new(0.0, 1.0, 0.0, -30.0),
                Vec4::new(0.0, -1.0, 0.0, -30.0),
                Vec4::new(0.0, 0.0, 1.0, -30.0),
                Vec4::new(0.0, 0.0, -1.0, -30.0),
            ],
            ..Default::default()
        };
        // The same shot as the wall case above, which hits at t ≈ 15.
        let cut = [cutter];
        assert!(
            ray_heightfield(&hf, &cut, Vec3::new(0.0, 5.0, -10.0), Vec3::Z, 100.0, 0.5).is_none(),
            "the ramp is cut away here"
        );
    }

    /// The march's skips change nothing: on a rough grid, rays from over,
    /// under and on the surface, in and out of the grid, cross where a march
    /// that lerps every sample crosses, bit for bit (card HULLC).
    #[test]
    fn the_probed_march_is_the_plain_march() {
        fn plain(hf: &Heightfield, o: Vec3, d: Vec3, max_t: f32, step: f32) -> Option<f32> {
            let side = |p: Vec3| p.y - hf.height_at(p.x, p.z) >= 0.0;
            let (mut t, mut prev_t, mut prev) = (0.0f32, 0.0f32, side(o));
            while t < max_t {
                t = (t + step).min(max_t);
                let above = side(o + d * t);
                if above != prev {
                    let mid = 0.5 * (prev_t + t);
                    return Some(if side(o + d * mid) == prev {
                        0.5 * (mid + t)
                    } else {
                        mid
                    });
                }
                (prev_t, prev) = (t, above);
            }
            None
        }
        let mut seed = 0x9E37_79B9u32;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let n = 17;
        let hf = Heightfield {
            size: n,
            unit: 4.0,
            offset_x: 30.0,
            offset_z: 20.0,
            heights: (0..n * n).map(|_| 12.0 * rnd() - 4.0).collect(),
        };
        let mut hits = 0;
        for _ in 0..20_000 {
            let o = Vec3::new(90.0 * rnd() - 40.0, 0.0, 90.0 * rnd() - 30.0);
            let y = hf.height_at(o.x, o.z) + 6.0 * rnd() - 2.0;
            let o = Vec3::new(o.x, y, o.z);
            let d = Vec3::new(rnd() - 0.5, 0.4 * rnd() - 0.25, rnd() - 0.5).normalize();
            let (max_t, step) = (80.0 * rnd(), [0.25, 0.5, 1.0][(rnd() * 2.99) as usize]);
            let want = plain(&hf, o, d, max_t, step);
            let got = ray_heightfield(&hf, &[], o, d, max_t, step);
            assert_eq!(
                got.map(f32::to_bits),
                want.map(f32::to_bits),
                "{o} {d} {max_t} {step}"
            );
            hits += usize::from(want.is_some());
        }
        assert!(hits > 5_000, "the rays meet the terrain: {hits}");
    }

    /// ... and the march CONTINUES past an ignored crossing: a ray that leaves
    /// the cut volume still meets the terrain outside it.
    #[test]
    fn crossing_out_of_a_cutter_into_terrain_hits() {
        // Two hills along +z: height 0 at z = −20, 10 at z = −10, 0 at z = 0,
        // 10 at z = 10, 0 at z = 20.
        let hf = Heightfield {
            size: 5,
            unit: 10.0,
            offset_x: 20.0,
            offset_z: 20.0,
            heights: (0..25)
                .map(|i| if (i / 5) % 2 == 1 { 10.0 } else { 0.0 })
                .collect(),
        };
        // A horizontal ray at y = 5 from z = −20 crosses INTO hill 1 at
        // z = −15, out of it at z = −5, into hill 2 at z = 5.
        let o = Vec3::new(0.0, 5.0, -20.0);
        let t = ray_heightfield(&hf, &[], o, Vec3::Z, 100.0, 0.5).expect("hill 1");
        assert!((t - 5.0).abs() < 0.6, "{t}");
        // Cut hill 1 away (z < −2): its two crossings are ignored and the
        // march goes on to hill 2 at z = 5, i.e. t = 25.
        let cut = [Cutter {
            planes: vec![
                Vec4::new(0.0, 0.0, 1.0, 2.0),
                Vec4::new(0.0, 0.0, -1.0, -30.0),
                Vec4::new(1.0, 0.0, 0.0, -30.0),
                Vec4::new(-1.0, 0.0, 0.0, -30.0),
                Vec4::new(0.0, 1.0, 0.0, -30.0),
                Vec4::new(0.0, -1.0, 0.0, -30.0),
            ],
            ..Default::default()
        }];
        let t = ray_heightfield(&hf, &cut, o, Vec3::Z, 100.0, 0.5).expect("hill 2");
        assert!((t - 25.0).abs() < 0.6, "{t}");
    }

    /// The `CUTR INFO` bounding sphere is a cheap REJECT before the (up to 18)
    /// planes: outside it is a definite miss; an all-zero sphere means the
    /// bake carried none and the planes decide alone.
    #[test]
    fn cutter_bounding_sphere_rejects_before_the_planes() {
        // Planes alone: an infinite slab |y| < 1, which contains both points.
        let planes = vec![
            Vec4::new(0.0, 1.0, 0.0, -1.0),
            Vec4::new(0.0, -1.0, 0.0, -1.0),
        ];
        let unbounded = [Cutter {
            planes: planes.clone(),
            ..Default::default()
        }];
        assert!(is_cut_by(&unbounded, Vec3::ZERO));
        assert!(is_cut_by(&unbounded, Vec3::new(100.0, 0.0, 0.0)));
        // A 10 m sphere at the origin keeps the near point and rejects the far
        // one without ever evaluating a plane.
        let bounded = [Cutter {
            sphere: [10.0, 0.0, 0.0, 0.0],
            planes,
        }];
        assert!(is_cut_by(&bounded, Vec3::ZERO));
        assert!(!is_cut_by(&bounded, Vec3::new(100.0, 0.0, 0.0)));
    }

    #[test]
    fn lead_point_solves_flat_intercept() {
        // Target 20 m ahead moving 5 m/s laterally, bolt 80 m/s:
        // t = sqrt(400 / (6400 - 25)) = 0.25049 → lateral lead 1.2525 m.
        let p = lead_point(Vec3::new(0.0, 0.0, -20.0), Vec3::X * 5.0, 80.0).unwrap();
        assert!((p.x - 1.2525).abs() < 1e-3, "{p}");
        assert!((p.z + 20.0).abs() < 1e-6);
        // Stationary target: the point itself.
        assert_eq!(
            lead_point(Vec3::new(3.0, 0.0, -10.0), Vec3::ZERO, 80.0).unwrap(),
            Vec3::new(3.0, 0.0, -10.0)
        );
        // Target faster than the bolt, running away: unreachable.
        assert!(lead_point(Vec3::NEG_Z * 10.0, Vec3::NEG_Z * 100.0, 80.0).is_none());
    }

    #[test]
    fn stopping_early_counts_the_rays_as_counting_them_all_does() {
        for (dirs, min) in [(&ENCLOSE_UP[..], 2), (&ENCLOSE_SIDE[..], ENCLOSE_SIDE_MIN)] {
            for mask in 0u32..1 << dirs.len() {
                let test = |d: &Vec3| {
                    let k = dirs.iter().position(|o| o == d).unwrap();
                    mask >> k & 1 == 1
                };
                let all = dirs.iter().filter(|d| test(d)).count() >= min;
                assert_eq!(at_least(dirs, min, test), all, "{mask:b}");
            }
        }
    }

    /// Card VMESH: a point inside a closed box mesh, and inside one open
    /// below (a hollow trunk), is enclosed; outside, under a lone roof and
    /// between two walls it is not. Only the closed box is watertight.
    #[test]
    fn enclosing_finds_closed_and_open_bottomed_shells_only() {
        use crate::collision::box_mesh;
        let half = Vec3::new(3.0, 2.0, 4.0);
        let closed = box_mesh(Vec3::new(0.0, 2.0, 0.0), half, false);
        let trunk = box_mesh(Vec3::new(20.0, 2.0, 0.0), half, true);
        assert!(closed.closed && !trunk.closed, "watertight");
        let roof = Body::with_layers(
            glam::Mat4::from_translation(Vec3::new(40.0, 3.0, 0.0)),
            Shape::Box {
                half: Vec3::new(3.0, 0.1, 3.0),
            },
            Layers::SOLDIER,
        );
        let w = CollisionWorld::new(vec![closed, trunk, roof], Vec::new());
        let at = |x: f32, z: f32| w.enclosing(Vec3::new(x, 1.0, z));
        assert_eq!(at(0.0, 0.0), Some(0));
        assert_eq!(at(2.5, -3.5), Some(0), "near a corner");
        assert_eq!(at(20.0, 0.0), Some(1), "open below");
        assert_eq!(at(4.0, 0.0), None, "beside it");
        assert_eq!(at(40.0, 0.0), None, "under a roof (not a mesh)");
        assert_eq!(w.enclosing(Vec3::new(0.0, 4.5, 0.0)), None, "over it");
        // A canopy over it (a box open below, raised): the rays up cross
        // once, none sideways.
        let canopy = box_mesh(Vec3::new(0.0, 4.0, 0.0), Vec3::new(3.0, 1.5, 3.0), true);
        let w = CollisionWorld::new(vec![canopy], Vec::new());
        assert_eq!(
            w.enclosing(Vec3::new(0.0, 1.0, 0.0)),
            None,
            "under a canopy"
        );
        assert_eq!(
            w.enclosing(Vec3::new(0.0, 3.5, 0.0)),
            Some(0),
            "in the canopy"
        );
        // A sealed room with walls of two faces: every ray crosses twice.
        let room = crate::collision::boxes_mesh(
            Vec3::new(0.0, 3.0, 0.0),
            &[(Vec3::new(4.0, 3.0, 4.0), 0), (Vec3::new(3.5, 2.5, 3.5), 0)],
        );
        let w = CollisionWorld::new(vec![room], Vec::new());
        assert_eq!(
            w.enclosing(Vec3::new(0.0, 1.0, 0.0)),
            None,
            "in a two-faced room"
        );
        assert_eq!(
            w.enclosing(Vec3::new(3.75, 1.0, 0.0)),
            Some(0),
            "in its wall"
        );
        // A tunnel open at both ends: the rays up cross once, the two along
        // it none.
        let tunnel = crate::collision::boxes_mesh(
            Vec3::new(0.0, 2.0, 0.0),
            &[(Vec3::new(12.0, 2.0, 2.0), crate::collision::BOX_X_ENDS)],
        );
        let w = CollisionWorld::new(vec![tunnel], Vec::new());
        assert_eq!(w.enclosing(Vec3::new(0.0, 1.0, 0.0)), None, "in a tunnel");
        // A mesh roof alone: one ray of nine crosses it.
        let mut roof_mesh = box_mesh(Vec3::new(0.0, 3.0, 0.0), Vec3::new(3.0, 0.1, 3.0), false);
        roof_mesh.closed = false;
        let w = CollisionWorld::new(vec![roof_mesh], Vec::new());
        assert_eq!(w.enclosing(Vec3::new(0.0, 1.0, 0.0)), None, "under a slab");
    }
}
