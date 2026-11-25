//! BF2's terrain light pass (card FXBL, `analysis/re-bolt.md` §5.3, [O]):
//! every omni light near the ground adds a soft disc of its colour to the
//! heightfield — the pink wash round every red bolt on HOT1's snow.
//!
//! What the PS2 does (B2 `FUN_0056f208`'s lit branch, 705214–705340): per
//! terrain patch, one pass per light of the patch's set (≤ 3), drawn on the
//! patch's top layer with `com_sfx_omniproj` (64×64 grey, a cos² blob)
//! projected straight down from 5 m above the light —
//! s = ½ + ½(L.z − P.z)/z′, t = ½ + ½(P.x − L.x)/z′, z′ = L.y + 5 − P.y,
//! CLAMP — so the lit area is a disc of radius ≈ 0.98·(5 + dy) m whatever the
//! light's range. Vertex colour = the light's RGB × f (f = max(0, 1 −
//! dy²/r²), dy = the light's height over the heightfield), vertex alpha 0x20;
//! GS MODULATE, ALPHA 0x148 (Cs·As + Cd), depth tested, no depth write, no
//! fog. On screen that adds T·c·f/4 per channel (BF2's HDR halving is undone
//! by its composite; the port keeps the scene full-scale, so it adds the screen value).
//!
//! The caller does the gather ([`TerrainLights`]: each light's colour
//! already × f, and per PS2 patch the ≤ 3 lights it takes); the shader
//! finds a fragment's patch among [`TerrainLights::patches`] and sums them.
//!
//! The GS adds in its gamma bytes; the target here is linear sRGB, and the
//! destination is a layered terrain the pass cannot recompute (as `fs_spec`
//! does its base). So the frame is COPIED once, after the terrain's layers
//! and before the far sheet, water and blend passes, and `fs_terrain_light`
//! reads the copy under it, adds in gamma and writes the linear difference
//! through the additive blend. A window whose surface cannot be copied
//! draws no pass (said once on stderr).

use crate::bindings;
use crate::texture::upload_levels;
use crate::{RenderError, Renderer};
use glam::Vec3;

/// The most lights and lit patches one frame hands the shader; more are
/// dropped (nearest first is the caller's order). The two caps are the
/// port's own (the size of the shader's table), not the PS2's.
pub const MAX_TERRAIN_LIGHTS: usize = 32;
pub const MAX_LIT_PATCHES: usize = 128;
/// The terrain draws the first 3 of a patch's top-4 gather (§5.3).
pub const LIGHTS_PER_PATCH: usize = 3;
// Consecutive groups preserve the shader's first-patch tie break. Their union
// bounds reject large unlit regions without scanning all 128 patch rectangles.
const PATCH_GROUP_SIZE: usize = 8;
const PATCH_GROUPS: usize = MAX_LIT_PATCHES.div_ceil(PATCH_GROUP_SIZE);

/// One light as the pass draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainLight {
    pub pos: Vec3,
    /// The light's colour × f, 1.0 = byte 255.
    pub rgb: [f32; 3],
}

/// One PS2 terrain patch that takes lights: its XZ rectangle and up to
/// [`LIGHTS_PER_PATCH`] indices into [`TerrainLights::lights`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LitPatch {
    /// (min x, min z), (max x, max z).
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub lights: [u32; LIGHTS_PER_PATCH],
    pub count: u32,
}

/// This frame's terrain lights, gathered per patch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerrainLights {
    pub lights: Vec<TerrainLight>,
    pub patches: Vec<LitPatch>,
}

/// The shader's `TlParams`: counts, then the lights (two rows each), the
/// patch rectangles and their light indices — as u32 words.
fn params_words(t: &TerrainLights) -> Vec<u32> {
    let nl = t.lights.len().min(MAX_TERRAIN_LIGHTS);
    // A patch's lights: `count` is a pub field, so it is clamped here.
    let taken = |p: &LitPatch| (p.count as usize).min(LIGHTS_PER_PATCH);
    let ok = |p: &&LitPatch| p.lights[..taken(p)].iter().all(|&i| (i as usize) < nl);
    let np = t.patches.iter().take(MAX_LIT_PATCHES).filter(ok).count();
    let groups = 4 + 8 * MAX_TERRAIN_LIGHTS + 8 * MAX_LIT_PATCHES;
    let mut w = vec![0u32; groups + 4 * PATCH_GROUPS];
    w[0] = nl as u32;
    w[1] = np as u32;
    w[2] = 1; // grouped search; prepare_terrain_light can select the reference
    for (k, l) in t.lights.iter().take(nl).enumerate() {
        let o = 4 + 8 * k;
        let row = [
            l.pos.x, l.pos.y, l.pos.z, 0.0, l.rgb[0], l.rgb[1], l.rgb[2], 0.0,
        ];
        for (j, v) in row.into_iter().enumerate() {
            w[o + j] = v.to_bits();
        }
    }
    let rects = 4 + 8 * MAX_TERRAIN_LIGHTS;
    let sel = rects + 4 * MAX_LIT_PATCHES;
    let valid = t.patches.iter().take(MAX_LIT_PATCHES).filter(ok);
    for (k, p) in valid.enumerate() {
        for (j, v) in [p.min[0], p.min[1], p.max[0], p.max[1]]
            .into_iter()
            .enumerate()
        {
            w[rects + 4 * k + j] = v.to_bits();
        }
        let n = taken(p);
        w[sel + 4 * k..sel + 4 * k + n].copy_from_slice(&p.lights[..n]);
        w[sel + 4 * k + 3] = n as u32;
    }
    for start in (0..np).step_by(PATCH_GROUP_SIZE) {
        let mut bound = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for k in start..(start + PATCH_GROUP_SIZE).min(np) {
            for (j, edge) in bound.iter_mut().enumerate() {
                let v = f32::from_bits(w[rects + 4 * k + j]);
                *edge = if j < 2 { edge.min(v) } else { edge.max(v) };
            }
        }
        for (j, v) in bound.into_iter().enumerate() {
            w[groups + 4 * (start / PATCH_GROUP_SIZE) + j] = v.to_bits();
        }
    }
    w
}

/// One cached per-resolution frame copy and its bind group.
pub(crate) struct TerrainLightDest {
    tex: wgpu::Texture,
    bg: wgpu::BindGroup,
    w: u32,
    h: u32,
}

/// The pass's per-renderer state.
#[derive(Default)]
pub(crate) struct TerrainLightState {
    pub frame: TerrainLights,
    /// `com_sfx_omniproj`: its view and CLAMP bilinear sampler.
    pub omniproj: Option<(wgpu::TextureView, wgpu::Sampler)>,
    pub params_buf: Option<wgpu::Buffer>,
    /// Cached per-resolution frame copies and their bind groups.
    pub dests: Vec<TerrainLightDest>,
    /// Can the target be copied (headless always; a window if its surface
    /// allows COPY_SRC)?
    pub copyable: bool,
    pub warned: bool,
    pub linear_search: bool,
}

impl Renderer {
    /// Install BF2's terrain light texture (`bake omniproj`: 64×64 RGBA8
    /// grey, one level), or remove it with `None` — then no light pass is
    /// drawn. Uploaded LINEAR: its bytes scale the light's colour as the
    /// GS's MODULATE does (a texel of 255 is ×255/128·the vertex colour
    /// byte's 1.0 = 0x80, i.e. the byte as is).
    pub fn set_omniproj(&mut self, tex: Option<(u32, u32, &[u8])>) -> Result<(), RenderError> {
        self.terrain_lights.dests.clear();
        let Some((w, h, rgba)) = tex else {
            self.terrain_lights.omniproj = None;
            return Ok(());
        };
        let view = upload_levels(
            &self.device,
            &self.queue,
            w,
            h,
            &[rgba],
            false,
            Some("com_sfx_omniproj"),
        )?;
        let sampler = crate::made::sampler(
            &self.device,
            &wgpu::SamplerDescriptor {
                label: Some("com_sfx_omniproj"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            },
        );
        self.terrain_lights.omniproj = Some((view, sampler));
        Ok(())
    }

    /// This frame's terrain lights (card FXBL); empty = no pass.
    pub fn set_terrain_lights(&mut self, lights: TerrainLights) {
        self.terrain_lights.frame = lights;
    }

    /// Diagnostic reference for measuring the patch prefilter and checking
    /// exact image equality. Both paths select the first matching rectangle.
    pub fn set_terrain_light_grouping(&mut self, enabled: bool) {
        self.terrain_lights.linear_search = !enabled;
    }

    /// Whether this frame draws the pass: lights on some patch, the texture
    /// installed, a target it can copy.
    pub(crate) fn terrain_light_wanted(&mut self) -> bool {
        let t = &mut self.terrain_lights;
        let want = !t.frame.patches.is_empty() && t.omniproj.is_some();
        if want && !t.copyable {
            if !t.warned {
                eprintln!("terrain lights: the window surface cannot be copied; no light pass");
                t.warned = true;
            }
            return false;
        }
        want
    }

    /// Upload the frame's parameters and (re)build the destination copy for
    /// a `w`×`h` target and the bind group over it.
    pub(crate) fn prepare_terrain_light(&mut self, w: u32, h: u32) {
        let mut words = params_words(&self.terrain_lights.frame);
        words[2] = u32::from(!self.terrain_lights.linear_search);
        let bytes: &[u8] = bytemuck::cast_slice(&words);
        let t = &mut self.terrain_lights;
        if t.params_buf.is_none() {
            t.params_buf = Some(crate::resources::buffer(
                &self.device,
                "terrain light params",
                bytes.len() as u64,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ));
        }
        let buf = t.params_buf.as_ref().expect("made above");
        self.queue.write_buffer(buf, 0, bytes);
        if crate::resources::touch_cached(&mut t.dests, |d| (d.w, d.h) == (w, h)) {
            return;
        }
        let Some((ov, os)) = &t.omniproj else {
            return;
        };
        let tex = crate::resources::texture_2d(
            &self.device,
            "terrain light destination",
            [w, h],
            self.format,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = crate::made::bind_group(
            &self.device,
            &wgpu::BindGroupDescriptor {
                label: Some("terrain light bg"),
                layout: &self.terrain_light_bgl,
                entries: &[
                    bindings::texture_view(0, ov),
                    bindings::sampler(1, os),
                    bindings::texture_view(2, &view),
                    bindings::entire_buffer(3, buf),
                ],
            },
        );
        crate::resources::push_cached(&mut t.dests, TerrainLightDest { tex, bg, w, h });
    }

    /// Copy `src` (the frame so far) and draw the pass over the base
    /// terrain's `runs` (mesh index, instance range) into `view`.
    pub(crate) fn terrain_light_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::Texture,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        runs: &[(usize, std::ops::Range<u32>)],
    ) -> u32 {
        let t = &self.terrain_lights;
        let Some(d) = t.dests.last() else {
            return 0;
        };
        encoder.copy_texture_to_texture(
            src.as_image_copy(),
            d.tex.as_image_copy(),
            wgpu::Extent3d {
                width: d.w,
                height: d.h,
                depth_or_array_layers: 1,
            },
        );
        let mut pass =
            crate::frame::continue_pass(encoder, view, depth_view, "terrain light pass");
        pass.set_pipeline(&self.terrain_light_pipeline);
        pass.set_bind_group(0, &self.globals_bg, &[]);
        pass.set_bind_group(1, &d.bg, &[]);
        pass.set_vertex_buffer(1, self.instance_buf.slice(..));
        let mut calls = 0;
        for (mesh, inst) in runs {
            crate::frame::draw_mesh(&mut pass, &self.meshes[*mesh], inst.clone());
            calls += 1;
        }
        calls
    }
}

/// Group 1 of the light pass: the texture, its sampler, the destination
/// copy (read with `textureLoad`) and `TlParams`.
pub(crate) fn terrain_light_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terrain light bgl"),
        entries: &[
            bindings::texture_layout(0, true),
            bindings::sampler_layout(1),
            bindings::texture_layout(2, false),
            bindings::uniform_layout(3, wgpu::ShaderStages::FRAGMENT, false, None),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words the shader reads: counts, a light's two rows, a patch's
    /// rectangle and its indices with the count in the fourth slot; a patch
    /// naming a light past the list is dropped.
    #[test]
    fn params_layout() {
        let t = TerrainLights {
            lights: vec![TerrainLight {
                pos: Vec3::new(1.0, 2.0, 3.0),
                rgb: [0.5, 0.25, 0.125],
            }],
            patches: vec![
                LitPatch {
                    min: [0.0, -16.0],
                    max: [16.0, 0.0],
                    lights: [0, 0, 0],
                    count: 1,
                },
                LitPatch {
                    min: [16.0, -16.0],
                    max: [32.0, 0.0],
                    lights: [0, 5, 0],
                    count: 2,
                },
            ],
        };
        let w = params_words(&t);
        assert_eq!(
            w.len(),
            4 + 8 * MAX_TERRAIN_LIGHTS + 8 * MAX_LIT_PATCHES + 4 * PATCH_GROUPS
        );
        assert_eq!(&w[..2], &[1, 1]);
        let f = |i: usize| f32::from_bits(w[i]);
        assert_eq!(
            [f(4), f(5), f(6), f(8), f(9), f(10)],
            [1.0, 2.0, 3.0, 0.5, 0.25, 0.125]
        );
        let r = 4 + 8 * MAX_TERRAIN_LIGHTS;
        assert_eq!(
            [f(r), f(r + 1), f(r + 2), f(r + 3)],
            [0.0, -16.0, 16.0, 0.0]
        );
        let s = r + 4 * MAX_LIT_PATCHES;
        assert_eq!(&w[s..s + 4], &[0, 0, 0, 1]);
        assert_eq!(
            &w[s + 4..s + 8],
            &[0, 0, 0, 0],
            "the bad patch is not written"
        );
    }

    /// A patch whose `count` runs past its three slots is read as three.
    #[test]
    fn a_count_past_the_slots_is_clamped() {
        let light = TerrainLight {
            pos: Vec3::ZERO,
            rgb: [1.0; 3],
        };
        let t = TerrainLights {
            lights: vec![light; 3],
            patches: vec![LitPatch {
                min: [0.0, 0.0],
                max: [16.0, 16.0],
                lights: [2, 1, 0],
                count: 7,
            }],
        };
        let w = params_words(&t);
        assert_eq!(&w[..2], &[3, 1]);
        let s = 4 + 8 * MAX_TERRAIN_LIGHTS + 4 * MAX_LIT_PATCHES;
        assert_eq!(&w[s..s + 4], &[2, 1, 0, 3]);
    }

    /// The grouped search against the plain one over the words the shader
    /// reads, at every rectangle's min, just below its max, max and centre,
    /// plus `extra` points. Returns the points that hit some rectangle.
    fn grouped_matches_linear(t: &TerrainLights, extra: &[f32]) -> usize {
        let w = params_words(t);
        let rects = 4 + 8 * MAX_TERRAIN_LIGHTS;
        let groups = rects + 8 * MAX_LIT_PATCHES;
        let rect =
            |offset: usize| std::array::from_fn::<_, 4, _>(|j| f32::from_bits(w[offset + j]));
        let inside = |offset: usize, x: f32, z: f32| {
            let r = rect(offset);
            x >= r[0] && z >= r[1] && x < r[2] && z < r[3]
        };
        let n = w[1] as usize;
        let mut xs = extra.to_vec();
        let mut zs = extra.to_vec();
        for i in 0..n {
            let r = rect(rects + 4 * i);
            for (axis, lo, hi) in [(&mut xs, r[0], r[2]), (&mut zs, r[1], r[3])] {
                axis.extend([lo, hi.next_down(), hi, lo + 0.5 * (hi - lo)]);
            }
        }
        let mut hits = 0;
        for &x in &xs {
            for &z in &zs {
                let plain = (0..n).find(|&i| inside(rects + 4 * i, x, z));
                let grouped = (0..n).find(|&i| {
                    inside(groups + 4 * (i / PATCH_GROUP_SIZE), x, z) && inside(rects + 4 * i, x, z)
                });
                assert_eq!(grouped, plain, "{x},{z}");
                hits += usize::from(plain.is_some());
            }
        }
        hits
    }

    fn patch(min: [f32; 2], max: [f32; 2]) -> LitPatch {
        LitPatch {
            min,
            max,
            lights: [0; 3],
            count: 1,
        }
    }

    fn one_light(patches: Vec<LitPatch>) -> TerrainLights {
        TerrainLights {
            lights: vec![TerrainLight {
                pos: Vec3::ZERO,
                rgb: [1.0; 3],
            }],
            patches,
        }
    }

    #[test]
    fn group_bounds_keep_the_first_match_at_edges_and_overlaps() {
        let mut t = one_light(
            (0..MAX_LIT_PATCHES)
                .map(|i| {
                    let (x, z) = ((i % 13) as f32, (i / 13) as f32);
                    patch([x - 6.0, z - 4.0], [x - 4.0, z - 2.0])
                })
                .collect(),
        );
        t.patches[7].lights[0] = 99; // filtered before groups are made
        t.patches[16].min[0] = f32::NEG_INFINITY;
        t.patches[16].max[0] = f32::INFINITY;
        t.patches[22].min[0] = f32::NAN; // cannot match
        let extra = [-20.0, -6.0, -5.5, -1.0, 0.0, 0.5, 6.0, 20.0, f32::NAN];
        assert!(
            grouped_matches_linear(&t, &extra) > 20,
            "the fixture must exercise matches"
        );
    }

    /// Disjoint rectangles in a row: every rectangle, the last of each group
    /// included, is the only one at its own sample points.
    #[test]
    fn group_bounds_cover_every_rectangle_of_their_group() {
        for np in [MAX_LIT_PATCHES, 13, 8, 1] {
            let t = one_light(
                (0..np)
                    .map(|i| {
                        let x = 3.0 * i as f32 - 100.0;
                        patch([x, -1.0 - i as f32], [x + 2.0, 1.0 + 0.5 * i as f32])
                    })
                    .collect(),
            );
            assert!(grouped_matches_linear(&t, &[]) >= 4 * np, "{np} patches");
        }
    }
}
