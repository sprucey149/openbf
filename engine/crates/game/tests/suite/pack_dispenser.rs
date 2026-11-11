use core::heightfield::Heightfield;
use core::sim::{buttons, Event, PadState, Sim};
use core::unit::Controller;
use game::roster::{load_classdb, ClassRoster};
use glam::Vec3;

#[test]
fn both_games_keep_every_pack_slot_and_deliver_health_and_ammo() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: both games' baked class data is required");
        return;
    }
    for (game, census, expected) in [
        ("bf1", include_str!("../data/weapon_slots_bf1.txt"), 7),
        ("bf2", include_str!("../data/weapon_slots_bf2.txt"), 13),
    ] {
        let db = load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"])
            .expect("baked class data");
        let mut count = 0;
        let mut weapons = std::collections::BTreeSet::new();
        for line in census.lines().filter(|l| l.contains("powerup_dispenser")) {
            let name = line.split_whitespace().next().unwrap();
            let roster = ClassRoster::build(&db, name, 1, None).unwrap();
            let held = roster
                .held
                .iter()
                .find(|h| h.weapon.dispenser.is_some())
                .unwrap_or_else(|| panic!("{game} {name}: missing dispenser"));
            let d = held.weapon.dispenser.as_ref().unwrap();
            assert_eq!(held.clips, 5);
            assert_eq!(held.channel, 1);
            assert_eq!(held.weapon.rounds_per_clip, 1);
            assert_eq!(held.weapon.reload_time, 1.0);
            assert_eq!(
                held.weapon.initial_salvo_delay,
                if game == "bf2" { 0.5 } else { 0.0 }
            );
            assert_eq!(
                held.weapon.shot_delay,
                [db.prop_f32(&held.weapon.name, "ShotDelay").unwrap(); 2]
            );
            assert_eq!(d.item.name, "com_item_powerup_dual");
            // BF1's pack heals the same third of a trooper as on its disc
            // (`game::normalize`).
            assert_eq!(
                d.item.health,
                if game == "bf2" {
                    200.0
                } else {
                    100.0 * game::normalize::BF1_SCALE
                }
            );
            assert_eq!(d.item.ammo, 2);
            assert_eq!(d.max_items, 4);
            assert_eq!(d.velocity, 10.0);
            assert_eq!(d.strength, [0.3, 0.4]);
            weapons.insert(held.weapon.name.clone());
            for collector in 0..=2 {
                let mut sim = Sim::empty(42);
                sim.ground_rules = if game == "bf2" {
                    core::soldier::GroundRules::Bf2
                } else {
                    core::soldier::GroundRules::Bf1
                };
                sim.heightfield = Some(Heightfield {
                    size: 33,
                    unit: 8.0,
                    offset_x: 128.0,
                    offset_z: 128.0,
                    heights: vec![0.0; 33 * 33],
                });
                let class = roster.register(&mut sim, &mut Vec::new());
                let player = sim
                    .spawn(&class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0))
                    .unwrap();
                let k = sim.units[player.index as usize]
                    .held
                    .iter()
                    .position(|h| sim.weapon_classes[h.class].dispenser.is_some())
                    .unwrap();
                for h in &mut sim.units[player.index as usize].held {
                    h.state.active = false;
                }
                sim.units[player.index as usize].cur1 = Some(k as u8);
                sim.units[player.index as usize].held[k].state.active = true;
                let target = if collector == 0 {
                    player
                } else {
                    sim.spawn(&core::unit::UnitSpec {
                        controller: Controller::Player { slot: 1 },
                        ..core::unit::UnitSpec::trooper(collector - 1, Vec3::X * 20.0)
                    })
                    .unwrap()
                };
                let fire = PadState {
                    buttons: buttons::SECONDARY_FIRE,
                    ..Default::default()
                };
                sim.step(&[fire, PadState::default()]);
                if game == "bf2" {
                    assert!(sim.pickups.is_empty());
                }
                let mut dropped = !sim.pickups.is_empty();
                for _ in 0..65 {
                    if dropped {
                        break;
                    }
                    sim.step(&[PadState::default(), PadState::default()]);
                    dropped |= !sim.pickups.is_empty();
                }
                assert!(dropped, "{game} {name}");
                assert_eq!(sim.pickups.len(), 1);
                assert!(sim.bolts.is_empty());
                assert!(sim.stickies.is_empty());
                assert!((sim.pickups[0].vel.z + 4.0).abs() < 0.01);
                assert_eq!(
                    sim.units[player.index as usize].held[k]
                        .state
                        .rounds(&held.weapon),
                    Some(4)
                );
                for _ in 0..90 {
                    sim.step(&[PadState::default(), PadState::default()]);
                }
                assert!(sim.pickups[0].landed);
                let pos = sim.pickups[0].pos;
                let u = &mut sim.units[target.index as usize];
                u.soldier.pos = pos;
                u.health.cur = 1.0;
                let wc = &sim.weapon_classes[u.weapon_class];
                let ammo_expected = u.weapon.issued(wc).is_some_and(|n| n > u.weapon.clip)
                    || u.held.iter().any(|h| {
                        let wc = &sim.weapon_classes[h.class];
                        wc.dispenser.is_none()
                            && h.state.issued(wc).is_some_and(|n| n > h.state.clip)
                    });
                if u.weapon.spare != core::weapon::SPARE_INFINITE {
                    u.weapon.spare = 0;
                }
                for h in &mut u.held {
                    if h.state.spare != core::weapon::SPARE_INFINITE {
                        h.state.spare = 0;
                    }
                }
                sim.step(&[PadState::default(), PadState::default()]);
                assert!(
                    sim.pickups.is_empty(),
                    "{game} {name} collector {collector}"
                );
                let u = sim.unit(target).unwrap();
                assert_eq!(u.health.cur, (1.0 + d.item.health).min(u.health.max));
                assert_eq!(
                    (u.weapon.spare > 0 && u.weapon.spare != core::weapon::SPARE_INFINITE)
                        || u.held
                            .iter()
                            .any(|h| sim.weapon_classes[h.class].dispenser.is_none()
                                && h.state.spare > 0
                                && h.state.spare != core::weapon::SPARE_INFINITE),
                    ammo_expected,
                    "{game} {name} collector {collector}: {} clip {} cap {}",
                    sim.weapon_classes[u.weapon_class].name,
                    u.weapon.clip,
                    u.spec.total_clips
                );
                assert!(u
                    .held
                    .iter()
                    .filter(|h| sim.weapon_classes[h.class].dispenser.is_some())
                    .all(|h| h.state.spare == 0));
                assert!(sim.events.iter().any(|e| matches!(
                    e,
                    Event::ItemTaken {
                        health: true,
                        ammo,
                        ..
                    } if *ammo == ammo_expected
                )));
            }
            count += 1;
        }
        assert_eq!(count, expected);
        assert_eq!(weapons.len(), 4);
    }
}
