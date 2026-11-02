//! Headless capture: `Renderer::capture_rgba` reads the last frame back (rows
//! padded to 256 bytes GPU-side, stripped here), and [`encode_png`] /
//! [`encode_png_rgb`] turn the rows into PNG bytes — with the (all-255) alpha
//! plane for the goldens, without it for the sets that exist to be looked at. The
//! padding and encoder tests live here.

use crate::{RenderError, Renderer, Target};

impl Renderer {
    // ── Capture (headless) ───────────────────────────────────────────────────

    /// Read the last rendered headless frame back as tight RGBA8 rows
    /// (sRGB-encoded bytes — exactly what a PNG expects). Returns
    /// `(rgba, width, height)`. Errors on a windowed renderer.
    ///
    /// Adapted from an earlier project's golden-image readback: texture->buffer
    /// copies need `bytes_per_row` aligned to 256, so rows are padded GPU-side
    /// and stripped here; `map_async` only requests, `poll(Wait)` blocks.
    pub fn capture_rgba(&self) -> Result<(Vec<u8>, u32, u32), RenderError> {
        let Target::Headless {
            texture,
            width,
            height,
            ..
        } = &self.target
        else {
            return Err(RenderError::HeadlessOnly);
        };
        let (w, h) = (*width, *height);
        let unpadded = 4 * w;
        let padded = padded_bytes_per_row(w);

        let buffer = crate::resources::buffer(
            &self.device,
            "capture readback",
            u64::from(padded) * u64::from(h),
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("capture encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| RenderError::Readback(format!("poll: {e}")))?;
        rx.recv()
            .map_err(|_| RenderError::Readback("map callback dropped".into()))?
            .map_err(|e| RenderError::Readback(format!("map: {e}")))?;

        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((unpadded * h) as usize);
        for row in 0..h {
            let start = (row * padded) as usize;
            out.extend_from_slice(&data[start..start + unpadded as usize]);
        }
        drop(data);
        buffer.unmap();
        Ok((out, w, h))
    }
}

/// Encode tight RGBA8 rows (as returned by [`Renderer::capture_rgba`]) to PNG bytes.
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    let img: image::ImageBuffer<image::Rgba<u8>, _> =
        image::ImageBuffer::from_raw(width, height, rgba)
            .ok_or_else(|| RenderError::Png("buffer length does not match dimensions".into()))?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| RenderError::Png(e.to_string()))?;
    Ok(out.into_inner())
}

/// Encode tight RGBA8 rows to PNG bytes as **RGB**, dropping the alpha plane.
///
/// The captured alpha USED to be a byproduct of the blend passes: 36.7 % of
/// the pixels in the walkabout's `bf2_end1/vantage.png` came back at alpha
/// 127..254, so a compositing viewer showed those pictures brighter than the
/// engine drew them (defect reports C §0 and D (3)). Since card FALPH no
/// pipeline writes alpha (`FRAME_WRITES`) and every captured pixel is 255, so
/// the goldens' [`encode_png`] carries an all-opaque plane; the sets that
/// exist to be LOOKED at still drop it, a third of the bytes.
pub fn encode_png_rgb(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for px in rgba.as_chunks::<4>().0 {
        rgb.extend_from_slice(&px[..3]);
    }
    let img: image::RgbImage = image::ImageBuffer::from_raw(width, height, rgb)
        .ok_or_else(|| RenderError::Png("buffer length does not match dimensions".into()))?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| RenderError::Png(e.to_string()))?;
    Ok(out.into_inner())
}

/// Texture->buffer copies require `bytes_per_row` aligned to
/// `COPY_BYTES_PER_ROW_ALIGNMENT` (256).
fn padded_bytes_per_row(width: u32) -> u32 {
    (4 * width).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readback_rows_are_padded_to_256_bytes() {
        assert_eq!(padded_bytes_per_row(64), 256); // exact fit
        assert_eq!(padded_bytes_per_row(100), 512); // 400 -> 512
        assert_eq!(padded_bytes_per_row(1), 256); // tiny rows still pad up
        assert_eq!(padded_bytes_per_row(1920), 7680); // 1080p rows are already aligned
    }

    /// `encode_png_rgb` keeps the colour planes and only drops the fourth.
    #[test]
    fn encode_png_rgb_drops_the_alpha_plane_and_keeps_the_colours() {
        // Two pixels whose alpha is exactly what the walkabout was carrying:
        // a fully opaque one and a half-transparent one.
        let rgba = [10u8, 20, 30, 255, 40, 50, 60, 127];
        let png = encode_png_rgb(&rgba, 2, 1).expect("encode");
        let img = image::load_from_memory(&png).expect("decode");
        assert_eq!(img.color(), image::ColorType::Rgb8, "not an RGB PNG");
        let rgb = img.to_rgb8();
        assert_eq!(rgb.as_raw(), &[10, 20, 30, 40, 50, 60]);

        // The same pixels through `encode_png` keep theirs, which is what the
        // goldens depend on.
        let kept =
            image::load_from_memory(&encode_png(&rgba, 2, 1).expect("encode")).expect("decode");
        assert_eq!(kept.color(), image::ColorType::Rgba8);
        assert_eq!(kept.to_rgba8().as_raw(), &rgba);

        // A buffer that is not width*height*4 is an error, not a panic.
        assert!(encode_png_rgb(&rgba, 3, 1).is_err());
    }
}
