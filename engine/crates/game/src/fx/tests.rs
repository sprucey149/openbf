use super::*;

mod jet;
mod resupply;
mod turret;

/// One finite white particle, with no motion, child, sound or asset lookup.
pub(super) fn one_shot_emitter() -> FxEmitter {
    let zero = manifest::FxRange(0.0, 0.0);
    let one = manifest::FxRange(1.0, 0.0);
    FxEmitter {
        label: "fixture".into(),
        start_delay: zero,
        burst_delay: zero,
        burst_count: one,
        max_particles: one,
        color: [one; 4],
        hsv: false,
        size: one,
        spawner: manifest::FxSpawner {
            shape: manifest::FxShape::None,
            shape_ranges: [zero; 9],
            offset: [zero; 3],
            position_scale: one,
            velocity_scale: zero,
            inherit_velocity_factor: zero,
            size: one,
            color: [one; 4],
            hsv: false,
            fade_in_time: 0.0,
            start_rotation: zero,
            rotation_velocity: zero,
        },
        transformer: manifest::FxTransformer {
            lifetime: 1.0,
            position: Vec::new(),
            size: Vec::new(),
            rotation: Vec::new(),
            color: Vec::new(),
        },
        geometry: manifest::FxGeometry {
            blend: manifest::FxBlend::Normal,
            kind: manifest::FxType::Particle,
            texture: None,
            model: None,
            child: None,
            time_per_frame: 0.1,
            frame_size: 1.0,
            total_frames: 1.0,
            blur_value: 0.0,
            blur_res: 0,
            spark_length: 0.0,
            looping: false,
        },
        min_lod_dist: 0.0,
        max_lod_dist: 100.0,
        bounding_radius: 5.0,
        no_register_step: false,
        world_aligned: false,
        sibling: None,
        sound_name: None,
    }
}

#[test]
fn geometry_and_child_geometry_draw_meshes_with_the_particle_transform() {
    let mut em = one_shot_emitter();
    em.spawner.size = manifest::FxRange(2.0, 0.0);
    em.spawner.start_rotation = manifest::FxRange(std::f32::consts::FRAC_PI_2, 0.0);
    em.transformer.lifetime = 5.0;
    em.geometry.kind = manifest::FxType::Geometry;
    em.geometry.model = Some("fixture".into());
    struct Assets;
    impl FxAssets for Assets {
        fn texture(&mut self, _: &str) -> Option<TextureHandle> {
            None
        }
        fn model(&mut self, _: &str) -> Option<Arc<draw::EffectModel>> {
            Some(Arc::new(draw::EffectModel {
                parts: vec![(None, vec![(Vec3::X, [0.0; 2]); 3], 0)],
            }))
        }
    }
    for (nested, blend) in [
        (false, manifest::FxBlend::Normal),
        (true, manifest::FxBlend::Normal),
        (false, manifest::FxBlend::Blur),
        (true, manifest::FxBlend::Blur),
    ] {
        let mut em = em.clone();
        em.geometry.blend = blend;
        let mut root = em.clone();
        if nested {
            root.geometry.kind = manifest::FxType::Emitter;
            root.geometry.blend = manifest::FxBlend::Normal;
            root.geometry.child = Some(Box::new(em.clone()));
        }
        let def = Arc::new(EffectDef::new(1, None, vec![root], Era::Bf2, &mut Lcg(1)));
        let mut effect = LiveEffect::new(def, Frame::at(Vec3::ZERO), &mut Lcg(1));
        effect.advance(0.5);
        let mut out = Vec::new();
        effect.draws_with(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::NEG_Z,
            &mut Assets,
            &mut out,
        );
        if blend == manifest::FxBlend::Blur {
            assert!(out.is_empty(), "BLUR geometry drew colour, nested={nested}");
            continue;
        }
        assert_eq!(out.len(), 1, "nested={nested}");
        let render::ParticleShape::SolidMesh { tris } = &out[0].shape else {
            panic!(
                "an opaque segment at full alpha draws solid: {:?}",
                out[0].shape
            )
        };
        assert!(
            (tris[0].0 - Vec3::new(0.0, 0.0, -2.0)).length() < 0.00001,
            "raw radians, size without sprite multiplier: {:?}",
            tris[0].0
        );
    }
}

#[test]
fn geometry_segments_follow_the_model_draw_rules() {
    let c = [200, 100, 50, 255];
    let faded = [200, 100, 50, 128];
    assert_eq!(draw::geometry_look(Era::Bf2, false, 0x10000, c), None);
    assert_eq!(
        draw::geometry_look(Era::Bf2, false, 0x10000, faded),
        Some((false, faded))
    );
    assert_eq!(draw::geometry_look(Era::Bf2, true, 0, c), Some((true, c)));
    assert_eq!(
        draw::geometry_look(Era::Bf1, false, 0x04, c),
        Some((false, c))
    );
    assert_eq!(
        draw::geometry_look(Era::Bf1, false, 0x06, c),
        Some((false, c))
    );
    assert_eq!(
        draw::geometry_look(Era::Bf1, false, 0x84, c),
        Some((true, c))
    );
    assert_eq!(
        draw::geometry_look(Era::Bf2, false, 0x44, c),
        Some((true, c))
    );
    assert_eq!(
        draw::geometry_look(Era::Bf2, false, 0x84, c),
        Some((false, c))
    );
}

#[test]
fn a_fading_lightning_model_blends_by_its_sheet_and_an_opaque_one_draws_solid() {
    let model = draw::EffectModel {
        parts: vec![(None, vec![(Vec3::X, [0.0; 2]); 3], 0x10000)],
    };
    let mut out = Vec::new();
    model.draws(
        Mat4::IDENTITY,
        [255, 255, 255, 254],
        Vec3::ZERO,
        false,
        Era::Bf2,
        &mut out,
    );
    model.draws(
        Mat4::IDENTITY,
        [255, 255, 255, 255],
        Vec3::ZERO,
        false,
        Era::Bf2,
        &mut out,
    );
    assert!(matches!(out[0].shape, render::ParticleShape::Mesh { .. }));
    assert_eq!(
        (out[0].color, out[0].additive),
        ([255, 255, 255, 254], false)
    );
    assert!(matches!(
        out[1].shape,
        render::ParticleShape::SolidMesh { .. }
    ));
    assert_eq!(out[1].color, [255; 4]);
}

fn puff(kind: Kind) -> Puff {
    Puff {
        pos: Vec3::ZERO,
        kind,
        team: 0,
        age: 0.0,
    }
}

#[test]
fn puffs_expire_after_their_lifetime() {
    let mut fx = Fx {
        puffs: vec![puff(Kind::Hit), puff(Kind::Impact)],
        ..Fx::default()
    };
    fx.update(HIT_LIFE + 1e-3);
    assert_eq!(fx.len(), 1, "the hit spark is the short one");
    fx.update(IMPACT_LIFE);
    assert!(fx.is_empty());
}

/// Effects fade by ALPHA now that they are composited, not by dimming
/// their colour — a flash that dims and stays opaque reads as a grey card.
#[test]
fn effects_change_size_and_fade_out_by_alpha() {
    let mut m = puff(Kind::Hit);
    let (_, c0) = m.look();
    assert_eq!(c0[3], 1.0, "born fully opaque");
    m.age = HIT_LIFE;
    let (_, c1) = m.look();
    assert_eq!(c1[3], 0.0, "and is gone by the end of its life");
    assert_eq!(c1[..3], c0[..3], "colour does not dim; alpha does the work");

    let mut i = puff(Kind::Impact);
    let (s0, a0) = i.look();
    i.age = IMPACT_LIFE;
    let (s1, a1) = i.look();
    assert!(s1 > s0, "the impact expands");
    assert!(a1[3] < a0[3]);
}

/// Every effect goes through the blend pass. Alpha-testing them at 0.5
/// would delete the soft rim and stop them fading at all.
#[test]
fn effects_are_composited_not_alpha_tested() {
    let fx = Fx {
        puffs: vec![puff(Kind::Impact), puff(Kind::Hit), puff(Kind::Blast(4.0))],
        ..Fx::default()
    };
    let draws = fx.draws(render::MeshHandle::default(), None, &Camera::default());
    assert_eq!(draws.len(), 3);
    assert!(draws.iter().all(|d| d.layer == render::DrawLayer::Blend));
}

#[test]
fn quads_face_the_camera() {
    let p = puff(Kind::Hit);
    // Camera looking down -Z: the quad's x axis is world +X, y axis +Y.
    let (m, _) = billboard(&p, &Camera::default());
    assert!(m.x_axis.truncate().normalize().abs_diff_eq(Vec3::X, 1e-4));
    assert!(m.y_axis.truncate().normalize().abs_diff_eq(Vec3::Y, 1e-4));

    // Yawed 90 deg: it turns with the camera.
    let cam = Camera {
        yaw: std::f32::consts::FRAC_PI_2,
        ..Camera::default()
    };
    let (m, _) = billboard(&p, &cam);
    let x = m.x_axis.truncate().normalize();
    assert!(x.abs_diff_eq(Vec3::NEG_Z, 1e-4), "{x:?}");
}

/// The BF1 common layer, when this tree has a bake.
fn bf1_common() -> Option<Library> {
    let dir = crate::assets::baked_root().join("common_bf1");
    if !dir.join("effects.ron").exists() {
        eprintln!("no baked common_bf1/effects.ron: skipped");
        return None;
    }
    let lib = Library::load(&crate::assets::baked_root(), &dir, "bf1", &[]);
    assert!(!lib.is_empty());
    Some(lib)
}

/// Card FXP3: an impact spawns its ordnance's effect for the surface,
/// then its explosion's; a name no layer holds spawns nothing, and air
/// expiry (no `Impact`) spawns nothing at all.
#[test]
fn impacts_spawn_the_surface_effect_then_the_explosion() {
    use core::sim::Event;
    use core::weapon::WeaponClass;
    let Some(lib) = bf1_common() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let mut w = WeaponClass::bf1_rifle();
    let l = w.ordnance.impact.as_mut().unwrap();
    l.building = Some("no_such_effect".into());
    l.explosion = Some(ExplosionLook {
        effects: vec!["small_explosion".into()],
        ..ExplosionLook::default()
    });
    let wc = sim.add_weapon_class(w) as u32;
    let impact = |surface| Event::Impact {
        pos: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        weapon_class: wc,
        surface,
        vehicle: None,
    };
    let names = |fx: &Fx| -> Vec<String> {
        fx.live
            .iter()
            .map(|e| e.def.name.clone().unwrap_or_default())
            .collect()
    };
    sim.events = vec![impact(Surface::Terrain)];
    fx.ingest_sim(&sim);
    assert_eq!(names(&fx), [BF1_IMPACT, "small_explosion"]);
    assert_eq!(
        fx.live[1].frame.pos,
        Vec3::Z * EXPLOSION_OFFSET,
        "backed off along the bolt"
    );
    // No SoundName on `small_explosion`: nothing queued (card WSND).
    assert!(fx.take_sounds().is_empty());
    // `Explosion` carries "com_weap_obj_med_exp defer" (`med_explosion`,
    // the AAT's death blast, carries none).
    let med = ExplosionLook {
        effects: vec!["Explosion".into()],
        ..ExplosionLook::default()
    };
    assert!(fx.explode(&med, Vec3::X, None));
    assert_eq!(
        fx.take_sounds(),
        [("com_weap_obj_med_exp".to_owned(), Vec3::X)]
    );
    assert!(fx.take_sounds().is_empty());
    fx.clear();
    sim.events = vec![impact(Surface::Building)];
    fx.ingest_sim(&sim);
    assert_eq!(names(&fx), ["small_explosion"]);
    fx.clear();
    sim.events = vec![Event::BoltExpired {
        pos: Vec3::ZERO,
        struck: false,
    }];
    fx.ingest_sim(&sim);
    assert!(fx.is_empty());
}

#[test]
fn water_takes_the_impact_onto_its_surface_and_swaps_the_blast() {
    use core::sim::Event;
    use core::weapon::WeaponClass;
    let Some(lib) = bf1_common() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    sim.water_level = Some(-2.0);
    let wc = sim.add_weapon_class(WeaponClass::bf1_rifle()) as u32;
    sim.events = vec![Event::Impact {
        pos: Vec3::new(1.0, -2.0, 3.0),
        dir: Vec3::NEG_Z,
        weapon_class: wc,
        surface: Surface::Water,
        vehicle: None,
    }];
    fx.ingest_sim(&sim);
    assert_eq!(fx.live.len(), 1);
    assert_eq!(fx.live[0].name(), Some("watersplash_ord"));
    assert_eq!(
        fx.live[0].frame.pos,
        Vec3::new(1.0, -2.0 + SPLASH_LIFT, 3.0)
    );
    let look = ExplosionLook {
        effects: vec!["small_explosion".into()],
        water_effect: Some("watersplash_lg".into()),
        radius_outer: 4.0,
        ..ExplosionLook::default()
    };
    let w = Some(-2.0);
    for (y, want) in [
        (-1.69, "small_explosion"),
        (-1.71, "watersplash_lg"),
        (-5.0, "watersplash_lg"),
    ] {
        fx.clear();
        assert!(fx.explode(&look, Vec3::new(1.0, y, 3.0), w));
        assert_eq!(fx.live[0].name(), Some(want), "{y}");
        if want == "watersplash_lg" {
            assert_eq!(
                fx.live[0].frame.pos,
                Vec3::new(1.0, -2.0 + SPLASH_LIFT, 3.0)
            );
        }
    }
    fx.clear();
    assert!(fx.explode(&look, Vec3::new(1.0, -5.0, 3.0), None));
    assert_eq!(fx.live[0].name(), Some("small_explosion"));
    fx.clear();
    let still = ExplosionLook {
        radius_outer: 0.0,
        ..look.clone()
    };
    assert!(!fx.explode(&still, Vec3::new(1.0, -2.0, 3.0), w));
    assert!(fx.live.is_empty(), "no damage radius: nothing in the water");
}

#[test]
fn water_hits_leave_ripples_and_their_splashes_ride_the_bob() {
    use core::sim::Event;
    use core::weapon::WeaponClass;
    let Some(lib) = bf1_common() else { return };
    let mut fx = Fx::with_library(lib);
    fx.water = ripple::Water::new(true, Some(ripple::kas1()));
    let mut sim = Sim::empty(1);
    sim.water_level = Some(-2.0);
    let wc = sim.add_weapon_class(WeaponClass::bf1_rifle()) as u32;
    sim.events = vec![Event::Impact {
        pos: Vec3::new(1.0, -2.0, 3.0),
        dir: Vec3::NEG_Z,
        weapon_class: wc,
        surface: Surface::Water,
        vehicle: None,
    }];
    fx.ingest_sim(&sim);
    let r = fx.ripples()[0];
    assert_eq!(
        (r.life, r.size, r.at),
        (1.5, 0.01, Vec3::new(1.0, -2.0, 3.0))
    );
    let t = std::f32::consts::FRAC_PI_2 / 1.884_955_8;
    assert!((fx.ride_the_water(t) - 0.06).abs() < 1e-6);
    assert!((fx.live[0].frame.pos.y - (-2.0 + 0.06 + SPLASH_LIFT)).abs() < 1e-6);
    assert_eq!(fx.ride_the_water(0.0), 0.0);
    assert_eq!(fx.live[0].frame.pos.y, -2.0 + SPLASH_LIFT);
    let look = ExplosionLook {
        effects: vec!["small_explosion".into()],
        water_effect: Some("watersplash_lg".into()),
        radius_outer: 6.0,
        ..ExplosionLook::default()
    };
    for (radius, life, size) in [(6.0, 3.4, 0.4), (0.5, 3.25, 0.25)] {
        fx.clear();
        let look = ExplosionLook {
            radius_outer: radius,
            ..look.clone()
        };
        assert!(fx.explode(&look, Vec3::new(4.0, -2.5, 5.0), Some(-2.0)));
        let r = fx.ripples()[0];
        assert_eq!(
            (r.life, r.size, r.at),
            (life, size, Vec3::new(4.0, -2.0, 5.0))
        );
        assert_eq!(fx.live[0].float, Some(-2.0));
    }
    fx.clear();
    assert!(fx.explode(&look, Vec3::new(4.0, 1.0, 5.0), Some(-2.0)));
    assert!(
        fx.ripples().is_empty() && fx.live[0].float.is_none(),
        "a land blast"
    );
    fx.clear();
    let quiet = ExplosionLook {
        water_effect: None,
        ..look.clone()
    };
    assert!(!fx.explode(&quiet, Vec3::new(4.0, -2.5, 5.0), Some(-2.0)));
    assert_eq!(
        fx.ripples().len(),
        1,
        "the ripple does not need a WaterEffect"
    );
}

#[test]
fn a_round_carries_its_trail_until_it_is_gone() {
    use core::projectile::{Bolt, Hit};
    use core::weapon::WeaponClass;
    let Some(lib) = bf1_common() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let mut w = WeaponClass::bf1_rifle();
    w.ordnance.impact.as_mut().unwrap().trail = Some("com_sfx_weap_grenade_red".into());
    let wc = sim.add_weapon_class(w);
    let vel = Vec3::new(30.0, 0.0, -40.0);
    let bolt = |weapon_class| Bolt {
        visual_shot: None,
        pos: Vec3::new(0.0, 1.0, 0.0),
        vel,
        remaining: 1.0,
        age: 0.0,
        owner: UnitHandle { index: 0, gen: 0 },
        team: 0,
        weapon_class,
        hit: Hit::None,
        end: Vec3::new(30.0, 1.0, -40.0),
        shooter_is_player: false,
        hulls: core::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    };
    sim.bolts.push(bolt(wc));
    sim.tick = 1;
    fx.ingest_sim(&sim);
    assert_eq!(fx.live.len(), 1);
    let e = &fx.live[0];
    assert_eq!(e.name(), Some("com_sfx_weap_grenade_red"));
    assert_eq!(e.frame.pos, Vec3::new(0.0, 1.0, 0.0));
    assert!((e.frame.rot.z_axis - vel.normalize()).length() < 1e-6);
    assert!(e.frame.rot.x_axis.y.abs() < 1e-6);
    sim.bolts[0].advance(core::TICK_DT);
    sim.tick = 2;
    fx.ingest_sim(&sim);
    assert_eq!(fx.live.len(), 1);
    assert_eq!(fx.live[0].frame.pos, sim.bolts[0].pos);
    assert!(!fx.live[0].is_stopped());
    sim.bolts.clear();
    sim.tick = 3;
    fx.ingest_sim(&sim);
    assert!(fx.live[0].is_stopped());
    let mut plain = WeaponClass::bf1_rifle();
    plain.ordnance.impact.as_mut().unwrap().trail = None;
    let bare = sim.add_weapon_class(plain);
    fx.clear();
    sim.bolts.push(bolt(bare));
    sim.tick = 4;
    fx.ingest_sim(&sim);
    assert!(fx.live.is_empty(), "no TrailEffect, no trail");
}

#[test]
fn grenade_trails_keep_their_rounds_when_an_earlier_one_goes_off() {
    use core::sticky::{Sticky, StickyClass};
    use core::weapon::WeaponClass;
    let Some(lib) = bf1_common() else { return };
    let mut fx = Fx::with_library(lib);
    let mut sim = Sim::empty(1);
    let mut w = WeaponClass::bf1_rifle();
    w.ordnance.impact.as_mut().unwrap().trail = Some("com_sfx_weap_grenade_red".into());
    let wc = sim.add_weapon_class(w);
    let class = StickyClass::new(false);
    let owner = UnitHandle { index: 0, gen: 0 };
    let mut first = Sticky::new(Vec3::ZERO, Vec3::NEG_Z, &class, 0.1, owner, 0, wc);
    first.armed = true;
    let second = Sticky::new(Vec3::X * 5.0, Vec3::X, &class, 0.1, owner, 0, wc);
    sim.stickies = vec![first, second];
    let mut spawned = 0;
    for _ in 0..8 {
        sim.step(&[]);
        fx.ingest_sim(&sim);
        spawned = spawned.max(fx.live.len());
        let running: Vec<_> = fx.live.iter().filter(|e| !e.is_stopped()).collect();
        assert_eq!(running.len(), sim.stickies.len());
        for (e, g) in running.iter().zip(&sim.stickies) {
            assert_eq!(e.frame.pos, g.pos);
        }
    }
    assert_eq!(sim.stickies.len(), 1);
    assert_eq!(spawned, 2, "no trail respawned on the survivor");
    assert!(fx.live[0].is_stopped() && !fx.live[1].is_stopped());
}

/// A synthetic emitter: MaxParticles is a LIFETIME budget — bursts of
/// 3 every 0.1 s stop after 7 (the last clamped to 1), and the instance
/// dies once the last burst outlives the Transformer's 0.25 s.
#[test]
fn bursts_spend_a_lifetime_budget() {
    let Some(lib) = bf1_common() else { return };
    let mut em = lib.get(BF1_IMPACT).unwrap().emitters[0].clone();
    em.start_delay = manifest::FxRange(0.05, 0.0);
    em.burst_delay = manifest::FxRange(0.1, 0.0);
    em.burst_count = manifest::FxRange(2.6, 0.0);
    em.max_particles = manifest::FxRange(7.0, 0.0);
    em.transformer.lifetime = 0.25;
    let mut inst = Instance::new(&em, Era::Bf1, 1, 0);
    let f = Frame::at(Vec3::ZERO);
    inst.update(&em, 0.04, f, false);
    assert_eq!(inst.emitted(), 0, "StartDelay 0.05");
    inst.update(&em, 0.06, f, false);
    assert_eq!(inst.emitted(), 3, "(int)(2.6 + 0.5)");
    inst.update(&em, 0.26, f, false);
    assert_eq!(inst.emitted(), 7, "3 + 3 + 1: clamped to the budget");
    assert!(!inst.is_dead());
    inst.update(&em, 0.25 + 0.25 + 1e-3, f, false);
    assert!(inst.is_dead(), "the last burst (t = 0.25) retired");
}

/// `ord_explosion` (re-fx §1): Sparks emits 2 ANIMATED particles and its
/// sibling Flare 1 PARTICLE, all at t = 0; the Flare is gone after its
/// 0.1 s, the effect after the Sparks' 0.2 s.
#[test]
fn ord_explosion_spawns_two_plus_one_and_dies_after_0_2_s() {
    let Some(lib) = bf1_common() else { return };
    let def = lib.get(BF1_IMPACT).unwrap();
    assert_eq!(
        def.emitters
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>(),
        ["Sparks", "Flare"]
    );
    let mut fx = Fx::with_library(lib);
    assert!(fx.spawn(BF1_IMPACT, Frame::at(Vec3::new(1.0, 2.0, 3.0))));
    let e = &fx.live()[0];
    assert_eq!(e.emitted(), [2, 1]);
    let ps = e.particles();
    assert_eq!((ps[0].len(), ps[1].len()), (2, 1));
    // Sparks: no velocity, so they sit at the hit; born at alpha 0.
    assert!(ps[0].iter().all(|p| p.pos == Vec3::new(1.0, 2.0, 3.0)));
    assert!(ps[0].iter().all(|p| p.frame == Some(0) && p.rgba[3] == 0.0));
    fx.update(0.15);
    let ps = fx.live()[0].particles();
    assert_eq!(
        (ps[0].len(), ps[1].len()),
        (2, 0),
        "the Flare's 0.1 s is over"
    );
    // 0.15 s of 0.015 s frames: frame 10; the colour well into orange.
    assert!(ps[0].iter().all(|p| p.frame == Some(10)));
    fx.update(0.049);
    assert_eq!(fx.live().len(), 1);
    fx.update(0.002);
    assert!(fx.live().is_empty(), "dead after 0.2 s");
}

/// BF1's Sparks draw from 20 variants: every Sparks particle's size is
/// one of its emitter's 20 cached sizes (0.3 + 0.3·rand, ×1000 in an
/// i16), and the square is drawn √2·size′ wide at sizeMul 1.2 up close.
#[test]
fn sparks_come_from_the_variant_cache_and_draw_at_the_size_multiplier() {
    let Some(lib) = bf1_common() else { return };
    let def = lib.get(BF1_IMPACT).unwrap().clone();
    let sizes: Vec<f32> = def.caches[0].iter().map(|v| v.size).collect();
    assert_eq!(sizes.len(), 20);
    assert!(sizes.iter().all(|s| (0.3..0.6).contains(s)));
    let mut rng = Lcg(3);
    let e = LiveEffect::new(def, Frame::at(Vec3::new(0.0, 0.0, -10.0)), &mut rng);
    let ps = e.particles();
    assert!(ps[0].iter().all(|p| sizes.contains(&p.size)));
    let mut out = Vec::new();
    e.draws(Vec3::ZERO, Vec3::NEG_Z, &mut |_| None, &mut out);
    assert_eq!(out.len(), 3);
    match &out[0].shape {
        render::ParticleShape::Sprite { half, .. } => {
            let want = ps[0][0].size * 1.2 * std::f32::consts::FRAC_1_SQRT_2;
            assert!((half - want).abs() < 1e-6, "{half} {want}");
        }
        s => panic!("{s:?}"),
    }
    assert!(out.iter().all(|d| d.additive));
}

fn emitter_ord(lightning: Option<&str>) -> core::weapon::EmitterOrdnance {
    core::weapon::EmitterOrdnance {
        cone_length: 30.0,
        cone_angle: 50.0 * core::weapon::DEG,
        max_targets: 4,
        jump_deduction: 75.0,
        damage_threshold: 75.0,
        max_jump_distance: 20.0,
        first_radius: 0.6,
        push: 0.0,
        lightning: lightning.map(str::to_owned),
        bolts: 1,
        affect_friends: true,
        affect_enemies: true,
        hit_all: false,
        radius: 0.0,
        bf2: false,
        effect: None,
        burn: Default::default(),
    }
}

/// Card UORDE: an `Event::Emitter` link with a `LightningEffect` starts
/// an arc named by it; a class that names none (the flamethrowers)
/// starts nothing. The arc dies after the record's 18 ticks.
#[test]
fn emitter_links_draw_their_lightning_and_only_those() {
    use core::sim::Event;
    use core::unit::UnitHandle;
    use core::weapon::WeaponClass;
    let mut fx = Fx::default();
    let mut sim = Sim::empty(1);
    sim.tick = 5;
    let arc = sim.add_weapon_class(WeaponClass {
        emitter: Some(emitter_ord(Some("arccaster_lightning"))),
        ..Default::default()
    }) as u32;
    let flame = sim.add_weapon_class(WeaponClass {
        emitter: Some(emitter_ord(None)),
        ..Default::default()
    }) as u32;
    let unit = UnitHandle { index: 0, gen: 0 };
    let link = |weapon_class| Event::Emitter {
        unit,
        weapon_class,
        from: Vec3::ZERO,
        to: Vec3::new(6.0, 0.0, 0.0),
    };
    sim.events = vec![link(arc), link(arc)];
    fx.ingest_sim(&sim);
    assert_eq!(fx.arcs.len(), 2, "one arc per link");
    assert!(fx.arcs.iter().all(|a| a.class() == "arccaster_lightning"));
    assert_eq!(fx.len(), 2);
    sim.tick = 22;
    sim.events.clear();
    fx.ingest_sim(&sim);
    assert_eq!(fx.arcs.len(), 2, "still inside its 18 ticks");
    sim.tick = 23;
    fx.ingest_sim(&sim);
    assert!(fx.arcs.is_empty(), "the record's FadeTime is over");
    sim.events = vec![link(flame)];
    fx.ingest_sim(&sim);
    assert!(fx.arcs.is_empty(), "a class with no LightningEffect");
}
