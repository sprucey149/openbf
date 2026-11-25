struct Params {
    corners: array<vec4<f32>, 4>,
    shape: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var hud: texture_2d<f32>;
@group(0) @binding(2) var hud_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    var sides = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0),
    );
    let corner = order[i];
    return VertexOut(params.corners[corner], vec2(0.5) + sides[corner] * params.shape.xy);
}

@fragment
fn fs_main(v: VertexOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(hud, hud_sampler, v.uv, 0.0) * params.shape.z;
}
