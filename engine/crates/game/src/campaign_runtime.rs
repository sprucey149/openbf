//! Rise of the Empire's mission logic in the battle: the translated mission
//! script and the shared objective framework ([`crate::campaign_script`])
//! run against the sim through the engine bindings
//! (`crate::campaign_bindings`).
//!
//! Order (BF2's mission flow, `analysis/a6-mission-lua.md`): the chunk runs
//! first (it loads the framework with `ScriptCB_DoFile` and defines the
//! mission's functions), then `ScriptInit`, then `ScriptPostLoad`. The
//! loader already applied `ScriptInit`'s setup from the same bytecode
//! (`gen_missions.campaign_facts50`: teams, classes, layers, sounds), so it
//! runs here with every engine binding a no-op: only the globals it assigns
//! count. Every tick the sim's events become the script's callbacks (OnX
//! handlers, timers), and what the bindings asked for (popups, messages,
//! voice-overs, markers, the objective list, victory) is read off by the HUD.
//!
//! Ownership: this module owns `World`'s state, its queries and the
//! script-visible writers in `world_ops`; `campaign_bindings` owns only the
//! Host/Ctx dispatch that calls them.

use crate::campaign_script::{Interp, Ud, Value};
use crate::layout::MapLayout;
use crate::mission::CampaignMissionDef;
use core::fmath::FloatExt;
use core::region::Region;
use core::sim::{Event, Sim};
use core::unit::UnitHandle;
use glam::Vec3;
use manifest::Manifest;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

mod world_events;
pub use world_events::WorldEventRuntime;

/// A scripted callback's event (`On<Kind>` registers, `Release<Kind>` drops).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    CharacterDeath,
    CharacterSpawn,
    CharacterChangeClass,
    CharacterEnterVehicle,
    CharacterExitVehicle,
    CharacterLandedFlyer,
    ObjectKill,
    ObjectRepair,
    ObjectRespawn,
    ObjectInit,
    ObjectDamage,
    EnterRegion,
    LeaveRegion,
    FinishCapture,
    FinishNeutralize,
    CommandPostKill,
    CommandPostRespawn,
    FlagPickUp,
    FlagDrop,
    FlagReset,
    TeamPointsChange,
    TicketCountChange,
    TimerElapse,
}

impl EventKind {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "CharacterDeath" => Self::CharacterDeath,
            "CharacterSpawn" => Self::CharacterSpawn,
            "CharacterChangeClass" => Self::CharacterChangeClass,
            "CharacterEnterVehicle" => Self::CharacterEnterVehicle,
            "CharacterExitVehicle" => Self::CharacterExitVehicle,
            "CharacterLandedFlyer" => Self::CharacterLandedFlyer,
            "ObjectKill" => Self::ObjectKill,
            "ObjectRepair" => Self::ObjectRepair,
            "ObjectRespawn" => Self::ObjectRespawn,
            "ObjectInit" => Self::ObjectInit,
            "ObjectDamage" => Self::ObjectDamage,
            "EnterRegion" => Self::EnterRegion,
            "LeaveRegion" => Self::LeaveRegion,
            "FinishCapture" => Self::FinishCapture,
            "FinishNeutralize" => Self::FinishNeutralize,
            "CommandPostKill" => Self::CommandPostKill,
            "CommandPostRespawn" => Self::CommandPostRespawn,
            "FlagPickUp" => Self::FlagPickUp,
            "FlagDrop" => Self::FlagDrop,
            "FlagReset" => Self::FlagReset,
            "TeamPointsChange" => Self::TeamPointsChange,
            "TicketCountChange" => Self::TicketCountChange,
            "TimerElapse" => Self::TimerElapse,
            _ => return None,
        })
    }
}

/// What a handler's second argument narrows it to: `OnXName(fn, name)`,
/// `OnXClass(fn, class)`, `OnXTeam(fn, team)`, `OnEnterRegion(fn, region)`,
/// `OnTimerElapse(fn, timer)`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Filter {
    Any,
    Name(String),
    Class(String),
    Team(u8),
    Region(u32),
    Timer(u32),
}

#[derive(Clone, Debug)]
pub(crate) struct Handler {
    pub kind: EventKind,
    pub filter: Filter,
    pub func: Value,
    pub released: bool,
}

/// The facts a filter is matched against.
#[derive(Clone, Debug, Default)]
pub(crate) struct EventKey {
    pub name: Option<String>,
    pub class: Option<String>,
    pub team: Option<u8>,
    pub region: Option<u32>,
    pub timer: Option<u32>,
}

impl Filter {
    fn matches(&self, k: &EventKey) -> bool {
        match self {
            Filter::Any => true,
            Filter::Name(n) => k.name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)),
            Filter::Class(c) => k
                .class
                .as_deref()
                .is_some_and(|x| x.eq_ignore_ascii_case(c)),
            Filter::Team(t) => k.team == Some(*t),
            Filter::Region(r) => k.region == Some(*r),
            Filter::Timer(t) => k.timer == Some(*t),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Timer {
    pub name: String,
    pub value: f32,
    pub rate: f32,
    pub running: bool,
    pub alive: bool,
}

/// A map/HUD marker (`MapAddEntityMarker` / `MapAddClassMarker` /
/// `MapAddRegionMarker`): drawn for `team` (BF2 numbering).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MarkTarget {
    Obj(Ud),
    Class(String),
    Region(u32),
}

#[derive(Clone, Debug)]
pub(crate) struct Marker {
    pub target: MarkTarget,
    pub icon: String,
    pub scale: f32,
    pub team: u8,
    /// The marker's sixth argument: an entity's or a class's world hint
    /// (`FUN_001ad960` → `FUN_0017cc00`), drawn pulsing.
    pub pulse: bool,
}

/// An objective of the mission's list (`AddMissionObjective`); state 1
/// added, 2 active, 3 complete (`FUN_0038abd8`'s argument).
#[derive(Clone, Debug)]
pub struct ObjectiveLine {
    pub team: u8,
    pub text: String,
    pub popup: Option<String>,
    pub state: u8,
}

/// A popup the bindings opened (`FUN_0038c7a0`'s kinds: 1 hint, 2
/// objective, 3 selection). The game pauses while one is up.
#[derive(Clone, Debug, PartialEq)]
pub enum Popup {
    Hint(String),
    Objective(String),
    Selection(String),
}

/// An objective marker in the world, for the HUD and the minimap.
#[derive(Clone, Debug, PartialEq)]
pub struct CampaignMarker {
    pub pos: Vec3,
    pub icon: String,
    pub scale: f32,
    pub pulse: bool,
}

/// A flag the script created (`CreateEntity` of a `flag` class).
#[derive(Clone, Debug)]
pub(crate) struct FlagEnt {
    pub name: String,
    pub class: String,
    pub ctf: usize,
    pub dead: bool,
}

/// A script's team / class reassignment of a native member before spawn.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScriptedChar {
    pub team: Option<u8>,
    pub class: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CharSeen {
    gen: u32,
    alive: bool,
    class: u8,
    mount: Option<core::vehicle::VehicleHandle>,
    landed: bool,
}

/// Everything the bindings read and write besides the sim.
#[derive(Clone, Debug)]
pub(crate) struct World {
    pub map: String,
    pub regions: Vec<(String, Region)>,
    pub paths: Vec<(String, Vec<(Vec3, f32)>)>,
    pub instances: Vec<(String, String, Vec3)>,
    pub hidden_instances: BTreeSet<u32>,
    pub disabled_prop_collision: BTreeMap<u32, Vec<(usize, core::collision::Layers)>>,
    pub class_bases: BTreeMap<String, String>,
    pub station_names: Vec<String>,
    pub station_classes: Vec<String>,
    pub door_names: Vec<String>,
    pub post_keys: Vec<String>,
    pub saved_cp_owners: Vec<Option<u8>>,
    pub saved_cp_captures: Vec<Region>,
    pub team_classes: [Vec<String>; 2],
    pub team_units: [u32; 2],
    /// Each team's per-faction value row ([`crate::walk::faction_row`]): which
    /// team a script's `SetProperty(post, "Value_ATK_<faction>", v)` writes,
    /// the same collapse `WalkSim::add_posts` makes for the posts' authored
    /// rows.
    pub team_value_rows: [usize; 2],
    pub native_classes: BTreeMap<u8, Vec<String>>,
    pub native_units: BTreeMap<u8, u32>,
    pub vehicle_classes: Vec<String>,
    /// BF2 class indices per army: the order `AddUnitClass` added them
    /// (what `GetCharacterClass` returns), as sim class indices.
    pub class_order: [Vec<usize>; 2],
    pub class_names: Vec<String>,

    pub setup: bool,
    pub post_loaded: bool,
    pub player: UnitHandle,
    pub player_team: u8,
    pub time: f32,
    pub handlers: Vec<Handler>,
    pub timers: Vec<Timer>,
    pub markers: Vec<Marker>,
    pub active_regions: BTreeSet<u32>,
    pub inside: BTreeSet<(u32, u32)>,
    seen: Vec<Option<CharSeen>>,
    pub char_alias: BTreeMap<u32, u32>,
    /// Each native team's members in `GetTeamMember` order, fixed once
    /// asked: the PS2's team holds one character per unit for the whole
    /// battle, and `SpawnCharacter` brings that character back.
    pub native_members: BTreeMap<u8, Vec<u32>>,
    /// A native member the script reassigns before `SpawnCharacter`: BF2's
    /// `BatchChangeTeams` / `SelectCharacterClass` (the Utapau campaign turns
    /// team 4's member into the CIS hero Grievous for Objective 3b). Keyed by
    /// the member's placeholder character id.
    pub scripted_chars: BTreeMap<u32, ScriptedChar>,
    last_shot: Vec<Option<u32>>,
    pub last_hit_weapon: Vec<(Ud, String)>,
    pub tickets_seen: [i64; 2],
    pub team_points: [i32; 2],
    points_seen: [i32; 2],
    pub show_points: [bool; 2],
    pub objectives: Vec<ObjectiveLine>,
    pub hints: Vec<String>,
    pub flags: Vec<FlagEnt>,
    pub entities: Vec<(String, String, Vec3)>,
    pub allow_ai_pickup: bool,
    pub hide_cps: bool,
    pub shown_timer: Option<u32>,
    pub voice_playing: bool,
    /// The script's stream slots: each stream's name hash to the hash of the
    /// stream its slot was opened with. `OpenAudioStream` opens a slot and
    /// `AudioStreamAppendSegments` adds a stream's segments to one, so a
    /// property of an appended stream plays and queues on that slot
    /// (B2 `FUN_005519c8` finds the slot holding the property's stream,
    /// re-announcer.md §1 steps 2 and 7).
    pub stream_slots: BTreeMap<u32, u32>,
    /// `ScriptCB_EnableCommandPostVO`'s flag (`DAT_005fed28`, default 1):
    /// the binding `FUN_001a0580` stores `(int)n == 1`, and the post line
    /// `FUN_003570b8` says nothing while it is clear; the post's sound and
    /// music still play (re-announcer.md §2.1).
    pub command_post_vo: bool,

    pub messages: Vec<String>,
    pub popups: VecDeque<Popup>,
    pub voice: Vec<String>,
    pub music: Vec<crate::music::InGameMusic>,
    pub sounds: Vec<String>,
    pub movies: Vec<String>,
    /// `SetMissionEndMovie`'s clip (`FUN_00192a50` → `FUN_00381598`, the
    /// round-end object's +0x300).
    pub end_movie: Option<String>,
    pub hud_events: Vec<(f32, &'static str)>,
    /// `player1.objectivesUpdated` is up: the last of its posts to fire.
    pub updated_on: bool,
    pub reported: BTreeSet<String>,
    /// True when a campaign script changed CP ownership, spawn paths, unit
    /// counts, or class availability and main-team AI should be topped up once
    /// the current tick's callbacks finish (so consecutive `OnComplete` +
    /// `OnStart` CP changes split bots across all newly opened posts).
    pub pending_ai_fill: bool,
    /// True once the human player has spawned at least once (so `onfirstspawn`
    /// runs before the campaign's opening AI wave spawns).
    pub human_spawned: bool,
    pub anims: crate::world_anim::WorldAnimPlayer,
}

#[derive(Clone, Debug)]
pub struct CampaignRuntime {
    interp: Interp,
    pub(crate) w: World,
    started: bool,
}

/// The bindings' view of one call: the world and the sim.
pub(crate) struct Ctx<'a> {
    pub w: &'a mut World,
    pub sim: &'a mut Sim,
}

impl CampaignRuntime {
    /// Mark CPs named in `killed` as disabled before the opening wave spawns,
    /// returning each post's pre-kill owner so `RespawnObject` can restore it.
    pub fn kill_startup_posts(sim: &mut Sim, killed: &[String]) -> Vec<Option<u8>> {
        let mut saved = Vec::with_capacity(sim.cps.len());
        for cp in &mut sim.cps {
            saved.push(cp.owner);
            if killed.iter().any(|k| k.eq_ignore_ascii_case(&cp.def.name)) {
                cp.killed = true;
                cp.owner = None;
                cp.capture = 0.0;
                cp.neutral = 0.0;
            }
        }
        saved
    }

    pub fn new(
        spec: CampaignMissionDef,
        layout: &MapLayout,
        world: &Manifest,
        post_keys: &[String],
        saved_cp_owners: Vec<Option<u8>>,
        sim: &Sim,
    ) -> Self {
        let script = Arc::new(
            spec.script
                .clone()
                .unwrap_or(crate::campaign_script::ScriptDef {
                    main: 0,
                    fns: vec![crate::campaign_script::FnDef {
                        name: "main".into(),
                        params: 0,
                        vararg: false,
                        locals: 0,
                        body: Vec::new(),
                    }],
                }),
        );
        let interp = Interp::new(script, &spec.libs);
        let regions = layout
            .regions
            .iter()
            .map(|(name, r)| (name.to_ascii_lowercase(), *r))
            .collect();
        let paths = layout
            .paths
            .iter()
            .map(|p| {
                (
                    p.name.to_ascii_lowercase(),
                    p.nodes.iter().map(|n| (n.pos, n.yaw)).collect(),
                )
            })
            .collect();
        let instances: Vec<(String, String, Vec3)> = world
            .instances
            .iter()
            .map(|i| {
                (
                    i.name.to_ascii_lowercase(),
                    i.class.to_ascii_lowercase(),
                    Vec3::from(i.translation),
                )
            })
            .collect();
        let class_bases = world
            .classes
            .iter()
            .map(|c| (c.name.to_ascii_lowercase(), c.base.to_ascii_lowercase()))
            .collect();
        let inst_name = |i: Option<u32>| -> String {
            i.and_then(|i| instances.get(i as usize))
                .map(|x| x.0.clone())
                .unwrap_or_default()
        };
        let station_names = sim.stations.iter().map(|s| inst_name(s.instance)).collect();
        let station_classes = sim
            .stations
            .iter()
            .map(|s| {
                sim.station_classes
                    .get(usize::from(s.class))
                    .map(|c| c.name.to_ascii_lowercase())
                    .unwrap_or_default()
            })
            .collect();
        let door_names = sim
            .doors
            .iter()
            .map(|d| inst_name(Some(d.instance as u32)))
            .collect();
        let saved_cp_captures = sim.cps.iter().map(|cp| cp.def.capture).collect();
        let map = world.map.trim_start_matches("bf2_").to_owned();
        let w = World {
            map,
            regions,
            paths,
            instances,
            hidden_instances: BTreeSet::new(),
            disabled_prop_collision: BTreeMap::new(),
            class_bases,
            station_names,
            station_classes,
            door_names,
            post_keys: post_keys.to_vec(),
            saved_cp_owners,
            saved_cp_captures,
            team_classes: [Vec::new(), Vec::new()],
            team_units: [0, 0],
            team_value_rows: [
                crate::walk::faction_row("all"),
                crate::walk::faction_row("imp"),
            ],
            native_classes: BTreeMap::new(),
            native_units: BTreeMap::new(),
            vehicle_classes: Vec::new(),
            class_order: [Vec::new(), Vec::new()],
            class_names: Vec::new(),
            setup: false,
            post_loaded: false,
            player: UnitHandle { index: 0, gen: 0 },
            player_team: 0,
            time: 0.0,
            handlers: Vec::new(),
            timers: Vec::new(),
            markers: Vec::new(),
            active_regions: BTreeSet::new(),
            inside: BTreeSet::new(),
            seen: Vec::new(),
            char_alias: BTreeMap::new(),
            native_members: BTreeMap::new(),
            scripted_chars: BTreeMap::new(),
            last_shot: Vec::new(),
            last_hit_weapon: Vec::new(),
            tickets_seen: [i64::MIN; 2],
            team_points: [0; 2],
            points_seen: [0; 2],
            show_points: [false; 2],
            objectives: Vec::new(),
            hints: Vec::new(),
            flags: Vec::new(),
            entities: Vec::new(),
            allow_ai_pickup: true,
            hide_cps: false,
            shown_timer: None,
            voice_playing: false,
            stream_slots: BTreeMap::new(),
            command_post_vo: true,
            messages: Vec::new(),
            popups: VecDeque::new(),
            voice: Vec::new(),
            music: Vec::new(),
            sounds: Vec::new(),
            movies: Vec::new(),
            end_movie: None,
            hud_events: Vec::new(),
            updated_on: false,
            reported: BTreeSet::new(),
            pending_ai_fill: false,
            human_spawned: false,
            anims: Default::default(),
        };
        Self {
            interp,
            w,
            started: false,
        }
    }

    pub fn set_team_rosters(&mut self, team_classes: [Vec<String>; 2], team_units: [u32; 2]) {
        self.w.team_classes = team_classes;
        self.w.team_units = team_units;
    }

    /// The two main teams' faction keys (`all`, `imp`, ...), before `start`:
    /// they route the script's per-faction post values to teams.
    pub fn set_team_factions(&mut self, factions: [&str; 2]) {
        self.w.team_value_rows = factions.map(crate::walk::faction_row);
    }

    /// Ask the runtime to spawn the main teams' opening AI wave on the first
    /// post-spawn `step` (after `onfirstspawn` updates initial CP ownership).
    pub fn defer_opening_wave(&mut self) {
        self.w.pending_ai_fill = true;
    }

    /// The local teams' rosters (team, member classes, unit count): who
    /// `GetTeamSize` / `GetTeamMember` count for the Ambush teams.
    pub fn set_native_rosters(&mut self, rosters: BTreeMap<u8, (Vec<String>, u32)>) {
        for (team, (classes, units)) in rosters {
            self.w.native_classes.insert(team, classes);
            self.w.native_units.insert(team, units);
        }
    }

    pub fn set_vehicle_classes(&mut self, classes: &[String]) {
        self.w.vehicle_classes = classes.iter().map(|c| c.to_ascii_lowercase()).collect();
    }

    /// Run the chunk, `ScriptInit` (setup already applied: bindings off) and
    /// `ScriptPostLoad`.
    pub fn start(&mut self, sim: &mut Sim) {
        if self.started {
            return;
        }
        self.started = true;
        // A flag-class object placed in the map (spa3's `plans` holocron) is a
        // real flag in the original's object registry: a soldier picks it up
        // on contact. The tutorial's holocron is made with `CreateEntity`,
        // which registers a flag; a placed one never was, leaving it a solid
        // prop. Register every placed flag-class instance the same way before
        // the script runs, so `GetFlagCarrier` / `SetProperty` and the pickup
        // itself work. `pose_ctf` fades the static draw for these instances.
        let placed: Vec<(String, String, Vec3)> = self.w.instances.clone();
        for (name, class, pos) in placed {
            if self.w.class_bases.get(&class).map(String::as_str) != Some("flag") {
                continue;
            }
            if self
                .w
                .flags
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let far = Vec3::new(0.0, -10_000.0, 0.0);
            let ctf = sim.ctf.get_or_insert_with(|| {
                let mut c = core::sim::ctf::CtfState::new_ground_one_flag(u32::MAX, pos, far, far);
                c.flags.clear();
                for r in &mut c.capture_regions {
                    r.radius = 0.0;
                }
                c
            });
            let mut flag = core::sim::ctf::Flag::new_ground_neutral(pos);
            // Match retail default (AllowAIPickUp 1) while disabling flyer
            // pickup so spa3 fighters never chase or scoop placed holocrons.
            flag.allow_ai_pickup = true;
            flag.flyer_pickup_radius = 0.0;
            ctf.flags.push(flag);
            let ci = ctf.flags.len() - 1;
            self.w.flags.push(FlagEnt {
                name,
                class,
                ctf: ci,
                dead: false,
            });
        }
        // The hero is the army's `SetHeroClass` slot, not an `AddUnitClass`
        // entry; the port keeps him in the class table (`heroes::append`),
        // so he must not take a place in the script's numbering: geo1c's
        // Objective6a waits for the engineer as class 1.
        for t in 0..2 {
            let hero = sim.hero_class(t as u8).map(usize::from);
            self.w.class_order[t] = (0..sim.classes[t].len())
                .filter(|&ci| Some(ci) != hero && sim.is_class_enabled(t as u8, ci))
                .collect();
        }
        self.w.tickets_seen = [0, 1].map(|t| reinforcement_count(sim, t));
        {
            let mut ctx = Ctx {
                w: &mut self.w,
                sim,
            };
            if let Err(e) = self.interp.run_main(&mut ctx) {
                ctx.w.report(&e.0);
            }
        }
        self.w.setup = true;
        self.call_global(sim, "ScriptInit", Vec::new());
        self.w.setup = false;
        self.call_global(sim, "ScriptPostLoad", Vec::new());
        self.w.post_loaded = true;
        sim.populate_native_teams();
        if self.w.pending_ai_fill && !sim.spawn_screens {
            self.w.pending_ai_fill = false;
            self.w.fill_enabled_ai(sim);
        }
    }

    fn call_global(&mut self, sim: &mut Sim, name: &str, args: Vec<Value>) {
        let f = self.interp.global(name);
        if f == Value::Nil {
            return;
        }
        let mut ctx = Ctx {
            w: &mut self.w,
            sim,
        };
        if let Err(e) = self.interp.call_value(&mut ctx, &f, args) {
            ctx.w.report(&e.0);
        }
    }

    pub fn step(
        &mut self,
        sim: &mut Sim,
        player: UnitHandle,
        _player_class_name: Option<&str>,
        team_classes: &[Vec<String>; 2],
        native_classes: &BTreeMap<u8, Vec<String>>,
        vehicle_classes: &[String],
        dt: f32,
    ) {
        if self.w.team_classes != *team_classes {
            self.w.team_classes = team_classes.clone();
        }
        for (team, classes) in native_classes {
            self.w.native_classes.insert(*team, classes.clone());
        }
        if self.w.vehicle_classes.len() != vehicle_classes.len() {
            self.set_vehicle_classes(vehicle_classes);
        }
        self.w.player = player;
        if let Some(u) = sim.unit(player) {
            self.w.player_team = u.team;
        }
        if sim.outcome.is_some() {
            return;
        }
        self.w.time += dt;
        for e in &mut self.w.hud_events {
            e.0 -= dt;
        }
        self.w.hud_events.sort_by(|a, b| a.0.total_cmp(&b.0));
        while self.w.hud_events.first().is_some_and(|e| e.0 <= 0.0) {
            let (_, e) = self.w.hud_events.remove(0);
            self.w.updated_on = e == "objectivesUpdated";
        }
        self.dispatch_sim_events(sim);
        self.dispatch_state_changes(sim);
        self.step_timers(sim, dt);
        if self.w.pending_ai_fill && (!sim.spawn_screens || self.w.human_spawned) {
            self.w.pending_ai_fill = false;
            self.w.fill_enabled_ai(sim);
        }
        self.w.anims.step(sim, dt);
    }

    /// The map's world animations (`crate::world_anim`), before `start` runs
    /// the script's setup. A layer still in `manifest.layers` was not
    /// merged, so its worlds' groups are left out.
    pub fn load_world_anims(
        &mut self,
        dir: &std::path::Path,
        manifest: &crate::assets::Manifest,
        sim: &Sim,
    ) {
        let unloaded: Vec<String> = manifest
            .layers
            .iter()
            .flat_map(|l| l.worlds.iter().chain(std::iter::once(&l.name)).cloned())
            .collect();
        let instance_names: Vec<String> = manifest
            .instances
            .iter()
            .map(|i| i.name.to_ascii_lowercase())
            .collect();
        let instance_mats: Vec<glam::Mat4> = manifest
            .instances
            .iter()
            .map(|i| crate::viewer::instance_matrix(i.rotation, i.translation))
            .collect();
        self.w.anims = crate::world_anim::WorldAnimPlayer::load(
            dir,
            &unloaded,
            &self.w.station_names,
            &instance_names,
            &instance_mats,
            sim,
        );
    }

    fn has_handlers(&self, kind: EventKind) -> bool {
        self.w
            .handlers
            .iter()
            .any(|h| !h.released && h.kind == kind)
    }

    // ---- what the HUD reads

    /// Posed world instances from the mission's world animations, for the
    /// viewer to apply to the map's instance draws (the library doors).
    pub fn instance_poses(&self) -> &std::collections::BTreeMap<u32, glam::Mat4> {
        self.w.anims.instance_poses()
    }

    pub fn hidden_instances(&self) -> &BTreeSet<u32> {
        &self.w.hidden_instances
    }

    pub fn excluded_camera_instances(&self) -> impl Iterator<Item = u32> + '_ {
        self.w
            .hidden_instances
            .iter()
            .chain(self.w.disabled_prop_collision.keys())
            .copied()
    }

    /// Manifest instance indices driven by loaded non-local world animations.
    pub fn animated_instances(&self) -> impl Iterator<Item = u32> + '_ {
        self.w.anims.animated_instances()
    }

    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.w.messages)
    }

    pub fn take_movies(&mut self) -> Vec<String> {
        std::mem::take(&mut self.w.movies)
    }

    pub fn has_movies(&self) -> bool {
        !self.w.movies.is_empty()
    }

    pub fn end_movie(&self) -> Option<&str> {
        self.w.end_movie.as_deref()
    }

    pub fn take_voice_overs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.w.voice)
    }

    /// `ScriptCB_SndPlaySound` names since the last call, in order.
    pub fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.w.sounds)
    }

    /// The in-game music the script started or stopped.
    pub fn take_music(&mut self) -> Vec<crate::music::InGameMusic> {
        std::mem::take(&mut self.w.music)
    }

    /// Whether the battle's voice slot is sounding a line: what
    /// `AudioStreamComplete` answers from. B2 `FUN_0019f0f8` finds the
    /// stream slot holding the named stream and answers whether that slot's
    /// voice is stopped (`FUN_00542338` state 4), queued lines not counted.
    /// The port has one voice queue, so every voice stream name reads it.
    /// trace: a slot other than the slow voice slot named here.
    pub fn set_voice_playing(&mut self, playing: bool) {
        self.w.voice_playing = playing;
    }

    /// The stream slots the script opened ([`World::stream_slots`]).
    pub fn stream_slots(&self) -> &BTreeMap<u32, u32> {
        &self.w.stream_slots
    }

    /// The script lets command posts speak ([`World::command_post_vo`]).
    pub fn command_post_vo(&self) -> bool {
        self.w.command_post_vo
    }

    /// Live script-created flags and their model classes, indexed in the sim.
    pub fn flag_objects(&self) -> impl Iterator<Item = (usize, &str)> {
        self.w
            .flags
            .iter()
            .filter(|f| !f.dead)
            .map(|f| (f.ctf, f.class.as_str()))
    }

    /// Script errors so far (each once), for logs and tests.
    pub fn errors(&self) -> impl Iterator<Item = &String> {
        self.w.reported.iter()
    }
}

/// A character as the scripts see it: its index, a number (`character == 0`
/// is the local player in geo1c's first-spawn check; `GetTeamMember`
/// returns these).
pub(crate) fn char_value(i: u32) -> Value {
    Value::Num(i as f32)
}

pub(crate) fn reinforcement_count(sim: &Sim, t: usize) -> i64 {
    if sim.battle.unlimited_reinforcements || sim.battle.unlimited_teams[t] {
        -1
    } else {
        i64::from(sim.battle.teams[t].tickets())
    }
}

pub(crate) fn unit_world_pos(sim: &Sim, u: &core::unit::Unit) -> Vec3 {
    u.mount
        .and_then(|m| sim.vehicles.get(m.vehicle.index as usize))
        .map_or(u.soldier.pos, |v| v.pos)
}

impl World {
    pub(crate) fn report(&mut self, msg: &str) {
        if self.reported.insert(msg.to_owned()) {
            eprintln!("campaign script: {msg}");
        }
    }

    pub(crate) fn region_id(&self, name: &str) -> Option<u32> {
        self.regions
            .iter()
            .position(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|i| i as u32)
    }

    pub(crate) fn path(&self, name: &str) -> Option<usize> {
        self.paths
            .iter()
            .position(|(n, _)| n.eq_ignore_ascii_case(name))
    }

    pub(crate) fn class_id(&mut self, name: &str) -> u32 {
        let lower = name.to_ascii_lowercase();
        if let Some(i) = self.class_names.iter().position(|c| *c == lower) {
            return i as u32;
        }
        self.class_names.push(lower);
        self.class_names.len() as u32 - 1
    }

    fn flag_of_ctf(&self, ctf: u8) -> Option<u8> {
        self.flags
            .iter()
            .position(|f| !f.dead && f.ctf == usize::from(ctf))
            .map(|i| i as u8)
    }

    pub(crate) fn find_cp(&self, sim: &Sim, name: &str) -> Option<usize> {
        sim.cps
            .iter()
            .position(|cp| cp.def.name.eq_ignore_ascii_case(name))
            .or_else(|| {
                let lower = name.to_ascii_lowercase();
                self.post_keys.iter().position(|k| {
                    let k = k.to_ascii_lowercase();
                    k == lower || k.rsplit('.').next() == Some(lower.as_str())
                })
            })
    }

    /// The object a script names: a post, a station, a pad's vehicle, a
    /// flag, a building turret, else the placed entity.
    pub(crate) fn resolve_name(&self, sim: &Sim, name: &str) -> Option<Ud> {
        let lower = name.to_ascii_lowercase();
        if let Some(ci) = self.find_cp(sim, &lower) {
            return Some(Ud::Post(ci as u16));
        }
        if let Some(si) = self
            .station_names
            .iter()
            .position(|n| !n.is_empty() && *n == lower)
        {
            return Some(Ud::Station(si as u16));
        }
        if let Some(f) = self
            .flags
            .iter()
            .position(|f| !f.dead && f.name.eq_ignore_ascii_case(&lower))
        {
            return Some(Ud::Flag(f as u8));
        }
        if let Some(pad) = sim
            .vehicle_pads
            .iter()
            .find(|p| p.def.name.eq_ignore_ascii_case(&lower))
        {
            if let Some(v) = pad
                .vehicles
                .iter()
                .find(|pv| sim.vehicle(pv.handle).is_some_and(|v| v.alive))
            {
                return Some(Ud::Vehicle(v.handle.index, v.handle.gen));
            }
        }
        if let Some(bt) = sim.building_turrets.iter().find(|bt| {
            self.instances
                .get(bt.instance as usize)
                .is_some_and(|i| i.0 == lower)
        }) {
            return Some(Ud::Vehicle(bt.handle.index, bt.handle.gen));
        }
        self.instances
            .iter()
            .position(|i| i.0 == lower)
            .map(|i| Ud::Entity(i as u32))
    }

    pub(crate) fn resolve(&self, sim: &Sim, v: &Value) -> Option<Ud> {
        match v {
            Value::Ud(Ud::Char(c)) => self.char_unit(sim, *c).map(|h| Ud::Unit(h.index, h.gen)),
            Value::Num(c) if *c >= 0.0 => self
                .char_unit(sim, *c as u32)
                .map(|h| Ud::Unit(h.index, h.gen)),
            Value::Ud(u) => Some(*u),
            Value::Str(s) => self.resolve_name(sim, s),
            _ => None,
        }
    }

    pub(crate) fn char_slot(&self, c: u32) -> u32 {
        self.char_alias.get(&c).copied().unwrap_or(c)
    }

    pub(crate) fn char_unit(&self, sim: &Sim, c: u32) -> Option<UnitHandle> {
        let u = sim.units.get(self.char_slot(c) as usize)?;
        u.alive.then_some(UnitHandle {
            index: self.char_slot(c),
            gen: u.gen,
        })
    }

    pub(crate) fn obj_name(&self, sim: &Sim, o: Ud) -> String {
        match o {
            Ud::Post(i) => sim
                .cps
                .get(usize::from(i))
                .map(|c| c.def.name.clone())
                .unwrap_or_default(),
            Ud::Station(i) => self
                .station_names
                .get(usize::from(i))
                .cloned()
                .unwrap_or_default(),
            Ud::Flag(f) => self
                .flags
                .get(usize::from(f))
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            Ud::Entity(i) => self
                .instances
                .get(i as usize)
                .map(|x| x.0.clone())
                .or_else(|| {
                    self.entities
                        .get((i as usize).wrapping_sub(self.instances.len()))
                        .map(|e| e.0.clone())
                })
                .unwrap_or_default(),
            Ud::Vehicle(i, _) => {
                let Some(v) = sim.vehicles.get(i as usize) else {
                    return String::new();
                };
                if let Some(pad) = v.spawn_pad.and_then(|p| sim.vehicle_pads.get(p as usize)) {
                    return pad.def.name.clone();
                }
                sim.building_turrets
                    .iter()
                    .find(|bt| bt.handle.index == i)
                    .and_then(|bt| self.instances.get(bt.instance as usize))
                    .map(|x| x.0.clone())
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    pub(crate) fn obj_class(&self, sim: &Sim, o: Ud) -> Option<String> {
        match o {
            Ud::Unit(i, _) => {
                let u = sim.units.get(i as usize)?;
                self.unit_class_name(u.team, u.spec.class)
            }
            Ud::Vehicle(i, _) => {
                let v = sim.vehicles.get(i as usize)?;
                self.vehicle_classes.get(v.class as usize).cloned()
            }
            Ud::Station(i) => self.station_classes.get(usize::from(i)).cloned(),
            Ud::Entity(i) => self.instances.get(i as usize).map(|x| x.1.clone()),
            Ud::Flag(f) => self.flags.get(usize::from(f)).map(|f| f.class.clone()),
            _ => None,
        }
    }

    pub(crate) fn unit_class_name(&self, team: u8, class: u8) -> Option<String> {
        let list = if team < 2 {
            self.team_classes.get(usize::from(team))?
        } else {
            self.native_classes.get(&team)?
        };
        list.get(usize::from(class)).map(|c| c.to_ascii_lowercase())
    }

    /// BF2 team (sim team + 1), 0 for neutral.
    pub(crate) fn obj_team(&self, sim: &Sim, o: Ud) -> u8 {
        let t = match o {
            Ud::Unit(i, _) => sim.units.get(i as usize).map(|u| u.team),
            Ud::Vehicle(i, _) => sim.vehicles.get(i as usize).and_then(|v| v.team),
            Ud::Station(i) => sim.stations.get(usize::from(i)).and_then(|s| s.team),
            Ud::Post(i) => sim.cps.get(usize::from(i)).and_then(|c| c.owner),
            _ => None,
        };
        t.map_or(0, |t| t + 1)
    }

    fn obj_key(&self, sim: &Sim, o: Ud) -> EventKey {
        EventKey {
            name: Some(self.obj_name(sim, o)).filter(|n| !n.is_empty()),
            class: self.obj_class(sim, o),
            team: Some(self.obj_team(sim, o)),
            ..Default::default()
        }
    }

    pub(crate) fn obj_pos(&self, sim: &Sim, o: Ud) -> Option<Vec3> {
        match o {
            Ud::Unit(i, _) => sim.units.get(i as usize).map(|u| unit_world_pos(sim, u)),
            Ud::Vehicle(i, _) => sim.vehicles.get(i as usize).map(|v| v.pos),
            Ud::Station(i) => sim.stations.get(usize::from(i)).map(|s| s.pos),
            Ud::Post(i) => sim.cps.get(usize::from(i)).map(|c| c.def.pos),
            Ud::Entity(i) => self.instances.get(i as usize).map(|x| x.2).or_else(|| {
                self.entities
                    .get((i as usize).wrapping_sub(self.instances.len()))
                    .map(|e| e.2)
            }),
            Ud::Flag(f) => {
                let ent = self.flags.get(usize::from(f))?;
                sim.ctf
                    .as_ref()?
                    .flags
                    .get(ent.ctf)
                    .map(|fl| fl.current_pos)
            }
            Ud::Char(c) => self
                .char_unit(sim, c)
                .and_then(|h| sim.unit(h))
                .map(|u| unit_world_pos(sim, u)),
            _ => None,
        }
    }

    pub(crate) fn obj_alive(&self, sim: &Sim, o: Ud) -> bool {
        match o {
            Ud::Unit(i, g) => sim
                .units
                .get(i as usize)
                .is_some_and(|u| u.alive && u.gen == g),
            Ud::Vehicle(i, g) => sim
                .vehicles
                .get(i as usize)
                .is_some_and(|v| v.alive && v.gen == g && v.health.alive()),
            Ud::Station(i) => sim
                .stations
                .get(usize::from(i))
                .is_some_and(|s| !s.destroyed),
            Ud::Post(i) => sim.cps.get(usize::from(i)).is_some_and(|c| !c.killed),
            Ud::Flag(f) => self.flags.get(usize::from(f)).is_some_and(|f| !f.dead),
            Ud::Entity(_) => true,
            _ => false,
        }
    }

    /// Targets of a class marker, including destroyed repairable stations.
    pub(crate) fn objects_of_class(&self, sim: &Sim, class: &str) -> Vec<Ud> {
        let mut out = Vec::new();
        for (i, u) in sim.units.iter().enumerate() {
            if u.alive
                && self
                    .unit_class_name(u.team, u.spec.class)
                    .is_some_and(|c| c.eq_ignore_ascii_case(class))
            {
                out.push(Ud::Unit(i as u32, u.gen));
            }
        }
        for (i, v) in sim.vehicles.iter().enumerate() {
            if v.alive
                && v.health.alive()
                && self
                    .vehicle_classes
                    .get(v.class as usize)
                    .is_some_and(|c| c.eq_ignore_ascii_case(class))
            {
                out.push(Ud::Vehicle(i as u32, v.gen));
            }
        }
        for (i, _) in sim.stations.iter().enumerate() {
            if self
                .station_classes
                .get(i)
                .is_some_and(|c| c.eq_ignore_ascii_case(class))
            {
                out.push(Ud::Station(i as u16));
            }
        }
        out
    }

    /// The weapon a kill is put down to: a rider's seat gun is its last
    /// shot, a soldier's the weapon in hand. OURS: the PS2 keeps the last
    /// hit's weapon on the victim (`GetObjectLastHitWeaponClass`,
    /// `FUN_00190*`); here the killer's at the kill.
    /// replace: trace the per-object last-hit record the binding reads.
    fn killer_weapon(&self, sim: &Sim, killer: UnitHandle) -> Option<String> {
        let u = sim.units.get(killer.index as usize)?;
        let wc = if u.mount.is_some() {
            self.last_shot
                .get(killer.index as usize)
                .copied()
                .flatten()
                .map(|wc| wc as usize)
                .or_else(|| sim.seat_primary_weapon(killer))?
        } else {
            u.weapon_class
        };
        objective_weapon_name(sim, killer, wc)
    }

    fn note_hit(&mut self, obj: Ud, weapon: Option<String>) {
        self.last_hit_weapon.retain(|(o, _)| *o != obj);
        if let Some(w) = weapon {
            self.last_hit_weapon.push((obj, w));
        }
    }
}

/// The weapon an award weapon replaces, for objectives: an award
/// (`MedalsTypeToUnlock` t) stands for the weapon in the same kit it
/// locks out (`MedalsTypeToLock` t), so "kill 3 droids with the sniper
/// rifle" counts the award beam rifle too. Any other weapon is itself.
/// A named difference (maintainer, 2026-11-21): the original names the award's
/// own class, so its kills never meet a base-weapon objective.
fn base_weapon(sim: &Sim, u: &core::unit::Unit, wc: usize) -> usize {
    let Some(t) = sim.weapon_classes.get(wc).and_then(|w| w.unlock_medal()) else {
        return wc;
    };
    std::iter::once(u.weapon_class)
        .chain(u.held.iter().map(|h| h.class))
        .find(|&c| {
            sim.weapon_classes
                .get(c)
                .is_some_and(|w| w.medal_lock == Some(t))
        })
        .unwrap_or(wc)
}

/// The class name `GetObjectLastHitWeaponClass` reports for a hit by
/// `shooter`'s weapon class `wc` (an award as its base weapon).
fn objective_weapon_name(sim: &Sim, shooter: UnitHandle, wc: usize) -> Option<String> {
    let wc = sim
        .units
        .get(shooter.index as usize)
        .map_or(wc, |u| base_weapon(sim, u, wc));
    sim.weapon_classes
        .get(wc)
        .map(|w| w.name.to_ascii_lowercase())
}

/// Why campaign mission `entry` cannot start from this build, in words for the
/// battle screen; `None` when its map is baked and its mission script loads.
/// The launch checks this before the mission's movies, so a build without the
/// map says so instead of playing the movies and dropping back to the screen.
pub fn launch_problem(
    missions_dir: &std::path::Path,
    assets_dir: &std::path::Path,
    entry: &ui::campaign::CampaignMissionEntry,
) -> Option<String> {
    if !pak::is_file(&assets_dir.join(entry.map).join("manifest.ron")) {
        return Some(format!(
            "This mission's map ({}) is not in this build. Run the pack again to bake it.",
            entry.map
        ));
    }
    if let Err(e) = crate::mission::MissionDef::load_campaign(missions_dir, entry.script) {
        eprintln!("campaign: {} does not load: {e:#}", entry.script);
        return Some(format!(
            "This mission's script ({}) could not be read. Run the pack again.",
            entry.script
        ));
    }
    None
}

mod dispatch;
mod markers;
mod objectives;
mod world_ops;

#[cfg(test)]
#[path = "campaign_runtime/tests.rs"]
mod tests;
