use anyhow::{anyhow, Result};
use core::sim::{buttons, PadState};
use game::{class_picker::ClassPicker, walk::WalkSim};
use glam::Vec3;
use std::path::Path;

fn main() -> Result<()> {
    let root = Path::new("assets_baked");
    let (scene, _) = game::assets::load_battle_scene(&root.join("bf1_bes2"), "con")?;
    let mut walk = WalkSim::new(198, Vec3::new(-241.0, -33.1, 186.6), scene.heightfield());
    scene.install_collision(&mut walk.sim);
    let db = game::roster::load_classdb(&root.join("classes"), "bf1", &["all"])?;
    let base = game::roster::ClassRoster::build(&db, "all_inf_soldier", 0, None)?
        .register(&mut walk.sim, &mut vec![]);
    walk.sim.classes = [vec![base.clone()], vec![base]];
    let mut picker = ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    let choice = picker
        .choices
        .iter()
        .position(|c| c.class.class == "rep_hero_anakin")
        .ok_or_else(|| anyhow!("hero"))?;
    assert!(picker.select(&mut walk, choice));
    let player = walk.player;
    let mut receipts = vec![];
    for (x, z) in [
        (-241.0, 186.6),
        (-242.0, 186.6),
        (-240.0, 186.6),
        (-224.0, 187.0),
        (-225.0, 190.0),
    ] {
        let start = Vec3::new(x, -33.1, z);
        walk.sim.unit_mut(player).unwrap().soldier = core::soldier::Soldier::new(start);
        let mut contacts = 0;
        let mut crossing = None;
        for tick in 0..300 {
            let from = walk.sim.unit(player).unwrap().capsule();
            walk.sim.unit_mut(player).unwrap().soldier.energy = 100.0;
            let pad = PadState {
                buttons: if tick % 6 == 0 { buttons::JUMP } else { 0 },
                ..Default::default()
            };
            walk.sim.step(&[pad]);
            let u = walk.sim.unit(player).unwrap();
            let delta = u.soldier.pos - from.base;
            if crossing.is_none() {
                if let Some((time, normal)) = walk.sim.collision.ceiling_sweep(&from, &u.capsule())
                {
                    if time < 0.999 {
                        crossing = Some(
                            serde_json::json!({"tick":tick,"global_tick":walk.sim.tick,
                            "from":from.base.to_array(),"to":u.soldier.pos.to_array(),"time":time,"normal":normal.to_array(),
                            "velocity":u.soldier.vel.to_array()}),
                        );
                    }
                }
            }
            if delta.y > 0.0 {
                let probe = core::collision::Capsule {
                    base: from.base + Vec3::Y * 1.0,
                    ..from
                };
                if let Some((t, n)) = walk.sim.collision.ceiling_sweep(&from, &probe) {
                    contacts += 1;
                    if contacts == 1 {
                        let mut bodies = vec![];
                        for (i, b) in walk.sim.collision.bodies().iter().enumerate() {
                            let mut body = core::collision::Body::new(b.to_world, b.shape.clone());
                            body.layers = b.layers;
                            let world = core::collision::CollisionWorld::new(vec![body], vec![]);
                            if world.ceiling_sweep(&from, &probe).is_some() {
                                let instance = b
                                    .instance
                                    .and_then(|k| scene.manifest.instances.get(k as usize));
                                bodies.push(serde_json::json!({"body":i,"instance":b.instance,"class":instance.map(|i|&i.class),"name":instance.map(|i|&i.name)}));
                            }
                        }
                        receipts.push(serde_json::json!({"spot":[x,z],"tick":tick,"global_tick":walk.sim.tick,
                            "from":from.base.to_array(),"to":u.soldier.pos.to_array(),"head":u.soldier.pos.y+u.capsule().height,
                            "velocity":u.soldier.vel.to_array(),"probe_t":t,"normal":n.to_array(),"bodies":bodies}));
                    }
                }
            }
        }
        receipts.push(serde_json::json!({"spot":[x,z],"contacts":contacts,"crossing":crossing,"final":walk.sim.unit(player).unwrap().soldier.pos.to_array()}));
    }
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "artifacts/hero-behavior/ceiling-map.json".into());
    std::fs::write(out, serde_json::to_vec_pretty(&receipts)?)?;
    println!("{}", serde_json::to_string_pretty(&receipts)?);
    Ok(())
}
