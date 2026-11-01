//! The two-layer animator on a synthetic rig: every clip writes its own
//! number into the bones it has a track for, so a pose says which clip each
//! bone was sampled from.

use super::*;
use crate::skin::BoneXform;
use glam::{Mat4, Quat, Vec3};
use manifest::anim_sets::{Bf1Sets, Bf2Animation, Bf2Layer, Bf2Sets, SetClip};
use std::collections::BTreeMap;

const LEG: usize = 2;
const ARM: usize = 4;
const BONES: [&str; 5] = [
    "DummyRoot",
    "bone_pelvis",
    "bone_l_thigh",
    "bone_ribcage",
    "bone_r_forearm",
];
// Clip numbers.
const IDLE: usize = 0;
const RUN: usize = 1;
const SHOOT: usize = 2;
const BAZOOKA_RUN: usize = 3;
const RELOAD: usize = 4;
const WALK: usize = 5;
const SHOOT_LEGS: usize = 6;
const ALERT: usize = 7;
const TURN: usize = 8;

fn skel() -> Skeleton {
    Skeleton::new(
        BONES.iter().map(|b| (*b).to_string()).collect(),
        vec![None, Some(0), Some(1), Some(1), Some(3)],
        vec![Mat4::IDENTITY; 5],
    )
}

/// A clip whose tracked bones sit at x = 100 * (number + 1) + frame.
fn clip(k: usize, name: &str, frames: usize, upper_only: bool) -> Clip {
    let track = |_| {
        Some(
            (0..frames)
                .map(|f| BoneXform {
                    rotation: Quat::IDENTITY,
                    translation: Vec3::new(100.0 * (k + 1) as f32 + f as f32, 0.0, 0.0),
                })
                .collect(),
        )
    };
    Clip {
        name: name.into(),
        fps: 30.0,
        frames,
        tracks: (0..5)
            .map(|b| if upper_only && b < 3 { None } else { track(b) })
            .collect(),
    }
}

fn clips() -> Vec<Clip> {
    vec![
        clip(IDLE, "idle", 31, false),
        clip(RUN, "run", 19, false),
        clip(SHOOT, "shoot", 7, true),
        clip(BAZOOKA_RUN, "bazooka_run", 19, true),
        clip(RELOAD, "reload", 31, true),
        clip(WALK, "walk", 19, false),
        clip(SHOOT_LEGS, "shoot_legs", 7, false),
        clip(ALERT, "alert_idle", 31, false),
        clip(TURN, "turn", 15, false),
    ]
}

fn set_clips(blend: f32) -> Vec<SetClip> {
    clips()
        .iter()
        .enumerate()
        .map(|(k, c)| SetClip {
            name: c.name.clone(),
            frames: c.frames as u32,
            looping: Some(matches!(k, IDLE | RUN | BAZOOKA_RUN | WALK | ALERT)),
            blend: Some(if k == SHOOT { 0.01 } else { blend }),
        })
        .collect()
}

fn masks() -> (Vec<String>, Vec<String>) {
    let n = |r: std::ops::Range<usize>| BONES[r].iter().map(|b| (*b).to_string()).collect();
    (n(3..5), n(0..3))
}

/// BF1 tables: rifle (set 0) and bazooka (set 1, its own run).
fn rig_bf1() -> Rig {
    let i = |k: usize| Some(k as u16);
    let mut base = vec![vec![None; 13]; 3];
    (base[0][0], base[0][5], base[0][7]) = (i(IDLE), i(WALK), i(RUN));
    (base[0][3], base[0][4]) = (i(TURN), i(TURN));
    let mut w = vec![vec![vec![None; 13]; 9]; 3];
    w[0][0] = base[0].clone();
    w[0][1] = base[0].clone();
    w[0][1][7] = i(BAZOOKA_RUN);
    let mut u = vec![vec![vec![None; 6]; 9]; 3];
    for row in u[0].iter_mut().take(2) {
        (row[1], row[5]) = (i(SHOOT), i(RELOAD));
    }
    let (upper_bones, lower_bones) = masks();
    let sets = SoldierSets {
        game: "bf1".into(),
        upper_bones,
        lower_bones,
        clips: set_clips(0.15),
        bf1: Some(Bf1Sets {
            weapons: bf1::WEAPONS.iter().map(|w| (*w).to_string()).collect(),
            base,
            special: vec![None; 31],
            w,
            alert: vec![vec![None; 13]; 9],
            u,
            s: vec![vec![None; 31]; 9],
        }),
        ..Default::default()
    };
    Rig::new(sets, &skel(), &clips())
}

/// BF2: one family, class rifle; shoot has a lower slot of its own.
fn rig_bf2() -> Rig {
    let mut slots = vec![None; 2 * bf2::PAIRS];
    let mut put = |pair: usize, up: usize, low: usize| {
        (slots[2 * pair], slots[2 * pair + 1]) = (Some(up as u16), Some(low as u16));
    };
    put(bf2::move_pair(0, 0), IDLE, IDLE);
    put(bf2::move_pair(0, 5), WALK, WALK);
    put(bf2::move_pair(0, 6), RUN, RUN);
    put(bf2::move_pair(0, 3), IDLE, TURN);
    put(bf2::move_pair(0, 4), IDLE, TURN);
    put(bf2::action_pair(0, 0), SHOOT, SHOOT_LEGS);
    put(bf2::action_pair(0, 5), RELOAD, SHOOT_LEGS);
    let layer = |looping: bool, t: f32| {
        Some(Bf2Layer {
            looping,
            blend_in: t,
            blend_out: t,
            ..Default::default()
        })
    };
    let bf2_animations = (0..bf2::PAIRS)
        .map(|p| {
            let act = p >= bf2::action_pair(0, 0);
            Bf2Animation {
                name: bf2::base_name(p),
                upper: layer(!act, if act { 0.02 } else { 0.25 }),
                lower: layer(!act, if act { 0.5 } else { 0.2 }),
            }
        })
        .collect();
    let (upper_bones, mut lower_bones) = masks();
    lower_bones.remove(0); // DummyRoot is in neither (§6.4)
    let sets = SoldierSets {
        game: "bf2".into(),
        upper_bones,
        lower_bones,
        clips: set_clips(0.15),
        bf2: vec![Bf2Sets {
            family: "human".into(),
            sets: BTreeMap::from([("rifle".to_string(), slots)]),
        }],
        bf2_animations,
        ..Default::default()
    };
    Rig::new(sets, &skel(), &clips())
}

fn input(speed: f32) -> AnimInput {
    AnimInput {
        stance: Stance::Stand,
        vel: glam::Vec2::new(0.0, speed),
        yaw: 0.0,
        mouse_aim: false,
        max_speed: 7.0,
        root_scale: 1.0,
        dead: false,
        hero_dead: false,
        hard_lock: false,
        set: 0,
        fired: false,
        throw: None,
        throw_windup: 0.0,
        reloading: false,
        reload_time: 0.0,
        alert: false,
        aim: (0.0, 0.0),
    }
}

const DT: f32 = 1.0 / 60.0;

#[test]
fn custom_actions_release_legs_until_next_action_and_keep_authored_full_body_moves() {
    let mut r = Run::new(rig_bf2());
    r.clips.push(clip(9, "human_rifle_swing_full", 31, false));
    r.rig.root.push(Some([0.0, 0.0]));
    let class = ClassAnim {
        family: "human".into(),
        ..Default::default()
    };
    for (speed, force, wants_custom) in [
        (0.0, false, true),
        (7.0, false, false),
        (0.0, false, false),
        (7.0, true, true),
    ] {
        r.a.combo_pose(&r.clips, &class, "saber", "swing", 0.2, false, force);
        r.a.step(&r.rig, &r.clips, &r.skel, &input(speed), DT);
        assert_eq!(r.a.upper_clip(), Some(9));
        assert_eq!(r.a.lower_clip() == Some(9), wants_custom);
    }
    r.a.clear_combo_pose();
    r.a.step(&r.rig, &r.clips, &r.skel, &input(7.0), DT);
    assert_eq!(r.a.lower_clip(), Some(RUN));
    assert_ne!(r.a.upper_clip(), Some(9));
}

#[test]
fn restarting_the_same_combo_clip_restarts_its_blend_clock() {
    use manifest::combo::{ComboDef, ComboField, ComboKeyword as K, ComboValue as V};
    let mut r = Run::new(rig_bf2());
    r.clips.push(clip(9, "human_rifle_swing_full", 31, false));
    r.rig.root.push(Some([0.0, 0.0]));
    let mut class = ClassAnim {
        family: "human".into(),
        ..Default::default()
    };
    class.custom.register(&ComboDef {
        name_hash: 0,
        fields: vec![ComboField {
            keyword: K::Animation,
            args: vec![V::Text("swing".into())],
            scoped: true,
            children: [K::BlendInTime, K::BlendOutTime]
                .map(|keyword| ComboField {
                    keyword,
                    args: vec![V::Number(0.2)],
                    scoped: false,
                    children: Vec::new(),
                })
                .to_vec(),
        }],
    });
    for time in [0.2, 0.4, 0.6] {
        r.a.combo_pose(&r.clips, &class, "saber", "swing", time, false, false);
        r.a.step(&r.rig, &r.clips, &r.skel, &input(0.0), DT);
    }
    let before = r.a.kept_pose(&r.skel);
    r.a.combo_pose(&r.clips, &class, "saber", "swing", 0.0, false, false);
    r.a.step(&r.rig, &r.clips, &r.skel, &input(0.0), DT);
    let after = r.a.kept_pose(&r.skel);
    for bone in [ARM, LEG] {
        // The first frame of clip 9 is x=1000. A fresh authored blend
        // approaches it from the old pose instead of snapping to it.
        let shown = after.0[bone].translation.x;
        assert!(shown > 1006.0 && shown < before.0[bone].translation.x);
    }
}

struct Run {
    rig: Rig,
    clips: Vec<Clip>,
    skel: Skeleton,
    a: Animator,
}

#[test]
fn named_offhand_pose_keeps_running_legs_and_applies_joined_legs() {
    let mut r = Run::new(rig_bf2());
    r.clips.push(clip(9, "human_rifle_cast_full", 31, false));
    r.steps(&input(7.0), 30);
    let before = r.a.pose(&r.skel);
    let class = ClassAnim {
        family: "human".into(),
        ..Default::default()
    };
    assert!(r
        .a
        .offhand_pose(&r.rig, &r.clips, &r.skel, &class, "power", "cast", 0.0, false,));
    let moving = r.a.pose(&r.skel);
    assert_eq!(moving.0[LEG].translation, before.0[LEG].translation);
    assert_eq!(moving.0[ARM].translation.x, 1000.0);
    assert_eq!(moving.0[0].translation, before.0[0].translation);
    assert!(!r.a.combo_override, "ordinary root twist remains enabled");
    assert!(r
        .a
        .offhand_pose(&r.rig, &r.clips, &r.skel, &class, "power", "cast", 0.0, true,));
    let joined = r.a.pose(&r.skel);
    assert_eq!(joined.0[LEG].translation.x, 1000.0);
    assert_eq!(joined.0[ARM].translation.x, 1000.0);
    assert!(!r
        .a
        .offhand_pose(&r.rig, &r.clips, &r.skel, &class, "power", "missing", 0.0, false,));
    let missing = r.a.pose(&r.skel);
    assert_eq!(missing.0[LEG].translation, joined.0[LEG].translation);
    assert_eq!(missing.0[ARM].translation, joined.0[ARM].translation);
}

#[test]
fn choking_pose_repeats_the_authored_period_and_keeps_moving() {
    let clips = [clip(9, "human_choking", 16, false)];
    let skeleton = skel();
    let mut animator = Animator::new();
    let mut samples = Vec::new();
    for time in [0.25, 1.25, 3.25, 3.3] {
        assert!(animator.status_pose(&clips, &skeleton, "human", time));
        samples.push(animator.kept_pose(&skeleton).0[ARM].translation);
    }
    assert_eq!(samples[0], samples[1]);
    assert_eq!(samples[0], samples[2]);
    assert_ne!(samples[2], samples[3]);
}

impl Run {
    fn new(rig: Rig) -> Self {
        Self {
            rig,
            clips: clips(),
            skel: skel(),
            a: Animator::new(),
        }
    }

    fn steps(&mut self, i: &AnimInput, n: usize) {
        for _ in 0..n {
            self.a.step(&self.rig, &self.clips, &self.skel, i, DT);
        }
    }

    fn shoot(&mut self, i: &AnimInput) {
        let mut f = *i;
        f.fired = true;
        self.steps(&f, 1);
    }

    /// The clip number a bone's pose was sampled from.
    fn clip_of(&self, bone: usize) -> usize {
        let x = self.a.pose(&self.skel).0[bone].translation.x;
        (x / 100.0).floor() as usize - 1
    }
}

#[test]
fn bf1_sprinting_keeps_the_run_animation_above_eight_metres_per_second() {
    for speed in [3.0, 7.9, 8.1, 9.75, 10.5] {
        for scale in [1.0, 1.15] {
            for (set, expected) in [(0, RUN), (1, BAZOOKA_RUN)] {
                for alert in [false, true] {
                    let mut r = Run::new(rig_bf1());
                    let sprint = AnimInput {
                        stance: Stance::Sprint,
                        root_scale: scale,
                        set,
                        alert,
                        ..input(speed)
                    };
                    r.steps(&input(0.0), 30);
                    r.steps(&sprint, 30);
                    assert_eq!(
                        r.a.body(),
                        Some(Body::Feet(0, Move::RunForward)),
                        "speed {speed}, scale {scale}, set {set}, alert {alert}"
                    );
                    assert_eq!(r.a.lower_clip(), Some(RUN));
                    assert_eq!(r.a.upper_clip(), Some(expected));
                    assert_eq!(r.clip_of(LEG), RUN);
                    assert_eq!(r.clip_of(ARM), expected);
                    let phase = r.a.lower_phase();
                    r.steps(&sprint, 1);
                    assert_ne!(r.a.lower_phase(), phase);
                    r.steps(
                        &AnimInput {
                            vel: glam::Vec2::ZERO,
                            ..sprint
                        },
                        30,
                    );
                    assert_eq!(r.a.body(), Some(Body::Feet(0, Move::Idle)));
                    r.steps(&sprint, 30);
                    assert_eq!(r.a.lower_clip(), Some(RUN));
                    r.steps(&input(0.0), 30);
                    assert_eq!(r.a.body(), Some(Body::Feet(0, Move::Idle)));
                }
            }
        }
    }
}

#[test]
fn baked_soldier_weapon_sets_keep_moving_during_sprint() {
    let root = crate::assets::baked_root();
    for game in ["bf1", "bf2"] {
        let side = root.join(format!("side_{game}"));
        if !side.join("soldiers").exists() || !root.join("classes").exists() {
            eprintln!("skip: no {game} soldier bake");
            continue;
        }
        let db =
            crate::roster::load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"])
                .unwrap();
        let mut assets = BTreeMap::new();
        let mut rows = 0;
        let mut classes: Vec<_> = db.names().collect();
        classes.sort_unstable();
        for class in classes {
            if !crate::roster::is_soldier_chain(&db, class) {
                continue;
            }
            let Some(geometry) = db.prop(class, "GeometryName") else {
                continue;
            };
            let geometry = geometry.trim().to_ascii_lowercase();
            let path = side.join("soldiers").join(format!("{geometry}.glb"));
            if !path.exists() || !manifest::anim_sets::path(&side, &geometry).exists() {
                continue;
            }
            let (asset, rig) = assets.entry(geometry.clone()).or_insert_with(|| {
                let asset = crate::assets::load_skinned_glb(&path).unwrap();
                let rig = crate::soldier_vis::SoldierVisuals::load_rig(&side, &geometry, &asset)
                    .expect("baked soldier rig");
                (asset, rig)
            });
            let slots = db.weapon_slots(class);
            let anim = crate::roster::class_anim(&db, class, &slots);
            let max_speed = db.prop_f32(class, "MaxSpeed").unwrap_or(7.0);
            for slot in &slots {
                let set = rig.weapon_set(&anim.family, anim.bank_of(&slot.weapon));
                let mut a = Animator::new();
                let i = AnimInput {
                    stance: Stance::Sprint,
                    max_speed,
                    root_scale: anim.root_scale,
                    set,
                    ..input(1.5 * max_speed)
                };
                for _ in 0..30 {
                    a.step(rig, &asset.clips, &asset.skeleton, &i, DT);
                }
                for clip in [a.lower_clip(), a.upper_clip()] {
                    let name = &asset.clips[clip.expect("a sprint clip")].name;
                    assert!(
                        name.contains("run") || name.contains("sprint"),
                        "{game} {class} {}: {name}",
                        slot.weapon
                    );
                }
                let phase = a.lower_phase();
                a.step(rig, &asset.clips, &asset.skeleton, &i, DT);
                assert_ne!(a.lower_phase(), phase, "{game} {class} {}", slot.weapon);
                if game == "bf2" {
                    let clip = a.lower_clip().unwrap();
                    let speed = rig.clip_speed(clip).map_or(11.5, |v| v[1].abs());
                    let speed = if speed > 0.0 { speed } else { 11.5 };
                    let expected = (phase
                        + DT * i.speed() / speed * 30.0
                            / asset.clips[clip].frames.saturating_sub(1).max(1) as f32)
                        .rem_euclid(1.0);
                    assert!(
                        (a.lower_phase() - expected).abs() < 0.0001,
                        "{class}: {} != {expected}",
                        a.lower_phase()
                    );
                }
                rows += 1;
            }
        }
        assert!(rows > 0, "no {game} sprint rows checked");
        println!("{game}: {rows} soldier/weapon sprint rows keep moving");
    }
}

#[test]
fn bf1_jump_changes_pose_after_three_tenths_and_plays_its_landing() {
    let root = crate::assets::baked_root().join("side_bf1");
    let path = root.join("soldiers/rep_inf_trooper.glb");
    if !path.exists() {
        eprintln!("skip: no BF1 clone bake");
        return;
    }
    let asset = crate::assets::load_skinned_glb(&path).unwrap();
    let rig = crate::soldier_vis::SoldierVisuals::load_rig(&root, "rep_inf_trooper", &asset)
        .expect("clone rig");
    let mut a = Animator::new();
    let mut i = input(0.0);
    let dt = 1.0 / 30.0;
    a.step(&rig, &asset.clips, &asset.skeleton, &i, dt);
    i.stance = Stance::Jump;
    let mut clock = 0.0f32;
    let mut poses = Vec::new();
    for _ in 0..27 {
        a.step(&rig, &asset.clips, &asset.skeleton, &i, dt);
        let expected = if clock > 0.3 {
            Special::Fall
        } else {
            Special::Jump
        };
        assert_eq!(a.body(), Some(Body::Special(expected)), "at {clock}");
        let expected = format!("human_rifle_{}", expected.name());
        for clip in [a.lower_clip(), a.upper_clip()] {
            assert_eq!(asset.clips[clip.unwrap()].name, expected);
        }
        poses.push(a.kept_pose(&asset.skeleton));
        clock += dt;
    }
    assert_ne!(poses[3], poses[8]);
    assert_ne!(poses[11], poses[20]);
    i.stance = Stance::Stand;
    for n in 0..17 {
        a.step(&rig, &asset.clips, &asset.skeleton, &i, dt);
        assert_eq!(
            a.body(),
            Some(Body::Special(Special::Landing(false))),
            "landing {n}"
        );
        assert_eq!(
            asset.clips[a.lower_clip().unwrap()].name,
            "human_rifle_landsoft"
        );
    }
    a.step(&rig, &asset.clips, &asset.skeleton, &i, dt);
    assert_eq!(a.body(), Some(Body::Feet(0, Move::Idle)));
    i.stance = Stance::Jump;
    a.step(&rig, &asset.clips, &asset.skeleton, &i, dt);
    assert_eq!(a.body(), Some(Body::Special(Special::Jump)));
}

#[test]
fn bf1_jump_blends_with_the_ps2s_thirty_update_weights() {
    let mut rig = rig_bf1();
    rig.sets.bf1.as_mut().unwrap().special[0] = Some(SHOOT_LEGS as u16);
    rig.sets.clips[SHOOT_LEGS].blend = Some(0.4);
    let mut r = Run::new(rig);
    r.clips[SHOOT_LEGS] = clip(SHOOT_LEGS, "shoot_legs", 1, false);
    r.steps(&input(0.0), 30);
    let i = AnimInput {
        stance: Stance::Jump,
        ..input(0.0)
    };
    let mut expected = r.a.kept_pose(&r.skel).0[LEG].translation.x;
    let mut upper = r.a.kept_pose(&r.skel).0[ARM].translation.x;
    for n in 1..=6 {
        let dt = 1.0 / 30.0;
        r.a.step(&r.rig, &r.clips, &r.skel, &i, dt);
        expected += (700.0 - expected) * (n as f32 * dt / 0.4);
        upper += (700.0 - upper) * ((n - 1) as f32 * dt / 0.4);
        let got = r.a.kept_pose(&r.skel).0[LEG].translation.x;
        assert!(
            (got - expected).abs() < 1e-3,
            "update {n}: {got} != {expected}"
        );
        let got = r.a.kept_pose(&r.skel).0[ARM].translation.x;
        assert!(
            (got - upper).abs() < 1e-3,
            "upper update {n}: {got} != {upper}"
        );
    }
}

#[test]
fn bf1_running_landing_can_end_after_point_fifteen_seconds_but_hard_landing_cannot() {
    for (air_updates, hard) in [(27, false), (63, true)] {
        let mut rig = rig_bf1();
        let table = rig.sets.bf1.as_mut().unwrap();
        table.special[0] = Some(SHOOT_LEGS as u16);
        table.special[4] = Some(SHOOT_LEGS as u16);
        table.special[if hard { 3 } else { 2 }] = Some(RELOAD as u16);
        let mut r = Run::new(rig);
        let airborne = AnimInput {
            stance: Stance::Jump,
            ..input(0.0)
        };
        for _ in 0..air_updates {
            r.a.step(&r.rig, &r.clips, &r.skel, &airborne, 1.0 / 30.0);
        }
        for _ in 0..5 {
            r.a.step(&r.rig, &r.clips, &r.skel, &input(6.5), 1.0 / 30.0);
            assert_eq!(r.a.body(), Some(Body::Special(Special::Landing(hard))));
        }
        r.a.step(&r.rig, &r.clips, &r.skel, &input(6.5), 1.0 / 30.0);
        assert_eq!(
            r.a.body(),
            Some(if hard {
                Body::Special(Special::Landing(true))
            } else {
                Body::Feet(0, Move::RunForward)
            })
        );
    }
}

#[test]
fn stance_table() {
    let at = |stance| {
        let mut i = input(3.0);
        i.stance = stance;
        stance_of(&i, Game::Bf1)
    };
    assert_eq!(at(Stance::Stand), Ok(0));
    assert_eq!(at(Stance::Crouch), Ok(1));
    assert_eq!(at(Stance::Prone), Ok(2));
    assert_eq!(at(Stance::Sprint), Ok(0));
    assert_eq!(at(Stance::Roll), Err(Special::Roll));
    assert_eq!(at(Stance::Jump), Err(Special::Jump));
    let mut d = input(5.0);
    d.dead = true;
    assert_eq!(stance_of(&d, Game::Bf1), Err(Special::Dead));
    // BF2 sprints on its own pair; BF1 on the run clip.
    let mut s = input(8.0);
    s.stance = Stance::Sprint;
    assert_eq!(stance_of(&s, Game::Bf2), Err(Special::Sprint));
    let mut r = Run::new(rig_bf1());
    r.steps(
        &AnimInput {
            vel: glam::Vec2::new(0.0, 3.0),
            ..s
        },
        10,
    );
    assert_eq!(r.a.body(), Some(Body::Feet(0, Move::RunForward)));
}

#[test]
fn a_shot_on_the_run_keeps_the_legs_running() {
    for rig in [rig_bf1(), rig_bf2()] {
        let mut r = Run::new(rig);
        let run = input(6.5);
        r.steps(&run, 30);
        assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (RUN, RUN));
        r.shoot(&run);
        r.steps(&run, 2);
        // One pose: the leg from the run clip, the arm from the shoot clip.
        assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (RUN, SHOOT));
        assert_eq!(r.a.lower_clip(), Some(RUN));
        assert_eq!(r.a.action(), Some((ActionKind::Shoot, false)));
    }
}

#[test]
fn a_shot_standing_takes_the_legs_too() {
    // BF1: the same clip on both players.
    let mut r = Run::new(rig_bf1());
    r.steps(&input(0.0), 30);
    r.shoot(&input(0.0));
    assert_eq!(
        (r.a.upper_clip(), r.a.lower_clip()),
        (Some(SHOOT), Some(SHOOT))
    );
    assert_eq!(r.clip_of(ARM), SHOOT);
    // BF2: the action's lower slot, crossfaded over its 0.5 s.
    let mut r = Run::new(rig_bf2());
    r.steps(&input(0.0), 30);
    r.shoot(&input(0.0));
    assert_eq!(
        (r.a.upper_clip(), r.a.lower_clip()),
        (Some(SHOOT), Some(SHOOT_LEGS))
    );
    r.steps(&input(0.0), 2);
    let x = r.a.pose(&r.skel).0[LEG].translation.x;
    assert!(x > 131.0 && x < 700.0, "the legs are on their way: {x}");
    // Once he moves the legs leave the action, and do not rejoin it.
    r.steps(&input(6.5), 1);
    assert_eq!(r.a.action(), Some((ActionKind::Shoot, false)));
    r.steps(&input(0.0), 1);
    assert_eq!(r.a.lower_clip(), Some(IDLE));
}

#[test]
fn the_action_ends_with_its_clip_and_a_second_shot_restarts_it() {
    for rig in [rig_bf1(), rig_bf2()] {
        let bf2 = rig.game == Game::Bf2;
        let mut r = Run::new(rig);
        let run = input(6.5);
        r.steps(&run, 10);
        r.shoot(&run);
        // 7 frames at 30 a second: 14 updates (BF2: 6 intervals, 12).
        let n = if bf2 { 12 } else { 14 };
        r.steps(&run, n - 4);
        assert!(r.a.action().is_some(), "still shooting");
        let late = r.a.upper.t;
        r.shoot(&run);
        assert!(r.a.upper.t < late, "restarted: {} < {late}", r.a.upper.t);
        r.steps(&run, n - 2);
        assert!(r.a.action().is_some(), "the second shot's clip is running");
        r.steps(&run, 3);
        assert_eq!(r.a.action(), None);
        assert_eq!(r.a.upper_clip(), Some(RUN));
        r.steps(&run, 30);
        assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (RUN, RUN));
    }
}

#[test]
fn the_weapon_in_hand_picks_the_upper_clip() {
    let rig = rig_bf1();
    assert_eq!(rig.weapon_set("", "bazooka"), 1);
    assert_eq!(rig.weapon_set("", "lightsabre"), 5);
    assert_eq!(rig.weapon_set("", "no_such"), 0);
    let mut r = Run::new(rig);
    let mut run = input(6.5);
    run.set = 1;
    r.steps(&run, 30);
    assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (RUN, BAZOOKA_RUN));
    // The upper clip started at the legs' phase and keeps it.
    assert!((r.a.upper.t - r.a.lower.t).abs() < 1e-4);
    // A gait the set has no clip for takes the other one of its direction
    // (BF2 ships no stand_walkbackward): here backward has neither -> idle.
    let (_, low) = r
        .rig
        .body_slots(0, Body::Feet(0, Move::WalkBackward), false);
    assert_eq!(low.map(|s| s.clip), Some(IDLE));
    let mut t = r.rig.clone();
    t.sets.bf1.as_mut().unwrap().base[0][8] = Some(RUN as u16);
    let (_, low) = t.body_slots(0, Body::Feet(0, Move::WalkBackward), false);
    assert_eq!(low.map(|s| s.clip), Some(RUN));
    // BF2: the text after the underscore names the class; no match = rifle.
    let rig = rig_bf2();
    assert_eq!(rig.weapon_set("human", "human_rifle"), 0);
    assert_eq!(rig.weapon_set("wookie", "bazooka"), 0);
    assert_eq!(rig.set_name(0), "human_rifle");
}

#[test]
fn a_reload_starts_once_and_bf2_fits_it_to_the_weapon() {
    let mut r = Run::new(rig_bf2());
    let mut i = input(6.5);
    (i.reloading, i.reload_time) = (true, 2.0);
    r.steps(&i, 60);
    // 31 frames = 1.0 s of clip over a 2.0 s reload: half way after 1 s.
    assert_eq!(r.a.action(), Some((ActionKind::Reload, false)));
    assert!((r.a.upper.t - 0.5).abs() < 0.03, "{}", r.a.upper.t);
    r.steps(&i, 65);
    assert_eq!(r.a.action(), None, "ended with its clip, not restarted");
    assert_eq!((r.clip_of(LEG), r.a.upper_clip()), (RUN, Some(RUN)));
}

#[test]
fn bf2_crossfades_linearly_and_bf1_pulls_the_kept_pose() {
    // BF2 idle -> run on the legs: T = min(in 0.2, out 0.2); half way at 0.1 s.
    let mut r = Run::new(rig_bf2());
    r.steps(&input(0.0), 1);
    let from = r.a.pose(&r.skel).0[LEG].translation.x;
    r.steps(&input(6.5), 6);
    let (x, to) = (r.a.pose(&r.skel).0[LEG].translation.x, 200.0);
    let k = (x - from) / (to - from);
    assert!((k - 0.5).abs() < 0.06, "linear: {k} at T / 2");
    r.steps(&input(6.5), 7);
    assert_eq!(r.clip_of(LEG), RUN);
    // BF1: a pull by t / T each update is past a linear fade's half at T / 2.
    let mut r = Run::new(rig_bf1());
    r.steps(&input(0.0), 1);
    let from = r.a.pose(&r.skel).0[LEG].translation.x;
    r.steps(&input(6.5), 5);
    let k = (r.a.pose(&r.skel).0[LEG].translation.x - from) / (200.0 - from);
    assert!(k > 0.6 && k < 1.0, "pulled: {k}");
    r.steps(&input(6.5), 5);
    assert_eq!(r.clip_of(LEG), RUN);
}

#[test]
fn bf1_pull_is_the_same_at_any_frame_rate() {
    let pose_after = |dt: f32, n: usize| {
        let mut r = Run::new(rig_bf1());
        r.steps(&input(0.0), 1);
        // A non-looping target so the sampled frame does not matter: reload
        // on a standing soldier, read on the arm.
        let mut i = input(0.0);
        i.reloading = true;
        for _ in 0..n {
            r.a.step(&r.rig, &r.clips, &r.skel, &i, dt);
        }
        (r.a.pose(&r.skel).0[ARM].translation.x - 100.0) / 400.0
    };
    let (a, b) = (pose_after(1.0 / 60.0, 4), pose_after(1.0 / 120.0, 8));
    assert!((a - b).abs() < 0.05, "60 Hz {a} against 120 Hz {b}");
}

#[test]
fn playback_rate_per_game_and_loop_wrap() {
    // BF1: a 19-frame loop takes 19/30 s; after 18/30 s t = 18/19.
    let mut r = Run::new(rig_bf1());
    let run = input(6.5);
    for _ in 0..18 {
        r.a.step(&r.rig, &r.clips, &r.skel, &run, 1.0 / 30.0);
    }
    assert!((r.a.lower.t - 18.0 / 19.0).abs() < 1e-4, "{}", r.a.lower.t);
    r.a.step(&r.rig, &r.clips, &r.skel, &run, 1.0 / 30.0);
    assert!(r.a.lower.t < 1e-3 || r.a.lower.t > 0.999, "wrapped");
    // BF2: exactly 30 fps over F-1 intervals.
    let mut r = Run::new(rig_bf2());
    for _ in 0..9 {
        r.a.step(&r.rig, &r.clips, &r.skel, &run, 1.0 / 30.0);
    }
    assert!((r.a.lower.t - 0.5).abs() < 1e-4);
    assert_eq!(find_clip(&r.clips, "reload"), Some(RELOAD));
}

#[test]
fn bf1_alert_plays_the_alert_row_until_the_timer_runs_out() {
    let mut rig = rig_bf1();
    let t = rig.sets.bf1.as_mut().unwrap();
    // The rifle's alert idle; the bazooka's row is the rifle's (the bake's
    // fallback), the tool's too - but the tool set takes no alert.
    for w in [0, 1, 4] {
        t.alert[w][0] = Some(ALERT as u16);
    }
    t.w[0][4] = t.base[0].clone();
    let mut r = Run::new(rig);
    let mut i = input(0.0);
    r.steps(&i, 30);
    assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (IDLE, IDLE));
    i.alert = true;
    r.steps(&i, 60);
    assert!(r.a.aim().alert());
    assert_eq!(
        (r.a.lower_clip(), r.a.upper_clip()),
        (Some(ALERT), Some(ALERT))
    );
    assert_eq!((r.clip_of(LEG), r.clip_of(ARM)), (ALERT, ALERT));
    // Running: the row has no run clip, the plain one plays.
    r.steps(
        &AnimInput {
            vel: glam::Vec2::new(0.0, 6.5),
            ..i
        },
        30,
    );
    assert_eq!((r.a.lower_clip(), r.a.upper_clip()), (Some(RUN), Some(RUN)));
    // The reason gone: 2.0 s more of it, then the plain idle.
    i.alert = false;
    r.steps(&i, 110);
    assert_eq!(r.a.upper_clip(), Some(ALERT));
    r.steps(&i, 20);
    assert_eq!(
        (r.a.lower_clip(), r.a.upper_clip()),
        (Some(IDLE), Some(IDLE))
    );
    // A shot alone makes him alert.
    r.shoot(&i);
    r.steps(&i, 30);
    assert_eq!(r.a.upper_clip(), Some(ALERT));
    // The tool in hand: never.
    let mut tool = Run::new(r.rig.clone());
    i.alert = true;
    i.set = 4;
    tool.steps(&i, 60);
    assert!(!tool.a.aim().alert());
    assert_eq!(tool.a.upper_clip(), Some(IDLE));
}

#[test]
fn bf2_alert_plays_the_alert_block_and_its_clip_aims_the_torso() {
    let mut rig = rig_bf2();
    let (plain, alert) = (bf2::move_pair(0, 0), bf2::move_pair(3, 0));
    let slots = rig.sets.bf2[0].sets.get_mut("rifle").unwrap();
    (slots[2 * alert], slots[2 * alert + 1]) = (Some(ALERT as u16), Some(ALERT as u16));
    for (p, aim) in [(plain, "Head"), (alert, "Torso")] {
        rig.sets.bf2_animations[p].upper.as_mut().unwrap().aim = aim.into();
    }
    let mut r = Run::new(rig);
    let mut i = input(0.0);
    i.aim = (0.0, 0.5);
    r.steps(&i, 30);
    assert_eq!(r.a.upper_clip(), Some(IDLE));
    assert_eq!(r.a.aim().weights(), (0.0, 1.0));
    i.alert = true;
    r.steps(&i, 30);
    assert_eq!(
        (r.a.lower_clip(), r.a.upper_clip()),
        (Some(ALERT), Some(ALERT))
    );
    assert_eq!(r.a.aim().weights(), (1.0, 0.0));
    i.alert = false;
    r.steps(&i, 140);
    assert_eq!(r.a.upper_clip(), Some(ALERT), "2.5 s of it");
    r.steps(&i, 40);
    assert_eq!(
        (r.a.lower_clip(), r.a.upper_clip()),
        (Some(IDLE), Some(IDLE))
    );
    assert_eq!(r.a.aim().weights(), (0.0, 1.0));
}

/// Card ROLLS: the travel angle is the velocity against the facing the body
/// is DRAWN with (`soldier_vis`'s model turn), positive to his left - the
/// PS2's row 0 - so a body turned by it dives head first along the velocity.
#[test]
fn travel_is_the_velocity_against_the_drawn_facing() {
    for yaw in [0.0f32, 1.0, -2.5, 3.0] {
        let model = Mat4::from_rotation_y(yaw + std::f32::consts::PI);
        let (fwd, left) = (
            model.transform_vector3(Vec3::Z),
            model.transform_vector3(Vec3::X),
        );
        // The sim's own right (`Unit::aim_frame`) is the drawn left's opposite.
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        assert!((right + left).length() < 1e-5, "{yaw}");
        for (v, want) in [
            (left, 90.0f32),
            (-left, -90.0),
            (fwd, 0.0),
            (fwd + left, 45.0),
            (fwd * 0.8 - left * 0.6, -36.87),
        ] {
            let vel = v.normalize() * 10.5 + Vec3::Y * 3.0;
            let t = travel(vel, yaw).expect("moving");
            assert!(
                (t.to_degrees() - want).abs() < 0.01,
                "yaw {yaw}: {} not {want}",
                t.to_degrees()
            );
            // The dive's forward (+Z) turned by the travel, drawn: along the
            // velocity.
            let dive = model.transform_vector3(Quat::from_rotation_y(t) * Vec3::Z);
            let flat = Vec3::new(vel.x, 0.0, vel.z).normalize();
            assert!(
                dive.dot(flat) > 0.9999,
                "yaw {yaw} want {want}: dive {dive} vel {flat}"
            );
        }
        assert_eq!(travel(left * 0.25 + Vec3::Y * 5.0, yaw), None);
    }
}

fn roll(t: f32) -> AnimInput {
    AnimInput {
        stance: Stance::Roll,
        vel: 10.5 * glam::Vec2::new(t.to_radians().sin(), t.to_radians().cos()),
        ..input(10.5)
    }
}

/// The root bone's yaw in the drawn pose (degrees, his left positive).
fn root_yaw(r: &Run) -> f32 {
    let q = r.a.pose(&r.skel).0[0].rotation;
    let f = q * Vec3::Z;
    f.x.atan2(f.z).to_degrees()
}

/// Card ROLLS: a roll turns `DummyRoot` toward its travel by the PS2's
/// quarter of the rest an update (re-roll-picture 4.1: 76.0 is BF2's first
/// update from a strafe twist of +71; from 0 it is 22.5), within 15 degrees
/// from the 9th update and 2 at the 26th; a forward roll does not turn it.
#[test]
fn a_roll_turns_the_root_toward_its_travel() {
    const STEP: f32 = 1.0 / 30.0;
    for rig in [rig_bf1(), rig_bf2()] {
        for want in [90.0f32, -90.0, 37.9, -37.5, 0.0] {
            let mut r = Run::new(rig.clone());
            r.steps(&input(0.0), 5);
            for n in 1..=26 {
                r.a.step(&r.rig, &r.clips, &r.skel, &roll(want), STEP);
                let eased = want * (1.0 - 0.75f32.powi(n));
                let tw = r.a.twist().to_degrees();
                assert!(
                    (tw - eased).abs() < 1e-3,
                    "{:?} {want} update {n}: {tw} not {eased}",
                    rig.game
                );
                assert!(
                    (root_yaw(&r) - tw).abs() < 1e-3,
                    "drawn {} twist {tw}",
                    root_yaw(&r)
                );
                if n >= 9 {
                    assert!((want - tw).abs() < 15.0, "{want} update {n}: {tw}");
                }
                if want == 0.0 {
                    assert!(tw.abs() < 1e-6);
                }
            }
            assert!((want - r.a.twist().to_degrees()).abs() < 2.0);
            assert_eq!(r.a.roll().updates, 26);
            // Up again: the same quarter back to 0, under 10 degrees in 0.5 s.
            r.a.step(&r.rig, &r.clips, &r.skel, &input(3.0), STEP);
            let gap = r.a.roll().ended.expect("the roll ended").to_degrees();
            assert!(gap.abs() < 0.1, "{gap}");
            for _ in 1..15 {
                r.a.step(&r.rig, &r.clips, &r.skel, &input(3.0), STEP);
            }
            assert!(r.a.roll().after.is_some_and(|t| (t - 0.5).abs() < 1e-4));
            assert!(
                r.a.twist().to_degrees().abs() < 10.0,
                "{}",
                r.a.twist().to_degrees()
            );
        }
    }
}

/// The twist eases by time, not by update; a slow tick keeps the last
/// travel.
#[test]
fn the_twist_is_frame_rate_free() {
    let mut at30 = Run::new(rig_bf2());
    let mut at60 = Run::new(rig_bf2());
    for _ in 0..13 {
        at30.a
            .step(&at30.rig, &at30.clips, &at30.skel, &roll(90.0), 1.0 / 30.0);
    }
    at60.steps(&roll(90.0), 26);
    assert!((at30.a.twist() - at60.a.twist()).abs() < 1e-4);
    // Too slow to tell: the target stays where it was.
    let slow = AnimInput {
        vel: glam::Vec2::ZERO,
        ..roll(0.0)
    };
    at60.steps(&slow, 60);
    assert!((at60.a.twist().to_degrees() - 90.0).abs() < 0.01);
}

/// Card ANIMM acceptance 1: the movement chooser against the PS2's rule
/// (re-soldier-anim-layers 3.5, 7; re-foley 8.2's live rows for the alert
/// strafes). `v` is (his left, ahead) m/s; `prev` the clip playing.
#[test]
fn the_chooser_is_the_ps2s() {
    use glam::Vec2;
    let deg = |a: f32| a.to_degrees();
    let pick = |game, v: Vec2, alert, prev: Legs| {
        let mut l = prev;
        choose(game, v, 1.0, false, alert, 7.0, &mut l).map(|(m, t)| (m, deg(t)))
    };
    let from = |mv| Legs {
        mv,
        ..Legs::default()
    };
    let idle = Legs::default();
    // BF1, alert: a left strafe is outside -103.5 .. +67.5 and runs
    // backward with the legs at -45 (limited); a right strafe walks
    // forward, legs at -45 too (re-foley 8.2: left run, right walk).
    let (m, t) = pick(Game::Bf1, Vec2::new(4.4, 0.0), true, idle).unwrap();
    assert_eq!(m, Move::RunBackward);
    assert!((t + 45.0).abs() < 0.01, "{t}");
    let (m, t) = pick(Game::Bf1, Vec2::new(-4.5, 0.0), true, idle).unwrap();
    assert_eq!(m, Move::WalkForward);
    assert!((t + 45.0).abs() < 0.01, "{t}");
    // ... at 70 degrees left, alert: backward (past +67.5); plain: forward.
    let v70 = 4.0 * Vec2::new(70f32.to_radians().sin(), 70f32.to_radians().cos());
    assert_eq!(
        pick(Game::Bf1, v70, true, idle).unwrap().0,
        Move::RunBackward
    );
    assert_eq!(
        pick(Game::Bf1, v70, false, idle).unwrap().0,
        Move::WalkForward
    );
    // Not alert: both strafes run backward from a stand, legs at -+72; a
    // soldier already walking forward keeps it to 108 degrees.
    let (m, t) = pick(Game::Bf1, Vec2::new(4.4, 0.0), false, idle).unwrap();
    assert_eq!((m, t.round()), (Move::RunBackward, -72.0));
    let (m, t) = pick(Game::Bf1, Vec2::new(-4.4, 0.0), false, idle).unwrap();
    assert_eq!((m, t.round()), (Move::RunBackward, 72.0));
    let walking = from(Move::WalkForward);
    let (m, t) = pick(Game::Bf1, Vec2::new(4.4, 0.0), false, walking).unwrap();
    assert_eq!((m, t.round()), (Move::WalkForward, 72.0));
    let v100 = 4.0 * Vec2::new(100f32.to_radians().sin(), 100f32.to_radians().cos());
    assert_eq!(
        pick(Game::Bf1, v100, false, walking).unwrap().0,
        Move::WalkForward
    );
    assert_eq!(
        pick(Game::Bf1, v100, false, idle).unwrap().0,
        Move::RunBackward
    );
    // Walk or run ahead: 5.5 to start running, 5.0 to stay.
    let ahead = |s| Vec2::new(0.0, s);
    assert_eq!(
        pick(Game::Bf1, ahead(5.2), false, idle).unwrap().0,
        Move::WalkForward
    );
    assert_eq!(
        pick(Game::Bf1, ahead(5.6), false, idle).unwrap().0,
        Move::RunForward
    );
    let running = from(Move::RunForward);
    assert_eq!(
        pick(Game::Bf1, ahead(5.2), false, running).unwrap().0,
        Move::RunForward
    );
    assert_eq!(
        pick(Game::Bf1, ahead(4.9), false, running).unwrap().0,
        Move::WalkForward
    );
    // Backward always asks for the run clip; idle under 0.3 on both axes
    // and over 8 m/s.
    assert_eq!(
        pick(Game::Bf1, ahead(-1.0), false, idle).unwrap().0,
        Move::RunBackward
    );
    assert_eq!(pick(Game::Bf1, Vec2::new(0.29, 0.29), false, idle), None);
    assert_eq!(pick(Game::Bf1, ahead(8.1), false, running), None);
    // BF2: the speed mode, 0.78 of MaxSpeed (7.0 -> 5.46) to run, 0.71
    // (4.97) to stay.
    let mut l = idle;
    let mut at = |s: f32| choose(Game::Bf2, ahead(s), 1.0, false, false, 7.0, &mut l).map(|c| c.0);
    assert_eq!(at(5.4), Some(Move::WalkForward));
    assert_eq!(at(5.5), Some(Move::RunForward));
    assert_eq!(at(5.0), Some(Move::RunForward));
    assert_eq!(at(4.9), Some(Move::WalkForward));
    assert_eq!(at(-2.0), Some(Move::WalkBackward));
    // Idle under 0.3 from a stand, 0.2 once moving (the caller keeps `mv`).
    assert_eq!(at(0.25), None);
    let mut l = Legs {
        mv: Move::WalkForward,
        ..idle
    };
    assert_eq!(
        choose(Game::Bf2, ahead(0.25), 1.0, false, false, 7.0, &mut l).map(|c| c.0),
        Some(Move::WalkForward)
    );
    assert_eq!(
        choose(Game::Bf2, ahead(0.15), 1.0, false, false, 7.0, &mut l),
        None
    );
    // BF2's forward bit is up from a stand: a strafe takes the wide window
    // and the forward clip, legs at +-72 (re-roll-picture 4.1's +71).
    let (m, t) = pick(Game::Bf2, Vec2::new(4.4, 0.0), false, idle).unwrap();
    assert_eq!((m, t.round()), (Move::WalkForward, 72.0));
    let back = Legs {
        bf2_forward: false,
        ..idle
    };
    let (m, t) = pick(Game::Bf2, Vec2::new(4.4, 0.0), false, back).unwrap();
    assert_eq!((m, t.round()), (Move::WalkBackward, -72.0));
    // Alert: BF1's window and limits.
    let (m, t) = pick(Game::Bf2, Vec2::new(4.4, 0.0), true, idle).unwrap();
    assert_eq!((m, t.round()), (Move::WalkBackward, -45.0));
    let (m, t) = pick(Game::Bf2, Vec2::new(-4.4, 0.0), true, idle).unwrap();
    assert_eq!((m, t.round()), (Move::WalkForward, -45.0));
}

/// A stance with strafe clips (BF1 prone) strafes on them: left walks
/// under 5.0 (5.5 while walking left), right always asks for the run.
#[test]
fn a_stance_with_strafe_clips_strafes() {
    let mut l = Legs::default();
    let v = glam::Vec2::new(1.0, 0.2);
    let (m, t) = choose(Game::Bf1, v, 1.0, true, false, 7.0, &mut l).unwrap();
    assert_eq!(m, Move::WalkLeft);
    assert!((t + 0.2f32.atan2(1.0)).abs() < 1e-5, "{t}");
    let r = choose(Game::Bf1, -v, 1.0, true, false, 7.0, &mut l).unwrap();
    assert_eq!(r.0, Move::RunRight);
    // Mostly ahead: the forward clip, the whole angle on the legs.
    let (m, t) = choose(
        Game::Bf1,
        glam::Vec2::new(1.0, 1.5),
        1.0,
        true,
        false,
        7.0,
        &mut l,
    )
    .unwrap();
    assert_eq!(m, Move::WalkForward);
    assert!((t - 1.0f32.atan2(1.5)).abs() < 1e-5);
}

/// A movement clip plays at 0.95 v / v_clip + 0.05 of the clock: the
/// phase moves that much slower or faster than the plain dt (B1:145143).
#[test]
fn a_walk_plays_at_its_speed() {
    let mut sets_rig = rig_bf1();
    sets_rig.root[WALK] = Some([0.0, 3.687]);
    sets_rig.root[RUN] = Some([0.0, 6.338]);
    for (speed, clip, v) in [(2.0, WALK, 3.687), (4.5, WALK, 3.687), (7.0, RUN, 6.338)] {
        let mut r = Run::new(sets_rig.clone());
        r.steps(&input(speed), 30);
        assert_eq!(r.a.lower_clip(), Some(clip));
        let t0 = r.a.lower.t;
        r.steps(&input(speed), 1);
        let got = (r.a.lower.t - t0).rem_euclid(1.0);
        let plain = DT * 30.0 / 19.0;
        let want = plain * move_rate(speed, v);
        assert!((got - want).abs() < 1e-5, "{speed}: {got} not {want}");
        // The feet's ground speed against his: off by 5 % of the gap only.
        let feet = v * move_rate(speed, v);
        assert!((feet - (0.95 * speed + 0.05 * v)).abs() < 1e-4);
    }
}

/// A class's SkeletonRootScale (card USKN, re-soldier-skeleton-skins 7): the
/// chooser and the clip rate read v / scale - a 1.15 Wookiee walks at 6.0
/// m/s (5.2 < BF1's 5.5 run speed), idles at 0.33 (0.29 < 0.3) and plays
/// his walk at v / 1.15; BF2's speed mode stays on v against MaxSpeed.
#[test]
fn a_scaled_class_chooses_and_plays_on_v_over_the_scale() {
    let ahead = |s: f32| glam::Vec2::new(0.0, s);
    let pick = |game, v, scale| choose(game, v, scale, false, false, 7.0, &mut Legs::default());
    assert_eq!(
        pick(Game::Bf1, ahead(6.0), 1.0).unwrap().0,
        Move::RunForward
    );
    assert_eq!(
        pick(Game::Bf1, ahead(6.0), 1.15).unwrap().0,
        Move::WalkForward
    );
    assert_eq!(
        pick(Game::Bf1, ahead(0.33), 1.0).unwrap().0,
        Move::WalkForward
    );
    assert_eq!(pick(Game::Bf1, ahead(0.33), 1.15), None);
    assert_eq!(pick(Game::Bf2, ahead(0.33), 1.15), None);
    assert_eq!(
        pick(Game::Bf2, ahead(5.5), 1.15).unwrap().0,
        Move::RunForward
    );
    let mut sets_rig = rig_bf1();
    sets_rig.root[WALK] = Some([0.0, 3.687]);
    let (speed, v, scale) = (4.5, 3.687, 1.15);
    let i = AnimInput {
        root_scale: scale,
        ..input(speed)
    };
    let mut r = Run::new(sets_rig);
    r.steps(&i, 30);
    assert_eq!(r.a.lower_clip(), Some(WALK));
    let t0 = r.a.lower.t;
    r.steps(&i, 1);
    let got = (r.a.lower.t - t0).rem_euclid(1.0);
    let want = DT * 30.0 / 19.0 * move_rate(speed / scale, v);
    assert!((got - want).abs() < 1e-5, "{got} not {want}");
}

/// Standing, the legs keep their place while the body turns; past the
/// stance's limit the turn clip takes them 0.2 rad beyond the facing over
/// its length, then idles (B1:142792-142816).
#[test]
fn standing_he_turns_on_the_spot() {
    const STEP: f32 = 1.0 / 30.0;
    for (game, rig) in [(Game::Bf1, rig_bf1()), (Game::Bf2, rig_bf2())] {
        let mut r = Run::new(rig);
        let mut i = input(0.0);
        for _ in 0..10 {
            r.a.step(&r.rig, &r.clips, &r.skel, &i, STEP);
        }
        // Turn him right by 0.3 rad: the legs stay, the twist is +0.3.
        i.yaw = -0.3;
        r.a.step(&r.rig, &r.clips, &r.skel, &i, STEP);
        assert!((r.a.twist() - 0.3).abs() < 1e-5, "{game:?} {}", r.a.twist());
        assert_eq!(r.a.body(), Some(Body::Feet(0, Move::Idle)));
        // 0.1 more: past 0.3927, the turn right.
        i.yaw = -0.4;
        r.a.step(&r.rig, &r.clips, &r.skel, &i, STEP);
        assert_eq!(r.a.body(), Some(Body::Feet(0, Move::TurnRight)), "{game:?}");
        // The turn clip's 15 frames time it.
        assert_eq!(r.a.lower_clip(), Some(TURN));
        let len = match game {
            Game::Bf1 => 15.0,
            Game::Bf2 => 14.0,
        } / 30.0;
        let n = (len / STEP).ceil() as usize + 1;
        for _ in 0..n {
            r.a.step(&r.rig, &r.clips, &r.skel, &i, STEP);
        }
        assert_eq!(r.a.body(), Some(Body::Feet(0, Move::Idle)), "{game:?}");
        assert!((r.a.twist() + 0.2).abs() < 0.05, "{game:?} {}", r.a.twist());
        // To his left the limit is -0.7069: 0.6 is held.
        let mut l = Run::new(r.rig.clone());
        let mut j = input(0.0);
        l.a.step(&l.rig, &l.clips, &l.skel, &j, STEP);
        j.yaw = 0.6;
        l.a.step(&l.rig, &l.clips, &l.skel, &j, STEP);
        assert_eq!(l.a.body(), Some(Body::Feet(0, Move::Idle)));
        j.yaw = 0.75;
        l.a.step(&l.rig, &l.clips, &l.skel, &j, STEP);
        assert_eq!(l.a.body(), Some(Body::Feet(0, Move::TurnLeft)));
        assert!((l.a.twist() + j.yaw).abs() < 1e-5);
    }
}

/// Strafing, the root turns toward the walk and the aim pose turns the
/// torso back (`AimStep::aim` is the facing's yaw; the twist is taken off
/// after the easing).
#[test]
fn a_strafe_twists_the_legs_and_the_aim_turns_back() {
    let mut r = Run::new(rig_bf1());
    let i = AnimInput {
        vel: glam::Vec2::new(-3.0, 0.0),
        alert: true,
        ..input(0.0)
    };
    r.steps(&i, 120);
    // Alert, right strafe: the forward clip, legs at -45.
    assert_eq!(r.a.body(), Some(Body::Feet(0, Move::WalkForward)));
    assert!((r.a.twist().to_degrees() + 45.0).abs() < 0.01);
    assert!((root_yaw(&r) + 45.0).abs() < 0.01);
    assert!(r.a.aim().eased().0.abs() < 1e-6, "{:?}", r.a.aim().eased());
    let turn = r.a.aim().turn(r.a.twist()).0;
    assert!((turn.to_degrees() - 45.0).abs() < 0.01, "{turn}");
}

#[test]
fn a_changing_twist_does_not_swing_the_aim() {
    let mut r = Run::new(rig_bf1());
    let mut i = AnimInput {
        alert: true,
        ..input(0.0)
    };
    for k in 0..240 {
        let a = k as f32 * 0.05;
        i.vel = glam::Vec2::new(3.0 * a.sin(), 3.0 * a.cos());
        r.steps(&i, 1);
        let (yaw, _) = r.a.aim().turn(r.a.twist());
        assert!(
            (yaw + r.a.twist()).abs() < 1e-5,
            "update {k}: torso {yaw} against twist {}",
            r.a.twist()
        );
    }
}

/// Card KNOCK (re-knockback §2): a thrown, tumbling or getting-up body -
/// alive, or killed by the blast that threw it - plays the PS2's `thrown_*`
/// clips by its velocity against the body, and every one of them is in both
/// games' special tables (B1 0x517af0, BF2's simple pairs).
#[test]
fn thrown_states_take_the_thrown_clips() {
    let at = |stance, vel: glam::Vec2, dead| {
        let mut i = input(0.0);
        i.stance = stance;
        i.vel = vel;
        i.dead = dead;
        stance_of(&i, Game::Bf1)
    };
    let (fwd, back, left) = (
        glam::Vec2::Y * 9.0,
        -glam::Vec2::Y * 9.0,
        glam::Vec2::X * 9.0,
    );
    assert_eq!(at(Stance::Thrown, fwd, false), Err(Special::Flying(0)));
    assert_eq!(at(Stance::Thrown, back, true), Err(Special::Flying(1)));
    assert_eq!(at(Stance::Thrown, left, false), Err(Special::Flying(2)));
    assert_eq!(at(Stance::Thrown, -left, false), Err(Special::Flying(3)));
    assert_eq!(at(Stance::Tumble, back, true), Err(Special::Tumble(true)));
    assert_eq!(at(Stance::GetUp, fwd, false), Err(Special::Land(false)));
    assert_eq!(at(Stance::Dead, fwd, true), Err(Special::Dead));
    for sp in [
        Special::Flying(0),
        Special::Flying(1),
        Special::Flying(2),
        Special::Flying(3),
        Special::Tumble(false),
        Special::Tumble(true),
        Special::Land(false),
        Special::Land(true),
        Special::Rest(false),
        Special::Rest(true),
    ] {
        assert!(bf1::SPECIAL.contains(&sp.name()), "{}", sp.name());
        assert!(bf2::simple_pair(sp.name()).is_some(), "{}", sp.name());
    }
}

/// The census measures feet in world units, using the animator's actual clock.
#[test]
fn census_slide_accounts_for_root_scale_and_matches_playback() {
    for scale in [1.0, 1.15, 2.0, 0.0, -1.0] {
        let effective = if scale > 0.0 { scale } else { 1.0 };
        let speed = 3.0 * effective;
        let mut rig = rig_bf1();
        rig.root[WALK] = Some([3.0, 3.0]);
        let mut run = Run::new(rig);
        let i = AnimInput {
            root_scale: scale,
            ..input(speed)
        };
        run.steps(&i, 30);
        assert_eq!(run.a.lower_clip(), Some(WALK));
        let before = run.a.lower.t;
        run.steps(&i, 1);
        let rate = (run.a.lower.t - before).rem_euclid(1.0) / (DT * 30.0 / 19.0);
        assert!((rate * 3.0 * effective - speed).abs() < 1e-4);
        let mut census = super::census::Census::default();
        census.note(&run.rig, &run.clips, &run.a, &i);
        let report = census.report("scale");
        assert!(
            report.contains(
                "feet off his speed mean 0.0 % with the rate (0.0 % over 5 %), 0.0 % plain"
            ),
            "{report}"
        );
    }
}

/// Use the public rider path; the run clip is a distinct, animated seat marker.
fn seat_updates(r: &mut Run) {
    for frame in 0..4 {
        r.a.seat(&r.clips, &r.skel, RUN, None, DT);
        assert!(r.a.seated());
        let pose = r.a.pose(&r.skel);
        let expected = 200.0 + frame as f32 * DT * 30.0;
        assert!((pose.0[LEG].translation.x - expected).abs() < 1e-4);
        assert!(root_yaw(r).abs() < 1e-5);
    }
}

fn idle_exit(r: &mut Run, i: &AnimInput) {
    for _ in 0..30 {
        r.steps(i, 1);
        assert!(!r.a.seated());
        assert_eq!(r.a.body(), Some(Body::Feet(0, Move::Idle)));
        assert_eq!(r.a.lower_clip(), Some(IDLE));
        assert!(r.a.twist().abs() < 1e-5, "stale twist {}", r.a.twist());
        assert!(root_yaw(r).abs() < 1e-5);
        // Through the blend the lower pose stays between seat and idle,
        // never approaching the turn clip's marker (900).
        let x = r.a.pose(&r.skel).0[LEG].translation.x;
        assert!((100.0..=202.0).contains(&x), "exit leg {x}");
    }
    assert_eq!(r.clip_of(LEG), IDLE);
}

fn seat_exit_yaw_scene(rig: Rig) {
    for yaw in [-PI / 2.0, PI / 2.0] {
        let mut r = Run::new(rig.clone());
        let mut i = input(0.0);
        r.steps(&i, 20);
        seat_updates(&mut r);
        i.yaw = yaw;
        idle_exit(&mut r, &i);
        // A new on-foot turn still works in both directions.
        i.yaw += yaw;
        r.steps(&i, 1);
        let turn = if yaw < 0.0 {
            Move::TurnRight
        } else {
            Move::TurnLeft
        };
        assert_eq!(r.a.body(), Some(Body::Feet(0, turn)));
        assert_eq!(r.a.lower_clip(), Some(TURN));
    }
}

fn seat_exit_turn_scene(rig: Rig) {
    for yaw in [-PI / 2.0, PI / 2.0] {
        let mut r = Run::new(rig.clone());
        let mut i = input(0.0);
        r.steps(&i, 20);
        i.yaw = yaw;
        r.steps(&i, 1);
        assert_eq!(r.a.lower_clip(), Some(TURN));
        seat_updates(&mut r);
        // No yaw change here: only the unfinished leg turn can leak out.
        idle_exit(&mut r, &i);
    }
}

#[test]
fn invalid_seat_clip_keeps_current_foot_or_seat_state() {
    for rig in [rig_bf1(), rig_bf2()] {
        let mut r = Run::new(rig.clone());
        let mut control = Run::new(rig);
        let mut i = input(0.0);
        r.steps(&i, 20);
        control.steps(&i, 20);
        r.a.seat(&r.clips, &r.skel, usize::MAX, None, DT);
        assert!(!r.a.seated());
        i.yaw = -PI / 2.0;
        r.steps(&i, 1);
        control.steps(&i, 1);
        assert_eq!(r.a.body(), control.a.body());
        assert_eq!(r.a.lower_clip(), Some(TURN));
        assert_eq!(r.a.twist(), control.a.twist());
        assert_eq!(r.a.pose(&r.skel).0, control.a.pose(&control.skel).0);
        seat_updates(&mut r);
        let before = r.a.pose(&r.skel);
        r.a.seat(&r.clips, &r.skel, usize::MAX, None, DT);
        assert!(r.a.seated());
        assert_eq!(r.a.pose(&r.skel).0, before.0);
        // The invalid call must not advance the seat clock either.
        r.a.seat(&r.clips, &r.skel, RUN, None, DT);
        assert!((r.a.pose(&r.skel).0[LEG].translation.x - 202.0).abs() < 1e-4);
    }
}

#[test]
fn seat_exit_mounted_yaw_bf1() {
    seat_exit_yaw_scene(rig_bf1());
}
#[test]
fn seat_exit_mounted_yaw_bf2() {
    seat_exit_yaw_scene(rig_bf2());
}
#[test]
fn seat_exit_interrupted_turn_bf1() {
    seat_exit_turn_scene(rig_bf1());
}
#[test]
fn seat_exit_interrupted_turn_bf2() {
    seat_exit_turn_scene(rig_bf2());
}

/// A knocked-down soldier moving at `speed` m/s, `t` degrees from his facing.
fn knocked(stance: Stance, t: f32, speed: f32) -> AnimInput {
    AnimInput {
        stance,
        vel: speed * glam::Vec2::new(t.to_radians().sin(), t.to_radians().cos()),
        ..input(0.0)
    }
}

/// A tumble's root turns to its travel on its first update (the front clip
/// toward it, the back clip away from it), so a soldier thrown sideways
/// rolls along his travel and not across it; the get-up holds the turn.
#[test]
fn a_tumble_rolls_along_its_travel() {
    for rig in [rig_bf1(), rig_bf2()] {
        for (travel, back, twist) in [
            (89.0, false, 89.0),
            (-89.0, false, -89.0),
            (0.0, false, 0.0),
            (180.0, true, 0.0),
            (120.0, true, -60.0),
            (-135.0, true, 45.0),
        ] {
            let mut r = Run::new(rig.clone());
            r.steps(&input(0.0), 5);
            r.steps(&knocked(Stance::Tumble, travel, 6.0), 1);
            assert_eq!(r.a.body(), Some(Body::Special(Special::Tumble(back))));
            let tw = r.a.twist().to_degrees();
            assert!((tw - twist).abs() < 1e-3, "{travel}: {tw} not {twist}");
            r.steps(&knocked(Stance::GetUp, 0.0, 0.0), 10);
            assert!(matches!(r.a.body(), Some(Body::Special(Special::Land(_)))));
            assert!((r.a.twist().to_degrees() - twist).abs() < 1e-3, "held");
        }
    }
}

/// The lock after a get-up plays `stand_getupfront / back` (the get-up's
/// side) instead of the standing idle; a lock with no get-up or prone change
/// before it does not, and the clip ends with the lock.
#[test]
fn the_getup_lock_plays_the_stand_up_clip() {
    let locked = AnimInput {
        hard_lock: true,
        ..input(0.0)
    };
    for rig in [rig_bf1(), rig_bf2()] {
        for back in [false, true] {
            let mut r = Run::new(rig.clone());
            r.steps(&input(0.0), 3);
            r.steps(
                &knocked(Stance::GetUp, if back { 180.0 } else { 0.0 }, 1.0),
                3,
            );
            assert_eq!(r.a.body(), Some(Body::Special(Special::Land(back))));
            r.steps(&locked, 20);
            assert_eq!(r.a.body(), Some(Body::Special(Special::StandUp(back))));
            assert_eq!(
                Special::StandUp(back).name(),
                ["stand_getupfront", "stand_getupback"][usize::from(back)]
            );
            r.steps(&input(0.0), 1);
            assert!(matches!(r.a.body(), Some(Body::Feet(0, _))));
        }
        let mut r = Run::new(rig);
        r.steps(&locked, 3);
        assert!(matches!(r.a.body(), Some(Body::Feet(0, _))));
    }
}

/// A prone change's hard lock plays its transition (`stand_getdown_prone` /
/// `crouch_getdown_prone`, `prone_getup_stand` / `prone_getup_crouch`), not
/// the lying or standing idle, and the feet take over when the lock ends.
#[test]
fn a_prone_change_plays_its_transition_through_the_lock() {
    let at = |stance, lock| AnimInput {
        stance,
        hard_lock: lock,
        ..input(0.0)
    };
    for rig in [rig_bf1(), rig_bf2()] {
        for (from, crouch) in [(Stance::Stand, false), (Stance::Crouch, true)] {
            let mut r = Run::new(rig.clone());
            r.steps(&at(from, false), 3);
            r.steps(&at(Stance::Prone, true), 10);
            assert_eq!(r.a.body(), Some(Body::Special(Special::ProneDown(crouch))));
            r.steps(&at(Stance::Prone, false), 1);
            assert!(matches!(r.a.body(), Some(Body::Feet(2, _))));
            r.steps(&at(from, true), 10);
            assert_eq!(r.a.body(), Some(Body::Special(Special::ProneUp(crouch))));
            r.steps(&at(from, false), 1);
            assert!(matches!(r.a.body(), Some(Body::Feet(s, _)) if s == usize::from(crouch)));
        }
    }
    assert_eq!(Special::ProneDown(true).name(), "crouch_getdown_prone");
    assert_eq!(Special::ProneUp(false).name(), "prone_getup_stand");
}

mod jet;
mod legs;

#[test]
fn bf2_hero_death_uses_the_hero_clip_but_a_thrown_hero_stays_thrown() {
    let mut i = input(0.0);
    i.dead = true;
    i.hero_dead = true;
    assert_eq!(stance_of(&i, Game::Bf2), Err(Special::HeroDead));
    assert_eq!(Special::HeroDead.name(), "stand_deadhero");
    assert_eq!(stance_of(&i, Game::Bf1), Err(Special::Dead));
    i.stance = Stance::Thrown;
    assert!(matches!(stance_of(&i, Game::Bf2), Err(Special::Flying(_))));
}
