//! A map's vehicle spawn pads, built for the sim (card VPAD,
//! analysis/re-vehicles.md §7 [O]): every `com_item_vehicle_spawn` instance
//! of the manifest, its props read as the PS2's setter reads them (B1
//! FUN_002af778), its class per team resolved against the mission's two
//! factions and the [`ClassDb`] — `core::vehicle_pad` runs the rest.
//!
//! The class table (B1 DAT_005183a0 = B2 DAT_005feeb0, 7 rows × 3 hashes, one
//! row per side): a prop whose key is a side's `Class<SideName>` goes to the
//! FIRST team of that side; `Class<Side>Atk` only to team 1, `Class<Side>Def`
//! only to team 2 (B1:239857-239875). Teams are tried in order 0 (neutral),
//! 1, 2; the port fields teams 1 and 2 (port teams 0 and 1), so the neutral
//! and locals rows spawn nothing here. A class the level does not carry is
//! null on the PS2 and spawns nothing (BF2 YAV1's `re_hover_barcspeeder`,
//! named by YAV1.LVL alone) — [`PadBuild::absent`]. A mission can load a
//! side beyond its two factions' ([`mission_side_files`]): BF1 NAB1's reads
//! SIDE\gun.lvl, whose `gun_walk_kaadu` three pads key for the defenders
//! (card KAAD; 3 kaadus live in the PS2's NAB1 RAM, one ridden).
//!
//! The building turrets (card VTURB, §8) come from the same manifest: every
//! instance whose class is an armed building with a gunner seat becomes a
//! `VehicleKind::Turret` at the instance ([`PadBuild::turrets`]) — its
//! gunner ring, guns and hull pool, never moving, no pad. The world keeps
//! drawing (and colliding with) the building's model, so its class has no
//! viewer geometry; the world's own draws of it are turned to the seat's gun
//! (card VTURD, `crate::viewer::turret_pose`, joined by the instance's
//! translation). PROVISIONAL: the instance's
//! `Team` is its `home_team` and, like any vehicle that empties to neutral,
//! it starts open to both teams (every BF1 turret instance authors Team 0;
//! how the PS2 gates a building's seat by team is untraced).

mod flight;
pub use flight::flight_paths;

use crate::classdb::{fnv1a_lowercase, ClassDb};
use crate::vehicle_class;
use core::vehicle::gun::{Aimer, GunSetup, GunWeapon, MAX_SEAT_WEAPONS};
use core::vehicle::{BlastSetup, SeatSetup, VehicleKind, VehicleSpec};
use core::vehicle_pad::{PadClass, PadDef, DECAY_TIME, SPAWN_TIME};
use core::weapon::ExplosionClass;
use glam::Vec3;
use manifest::{Manifest, ManifestHardpoint, ManifestInstance};
use std::collections::HashMap;

/// The pad's class (BASE `vehiclespawn`).
pub const PAD_CLASS: &str = "com_item_vehicle_spawn";

/// A vehicle's radius for the decay's player-proximity test (the PS2 reads
/// the entity's collision sphere, FUN_001645a0). PROVISIONAL: one number for
/// every class until the vehicle bounds are read.
pub const VEHICLE_RADIUS: f32 = 3.0;

pub const FLYER_AIM_BOXES: &[(&str, f32)] = &[
    ("all_fly_awing", 12.08),
    ("all_fly_kwing", 37.635),
    ("all_fly_snowspeeder", 7.388),
    ("all_fly_xwing", 16.2),
    ("all_fly_ywing", 19.037),
    ("all_la_at_gunship", 29.257),
    ("cis_fly_droidfighter", 11.787),
    ("cis_fly_droidgunship", 18.171),
    ("cis_fly_geofighter", 12.633),
    ("cis_fly_greviousfighter", 15.632),
    ("cis_fly_maf", 21.959),
    ("cis_fly_tridroidfighter", 8.438),
    ("imp_fly_shuttleproto", 15.334),
    ("imp_fly_tiebomber", 12.362),
    ("imp_fly_tiefighter", 9.541),
    ("imp_fly_tieinterceptor", 9.606),
    ("imp_fly_trooptrans", 21.628),
    ("rep_fly_anakinstarfighter", 6.985),
    ("rep_fly_arc170fighter", 19.043),
    ("rep_fly_jedifighter", 11.997),
    ("rep_fly_vtrans", 51.157),
    ("rep_fly_vwing", 10.894),
    ("rep_la_at_gunship", 29.257),
];

pub fn flyer_aim_box(geometry: &str) -> f32 {
    FLYER_AIM_BOXES
        .iter()
        .find(|(m, _)| m.eq_ignore_ascii_case(geometry))
        .map_or(0.0, |(_, v)| *v)
}

pub fn flyer_params(
    db: &crate::classdb::ClassDb,
    c: &str,
    bf2: bool,
    landed: f32,
) -> Option<core::vehicle::flyer::FlyerParams> {
    vehicle_class::FlyerClass::from_db(db, c, bf2).map(|f| core::vehicle::flyer::FlyerParams {
        never_crash_when_unpiloted: vehicle_class::prop_flag(db, c, "NeverCrashWhenUnpiloted"),
        trick_roll_speed: vehicle_class::prop_float(db, c, "TrickRollSpeed").unwrap_or(1.0),
        trick_drain: vehicle_class::prop_float(db, c, "EnergyTrickDrainSingleTap").unwrap_or(15.0),
        trick_side_strafe: vehicle_class::prop_float(db, c, "TrickSideRollStrafeSpeed")
            .unwrap_or(0.0),
        aim_box: db
            .prop(c, "GeometryName")
            .map(|g| flyer_aim_box(g.trim()))
            .unwrap_or(0.0),
        pilot_repair_scale: vehicle_class::prop_float(db, c, "PilotSkillRepairScale")
            .unwrap_or(0.005),
        has_chunks: vehicle_class::has_chunks(db, c),
        view_fov: vehicle_class::seats(db, c)
            .first()
            .and_then(|s| s.third_person_fov)
            .unwrap_or(48.0),
        ..f.params(landed)
    })
}

/// The side-table row of a port faction key: (`Class<SideName>`, the
/// `Class<Side>` stem of the Atk / Def keys).
pub fn side_row(faction: &str) -> Option<(&'static str, &'static str)> {
    Some(match faction {
        "all" => ("Alliance", "All"),
        "imp" => ("Empire", "Imp"),
        "rep" => ("Republic", "Rep"),
        "cis" => ("Cis", "Cis"),
        _ => return None,
    })
}

/// The class RONs of the sides a map's missions load beyond their two
/// factions' (`ReadDataFile("SIDE\\<x>.lvl", …)` in the mission lua), for
/// the classes its pads name. Loaded after [`crate::roster::class_files`],
/// so (first definition wins) they add classes and shadow none.
///
/// Only the sides a pad needs: BF1 NAB1's `nab1c`, `nab1i` and `nab1c_h`
/// read SIDE\gun.lvl with `gun_walk_kaadu`. The other locals sides the BF1
/// missions read (des, ewk, geo, gar, wok) carry no class a pad keys.
///
/// BF2's building turrets (card VTUR2): the conquest missions of these 11
/// maps read SIDE\tur.lvl (`cor1`, `end1`, `geo1`, `hot1`, `kam1`, `kas2`,
/// `myg1`, `nab2`, `pol1`, `tat2`, `yav1` `*_con.luac`; DAG1, DEA1, FEL1,
/// MUS1, TAN1 and UTA1's do not), and the map bake carries the geometry of
/// the TUR classes its worlds place (`bake::map`).
pub fn mission_side_files(game: &str, map: &str) -> &'static [&'static str] {
    match (game, map) {
        ("bf1", "bf1_nab1") => &["bf1_gun.ron"],
        (
            "bf2",
            "bf2_cor1" | "bf2_end1" | "bf2_geo1" | "bf2_hot1" | "bf2_kam1" | "bf2_kas2"
            | "bf2_myg1" | "bf2_nab2" | "bf2_pol1" | "tat2" | "bf2_yav1" | "bf2_fel1" | "bf2_mus1"
            | "bf2_uta1",
        ) => &["bf2_tur.ron"],
        _ => &[],
    }
}

/// Which port team a pad prop's key assigns its class to, for a battle of
/// `factions` (port team 0 = PS2 team 1), or `None` (not a class key, or a
/// key for a side the battle does not field).
pub fn class_key_team(key: u32, factions: &[String; 2]) -> Option<u8> {
    // PS2 team 0 is the Neutral side: `ClassNeutral` in all three columns.
    if key == fnv1a_lowercase("ClassNeutral") {
        return None;
    }
    for (t, f) in factions.iter().enumerate() {
        let Some((name, stem)) = side_row(f) else {
            continue;
        };
        let col = if t == 0 { "Atk" } else { "Def" };
        if key == fnv1a_lowercase(&format!("Class{name}"))
            || key == fnv1a_lowercase(&format!("Class{stem}{col}"))
        {
            return Some(t as u8);
        }
    }
    None
}

/// Which integrator a vehicle base drives (re-vehicles §2-§4).
pub fn vehicle_kind(base: &str) -> Option<VehicleKind> {
    Some(match base.to_ascii_lowercase().as_str() {
        "hover" | "treaded" | "commandhover" => VehicleKind::Hover,
        "walker" | "commandwalker" => VehicleKind::Walker,
        "flyer" | "commandflyer" | "carrier" => VehicleKind::Flyer,
        "armedbuilding" | "armedbuildingdynamic" => VehicleKind::Turret,
        _ => return None,
    })
}

/// The pads of one map and the vehicle classes they name.
#[derive(Debug, Default)]
pub struct PadBuild {
    pub pads: Vec<PadDef>,
    /// The vehicle class table: `VehicleSpec::class` indexes it.
    pub classes: Vec<String>,
    /// Each class's `GeometryName` (the model the viewer draws, card
    /// VDRAW), parallel to `classes`.
    pub geometry: Vec<Option<String>>,
    /// Each class's seat ring with its cameras (card VSEAT), parallel to
    /// `classes`; filled by [`install`].
    pub seats: Vec<Vec<vehicle_class::SeatSpec>>,
    pub walker_shot_styles: std::collections::HashMap<usize, crate::walker_shots::Style>,
    pub flyer_exhaust: Vec<Vec<FlyerExhaust>>,
    pub weapon_hud: Vec<Vec<Vec<crate::walk::WeaponHud>>>,
    /// Each class's `EngineSound` and `TurnOffTime` (card VSND), parallel
    /// to `classes`; filled by [`install`].
    pub engines: Vec<Option<crate::engine_sound::EngineClass>>,
    /// Each class's sound properties (collision, hurt, death), parallel to `classes`.
    pub sounds: Vec<vehicle_class::VehicleSoundClass>,
    /// Each class's `Label` (BF1 names a command walker's post on the spawn
    /// screen by it, re-command-walker-post §5), parallel to `classes`;
    /// filled by [`install`].
    pub labels: Vec<Option<String>>,
    /// Each class's seats' rider hangs (card VRIDE), parallel to `seats`;
    /// filled by [`install`] from the baked `skel` nodes.
    pub riders: Vec<Vec<vehicle_class::RiderHang>>,
    /// (pad, class) for every class prop of a fielded side that names no
    /// vehicle class the db carries — the PS2 spawns nothing there.
    pub absent: Vec<(String, String)>,
    /// The building turrets, in manifest order (card VTURB): spawned by
    /// [`install`] before any pad vehicle.
    pub turrets: Vec<TurretDef>,
}

#[derive(Clone, Debug)]
pub struct FlyerExhaust {
    pub effect: String,
    pub local: glam::Mat4,
    pub node: Option<String>,
    pub offset: Vec3,
    pub scale: [f32; 3],
}

/// The PS2 keeps at most eight thrust effects per class (`FUN_00211a40`,
/// B2:124203-124223); a later `ThrustEffect` is dropped.
const MAX_THRUST_EFFECTS: usize = 8;

fn flyer_exhaust(db: &ClassDb, class: &str, nodes: &[ManifestHardpoint]) -> Vec<FlyerExhaust> {
    // `FUN_00211a40` reads these keys in ODF order, each applying to the most
    // recent `ThrustEffect` (B2:124203-125043).
    let (effect_key, point_key, offset_key) = (
        fnv1a_lowercase("ThrustEffect"),
        fnv1a_lowercase("ThrustAttachPoint"),
        fnv1a_lowercase("ThrustAttachOffset"),
    );
    let (min_scale_key, max_scale_key, start_scale_key) = (
        fnv1a_lowercase("ThrustEffectMinScale"),
        fnv1a_lowercase("ThrustEffectMaxScale"),
        fnv1a_lowercase("ThrustEffectScaleStart"),
    );
    let mut effects: Vec<(String, Option<String>, Vec3, [f32; 3])> = Vec::new();
    for def in db.chain(class).into_iter().rev() {
        for (key, value) in &def.props {
            if *key == effect_key && effects.len() < MAX_THRUST_EFFECTS {
                effects.push((value.trim().to_owned(), None, Vec3::ZERO, [1.0, 1.0, 0.0]));
            } else if let Some((_, node, offset, scale)) = effects.last_mut() {
                let slot = match *key {
                    k if k == min_scale_key => Some(0),
                    k if k == max_scale_key => Some(1),
                    k if k == start_scale_key => Some(2),
                    _ => None,
                };
                if let Some(i) = slot {
                    if let Ok(v) = value.trim().parse::<f32>() {
                        scale[i] = v;
                    }
                } else if *key == point_key {
                    *node = Some(value.trim().to_owned());
                } else if *key == offset_key {
                    let values: Vec<_> = value
                        .split_whitespace()
                        .filter_map(|x| x.parse::<f32>().ok())
                        .collect();
                    if let [x, y, z] = values.as_slice() {
                        *offset = Vec3::new(*x, *y, *z);
                    }
                }
            }
        }
    }
    effects
        .into_iter()
        .map(|(effect, node, offset, scale)| {
            let base = node
                .as_deref()
                .and_then(|name| nodes.iter().find(|n| n.name.eq_ignore_ascii_case(name)))
                .map_or(glam::Mat4::IDENTITY, |n| {
                    crate::viewer::instance_matrix(
                        n.axes.unwrap_or(glam::Mat3::IDENTITY.to_cols_array_2d()),
                        n.pos,
                    )
                });
            FlyerExhaust {
                effect,
                local: base * glam::Mat4::from_translation(offset),
                node,
                offset,
                scale,
            }
        })
        .collect()
}

/// One armed-building instance as a vehicle: its spec (at the instance,
/// `yaw` its turn) and the instance's full rotation, which the vehicle takes
/// so its fire points sit where the world draws the model.
#[derive(Clone, Debug)]
pub struct TurretDef {
    pub name: String,
    /// BF2 authored UnbuiltGeometryName: a repairable pad at battle start.
    pub unbuilt: bool,
    /// The instance's index in the manifest (its bodies' `Body::instance`).
    pub instance: u32,
    pub spec: VehicleSpec,
    pub rot: glam::Quat,
    /// What it falls to and that look's geometry
    /// ([`crate::turret_fall::fall_look`], card TBLD).
    pub fall: (core::sim::TurretFall, Option<String>),
    /// BF2 authored self-piloted seats; never boardable by soldiers.
    pub self_seats: Vec<u8>,
}

/// A building turret's acquisition sphere (card HTGT, `Vehicle::aim_sphere`):
/// the box around its building's bodies (`Body::instance`), as a centre in
/// the turret's frame (at `pos`, turned by `rot`) and half the box's
/// diagonal; `None` when the building has no body. The PS2 picks the
/// building by its bounds (`analysis/re-target-hud.md` §2).
pub fn building_sphere(
    world: &core::collision::CollisionWorld,
    instance: u32,
    pos: Vec3,
    rot: glam::Quat,
) -> Option<(Vec3, f32)> {
    let mut b = core::collision::Aabb::empty();
    for body in world
        .bodies()
        .iter()
        .filter(|b| b.instance == Some(instance) && b.layers.contains(core::collision::Layers::ALL))
    {
        b.add(body.aabb.min);
        b.add(body.aabb.max);
    }
    (!b.is_empty()).then(|| {
        let c = (b.min + b.max) * 0.5;
        (rot.inverse() * (c - pos), (b.max - b.min).length() * 0.5)
    })
}

/// The instance matrix (rows = the model's axes in the world, as
/// `viewer::instance_matrix` draws it) as a rotation; `None` when it is not
/// a proper rotation.
fn instance_rot(r: &[[f32; 3]; 3]) -> Option<glam::Quat> {
    let m = glam::Mat3::from_cols(r[0].into(), r[1].into(), r[2].into());
    (m.determinant() > 0.5).then(|| glam::Quat::from_mat3(&m).normalize())
}

/// A port team from an instance's `Team` (PS2 1, 2 → 0, 1; else none).
fn port_team(inst: &ManifestInstance) -> Option<u8> {
    prop(inst, "Team")
        .and_then(|s| s.trim().parse::<i32>().ok())
        .and_then(|t| match t {
            1 => Some(0),
            2 => Some(1),
            _ => None,
        })
}

fn prop<'a>(inst: &'a ManifestInstance, key: &str) -> Option<&'a str> {
    let h = fnv1a_lowercase(key);
    inst.props
        .iter()
        .rev()
        .find(|(k, _)| *k == h)
        .map(|(_, v)| v.as_str())
}

/// The instance's turn about +Y (identity = 0, the soldier's convention:
/// positive turns left). PROVISIONAL until a vehicle is drawn at its pad.
fn yaw_of(r: &[[f32; 3]; 3]) -> f32 {
    core::fmath::atan2(r[0][2], r[0][0])
}

/// The spec a pad builds for `class`, or `None` if the db has no vehicle by
/// that name.
fn spec_for(db: &ClassDb, class: &str, index: u32) -> Option<VehicleSpec> {
    let kind = vehicle_kind(db.engine_base(class)?)?;
    let seats = vehicle_class::seats(db, class)
        .iter()
        .map(|s| s.kind)
        .collect();
    Some(VehicleSpec {
        kind,
        class: index,
        home_team: None,
        neutral_when_empty: vehicle_class::neutral_when_empty(db, class),
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: db.prop_f32(class, "MaxHealth").unwrap_or(0.0),
        health_type: vehicle_class::health_type(db, class),
        seats,
    })
}

/// Build every pad of `m` for a battle of `factions` (port team order).
/// `cp_index` maps a `ControlZone` name to its `Sim::cps` index. A BF2 map
/// reads `SpawnCount` (BF1's setter has no such branch).
pub fn build(
    m: &Manifest,
    db: &ClassDb,
    factions: &[String; 2],
    cp_index: impl Fn(&str) -> Option<usize>,
) -> PadBuild {
    let bf2 = m.source_game.eq_ignore_ascii_case("bf2");
    let mut out = PadBuild::default();
    for inst in m.instances.iter().filter(|i| i.class == PAD_CLASS) {
        let mut classes: [Option<PadClass>; 2] = [None, None];
        // In prop order: a later key for the same team slot wins, as the
        // setter's store does.
        for (k, v) in &inst.props {
            let Some(t) = class_key_team(*k, factions) else {
                continue;
            };
            let name = v.trim().to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            let index = out
                .classes
                .iter()
                .position(|c| *c == name)
                .unwrap_or(out.classes.len());
            classes[t as usize] = match spec_for(db, &name, index as u32) {
                Some(spec) => {
                    if index == out.classes.len() {
                        out.geometry.push(
                            db.prop(&name, "GeometryName")
                                .map(|g| g.trim().to_owned())
                                .filter(|g| !g.is_empty()),
                        );
                        out.classes.push(name);
                    }
                    Some(PadClass {
                        spec,
                        radius: VEHICLE_RADIUS,
                        // Filled by `install`, which has the side bakes.
                        landed_height: None,
                        vis_box: None,
                    })
                }
                None => {
                    out.absent.push((inst.name.clone(), name));
                    None
                }
            };
        }
        // The load-time team (+0xfc) as a port team (PS2 1, 2 → 0, 1; 0, −1
        // and the locals are no team the port fields). `Team` (atoi) comes
        // first on every stock pad and `ControlZone` then copies its post's
        // team (B1:239915-239968) — the post's WORLD `Team`: the mission
        // re-owns posts after load, and the first tick's follow then
        // reloads the countdown (B1:240799). [H]: the copy needs the post to
        // exist when the pad's props run; 21 of 313 stock pads come before
        // their post in the manifest, and the port copies for them too.
        let zone = prop(inst, "ControlZone").filter(|z| !z.trim().is_empty());
        let post = zone.and_then(|z| {
            m.instances
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(z.trim()))
        });
        let team = port_team(post.unwrap_or(inst));
        let f = |key: &str, default: f32| {
            prop(inst, key)
                .and_then(|s| s.trim().parse::<f32>().ok())
                .unwrap_or(default)
        };
        out.pads.push(PadDef {
            name: inst.name.clone(),
            pos: Vec3::from(inst.translation),
            yaw: yaw_of(&inst.rotation),
            cp: zone.and_then(|z| cp_index(z.trim())),
            team,
            spawn_time: f("SpawnTime", SPAWN_TIME),
            decay_time: f("DecayTime", DECAY_TIME),
            spawn_count: if bf2 {
                f("SpawnCount", 1.0).max(1.0) as u32
            } else {
                1
            },
            classes,
        });
    }
    add_turrets(&mut out, &m.instances, db, bf2);
    out
}

/// The building turrets of `instances` (§8) into `out`: every instance whose
/// class (matched ignoring case, [`ClassDb::resolve`]) is an armed building
/// with at least one gunner seat — BF2 POL1's three shields are armed
/// buildings with none — at the instance, sharing `out`'s class table with
/// no viewer geometry. With `unbuilt_zones` (BF2) a buildable turret (a
/// `manifest::BUILD_ZONE_PROP`: `tur_bldg_laser` and kin) starts as a
/// repairable pad. BF1's `com_weap_gunturret` authors the prop too, and
/// whether it starts built is open (card BZONE1), so BF1 keeps spawning it.
pub fn add_turrets(
    out: &mut PadBuild,
    instances: &[ManifestInstance],
    db: &ClassDb,
    unbuilt_zones: bool,
) {
    for (ii, inst) in instances.iter().enumerate() {
        let Some(name) = db.resolve(&inst.class).map(str::to_owned) else {
            continue;
        };
        if !vehicle_class::is_turret(db, &name) {
            continue;
        }
        let index = out
            .classes
            .iter()
            .position(|c| *c == name)
            .unwrap_or(out.classes.len());
        let Some(mut spec) = spec_for(db, &name, index as u32) else {
            continue;
        };
        if spec.seats.is_empty() {
            continue;
        }
        let fall = crate::turret_fall::fall_look(db, &name);
        let unbuilt = unbuilt_zones && db.prop_hash(&name, manifest::BUILD_ZONE_PROP).is_some();
        if index == out.classes.len() {
            out.classes.push(name.clone());
            out.geometry.push(None);
        }
        spec.pos = Vec3::from(inst.translation);
        spec.yaw = yaw_of(&inst.rotation);
        spec.home_team = port_team(inst);
        let self_seats: Vec<u8> = if db.is_bf2() {
            vehicle_class::seats(db, &name)
                .iter()
                .enumerate()
                .filter(|(_, s)| s.pilot_type == vehicle_class::PilotType::SelfPilot)
                .map(|(i, _)| i as u8)
                .collect()
        } else {
            Vec::new()
        };
        if !self_seats.is_empty() {
            spec.neutral_when_empty = false;
        }
        out.turrets.push(TurretDef {
            name: inst.name.clone(),
            unbuilt,
            instance: ii as u32,
            fall,
            self_seats,
            rot: instance_rot(&inst.rotation).unwrap_or(core::vehicle::hover::quat_y(-spec.yaw)),
            spec,
        });
    }
}

/// Card TCOL: the bodies of building turret `t`'s instance that hang on a
/// node its gunner seat moves (`core::sim::PosedParts`): each of its class's
/// prims whose `PRNT` is the seat's turret, aimer or barrel node, found
/// among the instance's bodies by where `Scene::collision_world` put it
/// (through the node's bind). `None` when it has none.
pub fn posed_parts(
    sim: &core::sim::Sim,
    m: &Manifest,
    common: &crate::stations::Common,
    db: &ClassDb,
    t: &TurretDef,
    class: &str,
) -> Option<core::sim::PosedParts> {
    use crate::viewer::turret_pose as tp;
    let inst = m.instances.get(t.instance as usize)?;
    let row = m
        .classes
        .iter()
        .chain(common.1.iter().flat_map(|c| &c.classes))
        .find(|k| k.name.eq_ignore_ascii_case(&inst.class) || k.name.eq_ignore_ascii_case(class))?;
    let rig = row.turret_rig.as_ref()?;
    let geom = row.geometry.as_deref()?;
    let entry = m
        .collision
        .iter()
        .chain(common.1.iter().flat_map(|c| &c.collision))
        .find(|e| e.geometry.eq_ignore_ascii_case(geom))?;
    let (seat, spec) = vehicle_class::seats(db, class)
        .iter()
        .enumerate()
        .map(|(i, s)| (i, tp::PoseSpec::of(rig, s)))
        .find(|(_, p)| p.turret.is_some() || p.aimer.is_some())?;
    let core_rig = tp::core_rig(rig, &spec);
    let im = crate::viewer::instance_matrix(inst.rotation, inst.translation);
    let to_instance = glam::Mat4::from_rotation_translation(t.rot, t.spec.pos).inverse();
    let mut taken = Vec::new();
    let mut bodies = Vec::new();
    for p in &entry.prims {
        let Some((_, n)) = crate::assets::armed_node(row, &p.parent) else {
            continue;
        };
        if !core_rig.moves(n) {
            continue;
        }
        let Some(shape) = crate::assets::prim_shape(p) else {
            continue;
        };
        let local = crate::viewer::instance_matrix(p.rotation, p.translation);
        let want = core::collision::Body::new(im * tp::bind_of(rig, n) * local, shape).to_world;
        let found = sim
            .collision
            .bodies()
            .iter()
            .enumerate()
            .position(|(i, b)| {
                b.instance == Some(t.instance)
                    && !taken.contains(&i)
                    && b.to_world.abs_diff_eq(want, 1e-3)
            });
        if let Some(i) = found {
            taken.push(i);
            bodies.push(core::sim::PosedBody {
                body: i,
                node: n,
                rest: to_instance * sim.collision.bodies()[i].to_world,
            });
        }
    }
    (!bodies.is_empty()).then_some(core::sim::PosedParts {
        rig: core_rig,
        seat,
        bodies,
        at: None,
    })
}

/// Load the vehicle classes a battle of `factions` can spawn (the map's own
/// classes first, then the side files, then [`mission_side_files`],
/// `ClassDb::load` order), build `m`'s
/// pads with each `ControlZone` matched to a `sim.cps` post by name, and add
/// them to `sim` with each class's hover numbers (`Sim::hover_params`), then
/// spawn the map's building turrets ([`add_turrets`], card VTURB). Call AFTER
/// the posts are in (`WalkSim::add_posts`) and with the mission's conquest
/// layer merged (`Scene::apply_mode_layer`) — and BEFORE
/// `WalkSim::opening_wave`, which spawns at the command walkers these pads
/// are about to build (card CWSP).
pub fn install(
    sim: &mut core::sim::Sim,
    m: &Manifest,
    map_dir: &std::path::Path,
    classes_dir: &std::path::Path,
    factions: &[String; 2],
) -> anyhow::Result<PadBuild> {
    install_with_objective(sim, m, map_dir, classes_dir, factions, None)
}

pub fn install_with_objective(
    sim: &mut core::sim::Sim,
    m: &Manifest,
    map_dir: &std::path::Path,
    classes_dir: &std::path::Path,
    factions: &[String; 2],
    space: Option<(&crate::mission::SpaceAssaultDef, u32)>,
) -> anyhow::Result<PadBuild> {
    let mut db = ClassDb::default();
    db.load_map(map_dir)?;
    let game = m.source_game.to_ascii_lowercase();
    let f: Vec<&str> = factions.iter().map(String::as_str).collect();
    let map = map_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let extra = mission_side_files(&game, &map).iter().copied();
    for file in crate::roster::class_files(&game, &f)
        .into_iter()
        .chain(extra)
    {
        let p = classes_dir.join(file);
        if pak::exists(&p) {
            db.load(&p)?;
        }
    }
    let names: Vec<String> = sim.cps.iter().map(|c| c.def.name.clone()).collect();
    let mut b = build(m, &db, factions, |zone| {
        names.iter().position(|n| n.eq_ignore_ascii_case(zone))
    });
    // Each flyer class's LandedHeight (card VREST), before any pad spawns.
    let sides = side_vehicle_files(map_dir, &game);
    let floors = vis_floors_from(&sides);
    let landed: Vec<Option<f32>> = b
        .classes
        .iter()
        .map(|c| vehicle_class::landed_height(&db, c, |k| floors.get(k).copied()))
        .collect();
    let vis_boxes: HashMap<String, ([f32; 3], [f32; 3])> = sides
        .iter()
        .flat_map(|(_, vm)| &vm.vehicles)
        .filter_map(|v| Some((v.class.name.to_ascii_lowercase(), v.vis_box?)))
        .collect();
    for c in b
        .pads
        .iter_mut()
        .flat_map(|p| p.classes.iter_mut())
        .flatten()
    {
        c.landed_height = landed[c.spec.class as usize];
        c.vis_box = vis_boxes
            .get(&b.classes[c.spec.class as usize].to_ascii_lowercase())
            .map(|(lo, hi)| (Vec3::from(*lo), Vec3::from(*hi)));
    }
    for p in &b.pads {
        sim.add_vehicle_pad(p.clone());
    }
    // The health / ammo / vehicle droids (card PWST): the same db, the same
    // mode-merged instances.
    crate::roster::add_stations(sim, &m.instances, &db, game == "bf2");
    if let Some((def, limit)) = space {
        crate::space_assault::install(sim, def, limit, &m.instances, &db);
    }
    crate::stations::install_collision(sim, m, map_dir, &db)?;
    let common = crate::stations::load_common(m, map_dir)?;
    for t in &b.turrets {
        let sphere = building_sphere(&sim.collision, t.instance, t.spec.pos, t.rot);
        let class = &b.classes[t.spec.class as usize];
        let parts = match &t.fall.1 {
            Some(g) => crate::turret_fall::collision_parts(m, map_dir, &common, g, class)?,
            None => Vec::new(),
        };
        sim.add_building_turret(t.spec.clone(), t.rot, sphere, t.instance, (t.fall.0, parts));
        sim.set_building_self_pilots(t.instance, &t.self_seats);
        if let Some(posed) = posed_parts(sim, m, &common, &db, t, class) {
            sim.set_turret_posed(t.instance, posed);
        }
        if t.unbuilt {
            sim.start_building_unbuilt(t.instance);
        }
    }
    // The hover numbers per class (card HOVR): `Sim::tick_hovers` moves
    // every pad vehicle whose class is a hover.
    // Its hull bodies (card HHUL) are the class's named primitives, out of
    // its geometry's baked collision.
    sim.hover_params = b
        .classes
        .iter()
        .map(|c| {
            vehicle_class::HoverClass::from_db(&db, c).map(|h| core::vehicle::hover::HoverParams {
                bf1_ai: game == "bf1",
                hull: hover_hull_from(m, map_dir, &sides, c, &h),
                ..h.params()
            })
        })
        .collect();
    // The walker numbers per class (card WALK), with each class's baked
    // anim table and prim origins from the side bakes.
    let walkers = walker_bakes_from(&sides);
    sim.walker_params = b
        .classes
        .iter()
        .map(|c| {
            vehicle_class::WalkerClass::from_db(&db, c)
                .map(|w| w.params(walkers.get(&c.to_ascii_lowercase())))
        })
        .collect();
    sim.flyer_params = b
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| flyer_params(&db, c, game == "bf2", landed[i].unwrap_or(0.0)))
        .collect();
    // BF2's path Properties now survive the bake; older manifests use the
    // PS2 constructor defaults until rebaked (analysis/re-flyer-ai.md).
    sim.flyer_paths = flight_paths(m);
    // Bots fly (`core::ai::flyer_drive`). OPENBF_FLYER_BOTS=0 keeps them on the ground,
    // for a run measuring a map's ground war alone.
    sim.flyer_bots = std::env::var_os("OPENBF_FLYER_BOTS").is_none_or(|v| v != "0");
    // What the use input and the exit read (card VSEAT).
    b.seats = b
        .classes
        .iter()
        .map(|c| vehicle_class::seats(&db, c))
        .collect();
    b.engines = b
        .classes
        .iter()
        .map(|c| crate::engine_sound::EngineClass::from_db(&db, c))
        .collect();
    b.sounds = b
        .classes
        .iter()
        .map(|c| vehicle_class::VehicleSoundClass::from_db(&db, c))
        .collect();
    b.labels = b
        .classes
        .iter()
        .map(|c| {
            db.prop(c, "Label")
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty())
        })
        .collect();
    // Each class's death explosion and blast sphere (card VDTH).
    // A building turret's model is the map's (or its game's common bake's),
    // not a side's: its sphere is that model's, as a station's is
    // (`stations::geometry_sphere`).
    let spheres = spheres_from(&sides);
    let common = crate::stations::load_common(m, map_dir)?;
    let mut setups = Vec::with_capacity(b.classes.len());
    for c in &b.classes {
        let mut sphr = spheres.get(&c.to_ascii_lowercase()).copied();
        if sphr.is_none() && vehicle_class::is_turret(&db, c) {
            if let Some(g) = db.prop(c, "GeometryName").map(str::trim) {
                sphr = crate::stations::geometry_sphere(m, map_dir, &common, &db, g, c)?;
            }
        }
        setups.push(Some(blast_setup(&db, c, sphr)));
    }
    sim.blast_setup = setups;
    let damage_nodes = hardpoints_from(m, &sides);
    b.flyer_exhaust = b
        .classes
        .iter()
        .map(|c| {
            if db.engine_base(c).and_then(vehicle_kind) != Some(VehicleKind::Flyer) {
                return Vec::new();
            }
            flyer_exhaust(
                &db,
                c,
                damage_nodes
                    .get(&c.to_ascii_lowercase())
                    .map_or(&[], Vec::as_slice),
            )
        })
        .collect();
    sim.vehicle_damage_effects =
        b.classes
            .iter()
            .map(|c| {
                let nodes = damage_nodes.get(&c.to_ascii_lowercase());
                vehicle_class::damage_effects(&db, c)
                    .into_iter()
                    .map(|effect| {
                        let node = effect.attach.as_deref().and_then(|name| {
                            nodes?.iter().find(|h| h.name.eq_ignore_ascii_case(name))
                        });
                        core::sim::StationDamageEffect {
                            local: node.map_or(glam::Mat4::IDENTITY, |h| {
                                crate::viewer::instance_matrix(
                                    h.axes.unwrap_or(glam::Mat3::IDENTITY.to_cols_array_2d()),
                                    h.pos,
                                )
                            }),
                            scale: effect.scale,
                            inherit: effect.inherit_velocity,
                            band: core::autoturret::DamageEffect {
                                start: effect.start,
                                stop: effect.stop,
                                effect: effect.effect,
                            },
                        }
                    })
                    .collect()
            })
            .collect();
    // Issue #35: each class's slice time.
    sim.slice_time = b
        .classes
        .iter()
        .map(|c| vehicle_class::slice_time(&db, c))
        .collect();
    // Each class's Technician repair rate (`Sim::vehicle_regen`).
    sim.repair_scale = b
        .classes
        .iter()
        .map(|c| vehicle_class::repair_scale(&db, c))
        .collect();
    // Each class's weak-spot factor on a ground map (x4 the big walkers).
    sim.weak_spot_factor = b
        .classes
        .iter()
        .map(|c| vehicle_class::weak_spot_factor(&db, c))
        .collect();
    // Each class's hull (card VHULL), before anything spawns from a pad; a
    // building turret has none — its building is the world's own body.
    sim.hull_parts = hull_parts_from(
        m,
        map_dir,
        &sides,
        &b.classes,
        |c| vehicle_class::is_turret(&db, c),
        |c| crate::assets::hit_locations(&db, c),
        |c| crate::assets::added_weak_parts(&db, c),
    );
    sim.sync_hulls();
    sim.missile_target_points = b
        .classes
        .iter()
        .map(|c| {
            let Some((dir, k, entry)) = class_collision(m, map_dir, &sides, c) else {
                return Vec::new();
            };
            let Ok(cg) = crate::assets::load_collision_geom(dir, &entry) else {
                return Vec::new();
            };
            crate::assets::missile_target_points(&cg, &k.collision)
        })
        .collect();
    let nodes = hardpoints_from(m, &sides);
    b.riders = b
        .classes
        .iter()
        .zip(&b.seats)
        .map(|(c, ring)| {
            let hp = nodes
                .get(&c.to_ascii_lowercase())
                .map_or(&[][..], Vec::as_slice);
            ring.iter()
                .map(|s| vehicle_class::RiderHang::of(s, hp))
                .collect()
        })
        .collect();
    b.weapon_hud = b
        .seats
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|seat| {
                    seat.weapons
                        .iter()
                        .map(|w| crate::roster::weapon_hud(&db, &w.class))
                        .collect()
                })
                .collect()
        })
        .collect();
    sim.seat_setup = b
        .classes
        .iter()
        .zip(&b.seats)
        .map(|(c, ring)| {
            let mut setup = seat_setup(&db, c, ring, &game);
            let empty = Vec::new();
            let hp = nodes.get(&c.to_ascii_lowercase()).unwrap_or(&empty);
            let building = vehicle_class::is_turret(&db, c);
            let walker = db.engine_base(c).and_then(vehicle_kind) == Some(VehicleKind::Walker);
            setup.guns = ring
                .iter()
                .map(|s| {
                    let mut g = seat_gun(sim, &db, s, hp)?;
                    if (building || walker) && !g.pilot {
                        turret_law(&mut g, s, game == "bf2");
                        if building {
                            g.gunner = gunner_seat(m, c, s, game == "bf2");
                        }
                        if walker && !g.framed {
                            if let Some(axes) = s
                                .aimer_node
                                .as_deref()
                                .and_then(|n| hp.iter().find(|h| h.name.eq_ignore_ascii_case(n)))
                                .and_then(|h| h.axes)
                            {
                                g.aimer_bind =
                                    glam::Quat::from_mat3(&glam::Mat3::from_cols_array_2d(&axes))
                                        .normalize();
                            }
                        }
                    } else if walker {
                        g.aimer = aimer(
                            s.aimer_yaw_limits.or(s.yaw_limits),
                            s.aimer_pitch_limits.or(s.pitch_limits),
                            s.aimer_yaw_rate.or(s.yaw_rate),
                            s.aimer_pitch_rate.or(s.pitch_rate),
                        );
                    }
                    Some(g)
                })
                .collect();
            Some(setup)
        })
        .collect();
    for (setup, blast) in sim.seat_setup.iter_mut().zip(&sim.blast_setup) {
        if let (Some(setup), Some(blast)) = (setup.as_mut(), blast.as_ref()) {
            if blast.sphere_radius > 0.0 {
                setup.radius = blast.sphere_radius;
            }
        }
    }
    let tow_parts = b
        .classes
        .iter()
        .map(|class| {
            let rows = db.props_all(class, "TowCableCollision");
            if rows.is_empty() {
                return Vec::new();
            }
            class_collision(m, map_dir, &sides, class)
                .and_then(|(dir, class, entry)| {
                    let collision = crate::assets::load_collision_geom(dir, &entry).ok()?;
                    crate::hero_riders::tow_parts(&rows, &collision, &class.collision)
                })
                .unwrap_or_default()
        })
        .collect();
    crate::hero_riders::install(sim, m, map_dir, &b, tow_parts);
    for (class, setup) in sim
        .seat_setup
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|s| (i, s)))
    {
        if !matches!(
            db.engine_base(&b.classes[class]).and_then(vehicle_kind),
            Some(VehicleKind::Walker | VehicleKind::Flyer)
        ) {
            continue;
        }
        for w in setup.guns.iter().flatten().flat_map(|g| &g.weapons) {
            let name = &sim.weapon_classes[w.class].name;
            let tracer = db.prop(name, "SecondaryOrdnanceName").map(|c| {
                let mut look = core::laser::LaserLook::replay(crate::roster::chain_props(&db, c));
                let range = db.prop_f32(c, "Range").unwrap_or(40.0);
                look.length = range;
                look.glow_length = range;
                look.blur_length = range;
                look
            });
            let frequency = db
                .prop_f32(name, "SecondaryOrdnancePeriod")
                .unwrap_or(1.0)
                .max(1.0) as u64;
            let fire_sound = db.prop_hash(name, 0x2298cae8).map(str::to_owned);
            b.walker_shot_styles.insert(
                w.class,
                crate::walker_shots::Style {
                    tracer,
                    frequency,
                    fire_sound,
                },
            );
        }
    }
    Ok(b)
}

/// Where a building turret's drawn gunner stands (`core::vehicle::gun::GunnerSeat`):
/// his `PilotPosition` on its parent node of the class's rig; `None` for a
/// hidden gunner (he stays at the origin) or a class with no rig.
fn gunner_seat(
    m: &Manifest,
    class: &str,
    s: &vehicle_class::SeatSpec,
    bf2: bool,
) -> Option<core::vehicle::gun::GunnerSeat> {
    use crate::viewer::turret_pose as tp;
    if s.gunner_draw(bf2) != vehicle_class::GunnerDraw::Drawn {
        return None;
    }
    // The class as the scene resolves it (a BF2 build zone's built class).
    let find = |n: &str| m.classes.iter().find(|k| k.name.eq_ignore_ascii_case(n));
    let k = find(class)?;
    let k = match &k.build_zone {
        Some(zone) if bf2 => find(zone)?,
        _ => k,
    };
    let rig = k.turret_rig.as_ref()?;
    let hang = tp::GunnerHang::of(rig, s)?;
    let node_bind = hang
        .parent
        .map_or(glam::Mat4::IDENTITY, |p| tp::bind_of(rig, p));
    Some(core::vehicle::gun::GunnerSeat {
        node_bind,
        local: hang.local,
        turns: hang.parent.is_some() && hang.parent == tp::PoseSpec::of(rig, s).turret,
        pitch_factor: s.pitch_turn_factor.unwrap_or(1.0),
    })
}

/// Every vehicle class's baked `skel` nodes (card VTUR), by lowercase class
/// name: the map's own classes (the TAT1 skiff), then each side's
/// `side_<game>/vehicles/*/vehicles.ron` beside the map's bake — the first
/// that has the class wins.
pub fn hardpoints(
    m: &Manifest,
    map_dir: &std::path::Path,
    game: &str,
) -> HashMap<String, Vec<ManifestHardpoint>> {
    hardpoints_from(m, &side_vehicle_files(map_dir, game))
}

fn hardpoints_from(
    m: &Manifest,
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
) -> HashMap<String, Vec<ManifestHardpoint>> {
    let mut out: HashMap<String, Vec<ManifestHardpoint>> = HashMap::new();
    let mut add = |k: &manifest::ManifestClass| {
        if !k.hardpoints.is_empty() {
            out.entry(k.name.to_ascii_lowercase())
                .or_insert_with(|| k.hardpoints.clone());
        }
    };
    m.classes.iter().for_each(&mut add);
    sides
        .iter()
        .flat_map(|(_, vm)| &vm.vehicles)
        .for_each(|v| add(&v.class));
    out
}

/// Every vehicle class's baked `modl SPHR` sphere (card VDTH), by lowercase
/// class name, from the side bakes as [`hardpoints`] walks them. A map's own
/// class (the TAT1 skiff) has none: the map bake does not write it.
pub fn spheres(map_dir: &std::path::Path, game: &str) -> HashMap<String, [f32; 4]> {
    spheres_from(&side_vehicle_files(map_dir, game))
}

fn spheres_from(
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
) -> HashMap<String, [f32; 4]> {
    let mut out = HashMap::new();
    for v in sides.iter().flat_map(|(_, vm)| &vm.vehicles) {
        if let Some(s) = v.sphere {
            out.entry(v.class.name.to_ascii_lowercase()).or_insert(s);
        }
    }
    out
}

/// Every vehicle class's baked vis-box floor (`ManifestVehicle::vis_min_y`,
/// card VREST), by lowercase class name, from the side bakes as
/// [`hardpoints`] walks them. A map's own class has none.
pub fn vis_floors(map_dir: &std::path::Path, game: &str) -> HashMap<String, f32> {
    vis_floors_from(&side_vehicle_files(map_dir, game))
}

fn vis_floors_from(
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
) -> HashMap<String, f32> {
    let mut out = HashMap::new();
    for v in sides.iter().flat_map(|(_, vm)| &vm.vehicles) {
        if let Some(y) = v.vis_min_y {
            out.entry(v.class.name.to_ascii_lowercase()).or_insert(y);
        }
    }
    out
}

/// Every walker class's baked anim table and prim origins (card WALK), by
/// lowercase class name, from the side bakes as [`hardpoints`] walks them.
pub fn walker_bakes(
    map_dir: &std::path::Path,
    game: &str,
) -> HashMap<String, manifest::ManifestWalker> {
    walker_bakes_from(&side_vehicle_files(map_dir, game))
}

fn walker_bakes_from(
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
) -> HashMap<String, manifest::ManifestWalker> {
    let mut out = HashMap::new();
    for v in sides.iter().flat_map(|(_, vm)| &vm.vehicles) {
        if let Some(w) = &v.walker {
            out.entry(v.class.name.to_ascii_lowercase())
                .or_insert_with(|| w.clone());
        }
    }
    out
}

/// `class`'s manifest row and its geometry's baked collision entry with the
/// bake directory it is in: the map's own bake first (the TAT1 skiff), then
/// the first side bake that has the class.
fn class_collision<'a>(
    m: &'a Manifest,
    map_dir: &'a std::path::Path,
    sides: &'a [(std::path::PathBuf, manifest::VehicleManifest)],
    class: &str,
) -> Option<(
    &'a std::path::Path,
    &'a manifest::ManifestClass,
    manifest::ManifestCollision,
)> {
    let geom_of = |k: &manifest::ManifestClass, cols: &[manifest::ManifestCollision]| {
        let g = k.geometry.as_deref()?;
        cols.iter()
            .find(|e| e.geometry.eq_ignore_ascii_case(g))
            .cloned()
    };
    let own = m
        .classes
        .iter()
        .find(|k| k.name.eq_ignore_ascii_case(class))
        .and_then(|k| Some((map_dir, k, geom_of(k, &m.collision)?)));
    own.or_else(|| {
        sides.iter().find_map(|(dir, vm)| {
            let k = vm
                .vehicles
                .iter()
                .map(|v| &v.class)
                .find(|k| k.name.eq_ignore_ascii_case(class))?;
            Some((dir.as_path(), k, geom_of(k, &vm.collision)?))
        })
    })
}

/// A hover class's hull bodies (card HHUL, `HoverClass::hull`) out of its
/// geometry's baked primitives; none for a class that names only its
/// springs — every shipped hover but the two BF2 fighter tanks — which
/// loads nothing.
pub fn hover_hull(
    m: &Manifest,
    map_dir: &std::path::Path,
    game: &str,
    class: &str,
    h: &vehicle_class::HoverClass,
) -> Vec<core::vehicle::hover::HullBody> {
    if !h.names_a_hull_body() {
        return Vec::new();
    }
    hover_hull_from(m, map_dir, &side_vehicle_files(map_dir, game), class, h)
}

fn hover_hull_from(
    m: &Manifest,
    map_dir: &std::path::Path,
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
    class: &str,
    h: &vehicle_class::HoverClass,
) -> Vec<core::vehicle::hover::HullBody> {
    if !h.names_a_hull_body() {
        return Vec::new();
    }
    class_collision(m, map_dir, sides, class)
        .map(|(_, _, entry)| h.hull(&entry.prims))
        .unwrap_or_default()
}

/// Every class's hull parts (card VHULL, `crate::assets::hull_parts`),
/// indexed like `classes`: the class's collision lists and its geometry's
/// baked collision from the map's own bake first (the TAT1 skiff), then the
/// first side bake that has the class. `None` for a turret (`turret`), a
/// class with no baked collision, or one whose bodies are in no subset.
pub fn hull_parts(
    m: &Manifest,
    map_dir: &std::path::Path,
    game: &str,
    classes: &[String],
    turret: impl Fn(&str) -> bool,
) -> Vec<Option<std::sync::Arc<[core::collision::HullPart]>>> {
    hull_parts_from(
        m,
        map_dir,
        &side_vehicle_files(map_dir, game),
        classes,
        turret,
        |_| Vec::new(),
        |_| Vec::new(),
    )
}

fn hull_parts_from(
    m: &Manifest,
    map_dir: &std::path::Path,
    sides: &[(std::path::PathBuf, manifest::VehicleManifest)],
    classes: &[String],
    turret: impl Fn(&str) -> bool,
    hit_locations: impl Fn(&str) -> Vec<(String, f32)>,
    added: impl Fn(&str) -> Vec<core::collision::HullPart>,
) -> Vec<Option<std::sync::Arc<[core::collision::HullPart]>>> {
    classes
        .iter()
        .map(|c| {
            if turret(c) {
                return None;
            }
            let (dir, k, entry) = class_collision(m, map_dir, sides, c)?;
            let cg = crate::assets::load_collision_geom(dir, &entry).ok()?;
            let mut parts = if m.source_game.eq_ignore_ascii_case("bf2")
                && vehicle_kind(&k.base) == Some(VehicleKind::Flyer)
            {
                crate::assets::bf2_flyer_hull_parts(&cg, &k.collision, &hit_locations(c))
            } else {
                crate::assets::hull_parts(&cg, &k.collision, &hit_locations(c))
            };
            // After the model's own, so their order (and indices) stay.
            if !parts.is_empty() {
                parts.extend(added(c));
            }
            (!parts.is_empty()).then(|| parts.into())
        })
        .collect()
}

/// Each `side_<game>/vehicles/*/vehicles.ron` beside the map's bake, in path
/// order: its directory and its manifest.
pub fn side_vehicle_files(
    map_dir: &std::path::Path,
    game: &str,
) -> Vec<(std::path::PathBuf, manifest::VehicleManifest)> {
    let root = map_dir
        .parent()
        .map(|p| p.join(format!("side_{game}")).join("vehicles"));
    let mut files: Vec<std::path::PathBuf> = root
        .and_then(|r| pak::read_dir(r).ok())
        .into_iter()
        .flatten()
        .map(|d| d.join("vehicles.ron"))
        .filter(|p| pak::is_file(p))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        let Ok(text) = pak::read_to_string(&f) else {
            continue;
        };
        if let Ok(vm) = ron::from_str::<manifest::VehicleManifest>(&text) {
            let dir = f
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            out.push((dir, vm));
        }
    }
    out
}

/// A seat's [`GunSetup`] (card VTUR, re-vehicles §5, §9): its weapons
/// registered in `sim` (once per name), its `FirePointName` and
/// `TurretNodeName` found among the class's nodes `hp`, its aimer from the
/// PS2's defaults overridden by the seat's limits (degrees → radians) and
/// rates. `None` for a seat with no weapon the db can build.
///
/// PROVISIONAL: a seat with no `FirePointName` (BF1 YAV1's tower turret,
/// HOT1's hoth turret) fires from the class's `hp_fire` node — the name the
/// PS2 gives a weapon's own fire node by default (the weapon class setter
/// B1 FUN_002864f0, B1:210168) — else from the turret node, then the model
/// origin; a missing turret node falls back to the fire point.
pub fn seat_gun(
    sim: &mut core::sim::Sim,
    db: &ClassDb,
    s: &vehicle_class::SeatSpec,
    hp: &[ManifestHardpoint],
) -> Option<GunSetup> {
    let node = |n: &str| {
        hp.iter()
            .find(|h| h.name.eq_ignore_ascii_case(n.trim()))
            .map(|h| Vec3::from(h.pos))
    };
    let weapons: Vec<GunWeapon> = s
        .weapons
        .iter()
        .take(MAX_SEAT_WEAPONS)
        .enumerate()
        .filter_map(|(weapon, w)| {
            let class = match sim
                .weapon_classes
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(&w.class))
            {
                Some(i) => i,
                None => match crate::roster::weapon_from_db(db, &w.class) {
                    Ok(mut wc) => {
                        if !sim.space_round() {
                            crate::normalize::vehicles::ground_gun(db, &mut wc);
                        }
                        sim.add_weapon_class(wc)
                    }
                    Err(e) => {
                        eprintln!("seat weapon {}: {e:#}", w.class);
                        return None;
                    }
                },
            };
            Some(GunWeapon {
                class,
                infinite: w.ammo < 1,
                reserve_clips: (w.ammo - 1).max(0) as u32,
                fire_points: s
                    .aimers
                    .iter()
                    .filter(|a| a.weapon == weapon)
                    .flat_map(|a| {
                        if a.fire_points.is_empty() {
                            node(&a.node).into_iter().collect::<Vec<_>>()
                        } else {
                            a.fire_points.iter().filter_map(|n| node(n)).collect()
                        }
                    })
                    .collect(),
            })
        })
        .collect();
    if weapons.is_empty() {
        return None;
    }
    let turret = s.turret_node.as_deref().and_then(node);
    let fire_point = s
        .fire_point
        .as_deref()
        .and_then(node)
        .or_else(|| s.fire_point.is_none().then(|| node("hp_fire")).flatten())
        .or(turret)
        .unwrap_or(Vec3::ZERO);
    let mut aimer = aimer(s.yaw_limits, s.pitch_limits, s.yaw_rate, s.pitch_rate);
    let pilot = s.kind == vehicle_class::SeatKind::Pilot;
    if pilot {
        // The hover pilot's aim-yaw clamp, K+0x690/+0x694 = 0/0 (re-hover §5.1 I11).
        aimer.yaw = [0.0, 0.0];
    }
    Some(GunSetup {
        weapons,
        fire_point,
        pivot: turret.unwrap_or(fire_point),
        eye: s.eye_point_offset.map_or(Vec3::ZERO, Vec3::from),
        aimer,
        pilot,
        turn: None,
        framed: false,
        aimer_bind: glam::Quat::IDENTITY,
        gunner: None,
    })
}

/// A building turret's gun under the PS2's MountedTurret law (card VTURL,
/// `analysis/re-turret-draw.md` 2.2, 2.3 [O]; `core::vehicle::gun::Turn`):
/// the look by `YawLimits` / `PitchLimits` (turret pitch + = nose down),
/// the full-stick speeds and filters (class defaults 2.0 / 0.5 / 10, BF1
/// exponential, BF2 linear), and the aimer from the `Aimer*` keys (defaults
/// ±30° / ±45°, 2.0 / 1.0 rad/s), in the aim frame when the section names a
/// `TurretNodeName`.
pub fn turret_law(g: &mut GunSetup, s: &vehicle_class::SeatSpec, bf2: bool) {
    use core::vehicle::gun::{Slew, Turn, PITCH_SPEED, TURN_FILTER, TURN_SPEED};
    g.turn = Some(Turn::from_class(
        s.yaw_limits,
        s.pitch_limits,
        (
            s.turn_speed.unwrap_or(TURN_SPEED),
            s.pitch_speed.unwrap_or(PITCH_SPEED),
        ),
        (
            s.turn_filter.unwrap_or(TURN_FILTER),
            s.pitch_filter.unwrap_or(TURN_FILTER),
        ),
        if bf2 { Slew::Linear } else { Slew::Exp },
    ));
    g.aimer = aimer(
        s.aimer_yaw_limits,
        s.aimer_pitch_limits,
        s.aimer_yaw_rate,
        s.aimer_pitch_rate,
    );
    g.framed = s.turret_node.is_some();
}

/// A class's [`BlastSetup`] (card VDTH): its `ExplosionName` class read as
/// an [`ExplosionClass`], and [`vehicle_class::blast_sphere`] over the baked
/// SPHR `sphr`.
pub fn blast_setup(db: &ClassDb, class: &str, sphr: Option<[f32; 4]>) -> BlastSetup {
    let explosion = vehicle_class::explosion_name(db, class)
        .filter(|e| !db.chain(e).is_empty())
        .map(|e| ExplosionClass {
            look: crate::roster::explosion_look(db, &e).unwrap_or_default(),
            ..ExplosionClass::from_props(&crate::roster::ClassSrc { db, class: &e })
        });
    let (c, w) = vehicle_class::blast_sphere(db, class, sphr);
    BlastSetup {
        explosion,
        sphere_centre: Vec3::from(c),
        sphere_radius: w,
    }
}

/// A class's [`SeatSetup`]: a seat boards a soldier for PilotType `vehicle`
/// or `vehicleself` (re-vehicles §5); the exit's ground ray is BF1's 10 m or
/// BF2's 15 m; the radius starts from the hover's spring spheres, else
/// [`VEHICLE_RADIUS`]; the install then replaces it with the class's collision
/// sphere (FUN_001645a0, [`vehicle_class::blast_sphere`]).
pub fn seat_setup(
    db: &ClassDb,
    class: &str,
    ring: &[vehicle_class::SeatSpec],
    game: &str,
) -> SeatSetup {
    use vehicle_class::PilotType;
    let radius = vehicle_class::HoverClass::from_db(db, class)
        .map(|h| {
            h.springs()
                .iter()
                .map(|s| s.pos.length() + s.radius)
                .fold(0.0f32, f32::max)
        })
        .filter(|r| *r > 0.0)
        .unwrap_or(VEHICLE_RADIUS);
    SeatSetup {
        radius,
        boardable: ring
            .iter()
            .map(|s| matches!(s.pilot_type, PilotType::Vehicle | PilotType::VehicleSelf))
            .collect(),
        body_active: ring
            .iter()
            .map(|s| {
                s.pilot_exposed
                    && s.gunner_draw(game.eq_ignore_ascii_case("bf2"))
                        == vehicle_class::GunnerDraw::Drawn
            })
            .collect(),
        exit_drop: if game.eq_ignore_ascii_case("bf2") {
            15.0
        } else {
            10.0
        },
        ai_size: vehicle_class::ai_size(db, class),
        guns: Vec::new(),
    }
}

/// Apply the optional authored limits and rates without changing default fields.
fn aimer(
    yaw_limits: Option<[f32; 2]>,
    pitch_limits: Option<[f32; 2]>,
    yaw_rate: Option<f32>,
    pitch_rate: Option<f32>,
) -> Aimer {
    let d = std::f32::consts::PI / 180.0;
    let lim = |l: [f32; 2]| [l[0].min(l[1]) * d, l[0].max(l[1]) * d];
    let mut aimer = Aimer::default();
    if let Some(l) = yaw_limits {
        aimer.yaw = lim(l);
    }
    if let Some(l) = pitch_limits {
        aimer.pitch = lim(l);
    }
    if let Some(r) = yaw_rate {
        aimer.yaw_rate = r;
    }
    if let Some(r) = pitch_rate {
        aimer.pitch_rate = r;
    }
    aimer
}

#[cfg(test)]
mod tests;
