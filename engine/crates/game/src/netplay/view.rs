//! [`LocalView`]: a lockstep peer's per-frame step and its drawn view (LAN
//! §4.4, §6), shared by the windowed loop and the tests. Presentation only:
//! the confirmed sim is only ever stepped by [`Peer`]'s frame.
//!
//! Per frame:
//! 1. [`LocalView::frame`] polls the host's self-check (a correction
//!    resets the prediction), then runs [`Peer::frame`]'s steps (inputs
//!    out, bundles stepped), handing the caller's per-tick hook whether the
//!    local unit's shot feedback this tick was already given by the
//!    prediction ([`Predictor::confirm_shots`]); then sets the render blend's fraction
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
//!
//! Several local players (issue #194 B18): [`LocalView::frame_locals`] takes
//! every local's pad, the predictor steps every local slot over its own
//! pending inputs, and each local has its own look pads. A further local's
//! view is drawn with [`LocalView::draw_in_local`] inside
//! `WalkSim::with_local`, so the blend and the walk's player are that
//! person's. With one local nothing differs.
//!
//! The per-local parts are found by slot, never by position: the walk's
//! locals are in adopt order and only grow, while the peer's further locals
//! (and so the predictor's tracks and the look pads here) follow
//! [`Peer::set_extra_slots`], which may drop or reorder them. A walk local
//! whose slot the peer no longer holds gets no prediction, no muted bit and
//! no look pads.

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
    /// Each further local's look pads and how many of them are pending.
    extra_look: Vec<(Vec<PadState>, usize)>,
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
        self.extra_look.clear();
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

    /// The look pads of the local player on `slot` (`WalkSim::local_slot_of`;
    /// the first player's are [`LocalView::look_pads`]); none for a slot the
    /// peer does not hold.
    pub fn look_pads_of_slot(&self, slot: u8) -> &[PadState] {
        match self.predictor.index_of_slot(slot) {
            Some(0) => &self.look_pads,
            Some(i) => self.extra_look.get(i - 1).map_or(&[], |(p, _)| p),
            None => &[],
        }
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
        self.frame_locals(peer, link, dt, &[pad], walk, feed, |w, b, f, muted| {
            each(w, b, f, muted & 1 != 0)
        })
    }

    /// [`LocalView::frame`] with every local player's pad, local 0 first
    /// (module header). `each`'s last argument has bit `i` set when local
    /// `i`'s shot feedback this tick was already given by the prediction.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_locals<L: Link>(
        &mut self,
        peer: &mut Peer,
        link: &mut L,
        dt: f32,
        pads: &[PadState],
        walk: &mut WalkSim,
        mut feed: Option<&mut EventFeed>,
        mut each: impl FnMut(&mut WalkSim, &TickBundle, Option<&mut EventFeed>, u8),
    ) -> FrameReport {
        // The host's game replaced by its replay: nothing predicted from
        // the old one holds. Before the frame's steps, whose bundles the
        // prediction then notes against the new game; the frame's only
        // poll of the check (`Peer::frame_steps`), so none goes unseen.
        let corrected = peer.poll_self_check(link, walk, feed.as_deref_mut());
        if corrected.is_some() {
            self.predictor.reset();
        }
        let extra_slots: Vec<u8> = peer.extra_locals().iter().map(|q| q.slot).collect();
        self.predictor.set_extra_slots(&extra_slots);
        let predictor = &mut self.predictor;
        let slot = peer.local_slot;
        let mut report = peer.frame_steps(link, f64::from(dt), pads, walk, feed, |w, b, f| {
            predictor.note_bundle(b, &w.sim, slot);
            let mut muted = u8::from(predictor.confirm_shots(&w.sim, w.player));
            for k in 1..w.local_count().min(8) {
                // An adopted local not seated yet has no shots of its own;
                // its part is the one predicting its slot (module header).
                let part = predictor
                    .index_of_slot(w.local_slot_of(k))
                    .filter(|&i| i > 0);
                if let (false, Some(i)) = (w.is_suspended(k), part) {
                    let unit = w.local_unit(k);
                    muted |= u8::from(predictor.confirm_shots_of(i, &w.sim, unit)) << k;
                }
            }
            each(w, b, f, muted);
        });
        report.corrected = corrected;
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
        // Each further local's look pads, as local 0's above.
        self.extra_look
            .resize_with(peer.extra_locals().len(), Default::default);
        for (k, (look, pending)) in self.extra_look.iter_mut().enumerate() {
            look.clear();
            *pending = 0;
            let Some(q) = peer.extra_locals().get(k) else {
                continue;
            };
            let seated = walk
                .sim
                .players
                .get(usize::from(q.slot))
                .is_some_and(|p| p.is_some());
            if seated {
                look.extend(q.pending_inputs().iter().copied());
                *pending = look.len();
                let pacer = q.pacer();
                look.extend(pacer.pending().iter().copied());
                if pacer.pending().is_empty() {
                    look.push(pacer.open());
                }
            }
        }
        let extra_pending: Vec<&[PadState]> =
            self.extra_look.iter().map(|(p, n)| &p[..*n]).collect();
        self.shots.clear();
        self.shots.extend_from_slice(self.predictor.update_locals(
            &walk.sim,
            peer.local_slot,
            &self.look_pads[..depth_pending],
            &extra_pending,
            dt.fmin(super::peer::MAX_INPUT_FRAME_S as f32),
        ));
        report
    }

    /// The predicted shots to feed back this frame of the local player on
    /// `slot` (`WalkSim::local_slot_of`; the first player's are
    /// [`LocalView::shots`]); none for a slot the peer does not hold.
    pub fn shots_of_slot(&self, slot: u8) -> &[PredictedShot] {
        match self.predictor.index_of_slot(slot) {
            Some(0) => &self.shots,
            Some(i) => self.predictor.shots_of(i),
            None => &[],
        }
    }

    /// The drawn view in (module header).
    pub fn draw_in(&mut self, walk: &mut WalkSim) {
        let (preview, drawn) = folded_look(&walk.sim, walk.player, &self.look_pads, self.share);
        blend_in_previewed(walk, preview, drawn);
        // After its own pending inputs in the pose: another local's longer
        // queue may make the copy deeper than this one's.
        let depth = self.predictor.depth_of(0).min(self.look_pads.len());
        self.predictor
            .swap_in(walk, self.share, &self.look_pads[depth..], self.share);
    }

    /// The drawn view in (module header) of the local player `walk` views
    /// now: inside `WalkSim::with_local(i)` that is local `i`, found by its
    /// slot (module header); outside it, [`LocalView::draw_in`]. Nothing for
    /// a slot the peer does not hold. [`LocalView::draw_out`] inside the
    /// same `with_local` puts it back.
    pub fn draw_in_local(&mut self, walk: &mut WalkSim) {
        let i = match self.predictor.index_of_slot(walk.local_slot_of(0)) {
            Some(0) => return self.draw_in(walk),
            Some(i) => i,
            None => return,
        };
        let Some((look, _)) = self.extra_look.get(i - 1) else {
            return;
        };
        let (preview, drawn) = folded_look(&walk.sim, walk.player, look, self.share);
        blend_in_previewed(walk, preview, drawn);
        let depth = self.predictor.depth_of(i).min(look.len());
        self.predictor
            .swap_in_local(i, walk, self.share, &look[depth..], self.share);
    }

    /// The drawn view out (module header); a no-op when nothing is in.
    pub fn draw_out(&mut self, walk: &mut WalkSim) {
        self.predictor.swap_out(walk);
        walk.blend_out();
    }
}
