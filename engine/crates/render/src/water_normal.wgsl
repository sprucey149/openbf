// PC Battlefront II normal-mapped water composite (`water_reflect.rs`, appended
// after `shader.wgsl`), modeled after `core.lvl`'s `water_shader.xml` and PC
// `BattlefrontII.exe` (`0x6fee10`–`0x6ff82a`):
// - Underwater pass: multiplies the unperturbed opaque scene (`t_water_scene`)
//   by `RefractionColor` (`in.color`) in display units.
// - Reflection pass: perturbs the planar mirrored world (`t_water_reflect`, sky
//   + geometry above `water_y`) by the animated normal map projected into the
//   horizontal camera basis at scale `0.03`, scaled by `ReflectionColor.a` plus
//   an 8th-power sun specular highlight.
// - Blends refraction and reflection by Fresnel:
//     f = mix(fresnel_min, fresnel_max, (1 - N·V)^2).

@group(3) @binding(3) var t_water_scene: texture_2d<f32>;
@group(3) @binding(4) var t_water_reflect: texture_2d<f32>;
@group(3) @binding(5) var s_water_rt: sampler;

fn water_normal_color(in: VsOut) -> vec4<f32> {
    far_cull(in);
    let uv = in.uv * material.scroll.zw + material.scroll.xy * globals.time;
    let texel = textureSample(t_color, s_color, uv);
    let n = normalize(vec3<f32>(
        texel.r * 2.0 - 1.0,
        max(texel.b * 2.0 - 1.0, 0.05),
        texel.g * 2.0 - 1.0,
    ));
    let v = normalize(globals.camera_pos - in.world_pos);
    let dims = max(vec2<f32>(textureDimensions(t_water_scene)), vec2<f32>(1.0));
    let screen_uv = clamp(in.clip_pos.xy / dims, vec2<f32>(0.0), vec2<f32>(1.0));

    let scene_g = textureSample(t_water_scene, s_water_rt, screen_uv).rgb;
    let refracted_g = scene_g * in.color.rgb;

    let w_xz = vec2<f32>(globals.view_proj[0].w, globals.view_proj[2].w);
    let w_len = length(w_xz);
    let fwd_xz = select(vec2<f32>(0.0, -1.0), w_xz / w_len, w_len > 1e-4);
    let right_xz = vec2<f32>(-fwd_xz.y, fwd_xz.x);
    let bump_view = vec2<f32>(dot(n.xz, right_xz), dot(n.xz, fwd_xz));
    let reflect_uv = clamp(
        screen_uv + bump_view * 0.03,
        vec2<f32>(0.001),
        vec2<f32>(0.999),
    );
    let refl_g = textureSample(t_water_reflect, s_water_rt, reflect_uv).rgb;
    let l = normalize(-globals.sun_dir);
    let h = normalize(l + v);
    let spec = pow(max(dot(n, h), 0.0), 8.0) * linear_to_srgb(globals.sun_color) * 0.35;
    let reflected_g = min(refl_g * globals.water_specular.a + spec, vec3<f32>(1.0));

    let limits = globals.speckle_axis.xy;
    let ndotv = clamp(dot(n, v), 0.0, 1.0);
    let f_raw = mix(limits.x, limits.y, (1.0 - ndotv) * (1.0 - ndotv));
    let above = select(0.0, 1.0, globals.camera_pos.y > in.world_pos.y);
    let f = f_raw * above;

    var display_rgb = mix(refracted_g, reflected_g, f);
    let fog = water_fog_amount(in);
    if fog > 0.0 {
        display_rgb = mix(display_rgb, linear_to_srgb(globals.water_fog_color), fog);
    } else if globals.water_fog_begin > 0.0 && in.view_dist > globals.water_fog_begin {
        display_rgb = linear_to_srgb(apply_fog(srgb_to_linear(display_rgb), in.fog_depth));
    }
    return vec4<f32>(display_rgb, far_fade(in.fog_depth));
}

@fragment
fn fs_water_normal_display(in: VsOut) -> @location(0) vec4<f32> {
    return water_normal_color(in);
}

@fragment
fn fs_water_normal(in: VsOut) -> @location(0) vec4<f32> {
    let c = water_normal_color(in);
    return vec4<f32>(srgb_to_linear(c.rgb), c.a);
}
