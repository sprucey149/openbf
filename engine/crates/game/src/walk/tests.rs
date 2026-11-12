use super::*;
use core::sim::buttons;

/// Both games take BF2's sections (ours, playtest 2026-10-28): 3.2 m
/// standing, 4.0 m sprinting.
#[test]
fn sprint_camera_eases_in_and_out_in_both_games() {
    use core::soldier::GroundRules;
    for (rules, standing, sprinting) in [(GroundRules::Bf1, 3.2, 4.0), (GroundRules::Bf2, 3.2, 4.0)]
    {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        w.sim.ground_rules = rules;
        let pad = |buttons| PadState {
            ly: 32767,
            buttons,
            ..Default::default()
        };
        let distance = |w: &WalkSim| {
            let me = &w.sim.units[w.player.index as usize];
            let ideal = me.ideal_camera(w.sim.ground_rules, w.sim.views());
            (w.camera().position - ideal.center).length()
        };
        run_ticks(&mut w, pad(0), 30);
        assert!((distance(&w) - standing).abs() < 1e-4);
        let mut entered = false;
        for _ in 0..30 {
            w.step_tick(&[pad(buttons::SPRINT)]);
            if w.soldier().stance == Stance::Sprint {
                entered = true;
                break;
            }
        }
        assert!(entered, "{rules:?}: sprint did not start");
        let k = core::fmath::exp(-15.0 * core::TICK_DT);
        let first = distance(&w);
        assert!(
            (first - (sprinting + (standing - sprinting) * k)).abs() < 1e-3,
            "{rules:?}: first sprint camera distance {first}"
        );
        run_ticks(&mut w, pad(buttons::SPRINT), 30);
        assert!((distance(&w) - sprinting).abs() < 1e-4);
        w.step_tick(&[pad(0)]);
        assert_ne!(w.soldier().stance, Stance::Sprint);
        let first = distance(&w);
        assert!(
            (first - (standing + (sprinting - standing) * k)).abs() < 1e-3,
            "{rules:?}: first walking camera distance {first}"
        );
        run_ticks(&mut w, pad(0), 30);
        assert!((distance(&w) - standing).abs() < 1e-4);
    }
}

/// A jet's camera lag is the PS2's ease: it settles at the measured 0.0514 s of his
/// velocity (`analysis/re-jetpack.md` §16) and ending the jet at speed does not jump.
#[test]
fn jet_camera_settles_at_the_measured_lag_and_never_jumps() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    w.sim.ground_rules = core::soldier::GroundRules::Bf2;
    run_ticks(&mut w, PadState::default(), 30);
    let i = w.player.index as usize;
    let vel = Vec3::new(10.5, 0.0, 0.0);
    let offset = |w: &WalkSim| {
        let me = &w.sim.units[i];
        let ideal = me.ideal_camera(w.sim.ground_rules, w.sim.views());
        ideal.position - w.camera().position
    };
    assert!(offset(&w).length() < 1e-4, "no lag on foot");
    let mut last = offset(&w);
    for _ in 0..30 {
        w.sim.units[i].soldier.stance = Stance::Hover;
        w.sim.units[i].soldier.vel = vel;
        w.cam_jet_lag = w.next_cam_jet_lag();
        assert!((offset(&w) - last).length() < 0.25, "the camera jumped");
        last = offset(&w);
    }
    assert!((last.x - 0.54).abs() < 0.01, "settled at {last}");
    w.sim.units[i].soldier.stance = Stance::Fall;
    for _ in 0..30 {
        w.cam_jet_lag = w.next_cam_jet_lag();
        assert!((offset(&w) - last).length() < 0.25, "the camera jumped");
        last = offset(&w);
    }
    assert!(last.length() < 1e-3, "the lag dies away, left {last}");
}

/// The five value rows, and the "everything else is Locals" rule that the
/// built-in trooper (faction "team") falls into.
#[test]
fn faction_rows_cover_the_four_mains_and_default_to_locals() {
    assert_eq!(super::faction_row("all"), 0);
    assert_eq!(super::faction_row("imp"), 1);
    assert_eq!(super::faction_row("rep"), 2);
    assert_eq!(super::faction_row("cis"), 3);
    assert_eq!(super::faction_row("CIS"), 3, "case-insensitive");
    assert_eq!(super::faction_row("alliance"), 0);
    assert_eq!(super::faction_row("team"), 4, "the built-in trooper");
    assert_eq!(super::faction_row("wookiee"), 4);
}

/// The PS2's measured vertical FOVs (FINDINGS F5); the chase camera takes
/// BF2's on every map (ours, playtest 2026-10-28), and the option scales it
/// within its range and cap.
#[test]
fn chase_camera_takes_bf2s_vertical_fov_on_every_map() {
    use core::soldier::GroundRules;
    let deg = |r: f32| r.to_degrees();
    assert!((deg(ps2_fov_y("bf1")) - 37.69).abs() < 0.01);
    assert!((deg(ps2_fov_y("BF2")) - 52.06).abs() < 0.01);
    assert_eq!(chase_fov_y(), ps2_fov_y("bf2"));
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        w.sim.ground_rules = rules;
        assert_eq!(w.camera().fov_y, chase_fov_y());
        w.fov_scale = 1.5;
        assert!((w.camera().fov_y - 1.5 * chase_fov_y()).abs() < 1e-6);
        w.fov_scale = 9.0;
        assert_eq!(w.camera().fov_y, MAX_FOV_Y);
        w.fov_scale = 0.0;
        assert!((w.camera().fov_y - FOV_SCALE_RANGE.0 * chase_fov_y()).abs() < 1e-6);
    }
}

/// Card ZOOM acceptance 4, the camera's half (`analysis/re-zoom.md` §5):
/// S after n ticks of a zoom z is z + (1 - z) exp(-25 n dt) - the PS2's
/// rows 1.848, 2.217, 2.377 for the rifle - the settled vertical FOV is
/// BF2's 24.24 deg on every map (the base FOV eased 65 -> 70 deg; the PS2's
/// BF1 settles at 15.55 from 48 deg, ours plays BF2's camera there too), the
/// camera stands at the ZOOM section, the reticle stays on the aim ray, and
/// the zoom out comes back to the plain FOV exactly.
#[test]
fn the_zoom_eases_the_projection_as_the_ps2_does() {
    use core::sim::buttons;
    use core::soldier::GroundRules;
    let pad = |buttons| PadState {
        buttons,
        ..Default::default()
    };
    for (game, rules, settled, first) in [
        ("bf1", GroundRules::Bf1, 24.24, 31.16),
        ("bf2", GroundRules::Bf2, 24.24, 31.16),
    ] {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        w.sim.ground_rules = rules;
        let plain = w.camera();
        w.step_tick(&[pad(buttons::ZOOM)]);
        let cam = w.camera();
        // The section is taken at once: 0.4 m to the right, 3.5 deg
        // under the aim.
        let me = &w.sim.units[w.player.index as usize];
        assert!(((cam.position - me.soldier.pos).dot(cam.right()) - 0.4).abs() < 1e-3);
        assert!(((me.soldier.pitch - cam.pitch).to_degrees() - 3.5).abs() < 0.01);
        let mut rows = vec![w.zoom_s];
        let first_fov = cam.fov_y.to_degrees();
        for n in 2..=40 {
            w.step_tick(&[pad(0)]);
            if n <= 4 {
                rows.push(w.zoom_s);
            }
        }
        let fov = w.camera().fov_y.to_degrees();
        println!(
            "{game}: plain {:.2} deg, S {rows:.4?}, first zoomed tick {first_fov:.2} deg, settled {fov:.2} deg, reticle line {:.1} of 480",
            plain.fov_y.to_degrees(),
            w.reticle_y() * 480.0
        );
        for (i, s) in rows.iter().enumerate() {
            let n = (i + 1) as f32;
            let want = 2.5 + (1.0 - 2.5) * (-25.0 * n * core::TICK_DT).exp();
            assert!(
                (s - want).abs() < 1e-4,
                "{game} tick {n}: S {s} want {want}"
            );
        }
        // The PS2's own rows (BF2's 1 / 30 s update; BF1's 33.36 ms
        // clock reads 1.8486, 2.2171, 2.3772).
        for (s, ps2) in rows.iter().zip([1.8481, 2.2167]) {
            assert!((s - ps2).abs() < 2e-3);
        }
        assert_eq!(w.zoom_s, 2.5, "S rests at the zoom");
        assert!((first_fov - first).abs() < 0.02, "{game}: {first_fov}");
        assert!((fov - settled).abs() < 0.01, "{game}: {fov}");
        // The reticle is where the aim ray goes (§5 [D]: tan(3.5 deg) x
        // S / tan of the half height above the centre).
        let t = (w.camera().fov_y * 0.5).tan();
        let want = 0.5 - 0.5 * 3.5f32.to_radians().tan() / t;
        assert!((w.reticle_y() - want).abs() < 2e-3);
        // Out again: the plain camera, to the bit.
        w.step_tick(&[pad(buttons::ZOOM)]);
        for _ in 0..40 {
            w.step_tick(&[pad(0)]);
        }
        assert_eq!(w.camera().fov_y, plain.fov_y);
        assert_eq!(w.camera().position, plain.position);
    }
}

/// Card AIMA: the chase camera looks the section's tilt below the aim
/// and the reticle sits where the aim ray goes on screen, at every pitch; a
/// wider FOV option moves it toward the centre. The PS2 draws it on line
/// 178.5 of 480 on a BF1 map and 153.35 on a BF2 map (`re-aim-assist.md`
/// §10.4); the port plays BF2's camera on every map, so 153.35 on both.
#[test]
fn the_reticle_sits_where_the_aim_ray_goes_on_screen() {
    use core::soldier::GroundRules;
    for (game, rules, ps2_line) in [
        ("bf1", GroundRules::Bf1, 178.5),
        ("bf2", GroundRules::Bf2, 153.35),
    ] {
        assert!((ps2_reticle_y(game) * 480.0 - ps2_line).abs() < 0.5);
        let (line, cam_deg) = (153.35, -14.011);
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        w.sim.ground_rules = rules;
        let cam = w.camera();
        let y = w.reticle_y() * 480.0;
        println!(
            "{game}: camera pitch {:.3} deg, {:.3} m up, reticle line {y:.2} of 480",
            cam.pitch.to_degrees(),
            cam.position.y
        );
        assert!((cam.pitch.to_degrees() - cam_deg).abs() < 0.01);
        assert!((y - line).abs() < 0.5, "{game}: line {y}");
        for pitch in [-0.3, 0.2, 0.6] {
            w.sim.units[w.player.index as usize].soldier.pitch = pitch;
            let y = w.reticle_y() * 480.0;
            assert!((y - line).abs() < 0.5, "{game} pitch {pitch}: line {y}");
        }
        w.fov_scale = 1.3;
        assert!(w.reticle_y() * 480.0 > line + 5.0);
    }
}

/// Playtest 2026-10-28: "the bf1 default camera seems to be cutting off the
/// players units feet [...] All maps should appear the same way". A BF1 and
/// a BF2 map frame the soldier alike: for every stance, zoomed or not, the
/// same camera offset from his feet, view and FOV - BF2's (3.2 m back and
/// 10 deg under the aim standing, 52.06 deg). Standing on flat ground with
/// the spawn pitch, his feet are inside the frame's bottom edge (the PS2's
/// BF1 camera, 3.5 m back, 5 deg, 37.69 deg, left them out by 6 deg).
#[test]
fn bf1_and_bf2_maps_frame_the_soldier_alike() {
    use core::soldier::{GroundRules, Stance};
    let frame = |rules, stance| {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        w.sim.ground_rules = rules;
        let me = &mut w.sim.units[w.player.index as usize];
        me.soldier.stance = stance;
        me.soldier.pitch = core::unit::SPAWN_PITCH;
        let sec = me.section();
        assert_eq!(sec, core::unit::camera_section(stance, false));
        let cam = w.camera();
        (
            cam.position - w.soldier().pos,
            cam.pitch,
            cam.yaw,
            cam.fov_y,
        )
    };
    for stance in [Stance::Stand, Stance::Crouch, Stance::Prone, Stance::Sprint] {
        let (a, b) = (
            frame(GroundRules::Bf1, stance),
            frame(GroundRules::Bf2, stance),
        );
        assert_eq!(a, b, "{stance:?}: BF1 {a:?} BF2 {b:?}");
        assert_eq!(
            core::unit::camera_section(stance, false),
            core::unit::ps2_camera_section(GroundRules::Bf2, stance, false)
        );
        assert_eq!(
            core::unit::camera_section(stance, true),
            core::unit::ps2_camera_section(GroundRules::Bf2, stance, true)
        );
    }
    let (off, pitch, _, fov) = frame(GroundRules::Bf1, Stance::Stand);
    assert!((fov.to_degrees() - 52.06).abs() < 0.01);
    // The feet's angle under the view axis against the frame's half height.
    let feet_down = off.y.atan2(off.with_y(0.0).length()) + pitch;
    assert!(
        feet_down < fov * 0.5,
        "feet {:.2} deg under the axis, frame half {:.2}",
        feet_down.to_degrees(),
        (fov * 0.5).to_degrees()
    );
    // The PS2's BF1 numbers leave them out.
    let ps2 = core::unit::ps2_camera_section(GroundRules::Bf1, Stance::Stand, false);
    let aim_down = -core::unit::SPAWN_PITCH + ps2.tilt;
    let (h, back) = (
        ps2.eye_y + ps2.offset.z * aim_down.sin(),
        ps2.offset.z * aim_down.cos(),
    );
    let bf1_feet = h.atan2(back) - aim_down;
    assert!(
        bf1_feet > ps2_fov_y("bf1") * 0.5,
        "{}",
        bf1_feet.to_degrees()
    );
}

/// Flat ground at y = 0 over ±2 km. A sim with NO heightfield now floors at
/// `core::collision::NO_GROUND` (−300) instead of 0, so a test about
/// standing on flat ground has to say so.
fn flat_ground() -> Heightfield {
    Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    }
}

#[test]
fn soldier_follows_flat_ground_and_camera_tracks() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let fwd = PadState {
        ly: 32767,
        ..Default::default()
    };
    for _ in 0..90 {
        w.step_tick(&[fwd]);
    }
    assert!(
        w.soldier().pos.z < -10.0,
        "walked forward: {}",
        w.soldier().pos
    );
    assert_eq!(w.soldier().pos.y, 0.0);
    let cam = w.camera();
    assert!(cam.position.z > w.soldier().pos.z, "camera behind soldier");
    // The camera looks the section's tilt below the aim, from above the
    // track centre (card AIMA).
    assert!((1.8..2.8).contains(&cam.position.y), "{}", cam.position);
    assert!(cam.pitch < w.soldier().pitch - 0.08);
}

/// The terrain floors a camera filming a soldier above it and caps one
/// filming a soldier under it (a cave or a hangar under uncut terrain,
/// BF2 GEO1's hangar post): it never lifts the camera through the sheet.
#[test]
fn camera_stays_on_the_soldiers_side_of_the_terrain() {
    let w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let settle = |t, p| w.settle_camera(CamKind::Soldier, t, p, None, None).0;
    let open = settle(Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, 2.5, 4.0));
    assert_eq!(
        open,
        Vec3::new(0.0, 2.5, 4.0),
        "nothing in the way: untouched"
    );
    let above = settle(Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, -2.0, 4.0));
    assert!(
        above.y >= CAMERA_TERRAIN_GAP && above.z < 4.0,
        "pulled in and floored: {above}"
    );
    let cave = settle(Vec3::new(0.0, -30.0, 0.0), Vec3::new(0.0, -28.0, 4.0));
    assert_eq!(cave.y, -28.0, "under the sheet it stays where it was");
    let roof = settle(Vec3::new(0.0, -2.0, 0.0), Vec3::new(0.0, 1.0, 4.0));
    assert!(
        roof.y <= -CAMERA_TERRAIN_GAP,
        "and never rises through it: {roof}"
    );
}

/// A seat's camera keeps the same rule from under uncut terrain: its pull point
/// rides the vehicle, so a vehicle in BF2 GEO1's hangar films from inside it.
#[test]
fn seat_camera_under_the_sheet_stays_under_it() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let h = w.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
        kind: core::vehicle::VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, -8.0, 0.0),
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![core::vehicle::SeatKind::Pilot],
    });
    let player = w.player;
    assert!(w.sim.seat_unit(player, h, 0));
    // The default camera wants 3 m over the vehicle: short of the sheet here...
    let y = w.camera().position.y;
    assert!(y <= -CAMERA_TERRAIN_GAP, "under the sheet: {y}");
    // ...and through it here, where the sheet pulls it in short.
    w.sim.vehicle_mut(h).unwrap().pos.y = -2.0;
    let y = w.camera().position.y;
    assert!(y <= -CAMERA_TERRAIN_GAP, "and never through it: {y}");
}

/// The PS2 sets a vehicle's camera origin from its model's vis box when it reads
/// `GeometryName`: 0.9 of its top on a hover and a flyer, 0.8 on a walker. A building
/// turret and a class with no model keep the section's 1.5 m.
#[test]
fn a_vehicles_pull_point_follows_its_model_as_on_the_ps2() {
    use core::vehicle::VehicleKind::{Flyer, Hover, Turret, Walker};
    assert_eq!(vehicle_pull_height(Hover, Some(10.0)), 9.0);
    assert_eq!(vehicle_pull_height(Flyer, Some(10.0)), 9.0);
    assert_eq!(vehicle_pull_height(Walker, Some(10.0)), 8.0);
    assert_eq!(vehicle_pull_height(Turret, Some(10.0)), CAMERA_PULL_Y);
    for kind in [Hover, Flyer, Walker] {
        assert_eq!(vehicle_pull_height(kind, None), CAMERA_PULL_Y);
    }
}

/// A tall model's pull point can stand past the sheet from the vehicle: over it from a
/// hangar, or under it from a flyer banked at the ground. The camera stays on the
/// vehicle's side.
#[test]
fn a_tall_models_pull_point_keeps_the_camera_on_the_vehicles_side_of_the_sheet() {
    let spawn = |w: &mut WalkSim, pos: Vec3| {
        let h = w.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
            kind: core::vehicle::VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        });
        w.vehicle_tops = vec![Some(10.0)];
        let player = w.player;
        assert!(w.sim.seat_unit(player, h, 0));
        h
    };
    let mut hangar = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    spawn(&mut hangar, Vec3::new(0.0, -8.0, 0.0));
    let y = hangar.camera().position.y;
    assert!(y <= -CAMERA_TERRAIN_GAP, "from under the sheet: {y}");

    let mut low = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let h = spawn(&mut low, Vec3::new(0.0, 3.0, 0.0));
    low.sim.vehicle_mut(h).unwrap().rot = glam::Quat::from_rotation_z(2.0);
    let y = low.camera().position.y;
    assert!(y >= CAMERA_TERRAIN_GAP, "from over the sheet: {y}");
}

/// The pull point rides the vehicle in its own frame, as the PS2's section point
/// does on its matrix. A camera on a banked flyer pulls toward the flyer's up.
#[test]
fn a_seat_camera_pulls_toward_the_point_in_the_vehicles_own_frame() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    let h = w.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
        kind: core::vehicle::VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(3.0, 50.0, 4.0),
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![core::vehicle::SeatKind::Pilot],
    });
    w.vehicle_tops = vec![Some(5.0)];
    let v = w.sim.vehicle_mut(h).unwrap();
    v.rot = glam::Quat::from_rotation_z(0.5);
    let v = w.sim.vehicle(h).unwrap();
    let (origin, _) = vehicle_camera(v, None, None, 1.0, w.seat_pull_origin(v, 0));
    let want = v.pos + glam::Quat::from_rotation_z(0.5) * (Vec3::Y * 4.5);
    assert!(origin.distance(want) < 1e-4, "{origin} / {want}");
}

#[test]
fn accumulator_steps_at_thirty_hz() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    let pad = PadState::default();
    assert_eq!(w.advance(1.0 / 60.0, &[pad]), 0); // half a tick: none yet
    assert_eq!(w.advance(1.0 / 60.0, &[pad]), 1); // second half: one tick
    let capped = w.advance(1.0, &[pad]);
    assert!(
        (2..=3).contains(&capped),
        "capped at MAX_FRAME_ACCUM_SECS (float rounding): {capped}"
    );
}

#[test]
fn marker_height_follows_stance() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    let press = PadState {
        buttons: buttons::ROLL_STANCE,
        ..Default::default()
    };
    let idle = PadState::default();
    for _ in 0..20 {
        w.step_tick(&[idle]);
    }
    w.step_tick(&[press]);
    assert_eq!(w.soldier().stance, Stance::Crouch);
    assert_eq!(capsule_height(w.soldier().stance), 1.6);
}

/// Aim assist on flat ground: a target 20 m ahead, slightly right of the
/// aim; holding fire with a neutral stick pulls yaw toward it — and the
/// rifle kills it.
#[test]
fn fire_magnetism_pulls_yaw_and_shots_kill_the_target() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    let t = w
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.4, 0.0, -20.0)))
        .unwrap();
    let fire = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    for _ in 0..30 {
        w.step_tick(&[fire]);
    }
    assert_eq!(w.magnet(), Some(t), "target acquired");
    // Target is to the RIGHT (+X); positive yaw turns left, so the pull
    // drives yaw negative.
    assert!(
        w.soldier().yaw < -0.005,
        "yaw pulled toward target: {}",
        w.soldier().yaw
    );
    assert!(w.target_draws()[0].health_frac < 1.0, "hits landed");
    for _ in 0..60 {
        w.step_tick(&[fire]);
    }
    assert!(w.sim.unit(t).is_none(), "five rifle hits kill a trooper");
    assert!(w.target_draws().is_empty());
    assert_eq!(w.sim.battle.teams[1].tickets(), 249);

    // Intensity 0 = no pull.
    let mut w0 = WalkSim::new(1, Vec3::ZERO, None);
    w0.sim.assist_intensity = 0.0;
    w0.sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.4, 0.0, -20.0)))
        .unwrap();
    for _ in 0..30 {
        w0.step_tick(&[fire]);
    }
    assert_eq!(w0.soldier().yaw, 0.0, "slider at 0 leaves aim untouched");
}

/// Out-of-range or off-cone targets never become magnets
/// (`re-aim-assist.md` §3.3): the one at 80 m is past LockOnRange + r
/// (64 + 1.42) but inside ExtremeRange (120), so it is the reticle's
/// pick and not the magnet. The dummy rig spawns three enemies and the
/// pacer moves.
#[test]
fn acquisition_respects_cone_and_range() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    let far = w
        .sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -80.0)))
        .unwrap();
    w.sim
        .spawn(&UnitSpec::trooper(1, Vec3::new(15.0, 0.0, -10.0))) // far off the cone
        .unwrap();
    // A level aim: the spawn pitch's ray is in the floor 33 m out.
    w.sim.units[w.player.index as usize].soldier.pitch = 0.0;
    w.step_tick(&[PadState::default()]);
    assert_eq!(w.magnet(), None);
    assert_eq!(
        w.sim.reticle_pick[0],
        Some((core::sim::AimTarget::Unit(far), -1))
    );

    let mut r = WalkSim::new(1, Vec3::ZERO, None);
    r.spawn_dummy_targets();
    assert_eq!(r.target_draws().len(), 3);
    let before = r.sim.units[3].soldier.pos;
    for _ in 0..30 {
        r.step_tick(&[PadState::default()]);
    }
    assert!(
        (r.sim.units[3].soldier.pos - before).length() > 1.5,
        "pacer moved"
    );
    assert!(r.bolt_draws().is_empty());
}

/// A 40 m box with its top at y = 6 around (100, 100) and nothing at the
/// origin — a stand-in for a BF2 map with no heightfield, where the only
/// ground is model collision (`Scene::collision_world`).
use crate::layout::{SpawnNode, SpawnPath};

fn island_world() -> core::collision::CollisionWorld {
    use core::collision::{Body, CollisionWorld, Shape};
    CollisionWorld::new(
        vec![Body::new(
            Mat4::from_translation(Vec3::new(100.0, 5.0, 100.0)),
            Shape::Box {
                half: Vec3::new(20.0, 1.0, 20.0),
            },
        )],
        Vec::new(),
    )
}

fn post(name: &str, team: u8, pos: Vec3, spawn_path: Option<&str>) -> CommandPostDef {
    CommandPostDef {
        name: name.to_string(),
        label: String::new(),
        class: "com_bldg_controlzone".to_string(),
        team,
        pos,
        capture: None,
        control: None,
        spawn_path: spawn_path.map(str::to_owned),
        ally_path: None,
        ally_count: None,
        value_bleed: crate::layout::VALUE_DEFAULT,
        ai_spawn_weight: core::cp::AI_SPAWN_WEIGHT_DEFAULT,
        value_atk: [crate::layout::VALUE_DEFAULT; 5],
        value_def: [crate::layout::VALUE_DEFAULT; 5],
        major: false,
        space: false,
        vo: Default::default(),
    }
}

/// The layout the rescue is judged on: an ENEMY post over the island
/// (wrong team), a FRIENDLY post over the void (right team, no ground)
/// and a friendly post over the island with a real spawn path — in that
/// order, so passing means the rescue skipped the first two for the
/// stated reasons and not by luck of ordering.
fn rescue_layout() -> MapLayout {
    MapLayout {
        cps: vec![
            post("cp_enemy", 2, Vec3::new(100.0, 6.0, 100.0), None),
            post("cp_void", 1, Vec3::new(500.0, 0.0, 500.0), None),
            post(
                "cp_home",
                1,
                Vec3::new(90.0, 6.0, 90.0),
                Some("cp_home_spawn"),
            ),
        ],
        paths: vec![SpawnPath {
            name: "cp_home_spawn".to_string(),
            nodes: vec![SpawnNode {
                pos: Vec3::new(110.0, 6.0, 110.0),
                yaw: 1.25,
            }],
        }],
        regions: Vec::new(),
        // Any map but TAT1: the rule under test applies everywhere else.
        map: "kamino1".to_string(),
    }
}

/// Card ERAM: a player seated on team 1 (its army's own team in the
/// mission picked) is a team-1 unit and starts at team 1's post, where
/// the team-0 player of the same battle starts at team 0's.
#[test]
fn a_player_seated_on_team_1_starts_at_its_own_post() {
    let start = |seat: u8| {
        let t = TeamTemplate::default_trooper();
        let mut w =
            WalkSim::with_teams_seat(Sim::empty(1), None, Vec3::ZERO, [t.clone(), t], None, seat);
        w.sim.collision = island_world();
        w.setup_conquest(&rescue_layout(), [0, 0], true);
        let u = &w.sim.units[w.player.index as usize];
        (u.team, u.soldier.pos)
    };
    // `cp_enemy` (team 2) has no spawn path: its one node is beside it.
    let own = Vec3::new(103.0, 6.0, 103.0);
    assert_eq!(start(1), (1, own));
    let (team, pos) = start(0);
    assert_eq!(team, 0);
    assert_ne!(pos, own);
}

/// A space hangar's post spawns its team and never changes hands, however long an
/// enemy stands in it; an ordinary post falls.
#[test]
fn a_space_hangars_post_cannot_be_taken() {
    let owner_after = |space: bool| {
        let t = TeamTemplate::default_trooper();
        let mut w =
            WalkSim::with_teams_seat(Sim::empty(1), None, Vec3::ZERO, [t.clone(), t], None, 0);
        w.sim.collision = island_world();
        let mut layout = rescue_layout();
        layout.cps.iter_mut().for_each(|c| c.space = space);
        w.setup_conquest(&layout, [0, 0], true);
        let cp = w
            .sim
            .cps
            .iter_mut()
            .find(|c| c.owner.is_some())
            .expect("a held post");
        let held = cp.owner;
        let enemy = usize::from(held.unwrap() == 0);
        let mut inside = [0u32; 4];
        inside[enemy] = 8;
        for _ in 0..10_000 {
            cp.update(1.0, &inside);
        }
        (held, cp.owner)
    };
    let (held, after) = owner_after(true);
    assert_eq!(held, after);
    let (held, after) = owner_after(false);
    assert_ne!(held, after);
}

/// `game --map` with no mission, no `--ai` and no `--spawn`: the player
/// starts on nothing, and the rescue moves them to the lowest-index
/// FRIENDLY post whose own spawn node has ground — node position, node
/// yaw, probed y.
#[test]
fn rescue_spawn_moves_a_groundless_player_to_the_first_friendly_post() {
    let mut w = WalkSim::new(1, Vec3::ZERO, None);
    w.sim.collision = island_world();
    assert_eq!(
        w.sim.ground_under(w.soldier().pos, false),
        core::collision::NO_GROUND,
        "the authored spawn has nothing under it"
    );

    let moved = w.rescue_spawn(&rescue_layout());

    assert_eq!(moved, Some(("cp_home".to_string(), 6.0)));
    assert_eq!(w.soldier().pos, Vec3::new(110.0, 6.0, 110.0));
    assert_eq!(w.soldier().yaw, 1.25);
}

/// A player standing on the island: `game --map` gave them ground, and
/// they STILL start at the post — the half queue item WS widened (24 maps
/// had ground under the default point and started at an arbitrary spot in
/// the terrain anyway). The move is silent — it returns `None`, because
/// the caller's line is the no-ground rescue's — and the two posts before
/// `cp_home` are skipped for their own reasons, not by ordering luck.
#[test]
fn a_grounded_player_starts_at_the_first_friendly_post_too() {
    let mut w = WalkSim::new(1, Vec3::new(100.0, 0.0, 100.0), None);
    w.sim.collision = island_world();
    // Standing on the island's top face (`Sim::spawn` ran before the
    // collision world was there, so put them where it would have).
    w.sim.units[w.player.index as usize].soldier.pos = Vec3::new(100.0, 6.0, 100.0);
    assert!(
        w.sim.ground_under(w.soldier().pos, false) > core::collision::NO_GROUND,
        "the authored spawn has ground under it"
    );

    assert_eq!(w.rescue_spawn(&rescue_layout()), None, "moved silently");

    assert_eq!(w.soldier().pos, Vec3::new(110.0, 6.0, 110.0));
    assert_eq!(w.soldier().yaw, 1.25);
}

/// TAT1 follows the same friendly-post rule as every other map.
#[test]
fn rescue_spawn_moves_a_grounded_tat1_player_to_the_post() {
    let mut w = WalkSim::new(1, Vec3::new(100.0, 0.0, 100.0), None);
    w.sim.collision = island_world();
    w.sim.units[w.player.index as usize].soldier.pos = Vec3::new(100.0, 6.0, 100.0);
    let mut tat1 = rescue_layout();
    tat1.map = "tat1".to_string();
    assert_eq!(w.rescue_spawn(&tat1), None, "moved silently");
    assert_eq!(w.soldier().pos, Vec3::new(110.0, 6.0, 110.0));
    assert_eq!(w.soldier().yaw, 1.25);
}

/// A map whose friendly posts are all over the void cannot rescue a player.
#[test]
fn rescue_spawn_leaves_a_hopeless_map_alone() {
    let mut v = WalkSim::new(1, Vec3::ZERO, None);
    v.sim.collision = island_world();
    let mut layout = rescue_layout();
    layout.cps.retain(|c| c.name != "cp_home");
    // `Sim::spawn` already dropped it to the sentinel, and with no
    // rescuable post it stays there — a broken map stays broken rather
    // than being teleported somewhere unauthored.
    let stranded = v.soldier().pos;
    assert_eq!(stranded, Vec3::new(0.0, core::collision::NO_GROUND, 0.0));
    assert_eq!(v.rescue_spawn(&layout), None, "no friendly post has ground");
    assert_eq!(v.soldier().pos, stranded, "left where it was");
}

/// The player's handle follows his slot through a respawn (its
/// generation goes up): before cleanse 16 it kept the first life's, so
/// every `sim.unit(walk.player)` read None from the first death on.
#[test]
fn the_player_handle_follows_a_respawn() {
    use core::cp::CpDef;
    use core::region::Region;
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let (capture_time, neutralize_time) = CpDef::class_times(false);
    walk.sim.add_cp(
        CpDef {
            name: "cp".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 8.0),
            control: None,
            value_bleed: 0,
            value_atk: [0, 0],
            value_def: [0, 0],
            spawn_nodes: vec![(Vec3::new(0.0, 0.0, 3.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        Some(0),
    );
    let first = walk.player;
    assert!(walk.player_on_foot());
    walk.sim
        .kill_environment(first, core::sim::DeathCause::Fall);
    let mut ticks = 0;
    while !walk.sim.units[first.index as usize].alive {
        walk.step_tick(&[PadState::default()]);
        ticks += 1;
        assert!(ticks < 3000, "the player never respawned");
    }
    assert_eq!(walk.player.index, first.index);
    assert_eq!(walk.player.gen, first.gen + 1);
    assert_eq!(Some(walk.player), walk.sim.players[0]);
    assert!(walk.player_on_foot(), "the new life is found");
    assert_eq!(walk.player_team(), 0);
}

/// Card SSELB: behind the level start's spawn screens the view is the
/// mission's camera shot; a screen change cuts to another shot, a move on
/// the same screen does not; the first spawn ends the backdrop for good.
#[test]
fn the_level_start_view_is_the_camera_shot_until_the_spawn() {
    use crate::spawn_backdrop::{Backdrop, CameraShot};
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let chase = w.camera();
    assert!(w.sim.open_spawn_screens(0, true));
    let shots: Vec<CameraShot> = (0..4)
        .map(|i| CameraShot::from_call([1.0, 0.0, 0.0, 0.0, 10.0 * i as f32, 50.0, 0.0]))
        .collect();
    w.backdrop = Backdrop::new(shots, 3);
    w.follow_backdrop();
    assert_eq!(w.camera().position, Vec3::new(0.0, 50.0, 0.0), "shot 0");
    let pad = |b: u32| PadState {
        buttons: b,
        ..PadState::default()
    };
    // Down moves the highlight: the same screen, the same shot.
    w.step_tick(&[pad(buttons::SPAWN_POST_DOWN)]);
    w.step_tick(&[pad(0)]);
    assert_eq!(w.sim.side_choice[0], core::sim::SideChoice::Team2);
    assert_eq!(w.camera().position.x, 0.0);
    // X takes the side: the map screen, another shot.
    w.step_tick(&[pad(buttons::SPAWN_ACCEPT)]);
    w.step_tick(&[pad(0)]);
    assert_eq!(w.sim.spawn_stage[0], core::sim::SpawnStage::Map);
    assert_ne!(w.camera().position.x, 0.0);
    assert_eq!(w.camera().position.y, 50.0);
    // On the field: the backdrop is gone and the chase camera is back.
    let i = w.player.index as usize;
    w.sim.units[i].alive = true;
    w.follow_backdrop();
    assert!(w.backdrop.is_none());
    assert_eq!(w.camera().position, chase.position);
}

fn run_ticks(walk: &mut WalkSim, pad: PadState, ticks: usize) {
    for _ in 0..ticks {
        walk.step_tick(&[pad]);
    }
}

#[test]
fn the_reticle_follows_a_flown_droids_aim() {
    struct NoProps;
    impl core::weapon::PropSource for NoProps {
        fn prop(&self, _: &str) -> Option<&str> {
            None
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let droid = core::remote::DroidClass::from_props(&NoProps, false);
    let remote =
        core::remote::RemoteWeapon::new(&NoProps, droid, core::weapon::WeaponClass::bf1_rifle());
    let mut class = core::weapon::WeaponClass::bf1_rifle();
    class.remote = Some(Box::new(remote.clone()));
    let weapon = w.sim.add_weapon_class(class);
    let d = core::remote::Droid::new(
        1,
        w.player,
        0,
        weapon,
        &remote,
        Vec3::new(0.0, 2.0, -6.0),
        Vec3::ZERO,
        0.0,
        true,
    );
    w.sim.droids.push(d);
    w.sim.droids[0].controlled = true;
    let mut seen = Vec::new();
    for pitch in [-0.8, -0.4, 0.0, 0.4, 0.8] {
        w.sim.droids[0].pitch = pitch;
        w.sim.units[w.player.index as usize].soldier.pitch = -pitch;
        let y = w.reticle_y();
        assert!(
            (0.3..0.7).contains(&y),
            "droid pitch {pitch}: reticle at {y}"
        );
        seen.push(y);
    }
    assert!(
        seen.windows(2).all(|p| (p[0] - p[1]).abs() < 0.05),
        "{seen:?}"
    );
}

/// A droid class with the disc's camera keys (`EyePointOffset` 0 0.5 0,
/// `TiltValue` 5), flown by the player from `pos` at `yaw`.
fn fly_droid(w: &mut WalkSim, pos: Vec3, yaw: f32) {
    struct Disc;
    impl core::weapon::PropSource for Disc {
        fn prop(&self, key: &str) -> Option<&str> {
            match key {
                "EyePointOffset" => Some("0 0.5 0"),
                "TrackOffset" => Some("0 -0.25 1.5"),
                "TiltValue" => Some("5"),
                _ => None,
            }
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }
    let droid = core::remote::DroidClass::from_props(&Disc, false);
    let remote =
        core::remote::RemoteWeapon::new(&Disc, droid, core::weapon::WeaponClass::bf1_rifle());
    let mut class = core::weapon::WeaponClass::bf1_rifle();
    class.remote = Some(Box::new(remote.clone()));
    let weapon = w.sim.add_weapon_class(class);
    let d = core::remote::Droid::new(1, w.player, 0, weapon, &remote, pos, Vec3::ZERO, yaw, true);
    w.sim.droids.push(d);
    w.sim.droids[0].controlled = true;
}

/// Flying a droid, the camera is a chase camera: [`DROID_CHASE`] from the
/// track centre 0.5 m above the droid, behind it along its aim and above
/// it, in the view pitched down by the 5° tilt; the droid sits below the
/// screen's middle, inside the frame. Back on foot, the soldier's camera.
#[test]
fn a_flown_droid_has_a_chase_camera_behind_it() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let on_foot = w.camera();
    fly_droid(&mut w, Vec3::new(3.0, 2.0, -6.0), 0.7);
    let d = w.sim.droids[0].clone();
    let cam = w.camera();
    let target = d.pos + Vec3::Y * 0.5;
    assert!(((cam.position - target).length() - DROID_CHASE.length()).abs() < 1e-4);
    let (fwd, _, _) = d.aim_frame();
    let back = (d.pos - cam.position).dot(fwd);
    assert!(
        (back - DROID_CHASE.z).abs() < 0.1,
        "{back} m behind along the aim"
    );
    // At the eye point's height, raised by the tilt: above the droid.
    assert!(cam.position.y > target.y + 0.1, "above: {:?}", cam.position);
    assert!((cam.yaw - d.yaw).abs() < 1e-6);
    assert!((cam.pitch - (d.pitch - 5f32.to_radians())).abs() < 1e-6);
    // The droid's model is in view, below the middle and clear of it.
    let c = cam.view_proj(16.0 / 9.0) * (d.pos + Vec3::Y * 0.15).extend(1.0);
    let (x, y) = (c.x / c.w, c.y / c.w);
    assert!(
        c.w > 0.0 && x.abs() < 0.01 && (-0.8..-0.15).contains(&y),
        "({x}, {y})"
    );
    // It swings round the droid as the droid pitches, like the soldier's.
    w.sim.droids[0].pitch = 0.6;
    let up = w.camera();
    assert!(up.position.y < cam.position.y);
    assert!(((up.position - target).length() - DROID_CHASE.length()).abs() < 1e-4);
    w.sim.droids[0].controlled = false;
    assert_eq!(w.camera().position, on_foot.position);
}

/// A droid skimming the ground and looking up would put its camera under
/// the terrain: the world floors it, as it does the soldier's.
#[test]
fn a_flown_droids_camera_stays_out_of_the_ground() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    fly_droid(&mut w, Vec3::new(0.0, 0.3, -6.0), 0.0);
    w.sim.droids[0].pitch = 0.87;
    let d = w.sim.droids[0].clone();
    let class = w.sim.droid_class(&d).unwrap().clone();
    let (_, ideal) = droid_camera(&d, &class, w.fov_y());
    assert!(ideal.position.y < 0.0, "the ideal place is underground");
    let cam = w.camera();
    assert!(
        cam.position.y >= CAMERA_TERRAIN_GAP - 1e-4,
        "{:?}",
        cam.position
    );
}

/// The reticle is drawn where the droid's aim ray goes: the ray from the
/// drawn camera through the reticle's screen point (the middle across, at
/// `reticle_y` down) meets the ray's point 1024 m out, at every pitch.
#[test]
fn the_reticle_is_on_a_flown_droids_aim_ray() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    fly_droid(&mut w, Vec3::new(0.0, 4.0, -6.0), -0.4);
    for pitch in [-0.8, -0.3, 0.0, 0.3, 0.8] {
        w.sim.droids[0].pitch = pitch;
        let d = w.sim.droids[0].clone();
        let cam = w.camera();
        let aspect = 16.0 / 9.0;
        let ndc = glam::Vec2::new(0.0, 1.0 - 2.0 * w.reticle_y());
        let inv = cam.view_proj(aspect).inverse();
        let far = inv * glam::Vec4::new(ndc.x, ndc.y, 0.5, 1.0);
        let through = (far.truncate() / far.w - cam.position).normalize();
        let aim = d.muzzle() + d.aim_frame().0 * 1024.0;
        let want = (aim - cam.position).normalize();
        assert!(
            through.angle_between(want) < 1e-3,
            "pitch {pitch}: {through:?} vs {want:?}"
        );
    }
}

#[test]
fn a_modelled_shell_draws_no_placeholder_box() {
    let mut r = WalkSim::new(1, Vec3::ZERO, None);
    let mortar = r.sim.add_weapon_class(core::weapon::WeaponClass {
        ordnance: core::weapon::OrdnanceClass {
            kind: core::weapon::OrdKind::Shell,
            geometry: Some("com_weap_inf_grenadethermal_ord".into()),
            ..Default::default()
        },
        ..Default::default()
    });
    let bare = r.sim.add_weapon_class(core::weapon::WeaponClass::default());
    for class in [mortar, bare] {
        let b = core::projectile::Bolt::fire(
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::X,
            40.0,
            8.0,
            core::projectile::Shooter {
                owner: core::unit::UnitHandle { index: 0, gen: 0 },
                team: 1,
                weapon_class: class,
                is_player: false,
            },
            None,
            None,
            &r.sim.collision,
            core::collision::HullFilter::All,
        );
        r.sim.bolts.push(b);
    }
    assert_eq!(r.bolt_draws().len(), 1);
    assert_eq!(crate::bolt_vis::missiles_in_flight(&r).len(), 1);
}

#[test]
fn first_person_arms_and_eye_view_gating() {
    let mut walk = WalkSim::new(1, Vec3::ZERO, None);
    assert!(!walk.first_person());
    assert!(!walk.in_eye_view());
    assert!(!walk.shows_first_person_arms());
    assert!(!walk.player_body_hidden());

    walk.sim.units[walk.player.index as usize].first_person = true;
    assert!(walk.first_person());
    assert!(walk.in_eye_view());
    assert!(walk.shows_first_person_arms());
    assert!(walk.player_body_hidden());

    walk.sim.units[walk.player.index as usize].zoom = Some(core::unit::Zoom {
        step: 0,
        value: 2.0,
        divisor: 2.0,
        scope: false,
    });
    assert!(walk.in_eye_view());
    assert!(!walk.shows_first_person_arms());

    walk.sim.units[walk.player.index as usize].zoom = None;
    walk.backdrop = crate::spawn_backdrop::Backdrop::new(
        vec![crate::spawn_backdrop::CameraShot {
            rot: glam::Quat::IDENTITY,
            pos: glam::Vec3::ZERO,
        }],
        crate::spawn_backdrop::LCG_RESEED,
    );
    assert!(!walk.in_eye_view());
    assert!(!walk.shows_first_person_arms());
    assert!(!walk.player_body_hidden());
}

#[test]
fn a_rider_lost_with_the_hull_holds_the_seat_view() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let h = w.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::new(0.0, 40.0, 0.0),
        yaw: 0.0,
        max_health: 100.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let mut seat = crate::vehicle_class::SeatSpec::new(SeatKind::Pilot);
    seat.track_center = Some([0.0, 4.0, -6.0]);
    seat.track_offset = Some([0.0, 0.0, 0.0]);
    w.vehicle_seats = vec![vec![seat]];
    w.vehicle_tops = vec![Some(2.0)];
    let player = w.player;
    assert!(w.sim.seat_unit(player, h, 0));
    run_ticks(&mut w, PadState::default(), 1);
    let view = w.camera().position;
    w.sim.destroy_vehicle(h, None);
    let at = player.index as usize;
    assert!(!w.sim.units[at].alive);
    assert_eq!(w.camera().position, view);
    run_ticks(&mut w, PadState::default(), 5);
    assert_eq!(w.camera().position, view);
    w.sim.units[at].alive = true;
    w.seat_view = w.next_seat_view();
    assert!(w.seat_view.is_none());
}

/// A ground rider lost with his hull keeps the seat view until he respawns;
/// a space round lets it go after `DEATH_VIEW_HOLD`.
#[test]
fn a_ground_rider_lost_with_the_hull_holds_the_seat_view_past_four_seconds() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    for space in [false, true] {
        let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
        if space {
            w.sim.space_assault = Some(core::sim::SpaceAssault::new(10));
        }
        w.sim.respawn_delay = 60.0;
        let h = w.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Hover,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(0.0, 2.0, 0.0),
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        let mut seat = crate::vehicle_class::SeatSpec::new(SeatKind::Pilot);
        seat.track_center = Some([0.0, 4.0, -6.0]);
        seat.track_offset = Some([0.0, 0.0, 0.0]);
        w.vehicle_seats = vec![vec![seat]];
        w.vehicle_tops = vec![Some(2.0)];
        let player = w.player;
        assert!(w.sim.seat_unit(player, h, 0));
        run_ticks(&mut w, PadState::default(), 1);
        let view = w.camera().position;
        w.sim.destroy_vehicle(h, None);
        let ticks = ((DEATH_VIEW_HOLD + 1.0) / core::TICK_DT) as usize;
        run_ticks(&mut w, PadState::default(), ticks);
        assert!(!w.sim.units[player.index as usize].alive);
        assert_eq!(w.holds_seat_view(), !space, "space {space}");
        if space {
            assert_ne!(w.camera().position, view);
        } else {
            assert_eq!(w.camera().position, view);
            let ears = crate::event_sounds::listener(&w);
            assert_eq!(ears.pos, view, "the ears stay with the held view");
        }
    }
}

#[test]
fn rc_flight_controls_follow_the_preference_in_ground_and_space_pilot_seats() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use input::FlightControls;
    for space in [false, true] {
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        if space {
            walk.sim.space_assault = Some(core::sim::SpaceAssault::new(10));
        }
        let rc = FlightControls::RcMode2;
        assert_eq!(walk.flight_controls(rc), None);
        let flyer = walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::ZERO,
            yaw: 0.0,
            max_health: 100.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
        });
        assert!(walk.sim.seat_unit(walk.player, flyer, 1));
        assert_eq!(walk.flight_controls(rc), None);
        walk.sim.unseat_unit(walk.player);
        assert!(walk.sim.seat_unit(walk.player, flyer, 0));
        assert_eq!(walk.flight_controls(rc), Some(rc));
        assert_eq!(
            walk.flight_controls(rc.toggled()),
            Some(FlightControls::Classic)
        );
        walk.sim.unseat_unit(walk.player);
        assert_eq!(walk.flight_controls(rc), None);
    }
}

#[test]
fn a_guided_rocket_flies_under_its_chase_camera_and_backs_off_when_spent() {
    struct NoProps;
    impl core::weapon::PropSource for NoProps {
        fn prop(&self, _: &str) -> Option<&str> {
            None
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let mut class = core::guided::GuidedClass::from_props(&NoProps);
    class.flight.min_speed = 30.0;
    class.flight.mid_speed = 30.0;
    class.flight.max_speed = 30.0;
    class.flight.energy_max = 100.0;
    class.view.track_center = Some([0.0, 1.0, -3.0]);
    class.view.track_offset = Some([0.0, 2.0, 6.0]);
    let flying = core::guided::GuidedRocket::flying(&class);
    let wc = w.sim.add_weapon_class(core::weapon::WeaponClass {
        guided: Some(Box::new(core::guided::GuidedWeapon {
            class,
            spawn_offset: Vec3::ZERO,
            requires_award: false,
        })),
        ..Default::default()
    });
    let player = w.player;
    w.sim.unit_mut(player).unwrap().weapon_class = wc;
    let pos = Vec3::new(10.0, 30.0, 30.0);
    let dir = Quat::from_rotation_y(0.7) * Vec3::NEG_Z;
    w.sim.guided.push(core::guided::GuidedRocket {
        id: 42,
        owner: player,
        team: 0,
        weapon_class: wc,
        seat: None,
        pos,
        vel: dir * 30.0,
        rot: core::guided::GuidedRocket::body(dir),
        flyer: flying,
        health: 1.0,
        age: 0.0,
        prev_buttons: 0,
        pending_end: None,
        spent: None,
        blind: 0.0,
    });
    assert!(w.player_controls_flyer());
    assert_eq!(
        w.flight_controls(input::FlightControls::RcMode2),
        Some(input::FlightControls::Classic)
    );
    let cam = w.camera();
    let behind = (cam.position - pos).dot(-dir);
    assert!((6.0..12.0).contains(&behind), "behind {behind}");
    assert!(cam.position.y > pos.y);
    assert!((cam.forward() - dir).length() < 0.2);
    assert!((w.reticle_y() - 0.5).abs() < 0.2);
    run_ticks(&mut w, PadState::default(), 3);
    let held = w.cam_guided.expect("the guided camera holds its eye");
    assert_eq!(held.rocket, 42);
    let r = w.sim.guided[0].clone();
    assert!((held.eye - r.pos).dot(-r.forward()) > 6.0);
    w.sim.guided[0].flyer.energy = 0.0;
    run_ticks(&mut w, PadState::default(), 15);
    let r = w.sim.guided[0].clone();
    assert!(!r.live());
    let cam = w.cam_guided.expect("the spent rocket keeps the camera");
    let back = (cam.eye - r.pos).dot(-(r.rot * Vec3::Z));
    assert!((20.0..22.0).contains(&back), "backed off {back}");
    run_ticks(&mut w, PadState::default(), 20);
    assert!(w.sim.guided.is_empty());
    assert!(w.cam_guided.is_none());
}

/// Playtest A7: a death on foot starts the death camera (the body drawn, a
/// view that is not the chase camera), an enemy's kill keeps its killer, and
/// the respawn ends it.
#[test]
fn a_death_on_foot_starts_the_death_view_and_respawn_clears_it() {
    use core::cp::CpDef;
    use core::region::Region;
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    let (capture_time, neutralize_time) = CpDef::class_times(false);
    w.sim.add_cp(
        CpDef {
            name: "cp".into(),
            pos: Vec3::ZERO,
            capture: Region::sphere(Vec3::ZERO, 8.0),
            control: None,
            value_bleed: 0,
            value_atk: [0, 0],
            value_def: [0, 0],
            spawn_nodes: vec![(Vec3::new(0.0, 0.0, 3.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        Some(0),
    );
    run_ticks(&mut w, PadState::default(), 10);
    let first = w.player;
    w.sim
        .kill_environment(first, core::sim::DeathCause::Suicide);
    run_ticks(&mut w, PadState::default(), 1);
    let d = w.death_cam.expect("the death camera starts");
    assert_eq!((d.body, d.killer), (first, None));
    run_ticks(&mut w, PadState::default(), 20);
    let me = &w.sim.units[first.index as usize];
    let chase = me.ideal_camera(w.sim.ground_rules, w.sim.views()).position;
    assert!(
        (w.camera().position - chase).length() > 1.0,
        "not the chase camera"
    );
    assert!(!w.player_body_hidden());
    let mut ticks = 0;
    while w.sim.unit(w.player).is_none() {
        run_ticks(&mut w, PadState::default(), 1);
        ticks += 1;
        assert!(ticks < 3000, "the player never respawned");
    }
    assert!(w.death_cam.is_none(), "the respawn ends it");

    // An enemy's blast: its thrower is the killer followed.
    let enemy = w
        .sim
        .spawn(&core::unit::UnitSpec::trooper(
            1,
            Vec3::new(0.0, 0.0, -15.0),
        ))
        .unwrap();
    run_ticks(&mut w, PadState::default(), 1);
    let me = w.player;
    let pos = w.sim.unit(me).unwrap().soldier.pos;
    let lethal = core::weapon::ExplosionClass {
        damage: 10_000.0,
        damage_outer: 10_000.0,
        ..Default::default()
    };
    w.sim.explode(&lethal, pos, Some(enemy), Some(1));
    assert!(w.sim.unit(me).is_none(), "the blast killed the player");
    run_ticks(&mut w, PadState::default(), 1);
    assert_eq!(w.death_cam.expect("started").killer, Some(enemy));
}

/// Space maps keep their own death flow: a death there keeps the chase
/// camera.
#[test]
fn a_space_map_death_keeps_the_chase_camera() {
    let mut w = WalkSim::new(1, Vec3::ZERO, Some(flat_ground()));
    // Only the space scripts add landing regions.
    w.sim
        .landing_regions
        .push(core::region::Region::sphere(Vec3::ZERO, 10.0));
    run_ticks(&mut w, PadState::default(), 10);
    w.sim
        .kill_environment(w.player, core::sim::DeathCause::Suicide);
    run_ticks(&mut w, PadState::default(), 30);
    assert!(w.death_cam.is_none());
}
