//! Real baked-map and vehicle-class coverage for issues #4 and #7.
use core::sim::{buttons, Event, PadState, Sim};
use core::squad::CommandKind;
use core::unit::{Controller, UnitHandle};
use core::vehicle::{VehicleHandle, VehicleKind};
use game::walk::{ClassVisual, TeamTemplate, WalkSim};
use glam::Vec3;

const MAPS: [(&str, VehicleKind); 4] = [
    ("tat1", VehicleKind::Hover),
    ("bf1_nab1", VehicleKind::Walker),
    ("bf2_nab2", VehicleKind::Hover),
    ("bf2_geo1", VehicleKind::Walker),
];

fn maps_available() -> bool {
    let root = game::assets::baked_root();
    if MAPS
        .iter()
        .all(|(map, _)| !root.join(map).join("manifest.ron").exists())
    {
        eprintln!("SKIP: squad vehicle scenes need baked BF1/BF2 maps and missions");
        return false;
    }
    true
}

struct Scene {
    walk: WalkSim,
    vehicle: VehicleHandle,
    approach: Vec3,
    class: String,
}

fn scene(map: &str, kind: VehicleKind) -> Scene {
    scene_in(map, "con", kind, None)
}

/// `scene` on mission `{map}_{mission}`, optionally only vehicles of `only` class.
fn scene_in(map: &str, mission: &str, kind: VehicleKind, only: Option<&str>) -> Scene {
    let root = game::assets::baked_root();
    let dir = root.join(map);
    let (loaded, _) =
        game::assets::load_battle_scene(&dir, "con").expect("real baked map and common");
    let mission = game::mission::MissionDef::load(
        &root
            .parent()
            .unwrap()
            .join(format!("missions/{map}_{mission}.ron")),
    )
    .expect("real mission");
    let factions = mission.factions();
    let (rosters, warnings) = game::roster::FactionRoster::pair(
        &root.join("classes"),
        &loaded.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        Some(&mission),
    )
    .expect("fielded rosters");
    assert!(
        warnings
            .iter()
            .all(|w| w.starts_with("roster cis: skipped class cis_inf_droideka")),
        "{warnings:?}"
    );
    let mut sim = Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(&loaded.manifest.source_game);
    sim.battle = core::battle::Battle::new(mission.tickets());
    let mut names = Vec::new();
    let mut team = 0;
    let teams = rosters.map(|r| {
        let table: Vec<_> = r
            .classes
            .iter()
            .map(|c| c.register(&mut sim, &mut names))
            .collect();
        let template = TeamTemplate::from_roster(
            &r,
            &table[0],
            r.classes.iter().map(ClassVisual::from_roster).collect(),
        );
        sim.classes[team] = table;
        team += 1;
        template
    });
    let mut nav = game::layout::nav_graph(&loaded.manifest, &loaded.dir);
    mission.apply_arc_blocks(nav.as_mut());
    mission.apply_barriers(nav.as_mut());
    let mut walk = WalkSim::with_teams(sim, loaded.heightfield(), Vec3::ZERO, teams, nav);
    loaded.install_collision(&mut walk.sim);
    loaded.install_water(&mut walk.sim);
    let mut layout = game::layout::MapLayout::from_manifest(&loaded.manifest);
    walk.sim.death_regions = layout.apply_mission(&mission).death_regions;
    walk.add_posts(&layout);
    mission.apply_ai(&mut walk.sim);
    let installed = game::vehicle_pads::install(
        &mut walk.sim,
        &loaded.manifest,
        &dir,
        &root.join("classes"),
        &factions,
    )
    .expect("real vehicle pads");
    walk.step_tick(&[PadState::default()]);
    let (vehicle, approach) = walk
        .sim
        .vehicles
        .iter()
        .enumerate()
        .find_map(|(i, v)| {
            let setup = walk.sim.seat_setup.get(v.class as usize)?.as_ref()?;
            if !v.alive
                || only.is_some_and(|c| installed.classes[v.class as usize] != c)
                || v.kind != kind
                || v.team.is_some_and(|t| t != 0)
                || v.seats.len() < 2
                || !setup.boardable.get(1).copied().unwrap_or(false)
            {
                return None;
            }
            let pos = v.pos;
            // 9 m outside its radius first; a class-picked scene also takes
            // other rings and the PILEIN report's 8 m ring round the centre.
            let rings: &[f32] = if only.is_some() {
                &[
                    setup.radius + 9.0,
                    setup.radius + 6.0,
                    setup.radius + 12.0,
                    8.0,
                ]
            } else {
                &[setup.radius + 9.0]
            };
            rings.iter().find_map(|&ring| {
                (0..16).find_map(|k| {
                    let angle = k as f32 * std::f32::consts::TAU / 16.0;
                    let p = pos + Vec3::new(angle.sin(), 0.0, angle.cos()) * ring;
                    let p = Vec3::new(p.x, walk.sim.spawn_ground(p), p.z);
                    (p.y > -299.0
                        && (p.y - pos.y).abs() < 3.0
                        && (ring == 8.0 || walk.sim.floor_line(p, pos)))
                    .then_some((
                        VehicleHandle {
                            index: i as u32,
                            gen: v.gen,
                        },
                        p,
                    ))
                })
            })
        })
        .or_else(|| {
            for v in &walk.sim.vehicles {
                let p = v.pos + Vec3::X * 8.0;
                eprintln!(
                    "candidate {} kind={:?} team={:?} seats={} alive={} pos={:?} ground8={}",
                    installed.classes[v.class as usize],
                    v.kind,
                    v.team,
                    v.seats.len(),
                    v.alive,
                    v.pos,
                    walk.sim.spawn_ground(p)
                );
            }
            None
        })
        .expect("fielded friendly ground vehicle with a clear approach and secondary seat");
    let class = installed.classes[walk.sim.vehicle(vehicle).unwrap().class as usize].clone();
    let origin = walk.sim.vehicle(vehicle).unwrap().pos;
    walk.sim.unit_mut(walk.player).unwrap().soldier.pos = origin;
    eprintln!(
        "MAP {map} class={class} vehicle={vehicle:?} origin={origin:?} approach={approach:?}"
    );
    Scene {
        walk,
        vehicle,
        approach,
        class,
    }
}

impl Scene {
    fn bot(&mut self, pos: Vec3) -> UnitHandle {
        self.walk
            .sim
            .spawn(&self.walk.teams[0].spec(0, pos, 0.0, Controller::Ai, 0))
            .unwrap()
    }
}

fn pad(bit: u32) -> PadState {
    PadState {
        buttons: bit,
        ..Default::default()
    }
}

impl Scene {
    fn ordered(&mut self, follow: bool) -> UnitHandle {
        let h = self.bot(self.approach);
        let order = if follow {
            CommandKind::Follow(self.walk.player)
        } else {
            CommandKind::Hold(self.approach)
        };
        assert_eq!(self.walk.sim.issue_command(0, order), 1);
        self.walk.sim.unit_mut(h).unwrap().ai.next_think = self.walk.sim.tick + 9;
        assert!(
            self.walk
                .sim
                .enter_vehicle(self.walk.player, self.vehicle, 0),
            "{}",
            self.class
        );
        h
    }

    fn step(&mut self, bit: u32) {
        self.walk.step_tick(&[pad(bit)]);
    }
}

#[test]
fn player_calls_board_real_vehicles_and_resume_hold_or_follow() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS {
        for follow in [false, true] {
            let mut s = scene(map, kind);
            let bot = s.ordered(follow);
            let original = s.walk.sim.unit(bot).unwrap().command;
            let due = s.walk.sim.unit(bot).unwrap().ai.next_think;
            let start = s.walk.sim.unit(bot).unwrap().soldier.pos;
            let (mut commands, mut answers, mut entry) = (0, Vec::new(), None);
            let mut walked = 0.0f32;
            for tick in 0..300 {
                let now = s.walk.sim.tick;
                s.step(if tick < 3 { buttons::SQUAD_FOLLOW } else { 0 });
                for e in &s.walk.sim.events {
                    match e {
                        Event::Command {
                            kind: 4, units: 1, ..
                        } => commands += 1,
                        Event::OrderTaken { unit, did, .. } if *unit == bot => {
                            answers.push((now, *did))
                        }
                        _ => (),
                    }
                }
                let u = s.walk.sim.unit(bot).unwrap();
                assert_eq!(
                    u.command, original,
                    "{map} follow={follow}: retained command"
                );
                if let Some(m) = u.mount {
                    assert_eq!(m.vehicle, s.vehicle);
                    assert_ne!(m.seat, 0);
                    entry = Some(now);
                    break;
                }
                walked = walked.max(u.soldier.pos.distance(start));
            }
            assert!(walked > 1.0, "{map}: walking before entering the seat");
            eprintln!("BOARD {map} follow={follow} due={due} entry={entry:?} answers={answers:?}");
            assert_eq!(commands, 1, "held button has one edge");
            assert_eq!(answers, [(due, true)]);
            assert!(
                entry.is_some(),
                "{map}: physical boarding after a real approach"
            );
            assert!(s.walk.sim.unit(bot).unwrap().soldier.pos.distance(start) > 1.0);
            s.walk.sim.exit_vehicle(bot);
            s.walk.sim.exit_vehicle(s.walk.player);
            let exit = s.walk.sim.unit(bot).unwrap().soldier.pos;
            s.walk.sim.unit_mut(s.walk.player).unwrap().soldier.pos = s.approach;
            for _ in 0..90 {
                s.step(0);
                assert_eq!(s.walk.sim.unit(bot).unwrap().command, original);
            }
            let u = s.walk.sim.unit(bot).unwrap();
            assert!(u.mount.is_none());
            assert!(
                u.soldier.pos.distance(exit) > 0.5,
                "{map}: retained order resumes walking"
            );
        }
    }
}

#[test]
fn a_full_real_vehicle_refuses_without_losing_the_prior_order() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS {
        for follow in [false, true] {
            for late in [false, true] {
                let mut s = scene(map, kind);
                let bot = s.ordered(follow);
                let original = s.walk.sim.unit(bot).unwrap().command;
                if late {
                    s.step(buttons::SQUAD_FOLLOW);
                }
                let n = s.walk.sim.vehicle(s.vehicle).unwrap().seats.len();
                for k in 1..n {
                    let p = s
                        .walk
                        .sim
                        .spawn(&core::unit::UnitSpec::player_in_slot(
                            k as u8, 0, s.approach,
                        ))
                        .unwrap();
                    assert!(s.walk.sim.enter_vehicle(p, s.vehicle, k as u8));
                }
                let mut answers = Vec::new();
                for tick in 0..45 {
                    s.step(if tick == 0 { buttons::SQUAD_FOLLOW } else { 0 });
                    for e in &s.walk.sim.events {
                        if let Event::OrderTaken { unit, did, .. } = e {
                            if *unit == bot {
                                answers.push(*did);
                            }
                        }
                    }
                    let u = s.walk.sim.unit(bot).unwrap();
                    assert!(u.mount.is_none(), "{map}: occupied seats stay occupied");
                    assert_eq!(u.command, original, "{map} full follow={follow} now={} order={:?} cmd={:?} ride={:?} answers={answers:?}", s.walk.sim.tick, u.order, u.ai.cmd, u.ai.ride);
                    assert!(u
                        .ai
                        .ride
                        .is_none_or(|r| r.player_board.is_none() && r.board.is_none()));
                }
                assert_eq!(answers, [false]);
            }
        }
    }
}

#[test]
fn called_goals_end_on_driver_death_vehicle_destruction_or_a_new_order() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS {
        for follow in [false, true] {
            for active in [false, true] {
                for end in ["driver", "vehicle", "order"] {
                    let mut s = scene(map, kind);
                    let bot = s.ordered(follow);
                    let original = s.walk.sim.unit(bot).unwrap().command;
                    s.step(buttons::SQUAD_FOLLOW);
                    assert!(s.walk.sim.unit(bot).unwrap().order.is_some());
                    if active {
                        let due = s.walk.sim.unit(bot).unwrap().ai.next_think;
                        while s.walk.sim.tick <= due {
                            s.step(0);
                        }
                        let u = s.walk.sim.unit(bot).unwrap();
                        assert!(u.mount.is_none());
                        assert!(u.ai.ride.is_some_and(
                            |r| r.board == Some(s.vehicle) && r.player_board.is_some()
                        ));
                    }
                    let replacement = core::squad::Command {
                        kind: CommandKind::Hold(s.walk.sim.unit(bot).unwrap().soldier.pos),
                        issuer: 0,
                        issued_tick: s.walk.sim.tick,
                    };
                    match end {
                        "driver" => s
                            .walk
                            .sim
                            .kill_environment(s.walk.player, core::sim::DeathCause::Fall),
                        "vehicle" => s.walk.sim.destroy_vehicle(s.vehicle, None),
                        _ => {
                            assert_eq!(
                                s.walk.sim.issue_command(0, CommandKind::Hold(Vec3::ZERO)),
                                1
                            );
                        }
                    }
                    let expected = if s.walk.sim.unit(s.walk.player).is_none() {
                        None
                    } else if end == "vehicle" {
                        original
                    } else {
                        Some(replacement)
                    };
                    for _ in 0..45 {
                        s.step(0);
                        let u = s.walk.sim.unit(bot).unwrap();
                        assert_eq!(
                            u.command, expected,
                            "{map} {end} active={active} follow={follow}"
                        );
                        assert!(
                            u.ai.ride.is_none_or(|r| r.player_board.is_none()),
                            "{map} {end}: no stale explicit call"
                        );
                        assert!(u.order.is_none(), "{map} {end}: no stale pending delivery");
                    }
                }
            }
        }
    }
}

fn locomotion(sim: &Sim, pilot: UnitHandle) -> f32 {
    sim.unit(pilot).unwrap().ai.ride.map_or(0.0, |r| {
        r.sticks.map_or_else(
            || {
                r.hover
                    .map_or(0.0, |h| h.sticks[0].abs().max(h.sticks[1].abs()))
            },
            |x| x[0].abs().max(x[1].abs()),
        )
    })
}

fn waiting(map: &str, kind: VehicleKind, distance: f32) -> (Scene, UnitHandle) {
    let mut s = scene(map, kind);
    let pilot = s.bot(s.approach);
    let pos = s.walk.sim.vehicle(s.vehicle).unwrap().pos;
    // Public entry installs the real class's pickup state; no wait or claim injection.
    assert!(s.walk.sim.enter_vehicle(pilot, s.vehicle, 0));
    let human = (0..32)
        .find_map(|k| {
            let angle = k as f32 * std::f32::consts::TAU / 32.0;
            let p = pos + Vec3::new(angle.sin(), 0.0, angle.cos()) * distance;
            let p = Vec3::new(p.x, s.walk.sim.spawn_ground(p), p.z);
            (p.y > -299.0
                && !s.walk.sim.level_at(p).is_off()
                && !s
                    .walk
                    .sim
                    .death_regions
                    .iter()
                    .any(|r| r.contains(p + Vec3::Y))
                && s.walk.sim.collision.enclosing(p + Vec3::Y).is_none())
            .then_some(p)
        })
        .expect("a live player position at the requested range on the real map");
    s.walk.sim.unit_mut(s.walk.player).unwrap().soldier.pos = human;
    let target = s
        .walk
        .sim
        .cps
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.def
                .pos
                .distance_squared(pos)
                .total_cmp(&b.def.pos.distance_squared(pos))
        })
        .map(|(i, _)| i as u32)
        .unwrap();
    s.walk.sim.unit_mut(pilot).unwrap().ai.target_cp = Some(target);
    s.walk.sim.unit_mut(pilot).unwrap().ai.next_think = s.walk.sim.tick + 9;
    for _ in 0..12 {
        s.step(0);
    }
    let u = s.walk.sim.unit(pilot).unwrap();
    assert!(u
        .mount
        .is_some_and(|m| m.vehicle == s.vehicle && m.seat == 0));
    assert!(u.ai.ride.unwrap().pickup.is_some_and(|w| !w.complete));
    assert_eq!(locomotion(&s.walk.sim, pilot), 0.0);
    (s, pilot)
}

#[test]
fn move_out_and_at_ease_reach_real_waiting_pilots_once_per_press() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            for distance in [8.0, 90.0, 140.0] {
                let (mut s, pilot) = waiting(map, kind, distance);
                // Both games since issue #7 (BF1's rule on BF2 too).
                let eligible = distance < 135.0;
                let sent = s.walk.sim.tick;
                let start = s.walk.sim.vehicle(s.vehicle).unwrap().pos;
                let start_forward = s.walk.sim.vehicle(s.vehicle).unwrap().rot * Vec3::Z;
                let due = s.walk.sim.unit(pilot).unwrap().ai.next_think;
                let (mut commands, mut answers, mut released) = (0, Vec::new(), None);
                let mut drive = None;
                for tick in 0..30 {
                    let now = s.walk.sim.tick;
                    assert!(
                        s.walk.sim.unit(s.walk.player).is_some(),
                        "{map}: live issuer"
                    );
                    s.step(if tick < 3 { bit } else { 0 });
                    for e in &s.walk.sim.events {
                        match e {
                            Event::Command { .. } => commands += 1,
                            Event::OrderTaken { unit, .. } if *unit == pilot => answers.push(now),
                            _ => (),
                        }
                    }
                    if locomotion(&s.walk.sim, pilot) > 0.0 {
                        drive.get_or_insert(now);
                    }
                    if let Some(w) = s
                        .walk
                        .sim
                        .unit(pilot)
                        .and_then(|u| u.ai.ride)
                        .and_then(|r| r.pickup)
                    {
                        if w.released.is_some() {
                            released.get_or_insert(now);
                        }
                    }
                }
                eprintln!("POKE {map} bit={bit} distance={distance} sent={sent} due={due} answers={answers:?} release={released:?} drive={drive:?}");
                assert_eq!(commands, 1);
                if eligible {
                    assert_eq!(answers, [due]);
                    assert_eq!(released, Some(due));
                    assert!(released.is_some(), "{map}: shortened pickup window");
                    assert!(
                        drive.is_some(),
                        "{map}: actual driver locomotion after release"
                    );
                    let v = s.walk.sim.vehicle(s.vehicle).unwrap();
                    let displacement =
                        Vec3::new(v.pos.x - start.x, 0.0, v.pos.z - start.z).length();
                    let forward = v.rot * Vec3::Z;
                    let turn = (forward.x * start_forward.z - forward.z * start_forward.x)
                        .atan2(forward.x * start_forward.x + forward.z * start_forward.z)
                        .abs();
                    eprintln!("MOTION {map} bit={bit} distance={distance}: moved={displacement} turned={turn}");
                    // A walker turns toward its route before translating.
                    assert!(
                        displacement > 0.05 || (kind == VehicleKind::Walker && turn > 0.01),
                        "{map}: physical drive or turn after release"
                    );
                } else {
                    assert!(answers.is_empty());
                    assert_eq!(released, None);
                    assert_eq!(drive, None);
                }
            }
        }
    }
}

#[test]
fn pending_release_does_not_cross_real_pilot_or_vehicle_lifetimes() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            for end in ["pilot", "vehicle", "leave", "reboard", "change"] {
                let (mut s, pilot) = waiting(map, kind, 8.0);
                // Keep the receiver delayed until after the lifecycle transition.
                s.walk.sim.unit_mut(pilot).unwrap().ai.next_think = s.walk.sim.tick + 20;
                s.step(bit);
                assert!(s.walk.sim.unit(pilot).unwrap().order.is_some());
                match end {
                    "pilot" => s
                        .walk
                        .sim
                        .kill_environment(pilot, core::sim::DeathCause::Fall),
                    "vehicle" => s.walk.sim.destroy_vehicle(s.vehicle, None),
                    "leave" => s.walk.sim.exit_vehicle(pilot),
                    "reboard" => {
                        s.walk.sim.exit_vehicle(pilot);
                        assert!(s.walk.sim.enter_vehicle(pilot, s.vehicle, 0));
                    }
                    _ => {
                        let other = s
                            .walk
                            .sim
                            .vehicles
                            .iter()
                            .enumerate()
                            .find_map(|(i, v)| {
                                (v.alive
                                    && v.kind == kind
                                    && v.seats[0].occupant.is_none()
                                    && v.team.is_none_or(|t| t == 0)
                                    && i != s.vehicle.index as usize)
                                    .then_some(VehicleHandle {
                                        index: i as u32,
                                        gen: v.gen,
                                    })
                            })
                            .expect("another real ground vehicle");
                        s.walk.sim.exit_vehicle(pilot);
                        assert!(s.walk.sim.enter_vehicle(pilot, other, 0));
                    }
                }
                let mut answers = 0;
                for _ in 0..35 {
                    s.step(0);
                    answers += s
                        .walk
                        .sim
                        .events
                        .iter()
                        .filter(|e| matches!(e, Event::OrderTaken { unit, .. } if *unit == pilot))
                        .count();
                    if let Some(u) = s.walk.sim.unit(pilot) {
                        assert!(u.order.is_none());
                        assert!(
                            u.ai.ride
                                .and_then(|r| r.pickup)
                                .is_none_or(|w| w.released.is_none()),
                            "{map} {end}: release escaped its tenure"
                        );
                    }
                }
                assert_eq!(answers, 0, "{map} {end}: invalidated delivery is silent");
            }
        }
    }
}

#[test]
fn a_poked_pilot_keeps_a_naturally_approaching_boarders_seat() {
    if !maps_available() {
        return;
    }
    for (map, kind) in MAPS.into_iter().take(2) {
        for bit in [buttons::SQUAD_MOVE_OUT, buttons::SQUAD_AT_EASE] {
            let mut s = scene(map, kind);
            s.walk.sim.unit_mut(s.walk.player).unwrap().soldier.pos = s.approach;
            let bot = s.bot(s.approach);
            // The release issuer must not cancel this other player's retained Hold.
            s.walk
                .sim
                .spawn(&core::unit::UnitSpec::player_in_slot(
                    1,
                    0,
                    s.approach + Vec3::X * 3.0,
                ))
                .unwrap();
            assert_eq!(
                s.walk.sim.issue_command(1, CommandKind::Hold(s.approach)),
                1
            );
            let retained = s.walk.sim.unit(bot).unwrap().command;
            let pilot = s.bot(s.approach + Vec3::X);
            assert!(s.walk.sim.enter_vehicle(pilot, s.vehicle, 0));
            // Choose a successful salted roll, then require the real event.
            let call_tick = (s.walk.sim.tick..s.walk.sim.tick + 30)
                .find(|&t| core::ai::unit_unit(pilot.index, t, 0, core::ai::SALT_NEED_GUNNER) < 0.3)
                .unwrap();
            s.walk.sim.unit_mut(pilot).unwrap().ai.next_think = call_tick;
            s.walk.sim.unit_mut(bot).unwrap().ai.next_think = call_tick + 10;
            while s.walk.sim.tick <= call_tick {
                s.step(0);
            }
            assert!(s.walk.sim.events.iter().any(|e| matches!(e,
                Event::Call { unit, call: core::sim::Call::NeedGunner } if *unit == pilot)));
            assert_eq!(
                s.walk.sim.unit(bot).unwrap().ai.pending_board,
                Some((bot, s.vehicle))
            );
            let mut since = None;
            for _ in 0..15 {
                s.step(0);
                if let Some(r) = s
                    .walk
                    .sim
                    .unit(bot)
                    .unwrap()
                    .ai
                    .ride
                    .filter(|r| r.board == Some(s.vehicle))
                {
                    since = Some(r.since);
                    break;
                }
            }
            let since = since.expect("the call delivers a claim at the receiver's think");
            assert!(s.walk.sim.unit(bot).unwrap().mount.is_none());
            s.walk.sim.unit_mut(pilot).unwrap().ai.next_think = s.walk.sim.tick;
            let mut ack = None;
            let mut entry = None;
            let mut walked = 0.0f32;
            for tick in 0..180 {
                let now = s.walk.sim.tick;
                s.step(if tick == 0 { bit } else { 0 });
                if s.walk
                    .sim
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::OrderTaken { unit, .. } if *unit == pilot))
                {
                    ack = Some(now);
                }
                let u = s.walk.sim.unit(bot).unwrap();
                assert_eq!(u.command, retained);
                if u.mount
                    .is_some_and(|m| m.vehicle == s.vehicle && m.seat != 0)
                {
                    entry = Some(now);
                    break;
                }
                walked = walked.max(u.soldier.pos.distance(s.approach));
                let r = u.ai.ride.expect("approach remains maintained");
                assert_eq!(r.board, Some(s.vehicle));
                assert_eq!(r.since, since, "maintenance never restarts arbitration");
                assert!(r.maintenance.is_some());
                let wait = s
                    .walk
                    .sim
                    .unit(pilot)
                    .unwrap()
                    .ai
                    .ride
                    .unwrap()
                    .pickup
                    .unwrap();
                assert!(!wait.complete, "live approaching claim precedes release");
                assert_eq!(locomotion(&s.walk.sim, pilot), 0.0);
            }
            eprintln!(
                "CLAIM {map} bit={bit} since={since} ack={ack:?} entry={entry:?} walked={walked}"
            );
            assert!(ack.is_some());
            assert!(entry.is_some_and(|t| t > ack.unwrap()));
            assert!(walked > 0.5, "real walking before the boarding teleport");
        }
    }
}

/// Issue #4's real AATs: CIS hover tanks with one free seat once the human pilots.
const AATS: [&str; 2] = ["bf1_nab1", "bf2_nab2"];

/// A CIS AAT with `n` receivers on Hold (or Follow) beside it, an autonomous
/// claimant 32-39 m out (outside the order reach) that claimed it first, and
/// the human in the pilot seat.
fn contested(map: &str, n: usize, follow: bool) -> (Scene, Vec<UnitHandle>, UnitHandle) {
    let mut s = scene_in(map, "con_cw", VehicleKind::Hover, Some("cis_hover_aat"));
    let pos = s.walk.sim.vehicle(s.vehicle).unwrap().pos;
    let receivers: Vec<_> = (0..n)
        .map(|k| {
            let p = s.approach + Vec3::X * (k as f32 * 1.5);
            let p = Vec3::new(p.x, s.walk.sim.spawn_ground(p), p.z);
            s.bot(p)
        })
        .collect();
    let order = if follow {
        CommandKind::Follow(s.walk.player)
    } else {
        CommandKind::Hold(s.approach)
    };
    assert_eq!(s.walk.sim.issue_command(0, order), n as u8);
    let spot = [32.0f32, 35.0, 38.0]
        .into_iter()
        .flat_map(|d| (0..32).map(move |k| (d, k)))
        .find_map(|(d, k)| {
            let angle = k as f32 * std::f32::consts::TAU / 32.0;
            let p = pos + Vec3::new(angle.sin(), 0.0, angle.cos()) * d;
            let p = Vec3::new(p.x, s.walk.sim.spawn_ground(p), p.z);
            (p.y > -299.0
                && (p.y - pos.y).abs() < 4.0
                && !s.walk.sim.level_at(p).is_off()
                && !s
                    .walk
                    .sim
                    .death_regions
                    .iter()
                    .any(|r| r.contains(p + Vec3::Y))
                && s.walk.sim.collision.enclosing(p + Vec3::Y).is_none())
            .then_some(p)
        })
        .expect("a claimant spot 32-39 m from the real AAT");
    // Only this AAT within the claimant's search: no other vehicle near.
    let others: Vec<_> = s
        .walk
        .sim
        .alive_vehicles()
        .filter(|(h, v)| *h != s.vehicle && v.pos.distance(pos) < 120.0)
        .map(|(h, _)| h)
        .collect();
    for h in others {
        s.walk.sim.remove_vehicle(h);
    }
    let claimant = s.bot(spot);
    let target = s
        .walk
        .sim
        .cps
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.def
                .pos
                .distance_squared(pos)
                .total_cmp(&b.def.pos.distance_squared(pos))
        })
        .map(|(i, _)| i as u32)
        .unwrap();
    s.walk.sim.unit_mut(claimant).unwrap().ai.target_cp = Some(target);
    s.walk.sim.unit_mut(claimant).unwrap().ai.next_think = s.walk.sim.tick;
    // The claimant takes the empty vehicle by its own search.
    for _ in 0..30 {
        s.step(0);
        if s.walk
            .sim
            .unit(claimant)
            .unwrap()
            .ai
            .ride
            .is_some_and(|r| r.board == Some(s.vehicle))
        {
            break;
        }
    }
    let u = s.walk.sim.unit(claimant).unwrap();
    let r = u.ai.ride.unwrap_or_default();
    if r.board != Some(s.vehicle) {
        let cps: Vec<_> = s
            .walk
            .sim
            .cps
            .iter()
            .map(|c| (c.def.name.clone(), c.def.pos.distance(u.soldier.pos)))
            .collect();
        eprintln!(
            "NOCLAIM {map} pos={:?} target={:?} cps={cps:?} ride={:?} behaviour={:?} next_think={} tick={}",
            u.soldier.pos, u.ai.target_cp, u.ai.ride, u.ai.behaviour, u.ai.next_think, s.walk.sim.tick
        );
    }
    assert_eq!(r.board, Some(s.vehicle), "{map}: autonomous claim");
    assert!(r.called_board.is_none() && r.player_board.is_none());
    assert!(s.walk.sim.enter_vehicle(s.walk.player, s.vehicle, 0));
    eprintln!(
        "CLAIMANT {map} since={} distance={:.1}",
        r.since,
        spot.distance(pos)
    );
    (s, receivers, claimant)
}

#[test]
fn pile_in_outranks_an_older_autonomous_claim_on_real_aats() {
    if !maps_available() {
        return;
    }
    for map in AATS {
        let (mut s, receivers, claimant) = contested(map, 1, false);
        let bot = receivers[0];
        let mut entry = None;
        for tick in 0..60 {
            s.step(if tick == 0 { buttons::SQUAD_FOLLOW } else { 0 });
            if tick == 0 {
                // The older claim is still live when the order lands.
                let r = s.walk.sim.unit(claimant).unwrap().ai.ride;
                assert!(r.is_some_and(|r| r.board == Some(s.vehicle)), "{map}");
            }
            if let Some(m) = s.walk.sim.unit(bot).unwrap().mount {
                assert_eq!((m.vehicle, m.seat), (s.vehicle, 1));
                entry = Some(tick + 1);
                break;
            }
        }
        eprintln!("PILEIN {map} entry={entry:?}");
        assert!(entry.is_some(), "{map}: receiver boards within 2 s");
        assert!(s.walk.sim.unit(claimant).unwrap().mount.is_none());
    }
}

#[test]
fn pile_in_with_one_free_seat_boards_exactly_one_receiver() {
    if !maps_available() {
        return;
    }
    for map in AATS {
        for follow in [false, true] {
            // FOLLOW ME takes two followers (FOLLOW_CAP).
            for n in if follow { vec![2] } else { vec![2, 3] } {
                let (mut s, receivers, claimant) = contested(map, n, follow);
                let original: Vec<_> = receivers
                    .iter()
                    .map(|&h| s.walk.sim.unit(h).unwrap().command)
                    .collect();
                let mut entry = None;
                for tick in 0..90 {
                    s.step(if tick == 0 { buttons::SQUAD_FOLLOW } else { 0 });
                    let mounted = receivers
                        .iter()
                        .filter(|&&h| s.walk.sim.unit(h).unwrap().mount.is_some())
                        .count();
                    assert!(mounted <= 1, "{map} n={n}: one free seat");
                    if mounted == 1 {
                        entry.get_or_insert(tick + 1);
                    }
                    // A boarder may give up its Follow inside its leader's vehicle.
                    for (h, c) in receivers.iter().zip(&original) {
                        if s.walk.sim.unit(*h).unwrap().mount.is_some() {
                            continue;
                        }
                        assert_eq!(
                            s.walk.sim.unit(*h).unwrap().command,
                            *c,
                            "{map} n={n} follow={follow}: retained order"
                        );
                    }
                }
                eprintln!("PILEIN1 {map} follow={follow} n={n} entry={entry:?}");
                assert!(
                    entry.is_some_and(|t| t <= 60),
                    "{map} n={n}: one boards in 2 s"
                );
                assert!(s.walk.sim.unit(claimant).unwrap().mount.is_none());
                for h in &receivers {
                    let u = s.walk.sim.unit(*h).unwrap();
                    if u.mount.is_none() {
                        assert!(
                            u.ai.ride.is_none_or(|r| r.player_board.is_none()),
                            "{map}: no stale PILE IN claim"
                        );
                    }
                }
            }
        }
    }
}

/// Issue #35: a player's fusion cutter slices a real enemy AAT on NAB1 and
/// NAB2, the same on both: its meter fills by ShotDelay a shot (10 s, BF2's
/// AAT `TimeRequiredToEject`, which BF1's AAT takes), from BF2's cutter reach
/// (3.5 m off the hull's sphere: past the 3 m of BF1's disc), both riders
/// are put out, only the slicer may board for SLICE_RESERVE, and it boards.
#[test]
fn a_player_slices_a_real_enemy_aat() {
    if !maps_available() {
        return;
    }
    for map in AATS {
        let mut s = scene_in(map, "con_cw", VehicleKind::Hover, Some("cis_hover_aat"));
        let class = s.walk.sim.vehicle(s.vehicle).unwrap().class as usize;
        let time = s.walk.sim.slice_time[class];
        assert_eq!(time, 10.0, "{map}");
        // Two enemy riders: idle players, so nobody drives off.
        let riders: Vec<_> = (0..2u8)
            .map(|k| {
                let h = s
                    .walk
                    .sim
                    .spawn(&core::unit::UnitSpec::player_in_slot(k + 1, 1, s.approach))
                    .unwrap();
                assert!(s.walk.sim.seat_unit(h, s.vehicle, k));
                h
            })
            .collect();
        assert_eq!(s.walk.sim.vehicle(s.vehicle).unwrap().team, Some(1));
        let cutter = s
            .walk
            .sim
            .weapon_classes
            .iter()
            .position(|w| w.repair.is_some())
            .expect("the side's cutter is loaded");
        let (centre, radius) = s.walk.sim.vehicle_sphere(s.vehicle).unwrap();
        let mut away = s.approach - centre;
        away.y = 0.0;
        let mut at = centre + away.normalize() * (radius + 3.5);
        at.y = s.walk.sim.spawn_ground(at);
        let reach = &s.walk.sim.weapon_classes[cutter];
        assert_eq!(
            (
                reach.lock_on_range,
                reach.lock_on_angle.to_degrees().round()
            ),
            (4.0, 90.0),
            "{map}: the cutter's reach"
        );
        s.walk.sim.cheats.infinite_stamina = true;
        let player = s.walk.player;
        let health = s.walk.sim.vehicle(s.vehicle).unwrap().health.cur;
        let mut ejected = None;
        for tick in 0..20 * 30 {
            // The hull settles on its hover: keep the stand-off to it.
            let (centre, radius) = s.walk.sim.vehicle_sphere(s.vehicle).unwrap();
            let mut at = centre + away.normalize() * (radius + 3.5);
            at.y = s.walk.sim.spawn_ground(at);
            {
                let u = s.walk.sim.unit_mut(player).unwrap();
                u.weapon_class = cutter;
                u.soldier.pos = at;
                u.soldier.vel = Vec3::ZERO;
            }
            let eye = s.walk.sim.unit(player).unwrap().muzzle();
            let dir = (centre - eye).normalize();
            {
                let u = s.walk.sim.unit_mut(player).unwrap();
                u.soldier.yaw = (-dir.x).atan2(-dir.z);
                u.soldier.pitch = dir.y.clamp(-1.0, 1.0).asin();
            }
            s.step(buttons::FIRE);
            if riders
                .iter()
                .all(|&r| s.walk.sim.unit(r).unwrap().mount.is_none())
            {
                ejected = Some(tick);
                break;
            }
        }
        eprintln!("SLICE {map} time={time} ejected={ejected:?}");
        let shots = (time / 0.5).floor() as u64 + 1;
        assert_eq!(
            ejected,
            Some((shots - 1) * 15),
            "{map}: shot {shots} ejects"
        );
        let v = s.walk.sim.vehicle(s.vehicle).unwrap();
        assert_eq!(v.health.cur, health, "{map}: slicing does not repair");
        assert_eq!(v.slice_hold.map(|(h, _)| h), Some(player));
        for &r in &riders {
            let u = s.walk.sim.unit(r).unwrap();
            assert!(u.alive);
            assert!(!s.walk.sim.enter_vehicle(r, s.vehicle, 0), "{map}: held");
        }
        let mate = s.bot(at);
        assert!(!s.walk.sim.enter_vehicle(mate, s.vehicle, 1), "{map}: held");
        assert!(
            s.walk.sim.enter_vehicle(player, s.vehicle, 0),
            "{map}: the slicer boards"
        );
    }
}
