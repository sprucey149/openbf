use super::*;
use core::cp::CpDef;
use core::region::Region;
use core::sim::{buttons, DeathCause, PadState};
use core::unit::{Controller, UnitSpec};
use glam::Vec3;

/// BF2's own lines and names, as `bake locl` writes them.
fn texts() -> Texts {
    let mut entries: Vec<(u32, String)> = [KILLED, TEAM_KILLED, SUICIDE, DIED, CAPTURED, LOST]
        .iter()
        .map(|(h, t)| (*h, t.to_string()))
        .collect();
    for (key, text) in [
        ("common.sides.all.name", "Rebels"),
        ("common.sides.imp.name", "Empire"),
        ("common.sides.all.firstnames.01", "Ben"),
        ("common.sides.all.lastnames.01", "Pulastra"),
        ("level.hoth.cp5", "Echo Base"),
        ("weapons.imp.weap.inf_rifle", "Blaster Rifle"),
        ("game.awards.bonus.regulator", "Award: Flechette Shotgun"),
        ("ifs.sideselect.chooseteam", "Select a team"),
        ("entity.all.inf_soldier", "Rebel Soldier"),
        ("entity.imp.inf_rifleman", "Stormtrooper"),
        ("entity.imp.hover_fightertank", "Fighter Tank"),
    ] {
        entries.push((manifest::hud::locl_key(key), text.to_string()));
    }
    entries.sort_by_key(|(h, _)| *h);
    let table = HudLocl {
        language: "english".into(),
        entries,
    };
    Texts {
        bf2: Some(table.clone()),
        map: Some(table),
        bf1_map: false,
    }
}

/// The Empire's player (team 1) with an AI beside him, two Rebel AI
/// (team 0), and a post the Empire holds.
struct Scene {
    sim: Sim,
    me: UnitHandle,
    mate: UnitHandle,
    rebels: [UnitHandle; 2],
    feed: EventFeed,
}

fn scene() -> Scene {
    let mut sim = Sim::empty(3);
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    let me = sim
        .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::ZERO))
        .expect("player");
    let ai = |sim: &mut Sim, team: u8, x: f32| {
        sim.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, Vec3::new(x, 0.0, 0.0))
        })
        .expect("bot")
    };
    let mate = ai(&mut sim, 1, 2.0);
    let rebels = [ai(&mut sim, 0, 40.0), ai(&mut sim, 0, 42.0)];
    let (capture_time, neutralize_time) = CpDef::class_times(true);
    sim.add_cp(
        CpDef {
            name: "cp5".into(),
            pos: Vec3::new(100.0, 0.0, 0.0),
            capture: Region::sphere(Vec3::new(100.0, 0.0, 0.0), 8.0),
            control: None,
            value_bleed: 10,
            value_atk: [10; 2],
            value_def: [10; 2],
            spawn_nodes: vec![(Vec3::new(100.0, 0.0, 0.0), 0.0)],
            capture_time,
            neutralize_time,
        },
        Some(1),
    );
    let feed = EventFeed::new(texts(), ["all", "imp"], &["level.hoth.cp5".to_string()]);
    Scene {
        sim,
        me,
        mate,
        rebels,
        feed,
    }
}

/// One tick whose events are exactly `events`.
fn happen(s: &mut Scene, events: &[Event]) -> Vec<HudMessage> {
    s.sim.tick += 1;
    s.sim.events = events.to_vec();
    s.feed.tick(&s.sim, s.me);
    s.feed.take()
}

fn line(text: &str, color: [u8; 3]) -> HudMessage {
    HudMessage {
        text: text.into(),
        color,
    }
}

#[test]
fn droideka_target_shield_and_hidden_energy_feedback() {
    let mut s = scene();
    let h = s.rebels[0];
    let u = &mut s.sim.units[h.index as usize];
    let c = std::sync::Arc::new(core::droideka::DroidekaClass::default());
    u.droideka = Some(Box::new(core::droideka::Droideka::new(
        &c,
        core::droideka::Order::Deploy,
    )));
    u.spec.droideka = Some(c);
    let d = u.droideka.as_mut().unwrap();
    d.shield.on = true;
    d.shield.points = 750.0;
    s.sim.reticle_pick = vec![Some((AimTarget::Unit(h), -1))];
    assert_eq!(
        s.feed.target(&s.sim, 0).unwrap().shield,
        Some(("Shield".into(), 0.25))
    );
    s.sim.units[h.index as usize]
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .points = -900.0;
    assert_eq!(
        s.feed.target(&s.sim, 0).unwrap().shield,
        Some(("Shield".into(), 0.0))
    );
    s.sim.units[h.index as usize]
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .on = false;
    assert_eq!(s.feed.target(&s.sim, 0).unwrap().shield, None);
    s.sim.events = vec![Event::Droideka {
        unit: h,
        pos: Vec3::ZERO,
        cue: core::droideka::Cue::EnergyRefused,
    }];
    s.feed.tick(&s.sim, s.me);
    assert!(
        s.feed.take().is_empty(),
        "another player's refusal is his own"
    );
    s.sim.tick += 1;
    s.sim.events = vec![Event::Droideka {
        unit: s.me,
        pos: Vec3::ZERO,
        cue: core::droideka::Cue::EnergyRefused,
    }];
    s.feed.tick(&s.sim, s.me);
    assert_eq!(
        s.feed.take(),
        vec![line(
            "Shield charge spent — wait for recharge",
            message_color::OWN
        )]
    );
}

/// Card HTGT (`re-target-hud.md` §3-4): the reticle's pick by its
/// class's Locl name and health — a Rebel at 300 then 150 of 300 reads
/// 1.0 then 0.5, enemy; a team-mate a friend; no pick, no target.
#[test]
fn the_target_is_the_reticles_pick_by_class_name() {
    let mut s = scene();
    s.feed.classes = [
        vec![s
            .feed
            .texts
            .class_name("all_inf_soldier")
            .expect("entity key")],
        vec![s
            .feed
            .texts
            .class_name("imp_inf_rifleman")
            .expect("entity key")],
    ];
    let [r0, _] = s.rebels;
    s.sim.reticle_pick.resize(1, None);
    assert_eq!(s.feed.target(&s.sim, 0), None);
    let u = &mut s.sim.units[r0.index as usize];
    u.health.max = 300.0;
    u.health.cur = 300.0;
    s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
    let t = s.feed.target(&s.sim, 0).expect("picked");
    assert_eq!((t.name.as_str(), t.health_fraction), ("Rebel Soldier", 1.0));
    assert_eq!(t.color(), ui::hud::target_color::ENEMY);
    s.sim.units[r0.index as usize].health.cur = 150.0;
    assert_eq!(s.feed.target(&s.sim, 0).unwrap().health_fraction, 0.5);
    s.sim.reticle_pick[0] = Some((AimTarget::Unit(s.mate), 1));
    let t = s.feed.target(&s.sim, 0).unwrap();
    assert_eq!(t.name, "Stormtrooper");
    assert_eq!(t.color(), ui::hud::target_color::FRIEND);
    s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
    assert_ne!(
        t.id,
        s.feed.target(&s.sim, 0).unwrap().id,
        "one id per unit"
    );
}

/// A health droid under the reticle with no unit or vehicle picked reads
/// its class's name and health, neutral white; a nearer pick wins, a
/// destroyed droid or one out of the ray shows nothing.
#[test]
fn a_station_under_the_reticle_reads_its_health() {
    use core::sim::Station;
    let mut s = scene();
    s.sim.reticle_pick.resize(1, None);
    let me = s.sim.units.iter().find(|u| u.is_player).expect("player");
    let (eye, dir) = me.aim_ray(s.sim.ground_rules, s.sim.views());
    // Every station class has a name in the baked text, on both games'
    // maps ("Medical Droid", "Ammunition Droid", "Machinery Repair Droid").
    let root = crate::assets::baked_root();
    if root.join("common_bf2/locl/english.ron").exists() {
        for game in ["bf1", "bf2"] {
            let texts = Texts::load(&root, game);
            for c in [
                "com_item_healthrecharge",
                "com_item_weaponrecharge",
                "com_item_vehiclerecharge",
            ] {
                let name = texts.class_name(c);
                println!("{game} {c}: {name:?}");
                assert!(name.is_some(), "{game} {c}: no entity name");
            }
        }
    }
    s.feed.stations = vec!["Health Droid".into()];
    let mut st = Station::new(0, eye + dir * 10.0, 2.0);
    (st.sphere_centre, st.sphere_radius) = (eye + dir * 10.0, 0.8);
    st.health.cur = 40.0;
    s.sim.stations.push(st);
    let t = s.feed.target(&s.sim, 0).expect("the droid");
    assert_eq!(
        (t.name.as_str(), t.health_fraction, t.relation, t.team0),
        ("Health Droid", 0.5, 0, true)
    );
    assert_eq!(t.color(), ui::hud::target_color::NONE);
    // Off the ray: nothing.
    let side = dir.cross(glam::Vec3::Y).normalize() * 3.0;
    s.sim.stations[0].sphere_centre += side;
    assert_eq!(s.feed.target(&s.sim, 0), None);
    s.sim.stations[0].sphere_centre -= side;
    // A unit or vehicle pick comes first.
    let [r0, _] = s.rebels;
    s.sim.reticle_pick[0] = Some((AimTarget::Unit(r0), -1));
    assert_ne!(s.feed.target(&s.sim, 0).unwrap().id, t.id);
    s.sim.reticle_pick[0] = None;
    // A destroyed droid shows nothing with the rifle up; under a cutter it
    // reads its rebuild progress (`a_wreck_under_the_cutter_...`).
    s.sim.stations[0].destroyed = true;
    assert_eq!(s.feed.target(&s.sim, 0), None);
}

/// Issue #35: with the cutter up, a destroyed droid or a fallen turret
/// it is on reads its name and rebuild progress like a live target's
/// health; with the rifle up, or off the cutter, a wreck shows nothing.
#[test]
fn a_wreck_under_the_cutter_reads_its_rebuild_progress() {
    use core::sim::{BuildingTurret, Station, TurretFall};
    use core::vehicle::{VehicleHandle, VehicleSpec};
    use core::weapon::{HealthType, Repair, WeaponClass};
    let mut s = scene();
    s.sim.reticle_pick.resize(1, None);
    let mut health = [0.0; 8];
    (health[3], health[4], health[5], health[6]) = (200.0, 100.0, 50.0, 50.0);
    s.sim.weapon_classes.push(WeaponClass {
        name: "fusioncutter".into(),
        repair: Some(Repair { health }),
        target_team: 6,
        lock_on_range: 4.0,
        lock_on_angle: 90.0 * core::weapon::DEG,
        shot_delay: [0.5; 2],
        ..WeaponClass::default()
    });
    let cutter = s.sim.weapon_classes.len() - 1;
    let rifle = s.sim.unit(s.me).unwrap().weapon_class;
    let me = s.sim.unit(s.me).unwrap();
    let (eye, dir) = me.aim_ray(s.sim.ground_rules, s.sim.views());
    let at = eye + dir * 2.0;
    s.feed.stations = vec!["Health Droid".into()];
    let mut st = Station::new(0, at, 2.0);
    (st.sphere_centre, st.sphere_radius) = (at, 0.8);
    st.health.health_type = HealthType::Building;
    st.health.cur = st.health.max * 0.75;
    st.destroyed = true;
    s.sim.stations.push(st);
    // Rifle up: the wreck shows nothing (as before).
    assert_eq!(s.feed.target(&s.sim, 0), None);
    s.sim.unit_mut(s.me).unwrap().weapon_class = cutter;
    let t = s
        .feed
        .target(&s.sim, 0)
        .expect("the wreck under the cutter");
    assert_eq!(
        (t.name.as_str(), t.health_fraction, t.relation, t.team0),
        ("Health Droid", 0.75, 0, true)
    );
    // Off the cutter's cone: nothing.
    s.sim.stations[0].sphere_centre = eye - dir * 3.0;
    assert_eq!(s.feed.target(&s.sim, 0), None);
    s.sim.stations.clear();
    // A fallen turret on its pad: its class's name and health / MaxHealth.
    s.feed.vehicles = vec!["Laser Turret".into()];
    s.sim.building_turrets.push(BuildingTurret {
        spec: VehicleSpec {
            kind: VehicleKind::Turret,
            class: 0,
            home_team: None,
            neutral_when_empty: true,
            spawn_pad: None,
            pos: at,
            yaw: 0.0,
            max_health: 800.0,
            health_type: HealthType::Building,
            seats: vec![SeatKind::Gunner(0)],
        },
        rot: Quat::IDENTITY,
        aim_sphere: Some((glam::Vec3::ZERO, 0.8)),
        instance: 99,
        handle: VehicleHandle { index: 0, gen: 0 },
        fall: TurretFall::Pad,
        fallen_parts: Vec::new(),
        standing_parts: Vec::new(),
        down: Some(200.0),
        posed: None,
        self_pilots: Vec::new(),
        prefer_walkable_exit: false,
    });
    let t = s.feed.target(&s.sim, 0).expect("the fallen turret");
    assert_eq!(
        (t.name.as_str(), t.health_fraction, t.relation, t.team0),
        ("Laser Turret", 0.25, 0, true)
    );
    assert_eq!(t.color(), ui::hud::target_color::NONE);
    s.sim.unit_mut(s.me).unwrap().weapon_class = rifle;
    assert_eq!(s.feed.target(&s.sim, 0), None);
}

/// Card HTGT slice 3: a picked vehicle reads its class's Locl name and
/// its hull's health; empty it is a team-0 target (white), crewed by the
/// other team an enemy; its id is not a unit's.
#[test]
fn a_picked_vehicle_reads_its_class_name_and_hull() {
    use core::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    let mut s = scene();
    s.feed.vehicles = vec![s
        .feed
        .texts
        .class_name("imp_hover_fightertank")
        .expect("entity key")];
    let v = s.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(0.0, 0.0, -20.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    s.sim.reticle_pick.resize(1, None);
    s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), 0));
    let t = s.feed.target(&s.sim, 0).expect("picked");
    assert_eq!((t.name.as_str(), t.health_fraction), ("Fighter Tank", 1.0));
    assert!(t.team0);
    assert_eq!(t.color(), ui::hud::target_color::NONE);
    let veh = &mut s.sim.vehicles[v.index as usize];
    veh.health.cur = 250.0;
    veh.team = Some(1);
    s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), -1));
    let t = s.feed.target(&s.sim, 0).unwrap();
    assert_eq!(t.health_fraction, 0.25);
    assert!(!t.team0);
    assert_eq!(t.color(), ui::hud::target_color::ENEMY);
    let unit = s.sim.handle(v.index as usize);
    s.sim.reticle_pick[0] = Some((AimTarget::Unit(unit), -1));
    if let Some(u) = s.feed.target(&s.sim, 0) {
        assert_ne!(u.id, t.id, "a vehicle's id is not its index's unit's");
    }
    s.sim.vehicles[v.index as usize].alive = false;
    s.sim.reticle_pick[0] = Some((AimTarget::Vehicle(v), -1));
    assert_eq!(s.feed.target(&s.sim, 0), None, "a wreck reads nothing");
}

/// Card HATK (`re-hud-attacker.md` §3): a hit on the player lights the
/// edge toward the shooter in HIS frame - a Rebel at +X shooting him
/// (the shot travelling -X, a little down) is on his right facing -Z,
/// behind him facing -X, on his left facing +Z, and in front of him
/// (nothing) facing +X; another unit's hit lights nothing.
#[test]
fn a_hit_on_the_player_lights_the_edge_toward_the_shooter() {
    let mut s = scene();
    let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
    let from = s.sim.units[rebel.index as usize].soldier.pos + Vec3::Y * 1.6;
    let to = s.sim.units[me.index as usize].soldier.pos + Vec3::Y * 1.0;
    let hit = |victim| Event::Hit {
        direct: true,
        shooter: rebel,
        victim,
        damage: 10.0,
        age: 0.5,
        health_left: 100.0,
        dir: (to - from).normalize(),
    };
    let edges = |s: &mut Scene, yaw: f32, victim| {
        s.sim.units[me.index as usize].soldier.yaw = yaw;
        happen(s, &[hit(victim)]);
        s.feed.take_ambush()
    };
    use std::f32::consts::{FRAC_PI_2, PI};
    // Yaw 0 faces -Z (positive yaw turns left).
    assert_eq!(edges(&mut s, 0.0, me), [Edge::Right]);
    assert_eq!(edges(&mut s, FRAC_PI_2, me), [Edge::Bottom]);
    assert_eq!(edges(&mut s, PI, me), [Edge::Left]);
    assert_eq!(edges(&mut s, -FRAC_PI_2, me), []);
    assert_eq!(edges(&mut s, 0.0, mate), []);
    assert!(s.feed.take_ambush().is_empty(), "taken once");
}

/// Hitmark (`re-target-hud.md` §12): the player's critical hit with a
/// sniper rifle lights the mark for the PS2's 0.5 s hit timer, in the
/// victim's relation colour; a rifle's critical hit, another unit's, or
/// a plain body hit lights nothing.
/// [ours] (the maintainer, 2026-10-29): every hit the viewer deals an
/// enemy soldier, any weapon, blasts too, lights the plain hit flash
/// for the 0.5 s hit timer; a team-mate, a bot's hit and a hit taken
/// do not.
#[test]
fn an_enemy_body_hit_lights_the_plain_flash() {
    let mut s = scene();
    let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
    let hit = |shooter, victim, age| Event::Hit {
        direct: true,
        shooter,
        victim,
        damage: 50.0,
        age,
        health_left: 250.0,
        dir: Vec3::X,
    };
    happen(&mut s, &[hit(me, mate, 0.1)]);
    assert_eq!(s.feed.hit_body(s.sim.tick), None, "a team-mate");
    happen(&mut s, &[hit(rebel, me, 0.1)]);
    assert_eq!(s.feed.hit_body(s.sim.tick), None, "a hit taken");
    happen(&mut s, &[hit(rebel, mate, 0.1)]);
    assert_eq!(s.feed.hit_body(s.sim.tick), None, "a bot's hit");
    happen(&mut s, &[hit(me, rebel, 0.1)]);
    let at = s.sim.tick;
    assert_eq!(s.feed.hit_body(at), Some(ui::hud::target_color::ENEMY));
    assert!(s.feed.hit_body(at + HIT_TIMER_TICKS - 1).is_some());
    assert_eq!(s.feed.hit_body(at + HIT_TIMER_TICKS), None, "0.5 s");
    happen(&mut s, &[hit(me, rebel, core::sim::BLAST_AGE)]);
    assert!(s.feed.hit_body(s.sim.tick).is_some(), "a blast");
}

#[test]
fn a_hit_on_a_ship_or_a_ships_system_lights_the_flash_in_its_relation_colour() {
    use ui::hud::target_color as color;
    let mut s = scene();
    let (me, rebel) = (s.me, s.rebels[0]);
    let foe = s.sim.units[rebel.index as usize].team;
    let mine = s.sim.units[me.index as usize].team;
    let landed = |shooter, team| Event::HitLanded { shooter, team };
    happen(&mut s, &[landed(rebel, Some(mine))]);
    assert_eq!(s.feed.hit_body(s.sim.tick), None, "a bot's hit");
    happen(&mut s, &[landed(me, Some(foe))]);
    let at = s.sim.tick;
    assert_eq!(s.feed.hit_body(at), Some(color::ENEMY));
    assert_eq!(s.feed.hit_body(at + HIT_TIMER_TICKS), None, "0.5 s");
    happen(&mut s, &[landed(me, Some(mine))]);
    assert_eq!(s.feed.hit_body(s.sim.tick), Some(color::FRIEND));
    happen(&mut s, &[landed(me, None)]);
    assert_eq!(s.feed.hit_body(s.sim.tick), Some(color::NONE));
}

#[test]
fn a_sniper_headshot_lights_the_critical_mark() {
    let mut s = scene();
    let (me, mate, rebel) = (s.me, s.mate, s.rebels[0]);
    let sniper = core::weapon::WeaponClass {
        yaw_spread: 0.0,
        trigger_single: true,
        zoom_max: 6.0,
        ..core::weapon::WeaponClass::bf2_rifle()
    };
    assert!(!core::weapon::WeaponClass::bf2_rifle().ai_distance_table());
    s.sim.weapon_classes = vec![core::weapon::WeaponClass::bf2_rifle(), sniper];
    let (rifle, sniper) = (0, 1);
    let crit = |shooter, victim, weapon_class| Event::CriticalHit {
        shooter,
        victim: Target::Unit(victim),
        weapon_class,
    };
    let body = Event::Hit {
        direct: true,
        shooter: me,
        victim: rebel,
        damage: 50.0,
        age: 0.1,
        health_left: 250.0,
        dir: Vec3::X,
    };
    happen(&mut s, &[body]);
    assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a body hit");
    happen(&mut s, &[body, crit(me, rebel, rifle)]);
    assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a rifle headshot");
    happen(&mut s, &[crit(rebel, me, sniper)]);
    assert_eq!(s.feed.hit_critical(s.sim.tick), None, "a bot's headshot");
    happen(&mut s, &[body, crit(me, rebel, sniper)]);
    let at = s.sim.tick;
    assert_eq!(
        s.feed.hit_critical(at),
        Some(ui::hud::target_color::ENEMY),
        "a sniper headshot on an enemy"
    );
    assert!(s.feed.hit_critical(at + HIT_TIMER_TICKS - 1).is_some());
    assert_eq!(s.feed.hit_critical(at + HIT_TIMER_TICKS), None, "0.5 s");
    happen(&mut s, &[crit(me, mate, sniper)]);
    assert_eq!(
        s.feed.hit_critical(s.sim.tick),
        Some(ui::hud::target_color::FRIEND),
        "a team-mate's head: blue"
    );
}

/// Hitmark: the player's rocket launcher on a vehicle's weak spot lights
/// the mark in the vehicle's relation colour (white for an empty one);
/// a sniper rifle or a rifle on the weak spot, the EMP launcher, a
/// rocket launcher on a soldier's head, or a bot's rocket on the weak
/// spot lights nothing.
#[test]
fn a_rocket_on_a_weak_spot_lights_the_critical_mark() {
    use core::vehicle::{VehicleKind, VehicleSpec};
    use core::weapon::WeaponClass;
    let mut s = scene();
    let (me, rebel) = (s.me, s.rebels[0]);
    let my_team = s.sim.units[me.index as usize].team;
    let spec = |team| VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: Some(team),
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![core::vehicle::SeatKind::Pilot],
    };
    let theirs = s.sim.spawn_vehicle(&spec(1 - my_team));
    let ours = s.sim.spawn_vehicle(&spec(my_team));
    let empty = s.sim.spawn_vehicle(&VehicleSpec {
        neutral_when_empty: true,
        ..spec(my_team)
    });
    let named = |name: &str, medals| WeaponClass {
        name: name.into(),
        medals_type: medals,
        ..WeaponClass::bf2_rifle()
    };
    let sniper = WeaponClass {
        yaw_spread: 0.0,
        trigger_single: true,
        zoom_max: 6.0,
        ..named("com_weap_inf_sniper_rifle", Some(4))
    };
    s.sim.weapon_classes = vec![
        named("com_weap_inf_rifle", Some(1)),
        sniper,
        named("rep_weap_inf_rocket_launcher", Some(2)),
        named("rep_weap_inf_emp_launcher", Some(-1)),
        named("all_weap_inf_torpedo_launcher", None),
    ];
    let (rifle, sniper, rocket, emp, bf1_rocket) = (0, 1, 2, 3, 4);
    let crit = |shooter, victim, weapon_class| Event::CriticalHit {
        shooter,
        victim,
        weapon_class,
    };
    let mark = |s: &mut Scene, e: Event| {
        s.sim.tick += HIT_TIMER_TICKS;
        happen(s, &[e]);
        s.feed.hit_critical(s.sim.tick)
    };
    let enemy = Some(ui::hud::target_color::ENEMY);
    let v = Target::Vehicle(theirs);
    assert_eq!(mark(&mut s, crit(me, v, rocket)), enemy, "a rocket");
    assert_eq!(mark(&mut s, crit(me, v, bf1_rocket)), enemy, "BF1's");
    assert_eq!(
        mark(&mut s, crit(me, Target::Vehicle(ours), rocket)),
        Some(ui::hud::target_color::FRIEND),
        "our own side's: blue"
    );
    assert_eq!(
        mark(&mut s, crit(me, Target::Vehicle(empty), rocket)),
        Some(ui::hud::target_color::NONE),
        "an empty one, of no side: white"
    );
    assert_eq!(mark(&mut s, crit(me, v, rifle)), None, "a rifle");
    assert_eq!(mark(&mut s, crit(me, v, sniper)), None, "a sniper rifle");
    assert_eq!(mark(&mut s, crit(me, v, emp)), None, "the EMP launcher");
    assert_eq!(mark(&mut s, crit(rebel, v, rocket)), None, "a bot's");
    assert_eq!(
        mark(&mut s, crit(me, Target::Unit(rebel), rocket)),
        None,
        "a rocket on a head"
    );
}

/// A hit on the hover he pilots lights the edge in the HULL's frame (its
/// left +X, forward +Z); a gunner's seat, a collision, or a flyer light
/// none (BF2's callers: soldier, hover, walker).
#[test]
fn a_hit_on_his_hover_lights_the_hulls_edge() {
    use core::vehicle::{Mount, SeatKind, VehicleKind, VehicleSpec};
    let mut s = scene();
    let me = s.me;
    let spec = |kind, yaw| VehicleSpec {
        kind,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Vehicle,
        // A building turret has gunner seats only.
        seats: if kind == VehicleKind::Turret {
            vec![SeatKind::Gunner(0)]
        } else {
            vec![SeatKind::Pilot, SeatKind::Gunner(0)]
        },
    };
    let mut edges = |kind, yaw, seat, dir: Option<Vec3>| {
        let v = s.sim.spawn_vehicle(&spec(kind, yaw));
        s.sim.units[me.index as usize].mount = Some(Mount { vehicle: v, seat });
        // The soldier's own facing must not matter.
        s.sim.units[me.index as usize].soldier.yaw = 1.0;
        let rot = s.sim.vehicle(v).unwrap().rot;
        let ev = Event::VehicleHit {
            shooter: Some(s.rebels[0]),
            vehicle: v,
            damage: 10.0,
            dir: dir.map(|d| rot * d),
        };
        happen(&mut s, &[ev]);
        s.feed.take_ambush()
    };
    // Travelling toward the hull's right (-X): from its left.
    let from_left = Some(Vec3::new(-1.0, -0.1, 0.0).normalize());
    assert_eq!(edges(VehicleKind::Hover, 0.0, 0, from_left), [Edge::Left]);
    assert_eq!(edges(VehicleKind::Walker, 2.0, 0, from_left), [Edge::Left]);
    let from_behind = Some(Vec3::new(0.0, -0.1, 1.0).normalize());
    assert_eq!(
        edges(VehicleKind::Hover, 1.0, 0, from_behind),
        [Edge::Bottom]
    );
    assert_eq!(edges(VehicleKind::Hover, 0.0, 1, from_left), []);
    assert_eq!(edges(VehicleKind::Hover, 0.0, 0, None), []);
    assert_eq!(edges(VehicleKind::Flyer, 0.0, 0, from_left), []);
    assert_eq!(edges(VehicleKind::Turret, 0.0, 0, from_left), []);
}

#[test]
fn hero_victim_uses_disc_points_and_hero_message_even_after_death() {
    let mut s = scene();
    let victim = s.rebels[0];
    s.sim.heroes = Some(core::sim::heroes::Heroes::new([Some(0), None]));
    s.feed.classes[0] = vec!["Luke Skywalker".into()];
    s.sim.units[victim.index as usize].alive = false;
    let me = s.me;
    assert_eq!(
        happen(
            &mut s,
            &[Event::Kill {
                shooter: me,
                victim
            }]
        ),
        [line("Player 1 defeated Luke Skywalker", [200, 100, 100])]
    );
    assert_eq!(s.feed.stats(&s.sim, me).points, points::KILL_HERO);
    assert_eq!(
        s.feed.round().units[me.index as usize].points,
        points::KILL_HERO
    );
}

#[test]
fn hero_earned_points_are_displayed_separately_and_do_not_rank_future_offers() {
    let mut s = scene();
    s.sim.heroes = Some(core::sim::heroes::Heroes::new([None, Some(0)]));
    let me = s.me;
    let victim = s.rebels[0];
    happen(
        &mut s,
        &[Event::Kill {
            shooter: me,
            victim,
        }],
    );
    let stats = s.feed.stats(&s.sim, me);
    assert_eq!((stats.points, stats.hero_points), (1, 1));
    assert_eq!(s.feed.round().units[me.index as usize].ordinary_points(), 0);
    crate::heroes::scores(&mut s.sim, &s.feed);
    assert_eq!(s.sim.heroes.as_ref().unwrap().points[me.index as usize], 0);
}

/// Card EVMSG's gate (`re-event-messages.md` §1.1): only a human's death
/// makes a line, coloured by the victim's team as the viewer sees it;
/// the player's own kill is his statistics.
#[test]
fn only_a_humans_death_makes_a_line() {
    let mut s = scene();
    let [r0, r1] = s.rebels;
    let (me, mate) = (s.me, s.mate);
    // AI kills AI, either way round: nothing.
    let kill = |shooter, victim| Event::Kill { shooter, victim };
    assert!(happen(&mut s, &[kill(r0, mate), kill(mate, r1)]).is_empty());
    assert_eq!(s.feed.stats(&s.sim, me), HudStats::default());

    // An AI kills the human: the line, in the viewer's own-team colour.
    assert_eq!(
        happen(&mut s, &[kill(r0, me)]),
        [line("Ben Pulastra killed Player 1", [1, 86, 213])]
    );
    // The Empire has no name lists: two letters and a number.
    let name = s.feed.unit_name(&s.sim, mate);
    let b = name.as_bytes();
    assert!(
        name.len() == 6
            && b[..2].iter().all(u8::is_ascii_uppercase)
            && b[2] == b' '
            && b[3..].iter().all(u8::is_ascii_digit),
        "{name:?}"
    );

    // The human kills an ordinary AI: no line, kills +1 and a point.
    assert!(happen(&mut s, &[kill(me, r1)]).is_empty());
    let st = s.feed.stats(&s.sim, me);
    assert_eq!((st.kills, st.points), (1, points::KILL_AI));

    // A human suicide: the line, kills −1, a point lost.
    assert_eq!(
        happen(&mut s, &[kill(me, me)]),
        [line("Player 1 killed himself", [1, 86, 213])]
    );
    let st = s.feed.stats(&s.sim, me);
    assert_eq!((st.kills, st.points), (0, 0));

    // A teammate killed by the human: no line (the victim is an AI),
    // kills −1, two points lost.
    assert!(happen(&mut s, &[kill(me, mate)]).is_empty());
    let st = s.feed.stats(&s.sim, me);
    assert_eq!((st.kills, st.points), (-1, -2));

    // Dying with no killer: "died", nobody's grey, a point lost.
    let fell = Event::EnvDeath {
        unit: me,
        cause: DeathCause::Fall,
    };
    assert_eq!(
        happen(&mut s, &[fell]),
        [line("Player 1 died", [192, 192, 192])]
    );
    assert_eq!(s.feed.stats(&s.sim, me).points, -3);
    // An AI's fall: nothing.
    let fell = Event::EnvDeath {
        unit: r0,
        cause: DeathCause::Fall,
    };
    assert!(happen(&mut s, &[fell]).is_empty());
}

/// `[ours]` post lines in BF2's own strings, coloured by the team that
/// captured or lost the post; standing in the region longest earns the
/// capture's 5 points, standing in it at all its 2.
#[test]
fn post_lines_name_the_team_and_the_post() {
    let mut s = scene();
    let me = s.me;
    // The Rebels take the Empire's post: lost (the viewer's team, blue),
    // then captured (the enemy, red).
    assert_eq!(
        happen(&mut s, &[Event::Neutralized { cp: 0, by: 0 }]),
        [line("Empire lost the Echo Base", [1, 86, 213])]
    );
    s.sim.cps[0].owner = None;
    assert_eq!(
        happen(&mut s, &[Event::Captured { cp: 0, team: 0 }]),
        [line("Rebels captured the Echo Base", [200, 100, 100])]
    );
    assert_eq!(s.feed.stats(&s.sim, me).captures, 0);

    // The Empire takes it back with the player in the region first and
    // a teammate arriving later: his capture, 5 points.
    s.sim.cps[0].owner = Some(0);
    let at_post = Vec3::new(100.0, 0.0, 0.0);
    s.sim.units[me.index as usize].soldier.pos = at_post;
    assert!(happen(&mut s, &[]).is_empty());
    let mate = s.mate;
    s.sim.units[mate.index as usize].soldier.pos = at_post;
    assert_eq!(
        happen(&mut s, &[Event::Neutralized { cp: 0, by: 1 }]),
        [line("Rebels lost the Echo Base", [200, 100, 100])]
    );
    assert_eq!(
        happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]),
        [line("Empire captured the Echo Base", [1, 86, 213])]
    );
    let st = s.feed.stats(&s.sim, me);
    assert_eq!((st.captures, st.points), (1, points::CAPTURE));

    // Arriving second is an assist.
    s.sim.units[me.index as usize].soldier.pos = Vec3::ZERO;
    assert!(happen(&mut s, &[]).is_empty());
    s.sim.units[me.index as usize].soldier.pos = at_post;
    for _ in 0..SAME_LINE_TICKS {
        assert!(happen(&mut s, &[]).is_empty());
    }
    happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
    let st = s.feed.stats(&s.sim, me);
    assert_eq!(
        (st.captures, st.points),
        (2, points::CAPTURE + points::CAPTURE_ASSIST)
    );
}

/// CAPCR: being counted on the completing tick still earns an assist,
/// but full credit requires positive residence (re-event-messages §9.1).
#[test]
fn capture_arrival_on_completing_tick_is_an_assist() {
    let mut s = scene();
    let me = s.me;
    s.sim.units[me.index as usize].soldier.pos = s.sim.cps[0].def.pos;
    happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
    let stats = s.feed.stats(&s.sim, me);
    assert_eq!((stats.captures, stats.points), (1, points::CAPTURE_ASSIST));
    s.feed.tick(&s.sim, me);
    assert!(s.feed.take().is_empty());
    assert_eq!(s.feed.stats(&s.sim, me), stats);
}

#[test]
fn capture_prior_tick_residence_retains_full_credit_and_ties() {
    // No teammate present, or a teammate entering on the same prior tick:
    // neither has longer residence than the viewer.
    for tied_teammate in [false, true] {
        let mut s = scene();
        let me = s.me;
        let at_post = s.sim.cps[0].def.pos;
        s.sim.units[me.index as usize].soldier.pos = at_post;
        if tied_teammate {
            s.sim.units[s.mate.index as usize].soldier.pos = at_post;
        }
        happen(&mut s, &[]);
        happen(&mut s, &[Event::Captured { cp: 0, team: 1 }]);
        let stats = s.feed.stats(&s.sim, me);
        assert_eq!((stats.captures, stats.points), (1, points::CAPTURE));
    }
}

#[test]
fn capture_sim_step_arrival_on_completing_tick_is_an_assist() {
    let mut s = scene();
    let (me, mate) = (s.me, s.mate);
    for unit in &mut s.sim.units {
        unit.controller = Controller::Idle;
    }
    s.sim.cps[0].owner = None;
    let at_post = s.sim.cps[0].def.pos;
    s.sim.units[mate.index as usize].soldier.pos = at_post;
    let limit = s.sim.cps[0].def.capture_time;
    for _ in 0..1000 {
        if s.sim.cps[0].capture + core::TICK_DT > limit {
            break;
        }
        s.sim.step(&[]);
        s.feed.tick(&s.sim, me);
        s.feed.take();
        assert_eq!(s.sim.cps[0].owner, None);
    }
    assert!(s.sim.cps[0].capture + core::TICK_DT > limit);
    s.sim.units[mate.index as usize].soldier.pos = Vec3::ZERO;
    s.sim.units[me.index as usize].soldier.pos = at_post;
    s.sim.step(&[]);
    assert!(s
        .sim
        .events
        .iter()
        .any(|event| matches!(event, Event::Captured { cp: 0, team: 1 })));
    s.feed.tick(&s.sim, me);
    let stats = s.feed.stats(&s.sim, me);
    assert_eq!((stats.captures, stats.points), (1, points::CAPTURE_ASSIST));
    s.feed.tick(&s.sim, me);
    assert_eq!(s.feed.stats(&s.sim, me), stats);
}

/// A post with no name on the disc is the PS2's "Command Post", and a
/// BF1 name is set in BF2's case.
#[test]
fn post_and_team_names_fall_back() {
    let t = texts();
    assert_eq!(t.post_name("level.hoth.cp5"), "Echo Base");
    assert_eq!(t.post_name("level.hoth.cp9"), "Command Post");
    assert_eq!(t.team_name("gun"), "GUN");
    assert_eq!(title_case("PIER ONE"), "Pier One");
    assert_eq!(title_case("WESTERN LZ"), "Western LZ");
    assert_eq!(
        fill("%s lost the %s", &["Empire", "Bunker"]),
        "Empire lost the Bunker"
    );
    // A side's lists from its keys; a side with none composes letters
    // and digits.
    let (first, last) = t.name_lists("all");
    assert_eq!((first, last), (vec!["Ben".into()], vec!["Pulastra".into()]));
    let feed = EventFeed::new(t.clone(), ["imp", "all"], &[]);
    assert_eq!(feed.teams[0].1[..2], ["DX 178", "KL 987"]);
    assert_eq!(feed.teams[1].1[0], "Ben Pulastra");
}

fn pad(buttons: u32) -> PadState {
    PadState {
        buttons,
        ..PadState::default()
    }
}

/// A battle of the player and `mates` bots of his team in a 5 m ring.
fn squad_scene(mates: usize) -> (Sim, UnitHandle, EventFeed) {
    let mut s = scene();
    for h in s.rebels {
        s.sim.units[h.index as usize].alive = false;
    }
    s.sim.units[s.mate.index as usize].alive = false;
    for k in 0..mates {
        let a = k as f32 / mates as f32 * std::f32::consts::TAU;
        s.sim
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(1, Vec3::new(5.0 * a.cos(), 0.0, 5.0 * a.sin()))
            })
            .expect("bot");
    }
    (s.sim, s.me, s.feed)
}

/// Press `button`, run a second of ticks (every bot has thought), and
/// answer the cross then.
fn order(sim: &mut Sim, me: UnitHandle, feed: &mut EventFeed, button: u32) -> SquadCross {
    sim.step(&[pad(0)]);
    feed.tick(sim, me);
    sim.step(&[pad(button)]);
    feed.tick(sim, me);
    for _ in 0..core::TICK_HZ {
        sim.step(&[pad(0)]);
        feed.tick(sim, me);
    }
    assert!(feed.take().is_empty(), "an order prints no line");
    feed.squad(sim.tick).expect("the cross is up a second on")
}

/// Card SQHUD: the press shows the cross — its arm, the PS2's 2 s timer
/// on the sim's clock — and prints no line, however many render frames
/// look at the tick.
#[test]
fn an_order_shows_the_cross_and_no_line() {
    let (mut sim, me, mut feed) = squad_scene(0);
    feed.tick(&sim, me);
    assert!(feed.squad(sim.tick).is_none());
    sim.step(&[pad(buttons::SQUAD_FOLLOW)]);
    for _ in 0..3 {
        feed.tick(&sim, me);
    }
    assert!(feed.take().is_empty());
    let at = sim.tick;
    let hz = u64::from(core::TICK_HZ);
    let c = feed.squad(at).expect("shown at the press");
    assert_eq!((c.arm, c.timer, c.count), (hud_squad::RIGHT, 1.0, None));
    assert_eq!(
        c.labels,
        ["MOVE OUT", "HOLD POSITION", "FOLLOW ME", "AT EASE"]
    );
    assert_eq!(feed.squad(at + hz).map(|c| c.timer), Some(0.5));
    assert_eq!(feed.squad(at + 2 * hz).map(|c| c.timer), Some(0.0));
    assert!(feed.squad(at + 2 * hz + 1).is_none(), "gone after 2 s");
    // Each order has its own arm.
    for (button, arm) in [
        (buttons::SQUAD_HOLD, hud_squad::DOWN),
        (buttons::SQUAD_AT_EASE, hud_squad::LEFT),
        (buttons::SQUAD_MOVE_OUT, hud_squad::UP),
    ] {
        assert_eq!(order(&mut sim, me, &mut feed, button).arm, arm);
    }
}

/// Card SQHUD's count (the maintainer, INBOX 2026-09-30): the units the order
/// changed. FOLLOW ME with five in reach shows 2, HOLD POSITION with
/// three shows 3, a release the number released, nobody no number.
#[test]
fn the_cross_counts_the_units_that_took_the_order() {
    let (mut sim, me, mut feed) = squad_scene(5);
    let c = order(&mut sim, me, &mut feed, buttons::SQUAD_FOLLOW);
    assert_eq!(c.count, Some(2), "FOLLOW ME, five in reach");
    let c = order(&mut sim, me, &mut feed, buttons::SQUAD_MOVE_OUT);
    assert_eq!(c.count, Some(2), "MOVE OUT releases the two followers");
    let c = order(&mut sim, me, &mut feed, buttons::SQUAD_AT_EASE);
    assert_eq!(c.count, None, "nobody left to release");

    let (mut sim, me, mut feed) = squad_scene(3);
    let c = order(&mut sim, me, &mut feed, buttons::SQUAD_HOLD);
    assert_eq!(c.count, Some(3), "HOLD POSITION, three in reach");

    let (mut sim, me, mut feed) = squad_scene(0);
    for button in [buttons::SQUAD_FOLLOW, buttons::SQUAD_HOLD] {
        let c = order(&mut sim, me, &mut feed, button);
        assert_eq!(c.count, None, "nobody in reach");
    }
}

/// The spawn screens' texts (card SSELB): a Lua key path, a hash the
/// ELF names, a weapon class by its `weapons.<side>.weap.<rest>` key,
/// each falling back to the disc's English where the table lacks it.
#[test]
fn the_spawn_screens_read_the_table() {
    let feed = EventFeed::new(texts(), ["all", "imp"], &[]);
    assert_eq!(
        feed.screen_text("ifs.sideselect.chooseteam", "x"),
        "Select a team"
    );
    assert_eq!(
        feed.screen_text("ifs.sideselect.autoassign", "Auto Assign"),
        "Auto Assign"
    );
    assert_eq!(
        feed.weapon_name("imp_weap_inf_rifle").as_deref(),
        Some("Blaster Rifle")
    );
    assert_eq!(feed.weapon_name("imp_weap_inf_pistol"), None);
    assert_eq!(feed.weapon_name("norifle"), None);
    assert_eq!(
        feed.text_by_hash(manifest::hud::locl_key("level.hoth.cp5"), "x"),
        "Echo Base"
    );
    assert_eq!(
        feed.text_by_hash(0xecf8_496f, "Unit Count: %d"),
        "Unit Count: %d"
    );
}

/// Our own award, the Heavy Blast Cannon, has our own name and earned line,
/// which no disc has; BF2's awards keep BF2's (`game.awards.bonus.<medal>`).
#[test]
fn the_heavy_blast_cannon_has_its_own_name_and_line() {
    let feed = EventFeed::new(texts(), ["all", "imp"], &[]);
    for class in ["all_weap_award_blast_cannon", "imp_weap_award_blast_cannon"] {
        assert_eq!(
            feed.weapon_name(class).as_deref(),
            Some("AWARD Blast Cannon"),
            "{class}"
        );
        assert_eq!(
            feed.award_bonus("regulator", Some(class)).as_deref(),
            Some("Award: Heavy Blast Cannon"),
            "{class}"
        );
    }
    assert_eq!(
        feed.award_bonus("regulator", Some("imp_weap_award_shotgun"))
            .as_deref(),
        Some("Award: Flechette Shotgun")
    );
    assert_eq!(
        feed.award_bonus("regulator", None).as_deref(),
        Some("Award: Flechette Shotgun")
    );
    assert_eq!(feed.award_bonus("warhero", None), None);
    assert_eq!(feed.weapon_name("imp_weap_award_shotgun"), None);
    assert_eq!(feed.weapon_name("geo_weap_award_blast_cannon"), None);
}

#[test]
fn a_pilot_flashes_when_their_hull_is_damaged_and_a_gunner_does_not() {
    use core::vehicle::{Mount, SeatKind, VehicleKind, VehicleSpec};
    let mut s = scene();
    let me = s.me;
    let v = s.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Flyer,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    s.sim.vehicles[v.index as usize].health.cur = 100.0;
    let hit = Event::VehicleHit {
        shooter: None,
        vehicle: v,
        damage: 20.0,
        dir: None,
    };
    s.sim.units[me.index as usize].mount = Some(Mount {
        vehicle: v,
        seat: 1,
    });
    happen(&mut s, &[hit]);
    assert_eq!(s.feed.damage_flash(s.sim.tick, me), 0.0);
    s.sim.units[me.index as usize].mount = Some(Mount {
        vehicle: v,
        seat: 0,
    });
    happen(&mut s, &[hit]);
    assert_eq!(s.feed.damage_flash(s.sim.tick, me), 128.0 / 255.0);
}

#[test]
fn local_damage_flash_filters_rearms_expires_and_cannot_follow_another_life() {
    let mut s = scene();
    let (me, bot, attacker) = (s.me, s.mate, s.rebels[0]);
    let hit = |victim, damage, health_left| Event::Hit {
        direct: false,
        shooter: attacker,
        victim,
        damage,
        health_left,
        age: core::sim::BLAST_AGE,
        dir: Vec3::X,
    };
    happen(
        &mut s,
        &[
            hit(bot, 20.0, 100.0),
            hit(me, 2.0, 100.0),
            hit(me, 100.0, 0.0),
        ],
    );
    assert_eq!(s.feed.damage_flash(s.sim.tick, me), 0.0);
    happen(&mut s, &[hit(me, 20.0, 100.0)]);
    let start = s.sim.tick;
    assert_eq!(s.feed.damage_flash(start, me), 128.0 / 255.0);
    let before = s.sim.hash();
    s.feed.tick(&s.sim, me);
    assert_eq!(s.sim.hash(), before);
    assert!((s.feed.damage_flash(start + 3, me) - (128.0 / 255.0) * (2.0 / 3.0)).abs() < 1e-6);
    assert_eq!(s.feed.damage_flash(start + 9, me), 0.0);
    assert_eq!(
        s.feed.damage_flash(
            start,
            core::unit::UnitHandle {
                gen: me.gen + 1,
                ..me
            }
        ),
        0.0
    );
    assert_eq!(s.feed.damage_flash(start, bot), 0.0);
    happen(&mut s, &[hit(me, 10.0, 100.0)]);
    assert_eq!(s.feed.damage_flash(s.sim.tick, me), 80.0 / 255.0);
}
