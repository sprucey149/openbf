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

/// Which mixer bus a sound plays on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    /// The battle: effects, engines, ambience and music. The pause menu
    /// pauses it.
    Game,
    /// Voice-over lines (commander, squad answers and campaign objective
    /// speech). Its own sub-track so a busy battle can never starve a line:
    /// the PS2 routes these on the separate `voiceover` bus
    /// (`analysis/re-mix-headroom.md` §1.4). Paused with the game bus.
    Voice,
    /// The menu clicks (`shellfx`), heard while the game bus is paused.
    Menu,
}

/// OURS: kira's default `TrackBuilder::new()` allows only 128 simultaneous
/// sounds per sub-track. A 22-flyer space round holds more engine, ordnance
/// and turret loops than that, so a one-shot — including a campaign objective
/// line — gets `PlaySoundError::SoundLimitReached` and is lost. The PS2 does
/// not cap voices by count (`analysis/re-mix-headroom.md` §1.3, §2): this only
/// gives the port enough slots to play what it submits.
/// measure: peak `Audio::num_sounds()` in a full space round before lowering.
pub const GAME_SOUND_CAPACITY: usize = 512;

/// OURS: a handful of slots reserved for voice lines on their own track, so a
/// saturated effects bus cannot drop a line.
/// measure: peak concurrent voice lines in a campaign round before lowering.
pub const VOICE_SOUND_CAPACITY: usize = 32;

/// OURS: the pause fade, short enough to read as instant without a click.
/// trace: the default fade time of BF1's `ingame` bus and BF2's 0xfa930d1c
/// bus, which the pause menus fade with time -1 (B1 `FUN_002d27f0`, B2
/// `FUN_0038b3c8`).
pub const PAUSE_FADE_SECS: f32 = 0.1;

pub struct Audio {
    #[cfg(feature = "backend")]
    manager: kira::AudioManager<kira::DefaultBackend>,
    /// [`Bus::Game`]: a sub-track of the main one, so the limiter covers it.
    #[cfg(feature = "backend")]
    game: kira::track::TrackHandle,
    /// [`Bus::Voice`]: a second sub-track, paused and renewed with the game.
    #[cfg(feature = "backend")]
    voice: kira::track::TrackHandle,
    game_paused: bool,
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
            let mut manager = match kira::AudioManager::<kira::DefaultBackend>::new(settings) {
                Ok(manager) => manager,
                Err(e) => {
                    eprintln!("audio: backend unavailable ({e})");
                    return None;
                }
            };
            let game = manager.add_sub_track(
                kira::track::TrackBuilder::new().sound_capacity(GAME_SOUND_CAPACITY),
            );
            let game = match game {
                Ok(game) => game,
                Err(e) => {
                    eprintln!("audio: game bus unavailable ({e})");
                    return None;
                }
            };
            match manager.add_sub_track(
                kira::track::TrackBuilder::new().sound_capacity(VOICE_SOUND_CAPACITY),
            ) {
                Ok(voice) => Some(Audio {
                    manager,
                    game,
                    voice,
                    game_paused: false,
                }),
                Err(e) => {
                    eprintln!("audio: voice bus unavailable ({e})");
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

    /// Pause or resume [`Bus::Game`] and [`Bus::Voice`] (the pause menu),
    /// fading over [`PAUSE_FADE_SECS`]; nothing when already so. OURS (trace:
    /// the original pause fade's length): the
    /// originals fade their in-game bus to silence while its voices run on,
    /// so a line or take that ends inside the pause is lost; here the buses
    /// stop, and everything on them resumes where it stopped.
    pub fn set_game_paused(&mut self, paused: bool) {
        if paused == self.game_paused {
            return;
        }
        self.game_paused = paused;
        #[cfg(feature = "backend")]
        {
            let tween = kira::Tween {
                duration: std::time::Duration::from_secs_f32(PAUSE_FADE_SECS),
                ..Default::default()
            };
            let (game, voice) = (&mut self.game, &mut self.voice);
            if paused {
                game.pause(tween);
                voice.pause(tween);
            } else {
                game.resume(tween);
                voice.resume(tween);
            }
        }
    }

    /// Drop everything on [`Bus::Game`] and [`Bus::Voice`] and start fresh,
    /// playing buses. Leaving a battle from the pause menu: the held one-shots
    /// (gunfire, explosion tails, voice-over) would otherwise resume over the
    /// shell.
    pub fn clear_game_bus(&mut self) {
        #[cfg(feature = "backend")]
        {
            // Pause immediately before renewing so held sounds do not leak
            // during track teardown.
            let _ = self.game.pause(kira::Tween::default());
            let _ = self.voice.pause(kira::Tween::default());
            let game = self.manager.add_sub_track(
                kira::track::TrackBuilder::new().sound_capacity(GAME_SOUND_CAPACITY),
            );
            let game = match game {
                Ok(game) => game,
                Err(e) => {
                    eprintln!("audio: game bus not renewed ({e})");
                    self.set_game_paused(false);
                    return;
                }
            };
            // Dropping the old handles removes the tracks and their sounds.
            let voice = self.manager.add_sub_track(
                kira::track::TrackBuilder::new().sound_capacity(VOICE_SOUND_CAPACITY),
            );
            match voice {
                Ok(voice) => {
                    self.game = game;
                    self.voice = voice;
                }
                Err(e) => {
                    eprintln!("audio: voice bus not renewed ({e})");
                    self.set_game_paused(false);
                    return;
                }
            }
        }
        self.game_paused = false;
    }

    /// Whether [`Bus::Game`] is paused ([`Audio::set_game_paused`]).
    pub fn game_paused(&self) -> bool {
        self.game_paused
    }

    /// Live sounds on the game bus, for the sound-limit diagnostic.
    pub fn num_sounds(&self) -> usize {
        #[cfg(feature = "backend")]
        {
            self.game.num_sounds()
        }
        #[cfg(not(feature = "backend"))]
        {
            0
        }
    }

    /// Live sounds on the voice bus, for the sound-limit diagnostic.
    pub fn voice_num_sounds(&self) -> usize {
        #[cfg(feature = "backend")]
        {
            self.voice.num_sounds()
        }
        #[cfg(not(feature = "backend"))]
        {
            0
        }
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
            let inner = self.game.play(data)?;
            Ok(LoopHandle { inner })
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (path, rate, volume_db);
            Ok(LoopHandle {})
        }
    }

    /// Fire-and-forget playback on [`Bus::Game`]: `volume_db` (0 = unity),
    /// `rate` (pitch multiplier), `pan` −1 (left) .. 1 (right).
    pub fn play_oneshot(&mut self, sound: &SoundData, volume_db: f32, rate: f64, pan: f32) {
        self.play_oneshot_on(Bus::Game, sound, volume_db, rate, pan);
    }

    /// [`Audio::play_oneshot`] on `bus`.
    pub fn play_oneshot_on(
        &mut self,
        bus: Bus,
        sound: &SoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) {
        #[cfg(feature = "backend")]
        {
            let _ = self.play_static(bus, &sound.inner, volume_db, rate, pan);
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (bus, sound, volume_db, rate, pan);
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
        self.play_shot_on(Bus::Game, sound, volume_db, rate, pan)
    }

    /// Play a one-shot on `Bus::Menu` (unaffected by game pause, stopped on [`Shot`] drop).
    pub fn play_menu_shot(&mut self, sound: &SoundData, volume_db: f32) -> Option<Shot> {
        self.play_shot_on(Bus::Menu, sound, volume_db, 1.0, 0.0)
    }

    fn play_shot_on(
        &mut self,
        bus: Bus,
        sound: &SoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) -> Option<Shot> {
        #[cfg(feature = "backend")]
        {
            self.play_static(bus, &sound.inner, volume_db, rate, pan)
                .map(|inner| Shot { inner })
        }
        #[cfg(not(feature = "backend"))]
        {
            let _ = (bus, sound, volume_db, rate, pan);
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
            self.play_static(
                Bus::Game,
                &sound.inner.loop_region(..),
                volume_db,
                rate,
                pan,
            )
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
        bus: Bus,
        sound: &kira::sound::static_sound::StaticSoundData,
        volume_db: f32,
        rate: f64,
        pan: f32,
    ) -> Option<kira::sound::static_sound::StaticSoundHandle> {
        let data = sound
            .volume(volume_db)
            .playback_rate(rate)
            .panning(pan.clamp(-1.0, 1.0));
        let (track, name) = match bus {
            Bus::Game => (&mut self.game, "game"),
            Bus::Voice => (&mut self.voice, "voice"),
            Bus::Menu => {
                return self
                    .manager
                    .play(data)
                    .inspect_err(|e| {
                        eprintln!("audio: play failed on menu ({e})");
                    })
                    .ok()
            }
        };
        track
            .play(data)
            .inspect_err(|e| {
                eprintln!(
                    "audio: play failed on {name} ({e}) [{} of {} sounds]",
                    track.num_sounds(),
                    track.sound_capacity()
                );
            })
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

    /// A silent mono WAV of 0.1 s, decoded.
    fn silence(tag: &str) -> SoundData {
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
        let path = std::env::temp_dir().join(format!("openbf-{tag}-{}.wav", std::process::id()));
        std::fs::write(&path, wav).unwrap();
        let data = decode(&path).expect("decode");
        let _ = std::fs::remove_file(&path);
        data
    }

    /// A looping voice starts, takes new values and stops when dropped -
    /// through the real mixer where there is one (silent: a WAV of zeros),
    /// as a no-op without.
    #[test]
    fn a_voice_loops_takes_values_and_stops_on_drop() {
        let data = silence("voice");
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

    /// The pause menu pauses the game and voice buses and resumes them, once
    /// per change; the menu bus plays on through it.
    #[cfg(feature = "backend")]
    #[test]
    fn the_game_bus_pauses_and_resumes() {
        let data = silence("pause");
        let Some(mut audio) = Audio::new() else {
            eprintln!("SKIP: no audio device");
            return;
        };
        use kira::track::TrackPlaybackState as S;
        let _voice = audio.play_voice(&data, -60.0, 1.0, 0.0).expect("voice");
        assert!(!audio.game_paused());
        audio.set_game_paused(true);
        audio.set_game_paused(true);
        assert!(audio.game_paused());
        audio.play_oneshot_on(Bus::Menu, &data, -60.0, 1.0, 0.0);
        let t = std::time::Instant::now();
        while !matches!(audio.game.state(), S::Pausing | S::Paused)
            || !matches!(audio.voice.state(), S::Pausing | S::Paused)
        {
            assert!(t.elapsed().as_secs() < 2, "{:?}", audio.game.state());
            std::thread::yield_now();
        }
        audio.set_game_paused(false);
        assert!(!audio.game_paused());
        let t = std::time::Instant::now();
        while !matches!(audio.game.state(), S::Resuming | S::Playing)
            || !matches!(audio.voice.state(), S::Resuming | S::Playing)
        {
            assert!(t.elapsed().as_secs() < 2, "{:?}", audio.game.state());
            std::thread::yield_now();
        }
    }

    /// Leaving from the pause drops the held sounds instead of resuming
    /// them, and leaves playing game and voice buses behind.
    #[cfg(feature = "backend")]
    #[test]
    fn clearing_the_paused_game_bus_drops_its_sounds() {
        let data = silence("clear");
        let Some(mut audio) = Audio::new() else {
            eprintln!("SKIP: no audio device");
            return;
        };
        audio.play_oneshot(&data, -60.0, 1.0, 0.0);
        audio.play_oneshot_on(Bus::Voice, &data, -60.0, 1.0, 0.0);
        assert_eq!((audio.num_sounds(), audio.voice_num_sounds()), (1, 1));
        audio.set_game_paused(true);
        audio.clear_game_bus();
        assert!(!audio.game_paused());
        assert_eq!((audio.num_sounds(), audio.voice_num_sounds()), (0, 0));
        assert_eq!(audio.game.state(), kira::track::TrackPlaybackState::Playing);
        assert_eq!(
            audio.voice.state(),
            kira::track::TrackPlaybackState::Playing
        );
        // The old buses and their one-shots go once the mixer next runs.
        let t = std::time::Instant::now();
        while audio.manager.num_sub_tracks() != 2 {
            assert!(t.elapsed().as_secs() < 2, "old buses still held");
            std::thread::yield_now();
        }
    }

    /// The game bus has real headroom over kira's 128-sound default, and the
    /// voice bus is independent: a line there cannot be refused because the
    /// effects bus is full (campaign objective VO in a busy space round).
    #[cfg(feature = "backend")]
    #[test]
    fn voice_lines_have_their_own_headroom() {
        let data = silence("headroom");
        let Some(mut audio) = Audio::new() else {
            eprintln!("SKIP: no audio device");
            return;
        };
        assert!(audio.game.sound_capacity() >= 512);
        assert!(audio.voice.sound_capacity() >= 32);
        let before = audio.voice_num_sounds();
        audio.play_oneshot_on(Bus::Voice, &data, -60.0, 1.0, 0.0);
        assert!(audio.voice_num_sounds() > before);
        assert_eq!(audio.num_sounds(), 0);
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
