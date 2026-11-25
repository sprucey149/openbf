//! What the windowed game does after every sim tick it steps, shared by the
//! single-player accumulator (`WalkSim::advance_with`) and a LAN peer's
//! bundles (`game::netplay::LocalView::frame`): the stations' poses, the
//! effects, the shot points, the event sounds and ambience, the feed's
//! medal lines, the black box, rumble, the match log and the frame record.
//!
//! Rules:
//! - It reads the sim; the one write is single player's score publication
//!   (`game::heroes::tick`), which a lockstep peer's `netplay::step` makes
//!   itself for every slot, so `lockstep` skips it.
//! - `muted`: the local unit's shot this tick was already fed back by the
//!   prediction (`game::netplay::predict`): its fire sound and muzzle flash
//!   are not started again.

use super::spawn_feedback::Selection;
use super::{note_sounds, play_ambience, play_event_sounds};
use game::walk::WalkSim;

/// Per-frame state the tick hook carries from one tick to the next.
pub(super) struct TickState {
    /// Each local player's spawn picker, by local index (issue #194 B12); `None` while that
    /// player is away. One entry with one local player.
    pub selection: Vec<Option<Selection>>,
    /// Each seated human slot's medal counts already announced (`EventFeed::note_medals`):
    /// the viewer's own lines and the spectator lines for the other humans all
    /// compare against this, so a peer announces each medal once (LAN §5b).
    /// Rebuilt every frame from the slots' current counts (`TickState::new`),
    /// so a frame announces only what its own ticks earned.
    pub medals_seen: std::collections::BTreeMap<u8, core::sim::MedalCounts>,
    /// Split (issue #194 B4): the same for each further local player's own feed, views 1-3,
    /// seeded alike; empty with one local player.
    pub views_seen: Vec<std::collections::BTreeMap<u8, core::sim::MedalCounts>>,
    pub previous_mount: Option<core::vehicle::Mount>,
    pub stuck_alarm: bool,
    pub tick_pad: core::sim::PadState,
    pub aspect: f32,
    /// Split (issue #194 B12): each local player's cell aspect, by local index, for the foley's
    /// views; empty with one local player (the window's `aspect`).
    pub view_aspects: Vec<f32>,
}

impl TickState {
    pub fn new(
        walk: &mut WalkSim,
        medals: core::sim::MedalCounts,
        pad: core::sim::PadState,
        aspect: f32,
    ) -> Self {
        // Every seated human slot from its installed career, so the spectator
        // lines start at each player's opening counts (`EventFeed::medals_seen`);
        // in single player just slot 0's profile career.
        let medals_seen = game::event_feed::EventFeed::medals_seen(walk, medals);
        // Split: each further player's feed starts from the same counts (issue #194 B4).
        let views_seen = match walk.local_count() {
            1 => Vec::new(),
            n => vec![medals_seen.clone(); n - 1],
        };
        Self {
            selection: selections(walk),
            medals_seen,
            views_seen,
            previous_mount: walk.sim.unit(walk.player).and_then(|u| u.mount),
            stuck_alarm: false,
            tick_pad: pad,
            aspect,
            view_aspects: Vec::new(),
        }
    }
}

/// Every local player's spawn picker (issue #194 B12): each read as that player
/// ([`WalkSim::with_local`]), `None` for a player who is away, whose screens
/// a bot now holds.
fn selections(walk: &mut WalkSim) -> Vec<Option<Selection>> {
    (0..walk.local_count())
        .map(|i| {
            (walk.local_count() == 1 || !walk.is_suspended(i))
                .then(|| walk.with_local(i, |w| Selection::read(w)))
        })
        .collect()
}

/// Where this tick's sounds are heard from and which soldiers' foley sounds
/// (issue #194 B12, `SPLITSCREEN.md` 18.1 item 6): every awake local player's
/// ears (`event_sounds::listeners`) and views, each view at its own cell's
/// aspect and widened field (`split_layout::fit_fov_y`), the first awake
/// player's first. With one local player: `listener(walk)` and the camera at
/// the window's `aspect`, as before. With every local player away no camera is
/// read (each may follow a bot's unit): the held ears and a view that draws nobody.
pub(super) fn ears_and_views(
    walk: &mut WalkSim,
    aspect: f32,
    view_aspects: &[f32],
) -> (game::sounds::Listener, game::foley::View) {
    let listener = game::event_sounds::listeners(walk);
    if walk.local_count() == 1 {
        return (listener, game::foley::View::of(&walk.camera(), aspect));
    }
    let awake: Vec<usize> = (0..walk.local_count())
        .filter(|&i| !walk.is_suspended(i))
        .collect();
    let mut views = awake
        .into_iter()
        .map(|i| {
            let a = view_aspects.get(i).copied().unwrap_or(aspect);
            let mut cam = walk.camera_of(i);
            cam.fov_y = game::split_layout::fit_fov_y(cam.fov_y, a);
            game::foley::View::of(&cam, a)
        })
        .collect::<Vec<_>>()
        .into_iter();
    let view = match views.next() {
        Some(first) => first.with_more(views),
        None => game::foley::View::blind(listener.pos),
    };
    (listener, view)
}

/// The App's fields the hook uses, borrowed for one frame.
pub(super) struct TickHook<'a> {
    pub gpu: Option<&'a mut game::viewer::SceneGpu>,
    pub fx: &'a mut game::fx::Fx,
    pub soldier_vis: Option<&'a mut game::soldier_vis::SoldierVisuals>,
    pub deka_vis: &'a mut game::viewer::DroidekaVisuals,
    pub bolts: &'a mut game::bolt_vis::BoltVis,
    pub audio: Option<&'a mut audio::Audio>,
    pub sounds: Option<&'a mut game::sounds::SoundLib>,
    pub foley: Option<&'a mut game::foley::Foley>,
    pub engines: &'a mut game::engine_sound::EngineSounds,
    pub ambience: &'a game::ambience::Ambience,
    pub blackbox: Option<&'a mut game::blackbox::BlackBox>,
    pub rumble_strength: f32,
    pub rumble_on_fire: bool,
    pub rumble_tick: &'a mut game::rumble::TickFeedback,
    pub gilrs: Option<&'a mut gilrs::Gilrs>,
    pub active_pad: &'a input::ActivePad<gilrs::GamepadId>,
    pub rumble_effect: &'a mut Option<gilrs::ff::Effect>,
    pub matchlog: Option<&'a mut game::matchlog::MatchLog>,
    pub session_stats: Option<&'a mut game::session_stats::SessionStats>,
    pub frame_profile: Option<&'a mut game::frame_profile::FrameProfile>,
    pub display_ext: Option<&'a dyn extensions::runtime::ActiveDisplayExtension>,
}

impl TickHook<'_> {
    /// One stepped tick (module rules).
    pub fn tick(
        &mut self,
        walk: &mut WalkSim,
        feed: &mut game::event_feed::EventFeed,
        st: &mut TickState,
        lockstep: bool,
        muted: u8,
    ) {
        let hook_t0 = std::time::Instant::now();
        // Bit `k`: local `k`'s shot this tick was fed back by the prediction (issue #194 B20:
        // every local player of this machine, not only the first).
        let muted: Vec<u32> = (0..walk.local_count().min(8))
            .filter(|k| muted >> k & 1 != 0)
            .map(|k| walk.local_unit(k).index)
            .collect();
        if let Some(gpu) = self.gpu.as_deref_mut() {
            gpu.step_stations(&walk.sim);
        }
        if let Some(vis) = self.soldier_vis.as_deref_mut() {
            vis.mute_shots(walk.sim.tick, &muted);
        }
        self.fx.ingest_posed(walk, self.soldier_vis.as_deref());
        let effect_sounds = self.fx.take_sounds();
        let buff_loops = self.fx.buff_loops(&walk.sim);
        if let Some(vis) = self.soldier_vis.as_deref_mut() {
            vis.note_shots(walk);
            let mut points = vis.tick_fire_points(walk, |_| false);
            self.deka_vis.fire_points(&walk.sim, &mut points);
            if let Some(view) = self
                .display_ext
                .filter(|_| !lockstep)
                .and_then(|ext| ext.stereo_view(&super::xr::sim_view(walk)))
            {
                super::xr::place_tracked_muzzle(&view, walk, vis, &mut points);
            }
            self.bolts.set_fire_points(points);
        }
        self.bolts.track(walk);
        // The drawn feet: a step needs one that came down (card FOLY, §4.1).
        if let (Some(foley), Some(vis)) = (self.foley.as_deref_mut(), self.soldier_vis.as_deref()) {
            let heights: Vec<_> = vis
                .water_feet(walk)
                .into_iter()
                .map(|f| f.map(|f| f.height))
                .collect();
            foley.feel(walk, &heights);
        }
        if let (Some(audio), Some(sl)) = (self.audio.as_deref_mut(), self.sounds.as_deref_mut()) {
            // Each awake player's own picker moves play its cue (2-D: every player hears it).
            let next = selections(walk);
            for (was, now) in st.selection.iter().zip(&next) {
                if let (Some(was), Some(now)) = (was, now) {
                    if let Some(cue) = was.movement_cue(*now, walk.sim.ground_rules) {
                        sl.play_2d(Some(audio), cue);
                    }
                }
            }
            st.selection = next;
            let (listener, view) = ears_and_views(walk, st.aspect, &st.view_aspects);
            // OpenBF VR: the headset's ears (one local player only).
            let listener = self
                .display_ext
                .filter(|_| !lockstep && walk.local_count() == 1)
                .and_then(|ext| ext.stereo_view(&super::xr::sim_view(walk)))
                .map_or(listener, |view| super::xr::listener(&view));
            play_event_sounds(
                audio,
                sl,
                self.foley.as_deref_mut(),
                self.engines,
                &effect_sounds,
                &listener,
                &view,
                walk,
                &muted,
            );
            // The auras' loops (the War Hero's crackle).
            sl.play_buff_loops(audio, &buff_loops, &listener);
            play_ambience(audio, sl, self.ambience, walk, &listener);
        }
        if !lockstep {
            if walk.local_count() > 1 && walk.is_suspended(0) {
                // Split, the first player away: their handle may name a bot's unit, so the feed
                // still scores the round for everyone but credits no medal points to it
                // (issue #194; the further players' feeds are skipped the same way), nor the
                // bot's shots, hits and captures to their profile (B22).
                feed.tick_away(&walk.sim, walk.player);
                game::heroes::scores(&mut walk.sim, feed);
            } else {
                game::heroes::tick(walk, feed);
            }
        }
        feed.note_medals(&walk.sim, walk.player, walk.local_slot, &mut st.medals_seen);
        if let Some(bb) = self.blackbox.as_deref_mut() {
            st.stuck_alarm |= bb.tick(walk, Some(st.tick_pad));
            note_sounds(bb, self.sounds.as_deref_mut(), walk.sim.tick);
        }
        if self.rumble_strength > 0.0 {
            if let Some(ext) = self.display_ext.filter(|_| !lockstep) {
                ext.on_sim_events(&super::xr::sim_view(walk), self.rumble_strength);
            }
            if let Some(pulse) = self.rumble_tick.pulse(
                &walk.sim,
                walk.player,
                st.previous_mount,
                self.rumble_on_fire,
            ) {
                if let Some(g) = self.gilrs.as_deref_mut() {
                    if let Some((id, _)) = input::active_gamepad(g, self.active_pad) {
                        *self.rumble_effect = input::rumble(g, id, pulse, self.rumble_strength);
                    }
                }
            }
        }
        st.previous_mount = walk.sim.unit(walk.player).and_then(|u| u.mount);
        if let Some(m) = self.matchlog.as_deref_mut() {
            m.follow_slot(walk.local_slot);
            m.tick(&walk.sim, &st.tick_pad);
            if walk.sim.outcome.is_some() && !m.written() {
                if let Some(p) = m.write(&walk.sim, "outcome") {
                    println!("matchlog: wrote {}", p.display());
                }
            }
        }
        if let Some(s) = self.session_stats.as_deref_mut() {
            s.tick(walk);
            if walk.sim.outcome.is_some() && !s.written() {
                if let Some(p) = s.write(walk, feed.round(), Some(feed), "outcome") {
                    println!("session stats: wrote {}", p.display());
                }
            }
        }
        if let Some(p) = self.frame_profile.as_deref_mut() {
            let hook = hook_t0.elapsed().as_secs_f64() * 1000.0;
            p.tick(walk.sim.ai_wall_ms, hook, &walk.sim.tick_cost, |u| {
                game::frame_profile::unit_name(walk, u)
            });
        }
        st.tick_pad.clear_once();
    }
}
