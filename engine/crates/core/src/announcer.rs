//! The commander's voice-overs (card ANNC; `analysis/re-announcer.md`, cited
//! below as RE §n). Pure rules, no audio: [`Announcer`] turns the battle's
//! state into requests for a named line, [`VoiceQueue`] is the one voice
//! stream those requests wait on. `game::announce` feeds both from the sim and
//! plays what the queue starts.
//!
//! The PS2 has no announcer object (RE §0): the post tick, the
//! VoiceOverManager's update and the out-of-bounds timer each ask the sound
//! handle for a 2-D stream line with the QUEUE flag. What is here:
//! - T1/T2, a post captured / neutralised: the post's own table
//!   `[listener faction][subject faction][event]` ([`PostLines`], RE §2.1);
//! - T4, bleeding: at a team's bleed start, again every 30 s for the
//!   listener's own team only (RE §2.2);
//! - T5, low reinforcements: once per battle, the first team at or below its
//!   threshold, team 0 checked first (RE §2.3);
//! - the gates: nothing before the local player's first spawn, nothing asked
//!   for once the battle is over (RE §2.0);
//! - the queue: one line at a time, at most 4 pending in descending key
//!   order, a commander line ahead of pending chatter and never cutting a line
//!   off (RE §1).
//!
//! Not here: T6 leaving the battlefield (card BNDY — the mission's names are
//! carried, nothing asks for them), T7 (BF1's no-post repeat of the T5 line),
//! T8 (BF2's `common_objComplete`; its script condition is not read, RE §8),
//! hero lines (card HERO). The port plays BF2's bleed on every map, so T4
//! follows that rate on BF1 maps too. On BF2 maps the rate the manager reads
//! is the conquest script's own ([`ScriptBleed`], card ANNC2), which is not
//! the sim's drain during a last stand.

use crate::fmath::FloatExt;

/// The PS2's faction order, 1-based on the disc: All, Imp, Rep, CIS (RE §2.1).
pub const FACTIONS: [&str; 4] = ["all", "imp", "rep", "cis"];
/// The factions as a post's `VO_<L>_<S><Event>` property spells them.
pub const FACTION_KEYS: [&str; 4] = ["All", "Imp", "Rep", "CIS"];

/// Index into [`FACTIONS`] of a mission faction key (`all|imp|rep|cis`).
pub fn faction_index(name: &str) -> Option<usize> {
    FACTIONS.iter().position(|f| f.eq_ignore_ascii_case(name))
}

/// A post's line entry, in the PS2's table order (RE §2.1: events 4-7 map to
/// entries 0-3, entry 4 is asked for by the post's own tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostEvent {
    /// Neutral -> owned; the subject is the NEW owner.
    Capture = 0,
    /// Owned -> neutral; the subject is the OLD owner.
    Lost = 1,
    InDispute = 2,
    Saved = 3,
    Info = 4,
}

impl PostEvent {
    pub const ALL: [PostEvent; 5] = [
        PostEvent::Capture,
        PostEvent::Lost,
        PostEvent::InDispute,
        PostEvent::Saved,
        PostEvent::Info,
    ];

    /// The property suffix: `VO_<L>_<S>Capture`, ...
    pub fn key(self) -> &'static str {
        match self {
            PostEvent::Capture => "Capture",
            PostEvent::Lost => "Lost",
            PostEvent::InDispute => "InDispute",
            PostEvent::Saved => "Saved",
            PostEvent::Info => "Info",
        }
    }
}

/// The instance property naming a post's line: `VO_<Listener>_<Subject><Event>`.
pub fn post_property(listener: usize, subject: usize, event: PostEvent) -> String {
    format!(
        "VO_{}_{}{}",
        FACTION_KEYS[listener],
        FACTION_KEYS[subject],
        event.key()
    )
}

/// One post's VO table as the world file sets it. A missing entry is silence:
/// only 10 retail capture posts set `VO_<L>_<enemy>Lost` (BF1 bes2 cp5, geo1
/// cp6, yav1's temple), so on every other post nothing is said when the
/// enemy's post is neutralised (RE §2.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PostLines {
    entries: Vec<(u8, u8, PostEvent, String)>,
}

impl PostLines {
    pub fn set(&mut self, listener: usize, subject: usize, event: PostEvent, line: &str) {
        self.entries
            .push((listener as u8, subject as u8, event, line.to_owned()));
    }

    pub fn get(&self, listener: usize, subject: usize, event: PostEvent) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| usize::from(e.0) == listener && usize::from(e.1) == subject && e.2 == event)
            .map(|e| e.3.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry: (listener faction, subject faction, event, line).
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, PostEvent, &str)> {
        self.entries
            .iter()
            .map(|e| (usize::from(e.0), usize::from(e.1), e.2, e.3.as_str()))
    }
}

/// A commander line's queue key. BF2's template sets Bias 1, Priority 1, so
/// the key IS the priority (RE §1.5-1.6 [D]). BF1's is 0.5 + 0.5 x the
/// voiceover bus gain, not read from RAM: PROVISIONAL at a bus gain of 1 —
/// live, every BF1 commander line played next, as this key orders it.
pub const COMMANDER_KEY: f32 = 1.0;
/// The queue holds at most this many pending lines (RE §1.3: `sltiu v0,v0,5`).
pub const MAX_PENDING: usize = 4;

/// Seconds the voice stays busy past the end of a line's take. Measured, not
/// read from code: over 11 lines of the BF2 UTA1 session
/// (`lead_tmp/annc/run4_bf2_uta1.log`) the PS2's stream slot went from
/// "started" to "stopped" (or to the next queued line) 0.29-0.68 s later than
/// the take is long — the stream's start-up and its tail — so two queued
/// lines are that far apart. PROVISIONAL: the mean; the PS2's varies per line.
pub const VOICE_GAP_SECS: f32 = 0.45;

/// A request for a line on the voice stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The sound property's name.
    pub name: String,
    /// `level * (1 - Bias) + Bias * Priority` (RE §1.5).
    pub key: f32,
    /// A Mode3D property (unit chatter); a commander line is 2-D.
    pub mode_3d: bool,
    /// The take drawn when the line was asked for (a squad answer, card
    /// SQACK); None = the weighted pick when it starts.
    pub take: Option<u8>,
    /// Where a 3-D line is said.
    pub at: Option<glam::Vec3>,
    /// Card UTALK: the unit whose own sound handle holds this line (a
    /// `LowHealthSound`, asked again only when its handle is idle:
    /// [`VoiceQueue::holds`]); None for every other line.
    pub speaker: Option<u32>,
}

impl Line {
    pub fn commander(name: &str) -> Line {
        Line {
            name: name.to_owned(),
            key: COMMANDER_KEY,
            mode_3d: false,
            take: None,
            at: None,
            speaker: None,
        }
    }
}

/// The battle's one voice slot: one line at a time, the rest waiting in key
/// order (RE §1.3-1.4).
#[derive(Debug, Clone, Default)]
pub struct VoiceQueue {
    /// Seconds left of the line now playing; 0 = the voice is idle.
    remaining: f32,
    pending: Vec<Line>,
    /// The [`Line::speaker`] of the line now playing.
    speaker: Option<u32>,
}

impl VoiceQueue {
    /// Enqueue: a key <= 0 is nothing; the entry goes in descending key order
    /// behind equal keys; a 5th drops the tail — and when the tail is the new
    /// entry itself, a 2-D line takes the place of the entry before it while a
    /// 3-D line is dropped (RE §1.3).
    pub fn ask(&mut self, line: Line) {
        if line.key <= 0.0 {
            return;
        }
        let at = self
            .pending
            .iter()
            .position(|p| p.key < line.key)
            .unwrap_or(self.pending.len());
        if at < MAX_PENDING {
            self.pending.insert(at, line);
            self.pending.truncate(MAX_PENDING);
        } else if !line.mode_3d {
            self.pending[MAX_PENDING - 1] = line;
        }
    }

    /// A line of this name is waiting to start.
    pub fn waiting(&self, name: &str) -> bool {
        self.pending.iter().any(|l| l.name == name)
    }

    /// One frame: the playing line runs down by `dt`; when the voice is idle
    /// the head of the queue starts. `start` plays the line and returns its
    /// length in seconds, or None when there is no take to play (the next
    /// pending line is tried at once). The voice is then busy for that length
    /// and [`VOICE_GAP_SECS`]. Returns the line that started.
    pub fn step(&mut self, dt: f32, mut start: impl FnMut(&Line) -> Option<f32>) -> Option<Line> {
        self.remaining = (self.remaining - dt).fmax(0.0);
        if self.remaining <= 0.0 {
            self.speaker = None;
        }
        while self.remaining <= 0.0 && !self.pending.is_empty() {
            let line = self.pending.remove(0);
            if let Some(secs) = start(&line) {
                self.remaining = secs.fmax(0.0) + VOICE_GAP_SECS;
                self.speaker = line.speaker;
                return Some(line);
            }
        }
        None
    }

    /// Card UTALK: does `unit`'s own handle hold a line - one of its
    /// [`Line::speaker`] lines waiting or playing?
    pub fn holds(&self, unit: u32) -> bool {
        self.speaker == Some(unit) || self.pending.iter().any(|l| l.speaker == Some(unit))
    }

    /// Card UTALK: `unit`'s handle is stopped (it died, or its health rose
    /// past the threshold): its waiting line is dropped. One already playing
    /// plays out - PROVISIONAL (the PS2 stops it; the port's mixer cannot
    /// stop one queued line's voice yet).
    pub fn release(&mut self, unit: u32) {
        self.pending.retain(|l| l.speaker != Some(unit));
        if self.speaker == Some(unit) {
            self.speaker = None;
        }
    }

    /// A line started outside the queue's own pick (tests; chatter that is
    /// already playing): the voice is busy for `secs`.
    pub fn playing(&mut self, secs: f32) {
        self.remaining = secs.fmax(0.0);
    }

    pub fn is_idle(&self) -> bool {
        self.remaining <= 0.0
    }

    pub fn pending(&self) -> &[Line] {
        &self.pending
    }
}

/// The VoiceOverManager's bleed repeat time (RE §2.2: ctor 30.0, `lui 0x41f0`;
/// no script on either disc changes it).
pub const BLEED_REPEAT_SECS: f32 = 30.0;
/// The low-reinforcement threshold a mission that never calls the setter is
/// left with (RE §2.0: ctor 20). Such a mission has no line either, so the
/// latch closes in silence.
pub const LOW_THRESHOLD_DEFAULT: u32 = 20;

/// The mission script's lines, by port team index (Lua team - 1):
/// `[listener team][subject team]`.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleLines {
    /// `SetBleedingVoiceOver(L, T, name, 1)` — table A of RE §2.2 (every
    /// retail call passes 1; table B stays empty and is not modelled).
    pub bleeding: [[Option<String>; 2]; 2],
    /// `SetLowReinforcementsVoiceOver(L, T, name, x, flag)`.
    pub low: [[Option<String>; 2]; 2],
    /// Per team: the ticket count at or below which its line is due.
    pub low_threshold: [u32; 2],
}

impl Default for BattleLines {
    fn default() -> Self {
        BattleLines {
            bleeding: Default::default(),
            low: Default::default(),
            low_threshold: [LOW_THRESHOLD_DEFAULT; 2],
        }
    }
}

impl BattleLines {
    /// The setter's threshold (RE §2.3): `(int)(x * tickets of team L at the
    /// call)` when the 5th argument is set, else `(int)x`.
    pub fn set_low(
        &mut self,
        listener: usize,
        team: usize,
        line: &str,
        x: f32,
        scaled: bool,
        listener_tickets: u32,
    ) {
        self.low[listener][team] = Some(line.to_owned());
        self.low_threshold[listener] = if scaled {
            (x * listener_tickets as f32) as u32
        } else {
            x as u32
        };
    }
}

/// The bleed rate BF2's conquest script hands to `SetBleedRate`, which is the
/// rate the VoiceOverManager reads (card ANNC2; `objectiveconquest.luac`
/// functions 7.0 / 7.2 / 7.3.0, live `lead_tmp/audit-annc/live1.csv`). At
/// Start and at every finished capture / neutralisation the script sets each
/// team's rate to the UNCLAMPED rule and starts a timer of 1.0 that runs down
/// at that rate; at each elapse a team with `alive < reinforcements` loses a
/// ticket and the timer gains 1.0, any other team's rate is set to 0 and its
/// timer stops — until the next post event. So a team on its last stand
/// bleeds for 3 s after every post event, and the bleeding line is asked for
/// again each time. BF1 has no such script: its engine gates the rate on
/// `alive < tickets` every update, which is the sim's `bleed_per_sec`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptBleed {
    rate: [f32; 2],
    timer: [Option<f32>; 2],
}

impl ScriptBleed {
    /// The script's Start: `rates` are the unclamped rule's, per team.
    pub fn new(rates: [f32; 2]) -> ScriptBleed {
        let mut s = ScriptBleed::default();
        s.post_event(rates);
        s
    }

    /// A capture or neutralisation finished: both teams' rates are set anew
    /// and their timers restarted.
    pub fn post_event(&mut self, rates: [f32; 2]) {
        self.rate = rates;
        self.timer = rates.map(|r| (r > 0.0).then_some(1.0));
    }

    /// `dt` seconds of the script's timers; returns the rates the manager
    /// reads this frame.
    pub fn step(&mut self, dt: f32, alive: [u32; 2], tickets: [u32; 2]) -> [f32; 2] {
        for t in 0..2 {
            let Some(left) = self.timer[t] else { continue };
            let left = left - self.rate[t] * dt;
            self.timer[t] = if left > 0.0 {
                Some(left)
            } else if alive[t] < tickets[t] {
                Some(left + 1.0)
            } else {
                self.rate[t] = 0.0;
                None
            };
        }
        self.rate
    }

    pub fn rates(&self) -> [f32; 2] {
        self.rate
    }
}

/// The VoiceOverManager's rules (RE §2.0-2.3).
#[derive(Debug, Clone)]
pub struct Announcer {
    pub lines: BattleLines,
    /// The listener's team index (the local player's team).
    listener: usize,
    /// Each team's index into [`FACTIONS`]; None for a team with no commander
    /// (a mod faction, the built-in troopers).
    factions: [Option<usize>; 2],
    /// Set at the local player's first spawn, never cleared.
    enabled: bool,
    over: bool,
    bleed_timer: [f32; 2],
    bleeding: [bool; 2],
    low_said: bool,
}

impl Announcer {
    pub fn new(lines: BattleLines, listener: usize, factions: [Option<usize>; 2]) -> Self {
        Announcer {
            lines,
            listener: listener.min(1),
            factions,
            enabled: false,
            over: false,
            bleed_timer: [0.0; 2],
            bleeding: [false; 2],
            low_said: false,
        }
    }

    pub fn listener(&self) -> usize {
        self.listener
    }

    pub fn set_listener(&mut self, team: usize) {
        self.listener = team.min(1);
    }

    /// The local player's unit spawned (RE §2.0: the manager's enabled flag).
    pub fn player_spawned(&mut self) {
        self.enabled = true;
    }

    /// The battle ended: the update, the post line and both play helpers
    /// return while the game-over flag is set (RE §2.0).
    pub fn battle_over(&mut self) {
        self.over = true;
    }

    pub fn speaks(&self) -> bool {
        self.enabled && !self.over
    }

    /// T1/T2: the line of `post` for `event` about `subject_team` (the new
    /// owner of a capture, the old owner of a neutralisation).
    pub fn post(&self, post: &PostLines, event: PostEvent, subject_team: usize) -> Option<Line> {
        if !self.speaks() {
            return None;
        }
        let l = self.factions[self.listener]?;
        let s = (*self.factions.get(subject_team)?)?;
        post.get(l, s, event).map(Line::commander)
    }

    /// One frame of the manager's update: `bleed` is each team's bleed rate
    /// (tickets per second), `tickets` its whole reinforcements. Returns the
    /// lines asked for, in the PS2's order: bleeding team 0, team 1, then low
    /// reinforcements.
    pub fn update(&mut self, dt: f32, bleed: [f32; 2], tickets: [u32; 2]) -> Vec<Line> {
        let mut out = Vec::new();
        if !self.speaks() {
            return out;
        }
        // Both teams' latches move before either line is picked (RE §2.2).
        let mut started = [false; 2];
        let mut repeat = [false; 2];
        for i in 0..2 {
            if bleed[i] <= 0.0 {
                if self.bleeding[i] {
                    self.bleeding[i] = false;
                    self.bleed_timer[i] = 0.0;
                }
                continue;
            }
            self.bleed_timer[i] += dt;
            if !self.bleeding[i] {
                self.bleeding[i] = true;
                started[i] = true;
                self.bleed_timer[i] = 0.0;
            } else if self.bleed_timer[i] > BLEED_REPEAT_SECS {
                repeat[i] = true;
                self.bleed_timer[i] = 0.0;
            }
        }
        let l = self.listener;
        for i in 0..2 {
            // The enemy's line needs the listener's own team not bleeding:
            // otherwise the PS2 reads its empty second table.
            if (started[i] || (repeat[i] && i == l)) && (i == l || !self.bleeding[l]) {
                out.extend(self.lines.bleeding[l][i].as_deref().map(Line::commander));
            }
        }
        // One latch for both teams: the first to reach its threshold speaks.
        for (i, &left) in tickets.iter().enumerate() {
            if !self.low_said && left <= self.lines.low_threshold[i] {
                self.low_said = true;
                out.extend(self.lines.low[l][i].as_deref().map(Line::commander));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REP: usize = 2;
    const CIS: usize = 3;
    const DT: f32 = 1.0 / 30.0;

    /// A BF2 post's generic table (RE §2.1: three properties per listener).
    fn bf2_post() -> PostLines {
        let mut p = PostLines::default();
        for (l, e, side) in [(CIS, REP, "cis"), (REP, CIS, "rep")] {
            let n = |s: &str| format!("{side}_off_com_report_{s}_commandpost");
            p.set(l, l, PostEvent::Capture, &n("captured"));
            p.set(l, l, PostEvent::Lost, &n("lost"));
            p.set(l, e, PostEvent::Capture, &n("enemyCaptured"));
        }
        p
    }

    /// BF2 UTA1 Clone Wars: CIS is team 0 (Lua team 1), 150 tickets a side.
    fn uta1(listener: usize) -> Announcer {
        let mut lines = BattleLines::default();
        for (l, side) in ["cis", "rep"].iter().enumerate() {
            for t in 0..2 {
                let (b, r) = if l == t {
                    ("us_overwhelmed", "defeat_im")
                } else {
                    ("enemy_losing", "victory_im")
                };
                lines.bleeding[l][t] = Some(format!("{side}_off_com_report_{b}"));
                lines.set_low(l, t, &format!("{side}_off_{r}"), 0.1, true, 150);
            }
        }
        let mut a = Announcer::new(lines, listener, [Some(CIS), Some(REP)]);
        a.player_spawned();
        a
    }

    fn names(lines: &[Line]) -> Vec<&str> {
        lines.iter().map(|l| l.name.as_str()).collect()
    }

    /// Run `secs` of updates; every line with the time it was asked for.
    fn run(a: &mut Announcer, secs: f32, bleed: [f32; 2], t0: f32) -> Vec<(f32, String)> {
        let mut out = Vec::new();
        for f in 0..(secs / DT).round() as u32 {
            for l in a.update(DT, bleed, [150; 2]) {
                out.push((t0 + f as f32 * DT, l.name));
            }
        }
        out
    }

    /// Acceptance 1 (RE §2.1): the names come from the post's own table; the
    /// enemy's post going neutral says nothing.
    #[test]
    fn cp_lines() {
        let a = uta1(0);
        let p = bf2_post();
        let say = |e, team| a.post(&p, e, team).map(|l| l.name);
        assert_eq!(
            say(PostEvent::Capture, 1).as_deref(),
            Some("cis_off_com_report_enemyCaptured_commandpost")
        );
        assert_eq!(
            say(PostEvent::Capture, 0).as_deref(),
            Some("cis_off_com_report_captured_commandpost")
        );
        assert_eq!(
            say(PostEvent::Lost, 0).as_deref(),
            Some("cis_off_com_report_lost_commandpost")
        );
        assert_eq!(say(PostEvent::Lost, 1), None);
        // A post with no table (BF2 SPA6) is silent, whatever the event.
        assert_eq!(a.post(&PostLines::default(), PostEvent::Capture, 0), None);
        assert_eq!(
            post_property(CIS, REP, PostEvent::Capture),
            "VO_CIS_RepCapture"
        );
    }

    /// Acceptance 2 (RE §2.2): own bleed at t, t+30, t+60, none once it stops;
    /// the enemy's once; the enemy's while the own team bleeds, never.
    #[test]
    fn bleed_line() {
        let own = "cis_off_com_report_us_overwhelmed";
        let enemy = "cis_off_com_report_enemy_losing";
        let third = 1.0 / 3.0;

        let mut a = uta1(0);
        let said = run(&mut a, 70.0, [third, 0.0], 0.0);
        assert_eq!(said.len(), 3, "{said:?}");
        for ((t, name), want) in said.iter().zip([0.0, 30.0, 60.0]) {
            assert_eq!(name, own);
            // The PS2's timer wraps on `> repeat`: one frame past 30 s, and
            // one more each round (30.06 s measured, RE §4 row 9).
            assert!((t - want).abs() < 0.5, "{t} vs {want}");
        }
        assert!(run(&mut a, 70.0, [0.0, 0.0], 70.0).is_empty(), "stopped");
        // A new start speaks at once: the timer was cleared with the latch.
        assert_eq!(run(&mut a, 1.0, [third, 0.0], 140.0).len(), 1);

        let mut a = uta1(0);
        let said = run(&mut a, 70.0, [0.0, third], 0.0);
        assert_eq!(said, vec![(0.0, enemy.to_owned())], "once, no repeat");

        let mut a = uta1(0);
        run(&mut a, 5.0, [third, 0.0], 0.0);
        let said = run(&mut a, 70.0, [third, third], 5.0);
        assert!(said.iter().all(|(_, n)| n == own), "{said:?}");
        // Both starting in one frame: the latches move first, so only the own
        // team's line is asked for.
        let mut a = uta1(1);
        let said = a.update(DT, [third, third], [150; 2]);
        assert_eq!(names(&said), ["rep_off_com_report_us_overwhelmed"]);
    }

    /// Acceptance 3 (RE §2.3): 10 % of the start count, once per battle, one
    /// latch for both teams; a mission without the call is silent.
    #[test]
    fn low_reinforcements_once() {
        let mut a = uta1(0);
        assert_eq!(a.lines.low_threshold, [15, 15]);
        assert!(a.update(DT, [0.0; 2], [16, 150]).is_empty());
        let said = a.update(DT, [0.0; 2], [15, 150]);
        assert_eq!(names(&said), ["cis_off_defeat_im"]);
        assert!(a.update(DT, [0.0; 2], [14, 150]).is_empty());
        assert!(
            a.update(DT, [0.0; 2], [14, 15]).is_empty(),
            "the other team"
        );

        // The enemy first: the listener hears his "victory" line.
        let mut a = uta1(0);
        let said = a.update(DT, [0.0; 2], [150, 15]);
        assert_eq!(names(&said), ["cis_off_victory_im"]);

        // BF1 NAB1 Clone Wars sets no line: the latch closes at the ctor's 20
        // and nothing is said (RE §4 row 5).
        let mut a = Announcer::new(BattleLines::default(), 0, [Some(CIS), Some(REP)]);
        a.player_spawned();
        assert!(a.update(DT, [0.0; 2], [20, 200]).is_empty());
        assert!(a.low_said);

        // 0.1f x the start count, truncated, per listener team.
        let mut l = BattleLines::default();
        l.set_low(0, 0, "x", 0.1, true, 200);
        l.set_low(1, 1, "y", 0.1, true, 225);
        l.set_low(1, 0, "z", 7.9, false, 225);
        assert_eq!(l.low_threshold, [20, 7]);
    }

    fn chatter(name: &str, key: f32) -> Line {
        Line {
            name: name.to_owned(),
            key,
            mode_3d: true,
            take: None,
            at: None,
            speaker: None,
        }
    }

    /// Acceptance 4 (RE §1.3-1.4, §4 row 11): never cut, key order, FIFO
    /// between commander lines, 4 pending.
    #[test]
    fn voice_queue() {
        // A 2.0 s chatter line is playing, two more wait; a commander line
        // asked now starts when the playing line ends, ahead of the chatter.
        let mut q = VoiceQueue::default();
        q.playing(2.0);
        q.ask(chatter("chat1", 0.6));
        q.ask(chatter("chat2", 0.6));
        q.ask(Line::commander("first"));
        q.ask(Line::commander("second"));
        let mut log = Vec::new();
        let mut t = 0.0;
        while t < 14.0 {
            t += DT;
            if let Some(l) = q.step(DT, |_| Some(1.5)) {
                log.push((t, l.name));
            }
        }
        let order: Vec<&str> = log.iter().map(|l| l.1.as_str()).collect();
        assert_eq!(order, ["first", "second", "chat1", "chat2"]);
        assert!((log[0].0 - 2.0).abs() <= 2.0 * DT, "not cut, not earlier");
        let second = 2.0 + 1.5 + VOICE_GAP_SECS;
        assert!(
            (log[1].0 - second).abs() <= 3.0 * DT,
            "after the first's 1.5 s"
        );
        assert!(q.is_idle() && q.pending().is_empty());

        // An idle voice starts the line in the frame it is asked for.
        q.ask(Line::commander("now"));
        assert_eq!(q.step(DT, |_| Some(1.0)).unwrap().name, "now");

        // 4 pending commander lines: a 5th is the new tail and, being 2-D,
        // takes the place of the entry before it.
        let mut q = VoiceQueue::default();
        q.playing(9.0);
        for n in ["a", "b", "c", "d", "e"] {
            q.ask(Line::commander(n));
        }
        assert_eq!(names(q.pending()), ["a", "b", "c", "e"]);
        // A 3-D line arriving at the tail is dropped itself.
        q.ask(chatter("late", 0.6));
        assert_eq!(names(q.pending()), ["a", "b", "c", "e"]);

        // 1 commander + 3 chatter pending: the 5th, a commander line, goes
        // behind its equal and the lowest (newest among equals) drops.
        let mut q = VoiceQueue::default();
        q.playing(9.0);
        q.ask(chatter("c1", 0.6));
        q.ask(Line::commander("a"));
        q.ask(chatter("c2", 0.6));
        q.ask(chatter("c3", 0.4));
        q.ask(Line::commander("b"));
        assert_eq!(names(q.pending()), ["a", "b", "c1", "c2"]);
        // A key <= 0 is nothing; a line with no take lets the next one start.
        q.ask(chatter("silent", 0.0));
        assert_eq!(q.pending().len(), 4);
        q.playing(0.0);
        let l = q.step(DT, |l| (l.name != "a").then_some(1.0)).unwrap();
        assert_eq!(l.name, "b");
    }

    /// Acceptance 5 (RE §2.0): nothing before the local player's first spawn,
    /// nothing asked for after the battle is over.
    #[test]
    fn gates() {
        let third = 1.0 / 3.0;
        let p = bf2_post();
        let mut a = uta1(0);
        a.enabled = false;
        assert!(!a.speaks());
        assert!(a.update(DT, [third, 0.0], [10, 150]).is_empty());
        assert_eq!(a.post(&p, PostEvent::Capture, 0), None);
        // The latches did not move while it was off: the first frame after
        // the spawn finds the bleed "starting" and the tickets low.
        a.player_spawned();
        let said = a.update(DT, [third, 0.0], [10, 150]);
        assert_eq!(
            names(&said),
            ["cis_off_com_report_us_overwhelmed", "cis_off_defeat_im"]
        );

        let mut a = uta1(0);
        a.battle_over();
        assert!(a.update(DT, [third, third], [0, 0]).is_empty());
        assert_eq!(a.post(&p, PostEvent::Capture, 0), None);

        // A team with no commander faction has no post line to pick.
        let mut a = Announcer::new(BattleLines::default(), 0, [None, Some(REP)]);
        a.player_spawned();
        assert_eq!(a.post(&p, PostEvent::Capture, 1), None);
        assert_eq!(faction_index("CIS"), Some(CIS));
        assert_eq!(faction_index("team"), None);
    }
    /// Card ANNC2, acceptance 2: the script's rate is the unclamped rule at
    /// an owner change; on a last stand (alive >= tickets) it reads 0 after
    /// 3.0 s and stays 0 until the next owner change; with alive < tickets
    /// it stays.
    #[test]
    fn last_stand_rate() {
        let third = 1.0 / 3.0;
        // Team 1 is down to 3 tickets with 5 alive; team 0 bleeds nothing.
        let mut b = ScriptBleed::new([0.0, third]);
        let mut zero_at = None;
        for f in 1..=(10.0 / DT) as u32 {
            let r = b.step(DT, [17, 5], [110, 3]);
            assert_eq!(r[0], 0.0);
            if r[1] == 0.0 && zero_at.is_none() {
                zero_at = Some(f as f32 * DT);
            }
            assert!(zero_at.is_none() || r[1] == 0.0, "it stays 0");
        }
        let at = zero_at.expect("the rate went to 0");
        println!("last stand: the rate reads 0 after {at:.3} s (PS2: 3.003 s)");
        assert!((at - 3.0).abs() <= DT + 1e-4, "{at}");
        // The next owner change sets it again, for another 3 s.
        b.post_event([0.0, third]);
        assert_eq!(b.rates(), [0.0, third]);
        for _ in 0..(2.9 / DT) as u32 {
            assert_eq!(b.step(DT, [17, 5], [110, 3])[1], third);
        }
        for _ in 0..(0.2 / DT) as u32 {
            b.step(DT, [17, 5], [110, 3]);
        }
        assert_eq!(b.rates(), [0.0, 0.0]);
        // A team with fewer alive than tickets keeps bleeding.
        let mut b = ScriptBleed::new([0.0, third]);
        for _ in 0..(60.0 / DT) as u32 {
            assert_eq!(b.step(DT, [17, 12], [110, 100]), [0.0, third]);
        }
        // An owner change that ends the majority stops it at once.
        b.post_event([0.0, 0.0]);
        assert_eq!(b.step(DT, [17, 12], [110, 100]), [0.0, 0.0]);

        // Fed to the manager, each last-stand event says the line again.
        let mut a = uta1(0);
        let mut b = ScriptBleed::new([0.0, 0.0]);
        let mut said = Vec::new();
        for f in 0..(20.0 / DT) as u32 {
            if f == 30 || f == 300 {
                b.post_event([0.0, third]);
            }
            let r = b.step(DT, [17, 5], [110, 3]);
            said.extend(a.update(DT, r, [110, 150]).into_iter().map(|l| (f, l.name)));
        }
        assert_eq!(
            said,
            vec![
                (30, "cis_off_com_report_enemy_losing".to_owned()),
                (300, "cis_off_com_report_enemy_losing".to_owned()),
            ]
        );
    }
}
