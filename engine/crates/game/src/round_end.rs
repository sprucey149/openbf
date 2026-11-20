//! The end of a round (issue #33, `analysis/re-round-end.md` §2-§3): one flow
//! on every map. When the round is decided the view goes to a random
//! camera shot of the mission and the victory / defeat banner stands over it
//! (5 s at least and until the music ends, 7 s at most: BF1 §2.1); then,
//! on a win with an end movie set, that movie; then
//! BF2's stats pages (issue #50, [`ui::stats`]: Team Stats, a row's Personal
//! Stats, the player's Career Stats), left with Square ("Done") or 30 s with
//! no input (§3, both games); leaving goes on to the next playlist map or
//! back to the shell.

use crate::spawn_backdrop::{CameraShot, ShotPicker};
use ui::shell_feel::MenuSound;
use ui::stats::{Command, Pad, Page, StatsScreens};

/// The banner's shortest and longest stay (BF1 §2.1: 7 s `FUN_0033f718`; 5 s
/// and the music's end `FUN_002be838` / `FUN_002bec60`).
pub const BANNER_MIN_SECS: f32 = 5.0;
pub const BANNER_MAX_SECS: f32 = 7.0;
/// A stats screen with no input this long quits by itself (§3, both games).
pub const STATS_IDLE_SECS: f32 = 30.0;
/// BF2's fade to black after the banner: `FUN_00381110` starts it on every
/// viewport with `FUN_0045ec50(1.0, 0.0001, 0, ..., 2, 2, FUN_00381088)`;
/// `FUN_0045f088` runs its first stage for 1.0 s, mode 2 (`FUN_0045edb8`)
/// raising the black's alpha linearly from 0 to 255, holds 0.0001 s, and
/// its callback `FUN_00381088` leaves the viewports black.
pub const FADE_SECS: f32 = 1.0;

/// Whether the windowed HUD pass draws this frame. At the end of the round it
/// always does, `hud` cheat or not: it draws no battle HUD then, only the
/// banner over the camera shot (issue #33) and later the stats pages.
pub fn hud_pass_drawn(round_end: bool, hud_hidden: bool) -> bool {
    round_end || !hud_hidden
}

/// Where the end of the round stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The banner over the camera shot.
    Banner,
    /// [`FADE_SECS`] of black rising over the banner and the shot. The
    /// movie and the stats wait on its callback (`FUN_00381110`: both test
    /// the bit `FUN_00381088` sets). trace: whether the viewport fader
    /// covers the banner too; drawn over it here.
    Fade,
    /// `SetMissionEndMovie`'s clip, between the banner and the stats
    /// (`FUN_00381110`): once the banner's timer has run out and the fade
    /// to black (`FUN_0045ec50`, done in `FUN_00381088`) is over, with the
    /// clip set and the winners mask's bit 2 up, `FUN_00564398` plays it;
    /// the stats wait on it (`FUN_00381358`: `FUN_00172df0`, a movie still
    /// playing). The caller plays it and calls [`RoundEnd::movie_over`].
    /// trace: `DAT_00600b4d`, the other flag it needs.
    Movie,
    /// The stats pages ([`RoundEnd::stats`] says which).
    Stats,
    /// Left: the next playlist map, else the shell.
    Done,
}

/// How the round went for the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Result {
    Victory,
    Defeat,
    /// The time limit ran out on equal reinforcements: both teams win.
    Draw,
}

#[derive(Clone, Debug)]
pub struct RoundEnd {
    pub result: Result,
    pub phase: Phase,
    /// The camera shot the view stands on; None when the mission has none
    /// (the battle's view stays).
    pub shot: Option<CameraShot>,
    banner_secs: f32,
    /// Seconds in the current phase, and since the last input on a stats
    /// screen.
    pub secs: f32,
    idle: f32,
    show_stats: bool,
    /// The stats pages, made once the banner is down
    /// ([`RoundEnd::wants_stats`]); input waits for them.
    pub stats: Option<StatsScreens>,
    /// The end movie to play after the banner ([`Phase::Movie`]).
    pub movie: Option<String>,
}

impl RoundEnd {
    /// The round just ended: draw a shot from `shots` (the picker's own draw,
    /// seeded) and stand the banner for the music's length within 5-7 s
    /// (5 s with no music); `movie` plays after it.
    pub fn start(
        result: Result,
        shots: &[CameraShot],
        seed: u32,
        music_secs: Option<f32>,
        movie: Option<String>,
    ) -> Self {
        let shot = (!shots.is_empty()).then(|| {
            let mut p = ShotPicker::new(seed);
            shots[p.next(shots.len()).min(shots.len() - 1)]
        });
        Self {
            result,
            phase: Phase::Banner,
            shot,
            banner_secs: music_secs
                .unwrap_or(BANNER_MIN_SECS)
                .clamp(BANNER_MIN_SECS, BANNER_MAX_SECS),
            secs: 0.0,
            idle: 0.0,
            show_stats: true,
            stats: None,
            movie,
        }
    }

    /// A campaign finale leaves the battle once its end movie is over.
    pub fn without_stats(mut self) -> Self {
        self.show_stats = false;
        self
    }

    fn after_movie(&self) -> Phase {
        if self.show_stats {
            Phase::Stats
        } else {
            Phase::Done
        }
    }

    /// The end movie is over, or there was none to play.
    pub fn movie_over(&mut self) {
        if self.phase == Phase::Movie {
            self.phase = self.after_movie();
            self.secs = 0.0;
        }
    }

    /// The end movie has had its turn (played, skipped or missing).
    pub fn movie_played(&self) -> bool {
        self.movie.is_some() && !matches!(self.phase, Phase::Banner | Phase::Fade)
    }

    /// The black over the banner and the shot, 0 (none) to 1 (opaque).
    pub fn fade(&self) -> f32 {
        match self.phase {
            Phase::Banner => 0.0,
            Phase::Fade => (self.secs / FADE_SECS).min(1.0),
            Phase::Movie | Phase::Stats | Phase::Done => 1.0,
        }
    }

    /// The clip [`Phase::Movie`] plays now.
    pub fn playing_movie(&self) -> Option<&str> {
        self.movie.as_deref().filter(|_| self.phase == Phase::Movie)
    }

    /// The battle's sounds are over: from the fade out of the banner on
    /// (`FUN_00381110` stops them before the movie: `FUN_0043df50` sets each
    /// of the six channels' playing voices to state 2 and stops the streams [D];
    /// `FUN_00380f48` again before the stats).
    pub fn silenced(&self) -> bool {
        self.phase != Phase::Banner
    }

    /// The banner is down and the pages are not made yet.
    pub fn wants_stats(&self) -> bool {
        self.phase == Phase::Stats && self.stats.is_none()
    }

    /// One frame: `pad` is this frame's presses.
    pub fn step(&mut self, dt: f32, pad: Pad) -> Option<MenuSound> {
        self.secs += dt;
        let mut sound = None;
        let next = match self.phase {
            Phase::Banner => (self.secs >= self.banner_secs).then_some(Phase::Fade),
            Phase::Fade => (self.secs >= FADE_SECS).then_some(if self.movie.is_some() {
                Phase::Movie
            } else {
                self.after_movie()
            }),
            Phase::Movie => None,
            Phase::Stats => {
                self.idle = if pad.any() { 0.0 } else { self.idle + dt };
                let quit = self.stats.as_mut().and_then(|s| {
                    let before = s.selection();
                    let command = s.input(pad);
                    let after = s.selection();
                    sound = if command == Some(Command::Quit)
                        || matches!(
                            (before.0, after.0),
                            (Page::Personal, Page::Team) | (Page::Career, Page::Personal)
                        ) {
                        Some(MenuSound::Exit)
                    } else if before.0 != after.0 {
                        Some(MenuSound::Accept)
                    } else {
                        (before != after).then_some(MenuSound::Select)
                    };
                    command
                });
                (self.idle >= STATS_IDLE_SECS || quit == Some(Command::Quit)).then_some(Phase::Done)
            }
            Phase::Done => None,
        };
        if let Some(p) = next {
            self.phase = p;
            self.secs = 0.0;
            self.idle = 0.0;
        }
        sound
    }

    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }
}

/// One frame of [`Phase::Movie`] on the shell's player `movie`: it starts
/// the clip with its sound track (dropped with `voice`, at the dialogue
/// `volume` as the shell's cutscenes) and ends the phase at the clip's end,
/// on `skip`, or at once when the clip is not baked. The newest decoded
/// frame, for the HUD. OURS: `skip` (the shell cutscenes' accept / back);
/// trace: whether `FUN_00564398`'s player takes a skip.
pub fn step_end_movie(
    r: &mut RoundEnd,
    movie: Option<&mut crate::shell_movie::ShellMovie>,
    audio: Option<&mut audio::Audio>,
    voice: &mut Option<audio::Shot>,
    volume: f32,
    dt: f32,
    skip: bool,
) -> Option<(Vec<u8>, u32, u32)> {
    let clip = r.playing_movie()?.to_owned();
    let Some(m) = movie.filter(|m| m.has(&clip)) else {
        *voice = None;
        r.movie_over();
        return None;
    };
    if m.current() != Some(clip.as_str()) {
        m.play_once(&clip);
        *voice = None;
        if let (Some(path), Some(a)) = (m.audio_path(&clip).filter(|_| volume > 0.0), audio) {
            match audio::decode(path) {
                Ok(sound) => *voice = a.play_menu_shot(&sound, crate::sounds::level_db(volume)),
                Err(e) => eprintln!("end movie audio {}: {e:#}", path.display()),
            }
        }
    }
    let frame = m.frame(dt);
    if m.is_finished() || skip {
        m.stop();
        *voice = None;
        r.movie_over();
    }
    frame
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    use ui::stats::{Career, Page, StatRow, StatsTexts, TeamList};

    fn shots() -> Vec<CameraShot> {
        (0..4)
            .map(|i| CameraShot::from_call([1.0, 0.0, 0.0, 0.0, i as f32 * 10.0, 5.0, 0.0]))
            .collect()
    }

    fn pages() -> StatsScreens {
        let me = StatRow {
            name: "Player 1".into(),
            me: true,
            personal: vec![
                ("Points".into(), "3".into()),
                (String::new(), String::new()),
                ("Kills".into(), "2".into()),
            ],
            ..Default::default()
        };
        StatsScreens::new(
            StatsTexts::default(),
            [
                TeamList {
                    icon: String::new(),
                    rows: vec![me],
                },
                TeamList::default(),
            ],
            [Vec::new(), Vec::new()],
            Career::default(),
        )
    }

    fn press(f: impl FnOnce(&mut Pad)) -> Pad {
        let mut p = Pad::default();
        f(&mut p);
        p
    }

    /// The banner stands for the music within 5-7 s (5 with none), then the
    /// stats pages, which wait to be made; X walks them, Triangle back,
    /// Square leaves.
    /// The banner and the pages are the HUD pass's: hiding the battle HUD
    /// must not hide them (the windowed game once showed no banner).
    #[test]
    fn the_hud_pass_draws_at_the_end_of_the_round_even_hidden() {
        assert!(hud_pass_drawn(true, false));
        assert!(hud_pass_drawn(true, true));
        assert!(hud_pass_drawn(false, false));
        assert!(!hud_pass_drawn(false, true));
    }

    #[test]
    fn the_banner_then_the_stats_pages_then_out() {
        for (music, want) in [
            (None, 5.0),
            (Some(3.0), 5.0),
            (Some(6.2), 6.2),
            (Some(20.0), 7.0),
        ] {
            let mut r = RoundEnd::start(Result::Victory, &shots(), 7, music, None);
            let mut t = 0.0f32;
            while r.phase == Phase::Banner {
                r.step(1.0 / 30.0, Pad::default());
                t += 1.0 / 30.0;
            }
            assert!((t - want).abs() < 0.05, "music {music:?}: {t}");
            assert_eq!(r.phase, Phase::Fade);
            assert!(r.fade() < 0.05);
            r.step(FADE_SECS * 0.5, Pad::default());
            assert!((r.fade() - 0.5).abs() < 0.05, "linear: {}", r.fade());
            assert!(!r.wants_stats(), "the stats wait for the black");
            r.step(FADE_SECS * 0.5, Pad::default());
            assert_eq!(r.phase, Phase::Stats);
            assert_eq!(r.fade(), 1.0);
        }
        let mut r = RoundEnd::start(Result::Defeat, &shots(), 7, Some(5.0), None);
        r.step(5.1, Pad::default());
        r.step(FADE_SECS, Pad::default());
        assert!(r.wants_stats());
        r.step(0.1, press(|p| p.square = true));
        assert!(!r.done(), "no pages yet: nothing to quit");
        r.stats = Some(pages());
        assert!(!r.wants_stats());
        assert_eq!(
            r.step(0.1, press(|p| p.cross = true)),
            Some(MenuSound::Accept)
        );
        assert_eq!(r.stats.as_ref().unwrap().page, Page::Personal);
        assert_eq!(
            r.step(0.1, press(|p| p.down = true)),
            Some(MenuSound::Select)
        );
        assert_eq!(r.stats.as_ref().unwrap().personal_row, 2);
        assert_eq!(r.step(0.1, press(|p| p.down = true)), None);
        r.step(0.1, press(|p| p.cross = true));
        assert_eq!(r.stats.as_ref().unwrap().page, Page::Career);
        assert_eq!(
            r.step(0.1, press(|p| p.triangle = true)),
            Some(MenuSound::Exit)
        );
        r.step(0.1, press(|p| p.triangle = true));
        assert_eq!(r.stats.as_ref().unwrap().page, Page::Team);
        r.step(0.1, press(|p| p.triangle = true));
        assert!(!r.done(), "Triangle does nothing on Team Stats");
        r.step(0.1, press(|p| p.square = true));
        assert!(r.done(), "Square: Done");
    }

    /// A won campaign round fades to black after the banner, then plays its
    /// end movie before the stats (`FUN_00381110`), the battle silent from
    /// the fade on.
    #[test]
    fn the_end_movie_comes_between_the_banner_and_the_stats() {
        let mut r = RoundEnd::start(Result::Victory, &shots(), 1, None, Some("geomon02".into()));
        assert!(!r.silenced());
        assert_eq!(r.playing_movie(), None);
        r.step(5.1, Pad::default());
        assert_eq!(r.phase, Phase::Fade);
        assert!(r.silenced());
        assert_eq!(r.playing_movie(), None, "no cut straight to the movie");
        assert!(!r.movie_played());
        r.step(FADE_SECS * 0.75, Pad::default());
        assert_eq!(r.phase, Phase::Fade);
        r.step(FADE_SECS * 0.25, Pad::default());
        assert_eq!(r.phase, Phase::Movie);
        assert_eq!(r.playing_movie(), Some("geomon02"));
        assert!(r.silenced());
        assert!(!r.wants_stats());
        r.step(60.0, press(|p| p.square = true));
        assert_eq!(r.phase, Phase::Movie, "the movie waits for its end");
        r.movie_over();
        assert!(r.wants_stats());
        assert!(r.silenced());
    }

    /// 30 s with no input on a stats page quits; any press restarts the
    /// count. A mission with no shots keeps the battle's view.
    #[test]
    fn an_idle_stats_screen_quits_after_30_s() {
        let mut r = RoundEnd::start(Result::Victory, &shots(), 1, None, None);
        assert!(r.shot.is_some());
        r.step(7.1, Pad::default());
        r.step(FADE_SECS, Pad::default());
        r.stats = Some(pages());
        for _ in 0..(29 * 30) {
            r.step(1.0 / 30.0, Pad::default());
        }
        assert!(!r.done());
        r.step(1.0 / 30.0, press(|p| p.down = true));
        for _ in 0..(29 * 30) {
            r.step(1.0 / 30.0, Pad::default());
        }
        assert!(!r.done(), "a press restarts it");
        for _ in 0..(2 * 30) {
            r.step(1.0 / 30.0, Pad::default());
        }
        assert!(r.done());
        assert!(RoundEnd::start(Result::Victory, &[], 1, None, None)
            .shot
            .is_none());
    }
}

#[cfg(test)]
mod tests {
    use crate::round_end::{Phase, Result};
    use crate::spawn_backdrop::CameraShot;
    use crate::walk::WalkSim;
    use ui::stats::Pad;

    /// Issue #33 in the walk: nothing before the round is decided; then the
    /// view stands on a mission shot and the viewer's result is drawn, the
    /// battle's own camera again with no shots; the stats pages follow,
    /// made by the caller, and Square leaves.
    #[test]
    fn the_walk_ends_its_round_on_a_mission_shot() {
        let mut w = WalkSim::new(7, glam::Vec3::ZERO, None);
        let shot = CameraShot::from_call([1.0, 0.0, 0.0, 0.0, 40.0, 25.0, -10.0]);
        w.end_shots = vec![shot];
        assert!(!w.follow_round_end(0.1, Pad::default(), None, true).0);
        assert!(w.round_end.is_none(), "not before the round is decided");
        let me = w.sim.units[w.player.index as usize].team;
        w.sim.outcome = Some(me);
        w.follow_round_end(1.0 / 30.0, Pad::default(), None, true);
        let r = w.round_end.as_ref().unwrap();
        assert_eq!((r.result, r.phase), (Result::Victory, Phase::Banner));
        assert_eq!(w.camera().position, shot.camera().position);
        w.follow_round_end(7.1, Pad::default(), None, true);
        w.follow_round_end(super::FADE_SECS, Pad::default(), None, true);
        let r = w.round_end.as_mut().unwrap();
        assert!(r.wants_stats());
        let unit = |i: usize| format!("U{i}");
        let text = |_: &str, en: &str| en.to_owned();
        let names = crate::round_stats::Names {
            unit: &unit,
            class: &|_, _| String::new(),
            weapon: &|_| String::new(),
            text: &text,
            sides: ["", ""],
        };
        r.stats = Some(crate::round_stats::screens(
            &w.sim,
            &Default::default(),
            w.player,
            &Default::default(),
            &names,
        ));
        let square = Pad {
            square: true,
            ..Pad::default()
        };
        assert!(w.follow_round_end(0.1, square, None, true).0, "Done: out");
        let mut w = WalkSim::new(7, glam::Vec3::ZERO, None);
        w.sim.outcome = Some(1 - me.min(1));
        let before = w.camera().position;
        w.follow_round_end(0.1, Pad::default(), None, true);
        assert_eq!(w.round_end.as_ref().unwrap().result, Result::Defeat);
        assert_eq!(w.camera().position, before, "no shots: the battle's view");
        let mut w = WalkSim::new(7, glam::Vec3::ZERO, None);
        w.sim.outcome = Some(me);
        assert!(!w.follow_round_end(1.0 / 30.0, Pad::default(), None, false).0);
        assert!(!w.follow_round_end(7.1, Pad::default(), None, false).0, "the fade first");
        let (done, _) = w.follow_round_end(super::FADE_SECS, Pad::default(), None, false);
        assert!(done, "the campaign finale leaves for its credits, no stats");
        assert!(w.round_end.as_ref().unwrap().stats.is_none());
    }
}
