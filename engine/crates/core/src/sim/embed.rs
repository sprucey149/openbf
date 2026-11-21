//! Issue #182, the EMBED ORACLE: one read-only answer to "is this unit
//! somewhere a unit must not be?" — inside a solid, shut in a shell, under
//! the ground, overlapping a body, or off the playable level.
//!
//! The leak census (`game/examples/leak_census.rs`), `round_to_outcome`'s
//! `SWEEP_EMBED` sweep and any later runtime rescue share it, so a site one
//! of them finds is a site the others can count. It reads the sim only: no
//! state, no randomness, no hash moves. Every query is a collision or
//! heightfield query in fixed body order, so it is deterministic.
//!
//! The classes, tested in this order (the first that holds wins):
//! - [`Embed::InPrim`]: a point of the soldier's axis inside a SOLDIER-layer
//!   primitive (box, cylinder, sphere) — [`crate::collision::CollisionWorld::inside_primitive`];
//! - [`Embed::InClosedMesh`] / [`Embed::InOpenShell`]: shut inside a mesh by
//!   ray parity where no nav hub of the map is ([`Sim::shut_in`]), the body
//!   watertight or not;
//! - [`Embed::UnderTerrain`]: the feet more than [`EMBED_UNDER`] under an
//!   UNCUT terrain with no SOLDIER face between them and the sheet (a real
//!   tunnel or base, such as BF2 Polis Massa's, has one);
//! - [`Embed::Overlap`]: the stance's STEPPED capsule (the shape the wall pass
//!   resolves) penetrates a map body by more than [`EMBED_OVERLAP`];
//! - [`Embed::OffLevel`]: [`Sim::level_at`] says off.
//!
//! All of it is OURS (engineering, no RE): the PS2 has no such query. The
//! thresholds are PROVISIONAL; measure: the census's false-positive count at
//! nav hubs and `SWEEP_EMBED`'s per-class count over normal bot rounds
//! (issue #182 leak census v2, A4).

use super::Sim;
use crate::collision::Capsule;
use crate::fmath::FloatExt;
use crate::soldier::Stance;
use crate::unit::{capsule_height, UnitHandle, SOLDIER_RADIUS};
use glam::Vec3;

/// PROVISIONAL (ours): feet further than this under an uncut terrain with
/// nothing overhead are under the map (m). Card TERR's bank climb allows
/// 2 m before the ground query drops the sheet (`TERRAIN_BURY`); a foot a
/// few cm under a bank is the terrain's bilinear step, not a leak.
/// measure: the census's hub control on BF1/BF2 Hoth.
pub const EMBED_UNDER: f32 = 0.3;
/// PROVISIONAL (ours): a penetration of the stepped capsule deeper than this
/// (m) is an overlap. The wall pass leaves a unit touching (mm), sub-steps
/// cut 0.25 m. measure: as [`EMBED_UNDER`].
pub const EMBED_OVERLAP: f32 = 0.05;
/// Points of the soldier's axis tested against primitives (m over the feet),
/// the leak census's crossing heights; one at mid-capsule too, so a prone
/// soldier is tested at all.
const AXIS_HEIGHTS: [f32; 3] = [0.8, 1.25, 1.7];
/// The parity probe's height over the feet, capped at mid-capsule: the
/// AI's own `SHUT_PROBE_UP` (1 m), kept here so the census's inputs stay
/// outside `core::ai` (engine/goldens/leak_deps.toml).
const SHUT_UP: f32 = 1.0;

/// Where a unit is against the world's solids ([`Sim::embed_at`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Embed {
    /// Nowhere a unit must not be.
    None,
    /// An axis point inside this primitive body.
    InPrim(usize),
    /// Shut inside this watertight mesh body.
    InClosedMesh(usize),
    /// Shut inside this open mesh shell (a room or cave nobody walks in).
    InOpenShell(usize),
    /// Under the uncut terrain by this much (m), nothing overhead.
    UnderTerrain(f32),
    /// The stepped capsule in this body by this much (m).
    Overlap(f32, usize),
    /// Off the playable level ([`Sim::level_at`]).
    OffLevel,
}

/// Every class's name, in test order (the census prints counts in it).
pub const EMBED_NAMES: [&str; 6] = [
    "inprim",
    "inclosed",
    "inshell",
    "underterrain",
    "overlap",
    "offlevel",
];

impl Embed {
    /// The class's short name ([`EMBED_NAMES`]), `"none"` for [`Embed::None`].
    pub fn name(self) -> &'static str {
        self.index().map_or("none", |i| EMBED_NAMES[i])
    }

    /// The class's index in [`EMBED_NAMES`], `None` for [`Embed::None`].
    pub fn index(self) -> Option<usize> {
        match self {
            Embed::None => None,
            Embed::InPrim(_) => Some(0),
            Embed::InClosedMesh(_) => Some(1),
            Embed::InOpenShell(_) => Some(2),
            Embed::UnderTerrain(_) => Some(3),
            Embed::Overlap(..) => Some(4),
            Embed::OffLevel => Some(5),
        }
    }

    /// The body the class names, if it names one.
    pub fn body(self) -> Option<usize> {
        match self {
            Embed::InPrim(b) | Embed::InClosedMesh(b) | Embed::InOpenShell(b) => Some(b),
            Embed::Overlap(_, b) => Some(b),
            _ => None,
        }
    }

    pub fn is_none(self) -> bool {
        self == Embed::None
    }
}

impl Sim {
    /// [`Sim::embed_capsule`] for unit `u` in its own capsule (a droideka's
    /// in its form's); [`Embed::None`] for a dead, missing or mounted unit.
    pub fn embed_state(&self, u: UnitHandle) -> Embed {
        let Some(unit) = self.unit(u).filter(|x| x.alive && x.mount.is_none()) else {
            return Embed::None;
        };
        self.embed_capsule(&self.unit_capsule(unit))
    }

    /// [`Sim::embed_capsule`] for a soldier's capsule in `stance` at `feet`.
    pub fn embed_at(&self, feet: Vec3, stance: Stance) -> Embed {
        self.embed_capsule(&Capsule {
            base: feet,
            height: capsule_height(stance),
            radius: SOLDIER_RADIUS,
        })
    }

    /// The module header's oracle for a body whose feet are `cap.base`.
    pub fn embed_capsule(&self, cap: &Capsule) -> Embed {
        let feet = cap.base;
        let h = cap.height;
        // 1. Inside a primitive: the axis points within the capsule.
        if let Some(b) = self.prim_on_axis(feet, h) {
            return Embed::InPrim(b);
        }
        // 2. Shut in a mesh (closed or open shell).
        let probe = feet + Vec3::Y * SHUT_UP.fmin(h * 0.5);
        if let Some(b) = self.shut_in(probe) {
            let closed = self.collision.bodies().get(b).is_some_and(|x| x.closed);
            return if closed {
                Embed::InClosedMesh(b)
            } else {
                Embed::InOpenShell(b)
            };
        }
        // 3. Under the uncut terrain with nothing overhead.
        if let Some(depth) = self.under_terrain(feet) {
            return Embed::UnderTerrain(depth);
        }
        // 4. The stepped capsule in a body.
        if let Some((d, b)) = self.collision.capsule_overlap(&cap.stepped()) {
            if d > EMBED_OVERLAP {
                return Embed::Overlap(d, b);
            }
        }
        // 5. Off the level.
        if self.level_at(feet).is_off() {
            return Embed::OffLevel;
        }
        Embed::None
    }

    /// Issue #182 B4: the oracle's classes a unit is never in by walking
    /// (the A4 sweep's verdict), for the runtime rescue: [`Embed::InPrim`],
    /// [`Embed::InClosedMesh`] and [`Embed::UnderTerrain`], each exactly as
    /// [`Sim::embed_capsule`] decides it (the body [`Sim::shut_in`] finds
    /// first must itself be watertight). Never the open shells (rooms read
    /// as shut in) nor an overlap (a wall graze). An earlier version asked
    /// for ANY watertight body round the point, and the census showed it
    /// killing soldiers on BF1 Mos Eisley's roofs.
    pub(crate) fn solid_embed(&self, cap: &Capsule) -> Option<Embed> {
        let (feet, h) = (cap.base, cap.height);
        if let Some(b) = self.prim_on_axis(feet, h) {
            return Some(Embed::InPrim(b));
        }
        let probe = feet + Vec3::Y * SHUT_UP.fmin(h * 0.5);
        if let Some(b) = self.shut_in(probe) {
            let closed = self.collision.bodies().get(b).is_some_and(|x| x.closed);
            return closed.then_some(Embed::InClosedMesh(b));
        }
        self.under_terrain(feet).map(Embed::UnderTerrain)
    }

    /// The oracle's step 1: the primitive an axis point of a capsule `h` m
    /// tall at `feet` is inside (mid-capsule first, then [`AXIS_HEIGHTS`]).
    fn prim_on_axis(&self, feet: Vec3, h: f32) -> Option<usize> {
        std::iter::once(h * 0.5)
            .chain(AXIS_HEIGHTS.into_iter().filter(|a| *a < h - 0.15))
            .find_map(|a| self.collision.inside_primitive(feet + Vec3::Y * a))
    }

    /// The oracle's step 3: how deep `feet` are under an UNCUT terrain with
    /// no SOLDIER face between them and the sheet, past [`EMBED_UNDER`].
    fn under_terrain(&self, feet: Vec3) -> Option<f32> {
        let hf = self.heightfield.as_ref()?;
        let depth = hf.height_at(feet.x, feet.z) - feet.y;
        (depth > EMBED_UNDER
            && !self.collision.is_cut(feet)
            && self
                .collision
                .ray(
                    feet + Vec3::Y * 0.05,
                    Vec3::Y,
                    depth,
                    crate::collision::Layers::SOLDIER,
                )
                .is_none())
        .then_some(depth)
    }

    /// The capsule unit `u` stands in: its stance's, a droideka's its
    /// form's.
    pub(crate) fn unit_capsule(&self, u: &crate::unit::Unit) -> Capsule {
        match (u.droideka_class(), u.droideka_form()) {
            (Some(c), Some(f)) => super::droideka_capsule(c, f, u.soldier.pos),
            _ => u.capsule(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{box_mesh, Body, CollisionWorld, Cutter, Layers, Shape};
    use crate::heightfield::Heightfield;
    use glam::{Mat4, Vec4};

    fn sim_with(bodies: Vec<Body>, cutters: Vec<Cutter>, terrain: Option<f32>) -> Sim {
        let mut s = Sim::empty(1);
        s.collision = CollisionWorld::new(bodies, cutters);
        s.heightfield = terrain.map(|y| Heightfield {
            size: 65,
            unit: 2.0,
            offset_x: 64.0,
            offset_z: 64.0,
            heights: vec![y; 65 * 65],
        });
        s
    }

    fn prim(centre: Vec3, shape: Shape) -> Body {
        Body::with_layers(Mat4::from_translation(centre), shape, Layers::SOLDIER)
    }

    /// The open world: nothing to be inside, on flat terrain.
    #[test]
    fn embed_state_open_ground_is_none() {
        let s = sim_with(Vec::new(), Vec::new(), Some(0.0));
        for st in [Stance::Stand, Stance::Crouch, Stance::Prone] {
            assert_eq!(s.embed_at(Vec3::new(3.0, 0.0, -2.0), st), Embed::None);
        }
    }

    /// The census's `in_primitive`, which `enclosing` lacks: a soldier in a
    /// box or a cylinder prim is InPrim with its index; one beside it is not
    /// (a wall-touching soldier is no leak).
    #[test]
    fn embed_state_inside_box_and_cylinder_prims() {
        let b = prim(
            Vec3::new(0.0, 1.0, 0.0),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 2.0),
            },
        );
        let c = prim(
            Vec3::new(10.0, 1.5, 0.0),
            Shape::Cylinder {
                radius: 1.0,
                half_height: 1.5,
            },
        );
        let s = sim_with(vec![b, c], Vec::new(), Some(0.0));
        assert_eq!(
            s.embed_at(Vec3::new(0.5, 0.0, 0.0), Stance::Stand),
            Embed::InPrim(0)
        );
        assert_eq!(
            s.embed_at(Vec3::new(10.2, 0.0, 0.1), Stance::Stand),
            Embed::InPrim(1)
        );
        // A prone soldier inside the box is seen at mid-capsule.
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 0.0, 1.0), Stance::Prone),
            Embed::InPrim(0)
        );
        // Standing on top of the box, and touching its side: not inside.
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 2.0, 0.0), Stance::Stand),
            Embed::None
        );
        assert_eq!(
            s.embed_at(Vec3::new(2.5, 0.0, 0.0), Stance::Stand),
            Embed::None
        );
    }

    /// A watertight mesh (Endor's boulder) is InClosedMesh; the same box
    /// with its bottom left open (a hollow trunk) is an open shell.
    #[test]
    fn embed_state_shut_in_closed_mesh_and_open_shell() {
        let half = Vec3::new(3.0, 3.0, 3.0);
        let s = sim_with(
            vec![
                box_mesh(Vec3::new(0.0, 3.0, 0.0), half, false),
                box_mesh(Vec3::new(20.0, 3.0, 0.0), half, true),
            ],
            Vec::new(),
            Some(0.0),
        );
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 0.0, 0.0), Stance::Stand),
            Embed::InClosedMesh(0)
        );
        assert_eq!(
            s.embed_at(Vec3::new(20.0, 0.0, 0.0), Stance::Stand),
            Embed::InOpenShell(1)
        );
        assert_eq!(
            s.embed_at(Vec3::new(10.0, 0.0, 0.0), Stance::Stand),
            Embed::None
        );
    }

    /// Under an uncut terrain sheet with the sky over it: UnderTerrain with
    /// the depth. The same feet under a roof (a SOLDIER face between them and
    /// the sheet: a real base) or in a cut (a trench) are not.
    #[test]
    fn embed_state_under_terrain_but_not_in_a_tunnel_or_a_cut() {
        let s = sim_with(Vec::new(), Vec::new(), Some(5.0));
        match s.embed_at(Vec3::new(0.0, 2.0, 0.0), Stance::Stand) {
            Embed::UnderTerrain(d) => assert!((d - 3.0).abs() < 1e-4, "depth {d}"),
            e => panic!("want UnderTerrain, got {e:?}"),
        }
        // A bank's few cm are not under the map.
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 4.8, 0.0), Stance::Stand),
            Embed::None
        );
        // A roof slab under the sheet: a tunnel (Polis Massa's case).
        let roof = prim(
            Vec3::new(0.0, 4.5, 0.0),
            Shape::Box {
                half: Vec3::new(6.0, 0.25, 6.0),
            },
        );
        let s = sim_with(vec![roof], Vec::new(), Some(5.0));
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 2.0, 0.0), Stance::Stand),
            Embed::None
        );
        // A cutter over the feet: a trench, the terrain is not there.
        let cut = Cutter {
            sphere: [0.0; 4],
            planes: vec![
                Vec4::new(1.0, 0.0, 0.0, -4.0),
                Vec4::new(-1.0, 0.0, 0.0, -4.0),
                Vec4::new(0.0, 0.0, 1.0, -4.0),
                Vec4::new(0.0, 0.0, -1.0, -4.0),
            ],
        };
        let s = sim_with(Vec::new(), vec![cut], Some(5.0));
        assert_eq!(
            s.embed_at(Vec3::new(0.0, 2.0, 0.0), Stance::Stand),
            Embed::None
        );
    }

    /// A head inside a low deck (the stand-up-under-a-ledge case): the
    /// standing capsule overlaps the slab, the crouched one under it fits.
    #[test]
    fn embed_state_overlap_standing_under_a_low_deck() {
        let deck = prim(
            Vec3::new(0.0, 1.95, 0.0),
            Shape::Box {
                half: Vec3::new(4.0, 0.25, 4.0),
            },
        );
        let s = sim_with(vec![deck], Vec::new(), Some(0.0));
        match s.embed_at(Vec3::ZERO, Stance::Stand) {
            Embed::Overlap(d, 0) => assert!((d - 0.2).abs() < 0.02, "depth {d}"),
            e => panic!("want Overlap, got {e:?}"),
        }
        assert_eq!(s.embed_at(Vec3::ZERO, Stance::Crouch), Embed::None);
    }

    /// A wall touched, or grazed by less than the threshold, is no overlap.
    #[test]
    fn embed_state_wall_contact_under_threshold_is_none() {
        let wall = prim(
            Vec3::new(1.0 + 0.5 - 0.03, 2.0, 0.0),
            Shape::Box {
                half: Vec3::new(1.0, 2.0, 4.0),
            },
        );
        let s = sim_with(vec![wall], Vec::new(), Some(0.0));
        assert_eq!(s.embed_at(Vec3::ZERO, Stance::Stand), Embed::None);
        let s2 = sim_with(
            vec![prim(
                Vec3::new(1.0 + 0.5 - 0.2, 2.0, 0.0),
                Shape::Box {
                    half: Vec3::new(1.0, 2.0, 4.0),
                },
            )],
            Vec::new(),
            Some(0.0),
        );
        assert!(matches!(
            s2.embed_at(Vec3::ZERO, Stance::Stand),
            Embed::Overlap(_, 0)
        ));
    }

    /// The oracle is a pure read: the same answer twice, and a dead or
    /// missing unit is None.
    #[test]
    fn embed_state_is_read_only_and_ignores_the_dead() {
        let b = prim(
            Vec3::new(0.0, 1.0, 0.0),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 2.0),
            },
        );
        let mut s = sim_with(vec![b], Vec::new(), Some(0.0));
        let h = s
            .spawn(&crate::unit::UnitSpec::trooper(0, Vec3::new(0.5, 0.0, 0.0)))
            .expect("spawned");
        s.units[h.index as usize].soldier.pos = Vec3::new(0.5, 0.0, 0.0);
        // A live, unmounted unit inside the box reads InPrim, twice, and
        // asking leaves the sim's state as it was.
        let before = s.hash();
        assert_eq!(s.embed_state(h), Embed::InPrim(0));
        assert_eq!(s.embed_state(h), Embed::InPrim(0));
        assert_eq!(s.hash(), before);
        // Mounted: the vehicle owns his body.
        s.units[h.index as usize].mount = Some(crate::vehicle::Mount {
            vehicle: crate::vehicle::VehicleHandle { index: 0, gen: 0 },
            seat: 0,
        });
        assert_eq!(s.embed_state(h), Embed::None);
        // Dead: nothing to rescue.
        s.units[h.index as usize].mount = None;
        assert_eq!(s.embed_state(h), Embed::InPrim(0));
        s.units[h.index as usize].alive = false;
        assert_eq!(s.embed_state(h), Embed::None);
        // A handle to no unit at all.
        assert_eq!(s.embed_state(UnitHandle { index: 7, gen: 0 }), Embed::None);
    }
}
