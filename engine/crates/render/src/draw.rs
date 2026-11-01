//! The draw list a caller hands `Renderer::render_frame`: [`DrawCmd`] (one
//! static draw, instanced where it can be), the pass it belongs to
//! ([`DrawLayer`]) and [`SkinnedDrawCmd`] (one skinned draw with its joint
//! palette). The ORDER the frame draws them in (`draw_key`,
//! `sort_far_to_near`) and its tests live with the frame, in `frame.rs`.

use crate::{LitSphere, MeshHandle, SkinnedMeshHandle, TextureHandle};
use glam::Mat4;

// Only for `DrawLayer::Water`'s docs, which link `Lighting::water_fog_begin`.
#[cfg(doc)]
use crate::Lighting;

/// Which pass a draw belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawLayer {
    /// Depth-tested and depth-writing, lit, NO alpha test. The default.
    #[default]
    Opaque,
    /// As `Opaque` — same sort, same instancing, depth written — plus the
    /// shader's MASK test (`MASK_CUTOFF`, the disc's alpha ≥ 193). For the
    /// disc's cutout materials. A draw's mode comes from its glTF `alphaMode`
    /// (`bake::alpha`), never from its texture: one texture can serve draws of
    /// all three modes. World draws only — a skinned draw is never tested
    /// ([`SkinnedDrawCmd::layer`]).
    Mask,
    /// Alpha-composited after every opaque, masked and water draw — the
    /// skinned opaque draws included — sorted back-to-front together with
    /// the skinned `Blend` draws, no depth write: `render_frame` has ONE
    /// blend pass (`blend_order`, queue SKO).
    Blend,
    /// A map's WATER surface: composited like `Blend` but with the depth
    /// WRITTEN, drawn in the opaque sort after every `Opaque`, `Mask`,
    /// `TerrainLayer`, `FarSheet` and `Spec` run, after the skinned opaque
    /// draws too (a wading soldier shows through it, tinted like the bed:
    /// the PS2 kicks its opaque model bucket before the water's,
    /// `analysis/re-water.md` §9.1 `[O]`, card WADE) and before any `Blend`
    /// draw, world or skinned.
    ///
    /// That is the PS2's own state for the near main layer — ALPHA 0x44
    /// (`Cs·As + Cd·(1−As)`) with TEST 0x50000, "depth-tested AND
    /// depth-written" (`analysis/re-water.md` §1 [V]) — and it is why the
    /// water cannot ride in `Blend`, which drops the depth write. It also
    /// takes its own fog gate ([`Lighting::water_fog_begin`]) and, on an
    /// ocean map, TWO coincident draws (base + foam), which is why its
    /// pipeline's depth test is `LessEqual` — see `Renderer::water_pipeline`.
    Water,
    /// The SPECULAR second pass (queue SPEC, `analysis/re-specular-pass.md`,
    /// `[O]`): the same mesh as its base draw, drawn again ADDITIVELY after
    /// every opaque and masked run and before the water, depth-tested
    /// `LessEqual` with no depth write. `texture` is the BASE texture (its
    /// alpha weights the add), `color` is `[r, g, b, power / 255]` — the flat
    /// pass colour and the table's `s` ([`DrawCmd::specular`]). World and
    /// skinned draws (`SkinnedDrawCmd::specular`, queue SPEC2); skipped while
    /// the renderer holds no table
    /// (`Renderer::set_specular_table`).
    Spec,
    /// The water's SPECKLE layer (queue W3, `analysis/re-water.md` §1/§3
    /// [V], `re-water-alpha.md` §7.1 `[O]`): the water surface drawn again
    /// ADDITIVELY — the PS2's record 0x69c3b0, ALPHA 0x48 (`Cs·As + Cd`),
    /// TEST 0x51001 (depth-tested, no depth write) — after every water run
    /// and before any `Blend` draw, depth test `LessEqual` so it lands on the
    /// water's own depth. `texture` is one speckle frame, sampled at the mesh
    /// UV × the texture's `uv_scale` and scrolled by its `scroll`; `color` is
    /// `[r, g, b, weight]`, and the sheet's own alpha is NOT read (the near
    /// speckle's TCC is cleared, §7.1). Unlit; world draws only.
    Speckle,
    /// The `LOWR` far-LOD terrain sheet (queue CLN5, `analysis/re-lowr.md`
    /// §2.2 [V]): depth-tested and depth-WRITING like `Opaque`, but UNLIT —
    /// the disc draws it at base colour 0x80808080, GS unity, with no normal
    /// — and alpha-tested at the sheet's own AREF 0x10 rather than
    /// `MASK_CUTOFF` (`fs_far_sheet`); fogged like any far-scene draw. Drawn
    /// after every `Mask` run, before the specular pass. World draws only,
    /// and never on a map with a [`crate::FarFade`] (the game draws no sheet
    /// there, so there is no `_fade` twin).
    FarSheet,
    /// A terrain texture LAYER above LTEX[0] (card TALPH,
    /// `analysis/re-terrain-layers.md` §2 [O]): the heightfield drawn again
    /// with the layer's texture, composited OVER what is there by the vertex
    /// alpha — the PS2's GS ALPHA 0x44, `(Cs − Cd)·As + Cd` — depth-tested
    /// `LessEqual` (the layer lies exactly on the base triangles) with no
    /// depth write. Drawn after every `Opaque` and `Mask` run and before
    /// the far sheet, in MESH-HANDLE order: the caller uploads the layers
    /// in ascending LTEX order, which is the order the PS2 composites them.
    /// World draws only; lit like the base ([`LightPath::Terrain`]).
    TerrainLayer,
}

/// How the world shader lights a static draw — the PS2 VU1 program its
/// segment takes and the light set its model uploads
/// (`analysis/re-model-lighting.md` §2/§3/§6, `[O]`; the bake picks it per
/// primitive, `bake::glb::light_path`). `tex` is the texel × the detail
/// factor, `c` the vertex colour (the disc's COL0, linearised; white where a
/// segment has none), `amb` the hemisphere ambient, `L` the sum of the
/// directional lights' `colour × max(0, N·L)` — the sun and `Light2` — and
/// `L'` that sum without the `Static()` ones:
///
/// | path | colour | VU1 |
/// |---|---|---|
/// | `Lit` | `tex × c × (amb + L)` | 0x1bb / 0x1e6, then 0x521 |
/// | `Prelit` | `tex × c × L'` | the prelit set: black ambient, Static skipped |
/// | `PrelitAdd` | `tex × (c + L')` | 0x537 (COL0 added) |
/// | `Unlit` | `tex × c` | 0x2a3 → 0x2da |
/// | `UnlitAdd` | `tex × (1 + c)` | 0x2a3 → 0x2bf, PROVISIONAL |
/// | `Terrain` | `min(tex × g, 1)` | 0x2a3 → 0x2da, the heightfield |
/// | `Character` | `tex × c × (amb' + L)` | `Lit` with BF1's character ambient |
/// | `Glow` | `min(1, D(tex × c) × (1 + As/128))` | BF2 ALPHA 8, colour 0xff404040 |
/// | `Unscaled` | `min(1, D(tex × c) × 2)` | BF2 0x14: the HDR 0.5 skipped |
///
/// `D` is display (gamma) units, where the GS multiplies. The two BF2 paths
/// exist only on a map whose `Effect("HDR")` is enabled (card GLOW,
/// `analysis/re-glow.md` §1, [O]; `bake::glb::glow_path`). For `Glow`,
/// `As = min(255, At·Av >> 7)` with `At = A·129 >> 8` from the texel's alpha
/// `A` and `Av` from the vertex colour's alpha (`bake::glb::
/// glow_vertex_alpha`): the texture's alpha is the glow mask, up to ×2.98.
/// It ignores the instance tint (the override replaces the draw colour), and
/// a `Mask` draw of it tests `As ≥ 0x60` — disc alpha ≥ 98 — in place of
/// `MASK_CUTOFF`.
///
/// `Terrain` is `Unlit` for the heightfield, whose vertex colour is NOT a
/// linearised COL0 but the raw GS byte (`bake::terrain_glb`, 0x80 = 1.0, up
/// to 248 on BF1): `g` = sRGB→linear(c × 255/128), which may exceed 1, and
/// the GS's 255 clamp is kept (`analysis/re-terrain-lighting.md` §0, [O]).
///
/// The sums are LINEAR where the GS adds in gamma units — the same deviation
/// the port's ambient + sun already makes. `UnlitAdd` is PROVISIONAL [D]: the
/// VU1 adds COL0 to the material constant, and that the constant is unity in
/// the port's terms is read off the lit path, not measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum LightPath {
    /// Every draw the bake gave no path: the port's lighting before queue
    /// LITEB, plus `Light2`.
    #[default]
    Lit = 0,
    Prelit = 1,
    PrelitAdd = 2,
    Unlit = 3,
    UnlitAdd = 4,
    Terrain = 5,
    /// `Lit`, with [`crate::Lighting::character_ambient`] as the ambient when
    /// it is set: BF1's light-set builder gives a non-static draw — a
    /// soldier and the weapon in his hand — `SkyInfo.CharacterAmbientColor`
    /// where a static draw takes `AmbientColor`
    /// (`analysis/re-model-lighting.md` §12, `[O]`). Exactly `Lit` while it
    /// is unset (every BF2 map, and a BF1 map that authors none).
    Character = 6,
    Glow = 7,
    Unscaled = 8,
    /// The PS2 BF2's INTERFACE light set (`0x7d6160`, card SSELB,
    /// `analysis/re-spawn-flow.md` §4.2): what a UI model is lit by - a
    /// flat ambient of 0.5 and one white light of 1.0 fixed to the camera
    /// ([`IFACE_AMBIENT`], [`IFACE_DIR_CAMERA`]), no map light. The draw's
    /// colour is the model's colour word, a factor on that light, not on
    /// the texel (`game::spawn_preview::colour_unit`).
    Iface = 9,
    /// PC geometry with baked vertex lighting; only dynamic local lights are added.
    VertexLit = 10,
}

/// The interface light set's ambient, GS units (`FUN_00398000` writes 0.5
/// to both hemisphere colours, B2:392826-392851).
pub const IFACE_AMBIENT: f32 = 0.5;

/// The interface light's travel direction in the CAMERA's frame (x right,
/// y up, z toward the viewer): `FUN_00398000` sets (-0.57735, -0.57735,
/// -0.57735) in the UI scene's frame (B2:392813-392839). That frame's axes
/// lie on the camera's as written here [M]: of the eight signs, this one
/// lit the side Snowtrooper of the PS2's Hoth frame closest (mean |lum|
/// error 10.3 over his pixels, the others 19-36; brighter on his left as
/// the PS2's, 62 / 71 against 57 / 66; `lead_tmp/sselb/tools/light_score.py`).
pub const IFACE_DIR_CAMERA: [f32; 3] = [-0.57735, -0.57735, -0.57735];

/// One entry in the draw list.
#[derive(Debug, Clone, Copy)]
pub struct DrawCmd {
    pub mesh: MeshHandle,
    /// `None` = flat color only (internally a 1x1 white texture).
    pub texture: Option<TextureHandle>,
    /// Flat tint multiplied over texture * vertex color; white = untinted.
    pub color: [f32; 4],
    pub model: Mat4,
    /// Which pass this draw belongs to.
    pub layer: DrawLayer,
    /// Metres from the eye past which this draw's fragments are DISCARDED, or
    /// `None` for a draw that is never culled by distance — the far-scene
    /// cull's mechanism. WHICH draw gets which distance is the caller's
    /// policy, not the renderer's: `game::viewer::far_cull` sets it from each
    /// map's `SkyInfo` ranges, and every other caller leaves it `None`.
    ///
    /// It is a per-fragment radial test against the same `view_dist` the far
    /// fade uses (the fog ramp takes planar depth), i.e. a spherical clip
    /// around the eye rather than the PS2's planar frustum far plane — stated
    /// in `game::viewer::far_cull`. Sky shells never carry one: their
    /// instances are built with `NO_CULL`.
    pub cull: Option<f32>,
    /// Metres from the eye NEARER than which this draw's fragments are
    /// DISCARDED, or `None` for a draw with no near edge — which is every
    /// draw but the `LOWR` far-LOD sheet and a geometry's LOWD extension
    /// (`game::viewer::scene`, card UTAS: cut at `NearSceneRange[1]`, the
    /// far scene's reach).
    ///
    /// The PS2's far scene is a SEPARATE buffer the near scene composites
    /// over, so its sheet may overlap the heightfield freely. The port has
    /// one depth buffer, so the overlap has to be cut instead:
    /// `game::viewer::lowres_range` sets this to where the heightfield ends.
    /// Same per-fragment radial test as [`DrawCmd::cull`], and the same
    /// deviation from the ELF's planar frustum.
    pub cull_near: Option<f32>,
    /// The far scene's model / LOWD switch (card LOD), or `None` — every draw
    /// but a LOWD-bearing world model and its LOWD on a map with a far scene
    /// ([`LodSwitch`]).
    pub lod: Option<LodSwitch>,
    /// How the shader lights this draw ([`LightPath`]); `Lit` unless the
    /// caller asks for another.
    pub light: LightPath,
    /// A per-instance distance fade ([`Fade`]), or `None` — every draw but
    /// the terrain foliage's.
    pub fade: Option<Fade>,
    /// The model sphere this draw gathers BF2's local lights for (card BPNT,
    /// `crate::point_light`), or `None` for a draw that takes none — every
    /// draw but a placed world model's. `Lit` and `Character` take all local
    /// lights; `Prelit`, `PrelitAdd` and PC `VertexLit` skip static lights.
    pub lit: Option<LitSphere>,
}

/// Which side of the far scene's switch a draw is ([`LodSwitch`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodSide {
    /// The geometry's own model: drawn nearer than the switch.
    Model,
    /// Its `LOWD`: drawn past the switch.
    Lowd,
}

/// The far scene's model / LOWD switch (card LOD, `analysis/re-lod.md` §4,
/// §7 [O]): a geometry that has a `LOWD` is two draws, its model and its
/// LOWD, and the WHOLE instance switches at once: the model while the
/// nearest point of its bounding sphere (`anchor`, `radius`) is nearer the
/// eye than `at`, the LOWD once all of it is past — [`LodSwitch::drawn`],
/// decided per draw by the frame, so the two never overlap and never mix. The
/// nearest point, not the centre: many LOWD-bearing geometries are whole map
/// sections (BF2 MUS1's `mus1_bldg_world_1`, r 260 m; 37 on UTA1), whose
/// centre is far while the part beside the player is near. The game picks
/// `at` (`game::viewer::look::LodBands`).
///
/// Removed deviation (2026-10-27): the switch was a per-pixel ordered-dither
/// band over `NearSceneRange[1]..[2]`, standing for the PS2's depth-ramp
/// composite of its near and far buffers. Where a model and its LOWD differ
/// (BF2 MUS1's lava wall) the band showed both, interleaved, as a grid of
/// dots; the PS2's blend shows neither.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodSwitch {
    pub at: f32,
    pub anchor: glam::Vec3,
    pub radius: f32,
    pub side: LodSide,
}

impl LodSwitch {
    /// Is this side of the switch drawn from `eye`?
    pub fn drawn(&self, eye: glam::Vec3) -> bool {
        let near = self.anchor.distance(eye) - self.radius < self.at;
        match self.side {
            LodSide::Model => near,
            LodSide::Lowd => !near,
        }
    }
}

/// A draw's cull bands as the shader reads them, `(cull, cull_near)`, each
/// `[lo, hi]` metres (`mesh::Instance`): a band of width 0 at the draw's own
/// edges. A [`LodSwitch`] cuts no fragment; the frame drops the side not
/// drawn.
pub(crate) fn instance_cull(d: &DrawCmd) -> ([f32; 2], [f32; 2]) {
    let far = d.cull.unwrap_or(crate::mesh::NO_CULL);
    let near = d.cull_near.unwrap_or(crate::mesh::NO_CULL_NEAR);
    ([far, far], [near, near])
}

/// The shader's `far_cull` distance test (`shader.wgsl`), for the tests: is
/// a fragment `dist` metres from the eye, at a pixel whose dither value is
/// `u` (0..1), kept by these bands?
pub fn fragment_kept(cull: [f32; 2], cull_near: [f32; 2], dist: f32, u: f32) -> bool {
    let lerp = |b: [f32; 2]| b[0] + (b[1] - b[0]) * u;
    !(dist > lerp(cull) || dist < lerp(cull_near))
}

/// Whether `far_cull` discards EVERY fragment of a draw whose fragments all
/// lie `near..=far` metres from the eye, at any dither value (card LODP):
/// the frame drops such a draw before it is submitted. A fragment's
/// `view_dist` is interpolated between its vertices' distances, so it stays
/// inside the range of its world box's ([`crate::frustum::distance_range`]).
/// It drops every draw wholly past its cull (card LODP), so a far model is
/// not rasterised only to be discarded.
pub fn all_discarded(cull: [f32; 2], cull_near: [f32; 2], near: f32, far: f32) -> bool {
    near > cull[0].max(cull[1]) || far < cull_near[0].min(cull_near[1])
}

/// The terrain foliage's per-instance fade (analysis/re-foliage.md §6, [O]):
/// with `d` the HORIZONTAL (x, z) distance from the draw's origin (its model
/// matrix's translation) to the eye, the instance's alpha is 1 up to `begin`,
/// falls linearly to 0 at `end`, and — when `near` > 0 — is
/// `max(d / near, 0.1)²` nearer than `near`. The PS2 blends that alpha; the
/// port draws it as an ORDERED DITHER in the draw's own pass (a 4×4 Bayer
/// screen-door, `far_cull` in the shader), which keeps the opaque sort and
/// the depth write — a stated deviation, PROVISIONAL.
///
/// `opacity` multiplies that alpha: 1 for the foliage; the chase camera's
/// see-through props (`game::camera_collide`, ours) take a constant one
/// through [`Fade::see_through`], 0 hiding the draw outright.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub begin: f32,
    pub end: f32,
    pub near: f32,
    pub opacity: f32,
}

impl Fade {
    /// A constant `opacity` at every distance (no distance fade).
    pub fn see_through(opacity: f32) -> Fade {
        Fade {
            begin: crate::mesh::NO_CULL,
            end: crate::mesh::NO_CULL,
            near: 0.0,
            opacity: opacity.clamp(0.0, 1.0),
        }
    }
}

impl DrawCmd {
    pub fn textured(mesh: MeshHandle, texture: TextureHandle, model: Mat4) -> Self {
        Self {
            texture: Some(texture),
            ..Self::flat(mesh, [1.0; 4], model)
        }
    }

    /// Light this draw by `light` ([`DrawCmd::light`]).
    pub fn with_light(self, light: LightPath) -> Self {
        Self { light, ..self }
    }

    /// Gather the local lights for this model sphere ([`DrawCmd::lit`]).
    pub fn with_lit(self, lit: Option<LitSphere>) -> Self {
        Self { lit, ..self }
    }

    /// Fade this draw by its horizontal distance ([`DrawCmd::fade`]).
    pub fn with_fade(self, fade: Option<Fade>) -> Self {
        Self { fade, ..self }
    }

    /// Cull this draw's fragments past `metres` from the eye. See
    /// [`DrawCmd::cull`].
    pub fn with_cull(self, metres: Option<f32>) -> Self {
        Self {
            cull: metres,
            ..self
        }
    }

    /// Cull this draw's fragments NEARER than `metres` from the eye. See
    /// [`DrawCmd::cull_near`].
    pub fn with_cull_near(self, metres: Option<f32>) -> Self {
        Self {
            cull_near: metres,
            ..self
        }
    }

    /// Put this draw on one side of the far scene's model / LOWD switch. See
    /// [`DrawCmd::lod`].
    pub fn with_lod(self, lod: Option<LodSwitch>) -> Self {
        Self { lod, ..self }
    }

    /// As [`DrawCmd::textured`], alpha-tested by the mask pipeline (`MASK_CUTOFF`).
    pub fn textured_masked(mesh: MeshHandle, texture: TextureHandle, model: Mat4) -> Self {
        Self {
            layer: DrawLayer::Mask,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// As [`DrawCmd::textured`], on the WATER pass ([`DrawLayer::Water`]),
    /// tinted by `color` — the PS2's per-vertex diffuse LUT stands in as one
    /// flat colour, and its alpha IS the blend weight (`game::viewer`).
    pub fn water(mesh: MeshHandle, texture: TextureHandle, color: [f32; 4], model: Mat4) -> Self {
        Self {
            color,
            layer: DrawLayer::Water,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// As [`DrawCmd::textured`], composited by the blend pass.
    pub fn textured_blended(mesh: MeshHandle, texture: TextureHandle, model: Mat4) -> Self {
        Self {
            layer: DrawLayer::Blend,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// The specular second pass over a base draw of `mesh` with `texture`
    /// ([`DrawLayer::Spec`]): `rgb` is the flat pass colour (1.0 = the table
    /// texel as is), `power` the MTRL's specular power.
    pub fn specular(
        mesh: MeshHandle,
        texture: TextureHandle,
        rgb: [f32; 3],
        power: u32,
        model: Mat4,
    ) -> Self {
        Self {
            color: [rgb[0], rgb[1], rgb[2], power as f32 / 255.0],
            layer: DrawLayer::Spec,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// One frame of the water's speckle layer ([`DrawLayer::Speckle`]): adds
    /// `texel.rgb × rgb × weight`, `color = [r, g, b, weight]`. The PS2's
    /// near pass cross-fades two frames in one draw; the port draws both
    /// frames, weighted `1 − w` and `w`, which is the same sum.
    pub fn speckle(mesh: MeshHandle, texture: TextureHandle, color: [f32; 4], model: Mat4) -> Self {
        Self {
            color,
            layer: DrawLayer::Speckle,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// As [`DrawCmd::textured`], on the terrain-layer pass
    /// ([`DrawLayer::TerrainLayer`]), lit as the terrain.
    pub fn terrain_layer(mesh: MeshHandle, texture: TextureHandle, model: Mat4) -> Self {
        Self {
            layer: DrawLayer::TerrainLayer,
            light: LightPath::Terrain,
            ..Self::textured(mesh, texture, model)
        }
    }

    /// As [`DrawCmd::textured`], on the far-sheet pass
    /// ([`DrawLayer::FarSheet`]).
    pub fn far_sheet(mesh: MeshHandle, texture: TextureHandle, model: Mat4) -> Self {
        Self {
            layer: DrawLayer::FarSheet,
            ..Self::textured(mesh, texture, model)
        }
    }

    pub fn flat(mesh: MeshHandle, color: [f32; 4], model: Mat4) -> Self {
        Self {
            mesh,
            texture: None,
            color,
            model,
            layer: DrawLayer::Opaque,
            cull: None,
            cull_near: None,
            lod: None,
            light: LightPath::Lit,
            fade: None,
            lit: None,
        }
    }
}

/// A skinned draw: like `DrawCmd`, plus the joint palette — model-space
/// joint matrices (`joint_global * inverse_bind`, glam column-major), indexed
/// by the mesh's per-vertex joints. At most `MAX_JOINTS`; unused entries are
/// zero and must carry zero weight. One draw call each (no instancing).
#[derive(Debug, Clone)]
pub struct SkinnedDrawCmd {
    pub mesh: SkinnedMeshHandle,
    pub texture: Option<TextureHandle>,
    pub color: [f32; 4],
    pub model: Mat4,
    pub palette: Vec<Mat4>,
    /// [`DrawLayer::Opaque`], [`DrawLayer::Blend`] or — a segment's second
    /// pass, [`SkinnedDrawCmd::specular`] — [`DrawLayer::Spec`]; the skinned
    /// path has no other pass, and `render_frame` refuses any other layer.
    ///
    /// There is no skinned alpha TEST because the disc runs none: it draws a
    /// side model's segment with the GS state its own `MTRL` word asks for,
    /// and no segment of any soldier on either disc carries the cutout bit
    /// (`analysis/re-side-alpha.md` §0–§1, `[O]`). `Opaque` draws the sheet's
    /// alpha IGNORED (`TEST 0x50000`); `Blend` is the disc's 0x04 segment —
    /// `ALPHA 0x44` by the sheet's alpha, Z-tested with no Z write — drawn
    /// in the world's blend pass, far to near among its draws, after every
    /// opaque draw of either kind. The layer is the GLB material's
    /// `alphaMode` (`bake::soldier::segment_blends` writes it).
    pub layer: DrawLayer,
    /// The sphere this draw gathers BF2's local lights for
    /// ([`DrawCmd::lit`]); a skinned draw is never prelit.
    pub lit: Option<LitSphere>,
    /// [`LightPath::Character`] (a soldier in the world) or
    /// [`LightPath::Iface`] (a spawn screen's soldier); no other path.
    pub light: LightPath,
}

impl SkinnedDrawCmd {
    /// The specular second pass over this draw (queue SPEC2): the same mesh,
    /// sheet, model and palette on [`DrawLayer::Spec`], with the pass's flat
    /// colour and power where [`DrawCmd::specular`] puts them. The pass
    /// belongs to the ONE per-segment draw every `RedModel` takes, skinned
    /// included (`analysis/re-specular-pass.md` §1, `analysis/re-side-alpha.md`
    /// §3, `[O]`), so R comes from the SKINNED normal. The base's instance
    /// colour is not carried: `fs_spec` recomputes the base as if it were
    /// white, which every base the port submits is but a debug tint's.
    pub fn specular(&self, rgb: [f32; 3], power: u32) -> Self {
        Self {
            color: [rgb[0], rgb[1], rgb[2], power as f32 / 255.0],
            layer: DrawLayer::Spec,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod lod_tests {
    use super::*;
    use crate::mesh::{NO_CULL, NO_CULL_NEAR};
    use glam::Vec3;

    fn cmd(cull: Option<f32>, lod: Option<LodSwitch>) -> DrawCmd {
        DrawCmd::flat(MeshHandle::default(), [1.0; 4], Mat4::IDENTITY)
            .with_cull(cull)
            .with_lod(lod)
    }

    fn switch(side: LodSide) -> LodSwitch {
        LodSwitch {
            at: 130.0,
            anchor: Vec3::new(0.0, 0.0, -50.0),
            radius: 20.0,
            side,
        }
    }

    #[test]
    fn a_plain_draw_keeps_its_one_edge() {
        assert_eq!(
            instance_cull(&cmd(None, None)),
            ([NO_CULL; 2], [NO_CULL_NEAR; 2])
        );
        assert_eq!(
            instance_cull(&cmd(Some(500.0), None)),
            ([500.0; 2], [NO_CULL_NEAR; 2])
        );
    }

    /// The switch cuts no fragment: both sides keep only their own edges.
    #[test]
    fn a_switch_adds_no_band() {
        for side in [LodSide::Model, LodSide::Lowd] {
            assert_eq!(
                instance_cull(&cmd(Some(500.0), Some(switch(side)))),
                instance_cull(&cmd(Some(500.0), None))
            );
        }
    }

    /// The whole instance switches by its sphere's nearest point: exactly
    /// one side drawn at every eye position, the model while any of the
    /// sphere is nearer than `at`, the LOWD once all of it is past.
    #[test]
    fn exactly_one_side_by_the_anchors_distance() {
        let (m, l) = (switch(LodSide::Model), switch(LodSide::Lowd));
        for d in (0..400).map(|k| k as f32 * 0.5) {
            let eye = m.anchor + Vec3::new(d, 0.0, 0.0);
            assert_ne!(m.drawn(eye), l.drawn(eye), "one side at {d} m");
            assert_eq!(m.drawn(eye), d < 150.0, "the model to {d} m");
        }
    }

    /// Card LODP: a draw is dropped only when no fragment of its distance
    /// range survives.
    #[test]
    fn a_draw_is_dropped_only_when_every_fragment_is_discarded() {
        let plain = instance_cull(&cmd(None, None));
        let culled = instance_cull(&cmd(Some(500.0), None));
        let dithers = || (0..16).map(|v| (v as f32 + 0.5) / 16.0);
        for (c, n) in [plain, culled] {
            for near in (0..60).map(|m| m as f32 * 20.0) {
                for len in [0.0, 1.0, 10.0, 29.0, 31.0, 200.0, 2000.0] {
                    let far = near + len;
                    let kept = (0..=200)
                        .map(|k| near + len * k as f32 / 200.0)
                        .any(|d| dithers().any(|u| fragment_kept(c, n, d, u)));
                    if all_discarded(c, n, near, far) {
                        assert!(!kept, "dropped {near}..{far} keeps a fragment");
                    }
                }
            }
        }
        let (c, n) = culled;
        assert!(all_discarded(c, n, 501.0, 600.0), "wholly past its cull");
        let (c, n) = plain;
        assert!(!all_discarded(c, n, 0.0, 0.0) && !all_discarded(c, n, 1e6, 2e6));
    }
}
