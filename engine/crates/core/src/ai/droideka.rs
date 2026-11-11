use super::state::{AiState, Behaviour};
use crate::droideka::{Form, Order};
use crate::sim::Sim;
use crate::unit::Unit;
use crate::vehicle::VehicleKind;

pub const SIGHT_FIGHT: f32 = 10.0;
pub const SHIELD_RAISE: f32 = 0.5;

/// The form a bot droideka is told to take: deployed to fight, rolled
/// otherwise (`analysis/re-droideka-unit.md` 5.1).
///
/// OURS (2026-11-10): a droideka in Recover rolls too. The original deploys
/// only for its stuck walk, a short walk to a point within 10 m that ends
/// 2 m out, and rolls again. The port's Recover is not that walk: it keeps
/// the leg to the goal until a progress window passes. Deployed, the slow
/// walking form kept failing that window, so bot droidekas spent a third of
/// their foot time in Recover (other classes 0.7-2.7 %) and stood in enemy
/// posts a third as often as the other CIS classes
/// (`analysis/faction-balance-2026-11.md` section 3).
/// measure: droideka Recover share, enemy-post time and both CW pairings'
/// bot-only win share against that note.
pub fn form_order(st: &AiState) -> Order {
    match st.behaviour {
        Behaviour::Fight => Order::Deploy,
        _ => Order::Roll,
    }
}

pub(super) fn takes_fight(sim: &Sim, u: &Unit, st: &AiState, e: &Unit, range: f32) -> bool {
    if u.droideka.is_none() {
        return true;
    }
    let flyer = e
        .mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .is_some_and(|v| v.kind == VehicleKind::Flyer);
    if flyer {
        return false;
    }
    let hit = u
        .last_hit
        .is_some_and(|(_, t)| sim.tick.saturating_sub(t) < u64::from(crate::TICK_HZ));
    range <= SIGHT_FIGHT || hit || st.behaviour == Behaviour::Fight
}

pub(super) struct DrivePlan {
    pub stop: bool,
    pub raise_shield: bool,
}

pub(super) fn drive_plan(u: &Unit, st: &AiState) -> Option<DrivePlan> {
    let d = u.droideka.as_deref()?;
    let c = u.droideka_class()?;
    let stop = st.behaviour == Behaviour::Fight && d.form() == Form::Walk && st.target.is_some();
    let raise_shield =
        stop && !d.in_transform() && !d.shield.on && d.shield.points > SHIELD_RAISE * c.shield.max;
    Some(DrivePlan { stop, raise_shield })
}

#[cfg(test)]
mod tests;
