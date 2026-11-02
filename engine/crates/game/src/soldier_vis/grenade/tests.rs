use crate::animator::ActionKind;
use crate::soldier_vis::SoldierVisuals;
use crate::walk::{TeamTemplate, WalkSim};
use core::heightfield::Heightfield;
use core::sim::{buttons, Event, PadState};
use core::soldier::GroundRules;
use core::unit::{SlotSpec, UnitSpec};
use glam::Vec3;
use std::sync::Arc;

const THROWN: &str = "rep_weap_inf_thermaldetonator";

#[test]
fn pack_dispensers_use_the_original_drop_action_for_every_faction() {
    let root = crate::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        return;
    }
    for (game, classes) in [
        (
            "bf1",
            [
                "all_inf_pilot",
                "imp_inf_pilottie",
                "rep_inf_clone_pilot",
                "cis_inf_pilotdroid",
            ],
        ),
        (
            "bf2",
            [
                "all_inf_engineer",
                "imp_inf_engineer",
                "rep_inf_ep3_engineer",
                "cis_inf_engineer",
            ],
        ),
    ] {
        let db =
            crate::roster::load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"])
                .unwrap();
        for class_name in classes {
            let roster = crate::roster::ClassRoster::build(&db, class_name, 0, None).unwrap();
            let pack = &roster
                .held
                .iter()
                .find(|h| h.weapon.dispenser.is_some())
                .unwrap()
                .weapon;
            for moving in [false, true] {
                let mut w = WalkSim::new(
                    8,
                    Vec3::ZERO,
                    Some(Heightfield {
                        size: 2,
                        unit: 4000.0,
                        offset_x: 2000.0,
                        offset_z: 2000.0,
                        heights: vec![0.0; 4],
                    }),
                );
                w.sim.ground_rules = if game == "bf1" {
                    GroundRules::Bf1
                } else {
                    GroundRules::Bf2
                };
                let primary = w.sim.add_weapon_class(roster.weapon.clone());
                let dispenser = w.sim.add_weapon_class(pack.clone());
                w.player = w
                    .sim
                    .spawn(&UnitSpec {
                        weapon_class: primary,
                        slots: Arc::new([SlotSpec {
                            slot: 3,
                            channel: 1,
                            weapon_class: dispenser,
                            clips: 5,
                        }]),
                        ..UnitSpec::player_in_slot(1, 0, Vec3::new(10.0, 0.0, 0.0))
                    })
                    .unwrap();
                let p = w.player.index as usize;
                let mut t = TeamTemplate::default_trooper();
                t.classes[0].geometry = roster.geometry.clone();
                t.classes[0].weapon_geometry = roster.weapon_geometry.clone();
                t.classes[0].anim = roster.anim.clone();
                let mut vis = SoldierVisuals::load_cpu(&root, game, &[t.clone(), t]);
                vis.update(&w, core::TICK_DT);
                let mut releases = 0;
                for tick in 0..70 {
                    let pad = PadState {
                        buttons: if tick == 0 {
                            buttons::SECONDARY_FIRE
                        } else {
                            0
                        },
                        ly: if moving { 16384 } else { 0 },
                        ..Default::default()
                    };
                    w.step_tick(&[PadState::default(), pad]);
                    vis.note_shots(&w);
                    vis.update(&w, core::TICK_DT * 0.5);
                    vis.update(&w, core::TICK_DT * 0.5);
                    let a = &vis.units[p].animator;
                    if tick == 0 {
                        assert_eq!(
                            a.action(),
                            Some((
                                if game == "bf1" {
                                    ActionKind::Shoot
                                } else {
                                    ActionKind::Throw(0)
                                },
                                !moving
                            )),
                            "{game} {class_name} moving {moving}"
                        );
                        let clips = &vis.assets[vis.per_class[0][0].body.unwrap()].asset.clips;
                        let name = &clips[a.upper_clip().unwrap()].name;
                        let suffix = if game == "bf1" {
                            "grenade_stand_shoot"
                        } else {
                            "rifle_stand_shoot_secondary_full"
                        };
                        assert!(name.ends_with(suffix), "{game} {class_name}: {name}");
                        assert_eq!(
                            vis.hand_of(&w.sim.units[p], &w.sim.weapon_classes).class,
                            if game == "bf1" { dispenser } else { primary }
                        );
                    }
                    assert!(
                        !matches!(a.action(), Some((ActionKind::Throw(1 | 2), _))),
                        "{game} {class_name}: grenade throw selected at {tick}"
                    );
                    if game == "bf2" {
                        for event in &w.sim.events {
                            if let Event::Throw { unit, origin, .. } = event {
                                if unit.index as usize != p {
                                    continue;
                                }
                                let asset = &vis.assets[vis.per_class[0][0].body.unwrap()].asset;
                                let bone = asset.skeleton.index_of("bone_l_hand").unwrap();
                                let pose = a.pose(&asset.skeleton);
                                let hand = (super::super::model_of(&w.sim.units[p].soldier)
                                    * asset.skeleton.palette(&pose.locals())[bone]
                                    * asset.skeleton.global_bind[bone])
                                    .transform_point3(Vec3::ZERO);
                                let item = &pack.dispenser.as_ref().unwrap().item;
                                let local =
                                    glam::Quat::from_rotation_y(-w.sim.units[p].soldier.yaw)
                                        * (hand - *origin)
                                        - item.centre;
                                let gap = (local.abs() - item.half).max(Vec3::ZERO).length();
                                assert!(gap < 0.08, "{class_name} moving {moving}: pack {origin:?}, hand {hand:?}, gap {gap}");
                                assert!(
                                    (tick as f32 * core::TICK_DT - pack.initial_salvo_delay).abs()
                                        <= core::TICK_DT
                                );
                            }
                        }
                    }
                    releases += w.sim.events.iter().filter(|e| matches!(e, Event::Throw { unit, weapon_class, .. } if unit.index as usize == p && *weapon_class as usize == dispenser)).count();
                }
                assert_eq!(releases, 1, "{game} {class_name}");
                assert_eq!(vis.units[p].animator.action(), None);
                assert_eq!(
                    vis.hand_of(&w.sim.units[p], &w.sim.weapon_classes).class,
                    primary
                );
            }
        }
    }
}

/// One soldier of `game` throws a thermal detonator (standing or moving);
/// `borrow` fields a BF1 soldier on a BF2 map: BF2's throw clips and timing
/// (card GTHRW, the class picker's case). Checks the clip, the hand, and that
/// the grenade leaves on the clip's release moment within one tick.
fn throw_run(root: &std::path::Path, game: &str, borrow: bool, moving: bool) {
    let db = crate::roster::load_classdb(&root.join("classes"), game, &["rep"]).unwrap();
    let class_name = if game == "bf1" {
        "rep_inf_clone_trooper"
    } else {
        "rep_inf_ep3_rifleman"
    };
    let roster = crate::roster::ClassRoster::build(&db, class_name, 0, None).unwrap();
    let mut grenade = crate::roster::thrown_weapon_from_db(&db, THROWN)
        .unwrap()
        .unwrap();
    if borrow {
        core::weapon::Thrown::throw_as_bf2(&mut grenade);
    }
    let slot = crate::classdb::WeaponSlot {
        slot: 2,
        weapon: THROWN.into(),
        ammo: Some(4),
        channel: 1,
    };
    let thrown_bank = crate::roster::class_anim(&db, class_name, &[slot]).banks;
    assert_eq!(grenade.offhand_weapon, game == "bf2");
    // Each game's own wind-up (re-grenades §9).
    assert_eq!(
        grenade.initial_salvo_delay,
        if game == "bf1" && !borrow {
            0.3
        } else {
            core::weapon::Thrown::BF2_WIND_UP
        }
    );
    let geometry = grenade.ordnance.geometry.as_deref().unwrap();
    assert_eq!(geometry, "com_weap_inf_grenadethermal_ord");
    let projectile =
        crate::assets::load_glb(&root.join(format!("side_{game}/weapons/{geometry}.glb"))).unwrap();
    assert_eq!(projectile.indices.len() / 3, 170);
    let mut w = WalkSim::new(
        8,
        Vec3::ZERO,
        Some(Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }),
    );
    let bf2_map = game == "bf2" || borrow;
    w.sim.ground_rules = if bf2_map {
        GroundRules::Bf2
    } else {
        GroundRules::Bf1
    };
    let primary = w.sim.add_weapon_class(roster.weapon.clone());
    let thrown = w.sim.add_weapon_class(grenade.clone());
    w.player = w
        .sim
        .spawn(&UnitSpec {
            weapon_class: primary,
            slots: Arc::new([SlotSpec {
                slot: 2,
                channel: 1,
                weapon_class: thrown,
                clips: 4,
            }]),
            ..UnitSpec::player_in_slot(1, 0, Vec3::new(10.0, 0.0, 0.0))
        })
        .unwrap();
    let p = w.player.index as usize;
    let mut t = TeamTemplate::default_trooper();
    t.classes[0].geometry = roster.geometry.clone();
    t.classes[0].weapon_geometry = roster.weapon_geometry.clone();
    t.classes[0].anim = roster.anim.clone();
    t.classes[0].anim.banks.extend(thrown_bank.iter().cloned());
    t.classes[0].held_models = vec![(2, Some("com_weap_inf_grenadethermal".into()))];
    let mut vis = SoldierVisuals::load_cpu(root, game, &[t.clone(), t]);
    let body = vis.per_class[0][0].body.unwrap();
    if borrow {
        lend_bf2_throw(root, &mut vis.assets[body]);
        vis.bf2 = true;
    }
    let rig = vis.assets[body].rig.as_ref().unwrap();
    assert_eq!(rig.throws_offhand(), borrow, "{game}");
    let bf2_clip = bf2_map;
    // The moment the clip's hand lets go: each game's own wind-up.
    let release_at = if bf2_clip {
        grenade.initial_salvo_delay
    } else {
        0.3
    };
    vis.update(&w, core::TICK_DT);
    let mut releases = 0;
    let mut throw_frames = 0;
    for tick in 0..100 {
        let pad = PadState {
            buttons: if tick < 2 { buttons::SECONDARY_FIRE } else { 0 },
            ly: if moving { 16384 } else { 0 },
            ..Default::default()
        };
        w.step_tick(&[PadState::default(), pad]);
        vis.note_shots(&w);
        vis.update(&w, core::TICK_DT * 0.5);
        vis.update(&w, core::TICK_DT * 0.5);
        let a = &vis.units[p].animator;
        let clips = &vis.assets[body].asset.clips;
        let what = format!("{game} borrow {borrow} moving {moving}");
        if tick == 0 {
            assert_eq!(
                a.action(),
                Some((ActionKind::Throw(if bf2_map { 1 } else { 2 }), !moving)),
                "{what}"
            );
            let hand = vis.hand_of(&w.sim.units[p], &w.sim.weapon_classes);
            assert_eq!(
                (hand.class, hand.hidden),
                if bf2_clip {
                    (primary, false)
                } else {
                    (thrown, true)
                },
                "{what}"
            );
        }
        if matches!(a.action(), Some((ActionKind::Throw(_), _))) {
            throw_frames += 1;
        }
        if w.sim
            .events
            .iter()
            .any(|e| matches!(e, Event::Throw { unit, .. } if unit.index as usize == p))
        {
            releases += 1;
            let up = a.upper_clip().unwrap();
            let name = &clips[up].name;
            let suffix = if bf2_clip {
                "rifle_stand_shoot_secondary2_full"
            } else {
                "grenade_stand_shoot3"
            };
            assert!(name.ends_with(suffix), "{what}: {name}");
            // The sim's release against the clip's: within one tick. The
            // clip's time by its own game's clock (BF1 frames / 30, BF2
            // (frames - 1) / 30: re-grenade-throw-anim §10.3's 9/51 and 16/40).
            let frames = clips[up].frames as f32 - if bf2_clip { 1.0 } else { 0.0 };
            let seconds = a.upper_phase() * frames / 30.0;
            assert!(
                (seconds - release_at).abs() <= core::TICK_DT,
                "{what}: released at clip time {seconds}, the hand lets go at {release_at}"
            );
            if bf2_clip {
                // BF2's throw lets go from its left hand: the grenade starts
                // there (`Thrown::BF2_HAND`, BF2's RAM).
                let origin = w
                    .sim
                    .events
                    .iter()
                    .find_map(|e| match e {
                        Event::Throw { unit, origin, .. } if unit.index as usize == p => {
                            Some(*origin)
                        }
                        _ => None,
                    })
                    .unwrap();
                let asset = &vis.assets[body].asset;
                let bone = asset.skeleton.index_of("bone_l_hand").unwrap();
                let pose = a.pose(&asset.skeleton);
                let hand = (super::super::model_of(&w.sim.units[p].soldier)
                    * asset.skeleton.palette(&pose.locals())[bone]
                    * asset.skeleton.global_bind[bone])
                    .transform_point3(Vec3::ZERO);
                let gap = hand.distance(origin);
                assert!(gap < 0.3, "{what}: hand {hand:?}, grenade {origin:?}");
            }
            if !moving {
                assert_eq!(a.upper_clip(), a.lower_clip(), "{what}");
            } else {
                assert_ne!(a.upper_clip(), a.lower_clip(), "{what}");
            }
        }
    }
    assert_eq!(releases, 1, "{game}");
    assert!(throw_frames > 30, "{game}: {throw_frames}");
    assert_eq!(vis.units[p].animator.action(), None);
    assert_eq!(
        vis.hand_of(&w.sim.units[p], &w.sim.weapon_classes).class,
        primary
    );
}

/// A BF1 soldier fielded on a BF2 map: BF2's two throw clips, baked into him
/// by `BORROWED_THROW` (lent here for a model baked without them).
fn lend_bf2_throw(root: &std::path::Path, sa: &mut crate::soldier_vis::SoldierAsset) {
    use crate::animator::BF2_THROW_CLIPS;
    let lender =
        crate::assets::load_skinned_glb(&root.join("side_bf2/soldiers/rep_inf_ep3trooper.glb"))
            .unwrap();
    super::lend_clips(&lender, &BF2_THROW_CLIPS, &mut sa.asset);
    sa.rig.as_mut().unwrap().play_bf2_throw(&sa.asset.clips);
}

#[test]
fn real_grenades_start_the_authored_throw_before_release_and_restore_the_gun() {
    let root = crate::assets::baked_root();
    if !root.join("classes/bf1_rep.ron").exists() {
        return;
    }
    for game in ["bf1", "bf2"] {
        for moving in [false, true] {
            throw_run(&root, game, false, moving);
        }
    }
}

/// A BF1 class on a BF2 map (the class picker; card GTHRW): he plays BF2's
/// overhand throw with the rifle in his hand, and the grenade leaves on its
/// release frame, as he did before BF1 maps went back to BF1's throw.
#[test]
fn bf1_soldier_on_a_bf2_map_throws_bf2s_throw_released_on_its_frame() {
    let root = crate::assets::baked_root();
    if !root.join("classes/bf1_rep.ron").exists()
        || !root
            .join("side_bf2/soldiers/rep_inf_ep3trooper.glb")
            .exists()
    {
        return;
    }
    for moving in [false, true] {
        throw_run(&root, "bf1", true, moving);
    }
}

/// One soldier of `class` throws slot `weapon` on a BF1 map (`bf2_class`: from
/// BF2's disc, as the class picker fields him): what his body plays and holds
/// at the release, and when the grenade leaves.
struct Bf1Throw {
    clip: String,
    prone_clip: String,
    in_hand: usize,
    hidden: bool,
    thrown_class: usize,
    primary: usize,
    released_at: f32,
}

fn throw_on_bf1_map(
    root: &std::path::Path,
    class: &str,
    grenade: core::weapon::WeaponClass,
    bf2_class: bool,
) -> Bf1Throw {
    let game = if bf2_class { "bf2" } else { "bf1" };
    let db = crate::roster::load_classdb(&root.join("classes"), game, &["rep"]).unwrap();
    let roster = crate::roster::ClassRoster::build(&db, class, 0, None).unwrap();
    let mut w = WalkSim::new(
        8,
        Vec3::ZERO,
        Some(Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }),
    );
    w.sim.ground_rules = GroundRules::Bf1;
    let primary = w.sim.add_weapon_class(roster.weapon.clone());
    let thrown = w.sim.add_weapon_class(grenade);
    w.player = w
        .sim
        .spawn(&UnitSpec {
            weapon_class: primary,
            slots: Arc::new([SlotSpec {
                slot: 2,
                channel: 1,
                weapon_class: thrown,
                clips: 4,
            }]),
            ..UnitSpec::player_in_slot(1, 0, Vec3::new(10.0, 0.0, 0.0))
        })
        .unwrap();
    let p = w.player.index as usize;
    let mut t = TeamTemplate::default_trooper();
    t.classes[0].geometry = roster.geometry.clone();
    t.classes[0].weapon_geometry = roster.weapon_geometry.clone();
    t.classes[0].anim = roster.anim.clone();
    let mut vis = SoldierVisuals::load_cpu(root, game, &[t.clone(), t]);
    // The map is BF1's whatever the class's disc ([`append_test_class`]).
    vis.fit_throw_to_map(root, false);
    vis.bf2 = false;
    let body = vis.per_class[0][0].body.unwrap();
    let set = manifest::anim_sets::bf1::weapon_set("grenade").unwrap();
    let prone = vis.assets[body].rig.as_ref().unwrap();
    let prone = prone.action_slots(set, 2, ActionKind::Throw(2)).unwrap().0;
    let prone_clip = vis.assets[body].asset.clips[prone.clip].name.clone();
    vis.update(&w, core::TICK_DT);
    let mut seen = None;
    let mut released_at = None;
    for tick in 0..100 {
        let pad = PadState {
            buttons: if tick < 2 { buttons::SECONDARY_FIRE } else { 0 },
            ..Default::default()
        };
        w.step_tick(&[PadState::default(), pad]);
        vis.note_shots(&w);
        vis.update(&w, core::TICK_DT * 0.5);
        vis.update(&w, core::TICK_DT * 0.5);
        let a = &vis.units[p].animator;
        if tick == 0 {
            let hand = vis.hand_of(&w.sim.units[p], &w.sim.weapon_classes);
            let clip = a.upper_clip().unwrap();
            seen = Some((
                vis.assets[body].asset.clips[clip].name.clone(),
                hand.class,
                hand.hidden,
            ));
            assert_eq!(a.action(), Some((ActionKind::Throw(2), true)), "{class}");
        }
        if released_at.is_none()
            && w.sim
                .events
                .iter()
                .any(|e| matches!(e, Event::Throw { unit, .. } if unit.index as usize == p))
        {
            let frames = vis.assets[body].asset.clips[a.upper_clip().unwrap()].frames as f32;
            released_at = Some(a.upper_phase() * frames / 30.0);
        }
    }
    let (clip, in_hand, hidden) = seen.unwrap();
    Bf1Throw {
        clip,
        prone_clip,
        in_hand,
        hidden,
        thrown_class: thrown,
        primary,
        released_at: released_at.expect("released"),
    }
}

fn assert_bf1s_throw(what: &str, t: &Bf1Throw) {
    assert_eq!(t.clip, "human_grenade_stand_shoot3", "{what}");
    assert_eq!(t.prone_clip, "human_grenade_prone_shoot", "{what}");
    assert_eq!(
        (t.in_hand, t.hidden),
        (t.thrown_class, true),
        "{what}: empty hand"
    );
    assert_ne!(t.in_hand, t.primary, "{what}: the rifle is put away");
    assert!(
        (t.released_at - 0.3).abs() <= core::TICK_DT,
        "{what}: released at clip time {}",
        t.released_at
    );
}

/// The maintainer's report (2026-11-01): the BF1 jet trooper looked as if he
/// threw BF2's grenade and the clone trooper BF1's. On a BF1 map both throw
/// BF1's: the same ordnance model, BF1's trails and BF1's throw at BF1's time,
/// with no BF2 clip, whatever the grenade.
#[test]
fn bf1_map_jet_and_clone_troopers_throw_the_same_bf1_grenade_and_animation() {
    let root = crate::assets::baked_root();
    if !root.join("classes/bf1_rep.ron").exists() {
        return;
    }
    let db = crate::roster::load_classdb(&root.join("classes"), "bf1", &["rep"]).unwrap();
    let grenades = |class: &str| -> Vec<(u32, core::weapon::WeaponClass)> {
        crate::roster::ClassRoster::build(&db, class, 0, None)
            .unwrap()
            .held
            .into_iter()
            .filter(|h| h.channel == 1 && h.weapon.thrown.is_some())
            .map(|h| (h.clips, h.weapon))
            .collect()
    };
    let jet = grenades("rep_inf_jet_trooper");
    let clone = grenades("rep_inf_clone_trooper");
    // The disc's counts (`WeaponAmmo`, re-grenades §7): the jet trooper's 6
    // is his own, not a misread.
    assert_eq!(
        jet.iter()
            .map(|(n, w)| (*n, w.name.as_str()))
            .collect::<Vec<_>>(),
        [(6, "rep_weap_inf_thermaldetonator")]
    );
    assert_eq!(
        clone
            .iter()
            .map(|(n, w)| (*n, w.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (3, "rep_weap_inf_haywiredetonator"),
            (2, "rep_weap_inf_crackgrenade")
        ]
    );
    for (class, (_, weapon)) in jet
        .iter()
        .map(|g| ("rep_inf_jet_trooper", g))
        .chain(clone.iter().map(|g| ("rep_inf_clone_trooper", g)))
    {
        let what = format!("{class} {}", weapon.name);
        assert_eq!(
            weapon.ordnance.geometry.as_deref(),
            Some("com_weap_inf_grenadethermal_ord"),
            "{what}"
        );
        assert_eq!(weapon.initial_salvo_delay, 0.3, "{what}");
        assert!(!weapon.offhand_weapon, "{what}");
        assert_eq!(weapon.thrown.unwrap().hand, None, "{what}");
        let sticky = weapon.ordnance.sticky.as_ref().unwrap();
        assert!(!sticky.bf2, "{what}: BF1's rest rule");
        let throw = throw_on_bf1_map(&root, class, weapon.clone(), false);
        assert_bf1s_throw(&what, &throw);
    }
    // Their colours stay their own, each BF1's (re-grenades): the thermal red,
    // the haywire blue, the crack gold.
    let trail =
        |w: &core::weapon::WeaponClass| w.ordnance.impact.as_ref().and_then(|i| i.trail.clone());
    assert_eq!(
        trail(&jet[0].1).as_deref(),
        Some("com_sfx_weap_grenade_red")
    );
    assert_eq!(
        trail(&clone[0].1).as_deref(),
        Some("com_sfx_weap_grenade_blue")
    );
    assert_eq!(
        trail(&clone[1].1).as_deref(),
        Some("com_sfx_weap_grenade_gold")
    );
}

/// A BF2 class picked on a BF1 map plays BF1's throw too: the clip lent to
/// his skeleton, on BF1's clock, with BF1's grenade from BF1's side.
#[test]
fn a_bf2_soldier_on_a_bf1_map_plays_bf1s_throw() {
    let root = crate::assets::baked_root();
    if !root.join("classes/bf1_rep.ron").exists()
        || !root
            .join("side_bf2/soldiers/rep_inf_ep3trooper.glb")
            .exists()
    {
        return;
    }
    let bf1 = crate::roster::load_classdb(&root.join("classes"), "bf1", &["rep"]).unwrap();
    let grenade = crate::roster::thrown_weapon_from_db(&bf1, THROWN)
        .unwrap()
        .unwrap();
    for class in ["rep_inf_ep3_rifleman", "rep_inf_ep3_jettrooper"] {
        let throw = throw_on_bf1_map(&root, class, grenade.clone(), true);
        assert_bf1s_throw(class, &throw);
    }
}

/// The pause menu's own path, as the game takes it ([`SoldierVisuals::append_test_class`],
/// not the tests' shortcut above): a BF2 class picked on a BF1 map plays BF1's
/// clip, draws BF1's grenade and has BF1's effects. Needs a GPU.
#[test]
fn the_pause_menu_fields_a_bf2_class_on_a_bf1_map_with_bf1s_throw() {
    let root = crate::assets::baked_root();
    let map = root.join("bf1_geo1");
    if !map.join("manifest.ron").is_file() {
        return;
    }
    let Ok(mut r) = render::Renderer::new_headless(64, 64) else {
        eprintln!("SKIP: no GPU");
        return;
    };
    let (scene, _) = crate::assets::load_battle_scene(&map, "con").unwrap();
    let mut w = WalkSim::new(
        8,
        Vec3::ZERO,
        Some(Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        }),
    );
    w.sim.ground_rules = GroundRules::Bf1;
    let db = crate::roster::load_classdb(&root.join("classes"), "bf1", &["rep"]).unwrap();
    let base = crate::roster::ClassRoster::build(&db, "rep_inf_clone_trooper", 0, None).unwrap();
    let uc = base.register(&mut w.sim, &mut Vec::new());
    w.sim.classes = [vec![uc.clone()], vec![uc]];
    let visual = crate::walk::ClassVisual::from_roster(&base);
    w.teams[0].classes = vec![visual.clone()];
    w.teams[1].classes = vec![visual];
    let mut picker = crate::class_picker::ClassPicker::new(&root.join("classes"), &scene);
    picker.load();
    let mut vis = SoldierVisuals::for_walk(&mut r, &root, &scene, &w);
    assert!(!vis.bf2, "the map's game");
    let choice = picker
        .choices
        .iter()
        .position(|c| c.game == "bf2" && c.class.class == "rep_inf_ep3_rifleman")
        .unwrap();
    assert!(picker.select(&mut w, choice));
    for request in &picker.pending_assets {
        vis.append_test_class(&mut r, &root, &w, request);
    }
    let p = w.player.index as usize;
    let u = &w.sim.units[p];
    let body = vis.asset_for(u.team, u.spec.class).unwrap();
    let names: Vec<_> = body.asset.clips.iter().map(|c| c.name.as_str()).collect();
    for clip in crate::animator::BF1_THROW_CLIPS {
        assert!(names.contains(&clip), "{clip}");
    }
    assert!(!vis.bf2_throw(u), "BF1's grenade is no offhand throw");
    vis.update(&w, core::TICK_DT);
    let mut thrown = None;
    let mut clip = None;
    let mut released_at = None;
    for tick in 0..40 {
        let pad = PadState {
            buttons: if tick < 2 { buttons::SECONDARY_FIRE } else { 0 },
            ..Default::default()
        };
        w.step_tick(&[pad]);
        vis.note_shots(&w);
        vis.update(&w, core::TICK_DT);
        let u = &w.sim.units[p];
        let body = vis.asset_for(u.team, u.spec.class).unwrap();
        let a = &vis.units[p].animator;
        if tick == 0 {
            clip = a.upper_clip().map(|c| body.asset.clips[c].name.clone());
        }
        for e in &w.sim.events {
            if let Event::Throw {
                unit, weapon_class, ..
            } = e
            {
                if unit.index as usize == p {
                    thrown = Some(*weapon_class as usize);
                    let frames = body.asset.clips[a.upper_clip().unwrap()].frames as f32;
                    released_at = Some(a.upper_phase() * frames / 30.0);
                }
            }
        }
    }
    assert_eq!(clip.as_deref(), Some("human_grenade_stand_shoot3"));
    let thrown = thrown.expect("thrown");
    let released_at = released_at.unwrap();
    assert!((released_at - 0.3).abs() <= core::TICK_DT, "{released_at}");
    let class = &w.sim.weapon_classes[thrown];
    assert!(!class.ordnance.sticky.as_ref().unwrap().bf2);
    assert!(vis.grenades[thrown].is_some(), "BF1's grenade model");
    assert!(
        !vis.weapons[vis.grenades[thrown].unwrap()].parts.is_empty(),
        "BF1's grenade model has a mesh"
    );
    let bf1 = pak::read_to_string(root.join("common_bf1/effects.ron")).unwrap();
    let blast = &class.ordnance.sticky.as_ref().unwrap();
    let blast = &blast.explosion.as_ref().unwrap().look.effects;
    assert!(!blast.is_empty());
    let trail = class.ordnance.impact.as_ref().unwrap().trail.as_ref();
    assert!(trail.is_some());
    for effect in blast.iter().chain(trail) {
        assert!(
            bf1.contains(effect.as_str()),
            "{effect} is in BF1's common effects"
        );
    }
}
