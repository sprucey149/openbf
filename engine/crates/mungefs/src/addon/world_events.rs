//! Compile named environment callbacks into finite, declarative action lists.

use std::collections::{BTreeMap, BTreeSet};

use crate::lua50::{LuaCall, LuaVal, MissionScriptAnalysis};

use super::{WorldAction, WorldEffectAnchor, WorldEvents};

const MAX_ACTIONS: usize = 2048;
const MAX_TIMER_DEPTH: usize = 8;

struct Compiler<'a> {
    analysis: &'a MissionScriptAnalysis,
    remaining: usize,
    translated: BTreeSet<String>,
}

impl Compiler<'_> {
    fn actions(&mut self, calls: &[LuaCall], depth: usize) -> Vec<WorldAction> {
        let mut out = Vec::new();
        let mut times = BTreeMap::new();
        let mut started = BTreeSet::new();
        if depth > MAX_TIMER_DEPTH {
            return out;
        }
        for call in calls {
            if self.remaining == 0 {
                break;
            }
            let name = call.args.first().and_then(LuaVal::as_str);
            let args = &call.args;
            let action = match call.func.as_str() {
                "PlayAnimation" => name.map(|s| WorldAction::PlayAnimation(s.to_owned())),
                "PauseAnimation" => name.map(|s| WorldAction::PauseAnimation(s.to_owned())),
                // Load-time scene changes already use the ordinary mission setup.
                "KillObject" if depth > 0 => name.map(|s| WorldAction::KillObject(s.to_owned())),
                "EnableBarriers" | "DisableBarriers" if depth > 0 => {
                    name.map(|s| WorldAction::Barrier(s.to_owned(), call.func == "EnableBarriers"))
                }
                "BlockPlanningGraphArcs" | "UnblockPlanningGraphArcs" if depth > 0 => {
                    for name in args.iter().filter_map(LuaVal::as_str).take(self.remaining) {
                        out.push(WorldAction::Arc(
                            name.to_owned(),
                            call.func == "BlockPlanningGraphArcs",
                        ));
                        self.remaining -= 1;
                    }
                    None
                }
                "AttachEffectToMatrix" => match args.as_slice() {
                    [LuaVal::Effect(effect), LuaVal::MatrixObject(object)] => {
                        Some(WorldAction::Effect(
                            effect.clone(),
                            WorldEffectAnchor::Object(object.clone()),
                        ))
                    }
                    [LuaVal::Effect(effect), LuaVal::MatrixPath(path, index)] => {
                        Some(WorldAction::Effect(
                            effect.clone(),
                            WorldEffectAnchor::PathPoint(path.clone(), *index),
                        ))
                    }
                    _ => None,
                },
                "SetTimerValue" => {
                    if let Some((timer, seconds)) = name.zip(args.get(1).and_then(LuaVal::as_f64)) {
                        if (0.0..=3600.0).contains(&seconds) {
                            times.insert(timer, seconds as f32);
                        }
                    }
                    None
                }
                "StartTimer" => {
                    if let Some(timer) = name {
                        started.insert(timer);
                    }
                    None
                }
                "OnTimerElapse" => {
                    let timer = args.get(1).and_then(LuaVal::as_str);
                    let timer = timer.filter(|timer| started.contains(timer));
                    let seconds = timer.and_then(|timer| times.get(timer)).copied();
                    if let Some((seconds, callback)) = seconds.zip(args.first()) {
                        self.remaining -= 1;
                        let nested =
                            self.actions(&self.analysis.callback_calls(callback), depth + 1);
                        if !nested.is_empty() {
                            self.translated.extend(
                                ["OnTimerElapse", "CreateTimer", "StartTimer"].map(str::to_owned),
                            );
                            out.push(WorldAction::After(seconds, nested));
                        }
                    }
                    None
                }
                _ => None,
            };
            if let Some(action) = action {
                self.remaining -= 1;
                out.push(action);
            }
        }
        out
    }
}

pub(super) fn compile(analysis: &MissionScriptAnalysis) -> (WorldEvents, BTreeSet<String>) {
    let mut compiler = Compiler {
        analysis,
        remaining: MAX_ACTIONS,
        translated: BTreeSet::new(),
    };
    let mut calls = analysis.init_calls.clone();
    calls.extend_from_slice(&analysis.postload_calls);
    let start = compiler.actions(&calls, 0);
    let mut object_destroyed = Vec::new();
    for call in calls.iter().filter(|call| call.func == "OnObjectKillName") {
        if let [callback, LuaVal::Str(name)] = call.args.as_slice() {
            if compiler.remaining == 0 {
                break;
            }
            let actions = compiler.actions(&analysis.callback_calls(callback), 1);
            if !actions.is_empty() {
                object_destroyed.push((name.clone(), actions));
                compiler.translated.insert(call.func.clone());
            }
        }
    }
    (
        WorldEvents {
            start,
            object_destroyed,
        },
        compiler.translated,
    )
}
