//! The specular second pass ([`DrawLayer::Spec`], queue SPEC): the disc's
//! shared `specular` lookup table, the per-frame parameters its tex-gen reads,
//! and [`spec_texcoord`] — the tex-gen on the CPU, the reference the shader's
//! `vs_spec` is tested against.
//!
//! What the PS2 does (`analysis/re-specular-pass.md`, every claim `[O]`, so
//! PROVISIONAL here): a segment whose material asks for it is drawn a second
//! time, right after its base, with ONE shared 128×128 grey table sampled
//! bilinear with REPEAT at `s = power / 255`, `t = 0.5 + 0.5·d` (§3, §5). R is
//! the per-vertex reflection vector `2(N·Î)N − Î` (Î = eye → vertex), and the
//! disc dots it with a light rotated into a DIFFERENT camera frame than R's —
//! its up and back axes negated — so `d = 2(X·L)(X·R) − L·R`, X the camera's
//! screen-right axis and L the sun's direction of travel (§4: the effective
//! light's camera-right component is mirrored). [`Renderer::spec_mirror`]
//! (default on) switches that to the consistent `d = L·R`. The colour is flat
//! and unlit, `min(1, T·rgb)`, added over the frame weighted by the base
//! pass's own alpha (`Cs·Ad + Cd`, §2, §6).
//!
//! The add is the GS's, in its gamma-space units: `fs_spec` recomputes the
//! base pass's colour, adds in gamma, and writes the linear DIFFERENCE through
//! an additive blend (One, One), so the sRGB target ends where the GS would.
//! Its depth test is `LessEqual` on the base's own depth, no depth write
//! (§7: the pass shares its base's Z and a nearer later draw covers it).
//!
//! A texture with a PC normal map ([`crate::BumpDesc`]) draws the PC's pass
//! instead (`analysis/re-pc-bump.md` §3, `fs_spec`'s `pc_bump_spec`): it
//! samples the PC core's `specularcubemap`, whose +Y face
//! [`Renderer::set_pc_spec_cube`] installs; without one it adds nothing.
//!
//! A texture with a PC environment cube ([`crate::EnvDesc`]) draws the PC's
//! environment pass (`analysis/re-pc-envmap.md`, `pc_env_spec`): its own cube
//! along the world reflection, times the material's colour.

use crate::bindings;
use crate::texture::upload_levels;
use crate::{RenderError, Renderer};
use glam::Vec3;

/// The shader's `SpecParams` (group 3, binding 2): 32 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SpecParams {
    /// The camera's screen-right axis, world space, unit.
    pub cam_right: [f32; 3],
    /// 1 = the disc's mirrored light (`d = 2(X·L)(X·R) − L·R`), 0 = `L·R`.
    pub mirror: f32,
    /// 1 = the PC cube face is installed (binding 3), 0 = it is not and a PC
    /// bump draw's pass adds nothing.
    pub pc_cube: f32,
    pub _pad: [f32; 3],
}

/// The pass's texture coordinate for one vertex: `(power / 255, 0.5 +
/// 0.5·d)` — the CPU twin of `vs_spec`. `n` is the unit world normal, `eye`
/// and `pos` world points, `sun_dir` the direction the light TRAVELS,
/// `cam_right` the camera's right axis; `mirror` picks the disc's frame mix
/// (see the module doc). `t` is not wrapped: the sampler REPEATs.
pub fn spec_texcoord(
    power: f32,
    n: Vec3,
    eye: Vec3,
    pos: Vec3,
    cam_right: Vec3,
    sun_dir: Vec3,
    mirror: bool,
) -> [f32; 2] {
    let i = (pos - eye).normalize();
    let r = 2.0 * n.dot(i) * n - i;
    let l = sun_dir.normalize();
    let lr = l.dot(r);
    let d = if mirror {
        2.0 * cam_right.dot(l) * cam_right.dot(r) - lr
    } else {
        lr
    };
    [power / 255.0, 0.5 + 0.5 * d]
}

impl Renderer {
    /// Install the specular lookup table (`bake specular`: 128×128, RGBA8,
    /// grey), or remove it with `None`. Without a table every
    /// [`DrawLayer::Spec`] draw is skipped. The table is uploaded LINEAR (a
    /// lookup, not a colour) as one level, sampled bilinear with REPEAT and no
    /// mips — the GS's own state for it (`analysis/re-specular-pass.md` §3).
    pub fn set_specular_table(
        &mut self,
        table: Option<(u32, u32, &[u8])>,
    ) -> Result<(), RenderError> {
        let Some((w, h, rgba)) = table else {
            self.spec_table = None;
            self.rebuild_spec_bg();
            return Ok(());
        };
        let view = upload_levels(
            &self.device,
            &self.queue,
            w,
            h,
            &[rgba],
            false,
            Some("specular table"),
        )?;
        let sampler = crate::made::sampler(
            &self.device,
            &wgpu::SamplerDescriptor {
                label: Some("specular table"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            },
        );
        self.spec_table = Some((view, sampler));
        self.rebuild_spec_bg();
        Ok(())
    }

    /// Install the +Y face of the PC core's `specularcubemap` (`bake
    /// pc-spec-cube`: 128×128 RGBA8), or remove it with `None`. Uploaded
    /// LINEAR, one level, and sampled with the table's sampler at a
    /// coordinate inside the face (`pc_bump_spec` in `shader.wgsl`).
    ///
    /// OURS: bilinear without mips; the PC binds it on stage 3 of
    /// `0x702449` under sampler states not read.
    /// trace: `0x702449`'s `SetSamplerState` calls, for its filter.
    pub fn set_pc_spec_cube(&mut self, face: Option<(u32, u32, &[u8])>) -> Result<(), RenderError> {
        self.pc_spec_cube = match face {
            Some((w, h, rgba)) => Some(upload_levels(
                &self.device,
                &self.queue,
                w,
                h,
                &[rgba],
                false,
                Some("pc specular cube"),
            )?),
            None => None,
        };
        self.rebuild_spec_bg();
        Ok(())
    }

    /// Whether [`Renderer::set_pc_spec_cube`] installed a face.
    pub(crate) fn has_pc_spec_cube(&self) -> bool {
        self.pc_spec_cube.is_some()
    }

    fn rebuild_spec_bg(&mut self) {
        let Some((view, sampler)) = &self.spec_table else {
            self.spec_bg = None;
            return;
        };
        self.spec_bg = Some(crate::made::bind_group(
            &self.device,
            &wgpu::BindGroupDescriptor {
                label: Some("specular bg"),
                layout: &self.spec_bgl,
                entries: &[
                    bindings::texture_view(0, view),
                    bindings::sampler(1, sampler),
                    bindings::entire_buffer(2, &self.spec_buf),
                    // No cube: the table, unread (`spec.pc_cube` is 0).
                    bindings::texture_view(3, self.pc_spec_cube.as_ref().unwrap_or(view)),
                ],
            },
        ));
    }
}

/// Group 3 of the specular pipeline: the table, its sampler, [`SpecParams`],
/// the PC cube face.
pub(crate) fn spec_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("specular bgl"),
        entries: &[
            bindings::texture_layout(0, true),
            bindings::sampler_layout(1),
            bindings::uniform_layout(
                2,
                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                false,
                None,
            ),
            bindings::texture_layout(3, true),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_params_is_two_uniform_rows() {
        assert_eq!(std::mem::size_of::<SpecParams>(), 32);
    }

    /// A known camera and sun: the eye straight above a floor (N = +Y), the
    /// camera's right axis +X. For a vertex straight below the eye Î = −Y and
    /// R = 2(N·Î)N − Î = −Y: R points along the light that would glint back
    /// into the eye, so L·R = 1 is the physical highlight.
    /// - A sun travelling straight DOWN (L = −Y): the consistent d = L·R = 1
    ///   (t = 1); the disc's, with X·L = 0, is d = −L·R = −1 (t = 0) — the
    ///   table's OTHER bright edge, as bright, because it is symmetric
    ///   (`re-specular-pass.md` §5).
    /// - A vertex off to the side, with the sun travelling along R: the
    ///   consistent light peaks there (d = +1) and the disc's does not — its
    ///   camera-right component is flipped, `d = 2(X·L)(X·R) − L·R` (§4).
    #[test]
    fn the_texcoord_for_a_known_camera_and_sun() {
        let eye = Vec3::new(0.0, 10.0, 0.0);
        let n = Vec3::Y;
        let x = Vec3::X;
        let [s, t] = spec_texcoord(51.0, n, eye, Vec3::ZERO, x, -Vec3::Y, true);
        assert!((s - 0.2).abs() < 1e-6);
        assert!(t.abs() < 1e-6, "mirrored head-on t {t}");
        let [_, t] = spec_texcoord(51.0, n, eye, Vec3::ZERO, x, -Vec3::Y, false);
        assert!((t - 1.0).abs() < 1e-6, "consistent head-on t {t}");

        // Vertex at (10, 0, 0): Î = (1, −1, 0)/√2, R = 2(N·Î)N − Î =
        // (−1, −1, 0)/√2. A light travelling along R gives L·R = 1.
        let pos = Vec3::new(10.0, 0.0, 0.0);
        let r = Vec3::new(-1.0, -1.0, 0.0).normalize();
        let [_, t] = spec_texcoord(51.0, n, eye, pos, x, r, false);
        assert!((t - 1.0).abs() < 1e-6, "consistent t {t}");
        // Mirrored: X·L = X·R = −1/√2, so d = 2·½ − 1 = 0 → t = 0.5 (the
        // table's dark middle at every power above column 0).
        let [_, t] = spec_texcoord(51.0, n, eye, pos, x, r, true);
        assert!((t - 0.5).abs() < 1e-6, "mirrored t {t}");
        // The disc's d is −(L − 2(X·L)X)·R: the light with its X component
        // negated, and the sign flipped. So the mirrored light hits the
        // table's OTHER bright edge, t = 0 — the same brightness, because the
        // table is symmetric (§5).
        let lm = Vec3::new(-r.x, r.y, r.z);
        let [_, t] = spec_texcoord(51.0, n, eye, pos, x, lm, true);
        assert!(t.abs() < 1e-6, "mirrored light t {t}");
    }
}
