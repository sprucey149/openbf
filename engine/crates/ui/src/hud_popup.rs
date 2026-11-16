//! The mission script's popups and the "Objectives Updated!" reminder
//! (`crate::hud::HudModel::popup`, `objectives_updated`) as the 1-player
//! layout's events.

use crate::hud_tree::{EventValue, Hud};

/// A mission popup (`FUN_0038c7a0`'s kinds: 1 hint, 2 objective, 3
/// selection).
#[derive(Debug, Clone, PartialEq)]
pub enum HudPopup {
    Hint(String),
    Objective(String),
    Selection(String),
}

/// `FUN_0038c7a0` posts the popup's event with its text, then
/// `player%d.pressSelectToReturn.disable`, `player%d.objectiveDetails.disable`
/// and `player%d.hintsAvailable.disable`; closing it (`FUN_0038cb28`) posts
/// the popup's `.disable`. `player%d.objectivesUpdated` and its disable come
/// from `FUN_0038abd8`'s delayed posts.
pub(crate) fn feed(
    hud: &mut Hud,
    popup: Option<&HudPopup>,
    objectives_updated: bool,
    has_mesh: &dyn Fn(&str) -> bool,
) {
    for (event, text) in [
        (
            "hintPopup",
            match popup {
                Some(HudPopup::Hint(t)) => Some(t),
                _ => None,
            },
        ),
        (
            "objectivePopup",
            match popup {
                Some(HudPopup::Objective(t)) => Some(t),
                _ => None,
            },
        ),
        (
            "selectionPopup",
            match popup {
                Some(HudPopup::Selection(t)) => Some(t),
                _ => None,
            },
        ),
    ] {
        let gate = format!("hud.gate.{event}");
        match text {
            Some(t) => {
                let opened = hud.last(&gate) != Some(&EventValue::Number(1.0));
                hud.falls(&gate, true);
                hud.publish(event, EventValue::Text(t.clone()), has_mesh);
                if opened {
                    for e in [
                        "player1.pressSelectToReturn.disable",
                        "player1.objectiveDetails.disable",
                        "player1.hintsAvailable.disable",
                    ] {
                        hud.send(e, EventValue::None, has_mesh);
                    }
                }
            }
            None => {
                if hud.falls(&gate, false) {
                    hud.forget(event);
                    hud.send(&format!("{event}.disable"), EventValue::None, has_mesh);
                }
            }
        }
    }
    if objectives_updated {
        hud.falls("hud.gate.objectivesUpdated", true);
        hud.publish("player1.objectivesUpdated", EventValue::None, has_mesh);
    } else if hud.falls("hud.gate.objectivesUpdated", false) {
        hud.forget("player1.objectivesUpdated");
        hud.send(
            "player1.objectivesUpdated.disable",
            EventValue::None,
            has_mesh,
        );
    }
}
