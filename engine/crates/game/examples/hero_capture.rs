//! Offscreen diagnostic battles for every PS2 BF2 hero class, on a BF2 map.
//! Usage: hero_capture <assets> <map> <out> [class substring]
//! The ordinary member's score is staged to the manager threshold, then the
//! real offer/accept path runs. This is presentation evidence, not earning
//! acceptance. All class physics, weapons, poses and cloth are live.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use anyhow::{anyhow, Context, Result};
use game::{soldier_vis::SoldierVisuals, walk::WalkSim};
use glam::{Mat4, Vec3};
use render::{Camera, Renderer};
use std::path::Path;

#[path = "support/hero_capture_battle.rs"]
mod battle;
#[path = "support/hero_capture_power.rs"]
mod power;

struct Capture<'a> {
    root: &'a Path,
    out: &'a Path,
    renderer: Renderer,
    gpu: game::viewer::SceneGpu,
    scene: game::assets::Scene,
    hud: game::hud_feed::HudView,
    bolts: game::bolt_vis::BoltVis,
    receipts: Vec<serde_json::Value>,
}

impl Capture<'_> {
    fn preview(&mut self, class: &str, walk: &WalkSim, vis: &SoldierVisuals) -> Result<()> {
        let camera = walk.camera();
        let page = ui::spawn_draw::SpawnPage::Class {
            title: class.into(),
            lines: vec![],
            back: "Back".into(),
            accept: "Select Character".into(),
            team: 0,
            class: 1,
            classes: vec![0, 1],
            refused: 0,
        };
        let mut preview = game::spawn_preview::Preview::default();
        let mut display = (vec![], vec![]);
        for _ in 0..120 {
            display = preview.draws_live(
                &mut self.renderer,
                vis,
                Some(&page),
                &camera,
                core::TICK_DT,
                |_, _, _, _, _| None,
            );
        }
        self.renderer.set_particles(vec![]);
        self.renderer.set_bolts(vec![]);
        self.hud.update(
            &ui::hud::HudModel {
                spawn: Some(page),
                ..Default::default()
            },
            core::TICK_DT,
            [1280, 720],
        );
        self.renderer
            .render_frame(
                &camera,
                &display.1,
                &display.0,
                self.gpu.clear,
                Some(&mut |d, q, e, v, s| self.hud.paint(d, q, e, v, s)),
            )
            .map_err(|e| anyhow!("preview: {e}"))?;
        let (rgba, w, h) = self
            .renderer
            .capture_rgba()
            .map_err(|e| anyhow!("preview capture: {e}"))?;
        let path = self.out.join(format!("{class}_preview.png"));
        std::fs::write(
            &path,
            render::encode_png_rgb(&rgba, w, h).map_err(|e| anyhow!("preview png: {e}"))?,
        )?;
        self.receipts.push(serde_json::json!({"class":class,"phase":"preview","intervention":"diagnostic class-page model after120cosmeticupdates","file":path.file_name().unwrap().to_string_lossy()}));
        Ok(())
    }

    fn frame(
        &mut self,
        label: (&str, &str),
        walk: &WalkSim,
        vis: &mut SoldierVisuals,
        fx: &mut game::fx::Fx,
        feed: &game::event_feed::EventFeed,
        front: bool,
    ) -> Result<()> {
        let (class, phase) = label;
        let u = walk
            .sim
            .units
            .get(walk.player.index as usize)
            .filter(|u| u.gen == walk.player.gen)
            .context("player generation")?;
        let (focus, height) = vis
            .drawn_body(walk, walk.player.index as usize)
            .map(|(tris, _)| {
                let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                for p in tris.iter().flatten() {
                    lo = lo.min(*p);
                    hi = hi.max(*p);
                }
                ((lo + hi) * 0.5, (hi.y - lo.y).max(0.1))
            })
            .unwrap_or((u.soldier.pos + Vec3::Y * 0.9, 1.8));
        let offset = if phase.ends_with("_side") {
            Vec3::new(4.4, 0.7, 0.3)
        } else if front {
            Vec3::new(2.0, 0.6, -3.7)
        } else {
            Vec3::new(2.0, 0.7, 3.7)
        };
        let mut eye =
            focus + Mat4::from_rotation_y(u.soldier.yaw).transform_vector3(offset) * (height / 1.8);
        if let Some(hf) = &walk.sim.heightfield {
            eye.y = eye.y.max(hf.height_at(eye.x, eye.z) + 0.2);
        }
        let delta = focus - eye;
        let camera = Camera {
            position: eye,
            yaw: (-delta.x).atan2(-delta.z),
            pitch: delta.y.atan2(Vec3::new(delta.x, 0., delta.z).length()),
            fov_y: 52.06f32.to_radians(),
            ..Default::default()
        };
        self.renderer.time = walk.sim.tick as f32 * core::TICK_DT;
        self.gpu.animate_water(
            &mut self.renderer,
            game::water_anim::clock(f64::from(walk.sim.tick as u32) / 30.0),
            camera.position,
        )?;
        self.gpu.pose_doors(&walk.sim.doors);
        let mut draws = self.gpu.view_draws(&walk.sim, walk.player);
        let body = vis.draws(walk);
        let hilt_draws = vis.saber_hilts(walk).len();
        draws.extend(vis.weapon_draws(walk));
        draws.extend(vis.cloth_draws(&mut self.renderer, walk));
        let mut particles = vis.flash_draws(walk);
        particles.extend(vis.saber_draws(walk));
        let blade_draws = particles.len();
        let trail_sections: Vec<_> = particles
            .iter()
            .filter_map(|p| match &p.shape {
                render::ParticleShape::Trail { sections, .. } => Some(sections.len()),
                _ => None,
            })
            .collect();
        if phase == "death"
            && walk.sim.weapon_classes[u.weapon_class].saber.is_some()
            && (hilt_draws == 0 || blade_draws != 0)
        {
            return Err(anyhow!(
                "{class}: visible hero corpse must retain hilt and extinguish blades"
            ));
        }
        if phase == "death4p20" && hilt_draws != 0 {
            return Err(anyhow!("{class}: expired corpse retained a hilt"));
        }
        fx.submit_with(&mut self.renderer, &camera, particles);
        self.bolts.set_fire_points(vis.fire_points(walk));
        self.bolts.update(walk, core::TICK_DT);
        self.bolts
            .submit(&mut self.renderer, walk, camera.position, &[]);
        let [weapon, weapon2] = game::hud_feed::weapon_rows(walk);
        let model = ui::hud::HudModel {
            team_names: [walk.teams[0].faction.clone(), walk.teams[1].faction.clone()],
            tickets: walk.sim.battle.teams.each_ref().map(|t| t.tickets()),
            hp: u.health.cur,
            hp_max: u.health.max,
            energy: u.soldier.energy,
            energy_max: u.consts.energy_bar,
            hero: walk.sim.is_hero(walk.player),
            hero_offer: walk.sim.hero_offer(0).map(|(c, secs)| {
                let name = feed
                    .class_names(u.team)
                    .get(c as usize)
                    .cloned()
                    .unwrap_or_else(|| walk.teams[u.team as usize].class(c).label.clone());
                (
                    feed.text_by_hash(0xadd5467c, "Play as %s?")
                        .replacen("%s", &name, 1),
                    secs,
                )
            }),
            weapon,
            weapon2,
            reticle: Some(0),
            reticle_y: walk.reticle_y(),
            dead: !u.alive,
            ..Default::default()
        };
        self.hud.update(&model, 1.0 / 30.0, [1280, 720]);
        self.renderer
            .render_frame(
                &camera,
                &draws,
                &body,
                self.gpu.clear,
                Some(&mut |dev, q, enc, view, size| self.hud.paint(dev, q, enc, view, size)),
            )
            .map_err(|e| anyhow!("frame: {e}"))?;
        let (rgba, width, height) = self
            .renderer
            .capture_rgba()
            .map_err(|e| anyhow!("capture: {e}"))?;
        let path = self.out.join(format!("{class}_{phase}.png"));
        std::fs::write(
            &path,
            render::encode_png_rgb(&rgba, width, height).map_err(|e| anyhow!("png: {e}"))?,
        )?;
        let melee = u.weapon.melee.as_ref();
        self.receipts.push(serde_json::json!({
            "class":class,"phase":phase,"tick":walk.sim.tick,"hero":model.hero,"health":[u.health.cur,u.health.max],
            "energy":u.soldier.energy,"position":u.soldier.pos.to_array(),"camera":[camera.position.to_array(),[camera.yaw,camera.pitch,camera.fov_y]],
            "alive":u.alive,"generation":u.gen,"body_draws":body.len(),"hilt_draws":hilt_draws,"blade_particles":blade_draws,
            "trail_sections":trail_sections,
            "last_hit":u.last_hit.map(|(h,t)|(format!("{h:?}"),t)),
            "lightning_bolts":fx.force_bolts(),"force_damage_effects":fx.force_damage_effects(),
            "combo_animation":melee.and_then(|m|m.animation.as_deref()),"hidden_edges":melee.map(|m|m.hidden_edges),
            "deflect":melee.and_then(|m|m.deflect).map(|d|format!("{d:?}")),
            "primary":walk.sim.weapon_classes[u.weapon_class].name,
            "flying_saber":std::iter::once(&u.weapon).chain(u.held.iter().map(|h|&h.state)).any(|s|s.saber_flight.is_some()),
            "secondary":u.cur1.and_then(|k|u.held.get(k as usize)).map(|h|walk.sim.weapon_classes[h.class].name.clone()),
            "targets":walk.sim.units.iter().enumerate().filter(|(_,t)|t.team!=u.team).map(|(i,t)|serde_json::json!({"unit":i,"health":t.health.cur,"alive":t.alive,"position":t.soldier.pos.to_array()})).collect::<Vec<_>>(),
            "events":walk.sim.events.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>(),"file":path.file_name().unwrap().to_string_lossy()
        }));
        std::fs::write(
            self.out.join("frames.json"),
            serde_json::to_vec_pretty(&self.receipts)?,
        )?;
        println!(
            "{}: {} body draws, {} blade/flash particles, health {}/{}",
            path.display(),
            body.len(),
            blade_draws,
            u.health.cur,
            u.health.max
        );
        Ok(())
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err(anyhow!(
            "usage: hero_capture <assets> <map> <out> [class substring]"
        ));
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    std::fs::create_dir_all(out)?;
    game::assets::set_texture_tier(game::assets::TextureTier::Ps2);
    let (scene, load) = game::assets::load_battle_scene(&root.join(&args[2]), "con")?;
    if scene.manifest.source_game != "bf2" {
        return Err(anyhow!("hero capture requires BF2 map"));
    }
    if let Some((_, Err(e))) = load.common {
        return Err(e);
    }
    let mut renderer = Renderer::new_headless(1280, 720).map_err(|e| anyhow!("{e}"))?;
    let gpu = game::viewer::upload_scene(&mut renderer, &scene)?;
    let hud = game::hud_feed::HudView::for_renderer(root, &renderer)?;
    let mut capture = Capture {
        root,
        out,
        renderer,
        gpu,
        scene,
        hud,
        bolts: game::bolt_vis::BoltVis::new(root, "bf2"),
        receipts: vec![],
    };
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &game::roster::FACTION_ORDER)?;
    let mut names: Vec<_> = db
        .names()
        .filter(|n| {
            n.split('_').nth(1) == Some("hero")
                && ["all", "imp", "rep", "cis"].contains(&n.split('_').next().unwrap_or(""))
                && args.get(4).is_none_or(|s| n.contains(s))
        })
        .collect();
    names.sort();
    for name in &names {
        capture.hero(name, &db).with_context(|| name.to_string())?;
    }
    std::fs::write(
        out.join("captures.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"intervention":"ordinary score cache set to manager threshold before offer; no natural earning claim","map":args[2],"heroes":names,"frames":capture.receipts}),
        )?,
    )?;
    Ok(())
}
