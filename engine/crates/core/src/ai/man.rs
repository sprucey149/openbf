//! Turret offers discard an earlier ground-claim heartbeat (CWAIT).
//! Card TURR (`analysis/bot-design.md` 6.1, deviation D11; the maintainer's annoyance
//! 1, 2026-10-02: "they'll get in the turret and rarely leave unless they're
//! being shot at or die"): a bot mans a building turret only with a PURPOSE,
//! and gets out by five rules.
//!
//! The PS2's rule, both games (`analysis/re-soldier-ai.md` 7.1-7.3, 7.5, 10a
//! [V]): a bot within 50 m is offered every free seat and takes a turret
//! while a live threat's last point lies in a turret weapon's arc (no range
//! test); it gets out after 10 s of thinks with no live threat - any
//! remembered enemy at any range, an enemy turret 100-180 m off, resets that
//! clock - or when hit from outside every arc. On Kamino 15 of 32 seated
//! spells ran over a minute, one gunner sat 91 s with no enemy on foot within
//! 100 m. The port's bots never boarded one (card TURR, main 53a9d7e: BF2
//! KAM1's 21 turrets, `ai_boards 0`).
//!
//! OURS, PROVISIONAL (D11; the constants are `crate::ai`'s `MAN_*`):
//! - the OFFER ([`offer_pass`], step 1, once in [`MAN_OFFER_PERIOD`]): per
//!   team, each free turret whose purpose test passes goes to the nearest
//!   free bot on foot within [`MAN_SEEK`] (the PS2's 50 m) - one bot a
//!   turret, [`MAN_POST_CAP`] turrets a post, none in the [`endgame`]. The
//!   claim is walked as a vehicle's is (`ride::board_goal`);
//! - the PURPOSE ([`purpose`]): (attack) an enemy the team knows of - a
//!   contact in a living team-mate's memory, at its last known point
//!   ([`known`]) - inside a weapon arc, inside the weapon's range and the
//!   distance a gunner sees him at, with a clear line from the muzzle
//!   ([`hittable`]); or (defence, [`defends`]) the turret within
//!   [`MAN_POST_NEAR`] of a post the team owns that has an enemy or neutral
//!   post as a neighbour on the soldiers' route graph ([`front_line`]), with
//!   an enemy the team knows of within [`MAN_SEEN_NEAR`] of that post. The
//!   card asked for one SEEN there in the last 30 s, and its notes for card
//!   DEFS's record (`commander::TeamPlan::threat`: an enemy within the map's
//!   threat radius, seen or not, in the last 30 s); MEASURED on BF2 KAM1, 8
//!   seeds (`lead_tmp/turr/proto`): with that record 7 of 8 rounds put over
//!   10 % of the AI's time in a seat (mean 10.95 %) and 64 % of the seated
//!   spells fired no shot - gunners walked to a quiet turret, sat its 10 s and
//!   left; with what the team knows now 8.30 % (6.16-9.94, the PS2 9.4 %) and
//!   46 %;
//! - the walk has an outcome: a turret it does not reach (its claim's clock,
//!   or the Recover ladder's give-up) is closed to that bot for
//!   [`MAN_UNREACHED_CLOSED`];
//! - the five EXITS ([`exit_rule`], asked at each think in the seat): (1)
//!   [`MAN_NO_TARGET`] with no target it can hit - only a hittable target
//!   resets the PS2's clock; (2) [`MAN_LONG`] seated with no shot in the last
//!   [`MAN_NO_SHOT`]; (3) hit from outside every arc (the PS2's); a turret
//!   low on health is never left (the PS2 has no bail-out for turrets; the
//!   port's 30 % one was dropped, the maintainer 2026-10-28), only not boarded;
//!   (4) the endgame; (5) the purpose test failed for [`MAN_PURPOSE_LOST`].
//!   Then the turret is closed to that bot for [`MAN_LEFT_CLOSED`] (the PS2's
//!   unboard cooldown). A turret fights a turret only inside its weapon's
//!   range: a target out of range is not hittable.
//! - with no target seen the gunner looks at the last known point of what it
//!   can hit (the PS2 aims a remembered target's last point; its idle look
//!   about the base direction, re-turret-ai 2.3, is not built);
//! - the gunner chooses its target among those inside its turret's arc
//!   ([`arc_of`]), and its sight passes its own building
//!   (`perception::past_buildings`); its aimer takes its aim point
//!   (`ManState::aim`, the PS2's AI seat).

use super::state::{AiState, Behaviour, ManState, NavTick, RideState};
use super::{
    HOLD_ENDGAME, MAN_ENDGAME_ALIVE, MAN_LEFT_CLOSED, MAN_LONG, MAN_LOW_HEALTH, MAN_NO_SHOT,
    MAN_NO_TARGET, MAN_OFFER_PERIOD, MAN_POST_CAP, MAN_POST_NEAR, MAN_PURPOSE_LOST, MAN_SEEK,
    MAN_SEEN_NEAR, MAN_UNREACHED_CLOSED, THINK_PERIOD,
};
use crate::collision::HullFilter;
use crate::nav;
use crate::perception;
use crate::sim::{PadState, Sim};
use crate::unit::{Controller, UnitHandle};
use crate::vehicle::gun::{self, GunSetup};
use crate::vehicle::{Mount, VehicleHandle, VehicleKind};
use glam::Vec3;
use std::f32::consts::TAU;

/// Is `vh` a building turret?
pub fn is_turret(sim: &Sim, vh: VehicleHandle) -> bool {
    sim.vehicle(vh)
        .is_some_and(|v| v.kind == VehicleKind::Turret)
}

/// Is `team` in the endgame: a team down to [`HOLD_ENDGAME`] tickets (the
/// port's endgame, card STEND), or `team` down to [`MAN_ENDGAME_ALIVE`]
/// living units? Nobody mans a turret then: out, and hunt.
pub fn endgame(sim: &Sim, team: u8) -> bool {
    sim.battle.teams.iter().any(|t| t.tickets() <= HOLD_ENDGAME)
        || sim.alive_units().filter(|(_, u)| u.team == team).count() <= MAN_ENDGAME_ALIVE
}

/// Seat `seat`'s gun of `vh`'s class, if it has one.
pub fn seat_gun(sim: &Sim, vh: VehicleHandle, seat: u8) -> Option<&GunSetup> {
    let v = sim.vehicle(vh)?;
    sim.seat_setup
        .get(v.class as usize)?
        .as_ref()?
        .guns
        .get(usize::from(seat))?
        .as_ref()
}

/// Does direction `local` (the vehicle's model frame, from the gun's pivot)
/// lie inside the gun's arc - where its barrel can point? A building
/// turret's look limits (`gun::Turn`, a wrapping axis turns all the way
/// round) plus the aimer's limits when the aimer rides the turret's frame;
/// else the aimer's limits alone.
pub fn arc_holds(g: &GunSetup, local: Vec3) -> bool {
    let (y, p) = gun::angles(local);
    let a = g.aimer;
    let within = |x: f32, lo: f32, hi: f32| hi - lo >= TAU || (x - lo).rem_euclid(TAU) <= hi - lo;
    match g.turn {
        Some(t) if g.framed => {
            (t.yaw_wrap || within(y, t.yaw[0] + a.yaw[0], t.yaw[1] + a.yaw[1]))
                && (t.pitch_wrap || within(p, t.pitch[0] + a.pitch[0], t.pitch[1] + a.pitch[1]))
        }
        _ => within(y, a.yaw[0], a.yaw[1]) && within(p, a.pitch[0], a.pitch[1]),
    }
}

/// What `team` knows of its enemies: every contact in a living team-mate's
/// memory (`perception::scan`), at its freshest last known point.
pub fn known(sim: &Sim, team: u8) -> Vec<(UnitHandle, Vec3)> {
    let mut out: Vec<(UnitHandle, Vec3, u64)> = Vec::new();
    for (_, u) in sim.alive_units() {
        if u.team != team {
            continue;
        }
        for c in u.ai.contacts.iter().flatten() {
            let Some(h) = c.handle.unit() else {
                continue;
            };
            match out.iter_mut().find(|k| k.0 == h) {
                Some(k) if c.last_seen > k.2 => (k.1, k.2) = (c.last_pos, c.last_seen),
                Some(_) => {}
                None => out.push((h, c.last_pos, c.last_seen)),
            }
        }
    }
    out.into_iter().map(|(h, p, _)| (h, p)).collect()
}

/// The purpose test's attack clause: the nearest last known point of a living
/// enemy in `known` that `vh`'s gun `g` can hit - inside its arc
/// ([`arc_holds`]), no farther from the muzzle (the barrel turned at it) than
/// the gun's first weapon reaches and a gunner sees that enemy at
/// (`perception::view_distance`), with a clear line from the muzzle past the
/// turret's own hull and the enemy's vehicle.
pub fn hittable(
    sim: &Sim,
    vh: VehicleHandle,
    g: &GunSetup,
    team: u8,
    known: &[(UnitHandle, Vec3)],
) -> Option<Vec3> {
    let v = sim.vehicle(vh)?;
    let wc = sim.weapon_classes.get(g.weapons.first()?.class)?;
    let view = sim.ai_tier(team).view_mult;
    let (pivot, inv) = (v.pos + v.rot * g.pivot, v.rot.inverse());
    // (distance, index, point, muzzle, the target's mount and its building)
    type Cand = (f32, u32, Vec3, Vec3, Option<(VehicleHandle, Option<u32>)>);
    let mut cand: Vec<Cand> = Vec::new();
    for &(h, p) in known {
        let Some(o) = sim.unit(h).filter(|o| o.team != team) else {
            continue;
        };
        let local = inv * (p - pivot);
        if !arc_holds(g, local) {
            continue;
        }
        let muzzle = v.pos + v.rot * g.muzzle(gun::angles(local));
        let d = (p - muzzle).length();
        let reach = wc
            .shot_range()
            .min(perception::view_distance(o.soldier.stance, view));
        if d <= reach {
            let theirs = o.mount.map(|m| {
                let b = sim.vehicle(m.vehicle).and_then(|t| t.instance);
                (m.vehicle, b)
            });
            cand.push((d, h.index, p, muzzle, theirs));
        }
    }
    // The ray is the cost: nearest first, the first clear one wins.
    cand.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    cand.into_iter()
        .find(|&(_, _, p, muzzle, theirs)| {
            perception::los_seat(
                sim,
                muzzle,
                p,
                HullFilter::Except([Some(vh), theirs.map(|t| t.0)]),
                [v.instance, theirs.and_then(|t| t.1)],
            )
        })
        .map(|c| c.2)
}

/// Is post `cp` on `team`'s front line: has it an enemy or neutral post as a
/// NEIGHBOUR on the soldiers' route graph - one no third post's hub lies
/// between on a shortest hop path? With no graph, any such post is one.
pub fn front_line(sim: &Sim, team: u8, cp: usize) -> bool {
    let others = || {
        sim.cps
            .iter()
            .enumerate()
            .filter(move |&(i, c)| i != cp && c.owner != Some(team))
    };
    let (Some(g), Some(from)) = (sim.nav.as_ref(), sim.cps.get(cp).and_then(|c| c.nav_hub)) else {
        return others().next().is_some();
    };
    let (mut a, mut b, mut q) = (Vec::new(), Vec::new(), Vec::new());
    g.hop_distances_into(from, nav::Mover::SOLDIER, &mut a, &mut q);
    others().any(|(j, c)| {
        let Some(to) = c.nav_hub else {
            return false;
        };
        let ab = a[usize::from(to)];
        if ab == u16::MAX {
            return false;
        }
        b.clear();
        g.hop_distances_into(to, nav::Mover::SOLDIER, &mut b, &mut q);
        !sim.cps.iter().enumerate().any(|(k, m)| {
            k != cp
                && k != j
                && m.nav_hub.is_some_and(|h| {
                    let h = usize::from(h);
                    u32::from(a[h]) + u32::from(b[h]) <= u32::from(ab)
                })
        })
    })
}

/// The purpose test's defence clause: `vh` stands within [`MAN_POST_NEAR`]
/// of a post `team` owns, on the front line ([`front_line`]), with an enemy
/// the team knows of ([`known`]) within [`MAN_SEEN_NEAR`] of the post.
pub fn defends(sim: &Sim, vh: VehicleHandle, team: u8, known: &[(UnitHandle, Vec3)]) -> bool {
    let Some(v) = sim.vehicle(vh) else {
        return false;
    };
    sim.cps.iter().enumerate().any(|(ci, cp)| {
        cp.owner == Some(team)
            && nav::xz_dist(v.pos, cp.def.pos) <= MAN_POST_NEAR
            && known.iter().any(|&(h, p)| {
                sim.unit(h).is_some_and(|o| o.team != team)
                    && nav::xz_dist(p, cp.def.pos) <= MAN_SEEN_NEAR
            })
            && front_line(sim, team, ci)
    })
}

/// The whole purpose test for `team` at `vh`'s gun `g`: the point it can hit
/// (attack, `Some`), else whether it defends a front-line post.
pub fn purpose(
    sim: &Sim,
    vh: VehicleHandle,
    g: &GunSetup,
    team: u8,
    known: &[(UnitHandle, Vec3)],
) -> (Option<Vec3>, bool) {
    match hittable(sim, vh, g, team, known) {
        Some(p) => (Some(p), true),
        None => (None, defends(sim, vh, team, known)),
    }
}

/// Has a bot claimed `vh` (on foot), or does anyone sit in it?
fn taken(sim: &Sim, vh: VehicleHandle) -> bool {
    sim.vehicle(vh)
        .is_some_and(|v| v.seats.iter().any(|s| s.occupant.is_some()))
        || sim
            .alive_units()
            .any(|(_, o)| o.mount.is_none() && o.ai.ride.is_some_and(|r| r.board == Some(vh)))
}

/// The seat of `vh` a bot of `team` would man: the first boardable seat in
/// ring order with a gun (CanEnter's order), when the turret is free, alive,
/// of no other team and over [`MAN_LOW_HEALTH`].
fn free_seat(sim: &Sim, vh: VehicleHandle, team: u8) -> Option<u8> {
    let v = sim.vehicle(vh).filter(|v| v.kind == VehicleKind::Turret)?;
    if v.team.is_some_and(|t| t != team)
        || v.health.cur < MAN_LOW_HEALTH * v.health.max
        || taken(sim, vh)
    {
        return None;
    }
    let setup = sim.seat_setup.get(v.class as usize)?.as_ref()?;
    (0..v.seats.len()).map(|k| k as u8).find(|&k| {
        setup
            .boardable
            .get(usize::from(k))
            .copied()
            .unwrap_or(false)
            && seat_gun(sim, vh, k).is_some()
    })
}

/// Has `team` [`MAN_POST_CAP`] turrets manned or claimed within
/// [`MAN_POST_NEAR`] of the post nearest `vh` (within that distance)?
fn post_full(sim: &Sim, vh: VehicleHandle, team: u8) -> bool {
    let Some(v) = sim.vehicle(vh) else {
        return true;
    };
    let near = |p: Vec3, c: &crate::cp::CommandPost| nav::xz_dist(p, c.def.pos);
    let Some(cp) = sim
        .cps
        .iter()
        .filter(|c| near(v.pos, c) <= MAN_POST_NEAR)
        .min_by(|a, b| near(v.pos, a).total_cmp(&near(v.pos, b)))
    else {
        return false;
    };
    let mine = |h: VehicleHandle| {
        sim.alive_units().any(|(_, o)| {
            o.team == team
                && (o.mount.is_some_and(|m| m.vehicle == h)
                    || (o.mount.is_none() && o.ai.ride.is_some_and(|r| r.board == Some(h))))
        })
    };
    sim.alive_vehicles()
        .filter(|(h, t)| {
            t.kind == VehicleKind::Turret
                && *h != vh
                && near(t.pos, cp) <= MAN_POST_NEAR
                && mine(*h)
        })
        .count()
        >= MAN_POST_CAP
}

/// May bot slot `i` of `team` take a turret offer for `vh`: alive, on foot,
/// in the commander's pool, on the level, with no vehicle claim and `vh` not
/// closed to it?
fn free_bot(sim: &Sim, i: usize, team: u8, vh: VehicleHandle) -> bool {
    let o = &sim.units[i];
    o.alive
        && o.team == team
        && o.controller == Controller::Ai
        && o.mount.is_none()
        && o.droideka.is_none()
        && o.command.is_none()
        && o.ai.lost.is_none()
        && o.ai.ride.is_none_or(|r| {
            r.board.is_none()
                && sim.tick >= r.own_search_until
                && !(r.closed == Some(vh) && sim.tick < r.closed_until)
        })
}

/// Step 1, after the commander, once in [`MAN_OFFER_PERIOD`]: per team (in
/// order) and turret (in index order), a free turret whose purpose test
/// passes is claimed for the nearest free bot within [`MAN_SEEK`] (ties: the
/// lower slot), who thinks at once and walks to it.
pub fn offer_pass(sim: &mut Sim) {
    if !sim.tick.is_multiple_of(MAN_OFFER_PERIOD) {
        return;
    }
    let turrets: Vec<VehicleHandle> = sim
        .alive_vehicles()
        .filter(|(_, v)| v.kind == VehicleKind::Turret)
        .map(|(h, _)| h)
        .collect();
    if turrets.is_empty() {
        return;
    }
    for team in 0..2u8 {
        if endgame(sim, team) {
            continue;
        }
        let known = known(sim, team);
        for &vh in &turrets {
            let Some(k) = free_seat(sim, vh, team) else {
                continue;
            };
            if post_full(sim, vh, team) {
                continue;
            }
            let Some(g) = seat_gun(sim, vh, k) else {
                continue;
            };
            let (hit, ok) = purpose(sim, vh, g, team, &known);
            if !ok {
                continue;
            }
            let at = sim.vehicle(vh).map_or(Vec3::ZERO, |v| v.pos);
            let mut best: Option<(f32, usize)> = None;
            for i in 0..sim.units.len() {
                if !free_bot(sim, i, team, vh) {
                    continue;
                }
                let d = sim.units[i].soldier.pos.distance(at);
                if d <= MAN_SEEK && best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, i));
                }
            }
            let Some((_, i)) = best else {
                continue;
            };
            let tick = sim.tick;
            let ai = &mut sim.units[i].ai;
            let r = ai.ride.unwrap_or_default();
            ai.ride = Some(RideState {
                board: Some(vh),
                maintenance: None,
                since: tick,
                use_req: false,
                ..r
            });
            ai.next_think = tick;
            sim.nav_stats.man_offers += 1;
            sim.nav_stats.man_defence_offers += u64::from(hit.is_none());
        }
    }
}

/// Close `vh` to this bot for `ticks` (it left it, or did not reach it).
pub(super) fn close(sim: &Sim, st: &mut AiState, vh: VehicleHandle, ticks: u64) {
    let r = st.ride.unwrap_or_default();
    st.ride = Some(RideState {
        closed: Some(vh),
        closed_until: sim.tick + ticks,
        ..r
    });
}

/// A turret claim the walk gave up on (its clock, or the Recover ladder):
/// closed to the bot for [`MAN_UNREACHED_CLOSED`].
pub(super) fn unreached(sim: &Sim, st: &mut AiState, vh: VehicleHandle, nt: &mut NavTick) {
    close(sim, st, vh, MAN_UNREACHED_CLOSED);
    nt.man = NavTick::MAN_UNREACHED;
}

/// Which of the five exits holds for a gunner of `team` with clocks `man`,
/// `last_shot` its last shot's tick (`u64::MAX`: none), in seat gun `g` of
/// `vh` - the first in rule order, or `None`.
pub fn exit_rule(
    sim: &Sim,
    vh: VehicleHandle,
    g: &GunSetup,
    team: u8,
    man: &ManState,
    last_shot: u64,
) -> Option<u8> {
    let v = sim.vehicle(vh)?;
    let tick = sim.tick;
    // 1. No target it can hit for 10 s.
    if tick.saturating_sub(man.hittable) >= MAN_NO_TARGET {
        return Some(1);
    }
    // 2. 45 s seated, no shot in the last 20 s.
    let shot = if last_shot != u64::MAX && last_shot >= man.seated {
        last_shot
    } else {
        man.seated
    };
    if tick.saturating_sub(man.seated) >= MAN_LONG && tick.saturating_sub(shot) >= MAN_NO_SHOT {
        return Some(2);
    }
    // 3. Hit since the last think by an enemy outside every arc.
    let pivot = v.pos + v.rot * g.pivot;
    let outside = v.last_hit.is_some_and(|(h, t)| {
        tick.saturating_sub(t) <= THINK_PERIOD
            && sim.unit(h).is_some_and(|o| {
                o.team != team && !arc_holds(g, v.rot.inverse() * (o.capsule().center() - pivot))
            })
    });
    if outside {
        return Some(3);
    }
    // 4. The endgame.
    if endgame(sim, team) {
        return Some(4);
    }
    // 5. No purpose for 10 s.
    if tick.saturating_sub(man.purpose) >= MAN_PURPOSE_LOST {
        return Some(5);
    }
    None
}

/// For a bot in a building turret's seat, the test of a point inside its
/// turret's arc ([`arc_holds`], from the gun's pivot): the gunner chooses
/// its target only among those (`perception::choose_target_with`). OURS
/// (card TURR; the PS2's AI pick for a turret seat is not traced): BF2
/// Kashyyyk's recoilless turret (`YawLimits -65 65`) held its look at the
/// limit on a target 120 deg round, 1,220 thinks of 1,281 with a target,
/// one spell of 29 with a shot over 8 seeds.
pub(crate) fn arc_of<'a>(
    sim: &'a Sim,
    u: &crate::unit::Unit,
) -> Option<impl Fn(Vec3) -> bool + 'a> {
    let m = u.mount?;
    if !is_turret(sim, m.vehicle) {
        return None;
    }
    let g = seat_gun(sim, m.vehicle, m.seat)?;
    let v = sim.vehicle(m.vehicle)?;
    let (pivot, inv) = (v.pos + v.rot * g.pivot, v.rot.inverse());
    Some(move |p: Vec3| arc_holds(g, inv * (p - pivot)))
}

/// The tick of a bot in a building turret's seat (`ride::drive`): at each
/// think its clocks and the exits; then the seat gun (`gunner::aim`), with
/// no target seen at the last known point of its own target, else of the
/// enemy it can hit.
pub(super) fn seated(
    sim: &Sim,
    me: UnitHandle,
    m: Mount,
    st: &mut AiState,
    pad: &mut PadState,
    nt: &mut NavTick,
    thinking: bool,
) {
    st.behaviour = Behaviour::Man;
    let Some(u) = sim.unit(me) else {
        return;
    };
    let mut r = st.ride.unwrap_or_default();
    let mut man = r.man.unwrap_or(ManState {
        seated: sim.tick,
        hittable: sim.tick,
        purpose: sim.tick,
        look: None,
        aim: None,
    });
    let g = seat_gun(sim, m.vehicle, m.seat);
    if thinking {
        if let Some(g) = g {
            let known = known(sim, u.team);
            let (hit, ok) = purpose(sim, m.vehicle, g, u.team, &known);
            man.look = hit;
            if hit.is_some() {
                man.hittable = sim.tick;
            }
            if ok {
                man.purpose = sim.tick;
            }
        }
        let rule = match g {
            Some(g) => exit_rule(sim, m.vehicle, g, u.team, &man, u.last_shot_tick),
            None => Some(5),
        };
        if let Some(rule) = rule {
            nt.man = rule;
            st.ride = Some(RideState {
                use_req: true,
                board: None,
                man: None,
                closed: Some(m.vehicle),
                closed_until: sim.tick + MAN_LEFT_CLOSED,
                ..r
            });
            return;
        }
    }
    r.man = Some(man);
    st.ride = Some(r);
    let idle = st
        .target
        .and_then(|h| perception::contact_of(st, h))
        .map(|c| c.last_pos)
        .or(man.look);
    let at = super::gunner::aim(sim, me, st, pad, super::gunner::Look::Seat, idle);
    if let Some(m) = st.ride.as_mut().and_then(|r| r.man.as_mut()) {
        m.aim = at;
    }
}

#[cfg(test)]
mod tests;
