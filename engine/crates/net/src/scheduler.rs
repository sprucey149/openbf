//! The host's per-slot input queue: how a human's inputs become the pad of
//! each closed tick.
//!
//! Rules:
//! - Inputs arrive with consecutive sequence numbers per connection and
//!   queue in order. Closing a tick pops one. Inputs are never dropped.
//! - An empty queue repeats the last pad with the once-only fields cleared
//!   (mouse deltas, fire lag, pick) and counts a starvation; the late input
//!   slips to the next tick. A cleared queue's last pad is the default one.
//! - A queue longer than its target + 3 merges its two oldest inputs
//!   ([`crate::bundle::merge_pads`]); a pair whose merge would lose a press
//!   edge or a pick is skipped. Past [`HARD_CAP`] (only a misbehaving peer)
//!   the oldest pair merges regardless.
//! - Each close records the depth before the pop: 0 means starved. The host
//!   reports depth and the starvation count back so the client can pace.
//! - Slack (input pacing, `LAN_MULTIPLAYER.md` §4.4): each input's arrival
//!   time is kept; a close records how long the popped input waited, and a
//!   starved close's input, when it arrives, how late it was (negative).
//!   Every [`SLACK_WINDOW`] closes the window's smallest value is published
//!   ([`SlotQueue::slack_us`]); the client paces so it stays just above 0.
//! - A slip: a close right after a starved one that finds two or more inputs
//!   (the late one and the on-time one) merges the oldest pair first, so one
//!   late input never leaves a standing tick of delay behind it.
//! - Consumption: a close records the sequence of the newest input it took
//!   ([`SlotQueue::consumed_seq`]; a merge counts as its newer input, a
//!   repeat takes none). The host's own inputs are numbered from 1 like a
//!   connection's. The tick's bundle carries it for every slot, so each
//!   peer knows which of its inputs a tick it steps already holds (its look
//!   preview and prediction run over the rest).
//! - One machine's local players (splitscreen) close together
//!   ([`SlotQueue::close_group`]) while their queues hold the same
//!   sequences: a slip or length merge happens at a pair only when every
//!   one of them can merge it, so all of them consume the same sequence on
//!   every tick. One queue alone closes exactly as [`SlotQueue::close`].
//!   "The same sequences" is the whole queued list
//!   ([`SlotQueue::same_queued`]), not only its length and ends.
//! - A local seated after its siblings (a late `AddHuman`, a local back
//!   after a lost pad) is lined up with a sibling that is already playing
//!   ([`SlotQueue::seat_beside`]): it consumes the sibling's sequence on the
//!   seating tick and queues its own received inputs for the sibling's
//!   queued sequences (a merged entry folds its run as a forced merge), so
//!   the two close as a group from the next tick on.
//! - Lag compensation (`LAN_MULTIPLAYER.md` §6): every input carries the
//!   confirmed tick its player was viewing. Closing tick `T` stamps the pad's
//!   `view_lag = min(cap, T - view_tick)` (0 when `cap` is 0). A merged pair
//!   keeps the newer input's view tick; a repeated pad keeps the last stamp.

use crate::bundle::{merge_pads, repeat_pad};
use crate::Micros;
use core::sim::PadState;
use std::collections::VecDeque;

/// Queue length past which merges stop checking for lost edges.
pub const HARD_CAP: usize = 1024;
/// Closes per published slack value.
pub const SLACK_WINDOW: u32 = 15;
/// Received inputs kept as delta bases.
const RECENT: usize = 64;

/// One slot's input FIFO.
#[derive(Clone, Debug)]
pub struct SlotQueue {
    /// Queued pads with the confirmed tick their player was viewing.
    fifo: VecDeque<(PadState, u32)>,
    /// Each queued pad's arrival time and input sequence.
    arrived: VecDeque<(Micros, u32)>,
    /// The sequence of the newest input a close consumed (a merge counts as
    /// its newer input; a repeat consumes none).
    consumed: u32,
    /// When the last close starved (its late input not yet arrived).
    starved_at: Option<Micros>,
    /// The last close starved.
    last_starved: bool,
    /// Slips merged away.
    pub slips: u32,
    window_min: Option<i64>,
    window_n: u32,
    slack: Option<i64>,
    last: PadState,
    /// Ticks closed with nothing queued.
    pub starved: u32,
    /// The queue depth before the last close's pop.
    pub depth: u8,
    pub target: usize,
    /// Pairs merged because the queue ran long.
    pub merges: u32,
    recv_through: u32,
    recent: VecDeque<(u32, PadState)>,
}

impl SlotQueue {
    pub fn new(target: usize) -> Self {
        Self {
            fifo: VecDeque::new(),
            arrived: VecDeque::new(),
            consumed: 0,
            starved_at: None,
            last_starved: false,
            slips: 0,
            window_min: None,
            window_n: 0,
            slack: None,
            last: PadState::default(),
            starved: 0,
            depth: 0,
            target,
            merges: 0,
            recv_through: 0,
            recent: VecDeque::new(),
        }
    }

    /// A new connection numbers its inputs from 1 again; anything queued from
    /// the old one is dropped (the slot was not playing in between).
    pub fn reset_stream(&mut self) {
        self.recv_through = 0;
        self.recent.clear();
        self.consumed = 0;
        self.clear();
    }

    /// Drops queued inputs and the last pad (a new round, an `AddHuman` or
    /// a new connection starts empty, so a starved first close repeats a
    /// neutral pad, not what was held when the slot last played).
    pub fn clear(&mut self) {
        self.last = PadState::default();
        self.fifo.clear();
        self.arrived.clear();
        self.starved_at = None;
        self.last_starved = false;
        self.window_min = None;
        self.window_n = 0;
        self.slack = None;
    }

    /// The last published slack (module rules), in microseconds: how early
    /// the window's latest input arrived before its close (negative: late).
    pub fn slack_us(&self) -> Option<i64> {
        self.slack
    }

    fn note_slack(&mut self, v: i64) {
        self.window_min = Some(self.window_min.map_or(v, |m| m.min(v)));
    }

    /// The newest input sequence a close consumed (`consumed` field).
    pub fn consumed_seq(&self) -> u32 {
        self.consumed
    }

    fn arrive(&mut self, pad: PadState, view: u32, now: Micros, seq: u32) {
        if let Some(at) = self.starved_at.take() {
            if self.fifo.is_empty() {
                self.note_slack(at.0 as i64 - now.0 as i64);
            }
        }
        self.fifo.push_back((pad, view));
        self.arrived.push_back((now, seq));
    }

    /// Starts this (new) queue beside `sib`, a local of the same machine:
    /// its next pushed input is numbered after `sib`'s last one, and every
    /// sequence `sib` still remembers counts as a neutral pad here (the
    /// player was not there yet), so [`SlotQueue::seat_beside`] finds the
    /// sibling's consumed sequence even when this queue is seated at once.
    pub fn start_beside(&mut self, sib: &SlotQueue) {
        self.recv_through = sib.recv_through;
        self.consumed = sib.recv_through;
        self.recent = sib
            .recent
            .iter()
            .map(|(s, _)| (*s, PadState::default()))
            .collect();
    }

    /// The highest input sequence received on this connection.
    pub fn recv_through(&self) -> u32 {
        self.recv_through
    }

    pub fn len(&self) -> usize {
        self.fifo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fifo.is_empty()
    }

    /// The pad of a received sequence, for decoding the next inputs (0 is the
    /// default pad).
    pub fn base(&self, seq: u32) -> Option<PadState> {
        if seq == 0 {
            return Some(PadState::default());
        }
        self.recent.iter().find(|(s, _)| *s == seq).map(|(_, p)| *p)
    }

    /// Takes decoded inputs `base_seq+1..`, each with its view tick. New ones
    /// are recorded; when `queue` is set they also join the FIFO (a slot not
    /// yet in the round acknowledges and discards them).
    pub fn receive(&mut self, base_seq: u32, inputs: &[(PadState, u32)], queue: bool, now: Micros) {
        for (i, (p, view)) in inputs.iter().enumerate() {
            let seq = base_seq.wrapping_add(1 + i as u32);
            if seq <= self.recv_through {
                continue;
            }
            if seq != self.recv_through + 1 {
                break;
            }
            self.recv_through = seq;
            self.recent.push_back((seq, *p));
            if self.recent.len() > RECENT {
                self.recent.pop_front();
            }
            if queue {
                self.arrive(*p, *view, now, seq);
            }
        }
    }

    /// The host's own input, with the confirmed tick its player was
    /// viewing: numbered after the last one, and queued when `queue` is set
    /// (every submission is numbered, as a connection numbers what it sends).
    pub fn push(&mut self, pad: PadState, view_tick: u32, now: Micros, queue: bool) {
        self.recv_through += 1;
        // Kept like a received input, so a host local seated late can line
        // up with a sibling ([`SlotQueue::seat_beside`]).
        self.recent.push_back((self.recv_through, pad));
        if self.recent.len() > RECENT {
            self.recent.pop_front();
        }
        if queue {
            self.arrive(pad, view_tick, now, self.recv_through);
        }
    }

    /// Closes tick `tick` at `now`: the pad this slot steps with, its
    /// `view_lag` stamped against `lag_cap` (module rules).
    pub fn close(&mut self, tick: u32, lag_cap: u8, now: Micros) -> PadState {
        self.depth = self.fifo.len().min(255) as u8;
        if self.last_starved && self.fifo.len() >= 2 && self.merge_at(0, false) {
            self.slips += 1;
        }
        let pad = self.pop_close(tick, lag_cap, now);
        while self.fifo.len() > self.target + 3 {
            if !self.merge_one() {
                break;
            }
        }
        pad
    }

    /// Whether `other` queues exactly the same input sequences under the
    /// same target: queues of one machine's locals that do hold the same
    /// inputs and close as a group.
    pub fn same_queued(&self, other: &SlotQueue) -> bool {
        self.target == other.target
            && self.arrived.len() == other.arrived.len()
            && self
                .arrived
                .iter()
                .zip(&other.arrived)
                .all(|(a, b)| a.1 == b.1)
    }

    /// Lines this queue up with `sib`, a sibling local of the same machine
    /// that is already playing and has closed this tick (module rules).
    /// Clears the queue, then returns the pad this slot steps with on the
    /// seating tick: its own input at the sibling's consumed sequence (a
    /// repeat of it when the sibling starved), stamped with `view_lag`.
    /// `None` when its received inputs no longer cover the sibling's (it
    /// is then left cleared and closes alone).
    pub fn seat_beside(&mut self, sib: &SlotQueue, view_lag: u8) -> Option<PadState> {
        self.clear();
        let own = self.base(sib.consumed)?;
        let mut fifo = VecDeque::with_capacity(sib.fifo.len());
        let mut prev = sib.consumed;
        for ((_, view), (_, seq)) in sib.fifo.iter().zip(&sib.arrived) {
            let mut run = (prev + 1..=*seq).map(|s| self.base(s));
            let first = run.next()??;
            let mut m = first;
            for p in run {
                m = force_merge(&m, &p?);
            }
            fifo.push_back((m, *view));
            prev = *seq;
        }
        self.fifo = fifo;
        self.arrived = sib.arrived.clone();
        self.consumed = sib.consumed;
        let mut pad = if sib.last_starved {
            repeat_pad(&own)
        } else {
            own
        };
        pad.view_lag = view_lag;
        self.last = pad;
        Some(pad)
    }

    /// Closes tick `tick` for every queue of one machine's locals at once
    /// (module rules): `qs` must agree on [`SlotQueue::same_queued`].
    /// Returns each queue's pad in order.
    pub fn close_group(
        qs: &mut [&mut SlotQueue],
        tick: u32,
        lag_cap: u8,
        now: Micros,
    ) -> Vec<PadState> {
        debug_assert!(qs.windows(2).all(|w| w[0].same_queued(w[1])));
        let Some(len) = qs.first().map(|q| q.fifo.len()) else {
            return Vec::new();
        };
        let slip = len >= 2 && qs.iter().all(|q| q.last_starved);
        for q in qs.iter_mut() {
            q.depth = len.min(255) as u8;
        }
        if slip && Self::merge_group(qs, 0, false) {
            for q in qs.iter_mut() {
                q.slips += 1;
            }
        }
        let pads: Vec<PadState> = qs
            .iter_mut()
            .map(|q| q.pop_close(tick, lag_cap, now))
            .collect();
        loop {
            let (len, target) = (qs[0].fifo.len(), qs[0].target);
            if len <= target + 3 {
                break;
            }
            let force = len > HARD_CAP;
            if !(0..len - 1).any(|i| Self::merge_group(qs, i, force)) {
                break;
            }
        }
        pads
    }

    /// Merges pair `i` in every queue when every one of them can; whether
    /// it did.
    fn merge_group(qs: &mut [&mut SlotQueue], i: usize, force: bool) -> bool {
        let merged: Option<Vec<PadState>> = qs.iter().map(|q| q.try_merge(i, force)).collect();
        let Some(merged) = merged else {
            return false;
        };
        for (q, m) in qs.iter_mut().zip(merged) {
            q.apply_merge(i, m);
        }
        true
    }

    /// The pop half of a close (after any slip merge, before the length
    /// merges).
    fn pop_close(&mut self, tick: u32, lag_cap: u8, now: Micros) -> PadState {
        let popped = self.fifo.pop_front();
        if let Some((at, seq)) = self.arrived.pop_front() {
            self.note_slack(now.0 as i64 - at.0 as i64);
            self.consumed = self.consumed.max(seq);
        }
        self.last_starved = popped.is_none();
        if popped.is_none() {
            self.starved_at = Some(now);
        }
        self.window_n += 1;
        if self.window_n >= SLACK_WINDOW {
            self.window_n = 0;
            if let Some(m) = self.window_min.take() {
                self.slack = Some(m);
            }
        }
        let pad = match popped {
            Some((mut p, view)) => {
                p.view_lag = u8::try_from(tick.saturating_sub(view))
                    .unwrap_or(u8::MAX)
                    .min(lag_cap);
                p
            }
            None => {
                self.starved = self.starved.wrapping_add(1);
                repeat_pad(&self.last)
            }
        };
        self.last = pad;
        pad
    }

    fn merge_one(&mut self) -> bool {
        let force = self.fifo.len() > HARD_CAP;
        (0..self.fifo.len() - 1).any(|i| self.merge_at(i, force))
    }

    /// Merges queued pads `i` and `i + 1` (forced past the hard cap);
    /// whether it did.
    fn merge_at(&mut self, i: usize, force: bool) -> bool {
        match self.try_merge(i, force) {
            Some(m) => {
                self.apply_merge(i, m);
                true
            }
            None => false,
        }
    }

    /// The merge of queued pads `i` and `i + 1`, if allowed (forced past
    /// the hard cap).
    fn try_merge(&self, i: usize, force: bool) -> Option<PadState> {
        {
            let prior = if i == 0 {
                self.last
            } else {
                self.fifo[i - 1].0
            };
            let merged = merge_pads(
                &prior,
                &self.fifo[i].0,
                &self.fifo[i + 1].0,
                self.fifo.get(i + 2).map(|(p, _)| p),
            );
            match (merged, force) {
                (Some(m), _) => Some(m),
                (None, true) => Some(force_merge(&self.fifo[i].0, &self.fifo[i + 1].0)),
                (None, false) => None,
            }
        }
    }

    fn apply_merge(&mut self, i: usize, merged: PadState) {
        // The newer input's view tick; the older arrival.
        self.fifo[i] = (merged, self.fifo[i + 1].1);
        self.fifo.remove(i + 1);
        if let Some((_, seq)) = self.arrived.remove(i + 1) {
            self.arrived[i].1 = seq;
        }
        self.merges += 1;
    }
}

/// The lossy merge past the hard cap: the newer pad with the older one's
/// buttons, mouse deltas and pick folded in.
fn force_merge(a: &PadState, b: &PadState) -> PadState {
    let mut m = *b;
    m.buttons |= a.buttons;
    m.mx = a.mx.saturating_add(b.mx);
    m.my = a.my.saturating_add(b.my);
    m.pick = a.pick.or(b.pick);
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::buttons::FIRE;

    fn pad(mx: i32, buttons: u32) -> (PadState, u32) {
        (
            PadState {
                mx,
                buttons,
                ..PadState::default()
            },
            0,
        )
    }

    #[test]
    fn scheduler_repeats_without_once_fields_and_counts_starvation() {
        let mut q = SlotQueue::new(1);
        q.push(
            PadState {
                mx: 5,
                lx: 100,
                fire_lag: 9,
                buttons: FIRE,
                ..PadState::default()
            },
            0,
            Micros::ZERO,
            true,
        );
        assert_eq!(q.close(0, 0, Micros::ZERO).mx, 5);
        let r = q.close(1, 0, Micros::ZERO);
        assert_eq!((r.mx, r.lx, r.fire_lag, r.buttons), (0, 100, 0, FIRE));
        assert_eq!(q.starved, 1);
        assert_eq!(q.depth, 0);
    }

    /// A cleared queue (a new round, an `AddHuman`, a new connection)
    /// repeats a neutral pad, not the buttons and sticks held at the end of
    /// the last round.
    #[test]
    fn scheduler_clear_forgets_the_last_pad() {
        use core::sim::buttons::JUMP;
        let held = PadState {
            lx: 900,
            ly: -400,
            rx: 300,
            buttons: FIRE | JUMP,
            ..PadState::default()
        };
        for reset in [false, true] {
            let mut q = SlotQueue::new(1);
            q.push(held, 0, Micros::ZERO, true);
            assert_eq!(q.close(0, 0, Micros::ZERO), held);
            if reset {
                q.reset_stream();
            } else {
                q.clear();
            }
            assert_eq!(q.close(0, 0, Micros::ZERO), PadState::default());
        }
    }

    #[test]
    fn scheduler_merges_long_queues_keeping_deltas_and_edges() {
        let mut q = SlotQueue::new(1);
        let pads: Vec<_> = (0..8)
            .map(|i| pad(i + 1, if i % 2 == 0 { FIRE } else { 0 }))
            .collect();
        q.receive(0, &pads, true, Micros::ZERO);
        let mut out = vec![q.close(0, 0, Micros::ZERO)];
        while !q.is_empty() {
            out.push(q.close(0, 0, Micros::ZERO));
        }
        let sum: i32 = out.iter().map(|p| p.mx).sum();
        assert_eq!(sum, (1..=8).sum::<i32>());
        let edges = |v: &[PadState]| {
            let mut prev = 0;
            v.iter()
                .filter(|p| {
                    let e = p.buttons & FIRE != 0 && prev & FIRE == 0;
                    prev = p.buttons;
                    e
                })
                .count()
        };
        let pads: Vec<_> = pads.iter().map(|(p, _)| *p).collect();
        assert_eq!(edges(&out), edges(&pads));
    }

    #[test]
    fn scheduler_merges_plain_inputs_when_long() {
        let mut q = SlotQueue::new(1);
        let pads: Vec<_> = (0..10).map(|i| pad(i, 0)).collect();
        q.receive(0, &pads, true, Micros::ZERO);
        q.close(0, 0, Micros::ZERO);
        assert!(q.merges > 0);
        assert!(q.len() <= 4);
        let mut sum = 0;
        while !q.is_empty() {
            sum += q.close(0, 0, Micros::ZERO).mx;
        }
        assert_eq!(sum, (1..10).sum::<i32>());
    }

    /// A group merges a pair only where every queue can: one local's FIRE
    /// edges block a pair the other could merge, and both consume the same
    /// sequences; a lone queue closes as before.
    #[test]
    fn scheduler_group_merges_only_where_all_can() {
        let plain: Vec<_> = (0..10).map(|i| pad(i, 0)).collect();
        let edgy: Vec<_> = (0..10)
            .map(|i| pad(i, if i % 2 == 1 { FIRE } else { 0 }))
            .collect();
        let (mut a, mut b) = (SlotQueue::new(1), SlotQueue::new(1));
        a.receive(0, &plain, true, Micros::ZERO);
        b.receive(0, &edgy, true, Micros::ZERO);
        let mut solo = a.clone();
        let mut sums = (0, 0);
        while !a.is_empty() {
            assert!(a.same_queued(&b));
            let pads = SlotQueue::close_group(&mut [&mut a, &mut b], 0, 0, Micros::ZERO);
            assert_eq!(a.consumed_seq(), b.consumed_seq());
            assert_eq!(a.len(), b.len());
            sums.0 += pads[0].mx;
            sums.1 += pads[1].mx;
        }
        assert!(b.is_empty());
        assert_eq!(sums, ((0..10).sum::<i32>(), (0..10).sum::<i32>()));
        // Every other pad presses FIRE: no pair of `b` merges, so neither
        // does `a`'s, though alone it would.
        assert_eq!((a.merges, b.merges), (0, 0));
        solo.close(0, 0, Micros::ZERO);
        assert!(solo.merges > 0);
    }

    /// Queues that agree on count and both ends but merged different
    /// pairs in between are not the same queue.
    #[test]
    fn scheduler_same_queued_compares_every_sequence() {
        let mut a = SlotQueue::new(0);
        let mut b = SlotQueue::new(0);
        a.receive(
            0,
            &(0..4).map(|i| pad(i, 0)).collect::<Vec<_>>(),
            true,
            Micros::ZERO,
        );
        b.receive(
            0,
            &(0..4).map(|i| pad(i, 0)).collect::<Vec<_>>(),
            true,
            Micros::ZERO,
        );
        assert!(a.same_queued(&b));
        // Sequences 1..=4, merged so `a` holds [2, 4] and `b` [3, 4].
        a.apply_merge(0, pad(0, 0).0);
        a.apply_merge(1, pad(0, 0).0);
        b.apply_merge(1, pad(0, 0).0);
        b.apply_merge(0, pad(0, 0).0);
        let seqs = |q: &SlotQueue| q.arrived.iter().map(|x| x.1).collect::<Vec<_>>();
        assert_eq!((seqs(&a), seqs(&b)), (vec![2, 4], vec![3, 4]));
        assert!(!a.same_queued(&b));
        let mut c = a.clone();
        c.target = 1;
        assert!(!a.same_queued(&c));
    }

    /// A local seated beside a sibling that holds queued (some merged)
    /// inputs takes the sibling's consumed sequence and queues its own
    /// inputs for the same sequences, so the two close as a group after.
    #[test]
    fn scheduler_seat_beside_lines_up_with_the_sibling() {
        let own: Vec<_> = (1..=12).map(|i| pad(100 + i, 0)).collect();
        let sib_in: Vec<_> = (1..=12).map(|i| pad(i, 0)).collect();
        let mut sib = SlotQueue::new(1);
        let mut late = SlotQueue::new(1);
        sib.receive(0, &sib_in, true, Micros::ZERO);
        // The late slot received the same sequences while not seated.
        late.receive(0, &own, false, Micros::ZERO);
        assert!(late.is_empty());
        sib.close(0, 0, Micros::ZERO);
        assert!(sib.merges > 0 && !sib.is_empty());
        let lag = 3;
        let pad0 = late.seat_beside(&sib, lag).expect("covered");
        assert_eq!(late.consumed_seq(), sib.consumed_seq());
        assert_eq!(pad0.mx, 100 + sib.consumed_seq() as i32);
        assert_eq!(pad0.view_lag, lag);
        assert!(late.same_queued(&sib));
        // Every own input after the seating tick's is still in the queue.
        let left: i32 = (sib.consumed_seq() as i32 + 1..=12).map(|i| 100 + i).sum();
        assert_eq!(late.fifo.iter().map(|p| p.0.mx).sum::<i32>(), left);
        let mut got = 0;
        while !sib.is_empty() {
            let pads = SlotQueue::close_group(&mut [&mut sib, &mut late], 1, 0, Micros::ZERO);
            assert_eq!(sib.consumed_seq(), late.consumed_seq());
            got += pads[1].mx;
        }
        assert_eq!(got, left);
        // Inputs no longer held: it cannot line up and stays cleared.
        let mut gone = SlotQueue::new(1);
        gone.receive(0, &[pad(1, 0)], false, Micros::ZERO);
        assert!(gone.seat_beside(&sib, 0).is_none());
        assert!(gone.is_empty());
    }

    /// Four locals with independent FIRE traffic, fed in bursts that keep
    /// the queues long: how far the group's queues run past a lone queue's
    /// and how many merges each makes. A group may only merge where all
    /// four can, so it holds more, but stays bounded.
    #[test]
    fn scheduler_four_locals_depth_against_a_lone_queue() {
        let mut rng = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut group: Vec<SlotQueue> = (0..4).map(|_| SlotQueue::new(2)).collect();
        let mut lone = SlotQueue::new(2);
        let mut held = [false; 4];
        let (mut seq, mut tick) = (0u32, 0u32);
        let (mut gsum, mut lsum, mut gmax, mut lmax) = (0usize, 0usize, 0usize, 0usize);
        let ticks = 6_000;
        for _ in 0..ticks {
            // Arrivals: 0-3 inputs a tick, one a tick on average, with a
            // standing backlog of a few.
            let n = match next() % 8 {
                0 | 1 => 0,
                2..=5 => 1,
                6 => 2,
                _ => 3,
            };
            for _ in 0..n {
                let mut rows = Vec::new();
                for h in held.iter_mut() {
                    // A press about every 10 inputs, held a few.
                    if next() % 10 == 0 {
                        *h = !*h;
                    }
                    rows.push(pad((next() % 21) as i32 - 10, if *h { FIRE } else { 0 }));
                }
                for (q, r) in group.iter_mut().zip(&rows) {
                    q.receive(seq, &[*r], true, Micros::ZERO);
                }
                lone.receive(seq, &[rows[0]], true, Micros::ZERO);
                seq += 1;
            }
            let mut qs: Vec<&mut SlotQueue> = group.iter_mut().collect();
            SlotQueue::close_group(&mut qs, tick, 0, Micros::ZERO);
            lone.close(tick, 0, Micros::ZERO);
            assert!(group.windows(2).all(|w| w[0].same_queued(&w[1])));
            gsum += group[0].len();
            lsum += lone.len();
            gmax = gmax.max(group[0].len());
            lmax = lmax.max(lone.len());
            tick += 1;
        }
        let (gmean, lmean) = (gsum as f64 / ticks as f64, lsum as f64 / ticks as f64);
        println!(
            "4 locals: mean depth {gmean:.2} max {gmax} merges {} | lone: mean {lmean:.2} max {lmax} merges {}",
            group[0].merges, lone.merges
        );
        assert!(gmax < HARD_CAP, "no forced merges");
        assert!(gmean < lmean + 2.0, "group {gmean:.2} lone {lmean:.2}");
    }

    #[test]
    fn scheduler_ignores_duplicate_and_gapped_inputs() {
        let mut q = SlotQueue::new(1);
        q.receive(0, &[pad(1, 0), pad(2, 0)], true, Micros::ZERO);
        q.receive(1, &[pad(2, 0), pad(3, 0)], true, Micros::ZERO);
        q.receive(5, &[pad(9, 0)], true, Micros::ZERO);
        assert_eq!(q.recv_through(), 3);
        assert_eq!(q.len(), 3);
        assert_eq!(q.base(2).unwrap().mx, 2);
    }

    #[test]
    fn scheduler_stamps_view_lag_from_the_view_tick() {
        let mut q = SlotQueue::new(8);
        let at = |view| (PadState::default(), view);
        q.receive(0, &[at(97), at(99), at(100), at(80)], true, Micros::ZERO);
        // Tick 100 viewed at 97: 3 ticks behind.
        assert_eq!(q.close(100, 8, Micros::ZERO).view_lag, 3);
        assert_eq!(q.close(101, 8, Micros::ZERO).view_lag, 2);
        // A view tick at or past the close is no lag.
        assert_eq!(q.close(100, 8, Micros::ZERO).view_lag, 0);
        // The cap bounds it.
        assert_eq!(q.close(103, 8, Micros::ZERO).view_lag, 8);
        // A starved close repeats the last stamp.
        assert_eq!(q.close(104, 8, Micros::ZERO).view_lag, 8);
        assert_eq!(q.starved, 1);
        // Cap 0: lag compensation off.
        q.push(PadState::default(), 10, Micros::ZERO, true);
        assert_eq!(q.close(105, 0, Micros::ZERO).view_lag, 0);
        // A merged pair keeps the newer input's view tick.
        let mut m = SlotQueue::new(0);
        m.receive(0, &[at(1), at(2), at(3), at(4), at(5)], true, Micros::ZERO);
        let first = m.close(10, 255, Micros::ZERO);
        assert_eq!(first.view_lag, 9);
        assert!(m.merges > 0);
        let mut last = first;
        while !m.is_empty() {
            last = m.close(10, 255, Micros::ZERO);
        }
        assert_eq!(last.view_lag, 5);
    }

    /// The published slack is the window's smallest wait; a starved close's
    /// late input counts as negative; a slip right after merges away.
    #[test]
    fn scheduler_publishes_slack_and_merges_a_slip() {
        let ms = Micros::from_millis;
        let mut q = SlotQueue::new(1);
        // Inputs 5 ms (then 2 ms) before each close at 33 ms intervals.
        for k in 0..SLACK_WINDOW {
            let close = ms(33 * u64::from(k) + 33);
            let early = if k == 4 { 2 } else { 5 };
            q.push(PadState::default(), k, close - ms(early), true);
            q.close(k, 0, close);
        }
        assert_eq!(q.slack_us(), Some(2_000));
        assert_eq!(q.starved, 0);
        // Close 15 starves; its input arrives 4 ms late, the next on time.
        let t = |k: u64| ms(33 * k + 33);
        q.close(15, 0, t(15));
        assert_eq!(q.starved, 1);
        q.push(pad(7, FIRE).0, 15, t(15) + ms(4), true);
        q.push(pad(3, FIRE).0, 16, t(16) - ms(5), true);
        let p = q.close(16, 0, t(16));
        assert_eq!((p.mx, p.buttons & FIRE), (10, FIRE), "the slip merged");
        assert_eq!(q.slips, 1);
        assert!(q.is_empty(), "no standing tick left behind");
        for k in 17..30 {
            q.push(PadState::default(), k as u32, t(k) - ms(5), true);
            q.close(k as u32, 0, t(k));
        }
        assert_eq!(q.slack_us(), Some(-4_000));
    }

    /// A close records the newest input sequence it took: a merge counts as
    /// its newer input, a repeat takes none.
    #[test]
    fn scheduler_reports_the_consumed_sequence() {
        let mut q = SlotQueue::new(0);
        q.receive(0, &[pad(1, 0), pad(2, 0)], true, Micros::ZERO);
        q.close(0, 0, Micros::ZERO);
        assert_eq!(q.consumed_seq(), 1);
        q.close(1, 0, Micros::ZERO);
        q.close(2, 0, Micros::ZERO);
        assert_eq!((q.consumed_seq(), q.starved), (2, 1));
        // After the starvation a late input and an on-time one merge.
        q.receive(2, &[pad(3, 0), pad(4, 0)], true, Micros::ZERO);
        q.close(3, 0, Micros::ZERO);
        assert_eq!((q.consumed_seq(), q.slips), (4, 1));
        // The host's own inputs are numbered too, queued or not.
        let mut own = SlotQueue::new(0);
        own.push(PadState::default(), 0, Micros::ZERO, false);
        own.push(PadState::default(), 0, Micros::ZERO, true);
        own.close(0, 0, Micros::ZERO);
        assert_eq!(own.consumed_seq(), 2);
    }
}
