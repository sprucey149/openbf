use super::*;
use crate::assets::{CpuSkinnedMesh, SkinnedAsset};
use crate::soldier_vis::{ClassDraw, SoldierAsset, UnitVisual};
use core::hero_weapon::{Blade, BladeSegment, MeleeState, SaberClass};

/// CPU-only geometry exercises the real trail and audio callers without a
/// renderer or an optional hero bake. The ninth edge is deliberately invalid
/// programmatic data; the authored parser has its own eight/nine boundary test.
fn scene(edges: usize, hidden_edges: u8) -> (WalkSim, SoldierVisuals) {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let player = walk.player.index as usize;
    walk.sim.units.truncate(player + 1);
    let bindings: Vec<_> = (0..edges)
        .map(|_| BladeBinding {
            blade: Blade {
                node: "hp_fire".into(),
                bone: "hp_weapons".into(),
                length: 1.0,
                width: 0.1,
                texture: "blade".into(),
                trail_color: Some([255; 4]),
            },
            geometry: None,
            glow_texture: None,
            tracked_mount: Mat4::IDENTITY,
            local: BladeSegment {
                start: Vec3::ZERO,
                end: Vec3::Y,
            },
        })
        .collect();
    let weapon_class = walk.sim.units[player].weapon_class;
    walk.sim.weapon_classes[weapon_class].saber = Some(Box::new(SaberClass {
        combo: manifest::combo::ComboDef {
            name_hash: 0,
            fields: vec![],
        },
        blades: bindings.iter().map(|b| b.blade.clone()).collect(),
        hand_edges: Vec::new(),
        throw_edges: vec![],
        deflect_edges: vec![],
        weapon_bind: Mat4::IDENTITY,
        throw_bindings: vec![],
        clips: vec![],
        state_seconds: vec![],
        damage: 0.0,
        hit_sound: None,
        deflect_sound: None,
        hit_effect: None,
        deflect_effect: None,
    }));
    walk.sim.units[player].weapon.melee = Some(MeleeState {
        hidden_edges,
        attacks: (0..edges)
            .map(|edge| core::combo::AttackWindow {
                index: edge,
                id: 0,
                record: 0,
                cache_end: 1.0,
                edge: edge as u8,
                damage: None,
                length: None,
                length_from_edge: false,
                width: None,
                width_from_edge: false,
                push: 0.0,
                play_explosion: false,
            })
            .collect(),
        ..Default::default()
    });
    let mut visuals = SoldierVisuals::default();
    visuals.assets.push(SoldierAsset {
        low: None,
        lod: None,
        asset: SkinnedAsset {
            name: "visibility_control".into(),
            skeleton: crate::skin::Skeleton::new(
                vec!["hp_weapons".into()],
                vec![None],
                vec![Mat4::IDENTITY],
            ),
            mesh: CpuSkinnedMesh {
                positions: vec![],
                normals: vec![],
                uvs: vec![],
                colors: vec![],
                joints: vec![],
                weights: vec![],
                indices: vec![],
                submeshes: vec![],
                spec: vec![],
            },
            clips: vec![],
        },
        meshes: vec![],
        rig: None,
        hp_weapons: Some(0),
        groups: vec![],
        cloths: vec![],
    });
    let u = &walk.sim.units[player];
    let class = usize::from(u.spec.class);
    visuals.per_class[u.team as usize].resize(class + 1, ClassDraw::default());
    visuals.per_class[u.team as usize][class].body = Some(0);
    visuals.sabers[u.team as usize].resize_with(class + 1, || None);
    visuals.sabers[u.team as usize][class] = Some(SaberVisual {
        bindings,
        textures: vec![None; edges],
        glows: vec![None; edges],
        hilts: vec![],
        rng: Cell::new(0),
    });
    visuals.units.resize_with(player + 1, UnitVisual::default);
    visuals.units[player].gen = u.gen;
    visuals.units[player].body = Some((u.team, u.spec.class, Some(0)));
    assert_eq!(visuals.posed(&walk).len(), 1, "visible CPU body control");
    (walk, visuals)
}

#[test]
fn byte_visibility_controls_actual_trail_counts_and_audio_at_the_eighth_edge() {
    for (edges, hidden, expected) in [(8, 0, 8), (8, 1, 7), (8, 128, 7), (8, 255, 0), (9, 255, 0)] {
        let (mut walk, mut visuals) = scene(edges, hidden);
        for _ in 0..3 {
            walk.sim.units[walk.player.index as usize].soldier.pos.x += 0.3;
            visuals.step_saber_trails(&walk, core::TICK_DT);
        }
        assert_eq!(
            visuals.saber_trail_draws(&walk).len(),
            expected,
            "{edges} edges, hidden {hidden}"
        );
        assert_eq!(
            crate::event_sounds::saber_visible(
                &walk.sim.units[walk.player.index as usize],
                &walk.sim.weapon_classes
            ),
            expected > 0
        );
        // The draw caller must reject the ninth programmatic edge before its
        // texture lookup, even when no GPU texture has been uploaded.
        visuals.saber_draws(&walk);
    }
}

#[cfg(all(feature = "vr", any(windows, target_os = "linux")))]
#[test]
#[ignore = "requires a GPU and the local BF2 saber bake"]
fn tracked_saber_draws_the_baked_blade_with_the_body_hidden() {
    let root = crate::assets::baked_root();
    if !root
        .join("side_bf2/weapons/imp_weap_inf_lightsabre.nodes.ron")
        .is_file()
    {
        eprintln!("SKIP: Vader's local saber bake is absent");
        return;
    }
    let (mut walk, mut vis) = scene(1, 0);
    walk.sim.unit_mut(walk.player).unwrap().first_person = true;
    assert!(walk.player_body_hidden());
    assert!(vis.saber_draws(&walk).is_empty());
    let mut team = crate::walk::TeamTemplate::default_trooper();
    team.faction = "imp".into();
    team.classes[0].name = "imp_hero_darthvader".into();
    let mut renderer = Renderer::new_headless(192, 192).unwrap();
    vis.sabers = vis.load_sabers(Some(&mut renderer), &root, &[team.clone(), team]);
    let at = Mat4::from_rotation_translation(
        glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        Vec3::new(0.25, -0.5, -1.6),
    );
    let mut blades = Vec::new();
    vis.tracked_saber_draws(&walk, [None, Some(at)], &mut blades);
    let blade = vis.saber_visual(0, 0).unwrap().bindings[0].segment(at);
    assert!(blades.iter().any(|draw| matches!(
        draw.shape, ParticleShape::CappedRibbon { head, .. } if head == blade.end
    )));
    let mut hilts = Vec::new();
    vis.tracked_draws(&walk, [None, Some(at)], &mut hilts);
    assert!(!hilts.is_empty());

    let camera = render::Camera::default();
    let clear = [0.0, 0.0, 0.0, 1.0];
    renderer
        .render_frame(&camera, &[], &[], clear, None)
        .unwrap();
    let background = renderer.capture_rgba().unwrap().0;
    renderer.set_particles(blades);
    renderer
        .render_frame(&camera, &[], &[], clear, None)
        .unwrap();
    let rendered = renderer.capture_rgba().unwrap().0;
    assert_ne!(
        rendered, background,
        "the tracked blade is visible in a headless frame"
    );
}

#[cfg(all(feature = "vr", any(windows, target_os = "linux")))]
#[test]
#[ignore = "requires a GPU and the local BF2 hero bake"]
fn tracked_sabers_draw_aligned_hilts_and_forearms_for_dual_wielders() {
    let root = crate::assets::baked_root();
    let cases = [
        ("rep", "rep_hero_aalya", "rep_inf_aaylasecura"),
        ("cis", "cis_hero_grievous", "cis_inf_generalgrevious"),
    ];
    if cases.iter().any(|(_, _, geometry)| {
        !root
            .join(format!("side_bf2/soldiers/{geometry}.glb"))
            .is_file()
    }) {
        eprintln!("SKIP: local dual-wield hero bakes are absent");
        return;
    }
    let mut renderer = Renderer::new_headless(192, 192).unwrap();
    for (faction, hero, geometry) in cases {
        let (mut walk, mut vis) = scene(1, 0);
        walk.sim.unit_mut(walk.player).unwrap().first_person = true;
        vis.assets[0] = SoldierVisuals::load_body(
            Some(&mut renderer),
            &root,
            "bf2",
            geometry,
            &mut HashMap::new(),
        )
        .unwrap();
        vis.load_tracked_arms(&mut renderer);
        let mut team = crate::walk::TeamTemplate::default_trooper();
        team.faction = faction.into();
        team.classes[0].name = hero.into();
        vis.sabers = vis.load_sabers(Some(&mut renderer), &root, &[team.clone(), team]);

        for (side, x) in [(0, -0.27), (1, 0.27)] {
            let at = Mat4::from_translation(Vec3::new(x, -0.42, -1.55));
            let mut mounts = [None; 2];
            mounts[side] = Some(at);
            let mut hilts = Vec::new();
            vis.tracked_saber_hilts(&walk, mounts, &mut hilts);
            let visual = vis.saber_visual(0, 0).unwrap();
            let binding = visual
                .bindings
                .iter()
                .find(|b| b.blade.tracked_hand() == Some(side))
                .unwrap();
            let matrix = at * binding.tracked_mount;
            assert!(
                !hilts.is_empty() && hilts.iter().all(|d| d.model.abs_diff_eq(matrix, 1e-5)),
                "{hero} hand {side}"
            );

            let mut draws = Vec::new();
            vis.tracked_draws(&walk, mounts, &mut draws);
            assert!(
                draws.len() > hilts.len(),
                "{hero} hand {side} draws a forearm"
            );
            let tip = binding.segment(matrix).end;
            let mut blades = Vec::new();
            vis.tracked_saber_draws(&walk, mounts, &mut blades);
            assert!(blades.iter().any(|draw| matches!(
                draw.shape, ParticleShape::CappedRibbon { head, .. } if head.abs_diff_eq(tip, 1e-5)
            )), "{hero} hand {side} blade stays on the emitter");
        }
    }
}
