//! Objective-list, popup, team-point and command-post-display bindings.

use super::*;

impl Ctx<'_> {
    pub(super) fn objectives(&mut self, _it: &mut Interp, name: &str, a: &[Value]) -> Option<Ret> {
        let w = &mut *self.w;
        Some(match name {
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

            _ => return None,
        })
    }
}
