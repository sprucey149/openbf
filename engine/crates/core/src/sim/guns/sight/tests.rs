use crate::sim::guns::tests::{pad, tap, world};
use crate::sim::{buttons, Event, PadState};
use crate::unit::UnitSpec;
use glam::Vec3;

#[test]
fn the_sight_follows_the_gunners_bolt_and_stops_on_the_soldier_it_covers() {
    let (mut s, p, v) = world(false);
    assert_eq!(s.seat_sight_ray(p), None);
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, 30.0)))
        .unwrap();
    tap(&mut s, buttons::ACTION);
    tap(&mut s, buttons::SWITCH_WEAPON);
    tap(&mut s, buttons::SWITCH_WEAPON);
    let veh = s.vehicle(v).unwrap();
    let eye = veh.pos + Vec3::new(0.0, 0.8, 3.3) + Vec3::new(0.0, 1.0, 2.0);
    let chest = Vec3::new(0.0, 1.0, 30.0);
    let want = ((chest - eye).y / (chest - eye).length()).asin();
    let down = PadState {
        ry: PadState::quantize(-1.0),
        ..PadState::default()
    };
    while s.seat_look(p).unwrap().look.1 > want {
        s.step(&[down]);
    }
    for _ in 0..30 {
        s.step(&[pad(0)]);
    }
    let hash = s.hash();
    let (origin, dir) = s.seat_sight_ray(p).unwrap();
    let hit = s.seat_sight_point(p).unwrap();
    assert_eq!(s.hash(), hash);
    let body = s.unit(enemy).unwrap().capsule();
    let reach = hit.distance(origin);
    assert!(reach < 40.0, "the sight stops on the soldier: {hit:?}");
    assert!(body.ray_hit(origin, dir, reach + 0.1).is_some());
    s.events.clear();
    s.step(&[pad(buttons::FIRE)]);
    let shot = s
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot {
                unit, origin, dir, ..
            } if *unit == p => Some((*origin, *dir)),
            _ => None,
        })
        .expect("a shot");
    assert!(shot.0.distance(origin) < 0.05, "{:?} {origin:?}", shot.0);
    assert!(shot.1.dot(dir) > 0.999, "{:?} {dir:?}", shot.1);
}

#[test]
fn the_sight_runs_between_parallel_barrels_and_holds_still_as_they_alternate() {
    let (mut s, p, v) = world(true);
    let points = [Vec3::new(2.0, 0.0, 3.0), Vec3::new(-2.0, 0.0, 3.0)];
    s.seat_setup[0].as_mut().unwrap().guns[0]
        .as_mut()
        .unwrap()
        .weapons[0]
        .fire_points = points.to_vec();
    s.vehicle_mut(v).unwrap().kind = crate::vehicle::VehicleKind::Flyer;
    s.vehicle_mut(v).unwrap().flyer.phase = crate::vehicle::flyer::FlightPhase::Flying;
    assert!(s.seat_unit(p, v, 0));
    s.seat_gun(p.index as usize, &pad(0));
    let veh = s.vehicle(v).unwrap();
    let middle = veh.pos + veh.rot * ((points[0] + points[1]) / 2.0);
    let first = s.seat_sight_ray(p).unwrap();
    assert!(first.0.distance(middle) < 1e-5, "{first:?} {middle:?}");
    s.vehicle_mut(v).unwrap().guns[0].as_mut().unwrap().weapons[0].shots_fired += 1;
    let second = s.seat_sight_ray(p).unwrap();
    assert!(second.0.distance(first.0) < 1e-5);
    assert!(second.1.dot(first.1) > 0.999_999);
}
