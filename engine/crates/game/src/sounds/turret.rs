//! The MountedTurrets' sounds (`analysis/re-vehicle-sounds.md` 8): every
//! gunner seat with a turn law - a vehicle's TURRET section or a building
//! turret's - whirs while its look turns, plays `TurretActivateSound` /
//! `TurretDeactivateSound` when its gunner changes and holds
//! `TurretAmbientSound` while it has one. B1 `FUN_00224f48`
//! (B1:137901-138010), B2 `FUN_0026edc8`, called once per update for the
//! pitch axis and then the yaw axis at the end of the turret's aim update
//! (B1:137306-137311), inside the test of its owner's live flag.

use super::{Listener, LoopRequest, LoopSource, SoundLib};
use crate::classdb::fnv1a_lowercase;
use crate::vehicle_class::TurretSounds;
use crate::walk::WalkSim;
use core::unit::UnitHandle;
use std::collections::HashMap;

/// A turret seat: vehicle slot, its generation, seat.
pub(super) type SeatKey = (u32, u32, u8);

/// One whir voice's own numbers: the handle's gain over the property's and
/// the pitch it was last set to, over the property's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Axis {
    pub gain: f32,
    pub pitch: f32,
}

/// A turret seat's sound state between updates.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Whir {
    /// The look last update as the PS2 compares it, `(int)(angle * 100)`
    /// on BF1 (B1:137306-137310), `(int)(angle * 1000)` on BF2
    /// (B2:188702-188706): (yaw, pitch).
    look: (i32, i32),
    /// The gunner last seen (T+0x3d4).
    pilot: Option<UnitHandle>,
    pub yaw: Option<Axis>,
    pub pitch: Option<Axis>,
}

/// The voice's gain falls by this share of itself each update the axis
/// rests (B1:138149-138152), and stops at or under [`QUIET`] (B1:138141).
const FADE: f32 = 0.4;
const QUIET: f32 = 0.0001;

/// One axis's update (B1 `FUN_00224f48`): `moved` when the angle's
/// hundredths changed; `rate` / `max` the axis's slewed rate over its
/// full-stick rate; `low` the class's `Turret*SoundPitch`; `gain` and
/// `pitch` the property's own. The new voice numbers, `None` once silent.
///
/// Resting, a playing voice's gain loses 40 % a step and the voice stops at
/// 0.0001. Turning, a voice not playing starts at the property's pitch and
/// gain; r = |rate| / max sets the pitch to ((1 - r) low + r) x the
/// property's, kept between `low` and the property's pitch (B1:138187-138193),
/// and the gain to clamp(r x the property's gain, 0, 1). The stored gain's
/// rise by a tenth of the property's while under it (B1:138168-138171)
/// never shows: a start sets it to the property's gain.
pub(super) fn axis(
    was: Option<Axis>,
    moved: bool,
    rate: f32,
    max: f32,
    low: f32,
    gain: f32,
    pitch: f32,
) -> Option<Axis> {
    if !moved {
        let a = was?;
        let g = gain * a.gain;
        if g <= QUIET {
            return None;
        }
        return Some(Axis {
            gain: (g - g * FADE).clamp(0.0, 1.0) / gain,
            ..a
        });
    }
    if max <= 0.0 {
        return was;
    }
    let r = rate.abs() / max;
    let p = ((1.0 - r) * low + r) * pitch;
    let p = match (low <= p, p < pitch) {
        (false, _) => low,
        (true, true) => p,
        (true, false) => pitch,
    };
    let over = |x: f32| if x > f32::MIN_POSITIVE { x } else { f32::MIN_POSITIVE };
    Some(Axis {
        gain: (r * gain).clamp(0.0, 1.0) / over(gain),
        pitch: p / over(pitch),
    })
}

/// A seat's sound keys, turn law and state now, for every live vehicle's
/// gunner seat whose class authors any turret sound.
struct Seat<'a> {
    key: SeatKey,
    sounds: &'a TurretSounds,
    look: (f32, f32),
    rate: (f32, f32),
    /// The full-stick rates. The PS2 passes them times 1 / the zoom while
    /// zoomed (B1:137196-137233), the same factor its slewed rate carries;
    /// the port's turn reads no zoom, so the share r is the PS2's unzoomed.
    max: (f32, f32),
    pilot: Option<UnitHandle>,
    pos: glam::Vec3,
}

fn seats(walk: &WalkSim) -> Vec<Seat<'_>> {
    let sim = &walk.sim;
    let mut out = Vec::new();
    for (slot, v) in sim.vehicles.iter().enumerate() {
        if !v.alive || v.health.cur <= 0.0 {
            continue;
        }
        let Some(specs) = walk.vehicle_seats.get(v.class as usize) else {
            continue;
        };
        let Some(setup) = sim.seat_setup.get(v.class as usize).and_then(Option::as_ref) else {
            continue;
        };
        for (s, spec) in specs.iter().enumerate() {
            let t = &spec.turret_sounds;
            if t.yaw.is_none() && t.pitch.is_none() && t.activate.is_none() && t.deactivate.is_none()
                && t.ambient.is_none()
            {
                continue;
            }
            let Some(turn) = setup.guns.get(s).and_then(Option::as_ref).and_then(|g| g.turn) else {
                continue;
            };
            let Some(state) = v.guns.get(s).and_then(Option::as_ref) else {
                continue;
            };
            out.push(Seat {
                key: (slot as u32, v.gen, s as u8),
                sounds: t,
                look: state.look,
                rate: state.rate,
                max: (turn.yaw_speed, turn.pitch_speed),
                pilot: v.seats.get(s).and_then(|x| x.occupant),
                pos: v.pos,
            });
        }
    }
    out
}

/// The angle as the PS2 compares it for "moved" (see [`Whir::look`]).
fn scaled(a: f32, bf2: bool) -> i32 {
    (a * if bf2 { 1000.0 } else { 100.0 }) as i32
}

impl SoundLib {
    /// The turret seats' whirs, gunner one-shots and ambient loops for the
    /// tick the sim just stepped, at each turret's owner (B1:137307-137310
    /// pass its matrix's position). A seat first seen this tick starts from
    /// its own look and no gunner, as the PS2's constructor leaves T+0x3d4.
    pub fn play_turret_sounds(&mut self, audio: &mut audio::Audio, walk: &WalkSim, listener: &Listener) {
        let mut live = std::collections::HashSet::new();
        let mut keep = HashMap::new();
        let mut shots: Vec<(String, glam::Vec3)> = Vec::new();
        let bf2 = walk.sim.ground_rules == core::soldier::GroundRules::Bf2;
        for seat in seats(walk) {
            let now = (scaled(seat.look.0, bf2), scaled(seat.look.1, bf2));
            let mut w = self.turret_whirs.remove(&seat.key).unwrap_or(Whir {
                look: now,
                ..Whir::default()
            });
            let s = seat.sounds;
            for (yaw, name, low) in [(false, &s.pitch, s.pitch_low), (true, &s.yaw, s.yaw_low)] {
                let Some(name) = name.as_deref() else {
                    continue;
                };
                let hash = fnv1a_lowercase(name);
                let Some((gain, pitch)) = self.find(hash).map(|(p, _)| (p.gain, p.pitch)) else {
                    continue;
                };
                let (was, moved, rate, max) = if yaw {
                    (w.yaw, now.0 != w.look.0, seat.rate.0, seat.max.0)
                } else {
                    (w.pitch, now.1 != w.look.1, seat.rate.1, seat.max.1)
                };
                let next = axis(was, moved, rate, max, low, gain, pitch);
                if yaw {
                    w.yaw = next;
                } else {
                    w.pitch = next;
                }
                let Some(a) = next else { continue };
                let (sk, sg, ss) = seat.key;
                let request = LoopRequest {
                    source: if yaw {
                        LoopSource::TurretYaw(sk, sg, ss)
                    } else {
                        LoopSource::TurretPitch(sk, sg, ss)
                    },
                    sound: hash,
                    pos: seat.pos,
                    gain: a.gain,
                    pitch: a.pitch,
                };
                if self.play_loop_voice(audio, &request, listener) {
                    live.insert(request.source);
                }
            }
            if seat.pilot != w.pilot {
                let shot = if seat.pilot.is_some() { &s.activate } else { &s.deactivate };
                if let Some(name) = shot {
                    shots.push((name.clone(), seat.pos));
                }
                w.pilot = seat.pilot;
            }
            if let (Some(name), Some(_)) = (&s.ambient, seat.pilot) {
                let (sk, sg, ss) = seat.key;
                let request = LoopRequest {
                    source: LoopSource::TurretAmbient(sk, sg, ss),
                    sound: fnv1a_lowercase(name),
                    pos: seat.pos,
                    gain: 1.0,
                    pitch: 1.0,
                };
                if self.play_loop_voice(audio, &request, listener) {
                    live.insert(request.source);
                }
            }
            w.look = now;
            keep.insert(seat.key, w);
        }
        self.turret_whirs = keep;
        self.loops.retain(|k, _| {
            !matches!(
                k,
                LoopSource::TurretYaw(..) | LoopSource::TurretPitch(..) | LoopSource::TurretAmbient(..)
            ) || live.contains(k)
        });
        for (name, pos) in shots {
            self.play(audio, &name, pos, listener);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turning_axis_sets_pitch_and_gain_by_its_rate_share() {
        let full = axis(None, true, 1.3, 1.3, 0.7, 1.0, 1.0).unwrap();
        assert_eq!((full.gain, full.pitch), (1.0, 1.0));
        let half = axis(None, true, -0.65, 1.3, 0.7, 1.0, 1.0).unwrap();
        assert!((half.gain - 0.5).abs() < 1e-6);
        assert!((half.pitch - 0.85).abs() < 1e-6);
        let slow = axis(None, true, 0.0, 1.3, 0.7, 1.0, 1.0).unwrap();
        assert!((slow.pitch - 0.7).abs() < 1e-6);
        assert_eq!(slow.gain, 0.0);
    }

    #[test]
    fn a_resting_axis_fades_by_forty_percent_an_update_then_stops() {
        let mut a = axis(None, true, 1.0, 1.0, 0.7, 1.0, 1.0);
        assert!(axis(None, false, 0.0, 1.0, 0.7, 1.0, 1.0).is_none(), "nothing playing");
        let mut steps = 0;
        let mut gains = Vec::new();
        while let Some(x) = a {
            gains.push(x.gain);
            a = axis(a, false, 0.0, 1.0, 0.7, 1.0, 1.0);
            steps += 1;
            assert!(steps < 100);
        }
        assert!((gains[1] - 0.6).abs() < 1e-6 && (gains[2] - 0.36).abs() < 1e-6);
        assert!(gains.last().unwrap() * 1.0 <= QUIET);
    }
}
