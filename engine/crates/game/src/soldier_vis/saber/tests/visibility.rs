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
