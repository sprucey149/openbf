//! Which ability a bot uses: officer support, the remote droid (card DROIDAI)
//! and the auto-turret (card ATUR, the BF2 cluster rule), pressed through
//! secondary fire and weapon switch. Follows `analysis/re-portable-turret.md`
//! §6.1; the officer rule is a PROVISIONAL port policy
//! (`analysis/re-officer-abilities.md`, Bot policy).

use super::arsenal;
use crate::sim::{buttons, Sim};
use crate::soldier::GroundRules;
use crate::unit::{Controller, UnitHandle};
use crate::weapon::FireState;

const SUPPORT_THREAT_RANGE: f32 = 30.0;

pub(crate) fn input(sim: &Sim, owner: UnitHandle) -> Option<u32> {
    let u = sim.unit(owner)?;
    if u.controller != Controller::Ai || u.mount.is_some() {
        return None;
    }
    if u.ai.target.is_none() {
        let current = &sim.weapon_classes[u.weapon_class];
        if let Some(c) = &current.cloak {
            if u.cloak.active || u.cloak.warmup.is_some() {
                return Some(0);
            }
            if c.affordable(&u.soldier) {
                return Some(if u.cloak.trigger { 0 } else { buttons::FIRE });
            }
        } else if u.held.iter().any(|h| {
            h.channel == 0
                && sim.weapon_classes[h.class]
                    .cloak
                    .as_ref()
                    .is_some_and(|c| c.affordable(&u.soldier))
        }) {
            return Some(if u.switch_held & 1 == 0 {
                buttons::SWITCH_WEAPON
            } else {
                0
            });
        }
    }
    if let Some(k) = u.cur1 {
        let w = &u.held[usize::from(k)];
        if w.state.state == FireState::Firing {
            let wc = &sim.weapon_classes[w.class];
            return (wc.ability.is_some() || wc.remote.is_some() || wc.turret.is_some())
                .then_some(0);
        }
    }
    let selected = u.held.iter().enumerate().find_map(|(k, w)| {
        let wc = &sim.weapon_classes[w.class];
        let ability = wc.ability.as_ref()?;
        if w.channel != 1 || w.state.reloading() || w.state.clip < wc.ammo_per_shot {
            return None;
        }
        // Who would gain from it first (cheap), then the rays: the caster's
        // sight of that soldier, and an enemy in sight of a soldier who would
        // gain damage or defense.
        let caster = owner.index as usize;
        let useful = sim
            .ability_candidates(caster, ability)
            .filter(|&target| {
                let other = &sim.units[target];
                (ability.poison.0 > 0.0 && other.buffs.poison.0 <= 0.0)
                    || (ability.regeneration.0 > 0.0
                        && other.health.cur < other.health.max
                        && other.buffs.regeneration.0 <= 0.0)
                    || ((ability.offense.0 > 0.0 && other.buffs.offense.0 <= 0.0
                        || ability.defense.0 > 0.0 && other.buffs.defense.0 <= 0.0)
                        && sim.alive_units().any(|(_, enemy)| {
                            sim.hostile(u.team, enemy.team)
                                && enemy.soldier.pos.distance_squared(other.soldier.pos)
                                    <= SUPPORT_THREAT_RANGE * SUPPORT_THREAT_RANGE
                                && crate::perception::los(sim, other.muzzle(), enemy.muzzle())
                        }))
            })
            .any(|target| sim.ability_sees(caster, target));
        useful.then_some(k)
    });
    let selected = match selected {
        Some(k) => k,
        None => placed_slot(sim, owner)?,
    };
    Some(if u.cur1 == Some(selected as u8) {
        if u.held[selected].state.fire_cmd == 0 {
            buttons::SECONDARY_FIRE
        } else {
            0
        }
    } else if u.switch_held & 2 == 0 {
        buttons::SWITCH_SECONDARY
    } else {
        0
    })
}

/// A placed item's slot: the remote droid ([`remote_slot`]), else the
/// auto-turret when the target stands in a group ([`turret_slot`]).
fn placed_slot(sim: &Sim, owner: UnitHandle) -> Option<usize> {
    remote_slot(sim, owner).or_else(|| turret_slot(sim, owner))
}

/// Card ATUR: the auto-turret goes down when the bot's target is on foot
/// in a group (BF2's grouping rule).
fn turret_slot(sim: &Sim, owner: UnitHandle) -> Option<usize> {
    let u = sim.unit(owner)?;
    let target = u.ai.target?.live_unit(sim)?;
    let cluster = || {
        let (own, foes) = arsenal::group_counts(sim, target);
        target.mount.is_none() && arsenal::is_group(GroundRules::Bf2, own, foes)
    };
    u.held.iter().enumerate().find_map(|(k, w)| {
        let wc = &sim.weapon_classes[w.class];
        (wc.turret.is_some()
            && w.channel == 1
            && !w.state.reloading()
            && w.state.clip >= wc.ammo_per_shot
            && cluster())
        .then_some(k)
    })
}

/// Card DROIDAI [ours]: the remote droid's launch. Not whenever an enemy is
/// near: a bot sends its droid into a fight it is not in, from where no enemy
/// sees it, and only some of the time ([`launch_ok`]).
fn remote_slot(sim: &Sim, owner: UnitHandle) -> Option<usize> {
    let u = sim.unit(owner)?;
    let k = u.held.iter().enumerate().find_map(|(k, w)| {
        let wc = &sim.weapon_classes[w.class];
        (wc.remote.is_some()
            && w.channel == 1
            && !w.state.reloading()
            && w.state.clip >= wc.ammo_per_shot
            && sim.droid_of(owner, w.class).is_none())
        .then_some(k)
    })?;
    launch_ok(sim, owner).then_some(k)
}

/// Whether bot `owner` may launch its droid this tick. Cheap tests first:
///
/// 1. its rest after the last flight is over (`BOT_REST`, `AiState::droid_rest`);
/// 2. this `BOT_ROLL` window's draw passes (`BOT_CHANCE`);
/// 3. it was not hit in the last `BOT_HIT_QUIET`;
/// 4. a fight is near: an enemy it or its team knows of (its contacts, told
///    ones included) is within `BOT_FIGHT`;
/// 5. no enemy is within `BOT_NEAR` (then it fights with its gun);
/// 6. no enemy within `BOT_FIGHT` has a line of sight to its body.
pub(crate) fn launch_ok(sim: &Sim, owner: UnitHandle) -> bool {
    use crate::remote::{BOT_CHANCE, BOT_FIGHT, BOT_HIT_QUIET, BOT_NEAR, BOT_ROLL};
    let Some(u) = sim.unit(owner) else {
        return false;
    };
    if sim.tick < u.ai.droid_rest {
        return false;
    }
    let window = sim.tick / (BOT_ROLL * crate::TICK_HZ as f32) as u64;
    let draw = super::draw::unit_unit(owner.index, window, owner.gen, super::draw::SALT_DROID);
    if draw >= BOT_CHANCE {
        return false;
    }
    let quiet = (BOT_HIT_QUIET * crate::TICK_HZ as f32) as u64;
    if u.last_hit.is_some_and(|(_, t)| sim.tick < t + quiet) {
        return false;
    }
    let pos = u.soldier.pos;
    let fight = u.ai.contacts.iter().flatten().any(|c| {
        c.handle.live_unit(sim).is_some_and(|e| e.alive)
            && c.last_pos.distance_squared(pos) <= BOT_FIGHT * BOT_FIGHT
    });
    if !fight {
        return false;
    }
    let body = u.capsule().center();
    !sim.alive_units().any(|(_, e)| {
        sim.hostile(u.team, e.team) && {
            let d2 = e.soldier.pos.distance_squared(pos);
            d2 <= BOT_NEAR * BOT_NEAR
                || (d2 <= BOT_FIGHT * BOT_FIGHT && crate::perception::los(sim, e.muzzle(), body))
        }
    })
}
