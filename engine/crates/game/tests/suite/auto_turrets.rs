use core::autoturret::TurretEnd;
use core::heightfield::Heightfield;
use core::sim::{buttons, Event, PadState, Sim};
use core::unit::{Controller, UnitSpec};
use core::weapon::{HealthType, WeaponClass, DEG};
use game::roster::{load_classdb, ClassRoster};
use glam::Vec3;

fn flat() -> Heightfield {
    Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    }
}

fn check_disc_values(name: &str, w: &WeaponClass) {
    let d = w.turret.as_ref().unwrap();
    assert_eq!(d.max_items, 1, "{name}");
    assert_eq!((d.strength, d.velocity), ([0.3, 0.4], 10.0), "{name}");
    assert_eq!(w.reload_time, 2.0, "{name}");
    let c = &d.item;
    let b = c.body;
    assert_eq!((b.max_health, b.lifetime), (50.0, 60.0), "{name}");
    assert_eq!((b.turn_speed, b.pitch_speed), (3.8, 3.8), "{name}");
    assert!((b.pitch_limits[1] - 35.0 * DEG).abs() < 1e-5, "{name}");
    assert!(b.wraps(), "{name}");
    assert_eq!(c.health_type, HealthType::Building, "{name}");
    assert_eq!(c.geometry.as_deref(), Some("com_weap_inf_dropturret"));
    let e = c.explosion.as_ref().expect("turret explosion");
    assert_eq!(e.damage, 50.0, "{name}");
    let g = &c.gun;
    assert_eq!(g.ai_range[2], 40.0, "{name}");
    assert_eq!((g.shot_delay, g.salvo_count), ([1.7, 1.7], 4), "{name}");
    assert_eq!(
        (
            g.ordnance.velocity,
            g.ordnance.life_span,
            g.ordnance.max_damage
        ),
        (100.0, 0.8, 100.0),
        "{name}"
    );
}

fn deploy_and_fight(name: &str, roster: &ClassRoster, slot: u8) {
    let mut sim = Sim::empty(42);
    sim.heightfield = Some(flat());
    let class = roster.register(&mut sim, &mut Vec::new());
    let owner = sim
        .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
        .unwrap();
    let u = &mut sim.units[owner.index as usize];
    for h in &mut u.held {
        h.state.active = h.slot == slot;
    }
    u.cur1 = Some(u.held.iter().position(|h| h.slot == slot).unwrap() as u8);
    let pad = |buttons: u32| PadState {
        buttons,
        ..Default::default()
    };
    sim.step(&[pad(buttons::SECONDARY_FIRE)]);
    for _ in 0..120 {
        if !sim.turrets.is_empty() {
            break;
        }
        sim.step(&[pad(0)]);
    }
    assert_eq!(sim.turrets.len(), 1, "{name}: placed");
    let placed = sim.tick;
    for _ in 0..3 * core::TICK_HZ {
        sim.step(&[pad(0)]);
    }
    let t = &sim.turrets[0];
    assert!(t.landed && t.pos.y.abs() < 0.05, "{name}: {:?}", t.pos);
    let gun = sim.turret_class(t).unwrap().gun_index;
    let centre = t.centre();
    assert!(sim.is_item_gun(gun), "{name}: its gun is an item's");
    let at = t.pos.with_y(0.0) - Vec3::Z * 20.0;
    sim.spawn(&UnitSpec::trooper(1, at)).unwrap();
    let (mut shots, mut end) = (0, None);
    for _ in 0..62 * core::TICK_HZ {
        sim.step(&[pad(0)]);
        for e in &sim.events {
            if let Event::Shot {
                weapon_class,
                origin,
                ..
            } = e
            {
                if *weapon_class as usize == gun {
                    assert!(origin.distance(centre) < 0.5, "{name}: shot from {origin}");
                    shots += 1;
                }
            }
        }
        end = end.or(sim.events.iter().find_map(|e| match e {
            Event::TurretGone { end, .. } => Some((*end, sim.tick)),
            _ => None,
        }));
        if end.is_some() {
            break;
        }
    }
    assert!(shots >= 4, "{name}: {shots} shots");
    let (end, tick) = end.unwrap_or_else(|| panic!("{name}: never expired"));
    assert_eq!(end, TurretEnd::Expired, "{name}");
    let secs = (tick - placed) as f32 / core::TICK_HZ as f32;
    assert!((secs - 60.0).abs() < 0.2, "{name}: {secs} s");
}

#[test]
fn every_bf2_sniper_deploys_the_discs_auto_turret() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires the local baked BF2 disc");
        return;
    }
    let db = load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"]).unwrap();
    // The turret's model and sheet come from a bake made with card ATUR's
    // bake changes (`scripts/rebake.sh common side`); an older bake skips
    // only these checks.
    if root
        .join("side_bf2/weapons/com_weap_inf_dropturret.glb")
        .exists()
    {
        for dir in ["common_bf2/models", "side_bf2/weapons"] {
            let model = root.join(dir).join("com_weap_inf_dropturret.glb");
            let cpu = game::assets::load_glb(&model).unwrap();
            assert!(!cpu.indices.is_empty(), "{dir}");
            let band = (0..cpu.submeshes.len()).map(|k| cpu.uv_scroll(k));
            assert!(
                band.into_iter().any(|s| s == [-200.0 / 255.0, 0.0]),
                "{dir}: no band scroll (rebake common and side)"
            );
        }
        assert!(root
            .join("side_bf2/textures/com_prop_dropturret_rep.ktx2")
            .exists());
    } else {
        eprintln!("SKIP (model checks): no side_bf2 com_weap_inf_dropturret bake");
    }
    let mut names: Vec<String> = db
        .names()
        .filter(|n| game::roster::is_soldier_chain(&db, n))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    let mut found = Vec::new();
    for name in &names {
        let Ok(roster) = ClassRoster::build(&db, name, 1, None) else {
            continue;
        };
        let Some(held) = roster.held.iter().find(|h| h.weapon.turret.is_some()) else {
            continue;
        };
        found.push(name.as_str());
        assert_eq!((held.slot, held.channel, held.clips), (3, 1, 1), "{name}");
        check_disc_values(name, &held.weapon);
        deploy_and_fight(name, &roster, held.slot);
    }
    for side in ["all", "cis", "imp", "rep"] {
        let default = format!("{side}_inf_default_sniper");
        assert!(found.contains(&default.as_str()), "{default}");
    }
    assert_eq!(found.len(), 14, "{found:?}");
}
