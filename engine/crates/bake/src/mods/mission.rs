//! Generic translation from statically inspected legacy BF2 Lua 5.0 mission
//! facts into OpenBF [`MissionDef`]-compatible RON files.

use anyhow::{bail, Result};
use manifest::world_events::{Action, Anchor, WorldEvents};
use mungefs::addon::{InspectedMission, WorldAction, WorldEffectAnchor};
use serde::Serialize;

#[derive(Serialize)]
struct TeamDefRon {
    faction: String,
    units: u32,
    tickets: u32,
    classes: Vec<(String, u32, Option<u32>)>,
}

#[derive(Serialize)]
struct SideClassLoadRon {
    side: String,
    request: String,
}

#[derive(Serialize)]
struct LocalTeamDefRon {
    team: u8,
    units: u32,
    classes: Vec<(String, u32, Option<u32>)>,
}

#[derive(Serialize)]
struct CtfFlagDefRon {
    name: String,
    home_region: String,
    capture_region: String,
    geometry: Option<String>,
    carried_geometry: Option<String>,
}

#[derive(Serialize)]
struct CtfDefRon {
    flag: String,
    home_region: String,
    capture_region_att: String,
    capture_region_def: String,
    cap_region_dummy_att: Option<String>,
    cap_region_dummy_def: Option<String>,
    hide_cps: bool,
    score_limit: Option<u32>,
    two_flag: bool,
    flags: Vec<CtfFlagDefRon>,
}

#[derive(Serialize)]
struct VoiceDefRon {
    bleeding: Vec<(u8, u8, String, bool)>,
    low_reinforcements: Vec<(u8, u8, String, f32, bool)>,
    out_of_bounds: Vec<(u8, String)>,
}

#[derive(Serialize)]
struct MusicCallsRon {
    ambient: Vec<(u8, f32, String, u8, bool)>,
    victory: Vec<(u8, String)>,
    defeat: Vec<(u8, String)>,
}

#[derive(Serialize)]
struct AiSettingsRon {
    view_mult: Option<f32>,
    aggressiveness: Vec<(u8, f32)>,
    snipe_suitability: Option<f32>,
    no_capture: Vec<(String, u8)>,
}

#[derive(Serialize)]
struct MissionDefRon {
    map: String,
    mode: String,
    source_game: String,
    heroes: Vec<(u8, String)>,
    class_load_order: Vec<SideClassLoadRon>,
    teams: Vec<TeamDefRon>,
    ai_alive_cap: u32,
    spawn_delay: f32,
    spawn_interval: f32,
    era: Option<String>,
    attacking_team: Option<u8>,
    cps: Vec<(String, u8)>,
    death_regions: Vec<String>,
    max_player_fly_height: Option<f32>,
    min_player_fly_height: Option<f32>,
    max_fly_height: Option<f32>,
    min_fly_height: Option<f32>,
    ground_flyer_map: bool,
    landing_regions: Vec<String>,
    ctf: Option<CtfDefRon>,
    particle_lod_bias: Option<f32>,
    posts: Vec<String>,
    killed: Vec<String>,
    local_teams: Vec<LocalTeamDefRon>,
    local_team_relations: Vec<(u8, u8, bool)>,
    arc_blocks: Vec<(String, bool)>,
    door_locks: Vec<(String, bool)>,
    barriers: Vec<(String, bool)>,
    voice: VoiceDefRon,
    music: MusicCallsRon,
    sound: Vec<(String, String)>,
    camera_shots: Vec<[f32; 7]>,
    ai: AiSettingsRon,
    layers: Vec<String>,
    #[serde(skip_serializing_if = "WorldEvents::is_empty")]
    world_events: WorldEvents,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    class_properties: Vec<(String, String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    script_setup: Option<(Option<i64>, bool)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    script: Option<manifest::script_ir::ScriptDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    script_libs: Vec<(String, String)>,
    source: String,
}

fn world_action(action: &WorldAction) -> Action {
    match action {
        WorldAction::PlayAnimation(name) => Action::PlayAnimation(name.clone()),
        WorldAction::PauseAnimation(name) => Action::PauseAnimation(name.clone()),
        WorldAction::KillObject(name) => Action::KillObject(name.clone()),
        WorldAction::Barrier(name, enable) => Action::Barrier(name.clone(), *enable),
        WorldAction::Arc(name, block) => Action::Arc(name.clone(), *block),
        WorldAction::Effect(effect, anchor) => Action::Effect(
            effect.clone(),
            match anchor {
                WorldEffectAnchor::Object(name) => Anchor::Object(name.clone()),
                WorldEffectAnchor::PathPoint(path, index) => {
                    Anchor::PathPoint(path.clone(), *index)
                }
            },
        ),
        WorldAction::After(seconds, actions) => {
            Action::After(*seconds, actions.iter().map(world_action).collect())
        }
    }
}

/// Convert a statically analyzed legacy BF2 mission into OpenBF mission RON.
pub fn mission_to_ron(
    map_key: &str,
    m: &InspectedMission,
    script: Option<manifest::script_ir::ScriptDef>,
    script_libs: Vec<(String, String)>,
) -> Result<String> {
    if !m.supported {
        bail!(
            "mission '{}' ({}) is unsupported: {}",
            m.script_name,
            m.mode,
            m.unsupported_reason
                .as_deref()
                .unwrap_or("unsupported script behavior")
        );
    }
    if m.teams.len() != 2 {
        bail!(
            "mission '{}' declared {} teams (expected 2)",
            m.script_name,
            m.teams.len()
        );
    }

    let teams: Vec<TeamDefRon> = m
        .teams
        .iter()
        .map(|t| TeamDefRon {
            faction: t.faction.clone(),
            units: t.units.max(1),
            tickets: t.tickets.max(1),
            classes: t.classes.clone(),
        })
        .collect();

    let (spawn_delay, spawn_interval) = m.spawn_delay.unwrap_or((10.0, 0.25));

    let ctf = m.ctf.as_ref().map(|c| {
        let mut flags: Vec<CtfFlagDefRon> = c
            .flags
            .iter()
            .map(|f| CtfFlagDefRon {
                name: f.name.clone(),
                home_region: f.home_region.clone(),
                capture_region: f.capture_region.clone(),
                geometry: f.geometry.clone(),
                carried_geometry: f.carried_geometry.clone(),
            })
            .collect();
        // Order two-flag entries cleanly by team hint when authored out of order.
        if flags.len() >= 2 && c.flags[0].team_hint == Some(2) && c.flags[1].team_hint == Some(1) {
            flags.swap(0, 1);
        }
        CtfDefRon {
            flag: c.flag.clone(),
            home_region: c.home_region.clone(),
            capture_region_att: c.capture_region_att.clone(),
            capture_region_def: c.capture_region_def.clone(),
            cap_region_dummy_att: c.cap_region_dummy_att.clone(),
            cap_region_dummy_def: c.cap_region_dummy_def.clone(),
            hide_cps: c.hide_cps,
            score_limit: c.score_limit,
            two_flag: c.two_flag,
            flags,
        }
    });

    let ron_struct = MissionDefRon {
        map: map_key.to_owned(),
        mode: m.mode.clone(),
        source_game: "bf2".to_owned(),
        heroes: m.heroes.clone(),
        class_load_order: m
            .class_load_order
            .iter()
            .map(|(side, request)| SideClassLoadRon {
                side: side.clone(),
                request: request.clone(),
            })
            .collect(),
        teams,
        ai_alive_cap: m.ai_alive_cap.max(1),
        spawn_delay,
        spawn_interval,
        era: Some(m.era.clone()),
        attacking_team: m.attacking_team,
        cps: m.cps.clone(),
        death_regions: m.death_regions.clone(),
        max_player_fly_height: m.max_player_fly_height,
        min_player_fly_height: m.min_player_fly_height,
        max_fly_height: m.max_fly_height,
        min_fly_height: m.min_fly_height,
        ground_flyer_map: m.ground_flyer_map,
        landing_regions: m.landing_regions.clone(),
        ctf,
        particle_lod_bias: m.particle_lod_bias,
        posts: m.posts.clone(),
        killed: m.killed.clone(),
        local_teams: m
            .local_teams
            .iter()
            .map(|(team, units, classes)| LocalTeamDefRon {
                team: *team,
                units: *units,
                classes: classes.clone(),
            })
            .collect(),
        local_team_relations: m.local_team_relations.clone(),
        arc_blocks: m.arc_blocks.clone(),
        door_locks: m.door_locks.clone(),
        barriers: m.barriers.clone(),
        voice: VoiceDefRon {
            bleeding: m.voice_bleeding.clone(),
            low_reinforcements: m.voice_low_reinforcements.clone(),
            out_of_bounds: m.voice_out_of_bounds.clone(),
        },
        music: MusicCallsRon {
            ambient: m.music_ambient.clone(),
            victory: m.music_victory.clone(),
            defeat: m.music_defeat.clone(),
        },
        sound: m.sound_banks.clone(),
        camera_shots: m.camera_shots.clone(),
        ai: AiSettingsRon {
            view_mult: m.ai_view_mult,
            aggressiveness: m.ai_aggressiveness.clone(),
            snipe_suitability: m.ai_snipe_suitability,
            no_capture: m.ai_no_capture.clone(),
        },
        layers: m.layers.clone(),
        world_events: WorldEvents {
            start: m.world_events.start.iter().map(world_action).collect(),
            object_destroyed: m
                .world_events
                .object_destroyed
                .iter()
                .map(|(name, actions)| (name.clone(), actions.iter().map(world_action).collect()))
                .collect(),
        },
        class_properties: m.class_properties.clone(),
        script_setup: (m.script_pick.is_some() || m.script_multiplayer)
            .then_some((m.script_pick, m.script_multiplayer)),
        script,
        script_libs,
        source: format!("addon:{}", m.script_name),
    };

    Ok(ron::ser::to_string_pretty(&ron_struct, crate::pretty())?)
}
