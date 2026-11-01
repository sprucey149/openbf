use anyhow::{anyhow, Result};
use core::sim::{buttons, PadState};
use game::{class_picker::ClassPicker, soldier_vis::SoldierVisuals, walk::WalkSim};
use glam::Vec3;
use render::{Camera, Renderer};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    let hero = &args[4];
    let fps: u32 = args[5].parse()?;
    std::fs::create_dir_all(out)?;
    let (scene, _) = game::assets::load_battle_scene(&root.join(&args[2]), "con")?;
    let mut walk = WalkSim::new(198, Vec3::ZERO, None);
    walk.blend.on = true;
    walk.sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let db = game::roster::load_classdb(&root.join("classes"), "bf1", &["all"])?;
    let base = game::roster::ClassRoster::build(&db, "all_inf_soldier", 0, None)?
        .register(&mut walk.sim, &mut Vec::new());
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let mut picker = ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    let choice = picker
        .choices
        .iter()
        .position(|c| c.class.class == *hero)
        .ok_or_else(|| anyhow!("missing {hero}"))?;
    let mut renderer = Renderer::new_headless(640, 480).map_err(|e| anyhow!("{e}"))?;
    let mut vis = SoldierVisuals::for_walk(&mut renderer, root, &scene, &walk);
    assert!(picker.select(&mut walk, choice));
    for request in &picker.pending_assets {
        vis.append_test_class(&mut renderer, root, &walk, request);
    }
    let cases = [
        ("stand_attack", 0, 0, buttons::FIRE),
        ("forward_attack", 0, 32767, buttons::FIRE),
        ("backward_attack", 0, -32767, buttons::FIRE),
        ("left_attack", -32767, 0, buttons::FIRE),
        ("right_attack", 32767, 0, buttons::FIRE),
        ("block", 0, 0, buttons::SWITCH_WEAPON),
        ("forward_block", 0, 32767, buttons::SWITCH_WEAPON),
        ("left_block", -32767, 0, buttons::SWITCH_WEAPON),
        ("jump_forward", 0, 32767, buttons::JUMP),
        ("jump_backward", 0, -32767, buttons::JUMP),
        ("jump_left", -32767, 0, buttons::JUMP),
        ("jump_right", 32767, 0, buttons::JUMP),
        ("jump_stationary", 0, 0, buttons::JUMP),
    ];
    let mut rows = Vec::new();
    for (name, lx, ly, action) in cases {
        let h = walk.player;
        {
            let u = walk.sim.unit_mut(h).unwrap();
            u.soldier = core::soldier::Soldier::new(Vec3::ZERO);
            u.soldier.energy = u.consts.energy_bar;
            u.soldier.fuel = core::soldier::Soldier::spawn_fuel(&u.consts);
            u.weapon.melee = None;
        }
        vis.units[h.index as usize].animator = game::animator::Animator::new();
        let mut last_pose: Option<Vec<Vec3>> = None;
        let start = walk.sim.tick;
        let mut frame = 0;
        while walk.sim.tick < start + 60 {
            let tick = walk.sim.tick - start;
            let pad = PadState {
                lx,
                ly,
                buttons: if tick == 6
                    || (action == buttons::JUMP && tick == 15)
                    || (action == buttons::SWITCH_WEAPON && tick == 36)
                {
                    action
                } else {
                    0
                },
                ..Default::default()
            };
            walk.advance_with(1.0 / fps as f32, &[pad], |w| vis.note_shots(w));
            walk.blend_in();
            vis.update(&walk, 1.0 / fps as f32);
            let u = walk.sim.unit(walk.player).unwrap();
            let pos = u.soldier.pos;
            let mesh = vis
                .drawn_body(&walk, walk.player.index as usize)
                .map(|(tris, _)| {
                    tris.into_iter()
                        .flatten()
                        .map(|p| p - pos)
                        .collect::<Vec<_>>()
                });
            assert!(mesh.is_some(), "capture must include the selected hero");
            let delta = mesh.as_ref().zip(last_pose.as_ref()).map(|(a, b)| {
                a.iter()
                    .zip(b)
                    .map(|(a, b)| a.distance(*b))
                    .fold(0.0f32, f32::max)
            });
            last_pose = mesh;
            rows.push(
                serde_json::json!({"case":name,"frame":frame,"tick":walk.sim.tick,
                "pos":pos.to_array(),"stance":format!("{:?}",u.soldier.stance),
                "jump":u.soldier.jump_animation,"jump_serial":u.soldier.jump_animation_serial,
                "body":format!("{:?}",vis.units[walk.player.index as usize].animator.body()),
                "pose":u.weapon.melee.as_ref().and_then(|m|m.pose()),"mesh_delta":delta}),
            );
            if frame % (fps / 15).max(1) == 0 {
                let eye = pos + Vec3::new(3.0, 1.6, 4.0);
                let d = pos + Vec3::Y - eye;
                let camera = Camera {
                    position: eye,
                    yaw: (-d.x).atan2(-d.z),
                    pitch: d.y.atan2(Vec3::new(d.x, 0.0, d.z).length()),
                    ..Default::default()
                };
                let body = vis.draws(&walk);
                let weapons = vis.weapon_draws(&walk);
                renderer.set_particles(vis.saber_draws(&walk));
                renderer
                    .render_frame(&camera, &weapons, &body, [0.13, 0.15, 0.18, 1.0], None)
                    .map_err(|e| anyhow!("{e}"))?;
                let (rgba, w, h) = renderer.capture_rgba().map_err(|e| anyhow!("{e}"))?;
                std::fs::write(
                    out.join(format!("{name}_{frame:04}.png")),
                    render::encode_png_rgb(&rgba, w, h).map_err(|e| anyhow!("{e}"))?,
                )?;
            }
            walk.blend_out();
            frame += 1;
        }
    }
    std::fs::write(
        out.join("frames.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
        "build":env!("OPENBF_BUILD_COMMIT"),"dirty":env!("OPENBF_BUILD_DIRTY"),"map":args[2],"hero":hero,"fps":fps,
        "intervention":"existing ClassPicker::select; flat diagnostic ground; reset actor per action",
        "hash":format!("{:016x}",walk.sim.hash()),"frames":rows}))?,
    )?;
    Ok(())
}
