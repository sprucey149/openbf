//! AI-goal, reinforcement, victory, class, spawn, hero and AI-tuning
//! bindings: the calls that change the sim rather than the HUD.

use super::*;

impl Ctx<'_> {
    pub(super) fn sim_ai(&mut self, it: &mut Interp, name: &str, a: &[Value]) -> Option<Ret> {
        let sim = &mut *self.sim;
        let w = &mut *self.w;
        Some(match name {
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
                    return Some(one(Value::Nil));
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
                if let (Some(delay), Some(interval), Some(t)) =
                    (num(a, 0), num(a, 1), sim_team(arg(a, 2)).filter(|t| *t < 2))
                {
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
                    // A stale `post_keys` name can resolve past the live CPs.
                    if let Some(cp) = sim.cps.get_mut(ci) {
                        let ban = &mut cp.ai_capture_ban;
                        *ban = if arg(a, 2).truthy() {
                            *ban & !bit
                        } else {
                            *ban | bit
                        };
                    }
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
                        if let Some(idx) =
                            v.num().map(|n| n as i32).filter(|&i| (1..=8).contains(&i))
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
                    let level = arg(a, 2)
                        .as_str()
                        .map(|s| match s.to_ascii_lowercase().as_str() {
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

            _ => return None,
        })
    }
}
