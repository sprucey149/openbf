struct ShadowView {
    light_clip: mat4x4<f32>,
    world_from_clip: mat4x4<f32>,
    eye_and_time: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> shadow: ShadowView;
@group(1) @binding(0) var diffuse: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;
struct Material {
    detail: vec4<f32>,
    scroll: vec4<f32>,
    offset: vec4<f32>,
};
@group(1) @binding(3) var<uniform> material: Material;
struct Caster {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) tint: vec4<f32>,
    @location(13) light: u32,
};

struct MaskVertex {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) @interpolate(flat) light: u32,
};

@vertex fn vs_rigid(in: Caster) -> MaskVertex {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    return MaskVertex(
        shadow.light_clip * model * vec4(in.position, 1.0),
        in.uv, in.color.a * in.tint.a, in.light,
    );
}

@fragment fn fs_mask(in: MaskVertex) {
    let uv = in.uv + material.scroll.xy * shadow.eye_and_time.w + material.offset.xy;
    let a = textureSample(diffuse, diffuse_sampler, uv).a;
    // The material mask thresholds used by the world pass.
    if (in.light == 7u) {
        let at = floor(round(a * 255.0) * 129.0 / 256.0);
        if (floor(at * round(in.alpha * 255.0) / 128.0) < 96.0) { discard; }
    } else if (a * in.alpha < 0.753) {
        discard;
    }
}

@group(0) @binding(1) var scene_depth: texture_depth_2d;
@group(0) @binding(2) var light_depth: texture_depth_2d;
@group(0) @binding(3) var light_compare: sampler_comparison;

@vertex fn vs_composite(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[i], 0.0, 1.0);
}

fn receiver(pixel: vec2<f32>) -> vec3<f32> {
    let scene_z = textureLoad(scene_depth, vec2<i32>(pixel), 0);
    if (scene_z == 0.0) { discard; }
    let ndc = pixel / vec2<f32>(textureDimensions(scene_depth)) * vec2(2.0, -2.0)
        + vec2(-1.0, 1.0);
    let h = shadow.world_from_clip * vec4(ndc, scene_z, 1.0);
    return h.xyz / h.w;
}

fn receiver_fade(world: vec3<f32>) -> f32 {
    let distance = length(world - shadow.eye_and_time.xyz);
    if (distance >= shadow.params.y) { discard; }
    return 1.0 - smoothstep(shadow.params.x, shadow.params.y, distance);
}

@fragment fn fs_stencil(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4(0.0, 0.0, 0.0, receiver_fade(receiver(pixel.xy)) * shadow.params.w);
}

@fragment fn fs_composite(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let world = receiver(pixel.xy);
    let fade = receiver_fade(world);
    let light = (shadow.light_clip * vec4(world, 1.0)).xyz;
    let uv = light.xy * vec2(0.5, -0.5) + vec2(0.5);
    if (any(uv < vec2(0.0)) || any(uv >= vec2(1.0)) || light.z < 0.0 || light.z > 1.0) {
        discard;
    }
    let lit = textureSampleCompareLevel(light_depth, light_compare, uv, light.z - shadow.params.z);
    return vec4(0.0, 0.0, 0.0, (1.0 - lit) * fade * shadow.params.w);
}
