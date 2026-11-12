//! How the BF2 PS2 shell's menus sound and move (`analysis/re-bf2-menus.md`
//! §4, §5): the click a menu event asks for ([`MenuSound`], [`Cues`]), the
//! focus pulse ([`Pulse`]), the one-time "flashy" entrance of a text
//! ([`flashy`]), which shell music take plays next ([`ShellPlaylist`]) and
//! a held direction's repeat ([`HoldRepeat`]). Pure state advanced by the
//! shell's frame `dt`; the game plays the names through its sound library.
//! The numbers are the PS2's as captured (`engine/artifacts/lead_tmp/menuref/`,
//! "menuref"; re-bf2-menus "PCSX2 check 2026-10-28").

/// A menu click: the template's sound fields (`common_ifelem_shellscreen`
/// main.f30 pc 14-42, §4.1 [V]). All are CORE.LVL bank `global`, bus
/// `shellfx` (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSound {
    /// `selectSound`: a focus move.
    Select,
    /// `acceptSound`: Cross.
    Accept,
    /// `exitSound`: played by the screen Back returns to, on its re-Enter.
    Exit,
    /// `cancelSound`: a popup's "No".
    Cancel,
    /// `errorSound`: a refused action.
    Error,
    /// The name keyboard's letter and delete (`shell_ifs_vkeyboard`).
    Key,
    Backspace,
}

impl MenuSound {
    /// The sound property's name.
    pub fn name(self) -> &'static str {
        match self {
            MenuSound::Select => "shell_select_change",
            MenuSound::Accept => "shell_menu_accept",
            MenuSound::Exit => "shell_menu_exit",
            MenuSound::Cancel => "shell_menu_cancel",
            MenuSound::Error => "shell_menu_error",
            MenuSound::Key => "shell_keyboard_key",
            MenuSound::Backspace => "shell_keyboard_backspace",
        }
    }
}

/// At most one `shell_select_change` per this many seconds (its BF2
/// `playinterval`, §4.2 [V]).
pub const SELECT_INTERVAL: f32 = 0.1;

/// The clicks asked for since the game last took them, with the select
/// click's rate limit on the shell's clock.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cues {
    clock: f32,
    /// The clock time the next select click may start.
    select_ok: f32,
    queued: Vec<MenuSound>,
}

impl Cues {
    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
    }

    /// Ask for `s`; a select click inside its interval is dropped, as the
    /// PS2 refuses a start inside `playinterval`.
    pub fn play(&mut self, s: MenuSound) {
        if s == MenuSound::Select {
            // The slack absorbs f32 rounding of a summed dt.
            if self.clock + 1e-4 < self.select_ok {
                return;
            }
            self.select_ok = self.clock + SELECT_INTERVAL;
        }
        self.queued.push(s);
    }

    /// A list cursor went `from` -> `to`: a click only if it really moved
    /// (`ListManager_fnNavUp/Down` f6/f7 pc 27, §4.1 [V]: silent at an end).
    pub fn list_move(&mut self, from: usize, to: usize) {
        if from != to {
            self.play(MenuSound::Select);
        }
    }

    /// The clicks asked for since the last call, oldest first.
    pub fn take(&mut self) -> Vec<MenuSound> {
        std::mem::take(&mut self.queued)
    }

    /// Take the queue, then advance the clock: a cue queued this frame counts
    /// from when it starts, as the PS2 refuses a start inside its playinterval.
    pub fn take_then_tick(&mut self, dt: f32) -> Vec<MenuSound> {
        let got = std::mem::take(&mut self.queued);
        self.tick(dt);
        got
    }
}

/// The shell's update rate: the PS2 shell steps its animations 30 times a
/// second (menuref README "General facts": every captured frame pair is
/// identical).
pub const SHELL_HZ: f32 = 30.0;

/// A linear alpha ping-pong between `lo` and `hi`, `rate` a second, stepped
/// at [`SHELL_HZ`] and clamped at the ends (`RoundIFButtonLabel_fnHilight`,
/// §5 [V]). The 30 Hz steps overshoot each end by one update, so the period
/// is 22 updates = 0.734 s, not 0.7 (menuref §9.4.1, §9.4.6 [M]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulse {
    value: f32,
    dir: f32,
    lo: f32,
    hi: f32,
    /// Seconds not yet stepped.
    acc: f32,
}

impl Pulse {
    /// A focused button: 0.3 .. 1.0 at 2.0 /s, starting at 0.5 going up
    /// (`gButton_CurSizeAdd` 0.5, `gButton_CurDir` 2.0; §5 [V]). The only
    /// pulse: a list's `listbox_cursor` box stays solid (menuref §9.1.9).
    pub const BUTTON: Pulse = Pulse {
        value: 0.5,
        dir: 2.0,
        lo: 0.3,
        hi: 1.0,
        acc: 0.0,
    };

    /// `dt` seconds: whole 30 Hz updates (main.f6 pc 2-32 each: add, then
    /// clamp and turn).
    pub fn advance(&mut self, dt: f32) {
        let step = 1.0 / SHELL_HZ;
        self.acc += dt;
        // The slack absorbs f32 rounding of a summed dt.
        while self.acc + 1e-5 >= step {
            self.acc -= step;
            self.value += step * self.dir;
            if self.value > self.hi {
                self.value = self.hi;
                self.dir = -self.dir.abs();
            } else if self.value < self.lo {
                self.value = self.lo;
                self.dir = self.dir.abs();
            }
        }
    }

    pub fn alpha(&self) -> f32 {
        self.value
    }
}

/// The flashy text's phase lengths after its start delay: bar wipe, square
/// at the left edge, square sweep, vertical pop, fade-in. Measured on the
/// PS2 (menuref §9.4.5, `measure/flashy_sp_entrance_strip.png`, Main ->
/// Singleplayer title: wipe 1.617-1.684 s, square 1.684, sweep 1.717-1.817,
/// pop 1.817-1.884, solid by ~1.95): 2, 1, 3, 2, 2 updates. The ELF's table
/// 0x5ff028 reads 0.1 / 0.05 / 0.05 / 0.15 / 0.15 (§5 [V]); the screen runs
/// faster than that.
pub const FLASHY_PHASES: [f32; 5] = [
    2.0 / SHELL_HZ,
    1.0 / SHELL_HZ,
    3.0 / SHELL_HZ,
    2.0 / SHELL_HZ,
    2.0 / SHELL_HZ,
];

/// What a flashy text draws `t` seconds after its start delay (the native
/// draw `FUN_00381858`, §5 [V]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flashy {
    /// How much of the bar's middle is out (0..1); `None` draws no bar.
    pub bar: Option<f32>,
    /// The `cursor` square's place across the text box (0 = its left
    /// edge, 1 = its right); `None` draws none.
    pub square: Option<f32>,
    /// The text's height (0..1, about its centre line) and alpha factor.
    pub text_h: f32,
    pub text_a: f32,
}

impl Flashy {
    /// The normal draw (phase 6 on).
    pub const DONE: Flashy = Flashy {
        bar: Some(1.0),
        square: None,
        text_h: 1.0,
        text_a: 1.0,
    };
}

/// [`Flashy`] at `t` seconds after the text's start delay; before it
/// (`t < 0`) nothing is drawn.
pub fn flashy(t: f32) -> Flashy {
    let [wipe, edge, sweep, pop, fade] = FLASHY_PHASES;
    let none = Flashy {
        bar: None,
        square: None,
        text_h: 0.0,
        text_a: 0.0,
    };
    let full = Flashy {
        bar: Some(1.0),
        ..none
    };
    if t < 0.0 {
        none
    } else if t < wipe {
        Flashy {
            bar: Some(t / wipe),
            ..none
        }
    } else if t < wipe + edge {
        Flashy {
            square: Some(0.0),
            ..full
        }
    } else if t < wipe + edge + sweep {
        Flashy {
            square: Some((t - wipe - edge) / sweep),
            ..full
        }
    } else if t < wipe + edge + sweep + pop {
        Flashy {
            text_h: (t - wipe - edge - sweep) / pop,
            text_a: 1.0,
            ..full
        }
    } else if t < wipe + edge + sweep + pop + fade {
        Flashy {
            text_h: 1.0,
            text_a: (t - wipe - edge - sweep - pop) / fade,
            ..full
        }
    } else {
        Flashy::DONE
    }
}

/// Seconds from one shell music take's end to the next take's start: the
/// shell music's interval, 10.0 in the ELF (0x5faeac, read by
/// `FUN_001a0da8`; `ScriptCB_SetShellMusicInterval(10)`, §4.4 [V]). On the
/// PS2 the next take starts 9.9-10.4 s after a take's length and the heard
/// silence is 10.6-11.0 s with the take's quiet tail (menuref "Music",
/// `coldboot_music`).
pub const MUSIC_GAP: f32 = 10.0;

/// Which take of `shell_soundtrack` plays when: a random take each time,
/// repeats allowed (menuref §9.3.9: A, B, C, C, C, B, C over 7.5 min of one
/// boot), the first at Press START, never restarted by a screen change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShellPlaylist {
    /// The take playing (or last played) and the seconds until the next
    /// one starts; `None` before the first.
    now: Option<(usize, f32)>,
}

impl ShellPlaylist {
    /// One frame of `dt`; `secs` are the takes' lengths in list order and
    /// `pick(n)` a random index below `n`. Returns the take to start now,
    /// if one is due.
    pub fn step(
        &mut self,
        dt: f32,
        secs: &[f32],
        pick: &mut dyn FnMut(usize) -> usize,
    ) -> Option<usize> {
        if secs.is_empty() {
            return None;
        }
        if let Some((_, left)) = &mut self.now {
            *left -= dt;
            if *left > 0.0 {
                return None;
            }
        }
        let next = pick(secs.len()).min(secs.len() - 1);
        self.now = Some((next, secs[next] + MUSIC_GAP));
        Some(next)
    }

    /// The take playing now (`None` in a gap or before the first).
    pub fn playing(&self, secs: &[f32]) -> Option<usize> {
        let (take, left) = self.now?;
        (left > MUSIC_GAP && take < secs.len()).then_some(take)
    }
}

/// A held direction's first repeat after this many seconds, then one every
/// [`REPEAT_EVERY`], each a press with its click (menuref Main Menu and
/// Instant Action: "repeats from 0.61-0.65 s, every 0.167 s (5 updates)").
pub const REPEAT_DELAY: f32 = 0.62;
pub const REPEAT_EVERY: f32 = 5.0 / SHELL_HZ;

/// The menus' hold repeat for the D-pad arms.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HoldRepeat {
    held: u32,
    /// Seconds the arms have been held, and when the next repeat is due.
    t: f32,
    next: f32,
}

impl HoldRepeat {
    /// One frame: `held` = the arms down now (a bit each). Returns the arms
    /// to press again this frame (0 on a change: that press is an edge).
    pub fn step(&mut self, held: u32, dt: f32) -> u32 {
        if held != self.held {
            *self = HoldRepeat {
                held,
                t: 0.0,
                next: REPEAT_DELAY,
            };
            return 0;
        }
        if held == 0 {
            return 0;
        }
        self.t += dt;
        let mut out = 0;
        while self.t + 1e-5 >= self.next {
            out = held;
            self.next += REPEAT_EVERY;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The updates (30 Hz) at which the pulse sits at `hi` / `lo`, fed by
    /// frames of `dt`, over `secs`.
    fn turns(dt: f32, secs: f32) -> (Vec<usize>, Vec<usize>) {
        let mut p = Pulse::BUTTON;
        let (mut tops, mut bottoms) = (Vec::new(), Vec::new());
        let mut last = p.alpha();
        for frame in 1..=(secs / dt).round() as usize {
            p.advance(dt);
            let a = p.alpha();
            let update = (frame as f32 * dt * SHELL_HZ).round() as usize;
            if a != last && a == 1.0 {
                tops.push(update);
            }
            if a != last && a == 0.3 {
                bottoms.push(update);
            }
            last = a;
        }
        (tops, bottoms)
    }

    /// menuref §9.4.1 / §9.4.6: 0.3 .. 1.0 at 2.0 /s in 30 Hz steps, one
    /// update over each end: tops every 22 updates (0.734 s), bottoms 11
    /// updates after a top, whatever the frame rate.
    #[test]
    fn the_button_pulse_period_is_22_updates() {
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            let (tops, bottoms) = turns(dt, 3.0);
            assert_eq!(tops, [8, 30, 52, 74], "dt {dt}");
            assert_eq!(bottoms, [19, 41, 63, 85], "dt {dt}");
        }
        assert!((22.0 / SHELL_HZ - 0.734).abs() < 1e-3);
        // Linear ramps: equal steps of 2/30 between the ends.
        let mut p = Pulse::BUTTON;
        let mut seen = Vec::new();
        for _ in 0..7 {
            p.advance(1.0 / 30.0);
            seen.push(p.alpha());
        }
        for w in seen.windows(2) {
            assert!((w[1] - w[0] - 2.0 / 30.0).abs() < 1e-5, "{seen:?}");
        }
        // Half an update waits for the other half.
        let mut p = Pulse::BUTTON;
        p.advance(0.5 / 30.0);
        assert_eq!(p.alpha(), 0.5);
        p.advance(0.5 / 30.0);
        assert!((p.alpha() - (0.5 + 2.0 / 30.0)).abs() < 1e-5);
    }

    /// §4.2: the select click at most once per 0.1 s; the other clicks are
    /// never held back.
    #[test]
    fn the_select_click_is_rate_limited_to_one_per_tenth_of_a_second() {
        let mut c = Cues::default();
        let dt = 1.0 / 60.0;
        let mut clicks = Vec::new();
        for frame in 0..60 {
            c.play(MenuSound::Select);
            if !c.take().is_empty() {
                clicks.push(frame);
            }
            c.tick(dt);
        }
        // Every 6th frame of 60 Hz: 10 clicks in a second.
        assert_eq!(clicks, (0..60).step_by(6).collect::<Vec<_>>());
        c.play(MenuSound::Select);
        c.play(MenuSound::Select);
        c.play(MenuSound::Accept);
        c.play(MenuSound::Exit);
        assert_eq!(
            c.take(),
            [MenuSound::Select, MenuSound::Accept, MenuSound::Exit]
        );
    }

    /// §4.1: a list click only when the cursor really moves.
    #[test]
    fn a_list_move_clicks_only_when_the_cursor_moves() {
        let mut c = Cues::default();
        c.list_move(0, 0);
        assert!(c.take().is_empty(), "held at the top: silent");
        c.list_move(0, 1);
        assert_eq!(c.take(), [MenuSound::Select]);
        c.tick(0.2);
        c.list_move(4, 4);
        assert!(c.take().is_empty(), "held at the bottom: silent");
    }

    #[test]
    fn the_names_are_the_cores() {
        let names: Vec<&str> = [
            MenuSound::Select,
            MenuSound::Accept,
            MenuSound::Exit,
            MenuSound::Cancel,
            MenuSound::Error,
            MenuSound::Key,
            MenuSound::Backspace,
        ]
        .map(MenuSound::name)
        .to_vec();
        assert_eq!(
            names,
            [
                "shell_select_change",
                "shell_menu_accept",
                "shell_menu_exit",
                "shell_menu_cancel",
                "shell_menu_error",
                "shell_keyboard_key",
                "shell_keyboard_backspace"
            ]
        );
    }

    /// menuref §9.4.5: bar wipe 2 updates, square 1 + sweep 3, pop 2,
    /// fade 2 (0.333 s in all).
    #[test]
    fn the_flashy_entrance_runs_its_five_phases_as_measured() {
        let u = 1.0 / SHELL_HZ;
        assert_eq!(flashy(-0.01).bar, None);
        assert!((flashy(u).bar.unwrap() - 0.5).abs() < 1e-4);
        assert_eq!(flashy(u).text_a, 0.0);
        assert_eq!(flashy(2.5 * u).square, Some(0.0));
        let f = flashy(4.5 * u);
        assert!((f.square.unwrap() - 0.5).abs() < 1e-4);
        let f = flashy(7.0 * u);
        assert!((f.text_h - 0.5).abs() < 1e-4 && f.text_a == 1.0 && f.square.is_none());
        let f = flashy(9.0 * u);
        assert!((f.text_a - 0.5).abs() < 1e-4 && f.text_h == 1.0);
        assert_eq!(flashy(10.0 * u + 1e-4), Flashy::DONE);
        assert!((FLASHY_PHASES.iter().sum::<f32>() - 1.0 / 3.0).abs() < 1e-5);
    }

    /// menuref §9.3.9: a random take each time (the picker's), repeats
    /// allowed; the next starts the take's length + 10 s after it.
    #[test]
    fn the_music_picks_a_random_take_each_time_with_a_gap() {
        let secs = [51.4, 51.4, 64.4, 56.6];
        // A scripted picker: A, B, C, C, C, B, C as on the PS2 boot.
        let script = [0usize, 1, 3, 3, 3, 1, 3];
        let mut asked = Vec::new();
        let mut k = 0;
        let mut pick = |n: usize| {
            asked.push(n);
            let v = script[k % script.len()];
            k += 1;
            v
        };
        let mut p = ShellPlaylist::default();
        assert_eq!(p.step(0.1, &secs, &mut pick), Some(0), "at once");
        assert_eq!(p.playing(&secs), Some(0));
        let mut started = vec![(0, 0.0f32)];
        let mut t = 0.0f32;
        while started.len() < script.len() {
            t += 0.1;
            if let Some(k) = p.step(0.1, &secs, &mut pick) {
                started.push((k, t));
            }
        }
        let ks: Vec<usize> = started.iter().map(|s| s.0).collect();
        assert_eq!(ks, script, "repeats allowed");
        for w in started.windows(2) {
            let gap = w[1].1 - w[0].1 - secs[w[0].0];
            assert!((gap - MUSIC_GAP).abs() < 0.15, "{started:?}");
        }
        assert!(asked.iter().all(|n| *n == secs.len()));
        // In a gap nothing plays.
        let mut p = ShellPlaylist::default();
        p.step(0.0, &secs, &mut |_| 2);
        p.step(65.0, &secs, &mut |_| 2);
        assert_eq!(p.playing(&secs), None);
        assert_eq!(ShellPlaylist::default().step(0.1, &[], &mut |_| 0), None);
    }

    /// menuref Main Menu: a held arm repeats from 0.62 s, then every 5
    /// updates; a new arm starts over.
    #[test]
    fn a_held_direction_repeats_from_0_62_s_every_0_167_s() {
        let mut h = HoldRepeat::default();
        let dt = 1.0 / 60.0;
        let mut at = Vec::new();
        assert_eq!(h.step(2, dt), 0, "the press itself is the edge");
        for frame in 1..=120 {
            if h.step(2, dt) != 0 {
                at.push(frame as f32 * dt);
            }
        }
        let want = [0.62, 0.787, 0.953, 1.12, 1.287, 1.453, 1.62, 1.787, 1.953];
        assert_eq!(at.len(), want.len(), "{at:?}");
        for (a, w) in at.iter().zip(want) {
            assert!((a - w).abs() <= dt, "{at:?}");
        }
        assert_eq!(h.step(1, dt), 0, "another arm: a new press");
        assert_eq!(h.step(1, 0.6), 0);
        assert_eq!(h.step(1, 0.05), 1);
        assert_eq!(h.step(0, dt), 0);
        assert_eq!(h.step(0, 5.0), 0);
    }
}
