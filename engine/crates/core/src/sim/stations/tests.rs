//! Card PWST's referees (`analysis/re-powerup-station.md` §3, §4): the
//! rate, the remainder, the reach, the team rule, the ammo cap.
use super::*;
use crate::sim::tests::flat_ground;
use crate::sim::PadState;
use crate::unit::UnitSpec;
use crate::TICK_HZ;

mod destruction;

/// The disc's health droids (§2.2): BF1 +15, BF2 +25 every 1.0 s; the ammo
/// droid +1 clip every 2.0 s.
fn health_droid(bf2: bool) -> StationClass {
    StationClass::soldier(
        "com_item_healthrecharge",
        1.0,
        if bf2 { 25.0 } else { 15.0 },
        0,
    )
}

fn ammo_droid() -> StationClass {
    StationClass::soldier("com_item_weaponrecharge", 2.0, 0.0, 1)
}

#[test]
fn hero_refuses_station_health_but_keeps_ammo_and_regular_service() {
    use crate::soldier::GroundRules;
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut s = sim(StationClass::soldier("mixed", 0.5, 25.0, 1));
        s.ground_rules = rules;
        s.heroes = Some(crate::sim::heroes::Heroes::new([Some(1), None]));
        let hero = s
            .spawn(&UnitSpec {
                class: 1,
                ..UnitSpec::trooper(0, Vec3::X)
            })
            .unwrap();
        let regular = s.spawn(&UnitSpec::trooper(1, Vec3::NEG_X)).unwrap();
        for h in [hero, regular] {
            s.units[h.index as usize].health.cur = 100.0;
            s.units[h.index as usize].weapon.spare = 0;
        }
        s.station_pass(0);
        assert_eq!(s.unit(hero).unwrap().health.cur, 100.0);
        assert_eq!(s.unit(regular).unwrap().health.cur, 125.0);
        assert!(s.unit(hero).unwrap().weapon.spare > 0);
        assert!(s.events.iter().any(|e| matches!(e,
            Event::StationServed { unit: Some(h), health: false, ammo: true, .. } if *h == hero)));
    }
}

/// A sim with one station of `class` at the origin, `Radius` 5.0.
fn sim(class: StationClass) -> Sim {
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    let c = s.add_station_class(class);
    s.stations.push(Station::new(c, Vec3::ZERO, 5.0));
    s
}

fn step(s: &mut Sim) {
    s.step(&[PadState::default()]);
}

/// The ticks at which `unit`'s health rose, and by how much, over `ticks`.
fn gains(s: &mut Sim, unit: UnitHandle, ticks: usize) -> Vec<(usize, f32)> {
    let ui = unit.index as usize;
    let mut out = Vec::new();
    for t in 1..=ticks {
        let before = s.units[ui].health.cur;
        step(s);
        let d = s.units[ui].health.cur - before;
        if d != 0.0 {
            out.push((t, d));
        }
    }
    out
}

/// §4 (measured BF1 75 -> 300 in 15 steps of +15, BF2 100 -> 400 in 12 of
/// +25, one per 1.0 s): a hurt soldier 2 m from the droid gains the class's
/// amount once a second until his max, then nothing.
#[test]
fn a_hurt_soldier_is_healed_once_a_second_to_his_max() {
    for (bf2, from, max, step_hp, steps) in [
        (false, 75.0, 300.0, 15.0, 15),
        (true, 100.0, 400.0, 25.0, 12),
    ] {
        let mut s = sim(health_droid(bf2));
        let h = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0)))
            .unwrap();
        let u = &mut s.units[h.index as usize];
        u.health.max = max;
        u.health.cur = from;
        let g = gains(&mut s, h, 20 * TICK_HZ as usize);
        println!(
            "bf2 {bf2}: {} gains, ticks {:?}",
            g.len(),
            g.iter().map(|x| x.0).collect::<Vec<_>>()
        );
        assert_eq!(g.len(), steps, "bf2 {bf2}");
        assert!(g.iter().all(|&(_, d)| d == step_hp));
        assert_eq!(s.units[h.index as usize].health.cur, max);
        // One a second: every interval 30 ticks, give or take the one tick
        // the clock's rounding moves it.
        for w in g.windows(2) {
            let gap = w[1].0 - w[0].0;
            assert!((29..=31).contains(&gap), "gap {gap}");
        }
        let mean = (g[steps - 1].0 - g[0].0) as f32 / (steps - 1) as f32 / TICK_HZ as f32;
        assert!((mean - 1.0).abs() < 0.01, "mean period {mean}");
        // Full: the station stops serving.
        assert!(!s.stations[0].serving);
    }
}

/// §4: the clock keeps the remainder. A 0.05 s station on a 1/30 s tick
/// gives 20 times a second (dropping the remainder would give 15).
#[test]
fn the_clock_keeps_its_remainder() {
    let mut s = sim(StationClass::soldier("fast", 0.05, 1.0, 0));
    let h = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let u = &mut s.units[h.index as usize];
    u.health.max = 10_000.0;
    u.health.cur = 1.0;
    let g = gains(&mut s, h, 3 * TICK_HZ as usize);
    println!("{} gains in 3 s", g.len());
    assert!((59..=60).contains(&g.len()), "{}", g.len());
}

/// §3 (served at 4.56 / 4.60 m, not at 5.34-5.55 m): the reach is the
/// sphere, strict and in 3-D.
#[test]
fn the_reach_is_the_radius_sphere() {
    for (pos, want) in [
        (Vec3::new(4.6, 0.0, 0.0), true),
        (Vec3::new(5.4, 0.0, 0.0), false),
        (Vec3::new(0.0, 0.0, -4.9), true),
        // On the sphere: not inside.
        (Vec3::new(3.0, 0.0, 4.0), false),
        // 4 m out and 3 m up is 5 m away: a floor above is out of reach.
        (Vec3::new(4.0, 3.0, 0.0), false),
        (Vec3::new(4.0, 2.9, 0.0), true),
    ] {
        // The soldier stands on the flat ground; the station sits `pos.y`
        // under it.
        let mut s = sim(health_droid(false));
        s.stations[0].pos = Vec3::new(0.0, -pos.y, 0.0);
        let feet = Vec3::new(pos.x, 0.0, pos.z);
        let h = s.spawn(&UnitSpec::trooper(0, feet)).unwrap();
        s.units[h.index as usize].health.cur = 100.0;
        let mut got = false;
        for _ in 0..40 {
            step(&mut s);
            assert_eq!(s.units[h.index as usize].soldier.pos, feet);
            got |= s.units[h.index as usize].health.cur > 100.0;
        }
        assert_eq!(got, want, "at {pos}");
    }
}

/// §4: no team is read. Both teams' soldiers inside are served by the same
/// pass; a seated one and a dead one are not.
#[test]
fn every_team_is_served_and_the_dead_are_not() {
    let mut s = sim(health_droid(true));
    let a = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(1.0, 0.0, 0.0)))
        .unwrap();
    let b = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(-1.0, 0.0, 0.0)))
        .unwrap();
    let d = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 1.0)))
        .unwrap();
    for h in [a, b, d] {
        s.units[h.index as usize].health.cur = 100.0;
    }
    s.units[d.index as usize].alive = false;
    s.units[d.index as usize].respawn_at = None;
    let mut served = Vec::new();
    for _ in 0..TICK_HZ + 1 {
        step(&mut s);
        for e in &s.events {
            if let Event::StationServed {
                unit: Some(u),
                health: true,
                ..
            } = e
            {
                served.push(*u);
            }
        }
    }
    assert_eq!(
        served,
        vec![a, b],
        "one pass serves both teams, not the dead"
    );
    assert_eq!(s.units[a.index as usize].health.cur, 125.0);
    assert_eq!(s.units[b.index as usize].health.cur, 125.0);
    assert_eq!(s.units[d.index as usize].health.cur, 100.0);
    assert!(s.stations[0].serving);
}

/// §4 (BF1 measured: reserves 0 -> max clips - 1, +1 every 2.0 s): the ammo
/// droid adds one clip to the weapon every 2 s, to its cap.
#[test]
fn the_ammo_droid_adds_a_clip_every_two_seconds() {
    let mut s = sim(ammo_droid());
    let h = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(2.0, 0.0, 0.0)))
        .unwrap();
    let ui = h.index as usize;
    let rpc = s.weapon_classes[0].rounds_per_clip;
    let full = s.units[ui].weapon.spare;
    s.units[ui].weapon.spare = 0;
    let mut when = Vec::new();
    for t in 1..=20 * TICK_HZ as usize {
        let before = s.units[ui].weapon.spare;
        step(&mut s);
        let now = s.units[ui].weapon.spare;
        if now != before {
            assert_eq!(now - before, rpc, "one clip a pass");
            when.push(t);
        }
    }
    println!("clips at ticks {when:?}");
    assert_eq!(s.units[ui].weapon.spare, full);
    assert_eq!(when.len() as u32, full / rpc);
    for w in when.windows(2) {
        let gap = w[1] - w[0];
        assert!((59..=61).contains(&gap), "gap {gap}");
    }
}

/// A full unit standing by it is not served, and the station does not
/// report serving.
#[test]
fn a_full_unit_is_not_served() {
    let mut s = sim(health_droid(false));
    s.spawn(&UnitSpec::trooper(0, Vec3::new(1.0, 0.0, 0.0)))
        .unwrap();
    for _ in 0..3 * TICK_HZ {
        step(&mut s);
        assert!(!s
            .events
            .iter()
            .any(|e| matches!(e, Event::StationServed { .. })));
    }
    assert!(!s.stations[0].serving);
}
