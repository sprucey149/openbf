//! Allocation policies shared by render passes; instrumentation stays in `made`.

/// Maximum number of per-resolution target sets cached per post-processing pass.
pub(crate) const MAX_CACHED_SIZES: usize = 4;

/// Move a matching cached target entry to the most-recently-used back slot, or
/// return `false` if none matches.
pub(crate) fn touch_cached<T>(cache: &mut Vec<T>, matches: impl FnMut(&T) -> bool) -> bool {
    let Some(idx) = cache.iter().position(matches) else {
        return false;
    };
    if idx + 1 != cache.len() {
        let entry = cache.remove(idx);
        cache.push(entry);
    }
    true
}

/// Insert a newly allocated target entry, evicting the least-recently-used one
/// once [`MAX_CACHED_SIZES`] is reached.
pub(crate) fn push_cached<T>(cache: &mut Vec<T>, entry: T) {
    if cache.len() >= MAX_CACHED_SIZES {
        cache.remove(0);
    }
    cache.push(entry);
}

pub(crate) fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    crate::made::buffer(
        device,
        &wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        },
    )
}

pub(crate) fn texture_2d(
    device: &wgpu::Device,
    label: &str,
    [width, height]: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    view_formats: &[wgpu::TextureFormat],
) -> wgpu::Texture {
    crate::made::texture(
        device,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats,
        },
    )
}
