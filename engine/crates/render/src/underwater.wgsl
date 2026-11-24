// PC BF2's underwater tint (`underwater.rs`, `analysis/re-water.md` §9.8).
// Each pixel's point on the near plane (reverse-Z depth 1) is unprojected;
// at or under the level it takes `UnderwaterColor`, alpha-blended.
struct UnderwaterParams {
    inverse: mat4x4<f32>,
    color: vec4<f32>,
    // x = level, y/z = 1 / target size, w = 1 to linearise for an sRGB target.
    info: vec4<f32>,
};
@group(0) @binding(0) var<uniform> up: UnderwaterParams;

@vertex
fn vs_underwater(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn u_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_underwater(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let ndc = vec2<f32>(pos.x * up.info.y * 2.0 - 1.0, 1.0 - pos.y * up.info.z * 2.0);
    let p = up.inverse * vec4<f32>(ndc, 1.0, 1.0);
    if p.y / p.w > up.info.x {
        discard;
    }
    if up.info.w > 0.5 {
        return vec4<f32>(u_srgb_to_linear(up.color.rgb), up.color.a);
    }
    return up.color;
}
