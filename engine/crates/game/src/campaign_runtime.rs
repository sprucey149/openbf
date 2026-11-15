//! Sequential objective runtime for Battlefront II's Rise of the Empire campaign.

use crate::classdb::fnv1a_lowercase;
use crate::layout::MapLayout;
use crate::mission::{
    CampaignAmbushDef, CampaignMissionDef, CampaignMutation, CampaignObjectiveKind,
    CampaignStageDef,
};
use core::fmath::FloatExt;
use core::region::Region;
use core::sim::{CtfState, Event, Sim};
use core::unit::UnitHandle;
use glam::Vec3;
use manifest::Manifest;
use std::collections::{BTreeMap, HashMap};

const DEFAULT_GOTO_RADIUS: f32 = 6.0;
const INTERACT_REACH: f32 = 4.5;
const VEHICLE_INTERACT_REACH: f32 = 6.5;

#[derive(Clone, Debug)]
struct RegionTriggerState {
    region: String,
    mutations: Vec<CampaignMutation>,
    fired: bool,
}

#[derive(Clone, Debug)]
struct PendingAmbush {
    def: CampaignAmbushDef,
    fired: bool,
}

#[derive(Clone, Debug)]
struct AssaultTargetState {
    name_lower: String,
    destroyed: bool,
}

/// Drives Rise of the Empire's sequential stages, region triggers, ambushes,
/// property mutations, and delayed victory timer on top of [`Sim`].
#[derive(Clone, Debug)]
pub struct CampaignRuntime {
    spec: CampaignMissionDef,
    stage_index: usize,
    completed: bool,
    stage_timer: Option<f32>,
    victory_timer: Option<f32>,
    pending_messages: Vec<String>,
    pending_movies: Vec<String>,
    allow_ai_pickup: bool,

    post_keys: Vec<String>,
    saved_cp_owners: Vec<Option<u8>>,
    saved_cp_captures: Vec<Region>,

    regions: HashMap<String, Region>,
    paths: HashMap<String, Vec<(Vec3, f32)>>,
    instances: HashMap<String, Vec3>,
    instance_names: Vec<String>,
    station_names: Vec<String>,
    station_classes: Vec<String>,
    door_names: Vec<String>,

    pending_ambushes: Vec<PendingAmbush>,
    region_triggers: Vec<RegionTriggerState>,

    stage_targets: Vec<AssaultTargetState>,
    stage_target_kills: Vec<(String, u32, u32)>,
    stage_tdm_kills: u32,

    seen_dead_units: Vec<u32>,
    seen_dead_vehicles: Vec<u32>,
    seen_destroyed_stations: Vec<bool>,
    team_classes: [Vec<String>; 2],
    team_units: [u32; 2],
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
        let instances = world
            .instances
            .iter()
            .map(|inst| (inst.name.to_ascii_lowercase(), Vec3::from(inst.translation)))
            .collect();
        let instance_names: Vec<String> = world
            .instances
            .iter()
            .map(|inst| inst.name.to_ascii_lowercase())
            .collect();
        let instance_name = |inst_idx: Option<u32>| -> String {
            inst_idx
                .and_then(|i| instance_names.get(i as usize))
                .cloned()
                .unwrap_or_default()
        };
        let station_names = sim
            .stations
            .iter()
            .map(|s| instance_name(s.instance))
            .collect();
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
            .map(|d| instance_name(Some(d.instance as u32)))
            .collect();

        let saved_cp_captures = sim.cps.iter().map(|cp| cp.def.capture).collect();

        Self {
            spec,
            stage_index: 0,
            completed: false,
            stage_timer: None,
            victory_timer: None,
            pending_messages: Vec::new(),
            pending_movies: Vec::new(),
            allow_ai_pickup: true,
            post_keys: post_keys.to_vec(),
            saved_cp_owners,
            saved_cp_captures,
            regions,
            paths,
            instances,
            instance_names,
            station_names,
            station_classes,
            door_names,
            pending_ambushes: Vec::new(),
            region_triggers: Vec::new(),
            stage_targets: Vec::new(),
            stage_target_kills: Vec::new(),
            stage_tdm_kills: 0,
            seen_dead_units: Vec::new(),
            seen_dead_vehicles: Vec::new(),
            seen_destroyed_stations: Vec::new(),
            team_classes: [Vec::new(), Vec::new()],
            team_units: [0, 0],
        }
    }

    pub fn set_team_rosters(&mut self, team_classes: [Vec<String>; 2], team_units: [u32; 2]) {
        self.team_classes = team_classes;
        self.team_units = team_units;
    }

    fn fill_enabled_ai(&self, sim: &mut Sim) {
        for t in 0..2usize {
            let team = t as u8;
            if !sim.ai_spawn_allowed[t] || self.team_units[t] == 0 {
                continue;
            }
            if !sim.classes[t].is_empty()
                && !(0..sim.classes[t].len()).any(|c| sim.is_class_enabled(team, c))
            {
                continue;
            }
            let mut pts = sim.spawn_points(team);
            pts.extend(sim.pad_walker_spawn_points(team));
            if pts.is_empty() {
                continue;
            }
            let target = self.team_units[t];
            let existing = sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == team)
                .count() as u32;
            for k in existing..target {
                let (pos, yaw) = pts[(k as usize) % pts.len()];
                sim.spawn_ai(team, pos, yaw);
            }
        }
    }

    pub fn active_stage_index(&self) -> usize {
        self.stage_index
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn time_left(&self) -> Option<f32> {
        self.stage_timer.map(|t| t.fmax(0.0))
    }

    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_messages)
    }

    pub fn take_movies(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_movies)
    }

    /// Apply startup properties, top-level ambushes, and enter stage 0.
    pub fn start(&mut self, sim: &mut Sim) {
        let init_props = self.spec.initial_properties.clone();
        for (target, key, value) in &init_props {
            self.apply_property(sim, target, key, value);
        }
        let ambushes = self.spec.ambushes.clone();
        for ambush in ambushes {
            self.run_ambush(sim, ambush);
        }
        if self.spec.stages.is_empty() {
            self.finish_all_stages(sim);
        } else {
            self.enter_stage(0, sim);
        }
    }

    pub fn step(
        &mut self,
        sim: &mut Sim,
        player: UnitHandle,
        player_class_name: Option<&str>,
        team_classes: &[Vec<String>; 2],
        native_classes: &BTreeMap<u8, Vec<String>>,
        vehicle_classes: &[String],
        dt: f32,
    ) {
        if sim.outcome.is_some() {
            return;
        }
        self.check_region_triggers(sim, player);
        self.check_pending_ambushes(sim, player);

        if let Some(left) = self.victory_timer.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.victory_timer = None;
                sim.outcome = Some(0);
            }
            return;
        }

        if self.completed {
            return;
        }
        let Some(stage) = self.spec.stages.get(self.stage_index).cloned() else {
            self.finish_all_stages(sim);
            return;
        };

        self.update_stage_progress(sim, &stage, team_classes, native_classes, vehicle_classes);

        let att_team = stage.att_team.saturating_sub(1);
        if let Some(left) = self.stage_timer.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.stage_timer = None;
                let timer_wins = matches!(stage.kind, CampaignObjectiveKind::Defend { .. })
                    || stage.time_limit_winner == Some(1);
                if timer_wins {
                    self.complete_current_stage(sim);
                } else {
                    sim.outcome = Some(if att_team == 0 { 1 } else { 0 });
                }
                return;
            }
        }

        if self.stage_is_complete(sim, &stage, player, player_class_name) {
            self.complete_current_stage(sim);
        }
    }

    pub fn objective_markers(&self, sim: &Sim) -> Vec<Vec3> {
        let Some(stage) = self.spec.stages.get(self.stage_index) else {
            return Vec::new();
        };
        let att_team = stage.att_team.saturating_sub(1);
        match &stage.kind {
            CampaignObjectiveKind::Conquest { posts } => {
                if posts.is_empty() {
                    sim.cps
                        .iter()
                        .filter(|cp| !cp.killed && cp.owner != Some(att_team))
                        .map(|cp| cp.def.pos + Vec3::Y * 3.0)
                        .collect()
                } else {
                    posts
                        .iter()
                        .filter_map(|want| self.find_cp_index(sim, want))
                        .filter_map(|ci| {
                            let cp = &sim.cps[ci];
                            (!cp.killed && cp.owner != Some(att_team))
                                .then_some(cp.def.pos + Vec3::Y * 3.0)
                        })
                        .collect()
                }
            }
            CampaignObjectiveKind::Goto { region } => self
                .find_region_center(region)
                .map(|p| vec![p + Vec3::Y * 1.5])
                .unwrap_or_default(),
            CampaignObjectiveKind::Ctf { capture_region, .. } => {
                if let Some(ctf) = &sim.ctf {
                    let mut out = Vec::new();
                    for f in &ctf.flags {
                        match f.status {
                            core::sim::FlagStatus::Carried { carrier }
                                if sim.unit(carrier).is_some_and(|u| u.team == att_team) =>
                            {
                                let goal = ctf
                                    .capture_regions
                                    .get(usize::from(att_team))
                                    .map(|r| r.pos)
                                    .or_else(|| self.find_region_center(capture_region))
                                    .unwrap_or(f.current_pos);
                                out.push(goal + Vec3::Y * 1.5);
                            }
                            _ => out.push(f.current_pos + Vec3::Y * 1.5),
                        }
                    }
                    out
                } else {
                    self.find_region_center(capture_region)
                        .map(|p| vec![p + Vec3::Y * 1.5])
                        .unwrap_or_default()
                }
            }
            CampaignObjectiveKind::Assault { .. } => {
                let mut out = Vec::new();
                for tgt in &self.stage_targets {
                    if !tgt.destroyed {
                        if let Some(pos) = self.target_pos(sim, &tgt.name_lower) {
                            out.push(pos + Vec3::Y * 2.0);
                        }
                    }
                }
                for (si, st) in sim.stations.iter().enumerate() {
                    if !st.destroyed {
                        let cls = self.station_classes.get(si).map_or("", String::as_str);
                        if self
                            .stage_target_kills
                            .iter()
                            .any(|(req_cls, killed, req)| req_cls == cls && *killed < *req)
                        {
                            out.push(st.pos + Vec3::Y * 2.0);
                        }
                    }
                }
                out
            }
            CampaignObjectiveKind::Interact { region, target, .. } => {
                if let Some(t) = target {
                    self.target_pos(sim, &t.to_ascii_lowercase())
                        .map(|p| vec![p + Vec3::Y * 1.5])
                        .unwrap_or_default()
                } else if let Some(r) = region {
                    self.find_region_center(r)
                        .map(|p| vec![p + Vec3::Y * 1.5])
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            }
            CampaignObjectiveKind::Tdm { .. } | CampaignObjectiveKind::Defend { .. } => Vec::new(),
        }
    }

    fn enter_stage(&mut self, index: usize, sim: &mut Sim) {
        self.stage_index = index;
        self.stage_timer = None;
        self.stage_targets.clear();
        self.stage_target_kills.clear();
        self.stage_tdm_kills = 0;

        let Some(stage) = self.spec.stages.get(index).cloned() else {
            self.finish_all_stages(sim);
            return;
        };

        for mut_op in &stage.on_start {
            self.apply_mutation(sim, mut_op);
        }
        self.fill_enabled_ai(sim);

        if let Some(limit) = stage.time_limit {
            self.stage_timer = Some(limit.fmax(0.1));
        } else if let CampaignObjectiveKind::Defend { seconds } = stage.kind {
            self.stage_timer = Some(seconds.fmax(0.1));
        }

        match &stage.kind {
            CampaignObjectiveKind::Assault {
                targets,
                target_types,
            } => {
                for t in targets {
                    self.stage_targets.push(AssaultTargetState {
                        name_lower: t.to_ascii_lowercase(),
                        destroyed: false,
                    });
                }
                for (cls, count) in target_types {
                    self.stage_target_kills
                        .push((cls.to_ascii_lowercase(), 0, *count));
                }
            }
            CampaignObjectiveKind::Ctf {
                flag,
                home_region,
                capture_region,
                capture_limit,
            } => {
                self.install_stage_ctf(
                    sim,
                    stage.att_team.saturating_sub(1),
                    flag,
                    home_region,
                    capture_region,
                    (*capture_limit).max(1),
                );
            }
            _ => {}
        }

        self.snapshot_entities(sim);

        if !stage.text.is_empty() {
            self.pending_messages.push(stage.text.clone());
        }
        if let Some(popup) = stage.popup_text.as_ref().filter(|s| !s.is_empty()) {
            self.pending_messages.push(popup.clone());
        }
    }

    fn complete_current_stage(&mut self, sim: &mut Sim) {
        let Some(stage) = self.spec.stages.get(self.stage_index).cloned() else {
            self.finish_all_stages(sim);
            return;
        };
        if matches!(stage.kind, CampaignObjectiveKind::Ctf { .. }) {
            sim.ctf = None;
            self.allow_ai_pickup = true;
        }
        self.stage_timer = None;
        for mut_op in &stage.on_complete {
            self.apply_mutation(sim, mut_op);
        }
        let next = self.stage_index + 1;
        if next < self.spec.stages.len() {
            self.enter_stage(next, sim);
        } else {
            self.finish_all_stages(sim);
        }
    }

    fn finish_all_stages(&mut self, sim: &mut Sim) {
        self.completed = true;
        self.stage_timer = None;
        if self.spec.delay_victory_time <= 0.0 {
            sim.outcome = Some(0);
        } else {
            self.victory_timer = Some(self.spec.delay_victory_time);
        }
    }

    fn snapshot_entities(&mut self, sim: &Sim) {
        self.seen_dead_units.resize(sim.units.len(), 0);
        for (i, u) in sim.units.iter().enumerate() {
            self.seen_dead_units[i] = if !u.alive { u.gen } else { 0 };
        }
        self.seen_dead_vehicles.resize(sim.vehicles.len(), 0);
        for (i, v) in sim.vehicles.iter().enumerate() {
            self.seen_dead_vehicles[i] = if !v.alive { v.gen } else { 0 };
        }
        self.seen_destroyed_stations
            .resize(sim.stations.len(), false);
        for (i, s) in sim.stations.iter().enumerate() {
            self.seen_destroyed_stations[i] = s.destroyed;
        }
    }

    fn update_stage_progress(
        &mut self,
        sim: &Sim,
        stage: &CampaignStageDef,
        team_classes: &[Vec<String>; 2],
        native_classes: &BTreeMap<u8, Vec<String>>,
        vehicle_classes: &[String],
    ) {
        let att_team = stage.att_team.saturating_sub(1);

        // Count TDM kills directly from this tick's kill events when attacker kills a foe.
        for e in &sim.events {
            if let Event::Kill { shooter, victim } = *e {
                let by_att = sim.unit(shooter).is_some_and(|u| u.team == att_team);
                let foe = sim
                    .units
                    .get(victim.index as usize)
                    .is_some_and(|v| sim.hostile(att_team, v.team));
                if by_att && foe {
                    self.stage_tdm_kills += 1;
                }
            }
        }

        // Update named target states.
        let station_names = &self.station_names;
        let instance_names = &self.instance_names;
        for tgt in &mut self.stage_targets {
            if tgt.destroyed {
                continue;
            }
            if let Some(si) = station_names
                .iter()
                .position(|n| !n.is_empty() && n == &tgt.name_lower)
            {
                if sim.stations.get(si).is_some_and(|s| s.destroyed) {
                    tgt.destroyed = true;
                }
                continue;
            }
            if let Some(bt) = sim.building_turrets.iter().find(|bt| {
                instance_names
                    .get(bt.instance as usize)
                    .is_some_and(|n| n == &tgt.name_lower)
            }) {
                let dead = bt.down.is_some()
                    || sim
                        .vehicle(bt.handle)
                        .is_none_or(|v| !v.alive || !v.health.alive());
                if dead {
                    tgt.destroyed = true;
                }
                continue;
            }
            if let Some(pad) = sim
                .vehicle_pads
                .iter()
                .find(|p| p.def.name.eq_ignore_ascii_case(&tgt.name_lower))
            {
                let has_dead_or_down = pad
                    .vehicles
                    .iter()
                    .any(|pv| sim.vehicle(pv.handle).is_none_or(|v| !v.alive || !v.health.alive()));
                if has_dead_or_down {
                    tgt.destroyed = true;
                }
            }
        }

        // Scan newly dead units for class-typed assault kills.
        self.seen_dead_units.resize(sim.units.len(), 0);
        for (i, u) in sim.units.iter().enumerate() {
            if u.alive {
                self.seen_dead_units[i] = 0;
                continue;
            }
            if u.gen == 0 || self.seen_dead_units[i] == u.gen {
                continue;
            }
            self.seen_dead_units[i] = u.gen;
            if sim.hostile(att_team, u.team) {
                if let Some(cls) = unit_class_name(u.team, u.spec.class, team_classes, native_classes)
                {
                    let lower = cls.to_ascii_lowercase();
                    for (req_cls, killed, _) in &mut self.stage_target_kills {
                        if req_cls == &lower {
                            *killed += 1;
                        }
                    }
                }
            }
        }

        // Scan newly dead vehicles for class-typed assault kills.
        self.seen_dead_vehicles.resize(sim.vehicles.len(), 0);
        for (i, v) in sim.vehicles.iter().enumerate() {
            if v.alive {
                self.seen_dead_vehicles[i] = 0;
                continue;
            }
            if v.gen == 0 || self.seen_dead_vehicles[i] == v.gen {
                continue;
            }
            self.seen_dead_vehicles[i] = v.gen;
            if v.team.is_none_or(|t| sim.hostile(att_team, t)) {
                if let Some(cls) = vehicle_classes.get(v.class as usize) {
                    let lower = cls.to_ascii_lowercase();
                    for (req_cls, killed, _) in &mut self.stage_target_kills {
                        if req_cls == &lower {
                            *killed += 1;
                        }
                    }
                }
            }
        }

        // Scan newly destroyed stations for class-typed assault targets.
        self.seen_destroyed_stations
            .resize(sim.stations.len(), false);
        for (i, s) in sim.stations.iter().enumerate() {
            if s.destroyed && !self.seen_destroyed_stations[i] {
                self.seen_destroyed_stations[i] = true;
                if let Some(lower) = self.station_classes.get(i) {
                    for (req_cls, killed, _) in &mut self.stage_target_kills {
                        if req_cls == lower {
                            *killed += 1;
                        }
                    }
                }
            }
        }
    }

    fn stage_is_complete(
        &self,
        sim: &Sim,
        stage: &CampaignStageDef,
        player: UnitHandle,
        player_class_name: Option<&str>,
    ) -> bool {
        let att_team = stage.att_team.saturating_sub(1);
        match &stage.kind {
            CampaignObjectiveKind::Conquest { posts } => {
                if posts.is_empty() {
                    !sim.cps.is_empty()
                        && sim
                            .cps
                            .iter()
                            .filter(|cp| !cp.killed)
                            .all(|cp| cp.owner == Some(att_team))
                } else {
                    posts.iter().all(|want| {
                        self.find_cp_index(sim, want).is_some_and(|ci| {
                            let cp = &sim.cps[ci];
                            !cp.killed && cp.owner == Some(att_team)
                        })
                    })
                }
            }
            CampaignObjectiveKind::Goto { region } => {
                self.actor_in_region(sim, player, att_team, region)
            }
            CampaignObjectiveKind::Tdm { kills } => self.stage_tdm_kills >= *kills,
            CampaignObjectiveKind::Defend { .. } => false,
            CampaignObjectiveKind::Ctf { capture_limit, .. } => sim
                .ctf
                .as_ref()
                .and_then(|c| c.team_points.get(usize::from(att_team)))
                .is_some_and(|&pts| pts >= (*capture_limit).max(1)),
            CampaignObjectiveKind::Assault { .. } => {
                self.stage_targets.iter().all(|t| t.destroyed)
                    && self
                        .stage_target_kills
                        .iter()
                        .all(|(_, killed, req)| *killed >= *req)
            }
            CampaignObjectiveKind::Interact {
                region,
                target,
                class_filter,
                ..
            } => {
                let Some(pos) = self.player_pos(sim, player, att_team) else {
                    return false;
                };
                if let Some(cf) = class_filter {
                    if !player_class_name.is_some_and(|n| n.eq_ignore_ascii_case(cf)) {
                        return false;
                    }
                }
                if let Some(reg) = region {
                    if !self.point_in_region(pos, reg) {
                        return false;
                    }
                }
                if let Some(tgt) = target {
                    let lower = tgt.to_ascii_lowercase();
                    // Mounted inside a vehicle whose spawn pad matches `tgt` satisfies vehicle boarding.
                    let in_matching_vehicle = sim
                        .unit(player)
                        .and_then(|u| u.mount)
                        .and_then(|m| sim.vehicles.get(m.vehicle.index as usize))
                        .and_then(|v| v.spawn_pad)
                        .and_then(|p| sim.vehicle_pads.get(p as usize))
                        .is_some_and(|pad| pad.def.name.eq_ignore_ascii_case(tgt));
                    if in_matching_vehicle {
                        return true;
                    }
                    let Some(tgt_pos) = self.target_pos(sim, &lower) else {
                        return false;
                    };
                    let reach = if sim
                        .vehicle_pads
                        .iter()
                        .any(|p| p.def.name.eq_ignore_ascii_case(tgt))
                    {
                        VEHICLE_INTERACT_REACH
                    } else {
                        INTERACT_REACH
                    };
                    if (pos - tgt_pos).length_squared() > reach * reach {
                        return false;
                    }
                }
                true
            }
        }
    }

    fn apply_mutation(&mut self, sim: &mut Sim, op: &CampaignMutation) {
        match op {
            CampaignMutation::KillObject(name) => self.kill_object(sim, name),
            CampaignMutation::RespawnObject(name) => self.respawn_object(sim, name),
            CampaignMutation::SetBarriers { name, enable } => {
                if let Some(nav) = sim.nav.as_mut() {
                    nav.set_barriers(fnv1a_lowercase(name), *enable);
                }
            }
            CampaignMutation::SetArcBlocks { name, block } => {
                if let Some(nav) = sim.nav.as_mut() {
                    nav.block_arcs(&[fnv1a_lowercase(name)], *block);
                }
            }
            CampaignMutation::SetProperty { target, key, value } => {
                self.apply_property(sim, target, key, value);
            }
            CampaignMutation::AddReinforcements { team, delta } => {
                if let Some(t) = team.checked_sub(1).map(usize::from).filter(|&t| t < 2) {
                    let cur = sim.battle.teams[t].reinforcements;
                    sim.battle.teams[t].reinforcements = (cur + *delta as f32).fmax(0.0);
                }
            }
            CampaignMutation::SetReinforcements { team, count } => {
                if let Some(t) = team.checked_sub(1).map(usize::from).filter(|&t| t < 2) {
                    if *count < 0 {
                        sim.battle.unlimited_teams[t] = true;
                        sim.battle.teams[t].reinforcements =
                            sim.battle.teams[t].reinforcements.fmax(100.0);
                    } else {
                        sim.battle.unlimited_teams[t] = false;
                        sim.battle.teams[t].reinforcements = (*count as f32).fmax(0.0);
                    }
                }
            }
            CampaignMutation::UnlockHeroForTeam(team) => {
                if let Some(t) = team.checked_sub(1).map(usize::from).filter(|&t| t < 2) {
                    if let Some(heroes) = sim.heroes.as_mut() {
                        heroes.teams[t].unlocked = true;
                        heroes.teams[t].exhausted = false;
                        heroes.teams[t].ready_at = 0;
                    }
                }
            }
            CampaignMutation::PlayMovie(name) => {
                self.pending_movies.push(name.clone());
            }
            CampaignMutation::Ambush(def) => {
                self.run_ambush(sim, def.clone());
            }
            CampaignMutation::AddUnitClass {
                team,
                class,
                min,
                max,
            } => {
                let sim_team = team.saturating_sub(1);
                let t = usize::from(sim_team);
                if t < 2 {
                    if let Some(ci) = self.team_classes[t]
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(class))
                    {
                        if let Some(uc) = sim.classes[t].get_mut(ci) {
                            uc.min = *min;
                            uc.max = *max;
                        }
                        sim.set_class_enabled(sim_team, ci, true);
                        self.fill_enabled_ai(sim);
                    }
                }
            }
            CampaignMutation::SetClassEnabled {
                team,
                class,
                enabled,
            } => {
                let sim_team = team.saturating_sub(1);
                let t = usize::from(sim_team);
                if t < 2 {
                    if let Some(ci) = self.team_classes[t]
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(class))
                    {
                        sim.set_class_enabled(sim_team, ci, *enabled);
                        if *enabled {
                            self.fill_enabled_ai(sim);
                        }
                    }
                }
            }
            CampaignMutation::AllowAiSpawn { team, allow } => {
                let sim_team = team.saturating_sub(1);
                if usize::from(sim_team) < 2 {
                    sim.set_ai_spawn_allowed(sim_team, *allow);
                    if *allow {
                        self.fill_enabled_ai(sim);
                    }
                }
            }
            CampaignMutation::RegionTrigger { region, mutations } => {
                self.region_triggers.push(RegionTriggerState {
                    region: region.to_ascii_lowercase(),
                    mutations: mutations.clone(),
                    fired: false,
                });
            }
        }
    }

    fn kill_object(&mut self, sim: &mut Sim, name: &str) {
        if let Some(ci) = self.find_cp_index(sim, name) {
            if self.saved_cp_owners.len() <= ci {
                self.saved_cp_owners.resize(sim.cps.len(), None);
            }
            if sim.cps[ci].owner.is_some() {
                self.saved_cp_owners[ci] = sim.cps[ci].owner;
            }
            sim.cps[ci].killed = true;
            sim.cps[ci].owner = None;
            sim.cps[ci].capture = 0.0;
            sim.cps[ci].neutral = 0.0;
        }
        let lower = name.to_ascii_lowercase();
        for (si, st_name) in self.station_names.iter().enumerate() {
            if !st_name.is_empty() && st_name == &lower {
                if sim.stations.get(si).is_some_and(|s| !s.destroyed) {
                    sim.install_station_wreck(si as u16);
                }
            }
        }
        for pad in &mut sim.vehicle_pads {
            if pad.def.name.eq_ignore_ascii_case(name) {
                pad.team = None;
                pad.armed = false;
                for pv in &pad.vehicles {
                    if let Some(v) = sim.vehicles.get_mut(pv.handle.index as usize) {
                        if v.gen == pv.handle.gen {
                            v.health.cur = 0.0;
                            v.alive = false;
                        }
                    }
                }
            }
        }
    }

    fn respawn_object(&mut self, sim: &mut Sim, name: &str) {
        if let Some(ci) = self.find_cp_index(sim, name) {
            sim.cps[ci].killed = false;
            if let Some(&cap) = self.saved_cp_captures.get(ci) {
                sim.cps[ci].def.capture = cap;
                sim.cps[ci].capture_authored = true;
            }
            if sim.cps[ci].owner.is_none() {
                let restored = self.saved_cp_owners.get(ci).copied().flatten().or(Some(1));
                sim.cps[ci].owner = restored;
                sim.cps[ci].capture = 0.0;
                sim.cps[ci].neutral = 0.0;
            }
            self.fill_enabled_ai(sim);
        }
        let lower = name.to_ascii_lowercase();
        for (si, st_name) in self.station_names.iter().enumerate() {
            if !st_name.is_empty() && st_name == &lower {
                sim.respawn_station(si as u16);
            }
        }
    }

    fn apply_property(&mut self, sim: &mut Sim, target: &str, key: &str, value: &str) {
        let key_lower = key.trim().to_ascii_lowercase();
        let val = value.trim();

        if key_lower == "allowaipickup" {
            let allow = val != "0";
            self.allow_ai_pickup = allow;
            if let Some(ctf) = sim.ctf.as_mut() {
                for f in &mut ctf.flags {
                    f.allow_ai_pickup = allow;
                }
            }
            return;
        }

        if key_lower == "islocked" {
            let locked = val.parse::<u8>().is_ok_and(|n| n != 0);
            let tgt_lower = target.to_ascii_lowercase();
            for (di, dname) in self.door_names.iter().enumerate() {
                if !dname.is_empty() && dname == &tgt_lower {
                    if let Some(door) = sim.doors.get_mut(di) {
                        door.locked = locked;
                    }
                }
            }
            return;
        }

        if let Some(ci) = self.find_cp_index(sim, target) {
            match key_lower.as_str() {
                "isvisible" => {
                    sim.cps[ci].visible = val != "0";
                }
                "team" => {
                    if let Ok(team) = val.parse::<u8>() {
                        let owner = (team >= 1).then(|| team - 1);
                        if self.saved_cp_owners.len() <= ci {
                            self.saved_cp_owners.resize(sim.cps.len(), None);
                        }
                        self.saved_cp_owners[ci] = owner;
                        if !sim.cps[ci].killed {
                            sim.cps[ci].owner = owner;
                            sim.cps[ci].capture = 0.0;
                            sim.cps[ci].neutral = 0.0;
                        }
                        self.fill_enabled_ai(sim);
                    }
                }
                "spawnpath" => {
                    if val.is_empty() {
                        sim.cps[ci].def.spawn_nodes.clear();
                    } else if let Some(nodes) = self.paths.get(&val.to_ascii_lowercase()) {
                        sim.cps[ci].def.spawn_nodes = nodes.clone();
                        self.fill_enabled_ai(sim);
                    }
                }
                "captureregion" => {
                    if !val.is_empty() {
                        if let Some(r) = self.regions.get(&val.to_ascii_lowercase()) {
                            sim.cps[ci].def.capture = *r;
                            sim.cps[ci].capture_authored = true;
                            sim.cps[ci].capturable = true;
                            if let Some(saved) = self.saved_cp_captures.get_mut(ci) {
                                *saved = *r;
                            }
                        } else {
                            sim.cps[ci].def.capture =
                                Region::sphere(Vec3::new(0.0, -10000.0, 0.0), 0.0);
                            sim.cps[ci].capture_authored = false;
                            sim.cps[ci].capturable = false;
                        }
                    } else {
                        sim.cps[ci].def.capture =
                            Region::sphere(Vec3::new(0.0, -10000.0, 0.0), 0.0);
                        sim.cps[ci].capture_authored = false;
                        sim.cps[ci].capturable = false;
                    }
                }
                "controlregion" => {
                    sim.cps[ci].def.control = self.regions.get(&val.to_ascii_lowercase()).copied();
                }
                "aispawnweight" => {
                    if let Ok(w) = val.parse::<f32>() {
                        sim.cps[ci].ai_spawn_weight = w.fmax(0.0);
                    }
                }
                "value_atk_alliance"
                | "value_atk_empire"
                | "value_atk_republic"
                | "value_atk_cis" => {
                    if let Ok(v) = val.parse::<i32>() {
                        let idx = usize::from(key_lower.ends_with("empire") || key_lower.ends_with("cis"));
                        sim.cps[ci].def.value_atk[idx] = v;
                    }
                }
                "value_def_alliance"
                | "value_def_empire"
                | "value_def_republic"
                | "value_def_cis" => {
                    if let Ok(v) = val.parse::<i32>() {
                        let idx = usize::from(key_lower.ends_with("empire") || key_lower.ends_with("cis"));
                        sim.cps[ci].def.value_def[idx] = v;
                    }
                }
                _ => {}
            }
        }

        let tgt_lower = target.to_ascii_lowercase();
        for (si, st_name) in self.station_names.iter().enumerate() {
            if st_name.is_empty() || st_name != &tgt_lower {
                continue;
            }
            let Some(st) = sim.stations.get_mut(si) else {
                continue;
            };
            match key_lower.as_str() {
                "team" => {
                    if let Ok(team) = val.parse::<u8>() {
                        st.team = (team >= 1).then(|| team - 1);
                    }
                }
                "curhealth" => {
                    if let Ok(hp) = val.parse::<f32>() {
                        if hp <= 0.0 {
                            if !st.destroyed {
                                sim.install_station_wreck(si as u16);
                            }
                        } else {
                            st.health.max = st.health.max.fmax(hp);
                            st.health.cur = hp;
                            if st.destroyed {
                                sim.respawn_station(si as u16);
                                sim.stations[si].health.cur = hp;
                            }
                        }
                    }
                }
                "maxhealth" => {
                    if let Ok(hp) = val.parse::<f32>() {
                        let max_hp = hp.fmax(1.0);
                        st.health.max = max_hp;
                        st.health.cur = st.health.cur.fmin(max_hp);
                    }
                }
                _ => {}
            }
        }
    }

    fn run_ambush(&mut self, sim: &mut Sim, def: CampaignAmbushDef) {
        if def.trigger_region.is_some() {
            self.pending_ambushes.push(PendingAmbush { def, fired: false });
        } else {
            self.spawn_ambush_now(sim, &def);
        }
    }

    fn spawn_ambush_now(&self, sim: &mut Sim, def: &CampaignAmbushDef) {
        let Some(nodes) = self.paths.get(&def.path.to_ascii_lowercase()) else {
            return;
        };
        let port_team = def.team.saturating_sub(1);
        sim.spawn_native_ambush(port_team, def.count, nodes);
    }

    fn check_pending_ambushes(&mut self, sim: &mut Sim, player: UnitHandle) {
        let Some(pos) = self.player_pos(sim, player, 0) else {
            return;
        };
        let regions = &self.regions;
        let instances = &self.instances;
        let mut to_spawn = Vec::new();
        for entry in &mut self.pending_ambushes {
            if entry.fired {
                continue;
            }
            if let Some(region) = &entry.def.trigger_region {
                if point_in_region_parts(regions, instances, pos, &region.to_ascii_lowercase()) {
                    entry.fired = true;
                    to_spawn.push(entry.def.clone());
                }
            }
        }
        for def in to_spawn {
            self.spawn_ambush_now(sim, &def);
        }
    }

    fn check_region_triggers(&mut self, sim: &mut Sim, player: UnitHandle) {
        let Some(pos) = self.player_pos(sim, player, 0) else {
            return;
        };
        let regions = &self.regions;
        let instances = &self.instances;
        let mut fired_mutations = Vec::new();
        for trig in &mut self.region_triggers {
            if !trig.fired && point_in_region_parts(regions, instances, pos, &trig.region) {
                trig.fired = true;
                fired_mutations.extend(trig.mutations.clone());
            }
        }
        for op in &fired_mutations {
            self.apply_mutation(sim, op);
        }
    }

    fn install_stage_ctf(
        &self,
        sim: &mut Sim,
        att_team: u8,
        flag: &str,
        home_region: &str,
        capture_region: &str,
        capture_limit: u32,
    ) {
        let home_pos = if !home_region.is_empty() {
            self.find_region_center(home_region)
                .or_else(|| self.target_pos(sim, &flag.to_ascii_lowercase()))
        } else {
            self.target_pos(sim, &flag.to_ascii_lowercase())
                .or_else(|| self.find_region_center(flag))
        }
        .or_else(|| sim.cps.first().map(|c| c.def.pos))
        .unwrap_or(Vec3::ZERO);

        let (goal_pos, goal_radius) = self
            .regions
            .get(&capture_region.to_ascii_lowercase())
            .map(|r| (r.center, r.radius().fmax(4.0)))
            .or_else(|| {
                self.instances
                    .get(&capture_region.to_ascii_lowercase())
                    .map(|&p| (p, core::sim::ctf::DEFAULT_CAPTURE_REGION_RADIUS))
            })
            .unwrap_or((home_pos, core::sim::ctf::DEFAULT_CAPTURE_REGION_RADIUS));

        let mut ctf = if sim.space_assault.is_some() {
            CtfState::new_space_one_flag(capture_limit, home_pos, goal_pos, goal_pos)
        } else {
            CtfState::new_ground_one_flag(capture_limit, home_pos, goal_pos, goal_pos)
        };
        if let Some(region) = ctf.capture_regions.get_mut(usize::from(att_team)) {
            region.pos = goal_pos;
            region.radius = goal_radius;
        }
        for f in &mut ctf.flags {
            f.allow_ai_pickup = self.allow_ai_pickup;
        }
        sim.ctf = Some(ctf);
    }

    fn find_cp_index(&self, sim: &Sim, name: &str) -> Option<usize> {
        sim.cps
            .iter()
            .position(|cp| cp.def.name.eq_ignore_ascii_case(name))
            .or_else(|| {
                let lower = name.to_ascii_lowercase();
                self.post_keys.iter().position(|k| {
                    let k_low = k.to_ascii_lowercase();
                    k_low == lower || k_low.rsplit('.').next() == Some(lower.as_str())
                })
            })
    }

    fn find_region_center(&self, name: &str) -> Option<Vec3> {
        let lower = name.to_ascii_lowercase();
        self.regions
            .get(&lower)
            .map(|r| r.center)
            .or_else(|| self.instances.get(&lower).copied())
    }

    fn target_pos(&self, sim: &Sim, name_lower: &str) -> Option<Vec3> {
        if let Some(&pos) = self.instances.get(name_lower) {
            return Some(pos);
        }
        if let Some(si) = self
            .station_names
            .iter()
            .position(|n| !n.is_empty() && n == name_lower)
        {
            return sim.stations.get(si).map(|s| s.pos);
        }
        if let Some(pad) = sim
            .vehicle_pads
            .iter()
            .find(|p| p.def.name.eq_ignore_ascii_case(name_lower))
        {
            return Some(pad.def.pos);
        }
        if let Some(ci) = self.find_cp_index(sim, name_lower) {
            return Some(sim.cps[ci].def.pos);
        }
        self.regions.get(name_lower).map(|r| r.center)
    }

    fn player_pos(&self, sim: &Sim, player: UnitHandle, att_team: u8) -> Option<Vec3> {
        if let Some(u) = sim.unit(player).filter(|u| u.alive && u.team == att_team) {
            return Some(unit_world_pos(sim, u));
        }
        sim.units
            .iter()
            .find(|u| u.alive && u.team == att_team && u.is_player)
            .or_else(|| sim.units.iter().find(|u| u.alive && u.team == att_team))
            .map(|u| unit_world_pos(sim, u))
    }

    fn actor_in_region(&self, sim: &Sim, player: UnitHandle, att_team: u8, name: &str) -> bool {
        self.actor_in_region_lower(sim, player, att_team, &name.to_ascii_lowercase())
    }

    fn actor_in_region_lower(
        &self,
        sim: &Sim,
        player: UnitHandle,
        att_team: u8,
        name_lower: &str,
    ) -> bool {
        let Some(pos) = self.player_pos(sim, player, att_team) else {
            return false;
        };
        self.point_in_region_lower(pos, name_lower)
    }

    fn point_in_region(&self, pos: Vec3, name: &str) -> bool {
        self.point_in_region_lower(pos, &name.to_ascii_lowercase())
    }

    fn point_in_region_lower(&self, pos: Vec3, name_lower: &str) -> bool {
        point_in_region_parts(&self.regions, &self.instances, pos, name_lower)
    }
}

fn point_in_region_parts(
    regions: &HashMap<String, Region>,
    instances: &HashMap<String, Vec3>,
    pos: Vec3,
    name_lower: &str,
) -> bool {
    if let Some(r) = regions.get(name_lower) {
        return r.contains(pos);
    }
    if let Some(&center) = instances.get(name_lower) {
        return (pos - center).length_squared() < DEFAULT_GOTO_RADIUS * DEFAULT_GOTO_RADIUS;
    }
    false
}

fn unit_world_pos(sim: &Sim, u: &core::unit::Unit) -> Vec3 {
    u.mount
        .and_then(|m| sim.vehicles.get(m.vehicle.index as usize))
        .map_or(u.soldier.pos, |v| v.pos)
}

fn unit_class_name<'a>(
    team: u8,
    class: u8,
    team_classes: &'a [Vec<String>; 2],
    native_classes: &'a BTreeMap<u8, Vec<String>>,
) -> Option<&'a str> {
    if usize::from(team) < 2 {
        team_classes[usize::from(team)]
            .get(usize::from(class))
            .map(String::as_str)
    } else {
        native_classes
            .get(&team)?
            .get(usize::from(class))
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::MissionDef;
    use core::cp::CpDef;
    use core::unit::UnitSpec;

    fn test_manifest() -> Manifest {
        Manifest {
            spawn_only_posts: false,
            map: "test".into(),
            source_game: "bf2".into(),
            terrain_name: None,
            sky_name: None,
            terrain_glb: None,
            models: Vec::new(),
            textures: Vec::new(),
            transparent: Vec::new(),
            texture_tiers: Vec::new(),
            classes: Vec::new(),
            geometries: Vec::new(),
            regions: Vec::new(),
            instances: vec![manifest::ManifestInstance {
                class: "prop".into(),
                name: "flag_home".into(),
                rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                translation: [10.0, 0.0, 0.0],
                props: Vec::new(),
            }],
            paths: Vec::new(),
            layers: Vec::new(),
            lighting: Default::default(),
            sky: None,
            water: None,
            collision: Vec::new(),
            terrain_cutters: Vec::new(),
            nav: None,
            texture_tier2: Vec::new(),
            lowres_glb: None,
            foliage: None,
            terrain_layers_glb: None,
            leaf: Vec::new(),
            hdr: None,
            point_lights: Vec::new(),
            lightning: None,
            precipitation: None,
            wind: None,
            terrain_query_size: None,
            fog_cloud: None,
            sun_flare: None,
            terrain_detail: None,
        }
    }

    fn test_layout() -> MapLayout {
        MapLayout {
            map: "test".into(),
            cps: Vec::new(),
            paths: Vec::new(),
            regions: vec![
                ("goto_zone".into(), Region::sphere(Vec3::new(5.0, 0.0, 0.0), 3.0)),
                ("cap_zone".into(), Region::sphere(Vec3::new(20.0, 0.0, 0.0), 4.0)),
            ],
        }
    }

    #[test]
    fn every_campaign_mission_initializes_runtime() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../missions/campaign");
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("campaign missions dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        assert_eq!(entries.len(), 18, "all 18 Rise of the Empire missions");
        let layout = MapLayout {
            map: "test".into(),
            cps: Vec::new(),
            paths: Vec::new(),
            regions: Vec::new(),
        };
        let manifest = test_manifest();
        for path in entries {
            let mission = MissionDef::load(&path).expect("load campaign mission");
            let spec = mission.campaign.expect("campaign block");
            let mut sim = Sim::empty(1);
            sim.battle.campaign_rules = true;
            sim.battle.unlimited_teams = spec.infinite_tickets;
            let saved = CampaignRuntime::kill_startup_posts(&mut sim, &mission.killed);
            let mut rt = CampaignRuntime::new(spec, &layout, &manifest, &[], saved, &sim);
            rt.start(&mut sim);
            assert_eq!(rt.active_stage_index(), 0);
            assert!(!rt.take_messages().is_empty());
        }
    }

    #[test]
    fn stages_advance_through_goto_conquest_ctf_and_victory() {
        let mut sim = Sim::empty(7);
        sim.battle = core::battle::Battle::new([50, 100]);
        sim.battle.campaign_rules = true;
        sim.battle.unlimited_teams = [false, true];
        sim.add_cp(
            CpDef {
                name: "CP1".into(),
                pos: Vec3::new(0.0, 0.0, 0.0),
                capture: Region::sphere(Vec3::ZERO, 5.0),
                control: None,
                value_bleed: 0,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: vec![(Vec3::ZERO, 0.0)],
                capture_time: 10.0,
                neutralize_time: 10.0,
            },
            Some(1),
        );
        let player = sim.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();

        let spec = CampaignMissionDef {
            intro_movie: None,
            outro_movie: None,
            mid_movies: Vec::new(),
            infinite_tickets: [false, true],
            delay_victory_time: 0.5,
            initial_properties: Vec::new(),
            ambushes: Vec::new(),
            stages: vec![
                CampaignStageDef {
                    name: "s1".into(),
                    kind: CampaignObjectiveKind::Goto {
                        region: "goto_zone".into(),
                    },
                    text: "Reach zone".into(),
                    popup_text: None,
                    att_team: 1,
                    def_team: 2,
                    time_limit: None,
                    time_limit_winner: None,
                    on_start: Vec::new(),
                    on_complete: vec![CampaignMutation::AddReinforcements { team: 1, delta: 10 }],
                },
                CampaignStageDef {
                    name: "s2".into(),
                    kind: CampaignObjectiveKind::Conquest {
                        posts: vec!["CP1".into()],
                    },
                    text: "Capture CP1".into(),
                    popup_text: None,
                    att_team: 1,
                    def_team: 2,
                    time_limit: None,
                    time_limit_winner: None,
                    on_start: Vec::new(),
                    on_complete: Vec::new(),
                },
                CampaignStageDef {
                    name: "s3".into(),
                    kind: CampaignObjectiveKind::Ctf {
                        flag: "flag_home".into(),
                        home_region: String::new(),
                        capture_region: "cap_zone".into(),
                        capture_limit: 1,
                    },
                    text: "Deliver flag".into(),
                    popup_text: None,
                    att_team: 1,
                    def_team: 2,
                    time_limit: None,
                    time_limit_winner: None,
                    on_start: Vec::new(),
                    on_complete: Vec::new(),
                },
            ],
        };

        let saved = vec![Some(1)];
        let manifest = test_manifest();
        let layout = test_layout();
        let mut rt = CampaignRuntime::new(spec, &layout, &manifest, &[], saved, &sim);
        rt.start(&mut sim);
        assert_eq!(rt.active_stage_index(), 0);
        assert_eq!(rt.take_messages(), vec!["Reach zone"]);

        // Move player into goto_zone -> advances to stage 1 and awards +10 tickets.
        sim.units[player.index as usize].soldier.pos = Vec3::new(5.0, 0.0, 0.0);
        rt.step(&mut sim, player, None, &[vec![], vec![]], &BTreeMap::new(), &[], core::TICK_DT);
        assert_eq!(rt.active_stage_index(), 1);
        assert_eq!(sim.battle.teams[0].tickets(), 60);

        // Flip CP1 to team 0 -> installs CTF stage 2.
        sim.cps[0].owner = Some(0);
        rt.step(&mut sim, player, None, &[vec![], vec![]], &BTreeMap::new(), &[], core::TICK_DT);
        assert_eq!(rt.active_stage_index(), 2);
        assert!(sim.ctf.is_some());

        // Score the CTF point; campaign_rules prevents premature round end until delay timer runs.
        sim.ctf.as_mut().unwrap().team_points[0] = 1;
        rt.step(&mut sim, player, None, &[vec![], vec![]], &BTreeMap::new(), &[], core::TICK_DT);
        assert!(rt.is_completed());
        assert!(sim.ctf.is_none());
        assert_eq!(sim.outcome, None);

        // Elapse 0.5s victory delay -> team 0 wins.
        for _ in 0..20 {
            rt.step(&mut sim, player, None, &[vec![], vec![]], &BTreeMap::new(), &[], core::TICK_DT);
        }
        assert_eq!(sim.outcome, Some(0));
    }

    fn setup_baked_campaign(
        missions_dir: &std::path::Path,
        baked_dir: &std::path::Path,
        script: &str,
    ) -> (crate::mission::MissionDef, crate::walk::WalkSim, CampaignRuntime) {
        let def = crate::mission::MissionDef::load_campaign(missions_dir, script)
            .unwrap_or_else(|e| panic!("load campaign {script}: {e:#}"));
        let m_path = baked_dir.join(&def.map).join("manifest.ron");
        let m_text = std::fs::read_to_string(&m_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", m_path.display()));
        let mut manifest: crate::assets::Manifest = ron::from_str(&m_text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", m_path.display()));
        manifest.spawn_only_posts = false;
        for req in &def.layers {
            if let Some(idx) = crate::assets::resolve_campaign_layer_index(&manifest, req) {
                let layer = manifest.layers.remove(idx);
                manifest.regions.extend(layer.regions);
                manifest.instances.extend(layer.instances);
            }
        }
        let mut layout = crate::layout::MapLayout::from_mission_manifest(&manifest, false, true);
        layout.apply_mission(&def);
        let mut walk = crate::walk::WalkSim::new(1, Vec3::ZERO, None);
        walk.add_posts(&layout);
        let factions = [
            def.teams[0].faction.clone(),
            def.teams[1].faction.clone(),
        ];
        let space = def
            .space_assault
            .as_ref()
            .filter(|_| def.mode == "ass" || (def.is_campaign() && def.map.contains("_spa")))
            .map(|s| (s, 180));
        let _ = crate::vehicle_pads::install_for_mission(
            &mut walk.sim,
            &manifest,
            &baked_dir.join(&def.map),
            &baked_dir.join("classes"),
            &factions,
            space,
            &def.class_load_order,
        );
        let u = &walk.sim.units[walk.player.index as usize];
        let base_uc = core::unit::UnitClass {
            consts: u.spec.consts,
            max_health: u.health.max,
            health_type: u.spec.health_type,
            weapon_class: u.weapon_class,
            total_clips: 6,
            weapon_slot: u.weapon_slot,
            slots: u.spec.slots.clone(),
            jet: false,
            capture_posts: true,
            drops: Default::default(),
            chunk: None,
            droideka: None,
            min: 1,
            max: None,
            unlock_points: 0,
        };
        for t in 0..2 {
            if !def.teams[t].classes.is_empty() {
                walk.sim.classes[t] = def.teams[t]
                    .classes
                    .iter()
                    .map(|(_, min, max)| {
                        let mut uc = base_uc.clone();
                        uc.min = *min;
                        uc.max = *max;
                        uc
                    })
                    .collect();
                walk.sim.class_enabled[t] = def.teams[t]
                    .classes
                    .iter()
                    .map(|(name, _, _)| def.teams[t].is_class_initially_enabled(name))
                    .collect();
            }
            walk.sim
                .set_ai_spawn_allowed(t as u8, def.teams[t].allows_ai_spawn_initially());
        }
        let spec = def
            .campaign
            .clone()
            .unwrap_or_else(|| panic!("{script} missing campaign spec"));
        let saved = CampaignRuntime::kill_startup_posts(&mut walk.sim, &def.killed);
        let mut rt = CampaignRuntime::new(
            spec,
            &layout,
            &manifest,
            &walk.post_keys,
            saved,
            &walk.sim,
        );
        rt.set_team_rosters(
            std::array::from_fn(|t| {
                def.teams[t]
                    .classes
                    .iter()
                    .map(|(c, _, _)| c.clone())
                    .collect()
            }),
            [def.teams[0].units, def.teams[1].units],
        );
        rt.start(&mut walk.sim);
        walk.step_tick(&[core::sim::PadState::default()]);
        (def, walk, rt)
    }

    #[test]
    fn geo1c_c_enforces_initial_cp_class_and_ai_rules_then_unlocks_by_stage() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let missions_dir = root.join("missions");
        let baked_dir = crate::assets::baked_root();
        if !missions_dir.join("campaign/bf2_geo1c_c.ron").is_file()
            || !baked_dir.join("bf2_geo1/manifest.ron").is_file()
        {
            eprintln!("SKIP: geo1c_c mission and baked assets required");
            return;
        }

        let (def, mut walk, mut rt) =
            setup_baked_campaign(&missions_dir, &baked_dir, "geo1c_c");
        assert!(!def.allows_side_select());
        assert_eq!(def.initial_player_team(), 0);
        assert_eq!(
            def.teams[0].class_whitelist.as_deref(),
            Some(&["rep_inf_ep2_rifleman".to_string()][..])
        );
        assert_eq!(def.teams[0].allow_ai_spawn, Some(false));
        assert!(!def.teams[0].allows_ai_spawn_initially());

        let player_posts = walk.sim.player_spawn_posts(0);
        assert_eq!(player_posts.len(), 1);
        let (core::unit::SpawnPost::Post(cp_idx), _) = player_posts[0] else {
            panic!("expected SpawnPost::Post, got {:?}", player_posts[0]);
        };
        let cp_idx = usize::from(cp_idx);
        assert!(walk.sim.cps[cp_idx].def.name.eq_ignore_ascii_case("c_cp6"));
        assert!(!walk.sim.cps[cp_idx].killed && walk.sim.cps[cp_idx].visible);

        assert!(!walk.sim.ai_spawn_allowed[0]);
        assert!(walk.sim.is_class_enabled(0, 0));
        for ci in 1..def.teams[0].classes.len() {
            assert!(!walk.sim.is_class_enabled(0, ci));
        }

        let obj2_idx = rt
            .spec
            .stages
            .iter()
            .position(|s| s.name == "Objective2")
            .expect("Objective2 stage");
        rt.enter_stage(obj2_idx, &mut walk.sim);
        assert!(walk.sim.ai_spawn_allowed[0]);
        let cis_spawn1 = walk
            .sim
            .cps
            .iter()
            .find(|cp| cp.def.name.eq_ignore_ascii_case("cis_spawn1"))
            .expect("cis_spawn1 CP");
        assert_eq!(cis_spawn1.owner, Some(1));
        assert!(!cis_spawn1.visible);

        let eng_idx = def.teams[0]
            .classes
            .iter()
            .position(|(c, _, _)| c == "rep_inf_ep2_engineer")
            .expect("engineer class index");
        assert!(!walk.sim.is_class_enabled(0, eng_idx));
        let obj6a_idx = rt
            .spec
            .stages
            .iter()
            .position(|s| s.name == "Objective6a")
            .expect("Objective6a stage");
        rt.enter_stage(obj6a_idx, &mut walk.sim);
        assert!(walk.sim.is_class_enabled(0, eng_idx));
    }

    #[test]
    fn all_18_campaign_missions_load_with_valid_player_spawn_post() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let missions_dir = root.join("missions");
        let baked_dir = crate::assets::baked_root();
        if !missions_dir.join("campaign/bf2_geo1c_c.ron").is_file()
            || !baked_dir.join("bf2_geo1/manifest.ron").is_file()
        {
            eprintln!("SKIP: campaign missions and baked assets required");
            return;
        }

        for m in ui::campaign::RISE_OF_THE_EMPIRE {
            let (def, walk, _rt) =
                setup_baked_campaign(&missions_dir, &baked_dir, m.script);
            assert!(!def.allows_side_select(), "{} side select", m.script);
            assert_eq!(def.initial_player_team(), 0, "{} team", m.script);
            assert!(
                !walk.sim.player_spawn_posts(def.initial_player_team()).is_empty(),
                "{} ({}) has no player spawn posts",
                m.script,
                def.map
            );
        }
    }
}
