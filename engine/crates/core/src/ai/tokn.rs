//! Card TOKN (D24, OURS): damage pacing against HUMAN players - hit tokens.
//! Each human has a budget of tokens per difficulty ([`budget`]); a bot's
//! shots at him can hit only while it holds one of his. The rest still fire
//! at him, but aim beside him ([`miss_offset`]: a near miss, seen and heard)
//! and lob their grenades short of him ([`short_of`]). Their bolts and blasts
//! stay real: one that meets him anyway hurts him (the user, 2026-10-28: no
//! fire passes through a soldier), and is counted (`tokn_stray`). Bot-vs-bot
//! fire is untouched, and so are bots in vehicles and turrets.
//!
//! A token is held for the tier's `hold`, then given to the best-placed
//! shooter: an enemy bot on foot whose target is the human, with a clear line
//! from its muzzle, the nearest - weighted by how nearly it already aims at
//! him - and the lowest unit index on a tie. With nobody else waiting the
//! holder keeps it. Dealt once a tick before the thinks ([`deal`]), in the
//! sim's fixed order, and hashed. Card DFOCUS: at Hard and Elite the holders
//! are his focusers - a bot that sees him may take one without targeting him
//! yet, then makes him its target, and keeps it a short while after losing
//! sight (`super::dfocus`).
//!
//! How it combines: the aim error model (`gunner_foot::seen_aim`, the PS2's
//! decaying error by skill) still decides whether a token holder's shot hits;
//! a token only allows it. A holder's hit does the weapon's own damage: the
//! D12 human-damage multiplier is retired (card DRETIRE, the maintainer 2026-10-28). So
//! the tier's danger to a human is (holders at once) x (their hit chance).

use crate::fmath::FloatExt;
use crate::sim::Sim;
use crate::skill::Difficulty;
use crate::unit::{Controller, UnitHandle};
use glam::Vec3;

/// One tier's row: tokens per human (`None`: off, every bot's shot is real),
/// how long one is held (ticks), how far beside him a bot without one aims
/// (m, at his chest).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub tokens: Option<u8>,
    pub hold: u64,
    pub miss: f32,
}

const S: u64 = crate::TICK_HZ as u64;

/// The table. PROVISIONAL (ours; the user's values after the first play,
/// 2026-10-28: Easy 2, Normal 3, Hard 5, Elite 7 - capped a little above
/// Hard so the whole map never converges on a human, card DFOCUS). Elite's
/// hold and miss continue the trend (ours).
pub const BUDGETS: [(Difficulty, Budget); 4] = [
    (
        Difficulty::Easy,
        Budget {
            tokens: Some(2),
            hold: 5 * S,
            miss: 1.8,
        },
    ),
    (
        Difficulty::Normal,
        Budget {
            tokens: Some(3),
            hold: 4 * S,
            miss: 1.4,
        },
    ),
    (
        Difficulty::Hard,
        Budget {
            tokens: Some(5),
            hold: 3 * S,
            miss: 1.0,
        },
    ),
    (
        Difficulty::Elite,
        Budget {
            tokens: Some(7),
            hold: 2 * S,
            miss: 0.7,
        },
    ),
];

/// The row for `d`.
pub fn budget(d: Difficulty) -> Budget {
    BUDGETS
        .iter()
        .find(|(t, _)| *t == d)
        .map_or(BUDGETS[3].1, |(_, b)| *b)
}

/// One token: `bot` may hit `human`, since tick `since`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub human: UnitHandle,
    pub bot: UnitHandle,
    pub since: u64,
}

impl Token {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for h in [self.human, self.bot] {
            eat(&h.index.to_le_bytes());
            eat(&h.gen.to_le_bytes());
        }
        eat(&self.since.to_le_bytes());
    }
}

/// The budget the bots attacking `human` work to: their team's tier.
fn budget_against(sim: &Sim, human_team: u8) -> Budget {
    budget(sim.difficulty_of(1 - human_team.min(1)))
}

/// Is `bot` a shooter the tokens govern against `human`: a bot on foot of
/// the other team, `human` a human player, the budget on?
fn governed(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> Option<Budget> {
    let (b, h) = (sim.unit(bot)?, sim.unit(human)?);
    let on = h.is_human()
        && b.controller == Controller::Ai
        && b.mount.is_none()
        && sim.hostile(b.team, h.team);
    let budget = budget_against(sim, h.team);
    (on && budget.tokens.is_some()).then_some(budget)
}

/// Does `bot` hold one of `human`'s tokens?
pub fn holds(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> bool {
    sim.tokens.iter().any(|t| t.bot == bot && t.human == human)
}

/// Must `bot` aim its fire off `human`: governed and holding no token.
pub fn bars(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> bool {
    governed(sim, bot, human).is_some() && !holds(sim, bot, human)
}

/// Where a bot without a token aims instead of `at` (from `eye`): beside
/// him by the tier's `miss`, a little high, always on the same side for the
/// same bot (by unit index): a bot that changed sides would sweep its aim,
/// and its shots, across him. `None` when its shots may hit.
pub(crate) fn miss_offset(
    sim: &Sim,
    bot: UnitHandle,
    human: UnitHandle,
    eye: Vec3,
    at: Vec3,
) -> Option<Vec3> {
    let b = governed(sim, bot, human).filter(|_| !holds(sim, bot, human))?;
    let right = (at - eye).cross(Vec3::Y).normalize_or(Vec3::X);
    let side = if bot.index & 1 == 0 { 1.0 } else { -1.0 };
    Some(right * side * b.miss + Vec3::Y * 0.3)
}

/// How far short of `human`'s feet a bot without a token lobs a grenade,
/// past the tier's near miss (m). PROVISIONAL (ours).
pub const GRENADE_SHORT: f32 = 4.0;

/// Where a bot without a token lands a grenade instead of `feet` (thrown
/// from `eye`): pulled back toward the thrower by the tier's `miss` plus
/// [`GRENADE_SHORT`], over the ground. `None` when it may throw at him.
pub(crate) fn short_of(
    sim: &Sim,
    bot: UnitHandle,
    human: UnitHandle,
    eye: Vec3,
    feet: Vec3,
) -> Option<Vec3> {
    let b = governed(sim, bot, human).filter(|_| !holds(sim, bot, human))?;
    let back = Vec3::new(eye.x - feet.x, 0.0, eye.z - feet.z);
    let d = back.length();
    let pull = (b.miss + GRENADE_SHORT).fmin(0.5 * d);
    Some(back / d.fmax(1e-3) * pull)
}

/// How well `bot` is placed to take one of `human`'s tokens (lower is
/// better), or `None`: not a shooter at him (its target is someone else, no
/// clear line from its muzzle).
fn placed(sim: &Sim, bot: UnitHandle, human: UnitHandle) -> Option<f32> {
    governed(sim, bot, human)?;
    let (b, h) = (sim.unit(bot)?, sim.unit(human)?);
    // Card DFOCUS: at Hard and Elite a bot that sees him may take one to
    // make him its target.
    if b.ai.target != Some(human.into()) && !super::dfocus::may_focus(sim, bot, human) {
        return None;
    }
    let (eye, c) = (b.muzzle(), h.capsule().center());
    // The line passes his own vehicle's hull and his turret's building: a
    // human in a seat can be shot at by a holder (card DFOCUS).
    let clear = crate::perception::los_seat(
        sim,
        eye,
        c,
        crate::perception::past_mounts(sim, bot, human),
        crate::perception::past_buildings(sim, bot, human),
    );
    if !clear {
        return None;
    }
    let to = c - eye;
    let d = to.length();
    // Already aiming at him: up to half the distance off.
    let aim = b.ai.aim_dir.dot(to / d.fmax(1e-3)).clamp(0.0, 1.0);
    Some(d * (1.0 - 0.5 * aim))
}

/// Deal the tokens for this tick, before the thinks: drop those whose
/// holder or human is gone or whose holder no longer shoots at him, then
/// fill each human's free tokens and the expired ones with the best-placed
/// shooters (an expired holder keeps his with nobody else waiting).
pub fn deal(sim: &mut Sim) {
    super::dfocus::upkeep(sim);
    let humans: Vec<(UnitHandle, Budget)> = sim
        .alive_units()
        .filter(|(_, u)| u.is_human())
        .map(|(h, u)| (h, budget_against(sim, u.team)))
        .collect();
    let mut tokens = std::mem::take(&mut sim.tokens);
    tokens.retain(|t| {
        humans
            .iter()
            .any(|(h, b)| *h == t.human && b.tokens.is_some())
            && (placed(sim, t.bot, t.human).is_some()
                || super::dfocus::remembers(sim, t.bot, t.human))
    });
    for (human, b) in humans {
        let Some(n) = b.tokens else {
            continue;
        };
        // The shooters at him without a token, best placed first.
        let mut wait: Vec<(f32, UnitHandle)> = sim
            .alive_units()
            .filter(|(h, _)| !tokens.iter().any(|t| t.bot == *h && t.human == human))
            // Card DFOCUS: a focuser of another human is not offered his.
            .filter(|(h, u)| {
                u.ai.target == Some(human.into()) || !tokens.iter().any(|t| t.bot == *h)
            })
            .filter_map(|(h, _)| placed(sim, h, human).map(|s| (s, h)))
            .collect();
        wait.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.index.cmp(&b.1.index)));
        let mut wait = wait.into_iter().map(|(_, h)| h);
        // Expired tokens go round, oldest first.
        let mut mine: Vec<usize> = (0..tokens.len())
            .filter(|&k| tokens[k].human == human)
            .collect();
        mine.sort_by_key(|&k| (tokens[k].since, tokens[k].bot.index));
        for k in mine {
            if sim.tick >= tokens[k].since + b.hold {
                if let Some(bot) = wait.next() {
                    tokens[k].bot = bot;
                }
                tokens[k].since = sim.tick;
            }
        }
        let held = tokens.iter().filter(|t| t.human == human).count();
        for bot in wait.take(usize::from(n).saturating_sub(held)) {
            tokens.push(Token {
                human,
                bot,
                since: sim.tick,
            });
        }
    }
    sim.tokens = tokens;
}

#[cfg(test)]
mod tests;
