// ── Laser bolts (`bolt.rs`, card FXB) ───────────────────────────────────────
// Appended after `grass.wgsl`: the bolts ride `vs_grass` with world-space
// vertices and one identity instance. GS MODULATE with TCC: colour =
// texel × the vertex colour (1.0 = 0x80), alpha = texel alpha × the vertex
// alpha; unlit and NEVER fogged (PRIM without FGE, re-bolt §3.3). The core
// composites source-over (ALPHA 0x44); the glow adds Cs·As (0x48), written
// premultiplied for the (One, One) blend.

fn bolt_texel(in: GrassOut) -> vec4<f32> {
    let texel = textureSample(t_color, s_color, in.uv);
    let rgb = min(texel.rgb * srgb_to_linear(in.color.rgb), vec3<f32>(1.0));
    return vec4<f32>(rgb, texel.a * in.color.a);
}

@fragment
fn fs_bolt_core(in: GrassOut) -> @location(0) vec4<f32> {
    let c = bolt_texel(in);
    if c.a <= 0.0 {
        discard;
    }
    return c;
}

@fragment
fn fs_bolt_glow(in: GrassOut) -> @location(0) vec4<f32> {
    let c = bolt_texel(in);
    if c.a <= 0.0 {
        discard;
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}

@fragment
fn fs_bloom_particle(in: GrassOut) -> @location(0) vec4<f32> {
    let c = bolt_texel(in);
    return vec4<f32>(linear_to_srgb(c.rgb) * c.a * 0.5, 1.0);
}

@fragment
fn fs_leaf_particle(in: GrassOut) -> @location(0) vec4<f32> {
    let c = bolt_texel(in);
    if c.a <= 0.25 {
        discard;
    }
    return c;
}

@fragment
fn fs_bolt_solid(in: GrassOut) -> @location(0) vec4<f32> {
    return vec4<f32>(bolt_texel(in).rgb, 1.0);
}

fn display_texel(in: GrassOut) -> vec4<f32> {
    let texel = textureSample(t_color, s_color, in.uv);
    let rgb = linear_to_srgb(texel.rgb) * in.color.rgb;
    return vec4<f32>(rgb, texel.a * in.color.a);
}

@fragment
fn fs_bolt_core_display(in: GrassOut) -> @location(0) vec4<f32> {
    let c = display_texel(in);
    if c.a <= 0.0 {
        discard;
    }
    return c;
}

@fragment
fn fs_bolt_glow_display(in: GrassOut) -> @location(0) vec4<f32> {
    let c = display_texel(in);
    if c.a <= 0.0 {
        discard;
    }
    return vec4<f32>(c.rgb * c.a, c.a);
}

@fragment
fn fs_leaf_particle_display(in: GrassOut) -> @location(0) vec4<f32> {
    let c = display_texel(in);
    if c.a <= 0.25 {
        discard;
    }
    return c;
}

@fragment
fn fs_bolt_solid_display(in: GrassOut) -> @location(0) vec4<f32> {
    return vec4<f32>(display_texel(in).rgb, 1.0);
}
