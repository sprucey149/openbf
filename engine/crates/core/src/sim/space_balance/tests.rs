use super::*;
use crate::sim::space_assault::{CriticalSystem, SpaceAssault};
use crate::sim::{Station, StationClass};
use crate::unit::UnitSpec;
use crate::vehicle::flyer::FlyerParams;
use crate::vehicle::gun::{Aimer, GunSetup, GunWeapon};
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::{Health, HealthType};

const FAR_AWAY: f32 = 50_000.0;

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

fn world() -> Sim {
    let mut s = Sim::empty(1);
    s.flyer_params = vec![Some(FlyerParams::default())];
    s.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 10.0,
        ai_size: 1,
        guns: vec![Some(gun())],
    })];
    let w = &mut s.weapon_classes[0];
    w.ordnance.max_damage = 100.0;
    w.ordnance.scales = [1.0; 7];
    w.shot_delay = [0.5, 0.5];
    s.install_space_assault(SpaceAssault::new(180));
    s
}

fn flyer(s: &mut Sim, team: u8, pos: Vec3) -> VehicleHandle {
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

fn piloted(s: &mut Sim, team: u8, pos: Vec3) -> (UnitHandle, VehicleHandle) {
    let v = flyer(s, team, pos);
    let u = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, pos)
        })
        .unwrap();
    assert!(s.seat_unit(u, v, 0));
    (u, v)
}

fn system(s: &mut Sim, team: u8, pos: Vec3, shield: f32) -> u16 {
    s.add_station_class(StationClass::soldier("part", 1.0, 0.0, 0));
    let mut st = Station::new(0, pos, 0.0);
    st.health = Health::new(1000.0, HealthType::Building);
    st.health.shield = shield;
    st.team = Some(team);
    st.sphere_centre = pos;
    st.sphere_radius = 1.0;
    s.stations.push(st);
    let station = (s.stations.len() - 1) as u16;
    s.space_assault
        .as_mut()
        .unwrap()
        .critical
        .push(CriticalSystem {
            station,
            points: 6,
            full_health: 1000.0,
            objective: None,
            shown: true,
        });
    station
}

fn add_player(s: &mut Sim, team: u8) -> UnitHandle {
    s.spawn(&UnitSpec::player(team, Vec3::ZERO)).unwrap()
}

fn behind(s: &Sim, h: UnitHandle) -> Vec3 {
    let (pos, dir) = s.units[h.index as usize].aim_ray(s.ground_rules, s.views());
    pos - dir * 500.0
}

fn ahead(s: &Sim, h: UnitHandle) -> Vec3 {
    let (pos, dir) = s.units[h.index as usize].aim_ray(s.ground_rules, s.views());
    pos + dir * 500.0
}

#[test]
fn the_nudges_follow_the_points_gap_past_its_margin() {
    assert_eq!(balance_nudges(50, 30), (0, 0), "a gap of 20 is inside");
    assert_eq!(balance_nudges(55, 25), (-1, 1));
    assert_eq!(balance_nudges(25, 55), (1, -1));
    assert_eq!(balance_nudges(180, 0), (-4, 4), "held at four");
}

#[test]
fn the_time_factor_quickens_when_the_human_leads_and_slows_when_behind() {
    assert_eq!(balance_time_factor(0), 1.0);
    assert_eq!(balance_time_factor(-1), 0.5);
    assert_eq!(balance_time_factor(-4), 0.2);
    assert_eq!(balance_time_factor(3), 3.0);
}

#[test]
fn a_weapon_with_a_clip_counts_its_reload() {
    assert_eq!(fire_rate_value(10.0, 0, 1, [0.5, 0.5], 0.0), 20.0);
    assert_eq!(fire_rate_value(10.0, 10, 2, [0.5, 0.5], 2.5), 100.0 / 5.0);
}

#[test]
fn a_flyer_in_the_players_view_is_never_chosen() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let seen = ahead(&s, p);
    let unseen = behind(&s, p);
    let (_, in_view) = piloted(&mut s, 1, seen);
    assert_eq!(s.balance_unseen_flyer(1, true), None);
    let (_, out) = piloted(&mut s, 1, unseen);
    assert_eq!(s.balance_unseen_flyer(1, true).map(|f| f.1), Some(out));
    assert!(s.vehicle(in_view).is_some());
}

#[test]
fn a_kill_takes_the_chosen_team_and_credits_an_unseen_enemy() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    let (_, victim) = piloted(&mut s, 0, unseen);
    piloted(&mut s, 1, unseen + Vec3::X * 40.0);
    s.balance_fighter_kill(0);
    assert!(s.vehicle(victim).is_none());
    assert_eq!(
        s.space_team_points().unwrap().0,
        [0, FIGHTER_KILL_TOTAL],
        "the same points as a kill in the open"
    );
}

const FIGHTER_KILL_TOTAL: i32 =
    crate::sim::space_assault::FIGHTER_KILL_POINTS + crate::sim::space_assault::KILL_AI_POINTS;

#[test]
fn nothing_dies_without_an_unseen_flyer_on_both_sides() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    let (_, lonely) = piloted(&mut s, 0, unseen);
    s.balance_fighter_kill(0);
    assert!(s.vehicle(lonely).is_some());
}

#[test]
fn shields_take_at_most_what_they_hold_and_the_cap() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    piloted(&mut s, 1, unseen);
    let target = system(&mut s, 0, unseen + Vec3::X * 40.0, 300.0);
    s.balance_system_attack(0);
    let st = &s.stations[usize::from(target)];
    assert_eq!(st.health.shield, 0.0, "200 x 5 is held to the 300 left");
    assert_eq!(st.health.cur, 1000.0);
}

#[test]
fn a_bare_system_takes_the_weapons_value() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    piloted(&mut s, 1, unseen);
    let target = system(&mut s, 0, unseen + Vec3::X * 40.0, 0.0);
    s.balance_system_attack(0);
    assert_eq!(s.stations[usize::from(target)].health.cur, 800.0);
}

#[test]
fn a_system_in_view_or_on_the_wrong_side_is_left_alone() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let (seen, unseen) = (ahead(&s, p), behind(&s, p));
    piloted(&mut s, 1, unseen);
    let visible = system(&mut s, 0, seen, 0.0);
    let theirs = system(&mut s, 1, unseen, 0.0);
    s.balance_system_attack(0);
    assert_eq!(s.stations[usize::from(visible)].health.cur, 1000.0);
    assert_eq!(s.stations[usize::from(theirs)].health.cur, 1000.0);
}

#[test]
fn a_system_without_an_icon_is_never_attacked() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    piloted(&mut s, 1, unseen);
    let target = system(&mut s, 0, unseen, 0.0);
    s.space_assault.as_mut().unwrap().critical[0].shown = false;
    s.balance_system_attack(0);
    assert_eq!(s.stations[usize::from(target)].health.cur, 1000.0);
}

#[test]
fn without_a_human_nothing_runs_and_no_randomness_is_drawn() {
    let mut s = world();
    let unseen = Vec3::splat(FAR_AWAY);
    let (_, a) = piloted(&mut s, 0, unseen);
    let (_, b) = piloted(&mut s, 1, unseen + Vec3::X * 40.0);
    let before = s.rng.state();
    for t in 0..(BALANCE_QUIET_SECS as u64 * 40 * u64::from(TICK_HZ)) {
        s.tick = t;
        s.tick_space_balance();
    }
    assert_eq!(s.rng.state(), before);
    assert!(s.vehicle(a).is_some() && s.vehicle(b).is_some());
    assert!(!s.space_assault.as_ref().unwrap().balance.started);
}

#[test]
fn nothing_happens_in_the_first_thirty_seconds() {
    let mut s = world();
    let p = add_player(&mut s, 0);
    let unseen = behind(&s, p);
    let (_, a) = piloted(&mut s, 0, unseen);
    let (_, b) = piloted(&mut s, 1, unseen + Vec3::X * 40.0);
    for t in 0..(BALANCE_QUIET_SECS as u64 * u64::from(TICK_HZ) - 1) {
        s.tick = t;
        s.tick_space_balance();
    }
    assert!(s.vehicle(a).is_some() && s.vehicle(b).is_some());
}

#[test]
fn the_same_seed_runs_the_same_attacks() {
    let run = || {
        let mut s = world();
        let p = add_player(&mut s, 0);
        let unseen = behind(&s, p);
        for k in 0..6 {
            piloted(&mut s, k % 2, unseen + Vec3::X * 40.0 * f32::from(k));
        }
        system(&mut s, 0, unseen, 0.0);
        system(&mut s, 1, unseen, 0.0);
        for t in 0..(600 * u64::from(TICK_HZ)) {
            s.tick = t;
            s.tick_space_balance();
        }
        s.hash()
    };
    assert_eq!(run(), run());
}
