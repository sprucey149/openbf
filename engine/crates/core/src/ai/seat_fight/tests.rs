//! Bots in ground vehicle seats fight enemy vehicles (`super`): the target
//! preference and its 15 m infantry exception, the seat weapon by target,
//! the hold against a hull no seat weapon may be used on, the seat guns'
//! sight reach, a hover driver turning its hull to an enemy vehicle while it
//! drives, and a walker's pilot taking an enemy hover over a nearer soldier.

use super::*;
use crate::ai::arsenal::tests::launcher;
use crate::cp::CpDef;
use crate::heightfield::Heightfield;
use crate::perception::Foe;
use crate::region::Region;
use crate::sim::Event;
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::gun::{Aimer, GunWeapon};
use crate::vehicle::hover::tests::skiff;
use crate::vehicle::walker::tests::atst;
use crate::vehicle::{SeatKind, SeatSetup, VehicleHandle, VehicleSpec};
use crate::weapon::{HealthType, OrdKind, OrdnanceClass, WeaponClass};

/// A tank cannon: a bolt of 400 on every kind, 0 / 100 / 150 m, used on a
/// person, an animal, a vehicle and a building.
fn cannon() -> WeaponClass {
    WeaponClass {
        name: "cannon".into(),
        ai_range: [0.0, 100.0, 150.0],
        ai_target_kind: 0b11011,
        ordnance: OrdnanceClass {
            velocity: 300.0,
            life_span: 1.0,
            max_damage: 400.0,
            scales: [1.0; 7],
            kind: OrdKind::Other,
            ..OrdnanceClass::default()
        },
        ..WeaponClass::bf2_rifle()
    }
}

/// A speeder bike's blaster: a bolt of 30, for a person, an animal or a
/// droid only.
fn blaster() -> WeaponClass {
    WeaponClass {
        name: "blaster".into(),
        ai_range: [0.0, 60.0, 100.0],
        ai_target_kind: 0x07,
        ordnance: OrdnanceClass {
            velocity: 300.0,
            life_span: 1.0,
            max_damage: 30.0,
            scales: [1.0; 7],
            kind: OrdKind::Other,
            ..OrdnanceClass::default()
        },
        ..WeaponClass::bf2_rifle()
    }
}

/// A soldier's rifle (personal kit: about 109 m of scan radius).
fn kit_rifle() -> WeaponClass {
    WeaponClass {
        ai_range: [0.0, 24.0, 64.0],
        ai_target_kind: 0x07,
        ..WeaponClass::bf2_rifle()
    }
}

/// A seat's gun with `weapons` (classes, endless rounds).
fn gun(weapons: &[usize], pilot: bool) -> GunSetup {
    GunSetup {
        weapons: weapons
            .iter()
            .map(|&class| GunWeapon {
                class,
                infinite: true,
                reserve_clips: 0,
                fire_points: Vec::new(),
                fire_limits: Vec::new(),
                fire_pivots: Vec::new(),
            })
            .collect(),
        fire_point: Vec3::new(0.0, 1.5, 3.0),
        pivot: Vec3::new(0.0, 1.5, 2.0),
        eye: Vec3::new(0.0, 0.5, 0.0),
        aimer: Aimer::default(),
        pilot,
        turn: None,
        framed: false,
        aimer_bind: glam::Quat::IDENTITY,
        weapon_aimer: None,
        follows_owner: false,
        gunner: None,
    }
}

fn seats(radius: f32, guns: Vec<Option<GunSetup>>) -> SeatSetup {
    SeatSetup {
        radius,
        body_active: vec![],
        boardable: vec![true; 2],
        exit_drop: 10.0,
        ai_size: 8,
        guns,
    }
}

fn flat() -> Sim {
    let mut s = Sim::empty(5);
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    s
}

fn post(s: &mut Sim, at: Vec3) {
    s.add_cp(
        CpDef {
            name: "far".into(),
            pos: at,
            capture: Region::sphere(at, 8.0),
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
}

fn hull(s: &mut Sim, kind: VehicleKind, class: u32, pos: Vec3, yaw: f32) -> VehicleHandle {
    s.spawn_vehicle(&VehicleSpec {
        kind,
        class,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw,
        max_health: 4000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    })
}

/// An enemy soldier that stands where he is and does not die.
fn dummy(s: &mut Sim, team: u8, pos: Vec3) -> UnitHandle {
    let e = s.spawn(&UnitSpec::trooper(team, pos)).unwrap();
    s.units[e.index as usize].health.max = 1e9;
    s.units[e.index as usize].health.cur = 1e9;
    e
}

/// An enemy hover of class 0 at `pos` with a dummy at its pilot seat.
fn enemy_hover(s: &mut Sim, pos: Vec3) -> (VehicleHandle, UnitHandle) {
    let v = hull(s, VehicleKind::Hover, 0, pos, 0.0);
    let e = dummy(s, 1, pos);
    assert!(s.seat_unit(e, v, 0));
    (v, e)
}

/// Flat ground and a team-0 hover (class 0, the skiff, sphere `radius`) at
/// the origin facing +Z whose GUNNER seat holds a bot with a rifle kit and
/// a gun of `weapons`; a dummy of its side at the pilot seat keeps it still.
fn gunner_scene(weapons: Vec<WeaponClass>, radius: f32) -> (Sim, UnitHandle, Vec<usize>) {
    let mut s = flat();
    s.hover_params = vec![Some(skiff())];
    let rifle = s.add_weapon_class(kit_rifle());
    let classes: Vec<usize> = weapons.into_iter().map(|w| s.add_weapon_class(w)).collect();
    s.seat_setup = vec![Some(seats(radius, vec![None, Some(gun(&classes, false))]))];
    let v = hull(&mut s, VehicleKind::Hover, 0, Vec3::new(0.0, 1.5, 0.0), 0.0);
    let pilot = dummy(&mut s, 0, Vec3::ZERO);
    assert!(s.seat_unit(pilot, v, 0));
    let bot = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            weapon_class: rifle,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -3.0))
        })
        .unwrap();
    assert!(s.seat_unit(bot, v, 1));
    (s, bot, classes)
}

/// The weapon classes of the shots `bot` fires in `secs` seconds.
fn shots_of(s: &mut Sim, bot: UnitHandle, secs: u32) -> Vec<u32> {
    let mut got = Vec::new();
    for _ in 0..secs * TICK_HZ {
        s.step(&[]);
        for ev in &s.events {
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

fn settle(s: &mut Sim, secs: u32) {
    for _ in 0..secs * TICK_HZ {
        s.step(&[]);
    }
}

fn target(s: &Sim, bot: UnitHandle) -> Option<Foe> {
    s.unit(bot).unwrap().ai.target
}

/// A tank's gun that can hurt an enemy tank in range takes it over a nearer
/// soldier (25 m) - unless enemy infantry is within 15 m, or the seat's only
/// weapon cannot hurt the tank.
#[test]
fn a_tank_takes_an_enemy_tank_over_a_nearer_soldier() {
    let (mut s, bot, _) = gunner_scene(vec![cannon()], 5.0);
    let (_, rider) = enemy_hover(&mut s, Vec3::new(5.0, 1.5, 70.0));
    dummy(&mut s, 1, Vec3::new(-3.0, 0.0, 25.0));
    settle(&mut s, 2);
    assert_eq!(target(&s, bot), Some(rider.into()));
    // Infantry within 15 m of the hull: the soldier first.
    let (mut s, bot, _) = gunner_scene(vec![cannon()], 5.0);
    enemy_hover(&mut s, Vec3::new(5.0, 1.5, 70.0));
    let man = dummy(&mut s, 1, Vec3::new(-3.0, 0.0, 10.0));
    settle(&mut s, 2);
    assert_eq!(target(&s, bot), Some(man.into()));
    // A seat whose only weapon cannot hurt the tank takes the soldier.
    let (mut s, bot, _) = gunner_scene(vec![blaster()], 5.0);
    enemy_hover(&mut s, Vec3::new(5.0, 1.5, 70.0));
    let man = dummy(&mut s, 1, Vec3::new(-3.0, 0.0, 25.0));
    settle(&mut s, 2);
    assert_eq!(target(&s, bot), Some(man.into()));
}

/// A speeder bike's blaster (a person's weapon by its AI mask) holds its
/// fire at a lone walker it sees; a cannon fires at it.
#[test]
fn a_bike_blaster_does_not_fire_at_a_walker() {
    let (mut s, bot, _) = gunner_scene(vec![blaster()], 5.0);
    let v = hull(
        &mut s,
        VehicleKind::Walker,
        0,
        Vec3::new(0.0, 1.5, 40.0),
        0.0,
    );
    let e = dummy(&mut s, 1, Vec3::new(0.0, 0.0, 40.0));
    assert!(s.seat_unit(e, v, 0));
    let got = shots_of(&mut s, bot, 6);
    assert_eq!(target(&s, bot), Some(e.into()));
    assert!(s.unit(bot).unwrap().ai.seat.hold);
    assert_eq!(got, Vec::<u32>::new());
    // Control: the cannon fires at it.
    let Some((mut s, bot, [c])) = gunner_scene(vec![cannon()], 5.0).map3() else {
        unreachable!()
    };
    let v = hull(
        &mut s,
        VehicleKind::Walker,
        0,
        Vec3::new(0.0, 1.5, 40.0),
        0.0,
    );
    let e = dummy(&mut s, 1, Vec3::new(0.0, 0.0, 40.0));
    assert!(s.seat_unit(e, v, 0));
    let got = shots_of(&mut s, bot, 6);
    assert!(
        !got.is_empty() && got.iter().all(|w| *w == c as u32),
        "{got:?}"
    );
}

/// A seat with a blaster and a missile launcher (the fighter tank's pilot
/// seat) fires the missile, on SECONDARY_FIRE, at an enemy tank, and the
/// blaster, on FIRE, at a soldier.
#[test]
fn a_seat_fires_its_missile_at_a_vehicle_and_its_cannon_at_a_soldier() {
    let Some((mut s, bot, [b, m])) = gunner_scene(vec![blaster(), launcher()], 5.0).map3() else {
        unreachable!()
    };
    enemy_hover(&mut s, Vec3::new(3.0, 1.5, 50.0));
    let got = shots_of(&mut s, bot, 12);
    assert!(
        !got.is_empty() && got.iter().all(|w| *w == m as u32),
        "{got:?}"
    );
    assert_eq!(s.unit(bot).unwrap().ai.seat.weapon, 1);
    let (mut s, bot, _) = gunner_scene(vec![blaster(), launcher()], 5.0);
    dummy(&mut s, 1, Vec3::new(-2.0, 0.0, 30.0));
    let got = shots_of(&mut s, bot, 6);
    assert!(
        !got.is_empty() && got.iter().all(|w| *w == b as u32),
        "{got:?}"
    );
    assert_eq!(s.unit(bot).unwrap().ai.seat.weapon, 0);
}

/// A ground seat fires only its first two weapons (FIRE and
/// SECONDARY_FIRE): a third that alone could hurt the tank is never chosen,
/// and the seat holds its fire.
#[test]
fn a_ground_seat_never_selects_a_weapon_above_its_second() {
    let (mut s, bot, _) = gunner_scene(vec![blaster(), blaster(), launcher()], 5.0);
    let (_, rider) = enemy_hover(&mut s, Vec3::new(3.0, 1.5, 50.0));
    let got = shots_of(&mut s, bot, 6);
    let st = &s.unit(bot).unwrap().ai;
    assert_eq!(st.target, Some(rider.into()));
    assert!(st.seat.weapon <= 1 && st.seat.hold, "{:?}", st.seat);
    assert_eq!(got, Vec::<u32>::new());
}

/// A rifle-kit bot in a seat whose cannon reaches 150 m sees an enemy hover
/// at 130 m: its own kit's scan radius (about 109 m) would not. The hull's
/// 6 m sphere sizes the body sight at 150 m, so only the scan radius was in
/// the way.
#[test]
fn a_seated_bot_sees_a_tank_by_its_seat_guns_reach() {
    let (mut s, bot, _) = gunner_scene(vec![cannon()], 6.0);
    let u = s.unit(bot).unwrap();
    let own = crate::perception::scan_radius(&s, u);
    let seat = crate::perception::seat_reach(&s, u);
    assert!(own < 130.0 && seat > 130.0, "own {own} seat {seat}");
    let (_, rider) = enemy_hover(&mut s, Vec3::new(5.0, 1.5, 130.0));
    settle(&mut s, 2);
    assert_eq!(target(&s, bot), Some(rider.into()));
}

/// A hover tank's driver turns its hull to an enemy tank 60 m off, 40 deg
/// off its way, and fires on its hull while it drives on (here no route: it
/// creeps toward its post and the tail turns to the aim point); with a seat
/// weapon that cannot hurt the tank it neither turns nor fires.
#[test]
fn a_tank_pilot_turns_to_and_fires_at_an_enemy_tank() {
    let run = |weapon: WeaponClass| {
        let mut s = flat();
        s.hover_params = vec![Some(skiff())];
        let c = s.add_weapon_class(weapon);
        s.seat_setup = vec![Some(seats(5.0, vec![Some(gun(&[c], true))]))];
        post(&mut s, Vec3::new(0.0, 0.0, 300.0));
        let v = hull(&mut s, VehicleKind::Hover, 0, Vec3::new(0.0, 1.5, 0.0), 0.0);
        let bot = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -3.0))
            })
            .unwrap();
        assert!(s.seat_unit(bot, v, 0));
        let (a, b) = 40f32.to_radians().sin_cos();
        let (ev, rider) = enemy_hover(&mut s, Vec3::new(60.0 * a, 1.5, 60.0 * b));
        let (mut shots, mut hits, mut turned) = (0, 0, 0.0f32);
        for _ in 0..10 * TICK_HZ {
            s.step(&[]);
            for e in &s.events {
                if let Event::Shot {
                    unit, origin, dir, ..
                } = *e
                {
                    if unit == bot {
                        shots += 1;
                        // A shot on the enemy hull: its line passes within
                        // the hull's 5 m sphere (the test world has no hull
                        // meshes for a bolt to strike).
                        let c = s.vehicle(ev).map_or(Vec3::ZERO, |x| x.pos);
                        let off = c - origin;
                        let along = off.dot(dir.normalize());
                        if along > 0.0 && (off - dir.normalize() * along).length() < 5.0 {
                            hits += 1;
                        }
                    }
                }
            }
            let f = s.vehicle(v).unwrap().rot * Vec3::Z;
            turned = turned.max(f.x.atan2(f.z).to_degrees().abs());
        }
        assert_eq!(s.unit(bot).unwrap().ai.target, Some(rider.into()));
        (shots, hits, turned)
    };
    let (shots, hits, turned) = run(cannon());
    assert!(
        shots > 0 && 4 * hits >= 3 * shots,
        "shots {shots} on the hull {hits}"
    );
    assert!(turned > 30.0, "turned {turned} deg");
    // Control: a blaster cannot hurt the tank - no turn, no shot.
    let (shots, _, turned) = run(blaster());
    assert_eq!(shots, 0);
    assert!(turned < 5.0, "turned {turned} deg");
}

/// On its route (a path follower: the throttle is the route's), a strafing
/// hover's driver fighting an enemy tank 40 deg off its way turns its hull
/// to it (the aim order and bit 5) and keeps its throttle; resting from a
/// stalled duel, or in a hover without a StrafeSpeed, it holds its route.
#[test]
fn a_strafing_hover_turns_to_its_enemy_while_it_drives_its_route() {
    use crate::ai::route::NavState;
    use crate::ai::state::Route;
    use crate::ai::NavTick;
    use crate::perception::Contact;
    let sticks = |strafe: f32, rest: u64| {
        let mut s = flat();
        let mut hp = skiff();
        hp.strafe_speed = strafe;
        s.hover_params = vec![Some(hp)];
        let c = s.add_weapon_class(cannon());
        s.seat_setup = vec![Some(seats(5.0, vec![Some(gun(&[c], true))]))];
        let v = hull(&mut s, VehicleKind::Hover, 0, Vec3::new(0.0, 1.5, 0.0), 0.0);
        let bot = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -3.0))
            })
            .unwrap();
        assert!(s.seat_unit(bot, v, 0));
        let (a, b) = 40f32.to_radians().sin_cos();
        let at = Vec3::new(60.0 * a, 1.5, 60.0 * b);
        let (ev, rider) = enemy_hover(&mut s, at);
        let m = s.unit(bot).unwrap().mount.unwrap();
        let mut st = AiState {
            nav: NavState::for_fixture(Some(Route {
                dest_hub: 1,
                next_hub: 1,
                waypoint: Vec3::new(0.0, 0.0, 300.0),
                ..Route::default()
            })),
            target: Some(rider.into()),
            seat: SeatFight {
                at: Some(m),
                duel: Some(ev),
                duel_hp: 4000.0,
                rest_until: rest,
                ..SeatFight::default()
            },
            ..AiState::default()
        };
        st.contacts[0] = Some(Contact {
            handle: rider.into(),
            first_seen: 0,
            last_seen: 0,
            last_pos: at,
            seen_now: true,
            heard: false,
            told: false,
        });
        let hp = s.hover_params[0].clone().unwrap();
        crate::ai::hover_drive::drive(&s, bot, m, &hp, &mut st, NavTick::default());
        st.ride
            .and_then(|r| r.hover)
            .expect("the driver's sticks")
            .sticks
    };
    let [thr, _, turn] = sticks(4.0, 0);
    assert!(thr > 0.9 && turn.abs() > 0.3, "thr {thr} turn {turn}");
    let [thr, _, turn] = sticks(4.0, 1000);
    assert!(
        thr > 0.9 && turn.abs() < 0.05,
        "resting: thr {thr} turn {turn}"
    );
    let [thr, _, turn] = sticks(0.0, 0);
    assert!(
        thr > 0.9 && turn.abs() < 0.05,
        "no strafe: thr {thr} turn {turn}"
    );
}

/// A walker's pilot (its gun yaws on its own) takes an enemy hover 60 m off
/// over a soldier at 25 m, and fires at it.
#[test]
fn a_walker_pilot_engages_an_enemy_hover_in_range() {
    let mut s = flat();
    s.walker_params = vec![Some(atst()), None];
    s.hover_params = vec![None, Some(skiff())];
    let c = s.add_weapon_class(cannon());
    s.seat_setup = vec![
        Some(seats(3.0, vec![Some(gun(&[c], true))])),
        Some(seats(5.0, Vec::new())),
    ];
    post(&mut s, Vec3::new(0.0, 0.0, 150.0));
    let v = hull(&mut s, VehicleKind::Walker, 0, Vec3::ZERO, 0.0);
    let bot = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -6.0))
        })
        .unwrap();
    assert!(s.seat_unit(bot, v, 0));
    let ev = hull(
        &mut s,
        VehicleKind::Hover,
        1,
        Vec3::new(20.0, 1.5, 56.0),
        0.0,
    );
    let rider = dummy(&mut s, 1, Vec3::new(20.0, 0.0, 56.0));
    assert!(s.seat_unit(rider, ev, 0));
    dummy(&mut s, 1, Vec3::new(-8.0, 0.0, 24.0));
    settle(&mut s, 2);
    assert_eq!(target(&s, bot), Some(rider.into()));
    let got = shots_of(&mut s, bot, 8);
    assert!(got.iter().any(|w| *w == c as u32), "{got:?}");
}

/// Destructures a scene's classes into a fixed array.
trait Map3 {
    fn map3<const N: usize>(self) -> Option<(Sim, UnitHandle, [usize; N])>;
}

impl Map3 for (Sim, UnitHandle, Vec<usize>) {
    fn map3<const N: usize>(self) -> Option<(Sim, UnitHandle, [usize; N])> {
        let (s, b, v) = self;
        Some((s, b, v.try_into().ok()?))
    }
}

/// A seat's missile is not fired at a tank with a friend on foot beside it
/// (the friend would die in the blast); without him it is (the test above).
#[test]
fn a_seat_holds_its_missile_with_a_friend_by_the_tank() {
    let (mut s, bot, _) = gunner_scene(vec![blaster(), launcher()], 5.0);
    let (_, rider) = enemy_hover(&mut s, Vec3::new(3.0, 1.5, 50.0));
    dummy(&mut s, 0, Vec3::new(8.0, 0.0, 50.0));
    let got = shots_of(&mut s, bot, 12);
    assert_eq!(target(&s, bot), Some(rider.into()));
    assert_eq!(got, Vec::<u32>::new());
}

/// A seat whose only gun has no long reach (`MaxRange` 60, the class
/// default many seat guns take) on a vehicle with no speed term still sees
/// an enemy hover 100 m off: the seat never sees less than its rider's own
/// kit did (`perception::seat_reach`).
#[test]
fn a_short_gunned_seat_still_sees_as_far_as_its_rider() {
    let short = WeaponClass {
        ai_range: [0.0, 30.0, 60.0],
        ..cannon()
    };
    let (mut s, bot, _) = gunner_scene(vec![short], 6.0);
    s.hover_params[0].as_mut().unwrap().forward_speed = 0.0;
    let u = s.unit(bot).unwrap();
    let own = crate::perception::scan_radius(&s, u);
    let guns = seat_scan_radius(&s, u).unwrap();
    let seat = crate::perception::seat_reach(&s, u);
    assert!(guns < 100.0 && own > 100.0, "own {own} guns {guns}");
    assert_eq!(seat, own);
    let (_, rider) = enemy_hover(&mut s, Vec3::new(5.0, 1.5, 100.0));
    settle(&mut s, 2);
    let st = &s.unit(bot).unwrap().ai;
    assert!(
        st.contacts
            .iter()
            .flatten()
            .any(|c| c.handle == Foe::from(rider)),
        "{:?}",
        st.contacts
    );
}

/// A hover tank's driver in a duel with an enemy tank its gun cannot dent
/// (the hull's health is kept full) gives up after [`DUEL_STALL`]: it rests
/// for [`DUEL_REST`] (no aim order, no duel), then takes the duel up again.
#[test]
fn a_stalled_duel_rests_then_resumes() {
    let mut s = flat();
    s.hover_params = vec![Some(skiff())];
    let c = s.add_weapon_class(cannon());
    s.seat_setup = vec![Some(seats(5.0, vec![Some(gun(&[c], true))]))];
    // The enemy keeps 60 m ahead of it on its route, in reach throughout.
    post(&mut s, Vec3::new(0.0, 0.0, 300.0));
    let v = hull(&mut s, VehicleKind::Hover, 0, Vec3::new(0.0, 1.5, 0.0), 0.0);
    let bot = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 0.0, -3.0))
        })
        .unwrap();
    assert!(s.seat_unit(bot, v, 0));
    let (ev, _) = enemy_hover(&mut s, Vec3::new(15.0, 1.5, 60.0));
    let (mut start, mut rest_at, mut resumed) = (None, None, None);
    let mut aimed = false;
    for _ in 0..20 * TICK_HZ {
        s.step(&[]);
        let ahead = s.vehicle(v).unwrap().pos + Vec3::new(15.0, 0.0, 60.0);
        let x = s.vehicle_mut(ev).unwrap();
        x.health.cur = x.health.max;
        x.pos = ahead;
        let tick = s.tick;
        let st = &s.unit(bot).unwrap().ai;
        let aim = hover_aim(&s, bot, st).is_some();
        if start.is_none() {
            if st.seat.duel == Some(ev) {
                start = Some(tick);
            }
        } else if rest_at.is_none() {
            aimed |= aim;
            if st.seat.rest_until > tick {
                rest_at = Some(tick);
            }
        } else if tick < st.seat.rest_until {
            // Resting: no duel, no aim order (the hull drives its route).
            assert_eq!(st.seat.duel, None, "tick {tick}");
            assert!(!aim, "tick {tick}");
        } else if st.seat.duel == Some(ev) && aim {
            resumed = Some(tick);
            break;
        }
    }
    assert!(aimed, "the hull was never turned to the enemy");
    let start = start.expect("a duel");
    let rest_at = rest_at.expect("a rest after the stall");
    let resumed = resumed.expect("the duel taken up again");
    assert!(
        rest_at - start > DUEL_STALL && rest_at - start <= DUEL_STALL + TICK_HZ as u64,
        "duel {start} rest {rest_at}"
    );
    assert!(
        resumed - rest_at >= DUEL_REST && resumed - rest_at <= DUEL_REST + 2 * TICK_HZ as u64,
        "rest {rest_at} resumed {resumed}"
    );
}

/// [ours] (the maintainer, 2026-11-20: the short vehicle and building guns'
/// bolts fly 200 m) A seat cannon whose 80 m bolt was lengthened to 200 m
/// (`OrdnanceClass::extend_bolt_reach`) does not open on an enemy hover
/// 100 m off, inside its seat's sight and its bolt's reach: its bot opens
/// within the disc's reach (`WeaponClass::engage_range`). Control: the same
/// seat with a cannon whose disc bolt reaches 200 m fires at it; and the
/// lengthened cannon fires at one 60 m off.
#[test]
fn a_lengthened_seat_gun_opens_at_the_disc_reach() {
    let disc = WeaponClass {
        ordnance: OrdnanceClass {
            life_span: 80.0 / 300.0,
            ..cannon().ordnance
        },
        ..cannon()
    };
    let mut long = disc.clone();
    long.ordnance.extend_bolt_reach();
    assert!(long.shot_range() >= crate::weapon::BOLT_REACH);
    assert!((long.engage_range() - 80.0).abs() < 1e-3);
    let far = WeaponClass {
        ordnance: OrdnanceClass {
            life_span: 200.0 / 300.0 + 1e-4,
            ..disc.ordnance.clone()
        },
        ..disc.clone()
    };
    let run = |w: WeaponClass, dist: f32| {
        let (mut s, bot, _) = gunner_scene(vec![w], 6.0);
        let (_, rider) = enemy_hover(&mut s, Vec3::new(5.0, 1.5, dist));
        settle(&mut s, 2);
        let seen = target(&s, bot) == Some(rider.into());
        (shots_of(&mut s, bot, 6).len(), seen)
    };
    let (shots, seen) = run(long.clone(), 100.0);
    assert!(seen, "the seat sees it");
    assert_eq!(shots, 0, "the lengthened gun holds past the disc's reach");
    let (shots, seen) = run(far, 100.0);
    assert!(seen && shots > 0, "control: a 200 m disc bolt fires");
    let (shots, _) = run(long, 60.0);
    assert!(shots > 0, "inside the disc's reach it fires");
}
