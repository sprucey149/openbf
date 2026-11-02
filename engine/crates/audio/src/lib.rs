//! Audio: kira mixer behind the `backend` feature (default on; needs ALSA dev
//! headers to link on Linux — alsa-lib-devel). Without the feature every call
//! is a silent no-op, so the game's audio hooks (loading beeps first — PLAN
//! §Presentation) stay wired on boxes that can't link it.

use std::path::Path;

pub mod limiter;
pub mod stream;
pub use stream::{MusicVoice, StreamTake, TakeReader};

/// Handle to a playing looped sound.
pub struct LoopHandle {
    #[cfg(feature = "backend")]
    inner: kira::sound::static_sound::StaticSoundHandle,
}

/// A looping voice whose gain, rate and pan follow its source every frame
/// (a vehicle's engine layer, card VSND; card WHUM's hum). Stops when
/// dropped.
pub struct Voice {
    #[cfg(feature = "backend")]
    inner: kira::sound::static_sound::StaticSoundHandle,
}

impl Voice {
    /// `volume_db` (0 = unity), `rate` (pitch multiplier), `pan` -1 (left)
    /// .. 1 (right), each reached over `secs`.
    pub fn set(&mut self, volume_db: f32, rate: f64, pan: f32, secs: f32) {
        #[cfg(feature = "backend")]
        {
            let tween = kira::Tween {
                duration: std::time::Duration::from_secs_f32(secs.max(0.0)),
                ..Default::default()
            };
            self.inner.set_volume(volume_db, tween);
            self.inner.set_playback_rate(rate, tween);
            self.inner.set_panning(pan.clamp(-1.0, 1.0), tween);
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (volume_db, rate, pan, secs);
        }
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        #[cfg(feature = "backend")]
        self.inner.stop(kira::Tween::default());
    }
}

/// A one-shot that can be cut short (a unit's hurt or death cry, card
/// UCRY: one voice per unit). Stops when dropped; a finished one stops
/// nothing.
pub struct Shot {
    #[cfg(feature = "backend")]
    inner: kira::sound::static_sound::StaticSoundHandle,
}

impl Drop for Shot {
    fn drop(&mut self) {
        #[cfg(feature = "backend")]
        self.inner.stop(kira::Tween::default());
    }
}

/// Decoded one-shot sound.
pub struct SoundData {
    #[cfg(feature = "backend")]
    inner: kira::sound::static_sound::StaticSoundData,
}

impl SoundData {
    /// Sample rate of the decoded buffer, 0 without a backend.
    pub fn sample_rate(&self) -> u32 {
        #[cfg(feature = "backend")]
        {
            self.inner.sample_rate
        }
        #[cfg(not(feature = "backend"))]
        {
            0
        }
    }

    /// Decoded frames, 0 without a backend.
    pub fn frames(&self) -> usize {
        #[cfg(feature = "backend")]
        {
            self.inner.num_frames()
        }
        #[cfg(not(feature = "backend"))]
        {
            0
        }
    }

    /// Length in seconds, 0.0 without a backend.
    pub fn duration_secs(&self) -> f32 {
        #[cfg(feature = "backend")]
        {
            self.inner.duration().as_secs_f32()
        }
        #[cfg(not(feature = "backend"))]
        {
            0.0
        }
    }
}

/// Decode a WAV/OGG off disk WITHOUT opening the mixer — the same decoder
/// playback uses. A headless box (CI: no output device, so
/// [`Audio::new`] is None) can still check what a baked file holds. Without
/// the backend feature this is the silent no-op the rest of the API is.
pub fn decode(path: &Path) -> anyhow::Result<SoundData> {
    #[cfg(feature = "backend")]
    {
        Ok(SoundData {
            inner: kira::sound::static_sound::StaticSoundData::from_cursor(std::io::Cursor::new(
                pak::read(path)?,
            ))?,
        })
    }
    #[cfg(not(feature = "backend"))]
    {
        let _ = path;
        Ok(SoundData {})
    }
}

pub struct Audio {
    #[cfg(feature = "backend")]
    manager: kira::AudioManager<kira::DefaultBackend>,
}

impl Audio {
    /// None when no backend is compiled in or the device can't be opened.
    /// Everything plays through the master limiter ([`limiter`], card MIXH).
    pub fn new() -> Option<Audio> {
        #[cfg(feature = "backend")]
        {
            let settings = kira::AudioManagerSettings {
                main_track_builder: kira::track::MainTrackBuilder::new()
                    .with_effect(limiter::Builder),
                ..Default::default()
            };
            match kira::AudioManager::<kira::DefaultBackend>::new(settings) {
                Ok(manager) => Some(Audio { manager }),
                Err(e) => {
                    eprintln!("audio: backend unavailable ({e})");
                    None
                }
            }
        }
        #[cfg(not(feature = "backend"))]
        {
            None
        }
    }

    pub fn backend_compiled() -> bool {
        cfg!(feature = "backend")
    }

    /// Loop an OGG/WAV at a playback-rate multiplier (BF1 loading beeps:
    /// load_xtracking 2.1 / load_ytracking 1.8 over the 440 Hz ui_load_lp).
    pub fn play_loop(
        &mut self,
        path: &Path,
        rate: f64,
        volume_db: f32,
    ) -> anyhow::Result<LoopHandle> {
        #[cfg(feature = "backend")]
        {
            use kira::sound::static_sound::StaticSoundData;
            let data = StaticSoundData::from_cursor(std::io::Cursor::new(pak::read(path)?))?
                .loop_region(..)
                .playback_rate(rate)
                .volume(volume_db);
            let inner = self.manager.play(data)?;
            Ok(LoopHandle { inner })
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (path, rate, volume_db);
            Ok(LoopHandle {})
        }
    }

    /// Fire-and-forget playback: `volume_db` (0 = unity), `rate` (pitch
    /// multiplier), `pan` −1 (left) .. 1 (right).
    pub fn play_oneshot(&mut self, sound: &SoundData, volume_db: f32, rate: f64, pan: f32) {
        #[cfg(feature = "backend")]
        {
            let _ = self.play_static(&sound.inner, volume_db, rate, pan);
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (sound, volume_db, rate, pan);
        }
    }

    /// [`Audio::play_oneshot`] with a handle that cuts it ([`Shot`]); None
    /// when the mixer refuses it.
    pub fn play_shot(
        &mut self,
        sound: &SoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) -> Option<Shot> {
        #[cfg(feature = "backend")]
        {
            self.play_static(&sound.inner, volume_db, rate, pan)
                .map(|inner| Shot { inner })
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (sound, volume_db, rate, pan);
            Some(Shot {})
        }
    }

    /// Loop a decoded sound as a [`Voice`]; None when the mixer refuses it.
    pub fn play_voice(
        &mut self,
        sound: &SoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) -> Option<Voice> {
        #[cfg(feature = "backend")]
        {
            self.play_static(&sound.inner.loop_region(..), volume_db, rate, pan)
                .map(|inner| Voice { inner })
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (sound, volume_db, rate, pan);
            Some(Voice {})
        }
    }

    #[cfg(feature = "backend")]
    fn play_static(
        &mut self,
        sound: &kira::sound::static_sound::StaticSoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) -> Option<kira::sound::static_sound::StaticSoundHandle> {
        let data = sound
            .volume(volume_db)
            .playback_rate(rate)
            .panning(pan.clamp(-1.0, 1.0));
        self.manager
            .play(data)
            .inspect_err(|e| eprintln!("audio: play failed ({e})"))
            .ok()
    }

    pub fn stop(&mut self, handle: &mut LoopHandle) {
        #[cfg(feature = "backend")]
        {
            handle.inner.stop(kira::Tween::default());
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = handle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A looping voice starts, takes new values and stops when dropped -
    /// through the real mixer where there is one (silent: a WAV of zeros),
    /// as a no-op without.
    #[test]
    fn a_voice_loops_takes_values_and_stops_on_drop() {
        let frames = 2205u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + frames * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for v in [16u32, 0x0001_0001, 22_050, 44_100, 0x0010_0002] {
            wav.extend_from_slice(&v.to_le_bytes());
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(frames * 2).to_le_bytes());
        wav.resize(wav.len() + frames as usize * 2, 0);
        let path = std::env::temp_dir().join(format!("openbf-voice-{}.wav", std::process::id()));
        std::fs::write(&path, wav).unwrap();
        let data = decode(&path).expect("decode");
        let _ = std::fs::remove_file(&path);
        if Audio::backend_compiled() {
            assert_eq!((data.frames(), data.sample_rate()), (2205, 22_050));
        }
        let Some(mut audio) = Audio::new() else {
            eprintln!("SKIP: no audio device");
            return;
        };
        let mut voice = audio.play_voice(&data, -60.0, 0.8, -0.5).expect("voice");
        voice.set(-40.0, 1.1, 0.5, 1.0 / 30.0);
        voice.set(-60.0, 0.1, 2.0, 0.0);
        drop(voice);
    }

    /// Opening the mixer must never panic; a box without an output device
    /// (CI, headless) yields None.
    #[test]
    fn backend_opens_or_degrades() {
        let a = Audio::new();
        println!(
            "backend compiled: {}, opened: {}",
            Audio::backend_compiled(),
            a.is_some()
        );
    }
}
