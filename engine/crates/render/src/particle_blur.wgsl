@group(2) @binding(0) var blur_scene: texture_2d<f32>;
@group(2) @binding(1) var blur_sampler: sampler;

struct BlurOut {
    @builtin(position) position: vec4<f32>,
    @location(0) mask_uv: vec2<f32>,
    @location(1) @interpolate(linear) screen_uv: vec2<f32>,
    @location(2) @interpolate(flat) alpha: f32,
};

@vertex
fn vs_particle_blur(
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> BlurOut {
    let clip = globals.view_proj * vec4<f32>(position, 1.0);
    return BlurOut(
        clip,
        uv,
        clip.xy / clip.w * vec2<f32>(0.5, -0.5) + 0.5,
        color.a,
    );
}

@fragment
fn fs_particle_blur(in: BlurOut) -> @location(0) vec4<f32> {
    let mask = saturate(textureSample(t_color, s_color, in.mask_uv).a * in.alpha);
    return vec4<f32>(textureSample(blur_scene, blur_sampler, in.screen_uv).rgb, mask);
}
