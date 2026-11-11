//! ExplosionExpire is independent of the ordnance's flight family.
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter};
use core::sim::{Event, Sim};
use glam::Vec3;

fn mortar_db(era: &str) -> game::classdb::ClassDb {
    let root = std::path::PathBuf::from(
        std::env::var_os("SHELL_DEFAULTS_ASSETS").expect("explicit retained profile required"),
    );
    game::roster::load_classdb(&root.join("classes"), era, &["all", "imp", "cis", "rep"]).unwrap()
}

/// Register the stock soldier kit, select its mortar and use real pad edges.
/// The empty no-terrain case isolates flight expiry from contact.
fn mortar_fire(era: &str, side: &str, hold: u32, ground: bool) -> (Sim, usize, Vec3, f32) {
    use core::sim::{buttons, PadState};
    use core::unit::Controller;
    let db = mortar_db(era);
    let soldier = match (era, side) {
        ("bf1", "imp") => "imp_inf_pilottie",
        ("bf1", "cis") => "cis_inf_pilotdroid",
        ("bf1", "all") => "all_inf_smuggler",
        ("bf2", "imp") => "imp_inf_officer",
        ("bf2", "cis") => "cis_inf_officer",
        ("bf2", "all") => "all_inf_wookiee",
        _ => panic!("unlisted stock mortar owner"),
    };
    let roster = game::roster::ClassRoster::build(&db, soldier, 1, None).unwrap();
    let mut sim = Sim::empty(103);
    sim.ground_rules = if era == "bf1" {
        core::soldier::GroundRules::Bf1
    } else {
        core::soldier::GroundRules::Bf2
    };
    if ground {
        sim.heightfield = Some(core::heightfield::Heightfield {
            size: 2,
            unit: 1000.0,
            offset_x: 500.0,
            offset_z: 500.0,
            heights: vec![0.0; 4],
        });
    }
    let registered = roster.register(&mut sim, &mut Vec::new());
    let owner = sim
        .spawn(&registered.spec(
            0,
            Vec3::Y * if ground { 0.0 } else { 1000.0 },
            0.0,
            Controller::Player { slot: 0 },
            0,
            0,
        ))
        .unwrap();
    let name = format!("{side}_weap_inf_mortar_launcher");
    for _ in 0..8 {
        let unit = sim.unit(owner).unwrap();
        if sim.weapon_classes[unit.weapon_class].name == name {
            break;
        }
        sim.step(&[PadState {
            buttons: buttons::SWITCH_WEAPON,
            ..Default::default()
        }]);
        sim.step(&[PadState::default()]);
    }
    assert_eq!(
        sim.weapon_classes[sim.unit(owner).unwrap().weapon_class].name,
        name
    );
    sim.unit_mut(owner).unwrap().soldier.pitch = 0.0;
    for tick in 0..hold + 150 {
        sim.step(&[PadState {
            buttons: if tick < hold { buttons::FIRE } else { 0 },
            ..Default::default()
        }]);
        if let Some(class) = sim.events.iter().find_map(|event| match event {
            Event::Shot { weapon_class, .. } | Event::Throw { weapon_class, .. } => {
                Some(*weapon_class as usize)
            }
            _ => None,
        }) {
            if let Some(b) = sim.bolts.first() {
                assert_eq!(b.weapon_class, class);
                assert!((b.age - core::TICK_DT).abs() < 1e-6);
                let velocity = b.vel + Vec3::Y * b.gravity * core::TICK_DT;
                let life = b.remaining + core::TICK_DT;
                return (sim, class, velocity, life);
            }
            let g = sim.stickies.first().expect("FIRE created a round");
            assert_eq!(g.weapon_class, class);
            assert!(!g.armed && g.touched.is_none());
            let velocity = g.vel - Vec3::Y * g.gravity * core::TICK_DT;
            let life = g.life;
            return (sim, class, velocity, life);
        }
    }
    panic!("{era} {soldier}: FIRE/release created no round");
}

fn mortar_end(mut sim: Sim, class: usize) -> (u32, bool, Vec3) {
    // The launch step has already advanced this projectile once.
    for flight_tick in 2..=250 {
        sim.step(&[]);
        if let Some((struck, pos)) = sim.events.iter().find_map(|e| match e {
            Event::BoltExpired { struck, pos } => Some((*struck, *pos)),
            _ => None,
        }) {
            assert!(sim.bolts.is_empty());
            let expiry = sim.events.iter().find_map(|e| match e {
                Event::OrdnanceExpired { weapon_class, pos } if *weapon_class as usize == class => {
                    Some(*pos)
                }
                _ => None,
            });
            if struck {
                assert!(expiry.is_none(), "impact must not also expire in air");
            } else {
                assert!(expiry.is_some(), "natural expiry must emit ExplosionExpire");
            }
            return (flight_tick, struck, expiry.unwrap_or(pos));
        }
        assert_eq!(
            sim.bolts.len(),
            1,
            "round disappeared without expiry/contact event"
        );
    }
    panic!("no runtime expiry or contact");
}

#[test]
#[ignore = "requires SHELL_DEFAULTS_ASSETS pointing to a retained source profile"]
fn shell_defaults_stock_bf1_taps_expire_after_three_seconds() {
    let mut expiries = Vec::new();
    for side in ["imp", "cis"] {
        let (sim, class, velocity, life) = mortar_fire("bf1", side, 1, false);
        assert!((sim.bolts[0].gravity - 17.15).abs() < 1e-5);
        let (ticks, struck, pos) = mortar_end(sim, class);
        println!(
            "bf1 {side} tap speed={} life={life} flight_ticks={ticks} struck={struck} end={pos:?}",
            velocity.length()
        );
        assert!(!struck);
        assert!((velocity.length() - 40.0).abs() < 1e-4);
        expiries.push((side, ticks, life));
    }
    for (side, ticks, life) in expiries {
        assert!(
            (90..=91).contains(&ticks),
            "{side}: observed runtime expiry at {ticks} ticks"
        );
        assert!((life - 3.0).abs() < 1e-6);
    }
}

#[test]
#[ignore = "requires SHELL_DEFAULTS_ASSETS pointing to a retained source profile"]
fn shell_defaults_stock_bomb_callers_keep_their_authored_motion() {
    let db = mortar_db("bf2");
    for (name, velocity, gravity) in [
        ("all_weap_fly_ywing_bomb", 175.0, 2.5),
        ("cis_weap_fly_greviousfighter_bomb", 200.0, 7.5),
        ("imp_weap_fly_tiebomber_bomb", 225.0, 5.0),
        ("rep_weap_fly_vwing_ptorpedo", 225.0, 5.0),
    ] {
        let w = game::roster::weapon_from_db(&db, name).unwrap();
        assert_eq!(
            (
                w.ordnance.velocity,
                w.ordnance.life_span,
                w.ordnance.gravity
            ),
            (velocity, 10.0, gravity)
        );
        println!("bf2 {name}: velocity={velocity} life=10 gravity={gravity}");
    }
}

#[test]
#[ignore = "requires SHELL_DEFAULTS_ASSETS pointing to a retained source profile"]
fn shell_defaults_stock_bf1_taps_impact_before_expiry() {
    for side in ["imp", "cis"] {
        let (sim, class, velocity, life) = mortar_fire("bf1", side, 1, true);
        let (ticks, struck, pos) = mortar_end(sim, class);
        println!("bf1 {side} impact speed={} life={life} flight_ticks={ticks} struck={struck} end={pos:?}", velocity.length());
        assert!(struck);
        assert!(ticks < 90);
        assert!((velocity.length() - 40.0).abs() < 1e-4);
        assert!((life - 3.0).abs() < 1e-6);
    }
}

#[test]
#[ignore = "requires SHELL_DEFAULTS_ASSETS pointing to a retained source profile"]
fn shell_defaults_stock_held_and_bf2_controls() {
    for (era, side, hold, speed, life) in [
        ("bf1", "imp", 40, 40.0, 0.5),
        ("bf1", "cis", 40, 40.0, 0.5),
        ("bf1", "all", 1, 25.0, 0.5),
        ("bf2", "imp", 1, 40.0, 8.0),
        ("bf2", "cis", 1, 40.0, 8.0),
        ("bf2", "all", 1, 40.0, 8.0),
        ("bf2", "imp", 40, 40.0, 0.1),
        ("bf2", "cis", 40, 40.0, 0.1),
        ("bf2", "all", 40, 40.0, 0.1),
    ] {
        let (sim, class, velocity, actual_life) = mortar_fire(era, side, hold, false);
        println!(
            "{era} {side} hold={hold} speed={} life={actual_life}",
            velocity.length()
        );
        assert!((velocity.length() - speed).abs() < 1e-4);
        assert!((actual_life - life).abs() < 1e-6);
        if era == "bf2" && hold == 1 {
            assert!(sim.stickies.is_empty());
            let (ticks, struck, _) = mortar_end(sim, class);
            println!("{era} {side} tap expiry flight_ticks={ticks} struck={struck}");
            assert!(!struck);
            assert!((240..=241).contains(&ticks));
        } else {
            assert!(sim.bolts.is_empty());
            assert_eq!(sim.stickies.len(), 1);
            assert_eq!(
                sim.weapon_classes[class]
                    .ordnance
                    .sticky
                    .as_ref()
                    .unwrap()
                    .bf2,
                era == "bf2"
            );
        }
    }
}

#[test]
fn rocket_expiry_classes_keep_the_authored_explosion() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf1_all.ron").exists() || !root.join("classes/bf2_all.ron").exists() {
        eprintln!("SKIP: requires both disc class bakes");
        return;
    }
    for (game, names) in [
        ("bf1", &["rep_weap_inf_rocket_launcher"][..]),
        (
            "bf2",
            &[
                "cis_weap_inf_wrist_rocket",
                "rep_weap_inf_emp_launcher",
                "tur_weap_spa_recoilless_rifle",
            ][..],
        ),
    ] {
        let mut db = game::roster::load_classdb(
            &root.join("classes"),
            game,
            &["all", "imp", "rep", "cis", "tur"],
        )
        .unwrap();
        if game == "bf2" {
            db.load(&root.join("classes/bf2_tur.ron")).unwrap();
        }
        for name in names {
            let ord = db.prop(name, "OrdnanceName").expect(name);
            assert!(db.prop(ord, "ExplosionExpire").is_some(), "{game} {ord}");
            let w = game::roster::weapon_from_db(&db, name).unwrap();
            assert!(
                w.ordnance.expire.is_some(),
                "{game} {ord}: lost ExplosionExpire"
            );
            println!("EXPIRE {game} {name} {ord} base={:?}", w.ordnance.kind);
        }
    }
}

#[test]
fn wrist_rocket_bursts_at_its_actual_air_expiry_position_and_spawns_its_effect() {
    let root = game::assets::baked_root();
    if !root.join("classes/bf2_cis.ron").exists() {
        eprintln!("SKIP: requires BF2 class bake");
        return;
    }
    let db = game::roster::load_classdb(&root.join("classes"), "bf2", &["cis"]).unwrap();
    let w = game::roster::weapon_from_db(&db, "cis_weap_inf_wrist_rocket").unwrap();
    let o = w.ordnance.clone();
    let look = o.expire.as_ref().unwrap().look.clone();
    let mut sim = Sim::empty(42);
    let class = sim.add_weapon_class(w);
    let mut b = Bolt::fire(
        Vec3::Y * 100.0,
        Vec3::Z,
        o.velocity,
        o.life_span,
        Shooter {
            owner: core::unit::UnitHandle { index: 100, gen: 0 },
            team: 0,
            weapon_class: class,
            is_player: true,
        },
        None,
        None,
        &sim.collision,
        HullFilter::Static,
    );
    b.as_missile(
        &o,
        o.missile.as_ref().unwrap(),
        None,
        &mut core::lcg::GameLcg::new(42),
    );
    sim.bolts.push(b);
    let mut fx = game::fx::Fx::load(&root, &root.join("bf2_kas2"), "bf2", &["cis"]);
    let mut burst = None;
    for tick in 0..(o.life_span / core::TICK_DT).ceil() as usize + 2 {
        let previous = sim.bolts[0].clone();
        sim.step(&[]);
        fx.ingest_sim(&sim);
        if let Some(pos) = sim.events.iter().find_map(|e| match e {
            Event::OrdnanceExpired { pos, weapon_class } if *weapon_class as usize == class => {
                Some(*pos)
            }
            _ => None,
        }) {
            assert!(previous.remaining <= core::TICK_DT);
            assert!(sim.bolts.is_empty());
            assert!(!sim.events.iter().any(|e| matches!(e, Event::Impact { .. })));
            assert!(pos.distance(previous.pos) <= o.velocity * core::TICK_DT + 0.01);
            let effects: Vec<_> = fx
                .live()
                .iter()
                .filter(|e| {
                    e.name()
                        .is_some_and(|n| look.effects.iter().any(|x| x == n))
                })
                .collect();
            assert_eq!(effects.len(), 1, "expiry effect missing or duplicated");
            assert_eq!(effects[0].frame.pos, pos);
            burst = Some((tick, pos));
            break;
        }
        assert!(!sim.bolts.is_empty(), "shot vanished without expiry event");
        fx.update(core::TICK_DT);
    }
    println!("WRIST natural expiry {:?}", burst.expect("no expiry burst"));
}
