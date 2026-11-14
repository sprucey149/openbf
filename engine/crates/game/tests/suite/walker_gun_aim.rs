//! Playtest 2026-11-13, BF2 Geonosis: the AT-TE's front guns are "deadly
//! precise in the original" and miss the reticle in the port.
//!
//! A driven walker's aim ray starts at its chase camera carried along the aim
//! to the eye's depth (B2 FUN_00261208, as the hover's), so what the reticle
//! covers is what the guns hit. The port cast the walker pilot's ray from the
//! gun's eye, off the drawn camera, and the guns met the ground where that
//! ray did, off the reticle and short of it.

use super::seat_camera::{map, missions_of};
use core::sim::{buttons, Event, PadState};
use core::vehicle::VehicleSpec;
use glam::Vec3;

fn terrain_hit(sim: &core::sim::Sim, from: Vec3, dir: Vec3) -> Option<Vec3> {
    let hf = sim.heightfield.as_ref()?;
    (1..6000)
        .map(|k| from + dir * (k as f32 * 0.05))
        .find(|p| p.y < hf.height_at(p.x, p.z))
}

/// The pilot's drawn camera stands on his aim ray, and his front guns'
/// bolts meet where that ray meets the ground. Mutation: the walker back on
/// the gun's eye (`walker_chase` empty) -> the camera is metres off the ray
/// and the bolts miss the reticle's ground.
#[test]
fn an_atte_pilots_bolts_meet_what_his_reticle_covers() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let Some(mission) = missions_of("bf2_geo1").into_iter().next() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let Some(mut m) = map(&dir, &mission.factions()) else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")
        .expect("an AT-TE pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 0));
    let mut checked = 0;
    for (yaw, pitch) in [(0.0f32, -0.12f32), (0.25, -0.18), (-0.3, -0.1)] {
        let w = &mut m.walk.sim.vehicle_mut(h).unwrap().walker;
        (w.aim_yaw, w.aim_pitch) = (yaw, pitch);
        let pad = |fire: bool| PadState {
            buttons: if fire { buttons::FIRE } else { 0 },
            ..Default::default()
        };
        for _ in 0..60 {
            m.walk.sim.step(&[pad(false)]);
        }
        let look = m.walk.sim.seat_look(player).expect("the pilot's gun");
        let cam = m.walk.camera();
        let off = cam.position - look.eye;
        let off = off - look.view * off.dot(look.view);
        assert!(
            off.length() < 0.05,
            "aim ({yaw}, {pitch}): the camera is {:.3} m off the aim ray",
            off.length()
        );
        let target = terrain_hit(&m.walk.sim, look.eye, look.view).expect("the ray meets ground");
        let mut shots = Vec::new();
        for _ in 0..20 {
            m.walk.sim.step(&[pad(true)]);
            for e in &m.walk.sim.events {
                if let Event::Shot {
                    unit, origin, dir, ..
                } = e
                {
                    if *unit == player {
                        shots.push((*origin, dir.normalize()));
                    }
                }
            }
        }
        for _ in 0..30 {
            m.walk.sim.step(&[pad(false)]);
        }
        assert!(
            shots.len() >= 2,
            "aim ({yaw}, {pitch}): {} shots",
            shots.len()
        );
        for (o, d) in shots {
            let miss = (target - o) - d * (target - o).dot(d);
            assert!(
                miss.length() < 0.25,
                "aim ({yaw}, {pitch}): a bolt from {o} misses the reticle's ground {target} by {:.2} m",
                miss.length()
            );
            checked += 1;
        }
    }
    assert!(checked >= 6);
}

/// Playtest 2026-11-13, BF2 Geonosis: under the AT-TE "the vehicle becomes
/// transparent, in the original the camera moves in". A trooper with his
/// back 1.5 m from the AT-TE's hull: on BF2 the hull pulls his camera in
/// short of it and nothing is seen through; on BF1 the camera stays back
/// and the AT-TE is seen through, as before. Mutation: `vehicles_pull`
/// false -> the BF2 camera stands behind the hull, the AT-TE dithered.
#[test]
fn a_bf2_vehicle_pulls_the_soldiers_camera_in() {
    let mut ran = 0;
    for key in ["bf2_geo1", "bf1_geo1"] {
        let dir = game::assets::baked_root().join(key);
        let Some(mission) = missions_of(key).into_iter().next() else {
            continue;
        };
        let Some(mut m) = map(&dir, &mission.factions()) else {
            continue;
        };
        let spec = m
            .build
            .pads
            .iter()
            .flat_map(|p| {
                p.classes.iter().flatten().map(|c| VehicleSpec {
                    pos: p.pos,
                    yaw: p.yaw,
                    ..c.spec.clone()
                })
            })
            .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")
            .expect("an AT-TE pad");
        let h = m.walk.sim.spawn_vehicle(&spec);
        for _ in 0..5 {
            m.walk.sim.step(&[PadState::default()]);
        }
        let v = m.walk.sim.vehicle(h).unwrap().clone();
        let fwd = (v.rot * Vec3::Z).with_y(0.0).normalize();
        let mut at = v.pos + fwd * 25.0;
        at.y = m.walk.sim.spawn_ground(at);
        let probe = at + Vec3::Y * 4.0;
        let front = m
            .walk
            .sim
            .collision
            .ray_with(
                probe,
                -fwd,
                40.0,
                core::collision::Layers::ORDNANCE,
                core::collision::HullFilter::Only(h),
            )
            .expect("the AT-TE's hull ahead")
            .t;
        let mut at = at - fwd * (front + 2.0);
        at.y = m.walk.sim.spawn_ground(at);
        let player = m.walk.player;
        let u = m.walk.sim.unit_mut(player).unwrap();
        u.soldier.pos = at;
        u.soldier.yaw = (-fwd.x).atan2(-fwd.z) + std::f32::consts::PI;
        u.soldier.pitch = -0.6;
        for _ in 0..3 {
            m.walk.sim.step(&[PadState::default()]);
        }
        let cam = m.walk.camera();
        let pull = m.walk.sim.unit(player).unwrap().soldier.pos + Vec3::Y * 1.5;
        let track = cam.position - pull;
        let crossed = m
            .walk
            .sim
            .collision
            .ray_with(
                pull,
                track.normalize(),
                track.length(),
                core::collision::Layers::ORDNANCE,
                core::collision::HullFilter::Only(h),
            )
            .is_some();
        let seen = m.walk.camera_see_through().vehicles;
        eprintln!(
            "{key}: camera {:.2} m from the pull point, past the hull {crossed}, see-through {seen:?}",
            track.length()
        );
        if key.starts_with("bf2") {
            assert!(!crossed, "{key}: the camera stands past the hull");
            assert!(seen.is_empty(), "{key}: {seen:?} seen through");
        } else {
            assert!(crossed, "{key}: the camera did not go past the hull");
            assert!(
                seen.iter().any(|&(s, _)| s == h),
                "{key}: the AT-TE is not seen through"
            );
        }
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: no Geonosis bake");
    }
}

/// Playtest 2026-11-13, BF2 Geonosis: the AT-TE's gatling gun (the rear
/// gunner's chaingun) is "deadly precise in the original", its reticle
/// centred; in the port the bolts met the ground well off it. A gunner's aim
/// ray starts at his turret's camera carried to the eye's depth (B2
/// FUN_0026cea0, `ChaseSection::gunner_ray_origin`), so where the bolts
/// land is where the reticle is drawn. Each barrel turns on its own aimer
/// (`aimer_Rgun_1` -35..110 deg, `aimer_Rgun_2` -110..35): past 35 deg to a
/// side the inner barrel stops at its limit and holds its fire, the outer
/// one alone firing (playtest 2026-11-13 on the PS2: "the weapon that can't
/// hit the target just stops shooting"). Mutation: `seat_chase` empty -> the
/// rear gun's bolts land 0.09 of the screen above the reticle and 0.02 right
/// of it, the camera 3.1 m off the ray; both barrels on the first aimer's
/// limits (playtest 2026-11-13, "what the hell have you done") -> turned 60
/// deg to one side no bolt lands under the reticle; `ring_reach` `None` ->
/// turned 60 deg the inner barrel fires too, off the reticle. The tick the
/// spin ends fires two rounds (the trigger's salvo and a full refire,
/// B2:311243-311290), the second with the first's spread grown, so a round
/// or two of a burst land a little off it.
#[test]
fn an_atte_gunners_bolts_land_under_his_reticle() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let Some(mission) = missions_of("bf2_geo1").into_iter().next() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let Some(mut m) = map(&dir, &mission.factions()) else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")
        .expect("an AT-TE pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    let pad = |fire: bool| PadState {
        buttons: if fire { buttons::FIRE } else { 0 },
        ..Default::default()
    };
    assert!(m.walk.sim.seat_unit(player, h, 2));
    for _ in 0..5 {
        m.walk.sim.step(&[pad(false)]);
    }
    let class = m.walk.sim.vehicle(h).unwrap().class as usize;
    let gun = m.walk.sim.seat_setup[class].as_ref().unwrap().guns[2]
        .as_ref()
        .unwrap()
        .weapons[0]
        .class;
    let pi = std::f32::consts::PI;
    let mut checked = 0;
    for look in [
        (pi, 0.0f32),
        (pi + 0.3, -0.12),
        (pi - 0.35, -0.08),
        (pi + 1.0, -0.15),
        (pi - 1.0, -0.15),
    ] {
        m.walk.sim.vehicle_mut(h).unwrap().guns[2]
            .as_mut()
            .unwrap()
            .look = look;
        for _ in 0..60 {
            m.walk.sim.step(&[pad(false)]);
        }
        let l = m.walk.sim.seat_look(player).expect("the gunner's gun");
        let cam = m.walk.camera();
        let off = cam.position - l.eye;
        let off = off - l.view * off.dot(l.view);
        assert!(
            off.length() < 0.05,
            "look {look:?}: the camera is {:.3} m off the aim ray",
            off.length()
        );
        let reticle = m.walk.reticle_y();
        assert!(
            (reticle - 0.5).abs() < 0.01,
            "look {look:?}: the reticle at {reticle}"
        );
        let mut shots = Vec::new();
        let mut barrels = std::collections::BTreeSet::new();
        for _ in 0..31 {
            m.walk.sim.step(&[pad(true)]);
            for (k, e) in m.walk.sim.events.iter().enumerate() {
                if let Event::Shot {
                    unit, origin, dir, ..
                } = e
                {
                    if *unit == player {
                        shots.push((*origin, dir.normalize()));
                        barrels.insert(game::walker_shots::turn(&m.walk, player, gun, k) % 2);
                    }
                }
            }
        }
        let cam = m.walk.camera();
        for _ in 0..60 {
            m.walk.sim.step(&[pad(false)]);
        }
        assert!(shots.len() >= 4, "look {look:?}: {} shots", shots.len());
        assert_eq!(
            barrels.len(),
            if (look.0 - pi).abs() < 0.4 { 2 } else { 1 },
            "look {look:?}: barrels {barrels:?} fired"
        );
        let (mut under, mut off) = (0, Vec::new());
        for &(o, d) in &shots {
            let screen = terrain_hit(&m.walk.sim, o, d).map(|hit| {
                let clip = cam.view_proj(4.0 / 3.0) * hit.extend(1.0);
                (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w)
            });
            match screen {
                Some(s) if (s.0 - 0.5).abs() < 0.01 && (s.1 - reticle).abs() < 0.01 => under += 1,
                s => off.push(s),
            }
        }
        let wide = off
            .iter()
            .any(|s| s.is_none_or(|s| (s.0 - 0.5).abs() > 0.05 || (s.1 - reticle).abs() > 0.05));
        assert!(
            under + 2 >= shots.len() && !wide,
            "look {look:?}: {under} of {} bolts under the reticle, the rest at {off:?}, the reticle at (0.5, {reticle})",
            shots.len()
        );
        checked += under;
    }
    assert!(checked >= 6, "{checked} bolts met the ground");
}

/// Playtest 2026-11-13 on the PS2: the AT-TE's gatling gun "has a delay
/// before it starts shooting, ours doesn't". Its class (`com_weap_veh_chaingun`)
/// spins up 0.35 s before the first round (class +0x2ac, B2 `FUN_0032e758` /
/// `FUN_0032e8c0`). The front guns, which author no spin, fire at the press.
/// Mutation: `wind_up` never read -> the rear gun's first round leaves on
/// the press's tick.
#[test]
fn the_atte_gatling_gun_spins_up_before_its_first_round() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let Some(mission) = missions_of("bf2_geo1").into_iter().next() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let Some(mut m) = map(&dir, &mission.factions()) else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")
        .expect("an AT-TE pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    let pad = |fire: bool| PadState {
        buttons: if fire { buttons::FIRE } else { 0 },
        ..Default::default()
    };
    let mut first = Vec::new();
    for seat in [2u8, 0] {
        m.walk.sim.unseat_unit(player);
        assert!(m.walk.sim.seat_unit(player, h, seat));
        for _ in 0..90 {
            m.walk.sim.step(&[pad(false)]);
        }
        let tick = (1..=60u32).find(|_| {
            m.walk.sim.step(&[pad(true)]);
            m.walk
                .sim
                .events
                .iter()
                .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == player))
        });
        for _ in 0..90 {
            m.walk.sim.step(&[pad(false)]);
        }
        first.push((seat, tick));
    }
    let dt = core::TICK_DT;
    let rear = first[0].1.expect("the rear gun fires") as f32 * dt;
    assert!(
        (0.35..0.35 + 2.0 * dt).contains(&rear),
        "the rear gun's first round after {rear:.3} s, {first:?}"
    );
    assert_eq!(first[1].1, Some(1), "the front guns: {first:?}");
}

/// Playtest 2026-11-13 on the PS2: the AT-TE's gatling gun draws its
/// streaks "one by one", each where its round went, and lights the ground;
/// the port drew the same two streaks every third round and no light. Each
/// round is the primary ordnance's own bolt (`com_weap_veh_chaingun_ord`,
/// LightRadius 3, FadeOutTime 0.25); the tracer is its secondary beam.
/// Mutation: the walker shot style hiding regular rounds -> no line and no
/// light on a firing tick.
#[test]
fn every_atte_gatling_round_draws_its_own_lit_bolt() {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let Some(mission) = missions_of("bf2_geo1").into_iter().next() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let Some(mut m) = map(&dir, &mission.factions()) else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    m.walk.walker_shot_styles = m.build.walker_shot_styles.clone();
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")
        .expect("an AT-TE pad");
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 2));
    let pad = |fire: bool| PadState {
        buttons: if fire { buttons::FIRE } else { 0 },
        ..Default::default()
    };
    for _ in 0..90 {
        m.walk.sim.step(&[pad(false)]);
    }
    let mut vis = game::bolt_vis::BoltVis::new(&root, "bf2");
    let (mut rounds, mut firing) = (0, 0);
    for _ in 0..60 {
        m.walk.sim.step(&[pad(true)]);
        vis.update(&m.walk, core::TICK_DT);
        let shots = m
            .walk
            .sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == player))
            .count();
        if shots == 0 {
            continue;
        }
        rounds += shots;
        firing += 1;
        let (lines, lights) = (vis.lines().len(), vis.omni_lights(&m.walk).len());
        assert!(
            lines >= shots && lights >= shots,
            "{shots} rounds drew {lines} lines and {lights} lights"
        );
    }
    assert!(
        rounds >= 10 && firing >= 5,
        "{rounds} rounds on {firing} ticks"
    );
}

/// The AT-TE's rear gun on Geonosis, seated with the real shot styles and
/// stepped `ticks` holding `fire` at turret look `look`.
fn atte_rear_gun() -> Option<(game::walk::WalkSim, core::vehicle::VehicleHandle, usize)> {
    let root = game::assets::baked_root();
    let dir = root.join("bf2_geo1");
    let mission = missions_of("bf2_geo1").into_iter().next()?;
    let mut m = map(&dir, &mission.factions())?;
    m.walk.walker_shot_styles = m.build.walker_shot_styles.clone();
    let spec = m
        .build
        .pads
        .iter()
        .flat_map(|p| {
            p.classes.iter().flatten().map(|c| VehicleSpec {
                pos: p.pos,
                yaw: p.yaw,
                ..c.spec.clone()
            })
        })
        .find(|s| m.build.classes[s.class as usize] == "rep_walk_atte")?;
    let h = m.walk.sim.spawn_vehicle(&spec);
    let player = m.walk.player;
    assert!(m.walk.sim.seat_unit(player, h, 2));
    for _ in 0..5 {
        m.walk.sim.step(&[PadState::default()]);
    }
    let class = m.walk.sim.vehicle(h)?.class as usize;
    let gun = m.walk.sim.seat_setup[class].as_ref()?.guns[2]
        .as_ref()?
        .weapons[0]
        .class;
    Some((m.walk, h, gun))
}

/// Playtest 2026-11-13: turned to one side the AT-TE's gatling gun "is
/// shooting from the left weapon at least visually even when aiming
/// completely to the right". Each barrel turns on its own aimer
/// (`aimer_Rgun_1`, `aimer_Rgun_2`, `analysis/re-turret-draw.md` 2.3), so a
/// round, its tracer beam and the drawn barrel all leave from the same
/// place. The port turned both fire points about the second barrel's: turned
/// 1 rad the open barrel's rounds left 2.9 m from it, from beside the other
/// barrel. Mutation: `fire_pivots` empty -> 2.9 m.
#[test]
fn an_atte_gatling_round_leaves_from_the_barrel_drawn_firing_it() {
    let Some((mut w, h, gun)) = atte_rear_gun() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let player = w.player;
    let pi = std::f32::consts::PI;
    let mut checked = 0;
    for look in [(pi, 0.0f32), (pi + 1.0, -0.1), (pi - 1.0, -0.1)] {
        w.sim.vehicle_mut(h).unwrap().guns[2].as_mut().unwrap().look = look;
        for _ in 0..60 {
            w.sim.step(&[PadState::default()]);
        }
        for _ in 0..30 {
            w.sim.step(&[PadState {
                buttons: buttons::FIRE,
                ..Default::default()
            }]);
            for (k, e) in w.sim.events.iter().enumerate() {
                let Event::Shot { unit, origin, .. } = e else {
                    continue;
                };
                if *unit != player {
                    continue;
                }
                let turn = game::walker_shots::turn(&w, player, gun, k);
                let drawn = game::walker_shots::frame(&w, player, gun, turn)
                    .unwrap()
                    .w_axis
                    .truncate();
                let off = origin.distance(drawn);
                assert!(
                    off < 0.6,
                    "look {look:?}: round {k} left {off:.2} m from its drawn barrel"
                );
                checked += 1;
            }
        }
        for _ in 0..60 {
            w.sim.step(&[PadState::default()]);
        }
    }
    assert!(checked >= 20, "{checked} rounds");
}

/// Playtest 2026-11-13: "if you stop shooting for a moment, and then start
/// pressing, it continues shooting (or at least plays the sound) yet we don't
/// see a shooting effect". A press inside the 1.25 s wind-down waits it out
/// and winds up again (B2 `FUN_0032ef58`; the PS2 from slot 9 fired again
/// 84-96 frames after a press 12 frames into it). Through that wait the PS2
/// plays the spin motor, its fire loop fading out (B2 `FUN_0032ffd8`) and
/// starting again only with the rounds (`FUN_0032fec8`). Mutation: the fire
/// loop on a held trigger -> it rises with no round leaving.
#[test]
fn a_pressed_atte_gatling_gun_plays_its_fire_loop_only_with_its_rounds() {
    use game::walker_shots::Ramp;
    let Some((mut w, _, gun)) = atte_rear_gun() else {
        eprintln!("SKIP: bf2_geo1 not baked");
        return;
    };
    let player = w.player;
    let pad = |fire: bool| PadState {
        buttons: if fire { buttons::FIRE } else { 0 },
        ..Default::default()
    };
    let (mut quiet, mut loud) = (0, 0);
    let mut fired = false;
    for (fire, n) in [(true, 40), (false, 6), (true, 70)] {
        for _ in 0..n {
            w.sim.step(&[pad(fire)]);
            let shot = w
                .sim
                .events
                .iter()
                .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == player));
            fired |= shot;
            let voices = game::walker_shots::voices(&w);
            let rising = voices
                .iter()
                .any(|v| v.class == gun && !v.spin && matches!(v.ramp, Ramp::Rise(_)));
            let spin = voices.iter().any(|v| v.class == gun && v.spin);
            let st = &w
                .sim
                .vehicle(w.sim.unit(player).unwrap().mount.unwrap().vehicle)
                .unwrap()
                .guns[2]
                .as_ref()
                .unwrap()
                .weapons[0];
            let rounds_due =
                st.state == core::weapon::FireState::Firing && !st.spin_down && st.spin <= 0.0;
            assert_eq!(
                rising, rounds_due,
                "fire loop {rising}, spin {} down {} {:?}",
                st.spin, st.spin_down, st.state
            );
            if fire && st.state == core::weapon::FireState::Firing && !rounds_due {
                assert!(spin, "the spin motor through the wait");
                quiet += 1;
            }
            loud += usize::from(rising && shot);
        }
    }
    assert!(
        fired && quiet > 30 && loud > 10,
        "quiet {quiet}, loud {loud}"
    );
}

/// Every map's building turrets and each pad class's gunner seats on a
/// walker, a flyer or a hover: a player gunner's drawn camera stands on his
/// aim ray, nearer it than to the eye's own ray the port cast before. Before:
/// up to 9 m off (Bespin's anti-aircraft turret); now 0 m, except the walker
/// seats whose camera rides the posed turret node while the ray rides the
/// gun's pivot (the AT-AT's chin guns 0.30 m, the AT-TE's cannon 0.08 m).
/// Mutation: `seat_chase` empty -> every seat its old metres off.
#[test]
fn every_gunners_camera_stands_on_his_aim_ray() {
    use core::vehicle::{VehicleHandle, VehicleKind};
    let maps = game::assets::discover_maps(&game::assets::baked_root());
    if maps.is_empty() {
        eprintln!("SKIP: no baked maps");
        return;
    }
    let (mut rows, mut wrong) = (0, Vec::new());
    for (key, _, dir) in &maps {
        let Some(mission) = missions_of(key).into_iter().next() else {
            continue;
        };
        let Some(mut m) = map(dir, &mission.factions()) else {
            continue;
        };
        let mut hs: Vec<VehicleHandle> = m
            .walk
            .sim
            .alive_vehicles()
            .filter(|(_, v)| v.kind == VehicleKind::Turret)
            .map(|(h, _)| h)
            .collect();
        let specs: Vec<_> = m
            .build
            .pads
            .iter()
            .flat_map(|p| {
                p.classes.iter().flatten().map(|c| VehicleSpec {
                    pos: p.pos,
                    yaw: p.yaw,
                    ..c.spec.clone()
                })
            })
            .collect();
        let mut classes = std::collections::BTreeSet::new();
        for s in &specs {
            if classes.insert(s.class) {
                hs.push(m.walk.sim.spawn_vehicle(s));
            }
        }
        let mut done = std::collections::BTreeSet::new();
        let player = m.walk.player;
        for h in hs {
            let Some(v) = m.walk.sim.vehicle(h).cloned() else {
                continue;
            };
            let class = m.build.classes[v.class as usize].clone();
            for seat in 0..v.seats.len() as u8 {
                let reached = v.kind == VehicleKind::Turret
                    || (seat != 0
                        && matches!(
                            v.kind,
                            VehicleKind::Walker | VehicleKind::Flyer | VehicleKind::Hover
                        ));
                if !reached || !done.insert((class.clone(), seat)) {
                    continue;
                }
                m.walk.sim.unseat_unit(player);
                if !m.walk.sim.seat_unit(player, h, seat) {
                    continue;
                }
                if v.kind == VehicleKind::Flyer {
                    m.walk.sim.vehicle_mut(h).unwrap().pos.y += 80.0;
                }
                for _ in 0..8 {
                    m.walk.sim.step(&[PadState::default()]);
                }
                let (Some(l), Some(v)) =
                    (m.walk.sim.seat_look(player), m.walk.sim.vehicle(h).cloned())
                else {
                    continue;
                };
                let cam = m.walk.camera().position;
                let off = |e: Vec3| {
                    let o = cam - e;
                    (o - l.view * o.dot(l.view)).length()
                };
                let old = off(v.pos + v.rot * l.gun.ray_eye(l.look, false));
                let new = off(l.eye);
                rows += 1;
                if new > 0.35 || (old > 0.05 && new >= old) {
                    wrong.push(format!(
                        "{key} {class} seat {seat}: the camera {new:.3} m off the aim ray (the eye's ray {old:.3})"
                    ));
                }
            }
        }
    }
    println!("{rows} gunner seats");
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(rows >= 40, "too few seats: {rows}");
}
