use super::*;
use crate::sim::{Station, StationClass};
use crate::unit::UnitSpec;
use crate::vehicle::flyer::FlyerParams;
use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
use crate::weapon::{DamageDesc, Health, HealthType};
use glam::Vec3;

fn desc(damage: f32) -> DamageDesc {
    DamageDesc {
        damage,
        shield_scale: 1.0,
        scales: [1.0; 7],
    }
}

fn station(sim: &mut Sim, health: f32, team: u8) -> u16 {
    if sim.station_classes.is_empty() {
        sim.add_station_class(StationClass::soldier("part", 1.0, 0.0, 0));
    }
    let mut s = Station::new(0, Vec3::ZERO, 0.0);
    s.health = Health::new(health, HealthType::Building);
    s.team = Some(team);
    sim.stations.push(s);
    (sim.stations.len() - 1) as u16
}

struct World {
    sim: Sim,
    ship: [u16; 2],
    generator: u16,
    engine: u16,
    console: u16,
}

fn world(limit: u32) -> World {
    let mut sim = Sim::empty(1);
    let ship = [station(&mut sim, 1000.0, 1), station(&mut sim, 1000.0, 1)];
    let generator = station(&mut sim, 100.0, 1);
    let engine = station(&mut sim, 500.0, 1);
    let console = station(&mut sim, 50.0, 1);
    let mut sa = SpaceAssault::new(limit);
    sa.critical = [ship[0], engine, console]
        .iter()
        .map(|&station| CriticalSystem {
            station,
            points: 18,
            full_health: 1000.0,
            objective: None,
            shown: true,
        })
        .collect();
    sa.shields = vec![ShieldGroup::new(
        vec![ship[0], ship[1], engine],
        Some(generator),
        2000.0,
        100.0,
    )];
    sa.destroyables = vec![vec![vec![engine], vec![console]]];
    sim.install_space_assault(sa);
    World {
        sim,
        ship,
        generator,
        engine,
        console,
    }
}

fn soldier(sim: &mut Sim, team: u8) -> UnitHandle {
    let h = sim.spawn(&UnitSpec::trooper(team, Vec3::ZERO)).unwrap();
    sim.unit_mut(h).unwrap().controller = Controller::Ai;
    h
}

#[test]
fn a_hit_on_one_part_drains_the_whole_ship() {
    let mut w = world(180);
    w.sim
        .damage_station(w.ship[0], &desc(600.0), 1.0, None, Some(0));
    for s in [w.ship[0], w.ship[1], w.engine] {
        let health = w.sim.stations[usize::from(s)].health;
        assert_eq!(health.shield, 1400.0);
        assert_eq!(health.cur, health.max);
    }
}

#[test]
fn shields_refill_at_their_add_rate_up_to_the_pool() {
    let mut w = world(180);
    w.sim
        .damage_station(w.ship[0], &desc(150.0), 1.0, None, Some(0));
    w.sim.tick_space_assault(1.0);
    assert_eq!(w.sim.stations[usize::from(w.ship[1])].health.shield, 1950.0);
    w.sim.tick_space_assault(1.0);
    assert_eq!(w.sim.stations[usize::from(w.ship[1])].health.shield, 2000.0);
}

#[test]
fn a_hit_on_a_station_tells_its_shooter_even_when_the_shield_takes_it() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    let landed = |sim: &Sim| {
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::HitLanded { shooter, .. } if *shooter == killer))
            .count()
    };
    let shielded = w.ship[1];
    assert!(w.sim.stations[usize::from(shielded)].health.shield > 0.0);
    w.sim
        .damage_station(shielded, &desc(150.0), 1.0, Some(killer), Some(0));
    assert_eq!(landed(&w.sim), 1);
    w.sim
        .damage_station(shielded, &desc(150.0), 1.0, None, Some(0));
    assert_eq!(landed(&w.sim), 1, "no shooter to tell");
}

#[test]
fn killing_the_generator_collapses_the_shields_at_once_and_stops_the_refill() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    w.sim
        .damage_station(w.generator, &desc(100.0), 1.0, Some(killer), Some(0));
    assert!(w.sim.stations[usize::from(w.generator)].destroyed);
    assert_eq!(w.sim.stations[usize::from(w.ship[0])].health.shield, 0.0);
    w.sim.tick_space_assault(1.0);
    assert_eq!(w.sim.stations[usize::from(w.ship[0])].health.shield, 0.0);
    assert_eq!(w.sim.space_team_points().unwrap().0, [0, 0]);
}

#[test]
fn draining_the_pool_kills_the_generator_without_a_score() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    w.sim
        .damage_station(w.ship[1], &desc(2000.0), 1.0, Some(killer), Some(0));
    assert!(w.sim.stations[usize::from(w.generator)].destroyed);
    assert_eq!(w.sim.space_team_points().unwrap().0, [0, 0]);
}

#[test]
fn a_rebuilt_generator_restores_the_full_pool() {
    let mut w = world(180);
    w.sim
        .damage_station(w.generator, &desc(100.0), 1.0, None, Some(0));
    w.sim.repair_station(w.generator, 100.0);
    w.sim.tick_space_assault(0.1);
    assert_eq!(w.sim.stations[usize::from(w.ship[0])].health.shield, 2000.0);
}

#[test]
fn a_critical_system_scores_once_and_its_link_scores_nothing() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    w.sim
        .damage_station(w.generator, &desc(100.0), 1.0, Some(killer), Some(0));
    w.sim
        .damage_station(w.engine, &desc(500.0), 1.0, Some(killer), Some(0));
    assert!(w.sim.stations[usize::from(w.console)].destroyed);
    assert_eq!(w.sim.space_team_points().unwrap().0, [18, 0]);
    assert_eq!(
        w.sim.stations[usize::from(w.engine)].health.max,
        SPENT_MAX_HEALTH
    );
    w.sim.repair_station(w.engine, 1_000_000.0);
    assert!(w.sim.stations[usize::from(w.engine)].destroyed);
}

#[test]
fn a_console_kill_also_downs_its_exterior_partner_for_one_score() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    w.sim
        .damage_station(w.console, &desc(50.0), 1.0, Some(killer), Some(0));
    assert!(w.sim.stations[usize::from(w.engine)].destroyed);
    assert_eq!(w.sim.space_team_points().unwrap().0, [18, 0]);
}

#[test]
fn a_bot_scores_nothing_for_its_own_ship_and_a_human_pays() {
    let mut w = world(180);
    let bot = soldier(&mut w.sim, 1);
    w.sim
        .damage_station(w.console, &desc(1e9), 1.0, Some(bot), None);
    assert_eq!(w.sim.space_team_points().unwrap().0, [0, 0]);
    let mut w = world(180);
    let human = soldier(&mut w.sim, 1);
    w.sim.unit_mut(human).unwrap().controller = Controller::Player { slot: 0 };
    w.sim
        .damage_station(w.console, &desc(1e9), 1.0, Some(human), None);
    assert_eq!(w.sim.space_team_points().unwrap().0, [0, -18]);
}

#[test]
fn the_limit_ends_the_round_for_the_scorer() {
    let mut w = world(18);
    let killer = soldier(&mut w.sim, 0);
    w.sim
        .damage_station(w.console, &desc(50.0), 1.0, Some(killer), Some(0));
    assert_eq!(w.sim.outcome, Some(0));
    assert!(w
        .sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TeamDefeated { team: 1 })));
}

#[test]
fn deaths_do_not_spend_reinforcements() {
    let mut w = world(180);
    let before = w.sim.battle.teams[0].tickets();
    w.sim.battle.on_death(0, false);
    assert_eq!(w.sim.battle.teams[0].tickets(), before);
}

#[test]
fn meters_read_the_shared_pool_and_the_systems_health() {
    let mut w = world(180);
    w.sim
        .damage_station(w.ship[0], &desc(2500.0), 1.0, None, Some(0));
    let m = w.sim.capital_meters().unwrap();
    assert_eq!(m[1].shield, 0.0);
    assert!(m[1].hull > 0.0 && m[1].hull < 1.0);
}

#[test]
fn hash_carries_the_points_only_in_a_space_round() {
    let a = Sim::empty(1).hash();
    let mut w = world(180);
    let b = w.sim.hash();
    assert_ne!(a, b);
    w.sim.space_assault.as_mut().unwrap().points[0] = 3;
    assert_ne!(w.sim.hash(), b);
}

fn flyer(sim: &mut Sim, team: u8) -> VehicleHandle {
    sim.flyer_params = vec![Some(FlyerParams::default())];
    sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 50.0, 0.0),
        yaw: 0.0,
        max_health: 100.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    })
}

fn manned_flyer(sim: &mut Sim, team: u8) -> VehicleHandle {
    let h = flyer(sim, team);
    let pilot = soldier(sim, team);
    assert!(sim.seat_unit(pilot, h, 0));
    h
}

#[test]
fn a_manned_enemy_fighter_is_worth_three() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    let h = manned_flyer(&mut w.sim, 1);
    w.sim.destroy_vehicle(h, Some(killer));
    assert_eq!(
        w.sim.space_team_points().unwrap().0,
        [FIGHTER_KILL_POINTS + KILL_AI_POINTS, 0]
    );
}

#[test]
fn friendly_and_environmental_fighter_deaths_score_nothing() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 1);
    let h = manned_flyer(&mut w.sim, 1);
    w.sim.destroy_vehicle(h, Some(killer));
    let h_empty = flyer(&mut w.sim, 1);
    w.sim.destroy_vehicle(h_empty, Some(killer));
    let h = manned_flyer(&mut w.sim, 0);
    w.sim.destroy_vehicle(h, None);
    assert_eq!(w.sim.space_team_points().unwrap().0, [0, 0]);
}

#[test]
fn an_empty_enemy_flyer_is_worth_one() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    let h = flyer(&mut w.sim, 1);
    w.sim.destroy_vehicle(h, Some(killer));
    assert_eq!(
        w.sim.space_team_points().unwrap().0,
        [UNMANNED_FLYER_POINTS, 0]
    );
}

#[test]
fn a_ground_round_keeps_conquest_rules() {
    let mut sim = Sim::empty(1);
    let killer = soldier(&mut sim, 0);
    let h = manned_flyer(&mut sim, 1);
    sim.destroy_vehicle(h, Some(killer));
    assert_eq!(sim.space_team_points(), None);
    assert!(!sim.battle.unlimited_reinforcements);
}

fn bot_kill(sim: &mut Sim, victim: UnitHandle, killer: Option<UnitHandle>) -> [i32; 2] {
    sim.unit_mut(victim).unwrap().health.cur = 0.0;
    sim.unit_killed(victim, killer);
    sim.space_team_points().unwrap().0
}

#[test]
fn an_enemy_bot_kill_scores_one_and_a_human_kill_two() {
    let mut w = world(180);
    let killer = soldier(&mut w.sim, 0);
    let bot = soldier(&mut w.sim, 1);
    assert_eq!(bot_kill(&mut w.sim, bot, Some(killer)), [1, 0]);
    let human = soldier(&mut w.sim, 1);
    w.sim.unit_mut(human).unwrap().spec.player_slot = Some(0);
    w.sim.unit_mut(human).unwrap().is_player = true;
    assert_eq!(bot_kill(&mut w.sim, human, Some(killer)), [3, 0]);
}

#[test]
fn a_bot_never_loses_points_and_a_human_does() {
    let mut w = world(180);
    let bot = soldier(&mut w.sim, 0);
    let mate = soldier(&mut w.sim, 0);
    assert_eq!(bot_kill(&mut w.sim, mate, Some(bot)), [0, 0]);
    assert_eq!(bot_kill(&mut w.sim, bot, Some(bot)), [0, 0]);
    let human = soldier(&mut w.sim, 0);
    w.sim.unit_mut(human).unwrap().is_player = true;
    let mate = soldier(&mut w.sim, 0);
    assert_eq!(bot_kill(&mut w.sim, mate, Some(human)), [-2, 0]);
    let human_2 = soldier(&mut w.sim, 1);
    w.sim.unit_mut(human_2).unwrap().is_player = true;
    assert_eq!(bot_kill(&mut w.sim, human_2, None), [-2, -1]);
    assert_eq!(bot_kill(&mut w.sim, human, Some(human)), [-3, -1]);
}

#[test]
fn a_death_reaching_the_limit_ends_the_round() {
    let mut w = world(1);
    let killer = soldier(&mut w.sim, 1);
    let bot = soldier(&mut w.sim, 0);
    bot_kill(&mut w.sim, bot, Some(killer));
    assert_eq!(w.sim.outcome, Some(1));
}

#[test]
fn a_grouped_objective_reports_once_on_its_last_system() {
    let mut w = world(180);
    let sa = w.sim.space_assault.as_mut().unwrap();
    sa.critical[0].objective = Some(0);
    sa.critical[1].objective = Some(0);
    sa.objectives_left = vec![2];
    w.sim.space_station_destroyed(w.ship[0], None);
    let down = |sim: &Sim| {
        sim.events
            .iter()
            .filter(|e| matches!(e, Event::SpaceObjectiveDown { .. }))
            .count()
    };
    assert_eq!(down(&w.sim), 0);
    w.sim.space_station_destroyed(w.engine, None);
    assert_eq!(down(&w.sim), 1);
    w.sim.space_station_destroyed(w.engine, None);
    assert_eq!(down(&w.sim), 1);
}

#[test]
fn time_limit_ends_round_in_favor_of_team_with_more_points() {
    let mut w = world(180);
    w.sim.time_limit = Some(2); // 2 seconds = 60 ticks
    w.sim.space_assault.as_mut().unwrap().points = [10, 5];
    for _ in 0..59 {
        w.sim.step(&[]);
        assert_eq!(w.sim.outcome, None);
    }
    w.sim.step(&[]);
    assert_eq!(w.sim.outcome, Some(0));
    assert!(w
        .sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TimeUp { winner: 0 })));
}

#[test]
fn time_limit_draw_when_points_equal() {
    use crate::sim::OUTCOME_DRAW;
    let mut w = world(180);
    w.sim.time_limit = Some(2);
    w.sim.space_assault.as_mut().unwrap().points = [10, 10];
    for _ in 0..60 {
        w.sim.step(&[]);
    }
    assert_eq!(w.sim.outcome, Some(OUTCOME_DRAW));
    assert!(w
        .sim
        .events
        .iter()
        .any(|e| matches!(e, Event::TimeUp { winner } if *winner == OUTCOME_DRAW)));
}
