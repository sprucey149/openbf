struct Params {
    eye: vec4<f32>,
    rotation: vec4<f32>,
    fov: vec4<f32>,
    half_angles: vec4<f32>,
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
    let uv = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4(uv * 2.0 - 1.0, 0.0, 1.0), uv);
}

@fragment
fn fs_main(v: VertexOut) -> @location(0) vec4<f32> {
    let fov = params.fov;
    let ray = normalize(vec3(mix(fov.x, fov.y, v.uv.x), mix(fov.z, fov.w, v.uv.y), -1.0));
    let q = params.rotation;
    let t = 2.0 * cross(q.xyz, ray);
    let direction = ray + q.w * t + cross(q.xyz, t);
    let eye = params.eye.xyz;
    let b = dot(eye, direction);
    let distance = sqrt(max(0.0, b * b + params.eye.w * params.eye.w - dot(eye, eye))) - b;
    let p = eye + direction * distance;
    let angular = vec2(atan2(p.x, -p.z), atan2(p.y, length(p.xz)));
    let uv = vec2(0.5) + angular / params.half_angles.xy * vec2(0.5, -0.5);
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
        // Extend full-screen veils/popups (present on all four HUD edges)
        // across the entire VR eye view instead of clipping to the visor quad.
        let b0 = textureSampleLevel(hud, hud_sampler, vec2(0.002, 0.5), 0.0);
        let b1 = textureSampleLevel(hud, hud_sampler, vec2(0.998, 0.5), 0.0);
        let b2 = textureSampleLevel(hud, hud_sampler, vec2(0.5, 0.002), 0.0);
        let b3 = textureSampleLevel(hud, hud_sampler, vec2(0.5, 0.998), 0.0);
        let backdrop = min(min(b0, b1), min(b2, b3));
        if backdrop.a <= 0.001 {
            discard;
        }
        return backdrop;
    }
    return textureSampleLevel(hud, hud_sampler, uv, 0.0);
}
