//! A bot's seat gun (card VTAI): the pad a seated bot makes for its seat's
//! weapons. `Sim::seat_gun` then moves the look and the aimer and fires, as
//! it does for a human, and holds the shot to the fairness rule.
//!
//! The rules are on foot's (`behave::think`, `gunner_foot::run`): the target is the one this
//! bot's perception SEES (`perception::scan` looks from the seat's eye along
//! the gun's look), aimed at through the lead point and the aim error (BF2: the target's own);
//! FIRE waits for the tier's reaction, the weapon's range, D11's bursts and
//! pauses, and the gun within [`AIM_TOLERANCE`] of the aim point. Card
//! BRSTU: a seat gun with unlimited rounds bursts on its heat, not a shot
//! count, and a single-shot seat gun is tapped.
//!
//! OURS, PROVISIONAL (the PS2's AI gunner, B1 FUN_00223850's MountedTurret
//! acquisition, re-vehicles §9, is located, not transcribed): the look goes
//! straight at the aim point at the aimer's full stick rates — the seat's
//! rates stand in for on foot's D9 tracker; the tolerance is a cone about the
//! BARREL (on foot's is per axis about the look); with nothing seen the look
//! returns to the aimer's rest (card TURR: a turret gunner's to the last
//! known point of what it can hit). Card STCHG shares on foot's charge
//! release policy (re-turret-ai §7; re-ai-difficulty §4.4), before tap dispatch.
//! A building turret's look turns by the PS2's rate law (card VTURL,
//! `gun::Turn`): the bot asks for the rate that would stop on its aim -
//! at most what the filter can brake from (BF1 `gap x filter`, BF2
//! `sqrt(2 filter gap)`) and the gap in one tick - and turns the short way
//! round a turret that wraps. Its AIMER takes the point [`aim`] returns
//! (`ai::ManState::aim`, card TURR) in place of the camera ray's hit, as the
//! PS2's does for a seat with no player (B1 `FUN_0026d748` hands it the AI's
//! own aim, `FUN_00236700`, B1:191963-191967); and it sees from its muzzle
//! (`perception::scan`).

use super::gunner_foot::{charge, droid_aim, fire_discipline, seen_aim, tap, turret_aim, Meter};
use super::state::AiState;
use super::AIM_TOLERANCE;
use crate::fmath;
use crate::perception;
use crate::sim::{buttons, PadState, Sim};
use crate::unit::UnitHandle;
use crate::vehicle::gun;

/// Whose look [`aim`] moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Look {
    /// A gunner: both axes at the aimer's rates.
    Seat,
    /// A hover's pilot: the right stick's x is the driver's turn, so only
    /// the pitch moves.
    Steering,
    /// A walker's pilot (card WAI): the look is the walker's own aim, moved
    /// by its integrator at the class's rates (re-walker §3 I7-I15) — the
    /// PS2's AI aims it with the same one-tick stick (re-walker-ai §4).
    Walker { yaw_rate: f32, pitch_rate: f32 },
}

/// Aim `me`'s seat gun and press FIRE when the shot is on. With no target
/// seen the look goes to `idle` (card TURR: a turret gunner's last known
/// point of what it can hit), else to the rest. Returns the point aimed at
/// (the seen target's aim point, else `idle`).
pub(super) fn aim(
    sim: &Sim,
    me: UnitHandle,
    st: &mut AiState,
    pad: &mut PadState,
    look: Look,
    idle: Option<glam::Vec3>,
) -> Option<glam::Vec3> {
    let (Some(s), Some(u)) = (sim.seat_look(me), sim.unit(me)) else {
        return None;
    };
    let w = s.gun.weapons.first()?;
    let wc = &sim.weapon_classes[w.class];
    let skill = sim.skill_of(u);
    let tier = sim.unit_tier(u);
    let held = st.target.and_then(|h| perception::contact_of(st, h));
    let seen = held.filter(|c| c.seen_now);
    let enemy = seen.and_then(|c| c.handle.live_unit(sim));
    let droid = seen
        .and_then(|c| c.handle.droid())
        .and_then(|id| sim.live_droid(id));
    let turret = seen
        .and_then(|c| c.handle.turret())
        .and_then(|id| sim.live_turret(id));
    let point = enemy
        .map(|e| seen_aim(sim, me, st, s.eye, e, wc.shot_ordnance().velocity, wc))
        .or_else(|| {
            droid.map(|d| droid_aim(sim, me, st, s.eye, d, wc.shot_ordnance().velocity, wc))
        })
        .or_else(|| turret.map(|t| turret_aim(sim, me, st, s.eye, t, wc)));
    let centre = enemy
        .map(|e| e.capsule().center())
        .or(droid.map(|d| d.pos))
        .or(turret.map(|t| t.centre()));
    let at = point.or(idle);

    if let (Look::Seat, Some(t)) = (look, s.gun.turn) {
        let want = at.map(|p| gun::angles(s.rot.inverse() * (p - s.eye)));
        turn_stick(&t, s.look, want, pad);
    } else {
        // The look: `Sim::seat_gun` turns yaw by −rx·rate·dt and pitch by
        // +ry·rate·dt, so the stick that closes the gap in one tick, clamped.
        let a = s.gun.aimer;
        let want = at.map_or(a.rest(), |p| gun::angles(s.rot.inverse() * (p - s.eye)));
        let want = (
            want.0.clamp(a.yaw[0], a.yaw[1]),
            want.1.clamp(a.pitch[0], a.pitch[1]),
        );
        let dt = crate::TICK_DT;
        let (yaw_rate, pitch_rate) = match look {
            Look::Walker {
                yaw_rate,
                pitch_rate,
            } => (yaw_rate, pitch_rate),
            _ => (a.yaw_rate, a.pitch_rate),
        };
        let yaws = match look {
            Look::Seat => !s.gun.pilot,
            Look::Steering => false,
            Look::Walker { .. } => true,
        };
        if yaws && yaw_rate > 0.0 {
            pad.rx = PadState::quantize(((s.look.0 - want.0) / (yaw_rate * dt)).clamp(-1.0, 1.0));
        }
        if pitch_rate > 0.0 {
            pad.ry = PadState::quantize(((want.1 - s.look.1) / (pitch_rate * dt)).clamp(-1.0, 1.0));
        }
    }

    // Card BRSTU: the seat gun's own heat, and its rounds - unlimited when
    // the class's `RoundsPerClip` is under 1 or the slot's `WeaponAmmo` is.
    let meter = Meter {
        heat: seat_heat(sim, u).unwrap_or(0.0),
        unlimited: w.infinite || wc.infinite_ammo(),
    };
    fire_discipline(sim, me, u, st, wc, meter, &skill);
    // A building turret's aimer was handed last tick's point (`ManState::aim`)
    // and turns about its pivot to this tick's before the shot leaves.
    let last = st.ride.and_then(|r| r.man).and_then(|m| m.aim);
    let pivot = u
        .mount
        .and_then(|m| sim.vehicle(m.vehicle))
        .map(|v| v.pos + s.rot * s.gun.pivot);
    let reach = s.gun.aimer.yaw_rate.min(s.gun.aimer.pitch_rate) * crate::TICK_DT;
    let shot = centre.zip(point).is_some_and(|(c, p)| {
        let cos_tol = fmath::sin_cos(AIM_TOLERANCE).1;
        let to = |q: glam::Vec3| (q - s.muzzle).normalize_or_zero();
        // [ours] Or the barrel is on last tick's point and the aimer, at its
        // rate, reaches this tick's: the barrel read here trails the point
        // by a tick. At the Geonosis cannon (muzzle 2.5 m before the pivot)
        // a man walking round it 5 m out moves 3.4 deg a tick seen from the
        // muzzle, past the cone on every tick once the aim point moved
        // smoothly (`gunner_foot::error_dir`); before, only a think's jump
        // against his walk let a shot go.
        let turned = |q: glam::Vec3, o: glam::Vec3| {
            (q - o).normalize_or_zero().dot((p - o).normalize_or_zero()) >= fmath::sin_cos(reach).1
        };
        let on = to(p).dot(s.barrel) >= cos_tol
            || last
                .zip(pivot)
                .is_some_and(|(q, o)| to(q).dot(s.barrel) >= cos_tol && turned(q, o));
        let range = (c - s.muzzle).length();
        let ready = held.is_some_and(|c| perception::fireable(&c, sim.tick, tier.reaction_ticks));
        ready && on && range <= wc.shot_range()
    });
    if wc.max_pressed_time > 0.0 {
        if centre.is_some() {
            let want_fire = shot && st.burst > 0 && sim.tick >= st.pause_until;
            charge(sim, me, st, wc, &skill, want_fire, false, pad);
        } else {
            if wc.trigger_single {
                tap(sim, me, st, wc, &skill, false);
            }
            st.charge_until = 0;
        }
    } else if wc.trigger_single {
        // Card BRSTU: a single-shot seat gun is TAPPED, by on foot's rule
        // (card STTAP; one trigger function for every seat on the PS2,
        // `re-soldier-ai.md` 23.2) - a held trigger fires it once.
        if tap(sim, me, st, wc, &skill, shot) {
            pad.buttons |= buttons::FIRE;
        }
    } else if shot && st.burst > 0 && sim.tick >= st.pause_until {
        pad.buttons |= buttons::FIRE;
    }
    at
}

/// The heat of the first weapon of `u`'s seat gun.
fn seat_heat(sim: &Sim, u: &crate::unit::Unit) -> Option<f32> {
    let m = u.mount?;
    let g = sim
        .vehicle(m.vehicle)?
        .guns
        .get(usize::from(m.seat))?
        .as_ref()?;
    Some(g.weapons.first()?.heat)
}

/// The sticks that turn a [`gun::Turn`] look toward `want` (the rest when
/// `None`) and stop there.
fn turn_stick(t: &gun::Turn, look: (f32, f32), want: Option<(f32, f32)>, pad: &mut PadState) {
    let want = want.unwrap_or(t.rest());
    let want = t.target(want.0, want.1);
    let dt = crate::TICK_DT;
    let axis = |cur: f32, w: f32, l: [f32; 2], wraps: bool, speed: f32, filter: f32| {
        let gap = if wraps {
            gun::wrap(w - cur)
        } else {
            w.clamp(l[0], l[1]) - cur
        };
        let brake = match t.slew {
            gun::Slew::Exp => gap.abs() * filter,
            gun::Slew::Linear => (2.0 * filter * gap.abs()).sqrt(),
        };
        let rate = brake.min(gap.abs() / dt).copysign(gap);
        if speed > 0.0 {
            (rate / speed).clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    let y = axis(look.0, want.0, t.yaw, t.yaw_wrap, t.yaw_speed, t.yaw_filter);
    let p = axis(
        look.1,
        want.1,
        t.pitch,
        t.pitch_wrap,
        t.pitch_speed,
        t.pitch_filter,
    );
    pad.rx = PadState::quantize(-y);
    pad.ry = PadState::quantize(p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle::gun::{wrap, Slew, Turn};

    #[test]
    fn a_rear_gunner_tracks_across_the_signed_yaw_seam_inside_its_arc() {
        let t = Turn::from_class(
            Some([120.0, 240.0]),
            None,
            (1.0, 1.0),
            (3.75, 3.75),
            Slew::Linear,
        );
        for degrees in [-170.0f32, 170.0] {
            let want = (degrees.to_radians(), 0.0);
            let (mut look, mut rate) = (t.rest(), (0.0, 0.0));
            for _ in 0..120 {
                let mut pad = PadState::default();
                turn_stick(&t, look, Some(want), &mut pad);
                (look, rate) = t.step(
                    look,
                    rate,
                    (PadState::axis(pad.rx), PadState::axis(pad.ry)),
                    crate::TICK_DT,
                );
                assert!((t.yaw[0]..=t.yaw[1]).contains(&look.0));
            }
            assert!(wrap(look.0 - want.0).abs() < 0.005, "{look:?} {want:?}");
        }
    }

    const DEG: f32 = std::f32::consts::PI / 180.0;

    /// Card VTURL: on a full-circle turret (BF2's dish, BF1's gun turret)
    /// the bot's stick turns the look the short way across the seam, and
    /// settles on its aim within a degree without swinging past it by more
    /// than two.
    #[test]
    fn a_bots_stick_turns_a_full_circle_turret_the_short_way_and_stops() {
        for (slew, speed) in [(Slew::Linear, 1.0), (Slew::Exp, 0.85)] {
            let t = Turn::from_class(
                Some([-180.0, 180.0]),
                Some([-10.0, 35.0]),
                (speed, speed),
                (10.0, 10.0),
                slew,
            );
            let want = (170.0 * DEG, -20.0 * DEG);
            let (mut look, mut rate) = ((-170.0 * DEG, 0.0), (0.0, 0.0));
            let mut past = 0.0f32;
            for _ in 0..60 {
                let mut pad = PadState::default();
                turn_stick(&t, look, Some(want), &mut pad);
                let stick = (PadState::axis(pad.rx), PadState::axis(pad.ry));
                (look, rate) = t.step(look, rate, stick, crate::TICK_DT);
                // The short way is DOWN through -180: never up past -170.
                assert!(wrap(look.0 + 170.0 * DEG) <= 1e-4, "{slew:?}: {look:?}");
                past = past.max(-wrap(look.0 - want.0));
            }
            assert!(wrap(look.0 - want.0).abs() < DEG, "{slew:?}: {look:?}");
            assert!((look.1 - want.1).abs() < DEG, "{slew:?}: {look:?}");
            assert!(past < 2.0 * DEG, "{slew:?}: swung {} deg past", past / DEG);
        }
    }
}
