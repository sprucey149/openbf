// The world target stretched over the frame (`world_scale.rs`).

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_upscale(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var world: texture_2d<f32>;
@group(0) @binding(1) var world_sampler: sampler;

@fragment
fn fs_upscale(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(world, world_sampler, in.uv);
}
