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

/// How the round went for a viewer on `team`, the round decided for `outcome` (the winning
/// team, or `core::sim::OUTCOME_DRAW`). Each split view takes its own (issue #194 B22).
pub fn result_for(outcome: u8, team: u8) -> Result {
    if outcome == core::sim::OUTCOME_DRAW {
        Result::Draw
    } else if outcome == team {
        Result::Victory
    } else {
        Result::Defeat
    }
}

impl Result {
    /// The HUD's banner code (`ui::hud::RoundEndModel::result`): 0 victory, 1 defeat, 2 draw.
    pub fn banner(self) -> u8 {
        match self {
            Result::Victory => 0,
            Result::Defeat => 1,
            Result::Draw => 2,
        }
    }
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
    /// Splitscreen (issue #194): which local players have left their own stats pages, by local
    /// index ([`RoundEnd::hold_for_players`]); empty with one player, whose Done ends the stats.
    left: Vec<bool>,
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
            left: Vec::new(),
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
                // Split: the first player's pages close on their Done; the stats end when every
                // player has left theirs ([`RoundEnd::step_views`]).
                let first_left = self.left.first().copied().unwrap_or(false);
                let quit = self.stats.as_mut().filter(|_| !first_left).and_then(|s| {
                    let (command, cue) = step_pages(s, pad);
                    sound = cue;
                    command
                }) == Some(Command::Quit);
                if let (true, Some(l)) = (quit, self.left.first_mut()) {
                    *l = true;
                }
                (self.idle >= STATS_IDLE_SECS || quit && self.left.is_empty())
                    .then_some(Phase::Done)
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

    /// Splitscreen (issue #194): the stats pages are `players` local players' each; from now on
    /// a Done closes only its own player's pages, and the stats end when every player here has
    /// left theirs ([`RoundEnd::step_views`]) or nobody has pressed for the idle time.
    pub fn hold_for_players(&mut self, players: usize) {
        self.left = vec![false; players];
    }

    /// Has local player `i` left their own stats pages (split, [`RoundEnd::hold_for_players`])?
    pub fn has_left(&self, i: usize) -> bool {
        self.left.get(i).copied().unwrap_or(false)
    }

    /// Splitscreen (issue #194 B9): one frame of the further players' own stats pages, after
    /// [`RoundEnd::step`] has stepped the first player's. `views[i]` is player `i + 1`'s pages
    /// (`None`: away, or not made yet) and `pads[i]` their own pad's presses; `first_away`: the
    /// first player is away. Each cursor follows its own pad only. Any press keeps the pages up
    /// (the idle quit counts from the last press of anyone's). A Done closes that player's pages
    /// alone; the stats end for everyone once every player here (the first included, unless
    /// away; a `None` view is away) has left theirs. The sound of the first press that made one.
    pub fn step_views(
        &mut self,
        pads: &[Pad],
        views: &mut [Option<StatsScreens>],
        first_away: bool,
    ) -> Option<MenuSound> {
        if self.phase != Phase::Stats {
            return None;
        }
        if self.left.len() < views.len() + 1 {
            self.left.resize(views.len() + 1, false);
        }
        let mut sound = None;
        for (k, (pad, view)) in pads.iter().zip(views.iter_mut()).enumerate() {
            let Some(s) = view.as_mut().filter(|_| !self.left[k + 1]) else {
                continue;
            };
            if pad.any() {
                self.idle = 0.0;
            }
            let (command, cue) = step_pages(s, *pad);
            sound = sound.or(cue);
            if command == Some(Command::Quit) {
                self.left[k + 1] = true;
            }
        }
        let everyone = (first_away || self.left[0])
            && views
                .iter()
                .enumerate()
                .all(|(k, v)| v.is_none() || self.left[k + 1]);
        if everyone {
            self.phase = Phase::Done;
            self.secs = 0.0;
            self.idle = 0.0;
        }
        sound
    }
}

/// One frame of one player's stats pages: the command `pad` gave and the menu sound it makes
/// (Exit on leaving or going back a page, Accept on going forward, Select on a cursor move).
pub fn step_pages(s: &mut StatsScreens, pad: Pad) -> (Option<Command>, Option<MenuSound>) {
    let before = s.selection();
    let command = s.input(pad);
    let after = s.selection();
    let sound = if command == Some(Command::Quit)
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
    (command, sound)
}

/// Decode a one-shot movie's sound track once per clip and expose its newest
/// decoded frame. The single canonical version of a repeated piece of the
/// round-ending movie ([`step_end_movie`]) and the shell's campaign cutscenes
/// (`front::campaign`): both keep a `(clip, voice)` slot, so a clip's audio is
/// decoded exactly once and both sites agree on how a one-shot is read. The
/// caller owns playback state (start / skip / phase changes) and clears
/// `voice` when the clip ends (cf. `holo::lens_flare` for a shared draw).
pub fn step_one_shot_movie(
    movie: &mut crate::shell_movie::ShellMovie,
    clip: &str,
    audio: Option<&mut audio::Audio>,
    voice: &mut Option<(String, audio::Shot)>,
    volume: f32,
    dt: f32,
) -> Option<(Vec<u8>, u32, u32)> {
    if voice.as_ref().is_none_or(|(name, _)| name != clip) {
        *voice = None;
        if volume > 0.0 {
            if let (Some(path), Some(a)) = (movie.audio_path(clip), audio) {
                match audio::decode(path) {
                    Ok(sound) => {
                        *voice = a
                            .play_menu_shot(&sound, crate::sounds::level_db(volume))
                            .map(|shot| (clip.to_owned(), shot));
                    }
                    Err(e) => eprintln!("movie audio {}: {e:#}", path.display()),
                }
            }
        }
    }
    movie.frame(dt)
}

/// One frame of [`Phase::Movie`] on the shell's player `movie`: it starts
/// the clip and ends the phase at the clip's end, on `skip`, or at once when
/// the clip is not baked. The newest decoded frame, for the HUD. OURS: `skip`
/// (the shell cutscenes' accept / back); trace: whether `FUN_00564398`'s
/// player takes a skip. Audio decoding and frame decoding are shared with the
/// shell's cutscenes in [`step_one_shot_movie`].
pub fn step_end_movie(
    r: &mut RoundEnd,
    movie: Option<&mut crate::shell_movie::ShellMovie>,
    audio: Option<&mut audio::Audio>,
    voice: &mut Option<(String, audio::Shot)>,
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
    }
    let frame = step_one_shot_movie(m, &clip, audio, voice, volume, dt);
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

    /// Issue #194 B9: each split player's stats pages follow their own pad only. P2's cross
    /// opens P2's Personal page and leaves P1's on Team Stats; P1's down moves P1's cursor
    /// alone; P2's press keeps the pages up past P1's idle; an away player's `None` takes
    /// nothing. Integration finding: P2's Done closes P2's pages only; the stats end once P1
    /// has left too.
    #[test]
    fn each_split_players_pages_follow_their_own_pad() {
        let mut r = RoundEnd::start(Result::Victory, &shots(), 7, None, None);
        r.step(5.1, Pad::default());
        r.step(FADE_SECS, Pad::default());
        r.stats = Some(pages());
        r.hold_for_players(3);
        let mut views = vec![Some(pages()), None];
        let none = [Pad::default(), Pad::default()];
        assert_eq!(r.step_views(&none, &mut views, false), None);
        r.step(0.1, Pad::default());
        let cue = r.step_views(
            &[press(|p| p.cross = true), press(|p| p.cross = true)],
            &mut views,
            false,
        );
        assert_eq!(cue, Some(MenuSound::Accept));
        assert_eq!(
            views[0].as_ref().unwrap().page,
            Page::Personal,
            "P2's page moved"
        );
        assert_eq!(r.stats.as_ref().unwrap().page, Page::Team, "P1's did not");
        assert!(views[1].is_none(), "the away player's stays empty");
        r.step(0.1, press(|p| p.down = true));
        assert_eq!(
            views[0].as_ref().unwrap().personal_row,
            0,
            "P1's pad moves P2's nothing"
        );
        // P1 idle for 29 s while P2 keeps pressing: the pages stay up.
        for _ in 0..29 {
            r.step(1.0, Pad::default());
            r.step_views(&[press(|p| p.up = true), Pad::default()], &mut views, false);
        }
        r.step(1.5, Pad::default());
        assert!(!r.done(), "P2's presses keep the pages up");
        r.step_views(
            &[press(|p| p.square = true), Pad::default()],
            &mut views,
            false,
        );
        assert!(!r.done(), "P2's Done closes P2's pages only");
        assert!(r.has_left(1) && !r.has_left(0));
        let page = views[0].as_ref().unwrap().page;
        r.step_views(
            &[press(|p| p.triangle = true), Pad::default()],
            &mut views,
            false,
        );
        assert_eq!(
            views[0].as_ref().unwrap().page,
            page,
            "left pages take no input"
        );
        r.step(0.1, press(|p| p.square = true));
        assert!(!r.done(), "a frame's views are stepped after P1's");
        r.step_views(&none, &mut views, false);
        assert!(r.done(), "everyone here has left (P3 is away)");
        assert_eq!(
            r.step_views(
                &[press(|p| p.cross = true), Pad::default()],
                &mut views,
                false
            ),
            None
        );
    }

    /// Integration finding (issue #194): in split, P1's Done does not end the stats while P2 reads
    /// theirs; an away P1 is not waited for; the idle time still ends them for everyone.
    #[test]
    fn split_stats_end_when_every_player_has_left() {
        let start = || {
            let mut r = RoundEnd::start(Result::Victory, &shots(), 7, None, None);
            r.step(5.1, Pad::default());
            r.step(FADE_SECS, Pad::default());
            r.stats = Some(pages());
            r.hold_for_players(2);
            r
        };
        let mut r = start();
        let mut views = vec![Some(pages())];
        r.step(0.1, press(|p| p.square = true));
        r.step_views(&[Pad::default()], &mut views, false);
        assert!(!r.done() && r.has_left(0), "P1 waits for P2");
        r.step(0.1, Pad::default());
        r.step_views(&[press(|p| p.square = true)], &mut views, false);
        assert!(r.done(), "both have left");

        let mut r = start();
        r.step(0.1, Pad::default());
        r.step_views(&[press(|p| p.square = true)], &mut views, true);
        assert!(r.done(), "P1 is away: P2's Done is the last");

        let mut r = start();
        r.step(0.1, press(|p| p.square = true));
        for _ in 0..31 {
            r.step(1.0, Pad::default());
            r.step_views(&[Pad::default()], &mut views, false);
        }
        assert!(r.done(), "nobody pressed for the idle time");
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
        assert!(
            !w.follow_round_end(1.0 / 30.0, Pad::default(), None, false)
                .0
        );
        assert!(
            !w.follow_round_end(7.1, Pad::default(), None, false).0,
            "the fade first"
        );
        let (done, _) = w.follow_round_end(super::FADE_SECS, Pad::default(), None, false);
        assert!(done, "the campaign finale leaves for its credits, no stats");
        assert!(w.round_end.as_ref().unwrap().stats.is_none());
    }
}
