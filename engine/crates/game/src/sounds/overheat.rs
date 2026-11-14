//! The weapons' overheat loops ([`core::weapon::OverheatSound`]): every
//! unit's weapon that sits overheated in its idle state, at its owner.

use super::{Listener, LoopRequest, LoopSource, SoundLib};
use core::sim::Sim;
use core::unit::UnitHandle;
use core::weapon::{FireState, WeaponClass, WeaponState};
use glam::Vec3;

/// One weapon's overheat loop this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice<'a> {
    pub unit: UnitHandle,
    pub class: usize,
    pub name: &'a str,
    pub pos: Vec3,
    /// Over the sound's own pitch ([`core::weapon::OverheatSound::rate`]).
    pub rate: f32,
}

fn voice<'a>(
    unit: UnitHandle,
    class: usize,
    wc: &'a WeaponClass,
    s: &WeaponState,
    pos: Vec3,
) -> Option<Voice<'a>> {
    let sound = wc.overheat_sound.as_ref()?;
    (s.active && s.overheated && s.state == FireState::Idle).then(|| Voice {
        unit,
        class,
        name: &sound.name,
        pos,
        rate: sound.rate(s.entered_idle(), wc.heat_threshold, s.heat),
    })
}

/// The loops sounding now: a soldier's weapon in hand at the soldier, a
/// seat's guns at their vehicle. The PS2 sets the voice at the owner's
/// matrix (B1 `FUN_00284a58` B1:208456-208461, B2 `FUN_003269f8`).
/// OURS: a holstered weapon (`WeaponState::active` false) is silent, as the
/// seat fire loops are (`crate::walker_shots::voices`).
/// trace: a holstered or empty-seat weapon's idle update on the PS2.
pub fn voices(sim: &Sim) -> Vec<Voice<'_>> {
    let mut out = Vec::new();
    for (h, u) in sim.alive_units() {
        let Some(m) = u.mount else {
            let (class, _, s) = u.active_weapon();
            if let Some(wc) = sim.weapon_classes.get(class) {
                out.extend(voice(h, class, wc, s, u.soldier.pos));
            }
            continue;
        };
        let Some(v) = sim.vehicle(m.vehicle) else {
            continue;
        };
        let Some(gun) = sim
            .seat_setup
            .get(v.class as usize)
            .and_then(Option::as_ref)
            .and_then(|s| s.guns.get(m.seat as usize))
            .and_then(Option::as_ref)
        else {
            continue;
        };
        let Some(state) = v.guns.get(m.seat as usize).and_then(Option::as_ref) else {
            continue;
        };
        for (w, s) in gun.weapons.iter().zip(&state.weapons) {
            if let Some(wc) = sim.weapon_classes.get(w.class) {
                out.extend(voice(h, w.class, wc, s, v.pos));
            }
        }
    }
    out
}

impl SoundLib {
    /// Play [`voices`]: each started the tick its weapon enters the idle
    /// state overheated, set to its rate every tick, stopped the tick the
    /// overheat clears.
    pub fn play_overheat_loops(
        &mut self,
        audio: &mut audio::Audio,
        sim: &Sim,
        listener: &Listener,
    ) {
        let requests = voices(sim).into_iter().map(|v| LoopRequest {
            source: LoopSource::Overheat(v.unit, v.class),
            sound: crate::classdb::fnv1a_lowercase(v.name),
            pos: v.pos,
            gain: 1.0,
            pitch: v.rate,
        });
        self.play_loop_set(audio, listener, requests, |k| {
            matches!(k, LoopSource::Overheat(..))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::weapon::OverheatSound;

    fn aat_cannon() -> WeaponClass {
        let mut wc = WeaponClass::default();
        wc.heat_per_shot = 0.15;
        wc.heat_recover_rate = 0.25;
        wc.heat_threshold = 0.2;
        wc.overheat_sound = Some(OverheatSound {
            name: "com_weap_energy_depleted".into(),
            pitch: 0.5,
        });
        wc
    }

    #[test]
    fn the_loop_sounds_only_while_the_overheated_weapon_is_idle() {
        let wc = aat_cannon();
        let mut s = WeaponState::new(&wc, true, 0);
        s.active = true;
        let unit = UnitHandle { index: 0, gen: 0 };
        let at = |s: &WeaponState| voice(unit, 0, &wc, s, Vec3::ZERO);
        s.heat = 1.0;
        assert!(at(&s).is_none(), "not overheated yet");
        s.overheated = true;
        s.state = FireState::Firing;
        assert!(at(&s).is_none(), "the overheating shot's cycle");
        s.state = FireState::Idle;
        assert!(at(&s).is_some());
        s.active = false;
        assert!(at(&s).is_none(), "holstered");
    }

    #[test]
    fn the_rate_starts_at_the_class_pitch_and_rises_to_one_at_the_threshold() {
        let sound = aat_cannon().overheat_sound.unwrap();
        assert_eq!(sound.rate(true, 0.2, 1.0), 0.5);
        assert!((sound.rate(false, 0.2, 1.0) - 0.6).abs() < 1e-6);
        assert!((sound.rate(false, 0.2, 0.4) - 0.75).abs() < 1e-6);
        assert!((sound.rate(false, 0.2, 0.2) - 1.0).abs() < 1e-6);
    }
}
