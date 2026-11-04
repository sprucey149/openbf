// Half-scale display light, thresholded before the quarter-size downsample.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> params: vec4<f32>;
@group(0) @binding(3) var frame: texture_2d<f32>;

@vertex
fn vs_bloom(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

@fragment
fn fs_extract(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let base = vec2<i32>(p.xy * 4.0) - vec2<i32>(2);
    var sum = vec3<f32>(0.0);
    var lamp = 0.0;
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            let c = textureLoad(source, clamp(base + vec2<i32>(x,y), vec2<i32>(0), size - vec2<i32>(1)), 0);
            if dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114)) >= params.x {
                sum += min(c.rgb, vec3<f32>(1.0)) * c.a;
                lamp = max(lamp, c.a - 1.0);
            }
        }
    }
    return vec4<f32>(sum / 16.0, lamp);
}

@fragment
fn fs_blur(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(source));
    let uv = p.xy / size;
    let step = vec2<f32>(params.z) / size;
    let center = textureLoad(source, vec2<i32>(p.xy), 0);
    var sum = center.rgb;
    sum += textureSampleLevel(source, linear_sampler, uv + step, 0.0).rgb;
    sum += textureSampleLevel(source, linear_sampler, uv - step, 0.0).rgb;
    sum += textureSampleLevel(source, linear_sampler, uv + step * vec2<f32>(1.0,-1.0), 0.0).rgb;
    sum += textureSampleLevel(source, linear_sampler, uv + step * vec2<f32>(-1.0,1.0), 0.0).rgb;
    return vec4<f32>(min(sum * params.y, vec3<f32>(1.0)), center.a);
}

@fragment
fn fs_composite(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    // The small buffer is ceil(size / 4): each texel covers 4x4 full-res pixels.
    let uv = p.xy / (4.0 * vec2<f32>(textureDimensions(source)));
    let glow = textureSampleLevel(source, linear_sampler, uv, 0.0);
    var base = textureLoad(frame, vec2<i32>(p.xy), 0).rgb;
    if glow.a > 0.0 {
        let size = vec2<f32>(textureDimensions(frame));
        let center = p.xy / size;
        let step = vec2<f32>(size.y / size.x, 1.0) / 448.0;
        let side = step * vec2<f32>(1.0, -1.0);
        let soft = (textureSampleLevel(frame, linear_sampler, center + step, 0.0).rgb
            + textureSampleLevel(frame, linear_sampler, center - step, 0.0).rgb
            + textureSampleLevel(frame, linear_sampler, center + side, 0.0).rgb
            + textureSampleLevel(frame, linear_sampler, center - side, 0.0).rgb) * 0.25;
        base = mix(base, soft, (60.0 / 128.0) * saturate(glow.a * 4.0));
    }
    let c = min(base + glow.rgb, vec3<f32>(1.0));
    let linear = select(c / 12.92, pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c > vec3<f32>(0.04045));
    return vec4<f32>(linear, 1.0);
}
