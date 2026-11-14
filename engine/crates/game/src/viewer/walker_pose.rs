//! A walker drawn alive (card WSKN, `analysis/re-walker.md` §5, §6, §10):
//! its skeleton posed from the integrator's anim state, each tick.
//!
//! - **The clip** is the anim-table entry `WalkerState::anim` at
//!   `clip_time` (the integrator's player position, reversed walks
//!   included), sampled between keyframes as a DRAWN PS2 walker is (§5).
//!   DummyRoot's translation is held at rest by the bake: its z is the root
//!   motion the integrator already moved the walker by (§4.9).
//! - **The leg layers** (FUN_002a1d00): each leg pair's P+0x120 blend b
//!   weighs the entry's `_leftup` layer by −b when b < 0, its `_rightup`
//!   layer by b when b > 0, over the subtrees of the pair's two layer bones
//!   (the class entry's +0x28 / +0x30 masks, FUN_002668c0). Only the AT-AT
//!   ships layer clips. PROVISIONAL: the layer is a per-bone lerp from the
//!   base pose by its weight — the player's own mix (+0x1340) is not read.
//! - **§10's bones** (FUN_0025f050): bone 0 is yawed by the legs' 2h, the
//!   quaternion (0, sin h, 0, cos h), so the model turns with the legs while
//!   the entity matrix stays the body's; `bone_root` (the name hashing to
//!   0x16e27226 — the anim CRC-32 of "bone_root") takes SkeletonRootScale,
//!   the pitch (sin a, 0, 0, cos a) and the roll (0, 0, R.s, R.c).
//!   PROVISIONAL: the order of those products (every shipped clip keeps
//!   both bones' rotations at identity, where the order does not show).
//!
//! The pose is skinned on the CPU (one bone per vertex, the disc's
//! softskin) into the model's per-submesh dynamic meshes, so a walker keeps
//! the world path's passes, lighting and specular (`super::vehicles`).

use crate::skin::{Clip, Pose, Skeleton};
use core::vehicle::walker::WalkerState;
use glam::{Mat4, Quat, Vec3};

/// The anim CRC-32 of the pitch/roll bone's name (§10).
pub const BONE_ROOT: &str = "bone_root";

/// The layers of an anim-table entry (FUN_002a0c78), as the bake names them.
pub const LAYERS: [&str; 3] = ["", "_leftup", "_rightup"];

/// One walker model's skeleton, its clips by anim-table entry and layer,
/// and each leg pair's layer mask.
pub struct WalkerRig {
    pub skeleton: Skeleton,
    aim: super::walker_aim::WalkerAim,
    clips: Vec<Clip>,
    /// Per anim-table entry: its base, `_leftup` and `_rightup` clip.
    entries: Vec<[Option<usize>; 3]>,
    /// Per leg pair: the bones its layers move.
    masks: Vec<Vec<bool>>,
    bone_root: Option<usize>,
    /// A posed hover's `FinAnimation` clip (card VPOSE).
    fin: Option<usize>,
}

impl WalkerRig {
    /// A flyer's rig: its `takeoff` clip and its fins clip by name
    /// (`manifest::ManifestFlyer::fin`), if the bake found them.
    pub fn new_flyer(skeleton: Skeleton, clips: Vec<Clip>, fin: Option<&str>) -> Self {
        let flyer = manifest::ManifestWalker {
            bank: String::new(),
            clips: Vec::new(),
            prims: Vec::new(),
            skin: None,
            layer_bones: Vec::new(),
        };
        let fin = fin.and_then(|f| clips.iter().position(|c| c.name.eq_ignore_ascii_case(f)));
        Self {
            fin,
            ..Self::new(skeleton, clips, &flyer)
        }
    }

    /// A hover's rig (card VPOSE, `analysis/re-vehicle-pose.md` 7): its
    /// `FinAnimation` clip by name, if the bake found one.
    pub fn new_hover(skeleton: Skeleton, clips: Vec<Clip>, fin: Option<&str>) -> Self {
        Self::new_flyer(skeleton, clips, fin)
    }

    /// The fin nine-pose (re-vehicle-pose 4.3): the clip at frame `a` and
    /// frame `b`, each slerped between its keys, blended by `c`; the bind
    /// with no clip or no numbers yet.
    fn fin_palette(&self, nine: Option<super::turret_pose::NinePose>) -> Vec<Mat4> {
        let pose = match (self.fin.map(|k| &self.clips[k]), nine) {
            (Some(clip), Some(n)) => clip
                .sample(&self.skeleton, n.a, false)
                .blend(&clip.sample(&self.skeleton, n.b, false), n.c),
            _ => Pose::bind(&self.skeleton),
        };
        self.skeleton.palette(&pose.locals())
    }

    /// A hover's nodes now: the fin pose, then its turret, aimer and barrel
    /// writers from its seats' guns (re-vehicle-pose 2.3).
    pub fn hover_nodes(
        &self,
        v: &core::vehicle::Vehicle,
        nine: Option<super::turret_pose::NinePose>,
    ) -> Vec<Mat4> {
        let fins = self.bind_nodes(&self.fin_palette(nine));
        self.aim.pose(&self.skeleton, &fins, v)
    }

    /// A hover's joint palette now ([`Self::hover_nodes`]).
    pub fn hover_palette(
        &self,
        v: &core::vehicle::Vehicle,
        nine: Option<super::turret_pose::NinePose>,
    ) -> Vec<Mat4> {
        self.hover_palette_kicked(v, nine, &super::walker_aim::at_rest)
    }

    /// [`Self::hover_palette`] with each barrel slid back by `kick`.
    pub fn hover_palette_kicked(
        &self,
        v: &core::vehicle::Vehicle,
        nine: Option<super::turret_pose::NinePose>,
        kick: super::walker_aim::Kick,
    ) -> Vec<Mat4> {
        self.aimed(self.fin_palette(nine), v, kick)
    }

    /// A flyer's pose (BF2 FUN_00201300 B2:114581-114690, BF1 FUN_001d8620
    /// B1:84287-84335): while its bank is set, a class with a `takeoff`
    /// clip plays it at its `lift` and never its fins; else the fins clip's
    /// nine-pose at `nine` ([`super::turret_pose::flyer_nine`]); else drawn
    /// rigid.
    pub fn flyer_palette(
        &self,
        lift: f32,
        nine: Option<super::turret_pose::NinePose>,
    ) -> Option<Vec<Mat4>> {
        let Some(clip) = self
            .clips
            .iter()
            .find(|clip| clip.name.eq_ignore_ascii_case("takeoff"))
        else {
            return self.fin.map(|_| self.fin_palette(nine));
        };
        let frame = lift.clamp(0.0, 1.0) * clip.frames.saturating_sub(1) as f32;
        Some(
            self.skeleton
                .palette(&clip.sample(&self.skeleton, frame, false).locals()),
        )
    }

    pub fn flyer_nodes(
        &self,
        lift: f32,
        nine: Option<super::turret_pose::NinePose>,
    ) -> Option<Vec<Mat4>> {
        Some(self.bind_nodes(&self.flyer_palette(lift, nine)?))
    }

    /// Bone nodes in bind space (`palette * global_bind`).
    fn bind_nodes(&self, palette: &[Mat4]) -> Vec<Mat4> {
        palette
            .iter()
            .zip(&self.skeleton.global_bind)
            .map(|(p, b)| *p * *b)
            .collect()
    }

    /// The rig from a skin GLB's skeleton and clips and the class's walker
    /// record (its entries' clip names and its pairs' layer bones).
    pub fn new(skeleton: Skeleton, clips: Vec<Clip>, walker: &manifest::ManifestWalker) -> Self {
        let find = |name: &str| clips.iter().position(|c| c.name.eq_ignore_ascii_case(name));
        let entries = walker
            .clips
            .iter()
            .map(|c| match c {
                Some(c) => LAYERS.map(|l| find(&format!("{}{l}", c.name))),
                None => [None; 3],
            })
            .collect();
        let masks = walker
            .layer_bones
            .iter()
            .map(|pair| {
                let mut m = vec![false; skeleton.len()];
                for b in pair.iter().filter_map(|n| skeleton.index_of(n)) {
                    mark_subtree(&skeleton, b, &mut m);
                }
                m
            })
            .collect();
        let bone_root = skeleton.index_of(BONE_ROOT);
        Self {
            aim: Default::default(),
            skeleton,
            clips,
            entries,
            masks,
            bone_root,
            fin: None,
        }
    }

    /// The local pose of walker state `w` (every bone, parent space), before
    /// SkeletonRootScale.
    pub fn pose(&self, w: &WalkerState) -> Pose {
        self.pose_sampled(w, false)
    }

    fn pose_sampled(&self, w: &WalkerState, simulation: bool) -> Pose {
        let entry = self.entries.get(usize::from(w.anim));
        let sample = |layer: usize| {
            let c = entry.and_then(|e| e[layer]).map(|k| &self.clips[k])?;
            Some(if simulation {
                crate::sim_pose::sample(c, &self.skeleton, w.clip_time * c.fps)
            } else {
                c.sample(&self.skeleton, w.clip_time * c.fps, false)
            })
        };
        let mut pose = sample(0).unwrap_or_else(|| Pose::bind(&self.skeleton));
        for (mask, &b) in self.masks.iter().zip(&w.layer_blend) {
            let (layer, weight) = if b < 0.0 {
                (1, -b)
            } else if b > 0.0 {
                (2, b)
            } else {
                continue;
            };
            let Some(lp) = sample(layer) else { continue };
            for (k, _) in mask.iter().enumerate().filter(|(_, &m)| m) {
                pose.0[k] = if simulation {
                    crate::sim_pose::blend(pose.0[k], lp.0[k], weight.min(1.0))
                } else {
                    pose.0[k].blend(lp.0[k], weight.min(1.0))
                };
            }
        }
        // FUN_0025f050: bone 0 turns with the legs (2h).
        let (c, s) = w.h;
        if let Some(b0) = pose.0.first_mut() {
            b0.rotation = (Quat::from_xyzw(0.0, s, 0.0, c) * b0.rotation).normalize();
        }
        if let Some(r) = self.bone_root.and_then(|r| pose.0.get_mut(r)) {
            let (sa, ca) = if simulation {
                core::fmath::sin_cos(w.pitch)
            } else {
                w.pitch.sin_cos()
            };
            let (rc, rs) = w.roll;
            r.rotation = (r.rotation
                * Quat::from_xyzw(sa, 0.0, 0.0, ca)
                * Quat::from_xyzw(0.0, 0.0, rs, rc))
            .normalize();
        }
        pose
    }

    /// The joint palette of walker state `w` for a class whose
    /// SkeletonRootScale is `root_scale`.
    pub fn palette(&self, w: &WalkerState, root_scale: f32) -> Vec<Mat4> {
        let mut locals = self.pose(w).locals();
        if let Some(r) = self.bone_root.filter(|_| root_scale != 1.0) {
            locals[r] *= Mat4::from_scale(Vec3::splat(root_scale));
        }
        self.skeleton.palette(&locals)
    }

    /// Node `name`'s model-space matrix as walker state `w` poses it: what
    /// an `AnimatedPilotPosition` rider hangs by (card VRIDE,
    /// `analysis/re-vehicle-riders.md` 2.1 [O]). `None` when the skeleton
    /// has no such node.
    /// Collision uses portable interpolation and trigonometry, independently
    /// of the host-math drawing sampler above.
    pub(crate) fn node_for_sim(
        &self,
        w: &WalkerState,
        root_scale: f32,
        name: &str,
    ) -> Option<Mat4> {
        let n = self.skeleton.index_of(name)?;
        Some(self.palette_for_sim(w, root_scale)[n] * self.skeleton.global_bind[n])
    }

    /// Map bind-space collision into the same deterministic pose as rider queries.
    pub(crate) fn palette_for_sim(&self, w: &WalkerState, root_scale: f32) -> Vec<Mat4> {
        let mut locals = self.pose_sampled(w, true).locals();
        if let Some(r) = self.bone_root.filter(|_| root_scale != 1.0) {
            locals[r] *= Mat4::from_scale(Vec3::splat(root_scale));
        }
        self.skeleton.palette(&locals)
    }

    pub fn with_seats(mut self, seats: Vec<crate::vehicle_class::SeatSpec>) -> Self {
        self.aim = super::walker_aim::WalkerAim::new(&self.skeleton, seats);
        self
    }

    pub fn nodes(&self, v: &core::vehicle::Vehicle, root_scale: f32) -> Vec<Mat4> {
        let walk = self.bind_nodes(&self.palette(&v.walker, root_scale));
        self.aim.pose(&self.skeleton, &walk, v)
    }

    pub fn aimed_palette(&self, v: &core::vehicle::Vehicle, root_scale: f32) -> Vec<Mat4> {
        self.aimed_palette_kicked(v, root_scale, &super::walker_aim::at_rest)
    }

    /// [`Self::aimed_palette`] with each barrel slid back by `kick`.
    pub fn aimed_palette_kicked(
        &self,
        v: &core::vehicle::Vehicle,
        root_scale: f32,
        kick: super::walker_aim::Kick,
    ) -> Vec<Mat4> {
        self.aimed(self.palette(&v.walker, root_scale), v, kick)
    }

    /// `palette` with the seats' turret, aimer and barrel writers applied.
    fn aimed(
        &self,
        palette: Vec<Mat4>,
        v: &core::vehicle::Vehicle,
        kick: super::walker_aim::Kick,
    ) -> Vec<Mat4> {
        let walk = self.bind_nodes(&palette);
        let now = self.aim.pose_kicked(&self.skeleton, &walk, v, kick);
        now.iter()
            .enumerate()
            .map(|(n, m)| {
                if *m == walk[n] {
                    palette[n]
                } else {
                    *m * self.skeleton.inverse_bind[n]
                }
            })
            .collect()
    }

    pub fn seat_frame(
        &self,
        v: &core::vehicle::Vehicle,
        root_scale: f32,
        turret: Option<&str>,
        look: (f32, f32),
    ) -> Mat4 {
        let carried =
            turret
                .and_then(|name| self.skeleton.index_of(name))
                .map_or(Mat4::IDENTITY, |n| {
                    let parent = self.skeleton.parents[n].map_or(Mat4::IDENTITY, |p| {
                        if root_scale == 1.0 {
                            self.skeleton.global_bind[p]
                        } else {
                            self.rest_nodes(root_scale)[p]
                        }
                    });
                    let mut carried = parent * self.skeleton.local_bind[n];
                    carried.w_axis = self.nodes(v, root_scale)[n].w_axis;
                    carried
                });
        super::turret_pose::turret_node(&carried, look.0, -look.1)
    }

    fn rest_nodes(&self, root_scale: f32) -> Vec<Mat4> {
        let mut locals = Pose::bind(&self.skeleton).locals();
        if let Some(r) = self.bone_root.filter(|_| root_scale != 1.0) {
            locals[r] *= Mat4::from_scale(Vec3::splat(root_scale));
        }
        self.bind_nodes(&self.skeleton.palette(&locals))
    }

    pub fn node_now(
        &self,
        v: &core::vehicle::Vehicle,
        root_scale: f32,
        name: &str,
    ) -> Option<Mat4> {
        let n = self.skeleton.index_of(name)?;
        Some(self.nodes(v, root_scale)[n])
    }
}

/// Mark `bone` and every bone below it.
fn mark_subtree(skel: &Skeleton, bone: usize, mask: &mut [bool]) {
    mask[bone] = true;
    for k in 0..skel.len() {
        if skel.parents[k] == Some(bone) && !mask[k] {
            mark_subtree(skel, k, mask);
        }
    }
}

/// Skin one vertex list by `palette` (one bone per vertex, `joints[i][0]`):
/// positions through the joint matrix, normals through it and renormalised.
/// A vertex whose bone is past the palette stays where it is.
pub fn skin(
    palette: &[Mat4],
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    joints: &[[u8; 4]],
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    positions
        .iter()
        .zip(normals)
        .zip(joints)
        .map(|((p, n), j)| match palette.get(usize::from(j[0])) {
            Some(m) => (
                m.transform_point3(Vec3::from(*p)).to_array(),
                m.transform_vector3(Vec3::from(*n))
                    .normalize_or_zero()
                    .to_array(),
            ),
            None => (*p, *n),
        })
        .unzip()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::BoneXform;

    /// DummyRoot → bone_root → bone_l_thigh → bone_l_calf, one metre apart.
    fn rig(with_layers: bool) -> WalkerRig {
        let names = ["DummyRoot", "bone_root", "bone_l_thigh", "bone_l_calf"];
        let skel = Skeleton::new(
            names.iter().map(|s| s.to_string()).collect(),
            vec![None, Some(0), Some(1), Some(2)],
            (0..4)
                .map(|i| {
                    Mat4::from_translation(Vec3::new(0.0, if i == 0 { 0.0 } else { 1.0 }, 0.0))
                })
                .collect(),
        );
        let key = |q: Quat| {
            vec![
                BoneXform {
                    rotation: q,
                    translation: Vec3::Y
                };
                2
            ]
        };
        let clip = |name: &str, thigh: Quat| Clip {
            name: name.into(),
            fps: 30.0,
            frames: 2,
            tracks: vec![None, None, Some(key(thigh)), None],
        };
        let mut clips = vec![clip("idle", Quat::IDENTITY)];
        if with_layers {
            clips.push(clip("idle_leftup", Quat::from_rotation_x(1.0)));
            clips.push(clip("idle_rightup", Quat::from_rotation_x(-1.0)));
        }
        let walker = manifest::ManifestWalker {
            bank: "test".into(),
            clips: vec![Some(manifest::ManifestWalkerClip {
                name: "idle".into(),
                frames: 2,
                duration: 1.0 / 30.0,
                looping: false,
                root_z: vec![0.0, 0.0],
            })],
            prims: vec![],
            skin: Some("test".into()),
            layer_bones: vec![["bone_l_thigh".into(), "bone_r_thigh".into()]],
        };
        WalkerRig::new(skel, clips, &walker)
    }

    fn angle_x(q: Quat) -> f32 {
        let (axis, a) = q.to_axis_angle();
        a * axis.x.signum()
    }

    #[test]
    fn flyer_takeoff_pose_tracks_lift_between_authored_frames() {
        let skeleton = Skeleton::new(vec!["root".into()], vec![None], vec![Mat4::IDENTITY]);
        let clip = Clip {
            name: "takeoff".into(),
            fps: 30.0,
            frames: 2,
            tracks: vec![Some(vec![
                BoneXform::IDENTITY,
                BoneXform {
                    rotation: Quat::IDENTITY,
                    translation: Vec3::X * 2.0,
                },
            ])],
        };
        let rig = WalkerRig::new_flyer(skeleton, vec![clip], None);
        for (lift, x) in [(0.0, 0.0), (0.5, 1.0), (1.0, 2.0)] {
            let palette = rig.flyer_palette(lift, None).unwrap();
            assert!((palette[0].w_axis.x - x).abs() < 1e-5);
        }
    }

    #[test]
    fn the_leftup_layer_takes_minus_b_and_the_rightup_layer_b() {
        let r = rig(true);
        let mut w = WalkerState::default();
        w.layer_blend[0] = -0.25;
        assert!((angle_x(r.pose(&w).0[2].rotation) - 0.25).abs() < 1e-4);
        w.layer_blend[0] = 0.5;
        assert!((angle_x(r.pose(&w).0[2].rotation) + 0.5).abs() < 1e-4);
        // The layer moves only its bones' subtrees: bone_root holds.
        assert!(r.pose(&w).0[1].rotation.abs_diff_eq(Quat::IDENTITY, 1e-6));
        // No layer clips: the base pose.
        let plain = rig(false);
        assert!(plain.pose(&w).0[2]
            .rotation
            .abs_diff_eq(Quat::IDENTITY, 1e-6));
    }

    #[test]
    fn bone_zero_turns_by_the_legs_two_h_and_bone_root_pitches_two_a() {
        let r = rig(false);
        let mut w = WalkerState::default();
        let h: f32 = 0.2;
        w.h = (h.cos(), h.sin());
        w.pitch = 0.1;
        let p = r.pose(&w);
        let (axis, a) = p.0[0].rotation.to_axis_angle();
        assert!((a - 2.0 * h).abs() < 1e-5 && axis.y > 0.99);
        assert!((angle_x(p.0[1].rotation) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn the_bind_pose_skins_every_vertex_where_it_is() {
        let r = rig(false);
        let pal = r.palette(&WalkerState::default(), 1.0);
        let (p, n) = skin(
            &pal,
            &[[0.3, 2.5, 0.1]],
            &[[0.0, 0.0, 1.0]],
            &[[2, 0, 0, 0]],
        );
        assert!(Vec3::from(p[0]).abs_diff_eq(Vec3::new(0.3, 2.5, 0.1), 1e-5));
        assert!(Vec3::from(n[0]).abs_diff_eq(Vec3::Z, 1e-5));
        // SkeletonRootScale scales about bone_root: a thigh vertex 1.5 m
        // above it sits 3 m above it at scale 2.
        let pal = r.palette(&WalkerState::default(), 2.0);
        let (p, _) = skin(
            &pal,
            &[[0.0, 2.5, 0.0]],
            &[[0.0, 1.0, 0.0]],
            &[[2, 0, 0, 0]],
        );
        assert!((p[0][1] - 4.0).abs() < 1e-5, "{p:?}");
    }
    #[test]
    fn rider_collision_layers_change_sim_hash_only_with_the_pose_provider() {
        use core::sim::{HeroRiderFrames, Sim};
        use core::vehicle::{SeatKind, Vehicle, VehicleKind, VehicleSpec};
        use std::sync::Arc;
        struct Frames(Arc<WalkerRig>);
        impl HeroRiderFrames for Frames {
            fn frame(&self, vehicle: &Vehicle, _: u8) -> Option<Mat4> {
                self.0.node_for_sim(&vehicle.walker, 1.0, "bone_l_calf")
            }
        }
        let frames = Arc::new(Frames(Arc::new(rig(true))));
        let mut sim = Sim::empty(73);
        let vehicle = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Walker,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 500.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let ordinary = sim.hash();
        sim.vehicle_mut(vehicle).unwrap().walker.layer_blend[0] = 0.25;
        assert_eq!(
            sim.hash(),
            ordinary,
            "a provider-free sim keeps its historical stream"
        );
        let first = frames.frame(sim.vehicle(vehicle).unwrap(), 0).unwrap();
        sim.hero_rider_frames = Some(frames.clone());
        let hashed = sim.hash();
        let mut old_walker_hash = Vec::new();
        sim.vehicle(vehicle)
            .unwrap()
            .walker
            .hash_into(&mut |b| old_walker_hash.extend_from_slice(b));
        sim.vehicle_mut(vehicle).unwrap().walker.layer_blend[0] = -0.75;
        let second = frames.frame(sim.vehicle(vehicle).unwrap(), 0).unwrap();
        assert_ne!(
            first.to_cols_array(),
            second.to_cols_array(),
            "the omitted history really moves the physical node"
        );
        let mut unchanged_old_walker_hash = Vec::new();
        sim.vehicle(vehicle)
            .unwrap()
            .walker
            .hash_into(&mut |b| unchanged_old_walker_hash.extend_from_slice(b));
        assert_eq!(
            old_walker_hash, unchanged_old_walker_hash,
            "negative control: the old walker hash omits the layer"
        );
        assert_ne!(
            sim.hash(),
            hashed,
            "the new physical consumer must hash that history"
        );
        sim.hero_rider_frames = None;
        assert_eq!(sim.hash(), ordinary);
    }

    #[test]
    fn portable_seat_nodes_match_actual_baked_walker_drawing_at_fractional_phases() {
        let root = crate::assets::baked_root();
        let path = root.join("bf2_hot1/manifest.ron");
        if !path.is_file() {
            eprintln!("SKIP: actual BF2 Hoth manifest and walker bakes required");
            return;
        }
        let manifest: manifest::Manifest =
            ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let db = crate::roster::load_classdb(
            &root.join("classes"),
            "bf2",
            &["all", "imp", "rep", "cis"],
        )
        .unwrap();
        let names: Vec<_> = db
            .names()
            .filter(|name| db.engine_base(name) == Some("walker"))
            .map(str::to_owned)
            .collect();
        let geometry: Vec<_> = names
            .iter()
            .map(|name| db.prop(name, "GeometryName").map(str::to_owned))
            .collect();
        let rigs = crate::viewer::vehicles::walker_rigs(&manifest, &root, &geometry);
        let mut nodes = 0;
        let mut node_classes = Vec::new();
        let mut samples = 0;
        for (name, rig) in names.iter().zip(rigs) {
            let Some(rig) = rig else { continue };
            for seat in crate::vehicle_class::seats(&db, name) {
                let Some(node) = seat.animated_pilot_position else {
                    continue;
                };
                nodes += 1;
                node_classes.push(name.as_str());
                for anim in 0..rig.entries.len() {
                    for time in [0.0, 1.0 / 60.0, 0.173, 0.41] {
                        let (s, c) = core::fmath::sin_cos(0.23);
                        let state = WalkerState {
                            anim: anim as u8,
                            clip_time: time,
                            h: (c, s),
                            pitch: -0.11,
                            roll: (c, s),
                            layer_blend: [0.37; core::vehicle::walker::MAX_PAIRS],
                            ..Default::default()
                        };
                        let actual = rig.node_for_sim(&state, 1.17, &node).unwrap();
                        let repeat = rig.node_for_sim(&state, 1.17, &node).unwrap();
                        assert_eq!(
                            actual.to_cols_array().map(f32::to_bits),
                            repeat.to_cols_array().map(f32::to_bits)
                        );
                        let n = rig.skeleton.index_of(&node).unwrap();
                        let drawn = rig.palette(&state, 1.17)[n] * rig.skeleton.global_bind[n];
                        let error = actual
                            .to_cols_array()
                            .into_iter()
                            .zip(drawn.to_cols_array())
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0f32, f32::max);
                        assert!(
                            error < 0.002,
                            "{name}/{node} anim{anim} time{time}: {error}"
                        );
                        samples += 1;
                    }
                }
            }
        }
        node_classes.sort_unstable();
        assert_eq!(node_classes, ["all_walk_tauntaun", "rep_walk_oneman_atst"]);
        // Each of the two original banks has eleven table entries.
        assert_eq!(samples, 2 * 11 * 4);
        eprintln!("actual portable seat nodes{nodes}, samples{samples}");
    }
}
