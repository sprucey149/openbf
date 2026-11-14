use super::*;

#[test]
fn actual_hero_force_looks_follow_the_referenced_ordnance_chain() {
    let root = crate::assets::baked_root();
    if !pak::exists(root.join("classes/bf2_imp.ron")) {
        eprintln!("SKIP: actual BF2 class bake absent");
        return;
    }
    let db =
        crate::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    let mut sim = Sim::empty(10);
    sim.weapon_classes = ["com_weap_inf_force_lightning", "cis_weap_hero_flamethrower"]
        .map(|name| core::weapon::WeaponClass {
            name: name.into(),
            ..Default::default()
        })
        .to_vec()
        .into();
    let state = State::load(&db, &sim);
    assert_eq!(
        state.looks[0].lightning.as_deref(),
        Some("com_sfx_lightning")
    );
    assert_eq!(state.looks[0].intensity, 0.15);
    assert_eq!(state.looks[0].minimum_bolts, 5);
    assert_eq!(state.looks[0].bolt_spread, 0.4);
    assert_eq!(
        state.looks[1].emission.as_deref(),
        Some("com_sfx_weap_flamethrower_ord")
    );
    println!("actual BF2 lightning and Jango flame emitter looks resolved through OrdnanceName");
}

#[test]
fn native_lightning_points_taper_fade_and_seed_control_are_preserved() {
    let camera = Camera {
        position: Vec3::new(5.0, 4.0, 8.0),
        ..Camera::default()
    };
    let spec = Strip {
        width: 0.5,
        spread: 0.8,
        phase: 0.25,
        break_distance: 1.0,
        texture_size: 3.5,
        branch_factor: 0.0,
        branch_spread: 0.8,
        branch_length: 4.0,
        max_branches: 3,
    };
    let draw = |seeds, spread| {
        let mut out = Vec::new();
        strip_draws(
            [Vec3::ZERO, Vec3::Z * 5.0],
            seeds,
            Strip { spread, ..spec },
            [130, 170, 255, 143],
            None,
            &camera,
            &mut out,
        );
        out
    };
    let jittered = draw([234, 671], 0.8);
    assert_eq!(jittered, draw([234, 671], 0.8));
    assert_ne!(
        jittered,
        draw([235, 671], 0.8),
        "seed affects actual geometry"
    );
    let straight = draw([234, 671], 0.0);
    let ParticleShape::Shaded { tris } = &straight[0].shape else {
        panic!("shaded strip")
    };
    assert_eq!(tris.len(), 5 * 6);
    assert_eq!(tris[0].2, [130, 170, 255, 0]);
    assert_eq!(tris[2].2, [130, 170, 255, 143]);
    assert_eq!(tris.last().unwrap().2[3], 0);
    assert!((tris[0].0.distance(tris[1].0) - 1.0).abs() < 1e-5);
    assert!(
        tris[2].0.distance(tris[5].0) > tris[26].0.distance(tris[29].0),
        "native taper narrows the end"
    );
    let centre = (tris[0].0 + tris[1].0) * 0.5;
    assert!(centre.length() < 1e-6);
    assert_eq!(tris[26].1[0], 5.0 / 3.5);
}

#[test]
fn each_same_tick_cast_has_its_own_minimum_bolts_and_charge() {
    let def = FxLightning {
        name: "bolt".into(),
        hash: hash("bolt"),
        fade_time: Some(0.6),
        ..FxLightning::default()
    };
    let lib = Library::from_sets(
        super::super::emitter::Era::Bf2,
        vec![manifest::FxSet {
            game: "bf2".into(),
            effects: Vec::new(),
            beams: Vec::new(),
            lightning: vec![def],
        }],
        &mut Lcg(9),
    );
    let mut fx = Fx::with_library(lib);
    fx.force.looks = vec![Look {
        lightning: Some("bolt".into()),
        intensity: 0.15,
        minimum_bolts: 1,
        ..Look::default()
    }];
    let mut sim = Sim::empty(8);
    sim.weapon_classes[0].force = Some(core::hero_weapon::ForcePower {
        range: 10.0,
        cone_cos: 0.5,
        max_targets: 5,
        push: 0.0,
        affect_friends: false,
        affect_enemies: true,
        energy: 0.0,
        kind: core::hero_weapon::ForceKind::Emitter(core::hero_weapon::Emitter {
            hit_all: false,
            first_radius: 1.5,
            jump_deduction: 0.0,
            damage_threshold: 0.0,
            max_jump_distance: 10.0,
            lightning: true,
            burn_seconds: 0.0,
            burn_damage_per_second: 0.0,
        }),
        effect: None,
        animation: None,
    });
    let owner = sim
        .spawn(&core::unit::UnitSpec::player(0, Vec3::ZERO))
        .unwrap();
    let target = sim
        .spawn(&core::unit::UnitSpec::trooper(1, Vec3::NEG_Z * 4.0))
        .unwrap();
    let from = sim.unit(owner).unwrap().muzzle();
    sim.events = vec![
        Event::Ability {
            unit: owner,
            weapon_class: 0,
            pos: from,
            charge: 1.0,
        },
        Event::ForceHit {
            owner,
            target: core::projectile::Target::Unit(target),
            from,
            pos: from + Vec3::NEG_Z * 4.0,
            weapon_class: 0,
            blocked: false,
            charge: 1.0,
        },
        Event::Ability {
            unit: owner,
            weapon_class: 0,
            pos: from,
            charge: 0.5,
        },
    ];
    fx.ingest_sim(&sim);
    assert_eq!(
        fx.force_bolts(),
        2,
        "first contact must not suppress the second cast's miss bolt"
    );
    assert_eq!(fx.force.bolts[0].strength, 0.15);
    assert_eq!(fx.force.bolts[1].strength, 0.075);
    fx.ingest_sim(&sim);
    assert_eq!(fx.force_bolts(), 2, "one event ingestion per sim tick");
    fx.update(0.61);
    assert_eq!(fx.force_bolts(), 0);
    fx.clear();
    fx.ingest_sim(&sim);
    assert_eq!(
        fx.force_bolts(),
        2,
        "clearing particles must preserve loaded class bindings"
    );
}
