//! Local health/ammo drops (#165). The original supply helpers suppress aid
//! already on the ground (`analysis/re-powerups.md` §13). Self and AI allies,
//! a short stationary drop instead of a 75 m service walk, and quiet-combat
//! scheduling are deliberate port rules. Packs still use their ordinary
//! secondary channel, finite ammunition, release delay and pickup contact.

use super::fix::ServePlan;
use super::{AiState, StepKind};
use crate::fmath;
use crate::sim::{buttons, ItemClass, PadState, Sim};
use crate::soldier::Stance;
use crate::unit::{Controller, Unit, UnitHandle};
use crate::weapon::FireState;
use glam::Vec3;

/// Original service scan interval (BF2 FUN_002d6828, B2:261420-261422).
const PERIOD: u64 = 5 * crate::TICK_HZ as u64;
/// Original nearby-item suppression radius (both games, re-powerups §6).
const ITEM_RADIUS: f32 = 15.0;
/// OURS, PROVISIONAL: only local drops, no support travel. measure: original
/// supply release distances and live ally collection with a 5 m search.
const REACH: f32 = 5.0;
/// OURS, PROVISIONAL: two seconds to aim, then two to release/collect.
/// measure: support animation
/// and aim time in matched original-game scenes.
const AIM_TICKS: u64 = 2 * crate::TICK_HZ as u64;
/// OURS, PROVISIONAL: no new drop just after damage. measure: original supply
/// interruption and combat re-entry after incoming fire.
const HIT_QUIET: u64 = 3 * crate::TICK_HZ as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SupplyState {
    pub next_scan: u64,
    pub drop: Option<DropPlan>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropPlan {
    pub target: UnitHandle,
    pub slot: u8,
    pub until: u64,
    pub pressed: bool,
}

impl SupplyState {
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        if *self == Self::default() {
            return;
        }
        eat(b"SUPPLY");
        eat(&self.next_scan.to_le_bytes());
        eat(&[u8::from(self.drop.is_some())]);
        if let Some(d) = self.drop {
            eat(&d.target.index.to_le_bytes());
            eat(&d.target.gen.to_le_bytes());
            eat(&[d.slot, u8::from(d.pressed)]);
            eat(&d.until.to_le_bytes());
        }
    }
}

// Like the original recipient reservation, with stable priority for plans
// created from the same tick's snapshot. A released throw keeps its claim.
fn claimed(sim: &Sim, me: UnitHandle, target: UnitHandle) -> bool {
    sim.alive_units().any(|(h, u)| {
        h != me
            && u.ai.supply.drop.is_some_and(|d| {
                d.target == target && sim.tick < d.until && (d.pressed || h.index < me.index)
            })
    })
}

fn useful(sim: &Sim, target: &Unit, item: &ItemClass) -> bool {
    let (mut health, mut ammo) = sim.item_needs(target, item);
    if !health && !ammo {
        return false;
    }
    for p in &sim.pickups {
        if p.life <= crate::sim::LIFE_END
            || p.pos.distance_squared(target.soldier.pos) > ITEM_RADIUS * ITEM_RADIUS
        {
            continue;
        }
        let Some(c) = sim.item_classes.get(usize::from(p.class)) else {
            continue;
        };
        // OURS: a pack through a wall or on another floor cannot serve this
        // need. measure: original supply suppression across walls/storeys.
        if (health && c.health > 0.0 || ammo && c.ammo > 0)
            && sim.floor_line(target.soldier.pos, p.pos)
            && crate::perception::los(sim, target.muzzle(), p.pos + Vec3::Y * 0.3)
        {
            health &= c.health <= 0.0;
            ammo &= c.ammo == 0;
        }
        if !health && !ammo {
            return false;
        }
    }
    health || ammo
}

/// A stopped throw's low arc to the recipient's feet, with no random aim
/// error. Self aid drops down, then approaches the released pickup.
fn aim(sim: &Sim, u: &Unit, target: &Unit, class: usize, own: bool) -> Option<(f32, f32)> {
    let wc = &sim.weapon_classes[class];
    let d = wc.dispenser.as_ref()?;
    let yaw = if own {
        u.soldier.yaw
    } else {
        super::yaw_toward(target.soldier.pos - u.soldier.pos)
    };
    // Compute the hand at the desired yaw, so rotating toward an ally does
    // not move the solved launch point out from under its own solution.
    let (sy, cy) = fmath::sin_cos(yaw);
    let origin = match wc.thrown.and_then(|t| t.hand) {
        Some((up, ahead)) => u.soldier.pos + Vec3::Y * up + Vec3::new(-sy, 0.0, -cy) * ahead,
        None => u.muzzle(),
    };
    let pitch = if own {
        u.consts.pitch_limits.0
    } else {
        let to = target.soldier.pos - origin;
        let (raise, _, reaches) = super::arsenal::lob(d.speed(1.0), crate::sim::ITEM_GRAVITY, to);
        if !reaches {
            return None;
        }
        let dir = (to + Vec3::Y * raise).normalize_or_zero();
        fmath::asin(dir.y.clamp(-1.0, 1.0))
    };
    (pitch >= u.consts.pitch_limits.0 && pitch <= u.consts.pitch_limits.1).then_some((yaw, pitch))
}

fn level_gun(u: &Unit) -> ServePlan {
    ServePlan {
        rx: 0,
        ry: PadState::quantize(-u.soldier.pitch / (u.consts.max_pitch_speed * crate::TICK_DT)),
        button: 0,
    }
}

pub(super) struct SupplyPlan {
    pub gun: ServePlan,
    pub walk: Option<(i16, i16)>,
}

/// Optional final gunner/mover request, after ordinary combat and emergency
/// movement have been decided. Only a scheduled think starts a search.
pub(super) fn plan(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    thinking: bool,
    pad: &PadState,
) -> Option<SupplyPlan> {
    if st.supply.drop.is_none() && (!thinking || sim.tick < st.supply.next_scan) {
        return None;
    }
    let u = sim.unit(me)?;
    let capable = u
        .held
        .iter()
        .any(|w| w.channel == 1 && sim.weapon_classes[w.class].dispenser.is_some());
    if !capable {
        return None;
    }
    if u.controller != Controller::Ai
        || u.proxy_human.is_some()
        || sim.space_round()
        || sim.outcome.is_some()
        || u.mount.is_some()
        || u.droideka.is_some()
        || !u.soldier.grounded
        || u.soldier.stance != Stance::Stand
        || u.soldier.no_fire()
        || u.choked.is_some()
        || st.ride.is_some()
        || st.lost.is_some()
        || st.fix.cur.is_some()
        || matches!(st.step, StepKind::Flee | StepKind::Return | StepKind::Board)
        || pad.buttons & (buttons::JUMP | buttons::ROLL_STANCE | buttons::ACTION) != 0
    {
        st.supply.drop = None;
        return None;
    }
    // Finish a pending salvo before changing aim. Firing can outlast the
    // actual release (the stock pack has a one-second refire period), so
    // collection waits for the salvo, not the entire weapon cycle.
    let committed = st.supply.drop.is_some_and(|d| {
        d.pressed
            && u.held.iter().any(|w| {
                w.slot == d.slot && w.state.state == FireState::Firing && w.state.salvo_counter > 0
            })
    });
    if !committed
        && (st.target.is_some()
            || st.contacts.iter().any(Option::is_some)
            || u.last_hit.is_some_and(|(_, at)| sim.tick < at + HIT_QUIET)
            || pad.buttons & (buttons::FIRE | buttons::SECONDARY_FIRE) != 0
            || u.weapon.state == FireState::Firing
            || u.held.iter().any(|w| {
                w.state.state == FireState::Firing
                    && !st
                        .supply
                        .drop
                        .is_some_and(|d| d.pressed && d.slot == w.slot)
            }))
    {
        st.supply.drop = None;
        return None;
    }
    if st.supply.drop.is_none() && thinking && sim.tick >= st.supply.next_scan {
        st.supply.next_scan = sim.tick + PERIOD;
        st.supply.drop = u.held.iter().find_map(|w| {
            let wc = &sim.weapon_classes[w.class];
            let item = &wc.dispenser.as_ref()?.item;
            if w.channel != 1
                || w.state.reloading()
                || w.state.clip < wc.ammo_per_shot
                || sim.weapon_locked(u, w.class)
            {
                return None;
            }
            // Self first, then the nearest friendly recipient; stable handles
            // break equal distances. Dead, mounted and enemy units never win.
            std::iter::once((me, u))
                .chain(sim.alive_units().filter(|(h, _)| *h != me))
                .filter(|(_, o)| {
                    o.team == u.team
                        && o.mount.is_none()
                        && o.soldier.pos.distance_squared(u.soldier.pos) <= REACH * REACH
                        && useful(sim, o, item)
                        && sim.floor_line(u.soldier.pos, o.soldier.pos)
                        && crate::perception::los(sim, u.muzzle(), o.muzzle())
                })
                .filter(|(h, o)| aim(sim, u, o, w.class, *h == me).is_some())
                .min_by_key(|(h, o)| {
                    (
                        u8::from(*h != me),
                        o.soldier.pos.distance_squared(u.soldier.pos).to_bits(),
                        h.index,
                    )
                })
                .map(|(target, _)| DropPlan {
                    target,
                    slot: w.slot,
                    until: sim.tick + AIM_TICKS,
                    pressed: false,
                })
        });
    }
    let drop = st.supply.drop?;
    let w = u.held.iter().find(|w| w.slot == drop.slot)?;
    if drop.pressed && w.state.salvo_counter == 0 && drop.target == me && sim.tick < drop.until {
        let pickup = sim.pickups.iter().rev().find(|p| {
            p.source == Some((me, w.class))
                && p.pos.distance_squared(u.soldier.pos) <= REACH * REACH
                && sim.floor_line(u.soldier.pos, p.pos)
                && crate::perception::los(sim, u.muzzle(), p.pos + Vec3::Y * 0.3)
                && sim
                    .item_classes
                    .get(usize::from(p.class))
                    .is_some_and(|c| sim.item_needed(u, c))
        });
        if let Some(p) = pickup {
            // Self drops land beyond the hand. Walk the short clear segment
            // to take the ordinary pickup; never grant health/ammo directly.
            return Some(SupplyPlan {
                gun: level_gun(u),
                walk: Some(PadState::walk_axes(u.soldier.yaw, p.pos - u.soldier.pos)),
            });
        }
        // Restore the travelling pitch even when the pickup was taken in
        // flight or a full pool refused the spawn. Combat can interrupt it.
        if u.soldier.pitch.abs() > super::AIM_TOLERANCE {
            return Some(SupplyPlan {
                gun: level_gun(u),
                walk: None,
            });
        }
    }
    if sim.tick >= drop.until || (drop.pressed && w.state.salvo_counter == 0) {
        st.supply.drop = None;
        return None;
    }
    let Some(target) = sim
        .unit(drop.target)
        .filter(|o| o.alive && o.team == u.team && o.mount.is_none())
    else {
        st.supply.drop = None;
        return None;
    };
    if !drop.pressed
        && (claimed(sim, me, drop.target)
            || !useful(
                sim,
                target,
                &sim.weapon_classes[w.class].dispenser.as_ref()?.item,
            )
            || target.soldier.pos.distance_squared(u.soldier.pos) > REACH * REACH
            || !sim.floor_line(u.soldier.pos, target.soldier.pos)
            || !crate::perception::los(sim, u.muzzle(), target.muzzle()))
    {
        st.supply.drop = None;
        return None;
    }
    let Some((yaw, pitch)) = aim(sim, u, target, w.class, drop.target == me) else {
        st.supply.drop = None;
        return None;
    };
    let e_yaw = (yaw - u.soldier.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    let e_pitch = pitch - u.soldier.pitch;
    let selected = u
        .cur1
        .is_some_and(|k| u.held[usize::from(k)].slot == drop.slot);
    let button = if drop.pressed {
        0
    } else if !selected {
        if u.switch_held & 2 == 0 {
            buttons::SWITCH_SECONDARY
        } else {
            0
        }
    } else if e_yaw.abs() < super::AIM_TOLERANCE
        && e_pitch.abs() < super::AIM_TOLERANCE
        && w.state.fire_cmd == 0
    {
        let drop = st.supply.drop.as_mut().unwrap();
        drop.pressed = true;
        drop.until = sim.tick + AIM_TICKS;
        st.supply.next_scan = sim.tick + PERIOD;
        buttons::SECONDARY_FIRE
    } else {
        0
    };
    Some(SupplyPlan {
        gun: ServePlan {
            rx: PadState::quantize(-e_yaw / (u.consts.max_turn_speed * crate::TICK_DT)),
            ry: PadState::quantize(e_pitch / (u.consts.max_pitch_speed * crate::TICK_DT)),
            button,
        },
        walk: None,
    })
}

#[cfg(test)]
mod tests;
