//! The battle's music, fed from the sim (card AMBM): the mission's
//! `SetAmbientMusic` / `SetVictoryMusic` / `SetDefeatMusic` calls and the
//! SOUND LVL's `Music` records go into `core::music`'s manager and player;
//! each frame's changes come back for the caller to play (the streamed take,
//! `main/windowed.rs`) and to log. Presentation only: nothing here writes to
//! the sim. RE: `analysis/re-ambience-music.md` section 3.
//!
//! The music has a level of its own (card MUSV; the PS2's `ingamemusic`
//! bus): [`BattleMusic::volume`], the profile's `music_volume` or
//! `OPENBF_MUSIC_VOLUME`; at 0 nothing is asked and no take starts.

use crate::mission::MusicCalls;
use crate::sounds::{MusicTrack, SoundLib};
use core::music::{threshold, Ambient, Change, Entry, Player, Rule};
use core::sim::Sim;
use core::unit::UnitHandle;

/// The music volume option's range (profile `Settings::music_volume`, the
/// pause menu, `OPENBF_MUSIC_VOLUME`): a factor on the stream property's own
/// gain. 1.0 = what the disc's record plays at; 0 = no music at all.
pub const VOLUME_RANGE: (f32, f32) = (0.0, 1.0);
pub const DEFAULT_VOLUME: f32 = 1.0;
/// OURS (maintainer, 2026-11-05: the music sat too high over the battle): a
/// factor on every music take's gain before the volume option, about -3 dB
/// (analysis/plan/FEATURE_SET.md, Presentation & audio).
pub const MUSIC_MIX: f32 = 0.7;

/// The option's value: `env` (`OPENBF_MUSIC_VOLUME`, for a headless or a
/// scripted run) when it is a number, else the profile's; held in the range.
pub fn volume_option(env: Option<&str>, profile: f32) -> f32 {
    let v = env.and_then(|s| s.trim().parse::<f32>().ok());
    let v = v.filter(|v| v.is_finite()).unwrap_or(profile);
    if v.is_finite() {
        v.clamp(VOLUME_RANGE.0, VOLUME_RANGE.1)
    } else {
        DEFAULT_VOLUME
    }
}

/// [`volume_option`] from the environment.
pub fn volume_from_env(profile: f32) -> f32 {
    volume_option(
        std::env::var("OPENBF_MUSIC_VOLUME").ok().as_deref(),
        profile,
    )
}

pub struct BattleMusic {
    calls: MusicCalls,
    tickets: [u32; 2],
    pub player: Player,
    pub ambient: Ambient,
    /// One per `player.musics` entry.
    tracks: Vec<MusicTrack>,
    bound: bool,
    hero_names: Vec<(u8, u8, String)>,
    /// `(team, class, track)` per hero class with a `SoldierMusic`.
    hero_tracks: Vec<(u8, u8, usize)>,
    /// Seconds of [`BattleMusic::observe`] so far.
    pub clock: f32,
    /// The music volume option ([`VOLUME_RANGE`]).
    pub volume: f32,
}

impl BattleMusic {
    /// The mission's calls; `tickets` are the teams' reinforcements at the
    /// script's calls (the battle's start), `bf2` picks the state rule.
    /// Silent until [`BattleMusic::bind`] finds the records.
    pub fn new(calls: &MusicCalls, bf2: bool, tickets: [u32; 2]) -> BattleMusic {
        BattleMusic {
            calls: calls.clone(),
            tickets,
            player: Player::default(),
            ambient: Ambient::new(if bf2 { Rule::Bf2 } else { Rule::Bf1 }),
            tracks: Vec::new(),
            bound: false,
            hero_names: Vec::new(),
            hero_tracks: Vec::new(),
            clock: 0.0,
            volume: DEFAULT_VOLUME,
        }
    }

    /// Look the script's names up in the map's bank. A name with no record
    /// leaves its entry empty, as the PS2's find does. Returns the names
    /// that were not found.
    pub fn bind(&mut self, sounds: &SoundLib) -> Vec<String> {
        self.bind_with(|n| sounds.music(n))
    }

    /// Each hero class's `SoldierMusic`, as `(team, class, property)`: the
    /// team's one hero in Conquest, every class in Hero Assault.
    pub fn set_hero_music(&mut self, names: Vec<(u8, u8, String)>) {
        self.hero_names = names;
        self.bound = false;
    }

    pub fn bind_with(&mut self, find: impl Fn(&str) -> Option<MusicTrack>) -> Vec<String> {
        self.bound = true;
        let team = |t: u8| matches!(t, 1 | 2).then(|| usize::from(t) - 1);
        let mut missing = Vec::new();
        let mut tracks: Vec<MusicTrack> = Vec::new();
        let mut index = |name: &str, missing: &mut Vec<String>| {
            if let Some(i) = tracks
                .iter()
                .position(|t| t.def.name.eq_ignore_ascii_case(name))
            {
                return Some(i);
            }
            match find(name) {
                Some(t) => {
                    tracks.push(t);
                    Some(tracks.len() - 1)
                }
                None => {
                    missing.push(name.to_owned());
                    None
                }
            }
        };
        for (t, x, name, state, flag) in &self.calls.ambient {
            if let Some(t) = team(*t) {
                let thr = threshold(*x, *flag, self.tickets[t] as i32);
                let m = index(name, &mut missing);
                self.ambient.set(t, usize::from(*state), m, thr);
            }
        }
        for (rows, table) in [
            (&self.calls.victory, &mut self.ambient.victory),
            (&self.calls.defeat, &mut self.ambient.defeat),
        ] {
            for (t, name) in rows {
                if let Some(t) = team(*t) {
                    table[t] = index(name, &mut missing).map(Entry::new);
                }
            }
        }
        self.hero_tracks = self
            .hero_names
            .iter()
            .filter_map(|(team, class, name)| Some((*team, *class, index(name, &mut missing)?)))
            .collect();
        self.player = Player::new(tracks.iter().map(|t| t.def.clone()).collect());
        self.tracks = tracks;
        missing
    }

    pub fn bound(&self) -> bool {
        self.bound
    }

    pub fn track(&self, music: usize) -> Option<&MusicTrack> {
        self.tracks.get(music)
    }

    /// Length of the listener's selected end-music take, even without a device;
    /// None when no end music is asked (volume 0, or the round ended before the
    /// first spawn: [`BattleMusic::observe`]).
    pub fn end_music_secs(&self, listener: usize, winner: u8) -> Option<f32> {
        if self.volume <= 0.0 || !self.ambient.enabled {
            return None;
        }
        let table = if usize::from(winner) == listener {
            &self.ambient.victory
        } else {
            &self.ambient.defeat
        };
        let entry = table.get(listener)?.as_ref()?;
        let music = self.player.musics.get(entry.music)?;
        music
            .def
            .takes
            .get(music.take)
            .copied()
            .filter(|s| s.is_finite() && *s > 0.0)
    }

    /// The Music that sounds and its level 0..1: the fade times the stream
    /// property's gain times the volume option.
    pub fn level(&self) -> Option<(usize, f32)> {
        let (i, g) = self.player.current()?;
        let v = self.volume.clamp(VOLUME_RANGE.0, VOLUME_RANGE.1);
        Some((i, g * self.tracks[i].gain * v * MUSIC_MIX))
    }

    /// One frame of the battle's state: `listener` is the local player's
    /// team, `spawned` whether a human unit is in the battle, `outcome` the
    /// winner once it is decided. Returns what changed.
    pub fn observe(
        &mut self,
        dt: f32,
        listener: usize,
        tickets: [u32; 2],
        spawned: bool,
        outcome: Option<u8>,
    ) -> Vec<Change> {
        self.clock += dt;
        if spawned {
            self.ambient.enabled = true;
        }
        // The volume at 0: nothing is asked, and what sounds is cut. Raised
        // again, the next frame asks the state's Music afresh.
        if self.volume <= 0.0 {
            return self.player.stop_all();
        }
        let mut out = Vec::new();
        match outcome {
            // Before the first spawn nobody listens: no end music either.
            Some(w) if self.ambient.enabled => {
                let won = usize::from(w) == listener;
                out.extend(self.ambient.finish(&mut self.player, listener, won));
            }
            Some(_) => {}
            None => {
                let t = [tickets[0] as i32, tickets[1] as i32];
                self.ambient.update(&mut self.player, listener, t);
            }
        }
        out.extend(self.player.update(dt));
        out
    }

    /// [`BattleMusic::observe`] from the sim.
    pub fn frame(&mut self, sim: &Sim, player: UnitHandle, dt: f32) -> Vec<Change> {
        let me = sim.unit(player);
        for &(_, _, index) in &self.hero_tracks {
            if let Some(music) = self.player.musics.get_mut(index) {
                music.wanted = false;
            }
        }
        // OURS (maintainer, 2026-11-05): Hero Assault plays its heroes'
        // music too, though `tat2g_eli` switches hero music off.
        let hero = sim.is_hero(player) || sim.hero_assault();
        if sim.outcome.is_none() && hero {
            let track = me.and_then(|u| {
                self.hero_tracks
                    .iter()
                    .find(|&&(t, c, _)| t == u.team && c == u.spec.class)
            });
            if let Some(&(_, _, index)) = track {
                if let Some(music) = self.player.musics.get_mut(index) {
                    music.wanted = me.is_some_and(|u| u.alive);
                }
            }
        }
        // The slot's team, dead or alive: a battle that ends while the
        // player waits to respawn plays HIS team's end music.
        let listener = sim
            .units
            .get(player.index as usize)
            .map_or(0, |u| usize::from(u.team));
        self.observe(
            dt,
            listener,
            [0, 1].map(|t| sim.battle.teams[t].tickets()),
            me.is_some(),
            sim.outcome,
        )
    }

    /// [`BattleMusic::line`] with the manager's state and the teams'
    /// reinforcements: `MUSIC_LOG`'s line, and the window's in dev mode.
    pub fn log_line(&self, c: &Change, tickets: [u32; 2]) -> String {
        format!(
            "{} [state {:?}, tickets {tickets:?}]",
            self.line(c),
            self.ambient.state
        )
    }

    /// A change as a log line: `music 12.3 s start cis_nab_amb_start take
    /// 72e2dd48 (53.9 s)`.
    pub fn line(&self, c: &Change) -> String {
        match *c {
            Change::Started { music, take } => {
                let t = &self.tracks[music];
                format!(
                    "music {:.1} s start {} take {:08x} ({:.1} s{})",
                    self.clock,
                    t.def.name,
                    t.ids.get(take).copied().unwrap_or(0),
                    t.def.takes.get(take).copied().unwrap_or(0.0),
                    if t.def.looping { ", looping" } else { "" }
                )
            }
            Change::Stopped { music } => {
                format!(
                    "music {:.1} s stop {}",
                    self.clock, self.tracks[music].def.name
                )
            }
        }
    }
}

/// `MUSIC_LOG=<1|2>` (a headless round, card AMBM's referee): the battle's
/// music as Lua team 1 or 2 hears it from the first tick - nobody is human
/// in a sweep round, so the manager is on from the start. Prints what the
/// script named and the bank lacks.
pub struct HeadlessLog {
    music: BattleMusic,
    listener: usize,
}

impl HeadlessLog {
    pub fn from_env(
        map_dir: &std::path::Path,
        mission: Option<&crate::mission::MissionDef>,
        tickets: [u32; 2],
    ) -> Option<HeadlessLog> {
        let team: usize = std::env::var("MUSIC_LOG").ok()?.parse().ok()?;
        let mission = mission?;
        // `OPENBF_SOUND_ROOT=<dir>`: a scratch bake's `<map>/sounds`.
        let map_dir = match (std::env::var_os("OPENBF_SOUND_ROOT"), map_dir.file_name()) {
            (Some(root), Some(name)) => std::path::PathBuf::from(root).join(name),
            _ => map_dir.to_path_buf(),
        };
        let sounds = SoundLib::load_for(&map_dir, mission)?;
        let mut music = BattleMusic::new(&mission.music, mission.source_game == "bf2", tickets);
        music.volume = volume_from_env(DEFAULT_VOLUME);
        let missing = music.bind(&sounds);
        eprintln!(
            "music: {} Musics for {}, thresholds {:?}, rule {:?}, not in the bank {missing:?}",
            music.player.musics.len(),
            mission.map,
            music.ambient.thr,
            music.ambient.rule,
        );
        Some(HeadlessLog {
            music,
            listener: team.clamp(1, 2) - 1,
        })
    }

    /// The 20 s after the battle's end: the victory or defeat music.
    pub fn finish(&mut self, sim: &Sim) {
        for _ in 0..(20.0 / core::TICK_DT) as usize {
            self.tick(sim);
        }
    }

    /// One tick; a line per change to stderr.
    pub fn tick(&mut self, sim: &Sim) {
        let tickets = [0, 1].map(|t| sim.battle.teams[t].tickets());
        let changes = self
            .music
            .observe(core::TICK_DT, self.listener, tickets, true, sim.outcome);
        for c in changes {
            eprintln!("{}", self.music.log_line(&c, tickets));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::music::MusicDef;

    fn track(name: &str) -> Option<MusicTrack> {
        let end = name.ends_with("victory") || name.ends_with("defeat");
        // A map with no `end` record: the entry stays empty.
        if name.ends_with("amb_end") {
            return None;
        }
        Some(MusicTrack {
            def: MusicDef {
                name: name.to_owned(),
                fade_in: if end { 0.0 } else { 1.0 },
                fade_out: 1.0,
                min_playback: 5.0,
                min_inactive: 30.0,
                max_playback: None,
                priority: if end { 1.0 } else { 0.0 },
                looping: false,
                takes: vec![20.0],
            },
            gain: 0.5,
            takes: Vec::new(),
            ids: vec![7],
        })
    }

    #[test]
    fn banner_time_uses_the_listener_and_selected_end_take() {
        use crate::round_end::{Phase, Result, RoundEnd};
        let mut m = BattleMusic::new(&calls(), false, [200, 200]);
        m.bind_with(|name| {
            let mut t = track(name)?;
            t.def.takes = vec![if name.ends_with("defeat") { 4.9 } else { 8.4 }];
            Some(t)
        });
        m.ambient.enabled = true;
        for listener in [0, 1] {
            for winner in [0, 1] {
                let won = listener == usize::from(winner);
                let secs = m.end_music_secs(listener, winner);
                assert_eq!(secs, Some(if won { 8.4 } else { 4.9 }));
                let mut r = RoundEnd::start(
                    if won { Result::Victory } else { Result::Defeat },
                    &[],
                    1,
                    secs,
                );
                r.step(4.99, ui::stats::Pad::default());
                assert_eq!(r.phase, Phase::Banner);
                r.step(0.02, ui::stats::Pad::default());
                assert_eq!(r.phase, if won { Phase::Banner } else { Phase::Stats });
                r.step(1.98, ui::stats::Pad::default());
                assert_eq!(r.phase, if won { Phase::Banner } else { Phase::Stats });
                r.step(0.02, ui::stats::Pad::default());
                assert_eq!(r.phase, Phase::Stats, "7 s at most");
            }
        }
        assert_eq!(m.end_music_secs(2, 0), None);
        m.ambient.enabled = false;
        assert_eq!(m.end_music_secs(0, 0), None, "no spawn: no end music");
        m.ambient.enabled = true;
        m.volume = 0.0;
        assert_eq!(m.end_music_secs(0, 0), None, "volume 0: no end music");
        m.volume = DEFAULT_VOLUME;
        let entry = m.ambient.victory[0].unwrap();
        m.player.musics[entry.music].def.takes = vec![8.4, 6.0];
        m.player.musics[entry.music].take = 1;
        assert_eq!(m.end_music_secs(0, 0), Some(6.0));
    }

    #[test]
    fn hero_music_follows_the_live_hero_and_releases_its_authored_track_at_death() {
        use core::{
            sim::{heroes::Heroes, Sim},
            unit::UnitSpec,
        };
        let mut sim = Sim::empty(1);
        let hero = sim.spawn(&UnitSpec::player(0, glam::Vec3::ZERO)).unwrap();
        sim.heroes = Some(Heroes::new([Some(0), None]));
        let mut m = BattleMusic::new(&MusicCalls::default(), true, [150; 2]);
        m.set_hero_music(vec![(0, 0, "hero_theme".into())]);
        m.bind_with(track);
        let changes = m.frame(&sim, hero, core::TICK_DT);
        assert!(changes
            .iter()
            .any(|c| matches!(c, Change::Started { music: 0, .. })));
        assert!(m.player.musics[0].wanted);
        sim.units[hero.index as usize].alive = false;
        m.frame(&sim, hero, core::TICK_DT);
        assert!(!m.player.musics[0].wanted);
        // Authored minimum playback and fade still own the audible stop.
        for _ in 0..210 {
            m.frame(&sim, hero, core::TICK_DT);
        }
        assert!(m.player.current().is_none());
    }

    fn calls() -> MusicCalls {
        let mut c = MusicCalls::default();
        for (t, side) in [(2u8, "rep"), (1, "cis")] {
            for (s, (name, x)) in [("start", 1.0), ("middle", 0.99), ("end", 0.1)]
                .iter()
                .enumerate()
            {
                let n = format!("{side}_nab_amb_{name}");
                c.ambient.push((t, *x, n, s as u8, true));
            }
            c.victory.push((t, format!("{side}_nab_amb_victory")));
            c.defeat.push((t, format!("{side}_nab_amb_defeat")));
        }
        c
    }

    #[test]
    fn the_scripts_calls_become_the_table_and_a_battle_is_heard() {
        let mut m = BattleMusic::new(&calls(), false, [200, 200]);
        let missing = m.bind_with(track);
        assert_eq!(missing, ["rep_nab_amb_end", "cis_nab_amb_end"]);
        // Lua team 1 (cis) is port team 0.
        assert_eq!(m.ambient.thr, [[200, 198, 20]; 2]);
        let dt = 1.0 / 60.0;
        let mut heard = Vec::new();
        let mut run = |m: &mut BattleMusic, secs: f32, tk, spawned, outcome| {
            for _ in 0..(secs / dt) as usize {
                for c in m.observe(dt, 0, tk, spawned, outcome) {
                    if matches!(c, Change::Started { .. }) {
                        heard.push(m.line(&c));
                    }
                }
            }
        };
        run(&mut m, 3.0, [200, 200], false, None);
        run(&mut m, 3.0, [200, 200], true, None);
        let level = m.level().map_or(0.0, |(_, g)| g);
        assert!(
            (level - 0.5 * MUSIC_MIX).abs() < 1e-6,
            "fade x gain: {level}"
        );
        run(&mut m, 8.0, [200, 197], true, None);
        run(&mut m, 12.0, [200, 0], true, Some(0));
        let names: Vec<&str> = heard
            .iter()
            .map(|l| l.split_whitespace().nth(4).unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "cis_nab_amb_start",
                "cis_nab_amb_middle",
                "cis_nab_amb_victory"
            ],
            "{heard:?}"
        );
        assert!(heard[0].starts_with("music 3.0 s start"), "{heard:?}");
    }

    #[test]
    fn the_volume_option_has_a_default_a_range_and_an_off() {
        assert_eq!(DEFAULT_VOLUME, 1.0);
        assert_eq!(VOLUME_RANGE, (0.0, 1.0));
        assert_eq!(crate::profile::Settings::default().music_volume, 1.0);
        assert_eq!(volume_option(None, 0.4), 0.4);
        assert_eq!(volume_option(Some("0.25"), 1.0), 0.25);
        assert_eq!(volume_option(Some("0"), 1.0), 0.0);
        assert_eq!(volume_option(Some("7"), 1.0), 1.0);
        assert_eq!(volume_option(Some("-1"), 1.0), 0.0);
        assert_eq!(volume_option(Some("loud"), 0.5), 0.5);
        assert_eq!(volume_option(Some("NaN"), f32::NAN), 1.0);

        let dt = 1.0 / 60.0;
        let mut m = BattleMusic::new(&calls(), false, [200, 200]);
        m.bind_with(track);
        assert_eq!(m.volume, DEFAULT_VOLUME);
        let run = |m: &mut BattleMusic, secs: f32| {
            let mut out = Vec::new();
            for _ in 0..(secs / dt) as usize {
                out.extend(m.observe(dt, 0, [200, 200], true, None));
            }
            out
        };
        // Off: no take is started, nothing sounds.
        m.volume = 0.0;
        assert!(run(&mut m, 5.0).is_empty());
        assert_eq!(m.level(), None);
        // Raised: the state's Music starts, at the option times the gain.
        m.volume = 0.5;
        let c = run(&mut m, 5.0);
        // Lua team 1 (cis) listens: its `start` is the third record bound.
        assert!(matches!(c[..], [Change::Started { music: 2, .. }]), "{c:?}");
        let level = m.level().map_or(0.0, |(_, g)| g);
        assert!((level - 0.25 * MUSIC_MIX).abs() < 1e-6, "{level}");
        // Off again: the take is cut, once.
        m.volume = 0.0;
        let c = run(&mut m, 1.0);
        assert!(matches!(c[..], [Change::Stopped { music: 2 }]), "{c:?}");
    }

    #[test]
    fn a_battle_nobody_spawned_in_ends_silent() {
        let mut m = BattleMusic::new(&calls(), true, [150, 150]);
        m.bind_with(track);
        for _ in 0..600 {
            assert!(m
                .observe(1.0 / 60.0, 0, [150, 0], false, Some(1))
                .is_empty());
        }
    }
}
