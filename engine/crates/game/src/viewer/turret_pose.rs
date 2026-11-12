//! A building turret drawn turned to its aim (card VTURD,
//! `analysis/re-turret-draw.md` 2 [O]): the node matrices the PS2 writes for
//! a turret, from the seat's `GunState`.
//!
//! The PS2 keeps one whole model-space matrix per named node and draws each
//! segment through its node's; a node nobody writes stays at its bind matrix
//! and **nothing composes parents** (2.4). Three kinds of node are written:
//!
//! - **The `TurretNodeName` node** (2.2, B1 FUN_00223850, B2 FUN_0026cea0):
//!   a look-along matrix of `(yaw, pitch × PitchTurnFactor)` at its bind
//!   origin, its up carried from the bind Y. The PS2's turret pitch is nose
//!   DOWN for a positive angle; the port's look pitch is up, so it goes in
//!   negated. `YawTurnFactor` is on neither disc (1.0).
//! - **The `AimerNodeName` node** (2.3, B1 FUN_0026ce10, B2 FUN_003018d8):
//!   a look-along matrix of the AIMER's own direction - the port's
//!   `GunState::aim`, not the camera's look - with its origin and up taken
//!   from its bind transform relative to its parent, through the parent's
//!   matrix as it is now.
//! - **A `BarrelNodeName` node**: its parent's matrix, moved to the barrel's
//!   bind offset from that parent, less `state × BarrelRecoil` along z while
//!   the state is above 0. A shot sets the next barrel's state to 1 and it
//!   falls by 4 a second.
//!
//! The port bakes a turret's parts in model space at the bind pose
//! (`bake::turret_rig`), so a part on node n is drawn at
//! `instance · node(n) · bind(n)⁻¹` ([`part_matrices`]).
//!
//! Matrices here are glam's (columns X, Y, Z, origin), which is the PS2's
//! row matrix read as columns: "M1 x M2" there is `M2 * M1` here.

use crate::vehicle_class::SeatSpec;
use glam::{Mat4, Vec3};
use manifest::ManifestTurretRig;
use render::DrawCmd;

/// A barrel's recoil state falls by this much a second (B1:192018-192023).
pub const RECOIL_DECAY: f32 = 4.0;

/// The `TurretNodeName` node: `bind` is its model-space bind matrix, `yaw`
/// turns +z toward +x, `pitch_down` is the PS2's (positive = nose down),
/// both already scaled by their turn factors.
pub use core::vehicle::gun::turret_node;

/// The aimer and barrel nodes (the math is the sim's too, card TCOL).
pub use core::vehicle::gun::{aimer_node, barrel_node};

/// A rig node's model-space bind matrix.
pub fn bind_of(rig: &ManifestTurretRig, n: usize) -> Mat4 {
    let b = &rig.nodes[n].bind;
    Mat4::from_cols(
        Vec3::from(b[0]).extend(0.0),
        Vec3::from(b[1]).extend(0.0),
        Vec3::from(b[2]).extend(0.0),
        Vec3::from(b[3]).extend(1.0),
    )
}

/// Which of a rig's nodes a seat's section moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoseSpec {
    pub turret: Option<usize>,
    /// `PitchTurnFactor` (class default 1.0).
    pub pitch_factor: f32,
    pub aimer: Option<usize>,
    /// (node, `BarrelRecoil`).
    pub barrels: Vec<(usize, f32)>,
}

impl PoseSpec {
    /// The seat's node names found in `rig` (a name the model lacks moves
    /// nothing, as on the PS2).
    pub fn of(rig: &ManifestTurretRig, seat: &SeatSpec) -> PoseSpec {
        let find = |n: &str| {
            rig.nodes
                .iter()
                .position(|x| x.name.eq_ignore_ascii_case(n.trim()))
        };
        PoseSpec {
            turret: seat.turret_node.as_deref().and_then(find),
            pitch_factor: seat.pitch_turn_factor.unwrap_or(1.0),
            aimer: seat.aimer_node.as_deref().and_then(find),
            barrels: seat
                .barrels
                .iter()
                .filter_map(|(n, r)| Some((find(n)?, *r)))
                .collect(),
        }
    }
}

/// Every node's model-space matrix with the seat's camera at `look` and its
/// aimer at `aim` (the port's angles: yaw turns +z toward +x, pitch > 0 is
/// up) and each barrel's recoil state in `recoil` (missing = at rest).
pub fn pose(
    rig: &ManifestTurretRig,
    spec: &PoseSpec,
    look: (f32, f32),
    aim: (f32, f32),
    recoil: &[f32],
) -> Vec<Mat4> {
    core_rig(rig, spec).pose(look, aim, recoil)
}

/// The rig and the seat's nodes as the sim keeps them
/// ([`core::vehicle::gun::Rig`]).
pub fn core_rig(rig: &ManifestTurretRig, spec: &PoseSpec) -> core::vehicle::gun::Rig {
    core::vehicle::gun::Rig {
        binds: (0..rig.nodes.len()).map(|n| bind_of(rig, n)).collect(),
        parents: rig
            .nodes
            .iter()
            .map(|n| n.parent.map(|p| p as usize))
            .collect(),
        turret: spec.turret,
        pitch_factor: spec.pitch_factor,
        aimer: spec.aimer,
        barrels: spec.barrels.clone(),
    }
}

/// Per drawn part of the rig's model, what moves it from where it was baked
/// (model space, the bind pose): `node(now) · bind⁻¹` of its node, `None`
/// for a part bound to no node.
pub fn part_matrices(rig: &ManifestTurretRig, now: &[Mat4]) -> Vec<Option<Mat4>> {
    rig.parts
        .iter()
        .map(|p| {
            let n = (*p)? as usize;
            Some(now[n] * bind_of(rig, n).inverse())
        })
        .collect()
}

/// Where a seat's soldier hangs (card VTURG, `analysis/re-turret-draw.md`
/// 3.2 [O]): the PS2 hands him `L x E x O` - `L` the `PilotPosition`
/// hardpoint's bind matrix relative to its PARENT node, `E` that parent's
/// matrix now, `O` the building's (B1:156190-156203, B2:285353-285366).
/// On the gun turrets, the dish and the towers the parent is the turret
/// node (he swings with the yaw and stays upright), on the anti-aircraft
/// guns the aimer node (he tilts with the barrels), on any other node its
/// bind matrix (he does not move).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunnerHang {
    /// The hardpoint's parent node; `None` for a root hardpoint.
    pub parent: Option<usize>,
    /// `L`: the hardpoint's bind matrix relative to its parent.
    pub local: Mat4,
}

impl GunnerHang {
    /// The seat's `PilotPosition` in `rig`; `None` when the seat names none
    /// or the model has no such node.
    pub fn of(rig: &ManifestTurretRig, seat: &SeatSpec) -> Option<GunnerHang> {
        let name = seat.pilot_position.as_deref()?.trim();
        let n = rig
            .nodes
            .iter()
            .position(|x| x.name.eq_ignore_ascii_case(name))?;
        let parent = rig.nodes[n].parent.map(|p| p as usize);
        let local = match parent {
            Some(p) => bind_of(rig, p).inverse() * bind_of(rig, n),
            None => bind_of(rig, n),
        };
        Some(GunnerHang { parent, local })
    }

    /// The soldier's world matrix: the building at `model`, the rig's nodes
    /// at `now` ([`pose`]). The soldier model faces +z, as the hardpoint.
    pub fn matrix(&self, model: Mat4, now: &[Mat4]) -> Mat4 {
        let e = self.parent.and_then(|p| now.get(p)).copied();
        model * e.unwrap_or(Mat4::IDENTITY) * self.local
    }
}

/// The three numbers a nine-pose seat poses its soldier by
/// (`analysis/re-turret-draw.md` 3.3 [O], B1:137688-137730,
/// B2:189522-189566): the pose is the clip's frame `a` blended toward frame
/// `b` by `c`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NinePose {
    pub a: f32,
    pub b: f32,
    pub c: f32,
}

/// [`NinePose`] of a turret at the PS2's angles: `pitch_down` (rad, positive
/// = nose DOWN: the port's look pitch negated) and `yaw` (rad, +z toward
/// +x: the port's look yaw), the class's `PitchLimits` / `YawLimits` as
/// authored (degrees, "lo hi"; absent = 0 0), and `stick` for a turret whose
/// yaw range is not inside (0, 270 degrees): BF1's raw turn stick, BF2's yaw
/// rate over its turn speed, -1 .. 1 (the PS2 turns the yaw against the
/// stick's sign - on the rows of `lead_tmp/vturd/ps2` with the stick past
/// 0.5, 147 BF1 and 230 BF2 rows turn against it and none with it - so for
/// one turn the two games' numbers have opposite signs).
///
/// ```text
/// pf = clamp(((pitch - pLo) / (pHi - pLo) - 0.5) x 2, -1, 1), 0.5 when pLo >= pHi
/// yf = the same of the yaw when 0 < yHi - yLo < 4.712389, else the stick
/// a = yf + 4    b = yf + (pf > 0 ? 7 : 1)    c = |pf|
/// ```
pub fn nine_pose(
    pitch_down: f32,
    yaw: f32,
    stick: f32,
    pitch_limits: Option<[f32; 2]>,
    yaw_limits: Option<[f32; 2]>,
) -> NinePose {
    let rad = |l: Option<[f32; 2]>| l.map_or([0.0; 2], |l| l.map(f32::to_radians));
    let place = |x: f32, [lo, hi]: [f32; 2]| (((x - lo) / (hi - lo) - 0.5) * 2.0).clamp(-1.0, 1.0);
    let p = rad(pitch_limits);
    let pf = if p[0] >= p[1] {
        0.5
    } else {
        place(pitch_down, p)
    };
    let y = rad(yaw_limits);
    let yf = if y[1] - y[0] > 0.0 && y[1] - y[0] < 4.712389 {
        place(yaw, y)
    } else {
        stick.clamp(-1.0, 1.0)
    };
    NinePose {
        a: yf + 4.0,
        b: yf + if pf > 0.0 { 7.0 } else { 1.0 },
        c: pf.abs(),
    }
}

/// A hover's rider numbers (card VRIDE, `analysis/re-vehicle-riders.md` 4.1
/// [O]): the drive smooths the throttle into `p` and the turn into `y` at
/// rate 5 a second while the hover has a pilot (B1 `FUN_001ec768`
/// B1:99598-99626, B2 `FUN_0021d8b8` B2:134693-134723), both 0 from the
/// constructor (B1:96395-96396, B2:130826-130827; it writes -1 only to a, b
/// and c: `analysis/re-vehicle-pose.md` 0.6) and left as they are when he
/// leaves. A nine-pose pilot seat (the speeder bikes, the BARC) is posed
/// by [`HoverNine::pose`]. Visual only: the sim never reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverNine {
    pub p: f32,
    pub y: f32,
}

impl HoverNine {
    /// The constructor's numbers.
    pub const START: HoverNine = HoverNine { p: 0.0, y: 0.0 };

    /// One drive update of `dt` seconds with the pilot's throttle, strafe
    /// and turn (the PS2's signs, the port's `HoverInput`). The turn the
    /// drive smooths: BF2 `clamp(trn + 0.5 str)`; BF1 `clamp(trn - str)`
    /// after its caller turns a strafe into a turn for a class with no
    /// `StrafeSpeed` (`no_strafe`): `trn = trn (1 - |str|) + str, str = 0`
    /// (B1:97668-97680).
    pub fn step(self, [thr, str_, trn]: [f32; 3], no_strafe: bool, bf2: bool, dt: f32) -> Self {
        let turn = if bf2 {
            trn + 0.5 * str_
        } else if no_strafe && str_ != 0.0 {
            trn * (1.0 - str_.abs()) + str_
        } else {
            trn - str_
        };
        HoverNine {
            p: self.p + (thr - self.p) * dt * 5.0,
            y: self.y + (turn.clamp(-1.0, 1.0) - self.y) * dt * 5.0,
        }
    }

    /// `a = y + 4`, `b = y + (p > 0 ? 1 : 7)`, `c = |p|`: he leans with the
    /// turn and crouches with the throttle (frames 0-2 full ahead, 3-5 at
    /// rest, 6-8 reversing).
    pub fn pose(self) -> NinePose {
        NinePose {
            a: self.y + 4.0,
            b: self.y + if self.p > 0.0 { 1.0 } else { 7.0 },
            c: self.p.abs(),
        }
    }
}

/// Per `Sim::vehicles` slot, each live hover's [`HoverNine`]: one drive
/// update of the tick's length per tick while it has a pilot, with his input;
/// a fresh vehicle starts at [`HoverNine::START`]. The rider's pose and the
/// hover's own fins read the same numbers (card VPOSE).
#[derive(Clone, Debug, Default)]
pub struct HoverClocks(Vec<Option<HoverClock>>);

#[derive(Clone, Copy, Debug)]
struct HoverClock {
    gen: u32,
    tick: u64,
    nine: HoverNine,
}

impl HoverClocks {
    /// Bring every hover's numbers to `sim`'s tick.
    pub fn step(&mut self, sim: &core::sim::Sim, bf2: bool) {
        let tick = sim.tick;
        let vs = &sim.vehicles;
        self.0.resize(vs.len(), None);
        for (slot, v) in vs.iter().enumerate() {
            if !v.alive || v.kind != core::vehicle::VehicleKind::Hover {
                self.0[slot] = None;
                continue;
            }
            let fresh = HoverClock {
                gen: v.gen,
                tick,
                nine: HoverNine::START,
            };
            let c = self.0[slot].get_or_insert(fresh);
            if c.gen != v.gen {
                *c = fresh;
            }
            let piloted = v.pilot_seat().is_some_and(|s| s.occupant.is_some());
            if piloted {
                let no_strafe = sim
                    .hover_params
                    .get(v.class as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|p| p.strafe_speed < 1e-4);
                let i = v.input;
                for _ in 0..tick.saturating_sub(c.tick).min(30) {
                    c.nine = c.nine.step(
                        [i.throttle, i.strafe, i.turn],
                        no_strafe,
                        bf2,
                        core::TICK_DT,
                    );
                }
            }
            c.tick = tick;
        }
    }

    /// Slot `slot`'s numbers, when they were stepped for generation `gen`.
    pub fn get(&self, slot: usize, gen: u32) -> Option<HoverNine> {
        self.0
            .get(slot)
            .copied()
            .flatten()
            .filter(|c| c.gen == gen)
            .map(|c| c.nine)
    }
}

/// One placed armed building whose class has a node rig, without a
/// renderer (card VTURG: where a seated gunner hangs): the sim's turret
/// stands at `pos`, the instance's translation, as [`TurretDraw::pos`].
#[derive(Clone, Debug)]
pub struct RiggedTurret {
    pub pos: Vec3,
    pub model: Mat4,
    pub class: String,
    pub rig: ManifestTurretRig,
}

/// Every placed building of `scene` with a rig, in manifest order (the
/// instances `SceneGpu` draws with one).
pub fn rigged_turrets(scene: &crate::assets::Scene) -> Vec<RiggedTurret> {
    scene
        .manifest
        .instances
        .iter()
        .filter_map(|inst| {
            let class = scene.resolve_class(&inst.class)?;
            // A droid's rig is its own animation's (card PWST).
            if class
                .base
                .eq_ignore_ascii_case(super::stations::STATION_BASE)
            {
                return None;
            }
            let rig = class.turret_rig.as_ref()?;
            Some(RiggedTurret {
                pos: Vec3::from(inst.translation),
                model: super::instance_matrix(inst.rotation, inst.translation),
                class: inst.class.clone(),
                rig: rig.clone(),
            })
        })
        .collect()
}

/// A barrel's recoil state `since` seconds after its shot: 1 at the shot,
/// falling by [`RECOIL_DECAY`] a second (the PS2 lets it run below 0, where
/// it slides nothing).
pub fn recoil_state(since: f32) -> f32 {
    1.0 - RECOIL_DECAY * since
}

/// One placed armed building whose class has a node rig (card VTURD,
/// this module): the manifest instance, its matrix and class, and
/// per part bound to a node the draws it pushed (the base draw and its
/// specular pass) with the part's index in the rig.
pub struct TurretDraw {
    pub instance: usize,
    /// The instance's class as the manifest spells it.
    pub class: String,
    /// The instance's translation: the sim's turret stands exactly there
    /// (`crate::vehicle_pads::add_turrets`), which is how the two are joined.
    pub pos: Vec3,
    model: Mat4,
    rig: ManifestTurretRig,
    /// (the part's draws in `SceneGpu::draws`, its index in the rig).
    pub(super) parts: Vec<(std::ops::Range<usize>, usize)>,
    /// Every draw the building pushed, posed or not (card VTURZ: what a
    /// zoomed gunner's own view leaves out).
    pub whole: std::ops::Range<usize>,
    /// The seat's weapons' shots counted so far, the barrel the next shot
    /// kicks, and per barrel the tick of its last shot.
    shots: u64,
    next_barrel: usize,
    kicked: Vec<Option<u64>>,
}

impl TurretDraw {
    /// A building as uploaded: at the bind pose, no shot seen.
    pub(super) fn new(
        instance: usize,
        inst: &manifest::ManifestInstance,
        model: Mat4,
        rig: &ManifestTurretRig,
        parts: Vec<(std::ops::Range<usize>, usize)>,
        whole: std::ops::Range<usize>,
    ) -> TurretDraw {
        TurretDraw {
            whole,
            instance,
            class: inst.class.clone(),
            pos: Vec3::from(inst.translation),
            model,
            rig: rig.clone(),
            parts,
            shots: 0,
            next_barrel: 0,
            kicked: Vec::new(),
        }
    }

    /// The rig of the instance's class.
    pub fn rig(&self) -> &ManifestTurretRig {
        &self.rig
    }

    /// Write the pose of `look` / `aim` / `recoil` into this building's draws.
    fn apply(
        &self,
        draws: &mut [DrawCmd],
        spec: &PoseSpec,
        look: (f32, f32),
        aim: (f32, f32),
        recoil: &[f32],
    ) {
        let now = pose(&self.rig, spec, look, aim, recoil);
        let moved = part_matrices(&self.rig, &now);
        for (range, k) in &self.parts {
            if let Some(Some(d)) = moved.get(*k) {
                let m = self.model * *d;
                for i in range.clone() {
                    draws[i].model = m;
                }
            }
        }
    }

    /// The model matrix of the first draw of rig part `k` (a test's and a
    /// census's read of what is drawn), `None` for a part with no draw.
    pub fn part_model(&self, draws: &[DrawCmd], k: usize) -> Option<Mat4> {
        let (range, _) = self.parts.iter().find(|(_, p)| *p == k)?;
        Some(draws[range.start].model)
    }
}

impl super::SceneGpu {
    /// Pose every drawn armed building from its sim turret (card VTURD,
    /// this module): the node matrices of the seat's `GunState` -
    /// the turret node from the look, the aimer node from the aim, a barrel
    /// slid back for a quarter second after its shot - and each part at
    /// `instance · node · bind⁻¹`. `seats` is each vehicle class's ring
    /// (`WalkSim::vehicle_seats`). A building with no live sim turret (a
    /// self-firing class, an unbuilt one, a dead one) keeps its last pose.
    pub fn pose_turrets(
        &mut self,
        sim: &core::sim::Sim,
        seats: &[Vec<crate::vehicle_class::SeatSpec>],
    ) {
        for v in &sim.vehicles {
            if !v.alive || v.kind != core::vehicle::VehicleKind::Turret {
                continue;
            }
            let Some(td) = self.turrets.iter_mut().find(|t| t.pos == v.pos) else {
                continue;
            };
            // The gunner seat with a gun, and its section's node names.
            let Some((seat, gun)) = v
                .guns
                .iter()
                .enumerate()
                .find_map(|(i, g)| Some((i, g.as_ref()?)))
            else {
                continue;
            };
            let Some(section) = seats.get(v.class as usize).and_then(|r| r.get(seat)) else {
                continue;
            };
            let spec = PoseSpec::of(&td.rig, section);
            // A shot kicks the next barrel (B1:191494).
            let shots: u64 = gun.weapons.iter().map(|w| w.shots_fired).sum();
            td.kicked.resize(spec.barrels.len(), None);
            let n = td.kicked.len();
            if n > 0 {
                for _ in td.shots..shots.min(td.shots + n as u64) {
                    td.kicked[td.next_barrel % n] = Some(sim.tick);
                    td.next_barrel = (td.next_barrel + 1) % n;
                }
            }
            td.shots = shots;
            let recoil: Vec<f32> = td
                .kicked
                .iter()
                .map(|k| match k {
                    Some(t) => recoil_state(sim.tick.saturating_sub(*t) as f32 * core::TICK_DT),
                    None => 0.0,
                })
                .collect();
            td.apply(&mut self.draws, &spec, gun.look, gun.aim, &recoil);
        }
    }

    /// The draws of the building `viewer` sits in zoomed, which that
    /// viewer's own view leaves out (card VTURZ, `analysis/re-turret-draw.md`
    /// 4 [O]: the armed building's draw returns before its table, pose and
    /// model when the manned turret is zoomed in that player's own view, B1
    /// `FUN_001c4878` / `FUN_00225260`, B2 `FUN_001e81e0` / `FUN_0026f0e0`).
    /// Another viewer's view still draws it; the gunner himself is drawn
    /// in every view. `None` = nothing left out.
    pub fn hidden_for(
        &self,
        sim: &core::sim::Sim,
        viewer: core::unit::UnitHandle,
    ) -> Option<std::ops::Range<usize>> {
        sim.seat_zoom(viewer)?;
        let v = sim.vehicle(sim.unit(viewer)?.mount?.vehicle)?;
        if !v.alive || v.kind != core::vehicle::VehicleKind::Turret {
            return None;
        }
        Some(self.turrets.iter().find(|t| t.pos == v.pos)?.whole.clone())
    }

    /// The scene's draws as `viewer` sees them: [`SceneGpu::draws`] less
    /// [`SceneGpu::hidden_for`], with every fallen building turret's look in
    /// place of its intact draws (card TBLD, `super::turret_fall`).
    pub fn view_draws(&self, sim: &core::sim::Sim, viewer: core::unit::UnitHandle) -> Vec<DrawCmd> {
        let (mut hidden, fallen) = self.fallen_view(sim);
        hidden.extend(self.hidden_for(sim, viewer));
        self.station_view_draws(sim, hidden, fallen)
    }

    /// Pose drawn armed buildings at one `look` and `aim` (the port's yaw,
    /// pitch), barrels at rest - the picture tool's way to turn a turret
    /// without a battle: the one nearest `near`, or all of them. `section`
    /// gives a class's gunner section by the class name the manifest spells.
    pub fn pose_turrets_at(
        &mut self,
        near: Option<Vec3>,
        section: impl Fn(&str) -> Option<crate::vehicle_class::SeatSpec>,
        look: (f32, f32),
        aim: (f32, f32),
    ) {
        let nearest = near.and_then(|p| {
            (0..self.turrets.len()).min_by(|&a, &b| {
                let d = |i: usize| self.turrets[i].pos.distance_squared(p);
                d(a).total_cmp(&d(b))
            })
        });
        for (i, td) in self.turrets.iter().enumerate() {
            if near.is_some() && nearest != Some(i) {
                continue;
            }
            if let Some(seat) = section(&td.class) {
                let spec = PoseSpec::of(&td.rig, &seat);
                td.apply(&mut self.draws, &spec, look, aim, &[]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manifest::ManifestRigNode;

    fn node(name: &str, parent: Option<u32>, at: [f32; 3]) -> ManifestRigNode {
        ManifestRigNode {
            name: name.into(),
            parent,
            bind: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], at],
        }
    }

    fn rig() -> ManifestTurretRig {
        ManifestTurretRig {
            nodes: vec![
                node("base", None, [0.0; 3]),
                node("aimer_y", Some(0), [0.0, 2.0, 0.0]),
                node("aimer_x", Some(1), [0.0, 3.0, 0.5]),
                node("barrel1", Some(2), [0.4, 3.0, 2.0]),
            ],
            parts: vec![Some(0), Some(1), Some(2), Some(3), None],
        }
    }

    fn spec() -> PoseSpec {
        PoseSpec {
            turret: Some(1),
            pitch_factor: 0.0,
            aimer: Some(2),
            barrels: vec![(3, 0.5)],
        }
    }

    /// At rest every node is its bind matrix, so no part moves.
    #[test]
    fn at_rest_the_pose_is_the_bind_pose() {
        let r = rig();
        let now = pose(&r, &spec(), (0.0, 0.0), (0.0, 0.0), &[]);
        for (m, p) in part_matrices(&r, &now).iter().zip(&r.parts) {
            assert_eq!(m.is_some(), p.is_some());
            if let Some(m) = m {
                assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6), "{m:?}");
            }
        }
    }

    /// The turret node takes the yaw and (PitchTurnFactor 0) no pitch; the
    /// aimer node looks along the aim from a place that swung with the
    /// turret node; the barrel rides the aimer and slides back along it.
    #[test]
    fn a_turned_turret_carries_its_aimer_and_barrel() {
        let r = rig();
        let q = std::f32::consts::FRAC_PI_2;
        let now = pose(&r, &spec(), (q, 0.5), (q, 0.3), &[1.0]);
        // Yaw 90 degrees: +z has gone to +x, and the node did not tilt.
        assert!(now[1].z_axis.truncate().abs_diff_eq(Vec3::X, 1e-6));
        assert!(now[1].y_axis.truncate().abs_diff_eq(Vec3::Y, 1e-6));
        // The aimer's origin (0, 1, 0.5 from the turret node) swung round.
        assert!(now[2]
            .w_axis
            .truncate()
            .abs_diff_eq(Vec3::new(0.5, 3.0, 0.0), 1e-5));
        let f = core::vehicle::gun::dir(q, 0.3);
        assert!(now[2].z_axis.truncate().abs_diff_eq(f, 1e-6));
        assert!(f.y > 0.29, "the port's aim pitch is up");
        // The barrel: the aimer's axes, 0.5 m back along them at state 1.
        assert!(now[3].z_axis.abs_diff_eq(now[2].z_axis, 1e-6));
        let rest = pose(&r, &spec(), (q, 0.5), (q, 0.3), &[-3.0]);
        let back = rest[3].w_axis.truncate() - now[3].w_axis.truncate();
        assert!(back.abs_diff_eq(0.5 * f, 1e-5), "{back:?}");
        // Full pitch on the node when the class's factor is 1.
        let tilt = PoseSpec {
            pitch_factor: 1.0,
            ..spec()
        };
        let now = pose(&r, &tilt, (0.0, 0.5), (0.0, 0.5), &[]);
        assert!((now[1].z_axis.y - 0.5f32.sin()).abs() < 1e-6);
    }

    #[test]
    fn a_recoil_state_falls_by_four_a_second() {
        assert_eq!(recoil_state(0.0), 1.0);
        assert!((recoil_state(0.125) - 0.5).abs() < 1e-6);
        assert!(recoil_state(1.0) < 0.0);
    }
}
