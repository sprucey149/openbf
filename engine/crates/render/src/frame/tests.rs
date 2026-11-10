use super::*;
use crate::{MeshHandle, SkinnedMeshHandle};

/// The interface light turns with the camera: looking down -Z it is the
/// camera-frame direction itself; turned a quarter left (looking down
/// -X) the camera's x axis is the world's -Z and its z axis +X.
#[test]
fn the_interface_light_is_fixed_to_the_camera() {
    let k = 1.0 / 3f32.sqrt();
    let close = |a: glam::Vec3, b: glam::Vec3| (a - b).length() < 1e-5;
    let mut cam = Camera::default();
    assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, -k)));
    cam.yaw = std::f32::consts::FRAC_PI_2;
    assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, k)));
    cam.position = glam::Vec3::new(5.0, 2.0, -3.0);
    assert!(close(iface_dir(&cam), glam::Vec3::new(-k, -k, k)));
}

/// The blend pass composites far to near through [`sort_far_to_near`],
/// and a tie keeps the input order (a stable sort), so two draws at one
/// origin always composite the same way round.
#[test]
fn blend_draws_sort_far_to_near_and_ties_keep_submission_order() {
    let at = |z: f32| Mat4::from_translation(glam::Vec3::new(0.0, 0.0, z));
    let models = [at(-2.0), at(-9.0), at(-5.0), at(-9.0), at(3.0)];
    let mut order = vec![0, 1, 2, 3, 4];
    sort_far_to_near(&mut order, glam::Vec3::ZERO, |i| models[i]);
    assert_eq!(order, vec![1, 3, 2, 4, 0]);
}

/// Queue SKO: the blend pass is ONE list — the world's BLEND draws the
/// frustum keeps and the skinned BLEND draws, interleaved far to near, a
/// tie keeping the world draw first. Opaque draws of either kind and a
/// culled world draw are not in it. (The GPU referee
/// `skinned::world_and_skinned_blend_draws_composite_far_to_near_together`
/// holds the pass to this order.)
#[test]
fn the_blend_pass_interleaves_world_and_skinned_draws_far_to_near() {
    let at = |z: f32| Mat4::from_translation(glam::Vec3::new(0.0, 0.0, z));
    let (m, t) = (MeshHandle::default(), TextureHandle(0));
    let draws = [
        DrawCmd::textured_blended(m, t, at(-2.0)),
        DrawCmd::textured(m, t, at(-20.0)),
        DrawCmd::textured_blended(m, t, at(-9.0)),
        DrawCmd::textured_blended(m, t, at(-30.0)),
    ];
    let keep = [true, true, true, false];
    let skin = |z: f32, layer| SkinnedDrawCmd {
        mesh: SkinnedMeshHandle(0),
        texture: None,
        color: [1.0; 4],
        model: at(z),
        palette: Vec::new().into(),
        layer,
        light: LightPath::Character,
        lit: None,
    };
    let skinned = [
        skin(-9.0, DrawLayer::Blend),
        skin(-25.0, DrawLayer::Opaque),
        skin(-5.0, DrawLayer::Blend),
    ];
    assert_eq!(
        blend_order(&draws, &keep, &skinned, glam::Vec3::ZERO),
        vec![
            Blended::World(2),
            Blended::Skinned(0),
            Blended::Skinned(2),
            Blended::World(0),
        ]
    );
}

/// Every draw is unculled unless a caller asks, and the ask survives the
/// pass-selecting constructors — the far-scene cull is opt-in per draw,
/// so a renderer with no policy behind it renders exactly as before.
#[test]
fn a_draw_carries_no_cull_until_one_is_asked_for() {
    let m = MeshHandle::default();
    let t = TextureHandle(0);
    assert_eq!(DrawCmd::textured(m, t, Mat4::IDENTITY).cull, None);
    assert_eq!(DrawCmd::textured_masked(m, t, Mat4::IDENTITY).cull, None);
    assert_eq!(DrawCmd::textured_blended(m, t, Mat4::IDENTITY).cull, None);
    assert_eq!(DrawCmd::flat(m, [1.0; 4], Mat4::IDENTITY).cull, None);

    let d = DrawCmd::textured_masked(m, t, Mat4::IDENTITY).with_cull(Some(1500.0));
    assert_eq!(d.cull, Some(1500.0));
    assert_eq!(d.layer, DrawLayer::Mask, "the cull does not move the pass");
    // `None` is a real value a caller can set back, not just the default.
    assert_eq!(d.with_cull(None).cull, None);

    // A cull is per INSTANCE, so two draws that differ only in it still
    // batch into one instanced call: `draw_key` must not see it.
    assert_eq!(
        draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY)),
        draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY).with_cull(Some(800.0)))
    );
}

/// Card LODP: the far switch's LOWDs sort after every other draw of
/// their layer, and still before the next layer.
#[test]
fn a_lowd_sorts_last_in_its_layer() {
    let (m, t) = (MeshHandle(0), TextureHandle(0));
    let side = |side| {
        Some(crate::LodSwitch {
            at: 90.0,
            anchor: glam::Vec3::ZERO,
            radius: 1.0,
            side,
        })
    };
    let lowd =
        draw_key(&DrawCmd::textured(m, t, Mat4::IDENTITY).with_lod(side(crate::LodSide::Lowd)));
    let model = draw_key(
        &DrawCmd::textured(MeshHandle(9), t, Mat4::IDENTITY).with_lod(side(crate::LodSide::Model)),
    );
    let plain = draw_key(&DrawCmd::textured(MeshHandle(9), t, Mat4::IDENTITY));
    let masked = draw_key(&DrawCmd::textured_masked(m, t, Mat4::IDENTITY));
    assert!(model < lowd && plain < lowd && lowd < masked);
    assert_eq!(model, plain, "the model side batches as any draw");
}

/// A section the frame's walk names can be left out by name: a label
/// missing from `frame_ab`'s table could never be priced.
#[test]
fn every_run_section_can_be_left_out() {
    let d = DrawCmd::textured(MeshHandle(0), TextureHandle(0), Mat4::IDENTITY);
    for layer in [
        DrawLayer::Opaque,
        DrawLayer::Mask,
        DrawLayer::TerrainLayer,
        DrawLayer::FarSheet,
        DrawLayer::Spec,
        DrawLayer::Water,
        DrawLayer::Speckle,
        DrawLayer::Blend,
    ] {
        let d = DrawCmd { layer, ..d };
        let section = run_section(&d);
        let ab = crate::FrameAb::parse(&format!("no:{section}"));
        assert!(ab.is_ok_and(|ab| ab.skips(section)), "{section}");
    }
}

fn sorts_at(near: &[Option<f32>]) -> Vec<DrawSort> {
    near.iter()
        .map(|&near| DrawSort {
            near,
            may_discard: false,
        })
        .collect()
}

/// A draw that may discard takes its own run, with its own pipeline, but
/// stays beside its mesh: the mesh order between runs does not move.
#[test]
fn a_draw_that_may_discard_splits_its_run_inside_its_mesh() {
    let t = TextureHandle(0);
    let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
    let draws = [d(1), d(1), d(2)];
    let at = |may_discard| DrawSort {
        near: Some(5.0),
        may_discard,
    };
    let sorts = [at(true), at(false), at(false)];
    assert_eq!(sort_runs(0..3, &draws, &sorts), vec![1, 0, 2]);
    assert_ne!(
        runs_key(&draws[0], sorts[0]).0,
        runs_key(&draws[1], sorts[1]).0
    );
}

#[test]
fn a_packed_key_sorts_as_the_key_and_place_do() {
    let places = [-1.0f32, -0.0, 0.0, 1e-30, 1.0, 512.0, f32::INFINITY];
    for &f in &places {
        assert_eq!(from_total_order(total_order(f)), f.to_bits());
        for &g in &places {
            assert_eq!(total_order(f).cmp(&total_order(g)), f.total_cmp(&g));
        }
    }
    let keys: [RunKey; 8] = [
        (0, 0, 0, 0, 0, 0, false),
        (0, 0, 0, 0, 0, 0, true),
        (0, 0, 0, 0, 0, 1, false),
        (0, 0, 0, 0, u32::MAX, 0, false),
        (0, 0, 0, u32::MAX, 0, 0, false),
        (0, 0, 1, 0, 0, 0, false),
        (0, 1, 0, 0, 0, 0, false),
        (6, 0, 0, 0, 0, 0, false),
    ];
    for w in keys.windows(2) {
        assert!(packed_key(w[0], 512.0) < packed_key(w[1], -1.0));
    }
}

/// Near-first order: runs and the instances in them go nearest first, a
/// run keeps together, and within a bucket the mesh order stands.
#[test]
fn opaque_runs_and_their_instances_go_nearest_first() {
    let t = TextureHandle(0);
    let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
    let draws = [d(1), d(2), d(1), d(3), d(3)];
    let near = sorts_at(&[Some(40.0), Some(100.0), Some(10.0), Some(10.5), Some(50.0)]);
    let order = sort_runs(0..draws.len(), &draws, &near);
    // Mesh 1 (10, 40) and mesh 3 (10.5, 50) tie on their bucket, so mesh
    // order decides between them; mesh 2 starts farther.
    assert_eq!(order, vec![2, 0, 3, 4, 1]);
}

/// One mesh at the eye and across the map splits, so its far instances
/// wait for what hides them.
#[test]
fn a_run_splits_across_three_distance_doublings() {
    let t = TextureHandle(0);
    let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
    let draws = [d(1), d(1), d(1), d(2)];
    let near = sorts_at(&[Some(2.0), Some(3.0), Some(900.0), Some(30.0)]);
    let order = sort_runs(0..draws.len(), &draws, &near);
    assert_eq!(order, vec![0, 1, 3, 2], "mesh 2 between mesh 1's two runs");
    let keys: Vec<_> = order
        .iter()
        .map(|&i| runs_key(&draws[i], near[i]).0)
        .collect();
    assert_eq!(keys[0], keys[1], "the near pair stays one instanced run");
    assert_ne!(keys[1], keys[3], "the far instance is a run of its own");
}

/// A draw that composites keeps its key order, whatever its distance:
/// the terrain's layers paint over each other in mesh order and the water
/// over everything, and they stay after every opaque run.
#[test]
fn composited_layers_do_not_move_for_depth() {
    let (m, t) = (MeshHandle(1), TextureHandle(0));
    let layer = |mesh: u32| DrawCmd::terrain_layer(MeshHandle(mesh), t, Mat4::IDENTITY);
    let draws = [
        layer(9),
        layer(3),
        DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY),
        DrawCmd::textured(m, t, Mat4::IDENTITY),
    ];
    assert!(!may_move(&draws[0], 5.0, None));
    assert!(!may_move(&draws[2], 5.0, None));
    assert!(may_move(&draws[3], 5.0, None));
    let near = sorts_at(&[None, None, None, Some(900.0)]);
    assert_eq!(sort_runs(0..draws.len(), &draws, &near), vec![3, 1, 0, 2]);
}

/// On a fade map only a draw wholly short of the fade moves: past its
/// start an opaque draw composites over what is already down.
#[test]
fn a_draw_reaching_the_far_fade_keeps_its_mesh_order_after_the_rest() {
    let t = TextureHandle(0);
    let d = |mesh| DrawCmd::textured(MeshHandle(mesh), t, Mat4::IDENTITY);
    assert!(may_move(&d(1), 149.0, Some(150.0)));
    assert!(!may_move(&d(1), 150.0, Some(150.0)));
    let draws = [d(1), d(2), d(9)];
    let near = sorts_at(&[None, None, Some(300.0)]);
    assert_eq!(
        sort_runs(0..draws.len(), &draws, &near),
        vec![2, 0, 1],
        "the movable draw goes first even from farther"
    );
}

/// The specular pass adds over its base, so it sorts after every opaque
/// and masked run, and before the water that composites over a model's
/// finished batch (`analysis/re-specular-pass.md` §7). It carries its
/// colour and power in the instance colour.
#[test]
fn the_specular_pass_sorts_between_the_solid_world_and_the_water() {
    let m = MeshHandle::default();
    let t = TextureHandle(0);
    let rank = |d: DrawCmd| draw_key(&d).0;
    let spec = DrawCmd::specular(m, t, [1.0, 0.5, 0.25], 51, Mat4::IDENTITY);
    assert_eq!(spec.layer, DrawLayer::Spec);
    assert_eq!(spec.color, [1.0, 0.5, 0.25, 0.2]);
    let spec = rank(spec);
    assert!(rank(DrawCmd::textured(m, t, Mat4::IDENTITY)) < spec);
    assert!(rank(DrawCmd::textured_masked(m, t, Mat4::IDENTITY)) < spec);
    assert!(spec < rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY)));
}

/// The water pass's position in the sort is what makes it work: it must
/// draw AFTER every opaque and masked run, so the world it composites
/// over is already in the framebuffer — `draw_key`'s layer rank enforces
/// that. That it draws BEFORE every blended draw (so a blended prop over
/// the sea sees the water's depth) is `render_frame`'s pass order, not the
/// key's — a blended draw never reaches `draw_key` — and the GPU referee
/// `water_pass::the_water_still_occludes_what_is_drawn_after_it` holds it.
#[test]
fn the_water_pass_sorts_after_the_solid_world() {
    let m = MeshHandle::default();
    let t = TextureHandle(0);
    let rank = |d: DrawCmd| draw_key(&d).0;
    let opaque = rank(DrawCmd::textured(m, t, Mat4::IDENTITY));
    let mask = rank(DrawCmd::textured_masked(m, t, Mat4::IDENTITY));
    let water = rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY));
    assert!(opaque < water && mask < water, "the water composites OVER");

    // The still plane's two layers are one mesh drawn twice with different
    // textures, the base uploaded first: the key's texture field keeps the
    // foam second within the pass. (A stepped ocean's foam rides its own
    // mesh, uploaded after the base's, so there the mesh field does —
    // `game::water_anim`.)
    let base = draw_key(&DrawCmd::water(
        m,
        TextureHandle(0),
        [1.0; 4],
        Mat4::IDENTITY,
    ));
    let foam = draw_key(&DrawCmd::water(
        m,
        TextureHandle(1),
        [1.0; 4],
        Mat4::IDENTITY,
    ));
    assert!(base < foam, "the foam layer draws over the base");

    // The tint is the instance colour, and the pass does not move.
    let d = DrawCmd::water(m, t, [0.08, 0.09, 0.08, 0.74], Mat4::IDENTITY);
    assert_eq!(d.color, [0.08, 0.09, 0.08, 0.74]);
    assert_eq!(d.layer, DrawLayer::Water);
    assert_eq!(d.cull, None, "the water is unculled until asked");
}

/// Card WADE: the skinned opaque draws go down after the last solid or
/// specular run and before the first water one — and at the end of the
/// runs on a map with no water. The picture is
/// `skinned::an_opaque_skinned_draw_under_the_water_shows_through_it`.
#[test]
fn the_skinned_opaque_draws_precede_the_first_water_run() {
    for solid in [
        DrawLayer::Opaque,
        DrawLayer::Mask,
        DrawLayer::TerrainLayer,
        DrawLayer::FarSheet,
        DrawLayer::Spec,
    ] {
        assert!(!skinned_go_before(Some(draw_key_layer(solid))), "{solid:?}");
    }
    for wet in [DrawLayer::Water, DrawLayer::Speckle] {
        assert!(skinned_go_before(Some(draw_key_layer(wet))), "{wet:?}");
    }
    assert!(skinned_go_before(None), "no water run: after the last run");
}

/// The water's speckle adds over the FINISHED water, so it sorts after
/// every water run (queue W3). That it draws before the blend pass is
/// `render_frame`'s pass order, not the key's: a blended draw never
/// reaches `draw_key`.
#[test]
fn the_speckle_sorts_after_the_water() {
    let rank = |d: DrawCmd| draw_key(&d).0;
    let (m, t) = (MeshHandle::default(), TextureHandle(0));
    let s = DrawCmd::speckle(m, t, [0.5, 0.5, 0.5, 0.25], Mat4::IDENTITY);
    assert_eq!(s.layer, DrawLayer::Speckle);
    assert_eq!(s.color, [0.5, 0.5, 0.5, 0.25]);
    let s = rank(s);
    assert!(rank(DrawCmd::water(m, t, [1.0; 4], Mat4::IDENTITY)) < s);
}

/// The frame's non-sRGB view is made only where it is legal: an sRGB
/// surface needs its plain twin listed, a plain surface is its own twin.
#[test]
fn the_display_view_needs_a_listed_twin_on_an_srgb_surface() {
    use wgpu::TextureFormat::{Bgra8Unorm, Bgra8UnormSrgb};
    assert!(!display_view_allowed(Bgra8UnormSrgb, None));
    assert!(display_view_allowed(Bgra8UnormSrgb, Some(Bgra8Unorm)));
    assert!(display_view_allowed(Bgra8Unorm, None));
}
