//! Issue #78: the HUD's ring and overheat signs read what the sim tracks, on foot
//! and in a seat, in both games (`analysis/re-heat-hud.md`). The drawing is
//! `hud_layout::the_heat_ring_and_its_overheat_signs_follow_the_gauge`.

use super::remote_droids::walk_of;
use super::seat_camera::{map, missions_of};
use core::sim::{buttons, PadState};
use core::vehicle::VehicleKind;
use game::roster::{load_classdb, ClassRoster};
use glam::Vec3;

const FIRE: PadState = PadState {
    lx: 0,
    ly: 0,
    rx: 0,
    ry: 0,
    buttons: buttons::FIRE,
    mx: 0,
    my: 0,
    fire_lag: 0,
    pick: None,
    view_lag: 0,
};

/// The first soldier class of a game that passes `test`.
fn class_where(game: &str, test: impl Fn(&ClassRoster) -> bool) -> Option<ClassRoster> {
    let root = game::assets::baked_root();
    let db = load_classdb(&root.join("classes"), game, &["all", "imp", "rep", "cis"]).ok()?;
    let mut names: Vec<String> = db
        .names()
        .filter(|n| game::roster::is_soldier_chain(&db, n) && !n.contains("_hero_"))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.iter().find_map(|n| {
        let r = ClassRoster::build(&db, n, 0, None).ok()?;
        test(&r).then_some(r)
    })
}

/// Fire until the weapon overheats, then let go until it clears. A pistol, which
/// fires once a press, is tapped as the PS2 trials did: 3 updates on, 5 off.
fn overheat(walk: &mut game::walk::WalkSim, what: &str, tap: bool) {
    let (mut peak, mut hot) = (0.0f32, None);
    for k in 0..(40.0 / core::TICK_DT) as usize {
        let pad = if !tap || k % 8 < 3 {
            FIRE
        } else {
            PadState::default()
        };
        walk.step_tick(&[pad]);
        let g = game::hud_feed::weapon_gauge(walk);
        peak = peak.max(g.ring);
        if g.overheated {
            hot = Some(g);
            break;
        }
    }
    let hot = hot.unwrap_or_else(|| panic!("{what}: never overheated (peak {peak})"));
    assert!(hot.ring > 0.9, "{what}: ring {} at the lock", hot.ring);
    assert!(
        hot.refire > 0.0 && hot.refire < 1.0,
        "{what}: refire {}",
        hot.refire
    );
    let mut cleared = None;
    for _ in 0..(30.0 / core::TICK_DT) as usize {
        walk.step_tick(&[PadState::default()]);
        let g = game::hud_feed::weapon_gauge(walk);
        if !g.overheated {
            cleared = Some(g);
            break;
        }
    }
    let g = cleared.unwrap_or_else(|| panic!("{what}: never cleared"));
    assert_eq!(g.refire, 0.0, "{what}");
    assert!(g.ring > 0.0 && g.ring < hot.ring, "{what}: ring {}", g.ring);
}

#[test]
fn a_soldiers_hot_weapon_fills_the_ring_and_locks_in_both_games() {
    let hot = |w: &core::weapon::WeaponClass| {
        w.infinite_ammo() && w.heat_per_shot > 0.0 && w.name.contains("pistol")
    };
    for game in ["bf1", "bf2"] {
        let holds = |r: &ClassRoster| r.held.iter().any(|h| h.channel == 0 && hot(&h.weapon));
        let Some(roster) = class_where(game, holds) else {
            eprintln!("SKIP: no {game} class bake");
            continue;
        };
        let mut walk = walk_of(&roster);
        // The switch forward, until the hot weapon is in his hands.
        for _ in 0..8 {
            let me = &walk.sim.units[walk.player.index as usize];
            if hot(&walk.sim.weapon_classes[me.weapon_class]) {
                break;
            }
            walk.step_tick(&[PadState {
                buttons: buttons::SWITCH_WEAPON,
                ..Default::default()
            }]);
            for _ in 0..30 {
                walk.step_tick(&[PadState::default()]);
            }
        }
        let me = &walk.sim.units[walk.player.index as usize];
        let held = &walk.sim.weapon_classes[me.weapon_class];
        assert!(hot(held), "{game} {}: holds {}", roster.class, held.name);
        println!("{game}: {} holds {}", roster.class, held.name);
        assert_eq!(
            game::hud_feed::weapon_gauge(&walk).ring,
            0.0,
            "{game}: cold"
        );
        overheat(&mut walk, game, true);
    }
}

#[test]
fn a_held_charge_weapon_fills_the_ring_and_empties_on_release() {
    let charge = |r: &ClassRoster| !r.weapon.infinite_ammo() && r.weapon.max_pressed_time > 0.0;
    for game in ["bf1", "bf2"] {
        let Some(roster) = class_where(game, charge) else {
            eprintln!("SKIP: no {game} class bake");
            continue;
        };
        let max = roster.weapon.max_pressed_time;
        println!(
            "{game}: {} holds {}, {max} s",
            roster.class, roster.weapon.name
        );
        let mut walk = walk_of(&roster);
        let ticks = (max * 0.5 / core::TICK_DT).round() as usize;
        for _ in 0..ticks {
            walk.step_tick(&[FIRE]);
        }
        let half = game::hud_feed::weapon_gauge(&walk).ring;
        assert!((half - 0.5).abs() < 0.1, "{game}: half way: {half}");
        for _ in 0..ticks * 3 {
            walk.step_tick(&[FIRE]);
        }
        let full = game::hud_feed::weapon_gauge(&walk).ring;
        assert_eq!(full, 1.0, "{game}: held past MaxPressedTime");
        walk.step_tick(&[PadState::default()]);
        walk.step_tick(&[PadState::default()]);
        assert_eq!(
            game::hud_feed::weapon_gauge(&walk).ring,
            0.0,
            "{game}: released"
        );
    }
}

/// Each game's first armed seat that heats, of a building turret and of a pad
/// vehicle on its map.
#[test]
fn a_seats_hot_gun_fills_the_ring_and_locks_in_both_games() {
    let mut kinds = std::collections::BTreeSet::new();
    let root = game::assets::baked_root();
    for (key, game, dir) in game::assets::discover_maps(&root) {
        let Some(mission) = missions_of(&key).into_iter().next() else {
            continue;
        };
        let Some(mut m) = map(&dir, &mission.factions()) else {
            continue;
        };
        let specs: Vec<_> = m
            .build
            .pads
            .iter()
            .flat_map(|p| {
                p.classes
                    .iter()
                    .flatten()
                    .map(|c| core::vehicle::VehicleSpec {
                        pos: p.pos + Vec3::Y,
                        yaw: p.yaw,
                        ..c.spec.clone()
                    })
            })
            .collect();
        for s in &specs {
            m.walk.sim.spawn_vehicle(s);
        }
        let all: Vec<_> = m
            .walk
            .sim
            .alive_vehicles()
            .map(|(h, v)| (h, v.kind))
            .collect();
        for (h, kind) in all {
            // A parked flyer's guns do not fire: both PS2 flyer updates clear the
            // triggers outside Flying (`Sim::seat_gun`).
            if kind == VehicleKind::Flyer {
                continue;
            }
            let want = (game.clone(), kind == VehicleKind::Turret);
            if kinds.contains(&want) {
                continue;
            }
            let class = m.walk.sim.vehicle(h).unwrap().class as usize;
            let guns = m
                .walk
                .sim
                .seat_setup
                .get(class)
                .and_then(Option::as_ref)
                .cloned();
            let Some(guns) = guns else { continue };
            let seat = guns.guns.iter().position(|g| {
                g.as_ref().is_some_and(|g| {
                    g.weapons.first().is_some_and(|w| {
                        let c = &m.walk.sim.weapon_classes[w.class];
                        // Held down it has to outrun its cooling, or it never locks.
                        let rate = c.heat_per_shot / c.shot_delay[0].max(core::TICK_DT);
                        c.infinite_ammo() && rate > c.heat_recover_rate
                    })
                })
            });
            let Some(seat) = seat else { continue };
            let player = m.walk.player;
            if !m.walk.sim.seat_unit(player, h, seat as u8) {
                continue;
            }
            let what = format!("{key} {} seat {seat}", m.build.classes[class]);
            println!("{what}");
            assert_eq!(
                game::hud_feed::weapon_gauge(&m.walk).ring,
                0.0,
                "{what}: cold"
            );
            overheat(&mut m.walk, &what, false);
            m.walk.sim.unseat_unit(player);
            kinds.insert(want);
        }
        if kinds.len() == 4 {
            break;
        }
    }
    println!("{kinds:?}");
    if !kinds.is_empty() {
        assert_eq!(kinds.len(), 4, "turret and vehicle, both games: {kinds:?}");
    }
}

#[test]
fn ps2_reload_progress_uses_remaining_time_and_reaches_reticle_events() {
    use core::weapon::{WeaponClass, WeaponState};
    let c = WeaponClass {
        rounds_per_clip: 10,
        reload_time: 2.0,
        ..Default::default()
    };
    let mut s = WeaponState::new(&c, true, 2);
    s.clip = 0;
    assert!(s.start_reload(&c));
    for (remaining, expected) in [(2.0, 1.0), (1.0, 0.5), (0.5, 0.25), (0.0, 0.0)] {
        s.reload_timer = remaining;
        let g = game::hud_feed::gauge_of(&c, &s);
        assert_eq!(g.refire, expected);
        assert_eq!(
            g.ring, 0.0,
            "counted weapons keep the charge/heat channel separate"
        );
        assert!(!g.overheated);
    }
}

#[test]
fn ps2_reload_firing_progress_obeys_display_refire_and_timer_bounds() {
    use core::weapon::{FireState, OrdnanceClass, PropSource, WeaponClass, WeaponState};
    struct Props<'a>(&'a [(&'a str, &'a str)]);
    impl PropSource for Props<'_> {
        fn prop(&self, key: &str) -> Option<&str> {
            self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
        }
        fn prop_hash(&self, _: u32) -> Option<&str> {
            None
        }
    }
    for props in [
        &[][..],
        &[("DisplayRefire", "1")][..],
        &[("DisplayRefire", "0")][..],
    ] {
        let mut c = WeaponClass::from_props("test", &Props(props), OrdnanceClass::default());
        c.rounds_per_clip = 10;
        let mut s = WeaponState::new(&c, true, 2);
        s.state = FireState::Firing;
        s.refire_time = 2.0;
        let enabled = props != [("DisplayRefire", "0")];
        for (elapsed, remaining) in [(-1.0, 1.0), (0.0, 1.0), (0.5, 0.75), (2.0, 0.0), (3.0, 0.0)] {
            s.refire_accum = elapsed;
            assert_eq!(
                game::hud_feed::gauge_of(&c, &s).refire,
                if enabled { remaining } else { 0.0 }
            );
        }
        s.refire_time = 0.0;
        assert_eq!(game::hud_feed::gauge_of(&c, &s).refire, 0.0);
        s.state = FireState::Idle;
        s.refire_time = 2.0;
        s.refire_accum = 0.0;
        assert_eq!(game::hud_feed::gauge_of(&c, &s).refire, 0.0);
    }
}

#[test]
fn ps2_reload_keeps_unlimited_heat_and_counted_charge_selection() {
    use core::weapon::{FireState, WeaponClass, WeaponState};
    let mut c = WeaponClass {
        heat_threshold: 0.25,
        ..Default::default()
    };
    let mut s = WeaponState::new(&c, true, 2);
    s.state = FireState::Firing;
    s.refire_time = 1.0;
    s.refire_accum = 0.25;
    s.heat = 0.1;
    assert_eq!(
        game::hud_feed::gauge_of(&c, &s).refire,
        0.0,
        "cold unlimited weapon suppresses refire"
    );
    s.heat = 1.0;
    s.overheated = true;
    assert_eq!(game::hud_feed::gauge_of(&c, &s).refire, 0.75);
    c.rounds_per_clip = 10;
    c.max_pressed_time = 2.0;
    s.overheated = false;
    s.state = FireState::Idle;
    s.active = true;
    s.fire_cmd = 1;
    s.refire_accum = 1.0;
    assert_eq!(game::hud_feed::gauge_of(&c, &s).ring, 0.5);
    assert_eq!(game::hud_feed::gauge_of(&c, &s).refire, 0.0);
}

#[test]
fn ps2_reload_publishes_to_bf2_reticle_bits_for_both_source_games() {
    let common = game::assets::baked_root().join("common_bf2/hud");
    if !common.join("1playerhud.ron").is_file() {
        eprintln!("SKIP: no baked BF2 HUD");
        return;
    }
    let read = |f: &str| -> manifest::hud::HudLayout {
        ron::from_str(&std::fs::read_to_string(common.join(f)).unwrap()).unwrap()
    };
    let c = core::weapon::WeaponClass {
        rounds_per_clip: 10,
        reload_time: 2.0,
        ..Default::default()
    };
    let mut s = core::weapon::WeaponState::new(&c, true, 2);
    for bf2 in [false, true] {
        let mut hud = ui::hud_tree::Hud::new(&read("1playerhud.ron"), &read("hudtransforms.ron"));
        for (remaining, lit) in [(2.0, 3), (1.0, 2), (0.5, 1), (0.0, 0)] {
            s.reload_timer = remaining;
            let model = ui::hud::HudModel {
                hp: 100.0,
                hp_max: 100.0,
                weapon: Some(ui::hud::HudWeapon {
                    bf2,
                    total: Some(20),
                    ..Default::default()
                }),
                reticle: Some(0),
                gauge: game::hud_feed::gauge_of(&c, &s),
                ..Default::default()
            };
            ui::hud::feed(&mut hud, &model, &|_| false);
            hud.advance(0.05);
            for name in [
                "player1weapon1refire_leftbit",
                "player1weapon1refire_rightbit",
            ] {
                let e = &hud.elems[hud.find(name).unwrap()];
                let count = (0..e.segments)
                    .filter(|k| (*k as f32 / e.segments as f32) < e.value - 1e-4)
                    .count();
                assert_eq!(count, lit, "bf2={bf2}, remaining={remaining}, {name}");
            }
            let overheat = &hud.elems[hud.find("player1weapon1overheat").unwrap()];
            assert_eq!(overheat.fader.state, 0);
        }
    }
}
