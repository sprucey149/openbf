//! Keyboard + mouse look in the sim: seat-gun look by angle, hover and walker
//! mouse driving, and the between-tick view preview. OURS: keyboard + mouse is a
//! deliberate deviation (public pull request #1); rules in `analysis/plan/FEATURE_SET.md`, Platform & meta
//! (Keyboard + mouse).

use super::input::PadState;
use super::state::Sim;
use crate::fmath::FloatExt;
use crate::unit::UnitHandle;
use crate::vehicle::gun::GunSetup;
use crate::vehicle::walker::{WalkerParams, WalkerState};
use crate::vehicle::{Vehicle, VehicleKind};

pub const MOUSE_STEER: f32 = 0.03;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LookPreview {
    Foot {
        unit: usize,
        yaw: f32,
        pitch: f32,
    },
    Seat {
        vehicle: usize,
        seat: usize,
        look: (f32, f32),
        walker: bool,
    },
    Droid {
        droid: usize,
        yaw: f32,
        pitch: f32,
    },
}

pub(super) fn seat_look(
    g: &GunSetup,
    look: (f32, f32),
    d: (f32, f32),
    zoom: Option<f32>,
) -> (f32, f32) {
    let k = zoom.map_or(1.0, |z| 1.0 / z.fmax(1.0));
    let (dy, dp) = (-d.0 * k, d.1 * k);
    if let Some(t) = g.turn {
        return t.hold(look.0 + dy, look.1 + dp);
    }
    let a = g.aimer;
    let yaw = if g.pilot {
        0.0
    } else {
        (look.0 + dy).clamp(a.yaw[0], a.yaw[1])
    };
    (yaw, (look.1 + dp).clamp(a.pitch[0], a.pitch[1]))
}

pub(super) fn walker_aim(w: &WalkerState, p: &WalkerParams, d: (f32, f32)) -> (f32, f32) {
    walker_aim_from((w.aim_yaw, w.aim_pitch), p, d)
}

fn walker_aim_from(aim: (f32, f32), p: &WalkerParams, d: (f32, f32)) -> (f32, f32) {
    let yaw = aim.0 - d.0;
    let pitch = (aim.1 + d.1).clamp(p.pitch_limits[0], p.pitch_limits[1]);
    (yaw, pitch)
}

/// The soldier of `h` aimed where an earlier pad's preview `from` left him
/// (a copy; the sim's own is untouched).
fn foot_from(
    s: &crate::soldier::Soldier,
    h: UnitHandle,
    from: Option<LookPreview>,
) -> std::borrow::Cow<'_, crate::soldier::Soldier> {
    match from {
        Some(LookPreview::Foot { unit, yaw, pitch }) if unit == h.index as usize => {
            let mut s = s.clone();
            (s.yaw, s.pitch) = (yaw, pitch);
            std::borrow::Cow::Owned(s)
        }
        _ => std::borrow::Cow::Borrowed(s),
    }
}

/// The seat look an earlier pad's preview `from` left for `vehicle`'s `seat`.
fn seat_from(from: Option<LookPreview>, vehicle: usize, seat: usize) -> Option<(f32, f32)> {
    match from {
        Some(LookPreview::Seat {
            vehicle: v,
            seat: s,
            look,
            ..
        }) if v == vehicle && s == seat => Some(look),
        _ => None,
    }
}

/// A stick's `(turn, pitch)` turned by a mouse look, as both a flyer's pad
/// and a guided rocket read it.
pub(super) fn look_steer(turn: f32, pitch: f32, d: (f32, f32)) -> (f32, f32) {
    (
        (turn + d.0 / MOUSE_STEER).clamp(-1.0, 1.0),
        (pitch + d.1 / MOUSE_STEER).clamp(-1.0, 1.0),
    )
}

pub(super) fn mouse_drive(
    v: &mut Vehicle,
    pad: &PadState,
    full: bool,
    walker: Option<&WalkerParams>,
) -> bool {
    if !pad.has_look() {
        return full;
    }
    let (dy, dp) = pad.look();
    if v.kind == VehicleKind::Walker {
        (v.walker.aim_yaw, v.walker.aim_pitch) = match walker {
            Some(p) => walker_aim(&v.walker, p, (dy, dp)),
            None => (v.walker.aim_yaw - dy, v.walker.aim_pitch + dp),
        };
        return full;
    }
    let (turn, pitch) = look_steer(v.input.turn, v.input.pitch, (dy, dp));
    v.input.turn = turn;
    if v.kind == VehicleKind::Flyer {
        v.input.pitch = pitch;
    }
    full || turn * turn + v.input.pitch * v.input.pitch >= 0.9
}

impl Sim {
    /// The aim the next tick's look step turns to under `pad` (the window's
    /// between-tick view and reticle). Presentation only.
    pub fn look_preview(&self, h: UnitHandle, pad: &PadState) -> Option<LookPreview> {
        self.look_preview_from(h, pad, None)
    }

    /// Issue #168 (LAN §6): [`Sim::look_preview`] folded over several pads in
    /// order - the aim after a lockstep peer's pending inputs land, each
    /// pad's mouse look or held-stick turn applied to the aim the pads
    /// before it left. Presentation only: nothing is written. A pad with no
    /// preview of its own (a flown craft, a frozen look) keeps the aim the
    /// earlier pads left.
    ///
    /// What the fold does not foresee (stepping the pads does): the assist's
    /// magnet and a droideka's look are read from the current state for
    /// every pad, and a stance, zoom, seat or life change inside the pads
    /// is not applied.
    pub fn look_preview_over(&self, h: UnitHandle, pads: &[PadState]) -> Option<LookPreview> {
        let mut aim = None;
        for pad in pads {
            if let Some(p) = self.look_preview_from(h, pad, aim) {
                aim = Some(p);
            }
        }
        aim
    }

    /// [`Sim::look_preview`] from the aim `from` (an earlier pad's preview
    /// of the same unit) instead of the current one.
    fn look_preview_from(
        &self,
        h: UnitHandle,
        pad: &PadState,
        from: Option<LookPreview>,
    ) -> Option<LookPreview> {
        if !pad.has_look() && !pad.mouse_aim() {
            return self.stick_preview(h, pad, from);
        }
        let u = self.unit(h).filter(|u| u.alive)?;
        if let Some(k) = self
            .droids
            .iter()
            .position(|d| d.owner == h && d.controlled)
        {
            let mut d = self.droids[k].clone();
            if let Some(LookPreview::Droid { droid, yaw, pitch }) = from {
                if droid == k {
                    (d.yaw, d.pitch) = (yaw, pitch);
                }
            }
            let c = self.droid_class(&d)?;
            let (yaw, pitch) = d.mouse_turned(c, pad.look(), u.look_scale());
            return Some(LookPreview::Droid {
                droid: k,
                yaw,
                pitch,
            });
        }
        if self.controlled_guided(h).is_some() {
            return None;
        }
        if u.droideka.is_some() && u.mount.is_none() {
            let (yaw, pitch) = self.droideka_look_preview(h.index as usize, pad)?;
            return Some(LookPreview::Foot {
                unit: h.index as usize,
                yaw,
                pitch,
            });
        }
        let Some(m) = u.mount else {
            if u.soldier.look_frozen() || self.saber_look_locked(h.index as usize) {
                return None;
            }
            let (yaw, pitch) =
                foot_from(&u.soldier, h, from).mouse_turned(pad.look(), u.look_scale(), &u.consts);
            return Some(LookPreview::Foot {
                unit: h.index as usize,
                yaw,
                pitch,
            });
        };
        let g = self.seat_gun_setup(h)?;
        let vehicle = m.vehicle.index as usize;
        let seat = usize::from(m.seat);
        let v = self.vehicles.get(vehicle)?;
        if g.pilot && v.kind == VehicleKind::Flyer {
            // Mouse input drives the craft's angular motion, not an
            // independent seat look. Its rendered pose supplies that view.
            return None;
        }
        if g.pilot && v.kind == VehicleKind::Walker {
            let p = self.walker_params.get(v.class as usize)?.as_ref()?;
            let aim =
                seat_from(from, vehicle, seat).unwrap_or((v.walker.aim_yaw, v.walker.aim_pitch));
            return Some(LookPreview::Seat {
                vehicle,
                seat,
                look: walker_aim_from(aim, p, pad.look()),
                walker: true,
            });
        }
        let st = v.guns.get(seat)?.as_ref()?;
        let look = seat_from(from, vehicle, seat).unwrap_or(st.look);
        Some(LookPreview::Seat {
            vehicle,
            seat,
            look: seat_look(&g, look, pad.look(), st.zoom),
            walker: false,
        })
    }
}

impl Sim {
    /// [ours] (the maintainer, 2026-10-29): a controller's preview on foot,
    /// as the mouse's: the aim the next tick's look step turns to under
    /// `pad` - the aim assist on the stick as that tick applies it (last
    /// tick's magnet), the zoom's divided rates, the stance's turn factor -
    /// so the reticle is drawn where the next tick fires. The tick's zoom
    /// press is not foreseen. `None` in a seat, flying a droid or a rocket.
    fn stick_preview(
        &self,
        h: UnitHandle,
        pad: &PadState,
        from: Option<LookPreview>,
    ) -> Option<LookPreview> {
        let u = self.unit(h).filter(|u| u.alive && u.mount.is_none())?;
        if self.controlled_droid(h).is_some() || self.controlled_guided(h).is_some() {
            return None;
        }
        // A saber combo that locks the turn: the tick drops the stick (and
        // the assist's pull on it), so nothing is drawn ahead of it.
        if self.saber_look_locked(h.index as usize) {
            return None;
        }
        let slot = self.players.iter().position(|p| *p == Some(h));
        let magnet = slot.and_then(|k| Some((k, self.magnet.get(k).copied().flatten()?)));
        let pad = magnet
            .and_then(|(k, m)| self.assisted(k, h, m, pad, crate::TICK_DT))
            .map_or(*pad, |(p, _, _)| p);
        if u.droideka.is_some() {
            let (yaw, pitch) = self.droideka_look_preview(h.index as usize, &pad)?;
            return Some(LookPreview::Foot {
                unit: h.index as usize,
                yaw,
                pitch,
            });
        }
        let mut c = u.consts;
        // The tick's own rates: a combo state's TurnFactor, a force power's.
        self.saber_constants(h.index as usize, &mut c);
        self.force_constants(h.index as usize, &mut c);
        if let Some(z) = u.zoom {
            c.max_turn_speed /= z.divisor;
            c.max_pitch_speed /= z.divisor;
        }
        let (yaw, pitch) =
            foot_from(&u.soldier, h, from).stick_turned(crate::TICK_DT, &pad, &c, u.is_player);
        Some(LookPreview::Foot {
            unit: h.index as usize,
            yaw,
            pitch,
        })
    }
}

#[cfg(test)]
mod tests;
