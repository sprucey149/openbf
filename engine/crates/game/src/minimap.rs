//! The minimap's feed (card BMAP, `analysis/re-hud-map.md` §8): what a map
//! and its classes fix once ([`MapStatic`]: the whole-map square, the image,
//! each post's and class's icon, the view radii, the north pointer's turn)
//! and the per-frame [`ui::hud_map::MapModel`] built from the battle
//! ([`MapStatic::model`]). Presentation only: the sim never reads it.

use crate::classdb::ClassDb;
use crate::walk::WalkSim;
use core::fmath::FloatExt;
use glam::Vec3;
use manifest::{Manifest, ManifestRegion};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ui::hud_map::{heading, post_color, view_radius, MapIcon, MapModel, VIEW_DEFAULT};
use ui::hud_map::{ENEMY, FRIEND, NEUTRAL};

/// Class keys by hash (`FUN_001e2610`, re-hud-map §2.1 [V]).
const MAP_TEXTURE: u32 = 2_646_838_656;
const MAP_SCALE: u32 = 244_072_421;
const MAP_VIEW: [u32; 4] = [
    2_515_460_992, // MapViewMin
    2_883_288_682, // MapViewMax
    2_139_769_856, // MapSpeedMin
    2_507_597_546, // MapSpeedMax
];
/// A command post instance's `HUDIndex`.
const HUD_INDEX: u32 = 1_723_652_842;

/// The enemy-detection rule, PROVISIONAL (re-hud-map §3: `FUN_003a64b8`'s
/// class defaults, their keys unrecovered): always inside `NEAR` metres,
/// inside `FAR` within `CONE` of the view, or while firing.
const NEAR: f32 = 15.0;
const FAR: f32 = 60.0;
const CONE_COS: f32 = 0.913_545_5; // cos 24°
/// `[ours]`, PROVISIONAL: "while the unit fires" = a shot within this many seconds.
const FIRING_SECONDS: f32 = 0.5;

pub fn carrier_marker(
    sim: &core::sim::Sim,
    carrier: core::unit::UnitHandle,
    viewer: core::unit::UnitHandle,
) -> Option<(Vec3, bool)> {
    let unit = sim.unit(carrier)?;
    if !unit.alive {
        return None;
    }
    let friendly = unit.team == sim.unit(viewer)?.team;
    if let Some(mount) = unit.mount {
        let vehicle = sim
            .vehicles
            .get(mount.vehicle.index as usize)
            .filter(|v| v.gen == mount.vehicle.gen && v.alive)?;
        let radius = sim
            .blast_setup
            .get(vehicle.class as usize)
            .and_then(Option::as_ref)
            .map(|b| b.sphere_radius)
            .or_else(|| {
                sim.seat_setup
                    .get(vehicle.class as usize)
                    .and_then(Option::as_ref)
                    .map(|s| s.radius)
            })?;
        let offset = if carrier == viewer {
            Vec3::new(0.0, 0.3, -0.2 * radius)
        } else {
            Vec3::Y * (0.7 * radius).fmin(4.0)
        };
        Some((vehicle.pos + vehicle.rot * offset, friendly))
    } else {
        Some((
            unit.soldier.pos + Vec3::Y * (0.7 * unit.capsule().height).fmin(4.0),
            friendly,
        ))
    }
}

pub fn ctf_marker(sim: &core::sim::Sim, flag: &core::sim::Flag, team: u8) -> Option<(Vec3, bool)> {
    match flag.status {
        core::sim::FlagStatus::AtHome | core::sim::FlagStatus::Dropped { .. } => {
            Some((flag.current_pos, false))
        }
        core::sim::FlagStatus::Carried { carrier } => {
            if sim.unit(carrier)?.team != team {
                return None;
            }
            Some((
                sim.ctf
                    .as_ref()?
                    .capture_regions
                    .get(usize::from(team))?
                    .pos,
                true,
            ))
        }
    }
}

/// A class's icon and view radii.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassIcon {
    /// `MapTexture`; `None` = the class draws nothing on the map.
    pub texture: Option<String>,
    pub scale: f32,
    pub view: [f32; 4],
}

impl ClassIcon {
    fn of(db: &ClassDb, class: &str) -> Self {
        let mut view = VIEW_DEFAULT;
        for (v, key) in view.iter_mut().zip(MAP_VIEW) {
            if let Some(x) = db.prop_hash_f32(class, key) {
                *v = x;
            }
        }
        Self {
            texture: db
                .prop_hash(class, MAP_TEXTURE)
                .map(str::to_ascii_lowercase),
            scale: db.prop_hash_f32(class, MAP_SCALE).unwrap_or(1.0),
            view,
        }
    }
}

/// A remote weapon's droid icon: the class its `OrdnanceName` names.
/// `[ours]` `troop_icon` 1.0 (the discs' value) when the class is not at hand.
pub fn droid_icon(db: &ClassDb, remote: &str) -> ClassIcon {
    let mut icon = db
        .prop(remote, "OrdnanceName")
        .and_then(|o| db.resolve(o.trim()))
        .map_or_else(|| ClassIcon::of(db, ""), |c| ClassIcon::of(db, c));
    if icon.texture.is_none() {
        icon.texture = Some("troop_icon".into());
        icon.scale = 1.0;
    }
    icon
}

#[derive(Debug, Clone, Default)]
pub struct MapStatic {
    /// The map's own `textures` directory (the image's sheet).
    pub sheet_dir: PathBuf,
    /// `<world>_map`, when the bake holds it.
    pub image: Option<String>,
    /// The `mapbounds` square (centre x, z; signed half side), when the map
    /// has that region; else the posts' box + 100 m, taken per frame.
    pub bounds: Option<([f32; 2], f32)>,
    /// The Small view's north pointer turn, radians.
    pub north: f32,
    /// The Large / Spawn view's turn: the mission's north angle, flag or
    /// not.
    pub turn: f32,
    /// Per post name: icon and number.
    pub posts: HashMap<String, (ClassIcon, Option<u32>)>,
    /// Per team: the soldiers' icon.
    pub soldiers: Vec<ClassIcon>,
    /// By `Vehicle::class`.
    pub vehicles: Vec<ClassIcon>,
    /// A remote droid's icon by its `Droid::weapon_class`: the droid
    /// class's own `MapTexture` / `MapScale` (`troop_icon` 1.0 on both discs,
    /// `analysis/re-remote-droid.md` §8).
    pub droids: HashMap<usize, ClassIcon>,
}

/// The whole-map square from a `mapbounds` region (`FUN_003a7af0`,
/// re-hud-map §2.3 [V]): half sides row0.x · size.x and row2.z · size.z of
/// the region's matrix AS THE PS2 HOLDS IT, the larger SIGNED one the radius.
/// The PS2's loader keeps a box whose stored rotation is the munge's plain
/// diag(−1, 1, −1) as an axis-aligned box with the identity, and any other as
/// stored — [M] PS2 RAM on all 16 BF2 maps (UTA1's 0.04° yaw: identity;
/// TAN1's 2.3° tilt: as stored, both halves negative, the image mirrored).
/// PROVISIONAL: the threshold between those two is not read.
pub fn bounds_of(r: &ManifestRegion) -> ([f32; 2], f32) {
    const D: [[f32; 3]; 3] = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]];
    let plain = (0..3).all(|i| (0..3).all(|j| (r.rotation[i][j] - D[i][j]).abs() < 0.01));
    let (hx, hz) = if plain {
        (r.size[0], r.size[2])
    } else {
        (r.rotation[0][0] * r.size[0], r.rotation[2][2] * r.size[2])
    };
    ([r.translation[0], r.translation[2]], hx.fmax(hz))
}

/// The Small view's pointer turn (re-hud-map §2.2 [M], every BF2 state's
/// RAM): `SetMapNorthAngle(180, 1)` on COR1's GCW mission and both NAB2
/// missions; TAN1's 180 has no flag and leaves the Small pointer alone.
fn north_of(map: &str, era: Option<&str>) -> f32 {
    match (map, era) {
        ("naboo2", _) | ("cor1", Some("gcw")) => std::f32::consts::PI,
        _ => 0.0,
    }
}

/// The Large / Spawn group's turn (re-hud-map §2.2 [M]): every mission's
/// `SetMapNorthAngle`, the flag aside - TAN1's 180 too.
fn turn_of(map: &str, era: Option<&str>) -> f32 {
    match map {
        "tan1" => std::f32::consts::PI,
        _ => north_of(map, era),
    }
}

/// Per post instance name (lower case): its `HUDIndex`, the number the map
/// prints. The scene's mode layer is merged into the base by now.
fn post_numbers(m: &Manifest) -> HashMap<String, u32> {
    m.instances
        .iter()
        .filter_map(|i| {
            let n = i.props.iter().find(|(k, _)| *k == HUD_INDEX)?;
            Some((i.name.to_ascii_lowercase(), n.1.trim().parse().ok()?))
        })
        .collect()
}

fn mapbounds(m: &Manifest) -> Option<&ManifestRegion> {
    m.regions
        .iter()
        .chain(m.layers.iter().flat_map(|l| l.regions.iter()))
        .find(|r| r.name.eq_ignore_ascii_case("mapbounds") && r.shape == "box")
}

/// `<map>_map`, else the one `*_map` sheet the map's bake holds.
fn image_of(dir: &Path, map: &str) -> Option<String> {
    let named = format!("{}_map", map.to_ascii_lowercase());
    if pak::is_file(dir.join(format!("{named}.ktx2"))) {
        return Some(named);
    }
    let mut found: Vec<String> = pak::read_dir(dir)
        .ok()?
        .into_iter()
        .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
        .filter_map(|n| n.strip_suffix("_map.ktx2").map(|s| format!("{s}_map")))
        .collect();
    found.sort();
    (found.len() == 1).then(|| found.remove(0))
}

impl MapStatic {
    /// Read the map's and the classes' part. `vehicle_classes` is the pads'
    /// class table (`vehicle_pads::PadBuild::classes`), `era` the mission's.
    pub fn load(
        scene: &crate::assets::Scene,
        classes_dir: &Path,
        walk: &WalkSim,
        vehicle_classes: &[String],
        era: Option<&str>,
    ) -> Self {
        let m = &scene.manifest;
        let game = m.source_game.to_ascii_lowercase();
        let mut db = ClassDb::default();
        // A missing file leaves its classes at the defaults below.
        let _ = db.load_map(&scene.dir);
        let factions = [
            walk.teams[0].faction.as_str(),
            walk.teams[1].faction.as_str(),
        ];
        let dir_name = scene
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        for f in crate::roster::class_files(&game, &factions)
            .into_iter()
            .chain(
                crate::vehicle_pads::mission_side_files(&game, &dir_name)
                    .iter()
                    .copied(),
            )
        {
            let p = classes_dir.join(f);
            if pak::exists(&p) {
                let _ = db.load(&p);
            }
        }
        let layout = crate::layout::MapLayout::from_manifest(m);
        let numbers = post_numbers(m);
        let posts = layout
            .cps
            .iter()
            .map(|cp| {
                let mut icon = ClassIcon::of(&db, &cp.class);
                // The command post classes' own values where the class file
                // is not at hand (`hud_flag_icon` 2.5, re-hud-map §3 [M]).
                if icon.texture.is_none() {
                    icon.texture = Some("hud_flag_icon".into());
                    icon.scale = 2.5;
                }
                let number = numbers.get(&cp.name.to_ascii_lowercase()).copied();
                (cp.name.to_ascii_lowercase(), (icon, number))
            })
            .collect();
        let soldiers = walk
            .teams
            .iter()
            .map(|t| {
                let mut icon = t
                    .classes
                    .first()
                    .map_or_else(|| ClassIcon::of(&db, ""), |c| ClassIcon::of(&db, &c.name));
                if icon.texture.is_none() {
                    icon.texture = Some("troop_icon".into());
                    icon.scale = 1.4;
                }
                icon
            })
            .collect();
        let droids = walk
            .sim
            .weapon_classes
            .iter()
            .enumerate()
            .filter(|(_, w)| w.remote.is_some())
            .map(|(k, w)| (k, droid_icon(&db, &w.name)))
            .collect();
        let sheet_dir = scene.dir.join("textures");
        Self {
            droids,
            image: image_of(&sheet_dir, &m.map),
            sheet_dir,
            bounds: mapbounds(m).map(bounds_of),
            north: north_of(&m.map.to_ascii_lowercase(), era),
            turn: turn_of(&m.map.to_ascii_lowercase(), era),
            posts,
            soldiers,
            vehicles: vehicle_classes
                .iter()
                .map(|c| ClassIcon::of(&db, c))
                .collect(),
        }
    }

    /// The command posts' box + 100 m (`FUN_003a7af0`'s fallback, BF1's only
    /// rule: re-hud-map §2.3, §6).
    fn post_bounds(walk: &WalkSim) -> ([f32; 2], f32) {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for cp in &walk.sim.cps {
            for (a, v) in [cp.def.pos.x, cp.def.pos.z].into_iter().enumerate() {
                lo[a] = lo[a].fmin(v);
                hi[a] = hi[a].fmax(v);
            }
        }
        if lo[0] > hi[0] {
            return ([0.0; 2], 100.0);
        }
        let half = [0, 1].map(|a| (hi[a] - lo[a]) * 0.5 + 100.0);
        (
            [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5],
            half[0].fmax(half[1]),
        )
    }

    /// This frame's minimap for the local player.
    pub fn model(&self, walk: &WalkSim) -> MapModel {
        let sim = &walk.sim;
        let me = &sim.units[walk.player.index as usize];
        // `[ours]` (maintainer, 2026-10-28) Flying a droid, the map is
        // centred on the droid and turns with its heading; its zoom follows
        // the droid's speed, by its class's view radii. On foot, the camera.
        let flown = sim.controlled_droid(walk.player);
        let (eye, yaw) = match flown {
            Some(d) => ([d.pos.x, d.pos.z], d.yaw),
            None => {
                let cam = walk.camera();
                ([cam.position.x, cam.position.z], cam.yaw)
            }
        };
        let (sy, cy) = core::fmath::sin_cos(yaw);
        let fwd = [-sy, -cy];
        let own = me.team;
        let rel = |team: Option<u8>| match team {
            Some(t) if t == own => FRIEND,
            Some(_) => ENEMY,
            None => NEUTRAL,
        };
        let soldier = |team: u8| &self.soldiers[usize::from(team).min(self.soldiers.len() - 1)];
        let mine = me
            .mount
            .and_then(|m| sim.vehicles.get(m.vehicle.index as usize));
        let (view, vel) = match (mine, flown) {
            (Some(v), _) => (
                self.vehicles
                    .get(v.class as usize)
                    .map_or(VIEW_DEFAULT, |c| c.view),
                v.vel,
            ),
            (None, Some(d)) => (
                self.droids
                    .get(&d.weapon_class)
                    .map_or(VIEW_DEFAULT, |c| c.view),
                d.vel,
            ),
            (None, None) => (soldier(own).view, me.soldier.vel),
        };
        let (centre, extent) = self.bounds.unwrap_or_else(|| Self::post_bounds(walk));
        // Detected: near, or ahead inside the cone, or firing.
        let firing = (FIRING_SECONDS * core::TICK_HZ as f32) as u64;
        let detected = |pos: Vec3, last_shot: Option<u64>| {
            let d = [pos.x - eye[0], pos.z - eye[1]];
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
            len <= NEAR
                || (len <= FAR && d[0] * fwd[0] + d[1] * fwd[1] >= CONE_COS * len)
                || last_shot.is_some_and(|t| sim.tick.saturating_sub(t) <= firing)
        };
        let clock = sim.tick as f32 / core::TICK_HZ as f32;
        let posts = sim
            .cps
            .iter()
            .map(|cp| {
                let known = self.posts.get(&cp.def.name.to_ascii_lowercase());
                // Who is taking it: an owned post going neutral is taken by
                // the other side; a neutral one by the team it charges for.
                let capturing = match cp.owner {
                    Some(o) if cp.neutral > 0.0 => Some(rel(Some(1 - o.min(1)))),
                    None => cp.capture_team.map(|t| rel(Some(t))),
                    _ => None,
                };
                MapIcon {
                    pos: [cp.def.pos.x, cp.def.pos.z],
                    texture: known
                        .and_then(|k| k.0.texture.clone())
                        .unwrap_or_else(|| "hud_flag_icon".into()),
                    scale: known.map_or(2.5, |k| k.0.scale),
                    color: post_color(rel(cp.owner), capturing, clock),
                    heading: None,
                    number: known.and_then(|k| k.1),
                }
            })
            .collect();
        let mut units = Vec::new();
        for (i, u) in sim.units.iter().enumerate() {
            // The player's own body shows while his droid flies.
            let own_body = i == walk.player.index as usize && flown.is_none();
            if !u.alive || own_body || u.mount.is_some() {
                continue;
            }
            let shot = (u.last_shot_tick != u64::MAX).then_some(u.last_shot_tick);
            if u.team != own && !detected(u.soldier.pos, shot) {
                continue;
            }
            // Card CLOAK: an enemy with any concealment has no icon, firing
            // or not (B2 FUN_003a5940, D:400941: a soldier whose byte +0x7e3
            // is above 0 skips its icon on the detected path).
            if u.team != own && !u.cloak.visible() {
                continue;
            }
            let icon = soldier(u.team);
            let Some(texture) = icon.texture.clone() else {
                continue;
            };
            units.push(MapIcon {
                pos: [u.soldier.pos.x, u.soldier.pos.z],
                texture,
                scale: icon.scale,
                color: rel(Some(u.team)),
                heading: Some(-u.soldier.yaw),
                number: None,
            });
        }
        // Remote droids. `[ours]` the player's own flown droid shows too
        // (the PS2's rule that hides the controlled unit's icon is not
        // traced, re-hud-map §3).
        for d in &sim.droids {
            if d.team != own && !detected(d.pos, None) {
                continue;
            }
            let Some((icon, texture)) = self
                .droids
                .get(&d.weapon_class)
                .and_then(|c| Some((c, c.texture.clone()?)))
            else {
                continue;
            };
            units.push(MapIcon {
                pos: [d.pos.x, d.pos.z],
                texture,
                scale: icon.scale,
                color: rel(Some(d.team)),
                heading: Some(-d.yaw),
                number: None,
            });
        }
        for (i, v) in sim.vehicles.iter().enumerate() {
            let own_ride = me.mount.is_some_and(|m| m.vehicle.index as usize == i);
            if !v.alive || own_ride {
                continue;
            }
            let Some((icon, texture)) = self
                .vehicles
                .get(v.class as usize)
                .and_then(|c| Some((c, c.texture.clone()?)))
            else {
                continue;
            };
            if v.team != Some(own) && !detected(v.pos, None) {
                continue;
            }
            let f = v.rot * Vec3::Z;
            units.push(MapIcon {
                pos: [v.pos.x, v.pos.z],
                texture,
                scale: icon.scale,
                color: rel(v.team),
                heading: Some(heading([f.x, f.z])),
                number: None,
            });
        }
        let mut markers = Vec::new();
        if let Some(ctf) = sim
            .ctf
            .as_ref()
            .filter(|ctf| ctf.mode == core::sim::CtfMode::OneFlag)
        {
            for flag in &ctf.flags {
                let Some((pos, delivery)) = ctf_marker(sim, flag, own) else {
                    continue;
                };
                markers.push(MapIcon {
                    pos: [pos.x, pos.z],
                    texture: if delivery {
                        "hud_objective_icon_circle"
                    } else {
                        "hud_target_flag_onscreen"
                    }
                    .into(),
                    scale: 0.9 + 3.0 * (clock % 1.0),
                    color: [223, 223, 32],
                    heading: None,
                    number: None,
                });
            }
        }
        MapModel {
            sheet_dir: Some(self.sheet_dir.clone()),
            image: self.image.clone(),
            centre,
            extent,
            north: self.north,
            turn: self.turn,
            eye,
            heading: heading(fwd),
            radius: view_radius(view, (vel.x * vel.x + vel.z * vel.z).sqrt()),
            posts,
            units,
            markers,
            ..MapModel::default()
        }
    }

    /// The spawn map page's minimap (card SSELB, re-hud-map §1-§3): the
    /// Spawn view of [`MapStatic::model`]'s posts plus the team's live
    /// command walkers - each a post on the PS2 (Hoth's AT-ATs: `atat_icon`
    /// 3.5 [M]) - and the ring on the slot's pick (`Sim::spawn_post_selected`).
    pub fn spawn_model(&self, walk: &WalkSim, slot: usize) -> MapModel {
        let sim = &walk.sim;
        let me = &sim.units[walk.player.index as usize];
        let mut map = self.model(walk);
        let pick = sim.spawn_post_selected(slot);
        let mut select = match pick {
            Some(core::unit::SpawnPost::Post(i)) => Some(usize::from(i)),
            _ => None,
        };
        for (post, pos) in sim.player_spawn_posts(me.team) {
            let core::unit::SpawnPost::Walker(h) = post else {
                continue;
            };
            let Some((icon, texture)) = sim
                .vehicle(h)
                .and_then(|v| self.vehicles.get(v.class as usize))
                .and_then(|c| Some((c, c.texture.clone()?)))
            else {
                continue;
            };
            let heading = sim.vehicle(h).map(|v| {
                let f = v.rot * Vec3::Z;
                heading([f.x, f.z])
            });
            if pick == Some(post) {
                select = Some(map.posts.len());
            }
            map.posts.push(MapIcon {
                pos: [pos.x, pos.z],
                texture,
                scale: icon.scale,
                color: FRIEND,
                heading,
                number: None,
            });
        }
        map.spawn_view(select)
    }
}

/// A minimap with no battle behind it, for captures and goldens: the
/// soldiers' 50 m view from `eye` looking along `yaw` (the camera's), the
/// conquest mission's surviving posts and starting owners (team 1 the viewer),
/// or the world posts when no valid mission resolves; no units.
pub fn still(scene: &crate::assets::Scene, eye: Vec3, yaw: f32) -> MapModel {
    still_map(scene, eye, yaw, false)
}

fn still_map(scene: &crate::assets::Scene, eye: Vec3, yaw: f32, numbered: bool) -> MapModel {
    let m = &scene.manifest;
    let mission = crate::point_light::still_mission(scene);
    let cps = crate::point_light::still_posts(m, mission.as_ref());
    let numbers = post_numbers(m);
    let sheet_dir = scene.dir.join("textures");
    let (sy, cy) = core::fmath::sin_cos(yaw);
    let posts: Vec<MapIcon> = cps
        .iter()
        .map(|cp| MapIcon {
            pos: [cp.pos.x, cp.pos.z],
            texture: "hud_flag_icon".into(),
            scale: 2.5,
            color: [NEUTRAL, FRIEND, ENEMY][usize::from(cp.team.min(2))],
            heading: None,
            number: if numbered {
                numbers.get(&cp.name.to_ascii_lowercase()).copied()
            } else {
                None
            },
        })
        .collect();
    let (centre, extent) = mapbounds(m).map(bounds_of).unwrap_or_else(|| {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &posts {
            for a in 0..2 {
                lo[a] = lo[a].fmin(p.pos[a]);
                hi[a] = hi[a].fmax(p.pos[a]);
            }
        }
        if posts.is_empty() {
            return ([0.0; 2], 100.0);
        }
        let half = [0, 1].map(|a| (hi[a] - lo[a]) * 0.5 + 100.0);
        (
            [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5],
            half[0].fmax(half[1]),
        )
    });
    MapModel {
        image: image_of(&sheet_dir, &m.map),
        sheet_dir: Some(sheet_dir),
        centre,
        extent,
        north: 0.0,
        eye: [eye.x, eye.z],
        heading: heading([-sy, -cy]),
        radius: 50.0,
        posts,
        units: Vec::new(),
        ..MapModel::default()
    }
}

/// [`still`]'s posts on the spawn map page (card SSELB's golden): the
/// Spawn view with each post's number and the ring on post `select`.
pub fn spawn_still(scene: &crate::assets::Scene, select: Option<usize>) -> MapModel {
    still_map(scene, Vec3::ZERO, 0.0, true).spawn_view(select)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_flag_markers_follow_the_carriers_team() {
        let mut sim = core::sim::Sim::empty(1);
        let home = Vec3::new(10.0, 20.0, 30.0);
        let goal0 = Vec3::new(100.0, 0.0, 0.0);
        let goal1 = -goal0;
        sim.ctf = Some(core::sim::CtfState::new_space_one_flag(
            5, home, goal0, goal1,
        ));
        let carrier = sim.spawn(&core::unit::UnitSpec::trooper(1, home)).unwrap();
        assert_eq!(
            ctf_marker(&sim, &sim.ctf.as_ref().unwrap().flags[0], 0),
            Some((home, false))
        );
        sim.ctf.as_mut().unwrap().flags[0].status = core::sim::FlagStatus::Carried { carrier };
        let flag = &sim.ctf.as_ref().unwrap().flags[0];
        assert_eq!(ctf_marker(&sim, flag, 1), Some((goal1, true)));
        assert_eq!(ctf_marker(&sim, flag, 0), None);
    }

    #[test]
    fn carrier_marker_tracks_live_unit_and_relation() {
        let mut sim = core::sim::Sim::empty(1);
        let viewer = sim
            .spawn(&core::unit::UnitSpec::trooper(0, Vec3::ZERO))
            .unwrap();
        let carrier = sim
            .spawn(&core::unit::UnitSpec::trooper(1, Vec3::new(2.0, 3.0, 4.0)))
            .unwrap();
        let (point, friendly) = carrier_marker(&sim, carrier, viewer).unwrap();
        assert!(!friendly);
        assert!(point.y > sim.unit(carrier).unwrap().soldier.pos.y);
        sim.unit_mut(carrier).unwrap().team = 0;
        assert!(carrier_marker(&sim, carrier, viewer).unwrap().1);
        sim.unit_mut(carrier).unwrap().alive = false;
        assert!(carrier_marker(&sim, carrier, viewer).is_none());
    }

    #[test]
    fn carrier_marker_follows_flyer_rotation_and_viewer() {
        use core::vehicle::{BlastSetup, SeatKind, VehicleKind, VehicleSpec};
        let mut sim = core::sim::Sim::empty(1);
        let pilot = sim
            .spawn(&core::unit::UnitSpec::trooper(0, Vec3::ZERO))
            .unwrap();
        let observer = sim
            .spawn(&core::unit::UnitSpec::trooper(1, Vec3::ZERO))
            .unwrap();
        let flyer = sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::new(10.0, 20.0, 30.0),
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        sim.blast_setup = vec![Some(BlastSetup {
            explosion: None,
            sphere_centre: Vec3::ZERO,
            sphere_radius: 10.0,
        })];
        assert!(sim.seat_unit(pilot, flyer, 0));
        let vehicle = &mut sim.vehicles[flyer.index as usize];
        vehicle.rot = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let (own, friendly) = carrier_marker(&sim, pilot, pilot).unwrap();
        assert!(friendly);
        assert!((own - Vec3::new(8.0, 20.3, 30.0)).length() < 0.001);
        let (other, friendly) = carrier_marker(&sim, pilot, observer).unwrap();
        assert!(!friendly);
        assert!((other - Vec3::new(10.0, 24.0, 30.0)).length() < 0.001);
    }

    fn region(rotation: [[f32; 3]; 3], size: [f32; 3]) -> ManifestRegion {
        ManifestRegion {
            shape: "box".into(),
            name: "mapbounds".into(),
            rotation,
            translation: [-299.02, 11.15, 134.93],
            size,
            props: Vec::new(),
        }
    }

    /// The PS2's RAM records (re-hud-map §2.3 [M]): HOT1 R 400, UTA1 233.78,
    /// TAN1 −89.43 from the stored regions.
    #[test]
    fn the_map_square_is_the_ps2s() {
        let d = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]];
        assert_eq!(bounds_of(&region(d, [400.0; 3])).1, 400.0);
        let uta = [
            [-0.999_999_76, 0.0, 0.000_713_992_8],
            [0.0, 1.0, 0.0],
            [-0.000_713_992_8, 0.0, -0.999_999_76],
        ];
        assert_eq!(
            bounds_of(&region(uta, [227.9457, 10.0, 233.77731])).1,
            233.77731
        );
        let tan = [
            [-1.0, 0.0, 0.0],
            [0.0, 0.999_204_8, -0.039_872_076],
            [0.0, -0.039_872_076, -0.999_204_8],
        ];
        let (c, r) = bounds_of(&region(tan, [137.3199, 0.100002, 89.50355]));
        assert_eq!(c, [-299.02, 134.93]);
        assert!((r + 89.432).abs() < 0.01, "{r}");
    }

    #[test]
    fn the_small_pointer_turns_only_with_the_flag() {
        assert_eq!(north_of("naboo2", Some("cw")), std::f32::consts::PI);
        assert_eq!(north_of("cor1", Some("gcw")), std::f32::consts::PI);
        assert_eq!(north_of("cor1", Some("cw")), 0.0);
        assert_eq!(north_of("tan1", Some("gcw")), 0.0);
        // The Large / Spawn group turns by the angle, flag or not.
        assert_eq!(turn_of("tan1", Some("gcw")), std::f32::consts::PI);
        assert_eq!(turn_of("naboo2", None), std::f32::consts::PI);
        assert_eq!(turn_of("hoth", None), 0.0);
    }

    /// The spawn map page's model: the whole-map square, the mission's
    /// turn, no units, no ring before a pick.
    #[test]
    fn the_spawn_model_is_the_whole_map() {
        let w = WalkSim::new(3, Vec3::ZERO, None);
        let st = MapStatic {
            bounds: Some(([10.0, -20.0], 300.0)),
            turn: std::f32::consts::PI,
            soldiers: vec![ClassIcon::of(&ClassDb::default(), "x"); 2],
            ..MapStatic::default()
        };
        let m = st.spawn_model(&w, 0);
        assert_eq!(m.view, ui::hud_map::MapView::Spawn);
        assert_eq!(
            (m.eye, m.radius, m.turn),
            ([10.0, -20.0], 300.0, std::f32::consts::PI)
        );
        assert!(m.units.is_empty());
        assert_eq!(m.select, None, "no pick yet");
    }

    /// A flying droid has its class's icon in its team's colour at its
    /// place; none once it is gone; an enemy one only when detected.
    #[test]
    fn a_flying_droid_has_a_map_icon() {
        struct NoProps;
        impl core::weapon::PropSource for NoProps {
            fn prop(&self, _: &str) -> Option<&str> {
                None
            }
            fn prop_hash(&self, _: u32) -> Option<&str> {
                None
            }
        }
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let droid = core::remote::DroidClass::from_props(&NoProps, true);
        let remote = core::remote::RemoteWeapon::new(
            &NoProps,
            droid,
            core::weapon::WeaponClass::bf1_rifle(),
        );
        let mut class = core::weapon::WeaponClass::bf1_rifle();
        class.remote = Some(Box::new(remote.clone()));
        let weapon = w.sim.add_weapon_class(class);
        let own = w.sim.units[w.player.index as usize].team;
        let st = MapStatic {
            bounds: Some(([0.0, 0.0], 300.0)),
            soldiers: vec![ClassIcon::of(&ClassDb::default(), "x"); 2],
            droids: [(weapon, droid_icon(&ClassDb::default(), "x"))].into(),
            ..MapStatic::default()
        };
        let icons = |w: &WalkSim| -> Vec<MapIcon> {
            st.model(w)
                .units
                .into_iter()
                .filter(|u| u.texture == "troop_icon" && u.scale == 1.0)
                .collect()
        };
        assert!(icons(&w).is_empty());
        let at = |team: u8, pos: Vec3| {
            core::remote::Droid::new(
                1,
                w.player,
                team,
                weapon,
                &remote,
                pos,
                Vec3::ZERO,
                0.0,
                true,
            )
        };
        let d = at(own, Vec3::new(12.0, 2.0, -30.0));
        w.sim.droids.push(d);
        w.sim.droids[0].controlled = true;
        let got = icons(&w);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].pos, [12.0, -30.0]);
        assert_eq!(got[0].color, FRIEND);
        // An enemy droid far behind the view is not detected.
        let far = at(1 - own.min(1), Vec3::new(0.0, 2.0, 200.0));
        w.sim.droids.push(far);
        assert_eq!(icons(&w).len(), 1);
        // Near the eye (the flown droid), it is, in the enemy colour.
        w.sim.droids[1].pos = Vec3::new(14.0, 2.0, -34.0);
        let got = icons(&w);
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].color, ENEMY);
        w.sim.droids.clear();
        assert!(icons(&w).is_empty());
    }

    /// PR #61 review fix 2: a cloaked enemy has no map icon, even close by
    /// and firing; a cloaked friend keeps his.
    #[test]
    fn a_cloaked_enemy_has_no_map_icon() {
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let own = w.sim.units[w.player.index as usize].team;
        let foe = 1 - own.min(1);
        let enemy = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                foe,
                Vec3::new(2.0, 0.0, -2.0),
            ))
            .unwrap();
        let friend = w
            .sim
            .spawn(&core::unit::UnitSpec::trooper(
                own,
                Vec3::new(-2.0, 0.0, -2.0),
            ))
            .unwrap();
        let icon = ClassIcon {
            texture: Some("troop_icon".into()),
            ..ClassIcon::of(&ClassDb::default(), "x")
        };
        let st = MapStatic {
            bounds: Some(([0.0, 0.0], 300.0)),
            soldiers: vec![icon; 2],
            ..MapStatic::default()
        };
        let colours = |w: &WalkSim| -> Vec<[u8; 3]> {
            st.model(w).units.into_iter().map(|u| u.color).collect()
        };
        let before = colours(&w);
        assert!(
            before.contains(&ENEMY) && before.contains(&FRIEND),
            "{before:?}"
        );
        for h in [enemy, friend] {
            let u = &mut w.sim.units[h.index as usize];
            u.cloak.active = true;
            u.cloak.strength = 0.5;
            u.last_shot_tick = w.sim.tick;
        }
        let after = colours(&w);
        assert!(!after.contains(&ENEMY), "{after:?}");
        assert!(after.contains(&FRIEND), "{after:?}");
    }

    /// Flying a droid, the map is centred on the droid, turns with its
    /// heading and zooms with its speed; the standing body shows as a
    /// soldier icon. Control back, the soldier's camera again and no body.
    #[test]
    fn the_map_follows_a_flown_droid_and_returns() {
        struct NoProps;
        impl core::weapon::PropSource for NoProps {
            fn prop(&self, _: &str) -> Option<&str> {
                None
            }
            fn prop_hash(&self, _: u32) -> Option<&str> {
                None
            }
        }
        let mut w = WalkSim::new(3, Vec3::ZERO, None);
        let droid = core::remote::DroidClass::from_props(&NoProps, true);
        let remote = core::remote::RemoteWeapon::new(
            &NoProps,
            droid,
            core::weapon::WeaponClass::bf1_rifle(),
        );
        let mut class = core::weapon::WeaponClass::bf1_rifle();
        class.remote = Some(Box::new(remote.clone()));
        let weapon = w.sim.add_weapon_class(class);
        let own = w.sim.units[w.player.index as usize].team;
        let soldier = ClassIcon {
            texture: Some("soldier_icon".into()),
            scale: 1.4,
            view: VIEW_DEFAULT,
        };
        let st = MapStatic {
            bounds: Some(([0.0, 0.0], 300.0)),
            soldiers: vec![soldier; 2],
            droids: [(weapon, droid_icon(&ClassDb::default(), "x"))].into(),
            ..MapStatic::default()
        };
        let bodies = |m: &MapModel| {
            m.units
                .iter()
                .filter(|u| u.texture == "soldier_icon")
                .count()
        };
        let on_foot = st.model(&w);
        assert_eq!(bodies(&on_foot), 0, "his own icon is the map's pointer");
        let cam = w.camera();
        assert_eq!(on_foot.eye, [cam.position.x, cam.position.z]);
        let mut d = core::remote::Droid::new(
            1,
            w.player,
            own,
            weapon,
            &remote,
            Vec3::new(40.0, 2.0, -25.0),
            Vec3::new(60.0, 0.0, 0.0),
            1.1,
            true,
        );
        d.controlled = true;
        w.sim.droids.push(d);
        let flying = st.model(&w);
        assert_eq!(flying.eye, [40.0, -25.0]);
        let (sy, cy) = core::fmath::sin_cos(1.1);
        assert_eq!(flying.heading, heading([-sy, -cy]));
        assert_eq!(flying.radius, view_radius(VIEW_DEFAULT, 60.0));
        assert!(flying.radius > on_foot.radius);
        assert_eq!(bodies(&flying), 1, "the standing body shows");
        let me = w.soldier().pos;
        assert!(flying.units.iter().any(|u| u.pos == [me.x, me.z]));
        w.sim.droids[0].controlled = false;
        let back = st.model(&w);
        assert_eq!((back.eye, back.heading), (on_foot.eye, on_foot.heading));
        assert_eq!(bodies(&back), 0);
    }

    struct Fixture {
        root: PathBuf,
        scene: crate::assets::Scene,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
    fn fixture(mission: Option<&str>) -> Fixture {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "openbf-stillcp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let dir = root.join("assets_baked/__planner_still_probe");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest: manifest::Manifest = ron::from_str(r#"(
        map: "__planner_still_probe", source_game: "bf2",
        terrain_name: None, sky_name: None, terrain_glb: None,
        models: [], textures: [], classes: [], geometries: [], regions: [],
        instances: [
            (class: "cp", name: "CP1", rotation: ((1,0,0),(0,1,0),(0,0,1)),
             translation: (0,0,0), props: [(2734521612,"1"),(3334562297,"capture"),(HUD_INDEX,"1")]),
            (class: "cp", name: "CP2", rotation: ((1,0,0),(0,1,0),(0,0,1)),
             translation: (100,0,0), props: [(2734521612,"2"),(3334562297,"capture"),(HUD_INDEX,"2")]),
            (class: "cp", name: "CP6", rotation: ((1,0,0),(0,1,0),(0,0,1)),
             translation: (1000,0,0), props: [(2734521612,"2"),(3334562297,"capture"),(HUD_INDEX,"6")]),
        ],
    )"#.replace("HUD_INDEX", &HUD_INDEX.to_string()).as_str()).unwrap();
        if let Some(fields) = mission {
            let missions = root.join("missions");
            std::fs::create_dir_all(&missions).unwrap();
            std::fs::write(
                missions.join("__planner_still_probe_con.ron"),
                format!(
                    r#"(
            map: "__planner_still_probe", mode: "con", source_game: "bf2",
            teams: [(faction: "imp", units: 16, tickets: 150),
                    (faction: "all", units: 16, tickets: 150)],
            ai_alive_cap: 16, spawn_delay: 10.0, spawn_interval: 0.25,
            {fields}
        )"#
                ),
            )
            .unwrap();
        }
        let scene = crate::assets::Scene {
            dir,
            manifest,
            meshes: HashMap::new(),
            terrain: None,
            water: None,
            lowres: None,
            terrain_layers: None,
            textures: HashMap::new(),
            collision: HashMap::new(),
            cutters: Vec::new(),
        };
        assert_eq!(
            crate::point_light::still_mission(&scene).is_some(),
            mission.is_some(),
            "legal mission fixture must resolve"
        );
        Fixture { root, scene }
    }

    fn check_posts(
        scene: &crate::assets::Scene,
        expected: &[(f32, u32, [u8; 3])],
        bounds: ([f32; 2], f32),
    ) {
        let map = still(scene, Vec3::ZERO, 0.0);
        assert_eq!(
            map.posts
                .iter()
                .map(|p| (p.pos, p.color))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(x, _, c)| ([*x, 0.0], *c))
                .collect::<Vec<_>>()
        );
        assert_eq!((map.centre, map.extent), bounds);
        let spawn = spawn_still(scene, Some(1));
        assert_eq!(
            spawn
                .posts
                .iter()
                .map(|p| (p.pos, p.number, p.color))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(x, n, c)| ([*x, 0.0], Some(*n), *c))
                .collect::<Vec<_>>()
        );
        assert_eq!((spawn.centre, spawn.extent), bounds);
        assert_eq!((spawn.eye, spawn.radius), bounds);
        assert_eq!(spawn.select, Some(1));
    }

    #[test]
    fn still_mission_kills_and_reowns_posts() {
        let f = fixture(Some(
            r#"posts: ["cp1","CP2","cp6"], killed: ["cP6"], cps: [("cP1",2),("cp2",1)],"#,
        ));
        check_posts(
            &f.scene,
            &[(0.0, 1, ENEMY), (100.0, 2, FRIEND)],
            ([50.0, 0.0], 150.0),
        );
    }

    #[test]
    fn spawn_still_uses_mission_posts_and_filtered_bounds() {
        let f = fixture(Some(
            r#"posts: ["cp1","cp2","cp6"], killed: ["cP6"], cps: [("cP1",2),("cp2",1)],"#,
        ));
        let map = spawn_still(&f.scene, Some(1));
        assert_eq!(
            map.posts
                .iter()
                .map(|p| (p.pos, p.number, p.color))
                .collect::<Vec<_>>(),
            vec![
                ([0.0, 0.0], Some(1), ENEMY),
                ([100.0, 0.0], Some(2), FRIEND)
            ]
        );
        assert_eq!((map.centre, map.extent), ([50.0, 0.0], 150.0));
        assert_eq!(map.select, Some(1));
    }

    #[test]
    fn still_mission_roster_removes_middle_without_renumbering() {
        let f = fixture(Some(r#"posts: ["cp6","cP1"],"#));
        check_posts(
            &f.scene,
            &[(0.0, 1, FRIEND), (1000.0, 6, ENEMY)],
            ([500.0, 0.0], 600.0),
        );
    }

    #[test]
    fn still_mission_kill_removes_middle_without_renumbering() {
        let f = fixture(Some(r#"posts: ["cp1","cp2","cp6"], killed: ["cP2"],"#));
        check_posts(
            &f.scene,
            &[(0.0, 1, FRIEND), (1000.0, 6, ENEMY)],
            ([500.0, 0.0], 600.0),
        );
    }

    #[test]
    fn still_mission_owners_include_neutral() {
        let f = fixture(Some(
            r#"posts: ["cp1","cp2","cp6"], cps: [("CP1",2),("cP2",1),("Cp6",0)],"#,
        ));
        check_posts(
            &f.scene,
            &[(0.0, 1, ENEMY), (100.0, 2, FRIEND), (1000.0, 6, NEUTRAL)],
            ([500.0, 0.0], 600.0),
        );
    }

    #[test]
    fn still_mission_all_killed_uses_empty_bounds() {
        let f = fixture(Some(
            r#"posts: ["cp1","cp2","cp6"], killed: ["CP1","cp2","Cp6"],"#,
        ));
        check_posts(&f.scene, &[], ([0.0, 0.0], 100.0));
    }

    #[test]
    fn still_mission_preserves_authored_bounds() {
        let mut f = fixture(Some(r#"posts: ["cp1"],"#));
        let r = region(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [400.0; 3],
        );
        let bounds = bounds_of(&r);
        f.scene.manifest.regions.push(r);
        check_posts(&f.scene, &[(0.0, 1, FRIEND)], bounds);
    }

    #[test]
    fn still_without_mission_keeps_world_posts_for_both_games() {
        let mut f = fixture(None);
        for game in ["bf1", "bf2"] {
            f.scene.manifest.source_game = game.into();
            check_posts(
                &f.scene,
                &[(0.0, 1, FRIEND), (100.0, 2, ENEMY), (1000.0, 6, ENEMY)],
                ([500.0, 0.0], 600.0),
            );
        }
    }
}
