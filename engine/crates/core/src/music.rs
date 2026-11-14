//! The battle's music (card AMBM; `analysis/re-ambience-music.md` section 3,
//! cited below as RE 3.n). Pure rules, no audio: [`Ambient`] is the
//! VoiceOverManager's music table and its per-frame update, [`Player`] holds
//! every [`Music`] and lets one play at a time. `game::music` feeds both from
//! the sim and plays what the player starts.
//!
//! What is here:
//! - the state a battle's music is in, by reinforcements: BF1 takes team 2's
//!   state ([`state_bf1`], RE 3.2 - PROVISIONAL: no PS2 state separates it
//!   from BF2's rule, it stands on the instructions alone, RE 11), BF2 the
//!   less advanced team's ([`state_bf2`], RE 3.3);
//! - the manager's update: ask the listener team's Music of that state, stop
//!   both teams' Musics of the state before it and the old listener's when
//!   the listener changes (RE 3.5-3.6);
//! - a Music's life: fade in, hold at least `MinPlaybackTime`, fade out, no
//!   start for `MinInactiveTime` after a stop; a BF1 take ends by itself, a
//!   BF2 take loops until `MaxPlaybackTime` (RE 3.7-3.8);
//! - one Music at a time, the asked one of the highest priority (RE 3.9);
//! - the battle's end: everything cut, the listener team's victory or defeat
//!   Music (RE 3.13).
//!
//! Not here: the pause path (RE 3.8, its requester is not read), the options
//! screen's interval override (RE 3.10), the capture / hero / script music
//! (RE 3.14).

use crate::fmath::FloatExt;

/// Seconds between the end of a take and the first frame its Music counts as
/// stopped. PROVISIONAL: measured, not read - BF1 Naboo's `cis_nab_amb_middle`
/// starts its next take 30.29 and 30.27 s after the last one's audio ends
/// with `MinInactiveTime` 30 (RE 3.12); the cause (the stream's stop
/// detection, the next one's open) is not known.
pub const RESTART_LAG: f32 = 0.28;

/// One team's music state by its reinforcements against its three thresholds
/// (start, middle, end): the last one it is at or under, None above the
/// first (RE 3.2).
pub fn team_state(tickets: i32, thr: [i32; 3]) -> Option<u8> {
    if tickets <= thr[2] {
        Some(2)
    } else if tickets <= thr[1] {
        Some(1)
    } else if tickets <= thr[0] {
        Some(0)
    } else {
        None
    }
}

/// BF1's state (`FUN_002b1280`): team index 0 then 1, the last team that has
/// a state sets it - team 2's whatever team 1's is. 0 when neither has one.
pub fn state_bf1(tickets: [i32; 2], thr: [[i32; 3]; 2]) -> u8 {
    let mut s = 0;
    for i in 0..2 {
        if let Some(x) = team_state(tickets[i], thr[i]) {
            s = x;
        }
    }
    s
}

/// BF2's state (`FUN_00373ad0`): the minimum over the teams that have one, 0
/// when none has.
pub fn state_bf2(tickets: [i32; 2], thr: [[i32; 3]; 2]) -> u8 {
    (0..2)
        .filter_map(|i| team_state(tickets[i], thr[i]))
        .min()
        .unwrap_or(0)
}

/// Which game's state rule a battle runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Bf1,
    Bf2,
}

impl Rule {
    pub fn state(self, tickets: [i32; 2], thr: [[i32; 3]; 2]) -> u8 {
        match self {
            Rule::Bf1 => state_bf1(tickets, thr),
            Rule::Bf2 => state_bf2(tickets, thr),
        }
    }
}

/// `SetAmbientMusic`'s threshold: (int) of x, times the team's reinforcements
/// at the call when the 5th argument is set (RE 3.1; the PS2's float
/// product, truncated).
pub fn threshold(x: f32, scaled: bool, tickets: i32) -> i32 {
    if scaled {
        (x * tickets as f32) as i32
    } else {
        x as i32
    }
}

/// A `Music` record of the SOUND LVL with its stream property's takes.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicDef {
    pub name: String,
    pub fade_in: f32,
    pub fade_out: f32,
    pub min_playback: f32,
    pub min_inactive: f32,
    /// BF2's `MaxPlaybackTime`; None on BF1.
    pub max_playback: Option<f32>,
    pub priority: f32,
    /// The stream property's `Looping`.
    pub looping: bool,
    /// Each take's length in seconds, in the property's list order.
    pub takes: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    FadingIn,
    Playing,
    FadingOut,
    Stopped,
}

/// One Music's state (RE 3.8).
#[derive(Debug, Clone)]
pub struct Music {
    pub def: MusicDef,
    pub phase: Phase,
    /// The target: asked to play (an entry's play, RE 3.6) or to stop.
    pub wanted: bool,
    /// Seconds since this play started.
    pub elapsed: f32,
    /// Seconds since it stopped; starts at the limit, so the first start is
    /// not delayed (RE 3.7).
    pub idle: f32,
    /// Seconds into the playing take.
    pub pos: f32,
    /// The playing take; the next start draws the one after it.
    /// PROVISIONAL: list order from the first take is BF1's measured order
    /// (`CyclePlayback` 1, RE 3.12); BF2's `CyclePlayback` 3 is not read.
    pub take: usize,
    pub gain: f32,
    /// The gain the running fade-out started from.
    fade_from: f32,
    started: bool,
}

impl Music {
    pub fn new(def: MusicDef) -> Music {
        let idle = def.min_inactive + RESTART_LAG;
        Music {
            def,
            phase: Phase::Stopped,
            wanted: false,
            elapsed: 0.0,
            idle,
            pos: 0.0,
            take: 0,
            gain: 0.0,
            fade_from: 1.0,
            started: false,
        }
    }

    pub fn playing(&self) -> bool {
        self.phase != Phase::Stopped
    }

    /// Whether the inactive time has passed.
    pub fn rested(&self) -> bool {
        self.idle >= self.def.min_inactive + RESTART_LAG
    }

    fn start(&mut self) {
        if self.started && !self.def.takes.is_empty() {
            self.take = (self.take + 1) % self.def.takes.len();
        }
        self.started = true;
        self.elapsed = 0.0;
        self.pos = 0.0;
        if self.def.fade_in > 0.0 {
            self.phase = Phase::FadingIn;
            self.gain = 0.0;
        } else {
            self.phase = Phase::Playing;
            self.gain = 1.0;
        }
    }

    /// Stopped at once, the idle time from 0: the take ended, or every sound
    /// was stopped.
    fn cut(&mut self) {
        self.phase = Phase::Stopped;
        self.wanted = false;
        self.gain = 0.0;
        self.idle = 0.0;
    }

    /// One frame of a Music that plays. True when it stopped in it.
    fn advance(&mut self, dt: f32) -> bool {
        match self.phase {
            Phase::Stopped => {
                self.idle += dt;
                return false;
            }
            Phase::FadingOut => {
                // The fade's own clock is the gain: `fade_from` to 0 over
                // FadeOutTime.
                let step = if self.def.fade_out > 0.0 {
                    dt / self.def.fade_out
                } else {
                    1.0
                };
                self.gain -= step * self.fade_from.fmax(f32::EPSILON);
                if self.gain <= 0.0 {
                    self.cut();
                    return true;
                }
                self.pos += dt;
                return false;
            }
            Phase::FadingIn | Phase::Playing => {}
        }
        self.elapsed += dt;
        self.pos += dt;
        let len = self.def.takes.get(self.take).copied().unwrap_or(0.0);
        if self.pos >= len {
            if self.def.looping && len > 0.0 {
                self.pos %= len;
            } else {
                // The slot's voice has stopped: the Music is done. The
                // PS2 passes through its fade-out state here; the measured
                // gap to the next take (RE 3.12) has no FadeOutTime in it.
                self.cut();
                return true;
            }
        }
        if self.phase == Phase::FadingIn {
            self.gain = (self.elapsed / self.def.fade_in).fmin(1.0);
            if self.gain >= 1.0 {
                self.phase = Phase::Playing;
            }
        }
        if self.def.max_playback.is_some_and(|m| self.elapsed >= m) {
            self.wanted = false;
        }
        if !self.wanted && self.elapsed >= self.def.min_playback {
            self.phase = Phase::FadingOut;
            self.fade_from = self.gain;
        }
        false
    }
}

/// What a frame changed, for the voice and the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Music `music` started its take `take`.
    Started { music: usize, take: usize },
    /// Music `music` is silent again.
    Stopped { music: usize },
}

/// Every Music of a battle (RE 3.9): one plays at a time.
#[derive(Debug, Clone, Default)]
pub struct Player {
    pub musics: Vec<Music>,
}

impl Player {
    pub fn new(defs: Vec<MusicDef>) -> Player {
        Player {
            musics: defs.into_iter().map(Music::new).collect(),
        }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.musics
            .iter()
            .position(|m| m.def.name.eq_ignore_ascii_case(name))
    }

    /// The Music that sounds, with its gain.
    pub fn current(&self) -> Option<(usize, f32)> {
        self.musics
            .iter()
            .position(Music::playing)
            .map(|i| (i, self.musics[i].gain))
    }

    /// Every sound stopped (the battle's end): the playing Musics are cut,
    /// and no Music stays asked. PROVISIONAL for a Music that was asked and
    /// waiting out its inactive time: the PS2's manager stops asking at the
    /// game's end, whether its last ask survives the stop is not read; here
    /// it does not, so the ambient music cannot come back over the end.
    pub fn stop_all(&mut self) -> Vec<Change> {
        let mut out = Vec::new();
        for (i, m) in self.musics.iter_mut().enumerate() {
            m.wanted = false;
            if m.playing() {
                m.cut();
                out.push(Change::Stopped { music: i });
            }
        }
        out
    }

    /// One frame: every Music's own update, then the asked ones by priority.
    pub fn update(&mut self, dt: f32) -> Vec<Change> {
        let mut out = Vec::new();
        for (i, m) in self.musics.iter_mut().enumerate() {
            if m.advance(dt) {
                out.push(Change::Stopped { music: i });
            }
        }
        // The asked Musics sorted by Priority + 0.0001, 0 while the inactive
        // time has not passed; the list's order breaks a tie.
        let key = |m: &Music| {
            if m.rested() || m.playing() {
                m.def.priority + 0.0001
            } else {
                0.0
            }
        };
        let mut first: Option<usize> = None;
        for (i, m) in self.musics.iter().enumerate() {
            if m.wanted && first.is_none_or(|f| key(m) > key(&self.musics[f])) {
                first = Some(i);
            }
        }
        let Some(first) = first else {
            return out;
        };
        // Every other asked Music that plays is told to stop.
        for (i, m) in self.musics.iter_mut().enumerate() {
            if i != first && m.wanted && m.playing() {
                m.wanted = false;
            }
        }
        // Only the first may start, and only while no Music plays.
        if !self.musics.iter().any(Music::playing) && self.musics[first].rested() {
            let m = &mut self.musics[first];
            m.start();
            out.push(Change::Started {
                music: first,
                take: m.take,
            });
        }
        out
    }
}

/// An entry of the manager's table (RE 3.6): a Music and whether the entry
/// has asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub music: usize,
    asked: bool,
}

impl Entry {
    pub fn new(music: usize) -> Entry {
        Entry {
            music,
            asked: false,
        }
    }

    /// Asks the Music when the entry is idle or the Music has stopped.
    pub fn play(&mut self, p: &mut Player) {
        let m = &mut p.musics[self.music];
        if !self.asked || m.phase == Phase::Stopped {
            m.wanted = true;
            self.asked = true;
        }
    }

    pub fn stop(&mut self, p: &mut Player) {
        p.musics[self.music].wanted = false;
        self.asked = false;
    }
}

/// The VoiceOverManager's music: `SetAmbientMusic`'s table per team and
/// state, `SetVictoryMusic` / `SetDefeatMusic` per team, and the update.
#[derive(Debug, Clone)]
pub struct Ambient {
    pub rule: Rule,
    pub thr: [[i32; 3]; 2],
    pub entries: [[Option<Entry>; 3]; 2],
    pub victory: [Option<Entry>; 2],
    pub defeat: [Option<Entry>; 2],
    /// 0 until a human-controlled unit spawns (RE 3.5).
    pub enabled: bool,
    pub over: bool,
    listener: Option<usize>,
    /// The state the last update asked for.
    pub state: Option<u8>,
}

impl Ambient {
    pub fn new(rule: Rule) -> Ambient {
        Ambient {
            rule,
            // A team with no `SetAmbientMusic` has no state: every count is
            // above the thresholds.
            thr: [[i32::MIN; 3]; 2],
            entries: [[None; 3]; 2],
            victory: [None; 2],
            defeat: [None; 2],
            enabled: false,
            over: false,
            listener: None,
            state: None,
        }
    }

    /// `SetAmbientMusic(team, x, name, state, flag)`, `team` 0-based and
    /// `tickets` the team's reinforcements at the call.
    pub fn set(&mut self, team: usize, state: usize, music: Option<usize>, thr: i32) {
        if team < 2 && state < 3 {
            self.thr[team][state] = thr;
            self.entries[team][state] = music.map(Entry::new);
        }
    }

    /// The frame's update (RE 3.5): nothing before the first human spawn or
    /// after the battle's end.
    pub fn update(&mut self, p: &mut Player, listener: usize, tickets: [i32; 2]) {
        if !self.enabled || self.over || listener >= 2 {
            return;
        }
        let s = usize::from(self.rule.state(tickets, self.thr));
        self.state = Some(s as u8);
        if let Some(e) = &mut self.entries[listener][s] {
            e.play(p);
        }
        if let Some(old) = self.listener.filter(|o| *o != listener) {
            if let Some(e) = &mut self.entries[old][s] {
                e.stop(p);
            }
        }
        self.listener = Some(listener);
        if s >= 1 {
            for t in 0..2 {
                if let Some(e) = &mut self.entries[t][s - 1] {
                    e.stop(p);
                }
            }
        }
    }

    /// The battle's end (RE 3.13), once: every sound stopped, then the
    /// listener team's victory or defeat Music.
    pub fn finish(&mut self, p: &mut Player, listener: usize, won: bool) -> Vec<Change> {
        if self.over {
            return Vec::new();
        }
        self.over = true;
        let out = p.stop_all();
        let table = if won {
            &mut self.victory
        } else {
            &mut self.defeat
        };
        if let Some(e) = table.get_mut(listener).and_then(Option::as_mut) {
            e.play(p);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (game, state, tickets, thresholds, asked team, asked state)
    type Row = (String, String, [i32; 2], [[i32; 3]; 2], usize, u8);

    /// The PS2's RAM in every reference state with the manager enabled.
    fn ram_rows() -> Vec<Row> {
        include_str!("../tests/data/music_ram.txt")
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                let n = |i: usize| f[i].parse::<i32>().unwrap();
                (
                    f[0].to_owned(),
                    f[1].to_owned(),
                    [n(2), n(3)],
                    [[n(4), n(5), n(6)], [n(7), n(8), n(9)]],
                    n(10) as usize,
                    n(11) as u8,
                )
            })
            .collect()
    }

    fn misses(rows: &[Row], game: &str, rule: Rule) -> Vec<String> {
        rows.iter()
            .filter(|r| r.0 == game && rule.state(r.2, r.3) != r.5)
            .map(|r| r.1.clone())
            .collect()
    }

    #[test]
    fn the_state_rules_give_the_ps2s_asked_state_in_every_ram_row() {
        let rows = ram_rows();
        let count = |g: &str| rows.iter().filter(|r| r.0 == g).count();
        assert_eq!((count("bf1"), count("bf2")), (105, 122));
        assert_eq!(misses(&rows, "bf1", Rule::Bf1), Vec::<String>::new());
        assert_eq!(misses(&rows, "bf2", Rule::Bf2), Vec::<String>::new());
        // The one state that separates the rules: Yavin, 134 / 119 against
        // 150 / 120 / 30 - team 2 is in `middle`, the PS2 plays `start`.
        let yav = rows
            .iter()
            .find(|r| r.1 == "bf2_yav1_cp3_landingzone_post1_spawn")
            .unwrap();
        assert_eq!((yav.2, yav.5), ([134, 119], 0));
        assert_eq!(state_bf2(yav.2, yav.3), 0);
        assert_eq!(state_bf1(yav.2, yav.3), 1);
        // Swapping the rules fails exactly that row.
        assert_eq!(
            misses(&rows, "bf2", Rule::Bf1),
            vec!["bf2_yav1_cp3_landingzone_post1_spawn".to_owned()]
        );
    }

    #[test]
    fn thresholds_are_the_ps2s() {
        // RAM: Naboo 200 / 198 / 20 (1.0 / 0.99 / 0.1 of 200), Utapau 150 /
        // 120 / 30 (1.0 / 0.8 / 0.2 of 150).
        let t = |x: f32, n: i32| threshold(x, true, n);
        assert_eq!([t(1.0, 200), t(0.99, 200), t(0.1, 200)], [200, 198, 20]);
        assert_eq!([t(1.0, 150), t(0.8, 150), t(0.2, 150)], [150, 120, 30]);
        assert_eq!(threshold(40.0, false, 150), 40);
    }

    fn def(name: &str, inactive: f32, takes: &[f32]) -> MusicDef {
        MusicDef {
            name: name.to_owned(),
            fade_in: 1.0,
            fade_out: 1.0,
            min_playback: 5.0,
            min_inactive: inactive,
            max_playback: None,
            priority: 0.0,
            looping: false,
            takes: takes.to_vec(),
        }
    }

    const DT: f32 = 1.0 / 60.0;

    /// BF1 Naboo's four `cis_nab_amb_middle` takes in the property's list
    /// order (82338421, B0338D63, AF338BD0, B63396D5), by the disc's stream
    /// index.
    const MIDDLE: [f32; 4] = [28.30, 26.66, 26.12, 31.29];

    #[test]
    fn naboo_middle_takes_start_where_the_ps2s_did() {
        // PS2 (`sndc/bf1_nab1_run2.csv`, RE 3.12): take AF338BD0 (the list's
        // third) started at clock 176.61, B63396D5 at 233.024 and 82338421
        // (the list's first: the order wraps) at 294.586.
        let mut p = Player::new(vec![def("cis_nab_amb_middle", 30.0, &MIDDLE)]);
        p.musics[0].take = 2;
        let mut e = Entry::new(0);
        let mut starts = Vec::new();
        let mut t = 176.61f32;
        for _ in 0..(140.0 / DT) as usize {
            e.play(&mut p);
            for c in p.update(DT) {
                if let Change::Started { take, .. } = c {
                    starts.push((t, take));
                }
            }
            t += DT;
        }
        let ps2 = [(176.61f32, 2), (233.024, 3), (294.586, 0)];
        assert_eq!(starts.len(), 3, "{starts:?}");
        for (k, ((at, take), (want, want_take))) in starts.iter().zip(ps2).enumerate() {
            assert!((at - want).abs() < 0.5, "start {k} at {at}, PS2 {want}");
            assert_eq!(*take, want_take, "the takes follow the list order");
        }
        // Tighter than the card asks: within 0.1 s with the measured lag.
        assert!((starts[2].0 - 294.586).abs() < 0.1, "{starts:?}");
    }

    #[test]
    fn a_stop_waits_for_min_playback_and_bars_a_restart_for_min_inactive() {
        let mut p = Player::new(vec![def("a", 30.0, &[53.9])]);
        let mut e = Entry::new(0);
        e.play(&mut p);
        p.update(DT);
        assert_eq!(p.musics[0].phase, Phase::FadingIn);
        e.stop(&mut p);
        let mut t = DT;
        while p.musics[0].playing() {
            p.update(DT);
            t += DT;
            assert!(t < 10.0);
        }
        // Held 5 s, then a 1 s fade.
        assert!((t - 6.0).abs() < 0.05, "stopped at {t}");
        // Asked again at once: silent for MinInactiveTime.
        let stopped = t;
        e.play(&mut p);
        while !p.musics[0].playing() {
            e.play(&mut p);
            p.update(DT);
            t += DT;
            assert!(t < 60.0);
        }
        assert!((t - stopped - 30.0 - RESTART_LAG).abs() < 0.05);
    }

    #[test]
    fn a_bf2_take_loops_until_max_playback() {
        let mut d = def("cis_uta_amb_start", 15.0, &[110.0, 120.0]);
        d.fade_in = 0.0;
        d.fade_out = 1.5;
        d.min_playback = 20.0;
        d.max_playback = Some(180.0);
        d.looping = true;
        let mut p = Player::new(vec![d]);
        let mut e = Entry::new(0);
        let (mut t, mut log) = (0.0f32, Vec::new());
        for _ in 0..(370.0 / DT) as usize {
            e.play(&mut p);
            for c in p.update(DT) {
                log.push((t, c));
            }
            t += DT;
        }
        // 180 s of the first take (looped past its 110 s), a 1.5 s fade,
        // 15 s of silence, the second take.
        assert_eq!(log.len(), 3, "{log:?}");
        assert_eq!(log[0].1, Change::Started { music: 0, take: 0 });
        assert!((log[1].0 - 181.5).abs() < 0.1, "{log:?}");
        assert_eq!(log[2].1, Change::Started { music: 0, take: 1 });
        assert!((log[2].0 - 181.5 - 15.0 - RESTART_LAG).abs() < 0.1);
    }

    /// A battle's table: three ambient Musics per team, victory and defeat.
    fn battle(rule: Rule) -> (Player, Ambient) {
        let mut defs = Vec::new();
        for team in ["rep", "cis"] {
            for (s, inactive) in [("start", 90.0), ("middle", 30.0), ("end", 30.0)] {
                defs.push(def(&format!("{team}_amb_{s}"), inactive, &[20.0]));
            }
            for s in ["victory", "defeat"] {
                let mut d = def(&format!("{team}_amb_{s}"), 60.0, &[8.4]);
                d.fade_in = 0.0;
                d.priority = 1.0;
                defs.push(d);
            }
        }
        let p = Player::new(defs);
        let mut a = Ambient::new(rule);
        for (t, team) in ["rep", "cis"].iter().enumerate() {
            for (s, (name, x)) in [("start", 1.0), ("middle", 0.99), ("end", 0.1)]
                .iter()
                .enumerate()
            {
                let m = p.find(&format!("{team}_amb_{name}"));
                a.set(t, s, m, threshold(*x, true, 200));
            }
            a.victory[t] = p.find(&format!("{team}_amb_victory")).map(Entry::new);
            a.defeat[t] = p.find(&format!("{team}_amb_defeat")).map(Entry::new);
        }
        (p, a)
    }

    fn run(p: &mut Player, a: &mut Ambient, tickets: [i32; 2], secs: f32) -> Vec<String> {
        let mut out = Vec::new();
        for _ in 0..(secs / DT) as usize {
            a.update(p, 1, tickets);
            for c in p.update(DT) {
                if let Change::Started { music, .. } = c {
                    out.push(p.musics[music].def.name.clone());
                }
            }
        }
        out
    }

    #[test]
    fn a_battle_plays_start_middle_end_then_the_listeners_victory() {
        let (mut p, mut a) = battle(Rule::Bf1);
        // Nothing before the first spawn.
        assert!(run(&mut p, &mut a, [200, 200], 5.0).is_empty());
        a.enabled = true;
        assert_eq!(run(&mut p, &mut a, [200, 200], 3.0), ["cis_amb_start"]);
        // Team 2 at 198: `middle`. `start` is held to 5 s, fades 1 s, then
        // `middle` starts (its first start has no inactive time to wait).
        assert_eq!(run(&mut p, &mut a, [200, 198], 6.0), ["cis_amb_middle"]);
        assert_eq!(run(&mut p, &mut a, [150, 20], 10.0), ["cis_amb_end"]);
        // The end: cut, and the listener (team 2, the loser) hears defeat.
        let cut = a.finish(&mut p, 1, false);
        assert_eq!(cut.len(), 1);
        assert_eq!(run(&mut p, &mut a, [150, 0], 30.0), ["cis_amb_defeat"]);
        // Once: the take is over and nothing asks again.
        assert_eq!(p.current(), None);
    }

    #[test]
    fn an_ambient_music_in_its_gap_does_not_come_back_after_the_end() {
        let (mut p, mut a) = battle(Rule::Bf1);
        a.enabled = true;
        // `middle`'s 20 s take ends at about 26 s; the end comes in its gap.
        assert_eq!(run(&mut p, &mut a, [200, 198], 30.0), ["cis_amb_middle"]);
        assert_eq!(p.current(), None);
        assert!(a.finish(&mut p, 1, true).is_empty());
        assert_eq!(run(&mut p, &mut a, [0, 198], 120.0), ["cis_amb_victory"]);
    }

    #[test]
    fn the_two_games_differ_when_team_2_is_ahead_in_state() {
        for (rule, want) in [(Rule::Bf1, "cis_amb_middle"), (Rule::Bf2, "cis_amb_start")] {
            let (mut p, mut a) = battle(rule);
            a.enabled = true;
            assert_eq!(run(&mut p, &mut a, [200, 190], 1.0), [want], "{rule:?}");
        }
    }

    #[test]
    fn a_listener_change_stops_the_old_teams_music() {
        let (mut p, mut a) = battle(Rule::Bf2);
        a.enabled = true;
        assert_eq!(run(&mut p, &mut a, [200, 200], 8.0), ["cis_amb_start"]);
        let mut names = Vec::new();
        for _ in 0..(8.0 / DT) as usize {
            a.update(&mut p, 0, [200, 200]);
            for c in p.update(DT) {
                if let Change::Started { music, .. } = c {
                    names.push(p.musics[music].def.name.clone());
                }
            }
        }
        assert_eq!(names, ["rep_amb_start"]);
    }
}
