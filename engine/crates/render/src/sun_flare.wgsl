// The PS2 flare's triangles; analysis/re-hoth-atmosphere.md.
struct Params { p: array<vec4<f32>, 10> };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var depth: texture_depth_2d;
struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) source: vec2<f32>,
    @location(3) kind: f32,
};
struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) source: vec2<f32>,
    @location(2) @interpolate(flat) kind: f32,
};
@vertex
fn vs_main(v: Vertex) -> Fragment {
    var out: Fragment;
    out.position = vec4(v.position / params.p[8].xy * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.color = v.color;
    out.source = v.source;
    out.kind = v.kind;
    return out;
}
fn sky_at(p: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(depth));
    if any(p < vec2(0.0)) || any(p >= vec2<f32>(size)) { return 0.0; }
    return select(0.0, 1.0, textureLoad(depth, vec2<i32>(p), 0) >= 0.999999);
}
fn flare(v: Fragment) -> vec3<f32> {
    var visible = sky_at(v.position.xy);
    if v.kind > 0.5 {
        // PROVISIONAL depth filter; measure: compare PS2 FUN_00470ce0's reduced source mask under partial occlusion.
        visible = 0.0;
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                visible += sky_at(params.p[0].xy + vec2<f32>(f32(x), f32(y)) * max(1.0, params.p[0].w * params.p[0].z * 0.5));
            }
        }
        visible /= 9.0;
        if v.kind > 1.5 {
            visible = select(0.0, sky_at(v.source), visible > 0.0);
        }
    }
    return v.color.rgb * v.color.a * visible * params.p[8].z;
}
@fragment
fn fs_main(v: Fragment) -> @location(0) vec4<f32> {
    return vec4(flare(v), 0.0);
}
@fragment
fn fs_bloom(v: Fragment) -> @location(0) vec4<f32> {
    return vec4(flare(v) * 0.5, 0.0);
}
