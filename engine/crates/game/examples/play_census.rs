//! Every usable thing on every map, tried with a player's own input.
//!
//! ```text
//! cargo run --release -p game --example play_census -- [--era cw|gcw] [--only <probe,...>] [map ...]
//! ```
//!
//! The round sweep (`round_to_outcome`) says how bots do; this says what a
//! player finds. Each probe teleports the player to its start and then uses
//! only stick, mouse and buttons. One line per thing that failed, and a
//! `summary` line per map and probe, on stdout. Each probe has a fresh battle.
//!
//! Run by default:
//! - `spawn`: every spawn node. Dropped where a spawn drops a soldier, does he
//!   live, stay on his floor and walk away from it?
//! - `post`: every command post. Can a soldier stand in its capture region, and
//!   does standing there flip it?
//! - `vehicle`: every vehicle and turret. Does it stand and survive 5 s empty,
//!   can a soldier walk up and get in, does it move, does each armed seat fire,
//!   can he get out onto ground he can walk from?
//! - `pad`: each pad with a class for its team has a vehicle, and replaces one
//!   destroyed. `CENSUS_PADS=1` lists pads and flyer class numbers.
//! - `station`: every health droid. Does standing at it give health?
//! - `door`: every door. Does it open for a soldier in its trigger, shut after
//!   him, and is there a straight walk through it?
//! - `round`: does holding every post end the round, and can each team spawn?
//!
//! By `--only`, because they take longer or need an argument:
//! - `roam`: a player with nothing in mind, from every spawn and hub, 3 passes
//!   of 30 s. Counts by 6 m cell what he trips over, and a `TRAP` where he then
//!   cannot get 6 m away walking, sprinting, jumping or crouched.
//!   `CENSUS_HISTORY=1` prints the walk that led to a trap.
//! - `drive`: every vehicle driven at random for 40 s, and got out of.
//! - `bots`: a real round of bots: posts never captured, classes never boarded,
//!   bots parked 60 s, deaths other than by a shot by 6 m cell. `CENSUS_SEED=n`,
//!   `CENSUS_DEATHS=1` (how each died), `CENSUS_PARKED=1` (what each parked bot
//!   is doing), `CENSUS_WATCH=unit,from,to` (one unit's
//!   ticks), `CENSUS_FLYERS=1` (each flyer's phase changes).
//! - `trace` (`CENSUS_TRACE=x,y,z[,lookx,lookz]`), `escape`
//!   (`CENSUS_ESCAPE=x,y,z`), `tris` (`CENSUS_TRIS=x,y,z,r`) and `rays`
//!   (`CENSUS_RAYS=x,y,z`): one spot in detail.
//!
//! Threads: `SWEEP_THREADS`, default 4. It only reads the bake.

use core::sim::{buttons, Event, PadState};
use core::unit::UnitHandle;
use core::vehicle::{VehicleHandle, VehicleKind};
use game::walk::WalkSim;
use glam::Vec3;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const SECOND: usize = 30;
const DIRECTIONS: usize = 8;
/// Soldier falls further than this from a spawn floor: not standing on it.
const FALL_M: f32 = 4.0;
/// A walk this short in every direction is a soldier who cannot leave.
const WALK_FREE_M: f32 = 1.5;

macro_rules! out {
    ($s:expr, $($a:tt)*) => { let _ = writeln!($s, $($a)*); };
}

struct World {
    key: String,
    w: WalkSim,
    classes: Vec<String>,
    seat_guns: Vec<Vec<bool>>,
    game: String,
    body_names: Vec<String>,
    cw: bool,
}

fn mission_path(key: &str, cw: bool) -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
    let era = dir.join(format!("{key}_con_cw.ron"));
    if cw && era.exists() {
        era
    } else {
        dir.join(format!("{key}_con.ron"))
    }
}

/// The round sweep's battle, in the game's order, with the player and no bot.
fn build(key: &str, path: &Path, cw: bool) -> anyhow::Result<World> {
    let mut scene = game::assets::load_scene(path)?;
    scene.apply_mode_layer("con");
    let parent = path.parent().expect("a map folder has a parent");
    let common = parent.join(format!("common_{}", scene.manifest.source_game));
    if common.join("common.ron").exists() {
        scene.merge_common(game::assets::load_common(&common)?);
    }
    let body_names = scene.collision_body_labels();
    let mission_file = mission_path(key, cw);
    let mission = mission_file
        .exists()
        .then(|| game::mission::MissionDef::load(&mission_file))
        .transpose()?;
    let mut layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let applied = mission.as_ref().map(|m| layout.apply_mission(m));
    let seed = std::env::var("CENSUS_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(42);
    let mut sim = core::sim::Sim::empty(seed);
    sim.omni_radius = core::sim::omni_radius_for(&scene.manifest.source_game);
    scene.install_water(&mut sim);
    scene.install_damage_regions(&mut sim);
    sim.ground_rules = core::soldier::GroundRules::for_game(&scene.manifest.source_game);
    sim.item_pool = game::roster::item_pool(&scene.manifest.source_game, &scene.manifest.map);
    sim.battle = core::battle::Battle::new([100_000; 2]);
    sim.team_combatants = Some([1; 2]);
    let factions = mission
        .as_ref()
        .map(game::mission::MissionDef::factions)
        .unwrap_or_else(|| {
            game::roster::default_factions(&scene.manifest.source_game).map(str::to_owned)
        });
    let classes_dir = parent.join("classes");
    let (rosters, _) = game::roster::FactionRoster::pair(
        &classes_dir,
        &scene.manifest.source_game,
        [factions[0].as_str(), factions[1].as_str()],
        mission.as_ref(),
    )?;
    let mut tables: [Vec<core::unit::UnitClass>; 2] = [Vec::new(), Vec::new()];
    let mut names: Vec<(String, usize)> = Vec::new();
    let mut t = 0usize;
    let teams = rosters.each_ref().map(|r| {
        for c in &r.classes {
            tables[t].push(c.register(&mut sim, &mut names));
        }
        t += 1;
        game::walk::TeamTemplate::from_roster(
            r,
            &tables[t - 1][0],
            r.classes
                .iter()
                .map(game::walk::ClassVisual::from_roster)
                .collect(),
        )
    });
    let mut nav = game::layout::nav_graph(&scene.manifest, &scene.dir);
    if let Some(m) = &mission {
        m.apply_arc_blocks(nav.as_mut());
        m.apply_barriers(nav.as_mut());
    }
    let mut w = WalkSim::with_teams(sim, scene.heightfield(), Vec3::ZERO, teams, nav);
    w.sim.classes = tables;
    scene.install_collision(&mut w.sim);
    if let Some(a) = &applied {
        w.sim.death_regions = a.death_regions.clone();
        w.sim.fly_ceiling = a.fly_ceiling;
        w.sim.flight_limits = a.flight_limits;
        w.sim.landing_regions = a.landing_regions.clone();
        w.sim.ground_flyer_map = mission.as_ref().is_some_and(|m| m.ground_flyer_map);
    }
    w.add_posts(&layout);
    if let Some(m) = &mission {
        m.apply_ai(&mut w.sim);
    }
    let factions = [w.teams[0].faction.clone(), w.teams[1].faction.clone()];
    let pads = game::vehicle_pads::install(
        &mut w.sim,
        &scene.manifest,
        &scene.dir,
        &classes_dir,
        &factions,
    )?;
    let seat_guns = pads
        .seats
        .iter()
        .map(|ring| ring.iter().map(|s| !s.weapons.is_empty()).collect())
        .collect();
    w.opening_wave([0, 0], true);
    Ok(World {
        key: key.to_owned(),
        w,
        classes: pads.classes,
        seat_guns,
        game: scene.manifest.source_game.clone(),
        body_names,
        cw,
    })
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl World {
    fn me(&self) -> UnitHandle {
        self.w.player
    }

    fn tick(&mut self, pad: PadState) {
        self.w.step_tick(&[pad]);
    }

    fn idle(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.tick(PadState::default());
        }
    }

    fn press(&mut self, b: u32) {
        self.tick(PadState::default());
        self.tick(PadState {
            buttons: b,
            ..Default::default()
        });
    }

    /// A live player at `p` on `team`, standing still, full health. A soldier
    /// who died comes back through the input the spawn screen sends.
    fn place(&mut self, p: Vec3, team: u8) {
        let mut waited = 0;
        while self.w.sim.unit(self.me()).is_none_or(|u| !u.alive) && waited < SECOND * 30 {
            self.tick(PadState {
                buttons: buttons::RESPAWN,
                ..Default::default()
            });
            self.tick(PadState::default());
            waited += 2;
        }
        let me = self.me();
        if self.w.sim.seat_of(me).is_some() {
            self.w.sim.exit_vehicle(me);
        }
        let u = self.w.sim.unit_mut(me).expect("the player came back");
        u.team = team;
        u.soldier.pos = p;
        u.soldier.vel = Vec3::ZERO;
        u.soldier.energy = u.consts.energy_bar;
        u.health.cur = u.health.max;
    }

    /// One tick walking at `at`: the mouse turns the view, the stick pushes.
    fn walk_at(&mut self, at: Vec3, buttons_held: u32) {
        let Some(u) = self.w.sim.unit(self.me()) else {
            self.tick(PadState::default());
            return;
        };
        let d = at - u.soldier.pos;
        let yaw = (-d.x).atan2(-d.z);
        let turn = wrap(yaw - u.soldier.yaw);
        self.tick(PadState {
            ly: if turn.abs() < 0.5 { i16::MAX } else { 0 },
            buttons: buttons_held | buttons::MOUSE_AIM,
            mx: PadState::quantize_look(f64::from(-turn)),
            ..Default::default()
        });
    }

    fn feet(&self) -> Vec3 {
        self.w.sim.units[self.w.player.index as usize].soldier.pos
    }

    /// Furthest a soldier at `start` gets with `ticks` of stick, over the
    /// directions, each from `start` again. Stops early on a clear answer.
    fn free_walk(&mut self, start: Vec3, team: u8, ticks: usize) -> f32 {
        let mut best = 0.0f32;
        for k in 0..DIRECTIONS {
            let a = k as f32 * std::f32::consts::TAU / DIRECTIONS as f32;
            self.place(start, team);
            let to = start + Vec3::new(a.cos(), 0.0, a.sin()) * 50.0;
            for _ in 0..ticks {
                self.walk_at(to, 0);
            }
            best = best.max((self.feet() - start).with_y(0.0).length());
            if best >= WALK_FREE_M * 2.0 {
                break;
            }
        }
        best
    }
}

fn spawn_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    let posts: Vec<_> = world
        .w
        .sim
        .cps
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.def.name.clone(), c.def.spawn_nodes.clone()))
        .collect();
    let (mut nodes, mut bad) = (0, 0);
    for (i, name, list) in posts {
        for (n, (pos, _)) in list.iter().enumerate() {
            nodes += 1;
            let g = world.w.sim.spawn_ground(*pos);
            if g <= core::collision::NO_GROUND {
                bad += 1;
                out!(
                    out,
                    "spawn {key} post {i} {name} node {n} ({:.1},{:.1},{:.1}): NO GROUND",
                    pos.x,
                    pos.y,
                    pos.z
                );
                continue;
            }
            let start = Vec3::new(pos.x, g, pos.z);
            world.w.sim.cheats.invincible = false;
            world.place(start, 0);
            let hp = world.w.sim.unit(world.me()).map_or(0.0, |u| u.health.cur);
            world.w.sim.events.clear();
            world.idle(SECOND);
            let me = world.me();
            let died = world.w.sim.unit(me).is_none_or(|u| !u.alive);
            let hurt = world.w.sim.unit(me).map_or(0.0, |u| hp - u.health.cur);
            if died || hurt > 0.5 {
                bad += 1;
                out!(
                    out,
                    "spawn {key} post {i} {name} node {n} ({:.1},{g:.1},{:.1}): {} standing 1 s",
                    pos.x,
                    pos.z,
                    if died {
                        "DIED".to_owned()
                    } else {
                        format!("lost {hurt:.0} health")
                    }
                );
            }
            world.w.sim.cheats.invincible = true;
            world.place(start, 0);
            world.idle(SECOND / 2);
            let settled = world.feet();
            if start.y - settled.y > FALL_M {
                bad += 1;
                out!(out, "spawn {key} post {i} {name} node {n} ({:.1},{g:.1},{:.1}): FELL {:.1} m on standing", pos.x, pos.z, start.y - settled.y);
            }
            if settled.y - start.y > 1.5 {
                bad += 1;
                out!(out, "spawn {key} post {i} {name} node {n} ({:.1},{g:.1},{:.1}): THROWN UP {:.1} m on standing", pos.x, pos.z, settled.y - start.y);
            }
            if world.w.camera_in_body() {
                bad += 1;
                out!(
                    out,
                    "spawn {key} post {i} {name} node {n} ({:.1},{g:.1},{:.1}): camera in the body",
                    pos.x,
                    pos.z
                );
            }
            let walked = world.free_walk(start, 0, SECOND);
            if walked < WALK_FREE_M {
                bad += 1;
                out!(out, "spawn {key} post {i} {name} node {n} ({:.1},{g:.1},{:.1}): STUCK, best walk {walked:.2} m in 1 s of 8 directions", pos.x, pos.z);
            }
        }
    }
    out!(out, "summary spawn {key}: {nodes} nodes, {bad} findings");
}

/// A floor a soldier can stand on inside `region`, trying the centre, then
/// rings: `(point, tried)`.
fn floor_in_post(world: &World, ci: usize) -> Option<Vec3> {
    let cp = &world.w.sim.cps[ci];
    let c = cp.def.capture.center;
    let mut rings = vec![c];
    for r in [1.0f32, 2.0, 3.0, 5.0, 8.0, 12.0] {
        for k in 0..12 {
            let a = k as f32 * std::f32::consts::TAU / 12.0;
            rings.push(c + Vec3::new(a.cos(), 0.0, a.sin()) * r);
        }
    }
    let mut heights = vec![c.y + 6.0, c.y + 2.0, cp.def.pos.y + 2.0, c.y + 20.0];
    heights.dedup();
    for p in rings {
        for y in &heights {
            let q = Vec3::new(p.x, *y, p.z);
            let g = world.w.sim.ground_under(q, false);
            if g > core::collision::NO_GROUND {
                let s = Vec3::new(p.x, g, p.z);
                if cp.def.capture.contains(s) && world.w.sim.shut_in(s + Vec3::Y).is_none() {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn post_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    world.w.sim.cheats.invincible = true;
    let n = world.w.sim.cps.len();
    let mut bad = 0;
    for ci in 0..n {
        let name = world.w.sim.cps[ci].def.name.clone();
        let owner = world.w.sim.cps[ci].owner;
        let team = if owner == Some(0) { 1 } else { 0 };
        let (cap, neu) = (
            world.w.sim.cps[ci].def.capture_time,
            world.w.sim.cps[ci].def.neutralize_time,
        );
        if cap == f32::MAX {
            continue;
        }
        let Some(stand) = floor_in_post(world, ci) else {
            bad += 1;
            let c = world.w.sim.cps[ci].def.capture.center;
            out!(
                out,
                "post {key} {ci} {name} centre ({:.1},{:.1},{:.1}): NO FLOOR in its capture region",
                c.x,
                c.y,
                c.z
            );
            continue;
        };
        world.place(stand, team);
        let limit = ((cap + neu + 4.0) * SECOND as f32) as usize;
        let mut done = None;
        world.w.sim.events.clear();
        for t in 0..limit {
            world.tick(PadState::default());
            if world.w.sim.events.iter().any(|e| {
                matches!(e, Event::Captured { cp, team: by } if *cp as usize == ci && *by == team)
            }) {
                done = Some(t);
                break;
            }
            world.w.sim.events.clear();
            let me = world.me();
            let f = world.feet();
            if !world.w.sim.cps[ci].def.capture.contains(f) || world.w.sim.seat_of(me).is_some() {
                break;
            }
        }
        if done.is_none() {
            bad += 1;
            let f = world.feet();
            let cp = &world.w.sim.cps[ci];
            out!(out, "post {key} {ci} {name}: NOT CAPTURED standing at ({:.1},{:.1},{:.1}), now ({:.1},{:.1},{:.1}), inside {}, owner {:?}, meters {:.1}/{:.1}, ban {}", stand.x, stand.y, stand.z, f.x, f.y, f.z, cp.def.capture.contains(f), cp.owner, cp.capture, cp.neutral, cp.ai_capture_ban);
        }
        // Back to the owner it had: a later post's spawn or pad must not move.
        world.w.sim.cps[ci].owner = owner;
        world.w.sim.cps[ci].capture = 0.0;
        world.w.sim.cps[ci].capture_team = None;
        world.w.sim.cps[ci].neutral = 0.0;
        world.place(Vec3::new(0.0, 900.0, 0.0), 0);
    }
    out!(out, "summary post {key}: {n} posts, {bad} findings");
}

fn start_ring(world: &World, at: Vec3, radius: f32) -> Vec<Vec3> {
    (0..DIRECTIONS)
        .filter_map(|k| {
            let a = k as f32 * std::f32::consts::TAU / DIRECTIONS as f32;
            let p = at + Vec3::new(a.cos(), 0.0, a.sin()) * radius;
            let g = world
                .w
                .sim
                .ground_under(Vec3::new(p.x, at.y + 3.0, p.z), false);
            (g > core::collision::NO_GROUND && (g - at.y).abs() < 6.0)
                .then_some(Vec3::new(p.x, g, p.z))
        })
        .collect()
}

/// Walk up from around the vehicle and press ACTION: how the seat was got.
fn walk_in(world: &mut World, vh: VehicleHandle, team: u8) -> Result<(), String> {
    let pos = world.w.sim.vehicle(vh).map(|v| v.pos).ok_or("gone")?;
    let reach = world
        .w
        .sim
        .seat_setup
        .get(world.w.sim.vehicle(vh).map_or(0, |v| v.class as usize))
        .and_then(|s| s.as_ref())
        .map_or(3.0, |s| s.radius);
    let ring = start_ring(world, pos, reach + 6.0);
    if ring.is_empty() {
        return Err(format!(
            "no floor within 6 m of its height around it (radius {reach:.1})"
        ));
    }
    let mut closest = f32::MAX;
    let mut reached = false;
    for start in ring {
        world.place(start, team);
        world.idle(2);
        for _ in 0..(SECOND * 5) {
            if world
                .w
                .sim
                .enter_target(world.me())
                .is_some_and(|(v, _)| v == vh)
            {
                reached = true;
                break;
            }
            world.walk_at(pos, 0);
            closest = closest.min((world.feet() - pos).length());
        }
        if reached {
            world.tick(PadState {
                buttons: buttons::ACTION,
                ..Default::default()
            });
            let me = world.me();
            if world.w.sim.seat_of(me).is_some_and(|(v, _)| v == vh) {
                return Ok(());
            }
            let got = world.w.sim.seat_of(me).map(|(v, s)| (v.index, s));
            let f = world.feet();
            return Err(format!(
                "in reach at ({:.1},{:.1},{:.1}), ACTION gave seat {got:?}; stage {:?} target now {:?}",
                f.x, f.y, f.z,
                world.w.sim.spawn_stage.first(),
                world.w.sim.enter_target(me).map(|(v, s)| (v.index, s)),
            ));
        }
    }
    Err(format!(
        "never in reach; closest {closest:.1} m, its radius {reach:.1}"
    ))
}

fn vehicle_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    world.w.sim.cheats.invincible = true;
    world.place(Vec3::new(0.0, 900.0, 0.0), 0);
    // A pad's first vehicle comes on its first tick.
    world.idle(2);
    let before: Vec<(VehicleHandle, Vec3)> = world
        .w
        .sim
        .alive_vehicles()
        .map(|(h, v)| (h, v.pos))
        .collect();
    world.idle(SECOND * 5);
    let mut bad = 0;
    let handles: Vec<VehicleHandle> = before.iter().map(|(h, _)| *h).collect();
    let name_of =
        |w: &World, class: u32| w.classes.get(class as usize).cloned().unwrap_or_default();
    for (vh, p0) in &before {
        let Some(v) = world.w.sim.vehicle(*vh) else {
            bad += 1;
            out!(
                out,
                "vehicle {key} at ({:.1},{:.1},{:.1}): DIED standing empty for 5 s",
                p0.x,
                p0.y,
                p0.z
            );
            continue;
        };
        if !v.alive {
            bad += 1;
            let n = name_of(world, v.class);
            out!(
                out,
                "vehicle {key} {n} at ({:.1},{:.1},{:.1}): DIED standing empty for 5 s",
                p0.x,
                p0.y,
                p0.z
            );
            continue;
        }
        let drift = v.pos - *p0;
        if drift.length() > 3.0 {
            bad += 1;
            let n = name_of(world, v.class);
            out!(out, "vehicle {key} {n} at ({:.1},{:.1},{:.1}): moved ({:.1},{:.1},{:.1}) standing empty", p0.x, p0.y, p0.z, drift.x, drift.y, drift.z);
        }
    }
    let total = handles.len();
    for vh in handles {
        let Some(v) = world.w.sim.vehicle(vh).filter(|v| v.alive) else {
            continue;
        };
        let (class, kind, home, pos, rot, seats) =
            (v.class, v.kind, v.home_team, v.pos, v.rot, v.seats.len());
        let name = name_of(world, class);
        let team = home.unwrap_or(0);
        let tag = format!(
            "{key} {name} {kind:?} at ({:.1},{:.1},{:.1})",
            pos.x, pos.y, pos.z
        );
        if world
            .w
            .sim
            .seat_setup
            .get(class as usize)
            .and_then(|s| s.as_ref())
            .is_none()
        {
            bad += 1;
            out!(out, "vehicle {tag}: no seat data at all");
            continue;
        }
        if let Err(e) = walk_in(world, vh, team) {
            bad += 1;
            out!(out, "vehicle {tag}: CAN'T ENTER as team {team}: {e}");
            world.place(Vec3::new(0.0, 900.0, 0.0), 0);
            continue;
        }
        // Drive: a mover covers ground, on the stick both ways.
        if kind != VehicleKind::Turret {
            let start = world.w.sim.vehicle(vh).map_or(pos, |v| v.pos);
            let mut far = 0.0f32;
            let mut wrecked = None;
            'drive: for (ly, ticks) in [(i16::MAX, SECOND * 3), (i16::MIN, SECOND * 3)] {
                for t in 0..ticks {
                    world.tick(PadState {
                        ly,
                        buttons: if kind == VehicleKind::Flyer {
                            buttons::JUMP
                        } else {
                            0
                        },
                        ..Default::default()
                    });
                    match world.w.sim.vehicle(vh).filter(|v| v.alive) {
                        Some(v) => far = far.max((v.pos - start).length()),
                        None => {
                            wrecked = Some(t);
                            break 'drive;
                        }
                    }
                }
            }
            if let Some(t) = wrecked {
                out!(
                    out,
                    "info vehicle {tag}: wrecked driving straight on, {far:.1} m out, tick {t}"
                );
                world.place(Vec3::new(0.0, 900.0, 0.0), 0);
                continue;
            }
            if far < 2.0 {
                bad += 1;
                let me = world.me();
                let seat = world.w.sim.seat_of(me);
                let v = world.w.sim.vehicle(vh);
                out!(out, "vehicle {tag}: WON'T MOVE, {far:.2} m in 6 s of stick; seat {seat:?} hover-params {} stuck_at {:?} input {:?} vel {:?} ring {:?} team {:?}/{:?}", world.w.sim.hover_params.get(class as usize).is_some_and(Option::is_some), v.and_then(|v| v.stuck_at), v.map(|v| v.input), v.map(|v| v.vel), v.map(|v| v.seats.iter().map(|s| (s.kind, s.occupant.is_some())).collect::<Vec<_>>()), v.and_then(|v| v.team), home);
            }
        }
        // Each seat with a gun fires.
        let me = world.me();
        let guns = world
            .seat_guns
            .get(class as usize)
            .cloned()
            .unwrap_or_default();
        let mut seen = vec![false; seats];
        for _ in 0..seats {
            let Some((v_now, _)) = world.w.sim.seat_of(me) else {
                break;
            };
            if v_now != vh {
                break;
            }
            let seat = world
                .w
                .sim
                .unit(me)
                .and_then(|u| u.mount)
                .map_or(0, |m| usize::from(m.seat));
            if seen.get(seat).copied().unwrap_or(true) {
                world.press(buttons::SWITCH_WEAPON);
                continue;
            }
            seen[seat] = true;
            if guns.get(seat).copied().unwrap_or(false) {
                // Held for 6 s (a charged gun takes its charge), then let go.
                let mut shots = 0;
                for t in 0..(SECOND * 8) {
                    world.w.sim.events.clear();
                    world.tick(PadState {
                        buttons: if t < SECOND * 6 { buttons::FIRE } else { 0 },
                        ..Default::default()
                    });
                    shots += world
                        .w
                        .sim
                        .events
                        .iter()
                        .filter(|e| matches!(e, Event::Shot { .. }))
                        .count();
                    if shots > 0 && t < SECOND * 6 {
                        break;
                    }
                }
                if shots == 0 {
                    bad += 1;
                    let g = world
                        .w
                        .sim
                        .vehicle(vh)
                        .and_then(|v| v.guns.get(seat).cloned().flatten());
                    out!(out, "vehicle {tag}: seat {seat} HAS A GUN, 6 s of fire and a release gave no shot: {g:?}");
                }
            }
            world.press(buttons::SWITCH_WEAPON);
        }
        // Out, and able to walk off.
        let v_at = world.w.sim.vehicle(vh).map_or(pos, |v| v.pos);
        world.tick(PadState::default());
        world.press(buttons::ACTION);
        let mut tries = 0;
        while world.w.sim.seat_of(me).is_some() && tries < SECOND * 2 {
            world.press(buttons::ACTION);
            tries += 2;
        }
        if world.w.sim.seat_of(me).is_some() {
            bad += 1;
            out!(
                out,
                "vehicle {tag}: WON'T LET HIM OUT at ({:.1},{:.1},{:.1})",
                v_at.x,
                v_at.y,
                v_at.z
            );
        } else if kind != VehicleKind::Flyer {
            world.idle(SECOND);
            let f = world.feet();
            let walked = world.free_walk(f, team, SECOND);
            if walked < WALK_FREE_M {
                bad += 1;
                out!(
                    out,
                    "vehicle {tag}: OUT BUT STUCK at ({:.1},{:.1},{:.1}), {walked:.2} m",
                    f.x,
                    f.y,
                    f.z
                );
            }
            if (f.y - v_at.y).abs() > 8.0 {
                bad += 1;
                out!(
                    out,
                    "vehicle {tag}: left {:.1} m off its height, at ({:.1},{:.1},{:.1})",
                    f.y - v_at.y,
                    f.x,
                    f.y,
                    f.z
                );
            }
        }
        world.place(Vec3::new(0.0, 900.0, 0.0), 0);
        if let Some(v) = world.w.sim.vehicle_mut(vh) {
            v.pos = pos;
            v.rot = rot;
            v.vel = Vec3::ZERO;
        }
    }
    out!(
        out,
        "summary vehicle {key}: {total} vehicles, {bad} findings"
    );
}

fn station_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    let stations: Vec<(usize, Vec3, f32, String)> = world
        .w
        .sim
        .stations
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let n = world
                .w
                .sim
                .station_classes
                .get(s.class as usize)
                .map_or_else(String::new, |c| c.name.clone());
            (i, s.pos, s.radius, n)
        })
        .collect();
    let mut bad = 0;
    world.w.sim.cheats.invincible = false;
    let total = stations.len();
    for (i, pos, radius, name) in stations {
        let mut stand = None;
        'find: for r in [0.5f32, 1.0, 1.5, 2.0, 3.0, 4.0] {
            for k in 0..12 {
                let a = k as f32 * std::f32::consts::TAU / 12.0;
                let p = pos + Vec3::new(a.cos(), 0.0, a.sin()) * r;
                let g = world
                    .w
                    .sim
                    .ground_under(Vec3::new(p.x, pos.y + 2.0, p.z), false);
                if g > core::collision::NO_GROUND
                    && world
                        .w
                        .sim
                        .ground_under(Vec3::new(p.x, g + 0.5, p.z), false)
                        > g - 0.5
                    && (Vec3::new(p.x, g, p.z) - pos).length_squared() < radius * radius
                {
                    stand = Some(Vec3::new(p.x, g, p.z));
                    break 'find;
                }
            }
        }
        let Some(s) = stand else {
            bad += 1;
            out!(out, "station {key} {i} {name} at ({:.1},{:.1},{:.1}): NO FLOOR within its radius {radius:.1}", pos.x, pos.y, pos.z);
            continue;
        };
        world.place(s, 0);
        world.idle(3);
        let me = world.me();
        let f = world.feet();
        if let Some(u) = world.w.sim.unit_mut(me) {
            u.health.cur = (u.health.max * 0.25).max(1.0);
        }
        let hp0 = world.w.sim.unit(me).map_or(0.0, |u| u.health.cur);
        let mut hp1 = hp0;
        let mut dead = String::new();
        for _ in 0..(SECOND * 8) {
            world.w.sim.events.clear();
            world.tick(PadState::default());
            match world.w.sim.unit(me) {
                Some(u) => hp1 = hp1.max(u.health.cur),
                None => {
                    bad += 1;
                    out!(out, "station {key} {i} {name} at ({:.1},{:.1},{:.1}): DIED standing at it, {:?}", pos.x, pos.y, pos.z, world.w.sim.events);
                    dead = format!(
                        "{:?} at {:?}",
                        world
                            .w
                            .sim
                            .events
                            .iter()
                            .filter(|e| matches!(e, Event::EnvDeath { .. } | Event::Kill { .. }))
                            .collect::<Vec<_>>(),
                        world.feet()
                    );
                    break;
                }
            }
        }
        let station = world.w.sim.stations[i];
        let class = &world.w.sim.station_classes[station.class as usize];
        let gives = class.health[0] > 0.0;
        if gives && hp1 <= hp0 + 0.5 {
            bad += 1;
            out!(out, "station {key} {i} {name} at ({:.1},{:.1},{:.1}): NO HEALTH in 8 s at ({:.1},{:.1},{:.1}), {hp0:.0} to {hp1:.0}, delay {:.1}, team {:?}, destroyed {}; died: {dead}", pos.x, pos.y, pos.z, f.x, f.y, f.z, class.delay, station.team, station.destroyed);
        }
    }
    world.w.sim.cheats.invincible = true;
    out!(
        out,
        "summary station {key}: {total} stations, {bad} findings"
    );
}

fn door_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    world.w.sim.cheats.invincible = true;
    let n = world.w.sim.doors.len();
    let mut bad = 0;
    for i in 0..n {
        let origin = world.w.sim.doors[i].origin;
        let triggers = world.w.sim.doors[i].triggers.clone();
        let tag = format!(
            "{key} door {i} at ({:.1},{:.1},{:.1})",
            origin.x, origin.y, origin.z
        );
        world.place(Vec3::new(0.0, 900.0, 0.0), 0);
        world.idle(SECOND * 4);
        let shut = world.w.sim.doors[i].open;
        if shut > 0.01 {
            bad += 1;
            out!(out, "door {tag}: stays {shut:.2} open with nobody near");
        }
        if triggers.is_empty() {
            bad += 1;
            out!(out, "door {tag}: no trigger, never opens");
            continue;
        }
        let mut stood = None;
        'find: for t in &triggers {
            for r in [0.0f32, 1.0, 2.0, 3.0, 4.0] {
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::TAU / 8.0;
                    let p = t.center + Vec3::new(a.cos(), 0.0, a.sin()) * r;
                    let g = world
                        .w
                        .sim
                        .ground_under(Vec3::new(p.x, t.center.y + 2.0, p.z), false);
                    let q = Vec3::new(p.x, g, p.z);
                    if g > core::collision::NO_GROUND
                        && (q - t.center).length_squared() < t.radius * t.radius
                    {
                        stood = Some(q);
                        break 'find;
                    }
                }
            }
        }
        let Some(s) = stood else {
            bad += 1;
            out!(
                out,
                "door {tag}: NO FLOOR in any of its {} triggers",
                triggers.len()
            );
            continue;
        };
        world.place(s, 0);
        world.idle(SECOND * 6);
        let opened = world.w.sim.doors[i].open;
        if opened < 0.99 {
            bad += 1;
            out!(out, "door {tag}: only {opened:.2} open after 6 s standing in its trigger at ({:.1},{:.1},{:.1}), now at {:?}", s.x, s.y, s.z, world.feet());
        }
        // A straight walk through: from one side to the other, door opening
        // as he comes.
        let mut passed = false;
        let mut tried = 0;
        for k in 0..12 {
            let a = k as f32 * std::f32::consts::TAU / 12.0;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            let side = |w: &World, at: Vec3| {
                let g =
                    w.w.sim
                        .ground_under(Vec3::new(at.x, origin.y + 2.0, at.z), false);
                (g > core::collision::NO_GROUND && (g - origin.y).abs() < 2.0)
                    .then_some(Vec3::new(at.x, g, at.z))
            };
            let (Some(a0), Some(b0)) =
                (side(world, origin + d * 6.0), side(world, origin - d * 6.0))
            else {
                continue;
            };
            tried += 1;
            world.place(a0, 0);
            for _ in 0..(SECOND * 5) {
                world.walk_at(b0, 0);
                if (world.feet() - b0).with_y(0.0).length() < 1.5 {
                    passed = true;
                    break;
                }
            }
            if passed {
                break;
            }
        }
        if tried > 0 && !passed {
            bad += 1;
            out!(
                out,
                "door {tag}: NO STRAIGHT WALK through in {tried} tries from 6 m each side"
            );
        }
        world.place(Vec3::new(0.0, 900.0, 0.0), 0);
        world.idle(SECOND * 8);
        let closed = world.w.sim.doors[i].open;
        if closed > 0.01 {
            bad += 1;
            out!(out, "door {tag}: still {closed:.2} open 8 s after he left");
        }
    }
    out!(out, "summary door {key}: {n} doors, {bad} findings");
}

fn round_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    let mut bad = 0;
    for team in 0..2u8 {
        let points = world.w.sim.spawn_points(team).len();
        if points == 0 {
            bad += 1;
            out!(
                out,
                "round {key}: team {team} HAS NOWHERE TO SPAWN at the start"
            );
        }
    }
    let posts = world.w.sim.cps.len();
    world.w.sim.cheats.invincible = true;
    world.place(Vec3::new(0.0, 900.0, 0.0), 0);
    for cp in &mut world.w.sim.cps {
        cp.owner = Some(0);
    }
    let mut ended = None;
    for t in 0..(SECOND * 40) {
        world.tick(PadState::default());
        if let Some(winner) = world.w.sim.outcome {
            ended = Some((t, winner));
            break;
        }
    }
    match ended {
        Some((_, 0)) => {}
        Some((t, winner)) => {
            bad += 1;
            out!(
                out,
                "round {key}: holding all {posts} posts, team {winner} won at {} s",
                t / SECOND
            );
        }
        None => {
            bad += 1;
            out!(
                out,
                "round {key}: holding all {posts} posts for 40 s did not end the round; game {}",
                world.game
            );
        }
    }
    out!(out, "summary round {key}: {posts} posts, {bad} findings");
}

/// A cheap deterministic stream for the roaming sticks.
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Cell a spot is counted in.
type Cell = (i32, i32, i32);

fn cell(p: Vec3) -> Cell {
    (
        (p.x / 6.0).floor() as i32,
        (p.y / 6.0).floor() as i32,
        (p.z / 6.0).floor() as i32,
    )
}

#[derive(Default)]
struct Spots(std::collections::BTreeMap<(&'static str, Cell), (u32, Vec3)>);
impl Spots {
    fn add(&mut self, what: &'static str, p: Vec3) {
        let e = self.0.entry((what, cell(p))).or_insert((0, p));
        e.0 += 1;
    }
    fn write(&self, key: &str, out: &mut String) {
        let mut rows: Vec<_> = self.0.iter().collect();
        rows.sort_by_key(|(k, v)| (k.0, std::cmp::Reverse(v.0)));
        let mut per: std::collections::BTreeMap<&str, (u32, u32, u32)> = Default::default();
        for ((what, _), (n, p)) in rows {
            let e = per.entry(what).or_default();
            e.0 += 1;
            e.1 += *n;
            if e.0 <= 12 {
                out!(
                    out,
                    "spot {key} {what} x{n} at ({:.1},{:.1},{:.1})",
                    p.x,
                    p.y,
                    p.z
                );
            }
            e.2 = e.0;
        }
        for (what, (cells, n, _)) in per {
            out!(out, "tally {key} {what}: {cells} cells, {n} times");
        }
    }
}

/// Is `p` below the terrain, where the terrain is there to stand on?
fn under_terrain(world: &World, p: Vec3, by: f32) -> bool {
    world.w.sim.heightfield.as_ref().is_some_and(|h| {
        h.height_at(p.x, p.z) - p.y > by
            && world.w.sim.ground_under(p + Vec3::Y * 0.2, false) <= core::collision::NO_GROUND
    })
}

/// Can a soldier at `p` get `need` m away? Each of 16 directions three ways,
/// 4 s a try: sprinting, jumping, and crouched, as a player would try.
fn escape(world: &mut World, p: Vec3, team: u8, need: f32) -> f32 {
    let mut best = 0.0f32;
    for k in 0..48 {
        let a = (k / 3) as f32 * std::f32::consts::TAU / 16.0;
        world.place(p, team);
        let to = p + Vec3::new(a.cos(), 0.0, a.sin()) * 80.0;
        let crouched = k % 3 == 2;
        if crouched {
            world.tick(PadState::default());
            world.tick(PadState {
                buttons: buttons::ROLL_STANCE,
                ..Default::default()
            });
            world.idle(10);
        }
        for t in 0..(SECOND * 4) {
            let b = match k % 3 {
                0 => buttons::SPRINT,
                1 if t % 25 == 5 => buttons::SPRINT | buttons::JUMP,
                1 => buttons::SPRINT,
                _ => 0,
            };
            world.walk_at(to, b);
        }
        best = best.max((world.feet() - p).with_y(0.0).length());
        if best >= need {
            break;
        }
    }
    best
}

/// Every vehicle driven with nothing in mind for 40 s by stick, and then got
/// out of. Counts where it ends up that it should not. A flyer takes off.
fn drive_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    world.w.sim.cheats.invincible = true;
    world.place(Vec3::new(0.0, 900.0, 0.0), 0);
    world.idle(2);
    let handles: Vec<VehicleHandle> = world.w.sim.alive_vehicles().map(|(h, _)| h).collect();
    let mut rng = Lcg(0x0bad_cafe);
    let mut spots = Spots::default();
    let (mut driven, mut wrecked, mut drove) = (0, 0, 0.0f32);
    for vh in handles {
        let Some(v) = world
            .w
            .sim
            .vehicle(vh)
            .filter(|v| v.alive && v.kind != VehicleKind::Turret)
        else {
            continue;
        };
        let (class, kind, team, pos, rot) =
            (v.class, v.kind, v.home_team.unwrap_or(0), v.pos, v.rot);
        let name = world
            .classes
            .get(class as usize)
            .cloned()
            .unwrap_or_default();
        if walk_in(world, vh, team).is_err() {
            world.place(Vec3::new(0.0, 900.0, 0.0), 0);
            continue;
        }
        driven += 1;
        let mut last = pos;
        let mut under = false;
        let mut grounded = pos;
        let (mut lx, mut ly) = (0i16, i16::MAX);
        let mut alive = true;
        for t in 0..(SECOND * 40) {
            if t % 60 == 0 {
                lx = ((rng.next() - 0.5) * 65000.0) as i16;
                ly = if rng.next() < 0.85 {
                    i16::MAX
                } else {
                    i16::MIN
                };
            }
            world.tick(PadState {
                lx,
                ly,
                rx: lx / 2,
                buttons: if kind == VehicleKind::Flyer && t < 60 {
                    buttons::JUMP
                } else {
                    0
                },
                ..Default::default()
            });
            let Some(v) = world.w.sim.vehicle(vh).filter(|v| v.alive) else {
                alive = false;
                break;
            };
            let p = v.pos;
            if let Some(floor) = world.w.sim.kill_floor {
                if p.y < floor && !under {
                    under = true;
                    out!(out, "drive {key} {name} UNDER THE KILL FLOOR {floor:.1} at ({:.1},{:.1},{:.1}), from ({:.1},{:.1},{:.1}) after {t} ticks, last near a floor at ({:.1},{:.1},{:.1})", p.x, p.y, p.z, pos.x, pos.y, pos.z, grounded.x, grounded.y, grounded.z);
                }
            }
            if world.w.sim.vehicle(vh).is_some_and(|v| v.vel.y.abs() < 1.0) {
                grounded = p;
            }
            drove += (p - last).with_y(0.0).length();
            if (p - last).length() > 10.0 {
                out!(out, "drive {key} {name} JUMPED ({:.1},{:.1},{:.1}) to ({:.1},{:.1},{:.1}) in a tick", last.x, last.y, last.z, p.x, p.y, p.z);
            }
            last = p;
            if !p.is_finite() {
                out!(out, "drive {key} {name} NOT FINITE");
                break;
            }
            if under_terrain(world, p, 2.0) {
                spots.add("vehicle-under-terrain", p);
            }
            if world.w.sim.shut_in_solid(p + Vec3::Y).is_some() {
                spots.add("vehicle-in-solid", p);
            }
            let cam = world.w.camera().position;
            if under_terrain(world, cam, 0.3) {
                spots.add("seat-camera-under-terrain", p);
            }
        }
        if !alive {
            wrecked += 1;
        } else {
            let me = world.me();
            world.press(buttons::ACTION);
            let mut tries = 0;
            while world.w.sim.seat_of(me).is_some() && tries < 60 {
                world.press(buttons::ACTION);
                tries += 2;
            }
            if world.w.sim.seat_of(me).is_some() {
                out!(
                    out,
                    "drive {key} {name}: WON'T LET HIM OUT at ({:.1},{:.1},{:.1}), {kind:?}",
                    last.x,
                    last.y,
                    last.z
                );
            } else if kind != VehicleKind::Flyer {
                world.idle(SECOND);
                let f = world.feet();
                let got = world.free_walk(f, team, SECOND);
                if got < WALK_FREE_M {
                    out!(out, "drive {key} {name}: OUT BUT STUCK at ({:.1},{:.1},{:.1}) by a vehicle at ({:.1},{:.1},{:.1}), walked {got:.2}", f.x, f.y, f.z, last.x, last.y, last.z);
                }
            }
        }
        world.place(Vec3::new(0.0, 900.0, 0.0), 0);
        if let Some(v) = world.w.sim.vehicle_mut(vh) {
            v.pos = pos;
            v.rot = rot;
            v.vel = Vec3::ZERO;
        }
    }
    spots.write(&key, out);
    out!(
        out,
        "summary drive {key}: {driven} driven, {wrecked} wrecked, {drove:.0} m"
    );
}

/// Each pad's vehicle destroyed: does the pad bring another?
fn pad_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    world.w.sim.cheats.invincible = true;
    world.place(Vec3::new(0.0, 900.0, 0.0), 0);
    world.idle(2);
    if std::env::var_os("CENSUS_PADS").is_some() {
        for (c, name) in world.classes.iter().enumerate() {
            if let Some(Some(p)) = world.w.sim.flyer_params.get(c) {
                out!(out, "flyerclass {key} {name}: takeoff h {} t {} v {} landed {} landing t {} v {} speeds {}/{}/{} accel {} pitch {} turn {}", p.takeoff_height, p.takeoff_time, p.takeoff_speed, p.landed_height, p.landing_time, p.landing_speed, p.min_speed, p.mid_speed, p.max_speed, p.acceleration, p.pitch_rate, p.turn_rate);
            }
        }
        for (i, pad) in world.w.sim.vehicle_pads.iter().enumerate() {
            let v: Vec<String> = world
                .w
                .sim
                .alive_vehicles()
                .filter(|(_, v)| v.spawn_pad == Some(i as u32))
                .map(|(_, v)| {
                    format!(
                        "{} {:?} at ({:.2},{:.2},{:.2})",
                        world
                            .classes
                            .get(v.class as usize)
                            .cloned()
                            .unwrap_or_default(),
                        v.kind,
                        v.pos.x,
                        v.pos.y,
                        v.pos.z
                    )
                })
                .collect();
            out!(
                out,
                "padlist {key} {i} {} at ({:.2},{:.2},{:.2}) team {:?} cp {:?} time {}: {v:?}",
                pad.def.name,
                pad.def.pos.x,
                pad.def.pos.y,
                pad.def.pos.z,
                pad.team,
                pad.def.cp,
                pad.def.spawn_time
            );
        }
    }
    let mut bad = 0;
    let n = world.w.sim.vehicle_pads.len();
    let live: Vec<usize> = (0..n).collect();
    let on_pad = |w: &World, pad: usize| {
        w.w.sim
            .alive_vehicles()
            .filter(|(_, v)| v.spawn_pad == Some(pad as u32))
            .map(|(h, _)| h)
            .collect::<Vec<_>>()
    };
    for pad in live {
        let vehicles = on_pad(world, pad);
        let d = &world.w.sim.vehicle_pads[pad].def;
        let spawn_time = d.spawn_time;
        let team = world.w.sim.vehicle_pads[pad].team;
        if vehicles.is_empty() {
            if d.class_for(team).is_some() {
                bad += 1;
                out!(
                    out,
                    "pad {key} {pad} at {:?}: nothing on it, team {team:?}",
                    d.pos
                );
            }
            continue;
        }
        for h in &vehicles {
            world.w.sim.destroy_vehicle(*h, None);
        }
        let mut back = None;
        for t in 0..(SECOND * (spawn_time.min(600.0) as usize + 30)) {
            world.tick(PadState::default());
            if !on_pad(world, pad).is_empty() {
                back = Some(t);
                break;
            }
        }
        // A class's spawn time this long is the data saying "once" (BF1 Naboo).
        if back.is_none() && spawn_time < 10_000.0 {
            bad += 1;
            out!(
                out,
                "pad {key} {pad}: no new vehicle in {} s, spawn time {spawn_time}, team {team:?}",
                spawn_time as usize + 30
            );
        }
    }
    out!(out, "summary pad {key}: {n} pads, {bad} findings");
}

/// A real round of bots, to the end. What it did not use, and where its units
/// died of anything but a shot. `CENSUS_SEED` picks the seed.
fn bots_probe(world: &mut World, out: &mut String) {
    let key = world.key.clone();
    let seed: u32 = std::env::var("CENSUS_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(42);
    let defaults = game::conquest_defaults::for_map(&key, world.cw).unwrap_or(
        game::conquest_defaults::Defaults {
            combatants: [16; 2],
            tickets: [250; 2],
        },
    );
    world.w.sim.battle = core::battle::Battle::new(defaults.tickets);
    world.w.sim.team_combatants = Some(defaults.combatants);
    world.w.sim.vacate_player_slot(0);
    world.w.opening_wave(defaults.combatants, false);
    let posts = world.w.sim.cps.len();
    let mut captured = vec![[0u32; 2]; posts];
    let mut boarded = vec![0u32; world.classes.len()];
    let mut spots = Spots::default();
    let mut seated: Vec<bool> = Vec::new();
    let mut kills = 0u32;
    let mut sites: Vec<Option<(Vec3, u32)>> = Vec::new();
    let watch: Option<(usize, u32, u32)> = std::env::var("CENSUS_WATCH").ok().and_then(|v| {
        let p: Vec<u32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        Some((*p.first()? as usize, *p.get(1)?, *p.get(2)?))
    });
    let flyer_log = std::env::var_os("CENSUS_FLYERS").is_some();
    let mut phases: Vec<Option<(core::vehicle::flyer::FlightPhase, bool, u32)>> = Vec::new();
    let mut contact: Vec<[u32; 2]> = Vec::new();
    let mut aimed: Vec<[u32; 2]> = Vec::new();
    let mut nearest: Vec<[(f32, Vec3); 2]> = Vec::new();
    let mut grounded: Vec<(Vec3, u32)> = Vec::new();
    let mut last_seat: Vec<Option<(u32, u32)>> = Vec::new();
    let mut t = 0u32;
    while t < 30 * 60 * 30 && world.w.sim.outcome.is_none() {
        world.w.sim.events.clear();
        world.w.sim.step(&[]);
        t += 1;
        for e in &world.w.sim.events {
            match e {
                Event::Captured { cp, team } => captured[*cp as usize][usize::from(*team)] += 1,
                Event::Kill { .. } => kills += 1,
                Event::EnvDeath { unit, cause } => {
                    let p = world.w.sim.units[unit.index as usize].soldier.pos;
                    let what = match cause {
                        core::sim::DeathCause::KillFloor => "bot-killfloor",
                        core::sim::DeathCause::DeathRegion => "bot-region",
                        core::sim::DeathCause::Fall => "bot-fall",
                        core::sim::DeathCause::UnderMap => "bot-under",
                        core::sim::DeathCause::Lost => "bot-lost",
                        core::sim::DeathCause::Stalled => "bot-stalled",
                        core::sim::DeathCause::VehicleDestroyed => "bot-with-vehicle",
                        _ => "bot-other",
                    };
                    spots.add(what, p);
                    if std::env::var_os("CENSUS_DEATHS").is_some() {
                        let i = unit.index as usize;
                        let (g, gt) = grounded.get(i).copied().unwrap_or((Vec3::ZERO, 0));
                        let m = last_seat.get(i).copied().flatten();
                        out!(out, "death {key} t{t} {what} unit {i} team {} at ({:.1},{:.1},{:.1}); last on ground ({:.1},{:.1},{:.1}) {:.1} s ago; last seat {:?} {:.1} s ago; goal {:?} {:?}",
                            world.w.sim.units[i].team, p.x, p.y, p.z, g.x, g.y, g.z, (t - gt) as f32 / 30.0,
                            m.and_then(|(c, _)| world.classes.get(c as usize)), m.map_or(-1.0, |(_, st)| (t - st) as f32 / 30.0),
                            world.w.sim.units[i].ai.route.as_ref().map(|r| r.next_hub), world.w.sim.units[i].ai.route.as_ref().map(|r| r.dest_hub));
                    }
                }
                _ => {}
            }
        }
        if let Some((unit, from, to)) = watch {
            if (from..=to).contains(&t) {
                let u = &world.w.sim.units[unit];
                out!(out, "watch t{t} unit {unit} alive {} pos ({:.2},{:.2},{:.2}) vel ({:.1},{:.1},{:.1}) {:?} grounded {} mount {:?} route {:?} does {:?} lost {:?} jet {:?} ev {:?}", u.alive, u.soldier.pos.x, u.soldier.pos.y, u.soldier.pos.z, u.soldier.vel.x, u.soldier.vel.y, u.soldier.vel.z, u.soldier.stance, u.soldier.grounded, u.mount, u.ai.route.as_ref().map(|r| (r.next_hub, r.dest_hub)), u.ai.behaviour, u.ai.lost.map(|l| l.confirmed), u.mount.and_then(|m| world.w.sim.vehicle(m.vehicle)).map(|v| (v.flyer.phase, v.pos, v.vel.length(), v.spawn_pad, v.input)), world.w.sim.events.iter().filter(|e| format!("{e:?}").contains(&format!("index: {unit},"))).collect::<Vec<_>>());
            }
        }
        if flyer_log {
            for (h, v) in world.w.sim.alive_vehicles() {
                if v.kind != VehicleKind::Flyer {
                    continue;
                }
                let i = h.index as usize;
                if phases.len() <= i {
                    phases.resize(i + 1, None);
                }
                let now = (
                    v.flyer.phase,
                    v.seats.iter().any(|s| s.occupant.is_some()),
                    h.gen,
                );
                if phases[i] != Some(now) {
                    phases[i] = Some(now);
                    out!(out, "flyer t{t} #{i} {} {:?} pilot {} at ({:.2},{:.2},{:.2}) vel ({:.1},{:.1},{:.1}) ground {:.2}", world.classes.get(v.class as usize).cloned().unwrap_or_default(), now.0, now.1, v.pos.x, v.pos.y, v.pos.z, v.vel.x, v.vel.y, v.vel.z, world.w.sim.ground_under_with(v.pos, false, core::collision::HullFilter::Static));
                }
            }
        }
        if t.is_multiple_of(15) {
            contact.resize(world.w.sim.cps.len(), [0, 0]);
            aimed.resize(world.w.sim.cps.len(), [0, 0]);
            for i in 0..world.w.sim.units.len() {
                let team = usize::from(world.w.sim.units[i].team).min(1);
                if let Some(core::commander::Objective::Attack(c)) =
                    world.w.sim.commander[team].of(i as u32)
                {
                    if let Some(n) = aimed.get_mut(c as usize) {
                        n[team] += 1;
                    }
                }
            }
            for c in 0..world.w.sim.cps.len() {
                let owner = world.w.sim.cps[c].owner;
                for i in 0..world.w.sim.units.len() {
                    let team = world.w.sim.units[i].team;
                    if owner != Some(team) && world.w.sim.units[i].alive {
                        let d = world.w.sim.units[i]
                            .soldier
                            .pos
                            .distance(world.w.sim.cps[c].def.pos);
                        let at = world.w.sim.units[i].soldier.pos;
                        let k = usize::from(team).min(1);
                        nearest.resize(world.w.sim.cps.len(), [(f32::MAX, Vec3::ZERO); 2]);
                        if d < nearest[c][k].0 {
                            nearest[c][k] = (d, at);
                        }
                    }
                    if owner != Some(team) && world.w.sim.counts_in_cp(i, c) {
                        contact[c][usize::from(team).min(1)] += 1;
                    }
                }
            }
        }
        grounded.resize(world.w.sim.units.len(), (Vec3::ZERO, 0));
        last_seat.resize(world.w.sim.units.len(), None);
        for (i, u) in world.w.sim.units.iter().enumerate() {
            if u.alive && u.soldier.grounded && u.mount.is_none() {
                grounded[i] = (u.soldier.pos, t);
            }
            if let Some(c) = u
                .mount
                .and_then(|m| world.w.sim.vehicle(m.vehicle))
                .map(|v| v.class)
            {
                last_seat[i] = Some((c, t));
            }
        }
        if t.is_multiple_of(15) {
            seated.resize(world.w.sim.units.len(), false);
            sites.resize(world.w.sim.units.len(), None);
            for (i, u) in world.w.sim.units.iter().enumerate() {
                let on = u.alive && u.mount.is_some();
                if on && !seated[i] {
                    if let Some(c) = u
                        .mount
                        .and_then(|m| world.w.sim.vehicle(m.vehicle))
                        .map(|v| v.class)
                    {
                        if let Some(b) = boarded.get_mut(c as usize) {
                            *b += 1;
                        }
                    }
                }
                seated[i] = on;
                // A live bot on foot who has not gone 3 m for 60 s.
                match sites[i] {
                    _ if !u.alive || u.mount.is_some() => sites[i] = None,
                    Some((at, since)) if (u.soldier.pos - at).with_y(0.0).length() < 3.0 => {
                        // Overwatch is a defender at his post, on purpose.
                        let posted = matches!(u.ai.behaviour, core::ai::Behaviour::Overwatch);
                        if t - since > 30 * 60 && posted {
                            sites[i] = Some((at, t));
                        } else if t - since > 30 * 60 {
                            spots.add("bot-parked-60s", at);
                            if std::env::var_os("CENSUS_PARKED").is_some() {
                                let u = &world.w.sim.units[i];
                                out!(out, "parked {key} t{t} unit {i} team {} at ({:.1},{:.1},{:.1}) {:?} seat {:?} route {:?} does {:?} lost {:?}", u.team, at.x, at.y, at.z, u.soldier.stance, u.mount.and_then(|m| world.w.sim.vehicle(m.vehicle)).map(|v| (world.classes.get(v.class as usize).cloned(), v.pos)), u.ai.route.as_ref().map(|r| (r.next_hub, r.dest_hub)), u.ai.behaviour, u.ai.lost.map(|l| l.confirmed));
                            }
                            sites[i] = Some((at, t));
                        }
                    }
                    _ => sites[i] = Some((u.soldier.pos, t)),
                }
            }
        }
    }
    if std::env::var_os("CENSUS_POSTS").is_some() {
        for (i, c) in world.w.sim.cps.iter().enumerate() {
            out!(out, "postlist {key} {i} {} hub {:?} owner {:?} captures {:?} attackers counted {:?} sent {:?} nearest {:?}", c.def.name, c.nav_hub, c.owner, captured[i], contact.get(i), aimed.get(i), nearest.get(i).map(|n| n.map(|(d, _)| d)));
        }
    }
    let unused: Vec<String> = world
        .w
        .sim
        .cps
        .iter()
        .enumerate()
        .filter(|(i, _)| captured[*i] == [0, 0])
        .map(|(i, c)| {
            // Half-seconds an attacker stood counted in its region.
            let n = contact.get(i).map_or(0, |v| v[0] + v[1]);
            format!("{} {n}", c.def.name)
        })
        .collect();
    let never: Vec<String> = world
        .classes
        .iter()
        .enumerate()
        .filter(|(i, _)| boarded[*i] == 0)
        .map(|(_, n)| n.clone())
        .collect();
    spots.write(&key, out);
    let tickets = [
        world.w.sim.battle.teams[0].tickets(),
        world.w.sim.battle.teams[1].tickets(),
    ];
    out!(out, "bots {key} seed {seed}: {} s, outcome {:?}, tickets {tickets:?} from {:?}, kills {kills}; posts never captured {unused:?}; vehicle classes never boarded {never:?}", t / 30, world.w.sim.outcome, defaults.tickets);
}

/// `CENSUS_RAYS=x,y,z`: what the rays from there meet, up and level.
fn rays_probe(world: &mut World, out: &mut String) {
    let v: Vec<f32> = std::env::var("CENSUS_RAYS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    let p = Vec3::new(v[0], v[1], v[2]);
    let mut dirs: Vec<Vec3> = vec![Vec3::Y, Vec3::NEG_Y];
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        dirs.push(Vec3::new(a.cos(), 0.0, a.sin()));
    }
    for d in dirs {
        let hit = world.w.sim.collision.ray_with(
            p,
            d,
            300.0,
            core::collision::Layers::SOLDIER,
            core::collision::HullFilter::Static,
        );
        let name = hit.as_ref().and_then(|h| {
            let bodies = world.w.sim.collision.bodies();
            let e = p + d * h.t;
            bodies
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    b.aabb.min.x - 0.1 <= e.x
                        && e.x <= b.aabb.max.x + 0.1
                        && b.aabb.min.y - 0.1 <= e.y
                        && e.y <= b.aabb.max.y + 0.1
                        && b.aabb.min.z - 0.1 <= e.z
                        && e.z <= b.aabb.max.z + 0.1
                })
                .map(|(i, _)| world.body_names.get(i).cloned().unwrap_or_default())
                .next()
        });
        out!(
            out,
            "ray {} from {p:?} along ({:.2},{:.2},{:.2}): {:?} {name:?}",
            world.key,
            d.x,
            d.y,
            d.z,
            hit.map(|h| h.t)
        );
    }
    let terrain = world
        .w
        .sim
        .heightfield
        .as_ref()
        .map(|h| h.height_at(p.x, p.z));
    out!(
        out,
        "terrain {terrain:?} limits {:?} ceiling {:?}",
        world.w.sim.flight_limits,
        world.w.sim.fly_ceiling
    );
}

/// `CENSUS_ESCAPE=x,y,z`: from there, how far each of 16 directions gets, and
/// what stopped it.
fn escape_probe(world: &mut World, out: &mut String) {
    let v: Vec<f32> = std::env::var("CENSUS_ESCAPE")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    world.w.sim.cheats.invincible = true;
    let p = Vec3::new(v[0], v[1], v[2]);
    let names = world.body_names.clone();
    let name = |b: usize| names.get(b).cloned().unwrap_or_default();
    out!(
        out,
        "escape {} from ({},{},{}): ground here {:.2}; enclosing {:?}",
        world.key,
        p.x,
        p.y,
        p.z,
        world.w.sim.ground_under(p + Vec3::Y, false),
        world.w.sim.collision.enclosing(p + Vec3::Y).map(name)
    );
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let to = p + d * 80.0;
        world.place(p, 0);
        for _ in 0..(SECOND * 4) {
            world.walk_at(to, buttons::SPRINT);
        }
        let f = world.feet();
        let cap = world.w.sim.units[world.w.player.index as usize].capsule();
        let hit = world
            .w
            .sim
            .collision
            .wall_sweep_body(&cap.stepped(), d, 1.0);
        out!(
            out,
            "  dir {k} ({:.2},{:.2}): feet ({:.1},{:.1},{:.1}) {:.1} m, stopped by {:?}",
            d.x,
            d.z,
            f.x,
            f.y,
            f.z,
            (f - p).with_y(0.0).length(),
            hit.map(|h| (
                h.0,
                world
                    .w
                    .sim
                    .collision
                    .bodies()
                    .iter()
                    .position(|b| std::ptr::eq(b, h.2))
                    .map(name)
            ))
        );
    }
    let doors: Vec<String> = world
        .w
        .sim
        .doors
        .iter()
        .enumerate()
        .filter(|(_, d)| (d.origin - p).length() < 30.0)
        .map(|(i, d)| {
            format!(
                "door {i} at {:?} open {:.2} triggers {:?}",
                d.origin, d.open, d.triggers
            )
        })
        .collect();
    out!(out, "  doors near: {doors:?}");
}

/// `CENSUS_TRIS=x,y,z,r`: every collision triangle with a corner within r.
fn tris_probe(world: &mut World, out: &mut String) {
    let v: Vec<f32> = std::env::var("CENSUS_TRIS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    let c = Vec3::new(v[0], v[1], v[2]);
    let r = v[3];
    for (i, b) in world.w.sim.collision.bodies().iter().enumerate() {
        if !(b.aabb.min.x - r <= c.x
            && c.x <= b.aabb.max.x + r
            && b.aabb.min.y - r <= c.y
            && c.y <= b.aabb.max.y + r
            && b.aabb.min.z - r <= c.z
            && c.z <= b.aabb.max.z + r)
        {
            continue;
        }
        let name = world.body_names.get(i).cloned().unwrap_or_default();
        let core::collision::Shape::Mesh(m) = &b.shape else {
            out!(
                out,
                "body {i} {name}: prim layers {:?} aabb {:?}",
                b.layers,
                b.aabb
            );
            continue;
        };
        let mut n = 0;
        for t in m.triangles() {
            let w = |x: f32, y: f32, z: f32| b.to_world.transform_point3(Vec3::new(x, y, z));
            let (a, bb, cc) = (
                w(t.a.x, t.a.y, t.a.z),
                w(t.b.x, t.b.y, t.b.z),
                w(t.c.x, t.c.y, t.c.z),
            );
            let close = |p: Vec3| (p - c).length() < r;
            if close(a) || close(bb) || close(cc) {
                n += 1;
                let nrm = (bb - a).cross(cc - a).normalize_or_zero();
                out!(out, "  {i} {name} tri ({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2}) n ({:.2},{:.2},{:.2})", a.x, a.y, a.z, bb.x, bb.y, bb.z, cc.x, cc.y, cc.z, nrm.x, nrm.y, nrm.z);
            }
        }
        out!(
            out,
            "body {i} {name}: {n} triangles near, layers {:?} closed {}",
            b.layers,
            b.closed
        );
    }
}

/// A player with nothing in mind: from every spawn and hub, wander with stick,
/// sprint and jump. Counts what a player would trip over, by 6 m cell.
fn roam_probe(world: &mut World, out: &mut String) {
    const LEG: usize = SECOND * 30;
    let key = world.key.clone();
    let mut starts: Vec<Vec3> = Vec::new();
    for c in &world.w.sim.cps {
        for (p, _) in &c.def.spawn_nodes {
            starts.push(Vec3::new(p.x, world.w.sim.spawn_ground(*p), p.z));
        }
    }
    let spawn_starts = starts.len();
    if let Some(g) = world.w.sim.nav.as_ref() {
        let hubs: Vec<Vec3> = g.hubs.iter().map(|h| h.pos).collect();
        for (i, h) in hubs.iter().enumerate() {
            if let Some(f) = world.w.sim.nav_floor(i as u16) {
                starts.push(Vec3::new(h.x, f, h.z));
            }
        }
    }
    let mut spots = Spots::default();
    let mut rng = Lcg(0x1234_5678);
    world.w.sim.cheats.invincible = false;
    let (mut legs, mut traps) = (0, 0);
    let (mut walked, mut ticks) = (0.0f32, 0u32);
    let owners: Vec<Option<u8>> = world.w.sim.cps.iter().map(|c| c.owner).collect();
    let passes: Vec<(usize, &Vec3)> = starts
        .iter()
        .enumerate()
        .chain(starts.iter().enumerate())
        .chain(starts.iter().enumerate())
        .collect();
    for (si, s) in passes {
        let kind = if si < spawn_starts { "spawn" } else { "hub" };
        legs += 1;
        // He captures what he walks through: the round must not end on him.
        for (c, o) in world.w.sim.cps.iter_mut().zip(&owners) {
            c.owner = *o;
        }
        world.w.sim.domination = Default::default();
        world.w.sim.outcome = None;
        world.place(*s, 0);
        let mut yaw = rng.next() * std::f32::consts::TAU;
        let mut last = *s;
        let mut history: Vec<String> = Vec::new();
        let mut still = 0;
        for t in 0..LEG {
            if t % 75 == 0 || still > 15 {
                yaw = rng.next() * std::f32::consts::TAU;
                still = 0;
            }
            let f = world.feet();
            let to = f + Vec3::new(yaw.cos(), 0.0, yaw.sin()) * 40.0;
            let roll = rng.next();
            let b = if roll < 0.01 { buttons::JUMP } else { 0 }
                | if t % 200 < 120 { buttons::SPRINT } else { 0 };
            world.w.sim.events.clear();
            let was = world.feet();
            let was_vel = world.w.sim.units[world.w.player.index as usize].soldier.vel;
            world.walk_at(to, b);
            let f = world.feet();
            let vel = world.w.sim.units[world.w.player.index as usize].soldier.vel;
            history.push(format!("t{t} {b:#x} ({:.2},{:.2},{:.2}) v({:.1},{:.1},{:.1}) -> ({:.2},{:.2},{:.2}) v({:.1},{:.1},{:.1}) {:?}", was.x, was.y, was.z, was_vel.x, was_vel.y, was_vel.z, f.x, f.y, f.z, vel.x, vel.y, vel.z, world.w.sim.units[world.w.player.index as usize].soldier.stance));
            still = if (f - last).with_y(0.0).length() < 0.05 {
                still + 1
            } else {
                0
            };
            walked += (f - last).with_y(0.0).length().min(5.0);
            ticks += 1;
            last = f;
            for e in &world.w.sim.events {
                if let Event::EnvDeath { cause, .. } = e {
                    let what = match cause {
                        core::sim::DeathCause::KillFloor => "death-killfloor",
                        core::sim::DeathCause::DeathRegion => "death-region",
                        core::sim::DeathCause::Fall => "death-fall",
                        core::sim::DeathCause::UnderMap => "death-under",
                        core::sim::DeathCause::Lost => "death-lost",
                        core::sim::DeathCause::Stalled => "death-stalled",
                        core::sim::DeathCause::Suicide => "death-suicide",
                        _ => "death-other",
                    };
                    spots.add(what, f);
                    if matches!(cause, core::sim::DeathCause::UnderMap) {
                        out!(out, "under {key} at ({:.1},{:.1},{:.1}) from start ({:.1},{:.1},{:.1}), {t} ticks in", f.x, f.y, f.z, s.x, s.y, s.z);
                    }
                }
            }
            if world.w.sim.unit(world.me()).is_none() {
                break;
            }
            if under_terrain(world, f, 0.6) {
                spots.add("under-terrain", f);
            }
            if world.w.sim.shut_in_solid(f + Vec3::Y).is_some() {
                spots.add("in-solid", f);
            }
            let cam = world.w.camera().position;
            if under_terrain(world, cam, 0.3) {
                spots.add("camera-under-terrain", f);
            }
            if world.w.camera_in_body() {
                spots.add("camera-in-body", f);
            }
        }
        if world.w.sim.unit(world.me()).is_some() {
            let f = world.feet();
            let h = world.w.sim.unit(world.me()).map_or(0.0, |u| u.health.cur);
            if h > 0.0 {
                world.w.sim.cheats.invincible = true;
                let got = escape(world, f, 0, 6.0);
                world.w.sim.cheats.invincible = false;
                if got < 6.0 {
                    traps += 1;
                    spots.add("TRAP", f);
                    if std::env::var_os("CENSUS_HISTORY").is_some() && (f - *s).length() > 3.0 {
                        // The last time it moved a good step, and the ticks round it.
                        for h in &history {
                            out!(out, "  {h}");
                        }
                    }
                    out!(out, "trap {key} at ({:.1},{:.1},{:.1}) from {kind} start ({:.1},{:.1},{:.1}) {:.1} m off; start in solid {:?}, here in solid {:?}, best escape {got:.1} m", f.x, f.y, f.z, s.x, s.y, s.z, (f - *s).length(), world.w.sim.shut_in_solid(*s + Vec3::Y), world.w.sim.shut_in_solid(f + Vec3::Y));
                }
            }
        }
    }
    spots.write(&key, out);
    out!(out, "summary roam {key}: {legs} legs of {} s, {ticks} ticks, walked {walked:.0} m, {traps} traps; outcome {:?}", LEG / SECOND, world.w.sim.outcome);
}

/// `CENSUS_TRACE=x,y,z[,lookx,lookz]`: stand there, walking at the look point when given; print the feet.
fn trace_probe(world: &mut World, out: &mut String) {
    let v: Vec<f32> = std::env::var("CENSUS_TRACE")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect();
    world.w.sim.cheats.invincible = false;
    let sprint = std::env::var_os("CENSUS_SPRINT").is_some();
    let every = if std::env::var_os("CENSUS_TICKS").is_some() {
        1
    } else {
        6
    };
    let p = Vec3::new(v[0], v[1], v[2]);
    world.place(p, 0);
    for t in 0..240 {
        world.w.sim.events.clear();
        if v.len() >= 5 {
            let held = if sprint { buttons::SPRINT } else { 0 };
            world.walk_at(Vec3::new(v[3], p.y, v[4]), held);
        } else {
            world.tick(PadState::default());
        }
        let f = world.feet();
        let vel = world.w.sim.units[world.w.player.index as usize].soldier.vel;
        if t % every == 0 || !world.w.sim.events.is_empty() {
            out!(out, "t{t} feet ({:.2},{:.2},{:.2}) vel ({:.1},{:.1},{:.1}) ground_under {:.2} stance {:?} ev {:?}", f.x, f.y, f.z, vel.x, vel.y, vel.z, world.w.sim.ground_under(f + Vec3::Y * 0.5, false), world.w.sim.units[world.w.player.index as usize].soldier.stance, world.w.sim.events);
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| {
        args.iter().position(|a| a == name).map(|i| {
            let v = args.get(i + 1).cloned().unwrap_or_default();
            args.drain(i..=(i + 1).min(args.len() - 1));
            v
        })
    };
    let cw = flag("--era").as_deref() == Some("cw");
    let only = flag("--only");
    let want = |p: &str| match only.as_deref() {
        Some(o) => o.split(',').any(|x| x == p),
        None => !matches!(
            p,
            "trace" | "roam" | "escape" | "tris" | "drive" | "bots" | "rays"
        ),
    };
    let root = game::assets::baked_root();
    let mut maps: Vec<(String, PathBuf)> = std::fs::read_dir(&root)?
        .flatten()
        .filter(|e| e.path().join("manifest.ron").exists())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(k, _)| args.is_empty() || args.contains(k))
        .collect();
    maps.sort();
    let threads = std::env::var("SWEEP_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4usize)
        .clamp(1, maps.len().max(1));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<String>>> = Mutex::new(vec![None; maps.len()]);
    type Probe = fn(&mut World, &mut String);
    let probes: Vec<(&str, Probe)> = vec![
        ("spawn", spawn_probe),
        ("post", post_probe),
        ("vehicle", vehicle_probe),
        ("station", station_probe),
        ("door", door_probe),
        ("round", round_probe),
        ("trace", trace_probe),
        ("roam", roam_probe),
        ("escape", escape_probe),
        ("tris", tris_probe),
        ("drive", drive_probe),
        ("pad", pad_probe),
        ("bots", bots_probe),
        ("rays", rays_probe),
    ];
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((key, path)) = maps.get(i) else {
                    break;
                };
                let mut text = String::new();
                for (name, probe) in &probes {
                    if !want(name) {
                        continue;
                    }
                    // Each probe has a battle of its own: none sees another's state.
                    match build(key, path, cw) {
                        Ok(mut world) => probe(&mut world, &mut text),
                        Err(e) => {
                            out!(text, "{name} {key}: WOULD NOT BUILD: {e:#}");
                        }
                    }
                }
                eprintln!("{key} done");
                results.lock().expect("no panic holds this")[i] = Some(text);
            });
        }
    });
    for r in results
        .into_inner()
        .expect("no panic holds this")
        .into_iter()
        .flatten()
    {
        print!("{r}");
    }
    Ok(())
}
