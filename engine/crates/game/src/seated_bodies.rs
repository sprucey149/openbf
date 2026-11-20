//! Issue #182: a seated rider is hit where he is drawn, whatever his size.
//!
//! A soldier in a seat that shows him (a building turret's gunner, card
//! VTURG; a vehicle's exposed rider, card VRIDE) is drawn posed by the
//! seat's clip on his own skeleton (`soldier_vis`, `Animator::seat`). His
//! bolt prims stood in the standing posture's places, a trooper's: a
//! MagnaGuard's drawn head (1.24 root, a longer rig) stood 0.18 m over the
//! prim, past its 0.17 m radius, and any gunner leaning over a gun pitched
//! down had his head under it. BF2 moves the head and torso prims onto the
//! animation's bone_head and bone_ribcage points each frame (FUN_0024f350
//! B2:168821-169060, `analysis/re-hitzones.md` 1); here they are the seat
//! pose's, sampled once at round setup from the same baked skeleton and
//! clip the draw uses (`assets::load_skinned_glb`, as `hero_blades` does)
//! and handed to the sim as [`core::sim::SeatedBodies`].
//!
//! The points are BF2's: bone_head x (0.035, 0.03, 0) and bone_ribcage x
//! (-0.12, -0.05, 0) in the bone's frame (FUN_00282f78 B2:216195-216340;
//! the bake's bones run +X up the spine and +Y to the front, so the head's
//! centre is 3.5 cm over the bone and the torso's 12 cm down the spine,
//! 1.19 m on a seated trooper against the standing posture's 1.15). Not
//! taken (PROVISIONAL): the class's `CollisionHeadOffset` /
//! `CollisionTorsoOffset` and the `CollisionRootScale` the PS2 multiplies
//! the point by, which would lift a 1.15 Wookiee's head off his drawn one;
//! measure: a BF2 PS2 Wookiee gunner's head prim in RAM against his drawn
//! head.

use crate::assets::SkinnedAsset;
use crate::vehicle_class::{GunnerDraw, SeatSpec};
use crate::viewer::turret_pose::NinePose;
use crate::walk::{TeamTemplate, WalkSim};
use core::sim::{SeatedBodies, SeatedBody, NINE_GRID};
use glam::Vec3;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Fill `walk.sim.seated_bodies` for every class of the two teams and the
/// native ones in every seat of `walk.vehicle_seats` that draws its rider
/// with a pose. A class whose model is not baked keeps the posture's prims.
/// So does a class the dev class picker registers after setup
/// (`ClassPicker::select`): it is not in the table and, seated, is hit at
/// the standing posture's prims.
pub fn install(walk: &mut WalkSim, assets_dir: &Path, map_game: &str) {
    let bf2 = map_game.eq_ignore_ascii_case("bf2");
    let teams: Vec<(u8, &TeamTemplate)> = walk
        .teams
        .iter()
        .enumerate()
        .map(|(t, tt)| (t as u8, tt))
        .chain(walk.native_teams.iter().map(|(t, tt)| (*t, tt)))
        .collect();
    let mut models: HashMap<(String, String), Option<SkinnedAsset>> = HashMap::new();
    let mut out = SeatedBodies::default();
    for (team, template) in teams {
        for (k, class) in template.classes.iter().enumerate() {
            let Some(geometry) = class.geometry.as_deref() else {
                continue;
            };
            let game = class.asset_game(map_game).to_owned();
            let asset = models
                .entry((game.clone(), geometry.to_owned()))
                .or_insert_with(|| load(assets_dir, &game, geometry));
            let Some(asset) = asset.as_ref() else {
                continue;
            };
            for (vclass, seats) in walk.vehicle_seats.iter().enumerate() {
                for (s, seat) in seats.iter().enumerate() {
                    if seat.gunner_draw(bf2) != GunnerDraw::Drawn {
                        continue;
                    }
                    if let Some(body) = seated_body(asset, seat) {
                        out.by.insert((team, k as u8, vclass as u32, s as u8), body);
                    }
                }
            }
        }
    }
    walk.sim.seated_bodies = (!out.by.is_empty()).then(|| Arc::new(out));
}

/// The class's sim skeleton and clips: its side's soldier, else a local's.
fn load(assets_dir: &Path, game: &str, geometry: &str) -> Option<SkinnedAsset> {
    let side = assets_dir.join(format!("side_{game}"));
    [side.join("soldiers"), side.join("locals").join("soldiers")]
        .iter()
        .map(|d| d.join(format!("{geometry}.glb")))
        .find(|p| pak::is_file(p))
        .and_then(|p| crate::assets::load_skinned_glb(&p).ok())
}

/// One class in one seat: its drawn head and torso at the seat's pose -
/// the clip's first frame for a played clip, the nine-pose grid for a
/// `Pilot9Pose` seat. `None` without the pose clip or the two bones.
pub fn seated_body(asset: &SkinnedAsset, seat: &SeatSpec) -> Option<SeatedBody> {
    let pose = seat.pilot_pose.as_ref()?;
    let clip = &asset.clips[crate::animator::find_clip(&asset.clips, &pose.clip())?];
    let skel = &asset.skeleton;
    let head = skel.index_of("bone_head")?;
    let rib = skel.index_of("bone_ribcage")?;
    let points_of = |p: crate::skin::Pose| {
        let palette = skel.palette(&p.locals());
        let at = |b: usize, off: Vec3| {
            quantize((palette[b] * skel.global_bind[b]).transform_point3(off))
        };
        [
            at(head, Vec3::new(0.035, 0.03, 0.0)),
            at(rib, Vec3::new(-0.12, -0.05, 0.0)),
        ]
    };
    let points = if pose.nine {
        let step = |i: usize| -1.0 + 2.0 * i as f32 / (NINE_GRID - 1) as f32;
        (0..NINE_GRID * NINE_GRID)
            .map(|i| {
                let (yf, pf) = (step(i / NINE_GRID), step(i % NINE_GRID));
                // `turret_pose::nine_pose`'s frames for these numbers.
                let n = NinePose {
                    a: yf + 4.0,
                    b: yf + if pf > 0.0 { 7.0 } else { 1.0 },
                    c: pf.abs(),
                };
                // The portable sampler (`sim_pose`), not the render one:
                // these points feed the sim's hit tests and the static
                // digest, so no platform libm may round them.
                let a = crate::sim_pose::sample(clip, skel, n.a);
                let b = crate::sim_pose::sample(clip, skel, n.b);
                points_of(crate::skin::Pose(
                    a.0.into_iter()
                        .zip(b.0)
                        .map(|(a, b)| crate::sim_pose::blend(a, b, n.c))
                        .collect(),
                ))
            })
            .collect()
    } else {
        vec![points_of(clip.sample_frame(skel, 0))]
    };
    Some(SeatedBody {
        pitch_limits: seat.pitch_limits,
        yaw_limits: seat.yaw_limits,
        points,
    })
}

/// To 1/1024 m, so the table every peer samples at setup is the same.
fn quantize(p: Vec3) -> Vec3 {
    (p * 1024.0).round() / 1024.0
}
