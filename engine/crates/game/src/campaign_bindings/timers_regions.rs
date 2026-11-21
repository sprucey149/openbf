//! Timer, region, path and marker bindings: script-visible handles into the
//! world state owned by `campaign_runtime`.

use super::*;

impl Ctx<'_> {
    pub(super) fn timers_regions(
        &mut self,
        _it: &mut Interp,
        name: &str,
        a: &[Value],
    ) -> Option<Ret> {
        let sim = &mut *self.sim;
        let w = &mut *self.w;
        Some(match name {
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
                    return Some(if name == "GetTimerValue" {
                        one(Value::Nil)
                    } else {
                        nothing()
                    });
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
                    _ => return Some(one(Value::Num(t.value))),
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
                let target = marker_target(w, sim, name, a);
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
                let target = marker_target(w, sim, name, a);
                let team = bf2_team(arg(a, 1));
                if let Some(target) = target {
                    w.markers
                        .retain(|m| !(m.target == target && team.is_none_or(|t| t == m.team)));
                }
                nothing()
            }

            _ => return None,
        })
    }
}
