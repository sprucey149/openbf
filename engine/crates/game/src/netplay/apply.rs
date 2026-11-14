//! The deterministic game-layer edits of a lockstep round (LAN §4.3, §4.4):
//! the round's humans installed from [`RoundSetup`], session commands applied
//! before a tick, and the per-slot class gate and medal points around it.
//!
//! Rules:
//! - Everything here writes the sim only from the round setup, the bundle
//!   and sim-derived state (the event feed's round stats, which every peer
//!   builds from the same ticks), so every peer makes the same writes. The
//!   local player's own profile and the viewer never enter it.
//! - Per tick, in order: the bundle's commands ([`apply`]), the class gate of
//!   every human slot ([`gate`]), the step, then every human's medal points
//!   and the hero scores ([`score`]). [`step`] does all of it.
//! - A joiner's round points count from its join: a unit index a joiner
//!   takes over may carry a bot's round points, so the gate subtracts what
//!   the index held on the first gate after the join.
//! - Single player never calls this module: its gate and scores stay where
//!   they were (`unlocks::set_player_gate`, `heroes::tick`).

use super::{cheats_from_bits, GameCommand, PlayerProfile, RoundSetup, ALL_SLOTS};
use crate::event_feed::EventFeed;
use crate::walk::WalkSim;
use net::{SessionCommand, TickBundle, TEAM_AUTO};
use std::collections::BTreeMap;

/// The lockstep round's per-slot game state, kept in `WalkSim::netplay`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoundState {
    /// Each human slot's profile, for its class costs.
    pub profiles: BTreeMap<u8, PlayerProfile>,
    /// Each human slot's round points before it joined (`None`: taken on
    /// the next gate).
    pub baseline: BTreeMap<u8, Option<i32>>,
    /// The cheats guests may ask for.
    pub cheat_policy: u32,
    /// The host's award policy is Off: no slot's earned awards apply
    /// (`RoundSetup::awards`, `GameCommand::SetAwardPolicy`).
    pub awards_off: bool,
    /// `EndRound` applied.
    pub ended: bool,
    /// A lockstep round (set by [`install_humans`]).
    pub active: bool,
}

/// What one command did, for the harness and the HUD's notices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    Joined {
        slot: u8,
        ok: bool,
    },
    /// `bot`: the bot that took the leaver's place (none when the leaver was
    /// dead or behind the spawn screens).
    Left {
        slot: u8,
        bot: Option<core::unit::UnitHandle>,
    },
    Game(Option<GameCommand>),
    Ended,
}

/// Give `slot` its profile's sim inputs: assist, medal career, award
/// bonuses, cheats, class costs.
fn install_profile(walk: &mut WalkSim, slot: u8, profile: PlayerProfile, cheats: u32) {
    let s = usize::from(slot);
    let space = walk.space_battle();
    walk.sim.set_slot_cheats(s, Some(cheats_from_bits(cheats)));
    walk.sim
        .set_slot_assist(s, Some(profile.assist.to_sim(space)));
    if let Some(Some(life)) = walk.sim.players.get(s).copied() {
        walk.sim.install_slot_medal_career(s, life, profile.medals);
        walk.sim
            .set_slot_career_award_bonuses(s, profile.award_bonuses && !walk.netplay.awards_off);
    }
    walk.netplay.profiles.insert(slot, profile);
    walk.netplay.baseline.insert(slot, None);
}

/// Seat the round-start humans of `setup` into a freshly built battle
/// (`make_walk` + the spawn screens of slot 0): slot 0 is the battle's own
/// player; every other human joins behind the spawn screens
/// ([`core::sim::Sim::join_human`]). Session cheats are off; each human
/// plays under its own.
pub fn install_humans(walk: &mut WalkSim, setup: &RoundSetup) -> anyhow::Result<()> {
    walk.netplay = RoundState {
        cheat_policy: setup.cheat_policy,
        awards_off: !setup.awards,
        active: true,
        ..Default::default()
    };
    walk.sim.cheats = core::sim::Cheats::default();
    walk.sim.team_handicap_on = setup.team_handicap;
    let mut seats: Vec<_> = setup.humans.iter().collect();
    seats.sort_by_key(|h| h.slot);
    for h in seats {
        if h.slot != 0 {
            walk.sim
                .join_human(usize::from(h.slot), h.team.filter(|t| *t < 2))
                .map_err(|e| anyhow::anyhow!("human {} (slot {}): {e}", h.name, h.slot))?;
        }
        install_profile(walk, h.slot, h.profile.clone(), h.cheats);
    }
    let local = walk.local_slot;
    walk.set_local_slot(local);
    Ok(())
}

/// Apply one session command before its tick's step (every peer alike).
pub fn apply(walk: &mut WalkSim, cmd: &SessionCommand) -> Applied {
    match cmd {
        SessionCommand::AddHuman {
            slot,
            team,
            profile,
        } => {
            let team = (*team != TEAM_AUTO && *team < 2).then_some(*team);
            let ok = walk.sim.join_human(usize::from(*slot), team).is_ok();
            if ok {
                install_profile(walk, *slot, PlayerProfile::decode_or_default(profile), 0);
                if *slot == walk.local_slot {
                    walk.set_local_slot(*slot);
                }
            }
            Applied::Joined { slot: *slot, ok }
        }
        SessionCommand::RemoveHuman { slot } => {
            let bot = walk.sim.leave_human(usize::from(*slot));
            walk.netplay.profiles.remove(slot);
            walk.netplay.baseline.remove(slot);
            Applied::Left { slot: *slot, bot }
        }
        SessionCommand::Game(bytes) => {
            let cmd = GameCommand::decode(bytes);
            if let Some(c) = &cmd {
                apply_game(walk, c);
            }
            Applied::Game(cmd)
        }
        SessionCommand::EndRound => {
            walk.netplay.ended = true;
            Applied::Ended
        }
    }
}

/// The human slots a command addresses.
fn slots_of(walk: &WalkSim, slot: u8) -> Vec<u8> {
    if slot == ALL_SLOTS {
        walk.netplay.profiles.keys().copied().collect()
    } else {
        vec![slot]
    }
}

/// Apply a decoded [`GameCommand`].
pub fn apply_game(walk: &mut WalkSim, cmd: &GameCommand) {
    match *cmd {
        GameCommand::SetSlotCheats { slot, bits } => {
            for s in slots_of(walk, slot) {
                walk.sim
                    .set_slot_cheats(usize::from(s), Some(cheats_from_bits(bits)));
            }
        }
        GameCommand::SetSlotCheat { slot, id, on } => {
            if id >= super::CHEAT_COUNT {
                return;
            }
            for s in slots_of(walk, slot) {
                let mut bits = super::cheat_bits(&walk.sim.cheats_for_slot(usize::from(s)));
                if on {
                    bits |= 1 << id;
                } else {
                    bits &= !(1 << id);
                }
                walk.sim
                    .set_slot_cheats(usize::from(s), Some(cheats_from_bits(bits)));
            }
        }
        GameCommand::SetCheatPolicy { allowed } => {
            walk.netplay.cheat_policy = allowed;
            let slots: Vec<usize> = (0..walk.sim.players.len()).collect();
            for s in slots {
                let bits = super::cheat_bits(&walk.sim.cheats_for_slot(s));
                if bits & !allowed != 0 {
                    walk.sim
                        .set_slot_cheats(s, Some(cheats_from_bits(bits & allowed)));
                }
            }
            let session = super::cheat_bits(&walk.sim.cheats) & allowed;
            walk.sim.cheats = cheats_from_bits(session);
        }
        GameCommand::SetTeamSize { pct } => walk.set_team_size_percent(pct),
        GameCommand::SetReinforcements { pct } => walk.reinforcement_pct = pct.clamp(10, 500),
        GameCommand::SetCareerAwards { slot, on } => {
            let allowed = !walk.netplay.awards_off;
            for s in slots_of(walk, slot) {
                if let Some(p) = walk.netplay.profiles.get_mut(&s) {
                    p.award_bonuses = on;
                }
                walk.sim
                    .set_slot_career_award_bonuses(usize::from(s), on && allowed);
            }
        }
        GameCommand::SetAwardPolicy { on } => {
            walk.netplay.awards_off = !on;
            let slots: Vec<(u8, bool)> = walk
                .netplay
                .profiles
                .iter()
                .map(|(s, p)| (*s, p.award_bonuses))
                .collect();
            for (s, own) in slots {
                walk.sim
                    .set_slot_career_award_bonuses(usize::from(s), on && own);
            }
        }
    }
}

/// Every human slot's class screen gate before a step: its round points
/// (from its join) and its class costs from its profile's counters.
pub fn gate(walk: &mut WalkSim, round: Option<&crate::round_stats::RoundStats>) {
    let slots: Vec<u8> = walk.netplay.profiles.keys().copied().collect();
    for slot in slots {
        let s = usize::from(slot);
        let Some(Some(h)) = walk.sim.players.get(s).copied() else {
            continue;
        };
        let raw = round
            .and_then(|r| r.units.get(h.index as usize))
            .map_or(0, |u| u.points);
        let base = walk.netplay.baseline.entry(slot).or_insert(None);
        let base = *base.get_or_insert(raw);
        walk.sim.set_spawn_points(s, raw - base);
        let kills = walk.netplay.profiles[&slot].unlock_kills;
        crate::unlocks::apply_slot_costs(walk, s, &|u| {
            crate::unlocks::UNLOCKABLES
                .iter()
                .position(|x| x.key == u.key)
                .map_or(0, |i| kills[i])
        });
    }
}

/// After a step: the feed reads the tick, every human's positive points feed
/// its medal career, and the hero manager gets the round's scores.
pub fn score(walk: &mut WalkSim, feed: &mut EventFeed) {
    feed.tick(&walk.sim, walk.player);
    let slots: Vec<u8> = walk.netplay.profiles.keys().copied().collect();
    for slot in slots {
        if let Some(Some(h)) = walk.sim.players.get(usize::from(slot)).copied() {
            let points = feed.positive_points(h);
            walk.sim.score_medal_points(h, points);
        }
    }
    crate::heroes::scores(&mut walk.sim, feed);
}

/// One lockstep tick: commands, gate, step, scores, then `each` (handed the
/// feed back, scored). Returns what the commands did.
pub fn step(
    walk: &mut WalkSim,
    bundle: &TickBundle,
    mut feed: Option<&mut EventFeed>,
    each: impl FnOnce(&mut WalkSim, Option<&mut EventFeed>),
) -> Vec<Applied> {
    let applied: Vec<Applied> = bundle.commands.iter().map(|c| apply(walk, c)).collect();
    gate(walk, feed.as_deref().map(EventFeed::round));
    walk.step_bundle(bundle, |w| {
        if let Some(f) = feed.as_deref_mut() {
            score(w, f);
        }
        each(w, feed);
    });
    applied
}
