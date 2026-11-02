//! Headless GPU tests of the skinned pipelines. A two-bone bar whose top
//! vertices blend 50/50 between bone 0 (identity) and bone 1: translating
//! bone 1 by +2 x must move the top of the bar by +1 x — checks the
//! joints/weights attributes, the palette upload + dynamic offset, and the
//! `vs_skinned` math end to end. Then the two skinned passes, OPAQUE and
//! BLEND, that neither alpha-tests (queue SKA), and where they fall among the
//! world's draws: every opaque draw first, then one blend pass (queue SKO).
//! Ignored by default — needs a real adapter (`scripts/gpu_tests.sh`).

use glam::{Mat4, Vec3};
use render::{Camera, DrawCmd, DrawLayer, SkinnedDrawCmd, SkinnedMeshData};

use crate::common;
use common::{capture_pixel, headless, pixel, skinned_quad, upload_tex, view_quad, N};

/// World point -> pixel through the camera (`N x N` target).
fn project(cam: &Camera, p: Vec3) -> (u32, u32) {
    let clip = cam.view_proj(1.0) * p.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let last = (N - 1) as f32;
    let x = ((ndc.x * 0.5 + 0.5) * N as f32).round().clamp(0.0, last) as u32;
    let y = ((1.0 - (ndc.y * 0.5 + 0.5)) * N as f32)
        .round()
        .clamp(0.0, last) as u32;
    (x, y)
}

fn is_green(p: [u8; 4]) -> bool {
    p[1] > 100 && p[0] < 60
}

#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn two_bone_bar_follows_its_palette() {
    let mut r = headless();
    let cam = Camera::default(); // origin, looking down -Z

    // Vertical bar 0.6 wide from y=-1 to y=+1 at z=-5, two quads. Bottom and
    // middle rows: bone 0 only; top row: half bone 0, half bone 1.
    let z = -5.0;
    let (l, rgt) = (-0.3, 0.3);
    let positions = [
        [l, -1.0, z],
        [rgt, -1.0, z],
        [l, 0.0, z],
        [rgt, 0.0, z],
        [l, 1.0, z],
        [rgt, 1.0, z],
    ];
    let joints = [
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 1, 0, 0],
    ];
    let weights = [
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.5, 0.5, 0.0, 0.0],
        [0.5, 0.5, 0.0, 0.0],
    ];
    let mesh = r
        .add_skinned_mesh(&SkinnedMeshData {
            positions: &positions,
            normals: &[[0.0, 0.0, 1.0]; 6],
            uvs: &[[0.0, 0.0]; 6],
            colors: None,
            joints: &joints,
            weights: &weights,
            indices: &[0, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4],
        })
        .expect("skinned mesh upload");

    let draw = |palette: Vec<Mat4>| SkinnedDrawCmd {
        mesh,
        texture: None,
        color: [0.0, 1.0, 0.0, 1.0],
        model: Mat4::IDENTITY,
        palette: palette.into(),
        layer: DrawLayer::Opaque,
        light: render::LightPath::Character,
        lit: None,
    };
    let clear = [1.0, 0.0, 0.0, 1.0];

    // Rest pose: identity palette -> straight bar; (0, 0.75) is on it.
    r.render_frame(&cam, &[], &[draw(vec![Mat4::IDENTITY; 2])], clear, None)
        .expect("rest frame");
    let (rgba, w, _) = r.capture_rgba().expect("capture rest");
    let (x0, y0) = project(&cam, Vec3::new(0.0, 0.75, z));
    assert!(
        is_green(pixel(&rgba, w, x0, y0)),
        "rest pose bar at {x0},{y0}"
    );
    let (xs, ys) = project(&cam, Vec3::new(0.75, 0.75, z));
    assert_eq!(
        pixel(&rgba, w, xs, ys),
        [255, 0, 0, 255],
        "nothing right of the bar at rest"
    );

    // Bone 1 moved +2 x: top vertices (50/50) shift by +1, so at y=0.75 the
    // bar's centre sits at x=0.75 and (0, 0.75) is bare.
    let posed = vec![
        Mat4::IDENTITY,
        Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
    ];
    r.render_frame(&cam, &[], &[draw(posed)], clear, None)
        .expect("posed frame");
    let (rgba, w, _) = r.capture_rgba().expect("capture posed");
    assert!(is_green(pixel(&rgba, w, xs, ys)), "posed bar at {xs},{ys}");
    assert_eq!(
        pixel(&rgba, w, x0, y0),
        [255, 0, 0, 255],
        "rest position vacated"
    );
    // Bottom half is bone 0 only: unchanged.
    let (xb, yb) = project(&cam, Vec3::new(0.0, -0.5, z));
    assert!(is_green(pixel(&rgba, w, xb, yb)), "bottom half stays put");

    // A palette too small for the mesh's joints is rejected up front.
    let err = r
        .render_frame(&cam, &[], &[draw(vec![Mat4::IDENTITY])], clear, None)
        .expect_err("one-joint palette for a two-joint mesh");
    assert!(format!("{err}").contains("palette"), "{err}");
}

/// One view-filling quad per entry — a 1x1 sheet of `texel`, at depth `z`, on
/// `layer` — the `world` ones as static draws (`common::view_quad`) and the
/// `skinned` ones as skinned draws, each list submitted in the order given,
/// over a black clear; returns the centre pixel. Each quad is built at z = 0
/// and PLACED at `z` by its model matrix, because the blend pass sorts by a
/// draw's model origin.
fn render_scene(
    world: &[([u8; 4], f32, DrawLayer)],
    skinned: &[([u8; 4], f32, DrawLayer)],
) -> [u8; 4] {
    let at = |z: f32| Mat4::from_translation(Vec3::new(0.0, 0.0, z));
    let mut r = headless();
    let mut statics = Vec::new();
    for &(texel, z, layer) in world {
        let texture = upload_tex(&mut r, 1, 1, &texel, "world sheet");
        statics.push(DrawCmd {
            layer,
            ..DrawCmd::textured(view_quad(&mut r, 0.0), texture, at(z))
        });
    }
    let mut draws = Vec::new();
    for &(texel, z, layer) in skinned {
        let texture = upload_tex(&mut r, 1, 1, &texel, "skinned sheet");
        draws.push(SkinnedDrawCmd {
            mesh: skinned_quad(&mut r, 0.0),
            texture: Some(texture),
            color: [1.0; 4],
            model: at(z),
            palette: vec![Mat4::IDENTITY].into(),
            layer,
            light: render::LightPath::Character,
            lit: None,
        });
    }
    r.render_frame(
        &Camera::default(),
        &statics,
        &draws,
        [0.0, 0.0, 0.0, 1.0],
        None,
    )
    .expect("frame");
    capture_pixel(&mut r, N / 2, N / 2)
}

/// [`render_scene`] with skinned quads only.
fn render_skinned(quads: &[([u8; 4], f32, DrawLayer)]) -> [u8; 4] {
    render_scene(&[], quads)
}

/// Queue SKA: the skinned path has NO alpha test, because the disc tests no
/// side model (`analysis/re-side-alpha.md` §0, `[O]`). A sheet whose every
/// texel is at alpha 0.25 draws SOLID on an opaque skinned draw — the port's
/// old 0.5 test discarded all of it — and composites at that alpha on a
/// blended one, the disc's 0x04 segment.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_skinned_draw_ignores_its_sheets_alpha_and_a_blended_one_composites_by_it() {
    let ghost = [255, 255, 255, 64];
    let opaque = render_skinned(&[(ghost, -5.0, DrawLayer::Opaque)]);
    assert!(
        opaque[0] > 200,
        "OPAQUE: the sheet's alpha must be ignored, got {opaque:?}"
    );
    let blended = render_skinned(&[(ghost, -5.0, DrawLayer::Blend)]);
    assert!(
        blended[0] > 20 && blended[0] < 200,
        "BLEND: a quarter-alpha composite over the black clear, got {blended:?}"
    );
}

/// A blended skinned draw composites after EVERY opaque skinned draw, whatever
/// the submission order: the disc's 0x04 segment is Z-tested with no Z write
/// (`TEST 0x51001`), so it composites over the opaque parts already drawn —
/// and a nearer opaque draw hides it.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_blended_skinned_draw_composites_over_the_opaque_ones_and_hides_behind_them() {
    let green_half = [0, 255, 0, 128];
    let red = [255, 0, 0, 255];
    // The blended quad is submitted FIRST and NEARER. It writes no depth, so
    // drawn in submission order the red behind it would paint over it.
    let over = render_skinned(&[
        (green_half, -4.0, DrawLayer::Blend),
        (red, -6.0, DrawLayer::Opaque),
    ]);
    assert!(
        over[0] > 60 && over[1] > 60,
        "the green must composite over the red, got {over:?}"
    );
    let hidden = render_skinned(&[
        (red, -4.0, DrawLayer::Opaque),
        (green_half, -6.0, DrawLayer::Blend),
    ]);
    assert!(
        hidden[0] > 200 && hidden[1] < 30,
        "the nearer red must hide the green behind it, got {hidden:?}"
    );
}

/// Queue SKO: a soldier behind a translucent WORLD surface shows through it.
/// The disc draws a skinned segment and a world one through one function,
/// which files each in a draw bucket by its own `MTRL` word
/// (`analysis/re-side-alpha.md` §3, `[O]`), so the port puts every opaque
/// draw, world and skinned, down before any translucent one composites over
/// it (`frame.rs`'s `blend_order` says what stays [H]). A world BLEND draw
/// writes no depth, so a soldier drawn after it paints straight over it:
/// submitted here BEHIND a quarter-alpha blue pane, the red must come out
/// tinted — before SKO it came out pure red.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn an_opaque_skinned_draw_behind_a_world_blend_draw_shows_through_it() {
    let red = [255, 0, 0, 255];
    let glass = [0, 0, 255, 64];
    let behind = render_scene(
        &[(glass, -4.0, DrawLayer::Blend)],
        &[(red, -6.0, DrawLayer::Opaque)],
    );
    assert!(
        behind[0] > 60 && behind[2] > 15,
        "the red behind must show through the pane, tinted by it, got {behind:?}"
    );
    // Control: a soldier IN FRONT of the pane still hides it — the blend pass
    // is depth-tested against the soldier drawn before it.
    let in_front = render_scene(
        &[(glass, -6.0, DrawLayer::Blend)],
        &[(red, -4.0, DrawLayer::Opaque)],
    );
    assert!(
        in_front[0] > 200 && in_front[2] < 10,
        "the nearer red must hide the pane behind it, got {in_front:?}"
    );
}

/// Card WADE: a soldier standing in the water shows through it. The water
/// blends `Cs·As + Cd·(1−As)` with the depth tested AND written, and both PS2s
/// kick the opaque model bucket before the water's (`analysis/re-water.md`
/// §9.1, `[O]`), so the red BEHIND a half-alpha blue water quad comes out
/// tinted. Drawn after the water, as before the card, it failed the water's
/// depth and the pixel was the water over the clear colour: no red at all.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn an_opaque_skinned_draw_under_the_water_shows_through_it() {
    let red = [255, 0, 0, 255];
    let water = [0, 0, 255, 128];
    let under = render_scene(
        &[(water, -4.0, DrawLayer::Water)],
        &[(red, -6.0, DrawLayer::Opaque)],
    );
    assert!(
        under[0] > 60 && under[2] > 60,
        "the red under the water must show through it, tinted by it, got {under:?}"
    );
    // Control: a soldier in front of the water hides it.
    let over = render_scene(
        &[(water, -6.0, DrawLayer::Water)],
        &[(red, -4.0, DrawLayer::Opaque)],
    );
    assert!(
        over[0] > 200 && over[2] < 10,
        "the nearer red must hide the water behind it, got {over:?}"
    );
}

/// Queue SKO: the world's BLEND draws and the skinned ones are ONE blend pass,
/// composited far to near together — the disc files a translucent segment of
/// either kind in bucket 0xc (BF2 `FUN_0058ed70` 723420,
/// `analysis/re-material-flags.md` §2). Two
/// half-alpha quads over black: whichever is drawn LAST dominates the pixel,
/// so the nearer one must, whichever list it came in. A pass that always
/// draws the world's blends first fails the first case; one that always draws
/// the skinned blends first fails the second.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn world_and_skinned_blend_draws_composite_far_to_near_together() {
    let blue = [0, 0, 255, 128];
    let green = [0, 255, 0, 128];
    let world_near = render_scene(
        &[(blue, -4.0, DrawLayer::Blend)],
        &[(green, -6.0, DrawLayer::Blend)],
    );
    assert!(
        world_near[2] > world_near[1] + 20,
        "the nearer world blue must composite over the skinned green, got {world_near:?}"
    );
    let skinned_near = render_scene(
        &[(blue, -6.0, DrawLayer::Blend)],
        &[(green, -4.0, DrawLayer::Blend)],
    );
    assert!(
        skinned_near[1] > skinned_near[2] + 20,
        "the nearer skinned green must composite over the world blue, got {skinned_near:?}"
    );
}

/// The skinned path has no MASK or WATER pass: a skinned draw asking for
/// one is refused up front rather than drawn some other way.
#[test]
#[ignore = "requires a GPU adapter (run with --ignored on the NVIDIA box)"]
fn a_skinned_draw_on_any_other_pass_is_refused() {
    let mut r = headless();
    let mesh = skinned_quad(&mut r, -5.0);
    for layer in [DrawLayer::Mask, DrawLayer::Water] {
        let draw = SkinnedDrawCmd {
            mesh,
            texture: None,
            color: [1.0; 4],
            model: Mat4::IDENTITY,
            palette: vec![Mat4::IDENTITY].into(),
            layer,
            light: render::LightPath::Character,
            lit: None,
        };
        let err = r
            .render_frame(&Camera::default(), &[], &[draw], [0.0; 4], None)
            .expect_err("a skinned draw off the Opaque and Blend passes");
        assert!(
            format!("{err}").contains("skinned draw"),
            "{layer:?}: {err}"
        );
    }
}
