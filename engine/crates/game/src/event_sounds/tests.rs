//! Device-free impact audio caller regressions (HITSND).
use super::*;
use core::sim::{buttons, PadState, Sim};
use core::unit::{SlotSpec, UnitSpec};
use core::weapon::{ExplosionClass, WeaponClass};

fn scene() -> (WalkSim, usize, usize) {
    let flat = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: vec![0.0; 33 * 33],
    };
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    walk.sim = Sim::empty(42);
    walk.sim.heightfield = Some(flat);
    let mut wc = WeaponClass::bf1_rifle();
    wc.fire_sound = None;
    wc.change_sound = None;
    wc.ordnance.collision_sound = Some("fired_contact".into());
    wc.ordnance.impact.as_mut().unwrap().explosion = None;
    let fired = walk.sim.add_weapon_class(wc.clone());
    wc.ordnance.collision_sound = Some("held_contact".into());
    let held = walk.sim.add_weapon_class(wc);
    walk.player = walk
        .sim
        .spawn(&UnitSpec {
            weapon_class: fired,
            slots: vec![SlotSpec {
                slot: 1,
                channel: 0,
                weapon_class: held,
                clips: 2,
            }]
            .into(),
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    walk.sim.tick = 2;
    (walk, fired, held)
}
fn sounds(walk: &WalkSim) -> Vec<(String, Vec3)> {
    let mut got = Vec::new();
    let hash = walk.sim.hash();
    let events = format!("{:?}", walk.sim.events);
    each(walk, Vec3::ZERO, |n, p| got.push((n.into(), p)));
    assert_eq!(walk.sim.hash(), hash);
    assert_eq!(format!("{:?}", walk.sim.events), events);
    got
}
fn pad(b: u32) -> PadState {
    PadState {
        buttons: b,
        ..PadState::default()
    }
}
fn target_case(switch: bool) -> (WalkSim, Vec3) {
    let (mut w, fired, held) = scene();
    let victim = w
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
        .unwrap();
    w.sim.step(&[pad(buttons::FIRE)]);
    assert_eq!(w.sim.bolts.len(), 1, "real fire must spawn a bolt");
    // The victim is met along the flight (issue #41), not at the press.
    let _ = victim;
    assert_eq!(w.sim.bolts[0].weapon_class, fired);
    if switch {
        w.sim.step(&[pad(0)]);
        w.sim.step(&[pad(buttons::SWITCH_WEAPON)]);
        assert_eq!(w.sim.units[w.player.index as usize].weapon_class, held);
    }
    for _ in 0..20 {
        w.sim.step(&[pad(0)]);
        if w.sim
            .events
            .iter()
            .any(|e| matches!(e,Event::Hit { victim: v,.. } if *v==victim))
        {
            assert!(w.sim.unit(victim).unwrap().health.cur < 300.0);
            assert!(w.sim.events.iter().any(
                |e| matches!(e, Event::Hit {victim:v,damage,..} if *v==victim && *damage>0.0)
            ));
            let at = w
                .sim
                .events
                .iter()
                .find_map(|e| match e {
                    Event::Impact {
                        pos, weapon_class, ..
                    } if *weapon_class as usize == fired => Some(*pos),
                    _ => None,
                })
                .unwrap();
            println!(
                "target switch={switch} tick={} sounds={:?} at={at:?}",
                w.sim.tick,
                sounds(&w)
            );
            return (w, at);
        }
    }
    panic!("real bolt never hit");
}
#[test]
fn switched_bolt_keeps_fired_property() {
    let (w, _) = target_case(true);
    let got = sounds(&w).into_iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(got, ["fired_contact"]);
}
#[test]
fn unchanged_property_control() {
    let (w, _) = target_case(false);
    let got = sounds(&w).into_iter().map(|x| x.0).collect::<Vec<_>>();
    assert_eq!(got, ["fired_contact"]);
}
#[test]
fn bolt_contact_position_is_impact_position() {
    let (w, at) = target_case(false);
    assert_eq!(sounds(&w), [("fired_contact".into(), at)]);
}
#[test]
fn splash_does_not_start_held_contact_per_victim() {
    let (mut w, _, _) = scene();
    for x in [-1.0, 1.0] {
        w.sim
            .spawn(&UnitSpec::trooper(1, Vec3::new(x, 0.0, -20.0)))
            .unwrap();
    }
    let e = ExplosionClass {
        damage: 1.0,
        damage_inner: 3.0,
        damage_outer: 5.0,
        ..ExplosionClass::default()
    };
    w.sim
        .explode(&e, Vec3::new(0.0, 1.0, -20.0), Some(w.player), Some(0));
    let n = w
        .sim
        .events
        .iter()
        .filter(|e| matches!(e, Event::Hit { .. }))
        .count();
    assert_eq!(n, 2, "real blast must damage both victims");
    assert!(!w
        .sim
        .events
        .iter()
        .any(|e| matches!(e, Event::Impact { .. })));
    println!("blast hits={n} sounds={:?}", sounds(&w));
    assert!(
        sounds(&w).is_empty(),
        "damage recipients aren't ordnance contacts"
    );
}

#[test]
fn other_players_terrain_hit_uses_firing_class() {
    let (mut w, fired, held) = scene();
    w.sim.step(&[pad(buttons::SWITCH_WEAPON)]);
    assert_eq!(w.sim.units[w.player.index as usize].weapon_class, held);
    let other = w
        .sim
        .spawn(&UnitSpec {
            weapon_class: fired,
            ..UnitSpec::player_in_slot(1, 1, Vec3::new(30.0, 0.0, 0.0))
        })
        .unwrap();
    let down = PadState {
        ry: -32767,
        ..PadState::default()
    };
    for _ in 0..40 {
        w.sim.step(&[pad(0), down]);
    }
    w.sim.step(&[pad(0), pad(buttons::FIRE)]);
    assert!(w
        .sim
        .events
        .iter()
        .any(|e| matches!(e,Event::Shot{unit,..} if *unit==other)));
    let at = w
        .sim
        .events
        .iter()
        .find_map(|e| match e {
            Event::Impact {
                pos,
                weapon_class,
                surface: core::impact::Surface::Terrain,
                ..
            } if *weapon_class as usize == fired => Some(*pos),
            _ => None,
        })
        .expect("other player's real shot must strike terrain");
    println!("other terrain sounds={:?} at={at:?}", sounds(&w));
    assert_eq!(sounds(&w), [("fired_contact".into(), at)]);
}

#[test]
fn seat_gun_contact_uses_seat_weapon() {
    use core::vehicle::gun::{Aimer, GunSetup, GunWeapon};
    use core::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    let (mut w, _, gun_class) = scene();
    w.sim.hover_params = vec![None];
    w.sim.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 1,
        guns: vec![Some(GunSetup {
            weapons: vec![GunWeapon {
                class: gun_class,
                infinite: true,
                reserve_clips: 0,
                fire_points: Vec::new(),
                fire_limits: Vec::new(),
                fire_pivots: Vec::new(),
            }],
            fire_point: Vec3::new(0.0, 1.0, 2.0),
            pivot: Vec3::Y,
            eye: Vec3::Y,
            aimer: Aimer::default(),
            pilot: false,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::IDENTITY,
            weapon_aimer: None,
            follows_owner: false,
            gunner: None,
        })],
    })];
    let vh = w.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    });
    assert!(w.sim.enter_vehicle(w.player, vh, 0));
    let victim = w
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 20.0)))
        .unwrap();
    w.sim.step(&[pad(buttons::FIRE)]);
    assert_eq!(w.sim.bolts.len(), 1, "real seat must fire");
    assert_eq!(w.sim.bolts[0].weapon_class, gun_class);
    // The victim is met along the flight (issue #41), not at the press.
    let _ = victim;
    for _ in 0..20 {
        w.sim.step(&[pad(0)]);
        if w.sim
            .events
            .iter()
            .any(|e| matches!(e,Event::Hit{victim:v,..} if *v==victim))
        {
            assert!(w.sim.unit(victim).unwrap().health.cur < 300.0);
            let at = w
                .sim
                .events
                .iter()
                .find_map(|e| match e {
                    Event::Impact {
                        pos, weapon_class, ..
                    } if *weapon_class as usize == gun_class => Some(*pos),
                    _ => None,
                })
                .expect("seat bolt completed its contact");
            assert_eq!(sounds(&w), [("held_contact".into(), at)]);
            return;
        }
    }
    panic!("seat hit missing");
}

#[test]
fn rocket_contact_is_one_sound_beside_multiple_damaged_victims() {
    let (mut w, fired, _) = scene();
    let explosion = ExplosionClass {
        damage: 1.0,
        damage_inner: 3.0,
        damage_outer: 5.0,
        look: core::impact::ExplosionLook {
            sound: Some("rocket_blast".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let wc = &mut w.sim.weapon_classes[fired];
    wc.ordnance.kind = core::weapon::OrdKind::Missile;
    wc.ordnance.explosion = Some(explosion.clone());
    wc.ordnance.impact.as_mut().unwrap().explosion = Some(explosion.look);
    let mut victims = Vec::new();
    for x in [0.0, -2.0, 2.0] {
        victims.push(
            w.sim
                .spawn(&UnitSpec::trooper(1, Vec3::new(x, 0.0, -20.0)))
                .unwrap(),
        );
    }
    w.sim.step(&[pad(buttons::FIRE)]);
    assert_eq!(w.sim.bolts.len(), 1);
    assert_eq!(w.sim.bolts[0].weapon_class, fired);
    for _ in 0..25 {
        w.sim.step(&[pad(0)]);
        if let Some(at) = w.sim.events.iter().find_map(|e| match e {
            Event::Impact { pos, .. } => Some(*pos),
            _ => None,
        }) {
            for victim in victims {
                assert!(w.sim.events.iter().any(
                    |e| matches!(e, Event::Hit {victim:v,damage,..} if *v==victim && *damage>0.0)
                ));
                assert!(w.sim.unit(victim).unwrap().health.cur < 300.0);
            }
            assert_eq!(
                sounds(&w),
                [("rocket_blast".into(), at), ("fired_contact".into(), at)]
            );
            return;
        }
    }
    panic!("rocket never contacted");
}

#[test]
fn world_and_vehicle_contacts_use_exact_contact_position() {
    use core::collision::{Body, HullPart, Layers, Shape};
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    for vehicle in [false, true] {
        let (mut w, fired, _) = scene();
        let shape = Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        };
        let at = Vec3::new(0.0, 5.0, -10.0);
        if vehicle {
            w.sim.hover_params = vec![None];
            w.sim.hull_parts = vec![Some(
                vec![HullPart::new(glam::Mat4::IDENTITY, shape, Layers::ALL)].into(),
            )];
            w.sim.spawn_vehicle(&VehicleSpec {
                kind: VehicleKind::Turret,
                class: 0,
                home_team: Some(1),
                neutral_when_empty: false,
                spawn_pad: None,
                pos: at,
                yaw: 0.0,
                max_health: 1000.0,
                health_type: core::weapon::HealthType::Building,
                seats: vec![SeatKind::Gunner(0)],
            });
            w.sim.sync_hulls();
        } else {
            w.sim
                .collision
                .push_body(Body::new(glam::Mat4::from_translation(at), shape));
        }
        w.sim.step(&[pad(buttons::FIRE)]);
        assert_eq!(w.sim.bolts.len(), 1);
        assert_eq!(w.sim.bolts[0].weapon_class, fired);
        for _ in 0..20 {
            if let Some(at) = w.sim.events.iter().find_map(|e| match e {
                Event::Impact { pos, surface, .. } => {
                    assert_eq!(
                        *surface,
                        if vehicle {
                            core::impact::Surface::Vehicle
                        } else {
                            core::impact::Surface::Building
                        }
                    );
                    Some(*pos)
                }
                _ => None,
            }) {
                assert!((at.z + 9.5).abs() < 0.001);
                assert_eq!(sounds(&w), [("fired_contact".into(), at)]);
                break;
            }
            w.sim.step(&[pad(0)]);
        }
        assert!(
            w.sim
                .events
                .iter()
                .any(|e| matches!(e, Event::Impact { .. })),
            "completed contact required"
        );
    }
}

#[test]
fn coincident_contacts_and_missing_property_are_not_deduplicated_or_guessed() {
    let (mut w, fired, held) = scene();
    let mut quiet = w.sim.weapon_classes[fired].clone();
    quiet.ordnance.collision_sound = None;
    let quiet = w.sim.add_weapon_class(quiet);
    let at = Vec3::new(3.0, 4.0, 5.0);
    w.sim.events = [fired, held, fired, quiet]
        .into_iter()
        .map(|weapon_class| Event::Impact {
            pos: at,
            dir: Vec3::NEG_Z,
            weapon_class: weapon_class as u32,
            surface: core::impact::Surface::Building,
            vehicle: None,
        })
        .collect();
    assert_eq!(
        sounds(&w),
        [
            ("fired_contact".into(), at),
            ("held_contact".into(), at),
            ("fired_contact".into(), at)
        ]
    );
}

#[test]
fn airborne_expiry_has_no_contact_sound() {
    // A bolt fired into the empty sky expires with no contact. (The stale
    // target - one recycled while the bolt was in flight - is gone: since
    // issue #41 the bolt is swept against whoever stands in its way.)
    let (mut w, _, _) = scene();
    w.sim.heightfield = None;
    w.sim.units[w.player.index as usize].soldier.pos.y = 100.0;
    w.sim.units[w.player.index as usize].soldier.pitch = 0.5;
    w.sim.step(&[pad(buttons::FIRE)]);
    assert_eq!(w.sim.bolts.len(), 1);
    assert_eq!(w.sim.bolts[0].hit, core::projectile::Hit::None);
    for _ in 0..90 {
        w.sim.step(&[pad(0)]);
        assert!(!w
            .sim
            .events
            .iter()
            .any(|e| matches!(e, Event::Impact { .. })));
        assert!(sounds(&w).is_empty());
    }
    assert!(w.sim.bolts.is_empty());
}

#[test]
fn thrown_grenade_keeps_bounce_and_blast_sounds_without_damage_contacts() {
    use core::sticky::StickyClass;
    use core::weapon::{OrdnanceClass, Thrown};
    let (mut w, fired, _) = scene();
    let grenade = w.sim.add_weapon_class(WeaponClass {
        rounds_per_clip: 1,
        reload_time: 0.01,
        trigger_single: true,
        shot_delay: [1.3; 2],
        initial_salvo_delay: 0.51,
        fire_sound: Some("throw".into()),
        thrown: Some(Thrown {
            shot_elevate: 0.21,
            hide_on_fire: true,
            hand: Some(Thrown::BF2_HAND),
            native_thermal_hand: false,
        }),
        ordnance: OrdnanceClass {
            velocity: 25.0,
            life_span: 0.6,
            collision_sound: Some("wrong_bolt_contact".into()),
            sticky: Some(StickyClass {
                gravity: -12.25,
                stick_mask: 0x7fff_ffd8,
                bounce_sound: Some("bounce".into()),
                explosion: Some(ExplosionClass {
                    damage: 1.0,
                    damage_inner: 4.5,
                    damage_outer: 6.5,
                    look: core::impact::ExplosionLook {
                        sound: Some("grenade_blast".into()),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..StickyClass::new(true)
            }),
            ..Default::default()
        },
        ..Default::default()
    });
    // Replace the scene's player so channel 1 runs its real throw cycle.
    w.sim = {
        let mut sim = Sim::empty(42);
        sim.heightfield = w.sim.heightfield.take();
        sim.weapon_classes = w.sim.weapon_classes.clone();
        sim
    };
    w.player = w
        .sim
        .spawn(&UnitSpec {
            weapon_class: fired,
            slots: vec![SlotSpec {
                slot: 2,
                channel: 1,
                weapon_class: grenade,
                clips: 4,
            }]
            .into(),
            ..UnitSpec::player(0, Vec3::ZERO)
        })
        .unwrap();
    w.sim.tick = 2;
    // Steep aim keeps the entire real throw on the fixture's flat ground.
    w.sim.units[w.player.index as usize].soldier.pitch = -0.7;
    let (mut throws, mut contacts, mut blasts) = (0, 0, 0);
    let mut victims = Vec::new();
    let mut damaged = 0;
    for tick in 0..240 {
        if victims.is_empty() {
            if let Some(s) = w.sim.stickies.iter().find(|s| s.armed && !s.moving) {
                let at = s.pos;
                for x in [-1.0, 1.0] {
                    victims.push(
                        w.sim
                            .spawn(&UnitSpec::trooper(1, at + Vec3::X * x))
                            .unwrap(),
                    );
                }
            }
        }
        w.sim.step(&[pad(if tick < 20 {
            buttons::SECONDARY_FIRE
        } else {
            0
        })]);
        let mut expected = Vec::new();
        for e in &w.sim.events {
            match e {
                Event::Throw {
                    weapon_class,
                    origin,
                    ..
                } => {
                    assert_eq!(*weapon_class as usize, grenade);
                    throws += 1;
                    expected.push(("throw".into(), *origin));
                }
                Event::StickyContact {
                    weapon_class,
                    pos,
                    water: false,
                } => {
                    assert_eq!(*weapon_class as usize, grenade);
                    contacts += 1;
                    expected.push(("bounce".into(), *pos));
                }
                Event::StickyBlast { weapon_class, pos } => {
                    assert_eq!(*weapon_class as usize, grenade);
                    blasts += 1;
                    expected.push(("grenade_blast".into(), *pos));
                }
                _ => {}
            }
        }
        damaged += w.sim.events.iter().filter(|e| matches!(e, Event::Hit {victim,damage,..} if victims.contains(victim) && *damage>0.0)).count();
        assert_eq!(sounds(&w), expected, "tick {tick}");
    }
    assert_eq!(throws, 1);
    assert!(contacts > 0);
    assert_eq!(blasts, 1);
    assert_eq!(damaged, 2, "blast damaged both recipients");
}

// Existing water policy is silent; multiple non-water contacts retain cadence.
#[test]
fn sticky_water_stays_silent_and_every_other_contact_sounds() {
    let (mut w, fired, _) = scene();
    w.sim.weapon_classes[fired].ordnance.sticky = Some(core::sticky::StickyClass {
        bounce_sound: Some("bounce".into()),
        ..core::sticky::StickyClass::new(true)
    });
    let at = Vec3::new(1.0, 2.0, 3.0);
    w.sim.events = [false, true, false]
        .into_iter()
        .map(|water| Event::StickyContact {
            weapon_class: fired as u32,
            pos: at,
            water,
        })
        .collect();
    assert_eq!(sounds(&w), [("bounce".into(), at), ("bounce".into(), at)]);
}

#[test]
fn destroyed_stations_stop_chatter_and_play_their_death_explosion_sound_once() {
    let (mut w, _, _) = scene();
    let mut class = core::sim::StationClass::soldier("medical", 1.0, 15.0, 0);
    class.ambient_sound = Some("station_idle".into());
    let mut explosion = ExplosionClass::default();
    explosion.look.sound = Some("station_boom".into());
    class.explosion = Some(explosion);
    let c = w.sim.add_station_class(class);
    w.sim
        .stations
        .push(core::sim::Station::new(c, Vec3::ZERO, 5.0));
    assert_eq!(station_asks(&w).len(), 1);
    let damage = core::weapon::OrdnanceClass {
        max_damage: 1000.0,
        ..Default::default()
    }
    .damage_desc(0.0, false);
    w.sim
        .damage_station(0, &damage, 1.0, Some(w.player), Some(0));
    w.sim
        .damage_station(0, &damage, 1.0, Some(w.player), Some(0));
    assert!(station_asks(&w).is_empty());
    assert_eq!(sounds(&w), [("station_boom".into(), Vec3::ZERO)]);
}

#[test]
fn air_expiry_uses_the_fired_class_expire_sound_once_at_the_final_position() {
    let (mut walk, fired, held) = scene();
    let mut e = ExplosionClass::default();
    e.look.sound = Some("air_burst".into());
    walk.sim.weapon_classes[fired].ordnance.expire = Some(e);
    walk.sim.weapon_classes[held].ordnance.expire = None;
    let pos = Vec3::new(10.0, 20.0, 30.0);
    walk.sim.events = vec![Event::OrdnanceExpired {
        pos,
        weapon_class: fired as u32,
    }];
    assert_eq!(sounds(&walk), vec![("air_burst".into(), pos)]);
    walk.sim.events = vec![Event::OrdnanceExpired {
        pos,
        weapon_class: held as u32,
    }];
    assert!(sounds(&walk).is_empty());
}
