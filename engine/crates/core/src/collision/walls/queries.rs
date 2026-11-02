//! The soldier-vs-wall queries: [`CollisionWorld::push_out`], the push out of
//! every wall a capsule penetrates, and [`CollisionWorld::wall_sweep`], the
//! first wall a step would sweep into — with the Phase A.4 penetration test
//! ([`penetration_block`]) and the parry 0.22.1 contact workaround
//! ([`mesh_contact`]) that the module header in `collision.rs` describes.

use super::super::shape::{isometry_of, Aabb, Body, Capsule, Layers, Shape};
use super::super::world::{CollisionWorld, HullFilter};
use super::super::{point, vector, Isometry, TriMesh, WALKABLE_MIN_NY};
use glam::Vec3;

/// PROVISIONAL (ours): capsule-vs-world push-out passes per tick.
pub const WALL_ITERATIONS: u32 = 2;
/// PROVISIONAL (ours), card WPSH: the longest XZ sub-step of
/// [`CollisionWorld::push_out_soldier_from`], in capsule radii. From a pose
/// touching a sheet, a step under one radius leaves the axis on the near
/// side; half a radius keeps a margin for the push's own slide.
pub const SUBSTEP_OF_RADIUS: f32 = 0.5;
/// PROVISIONAL (ours), card WPSH: at most this many sub-steps a tick (a
/// 2 m move at the 0.25 m sub-step). Anything longer is not a step but a
/// teleport; all sub-steps remain equal and can exceed the nominal limit.
pub const MAX_SUBSTEPS: u32 = 8;
/// PROVISIONAL (ours): the one tolerance [`CollisionWorld::wall_sweep`] has,
/// in metres. It does three jobs, all of them the same 1 mm:
///
/// 1. A cast hit NEARER than this is not a wall ahead but a body the capsule
///    already TOUCHES or is INSIDE OF, and the cast's answer for it is thrown
///    away — that body is decided by the penetration test instead
///    ([`penetration_block`]).
/// 2. It splits TOUCHING from PENETRATING in that test: deeper than this and
///    the start contact's own normal is trusted as the way out; shallower and
///    the Phase A.1 point test at the end pose decides.
/// 3. It is the tolerance of that test's depth comparison — a step may not
///    deepen a penetration by MORE than this.
///
/// The wall pass leaves a unit EXACTLY touching whatever it pushed it out of,
/// and a shape cast from a touching pose returns a hit at (float) zero
/// distance for every direction that does not strictly separate — a purely
/// TANGENTIAL one included. Measured: a capsule touching a wall at x = 1.5
/// reports a hit 6.2e-5 m along +Z, parallel to the wall, with normal
/// (-0.998, 0, -0.059) — a real wall normal, so no closing-velocity threshold
/// separates it from a genuine graze without becoming a tuning knob. The point
/// test does: "would the capsule be INSIDE this body at the end of the step"
/// is exact, and it is the rule Phase A.1 was measured against. From INSIDE a
/// body the cast is worse than uninformative — it reports zero in every
/// direction, out of the body included (measured 2026-09-03; see
/// [`CollisionWorld::wall_sweep`]) — which is why job 1 covers that case too.
///
/// It only has to be bigger than the float dust in that zero and smaller than
/// anything the steering must react to; 1 mm is four orders above the measured
/// 6.2e-5 m and three below a tick of walking (0.3 m). As job 3 it is also
/// what lets a unit walk ALONG a wall it is inside: over the AI's 1 m probe it
/// admits every direction within 0.06 deg of tangential.
pub const SWEEP_CONTACT_DIST: f32 = 1e-3;

/// The highest point of the triangle `t` whose XZ lies
/// within `r` of `c`, or `None` when the triangle's footprint misses the disc.
/// Height is linear over the triangle, so the maximum is at a vertex inside
/// the disc, where an edge crosses the circle, or (a leaning triangle) where
/// the circle is furthest up the triangle's own slope. The ledge rule's
/// "below the step where the capsule is" (card SKIRT, [`ledge_contact`]).
fn top_in_disc(t: [Vec3; 3], c: Vec3, r: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    let mut take = |y: f32| best = Some(best.map_or(y, |b: f32| b.max(y)));
    let d2 = |p: Vec3| (p.x - c.x) * (p.x - c.x) + (p.z - c.z) * (p.z - c.z);
    for p in t {
        if d2(p) <= r * r {
            take(p.y);
        }
    }
    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
        // |a + s (b - a) - c|^2 = r^2 in XZ
        let (ex, ez) = (b.x - a.x, b.z - a.z);
        let (fx, fz) = (a.x - c.x, a.z - c.z);
        let qa = ex * ex + ez * ez;
        if qa < 1e-12 {
            continue;
        }
        let qb = 2.0 * (fx * ex + fz * ez);
        let qc = fx * fx + fz * fz - r * r;
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            continue;
        }
        let sq = disc.sqrt();
        for s in [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] {
            if (0.0..=1.0).contains(&s) {
                take(a.y + (b.y - a.y) * s);
            }
        }
    }
    // The circle's point furthest up the slope, when the triangle holds it.
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    if n.y.abs() > 1e-6 {
        // y = y0 - (n.x (x - x0) + n.z (z - z0)) / n.y: the gradient in XZ.
        let g = Vec3::new(-n.x / n.y, 0.0, -n.z / n.y);
        let gl = (g.x * g.x + g.z * g.z).sqrt();
        if gl > 1e-6 {
            let p = Vec3::new(c.x + g.x / gl * r, 0.0, c.z + g.z / gl * r);
            // barycentric in XZ
            let (v0x, v0z) = (t[1].x - t[0].x, t[1].z - t[0].z);
            let (v1x, v1z) = (t[2].x - t[0].x, t[2].z - t[0].z);
            let (v2x, v2z) = (p.x - t[0].x, p.z - t[0].z);
            let den = v0x * v1z - v1x * v0z;
            if den.abs() > 1e-9 {
                let u = (v2x * v1z - v1x * v2z) / den;
                let v = (v0x * v2z - v2x * v0z) / den;
                if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
                    take(t[0].y + (t[1].y - t[0].y) * u + (t[2].y - t[0].y) * v);
                }
            }
        }
    }
    best
}

/// The world box `world` in `body`'s own space, as a parry AABB — the query
/// box for that body's mesh BVH ([`parry3d::partitioning::Bvh::intersect_aabb`]).
/// Bounds of the 8 transformed corners, so it is conservative for a rotated
/// instance, which is what a broad phase wants.
fn local_query_aabb(world: Aabb, body: &Body) -> parry3d::bounding_volume::Aabb {
    let local = Aabb::transformed(world, body.to_local);
    parry3d::bounding_volume::Aabb::new(point(local.min), point(local.max))
}

/// The deepest contact between the capsule (`cap_shape` placed by the WORLD
/// isometry `cap_iso`) and `mesh` as placed by `body`, or `None` when they do
/// not touch. Points and normals come back in WORLD space.
///
/// WORKAROUND for the parry 0.22.1 composite-contact bug — see the module
/// header. This is `CompositeShapeRef::contact_with_shape`'s own loop with the
/// pose the bug inverts: BVH candidates over the capsule's world bounds mapped
/// into body space, then parry's `contact` per TRIANGLE (a `Triangle` is
/// convex, not composite, so that call is correct), keeping the DEEPEST (most
/// negative `dist`). With `walls_first` (the wall pass) the deepest WALL-like
/// contact (|n.y| < [`WALKABLE_MIN_NY`]) wins over any deeper floor- or
/// ceiling-like one: the pass answers one contact per body, and an
/// overhang's underside deeper than the wall under it skipped the whole body
/// (card WTUN: BF1 BES1 body 26, underside 0.24 m, wall 0.07 m, walked
/// through by 95 census runs at one site).
///
/// Determinism: the same rule as everywhere else in this module — a strict `<`
/// in the BVH's TRAVERSAL ORDER, so the first candidate wins a tie, and that
/// order is fixed for a fixed mesh (the tree is built once, in [`trimesh`],
/// and never rebuilt). Nothing here is transcendental.
///
/// With `start` (the mover's axis segment at the start of its step, card
/// WPSH) a triangle whose contact would push the axis to the FAR side of
/// its face is not a candidate ([`pushes_past`]).
fn mesh_contact<S: parry3d::shape::Shape>(
    mesh: &TriMesh,
    body: &Body,
    cap_shape: &S,
    cap_iso: &Isometry,
    start: Option<(Vec3, Vec3)>,
    walls_first: bool,
) -> Option<parry3d::query::Contact> {
    let a = parry3d::shape::Shape::compute_aabb(cap_shape, cap_iso);
    let world = Aabb {
        min: Vec3::new(a.mins.x, a.mins.y, a.mins.z),
        max: Vec3::new(a.maxs.x, a.maxs.y, a.maxs.z),
    };
    let query = local_query_aabb(world, body);
    let mut best: Option<parry3d::query::Contact> = None;
    let mut best_wall: Option<parry3d::query::Contact> = None;
    for i in mesh.bvh().intersect_aabb(&query) {
        let tri = mesh.triangle(i);
        let c = triangle_contact(cap_iso, cap_shape, &body.iso, &tri);
        let Some(c) = c else { continue };
        if start.is_some_and(|s| pushes_past(body, &tri, &c, s)) {
            continue;
        }
        if walls_first
            && c.dist < 0.0
            && c.normal2.y.abs() < WALKABLE_MIN_NY
            && best_wall.is_none_or(|b| c.dist < b.dist)
        {
            best_wall = Some(c);
        }
        if best.is_none_or(|b| c.dist < b.dist) {
            best = Some(c);
        }
    }
    best_wall.or(best)
}

/// PROVISIONAL (ours), card WPSH (`analysis/re-soldier-walls.md` §4.1): would
/// the contact `c` with the mesh triangle `tri` (in `body`'s space) push the
/// mover's axis to the FAR side of that triangle's face?
///
/// `start` is the axis segment (bottom and top sphere centres, world) at the
/// START of the step. When the whole segment lay on one side of the face's
/// plane, the soldier came from that side; a horizontal push along `c`'s
/// normal that points to the other side is not a way out of the triangle but
/// a way through it, and the pass must not take it. A segment that already
/// straddled the plane at the start says nothing, and the contact stands.
///
/// The case (the LEAK census's KAS2 control, the cp5 hut): a jump at the
/// flared, double-skinned `kas2_bldg_platform_roof` wall. The outer skin
/// leans out over the soldier (n.y −0.54, a ceiling, then skipped), so the capsule
/// rises into the flare until its axis pierces the INNER skin, triangle 5,
/// from behind. That triangle's contact is the body's deepest (0.544 m,
/// normal (0.303, 0, −0.953)), and it pushed the soldier 0.84 m into the hut.
/// A zero-thickness sheet stepped half through (M2) is the same case.
fn pushes_past(
    body: &Body,
    tri: &parry3d::shape::Triangle,
    c: &parry3d::query::Contact,
    start: (Vec3, Vec3),
) -> bool {
    let Some(face) = tri.normal() else {
        return false;
    };
    let f = body.iso.rotation * face.into_inner();
    let f = Vec3::new(f.x, f.y, f.z);
    let on = body
        .to_world
        .transform_point3(Vec3::new(tri.a.x, tri.a.y, tri.a.z));
    let (s0, s1) = ((start.0 - on).dot(f), (start.1 - on).dot(f));
    let side = if s0 > 0.0 && s1 > 0.0 {
        1.0
    } else if s0 < 0.0 && s1 < 0.0 {
        -1.0
    } else {
        return false; // already straddling the plane at the start
    };
    // Only a start IN FRONT OF (or behind) the triangle itself: the point of
    // the start segment at the triangle's mid height, projected on its plane,
    // inside it. Beside the triangle, the soldier is behind its plane's
    // EXTENSION, not the face (a neighbouring face of a corner or a crevice),
    // and its contact stands; dropping it trapped soldiers in BF2 MYG1's
    // crevices, pushed to and fro each tick (census `float` 43 -> 150).
    let w = |p: parry3d::na::Point3<f32>| body.to_world.transform_point3(Vec3::new(p.x, p.y, p.z));
    let (a, b, cc) = (on, w(tri.b), w(tri.c));
    let mid = (a.y + b.y + cc.y) / 3.0;
    let (lo, hi) = if start.0.y <= start.1.y {
        start
    } else {
        (start.1, start.0)
    };
    let p = if hi.y - lo.y > 1e-6 {
        lo.lerp(hi, ((mid - lo.y) / (hi.y - lo.y)).clamp(0.0, 1.0))
    } else {
        lo
    };
    let q = p - f * (p - a).dot(f);
    // No margin: widening the prism by 0.25 or 0.5 m saves 14-40 of the
    // census's pushes on KAS2/DAG1/MYG1 and brings MYG1's crevice trap back.
    let inside = |u: Vec3, v: Vec3| (v - u).cross(q - u).dot(f) >= 0.0;
    if !(inside(a, b) && inside(b, cc) && inside(cc, a)) {
        return false;
    }
    // The push is the contact normal's horizontal part (`push_out_with`).
    let push = Vec3::new(c.normal2.x, 0.0, c.normal2.z);
    side * push.dot(f) < 0.0
}

/// PROVISIONAL (ours): how far the ledge rule's HORIZONTAL escape
/// ([`ledge_escape`]) may push. A soldier deeper than this inside a
/// primitive was inside before the rule existed (an old replay, a spawn); he
/// is left where he is rather than teleported across a lobe.
const LEDGE_ESCAPE_MAX: f32 = 1.5;

/// A LEDGE's answer for [`CollisionWorld::push_out_soldier`], as
/// `(unit normal out of the body, depth)`, from the FULL capsule `full` (its
/// parry shape `fshape` placed by `fiso`) against `body`.
///
/// A penetrating WALL-like contact (|n.y| < [`WALKABLE_MIN_NY`]) is the
/// answer. Otherwise the result is `None` (no contact, or only touching),
/// except for one case. A floor-like full contact means the shortest way out
/// is UP. That is a real step or landing when the ground query can stand the
/// soldier on the top. For a primitive whose top it cannot reach, it is
/// [`ledge_escape`]'s horizontal way out instead.
///
/// A primitive is convex, so parry's `contact` is the whole answer. A MESH
/// is asked per triangle, over the triangles that are WALLS by their own FACE
/// normal (rotated to world; instances are rigid) and that reach above the
/// step (a world vertex above `full.base.y + lift`) inside the capsule's
/// footprint ([`top_in_disc`]). A floor triangle under the feet, or a kerb's
/// riser below the step where the capsule is, cannot answer for the ledge.
/// Among those, the deepest wall-like contact wins, with a strict `<` in the
/// BVH's traversal order, as in [`mesh_contact`], and with its `start` rule
/// ([`pushes_past`]) on the FULL capsule's start segment.
fn ledge_contact(
    body: &Body,
    full: &Capsule,
    fshape: &parry3d::shape::Capsule,
    fiso: &Isometry,
    start: Option<(Vec3, Vec3)>,
) -> Option<(Vec3, f32)> {
    let wall = |c: &parry3d::query::Contact| c.dist < 0.0 && c.normal2.y.abs() < WALKABLE_MIN_NY;
    let answer =
        |c: parry3d::query::Contact| (Vec3::new(c.normal2.x, c.normal2.y, c.normal2.z), -c.dist);
    match &body.shape {
        Shape::Mesh(mesh) => {
            let step_top = full.base.y + super::super::STEP_HEIGHT.min(full.height * 0.5);
            let query = local_query_aabb(full.aabb(), body);
            let mut best: Option<parry3d::query::Contact> = None;
            for i in mesh.bvh().intersect_aabb(&query) {
                let tri = mesh.triangle(i);
                let Some(face) = tri.normal() else { continue };
                if (body.iso.rotation * face.into_inner()).y.abs() >= WALKABLE_MIN_NY {
                    continue; // a floor TRIANGLE is not the ledge's wall
                }
                let top = [tri.a, tri.b, tri.c]
                    .iter()
                    .map(|p| body.to_world.transform_point3(Vec3::new(p.x, p.y, p.z)).y)
                    .fold(f32::MIN, f32::max);
                if top <= step_top {
                    continue; // below the step: a kerb, climbed, not a wall
                }
                // ...and below the step WHERE THE CAPSULE IS — the triangle's
                // highest point inside the capsule's footprint — is a kerb there
                // too, whatever it rises to further on (card SKIRT: the riser
                // beside BF2 Kamino's pod skirt, two vertical triangles 1.9 m
                // high whose low end the soldier walks up beside, took 6 of his
                // 7 m/s in one tick; re-soldier-ground.md §6-7).
                let w = |p: parry3d::na::Point3<f32>| {
                    body.to_world.transform_point3(Vec3::new(p.x, p.y, p.z))
                };
                if top_in_disc([w(tri.a), w(tri.b), w(tri.c)], full.base, full.radius)
                    .is_none_or(|t| t <= step_top)
                {
                    continue;
                }
                let c = triangle_contact(fiso, fshape, &body.iso, &tri);
                let Some(c) = c.filter(wall) else { continue };
                if start.is_some_and(|s| pushes_past(body, &tri, &c, s)) {
                    continue; // card WPSH, as in `mesh_contact`
                }
                if best.is_none_or(|b| c.dist < b.dist) {
                    best = Some(c);
                }
            }
            best.map(answer)
        }
        _ => {
            let c = body.shape.with_parry(&mut |s| {
                parry3d::query::contact(fiso, fshape, &body.iso, s, 0.0)
                    .ok()
                    .flatten()
            })?;
            if c.dist >= 0.0 {
                return None;
            }
            if wall(&c) {
                return Some(answer(c));
            }
            ledge_escape(body, full)
        }
    }
}

/// The HORIZONTAL way out of an upright primitive (a `Cylinder`, or a `Box`
/// turned only about Y) whose top is beyond the ground query's reach
/// (`top − feet > STEP_HEIGHT`), when the full capsule's own contact says
/// "up". `None` for anything else (a sphere, a tilted box such as a stair
/// ramp, a top within the step, no horizontal overlap, or deeper than
/// [`LEDGE_ESCAPE_MAX`]).
///
/// The case it serves (re-soldier-walls §2, the City Hall residue after the
/// ledge rule): a jump reaches a lobe's rim at its apex. The stepped
/// capsule's round bottom clears the rim in one tick without touching it,
/// and one tick later the full capsule is 0.3 m inside, where the shortest
/// way out of the solid is up (0.61) rather than out (0.81). The ground query
/// cannot lift him 0.61 m, so up is no way out at all. Radially out of a
/// cylinder, or out through the nearest side face of a box, is.
fn ledge_escape(body: &Body, full: &Capsule) -> Option<(Vec3, f32)> {
    let up = body.to_world.transform_vector3(Vec3::Y);
    if up.y < 0.9999 {
        return None; // not upright
    }
    let lift = super::super::STEP_HEIGHT.min(full.height * 0.5);
    if body.aabb.max.y - full.base.y <= lift {
        return None; // the top is within the step: the ground query's
    }
    let l = body.to_local.transform_point3(full.base);
    let r = full.radius;
    let (dir, escape) = match &body.shape {
        Shape::Cylinder { radius, .. } => {
            let d = (l.x * l.x + l.z * l.z).sqrt();
            if d < 1e-6 {
                return None;
            }
            (Vec3::new(l.x / d, 0.0, l.z / d), radius + r - d)
        }
        Shape::Box { half } => {
            let ex = half.x + r - l.x.abs();
            let ez = half.z + r - l.z.abs();
            if ex <= ez {
                (Vec3::new(if l.x < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0), ex)
            } else {
                (Vec3::new(0.0, 0.0, if l.z < 0.0 { -1.0 } else { 1.0 }), ez)
            }
        }
        _ => return None,
    };
    if !(escape > 0.0 && escape <= LEDGE_ESCAPE_MAX) {
        return None;
    }
    Some((body.to_world.transform_vector3(dir), escape))
}

impl CollisionWorld {
    /// PROVISIONAL (ours): the horizontal push that moves `cap` out of every
    /// wall it penetrates. The PS2's soldier-vs-world response is untraced, so
    /// this is a plain iterated push-out along the contact normal, with the
    /// vertical component dropped — floors are the ground query's job and a
    /// ceiling-like contact pushes only by its normal's horizontal part (card
    /// WTUN). Returns the total push (y is exactly 0).
    ///
    /// A SOLDIER query: bodies outside [`Layers::SOLDIER`] are skipped (RE §4).
    ///
    /// Order matters here beyond the tie-breaks: pushes are ACCUMULATED in
    /// ascending `bodies` order, which is the order the broad phase offers.
    /// Contacts use one pose snapshot per iteration; the next iteration
    /// asks the poses moved by the accumulated pushes.
    ///
    /// This is the bare pass, with no LEDGE rule: the sim's soldiers go
    /// through [`Self::push_out_soldier`].
    pub fn push_out(&self, cap: &Capsule) -> Vec3 {
        self.push_out_with(cap, None, None, HullFilter::All)
    }

    /// PROVISIONAL (ours): the soldier's wall pass (card JWAL,
    /// `analysis/re-soldier-walls.md` §7). It is [`Self::push_out`] of the
    /// STEPPED capsule, `full.stepped()`, so a kerb under `STEP_HEIGHT` stays
    /// a thing to climb, plus the LEDGE rule.
    ///
    /// The bare pass skips every contact whose normal has
    /// |n.y| ≥ [`WALKABLE_MIN_NY`] as "a floor, the ground query's job". But
    /// an UPWARD contact on the stepped capsule is a surface ABOVE the step,
    /// which the ground query (its probe starts at feet + `STEP_HEIGHT`) can
    /// never stand the soldier on. Such a contact is a ledge's rim under the
    /// lifted capsule's lower sphere.
    ///
    /// Skipping it let a jump carry a BF1 soldier over the rim of a KAS2 City
    /// Hall lobe (a solid prim, 2.25 m tall), and let a walk pass through a
    /// 0.6–0.75 m box. The soldier then fell inside the solid onto the
    /// terrain (re-soldier-walls §2). The PS2 lets neither happen (§5.1,
    /// PCSX2).
    ///
    /// So a LEDGE is re-asked with the FULL capsule, `full`
    /// ([`ledge_contact`]), and its wall-like contact pushes as a wall does. A
    /// floor-like full contact is a real step or landing (the feet within the
    /// penetration of the top), which the ground query lifts him onto.
    /// Downward contacts (ceilings) push only by their horizontal part (card
    /// WTUN, in [`Self::push_out`]). A body the stepped capsule
    /// does not penetrate at all is asked the same ledge question: a riser a
    /// few cm over the step slips under the lifted capsule's round bottom.
    pub fn push_out_soldier(&self, full: &Capsule) -> Vec3 {
        self.push_out_with(&full.stepped(), Some(*full), None, HullFilter::All)
    }

    /// [`Self::push_out_soldier`] against the vehicles' HULLS alone: the AI's
    /// probe for a parked vehicle's low parts, which the steering's sweep
    /// (the stepped capsule) passes over and this pass pushes back from
    /// (card STFAL, `analysis/re-round-stalls.md` C8).
    pub fn push_out_soldier_hulls(&self, full: &Capsule) -> Vec3 {
        self.push_out_with(&full.stepped(), Some(*full), None, HullFilter::Hulls)
    }

    /// PROVISIONAL (ours): the soldier's wall pass for a tick that moved him
    /// from `from` to `full` (card WPSH, `analysis/re-soldier-walls.md` §4):
    /// [`Self::push_out_soldier`] along the move, with two rules the pose
    /// alone cannot give.
    ///
    /// - SUB-STEPS. A `coll` mesh wall is a zero-thickness sheet. Once a
    ///   tick's step carries the axis past it, the deepest contact points to
    ///   the far side and the push completes the crossing. From a pose
    ///   touching the wall that takes a step over the radius: a BF2
    ///   sprint-jump (17.55 m/s, 0.585 m a tick) passed a 4 m sheet in 46 of
    ///   51 timings. So the move is cut into equal sub-steps of at most
    ///   [`SUBSTEP_OF_RADIUS`] radii in XZ, each followed by the pass, and
    ///   the pushes carry forward. A walk (0.22 m a tick in BF1) is one step,
    ///   as before; a sprint or a jump is two or three.
    /// - NO PUSH THROUGH A FACE ([`pushes_past`]). Each (sub-)step's start
    ///   pose says which side of every mesh face the soldier came from; a
    ///   triangle contact that would push him to the other side is dropped.
    ///
    /// [H] The PS2 tests the soldier SWEPT from its previous position C+0x198
    /// to C+0x228 (§5.3: a RAM teleport that leaves C+0x198 behind falls
    /// through the world). The sub-steps are the port's stand-in for that
    /// sweep; the PS2's own test is not read.
    ///
    /// Returns the total push (y exactly 0), to be added to `full.base`.
    pub fn push_out_soldier_from(&self, from: &Capsule, full: &Capsule) -> Vec3 {
        let d = full.base - from.base;
        let dxz = (d.x * d.x + d.z * d.z).sqrt();
        let limit = full.radius * SUBSTEP_OF_RADIUS;
        let n = if limit > 0.0 {
            ((dxz / limit).ceil() as u32).clamp(1, MAX_SUBSTEPS)
        } else {
            1
        };
        let mut total = Vec3::ZERO;
        let mut at = Capsule {
            base: from.base,
            ..*full
        };
        for k in 1..=n {
            let start = at;
            let mut now = *full;
            now.base = from.base + d * (k as f32 / n as f32) + total;
            total += self.push_out_with(&now.stepped(), Some(now), Some(start), HullFilter::All);
            at = now;
            at.base = from.base + d * (k as f32 / n as f32) + total;
        }
        total
    }

    /// [`Self::push_out`] of `cap`, with the LEDGE rule when `full` (the
    /// unstepped capsule `cap` was lifted from) is given. Contacts in each
    /// iteration use fixed capsule snapshots; pushes move both capsules for
    /// the next iteration. `start` is
    /// the full capsule at the start of the step, for [`pushes_past`].
    fn push_out_with(
        &self,
        cap: &Capsule,
        full: Option<Capsule>,
        start: Option<Capsule>,
        hulls: HullFilter,
    ) -> Vec3 {
        let mut total = Vec3::ZERO;
        let mut cap = *cap;
        let mut full = full;
        let start_seg = start.map(|s| s.stepped().segment());
        let start_full_seg = start.map(|s| s.segment());
        for _ in 0..WALL_ITERATIONS {
            let mut moved = false;
            let (shape, iso) = cap.to_parry();
            let whole = full.map(|f| (f, f.to_parry()));
            let cap_aabb = cap.aabb();
            self.visit_xz(
                cap_aabb.min.x,
                cap_aabb.max.x,
                cap_aabb.min.z,
                cap_aabb.max.z,
                hulls,
                |body| {
                    if !body.layers.contains(Layers::SOLDIER) {
                        return;
                    }
                    if !body.aabb.overlaps(&cap_aabb) {
                        return;
                    }
                    let c = match &body.shape {
                        // A MESH is composite: parry's own `contact` would test the
                        // capsule at a mirrored pose (module header), so ask the
                        // triangles ourselves.
                        Shape::Mesh(mesh) => {
                            mesh_contact(mesh, body, &shape, &iso, start_seg, true)
                        }
                        // Primitives are convex, not composite — parry's `contact`
                        // is correct for them.
                        _ => body.shape.with_parry(&mut |s| {
                            parry3d::query::contact(&iso, &shape, &body.iso, s, 0.0)
                                .ok()
                                .flatten()
                        }),
                    };
                    // Touching is not penetrating.
                    // `normal2` points out of shape 2 (the body), i.e. at the
                    // capsule — the direction the capsule has to move.
                    let stepped = c
                        .filter(|c| c.dist < 0.0)
                        .map(|c| (Vec3::new(c.normal2.x, c.normal2.y, c.normal2.z), -c.dist));
                    let (n, depth) = match stepped {
                        Some((n, depth)) if n.y.abs() < WALKABLE_MIN_NY => (n, depth),
                        // A ceiling-like contact (n.y ≤ −WALKABLE_MIN_NY): the
                        // pass has no vertical response (card VCOL), but its
                        // HORIZONTAL part still pushes, NOT renormalised
                        // (card WTUN). A slab leaning over the soldier, an
                        // overhang's lower edge at his head: skipping it let
                        // him walk through the face under it (BF2 DAG1's
                        // tilted boxes, 59 census runs at one site). A flat
                        // ceiling's contact is almost straight down and
                        // pushes almost nothing.
                        Some((n, depth)) if n.y < 0.0 => {
                            (n, depth * (n.x * n.x + n.z * n.z).sqrt())
                        }
                        // Floor-like upward: the bare pass skips it; a
                        // soldier's LEDGE is asked again with the full capsule
                        // (`push_out_soldier`). And when the stepped capsule is
                        // clear, the FULL one may still be in a wall whose top
                        // is just over the step: the lifted capsule's round
                        // bottom passes over the edge (a 0.62 m riser, BF2
                        // KAM1), the ground query cannot climb it, and a tick
                        // later the axis is past the face. The ledge rule
                        // answers for that too.
                        _ => {
                            let Some((f, (fshape, fiso))) = whole.as_ref() else {
                                return;
                            };
                            let Some(ledge) = ledge_contact(body, f, fshape, fiso, start_full_seg)
                            else {
                                return;
                            };
                            ledge
                        }
                    };
                    let n_xz = Vec3::new(n.x, 0.0, n.z);
                    let len = n_xz.length();
                    if len < 1e-6 {
                        return;
                    }
                    let push = n_xz / len * depth;
                    cap.base += push;
                    if let Some(f) = full.as_mut() {
                        f.base += push;
                    }
                    total += push;
                    moved = true;
                },
            );
            if !moved {
                break;
            }
        }
        total
    }

    /// Every body in `layers` a sphere penetrates, in ascending body order:
    /// `(depth, unit normal out of the body)` — the deepest contact per
    /// body (meshes per triangle, as [`Self::push_out`]). The hover's spring
    /// and hull spheres (`crate::vehicle::hover`) read the world through it,
    /// so it sees the map's bodies only ([`HullFilter::Static`]: no
    /// vehicle meets another's hull yet, card VHULL).
    pub fn sphere_contacts(
        &self,
        centre: Vec3,
        radius: f32,
        layers: Layers,
        out: &mut Vec<(f32, Vec3)>,
    ) {
        let ball = parry3d::shape::Ball::new(radius);
        let iso = Isometry::translation(centre.x, centre.y, centre.z);
        let aabb = Aabb {
            min: centre - Vec3::splat(radius),
            max: centre + Vec3::splat(radius),
        };
        self.collect_contacts(&ball, &iso, &aabb, layers, out);
    }

    /// [`Self::sphere_contacts`] for a box or a cylinder placed by `to_world`
    /// (rigid): a hover's hull body against the buildings (card HHUL,
    /// re-hover §13). PROVISIONAL: parry's exact contact stands in for the
    /// PS2's prim-vs-body narrowphase (FUN_0016bf10, not transcribed). A
    /// sphere or a mesh mover finds nothing here.
    pub fn shape_contacts(
        &self,
        shape: &Shape,
        to_world: glam::Mat4,
        layers: Layers,
        out: &mut Vec<(f32, Vec3)>,
    ) {
        let iso = isometry_of(to_world);
        match *shape {
            Shape::Box { half } => self.convex_contacts(
                &parry3d::shape::Cuboid::new(vector(half)),
                &iso,
                layers,
                out,
            ),
            Shape::Cylinder {
                radius,
                half_height,
            } => self.convex_contacts(
                &parry3d::shape::Cylinder::new(half_height, radius),
                &iso,
                layers,
                out,
            ),
            Shape::Sphere { .. } | Shape::Mesh(_) => {}
        }
    }

    fn convex_contacts<S: parry3d::shape::Shape>(
        &self,
        mover: &S,
        iso: &Isometry,
        layers: Layers,
        out: &mut Vec<(f32, Vec3)>,
    ) {
        let a = parry3d::shape::Shape::compute_aabb(mover, iso);
        let aabb = Aabb {
            min: Vec3::new(a.mins.x, a.mins.y, a.mins.z),
            max: Vec3::new(a.maxs.x, a.maxs.y, a.maxs.z),
        };
        self.collect_contacts(mover, iso, &aabb, layers, out);
    }

    /// Shared narrowphase; callers retain their own exact query bounds.
    fn collect_contacts<S: parry3d::shape::Shape>(
        &self,
        mover: &S,
        iso: &Isometry,
        aabb: &Aabb,
        layers: Layers,
        out: &mut Vec<(f32, Vec3)>,
    ) {
        self.visit_xz(
            aabb.min.x,
            aabb.max.x,
            aabb.min.z,
            aabb.max.z,
            HullFilter::Static,
            |body| {
                if !body.layers.contains(layers) || !body.aabb.overlaps(aabb) {
                    return;
                }
                let c = match &body.shape {
                    Shape::Mesh(mesh) => mesh_contact(mesh, body, mover, iso, None, false),
                    _ => body.shape.with_parry(&mut |s| {
                        parry3d::query::contact(iso, mover, &body.iso, s, 0.0)
                            .ok()
                            .flatten()
                    }),
                };
                let Some(c) = c.filter(|c| c.dist < 0.0) else {
                    return;
                };
                out.push((-c.dist, Vec3::new(c.normal2.x, c.normal2.y, c.normal2.z)));
            },
        );
    }

    /// PROVISIONAL (ours): the first WALL the capsule would SWEEP into moving
    /// `dist` metres along `dir_xz` — `(distance to the hit, unit XZ push
    /// normal)`, or `None` when the whole sweep is clear. The normal is the
    /// direction the capsule would have to move to get out, so it points back
    /// at the mover.
    ///
    /// TWO TESTS PER BODY, each answering what the other cannot (Phase A.4):
    ///
    /// (a) THE CAST — parry's LINEAR shape cast (`query::cast_shapes`, the
    /// capsule moving at unit speed along `dir_xz` against a static body, so
    /// the returned time of impact IS a distance in metres). It catches a wall
    /// the step passes THROUGH, which is why A.2 replaced the A.1 point probe:
    /// a wall thinner than the gap the step jumped — every BF2 partition and
    /// railing — sat between the unit and the probe and was never touched,
    /// while the wall pass pushed the unit back every tick (PLAN "PHASE A.2
    /// DESIGN"). It runs with the same AABB prefilter as [`Self::push_out`]
    /// widened to the SWEPT capsule's bounds, the same [`WALKABLE_MIN_NY`]
    /// floor filter (a hit within 60 deg of vertical is a floor, not a wall),
    /// and `stop_at_penetration`.
    ///
    /// (b) THE PENETRATION TEST — a body the capsule already TOUCHES or is
    /// INSIDE OF, which the cast cannot judge (below); it is decided by
    /// penetration DEPTH instead. A step may keep or reduce a penetration —
    /// walking ALONG a wall, or back OUT of one — but it may never deepen one,
    /// and it may not end inside a body it started clear of (that half is the
    /// Phase A.1 point test, which the whole steering layer was measured
    /// against). See [`penetration_block`] for the rule and
    /// [`deepened_by_step`] for the two exact comparisons.
    ///
    /// (b) OWNS every body whose cast reports nothing usable — no hit at all,
    /// or a hit inside [`SWEEP_CONTACT_DIST`] — and only those: a usable cast
    /// hit means the capsule is CLEAR of that body at the start of the step
    /// (a cast from an overlapping pose reports zero), so the cast's distance
    /// is the honest answer and (b) is not asked. A (b) block is reported at
    /// `t = 0` and so beats any (a) hit under the strict `<` below.
    ///
    /// WHY the cast cannot judge a body the capsule is inside (measured
    /// 2026-09-03 on the fixtures of the two A.4 tests, and read in parry3d
    /// 0.22.1): `cast_shapes` on a penetrating pose returns
    /// `time_of_impact = 0.0` for EVERY direction — including straight back
    /// out — with the SAME normal each time. `gjk::directional_distance`
    /// (`query/gjk/gjk.rs`) ray-casts the ray's origin against the Minkowski
    /// difference; with the origin INSIDE it, `project_origin_and_reduce`
    /// gives the zero vector, `Unit::try_new_and_get` fails and it returns
    /// `(ltoi = 0, ldir)` whatever the direction. `cast_shapes_support_map_-`
    /// `support_map` then takes its `time_of_impact < 1.0e-5` branch and fills
    /// the hit in from `contact_support_map_support_map`, i.e. from the
    /// direction-INDEPENDENT contact. So the cast answers "you are inside",
    /// never "which way".
    ///
    /// WHY the old rule then got it wrong. Inside [`SWEEP_CONTACT_DIST`] the
    /// A.2 code asked the A.1 point test — does the capsule PENETRATE at the
    /// END of the step? — which is exact for a body it is merely touching and
    /// wrong for one it is already inside, in both directions:
    /// - a 118 deg rotated `Box { half (4, 4, 9.8) }`, capsule (h 1.9, r 0.5)
    ///   with its feet at y 2.9 pushed 0.1 … 1.0 m into a side face, stepping
    ///   1.0 m FURTHER IN (`ai::PROBE_DIST`): the end pose is deeper than the
    ///   capsule's feet are below the box's TOP, so the deepest contact's
    ///   minimum-translation direction flips to (0, 1, 0) — a FLOOR — and the
    ///   whole body was skipped. `wall_sweep` answered `None` INTO the wall at
    ///   every depth while `push_out` reported the penetration exactly. That
    ///   is BF2 MYG1 unit 6, 53,400 ticks of `steer fwd` with `push 0.300`
    ///   0.33 m inside `myg1_bldg_large_platform_b : p_-v-rob`.
    /// - a `coll` MESH wall is a zero-thickness sheet: a capsule 0.1 … 0.45 m
    ///   inside one, stepping 1.0 m further in, ends CLEAN THROUGH it with no
    ///   contact at all, so INTO the wall answered `None` — while stepping
    ///   ALONG it, where the end pose is still inside, answered blocked. The
    ///   wrong way round in both directions.
    ///
    /// Phase A.3 — THE FIRST NON-FLOOR HIT PER MESH BODY, unchanged except
    /// that its own touching sub-branch is gone with the body one (a triangle
    /// hit inside [`SWEEP_CONTACT_DIST`] is ignored; those bodies belong to
    /// (b)). One `cast_shapes` per body answers for a body whose hit is a
    /// WALL. But a body whose whole-body hit is a FLOOR used to contribute
    /// nothing at all, so a wall standing behind a ramp, a lip or a sloped
    /// floor OF THE SAME MESH was never seen: on BF1 NAB2 ten units sat at
    /// y −49 reporting `steer fwd` while the wall pass pushed them back 0.30 m
    /// every tick (PLAN Log, "Phase A.2 landed"). So ONLY when the whole-body
    /// answer is a floor and the body is a [`Shape::Mesh`], the mesh is
    /// re-asked PER TRIANGLE: the earliest hit over its NON-FLOOR triangles,
    /// where floor-vs-wall is decided by the triangle's own FACE normal
    /// (rotated into world space — instances are rigid) and the returned push
    /// normal is still the CONTACT normal. Candidates come from the mesh's BVH
    /// over the swept AABB in body-local space.
    ///
    /// PRIMITIVES (`Sphere`, `Cylinder`, `Box`) get no per-face pass in (a):
    /// they are convex, so a sweep enters them once and the one contact is the
    /// whole answer. A graze along a primitive's TOP EDGE reports a
    /// floor-like normal. That graze is a LEDGE (card JWAL,
    /// `analysis/re-soldier-walls.md`): the swept capsule is the stepped one,
    /// so the primitive stands above the step, and it blocks like a wall. It
    /// used to skip the box, and a bot's probe called a 0.75 m crate open,
    /// which is the rule [`Self::push_out_soldier`] now shares. (b)'s
    /// primitive branch reads a floor-like upward normal the same way.
    ///
    /// `dir_xz`'s `y` is ignored; the sweep is horizontal, like the push.
    ///
    /// The AI ([`crate::ai`]) uses it to pick a direction it can actually walk
    /// in, which the PS2 does NOT do — the originals steer around authored
    /// barriers plus a contact push (RE_LOG "Barriers are the PS2's
    /// Obstacles…", local avoidance) — so this is the port's own answer for
    /// the model geometry the barriers do not cover.
    ///
    /// Determinism: bodies in ascending `bodies` order with a strict `<` on
    /// the time of impact, so the FIRST body wins a tie, as everywhere else
    /// in this module; that covers the (b) blocks too, which all tie at `t = 0`.
    /// Inside the two per-triangle passes the same strict comparison runs in
    /// the BVH's TRAVERSAL ORDER, which is fixed for a fixed mesh (the tree is
    /// built once, in `trimesh`, and never rebuilt), so the first candidate
    /// wins a tie there. Nothing on either path is transcendental: the cast
    /// reaches `cast_shapes_support_map_support_map` (GJK's
    /// `directional_distance`, and EPA only when the shapes already overlap),
    /// reached for a mesh through a BVH descent whose node cost is an AABB ray
    /// test; (b)'s `query::contact` reaches, for the four shapes this world
    /// holds, `contact_support_map_support_map` (`Cylinder`, `Cuboid` and a
    /// mesh's `Triangle` — GJK plus EPA on overlap) and
    /// `contact_convex_polyhedron_ball` (`Sphere`, which projects the ball's
    /// centre on the capsule) — all add/mul/compare plus `sqrt`. The `sin` /
    /// `cos` in parry's `Cuboid` and `Segment` are in
    /// `ConvexPolyhedron::support_feature_toward`, which is the contact-
    /// MANIFOLD path and is on neither of these (PLAN's P9 audit found
    /// `query/` clean; re-checked against parry3d 0.22.1's sources, contact
    /// paths included, 2026-09-03). The triangle face normal adds nothing
    /// transcendental either — a cross product and a normalisation.
    ///
    /// A SOLDIER query: bodies outside [`Layers::SOLDIER`] are skipped (RE §4).
    pub fn wall_sweep(&self, cap: &Capsule, dir_xz: Vec3, dist: f32) -> Option<(f32, Vec3)> {
        self.wall_sweep_body(cap, dir_xz, dist)
            .map(|(t, n, _)| (t, n))
    }

    /// [`Self::wall_sweep`], and the body that answered: for a diagnostic
    /// that names what a bot is walking into (the playtest black box, card
    /// BLKB). The sim asks only for the time and the normal.
    pub fn wall_sweep_body(
        &self,
        cap: &Capsule,
        dir_xz: Vec3,
        dist: f32,
    ) -> Option<(f32, Vec3, &Body)> {
        self.wall_sweep_small(cap, dir_xz, dist, f32::INFINITY)
    }

    /// [`Self::wall_sweep_body`] against only the bodies whose bounds are at
    /// most `max_half` from their middle along X and along Z: what a bot
    /// that looks ahead for a SMALL body asks (`ai::avoid`), without paying
    /// for a cast against every floor and wall the look crosses.
    pub fn wall_sweep_small(
        &self,
        cap: &Capsule,
        dir_xz: Vec3,
        dist: f32,
        max_half: f32,
    ) -> Option<(f32, Vec3, &Body)> {
        if dist <= 0.0 {
            return None;
        }
        let flat = Vec3::new(dir_xz.x, 0.0, dir_xz.z);
        let len = flat.length();
        if len < 1e-6 {
            return None;
        }
        let dir = flat / len;
        let (shape, iso) = cap.to_parry();
        // The prefilter box is the union of the capsule's bounds at both ends
        // of the sweep (the motion is a pure translation, so that encloses it).
        let here = cap.aabb();
        let step = dir * dist;
        let swept = Aabb {
            min: here.min.min(here.min + step),
            max: here.max.max(here.max + step),
        };
        let vel = vector(dir);
        let still = vector(Vec3::ZERO);
        let opts = parry3d::query::ShapeCastOptions {
            max_time_of_impact: dist,
            target_distance: 0.0,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        };
        // The pose at the END of the step, for the penetration test below.
        let mut end = *cap;
        end.base += step;
        let end_iso = end.to_parry().1;
        // (b)'s own broad phase: the capsule's bounds at the two ENDS of the
        // step, each grown by the contact tolerance. A body that lies between
        // the two poses without touching either is the CAST's business, and
        // the cast sees it correctly — the capsule is clear of it at the start.
        let grow = |a: Aabb| Aabb {
            min: a.min - Vec3::splat(SWEEP_CONTACT_DIST),
            max: a.max + Vec3::splat(SWEEP_CONTACT_DIST),
        };
        let start_box = grow(here);
        let end_box = grow(end.aabb());
        let mut best: Option<(f32, Vec3)> = None;
        // Whose answer `best` is (it changes only where `best` does).
        let mut who: Option<&Body> = None;
        self.visit_xz(
            swept.min.x,
            swept.max.x,
            swept.min.z,
            swept.max.z,
            HullFilter::All,
            |body| {
                if !body.layers.contains(Layers::SOLDIER) {
                    return;
                }
                if !body.aabb.overlaps(&swept) {
                    return;
                }
                let size = body.aabb.max - body.aabb.min;
                if 0.5 * size.x.max(size.z) > max_half {
                    return;
                }
                let hit = body.shape.with_parry(&mut |s| {
                    parry3d::query::cast_shapes(&iso, &vel, &shape, &body.iso, &still, s, opts)
                        .ok()
                        .flatten()
                });
                let usable = hit.filter(|h| h.time_of_impact >= SWEEP_CONTACT_DIST);
                let Some(hit) = usable else {
                    // (b): the cast has nothing usable for this body — the capsule
                    // is touching it, inside it, or nowhere near it. Depth decides.
                    if best.is_some_and(|(t, _)| t <= 0.0) {
                        return; // strict `<`: the first body wins a tie at t = 0
                    }
                    if !body.aabb.overlaps(&start_box) && !body.aabb.overlaps(&end_box) {
                        return;
                    }
                    let n = penetration_block(
                        body, &shape, &iso, start_box, &end_iso, end_box, swept, dir, dist,
                    );
                    let Some(n) = n else { return };
                    best = Some((0.0, n));
                    who = Some(body);
                    return;
                };
                if best.is_some_and(|(t, _)| hit.time_of_impact >= t) {
                    return; // strict `<`: the first body wins a tie
                }
                // `normal1` is the outward normal ON THE CAPSULE, in the capsule's
                // own frame — and `Capsule::to_parry` places it by a pure
                // TRANSLATION, so that frame is the world's. It points at the
                // body, so the way out is its negation (`push_out`'s `normal2`,
                // without needing the body transform).
                let n = Vec3::new(-hit.normal1.x, -hit.normal1.y, -hit.normal1.z);
                if n.y.abs() < WALKABLE_MIN_NY {
                    // A WALL: the whole-body answer, exactly as before.
                    let Some(n) = xz_unit(n) else { return };
                    best = Some((hit.time_of_impact, n));
                    who = Some(body);
                    return;
                }
                // A FLOOR. For a primitive that is the whole answer (convex: one
                // entry contact). An UPWARD one is a LEDGE (card JWAL, the rule of
                // `push_out_soldier`): the swept capsule is the stepped one, so the
                // primitive reaches above the step, where the soldier cannot stand.
                // It blocks like a wall. A downward one (an overhang) stays skipped.
                // For a MESH it only means the nearest triangle of
                // this body is walkable — a wall of the same mesh may stand behind
                // it — so re-ask the mesh per triangle. This is the only path that
                // costs more than it did, and it runs only on a floor hit.
                let Shape::Mesh(mesh) = &body.shape else {
                    if n.y > 0.0 {
                        if let Some(n) = xz_unit(n) {
                            best = Some((hit.time_of_impact, n));
                            who = Some(body);
                        }
                    }
                    return;
                };
                // The swept AABB in the mesh's own space (its BVH's space).
                let local_swept = local_query_aabb(swept, body);
                let mut tri_best: Option<(f32, Vec3)> = None;
                for i in mesh.bvh().intersect_aabb(&local_swept) {
                    let tri = mesh.triangle(i);
                    // The FACE normal decides floor-vs-wall, in WORLD space; the
                    // instance is rigid, so rotating the local normal is enough.
                    // A degenerate triangle has none and is not a wall.
                    let Some(face) = tri.normal() else { continue };
                    let face_y = (body.iso.rotation * face.into_inner()).y;
                    if face_y.abs() >= WALKABLE_MIN_NY {
                        continue; // a floor TRIANGLE is not a wall
                    }
                    // The TRIANGLE is shape 1 and the capsule shape 2, then the
                    // hit is swapped back — the argument order parry's own
                    // composite cast uses internally (it inverts the pose and
                    // negates the velocity before visiting the parts). Measured:
                    // with the capsule first instead, the same wall as ONE mesh
                    // and as TWO bodies answers 0.8000001 and 0.7998484 — the
                    // same arithmetic in a different frame, 1.5e-4 apart. This
                    // order makes the per-triangle answer BIT-IDENTICAL to the
                    // whole-body one (the test below asserts it), so how the bake
                    // grouped triangles into bodies cannot change the answer.
                    let th = parry3d::query::cast_shapes(
                        &body.iso, &still, &tri, &iso, &vel, &shape, opts,
                    )
                    .ok()
                    .flatten()
                    .map(|h| h.swapped());
                    // A hit inside the contact distance is the cast talking about
                    // a triangle the capsule already touches, which it cannot
                    // judge: (b) owns those, as for a whole body.
                    let Some(th) = th.filter(|h| h.time_of_impact >= SWEEP_CONTACT_DIST) else {
                        continue;
                    };
                    if tri_best.is_some_and(|(t, _)| th.time_of_impact >= t) {
                        continue; // strict `<`: the first candidate wins a tie
                    }
                    // The push direction is the CONTACT normal, as for a body.
                    let tn = Vec3::new(-th.normal1.x, -th.normal1.y, -th.normal1.z);
                    let Some(tn) = xz_unit(tn) else { continue };
                    tri_best = Some((th.time_of_impact, tn));
                }
                // The body's result competes with the others exactly as before.
                let Some((t, tn)) = tri_best else { return };
                if best.is_some_and(|(bt, _)| t >= bt) {
                    return;
                }
                best = Some((t, tn));
                who = Some(body);
            },
        );
        best.zip(who).map(|((t, n), b)| (t, n, b))
    }
}

/// `n` flattened to XZ and normalised, or `None` when it has no horizontal
/// part — the sweep and the push are horizontal.
fn xz_unit(n: Vec3) -> Option<Vec3> {
    let f = Vec3::new(n.x, 0.0, n.z);
    let l = f.length();
    (l >= 1e-6).then(|| f / l)
}

/// Does a step of `dist` along `dir` DEEPEN this contact pair's penetration?
/// `(penetration depth at the end of the step, the way out)` when it does.
/// The two arguments are the contacts between the same capsule and the same
/// convex piece — a primitive body, or one triangle of a mesh — at the START
/// and END poses of the step; `None` for either means "not touching there".
///
/// PROVISIONAL (ours), Phase A.4. Two regimes, split at
/// [`SWEEP_CONTACT_DIST`]:
///
/// - ALREADY PENETRATING at the start (`pen0 > SWEEP_CONTACT_DIST`): the depth
///   at the end is measured ALONG THE START CONTACT'S OWN NORMAL,
///   `pen0 − dist·(dir·n0)`, not by asking parry for a second contact. Both
///   other readings are measured to be wrong here: parry's cast reports
///   `t = 0` in every direction from inside (see [`CollisionWorld::wall_sweep`]),
///   and the end-pose contact answers about a DIFFERENT escape direction —
///   for a deep box the minimum translation flips to vertical, and for a
///   zero-thickness mesh wall a step clean through it reports no contact at
///   all, so a pure depth comparison would call both "not deepening". Along
///   `n0` the comparison is exact and needs no new constant: at 1 m (the AI's
///   `PROBE_DIST`) the 1 mm tolerance admits every direction within 0.06 deg
///   of tangential, which is what "may walk ALONG a wall it is inside" means.
/// - NOT penetrating at the start (touching, or clear): the Phase A.1 point
///   test — the end pose must not be inside the body — with the same 1 mm
///   tolerance. This is the case the whole steering layer was measured
///   against, and the case a cast from a touching pose cannot judge (it
///   reports a float-zero hit in every non-separating direction, including a
///   purely tangential one; see [`SWEEP_CONTACT_DIST`]).
///
/// The normal returned is the one the comparison was made with — the START
/// contact's in the first regime, the END contact's in the second — and it
/// points out of the body, at the mover (`push_out`'s `normal2`).
///
/// `end` is asked only in the second regime: a capsule deep in a dense mesh
/// otherwise pays an EPA for an end contact that cannot change the answer.
fn deepened_by_step(
    start: Option<parry3d::query::Contact>,
    end: impl FnOnce() -> Option<parry3d::query::Contact>,
    dir: Vec3,
    dist: f32,
) -> Option<(f32, Vec3)> {
    let depth = |c: Option<parry3d::query::Contact>| c.map_or(0.0, |c| (-c.dist).max(0.0));
    let pen0 = depth(start);
    let out = |c: parry3d::query::Contact| Vec3::new(c.normal2.x, c.normal2.y, c.normal2.z);
    let (pen1, n) = if pen0 > SWEEP_CONTACT_DIST {
        let n = out(start?);
        (pen0 - dist * dir.dot(n), n)
    } else {
        let c = end()?;
        ((-c.dist).max(0.0), out(c))
    };
    (pen1 > pen0 + SWEEP_CONTACT_DIST).then_some((pen1, n))
}

/// One contact's inputs, by exact bits: the capsule's half height, radius and
/// centre (`Capsule::to_parry` makes it Y-aligned and places it by a pure
/// translation), the triangle, and the body's pose. A contact is a pure function
/// of these, so an entry cannot go stale whatever moves in the world.
pub(super) type StartContactKey = ([u32; 5], [u32; 9], [u32; 7]);

/// A bot's steering pick asks a dozen sweeps from the one pose, one per heading.
/// The start contact of a triangle is the same for all of them, and from inside
/// a dense mesh each is an EPA. Kept only inside
/// [`CollisionWorld::remembering_start_contacts`] (the navigation flood asks
/// each pose once and must not pay for this) and only when penetrating (the
/// others are cheap). Only ever looked up: nothing iterates it.
#[derive(Default)]
pub(super) struct StartContacts {
    pub(super) on: bool,
    pub(super) deep: std::collections::HashMap<StartContactKey, parry3d::query::Contact>,
}

thread_local! {
    static EPA_BUFFERS: std::cell::RefCell<parry3d::query::epa::EPA> =
        std::cell::RefCell::new(parry3d::query::epa::EPA::new());
}

/// `parry3d::query::contact(iso, shape, tri_iso, tri, 0.0)`, without its
/// allocations. A shape inside a mesh triangle falls back to EPA, and parry
/// makes a new EPA, with empty lists, for every triangle: on BF2 Kashyyyk that
/// was over half of a tick's allocations. These are parry 0.22.1's own steps
/// (`contact_support_map_support_map_with_params`) with one EPA kept per
/// thread; `closest_points` resets it first, so it holds nothing between calls.
/// A ball, or a shape with no support map, takes parry's own path.
pub(super) fn triangle_contact(
    iso: &Isometry,
    shape: &dyn parry3d::shape::Shape,
    tri_iso: &Isometry,
    tri: &parry3d::shape::Triangle,
) -> Option<parry3d::query::Contact> {
    use parry3d::query::gjk::{self, CSOPoint, GJKResult, VoronoiSimplex};
    let support = shape.as_support_map().filter(|_| {
        shape.as_ball().is_none() && shape.as_shape::<parry3d::shape::HalfSpace>().is_none()
    });
    let Some(support) = support else {
        return parry3d::query::contact(iso, shape, tri_iso, tri, 0.0)
            .ok()
            .flatten();
    };
    let pos12 = iso.inv_mul(tri_iso);
    let dir = parry3d::na::Unit::try_new(pos12.translation.vector, parry3d::math::DEFAULT_EPSILON)
        .unwrap_or_else(parry3d::math::Vector::x_axis);
    let mut simplex = VoronoiSimplex::new();
    simplex.reset(CSOPoint::from_shapes(&pos12, support, tri, &dir));
    let closest = match gjk::closest_points(&pos12, support, tri, 0.0, true, &mut simplex) {
        GJKResult::Intersection => {
            EPA_BUFFERS.with_borrow_mut(|epa| epa.closest_points(&pos12, support, tri, &simplex))
        }
        GJKResult::ClosestPoints(a, b, n) => Some((a, b, n)),
        _ => None,
    };
    let (point1, point2_1, normal1) = closest?;
    let dist = (point2_1 - point1).dot(&normal1);
    let point2 = pos12.inverse_transform_point(&point2_1);
    let normal2 = pos12.inverse_transform_unit_vector(&-normal1);
    let mut c = parry3d::query::Contact::new(point1, point2, normal1, normal2, dist);
    c.transform_by_mut(iso, tri_iso);
    Some(c)
}

thread_local! {
    pub(super) static START_CONTACTS: std::cell::RefCell<StartContacts> = std::cell::RefCell::default();
}

fn f32_bits<const N: usize>(v: [f32; N]) -> [u32; N] {
    v.map(f32::to_bits)
}

impl CollisionWorld {
    /// Run `f`, whose mesh sweeps share the start contacts they compute.
    /// Changes no answer, only how often parry is asked.
    pub fn remembering_start_contacts<R>(&self, f: impl FnOnce() -> R) -> R {
        START_CONTACTS.with_borrow_mut(|memo| memo.on = true);
        let r = f();
        START_CONTACTS.with_borrow_mut(|memo| {
            memo.on = false;
            memo.deep.clear();
        });
        r
    }
}

/// `contact` of a capsule with a mesh triangle at a sweep's start pose: see
/// [`StartContacts`].
fn start_contact(
    shape: &parry3d::shape::Capsule,
    iso: &Isometry,
    body: &Body,
    tri: &parry3d::shape::Triangle,
    contact: impl FnOnce() -> Option<parry3d::query::Contact>,
) -> Option<parry3d::query::Contact> {
    START_CONTACTS.with_borrow_mut(|memo| {
        let t = iso.translation.vector;
        let (b, q) = (body.iso.translation.vector, body.iso.rotation);
        let key: StartContactKey = (
            f32_bits([t.x, t.y, t.z, shape.half_height(), shape.radius]),
            f32_bits([
                tri.a.x, tri.a.y, tri.a.z, tri.b.x, tri.b.y, tri.b.z, tri.c.x, tri.c.y, tri.c.z,
            ]),
            f32_bits([b.x, b.y, b.z, q.i, q.j, q.k, q.w]),
        );
        if let Some(c) = memo.deep.get(&key) {
            return Some(*c);
        }
        let c = contact();
        if let Some(c) = c.filter(|c| c.dist < 0.0) {
            memo.deep.insert(key, c);
        }
        c
    })
}

/// Test (b) of [`CollisionWorld::wall_sweep`] for one body: the XZ push normal
/// when the step deepens a penetration of a WALL of it, else `None`. The
/// answer is always at `t = 0` — the capsule is touching or inside the body
/// NOW.
///
/// - Primitives (`Sphere` / `Cylinder` / `Box`) are convex, so one
///   [`parry3d::query::contact`] per pose is the whole answer (parry's own
///   `contact` is correct for a convex shape — module header), and the CONTACT
///   normal decides wall-vs-floor, as it does in [`Self::push_out`].
/// - A MESH is asked PER TRIANGLE, never deepest-of-mesh: the deepest contact
///   can be a floor triangle while a wall triangle is the one deepening (the
///   test `a_capsule_touching_a_wall_over_a_ramp_of_the_same_mesh_is_blocked`
///   is exactly that). Candidates are the mesh's BVH over the swept bounds in
///   body space; a triangle is a WALL by its own FACE normal rotated to world,
///   as in the A.3 cast pass; the contact per triangle is `query::contact` on
///   a `Triangle`, which is convex — never on the composite mesh (module
///   header). Among the triangles that block, the DEEPEST end-of-step
///   penetration wins, with a strict `>` in BVH traversal order.
///
/// Each pose's contact is skipped when the body's (or triangle's) own bounds
/// do not reach that pose's capsule box: a miss there is `None`, which is what
/// the contact would have returned anyway.
#[allow(clippy::too_many_arguments)]
fn penetration_block(
    body: &Body,
    shape: &parry3d::shape::Capsule,
    start_iso: &Isometry,
    start_box: Aabb,
    end_iso: &Isometry,
    end_box: Aabb,
    swept: Aabb,
    dir: Vec3,
    dist: f32,
) -> Option<Vec3> {
    match &body.shape {
        Shape::Mesh(mesh) => {
            use parry3d::bounding_volume::BoundingVolume;
            let local_start = local_query_aabb(start_box, body);
            let local_end = local_query_aabb(end_box, body);
            let remember = START_CONTACTS.with_borrow(|memo| memo.on);
            let mut best: Option<(f32, Vec3)> = None;
            for i in mesh.bvh().intersect_aabb(&local_query_aabb(swept, body)) {
                let tri = mesh.triangle(i);
                let Some(face) = tri.normal() else { continue };
                if (body.iso.rotation * face.into_inner()).y.abs() >= WALKABLE_MIN_NY {
                    continue; // a floor TRIANGLE is not a wall
                }
                let box_of = tri.local_aabb();
                let at = |iso: &Isometry| triangle_contact(iso, shape, &body.iso, &tri);
                let s = box_of
                    .intersects(&local_start)
                    .then(|| {
                        if remember {
                            start_contact(shape, start_iso, body, &tri, || at(start_iso))
                        } else {
                            at(start_iso)
                        }
                    })
                    .flatten();
                let e = || box_of.intersects(&local_end).then(|| at(end_iso)).flatten();
                let Some((pen1, n)) = deepened_by_step(s, e, dir, dist) else {
                    continue;
                };
                let Some(n) = xz_unit(n) else { continue };
                if best.is_some_and(|(p, _)| pen1 <= p) {
                    continue; // strict `>`: the first candidate wins a tie
                }
                best = Some((pen1, n));
            }
            best.map(|(_, n)| n)
        }
        _ => {
            let at = |iso: &Isometry| {
                body.shape.with_parry(&mut |s| {
                    parry3d::query::contact(iso, shape, &body.iso, s, 0.0)
                        .ok()
                        .flatten()
                })
            };
            let s = body
                .aabb
                .overlaps(&start_box)
                .then(|| at(start_iso))
                .flatten();
            let e = || body.aabb.overlaps(&end_box).then(|| at(end_iso)).flatten();
            let (_, n) = deepened_by_step(s, e, dir, dist)?;
            if n.y <= -WALKABLE_MIN_NY {
                // An overhang. `push_out` now pushes its horizontal part (card
                // WTUN), but a probe that blocks a unit under every low deck
                // it grazes would stall the AI; the push alone answers it.
                return None;
            }
            // A floor-like UPWARD normal is a LEDGE above the step (card JWAL,
            // as in `push_out_soldier` and the cast above): a wall.
            xz_unit(n)
        }
    }
}
