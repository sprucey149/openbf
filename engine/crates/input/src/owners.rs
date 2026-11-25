//! Who owns which controller when several people share a machine (issue #194,
//! `analysis/plan/SPLITSCREEN.md` §4).
//!
//! One player index per person, 0 the first. A gamepad belongs to at most one player; a gamepad's
//! input drives that player and no other, so a second player's pad can never steal the first
//! player's (the single-player [`crate::ActivePad`] follows whichever pad moved last, which is
//! exactly wrong here). The keyboard and mouse belong to at most one player, since the system
//! merges every keyboard into one key stream and every mouse into one pointer; that player may
//! also own a pad, as a single player does today. Generic over the pad id so it tests without
//! hardware.

/// The most people on one machine.
pub const MAX_LOCAL_PLAYERS: usize = 4;

/// Pad and keyboard ownership of up to [`MAX_LOCAL_PLAYERS`] players.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadOwners<Id> {
    pads: [Option<Id>; MAX_LOCAL_PLAYERS],
    /// Which player indexes are seated (joined), in any order of joining.
    seated: [bool; MAX_LOCAL_PLAYERS],
    keyboard: Option<usize>,
    /// Seated players whose pad was unplugged and has not come back (a keyboard player does not
    /// count: they still play on the keyboard).
    lost: [bool; MAX_LOCAL_PLAYERS],
    /// The pad each lost seat held, so the same pad coming back finds its own seat first.
    last: [Option<Id>; MAX_LOCAL_PLAYERS],
}

impl<Id> Default for PadOwners<Id> {
    fn default() -> Self {
        Self {
            pads: std::array::from_fn(|_| None),
            seated: [false; MAX_LOCAL_PLAYERS],
            keyboard: None,
            lost: [false; MAX_LOCAL_PLAYERS],
            last: std::array::from_fn(|_| None),
        }
    }
}

impl<Id: Copy + PartialEq> PadOwners<Id> {
    /// The player a pad drives, if any.
    pub fn player_of(&self, pad: Id) -> Option<usize> {
        self.pads.iter().position(|p| *p == Some(pad))
    }

    /// The pad player `i` holds, if any (none while it is unplugged).
    pub fn pad_of(&self, i: usize) -> Option<Id> {
        self.pads.get(i).copied().flatten()
    }

    /// The player who has the keyboard and mouse, if any.
    pub fn keyboard_player(&self) -> Option<usize> {
        self.keyboard
    }

    /// Is player `i` seated?
    pub fn is_seated(&self, i: usize) -> bool {
        self.seated.get(i).copied().unwrap_or(false)
    }

    /// How many people are seated.
    pub fn count(&self) -> usize {
        self.seated.iter().filter(|s| **s).count()
    }

    fn lowest_free(&self) -> Option<usize> {
        self.seated.iter().position(|s| !s)
    }

    /// A pad pressed its join button: seat a new player on it, or return the player it already
    /// drives. `None` when four people are seated.
    pub fn join_pad(&mut self, pad: Id) -> Option<usize> {
        if let Some(i) = self.player_of(pad) {
            return Some(i);
        }
        let i = self.lowest_free()?;
        self.seated[i] = true;
        self.pads[i] = Some(pad);
        Some(i)
    }

    /// The keyboard player joins (Enter): seat a new player on the keyboard and mouse, or return
    /// the player who already has them. `None` when four are seated.
    pub fn join_keyboard(&mut self) -> Option<usize> {
        if let Some(i) = self.keyboard {
            return Some(i);
        }
        let i = self.lowest_free()?;
        self.seated[i] = true;
        self.keyboard = Some(i);
        Some(i)
    }

    /// A pad is plugged in again, or the keyboard player picks up a pad: give player `i` the pad
    /// if it is free (not another player's) and `i` has none.
    pub fn bind_pad(&mut self, i: usize, pad: Id) -> bool {
        if !self.is_seated(i) || self.pads[i].is_some() || self.player_of(pad).is_some() {
            return false;
        }
        self.pads[i] = Some(pad);
        self.lost[i] = false;
        true
    }

    /// A pad was unplugged: its player keeps the seat and has no pad. Returns that player, so
    /// their screen (and only theirs) shows the notice while the others play on.
    pub fn disconnected(&mut self, pad: Id) -> Option<usize> {
        let i = self.player_of(pad)?;
        self.pads[i] = None;
        self.lost[i] = self.keyboard != Some(i);
        self.last[i] = Some(pad);
        Some(i)
    }

    /// The pad lost seat `i` held, while it waits for it: the Players screen tells the same
    /// pad coming back from another pad taking the seat.
    pub fn lost_pad(&self, i: usize) -> Option<Id> {
        if self.is_lost(i) {
            self.last.get(i).copied().flatten()
        } else {
            None
        }
    }

    /// Does player `i` wait for a pad to come back? Their screen says so, and only theirs.
    pub fn is_lost(&self, i: usize) -> bool {
        self.lost.get(i).copied().unwrap_or(false)
    }

    /// A pad nobody owns pressed a button while a player waits for one: it takes the waiting
    /// seat it held before (the same id coming back), else the lowest waiting seat, so a pad
    /// that comes back under a new id (a replugged or re-paired controller) still returns to its
    /// player when only one waits. This is the one way a pad the match has not seated enters it.
    /// `None` when no seat waits, or the pad already belongs to someone.
    pub fn reclaim(&mut self, pad: Id) -> Option<usize> {
        self.reclaim_where(pad, |_| true)
    }

    /// [`PadOwners::reclaim`], where a pad that is not a seat's own lost pad takes only a waiting
    /// seat `open(i)` allows (issue #194: in battle, not a seat whose own pad is plugged back in
    /// and has yet to press, so no other pad takes that player's cell and profile).
    pub fn reclaim_where(&mut self, pad: Id, open: impl Fn(usize) -> bool) -> Option<usize> {
        if self.player_of(pad).is_some() {
            return None;
        }
        let i = self
            .lost
            .iter()
            .enumerate()
            .position(|(i, l)| *l && self.last[i] == Some(pad))
            .or_else(|| (0..self.lost.len()).find(|&i| self.lost[i] && open(i)))?;
        self.pads[i] = Some(pad);
        self.lost[i] = false;
        Some(i)
    }

    /// The waiting seats a returning pad could be (issue #194): every lost seat whose lost pad
    /// is `alike` the returning one, the same model. The platform hands a pad that comes back the
    /// old id of a disconnected pad of its model, the lowest such id, and gives nothing else to
    /// tell two identical pads apart (no serial, no device path). With two or more such seats the
    /// pad's id says nothing about whose it is, and [`PadOwners::reclaim_where`] would trust it:
    /// the caller asks the person instead ([`PadOwners::reclaim_seat`]). With one, the pad is
    /// that seat's whatever its id.
    pub fn alike_waiting(&self, alike: impl Fn(Id) -> bool) -> Vec<usize> {
        (0..MAX_LOCAL_PLAYERS)
            .filter(|&i| self.lost_pad(i).is_some_and(&alike))
            .collect()
    }

    /// Waiting seat `i` takes `pad`, picked by the person holding it (from
    /// [`PadOwners::alike_waiting`]). `false` when the pad is someone's or `i` does not wait.
    pub fn reclaim_seat(&mut self, pad: Id, i: usize) -> bool {
        if self.player_of(pad).is_some() || !self.is_lost(i) {
            return false;
        }
        self.pads[i] = Some(pad);
        self.lost[i] = false;
        true
    }

    /// Seat `i` as `other` has it, when it is free here and seated there (its pad only when no
    /// player here holds it): a LAN round hands back the seats it left out (issue #194 B20).
    pub fn take_seat_from(&mut self, other: &Self, i: usize) {
        if i >= MAX_LOCAL_PLAYERS || self.seated[i] || !other.seated[i] {
            return;
        }
        self.seated[i] = true;
        self.pads[i] = other.pads[i].filter(|p| self.player_of(*p).is_none());
        self.lost[i] = other.lost[i];
        self.last[i] = other.last[i];
        if other.keyboard == Some(i) && self.keyboard.is_none() {
            self.keyboard = Some(i);
        }
    }

    /// Player `i` leaves: the seat, the pad and the keyboard are free again.
    pub fn leave(&mut self, i: usize) {
        if i >= MAX_LOCAL_PLAYERS {
            return;
        }
        self.seated[i] = false;
        self.pads[i] = None;
        self.lost[i] = false;
        if self.keyboard == Some(i) {
            self.keyboard = None;
        }
    }

    /// The seats with the holes closed, for a match whose local players are contiguous: the
    /// seated players in seat order become players 0..count, each with their own pad, keyboard,
    /// lost mark and remembered pad. The second value is each packed player's seat here
    /// ([`PadOwners::unpacked`] puts them back). The seats themselves are not changed.
    pub fn packed(&self) -> (Self, Vec<usize>) {
        let mut out = Self::default();
        let mut from = Vec::new();
        for i in (0..MAX_LOCAL_PLAYERS).filter(|i| self.seated[*i]) {
            let k = from.len();
            out.seated[k] = true;
            out.pads[k] = self.pads[i];
            out.lost[k] = self.lost[i];
            out.last[k] = self.last[i];
            if self.keyboard == Some(i) {
                out.keyboard = Some(k);
            }
            from.push(i);
        }
        (out, from)
    }

    /// [`PadOwners::packed`] in a given order: player `k` here is seat `order[k]` (a LAN round's
    /// locals come in the session's slot order, issue #194 B20). A seat in `order` that is not
    /// seated (or repeats) gives a seated player with nothing to play with.
    pub fn picked(&self, order: &[usize]) -> Self {
        let mut out = Self::default();
        let mut used = [false; MAX_LOCAL_PLAYERS];
        for (k, &i) in order.iter().enumerate().take(MAX_LOCAL_PLAYERS) {
            out.seated[k] = true;
            if i >= MAX_LOCAL_PLAYERS || used[i] || !self.seated[i] {
                continue;
            }
            used[i] = true;
            out.pads[k] = self.pads[i];
            out.lost[k] = self.lost[i];
            out.last[k] = self.last[i];
            if self.keyboard == Some(i) {
                out.keyboard = Some(k);
            }
        }
        out
    }

    /// The inverse of [`PadOwners::packed`]: player `k` here goes back to seat `from[k]`, so the
    /// Players screen gets the same seat numbers it sent. A player with no seat in `from` (or a
    /// clash) takes the lowest seat still free.
    pub fn unpacked(&self, from: &[usize]) -> Self {
        let mut out = Self::default();
        let mut placed = [false; MAX_LOCAL_PLAYERS];
        let put = |out: &mut Self, k: usize, i: usize| {
            out.seated[i] = true;
            out.pads[i] = self.pads[k];
            out.lost[i] = self.lost[k];
            out.last[i] = self.last[k];
            if self.keyboard == Some(k) {
                out.keyboard = Some(i);
            }
        };
        for k in (0..MAX_LOCAL_PLAYERS).filter(|k| self.seated[*k]) {
            if let Some(&i) = from.get(k).filter(|i| **i < MAX_LOCAL_PLAYERS) {
                if !out.seated[i] {
                    put(&mut out, k, i);
                    placed[k] = true;
                }
            }
        }
        for k in (0..MAX_LOCAL_PLAYERS).filter(|k| self.seated[*k] && !placed[*k]) {
            if let Some(i) = out.lowest_free() {
                put(&mut out, k, i);
            }
        }
        out
    }

    /// The highest seated player, if any.
    pub fn last_seated(&self) -> Option<usize> {
        self.seated.iter().rposition(|s| *s)
    }

    /// Does this pad's input reach gameplay, and for whom? An unowned pad drives nobody.
    pub fn route(&self, pad: Id) -> Option<usize> {
        self.player_of(pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_seats_players_in_the_given_order_and_take_seat_from_fills_a_free_seat() {
        let mut o = PadOwners::<u8>::default();
        o.join_keyboard();
        o.join_pad(10);
        o.join_pad(11);
        let p = o.picked(&[2, 0]);
        assert_eq!((p.pad_of(0), p.pad_of(1)), (Some(11), None));
        assert_eq!(p.keyboard_player(), Some(1));
        assert!(p.is_seated(0) && p.is_seated(1) && !p.is_seated(2));
        let mut back = PadOwners::<u8>::default();
        back.join_pad(11);
        back.take_seat_from(&o, 1);
        assert_eq!(back.pad_of(1), Some(10));
        back.take_seat_from(&o, 0);
        assert_eq!(
            back.pad_of(0),
            Some(11),
            "a seat already held is left as it is"
        );
    }

    #[test]
    fn each_pad_drives_exactly_one_player() {
        let mut o = PadOwners::<u8>::default();
        assert_eq!(o.join_pad(10), Some(0));
        assert_eq!(o.join_pad(11), Some(1));
        assert_eq!(
            o.join_pad(10),
            Some(0),
            "a pad that joins twice is still one player"
        );
        assert_eq!(
            (o.route(10), o.route(11), o.route(12)),
            (Some(0), Some(1), None)
        );
        assert_eq!(o.count(), 2);
    }

    #[test]
    fn a_fifth_person_is_refused_and_a_leaver_frees_the_lowest_seat() {
        let mut o = PadOwners::<u8>::default();
        for pad in 0..4 {
            assert_eq!(o.join_pad(pad), Some(usize::from(pad)));
        }
        assert_eq!(o.join_pad(9), None);
        assert_eq!(o.join_keyboard(), None);
        o.leave(1);
        assert_eq!(o.join_pad(9), Some(1));
    }

    #[test]
    fn the_keyboard_belongs_to_one_player_and_a_pad_never_moves_to_it() {
        let mut o = PadOwners::<u8>::default();
        assert_eq!(o.join_pad(10), Some(0));
        assert_eq!(o.join_keyboard(), Some(1));
        assert_eq!(o.join_keyboard(), Some(1), "one keyboard player");
        assert_eq!(o.keyboard_player(), Some(1));
        // Player 0's pad is not the keyboard player's, and cannot be taken by them.
        assert!(!o.bind_pad(1, 10));
        assert_eq!(o.route(10), Some(0));
        // The keyboard player may own their own pad, as a single player does.
        assert!(o.bind_pad(1, 11));
        assert_eq!((o.route(11), o.pad_of(1)), (Some(1), Some(11)));
    }

    #[test]
    fn an_unplugged_pad_keeps_its_seat_and_a_second_pad_cannot_take_it_over() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        assert_eq!(o.disconnected(10), Some(0));
        assert!(o.is_seated(0) && o.pad_of(0).is_none());
        assert_eq!(o.route(10), None);
        // Player 1's pad cannot be rebound to player 0; a free pad can.
        assert!(!o.bind_pad(0, 11));
        assert!(o.bind_pad(0, 12));
        assert_eq!(o.route(12), Some(0));
        assert_eq!(o.disconnected(99), None);
    }

    #[test]
    fn the_keyboard_is_never_bound_unless_someone_joins_on_it() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        assert_eq!(
            o.keyboard_player(),
            None,
            "two pad players, no keyboard player"
        );
        assert_eq!(o.count(), 2);
        // A keyboard that only operates the menus is simply not a player.
        assert!(!o.is_seated(2));
    }

    #[test]
    fn a_lost_pad_marks_only_its_player_and_the_pad_coming_back_clears_it() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        assert_eq!(o.disconnected(11), Some(1));
        assert!(o.is_lost(1) && !o.is_lost(0), "only player 1 waits");
        // Player 0's own pad cannot be taken over, and a pad that is not waiting for anyone does nothing.
        assert_eq!(o.reclaim(10), None);
        // The same pad (or a new id for it) returns to the waiting seat.
        assert_eq!(o.reclaim(42), Some(1));
        assert!(!o.is_lost(1));
        assert_eq!((o.route(42), o.route(10)), (Some(1), Some(0)));
        assert_eq!(o.reclaim(43), None, "nobody waits now");
    }

    #[test]
    fn two_waiting_seats_are_filled_in_order_and_a_keyboard_player_never_waits() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        assert_eq!(o.join_keyboard(), Some(2));
        assert!(o.bind_pad(2, 12));
        o.disconnected(10);
        o.disconnected(11);
        assert_eq!(o.disconnected(12), Some(2));
        assert!(
            !o.is_lost(2),
            "a keyboard player keeps playing on the keyboard"
        );
        assert_eq!(o.reclaim(20), Some(0));
        assert_eq!(o.reclaim(21), Some(1));
        assert_eq!(o.reclaim(22), None);
    }

    #[test]
    fn packing_closes_the_holes_in_seat_order_and_unpacking_restores_the_seats() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        assert_eq!(o.join_keyboard(), Some(1));
        o.join_pad(12);
        o.join_pad(13);
        o.disconnected(13);
        o.leave(1);
        assert_eq!((o.count(), o.route(12)), (3, Some(2)), "nobody moved");
        let (p, from) = o.packed();
        assert_eq!(from, vec![0, 2, 3]);
        assert_eq!((p.route(10), p.route(12)), (Some(0), Some(1)));
        assert!(p.is_lost(2) && !p.is_seated(3), "the lost seat packs too");
        assert_eq!(p.unpacked(&from), o, "back to the same seats");
        let mut q = p.clone();
        assert_eq!(q.reclaim(13), Some(2), "and still knows its pad");
        assert_eq!(q.unpacked(&from).route(13), Some(3));
        // The keyboard packs with its seat.
        let mut k = PadOwners::<u8>::default();
        k.join_pad(1);
        k.join_pad(2);
        k.join_keyboard();
        k.leave(0);
        let (p, from) = k.packed();
        assert_eq!((p.route(2), p.keyboard_player()), (Some(0), Some(1)));
        assert_eq!(p.unpacked(&from), k);
    }

    /// Two pads of one model drop together: a returning one may carry either's old id, so both
    /// seats are its candidates and the id seats it nowhere by itself; a pad of another model
    /// waiting meanwhile does not count. The person's pick seats it, and only on a waiting seat.
    #[test]
    fn identical_lost_pads_are_ambiguous_and_the_pick_seats_the_pad() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        o.join_pad(20);
        o.disconnected(10);
        o.disconnected(11);
        o.disconnected(20);
        // Pads 10 and 11 are one model (`< 20`), pad 20 another.
        let model = |a: u8, b: u8| (a < 20) == (b < 20);
        assert_eq!(o.alike_waiting(|old| model(old, 10)), vec![0, 1]);
        assert_eq!(
            o.alike_waiting(|old| model(old, 20)),
            vec![2],
            "a unique model"
        );
        // Seat 1's player picks seat 1 for the pad that came back on seat 0's old id.
        assert!(o.reclaim_seat(10, 1));
        assert_eq!(
            (o.route(10), o.is_lost(0), o.is_lost(1)),
            (Some(1), true, false)
        );
        assert!(!o.reclaim_seat(10, 0), "a seated pad is not picked again");
        assert!(!o.reclaim_seat(12, 1), "seat 1 waits no more");
        assert_eq!(
            o.alike_waiting(|old| model(old, 11)),
            vec![0],
            "one left: no question"
        );
    }

    #[test]
    fn a_returning_pad_finds_its_own_seat_before_the_lowest_waiting_one() {
        let mut o = PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(11);
        o.disconnected(10);
        o.disconnected(11);
        // Player 1's pad returns first, under its old id: it is player 1's, not player 0's.
        assert_eq!(o.reclaim(11), Some(1));
        assert_eq!(o.reclaim(10), Some(0));
        // Under new ids the lowest waiting seat is filled first.
        o.disconnected(11);
        o.disconnected(10);
        assert_eq!(o.reclaim(30), Some(0));
    }
}
