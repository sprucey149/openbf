@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
struct VolumeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) depth: vec2<f32>,
};
@vertex fn vs_volume(@location(0) p:vec3<f32>) -> VolumeOut {
    let clip=view_proj*vec4<f32>(p,1.0);
    var out: VolumeOut;
    // Disable only near/far clipping. Preserve homogeneous x/y clipping and
    // the original depth for a per-fragment clamp, including the far cap.
    // Clamping z per vertex warps the interpolated depths of long side faces.
    out.clip=vec4(clip.xy,0.5*clip.w,clip.w);
    out.depth=clip.zw;
    return out;
}
struct VolumeFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};
@fragment fn fs_volume(in:VolumeOut)->VolumeFragment {
    var out: VolumeFragment;
    out.color=vec4(0.);
    out.depth=clamp(in.depth.x/in.depth.y,0.,1.);
    return out;
}
@vertex fn vs_full(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[i],0.,1.);
}
@fragment fn fs_black()->@location(0) vec4<f32> { return vec4(0.,0.,0.,0.375); }
// Only surfaces lacking a display view use this gamma approximation.
@fragment fn fs_linear_fallback()->@location(0) vec4<f32> { return vec4(0.,0.,0.,1.-pow(0.625,2.2)); }
