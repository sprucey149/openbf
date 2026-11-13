// ── The DomeInfo frame filter (`filter.rs`, card BFLT) ──────────────────────
// `analysis/re-glow.md` §3 [O]: over the finished 3D frame, per channel in
// display bytes, out = min(1, v + gain · max(0, lowpass(v) − threshold)).
// The frame's copy is read through two views: `t_frame` in the target's
// sRGB format (linear light, for the pixels left as they were) and `t_disp`
// without the sRGB suffix (the GS's display bytes, so the box averages
// bytes, not linear light). The n × n box is separable (card ISPF):
// `fs_filter_rows` averages n texels along x into `t_rows`, `fs_filter`
// averages n of those along y, two texels a sample.
struct FilterParams {
    // x = threshold (0..1 of a display byte 255), y = gain (Intensity/128),
    p: vec4<f32>,
};
@group(0) @binding(0) var t_frame: texture_2d<f32>;
@group(0) @binding(1) var<uniform> fp: FilterParams;
@group(0) @binding(2) var t_disp: texture_2d<f32>;
@group(0) @binding(3) var t_rows: texture_2d<f32>;
// Linear, clamped to the edge: one sample half-way between two texel
// centres is their mean (its weight 1/2 is exact in the filter's fixed
// point), and an edge pair is the border texel, as the per-texel clamp.
@group(0) @binding(4) var s_pair: sampler;

// The sum of the n texels from `first` along `axis` (1,0 or 0,1) through
// `px`'s other coordinate: n/2 pair samples plus the odd one.
fn box_sum(t: texture_2d<f32>, px: vec2<i32>, axis: vec2<i32>, first: i32, n: i32) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(t));
    let inv = 1.0 / vec2<f32>(size);
    let along = vec2<f32>(axis);
    let across = (vec2<f32>(px) + 0.5) * (vec2<f32>(1.0) - along);
    var sum = vec3<f32>(0.0);
    var i = 0;
    for (; i + 1 < n; i += 2) {
        // The boundary between texels first+i and first+i+1.
        let uv = (across + along * f32(first + i + 1)) * inv;
        sum += 2.0 * textureSampleLevel(t, s_pair, uv, 0.0).rgb;
    }
    if i < n {
        let q = clamp(px * (vec2<i32>(1) - axis) + axis * (first + i), vec2<i32>(0), size - 1);
        sum += textureLoad(t, q, 0).rgb;
    }
    return sum;
}

fn f_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// One triangle over the whole target.
@vertex
fn vs_filter(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// The PS2's 2:1 box at 256×224 is a box of height/224 output pixels here,
// centred on the pixel (its sub-pixel blur is below one PS2 pixel); the
// edges repeat the border texel.

// Pass 1: the mean of the n display texels along x.
@fragment
fn fs_filter_rows(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let n = i32(fp.p.z);
    let sum = box_sum(t_disp, px, vec2<i32>(1, 0), px.x - n / 2, n);
    return vec4<f32>(sum / f32(n), 1.0);
}

// Pass 2: the mean of the n row means along y, thresholded and added.
@fragment
fn fs_filter(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let n = i32(fp.p.z);
    let low = box_sum(t_rows, px, vec2<i32>(0, 1), px.y - n / 2, n) / f32(n);
    if fp.p.w > 0.0 {
        let original = textureLoad(t_disp, px, 0).rgb;
        return vec4<f32>(f_srgb_to_linear(mix(original, low, fp.p.w)), 1.0);
    }
    let add = fp.p.y * max(low - vec3<f32>(fp.p.x), vec3<f32>(0.0));
    // Below the threshold the pixel is left as it was, bit for bit.
    if all(add == vec3<f32>(0.0)) {
        return textureLoad(t_frame, px, 0);
    }
    let v = textureLoad(t_disp, px, 0).rgb;
    let out = min(v + add, vec3<f32>(1.0));
    return vec4<f32>(f_srgb_to_linear(out), 1.0);
}
