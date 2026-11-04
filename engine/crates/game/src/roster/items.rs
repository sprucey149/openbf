//! Pickups, stations, drop tables and item pools read from the class db.

use super::*;

/// A `powerupitem` class's box (`analysis/re-powerups.md` §2.3): the bounds
/// of its model, read from the PS2's RAM — (model, half extents, centre).
pub(super) const ITEM_BOXES: [(&str, [f32; 3], [f32; 3]); 4] = [
    (
        "item_powerup_ammo",
        [0.1255, 0.2390, 0.1202],
        [0.013, 0.239, 0.0],
    ),
    (
        "com_item_powerup_dual",
        [0.1565, 0.2504, 0.2451],
        [-0.058, 0.240, 0.022],
    ),
    (
        "item_powerup_health100",
        [0.1206, 0.2976, 0.1206],
        [0.0, 0.296, 0.0],
    ),
    (
        "item_powerup_health25",
        [0.0965, 0.2381, 0.0965],
        [0.0, 0.237, 0.0],
    ),
];

/// A `powerupitem` class's numbers (card PWUP, `re-powerups.md` §2.1, §2.2):
/// `SoldierHealth` / `SoldierAmmo` / BF2's `SoldierEnergy` and buff keys,
/// `LifeSpan`, and the box of its `GeometryName` (BF2's five small items
/// share health25's box, its health100 too). `None` when the
/// db has no such class or it is not a `powerupitem`.
pub fn item_class_from_db(db: &ClassDb, item: &str) -> Option<ItemClass> {
    let name = db.resolve(item)?;
    if db.engine_base(name) != Some("powerupitem") {
        return None;
    }
    let f = |key: &str| db.prop_f32(name, key);
    // A sound property's name: the value's first word ("<name> defer").
    let sound = |key: &str| {
        db.prop(name, key)
            .and_then(|v| v.split_whitespace().next())
            .map(str::to_owned)
    };
    let geometry = db
        .prop(name, "GeometryName")
        .map(|g| g.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let (_, half, centre) = ITEM_BOXES
        .iter()
        // BF2's `item_powerup_health100` model is the small one (§2.3).
        .find(|(m, _, _)| *m == geometry && !(db.is_bf2() && *m == "item_powerup_health100"))
        .copied()
        .unwrap_or(ITEM_BOXES[3]);
    Some(ItemClass {
        name: name.to_ascii_lowercase(),
        health: f("SoldierHealth").unwrap_or(0.0),
        ammo: f("SoldierAmmo").unwrap_or(0.0).max(0.0) as u32,
        energy: f("SoldierEnergy").unwrap_or(0.0),
        offense: (
            f("BuffOffenseTimer").unwrap_or(0.0),
            f("BuffOffenseMult").unwrap_or(1.0),
        ),
        defense: (
            f("BuffDefenseTimer").unwrap_or(0.0),
            f("BuffDefenseMult").unwrap_or(1.0),
        ),
        // The base item's default is FLT_MAX (§2.2); every disc item says 30.
        life_span: f("LifeSpan").unwrap_or(f32::MAX),
        half: half.into(),
        centre: centre.into(),
        geometry,
        pickup_sound: sound("PowerupSound"),
        land_sound: sound("CollisionOtherSound"),
    })
}

/// A `powerupstation` class's numbers (card PWST, `re-powerup-station.md`
/// §2.1): `PowerupDelay`, and per health type the health and clips one pass
/// gives (`SoldierHealth` / `SoldierAmmo` fill the person and droid slots,
/// then `Person..BuildingHealth` / `..Ammo` their own; no disc class authors
/// both forms), with the sounds the game layer plays. `None` when the db has
/// no such class or it is not a `powerupstation`.
pub fn station_class_from_db(db: &ClassDb, class: &str) -> Option<StationClass> {
    let name = db.resolve(class)?;
    if !crate::stations::is_station_base(db.engine_base(name)?) {
        return None;
    }
    let f = |key: &str| db.prop_f32(name, key);
    let sound = |key: &str| {
        db.prop(name, key)
            .and_then(|v| v.split_whitespace().next())
            .map(str::to_owned)
    };
    const TYPES: [&str; 5] = ["Person", "Animal", "Droid", "Vehicle", "Building"];
    let mut health = [0.0f32; 5];
    let mut ammo = [0u32; 5];
    if let Some(h) = f("SoldierHealth") {
        (health[0], health[2]) = (h, h);
    }
    if let Some(a) = f("SoldierAmmo") {
        let a = a.max(0.0) as u32;
        (ammo[0], ammo[2]) = (a, a);
    }
    for (i, t) in TYPES.iter().enumerate() {
        if let Some(h) = f(&format!("{t}Health")) {
            health[i] = h;
        }
        if let Some(a) = f(&format!("{t}Ammo")) {
            ammo[i] = a.max(0.0) as u32;
        }
    }
    Some(StationClass {
        name: name.to_ascii_lowercase(),
        delay: f("PowerupDelay").unwrap_or(core::sim::STATION_DELAY_DEFAULT),
        health,
        ammo,
        geometry: db
            .prop(name, "GeometryName")
            .map(|g| g.trim().to_ascii_lowercase())
            .unwrap_or_default(),
        powerup_sound: sound("PowerupSound"),
        ambient_sound: sound("AmbientSound"),
        anim: station_anim(db, name),
        sphere: None,
        wreck_sphere: None,
        damage_effects: Vec::new(),
        max_health: f("MaxHealth").unwrap_or(0.0),
        health_type: db
            .prop(name, "HealthType")
            .and_then(HealthType::parse)
            .unwrap_or(HealthType::Building),
        destroyed_geometry: db
            .prop(name, "DestroyedGeometryName")
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty()),
        explosion: db
            .prop(name, "ExplosionName")
            .and_then(|s| explosion_class(db, s)),
    })
}

/// A station class's animation keys (re-powerup-station §2.1, §6): the
/// class keeps the first 4 `ActiveRotateNode`s.
pub(super) fn station_anim(db: &ClassDb, name: &str) -> core::sim::StationAnim {
    let node = |key: &str| {
        db.prop(name, key)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    core::sim::StationAnim {
        rotate_speed: db.prop_f32(name, "IdleRotateSpeed").unwrap_or(0.0),
        wait_time: db.prop_f32(name, "IdleWaitTime").unwrap_or(0.0),
        wobble_node: node("IdleWobbleNode"),
        wobble_factor: db.prop_f32(name, "IdleWobbleFactor").unwrap_or(0.0),
        rotate_nodes: db
            .props_all(name, "ActiveRotateNode")
            .into_iter()
            .map(|v| v.trim().to_owned())
            .take(4)
            .collect(),
        spin_node: node("ActiveSpinNode"),
    }
}

/// The stations of `instances` (card PWST, §2.2, §3): every instance whose
/// class is a `powerupstation`, at its origin with its `Radius` (an
/// instance without one has no region and serves nobody, §3: left out).
/// With `unbuilt_zones` (BF2) a class with a build zone
/// (`manifest::BUILD_ZONE_PROP`, the `*_unbuilt` stations) starts unbuilt
/// and gives nothing (§7): left out too. Registers the classes with `sim`.
pub fn add_stations(
    sim: &mut core::sim::Sim,
    instances: &[manifest::ManifestInstance],
    db: &ClassDb,
    unbuilt_zones: bool,
) -> usize {
    let radius_key = crate::classdb::fnv1a_lowercase("Radius");
    let mut n = 0;
    for (instance, inst) in instances.iter().enumerate() {
        let Some(name) = db.resolve(&inst.class).map(str::to_owned) else {
            continue;
        };
        if unbuilt_zones && db.prop_hash(&name, manifest::BUILD_ZONE_PROP).is_some() {
            continue;
        }
        let Some(class) = station_class_from_db(db, &name) else {
            continue;
        };
        let destructible = db
            .engine_base(&name)
            .is_some_and(crate::stations::is_destructible_building);
        let instance_f32 = |key: u32| {
            inst.props
                .iter()
                .rev()
                .find(|(k, _)| *k == key)
                .and_then(|(_, v)| v.trim().parse::<f32>().ok())
        };
        let radius = if destructible {
            0.0
        } else {
            instance_f32(radius_key).unwrap_or(0.0)
        };
        let unusable = if destructible {
            !class.max_health.is_finite() || class.max_health <= 0.0
        } else {
            radius <= 0.0
        };
        if unusable {
            continue;
        }
        let c = sim.add_station_class(class);
        let class = &sim.station_classes[usize::from(c)];
        let mut s = core::sim::Station::new(c, glam::Vec3::from(inst.translation), radius);
        s.health = core::weapon::Health::new(class.max_health, class.health_type);
        if destructible {
            let current = instance_f32(crate::classdb::fnv1a_lowercase("CurHealth"))
                .or_else(|| db.prop_f32(&name, "CurHealth"));
            if let Some(current) = current.filter(|v| v.is_finite()) {
                s.health.cur = current.clamp(0.0, class.max_health);
            }
        }
        s.instance = Some(instance as u32);
        s.transform = crate::viewer::instance_matrix(inst.rotation, inst.translation);
        s.team = inst
            .props
            .iter()
            .rev()
            .find(|(k, _)| *k == crate::classdb::fnv1a_lowercase("Team"))
            .and_then(|(_, v)| v.trim().parse::<u8>().ok())
            .filter(|t| (1..=2).contains(t))
            .map(|t| t - 1);
        s.motion = core::sim::StationMotion::new(instance as u32 + 1);
        sim.collision
            .bind_station(instance as u32, sim.stations.len() as u16);
        sim.stations.push(s);
        n += 1;
    }
    n
}

/// A soldier class's drop table with its items resolved
/// ([`ClassDb::drop_table`]).
pub fn drop_entries(db: &ClassDb, soldier: &str) -> Vec<DropEntry> {
    db.drop_table(soldier)
        .into_iter()
        .map(|(item, prob)| DropEntry {
            item: item_class_from_db(db, &item),
            prob,
        })
        .collect()
}

/// The `PowerupItem` pool's capacity on a map (`re-powerups.md` §2.4): 25,
/// but for the BF1 missions whose script calls
/// `SetMemoryPoolSize("PowerupItem", n)`. No BF2 mission names the pool.
pub fn item_pool(game: &str, map: &str) -> usize {
    // Each mission's code and the name its baked manifest carries.
    const BF1: [(&str, usize); 13] = [
        ("bes1", 40),
        ("geo1", 25),
        ("hot1", 60),
        ("hoth", 60),
        ("kas1", 40),
        ("kashyyyk1", 40),
        ("kas2", 30),
        ("nab1", 40),
        ("naboo1", 40),
        ("rhn1", 60),
        ("rhenvar1", 60),
        ("tat1", 60),
        ("yav2", 40),
    ];
    let map = map.to_ascii_lowercase();
    if !game.eq_ignore_ascii_case("bf1") {
        return core::sim::ITEM_POOL_DEFAULT;
    }
    BF1.iter()
        .find(|(m, _)| map.starts_with(m))
        .map_or(core::sim::ITEM_POOL_DEFAULT, |(_, n)| *n)
}
