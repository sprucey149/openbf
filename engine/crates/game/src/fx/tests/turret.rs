//! Issue #91: original BF2 class bands and baked emitters, without a window.
use super::*;
use core::autoturret::Turret;
use core::unit::UnitHandle;

#[test]
fn portable_damage_effects_follow_health_bands_repair_and_death() {
    let root = crate::assets::baked_root();
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep"])
        .expect("BF2 PS2 class data");
    let weapon = crate::roster::weapon_from_db(&db, "rep_weap_inf_autoturret_dispenser").unwrap();
    let class = &weapon.turret.as_ref().unwrap().item;
    assert_eq!(
        class.damage_effects.len(),
        2,
        "properties after TURRET1 retained"
    );
    assert_eq!(class.damage_effects[0].effect, "com_sfx_vehiclespark");
    assert_eq!(class.damage_effects[1].effect, "com_sfx_smokeplume");
    assert!((class.damage_effects[0].start - 0.5).abs() < 1e-6);
    assert!((class.damage_effects[0].stop - 0.3).abs() < 1e-6);
    assert!((class.damage_effects[1].start - 0.3).abs() < 1e-6);
    let mut sim = Sim::empty(1);
    let wi = sim.add_weapon_class(weapon);
    let c = &sim.weapon_classes[wi].turret.as_ref().unwrap().item;
    let mut t = Turret::new(
        1,
        UnitHandle { index: 0, gen: 0 },
        0,
        wi,
        c,
        Vec3::Y,
        Vec3::ZERO,
        0.0,
    );
    t.land(Vec3::new(3.0, 0.0, 4.0), Vec3::Y);
    sim.turrets.push(t);
    let lib = Library::load(&root, &root.join("common_bf2"), "bf2", &["rep"]);
    for name in ["com_sfx_vehiclespark", "com_sfx_smokeplume"] {
        assert!(lib.get(name).is_some(), "{name} actually baked");
    }
    let mut fx = Fx::with_library(lib);
    let mut renderer = render::Renderer::new_headless(256, 256).expect("headless GPU");
    let active = |fx: &Fx| {
        fx.live
            .iter()
            .filter(|e| e.turret.is_some())
            .map(|e| e.name().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    fx.ingest_sim(&sim);
    assert!(active(&fx).is_empty());
    for (hp, name) in [(25.0, "com_sfx_vehiclespark"), (14.0, "com_sfx_smokeplume")] {
        sim.turrets[0].health = hp;
        fx.ingest_sim(&sim);
        fx.ingest_sim(&sim);
        assert_eq!(active(&fx), [name]);
        sim.turrets[0].pos.x += 2.0;
        fx.ingest_sim(&sim);
        fx.update(0.5);
        let e = fx.live.iter().find(|e| e.turret.is_some()).unwrap();
        assert_eq!(e.frame.pos, sim.turrets[0].pos);
        assert!(e.emitted().iter().sum::<u32>() > 0, "authored emitter ran");
        let camera = render::Camera {
            position: sim.turrets[0].pos + Vec3::new(0.0, 0.6, 3.0),
            pitch: -0.1,
            ..Default::default()
        };
        let clear = [0.08, 0.08, 0.08, 1.0];
        renderer.render(&camera, &[], clear).unwrap();
        let (blank, _, _) = renderer.capture_rgba().unwrap();
        fx.submit(&mut renderer, &camera);
        renderer.render(&camera, &[], clear).unwrap();
        let (rgba, w, h) = renderer.capture_rgba().unwrap();
        assert!(
            rgba.chunks_exact(4)
                .zip(blank.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
                > 5,
            "{name} reached frame"
        );
        if let Some(dir) = std::env::var_os("OPENBF_TURRET_CAPTURE") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("{name}.png")),
                render::encode_png(&rgba, w, h).unwrap(),
            )
            .unwrap();
        }
    }
    sim.turrets[0].health = 50.0;
    fx.ingest_sim(&sim);
    assert!(active(&fx).is_empty(), "repair stops damage effects");
    sim.turrets[0].health = 10.0;
    fx.ingest_sim(&sim);
    assert_eq!(active(&fx), ["com_sfx_smokeplume"]);
    sim.turrets.clear();
    fx.ingest_sim(&sim);
    assert!(
        active(&fx).is_empty(),
        "expired/deleted turret stops emission"
    );
    assert!(fx.live.iter().all(|e| e.is_stopped()));
}
