//! Contact sheet of a baked soldier posed by its clips (headless):
//! `cargo run -p game --example pose_preview -- <soldier.glb> <out.png> [clip:frame ...]`
//!
//! Flags (after the two paths): `--weapon <glb>` draws a weapon at
//! hp_weapons, `--cell <px>` sets the per-pose cell width (default 256),
//! `--dist <m>` the camera distance (default 4.5) — shrink it to inspect
//! grip/orientation — and `--yaw <deg>` the model's yaw (default 180, facing
//! the camera; 90 shows the profile).

use glam::{Mat4, Vec3};
use render::{Camera, Renderer, SkinnedDrawCmd};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let glb = std::path::Path::new(&args[1]);
    let out = &args[2];
    // Optional weapon GLB drawn at hp_weapons: `--weapon <glb>` anywhere.
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let weapon_path = flag("--weapon");
    let cell: u32 = flag("--cell").and_then(|v| v.parse().ok()).unwrap_or(256);
    let dist: f32 = flag("--dist").and_then(|v| v.parse().ok()).unwrap_or(4.5);
    // Model yaw in degrees; 180 (default) turns the +Z-forward soldier to face
    // the camera, 90 shows the profile (weapon-orientation checks).
    let yaw: f32 = flag("--yaw").and_then(|v| v.parse().ok()).unwrap_or(180.0);
    let args: Vec<String> = {
        let mut v = Vec::new();
        let mut skip = false;
        for a in &args {
            if skip {
                skip = false;
                continue;
            }
            if a == "--weapon" || a == "--cell" || a == "--dist" || a == "--yaw" {
                skip = true;
                continue;
            }
            v.push(a.clone());
        }
        v
    };
    let picks: Vec<(String, f32)> = args[3..]
        .iter()
        .map(|s| {
            let (c, f) = s.rsplit_once(':').unwrap_or((s, "0"));
            (c.to_string(), f.parse().unwrap_or(0.0))
        })
        .collect();
    let asset = game::assets::load_skinned_glb(glb)?;
    // Textures live beside the model: <side>/textures/<material>.ktx2 (with
    // their `.dtl.ktx2` detail companions). Missing ones fall back to flat.
    let tex_dir = glb
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("textures"));
    let (w, h) = (cell * picks.len().max(1) as u32, cell * 5 / 4);
    let mut r = Renderer::new_headless(w, h)?;
    let mut cache: std::collections::HashMap<String, Option<render::TextureHandle>> =
        std::collections::HashMap::new();
    let mut load_tex = |r: &mut Renderer, mat: Option<&str>| -> Option<render::TextureHandle> {
        let (name, dir) = (mat?.to_ascii_lowercase(), tex_dir.as_ref()?);
        if !cache.contains_key(&name) {
            let h = game::viewer::upload_texture_file(r, dir, &name).unwrap_or(None);
            cache.insert(name.clone(), h);
        }
        cache[&name]
    };
    let meshes: Vec<_> = game::viewer::upload_skinned(&mut r, &asset.mesh)?
        .into_iter()
        .map(|(m, mat, layer)| {
            let t = load_tex(&mut r, mat);
            (m, t, layer)
        })
        .collect();
    // Camera `dist` in front at chest height; the model faces +Z, so `--yaw`
    // (default 180) turns it around to face the camera (which looks down -Z).
    let cam = Camera {
        position: Vec3::new(0.0, 1.0, 0.0),
        ..Camera::default()
    };
    let weapon = match &weapon_path {
        Some(wp) => {
            let cpu = game::assets::load_glb(std::path::Path::new(wp))?;
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &cpu.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            println!(
                "weapon {wp}: {} verts, extents {lo:?}..{hi:?}",
                cpu.positions.len()
            );
            game::viewer::upload_submeshes(&mut r, &cpu)?
                .into_iter()
                .map(|(m, mat, _, _)| {
                    let t = load_tex(&mut r, mat);
                    (m, t)
                })
                .collect::<Vec<_>>()
        }
        None => Vec::new(),
    };
    let hp = asset.skeleton.index_of("hp_weapons");
    println!("hp_weapons index {hp:?}");
    let mut statics = Vec::new();
    let mut skinned = Vec::new();
    let n = picks.len().max(1) as f32;
    for (i, (clip_name, frame)) in picks.iter().enumerate() {
        let pose = match game::animator::find_clip(&asset.clips, clip_name) {
            Some(i) => asset.clips[i].sample(&asset.skeleton, *frame, true),
            None => {
                eprintln!("clip {clip_name} not baked; bind pose");
                game::skin::Pose::bind(&asset.skeleton)
            }
        };
        let palette = asset.skeleton.palette(&pose.locals());
        let x = (i as f32 + 0.5 - n / 2.0) * dist * 0.36;
        let model = Mat4::from_translation(Vec3::new(x, 0.0, -dist))
            * Mat4::from_rotation_y(yaw.to_radians());
        if let Some(hp) = hp {
            let global = palette[hp] * asset.skeleton.global_bind[hp];
            println!(
                "{clip_name}:{frame} hp_weapons at {:?} (model space)",
                global.transform_point3(Vec3::ZERO)
            );
            for (m, t) in &weapon {
                let mut cmd = render::DrawCmd::flat(*m, [1.0, 1.0, 1.0, 1.0], model * global);
                match t {
                    Some(t) => cmd.texture = Some(*t),
                    // Untextured weapon: red, so the grip is still readable.
                    None => cmd.color = [0.9, 0.2, 0.2, 1.0],
                }
                statics.push(cmd);
            }
        }
        for (mesh, tex, layer) in &meshes {
            skinned.push(SkinnedDrawCmd {
                mesh: *mesh,
                texture: *tex,
                color: if tex.is_some() {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.85, 0.85, 0.9, 1.0]
                },
                model,
                palette: palette.clone().into(),
                layer: *layer,
                light: render::LightPath::Character,
                lit: None,
            });
        }
    }
    r.render_frame(&cam, &statics, &skinned, [0.2, 0.3, 0.5, 1.0], None)?;
    let (rgba, w, h) = r.capture_rgba()?;
    std::fs::write(out, render::encode_png(&rgba, w, h)?)?;
    println!(
        "wrote {out} ({w}x{h}, {} clips: {:?})",
        asset.clips.len(),
        asset.clips.iter().map(|c| c.frames).collect::<Vec<_>>()
    );
    Ok(())
}
