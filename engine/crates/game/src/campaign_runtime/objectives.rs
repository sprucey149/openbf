//! HUD-facing objective, popup and team-point state. The bindings write this
//! state through [super::World]; the facade and the battle read it back.

use super::*;

impl CampaignRuntime {
    /// The popup up now (the battle pauses under it).
    pub fn popup(&self) -> Option<&Popup> {
        self.w.popups.front()
    }

    /// Dev referee: the script's objective table `name` runs its own
    /// `Complete` for team 1, so a test reaches a later objective set the way
    /// play does. `false` when the table or its method is missing or fails.
    pub fn complete_objective(&mut self, sim: &mut Sim, name: &str) -> bool {
        let this = self.interp.global(name);
        let mut host = Ctx {
            w: &mut self.w,
            sim,
        };
        let Ok(f) = self.interp.index(&mut host, &this, &Value::str("Complete")) else {
            return false;
        };
        self.interp
            .call_value(&mut host, &f, vec![this, Value::Num(1.0)])
            .is_ok()
    }

    /// The player dismissed the popup (`FUN_0038cb28`).
    pub fn close_popup(&mut self) {
        self.w.popups.pop_front();
    }

    /// The player's press closes the popup; a live class screen it hid
    /// comes back without taking that press ([`Sim::hold_class_screen_accept`]).
    pub fn dismiss_popup(&mut self, sim: &mut Sim) {
        sim.hold_class_screen_accept();
        self.close_popup();
    }

    /// `player1.objectivesUpdated` is up: ActivateObjective posts it 0.25 s
    /// after and its disable 6.0 s after (`FUN_0038abd8`).
    pub fn objectives_updated(&self) -> bool {
        self.w.updated_on
    }

    pub fn objectives(&self) -> &[ObjectiveLine] {
        &self.w.objectives
    }

    pub fn show_details(&mut self, index: usize) {
        if let Some(o) = self.w.objectives.get(index) {
            self.w.popups.push_back(Popup::Objective(
                o.popup.as_ref().unwrap_or(&o.text).clone(),
            ));
        }
    }

    pub fn show_hint(&mut self) {
        if let Some(hint) = self.w.hints.last() {
            self.w.popups.push_back(Popup::Hint(hint.clone()));
        }
    }

    pub fn hints(&self) -> &[String] {
        &self.w.hints
    }

    /// The teams `ShowTeamPoints` turned on, and their points.
    pub fn team_points(&self) -> Option<[i32; 2]> {
        (self.w.show_points[0] || self.w.show_points[1]).then_some(self.w.team_points)
    }

    /// True when the script enabled CTF flag HUD rules via
    /// `SetFlagGameplayType("1flag" | "2flag")`.
    pub fn flag_ctf_hud(&self) -> bool {
        matches!(
            self.w.flag_gameplay.as_deref(),
            Some("1flag" | "2flag")
        )
    }

    pub fn command_posts_hidden(&self) -> bool {
        self.w.hide_cps
    }

    /// The timer `ShowTimer` put on screen.
    pub fn time_left(&self) -> Option<f32> {
        let t = self.w.timers.get(self.w.shown_timer? as usize)?;
        t.alive.then_some(t.value.fmax(0.0))
    }
}
