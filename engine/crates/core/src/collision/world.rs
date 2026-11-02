//! [`CollisionWorld`] — the map's placed bodies plus its terrain cutters —
//! and its broad phase: [`BodyGrid`], a uniform XZ grid of body indices built
//! once by [`CollisionWorld::new`], through which every query is offered its
//! candidate bodies in ascending index order ([`CollisionWorld::visit_xz`]).
//! Why the grid cannot change an answer is on [`BodyGrid`].
//!
//! And the vehicles' HULLS (card VHULL): each live vehicle's bodies at its
//! transform, a short list kept apart from `bodies` (so the grid and the
//! doors' body indices never move) and offered to every query AFTER the
//! map's bodies, in vehicle slot order — see [`HullFilter`] for who skips
//! which.

use super::shape::{Aabb, Body, Cutter, HullPart};
use crate::vehicle::VehicleHandle;
use glam::Vec3;

/// Cell edge of [`BodyGrid`], in metres. Not a PS2 number: the PS2's own
/// broad phase is not RE'd (nothing in `analysis/` describes it), and this
/// structure is a pure prefilter that cannot change an answer, so it is a
/// tuning constant and not a fidelity claim.
///
/// MEASURED, 2026-09-08, `--bench` TAT2 `--ai 64 --ai-cap 64`, median of five
/// per point, one binary rebuilt per point on one machine (sim tick mean /
/// p99 in ms): no grid 1.467 / 3.531, 64 m 0.644 / 2.287, **16 m 0.441 /
/// 1.547**, 12 m 0.445 / 1.548, 8 m 0.470 / 1.631, 32 m 0.502 / 1.706. The
/// curve is flat between 12 and 16 m and turns up either side — too coarse
/// and a cell holds bodies the query will only reject, too fine and one
/// body is filed in (and re-sorted out of) many cells.
const BROADPHASE_CELL: f32 = 16.0;

/// A body covering more cells than this is filed in [`BodyGrid::everywhere`]
/// instead of in each of them — one map-spanning `coll` mesh must not be
/// copied into a thousand cells.
const BROADPHASE_MAX_CELLS_PER_BODY: usize = 64;

/// Cap on the grid's side in cells, so a body parked 10^6 m out cannot ask
/// for a gigantic index; the cell grows instead. 128 x 128 = 16,384 cells.
const BROADPHASE_MAX_SIDE: usize = 128;
/// How far past a segment [`CollisionWorld::visit_segment`] reads (m): far more
/// than the rounding of a point on a line this long, or of the slab test.
const SEGMENT_MARGIN: f32 = 1.0;
/// The longest segment it walks cell by cell (m).
const SEGMENT_MAX_WALK: f32 = 100_000.0;

thread_local! {
    /// Scratch for [`CollisionWorld::visit_xz`]: the candidate list, reused so
    /// a per-tick query allocates nothing after the first. Per THREAD (the
    /// sweep runs a map per worker), and taken out of the cell while it is in
    /// use, so a re-entrant query would simply get a fresh one.
    static BROADPHASE_SCRATCH: std::cell::Cell<Vec<u32>> =
        const { std::cell::Cell::new(Vec::new()) };
}

/// The inclusive cell range an XZ footprint covers.
#[derive(Clone, Copy)]
struct CellRange {
    x0: usize,
    x1: usize,
    z0: usize,
    z1: usize,
}

/// `v`'s cell on one axis, clamped into `0..n`. NaN lands in cell 0 (the
/// saturating float cast), which is why [`BodyGrid::cells_for`] rejects a
/// non-finite query outright rather than trusting this.
fn cell_of(v: f32, min: f32, inv_cell: f32, n: usize) -> usize {
    let c = ((v - min) * inv_cell).floor();
    if c <= 0.0 {
        0
    } else {
        (c as usize).min(n - 1)
    }
}

/// Can this AABB be filed in the grid at all?
fn indexable(a: &Aabb) -> bool {
    !a.is_empty()
        && a.min.x.is_finite()
        && a.max.x.is_finite()
        && a.min.z.is_finite()
        && a.max.z.is_finite()
}

/// The broad phase over [`CollisionWorld::bodies`]: a uniform grid in XZ,
/// built ONCE by [`CollisionWorld::new`], holding body INDICES.
///
/// WHY IT CANNOT CHANGE AN ANSWER. It is a prefilter and nothing else.
/// - Every query still runs, on every body it is handed, the exact `Aabb`
///   test it ran before (`overlaps` / `hits_ray`) and then the exact parry
///   query. Nothing is decided by a cell.
/// - The set it hands over is a SUPERSET of the bodies whose world AABB meets
///   the query's XZ footprint: a body is filed in every cell its own AABB
///   touches, a query reads every cell its footprint touches, and both map a
///   coordinate to a cell through the same clamped [`cell_of`] — so two
///   overlapping XZ ranges always share a cell. A body the grid cannot place
///   (empty, non-finite, or spanning more than
///   [`BROADPHASE_MAX_CELLS_PER_BODY`] cells) is in `everywhere` and is handed
///   to every query.
/// - The indices are handed over SORTED ASCENDING and deduped, i.e. in
///   `bodies` order — which is what this module's tie-breaks ("the first body
///   wins") and [`CollisionWorld::push_out`]'s running sum are defined on.
/// - A grid is stamped with the [`CollisionWorld`] generation it was built
///   for, and every edit to `bodies` goes through a method that bumps that
///   generation (`bodies` is private). A world built any other way
///   ([`CollisionWorld::default`], or one pushed to afterwards) has a grid
///   whose stamp no longer matches, and then every query scans every body
///   exactly as before.
///
/// Y is not indexed: the maps are wide and short, and the exact AABB test
/// that follows is 3D anyway.
#[derive(Default)]
struct BodyGrid {
    /// The [`CollisionWorld`] generation this grid was built for; any other
    /// generation (and `None`, the default) is stale.
    built_for: Option<u64>,
    min_x: f32,
    min_z: f32,
    inv_cell: f32,
    nx: usize,
    nz: usize,
    /// CSR: cell `c` owns `cells[starts[c] as usize..starts[c + 1] as usize]`,
    /// ascending by body index. Length `nx * nz + 1`.
    starts: Vec<u32>,
    cells: Vec<u32>,
    /// Bodies in no cell — handed to every query. Ascending.
    everywhere: Vec<u32>,
}

impl BodyGrid {
    /// File every body under its own AABB, except a MOVER, which is filed
    /// under its envelope (`envelopes`, ascending by index) — the box holding
    /// every place it can move to, so moving it inside that box leaves the
    /// grid a superset and therefore current.
    fn build(bodies: &[Body], envelopes: &[(u32, Aabb)], generation: u64) -> BodyGrid {
        let len = bodies.len();
        let filed: Vec<Aabb> = {
            let mut env = envelopes.iter().peekable();
            (0..len)
                .map(|i| match env.next_if(|(k, _)| *k as usize == i) {
                    // The envelope AND where the body is now: a mover moved
                    // out of its envelope is still filed where it stands.
                    Some((_, e)) if !bodies[i].aabb.is_empty() => {
                        let mut e = *e;
                        e.add(bodies[i].aabb.min);
                        e.add(bodies[i].aabb.max);
                        e
                    }
                    _ => bodies[i].aabb,
                })
                .collect()
        };
        let (mut lo_x, mut lo_z) = (f32::INFINITY, f32::INFINITY);
        let (mut hi_x, mut hi_z) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for a in &filed {
            if !indexable(a) {
                continue;
            }
            lo_x = lo_x.min(a.min.x);
            hi_x = hi_x.max(a.max.x);
            lo_z = lo_z.min(a.min.z);
            hi_z = hi_z.max(a.max.z);
        }
        if !(lo_x <= hi_x && lo_z <= hi_z) {
            // Nothing placeable: one empty cell, everyone in `everywhere`.
            return BodyGrid {
                built_for: Some(generation),
                min_x: 0.0,
                min_z: 0.0,
                inv_cell: 1.0 / BROADPHASE_CELL,
                nx: 1,
                nz: 1,
                starts: vec![0; 2],
                cells: Vec::new(),
                everywhere: (0..len as u32).collect(),
            };
        }
        let (span_x, span_z) = (hi_x - lo_x, hi_z - lo_z);
        let cell = BROADPHASE_CELL
            .max(span_x / BROADPHASE_MAX_SIDE as f32)
            .max(span_z / BROADPHASE_MAX_SIDE as f32);
        let inv_cell = 1.0 / cell;
        let nx = (((span_x * inv_cell).floor() as usize) + 1).min(BROADPHASE_MAX_SIDE);
        let nz = (((span_z * inv_cell).floor() as usize) + 1).min(BROADPHASE_MAX_SIDE);

        // Pass 1: where each body goes, and how many bodies each cell holds.
        let mut ranges: Vec<Option<CellRange>> = Vec::with_capacity(len);
        let mut counts = vec![0u32; nx * nz];
        let mut everywhere = Vec::new();
        for (i, a) in filed.iter().enumerate() {
            let r = indexable(a)
                .then(|| CellRange {
                    x0: cell_of(a.min.x, lo_x, inv_cell, nx),
                    x1: cell_of(a.max.x, lo_x, inv_cell, nx),
                    z0: cell_of(a.min.z, lo_z, inv_cell, nz),
                    z1: cell_of(a.max.z, lo_z, inv_cell, nz),
                })
                .filter(|r| (r.x1 - r.x0 + 1) * (r.z1 - r.z0 + 1) <= BROADPHASE_MAX_CELLS_PER_BODY);
            match r {
                Some(r) => {
                    for z in r.z0..=r.z1 {
                        for x in r.x0..=r.x1 {
                            counts[z * nx + x] += 1;
                        }
                    }
                }
                None => everywhere.push(i as u32),
            }
            ranges.push(r);
        }

        // Pass 2: the CSR offsets, then the bodies in ASCENDING index order,
        // which is what makes each cell's own list ascending.
        let mut starts = Vec::with_capacity(nx * nz + 1);
        let mut acc = 0u32;
        starts.push(0);
        for c in &counts {
            acc += c;
            starts.push(acc);
        }
        let mut cursor = starts[..nx * nz].to_vec();
        let mut cells = vec![0u32; acc as usize];
        for (i, r) in ranges.iter().enumerate() {
            let Some(r) = r else { continue };
            for z in r.z0..=r.z1 {
                for x in r.x0..=r.x1 {
                    let c = z * nx + x;
                    cells[cursor[c] as usize] = i as u32;
                    cursor[c] += 1;
                }
            }
        }
        BodyGrid {
            built_for: Some(generation),
            min_x: lo_x,
            min_z: lo_z,
            inv_cell,
            nx,
            nz,
            starts,
            cells,
            everywhere,
        }
    }

    /// The cells an XZ footprint covers, or `None` when this grid must not
    /// serve the query: it was built for another generation (stale), it has
    /// no cells, or a coordinate is not finite. The caller then scans
    /// everything, exactly as before the grid existed.
    fn cells_for(
        &self,
        generation: u64,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Option<CellRange> {
        if self.built_for != Some(generation) || self.nx == 0 || self.nz == 0 {
            return None;
        }
        if !(min_x.is_finite() && max_x.is_finite() && min_z.is_finite() && max_z.is_finite()) {
            return None;
        }
        Some(CellRange {
            x0: cell_of(min_x, self.min_x, self.inv_cell, self.nx),
            x1: cell_of(max_x, self.min_x, self.inv_cell, self.nx),
            z0: cell_of(min_z, self.min_z, self.inv_cell, self.nz),
            z1: cell_of(max_z, self.min_z, self.inv_cell, self.nz),
        })
    }

    /// Append every candidate index for `r` to `out` — unsorted, with repeats
    /// (a body wide enough to sit in several of the cells read). A row's
    /// cells are contiguous in the CSR, so a row costs one slice copy.
    fn gather(&self, r: CellRange, out: &mut Vec<u32>) {
        out.extend_from_slice(&self.everywhere);
        for z in r.z0..=r.z1 {
            self.gather_row(z, r.x0, r.x1, out);
        }
    }

    fn gather_row(&self, z: usize, x0: usize, x1: usize, out: &mut Vec<u32>) {
        let row = z * self.nx;
        let a = self.starts[row + x0] as usize;
        let b = self.starts[row + x1 + 1] as usize;
        out.extend_from_slice(&self.cells[a..b]);
    }

    /// [`Self::gather`] for the segment `from`..`to` inside `r`: in each row of
    /// `r`, only the span of cells the segment crosses in that row, widened by
    /// [`SEGMENT_MARGIN`]. A coordinate off the grid clamps to its edge cell,
    /// exactly as [`cell_of`] clamps a body's, so the first and last rows also
    /// answer for everything beyond them.
    fn gather_segment(&self, r: CellRange, from: Vec3, to: Vec3, out: &mut Vec<u32>) {
        out.extend_from_slice(&self.everywhere);
        let (dx, dz) = (to.x - from.x, to.z - from.z);
        let cell = 1.0 / self.inv_cell;
        for z in r.z0..=r.z1 {
            let low = if z == 0 {
                f32::NEG_INFINITY
            } else {
                self.min_z + z as f32 * cell - SEGMENT_MARGIN
            };
            let high = if z + 1 == self.nz {
                f32::INFINITY
            } else {
                self.min_z + (z + 1) as f32 * cell + SEGMENT_MARGIN
            };
            // Where the segment is at this row's two edges.
            let (mut t0, mut t1) = if dz == 0.0 {
                (0.0, 1.0)
            } else {
                let (a, b) = ((low - from.z) / dz, (high - from.z) / dz);
                (a.min(b), a.max(b))
            };
            t0 = t0.clamp(0.0, 1.0);
            t1 = t1.clamp(0.0, 1.0);
            let (xa, xb) = (from.x + dx * t0, from.x + dx * t1);
            let x0 = cell_of(
                xa.min(xb) - SEGMENT_MARGIN,
                self.min_x,
                self.inv_cell,
                self.nx,
            );
            let x1 = cell_of(
                xa.max(xb) + SEGMENT_MARGIN,
                self.min_x,
                self.inv_cell,
                self.nx,
            );
            self.gather_row(z, x0.max(r.x0), x1.min(r.x1), out);
        }
    }
}

/// Which vehicles' hull bodies a query sees (card VHULL). A body with no
/// owner (the map's) is always seen.
///
/// PROVISIONAL (ours) in who passes what: soldiers' queries, bolts and lines
/// of sight see every hull; a ray from a rider, or at a vehicle or its rider,
/// skips that vehicle's own ([`HullFilter::Except`]) — its eye and its
/// target's centre are inside the hull; the vehicles' own contacts see none
/// ([`HullFilter::Static`]) — the PS2 pairs vehicles with each other
/// (analysis/re-collision-layers.md §3, "pairs vs 1|4"), which the port does
/// not model yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HullFilter {
    /// Every body, every hull.
    All,
    /// The map's bodies only.
    Static,
    /// Every body but these vehicles' hulls.
    Except([Option<VehicleHandle>; 2]),
    /// The vehicles' hulls only (the AI's hull probe, card STFAL).
    Hulls,
    /// The vehicles' hulls but these (a live bolt's per-tick test, issue
    /// #41: a rider's bolt passes its own vehicle).
    HullsExcept([Option<VehicleHandle>; 2]),
    /// This one vehicle's hull alone (the chase camera's see-through test,
    /// `game::camera_collide`).
    Only(VehicleHandle),
}

impl HullFilter {
    /// Every hull but `a`'s (none when `a` is `None`).
    pub fn except(a: Option<VehicleHandle>) -> HullFilter {
        HullFilter::Except([a, None])
    }

    /// The hulls this filter admits and no map body; `None` when it admits
    /// no hull.
    pub fn hulls_only(self) -> Option<HullFilter> {
        match self {
            HullFilter::All | HullFilter::Hulls => Some(HullFilter::Hulls),
            HullFilter::Only(h) => Some(HullFilter::Only(h)),
            HullFilter::Except(skip) | HullFilter::HullsExcept(skip) => {
                Some(HullFilter::HullsExcept(skip))
            }
            HullFilter::Static => None,
        }
    }

    /// Does a query under this filter see a body owned by `owner`?
    pub fn admits(self, owner: Option<VehicleHandle>) -> bool {
        match (self, owner) {
            (HullFilter::Only(h), owner) => owner == Some(h),
            (HullFilter::Hulls, owner) => owner.is_some(),
            (HullFilter::HullsExcept(skip), owner) => owner.is_some() && !skip.contains(&owner),
            (_, None) | (HullFilter::All, _) => true,
            (HullFilter::Static, Some(_)) => false,
            (HullFilter::Except(skip), Some(o)) => !skip.contains(&Some(o)),
        }
    }
}

/// One live vehicle's hull: its class's [`HullPart`]s placed at its pose.
struct Hull {
    owner: VehicleHandle,
    bodies: Vec<Body>,
    /// What `bodies` were placed from (card HULLC): a call with the same
    /// parts at the same pose (a parked vehicle, every tick) keeps them.
    pose: glam::Mat4,
    parts: Vec<HullPart>,
}

/// The map's collision: placed bodies plus the terrain cutters. Static after
/// construction except for its MOVERS (queue DR2's doors): bodies declared at
/// build time with an envelope, which [`CollisionWorld::move_body`] moves.
///
/// `bodies` is private: read it through [`CollisionWorld::bodies`], add to it
/// with [`CollisionWorld::push_body`] (the fixtures do), move one with
/// [`CollisionWorld::move_body`]. Every edit bumps `generation`, the stamp the
/// broad phase checks, so no edit can leave a grid that believes itself
/// current. A push leaves the grid stale — every query falls back to the full
/// scan, which costs speed and not correctness — until
/// [`CollisionWorld::rebuild_broadphase`].
#[derive(Default)]
pub struct CollisionWorld {
    /// The placed bodies.
    bodies: Vec<Body>,
    pub cutters: Vec<Cutter>,
    /// Bumped by every edit to `bodies`; `grid` serves only the generation it
    /// was built for.
    generation: u64,
    /// The broad phase over `bodies`, built by [`CollisionWorld::new`]. Empty
    /// or stale = every query scans every body, as it did before it existed.
    grid: BodyGrid,
    /// The movers: (body index, envelope), ascending by index. The grid files
    /// each under its envelope instead of its current AABB.
    envelopes: Vec<(u32, Aabb)>,
    /// The vehicles' hulls (card VHULL), ascending by slot index. Not in the
    /// grid: a query reads them all, after `bodies`, through its own AABB
    /// test.
    hulls: Vec<Hull>,
    stations: std::collections::BTreeMap<u32, u16>,
    /// The building turrets: a placed instance whose bodies are a live
    /// turret's building (`Sim::bind_building`).
    turrets: std::collections::BTreeMap<u32, VehicleHandle>,
}

impl CollisionWorld {
    /// The world, with its broad phase built — what
    /// `game::assets::Scene::collision_world` returns and what every query
    /// wants. Callers can also start with [`Self::default`] and add bodies
    /// through [`Self::push_body`]; those queries scan every body until
    /// [`Self::rebuild_broadphase`] is called.
    pub fn new(bodies: Vec<Body>, cutters: Vec<Cutter>) -> CollisionWorld {
        CollisionWorld::with_movers(bodies, cutters, Vec::new())
    }

    /// [`CollisionWorld::new`] with MOVERS: each `(index, envelope)` names a
    /// body that [`CollisionWorld::move_body`] may move, and an AABB holding
    /// every place it will move to. The grid files it under that envelope,
    /// so a move inside it costs no rebuild. With no movers this is `new`.
    pub fn with_movers(
        bodies: Vec<Body>,
        cutters: Vec<Cutter>,
        mut movers: Vec<(usize, Aabb)>,
    ) -> CollisionWorld {
        movers.sort_by_key(|(i, _)| *i);
        movers.dedup_by_key(|(i, _)| *i);
        let envelopes: Vec<(u32, Aabb)> = movers
            .into_iter()
            .filter(|(i, _)| *i < bodies.len())
            .map(|(i, mut e)| {
                // The envelope holds the body where it is now, too.
                e.add(bodies[i].aabb.min);
                e.add(bodies[i].aabb.max);
                (i as u32, e)
            })
            .collect();
        let grid = BodyGrid::build(&bodies, &envelopes, 0);
        CollisionWorld {
            bodies,
            cutters,
            generation: 0,
            grid,
            envelopes,
            hulls: Vec::new(),
            stations: std::collections::BTreeMap::new(),
            turrets: std::collections::BTreeMap::new(),
        }
    }

    /// The placed bodies, in the order every query's tie-break is defined on.
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// Map bodies followed by every live vehicle hull, in stable slot order.
    /// Physical weapon sweeps need both; `bodies` alone is only the map.
    pub fn bodies_and_hulls(&self) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .chain(self.hulls.iter().flat_map(|h| h.bodies.iter()))
    }

    /// Associate a placed map instance with a live station index.
    pub fn bind_station(&mut self, instance: u32, station: u16) {
        self.stations.insert(instance, station);
    }

    /// Resolve a collision instance to its station, if it is a service droid.
    pub fn station(&self, instance: u32) -> Option<u16> {
        self.stations.get(&instance).copied()
    }

    /// Make a placed instance's bodies a building turret's building: a hit on
    /// one is a hit on the turret (`analysis/re-turret-collision.md` 1.2, 3.1:
    /// the PS2's ray tests the armed building's own bodies and damages it).
    pub fn bind_turret(&mut self, instance: u32, turret: VehicleHandle) {
        self.turrets.insert(instance, turret);
    }

    /// Release a building turret's instance (its turret is gone): its bodies
    /// are plain map bodies again.
    pub fn unbind_turret(&mut self, turret: VehicleHandle) {
        self.turrets.retain(|_, v| *v != turret);
    }

    /// Resolve a collision instance to its building turret, if it is one.
    pub fn turret(&self, instance: u32) -> Option<VehicleHandle> {
        self.turrets.get(&instance).copied()
    }

    /// The instance whose bodies are building turret `turret`'s building.
    pub fn turret_instance(&self, turret: VehicleHandle) -> Option<u32> {
        self.turrets
            .iter()
            .find_map(|(i, v)| (*v == turret).then_some(*i))
    }

    /// Make bodies movers after construction: each `(index, envelope)` as
    /// [`CollisionWorld::with_movers`] takes them, then the broad phase is
    /// rebuilt over them.
    pub fn add_movers(&mut self, movers: &[(usize, Aabb)]) {
        for &(i, mut e) in movers {
            let Some(b) = self.bodies.get(i) else {
                continue;
            };
            e.add(b.aabb.min);
            e.add(b.aabb.max);
            self.envelopes.retain(|(k, _)| *k as usize != i);
            self.envelopes.push((i as u32, e));
        }
        self.envelopes.sort_by_key(|(i, _)| *i);
        self.rebuild_broadphase();
    }

    /// Register swept rotation envelopes and return body-local transforms.
    pub fn prepare_rotation(
        &mut self,
        instance: u32,
        pose: glam::Mat4,
    ) -> Vec<(usize, glam::Mat4)> {
        let origin = pose.w_axis.truncate();
        let inverse = pose.inverse();
        let mut parts = Vec::new();
        for (i, body) in self.bodies.iter().enumerate() {
            if body.instance != Some(instance) || body.layers == super::Layers::NONE {
                continue;
            }
            let extent = (body.aabb.min - origin)
                .abs()
                .max((body.aabb.max - origin).abs())
                .length()
                + 0.01;
            let envelope = Aabb {
                min: origin - glam::Vec3::splat(extent),
                max: origin + glam::Vec3::splat(extent),
            };
            self.envelopes.retain(|(k, _)| *k as usize != i);
            self.envelopes.push((i as u32, envelope));
            parts.push((i, inverse * body.to_world));
        }
        self.envelopes.sort_by_key(|(i, _)| *i);
        self.rebuild_broadphase();
        parts
    }

    /// An instance's live bodies as parts in the space of `pose` (the
    /// instance's own matrix): what [`CollisionWorld::replace_instance`]
    /// puts back. Bodies in no subset are left out.
    pub fn instance_parts(&self, instance: u32, pose: glam::Mat4) -> Vec<HullPart> {
        let inverse = pose.inverse();
        self.bodies
            .iter()
            .filter(|b| b.instance == Some(instance) && b.layers != super::Layers::NONE)
            .map(|b| HullPart {
                local: inverse * b.to_world,
                shape: b.shape.clone(),
                layers: b.layers,
                closed: b.closed,
                hit_mult: b.hit_mult,
            })
            .collect()
    }

    /// Replace an instance's bodies with its authored wreck at the death pose.
    pub fn replace_instance(&mut self, instance: u32, parts: &[HullPart], pose: glam::Mat4) {
        self.stations.remove(&instance);
        let slots: Vec<_> = self
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(i, b)| (b.instance == Some(instance)).then_some(i))
            .collect();
        self.envelopes
            .retain(|(i, _)| !slots.contains(&(*i as usize)));
        for &i in &slots {
            self.bodies[i].layers = super::Layers::NONE;
        }
        for (k, p) in parts.iter().enumerate() {
            let mut b = Body::with_layers(pose * p.local, p.shape.clone(), p.layers);
            b.instance = Some(instance);
            if let Some(&i) = slots.get(k) {
                self.bodies[i] = b;
            } else {
                self.bodies.push(b);
            }
        }
        self.generation += 1;
        self.rebuild_broadphase();
    }

    /// Append a body. The broad phase goes stale (every query scans every
    /// body, still exact) until [`CollisionWorld::rebuild_broadphase`].
    pub fn push_body(&mut self, body: Body) {
        self.bodies.push(body);
        self.generation += 1;
    }

    /// Move body `i` to `to_world`, keeping its shape and layers. A mover
    /// that stays inside its envelope leaves the broad phase as it is (it is
    /// filed there already); anything else — a body that is not a mover, or
    /// a mover leaving its envelope — rebuilds it, so the answer is right
    /// either way and only the cost differs.
    pub fn move_body(&mut self, i: usize, to_world: glam::Mat4) {
        let Some(b) = self.bodies.get(i) else {
            return;
        };
        let mut moved = Body::placed(to_world, b.shape.clone(), b.layers, b.closed);
        moved.owner = b.owner;
        moved.instance = b.instance;
        let inside = self
            .envelopes
            .binary_search_by_key(&(i as u32), |(k, _)| *k)
            .ok()
            .is_some_and(|k| {
                let e = self.envelopes[k].1;
                moved.aabb.min.cmpge(e.min).all() && moved.aabb.max.cmple(e.max).all()
            });
        self.bodies[i] = moved;
        let was_live = self.grid.built_for == Some(self.generation);
        self.generation += 1;
        if inside && was_live {
            // Filed under its envelope already: the grid is still a superset.
            self.grid.built_for = Some(self.generation);
        } else if !inside {
            self.rebuild_broadphase();
        }
    }

    /// The movers' envelopes (queue DR2's doors): where a body may stand
    /// over the round, whatever it is doing now.
    pub fn mover_envelopes(&self) -> impl Iterator<Item = &Aabb> + '_ {
        self.envelopes.iter().map(|(_, a)| a)
    }

    /// Place vehicle `owner`'s hull: each of `parts` at `pose · part.local`,
    /// replacing whatever the slot held (any generation). No part → no hull.
    pub fn set_hull(&mut self, owner: VehicleHandle, parts: &[HullPart], pose: glam::Mat4) {
        let slot = self
            .hulls
            .binary_search_by_key(&owner.index, |h| h.owner.index);
        if let Ok(k) = slot {
            let h = &self.hulls[k];
            let bits = |m: &glam::Mat4| m.to_cols_array().map(f32::to_bits);
            if h.owner == owner
                && bits(&h.pose) == bits(&pose)
                && h.parts.len() == parts.len()
                && h.parts.iter().zip(parts).all(|(a, b)| a.same_as(b))
            {
                // Every body would be placed again bit for bit as it is.
                return;
            }
        }
        let place = |p: &HullPart| {
            let mut b = Body::placed(pose * p.local, p.shape.clone(), p.layers, p.closed);
            b.owner = Some(owner);
            b.hit_mult = p.hit_mult;
            b
        };
        match slot {
            Ok(k) if parts.is_empty() => {
                self.hulls.remove(k);
            }
            // A moving vehicle comes here every tick: its lists are kept.
            Ok(k) => {
                let h = &mut self.hulls[k];
                h.owner = owner;
                h.pose = pose;
                h.bodies.clear();
                h.bodies.extend(parts.iter().map(place));
                h.parts.clear();
                h.parts.extend_from_slice(parts);
            }
            Err(_) if parts.is_empty() => {}
            Err(k) => self.hulls.insert(
                k,
                Hull {
                    owner,
                    bodies: parts.iter().map(place).collect(),
                    pose,
                    parts: parts.to_vec(),
                },
            ),
        }
    }

    /// Take the hull of vehicle slot `owner.index` out (any generation).
    pub fn remove_hull(&mut self, owner: VehicleHandle) {
        if let Ok(k) = self
            .hulls
            .binary_search_by_key(&owner.index, |h| h.owner.index)
        {
            self.hulls.remove(k);
        }
    }

    /// The hull bodies of `owner`, or none (an exact handle).
    pub fn hull(&self, owner: VehicleHandle) -> &[Body] {
        self.hulls
            .iter()
            .find(|h| h.owner == owner)
            .map_or(&[], |h| &h.bodies)
    }

    /// No body and no hull: nothing for a soldier to meet.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty() && self.hulls.is_empty()
    }

    /// Rebuild the broad phase after [`CollisionWorld::push_body`]. Only
    /// speed rides on it: until then the stale stamp makes every query scan
    /// every body.
    pub fn rebuild_broadphase(&mut self) {
        self.envelopes
            .retain(|(i, _)| (*i as usize) < self.bodies.len());
        self.grid = BodyGrid::build(&self.bodies, &self.envelopes, self.generation);
    }

    /// The bodies whose world AABB can meet a query with the XZ footprint
    /// `[min_x, max_x] x [min_z, max_z]`, handed to `f` in ASCENDING `bodies`
    /// order — the broad phase ([`BodyGrid`], and its "why it cannot change
    /// an answer"). A SUPERSET: `f` still runs the exact test. Then the
    /// hulls `hulls` admits, in slot order, each body whose XZ bounds meet
    /// the footprint.
    pub(super) fn visit_xz<'a>(
        &'a self,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        hulls: HullFilter,
        f: impl FnMut(&'a Body),
    ) {
        let cells = self
            .grid
            .cells_for(self.generation, min_x, max_x, min_z, max_z);
        self.visit_cells(
            cells,
            [min_x, max_x, min_z, max_z],
            hulls,
            f,
            |grid, r, out| grid.gather(r, out),
        );
    }

    /// [`Self::visit_xz`] for the segment `from + dir * [0, len]`: only the cells
    /// the segment crosses, not every cell of its bounding box. A long diagonal
    /// line of sight's box is most of the map, and copying and sorting that
    /// list was a fifth of a tick on Mygeeto. Still a superset of the bodies
    /// whose AABB the segment meets: such a body is filed under the cell of
    /// the meeting point, and the walk widens by [`SEGMENT_MARGIN`] on both
    /// axes. It offers bodies the segment misses too, so `f` keeps its own
    /// slab test.
    pub(super) fn visit_segment<'a>(
        &'a self,
        from: Vec3,
        dir: Vec3,
        len: f32,
        hulls: HullFilter,
        f: impl FnMut(&'a Body),
    ) {
        let to = from + dir * len;
        let m = SEGMENT_MARGIN;
        let (min_x, max_x) = (from.x.min(to.x) - m, from.x.max(to.x) + m);
        let (min_z, max_z) = (from.z.min(to.z) - m, from.z.max(to.z) + m);
        let cells = self
            .grid
            .cells_for(self.generation, min_x, max_x, min_z, max_z);
        // Past this length the walk's own rounding nears the margin, and the
        // box it would have read is the whole map anyway.
        let walkable = (to - from).length_squared() < SEGMENT_MAX_WALK * SEGMENT_MAX_WALK;
        self.visit_cells(
            cells,
            [min_x, max_x, min_z, max_z],
            hulls,
            f,
            |grid, r, out| {
                if walkable {
                    grid.gather_segment(r, from, to, out)
                } else {
                    grid.gather(r, out)
                }
            },
        );
    }

    fn visit_cells<'a>(
        &'a self,
        cells: Option<CellRange>,
        [min_x, max_x, min_z, max_z]: [f32; 4],
        hulls: HullFilter,
        mut f: impl FnMut(&'a Body),
        gather: impl FnOnce(&BodyGrid, CellRange, &mut Vec<u32>),
    ) {
        let mut seen = |body: &'a Body| {
            if hulls.admits(body.owner) {
                f(body);
            }
        };
        let hulls_only = matches!(
            hulls,
            HullFilter::Hulls | HullFilter::HullsExcept(_) | HullFilter::Only(_)
        );
        match cells {
            // A hull query visits no map body: skip the grid.
            _ if hulls_only => {}
            None => {
                for body in &self.bodies {
                    seen(body);
                }
            }
            Some(r) if r.x0 == r.x1 && r.z0 == r.z1 => {
                // A cell and `everywhere` are disjoint ascending lists. A
                // point query (notably ground rays) can merge them directly
                // instead of copying, sorting and deduplicating a scratch list.
                let cell = r.z0 * self.grid.nx + r.x0;
                let mut local = &self.grid.cells
                    [self.grid.starts[cell] as usize..self.grid.starts[cell + 1] as usize];
                let mut everywhere = self.grid.everywhere.as_slice();
                while let (Some(&a), Some(&b)) = (local.first(), everywhere.first()) {
                    if a < b {
                        seen(&self.bodies[a as usize]);
                        local = &local[1..];
                    } else {
                        seen(&self.bodies[b as usize]);
                        everywhere = &everywhere[1..];
                    }
                }
                for &i in local.iter().chain(everywhere) {
                    seen(&self.bodies[i as usize]);
                }
            }
            Some(r) => {
                let mut ids = BROADPHASE_SCRATCH.with(|s| s.take());
                ids.clear();
                gather(&self.grid, r, &mut ids);
                ids.sort_unstable();
                ids.dedup();
                for &i in &ids {
                    seen(&self.bodies[i as usize]);
                }
                BROADPHASE_SCRATCH.with(|s| s.set(ids));
            }
        }
        if hulls == HullFilter::Static {
            return;
        }
        for h in &self.hulls {
            for body in &h.bodies {
                let a = &body.aabb;
                // NaN footprints fail every compare and keep the body, as the
                // grid's full-scan fallback does.
                if a.max.x < min_x || a.min.x > max_x || a.max.z < min_z || a.min.z > max_z {
                    continue;
                }
                seen(body);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::{trimesh, Capsule, Layers, Shape};
    use glam::{Mat4, Vec3};

    // ---- the broad phase (`BodyGrid`) ------------------------------------

    /// A deterministic pseudo-random world: `n` bodies of all four shapes
    /// scattered over 400 x 400 m, sized 0.5 to 8 m, with a handful of long
    /// slabs so some bodies cross several cells. Nothing here is a PS2
    /// number — it is a fixture for the "the grid changes no answer" test.
    fn scattered(n: usize) -> Vec<Body> {
        let mut seed = 0x2545_f491u32;
        let mut next = || {
            // xorshift32 — reproducible, and no float transcendentals.
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let quad = |half: f32| {
            let p = |x: f32, z: f32| [x, 0.0, z];
            trimesh(
                &[
                    p(-half, -half),
                    p(half, -half),
                    p(half, half),
                    p(-half, half),
                ],
                &[0, 1, 2, 0, 2, 3],
            )
            .expect("mesh")
        };
        (0..n)
            .map(|i| {
                let at = Vec3::new(
                    next() * 400.0 - 200.0,
                    next() * 12.0 - 4.0,
                    next() * 400.0 - 200.0,
                );
                let m = Mat4::from_translation(at) * Mat4::from_rotation_y(next() * 6.0);
                let s = 0.5 + next() * 7.5;
                let shape = match i % 5 {
                    0 => Shape::Box {
                        half: Vec3::new(s, 1.0 + s, s),
                    },
                    1 => Shape::Sphere { radius: s },
                    2 => Shape::Cylinder {
                        radius: s * 0.5,
                        half_height: s,
                    },
                    3 => Shape::Mesh(quad(s)),
                    // A long slab: several cells wide, so the grid has to
                    // file one body in many of them.
                    _ => Shape::Box {
                        half: Vec3::new(s * 12.0, 4.0, 0.5),
                    },
                };
                let layers = if i % 7 == 0 {
                    Layers::ORDNANCE
                } else {
                    Layers::ALL
                };
                Body::with_layers(m, shape, layers)
            })
            .collect()
    }

    impl CollisionWorld {
        fn grid_is_live(&self) -> bool {
            self.grid.built_for == Some(self.generation)
        }
    }

    fn bits(o: Option<(f32, Vec3)>) -> Option<(u32, [u32; 3])> {
        o.map(|(t, n)| (t.to_bits(), [n.x.to_bits(), n.y.to_bits(), n.z.to_bits()]))
    }

    /// THE GATE THIS WHOLE STRUCTURE HAS TO PASS: the same world with the
    /// broad phase and without it answers every query with the SAME BITS.
    /// The control is `Default` + `push_body`, which leaves the
    /// grid stale and so makes every query take the full scan it took before
    /// the grid existed.
    #[test]
    fn the_broadphase_answers_bit_for_bit_what_the_full_scan_answers() {
        let gridded = CollisionWorld::new(scattered(400), Vec::new());
        let mut plain = CollisionWorld::default();
        for b in scattered(400) {
            plain.push_body(b);
        }
        assert!(gridded.grid_is_live(), "the grid is live");
        assert!(!plain.grid_is_live(), "the control full-scans");

        let dirs = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(-0.6, -0.3, 0.74),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.5, 0.5, 0.75),
        ];
        let mut checked = 0usize;
        let mut hits = 0usize;
        for gx in -10..=10 {
            for gz in -10..=10 {
                let at = Vec3::new(gx as f32 * 19.0, 0.0, gz as f32 * 19.0);
                let down = at + Vec3::Y * 40.0;
                assert_eq!(
                    bits(gridded.ray_down(down, 100.0)),
                    bits(plain.ray_down(down, 100.0)),
                    "ray_down at {at}"
                );
                hits += gridded.ray_down(down, 100.0).is_some() as usize;
                let cap = Capsule {
                    base: at,
                    height: 1.9,
                    radius: 0.5,
                };
                let (a, b) = (gridded.push_out(&cap), plain.push_out(&cap));
                assert_eq!(
                    a.to_array().map(f32::to_bits),
                    b.to_array().map(f32::to_bits)
                );
                for d in dirs {
                    let d = d.normalize();
                    let eye = at + Vec3::Y * 1.6;
                    for layers in [Layers::SOLDIER, Layers::ORDNANCE, Layers::ALL] {
                        assert_eq!(
                            bits(gridded.ray(eye, d, 60.0, layers)),
                            bits(plain.ray(eye, d, 60.0, layers)),
                            "ray at {at} {d} {layers:?}"
                        );
                        hits += gridded.ray(eye, d, 60.0, layers).is_some() as usize;
                    }
                    assert_eq!(
                        bits(gridded.wall_sweep(&cap, d, 1.0)),
                        bits(plain.wall_sweep(&cap, d, 1.0)),
                        "wall_sweep at {at} {d}"
                    );
                    hits += gridded.wall_sweep(&cap, d, 1.0).is_some() as usize;
                    checked += 1;
                }
            }
        }
        // A fixture that misses everything would prove nothing.
        assert!(checked >= 1_700, "{checked} query sites");
        assert!(hits > 500, "only {hits} of the queries hit anything");
    }

    /// DR2's movers: a body moved inside its envelope keeps the grid current
    /// (no rebuild — the grid is filed by the envelope) and every query
    /// answers bit for bit what a full scan of the moved world answers; a
    /// move OUTSIDE the envelope, or of a body that is no mover, rebuilds.
    #[test]
    fn a_mover_moves_and_the_broadphase_stays_exact() {
        let mut bodies = scattered(200);
        let slab = |y: f32| Mat4::from_translation(Vec3::new(3.0, y, -4.0));
        let door = || Shape::Box {
            half: Vec3::new(1.0, 2.0, 0.2),
        };
        bodies.push(Body::new(slab(2.0), door()));
        let i = bodies.len() - 1;
        let mut env = Aabb::empty();
        for y in [2.0, 6.0] {
            let a = Body::new(slab(y), door()).aabb;
            env.add(a.min);
            env.add(a.max);
        }
        let mut w = CollisionWorld::with_movers(bodies, Vec::new(), vec![(i, env)]);
        let before = w.grid.cells.clone();
        let eye = Vec3::new(3.0, 2.0, 10.0);
        assert!(
            w.ray(eye, Vec3::NEG_Z, 30.0, Layers::SOLDIER).is_some(),
            "shut"
        );
        w.move_body(i, slab(6.0));
        assert_eq!(w.grid.cells, before, "inside its envelope: no rebuild");
        assert!(w.grid_is_live(), "and the grid still serves");
        assert!(
            w.ray(eye, Vec3::NEG_Z, 30.0, Layers::SOLDIER).is_none(),
            "open"
        );
        let mut plain = CollisionWorld::default();
        for b in w.bodies() {
            plain.push_body(Body::with_layers(b.to_world, b.shape.clone(), b.layers));
        }
        for x in [-2.0f32, 1.0, 3.0, 5.0] {
            for y in [0.5f32, 2.0, 4.0, 6.0, 7.5] {
                let o = Vec3::new(x, y, 10.0);
                let d = Vec3::NEG_Z;
                let bits = |h: Option<(f32, Vec3)>| h.map(|(t, _)| t.to_bits());
                assert_eq!(
                    bits(w.ray(o, d, 30.0, Layers::ALL)),
                    bits(plain.ray(o, d, 30.0, Layers::ALL))
                );
            }
        }
        // Out of its envelope, far away in XZ: the grid is rebuilt and
        // files it where it now stands.
        w.move_body(i, Mat4::from_translation(Vec3::new(90.0, 2.0, 70.0)));
        let far = Vec3::new(90.0, 2.0, 80.0);
        assert!(w.ray(far, Vec3::NEG_Z, 30.0, Layers::SOLDIER).is_some());
        assert!(w.ray(eye, Vec3::NEG_Z, 30.0, Layers::SOLDIER).is_none());
    }

    /// And it does prune: a capsule-sized footprint sees a small fraction of
    /// the bodies, while the ORDNANCE ray's 60 m box sees more but not all.
    #[test]
    fn the_broadphase_hands_over_far_fewer_bodies_than_the_scan() {
        let w = CollisionWorld::new(scattered(400), Vec::new());
        let cap = Capsule {
            base: Vec3::new(7.0, 0.0, -13.0),
            height: 1.9,
            radius: 0.5,
        };
        let a = cap.aabb();
        let mut small = 0usize;
        w.visit_xz(a.min.x, a.max.x, a.min.z, a.max.z, HullFilter::All, |_| {
            small += 1
        });
        let mut wide = 0usize;
        w.visit_xz(-30.0, 30.0, -30.0, 30.0, HullFilter::All, |_| wide += 1);
        assert!(small < 60, "a capsule footprint saw {small} of 400");
        assert!(wide < 300, "a 60 m box saw {wide} of 400");
        assert!(small <= wide);
        // Every body is reachable: the whole map's footprint is everything.
        let mut all = 0usize;
        w.visit_xz(-1e6, 1e6, -1e6, 1e6, HullFilter::All, |_| all += 1);
        assert_eq!(all, 400, "the grid loses no body");
    }

    /// Ascending `bodies` order is what the tie-breaks and `push_out`'s
    /// running sum are defined on, so the candidate list must be sorted and
    /// free of the repeats a wide body would otherwise produce.
    #[test]
    fn the_candidates_come_out_ascending_and_once_each() {
        let bodies = scattered(200);
        let w = CollisionWorld::new(bodies, Vec::new());
        let ptr = |b: &Body| b as *const Body as usize;
        let base = ptr(&w.bodies[0]);
        let step = ptr(&w.bodies[1]) - base;
        for (x, z) in [(0.0, 0.0), (-90.0, 40.0), (150.0, -150.0), (1e5, 1e5)] {
            let mut seen: Vec<usize> = Vec::new();
            w.visit_xz(
                x - 25.0,
                x + 25.0,
                z - 25.0,
                z + 25.0,
                HullFilter::All,
                |b| seen.push((ptr(b) - base) / step),
            );
            let mut sorted = seen.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(seen, sorted, "at ({x}, {z})");
        }
    }

    /// Point queries must preserve the full gather/sort order even when
    /// map-spanning bodies are interleaved with the bodies filed in cells.
    #[test]
    fn single_cell_queries_match_gathered_candidates() {
        let huge = || {
            Body::new(
                Mat4::IDENTITY,
                Shape::Box {
                    half: Vec3::new(10_000.0, 1.0, 10_000.0),
                },
            )
        };
        let mut bodies = scattered(200);
        bodies.insert(37, huge());
        bodies.insert(101, huge());
        let w = CollisionWorld::new(bodies, Vec::new());
        assert!(w.grid.everywhere.contains(&37));
        assert!(w.grid.everywhere.contains(&101));
        let index = |b: &Body| {
            (b as *const Body as usize - w.bodies.as_ptr() as usize) / std::mem::size_of::<Body>()
        };
        let mut local_hits = 0;
        for (x, z) in [(0.0, 0.0), (-90.0, 40.0), (150.0, -150.0), (1e5, 1e5)] {
            let r = w.grid.cells_for(w.generation, x, x, z, z).unwrap();
            let mut expected = Vec::new();
            w.grid.gather(r, &mut expected);
            expected.sort_unstable();
            expected.dedup();
            local_hits += expected.len() - w.grid.everywhere.len();
            let mut actual = Vec::new();
            w.visit_xz(x, x, z, z, HullFilter::Static, |b| {
                actual.push(index(b) as u32);
                // A callback may make another query on the same thread.
                let mut nested = Vec::new();
                w.visit_xz(x, x, z, z, HullFilter::Static, |b| {
                    nested.push(index(b) as u32);
                });
                assert_eq!(nested, expected);
            });
            assert_eq!(actual, expected, "at ({x}, {z})");
        }
        assert!(local_hits > 0, "exercise both merged lists");
    }

    /// A fixture can still push after construction — and then the grid is
    /// stale and every query falls back to the full scan, which is why that
    /// is correct rather than merely slow.
    #[test]
    fn a_body_pushed_after_construction_is_still_seen() {
        let mut w = CollisionWorld::new(Vec::new(), Vec::new());
        assert!(w.grid_is_live());
        w.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        let eye = Vec3::new(0.0, 0.0, 0.0);
        let stale = w.ray(eye, Vec3::NEG_Z, 30.0, Layers::SOLDIER);
        assert!(!w.grid_is_live());
        assert!(stale.is_some(), "the stale grid must not hide the wall");
        w.rebuild_broadphase();
        assert!(w.grid_is_live());
        assert_eq!(
            bits(stale),
            bits(w.ray(eye, Vec3::NEG_Z, 30.0, Layers::SOLDIER)),
            "and rebuilding it changes nothing"
        );
    }

    /// A body the grid cannot place — an empty AABB, a non-finite one, or one
    /// spanning more than `BROADPHASE_MAX_CELLS_PER_BODY` cells — goes in
    /// `everywhere` and is handed to every query.
    #[test]
    fn an_unplaceable_body_is_handed_to_every_query() {
        // 20 km on a side at 32 m cells is far past the per-body cell cap.
        let huge = Body::new(
            Mat4::IDENTITY,
            Shape::Box {
                half: Vec3::new(10_000.0, 1.0, 10_000.0),
            },
        );
        let near = Body::new(
            Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)),
            Shape::Sphere { radius: 1.0 },
        );
        let w = CollisionWorld::new(vec![huge, near], Vec::new());
        assert_eq!(
            w.grid.everywhere,
            vec![0],
            "the map-wide slab is in no cell"
        );
        let mut seen = 0usize;
        w.visit_xz(9_000.0, 9_001.0, 9_000.0, 9_001.0, HullFilter::All, |_| {
            seen += 1
        });
        assert_eq!(seen, 1, "and is still offered 9 km away");
    }

    /// The grid is a size hint, not a bound: a query far outside it clamps to
    /// the edge cell, which can only ADD candidates, never drop one.
    #[test]
    fn a_query_outside_the_grid_is_clamped_not_lost() {
        let w = CollisionWorld::new(scattered(64), Vec::new());
        let far = Vec3::new(50_000.0, 0.0, 50_000.0);
        assert_eq!(
            bits(w.ray_down(far + Vec3::Y * 40.0, 100.0)),
            None,
            "nothing is out there"
        );
        // A NaN footprint is refused outright and takes the full scan.
        let mut nan_seen = 0usize;
        w.visit_xz(f32::NAN, 1.0, 0.0, 1.0, HullFilter::All, |_| nan_seen += 1);
        assert_eq!(nan_seen, 64);
    }

    /// The cells a segment walks hold every body whose box it meets, in the
    /// same order as reading its whole bounding box, on rays of every slant.
    #[test]
    fn a_segment_is_offered_every_body_its_bounding_box_would_offer_that_it_meets() {
        let w = CollisionWorld::new(scattered(600), Vec::new());
        let mut seed = 0x1234_5679u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let (mut read, mut boxed) = (0usize, 0usize);
        for k in 0..4000 {
            let span = if k % 3 == 0 { 2000.0 } else { 500.0 };
            let at = |n: f32| (n - 0.5) * span;
            let from = Vec3::new(at(next()), 5.0 * next(), at(next()));
            let mut to = Vec3::new(at(next()), 5.0 * next(), at(next()));
            // Along the axes too, and almost along them.
            match k % 7 {
                0 => to.x = from.x,
                1 => to.z = from.z,
                2 => to.x = from.x + 1e-4,
                3 => to.z = from.z + 1e-4,
                _ => {}
            }
            let (d, len) = ((to - from).normalize(), (to - from).length());
            let mut seg = Aabb::empty();
            seg.add(from);
            seg.add(to);
            let mut all = Vec::new();
            w.visit_xz(
                seg.min.x,
                seg.max.x,
                seg.min.z,
                seg.max.z,
                HullFilter::All,
                |b| {
                    all.push(b as *const Body);
                    boxed += 1;
                },
            );
            let mut walked = Vec::new();
            w.visit_segment(from, d, len, HullFilter::All, |b| {
                walked.push(b as *const Body);
                read += 1;
            });
            let order = |v: &Vec<*const Body>| v.windows(2).all(|p| p[0] < p[1]);
            assert!(order(&walked), "not ascending at ray {k}");
            for b in &all {
                // SAFETY: a pointer to a body of the live world, just taken.
                let body = unsafe { &**b };
                if body.aabb.hits_ray(from, d, len) {
                    assert!(walked.contains(b), "ray {k} lost a body its line meets");
                }
            }
        }
        assert!(read * 2 < boxed, "the walk read {read} of {boxed}");
    }

    /// A ray that stops looking past its best hit, and a line of sight that
    /// stops at its first, answer as casting every body the whole way does.
    #[test]
    fn a_shortened_ray_answers_as_the_whole_ray_does() {
        let w = CollisionWorld::new(scattered(600), Vec::new());
        let mut seed = 0x0bad_cafeu32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut hits = 0;
        for _ in 0..4000 {
            let at = |n: f32| (n - 0.5) * 300.0;
            let from = Vec3::new(at(next()), 8.0 * next(), at(next()));
            let to = Vec3::new(at(next()), 8.0 * next(), at(next()));
            let (d, len) = ((to - from).normalize(), (to - from).length());
            let mut whole: Option<(f32, [f32; 3])> = None;
            for b in &w.bodies {
                if !b.layers.contains(Layers::ORDNANCE) {
                    continue;
                }
                let o = b.to_local.transform_point3(from);
                let v = b.to_local.transform_vector3(d);
                let ray = parry3d::query::Ray::new(
                    crate::collision::point(o),
                    crate::collision::vector(v),
                );
                if let Some(h) = b.shape.cast_local(&ray, len) {
                    if whole.is_none_or(|(t, _)| h.time_of_impact < t) {
                        whole = Some((h.time_of_impact, [h.normal.x, h.normal.y, h.normal.z]));
                    }
                }
            }
            let got = w.ray_with(from, d, len, Layers::ORDNANCE, HullFilter::All);
            assert_eq!(got.map(|h| h.t.to_bits()), whole.map(|h| h.0.to_bits()));
            assert_eq!(
                w.ray_blocked(from, d, len, Layers::ORDNANCE, HullFilter::All, [None; 2]),
                whole.is_some()
            );
            hits += usize::from(whole.is_some());
        }
        assert!(hits > 500, "only {hits} rays hit anything");
    }
}
