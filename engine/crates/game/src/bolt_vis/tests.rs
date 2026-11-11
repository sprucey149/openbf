use super::*;
use core::weapon::WeaponClass;

use core::impact::Surface;
use core::sim::{buttons, Event, PadState};

fn surface_walk(slope: f32, building: bool, heightfield: bool, pitch: usize) -> WalkSim {
    let h = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: (0..33 * 33)
            .map(|i| slope * ((i / 33) as f32 * 8.0 - 128.0))
            .collect(),
    };
    let mut walk = WalkSim::new(42, Vec3::ZERO, heightfield.then_some(h));
    if building {
        walk.sim.collision = core::collision::CollisionWorld::new(
            vec![core::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, -0.4, 0.0)),
                core::collision::Shape::Box {
                    half: Vec3::new(128.0, 0.5, 128.0),
                },
            )],
            vec![],
        );
    }
    // The built-in rifle with the existing BF2 light, as in the flat control.
    for w in &mut walk.sim.weapon_classes {
        if let Some(l) = w.ordnance.laser.as_mut() {
            l.light_color = [148, 25, 25, 255];
            l.light_radius = 4.0;
        }
    }
    for _ in 0..pitch {
        walk.step_tick(&[PadState {
            ry: -32767,
            ..PadState::default()
        }]);
    }
    walk.step_tick(&[PadState {
        buttons: buttons::FIRE,
        ..PadState::default()
    }]);
    walk
}

// Public seating and real step_tick/FIRE events are the principal witness.
// The gun deliberately differs in data as well as class identity.
fn mounted_walk(different: bool, building: bool) -> WalkSim {
    mounted_fixture(different, building, |_, _, _| {})
}

fn mounted_fixture(
    different: bool,
    building: bool,
    before_fire: impl FnOnce(&mut WalkSim, usize, usize),
) -> WalkSim {
    use core::vehicle::gun::{Aimer, GunSetup, GunWeapon};
    use core::vehicle::{SeatKind, SeatSetup, VehicleKind, VehicleSpec};
    let h = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: vec![0.0; 33 * 33],
    };
    let mut walk = WalkSim::new(42, Vec3::ZERO, Some(h));
    let rider = walk.sim.unit(walk.player).unwrap().weapon_class;
    let rifle = &mut walk.sim.weapon_classes[rider];
    let look = rifle.ordnance.laser.as_mut().unwrap();
    look.light_color = [148, 25, 25, 255];
    look.light_radius = 4.0;
    let gun = if different {
        let mut gun = rifle.clone();
        gun.ordnance.velocity *= 1.25;
        let look = gun.ordnance.laser.as_mut().unwrap();
        look.light_radius = 9.0;
        look.light_color = [25, 148, 25, 255];
        look.width *= 2.0;
        look.length *= 2.0;
        walk.sim.add_weapon_class(gun)
    } else {
        rider
    };
    walk.sim.seat_setup = vec![Some(SeatSetup {
        radius: 5.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 0.0,
        ai_size: 1,
        guns: vec![Some(GunSetup {
            weapons: vec![GunWeapon {
                class: gun,
                infinite: true,
                reserve_clips: 0,
                fire_points: Vec::new(),
            }],
            fire_point: Vec3::ZERO,
            pivot: Vec3::ZERO,
            eye: Vec3::ZERO,
            aimer: Aimer {
                pitch: [-1.57, 1.57],
                ..Aimer::default()
            },
            pilot: false,
            turn: None,
            framed: false,
            aimer_bind: glam::Quat::IDENTITY,
            gunner: None,
        })],
    })];
    let vehicle = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: Some(0),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 2.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Gunner(0)],
    });
    assert!(walk.sim.enter_vehicle(walk.player, vehicle, 0));
    assert!(walk.sim.unit(walk.player).unwrap().mount.is_some());
    if building {
        walk.sim.collision = core::collision::CollisionWorld::new(
            vec![core::collision::Body::new(
                glam::Mat4::from_translation(Vec3::new(0.0, -0.4, 0.0)),
                core::collision::Shape::Box {
                    half: Vec3::new(128.0, 0.5, 128.0),
                },
            )],
            vec![],
        );
    }
    for _ in 0..40 {
        walk.step_tick(&[PadState {
            ry: -32767,
            ..PadState::default()
        }]);
    }
    before_fire(&mut walk, rider, gun);
    walk.step_tick(&[PadState {
        buttons: buttons::FIRE,
        ..PadState::default()
    }]);
    let shots: Vec<_> = walk
        .sim
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Shot {
                unit,
                weapon_class,
                origin,
                dir,
                ..
            } if *unit == walk.player => {
                assert_eq!(*weapon_class as usize, gun);
                Some((*origin, *dir))
            }
            _ => None,
        })
        .collect();
    assert_eq!(shots.len(), 1);
    if !walk.sim.bolts.is_empty() {
        assert_eq!(walk.sim.bolts.len(), 1);
        assert!(!walk
            .sim
            .events
            .iter()
            .any(|e| matches!(e, Event::BoltExpired { .. })));
        return walk;
    }
    let head = impact(
        &walk,
        if building {
            Surface::Building
        } else {
            Surface::Terrain
        },
    );
    assert!(walk.sim.events.iter().any(|e| matches!(e,
        Event::Impact { weapon_class, .. } if *weapon_class as usize == gun)));
    assert!(walk.sim.events.iter().any(|e| matches!(e,
        Event::BoltExpired { pos, struck: true } if *pos == head)));
    assert!(walk.sim.bolts.is_empty(), "actual same-tick gun hit");
    walk
}

fn mounted_light(mut walk: WalkSim, terrain: bool, label: &str) {
    let (origin, dir) = walk
        .sim
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot { origin, dir, .. } => Some((*origin, *dir)),
            _ => None,
        })
        .unwrap();
    let head = walk
        .sim
        .events
        .iter()
        .find_map(|e| match e {
            Event::BoltExpired { pos, struck: true } => Some(*pos),
            _ => None,
        })
        .unwrap();
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 1);
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(
        lights[0].pos.abs_diff_eq((origin + head) * 0.5, 1e-4),
        "reaching midpoint"
    );
    let normal = if terrain {
        Vec3::from(
            walk.sim
                .heightfield
                .as_ref()
                .unwrap()
                .normal_at(head.x, head.z),
        )
    } else {
        -dir
    };
    walk.step_tick(&[PadState::default()]);
    visual_update(&mut vis, &walk);
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(
        lights[0].pos.abs_diff_eq(head + normal * 0.5, 1e-4),
        "{label}: actual={:?}, expected={:?}, error={}",
        lights[0].pos,
        head + normal * 0.5,
        lights[0].pos.distance(head + normal * 0.5)
    );
    walk.step_tick(&[PadState::default()]);
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 0);
    assert!(vis.omni_lights(&walk).is_empty());
}

#[test]
fn mounted_different_class_terrain_actual_step() {
    mounted_light(
        mounted_walk(true, false),
        true,
        "valid gun-class terrain impact",
    );
}

#[test]
fn mounted_same_class_terrain_control() {
    mounted_light(
        mounted_walk(false, false),
        true,
        "same-class terrain control",
    );
}

#[test]
fn mounted_building_control() {
    mounted_light(mounted_walk(true, true), false, "mounted Building");
}

fn mounted_decoy(field: usize) {
    let mut walk = mounted_walk(true, true);
    let rider = walk.sim.unit(walk.player).unwrap().weapon_class;
    let mut decoy = *walk
        .sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .unwrap();
    if let Event::Impact {
        pos,
        dir,
        weapon_class,
        surface,
        ..
    } = &mut decoy
    {
        *surface = Surface::Terrain;
        match field {
            0 => *weapon_class = rider as u32,
            1 => *pos += Vec3::X,
            2 => *dir = -*dir,
            _ => unreachable!(),
        }
    }
    walk.sim.events.insert(0, decoy);
    mounted_light(
        walk,
        false,
        match field {
            0 => "reject unrelated rider-class terrain decoy",
            1 => "reject displaced gun-class terrain decoy",
            2 => "reject reversed gun-class terrain decoy",
            _ => unreachable!(),
        },
    );
}

#[test]
fn mounted_decoy_class() {
    mounted_decoy(0);
}
#[test]
fn mounted_decoy_position() {
    mounted_decoy(1);
}
#[test]
fn mounted_decoy_direction() {
    mounted_decoy(2);
}

#[test]
fn mounted_fired_class_appearance() {
    let walk = mounted_fixture(true, false, |walk, rider, gun| {
        for (class, width, length, glow, texture, color) in [
            (rider, 0.1, 0.4, 0.6, "Rider", [200, 20, 20, 100]),
            (gun, 0.3, 0.8, 1.2, "Gun", [20, 200, 20, 150]),
        ] {
            let l = walk.sim.weapon_classes[class]
                .ordnance
                .laser
                .as_mut()
                .unwrap();
            l.width = width;
            l.length = length;
            l.glow_length = glow;
            l.texture = Some(texture.into());
            l.glow_color = color;
            l.fade_light = false;
        }
    });
    let gun = walk
        .sim
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot { weapon_class, .. } => Some(*weapon_class as usize),
            _ => None,
        })
        .unwrap();
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 1);
    let m = &vis.remnants[0];
    assert_eq!(m.class, gun, "stored fired class");
    let light = vis.omni_lights(&walk)[0];
    assert_eq!(light.radius, 9.0, "gun light radius");
    assert_eq!(light.rgb, [25, 148, 25], "gun light color");
    assert!(
        (m.age - m.len / walk.sim.weapon_classes[gun].ordnance.velocity).abs() < 1e-6,
        "gun flight age"
    );
    let look = walk.sim.weapon_classes[m.class]
        .ordnance
        .laser
        .as_ref()
        .unwrap();
    assert_eq!(texture_name(look.texture.as_deref().unwrap()), "gun");
    let draw = bolt_draw(look, m.head, m.dir, m.len, m.age, m.fade, [None, None]);
    assert_eq!(draw.width, 0.3);
    assert_eq!(draw.core_len, 0.8);
    assert_eq!(draw.glow_len, 1.2);
    assert_eq!(draw.core_color, [255; 4]);
    assert_eq!(draw.glow_color, [20, 200, 20, 150]);
}

#[test]
fn mounted_rider_without_laser() {
    let walk = mounted_fixture(true, false, |w, rider, _| {
        w.sim.weapon_classes[rider].ordnance.laser = None;
    });
    mounted_light(walk, true, "laser gun with nonlaser rider");
}

#[test]
fn mounted_slow_rider() {
    let walk = mounted_fixture(true, false, |w, rider, _| {
        w.sim.weapon_classes[rider].ordnance.velocity = 0.1;
    });
    mounted_light(walk, true, "gun reach with slow rider");
}

#[test]
fn mounted_gun_without_laser() {
    let walk = mounted_fixture(true, false, |w, _, gun| {
        w.sim.weapon_classes[gun].ordnance.laser = None;
    });
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 0, "nonlaser gun has no remnant");
    assert!(vis.omni_lights(&walk).is_empty());
}

#[test]
fn mounted_slower_gun_rejects_distant_expiry() {
    let mut walk = mounted_fixture(true, false, |w, rider, gun| {
        w.sim.weapon_classes[rider].ordnance.velocity *= 2.0;
        w.sim.weapon_classes[gun].ordnance.velocity *= 0.1;
    });
    // The real shot has not reached its terrain hit. An unrelated struck
    // expiry on that ray lies inside only the rider's longer first step.
    let (origin, dir, gun) = walk
        .sim
        .events
        .iter()
        .find_map(|e| match e {
            Event::Shot {
                origin,
                dir,
                weapon_class,
                ..
            } => Some((*origin, *dir, *weapon_class as usize)),
            _ => None,
        })
        .unwrap();
    assert_eq!(walk.sim.bolts.len(), 1, "slow gun still in flight");
    let step = walk.sim.weapon_classes[gun].ordnance.velocity * core::TICK_DT;
    walk.sim.events.push(Event::BoltExpired {
        pos: origin + dir * (step * 2.0),
        struck: true,
    });
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 0, "gun reach rejects unrelated expiry");
}

#[test]
fn mounted_gun_fade_and_bf1_light_control() {
    for building in [false, true] {
        let mut walk = mounted_fixture(true, building, |w, rider, gun| {
            w.sim.weapon_classes[rider]
                .ordnance
                .laser
                .as_mut()
                .unwrap()
                .fade_out_rate = 60.0;
            let l = w.sim.weapon_classes[gun].ordnance.laser.as_mut().unwrap();
            l.fade_out_rate = 6.0;
            l.fade_light = true;
            l.fade_start = 0.0;
            l.fade_duration = 0.05;
            l.end_light_color = [100, 40, 200, 255];
            l.end_glow_color = [10, 30, 70, 100];
        });
        let mut vis = lit_vis();
        let mut bf1 = BoltVis::new(Path::new("unused-fixture-assets"), "bf1");
        visual_update(&mut vis, &walk);
        visual_update(&mut bf1, &walk);
        let m = vis.remnants[0].clone();
        let l = walk.sim.weapon_classes[m.class]
            .ordnance
            .laser
            .as_ref()
            .unwrap()
            .clone();
        assert_eq!(vis.omni_lights(&walk)[0].pos, (m.prev + m.head) * 0.5);
        assert_eq!(vis.omni_lights(&walk)[0].rgb, l.light(m.age).unwrap().0);
        assert!(bf1.omni_lights(&walk).is_empty());
        for tick in 1..=6 {
            walk.step_tick(&[PadState::default()]);
            visual_update(&mut vis, &walk);
            visual_update(&mut bf1, &walk);
            assert!(bf1.omni_lights(&walk).is_empty());
            if tick < 5 {
                assert_eq!(vis.remnant_count(), 1, "gun fade lifetime tick {tick}");
                let r = &vis.remnants[0];
                assert!((r.fade - (1.0 - tick as f32 * core::TICK_DT * 6.0)).abs() < 1e-6);
                assert_eq!(r.age, m.age, "flight age freezes at hit");
                let draw = bolt_draw(&l, r.head, r.dir, r.len, r.age, r.fade, [None, None]);
                assert_eq!(draw.glow_color, l.glow_bytes(m.age, r.fade));
                assert_eq!(draw.width, l.half_width(r.fade));
                let normal = if building { -m.dir } else { Vec3::Y };
                assert!(vis.omni_lights(&walk)[0]
                    .pos
                    .abs_diff_eq(m.head + normal * 0.5, 1e-4));
                assert_eq!(vis.omni_lights(&walk)[0].rgb, l.light(m.age).unwrap().0);
            }
        }
        assert_eq!(vis.remnant_count(), 0, "gun fade expires");
        assert_eq!(bf1.remnant_count(), 0);
    }
}

fn lit_vis() -> BoltVis {
    BoltVis::new(Path::new("unused-fixture-assets"), "bf2")
}

fn visual_update(vis: &mut BoltVis, walk: &WalkSim) {
    let hash = walk.sim.hash();
    let events = walk.sim.events.clone();
    vis.update(walk, core::TICK_DT);
    let _ = vis.omni_lights(walk);
    assert_eq!(
        walk.sim.hash(),
        hash,
        "visual update preserves simulation hash"
    );
    assert_eq!(walk.sim.events, events, "visual update preserves events");
}

fn impact(walk: &WalkSim, expected: Surface) -> Vec3 {
    let hits: Vec<_> = walk
        .sim
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Impact { pos, surface, .. } if *surface == expected => Some(*pos),
            _ => None,
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "legal fixture must hit {expected:?}: {:?}",
        walk.sim.events
    );
    hits[0]
}

fn check_same_tick(mut walk: WalkSim, surface: Surface) {
    let head = impact(&walk, surface);
    assert!(walk.sim.bolts.is_empty(), "actual first-tick impact");
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 1);
    let prev = vis.remnants[0].prev;
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(lights[0].pos.abs_diff_eq((head + prev) * 0.5, 1e-4));
    vis.lights = false;
    assert!(vis.omni_lights(&walk).is_empty(), "BF1 no-light control");
    vis.lights = true;
    let normal = if surface == Surface::Terrain {
        Vec3::from(
            walk.sim
                .heightfield
                .as_ref()
                .unwrap()
                .normal_at(head.x, head.z),
        )
    } else {
        -vis.remnants[0].dir
    };
    walk.step_tick(&[PadState::default()]);
    visual_update(&mut vis, &walk);
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(
        lights[0].pos.abs_diff_eq(head + normal * 0.5, 1e-4),
        "surface={surface:?} head={head:?} actual={:?} expected={:?}",
        lights[0].pos,
        head + normal * 0.5
    );
    walk.step_tick(&[PadState::default()]);
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 0);
    assert!(vis.omni_lights(&walk).is_empty());
}

#[test]
fn same_tick_steep_terrain_uses_actual_surface() {
    for pitch in 18..=22 {
        check_same_tick(surface_walk(-2.0, false, true, pitch), Surface::Terrain);
    }
}

#[test]
fn same_tick_near_ground_building_uses_actual_surface() {
    for pitch in 19..=22 {
        check_same_tick(surface_walk(0.0, true, true, pitch), Surface::Building);
    }
}

#[test]
fn same_tick_surface_controls() {
    check_same_tick(surface_walk(0.0, false, true, 22), Surface::Terrain);
    check_same_tick(surface_walk(-0.5, false, true, 20), Surface::Terrain);
    // Also exercise the renderer without a heightfield after a real hit.
    let mut no_heightfield = surface_walk(0.0, true, true, 22);
    no_heightfield.sim.heightfield = None;
    check_same_tick(no_heightfield, Surface::Building);
}

#[test]
fn tracked_multitick_terrain_keeps_surface() {
    let mut walk = surface_walk(0.0, false, true, 10);
    assert_eq!(walk.sim.bolts.len(), 1, "fixture starts in flight");
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.tracked.len(), 1);
    for _ in 0..30 {
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        if vis.remnant_count() == 0 {
            continue;
        }
        let head = impact(&walk, Surface::Terrain);
        assert_eq!(vis.remnant_count(), 1);
        assert!(vis.omni_lights(&walk)[0]
            .pos
            .abs_diff_eq((vis.remnants[0].prev + head) * 0.5, 1e-4));
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        assert!(vis.omni_lights(&walk)[0]
            .pos
            .abs_diff_eq(head + Vec3::Y * 0.5, 1e-4));
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        assert_eq!(vis.remnant_count(), 0);
        return;
    }
    panic!("tracked fixture must reach terrain");
}

#[test]
fn same_tick_fade_preserves_duration_and_light_offset() {
    let mut walk = surface_walk(-2.0, false, true, 20);
    let head = impact(&walk, Surface::Terrain);
    let normal = Vec3::from(
        walk.sim
            .heightfield
            .as_ref()
            .unwrap()
            .normal_at(head.x, head.z),
    );
    for w in &mut walk.sim.weapon_classes {
        if let Some(l) = w.ordnance.laser.as_mut() {
            l.fade_out_rate = 10.0;
        }
    }
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    for tick in 1..=4 {
        walk.step_tick(&[PadState::default()]);
        visual_update(&mut vis, &walk);
        if tick < 3 {
            assert_eq!(vis.remnant_count(), 1);
            assert!(
                (vis.remnants[0].fade - (1.0 - tick as f32 * core::TICK_DT * 10.0)).abs() < 1e-6
            );
            assert!(vis.omni_lights(&walk)[0]
                .pos
                .abs_diff_eq(head + normal * 0.5, 1e-4));
        }
    }
    assert_eq!(vis.remnant_count(), 0);
    assert!(vis.omni_lights(&walk).is_empty());
}

// Supplemental event controls deliberately put a terrain decoy before the
// real building impact. Each input must independently reject that decoy.
fn decoy_case(field: usize) {
    let mut walk = surface_walk(0.0, true, true, 22);
    let head = impact(&walk, Surface::Building);
    let mut decoy = *walk
        .sim
        .events
        .iter()
        .find(|e| matches!(e, Event::Impact { .. }))
        .unwrap();
    if let Event::Impact {
        pos,
        dir,
        weapon_class,
        surface,
        ..
    } = &mut decoy
    {
        *surface = Surface::Terrain;
        match field {
            0 => *pos += Vec3::X,
            1 => *dir = -*dir,
            2 => *weapon_class += 1,
            _ => unreachable!(),
        }
    }
    walk.sim.events.insert(0, decoy);
    let mut vis = lit_vis();
    visual_update(&mut vis, &walk);
    assert_eq!(vis.remnant_count(), 1);
    let dir = vis.remnants[0].dir;
    walk.step_tick(&[PadState::default()]);
    visual_update(&mut vis, &walk);
    assert!(
        vis.omni_lights(&walk)[0]
            .pos
            .abs_diff_eq(head - dir * 0.5, 1e-4),
        "decoy field {field} must not select terrain"
    );
}

#[test]
fn impact_decoy_position() {
    decoy_case(0);
}
#[test]
fn impact_decoy_direction() {
    decoy_case(1);
}
#[test]
fn impact_decoy_weapon_class() {
    decoy_case(2);
}

/// A BF1 rifle bolt 2 m out draws 1 m of core and 2 m of glow; at 10 m
/// the core 1 m and the glow 3 m (GlowLength), whatever BlurLength's cap.
/// A sniper remnant at fade 0.5 is 0.55·w wide, core alpha 127, glow
/// bytes halved.
#[test]
fn drawn_lengths_widths_and_fades() {
    let l = WeaponClass::bf1_rifle().ordnance.laser.unwrap();
    let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 2.0, 0.0, 1.0, [None, None]);
    assert_eq!((d.core_len, d.glow_len), (1.0, 2.0));
    assert!((d.width - 0.1).abs() < 1e-7);
    assert_eq!(d.core_color, [255, 255, 255, 255]);
    assert_eq!(d.glow_color, [253, 43, 43, 100]);
    let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 10.0, 0.0, 1.0, [None, None]);
    assert_eq!((d.core_len, d.glow_len), (1.0, 3.0));
    let d = bolt_draw(&l, Vec3::ZERO, Vec3::X, 10.0, 0.0, 0.5, [None, None]);
    assert!((d.width - 0.055).abs() < 1e-7, "{}", d.width);
    assert_eq!(d.core_color[3], 127);
    assert_eq!(d.glow_color, [126, 21, 21, 50]);
}

/// A soldier firing at the flat ground under his feet: the sim spawns
/// and resolves each bolt inside one tick, and the vis still keeps it
/// at its impact; on BF2 it lights the ground there, 0.5 m up the
/// terrain's normal, for one update; BF1 lights nothing.
#[test]
fn a_shot_at_the_ground_underfoot_is_kept_and_lit() {
    use core::sim::{buttons, PadState};
    let flat = core::heightfield::Heightfield {
        size: 33,
        unit: 8.0,
        offset_x: 128.0,
        offset_z: 128.0,
        heights: vec![0.0; 33 * 33],
    };
    let mut walk = WalkSim::new(42, Vec3::ZERO, Some(flat));
    // The built-in trooper's BF1 rifle, given BF2's rifle light.
    for w in &mut walk.sim.weapon_classes {
        if let Some(l) = w.ordnance.laser.as_mut() {
            l.light_color = [148, 25, 25, 255];
            l.light_radius = 4.0;
        }
    }
    let mut vis = BoltVis {
        lights: true,
        ..BoltVis::default()
    };
    let down = PadState {
        ry: -32767,
        ..PadState::default()
    };
    for _ in 0..40 {
        walk.step_tick(&[down]);
    }
    let fire = PadState {
        buttons: buttons::FIRE,
        ..PadState::default()
    };
    let mut kept = None;
    for _ in 0..30 {
        walk.step_tick(&[fire]);
        vis.update(&walk, core::TICK_DT);
        if walk.sim.bolts.is_empty() && !vis.remnants.is_empty() {
            kept = Some(vis.remnants[0].head);
            break;
        }
    }
    let head = kept.expect("a same-tick hit kept as a remnant");
    let feet = walk.sim.unit(walk.player).unwrap().soldier.pos;
    assert!(
        // The terrain ray's crossing is good to a quarter of its 0.5 m
        // step along the bolt.
        head.y.abs() < 0.2 && head.distance(feet) < 8.0,
        "{head} {feet}"
    );
    // The reaching update: the midpoint of the one step it flew.
    let prev = vis.remnants[0].prev;
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(lights[0].pos.abs_diff_eq((prev + head) * 0.5, 1e-4));
    vis.lights = false;
    assert!(vis.omni_lights(&walk).is_empty(), "BF1: no light");
    vis.lights = true;
    // The hit: 0.5 m up the flat ground's normal; then gone.
    let idle = PadState::default();
    walk.step_tick(&[idle]);
    vis.update(&walk, core::TICK_DT);
    let lights = vis.omni_lights(&walk);
    assert_eq!(lights.len(), 1);
    assert!(lights[0].pos.abs_diff_eq(head + Vec3::Y * 0.5, 1e-4));
    walk.step_tick(&[idle]);
    vis.update(&walk, core::TICK_DT);
    assert!(vis.omni_lights(&walk).is_empty());
}

/// The drawn line starts at the fire point for a soldier, at the spawn
/// for a rider or when the fire point would be past the end.
#[test]
fn a_soldiers_bolt_starts_at_the_fire_point() {
    let dir = Vec3::NEG_Z;
    let end = Vec3::new(0.0, 0.0, -50.0);
    let s = drawn_start(Vec3::ZERO, dir, end, true, None);
    assert!(s.abs_diff_eq(Vec3::new(0.3, 0.0, -1.2), 1e-6), "{s}");
    let fire = Vec3::new(0.2, 0.1, -0.9);
    assert_eq!(drawn_start(Vec3::ZERO, dir, end, true, Some(fire)), fire);
    assert_eq!(
        drawn_start(Vec3::ZERO, dir, end, false, Some(fire)),
        Vec3::ZERO
    );
    let near = Vec3::new(0.0, 0.0, -1.0);
    assert_eq!(drawn_start(Vec3::ZERO, dir, near, true, None), Vec3::ZERO);
    assert_eq!(texture_name("purplelaser_D"), "violetlaser_d");
    assert_eq!(texture_name("com_sfx_laser_red"), "com_sfx_laser_red");
}

#[test]
fn a_turrets_bolt_is_drawn_from_its_own_gun_not_its_owners_hand() {
    let (mut walk, weapon, gun) = crate::walk::turret_test_walk();
    let hand = Vec3::new(0.3, 1.4, -0.5);
    let mut points = vec![None; walk.sim.units.len()];
    points[walk.player.index as usize] = Some(hand);
    let mut vis = BoltVis::default();
    let mut drawn = None;
    for _ in 0..10 * core::TICK_HZ {
        walk.step_tick(&[PadState::default()]);
        vis.set_fire_points(points.clone());
        vis.update(&walk, core::TICK_DT);
        let b = walk.sim.bolts.iter().find(|b| b.weapon_class == gun);
        if let (Some(b), Some(v)) = (b, vis.tracked.iter().find(|v| v.class == gun)) {
            drawn = Some((v.start, b.pos - b.vel * b.age));
            break;
        }
    }
    let (start, spawn) = drawn.expect("the turret never fired");
    assert!(walk.sim.is_item_gun(gun) && !walk.sim.is_item_gun(weapon));
    assert!(start.abs_diff_eq(spawn, 1e-3), "{start} vs spawn {spawn}");
    let centre = walk.sim.turrets[0].centre();
    assert!(start.distance(centre) < 0.5, "{start} vs turret {centre}");
    assert!(start.distance(hand) > 10.0, "{start} at the owner's hand");
}

/// A missile round of a fixture class (the REP torpedo's numbers, card
/// UORD): the ctor's two waver phases come from `seed`, as the class's
/// `Missile::launch` draws them.
fn missile_round(seed: u32) -> (WeaponClass, core::projectile::Bolt) {
    let mut w = WeaponClass::bf1_rifle();
    let o = &mut w.ordnance;
    o.kind = core::weapon::OrdKind::Missile;
    o.velocity = 40.0;
    o.life_span = 4.0;
    o.missile = Some(core::missile::MissileClass {
        turn_rate: 2.0,
        ..core::missile::MissileClass::default()
    });
    let mut lcg = core::lcg::GameLcg::new(seed);
    let mut b = core::projectile::Bolt {
        visual_shot: None,
        pos: Vec3::new(1.0, 1.6, -2.0),
        vel: Vec3::NEG_Z * 40.0,
        remaining: 4.0,
        age: 0.0,
        owner: core::unit::UnitHandle { index: 3, gen: 2 },
        team: 0,
        weapon_class: 9,
        hit: core::projectile::Hit::None,
        end: Vec3::new(1.0, 1.6, -162.0),
        shooter_is_player: false,
        hulls: core::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    };
    let mc = *o.missile.as_ref().unwrap();
    b.as_missile(o, &mc, None, &mut lcg);
    (w, b)
}

/// §4.3: the law turns a missile BEFORE it moves, so its `vel` and `end`
/// are new values every tick. One missile must stay one round for the
/// whole flight — one trail emitter, not one per tick of its curve — and
/// two rounds of one class from one launcher must stay two.
#[test]
fn one_missile_stays_one_round_while_it_steers() {
    let (w, mut b) = missile_round(3);
    let mc = *w.ordnance.missile.as_ref().unwrap();
    let first = key(&b);
    let mut lcg = core::lcg::GameLcg::new(11);
    for tick in 0..20 {
        b.guide(
            &mc,
            Some(Vec3::new(30.0, 0.0, 0.0)),
            core::TICK_DT,
            &mut lcg,
        );
        b.advance(core::TICK_DT);
        assert_eq!(key(&b), first, "tick {tick} is the same round");
    }
    assert!(
        b.vel.normalize().angle_between(Vec3::NEG_Z) > 0.2,
        "the fixture did turn: {}",
        b.vel
    );
    // The end point is the probe's on a hit; the key does not move with
    // it either.
    b.end = Vec3::ZERO;
    assert_eq!(key(&b), first);
    // A second round of the same class, from the same launcher and the
    // same launch line, is its own round.
    let (_, other) = missile_round(4);
    assert_ne!(key(&other), first);
    // A straight bolt keeps the old identity: its line is its key.
    let mut plain = other;
    plain.missile = None;
    let k = key(&plain);
    plain.end += Vec3::X;
    assert_ne!(key(&plain), k, "a bolt's line is its identity");
}

/// The missile draw (card UORD): a round whose class has an
/// `ordnance.geometry` model belongs to `SoldierVisuals` — this module
/// keeps no ribbon and no remnant for it — and the model sits at the
/// sim's place, its +Z the way the round flies. A round with no model
/// still tracks on a class that authored a `LaserLook`, and a stopped
/// one is not drawn at all.
#[test]
fn a_bullet_with_a_model_draws_as_its_model() {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let (mut w, mut b) = missile_round(5);
    w.ordnance.missile = None;
    w.ordnance.geometry = Some("com_weap_missile".into());
    let class = walk.sim.add_weapon_class(w);
    b.missile = None;
    b.weapon_class = class;
    walk.sim.bolts.push(b);
    let listed = missiles_in_flight(&walk);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, class);
    let mut vis = BoltVis::default();
    vis.update(&walk, core::TICK_DT);
    assert!(vis.tracked.is_empty());
}

#[test]
fn a_modelled_walker_round_does_not_also_draw_its_tracer() {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let (mut w, mut b) = missile_round(5);
    w.ordnance.geometry = Some("com_weap_inf_grenadethermal_ord".into());
    let class = walk.sim.add_weapon_class(w);
    b.weapon_class = class;
    walk.walker_shot_styles.insert(
        class,
        crate::walker_shots::Style {
            tracer: Some(core::laser::LaserLook::default()),
            fire_sound: None,
            frequency: 1,
        },
    );
    walk.sim.events.push(Event::Shot {
        unit: b.owner,
        weapon_class: class as u32,
        origin: b.pos,
        dir: b.vel.normalize(),
    });
    walk.sim.bolts.push(b);
    let mut vis = BoltVis::default();
    vis.update(&walk, core::TICK_DT);
    assert_eq!(missiles_in_flight(&walk).len(), 1);
    assert!(vis.tracked.is_empty());
    assert_eq!(vis.remnant_count(), 0);
}

#[test]
fn a_missile_draws_as_its_model_not_as_a_ribbon() {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let (mut w, b) = missile_round(5);
    w.ordnance.laser = Some(core::laser::LaserLook::default());
    w.ordnance.geometry = Some("com_weap_missile".into());
    let class = walk.sim.add_weapon_class(w);
    let mut b = b;
    b.weapon_class = class;
    walk.sim.bolts.push(b);

    let mut vis = BoltVis::default();
    vis.update(&walk, core::TICK_DT);
    assert!(vis.tracked.is_empty(), "the model path draws it");
    assert!(vis.remnant_count() == 0 && vis.omni_lights(&walk).is_empty());
    let listed = missiles_in_flight(&walk);
    assert_eq!(listed.len(), 1);
    let (c, m) = listed[0];
    assert_eq!(c, class);
    assert!(m
        .transform_point3(Vec3::ZERO)
        .abs_diff_eq(Vec3::new(1.0, 1.6, -2.0), 1e-4));
    assert!(
        m.z_axis.truncate().abs_diff_eq(Vec3::NEG_Z, 1e-4),
        "{:?}",
        m.z_axis
    );

    // A stopped round is no draw and no panic (no direction to point).
    walk.sim.bolts[0].vel = Vec3::ZERO;
    assert!(missiles_in_flight(&walk).is_empty());

    // The same round with no model: the look this fixture authored gets
    // the ribbon path (nothing is silently dropped).
    walk.sim.bolts[0].vel = Vec3::NEG_Z * 40.0;
    walk.sim.weapon_classes[class].ordnance.geometry = None;
    let mut vis = BoltVis::default();
    vis.update(&walk, core::TICK_DT);
    assert_eq!(
        vis.tracked.len(),
        1,
        "an authored look is the fallback ribbon"
    );
    assert_eq!(vis.tracked[0].class, class);
}

#[test]
fn a_beam_draws_from_its_origin_to_what_it_struck() {
    let look = core::laser::LaserLook {
        width: 0.1,
        ..Default::default()
    };
    let beam = core::sim::Beam {
        shooter: core::projectile::Shooter {
            owner: core::unit::UnitHandle { index: 0, gen: 0 },
            team: 0,
            weapon_class: 0,
            is_player: true,
        },
        hulls: core::collision::HullFilter::All,
        charge: 1.0,
        origin: Vec3::new(0.0, 1.0, 0.0),
        dir: Vec3::X,
        life: 0.1,
        len: 40.0,
        follow: true,
    };
    let d = beam_draw(&look, &beam, [None, None]);
    assert_eq!(d.head, Vec3::new(40.5, 1.0, 0.0));
    assert_eq!(d.dir, Vec3::X);
    assert_eq!((d.core_len, d.glow_len), (41.0, 41.0));
    assert!((d.width - 0.1).abs() < 1e-7);
}
