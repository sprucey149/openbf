//! Sound playback: prepared voices, one-shots, cries, station starts and delayed layers.
//! Shared state, lookup, loop voices, mixing and rules remain in the parent sounds module.
use super::{
    attenuation, decode_noted, ears_3d, mixer_pair, side, Cry, Handed, Listener, LoopRequest,
    LoopSource, Refused, SoundLib, Start, Timer, WeaponAsk, CHATTER_KEY_MAX, LAYER_TIMERS,
};
use crate::classdb::fnv1a_lowercase;
use anyhow::{Context, Result};
use core::announcer::Line;
use glam::Vec3;
use std::path::{Path, PathBuf};

impl SoundLib {
    pub(super) fn play_loop_voice(
        &mut self,
        audio: &mut audio::Audio,
        request: &LoopRequest,
        listener: &Listener,
    ) -> bool {
        let Some((ears, rate)) = self.loop_level(
            request.sound,
            request.pos,
            request.gain,
            request.pitch,
            listener,
        ) else {
            return false;
        };
        let (db, pan) = mixer_pair(ears);
        let rate = f64::from((rate * self.rate_mul).max(0.05));
        if let Some(voice) = self.loops.get_mut(&request.source) {
            if let Some(voice) = voice {
                voice.set(db, rate, pan, core::TICK_DT);
            }
            return true;
        }
        let path = self.find(request.sound).and_then(|(p, core)| {
            let id = self.pick_sample(&p.samples)?;
            self.bank(core)?.sample_path(id)
        });
        let voice = path
            .filter(|p| self.sound(audio, p).is_ok())
            .and_then(|p| audio.play_voice(&self.cache[&p], db, rate, pan));
        self.played += u64::from(voice.is_some());
        self.loops.insert(request.source, voice);
        true
    }

    pub fn play_repair_loops(
        &mut self,
        audio: &mut audio::Audio,
        sim: &core::sim::Sim,
        listener: &Listener,
    ) {
        let requests =
            crate::repair_feedback::loops(sim)
                .into_iter()
                .map(|(unit, class, name, pos)| LoopRequest {
                    source: LoopSource::Repair(unit, class),
                    sound: fnv1a_lowercase(name),
                    pos,
                    gain: 1.0,
                    pitch: 1.0,
                });
        self.play_loop_set(audio, listener, requests, |k| {
            matches!(k, LoopSource::Repair(..))
        });
    }

    pub fn play_shield_loops(
        &mut self,
        audio: &mut audio::Audio,
        sim: &core::sim::Sim,
        listener: &Listener,
    ) {
        let requests =
            crate::droideka::shield_loops(sim)
                .into_iter()
                .map(|(unit, name, gain, pos)| LoopRequest {
                    source: LoopSource::Shield(unit),
                    sound: fnv1a_lowercase(name),
                    pos,
                    gain,
                    pitch: 1.0,
                });
        self.play_loop_set(audio, listener, requests, |k| {
            matches!(k, LoopSource::Shield(..))
        });
    }

    /// The auras' looping sounds (`crate::fx::Fx::buff_loops`): each starts
    /// with its aura and stops with it. The property holds its own gain and
    /// distances (`buff_rage_effect`: 0.5, 15 to 30 m, looping).
    pub fn play_buff_loops(
        &mut self,
        audio: &mut audio::Audio,
        loops: &[(core::unit::UnitHandle, usize, String, Vec3)],
        listener: &Listener,
    ) {
        let requests = loops.iter().map(|(unit, kind, name, pos)| LoopRequest {
            source: LoopSource::Buff(*unit, *kind),
            sound: fnv1a_lowercase(name),
            pos: *pos,
            gain: 1.0,
            pitch: 1.0,
        });
        self.play_loop_set(audio, listener, requests, |k| {
            matches!(k, LoopSource::Buff(..))
        });
    }

    fn play_loop_set(
        &mut self,
        audio: &mut audio::Audio,
        listener: &Listener,
        requests: impl IntoIterator<Item = LoopRequest>,
        owns: impl Fn(&LoopSource) -> bool,
    ) {
        let mut live = std::collections::HashSet::new();
        for request in requests {
            if self.play_loop_voice(audio, &request, listener) {
                live.insert(request.source);
            }
        }
        self.loops.retain(|k, _| !owns(k) || live.contains(k));
    }

    pub(super) fn sound(&mut self, audio: &mut audio::Audio, path: &Path) -> Result<()> {
        if self.cache.contains_key(path) {
            return Ok(());
        }
        let _ = audio;
        let data = decode_noted(path).with_context(|| format!("decode {}", path.display()))?;
        self.cache.insert(path.to_path_buf(), data);
        Ok(())
    }

    /// The take a property would play now — the same weighted pick
    /// [`SoundLib::play`] makes, a bank sample or a streamed segment — with
    /// its gain and its length in seconds at the property's pitch. Needs no
    /// mixer (a headless box can time a line); the length is 0.0 without the
    /// audio backend. None when the property is unknown or names no clip
    /// this bake holds.
    pub fn voice_take(&mut self, name: &str) -> Option<(PathBuf, f32, f32)> {
        let (p, core) = self.find_named(name)?;
        let path = self.file_take(&p, core)?;
        let secs = self.voice_secs(&path)? / p.pitch.max(0.05);
        Some((path, p.gain, secs))
    }

    /// Play a 2-D line (a commander voice-over: no position, no attenuation,
    /// centred — analysis/re-announcer.md §1.6: Mode3D 0) and return its
    /// length in seconds; None when there is no take to play. The `slow`
    /// cheat's pitch is for positional sounds and does not apply.
    pub fn play_voice(&mut self, audio: &mut audio::Audio, name: &str) -> Option<f32> {
        let take = self.prepare_voice(name)?;
        self.play_prepared(audio, &take);
        Some(take.secs)
    }

    // Resolve once, draw once, and decode without requiring an output device.
    pub(super) fn prepare_voice(&mut self, name: &str) -> Option<Handed> {
        let (p, core) = self.find_named(name)?;
        let path = self.file_take(&p, core)?;
        let pitch = p.pitch.max(0.05);
        let secs = self.voice_secs(&path)? / pitch;
        let a = p.gain * self.bus_of(&p);
        Some(Handed {
            hash: fnv1a_lowercase(name),
            path,
            gain: p.gain,
            ears: (a, a),
            pitch,
            dist: 0.0,
            secs,
        })
    }

    /// Play take `nth` of a 3-D line at `pos` (a squad answer: the take was
    /// drawn when the line was asked for, card SQACK) and return its length
    /// at the positional rate, including the active slow-sound multiplier;
    /// None when the property has no such take in this bake.
    pub fn play_take(
        &mut self,
        audio: &mut audio::Audio,
        name: &str,
        nth: usize,
        pos: Vec3,
        listener: &Listener,
    ) -> Option<f32> {
        let take = self.prepare_take(name, nth, pos, listener)?;
        self.play_prepared(audio, &take);
        Some(take.secs)
    }

    pub(super) fn prepare_take(
        &mut self,
        name: &str,
        nth: usize,
        pos: Vec3,
        listener: &Listener,
    ) -> Option<Handed> {
        let hash = line_hash(name);
        let (p, _) = self.find(hash)?;
        let path = self.take_path_by_hash(hash, nth)?;
        let pitch = (p.pitch * self.rate_mul).max(0.05);
        let secs = self.voice_secs(&path)? / pitch;
        let dist = (pos - listener.pos).length();
        let gain = p.gain * attenuation(dist, &p);
        let a = gain * self.bus_of(&p);
        let ears = if p.flat {
            (a, a)
        } else {
            ears_3d(a, side(listener, pos))
        };
        Some(Handed {
            hash,
            path,
            gain,
            ears,
            pitch,
            dist,
            secs,
        })
    }

    fn play_prepared(&mut self, audio: &mut audio::Audio, take: &Handed) {
        if take.gain > 0.001 {
            let (db, pan) = mixer_pair(take.ears);
            audio.play_oneshot(&self.cache[&take.path], db, f64::from(take.pitch), pan);
            self.note(
                take.hash, &take.path, take.gain, take.ears, take.pitch, take.dist,
            );
            self.played += 1;
        }
    }

    /// A unit's line asked for on the battle's voice (card UCALL: BF1's
    /// combat calls), gated as BF1's stream path gates it (B1 `FUN_0045ec68`
    /// 522512-522522, re-unit-chatter.md 1): ONE take is drawn by weight
    /// and a take inside its own lock silences the request; then the
    /// PROPERTY's gate - refused inside its interval, else the interval is
    /// re-armed, and only then the probability rolled ([`SoundLib::start`]'s
    /// gate: one clock per property, shared with its one-shot starts); then
    /// the level at `ears`, and a line of level 0 is not queued. The line's
    /// key is that level x its bus (a chatter template's Bias 0,
    /// re-announcer.md 1.5), kept under a commander line's. Returns the
    /// line for the voice queue, or why not.
    pub fn ask_line(&mut self, name: &str, pos: Vec3, ears: Vec3) -> Result<Line, Refused> {
        self.ask_line_with(name, pos, ears, false)
    }

    /// [`SoundLib::ask_line`]; `step` = BF2's take rule (card UVO2, B2
    /// `FUN_0055bf60` 693184-693246): a drawn take inside its lock gives way
    /// to the next one in the list, round to the first, and only when every
    /// take is locked is the request silent.
    pub fn ask_line_with(
        &mut self,
        name: &str,
        pos: Vec3,
        ears: Vec3,
        step: bool,
    ) -> Result<Line, Refused> {
        let hash = line_hash(name);
        let Some((p, _)) = self.find(hash) else {
            return Err(Refused::Unknown);
        };
        let list = if p.samples.is_empty() {
            &p.segments
        } else {
            &p.samples
        };
        let Some(drawn) = self.pick_index(list) else {
            return Err(Refused::Unknown);
        };
        // The take's own gate (noProb: its weight is not a chance).
        let locked = |t: usize| {
            self.take_ok
                .get(&(hash, t))
                .is_some_and(|until| self.now < *until)
        };
        let n = if step { list.len() } else { 1 };
        let Some(take) = (0..n)
            .map(|i| (drawn + i) % list.len())
            .find(|&t| !locked(t))
        else {
            return Err(Refused::TakeLocked);
        };
        if let Some(&(interval, dev)) = p.take_locks.get(take) {
            if (interval, dev) != (0.0, 0.0) {
                let until = self.now + interval + self.spread(dev);
                self.take_ok.insert((hash, take), until);
            }
        }
        if self.next_ok.get(&hash).is_some_and(|t| self.now < *t) {
            return Err(Refused::Interval);
        }
        if let Some((interval, dev)) = p.play_interval {
            let next = self.now + interval + self.spread(dev);
            self.next_ok.insert(hash, next);
        }
        if p.play_probability.is_some_and(|pr| pr < self.next_rand()) {
            return Err(Refused::Probability);
        }
        let level = attenuation((pos - ears).length(), &p);
        if level <= 0.0 {
            return Err(Refused::Far);
        }
        Ok(Line {
            name: name.to_owned(),
            key: (level * self.bus_of(&p)).min(CHATTER_KEY_MAX),
            mode_3d: true,
            take: u8::try_from(take).ok(),
            at: Some(pos),
            speaker: None,
        })
    }

    /// How long a queued line plays, in seconds, without a mixer (a
    /// headless log's voice queue): its drawn take's, else the weighted
    /// pick's ([`SoundLib::voice_take`]). None = no take in this bake.
    pub fn line_secs(&mut self, line: &Line) -> Option<f32> {
        let Some(nth) = line.take else {
            return self.voice_take(&line.name).map(|t| t.2);
        };
        let hash = line_hash(&line.name);
        let pitch = self.find(hash)?.0.pitch.max(0.05);
        let path = self.take_path_by_hash(hash, usize::from(nth))?;
        Some(self.voice_secs(&path)? / pitch)
    }

    pub(super) fn voice_secs(&mut self, path: &Path) -> Option<f32> {
        if !self.cache.contains_key(path) {
            let data = decode_noted(path)
                .map_err(|e| eprintln!("decode {}: {e:#}", path.display()))
                .ok()?;
            self.cache.insert(path.to_path_buf(), data);
        }
        Some(self.cache[path].duration_secs())
    }

    /// The index of a weighted draw over `list` ([`SoundLib::pick_sample`]'s
    /// draw).
    fn pick_index(&mut self, list: &[(u32, f32)]) -> Option<usize> {
        let id = self.pick_sample(list)?;
        list.iter().position(|(i, _)| *i == id)
    }

    /// Play a property by name at `pos` for `listener`. Returns true if a
    /// take was played.
    pub fn play(
        &mut self,
        audio: &mut audio::Audio,
        name: &str,
        pos: Vec3,
        listener: &Listener,
    ) -> bool {
        self.play_by_hash(audio, fnv1a_lowercase(name), pos, listener)
    }

    /// A 2-D one-shot by name (a menu click): its pitch, gain, bus and
    /// `PlayInterval` as any start; played when there is a mixer. Returns
    /// what started.
    pub fn play_2d(&mut self, audio: Option<&mut audio::Audio>, name: &str) -> Vec<Start> {
        let mut starts = Vec::new();
        self.start(fnv1a_lowercase(name), Vec3::ZERO, Vec3::ZERO, &mut starts);
        if let Some(audio) = audio {
            let ear = Listener {
                pos: Vec3::ZERO,
                yaw: 0.0,
            };
            self.mix(audio, &starts, &ear);
        }
        starts
    }

    /// [`SoundLib::play`] for a property known by its hash (the foley
    /// table's slots, `crate::foley`).
    pub fn play_by_hash(
        &mut self,
        audio: &mut audio::Audio,
        hash: u32,
        pos: Vec3,
        listener: &Listener,
    ) -> bool {
        let mut starts = Vec::new();
        self.start(hash, pos, listener.pos, &mut starts);
        self.mix(audio, &starts, listener)
    }

    /// One sound update: start the waiting layers whose delay has passed,
    /// heard by `listener` now. Once per stepped tick, after its events.
    pub fn update(&mut self, audio: &mut audio::Audio, dt: f32, listener: &Listener) {
        let mut starts = Vec::new();
        self.tick(dt, listener.pos, &mut starts);
        self.mix(audio, &starts, listener);
    }

    /// A unit's hurt or death cry (card UCRY, `analysis/re-unit-chatter.md`
    /// 2.7, 3.6): ONE voice per unit (B1 entity +0xec, B2 +0x25c) - a cry
    /// still sounding on it is stopped first, whatever this one's draw says
    /// (B1 `FUN_002219d0` / `FUN_00221930` stop, then ask), then `hash`
    /// (the class's `HurtSound` / `DeathSound`; 0 for a class without one)
    /// is asked at `pos` like any one-shot: its `PlayProbability`, its range.
    /// What it starts goes to `out` and holds the unit's voice for the take's
    /// length. Returns whether a sounding cry was cut.
    pub fn cry(
        &mut self,
        unit: u32,
        hash: u32,
        pos: Vec3,
        ears: Vec3,
        out: &mut Vec<Start>,
    ) -> bool {
        let cut = self.cries.remove(&unit).is_some_and(|c| c.ends > self.now);
        if hash == 0 {
            return cut;
        }
        let from = out.len();
        self.start(hash, pos, ears, out);
        // A cry is one sample-bank property with no layers: one start at most.
        if let Some(s) = out.get(from) {
            let (path, pitch) = (s.path.clone(), s.pitch.max(0.05));
            let secs = self.decoded(&path).map_or(0.0, |d| d.duration_secs());
            let ends = self.now + secs / pitch;
            self.cries.insert(unit, Cry { ends, shot: None });
        }
        cut
    }

    /// Whether unit `unit`'s voice holds a cry now (card UCRY).
    pub fn crying(&self, unit: u32) -> bool {
        self.cries.get(&unit).is_some_and(|c| c.ends > self.now)
    }

    /// A tick's cries (card UCRY): each ask `(unit, hash, pos)` in event
    /// order through [`SoundLib::cry`] (its cut, its draw), and per unit
    /// only the LAST ask's start, if its draw let it start: a cry cut in the
    /// tick it began is never heard (a shotgun's eight pellets on one unit
    /// are one cry, not eight starts summed in the mixer).
    pub fn cries(&mut self, asks: &[(u32, u32, Vec3)], ears: Vec3) -> Vec<(u32, Start)> {
        let mut last: Vec<(u32, Option<Start>)> = Vec::new();
        for &(unit, hash, pos) in asks {
            let mut out = Vec::new();
            self.cry(unit, hash, pos, ears, &mut out);
            let start = out.pop();
            match last.iter_mut().find(|l| l.0 == unit) {
                Some(l) => l.1 = start,
                None => last.push((unit, start)),
            }
        }
        last.into_iter()
            .filter_map(|(u, s)| Some((u, s?)))
            .collect()
    }

    /// [`SoundLib::cries`] through the mixer: a cut stops the sounding take.
    pub fn play_cries(
        &mut self,
        audio: &mut audio::Audio,
        asks: &[(u32, u32, Vec3)],
        listener: &Listener,
    ) {
        for (unit, s) in self.cries(asks, listener.pos) {
            let shot = self.mix_shot(audio, &s, listener);
            if let Some(c) = self.cries.get_mut(&unit) {
                c.shot = shot;
            }
        }
    }

    pub(super) fn mix_shot(
        &mut self,
        audio: &mut audio::Audio,
        s: &Start,
        listener: &Listener,
    ) -> Option<audio::Shot> {
        self.sound(audio, &s.path).ok()?;
        let pitch = (s.pitch * self.rate_mul).max(0.05);
        let ears = s.ears(listener);
        let (db, pan) = mixer_pair(ears);
        let shot = audio.play_shot(&self.cache[&s.path], db, f64::from(pitch), pan);
        self.note(
            s.hash,
            &s.path,
            s.gain,
            ears,
            pitch,
            (s.pos - listener.pos).length(),
        );
        self.played += 1;
        shot
    }

    pub fn boost(
        &mut self,
        vehicle: u32,
        name: &str,
        rising: bool,
        pos: Vec3,
        ears: Vec3,
    ) -> Option<Start> {
        let sounding = self.boosts.get(&vehicle).is_some_and(|c| c.ends > self.now);
        if sounding && !rising {
            return None;
        }
        self.boosts.remove(&vehicle);
        let mut out = Vec::new();
        self.start(fnv1a_lowercase(name), pos, ears, &mut out);
        let s = out.pop()?;
        let (path, pitch) = (s.path.clone(), s.pitch.max(0.05));
        let secs = self.decoded(&path).map_or(0.0, |d| d.duration_secs());
        let ends = self.now + secs / pitch;
        self.boosts.insert(vehicle, Cry { ends, shot: None });
        Some(s)
    }

    pub fn play_boost(
        &mut self,
        audio: &mut audio::Audio,
        vehicle: u32,
        name: &str,
        rising: bool,
        pos: Vec3,
        listener: &Listener,
    ) {
        if let Some(s) = self.boost(vehicle, name, rising, pos, listener.pos) {
            let shot = self.mix_shot(audio, &s, listener);
            if let Some(c) = self.boosts.get_mut(&vehicle) {
                c.shot = shot;
            }
        }
    }

    /// A tick's starts on a human's weapon handles (B1 +0xf4, B2 +0xd8), each
    /// with the handle that holds it, if one does. A click stops everything
    /// still sounding on its handle first, the last shot's tail and the click
    /// before it, and only when it starts (`FUN_00355768` asks its gate
    /// first). A `soundlayered` property holds no handle (`FUN_00355768` kind
    /// 2, `FUN_0045ca08`), so it starts and nothing can cut it. A shot does
    /// not cut the shots before it, though the PS2's does: that would change
    /// how every gun sounds.
    /// PROVISIONAL (ours), measure: the PS2's and the port's player fire
    /// sounds compared by ear with one voice per handle.
    pub fn weapon_voices(
        &mut self,
        asks: &[WeaponAsk],
        ears: Vec3,
    ) -> Vec<(Option<(u32, u32)>, Start)> {
        let now = self.now;
        self.weapon_handles.retain(|_, v| {
            v.retain(|c| c.ends > now);
            !v.is_empty()
        });
        let mut out: Vec<(Option<(u32, u32)>, Start)> = Vec::new();
        for a in asks {
            let key = (a.unit, a.weapon_class);
            let held = self.find(a.hash).is_some_and(|(p, _)| p.layers.is_empty());
            let mut starts = Vec::new();
            self.start(a.hash, a.pos, ears, &mut starts);
            if a.click && !starts.is_empty() {
                self.weapon_handles.remove(&key);
                // This tick's earlier starts on the handle are never heard.
                out.retain(|(k, _)| *k != Some(key));
            }
            for s in starts {
                if held {
                    let pitch = (s.pitch * self.rate_mul).max(0.05);
                    let secs = self.decoded(&s.path).map_or(0.0, |d| d.duration_secs());
                    self.weapon_handles.entry(key).or_default().push(Cry {
                        ends: now + secs / pitch,
                        shot: None,
                    });
                }
                out.push((held.then_some(key), s));
            }
        }
        out
    }

    /// How many voices a human's weapon handle sounds now.
    pub fn weapon_handle_voices(&self, unit: u32, weapon_class: u32) -> usize {
        self.weapon_handles
            .get(&(unit, weapon_class))
            .map_or(0, |v| v.iter().filter(|c| c.ends > self.now).count())
    }

    /// [`SoundLib::weapon_voices`] through the mixer: a click's cut stops the
    /// sounding takes.
    pub fn play_weapon_voices(
        &mut self,
        audio: &mut audio::Audio,
        asks: &[WeaponAsk],
        listener: &Listener,
    ) {
        for (key, s) in self.weapon_voices(asks, listener.pos) {
            let shot = self.mix_shot(audio, &s, listener);
            let voice = key
                .and_then(|k| self.weapon_handles.get_mut(&k))
                .and_then(|v| v.iter_mut().find(|c| c.shot.is_none()));
            if let Some(c) = voice {
                c.shot = shot;
            }
        }
    }

    /// A tick's station ambient asks (card PWST, re-ambience-music §5.2):
    /// each `(station, hash, pos)` whose handle is not sounding asks its
    /// `AmbientSound` at the station like any one-shot (its
    /// `PlayProbability`, its range: B1 `FUN_002a9750` 235630, B2
    /// `FUN_003674c0` 362446 ask every update while the handle is inactive);
    /// what starts holds the handle for the take's length.
    pub fn station_ambient(&mut self, asks: &[(u16, u32, Vec3)], ears: Vec3) -> Vec<Start> {
        let mut out = Vec::new();
        for &(station, hash, pos) in asks {
            if self
                .station_voices
                .get(&station)
                .is_some_and(|v| v.ends > self.now)
            {
                continue;
            }
            let from = out.len();
            self.start(hash, pos, ears, &mut out);
            if let Some(s) = out.get(from) {
                let (path, pitch) = (s.path.clone(), (s.pitch * self.rate_mul).max(0.05));
                let secs = self.decoded(&path).map_or(0.0, |d| d.duration_secs());
                self.station_voices.insert(
                    station,
                    Cry {
                        ends: self.now + secs / pitch,
                        shot: None,
                    },
                );
            }
        }
        out
    }

    /// [`SoundLib::station_ambient`] through the mixer.
    pub fn play_station_ambient(
        &mut self,
        audio: &mut audio::Audio,
        asks: &[(u16, u32, Vec3)],
        listener: &Listener,
    ) {
        self.station_voices
            .retain(|id, _| asks.iter().any(|(i, _, _)| i == id));
        for ask in asks {
            for s in self.station_ambient(std::slice::from_ref(ask), listener.pos) {
                let shot = self.mix_shot(audio, &s, listener);
                if let Some(v) = self.station_voices.get_mut(&ask.0) {
                    v.shot = shot;
                }
            }
        }
    }

    /// A take decoded without the mixer, for its length ([`decode_noted`]).
    fn decoded(&mut self, path: &Path) -> Option<&audio::SoundData> {
        if !self.cache.contains_key(path) {
            let data = decode_noted(path).ok()?;
            self.cache.insert(path.to_path_buf(), data);
        }
        self.cache.get(path)
    }

    /// Engine layers looping now.
    pub fn loops_playing(&self) -> usize {
        self.loops
            .iter()
            .filter(|(k, v)| {
                matches!(k, LoopSource::Engine(..) | LoopSource::Jet(..)) && v.is_some()
            })
            .count()
    }

    /// U(-1, 1) x `dev` (the PS2's `2 x (r - 0.5) x dev`, B1 `FUN_0044e288`).
    fn spread(&mut self, dev: f32) -> f32 {
        2.0 * (self.next_rand() - 0.5) * dev
    }

    /// What starting property `hash` at `pos` starts now, heard at `ears`
    /// (the mixer is not needed: a headless referee reads `out`). A layered
    /// record starts each layer of delay exactly 0 at once and arms a timer
    /// for any other, at delay + deviation x U(-1, 1), which [`SoundLib::tick`]
    /// fires at `pos` (B1 `FUN_0045ca08`, B2 `FUN_0054fa38`).
    pub fn start(&mut self, hash: u32, pos: Vec3, ears: Vec3, out: &mut Vec<Start>) {
        let Some((p, _)) = self.find(hash) else {
            return;
        };
        for (i, l) in p.layers.iter().enumerate() {
            let (delay, dev) = p.layer_delays.get(i).copied().unwrap_or((0.0, 0.0));
            if delay == 0.0 {
                self.start_prop(*l, pos, ears, out);
                continue;
            }
            let fire = delay + self.spread(dev);
            // PROVISIONAL: the PS2 ranks a waiting layer by its level x a
            // priority term (B1 `FUN_0045bec0`); this is level x priority.
            let value = self.find(*l).map_or(0.0, |(lp, _)| {
                attenuation((pos - ears).length(), &lp) * lp.priority
            });
            let timer = Timer {
                sound: *l,
                pos,
                fire,
                elapsed: 0.0,
                value,
            };
            if self.timers.len() < LAYER_TIMERS {
                self.timers.push(timer);
                continue;
            }
            // Full: the waiting layer of the lowest value below this one's
            // gives way, or this one is dropped (B1 `FUN_0045c060`).
            let low = self
                .timers
                .iter_mut()
                .filter(|t| t.value < value)
                .min_by(|a, b| a.value.total_cmp(&b.value));
            if let Some(t) = low {
                *t = timer;
            }
        }
        self.start_prop(hash, pos, ears, out);
    }

    /// One property's own take (a layer's sound is a `soundproperties`: the
    /// PS2's parser keeps no other, so a layer never nests).
    fn start_prop(&mut self, hash: u32, pos: Vec3, ears: Vec3, out: &mut Vec<Start>) {
        self.start_prop_with(hash, pos, ears, None, out);
    }

    /// [`SoundLib::start_prop`] with a world emitter's distances in place of
    /// the property's (card AMBS).
    pub(super) fn start_prop_with(
        &mut self,
        hash: u32,
        pos: Vec3,
        ears: Vec3,
        emitter: Option<&crate::ambience::Emitter>,
        out: &mut Vec<Start>,
    ) {
        let Some((p, core)) = self.find(hash) else {
            return;
        };
        let p = match emitter {
            Some(e) => crate::ambience::with_distances(e, &p),
            None => p,
        };
        if p.samples.is_empty() && p.segments.is_empty() {
            return;
        }
        // The start's gates, in the PS2's order (B1 `FUN_0046bf10`): the
        // interval's clock (re-armed whatever the draw says), then the
        // probability.
        if self.next_ok.get(&hash).is_some_and(|t| self.now < *t) {
            return;
        }
        if let Some((interval, dev)) = p.play_interval {
            let next = self.now + interval + self.spread(dev);
            self.next_ok.insert(hash, next);
        }
        if p.play_probability.is_some_and(|pr| pr < self.next_rand()) {
            return;
        }
        // A bank sample when the property has one; otherwise the streamed VO
        // take. No property on either disc carries both (measured over the
        // 34 baked banks: 0 of 69,795 records name samples AND segments).
        let path = self.file_take(&p, core);
        let Some(path) = path else {
            return;
        };
        // Refused at level 0 (B1 `FUN_004599e0` 518974).
        let level = attenuation((pos - ears).length(), &p);
        if level <= 0.0 {
            return;
        }
        // Each start's own pitch and gain (B1 `FUN_0045ac00` 519742-519746).
        let pitch = p.pitch + p.pitch_dev.map_or(0.0, |d| self.spread(d));
        let gain = match p.gain_dev {
            Some(d) => (p.gain + self.spread(d)).clamp(0.0, 1.0),
            None => p.gain,
        };
        if gain * level <= 0.001 {
            return;
        }
        out.push(Start {
            hash,
            path,
            gain: gain * level,
            pitch,
            pos,
            bus: self.bus_of(&p),
            flat: p.flat,
            menu: p.bus == super::SHELLFX,
        });
    }

    /// Advance the waiting layers by `dt`: one whose time has passed starts
    /// (its level taken now, at the position it was armed with), then the
    /// others age (B1 `FUN_0045be28`, run once per sound update).
    pub fn tick(&mut self, dt: f32, ears: Vec3, out: &mut Vec<Start>) {
        let mut due = Vec::new();
        self.timers.retain_mut(|t| {
            // The slack is f32 rounding of a summed dt (0.3 s is 9 ticks at 30 Hz).
            if t.fire <= t.elapsed + 1e-5 {
                due.push((t.sound, t.pos));
                return false;
            }
            t.elapsed += dt;
            true
        });
        for (sound, pos) in due {
            self.start_prop(sound, pos, ears, out);
        }
        self.now += dt;
    }

    /// Layers waiting for their delay.
    pub fn waiting(&self) -> usize {
        self.timers.len()
    }
}

fn line_hash(name: &str) -> u32 {
    name.strip_prefix('#')
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .unwrap_or_else(|| fnv1a_lowercase(name))
}
