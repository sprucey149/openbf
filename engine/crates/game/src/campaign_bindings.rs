//! The engine bindings the translated mission scripts call (BF2's
//! `ScriptCB_*` / mission API, registered by the PS2 ELF's Lua binding
//! table and documented by the PC mod tools' Lua headers): each reads or
//! changes the sim, or queues something for the HUD.
//!
//! Characters are player slots (a number: the sim unit index). A native
//! team's members that never spawned are placeholders until
//! `SpawnCharacter` gives them a slot. Objects are the sim's posts,
//! stations, vehicles, units, the script's flags, or the placed entities.
//! BF2 teams are 1-based (sim team + 1); 0 is neutral.

use crate::campaign_runtime::{
    char_value, reinforcement_count, unit_world_pos, Ctx, EventKind, Filter, FlagEnt, Handler,
    MarkTarget, Marker, ObjectiveLine, Popup, Timer, World,
};
use crate::campaign_script::{num_text, Host, Interp, Ret, ScriptError, Ud, Value};
use crate::classdb::fnv1a_lowercase;
use core::fmath::FloatExt;
use core::region::Region;
use core::sim::{CtfState, Sim};
use core::unit::Controller;
use glam::Vec3;

/// The bindings these scripts call that do something here.
const BOUND: &[&str] = &[
    "AcceptHero",
    "SetHeroClass",
    "BatchChangeTeams",
    "SelectCharacterClass",
    "ActivateObjective",
    "ActivateRegion",
    "AddDeathRegion",
    "AddMissionHint",
    "AddMissionObjective",
    "AddReinforcements",
    "AddTeamPoints",
    "AddUnitClass",
    "AICanCaptureCP",
    "AllowAISpawn",
    "AudioStreamComplete",
    "BlockPlanningGraphArcs",
    "BroadcastVoiceOver",
    "CanCharacterInteractWithFlag",
    "CompleteObjective",
    "CreateEntity",
    "CreateTimer",
    "DeactivateRegion",
    "DeleteEntity",
    "DestroyTimer",
    "DisableBarriers",
    "EnableBarriers",
    "EnableBuildingLockOn",
    "EnableLockOn",
    "EnableSPHeroRules",
    "EnableSPScriptedHeroes",
    "ScriptCB_EnableCommandPostVO",
    "FindTimer",
    "GetCharacterClass",
    "GetCharacterTeam",
    "GetCharacterUnit",
    "GetCommandPostBleedValue",
    "GetEntityClass",
    "GetEntityClassPtr",
    "GetEntityName",
    "GetFlagCarrier",
    "GetNumTeamMembersAlive",
    "GetObjectHealth",
    "GetObjectLastHitWeaponClass",
    "GetObjectPtr",
    "GetObjectShield",
    "GetObjectTeam",
    "GetPathPoint",
    "GetRegion",
    "GetRegionName",
    "GetReinforcementCount",
    "GetTeamMember",
    "GetTeamPoints",
    "GetTeamSize",
    "GetTimerValue",
    "GetWorldFilename",
    "IsCampaign",
    "IsCharacterHuman",
    "IsCharacterInRegion",
    "IsMultiplayerGametype",
    "IsObjectAlive",
    "KillObject",
    "MapAddClassMarker",
    "MapAddEntityMarker",
    "MapAddRegionMarker",
    "MapHideCommandPosts",
    "MapRemoveClassMarker",
    "MapRemoveEntityMarker",
    "MapRemoveRegionMarker",
    "MissionDefeat",
    "MissionVictory",
    "OpenAudioStream",
    "AudioStreamAppendSegments",
    "RemoveRegion",
    "RespawnObject",
    "PauseAnimation",
    "PlayAnimation",
    "PlayAnimationFromTo",
    "RewindAnimation",
    "SetAnimationStartPoint",
    "ScriptCB_GetMissionTime",
    "ScriptCB_IsMissionSetupSaved",
    "ScriptCB_PlayInGameMovie",
    "SetMissionEndMovie",
    "AddAIGoal",
    "DeleteAIGoal",
    "ClearAIGoals",
    "ScriptCB_PlayInGameMusic",
    "ScriptCB_SndPlaySound",
    "ScriptCB_StopInGameMusic",
    "SetObjectTeam",
    "SetProperty",
    "SetReinforcementCount",
    "SetSpawnDelay",
    "SetSpawnDelayTeam",
    "SetTeamAsEnemy",
    "SetTeamAsFriend",
    "SetTeamPoints",
    "SetAIDamageThreshold",
    "SetAIDifficulty",
    "ForceAIOutOfVehicles",
    "SetTimerRate",
    "SetTimerValue",
    "SetUnitCount",
    "ShowMessageText",
    "ShowObjectiveTextPopup",
    "ShowSelectionTextPopup",
    "ShowTeamPoints",
    "ShowTimer",
    "SpawnCharacter",
    "StartTimer",
    "StopTimer",
    "UnblockPlanningGraphArcs",
    "UnlockHeroForTeam",
];

/// Bindings with nothing to do in this port: loading, memory, rendering,
/// sound mixing, camera shots, the AI planner's goals and tuning, the
/// gameplay-option queries the campaign does not read.
/// PROVISIONAL: `EntityFlyerTakeOff`/`InitAsLanded` play nothing. The world
/// animation bindings (`PlayAnimation`, ...) drive `crate::world_anim`.
/// `ShowPopup` is bound to `0x18e920` (the table entry at `0x5f9df0`),
/// `jr ra; move v0,zero`: it does nothing on the PS2.
const IGNORED: &[&str] = &[
    "ShowPopup",
    "AddAssaultDestroyPoints",
    "AddCameraShot",
    "AddFlagCapturePoints",
    "AddHeroClass",
    "AddLandingRegion",
    "AddWalkerType",
    "AISnipeSuitabilityDist",
    "BeginScreenTransition",
    "ClearWalkers",
    "DisableAIAutoBalance",
    "EntityFlyerInitAsLanded",
    "EntityFlyerTakeOff",
    "FillAsteroidPath",
    "FillAsteroidRegion",
    "ReadDataFile",
    "ScaleSoundParameter",
    "ScriptCB_DisableFlyerShadows",
    "ScriptCB_LoadMissionSetup",
    "ScriptCB_SetControlMode",
    "ScriptCB_SetDopplerFactor",
    "ScriptCB_SetFunctionIdForAnalogId",
    "ScriptCB_SetFunctionIdForButtonId",
    "ScriptCB_SetGameRules",
    "ScriptCB_SetNumBots",
    "ScriptCB_SetSpawnDisplayGain",
    "ScriptCB_SetUberScoreLimit",
    "ScriptCB_SetYAxisFlip",
    "ScriptCB_ShowHuntScoreLimit",
    "SetAIVehicleNotifyRadius",
    "SetAIViewMultiplier",
    "SetAllowBlindJetJumps",
    "SetAttackingTeam",
    "SetBirdType",
    "SetBleedingVoiceOver",
    "SetBleedRate",
    "SetClassProperty",
    "SetDefeatMusic",
    "SetDefeatTimer",
    "SetDefenderSnipeRange",
    "SetDenseEnvironment",
    "SetFishType",
    "SetFlagGameplayType",
    "SetGroundFlyerMap",
    "SetLowReinforcementsVoiceOver",
    "SetMapNorthAngle",
    "SetMaxCollisionDistance",
    "SetMaxFlyHeight",
    "SetMaxPlayerFlyHeight",
    "SetMemoryPoolSize",
    "SetMinFlyHeight",
    "SetMinPlayerFlyHeight",
    "SetNumBirdTypes",
    "SetNumFishTypes",
    "SetOutOfBoundsVoiceOver",
    "SetParticleLODBias",
    "SetPS2ModelMemory",
    "SetSoundEffect",
    "SetTeamAggressiveness",
    "SetTeamIcon",
    "SetTeamName",
    "SetVictoryMusic",
    "SetVictoryTimer",
    "SetWorldExtents",
    "StealArtistHeap",
];

/// Game-option queries: the campaign plays with no game-option limits.
const ZERO: &[&str] = &[
    "ScriptCB_GetAssaultScoreLimit",
    "ScriptCB_GetASSNumBots",
    "ScriptCB_GetCONMaxTimeLimit",
    "ScriptCB_GetCONNumBots",
    "ScriptCB_GetControlMode",
    "ScriptCB_GetCTFCaptureLimit",
    "ScriptCB_GetCTFMaxTimeLimit",
    "ScriptCB_GetCTFNumBots",
    "ScriptCB_GetHuntMaxTimeLimit",
    "ScriptCB_GetHuntScoreLimit",
    "ScriptCB_GetUberScoreLimit",
    "ScriptCB_GetYAxisFlip",
];

fn is_handler_binding(name: &str) -> bool {
    handler_kind(name).is_some()
}

/// `On<Kind>[Name|Class|Team]` → (kind, what its second argument names).
fn handler_kind(name: &str) -> Option<(EventKind, &'static str)> {
    let rest = name.strip_prefix("On")?;
    for suffix in ["Name", "Class", "Team"] {
        if let Some(k) = rest.strip_suffix(suffix).and_then(EventKind::parse) {
            return Some((k, suffix));
        }
    }
    EventKind::parse(rest).map(|k| (k, ""))
}

fn release_kind(name: &str) -> Option<EventKind> {
    EventKind::parse(name.strip_prefix("Release")?)
}

fn nothing() -> Ret {
    Ok(Vec::new())
}

fn one(v: Value) -> Ret {
    Ok(vec![v])
}

fn arg(args: &[Value], i: usize) -> &Value {
    args.get(i).unwrap_or(&Value::Nil)
}

fn num(args: &[Value], i: usize) -> Option<f32> {
    arg(args, i).num()
}

/// A script value as `SetProperty` text: numbers without a trailing `.0`.
fn text(v: &Value) -> String {
    match v {
        Value::Num(n) => num_text(*n),
        Value::Str(s) => s.to_string(),
        Value::Bool(b) => if *b { "1" } else { "0" }.into(),
        _ => String::new(),
    }
}

/// `FUN_00191d58`, `AddAIGoal(team, name, weight[, target])`: a BF2 team
/// under 8, the name's goal type, and for Destroy, Defend and Follow the
/// object or player the fourth argument names (`FUN_001935a8`, else a
/// player index). CtfOffense is made only when its fourth argument names a
/// region (`FUN_00538108`); the goal keeps the region's centre and the flag
/// its fifth argument names (`FUN_002e8ab8` +0x30, +0x20). The handle is the goal's id (the PS2's goal
/// pointer, which `DeleteAIGoal` takes back).
fn add_ai_goal(w: &World, sim: &mut Sim, a: &[Value]) -> Option<u32> {
    use core::goals::{GoalKind, GoalTarget};
    let team = bf2_team(arg(a, 0)).filter(|t| *t < 8)?.checked_sub(1)?;
    let kind = GoalKind::named(arg(a, 1).as_str()?)?;
    let weight = arg(a, 2).num()?;
    if kind == GoalKind::CtfOffense {
        let r = arg(a, 3).as_str().and_then(|r| w.region_id(r))?;
        let region = w.regions.get(r as usize)?.1.center;
        let flag = match w.resolve(sim, arg(a, 4)) {
            Some(Ud::Flag(f)) => w.flags.get(usize::from(f)).map(|e| e.ctf as u16),
            _ => None,
        };
        return sim
            .ai_goals
            .add(team, kind, weight, Some(GoalTarget::Ctf { region, flag }));
    }
    let target = w.resolve(sim, arg(a, 3)).and_then(|o| match o {
        Ud::Post(i) => Some(GoalTarget::Post(u32::from(i))),
        Ud::Unit(i, gen) => Some(GoalTarget::Unit(core::unit::UnitHandle { index: i, gen })),
        Ud::Station(si) => Some(GoalTarget::Station(u16::from(si))),
        o => w.obj_pos(sim, o).map(GoalTarget::Point),
    });
    sim.ai_goals.add(team, kind, weight, target)
}

fn bf2_team(v: &Value) -> Option<u8> {
    v.num().filter(|n| *n >= 0.0).map(|n| n as u8)
}

/// BF2 team → sim team.
fn sim_team(v: &Value) -> Option<u8> {
    bf2_team(v)?.checked_sub(1)
}

impl Host for Ctx<'_> {
    fn known(&self, name: &str) -> bool {
        BOUND.contains(&name)
            || IGNORED.contains(&name)
            || ZERO.contains(&name)
            || is_handler_binding(name)
            || release_kind(name).is_some()
    }

    fn call(&mut self, it: &mut Interp, name: &str, args: &[Value]) -> Ret {
        if self.w.setup
            && !is_query(name)
            && !is_stream_routing(name)
            && name != "SetSpawnDelay"
            && name != "SetSpawnDelayTeam"
            && !(name == "SetReinforcementCount"
                && sim_team(arg(args, 0)).is_some_and(|t| t >= 2))
        {
            return nothing();
        }
        if IGNORED.contains(&name) {
            return nothing();
        }
        if ZERO.contains(&name) {
            return one(Value::Num(0.0));
        }
        if let Some((kind, by)) = handler_kind(name) {
            return self.register(kind, by, args);
        }
        if let Some(kind) = release_kind(name) {
            if let Value::Ud(Ud::Handler(h)) = arg(args, 0) {
                if let Some(h) = self
                    .w
                    .handlers
                    .get_mut(*h as usize)
                    .filter(|h| h.kind == kind)
                {
                    h.released = true;
                }
            }
            return nothing();
        }
        self.bound(it, name, args)
    }
}

/// The stream-slot calls `ScriptInit` makes: the loader applies none of
/// them, so they run during setup ([`World::stream_slots`]).
fn is_stream_routing(name: &str) -> bool {
    matches!(name, "OpenAudioStream" | "AudioStreamAppendSegments")
}

/// What may run while `ScriptInit` replays: the reads, never a change.
fn is_query(name: &str) -> bool {
    name.starts_with("Get")
        || name.starts_with("Is")
        || name.starts_with("ScriptCB_Get")
        || name.starts_with("ScriptCB_Is")
        || name == "FindTimer"
}

impl Ctx<'_> {
    fn register(&mut self, kind: EventKind, by: &str, args: &[Value]) -> Ret {
        let func = arg(args, 0).clone();
        let key = arg(args, 1);
        let filter = match (kind, by) {
            (_, "Class") => key
                .as_str()
                .map_or(Filter::Any, |c| Filter::Class(c.to_ascii_lowercase())),
            (_, "Team") => bf2_team(key).map_or(Filter::Any, Filter::Team),
            (EventKind::EnterRegion | EventKind::LeaveRegion, _) => match key {
                Value::Ud(Ud::Region(r)) => Filter::Region(*r),
                v => v
                    .as_str()
                    .and_then(|n| self.w.region_id(n))
                    .map_or(Filter::Region(u32::MAX), Filter::Region),
            },
            // The flag events take no filter on the PS2: the binding stores
            // -1 and never reads its second argument (`FUN_00481b60`,
            // `FUN_004847e8`). cor1c registers `"holocron"` for flags named
            // `holocron1` / `holocron2`; a name filter never fired there and
            // Objective 5 never marked the Jedi carrying the holocron.
            (EventKind::FlagPickUp | EventKind::FlagDrop | EventKind::FlagReset, _) => Filter::Any,
            (EventKind::TimerElapse, _) => match key {
                Value::Ud(Ud::Timer(t)) => Filter::Timer(*t),
                Value::Str(s) => self
                    .w
                    .find_timer(s)
                    .map_or(Filter::Timer(u32::MAX), Filter::Timer),
                _ => Filter::Any,
            },
            _ => match key {
                Value::Str(s) => Filter::Name(s.to_string()),
                Value::Ud(u) => Filter::Name(self.w.obj_name(self.sim, *u)),
                _ => Filter::Any,
            },
        };
        self.w.handlers.push(Handler {
            kind,
            filter,
            func,
            released: false,
        });
        one(Value::Ud(Ud::Handler(self.w.handlers.len() as u32 - 1)))
    }

    fn bound(&mut self, it: &mut Interp, name: &str, a: &[Value]) -> Ret {
        let sim = &mut *self.sim;
        let w = &mut *self.w;
        match name {
            "IsCampaign" => one(Value::Bool(true)),
            "IsMultiplayerGametype" => one(Value::Bool(false)),
            "ScriptCB_IsMissionSetupSaved" => one(Value::Bool(false)),
            "GetWorldFilename" => one(Value::str(&w.map)),
            "ScriptCB_GetMissionTime" => one(Value::Num(w.time)),

            "ShowMessageText" => {
                if w.for_player(arg(a, 1)) {
                    if let Some(t) = arg(a, 0).as_str() {
                        w.messages.push(t.to_owned());
                    }
                }
                nothing()
            }
            "BroadcastVoiceOver" => {
                if w.for_player(arg(a, 1)) {
                    if let Some(t) = arg(a, 0).as_str() {
                        w.voice.push(t.to_owned());
                    }
                }
                nothing()
            }
            // B2 `FUN_0019e458`: one argument plays the property 2-D with
            // the queue flag set (`FUN_0043ceb0(h, 0, 0, _, 1)`), so a
            // stream line waits its turn on its stream's slot
            // (re-announcer.md §1 step 2). The battle routes it by kind.
            "ScriptCB_SndPlaySound" => {
                if let Some(t) = arg(a, 0).as_str() {
                    w.sounds.push(t.to_owned());
                }
                nothing()
            }
            "ScriptCB_PlayInGameMusic" => {
                // FUN_001a04a0 (45241): the name, then the replay delay when
                // the call has a second argument (lua_gettop >= 2), else 15.
                if let Some(name) = arg(a, 0).as_str() {
                    let replay = if a.len() < 2 {
                        crate::music::IN_GAME_REPLAY
                    } else {
                        num(a, 1).unwrap_or(0.0)
                    };
                    w.music.push(crate::music::InGameMusic::Play {
                        name: name.to_owned(),
                        replay,
                    });
                }
                nothing()
            }
            "ScriptCB_StopInGameMusic" => {
                w.music.push(crate::music::InGameMusic::Stop);
                nothing()
            }
            "ScriptCB_PlayInGameMovie" => {
                if let Some(t) = arg(a, 1).as_str().or(arg(a, 0).as_str()) {
                    w.movies.push(t.to_owned());
                }
                nothing()
            }
            "SetMissionEndMovie" => {
                w.end_movie = arg(a, 1).as_str().map(str::to_owned);
                nothing()
            }
            "AddAIGoal" => {
                one(add_ai_goal(w, sim, a).map_or(Value::Nil, |id| Value::Num(id as f32)))
            }
            "DeleteAIGoal" => {
                if let Some(id) = arg(a, 0).num().filter(|n| *n >= 1.0) {
                    sim.ai_goals.delete(id as u32);
                }
                nothing()
            }
            "ClearAIGoals" => {
                if let Some(t) = sim_team(arg(a, 0)) {
                    sim.ai_goals.clear(t);
                }
                nothing()
            }
            "OpenAudioStream" => {
                let slot = arg(a, 1).as_str().map_or(0, fnv1a_lowercase);
                if slot != 0 {
                    w.stream_slots.entry(slot).or_insert(slot);
                }
                one(Value::Ud(Ud::Stream(slot)))
            }
            "AudioStreamAppendSegments" => {
                if let (Some(name), Value::Ud(Ud::Stream(slot))) = (arg(a, 1).as_str(), arg(a, 2)) {
                    if *slot != 0 {
                        w.stream_slots.entry(fnv1a_lowercase(name)).or_insert(*slot);
                    }
                }
                nothing()
            }
            "AudioStreamComplete" => {
                one(Value::Num(if w.voice_playing { 0.0 } else { 1.0 }))
            }

            "AddMissionObjective" => {
                let team = bf2_team(arg(a, 0)).unwrap_or(0);
                if let Some(t) = arg(a, 1).as_str() {
                    w.objectives.push(ObjectiveLine {
                        team,
                        text: t.to_owned(),
                        popup: arg(a, 2).as_str().map(str::to_owned),
                        state: 1,
                    });
                }
                nothing()
            }
            "ActivateObjective" => {
                w.set_objective(arg(a, 0), 2);
                nothing()
            }
            "CompleteObjective" => {
                w.set_objective(arg(a, 0), 3);
                nothing()
            }
            "AddMissionHint" => {
                if let Some(t) = arg(a, 0).as_str() {
                    w.hints.push(t.to_owned());
                }
                nothing()
            }
            // `FUN_0018e928` → `FUN_0038c7a0(2, text)` (behind the option
            // flag at `FUN_00436850(0)+0x38`), `FUN_0018e988` → kind 3. A
            // popup already up is closed first (`FUN_0038ce58` →
            // `FUN_0038cb28`): the new one replaces it.
            "ShowObjectiveTextPopup" | "ShowSelectionTextPopup" => {
                if let Some(t) = arg(a, 0).as_str() {
                    let t = t.to_owned();
                    w.popups.clear();
                    w.popups.push_back(match name {
                        "ShowObjectiveTextPopup" => Popup::Objective(t),
                        _ => Popup::Selection(t),
                    });
                }
                nothing()
            }
            "ShowTeamPoints" => {
                if let Some(t) = sim_team(arg(a, 0)).filter(|t| *t < 2) {
                    w.show_points[usize::from(t)] = a.get(1).is_none_or(Value::truthy);
                }
                nothing()
            }
            "GetTeamPoints" => one(Value::Num(
                sim_team(arg(a, 0))
                    .filter(|t| *t < 2)
                    .map_or(0, |t| w.team_points[usize::from(t)]) as f32,
            )),
            "SetTeamPoints" | "AddTeamPoints" => {
                if let (Some(t), Some(n)) = (sim_team(arg(a, 0)).filter(|t| *t < 2), num(a, 1)) {
                    let p = &mut w.team_points[usize::from(t)];
                    *p = if name == "SetTeamPoints" {
                        n as i32
                    } else {
                        *p + n as i32
                    };
                }
                nothing()
            }
            "MapHideCommandPosts" => {
                w.hide_cps = a.first().is_none_or(Value::truthy);
                nothing()
            }

            "CreateTimer" => {
                let name = arg(a, 0).as_str().unwrap_or_default().to_owned();
                w.timers.push(Timer {
                    name,
                    value: 0.0,
                    rate: 1.0,
                    running: false,
                    alive: true,
                });
                one(Value::Ud(Ud::Timer(w.timers.len() as u32 - 1)))
            }
            "FindTimer" => one(arg(a, 0)
                .as_str()
                .and_then(|n| w.find_timer(n))
                .map_or(Value::Nil, |t| Value::Ud(Ud::Timer(t)))),
            "DestroyTimer" | "StartTimer" | "StopTimer" | "SetTimerValue" | "SetTimerRate"
            | "GetTimerValue" => {
                let Some(i) = w.timer_of(arg(a, 0)) else {
                    return if name == "GetTimerValue" {
                        one(Value::Nil)
                    } else {
                        nothing()
                    };
                };
                let t = &mut w.timers[i as usize];
                match name {
                    "DestroyTimer" => {
                        t.alive = false;
                        t.running = false;
                        if w.shown_timer == Some(i) {
                            w.shown_timer = None;
                        }
                    }
                    "StartTimer" => t.running = true,
                    "StopTimer" => t.running = false,
                    "SetTimerValue" => t.value = num(a, 1).unwrap_or(0.0),
                    "SetTimerRate" => t.rate = num(a, 1).unwrap_or(1.0),
                    _ => return one(Value::Num(t.value)),
                }
                nothing()
            }
            "ShowTimer" => {
                w.shown_timer = w.timer_of(arg(a, 0));
                nothing()
            }

            "ActivateRegion" | "DeactivateRegion" => {
                if let Some(r) = w.region_of(arg(a, 0)) {
                    if name == "ActivateRegion" {
                        w.active_regions.insert(r);
                    } else {
                        w.active_regions.remove(&r);
                        w.clear_inside(r);
                    }
                }
                nothing()
            }
            "GetRegion" => one(w
                .region_of(arg(a, 0))
                .map_or(Value::Nil, |r| Value::Ud(Ud::Region(r)))),
            "GetRegionName" => one(w
                .region_of(arg(a, 0))
                .and_then(|r| w.regions.get(r as usize))
                .map_or(Value::Nil, |(n, _)| Value::str(n))),
            "IsCharacterInRegion" => {
                let inside = match (w.char_of(arg(a, 0)), w.region_of(arg(a, 1))) {
                    (Some(c), Some(r)) => w
                        .char_unit(sim, c)
                        .and_then(|h| sim.unit(h))
                        .is_some_and(|u| w.regions[r as usize].1.contains(unit_world_pos(sim, u))),
                    _ => false,
                };
                one(Value::Bool(inside))
            }
            "AddDeathRegion" => {
                if let Some(r) = w.region_of(arg(a, 0)) {
                    let region = w.regions[r as usize].1;
                    if !sim.death_regions.contains(&region) {
                        sim.death_regions.push(region);
                    }
                }
                nothing()
            }
            "RemoveRegion" => {
                if let Some(r) = w.region_of(arg(a, 0)) {
                    let region = w.regions[r as usize].1;
                    sim.death_regions.retain(|d| *d != region);
                    w.active_regions.remove(&r);
                    w.clear_inside(r);
                }
                nothing()
            }
            "GetPathPoint" => {
                let p = arg(a, 0).as_str().and_then(|n| w.path(n));
                let i = num(a, 1).unwrap_or(0.0).fmax(0.0) as u32;
                one(p.map_or(Value::Nil, |p| Value::Ud(Ud::Point(p as u32, i))))
            }

            "MapAddEntityMarker" | "MapAddClassMarker" | "MapAddRegionMarker" => {
                let target = match name {
                    "MapAddEntityMarker" => w.resolve(sim, arg(a, 0)).map(MarkTarget::Obj),
                    "MapAddClassMarker" => arg(a, 0)
                        .as_str()
                        .map(|c| MarkTarget::Class(c.to_ascii_lowercase())),
                    _ => w.region_of(arg(a, 0)).map(MarkTarget::Region),
                };
                if let Some(target) = target {
                    let team = bf2_team(arg(a, 3)).unwrap_or(0);
                    w.markers
                        .retain(|m| !(m.target == target && m.team == team));
                    w.markers.push(Marker {
                        target,
                        icon: arg(a, 1).as_str().unwrap_or_default().to_owned(),
                        scale: num(a, 2).unwrap_or(1.0),
                        team,
                        // The sixth argument, true when absent
                        // (`FUN_0018eda8`, `FUN_0018eb58`, `FUN_0018eff8`).
                        pulse: a.len() <= 5 || arg(a, 5).truthy(),
                    });
                }
                nothing()
            }
            "MapRemoveEntityMarker" | "MapRemoveClassMarker" | "MapRemoveRegionMarker" => {
                let target = match name {
                    "MapRemoveEntityMarker" => w.resolve(sim, arg(a, 0)).map(MarkTarget::Obj),
                    "MapRemoveClassMarker" => arg(a, 0)
                        .as_str()
                        .map(|c| MarkTarget::Class(c.to_ascii_lowercase())),
                    _ => w.region_of(arg(a, 0)).map(MarkTarget::Region),
                };
                let team = bf2_team(arg(a, 1));
                if let Some(target) = target {
                    w.markers
                        .retain(|m| !(m.target == target && team.is_none_or(|t| t == m.team)));
                }
                nothing()
            }

            "GetCharacterUnit" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| w.char_unit(sim, c))
                .map_or(Value::Nil, |h| Value::Ud(Ud::Unit(h.index, h.gen)))),
            "GetCharacterTeam" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| w.char_team(sim, c))
                .map_or(Value::Nil, |t| Value::Num(f32::from(t) + 1.0))),
            "IsCharacterHuman" => one(Value::Bool(
                w.char_of(arg(a, 0))
                    .and_then(|c| sim.units.get(w.char_slot(c) as usize))
                    .is_some_and(|u| u.is_player),
            )),
            "GetCharacterClass" => one(w
                .char_of(arg(a, 0))
                .and_then(|c| sim.units.get(w.char_slot(c) as usize))
                .map_or(Value::Nil, |u| {
                    let ci = usize::from(u.spec.class);
                    let idx = w
                        .class_order
                        .get(usize::from(u.team))
                        .and_then(|o| o.iter().position(|c| *c == ci))
                        .unwrap_or(ci);
                    Value::Num(idx as f32)
                })),
            "CanCharacterInteractWithFlag" => one(Value::Bool(
                w.char_of(arg(a, 0))
                    .and_then(|c| w.char_unit(sim, c))
                    .is_some(),
            )),
            "GetTeamSize" => one(Value::Num(
                sim_team(arg(a, 0)).map_or(0, |t| w.team_size(t)) as f32,
            )),
            "GetTeamMember" => {
                let (Some(t), Some(i)) = (sim_team(arg(a, 0)), num(a, 1)) else {
                    return one(Value::Nil);
                };
                one(w
                    .team_member(sim, t, i.fmax(0.0) as usize)
                    .map_or(Value::Nil, char_value))
            }
            "GetNumTeamMembersAlive" => one(Value::Num(sim_team(arg(a, 0)).map_or(0, |t| {
                sim.units.iter().filter(|u| u.alive && u.team == t).count()
            }) as f32)),
            // BF2's scripted hero path: `BatchChangeTeams(from, to, count)`
            // moves native members to a main army, and
            // `SelectCharacterClass(member, class)` gives one a hero kit.
            // Utapau's Objective 3a.OnComplete uses both to field Grievous
            // for the "Fight General Grievous" target objective.
            "BatchChangeTeams" => {
                if let (Some(from), Some(to), Some(n)) =
                    (bf2_team(arg(a, 0)), bf2_team(arg(a, 1)), num(a, 2))
                {
                    let from = from.saturating_sub(1);
                    let to = to.saturating_sub(1);
                    for i in 0..(n.max(0.0) as u32).min(1000) {
                        let c = PLACEHOLDER + u32::from(from) * 1000 + i;
                        w.scripted_chars.entry(c).or_default().team = Some(to);
                    }
                }
                nothing()
            }
            "SelectCharacterClass" => {
                if let Some(c) = w.char_of(arg(a, 0)) {
                    let class = if let Some(s) = arg(a, 1).as_str() {
                        Some(s.to_ascii_lowercase())
                    } else if let Some(idx) = num(a, 1).filter(|n| *n >= 0.0) {
                        let team = w
                            .scripted_chars
                            .get(&c)
                            .and_then(|s| s.team)
                            .or_else(|| w.char_team(sim, c));
                        team.and_then(|t| {
                            let ti = usize::from(t);
                            let ci = *w.class_order.get(ti)?.get(idx as usize)?;
                            w.team_classes
                                .get(ti)?
                                .get(ci)
                                .map(|s| s.to_ascii_lowercase())
                        })
                    } else {
                        None
                    };
                    if let Some(class) = class {
                        w.scripted_chars.entry(c).or_default().class = Some(class);
                    }
                }
                nothing()
            }
            "SpawnCharacter" => {
                let (Some(c), Some(pos)) = (w.char_of(arg(a, 0)), w.point_of(sim, arg(a, 1)))
                else {
                    return nothing();
                };
                w.spawn_character(sim, c, pos);
                nothing()
            }

            "GetObjectPtr" => one(arg(a, 0)
                .as_str()
                .and_then(|n| w.resolve_name(sim, n))
                .map_or(Value::Nil, Value::Ud)),
            "GetEntityName" => one(w
                .resolve(sim, arg(a, 0))
                .map_or(Value::Nil, |o| Value::str(&w.obj_name(sim, o)))),
            "GetEntityClass" => {
                let c = w.resolve(sim, arg(a, 0)).and_then(|o| w.obj_class(sim, o));
                one(c.map_or(Value::Nil, |c| Value::Ud(Ud::Class(w.class_id(&c)))))
            }
            "GetEntityClassPtr" => one(arg(a, 0)
                .as_str()
                .map_or(Value::Nil, |c| Value::Ud(Ud::Class(w.class_id(c))))),
            "GetObjectTeam" => one(Value::Num(
                w.resolve(sim, arg(a, 0)).map_or(0, |o| w.obj_team(sim, o)) as f32,
            )),
            "SetObjectTeam" => {
                let t = arg(a, 1).clone();
                w.set_property(sim, arg(a, 0), "team", &text(&t));
                nothing()
            }
            "IsObjectAlive" => one(Value::Bool(
                w.resolve(sim, arg(a, 0))
                    .is_some_and(|o| w.obj_alive(sim, o)),
            )),
            "GetObjectHealth" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| w.obj_health(sim, o))
                .map_or(Value::Nil, Value::Num)),
            "GetObjectShield" => {
                let (cur, max, add) = w
                    .resolve(sim, arg(a, 0))
                    .map_or((0.0, 0.0, 0.0), |o| w.obj_shield(sim, o));
                Ok(vec![Value::Num(cur), Value::Num(max), Value::Num(add)])
            }
            "EnableLockOn" | "EnableBuildingLockOn" => {
                let enable = !matches!(arg(a, 1), Value::Bool(false))
                    && num(a, 1).is_none_or(|n| n != 0.0);
                if let Some(Ud::Station(si)) = w.resolve(sim, arg(a, 0)) {
                    sim.set_station_lock_on(si, enable);
                }
                nothing()
            }
            "GetObjectLastHitWeaponClass" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| w.last_hit_weapon.iter().find(|(x, _)| *x == o))
                .map_or(Value::Nil, |(_, c)| Value::str(c))),
            "GetCommandPostBleedValue" => one(Value::Num(0.0)),
            "SetProperty" => {
                let key = text(arg(a, 1));
                let val = text(arg(a, 2));
                w.set_property(sim, arg(a, 0), &key, &val);
                nothing()
            }
            "KillObject" => {
                w.kill_object(sim, arg(a, 0), true);
                nothing()
            }
            "DeleteEntity" => {
                w.kill_object(sim, arg(a, 0), false);
                nothing()
            }
            "RespawnObject" => {
                w.respawn_object(sim, arg(a, 0));
                nothing()
            }
            "PlayAnimation" => {
                w.anims.play(&text(arg(a, 0)));
                nothing()
            }
            "PauseAnimation" => {
                w.anims.pause(&text(arg(a, 0)));
                nothing()
            }
            "RewindAnimation" => {
                w.anims.rewind(&text(arg(a, 0)));
                nothing()
            }
            "PlayAnimationFromTo" => {
                if let (Some(from), Some(to)) = (num(a, 1), num(a, 2)) {
                    w.anims.play_from_to(&text(arg(a, 0)), from, to);
                }
                nothing()
            }
            "SetAnimationStartPoint" => {
                w.anims.set_start_point(&text(arg(a, 0)), sim);
                nothing()
            }
            "CreateEntity" => {
                let class = arg(a, 0).as_str().unwrap_or_default().to_ascii_lowercase();
                let name = arg(a, 2).as_str().unwrap_or_default().to_owned();
                let pos = w
                    .point_of(sim, arg(a, 1))
                    .map(|(p, _)| p)
                    .unwrap_or(Vec3::ZERO);
                one(w.create_entity(sim, &class, &name, pos))
            }
            "GetFlagCarrier" => one(w
                .resolve(sim, arg(a, 0))
                .and_then(|o| match o {
                    Ud::Flag(f) => w.flags.get(usize::from(f)),
                    _ => None,
                })
                .and_then(|f| sim.ctf.as_ref()?.flags.get(f.ctf)?.carrier())
                .map_or(Value::Nil, |h| char_value(h.index))),

            "GetReinforcementCount" => one(Value::Num(sim_team(arg(a, 0)).map_or(0.0, |t| {
                if t < 2 {
                    reinforcement_count(sim, usize::from(t)) as f32
                } else {
                    sim.native_reinforcements(t).map_or(-1.0, |n| n as f32)
                }
            }))),
            "SetReinforcementCount" | "AddReinforcements" => {
                if let (Some(t), Some(n)) = (sim_team(arg(a, 0)), num(a, 1)) {
                    if t < 2 {
                        let t = usize::from(t);
                        if name == "AddReinforcements" {
                            if !sim.battle.unlimited_teams[t] {
                                let cur = sim.battle.teams[t].reinforcements;
                                sim.battle.teams[t].reinforcements = (cur + n).fmax(0.0);
                            }
                        } else if n < 0.0 {
                            sim.battle.unlimited_teams[t] = true;
                        } else {
                            sim.battle.unlimited_teams[t] = false;
                            sim.battle.teams[t].reinforcements = n;
                        }
                    } else if name == "AddReinforcements" {
                        if let Some(cur) = sim.native_reinforcements(t) {
                            let next = (cur as f32 + n).fmax(0.0).round() as u32;
                            sim.set_native_reinforcements(t, Some(next));
                        }
                    } else if n < 0.0 {
                        sim.set_native_reinforcements(t, None);
                    } else {
                        sim.set_native_reinforcements(t, Some(n.fmax(0.0).round() as u32));
                    }
                }
                nothing()
            }
            "MissionVictory" | "MissionDefeat" => {
                let teams: Vec<u8> = match arg(a, 0) {
                    Value::Table(t) => it.array(*t).iter().filter_map(bf2_team).collect(),
                    v => bf2_team(v).into_iter().collect(),
                };
                let winner = match (name, teams.as_slice()) {
                    ("MissionVictory", [t]) => t.checked_sub(1),
                    ("MissionDefeat", [t]) => t.checked_sub(1).map(|t| if t == 0 { 1 } else { 0 }),
                    _ => Some(core::sim::OUTCOME_DRAW),
                };
                if sim.outcome.is_none() {
                    sim.outcome = winner;
                }
                nothing()
            }

            "AddUnitClass" => {
                let (Some(t), Some(class)) = (sim_team(arg(a, 0)), arg(a, 1).as_str()) else {
                    return one(Value::Nil);
                };
                one(w
                    .add_unit_class(sim, t, class, num(a, 2), num(a, 3))
                    .map_or(Value::Nil, |idx| Value::Num(idx as f32)))
            }
            "SetUnitCount" => {
                if let (Some(t), Some(n)) = (sim_team(arg(a, 0)), num(a, 1)) {
                    if t < 2 {
                        if !w.setup {
                            w.team_units[usize::from(t)] = n as u32;
                            w.pending_ai_fill = true;
                        }
                    } else {
                        w.native_units.insert(t, n as u32);
                    }
                }
                nothing()
            }
            "AllowAISpawn" => {
                if let Some(t) = sim_team(arg(a, 0)).filter(|t| *t < 2) {
                    let allow = arg(a, 1).truthy();
                    sim.set_ai_spawn_allowed(t, allow);
                    if allow {
                        w.pending_ai_fill = true;
                    }
                }
                nothing()
            }
            "SetSpawnDelay" => {
                // SWBFRNT2.ELF FUN_0018d348 (lines 32695-32722) -> FUN_0036bec0:
                // sets wave delay and release interval for main teams 1 and 2.
                if let (Some(delay), Some(interval)) = (num(a, 0), num(a, 1)) {
                    let delay = delay.fmax(0.0);
                    let interval = interval.fmax(0.0);
                    for t in 0..2 {
                        sim.waves[t].delay = delay;
                        sim.waves[t].interval = interval;
                    }
                }
                nothing()
            }
            "SetSpawnDelayTeam" => {
                // SWBFRNT2.ELF FUN_0018d3f8 (lines 32725-32747) -> FUN_0036bec0:
                // sets wave delay and release interval for one team.
                if let (Some(delay), Some(interval), Some(t)) = (
                    num(a, 0),
                    num(a, 1),
                    sim_team(arg(a, 2)).filter(|t| *t < 2),
                ) {
                    let delay = delay.fmax(0.0);
                    let interval = interval.fmax(0.0);
                    sim.waves[usize::from(t)].delay = delay;
                    sim.waves[usize::from(t)].interval = interval;
                }
                nothing()
            }
            "AICanCaptureCP" => {
                let cp = arg(a, 0).as_str().and_then(|n| w.find_cp(sim, n));
                if let (Some(ci), Some(t)) = (cp, sim_team(arg(a, 1)).filter(|t| *t < 8)) {
                    let bit = 1u8 << t;
                    let ban = &mut sim.cps[ci].ai_capture_ban;
                    *ban = if arg(a, 2).truthy() {
                        *ban & !bit
                    } else {
                        *ban | bit
                    };
                }
                nothing()
            }
            "SetTeamAsFriend" | "SetTeamAsEnemy" => {
                if let (Some(x), Some(y)) = (sim_team(arg(a, 0)), sim_team(arg(a, 1))) {
                    if x >= 2 || y >= 2 {
                        sim.set_team_relation(x, y, name == "SetTeamAsFriend");
                    }
                }
                nothing()
            }
            "SetHeroClass" => {
                if let (Some(t), Some(name)) =
                    (sim_team(arg(a, 0)).filter(|t| *t < 2), arg(a, 1).as_str())
                {
                    if let Some(index) = w.team_classes[usize::from(t)]
                        .iter()
                        .position(|c| c.eq_ignore_ascii_case(name))
                    {
                        if let Some(heroes) = sim.heroes.as_mut() {
                            heroes.classes[usize::from(t)] = Some(index as u8);
                        }
                    }
                }
                nothing()
            }
            "AcceptHero" => {
                if let Some(h) = w.char_of(arg(a, 0)).and_then(|c| w.char_unit(sim, c)) {
                    sim.accept_scripted_hero(h.index);
                }
                nothing()
            }
            "ScriptCB_EnableCommandPostVO" => {
                w.command_post_vo = num(a, 0).is_some_and(|n| n as i32 == 1);
                nothing()
            }
            "EnableSPHeroRules" | "EnableSPScriptedHeroes" => {
                if let Some(heroes) = sim.heroes.as_mut() {
                    heroes.rules.unlock = core::sim::heroes::HeroUnlock::Points(i32::MAX);
                    heroes.rules.respawn_ticks = None;
                    heroes.rules.ai_candidates = false;
                }
                nothing()
            }
            "UnlockHeroForTeam" => {
                if let (Some(t), Some(heroes)) =
                    (sim_team(arg(a, 0)).filter(|t| *t < 2), sim.heroes.as_mut())
                {
                    let h = &mut heroes.teams[usize::from(t)];
                    h.unlocked = true;
                    h.exhausted = false;
                    h.ready_at = 0;
                    h.declined.clear();
                }
                nothing()
            }
            "EnableBarriers" | "DisableBarriers" => {
                if let (Some(n), Some(nav)) = (arg(a, 0).as_str(), sim.nav.as_mut()) {
                    nav.set_barriers(fnv1a_lowercase(n), name == "EnableBarriers");
                }
                nothing()
            }
            "BlockPlanningGraphArcs" | "UnblockPlanningGraphArcs" => {
                if let Some(nav) = sim.nav.as_mut() {
                    let block = name == "BlockPlanningGraphArcs";
                    for v in a {
                        if let Some(idx) = v
                            .num()
                            .map(|n| n as i32)
                            .filter(|&i| (1..=8).contains(&i))
                        {
                            nav.block_group((idx - 1) as u32, block);
                        } else if let Some(s) = v.as_str() {
                            nav.block_arcs(&[fnv1a_lowercase(s)], block);
                        }
                    }
                }
                nothing()
            }
            "SetAIDifficulty" => {
                if let (Some(human_team), Some(others)) = (num(a, 0), num(a, 1)) {
                    let level = arg(a, 2).as_str().map(|s| match s.to_ascii_lowercase().as_str() {
                        "easy" => 1,
                        "medium" => 2,
                        "hard" => 3,
                        _ => 0,
                    });
                    self.sim
                        .mission_ai
                        .set_skill_delta(human_team as i32, others as i32, level);
                }
                nothing()
            }
            "ForceAIOutOfVehicles" => {
                if let Some(team) = sim_team(arg(a, 0)) {
                    sim.force_ai_out_of_vehicles(team, arg(a, 1).truthy());
                }
                nothing()
            }
            "SetAIDamageThreshold" => {
                if let (Some(Ud::Station(si)), Some(thresh)) =
                    (self.w.resolve(sim, arg(a, 0)), num(a, 1))
                {
                    if let Some(st) = sim.stations.get_mut(usize::from(si)) {
                        st.ai_damage_threshold = thresh.clamp(0.0, 1.0);
                    }
                }
                nothing()
            }
            _ => Err(ScriptError(format!("unbound engine call {name}"))),
        }
    }
}

impl World {
    /// A message or voice-over is for the player: no team given, or theirs.
    fn for_player(&self, team: &Value) -> bool {
        bf2_team(team).is_none_or(|t| t == 0 || t == self.player_team + 1)
    }

    /// `FUN_0038abd8(text, state)`: activating an objective the player's
    /// team holds that is not already active posts
    /// `player1.objectivesUpdated` 0.25 s later and its disable 6.0 s later
    /// (`0x3e800000`, `0x40c00000` through `FUN_003b0558`).
    fn set_objective(&mut self, text: &Value, state: u8) {
        let Some(text) = text.as_str() else {
            return;
        };
        let team = self.player_team + 1;
        let mut post = false;
        for o in self
            .objectives
            .iter_mut()
            .filter(|o| o.text.eq_ignore_ascii_case(text))
        {
            if state == 2 && o.state != 2 && o.team == team {
                post = true;
            }
            o.state = state;
        }
        if post {
            self.hud_events.push((0.25, "objectivesUpdated"));
            self.hud_events.push((6.0, "objectivesUpdated.disable"));
        }
    }

    pub(crate) fn find_timer(&self, name: &str) -> Option<u32> {
        self.timers
            .iter()
            .position(|t| t.alive && t.name.eq_ignore_ascii_case(name))
            .map(|i| i as u32)
    }

    fn timer_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Timer(t)) => self.timers.get(*t as usize).filter(|t| t.alive).map(|_| *t),
            Value::Str(s) => self.find_timer(s),
            _ => None,
        }
    }

    fn region_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Region(r)) => Some(*r),
            Value::Str(s) => self.region_id(s),
            _ => None,
        }
    }

    fn clear_inside(&mut self, r: u32) {
        self.inside.retain(|(x, _)| *x != r);
    }

    pub(crate) fn clear_obj_markers(&mut self, obj: Ud) {
        self.markers.retain(|m| m.target != MarkTarget::Obj(obj));
    }

    fn char_of(&self, v: &Value) -> Option<u32> {
        match v {
            Value::Ud(Ud::Char(c)) => Some(*c),
            Value::Num(n) if *n >= 0.0 => Some(*n as u32),
            _ => None,
        }
    }

    fn char_team(&self, sim: &Sim, c: u32) -> Option<u8> {
        if let Some(u) = sim.units.get(self.char_slot(c) as usize) {
            return Some(u.team);
        }
        Some((c.checked_sub(PLACEHOLDER)? / 1000) as u8)
    }

    fn team_size(&self, t: u8) -> u32 {
        if t < 2 {
            self.team_units[usize::from(t)]
        } else {
            self.native_units.get(&t).copied().unwrap_or(0)
        }
    }

    /// The team's i-th member: its slots in order, then (for a native team)
    /// the members that never spawned. A native team's list is taken once
    /// and kept: a member spawned again lands in a new slot
    /// (`Sim::spawn_native_character`) and keeps its place through
    /// `char_alias`, where a fresh scan would shift every later member onto
    /// the units just spawned. Death Star's 45 s hangar
    /// `Ambush("hangar_ambush_spawn", 6, 5)` after `defend_ambush`'s three
    /// fielded three of its six.
    fn team_member(&mut self, sim: &Sim, t: u8, i: usize) -> Option<u32> {
        let slots: Vec<u32> = sim
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.team == t)
            .map(|(k, _)| k as u32)
            .collect();
        if t < 2 {
            return slots.get(i).copied();
        }
        let size = self.team_size(t) as usize;
        if !self.native_members.contains_key(&t) {
            let mut members: Vec<u32> = slots
                .iter()
                .map(|&s| {
                    self.char_alias
                        .iter()
                        .find(|&(_, &slot)| slot == s)
                        .map_or(s, |(&c, _)| c)
                })
                .collect();
            let first_free = members.len();
            members.extend(
                (first_free..size).map(|k| PLACEHOLDER + u32::from(t) * 1000 + k as u32),
            );
            self.native_members.insert(t, members);
        }
        self.native_members.get(&t)?.get(i).copied()
    }

    fn point_of(&self, sim: &Sim, v: &Value) -> Option<(Vec3, f32)> {
        match v {
            Value::Ud(Ud::Point(p, i)) => {
                let nodes = &self.paths.get(*p as usize)?.1;
                nodes.get(*i as usize).or(nodes.last()).copied()
            }
            Value::Str(s) => {
                if let Some(p) = self.path(s) {
                    return self.paths[p].1.first().copied();
                }
                let o = self.resolve_name(sim, s)?;
                self.obj_pos(sim, o).map(|p| (p, 0.0))
            }
            Value::Ud(u) => self.obj_pos(sim, *u).map(|p| (p, 0.0)),
            _ => None,
        }
    }

    /// `SpawnCharacter(char, point)` (`FUN_001916e8`): the character's
    /// member enters at the point, once (`Sim::spawn_native_character`).
    fn spawn_character(&mut self, sim: &mut Sim, c: u32, pos: (Vec3, f32)) {
        let scripted = self.scripted_chars.get(&c).cloned();
        if scripted.is_none() && self.char_unit(sim, c).is_some() {
            return;
        }
        let team = scripted
            .as_ref()
            .and_then(|s| s.team)
            .or_else(|| self.char_team(sim, c));
        let Some(team) = team else {
            return;
        };
        // A scripted hero is spawned with the class the script selected, on
        // the army it moved the member to, at the path point it asked for.
        // The native pool's own (already spawned) template is not used.
        if let Some(class) = scripted.as_ref().and_then(|s| s.class.as_deref()) {
            let Some(ci) = self
                .team_classes
                .get(usize::from(team))
                .and_then(|classes| classes.iter().position(|n| n.eq_ignore_ascii_case(class)))
            else {
                self.report(&format!(
                    "SpawnCharacter: {class} is not in team {}'s roster",
                    team + 1
                ));
                return;
            };
            let Some(uc) = sim
                .classes
                .get(usize::from(team))
                .and_then(|classes| classes.get(ci))
            else {
                return;
            };
            let spec = uc.spec(
                team,
                pos.0,
                pos.1,
                Controller::Ai,
                ci as u8,
                sim.bot_skill(team),
            );
            if let Some(h) = sim.spawn(&spec) {
                self.char_alias.insert(c, h.index);
            }
            return;
        }
        let old = sim
            .units
            .get(self.char_slot(c) as usize)
            .filter(|u| u.team == team && !u.alive)
            .map(|u| u.spec.class);
        if let Some(h) = sim.spawn_native_member(team, pos, old) {
            if h.index != c {
                self.char_alias.insert(c, h.index);
            }
        }
    }

    fn obj_health(&self, sim: &Sim, o: Ud) -> Option<f32> {
        match o {
            Ud::Unit(i, _) => sim.units.get(i as usize).map(|u| u.health.cur),
            Ud::Vehicle(i, _) => sim.vehicles.get(i as usize).map(|v| v.health.cur),
            Ud::Station(i) => sim.stations.get(usize::from(i)).map(|s| s.health.cur),
            _ => None,
        }
    }

    fn obj_shield(&self, sim: &Sim, o: Ud) -> (f32, f32, f32) {
        match o {
            Ud::Station(i) => sim
                .stations
                .get(usize::from(i))
                .map_or((0.0, 0.0, 0.0), |s| {
                    (s.health.shield, s.health.shield_max, s.add_shield)
                }),
            Ud::Vehicle(i, g) => sim
                .vehicles
                .get(i as usize)
                .filter(|v| v.gen == g)
                .map_or((0.0, 0.0, 0.0), |v| {
                    (v.health.shield, v.health.shield_max, v.add_shield)
                }),
            Ud::Unit(i, g) => sim
                .units
                .get(i as usize)
                .filter(|u| u.gen == g)
                .map_or((0.0, 0.0, 0.0), |u| {
                    u.droideka
                        .as_ref()
                        .and_then(|d| {
                            Some((d.shield.points, u.droideka_class()?.shield.max, 0.0))
                        })
                        .unwrap_or((u.health.shield, u.health.shield_max, 0.0))
                }),
            _ => (0.0, 0.0, 0.0),
        }
    }

    /// Top a main army back up to its unit count where it may spawn.
    /// Collect spawn points distributed round-robin across owned CPs (then
    /// command walkers), so a CP with 20 nodes (e.g. Kamino's `cp1` in the DNA
    /// hall) does not absorb an entire team before the second CP (`cp4`) gets
    /// any spawns.
    pub(crate) fn fill_enabled_ai(&mut self, sim: &mut Sim) {
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
            if sim.spawn_posts(team).is_empty() {
                continue;
            }
            let existing = sim
                .units
                .iter()
                .filter(|u| !u.is_player && u.team == team)
                .count() as u32;
            if existing >= self.team_units[t] {
                continue;
            }
            // Opening wave (`tick <= 1` or team 0's first wave after `onfirstspawn`)
            // spawns immediately by `AISpawnWeight` (`SWBFRNT2.ELF` `FUN_002e7240`,
            // lines 260573-260620). Mid-mission team activation (`tick > 1` on
            // non-opening teams, e.g. Kamino `DEF` when `cp1` + `cp4` open) enters
            // through `sim.waves[t]` (`FUN_0036c750`, lines 353853-353875) so bots
            // release on the team's spawn wave instead of materializing all at once.
            let use_wave = sim.tick > 1 && t != 0;
            for k in existing..self.team_units[t] {
                if use_wave {
                    sim.queue_ai_for_wave(team);
                } else {
                    sim.spawn_ai_at_drawn_post(team, k as usize);
                }
            }
        }
    }

    /// `AddUnitClass(team, class, min, max)`: the class joins the army's
    /// selectable list (and `GetCharacterClass`'s numbering) and returns its
    /// 0-based index in that list.
    fn add_unit_class(
        &mut self,
        sim: &mut Sim,
        t: u8,
        class: &str,
        min: Option<f32>,
        max: Option<f32>,
    ) -> Option<usize> {
        if t >= 2 {
            return None;
        }
        let ti = usize::from(t);
        let Some(ci) = self.team_classes[ti]
            .iter()
            .position(|c| c.eq_ignore_ascii_case(class))
        else {
            self.report(&format!(
                "AddUnitClass: {class} is not in team {}'s roster",
                t + 1
            ));
            return None;
        };
        if let Some(uc) = sim.classes[ti].get_mut(ci) {
            if let Some(m) = min {
                uc.min = m as u32;
            }
            if let Some(m) = max {
                uc.max = Some(m as u32);
            }
        }
        let idx = if let Some(idx) = self.class_order[ti].iter().position(|&c| c == ci) {
            idx
        } else {
            self.class_order[ti].push(ci);
            self.class_order[ti].len() - 1
        };
        sim.set_class_enabled(t, ci, true);
        self.pending_ai_fill = true;
        Some(idx)
    }

    /// `CreateEntity(class, point, name)`: a `flag` class becomes a
    /// carryable flag (`ObjectiveCTF` scores it in script: its
    /// `OnEnterRegion(captureRegion)` with the carrier); anything else is
    /// recorded by name and place.
    fn create_entity(&mut self, sim: &mut Sim, class: &str, name: &str, pos: Vec3) -> Value {
        let is_flag = self.class_bases.get(class).is_some_and(|b| b == "flag");
        if !is_flag {
            self.entities
                .push((name.to_ascii_lowercase(), class.to_owned(), pos));
            return Value::Ud(Ud::Entity(
                (self.instances.len() + self.entities.len() - 1) as u32,
            ));
        }
        let far = Vec3::new(0.0, -10000.0, 0.0);
        let ctf = sim.ctf.get_or_insert_with(|| {
            let mut c = CtfState::new_ground_one_flag(u32::MAX, pos, far, far);
            c.flags.clear();
            for r in &mut c.capture_regions {
                r.radius = 0.0;
            }
            c
        });
        let mut flag = core::sim::ctf::Flag::new_ground_neutral(pos);
        flag.allow_ai_pickup = self.allow_ai_pickup;
        flag.flyer_pickup_radius = 0.0;
        ctf.flags.push(flag);
        let ci = ctf.flags.len() - 1;
        self.flags.push(FlagEnt {
            name: name.to_owned(),
            class: class.to_owned(),
            ctf: ci,
            dead: false,
        });
        Value::Ud(Ud::Flag(self.flags.len() as u8 - 1))
    }

    pub(crate) fn kill_object(&mut self, sim: &mut Sim, v: &Value, notify: bool) {
        let Some(o) = self.resolve(sim, v) else {
            return;
        };
        self.clear_obj_markers(o);
        match o {
            Ud::Post(ci) => {
                let ci = usize::from(ci);
                if self.saved_cp_owners.len() <= ci {
                    self.saved_cp_owners.resize(sim.cps.len(), None);
                }
                let cp = &mut sim.cps[ci];
                if cp.owner.is_some() {
                    self.saved_cp_owners[ci] = cp.owner;
                }
                cp.killed = true;
                cp.owner = None;
                cp.capture = 0.0;
                cp.neutral = 0.0;
            }
            Ud::Station(si) => {
                if sim
                    .stations
                    .get(usize::from(si))
                    .is_some_and(|s| !s.destroyed)
                {
                    if notify && self.post_loaded {
                        sim.kill_station(si, None);
                    } else {
                        sim.install_station_wreck(si);
                    }
                }
            }
            Ud::Vehicle(i, g) => {
                let pad = sim
                    .vehicles
                    .get(i as usize)
                    .filter(|v| v.gen == g)
                    .and_then(|v| v.spawn_pad);
                if let Some(p) = pad.and_then(|p| sim.vehicle_pads.get_mut(p as usize)) {
                    p.team = None;
                    p.armed = false;
                }
                if notify && self.post_loaded {
                    sim.destroy_vehicle(core::vehicle::VehicleHandle { index: i, gen: g }, None);
                } else if let Some(v) = sim.vehicles.get_mut(i as usize).filter(|v| v.gen == g) {
                    v.health.cur = 0.0;
                    v.alive = false;
                }
            }
            Ud::Unit(i, g) => {
                if sim
                    .units
                    .get(i as usize)
                    .is_some_and(|u| u.alive && u.gen == g)
                {
                    sim.kill_environment(
                        core::unit::UnitHandle { index: i, gen: g },
                        core::sim::DeathCause::DeathRegion,
                    );
                }
            }
            Ud::Flag(f) => {
                let Some(ent) = self.flags.get_mut(usize::from(f)).filter(|e| !e.dead) else {
                    return;
                };
                ent.dead = true;
                let ci = ent.ctf;
                if let Some(ctf) = sim.ctf.as_mut() {
                    if ci < ctf.flags.len() {
                        ctf.flags.remove(ci);
                    }
                    for e in self.flags.iter_mut().filter(|e| !e.dead && e.ctf > ci) {
                        e.ctf -= 1;
                    }
                    if ctf.flags.is_empty() {
                        sim.ctf = None;
                    }
                }
            }
            _ => {}
        }
        if let Value::Str(name) = v {
            for pad in sim
                .vehicle_pads
                .iter_mut()
                .filter(|p| p.def.name.eq_ignore_ascii_case(name))
            {
                pad.team = None;
                pad.armed = false;
            }
        }
    }

    pub(crate) fn respawn_object(&mut self, sim: &mut Sim, v: &Value) {
        let Some(o) = self.resolve(sim, v) else {
            return;
        };
        match o {
            Ud::Post(ci) => {
                let ci = usize::from(ci);
                sim.cps[ci].killed = false;
                if let Some(&cap) = self.saved_cp_captures.get(ci) {
                    sim.cps[ci].def.capture = cap;
                    sim.cps[ci].capture_authored = true;
                }
                if sim.cps[ci].owner.is_none() {
                    sim.cps[ci].owner = self.saved_cp_owners.get(ci).copied().flatten();
                    sim.cps[ci].capture = 0.0;
                    sim.cps[ci].neutral = 0.0;
                }
                self.pending_ai_fill = true;
            }
            Ud::Station(si) => sim.respawn_station(si),
            _ => {}
        }
    }

    /// `SetProperty(object, key, value)` for the keys the campaigns set
    /// that the sim models.
    pub(crate) fn set_property(&mut self, sim: &mut Sim, target: &Value, key: &str, val: &str) {
        let key = key.trim().to_ascii_lowercase();
        let val = val.trim();
        // The PS2 creates a scripted vehicle at setup, so the mission can
        // author DisableTime/Team before anyone can board it. Our pads spawn
        // later (spa3's LAAT after SpawnTime = 15 s) and its name also names a
        // placed marker, so a ScriptPostLoad write resolves to that marker and
        // the vehicle arrives enabled for the AI to fly off. Bring the pad's
        // vehicle out first; `resolve` then finds it ahead of the marker.
        if let Some(name) = target.as_str() {
            let lower = name.to_ascii_lowercase();
            if let Some(pi) = sim
                .vehicle_pads
                .iter()
                .position(|p| p.def.name.eq_ignore_ascii_case(&lower))
            {
                let live = sim.vehicle_pads[pi]
                    .vehicles
                    .iter()
                    .any(|pv| sim.vehicle(pv.handle).is_some_and(|v| v.alive));
                if !live {
                    let team = sim.vehicle_pads[pi].team;
                    sim.spawn_from_pad(pi, team);
                }
            }
        }
        let Some(o) = self.resolve(sim, target) else {
            return;
        };
        match (o, key.as_str()) {
            (Ud::Flag(f), "allowaipickup") => {
                let allow = val != "0";
                if let Some(flag) = self
                    .flags
                    .get(usize::from(f))
                    .and_then(|e| sim.ctf.as_mut()?.flags.get_mut(e.ctf))
                {
                    flag.allow_ai_pickup = allow;
                }
            }
            (Ud::Unit(i, _), "captureposts") => {
                if let Some(u) = sim.units.get_mut(i as usize) {
                    u.capture_posts = val != "0";
                }
            }
            (Ud::Entity(i), "isvisible") if (i as usize) < self.instances.len() => {
                if val == "0" || val.eq_ignore_ascii_case("false") {
                    self.hidden_instances.insert(i);
                } else {
                    self.hidden_instances.remove(&i);
                }
            }
            (Ud::Entity(i), "iscollidable" | "iscollidible")
                if (i as usize) < self.instances.len() =>
            {
                let saved = &mut self.disabled_prop_collision;
                if val == "0" || val.eq_ignore_ascii_case("false") {
                    saved.entry(i).or_insert_with(|| {
                        let layers: Vec<_> = sim
                            .collision
                            .bodies()
                            .iter()
                            .enumerate()
                            .filter(|(_, body)| body.instance == Some(i))
                            .map(|(index, body)| (index, body.layers))
                            .collect();
                        sim.collision.set_body_layers(
                            layers
                                .iter()
                                .map(|&(index, _)| (index, core::collision::Layers::NONE)),
                        );
                        layers
                    });
                } else if let Some(layers) = saved.remove(&i) {
                    sim.collision.set_body_layers(layers);
                }
            }
            (Ud::Entity(_), "islocked") => {
                let name = self.obj_name(sim, o);
                let locked = val.parse::<f32>().is_ok_and(|n| n != 0.0);
                for (di, d) in self.door_names.iter().enumerate() {
                    if !d.is_empty() && *d == name {
                        if let Some(door) = sim.doors.get_mut(di) {
                            door.locked = locked;
                        }
                    }
                }
            }
            (Ud::Post(ci), _) => self.set_post_property(sim, usize::from(ci), &key, val),
            (Ud::Station(si), _) => {
                set_station_property(sim, usize::from(si), &key, val);
                if key == "curhealth" && val.parse::<f32>().is_ok_and(|n| n <= 0.0) {
                    self.clear_obj_markers(o);
                }
            }
            (
                Ud::Vehicle(i, g),
                "curhealth" | "maxhealth" | "curshield" | "maxshield" | "addshield" | "team"
                | "disabletime",
            ) => {
                let Some(v) = sim.vehicles.get_mut(i as usize).filter(|v| v.gen == g) else {
                    return;
                };
                let Ok(n) = val.parse::<f32>() else {
                    return;
                };
                match key.as_str() {
                    "curhealth" => v.health.cur = n.fmin(v.health.max.fmax(n)),
                    "maxhealth" => {
                        v.health.max = n.fmax(1.0);
                        v.health.cur = v.health.cur.fmin(v.health.max);
                    }
                    "curshield" => {
                        v.health.shield = if v.health.shield_max > 0.0 {
                            n.clamp(0.0, v.health.shield_max)
                        } else {
                            n.fmax(0.0)
                        };
                    }
                    "maxshield" => {
                        v.health.shield_max = n.fmax(0.0);
                        v.health.shield = v.health.shield.fmin(v.health.shield_max);
                    }
                    "addshield" => v.add_shield = n.fmax(0.0),
                    "disabletime" => v.disable = n,
                    _ => v.team = (n >= 1.0).then(|| n as u8 - 1),
                }
            }
            _ => {}
        }
    }

    fn set_post_property(&mut self, sim: &mut Sim, ci: usize, key: &str, val: &str) {
        match key {
            "isvisible" => sim.cps[ci].visible = val != "0",
            "team" => {
                if let Ok(team) = val.parse::<f32>() {
                    let owner = (team >= 1.0).then(|| team as u8 - 1);
                    if self.saved_cp_owners.len() <= ci {
                        self.saved_cp_owners.resize(sim.cps.len(), None);
                    }
                    self.saved_cp_owners[ci] = owner;
                    if !sim.cps[ci].killed {
                        sim.cps[ci].owner = owner;
                        sim.cps[ci].capture = 0.0;
                        sim.cps[ci].neutral = 0.0;
                    }
                    self.pending_ai_fill = true;
                }
            }
            "spawnpath" => {
                if val.is_empty() {
                    sim.cps[ci].def.spawn_nodes.clear();
                } else if let Some(p) = self.path(val) {
                    sim.cps[ci].def.spawn_nodes = self.paths[p].1.clone();
                    self.pending_ai_fill = true;
                }
            }
            "captureregion" => {
                let r = self.region_id(val).map(|r| self.regions[r as usize].1);
                let cp = &mut sim.cps[ci];
                match r {
                    Some(r) => {
                        cp.def.capture = r;
                        cp.capture_authored = true;
                        cp.capturable = true;
                        if let Some(saved) = self.saved_cp_captures.get_mut(ci) {
                            *saved = r;
                        }
                    }
                    None => {
                        cp.def.capture = Region::sphere(Vec3::new(0.0, -10000.0, 0.0), 0.0);
                        cp.capture_authored = false;
                        cp.capturable = false;
                    }
                }
            }
            "controlregion" => {
                sim.cps[ci].def.control = self.region_id(val).map(|r| self.regions[r as usize].1);
            }
            "aispawnweight" => {
                if let Ok(wt) = val.parse::<f32>() {
                    sim.cps[ci].ai_spawn_weight = wt.fmax(0.0);
                }
            }
            k if k.starts_with("value_atk_") || k.starts_with("value_def_") => {
                if let Ok(v) = val.parse::<f32>() {
                    let row = crate::walk::faction_row(&k["value_atk_".len()..]);
                    for t in 0..2 {
                        if self.team_value_rows[t] != row {
                            continue;
                        }
                        if k.starts_with("value_atk_") {
                            sim.cps[ci].def.value_atk[t] = v as i32;
                        } else {
                            sim.cps[ci].def.value_def[t] = v as i32;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn set_station_property(sim: &mut Sim, si: usize, key: &str, val: &str) {
    if matches!(key, "iscollidable" | "iscollidible") {
        let collidable = val != "0" && !val.eq_ignore_ascii_case("false");
        sim.set_station_collidable(si as u16, collidable);
        return;
    }
    let Ok(n) = val.parse::<f32>() else {
        return;
    };
    let Some(st) = sim.stations.get_mut(si) else {
        return;
    };
    match key {
        "team" => st.team = (n >= 1.0).then(|| n as u8 - 1),
        "curhealth" => {
            if n <= 0.0 {
                if !st.destroyed {
                    sim.install_station_wreck(si as u16);
                }
            } else {
                st.health.max = st.health.max.fmax(n);
                st.health.cur = n;
                if st.destroyed {
                    sim.respawn_station(si as u16);
                    sim.stations[si].health.cur = n;
                }
            }
        }
        "maxhealth" => {
            st.health.max = n.fmax(1.0);
            st.health.cur = st.health.cur.fmin(st.health.max);
        }
        "curshield" => {
            st.health.shield = if st.health.shield_max > 0.0 {
                n.clamp(0.0, st.health.shield_max)
            } else {
                n.fmax(0.0)
            };
        }
        "maxshield" => {
            st.health.shield_max = n.fmax(0.0);
            st.health.shield = st.health.shield.fmin(st.health.shield_max);
        }
        "addshield" => st.add_shield = n.fmax(0.0),
        _ => {}
    }
}

/// Character ids of native members that never spawned:
/// `PLACEHOLDER + team * 1000 + member`.
pub(crate) const PLACEHOLDER: u32 = 1_000_000;
