//! BF2's local lights as the renderer takes them (card BPNT,
//! `analysis/re-point-lights.md` [O]): the sources, the VU's colour packing,
//! and the sphere a model's lights are gathered for. The gather and the
//! per-vertex term are `render::point_light`'s.
//!
//! **Sources** (§0–§3):
//! - the authored `lght` omnis and spots ([`authored`], the manifest's
//!   `point_lights`);
//! - the EntityLights a placed prop's class hangs on its hardpoints
//!   ([`entity`], `ManifestClass::lights`) — steady: the flicker types are
//!   not built, PROVISIONAL;
//! - the command post's hologram light ([`command_post`]): r 8, 0.32 m over
//!   the post, never Static, in the VIEWER's team-relation colour — own
//!   (1, 86, 213), enemy (200, 100, 100), neutral (192, 192, 192), times
//!   the hologram's intensity I in a battle (its 2 s fades and flicker dips,
//!   `crate::holo`, card CPHOL; steady in a render with no battle).
//!   Every BF2 post gets one (each names `com_holo_controlzone`, §3 [D]).
//! - an item's `LightOdf` EntityLight (§4): the campaign's holocron.
//!
//! The other runtime lights (explosion flashes, CTF flags, mines) are not built; the
//! bolts' reach models through [`omni`].
//!
//! **Packing** (§1): the VU's colour is the packed byte / 128 —
//! trunc(c·128), or (int)(c·255) with `PS2Overbright`, which doubles it —
//! and the gather's score reads the colour floats, not doubled.
//!
//! **The sphere** ([`model_sphere`]): the PS2 gathers per model from the
//! model's own sphere, which the map bake writes on each world model
//! (`bake::glb::model_sphere`, PROVISIONAL [D]); a model without one takes
//! its bounding box's.

use glam::{Mat4, Vec3};
use manifest::Manifest;
use render::{LitSphere, PointLight, Renderer, SpotCone};

use crate::assets::{CpuMesh, Scene};
use crate::bolt_light::OmniLight;
use crate::bolt_vis::BoltVis;
use crate::walk::WalkSim;

/// `HoloLightRadius` of `com_holo_controlzone` (§3).
pub const POST_LIGHT_RADIUS: f32 = 8.0;
/// The hologram's `hp_hologram` over the post's origin, metres (§3, RAM).
pub const POST_LIGHT_HEIGHT: f32 = 0.32;

/// How a post's team stands to the viewer's (§3's colour table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Friend,
    Enemy,
    Neutral,
}

impl Relation {
    /// The post owned by `owner` as the viewer on `viewer` sees it (team
    /// numbers of one kind; `None` = neutral).
    pub fn of(owner: Option<u8>, viewer: u8) -> Relation {
        match owner {
            None => Relation::Neutral,
            Some(t) if t == viewer => Relation::Friend,
            Some(_) => Relation::Enemy,
        }
    }

    fn bytes(self) -> [u8; 3] {
        match self {
            Relation::Friend => [1, 86, 213],
            Relation::Enemy => [200, 100, 100],
            Relation::Neutral => [192, 192, 192],
        }
    }
}

/// A light from its colour floats (0..1): the packing and the score weight.
fn packed(pos: Vec3, c: [f32; 3], range: f32, is_static: bool, overbright: bool) -> PointLight {
    let byte = |v: f32| {
        if overbright {
            (v * 255.0) as i32 as f32
        } else {
            (v * 128.0).trunc()
        }
    };
    PointLight {
        pos,
        color: c.map(|v| byte(v) / 128.0),
        lum: 0.3 * c[0] + 0.45 * c[1] + 0.25 * c[2],
        range,
        is_static,
        spot: false,
        cone: None,
    }
}

/// A non-Static omni from colour bytes: a bolt's light, an EntityLight.
pub fn omni(pos: Vec3, rgb: [u8; 3], range: f32, is_static: bool) -> PointLight {
    packed(
        pos,
        rgb.map(|b| f32::from(b) / 255.0),
        range,
        is_static,
        false,
    )
}

/// The map's authored lights.
pub fn authored(m: &Manifest) -> Vec<PointLight> {
    m.point_lights
        .iter()
        .map(|l| PointLight {
            spot: l.spot,
            cone: l.cone.map(|c| SpotCone {
                dir: Vec3::from(c.dir),
                sin_half_outer: core::fmath::sin_cos(c.outer * 0.5).0,
            }),
            ..packed(
                Vec3::from(l.pos),
                l.color,
                l.range,
                l.is_static,
                l.overbright,
            )
        })
        .collect()
}

/// The EntityLights of every placed instance whose class carries any.
pub fn entity(scene: &Scene) -> Vec<PointLight> {
    let mut out = Vec::new();
    for inst in &scene.manifest.instances {
        let Some(class) = scene.resolve_class(&inst.class) else {
            continue;
        };
        if class.lights.is_empty() {
            continue;
        }
        let m = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        out.extend(class.lights.iter().map(|l| {
            omni(
                m.transform_point3(Vec3::from(l.pos)),
                l.color,
                l.radius,
                l.is_static,
            )
        }));
    }
    out
}

/// The scene's own lights: authored, then the EntityLights.
pub fn scene_lights(scene: &Scene) -> Vec<PointLight> {
    let mut out = authored(&scene.manifest);
    out.extend(entity(scene));
    out
}

/// The map's conquest mission for a render with no battle: `<map>_con.ron`
/// beside the bake (`<bake>/../missions`), else the source tree's.
pub(crate) fn still_mission(scene: &Scene) -> Option<crate::mission::MissionDef> {
    let file = format!("{}_con.ron", scene.dir.file_name()?.to_str()?);
    let beside = scene.dir.parent()?.parent()?.join("missions");
    let tree = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    [beside.as_path(), tree]
        .iter()
        .map(|d| d.join(&file))
        .find(|p| pak::is_file(p))
        .and_then(|p| crate::mission::MissionDef::load(&p).ok())
}

/// The posts a render with no battle lights: the conquest mission's roster,
/// without the posts it kills, at the owners it sets (`MapLayout::
/// apply_mission`) — BF2 Death Star's script kills CP6 and the PS2 has no
/// light there (`analysis/re-vertex-colour-interp.md` §6 `[O]`). The world
/// layer's posts as they are where the map has no conquest mission file.
pub(crate) fn still_posts(
    manifest: &Manifest,
    mission: Option<&crate::mission::MissionDef>,
) -> Vec<crate::layout::CommandPostDef> {
    let mut layout = crate::layout::MapLayout::from_manifest(manifest);
    if let Some(m) = mission {
        layout.apply_mission(m);
    }
    layout.cps
}

/// The scene's lights with every post of the conquest mission at its
/// STARTING owner ([`still_posts`]), seen by the mission's team 1 — what a
/// render with no battle shows (the goldens, the walkabout, `pose_render`;
/// the PS2 player is on team 1 in every stored walk state, RAM). A BF1 map
/// has neither.
pub fn still_lights(scene: &Scene) -> Vec<PointLight> {
    let mut out = scene_lights(scene);
    if scene.manifest.source_game == "bf2" {
        let mission = still_mission(scene);
        out.extend(
            still_posts(&scene.manifest, mission.as_ref())
                .iter()
                .map(|c| command_post(c.pos, Relation::of((c.team != 0).then_some(c.team), 1))),
        );
    }
    out
}

/// A light as the terrain pass takes it: the colour floats as bytes
/// (the terrain reads the floats, never the Overbright packing, §1).
fn ground_light(l: &PointLight, rgb: [u8; 3]) -> OmniLight {
    OmniLight {
        pos: l.pos,
        rgb,
        radius: l.range,
    }
}

/// One battle frame's lights (BF2; a no-op list on BF1): the scene's, each
/// post's in the player's relation to its owner NOW, and the bolts' — to the
/// models ([`Renderer::set_point_lights`]) and, the non-Static omnis, to the
/// terrain pass beside the bolts (`BoltVis::submit`, §8). A post's light
/// takes its hologram's intensity (`holos`, already advanced to this frame).
/// `items` are the items' EntityLights (position, colour, OmniRadius; §4).
pub fn submit(
    r: &mut Renderer,
    gpu: &crate::viewer::SceneGpu,
    walk: &WalkSim,
    bolts: &mut BoltVis,
    eye: Vec3,
    holos: &crate::holo::Holos,
    items: &[(Vec3, [u8; 3], f32)],
) {
    let mut lights = gpu.lights.clone();
    let mut ground: Vec<OmniLight> = gpu.ground_lights.clone();
    if gpu.post_lights {
        let viewer = walk.player_team();
        for (i, cp) in walk.sim.cps.iter().enumerate() {
            // The light is the hologram's: a post with none has none.
            if gpu.holo.bare.contains(&cp.def.name) {
                continue;
            }
            // A killed post's light is off the render list (DEA1's CP6,
            // `analysis/re-vertex-colour-interp.md` section 6).
            if cp.killed {
                continue;
            }
            // The hologram shows its own team until the flip's fade is over.
            let team = holos.list.get(i).map_or(cp.owner, |h| h.team);
            let rgb = dimmed(Relation::of(team, viewer).bytes(), holos.intensity(i));
            let l = post_light(cp.def.pos, rgb);
            ground.push(ground_light(&l, rgb));
            lights.push(l);
        }
    }
    for &(pos, rgb, range) in items {
        let l = omni(pos, rgb, range, false);
        ground.push(ground_light(&l, rgb));
        lights.push(l);
    }
    let shots = bolts.submit(r, walk, eye, &ground);
    lights.extend(shots.iter().map(|b| omni(b.pos, b.rgb, b.radius, false)));
    r.set_point_lights(lights);
}

/// The scene's non-Static omnis as the terrain pass takes them: the
/// authored ones and the EntityLights (their colours back to bytes).
pub fn ground_lights(scene: &Scene) -> Vec<OmniLight> {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out: Vec<OmniLight> = scene
        .manifest
        .point_lights
        .iter()
        .filter(|l| !l.is_static && !l.spot)
        .map(|l| OmniLight {
            pos: Vec3::from(l.pos),
            rgb: l.color.map(byte),
            radius: l.range,
        })
        .collect();
    for inst in &scene.manifest.instances {
        let Some(class) = scene.resolve_class(&inst.class) else {
            continue;
        };
        let m = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        out.extend(
            class
                .lights
                .iter()
                .filter(|l| !l.is_static)
                .map(|l| OmniLight {
                    pos: m.transform_point3(Vec3::from(l.pos)),
                    rgb: l.color,
                    radius: l.radius,
                }),
        );
    }
    out
}

/// A relation colour at the hologram's intensity `i`: each byte
/// `(int)(c·i + 0.5)` (§3: the lerp from black, `FUN_002172a8` 129615).
pub fn dimmed(rgb: [u8; 3], i: f32) -> [u8; 3] {
    rgb.map(|c| (f32::from(c) * i.clamp(0.0, 1.0) + 0.5) as u8)
}

/// A command post's hologram light, for a post at `pos`.
pub fn command_post(pos: Vec3, relation: Relation) -> PointLight {
    post_light(pos, relation.bytes())
}

fn post_light(pos: Vec3, rgb: [u8; 3]) -> PointLight {
    omni(
        pos + Vec3::Y * POST_LIGHT_HEIGHT,
        rgb,
        POST_LIGHT_RADIUS,
        false,
    )
}

/// The world sphere of a model drawn at `matrix`: the disc's own
/// (`CpuMesh::sphere`), else its bounding box's centre and half-diagonal,
/// through the matrix (uniform scale assumed, as the renderer's normals do).
pub fn model_sphere(cpu: &CpuMesh, matrix: Mat4) -> Option<LitSphere> {
    let (centre, radius) = match cpu.sphere {
        Some([x, y, z, r]) => (Vec3::new(x, y, z), r),
        None => {
            let mut it = cpu.positions.iter().map(|p| Vec3::from(*p));
            let first = it.next()?;
            let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
            ((lo + hi) * 0.5, (hi - lo).length() * 0.5)
        }
    };
    Some(LitSphere {
        centre: matrix.transform_point3(centre),
        radius: radius * matrix.transform_vector3(Vec3::X).length(),
    })
}

/// The sphere a soldier standing at `feet` gathers for: a metre around his
/// middle, PROVISIONAL (the PS2 uses the skinned model's own sphere).
pub fn soldier_sphere(feet: Vec3) -> LitSphere {
    LitSphere {
        centre: feet + Vec3::Y * 0.9,
        radius: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §1's packing: ×128 truncated, or ×255 with Overbright — UTA1's
    /// yellow spots (243, 253, 68) reach 1.9 / 1.97 / 0.52 GS units.
    #[test]
    fn the_colour_packs_as_the_vu_reads_it() {
        let c = [0.952_941, 0.992_157, 0.266_667];
        let plain = packed(Vec3::ZERO, c, 10.0, true, false);
        assert_eq!(plain.color, [121.0 / 128.0, 126.0 / 128.0, 34.0 / 128.0]);
        let over = packed(Vec3::ZERO, c, 10.0, true, true);
        assert_eq!(over.color, [242.0 / 128.0, 253.0 / 128.0, 68.0 / 128.0]);
        assert_eq!(plain.lum, over.lum);
        assert!((plain.lum - (0.3 * c[0] + 0.45 * c[1] + 0.25 * c[2])).abs() < 1e-6);
    }

    /// §3: the post's light by the viewer's relation, 0.32 m up, r 8.
    #[test]
    fn a_post_lights_in_the_viewers_relation_colour() {
        let l = command_post(Vec3::new(1.0, 2.0, 3.0), Relation::of(Some(2), 1));
        assert_eq!(l.pos, Vec3::new(1.0, 2.32, 3.0));
        assert_eq!((l.range, l.is_static, l.spot), (8.0, false, false));
        assert_eq!(l.color, [100.0 / 128.0, 50.0 / 128.0, 50.0 / 128.0]);
        assert_eq!(Relation::of(Some(1), 1), Relation::Friend);
        assert_eq!(Relation::of(None, 1), Relation::Neutral);
        let own = command_post(Vec3::ZERO, Relation::Friend);
        assert_eq!(own.color, [0.0, 43.0 / 128.0, 106.0 / 128.0]);
    }

    /// §3: the light is the relation colour × the hologram's intensity —
    /// DEA1 cp2 mid-dip, I 0.488, read (0, 42, 104) in RAM.
    #[test]
    fn a_posts_light_dims_with_its_hologram() {
        assert_eq!(dimmed([1, 86, 213], 0.488), [0, 42, 104]);
        assert_eq!(dimmed([200, 100, 100], 1.0), [200, 100, 100]);
        assert_eq!(dimmed([200, 100, 100], 0.0), [0, 0, 0]);
    }
}
