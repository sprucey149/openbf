use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{
    resolve_inside_lvl_pc, stock_world_path, AddonWorldLoad,InspectedMission, ParsedCtf, ParsedCtfFlag, ParsedTeam,
};
use crate::lua50::{self, LuaCall, LuaVal, MissionScriptAnalysis};

fn norm_faction(raw: &str) -> String {
    let low = raw.trim().to_ascii_lowercase();
    match low.as_str() {
        "rep" | "republic" => "rep".into(),
        "cis" | "confederacy" => "cis".into(),
        "all" | "alliance" | "reb" | "rebel" => "all".into(),
        "imp" | "empire" | "imperial" => "imp".into(),
        _ => low,
    }
}

/// `cor/cor1.lvl`: one of the game's own worlds, named for its folder and a
/// digit. `spa/spa_sky.lvl` and `side/rep.lvl` are not.
pub(super) fn is_stock_world_path(path: &str) -> bool {
    path.split_once('/').is_some_and(|(dir, file)| {
        file.strip_suffix(".lvl")
            .and_then(|stem| stem.strip_prefix(dir))
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
    })
}

pub(super) fn fold_calls(
    analysis: &MissionScriptAnalysis,
    script_name: &str,
    registered_in_addme: bool,
    declared_era: Option<&str>,
    root_canon: &Path,
    lvl_pc: Option<&Path>,
) -> InspectedMission {
    let mut m = InspectedMission {
        script_name: script_name.to_owned(),
        registered_in_addme,
        ai_alive_cap: 16,
        supported: true,
        unsupported_features: analysis.env.unsupported.clone(),
        ..Default::default()
    };
    let (world_events, translated) = super::world_events::compile(analysis);
    m.world_events = world_events;
    m.unsupported_features
        .retain(|feature| !translated.contains(&feature.api));

    let all_calls: Vec<&LuaCall> = analysis
        .init_calls
        .iter()
        .chain(&analysis.postload_calls)
        .collect();

    let mut team_names: BTreeMap<u8, String> = BTreeMap::new();
    let mut team_units: BTreeMap<u8, u32> = BTreeMap::new();
    let mut team_tickets: BTreeMap<u8, u32> = BTreeMap::new();
    let mut team_classes: BTreeMap<u8, Vec<(String, u32, Option<u32>)>> = BTreeMap::new();
    let mut team_order: Vec<u8> = Vec::new();
    let mut note_team = |t: u8| {
        if t >= 1 && !team_order.contains(&t) {
            team_order.push(t);
        }
    };

    type FlagProps = (Option<String>, Option<String>, Option<u8>);
    let mut flag_props: BTreeMap<String, FlagProps> = BTreeMap::new();
    let mut has_obj_conquest = false;
    let mut has_obj_ctf = false;
    let mut has_obj_1flag = false;
    let mut has_obj_assault = false;
    let mut has_obj_tdm = false;
    let mut objective_ctf_cfg: Option<(Option<u32>, bool)> = None;
    let mut ctf_flags: Vec<ParsedCtfFlag> = Vec::new();
    let mut one_flag_cfg: Option<ParsedCtf> = None;
    let mut eras_seen: BTreeSet<String> = BTreeSet::new();

    for call in &all_calls {
        let a = &call.args;
        match call.func.as_str() {
            "ReadDataFile" => {
                if let Some(first) = a.first().and_then(|v| v.as_str()) {
                    let norm = first.replace('\\', "/");
                    let low = norm.to_ascii_lowercase();
                    if let Some(inner) = low.strip_prefix("dc:") {
                        if inner.starts_with("load/") {
                            if let (Some(canon), Some(pc)) = (Some(root_canon), lvl_pc) {
                                if let Some((_, p)) = resolve_inside_lvl_pc(canon, pc, first) {
                                    m.loadscreen_lvl_path = Some(p);
                                }
                            }
                        } else if inner.starts_with("side/") {
                            let side_stem = inner
                                .trim_start_matches("side/")
                                .trim_end_matches(".lvl")
                                .to_owned();
                            if let Some(pc) = lvl_pc {
                                if let Some((_, p)) = resolve_inside_lvl_pc(root_canon, pc, first) {
                                    if !m.custom_side_lvls.iter().any(|(s, _)| s == &side_stem) {
                                        m.custom_side_lvls.push((side_stem.clone(), p));
                                    }
                                }
                            }
                            for arg in a.iter().skip(1).filter_map(|v| v.as_str()) {
                                m.class_load_order.push((side_stem.clone(), arg.to_owned()));
                            }
                        } else if inner.starts_with("sound/") {
                            let stem = inner
                                .split(';')
                                .next()
                                .unwrap_or(inner)
                                .trim_end_matches(".lvl")
                                .to_owned();
                            if let Some(pc) = lvl_pc {
                                if let Some((_, p)) = resolve_inside_lvl_pc(root_canon, pc, first) {
                                    m.custom_sound_lvls.push((stem, p));
                                    if let Some((archive, level)) = inner.rsplit_once(';') {
                                        let planet = archive
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or(archive)
                                            .trim_end_matches(".lvl");
                                        m.sound_banks.push((planet.to_owned(), level.to_owned()));
                                    }
                                }
                            }
                        } else if inner.contains(".lvl") {
                            if let Some(pc) = lvl_pc {
                                if let Some((rel, abs)) =
                                    resolve_inside_lvl_pc(root_canon, pc, first)
                                {
                                    m.map_lvl_rel = Some(rel.clone());
                                    m.map_lvl_path = Some(abs.clone());
                                    m.world_loads.push(AddonWorldLoad {
                                        rel_path: rel,
                                        path: abs,
                                        selectors: a
                                            .iter()
                                            .skip(1)
                                            .filter_map(LuaVal::as_str)
                                            .filter(|s| !s.is_empty())
                                            .map(str::to_owned)
                                            .collect(),
                                    });
                                } else {
                                    m.map_lvl_rel = Some(norm[3..].to_owned());
                                }
                            }
                            for arg in a.iter().skip(1).filter_map(|v| v.as_str()) {
                                if !arg.is_empty() && !m.layers.contains(&arg.to_owned()) {
                                    m.layers.push(arg.to_owned());
                                }
                            }
                        }
                    } else if low.starts_with("side\\") || low.starts_with("side/") {
                        let fname = low.rsplit('/').next().unwrap_or(&low);
                        let side = fname.strip_suffix(".lvl").unwrap_or(fname).to_owned();
                        match side.as_str() {
                            "all" | "imp" => {
                                eras_seen.insert("gcw".into());
                            }
                            "rep" | "cis" => {
                                eras_seen.insert("cw".into());
                            }
                            _ => {}
                        }
                        if let Some(pc) = lvl_pc {
                            if let Some((_, p)) = resolve_inside_lvl_pc(root_canon, pc, first) {
                                if !m.custom_side_lvls.iter().any(|(s, _)| s == &side) {
                                    m.custom_side_lvls.push((side.clone(), p));
                                }
                            }
                        }
                        for arg in a.iter().skip(1).filter_map(|v| v.as_str()) {
                            m.class_load_order.push((side.clone(), arg.to_owned()));
                        }
                    } else if low.starts_with("sound\\") || low.starts_with("sound/") {
                        if let Some((lvl_part, sub)) = norm.split_once(';') {
                            let planet = lvl_part
                                .rsplit('/')
                                .next()
                                .unwrap_or(lvl_part)
                                .trim_end_matches(".lvl")
                                .trim_end_matches(".LVL")
                                .to_ascii_lowercase();
                            let bank = sub.trim().to_ascii_lowercase();
                            if !planet.is_empty() && !bank.is_empty() {
                                m.sound_banks.push((planet, bank));
                            }
                        }
                    } else if low.ends_with(".lvl") {
                        if is_stock_world_path(&low) {
                            m.on_stock_world = true;
                            m.world_loads.push(AddonWorldLoad {
                                rel_path: low.clone(),
                                path: stock_world_path(&low),
                                selectors: a
                                    .iter()
                                    .skip(1)
                                    .filter_map(LuaVal::as_str)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned)
                                    .collect(),
                            });
                        }
                        for arg in a.iter().skip(1).filter_map(|v| v.as_str()) {
                            if !arg.is_empty() && !m.layers.contains(&arg.to_owned()) {
                                m.layers.push(arg.to_owned());
                            }
                        }
                    }
                }
            }
            "SetupTeams" => {
                if let Some(LuaVal::Table(tid)) = a.first() {
                    if let Some(tbl) = analysis.env.table(*tid) {
                        for (k, v) in &tbl.entries {
                            let lua50::LuaKey::Str(faction_key) = k else {
                                continue;
                            };
                            let LuaVal::Table(side_tid) = v else { continue };
                            let Some(side) = analysis.env.table(*side_tid) else {
                                continue;
                            };
                            let Some(t) = side
                                .get_str_ci("team")
                                .and_then(|val| val.as_i64())
                                .and_then(|i| u8::try_from(i).ok())
                            else {
                                continue;
                            };
                            note_team(t);
                            team_names.insert(t, norm_faction(faction_key));
                            if let Some(u) = side
                                .get_str_ci("units")
                                .and_then(|val| val.as_i64())
                                .and_then(|i| u32::try_from(i).ok())
                            {
                                team_units.insert(t, u);
                            }
                            if let Some(r) = side
                                .get_str_ci("reinforcements")
                                .and_then(|val| val.as_i64())
                            {
                                team_tickets.insert(t, if r < 0 { 999_999 } else { r as u32 });
                            }
                            for (slot_k, slot_v) in &side.entries {
                                if let lua50::LuaKey::Str(sk) = slot_k {
                                    if sk.eq_ignore_ascii_case("hero") {
                                        continue;
                                    }
                                }
                                let LuaVal::Table(ctid) = slot_v else {
                                    continue;
                                };
                                let Some(ctbl) = analysis.env.table(*ctid) else {
                                    continue;
                                };
                                let Some(cls) = ctbl.get_int(1).and_then(|v| v.as_str()) else {
                                    continue;
                                };
                                let mn = ctbl
                                    .get_int(2)
                                    .and_then(|v| v.as_i64())
                                    .and_then(|i| u32::try_from(i.max(0)).ok())
                                    .unwrap_or(1);
                                let mx = ctbl
                                    .get_int(3)
                                    .and_then(|v| v.as_i64())
                                    .and_then(|i| u32::try_from(i.max(0)).ok());
                                team_classes
                                    .entry(t)
                                    .or_default()
                                    .push((cls.to_owned(), mn, mx));
                            }
                        }
                    }
                }
            }
            "SetTeamName" => {
                if let (Some(t), Some(name)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    note_team(t);
                    team_names.insert(t, norm_faction(name));
                }
            }
            "SetUnitCount" => {
                if let (Some(t), Some(n)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u32::try_from(i.max(0)).ok()),
                ) {
                    note_team(t);
                    team_units.insert(t, n);
                }
            }
            "SetReinforcementCount" => {
                if let (Some(t), Some(n)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_i64()),
                ) {
                    note_team(t);
                    team_tickets.insert(t, if n < 0 { 999_999 } else { n as u32 });
                }
            }
            "AddUnitClass" => {
                if let (Some(t), Some(cls)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    note_team(t);
                    let mn = a
                        .get(2)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u32::try_from(i.max(0)).ok())
                        .unwrap_or(1);
                    let mx = a
                        .get(3)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u32::try_from(i.max(0)).ok());
                    team_classes
                        .entry(t)
                        .or_default()
                        .push((cls.to_owned(), mn, mx));
                }
            }
            "SetHeroClass" => {
                if let (Some(t), Some(h)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    m.heroes.push((t, h.to_owned()));
                }
            }
            "SetTeamAsFriend" | "SetTeamAsEnemy" => {
                if let (Some(s), Some(t)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                ) {
                    if s.max(t) >= 3 {
                        m.local_team_relations
                            .push((s, t, call.func == "SetTeamAsFriend"));
                    }
                }
            }
            "SetSpawnDelay" => {
                if let (Some(d), Some(iv)) = (
                    a.first().and_then(|v| v.as_f64()),
                    a.get(1).and_then(|v| v.as_f64()),
                ) {
                    m.spawn_delay = Some((d as f32, iv as f32));
                }
            }
            "SetAttackingTeam" => {
                m.attacking_team = a
                    .first()
                    .and_then(|v| v.as_i64())
                    .and_then(|i| u8::try_from(i).ok());
            }
            "AddDeathRegion" => {
                if let Some(s) = a.first().and_then(|v| v.as_str()) {
                    m.death_regions.push(s.to_owned());
                }
            }
            "AddLandingRegion" => {
                if let Some(s) = a.first().and_then(|v| v.as_str()) {
                    m.landing_regions.push(s.to_owned());
                }
            }
            "SetMaxPlayerFlyHeight" => {
                m.max_player_fly_height = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "SetMinPlayerFlyHeight" => {
                m.min_player_fly_height = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "SetMaxFlyHeight" => {
                m.max_fly_height = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "SetMinFlyHeight" => {
                m.min_fly_height = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "SetGroundFlyerMap" => {
                if let Some(v) = a.first().and_then(|v| v.as_f64()) {
                    m.ground_flyer_map = v != 0.0;
                }
            }
            "SetParticleLODBias" => {
                m.particle_lod_bias = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "ScriptCB_SetNumBots" => {
                if let Some(n) = a
                    .first()
                    .and_then(|v| v.as_i64())
                    .and_then(|i| u32::try_from(i.max(1)).ok())
                {
                    m.ai_alive_cap = n;
                }
            }
            "SetProperty" => {
                if let (Some(obj), Some(prop), Some(val)) = (
                    a.first().and_then(|v| v.as_str()),
                    a.get(1).and_then(|v| v.as_str()),
                    a.get(2),
                ) {
                    if prop.eq_ignore_ascii_case("Team") {
                        let t = val
                            .as_i64()
                            .or_else(|| val.as_str().and_then(|s| s.trim().parse().ok()))
                            .and_then(|i| u8::try_from(i).ok());
                        if let Some(t) = t {
                            m.cps.push((obj.to_owned(), t));
                            flag_props.entry(obj.to_ascii_lowercase()).or_default().2 = Some(t);
                        }
                    } else if prop.eq_ignore_ascii_case("GeometryName") {
                        if let Some(s) = val.as_str() {
                            flag_props.entry(obj.to_ascii_lowercase()).or_default().0 =
                                Some(s.to_owned());
                        }
                    } else if prop.eq_ignore_ascii_case("CarriedGeometryName") {
                        if let Some(s) = val.as_str() {
                            flag_props.entry(obj.to_ascii_lowercase()).or_default().1 =
                                Some(s.to_owned());
                        }
                    } else if prop.eq_ignore_ascii_case("IsLocked") {
                        let locked = val.as_i64() == Some(1)
                            || val.as_str().is_some_and(|s| s.trim() == "1");
                        let unlocked = val.as_i64() == Some(0)
                            || val.as_str().is_some_and(|s| s.trim() == "0");
                        if locked || unlocked {
                            m.door_locks.push((obj.to_owned(), locked));
                        }
                    }
                }
            }
            "SetObjectTeam" => {
                if let (Some(obj), Some(t)) = (
                    a.first().and_then(|v| v.as_str()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                ) {
                    m.cps.push((obj.to_owned(), t));
                }
            }
            "CommandPost:New" => {
                if let Some(LuaVal::Table(tid)) = a.get(1) {
                    if let Some(name) = analysis
                        .env
                        .table(*tid)
                        .and_then(|t| t.get_str_ci("name"))
                        .and_then(|v| v.as_str())
                    {
                        m.posts.push(name.to_owned());
                    }
                }
            }
            "KillObject" => {
                if let Some(s) = a.first().and_then(|v| v.as_str()) {
                    m.killed.push(s.to_owned());
                }
            }
            "BlockPlanningGraphArcs" | "UnblockPlanningGraphArcs" => {
                let block = call.func == "BlockPlanningGraphArcs";
                for s in a.iter().filter_map(|v| v.as_str()) {
                    m.arc_blocks.push((s.to_owned(), block));
                }
            }
            "EnableBarriers" | "DisableBarriers" => {
                let enable = call.func == "EnableBarriers";
                for s in a.iter().filter_map(|v| v.as_str()) {
                    m.barriers.push((s.to_owned(), enable));
                }
            }
            "SetAIViewMultiplier" => {
                m.ai_view_mult = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "SetTeamAggressiveness" => {
                if let (Some(t), Some(val)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_f64()),
                ) {
                    if let Some((_, prev)) = m.ai_aggressiveness.iter_mut().find(|(k, _)| *k == t) {
                        *prev = val as f32;
                    } else {
                        m.ai_aggressiveness.push((t, val as f32));
                    }
                }
            }
            "AISnipeSuitabilityDist" => {
                m.ai_snipe_suitability = a.first().and_then(|v| v.as_f64()).map(|v| v as f32);
            }
            "AICanCaptureCP" => {
                if let (Some(post), Some(t), Some(can)) = (
                    a.first().and_then(|v| v.as_str()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(2).and_then(|v| v.as_bool()),
                ) {
                    if !can {
                        m.ai_no_capture.push((post.to_owned(), t));
                    }
                }
            }
            "SetBleedingVoiceOver" => {
                if let (Some(l), Some(t), Some(line)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(2).and_then(|v| v.as_str()),
                ) {
                    let flag = a.get(3).and_then(|v| v.is_truthy()).unwrap_or(false);
                    m.voice_bleeding.push((l, t, line.to_owned(), flag));
                }
            }
            "SetLowReinforcementsVoiceOver" => {
                if let (Some(l), Some(t), Some(line), Some(x)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(2).and_then(|v| v.as_str()),
                    a.get(3).and_then(|v| v.as_f64()),
                ) {
                    let flag = a.get(4).and_then(|v| v.is_truthy()).unwrap_or(false);
                    m.voice_low_reinforcements
                        .push((l, t, line.to_owned(), x as f32, flag));
                }
            }
            "SetOutOfBoundsVoiceOver" => {
                if let (Some(t), Some(line)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    m.voice_out_of_bounds.push((t, line.to_owned()));
                }
            }
            "SetAmbientMusic" => {
                if let (Some(t), Some(x), Some(mus), Some(st)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_f64()),
                    a.get(2).and_then(|v| v.as_str()),
                    a.get(3)
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                ) {
                    let flag = a.get(4).and_then(|v| v.is_truthy()).unwrap_or(false);
                    m.music_ambient
                        .push((t, x as f32, mus.to_owned(), st, flag));
                }
            }
            "SetVictoryMusic" => {
                if let (Some(t), Some(mus)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    m.music_victory.push((t, mus.to_owned()));
                }
            }
            "SetDefeatMusic" => {
                if let (Some(t), Some(mus)) = (
                    a.first()
                        .and_then(|v| v.as_i64())
                        .and_then(|i| u8::try_from(i).ok()),
                    a.get(1).and_then(|v| v.as_str()),
                ) {
                    m.music_defeat.push((t, mus.to_owned()));
                }
            }
            "SetClassProperty" => {
                if let (Some(cls), Some(prop), Some(val)) = (
                    a.first().and_then(|v| v.as_str()),
                    a.get(1).and_then(|v| v.as_str()),
                    a.get(2).and_then(|v| match v {
                        LuaVal::Str(s) => Some(s.clone()),
                        LuaVal::Num(n) => Some(if n.fract() == 0.0 {
                            format!("{}", *n as i64)
                        } else {
                            format!("{n}")
                        }),
                        LuaVal::Bool(b) => Some(if *b { "1" } else { "0" }.into()),
                        _ => None,
                    }),
                ) {
                    m.class_properties
                        .push((cls.to_owned(), prop.to_owned(), val));
                }
            }
            "AddCameraShot" => {
                if a.len() >= 7 && m.camera_shots.len() < 16 {
                    if let Some(shot) = a[..7]
                        .iter()
                        .map(|v| v.as_f64().map(|x| x as f32))
                        .collect::<Option<Vec<f32>>>()
                    {
                        m.camera_shots.push(shot.try_into().unwrap());
                    }
                }
            }
            "ObjectiveConquest:New" => has_obj_conquest = true,
            "ObjectiveAssault:New" | "ObjectiveSpaceAssault:New" => has_obj_assault = true,
            "ObjectiveTDM:New" => has_obj_tdm = true,
            "ObjectiveCTF:New" => {
                has_obj_ctf = true;
                if let Some(LuaVal::Table(tid)) = a.get(1) {
                    if let Some(tbl) = analysis.env.table(*tid) {
                        let score_limit = tbl
                            .get_str_ci("captureLimit")
                            .and_then(|v| v.as_i64())
                            .and_then(|i| u32::try_from(i.max(1)).ok());
                        let hide_cps = tbl
                            .get_str_ci("hideCPs")
                            .and_then(|v| v.is_truthy())
                            .unwrap_or(false);
                        objective_ctf_cfg = Some((score_limit, hide_cps));
                    }
                }
            }
            func if func.ends_with(":AddFlag") => {
                if let Some(LuaVal::Table(tid)) = a.get(1) {
                    if let Some(tbl) = analysis.env.table(*tid) {
                        let name = tbl
                            .get_str_ci("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned();
                        let home_region = tbl
                            .get_str_ci("homeRegion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned();
                        let capture_region = tbl
                            .get_str_ci("captureRegion")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned();
                        if !name.is_empty() {
                            ctf_flags.push(ParsedCtfFlag {
                                name,
                                home_region,
                                capture_region,
                                team_hint: None,
                                geometry: None,
                                carried_geometry: None,
                            });
                        }
                    }
                }
            }
            "ObjectiveOneFlagCTF:New" => {
                has_obj_1flag = true;
                if let Some(LuaVal::Table(tid)) = a.get(1) {
                    if let Some(tbl) = analysis.env.table(*tid) {
                        let str_field = |k: &str| {
                            tbl.get_str_ci(k)
                                .and_then(|v| v.as_str())
                                .map(str::to_owned)
                        };
                        let score_limit = tbl
                            .get_str_ci("captureLimit")
                            .and_then(|v| v.as_i64())
                            .and_then(|i| u32::try_from(i.max(1)).ok());
                        let hide_cps = tbl
                            .get_str_ci("hideCPs")
                            .and_then(|v| v.is_truthy())
                            .unwrap_or(false);
                        one_flag_cfg = Some(ParsedCtf {
                            two_flag: false,
                            score_limit,
                            hide_cps,
                            flag: str_field("flag").unwrap_or_default(),
                            home_region: str_field("homeRegion").unwrap_or_default(),
                            capture_region_att: str_field("captureRegionATT").unwrap_or_default(),
                            capture_region_def: str_field("captureRegionDEF").unwrap_or_default(),
                            cap_region_dummy_att: str_field("capRegionDummyATT"),
                            cap_region_dummy_def: str_field("capRegionDummyDEF"),
                            flags: Vec::new(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Build main teams (1..=2) and local teams (3..).
    for t in &team_order {
        let units = team_units.get(t).copied().unwrap_or(16);
        let classes = team_classes.get(t).cloned().unwrap_or_default();
        if *t <= 2 {
            let faction = team_names.get(t).cloned().unwrap_or_else(|| "all".into());
            let tickets = team_tickets.get(t).copied().unwrap_or(150);
            m.teams.push(ParsedTeam {
                team_idx: *t,
                faction,
                units,
                tickets,
                classes,
            });
        } else {
            m.local_teams.push((*t, units, classes));
        }
    }
    m.teams.sort_by_key(|t| t.team_idx);

    if has_obj_ctf {
        m.mode = "ctf".into();
        for f in &mut ctf_flags {
            if let Some((geom, carried, th)) = flag_props.get(&f.name.to_ascii_lowercase()) {
                f.geometry = geom.clone();
                f.carried_geometry = carried.clone();
                f.team_hint = *th;
            }
        }
        let (score_limit, hide_cps) = objective_ctf_cfg.unwrap_or((Some(5), true));
        let first_flag = ctf_flags
            .first()
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let first_home = ctf_flags
            .first()
            .map(|f| f.home_region.clone())
            .unwrap_or_default();
        let cap_att = ctf_flags
            .get(1)
            .map(|f| f.capture_region.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| ctf_flags.first().map(|f| f.home_region.clone()))
            .unwrap_or_default();
        let cap_def = ctf_flags
            .first()
            .map(|f| f.capture_region.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| ctf_flags.get(1).map(|f| f.home_region.clone()))
            .unwrap_or_default();
        m.ctf = Some(ParsedCtf {
            two_flag: true,
            score_limit,
            hide_cps,
            flag: first_flag,
            home_region: first_home,
            capture_region_att: cap_att,
            capture_region_def: cap_def,
            cap_region_dummy_att: None,
            cap_region_dummy_def: None,
            flags: ctf_flags,
        });
        if m.ctf.as_ref().is_some_and(|c| c.flags.len() < 2) {
            m.supported = false;
            m.unsupported_reason = Some("Two-flag CTF script declared fewer than 2 flags".into());
        }
    } else if has_obj_1flag {
        m.mode = "1flag".into();
        m.ctf = one_flag_cfg;
    } else if has_obj_conquest {
        m.mode = "con".into();
    } else if has_obj_assault {
        m.mode = "ass".into();
        m.scripted_mode = true;
    } else if has_obj_tdm {
        let lower = script_name.to_ascii_lowercase();
        m.mode = if lower.ends_with("_hunt") {
            "hunt".into()
        } else {
            "eli".into()
        };
    } else {
        let lower = script_name.to_ascii_lowercase();
        let suffix = lower
            .rsplit_once('_')
            .map(|(_, s)| s)
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric()));
        match suffix {
            Some("con") => m.mode = "con".into(),
            Some("eli") => m.mode = "eli".into(),
            Some("hunt") => m.mode = "hunt".into(),
            Some(custom) => {
                m.mode = custom.to_owned();
                m.scripted_mode = true;
            }
            None => {
                m.mode = "unknown".into();
                m.supported = false;
                m.unsupported_reason = Some(format!(
                    "Mission '{script_name}' uses an unrecognized custom game mode"
                ));
            }
        }
    }

    let global_ci = |name: &str| {
        analysis
            .env
            .globals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v)
    };
    if global_ci("SupportsCustomEraTeams")
        .and_then(LuaVal::is_truthy)
        .unwrap_or(false)
    {
        let t1 = global_ci("CustomEraTeam1")
            .and_then(LuaVal::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Team 1")
            .to_owned();
        let t2 = global_ci("CustomEraTeam2")
            .and_then(LuaVal::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Team 2")
            .to_owned();
        m.custom_team_names = Some([t1, t2]);
    }

    let lower_name = script_name.to_ascii_lowercase();
    m.era = if let Some(era) = declared_era {
        era.to_owned()
    } else if eras_seen.contains("cw") && !eras_seen.contains("gcw") {
        "cw".into()
    } else if eras_seen.contains("gcw") && !eras_seen.contains("cw") {
        "gcw".into()
    } else if lower_name.contains("c_") {
        "cw".into()
    } else {
        "gcw".into()
    };

    if m.teams.len() < 2 && m.supported {
        m.supported = false;
        m.unsupported_reason = Some(format!(
            "Mission '{script_name}' did not declare two playable teams"
        ));
    }

    m
}
