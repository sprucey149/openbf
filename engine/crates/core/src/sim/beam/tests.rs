use super::*;
use crate::sim::tests::{flat_ground, hits};
use crate::unit::{UnitHandle, UnitSpec};
use crate::weapon::{BeamClass, OrdnanceClass, WeaponClass};

fn award_rifle() -> BeamClass {
    BeamClass {
        range: 500.0,
        life: 0.15,
        pierce: true,
        impact: None,
    }
}

fn scene() -> (Sim, usize, UnitHandle) {
    let mut s = Sim::empty(5);
    s.heightfield = Some(flat_ground());
    let class = s.add_weapon_class(WeaponClass {
        ordnance: OrdnanceClass {
            max_damage: 30.0,
            beam: Some(award_rifle()),
            ..OrdnanceClass::default()
        },
        ..WeaponClass::default()
    });
    let shooter = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    (s, class, shooter)
}

fn shooter(owner: UnitHandle, class: usize) -> Shooter {
    Shooter {
        owner,
        team: 0,
        weapon_class: class,
        is_player: true,
    }
}

fn open(s: &mut Sim, class: usize, owner: UnitHandle, follow: bool) {
    let round = Bolt::fire(
        Vec3::new(0.0, 1.4, 0.0),
        Vec3::X,
        150.0,
        0.15,
        shooter(owner, class),
        None,
        None,
        &s.collision,
        HullFilter::All,
    );
    s.launch_round(round, follow);
}

fn run(s: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        s.fly_bolts(crate::TICK_DT);
    }
}

#[test]
fn a_beam_spreads_its_damage_over_its_life() {
    let (mut s, class, owner) = scene();
    let target = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(20.0, 0.0, 0.0)))
        .unwrap();
    open(&mut s, class, owner, false);
    run(&mut s, 10);
    let got = hits(&s);
    assert_eq!(got.len(), 5);
    for (victim, damage) in got {
        assert_eq!(victim, target.index);
        assert!((damage - 30.0 * crate::TICK_DT / 0.15).abs() < 1e-3);
    }
    assert!(s.beams.is_empty());
    assert!(s.bolts.is_empty());
}

fn line_of_troopers(s: &mut Sim, at: &[f32]) -> Vec<UnitHandle> {
    at.iter()
        .map(|&x| {
            s.spawn(&UnitSpec::trooper(1, Vec3::new(x, 0.0, 0.0)))
                .unwrap()
        })
        .collect()
}

/// A 1 m slab across the beam's line, its near face at x = 29.5.
fn wall(s: &mut Sim) {
    use crate::collision::{Body, Shape};
    s.collision.push_body(Body::new(
        glam::Mat4::from_translation(Vec3::new(30.0, 5.0, 0.0)),
        Shape::Box {
            half: Vec3::new(0.5, 5.0, 5.0),
        },
    ));
}

fn struck(s: &Sim) -> Vec<u32> {
    hits(s).iter().map(|h| h.0).collect()
}

#[test]
fn a_beam_without_pass_through_stops_on_the_first_soldier() {
    let (mut s, class, owner) = scene();
    s.weapon_classes[class]
        .ordnance
        .beam
        .as_mut()
        .unwrap()
        .pierce = false;
    let line = line_of_troopers(&mut s, &[10.0, 20.0]);
    open(&mut s, class, owner, false);
    run(&mut s, 1);
    assert_eq!(struck(&s), [line[0].index]);
    let len = s.beams[0].len;
    assert!(len > 9.0 && len < 10.5, "{len}");
}

/// PassThrough (BF2 FUN_00311d00): each soldier on the line takes his
/// share once a tick, and the ray ends at the wall, so the one behind it
/// is safe and the drawn beam reaches the wall.
#[test]
fn a_pass_through_beam_strikes_each_soldier_in_line_and_stops_at_a_wall() {
    let (mut s, class, owner) = scene();
    wall(&mut s);
    let line = line_of_troopers(&mut s, &[10.0, 20.0, 40.0]);
    open(&mut s, class, owner, false);
    run(&mut s, 1);
    assert_eq!(struck(&s), [line[0].index, line[1].index]);
    let share = 30.0 * crate::TICK_DT / 0.15;
    for (_, damage) in hits(&s) {
        assert!((damage - share).abs() < 1e-3, "{damage}");
    }
    let len = s.beams[0].len;
    assert!((len - 29.5).abs() < 1e-3, "{len}");
    s.events.clear();
    run(&mut s, 1);
    assert_eq!(struck(&s), [line[0].index, line[1].index], "once a tick");
}

/// A vehicle is passed too: the hull and the soldier behind it each take
/// a share.
#[test]
fn a_pass_through_beam_goes_on_through_a_hull() {
    use crate::collision::{HullPart, Layers, Shape};
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let (mut s, class, owner) = scene();
    s.hull_parts = vec![Some(
        vec![HullPart::new(
            glam::Mat4::from_translation(Vec3::Y),
            Shape::Box {
                half: Vec3::new(2.0, 1.0, 3.0),
            },
            Layers::ALL,
        )]
        .into(),
    )];
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(10.0, 0.0, 0.0),
        yaw: 0.0,
        max_health: 900.0,
        health_type: crate::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let behind = line_of_troopers(&mut s, &[20.0]);
    open(&mut s, class, owner, false);
    run(&mut s, 1);
    assert!(s.vehicle(v).unwrap().health.cur < 900.0);
    assert_eq!(struck(&s), [behind[0].index]);
}

/// Space keeps the first hit stopping every beam.
#[test]
fn a_space_round_beam_stops_on_the_first_soldier() {
    let (mut s, class, owner) = scene();
    s.space_assault = Some(crate::sim::SpaceAssault::new(10));
    let line = line_of_troopers(&mut s, &[10.0, 20.0]);
    open(&mut s, class, owner, false);
    run(&mut s, 1);
    assert_eq!(struck(&s), [line[0].index]);
}

#[test]
fn a_beam_that_meets_nothing_reaches_its_range() {
    let (mut s, class, owner) = scene();
    s.heightfield = None;
    open(&mut s, class, owner, false);
    run(&mut s, 1);
    assert!((s.beams[0].len - 500.0).abs() < 1e-2);
    assert!((s.beams[0].life - (0.15 - crate::TICK_DT)).abs() < 1e-6);
    assert!(hits(&s).is_empty());
}

#[test]
fn a_held_beam_follows_its_shooters_fire_point() {
    let (mut s, class, _) = scene();
    let bot = s
        .spawn(&UnitSpec::trooper(0, Vec3::new(0.0, 0.0, 5.0)))
        .unwrap();
    open(&mut s, class, bot, true);
    run(&mut s, 1);
    s.units[bot.index as usize].soldier.pos = Vec3::new(3.0, 0.0, 5.0);
    run(&mut s, 1);
    let u = &s.units[bot.index as usize];
    assert_eq!(s.beams[0].origin, u.muzzle());
    assert_eq!(s.beams[0].dir, u.aim_frame().0);
}

#[test]
fn a_beam_hashes_only_while_it_lives() {
    let (mut s, class, owner) = scene();
    let before = s.hash();
    open(&mut s, class, owner, false);
    assert_ne!(s.hash(), before);
    s.beams.clear();
    assert_eq!(s.hash(), before);
}

/// Issue #168 lag compensation: a lagged shooter's beam, and its piercing
/// chain, sweep the volumes his view lag back; with no lag, the present.
#[test]
fn a_lagged_beam_meets_the_soldiers_where_its_shooter_saw_them() {
    let strike = |lag: u8| {
        let (mut s, class, owner) = scene();
        let line = line_of_troopers(&mut s, &[10.0, 20.0]);
        // Two ticks with the troopers on the line, then they step off it.
        for _ in 0..2 {
            s.tick += 1;
            s.record_hit_frame();
        }
        for t in &line {
            s.units[t.index as usize].soldier.pos.z = 3.0;
        }
        s.tick += 1;
        s.record_hit_frame();
        s.view_lags = vec![0; s.units.len()];
        s.view_lags[owner.index as usize] = lag;
        open(&mut s, class, owner, false);
        run(&mut s, 1);
        struck(&s)
    };
    let (s, _, _) = scene();
    let first = s.units.len() as u32;
    assert_eq!(strike(3), [first, first + 1], "both, as they stood");
    assert_eq!(strike(0), Vec::<u32>::new(), "the line is empty now");
}
