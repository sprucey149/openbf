//! Headless "look at it" renders: one sheet per map and per unit, used both by
//! the Tier-2 golden tests (`tests/golden_gallery.rs`) and regenerated as
//! PNGs for the maintainer by `scripts/goldens.sh`
//! (`UPDATE_GOLDENS=1 cargo test -p game --test golden_gallery -- --ignored`);
//! there is no `game --gallery` flag.
//!
//! Every shot is deterministic — cameras come from the baked bounds and from
//! the map's own command posts, poses from fixed clip frames — so a diff
//! between two runs is a real change in the engine or the bake, not noise.
//!
//! A map sheet's TWO panels are both inside the map's own atmosphere since
//! 2026-09-03 (the maintainer's decision 6, PLAN Log "Decisions on the session's
//! report"): a fogged [`vantage_camera`] a flyer could have, and the ground
//! stand. The first panel used to be an unfogged orbital camera hundreds of
//! metres outside the map, which no player ever gets and which looked up at
//! the underside of every sky shell.

use crate::animator::{ActionKind, Body, Move, Special};
use crate::assets::{load_skinned_glb, Scene};
use crate::viewer::{look_at, overview_bounds, upload_scene, upload_skinned, SceneGpu};
use anyhow::{anyhow, Context, Result};
use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, Renderer, SkinnedDrawCmd};
use std::path::Path;

/// Panel size for the map sheets. Two panels side by side per map, so a sheet
/// is 1920x540. These are COMMITTED, so the size is a repo decision as much
/// as a looking one — doubled from 480x270 on 2026-09-03 because the maintainer opens
/// them on a screen and half a 720p frame was too small to see a missing
/// texture in. The whole map set is ~50 MB at this size; going further is
/// what `engine/artifacts/walkabout/` is for, and that is gitignored.
pub const MAP_PANEL: (u32, u32) = (960, 540);
/// Cell size for a unit sheet (four poses side by side, each 5:4). Doubled
/// from 320 with [`MAP_PANEL`] and for the same reason.
pub const UNIT_CELL: u32 = 640;

/// Panel size for one [`walkabout`] view. NOT committed — the walkabout set
/// is a Tier-4 artifact under `engine/artifacts/walkabout/`, so it is sized
/// to be READ on a screen rather than to fit in a repo.
pub const WALK_PANEL: (u32, u32) = (1280, 720);
/// The walkabout's four world bearings, degrees, in sheet order: top row
/// 0 deg then 90 deg, bottom row 180 deg then 270 deg. 0 deg is the camera
/// yaw convention's own zero — looking down −Z (`render::Camera::yaw`).
pub const WALK_BEARINGS: [f32; 4] = [0.0, 90.0, 180.0, 270.0];
/// Eye height over the stand: a standing soldier's, the same 1.7 m the sky
/// captures use (`game --example sky_wrap_captures`).
pub const WALK_EYE_HEIGHT: f32 = 1.7;
/// How far in front of the eye the reference soldier stands, metres. Close
/// enough to read as a scale bar, far enough not to fill the frame.
const WALK_SOLDIER_RANGE: f32 = 3.0;
/// How far to the view's left of the post the map sheet's reference soldier
/// stands, metres: clear of the post's console (about 1 m across), still in
/// the post's frame. PROVISIONAL, ours (card SKIPC).
const SHEET_SOLDIER_SIDE: f32 = 1.6;
/// Height of the team-coloured bar painted across the top of each walkabout
/// panel — the only marking on the sheet. Names go in `INDEX.md`. The same
/// height at every scale, so a panel statistic skips exactly these rows.
pub const WALK_BAR: u32 = 4;

/// Poses shown for every unit: (clip, frame). Chosen to exercise the parts
/// that break — the weapon grip, a running stride, a crouch, and death.
pub const UNIT_POSES: [(&str, f32); 4] = [
    ("human_rifle_stand_idle_emote", 0.0),
    ("human_rifle_stand_runforward", 5.0),
    ("human_rifle_crouch_shoot", 2.0),
    ("human_rifle_stand_death_forward", 12.0),
];

/// [`UNIT_POSES`] as the animator's states: a unit sheet shows each through
/// the set of the unit's own weapon (`crate::animator::Rig::still`), so a
/// launcher sits on the shoulder and a super battle droid holds its forearms
/// out.
const UNIT_STATES: [(Body, Option<ActionKind>); 4] = [
    (Body::Feet(0, Move::Idle), None),
    (Body::Feet(0, Move::RunForward), None),
    (Body::Feet(1, Move::Idle), Some(ActionKind::Shoot)),
    (Body::Special(Special::Dead), None),
];

/// What picks a unit sheet's animation set: the class's skeleton family and
/// its primary weapon's animation bank (`crate::roster::class_anim`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSet {
    pub family: String,
    pub bank: String,
}

impl Default for UnitSet {
    fn default() -> Self {
        Self {
            family: "human".into(),
            bank: "rifle".into(),
        }
    }
}

/// Stitch equal-height RGBA panels side by side.
fn stitch(panels: &[(Vec<u8>, u32, u32)]) -> (Vec<u8>, u32, u32) {
    let h = panels.iter().map(|p| p.2).max().unwrap_or(0);
    let w: u32 = panels.iter().map(|p| p.1).sum();
    let mut out = vec![0u8; (w * h * 4) as usize];
    let mut x0 = 0u32;
    for (px, pw, ph) in panels {
        for y in 0..*ph {
            let src = (y * pw * 4) as usize;
            let dst = ((y * w + x0) * 4) as usize;
            out[dst..dst + (*pw * 4) as usize].copy_from_slice(&px[src..src + (*pw * 4) as usize]);
        }
        x0 += pw;
    }
    (out, w, h)
}

/// The middle of the map, for a scene with no command post to stand at.
/// Robust bounds, or one stray prop drags the fallback outside the map
/// entirely — which is exactly what put Death Star's soldier in open space.
fn map_centroid(scene: &Scene) -> Vec3 {
    let pts: Vec<Vec3> = scene
        .manifest
        .instances
        .iter()
        .map(|i| Vec3::from(i.translation))
        .collect();
    let (lo, hi) = crate::viewer::robust_bounds(&pts);
    if lo.x > hi.x {
        Vec3::ZERO
    } else {
        (lo + hi) * 0.5
    }
}

/// Where a soldier stands over `focus`: `focus`'s own XZ, with the terrain
/// taken only as a FLOOR.
///
/// A command post sits on whatever surface the editor put it on, which is
/// often NOT the terrain: Mos Eisley's cp7 is at y = 8.3 on an upper level
/// over desert floor at y = 0, and snapping to the heightfield buried the
/// soldier inside the building underneath. Flooring instead of snapping
/// leaves a post in a pit standing on the ground and a post on a balcony on
/// the balcony.
///
/// Shared by [`ground_view`] (the committed sheet's stand) and [`walkabout`]
/// (every post's stand; its reference soldier stands on the floor under
/// him, [`soldier_spot`]), so the two cannot drift apart.
fn ground_stand(scene: &Scene, focus: Vec3) -> Vec3 {
    let ground = scene
        .heightfield()
        .map_or(focus.y, |h| h.height_at(focus.x, focus.z).max(focus.y));
    Vec3::new(focus.x, ground, focus.z)
}

/// Where the walkabout's reference soldier stands at `p` (card WADE): on the
/// floor under feet at `p`, not at `p`'s own height — the model floor within
/// a step over them (`world`, the scene's collision), else the terrain, read
/// the way `core::sim::Sim::ground_under` reads them (no terrain inside a
/// cutter or more than `TERRAIN_BURY` over the feet). A post floats over its
/// floor: BF2 YAV1's bazaar post is 0.59 m over a floor that lies 0.32 m
/// under the water, and a free PS2 soldier stands on that floor
/// (`analysis/re-water.md` §9.2 `[O]`). With no floor at all under `p` he
/// stays at [`ground_stand`]'s.
///
/// It is the sim's rule without the sim's foot (one ray, no ring): a golden
/// test must not link `core::sim` (`engine/goldens/deps.toml`), and a scale
/// bar needs no more.
fn soldier_spot(scene: &Scene, world: &core::collision::CollisionWorld, p: Vec3) -> Vec3 {
    use core::collision::{STEP_HEIGHT, TERRAIN_BURY};
    let terrain = scene
        .heightfield()
        .map(|h| h.height_at(p.x, p.z))
        .filter(|&t| !world.is_cut(p) && t <= p.y + TERRAIN_BURY);
    let probe = p.y + STEP_HEIGHT;
    let reach = terrain.map_or(probe - core::collision::NO_GROUND, |t| probe - t);
    let model = (reach > 0.0)
        .then(|| world.ray_down(Vec3::new(p.x, probe, p.z), reach))
        .flatten()
        .map(|(y, _)| y);
    match model.or(terrain) {
        Some(floor) => Vec3::new(p.x, floor, p.z),
        None => ground_stand(scene, p),
    }
}

/// Ground-level camera looking AT the map's first command post (else the
/// centre of the world bounds), from behind-right at chase-camera height, and
/// pulled in off any geometry in the way with the same code the game's camera
/// uses. Returns the camera and where the reference soldier stands:
/// `SHEET_SOLDIER_SIDE` to the view's left of the post, because the post's
/// console stands on the post's own point (COMMON merged, card SKIPC) — the
/// camera framed on, and pulled in from, a chest inside the console would
/// end up inside it.
///
/// Public because the sky captures (`game --example sky_wrap_captures`) stand
/// their four-way panorama at exactly the spot this sheet's ground panel
/// uses — one framing, not two that can drift apart.
pub fn ground_view(scene: &Scene) -> (Camera, Vec3) {
    let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
    // No posts: stand at the middle of the map instead.
    let focus = layout
        .cps
        .first()
        .map(|c| c.pos)
        .unwrap_or_else(|| map_centroid(scene));
    // Beside the post, to the left of a camera that sits at bearing `a`
    // from the soldier and looks back at him.
    let beside = |a: f32| {
        let left = Vec3::Y.cross(-Vec3::new(a.cos(), 0.0, a.sin()));
        ground_stand(scene, focus + left * SHEET_SOLDIER_SIDE)
    };
    // A command post in a tight interior (Mos Eisley's alleys, the Polis Massa
    // corridors) blocks the fixed 3.6 m over-the-shoulder offset, and the
    // pull-in then collapses the camera into the soldier's back — a sheet you
    // cannot read. Try eight directions around the post and keep the one the
    // geometry leaves longest. This is the GALLERY's framing only; the game's
    // chase camera is untouched.
    let blockers = crate::camera_collide::CameraBlockers::from_scene(scene);
    let reach = 5.1_f32; // |(3.6, 1.4, 3.6)|
    let mut stand = beside(0.0);
    let mut target = stand + Vec3::Y * 1.1;
    let mut eye = target + Vec3::new(3.6, 1.4, 3.6);
    let mut best = -1.0_f32;
    for k in 0..8 {
        let a = std::f32::consts::TAU * k as f32 / 8.0;
        let s = beside(a);
        let t = s + Vec3::Y * 1.1;
        let desired = t + Vec3::new(a.cos() * 3.6, 1.4, a.sin() * 3.6);
        let got = blockers.pull_in(t, desired);
        let d = (got - t).length();
        if d > best {
            best = d;
            eye = got;
            stand = s;
            target = t;
        }
        if d >= reach - 0.05 {
            break; // unobstructed; no need to keep looking
        }
    }
    let d = target - eye;
    let flat = (d.x * d.x + d.z * d.z).sqrt();
    let cam = Camera {
        position: eye,
        yaw: (-d.x).atan2(-d.z),
        pitch: d.y.atan2(flat.max(1e-3)),
        ..Camera::default()
    };
    (cam, stand)
}

/// The steepest the vantage is allowed to look DOWN, degrees — PROVISIONAL,
/// ours, not the disc's.
///
/// The panel's vertical field of view is 70° ([`Camera`]'s default), so the
/// horizon sits `pitch` below a top edge that is 35° up: at 25° there is 10°
/// of sky left over it, and past 35° there is none at all. Session 13's
/// terrain floor lifted BF2 UTA1's eye 45.6 m over its framing height and the
/// panel came out at 31.81° with a measured ZERO rows of sky at the top —
/// every other baked map has an unbroken band there, and the next worst (BF2
/// KAS2) still has 21 rows.
const VANTAGE_MAX_PITCH_DEG: f32 = 25.0;

/// How many push-out/re-floor passes [`vantage_camera`] takes.
///
/// Moving out along the bearing can cross ground HIGHER than the spot it left,
/// which lifts the eye and re-opens the angle, so the two settle together.
/// Both only ever grow, so the search converges on its own; the bound is here
/// so a pathological map widens its shot rather than hanging. Every one of the
/// 32 baked maps settles in ONE pass (measured).
const VANTAGE_PUSH_PASSES: usize = 4;

/// A vantage a flyer or a high walkway could have — inside the map's fog, a
/// few dozen metres up — instead of the orbital camera hundreds of metres out
/// that no player ever gets (and which saw every sky shell's underside).
///
/// Framing comes from [`overview_bounds`] exactly as [`overview_camera`]'s
/// does, so the two stay comparable: today's +X/+Z diagonal bearing, `0.55 r`
/// out from the centre in XZ, and an eye `max(0.18 r, 18 m)` above the
/// centre, then FLOORED 6 m over the terrain where the map has a heightfield
/// so the camera is never inside the ground.
///
/// That floor is not free: it steepens the look-down angle, and on BF2 UTA1 it
/// steepened it until the horizon left the frame. The eye is never LOWERED to
/// get it back — being inside the ground is the worse picture — it is pushed
/// OUT along the same bearing until the angle is inside
/// [`VANTAGE_MAX_PITCH_DEG`]. A map the floor does not lift keeps exactly the
/// framing it had: `0.18 r / 0.55 r` is 18.1°, well inside the cap.
///
/// PROVISIONAL numbers, the lead's — not tuned by the build. `Camera::default`
/// for a scene with no bounds at all, like [`overview_camera`].
///
/// [`overview_camera`]: crate::viewer::overview_camera
pub fn vantage_camera(scene: &Scene) -> Camera {
    let Some(o) = overview_bounds(scene) else {
        return Camera::default();
    };
    // The bearing `overview_camera` uses, as a unit vector: behind (+Z) and
    // right (+X) of the centre. (The brief spells it 0.7071; clippy's
    // `approx_constant` wants the constant, which is the same bearing to
    // 7e-6 — under 5 mm at the largest map's 0.55 r.)
    const K: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let dir = Vec3::new(K, 0.0, K);

    // 6 m over whatever the map puts under the eye. A model-built map (a stub
    // `tern`, so no heightfield: COR1, DEA1, MYG1, TAN1, UTA1, BF2 KAM1) keeps
    // its floor in the collision world instead — flooring those against the
    // heightfield alone put COR1's vantage UNDER the plaza, looking at its
    // underside (walkabout, session 12) — so cast down from above the map's
    // bounds, where a roof counts too. Both are built ONCE here: the search
    // below asks for the floor at more than one spot.
    let heights = scene.heightfield();
    let bodies = heights.is_none().then(|| scene.collision_world());
    let floor_at = |x: f32, z: f32| -> Option<f32> {
        if let Some(h) = &heights {
            return Some(h.height_at(x, z) + 6.0);
        }
        let top = o.center.y + o.radius;
        bodies
            .as_ref()?
            .ray_down(Vec3::new(x, top, z), 2.0 * o.radius)
            .map(|(y, _)| y + 6.0)
    };

    let max_tan = VANTAGE_MAX_PITCH_DEG.to_radians().tan();
    let mut dist = 0.55 * o.radius;
    let mut eye_y = o.center.y + (0.18 * o.radius).max(18.0);
    for _ in 0..VANTAGE_PUSH_PASSES {
        let p = o.center + dir * dist;
        if let Some(f) = floor_at(p.x, p.z) {
            eye_y = eye_y.max(f);
        }
        // The XZ distance this height needs to look down no steeper than the
        // cap. Shorter than `dist` on an unlifted map, so it breaks at once
        // and nothing moves.
        let need = (eye_y - o.center.y) / max_tan;
        if need <= dist {
            break;
        }
        dist = need;
    }
    let mut eye = o.center + dir * dist;
    // The last pass can move out without re-flooring. Ask once more: staying
    // above the ground wins over the cap wherever a map makes the two
    // disagree.
    if let Some(f) = floor_at(eye.x, eye.z) {
        eye_y = eye_y.max(f);
    }
    eye.y = eye_y;
    look_at(eye, o.center)
}

/// The gallery's reference soldier, uploaded once per scene: the side's
/// rifleman in [`UNIT_POSES`]`[0]`, textured, ready to be planted anywhere.
///
/// One upload serves every panel of a [`walkabout`] — a map with eight posts
/// draws this soldier 32 times — and the single ground panel of a
/// [`map_sheet`]. It exists so the two sets show the SAME soldier in the same
/// pose; a second copy of this code is how they would stop matching.
struct ReferenceSoldier {
    /// Per skinned submesh: mesh, texture, pass (`upload_skinned`) and
    /// specular second pass (`crate::soldier_vis::part_spec`).
    parts: Vec<(
        render::SkinnedMeshHandle,
        Option<render::TextureHandle>,
        render::DrawLayer,
        crate::soldier_vis::PartSpec,
    )>,
    palette: Vec<Mat4>,
}

impl ReferenceSoldier {
    /// Upload the soldier for `scene`'s source game into `r`.
    ///
    /// `Ok(None)` when that side's GLB is not baked — a map sheet without a
    /// soldier is still a map sheet, and this has always been non-fatal.
    fn upload(r: &mut Renderer, scene: &Scene, assets_dir: &Path) -> Result<Option<Self>> {
        let side = assets_dir.join(format!("side_{}", scene.manifest.source_game));
        let unit = match scene.manifest.source_game.as_str() {
            "bf2" => "rep_inf_ep3trooper",
            _ => "all_inf_soldier",
        };
        let Ok(asset) = load_skinned_glb(&side.join("soldiers").join(format!("{unit}.glb"))) else {
            return Ok(None);
        };
        let pose = crate::animator::find_clip(&asset.clips, UNIT_POSES[0].0)
            .map(|i| asset.clips[i].sample(&asset.skeleton, UNIT_POSES[0].1, true))
            .unwrap_or_else(|| crate::skin::Pose::bind(&asset.skeleton));
        let palette = asset.skeleton.palette(&pose.locals());
        let mut parts = Vec::new();
        for (k, (mesh, mat, layer)) in upload_skinned(r, &asset.mesh)?.into_iter().enumerate() {
            let texture = match mat {
                Some(m) => crate::viewer::upload_texture_file(
                    r,
                    &side.join("textures"),
                    &m.to_ascii_lowercase(),
                )?,
                None => None,
            };
            let spec = crate::soldier_vis::part_spec(r, asset.mesh.spec_pass(k));
            parts.push((mesh, texture, layer, spec));
        }
        Ok(Some(Self { parts, palette }))
    }

    /// Stand the soldier at `pos` FACING a camera whose yaw is `cam_yaw`.
    /// Models are +Z forward and a camera at yaw 0 looks down −Z, so the
    /// model's own yaw IS the camera's — no half turn. Verified by looking:
    /// `artifacts/walkabout/tat1/cp5_cp1.png` shows a face.
    fn facing_camera(pos: Vec3, cam_yaw: f32) -> Mat4 {
        Mat4::from_translation(pos) * Mat4::from_rotation_y(cam_yaw)
    }

    /// Stand the soldier at `pos` with his BACK to a camera at `cam_yaw`.
    ///
    /// This is what the committed map sheets have always shown (look at
    /// `goldens/maps/tat1.png`: a backpack, not a face) — the old inline
    /// comment claimed the opposite, and the half turn is the extra one that
    /// makes it so. Kept exactly as it was: correcting the framing here would
    /// move all 32 map goldens for nothing, and the walkabout set is where
    /// the soldier now faces you.
    fn seen_from_behind(pos: Vec3, cam_yaw: f32) -> Mat4 {
        Mat4::from_translation(pos) * Mat4::from_rotation_y(cam_yaw + std::f32::consts::PI)
    }

    /// One draw per part, all at `model`, each with its specular pass.
    fn draws(&self, model: Mat4) -> Vec<SkinnedDrawCmd> {
        self.parts
            .iter()
            .flat_map(|(mesh, texture, layer, spec)| {
                let base = SkinnedDrawCmd {
                    mesh: *mesh,
                    texture: *texture,
                    color: [1.0; 4],
                    model,
                    palette: self.palette.clone().into(),
                    layer: *layer,
                    light: render::LightPath::Character,
                    lit: Some(crate::point_light::soldier_sphere(
                        model.transform_point3(Vec3::ZERO),
                    )),
                };
                crate::soldier_vis::with_spec(base, *spec)
            })
            .collect()
    }
}

/// Two panels for one baked map: the fogged [`vantage_camera`], and a
/// ground-level view at its first command post with a soldier standing there
/// for scale. Both panels use the map's OWN lighting and clear colour, so
/// they share one atmosphere.
pub fn map_sheet(map_dir: &Path, assets_dir: &Path) -> Result<(Vec<u8>, u32, u32)> {
    // The map as a battle loads it: the conquest layer and the COMMON bake.
    // Every BF2 map keeps its command posts in a `lvl_` mode sublevel and none
    // in the base world, so without the layer the "ground view at the first
    // command post" was not at a command post on any of the 16 BF2 maps — it
    // fell through to the whole-map centroid; without COMMON the post had no
    // console and no recharge droid beside it (card SKIPC).
    let (scene, _) = crate::assets::load_battle_scene(map_dir, "con")
        .with_context(|| map_dir.display().to_string())?;
    let (w, h) = MAP_PANEL;
    let mut r = Renderer::new_headless(w, h).map_err(|e| anyhow!("renderer: {e}"))?;
    let mut gpu = upload_scene(&mut r, &scene)?;

    // The vantage panel is inside the map's own atmosphere: its OWN lighting
    // (fog included) and `gpu.clear`, the same clear the ground panel uses.
    // Until 2026-09-03 this panel was an unfogged ORBITAL camera hundreds of
    // metres out, because a map whose fog is authored for a 350 m view (BF1
    // BES2, BF2 END1/FEL1/YAV1) renders as a flat grey square from there —
    // the fix for that is to stop putting the camera where no player stands,
    // not to switch the map's weather off (the maintainer's decision 6).
    let vantage = vantage_camera(&scene);
    step_water(&mut r, &mut gpu, vantage.position)?;
    // The map's precipitation, in both panels (card WTHR): a still state.
    crate::weather::Weather::submit_still(&mut r, &scene.manifest, assets_dir, map_dir, &vantage);
    r.add_particles(crate::holo::still_draws(&gpu.holo, &scene, &vantage));
    r.render(&vantage, &gpu.draws, gpu.clear)
        .map_err(|e| anyhow!("vantage: {e}"))?;
    let (over, _, _) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;

    // Ground view: the same scene plus a reference soldier for the map's game,
    // back to the camera, beside the post (`ground_view`).
    let (cam, stand) = ground_view(&scene);
    let skinned = match ReferenceSoldier::upload(&mut r, &scene, assets_dir)? {
        Some(s) => s.draws(ReferenceSoldier::seen_from_behind(stand, cam.yaw)),
        None => Vec::new(),
    };
    step_water(&mut r, &mut gpu, cam.position)?;
    crate::weather::Weather::submit_still(&mut r, &scene.manifest, assets_dir, map_dir, &cam);
    r.add_particles(crate::holo::still_draws(&gpu.holo, &scene, &cam));
    r.render_frame(&cam, &gpu.draws, &skinned, gpu.clear, None)
        .map_err(|e| anyhow!("ground: {e}"))?;
    let (ground, _, _) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;

    Ok(stitch(&[(over, w, h), (ground, w, h)]))
}

/// The map's water as the game draws it from `eye`, at the water clock's 0 s
/// (`SceneGpu::animate_water`), for a sheet. It changes one thing: an ocean
/// map (both KAM1s) shows its FFT sea, ending at the far pass, where an
/// unstepped scene holds a flat plane (card KSEA, `crate::water_anim`). Flat
/// water is where `upload_scene` left it — the bob is 0 at 0 s and the
/// speckle's first frame is the one already drawn.
fn step_water(r: &mut Renderer, gpu: &mut SceneGpu, eye: Vec3) -> Result<()> {
    gpu.animate_water(r, crate::water_anim::clock(0.0), eye)
}

/// What one walkabout panel submitted, from [`Renderer::frame_stats`].
#[derive(Debug, Clone, Copy)]
pub struct PanelStats {
    /// World bearing of the panel, degrees — see [`WALK_BEARINGS`].
    pub bearing: f32,
    /// `draw_indexed` calls the frame issued, across every pass.
    pub draw_calls: u32,
    /// Draws whose world AABB the frustum removed.
    pub culled: u32,
    /// Draws submitted (the list length minus `culled`).
    pub submitted: u32,
}

/// One image of the walkabout set.
pub struct WalkaboutShot {
    /// File stem: `cp<k>_<name>` for a post, `vantage` for the map's one
    /// vantage shot.
    pub name: String,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The post's own name, `None` for the vantage shot and for the centroid
    /// fallback.
    pub post: Option<String>,
    /// Starting owner of the post: 0 neutral, 1/2 the two teams. The colour
    /// of the bar across the top of each panel.
    pub team: u8,
    /// Where the soldier stands (the vantage shot reports the camera's eye
    /// for both).
    pub stand: Vec3,
    /// The camera's eye: `stand + 1.7 m` for a post.
    pub eye: Vec3,
    /// One entry per panel, in [`WALK_BEARINGS`] order; one entry for the
    /// vantage shot, whose `bearing` is the vantage camera's own yaw.
    pub panels: Vec<PanelStats>,
}

/// A whole map's walkabout: its images, and the scene census behind them.
///
/// The census is [`crate::viewer::upload_scene`]'s own — the same numbers
/// `--scene-report` prints — carried here so `INDEX.md` can say what each
/// map is MISSING next to the pictures that show it.
pub struct WalkaboutSet {
    pub shots: Vec<WalkaboutShot>,
    /// Command posts found in the con layer. 0 means the set fell back to
    /// [`map_centroid`] for its single stand.
    pub posts: usize,
    /// Draws that resolved no texture and fell back to a flat colour.
    pub untextured: usize,
    /// Segments the DISC itself left untextured, skipped rather than drawn.
    pub disc_untextured: usize,
    /// Draws skipped because the material names a texture on neither disc.
    pub dangling: usize,
    /// Terrain chunks dropped for a layer texture in another map's LVL.
    pub terrain_dropped: usize,
    /// Instances skipped for having no drawable model, summed over classes.
    pub skipped: usize,
    /// Sky shell segments uploaded with V clamped (`viewer::sky_wrap_v`).
    pub sky_clamped: usize,
}

/// Four equal panels into one 2x2 sheet: bearing 0 and 90 on the top row,
/// 180 and 270 on the bottom — the layout `game --example sky_wrap_captures`
/// already uses. ([`stitch`] is row-wise only.)
fn grid_2x2(panels: &[(Vec<u8>, u32, u32)]) -> (Vec<u8>, u32, u32) {
    let (pw, ph) = (panels[0].1, panels[0].2);
    let (w, h) = (pw * 2, ph * 2);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for (i, (px, _, _)) in panels.iter().enumerate() {
        let (cx, cy) = ((i as u32 % 2) * pw, (i as u32 / 2) * ph);
        for y in 0..ph {
            let src = (y * pw * 4) as usize;
            let dst = (((cy + y) * w + cx) * 4) as usize;
            out[dst..dst + (pw * 4) as usize].copy_from_slice(&px[src..src + (pw * 4) as usize]);
        }
    }
    (out, w, h)
}

/// Paint a [`WALK_BAR`]-tall bar in `rgb` across the top of the `bw`-wide
/// rectangle at (`x0`, `y0`) in an image `w` px wide.
///
/// This is the ONLY marking on a walkabout sheet: no text, because text in
/// the renderer means a font and a layout pass for a picture whose captions
/// belong in `INDEX.md` anyway. The bar says whose post it is at a glance;
/// which quadrant is which bearing is fixed by [`WALK_BEARINGS`].
fn team_bar(px: &mut [u8], w: u32, x0: u32, y0: u32, bw: u32, rgb: [f32; 3]) {
    let c = rgb.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    for y in y0..y0 + WALK_BAR {
        for x in x0..x0 + bw {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 3].copy_from_slice(&c);
            px[i + 3] = 255;
        }
    }
}

/// A file-name-safe form of a command post's name.
fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_owned();
    if s.is_empty() {
        "unnamed".to_owned()
    } else {
        s
    }
}

/// The walkabout: what the map looks like from where a soldier stands.
///
/// One 2x2 sheet per command post in the con layer, in `layout.cps` order —
/// four [`WALK_PANEL`] panels at world bearings [`WALK_BEARINGS`], level
/// pitch, from an eye [`WALK_EYE_HEIGHT`] over the post's stand
/// ([`ground_stand`], the committed sheet's own rule) — plus one vantage
/// panel per map at the sheet size. The reference soldier stands
/// [`WALK_SOLDIER_RANGE`] in front of the eye along each bearing, facing the
/// camera: a scale bar in every frame, so "that doorway is two metres" is
/// something you can read off the picture.
///
/// Where the committed gallery is a REGRESSION NET (two small panels per map,
/// diffed by `tests/golden_gallery.rs`), this is the LOOKING set: every post
/// rather than the first, every direction rather than one, and big enough to
/// see a missing texture. It is Tier-4 and gitignored — `examples/walkabout.rs`
/// writes it under `engine/artifacts/walkabout/`.
///
/// A map with no post in the con layer falls back to one stand at
/// [`map_centroid`]. (None of the 32 baked maps does — see that folder's
/// `INDEX.md`, which reports the post count per map.)
///
/// `scale` divides every panel edge: 1 is the looking set, 4 the quarter-size
/// set `tests/walkabout_stats.rs` takes per-panel statistics of.
pub fn walkabout(map_dir: &Path, assets_dir: &Path, scale: u32) -> Result<WalkaboutSet> {
    // The posts live in the conquest layer on every BF2 map, and their
    // consoles and droids in the COMMON bake: loaded as `map_sheet` does.
    let (scene, _) = crate::assets::load_battle_scene(map_dir, "con")
        .with_context(|| map_dir.display().to_string())?;
    let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
    let scale = scale.max(1);
    let (pw, ph) = (WALK_PANEL.0 / scale, WALK_PANEL.1 / scale);
    let (sw, sh) = (pw * 2, ph * 2);
    let mut shots = Vec::new();

    // The vantage first, in its OWN renderer: a headless target cannot be
    // resized (`Renderer::resize` is a window-only call), and this shot is the
    // sheet size rather than the panel size. It is dropped before the panel
    // renderer is built so only one scene is ever resident on the GPU.
    let census = {
        let mut r = Renderer::new_headless(sw, sh).map_err(|e| anyhow!("renderer: {e}"))?;
        let mut gpu = upload_scene(&mut r, &scene)?;
        let cam = vantage_camera(&scene);
        step_water(&mut r, &mut gpu, cam.position)?;
        r.render(&cam, &gpu.draws, gpu.clear)
            .map_err(|e| anyhow!("vantage: {e}"))?;
        let st = r.frame_stats();
        let (rgba, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        shots.push(WalkaboutShot {
            name: "vantage".to_owned(),
            rgba,
            width: w,
            height: h,
            post: None,
            team: 0,
            stand: cam.position,
            eye: cam.position,
            panels: vec![PanelStats {
                bearing: cam.yaw.to_degrees(),
                draw_calls: st.draw_calls,
                culled: st.culled,
                submitted: st.submitted,
            }],
        });
        gpu
    };

    let mut r = Renderer::new_headless(pw, ph).map_err(|e| anyhow!("renderer: {e}"))?;
    let mut gpu = upload_scene(&mut r, &scene)?;
    let soldier = ReferenceSoldier::upload(&mut r, &scene, assets_dir)?;
    let floors = scene.collision_world();

    // Every post, else one stand at the map's middle.
    let posts: Vec<(String, Option<String>, u8, Vec3)> = if layout.cps.is_empty() {
        vec![("centroid".to_owned(), None, 0, map_centroid(&scene))]
    } else {
        layout
            .cps
            .iter()
            .enumerate()
            .map(|(k, c)| {
                (
                    format!("cp{k}_{}", slug(&c.name)),
                    Some(c.name.clone()),
                    c.team,
                    c.pos,
                )
            })
            .collect()
    };

    for (name, post, team, pos) in posts {
        let stand = ground_stand(&scene, pos);
        let eye = stand + Vec3::Y * WALK_EYE_HEIGHT;
        // One eye for the post's four bearings, so one step.
        step_water(&mut r, &mut gpu, eye)?;
        let mut panels = Vec::new();
        let mut stats = Vec::new();
        for deg in WALK_BEARINGS {
            let cam = Camera {
                position: eye,
                yaw: deg.to_radians(),
                pitch: 0.0,
                ..Camera::default()
            };
            // Level pitch, so `forward` is already horizontal. The spot is
            // the floor under it rather than the post's own height — three
            // metres off a balcony is a soldier in mid-air, and a post over
            // a flooded floor is a soldier standing on the water.
            let f = cam.forward();
            let spot = soldier_spot(
                &scene,
                &floors,
                Vec3::new(
                    eye.x + f.x * WALK_SOLDIER_RANGE,
                    stand.y,
                    eye.z + f.z * WALK_SOLDIER_RANGE,
                ),
            );
            let skinned = soldier
                .as_ref()
                .map(|s| s.draws(ReferenceSoldier::facing_camera(spot, cam.yaw)))
                .unwrap_or_default();
            r.render_frame(&cam, &gpu.draws, &skinned, gpu.clear, None)
                .map_err(|e| anyhow!("{name} bearing {deg}: {e}"))?;
            let st = r.frame_stats();
            let (px, w, h) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
            stats.push(PanelStats {
                bearing: deg,
                draw_calls: st.draw_calls,
                culled: st.culled,
                submitted: st.submitted,
            });
            panels.push((px, w, h));
        }
        let (mut rgba, w, h) = grid_2x2(&panels);
        let tint = crate::walk::team_color(team.checked_sub(1));
        for i in 0..4u32 {
            team_bar(&mut rgba, w, (i % 2) * pw, (i / 2) * ph, pw, tint);
        }
        shots.push(WalkaboutShot {
            name,
            rgba,
            width: w,
            height: h,
            post,
            team,
            stand,
            eye,
            panels: stats,
        });
    }

    Ok(WalkaboutSet {
        shots,
        posts: layout.cps.len(),
        untextured: census.untextured.iter().map(|(_, _, n)| n).sum(),
        disc_untextured: census.disc_untextured,
        dangling: census.dangling,
        terrain_dropped: census.terrain_dropped,
        skipped: census.skipped.iter().map(|(_, n)| n).sum(),
        sky_clamped: census.sky_clamped.len(),
    })
}

/// One sheet per unit: [`UNIT_POSES`] side by side, textured, with the unit's
/// weapon at `hp_weapons` when one is baked, each pose through the animation
/// set `set` picks (the plain rifle clips when the unit's sets are not
/// baked).
pub fn unit_sheet(glb: &Path, weapon: Option<&Path>, set: &UnitSet) -> Result<(Vec<u8>, u32, u32)> {
    unit_sheet_heading(glb, weapon, set, std::f32::consts::PI * 0.75)
}

/// Asset inspection with a chosen model heading, radians. The default gallery
/// keeps its original view; hero sheets also inspect faces from the front.
pub fn unit_sheet_heading(
    glb: &Path,
    weapon: Option<&Path>,
    set: &UnitSet,
    heading: f32,
) -> Result<(Vec<u8>, u32, u32)> {
    unit_sheet_inner(glb, weapon, set, heading, false)
}

/// Hero asset inspection with four seconds of cloth settling at each held
/// pose. This diagnostic does not alter the ordinary golden gallery.
pub fn unit_sheet_clothed(
    glb: &Path,
    weapon: Option<&Path>,
    set: &UnitSet,
    heading: f32,
) -> Result<(Vec<u8>, u32, u32)> {
    unit_sheet_inner(glb, weapon, set, heading, true)
}

fn unit_sheet_inner(
    glb: &Path,
    weapon: Option<&Path>,
    set: &UnitSet,
    heading: f32,
    cloth: bool,
) -> Result<(Vec<u8>, u32, u32)> {
    let asset = load_skinned_glb(glb).with_context(|| glb.display().to_string())?;
    let rig = glb
        .parent()
        .and_then(Path::parent)
        .zip(glb.file_stem().and_then(|n| n.to_str()))
        .and_then(|(side, model)| {
            crate::soldier_vis::SoldierVisuals::load_rig(side, model, &asset)
        });
    let rig = rig.map(|r| {
        let id = r.weapon_set(&set.family, &set.bank);
        (r, id)
    });
    let tex_dir = glb
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("textures"))
        .unwrap_or_default();
    let (w, h) = (UNIT_CELL, UNIT_CELL * 5 / 4);
    let mut r = Renderer::new_headless(w, h).map_err(|e| anyhow!("renderer: {e}"))?;
    // The side's specular table (queue SPEC2): `<assets>/side_<game>/soldiers/
    // <unit>.glb` → `<assets>/common_<game>/specular.ktx2`. The sheet has no
    // map, so a BF1 sun-coloured pass takes the renderer's default sun.
    let side_dir = glb.parent().and_then(Path::parent);
    if let (Some(assets), Some(game)) = (
        side_dir.and_then(Path::parent),
        side_dir
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("side_")),
    ) {
        crate::viewer::install_spec_table(&mut r, assets, game)?;
    }

    let mut cache: std::collections::HashMap<String, Option<render::TextureHandle>> =
        std::collections::HashMap::new();
    let mut tex_for = |r: &mut Renderer, mat: Option<&str>| -> Option<render::TextureHandle> {
        let name = mat?.to_ascii_lowercase();
        if !cache.contains_key(&name) {
            let h = crate::viewer::upload_texture_file(r, &tex_dir, &name).unwrap_or(None);
            cache.insert(name.clone(), h);
        }
        cache[&name]
    };
    let meshes: Vec<_> = upload_skinned(&mut r, &asset.mesh)?
        .into_iter()
        .enumerate()
        .map(|(k, (m, mat, layer))| {
            let t = tex_for(&mut r, mat);
            let spec = crate::soldier_vis::part_spec(&r, asset.mesh.spec_pass(k));
            (m, t, layer, spec)
        })
        .collect();
    let weapon_parts: Vec<_> = match weapon.map(crate::assets::load_glb).transpose()? {
        Some(cpu) => crate::viewer::upload_submeshes(&mut r, &cpu)?
            .into_iter()
            .enumerate()
            .map(|(k, (m, mat, layer, _))| {
                let t = tex_for(&mut r, mat);
                let spec = crate::soldier_vis::part_spec(&r, cpu.spec_pass(k));
                (m, t, layer, spec)
            })
            .collect(),
        None => Vec::new(),
    };
    let hp = asset.skeleton.index_of("hp_weapons");
    let cloths: Vec<crate::cloth::Definition> = if cloth {
        let path = manifest::cloth::path(side_dir.context("side directory")?, &asset.name);
        let raw: Vec<manifest::cloth::Cloth> = ron::from_str(&pak::read_to_string(path)?)?;
        raw.into_iter()
            .map(|c| crate::cloth::Definition::new(c, &asset.skeleton))
            .collect::<Result<_>>()?
    } else {
        Vec::new()
    };

    let cam = Camera {
        position: Vec3::new(0.0, 1.0, 0.0),
        ..Camera::default()
    };
    let mut panels = Vec::new();
    for ((clip_name, frame), (body, action)) in UNIT_POSES.into_iter().zip(UNIT_STATES) {
        let pose = match (&rig, crate::animator::find_clip(&asset.clips, clip_name)) {
            (Some((rig, set)), _) => {
                rig.still((&asset.clips, &asset.skeleton), *set, body, action, frame)
            }
            (None, Some(i)) => asset.clips[i].sample(&asset.skeleton, frame, true),
            (None, None) => crate::skin::Pose::bind(&asset.skeleton),
        };
        let palette = asset.skeleton.palette(&pose.locals());
        let model =
            Mat4::from_translation(Vec3::new(0.0, 0.0, -2.6)) * Mat4::from_rotation_y(heading);
        let mut statics = Vec::new();
        let globals: Vec<_> = palette
            .iter()
            .zip(&asset.skeleton.global_bind)
            .map(|(p, b)| *p * *b)
            .collect();
        for (k, def) in cloths.iter().enumerate() {
            let mut state = crate::cloth::State::new(def, model, &globals, k as u32);
            for _ in 0..120 {
                state.step(def, model, &globals, 1.0 / 30.0);
            }
            let vertices = state.vertices(def, &globals);
            let positions: Vec<_> = vertices.iter().map(|v| v.position).collect();
            let normals: Vec<_> = vertices.iter().map(|v| v.normal).collect();
            let uvs: Vec<_> = vertices.iter().map(|v| v.uv).collect();
            let indices: Vec<_> = def.raw.triangles.iter().flatten().copied().collect();
            let mesh = r
                .add_dynamic_mesh(&render::MeshData {
                    positions: &positions,
                    normals: &normals,
                    uvs: &uvs,
                    colors: None,
                    indices: &indices,
                })
                .map_err(|e| anyhow!("cloth: {e}"))?;
            let (_, tex, layer, _) = meshes[def.raw.part];
            let mut cmd = match tex {
                Some(t) => DrawCmd::textured(mesh, t, model),
                None => DrawCmd::flat(mesh, [1.; 4], model),
            }
            .with_light(render::LightPath::Character);
            cmd.layer = layer;
            statics.push(cmd);
        }
        if let Some(hp) = hp {
            let global = palette[hp] * asset.skeleton.global_bind[hp];
            for (m, t, layer, spec) in &weapon_parts {
                let mut cmd = DrawCmd::flat(*m, [0.9, 0.2, 0.2, 1.0], model * global);
                if let Some(t) = t {
                    cmd.texture = Some(*t);
                    cmd.color = [1.0; 4];
                    // The weapon bake's own (legacy, per-texture) alpha
                    // decision, written on the material — design D6.
                    cmd.layer = *layer;
                    if let Some((rgb, power)) = spec {
                        statics.push(DrawCmd::specular(*m, *t, *rgb, *power, model * global));
                    }
                }
                statics.push(cmd);
            }
        }
        let skinned: Vec<SkinnedDrawCmd> = meshes
            .iter()
            .enumerate()
            .filter(|(k, _)| !cloths.iter().any(|c| c.raw.part == *k))
            .flat_map(|(_, (mesh, tex, layer, spec))| {
                let base = SkinnedDrawCmd {
                    mesh: *mesh,
                    texture: *tex,
                    color: [1.0; 4],
                    model,
                    palette: palette.clone().into(),
                    layer: *layer,
                    light: render::LightPath::Character,
                    lit: None,
                };
                crate::soldier_vis::with_spec(base, *spec)
            })
            .collect();
        r.render_frame(&cam, &statics, &skinned, [0.16, 0.18, 0.22, 1.0], None)
            .map_err(|e| anyhow!("{clip_name}: {e}"))?;
        let (rgba, pw, ph) = r.capture_rgba().map_err(|e| anyhow!("capture: {e}"))?;
        panels.push((rgba, pw, ph));
    }
    Ok(stitch(&panels))
}

/// Every baked load screen for one source game, thumbnailed into one grid —
/// the loading art is a big slab of decoded content that nothing else in the
/// golden set covers, and a decode regression shows up here immediately.
/// Nearest-neighbour downscale, so the sheet is deterministic.
pub fn loadscreen_sheet(assets_dir: &Path, game: &str) -> Result<(Vec<u8>, u32, u32)> {
    const CELL: u32 = 160;
    const COLS: u32 = 8;
    let mut shots: Vec<(String, crate::assets::CpuTexture)> = Vec::new();
    let mut dirs: Vec<std::path::PathBuf> = crate::assets::discover_maps(assets_dir)
        .into_iter()
        .filter(|(_, g, _)| g == game)
        .map(|(_, _, d)| d)
        .collect();
    dirs.push(assets_dir.join(format!("common_{game}")));
    for d in dirs {
        if let Some(ls) = crate::assets::load_loadscreen(&d) {
            let map = d
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_owned();
            for (name, tex) in ls.sequence {
                shots.push((format!("{map}/{name}"), tex));
            }
        }
    }
    shots.sort_by(|a, b| a.0.cmp(&b.0));
    if shots.is_empty() {
        return Err(anyhow!("no load screens baked for {game}"));
    }
    let rows = shots.len().div_ceil(COLS as usize) as u32;
    let (w, h) = (CELL * COLS, CELL * rows);
    let mut out = vec![24u8; (w * h * 4) as usize];
    for px in out.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    for (i, (_, tex)) in shots.iter().enumerate() {
        let (cx, cy) = ((i as u32 % COLS) * CELL, (i as u32 / COLS) * CELL);
        for y in 0..CELL {
            for x in 0..CELL {
                // Fit the still into the cell, letterboxed, nearest sample.
                let scale = (tex.width as f32 / CELL as f32).max(tex.height as f32 / CELL as f32);
                let (sw, sh) = (
                    (tex.width as f32 / scale) as u32,
                    (tex.height as f32 / scale) as u32,
                );
                let (ox, oy) = ((CELL - sw.min(CELL)) / 2, (CELL - sh.min(CELL)) / 2);
                if x < ox || y < oy || x >= ox + sw || y >= oy + sh {
                    continue;
                }
                let sx = (((x - ox) as f32) * scale) as u32;
                let sy = (((y - oy) as f32) * scale) as u32;
                if sx >= tex.width || sy >= tex.height {
                    continue;
                }
                let src = ((sy * tex.width + sx) * 4) as usize;
                let dst = (((cy + y) * w + cx + x) * 4) as usize;
                out[dst..dst + 4].copy_from_slice(&tex.rgba[src..src + 4]);
                out[dst + 3] = 255;
            }
        }
    }
    Ok((out, w, h))
}

/// The soldier class a gallery sheet of `model` stands for: of the soldier
/// classes whose `GeometryName` is `model`, the first in the faction's
/// [`crate::roster::DEFAULT_CLASSES`] mix, else the first by name. Several
/// classes share one body (BF2 `cis_inf_bdroid` is the rocketeer, engineer,
/// sniper, pilot and marine), so one sheet can only show one of their guns.
pub fn model_class(db: &crate::classdb::ClassDb, game: &str, model: &str) -> Option<String> {
    let side = model.split('_').next().unwrap_or_default();
    let defaults = crate::roster::default_classes(game, side).unwrap_or_default();
    db.names()
        .filter(|c| {
            crate::roster::is_soldier_chain(db, c)
                && db.prop(c, "GeometryName").map(str::trim) == Some(model)
        })
        .min_by_key(|c| {
            (
                defaults.iter().position(|d| d == c).unwrap_or(usize::MAX),
                *c,
            )
        })
        .map(str::to_owned)
}

/// Baked soldier GLBs under `assets_dir`, as (key, model path, weapon path,
/// animation set). The weapon is the one the game draws: [`model_class`]'s
/// primary weapon's geometry ([`crate::roster::weapon_geometry`]), or none
/// when that weapon has no model or its GLB is not baked; the set is that
/// class's with that weapon in hand.
pub fn discover_units(
    assets_dir: &Path,
) -> Vec<(
    String,
    std::path::PathBuf,
    Option<std::path::PathBuf>,
    UnitSet,
)> {
    let mut out = Vec::new();
    for game in ["bf1", "bf2"] {
        let side = assets_dir.join(format!("side_{game}"));
        let Ok(rd) = pak::read_dir(side.join("soldiers")) else {
            continue;
        };
        let db = crate::roster::load_classdb(
            &assets_dir.join("classes"),
            game,
            &crate::roster::FACTION_ORDER,
        )
        .unwrap_or_else(|e| {
            eprintln!("unit gallery: {e:#}");
            crate::classdb::ClassDb::default()
        });
        let mut names: Vec<String> = rd
            .into_iter()
            .filter_map(|p| {
                (p.extension()? == "glb").then(|| p.file_stem()?.to_str().map(str::to_owned))?
            })
            .collect();
        names.sort();
        for n in names {
            let class = model_class(&db, game, &n);
            let weapon = class
                .as_deref()
                .and_then(|c| crate::roster::weapon_geometry(&db, c))
                .map(|g| side.join("weapons").join(format!("{g}.glb")))
                .filter(|p| pak::exists(p));
            let set = class
                .as_deref()
                .and_then(|c| {
                    let anim = crate::roster::class_anim(&db, c, &db.weapon_slots(c));
                    let (primary, _) = crate::roster::primary_weapon(&db, c)?;
                    Some(UnitSet {
                        bank: anim.bank_of(&primary).to_owned(),
                        family: anim.family,
                    })
                })
                .unwrap_or_default();
            out.push((
                format!("{game}_{n}"),
                side.join("soldiers").join(format!("{n}.glb")),
                weapon,
                set,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::CpuMesh;
    use std::collections::HashMap;

    /// A scene with a FLAT heightfield at y = 0 and instances spanning 200 m
    /// in X and Z about the origin — enough for `overview_bounds` and
    /// `Scene::heightfield` to both answer.
    fn flat_scene() -> Scene {
        let inst = |x: f32, z: f32| {
            format!(
                r#"(class: "c", name: "i", rotation: ((1,0,0),(0,1,0),(0,0,1)),
                    translation: ({x},0,{z}), props: [])"#
            )
        };
        let src = format!(
            r#"(
            map: "t", source_game: "bf1",
            terrain_name: None, sky_name: None, terrain_glb: None,
            models: [], textures: [], classes: [], geometries: [],
            regions: [], layers: [], nav: None,
            instances: [{}, {}, {}, {}],
        )"#,
            inst(-100.0, -100.0),
            inst(100.0, -100.0),
            inst(-100.0, 100.0),
            inst(100.0, 100.0),
        );
        // 4x4 grid of terrain vertices at y = 0, 100 m apart, so `height_at`
        // answers 0 anywhere over the map.
        let mut positions = Vec::new();
        for z in 0..4i16 {
            for x in 0..4i16 {
                positions.push([
                    -150.0 + 100.0 * f32::from(x),
                    0.0,
                    -150.0 + 100.0 * f32::from(z),
                ]);
            }
        }
        let n = positions.len();
        Scene {
            dir: std::path::PathBuf::new(),
            manifest: ron::from_str(&src).expect("manifest parses"),
            meshes: HashMap::new(),
            terrain: Some(CpuMesh {
                positions,
                normals: vec![[0.0, 1.0, 0.0]; n],
                uvs: vec![[0.0, 0.0]; n],
                colors: vec![[255; 4]; n],
                indices: Vec::new(),
                submeshes: Vec::new(),
                spec: Vec::new(),
                light: Vec::new(),
                scroll: Vec::new(),
                sphere: None,
                vis_top: None,
            }),
            water: None,
            lowres: None,
            terrain_layers: None,
            textures: HashMap::new(),
            collision: HashMap::new(),
            cutters: Vec::new(),
        }
    }

    /// The map sheet's first panel (the maintainer's decision 6): a vantage inside the
    /// map, not an orbit outside it. PROVISIONAL framing — this pins the
    /// SHAPE (over the ground, at the authored distance, looking down at the
    /// centre), not the lead's placeholder constants beyond what they mean.
    #[test]
    fn the_vantage_camera_stands_over_the_map_looking_down_at_its_centre() {
        let scene = flat_scene();
        let o = crate::viewer::overview_bounds(&scene).expect("bounds");
        let cam = vantage_camera(&scene);

        // Never inside the ground: the flat heightfield is at y = 0 and the
        // floor is +6 m. (Here the 0.18 r term already clears it — the floor
        // is what saves a map whose centre sits in a pit.)
        assert!(
            cam.position.y >= 6.0,
            "vantage camera below the terrain floor: {}",
            cam.position.y
        );
        assert!((cam.position.y - (0.18 * o.radius).max(18.0)).abs() < 1e-3);

        // 0.55 r out from the centre, measured in XZ only.
        let flat =
            ((cam.position.x - o.center.x).powi(2) + (cam.position.z - o.center.z).powi(2)).sqrt();
        assert!(
            (flat - 0.55 * o.radius).abs() < 0.01 * o.radius,
            "vantage camera at {flat} m, expected {} m",
            0.55 * o.radius
        );

        // Looking DOWN at the centre.
        assert!(cam.pitch < 0.0, "vantage camera pitch {}", cam.pitch);
        let want = look_at(cam.position, o.center);
        assert!((cam.yaw - want.yaw).abs() < 1e-5);
        assert!((cam.pitch - want.pitch).abs() < 1e-5);
    }

    /// The floor may not take the horizon with it. A map whose ground under
    /// the vantage spot stands 200 m over the framing centre keeps its eye
    /// where the floor put it and moves OUT until the look-down angle is back
    /// inside [`VANTAGE_MAX_PITCH_DEG`] — BF2 UTA1's case, where the floor
    /// alone left the panel at 31.81 deg.
    #[test]
    fn a_floor_that_lifts_the_vantage_pushes_it_out_instead_of_tipping_it_over() {
        let mut scene = flat_scene();
        // Raise the quarter of the terrain grid the +X/+Z bearing crosses.
        let t = scene.terrain.as_mut().expect("terrain");
        for (i, p) in t.positions.iter_mut().enumerate() {
            if i % 4 >= 2 && i / 4 >= 2 {
                p[1] = 200.0;
            }
        }
        let scene = scene;
        let o = crate::viewer::overview_bounds(&scene).expect("bounds");
        let cam = vantage_camera(&scene);

        // The eye is NOT lowered: it is still 6 m over that ground.
        assert!(
            (cam.position.y - 206.0).abs() < 1e-3,
            "eye at {}, expected the floor's 206 m",
            cam.position.y
        );
        // It is further out than the 0.55 r the framing asks for, and looking
        // down at exactly the cap.
        let flat =
            ((cam.position.x - o.center.x).powi(2) + (cam.position.z - o.center.z).powi(2)).sqrt();
        assert!(
            flat > 0.55 * o.radius,
            "vantage at {flat} m, not pushed out past {}",
            0.55 * o.radius
        );
        let pitch = -cam.pitch.to_degrees();
        assert!(
            (pitch - VANTAGE_MAX_PITCH_DEG).abs() < 0.01,
            "pitch {pitch} deg, expected the {VANTAGE_MAX_PITCH_DEG} deg cap"
        );
        // Still aimed at the centre, like every other vantage.
        let want = look_at(cam.position, o.center);
        assert!((cam.yaw - want.yaw).abs() < 1e-5);
    }

    /// The stand rule both the committed ground panel and every walkabout
    /// panel go through: the terrain is a FLOOR, not a snap. A post in a pit
    /// stands on the ground; a post on a balcony stays on the balcony.
    #[test]
    fn the_stand_takes_the_terrain_as_a_floor_and_never_pulls_a_post_down() {
        let scene = flat_scene(); // heightfield flat at y = 0
        let in_a_pit = ground_stand(&scene, Vec3::new(10.0, -5.0, 10.0));
        assert_eq!(in_a_pit, Vec3::new(10.0, 0.0, 10.0));
        // Mos Eisley's cp7 case: 8.3 m up on an upper level over desert at 0.
        let on_a_balcony = ground_stand(&scene, Vec3::new(10.0, 8.3, 10.0));
        assert_eq!(on_a_balcony, Vec3::new(10.0, 8.3, 10.0));
    }

    /// Card WADE: the walkabout's soldier stands on the floor under him, not at
    /// the post's height. BF2 YAV1's bazaar post is at y = -72.225; three
    /// metres north of it (the PS2 stand `baz_n`, a walkabout bearing's
    /// spot) the floor is -72.818 on the PS2, 0.32 m under the water plane,
    /// and a free PS2 soldier stands on it (`analysis/re-water.md` §9.2,
    /// `[O]`) — the old stand (max of terrain and post) put him 0.59 m up,
    /// on the water. On the post's own base both stand at the post
    /// (the PS2 -72.234).
    #[test]
    fn the_walkabout_soldier_at_yavin_s_bazaar_stands_on_the_flooded_floor() {
        let dir = crate::assets::baked_root().join("bf2_yav1");
        if !dir.join("manifest.ron").exists() {
            eprintln!("no baked bf2_yav1: skipped");
            return;
        }
        let mut scene = crate::assets::load_scene(&dir).expect("bf2_yav1 loads");
        scene.apply_mode_layer("con");
        let layout = crate::layout::MapLayout::from_manifest(&scene.manifest);
        let post = layout
            .cps
            .iter()
            .find(|c| slug(&c.name) == "bazaar")
            .expect("the bazaar post")
            .pos;
        assert!((post.y - -72.225).abs() < 0.01, "the post moved: {post:?}");
        let floors = scene.collision_world();
        let north = Vec3::new(108.9, post.y, -49.9);
        assert!((ground_stand(&scene, north).y - post.y).abs() < 1e-4);
        let spot = soldier_spot(&scene, &floors, north);
        assert!(
            (spot.y - -72.80).abs() <= 0.03,
            "the soldier's feet: {spot:?}"
        );
        assert_eq!((spot.x, spot.z), (north.x, north.z));
        let on_the_base = soldier_spot(&scene, &floors, post);
        assert!((on_the_base.y - -72.214).abs() <= 0.03, "{on_the_base:?}");
    }

    /// With no floor under him at all (no terrain, no model) the soldier
    /// keeps [`ground_stand`]'s height, and on plain terrain he stands on it.
    #[test]
    fn the_walkabout_soldier_stands_on_the_terrain_and_stays_put_over_nothing() {
        let scene = flat_scene();
        let p = Vec3::new(10.0, 0.4, 10.0);
        assert_eq!(soldier_spot(&scene, &scene.collision_world(), p).y, 0.0);
        let bare = Scene {
            terrain: None,
            ..flat_scene()
        };
        assert_eq!(soldier_spot(&bare, &bare.collision_world(), p), p);
    }

    /// The walkabout sheet's reading order — bearing 0 top-left, 90
    /// top-right, 180 bottom-left, 270 bottom-right. Getting this wrong
    /// silently mislabels every row of `INDEX.md`.
    #[test]
    fn a_walkabout_sheet_lays_its_four_bearings_out_in_reading_order() {
        // One 1x1 panel per bearing, each a different red.
        let panels: Vec<(Vec<u8>, u32, u32)> =
            (0..4u8).map(|k| (vec![k, 0, 0, 255], 1, 1)).collect();
        let (px, w, h) = grid_2x2(&panels);
        assert_eq!((w, h), (2, 2));
        let at = |x: usize, y: usize| px[(y * 2 + x) * 4];
        assert_eq!((at(0, 0), at(1, 0), at(0, 1), at(1, 1)), (0, 1, 2, 3));
    }

    /// The bar marks the top of its own panel and nothing else — it must not
    /// bleed into the panel beside it or the one below.
    #[test]
    fn the_team_bar_paints_only_the_top_rows_of_its_own_panel() {
        let (w, h) = (8u32, 8u32);
        let mut px = vec![0u8; (w * h * 4) as usize];
        team_bar(&mut px, w, 4, 4, 4, [1.0, 0.0, 0.0]);
        for y in 0..h {
            for x in 0..w {
                let want = x >= 4 && (4..4 + WALK_BAR).contains(&y);
                let got = px[((y * w + x) * 4) as usize] == 255;
                assert_eq!(got, want, "pixel ({x}, {y})");
            }
        }
    }

    /// Post names come off the disc and land in a file name.
    #[test]
    fn a_post_name_becomes_a_file_name() {
        assert_eq!(slug("CP1"), "cp1");
        assert_eq!(slug("Reflecting Pool"), "reflecting_pool");
        assert_eq!(slug("cp1_1"), "cp1_1");
        assert_eq!(slug("-"), "unnamed");
    }

    /// A scene with neither instances nor terrain falls back exactly as
    /// `overview_camera` does, rather than dividing by an absent radius.
    #[test]
    fn a_scene_with_no_bounds_falls_back_to_the_default_camera() {
        let mut scene = flat_scene();
        scene.manifest.instances.clear();
        scene.terrain = None;
        let cam = vantage_camera(&scene);
        assert_eq!(cam.position, Camera::default().position);
        assert_eq!(cam.yaw, Camera::default().yaw);
        assert_eq!(cam.pitch, Camera::default().pitch);
    }
}
