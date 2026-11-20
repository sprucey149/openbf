//! The engine bindings the translated mission scripts call (BF2's
//! `ScriptCB_*` / mission API, registered by the PS2 ELF's Lua binding
//! table and documented by the PC mod tools' Lua headers): each reads or
//! changes the sim, or queues something for the HUD.
//!
//! This module owns the binding tables and the Host/Ctx dispatch only. The
//! `World` writers that dispatch calls live beside the state in
//! `campaign_runtime::world_ops`; state and queries are the runtime's.
//!
//! Characters are player slots (a number: the sim unit index). A native
//! team's members that never spawned are placeholders until
//! `SpawnCharacter` gives them a slot. Objects are the sim's posts,
//! stations, vehicles, units, the script's flags, or the placed entities.
//! BF2 teams are 1-based (sim team + 1); 0 is neutral.

use crate::campaign_runtime::{
    char_value, reinforcement_count, unit_world_pos, Ctx, EventKind, Filter, Handler, MarkTarget,
    Marker, ObjectiveLine, Popup, Timer, World,
};
use crate::campaign_script::{num_text, Host, Interp, Ret, ScriptError, Ud, Value};
use crate::classdb::fnv1a_lowercase;
use core::fmath::FloatExt;
use core::sim::Sim;
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
/// PROVISIONAL: `EntityFlyerTakeOff`/`InitAsLanded` play nothing; the world
/// animation bindings (`PlayAnimation`, ...) drive `crate::world_anim` instead.
/// replace: measure the PS2's take-off and landing animation binding before
/// wiring these two.
/// `ShowPopup` is ignored too: the PS2's binding-table handler returns
/// without publishing a popup, so the port does the same.
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

pub(crate) fn bf2_team(v: &Value) -> Option<u8> {
    v.num().filter(|n| *n >= 0.0).map(|n| n as u8)
}

/// The marker target named by the first argument of a `MapAdd*` /
/// `MapRemove*` binding. Add and remove share this resolution so a marker
/// can only be taken away by naming the same entity, class or region.
fn marker_target(w: &World, sim: &Sim, name: &str, a: &[Value]) -> Option<MarkTarget> {
    match name {
        "MapAddEntityMarker" | "MapRemoveEntityMarker" => {
            w.resolve(sim, arg(a, 0)).map(MarkTarget::Obj)
        }
        "MapAddClassMarker" | "MapRemoveClassMarker" => arg(a, 0)
            .as_str()
            .map(|c| MarkTarget::Class(c.to_ascii_lowercase())),
        _ => w.region_of(arg(a, 0)).map(MarkTarget::Region),
    }
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
            && !(name == "SetReinforcementCount" && sim_team(arg(args, 0)).is_some_and(|t| t >= 2))
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

    /// One `ScriptCB_*` call. The arms are split into cohesive child modules;
    /// each group returns `None` for a name it does not own. This module owns
    /// only Host/Ctx dispatch, never `World` state: the writers it calls live
    /// beside the state in `campaign_runtime::world_ops`.
    fn bound(&mut self, it: &mut Interp, name: &str, a: &[Value]) -> Ret {
        if let Some(r) = self.general(it, name, a) {
            return r;
        }
        if let Some(r) = self.objectives(it, name, a) {
            return r;
        }
        if let Some(r) = self.timers_regions(it, name, a) {
            return r;
        }
        if let Some(r) = self.characters(it, name, a) {
            return r;
        }
        if let Some(r) = self.sim_ai(it, name, a) {
            return r;
        }
        Err(ScriptError(format!("unbound engine call {name}")))
    }
}

/// Character ids of native members that never spawned:
/// `PLACEHOLDER + team * 1000 + member`.
pub(crate) const PLACEHOLDER: u32 = 1_000_000;

mod characters;
mod general;
mod objectives;
mod sim_ai;
mod timers_regions;
