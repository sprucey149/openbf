//! The flight hum of an ordnance in the air (card WHUM; `re-weapon-sounds.md`
//! 0.5, 5.2-5.4): one looping voice per live round whose class authors
//! `OrdnanceSound`, moved with the round and pitched by the PS2's doppler.
//!
//! The PS2 asks the hum on a handle the ordnance owns when the ordnance is
//! made and again on every update until a voice starts (a start at level 0 is
//! refused, and a looping sound is retried); the update moves the handle with
//! the ordnance's position and velocity and the handle is stopped when the
//! ordnance's life runs out or it is destroyed (B1 `FUN_002728a8` 195215,
//! `FUN_00272e98` 195357-195370, B2 `FUN_0030eef8` 301500, `FUN_0030f988`
//! 301815-301827). Every round with the key hums - an infantry bolt's `_sm`
//! as much as a rocket's: the PS2 starts one `com_weap_inf_ord_hum_sm` with
//! every blaster shot at the player (`re-weapon-sounds.md` 5.3,
//! `analysis/audio-census.md`).
//!
//! Render-only: this reads `sim.bolts` and the class table and never writes
//! the sim (the caller's loop voice is `crate::sounds`' handle, card WHUM's
//! note from VSND).
//!
//! PROVISIONAL, named here rather than hidden:
//! - the voice count is capped at [`HUM_VOICES`] and, over it, the rounds
//!   nearest the listener keep their voice. The PS2 keeps 78 one-shot voices
//!   and steals the lowest level x priority (card WSND); that pool is not
//!   ported, so this cap is the port's own;
//! - each start's `PitchDev` (0.15 on the hum properties) is not drawn: the
//!   mixer path ([`crate::sounds::SoundLib`]'s loop voices) takes the
//!   property's pitch and the doppler alone;
//! - the doppler's inputs are the last tick's round and the listener's own
//!   velocity (`Soldier::vel`), as the PS2's `d1` reads the listener's
//!   velocity too. The doc marks the term's units and a missing clamp near 0
//!   [H] (`re-weapon-sounds.md` 12); the mixer's 0.1 rate floor is all the
//!   clamp there is.

use crate::sounds::Listener;
use core::projectile::Bolt;
use glam::Vec3;

/// The speed of sound the PS2's doppler divides by: 344.0 (B1
/// `FUN_0044cf80` 510467-510520, the ELF's `lui 0x43ac`).
pub const SOUND_SPEED: f32 = 344.0;

/// The closing speed at or over which the PS2 leaves the pitch alone: 172.0
/// (the same function, the ELF's `lui 0x432c`). [H] its units, as the doc
/// says (`re-weapon-sounds.md` 5.4, 12).
pub const DOPPLER_LIMIT: f32 = 172.0;

/// How many rounds' hums may sound at once (PROVISIONAL, see the module
/// doc). The PS2's 78-voice pool and its level x priority steal are card
/// WSND's; this is a smaller bound so a firefight cannot fill the mix.
pub const HUM_VOICES: usize = 24;

/// The doppler factor of a round at `pos` flying at `vel` m/s, heard from
/// `ears` moving at `ears_vel` (B1 `FUN_0044cf80` 510467-510520): with
/// d0 = |pos - ears| now and d1 = |(pos - vel) - (ears - ears_vel)| - the
/// distance one second back - the pitch multiplies by `1 + (d1 - d0) / 344`
/// while d1 - d0 is under 172, and by 1.0 at or over it. Closing raises the
/// pitch and receding lowers it: 100 m/s closing is 1.2907, 100 m/s
/// receding 0.7093, 200 m/s closing 1.0 (card WHUM's acceptance).
pub fn doppler(pos: Vec3, vel: Vec3, ears: Vec3, ears_vel: Vec3) -> f32 {
    let d0 = (pos - ears).length();
    let d1 = ((pos - vel) - (ears - ears_vel)).length();
    // d1 - d0 is how much more the round was closing a second ago.
    let closing = d1 - d0;
    if closing >= DOPPLER_LIMIT {
        1.0
    } else {
        1.0 + closing / SOUND_SPEED
    }
}

/// One round's hum for this tick, as the loop voice is handed it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hum {
    /// The round's identity across frames ([`crate::bolt_vis::key`]): the
    /// voice's key, so the same round keeps its voice as it steers and
    /// another round never inherits it.
    pub key: crate::bolt_vis::Key,
    /// The class's `OrdnanceSound`, lower-case FNV-1a (the bank's key).
    pub sound: u32,
    /// Where the round is now: the voice follows it.
    pub pos: Vec3,
    /// `doppler`'s factor: the property's own pitch and the range level are
    /// the mixer's.
    pub pitch: f32,
}

/// This tick's hums, nearest the listener first: one per round in
/// `sim.bolts` whose class authors `OrdnanceSound`, at most
/// [`HUM_VOICES`]. The count is the slice of `out` that is set; the array
/// is the hot loop's (no allocation per tick).
pub fn hums(sim: &core::sim::Sim, listener: &Listener) -> ([Option<Hum>; HUM_VOICES], usize) {
    let ears = listener.pos;
    // The listener is the local player's ears: its own velocity joins the
    // doppler's `d1` (the PS2 reads the listener's too).
    let ears_vel = sim
        .units
        .iter()
        .find(|u| u.is_player)
        .map_or(Vec3::ZERO, |u| u.soldier.vel);
    let mut best: [(f32, Option<Hum>); HUM_VOICES] = [(f32::INFINITY, None); HUM_VOICES];
    let mut n = 0;
    for b in &sim.bolts {
        let Some(h) = hum_of(sim, b, ears, ears_vel) else {
            continue;
        };
        let d = (h.pos - ears).length_squared();
        // Keep the nearest HUM_VOICES: find where this one belongs (a tie
        // keeps the lower key, so the choice does not depend on the bolt
        // list's order).
        let mut at = n.min(HUM_VOICES);
        while at > 0 {
            let (pd, ph) = best[at - 1];
            let Some(ph) = ph else {
                at -= 1;
                continue;
            };
            if d < pd || (d == pd && h.key < ph.key) {
                at -= 1;
            } else {
                break;
            }
        }
        if at == HUM_VOICES {
            continue;
        }
        let last = n.min(HUM_VOICES - 1);
        for i in (at..last).rev() {
            best[i + 1] = best[i];
        }
        best[at] = (d, Some(h));
        n = (n + 1).min(HUM_VOICES);
    }
    (best.map(|(_, h)| h), n)
}

/// The round's hum, or `None` when its class authors no `OrdnanceSound`.
fn hum_of(sim: &core::sim::Sim, b: &Bolt, ears: Vec3, ears_vel: Vec3) -> Option<Hum> {
    let name = sim
        .weapon_classes
        .get(b.weapon_class)?
        .ordnance
        .ordnance_sound
        .as_deref()?;
    Some(Hum {
        key: crate::bolt_vis::key(b),
        sound: crate::classdb::fnv1a_lowercase(name),
        pos: b.pos,
        pitch: doppler(b.pos, b.vel, ears, ears_vel),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::collision::HullFilter;
    use core::projectile::Hit;
    use core::sim::Sim;
    use core::unit::{UnitHandle, UnitSpec};
    use core::weapon::WeaponClass;

    /// A sim with one added weapon class (`sound` its `OrdnanceSound`) and
    /// the round of it at 20 m flying 100 m/s away from the listener.
    fn sim_with(sound: Option<&str>) -> Sim {
        let mut sim = Sim::empty(42);
        let mut w = WeaponClass::bf1_rifle();
        w.ordnance.ordnance_sound = sound.map(str::to_owned);
        w.ordnance.collision_sound = None;
        w.ordnance.impact = None;
        let class = sim.add_weapon_class(w);
        assert!(sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).is_some());
        sim.bolts.push(Bolt {
            tracked_origin: false,
            visual_shot: None,
            pos: Vec3::new(0.0, 0.0, -20.0),
            vel: Vec3::Z * 100.0,
            remaining: 2.0,
            age: 0.0,
            owner: UnitHandle { index: 0, gen: 0 },
            team: 0,
            weapon_class: class,
            hit: Hit::None,
            end: Vec3::new(0.0, 0.0, -180.0),
            shooter_is_player: true,
            hulls: HullFilter::All,
            hit_mult: 1.0,
            gravity: 0.0,
            charge: 1.0,
            missile: None,
            fat: None,
        });
        sim
    }

    fn list(sim: &Sim) -> Vec<Hum> {
        let (out, n) = hums(
            sim,
            &Listener {
                pos: Vec3::ZERO,
                yaw: 0.0,
            },
        );
        out[..n].iter().flatten().copied().collect()
    }

    /// Card WHUM: a round whose class authors `OrdnanceSound` has exactly
    /// one hum, keyed on the round; the same round keeps that one voice as
    /// it flies (its place is followed each frame, not restarted); a round
    /// gone from `sim.bolts` has none - the loop voice's stop.
    #[test]
    fn a_round_hums_once_and_its_voice_follows_it_until_it_is_gone() {
        let mut sim = sim_with(Some("com_weap_inf_ord_hum_sm"));
        let first = list(&sim);
        assert_eq!(first.len(), 1, "one voice for one round: {first:?}");
        assert_eq!(
            first[0].sound,
            crate::classdb::fnv1a_lowercase("com_weap_inf_ord_hum_sm")
        );
        assert_eq!(first[0].pos, sim.bolts[0].pos);
        // Closing on the listener at 100 m/s from 20 m: 1 + 100 / 344.
        assert!(
            (first[0].pitch - 1.290_7).abs() < 1e-3,
            "{}",
            first[0].pitch
        );
        // Three frames of flight: the same key, at the round's new place,
        // with the pitch of the new closing geometry - one voice moved.
        for frame in 1..=3 {
            let b = &mut sim.bolts[0];
            b.pos += b.vel * core::TICK_DT;
            b.age += core::TICK_DT;
            b.remaining -= core::TICK_DT;
            let now = list(&sim);
            assert_eq!(now.len(), 1, "frame {frame}: {now:?}");
            assert_eq!(now[0].key, first[0].key, "frame {frame}: one voice, moved");
            assert_eq!(now[0].sound, first[0].sound, "frame {frame}");
            assert_eq!(now[0].pos, sim.bolts[0].pos, "frame {frame}");
        }
        // The round lands: the voice stops.
        sim.bolts.clear();
        assert!(list(&sim).is_empty(), "a gone round keeps no voice");
    }

    /// Card WHUM: a class without the key is silent, and the count is
    /// bounded - over `HUM_VOICES` rounds only the nearest keep a voice.
    #[test]
    fn only_rounds_with_the_key_hum_and_the_count_is_bounded() {
        let sim = sim_with(None);
        assert!(list(&sim).is_empty(), "no OrdnanceSound, no voice");

        let mut sim = sim_with(Some("com_weap_inf_ord_hum_lg"));
        let class = sim.bolts[0].weapon_class;
        let round = sim.bolts[0].clone();
        for i in 0..HUM_VOICES + 8 {
            sim.bolts.push(Bolt {
                pos: Vec3::new(0.0, 0.0, -1.0 - i as f32),
                vel: Vec3::ZERO,
                end: Vec3::new(0.0, 0.0, -1.0 - i as f32),
                remaining: 2.0,
                weapon_class: class,
                ..round
            });
        }
        let got = list(&sim);
        assert_eq!(got.len(), HUM_VOICES, "the cap holds");
        // Nearest first: the round at 1 m, and never the farthest ones.
        assert_eq!(got[0].pos, Vec3::new(0.0, 0.0, -1.0));
        assert!(
            got.iter().all(|h| h.pos.z > -(HUM_VOICES as f32) - 1.0),
            "the nearest rounds keep their voice: {got:?}"
        );
    }

    /// Card WHUM's acceptance: the PS2's doppler values.
    #[test]
    fn the_doppler_factor_is_the_ps2s() {
        let ears = Vec3::ZERO;
        // 1 km out, closing / receding at the given m/s: no crossing, so
        // the distance a second back differs by exactly the speed.
        let at = |closing: f32| doppler(-Vec3::Z * 1000.0, Vec3::Z * closing, ears, Vec3::ZERO);
        assert!(
            (at(100.0) - 1.290_7).abs() < 1e-3,
            "closing 100: {}",
            at(100.0)
        );
        assert!(
            (at(-100.0) - 0.709_3).abs() < 1e-3,
            "receding 100: {}",
            at(-100.0)
        );
        assert_eq!(at(200.0), 1.0, "closing 200 leaves the pitch alone");
        assert_eq!(at(-200.0), 1.0 - 200.0 / SOUND_SPEED);
        // A moving listener closes too: the same 100 m/s from its own motion.
        let heard = doppler(-Vec3::Z * 1000.0, Vec3::ZERO, ears, -Vec3::Z * 100.0);
        assert!((heard - 1.290_7).abs() < 1e-3, "listener closing: {heard}");
    }
}
