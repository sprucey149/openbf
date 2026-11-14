//! Sim-side collision: ray vs soldier capsule, ray vs heightfield (marched),
//! the flat-fire ballistic lead solve, and — from the walkable-model work —
//! the baked WORLD ([`CollisionWorld`]): the map's `coll` meshes, `prim`
//! primitives and terrain cutters, queried for the ground under a soldier's
//! feet and for the walls it may not walk through.
//!
//! The originals run these through the collision system (prim masks 0x48 /
//! 0x90 for bolts, 0x5e for aim-assist acquisition — layer semantics still
//! open, re-blaster-damage §10.7). Terrain is queried as a cutter-aware SHEET
//! ([`ray_heightfield`]) and soldiers as capsules — the analytic tests below
//! are cheaper and bit-deterministic; the world's meshes and primitives go
//! through `parry3d` (PLAN tech stack).
//!
//! Bolts and line of sight now read the world: [`CollisionWorld::ray`] is the
//! general (no walkability filter) ray that [`crate::projectile::Bolt::spawn`]
//! and [`crate::ai`] take alongside the terrain crossing, so a bolt stops at a
//! wall and the AI cannot see through one.
//!
//! ## Layer masks: soldier and ordnance
//!
//! Every [`Body`] carries a [`Layers`] mask saying which QUERIES it answers,
//! and the world holds the UNION of the two subsets that exist so far.
//! `analysis/re-collision-layers.md` §4: a soldier's walk / ground / wall
//! queries test the model's SOLDIER subset, while a ray with the trailing flag
//! — a bolt (`FUN_002764C0`), the reticle (`FUN_00204748`) and the AI line of
//! sight (`FUN_00162E88`) — tests only its ORDNANCE subset. So
//! [`CollisionWorld::ray_down`], [`CollisionWorld::push_out`] and
//! [`CollisionWorld::wall_sweep`] skip a body that is not in
//! [`Layers::SOLDIER`], and [`CollisionWorld::ray`] takes the mask it wants.
//! Which bodies of a model are in which subset is the class rule in
//! `game::collision_layers`; this module only obeys the mask it is handed.
//!
//! ## Vehicle hulls (card VHULL)
//!
//! A live vehicle's own bodies — its class's `coll` mesh and prims, in the
//! soldier / ordnance subsets its class's lists name — ride in the world at
//! its pose ([`CollisionWorld::set_hull`], kept by `Sim::sync_hulls`), apart
//! from the map's bodies and after them in every query's order. Each such
//! [`Body`] names its vehicle as `owner`; [`HullFilter`] says whose hulls a
//! query sees, and [`CollisionWorld::ray_with`] reports the owner it hit.
//!
//! The VEHICLE subset ([`Layers::VEHICLE`]) is built; a flyer's landing ray
//! queries it. A flyer's own contact and its pilot's probes query
//! [`Layers::FLYER`], which the class rule sets to the soldier subset or,
//! with `flyer_uses_vehicle`, the vehicle subset.
//! Walker/hover traffic uses the named SOLDIER-subset surrogate
//! in `traffic`; ordinary ground contacts still use [`HullFilter::Static`].
//! Also pending: the building/terrain subsets (nothing queries them), and
//! the acquisition mask discrepancy: the PS2 aim-assist acquisition ray
//! passes flag 0 and tests ALL bodies (RE §4, `FUN_00223850`), while
//! `Sim::acquire_magnet` currently uses `perception::los_seat`, which queries
//! [`Layers::ORDNANCE`] for both acquisition and the reticle pick.
//!
//! Formats and conventions of the baked data: `mungefs::coll` /
//! `mungefs::terrain` (measured by the lead — RE_LOG 2026-09-01 coll/prim/CUTR
//! measurement, Fable session 7).
//!
//! ## Determinism
//! - Bodies are visited in `bodies` order — ASCENDING INDEX, which since the
//!   broad phase (`world::BodyGrid`) is what its candidate lists are sorted into —
//!   and that is the manifest's instance order (`game::assets::Scene::collision_world`): base instances, then the
//!   merged mode layer's, and within an instance the `coll` mesh first, then
//!   the `prim` primitives in manifest order — skipping the bodies that are in
//!   NEITHER subset and those of a kind this port does not build. The
//!   SOLDIER-masked subsequence of that list is exactly the list the soldier
//!   world held before the ordnance subset was added. No HashMap is iterated.
//! - The layer mask is world data, like the geometry: it is NOT fed into
//!   `Sim::hash`.
//! - Triangle order inside a mesh is leaf order, then fan order
//!   (`mungefs::coll::CollisionMesh::triangles`).
//! - The broad phase (`world::BodyGrid`, built by [`CollisionWorld::new`]) only
//!   chooses WHICH bodies a query is offered, never what it answers about
//!   one: it hands over a superset of the bodies whose AABB meets the query's
//!   XZ footprint, in ascending index order, and every exact test that
//!   followed the old full scan still runs. A world without one scans
//!   everything, as before.
//! - parry's BVH returns the MIN toi / deepest contact regardless of traversal
//!   order, and ties between bodies are broken by a strict `<` so the FIRST
//!   body wins. The candidate list is sorted, by index; nothing else is. The three queries that walk a mesh's BVH
//!   THEMSELVES — [`CollisionWorld::wall_sweep`]'s per-triangle CAST pass
//!   (A.3), the per-triangle PENETRATION pass it now pairs with
//!   (`walls::queries::penetration_block`, A.4) and `walls::queries::mesh_contact` — break their ties in
//!   traversal order, which is fixed for a fixed mesh (built once in
//!   [`trimesh`], never rebuilt).
//! - The world is immutable after construction and is NOT fed into
//!   `Sim::hash` — it is an input, like the heightfield — except for the
//!   doors' movers and the vehicles' hulls, both placed from hashed state.
//!
//! ## parry 0.22.1: `contact` against a COMPOSITE shape tests a MIRRORED pose
//!
//! UPSTREAM BUG, worked around here (found 2026-09-02).
//! `parry3d 0.22.1`'s `src/query/contact/contact_composite_shape_shape.rs`
//! ends `contact_composite_shape_shape` with
//! `CompositeShapeRef(g1).contact_with_shape(dispatcher, &pose12.inverse(), g2, …)`
//! — but `contact_with_shape` already documents `pose12` as "`shape2`
//! positioned relative to `self`" and uses it that way
//! (`shape2.compute_aabb(pose12)`, then `part_pos1.inv_mul(pose12)` per part).
//! The extra `.inverse()` therefore tests the other shape at the INVERSE pose:
//! for a translation-only instance, the capsule POINT-MIRRORED through the
//! instance's origin; for a rotated one, also rotated the wrong way. So
//! `query::contact(cap_iso, capsule, body.iso, trimesh, …)` answers about a
//! PHANTOM copy of the building — a wall the unit is really inside of is
//! missed, and empty air where the mirror image lands pushes it. That is the
//! "the sweep says clear but the wall pass pushes 0.30 m every tick" stall.
//!
//! CONSEQUENCE FOR THIS MODULE: `parry3d::query::contact` is NEVER called on a
//! composite shape ([`Shape::Mesh`]) anywhere in `core`. Meshes go through
//! `walls::queries::mesh_contact`, which is `contact_with_shape`'s own loop with the pose the
//! bug inverts; primitives (`Ball`, `Cylinder`, `Cuboid`) are convex, not
//! composite, and keep parry's `contact`. Unaffected, and left alone: the
//! shape-CAST twin (`shape_cast_composite_shape_shape.rs`) passes `pos12`
//! straight through, so [`CollisionWorld::wall_sweep`]'s `cast_shapes` is
//! right, and the rays run `cast_local_ray` in body-local space themselves.
//!
//! parry's `master` already passes `pose12` straight through, so a release
//! will fix this and the workaround can then be dropped (delete
//! `walls::queries::mesh_contact`, call `contact` on the mesh again). THE DETECTOR is the
//! unit test `mesh_contact_is_not_mirrored`, which asserts that RAW parry is
//! still mirrored and fails loudly when it stops being. parry is deliberately
//! NOT upgraded here: PLAN's P9 audit pinned 0.22.1's transcendental-free
//! query paths, and an upgrade would move every hash for a second reason.
//!
//! Instance matrices are RIGID: measured over all 15,199 instances of the 32
//! baked maps, every stored 3×3 is orthonormal (worst row-length deviation
//! 1.1e-7) with a positive determinant. The wall pass relies on that to build a
//! parry `Isometry`; the ground ray does not (it works through the full
//! affine matrix).
//!
//! ## Where the code lives
//!
//! Split by queue SPL3 (2026-09-24, a pure move) into four parts, each
//! re-exported here so every `crate::collision::X` path is unchanged: `shape`
//! (the capsule, the AABB, a body's shape and the `TriMesh` builder, the layer
//! mask, the body and the terrain cutter), `world` (the `CollisionWorld` and
//! its broad phase), `rays` (the cutter test, the ground and world rays, the
//! terrain ray and the lead solve) and `walls` (the wall pass — `push_out` and
//! `wall_sweep` — and the parry contact workaround this header describes).
//! This file keeps the step and ground constants, the parry conversions every
//! part uses, and the test fixtures the parts' tests share.

mod ceilings;
pub mod cost;
mod prim_contact;
mod rays;
mod shape;
mod traffic;
mod walls;
mod world;

pub use prim_contact::{bounding_radius, PrimContact};
pub use rays::{is_cut_by, lead_point, ray_heightfield, ray_terrain_ps2, RayHit};
pub(crate) use shape::MeshDigests;
pub use shape::{trimesh, Aabb, Body, Capsule, Cutter, HullPart, Layers, Shape};
pub use walls::{SWEEP_CONTACT_DIST, WALL_ITERATIONS};
pub use world::{CollisionWorld, HullFilter};

use glam::Vec3;

pub use parry3d::shape::TriMesh;

/// PROVISIONAL (ours): how far above the feet the ground probe starts — the step a soldier
/// can climb in one tick. The PS2 has no such probe: its soldier is carried up a step by the
/// collision response's push-out (re-soldier-ground.md §1.3, §1.6).
pub const STEP_HEIGHT: f32 = 0.6;
/// PROVISIONAL (ours): the WALL PASS's step for a soldier in the AIR (the jump
/// and fall stances, off the floor) — what a jump can mantle over its apex.
/// The PS2 has no step at all there: a BF1 jump (apex 1.912 m) at KAS2's
/// 2.26 m lobe ends 0.35 m short of the top (re-soldier-walls.md §5.2). With
/// the whole [`STEP_HEIGHT`] a jump reached 2.5 m (card JARC). Under that
/// 0.35 m. The ground query keeps the whole step (it catches a floor the
/// feet dipped under within one tick's move: the leak census's `floor` class
/// rose by 640 runs without it), but for a face too steep to stand on.
pub const AIR_STEP: f32 = 0.3;
/// The PS2 soldier's foot: the sphere `coll_body_lower`, r 0.57 (re-soldier-ground.md §1.2).
pub const FOOT_RADIUS: f32 = 0.57;
/// PROVISIONAL (ours): the radius of the ground query's ring of rays, cast when the centre ray
/// finds no floor within a step (`Sim::ground_under`, card GRND). Inside [`FOOT_RADIUS`]: a
/// slit under 0.6 m is bridged as the sphere bridges it, and a ledge holds the feet 0.3 m out.
pub const FOOT_RING: f32 = 0.3;
/// The ring's eight directions (x, z), written out: no trig in the sim.
pub const FOOT_RING_DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (0.707_106_77, 0.707_106_77),
    (0.0, 1.0),
    (-0.707_106_77, 0.707_106_77),
    (-1.0, 0.0),
    (-0.707_106_77, -0.707_106_77),
    (0.0, -1.0),
    (0.707_106_77, -0.707_106_77),
];
/// PROVISIONAL (ours): how far a soldier's feet may be BELOW the heightfield
/// before the terrain stops being a ground candidate at all. The terrain is a
/// one-sided SHEET, not a solid: a base under an uncut heightfield (BF2 Polis
/// Massa's `cp5con`/`cp6con` sit 21 m and 34 m under the asteroid surface, and
/// none of POL1's 7 cutters contains them) is legitimately underground, and
/// snapping its occupants up onto the surface is what the old always-solid
/// rule did. Between `STEP_HEIGHT` and this the terrain still wins outright —
/// that is a soldier climbing a steep bank in one tick. The PS2's own rule is
/// untraced (re-terrain-heights.md covers the height query, not the stance
/// probe).
pub const TERRAIN_BURY: f32 = 2.0;
/// PROVISIONAL (ours): the down ray and the wall pass call a surface a floor when |normal.y| >= this
/// (60°); steeper is a wall. A soldier STANDS only above `soldier::FLOOR_MIN_NY` (0.6, the PS2's).
pub const WALKABLE_MIN_NY: f32 = 0.5;
/// Ground when nothing is below: −300.0 — the PS2's own "cut terrain" sentinel
/// (analysis/re-terrain-heights.md, FUN_00473db0 returns −300.0 for a cut
/// point) reused as the floor of a map with no terrain under a point.
/// PROVISIONAL as a no-terrain floor.
pub const NO_GROUND: f32 = -300.0;

type Isometry = parry3d::na::Isometry3<f32>;

fn point(v: Vec3) -> parry3d::na::Point3<f32> {
    parry3d::na::Point3::new(v.x, v.y, v.z)
}

fn vector(v: Vec3) -> parry3d::na::Vector3<f32> {
    parry3d::na::Vector3::new(v.x, v.y, v.z)
}

/// Card VMESH's mesh fixtures, for the other modules' tests.
#[cfg(test)]
pub(crate) use tests::{box_mesh, boxes_mesh, BOX_X_ENDS};

#[cfg(test)]
mod tests {
    //! The fixtures the parts' test modules share; each part's own tests
    //! live beside its code, in that part's `mod tests`.
    use super::*;
    use glam::Mat4;

    /// Card VMESH's fixture: an axis box `half` about `centre` as a triangle
    /// MESH (12 faces, 8 shared corners: watertight), or with its bottom face
    /// left out (`open_bottom`: a hollow trunk). SOLDIER layer, as a map prop.
    pub(crate) fn box_mesh(centre: Vec3, half: Vec3, open_bottom: bool) -> Body {
        boxes_mesh(centre, &[(half, if open_bottom { BOX_BOTTOM } else { 0 })])
    }

    /// [`boxes_mesh`]'s faces to leave out: the ends along x, and the bottom.
    pub(crate) const BOX_X_ENDS: u8 = 0b0_1100;
    pub(crate) const BOX_BOTTOM: u8 = 0b10_0000;

    /// [`box_mesh`] of several boxes about one centre in ONE mesh (a box in a
    /// box: a sealed room with walls of two faces), each with the faces its
    /// mask names left out (bits: -z, +z, -x, +x, top, bottom).
    pub(crate) fn boxes_mesh(centre: Vec3, boxes: &[(Vec3, u8)]) -> Body {
        let (mut v, mut idx) = (Vec::new(), Vec::<u32>::new());
        for &(half, open) in boxes {
            let base = v.len() as u32;
            let c = |x: f32, y: f32, z: f32| [x * half.x, y * half.y, z * half.z];
            v.extend([
                c(-1.0, -1.0, -1.0),
                c(1.0, -1.0, -1.0),
                c(1.0, 1.0, -1.0),
                c(-1.0, 1.0, -1.0),
                c(-1.0, -1.0, 1.0),
                c(1.0, -1.0, 1.0),
                c(1.0, 1.0, 1.0),
                c(-1.0, 1.0, 1.0),
            ]);
            let quads = [
                [0, 3, 2, 1], // -z
                [4, 5, 6, 7], // +z
                [0, 4, 7, 3], // -x
                [1, 2, 6, 5], // +x
                [3, 7, 6, 2], // top
                [0, 1, 5, 4], // bottom
            ];
            idx.extend(
                quads
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| open & (1 << k) == 0)
                    .map(|(_, q)| q)
                    .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
                    .map(|i| base + i),
            );
        }
        Body::with_layers(
            glam::Mat4::from_translation(centre),
            Shape::Mesh(trimesh(&v, &idx).expect("a box mesh")),
            Layers::SOLDIER,
        )
    }

    /// One body holding every triangle, at identity.
    pub(super) fn world_from_triangles(tris: Vec<[Vec3; 3]>) -> CollisionWorld {
        CollisionWorld::new(vec![mesh_body(&tris)], Vec::new())
    }

    pub(super) fn mesh_body(tris: &[[Vec3; 3]]) -> Body {
        mesh_body_at(tris, Mat4::IDENTITY)
    }

    /// The same, PLACED — an instance matrix, as the bake gives one.
    pub(super) fn mesh_body_at(tris: &[[Vec3; 3]], to_world: Mat4) -> Body {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for t in tris {
            let b = positions.len() as u32;
            for p in t {
                positions.push([p.x, p.y, p.z]);
            }
            indices.extend([b, b + 1, b + 2]);
        }
        Body::new(
            to_world,
            Shape::Mesh(trimesh(&positions, &indices).expect("mesh")),
        )
    }

    /// A `half`-sized square floor at height `y`, as two triangles.
    pub(super) fn floor_quad(y: f32, half: f32) -> Vec<[Vec3; 3]> {
        let p = |x: f32, z: f32| Vec3::new(x, y, z);
        vec![
            [p(-half, -half), p(half, -half), p(half, half)],
            [p(-half, -half), p(half, half), p(-half, half)],
        ]
    }

    /// A big triangle on the plane `y = at_x0 + tan(deg)·x`.
    pub(super) fn ramp(deg: f32, at_x0: f32) -> Vec<[Vec3; 3]> {
        let t = deg.to_radians().tan();
        vec![[
            Vec3::new(-10.0, at_x0 - 10.0 * t, -10.0),
            Vec3::new(10.0, at_x0 + 10.0 * t, -10.0),
            Vec3::new(0.0, at_x0, 10.0),
        ]]
    }

    /// A `half`-thick slab across z, its near face at `z + half`.
    pub(super) fn slab(z: f32) -> Body {
        Body::new(
            Mat4::from_translation(Vec3::new(0.0, 0.0, z)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        )
    }
}
