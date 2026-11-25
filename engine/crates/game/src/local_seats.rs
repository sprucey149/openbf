//! Which gamepad drives which local player in splitscreen (issue #194,
//! `analysis/plan/SPLITSCREEN.md` section 4). The Players screen seats people in the order they
//! join ([`Party`]) and keeps their numbers while they stay seated; a match starts from those
//! seats with the holes closed ([`Seats::from_owners`]) and hands the same numbers back after
//! it. The debug flag
//! `--local-players` starts from empty seats that pads fill in the order they send input.
//!
//! A thin rule over [`input::owners::PadOwners`]: a fixed number of players, an optional
//! keyboard-and-mouse player who always holds seat 0, a pad that moves while a seat is free
//! takes the lowest free seat, a pad that is already seated never moves, and a pad that comes
//! back after an unplug returns to the seat that waits for it. Generic over the pad id so it
//! tests without hardware.

use crate::profile::{Profile, Store, NAME_LIMIT};
use input::owners::PadOwners;
use ui::shell_feel::MenuSound;
use ui::shell_ps2::profiles::{Page, Picker, Target};

/// The seats of one debug splitscreen match.
#[derive(Debug, Clone)]
pub struct Seats<Id> {
    /// By local player: contiguous, 0..count.
    owners: PadOwners<Id>,
    players: usize,
    /// Each local player's seat number on the Players screen (identity without a party).
    seat_numbers: Vec<usize>,
}

impl<Id: Copy + PartialEq> Seats<Id> {
    /// `players` seats (clamped to 2..=4). With `keyboard`, seat 0 is the keyboard-and-mouse
    /// player and pads fill seats 1 up; without, the keyboard drives no player.
    pub fn new(players: usize, keyboard: bool) -> Self {
        let mut owners = PadOwners::default();
        if keyboard {
            owners.join_keyboard();
        }
        Seats {
            owners,
            players: players.clamp(2, input::owners::MAX_LOCAL_PLAYERS),
            seat_numbers: (0..input::owners::MAX_LOCAL_PLAYERS).collect(),
        }
    }

    /// The seats the Players screen filled (`Party`): the same people in the same order, with
    /// any free seat between them closed up, so local player 0 is the lowest seat number, 1 the
    /// next, and so on, and the battle's views run in the screen's order. `players` is the
    /// walk's local count (clamped to 2..=4).
    pub fn from_owners(owners: PadOwners<Id>, players: usize) -> Self {
        let (owners, seat_numbers) = owners.packed();
        Seats {
            owners,
            players: players.clamp(2, input::owners::MAX_LOCAL_PLAYERS),
            seat_numbers,
        }
    }

    /// [`Seats::from_owners`] with each local player's seat given (`order[k]` is local `k`'s
    /// seat number): a LAN round's locals follow the session's slots, not the screen's order
    /// (issue #194 B20).
    pub fn from_seats(owners: &PadOwners<Id>, order: &[usize]) -> Self {
        Seats {
            owners: owners.picked(order),
            players: order.len().clamp(2, input::owners::MAX_LOCAL_PLAYERS),
            seat_numbers: order.to_vec(),
        }
    }

    /// Who holds what now, by seat number, to hand back to the Players screen after the match:
    /// every player returns to the seat number they came with (a pad that came back under a
    /// new id keeps its seat for the next round).
    pub fn party_owners(&self) -> PadOwners<Id> {
        self.owners.unpacked(&self.seat_numbers)
    }

    /// Local player `i`'s seat number on the Players screen (0 is P1).
    pub fn seat_number(&self, i: usize) -> usize {
        self.seat_numbers.get(i).copied().unwrap_or(i)
    }

    /// The number of players in the match.
    pub fn players(&self) -> usize {
        self.players
    }

    /// A pad sent deliberate input. Returns the player it drives: the one it already drives, the
    /// waiting seat it reclaims, or a newly seated player; `None` when every seat is taken.
    pub fn offer(&mut self, pad: Id) -> Option<usize> {
        self.offer_with(pad, |_| false)
    }

    /// [`Seats::offer`] in battle: a pad that is not a waiting seat's own lost pad reclaims only a
    /// waiting seat whose lost pad is not back (`back(id)`: that pad is connected again). A
    /// player whose own controller is plugged back in keeps their cell for it, and no other pad
    /// takes their profile meanwhile. Two pads that both come back under new ids still go to the
    /// lowest waiting seats in turn: nothing tells them apart.
    pub fn offer_with(&mut self, pad: Id, back: impl Fn(Id) -> bool) -> Option<usize> {
        if let Some(i) = self.owners.player_of(pad) {
            return Some(i);
        }
        let owners = &self.owners;
        // A seat's old id that another player holds now (an identical pad came back on it and
        // was picked for them) is not that seat's pad being back.
        let open: Vec<bool> = (0..input::owners::MAX_LOCAL_PLAYERS)
            .map(|i| {
                owners
                    .lost_pad(i)
                    .is_none_or(|old| owners.player_of(old).is_some() || !back(old))
            })
            .collect();
        if let Some(i) = self.owners.reclaim_where(pad, |i| open[i]) {
            return Some(i);
        }
        if self.owners.count() >= self.players {
            return None;
        }
        self.owners.join_pad(pad)
    }

    /// [`Seats::offer_with`] that will not guess between identical pads (issue #194): when two or
    /// more waiting players lost a pad `alike` this one (the same model,
    /// [`PadOwners::alike_waiting`]), its id may be another of them's old id, so it is seated
    /// nowhere and the waiting players are offered as a [`SeatChoice`] for its holder to pick
    /// from. Otherwise as [`Seats::offer_with`]: a pad of a model only one waiting player lost
    /// still goes back to them by itself.
    pub fn offer_alike(
        &mut self,
        pad: Id,
        back: impl Fn(Id) -> bool,
        alike: impl Fn(Id) -> bool,
    ) -> Option<Offered<Id>> {
        if let Some(i) = self.owners.player_of(pad) {
            return Some(Offered::Seat(i));
        }
        let seats = self.owners.alike_waiting(alike);
        if seats.len() >= 2 {
            return Some(Offered::Choose(SeatChoice { pad, seats, at: 0 }));
        }
        self.offer_with(pad, back).map(Offered::Seat)
    }

    /// The pick of an open [`SeatChoice`]: its pad takes the waiting seat it shows. `None` when
    /// that seat no longer waits or the pad is already someone's.
    pub fn take_choice(&mut self, choice: &SeatChoice<Id>) -> Option<usize> {
        let i = choice.seat()?;
        self.owners.reclaim_seat(choice.pad, i).then_some(i)
    }

    /// The seats an open choice still offers: those of its seats that still wait (a seat another
    /// pad has taken meanwhile drops out).
    pub fn still_waiting(&self, choice: &mut SeatChoice<Id>) {
        let keep: Vec<usize> = choice
            .seats
            .iter()
            .copied()
            .filter(|&i| self.owners.is_lost(i))
            .collect();
        if keep != choice.seats {
            let shown = choice.seat();
            choice.seats = keep;
            choice.at = shown
                .and_then(|i| choice.seats.iter().position(|&j| j == i))
                .unwrap_or(0);
        }
    }

    /// A seated pad is gone: its player keeps the seat and drives nothing until a pad is offered.
    pub fn gone(&mut self, pad: Id) -> Option<usize> {
        self.owners.disconnected(pad)
    }

    /// The player a pad drives.
    pub fn player_of(&self, pad: Id) -> Option<usize> {
        self.owners.route(pad)
    }

    /// The pad player `i` holds.
    pub fn pad_of(&self, i: usize) -> Option<Id> {
        self.owners.pad_of(i)
    }

    /// The keyboard-and-mouse player.
    pub fn keyboard_player(&self) -> Option<usize> {
        self.owners.keyboard_player()
    }

    /// Player `i` has something to play with: a pad or the keyboard and mouse.
    pub fn has_controller(&self, i: usize) -> bool {
        self.keyboard_player() == Some(i) || self.pad_of(i).is_some()
    }

    /// Player `i` still waits for a controller (never joined, or lost their pad): their pad is
    /// neutral and their view is the "waiting for controller" state.
    pub fn is_waiting(&self, i: usize) -> bool {
        i < self.players && !self.has_controller(i)
    }

    /// Did player `i` lose a pad they had (as opposed to never having had one)? The keyboard
    /// player is never lost.
    pub fn is_lost(&self, i: usize) -> bool {
        self.owners.is_lost(i)
    }
}

/// What a pad offered in battle gets ([`Seats::offer_alike`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offered<Id> {
    /// The player it drives now.
    Seat(usize),
    /// It could be any of several waiting players: its holder picks.
    Choose(SeatChoice<Id>),
}

/// A returning pad that could be any of several waiting players (identical models, issue #194):
/// the waiting cells say whose seat it would take, the pad's d-pad moves that between them and
/// its A takes it ([`Seats::take_choice`]). Nothing is seated until then.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatChoice<Id> {
    /// The pad that pressed.
    pub pad: Id,
    /// The waiting local players it could be, lowest first.
    pub seats: Vec<usize>,
    /// Which of them it would take now.
    pub at: usize,
}

impl<Id> SeatChoice<Id> {
    /// The local player the pad would take now.
    pub fn seat(&self) -> Option<usize> {
        self.seats.get(self.at).copied()
    }

    /// The d-pad moved the pick one waiting player on (`step` +1) or back (-1), round the ends.
    pub fn step(&mut self, step: i32) {
        let n = self.seats.len() as i32;
        if n > 0 {
            self.at = (self.at as i32 + step).rem_euclid(n) as usize;
        }
    }
}

/// Issue #194 B20 (B17's game side): a LAN round's pad losses and returns go to the session
/// only, never to the walk: each local whose seat has lost its pad is away
/// ([`crate::netplay::Peer::set_local_away`]: the host takes the slot out with a `RemoveHuman`
/// and a bot plays on), and one whose pad is back returns (an `AddHuman` that seats it on its
/// old side with its round career, `netplay::TEAM_KEPT`). The bundles then change the walk on
/// every machine alike; nothing here suspends, resumes or installs a profile. `slots[k]` is walk
/// local `k`'s slot. Asked every frame (only a change is sent), so a new link (a new peer, none
/// away) is told again of every pad still missing. Returns the players whose pad came back.
pub fn lan_presence<Id: Copy + PartialEq, L: crate::netplay::Link>(
    seats: &Seats<Id>,
    slots: &[u8],
    peer: &mut crate::netplay::Peer,
    link: &mut L,
) -> Vec<usize> {
    let mut back = Vec::new();
    for (k, slot) in slots.iter().enumerate() {
        let Some(j) = (0..peer.local_count()).find(|&j| peer.local_slot_of(j) == Some(*slot))
        else {
            continue;
        };
        let away = seats.is_lost(k);
        let was = peer.local_away(j);
        if was != away {
            peer.set_local_away(link, j, away);
            println!(
                "split: player {k} {} (LAN slot {slot})",
                if away { "is away" } else { "is back" }
            );
            if !away {
                back.push(k);
            }
        }
    }
    back
}

/// Issue #194 B20: the pads a LAN round's frame submits, one per local player of the session in
/// its own order (`Peer::local_slot_of`), from this frame's finished pads by slot
/// (the window's `SplitState::finish_pads`: each player's own viewpoint stamped, the first player's own pad
/// already neutral under the Game Menu and carrying its Respawn, `NetplaySession::frame_pad`).
/// A further player's Game Menu Respawn is taken from the walk
/// ([`crate::walk::WalkSim::take_respawn_of`], which a LAN round never applies itself) and goes
/// in their pad, so every machine sees it through the bundles. Tracked inputs never go out.
pub fn lan_pads(
    peer: &crate::netplay::Peer,
    walk: &mut crate::walk::WalkSim,
    by_slot: &[core::sim::PadState],
) -> Vec<core::sim::PadState> {
    (0..peer.local_count())
        .map(|j| {
            let Some(slot) = peer.local_slot_of(j) else {
                return core::sim::PadState::default();
            };
            let mut pad = by_slot.get(usize::from(slot)).copied().unwrap_or_default();
            let k = (1..walk.local_count()).find(|&k| walk.local_slot_of(k) == slot);
            if k.is_some_and(|k| walk.take_respawn_of(k)) {
                pad.buttons |= core::sim::buttons::RESPAWN;
            }
            net::bundle::strip_tracked_inputs(pad)
        })
        .collect()
}

/// One pad's fresh presses on the Players screen this frame (the arms with their hold repeat).
/// The keyboard's are the same shape: Enter is `join`, Esc `back`, Space `start`, the arrows the
/// arms and Page Up / Page Down L1 / R1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartyPress {
    /// A / Cross (South): join; confirm inside a picker; Continue on a Ready card.
    pub join: bool,
    /// Start: join; Accept on the name keyboard; else Continue.
    pub start: bool,
    /// B / Circle (East) only: back (`SPLITSCREEN.md` 18.1 item 1).
    pub back: bool,
    /// X / Square (West): a Ready card's picker again; Backspace on the name keyboard.
    pub west: bool,
    /// Y / Triangle (North): the name keyboard's symbols page. Never Back here.
    pub north: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub l1: bool,
    pub r1: bool,
    /// Select (Share, Create, View, Back) held for [`REMOVE_HOLD`] on a pad, or Tab on the
    /// keyboard: empty the lowest guest seat whose pad is gone ([`Party::step`]). Sent once per
    /// hold.
    pub remove: bool,
}

/// How long Select (or Tab) is held to empty a lost seat: long enough never to happen by
/// accident (OURS; replace: a measured hold from playtests).
pub const REMOVE_HOLD: f32 = 1.0;

/// One button's hold toward [`PartyPress::remove`]: it fires once when the button has been down
/// [`REMOVE_HOLD`] seconds, and again only after it is let go and held anew.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveHold {
    held: f32,
    sent: bool,
}

impl RemoveHold {
    /// This frame: is the button `down`, `dt` seconds since the last? `true` on the frame the
    /// hold completes.
    pub fn step(&mut self, down: bool, dt: f32) -> bool {
        if !down {
            *self = RemoveHold::default();
            return false;
        }
        self.held += dt.max(0.0);
        if self.sent || self.held < REMOVE_HOLD {
            return false;
        }
        self.sent = true;
        true
    }

    /// How far the hold is, 0..1 (a prompt may fill as it goes).
    pub fn progress(&self) -> f32 {
        (self.held / REMOVE_HOLD).clamp(0.0, 1.0)
    }
}

/// One seat's profile on the Players screen (`SPLITSCREEN.md` 18.1 item 1), by seat number.
#[derive(Debug, Default)]
pub enum SeatProfile {
    /// Nobody sits here.
    #[default]
    Open,
    /// Seated and choosing in the seat's own picker.
    Picking(Picker),
    /// Seat 0 plays on the current profile (`App.profile`); no copy is kept here.
    Host,
    /// Seats 1-3: the one live copy of the seat's profile.
    Ready(Profile),
    /// The Ready profile is in the battle (`App.split_profiles`), moved out with
    /// [`Party::lend`]; it holds its id meanwhile (greying, refusals and Continue treat it as
    /// Ready). `None` for a profile with no catalog slot.
    Lent(Option<u64>),
}

impl SeatProfile {
    /// Ready, the host's, or lent to a battle.
    pub fn is_ready(&self) -> bool {
        matches!(
            self,
            SeatProfile::Host | SeatProfile::Ready(_) | SeatProfile::Lent(_)
        )
    }

    pub fn picker(&self) -> Option<&Picker> {
        match self {
            SeatProfile::Picking(p) => Some(p),
            _ => None,
        }
    }
}

/// Everything the Players screen hears in one frame.
#[derive(Clone, Debug)]
pub struct PartyFrame<Id> {
    /// The usable pads connected now.
    pub connected: Vec<Id>,
    /// Each pad's fresh presses, in any order.
    pub pads: Vec<(Id, PartyPress)>,
    /// The keyboard's: Enter joins the keyboard and mouse as a pad's A does (the lowest free
    /// seat), then acts for the keyboard seat only.
    pub keyboard: PartyPress,
    /// Letters typed and Backspaces on the keyboard this frame: only the keyboard seat's name
    /// keyboard hears them.
    pub typed: String,
    pub backspaces: usize,
    /// A click on a live Continue.
    pub click_continue: bool,
    /// The pointer over a seat's picker: (seat, target, clicked). Only the keyboard seat's
    /// picker hears it.
    pub pointer: Option<(usize, Target, bool)>,
    /// The mouse wheel: it scrolls the keyboard seat's profile list only.
    pub wheel: f32,
    /// Each known pad's model (connected, and every waiting seat's lost pad): two pads of one
    /// model cannot be told apart once they drop, so a returning one that two waiting seats'
    /// lost pads share is not trusted to be either's (issue #194, [`PadOwners::alike_waiting`]).
    pub models: Vec<(Id, [u8; 16])>,
}

impl<Id> Default for PartyFrame<Id> {
    fn default() -> Self {
        PartyFrame {
            connected: Vec::new(),
            pads: Vec::new(),
            keyboard: PartyPress::default(),
            typed: String::new(),
            backspaces: 0,
            click_continue: false,
            pointer: None,
            wheel: 0.0,
            models: Vec::new(),
        }
    }
}

/// What the seats' pickers read from the window.
#[derive(Clone, Copy)]
pub struct PartyEnv<'a> {
    /// The profile catalog; `None` (a dev profile file) seats P2-P4 on unsaved "Player N"
    /// profiles at once.
    pub store: Option<&'a Store>,
    /// The current profile's id: seat 0's (the host's).
    pub host_id: Option<u64>,
    /// The host's difficulty: a profile made in a seat's picker takes it (no Difficulty page).
    pub difficulty: core::skill::Difficulty,
}

/// What the Players screen does after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartyStep {
    Stay,
    /// Back with nobody seated, or P1 (or the free keyboard's Esc) backing the whole screen
    /// out, every seat emptied: the main menu.
    Leave,
    /// Two or more are seated, every one Ready, and someone asked to go on: the mode list.
    Continue,
}

/// A frame's result: the step, the menu cues to play, and seat 0's new profile.
#[derive(Debug)]
pub struct PartyOutcome {
    pub step: PartyStep,
    pub cues: Vec<MenuSound>,
    /// Seat 0 picked or made a profile: it becomes the current profile
    /// (`App::apply_host_profile`), and the Players screen stays.
    pub host: Option<Profile>,
}

/// The message a seat's picker shows on a profile another seat holds.
pub const TAKEN: &str = "Another player has this profile";

/// The Players screen's rule over [`PadOwners`] and the seats' profiles (issue #194,
/// `analysis/plan/SPLITSCREEN.md` sections 3, 15 and 18.1 item 1). Kept for the whole session,
/// so the same people come back to the same seats after every round. A seated player's number
/// never changes while they stay seated: nobody else leaving moves them.
///
/// - An unseated pad's A or Start reclaims the seat it lost (or the lowest waiting seat, see
///   [`PadOwners::reclaim`]), else takes the lowest free seat. Joining never moves anyone. P1
///   (seat 0) is the host and plays on the current profile; P2-P4 open their own picker. A
///   different pad that reclaims a lost seat reopens that seat's picker, so nobody plays on
///   another person's profile; the same pad gets the seat back as it was.
/// - A seated pad that is unplugged keeps its seat and its pick and waits for a pad.
/// - Each seat hears only its own pad (the keyboard seat: the keys, typing and the pointer).
/// - B: on the name keyboard, the picker's list; on a Ready card, the picker; on the list,
///   that seat (the presser's) empties. Seat 0 cannot empty while another seat is seated (the
///   host rule): its B there is refused. B on the host card alone empties seat 0. An unseated
///   pad's B, or an unbound Esc, leaves only when nobody is seated.
/// - Select held ([`REMOVE_HOLD`]) on any seated pad, or Tab on the keyboard: the lowest guest
///   seat whose pad is gone empties ([`Party::removable`]), so the others can go on without a
///   controller that is not coming back. Only that seat changes.
/// - Start from anyone seated, A on a Ready card, Enter on the keyboard seat's Ready card or a
///   click on Continue, with two or more seated and every one Ready: continue.
#[derive(Debug)]
pub struct Party<Id> {
    owners: PadOwners<Id>,
    profiles: [SeatProfile; input::owners::MAX_LOCAL_PLAYERS],
    /// Waiting seats whose returning pad can't be told apart (issue #194): two or more of them
    /// lost identical pads and one of those came back ([`Party::join_pad_alike`]). Whichever pad
    /// takes such a seat later asks for its profile too, even once only one of them waits, and
    /// meanwhile the seat's pick is no one's to grey out. Cleared when the seat is taken or
    /// emptied.
    unsure: [bool; input::owners::MAX_LOCAL_PLAYERS],
}

impl<Id> Default for Party<Id> {
    fn default() -> Self {
        Party {
            owners: PadOwners::default(),
            profiles: Default::default(),
            unsure: [false; input::owners::MAX_LOCAL_PLAYERS],
        }
    }
}

/// What a seat's picker asks of its seat after a frame.
#[derive(Debug, PartialEq)]
enum PickAct {
    Stay,
    /// B on the list: empty the seat.
    Back,
    /// Start outside the name keyboard: go on (refused while anyone picks).
    Continue,
    Chosen(u64),
    Create(String),
}

impl<Id: Copy + PartialEq> Party<Id> {
    pub fn owners(&self) -> &PadOwners<Id> {
        &self.owners
    }

    /// The seats as a match left them (a reclaimed pad's new id). The seats' profiles follow
    /// on the next [`Party::step`].
    pub fn set_owners(&mut self, owners: PadOwners<Id>) {
        self.owners = owners;
        self.unsure = [false; input::owners::MAX_LOCAL_PLAYERS];
    }

    /// Does waiting seat `i` ask whoever takes it for their profile, as its returning pad
    /// can't be told from another seat's ([`Party::join_pad_alike`])?
    pub fn is_unsure(&self, i: usize) -> bool {
        self.unsure.get(i).copied().unwrap_or(false)
    }

    pub fn count(&self) -> usize {
        self.owners.count()
    }

    /// Seat `i`'s profile.
    pub fn profile(&self, i: usize) -> &SeatProfile {
        &self.profiles[i.min(input::owners::MAX_LOCAL_PLAYERS - 1)]
    }

    /// Two or more are seated and every seated player is Ready (the original's rule), and no
    /// two seats are Ready on one profile (a waiting seat whose pick was freed while its pad
    /// could not be told apart, [`Party::is_unsure`], and another seat took that profile).
    pub fn can_start(&self) -> bool {
        self.owners.count() >= 2
            && self.twice().is_empty()
            && (0..input::owners::MAX_LOCAL_PLAYERS)
                .filter(|&i| self.owners.is_seated(i))
                .all(|i| self.profiles[i].is_ready())
    }

    /// The seated seats Ready on a profile a lower seated seat is Ready on too ([`can_start`]'s
    /// refusal): the later of each pair, so its card can say so and its player changes profile.
    ///
    /// [`can_start`]: Party::can_start
    pub fn twice(&self) -> Vec<usize> {
        let id = |i: usize| match &self.profiles[i] {
            SeatProfile::Ready(p) if self.owners.is_seated(i) => p.id(),
            _ => None,
        };
        (0..input::owners::MAX_LOCAL_PLAYERS)
            .filter(|&i| id(i).is_some_and(|me| (0..i).any(|j| id(j) == Some(me))))
            .collect()
    }

    /// Is seated seat `i` Ready on a profile another seated seat is Ready on too? Every such
    /// card says so (one of them must change profile before Continue).
    pub fn shares_profile(&self, i: usize) -> bool {
        let id = |j: usize| match &self.profiles[j] {
            SeatProfile::Ready(p) if self.owners.is_seated(j) => p.id(),
            _ => None,
        };
        id(i).is_some_and(|me| {
            (0..input::owners::MAX_LOCAL_PLAYERS).any(|j| j != i && id(j) == Some(me))
        })
    }

    /// Can a deliberate hold ([`PartyPress::remove`]) empty lost seat `i`? Any guest seat whose
    /// pad is gone; never seat 0, the host, while anyone else sits (the host rule: seat 0 is
    /// never a hole while others are seated). Another pad's A or Start still takes seat 0.
    pub fn removable(&self, i: usize) -> bool {
        i != 0 && self.owners.is_lost(i)
    }

    /// A hold from seat `by` (`None`: the free keyboard) empties the lowest removable seat that
    /// is not its own. Only that seat changes; nobody is renumbered.
    fn remove_lost(&mut self, by: Option<usize>, out: &mut PartyOutcome) {
        let Some(i) =
            (0..input::owners::MAX_LOCAL_PLAYERS).find(|&i| Some(i) != by && self.removable(i))
        else {
            return;
        };
        self.unseat(i);
        out.cues.push(MenuSound::Exit);
    }

    /// The ids seats other than `seat` hold: their Ready and lent profiles, and the current
    /// profile's `host_id` while seat 0 is seated (as the host or choosing). A waiting seat
    /// that will ask for its profile again ([`Party::is_unsure`]) holds no guest pick: its
    /// player may be the one choosing now.
    pub fn held_ids(&self, seat: usize, host_id: Option<u64>) -> Vec<u64> {
        (0..input::owners::MAX_LOCAL_PLAYERS)
            .filter(|&j| j != seat)
            .filter(|&j| j == 0 || !self.unsure[j])
            .filter_map(|j| match &self.profiles[j] {
                SeatProfile::Ready(p) => p.id(),
                SeatProfile::Lent(id) => *id,
                SeatProfile::Host | SeatProfile::Picking(_) if j == 0 => host_id,
                _ => None,
            })
            .collect()
    }

    /// The ids P2-P4 hold: the title picker greys them and refuses Select and Delete on them.
    pub fn guest_ids(&self) -> Vec<u64> {
        self.held_ids(0, None)
    }

    /// The keyboard seat is on its name keyboard: typed letters go there.
    pub fn keyboard_typing(&self) -> bool {
        self.owners.keyboard_player().is_some_and(|i| {
            matches!(
                &self.profiles[i],
                SeatProfile::Picking(p) if matches!(p.page, Page::Name { .. })
            )
        })
    }

    /// A launch moves seat `seat`'s Ready profile out (never a copy): the seat holds its id
    /// as lent until [`Party::give_back`]. Seat 0 lends nothing (the host's is `App.profile`).
    pub fn lend(&mut self, seat: usize) -> Option<Profile> {
        if seat == 0 || seat >= input::owners::MAX_LOCAL_PLAYERS {
            return None;
        }
        match std::mem::take(&mut self.profiles[seat]) {
            SeatProfile::Ready(p) => {
                self.profiles[seat] = SeatProfile::Lent(p.id());
                Some(p)
            }
            other => {
                self.profiles[seat] = other;
                None
            }
        }
    }

    /// The battle is over: a lent seat's profile comes back as it was left.
    pub fn give_back(&mut self, seat: usize, profile: Profile) {
        if let Some(sp) = self.profiles.get_mut(seat) {
            if matches!(sp, SeatProfile::Lent(_)) {
                *sp = SeatProfile::Ready(profile);
            }
        }
    }

    /// A seat that has just been taken: the host's on seat 0, a picker elsewhere.
    fn fresh(&self, seat: usize, env: &PartyEnv<'_>) -> SeatProfile {
        if seat == 0 {
            return SeatProfile::Host;
        }
        match env.store {
            Some(_) => SeatProfile::Picking(self.picker(seat, env, None)),
            None => {
                let mut guest = Profile::default().into_seat();
                guest.name = format!("Player {}", seat + 1);
                SeatProfile::Ready(guest)
            }
        }
    }

    /// Seat `seat`'s picker over the catalog, on `focus` when listed.
    fn picker(&self, seat: usize, env: &PartyEnv<'_>, focus: Option<u64>) -> Picker {
        let mut pick = match env.store.map(|s| s.list()) {
            Some(Ok(list)) => Picker::new(list.names, None),
            Some(Err(error)) => {
                eprintln!("profiles: {error:#}");
                let mut p = Picker::new(Vec::new(), None);
                p.message = Some("Could not read profiles".into());
                p
            }
            None => Picker::new(Vec::new(), None),
        };
        pick.seat = true;
        pick.focus_id(focus);
        pick.taken = self.held_ids(seat, env.host_id);
        pick
    }

    /// The seats' profiles follow the seats: a seat filled from outside (a match's hand-back,
    /// the debug flag) gets its fresh profile, an emptied one is open.
    fn sync(&mut self, env: &PartyEnv<'_>) {
        for i in 0..input::owners::MAX_LOCAL_PLAYERS {
            let seated = self.owners.is_seated(i);
            match (&self.profiles[i], seated) {
                (SeatProfile::Open, true) => self.profiles[i] = self.fresh(i, env),
                (SeatProfile::Open, false) => {}
                (_, false) => {
                    self.profiles[i] = SeatProfile::Open;
                    self.unsure[i] = false;
                }
                _ => {}
            }
        }
    }

    /// A pad pressed its join button: the player it drives, the seat it reclaims, or a new
    /// seat; `None` when four are seated and none waits.
    pub fn join_pad(&mut self, pad: Id, env: &PartyEnv<'_>) -> Option<usize> {
        self.join_pad_alike(pad, env, |old| old == pad)
    }

    /// [`Party::join_pad`] knowing which lost pads are `alike` the joining one (its model): when
    /// two or more waiting seats lost a pad of its model, its id is no proof of whose it is (the
    /// platform hands a returning pad the lowest old id of its model), so the seat it takes asks
    /// its player for their profile as any other pad's would, and nobody plays on somebody
    /// else's profile.
    pub fn join_pad_alike(
        &mut self,
        pad: Id,
        env: &PartyEnv<'_>,
        alike: impl Fn(Id) -> bool,
    ) -> Option<usize> {
        if let Some(i) = self.owners.player_of(pad) {
            return Some(i);
        }
        // Two or more waiting seats lost a pad of this model: none of them can trust its
        // returning pad any more, this one or a later one (the first back changes the count).
        let group = self.owners.alike_waiting(alike);
        if group.len() >= 2 {
            for &i in &group {
                self.unsure[i] = true;
            }
        }
        let own = (0..input::owners::MAX_LOCAL_PLAYERS)
            .find(|&i| self.owners.is_lost(i) && self.owners.lost_pad(i) == Some(pad))
            .filter(|&i| !self.unsure[i]);
        if let Some(i) = self.owners.reclaim(pad) {
            let asks = own != Some(i);
            // Taken: this seat's question, if any, is answered by the picker it opens now.
            self.unsure[i] = false;
            if asks {
                // Somebody else's pad took a waiting seat: they choose their own profile. With
                // no catalog there is nothing to choose from, so the seat starts afresh (the
                // host on seat 0, an unsaved guest elsewhere) rather than on an empty picker.
                self.profiles[i] = match env.store {
                    Some(_) => {
                        let host = env.host_id.filter(|_| i == 0);
                        SeatProfile::Picking(self.picker(i, env, host))
                    }
                    None => self.fresh(i, env),
                };
            }
            return Some(i);
        }
        let i = self.owners.join_pad(pad)?;
        self.profiles[i] = self.fresh(i, env);
        Some(i)
    }

    /// The keyboard and mouse join: their seat, the one they already hold or the lowest free
    /// one; `None` when four are seated.
    pub fn join_keyboard(&mut self, env: &PartyEnv<'_>) -> Option<usize> {
        if let Some(i) = self.owners.keyboard_player() {
            return Some(i);
        }
        let i = self.owners.join_keyboard()?;
        self.profiles[i] = self.fresh(i, env);
        Some(i)
    }

    /// Seat `i` empties: the presser's own, never another's.
    fn unseat(&mut self, i: usize) {
        self.owners.leave(i);
        self.profiles[i] = SeatProfile::Open;
        self.unsure[i] = false;
    }

    /// B on seat `i`'s Ready card opens its picker again rather than leaving the seat: a guest
    /// seat with a catalog, and seat 0 while others sit (its help says "Change profile").
    pub fn back_rechooses(&self, i: usize, has_catalog: bool) -> bool {
        has_catalog && (i != 0 || self.others_seated(0))
    }

    /// Anyone but seat `i` is seated.
    fn others_seated(&self, i: usize) -> bool {
        (0..input::owners::MAX_LOCAL_PLAYERS).any(|j| j != i && self.owners.is_seated(j))
    }

    /// One frame of the screen.
    pub fn step(&mut self, f: &PartyFrame<Id>, env: &PartyEnv<'_>) -> PartyOutcome {
        let mut env = *env;
        let mut out = PartyOutcome {
            step: PartyStep::Stay,
            cues: Vec::new(),
            host: None,
        };
        // An unplugged pad: its player keeps the seat and the pick, and waits.
        for i in 0..input::owners::MAX_LOCAL_PLAYERS {
            if let Some(pad) = self.owners.pad_of(i) {
                if !f.connected.contains(&pad) {
                    self.owners.disconnected(pad);
                }
            }
        }
        self.sync(&env);
        let mut go_on = f.click_continue;
        let mut leave = false;
        for (pad, press) in &f.pads {
            match self.owners.player_of(*pad) {
                Some(i) if press.remove => self.remove_lost(Some(i), &mut out),
                Some(i) => {
                    let input = SeatFrame::default();
                    self.seat_input(i, press, &input, &mut env, &mut out, &mut go_on)
                }
                None if press.join || press.start => {
                    let model = f.models.iter().find(|(id, _)| id == pad).map(|(_, m)| *m);
                    let alike = |old: Id| {
                        model.is_some_and(|m| f.models.iter().any(|(id, n)| *id == old && *n == m))
                    };
                    if self.join_pad_alike(*pad, &env, alike).is_some() {
                        out.cues.push(MenuSound::Accept);
                    }
                }
                None => leave |= press.back && self.count() == 0,
            }
        }
        let k = &f.keyboard;
        match self.owners.keyboard_player() {
            Some(i) if k.remove => self.remove_lost(Some(i), &mut out),
            None if k.remove => self.remove_lost(None, &mut out),
            Some(i) => {
                // While the keyboard seat types a name Space is a letter, so Enter accepts it
                // (the title picker's Enter does the same), never the focused key.
                let k = if self.keyboard_typing() && k.join {
                    PartyPress {
                        join: false,
                        start: true,
                        ..*k
                    }
                } else {
                    *k
                };
                let input = SeatFrame {
                    typed: &f.typed,
                    backspaces: f.backspaces,
                    pointer: f
                        .pointer
                        .filter(|(seat, _, _)| *seat == i)
                        .map(|(_, t, click)| (t, click)),
                    wheel: f.wheel,
                };
                self.seat_input(i, &k, &input, &mut env, &mut out, &mut go_on);
            }
            None if k.join => {
                if self.join_keyboard(&env).is_some() {
                    out.cues.push(MenuSound::Accept);
                }
            }
            // With nobody on the keyboard, its Space continues when everyone is Ready (it drives
            // the menus, and Enter would seat it), and Esc backs the whole screen out to the
            // main menu, every seat emptied (integration finding: the screen never looks
            // stuck). An unseated pad's B leaves only an empty screen.
            None if k.start => go_on = true,
            None if k.back => out.step = PartyStep::Leave,
            None => {}
        }
        // Every picker greys what the others hold now (a pick this frame included).
        for i in 0..input::owners::MAX_LOCAL_PLAYERS {
            let held = self.held_ids(i, env.host_id);
            if let SeatProfile::Picking(p) = &mut self.profiles[i] {
                p.taken = held;
            }
        }
        if out.step == PartyStep::Leave {
            // Backing out empties every seat; the next visit starts from nobody.
            for i in 0..input::owners::MAX_LOCAL_PLAYERS {
                self.unseat(i);
            }
            out.cues.push(MenuSound::Exit);
        } else if leave && self.count() == 0 {
            out.step = PartyStep::Leave;
        } else if go_on {
            if self.can_start() {
                out.step = PartyStep::Continue;
            } else if self.count() >= 2 {
                out.cues.push(MenuSound::Error);
            }
        }
        out
    }

    /// Seat `i`'s own input this frame.
    fn seat_input(
        &mut self,
        i: usize,
        press: &PartyPress,
        input: &SeatFrame<'_>,
        env: &mut PartyEnv<'_>,
        out: &mut PartyOutcome,
        go_on: &mut bool,
    ) {
        match std::mem::take(&mut self.profiles[i]) {
            SeatProfile::Open => {}
            SeatProfile::Picking(mut pick) => {
                pick.taken = self.held_ids(i, env.host_id);
                let act = step_picker(&mut pick, i, press, input, &mut out.cues);
                self.profiles[i] = SeatProfile::Picking(pick);
                match act {
                    PickAct::Stay => {}
                    PickAct::Continue => *go_on = true,
                    // P1 backs out of their list while others sit: the whole screen goes back
                    // to the main menu, every seat emptied (integration finding: one exit a
                    // pad can reach; the others' B only leaves their own seat).
                    PickAct::Back if i == 0 && self.others_seated(0) => {
                        out.step = PartyStep::Leave;
                    }
                    PickAct::Back => {
                        self.unseat(i);
                        out.cues.push(MenuSound::Exit);
                    }
                    PickAct::Chosen(id) => self.choose(i, Ok(id), env, out),
                    PickAct::Create(name) => self.choose(i, Err(name), env, out),
                }
            }
            ready => {
                let host = matches!(ready, SeatProfile::Host);
                let id = match &ready {
                    SeatProfile::Ready(p) => p.id(),
                    SeatProfile::Lent(id) => *id,
                    _ => env.host_id,
                };
                self.profiles[i] = ready;
                // With no catalog there is no picker to go back to: B unseats.
                let pickers = env.store.is_some();
                let held = i == 0 && self.others_seated(0);
                if press.back && !held && (host || !pickers) {
                    // Alone, seat 0's B leaves the seat as before profiles.
                    self.unseat(i);
                    out.cues.push(MenuSound::Exit);
                } else if press.back && !pickers {
                    // No picker to go back to: P1's B backs the whole screen out, as on their
                    // list.
                    out.step = PartyStep::Leave;
                } else if (press.back || press.west) && pickers {
                    self.profiles[i] = SeatProfile::Picking(self.picker(i, env, id));
                    out.cues.push(if press.back {
                        MenuSound::Exit
                    } else {
                        MenuSound::Accept
                    });
                } else if press.join || press.start {
                    *go_on = true;
                }
            }
        }
    }

    /// Seat `i` chose a listed profile (`Ok(id)`) or made one (`Err(name)`): seat 0's becomes
    /// the current profile (moving the catalog's last-used, as the title picker does), the
    /// others are bound quietly.
    fn choose(
        &mut self,
        i: usize,
        pick: Result<u64, String>,
        env: &mut PartyEnv<'_>,
        out: &mut PartyOutcome,
    ) {
        let Some(store) = env.store else { return };
        if let Ok(id) = pick {
            if self.held_ids(i, env.host_id).contains(&id) {
                if let SeatProfile::Picking(p) = &mut self.profiles[i] {
                    p.message = Some(TAKEN.into());
                }
                out.cues.push(MenuSound::Error);
                return;
            }
        }
        let result = match (&pick, i) {
            (Ok(id), 0) => store.select(*id),
            (Ok(id), _) => store.bind(*id),
            (Err(name), 0) => store.create(name, env.difficulty),
            (Err(name), _) => store.create_quiet(name, env.difficulty),
        };
        match result {
            Ok(profile) if i == 0 => {
                env.host_id = profile.id();
                self.profiles[0] = SeatProfile::Host;
                out.host = Some(profile);
                out.cues.push(MenuSound::Accept);
            }
            Ok(profile) => {
                self.profiles[i] = SeatProfile::Ready(profile);
                out.cues.push(MenuSound::Accept);
            }
            Err(error) => {
                eprintln!("profiles: {error:#}");
                if let SeatProfile::Picking(p) = &mut self.profiles[i] {
                    p.message = Some(error.to_string());
                }
                out.cues.push(MenuSound::Error);
            }
        }
    }
}

/// The title picker (Back on the main menu) refuses Select and Delete on a profile another
/// seat holds ([`Party::guest_ids`] in `pick.taken`), so it can neither delete a held profile
/// nor make the current profile the same as another seat's.
pub fn title_refuses(pick: &Picker, action: Target) -> bool {
    match action {
        Target::Profile(i) => pick.is_taken(i),
        Target::Delete => pick.selected_taken(),
        _ => false,
    }
}

/// The keyboard seat's extras this frame (typing, the pointer and the wheel); a pad's are empty.
#[derive(Clone, Copy, Default)]
struct SeatFrame<'a> {
    typed: &'a str,
    backspaces: usize,
    pointer: Option<(Target, bool)>,
    wheel: f32,
}

/// One frame of a seat's picker from its own presses: the title picker's rules without
/// Delete or the Difficulty page, B as Back everywhere, North the symbols page and West the
/// name keyboard's Backspace. `seat` (0 is P1): Create suggests "Player {seat + 1}" first.
fn step_picker(
    pick: &mut Picker,
    seat: usize,
    p: &PartyPress,
    input: &SeatFrame<'_>,
    cues: &mut Vec<MenuSound>,
) -> PickAct {
    let SeatFrame {
        typed,
        backspaces,
        pointer,
        wheel,
    } = *input;
    let editing = matches!(pick.page, Page::Name { .. });
    if editing {
        if backspaces != 0 || p.west {
            for _ in 0..backspaces.max(1) {
                pick.backspace();
            }
            cues.push(MenuSound::Backspace);
        }
        if !typed.is_empty() {
            pick.type_name(typed, NAME_LIMIT);
            cues.push(MenuSound::Key);
        }
        if p.north {
            pick.toggle_symbols();
            cues.push(MenuSound::Key);
            return PickAct::Stay;
        }
    } else if p.start {
        return PickAct::Continue;
    }
    if p.back {
        if editing {
            pick.page = Page::List;
            pick.set_focus(Target::Create);
            pick.message = None;
            cues.push(MenuSound::Exit);
            return PickAct::Stay;
        }
        return PickAct::Back;
    }
    let old = pick.focus;
    let mut x = isize::from(p.right) - isize::from(p.left);
    if editing {
        x += isize::from(p.r1) - isize::from(p.l1);
    }
    let y = isize::from(p.down) - isize::from(p.up);
    if x != 0 || y != 0 {
        pick.move_focus(x, y);
    }
    if !editing {
        if p.l1 {
            pick.page_list(-1);
        } else if p.r1 {
            pick.page_list(1);
        }
        if wheel != 0.0 {
            pick.wheel(wheel);
        }
    }
    let mut action = if editing && p.start {
        Some(Target::Accept)
    } else if p.join {
        Some(pick.focus)
    } else {
        None
    };
    if let Some((target, click)) = pointer {
        if !matches!(
            target,
            Target::Delete | Target::Previous | Target::Next | Target::Symbols
        ) {
            pick.point_focus(target);
        }
        if click {
            action = Some(target);
        }
    }
    if pick.focus != old {
        cues.push(MenuSound::Select);
    }
    let Some(action) = action else {
        return PickAct::Stay;
    };
    match action {
        Target::Profile(i) => {
            if pick.is_taken(i) {
                pick.message = Some(TAKEN.into());
                cues.push(MenuSound::Error);
                return PickAct::Stay;
            }
            match pick.names.get(i) {
                Some((id, _)) => PickAct::Chosen(*id),
                None => PickAct::Stay,
            }
        }
        Target::Create => {
            // The seat's own number first ("Player 3" beside the P3 tag), then the next free.
            let suggestion = (seat + 1..)
                .map(|i| format!("Player {i}"))
                .find(|name| !pick.names.iter().any(|(_, n)| n.eq_ignore_ascii_case(name)))
                .unwrap_or_default();
            pick.edit_name(suggestion);
            cues.push(MenuSound::Accept);
            PickAct::Stay
        }
        Target::Key(i) => {
            if let Some(c) = pick.keys().as_bytes().get(i) {
                let mut key = [0; 1];
                pick.type_name(char::from(*c).encode_utf8(&mut key), NAME_LIMIT);
            }
            cues.push(MenuSound::Key);
            PickAct::Stay
        }
        Target::Symbols => {
            pick.toggle_symbols();
            cues.push(MenuSound::Key);
            PickAct::Stay
        }
        Target::Backspace => {
            pick.backspace();
            cues.push(MenuSound::Backspace);
            PickAct::Stay
        }
        Target::Cancel => {
            pick.page = Page::List;
            pick.set_focus(Target::Create);
            cues.push(MenuSound::Exit);
            PickAct::Stay
        }
        Target::Accept => {
            let Page::Name { text, .. } = &pick.page else {
                return PickAct::Stay;
            };
            let name = text.trim().to_owned();
            let refusal = if name.is_empty() {
                Some("Enter at least one character")
            } else if pick
                .names
                .iter()
                .any(|(_, n)| n.trim().eq_ignore_ascii_case(&name))
            {
                Some("That profile name already exists")
            } else {
                None
            };
            if let Some(message) = refusal {
                pick.message = Some(message.into());
                cues.push(MenuSound::Error);
                return PickAct::Stay;
            }
            PickAct::Create(name)
        }
        Target::Previous | Target::Next => {
            pick.page_list(if action == Target::Previous { -1 } else { 1 });
            cues.push(MenuSound::Select);
            PickAct::Stay
        }
        Target::Delete | Target::Difficulty(_) | Target::Yes | Target::No => PickAct::Stay,
    }
}

/// What the window must do to the walk for a player whose controller went or came back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The controller is gone: `WalkSim::suspend_local_player(i)`.
    Suspend(usize),
    /// The controller is back: `WalkSim::resume_local_player(i, ..)`, then the spawn screens.
    Resume(usize),
}

/// Which players the walk has suspended, and the changes that bring it in line with the seats.
/// Pure, so the state machine from connect and disconnect events to suspend and resume calls
/// tests without a window.
#[derive(Clone, Debug, Default)]
pub struct Presence {
    away: Vec<bool>,
}

impl Presence {
    pub fn new(players: usize) -> Self {
        Presence {
            away: vec![false; players],
        }
    }

    /// Is player `i` suspended in the walk?
    pub fn is_away(&self, i: usize) -> bool {
        self.away.get(i).copied().unwrap_or(false)
    }

    /// The calls still to make so the walk matches the seats, in player order.
    pub fn pending<Id: Copy + PartialEq>(&self, seats: &Seats<Id>) -> Vec<Change> {
        (0..self.away.len())
            .filter_map(|i| match (seats.is_lost(i), self.away[i]) {
                (true, false) => Some(Change::Suspend(i)),
                (false, true) => Some(Change::Resume(i)),
                _ => None,
            })
            .collect()
    }

    /// The walk carried out `change`; a call that failed is simply not recorded and comes back
    /// from [`Presence::pending`] next frame.
    pub fn done(&mut self, change: Change) {
        match change {
            Change::Suspend(i) => self.away[i] = true,
            Change::Resume(i) => self.away[i] = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_become_players_in_the_order_they_send_input() {
        let mut s = Seats::<u8>::new(3, false);
        assert_eq!(s.offer(20), Some(0));
        assert_eq!(s.offer(10), Some(1));
        assert_eq!(s.offer(20), Some(0), "a seated pad stays put");
        assert_eq!(s.offer(30), Some(2));
        assert_eq!(s.offer(40), None, "no fourth seat in a 3-player match");
        assert_eq!((s.player_of(10), s.player_of(40)), (Some(1), None));
    }

    #[test]
    fn the_keyboard_is_unbound_unless_asked_for_and_then_holds_seat_zero() {
        let mut s = Seats::<u8>::new(2, false);
        assert_eq!(s.keyboard_player(), None);
        assert!(s.is_waiting(0) && s.is_waiting(1));
        assert_eq!(s.offer(5), Some(0));
        let mut k = Seats::<u8>::new(2, true);
        assert_eq!(k.keyboard_player(), Some(0));
        assert!(!k.is_waiting(0), "the keyboard player never waits");
        assert!(k.is_waiting(1));
        assert_eq!(k.offer(5), Some(1), "the first pad is player 1");
        assert_eq!(k.offer(6), None, "a 2-player match is full");
        assert_eq!(
            k.pad_of(0),
            None,
            "the keyboard player owns no pad unless given one"
        );
    }

    #[test]
    fn a_lost_pad_leaves_its_player_waiting_and_a_pad_returns_to_it() {
        let mut s = Seats::<u8>::new(2, false);
        s.offer(1);
        s.offer(2);
        assert_eq!(s.gone(1), Some(0));
        assert!(s.is_waiting(0) && !s.is_waiting(1));
        assert_eq!(s.offer(2), Some(1), "player 1's pad is unaffected");
        assert_eq!(s.offer(9), Some(0), "a new pad id takes the waiting seat");
        assert_eq!(s.pad_of(0), Some(9));
        assert!(!s.is_waiting(0));
    }

    /// Drive the seats and the presence the way the window does each frame.
    fn settle(s: &Seats<u8>, p: &mut Presence) -> Vec<Change> {
        let c = p.pending(s);
        for x in &c {
            p.done(*x);
        }
        c
    }

    #[test]
    fn a_lost_pad_suspends_only_its_player_and_its_return_resumes_them() {
        let mut s = Seats::<u8>::new(3, false);
        let mut p = Presence::new(3);
        for pad in [1, 2, 3] {
            s.offer(pad);
        }
        assert!(settle(&s, &mut p).is_empty(), "nobody is away at the start");
        s.gone(2);
        assert_eq!(settle(&s, &mut p), vec![Change::Suspend(1)]);
        assert!(p.is_away(1) && !p.is_away(0) && !p.is_away(2));
        assert!(settle(&s, &mut p).is_empty(), "a suspend is made once");
        assert_eq!(s.offer(7), Some(1), "the pad comes back under a new id");
        assert_eq!(settle(&s, &mut p), vec![Change::Resume(1)]);
        assert!(!p.is_away(1));
        assert!(settle(&s, &mut p).is_empty());
    }

    #[test]
    fn two_lost_pads_return_in_either_order() {
        let mut s = Seats::<u8>::new(2, false);
        let mut p = Presence::new(2);
        s.offer(1);
        s.offer(2);
        s.gone(1);
        s.gone(2);
        assert_eq!(
            settle(&s, &mut p),
            vec![Change::Suspend(0), Change::Suspend(1)]
        );
        assert_eq!(s.offer(2), Some(1), "the same id returns to its own seat");
        assert_eq!(settle(&s, &mut p), vec![Change::Resume(1)]);
        assert_eq!(s.offer(9), Some(0));
        assert_eq!(settle(&s, &mut p), vec![Change::Resume(0)]);
    }

    /// Integration finding (issue #194): in battle, a pad that is not a waiting seat's own takes
    /// no seat whose own pad is plugged back in (that player keeps their cell for it); with that
    /// pad still gone, a new id takes the seat as before.
    /// Issue #194, the Players screen: two seats' identical pads drop, and one comes back on
    /// P2's old id. It is not taken for P2's own pad: the seat it fills asks its player for their
    /// profile, as for any other pad, so nobody sits on somebody else's pick. A pad of a model
    /// only one seat lost is still that seat's, pick and all.
    #[test]
    fn an_identical_pad_on_the_players_screen_asks_for_its_profile() {
        let c = catalog("alike", &["Ann", "Bo", "Cy", "Di"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [5, 1, 2, 7] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        for (pad, k) in [(1, 1), (2, 2), (7, 3)] {
            choose(&mut p, &e, pad, c.ids[k]);
            assert!(p.profile(k).is_ready(), "seat {k}");
        }
        let models = vec![(1, [1; 16]), (2, [1; 16]), (5, [5; 16]), (7, [7; 16])];
        let frame = |connected: Vec<u8>, presses: &[(u8, PartyPress)]| PartyFrame {
            connected,
            pads: presses.to_vec(),
            models: models.clone(),
            ..Default::default()
        };
        p.step(&frame(vec![5], &[]), &e);
        assert!((1..4).all(|k| p.owners().is_lost(k)));
        // The unique model goes straight back to its seat and pick.
        p.step(&frame(vec![5, 7], &[(7, A)]), &e);
        assert_eq!(p.owners().route(7), Some(3));
        assert!(p.profile(3).is_ready());
        // Id 1 is back, but either identical pad could carry it: P2's seat asks.
        p.step(&frame(vec![5, 7, 1], &[(1, A)]), &e);
        assert_eq!(p.owners().route(1), Some(1));
        assert!(
            p.profile(1).picker().is_some(),
            "the seat asks for a profile"
        );
        assert!(p.profile(2).is_ready() && p.owners().is_lost(2));
        // Finding players-screen-second-identical-pad-trusted: P3's seat can't trust its pad
        // either now, and its pick is no one's to grey: the pad on P2's seat may be P3's own
        // player, who picks their own profile there.
        assert!(p.is_unsure(2));
        let picker = p.profile(1).picker().expect("P2's seat asks");
        assert!(
            !picker.taken.contains(&c.ids[2]),
            "Cy, P3's pick, is free to the player choosing on P2's seat"
        );
        choose(&mut p, &e, 1, c.ids[2]);
        assert!(p.profile(1).is_ready());
        assert!(!p.can_start(), "two seats Ready on one profile never start");
        // Finding players-continue-live-but-refused-duplicate-profile: the party names the
        // later seat of the pair, and both cards can say they share a profile.
        assert_eq!(p.twice(), vec![2]);
        assert!(p.shares_profile(1) && p.shares_profile(2) && !p.shares_profile(3));
        // The second identical pad comes back on P3's old id with only P3 waiting: it is not
        // trusted to be P3's own either. P3's seat asks too, and Cy is taken there now.
        p.step(&frame(vec![5, 7, 1, 2], &[(2, A)]), &e);
        assert_eq!(p.owners().route(2), Some(2));
        let picker = p.profile(2).picker().expect("P3's seat asks too");
        assert!(picker.taken.contains(&c.ids[2]), "Cy is P2's seat's now");
        assert!(!p.is_unsure(2), "answered by its picker");
        choose(&mut p, &e, 2, c.ids[1]);
        assert!(p.profile(2).is_ready());
        assert!(p.can_start(), "Bo and Cy each on their own seat");
    }

    /// Issue #194: two players' identical pads drop together and one comes back on the other's
    /// old id (the platform gives the lowest). It is seated on neither: both are offered, its
    /// d-pad moves the pick and its A takes the seat its holder picked, so nobody's seat, cell or
    /// profile swaps. The second then comes back with one seat left and takes it by itself; a
    /// pad of a model only one waiting player lost goes straight back too.
    #[test]
    fn identical_pads_coming_back_ask_whose_they_are() {
        let mut s = Seats::<u8>::new(3, false);
        for pad in [1, 2, 7] {
            s.offer(pad);
        }
        s.gone(1);
        s.gone(2);
        // Pads 1 and 2 are one model, 7 another.
        let alike = |pad: u8| move |old: u8| (old == 7) == (pad == 7);
        // Player 1's pad comes back first and is handed player 0's old id, 1.
        let Some(Offered::Choose(mut c)) = s.offer_alike(1, |_| true, alike(1)) else {
            panic!("an ambiguous pad is asked, not seated");
        };
        assert_eq!((c.seats.clone(), c.seat()), (vec![0, 1], Some(0)));
        assert!(s.is_waiting(0) && s.is_waiting(1), "nobody seated yet");
        c.step(1);
        assert_eq!(c.seat(), Some(1));
        assert_eq!(s.take_choice(&c), Some(1));
        assert_eq!((s.player_of(1), s.is_waiting(0)), (Some(1), true));
        // The other pad comes back on id 2: one such seat waits, so it goes there by itself.
        assert_eq!(s.offer_alike(2, |_| true, alike(2)), Some(Offered::Seat(0)));
        // A unique model is never asked about.
        s.gone(7);
        s.gone(1);
        assert_eq!(s.offer_alike(7, |_| true, alike(7)), Some(Offered::Seat(2)));
        // A choice whose seat was taken meanwhile keeps the ones still waiting.
        s.gone(2);
        let Some(Offered::Choose(mut c)) = s.offer_alike(9, |_| false, alike(9)) else {
            panic!("two of the model wait again");
        };
        c.step(1);
        assert_eq!(
            s.offer_with(8, |_| false),
            Some(0),
            "another pad takes seat 0"
        );
        s.still_waiting(&mut c);
        assert_eq!((c.seats.clone(), c.seat()), (vec![1], Some(1)));
        c.step(-1);
        assert_eq!(s.take_choice(&c), Some(1));
    }

    #[test]
    fn another_pad_takes_no_seat_whose_own_pad_is_back() {
        let mut s = Seats::<u8>::new(3, false);
        for pad in [1, 2, 3] {
            s.offer(pad);
        }
        s.gone(1);
        s.gone(2);
        // Pad 1 is plugged back in but has not pressed yet; a spare pad 9 presses.
        assert_eq!(
            s.offer_with(9, |id| id == 1),
            Some(1),
            "not P1's seat: P2's"
        );
        assert!(s.is_waiting(0));
        assert_eq!(
            s.offer_with(1, |id| id == 1),
            Some(0),
            "P1's own pad comes home"
        );
        s.gone(3);
        assert_eq!(
            s.offer_with(8, |_| false),
            Some(2),
            "a new id, the old pad still gone"
        );
    }

    #[test]
    fn a_player_who_never_had_a_pad_is_waiting_not_away() {
        let s = Seats::<u8>::new(2, false);
        let p = Presence::new(2);
        assert!(s.is_waiting(0) && !s.is_lost(0));
        assert!(p.pending(&s).is_empty());
    }

    #[test]
    fn the_keyboard_player_never_waits_or_goes_away() {
        let mut s = Seats::<u8>::new(2, true);
        let mut p = Presence::new(2);
        s.offer(5);
        assert_eq!(s.gone(5), Some(1));
        assert_eq!(settle(&s, &mut p), vec![Change::Suspend(1)]);
        assert!(!p.is_away(0) && !s.is_lost(0));
    }

    #[test]
    fn a_failed_resume_is_retried() {
        let mut s = Seats::<u8>::new(2, false);
        let mut p = Presence::new(2);
        s.offer(1);
        s.offer(2);
        s.gone(1);
        settle(&s, &mut p);
        s.offer(1);
        assert_eq!(p.pending(&s), vec![Change::Resume(0)]);
        assert_eq!(p.pending(&s), vec![Change::Resume(0)], "not done yet");
    }

    const NONE: PartyPress = PartyPress {
        join: false,
        start: false,
        back: false,
        west: false,
        north: false,
        up: false,
        down: false,
        left: false,
        right: false,
        l1: false,
        r1: false,
        remove: false,
    };
    const A: PartyPress = PartyPress { join: true, ..NONE };
    const START: PartyPress = PartyPress {
        start: true,
        ..NONE
    };
    const B: PartyPress = PartyPress { back: true, ..NONE };
    const WEST: PartyPress = PartyPress { west: true, ..NONE };
    const NORTH: PartyPress = PartyPress {
        north: true,
        ..NONE
    };
    const DOWN: PartyPress = PartyPress { down: true, ..NONE };
    const UP: PartyPress = PartyPress { up: true, ..NONE };
    const RIGHT: PartyPress = PartyPress {
        right: true,
        ..NONE
    };

    fn pads(presses: &[(u8, PartyPress)]) -> PartyFrame<u8> {
        PartyFrame {
            connected: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
            pads: presses.to_vec(),
            ..Default::default()
        }
    }

    fn keys(k: PartyPress) -> PartyFrame<u8> {
        PartyFrame {
            keyboard: k,
            ..pads(&[])
        }
    }

    fn enter() -> PartyFrame<u8> {
        keys(A)
    }

    fn esc() -> PartyFrame<u8> {
        keys(B)
    }

    /// No catalog: P2-P4 sit on unsaved "Player N" profiles at once (a dev profile file).
    const BARE: PartyEnv<'static> = PartyEnv {
        store: None,
        host_id: None,
        difficulty: core::skill::Difficulty::Normal,
    };

    /// A profile catalog in a temp dir, removed on drop.
    struct Cat {
        dir: std::path::PathBuf,
        store: Store,
        ids: Vec<u64>,
    }

    impl Drop for Cat {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn catalog(tag: &str, names: &[&str]) -> Cat {
        let dir = std::env::temp_dir().join(format!("openbf-seats-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::new(dir.clone());
        let ids = names
            .iter()
            .map(|n| {
                store
                    .create_quiet(n, core::skill::Difficulty::Normal)
                    .unwrap()
                    .id()
                    .unwrap()
            })
            .collect();
        Cat { dir, store, ids }
    }

    /// The host (seat 0, `App.profile`) plays on `ids[0]`.
    fn env(c: &Cat) -> PartyEnv<'_> {
        PartyEnv {
            store: Some(&c.store),
            host_id: Some(c.ids[0]),
            difficulty: core::skill::Difficulty::Hard,
        }
    }

    fn focus(p: &Party<u8>, seat: usize) -> Target {
        p.profile(seat).picker().expect("picking").focus
    }

    /// Pad `pad` moves its picker to `id` and presses A.
    fn choose(p: &mut Party<u8>, e: &PartyEnv<'_>, pad: u8, id: u64) -> PartyOutcome {
        let seat = p.owners().route(pad).expect("seated");
        let picker = p.profile(seat).picker().expect("picking");
        let want = picker.names.iter().position(|(k, _)| *k == id).unwrap();
        let mut at = match picker.focus {
            Target::Profile(i) => i,
            _ => picker.names.len(),
        };
        while at != want {
            let arm = if at < want { DOWN } else { UP };
            p.step(&pads(&[(pad, arm)]), e);
            at = match focus(p, seat) {
                Target::Profile(i) => i,
                _ => picker_len(p, seat),
            };
        }
        p.step(&pads(&[(pad, A)]), e)
    }

    fn picker_len(p: &Party<u8>, seat: usize) -> usize {
        p.profile(seat).picker().unwrap().names.len()
    }

    fn ready_id(p: &Party<u8>, seat: usize) -> Option<u64> {
        match p.profile(seat) {
            SeatProfile::Ready(profile) => profile.id(),
            _ => None,
        }
    }

    #[test]
    fn players_take_seats_in_the_order_they_join_and_never_move() {
        let mut p = Party::<u8>::default();
        assert_eq!(p.step(&pads(&[(3, A)]), &BARE).step, PartyStep::Stay);
        assert_eq!(
            p.step(&pads(&[(1, START)]), &BARE).step,
            PartyStep::Stay,
            "Start joins too"
        );
        p.step(&pads(&[(2, A)]), &BARE);
        let o = p.owners();
        assert_eq!(
            (o.route(3), o.route(1), o.route(2)),
            (Some(0), Some(1), Some(2))
        );
        assert!(matches!(p.profile(0), SeatProfile::Host));
        // Pressing again changes nothing; a seated pad's A is not a second seat.
        p.step(&pads(&[(2, A), (1, A)]), &BARE);
        assert_eq!(p.count(), 3);
        assert_eq!(p.owners().route(3), Some(0));
    }

    #[test]
    fn a_pad_joining_opens_its_own_picker() {
        let c = catalog("join", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        assert!(matches!(p.profile(0), SeatProfile::Host), "P1 is the host");
        p.step(&pads(&[(2, A)]), &e);
        let pick = p.profile(1).picker().expect("P2 chooses");
        assert!(pick.seat, "no Delete in a seat's picker");
        assert_eq!(pick.taken, vec![c.ids[0]], "the host's profile is greyed");
        assert!(matches!(p.profile(0), SeatProfile::Host));
        // Joining with A does not also pick: P2 is still on the list.
        assert_eq!(p.count(), 2);
        let out = choose(&mut p, &e, 2, c.ids[1]);
        assert_eq!(out.step, PartyStep::Stay);
        assert_eq!(ready_id(&p, 1), Some(c.ids[1]));
        match p.profile(1) {
            SeatProfile::Ready(profile) => {
                assert!(profile.is_seat(), "bound quietly");
                assert_eq!(profile.name, "Bob");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            c.store.list().unwrap().last,
            None,
            "a seat's pick never moves the catalog's last-used"
        );
        // A new profile made on a seat takes the host's difficulty and no Difficulty page.
        p.step(&pads(&[(3, A)]), &e);
        let len = picker_len(&p, 2);
        for _ in 0..len {
            p.step(&pads(&[(3, DOWN)]), &e);
        }
        assert_eq!(focus(&p, 2), Target::Create);
        p.step(&pads(&[(3, A)]), &e);
        assert!(
            matches!(
                &p.profile(2).picker().unwrap().page,
                Page::Name { text, .. } if text == "Player 3"
            ),
            "P3's suggestion is their own number"
        );
        p.step(&pads(&[(3, START)]), &e);
        match p.profile(2) {
            SeatProfile::Ready(profile) => {
                assert_eq!(profile.name, "Player 3");
                assert_eq!(profile.settings.difficulty, core::skill::Difficulty::Hard);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn b_on_the_list_unseats_only_the_presser() {
        let c = catalog("back", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [1, 2, 3, 4] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        p.step(&pads(&[(4, DOWN)]), &e);
        let p4 = focus(&p, 3);
        // P3 (pad 3) leaves; P2 and P4 stay where they are, P4's picker untouched.
        let out = p.step(&pads(&[(3, B)]), &e);
        assert_eq!(out.step, PartyStep::Stay);
        assert!(out.cues.contains(&MenuSound::Exit));
        let o = p.owners();
        assert_eq!(
            (o.route(1), o.route(2), o.route(3), o.route(4)),
            (Some(0), Some(1), None, Some(3))
        );
        assert!(matches!(p.profile(2), SeatProfile::Open));
        assert_eq!(focus(&p, 3), p4);
        assert!(p.profile(1).picker().is_some());
        // B on a name keyboard is that picker's list, not a leave.
        p.step(&pads(&[(2, UP)]), &e);
        assert_eq!(focus(&p, 1), Target::Create);
        p.step(&pads(&[(2, A)]), &e);
        assert!(matches!(
            p.profile(1).picker().unwrap().page,
            Page::Name { .. }
        ));
        p.step(&pads(&[(2, B)]), &e);
        assert!(matches!(p.profile(1).picker().unwrap().page, Page::List));
        assert_eq!(p.owners().route(2), Some(1));
        // B on a Ready card is the picker again.
        choose(&mut p, &e, 2, c.ids[1]);
        assert!(ready_id(&p, 1).is_some());
        p.step(&pads(&[(2, B)]), &e);
        assert_eq!(focus(&p, 1), Target::Profile(1), "on its own profile");
    }

    /// Integration finding: P1's B on their own list while others sit backs the whole screen
    /// out to the main menu, every seat emptied (two presses on P1's pad from the Ready card).
    #[test]
    fn seat_zeros_back_while_others_sit_leaves_the_screen() {
        let c = catalog("host", &["Ann", "Bob"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        // B on the host card with P2 seated opens P1's picker on the current profile.
        p.step(&pads(&[(1, B)]), &e);
        assert_eq!(focus(&p, 0), Target::Profile(0));
        // B on seat 0's list leaves the screen, P2 unseated with them.
        let out = p.step(&pads(&[(1, B)]), &e);
        assert_eq!(out.step, PartyStep::Leave);
        assert!(out.cues.contains(&MenuSound::Exit));
        assert_eq!(p.count(), 0);
        assert_eq!(p.owners().route(2), None);
    }

    #[test]
    fn seat_zeros_back_alone_empties_the_seat_then_leaves() {
        let c = catalog("host_alone", &["Ann", "Bob"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        // P2 leaves; now seat 0's B empties it, and the next B leaves the screen.
        p.step(&pads(&[(2, B)]), &e);
        assert_eq!(p.count(), 1);
        assert_eq!(p.step(&pads(&[(1, B)]), &e).step, PartyStep::Stay);
        assert_eq!(p.count(), 0);
        assert_eq!(p.step(&pads(&[(1, B)]), &e).step, PartyStep::Leave);
        // Alone, B on the host card empties the seat at once, as before profiles.
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(1, B)]), &e);
        assert_eq!(p.count(), 0);
    }

    #[test]
    fn a_taken_id_is_refused_and_the_hosts_while_seat_zero_picks() {
        let c = catalog("taken", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [1, 2, 3] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        choose(&mut p, &e, 2, c.ids[1]);
        assert_eq!(ready_id(&p, 1), Some(c.ids[1]));
        // P3 cannot take Bob, nor the host's Ann.
        for id in [c.ids[1], c.ids[0]] {
            let out = choose(&mut p, &e, 3, id);
            assert!(out.cues.contains(&MenuSound::Error), "{id}");
            let pick = p.profile(2).picker().expect("still choosing");
            assert_eq!(pick.message.as_deref(), Some(TAKEN));
            assert!(pick.taken.contains(&id));
        }
        // While seat 0 chooses again, the current profile's id still counts as held.
        p.step(&pads(&[(1, WEST)]), &e);
        assert!(p.profile(0).picker().is_some());
        let out = choose(&mut p, &e, 3, c.ids[0]);
        assert!(out.cues.contains(&MenuSound::Error));
        assert!(p.profile(2).picker().is_some());
        // Seat 0 sees Bob greyed but its own Ann free.
        let p1 = p.profile(0).picker().unwrap();
        assert_eq!(p1.taken, vec![c.ids[1]]);
        // A lent profile is held too.
        assert_eq!(p.lend(1).and_then(|x| x.id()), Some(c.ids[1]));
        assert!(matches!(p.profile(1), SeatProfile::Lent(Some(id)) if *id == c.ids[1]));
        assert!(p.held_ids(2, e.host_id).contains(&c.ids[1]));
        assert_eq!(p.guest_ids(), vec![c.ids[1]]);
        // Cy is free.
        choose(&mut p, &e, 3, c.ids[2]);
        assert_eq!(ready_id(&p, 2), Some(c.ids[2]));
    }

    #[test]
    fn continue_waits_for_every_seat() {
        let c = catalog("continue", &["Ann", "Bob"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&enter(), &e);
        assert_eq!(p.owners().keyboard_player(), Some(0), "the keyboard is P1");
        assert_eq!(
            p.step(&enter(), &e).step,
            PartyStep::Stay,
            "alone, Enter waits"
        );
        p.step(&pads(&[(1, A)]), &e);
        assert!(!p.can_start());
        // Start, Enter, Space, a click and A on a Ready card are all refused while P2 chooses.
        let space = keys(START);
        let click = PartyFrame {
            click_continue: true,
            ..pads(&[])
        };
        for f in [pads(&[(1, START)]), enter(), space.clone(), click.clone()] {
            let out = p.step(&f, &e);
            assert_eq!(out.step, PartyStep::Stay);
            assert!(out.cues.contains(&MenuSound::Error));
        }
        assert!(p.profile(1).picker().is_some(), "P2's Start did not pick");
        choose(&mut p, &e, 1, c.ids[1]);
        assert!(p.can_start());
        for f in [pads(&[(1, START)]), pads(&[(1, A)]), enter(), space, click] {
            assert_eq!(p.step(&f, &e).step, PartyStep::Continue);
        }
        // A joining pad's own press does not also continue, and its picker holds the rest.
        assert_eq!(p.step(&pads(&[(3, START)]), &e).step, PartyStep::Stay);
        assert_eq!(p.count(), 3);
        assert_eq!(p.step(&pads(&[(1, START)]), &e).step, PartyStep::Stay);
    }

    #[test]
    fn a_pads_input_moves_only_its_own_picker_and_north_is_not_back() {
        let c = catalog("own", &["Ann", "Bob", "Cy", "Di"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [1, 2, 3] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        let before = focus(&p, 2);
        p.step(&pads(&[(2, DOWN)]), &e);
        p.step(&pads(&[(2, DOWN)]), &e);
        assert_ne!(focus(&p, 1), before);
        assert_eq!(focus(&p, 2), before, "P3's picker did not move");
        // North on the list is nothing; on the name keyboard it is the symbols page.
        let out = p.step(&pads(&[(2, NORTH)]), &e);
        assert_eq!(out.step, PartyStep::Stay);
        assert_eq!(p.owners().route(2), Some(1), "North is not Back");
        assert!(p.profile(1).picker().is_some());
        for _ in 0..3 {
            p.step(&pads(&[(2, UP), (3, UP)]), &e);
        }
        assert_eq!(focus(&p, 1), Target::Create);
        p.step(&pads(&[(2, A)]), &e);
        p.step(&pads(&[(2, NORTH)]), &e);
        assert!(matches!(
            p.profile(1).picker().unwrap().page,
            Page::Name { symbols: true, .. }
        ));
        assert!(matches!(p.profile(2).picker().unwrap().page, Page::List));
    }

    /// With no catalog, another pad taking lost seat 0 seats the host again (no empty picker
    /// it could never leave), and a lost guest seat an unsaved guest.
    #[test]
    fn another_pad_on_a_lost_seat_without_a_catalog_starts_it_afresh() {
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &BARE);
        p.step(&pads(&[(2, A)]), &BARE);
        let only = |connected: Vec<u8>, presses: &[(u8, PartyPress)]| PartyFrame {
            connected,
            pads: presses.to_vec(),
            ..Default::default()
        };
        p.step(&only(vec![2, 3], &[]), &BARE);
        assert!(p.owners().is_lost(0));
        p.step(&only(vec![2, 3], &[(3, A)]), &BARE);
        assert_eq!(p.owners().route(3), Some(0));
        assert!(matches!(p.profile(0), SeatProfile::Host));
        let out = p.step(&only(vec![2, 3], &[(3, START)]), &BARE);
        assert_eq!(out.step, PartyStep::Continue);
        p.step(&only(vec![3, 4], &[]), &BARE);
        p.step(&only(vec![3, 4], &[(4, A)]), &BARE);
        assert_eq!(p.owners().route(4), Some(1));
        assert!(matches!(p.profile(1), SeatProfile::Ready(g) if g.name == "Player 2"));
    }

    #[test]
    fn only_the_keyboard_seat_types() {
        let c = catalog("type", &["Ann"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&enter(), &e);
        p.step(&pads(&[(2, A)]), &e);
        assert_eq!(p.owners().keyboard_player(), Some(1));
        // Both open their name keyboards (Create is below the one name).
        p.step(&keys(DOWN), &e);
        p.step(&pads(&[(2, DOWN)]), &e);
        p.step(
            &PartyFrame {
                keyboard: A,
                pads: vec![(2, A)],
                ..pads(&[])
            },
            &e,
        );
        assert!(p.keyboard_typing());
        p.step(
            &PartyFrame {
                typed: "Zed".into(),
                ..pads(&[])
            },
            &e,
        );
        let text = |seat: usize| match &p.profile(seat).picker().unwrap().page {
            Page::Name { text, .. } => text.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(text(1), "Zed", "the keyboard seat's own name");
        assert_eq!(
            text(2),
            "Player 3",
            "the pad seat's suggestion (its own number) is untouched"
        );
        // Typing with nobody on the keyboard types nowhere.
        let mut q = Party::<u8>::default();
        q.step(&pads(&[(1, A)]), &e);
        q.step(&pads(&[(2, A)]), &e);
        q.step(&pads(&[(2, DOWN)]), &e);
        q.step(&pads(&[(2, A)]), &e);
        assert!(!q.keyboard_typing());
        q.step(
            &PartyFrame {
                typed: "Zed".into(),
                ..pads(&[])
            },
            &e,
        );
        assert!(matches!(
            &q.profile(1).picker().unwrap().page,
            Page::Name { text, .. } if text == "Player 2"
        ));
        // The pointer reaches the keyboard seat's picker only.
        let before = focus(&q, 1);
        q.step(
            &PartyFrame {
                pointer: Some((1, Target::Cancel, true)),
                ..pads(&[])
            },
            &e,
        );
        assert_eq!(focus(&q, 1), before);
        // Enter accepts the typed name (Space is a letter there), whatever key has the focus.
        p.step(&keys(RIGHT), &e);
        p.step(&enter(), &e);
        assert!(!p.keyboard_typing());
        match p.profile(1) {
            SeatProfile::Ready(profile) => assert_eq!(profile.name, "Zed"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn seat_zeros_pick_changes_the_host_and_nobody_else() {
        let c = catalog("repick", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        choose(&mut p, &e, 2, c.ids[1]);
        p.step(&pads(&[(1, WEST)]), &e);
        let out = choose(&mut p, &e, 1, c.ids[2]);
        assert_eq!(out.step, PartyStep::Stay, "the Players screen stays");
        let host = out.host.expect("the current profile changes");
        assert_eq!(host.name, "Cy");
        assert!(!host.is_seat(), "the host's moves the catalog's last-used");
        assert_eq!(c.store.list().unwrap().last, Some(c.ids[2]));
        assert!(matches!(p.profile(0), SeatProfile::Host));
        assert_eq!(ready_id(&p, 1), Some(c.ids[1]), "P2 is as it was");
        assert_eq!(p.owners().route(2), Some(1));
    }

    /// Integration finding: a keyboard that only drives the menus continues with Space once
    /// every seated pad is Ready, without sitting down itself.
    #[test]
    fn an_unseated_keyboards_space_continues() {
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &BARE);
        assert_eq!(p.step(&keys(START), &BARE).step, PartyStep::Stay, "alone");
        p.step(&pads(&[(2, A)]), &BARE);
        assert!(p.can_start());
        assert_eq!(p.step(&keys(START), &BARE).step, PartyStep::Continue);
        assert_eq!(
            p.owners().keyboard_player(),
            None,
            "the keyboard did not sit"
        );
    }

    /// Integration finding: with nobody on the keyboard, Esc backs the whole screen out to the
    /// main menu, every seat emptied; an unseated pad's B is nothing while anyone sits.
    #[test]
    fn an_unbound_esc_with_pad_seats_seated_leaves_the_screen() {
        let c = catalog("esc", &["Ann", "Bob"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        p.step(&pads(&[(2, DOWN)]), &e);
        let before = focus(&p, 1);
        // An unseated pad's B is nothing while anyone sits.
        assert_eq!(p.step(&pads(&[(7, B)]), &e).step, PartyStep::Stay);
        assert_eq!(p.count(), 2);
        assert_eq!(focus(&p, 1), before);
        let out = p.step(&esc(), &e);
        assert_eq!(out.step, PartyStep::Leave);
        assert!(out.cues.contains(&MenuSound::Exit));
        assert_eq!(p.count(), 0);
        // On an empty screen Esc and any pad's B leave.
        let mut q = Party::<u8>::default();
        assert_eq!(q.step(&esc(), &e).step, PartyStep::Leave);
        assert_eq!(q.step(&pads(&[(4, B)]), &e).step, PartyStep::Leave);
        // The keyboard seat's Esc is its own B.
        q.step(&enter(), &e);
        q.step(&esc(), &e);
        assert_eq!(q.count(), 0);
    }

    /// Finding players-screen-lost-seat-cannot-be-freed: a guest seat whose pad is gone for good
    /// is emptied by Select held on another seated pad (or Tab on the keyboard), the lowest such
    /// seat only, and nobody is renumbered. Seat 0 (the host) is never emptied this way while
    /// others sit; an unseated pad's hold does nothing.
    #[test]
    fn a_held_select_empties_a_lost_guest_seat_only() {
        let c = catalog("remove", &["Ann", "Bob", "Cy", "Di"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [1, 2, 3, 4] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        for (pad, k) in [(2, 1), (3, 2), (4, 3)] {
            choose(&mut p, &e, pad, c.ids[k]);
        }
        let hold = PartyPress {
            remove: true,
            ..NONE
        };
        let frame = |connected: Vec<u8>, presses: &[(u8, PartyPress)]| PartyFrame {
            connected,
            pads: presses.to_vec(),
            ..Default::default()
        };
        // P3's and P4's pads die.
        p.step(&frame(vec![1, 2], &[]), &e);
        assert!(p.owners().is_lost(2) && p.owners().is_lost(3));
        assert!(p.can_start(), "a lost seat still counts, as before");
        assert!(p.removable(2) && !p.removable(0) && !p.removable(1));
        // An unseated pad's hold is nothing.
        p.step(&frame(vec![1, 2, 9], &[(9, hold)]), &e);
        assert_eq!(p.count(), 4);
        // P2 holds Select: P3's seat (the lowest lost one) empties, nobody else changes.
        let out = p.step(&frame(vec![1, 2], &[(2, hold)]), &e);
        assert!(out.cues.contains(&MenuSound::Exit));
        assert_eq!(out.step, PartyStep::Stay);
        assert!(!p.owners().is_seated(2));
        assert_eq!(p.owners().route(1), Some(0));
        assert_eq!(p.owners().route(2), Some(1));
        assert_eq!(ready_id(&p, 1), Some(c.ids[1]));
        assert!(p.owners().is_lost(3) && ready_id(&p, 3) == Some(c.ids[3]));
        // The free keyboard's Tab hold empties P4's.
        p.step(
            &PartyFrame {
                keyboard: hold,
                ..frame(vec![1, 2], &[])
            },
            &e,
        );
        assert!(!p.owners().is_seated(3));
        assert_eq!(p.count(), 2);
        assert!(p.can_start());
        // P1's pad dies: P2's hold never empties the host's seat.
        p.step(&frame(vec![2], &[]), &e);
        assert!(p.owners().is_lost(0));
        p.step(&frame(vec![2], &[(2, hold)]), &e);
        assert!(p.owners().is_seated(0) && p.count() == 2);
    }

    /// The hold fires once, after [`REMOVE_HOLD`], and needs letting go to fire again; a tap
    /// never fires.
    #[test]
    fn a_remove_hold_fires_once_per_full_hold() {
        let mut h = RemoveHold::default();
        assert!(!h.step(true, 0.5));
        assert!(!h.step(false, 0.5), "let go early: nothing");
        assert!(!h.step(true, 0.6));
        assert!(h.step(true, 0.5), "a full second held");
        assert!(!h.step(true, 2.0), "once per hold");
        h.step(false, 0.1);
        assert!(!h.step(true, 0.9));
        assert!(h.step(true, 0.2));
    }

    #[test]
    fn a_dropped_pad_changes_only_its_region_and_comes_back_to_its_pick() {
        let c = catalog("drop", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        for pad in [1, 2, 3] {
            p.step(&pads(&[(pad, A)]), &e);
        }
        choose(&mut p, &e, 2, c.ids[1]);
        p.step(&pads(&[(3, DOWN)]), &e);
        let p3 = focus(&p, 2);
        let gone = PartyFrame {
            connected: vec![1, 3],
            ..Default::default()
        };
        p.step(&gone, &e);
        assert!(p.owners().is_lost(1) && p.owners().is_seated(1));
        assert_eq!(
            ready_id(&p, 1),
            Some(c.ids[1]),
            "the pick waits with the seat"
        );
        assert_eq!(focus(&p, 2), p3, "P3 is untouched");
        assert!(matches!(p.profile(0), SeatProfile::Host));
        // The same pad comes back: the seat and its pick as they were.
        p.step(&pads(&[(2, A)]), &e);
        assert_eq!(p.owners().route(2), Some(1));
        assert_eq!(ready_id(&p, 1), Some(c.ids[1]));
        // Another pad takes the waiting seat: it chooses its own profile.
        p.step(&gone, &e);
        p.step(&pads(&[(9, A)]), &e);
        assert_eq!(p.owners().route(9), Some(1));
        assert!(
            p.profile(1).picker().is_some(),
            "nobody plays on Bob's profile"
        );
        assert_eq!(focus(&p, 2), p3);
    }

    #[test]
    fn the_title_picker_refuses_select_and_delete_on_a_held_id() {
        let c = catalog("title", &["Ann", "Bob", "Cy"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        choose(&mut p, &e, 2, c.ids[1]);
        let list = c.store.list().unwrap();
        let mut title = Picker::new(list.names, None);
        title.taken = p.guest_ids();
        assert_eq!(title.taken, vec![c.ids[1]], "P2's, not the host's own");
        let bob = title
            .names
            .iter()
            .position(|(k, _)| *k == c.ids[1])
            .unwrap();
        title.set_focus(Target::Profile(bob));
        assert!(title_refuses(&title, Target::Profile(bob)));
        assert!(title_refuses(&title, Target::Delete));
        title.set_focus(Target::Profile((bob + 1) % 3));
        assert!(!title_refuses(&title, Target::Profile((bob + 1) % 3)));
        assert!(!title_refuses(&title, Target::Delete));
    }

    #[test]
    fn lend_moves_the_one_profile_out_and_back() {
        let c = catalog("lend", &["Ann", "Bob"]);
        let e = env(&c);
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &e);
        p.step(&pads(&[(2, A)]), &e);
        choose(&mut p, &e, 2, c.ids[1]);
        assert_eq!(p.lend(0).map(|x| x.name), None, "the host lends nothing");
        let mut bob = p.lend(1).expect("P2's profile");
        assert!(p.lend(1).is_none(), "moved, not copied");
        assert!(p.can_start(), "a lent seat counts as Ready");
        bob.career.kills = 7;
        p.give_back(1, bob);
        match p.profile(1) {
            SeatProfile::Ready(profile) => assert_eq!(profile.career.kills, 7),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_keyboard_joins_like_a_pad_and_enter_again_moves_nobody() {
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &BARE);
        assert_eq!(p.step(&enter(), &BARE).step, PartyStep::Stay);
        assert_eq!(p.owners().keyboard_player(), Some(1), "the keyboard is P2");
        p.step(&pads(&[(2, A)]), &BARE);
        assert_eq!(p.owners().route(2), Some(2));
        assert_eq!(p.owners().keyboard_player(), Some(1));
        assert_eq!(p.count(), 3);
        // Esc is the keyboard seat's B: P2 leaves, P3 stays P3.
        p.step(&esc(), &BARE);
        assert_eq!((p.count(), p.owners().keyboard_player()), (2, None));
        assert_eq!(p.owners().route(2), Some(2), "P3 stays P3");
    }

    #[test]
    fn seats_from_the_party_keep_its_order() {
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(3, A)]), &BARE);
        p.step(&enter(), &BARE);
        p.step(&pads(&[(5, A)]), &BARE);
        let s = Seats::from_owners(p.owners().clone(), p.count());
        assert_eq!(s.players(), 3);
        assert_eq!(
            (s.pad_of(0), s.keyboard_player(), s.pad_of(2)),
            (Some(3), Some(1), Some(5))
        );
        assert!(!s.is_waiting(1));
        assert_eq!(&s.party_owners(), p.owners());
    }

    /// Seat `i` empties with the others in place: how a match can hand seats back with a hole.
    fn free_seat(p: &mut Party<u8>, i: usize) {
        let mut o = p.owners().clone();
        o.leave(i);
        p.set_owners(o);
    }

    /// P1 pad, P2 keyboard, P3 pad, P4 pad.
    fn four_with_the_keyboard_second() -> Party<u8> {
        let mut p = Party::<u8>::default();
        p.step(&pads(&[(1, A)]), &BARE);
        p.step(&enter(), &BARE);
        p.step(&pads(&[(3, A)]), &BARE);
        p.step(&pads(&[(4, A)]), &BARE);
        let o = p.owners();
        assert_eq!(
            (o.route(1), o.keyboard_player(), o.route(3), o.route(4)),
            (Some(0), Some(1), Some(2), Some(3))
        );
        p
    }

    #[test]
    fn a_free_middle_seat_renumbers_nobody_and_the_next_joiner_takes_it() {
        let mut p = four_with_the_keyboard_second();
        free_seat(&mut p, 1);
        p.step(&pads(&[]), &BARE);
        let o = p.owners();
        assert_eq!(o.keyboard_player(), None);
        assert!(!o.is_seated(1), "P2 is free");
        assert!(matches!(p.profile(1), SeatProfile::Open));
        assert_eq!(
            (o.route(1), o.route(3), o.route(4)),
            (Some(0), Some(2), Some(3))
        );
        // A new pad takes the lowest free seat, P2.
        p.step(&pads(&[(5, A)]), &BARE);
        assert_eq!(p.owners().route(5), Some(1));
        assert_eq!(
            (p.owners().route(3), p.owners().route(4)),
            (Some(2), Some(3))
        );
        // P4's own B empties P4 and moves nobody else.
        p.step(&pads(&[(4, B)]), &BARE);
        let o = p.owners();
        assert_eq!(
            (o.route(1), o.route(5), o.route(3), o.route(4)),
            (Some(0), Some(1), Some(2), None)
        );
        // The keyboard rejoining takes the lowest free seat, P4.
        p.step(&enter(), &BARE);
        assert_eq!(p.owners().keyboard_player(), Some(3));
    }

    #[test]
    fn a_battle_plays_seats_one_three_four_as_local_players_zero_one_two() {
        let mut p = four_with_the_keyboard_second();
        free_seat(&mut p, 1);
        let s = Seats::from_owners(p.owners().clone(), p.count());
        assert_eq!(s.players(), 3);
        assert_eq!(
            (s.pad_of(0), s.pad_of(1), s.pad_of(2)),
            (Some(1), Some(3), Some(4))
        );
        assert_eq!(
            (s.player_of(1), s.player_of(3), s.player_of(4)),
            (Some(0), Some(1), Some(2))
        );
        assert_eq!(s.keyboard_player(), None);
        assert!((0..3).all(|i| !s.is_waiting(i)));
        assert_eq!(
            (s.seat_number(0), s.seat_number(1), s.seat_number(2)),
            (0, 2, 3),
            "each view knows its Players screen number"
        );
    }

    #[test]
    fn back_from_the_battle_the_players_screen_shows_the_same_numbers() {
        let mut p = four_with_the_keyboard_second();
        free_seat(&mut p, 1);
        let before = p.owners().clone();
        let mut s = Seats::from_owners(before.clone(), p.count());
        assert_eq!(s.party_owners(), before);
        // P3's pad drops and comes back under a new id during the battle.
        assert_eq!(s.gone(3), Some(1));
        assert_eq!(s.offer(9), Some(1));
        p.set_owners(s.party_owners());
        let o = p.owners();
        assert_eq!(
            (o.route(1), o.route(9), o.route(4)),
            (Some(0), Some(2), Some(3))
        );
        assert!(!o.is_seated(1), "P2 is still the free seat");
        // A round played with the keyboard on P2 hands it back on P2.
        let k = four_with_the_keyboard_second();
        let s = Seats::from_owners(k.owners().clone(), k.count());
        assert_eq!(&s.party_owners(), k.owners());
    }

    #[test]
    fn the_player_count_is_clamped_to_two_to_four() {
        assert_eq!(Seats::<u8>::new(1, false).players(), 2);
        assert_eq!(Seats::<u8>::new(9, false).players(), 4);
    }
}
