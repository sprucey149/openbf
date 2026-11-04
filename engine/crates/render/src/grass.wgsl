
struct GrassIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) up: f32,
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) inst_color: vec4<f32>,
    @location(9) params: vec4<f32>,
};

struct GrassOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) view_dist: f32,
    @location(3) fog_depth: f32,
    @location(4) light: vec3<f32>,
    @location(5) @interpolate(flat) params: vec4<f32>,
};

@vertex
fn vs_grass(in: GrassIn) -> GrassOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let world = model * vec4<f32>(in.position, 1.0);
    var out: GrassOut;
    out.clip_pos = globals.view_proj * world;
    out.uv = in.uv;
    out.color = in.color * in.inst_color;
    out.view_dist = distance(world.xyz, globals.camera_pos);
    out.fog_depth = out.clip_pos.w;
    out.params = in.params;
    out.light = vec3<f32>(1.0);
    // A BF2 leaf (`leaf.rs`, PROVISIONAL): the static `Lit` set on the
    // quad's ±up normal through its object's matrix, per vertex as the VU.
    if in.params.x > 0.5 {
        let n = normalize((model * vec4<f32>(0.0, in.up, 0.0, 0.0)).xyz);
        let ambient = mix(globals.ambient_ground, globals.ambient_sky, n.y * 0.5 + 0.5);
        let sun = globals.sun_color * max(dot(n, -globals.sun_dir), 0.0);
        let light2 = globals.light2_color * max(dot(n, -normalize(globals.light2_dir)), 0.0);
        out.light = ambient + sun;
        if any(globals.light2_color != vec3<f32>(0.0)) {
            out.light = ambient + sun + light2;
        }
    } else if in.params.y < 0.5 {
        let n = (model * vec4<f32>(0.0, 127.0 / 128.0, 0.0, 0.0)).xyz;
        let ambient = mix(
            linear_to_srgb(globals.ambient_ground),
            linear_to_srgb(globals.ambient_sky),
            n.y * 0.5 + 0.5,
        );
        var ndotl = max(dot(n, -globals.sun_dir), 0.0);
        if globals.sun_d4 > 0.5 {
            let d = (1.0 + dot(n, -globals.sun_dir)) * 0.5;
            ndotl = d * d * d * d;
        }
        var light = ambient + linear_to_srgb(globals.sun_color) * ndotl;
        if any(globals.light2_color != vec3<f32>(0.0)) {
            light += linear_to_srgb(globals.light2_color)
                * max(dot(n, -normalize(globals.light2_dir)), 0.0);
        }
        out.light = clamp(light, vec3<f32>(0.0), vec3<f32>(GS_MAX));
    }
    return out;
}

@fragment
fn fs_grass(in: GrassOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_color_display, s_color, in.uv);
    var a = texel.a * in.color.a * far_fade(in.view_dist);
    if in.params.y > 0.5 {
        a = texel.a * in.color.a;
    }
    if a <= 0.0 {
        discard;
    }
    var rgb = min(srgb_to_linear(texel.rgb) * srgb_to_linear(min(in.color.rgb, vec3<f32>(1.0))), vec3<f32>(1.0));
    if in.params.x > 0.5 {
        rgb = rgb * in.light;
    } else if in.params.y < 0.5 {
        rgb = min(rgb * srgb_to_linear(in.light), vec3<f32>(1.0));
    }
    return vec4<f32>(apply_fog(rgb, in.fog_depth), a);
}

@fragment
fn fs_grass_display(in: GrassOut) -> @location(0) vec4<f32> {
    let texel = textureSample(t_color_display, s_color, in.uv);
    var a = texel.a * in.color.a * far_fade(in.view_dist);
    if in.params.y > 0.5 {
        a = texel.a * in.color.a;
    }
    if a <= 0.0 {
        discard;
    }
    var rgb = min(texel.rgb * min(in.color.rgb, vec3<f32>(1.0)), vec3<f32>(1.0));
    if in.params.x > 0.5 {
        rgb = linear_to_srgb(srgb_to_linear(rgb) * in.light);
    } else if in.params.y < 0.5 {
        rgb = min(rgb * in.light, vec3<f32>(1.0));
    }
    if globals.fog_on > 0.5 {
        rgb = mix(rgb, linear_to_srgb(globals.fog_color), fog_amount(in.fog_depth));
    }
    return vec4<f32>(rgb, a);
}
