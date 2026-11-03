//! ExplosionExpire is independent of the ordnance's flight family.
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter};
use core::sim::{Event, Sim};
use glam::Vec3;

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
