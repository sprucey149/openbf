//! Walk mode: the local player inside `core::Sim` on a baked map — fixed-tick
//! (A5 spine) + third-person chase camera from the games' CAMERASECTION data.
//! P4: the dummy targets are real units with health; R2 fires the soldier's
//! rifle (a class with a laser look draws its bolts as `bolt_vis` ribbons;
//! the rest are thin boxes).
//!
//! Soldiers render as skinned models (game::soldier_vis); a team with no
//! baked model falls back to a stance-height box. Heights = BF2 capsule
//! table DAT_0061ed50
//! {1.9 stand, 1.6 crouch, 0.55 prone} (core::unit::capsule_height).
//!
//! A player in a vehicle seat (card VSEAT) is hidden and the camera is the
//! seat's ([`vehicle_camera`]). A zoomed player (card ZOOM) is framed by the
//! PS2's eased projection scale; in the scope view his own body is not drawn.
//! A building turret's gunner zoomed with R3 (card VTURZ) looks from the eye
//! point along the look ([`turret_scope_camera`]) with the turret class's
//! `FirstPersonFOV`; his own building is left out of his view by the scene
//! (`SceneGpu::view_draws`).
//!
//! Children: `camera_math` owns the pure FOV/camera helpers and team tints;
//! `visual` owns the class/HUD presentation types; `campaign` owns the Rise
//! of the Empire campaign step called from `step_tick`; `test_support` owns
//! the shared turret fixture. The camera *methods* live in `camera`,
//! `flyer_camera`, `death_camera`, `spectate` and `walker_view`.

mod walker_view;

use crate::camera_collide::CameraBlockers;
use crate::layout::{CommandPostDef, MapLayout};
use core::cp::CpDef;
use core::fmath::FloatExt;
use core::heightfield::Heightfield;
use core::region::Region;
use core::sim::{PadState, Sim};
use core::soldier::Soldier;
#[cfg(test)]
pub(crate) use core::soldier::Stance;
use core::unit::{capsule_height, Controller, UnitHandle, UnitSpec};
use glam::{Mat4, Quat, Vec3};
use render::Camera;

mod camera_math;
mod campaign;
#[cfg(test)]
mod test_support;
mod visual;
pub use camera_math::{
    chase_fov_y, chase_fovs, clamp_fov_scale, droid_camera, look_camera, ps2_fov_y, ps2_reticle_y,
    ps2_soldier_fovs, team_color, turret_scope_camera, vehicle_camera, vehicle_pull_height,
    DROID_CHASE, FOV_SCALE_RANGE, MAX_FOV_Y, PS2_FOV_TAN_SCALE, PS2_TAN_HALF_FOV_Y_BF1,
    PS2_TAN_HALF_FOV_Y_BF2, TEAM_COLORS, ZOOM_EASE_RATE,
};
pub(crate) use camera_math::{
    ease_camera_place, CAMERA_SNAP_SQUARED, CAMERA_TERRAIN_GAP, VIEW_SETTLE_EPSILON,
};
#[cfg(test)]
pub(crate) use test_support::turret_test_walk;
#[cfg(test)]
pub(crate) use test_support::{posts_walk, spawn_locals, two_locals_walk};
pub use visual::{
    faction_row, ClassVisual, Cries, DroidHud, TargetDraw, TeamTemplate, WeaponHud,
    LOW_HEALTH_THRESHOLD,
};

/// The point a wall pulls the chase camera in toward, this high above the
/// soldier's feet (m) in every stance: the PS2's own camera-collision
/// origin, the section's fixed point (0, 1.5, 0) that no key writes
/// (`re-aim-assist.md` §10.1, §10.6 step 4 [V]). Above his middle (issue
/// #36's 1.0 m point took the camera down to the floor beside a droid,
/// playtest 2026-10-28) and under his eye, so a close camera still shows
/// him.
pub const CAMERA_PULL_Y: f32 = 1.5;

/// The soldier's body middle, this high above his feet (m; at most his eye
/// point): the chase camera's second sight point, besides his eye, for the
/// props it sees through (`crate::camera_collide`, ours).
pub const CAMERA_SIGHT_BODY_Y: f32 = 1.0;

/// A pull point that moved farther than this in a tick starts the camera's
/// stored fraction afresh (m; B1 `FUN_001b58f0`'s 6400.0 on the squared
/// distance, B1:59356).
const CAM_PULL_SNAP_M: f32 = 80.0;

/// Which camera the world pulls in ([`WalkSim::cam_pull`]'s owner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CamKind {
    Soldier,
    Seat,
    Droid,
}

/// One frame's camera track: its kind, the point it is pulled in toward and
/// the fraction of it clear of the world; what the camera passes (a
/// gunner's building, a rider's vehicle) and the points it must see, for
/// the see-through test (a soldier's eye and middle; else the target).
#[derive(Debug, Clone, Copy)]
struct Track {
    kind: CamKind,
    target: Vec3,
    raw: f32,
    own_instance: Option<u32>,
    own_vehicle: Option<core::vehicle::VehicleHandle>,
    sights: [Vec3; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct ClassAssetRequest {
    pub team: usize,
    pub class: usize,
    pub game: &'static str,
}

impl ClassAssetRequest {
    pub fn templates(&self, walk: &WalkSim) -> [TeamTemplate; 2] {
        std::array::from_fn(|team| TeamTemplate {
            classes: if team == self.team {
                vec![walk.teams[team].classes[self.class].clone()]
            } else {
                Vec::new()
            },
            ..walk.teams[team].clone()
        })
    }
}

pub struct WalkSim {
    pub class_picker: Option<crate::class_picker::ClassPicker>,
    pub sim: Sim,
    pub conquest_defaults: Option<crate::conquest_defaults::Defaults>,
    pub team_size_pct: u32,
    pub reinforcement_pct: u32,
    /// Humans per team at the last tick (`refill_after_side_change`).
    pub(crate) human_teams: Option<[u32; 2]>,
    pending_class_kills: std::collections::BTreeMap<String, u32>,
    /// The local player's counters from before a dropped controller (issue #194, split only).
    round_carry: locals::RoundCarry,
    /// The further people playing on this machine (issue #194); empty with one local player.
    pub locals: Vec<locals::LocalPlayer>,
    /// Split (issue #194 B12): the last awake players' ears
    /// (`event_sounds::listeners`), heard from while every local player is away.
    pub(crate) held_ears: Option<crate::sounds::Listener>,
    /// The local player's unit: player slot [`WalkSim::local_slot`]
    /// (`Sim::players[local_slot]`).
    pub player: UnitHandle,
    /// The local player's slot (issue #168): 0 in single player and on a LAN
    /// host, the slot the host gave a guest. Set with
    /// [`WalkSim::set_local_slot`].
    pub local_slot: u8,
    /// The lockstep round's per-slot game state (issue #168,
    /// [`crate::netplay::RoundState`]); empty in single player.
    pub netplay: crate::netplay::RoundState,
    /// Pending menu input, retained across render frames with no sim tick.
    respawn_requested: bool,
    pub teams: [TeamTemplate; 2],
    pub native_teams: std::collections::BTreeMap<u8, TeamTemplate>,
    /// The models the chase camera must stay out of (visual only — never read
    /// by the sim, so it cannot affect determinism). Set by the caller once
    /// the scene is loaded; empty = terrain-only camera flooring.
    pub camera_blockers: CameraBlockers,
    /// Each vehicle class's `GeometryName`, by `Vehicle::class` (visual
    /// only, like `camera_blockers`): set with the pads
    /// (`crate::vehicle_pads::PadBuild::geometry`), drawn by
    /// `crate::viewer::VehicleVisuals`.
    pub vehicle_geometry: Vec<Option<String>>,
    /// Each vehicle class's seat ring with its camera props, by
    /// `Vehicle::class` (visual only): set with the pads
    /// (`crate::vehicle_pads::PadBuild::seats`), read by [`vehicle_camera`].
    pub vehicle_seats: Vec<Vec<crate::vehicle_class::SeatSpec>>,
    /// Each vehicle class's model top, by `Vehicle::class`
    /// ([`crate::viewer::model_tops`]), for [`vehicle_pull_height`].
    pub vehicle_tops: Vec<Option<f32>>,
    pub guided_tops: Vec<Option<f32>>,
    pub walker_rigs: Vec<Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>>,
    pub flyer_rigs: Vec<Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>>,
    /// Each posed hover class's rig ([`crate::viewer::hover_rigs`], card VPOSE).
    pub hover_rigs: Vec<Option<std::sync::Arc<crate::viewer::walker_pose::WalkerRig>>>,
    pub walker_shot_styles: std::collections::HashMap<usize, crate::walker_shots::Style>,
    pub flyer_exhaust: Vec<Vec<crate::vehicle_pads::FlyerExhaust>>,
    /// Each vehicle class's hover `WaterEffect` ([`crate::vehicle_pads::PadBuild::hover_water`]).
    pub hover_water: Vec<Option<String>>,
    /// Each vehicle class's tread scroll ([`crate::vehicle_pads::PadBuild::hover_treads`]).
    pub hover_treads: Vec<Vec<crate::vehicle_class::tread::Tread>>,
    pub vehicle_weapon_hud: Vec<Vec<Vec<WeaponHud>>>,
    /// Each vehicle class's seats' rider hangs, parallel to `vehicle_seats`
    /// (visual only, card VRIDE): read by `crate::soldier_vis`.
    pub vehicle_riders: Vec<Vec<crate::vehicle_class::RiderHang>>,
    /// Each vehicle class's engine sound keys, by `Vehicle::class` (sound
    /// only, card VSND): set with the pads, read by
    /// `crate::engine_sound::EngineSounds`.
    pub vehicle_engines: Vec<Option<crate::engine_sound::EngineClass>>,
    /// Each vehicle class's sound properties (collision, hurt, death), by `Vehicle::class`.
    pub vehicle_sounds: Vec<crate::vehicle_class::VehicleSoundClass>,
    pub sound_triggers: Vec<crate::ambience::SoundTrigger>,
    /// Each vehicle class's `Label`, by `Vehicle::class` (display only): set
    /// with the pads (`crate::vehicle_pads::PadBuild::labels`); the spawn
    /// screen names a command walker's post by it (card CWSP).
    pub vehicle_labels: Vec<Option<String>>,
    /// Each vehicle class's ODF name, by `Vehicle::class` (display only):
    /// set with the pads (`crate::vehicle_pads::PadBuild::classes`); the
    /// HUD's target readout names a picked vehicle by it (card HTGT).
    pub vehicle_classes: Vec<String>,
    /// Mission particle LOD bias (re-fx §8, FUN_0037a040).
    pub particle_lod_bias: f32,
    /// The camera's projection scale S: 1.0 unzoomed, eased toward the zoom
    /// of the player's weapon each tick ([`WalkSim::step_tick`]). Visual
    /// only, like the FOV.
    pub zoom_s: f32,
    /// How far the FOV has eased from the class's third-person FOV (0) to
    /// its first-person one (1), the zoomed view's.
    pub zoom_fov_mix: f32,
    pub cam_place: Option<(f32, Vec3)>,
    /// How far the chase camera lags a jetting soldier ([`core::unit::jet_camera_lag`]).
    pub cam_jet_lag: Vec3,
    pub cam_flyer: Option<FlyerCam>,
    pub cam_gunner: Option<GunnerCam>,
    pub cam_exit: Option<Vec3>,
    pub cam_handover: Option<Handover>,
    pub cam_owner: Option<CamOwner>,
    pub cam_guided: Option<GuidedCam>,
    seat_view: Option<Camera>,
    pub seat_view_timer: f32,
    /// The camera collision's stored track fraction (B1 `FUN_0019fd70`,
    /// [`crate::camera_collide::ease_pull`]): stepped each tick, it lets a
    /// pulled-in camera return slowly. Visual only.
    pub cam_pull: f32,
    /// Which camera [`WalkSim::cam_pull`] belongs to and its pull point at
    /// the last tick: another camera, or a point moved over 80 m, starts
    /// from its own fraction (the PS2 snaps past 80 m, B1:59356).
    cam_pull_track: Option<(CamKind, Vec3)>,
    /// The player's FOV option (profile `Settings::fov_scale`), clamped to
    /// [`FOV_SCALE_RANGE`]; 1.0 = the PS2's framing.
    pub fov_scale: f32,
    /// The commander's voice-overs (card ANNC; presentation only — the sim
    /// never reads it). None without a mission: the lines are the script's.
    pub voice: Option<crate::announce::Commander>,
    /// The battle's music (card AMBM; presentation only). None without a
    /// mission: the Musics are the script's.
    pub music: Option<crate::music::BattleMusic>,
    /// The sound sub-levels the mission's script loads (card SNDE,
    /// `MissionDef::sound_levels`; presentation only): the battle's sound
    /// library, foley table and engine records are theirs
    /// (`crate::foley::battle_levels`). Empty without a mission.
    pub sound_levels: Vec<String>,
    pub space_round: Option<std::sync::Arc<crate::space_assault::SpaceRound>>,
    /// Each command post's name key, `level.<map>.<label>`, by `Sim::cps`
    /// index (card EVMSG; presentation only): what the post is called in the
    /// disc's text table (`crate::event_feed`).
    pub post_keys: Vec<String>,
    /// The minimap's fixed part (card BMAP; presentation only): the map
    /// square, the image and the classes' icons. `None` draws no minimap.
    pub minimap: Option<crate::minimap::MapStatic>,
    /// The spawn screens' level-start view (card SSELB; presentation only):
    /// the mission's camera shots, cut on each screen change, until the
    /// player's first spawn. `None` = the battle's own view.
    pub backdrop: Option<crate::spawn_backdrop::Backdrop>,
    /// The mission's camera shots, for the end of the round's view.
    pub end_shots: Vec<crate::spawn_backdrop::CameraShot>,
    /// The end of the round once it is decided (issue #33,
    /// [`crate::round_end`]): its camera shot, banner and stats screens.
    pub round_end: Option<crate::round_end::RoundEnd>,
    /// The death camera while the player is dead on a ground map
    /// ([`death_camera`]; presentation only).
    pub death_cam: Option<DeathCam>,
    /// The life whose death the death camera last noticed (once per life).
    death_seen: Option<UnitHandle>,
    /// Watching teammates while the player cannot spawn (issue #182,
    /// [`spectate`]; presentation only).
    pub spectate: Spectate,
    /// The drawn frame's blend between the last two ticks (card RINT;
    /// presentation only - the sim never reads it): [`WalkSim::advance_with`]
    /// keeps the pose each tick starts from.
    pub blend: crate::render_blend::Blend,
    /// Rise of the Empire sequential campaign state (active on campaign missions only).
    pub campaign: Option<crate::campaign_runtime::CampaignRuntime>,
    pub world_events: Option<crate::campaign_runtime::WorldEventRuntime>,
    accum: f32,
}

/// A command post's spawn nodes as `(position, yaw)`. A post that names no
/// SpawnPath has none: `CanTeamSpawnAt` (B2 `FUN_00356880`) wants the path
/// or a class SpawnPointCount, and no post class authors one
/// (`re-command-walker-post.md` §3) - the campaigns' invisible objective
/// posts (geo1's CIS `obj3_wheel`) are never spawned at. A named path the
/// map lacks, or an empty one, keeps the fallback [`WalkSim::setup_conquest`]
/// has always used: one node 3 m off the post's +x/+z corner (OURS).
/// measure: which posts reach this fallback on the stock maps.
fn cp_spawn_nodes(layout: &MapLayout, cp: &CommandPostDef) -> Vec<(Vec3, f32)> {
    let Some(name) = cp.spawn_path.as_deref() else {
        return Vec::new();
    };
    layout
        .path(name)
        .map(|p| p.nodes.iter().map(|n| (n.pos, n.yaw)).collect())
        .filter(|v: &Vec<(Vec3, f32)>| !v.is_empty())
        .unwrap_or_else(|| vec![(cp.pos + Vec3::new(3.0, 0.0, 3.0), 0.0)])
}

/// The classes a map's posts are built from, for
/// [`WalkSim::set_hunt_class_change`]: the map's own (`classes.ron`) and the
/// shared ones of its source game (BF2's INGAME, BF1's COMMON).
pub fn hunt_post_classes(
    classes_dir: &std::path::Path,
    map_dir: &std::path::Path,
    source_game: &str,
) -> anyhow::Result<crate::classdb::ClassDb> {
    let mut db = crate::classdb::ClassDb::default();
    db.load_map(map_dir)?;
    let shared = classes_dir.join(if source_game.eq_ignore_ascii_case("bf2") {
        "bf2_ingame.ron"
    } else {
        "bf1_common.ron"
    });
    if pak::is_file(&shared) {
        db.load(&shared)?;
    }
    Ok(db)
}

/// `[ours]` (issue #161): the class-change reach of a BF1 Hunt post. BF1
/// has no class change and its post classes author no `SwitchClassRadius`;
/// they take BF2's `com_bldg_controlzone` value.
pub const HUNT_SWITCH_CLASS_RADIUS: f32 = 5.0;

impl WalkSim {
    /// Class presentation of a main or native unit.
    pub fn team_template(&self, team: u8) -> &TeamTemplate {
        self.teams
            .get(usize::from(team))
            .or_else(|| self.native_teams.get(&team))
            .unwrap_or(&self.teams[1])
    }

    /// `seed` feeds both sim RNGs (spread); replays carry it in the log.
    /// Default templates: BF1 troopers with the BF1 rifle.
    pub fn new(seed: u32, spawn: Vec3, hf: Option<Heightfield>) -> Self {
        let sim = Sim::empty(seed);
        let t = TeamTemplate::default_trooper();
        Self::with_teams(sim, hf, spawn, [t.clone(), t], None)
    }

    /// Build on a prepared sim (weapon classes registered) with per-team
    /// templates; the player spawns as a team-0 unit at `spawn`
    /// ([`WalkSim::with_teams_seat`] seats it on either team).
    ///
    /// `nav` is the map's authored planning graph
    /// (`game::layout::nav_graph`), an INPUT like the heightfield. It is set
    /// BEFORE any command post is added, because `Sim::add_cp` resolves each
    /// post's planning hub as it goes in. `None` = the AI walks straight at
    /// its post, as it did before Phase A.
    pub fn with_teams(
        sim: Sim,
        hf: Option<Heightfield>,
        spawn: Vec3,
        teams: [TeamTemplate; 2],
        nav: Option<core::nav::NavGraph>,
    ) -> Self {
        Self::with_teams_seat(sim, hf, spawn, teams, nav, 0)
    }

    /// [`WalkSim::with_teams`] with the player a unit of `player_team` (0 or
    /// 1): the team its army has in the mission picked (card ERAM).
    pub fn with_teams_seat(
        mut sim: Sim,
        hf: Option<Heightfield>,
        spawn: Vec3,
        teams: [TeamTemplate; 2],
        nav: Option<core::nav::NavGraph>,
        player_team: u8,
    ) -> Self {
        let seat = player_team.min(1);
        sim.heightfield = hf;
        sim.nav = nav;
        // A caller's spawn is an (x, z) with y = 0 unless it pinned one; the
        // ground probe starts from the authored y, so lift it onto the
        // terrain first — under a heightfield more than TERRAIN_BURY above
        // y = 0 the player would otherwise be placed at NO_GROUND.
        let spawn = match &sim.heightfield {
            Some(h) => Vec3::new(
                spawn.x,
                spawn.y.fmax(h.height_at(spawn.x, spawn.z)),
                spawn.z,
            ),
            None => spawn,
        };
        let player = sim
            .spawn(&teams[usize::from(seat)].spec(
                seat,
                spawn,
                0.0,
                Controller::Player { slot: 0 },
                0,
            ))
            .expect("fresh battle has reinforcements");
        Self {
            sim,
            player,
            local_slot: 0,
            netplay: Default::default(),
            conquest_defaults: None,
            team_size_pct: 100,
            reinforcement_pct: 100,
            human_teams: None,
            pending_class_kills: Default::default(),
            round_carry: Default::default(),
            respawn_requested: false,
            class_picker: None,
            camera_blockers: CameraBlockers::default(),
            vehicle_geometry: Vec::new(),
            vehicle_seats: Vec::new(),
            vehicle_tops: Vec::new(),
            guided_tops: Vec::new(),
            walker_rigs: Vec::new(),
            flyer_rigs: Vec::new(),
            hover_rigs: Vec::new(),
            vehicle_weapon_hud: Vec::new(),
            walker_shot_styles: Default::default(),
            flyer_exhaust: Vec::new(),
            hover_water: Vec::new(),
            hover_treads: Vec::new(),
            vehicle_riders: Vec::new(),
            vehicle_engines: Vec::new(),
            vehicle_sounds: Vec::new(),
            sound_triggers: Vec::new(),
            vehicle_labels: Vec::new(),
            vehicle_classes: Vec::new(),
            particle_lod_bias: 0.0,
            zoom_s: 1.0,
            zoom_fov_mix: 0.0,
            cam_place: None,
            cam_jet_lag: Vec3::ZERO,
            cam_flyer: None,
            cam_gunner: None,
            cam_exit: None,
            cam_handover: None,
            cam_owner: None,
            cam_guided: None,
            seat_view: None,
            seat_view_timer: 0.0,
            cam_pull: 1.0,
            cam_pull_track: None,
            fov_scale: 1.0,
            voice: None,
            music: None,
            sound_levels: Vec::new(),
            space_round: None,
            post_keys: Vec::new(),
            minimap: None,
            backdrop: None,
            end_shots: Vec::new(),
            round_end: None,
            death_cam: None,
            death_seen: None,
            spectate: Spectate::default(),
            teams,
            native_teams: Default::default(),
            blend: crate::render_blend::Blend::default(),
            campaign: None,
            world_events: None,
            accum: 0.0,
            locals: Vec::new(),
            held_ears: None,
        }
    }

    /// Drop a line of enemy troopers ahead of the spawn (one pacing
    /// sideways, reversing every 3 s), feet on the terrain.
    pub fn spawn_dummy_targets(&mut self) {
        let base = self.soldier().pos;
        for (dx, dz, vx) in [
            (-4.0, -18.0, 0.0),
            (5.0, -25.0, 0.0),
            (0.0, -35.0, 2.0), // pacer
        ] {
            let controller = if vx != 0.0 {
                Controller::Pacer {
                    vel: Vec3::new(vx, 0.0, 0.0),
                    period: 3 * core::TICK_HZ,
                }
            } else {
                Controller::Idle
            };
            self.sim.spawn(&UnitSpec {
                controller,
                ..UnitSpec::trooper(1, base + Vec3::new(dx, 0.0, dz))
            });
        }
    }

    /// Conquest setup from the baked layout: the posts
    /// ([`WalkSim::add_posts`]), then the opening wave
    /// ([`WalkSim::opening_wave`]). A battle with vehicle pads calls the two
    /// itself with `vehicle_pads::install` between them, so the wave also
    /// spawns at the command walkers its pads build (card CWSP).
    pub fn setup_conquest(
        &mut self,
        layout: &MapLayout,
        ai_per_team: [u32; 2],
        relocate_player: bool,
    ) {
        self.add_posts(layout);
        self.opening_wave(ai_per_team, relocate_player);
    }

    /// The layout's command posts into the sim (owner = mission Team − 1,
    /// spawn nodes from their SpawnPath, class timings by major / minor).
    pub fn add_posts(&mut self, layout: &MapLayout) {
        for cp in &layout.cps {
            let nodes = cp_spawn_nodes(layout, cp);
            let (capture_time, neutralize_time) = if cp.space {
                (f32::MAX, f32::MAX)
            } else {
                CpDef::class_times(cp.major)
            };
            let capture = cp.capture.unwrap_or_else(|| {
                if cp.capturable {
                    Region::sphere(cp.pos, 8.0)
                } else {
                    Region::sphere(Vec3::new(0.0, -10000.0, 0.0), 0.0)
                }
            });
            // The post's five per-faction rows collapse to the two the sim
            // needs, HERE, because `core` has no faction notion: team index t
            // takes the row of `self.teams[t].faction` (PLAN "Phase B
            // addendum").
            let rows = [
                faction_row(&self.teams[0].faction),
                faction_row(&self.teams[1].faction),
            ];
            let def = CpDef {
                name: cp.name.clone(),
                pos: cp.pos,
                capture,
                control: cp.control,
                value_bleed: cp.value_bleed,
                value_atk: [cp.value_atk[rows[0]], cp.value_atk[rows[1]]],
                value_def: [cp.value_def[rows[0]], cp.value_def[rows[1]]],
                spawn_nodes: nodes,
                capture_time,
                neutralize_time,
            };
            let owner = cp.team.checked_sub(1);
            self.sim.add_cp_unresolved(def, owner);
            if let Some(post) = self.sim.cps.last_mut() {
                post.ai_spawn_weight = cp.ai_spawn_weight;
                post.capturable = cp.capturable;
                post.visible = cp.visible;
                post.capture_authored = cp.capture.is_some();
            }
            self.post_keys
                .push(format!("level.{}.{}", layout.map, cp.label));
        }
        if !layout.cps.is_empty() {
            self.sim.resolve_posts();
        }
        // Card TPOS: the position table stands on the floors walked from
        // the posts just added.
        self.sim.build_positions();
    }

    /// Hunt (issue #161): each post's reach for a class change, after
    /// [`Self::add_posts`] with the same layout. On a BF2 map, the post
    /// class's `SwitchClassRadius` from `db` (the map's shared and own
    /// classes; BF2's control zones author 5 m); a BF2 class without one
    /// keeps the original's default of -1 (B2 `FUN_00269d98`), which no
    /// distance is under: no reach. `[ours]` every post on a BF1 map (BF1 has
    /// no class change) takes [`HUNT_SWITCH_CLASS_RADIUS`].
    /// measure: a BF2 Hunt class-change capture at a post's edge.
    pub fn set_hunt_class_change(
        &mut self,
        layout: &MapLayout,
        db: &crate::classdb::ClassDb,
        source_game: &str,
    ) {
        let bf2 = source_game.eq_ignore_ascii_case("bf2");
        for (post, cp) in self.sim.cps.iter_mut().zip(&layout.cps) {
            post.switch_class_radius = if bf2 {
                db.resolve(&cp.class)
                    .and_then(|class| db.prop_f32(class, "SwitchClassRadius"))
                    .filter(|r| *r > 0.0)
                    .unwrap_or(0.0)
            } else {
                HUNT_SWITCH_CLASS_RADIUS
            };
            post.capture_authored = cp.capture.is_some();
        }
    }

    /// The opening wave: `ai_per_team` AI spawned round-robin over their
    /// team's spawn points — its posts' nodes, then the points of the command
    /// walkers its pads are about to build (`Sim::pad_walker_spawn_points`;
    /// none unless the pads are installed) — and the player relocated to a
    /// friendly post unless the caller pinned a spawn.
    pub fn opening_wave(&mut self, ai_per_team: [u32; 2], relocate_player: bool) {
        self.opening_wave_with_slots(ai_per_team, relocate_player, false);
    }

    pub(crate) fn grow_roster(&mut self, additions: [u32; 2]) {
        self.opening_wave_with_slots(additions, false, true);
    }

    fn opening_wave_with_slots(
        &mut self,
        ai_per_team: [u32; 2],
        relocate_player: bool,
        preserve_respawns: bool,
    ) {
        if relocate_player {
            let seat = self.sim.units[self.player.index as usize].team;
            if let Some(&(pos, yaw)) = self.sim.spawn_points(seat).first() {
                let g = self.sim.ground_under(pos, false);
                let u = &mut self.sim.units[self.player.index as usize];
                u.soldier.pos = Vec3::new(pos.x, g, pos.z);
                u.soldier.yaw = yaw;
            }
        }
        for team in 0..2u8 {
            let mut points = self.sim.spawn_points(team);
            points.extend(self.sim.pad_walker_spawn_points(team));
            if points.is_empty() {
                continue;
            }
            // With a class table the AI come out of `Sim::spawn_ai` (the
            // mission's mix, `Sim::ai_class`); without one — every test and
            // every caller that never built a roster — they are the team
            // template, exactly as before.
            let has_classes = !self.sim.classes[team as usize].is_empty();
            for k in 0..ai_per_team[team as usize] as usize {
                let (pos, yaw) = points[k % points.len()];
                if has_classes {
                    if preserve_respawns {
                        self.sim.spawn_ai_roster_slot(team, pos, yaw);
                    } else {
                        self.sim.spawn_ai(team, pos, yaw);
                    }
                } else {
                    let spec = self.teams[team as usize].spec(team, pos, yaw, Controller::Ai, 0);
                    if preserve_respawns {
                        self.sim.spawn_roster_slot(&spec);
                    } else {
                        self.sim.spawn(&spec);
                    }
                }
            }
        }
    }

    /// Where the player starts on the plain `game --map` path (no
    /// `--mission`, no `--ai`, no `--spawn`): the LOWEST-INDEX friendly
    /// command post's own spawn node, on every map but one.
    ///
    /// Nothing calls [`WalkSim::setup_conquest`] on that path, so the player
    /// keeps the CLI's `--spawn` default — a single hard-coded point
    /// (`main.rs`'s `-140, 230`) picked on TAT1 in P3, which means nothing on
    /// the other 31 maps. On seven BF2 maps it is off the walkable surface
    /// entirely and the player starts ~300 m under the world on
    /// `Sim::spawn`'s [`core::collision::NO_GROUND`] sentinel (cor1, dea1,
    /// kam1, mus1, myg1, tan1, uta1 — measured); on the 24 that do have ground
    /// there it is an arbitrary spot in the terrain. Both are the same defect
    /// (HANDOFF §Standing backlog 7; queue items W and WS), so both take the
    /// same answer: start where the map says a friendly soldier comes out.
    ///
    /// Lowest index because the layout's post order is the manifest's, which
    /// is the bake's, so the choice is the same on every run; friendly =
    /// mission team 1, the same 1 → sim team 0 mapping
    /// [`WalkSim::setup_conquest`] uses, because that is where the player
    /// would come out once a mission existed. The post's spawn node, its yaw
    /// and its ground probe are `setup_conquest`'s own ([`cp_spawn_nodes`] +
    /// `ground_under`), so a later `--ai` run puts the player in the same
    /// place. A post whose own spawn node has no ground is skipped, and a map
    /// with no friendly post at all is left alone — a groundless player stays
    /// on the sentinel rather than being teleported somewhere unauthored.
    /// That is one map on the two discs: BF2 Hoth authors all 16 of its posts
    /// as team 2 (measured), because on that map ownership is the MISSION's to
    /// re-author (`MapLayout::apply_mission`) and this path has no mission.
    ///
    /// Recorded walk replays pin their start with `--spawn`; the caller skips
    /// this rule for any explicit spawn, on every map.
    ///
    /// Returns the post ONLY when the authored point had NO GROUND — the case
    /// the caller announces, in its own words, since before this rule existed.
    /// An ordinary post start announces itself here, so that every line
    /// printed says what actually happened.
    pub fn rescue_spawn(&mut self, layout: &MapLayout) -> Option<(String, f32)> {
        let grounded = self
            .sim
            .ground_hit_under(self.soldier().pos, false)
            .is_some();
        let (post, y) = self.move_to_friendly_post(layout)?;
        if grounded {
            println!("spawn: no mission — starting at command post {post} (y {y:.2})");
            return None;
        }
        Some((post, y))
    }

    /// Move the player to the lowest-index friendly command post whose own
    /// spawn node has ground, and return that post's name and the y they
    /// landed on. `None` = no friendly post has ground; the player is left
    /// exactly where they were. See [`WalkSim::rescue_spawn`] for why this
    /// post and not another.
    fn move_to_friendly_post(&mut self, layout: &MapLayout) -> Option<(String, f32)> {
        let own = self.sim.units[self.player.index as usize].team + 1;
        for cp in layout.cps.iter().filter(|c| c.team == own) {
            let Some(&(pos, yaw)) = cp_spawn_nodes(layout, cp).first() else {
                continue;
            };
            let Some(g) = self.sim.ground_hit_under(pos, false) else {
                continue;
            };
            let u = &mut self.sim.units[self.player.index as usize];
            u.soldier.pos = Vec3::new(pos.x, g, pos.z);
            u.soldier.yaw = yaw;
            return Some((cp.name.clone(), g));
        }
        None
    }

    /// The player's soldier (even when dead — the slot persists).
    pub fn soldier(&self) -> &Soldier {
        &self.sim.units[self.player.index as usize].soldier
    }

    /// The player's team, dead or alive (the slot keeps it between lives).
    pub fn player_team(&self) -> u8 {
        self.sim.units[self.player.index as usize].team
    }

    /// A space battle ([`core::sim::Sim::space_round`]): the pause menu,
    /// resume, pause audio, pad look, death flow, debris and the lost
    /// vehicle's view keep their space behaviour there.
    pub fn space_battle(&self) -> bool {
        self.sim.space_round()
    }

    /// Is the player on foot (alive and in no seat)? A rider is not drawn.
    pub fn player_on_foot(&self) -> bool {
        self.sim
            .unit(self.player)
            .is_some_and(|u| u.mount.is_none())
    }

    pub fn apply_sprint_latch(&self, latch: &mut input::SprintLatch, pad: PadState) -> PadState {
        self.apply_sprint_latch_with_buffer(latch, pad, false)
    }

    pub fn apply_sprint_latch_with_buffer(
        &self,
        latch: &mut input::SprintLatch,
        pad: PadState,
        buffer_start: bool,
    ) -> PadState {
        let weapon = self.sim.units[self.player.index as usize].weapon_class;
        latch.apply_with_start_buffer(
            pad,
            self.player_on_foot(),
            self.soldier().stance == core::soldier::Stance::Sprint,
            self.sim.weapon_classes[weapon].saber.is_none(),
            buffer_start,
        )
    }

    /// Is the player in a flyer's pilot seat? (`input::flyer_sticks` applies.)
    pub fn player_pilots_flyer(&self) -> bool {
        self.sim
            .unit(self.player)
            .and_then(|u| u.mount)
            .filter(|m| m.seat == 0)
            .and_then(|m| self.sim.vehicle(m.vehicle))
            .is_some_and(|v| v.kind == core::vehicle::VehicleKind::Flyer)
    }

    /// The controller look sensitivity a battle takes from the profile's
    /// `value`: space battles keep the stick as it is.
    pub fn pad_look_for_battle(&self, value: f32) -> f32 {
        match self.space_battle() {
            true => 1.0,
            false => crate::profile::clamp_pad_look(value),
        }
    }

    /// The right stick's scale for the player's seat now. On foot it is the
    /// battle's look sensitivity over [`input::LOOK_SCALE`] (ours, the
    /// soldier camera's feel). In any vehicle seat the stick reaches 1 at a
    /// full throw, as the PS2's does (`re-look-ramp.md` §1 [M]: BF2's stick
    /// word is 1.0 at a full throw and the game applies no sensitivity),
    /// so the full-stick timer (turn² + pitch² ≥ 0.9, B2 `FUN_002b0548`,
    /// `re-hover.md` §5.2) runs and a hover turns at its TurnRate rather
    /// than its PitchRate. A flyer's own sticks undo the scale the same way
    /// (`input::flyer_sticks`).
    pub fn pad_look_for_seat(&self, value: f32) -> f32 {
        match self.player_on_foot() {
            true => self.pad_look_for_battle(value),
            false => 1.0 / input::LOOK_SCALE,
        }
    }

    /// Is the player piloting a flyer or controlling a guided rocket?
    pub fn player_controls_flyer(&self) -> bool {
        self.flight_controls(input::FlightControls::Classic)
            .is_some()
    }

    /// Choose the controller layout for the player's current flight controls.
    /// The ship preference applies in the pilot seat. Guided ordnance uses
    /// its own Classic input scheme.
    pub fn flight_controls(&self, pilot: input::FlightControls) -> Option<input::FlightControls> {
        if self.player_pilots_flyer() {
            Some(pilot)
        } else {
            self.sim
                .controlled_guided(self.player)
                .map(|_| input::FlightControls::Classic)
        }
    }

    /// Classic keeps translation on the left for aircraft that can stop,
    /// reverse and strafe; continuously flying aircraft steer with the left.
    /// Select by supported motion capabilities, never by map or class name.
    pub fn map_flight_sticks(
        &self,
        pilot: input::FlightControls,
        pad: PadState,
    ) -> Option<PadState> {
        let layout = self.flight_controls(pilot)?;
        let hover = self
            .sim
            .unit(self.player)
            .and_then(|u| u.mount)
            .and_then(|m| self.sim.vehicle(m.vehicle))
            .and_then(|v| self.sim.flyer_params.get(v.class as usize))
            .and_then(Option::as_ref)
            .is_some_and(|p| {
                // Only the BF1 flight model implements continuous lateral translation
                // (core::vehicle::flyer::fly); BF2 uses this axis for roll.
                !p.bf2 && p.min_speed < 0.0 && p.mid_speed == 0.0 && p.strafe_speed > 0.0
            });
        Some(if hover {
            layout.map_hover_sticks(pad)
        } else {
            layout.map_sticks(pad)
        })
    }

    /// The magnet target when it is a unit (draw order of `target_draws`;
    /// a vehicle magnet draws no unit marker). Slot 0 — what the HUD draws.
    pub fn magnet(&self) -> Option<UnitHandle> {
        self.sim
            .magnet_at(usize::from(self.local_slot))
            .and_then(core::sim::AimTarget::unit)
    }

    /// Make player slot `slot` the local player (issue #168): the camera,
    /// the HUD and the profile's career follow it. Until the slot holds a
    /// unit (a guest joining mid-round) the view keeps the current one.
    pub fn set_local_slot(&mut self, slot: u8) {
        self.local_slot = slot;
        if let Some(Some(h)) = self.sim.players.get(usize::from(slot)) {
            self.player = *h;
        }
    }

    /// A copy of this round's game for the LAN host's self-check (issue
    /// #168, `crate::netplay::selfcheck::Checkpoint`): everything a tick
    /// reads or writes, so the copy stepped through the same bundles is the
    /// same game. Each field is named (a new one must be decided here):
    /// - The sim and the walk's own round state are copied: `sim`,
    ///   `netplay`, `human_teams`, `conquest_defaults`, the team size and
    ///   reinforcement shares, `teams` and `native_teams` (class costs read
    ///   their names), `player` and `local_slot`.
    /// - The small per-class tables and the cameras' and zoom's state are
    ///   copied too: a tick eases them, and nothing then reads a missing
    ///   class.
    /// - The window's own state is left out: the class picker, the camera
    ///   blockers, the voice-overs and music, the minimap, the render
    ///   blend, a pending respawn press and the career's pending class
    ///   kills (only the live walk's are the player's).
    pub fn checkpoint(&self) -> WalkSim {
        WalkSim {
            class_picker: None,
            sim: self.sim.clone(),
            conquest_defaults: self.conquest_defaults,
            team_size_pct: self.team_size_pct,
            reinforcement_pct: self.reinforcement_pct,
            human_teams: self.human_teams,
            pending_class_kills: Default::default(),
            round_carry: Default::default(),
            locals: Vec::new(),
            held_ears: None,
            player: self.player,
            local_slot: self.local_slot,
            netplay: self.netplay.clone(),
            respawn_requested: false,
            teams: self.teams.clone(),
            native_teams: self.native_teams.clone(),
            camera_blockers: CameraBlockers::default(),
            vehicle_geometry: self.vehicle_geometry.clone(),
            vehicle_seats: self.vehicle_seats.clone(),
            vehicle_tops: self.vehicle_tops.clone(),
            guided_tops: self.guided_tops.clone(),
            walker_rigs: self.walker_rigs.clone(),
            hover_rigs: self.hover_rigs.clone(),
            flyer_rigs: self.flyer_rigs.clone(),
            walker_shot_styles: self.walker_shot_styles.clone(),
            flyer_exhaust: self.flyer_exhaust.clone(),
            hover_water: self.hover_water.clone(),
            hover_treads: self.hover_treads.clone(),
            vehicle_weapon_hud: self.vehicle_weapon_hud.clone(),
            vehicle_riders: self.vehicle_riders.clone(),
            vehicle_engines: self.vehicle_engines.clone(),
            vehicle_sounds: self.vehicle_sounds.clone(),
            sound_triggers: self.sound_triggers.clone(),
            vehicle_labels: self.vehicle_labels.clone(),
            vehicle_classes: self.vehicle_classes.clone(),
            particle_lod_bias: self.particle_lod_bias,
            zoom_s: self.zoom_s,
            zoom_fov_mix: self.zoom_fov_mix,
            cam_place: self.cam_place,
            cam_jet_lag: self.cam_jet_lag,
            cam_flyer: self.cam_flyer,
            cam_gunner: self.cam_gunner,
            cam_exit: self.cam_exit,
            cam_handover: self.cam_handover,
            cam_owner: self.cam_owner,
            cam_guided: self.cam_guided,
            seat_view: self.seat_view,
            seat_view_timer: self.seat_view_timer,
            cam_pull: self.cam_pull,
            cam_pull_track: self.cam_pull_track,
            fov_scale: self.fov_scale,
            voice: None,
            music: None,
            sound_levels: Vec::new(),
            space_round: self.space_round.clone(),
            post_keys: self.post_keys.clone(),
            minimap: None,
            backdrop: self.backdrop.clone(),
            end_shots: self.end_shots.clone(),
            round_end: self.round_end.clone(),
            death_cam: self.death_cam,
            death_seen: self.death_seen,
            spectate: self.spectate,
            blend: crate::render_blend::Blend::default(),
            campaign: self.campaign.clone(),
            world_events: self.world_events.clone(),
            accum: self.accum,
        }
    }

    /// A LAN host's self-correction (issue #168, `crate::netplay::selfcheck`):
    /// this round's game becomes `replayed`'s, the same round replayed from
    /// a fresh build or a [`WalkSim::checkpoint`] through the round's log to
    /// this tick. Each field is named (a new one must be decided here):
    /// - From the replay, the game: `sim`, `netplay`, `human_teams`, the
    ///   population settings, `teams` and `native_teams` (a command may add
    ///   a class), and `player` (followed again from the local slot).
    /// - Kept, the career: the pending class kills (and the split players'
    ///   dropped-controller counters, `round_carry`) are the live walk's,
    ///   kills since the last flush. The replay's count from its own start
    ///   and would credit kills already flushed a second time; the medal
    ///   counts are the sim's, so the replay's.
    /// - Kept, what the window set after the build or eases from frame to
    ///   frame: the class picker, the per-class tables, the cameras, zoom
    ///   and FOV, the voice-overs and music, the minimap, the spawn
    ///   backdrop, the round's end screens, the death camera, a pending
    ///   respawn press and the accumulator. The view does not start over.
    /// - The render blend is kept but forgets its last pose: it was the old
    ///   game's.
    pub fn adopt_round(&mut self, replayed: WalkSim) {
        let WalkSim {
            class_picker: _,
            sim,
            conquest_defaults,
            team_size_pct,
            reinforcement_pct,
            human_teams,
            pending_class_kills: _,
            round_carry: _,
            locals: _,
            held_ears: _,
            player,
            local_slot: _,
            netplay,
            respawn_requested: _,
            teams,
            native_teams,
            camera_blockers: _,
            vehicle_geometry: _,
            vehicle_seats: _,
            vehicle_tops: _,
            guided_tops: _,
            walker_rigs: _,
            hover_rigs: _,
            flyer_rigs: _,
            walker_shot_styles: _,
            flyer_exhaust: _,
            hover_water: _,
            hover_treads: _,
            vehicle_weapon_hud: _,
            vehicle_riders: _,
            vehicle_engines: _,
            vehicle_sounds: _,
            sound_triggers: _,
            vehicle_labels: _,
            vehicle_classes: _,
            particle_lod_bias: _,
            zoom_s: _,
            zoom_fov_mix: _,
            cam_place: _,
            cam_jet_lag: _,
            cam_flyer: _,
            cam_gunner: _,
            cam_exit: _,
            cam_handover: _,
            cam_owner: _,
            cam_guided: _,
            seat_view: _,
            seat_view_timer: _,
            cam_pull: _,
            cam_pull_track: _,
            fov_scale: _,
            voice: _,
            music: _,
            sound_levels: _,
            space_round: _,
            post_keys: _,
            minimap: _,
            backdrop: _,
            end_shots: _,
            round_end: _,
            death_cam: _,
            death_seen: _,
            spectate: _,
            blend: _,
            campaign: _,
            world_events,
            accum: _,
        } = replayed;
        self.sim = sim;
        self.conquest_defaults = conquest_defaults;
        self.team_size_pct = team_size_pct;
        self.reinforcement_pct = reinforcement_pct;
        self.human_teams = human_teams;
        self.player = player;
        self.netplay = netplay;
        self.teams = teams;
        self.native_teams = native_teams;
        self.world_events = world_events;
        self.blend.reset();
        let slot = self.local_slot;
        self.set_local_slot(slot);
        // Every further local player too (issue #194 B18): the replay's units
        // may sit at other indices or generations.
        self.follow_locals();
    }

    /// One lockstep tick (issue #168): `bundle`'s pads by player slot (a
    /// slot it does not list gets the default pad), stepped like a tick of
    /// [`WalkSim::advance_with`] (the render blend's pose first, `each`
    /// after). Its session commands must already be applied
    /// (`crate::netplay::step` does both).
    pub fn step_bundle(&mut self, bundle: &net::TickBundle, each: impl FnOnce(&mut WalkSim)) {
        let top = bundle.pads.last().map_or(0, |p| usize::from(p.0) + 1);
        let mut pads = vec![PadState::default(); top.max(self.sim.players.len())];
        for (slot, pad) in &bundle.pads {
            pads[usize::from(*slot)] = *pad;
        }
        let mut blend = std::mem::take(&mut self.blend);
        blend.before_tick(self);
        self.blend = blend;
        self.step_tick(&pads);
        each(self);
    }

    /// Advance the fixed-tick sim by a real frame's dt (A5: 30 Hz, accumulator
    /// capped at 0.1 s). Returns ticks stepped.
    pub fn advance(&mut self, real_dt: f32, pads: &[PadState]) -> u32 {
        self.advance_with(real_dt, pads, |_| {})
    }

    /// [`WalkSim::advance`], calling `each` after every tick it steps.
    /// `Sim::events` holds ONE tick's events, until the next step: whatever
    /// turns them into something seen or heard (effects, event sounds, bolt
    /// remnants, the message box) reads them here — once per tick, whether
    /// the frame steps none (every other frame at 60 Hz), one or several.
    /// The mutable hook also publishes event-derived inputs for the next tick.
    pub fn advance_with(
        &mut self,
        real_dt: f32,
        pads: &[PadState],
        mut each: impl FnMut(&mut WalkSim),
    ) -> u32 {
        self.advance_while(real_dt, pads, |walk| {
            each(walk);
            true
        })
    }

    /// Stop after the tick that requests a modal presentation. Drop any
    /// pending wall time so it cannot become catch-up ticks after the movie.
    pub fn advance_while(
        &mut self,
        real_dt: f32,
        pads: &[PadState],
        mut each: impl FnMut(&mut WalkSim) -> bool,
    ) -> u32 {
        self.accum = (self.accum + real_dt).fmin(core::MAX_FRAME_ACCUM_SECS);
        let mut ticks = 0;
        let mut pads = std::borrow::Cow::Borrowed(pads);
        while self.accum >= core::TICK_DT {
            let mut blend = std::mem::take(&mut self.blend);
            blend.before_tick(self);
            self.blend = blend;
            self.step_tick(&pads);
            if pads.iter().any(PadState::has_once) {
                pads.to_mut().iter_mut().for_each(PadState::clear_once);
            }
            let keep_stepping = each(self);
            self.accum -= core::TICK_DT;
            ticks += 1;
            if !keep_stepping {
                self.accum = 0.0;
                break;
            }
        }
        ticks
    }

    /// The accumulator's left-over after the last frame's ticks (s): how far
    /// the frame is into the next tick (`render_blend::fraction`).
    pub fn accum(&self) -> f32 {
        self.accum
    }

    /// A lockstep peer (issue #168) steps bundles instead of
    /// [`WalkSim::advance_with`]: its playback's fraction toward the next
    /// tick (0..1) stands in for the accumulator's, for the render blend.
    pub fn set_frame_fraction(&mut self, f: f32) {
        self.accum = f.clamp(0.0, 1.0) * core::TICK_DT;
    }

    /// Write the pose blended between the last two ticks in, for the frame's
    /// draw lists (card RINT, `render_blend::Blend::apply`, at the
    /// accumulator's fraction); [`WalkSim::blend_out`] must follow before a
    /// tick or anything but drawing reads the walk.
    pub fn blend_in(&mut self) {
        let t = crate::render_blend::fraction(self);
        let mut blend = std::mem::take(&mut self.blend);
        blend.apply(self, t);
        self.blend = blend;
    }

    /// The last tick's own pose back after [`WalkSim::blend_in`] (a no-op
    /// without one).
    pub fn blend_out(&mut self) {
        let mut blend = std::mem::take(&mut self.blend);
        blend.restore(self);
        self.blend = blend;
    }

    /// Queue the confirmed pause-menu command for the next fixed tick.
    pub fn request_respawn(&mut self) {
        self.respawn_requested = true;
    }

    pub fn world_instance_poses(&self) -> Option<&std::collections::BTreeMap<u32, glam::Mat4>> {
        self.campaign
            .as_ref()
            .map(|rt| rt.instance_poses())
            .or_else(|| self.world_events.as_ref().map(|rt| rt.instance_poses()))
    }

    /// One fixed tick (also the headless/replay entry — no accumulator).
    /// `pads` is indexed by player slot; slot [`WalkSim::local_slot`] is
    /// [`WalkSim::player`].
    pub fn step_tick(&mut self, pads: &[PadState]) {
        // The people on this machine besides the first (issue #194): their reads before the
        // step, and their pending respawn presses as pad bits. None with one local player.
        let extras = self.locals_before();
        let extra_respawns: Vec<bool> = extras.iter().map(|e| e.1).collect();
        let with_respawns;
        // In a LAN round a further local's respawn is in the pad its machine
        // submitted (`walk::locals` module header): one machine's own press
        // here would part its sim from the others'.
        let pads = if extra_respawns.contains(&true) && !self.netplay.active {
            with_respawns = self.pads_with_local_respawns(pads, &extra_respawns);
            &with_respawns[..]
        } else {
            pads
        };
        let old_mount = self.sim.unit(self.player).and_then(|u| u.mount);
        let old_camera = self.camera();
        let kills_before = self.sim.units[self.player.index as usize].kills;
        // A death outside the tick (a test, a menu) left its kill in the
        // events this step clears.
        let killer_before = self.killer_of(self.player);
        if std::mem::take(&mut self.respawn_requested) {
            let mut requested = pads.to_vec();
            let local = usize::from(self.local_slot);
            if requested.len() <= local {
                requested.resize(local + 1, PadState::default());
            }
            requested[local].buttons |= core::sim::buttons::RESPAWN;
            self.sim.step(&requested);
        } else {
            self.sim.step(pads);
        }
        // A respawn gives the player's slot a new generation
        // (`Sim::respawn_slot`): follow it, or every `sim.unit(self.player)`
        // reads None from the first death on (zoom, sprint latch, scope body,
        // music, commander, black box).
        if let Some(Some(h)) = self.sim.players.get(usize::from(self.local_slot)) {
            self.player = *h;
        }
        self.refill_after_side_change();
        if self.sim.outcome.is_none() {
            campaign::step(self);
            if let Some(events) = &mut self.world_events {
                events.step(&mut self.sim);
            }
        }
        self.track_class_kills(kills_before);
        self.step_view(pads, old_mount, &old_camera, killer_before);
        for (k, (before, _)) in extras.into_iter().enumerate() {
            // An adopted LAN local (B18) the bundles have not seated yet has no
            // view to step; on the tick they seat it, its `before` was read from
            // another unit, so its class kills start from the next tick.
            let was_seated = self.locals[k].seated;
            self.with_local(k + 1, |w| {
                // A respawn gives the slot a new generation: follow it.
                if let Some(Some(h)) = w.sim.players.get(usize::from(w.local_slot)) {
                    w.player = *h;
                }
            });
            if !was_seated {
                let held = self
                    .sim
                    .players
                    .get(usize::from(self.locals[k].slot))
                    .is_some_and(Option::is_some);
                self.locals[k].seated = held;
                continue;
            }
            self.with_local(k + 1, |w| {
                w.track_class_kills(before.kills);
                w.step_view(pads, before.mount, &before.camera, before.killer);
            });
        }
        // The squad's answers to this tick's orders join the voice queue;
        // the units' combat calls wait for the sound bank's gates
        // (`Commander::gate_calls`).
        // Split: the voice follows the first local player who is not away (issue #194 B12);
        // the walk's own player with one local player. Its per-player lines (orders, leaving,
        // cockpit, loudspeakers) are that player's alone, and nobody's while everyone is away.
        let heard = self.first_awake_unit();
        if let Some(v) = self.voice.as_mut() {
            if let Some(heard) = heard {
                v.orders(&self.sim, heard);
                v.leaving(&self.sim, heard);
            }
            v.calls(&self.sim, &self.teams);
            if let Some(heard) = heard {
                v.cockpit(&self.sim, heard, &self.vehicle_sounds);
                v.loudspeakers(&self.sim, heard, &self.sound_triggers, &self.teams);
            }
        }
    }

    /// The per-player part of a tick, after the sim has stepped: the spawn backdrop, the death
    /// and spectator cameras, the zoom and every camera state, the seat view. It reads and
    /// writes the walk's own view fields, so for a further local player it runs inside
    /// [`WalkSim::with_local`].
    fn step_view(
        &mut self,
        pads: &[PadState],
        old_mount: Option<core::vehicle::Mount>,
        old_camera: &Camera,
        killer_before: Option<UnitHandle>,
    ) {
        self.follow_backdrop();
        self.step_death_cam();
        self.notice_death(old_camera, killer_before);
        self.step_spectate(pads.get(usize::from(self.local_slot)));
        // The camera's zoom follows the player's (`re-zoom.md` §5): S and
        // the FOV ease toward their targets, and rest exactly once there.
        // A building turret's zoom is the seat's (card VTURZ); the same ease
        // (PROVISIONAL for a turret: its projection is not read).
        let zoom = self
            .sim
            .unit(self.player)
            .and_then(|u| u.zoom)
            .or_else(|| self.sim.seat_zoom(self.player));
        let k = core::fmath::exp(-ZOOM_EASE_RATE * core::TICK_DT);
        let ease = |v: f32, to: f32| {
            let v = to + (v - to) * k;
            if (v - to).abs() < VIEW_SETTLE_EPSILON {
                to
            } else {
                v
            }
        };
        self.zoom_s = ease(self.zoom_s, zoom.map_or(1.0, |z| z.value));
        let first = zoom.is_some() || self.first_person();
        self.zoom_fov_mix = ease(self.zoom_fov_mix, if first { 1.0 } else { 0.0 });
        self.cam_place = self.next_cam_place();
        self.cam_jet_lag = self.next_cam_jet_lag();
        self.step_cam_pull();
        self.cam_flyer = self.next_cam_flyer(old_camera.position);
        self.cam_gunner = self.next_cam_gunner(old_camera);
        self.cam_exit = self.next_cam_exit(old_mount.is_some(), old_camera);
        self.cam_guided = self.next_cam_guided(old_camera.position);
        let owner = self.cam_owner_now();
        let handed = owner.is_some() && self.cam_owner.is_some() && owner != self.cam_owner;
        self.cam_handover = self.next_cam_handover(handed, old_camera);
        self.cam_owner = owner;
        self.feed_handover();
        let me = self.sim.units.get(self.player.index as usize);
        if me.is_some_and(|u| u.alive && u.mount.is_some()) {
            self.seat_view_timer = camera::DEATH_VIEW_HOLD;
        } else if self.seat_view_timer > 0.0 {
            self.seat_view_timer = (self.seat_view_timer - core::TICK_DT).fmax(0.0);
        }
        self.seat_view = self.next_seat_view();
    }

    fn track_class_kills(&mut self, kills_before: u32) {
        let Some(me) = self.sim.units.get(self.player.index as usize).filter(|u| {
            u.gen == self.player.gen
                && u.spec.player_slot == Some(self.local_slot)
                && u.kills > kills_before
        }) else {
            return;
        };
        let Some(class) = self
            .teams
            .get(usize::from(me.team))
            .and_then(|team| team.classes.get(usize::from(me.spec.class)))
        else {
            return;
        };
        let count = self
            .pending_class_kills
            .entry(class.name.clone())
            .or_default();
        *count = count.saturating_add(me.kills - kills_before);
    }

    pub fn flush_class_kills(&mut self, profile: &mut crate::profile::Profile) -> bool {
        let changed = !self.pending_class_kills.is_empty();
        for (class, kills) in std::mem::take(&mut self.pending_class_kills) {
            let count = profile.career.class_kills.entry(class).or_default();
            *count = count.saturating_add(kills);
        }
        changed
    }

    /// The local player's career medal counts: player slot
    /// [`WalkSim::local_slot`]'s, when it has a career (a LAN guest's own,
    /// never the host's slot 0; none for a joiner not yet seated).
    pub fn local_medal_counts(&self) -> Option<core::sim::MedalCounts> {
        self.sim.slot_medal_counts(usize::from(self.local_slot))
    }

    pub fn flush_career_progress(&mut self, profile: &mut crate::profile::Profile) -> bool {
        let mut changed = self.flush_class_kills(profile);
        if let Some(counts) = self.local_medal_counts() {
            for (saved, count) in profile.career.medals.iter_mut().zip(counts) {
                // A LAN replay or an old lobby snapshot may have started
                // with fewer medals than this player has already saved.
                if count > *saved || (!self.netplay.active && count != *saved) {
                    *saved = count;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn record_battle(
        &mut self,
        profile: &mut crate::profile::Profile,
        stats: crate::profile::BattleStats,
    ) {
        self.flush_career_progress(profile);
        // Issue #194: a split player away at the end has no unit of theirs (a bot may hold the
        // index), and one who came back has a fresh unit: their banked counters and side count.
        let carry = self.round_carry;
        let vacant = self
            .sim
            .players
            .get(usize::from(self.local_slot))
            .is_none_or(|p| p.is_none());
        let (kills, deaths, team) = match carry.team {
            Some(team) if vacant => (0, 0, team),
            _ => {
                let me = &self.sim.units[self.player.index as usize];
                (me.kills, me.deaths, me.team)
            }
        };
        let won = self
            .sim
            .outcome
            .map(|winner| winner == team || winner == core::sim::OUTCOME_DRAW);
        profile.record_round(
            won,
            carry.kills.saturating_add(kills),
            carry.deaths.saturating_add(deaths),
            stats.shots,
            stats.hits,
            stats.captures,
        );
        profile.career.points += i64::from(stats.points);
    }

    /// Placeholder soldier visual: a stance-height box standing on the ground
    /// at the soldier's position, facing their yaw.
    pub fn marker_transform(&self) -> Mat4 {
        let s = self.soldier();
        let h = capsule_height(s.stance);
        Mat4::from_translation(s.pos + Vec3::Y * (h * 0.5))
            * Mat4::from_rotation_y(s.yaw)
            * Mat4::from_scale(Vec3::new(0.6, h, 0.6))
    }

    /// Alive non-player units on foot as stance-height boxes.
    pub fn target_draws(&self) -> Vec<TargetDraw> {
        self.sim
            .alive_units()
            .filter(|(h, u)| *h != self.player && u.mount.is_none())
            .map(|(h, u)| {
                let hgt = capsule_height(u.soldier.stance);
                TargetDraw {
                    transform: Mat4::from_translation(u.soldier.pos + Vec3::Y * (hgt * 0.5))
                        * Mat4::from_rotation_y(u.soldier.yaw)
                        * Mat4::from_scale(Vec3::new(0.7, hgt, 0.7)),
                    magnet: self.magnet() == Some(h),
                    health_frac: (u.health.cur / u.health.max).clamp(0.0, 1.0),
                    team: u.team,
                    droideka: u.droideka.is_some(),
                }
            })
            .collect()
    }

    /// In-flight bolts of a class with nothing authored to draw (no laser
    /// look, no model, no trail) as thin placeholder boxes along their
    /// velocity. A laser class's bolts are drawn as the PS2's ribbons by
    /// [`crate::bolt_vis`], a modelled round by its model. Returns
    /// (transform, team).
    pub fn bolt_draws(&self) -> Vec<(Mat4, u8)> {
        const LEN: f32 = 1.2;
        const THICK: f32 = 0.08;
        self.sim
            .bolts
            .iter()
            .filter(|b| {
                self.sim.weapon_classes.get(b.weapon_class).is_none_or(|w| {
                    w.ordnance.laser.is_none()
                        && w.ordnance.geometry.is_none()
                        && w.ordnance.impact.as_ref().is_none_or(|l| l.trail.is_none())
                })
            })
            .map(|b| {
                let dir = b.vel.normalize_or_zero();
                let rot = Quat::from_rotation_arc(Vec3::Z, dir);
                (
                    Mat4::from_translation(b.pos - dir * (LEN * 0.5))
                        * Mat4::from_quat(rot)
                        * Mat4::from_scale(Vec3::new(THICK, THICK, LEN)),
                    b.team,
                )
            })
            .collect()
    }
}

mod camera;
mod death_camera;
mod flyer_camera;
mod locals;
mod spectate;
pub use camera::{CamOwner, Handover, DEATH_VIEW_HOLD};
pub use death_camera::DeathCam;
pub use flyer_camera::{
    guided_section, FlyerCam, FlyerSection, GuidedCam, GunnerCam, FLYER_CRASH_DETACH_RATE,
};
pub use locals::{LocalPlayer, LocalView, Suspended};
pub use spectate::{Spectate, SPECTATE_HOLD_TICKS};
pub use walker_view::walker_section;

#[cfg(test)]
mod tests;
