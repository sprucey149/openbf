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
    pub selection: Selection,
    pub medals_seen: core::sim::MedalCounts,
    pub previous_mount: Option<core::vehicle::Mount>,
    pub stuck_alarm: bool,
    pub tick_pad: core::sim::PadState,
    pub aspect: f32,
}

impl TickState {
    pub fn new(
        walk: &WalkSim,
        medals: core::sim::MedalCounts,
        pad: core::sim::PadState,
        aspect: f32,
    ) -> Self {
        Self {
            selection: Selection::read(walk),
            medals_seen: medals,
            previous_mount: walk.sim.unit(walk.player).and_then(|u| u.mount),
            stuck_alarm: false,
            tick_pad: pad,
            aspect,
        }
    }
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
    pub frame_profile: Option<&'a mut game::frame_profile::FrameProfile>,
}

impl TickHook<'_> {
    /// One stepped tick (module rules).
    pub fn tick(
        &mut self,
        walk: &mut WalkSim,
        feed: &mut game::event_feed::EventFeed,
        st: &mut TickState,
        lockstep: bool,
        muted: bool,
    ) {
        let hook_t0 = std::time::Instant::now();
        let local = walk.player.index;
        if let Some(gpu) = self.gpu.as_deref_mut() {
            gpu.step_stations(&walk.sim);
        }
        if let Some(vis) = self.soldier_vis.as_deref_mut() {
            vis.mute_shot(muted.then_some((walk.sim.tick, local)));
        }
        self.fx.ingest_posed(walk, self.soldier_vis.as_deref());
        let effect_sounds = self.fx.take_sounds();
        let buff_loops = self.fx.buff_loops(&walk.sim);
        if let Some(vis) = self.soldier_vis.as_deref_mut() {
            vis.note_shots(walk);
            let mut points = vis.fire_points(walk);
            self.deka_vis.fire_points(&walk.sim, &mut points);
            self.bolts.set_fire_points(points);
        }
        self.bolts.track(walk);
        if let (Some(audio), Some(sl)) = (self.audio.as_deref_mut(), self.sounds.as_deref_mut()) {
            let next = Selection::read(walk);
            if let Some(cue) = st.selection.movement_cue(next, walk.sim.ground_rules) {
                sl.play_2d(Some(audio), cue);
            }
            st.selection = next;
            play_event_sounds(
                audio,
                sl,
                self.foley.as_deref_mut(),
                self.engines,
                &effect_sounds,
                st.aspect,
                walk,
                muted.then_some(local),
            );
            // The auras' loops (the War Hero's crackle).
            let listener = game::event_sounds::listener(walk);
            sl.play_buff_loops(audio, &buff_loops, &listener);
            play_ambience(audio, sl, self.ambience, walk);
        }
        if !lockstep {
            game::heroes::tick(walk, feed);
        }
        feed.note_medals(&walk.sim, walk.player, walk.local_slot, &mut st.medals_seen);
        if let Some(bb) = self.blackbox.as_deref_mut() {
            st.stuck_alarm |= bb.tick(walk, Some(st.tick_pad));
            note_sounds(bb, self.sounds.as_deref_mut(), walk.sim.tick);
        }
        if self.rumble_strength > 0.0 {
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
            m.tick(&walk.sim, &st.tick_pad);
            if walk.sim.outcome.is_some() && !m.written() {
                if let Some(p) = m.write(&walk.sim, "outcome") {
                    println!("matchlog: wrote {}", p.display());
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
