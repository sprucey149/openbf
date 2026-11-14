//! [`LocalView`]: a lockstep peer's per-frame step and its drawn view (LAN
//! §4.4, §6), shared by the windowed loop and the tests. Presentation only:
//! the confirmed sim is only ever stepped by [`Peer::frame`].
//!
//! Per frame:
//! 1. [`LocalView::frame`] runs [`Peer::frame`] (inputs out, bundles
//!    stepped), handing the caller's per-tick hook whether the local unit's
//!    shot feedback this tick was already given by the prediction
//!    ([`Predictor::confirm_shots`]); then sets the render blend's fraction
//!    (a guest's playback fraction; the host's time since its last close,
//!    carried from tick to tick like single player's accumulator) and
//!    updates the prediction over the inputs still pending. The predicted
//!    shots to feed back now are [`LocalView::shots`].
//! 2. [`LocalView::draw_in`] before the draw lists: the render blend with
//!    the look folded over every pending input and the open input tick
//!    ([`folded_look`]; right after a frame sent the tick it closed, the
//!    open tick is that pad's held levels), then the predicted local pose
//!    over it ([`Predictor::swap_in`], drawn at the input clock's phase: the
//!    drawn soldier advances with the inputs sent, predicted or not).
//! 3. [`LocalView::draw_out`] after them (and before anything else reads
//!    the walk): the prediction, then the blend, written back.

use super::predict::{PredictedShot, Predictor};
use super::{FrameReport, Link, Peer};
use crate::event_feed::EventFeed;
use crate::render_blend::{blend_in_previewed, folded_look};
use crate::walk::WalkSim;
use core::fmath::FloatExt;
use core::sim::PadState;
use net::TickBundle;

/// A lockstep peer's drawn view (module header).
#[derive(Default)]
pub struct LocalView {
    pub predictor: Predictor,
    since_step: f32,
    frac: f32,
    share: f32,
    look_pads: Vec<PadState>,
    shots: Vec<PredictedShot>,
}

impl LocalView {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new round: nothing predicted, nothing to blend from.
    pub fn reset(&mut self) {
        self.predictor.reset();
        self.since_step = 0.0;
        self.look_pads.clear();
        self.shots.clear();
    }

    /// The confirmed sim's fraction toward its next tick.
    pub fn fraction(&self) -> f32 {
        self.frac
    }

    /// The open input tick's phase (the stick preview's share, the fire
    /// lag's reference).
    pub fn share(&self) -> f32 {
        self.share
    }

    /// The local unit's predicted shots to feed back this frame.
    pub fn shots(&self) -> &[PredictedShot] {
        &self.shots
    }

    /// The pads the look folds over: pending, then the open input tick.
    pub fn look_pads(&self) -> &[PadState] {
        &self.look_pads
    }

    /// One frame (module header). `each(walk, bundle, feed, muted)` runs
    /// after every stepped tick. `dt` is the frame's whole wall time (sim
    /// first, [`Peer::frame`]); the prediction's easing and budget take at
    /// most [`super::peer::MAX_INPUT_FRAME_S`] of it, as the windowed
    /// loop's other per-frame work does.
    #[allow(clippy::too_many_arguments)]
    pub fn frame<L: Link>(
        &mut self,
        peer: &mut Peer,
        link: &mut L,
        dt: f32,
        pad: PadState,
        walk: &mut WalkSim,
        feed: Option<&mut EventFeed>,
        mut each: impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>, bool),
    ) -> FrameReport {
        let predictor = &mut self.predictor;
        let slot = peer.local_slot;
        let report = peer.frame(link, f64::from(dt), pad, walk, feed, |w, b, f| {
            predictor.note_bundle(b, &w.sim, slot);
            let muted = predictor.confirm_shots(&w.sim, w.player);
            each(w, b, f, muted);
        });
        // The host steps each tick as its network thread closes it: the
        // time since the last close, kept like single player's accumulator
        // (each tick stepped takes one tick's time off, the rest carries) so
        // the fraction runs on evenly between the frames that step; held at
        // the next tick while a close is late, and never ahead of the last.
        self.since_step += dt - report.ticks as f32 * core::TICK_DT;
        self.since_step = self.since_step.clamp(0.0, core::TICK_DT);
        self.frac = if peer.is_host() {
            self.since_step * core::TICK_HZ as f32
        } else {
            report.blend as f32
        };
        walk.set_frame_fraction(self.frac);
        self.share = peer.pacer().phase().clamp(0.0, 1.0) as f32;
        self.look_pads.clear();
        // Until the local slot holds a unit (a joiner behind its catch-up)
        // the view is someone else's: nothing of ours is folded onto it.
        let seated = walk
            .sim
            .players
            .get(usize::from(peer.local_slot))
            .is_some_and(|p| *p == Some(walk.player));
        let mut depth_pending = 0;
        if seated {
            self.look_pads.extend(peer.pending_inputs().iter().copied());
            depth_pending = self.look_pads.len();
            // The open tick is always last: right after a frame closed one,
            // the levels it held (a held stick keeps turning through the
            // phase already elapsed).
            let pacer = peer.pacer();
            self.look_pads.extend(pacer.pending().iter().copied());
            if pacer.pending().is_empty() {
                self.look_pads.push(pacer.open());
            }
        }
        self.shots.clear();
        self.shots.extend_from_slice(self.predictor.update(
            &walk.sim,
            peer.local_slot,
            &self.look_pads[..depth_pending],
            dt.fmin(super::peer::MAX_INPUT_FRAME_S as f32),
        ));
        report
    }

    /// The drawn view in (module header).
    pub fn draw_in(&mut self, walk: &mut WalkSim) {
        let (preview, drawn) = folded_look(&walk.sim, walk.player, &self.look_pads, self.share);
        blend_in_previewed(walk, preview, drawn);
        let depth = self.predictor.depth().min(self.look_pads.len());
        self.predictor
            .swap_in(walk, self.share, &self.look_pads[depth..], self.share);
    }

    /// The drawn view out (module header); a no-op when nothing is in.
    pub fn draw_out(&mut self, walk: &mut WalkSim) {
        self.predictor.swap_out(walk);
        walk.blend_out();
    }
}
