//! Hero sample handles: looping weapon hum and the soldier's breathing.

use super::{Cry, Listener, LoopRequest, LoopSource, SoundLib, Start};
use core::fmath::FloatExt;
use core::unit::UnitHandle;
use glam::Vec3;

pub(super) struct BladeVoice {
    unit: UnitHandle,
    on: Option<String>,
    off: Option<String>,
    pos: Vec3,
}

fn blade_changes(previous: &mut Vec<BladeVoice>, now: Vec<BladeVoice>) -> Vec<(String, Vec3)> {
    let mut out = Vec::new();
    for old in previous
        .iter()
        .filter(|old| !now.iter().any(|new| new.unit == old.unit))
    {
        if let Some(name) = &old.off {
            out.push((name.clone(), old.pos));
        }
    }
    for new in now
        .iter()
        .filter(|new| !previous.iter().any(|old| old.unit == new.unit))
    {
        if let Some(name) = &new.on {
            out.push((name.clone(), new.pos));
        }
    }
    *previous = now;
    out
}

impl SoundLib {
    /// PS2 0x3804b0 asks TurnOnSound/TurnOffSound on visibility edges.
    /// Observe on fixed ticks, so catch-up frames retain both transitions.
    pub fn hero_blade_changes(&mut self, walk: &crate::walk::WalkSim) -> Vec<(String, Vec3)> {
        for old in &mut self.hero_blades {
            if let Some(unit) = walk
                .sim
                .units
                .get(old.unit.index as usize)
                .filter(|u| u.gen == old.unit.gen)
            {
                old.pos = unit.soldier.pos;
            }
        }
        let now = walk
            .sim
            .units
            .iter()
            .enumerate()
            .filter_map(|(i, u)| {
                if !crate::event_sounds::sounds_as_hero(&walk.sim, u)
                    || !crate::event_sounds::saber_visible(u, &walk.sim.weapon_classes)
                {
                    return None;
                }
                let audio = &walk.teams[usize::from(u.team.min(1))]
                    .class(u.spec.class)
                    .cries
                    .hero;
                Some(BladeVoice {
                    unit: UnitHandle {
                        index: i as u32,
                        gen: u.gen,
                    },
                    on: audio.saber_on.clone(),
                    off: audio.saber_off.clone(),
                    pos: u.soldier.pos,
                })
            })
            .collect();
        blade_changes(&mut self.hero_blades, now)
    }

    pub fn play_hero_blades(
        &mut self,
        audio: &mut audio::Audio,
        walk: &crate::walk::WalkSim,
        listener: &Listener,
    ) {
        for (name, pos) in self.hero_blade_changes(walk) {
            self.play(audio, &name, pos, listener);
        }
    }
    /// The soldier asks again once its own handle ends. Streamed speech stays
    /// with Commander; a sample never occupies the commander's voice queue.
    pub fn hero_samples(
        &mut self,
        asks: &[(UnitHandle, String, Vec3)],
        ears: Vec3,
    ) -> Vec<(UnitHandle, Start)> {
        self.hero_voices.retain(|(h, hash), _| {
            asks.iter()
                .any(|(a, n, _)| a == h && crate::classdb::fnv1a_lowercase(n) == *hash)
        });
        let mut out = Vec::new();
        for (unit, name, pos) in asks {
            let key = (*unit, crate::classdb::fnv1a_lowercase(name));
            if self.streamed(name)
                || self.find_named(name).is_some_and(|(p, _)| p.looping)
                || self
                    .hero_voices
                    .get(&key)
                    .is_some_and(|v| v.ends > self.now)
            {
                continue;
            }
            let mut starts = Vec::new();
            self.start(
                crate::classdb::fnv1a_lowercase(name),
                *pos,
                ears,
                &mut starts,
            );
            if let Some(first) = starts.first() {
                let pitch = (first.pitch * self.rate_mul).fmax(0.05);
                let secs = self.voice_secs(&first.path).unwrap_or(0.0) / pitch;
                self.hero_voices.insert(
                    key,
                    Cry {
                        ends: self.now + secs,
                        shot: None,
                    },
                );
            }
            out.extend(starts.into_iter().map(|s| (*unit, s)));
        }
        out
    }

    pub fn play_hero_samples(
        &mut self,
        audio: &mut audio::Audio,
        asks: &[(UnitHandle, String, Vec3)],
        listener: &Listener,
    ) {
        let mut live = std::collections::HashSet::new();
        for (unit, name, pos) in asks {
            if !self
                .find_named(name)
                .is_some_and(|(p, _)| p.looping && p.stream.is_none())
            {
                continue;
            }
            let request = LoopRequest {
                source: LoopSource::Hero(*unit, crate::classdb::fnv1a_lowercase(name)),
                sound: crate::classdb::fnv1a_lowercase(name),
                pos: *pos,
                gain: 1.0,
                pitch: 1.0,
            };
            if self.play_loop_voice(audio, &request, listener) {
                live.insert(request.source);
            }
        }
        self.loops
            .retain(|k, _| !matches!(k, LoopSource::Hero(..)) || live.contains(k));
        for (unit, start) in self.hero_samples(asks, listener.pos) {
            let shot = self.mix_shot(audio, &start, listener);
            if let Some(voice) = self.hero_voices.get_mut(&(unit, start.hash)) {
                voice.shot = shot;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_blade_sound_tracks_visibility_edges_and_generation_reuse() {
        let voice = |gen| BladeVoice {
            unit: UnitHandle { index: 3, gen },
            on: Some("on".into()),
            off: Some("off".into()),
            pos: Vec3::ZERO,
        };
        let mut state = Vec::new();
        assert_eq!(
            blade_changes(&mut state, vec![voice(1)]),
            vec![("on".into(), Vec3::ZERO)]
        );
        assert!(blade_changes(&mut state, vec![voice(1)]).is_empty());
        assert_eq!(
            blade_changes(&mut state, vec![voice(2)]),
            vec![("off".into(), Vec3::ZERO), ("on".into(), Vec3::ZERO)]
        );
        assert_eq!(
            blade_changes(&mut state, Vec::new()),
            vec![("off".into(), Vec3::ZERO)]
        );
        assert!(blade_changes(&mut state, Vec::new()).is_empty());
    }
}
