//! Card GRAI's tests: the low-arc solver, the grenade window, the group
//! test, BF2's roll, the target-kind mask and the choice's preferences, and
//! a bot's throw in the sim; card GRAIA's (the audit's mutations): BF1's
//! plenty threshold, the claim, the group radius, the lead, the clear arc,
//! MinRange at the landing, the stand to throw and the thrower's head start.

use super::*;
use crate::heightfield::Heightfield;
use crate::sim::Event;
use crate::sticky::StickyClass;
use crate::unit::{Controller, SlotSpec, UnitSpec};
use crate::weapon::{ExplosionClass, OrdnanceClass, Thrown};
use crate::TICK_DT;

#[test]
fn guided_scoring_uses_authored_explosion_damage_and_target_scales() {
    let src = crate::remote::tests::Props(&[]);
    let mut class = crate::guided::GuidedClass::from_props(&src);
    let mut scales = [1.0; 7];
    scales[2] = 0.2;
    class.explosion = Some(ExplosionClass {
        damage: 500.0,
        scales,
        ..Default::default()
    });
    let mut weapon = WeaponClass {
        guided: Some(Box::new(crate::guided::GuidedWeapon::new(&src, class))),
        ..Default::default()
    };
    assert_eq!(shot(&weapon, 0), 500.0);
    assert_eq!(shot(&weapon, 2), 100.0);
    weapon
        .guided
        .as_mut()
        .unwrap()
        .class
        .explosion
        .as_mut()
        .unwrap()
        .damage = 0.0;
    assert_eq!(shot(&weapon, 0), 0.0);
}

#[test]
fn supply_dispensers_are_never_attack_weapons() {
    let mut pack = thermal();
    pack.dispenser = Some(crate::weapon::Dispenser {
        item: crate::sim::ItemClass::basic("dual", 100.0, 2),
        velocity: 10.0,
        strength: [0.3, 0.4],
        max_items: 4,
    });
    for bf2 in [false, true] {
        let mut request = ask(16.0);
        request.bf2 = bf2;
        request.forced = true;
        request.group = true;
        assert!(choose([cand(3, &pack, Some(5))].into_iter(), &request).is_none());
    }
}

/// The thermal detonator as BF2's disc authors it (`re-grenades.md` 9).
fn thermal() -> WeaponClass {
    WeaponClass {
        name: "thermal".into(),
        rounds_per_clip: 1,
        reload_time: 0.01,
        trigger_single: true,
        shot_delay: [1.3; 2],
        initial_salvo_delay: 0.51,
        ai_range: [8.0, 16.0, 24.0],
        ai_target_kind: 0x07,
        thrown: Some(Thrown {
            shot_elevate: 0.21,
            hide_on_fire: true,
            hand: Some(Thrown::BF2_HAND),
            native_thermal_hand: false,
        }),
        ordnance: OrdnanceClass {
            velocity: 25.0,
            life_span: 0.6,
            sticky: Some(StickyClass {
                gravity: -12.25,
                stick_mask: 0x7fff_ffd8,
                explosion: Some(ExplosionClass {
                    damage: 320.0,
                    damage_inner: 4.5,
                    damage_outer: 6.5,
                    ..ExplosionClass::default()
                }),
                ..StickyClass::new(true)
            }),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    }
}

fn cand(slot: u8, class: &WeaponClass, rounds: Option<u32>) -> Cand<'_> {
    Cand {
        slot,
        class,
        rounds,
        empty: rounds == Some(0),
        busy: false,
        heavy: false,
    }
}

fn ask(d: f32) -> Ask {
    Ask {
        kind: 0,
        d2: d * d,
        bf2: true,
        group: false,
        barred: false,
        forced: false,
        coin: false,
        vehicle: None,
        crowd: false,
        droid: None,
    }
}

/// Acceptance (a): s 25, g -12.25, a still target 16 m away and level — the
/// launch elevation is 9.14 deg (sin 2a = 12.25 x 16 / 625) and a target
/// past the range reports failure. Flown as the sim steps a grenade (the
/// PS2's shell update: gravity into the velocity, then the move, 30 times a
/// second) the first crossing of the target's height is 0.83 m SHORT of it:
/// the solution is the exact parabola's, the stepped flight drops g dt / 2
/// faster. The PS2's bots have the same shortfall; it is kept.
#[test]
fn the_low_arc_solution_lands_on_a_level_target() {
    let (s, g) = (25.0f32, -12.25f32);
    let d = Vec3::new(0.0, 0.0, -16.0);
    let (raise, t, ok) = lob(s, g, d);
    assert!(ok);
    let elev = (raise / 16.0).atan().to_degrees();
    let want = 0.5 * (12.25f32 * 16.0 / 625.0).asin().to_degrees();
    assert!((elev - want).abs() < 0.02, "{elev} against {want}");
    assert!((want - 9.14).abs() < 0.01, "{want}");
    // Fly it as the sim steps a grenade.
    let dir = (d + Vec3::Y * raise).normalize();
    let (mut p, mut v, mut n) = (Vec3::ZERO, dir * s, 0);
    loop {
        v.y += g * TICK_DT;
        let q = p + v * TICK_DT;
        if q.y <= 0.0 {
            p += (q - p) * (p.y / (p.y - q.y));
            break;
        }
        p = q;
        n += 1;
        assert!(n < 300);
    }
    assert!(p.distance(d) < 1.0 && p.z > d.z, "landed {p} for {d}");
    assert!(
        (n as f32 * TICK_DT - t).abs() < 0.1,
        "{n} ticks against {t} s"
    );
    // The range on the level is s^2 / |g| = 51 m.
    assert!(lob(s, g, Vec3::new(0.0, 0.0, -50.0)).2);
    assert!(!lob(s, g, Vec3::new(0.0, 0.0, -52.0)).2);
}

/// Acceptance (b): bot number 3 is forced only for t in [9, 12) of every
/// 30 s (BF1, no group); (c) a fresh group stamp forces it outside the window.
#[test]
fn the_grenade_window_is_three_seconds_in_thirty() {
    let hz = u64::from(TICK_HZ);
    for tick in 0..90 * hz {
        let t = (tick / hz) % 30;
        let want = (9..12).contains(&t);
        assert_eq!(in_window(3, tick), want, "tick {tick}");
        assert_eq!(in_window(13, tick), want, "n = 13, tick {tick}");
        assert_eq!(grenade_forced(GroundRules::Bf1, 3, tick, false, 0.0), want);
        assert!(grenade_forced(GroundRules::Bf1, 3, tick, true, 0.0));
    }
}

/// Acceptance (c): the group test — 3 soldiers of the target's side within
/// 9 m of it (itself included) and no friend of the attacker among them.
#[test]
fn a_group_of_enemies_is_a_grenade_target_and_not_with_a_friend_in_it() {
    assert!(is_group(GroundRules::Bf1, 3, 0));
    assert!(!is_group(GroundRules::Bf1, 2, 0));
    assert!(!is_group(GroundRules::Bf1, 3, 1));
    // BF2: more than 3 beside the target.
    assert!(!is_group(GroundRules::Bf2, 4, 0));
    assert!(is_group(GroundRules::Bf2, 5, 0));
    assert!(!is_group(GroundRules::Bf2, 5, 1));
    // In the sim: three enemies 4 m apart, then a friend among them.
    let mut sim = Sim::empty(3);
    sim.ground_rules = GroundRules::Bf1;
    let at = |x: f32| Vec3::new(x, 0.0, -20.0);
    let e = sim.spawn(&UnitSpec::trooper(1, at(0.0))).unwrap();
    sim.spawn(&UnitSpec::trooper(1, at(4.0))).unwrap();
    let target = |sim: &Sim| {
        let (own, foes) = group_counts(sim, sim.unit(e).unwrap());
        is_group(sim.ground_rules, own, foes)
    };
    assert!(!target(&sim), "two are no group");
    // The radius is 9 m: a third 10 m and a fourth 12 m off are not in it.
    sim.spawn(&UnitSpec::trooper(1, at(-10.0))).unwrap();
    sim.spawn(&UnitSpec::trooper(1, at(12.0))).unwrap();
    assert!(!target(&sim), "9 m, not 10");
    sim.spawn(&UnitSpec::trooper(1, at(-8.5))).unwrap();
    assert!(target(&sim));
    sim.spawn(&UnitSpec::trooper(0, at(2.0))).unwrap();
    assert!(!target(&sim), "a friend of the thrower is in it");
}

/// Acceptance (d): BF2's roll (n + floor(10 t)) mod 100 <= floor(100 x
/// chance). At chance 1 every tick of the window forces; at chance 0 only the
/// tenths of a second on which the sum is a multiple of 100 do (the PS2's
/// test is <=, so "never" is one tenth of a second in ten).
#[test]
fn bf2_rolls_the_difficulty_inside_the_window() {
    let hz = u64::from(TICK_HZ);
    let (mut zero, mut window) = (0, 0);
    for tick in 0..30 * hz {
        let w = in_window(3, tick);
        window += u64::from(w);
        assert_eq!(grenade_forced(GroundRules::Bf2, 3, tick, false, 1.0), w);
        let f = grenade_forced(GroundRules::Bf2, 3, tick, false, 0.0);
        assert_eq!(f, w && (3 + tick * 10 / hz) % 100 == 0, "tick {tick}");
        zero += u64::from(f);
        // Half the chance: the sum's last two digits up to 50.
        let h = grenade_forced(GroundRules::Bf2, 3, tick, false, 0.5);
        assert_eq!(h, w && (3 + tick * 10 / hz) % 100 <= 50);
    }
    assert_eq!(window, 3 * hz);
    assert_eq!(zero, hz / 10, "t in [9.7, 9.8)");
    // A group opens the gate, not the roll.
    assert!(!grenade_forced(GroundRules::Bf2, 3, 15, true, 0.0));
    assert!(grenade_forced(GroundRules::Bf2, 3, 15, true, 1.0));
}

/// Acceptance (e): the target-kind mask — a thermal detonator (person,
/// animal, droid) is never chosen against a vehicle, a grenade for vehicles
/// never against a soldier, forced or not.
#[test]
fn the_mask_keeps_a_grenade_off_the_wrong_kind() {
    let rifle = WeaponClass {
        ai_target_kind: 0x1f,
        ..WeaponClass::bf2_rifle()
    };
    let th = thermal();
    let crack = WeaponClass {
        name: "crack".into(),
        ai_target_kind: 0x18,
        ..thermal()
    };
    let slots = [
        cand(0, &rifle, Some(200)),
        cand(2, &th, Some(4)),
        cand(3, &crack, Some(4)),
    ];
    for forced in [false, true] {
        for d in [4.0, 12.0, 16.0, 23.0, 40.0] {
            let soldier = Ask { forced, ..ask(d) };
            let vehicle = Ask { kind: 3, ..soldier };
            assert_ne!(choose(slots.into_iter(), &soldier).map(|p| p.0), Some(3));
            assert_ne!(choose(slots.into_iter(), &vehicle).map(|p| p.0), Some(2));
        }
    }
    // Forced and in the band, each goes at its own kind.
    let forced = Ask {
        forced: true,
        ..ask(16.0)
    };
    assert_eq!(choose(slots.into_iter(), &forced), Some((2, true)));
    let vehicle = Ask { kind: 3, ..forced };
    assert_eq!(
        choose(slots[1..].iter().copied(), &vehicle),
        Some((3, true))
    );
    // BF2: a soldier in a group is exempt from the mask.
    let group = Ask {
        group: true,
        ..forced
    };
    assert_eq!(choose(slots[2..].iter().copied(), &group), Some((3, true)));
}

/// The choice's preferences (7.1): the rifle unless the grenade is forced
/// inside its 8-24 m band and no team mate has the claim; a weapon with
/// plenty of ammunition before one without; an empty one never.
#[test]
fn the_choice_prefers_range_then_plenty_then_score() {
    let rifle = WeaponClass::bf2_rifle();
    let pistol = WeaponClass {
        name: "pistol".into(),
        rounds_per_clip: 0,
        ai_range: [0.0, 16.0, 24.0],
        ordnance: OrdnanceClass {
            max_damage: 0.8 * rifle.ordnance.max_damage,
            ..rifle.ordnance.clone()
        },
        ..WeaponClass::bf2_rifle()
    };
    let th = thermal();
    assert!(score(&rifle, 0) > score(&th, 0), "the fixture's premise");
    assert!(score(&rifle, 0) > score(&pistol, 0));
    assert!(score(&pistol, 0) > score(&th, 0));
    let slots = |rounds| {
        [
            cand(0, &rifle, Some(rounds)),
            cand(1, &pistol, None),
            cand(2, &th, Some(4)),
        ]
    };
    let pick = |rounds, a: &Ask| choose(slots(rounds).into_iter(), a);
    assert_eq!(pick(200, &ask(16.0)), Some((0, true)));
    let forced = |d| Ask {
        forced: true,
        ..ask(d)
    };
    assert_eq!(pick(200, &forced(16.0)), Some((2, true)));
    assert_eq!(pick(200, &forced(6.0)), Some((0, true)), "inside MinRange");
    assert_eq!(pick(200, &forced(30.0)), Some((0, true)), "past MaxRange");
    let barred = Ask {
        barred: true,
        ..forced(16.0)
    };
    assert_eq!(pick(200, &barred), Some((0, true)), "a team mate's claim");
    // A weapon ISSUED two rounds has not "plenty" (BF2): the endless pistol
    // takes over, in its range; an empty rifle is skipped at any range.
    assert_eq!(pick(2, &ask(16.0)), Some((1, true)));
    assert_eq!(pick(2, &ask(40.0)), Some((0, true)));
    assert_eq!(pick(0, &ask(16.0)), Some((1, true)));
    // Out of every band: the fallback, not in range.
    assert_eq!(pick(200, &ask(100.0)), Some((0, false)));
}

/// A bot with a rifle and thermal detonators, a soldier 16 m in front of
/// it that the rifle cannot kill: over a minute it throws inside its own
/// window only (twice in each), and each grenade lands within 2 m of the
/// target inside a second of its throw (0.4-1.2 m: the tracked error and the
/// stepped flight). A contact is not tied to its grenade, and
/// the one before may still be bouncing ten metres on when the next leaves
/// the hand - so the check is "one by the target", not "the first".
#[test]
fn a_bot_throws_its_grenade_in_its_window_and_lands_it_by_the_target() {
    let _stand = super::super::tests::all_stand();
    let mut sim = Sim::empty(11);
    sim.ground_rules = GroundRules::Bf1;
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let th = sim.add_weapon_class(thermal());
    let bot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            slots: std::sync::Arc::new([SlotSpec {
                slot: 2,
                channel: 1,
                weapon_class: th,
                clips: 4,
            }]),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let target = Vec3::new(0.0, 0.0, -16.0);
    let e = sim.spawn(&UnitSpec::trooper(1, target)).unwrap();
    let n = number(bot);
    let hz = u64::from(TICK_HZ);
    let (mut throws, mut landed, mut thrown_at) = (0, 0, None::<u64>);
    sim.units[e.index as usize].health.max = 1e9;
    for _ in 0..60 * hz {
        let h = &mut sim.units[e.index as usize].health;
        h.cur = h.max;
        sim.step(&[]);
        for ev in &sim.events {
            match *ev {
                Event::Throw { unit, .. } => {
                    assert_eq!(unit, bot);
                    // The press was a wind-up (0.51 s) earlier at the least.
                    let press = sim.tick - (0.51 * TICK_HZ as f32) as u64;
                    assert!(
                        in_window(n, press) || in_window(n, press - hz),
                        "a throw at tick {} outside bot {n}'s window",
                        sim.tick
                    );
                    throws += 1;
                    assert_eq!(thrown_at, None, "the throw before landed nowhere near");
                    thrown_at = Some(sim.tick);
                }
                Event::StickyContact { pos, .. }
                    if thrown_at.is_some_and(|t| sim.tick <= t + hz)
                        && crate::nav::xz_dist(pos, target) < 2.0 =>
                {
                    thrown_at = None;
                    landed += 1;
                }
                _ => {}
            }
        }
    }
    assert!((2..=4).contains(&throws), "{throws} throws in two windows");
    assert_eq!(landed, throws);
}

/// BF1's "plenty" is more than 20 rounds, BF2's more than 2 (7.1): a rifle
/// of 10 rounds gives way to a weaker weapon of 30 under BF1's rules and
/// keeps the choice under BF2's; 20 is not plenty, 21 is.
#[test]
fn bf1_plenty_is_over_twenty_rounds_and_bf2_over_two() {
    let rifle = WeaponClass::bf2_rifle();
    let weak = WeaponClass {
        name: "weak".into(),
        ordnance: OrdnanceClass {
            max_damage: 0.5 * rifle.ordnance.max_damage,
            ..rifle.ordnance.clone()
        },
        ..WeaponClass::bf2_rifle()
    };
    assert!(score(&rifle, 0) > score(&weak, 0), "the fixture's premise");
    let pick = |rounds, bf2| {
        let slots = [cand(0, &rifle, Some(rounds)), cand(1, &weak, Some(30))];
        choose(slots.into_iter(), &Ask { bf2, ..ask(16.0) }).map(|p| p.0)
    };
    assert_eq!(pick(10, false), Some(1), "BF1: 10 rounds are not plenty");
    assert_eq!(pick(10, true), Some(0), "BF2: 10 rounds are");
    assert_eq!(pick(20, false), Some(1));
    assert_eq!(pick(21, false), Some(0));
    assert_eq!(pick(2, true), Some(1));
    assert_eq!(pick(3, true), Some(0));
}

/// OURS (7.5's named deviation): a shell launcher counts as plenty on every
/// map. A BF1 pilot with a 15-round mortar in slot 0 (its disc band, 10/32/48
/// m, one shell 150 x 4/3) and an endless pistol in slot 1 (0/16/32 m, 40 x
/// 1.5) fires the mortar at 20 m and the pistol inside the mortar's 10 m.
/// The same 15 rounds of a sticky launcher are still not plenty.
#[test]
fn a_bf1_pilot_keeps_its_mortar_over_the_pistol_in_both_bands() {
    let mortar = WeaponClass {
        name: "mortar".into(),
        rounds_per_clip: 5,
        reload_time: 2.25,
        shot_delay: [1.5; 2],
        ai_range: [10.0, 32.0, 48.0],
        ai_target_kind: 0x07,
        ordnance: OrdnanceClass {
            kind: OrdKind::Shell,
            max_damage: 150.0,
            scales: [4.0 / 3.0; 7],
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    };
    let pistol = WeaponClass {
        name: "pistol".into(),
        rounds_per_clip: 0,
        reload_time: 2.0,
        shot_delay: [0.2; 2],
        ai_range: [0.0, 16.0, 32.0],
        ai_target_kind: 0x07,
        ordnance: OrdnanceClass {
            max_damage: 40.0,
            scales: [1.5; 7],
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    };
    assert!(shot(&mortar, 0) > shot(&pistol, 0), "the fixture's premise");
    let pick = |m: &WeaponClass, d| {
        let slots = [cand(0, m, Some(15)), cand(1, &pistol, None)];
        choose(
            slots.into_iter(),
            &Ask {
                bf2: false,
                ..ask(d)
            },
        )
    };
    assert_eq!(pick(&mortar, 20.0), Some((0, true)), "the mortar at 20 m");
    assert_eq!(
        pick(&mortar, 8.0),
        Some((1, true)),
        "the pistol inside 10 m"
    );
    assert_eq!(
        pick(&mortar, 40.0),
        Some((0, true)),
        "past the pistol's band"
    );
    let lobbed = WeaponClass {
        ordnance: OrdnanceClass {
            kind: OrdKind::Lobbed,
            ..mortar.ordnance.clone()
        },
        ..mortar.clone()
    };
    assert_eq!(
        pick(&lobbed, 20.0),
        Some((1, true)),
        "a sticky's 15 are not"
    );
}

/// A flat 400 m floor.
fn flat() -> Heightfield {
    Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    }
}

/// A bot of team 0 at `pos` with the rifle and four thermal detonators.
fn grenadier(sim: &mut Sim, th: usize, pos: Vec3) -> UnitHandle {
    sim.spawn(&UnitSpec {
        controller: Controller::Ai,
        slots: std::sync::Arc::new([SlotSpec {
            slot: 2,
            channel: 1,
            weapon_class: th,
            clips: 4,
        }]),
        ..UnitSpec::trooper(0, pos)
    })
    .unwrap()
}

/// The aim of a throw (7.3): the solution is for where the target will be
/// when the grenade lands — a target crossing at 5 m/s is led by 5 t m — and
/// there is none at a target that will be inside MinRange by then.
#[test]
fn the_throw_leads_a_moving_target_and_refuses_one_coming_inside_min_range() {
    let mut sim = Sim::empty(5);
    sim.heightfield = Some(flat());
    let gc = thermal();
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let u = sim.unit(h).unwrap();
    let eye = u.capsule().center();
    let target = Vec3::new(0.0, 1.0, -16.0);
    let (still, ok) = lob_aim(&sim, u, eye, target, Vec3::ZERO, &gc);
    assert!(ok && (still - eye).x.abs() < 1e-3, "{still}");
    let vel = Vec3::new(5.0, 0.0, 0.0);
    let (led, ok) = lob_aim(&sim, u, eye, target, vel, &gc);
    assert!(ok);
    let dir = led - eye;
    // The flight to 16 m takes about 0.65 s: the lead is 3.3 m to the side.
    let side = -16.0 * dir.x / dir.z;
    assert!((2.8..3.8).contains(&side), "led {side} m");
    // 10 m off and still: in the band. Walking in at 6 m/s it is at 7.5 m,
    // inside the 8 m MinRange, when the grenade comes down: no throw.
    let near = Vec3::new(0.0, 1.0, -10.0);
    assert!(lob_aim(&sim, u, eye, near, Vec3::ZERO, &gc).1);
    assert!(!lob_aim(&sim, u, eye, near, Vec3::new(0.0, 0.0, 6.0), &gc).1);
    assert!(lob_aim(&sim, u, eye, near, Vec3::new(0.0, 0.0, -6.0), &gc).1);
}

/// No throw into a wall (OURS): with a wall 12 m tall across the way the
/// solution is refused, and with one the arc clears (0.5 m) it stands.
#[test]
fn the_throw_needs_a_clear_arc() {
    use crate::collision::{Body, CollisionWorld, Shape};
    let wall = |h: f32| {
        CollisionWorld::new(
            vec![Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -8.0)),
                Shape::Box {
                    half: Vec3::new(10.0, h, 0.25),
                },
            )],
            Vec::new(),
        )
    };
    let mut sim = Sim::empty(5);
    sim.heightfield = Some(flat());
    let gc = thermal();
    let h = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let target = Vec3::new(0.0, 1.0, -16.0);
    let reaches = |sim: &Sim| {
        let u = sim.unit(h).unwrap();
        lob_aim(sim, u, u.capsule().center(), target, Vec3::ZERO, &gc).1
    };
    assert!(reaches(&sim), "open ground");
    sim.collision = wall(6.0);
    assert!(!reaches(&sim), "a wall in the arc");
    sim.collision = wall(0.5);
    assert!(reaches(&sim), "a kerb under it");
}

/// The explosive claim (4.3): two bots of one team whose grenade windows
/// are the same, one enemy 16 m in front of both — in each window only ONE
/// of them throws at it.
#[test]
fn one_bot_of_a_team_throws_at_a_soldier_at_a_time() {
    let _stand = super::super::tests::all_stand();
    let mut sim = Sim::empty(11);
    sim.ground_rules = GroundRules::Bf1;
    sim.heightfield = Some(flat());
    let th = sim.add_weapon_class(thermal());
    // Two unit slots whose numbers open the same window; the slots between
    // are filled with idle soldiers far away.
    let gen = Sim::empty(11)
        .spawn(&UnitSpec::trooper(0, Vec3::ZERO))
        .unwrap()
        .gen;
    let n = |i: u32| number(UnitHandle { index: i, gen }) % 10;
    let (a, b) = (0..12u32)
        .flat_map(|a| (a + 1..12).map(move |b| (a, b)))
        .find(|&(a, b)| n(a) == n(b))
        .expect("two of twelve share a window");
    let mut bots = Vec::new();
    for i in 0..=b {
        if i == a || i == b {
            let x = if i == a { -2.0 } else { 2.0 };
            bots.push(grenadier(&mut sim, th, Vec3::new(x, 0.0, 0.0)));
        } else {
            let far = Vec3::new(150.0, 0.0, 150.0 + 3.0 * i as f32);
            sim.spawn(&UnitSpec::trooper(0, far)).unwrap();
        }
    }
    assert_eq!((bots[0].index, bots[1].index), (a, b));
    assert_eq!(number(bots[0]) % 10, number(bots[1]) % 10);
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -16.0)))
        .unwrap();
    sim.units[e.index as usize].health.max = 1e9;
    let hz = u64::from(TICK_HZ);
    // Who threw in each 30 s round of the window.
    let mut by_round: [Vec<u32>; 3] = Default::default();
    for _ in 0..90 * hz {
        let h = &mut sim.units[e.index as usize].health;
        h.cur = h.max;
        sim.step(&[]);
        for ev in &sim.events {
            if let Event::Throw { unit, .. } = *ev {
                let press = sim.tick - (0.51 * TICK_HZ as f32) as u64;
                let r = &mut by_round[(press.saturating_sub(hz) / (30 * hz)).min(2) as usize];
                if !r.contains(&unit.index) {
                    r.push(unit.index);
                }
            }
        }
    }
    let rounds: Vec<usize> = by_round.iter().map(Vec::len).collect();
    assert!(
        rounds.iter().sum::<usize>() >= 2,
        "throws by round {rounds:?}"
    );
    assert!(
        rounds.iter().all(|&k| k <= 1),
        "throwers by round {rounds:?}"
    );
}

/// The stand to throw: a bot that circles its target stops for the throw —
/// it is slower than a walk when the grenade leaves its hand, and it does
/// move between the throws.
#[test]
fn a_circling_bot_stands_to_throw() {
    let _circle = super::super::tests::all_circle();
    let mut sim = Sim::empty(11);
    sim.ground_rules = GroundRules::Bf1;
    sim.heightfield = Some(flat());
    let th = sim.add_weapon_class(thermal());
    let bot = grenadier(&mut sim, th, Vec3::ZERO);
    let e = sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -16.0)))
        .unwrap();
    sim.units[e.index as usize].health.max = 1e9;
    let (mut throws, mut walked, mut fastest) = (0, 0.0f32, 0.0f32);
    let mut prev = sim.unit(bot).unwrap().soldier.pos;
    for _ in 0..90 * u64::from(TICK_HZ) {
        let h = &mut sim.units[e.index as usize].health;
        h.cur = h.max;
        sim.step(&[]);
        let pos = sim.unit(bot).unwrap().soldier.pos;
        let speed = crate::nav::xz_dist(pos, prev) / TICK_DT;
        walked += speed * TICK_DT;
        fastest = fastest.max(speed);
        prev = pos;
        for ev in &sim.events {
            if matches!(*ev, Event::Throw { .. }) {
                throws += 1;
                assert!(speed < 0.5, "throw {throws} at {speed} m/s");
            }
        }
    }
    assert!(throws >= 2, "{throws} throws");
    assert!(
        walked > 10.0 && fastest > 2.0,
        "walked {walked} m, {fastest} m/s at most"
    );
}

/// The thrower's head start (OURS): on Easy a bot reacts to a stranger's
/// grenade 3 m from it after 0.6 s at the earliest (its tier's delay), and
/// to its own at once.
#[test]
fn the_thrower_runs_from_its_own_grenade_at_once() {
    let moved = |own: bool| {
        let mut sim = Sim::empty(7);
        sim.difficulty = crate::skill::Difficulty::Easy;
        sim.heightfield = Some(flat());
        let bot = sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        for _ in 0..30 {
            sim.step(&[]);
        }
        let from = sim.unit(bot).unwrap().soldier.pos;
        let at = from + Vec3::new(3.0, 0.0, 0.0);
        sim.add_hazard_by(at, 600, own.then_some(bot));
        // Under Easy's earliest reaction to a stranger's, 18 ticks.
        let (lo, _) = crate::skill::Difficulty::Easy.tier().grenade_delay;
        for _ in 0..lo - 1 {
            sim.step(&[]);
        }
        let pos = sim.unit(bot).unwrap().soldier.pos;
        crate::nav::xz_dist(pos, at) - 3.0
    };
    let (own, other) = (moved(true), moved(false));
    assert!(own > 1.0, "its own grenade: {own} m further off");
    assert!(other.abs() < 0.2, "a stranger's: {other} m");
}

/// "Plenty" asks the load a weapon was ISSUED (7.5; B1 weapon+0x94, B2
/// ammo[2]): a rifle shot down to its last two rounds is still the choice,
/// and gives way only when it is empty.
#[test]
fn plenty_is_the_load_issued_not_the_rounds_left() {
    let mut sim = Sim::empty(3);
    let pistol = sim.add_weapon_class(WeaponClass {
        name: "pistol".into(),
        rounds_per_clip: 0,
        ai_range: [0.0, 16.0, 24.0],
        ordnance: OrdnanceClass {
            max_damage: 8.0,
            ..WeaponClass::bf2_rifle().ordnance
        },
        ..WeaponClass::bf2_rifle()
    });
    let h = sim
        .spawn(&UnitSpec {
            slots: std::sync::Arc::new([SlotSpec {
                slot: 1,
                channel: 0,
                weapon_class: pistol,
                clips: 0,
            }]),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let issued = sim.unit(h).unwrap().weapon.issued;
    assert!(issued > 20, "{issued} rounds issued");
    let pick = |sim: &Sim, bf2| {
        let cands = slots(sim, sim.unit(h).unwrap());
        assert_eq!(cands[1].unwrap().rounds, None, "the pistol is endless");
        (
            cands[0].unwrap().rounds,
            choose(cands.into_iter().flatten(), &Ask { bf2, ..ask(16.0) }).map(|p| p.0),
        )
    };
    for bf2 in [false, true] {
        let w = &mut sim.units[h.index as usize].weapon;
        (w.clip, w.spare) = (2, 0);
        assert_eq!(pick(&sim, bf2), (Some(issued), Some(0)), "two rounds left");
        sim.units[h.index as usize].weapon.clip = 0;
        assert_eq!(pick(&sim, bf2), (Some(issued), Some(1)), "empty");
    }
    // The choice is told of a reload under way.
    assert!(!slots(&sim, sim.unit(h).unwrap())[0].unwrap().busy);
    sim.units[h.index as usize].weapon.reload_timer = 1.0;
    assert!(slots(&sim, sim.unit(h).unwrap())[0].unwrap().busy);
}

/// BF1 scores ONE shot's damage, BF2 a rate (7.5): of a slow weapon of 160
/// a shot and a quick one of 40, both in range and endless, BF1's bot takes
/// the slow one and BF2's the quick one.
#[test]
fn bf1_scores_a_shot_and_bf2_a_rate() {
    let class = |name: &str, dmg: f32, delay: f32| WeaponClass {
        name: name.into(),
        rounds_per_clip: 0,
        shot_delay: [delay; 2],
        ordnance: OrdnanceClass {
            max_damage: dmg,
            ..WeaponClass::bf2_rifle().ordnance
        },
        ..WeaponClass::bf2_rifle()
    };
    let (slow, quick) = (class("slow", 160.0, 2.0), class("quick", 40.0, 0.2));
    assert_eq!((shot(&slow, 0), shot(&quick, 0)), (160.0, 40.0));
    assert!(score(&slow, 0) < score(&quick, 0), "the fixture's premise");
    // A thrown weapon's shot is the grenade's and its explosion's.
    assert_eq!(shot(&thermal(), 0), 320.0);
    for (bf2, want) in [(false, 0), (true, 1)] {
        for order in [[0, 1], [1, 0]] {
            let slots = [cand(0, &slow, None), cand(1, &quick, None)];
            let it = order.into_iter().map(|i| slots[i]);
            let got = choose(it, &Ask { bf2, ..ask(16.0) });
            assert_eq!(got, Some((want, true)), "bf2 {bf2}, order {order:?}");
        }
    }
}

/// BF2 does not choose a weapon that is reloading (7.5): its bot fights on
/// with the pistol; BF1's keeps the rifle.
#[test]
fn bf2_passes_over_a_reloading_weapon() {
    let rifle = WeaponClass::bf2_rifle();
    let pistol = WeaponClass {
        name: "pistol".into(),
        rounds_per_clip: 0,
        ordnance: OrdnanceClass {
            max_damage: 8.0,
            ..rifle.ordnance.clone()
        },
        ..WeaponClass::bf2_rifle()
    };
    let pick = |busy, bf2| {
        let slots = [
            Cand {
                busy,
                ..cand(0, &rifle, Some(200))
            },
            cand(1, &pistol, None),
        ];
        choose(slots.into_iter(), &Ask { bf2, ..ask(16.0) }).map(|p| p.0)
    };
    assert_eq!(pick(false, true), Some(0));
    assert_eq!(pick(true, true), Some(1), "BF2: reloading");
    assert_eq!(pick(false, false), Some(0));
    assert_eq!(pick(true, false), Some(0), "BF1 has no such test");
}

/// OURS (card GRAIA): no grenade where a friend stands by — with a soldier
/// of the bot's team 11 m from the target the bot throws nothing in a minute
/// (the grenade is not chosen and the solution itself is refused), with him
/// 14 m from it the bot throws as it does alone; and none where a friend
/// 20 m off walks to while the grenade flies.
#[test]
fn no_grenade_is_thrown_at_a_point_a_friend_stands_by() {
    let _stand = super::super::tests::all_stand();
    let throws = |friend: f32| {
        let mut sim = Sim::empty(11);
        sim.ground_rules = GroundRules::Bf1;
        sim.heightfield = Some(flat());
        let th = sim.add_weapon_class(thermal());
        let bot = grenadier(&mut sim, th, Vec3::ZERO);
        let target = Vec3::new(0.0, 0.0, -16.0);
        let e = sim.spawn(&UnitSpec::trooper(1, target)).unwrap();
        sim.spawn(&UnitSpec::trooper(0, target + Vec3::X * friend))
            .unwrap();
        let u = sim.unit(bot).unwrap();
        let aim = target + Vec3::Y;
        let gc = &sim.weapon_classes[th];
        let ok = lob_aim(&sim, u, u.capsule().center(), aim, Vec3::ZERO, gc).1;
        sim.units[e.index as usize].health.max = 1e9;
        let (mut n, mut picked) = (0, false);
        for _ in 0..60 * u64::from(TICK_HZ) {
            let h = &mut sim.units[e.index as usize].health;
            h.cur = h.max;
            sim.step(&[]);
            picked |= sim.unit(bot).unwrap().ai.arm.pick == Some(2);
            n += sim
                .events
                .iter()
                .filter(|ev| matches!(ev, Event::Throw { .. }))
                .count();
        }
        (ok, picked, n)
    };
    // Neither chosen (the bot keeps its rifle up) nor thrown.
    assert_eq!(throws(11.0), (false, false, 0), "a friend 11 m from it");
    let (ok, picked, n) = throws(14.0);
    assert!(ok && picked && n >= 2, "a friend 14 m from it: {n} throws");
    // The friend's walk: 20 m off and still, the throw stands; running at
    // the target at 6 m/s he is 10 m from it a second after it lands.
    let mut sim = Sim::empty(11);
    sim.heightfield = Some(flat());
    let gc = thermal();
    let bot = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let target = Vec3::new(0.0, 1.0, -16.0);
    let friend = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(20.0, 0.0, -16.0)))
        .unwrap();
    let reaches = |sim: &Sim| {
        let u = sim.unit(bot).unwrap();
        lob_aim(sim, u, u.capsule().center(), target, Vec3::ZERO, &gc).1
    };
    assert!(reaches(&sim));
    sim.units[friend.index as usize].soldier.vel = Vec3::new(-6.0, 0.0, 0.0);
    assert!(!reaches(&sim), "he walks into it");
    sim.units[friend.index as usize].soldier.vel = Vec3::new(6.0, 0.0, 0.0);
    assert!(reaches(&sim), "he walks away");
}

// ---- Card HVYW: heavy weapons with a purpose.

use crate::vehicle::hover::tests::skiff;
use crate::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleSpec};
use crate::weapon::HealthType;

/// BF2's rocket launcher as the disc authors it: one round and a 4 s
/// reload, `TriggerSingle`, 15 / 75 / 300 m, `AITargetPerson 0`, `Animal 1`, `Droid 0`,
/// `Vehicle 1`, `Building 1`, a missile of 500 on every kind.
pub(crate) fn launcher() -> WeaponClass {
    WeaponClass {
        name: "launcher".into(),
        rounds_per_clip: 1,
        reload_time: 4.0,
        trigger_single: true,
        shot_delay: [0.0; 2],
        initial_salvo_delay: 0.25,
        ai_range: [15.0, 75.0, 300.0],
        ai_target_kind: 0b11010,
        ordnance: OrdnanceClass {
            velocity: 100.0,
            life_span: 10.0,
            max_damage: 500.0,
            scales: [1.0; 7],
            kind: OrdKind::Missile,
            ..OrdnanceClass::default()
        },
        ..WeaponClass::bf2_rifle()
    }
}

/// BF2's wrist rocket: one round and a 2 s reload, 0 / 16 / 48 m, a person's
/// weapon by its mask (`TargetVehicle 0`), a missile of 100.
fn wrist_rocket() -> WeaponClass {
    WeaponClass {
        name: "wrist rocket".into(),
        reload_time: 2.0,
        ai_range: [0.0, 16.0, 48.0],
        ai_target_kind: 0x07,
        ordnance: OrdnanceClass {
            max_damage: 100.0,
            ..launcher().ordnance
        },
        ..launcher()
    }
}

/// A rifle: 0 / 24 / 64 m, a person's weapon, 0.1 of its bolt on a hull.
pub(crate) fn rifle() -> WeaponClass {
    WeaponClass {
        ai_range: [0.0, 24.0, 64.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    }
}

fn heavy(slot: u8, class: &WeaponClass, rounds: Option<u32>) -> Cand<'_> {
    Cand {
        heavy: true,
        ..cand(slot, class, rounds)
    }
}

const TANK: VehAsk = VehAsk {
    max_health: 4000.0,
    speed: 0.0,
    kind: VehicleKind::Hover,
};

/// The PS2's "too weak" rule (22): a weapon that does not take half the
/// hull's pool - BF2 in ten seconds of its rate, BF1 with min(its rounds, 50)
/// shots - is used on it only by its ordnance: a missile is, a rifle is not,
/// a grenade on a hull slower than 12 m/s. BF2 asks the AI mask of the
/// hull's kind first, BF1 does not; a heavy weapon's mask is not asked (OURS).
#[test]
fn a_weapon_too_weak_for_a_vehicle_is_used_on_it_only_by_its_ordnance() {
    let (l, w) = (launcher(), wrist_rocket());
    let th = WeaponClass {
        ordnance: OrdnanceClass {
            kind: OrdKind::Lobbed,
            ..thermal().ordnance
        },
        ..thermal()
    };
    // The launcher: 500 a shot, 125 a second - 1,250 in ten seconds is weak
    // against 4,000, and it is a missile.
    let got = against_vehicle(&heavy(0, &l, Some(7)), &TANK, 3, true).unwrap();
    assert!((got - 125.0).abs() < 0.1, "{got}");
    // A rifle whose mask takes a vehicle: 0.1 of its bolt, weak, no missile.
    let any = WeaponClass {
        ai_target_kind: 0x1f,
        ..rifle()
    };
    assert!(score(&any, 3) > 0.0 && score(&any, 3) * 10.0 < 2000.0);
    assert_eq!(
        against_vehicle(&cand(0, &any, Some(200)), &TANK, 3, true),
        None
    );
    // ... and it is used on a hull it does take half of in ten seconds.
    let bike = VehAsk {
        max_health: 2.0 * score(&any, 3) * 10.0 - 1.0,
        ..TANK
    };
    assert!(against_vehicle(&cand(0, &any, Some(200)), &bike, 3, true).is_some());
    let bike = VehAsk {
        max_health: 2.0 * score(&any, 3) * 10.0,
        ..TANK
    };
    assert_eq!(
        against_vehicle(&cand(0, &any, Some(200)), &bike, 3, true),
        None,
        "<= is weak"
    );
    // BF2's mask: the rifle's own refuses a vehicle whatever its damage;
    // BF1 does not ask it, and counts shots: 50 x 6 = 300 against half of 599.
    let bike = VehAsk {
        max_health: 599.0,
        ..TANK
    };
    let r = WeaponClass {
        ordnance: OrdnanceClass {
            max_damage: 60.0,
            ..rifle().ordnance
        },
        ..rifle()
    };
    assert_eq!(shot(&r, 3), 6.0);
    assert_eq!(
        against_vehicle(&cand(0, &r, Some(200)), &bike, 3, true),
        None,
        "BF2: the mask"
    );
    assert_eq!(
        against_vehicle(&cand(0, &r, Some(200)), &bike, 3, false),
        Some(6.0)
    );
    assert_eq!(
        against_vehicle(&cand(0, &r, None), &bike, 3, false),
        Some(6.0),
        "endless = 50"
    );
    // No more than 50 of them count: 200 rounds are 300 too, under half of
    // 800 (uncapped: 1,200).
    let b800 = VehAsk {
        max_health: 800.0,
        ..TANK
    };
    assert_eq!(
        against_vehicle(&cand(0, &r, Some(200)), &b800, 3, false),
        None,
        "min(rounds, 50)"
    );
    // ... of the rounds it was issued: 49 x 6 = 294 is under half of 599,
    // and 300 is no more than half of 600.
    assert_eq!(
        against_vehicle(&cand(0, &r, Some(49)), &bike, 3, false),
        None
    );
    let b600 = VehAsk {
        max_health: 600.0,
        ..TANK
    };
    assert_eq!(
        against_vehicle(&cand(0, &r, Some(50)), &b600, 3, false),
        None
    );
    // A weapon that does a vehicle nothing is never used on it.
    let none = WeaponClass {
        ordnance: OrdnanceClass {
            scales: [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            ..launcher().ordnance
        },
        ..launcher()
    };
    assert_eq!(
        against_vehicle(&heavy(0, &none, Some(7)), &TANK, 3, true),
        None
    );
    // The wrist rocket's mask refuses a vehicle: as a heavy weapon it is
    // used all the same (OURS); as any other weapon of BF2's it is not.
    assert!(against_vehicle(&heavy(1, &w, Some(4)), &TANK, 3, true).is_some());
    assert_eq!(against_vehicle(&cand(1, &w, Some(4)), &TANK, 3, true), None);
    assert!(
        against_vehicle(&cand(1, &w, Some(4)), &TANK, 3, false).is_some(),
        "BF1: no mask"
    );
    // A grenade (BF1: no mask): on a hull slower than 12 m/s only.
    assert!(shot(&th, 3) > 0.0);
    let moving = |speed| VehAsk { speed, ..TANK };
    assert!(against_vehicle(&cand(2, &th, Some(4)), &moving(11.9), 3, false).is_some());
    assert_eq!(
        against_vehicle(&cand(2, &th, Some(4)), &moving(12.0), 3, false),
        None
    );
    // The missile cares for no speed - but a flyer in the air is not
    // attacked from the ground (16 m/s and over).
    assert!(against_vehicle(&heavy(0, &l, Some(7)), &moving(40.0), 3, true).is_some());
    let flyer = |speed| VehAsk {
        speed,
        kind: VehicleKind::Flyer,
        ..TANK
    };
    assert!(against_vehicle(&heavy(0, &l, Some(7)), &flyer(15.9), 3, true).is_some());
    assert_eq!(
        against_vehicle(&heavy(0, &l, Some(7)), &flyer(16.0), 3, true),
        None
    );
    // BF1's test has no flyer: a weak weapon (3 rockets: 1,500 against half
    // of 4,000) is not used on one at any speed, as it is on a hover.
    assert_eq!(
        against_vehicle(&heavy(0, &l, Some(3)), &flyer(0.0), 3, false),
        None
    );
    assert!(against_vehicle(&heavy(0, &l, Some(3)), &TANK, 3, false).is_some());
    // The ordnance classes the exemption names.
    for (base, kind) in [
        ("missile", OrdKind::Missile),
        ("Shell", OrdKind::Shell),
        ("beam", OrdKind::Beam),
        ("haywire", OrdKind::Lobbed),
        ("beacon", OrdKind::Lobbed),
        ("sticky", OrdKind::Lobbed),
        ("grenade", OrdKind::Lobbed),
        ("bolt", OrdKind::Other),
        ("bullet", OrdKind::Other),
    ] {
        assert_eq!(OrdKind::from_base(base), kind, "{base}");
    }
}

/// The choice for a vehicle: the launcher, whatever else is held; a bot
/// with nothing that hurts it picks nothing; a launcher that reloads is
/// kept (OURS: BF2 passes a reloading weapon over for a soldier).
#[test]
fn the_choice_for_a_vehicle_is_the_weapon_that_hurts_it() {
    let (l, r) = (launcher(), rifle());
    let tank = |bf2| Ask {
        kind: 3,
        bf2,
        vehicle: Some(TANK),
        ..ask(40.0)
    };
    for bf2 in [true, false] {
        let both = [heavy(0, &l, Some(7)), cand(1, &r, Some(200))];
        assert_eq!(
            choose(both.into_iter(), &tank(bf2)),
            Some((0, true)),
            "bf2 {bf2}"
        );
        let rifle_only = [cand(1, &r, Some(200))];
        assert_eq!(
            choose(rifle_only.into_iter(), &tank(bf2)),
            None,
            "bf2 {bf2}"
        );
        let spent = [
            Cand {
                empty: true,
                ..heavy(0, &l, Some(7))
            },
            cand(1, &r, Some(200)),
        ];
        assert_eq!(choose(spent.into_iter(), &tank(bf2)), None, "bf2 {bf2}");
    }
    let reloading = [
        Cand {
            busy: true,
            ..heavy(0, &l, Some(7))
        },
        cand(1, &r, Some(200)),
    ];
    assert_eq!(choose(reloading.into_iter(), &tank(true)), Some((0, true)));
    // The same pair against a soldier: the rifle - the launcher's mask
    // refuses him, and it is a heavy weapon with no crowd.
    let both = [heavy(0, &l, Some(7)), cand(1, &r, Some(200))];
    assert_eq!(choose(both.into_iter(), &ask(30.0)), Some((1, true)));
}

/// OURS: a heavy weapon against a soldier - in a crowd of three it is the
/// answer whatever else is in range (and no later slot replaces it); with no
/// crowd it is passed over, mask or no mask; never inside 6 m.
#[test]
fn a_heavy_weapon_is_for_a_crowd() {
    assert!(is_crowd(3, 0) && is_crowd(7, 0));
    assert!(!is_crowd(2, 0), "two are no crowd");
    assert!(!is_crowd(4, 1), "a friend of the shooter among them");
    let (l, w, r) = (launcher(), wrist_rocket(), rifle());
    for bf2 in [true, false] {
        let crowd = |d: f32| Ask {
            crowd: true,
            bf2,
            ..ask(d)
        };
        let alone = |d: f32| Ask { bf2, ..ask(d) };
        // The rocketeer: launcher, then a rifle with plenty of rounds.
        let held = || [heavy(0, &l, Some(7)), cand(1, &r, Some(200))].into_iter();
        assert_eq!(choose(held(), &crowd(30.0)), Some((0, true)), "bf2 {bf2}");
        assert_eq!(choose(held(), &alone(30.0)), Some((1, true)), "bf2 {bf2}");
        // Inside the launcher's MinRange the crowd gets the rifle.
        assert_eq!(choose(held(), &crowd(14.0)), Some((1, true)), "bf2 {bf2}");
        // The super battle droid: blaster, wrist rocket, a third weapon.
        let held = || {
            [
                cand(0, &r, Some(200)),
                heavy(1, &w, Some(4)),
                cand(2, &r, Some(200)),
            ]
            .into_iter()
        };
        assert_eq!(choose(held(), &crowd(20.0)), Some((1, true)), "bf2 {bf2}");
        assert_ne!(
            choose(held(), &alone(10.0)).map(|p| p.0),
            Some(1),
            "bf2 {bf2}"
        );
        assert_ne!(
            choose(held(), &alone(20.0)).map(|p| p.0),
            Some(1),
            "bf2 {bf2}"
        );
        // MinRange 0: its own blast keeps it off a crowd 6 m away or nearer.
        assert_eq!(
            choose(held(), &crowd(6.1)).map(|p| p.0),
            Some(1),
            "bf2 {bf2}"
        );
        assert_ne!(
            choose(held(), &crowd(6.0)).map(|p| p.0),
            Some(1),
            "bf2 {bf2}"
        );
    }
    // BF2's own exemption (a group of five) no longer brings the launcher
    // out for a soldier who is in no crowd of OURS.
    let group = Ask {
        group: true,
        ..ask(30.0)
    };
    let held = [heavy(0, &l, Some(7)), cand(1, &r, Some(200))];
    assert_eq!(choose(held.into_iter(), &group), Some((1, true)));
}

/// Flat ground under BF2's rules, and a bot of team 0 at the origin facing
/// -Z with `first` in its hands and `second` in its slot 1.
pub(crate) fn heavy_scene(first: WeaponClass, second: WeaponClass) -> (Sim, UnitHandle, [u32; 2]) {
    let mut sim = Sim::empty(21);
    sim.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    sim.hover_params = vec![Some(skiff())];
    sim.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true; 1],
        exit_drop: 10.0,
        ai_size: 8,
        guns: Vec::new(),
    })];
    let (a, b) = (sim.add_weapon_class(first), sim.add_weapon_class(second));
    let bot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            weapon_class: a,
            slots: std::sync::Arc::new([SlotSpec {
                slot: 1,
                channel: 0,
                weapon_class: b,
                clips: 6,
            }]),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    (sim, bot, [a as u32, b as u32])
}

/// An enemy soldier that stands where he is and does not die.
fn dummy(sim: &mut Sim, pos: Vec3) -> UnitHandle {
    let e = sim.spawn(&UnitSpec::trooper(1, pos)).unwrap();
    sim.units[e.index as usize].health.max = 1e9;
    sim.units[e.index as usize].health.cur = 1e9;
    e
}

/// A vehicle of `kind` at `pos` with an enemy at its pilot seat.
pub(crate) fn manned(sim: &mut Sim, kind: VehicleKind, pos: Vec3) -> (VehicleHandle, UnitHandle) {
    let v = sim.spawn_vehicle(&VehicleSpec {
        kind,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 4000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let e = dummy(sim, pos);
    assert!(sim.seat_unit(e, v, 0));
    (v, e)
}

/// The weapon class of every shot `bot` fires in `secs` seconds.
fn shots_of(sim: &mut Sim, bot: UnitHandle, secs: u32) -> Vec<u32> {
    let mut got = Vec::new();
    for _ in 0..secs * TICK_HZ {
        sim.step(&[]);
        for ev in &sim.events {
            if let Event::Shot {
                unit, weapon_class, ..
            } = *ev
            {
                if unit == bot {
                    got.push(weapon_class);
                }
            }
        }
    }
    got
}

/// Acceptance: a rocket at a hover tank in range and line - the bot with a
/// launcher and a rifle fires the launcher at it, and nothing else, standing
/// where it saw it; with a wall between them it fires nothing.
#[test]
fn a_bot_with_a_launcher_fires_it_at_a_tank_in_range_and_line() {
    let (mut sim, bot, [l, _]) = heavy_scene(launcher(), rifle());
    let (_, rider) = manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -40.0));
    let got = shots_of(&mut sim, bot, 20);
    assert!(got.len() >= 2, "{} rockets in 20 s", got.len());
    assert!(got.iter().all(|c| *c == l), "{got:?}");
    let u = sim.unit(bot).unwrap();
    assert_eq!(u.ai.target, Some(rider.into()));
    assert!(u.ai.arm.vehicle && !u.ai.arm.spare);
    assert!(u.ai.hold.is_some(), "a fight position against it");
    assert!(u.soldier.pos.length() < 3.0, "it stood: {}", u.soldier.pos);
    // The line shut: a wall across the way, taller than both.
    use crate::collision::{Body, CollisionWorld, Shape};
    let (mut sim, bot, _) = heavy_scene(launcher(), rifle());
    manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -40.0));
    sim.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -30.0)),
            Shape::Box {
                half: Vec3::new(40.0, 12.0, 0.5),
            },
        )],
        Vec::new(),
    );
    assert_eq!(shots_of(&mut sim, bot, 20), Vec::<u32>::new());
}

/// Acceptance: a rocket at a 4-man group - and none at a lone soldier at
/// 10 m or at 30 m, who gets the rifle. The same for the wrist rocket, whose
/// own mask takes a person: on the PS2 it is one more weapon in its band.
#[test]
fn a_rocket_goes_at_a_group_and_the_rifle_at_a_lone_soldier() {
    let _stand = super::super::tests::all_stand();
    for first in [launcher(), wrist_rocket()] {
        let name = first.name.clone();
        // The heavy weapon is slot 1 for the droid (its first is the rifle).
        let droid = name == "wrist rocket";
        let scene = || {
            let (sim, bot, [a, b]) = if droid {
                heavy_scene(rifle(), first.clone())
            } else {
                heavy_scene(first.clone(), rifle())
            };
            let (h, r) = if droid { (b, a) } else { (a, b) };
            (sim, bot, h, r)
        };
        let (mut sim, bot, h, _) = scene();
        for p in [(0.0, -30.0), (3.0, -30.0), (-3.0, -30.0), (0.0, -33.0)] {
            dummy(&mut sim, Vec3::new(p.0, 0.0, p.1));
        }
        let got = shots_of(&mut sim, bot, 15);
        let heavy = got.iter().filter(|c| **c == h).count();
        assert!(
            heavy >= 2,
            "{name}: {heavy} of {} shots at the group",
            got.len()
        );
        for d in [10.0, 30.0] {
            let (mut sim, bot, h, r) = scene();
            dummy(&mut sim, Vec3::new(0.0, 0.0, -d));
            let got = shots_of(&mut sim, bot, 15);
            assert!(got.len() >= 5, "{name} at {d} m: {} shots", got.len());
            assert!(
                got.iter().all(|c| *c == r && *c != h),
                "{name} at {d} m: {got:?}"
            );
        }
        // The rifle empty and the launcher in the hands: it is not fired at
        // the lone soldier for want of another (30 m is inside its range) -
        // and it is at the crowd.
        if droid {
            continue;
        }
        let (mut sim, bot, h, _) = scene();
        let second = &mut sim.units[bot.index as usize].held[0].state;
        (second.clip, second.spare) = (0, 0);
        dummy(&mut sim, Vec3::new(0.0, 0.0, -30.0));
        assert_eq!(shots_of(&mut sim, bot, 15), Vec::<u32>::new());
        let u = sim.unit(bot).unwrap();
        assert!(u.ai.arm.spare && u.ai.arm.pick.is_none());
        for p in [(3.0, -30.0), (-3.0, -30.0), (0.0, -33.0)] {
            dummy(&mut sim, Vec3::new(p.0, 0.0, p.1));
        }
        let got = shots_of(&mut sim, bot, 15);
        assert!(got.len() >= 2 && got.iter().all(|c| *c == h), "{got:?}");
    }
}

/// Acceptance: a rifleman does not fire at a walker he cannot hurt - and
/// takes no position against it; the same rider on foot he shoots. With a
/// soldier behind the walker he takes the soldier (OURS), where the bot with
/// the launcher takes the walker over a nearer soldier.
#[test]
fn a_rifleman_does_not_fire_at_a_walker_and_takes_the_soldier() {
    let _stand = super::super::tests::all_stand();
    let (mut sim, bot, _) = heavy_scene(rifle(), rifle());
    let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(0.0, 1.5, -30.0));
    assert_eq!(shots_of(&mut sim, bot, 15), Vec::<u32>::new());
    let u = sim.unit(bot).unwrap();
    assert_eq!(
        u.ai.target,
        Some(rider.into()),
        "the only contact is looked at"
    );
    assert!(u.ai.arm.spare && !u.ai.arm.vehicle && u.ai.arm.pick.is_none());
    assert!(u.ai.hold.is_none(), "no fight position against it");
    // The control: he gets out, and is shot at.
    sim.unseat_unit(rider);
    assert!(shots_of(&mut sim, bot, 15).len() >= 5);
    assert!(!sim.unit(bot).unwrap().ai.arm.spare);
    // A soldier 12 m behind the walker: the rifleman's target.
    let (mut sim, bot, _) = heavy_scene(rifle(), rifle());
    manned(&mut sim, VehicleKind::Walker, Vec3::new(0.0, 1.5, -30.0));
    let foot = dummy(&mut sim, Vec3::new(8.0, 0.0, -42.0));
    assert!(shots_of(&mut sim, bot, 15).len() >= 5);
    assert_eq!(sim.unit(bot).unwrap().ai.target, Some(foot.into()));
    // The launcher's bot, a soldier at 20 m and the walker at 40: the walker.
    let (mut sim, bot, [l, _]) = heavy_scene(launcher(), rifle());
    let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(0.0, 1.5, -40.0));
    dummy(&mut sim, Vec3::new(8.0, 0.0, -20.0));
    let got = shots_of(&mut sim, bot, 15);
    assert_eq!(sim.unit(bot).unwrap().ai.target, Some(rider.into()));
    assert!(!got.is_empty() && got.iter().all(|c| *c == l), "{got:?}");
}

/// Card HVYW (OURS): no rocket where a friend WALKS into its line. A tank
/// 40 m ahead and a friend pacing 4 m either side of the line 5 m in front
/// of the bot at 6 m/s: the rocket leaves a quarter of a second after the
/// trigger, so a line that is clear at the press (`LINE_CLEAR`, 0.75 m) is
/// not when the friend is 2 m off it and coming. None of the bot's rockets
/// lands on him, the waiting is counted, and the bot still fires every
/// round it has when he is at the far end of his walk.
#[test]
fn no_rocket_leaves_with_a_friend_walking_into_its_line() {
    let _stand = super::super::tests::all_stand();
    let (mut sim, bot, [l, _]) = heavy_scene(launcher(), rifle());
    let (tank, _) = manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -40.0));
    let friend = sim
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -5.0)))
        .unwrap();
    let (mut shots, mut hits, mut dir) = (0, 0, 1.0f32);
    for _ in 0..120 * TICK_HZ {
        let u = &mut sim.units[friend.index as usize];
        u.health.cur = 1e9;
        if u.soldier.pos.x.abs() > 4.0 {
            dir = -u.soldier.pos.x.signum();
        }
        u.soldier.pos.x += dir * 6.0 / TICK_HZ as f32;
        u.soldier.vel = Vec3::new(dir * 6.0, 0.0, 0.0);
        sim.vehicles[tank.index as usize].health.cur = 4000.0;
        sim.step(&[]);
        for ev in &sim.events {
            match *ev {
                Event::Shot {
                    unit, weapon_class, ..
                } if unit == bot => {
                    assert_eq!(weapon_class, l);
                    shots += 1;
                }
                Event::Hit {
                    shooter, victim, ..
                } if shooter == bot && victim == friend => hits += 1,
                _ => {}
            }
        }
    }
    assert_eq!(hits, 0, "rockets into the friend, of {shots}");
    assert_eq!(shots, 6, "every round it has, between his passes");
    // (Card TPOS: 131 ticks once the strafe on a burst went - the step out
    // of the line takes the bot out of it sooner.)
    assert!(sim.nav_stats.line_held > 100, "{}", sim.nav_stats.line_held);
}

/// Card HVYW (OURS): a vehicle nothing the bot holds is for is left alone
/// only past [`SPARE_BEYOND`]. A rifleman with a walker 8 m off fires his
/// rifle at it - and takes no fight position: he walks on; at 30 m he does
/// not fire (`a_rifleman_does_not_fire_at_a_walker_and_takes_the_soldier`).
#[test]
fn a_rifleman_fires_at_a_vehicle_close_by() {
    let _stand = super::super::tests::all_stand();
    let (mut sim, bot, [r, _]) = heavy_scene(rifle(), rifle());
    let (_, rider) = manned(&mut sim, VehicleKind::Walker, Vec3::new(0.0, 1.5, -8.0));
    let got = shots_of(&mut sim, bot, 15);
    assert!(got.len() >= 5 && got.iter().all(|c| *c == r), "{got:?}");
    let u = sim.unit(bot).unwrap();
    assert_eq!(u.ai.target, Some(rider.into()));
    assert!(!u.ai.arm.spare && !u.ai.arm.vehicle && u.ai.arm.pick.is_none());
    assert!(u.ai.hold.is_none(), "no fight position against it");
}

/// Card HVYW: the standing fight against a vehicle is for a weapon in the
/// hands with the hull in its band. A launcher (`MinRange` 15) with a tank
/// 8 m off is fired, and the bot takes no position; a grenade at a tank in
/// its band is thrown, and the bot takes none either.
#[test]
fn no_fight_position_for_a_launcher_out_of_its_band_or_a_grenade() {
    let _stand = super::super::tests::all_stand();
    let (mut sim, bot, [l, _]) = heavy_scene(launcher(), rifle());
    manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -8.0));
    let got = shots_of(&mut sim, bot, 15);
    assert!(!got.is_empty() && got.iter().all(|c| *c == l), "{got:?}");
    let u = sim.unit(bot).unwrap();
    assert!(u.ai.arm.pick.is_some() && !u.ai.arm.in_range && !u.ai.arm.vehicle);
    assert!(u.ai.hold.is_none(), "no fight position inside MinRange");
    // The grenade: BF1's rules (no mask asked of a hull), a tank at 16 m.
    let (mut sim, bot) = thrower_scene();
    manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -16.0));
    let (mut throws, mut held) = (0, 0);
    for _ in 0..15 * TICK_HZ {
        sim.step(&[]);
        throws += sim
            .events
            .iter()
            .filter(|e| matches!(**e, Event::Throw { unit, .. } if unit == bot))
            .count();
        let u = sim.unit(bot).unwrap();
        held += u32::from(u.ai.hold.is_some() || u.ai.arm.vehicle);
    }
    assert!(throws >= 2, "{throws} throws at the tank");
    assert_eq!(held, 0, "ticks with a fight position");
}

/// BF1's rules on flat ground with [`skiff`]'s vehicle class, and a bot of
/// team 0 at the origin facing -Z with the rifle and four thermal detonators.
fn thrower_scene() -> (Sim, UnitHandle) {
    let (mut sim, _, [r, _]) = heavy_scene(rifle(), rifle());
    sim.ground_rules = GroundRules::Bf1;
    let first = sim.units.iter().position(|u| u.alive).unwrap();
    sim.units[first].soldier.pos = Vec3::new(900.0, 0.0, 900.0);
    // The roster's kind for a grenade (the class chain's engine base).
    let mut th = thermal();
    th.ordnance.kind = OrdKind::Lobbed;
    let th = sim.add_weapon_class(th);
    let bot = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            weapon_class: r as usize,
            slots: std::sync::Arc::new([SlotSpec {
                slot: 2,
                channel: 1,
                weapon_class: th,
                clips: 4,
            }]),
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    (sim, bot)
}

/// Card HVYW (OURS): a bot does not stand for a throw at a VEHICLE that
/// cannot be made from where it is. A walker 6 m beside the first leg of a
/// bot's way to a post, inside the grenade's `MinRange` 8: the grenade is
/// its one weapon for the hull and the throw is refused every tick, and the
/// bot walks on - and throws from where the throw can be made, standing
/// for it. (It stood there until it was shot: BF1 Rhen Var, seed 3, 40 s.)
#[test]
fn a_bot_does_not_stand_for_a_throw_at_a_vehicle_it_cannot_make() {
    let (mut sim, bot) = thrower_scene();
    sim.nav = Some(super::super::tests::l_graph());
    sim.add_cp(
        crate::cp::CpDef {
            name: "cp1".into(),
            pos: Vec3::new(40.0, 0.0, -40.0),
            capture: crate::region::Region::sphere(Vec3::new(40.0, 0.0, -40.0), 6.0),
            control: None,
            value_bleed: 100,
            value_atk: [100, 100],
            value_def: [100, 100],
            spawn_nodes: vec![],
            capture_time: 10.0,
            neutralize_time: 12.0,
        },
        Some(1),
    );
    manned(&mut sim, VehicleKind::Walker, Vec3::new(5.0, 1.5, -3.0));
    let (mut refused, mut throws, mut at_5s) = (0, 0, 0.0);
    let mut prev = Vec3::ZERO;
    for t in 0..20 * TICK_HZ {
        sim.step(&[]);
        let u = sim.unit(bot).unwrap();
        // The throw it can make it stands for, as for one at a soldier.
        let speed = crate::nav::xz_dist(u.soldier.pos, prev) / TICK_DT;
        prev = u.soldier.pos;
        for ev in &sim.events {
            if matches!(*ev, Event::Throw { unit, .. } if unit == bot) {
                assert!(speed < 0.5, "a throw at {speed} m/s");
            }
        }
        // The grenade is the pick and the walker is inside its `MinRange`.
        let near = (u.soldier.pos - Vec3::new(5.0, 0.0, -3.0)).length() < 7.5;
        refused += u32::from(near && matches!(armed(&sim, u, &u.ai), Armed::Thrown(_, true)));
        throws += sim
            .events
            .iter()
            .filter(|e| matches!(**e, Event::Throw { unit, .. } if unit == bot))
            .count();
        if t == 5 * TICK_HZ {
            at_5s = u.soldier.pos.length();
        }
    }
    assert!(
        refused >= 10,
        "the grenade was its pick inside MinRange: {refused} ticks"
    );
    assert!(
        at_5s > 4.0,
        "it walked on: {at_5s} m from its start after 5 s"
    );
    assert!(throws >= 1, "{throws} throws on its way");
}

/// Card HVYW: the no-stand rule is for a target that RIDES only - at a
/// soldier a bot stands from the first tick the grenade is its pick, the
/// throw not yet in reach (the aim still rising), as on main. Three enemies
/// 12 m and 20 m beside a bot that side-steps in its fight: on the tick the
/// grenade comes up in a step the stick is let go (the step's 3.8 m/s is
/// 0.6 that tick; walking on would keep 3.8 and throw on the move). (Card
/// TPOS: the strafe on a burst that made the steps went; the test makes
/// them, a 3 m step aside every 1.5 s, as a position's lean step would.)
#[test]
fn a_bot_stands_for_a_throw_at_soldiers_from_the_tick_it_is_picked() {
    // 15 and 20 m: which distances have a pick come up in a step (the case
    // this test needs) moves with the aim tracker's swing
    // (`skill::TRACKER_SHARE`); under the plain PS2 skill it is 11-12 m.
    for d in [15.0, 20.0] {
        let (mut sim, bot) = thrower_scene();
        // On its way to a post, as the bot of the vehicle scene above.
        sim.nav = Some(super::super::tests::l_graph());
        sim.add_cp(
            crate::cp::CpDef {
                name: "cp1".into(),
                pos: Vec3::new(40.0, 0.0, -40.0),
                capture: crate::region::Region::sphere(Vec3::new(40.0, 0.0, -40.0), 6.0),
                control: None,
                value_bleed: 100,
                value_atk: [100, 100],
                value_def: [100, 100],
                spawn_nodes: vec![],
                capture_time: 10.0,
                neutralize_time: 12.0,
            },
            Some(1),
        );
        for p in [(d, -3.0), (d + 3.0, -3.0), (d, -6.0)] {
            dummy(&mut sim, Vec3::new(p.0, 0.0, p.1));
        }
        let (mut live, mut fastest, mut from_a_step, mut throws) = (0, 0.0f32, 0, 0);
        let (mut prev, mut before) = (Vec3::ZERO, 0.0);
        for t in 0..12 * TICK_HZ {
            let picked = sim
                .unit(bot)
                .is_some_and(|u| matches!(armed(&sim, u, &u.ai), Armed::Thrown(..)));
            if t % 45 == 0 && !picked {
                let side = if (t / 45) % 2 == 0 { 3.0 } else { -3.0 };
                if let Some(h) = sim.unit_mut(bot).and_then(|u| u.ai.hold.as_mut()) {
                    h.goal = h.spot + Vec3::X * side;
                }
            }
            sim.step(&[]);
            let u = sim.unit(bot).unwrap();
            let speed = crate::nav::xz_dist(u.soldier.pos, prev) / TICK_DT;
            prev = u.soldier.pos;
            if matches!(armed(&sim, u, &u.ai), Armed::Thrown(_, true)) {
                live += 1;
                fastest = fastest.max(speed);
                // The case the rule is about: the pick came up in a step.
                from_a_step += u32::from(before > 3.0);
            }
            before = speed;
            throws += sim
                .events
                .iter()
                .filter(|e| matches!(**e, Event::Throw { unit, .. } if unit == bot))
                .count();
        }
        assert!(
            live >= 20 && throws >= 2,
            "{d} m: {live} ticks, {throws} throws"
        );
        assert!(
            from_a_step >= 1,
            "{d} m: the grenade never came up in a step"
        );
        assert!(
            fastest < 1.5,
            "{d} m: {fastest} m/s with the grenade picked"
        );
    }
}

/// Card SNPR: a sniper's pistol (`sidearm`) - a BF2 rifle (8 / 64 / 128 m)
/// and a pistol (0 / 16 / 24 m): no rule at 20 m (the choice's), the pistol at 20 m when
/// the target comes on and at 6 m (inside the rifle's MinRange), kept at
/// 23 m once drawn and not at 25 m (past its reach); no rule while the
/// pistol reloads in BF2, for another weapon's pick, or for a trooper with
/// no scoped rifle (the choice stands).
#[test]
fn a_snipers_pistol_is_for_a_target_close_or_coming_on() {
    let scoped = WeaponClass {
        trigger_single: true,
        zoom_max: 8.0,
        yaw_spread: 0.0,
        ai_range: [8.0, 64.0, 128.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    };
    let pistol = WeaponClass {
        ai_range: [0.0, 16.0, 24.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    };
    let th = thermal();
    let cands = |busy| {
        let mut c: [Option<Cand>; 8] = [None; 8];
        c[0] = Some(cand(0, &scoped, Some(30)));
        c[1] = Some(Cand {
            busy,
            ..cand(1, &pistol, None)
        });
        c[2] = Some(cand(2, &th, Some(4)));
        c
    };
    let arm = |d, closing, last| sidearm(&cands(false), None, last, d, closing, true).map(|p| p.0);
    assert_eq!(arm(20.0, false, None), None, "the choice's");
    assert_eq!(arm(20.0, true, None), Some(1), "within 25 m and coming on");
    assert_eq!(arm(26.0, true, None), None, "coming on, 26 m off");
    assert_eq!(
        arm(6.0, false, None),
        Some(1),
        "inside the rifle's MinRange"
    );
    assert_eq!(arm(23.0, false, Some(1)), Some(1), "kept once drawn");
    assert_eq!(arm(25.0, false, Some(1)), None, "past the pistol's reach");
    assert_eq!(arm(23.0, false, Some(0)), None);
    // A BF1 rifle's MinRange of 32 m: no pistol at 28 m, past its reach.
    let bf1 = WeaponClass {
        ai_range: [32.0, 64.0, 128.0],
        ..scoped.clone()
    };
    let mut c1 = cands(false);
    c1[0] = Some(cand(0, &bf1, Some(30)));
    assert_eq!(sidearm(&c1, None, None, 28.0, false, false), None, "28 m");
    assert_eq!(
        sidearm(&c1, None, None, 20.0, false, false).map(|p| p.0),
        Some(1)
    );
    assert_eq!(
        sidearm(&cands(true), None, None, 6.0, false, true),
        None,
        "BF2: the pistol reloads - no rule, the choice (`choose`) stands"
    );
    assert_eq!(
        sidearm(&cands(false), Some((2, true)), None, 6.0, false, true),
        None
    );
    let mut plain = cands(false);
    plain[0] = None;
    assert_eq!(sidearm(&plain, None, None, 6.0, false, true), None);
}
