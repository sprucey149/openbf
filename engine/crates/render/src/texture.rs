//! Texture registry: raw RGBA8 (+ optional pre-built mip chain) in, bind group out.
//!
//! The bake pipeline stores KTX2; the loader decodes to RGBA8 levels and hands them
//! here — this crate deliberately knows nothing about KTX2. Sampler descriptor
//! adapted from an earlier project's terrain sampler (`create_terrain_sampler`), with Repeat
//! addressing because game textures tile.

use crate::bindings;
use crate::RenderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplerMode {
    /// Point sampling (PS2-authentic look for close inspection).
    Nearest,
    /// Bilinear + linear mip blend (trilinear when mips exist).
    Linear,
}

/// V-axis address mode. U (and W) always REPEAT — the discs' panoramas and
/// tiling sheets wrap in U on most maps (`engine/artifacts/sky_wrap/dome_uv.txt`:
/// Hoth's dome runs u = -0.5..1.5, Mygeeto's mountains -4.5..5.6, Felucia's
/// trees -5..5). `ClampToEdge` exists for the sky shells whose bottom ring
/// sits a hair past v = 1.0 and would otherwise wrap to the texture's top row
/// — a stripe of zenith sky painted along the horizon.
///
/// OURS — a stated deviation (`analysis/PLAN_LOG_2026.md`, "Stated deviations from the PS2", deviation 3): the
/// PS2 REPEATs every model draw's texture, sky shells included
/// (`analysis/re-texture-wrap.md` §0, RE_LOG "Texture wrap mode"), so the
/// clamp is the port's own fix for that stripe. It is a per-TEXTURE-upload
/// flag, and `game::viewer::sky_wrap_v` is the one function that decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapV {
    Repeat,
    ClampToEdge,
}

/// A PS2 detail map: the `<name>_dtl` companion of a texture whose `tex_`
/// INFO has `detailCompressed` set (audit A2 §4 — "colors must be modulated
/// x2 against the sibling `*_dtl` texture"). The map carries its signal in
/// ALPHA (RGB is 0) and is registered 1:1 with the base at 4x the resolution,
/// so `scale` is 1.0 for every pair shipped on the discs (RE_LOG "BF1 detail
/// textures"); it stays a field so a tiling pair can be described in data.
#[derive(Debug, Clone, Copy)]
pub struct DetailDesc<'a> {
    pub width: u32,
    pub height: u32,
    /// Mip chain, level 0 first (same rules as `TextureDesc::mips`).
    pub mips: &'a [&'a [u8]],
    /// UV multiplier for the detail lookup; 1.0 = same UVs as the base.
    pub scale: f32,
}

/// CPU-side description of one texture upload.
#[derive(Debug, Clone, Copy)]
pub struct TextureDesc<'a> {
    pub width: u32,
    pub height: u32,
    /// Mip chain, level 0 first; each level tightly-packed RGBA8 of
    /// `max(1, width >> i) * max(1, height >> i) * 4` bytes. One entry = no mips.
    pub mips: &'a [&'a [u8]],
    pub sampler: SamplerMode,
    /// V-axis addressing for this upload — see [`WrapV`]. `Repeat` for
    /// everything but a clamped sky shell.
    pub wrap_v: WrapV,
    /// True for color/albedo textures (bytes are sRGB-encoded, the usual case).
    pub srgb: bool,
    /// Optional PS2 detail map modulated over this texture.
    pub detail: Option<DetailDesc<'a>>,
    /// UV units per second; zero for static materials. BF2 models use baked
    /// byte/255 rates and the 255 s material clock. Water uses the wrapped
    /// `time` clock and its `fx__ Effect("Water")` velocity / INFO5 (magnitude
    /// [V], sign PROVISIONAL; `mungefs::fx::WaterConfig::scroll_per_second`).
    pub scroll: [f32; 2],
    /// Multiplies the mesh UV before the scroll — read ONLY by the water's
    /// speckle layer (`fs_speckle`, [`crate::DrawLayer::Speckle`]) and the
    /// ocean's foam layer (`fs_water`), which sample over the water mesh at
    /// their own tiling; `[1.0, 1.0]` for every other texture, and no other
    /// entry point reads it.
    pub uv_scale: [f32; 2],
    pub label: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub(crate) u32);

pub(crate) struct GpuTexture {
    /// Texture view + sampler baked into one bind group at upload time; draw-time
    /// cost is a single set_bind_group.
    pub bind_group: wgpu::BindGroup,
}

pub(crate) fn mip_extent(base: u32, level: u32) -> u32 {
    (base >> level).max(1)
}

impl GpuTexture {
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        desc: &TextureDesc<'_>,
    ) -> Result<Self, RenderError> {
        let (view, display_view) = upload_level_views(
            device,
            queue,
            desc.width,
            desc.height,
            desc.mips,
            desc.srgb,
            desc.label,
        )?;
        // A detail map is a multiplier, never colour: upload it linear.
        let detail_view = match &desc.detail {
            Some(d) => Some(upload_levels(
                device, queue, d.width, d.height, d.mips, false, desc.label,
            )?),
            None => None,
        };

        let filter = match desc.sampler {
            SamplerMode::Nearest => wgpu::FilterMode::Nearest,
            SamplerMode::Linear => wgpu::FilterMode::Linear,
        };
        // U and W stay REPEAT unconditionally; only V is described by the
        // desc — see [`WrapV`].
        let sampler = crate::made::sampler(
            device,
            &wgpu::SamplerDescriptor {
                label: desc.label,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: match desc.wrap_v {
                    WrapV::Repeat => wgpu::AddressMode::Repeat,
                    WrapV::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                },
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: filter,
                ..Default::default()
            },
        );

        let (detail_scale, detail_strength) = match &desc.detail {
            Some(d) => (d.scale, 1.0),
            None => (1.0, 0.0),
        };
        let params: [f32; 8] = [
            detail_scale,
            detail_strength,
            f32::from(u8::from(!desc.srgb)),
            0.0,
            desc.scroll[0],
            desc.scroll[1],
            desc.uv_scale[0],
            desc.uv_scale[1],
        ];
        let params_buf = crate::made::buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: desc.label,
                contents: bytemuck::cast_slice(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );

        let bind_group = crate::made::bind_group(
            device,
            &wgpu::BindGroupDescriptor {
                label: desc.label,
                layout,
                entries: &[
                    bindings::texture_view(0, &view),
                    bindings::sampler(1, &sampler),
                    // No detail map: bind the base view (never sampled, but
                    // WebGPU requires every binding to be filled).
                    bindings::texture_view(2, detail_view.as_ref().unwrap_or(&view)),
                    bindings::entire_buffer(3, &params_buf),
                    bindings::texture_view(4, &display_view),
                ],
            },
        );

        Ok(Self { bind_group })
    }
}

/// Create a 2D texture from a tightly-packed RGBA8 mip chain and return its view.
pub(crate) fn upload_levels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    mips: &[&[u8]],
    srgb: bool,
    label: Option<&str>,
) -> Result<wgpu::TextureView, RenderError> {
    Ok(upload_level_views(device, queue, width, height, mips, srgb, label)?.0)
}

fn upload_level_views(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    mips: &[&[u8]],
    srgb: bool,
    label: Option<&str>,
) -> Result<(wgpu::TextureView, wgpu::TextureView), RenderError> {
    if width == 0 || height == 0 || mips.is_empty() {
        return Err(RenderError::BadTexture(
            "zero-sized texture or empty mip chain".into(),
        ));
    }
    for (i, level) in mips.iter().enumerate() {
        let (w, h) = (mip_extent(width, i as u32), mip_extent(height, i as u32));
        let expected = (w * h * 4) as usize;
        if level.len() != expected {
            return Err(RenderError::BadTexture(format!(
                "mip {i}: got {} bytes, expected {expected} ({w}x{h} RGBA8)",
                level.len()
            )));
        }
    }

    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };
    let texture = crate::made::texture(
        device,
        &wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        },
    );

    for (i, level) in mips.iter().enumerate() {
        let (w, h) = (mip_extent(width, i as u32), mip_extent(height, i as u32));
        // queue.write_texture handles staging + row alignment internally
        // (unlike texture->buffer readback, which needs 256-byte rows).
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
    Ok((
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::mip_extent;

    #[test]
    fn mip_extent_clamps_to_one() {
        assert_eq!(mip_extent(256, 0), 256);
        assert_eq!(mip_extent(256, 3), 32);
        assert_eq!(mip_extent(256, 10), 1); // past the chain end
        assert_eq!(mip_extent(100, 1), 50);
    }
}
