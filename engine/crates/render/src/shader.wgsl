// Single v0 shader: textured lambert with the map's own sun (and BF2's
// Light2), hemisphere ambient and the PS2's `fogspread` fog (bake::lighting reads
// them out of each map's `sky_`/`lght`), modulated by vertex color (terrain
// lightmap; a world model's COL0) and per-instance flat color. Each static
// draw picks one of the PS2's VU1 lighting paths (`render::LightPath`,
// `shade_unfogged`).
// Three transparency modes, PIPELINE-selected from the draw's glTF alphaMode
// (ALPHA RULE v2 — `bake::alpha` writes it per primitive):
//   OPAQUE -> `fs_main`, no discard, depth written
//   MASK   -> `fs_mask`, alpha-test at MASK_CUTOFF (the PS2's: disc alpha >= 193)
//             then `fs_main`'s shading, depth written
//   BLEND  -> `fs_main` through the blend pipeline, composited, no depth write
// SKINNED draws take the OPAQUE and BLEND entry points through `vs_skinned`
// (and the specular pass through `vs_spec_skinned`) and never MASK: the disc alpha-tests no side model, and blends only a
// segment whose own MTRL word carries 0x04 (`analysis/re-side-alpha.md` §0,
// [O]; `render::SkinnedDrawCmd::layer`).
// The alpha mode is NOT a property of the texture any more: one texture can
// serve draws of all three modes.
// The SKY pass runs TWO modes off the RAW MTRL word instead
// (`render::SkyMode`, the ELF's own rule under the sky draw flag): ONE entry
// point, `fs_sky`, for both OPAQUE and BLEND. It has no alpha-test entry
// point -- under the sky flag the ELF composites every alpha-carrying
// material -- and no FOGGED entry point either: the same 0x20 flag excludes
// every sky draw from the fog pass in both games (re-fog.md 2.4).
//
// Structure adapted from an earlier wgpu project's mesh shader (camera uniform at group 0,
// vs/fs pair) — model matrix moved from a per-object uniform to instance
// attributes so the whole draw list is one buffer write.

struct Globals {
    view_proj: mat4x4<f32>,
    // guard_x / guard_y: the PS2's guard band per unit of this frame's clip x / y
    // (`render::affine::guard`, `affine_w`).
    camera_pos: vec3<f32>, guard_x: f32,
    // Direction the sunlight TRAVELS (sun -> scene), normalized.
    sun_dir: vec3<f32>, sun_d4: f32,
    sun_color: vec3<f32>, guard_y: f32,
    // Hemisphere ambient: sky lights up-facing surfaces, ground down-facing.
    // A map that authors one ambient colour sets both the same.
    ambient_sky: vec3<f32>, material_time: f32,
    ambient_ground: vec3<f32>, fog_on: f32,
    fog_color: vec3<f32>, fog_near: f32,
    // `time` is seconds, WRAPPED by the caller (see Renderer::time) so the
    // water scroll keeps its precision in a long session; model materials
    // scroll on `material_time` (unwrapped clock modulo 255 s).
    // fade_begin/fade_end: NearSceneRange's b and c on a map with NO far
    // layer (`render::FarFade`). fade_end <= fade_begin means "no fade".
    fog_far: f32, time: f32, fade_begin: f32, fade_end: f32,
    // water_fog_begin: metres past which a WATER draw takes the fog ramp;
    // 0 means "this map's water is never fogged" (`render::Lighting`).
    // water_lit: the weight of the sun's diffuse term on a WATER draw; 0 =
    // unlit (`render::Lighting::water_lit`).
    // water_dim_*: a lit water draw's loss with planar depth (`render::
    // Lighting::water_dim`); rate 0 = none.
    water_fog_begin: f32, water_lit: f32, water_dim_begin: f32, water_dim_rate: f32,
    // The second directional light (BF2 `GlobalLights.Light2`, travel
    // direction; zero colour on a map without one) and the two `Static()`
    // flags, 1.0 = set (`render::Lighting`).
    light2_dir: vec3<f32>, sun_static: f32,
    light2_color: vec3<f32>, light2_static: f32,
    // The flat water's colour constants (`render::WaterShading`, the PS2's
    // VU1 water overlay's constant block): `fs_water` and `fs_speckle`.
    // glow_gain: a factor on LIGHT_GLOW / LIGHT_UNSCALED's display gain, 1
    // but on a map `game::viewer::look` tunes (`render::Lighting::glow_gain`).
    water_eye_shift: vec3<f32>, glow_gain: f32,
    water_specular: vec4<f32>,
    speckle_ambient: vec4<f32>,
    speckle_specular: vec4<f32>,
    speckle_axis: vec3<f32>, water_gain: f32,
    speckle_light: vec3<f32>, speckle_far_begin: f32,
    speckle_shift: vec3<f32>, speckle_far_end: f32,
    water_grid_origin: vec2<f32>, water_grid_step: vec2<f32>,
    water_grid_dims: vec2<u32>, water_grid_range: vec2<f32>,
    water_grid_colors: array<vec4<u32>, 16>,
    water_fog_color: vec3<f32>, water_fog_on: f32,
    water_fog_range: vec2<f32>, _pad_water_fog: vec2<f32>,
    // BF1's character ambient, flat, for LIGHT_CHARACTER draws while
    // character_ambient_on is 1 (`render::Lighting::character_ambient`).
    character_ambient: vec3<f32>, character_ambient_on: f32,
    // `render::IFACE_DIR_CAMERA` in the world (travel direction), for
    // LIGHT_IFACE draws.
    iface_dir: vec3<f32>, _pad11: f32,
};

// The twins in `render::NoDiscard` set this false: a draw whose bands and fade
// cannot fire this frame (`render::draw::may_discard`) runs without `far_cull`'s
// discards, so the GPU can write its depth before shading.
override CULLS: bool = true;

@group(0) @binding(0) var<uniform> globals: Globals;
// BF2's local lights (`render::point_light`, card BPNT): the view's table.
// Record 0 is "no light"; an instance names up to four records.
struct PointRecord {
    // xyz = the light's world position, w = 1 / range^2.
    pos_inv_r2: vec4<f32>,
    // rgb in GS units (0x80 = 1.0): the VU's packed colour byte / 128.
    color: vec4<f32>,
};
struct PointTable {
    lights: array<PointRecord, 256>,
};
@group(0) @binding(1) var<uniform> points: PointTable;
@group(1) @binding(0) var t_color: texture_2d<f32>;
@group(1) @binding(1) var s_color: sampler;
@group(1) @binding(4) var t_color_display: texture_2d<f32>;
// PS2 detail map (`<name>_dtl`): signal in ALPHA, modulated x2 over the base
// (audit A2 §4). Materials without one bind the base view here and set
// detail_strength = 0.
@group(1) @binding(2) var t_detail: texture_2d<f32>;
struct MaterialParams {
    // x = detail UV scale (1.0 = same UVs), y = 0/1 detail on,
    detail: vec4<f32>,
    // xy = UV units per second. Zero on every texture but a map's water, so
    // `uv + 0.0 * time` is exactly `uv` and no other draw's output moves.
    // zw = the mesh-UV multiplier, read ONLY by `fs_speckle` (1 elsewhere).
    scroll: vec4<f32>,
};
@group(1) @binding(3) var<uniform> material: MaterialParams;

// Skinned draws: per-draw joint palette (model-space joint matrices, i.e.
// joint_global * inverse_bind), bound with a dynamic offset per draw.
struct Palette {
    joints: array<mat4x4<f32>, 96>,
};
@group(2) @binding(0) var<uniform> palette: Palette;

struct VsIn {
    // Vertex-rate (slot 0)
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>, // Unorm8x4 -> 0..1
    // Instance-rate (slot 1): model matrix columns + flat tint
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) inst_color: vec4<f32>,
    @location(9) region_top: vec4<f32>,
    @location(10) region_bottom: vec4<f32>,
    // Far-scene cull band [lo, hi], metres (render::mesh::NO_CULL = never).
    @location(11) cull: vec2<f32>,
    // Far scene's NEAR edge band, metres (render::mesh::NO_CULL_NEAR = 0 = never).
    @location(12) cull_near: vec2<f32>,
    // render::LightPath.
    @location(13) light: u32,
    // render::Fade [begin, end, near] metres, opacity (render::mesh::NO_FADE = never).
    @location(14) fade: vec4<f32>,
    // The point-light table records this instance takes, 0 = none.
    @location(15) lights: vec4<u32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    // Radial distance from the eye, for the far-scene cull and fades.
    @location(3) view_dist: f32,
    // Per-instance, so it is the same value across the whole primitive.
    @location(4) cull: vec2<f32>,
    @location(5) cull_near: vec2<f32>,
    @location(6) @interpolate(flat) light: u32,
    // World position, read by the water's two entry points only.
    @location(7) world_pos: vec3<f32>,
    // The instance's fade alpha (`fade_alpha`); 1 = drawn whole.
    @location(8) @interpolate(flat) fade: f32,
    // PLANAR view depth (clip w), the fog's coordinate (`fog_amount`).
    @location(9) fog_depth: f32,
    // The complete per-vertex material lighting in display units.
    @location(10) lighting: vec3<f32>,
    // The depth `color.rgb` and `lighting` are weighted by (`affine_w`); 0 on a
    // draw that blends them in perspective.
    @location(11) wc: f32,
    @location(12) @interpolate(flat) region_top: vec4<f32>,
    @location(13) @interpolate(flat) region_bottom: vec4<f32>,
    @location(14) @interpolate(flat) bloom_gain: f32,
};

// The PS2's vertex-colour blend (card BPRL, `analysis/re-vertex-colour-interp.md`
// §3-§5 [O]): the GS blends a triangle's vertex colour linearly in SCREEN
// space, and the VU1 clipper cuts a triangle only beyond the guard band or the
// near plane. A GPU blends in perspective, which carries the colour of a
// vertex under the camera far up a big floor. A model draw's vertex colour
// (rgb) and point sum therefore go out as a * wc beside wc and the fragment
// stage divides: weights beta * wc, the screen-space ones for a vertex in
// view, a convex mix always (plain `@interpolate(linear)` extrapolates across
// a triangle with a vertex behind the eye). wc is the vertex's depth, no
// nearer than where its view ray meets the guard band, or the near plane.
// A NAMED DEVIATION from perspective shading; AFFINE_COLOUR = false switches
// it off. Not taken by terrain, sky, water or glow, and only rgb is weighted:
// a vertex's alpha stays perspective-interpolated (§8, unmeasured).
// The CPU twin and its referee: `render::affine`.
const AFFINE_COLOUR: bool = true;
// `Instance::light`'s bit for "this draw's pass takes the blend"
// (`render::affine::AFFINE_BIT`); the low byte is the `render::LightPath`.
const AFFINE_BIT: u32 = 0x100u;
const AFFINE_NEAR: f32 = 0.1;

fn affine_w(clip: vec4<f32>, light: u32) -> f32 {
    let path = light & 0xffu;
    if !AFFINE_COLOUR || (light & AFFINE_BIT) == 0u || (path > LIGHT_UNLIT_ADD && path != LIGHT_CHARACTER && path != LIGHT_VERTEX_LIT) {
        return 0.0;
    }
    return max(max(clip.w, AFFINE_NEAR), max(abs(clip.x) * globals.guard_x, abs(clip.y) * globals.guard_y));
}

// The fragment side: a * wc and wc, both blended in perspective, back to a.
fn unweight(a: vec3<f32>, wc: f32) -> vec3<f32> {
    if wc > 0.0 {
        return a / wc;
    }
    return a;
}

fn affine_in(v: VsOut) -> VsOut {
    var o = v;
    o.color = vec4<f32>(unweight(v.color.rgb, v.wc), v.color.a);
    o.lighting = unweight(v.lighting, v.wc);
    return o;
}

// BF2's point-light term (card BPNT, `analysis/re-point-lights.md` §7 [O];
// VU1 0x23a-0x2a0), per vertex as the PS2 computes it, for the instance's
// <= 4 lights:
//     sum += C * ((1 + N.L^)/2)^2 * (1 - min(1, d^2/r^2))^2
// in GS units; the fragment stage adds it to the draw's light
// (`gs_lit`). The VU's 0.5/d is a fast reciprocal root, up to ~9 % high;
// this one is exact -- a stated deviation.
fn point_sum(p: vec3<f32>, normal: vec3<f32>, lights: vec4<u32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    if all(lights == vec4<u32>(0u)) {
        return sum;
    }
    let n = normalize(normal);
    for (var k = 0u; k < 4u; k++) {
        let i = lights[k];
        if i == 0u {
            continue;
        }
        let l = points.lights[i];
        let to = l.pos_inv_r2.xyz - p;
        let d2 = dot(to, to);
        let fall = 1.0 - min(d2 * l.pos_inv_r2.w, 1.0);
        let s = 0.5 + 0.5 * dot(to, n) * inverseSqrt(max(d2, 1e-12));
        sum += l.color.rgb * (s * s * fall * fall);
    }
    return sum;
}

fn mesh_vertex(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    var out: VsOut;
    let world_pos = model * vec4<f32>(in.position, 1.0);
    out.clip_pos = globals.view_proj * world_pos;
    // NOTE: correct for rotation + uniform scale only (fine for map props);
    // non-uniform scale would need an inverse-transpose normal matrix.
    out.world_normal = (model * vec4<f32>(in.normal, 0.0)).xyz;
    out.uv = in.uv;
    out.color = in.color * in.inst_color;
    // A self-lit BF2 segment's colour word replaces the draw's (card GLOW):
    // no instance tint.
    let path = in.light & 0xffu;
    out.bloom_gain = in.inst_color.a;
    if path == LIGHT_GLOW {
        out.color = in.color;
    }
    out.view_dist = length(world_pos.xyz - globals.camera_pos);
    out.fog_depth = out.clip_pos.w;
    out.world_pos = world_pos.xyz;
    out.cull = in.cull;
    out.cull_near = in.cull_near;
    out.light = path;
    out.region_top = in.region_top;
    out.region_bottom = in.region_bottom;
    out.fade = fade_alpha(in.m3.xyz, in.fade);
    out.lighting = vertex_lighting(out, in.lights);
    out.wc = affine_w(out.clip_pos, in.light);
    if out.wc > 0.0 {
        out.color = vec4<f32>(out.color.rgb * out.wc, out.color.a);
        out.lighting = out.lighting * out.wc;
    }
    return out;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    return mesh_vertex(in);
}

@vertex
fn vs_speckle(in: VsIn) -> VsOut {
    var out = mesh_vertex(in);
    out.color = in.inst_color;
    return out;
}

@vertex
fn vs_sky_bloom(in: VsIn) -> VsOut {
    var out = mesh_vertex(in);
    out.clip_pos.z = out.clip_pos.w;
    return out;
}

// The terrain foliage's fade (`render::Fade`, analysis/re-foliage.md §6):
// by the HORIZONTAL distance from the instance's origin to the eye, 1 up to
// `begin`, linear to 0 at `end`, and max(d / near, 0.1)^2 nearer than `near`.
// Per instance, as the PS2 computes it once per instance. Times the
// instance's opacity (`f.w`): 1 for the foliage, a constant for the chase
// camera's see-through props (`render::Fade::see_through`, ours).
fn fade_alpha(origin: vec3<f32>, f: vec4<f32>) -> f32 {
    let d = length(origin.xz - globals.camera_pos.xz);
    if d > f.x {
        return clamp((f.y - d) / max(f.y - f.x, 1e-6), 0.0, 1.0) * f.w;
    }
    if f.z > 0.0 && d < f.z {
        let r = max(d / f.z, 0.1);
        return r * r * f.w;
    }
    return f.w;
}

// The 4x4 Bayer threshold of a pixel, in (0, 1).
fn bayer4(p: vec2<f32>) -> f32 {
    let x = u32(p.x) & 3u;
    let y = u32(p.y) & 3u;
    let a = x ^ y;
    let v = ((a & 1u) << 3u) | ((y & 1u) << 2u) | (a & 2u) | ((y & 2u) >> 1u);
    return (f32(v) + 0.5) / 16.0;
}

// PS2 detail modulation. The GS multiplies the two texels as stored, i.e. in
// gamma space; our base texel arrives linearised by the sRGB sampler, so the
// equivalent linear-space factor is pow(d, 2.2) (gamma 2.2 approximates the
// sRGB curve). Alpha is untouched — the alpha test must see the base alpha.
fn detail_factor(uv: vec2<f32>) -> f32 {
    if material.detail.y == 0.0 || material.detail.w > 0.0 {
        return 1.0;
    }
    let d = textureSample(t_detail, s_color, uv * material.detail.x).a * 2.0;
    return pow(d, 2.2);
}

// OPAQUE and BLEND draws: no alpha test at all. An OPAQUE material's alpha is
// not a mask (grime, specular, a scroll ramp) and reading it as one is what
// shredded the Kamino crates; a BLEND material's alpha is the composite
// weight and testing it would delete the surface.
@fragment
fn fs_main(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw);
    far_cull(in);
    return shade(in);
}

// The far-scene cull: a draw carrying a cull distance is not drawn past it,
// and one carrying a NEAR edge is not drawn nearer than it.
// The PS2 ends its NEAR scene at a camera frustum whose far plane is
// `NearSceneRange[2]` -- that is where its heightfield stops -- and culls
// far-scene draws (world models by bounding sphere, the water's low-res pass,
// the LOWR sheet) at `FarSceneRange` (`analysis/re-fog.md` 2.5,
// `analysis/re-lowr.md` 2.2 [V]). `game::viewer::far_cull` picks the distance
// per draw; the two DEVIATIONS from the ELF are that this is one radial test
// per fragment rather than a planar clip plus a per-object sphere test, and
// that a culled draw leaves the frame's clear colour behind it rather than
// the PS2's far-scene buffer. NO sky draw ever reaches here.
// The near edge is for far-scene draws only: the PS2 draws the `LOWR` sheet into a
// SEPARATE far-scene buffer the near scene composites over, so its overlap
// with the heightfield costs it nothing; one depth buffer has to cut the
// overlap instead (`game::viewer::lowres_range`). A geometry's LOWD
// extension (card UTAS) takes the same edge at `NearSceneRange[1]`, the far
// scene's reach. NO_CULL_NEAR is 0, so the test is a no-op for every other draw.
// Each edge is a band [lo, hi] (`render::draw::instance_cull`), read at ONE
// dithered point per pixel; every draw's band has width 0 today (the far
// scene's model / LOWD switch picks a whole draw on the CPU,
// `render::LodSwitch`). `render::fragment_kept` is this test in Rust.
fn far_cull(in: VsOut) {
    if !CULLS {
        return;
    }
    let u = bayer4(in.clip_pos.xy);
    if in.view_dist > mix(in.cull.x, in.cull.y, u)
        || in.view_dist < mix(in.cull_near.x, in.cull_near.y, u) {
        discard;
    }
    // A fading instance (`fade_alpha`) as an ordered dither: the PS2 blends
    // that alpha, the port screens it (`render::Fade`, PROVISIONAL). Every
    // draw without a fade carries 1.0 and never reaches the test.
    if in.fade < 1.0 && in.fade <= bayer4(in.clip_pos.xy) {
        discard;
    }
}

// The PS2's alpha-test cutoff in DISC alpha terms (the lead's [D]
// composition of two [V] facts, `analysis/RE_LOG.md` "Alpha-test cutoff in
// disc terms"):
//   * a cutout (0x02) segment is tested ATST GEQUAL at AREF 0x60 = 96 on the
//     GS's 0..0x80 scale, TFX MODULATE, so the tested alpha is At * Av >> 7,
//     and the opaque vertex colour these draws carry gives
//     Av = (0xff * 0xff >> 8 + 1) >> 1 = 0x7f (`analysis/re-material-flags.md`
//     §2 "Texture function", §3 for the colour);
//   * the `pal_` loader uploads every CLUT alpha as At = (A * 0x81) >> 8, so
//     the disc's 0xff is the GS's 0x80 (`analysis/re-hud-icons.md` §1
//     citation 3).
// A texel survives iff ((A * 129 >> 8) * 127 >> 7) >= 96, i.e. A >= 193
// (191 if Av were a full 0x80; the cited formula gives 0x7f, so 193). The
// literal sits between 192/255 = 0.75294 and 193/255 = 0.75686, ~1000 float
// ULPs clear of the lower one, so 192 is discarded and 193 kept.
// `bake::detail::ALPHA_TEST` is the same number in 8 bits;
// `alpha_test_threshold_matches_the_shader` reads this line and pins both to
// the composition for every alpha.
const MASK_CUTOFF: f32 = 0.753;

// Surfaces and their bloom mask share the same material clock.
fn material_uv(uv: vec2<f32>) -> vec2<f32> {
    return uv + material.scroll.xy * globals.material_time;
}

// MASK draws: the disc's cutout materials (leaves, bamboo, banners, grates).
// A self-lit BF2 segment (`LIGHT_GLOW`) tests its overridden As against the
// same AREF 0x60 — disc alpha >= 98 under the full vertex alpha
// (`analysis/re-glow.md` §1.3 [D]).
fn mask_test(in: VsOut) {
    let uv = material_uv(in.uv);
    let texel = textureSample(t_color, s_color, uv);
    if in.light == LIGHT_GLOW {
        if glow_as(texel.a, in.color.a) < 96.0 {
            discard;
        }
        return;
    }
    let base = texel * in.color;
    if base.a < MASK_CUTOFF {
        discard;
    }
}

// BF2's self-lit alpha (`render::LightPath::Glow`, `analysis/re-glow.md`
// §1.2): As = min(255, At * Av >> 7) on the GS's scale, At = A * 129 >> 8
// the CLUT upload of the texel's disc alpha A, Av the VU1's vertex alpha
// (the vertex colour's alpha x 255, `bake::glb::glow_vertex_alpha`).
fn glow_as(tex_a: f32, vert_a: f32) -> f32 {
    let at = floor(round(tex_a * 255.0) * 129.0 / 256.0);
    let av = round(vert_a * 255.0);
    return min(255.0, floor(at * av / 128.0));
}

@fragment
fn fs_mask(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw);
    far_cull(in);
    mask_test(in);
    return shade(in);
}

// The `LOWR` far-LOD sheet's alpha test (`render::DrawLayer::FarSheet`, queue
// CLN5), in DISC alpha terms — `MASK_CUTOFF`'s composition with the sheet's
// own numbers: TEST 0x5010b is ATST GEQUAL at AREF 0x10 (`analysis/re-lowr.md`
// 2.2 [V]); the draw's base colour is 0x80808080, so Av = 0x80 and the tested
// alpha is At itself (TFX MODULATE, TCC 1 — the engine's one texture path,
// `re-water-alpha.md` 0, [D] for this draw); the CLUT upload gives
// At = (A * 0x81) >> 8. A texel survives iff A >= 32. PROVISIONAL [D].
const SHEET_CUTOFF: f32 = 0.1235;

// The `LOWR` sheet: UNLIT — base colour 0x80808080 is GS unity and the disc
// stores no normal (`analysis/re-lowr.md` 1/2.2 [V]) — alpha-tested at its own
// AREF, and fogged like any far-scene draw (`apply_fog`). The
// texture times the vertex/instance colour, nothing else. No `_fade` twin: a
// map with no far layer draws no sheet (`game::viewer::lowres_range`).
@fragment
fn fs_far_sheet(in: VsOut) -> @location(0) vec4<f32> {
    far_cull(in);
    let base = textureSample(t_color, s_color, in.uv) * in.color;
    if base.a < SHEET_CUTOFF {
        discard;
    }
    return vec4<f32>(apply_fog(base.rgb, in.fog_depth), 1.0);
}

fn shade(in: VsOut) -> vec4<f32> {
    let c = shade_unfogged(in);
    return vec4<f32>(apply_fog(c.rgb, in.fog_depth), c.a);
}

// `shade` without the fog. Split out for the WATER pass, which is fogged by
// its own rule and not by the world's (`fs_water`); every other entry point
// goes on reaching it through `shade`.
//
// The colour is the PS2's VU1 path for the draw (`render::LightPath`,
// `analysis/re-model-lighting.md` §2/§3, [O]): `in.color` carries the
// vertex colour — a world model's COL0, white where it has none — times the
// instance tint.
const LIGHT_PRELIT: u32 = 1u;
const LIGHT_PRELIT_ADD: u32 = 2u;
const LIGHT_UNLIT: u32 = 3u;
const LIGHT_UNLIT_ADD: u32 = 4u;
const LIGHT_TERRAIN: u32 = 5u;
const LIGHT_CHARACTER: u32 = 6u;
const LIGHT_GLOW: u32 = 7u;
const LIGHT_UNSCALED: u32 = 8u;
const LIGHT_REGION: u32 = 0x200u;
const LIGHT_IFACE: u32 = 9u;
const LIGHT_VERTEX_LIT: u32 = 10u;
// `render::IFACE_AMBIENT`.
const IFACE_AMBIENT: f32 = 0.5;

fn region_top(cull: vec2<f32>, cull_near: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(cull, cull_near.x, cull_near.y + 1.0);
}

fn vertex_lit_path(path: u32) -> bool {
    return path == 0u || path == LIGHT_CHARACTER || path == LIGHT_PRELIT
        || path == LIGHT_PRELIT_ADD || path == LIGHT_VERTEX_LIT || path == LIGHT_IFACE;
}

fn vertex_lighting(in: VsOut, lights: vec4<u32>) -> vec3<f32> {
    if !vertex_lit_path(in.light) {
        return vec3<f32>(0.0);
    }
    let n = normalize(in.world_normal);
    var ndotl = max(dot(n, -globals.sun_dir), 0.0);
    // BF1's VU1 sun term (`analysis/re-sun-direction.md` §1.8): its light
    // record is (-L/2, 1/2), so d = (1 + n.l)/2, squared twice.
    if globals.sun_d4 > 0.5 {
        let d = (1.0 + dot(n, -globals.sun_dir)) * 0.5;
        let d2 = d * d;
        ndotl = d2 * d2;
    }
    // The light is ONE sum in GS (display) units, as the VU1 makes it, and
    // the GS multiplies it into the texel once (card LSUM,
    // `analysis/re-kamino-decks.md` §2 [O]): every baked colour is linear,
    // so each goes back to display units before its weight. Summing the
    // linear colours left a deck under an ambient and a sun of one size
    // 1.4 x dark. The CPU twin and its referee: `render::lighting::light_sum`.
    var sun_g = linear_to_srgb(globals.sun_color) * ndotl;
    var light2_g = vec3<f32>(0.0);
    if any(globals.light2_color != vec3<f32>(0.0)) {
        light2_g = linear_to_srgb(globals.light2_color)
            * max(dot(n, -normalize(globals.light2_dir)), 0.0);
    }
    // Hemisphere ambient: n.y = +1 takes the sky colour, -1 the ground
    // (the VU's ambient pass, `analysis/re-model-lighting.md` §2).
    var ambient_g = mix(
        linear_to_srgb(globals.ambient_ground),
        linear_to_srgb(globals.ambient_sky),
        n.y * 0.5 + 0.5,
    );
    // A soldier and his weapon on BF1 take `CharacterAmbientColor` instead
    // (`analysis/re-model-lighting.md` §12); otherwise `Lit` exactly.
    if in.light == LIGHT_CHARACTER && globals.character_ambient_on > 0.5 {
        ambient_g = linear_to_srgb(globals.character_ambient);
    }
    if in.region_top.w >= 1.0 {
        ambient_g = mix(in.region_bottom.rgb, in.region_top.rgb, n.y * 0.5 + 0.5);
        sun_g *= in.region_top.w - 1.0;
        light2_g *= in.region_bottom.w;
    }
    let local = point_sum(in.world_pos, in.world_normal, lights);
    let dynamic_g = sun_g * (1.0 - globals.sun_static)
        + light2_g * (1.0 - globals.light2_static) + local;
    var lit: vec3<f32>;
    switch in.light {
        case LIGHT_IFACE: {
            let sum = (IFACE_AMBIENT + max(dot(n, -globals.iface_dir), 0.0)) * in.color.rgb;
            lit = gs_lit(vec3<f32>(1.0), sum);
        }
        case LIGHT_PRELIT: {
            lit = gs_lit(in.color.rgb, dynamic_g);
        }
        case LIGHT_PRELIT_ADD: {
            lit = gs_lit(vec3<f32>(1.0), linear_to_srgb(in.color.rgb) + dynamic_g);
        }
        case LIGHT_VERTEX_LIT: {
            lit = gs_lit(vec3<f32>(1.0), linear_to_srgb(in.color.rgb) + local);
        }
        default: {
            lit = gs_lit(in.color.rgb, ambient_g + sun_g + light2_g + local);
        }
    }
    return linear_to_srgb(lit);
}

fn emission_gain(texel_alpha: f32, in: VsOut) -> f32 {
    return select(2.0, 1.0 + glow_as(texel_alpha, in.color.a) / 128.0, in.light == LIGHT_GLOW)
        * globals.glow_gain;
}

fn shade_unfogged(in: VsOut) -> vec4<f32> {
    let uv = material_uv(in.uv);
    var texel = textureSample(t_color, s_color, uv);
    if in.light == LIGHT_TERRAIN {
        let display = textureSample(t_color_display, s_color, uv);
        texel = vec4<f32>(srgb_to_linear(display.rgb), display.a);
    }
    let base = texel * in.color;
    let detail = detail_factor(uv);
    if vertex_lit_path(in.light) {
        return vec4<f32>(texel.rgb * detail * srgb_to_linear(in.lighting), base.a);
    }
    let tex = texel.rgb * detail;
    var rgb: vec3<f32>;
    switch in.light {
        case LIGHT_UNLIT: {
            rgb = base.rgb * detail;
        }
        case LIGHT_UNLIT_ADD: {
            rgb = tex * (vec3<f32>(1.0) + in.color.rgb);
        }
        // BF2's self-lit segments on an HDR map (card GLOW): the unlit
        // colour times the GS's gain in DISPLAY units, clamped where the
        // frame buffer clamps. `Glow`'s alpha is the gain's mask, not a
        // coverage: an opaque draw writes it nowhere, so it returns 1.
        case LIGHT_GLOW: {
            let gain = (1.0 + glow_as(texel.a, in.color.a) / 128.0) * globals.glow_gain;
            let d = min(linear_to_srgb(base.rgb * detail) * gain, vec3<f32>(1.0));
            return vec4<f32>(srgb_to_linear(d), 1.0);
        }
        case LIGHT_UNSCALED: {
            let d = min(linear_to_srgb(base.rgb * detail) * 2.0 * globals.glow_gain, vec3<f32>(1.0));
            rgb = srgb_to_linear(d);
        }
        default: {
            // LIGHT_TERRAIN: the heightfield's GS vertex byte, 0x80 = 1.0,
            // reaching 1.94 on BF1; the GS clamps the product at 255.
            rgb = min(tex * srgb_to_linear(in.color.rgb * (255.0 / 128.0)), vec3<f32>(1.0));
        }
    }
    if in.light == LIGHT_TERRAIN && material.detail.w > 0.0 {
        let patch_size = material.detail.w;
        let local = floor(in.world_pos.xz / patch_size) - floor(globals.camera_pos.xz / patch_size);
        let detail_alpha = textureSample(t_detail, s_color, in.world_pos.xz * material.detail.x / patch_size).a;
        if all(abs(local) <= vec2<f32>(1.0)) {
            rgb = srgb_to_linear(clamp(linear_to_srgb(rgb) * (2.0 * detail_alpha), vec3<f32>(0.0), vec3<f32>(1.0)));
        }
    }
    return vec4<f32>(rgb, base.a);
}

// A texel (linear) under a light summed in GS units: the VU clamps the sum
// at 255/128, the GS multiplies it into the texel. The sum is decoded once
// and multiplied into the linear texel. The GS's own arithmetic -- the texel
// in display units, the product clamped at white -- scored worse over the
// 954 PCSX2 pairs (10.052 against 9.742 mean block error, card LSUM), so the
// frame buffer's clamp is not modelled here.
fn gs_lit(texel: vec3<f32>, light: vec3<f32>) -> vec3<f32> {
    return texel * srgb_to_linear(clamp(light, vec3<f32>(0.0), vec3<f32>(GS_MAX)));
}

// The PS2's fog (`analysis/re-fog-amount.md` §8, [O]; the curve PROVISIONAL,
// card FOGC). The maps author FogColor + FogRange(near, far) in their `sky_`
// chunk. Every fog pass binds the CORE.LVL texture `fogspread` and feeds it
// (s, t) = (f, f), f = (planar view depth - near) / (far - near) (§2-§4);
// the amount is its alpha there, and the GS lerps the framebuffer's STORED
// bytes toward the fog colour (§5.3). So: encode the shaded colour and the
// (linear, baked) fog colour to sRGB, lerp, decode for the sRGB target.
// `depth` is planar view depth (the vertex's clip w), not radial distance.
// Nearer than `near` (f <= 0) nothing is fogged: the PS2 never fogs its near
// scene, where the table's floor of 1/255 would tint dark texels (§5.5).
fn apply_fog(color: vec3<f32>, depth: f32) -> vec3<f32> {
    if globals.fog_on < 0.5 {
        return color;
    }
    let a = fog_amount(depth);
    if a <= 0.0 {
        return color;
    }
    let c = linear_to_srgb(max(color, vec3<f32>(0.0)));
    return srgb_to_linear(mix(c, linear_to_srgb(globals.fog_color), a));
}

// The fog amount at planar view depth `depth`, 0..248/255 (the fog on).
fn fog_amount(depth: f32) -> f32 {
    return fog_amount_range(depth, vec2<f32>(globals.fog_near, globals.fog_far));
}

fn fog_amount_range(depth: f32, range: vec2<f32>) -> f32 {
    let f = (depth - range.x) / max(range.y - range.x, 1e-3);
    if f <= 0.0 {
        return 0.0;
    }
    let x = min(f, 1.0) * 128.0;
    let i = min(u32(x), 127u);
    return mix(FOGSPREAD_DIAGONAL[i], FOGSPREAD_DIAGONAL[i + 1u], x - f32(i)) / 255.0;
}

// `fogspread`'s alpha along its diagonal: the 64x32 texture (identical on
// both discs, `re-fog-amount.md` §3) sampled bilinear and clamped at
// normalized (f, f), f = k / 128. Linear between entries it is within 0.1/255
// of the texture itself. Swappable: FOGC may replace the curve.
var<private> FOGSPREAD_DIAGONAL: array<f32, 129> = array<f32, 129>(
    1.0, 1.0, 2.0, 3.25, 4.25, 5.5, 6.5, 7.25, 8.5, 9.75, 11.0, 12.5, 14.0, 15.0, 16.5, 18.25,
    19.75, 21.5, 23.5, 25.25, 26.75, 28.0, 30.0, 32.25, 33.5, 34.75, 37.0, 39.25, 41.25, 43.5, 45.0, 46.5,
    48.5, 50.5, 52.5, 54.5, 56.5, 58.5, 61.0, 63.5, 65.5, 67.5, 69.5, 71.5, 73.75, 75.75, 78.5, 81.25,
    83.25, 85.5, 88.0, 90.25, 92.25, 94.5, 97.0, 99.5, 101.75, 103.75, 106.0, 108.75, 111.25, 113.5, 116.0, 118.5,
    120.75, 123.25, 125.5, 127.5, 130.0, 132.5, 135.0, 137.5, 140.0, 142.5, 145.0, 147.5, 149.5, 151.5, 154.0, 156.5,
    159.0, 161.5, 163.5, 165.5, 168.25, 170.75, 172.5, 174.75, 177.25, 179.5, 182.0, 184.25, 186.25, 188.5, 190.5, 192.5,
    194.5, 196.5, 199.0, 201.5, 203.25, 204.75, 207.0, 209.25, 211.25, 213.5, 215.5, 217.25, 218.75, 220.5, 222.5, 224.5,
    226.0, 227.5, 229.5, 231.25, 232.75, 234.0, 235.5, 237.25, 238.75, 240.5, 242.0, 243.25, 244.5, 245.75, 247.0, 248.0,
    248.0,
);

// The near scene's fade into the sky, for a map that draws NO far layer:
// BF2 END1/FEL1/YAV1 and the three that author no fog at all (DAG1, DEA1,
// NAB2) set `FarSceneRange(0)`, so the PS2 draws nothing into the far-scene
// buffer and its composite pass — a full-screen sprite keyed on a 256-entry
// depth->alpha palette, alpha 0 nearer than b, a ramp b..c, the far buffer
// alone past c (`analysis/re-fog.md` 2.5 [V code, D look]) — fades the near
// scene into the SKY. The authored fog colour is never drawn on those maps
// (re-fog.md 6), and `game::viewer::apply_lighting` drops it.
//
// Returns the NEAR scene's weight, which the `_fade` entry points write as
// alpha: 1 nearer than b, 0 past c. DEVIATIONS, both PROVISIONAL: the ramp is
// an unquantized reciprocal-depth ramp where the PS2 uses Z-buffer high bytes,
// and this composites per FRAGMENT in draw order rather than compositing the
// finished near-scene buffer once.
fn far_fade(dist: f32) -> f32 {
    if globals.fade_end <= globals.fade_begin {
        return 1.0;
    }
    let begin = max(globals.fade_begin, 1e-3);
    let end = globals.fade_end;
    return clamp((1.0 / max(dist, 1e-3) - 1.0 / end) / (1.0 / begin - 1.0 / end), 0.0, 1.0);
}

@fragment
fn fs_terrain_layer(in: VsOut) -> @location(0) vec4<f32> {
    far_cull(in);
    let c = shade(in);
    return vec4<f32>(c.rgb, c.a * far_fade(in.fog_depth));
}

@fragment
fn fs_terrain_layer_display(in: VsOut) -> @location(0) vec4<f32> {
    far_cull(in);
    let c = shade(in);
    return vec4<f32>(linear_to_srgb(c.rgb), c.a * far_fade(in.fog_depth));
}

// The three world entry points again, with the fade in alpha. OPAQUE and MASK
// draws REPLACE alpha with the fade -- an opaque material's own alpha is not
// a coverage (grime, specular, a scroll ramp) and compositing with it would
// dissolve the surface -- while a BLEND draw multiplies, keeping the
// material's composite weight. The skinned fade pipelines take `fs_fade` and
// `fs_blend_fade` too.
@fragment
fn fs_fade(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw);
    far_cull(in);
    return vec4<f32>(shade(in).rgb, far_fade(in.fog_depth));
}

@fragment
fn fs_mask_fade(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw);
    far_cull(in);
    mask_test(in);
    return vec4<f32>(shade(in).rgb, far_fade(in.fog_depth));
}

@fragment
fn fs_blend_fade(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw);
    far_cull(in);
    let c = shade(in);
    return vec4<f32>(c.rgb, c.a * far_fade(in.fog_depth));
}

// WATER draws (`render::DrawLayer::Water`): the map's surface, composited by
// its own alpha over the solid world and writing depth. UNLIT: the PS2
// colours its flat water per vertex in a VU1 overlay that reads no light
// colour and no normal, and draws its ocean at unity
// (`analysis/re-water-colour.md` §0/§1.2/§1.9 [O]). The instance colour is
// the GS-terms tint (`game::viewer::water_look`: MaxDiffuse / 255, the LUT's
// mean, and the alpha table's mean), and routine B adds the view-angle term
//     F = (1 - max(0, V.y))^3,  V = normalize((eye - water_eye_shift) - p)
// to colour AND alpha with `SpecularColor` / 255 as its weight (0 on an
// ocean), each clamped at the VU's 255/128. The GS then modulates:
// PROVISIONAL [O]: F per fragment (the PS2's per vertex on a 4 m grid) and
// the LUT's mean in place of its per-vertex noise (§5).
//
// BF2's OCEAN is lit (`globals.water_lit` > 0, card KSEAL): its packet
// carries the FFT normals, and the PS2's sea reads the texture x the sun's
// colour. The GS colour is scaled by `sun byte / 128 x max(n.l, 0) x weight`
// and by a loss with planar depth (`water_dim_*`), both fitted to PCSX2
// frames, PROVISIONAL (`game::viewer::water_look`, **Lit ocean**). Weight 0
// leaves every other map's water as the statements above made it.
//
// The fog is the water's own. The PS2 never fogs its near water in either
// game, and only BF1 fogs the far pass — so this fogs nothing inside
// `water_fog_begin`, applies the world's ramp past it, and takes
// `water_fog_begin == 0` as "never" (`analysis/re-water.md` §1 [V];
// `game::viewer::water_look` picks the distance per map). The port draws
// ONE water pass where the PS2 draws two of the same tiles, so the crossover
// is where the near scene ends rather than where the far pass starts.
//
// The far fade rides along unconditionally: `far_fade` is exactly 1.0 on a
// map without one, and this target is blended on every map, so there is no
// `_fade` twin of this entry point.
const GS_MAX: f32 = 255.0 / 128.0;

fn water_grid_color(at: vec2<i32>) -> vec3<f32> {
    let dims = vec2<i32>(globals.water_grid_dims);
    let wrapped = ((at % dims) + dims) % dims;
    let index = u32(wrapped.y * dims.x + wrapped.x);
    let packed = globals.water_grid_colors[index / 4u][index % 4u];
    return vec3<f32>(f32(packed & 255u), f32((packed >> 8u) & 255u), f32((packed >> 16u) & 255u)) / 255.0;
}

fn water_diffuse(in: VsOut) -> vec3<f32> {
    if any(globals.water_grid_dims == vec2<u32>(0u)) || in.fog_depth <= globals.water_grid_range.x {
        return in.color.rgb;
    }
    let q = (in.world_pos.xz - globals.water_grid_origin) / globals.water_grid_step;
    let cell = vec2<i32>(floor(q));
    let f = fract(q);
    let a = water_grid_color(cell);
    let b = water_grid_color(cell + vec2<i32>(1, 0));
    let c = water_grid_color(cell + vec2<i32>(0, 1));
    let d = water_grid_color(cell + vec2<i32>(1, 1));
    let low = vec4<f32>(1.0 - f.x - f.y, f.x, f.y, 0.0);
    let high = vec4<f32>(0.0, 1.0 - f.y, 1.0 - f.x, f.x + f.y - 1.0);
    let depth_step = vec2<f32>(globals.view_proj[0].w, globals.view_proj[2].w) * globals.water_grid_step;
    let depth = in.fog_depth - dot(f, depth_step);
    let corner_depths = depth + vec4<f32>(0.0, depth_step.x, depth_step.y, depth_step.x + depth_step.y);
    let weights = select(low, high, f.x + f.y > 1.0) * max(corner_depths, vec4<f32>(AFFINE_NEAR));
    let far = (a * weights.x + b * weights.y + c * weights.z + d * weights.w) / dot(weights, vec4<f32>(1.0));
    return mix(in.color.rgb, far, water_range_weight(in.fog_depth, globals.water_grid_range));
}

fn water_range_weight(depth: f32, range: vec2<f32>) -> f32 {
    if range.y <= range.x {
        return 0.0;
    }
    return clamp((depth - range.x) / (range.y - range.x), 0.0, 1.0);
}

fn water_texel(uv: vec2<f32>) -> vec4<f32> {
    let display_bytes = material.detail.z > 0.5;
    let texel = textureSampleBias(t_color, s_color, uv, select(0.0, -115.0 / 16.0, display_bytes));
    return vec4<f32>(select(linear_to_srgb(texel.rgb), texel.rgb, display_bytes), texel.a);
}

fn far_water_add(rgb: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let row = select(vec4<f32>(0.0, 2.0, 1.0, 3.0), vec4<f32>(1.0, 3.0, 0.0, 2.0), (u32(pixel.y) & 2u) != 0u);
    let step = 8.0 * globals.water_gain / 255.0;
    return floor(rgb / step + row[u32(pixel.x) & 3u] / 8.0) * step;
}

fn water_color(in: VsOut) -> vec4<f32> {
    far_cull(in);
    // The ocean's foam layer takes its own tiling (`uv_scale`, 1 on every
    // other water draw): the PS2's foam texture matrix scales by `FoamTile`
    // and shares the base's scroll offset (`analysis/re-water.md` §3d).
    let uv = in.uv * material.scroll.zw + material.scroll.xy * globals.time;
    // Donor water approximation. Normals are uploaded LINEAR, with tangent
    // X along world X and tangent Y along +Z (the baked water UV basis).
    // The authored reflection/refraction colours replace unavailable scene
    // reflection/refraction buffers. PROVISIONAL; replace: matched original
    // Xbox captures of the near and grazing water, including animated normals.
    if in.light == 11u {
        let texel = textureSample(t_color, s_color, uv);
        let n = normalize(vec3<f32>(texel.r * 2.0 - 1.0, texel.b * 2.0 - 1.0, texel.g * 2.0 - 1.0));
        let v = normalize(globals.camera_pos - in.world_pos);
        let limits = globals.speckle_axis.xy;
        let f = mix(limits.x, limits.y, pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 5.0));
        // Approximate an environment's horizon-to-zenith response. In
        // particular, equal reflection/refraction colours still show waves.
        let sky = 0.5 + 0.5 * max(reflect(-v, n).y, 0.0);
        let reflected = vec4<f32>(globals.water_specular.rgb * sky, globals.water_specular.a);
        let col = mix(in.color, reflected, f);
        var rgb = srgb_to_linear(col.rgb);
        if globals.water_fog_begin > 0.0 && in.view_dist > globals.water_fog_begin {
            rgb = apply_fog(rgb, in.fog_depth);
        }
        return vec4<f32>(rgb, clamp(col.a, 0.0, 1.0) * far_fade(in.fog_depth));
    }
    let texel = water_texel(uv);
    let v = normalize(globals.camera_pos - globals.water_eye_shift - in.world_pos);
    let f = pow(1.0 - max(v.y, 0.0), 3.0);
    let tint = min(vec4<f32>(water_diffuse(in), in.color.a) + globals.water_specular * f, vec4<f32>(GS_MAX));
    var rgb = srgb_to_linear(min(texel.rgb * tint.rgb * globals.water_gain, vec3<f32>(1.0)));
    if globals.water_lit > 0.0 {
        let ndotl = max(dot(normalize(in.world_normal), -globals.sun_dir), 0.0);
        let dim = max(1.0 - globals.water_dim_rate * max(in.fog_depth - globals.water_dim_begin, 0.0), 0.0);
        let light = linear_to_srgb(globals.sun_color) * (GS_MAX * ndotl * globals.water_lit);
        rgb = srgb_to_linear(linear_to_srgb(rgb) * min(light, vec3<f32>(GS_MAX)) * dim);
    }
    var alpha = min(texel.a * tint.a, 1.0);
    let fog = water_fog_amount(in);
    if fog > 0.0 {
        let combined = alpha * (1.0 - fog) + fog;
        let display = linear_to_srgb(rgb) * (alpha * (1.0 - fog))
            + linear_to_srgb(globals.water_fog_color) * fog;
        rgb = srgb_to_linear(display / combined);
        alpha = combined;
    }
    return vec4<f32>(rgb, alpha * far_fade(in.fog_depth));
}

fn water_fog_amount(in: VsOut) -> f32 {
    if globals.water_fog_on < 0.5 || globals.water_fog_begin <= 0.0 || in.view_dist <= globals.water_fog_begin {
        return 0.0;
    }
    return fog_amount_range(in.fog_depth, globals.water_fog_range);
}

@fragment
fn fs_water(in: VsOut) -> @location(0) vec4<f32> {
    return water_color(in);
}

@fragment
fn fs_water_display(in: VsOut) -> @location(0) vec4<f32> {
    let c = water_color(in);
    return vec4<f32>(linear_to_srgb(c.rgb), c.a);
}

// The water's SPECKLE layer (queue W3, `DrawLayer::Speckle`): one frame of
// the map's `SpeckleTextures`, tiled over the water mesh at its own scale and
// scrolled, ADDED over the finished water — the PS2's ALPHA 0x48, `Cs·As +
// Cd` (`analysis/re-water.md` §1 [V]). Its colour is the VU1 overlay's
// routine A (`re-water-colour.md` §1.7 [O]), unlit:
//     colour = SpeckleAmbient + SpeckleSpecular x lobe^32  (GS terms, rgba,
//              clamped at 255/128)
//     lobe   = max(0, l . normalize((p - eye + speckle_shift) * speckle_axis))
// — with `LightAzimAndElev` (1, 0) on every map, a horizontal cos^32 lobe
// toward the sun's azimuth. `Cs` = texel x colour (MODULATE); `As` = the
// sum's alpha ALONE, because the near speckle's TEX0 has its TCC cleared, so
// the sheet's alpha never reaches the blend (`re-water-alpha.md` §7.1
// [V][O]). The instance alpha carries the port's cross-fade weight between
// two frames (`game::viewer::SpeckleAnim`).
//
// Premultiplied into the (One, One) blend. Where the WATER takes the fog
// (`fs_water`), the add fades by the same `fog_amount`: the PS2 lays its `fogspread`
// pass over the far tile after the speckle. The far fade scales it as it
// scales the water.
fn speckle_color(in: VsOut) -> vec4<f32> {
    far_cull(in);
    let uv = in.uv * material.scroll.zw + material.scroll.xy * globals.time;
    let d = (in.world_pos - globals.camera_pos + globals.speckle_shift) * globals.speckle_axis;
    var lobe = 0.0;
    if dot(d, d) > 0.0 {
        lobe = pow(max(dot(globals.speckle_light, normalize(d)), 0.0), 32.0);
    }
    let col = min(globals.speckle_ambient + globals.speckle_specular * lobe, vec4<f32>(GS_MAX));
    let texel = water_texel(uv);
    let rgb = min(texel.rgb * col.rgb * globals.water_gain, vec3<f32>(1.0));
    let far = water_range_weight(in.fog_depth, vec2<f32>(globals.speckle_far_begin, globals.speckle_far_end));
    let near_weight = col.a * in.color.a;
    let far_weight = col.a * in.color.r * texel.a;
    let weight = mix(near_weight, far_weight, far);
    let far_rgb = rgb * far_weight;
    let quantized = select(far_rgb, far_water_add(far_rgb, in.clip_pos.xy), material.detail.z > 0.5);
    let contribution = mix(rgb * near_weight, quantized, far);
    let fade = far_fade(in.fog_depth) * (1.0 - water_fog_amount(in));
    return vec4<f32>(contribution / max(weight, 1e-6), weight * fade);
}

@fragment
fn fs_speckle(in: VsOut) -> @location(0) vec4<f32> {
    let c = speckle_color(in);
    return vec4<f32>(srgb_to_linear(c.rgb) * c.a, 0.0);
}

@fragment
fn fs_speckle_display(in: VsOut) -> @location(0) vec4<f32> {
    let c = speckle_color(in);
    return vec4<f32>(c.rgb * c.a, 0.0);
}

// Sky shells: pre-lit dome art, drawn before everything with no depth test.
// No lighting (the dome texture IS the sky). The map's `DomeInfo.Ambient`
// arrives as the instance colour.
//
// NO sky draw is ever fogged, and that is the ELF's rule, not ours: the sky
// tick draws every shell, the base dome, `Stars` and every `SkyObject` with
// the draw flag 0x20 set (0x20031), and both fog gates test it out --
// BF2 `(flags & 0x20020) == 0x20000` at 723621, BF1 `(flags & 0x20) == 0` at
// 594590 (`analysis/re-fog.md` 2.4 [V]). So `apply_fog` is never reached
// from here. Until 2026-09-03 a PARALLAX shell went through a second,
// FOGGED entry point (`fs_sky_fogged`) on the argument that it is authored
// LANDSCAPE and that leaving it sharp behind fogged terrain left a seam at
// the horizon -- the seam on Kamino. The PS2 draws that shell sharp and the
// seam is what the disc shows; the entry point is deleted.
//
// The shell's own material picks the blend state -- `render::SkyMode`, the
// ELF's per-material GS state under the sky draw flag. OPAQUE and BLEND share
// this entry point and differ only in the pipeline's blend: `None` paints,
// ALPHA_BLENDING composites. There is no third, alpha-tested sky mode (LEAD
// CORRECTION 2, session 12).
@fragment
fn fs_sky(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_color, s_color, in.uv);
    let base = texel * in.color;
    if in.light == LIGHT_GLOW || in.light == LIGHT_UNSCALED {
        let gain = emission_gain(texel.a, in);
        let rgb = srgb_to_linear(min(linear_to_srgb(base.rgb) * gain, vec3<f32>(1.0)));
        return vec4<f32>(rgb, select(base.a, 1.0, in.light == LIGHT_GLOW));
    }
    return base;
}

@fragment
fn fs_sky_bloom(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_color, s_color, in.uv);
    let base = texel * in.color;
    if in.light != LIGHT_GLOW && in.light != LIGHT_UNSCALED {
        return vec4<f32>(vec3<f32>(0.0), base.a);
    }
    let gain = emission_gain(texel.a, in);
    let rgb = min(linear_to_srgb(base.rgb) * gain, vec3<f32>(2.0));
    return vec4<f32>(rgb * 0.5, select(base.a, 1.0, in.light == LIGHT_GLOW));
}

// Sky objects use material lighting, with fog disabled around their draw
// (`analysis/re-bf2-geonosis-atmosphere.md`).
@fragment
fn fs_sky_lit(in: VsOut) -> @location(0) vec4<f32> {
    return shade_unfogged(in);
}

// SPECULAR draws (`render::DrawLayer::Spec`, queue SPEC): the second pass
// the disc draws right after a specular segment's base, ADDED over it
// (`analysis/re-specular-pass.md`, every claim [O], so PROVISIONAL):
//   * vs_spec: the tex-gen, per vertex as the VU1 does it (§3). R = 2(N.I)N - I
//     with I the unit eye -> vertex ray, then t = 0.5 + 0.5 d with
//     d = 2(X.L)(X.R) - L.R -- the disc dots the light in a camera frame whose
//     up and back axes are negated against R's (§4); `spec.mirror` = 0 takes
//     the consistent d = L.R. s = power / 255 rides in the instance alpha.
//     The table REPEATs, so s past 1 wraps (power 255 -> column 0).
//   * fs_spec: the GS's add, Cs * Ad + Cd with Cs = min(1, T * rgb) (TFX
//     MODULATE over a flat unlit colour, §2) and Ad the BASE pass's own
//     alpha -- its texel's, since the base writes it to the PSMCT32 frame
//     (§6). The GS adds in its gamma units; the target here is linear, so
//     this recomputes the base's colour (`shade_unfogged`, the same inputs
//     the base draw had), adds in gamma, and returns the linear difference
//     for an additive (One, One) blend. The fog and the far fade are applied
//     to both sides of the difference -- PROVISIONAL, [H]: whether BF1 fogs a
//     Specular segment is not read (§7), and BF2 draws the pass only in the
//     near scene, where its fog never reaches (the viewer culls the pass
//     there).
@group(3) @binding(0) var t_spec: texture_2d<f32>;
@group(3) @binding(1) var s_spec: sampler;
struct SpecParams {
    // The camera's screen-right axis, world space.
    cam_right: vec3<f32>,
    // 1 = the disc's mirrored light, 0 = the consistent one.
    mirror: f32,
};
@group(3) @binding(2) var<uniform> spec: SpecParams;

struct VsSpecOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // The BASE draw's colour: the vertex colour alone, because every base
    // draw the viewer makes carries a white instance colour.
    @location(2) color: vec4<f32>,
    @location(3) view_dist: f32,
    @location(4) cull: vec2<f32>,
    @location(5) cull_near: vec2<f32>,
    @location(6) spec_uv: vec2<f32>,
    @location(7) spec_rgb: vec3<f32>,
    // The base draw's `render::LightPath`, so `fs_spec` recomputes the base
    // colour the base draw had.
    @location(8) @interpolate(flat) light: u32,
    // The base draw's fade, so the pass screens the same pixels away.
    @location(9) @interpolate(flat) fade: f32,
    @location(10) fog_depth: f32,
    // The base draw's complete per-vertex material lighting.
    @location(11) lighting: vec3<f32>,
    // The base draw's `VsOut::wc`.
    @location(12) wc: f32,
};

@vertex
fn vs_spec(in: VsIn) -> VsSpecOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    // The same expressions as `vs_main`, so the pass lands on its base's
    // depth and the LessEqual test keeps it.
    let world_pos = model * vec4<f32>(in.position, 1.0);
    let world_normal = (model * vec4<f32>(in.normal, 0.0)).xyz;
    return spec_varyings(world_pos, world_normal, in.uv, in.color, in.inst_color, in.cull, in.cull_near, in.light, fade_alpha(in.m3.xyz, in.fade), in.lights, in.region_bottom, in.region_top);
}

// The skinned twin (queue SPEC2): `vs_skinned`'s expressions, so the pass
// lands on a soldier's own depth, and R from the SKINNED normal -- the disc
// runs the pass inside the one per-segment draw every RedModel takes, skinned
// included (`analysis/re-specular-pass.md` §1, [O]).
@vertex
fn vs_spec_skinned(in: VsSkinnedIn) -> VsSpecOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let skin = in.weights.x * palette.joints[in.joints.x]
        + in.weights.y * palette.joints[in.joints.y]
        + in.weights.z * palette.joints[in.joints.z]
        + in.weights.w * palette.joints[in.joints.w];
    let world = model * skin;
    let world_pos = world * vec4<f32>(in.position, 1.0);
    let world_normal = (world * vec4<f32>(in.normal, 0.0)).xyz;
    return spec_varyings(world_pos, world_normal, in.uv, in.color, in.inst_color, in.cull, in.cull_near, in.light, 1.0, in.lights, in.region_bottom, vec4<f32>(0.0));
}

// The pass's varyings from one vertex's world position and normal: the base's
// own (clip position, colour, distance) plus the tex-gen.
fn spec_varyings(
    world_pos: vec4<f32>,
    world_normal: vec3<f32>,
    uv: vec2<f32>,
    color: vec4<f32>,
    inst_color: vec4<f32>,
    cull: vec2<f32>,
    cull_near: vec2<f32>,
    light: u32,
    fade: f32,
    lights: vec4<u32>,
    region_bottom: vec4<f32>,
    rigid_region_top: vec4<f32>,
) -> VsSpecOut {
    var out: VsSpecOut;
    out.light = light & 0xffu;
    out.fade = fade;
    var base: VsOut;
    base.world_pos = world_pos.xyz;
    base.world_normal = world_normal;
    base.color = color;
    base.light = out.light;
    base.region_top = rigid_region_top;
    base.region_bottom = region_bottom;
    if (light & LIGHT_REGION) != 0u {
        base.region_top = region_top(cull, cull_near);
        base.region_bottom = region_bottom;
    }
    out.lighting = vertex_lighting(base, lights);
    out.clip_pos = globals.view_proj * world_pos;
    out.world_normal = world_normal;
    out.uv = uv;
    out.color = color;
    out.wc = affine_w(out.clip_pos, light);
    if out.wc > 0.0 {
        out.color = vec4<f32>(color.rgb * out.wc, color.a);
        out.lighting = out.lighting * out.wc;
    }
    out.view_dist = length(world_pos.xyz - globals.camera_pos);
    out.fog_depth = out.clip_pos.w;
    out.cull = cull;
    out.cull_near = cull_near;
    if (light & LIGHT_REGION) != 0u {
        out.cull = vec2<f32>(1.0e9);
        out.cull_near = vec2<f32>(0.0);
    }

    let n = normalize(world_normal);
    let i = normalize(world_pos.xyz - globals.camera_pos);
    let r = 2.0 * dot(n, i) * n - i;
    let l = normalize(globals.sun_dir);
    var d = dot(l, r);
    if spec.mirror > 0.5 {
        d = 2.0 * dot(spec.cam_right, l) * dot(spec.cam_right, r) - d;
    }
    out.spec_uv = vec2<f32>(inst_color.a, 0.5 + 0.5 * d);
    out.spec_rgb = inst_color.rgb;
    return out;
}

// sRGB transfer, both ways (the exact curve, not gamma 2.2: the target's own
// encode is the exact one, and the difference below must vanish where the
// add does).
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_spec(in: VsSpecOut) -> @location(0) vec4<f32> {
    var b: VsOut;
    b.clip_pos = in.clip_pos;
    b.world_normal = in.world_normal;
    b.uv = in.uv;
    b.color = vec4<f32>(unweight(in.color.rgb, in.wc), in.color.a);
    b.view_dist = in.view_dist;
    b.cull = in.cull;
    b.cull_near = in.cull_near;
    b.light = in.light;
    b.fade = in.fade;
    b.fog_depth = in.fog_depth;
    b.lighting = unweight(in.lighting, in.wc);
    b.wc = 0.0;
    far_cull(b);
    let base = shade_unfogged(b);
    let t = textureSample(t_spec, s_spec, in.spec_uv).r;
    let add = min(t * in.spec_rgb, vec3<f32>(1.0)) * clamp(base.a, 0.0, 1.0);
    let before = clamp(base.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let after = srgb_to_linear(min(linear_to_srgb(before) + add, vec3<f32>(1.0)));
    let stored = clamp(apply_fog(base.rgb, in.fog_depth), vec3<f32>(0.0), vec3<f32>(1.0));
    let wanted = clamp(apply_fog(after, in.fog_depth), vec3<f32>(0.0), vec3<f32>(1.0));
    let delta = max(wanted - stored, vec3<f32>(0.0));
    return vec4<f32>(delta * far_fade(in.fog_depth), 0.0);
}

struct VsSkinnedIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) inst_color: vec4<f32>,
    @location(9) joints: vec4<u32>,
    @location(10) weights: vec4<f32>,
    @location(11) cull: vec2<f32>,
    @location(12) cull_near: vec2<f32>,
    @location(13) light: u32,
    @location(14) region_bottom: vec4<f32>,
    @location(15) lights: vec4<u32>,
};

@vertex
fn vs_skinned(in: VsSkinnedIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let skin = in.weights.x * palette.joints[in.joints.x]
        + in.weights.y * palette.joints[in.joints.y]
        + in.weights.z * palette.joints[in.joints.z]
        + in.weights.w * palette.joints[in.joints.w];
    let world = model * skin;
    var out: VsOut;
    let world_pos = world * vec4<f32>(in.position, 1.0);
    out.clip_pos = globals.view_proj * world_pos;
    out.world_normal = (world * vec4<f32>(in.normal, 0.0)).xyz;
    out.uv = in.uv;
    out.color = in.color * in.inst_color;
    out.view_dist = length(world_pos.xyz - globals.camera_pos);
    out.fog_depth = out.clip_pos.w;
    out.world_pos = world_pos.xyz;
    out.cull = in.cull;
    out.cull_near = in.cull_near;
    out.light = in.light & 0xffu;
    if (in.light & LIGHT_REGION) != 0u {
        out.region_top = region_top(in.cull, in.cull_near);
        out.region_bottom = in.region_bottom;
        out.cull = vec2<f32>(1.0e9);
        out.cull_near = vec2<f32>(0.0);
    }
    out.fade = 1.0;
    out.lighting = vertex_lighting(out, in.lights);
    out.wc = affine_w(out.clip_pos, in.light);
    if out.wc > 0.0 {
        out.color = vec4<f32>(out.color.rgb * out.wc, out.color.a);
        out.lighting = out.lighting * out.wc;
    }
    return out;
}

// [ours] Gather only marked emissive surfaces, before display clipping.
fn bloom_light(in: VsOut) -> vec4<f32> {
    let uv = material_uv(in.uv);
    let texel = textureSample(t_color, s_color, uv);
    let base = texel * in.color;
    let gain = emission_gain(texel.a, in);
    let d = min(linear_to_srgb(base.rgb * detail_factor(uv)) * gain, vec3<f32>(2.0));
    let fogged = linear_to_srgb(apply_fog(srgb_to_linear(d), in.fog_depth));
    let coverage = select(base.a, 1.0, in.light == LIGHT_GLOW);
    return vec4<f32>(fogged * 0.5 * coverage * far_fade(in.fog_depth), max(1.0, in.bloom_gain));
}
@fragment
fn fs_bloom(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw); far_cull(in); return bloom_light(in);
}
@fragment
fn fs_bloom_mask(raw: VsOut) -> @location(0) vec4<f32> {
    let in = affine_in(raw); far_cull(in); mask_test(in); return bloom_light(in);
}
