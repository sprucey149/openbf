//! A bot in a GROUND vehicle's seat fights enemy vehicles (playtest note
//! "vehicles should attack enemy vehicles too"; the batch's product rule):
//! a tank, a walker, a vehicle's gunner seat or a building turret prefers an
//! enemy vehicle its seat guns can hurt over infantry, unless enemy infantry
//! is within [`INFANTRY_NEAR`]; it fires the seat weapon that is for its
//! target (a fighter tank's missile at a tank, its cannon at a soldier) and
//! holds its fire at a vehicle none of them can hurt; a hover's driver turns
//! its hull to the vehicle it fights while it drives on ([`hover_aim`]), and
//! after [`DUEL_STALL`] of a duel with no hull damage done it goes back to its
//! route for [`DUEL_REST`]. Empty vehicles are never targets: the scan sees a
//! vehicle only by its riders (`perception::scan`).
//!
//! The PS2's side, as far as it is read: a seat opens a fight only when one of
//! its weapons may be used on the target and reaches it
//! (`analysis/re-turret-ai.md` C9), by the same suitability test a soldier's
//! weapon choice asks of a vehicle - the AI target mask and "too weak"
//! (`analysis/re-soldier-ai.md` 22, `arsenal::against_vehicle`); and a hover
//! with an aim order turns its hull toward the aim point through its steering
//! tail (`analysis/re-hover-ai.md` §3, the mover's bit 5).
//!
//! OURS, PROVISIONAL, named deviations:
//! - the seat's scan reach ([`seat_scan_radius`]): the longest `MaxRange` of
//!   the seat's first two weapons plus 5 x the vehicle's ForwardSpeed, or
//!   the soldier's own kit-and-run-speed reach when that is longer
//!   (`perception::seat_reach`: many seat guns have no `MaxRange` and take
//!   the class default of 60 m, and a seat must not see less than its rider
//!   did on foot). The PS2's scan radius
//!   (B2 `FUN_002a9d90`) is read for a soldier; what it is asked of a vehicle
//!   controller is not traced. A rider is still seen no farther than his
//!   vehicle's body allows, `min((20 r + 30) x view_mult, reach)`.
//!   trace: FUN_002a9d90 on a seated AI's controller.
//! - the target preference ([`bonus`]): `arsenal::VEHICLE_BONUS`, the
//!   rocket trooper's, for a vehicle a seat weapon can hurt within its
//!   `MaxRange`, nothing while enemy infantry is within [`INFANTRY_NEAR`] of
//!   the vehicle, and minus that bonus for a vehicle no seat weapon can hurt.
//!   measure: VEHICLE_CENSUS eng/opp and seat_vhit per round.
//! - the seat weapon ([`pick`]): the best-scoring usable weapon of the first
//!   two (FIRE and SECONDARY_FIRE; a ground seat fires no third), in range
//!   first, then ready (not reloading or overheated), then by score, ties to
//!   the lower slot. A soldier's weapon must pass the AI target mask; with
//!   none that does, the first weapon fires as before.
//! - the hover's duel ([`hover_aim`]): the driver sets the PS2's aim order
//!   AND its bit 5 ("stand"), whose PS2 senders are behaviour states that
//!   were not decoded (`re-hover-ai.md` §1/§8), so a strafing hover slides
//!   along its route facing the enemy; a hover without a StrafeSpeed keeps
//!   its route and turns to the target only when the tail already turns to
//!   an aim point (stopped). [`DUEL_STALL`] / [`DUEL_REST`] are ours.
//!   measure: VEHICLE_CENSUS cap / round ticks / stuckmark against base.
//!
//! None of this runs in a space round or in a flyer's seat: those keep their
//! own rules (`flyer_weapons`), and their bots' AI state is unchanged.

use super::arsenal::{against_vehicle, score, shot, Cand, VehAsk, VEHICLE_BONUS};
use super::state::{AiState, SeatFight};
use crate::fmath::FloatExt;
use crate::sim::Sim;
use crate::soldier::GroundRules;
use crate::unit::{Unit, UnitHandle};
use crate::vehicle::gun::{GunSetup, GunState};
use crate::vehicle::{Mount, Vehicle, VehicleKind};
use crate::TICK_HZ;
use glam::Vec3;

/// OURS (product rule): enemy infantry this near the vehicle, m, takes the
/// seat's fire before an enemy vehicle does.
pub const INFANTRY_NEAR: f32 = 15.0;
/// OURS (product rule): a duel with no hull damage done to the target for
/// this long ends, ticks (6 s) ...
pub const DUEL_STALL: u64 = 6 * TICK_HZ as u64;
/// ... and the hover drives its route for this long before it turns to a
/// vehicle again, ticks (4 s). measure: VEHICLE_CENSUS against base.
pub const DUEL_REST: u64 = 4 * TICK_HZ as u64;
/// A ground seat fires its first weapon on FIRE and its second on
/// SECONDARY_FIRE (`sim::guns`); no button fires a third.
pub const GROUND_SEAT_WEAPONS: usize = 2;

/// The ground seat `u` fights from - a hover's, a walker's or a building
/// turret's, with guns, in a round that is not a space round - with its
/// vehicle, gun setup and gun state.
pub(crate) fn ground_seat<'a>(
    sim: &'a Sim,
    u: &Unit,
) -> Option<(Mount, &'a Vehicle, &'a GunSetup, Option<&'a GunState>)> {
    if sim.space_round() {
        return None;
    }
    let m = u.mount?;
    let v = sim.vehicle(m.vehicle)?;
    if !matches!(
        v.kind,
        VehicleKind::Hover | VehicleKind::Walker | VehicleKind::Turret
    ) {
        return None;
    }
    let g = sim
        .seat_setup
        .get(v.class as usize)?
        .as_ref()?
        .guns
        .get(usize::from(m.seat))?
        .as_ref()?;
    if g.weapons.is_empty() {
        return None;
    }
    let gs = v.guns.get(usize::from(m.seat)).and_then(Option::as_ref);
    Some((m, v, g, gs))
}

/// The seat's first two weapons as the choice sees them. A seat weapon is
/// never HEAVY: its AI target mask is always asked.
fn cands<'a>(
    sim: &'a Sim,
    g: &GunSetup,
    gs: Option<&GunState>,
) -> [Option<Cand<'a>>; GROUND_SEAT_WEAPONS] {
    let mut out = [None; GROUND_SEAT_WEAPONS];
    for (k, w) in g.weapons.iter().take(GROUND_SEAT_WEAPONS).enumerate() {
        let Some(class) = sim.weapon_classes.get(w.class) else {
            continue;
        };
        let ws = gs.and_then(|s| s.weapons.get(k));
        out[k] = Some(Cand {
            slot: k as u8,
            class,
            rounds: if w.infinite {
                None
            } else {
                ws.and_then(|s| s.issued(class))
            },
            empty: !w.infinite && ws.is_some_and(|s| s.out_of_ammo(class)),
            busy: ws.is_some_and(|s| s.reloading() || s.overheated),
            heavy: false,
        });
    }
    out
}

/// The seat's sight reach (module header: PROVISIONAL), or `None` for a
/// unit not in a ground seat (`perception::scan_radius` then holds).
pub(crate) fn seat_scan_radius(sim: &Sim, u: &Unit) -> Option<f32> {
    let (_, v, g, _) = ground_seat(sim, u)?;
    let range = g
        .weapons
        .iter()
        .take(GROUND_SEAT_WEAPONS)
        .filter_map(|w| sim.weapon_classes.get(w.class))
        .map(|w| w.ai_range[2])
        .fold(0.0, crate::fmath::max);
    let speed = match v.kind {
        VehicleKind::Hover => sim
            .hover_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |p| p.forward_speed),
        VehicleKind::Walker => sim
            .walker_params
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .map_or(0.0, |p| p.max_speed),
        _ => 0.0,
    };
    Some(range + 5.0 * speed)
}

/// Where the seat's shots leave from: its fire point on the vehicle.
fn muzzle(v: &Vehicle, g: &GunSetup) -> Vec3 {
    v.pos + v.rot * g.fire_point
}

/// The hostile vehicle `o` rides, as the choice asks it.
fn ridden<'a>(sim: &'a Sim, o: &Unit) -> Option<(&'a Vehicle, VehAsk, u32)> {
    let v = o.mount.and_then(|m| sim.vehicle(m.vehicle))?;
    let ask = VehAsk {
        max_health: v.health.max,
        speed: v.vel.length(),
        kind: v.kind,
    };
    Some((v, ask, v.health.health_type as u32))
}

/// Is a hostile soldier on foot within [`INFANTRY_NEAR`] of `v`?
fn infantry_near(sim: &Sim, team: u8, v: &Vehicle) -> bool {
    sim.alive_units().any(|(_, e)| {
        e.mount.is_none()
            && sim.hostile(team, e.team)
            && e.soldier.pos.distance_squared(v.pos) < INFANTRY_NEAR * INFANTRY_NEAR
    })
}

/// [`super::vehicle_bonus`] for a bot in a ground seat (module header):
/// `None` when `u` is not in one.
pub(crate) fn bonus(sim: &Sim, u: &Unit, o: &Unit) -> Option<f32> {
    let (_, v, g, gs) = ground_seat(sim, u)?;
    let Some((ov, ask, kind)) = ridden(sim, o) else {
        return Some(0.0);
    };
    if ov.kind == VehicleKind::Flyer {
        // A flyer is the flyer rules' business (`against_vehicle` keeps a
        // fast one out of reach); no preference either way.
        return Some(0.0);
    }
    if sim.tick < u.ai.seat.rest_until {
        return Some(0.0);
    }
    let bf2 = sim.ground_rules == GroundRules::Bf2;
    let d2 = (o.capsule().center() - muzzle(v, g)).length_squared();
    let (mut any, mut reaches) = (false, false);
    for c in cands(sim, g, gs).iter().flatten().filter(|c| !c.empty) {
        if against_vehicle(c, &ask, kind, bf2).is_some() {
            any = true;
            reaches |= d2 < c.class.ai_range[2] * c.class.ai_range[2];
        }
    }
    Some(if !any {
        -VEHICLE_BONUS
    } else if reaches && !infantry_near(sim, u.team, v) {
        VEHICLE_BONUS
    } else {
        0.0
    })
}

/// The seat weapon for target `e` (module header) and whether the seat holds
/// its fire (a rider no seat weapon may be used on).
pub(crate) fn pick(sim: &Sim, u: &Unit, e: Option<&Unit>) -> (u8, bool) {
    let Some((_, v, g, gs)) = ground_seat(sim, u) else {
        return (0, false);
    };
    let Some(e) = e else {
        return (0, false);
    };
    let bf2 = sim.ground_rules == GroundRules::Bf2;
    let d2 = (e.capsule().center() - muzzle(v, g)).length_squared();
    let rider = ridden(sim, e);
    let mut best: Option<((bool, bool, f32), u8)> = None;
    for c in cands(sim, g, gs).iter().flatten().filter(|c| !c.empty) {
        let s = match rider {
            Some((_, ask, kind)) => against_vehicle(c, &ask, kind, bf2),
            None => {
                let kind = e.kind();
                (c.class.ai_target_kind >> kind.min(31) & 1 != 0).then(|| {
                    if bf2 {
                        score(c.class, kind)
                    } else {
                        shot(c.class, kind)
                    }
                })
            }
        };
        let Some(s) = s else {
            continue;
        };
        let [_, _, max] = c.class.ai_range;
        let key = (d2 < max * max, !c.busy, s);
        // Lexicographic: in range, then ready, then score; the lower slot
        // keeps a tie.
        let better = best.is_none_or(|(b, _)| {
            (key.0, key.1) > (b.0, b.1) || ((key.0, key.1) == (b.0, b.1) && key.2 > b.2)
        });
        if better {
            best = Some((key, c.slot));
        }
    }
    match (best, rider) {
        (Some((_, slot)), _) => (slot, false),
        // C9: no seat weapon may be used on the vehicle - hold the fire.
        (None, Some(_)) => (0, true),
        (None, None) => (0, false),
    }
}

/// Once a think, for a bot in a ground seat: its seat weapon and the duel's
/// clock (module header). A bot in any other seat, or on foot, keeps
/// [`SeatFight::default`], which adds nothing to the hash.
pub(crate) fn think(sim: &Sim, me: UnitHandle, st: &mut AiState) {
    let Some(u) = sim.unit(me) else {
        return;
    };
    let Some((m, _, _, _)) = ground_seat(sim, u) else {
        st.seat = SeatFight::default();
        return;
    };
    if st.seat.at != Some(m) {
        st.seat = SeatFight {
            at: Some(m),
            ..SeatFight::default()
        };
    }
    let target = st
        .target
        .and_then(|h| crate::perception::contact_of(st, h))
        .and_then(|c| c.handle.live_unit(sim));
    let (weapon, hold) = pick(sim, u, target);
    if weapon != st.seat.weapon {
        // A charge under way is the other weapon's.
        st.charge_until = 0;
    }
    st.seat.weapon = weapon;
    st.seat.hold = hold;
    // The duel: the hostile ground vehicle the target rides, and when its
    // hull last lost health.
    let tick = sim.tick;
    let duel = target
        .and_then(|e| e.mount)
        .and_then(|em| sim.vehicle(em.vehicle).map(|v| (em.vehicle, v)))
        .filter(|(_, v)| v.kind != VehicleKind::Flyer && !hold);
    match duel {
        Some((h, v)) if tick >= st.seat.rest_until => {
            let hp = v.health.cur;
            if st.seat.duel != Some(h) {
                st.seat.duel = Some(h);
                st.seat.duel_hp = hp;
                st.seat.duel_tick = tick;
            } else if hp < st.seat.duel_hp {
                st.seat.duel_hp = hp;
                st.seat.duel_tick = tick;
            } else if tick.saturating_sub(st.seat.duel_tick) > DUEL_STALL {
                st.seat.rest_until = tick + DUEL_REST;
                st.seat.duel = None;
                st.seat.duel_hp = 0.0;
                st.seat.duel_tick = 0;
            }
        }
        Some(_) => {}
        None => {
            st.seat.duel = None;
            st.seat.duel_hp = 0.0;
            st.seat.duel_tick = 0;
        }
    }
}

/// The seat weapon index `gunner::aim` fires for `u` (`None`: not a ground
/// seat's think, the gunner's old rule) and whether it holds its fire.
pub(crate) fn seat_weapon(sim: &Sim, u: &Unit, st: &AiState) -> Option<(usize, bool)> {
    let (m, _, _, _) = ground_seat(sim, u)?;
    (st.seat.at == Some(m)).then_some((usize::from(st.seat.weapon), st.seat.hold))
}

/// The point a hover's driver turns its hull to (`hover_drive::Order::aim`):
/// the lead point of the seen enemy vehicle it fights, when its seat weapon
/// may be used on it and reaches, and it is not resting from a duel.
pub(crate) fn hover_aim(sim: &Sim, me: UnitHandle, st: &AiState) -> Option<Vec3> {
    let u = sim.unit(me)?;
    let (m, v, g, _) = ground_seat(sim, u)?;
    if v.kind != VehicleKind::Hover
        || !g.pilot
        || st.seat.at != Some(m)
        || st.seat.hold
        || sim.tick < st.seat.rest_until
        || st.seat.duel.is_none()
    {
        return None;
    }
    let c = st
        .target
        .and_then(|h| crate::perception::contact_of(st, h))
        .filter(|c| c.seen_now)?;
    let e = c.handle.live_unit(sim)?;
    let ev = e.mount.and_then(|em| sim.vehicle(em.vehicle))?;
    if ev.kind == VehicleKind::Flyer {
        return None;
    }
    let w = g.weapons.get(usize::from(st.seat.weapon))?;
    let wc = sim.weapon_classes.get(w.class)?;
    let s = sim.seat_look(me)?;
    if (e.capsule().center() - s.muzzle).length() > wc.engage_range().fmax(wc.ai_range[2]) {
        return None;
    }
    let speed = wc.shot_ordnance().velocity;
    Some(super::gunner_foot::seen_aim(
        sim, me, st, s.eye, e, speed, wc, false,
    ))
}

#[cfg(test)]
mod tests;
