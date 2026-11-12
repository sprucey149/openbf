//! Play the actual baked PS2 saber programs through the sim evaluator.

use core::combo::{ComboInput, ComboRuntime, FIRE, FIRE_SECONDARY, STAND};

#[test]
fn every_bf2_saber_hero_attacks_and_blocks_with_its_baked_program() {
    let root = game::assets::baked_root();
    if !pak::exists(root.join("side_bf2/heroes.ron")) {
        eprintln!("SKIP: BF2 hero bake absent; run bake heroes for the real-content combo check");
        return;
    }
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    let mut names: Vec<_> = db
        .names()
        .filter(|n| n.split('_').nth(1) == Some("hero"))
        .collect();
    names.sort_unstable();
    let mut checked = 0;
    for soldier in names {
        let Some(weapon) = db
            .weapon_slots(soldier)
            .into_iter()
            .find(|s| s.channel == 0)
        else {
            continue;
        };
        if !db
            .chain(&weapon.weapon)
            .last()
            .is_some_and(|c| c.base.eq_ignore_ascii_case("melee"))
        {
            continue;
        }
        let saber = game::hero_blades::saber_class(&db, soldier, &weapon.weapon)
            .unwrap_or_else(|e| panic!("{soldier}: {e:#}"));
        let input = ComboInput {
            posture: STAND,
            energy: 100.0,
            ..ComboInput::default()
        };
        let mut attack = ComboRuntime::default();
        let mut active = 0;
        let mut damage = 0.0f32;
        let mut had_animation = false;
        for tick in 0..900 {
            let out = attack.step(
                &saber.combo,
                &saber.state_seconds,
                ComboInput {
                    buttons: if tick == 0 { FIRE } else { 0 },
                    ..input
                },
                1.0 / 60.0,
            );
            had_animation |= out.animation.is_some();
            for hit in out.attacks {
                assert!(
                    (hit.edge as usize) < saber.blades.len(),
                    "{soldier}: edge {}",
                    hit.edge
                );
                let amount = hit.damage.unwrap_or(saber.damage);
                assert!(
                    amount.is_finite() && amount > 0.0,
                    "{soldier}: non-damaging authored attack"
                );
                active += 1;
                damage = damage.max(amount);
            }
        }
        assert!(
            had_animation && active > 0,
            "{soldier}: FIRE never reached an animated damage window"
        );
        let mut block = ComboRuntime::default();
        let out = block.step(
            &saber.combo,
            &saber.state_seconds,
            ComboInput {
                buttons: FIRE_SECONDARY,
                ..input
            },
            1.0 / 60.0,
        );
        let deflect = out
            .deflect
            .unwrap_or_else(|| panic!("{soldier}: block input never reached Deflect"));
        assert!(
            deflect.covers(0.0),
            "{soldier}: forward block does not cover a shot"
        );
        assert_eq!(
            out.energy_restore_rate,
            Some(-10.0),
            "{soldier}: authored block rate changed"
        );
        println!(
            "{soldier}: {active} active attack ticks, max damage {damage}, block {}..{} deg",
            deflect.min_angle, deflect.max_angle
        );
        checked += 1;
    }
    assert_eq!(checked, 15);
}

#[test]
fn saber_sprint_attack_reads_the_current_keyboard_and_controller_movement() {
    use core::heightfield::Heightfield;
    use core::sim::{buttons, PadState, Sim};
    use core::soldier::Stance;
    use game::kbm::{Kbm, Trigger};
    use game::roster::{ClassRoster, FactionRoster};
    use game::walk::{ClassVisual, TeamTemplate, WalkSim};
    use glam::Vec3;
    use winit::{event::MouseButton, keyboard::KeyCode};

    let root = game::assets::baked_root();
    if !pak::exists(root.join("side_bf2/heroes.ron")) {
        eprintln!("SKIP: BF2 hero bake absent");
        return;
    }
    let db =
        game::roster::load_classdb(&root.join("classes"), "bf2", &["all", "imp", "rep", "cis"])
            .unwrap();
    let mut names: Vec<_> = db
        .names()
        .filter(|n| n.split('_').nth(1) == Some("hero"))
        .filter(|n| {
            db.weapon_slots(n)
                .into_iter()
                .find(|s| s.channel == 0)
                .is_some_and(|s| {
                    db.chain(&s.weapon)
                        .last()
                        .is_some_and(|c| c.base.eq_ignore_ascii_case("melee"))
                })
        })
        .collect();
    names.sort_unstable();
    assert_eq!(names.len(), 15);
    let forward = Trigger::Key(KeyCode::KeyW);
    let sprint = Trigger::Key(KeyCode::ShiftLeft);
    let fire = Trigger::Mouse(MouseButton::Left);
    for name in names {
        let roster = ClassRoster::build(&db, name, 0, None).unwrap();
        for (scenario, moving, established, keep_forward, keyboard, dash) in [
            ("standing click", false, false, false, true, false),
            ("Shift and click", true, false, true, true, true),
            ("sprinting click", true, true, true, true, true),
            ("release W and click", true, true, false, true, false),
            ("latched stick and trigger", true, true, true, false, true),
            ("release stick and trigger", true, true, false, false, false),
        ] {
            let mut sim = Sim::empty(42);
            sim.ground_rules = core::soldier::GroundRules::Bf2;
            let unit = roster.register(&mut sim, &mut Vec::new());
            let faction = FactionRoster {
                faction: "hero".into(),
                classes: vec![roster.clone()],
            };
            let team =
                TeamTemplate::from_roster(&faction, &unit, vec![ClassVisual::from_roster(&roster)]);
            let ground = Heightfield {
                size: 2,
                unit: 400.0,
                offset_x: 200.0,
                offset_z: 200.0,
                heights: vec![0.0; 4],
            };
            let mut walk =
                WalkSim::with_teams(sim, Some(ground), Vec3::ZERO, [team.clone(), team], None);
            let mut kbm = Kbm::default();
            let mut latch = input::SprintLatch::default();
            let mut step = |walk: &mut WalkSim, kbm: &mut Kbm, ly, bits| {
                let pad = kbm.pad(PadState {
                    ly,
                    buttons: bits,
                    ..PadState::default()
                });
                let pad = walk.apply_sprint_latch(&mut latch, pad);
                walk.step_tick(&[pad]);
                kbm.consumed(&pad, 1);
            };
            kbm.press(forward, keyboard && moving);
            for _ in 0..12 {
                step(&mut walk, &mut kbm, if keyboard { 0 } else { i16::MAX }, 0);
            }
            if established {
                kbm.press(sprint, keyboard);
                for frame in 0..4 {
                    let buttons = if !keyboard && frame == 0 {
                        buttons::SPRINT
                    } else {
                        0
                    };
                    step(
                        &mut walk,
                        &mut kbm,
                        if keyboard { 0 } else { i16::MAX },
                        buttons,
                    );
                }
                assert_eq!(walk.soldier().stance, Stance::Sprint, "{name}: {scenario}");
            }
            if keyboard {
                kbm.press(forward, keep_forward);
                kbm.press(sprint, moving);
                kbm.press(fire, true);
                step(&mut walk, &mut kbm, 0, 0);
            } else {
                let ly = if keep_forward { i16::MAX } else { 0 };
                step(&mut walk, &mut kbm, ly, buttons::FIRE);
            }
            let u = walk.sim.unit(walk.player).unwrap();
            let saber = walk.sim.weapon_classes[u.weapon_class]
                .saber
                .as_ref()
                .unwrap();
            let state = u.weapon.melee.as_ref().unwrap().runtime.state;
            let picked = saber
                .combo
                .states()
                .nth(state)
                .and_then(|field| field.text(0));
            let expected = if dash && name == "rep_hero_kiyadimundi" {
                "DASHFLIP"
            } else if dash {
                "DASHATTACK"
            } else {
                "ATTACK1"
            };
            assert_eq!(picked, Some(expected), "{name}: {scenario}");
        }
    }
}

fn capture_score_scene() -> (game::walk::WalkSim, game::event_feed::EventFeed) {
    use core::{cp::CpDef, region::Region, sim::PadState};
    use game::event_feed::{EventFeed, Texts};
    use glam::Vec3;
    let mut walk = game::walk::WalkSim::new(
        42,
        Vec3::ZERO,
        Some(core::heightfield::Heightfield {
            size: 2,
            unit: 400.0,
            offset_x: 200.0,
            offset_z: 200.0,
            heights: vec![0.0; 4],
        }),
    );
    walk.sim.add_cp(
        CpDef {
            name: "score post".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 10.0),
            control: None,
            value_bleed: 1,
            value_atk: [1; 2],
            value_def: [1; 2],
            spawn_nodes: vec![(Vec3::ZERO, 0.0)],
            capture_time: core::TICK_DT * 0.5,
            neutralize_time: 1.0,
        },
        None,
    );
    walk.sim.cps[0].capture_team = Some(0);
    let mut feed = EventFeed::new(Texts::builtin(), ["all", "imp"], &["post".into()]);
    feed.tick(&walk.sim, walk.player);
    assert!(walk.sim.counts_in_cp(walk.player.index as usize, 0));
    walk.step_tick(&[PadState::default()]);
    assert!(
        walk.sim
            .events
            .iter()
            .any(|e| matches!(e, core::sim::Event::Captured { cp: 0, team: 0 })),
        "capture fixture: player {:?}, post {:?}, events {:?}",
        walk.sim.unit(walk.player).map(|u| u.soldier.pos),
        walk.sim.cps[0],
        walk.sim.events,
    );
    feed.tick(&walk.sim, walk.player);
    assert_eq!(feed.round().units[walk.player.index as usize].points, 5);
    // A second genuine CP capture will cross the ordinary ten-point gate.
    let post = &mut walk.sim.cps[0];
    post.owner = None;
    post.capture = 0.0;
    post.capture_team = Some(0);
    game::heroes::install(&mut walk.sim, [Some(1), None]);
    game::heroes::scores(&mut walk.sim, &feed);
    (walk, feed)
}

fn capture_offer(batch: bool, publish_each_tick: bool) -> (u64, Option<u64>) {
    use core::sim::PadState;
    let (mut walk, mut feed) = capture_score_scene();
    for _ in 0..if batch { 1 } else { 2 } {
        game::heroes::scores(&mut walk.sim, &feed);
        walk.advance_with(
            core::TICK_DT * if batch { 2.0 } else { 1.0 },
            &[PadState::default()],
            |walk| {
                if publish_each_tick {
                    game::heroes::tick(walk, &mut feed);
                } else {
                    feed.tick(&walk.sim, walk.player);
                }
            },
        );
    }
    assert_eq!(walk.sim.tick, 3);
    assert_eq!(feed.round().units[walk.player.index as usize].points, 10);
    (
        walk.sim.hash(),
        walk.sim.heroes.as_ref().unwrap().teams[0]
            .offer
            .map(|o| o.expires),
    )
}

#[test]
fn hero_unlock_tick_and_hash_do_not_depend_on_render_batch_size() {
    let batch = capture_offer(true, true);
    let individual = capture_offer(false, true);
    assert!(
        batch.1.is_some(),
        "the ten-point capture must actually unlock an offer"
    );
    assert_eq!(
        batch, individual,
        "two fixed ticks together equal two individual frames"
    );
    // The old frame-only publication is the instrument's negative control.
    let old_batch = capture_offer(true, false);
    let old_individual = capture_offer(false, false);
    assert!(old_batch.1.is_none());
    assert!(old_individual.1.is_some());
    assert_ne!(old_batch.0, old_individual.0);
}
