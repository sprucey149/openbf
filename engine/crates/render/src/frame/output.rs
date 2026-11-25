//! Offscreen frame targets and configurable view projections.

use super::OverlayFn;
use crate::{Camera, DrawCmd, RenderError, Renderer, SkinnedDrawCmd, Target};
use glam::{Mat4, Vec4};

/// Tangents of a view frustum's left, right, bottom, and top bounds relative to
/// the camera's forward ray (`left < right`, `down < up`).
///
/// For a symmetric camera, `left = -aspect * tan(fov_y / 2)`, `right = -left`,
/// `down = -tan(fov_y / 2)`, and `up = -down`. Off-axis views shift these bounds
/// independently around the forward ray.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewFov {
    pub left: f32,
    pub right: f32,
    pub down: f32,
    pub up: f32,
}

impl ViewFov {
    /// Combined `projection * view` matrix using `camera`'s pose and `near`/`far` clip planes.
    pub fn view_projection(self, camera: &Camera) -> Mat4 {
        self.projection(camera.near, camera.far) * camera.view()
    }

    /// Right-handed reverse-Z projection matrix in wgpu clip space (`1` at `near`, `0` at `far`).
    pub fn projection(self, near: f32, far: f32) -> Mat4 {
        let width = self.right - self.left;
        let height = self.up - self.down;
        debug_assert!(
            width > 0.0 && height > 0.0,
            "ViewFov bounds must satisfy left < right and down < up"
        );
        // The world depth buffer is cleared to 0 and keeps larger values.
        // Match Camera's reverse-Z mapping while retaining asymmetric offsets.
        let depth = near / (far - near);
        Mat4::from_cols(
            Vec4::new(2.0 / width, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / height, 0.0, 0.0),
            Vec4::new(
                (self.right + self.left) / width,
                (self.up + self.down) / height,
                depth,
                -1.0,
            ),
            Vec4::new(0.0, 0.0, far * depth, 0.0),
        )
    }
}

/// Projection used when rendering a view.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ViewProjection {
    /// Symmetric perspective derived from [`Camera::fov_y`] and the target's aspect ratio.
    #[default]
    Camera,
    /// Asymmetric (or off-axis) perspective defined by frustum tangents.
    Asymmetric(ViewFov),
}

impl From<ViewFov> for ViewProjection {
    fn from(fov: ViewFov) -> Self {
        Self::Asymmetric(fov)
    }
}

impl ViewProjection {
    /// Combined `projection * view` matrix for `camera` at `aspect` (`width / height`).
    pub fn view_projection(self, camera: &Camera, aspect: f32) -> Mat4 {
        match self {
            Self::Camera => camera.view_proj(aspect),
            Self::Asymmetric(fov) => fov.view_projection(camera),
        }
    }

    pub(crate) fn asymmetric(self) -> Option<ViewFov> {
        match self {
            Self::Camera => None,
            Self::Asymmetric(fov) => Some(fov),
        }
    }
}

/// Caller-owned offscreen render target allocated on a [`Renderer`]'s GPU device.
///
/// Created via [`Renderer::frame_target`]. Each `FrameTarget` owns:
/// - a 2D color texture in [`Renderer::surface_format`] with
///   `RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING` usage and a non-sRGB view format;
/// - its primary [`wgpu::TextureView`] exposed by [`FrameTarget::view`];
/// - a matching depth-stencil texture/view; and
/// - internal sampling state used by the renderer.
///
/// # Ownership and lifetime
/// - Callers own the `FrameTarget` and should reuse it across frames rather than
///   re-allocating per frame (recreate only when dimensions change; drop to release
///   the underlying GPU textures).
/// - A `FrameTarget` is only valid for use with the [`Renderer`] (and underlying
///   [`wgpu::Device`]) that created it.
/// - [`Renderer::render_frame_to`] renders into the target without taking ownership,
///   changing the renderer's default window/headless target, or presenting.
pub struct FrameTarget(pub(super) crate::world_scale::WorldTarget);

impl FrameTarget {
    /// Target dimensions `[width, height]` in pixels.
    pub fn size(&self) -> [u32; 2] {
        self.0.size
    }

    /// Color attachment / sampleable view for this offscreen target.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.0.view
    }
}

impl Renderer {
    /// Begin a logical frame covering one or more views.
    ///
    /// Advances per-frame sky state once upfront; airborne leaf and bird
    /// updates run once on the first view rendered through [`Frame::render_to`]
    /// or [`Frame::render_frame`]. Queued vehicle shadow triangles remain
    /// active for every view in this [`Frame`] and are cleared automatically on
    /// [`Frame::finish`] or drop.
    pub fn begin_frame(&mut self) -> Frame<'_> {
        self.advance_sky();
        Frame {
            renderer: self,
            falling_updated: false,
        }
    }

    /// Also expose the finished desktop frame as a sampleable texture.
    pub fn capture_flat_frame(&mut self, enabled: bool) {
        self.capture_flat = enabled;
    }

    /// Most recent captured flat desktop frame when [`Self::capture_flat_frame`] is enabled.
    pub fn flat_frame(&self) -> Option<&FrameTarget> {
        self.flat_frame.as_ref().filter(|_| self.capture_flat)
    }

    /// Present an offscreen [`FrameTarget`] to the desktop window with optional overlay.
    pub fn mirror_frame(
        &mut self,
        source: &FrameTarget,
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<(), RenderError> {
        self.shadows.clear();
        let Some(frame) = self.acquire_window()? else {
            return Ok(());
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (w, h) = self.size();
        let [sw, sh] = source.size();
        let scale = (w as f32 / sw as f32).min(h as f32 / sh as f32);
        let (mw, mh) = (
            (sw as f32 * scale).min(w as f32),
            (sh as f32 * scale).min(h as f32),
        );
        self.world_scale.blit(
            &mut encoder,
            &source.0,
            &view,
            Some([(w as f32 - mw) * 0.5, (h as f32 - mh) * 0.5, mw, mh]),
        );
        if let Some(overlay) = overlay {
            overlay(&self.device, &self.queue, &mut encoder, &view, [w, h]);
        }
        self.queue.submit([encoder.finish()]);
        self.present_window(frame);
        Ok(())
    }

    /// Allocate an offscreen frame target compatible with this renderer's pipelines.
    pub fn frame_target(&self, size: [u32; 2]) -> Result<FrameTarget, RenderError> {
        let limit = self.device.limits().max_texture_dimension_2d;
        if size.iter().any(|&n| n == 0 || n > limit) {
            return Err(RenderError::BadTexture(format!(
                "view size {size:?} is outside the device's 1..={limit} range"
            )));
        }
        Ok(FrameTarget(self.world_scale.make_target(
            &self.device,
            self.format,
            size,
        )))
    }

    pub(super) fn acquire_window(&mut self) -> Result<Option<wgpu::SurfaceTexture>, RenderError> {
        let Target::Window { surface, .. } = &self.target else {
            return Err(RenderError::Surface("a window target is required".into()));
        };
        match surface.get_current_texture() {
            Ok(frame) => Ok(Some(frame)),
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                // Rebuilt at the window's size, not the stale one; left
                // alone while minimized (#142).
                let window = self.window_size().unwrap_or((0, 0));
                if let Some((w, h)) = crate::device::outdated_surface_size(window) {
                    self.reconfigure_window(w, h);
                }
                Ok(None)
            }
            Err(wgpu::SurfaceError::Timeout) => Ok(None),
            Err(e) => Err(RenderError::Surface(e.to_string())),
        }
    }

    pub(super) fn present_window(&mut self, frame: wgpu::SurfaceTexture) {
        let suboptimal = frame.suboptimal;
        frame.present();
        // A frame presented at a stale size: rebuild the surface at the
        // window's, once the frame is gone (#142). The window is asked
        // only then: on X11 its size is a round trip to the server.
        if suboptimal {
            let window = self.window_size().unwrap_or((0, 0));
            let config = self.size();
            if let Some((w, h)) = crate::device::suboptimal_rebuild_size(suboptimal, window, config)
            {
                self.reconfigure_window(w, h);
            }
        }
    }
}

/// Logical frame handle returned by [`Renderer::begin_frame`].
///
/// Use `Frame` when rendering multiple views of the same simulation frame (such
/// as stereo eyes, mirrors, or offscreen + desktop passes) so per-frame updates
/// (`advance_sky`, airborne leaves/birds, and queued vehicle shadow triangles)
/// happen once per frame and are shared across all views until [`Self::finish`]
/// or drop.
pub struct Frame<'a> {
    renderer: &'a mut Renderer,
    falling_updated: bool,
}

impl<'a> Frame<'a> {
    /// Read-only access to the underlying [`Renderer`] while this frame is active.
    pub fn renderer(&self) -> &Renderer {
        self.renderer
    }

    /// Render a view into a caller-owned offscreen [`FrameTarget`] as part of
    /// this logical frame.
    #[allow(clippy::too_many_arguments)]
    pub fn render_to(
        &mut self,
        target: &FrameTarget,
        camera: &Camera,
        projection: ViewProjection,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
        clear: [f64; 4],
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<(), RenderError> {
        if !self.falling_updated {
            self.renderer.update_falling(camera, target.size().into());
            self.falling_updated = true;
        }
        self.renderer.render_scene(
            camera,
            projection,
            draws,
            skinned,
            clear,
            overlay,
            Some(target),
            false,
            None,
        )
    }

    /// Render the default target (window surface or headless texture) as part of
    /// this logical frame.
    pub fn render_frame(
        &mut self,
        camera: &Camera,
        draws: &[DrawCmd],
        skinned: &[SkinnedDrawCmd],
        clear: [f64; 4],
        overlay: Option<OverlayFn<'_>>,
    ) -> Result<(), RenderError> {
        if !self.falling_updated {
            let size = self.renderer.size();
            self.renderer.update_falling(camera, size);
            self.falling_updated = true;
        }
        self.renderer.render_scene(
            camera,
            ViewProjection::Camera,
            draws,
            skinned,
            clear,
            overlay,
            None,
            false,
            None,
        )
    }

    /// Finish this logical frame and clear any queued per-frame shadow volumes.
    pub fn finish(self) {}
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        self.renderer.shadows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn capture_target(r: &Renderer, target: &FrameTarget) -> Vec<u8> {
        let Target::Headless { view, .. } = &r.target else {
            unreachable!()
        };
        let mut encoder = r.device().create_command_encoder(&Default::default());
        r.world_scale.upscale(&mut encoder, &target.0, view);
        r.queue().submit([encoder.finish()]);
        r.capture_rgba().unwrap().0
    }

    #[test]
    fn asymmetric_frustum_maps_each_corner_and_reverse_z_depth() {
        let bounds = ViewFov {
            left: -1.2,
            right: 0.8,
            down: -0.7,
            up: 1.0,
        };
        let p = bounds.projection(0.1, 100.0);
        let ndc = |x, y, z| {
            let clip = p * Vec4::new(x, y, z, 1.0);
            clip.truncate() / clip.w
        };
        assert!(ndc(-0.12, -0.07, -0.1).abs_diff_eq(Vec3::new(-1.0, -1.0, 1.0), 1e-6));
        assert!(ndc(80.0, 100.0, -100.0).abs_diff_eq(Vec3::new(1.0, 1.0, 0.0), 1e-6));
        assert!(ndc(0.0, 0.0, -1.0).x > 0.0);

        let desktop = Camera {
            near: 0.1,
            far: 100.0,
            ..Camera::default()
        }
        .view_proj(1.0);
        for z in [-0.1, -2.0, -40.0, -100.0] {
            let clip = desktop * Vec4::new(0.0, 0.0, z, 1.0);
            assert!((ndc(0.0, 0.0, z).z - clip.z / clip.w).abs() < 1e-6);
        }
    }

    #[test]
    fn symmetric_view_fov_matches_camera_view_proj() {
        let camera = Camera {
            position: Vec3::new(3.0, -2.0, 5.0),
            yaw: 0.4,
            pitch: -0.2,
            roll: 0.15,
            fov_y: 65f32.to_radians(),
            near: 0.2,
            far: 500.0,
        };
        let aspect = 16.0 / 9.0;
        let tan_half_y = (camera.fov_y * 0.5).tan();
        let tan_half_x = aspect * tan_half_y;
        let fov = ViewFov {
            left: -tan_half_x,
            right: tan_half_x,
            down: -tan_half_y,
            up: tan_half_y,
        };
        let from_camera = ViewProjection::Camera.view_projection(&camera, aspect);
        let from_fov = ViewProjection::from(fov).view_projection(&camera, aspect);
        assert!(from_camera.abs_diff_eq(from_fov, 1e-5));
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn frame_target_rejects_zero_dimensions() {
        let r = Renderer::new_headless(64, 48).unwrap();
        assert!(matches!(
            r.frame_target([0, 64]),
            Err(RenderError::BadTexture(_))
        ));
        assert!(matches!(
            r.frame_target([64, 0]),
            Err(RenderError::BadTexture(_))
        ));
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn offscreen_targets_render_independent_views_and_preserve_default_target() {
        let mut r = Renderer::new_headless(64, 48).unwrap();
        let targets = [
            r.frame_target([64, 64]).unwrap(),
            r.frame_target([128, 64]).unwrap(),
        ];
        assert_eq!(targets[0].size(), [64, 64]);
        assert_eq!(targets[1].size(), [128, 64]);

        let mesh = r
            .add_mesh(&crate::MeshData {
                positions: &[[-0.4, -0.4, -4.0], [0.4, -0.4, -4.0], [0.0, 0.4, -4.0]],
                normals: &[[0.0, 0.0, 1.0]; 3],
                uvs: &[[0.0, 0.0]; 3],
                colors: None,
                indices: &[0, 1, 2],
            })
            .unwrap();
        let green_draw = crate::DrawCmd::flat(mesh, [0.0, 1.0, 0.0, 1.0], Mat4::IDENTITY)
            .with_light(crate::LightPath::Unlit);
        let blue_draw = crate::DrawCmd::flat(
            mesh,
            [0.0, 0.0, 1.0, 1.0],
            Mat4::from_translation(Vec3::X * 10.0),
        )
        .with_light(crate::LightPath::Unlit);
        let draws = [green_draw, blue_draw];
        let clear = [1.0, 0.0, 0.0, 1.0];

        let cam_a = Camera::default();
        let cam_b = Camera {
            position: Vec3::X * 10.0,
            ..Camera::default()
        };

        r.render_frame_to(
            &targets[0],
            &cam_a,
            ViewProjection::Asymmetric(ViewFov {
                left: -1.5,
                right: 0.5,
                down: -1.0,
                up: 1.0,
            }),
            &draws,
            &[],
            clear,
            None,
        )
        .unwrap();
        r.render_frame_to(
            &targets[1],
            &cam_b,
            ViewProjection::Asymmetric(ViewFov {
                left: -0.5,
                right: 1.5,
                down: -1.0,
                up: 1.0,
            }),
            &draws,
            &[],
            clear,
            None,
        )
        .unwrap();

        let channel = |rgba: &[u8], x: usize, c: usize| rgba[(24 * 64 + x) * 4 + c];
        let view_a = capture_target(&r, &targets[0]);
        let view_b = capture_target(&r, &targets[1]);
        assert!(channel(&view_a, 48, 1) > 200 && channel(&view_a, 16, 1) == 0);
        assert!(channel(&view_b, 16, 2) > 200 && channel(&view_b, 48, 2) == 0);

        r.render(&cam_a, &draws, clear).unwrap();
        let (flat, w, h) = r.capture_rgba().unwrap();
        assert_eq!((w, h), (64, 48));
        assert!(channel(&flat, 32, 1) > 200 && channel(&flat, 32, 2) == 0);
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn offscreen_asymmetric_depth_orders_world_and_skinned_draws() {
        use crate::{DrawCmd, DrawLayer, LightPath, MeshData, SkinnedDrawCmd, SkinnedMeshData};

        let size = [64, 64];
        let mut renderer = Renderer::new_headless(size[0], size[1]).unwrap();
        let target = renderer.frame_target(size).unwrap();
        let quad = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let normals = [[0.0, 0.0, 1.0]; 4];
        let uvs = [[0.0; 2]; 4];
        let indices = [0, 1, 2, 2, 1, 3];
        let mesh = renderer
            .add_mesh(&MeshData {
                positions: &quad,
                normals: &normals,
                uvs: &uvs,
                colors: None,
                indices: &indices,
            })
            .unwrap();
        let body = renderer
            .add_skinned_mesh(&SkinnedMeshData {
                positions: &quad,
                normals: &normals,
                uvs: &uvs,
                colors: None,
                joints: &[[0; 4]; 4],
                weights: &[[1.0, 0.0, 0.0, 0.0]; 4],
                indices: &indices,
            })
            .unwrap();
        let model = |at, scale| Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(scale));
        let near_at = Vec3::new(-0.7, 0.0, -2.0);
        let skinned_at = Vec3::new(2.0, 0.0, -6.0);
        let world = [
            DrawCmd::flat(mesh, [1.0, 0.0, 0.0, 1.0], model(Vec3::NEG_Z * 15.0, 24.0))
                .with_light(LightPath::Unlit),
            DrawCmd::flat(mesh, [0.0, 1.0, 0.0, 1.0], model(near_at, 0.2))
                .with_light(LightPath::Unlit),
        ];
        let skinned = [SkinnedDrawCmd {
            mesh: body,
            texture: None,
            color: [0.0, 0.0, 1.0, 1.0],
            model: model(skinned_at, 0.7),
            palette: [Mat4::IDENTITY].into(),
            layer: DrawLayer::Opaque,
            lit: None,
            light: LightPath::Character,
        }];
        let camera = Camera::default();
        for offset in [-0.25, 0.25] {
            let fov = ViewFov {
                left: -1.0 + offset,
                right: 1.0 + offset,
                down: -1.0,
                up: 1.0,
            };
            renderer
                .render_frame_to(
                    &target,
                    &camera,
                    fov.into(),
                    &world,
                    &skinned,
                    [0.0, 0.0, 0.0, 1.0],
                    None,
                )
                .unwrap();
            let rgba = capture_target(&renderer, &target);
            let pixel = |at: Vec3| {
                let clip = fov.view_projection(&camera) * at.extend(1.0);
                let x = ((clip.x / clip.w + 1.0) * 0.5 * size[0] as f32) as usize;
                let n = (size[0] as usize * (size[1] as usize / 2) + x) * 4;
                &rgba[n..n + 4]
            };
            assert_eq!(&rgba[0..4], &[255, 0, 0, 255], "background wall");
            assert_eq!(
                pixel(near_at),
                &[0, 255, 0, 255],
                "near draw in view {offset}"
            );
            let body_px = pixel(skinned_at);
            assert!(
                body_px[2] > 50 && body_px[0] < 10,
                "skinned draw in view {offset}: {body_px:?}"
            );
        }
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn offscreen_asymmetric_sun_flare_matches_off_axis_rays_and_depth() {
        let size = [128, 96];
        let mut r = Renderer::new_headless(size[0], size[1]).unwrap();
        let target = r.frame_target(size).unwrap();
        let blocker = r
            .add_mesh(&crate::MeshData {
                positions: &[[-0.5, -0.5, -2.0], [0.5, -0.5, -2.0], [0.0, 0.5, -2.0]],
                normals: &[[0.0, 0.0, 1.0]; 3],
                uvs: &[[0.0; 2]; 3],
                colors: None,
                indices: &[0, 1, 2],
            })
            .unwrap();
        let shadow = crate::DrawCmd::flat(blocker, [0.0, 0.0, 0.0, 1.0], Mat4::IDENTITY)
            .with_light(crate::LightPath::Unlit);
        r.set_sun_flare(Some(crate::SunFlare {
            display_gain: 1.0,
            distance: 720.0,
            angle: [90.0, -90.0],
            color: [255.0; 4],
            size: 5.0,
            flare_out_size: 20.0,
            num_flare_outs: 12,
            initial_alpha: 70.0,
            halo_rings: [[0.0; 5]; 3],
            spike_color: [0.0; 4],
            spike_size: 0.0,
        }));
        let camera = Camera::default();
        let clear = [0.0, 0.0, 0.0, 1.0];
        for offset in [-0.5, 0.5] {
            let fov = ViewFov {
                left: -1.0 + offset,
                right: 1.0 + offset,
                down: -1.0,
                up: 1.0,
            };
            let x = ((1.0 - offset) * 0.5 * size[0] as f32) as usize;
            let lit = |rgba: &[u8], at: usize| rgba[(48 * 128 + at) * 4];
            r.render_frame_to(&target, &camera, fov.into(), &[], &[], clear, None)
                .unwrap();
            let rgba = capture_target(&r, &target);
            assert!(lit(&rgba, x) > 240);
            let peak = (0..128).max_by_key(|&at| lit(&rgba, at)).unwrap();
            assert!(
                peak.abs_diff(x) <= 2,
                "the source sits on the view's own ray, not the image center ({peak} vs {x})"
            );
            r.render_frame_to(&target, &camera, fov.into(), &[shadow], &[], clear, None)
                .unwrap();
            let rgba = capture_target(&r, &target);
            assert_eq!(
                lit(&rgba, x),
                0,
                "the world blocks the source at its projected position"
            );
        }
    }

    #[test]
    fn post_process_lru_cache_promotes_hits_and_evicts_oldest() {
        let mut cache = Vec::new();
        for size in [[64, 48], [128, 64], [256, 128], [320, 240]] {
            assert!(!crate::resources::touch_cached(&mut cache, |&s| s == size));
            crate::resources::push_cached(&mut cache, size);
        }
        assert_eq!(cache, [[64, 48], [128, 64], [256, 128], [320, 240]]);

        // Reusing [64, 48] promotes it to MRU so [128, 64] becomes the LRU victim.
        assert!(crate::resources::touch_cached(&mut cache, |&s| s == [64, 48]));
        assert_eq!(cache, [[128, 64], [256, 128], [320, 240], [64, 48]]);

        crate::resources::push_cached(&mut cache, [512, 512]);
        assert_eq!(cache, [[256, 128], [320, 240], [64, 48], [512, 512]]);
    }

    fn solid_tex(r: &mut Renderer, rgba: &[u8; 4], label: &str) -> crate::TextureHandle {
        r.add_texture(&crate::TextureDesc {
            width: 1,
            height: 1,
            mips: &[rgba],
            sampler: crate::SamplerMode::Nearest,
            wrap_v: crate::WrapV::Repeat,
            srgb: true,
            detail: None,
            bump: None,
            scroll: [0.0; 2],
            uv_scale: [1.0; 2],
            label: Some(label),
        })
        .unwrap()
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn logical_frame_shares_scene_updates_and_shadows_across_views_and_clears_on_finish() {
        use crate::{DrawCmd, LeafEmitter, LeafObject, LightPath, MeshData, ShadowMesh};

        let size = [64, 64];
        let mut r = Renderer::new_headless(size[0], size[1]).unwrap();
        let target_a = r.frame_target(size).unwrap();
        let target_b = r.frame_target(size).unwrap();

        let leaf_tex = solid_tex(&mut r, &[40, 230, 40, 255], "leaf");
        r.set_leaves(
            Vec::new(),
            vec![
                LeafObject {
                    class: 0,
                    matrix: Mat4::from_translation(Vec3::new(0.0, 0.2, -2.0)),
                    color: [1.0; 4],
                    distance: 100.0,
                },
                LeafObject {
                    class: 0,
                    matrix: Mat4::from_translation(Vec3::new(0.0, 0.2, 2.0)),
                    color: [1.0; 4],
                    distance: 100.0,
                },
            ],
            None,
        );
        r.set_leaf_fall(
            vec![LeafEmitter {
                count: 1,
                birds: 0,
                radius: 0.0,
                height: 0.0,
                scale: 1.0,
                distance: 100.0,
            }],
            Some(leaf_tex),
            Some(Box::new(|_, _| -10.0)),
        );

        let floor_mesh = r
            .add_mesh(&MeshData {
                positions: &[
                    [-20.0, -1.5, -20.0],
                    [-20.0, -1.5, 20.0],
                    [20.0, -1.5, 20.0],
                    [20.0, -1.5, -20.0],
                ],
                normals: &[[0.0, 1.0, 0.0]; 4],
                uvs: &[[0.0; 2]; 4],
                colors: None,
                indices: &[0, 1, 2, 0, 2, 3],
            })
            .unwrap();
        let world = [
            DrawCmd::flat(floor_mesh, [0.8, 0.8, 0.8, 1.0], Mat4::IDENTITY)
                .with_light(LightPath::Unlit),
        ];

        let mut caster = ShadowMesh::new(
            &[
                [-8.0, 0.5, -8.0],
                [8.0, 0.5, -8.0],
                [8.0, 2.0, -8.0],
                [-8.0, 2.0, -8.0],
                [-8.0, 0.5, 8.0],
                [8.0, 0.5, 8.0],
                [8.0, 2.0, 8.0],
                [-8.0, 2.0, 8.0],
            ],
            &[
                vec![0, 3, 2, 1],
                vec![4, 5, 6, 7],
                vec![0, 1, 5, 4],
                vec![1, 2, 6, 5],
                vec![2, 3, 7, 6],
                vec![3, 0, 4, 7],
            ],
        )
        .unwrap();
        let mut shadow_tris = Vec::new();
        caster.append_volume(Mat4::IDENTITY, Vec3::new(0.0, -6.0, 0.0), &mut shadow_tris);

        let cam_a = Camera {
            position: Vec3::new(0.0, 1.0, 0.0),
            pitch: -0.35,
            ..Default::default()
        };
        let cam_b = Camera {
            yaw: std::f32::consts::PI,
            ..cam_a
        };

        // Baseline unshadowed floor brightness in lower image row.
        r.render(&cam_a, &world, [0.0; 4]).unwrap();
        let baseline = r.capture_rgba().unwrap().0;
        let floor_red = |rgba: &[u8]| rgba[(54 * 64 + 32) * 4];
        let unshadowed = floor_red(&baseline);
        assert!(unshadowed > 180, "expected bright floor, got {unshadowed}");

        // Logical frame: first view spawns leaf_a only; queued shadow volume darkens both views.
        r.set_shadow_triangles(&shadow_tris);
        {
            let mut frame = r.begin_frame();
            frame
                .render_to(
                    &target_a,
                    &cam_a,
                    ViewProjection::Camera,
                    &world,
                    &[],
                    [0.0; 4],
                    None,
                )
                .unwrap();
            frame
                .render_to(
                    &target_b,
                    &cam_b,
                    ViewProjection::Camera,
                    &world,
                    &[],
                    [0.0; 4],
                    None,
                )
                .unwrap();
            frame.finish();
        }
        let view_a = capture_target(&r, &target_a);
        let view_b = capture_target(&r, &target_b);
        let has_green_leaf = |rgba: &[u8]| {
            rgba.chunks_exact(4)
                .any(|p| i16::from(p[1]) - i16::from(p[0]) > 40)
        };
        assert!(
            has_green_leaf(&view_a),
            "first view should show spawned leaf"
        );
        assert!(
            !has_green_leaf(&view_b),
            "second view in same logical frame must not spawn new leaf_b"
        );
        assert!(
            floor_red(&view_a) + 20 < unshadowed,
            "first view should receive queued shadows ({} vs {unshadowed})",
            floor_red(&view_a)
        );
        assert!(
            floor_red(&view_b) + 20 < unshadowed,
            "second view should share queued shadows ({} vs {unshadowed})",
            floor_red(&view_b)
        );

        // Frame::finish cleared the queued shadows automatically.
        r.render(&cam_a, &world, [0.0; 4]).unwrap();
        let after_finish = r.capture_rgba().unwrap().0;
        assert_eq!(floor_red(&after_finish), unshadowed);

        // Standalone render_frame_to must also clear queued shadows on completion.
        r.set_shadow_triangles(&shadow_tris);
        r.render_frame_to(
            &target_a,
            &cam_a,
            ViewProjection::Camera,
            &world,
            &[],
            [0.0; 4],
            None,
        )
        .unwrap();
        let standalone = capture_target(&r, &target_a);
        assert!(floor_red(&standalone) + 20 < unshadowed);
        r.render(&cam_a, &world, [0.0; 4]).unwrap();
        let after_standalone = r.capture_rgba().unwrap().0;
        assert_eq!(floor_red(&after_standalone), unshadowed);
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn alternating_view_resolutions_reuse_post_processing_caches() {
        use crate::{
            Bloom, DrawCmd, FrameFilter, LightPath, LitPatch, MeshData, ParticleDraw,
            ParticleShape, TerrainLight, TerrainLights,
        };

        struct MadeGuard;
        impl Drop for MadeGuard {
            fn drop(&mut self) {
                crate::made::enable(false);
                let _ = crate::made::take();
            }
        }

        let mut r = Renderer::new_headless(64, 48).unwrap();
        let target = r.frame_target([128, 64]).unwrap();
        let white_tex = solid_tex(&mut r, &[255, 255, 255, 255], "blur mask");
        r.set_omniproj(Some((1, 1, &[128, 128, 128, 255]))).unwrap();
        r.set_terrain_lights(TerrainLights {
            lights: vec![TerrainLight {
                pos: Vec3::new(0.0, 0.0, -4.0),
                rgb: [1.0, 1.0, 1.0],
            }],
            patches: vec![LitPatch {
                min: [-4.0, -8.0],
                max: [4.0, 0.0],
                lights: [0, 0, 0],
                count: 1,
            }],
        });
        r.set_bloom(Some(Bloom {
            passes: 3,
            weight: 1.1,
            threshold: 0.3,
        }));
        r.set_frame_filter(Some(FrameFilter {
            threshold: 20.0,
            intensity: 30.0,
        }));
        r.set_softness(Some(40.0));
        r.set_particles(vec![ParticleDraw {
            shape: ParticleShape::Blur {
                pos: Vec3::new(0.0, 0.0, -3.0),
                half: 1.5,
                angle: 0.0,
            },
            key: Vec3::new(0.0, 0.0, -3.0),
            color: [255, 255, 255, 255],
            texture: Some(white_tex),
            additive: false,
        }]);

        let quad = r
            .add_mesh(&MeshData {
                positions: &[
                    [-2.0, -2.0, -4.0],
                    [2.0, -2.0, -4.0],
                    [-2.0, 2.0, -4.0],
                    [2.0, 2.0, -4.0],
                ],
                normals: &[[0.0, 1.0, 0.0]; 4],
                uvs: &[[0.0; 2]; 4],
                colors: None,
                indices: &[0, 1, 2, 2, 1, 3],
            })
            .unwrap();
        let draws = [DrawCmd::flat(quad, [0.9, 0.9, 0.9, 1.0], Mat4::IDENTITY)
            .with_light(LightPath::Terrain)];
        let camera = Camera::default();
        let clear = [0.0, 0.0, 0.0, 1.0];

        // Warm up both resolutions [128, 64] and [64, 48].
        {
            let mut frame = r.begin_frame();
            frame
                .render_to(
                    &target,
                    &camera,
                    ViewProjection::Camera,
                    &draws,
                    &[],
                    clear,
                    None,
                )
                .unwrap();
            frame
                .render_frame(&camera, &draws, &[], clear, None)
                .unwrap();
            frame.finish();
        }

        let _guard = MadeGuard;
        crate::made::enable(true);
        let _ = crate::made::take();

        for _ in 0..2 {
            let mut frame = r.begin_frame();
            frame
                .render_to(
                    &target,
                    &camera,
                    ViewProjection::Camera,
                    &draws,
                    &[],
                    clear,
                    None,
                )
                .unwrap();
            frame
                .render_frame(&camera, &draws, &[], clear, None)
                .unwrap();
            frame.finish();
        }

        let made = crate::made::take();
        let textures: Vec<_> = made.iter().filter(|m| m.kind == "texture").collect();
        assert!(
            textures.is_empty(),
            "alternating view resolutions allocated GPU textures: {textures:?}"
        );
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn flat_capture_includes_the_games_overlay_and_restores_regular_rendering() {
        let mut r = Renderer::new_headless(64, 48).unwrap();
        let camera = Camera::default();
        r.capture_flat_frame(true);
        r.render_frame(
            &camera,
            &[],
            &[],
            [1.0, 0.0, 0.0, 1.0],
            Some(&mut |_, _, encoder, view, size| {
                assert_eq!(size, [64, 48]);
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }),
        )
        .unwrap();
        let source = r.flat_frame().unwrap();
        assert_eq!(&capture_target(&r, source)[..4], &[0, 0, 255, 255]);
        r.capture_flat_frame(false);
        assert!(r.flat_frame().is_none());
        r.render(&camera, &[], [0.0, 1.0, 0.0, 1.0]).unwrap();
        assert_eq!(&r.capture_rgba().unwrap().0[..4], &[0, 255, 0, 255]);
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn overlay_layer_draws_unoccluded_in_front_of_closer_geometry() {
        use crate::{DrawCmd, LightPath, MeshData, SamplerMode, TextureDesc, WrapV};
        let mut r = Renderer::new_headless(64, 48).unwrap();
        let quad = r
            .add_mesh(&MeshData {
                positions: &[
                    [-2.0, -2.0, 0.0],
                    [2.0, -2.0, 0.0],
                    [-2.0, 2.0, 0.0],
                    [2.0, 2.0, 0.0],
                ],
                normals: &[[0.0, 0.0, 1.0]; 4],
                uvs: &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
                colors: None,
                indices: &[0, 1, 2, 2, 1, 3],
            })
            .unwrap();
        let white = r
            .add_texture(&TextureDesc {
                width: 1,
                height: 1,
                mips: &[&[255, 255, 255, 255]],
                sampler: SamplerMode::Nearest,
                wrap_v: WrapV::Repeat,
                srgb: true,
                detail: None,
                bump: None,
                scroll: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
                label: Some("overlay white"),
            })
            .unwrap();
        let camera = Camera::default();
        let draws = [
            DrawCmd::flat(
                quad,
                [1.0, 0.0, 0.0, 1.0],
                Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -2.0)),
            )
            .with_light(LightPath::Unlit),
            DrawCmd::overlay(
                quad,
                white,
                [0.0, 1.0, 0.0, 1.0],
                Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -5.0)),
            ),
        ];
        r.render(&camera, &draws, [0.0, 0.0, 0.0, 1.0]).unwrap();
        let rgba = r.capture_rgba().unwrap().0;
        let center = &rgba[(24 * 64 + 32) * 4..][..4];
        assert!(
            center[1] > 200 && center[0] < 40,
            "expected green overlay in front of closer red wall, got {center:?}"
        );
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn stereo_visor_extends_fullscreen_fade_across_entire_eye_view() {
        use crate::{Visor, VisorEye, VisorPass};
        let mut r = Renderer::new_headless(128, 128).unwrap();
        let eye = r.frame_target([128, 128]).unwrap();
        let hud = r.frame_target([128, 72]).unwrap();
        let camera = Camera::default();
        let eyes = [-1.0, 1.0].map(|side| VisorEye {
            offset: glam::Vec3::X * side * 0.032,
            orientation: glam::Quat::from_rotation_y(-side * 0.12),
            fov: ViewFov {
                left: -1.1,
                right: 1.1,
                down: -1.3,
                up: 1.3,
            },
        });
        let visor = Visor::new(eyes);
        let pass = VisorPass::new(r.device(), r.surface_format());

        let clear_and_composite = |r: &mut Renderer, hud_clear: wgpu::Color| {
            r.render_frame_to(
                &eye,
                &camera,
                ViewProjection::Camera,
                &[],
                &[],
                [0.8, 0.8, 0.8, 1.0],
                None,
            )
            .unwrap();
            let mut encoder = r.device().create_command_encoder(&Default::default());
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: hud.view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(hud_clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.draw(r, &mut encoder, &eye, &hud, &visor, 0);
            r.queue().submit([encoder.finish()]);
            capture_target(r, &eye)
        };

        let px = |rgba: &[u8], x: usize, y: usize| rgba[(y * 128 + x) * 4];
        let clear_hud = clear_and_composite(&mut r, wgpu::Color::TRANSPARENT);
        let baseline = px(&clear_hud, 0, 0);
        assert!(
            baseline >= 210,
            "expected transparent HUD borders to leave exterior eye pixels un-darkened, got {baseline}"
        );

        let faded = clear_and_composite(
            &mut r,
            wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            },
        );
        let corner = px(&faded, 0, 0);
        let center = px(&faded, 64, 64);
        assert!(
            corner + 40 < baseline,
            "expected full-screen fade to darken exterior eye pixels ({corner} vs {baseline})"
        );
        assert!(
            (i16::from(corner) - i16::from(center)).abs() <= 4,
            "expected uniform veil across exterior ({corner}) and visor center ({center})"
        );
    }

    #[test]
    #[ignore = "GPU referee: scripts/gpu_tests.sh"]
    fn stereo_visor_composites_the_native_hud_edges_over_a_clear_center() {
        use crate::hud::{HudBatch, HudPass, HudVertex};
        use crate::{Visor, VisorEye, VisorPass};

        let size = [128, 96];
        let mut r = Renderer::new_headless(size[0], size[1]).unwrap();
        let hud = r.frame_target(size).unwrap();
        let eye = r.frame_target(size).unwrap();
        let fov = ViewFov {
            left: -1.2,
            right: 0.95,
            down: -0.9,
            up: 0.95,
        };
        let eyes = [-1.0, 1.0].map(|side| VisorEye {
            offset: Vec3::X * side * 0.034,
            orientation: glam::Quat::from_rotation_y(-side * 0.09),
            fov,
        });
        let visor = Visor::new(eyes);
        let pass = VisorPass::new(r.device(), r.surface_format());
        let mut native = HudPass::new(
            r.device(),
            r.queue(),
            r.surface_format(),
            r.display_view_format(),
        );
        let mut verts = Vec::new();
        let mut indices = Vec::new();
        for (x, y, color) in [
            (44.0, 2.0, [1.0, 0.0, 0.0, 1.0]),
            (2.0, 74.0, [0.0, 1.0, 0.0, 1.0]),
            (104.0, 74.0, [1.0, 1.0, 1.0, 0.5]),
        ] {
            let base = verts.len() as u32;
            verts.extend(
                [[x, y], [x + 22.0, y], [x + 22.0, y + 20.0], [x, y + 20.0]].map(|[x, y]| {
                    HudVertex {
                        pos: [x, y, 0.0],
                        uv: [0.0, 0.0],
                        color,
                    }
                }),
            );
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let batches = [HudBatch {
            texture: None,
            additive: false,
            depth: false,
            first: 0,
            count: indices.len() as u32,
        }];
        let mut encoder = r.device().create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hud.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        native.paint(
            &verts,
            &indices,
            &batches,
            r.device(),
            r.queue(),
            &mut encoder,
            hud.view(),
            size,
        );
        r.queue().submit([encoder.finish()]);
        for index in 0..2 {
            r.render_frame_to(
                &eye,
                &Camera::default(),
                fov.into(),
                &[],
                &[],
                [0.0, 0.0, 1.0, 1.0],
                None,
            )
            .unwrap();
            let mut encoder = r.device().create_command_encoder(&Default::default());
            pass.draw(&r, &mut encoder, &eye, &hud, &visor, index);
            r.queue().submit([encoder.finish()]);
            let rgba = capture_target(&r, &eye);
            let center = (48 * 128 + 64) * 4;
            assert_eq!(&rgba[center..center + 4], &[0, 0, 255, 255]);
            let pixels = rgba.chunks_exact(4);
            assert!(pixels.clone().any(|p| p == [255, 0, 0, 255]));
            assert!(pixels.clone().any(|p| p == [0, 255, 0, 255]));
            assert!(pixels
                .clone()
                .any(|p| (125..=129).contains(&p[0]) && p[0] == p[1] && p[2] == 255));
        }
    }
}
