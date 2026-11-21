//! Issue #182, leak census v2 A4 (card LEAK2), on the real bake (data-gated:
//! skipped when the bake is absent): the embed oracle (`Sim::embed_at`)
//! against known controls before anything may act on it.
//!
//! - The HUB control: a soldier standing at every resolved nav hub floor is
//!   where bots walk, so every oracle hit there is a FALSE POSITIVE. Run on
//!   BF1 KAS2 (City Hall), BF1 Endor (hollow trees, boulders), both Hoths
//!   (trenches 2.0-2.5 m under the snow) and BF2 Polis Massa (the floors
//!   under its cut terrain).
//! - The SOLID control: inside KAS2 City Hall's lobes (solid primitives,
//!   re-soldier-walls §2) is `inprim`; inside a watertight prop of Endor is
//!   `inclosed`.
//! - The UNDER control: on Hoth, a soldier stood in a trench (a cut floor
//!   under the snow) is not `underterrain`; the same feet moved just outside
//!   the cut, under the snow, are.
//!
//! Each test prints its counts (`--nocapture`); the bounds are the measured
//! counts of the leak census v2 baseline (2026-11-20), a ratchet. The A4
//! verdict they support: `inprim`, `inclosed` and `underterrain` are near
//! zero at the hubs; `inshell` is not (46 of 492 hub floors, 41 of them on
//! Polis Massa) and must not drive a rescue.

use core::sim::{Embed, Sim, EMBED_NAMES};
use core::soldier::Stance;
use glam::Vec3;
use std::path::Path;

/// One soldier on the map's collision, with the mission's posts so the
/// level (`Sim::level_at`) and the hub floors resolve.
fn build_sim(root: &Path, key: &str) -> game::walk::WalkSim {
    let path = root.join(key);
    let (scene, _) = game::assets::load_battle_scene(&path, "con").unwrap();
    let missions = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let mission = game::mission::MissionDef::load(&missions.join(format!("{key}_con.ron"))).ok();
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));
    let team = game::walk::TeamTemplate::default_trooper();
    let nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    let mut sim = Sim::empty(1);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    let mut w = game::walk::WalkSim::with_teams(
        sim,
        scene.heightfield(),
        Vec3::ZERO,
        [team.clone(), team],
        nav,
    );
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
    }
    w.add_posts(&layout);
    w
}

fn bake_or_skip(key: &str) -> Option<std::path::PathBuf> {
    let root = game::assets::baked_root();
    if !root.join(key).join("manifest.ron").exists() || !root.join("classes").exists() {
        eprintln!("skipped: no {key} bake");
        return None;
    }
    Some(root)
}

/// The oracle at every resolved hub floor: (hubs asked, hits per class).
fn hub_control(sim: &Sim) -> (usize, [usize; 6], Vec<(Vec3, Embed)>) {
    let mut hits = [0usize; 6];
    let mut asked = 0;
    let mut list = Vec::new();
    let Some(g) = sim.nav.as_ref() else {
        return (0, hits, list);
    };
    for (i, h) in g.hubs.iter().enumerate() {
        let Some(y) = sim.nav_floor(i as u16) else {
            continue;
        };
        let feet = Vec3::new(h.pos.x, y, h.pos.z);
        asked += 1;
        let e = sim.embed_at(feet, Stance::Stand);
        if let Some(c) = e.index() {
            hits[c] += 1;
            list.push((feet, e));
        }
    }
    (asked, hits, list)
}

/// The A4 measurement: the oracle's false positives at the hub floors of
/// the five control maps, printed per class. The bound per map is the
/// count measured on the v2 baseline: a change that adds a false positive
/// fails here first.
#[test]
fn embed_oracle_hub_control_counts() {
    // (map, the hub-floor hits the v2 baseline measured, 2026-11-20 at
    // 266dd51fa). Every one is `inshell` but BF2 Polis Massa's 1 `inprim`
    // and 2 `overlap` (hub centres against a prop: the hub, not the
    // soldier, is in the wrong place). `underterrain`, `inclosed` and
    // `offlevel` read 0 on all five, Polis Massa's floors under its cut
    // terrain and both Hoths' trench hubs included.
    let maps: [(&str, usize); 5] = [
        ("bf1_kas2", 0),
        ("bf1_end1", 0),
        ("bf1_hot1", 3),
        ("bf2_hot1", 2),
        ("bf2_pol1", 44),
    ];
    for (key, bound) in maps {
        let Some(root) = bake_or_skip(key) else {
            continue;
        };
        let w = build_sim(&root, key);
        let (asked, hits, list) = hub_control(&w.sim);
        eprintln!(
            "hubs {key}: asked {asked} | {}",
            EMBED_NAMES
                .iter()
                .zip(hits)
                .map(|(n, h)| format!("{n} {h}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        for (p, e) in &list {
            eprintln!("  hub hit {key} ({:.2},{:.2},{:.2}) {e:?}", p.x, p.y, p.z);
        }
        assert!(asked > 0, "{key}: no resolved hub floor");
        let total: usize = hits.iter().sum();
        assert!(
            total <= bound,
            "{key}: {total} oracle hits at hub floors, the baseline bound is {bound}"
        );
    }
}

/// Inside KAS2 City Hall's lobes (`kas1_bldg_hut_grand`'s primitives):
/// `inprim`, for every SOLDIER primitive of the building whose centre the
/// soldier's axis can reach.
#[test]
fn embed_oracle_kas2_city_hall_lobes_are_inprim() {
    let Some(root) = bake_or_skip("bf1_kas2") else {
        return;
    };
    let path = root.join("bf1_kas2");
    let (scene, _) = game::assets::load_battle_scene(&path, "con").unwrap();
    let w = build_sim(&root, "bf1_kas2");
    let sim = &w.sim;
    let hut: Vec<u32> = scene
        .manifest
        .instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.class.contains("hut_grand"))
        .map(|(k, _)| k as u32)
        .collect();
    assert!(!hut.is_empty(), "no City Hall instance");
    let mut tried = 0;
    for (b, body) in sim.collision.bodies().iter().enumerate() {
        if !body.instance.is_some_and(|i| hut.contains(&i))
            || !body.layers.contains(core::collision::Layers::SOLDIER)
            || matches!(body.shape, core::collision::Shape::Mesh(_))
        {
            continue;
        }
        let c = body.to_world.transform_point3(Vec3::ZERO);
        // Feet 1.25 m under the centre: the axis point at 1.25 is in it.
        let e = sim.embed_at(c - Vec3::Y * 1.25, Stance::Stand);
        eprintln!(
            "city hall prim {b} centre ({:.2},{:.2},{:.2}) {e:?}",
            c.x, c.y, c.z
        );
        assert_eq!(e.index(), Some(0), "prim {b}: {e:?}");
        tried += 1;
    }
    assert!(tried >= 4, "the four lobes: found {tried} prims");
}

/// Inside a watertight prop of BF1 Endor (a boulder): `inclosed`. Every
/// closed SOLDIER mesh smaller than 30 m whose centre is enclosed by it.
#[test]
fn embed_oracle_endor_closed_props_are_inclosed() {
    let Some(root) = bake_or_skip("bf1_end1") else {
        return;
    };
    let w = build_sim(&root, "bf1_end1");
    let sim = &w.sim;
    let (mut asked, mut closed) = (0, 0);
    for (b, body) in sim.collision.bodies().iter().enumerate() {
        if !body.closed || !body.layers.contains(core::collision::Layers::SOLDIER) {
            continue;
        }
        let ext = body.aabb.max - body.aabb.min;
        if ext.max_element() > 30.0 || ext.min_element() < 2.0 {
            continue;
        }
        let c = (body.aabb.min + body.aabb.max) * 0.5;
        if sim.collision.enclosing(c) != Some(b) {
            continue;
        }
        asked += 1;
        let e = sim.embed_at(c - Vec3::Y * 1.0, Stance::Stand);
        if matches!(e, Embed::InClosedMesh(x) if x == b) {
            closed += 1;
        } else {
            eprintln!(
                "endor closed body {b} centre ({:.2},{:.2},{:.2}): {e:?}",
                c.x, c.y, c.z
            );
        }
    }
    eprintln!("endor closed props: {closed} of {asked} inclosed");
    assert!(asked > 0, "no closed prop found");
    assert_eq!(closed, asked);
}

/// Hoth's trenches: a floor in a cut under the snow is not `underterrain`;
/// the same feet in uncut snow are. Every 3 m over each map's cut cells
/// that have a floor 1-3 m under the snow.
#[test]
fn embed_oracle_hoth_trenches_are_not_under_the_terrain() {
    for key in ["bf1_hot1", "bf2_hot1"] {
        let Some(root) = bake_or_skip(key) else {
            continue;
        };
        let w = build_sim(&root, key);
        let sim = &w.sim;
        let hf = sim.heightfield.as_ref().expect("Hoth has terrain");
        let ext = (hf.size - 1) as f32 * hf.unit;
        let (mut trench, mut under) = (0, 0);
        let mut z = -hf.offset_z;
        while z < -hf.offset_z + ext {
            let mut x = -hf.offset_x;
            while x < -hf.offset_x + ext {
                let t = hf.height_at(x, z);
                let p = Vec3::new(x, t - 0.5, z);
                if sim.collision.is_cut(p) {
                    let floor = sim.ground_under(Vec3::new(x, t + 1.0, z), false);
                    let d = t - floor;
                    if (1.0..=3.0).contains(&d) {
                        trench += 1;
                        let e = sim.embed_at(Vec3::new(x, floor, z), Stance::Stand);
                        if matches!(e, Embed::UnderTerrain(_)) {
                            under += 1;
                            eprintln!("{key} trench ({x:.1},{floor:.2},{z:.1}) {e:?}");
                        }
                    }
                }
                x += 3.0;
            }
            z += 3.0;
        }
        eprintln!("{key}: trench floors {trench}, underterrain {under}");
        assert!(trench > 0, "{key}: no trench floor found");
        assert_eq!(under, 0, "{key}: trench floors read under the terrain");
        // The control's other half: uncut snow 1.5 m over the feet.
        let mut seen = false;
        'find: for k in 0..200 {
            let (x, z) = (
                -hf.offset_x + 7.0 * k as f32 + 3.0,
                -hf.offset_z + ext * 0.5,
            );
            let t = hf.height_at(x, z);
            if !sim.collision.is_cut(Vec3::new(x, t - 0.5, z))
                && sim
                    .collision
                    .ray(
                        Vec3::new(x, t - 1.5, z),
                        Vec3::Y,
                        2.0,
                        core::collision::Layers::SOLDIER,
                    )
                    .is_none()
            {
                let e = sim.embed_at(Vec3::new(x, t - 1.5, z), Stance::Stand);
                assert!(
                    matches!(e, Embed::UnderTerrain(_)),
                    "{key} ({x},{z}): {e:?}"
                );
                seen = true;
                break 'find;
            }
        }
        assert!(seen, "{key}: no uncut snow found for the control");
    }
}
