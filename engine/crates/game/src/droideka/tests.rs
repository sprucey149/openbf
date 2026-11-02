use super::*;
use manifest::{ManifestPrimPoint, ManifestWalkerClip};

fn prim(name: &str, pos: [f32; 3], radius: f32) -> ManifestPrimPoint {
    ManifestPrimPoint {
        name: name.into(),
        bone: "DummyRoot".into(),
        pos,
        radius,
    }
}

fn clip(name: &str, frames: u16, travel: f32) -> Option<ManifestWalkerClip> {
    Some(ManifestWalkerClip {
        name: name.into(),
        frames,
        duration: f32::from(frames - 1) / 30.0,
        looping: false,
        root_z: vec![0.0, travel * 0.5, travel],
    })
}

fn record(prims: Vec<ManifestPrimPoint>) -> ManifestDroideka {
    let mut clips = vec![None; STATES];
    clips[0] = clip("idle", 46, 0.0);
    clips[7] = clip("walk_leftfoot_rightfoot", 17, 0.642);
    clips[8] = clip("walk_rightfoot_leftfoot", 17, 0.642);
    ManifestDroideka {
        class: "cis_inf_droideka".into(),
        geometry: "cis_walk_droideka".into(),
        bank: "cis_walk_droideka".into(),
        clips,
        skin: Some("cis_walk_droideka".into()),
        prims,
    }
}

#[test]
fn clips_take_the_baked_frames_and_the_root_travel() {
    let c = clips(&record(Vec::new()));
    assert_eq!(c[0].frames, 46);
    assert_eq!(c[7].frames, 17);
    assert!((c[7].root_travel - 0.642).abs() < 1e-6);
    assert_eq!(
        c[12].frames, CLIP_FRAMES[12],
        "an unbaked state keeps the table's"
    );
    assert_eq!(c[12].root_travel, 0.0);
}

#[test]
fn bf2_tagged_prims_give_the_ordnance_set_mirrored_into_the_port_frame() {
    let r = record(vec![
        prim("p_-o-gunL", [0.5, 0.81, 1.06], 0.4),
        prim("p_-o-main", [0.0, 0.82, 0.07], 0.4),
        prim("p_-bt-building", [0.0, 0.99, 0.42], 1.0),
        prim("p_-s-yoda", [0.0, 1.0, 0.0], 1.0),
        prim("p_rollsphere", [0.0, 0.65, 0.0], 0.65),
    ]);
    let o = ordnance(&r);
    assert_eq!(o.len(), 2);
    assert_eq!(o[0].center, Vec3::new(-0.5, 0.81, 1.06));
}

#[test]
fn an_untagged_bf1_bake_takes_every_body_but_the_ball_and_the_base() {
    let r = record(vec![
        prim("p_gunL", [0.5, 0.81, 1.06], 0.4),
        prim("p_head", [0.0, 1.45, 0.4], 0.7),
        prim("p_base", [0.0, 1.0, 0.0], 1.75),
        prim("p_rollsphere", [0.0, 0.65, 0.0], 0.65),
    ]);
    let o = ordnance(&r);
    assert_eq!(o.len(), 2);
}

#[test]
fn the_guns_are_hp_gun_1_then_hp_gun_2_the_first_on_the_left() {
    let hp = |name: &str, x: f32| ManifestHardpoint {
        name: name.into(),
        pos: [x, 0.806, 1.462],
        axes: None,
    };
    let g = guns(&[
        hp("hp_gun_2", -0.5),
        hp("bone_l_gun", 0.5),
        hp("hp_gun_1", 0.5),
    ]);
    assert_eq!(
        g,
        [Vec3::new(-0.5, 0.806, 1.462), Vec3::new(0.5, 0.806, 1.462)]
    );
    let world = core::droideka::local_to_world(Vec3::ZERO, 0.0, g[0]);
    assert!(world.x < 0.0, "yaw 0 faces -Z: the left gun is at -X");
}

#[test]
fn apply_keeps_the_gameplay_values_and_sets_the_baked_rig() {
    let base = DroidekaClass::default();
    let b = DroidekaBake {
        record: record(vec![prim("p_-o-head", [0.0, 1.45, 0.4], 0.7)]),
        hardpoints: Vec::new(),
    };
    let c = apply(&base, &b);
    assert_eq!(c.max_speed, base.max_speed);
    assert_eq!(c.shield, base.shield);
    assert_eq!(c.walk_ordnance.len(), 1);
    assert!(
        (c.speed[7] * c.rate[7] - c.max_speed).abs() < 1e-3,
        "{} {}",
        c.speed[7],
        c.rate[7]
    );
}

#[test]
fn as_walker_carries_the_clips_and_skin_with_no_layers() {
    let r = record(Vec::new());
    let w = as_walker(&r);
    assert_eq!(w.clips.len(), STATES);
    assert_eq!(w.skin.as_deref(), Some("cis_walk_droideka"));
    assert!(w.layer_bones.is_empty());
}

#[test]
fn the_side_bakes_carry_each_droideka_with_its_hardpoints() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked");
    for game in ["bf1", "bf2"] {
        let Some(map) = pak::read_dir(&root).into_iter().flatten().find(|d| {
            d.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(game))
        }) else {
            return;
        };
        let b = bakes(&map, game);
        let Some(d) = b.get("cis_inf_droideka") else {
            eprintln!("{game}: no droideka in the side bakes (rebake the CIS side)");
            continue;
        };
        let c = apply(&DroidekaClass::default(), d);
        println!(
            "{game}: clips {:?} ordnance {} guns {:?} walk rate {}",
            c.duration,
            c.walk_ordnance.len(),
            c.guns,
            c.rate[7]
        );
        assert_eq!(c.guns.len(), 2);
        assert!(c.walk_ordnance.len() >= 13);
        for (clip, frames) in clips(&d.record).iter().zip(CLIP_FRAMES).take(STATES) {
            assert_eq!(clip.frames, frames.max(clip.frames));
        }
    }
}

/// The sprint's numbers from each disc's own table.
#[test]
fn the_ball_sprint_reads_the_class_tables() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets_baked/classes");
    for game in ["bf1", "bf2"] {
        let file = root.join(format!("{game}_cis.ron"));
        if !file.exists() {
            return;
        }
        let mut db = crate::classdb::ClassDb::default();
        db.load(&file).unwrap();
        let c = droideka_class(&db, &db, "cis_inf_droideka");
        println!(
            "{game}: bar {} drain {} boost x{} shield {} off {} on {} -> {}/s, {} recover",
            c.energy_bar,
            c.energy_drain_boost,
            c.ball_boost_scale,
            c.shield.max,
            c.shield.rate_off,
            c.shield.rate_on,
            c.ball_sprint_drain(),
            c.ball_sprint_recover()
        );
        assert!(c.ball_sprint_drain() > 0.0);
        assert!(c.ball_sprint_drain() > c.shield.rate_off, "it must empty");
        assert!(c.ball_sprint_recover() > 0.0 && c.ball_sprint_recover() < c.shield.max);
    }
}

#[test]
fn the_shield_hum_fades_as_the_shield_wears() {
    assert_eq!(shield_gain(3000.0, 3000.0), 1.0);
    assert_eq!(shield_gain(1500.0, 3000.0), 0.75);
    assert_eq!(shield_gain(0.0, 3000.0), 0.0);
    assert_eq!(shield_gain(10.0, 0.0), 0.0);
}
