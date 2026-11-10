use super::*;
use crate::sim::space_assault::{CriticalSystem, ShieldGroup, SpaceAssault};
use crate::sim::{Event, Station, StationClass};
use crate::unit::UnitSpec;
use crate::vehicle::flyer::FlyerParams;
use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
use crate::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
use crate::weapon::{Health, HealthType};

const STEP: f32 = 1.0 / 30.0;

fn gun() -> GunSetup {
    GunSetup {
        weapons: vec![GunWeapon {
            class: 0,
            infinite: true,
            reserve_clips: 0,
        }],
        fire_point: Vec3::ZERO,
        pivot: Vec3::ZERO,
        eye: Vec3::ZERO,
        aimer: Aimer::default(),
        pilot: true,
        turn: None,
        framed: false,
        aimer_bind: glam::Quat::IDENTITY,
        gunner: None,
    }
}

fn fighter(s: &mut Sim, team: u8, pos: Vec3) -> VehicleHandle {
    s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 100.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    })
}

fn world() -> (Sim, UnitHandle, VehicleHandle) {
    let mut s = Sim::empty(1);
    s.flyer_params = vec![Some(FlyerParams::default())];
    let w = &mut s.weapon_classes[0];
    w.target_kind = VEHICLE_KIND_BIT;
    w.target_lock_max = 1000.0;
    w.target_lock_lose = 1100.0;
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 1,
        guns: vec![Some(gun())],
    })];
    let own = fighter(&mut s, 0, Vec3::ZERO);
    let p = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    assert!(s.seat_unit(p, own, 0));
    (s, p, own)
}

fn pads(s: &Sim, held: bool) -> Vec<PadState> {
    let mut pads = vec![PadState::default(); s.units.len()];
    if held {
        pads[0].buttons = buttons::RELOAD;
    }
    pads
}

fn tap(s: &mut Sim) {
    let down = pads(s, true);
    s.tick_lock_on(&down, STEP);
    let up = pads(s, false);
    s.tick_lock_on(&up, STEP);
}

fn ahead(s: &Sim, h: UnitHandle, metres: f32, side: f32) -> Vec3 {
    let (pos, dir) = s.flyer_ray(h);
    let across = dir.cross(Vec3::Y).normalize_or_zero();
    pos + dir * metres + across * side
}

#[test]
fn a_press_takes_the_target_nearest_the_aim_then_the_next() {
    let (mut s, p, _) = world();
    let centre = ahead(&s, p, 500.0, 0.0);
    let aside = ahead(&s, p, 200.0, 150.0);
    let near = fighter(&mut s, 1, centre);
    let off = fighter(&mut s, 1, aside);
    tap(&mut s);
    assert_eq!(
        s.lock_on[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(near)))
    );
    tap(&mut s);
    assert_eq!(
        s.lock_on[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(off)))
    );
    tap(&mut s);
    assert_eq!(
        s.lock_on[0].target,
        Some(LockTarget::Aim(AimTarget::Vehicle(near))),
        "the exhausted list starts again"
    );
}

#[test]
fn friends_and_fighters_past_the_range_are_never_taken() {
    let (mut s, p, _) = world();
    let friend = ahead(&s, p, 100.0, 0.0);
    let far = ahead(&s, p, 1001.0, 0.0);
    fighter(&mut s, 0, friend);
    fighter(&mut s, 1, far);
    tap(&mut s);
    assert_eq!(s.lock_on[0].target, None);
}

#[test]
fn holding_the_button_clears_the_target() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 100.0, 0.0);
    fighter(&mut s, 1, at);
    tap(&mut s);
    assert!(s.lock_on[0].target.is_some());
    let down = pads(&s, true);
    let ticks = (LOCK_CLEAR_HOLD_SECS / STEP).ceil() as usize + 1;
    for _ in 0..ticks {
        s.tick_lock_on(&down, STEP);
    }
    assert_eq!(s.lock_on[0].target, None);
}

#[test]
fn a_target_past_the_lose_distance_drops() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 100.0, 0.0);
    let far = ahead(&s, p, 1101.0, 0.0);
    let v = fighter(&mut s, 1, at);
    tap(&mut s);
    assert!(s.lock_on[0].target.is_some());
    s.vehicles[v.index as usize].pos = far;
    let up = pads(&s, false);
    s.tick_lock_on(&up, STEP);
    assert_eq!(s.lock_on[0].target, None);
}

#[test]
fn leaving_the_flyer_forgets_the_lock() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 100.0, 0.0);
    fighter(&mut s, 1, at);
    tap(&mut s);
    assert!(s.lock_on[0].target.is_some());
    s.unit_mut(p).unwrap().mount = None;
    let up = pads(&s, false);
    s.tick_lock_on(&up, STEP);
    assert_eq!(s.lock_on[0], LockOn::default());
}

fn enemy_ship(s: &mut Sim, at: Vec3, lock: bool) -> (u16, u16) {
    s.add_station_class(StationClass::soldier("part", 1.0, 0.0, 0));
    let add = |s: &mut Sim, pos: Vec3| {
        let mut st = Station::new(0, pos, 0.0);
        st.health = Health::new(100.0, HealthType::Building);
        st.team = Some(1);
        st.sphere_centre = pos;
        s.stations.push(st);
        (s.stations.len() - 1) as u16
    };
    let part = add(s, at);
    let generator = add(s, at + Vec3::X * 200.0);
    let mut sa = SpaceAssault::new(180);
    let mut group = ShieldGroup::new(vec![part], Some(generator), 2000.0, 100.0);
    group.lock_when_down = lock;
    sa.shields = vec![group];
    sa.critical = vec![CriticalSystem {
        station: generator,
        points: 6,
        full_health: 100.0,
        objective: None,
        shown: true,
    }];
    s.install_space_assault(sa);
    (part, generator)
}

#[test]
fn a_hull_only_takes_a_lock_once_its_shields_fall() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 300.0, 0.0);
    let (part, generator) = enemy_ship(&mut s, at, true);
    assert!(s
        .lock_probe(LockTarget::Station(part))
        .is_some_and(|l| l.kind_bit == BUILDING_KIND_BIT));
    s.space_station_destroyed(generator, None);
    assert!(s
        .lock_probe(LockTarget::Station(part))
        .is_some_and(|l| l.kind_bit == VEHICLE_KIND_BIT));
    tap(&mut s);
    assert_eq!(s.lock_on[0].target, Some(LockTarget::Station(part)));
}

#[test]
fn the_shields_returning_take_the_lock_away_again() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 300.0, 0.0);
    let (part, generator) = enemy_ship(&mut s, at, true);
    s.space_station_destroyed(generator, None);
    tap(&mut s);
    assert_eq!(s.lock_on[0].target, Some(LockTarget::Station(part)));
    let g = &mut s.stations[usize::from(generator)];
    g.destroyed = false;
    g.health.cur = g.health.max;
    s.tick_space_assault(STEP);
    let up = pads(&s, false);
    s.tick_lock_on(&up, STEP);
    assert_eq!(s.lock_on[0].target, None);
}

#[test]
fn each_shield_change_reports_once_and_a_repeat_kill_awards_nothing_new() {
    let (mut s, p, _) = world();
    let at = ahead(&s, p, 300.0, 0.0);
    let (_, generator) = enemy_ship(&mut s, at, false);
    let count = |s: &Sim, want: bool| {
        s.events
            .iter()
            .filter(|e| matches!(e, Event::SpaceLinkChanged { kind: 0, up, .. } if *up == want))
            .count()
    };
    s.space_station_destroyed(generator, None);
    assert_eq!(count(&s, false), 1);
    let points = s.space_team_points().unwrap().0;
    let g = &mut s.stations[usize::from(generator)];
    g.destroyed = false;
    g.health.cur = g.health.max;
    s.tick_space_assault(STEP);
    assert_eq!(count(&s, true), 1);
    s.tick_space_assault(STEP);
    assert_eq!(count(&s, true), 1, "the return is reported once");
    s.events.clear();
    s.space_station_destroyed(generator, None);
    assert_eq!(count(&s, false), 1);
    assert_eq!(s.space_team_points().unwrap().0, points);
}

#[test]
fn a_launchers_lock_fills_only_while_the_target_holds_the_cone() {
    let (mut s, p, _) = world();
    let w = &mut s.weapon_classes[0];
    w.lock_time = 3.0;
    w.ordnance.missile = Some(Default::default());
    let centre = ahead(&s, p, 500.0, 0.0);
    let target = fighter(&mut s, 1, centre);
    assert_eq!(s.lock_on_charge(0, 0), None, "nothing to lock");
    tap(&mut s);
    let early = s.lock_on_charge(0, 0).expect("in the cone");
    assert!(early < 0.1, "{early}");
    let idle = pads(&s, false);
    for _ in 0..(3.1 / STEP) as usize {
        s.tick_lock_on(&idle, STEP);
    }
    assert_eq!(s.lock_on_charge(0, 0), Some(1.0));
    assert_eq!(s.lock_on[0].held[0], 3.0, "the dwell stops at the class's");

    let aside = ahead(&s, p, 500.0, 800.0);
    s.vehicles[target.index as usize].pos = aside;
    s.tick_lock_on(&idle, STEP);
    assert!(s.lock_on[0].target.is_some(), "the target stays selected");
    assert_eq!(s.lock_on_charge(0, 0), None, "out of the 0.9 cone");
    assert_eq!(s.lock_on[0].held[0], 0.0, "and the dwell restarts");
}

#[test]
fn a_cannon_with_no_lock_time_never_charges() {
    let (mut s, p, _) = world();
    let centre = ahead(&s, p, 500.0, 0.0);
    fighter(&mut s, 1, centre);
    tap(&mut s);
    assert!(s.lock_on[0].target.is_some());
    assert_eq!(s.lock_on_charge(0, 0), None);
}

#[test]
fn a_fully_locked_shot_keeps_its_target_when_its_tilt_leaves_the_lock_cone() {
    let (mut s, p, own) = world();
    s.vehicle_mut(own).unwrap().flyer.phase = crate::vehicle::flyer::FlightPhase::Flying;
    let w = &mut s.weapon_classes[0];
    w.lock_time = 3.0;
    w.ordnance.missile = Some(Default::default());
    w.shot_tilts = vec![(0.0, 30.0_f32.to_radians())];
    w.pitch_spread = 0.0;
    w.yaw_spread = 0.0;
    let centre = ahead(&s, p, 500.0, 0.0);
    let target = fighter(&mut s, 1, centre);
    tap(&mut s);
    let idle = pads(&s, false);
    for _ in 0..91 {
        s.tick_lock_on(&idle, STEP);
    }
    assert_eq!(s.lock_on_charge(0, 0), Some(1.0));
    let fire = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    for _ in 0..4 {
        s.seat_gun(p.index as usize, &fire);
        if !s.bolts.is_empty() {
            break;
        }
    }
    let missile = s
        .bolts
        .first()
        .expect("the locked shot fires")
        .missile
        .unwrap();
    assert!(missile.launch.dot(s.flyer_ray(p).1) < LOCK_FIRE_COS);
    assert_eq!(missile.target, Some(MissileTarget::Vehicle(target)));
}
