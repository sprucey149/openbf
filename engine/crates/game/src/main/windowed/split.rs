//! Splitscreen in the window (issue #194, `analysis/plan/SPLITSCREEN.md` phase 2b, stage A).
//!
//! The main menu's Splitscreen row opens the Players screen (`front.rs`, `game::local_seats::Party`),
//! and an Instant Action launched from its mode list starts with those seats, in that order
//! ([`seat`] with the party). The developer shortcut `--local-players N` (2..=4) seats N people on
//! this machine from empty seats and draws one view each.
//! `--local-keyboard` makes player 0 the keyboard-and-mouse player; without it the keyboard and
//! mouse drive menus, the pause menu and Esc only. Gamepads become players in the order they
//! send input ([`game::local_seats::Seats`]). A player with no pad yet has a neutral pad and
//! their view is still drawn from their camera (they stand on the spawn screens).
//!
//! Each view draws its own BF2 HUD, spawn pages and round-end screens inside its cell, and the
//! native Game Menu covers the whole window, driven by the player who opened it
//! (`game::split_ui`, laid out over the whole cell by `ui::hud_draw::Frame::cell`). The egui half
//! of each view's HUD is drawn per cell too (issue #194 B8): its round timer (in the counts row,
//! left of the counts, `game::name_tags::cell_timer_at`), its banner and the name
//! tags it sees, each in its own cell. The fps counter (under the FPS Counter option),
//! lost-controller notices and pad warning are egui, over the whole window. The one-player path
//! is not touched: every function here is reached only when `App::split_on` holds.
//!
//! Shared once per frame (the code that was already in the frame handler, run for player 0):
//! the sim step, particles and fx advance, soldier animation, the world's poses (doors,
//! turrets, stations), holograms, voice, music, the black box, dynamic resolution and the frame
//! profile. Run per view ([`render_views`]): the camera, the draw lists (units, vehicles,
//! soldier skins, weapons, cloth), see-through, first-person arms, the point lights and bolts,
//! the particle submit, the puff and squad-marker draws, and the render itself.

use super::{stick_full, tracked_velocity, walk_draws, App, GameplayStick};
use core::sim::PadState;
use game::split_layout::Cell;
use render::{Camera, DrawCmd, MeshHandle, Renderer, SkinnedDrawCmd};
use std::sync::OnceLock;

/// The debug flags: how many people and whether the keyboard and mouse play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::windowed) struct Config {
    pub players: usize,
    pub keyboard: bool,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Record `--local-players` and `--local-keyboard` (once, before the window opens).
pub(crate) fn set_config(players: usize, keyboard: bool) {
    let players = players.clamp(2, input::owners::MAX_LOCAL_PLAYERS);
    let _ = CONFIG.set(Config { players, keyboard });
}

fn config() -> Option<Config> {
    CONFIG.get().copied()
}

/// What one player's pad needs between frames (the fields that are single on `App`).
pub(in crate::windowed) struct PlayerInput {
    gameplay_stick: GameplayStick<gilrs::GamepadId>,
    stick_range: input::StickRange,
    sprint_latch: input::SprintLatch,
    pub resume_gate: core::sim::HeldEat,
    view_toggle: game::viewpoint::ViewToggle,
    viewpoint: game::viewpoint::Viewpoint,
    pub rumble_effect: Option<gilrs::ff::Effect>,
    /// A player after the first: the control context their pad table is for, and that table
    /// (their seat profile's per-context layout, as `App::context_gamepad` is the first's).
    pad_context: Option<ui::menu::ControlContext>,
    pad_map: Option<input::Mapping>,
    /// Their profile's layouts changed (the Game Menu's controls editor, issue #194 B10): the
    /// pad table is rebuilt on the next sample whatever the context.
    pad_stale: bool,
}

impl PlayerInput {
    pub(in crate::windowed) fn new(viewpoint: game::viewpoint::Viewpoint) -> Self {
        PlayerInput {
            gameplay_stick: GameplayStick::default(),
            stick_range: input::StickRange::default(),
            sprint_latch: input::SprintLatch::default(),
            resume_gate: core::sim::HeldEat::default(),
            view_toggle: game::viewpoint::ViewToggle::default(),
            viewpoint,
            rumble_effect: None,
            pad_context: None,
            pad_map: None,
            pad_stale: false,
        }
    }

    /// Issue #194 (B10): this player's viewpoint, set from their profile by a Game Menu row.
    pub(in crate::windowed) fn set_viewpoint(&mut self, v: game::viewpoint::Viewpoint) {
        self.viewpoint = v;
    }

    /// The viewpoint stamped on this player's pad.
    pub(in crate::windowed) fn viewpoint(&self) -> game::viewpoint::Viewpoint {
        self.viewpoint
    }

    /// Issue #194 (B10): this player's profile layouts changed; the next sample rebuilds the
    /// pad table from them.
    pub(in crate::windowed) fn relayout(&mut self) {
        self.pad_stale = true;
    }

    /// A player after the first now plays in `context` (on foot, a vehicle, a flyer): their pad
    /// table is their own profile's (`profile`, their seat's, [`seat_settings`]) layout for it,
    /// rebuilt when the context changes or their layouts did ([`PlayerInput::relayout`]). The
    /// first player's is `App::context_gamepad`, the host profile's (issue #194 B22).
    fn follow_context(
        &mut self,
        context: Option<ui::menu::ControlContext>,
        profile: &game::profile::Settings,
    ) {
        if context != self.pad_context || self.pad_stale {
            self.pad_stale = false;
            self.pad_context = context;
            self.pad_map = context
                .and_then(|c| game::control_layout::gamepad(c, profile.gamepad_layouts.get(c)));
        }
    }

    fn reset_stick(&mut self) {
        self.gameplay_stick.select(None);
        self.stick_range.reset();
    }
}

/// The split match's state on the `App`.
pub(in crate::windowed) struct SplitState {
    pub seats: game::local_seats::Seats<gilrs::GamepadId>,
    pub inputs: Vec<PlayerInput>,
    /// This frame's finished pad of each player, by local index.
    pub pads: Vec<PadState>,
    /// The held-stick look each player's view folds in (`render_blend::blend_in_looking`).
    pub looks: Vec<PadState>,
    /// Which players the walk has suspended (a lost controller), and what to give back to each
    /// when their pad returns (`WalkSim::suspend_local_player`).
    pub presence: game::local_seats::Presence,
    pub away: Vec<Option<game::walk::Suspended>>,
    /// Each player's own scoreboard hold, stepped by their own pad (the keyboard's key only for
    /// the keyboard player), by local index (issue #194 B9).
    pub score_holds: Vec<ui::scoreboard::Hold>,
    /// Each further player's own round-end stats pages, by local index (index 0 stays `None`:
    /// the first player's are `walk.round_end`'s). `None` while that player is away; made when
    /// they come back while the pages are up ([`fill_split_stats_pages`]).
    pub stats: Vec<Option<ui::stats::StatsScreens>>,
    /// Each player's menu buttons held last frame, for their own pad's menu edges
    /// ([`App::seat_menu_edges`]).
    pub menu_prev: Vec<u32>,
    /// A LAN round's pads of the Players-screen seats the session left out (refused, or with no
    /// profile; issue #194 B20): they hold no seat here, and never take a waiting player's.
    pub left_out: Vec<gilrs::GamepadId>,
    /// A LAN round's split (issue #194 B20).
    pub lan: bool,
    /// The open questions of returning pads that could be several waiting players' (identical
    /// models): a holder picks whose seat their pad takes ([`PadPick`]). One per pad model, each
    /// over its own waiting players; identical pads share theirs.
    pub picks: Vec<PadPick<gilrs::GamepadId>>,
}

/// An open [`game::local_seats::SeatChoice`] and its pads' pick buttons held last frame: any of
/// its pads' d-pad left or right moves the pick between the waiting players and that pad's A
/// (South) takes the seat shown, each on the press. A second identical pad that comes back while
/// the question is open joins it ([`PadPick::join`]), so either holder can answer and nobody is
/// left pressing a d-pad that does nothing. The waiting cells say it ([`pick_line`]).
pub(in crate::windowed) struct PadPick<Id> {
    /// The question; its `pad` is the pad that opened it (whose model the lines name).
    pub choice: game::local_seats::SeatChoice<Id>,
    /// Every pad that may answer, with its pick buttons held last frame.
    pads: Vec<(Id, u8)>,
}

/// [`PadPick`]'s buttons as bits.
pub(in crate::windowed) const PICK_LEFT: u8 = 1;
pub(in crate::windowed) const PICK_RIGHT: u8 = 2;
pub(in crate::windowed) const PICK_TAKE: u8 = 4;

impl<Id: Copy + PartialEq> PadPick<Id> {
    /// A pick opened by a press whose buttons (`held`) are still down: none of them counts until
    /// it is let go and pressed again.
    pub fn new(choice: game::local_seats::SeatChoice<Id>, held: u8) -> Self {
        let pads = vec![(choice.pad, held)];
        PadPick { choice, pads }
    }

    /// Another returning pad's own question (`other`, opened by its press with `held` down)
    /// while this one is open: when the two share a waiting player (identical pads of one
    /// group), that pad may answer this question too, and its press now counts as the opening
    /// one did. `false` for an unrelated question, which waits for this one to close.
    pub fn join(&mut self, other: &game::local_seats::SeatChoice<Id>, held: u8) -> bool {
        if !other.seats.iter().any(|i| self.choice.seats.contains(i)) {
            return false;
        }
        if !self.has(other.pad) {
            self.pads.push((other.pad, held));
        }
        for i in &other.seats {
            if !self.choice.seats.contains(i) {
                self.choice.seats.push(*i);
            }
        }
        self.choice.seats.sort_unstable();
        true
    }

    /// May `pad` answer this question?
    pub fn has(&self, pad: Id) -> bool {
        self.pads.iter().any(|(p, _)| *p == pad)
    }

    /// The pads that may answer, in the order they came.
    pub fn pads(&self) -> Vec<Id> {
        self.pads.iter().map(|(p, _)| *p).collect()
    }

    /// Only the pads `keep` allows may answer from now on (one that went, or was seated).
    /// `false` when none is left.
    pub fn retain(&mut self, keep: impl Fn(Id) -> bool) -> bool {
        self.pads.retain(|(p, _)| keep(*p));
        if let Some((p, _)) = self.pads.first() {
            if !self.has(self.choice.pad) {
                self.choice.pad = *p;
            }
        }
        !self.pads.is_empty()
    }

    /// One frame of `pad`'s pick buttons (`bits`): moves the pick, or takes its seat in `seats`
    /// for that pad (the local player it took; the pad answers no more). Seats that stopped
    /// waiting drop out first.
    pub fn step(
        &mut self,
        seats: &mut game::local_seats::Seats<Id>,
        pad: Id,
        bits: u8,
    ) -> Option<usize> {
        seats.still_waiting(&mut self.choice);
        let k = self.pads.iter().position(|(p, _)| *p == pad)?;
        let pressed = bits & !self.pads[k].1;
        self.pads[k].1 = bits;
        if pressed & PICK_LEFT != 0 {
            self.choice.step(-1);
        }
        if pressed & PICK_RIGHT != 0 {
            self.choice.step(1);
        }
        if pressed & PICK_TAKE != 0 {
            let opener = self.choice.pad;
            self.choice.pad = pad;
            let took = seats.take_choice(&self.choice);
            self.choice.pad = opener;
            if took.is_some() {
                self.pads.remove(k);
                if let Some((p, _)) = self.pads.first().filter(|_| opener == pad) {
                    self.choice.pad = *p;
                }
                seats.still_waiting(&mut self.choice);
            }
            return took;
        }
        None
    }

    /// Is the pick still a question? Fewer than two waiting players left, or no pad left to
    /// answer: each pad's next press goes to the one left by itself.
    pub fn open(&self) -> bool {
        self.choice.seats.len() >= 2 && !self.pads.is_empty()
    }
}

/// The acting line of waiting player `i`'s lost-controller notice while a returning pad asks
/// whose it is ([`PadPick`]), its glyph's sheet then its label: the player it would take now sees
/// the pad's A glyph, the others its d-pad left/right glyph (which moves the pick to them), in the pad's own button names under waiting
/// player `i`'s own icon choice ([`seat_settings`]: `host` for the first player, their seat's
/// profile otherwise; `fallback` without the pad). `None` for anyone else (their notice keeps
/// its own line). By field, as the window's overlay pass borrows the `App`.
pub(in crate::windowed) fn pick_line(
    split: Option<&SplitState>,
    gilrs: Option<&gilrs::Gilrs>,
    host: &game::profile::Profile,
    profiles: &[Option<game::profile::Profile>; 4],
    fallback: game::input_icons::Device,
    i: usize,
    name: &str,
) -> Option<(&'static str, String)> {
    let split = split?;
    let pick = split.picks.iter().find(|p| p.choice.seats.contains(&i))?;
    let family = gilrs.map(|g| input::pad_family(&g.gamepad(pick.choice.pad)));
    pick_line_for(
        pick,
        &split.seats,
        host,
        profiles,
        family,
        fallback,
        i,
        name,
    )
}

/// [`pick_line`] with the pad's `family` known (generic over the pad id, for tests).
#[allow(clippy::too_many_arguments)]
fn pick_line_for<Id: Copy + PartialEq>(
    pick: &PadPick<Id>,
    seats: &game::local_seats::Seats<Id>,
    host: &game::profile::Profile,
    profiles: &[Option<game::profile::Profile>; 4],
    family: Option<input::PadFamily>,
    fallback: game::input_icons::Device,
    i: usize,
    name: &str,
) -> Option<(&'static str, String)> {
    if !pick.choice.seats.contains(&i) {
        return None;
    }
    let choice = seat_settings(host, profiles, seats, i).input_icons;
    let device = game::split_ui::seat_device(false, family, fallback);
    let set = game::input_icons::set_for(choice, device);
    Some(game::split_ui::pick_line(
        set,
        pick.choice.seat() == Some(i),
        name,
    ))
}

/// A pad's [`PadPick`] buttons held now.
fn pick_bits(pad: &gilrs::Gamepad<'_>) -> u8 {
    use gilrs::{Axis, Button};
    let x = pad.value(Axis::LeftStickX);
    let mut bits = 0;
    if input::is_down(pad, Button::DPadLeft) || x <= -input::DELIBERATE {
        bits |= PICK_LEFT;
    }
    if input::is_down(pad, Button::DPadRight) || x >= input::DELIBERATE {
        bits |= PICK_RIGHT;
    }
    if input::is_down(pad, Button::South) {
        bits |= PICK_TAKE;
    }
    bits
}

impl SplitState {
    /// The seats as the match left them, for the Players screen (`Seats::party_owners`); a LAN
    /// round also hands back `party`'s seats it left out (refused, or with no profile).
    pub fn party_back(&self, party: &Party) -> Party {
        let mut back = self.seats.party_owners();
        if self.lan {
            for i in 0..input::owners::MAX_LOCAL_PLAYERS {
                back.take_seat_from(party, i);
            }
        }
        back
    }

    /// Carry each player's own state from `old` (a LAN round's line-up changed, issue #194 B20):
    /// a player on the same seat keeps their input, held board, stats pages and menu edges.
    pub fn carry_from(&mut self, mut old: SplitState) {
        for k in 0..self.inputs.len() {
            let seat = self.seats.seat_number(k);
            let Some(j) = (0..old.inputs.len()).find(|&j| old.seats.seat_number(j) == seat) else {
                continue;
            };
            std::mem::swap(&mut self.inputs[k], &mut old.inputs[j]);
            if let (Some(h), Some(o)) = (self.score_holds.get_mut(k), old.score_holds.get(j)) {
                *h = *o;
            }
            if let Some(m) = old.menu_prev.get(j) {
                self.menu_prev[k] = *m;
            }
        }
    }

    /// Every player's pad for the sim, indexed by slot, with player 0's own `pad0` (it went
    /// through the blend's fire-lag stamp and, in a LAN round, the netplay pad) and each
    /// player's viewpoint stamped as the single player's profile stamps theirs.
    pub fn finish_pads(&self, pad0: PadState, walk: &game::walk::WalkSim) -> Vec<PadState> {
        let n = walk.local_count();
        let top = (0..n)
            .map(|i| usize::from(walk.local_slot_of(i)))
            .max()
            .unwrap_or(0);
        let mut out = vec![PadState::default(); top + 1];
        for i in 0..n {
            let pad = if i == 0 {
                pad0
            } else {
                self.pads.get(i).copied().unwrap_or_default()
            };
            let view = self
                .inputs
                .get(i)
                .map_or(Default::default(), |p| p.viewpoint);
            out[usize::from(walk.local_slot_of(i))] = view.stamp(pad);
        }
        out
    }
}

/// The pad the keyboard and mouse fed this frame, which `Kbm::consumed` takes back: the
/// keyboard player's own (`pads` by local player, `pad0` the first player's finished pad);
/// neutral when no seat holds the keyboard.
pub(in crate::windowed) fn keyboard_pad(
    keyboard: Option<usize>,
    pads: &[PadState],
    pad0: PadState,
) -> PadState {
    match keyboard {
        Some(0) => pad0,
        Some(k) => pads.get(k).copied().unwrap_or_default(),
        None => PadState::default(),
    }
}

/// How many people a split battle seats: the Players screen's party (two or more), else the
/// `--local-players` flag, else 0 (one player).
pub(in crate::windowed) fn wanted_players(party: Option<&Party>) -> usize {
    match party.filter(|p| p.count() >= 2) {
        Some(p) => p.count(),
        None => config().map_or(0, |c| c.players),
    }
}

/// The Players screen's seats, by gamepad id.
pub(in crate::windowed) type Party = input::owners::PadOwners<gilrs::GamepadId>;

/// Seat the local players on a freshly built walk and open their spawn screens: the Players
/// screen's `party` when one is given (two or more seated), else the developer flag. `None` when
/// neither asks for split, or no second player could be seated.
///
/// A LAN round (`lan`: each walk local's Players-screen seat, issue #194 B20) seats nobody
/// here: the session gave this machine's further players their slots and the round's bundles
/// seat them (`WalkSim::adopt_local`, `Peer::round_built`). Only the Players screen's party
/// splits a LAN round (the developer flag never does), one view per walk local.
pub(in crate::windowed) fn seat(
    walk: &mut game::walk::WalkSim,
    mission: Option<&game::mission::MissionDef>,
    lan: Option<&[usize]>,
    viewpoint: game::viewpoint::Viewpoint,
    party: Option<&Party>,
) -> Option<SplitState> {
    let party = party.filter(|p| p.count() >= 2);
    let cfg = config();
    if party.is_none() && cfg.is_none() {
        return None;
    }
    if let Some(lan_seats) = lan {
        return seat_lan(walk, lan_seats, viewpoint, party?);
    }
    let wanted = wanted_players(party);
    super::super::battle::seat_local_players(walk, wanted, mission);
    let n = walk.local_count();
    if n < 2 {
        eprintln!(
            "splitscreen: {wanted} players wanted, no second player could be seated on this map; \
             playing one view"
        );
        return None;
    }
    // A local player's own blend is on so its look folds in (it has no previous pose, so only
    // the look preview is drawn; the world blend is the first player's).
    let on = walk.blend.on;
    for i in 1..n {
        walk.with_local(i, |w| w.blend.on = on);
    }
    let seats = match party {
        Some(p) => game::local_seats::Seats::from_owners(p.clone(), n),
        None => game::local_seats::Seats::new(n, cfg.is_some_and(|c| c.keyboard)),
    };
    println!(
        "split: {n} local players{}, keyboard {}",
        if party.is_some() {
            " from the Players screen"
        } else {
            ""
        },
        match seats.keyboard_player() {
            Some(i) => format!("is player {i}"),
            None => "unbound".to_string(),
        }
    );
    Some(SplitState {
        seats,
        inputs: (0..n).map(|_| PlayerInput::new(viewpoint)).collect(),
        pads: Vec::new(),
        looks: Vec::new(),
        presence: game::local_seats::Presence::new(n),
        away: vec![None; n],
        score_holds: vec![Default::default(); n],
        stats: vec![None; n],
        menu_prev: vec![0; n],
        left_out: Vec::new(),
        lan: false,
        picks: Vec::new(),
    })
}

/// [`seat`] for a LAN round (issue #194 B20): the walk's locals are the session's (local 0 this
/// machine's first player, then the further players in their slots' adopt order), and
/// `lan_seats[k]` is walk local `k`'s Players-screen seat. Each local is mapped to its own seat
/// whatever the order, so each view keeps its own pad. The party's other seats (a seat the host
/// refused, or one with no profile) hold nothing here; their pads are kept on `left_out` so they
/// never take a waiting player's seat. `None` with one local.
pub(in crate::windowed) fn seat_lan(
    walk: &mut game::walk::WalkSim,
    lan_seats: &[usize],
    viewpoint: game::viewpoint::Viewpoint,
    party: &Party,
) -> Option<SplitState> {
    let n = walk.local_count();
    if n < 2 || lan_seats.len() != n {
        return None;
    }
    let seats = game::local_seats::Seats::from_seats(party, lan_seats);
    let left_out = left_out_pads(party, lan_seats);
    let on = walk.blend.on;
    for i in 1..n {
        walk.with_local(i, |w| w.blend.on = on);
    }
    println!(
        "split: {n} local players in a LAN round, keyboard {}",
        match seats.keyboard_player() {
            Some(i) => format!("is player {i}"),
            None => "unbound".to_string(),
        }
    );
    Some(SplitState {
        seats,
        inputs: (0..n).map(|_| PlayerInput::new(viewpoint)).collect(),
        pads: Vec::new(),
        looks: Vec::new(),
        presence: game::local_seats::Presence::new(n),
        away: vec![None; n],
        score_holds: vec![Default::default(); n],
        stats: vec![None; n],
        menu_prev: vec![0; n],
        left_out,
        lan: true,
        picks: Vec::new(),
    })
}

/// The pads of the party's seats that are not among `lan_seats` (issue #194 B20): seated on the
/// Players screen, left out of the LAN round.
pub(in crate::windowed) fn left_out_pads<Id: Copy + PartialEq>(
    party: &input::owners::PadOwners<Id>,
    lan_seats: &[usize],
) -> Vec<Id> {
    (0..input::owners::MAX_LOCAL_PLAYERS)
        .filter(|s| party.is_seated(*s) && !lan_seats.contains(s))
        .filter_map(|s| party.pad_of(s).or_else(|| party.lost_pad(s)))
        .collect()
}

/// The cells of `n` local players in a `w` x `h` window under the host profile's Splitscreen
/// options (`split_layout::arranged`), `seat_of` giving each local player's seat number.
pub(in crate::windowed) fn split_cells_for(
    options: &game::profile::SplitscreenSettings,
    n: usize,
    w: u32,
    h: u32,
    seat_of: impl Fn(usize) -> usize,
) -> Vec<Cell> {
    let mut options = *options;
    options.normalize();
    game::split_layout::arranged(n, w, h, options.two_player.layout(), options.cells, seat_of)
}

/// Issue #194 B20: each walk local's Players-screen seat in a LAN round, from the session's
/// locals (local 0 is this machine's first player: the lowest seated seat no further player
/// holds).
pub(in crate::windowed) fn lan_seats_of(
    locals: &[super::lan::LanLocal],
    party: Option<&Party>,
    walk: &game::walk::WalkSim,
) -> Vec<usize> {
    let theirs: Vec<usize> = locals.iter().map(|l| usize::from(l.seat)).collect();
    let first = party
        .and_then(|p| {
            (0..input::owners::MAX_LOCAL_PLAYERS).find(|s| p.is_seated(*s) && !theirs.contains(s))
        })
        .unwrap_or(0);
    (0..walk.local_count())
        .map(|k| match k {
            0 => first,
            _ => {
                let slot = walk.local_slot_of(k);
                locals
                    .iter()
                    .find(|l| l.slot == Some(slot))
                    .map_or(k, |l| usize::from(l.seat))
            }
        })
        .collect()
}

/// Each walk local's seat number for the round's record and career flush: the split state's,
/// else a LAN round's session seats ([`lan_seats_of`]: the further players' lent profiles are
/// their own even with one view), else the local index itself.
pub(in crate::windowed) fn round_seat_numbers(
    split: Option<&SplitState>,
    netplay: Option<&super::lan::NetplaySession>,
    party: Option<&Party>,
    walk: &game::walk::WalkSim,
) -> Vec<usize> {
    match (split, netplay) {
        (None, Some(n)) => lan_seats_of(&n.locals, party, walk),
        _ => seat_numbers(split, walk.local_count()),
    }
}

/// Each of `n` local players' Players-screen seat numbers, snapshotted (the split state may be
/// gone by the time a round is recorded); the local index itself when not split.
pub(in crate::windowed) fn seat_numbers(split: Option<&SplitState>, n: usize) -> Vec<usize> {
    match split {
        Some(s) => (0..n).map(|k| s.seats.seat_number(k)).collect(),
        None => (0..n).collect(),
    }
}

/// The settings local player `i` plays with (issue #194, `SPLITSCREEN.md` 18.1 item 6): their own
/// seat's profile (`App.split_profiles`), else the host's (`App.profile`: the first player, and a
/// seat with no profile, as the developer flag's).
pub(in crate::windowed) fn seat_settings<'a, Id: Copy + PartialEq>(
    host: &'a game::profile::Profile,
    profiles: &'a [Option<game::profile::Profile>; 4],
    seats: &game::local_seats::Seats<Id>,
    i: usize,
) -> &'a game::profile::Settings {
    let seat = profile_seat(seats, i);
    match profiles.get(seat).and_then(Option::as_ref) {
        Some(p) if seat != 0 => &p.settings,
        _ => &host.settings,
    }
}

/// A split player's in-battle view toggle (`v`, the view they now play in), kept in their own
/// profile (`menu_actions::opener_profile`: the host's for the first player, their seat's
/// otherwise) and saved, as the single player's toggle is: the Game Menu's Viewpoint row and
/// their next round then agree with what they see. A seat with no profile keeps it for the
/// round only (its `SplitInput`).
pub(in crate::windowed) fn keep_viewpoint<Id: Copy + PartialEq>(
    host: &mut game::profile::Profile,
    seat_profiles: &mut [Option<game::profile::Profile>; 4],
    seats: &game::local_seats::Seats<Id>,
    i: usize,
    v: game::viewpoint::Viewpoint,
) {
    let Some(p) = super::menu_actions::opener_profile(host, seat_profiles, Some(seats), i) else {
        return;
    };
    if p.settings.viewpoint != v {
        p.settings.viewpoint = v;
        if let Err(e) = p.save() {
            eprintln!("profile autosave failed: {e:#}");
        }
    }
}

/// Issue #194 (B10): the seat whose profile local player `i` keeps their settings in: 0 (the
/// host's, `App.profile`) for the first player whichever Players-screen seat they sit in
/// (`Seats::from_owners` closes up empty seats), else their own seat number.
pub(in crate::windowed) fn profile_seat<Id: Copy + PartialEq>(
    seats: &game::local_seats::Seats<Id>,
    i: usize,
) -> usize {
    match i {
        0 => 0,
        _ => seats.seat_number(i),
    }
}

/// The icon set the Controls editor names Players-screen seat `seat`'s pad buttons in: that
/// player's own icon choice (`input_icons`, a Player-scoped setting) on their own pad's family
/// (`family(i)` for local `i`; `fallback` without a pad). The editor shows pad buttons, so the
/// keyboard player's are named for a pad too. `None` when no local player sits on `seat`.
pub(in crate::windowed) fn seat_icon_set<Id: Copy + PartialEq>(
    host: &game::profile::Profile,
    profiles: &[Option<game::profile::Profile>; 4],
    seats: &game::local_seats::Seats<Id>,
    seat: usize,
    family: impl Fn(usize) -> Option<input::PadFamily>,
    fallback: game::input_icons::Device,
) -> Option<ui::input_icons::IconSet> {
    let i = (0..seats.players()).find(|&i| profile_seat(seats, i) == seat)?;
    let settings = seat_settings(host, profiles, seats, i);
    let device = game::split_ui::seat_device(false, family(i), fallback);
    Some(game::input_icons::set_for(settings.input_icons, device))
}

/// Issue #194 (B10): the Controls editor for the Game Menu's opener `owner` (`None`: not split),
/// on their own profile's layouts and writing back to it ([`profile_seat`]). `None` when the
/// opener's seat has no profile (the developer flag): nothing could keep the edit.
pub(in crate::windowed) fn controls_for(
    host: &game::profile::Profile,
    profiles: &[Option<game::profile::Profile>; 4],
    split: Option<&SplitState>,
    owner: Option<usize>,
    device: ui::menu::SettingsPage,
    context: ui::menu::ControlContext,
) -> Option<super::front::Controls> {
    let Some((s, o)) = split.zip(owner) else {
        return Some(super::front::Controls::new(device, context, &host.settings));
    };
    let seat = profile_seat(&s.seats, o);
    if seat != 0 && profiles.get(seat).and_then(Option::as_ref).is_none() {
        return None;
    }
    let settings = seat_settings(host, profiles, &s.seats, o);
    Some(super::front::Controls::new(device, context, settings).for_seat(seat))
}

/// The line the feed shows when a seat with no profile asks for the Controls editor.
pub(in crate::windowed) const NO_PROFILE_BINDINGS: &str =
    "This player has no profile: their bindings can't be changed";

/// [`App::keyboard_settings`] from the fields it reads, for a caller that holds other fields of
/// the App mutably (the Game Menu's actions, while the walk is borrowed).
pub(in crate::windowed) fn keyboard_settings_of<'a, Id: Copy + PartialEq>(
    host: &'a game::profile::Profile,
    profiles: &'a [Option<game::profile::Profile>; 4],
    seats: Option<&game::local_seats::Seats<Id>>,
) -> &'a game::profile::Settings {
    match seats.and_then(|s| Some((s, s.keyboard_player()?))) {
        Some((s, k)) => seat_settings(host, profiles, s, k),
        None => &host.settings,
    }
}

/// What a player after the first needs to rumble their own pad: the first player's is on the tick
/// hook.
#[derive(Default)]
pub(in crate::windowed) struct ExtraRumble {
    feedback: game::rumble::TickFeedback,
    previous_mount: Option<core::vehicle::Mount>,
    effect: Option<gilrs::ff::Effect>,
}

impl ExtraRumble {
    /// One stepped tick of local player `k`: a pulse for their unit goes to their pad (`ids[k]`),
    /// with the hook's strength and fire setting.
    /// `rumble` is that player's own (strength, on fire) from their seat's profile.
    pub fn tick(
        &mut self,
        walk: &game::walk::WalkSim,
        k: usize,
        hook: &mut super::tick_hook::TickHook<'_>,
        ids: &[Option<gilrs::GamepadId>],
        rumble: (f32, bool),
    ) {
        let unit = walk.local_unit(k);
        let (strength, on_fire) = rumble;
        if strength > 0.0 {
            let pulse = self
                .feedback
                .pulse(&walk.sim, unit, self.previous_mount, on_fire);
            if let (Some(pulse), Some(Some(id)), Some(g)) =
                (pulse, ids.get(k), hook.gilrs.as_deref_mut())
            {
                self.effect = input::rumble(g, *id, pulse, strength);
            }
        }
        self.previous_mount = walk.sim.unit(unit).and_then(|u| u.mount);
    }
}

/// One message feed for each of `players` after the first ([`wanted_players`]). Each is ticked
/// for its player's unit so their view shows their own messages; none of them is ever passed to
/// the scoring paths (`netplay::score`, `heroes::tick`): the round's one feed scores.
pub(in crate::windowed) fn extra_feeds(
    assets_dir: &std::path::Path,
    source_game: &str,
    walk: &game::walk::WalkSim,
    players: usize,
) -> Vec<game::event_feed::EventFeed> {
    (1..players)
        .map(|_| game::event_feed::EventFeed::for_walk(assets_dir, source_game, walk))
        .collect()
}

/// A seated player's name by Players screen seat number `i`, wherever it shows in battle: the
/// kill feed, awards, name tags, stats pages and the lost-controller notice (through
/// `App.names`, [`battle_names`]). Seat 0 plays on the host's profile (`host`), seats 1-3 on
/// their own seat profile (`App.split_profiles`, issue #194 B4), each filtered as a single
/// player's name is (`single_player_name`); a seat with no profile, or a name the filter
/// refuses, is "Player N".
pub(in crate::windowed) fn seat_name(
    i: usize,
    host: &str,
    profiles: &[Option<game::profile::Profile>; 4],
) -> String {
    let name = match i {
        0 => Some(host),
        _ => profiles
            .get(i)
            .and_then(Option::as_ref)
            .map(|p| p.name.as_str()),
    };
    name.and_then(|n| super::single_player_name(false, n))
        .unwrap_or_else(|| format!("Player {}", i + 1))
}

/// Each of a split battle's local players named by its Players screen seat ([`seat_name`] of
/// [`game::local_seats::Seats::seat_number`]), keyed by that player's sim slot (`slots[k]`,
/// `WalkSim::local_slot_of(k)`), for `App.names`: a battle that closed a free seat between its
/// players still calls each of them by their own profile, or by the number they had on the
/// Players screen.
pub(in crate::windowed) fn battle_names<Id: Copy + PartialEq>(
    seats: &game::local_seats::Seats<Id>,
    slots: &[u8],
    host: &str,
    profiles: &[Option<game::profile::Profile>; 4],
) -> Vec<(u8, String)> {
    slots
        .iter()
        .take(input::owners::MAX_LOCAL_PLAYERS)
        .enumerate()
        .map(|(k, &slot)| (slot, local_name(seats, k, host, profiles)))
        .collect()
}

/// Local player `k`'s battle name ([`battle_names`]): their seat's [`seat_name`], except that
/// local player 0 always plays on the host's profile (its settings and career, as
/// [`seat_settings`]), so on a seat other than P1 it is "Player N" by seat number rather than
/// that seat's profile name.
pub(in crate::windowed) fn local_name<Id: Copy + PartialEq>(
    seats: &game::local_seats::Seats<Id>,
    k: usize,
    host: &str,
    profiles: &[Option<game::profile::Profile>; 4],
) -> String {
    match seats.seat_number(k) {
        seat if k == 0 && seat != 0 => format!("Player {}", seat + 1),
        seat => seat_name(seat, host, profiles),
    }
}

/// The second pass of one stepped split tick (issue #194 B4): every view notes its medal lines
/// once every local player's points are scored for the tick (the first's in `heroes::tick`,
/// the others' after their feed's tick), so a medal a later player earned this tick reaches the
/// earlier views too before the frame's next `TickState` takes the new counts as seen. `feed`
/// and `seen0` are the first player's; `feeds[k - 1]` and `views_seen[k - 1]` player `k`'s. A
/// suspended player's feed is skipped, as its tick is. Noting twice never repeats a line.
pub(in crate::windowed) fn note_split_medals(
    walk: &game::walk::WalkSim,
    feed: &mut game::event_feed::EventFeed,
    seen0: &mut std::collections::BTreeMap<u8, core::sim::MedalCounts>,
    feeds: &mut [game::event_feed::EventFeed],
    views_seen: &mut [std::collections::BTreeMap<u8, core::sim::MedalCounts>],
) {
    feed.note_medals(&walk.sim, walk.player, walk.local_slot, seen0);
    for k in 1..walk.local_count() {
        if walk.is_suspended(k) {
            continue;
        }
        if let (Some(f), Some(seen)) = (feeds.get_mut(k - 1), views_seen.get_mut(k - 1)) {
            f.note_medals(&walk.sim, walk.local_unit(k), walk.local_slot_of(k), seen);
        }
    }
}

/// Issue #194 (B9, B21): the HUD model of each view after the first, in local order from 1, and
/// whether that view's spawn screens are up: each built under `with_local(i)` from that player's
/// own feed (`feeds[i - 1]`), with their own side's banner and their own stats pages (`stats[i]`)
/// at the round's end. An away view (`away[i]`, or a missing feed) gets `None`: its handle may
/// name a bot's unit. Every kind of round builds them the same way, a space round included.
pub(in crate::windowed) fn further_view_models(
    walk: &mut game::walk::WalkSim,
    feeds: &mut [game::event_feed::EventFeed],
    away: &[bool],
    aspect: f32,
    stats: &[Option<ui::stats::StatsScreens>],
) -> Vec<(Option<ui::hud::HudModel>, bool)> {
    (1..away.len())
        .map(|i| match feeds.get_mut(i - 1) {
            Some(feed) if !away[i] => walk.with_local(i, |w| {
                let mut model = super::hud_model(w, feed, aspect);
                // Their own stats pages, not the first player's.
                if let Some(end) = model.round_end.as_mut() {
                    // Their own side's banner (B22).
                    own_banner(w, end);
                    end.stats = stats.get(i).cloned().flatten();
                }
                (Some(model), super::spawn_screen_up(w))
            }),
            _ => (None, false),
        })
        .collect()
}

/// A further view's round-end banner for its own side (issue #194 B22): the walk's round end is
/// shared and its result the first player's (`WalkSim::follow_round_end`), so view `w` (inside
/// `with_local`) takes VICTORY or DEFEAT from its own player's side. The round-end music and
/// announcer stay the first player's: the speakers are shared.
pub(in crate::windowed) fn own_banner(w: &game::walk::WalkSim, end: &mut ui::hud::RoundEndModel) {
    if let Some(outcome) = w.sim.outcome {
        end.result = game::round_end::result_for(outcome, w.viewer_team()).banner();
    }
}

/// What a player's cell shows while their controller is gone ([`game::split_ui::lost_notice`]).
pub(in crate::windowed) use game::split_ui::lost_notice;

/// A pad that is being used on purpose: a button down, or a stick or trigger well past rest.
fn pad_wants_in(pad: &gilrs::Gamepad<'_>) -> bool {
    use gilrs::{Axis, Button};
    const BUTTONS: [Button; 12] = [
        Button::South,
        Button::East,
        Button::North,
        Button::West,
        Button::Start,
        Button::Select,
        Button::LeftTrigger,
        Button::RightTrigger,
        Button::DPadUp,
        Button::DPadDown,
        Button::DPadLeft,
        Button::DPadRight,
    ];
    const AXES: [Axis; 4] = [
        Axis::LeftStickX,
        Axis::LeftStickY,
        Axis::RightStickX,
        Axis::RightStickY,
    ];
    BUTTONS.iter().any(|b| input::is_down(pad, *b))
        || AXES
            .iter()
            .any(|a| pad.value(*a).abs() >= input::DELIBERATE)
        || [Button::LeftTrigger2, Button::RightTrigger2]
            .iter()
            .any(|b| input::is_down(pad, *b))
}

/// The feed line when a split battle could seat only its first player (integration finding).
pub(in crate::windowed) const SEAT_FAILED: &str =
    "Splitscreen: only one player could be seated on this map";

impl App {
    /// After [`seat`]: split was asked for (the Players screen's seats or the developer flag)
    /// but no second player could be seated, so the battle plays one view. Say so in the feed,
    /// not only on stderr.
    pub(in crate::windowed) fn say_split_seat_failed(&mut self, walk: &game::walk::WalkSim) {
        // A LAN round (issue #194 B20): a Players-screen seat the session left out (the host
        // refused it, or it has no profile) has no cell; the battle says so, not only the lobby.
        if let (Some(party), Some(lan)) = (self.split_party.as_ref(), self.lan_split_seats(walk)) {
            let out: Vec<String> = (0..input::owners::MAX_LOCAL_PLAYERS)
                .filter(|s| party.is_seated(*s) && !lan.contains(s))
                .map(|s| {
                    let name = seat_name(s, &self.profile.name, &self.split_profiles);
                    format!("{name} could not join this game")
                })
                .collect();
            for line in out {
                self.feed
                    .say(line, ui::hud::message_color::YELLOW, walk.sim.tick);
            }
        }
        if self.split.is_none()
            && self.netplay.is_none()
            && wanted_players(self.split_party.as_ref()) >= 2
        {
            self.feed.say(
                SEAT_FAILED.to_owned(),
                ui::hud::message_color::YELLOW,
                walk.sim.tick,
            );
        }
    }

    /// Name a freshly seated split battle's local players in `App.names` by their seats'
    /// profiles, keyed by each one's sim slot ([`battle_names`]); replaces the single player's
    /// slot-0 profile name, which belongs to seat P1 only. Every feed follows
    /// ([`App::feed_names`]).
    pub(in crate::windowed) fn name_split_players(&mut self, walk: &game::walk::WalkSim) {
        // A LAN round names every human from the session (`lan::net_names_to_feed`), this
        // machine's players included (issue #194 B20).
        if let Some(s) = self.split.as_ref().filter(|_| self.netplay.is_none()) {
            let slots: Vec<u8> = (0..s.inputs.len().min(walk.local_count()))
                .map(|k| walk.local_slot_of(k))
                .collect();
            let names = battle_names(&s.seats, &slots, &self.profile.name, &self.split_profiles);
            self.names
                .set(names.iter().map(|(slot, n)| (*slot, n.as_str())));
        }
        self.feed_names();
    }

    /// Copy `App.names` into the round's feed and every split feed when its generation moved
    /// since the last copy (or the feeds are new). Each feed's own name map is its cache.
    pub(in crate::windowed) fn feed_names(&mut self) {
        let generation = self.names.generation();
        if self.names_fed == Some(generation) {
            return;
        }
        self.feed.set_player_names(self.names.iter());
        for feed in &mut self.split_feeds {
            feed.set_player_names(self.names.iter());
        }
        self.names_fed = Some(generation);
    }

    /// Issue #194 (`SPLITSCREEN.md` 18.2 B3): give a freshly seated split battle's further players
    /// their own seat profiles (`App.split_profiles`, lent by the Players screen): each slot's aim
    /// assist, medal career, award bonuses and class costs
    /// (`game::netplay::install_local_profile`), at this first seating only; each extra feed's
    /// opening medals read again under that player (`EventFeed::set_opening_medals`); each
    /// player's viewpoint, and the keyboard player's mouse and keys, from their own profile. A
    /// seat with no profile (the developer flag) keeps the host's settings and no career.
    pub(in crate::windowed) fn install_seat_profiles(&mut self, walk: &mut game::walk::WalkSim) {
        let lan = self.netplay.is_some();
        let Some(split) = self.split.as_mut() else {
            return;
        };
        for k in 1..walk.local_count() {
            let seat = split.seats.seat_number(k);
            if let Some(p) = self
                .split_profiles
                .get(seat)
                .and_then(Option::as_ref)
                .filter(|_| seat != 0)
            {
                // A LAN round's slots got their profiles from the session, on every machine
                // alike (`netplay::install_humans`, `AddHuman`): only what is this machine's
                // own (the view, the FOV, the pad) comes from the seat here (issue #194 B20).
                if !lan {
                    let slot = walk.local_slot_of(k);
                    game::netplay::install_local_profile(
                        walk,
                        slot,
                        &game::netplay::PlayerProfile::from_profile(p),
                    );
                }
                if let Some(input) = split.inputs.get_mut(k) {
                    input.viewpoint = p.settings.viewpoint;
                }
                // Their own FOV (issue #194 B10: the Game Menu's FOV row is theirs).
                let fov = game::walk::clamp_fov_scale(p.settings.fov_scale);
                walk.with_local(k, |w| w.fov_scale = fov);
            }
            if let Some(f) = self.split_feeds.get_mut(k - 1) {
                walk.with_local(k, |w| f.set_opening_medals(w));
            }
        }
        if let Some(kb) = split.seats.keyboard_player().filter(|&k| k > 0) {
            let settings = seat_settings(&self.profile, &self.split_profiles, &split.seats, kb);
            self.kbm.follow(settings);
            self.refresh_context_bindings();
        }
    }

    /// The settings the keyboard and mouse play with: the keyboard player's own profile's when
    /// split ([`seat_settings`]), else the host's.
    pub(in crate::windowed) fn keyboard_settings(&self) -> &game::profile::Settings {
        keyboard_settings_of(
            &self.profile,
            &self.split_profiles,
            self.split.as_ref().map(|s| &s.seats),
        )
    }

    /// The local player who holds the keyboard and mouse: their seat when split, else the
    /// first player (0 too when no seat holds it; its input is dropped then).
    pub(in crate::windowed) fn keyboard_seat(&self) -> usize {
        match self.split.as_ref() {
            Some(s) if self.split_on() => s.seats.keyboard_player().unwrap_or(0),
            _ => 0,
        }
    }

    /// Split with the Game Menu open by a known player: `Some(their pad)` (`Some(None)` when they
    /// play on the keyboard or their pad is gone), else `None` (any pad drives the menus).
    pub(in crate::windowed) fn pause_owner_pad(&self) -> Option<Option<gilrs::GamepadId>> {
        let owner = self
            .pause_owner
            .filter(|_| self.paused && self.split_on())?;
        if self.lan_menu_by_free_keyboard() {
            return Some(None);
        }
        Some(self.split.as_ref().and_then(|s| s.seats.pad_of(owner)))
    }

    /// A LAN split round's Game Menu (issue #194 B20): the local player whose cell it covers,
    /// its opener (the round runs on under it, so every other cell keeps its HUD, tags and
    /// notices, and the mouse keeps aiming for a keyboard player who did not open it). `None`
    /// out of one: an offline split menu covers the whole window and freezes everyone.
    pub(in crate::windowed) fn lan_menu_view(&self) -> Option<usize> {
        if !(self.paused && self.netplay.is_some() && self.split_on()) {
            return None;
        }
        Some(
            self.pause_opened_by
                .map(|(o, _)| o)
                .or(self.pause_owner)
                .unwrap_or_else(|| self.keyboard_seat()),
        )
    }

    /// A LAN split round's Game Menu opened by Esc with nobody on the keyboard (issue #194
    /// B20): only the keyboard drives it, and no pad player stands still for it (the round runs
    /// on under the menu, and the first player did not open it).
    pub(in crate::windowed) fn lan_menu_by_free_keyboard(&self) -> bool {
        self.netplay.is_some() && self.paused && matches!(self.pause_opened_by, Some((_, true)))
    }

    /// Split, the Game Menu open: `live[i]` says local player `i`'s own pad is connected now.
    /// Offline, a menu whose owner's pad has gone (unplugged, flat, dropped) passes to a seat
    /// that can still drive it ([`menu_driver`]), never left with nobody. A LAN round runs on
    /// under the menu, so there it is never passed on: it closes ([`lan_menu_closes`]; the
    /// opener's bot stands in, `lan_presence`), and an open Controls page goes with it,
    /// unsaved, as Back would leave it.
    pub(in crate::windowed) fn settle_split_menu(
        &mut self,
        keyboard: Option<usize>,
        live: &[bool],
    ) {
        let Some(owner) = self.pause_owner.filter(|_| self.paused) else {
            return;
        };
        let connected = |i: usize| live.get(i).copied().unwrap_or(false);
        if self.netplay.is_some() {
            let by_keys = matches!(self.pause_opened_by, Some((_, true)));
            if lan_menu_closes(owner, by_keys, live.len(), keyboard, connected) {
                self.controls = None;
                self.paused = false;
                self.pause_owner = None;
                self.pause_opened_by = None;
                self.pause_menu.reset();
            }
        } else {
            self.pause_owner = Some(menu_driver(owner, live.len(), keyboard, connected));
        }
    }

    /// Local player `i`'s own pad is seated and connected now.
    pub(in crate::windowed) fn seat_pad_connected(&self, i: usize) -> bool {
        let Some(id) = self.split.as_ref().and_then(|s| s.seats.pad_of(i)) else {
            return false;
        };
        self.gilrs
            .as_ref()
            .is_some_and(|g| g.connected_gamepad(id).is_some())
    }

    /// Split, paused: the pad that captures on the Controls editor, that of the local player
    /// whose profile the page edits (`Controls::seat`), whoever drives the menu now; `Some(None)`
    /// when they have no pad (the keyboard, or theirs is gone). `None` otherwise: the active pad
    /// captures, as it always did (issue #194 B10).
    pub(in crate::windowed) fn controls_pad(&self) -> Option<Option<gilrs::GamepadId>> {
        let seat = self.controls.as_ref()?.seat;
        let s = self
            .split
            .as_ref()
            .filter(|_| self.paused && self.split_on())?;
        let local = (0..s.seats.players()).find(|&k| profile_seat(&s.seats, k) == seat);
        Some(local.and_then(|k| s.seats.pad_of(k)))
    }

    /// Do the keyboard's menu keys drive the menus this frame? Always, except in split with the
    /// Game Menu open by someone else: the keyboard counts as its own player, or as the lowest
    /// seated player when nobody plays on it (`SPLITSCREEN.md` section 15). A menu the free
    /// keyboard opened (Esc with nobody on it) stays the keyboard's to move through and close,
    /// whichever pad drives it too once its first seat's pad has gone ([`menu_driver`]).
    pub(in crate::windowed) fn pause_keyboard_drives(&self) -> bool {
        match self.pause_owner.filter(|_| self.paused && self.split_on()) {
            Some(owner) => {
                owner == self.keyboard_seat() || matches!(self.pause_opened_by, Some((_, true)))
            }
            None => true,
        }
    }

    /// Who opens the Game Menu with this frame's Start: the keyboard's player (Esc) or the seat
    /// whose own pad pressed it (`menu_edges` reads every seated pad's Start); `None` when no
    /// seated pad did (an unseated pad's Start opens nothing).
    ///
    /// A LAN split round with nobody on the keyboard: Esc opens nothing while any seated player
    /// still has their pad ([`lan_free_keyboard_opens`]), since the round runs on under the menu
    /// and it would cover a player who did not open it.
    pub(in crate::windowed) fn pause_opener(&self, keyboard: bool) -> Option<usize> {
        let split = self.split.as_ref()?;
        if keyboard {
            if self.netplay.is_some()
                && self.split_on()
                && split.seats.keyboard_player().is_none()
                && !lan_free_keyboard_opens(split.seats.players(), |i| self.seat_pad_connected(i))
            {
                return None;
            }
            if self.netplay.is_none() && split.seats.keyboard_player().is_none() {
                return Some(free_keyboard_opener(
                    split.seats.players(),
                    |i| self.seat_pad_connected(i),
                    &self.away_flags(),
                ));
            }
            return Some(self.keyboard_seat());
        }
        self.split_start_seat
    }

    /// Split: the icon set the Controls editor for Players-screen seat `seat` names pad buttons
    /// in ([`seat_icon_set`]). `None` out of a split battle: the host's choice on the device in
    /// use, as in single player.
    pub(in crate::windowed) fn editor_icon_set(
        &self,
        seat: usize,
    ) -> Option<ui::input_icons::IconSet> {
        if !self.split_on() {
            return None;
        }
        let split = self.split.as_ref()?;
        seat_icon_set(
            &self.profile,
            &self.split_profiles,
            &split.seats,
            seat,
            |i| {
                split
                    .seats
                    .pad_of(i)
                    .zip(self.gilrs.as_ref())
                    .map(|(id, g)| input::pad_family(&g.gamepad(id)))
            },
            self.split_icon_fallback(),
        )
    }

    /// The device a split player's prompts show before their pad is known: the device in use,
    /// a pad's family for the keyboard (a split prompt is for a pad).
    fn split_icon_fallback(&self) -> game::input_icons::Device {
        match self.icon_device.device() {
            game::input_icons::Device::KeyboardMouse => {
                game::input_icons::Device::Pad(input::PadFamily::XboxOne)
            }
            pad => pad,
        }
    }

    /// Each local player's prompt icons: their own pad's (the profile's icon choice applied to
    /// their pad's family and the profile's pad bindings), the keyboard's for the keyboard player
    /// only. A player with no pad yet shows the pad family last in use.
    pub(in crate::windowed) fn split_icons(&self) -> Vec<ui::input_icons::Icons> {
        let Some(split) = self.split.as_ref() else {
            return Vec::new();
        };
        let fallback = self.split_icon_fallback();
        (0..split.inputs.len())
            .map(|i| {
                let keyboard = split.seats.keyboard_player() == Some(i);
                let family = split
                    .seats
                    .pad_of(i)
                    .zip(self.gilrs.as_ref())
                    .map(|(id, g)| input::pad_family(&g.gamepad(id)));
                let device = game::split_ui::seat_device(keyboard, family, fallback);
                let settings = seat_settings(&self.profile, &self.split_profiles, &split.seats, i);
                let set = game::input_icons::set_for(settings.input_icons, device);
                game::input_icons::with_keyboard(
                    game::input_icons::icons(set, &settings.controller_bindings),
                    &self.kbm.bindings,
                )
            })
            .collect()
    }

    /// Each local player's own menu edges this frame (issue #194 B9): their own pad's presses
    /// ([`pad_menu_bits`]), and the keyboard's menu keys for the keyboard player only. Empty when
    /// not split. Read after `menu_edges` pumped the pads this frame.
    pub(in crate::windowed) fn seat_menu_edges(&mut self) -> Vec<u32> {
        let keys = self.key_menu_edges;
        let blocked = self.loading_pad_blocked;
        let stats = self
            .walk
            .as_ref()
            .is_some_and(|(w, _)| w.round_end.as_ref().is_some_and(|r| r.stats.is_some()));
        let gilrs = self.gilrs.as_ref();
        let Some(split) = self.split.as_mut() else {
            return Vec::new();
        };
        let n = split.inputs.len();
        split.menu_prev.resize(n, 0);
        let keyboard = split.seats.keyboard_player();
        (0..n)
            .map(|i| {
                let prev = split.menu_prev[i];
                let now = match (gilrs, split.seats.pad_of(i)) {
                    (Some(g), Some(id)) if !blocked => g
                        .connected_gamepad(id)
                        .map_or(0, |p| pad_menu_bits(&p, prev, stats)),
                    _ => 0,
                };
                split.menu_prev[i] = now;
                let mut edges = game::kbm::menu_edges(now, prev, 0, false);
                if keyboard == Some(i) {
                    edges |= keys;
                }
                edges
            })
            .collect()
    }

    /// Step every local player's own scoreboard hold (issue #194 B9): their own pad's board
    /// button and Switch Weapon (`split.pads`, sampled this frame when `sampled`, else released),
    /// and the keyboard's board key for the keyboard player only.
    pub(in crate::windowed) fn step_split_score_holds(&mut self, sampled: bool, dt: f32) {
        let key = self.kbm.scoreboard_held();
        let Some(split) = self.split.as_mut() else {
            return;
        };
        let keyboard = split.seats.keyboard_player();
        let n = split.inputs.len();
        split.score_holds.resize(n, Default::default());
        for i in 0..n {
            let pad = split
                .pads
                .get(i)
                .copied()
                .filter(|_| sampled)
                .unwrap_or_default();
            split.score_holds[i].step(
                pad.buttons & core::sim::buttons::SPAWN_CLASS_PREV != 0,
                pad.buttons & core::sim::buttons::SWITCH_WEAPON != 0,
                key && keyboard == Some(i),
                dt,
            );
        }
    }

    /// Is this frame a split-screen one: the flag seated several people on the live walk.
    pub(in crate::windowed) fn split_on(&self) -> bool {
        self.split.is_some() && self.walk.as_ref().is_some_and(|(w, _)| w.local_count() > 1)
    }

    /// Issue #194 B20: each walk local's Players-screen seat in a LAN round ([`lan_seats_of`]).
    /// `None` out of a LAN round.
    pub(in crate::windowed) fn lan_split_seats(
        &self,
        walk: &game::walk::WalkSim,
    ) -> Option<Vec<usize>> {
        let n = self.netplay.as_ref()?;
        Some(lan_seats_of(&n.locals, self.split_party.as_ref(), walk))
    }

    /// The cells of this frame's views in the window, when split, by local player: the host
    /// profile's Splitscreen options (two-player split and each seat's place, `SPLITSCREEN.md`
    /// 18.1 item 4), read every frame, so a change re-lays out on the next one.
    pub(in crate::windowed) fn split_cells(&self) -> Option<Vec<Cell>> {
        if !self.split_on() {
            return None;
        }
        let (w, h) = self.renderer.as_ref()?.size();
        let n = self.walk.as_ref()?.0.local_count();
        let seats = &self.split.as_ref()?.seats;
        Some(split_cells_for(
            &self.profile.settings.splitscreen,
            n,
            w,
            h,
            |i| seats.seat_number(i),
        ))
        .filter(|c| c.len() == n)
    }

    /// Local player `i`'s name as the cells show it: `App.names` by their slot, else the
    /// Players screen's number, as `battle_names` would give.
    pub(in crate::windowed) fn local_display_name(&self, i: usize) -> String {
        let slot = self
            .walk
            .as_ref()
            .map_or(i as u8, |(w, _)| w.local_slot_of(i));
        match (self.names.get(slot), &self.split) {
            (Some(n), _) => n.to_owned(),
            (None, Some(s)) => local_name(&s.seats, i, &self.profile.name, &self.split_profiles),
            (None, None) => self.names.name_or_default(slot),
        }
    }

    /// The Splitscreen page's row names by seat (issue #194 B22, `MenuModel::split_names`): each
    /// seated player's name from the same source as the "Paused by" line and the kill feed
    /// ([`App::local_display_name`]: the session names first, then the seat's profile), through
    /// the single-player filter, so one player reads the same across the menu and the feed.
    /// `None` (the row says "P#") where nothing better than the "Player N" fallback exists, as
    /// for local player 0 on a seat other than P1, who plays on the host profile there.
    pub(in crate::windowed) fn split_seat_names<Id: Copy + PartialEq>(
        &self,
        seats: &game::local_seats::Seats<Id>,
    ) -> [Option<String>; 4] {
        let mut out: [Option<String>; 4] = Default::default();
        let count = self.walk.as_ref().map(|(w, _)| w.local_count());
        for i in 0..seats.players() {
            let seat = seats.seat_number(i);
            let slot = match &self.walk {
                Some((w, _)) if i < w.local_count() => w.local_slot_of(i),
                _ => i as u8,
            };
            let shown = if count.is_none_or(|c| i < c) {
                self.local_display_name(i)
            } else {
                local_name(seats, i, &self.profile.name, &self.split_profiles)
            };
            let fallback = |n: &str| {
                n == format!("Player {}", seat + 1)
                    || n == format!("Player {}", u32::from(slot) + 1)
            };
            if let Some(cell) = out.get_mut(seat) {
                *cell = super::single_player_name(false, &shown).filter(|n| !fallback(n));
            }
        }
        out
    }

    /// Split: the Game Menu's line naming who opened it (issue #194 B22), `opener` the local
    /// player: "Paused by" and their name as the kill feed, the waiting line and the lost notice
    /// give it ([`App::local_display_name`]: the session names first, then the seat's profile),
    /// or "Paused (keyboard)" when the keyboard opened it with nobody playing on it. `None` out of
    /// a split battle or with no menu open. Local player 0 sitting on a seat other than P1 has
    /// no profile of its own there and reads "Player N" (`local_name`). `driver` is who drives
    /// the menu now ([`menu_driver`]): once the opener's pad is gone and the menu has passed to
    /// someone else, the line names that new owner (`SPLITSCREEN.md` §4, B13b), while the rows
    /// stay the opener's.
    ///
    /// A LAN round never pauses (the round and the other cells run on under the menu), so
    /// there the line claims no pause: "Han's menu (battle runs on)", or "Keyboard menu (battle
    /// runs on)" for the keyboard with nobody on it.
    pub(in crate::windowed) fn paused_by_line(
        &self,
        opener: Option<usize>,
        driver: Option<usize>,
    ) -> Option<String> {
        let o = opener?;
        self.split.as_ref()?;
        if self.netplay.is_some() {
            let who = match self.pause_opened_by {
                Some((_, true)) => None,
                _ => Some(self.local_display_name(o)),
            };
            return Some(game::split_ui::lan_menu_line(who.as_deref()));
        }
        // The free keyboard opened it: it still drives it ([`App::pause_keyboard_drives`]),
        // whichever pad drives it too, so the line keeps saying so.
        if matches!(self.pause_opened_by, Some((_, true))) {
            return Some("Paused (keyboard)".to_string());
        }
        if let Some(d) = driver.filter(|d| *d != o) {
            return Some(format!("Paused by {}", self.local_display_name(d)));
        }
        Some(format!("Paused by {}", self.local_display_name(o)))
    }

    /// Split, round-end stats up (issue #194): each cell whose player has left their own pages,
    /// with the line naming who the round still waits for (the players here who have not left;
    /// an away player is not waited for). `away[i]`: player `i` is away.
    pub(in crate::windowed) fn split_waiting(
        &self,
        cells: &[Cell],
        away: &[bool],
    ) -> Vec<(Cell, String)> {
        let Some(r) = self
            .walk
            .as_ref()
            .and_then(|(w, _)| w.round_end.as_ref())
            .filter(|r| r.stats.is_some() && !r.done())
        else {
            return Vec::new();
        };
        let here = |i: usize| !away.get(i).copied().unwrap_or(false);
        let still: Vec<String> = (0..cells.len())
            .filter(|&i| here(i) && !r.has_left(i))
            .map(|i| self.local_display_name(i))
            .collect();
        if still.is_empty() {
            return Vec::new();
        }
        let line = game::split_ui::waiting_line(&still);
        (0..cells.len())
            .filter(|&i| here(i) && r.has_left(i))
            .map(|i| (cells[i], line.clone()))
            .collect()
    }

    /// Per view: is that player away (their controller is gone)? Empty when not split.
    pub(in crate::windowed) fn away_flags(&self) -> Vec<bool> {
        match (self.split_on(), &self.walk) {
            (true, Some((w, _))) => (0..w.local_count()).map(|i| w.is_suspended(i)).collect(),
            _ => Vec::new(),
        }
    }

    /// Seat this frame's pads (issue #194): a seated pad that is gone leaves its player waiting, a
    /// deliberate press from a free pad takes a waiting seat or a new one, and the walk suspends or
    /// resumes each player whose controller went or came back (`game::local_seats::Presence`).
    /// Run every split frame with the battle's pads ([`App::sample_split_pads`]) and also while
    /// the round's end is up, so a player who comes back during the stats gets their own pages.
    pub(in crate::windowed) fn seat_split_pads(&mut self) {
        let Some(split) = self.split.as_mut() else {
            return;
        };
        let Some((walk, _)) = self.walk.as_mut() else {
            return;
        };
        let n = walk.local_count();
        if let Some(g) = self.gilrs.as_mut() {
            let map = &self.profile.settings.controller_bindings;
            if input::pump_tracking(g, &mut self.active_pad) {
                self.pad_warning = input::log_pads_with(g, map, &self.active_pad);
            }
            for i in 0..n {
                let Some(id) = split.seats.pad_of(i) else {
                    continue;
                };
                if !g.gamepad(id).is_connected() {
                    split.seats.gone(id);
                    split.inputs[i].reset_stick();
                    println!("split: player {i} lost their controller");
                }
            }
            // A pad asking whose it is (identical models): any of its pads' d-pad picks, the
            // A of the pad pressing it takes the seat shown for that pad.
            let seats = &mut split.seats;
            let inputs = &mut split.inputs;
            split.picks.retain_mut(|p| {
                let live =
                    p.retain(|id| g.gamepad(id).is_connected() && seats.player_of(id).is_none());
                if live {
                    for id in p.pads() {
                        let pad = g.gamepad(id);
                        if let Some(i) = p.step(seats, id, pick_bits(&pad)) {
                            inputs[i].reset_stick();
                            println!("split: player {i} picked {}", pad.name());
                        }
                        if !p.open() {
                            break;
                        }
                    }
                }
                live && p.open()
            });
            for id in input::usable_ids(g) {
                let pad = g.gamepad(id);
                if split.seats.player_of(id).is_none()
                    && !split.picks.iter().any(|p| p.has(id))
                    && !split.left_out.contains(&id)
                    && pad.is_connected()
                    && pad_wants_in(&pad)
                {
                    let uuid = pad.uuid();
                    match split.seats.offer_alike(
                        id,
                        |old| g.gamepad(old).is_connected(),
                        |old| g.gamepad(old).uuid() == uuid,
                    ) {
                        Some(game::local_seats::Offered::Seat(i)) => {
                            split.inputs[i].reset_stick();
                            println!("split: player {i} is on {}", pad.name());
                        }
                        Some(game::local_seats::Offered::Choose(c)) => {
                            let held = pick_bits(&pad);
                            // An identical pad of an open question answers it too; a pad of
                            // another model opens its own (each cell names its own question).
                            match split.picks.iter_mut().position(|p| p.join(&c, held)) {
                                Some(k) => println!(
                                    "split: {} joins the pick of players {:?}",
                                    pad.name(),
                                    split.picks[k].choice.seats
                                ),
                                None => {
                                    println!(
                                        "split: {} could be players {:?}; its holder picks",
                                        pad.name(),
                                        c.seats
                                    );
                                    split.picks.push(PadPick::new(c, held));
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        // A LAN round (issue #194 B20, B17's game side): a lost pad only tells the session
        // (`Peer::set_local_away`); the round's bundles take that slot out and seat it again on
        // every machine alike. Asked every frame, so a new link hears every pad still missing.
        if let Some(net) = self.netplay.as_mut() {
            let slots: Vec<u8> = (0..n).map(|k| walk.local_slot_of(k)).collect();
            for i in
                game::local_seats::lan_presence(&split.seats, &slots, &mut net.peer, &mut net.link)
            {
                split.inputs[i].reset_stick();
            }
            return;
        }
        // A lost pad suspends its player in the walk and a returning one resumes them behind
        // their spawn screens (`game::local_seats::Presence`). Only that player is touched.
        for i in apply_presence(
            &split.seats,
            &mut split.presence,
            &mut split.away,
            walk,
            &self.split_profiles,
        ) {
            split.inputs[i].reset_stick();
        }
    }

    /// Sample every player's own pad for this frame into `split.pads` (and `split.looks`):
    /// pads join as they are used, a pad that went away leaves its player on a neutral pad.
    /// The first player's stick sample goes to the black box as the single player's does.
    pub(in crate::windowed) fn sample_split_pads(&mut self) {
        self.seat_split_pads();
        let Some(split) = self.split.as_mut() else {
            return;
        };
        let Some((walk, _)) = self.walk.as_mut() else {
            return;
        };
        let n = walk.local_count();
        let keyboard = split.seats.keyboard_player();
        if keyboard.is_none() {
            // The keyboard and mouse drive no soldier: what they pile up is dropped.
            self.kbm.drop_taps();
            self.kbm.clear_look();
        }
        split.pads.clear();
        split.looks.clear();
        for i in 0..n {
            let profile = seat_settings(&self.profile, &self.split_profiles, &split.seats, i);
            let input = &mut split.inputs[i];
            let is_keyboard = keyboard == Some(i);
            let held = split.seats.pad_of(i);
            let gilrs = self.gilrs.as_ref();
            let kbm = &mut self.kbm;
            if i > 0 {
                // Their own profile's layout for the context they play in (vehicle, flyer).
                input.follow_context(game::control_layout::current_of(walk, i), profile);
            }
            let sample0 = &mut self.stick_sample;
            if walk.is_suspended(i) {
                // Away: their unit handle may name a bot's index, so nothing reads it.
                split.pads.push(PadState::default());
                split.looks.push(PadState::default());
                continue;
            }
            // Lent out for the sample (the closure below holds `input`), given back after it.
            let own_map = input.pad_map.take();
            let mut toggled = None;
            let bindings = match i {
                0 => self.context_gamepad.as_ref(),
                _ => own_map.as_ref(),
            }
            .unwrap_or(&profile.controller_bindings);
            let pad = walk.with_local(i, |w| {
                let controls = w.flight_controls(profile.flight_controls);
                let look = if controls.is_some() {
                    1.0
                } else {
                    w.pad_look_for_seat(profile.pad_look_sensitivity)
                };
                let learned = w.player_on_foot() && !w.space_battle();
                let (raw, sample) = match (gilrs, held) {
                    (Some(g), Some(id)) if g.gamepad(id).is_connected() => {
                        let pad = g.gamepad(id);
                        if input.gameplay_stick.controller != Some(id) {
                            input.stick_range.reset();
                        }
                        input.gameplay_stick.select(Some(id));
                        let sticks = [
                            pad.value(gilrs::Axis::LeftStickX),
                            pad.value(gilrs::Axis::LeftStickY),
                        ];
                        input.stick_range.feed(sticks[0], sticks[1]);
                        let full = stick_full(&input.stick_range, learned);
                        let (state, arms) = input::pad_from_gamepad_with_arms(
                            &pad,
                            bindings,
                            input.gameplay_stick.arms,
                            look,
                            full,
                        );
                        input.gameplay_stick.arms = arms;
                        (state, (sticks, full))
                    }
                    _ => {
                        input.reset_stick();
                        (PadState::default(), ([0.0; 2], [0.0; 2]))
                    }
                };
                if i == 0 {
                    *sample0 = sample;
                }
                let pad = match (controls, is_keyboard) {
                    (Some(_), true) => kbm.flyer_pad(
                        w.map_flight_sticks(profile.flight_controls, raw)
                            .expect("flight controls selected"),
                    ),
                    (Some(_), false) => w
                        .map_flight_sticks(profile.flight_controls, raw)
                        .expect("flight controls selected"),
                    (None, true) => kbm.pad(raw),
                    (None, false) => raw,
                };
                let pad = game::pause::resume_pad(&mut input.resume_gate, pad);
                let pad = w.apply_sprint_latch(&mut input.sprint_latch, pad);
                if input.view_toggle.pressed(&pad) {
                    input.viewpoint = input.viewpoint.toggled();
                    toggled = Some(input.viewpoint);
                }
                pad
            });
            if let Some(v) = toggled {
                keep_viewpoint(
                    &mut self.profile,
                    &mut self.split_profiles,
                    &split.seats,
                    i,
                    v,
                );
            }
            input.pad_map = own_map;
            split.pads.push(pad);
            // The held stick's look; the keyboard player's mouse look is the frame's own.
            split.looks.push(PadState {
                mx: 0,
                my: 0,
                ..pad
            });
        }
    }
}

/// Everything the extra views build from, borrowed from the `App` for one frame.
pub(in crate::windowed) struct ViewParts<'a> {
    pub walk: &'a mut game::walk::WalkSim,
    pub cube: MeshHandle,
    pub gpu: &'a mut game::viewer::SceneGpu,
    pub soldier_vis: Option<&'a mut game::soldier_vis::SoldierVisuals>,
    pub vehicle_vis: &'a mut game::viewer::VehicleVisuals,
    pub deka_vis: &'a mut game::viewer::DroidekaVisuals,
    pub fx: &'a mut game::fx::Fx,
    pub holos: &'a game::holo::Holos,
    pub weather: Option<&'a mut game::weather::Weather>,
    pub bolts: &'a mut game::bolt_vis::BoltVis,
    pub fx_quad: Option<MeshHandle>,
    pub fx_sprite: Option<render::TextureHandle>,
    pub squad_icons: (Option<render::TextureHandle>, Option<render::TextureHandle>),
    /// Each extra view's own spawn-screen soldiers (`previews[i - 1]` for view `i`), kept
    /// between frames for their clips and the line-up's slide.
    pub previews: &'a mut Vec<game::spawn_preview::Preview>,
    /// Each view's spawn page this frame (its HUD model's), by local index; empty or `None`
    /// where nothing stands (no HUD, the `hud` cheat, alive).
    pub pages: &'a [Option<ui::spawn_draw::SpawnPage>],
    /// The previews' clip step (0 while the game is frozen).
    pub preview_dt: f32,
    /// A LAN round's drawn view (issue #194 B20): each further view draws its own player's
    /// look over their pending inputs and their predicted pose
    /// (`LocalView::draw_in_local`), as view 0 does with `draw_in`. `None`: the held stick's
    /// look ([`build_view`]'s `look`).
    pub lan_view: Option<&'a mut game::netplay::LocalView>,
}

/// Paints view `i`'s UI (`game::split_ui::SplitUi::paint`) into the window's image: called for
/// every view, a player who is away included.
pub(in crate::windowed) type ViewHud<'a> = &'a mut dyn FnMut(
    usize,
    Cell,
    &wgpu::Device,
    &wgpu::Queue,
    &mut wgpu::CommandEncoder,
    &wgpu::TextureView,
);

/// The lowest seat whose own pad's Start went down this frame: `pressed[i]` is seat `i`'s Start
/// now, `prev` last frame's (updated here, so a held Start counts once).
pub(in crate::windowed) fn seat_start_edge(
    prev: &mut Vec<bool>,
    pressed: &[bool],
) -> Option<usize> {
    prev.resize(pressed.len(), false);
    let edge = (0..pressed.len()).find(|&i| pressed[i] && !prev[i]);
    prev.copy_from_slice(pressed);
    edge
}

/// The stats pages' presses in a frame's menu edge bits (`App::menu_edges`' layout).
pub(in crate::windowed) fn stats_pad(edges: u32) -> ui::stats::Pad {
    ui::stats::Pad {
        up: edges & 1 != 0,
        down: edges & 2 != 0,
        left: edges & 4 != 0,
        right: edges & 8 != 0,
        cross: edges & 16 != 0,
        triangle: edges & 32 != 0,
        l1: edges & game::kbm::MENU_L1 != 0,
        r1: edges & game::kbm::MENU_R1 != 0,
        square: edges & game::kbm::MENU_SQUARE != 0,
    }
}

/// One pad's held menu buttons for the round-end pages, in `App::menu_edges`' bits: the D-pad
/// and the left stick (with `prev`'s latch), South, East and (once the `stats` pages are up)
/// North as Back, West (Square), the shoulders.
fn pad_menu_bits(pad: &gilrs::Gamepad<'_>, prev: u32, stats: bool) -> u32 {
    use gilrs::Button;
    let mut now = 0;
    for (b, bit) in [
        (Button::DPadUp, 1),
        (Button::DPadDown, 2),
        (Button::DPadLeft, 4),
        (Button::DPadRight, 8),
        (Button::South, 16),
        (Button::East, 32),
        (Button::LeftTrigger, game::kbm::MENU_L1),
        (Button::RightTrigger, game::kbm::MENU_R1),
        (Button::West, game::kbm::MENU_SQUARE),
    ] {
        if pad.is_pressed(b) {
            now |= bit;
        }
    }
    let stick = input::stick_arms(
        pad.value(gilrs::Axis::LeftStickX),
        pad.value(gilrs::Axis::LeftStickY),
        game::kbm::stick_latch(prev),
    );
    // North is Back only once the stats pages are up, as in `App::menu_edges`: it does not
    // skip the end movie.
    if stats && pad.is_pressed(Button::North) {
        now |= 32;
    }
    now |= stick;
    if stick & 4 != 0 {
        now |= game::kbm::MENU_ADJUST_LEFT;
    }
    if stick & 8 != 0 {
        now |= game::kbm::MENU_ADJUST_RIGHT;
    }
    now
}

/// Each further local player's own stats pages (issue #194 B9), by local index (index 0 is
/// `None`: the first player's are made as in single player): made under `with_local(i)` from
/// their own view's feed (`feeds[i - 1]`: its names, its round and its opening medals, so "me" is
/// their unit and "medals this match" count from their slot) and their own seat's career
/// (`profiles[seats[i]]`, this round already recorded in it). A player away, or with no seat
/// profile (the developer flag), gets the career of nobody: away gets `None`, a profile-less
/// seat an empty career. Tests only: the frame fills the pages in place
/// ([`fill_split_stats_pages`]).
#[cfg(test)]
pub(in crate::windowed) fn split_stats_pages(
    walk: &mut game::walk::WalkSim,
    feeds: &[game::event_feed::EventFeed],
    profiles: &[Option<game::profile::Profile>; 4],
    seats: &[usize],
) -> Vec<Option<ui::stats::StatsScreens>> {
    let mut out = vec![None; walk.local_count()];
    fill_split_stats_pages(walk, feeds, profiles, seats, &mut out);
    out
}

/// [`split_stats_pages`] for only the further players in `out` with none yet who are here now:
/// run every frame the pages are up, so a player whose pad comes back during the stats gets
/// their own pages then. Pages already made are kept (their cursors with them) while their
/// player stays; a player whose pad goes during the stats loses theirs (`SplitState::stats`:
/// `None` while away), so [`game::round_end::RoundEnd::step_views`] no longer waits for them,
/// and gets fresh ones when back.
pub(in crate::windowed) fn fill_split_stats_pages(
    walk: &mut game::walk::WalkSim,
    feeds: &[game::event_feed::EventFeed],
    profiles: &[Option<game::profile::Profile>; 4],
    seats: &[usize],
    out: &mut Vec<Option<ui::stats::StatsScreens>>,
) {
    let n = walk.local_count();
    out.resize(n.max(1), None);
    for i in 1..n {
        if walk.is_suspended(i) {
            out[i] = None;
            continue;
        }
        if out[i].is_some() {
            continue;
        }
        let Some(feed) = feeds.get(i - 1) else {
            continue;
        };
        let none = game::profile::Career::default();
        let career = seats
            .get(i)
            .and_then(|&seat| profiles.get(seat))
            .and_then(Option::as_ref)
            .filter(|_| seats.get(i).is_some_and(|&seat| seat != 0))
            .map_or(&none, |p| &p.career);
        out[i] = Some(walk.with_local(i, |w| super::stats_pages(w, feed, career)));
    }
}

/// Which of the window's per-frame pad branches takes a battle frame's pads (`events.rs`), in
/// their order: a LAN split round's Game Menu (the others play on), the Game Menu, the round's
/// end (results, banner, stats: a neutral pad, and split pads are still seated and dropped so a
/// player whose pad comes back during the stats gets their pages and one whose pad goes is not
/// waited for), a popup that holds the game (a mission popup, the campaign map), and play. The
/// round's end comes ahead of the popups: offline it holds the game as one
/// (`App::round_end_held`), and must not lose its seating to that arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::windowed) enum FramePads {
    LanMenu,
    Paused,
    RoundEnd,
    Popup,
    Play,
}

/// [`FramePads`] for this frame: `paused` (the Game Menu is up), `lan_split` (a LAN split
/// round), `popup_up` (a popup or an offline round's end holds the game) and `round_over` (the
/// round has its result).
pub(in crate::windowed) fn frame_pads(
    paused: bool,
    lan_split: bool,
    popup_up: bool,
    round_over: bool,
) -> FramePads {
    match () {
        _ if paused && lan_split => FramePads::LanMenu,
        _ if paused => FramePads::Paused,
        _ if round_over => FramePads::RoundEnd,
        _ if popup_up => FramePads::Popup,
        _ => FramePads::Play,
    }
}

/// Carry out the seats' pad losses and returns on the walk (`game::local_seats::Presence`): a
/// player whose pad is lost is suspended ([`game::walk::WalkSim::suspend_local_player`]: a bot
/// takes their soldier, a rocket or droid they flew is let go, their held views go) and one whose
/// pad is back rejoins behind their spawn screens with the round's career, assist and cheats
/// (`Sim::restore_slot`); of their seat profile only the award-bonus preference is applied again,
/// never its career (`SPLITSCREEN.md` 18.2 B3). Only that player is touched. Returns the players
/// who came back, whose stick state the caller resets. The window runs it on frames whose pads
/// it samples (and at the round's end), never under the Game Menu: there a lost pad only passes
/// the menu on ([`menu_driver`]) and the player is suspended once the game resumes.
pub(in crate::windowed) fn apply_presence<Id: Copy + PartialEq>(
    seats: &game::local_seats::Seats<Id>,
    presence: &mut game::local_seats::Presence,
    away: &mut [Option<game::walk::Suspended>],
    walk: &mut game::walk::WalkSim,
    profiles: &[Option<game::profile::Profile>; 4],
) -> Vec<usize> {
    let mut back = Vec::new();
    for c in presence.pending(seats) {
        match c {
            game::local_seats::Change::Suspend(i) => {
                away[i] = walk.suspend_local_player(i);
                presence.done(c);
                println!("split: player {i} is away");
            }
            game::local_seats::Change::Resume(i) => {
                let Some(gone) = away[i].clone() else {
                    presence.done(c);
                    continue;
                };
                match walk.resume_local_player(i, gone) {
                    Ok(_) => {
                        let slot = usize::from(walk.local_slot_of(i));
                        let seat = seats.seat_number(i);
                        if let Some(p) = profiles
                            .get(seat)
                            .and_then(Option::as_ref)
                            .filter(|_| i > 0 && seat != 0)
                        {
                            walk.sim.set_slot_career_award_bonuses(
                                slot,
                                p.settings.career_awards && !walk.netplay.awards_off,
                            );
                        }
                        walk.sim.open_spawn_screens(slot, false);
                        away[i] = None;
                        presence.done(c);
                        back.push(i);
                        println!("split: player {i} is back, on the spawn screens");
                    }
                    Err(e) => eprintln!("split: player {i} could not rejoin: {e:?}"),
                }
            }
        }
    }
    back
}

/// Who drives a Game Menu opened by `owner` of `players` seats: the owner while they still have
/// a device (`keyboard` is the keyboard's seat, `connected(i)` says seat `i`'s pad is there),
/// else the lowest seat with a connected pad, else the keyboard's seat (the keyboard counts as
/// the lowest seated player when nobody plays on it). A menu is never left with no one.
pub(in crate::windowed) fn menu_driver(
    owner: usize,
    players: usize,
    keyboard: Option<usize>,
    connected: impl Fn(usize) -> bool,
) -> usize {
    if keyboard == Some(owner) || connected(owner) {
        return owner;
    }
    (0..players)
        .find(|&i| connected(i))
        .unwrap_or(keyboard.unwrap_or(0))
}

/// An offline split round, nobody on the keyboard: the seat Esc opens the Game Menu as, whose
/// profile its rows edit. The lowest of `players` seats whose own pad is connected now
/// (`connected(i)`), else the lowest seat not away (`away[i]`, the walk's suspended players),
/// else the first seat: never a player who has gone while one is still here.
pub(in crate::windowed) fn free_keyboard_opener(
    players: usize,
    connected: impl Fn(usize) -> bool,
    away: &[bool],
) -> usize {
    (0..players)
        .find(|&i| connected(i))
        .or_else(|| (0..players).find(|&i| !away.get(i).copied().unwrap_or(false)))
        .unwrap_or(0)
}

/// A LAN split round, nobody on the keyboard: may Esc open the Game Menu? Only once no seated
/// player has their pad (`connected(i)`, for each of `players`): the round runs on under a LAN
/// menu, so one opened by the free keyboard would cover a player who is still playing and whom
/// no pad could close it for. With every pad gone the keyboard is the only way out.
pub(in crate::windowed) fn lan_free_keyboard_opens(
    players: usize,
    connected: impl Fn(usize) -> bool,
) -> bool {
    !(0..players).any(connected)
}

/// A LAN split round's Game Menu, `owner` its opener: does it close now? The round runs on
/// under it, so it is never passed to another player (whose soldier would stand still in the
/// live round while they steer someone else's menu, [`menu_driver`] is offline's rule): it
/// closes once its opener's pad is gone (their bot stands in, `lan_presence`). One the free
/// keyboard opened ([`lan_free_keyboard_opens`]) closes once any seated pad is back.
pub(in crate::windowed) fn lan_menu_closes(
    owner: usize,
    by_free_keyboard: bool,
    players: usize,
    keyboard: Option<usize>,
    connected: impl Fn(usize) -> bool,
) -> bool {
    if by_free_keyboard {
        return (0..players).any(connected);
    }
    keyboard != Some(owner) && !connected(owner)
}

/// The clear colour (linear) of a cell whose player is away.
const DARK: [f64; 4] = [0.003, 0.003, 0.005, 1.0];

/// One extra view's finished inputs for the renderer.
struct Built {
    camera: Camera,
    draws: Vec<DrawCmd>,
    skinned: Vec<SkinnedDrawCmd>,
}

/// Draw every view of this frame: view 0 from the lists the frame handler already built (its
/// camera, shake and spawn preview included), then each further player's view built here from
/// their own camera, `cameras[i]` (issue #194 B8: `game::render_blend::view_camera`, made once
/// per frame by the caller, which the view's name tags read too; `cameras[0]` is view 0's). One submit per view; the last carries `last_overlay` (the egui overlay, over
/// the whole window). The caller has put the tick's own pose back (`draw_out`): each extra view
/// blends the world in again for its build and out before it renders, so a menu action never
/// sees a blended pose.
///
/// A view in `away` is skipped whole (see above).
///
/// Each extra view stands its own spawn-screen soldiers (`ViewParts::previews`), fitted to its
/// UI rectangle as view 0's are (`split_layout::ui_view`).
///
/// Each extra view's camera carries its own shake (`cameras[i]`, from `Fx::shake_of`) and its rain
/// is sized to its cell (issue #194 B13).
#[allow(clippy::too_many_arguments)]
pub(in crate::windowed) fn render_views(
    p: &mut ViewParts<'_>,
    r: &mut Renderer,
    cells: &[Cell],
    view0: (&[DrawCmd], &[SkinnedDrawCmd]),
    cameras: &[Camera],
    looks: &[PadState],
    away: &[bool],
    clear: [f64; 4],
    mut last_overlay: Option<render::OverlayFn<'_>>,
    mut view_hud: Option<ViewHud<'_>>,
) -> Result<(), render::RenderError> {
    let n = cells.len();
    for (i, cell) in cells.iter().enumerate() {
        let slot = render::ViewSlot {
            rect: *cell,
            first: i == 0,
            last: i + 1 == n,
        };
        let mut last = if slot.last { last_overlay.take() } else { None };
        let cell = *cell;
        // The view's own UI paints into its cell (`game::split_ui`: nothing for a player who is
        // away; the Game Menu over every cell rides the last view); the last view then runs the
        // whole-window overlay (egui) over everything.
        let mut compose = |dev: &wgpu::Device,
                           q: &wgpu::Queue,
                           enc: &mut wgpu::CommandEncoder,
                           view: &wgpu::TextureView,
                           size: [u32; 2]| {
            if let Some(h) = view_hud.as_deref_mut() {
                h(i, cell, dev, q, enc, view);
            }
            if let Some(o) = last.as_deref_mut() {
                o(dev, q, enc, view, size);
            }
        };
        let overlay: Option<render::OverlayFn<'_>> = Some(&mut compose);
        let cell = &cell;
        if away.get(i).copied().unwrap_or(false) {
            // A player whose controller is gone: nothing of theirs is read and their cell is
            // drawn empty on a dark clear; the overlay covers it with the notice. The view is
            // still presented in its slot, so the first and last views of the present hold.
            r.render_view(slot, &Camera::default(), &[], &[], DARK, overlay)?;
            continue;
        }
        if i == 0 {
            let camera = cameras.first().copied().unwrap_or_default();
            r.render_view(slot, &camera, view0.0, view0.1, clear, overlay)?;
            continue;
        }
        let look = looks.get(i).copied().unwrap_or_default();
        let camera = cameras.get(i).copied().unwrap_or_default();
        let built = build_view(p, r, i, *cell, &look, camera);
        r.render_view(
            slot,
            &built.camera,
            &built.draws,
            &built.skinned,
            clear,
            overlay,
        )?;
    }
    Ok(())
}

/// Colours the CTF flags (`viewer::pickups::flag_tint`) for the player `w` views: the first
/// player's, or local player `k`'s inside [`game::walk::WalkSim::with_local`]. Every view sets it
/// before its flags draw, since the one `VehicleVisuals` serves every cell.
pub(in crate::windowed) fn set_ctf_viewer(
    vis: &mut game::viewer::VehicleVisuals,
    w: &game::walk::WalkSim,
) {
    vis.set_ctf_viewer(w.sim.unit(w.player).map(|u| u.team));
}

/// Player `i`'s draw lists and renderer state from `camera` (their `view_camera`), with the world
/// blended in as that camera was made.
fn build_view(
    p: &mut ViewParts<'_>,
    r: &mut Renderer,
    i: usize,
    cell: Cell,
    look: &PadState,
    camera: Camera,
) -> Built {
    let aspect = cell[2] as f32 / cell[3].max(1) as f32;
    if p.previews.len() < i {
        p.previews.resize_with(i, Default::default);
    }
    let page = p.pages.get(i).and_then(Option::as_ref);
    p.walk.blend_in();
    let cube = p.cube;
    // Lent out for the build (the closure holds `p`), given back after it.
    let mut lan_view = p.lan_view.take();
    let built = p.walk.with_local(i, |w| {
        match lan_view.as_deref_mut() {
            Some(v) => v.draw_in_local(w),
            None => game::render_blend::blend_in_looking(w, look),
        }
        let mut extra: Vec<DrawCmd> = Vec::new();
        let mut skinned: Vec<SkinnedDrawCmd> = Vec::new();
        walk_draws(cube, w, &mut extra, p.soldier_vis.as_deref());
        let see = w.camera_see_through();
        p.vehicle_vis.set_tick_share(game::render_blend::share(w));
        // The flags are coloured for this view's own side (two-flag CTF), not the first view's.
        set_ctf_viewer(p.vehicle_vis, w);
        extra.extend(p.vehicle_vis.draws(r, &w.sim, &see.vehicles));
        extra.extend(p.vehicle_vis.campaign_flags(w));
        extra.extend(p.vehicle_vis.chunk_draws(p.fx.lit_chunk_poses()));
        p.deka_vis.eye = Some(camera.position);
        extra.extend(p.deka_vis.draws(r, &w.sim));
        if let Some(vis) = p.soldier_vis.as_deref_mut() {
            vis.prepare_view(w, &camera, aspect);
            skinned = vis.draws(w);
            extra.extend(vis.weapon_draws(w));
            extra.extend(vis.cloth_draws(r, w));
        }
        if let Err(e) = p.gpu.animate_water_live(r, camera.position) {
            eprintln!("water: {e:#}");
        }
        game::forest::submit_actors(r, &w.sim);
        p.gpu.see_through(&see.props);
        p.gpu.pose_ctf(&w.sim, w.player);
        let items = p.vehicle_vis.campaign_lights(w);
        game::point_light::submit(r, p.gpu, w, p.bolts, camera.position, p.holos, &items);
        let flashes = p
            .soldier_vis
            .as_deref()
            .map(|v| {
                let mut points = v.fire_points(w);
                p.deka_vis.fire_points(&w.sim, &mut points);
                v.flash_draws_at(w, &points)
            })
            .unwrap_or_default();
        let mut particles = flashes;
        if let Some(vis) = p.soldier_vis.as_deref() {
            particles.extend(vis.saber_draws(w));
        }
        particles.extend(p.deka_vis.bubbles(&w.sim, &camera, r.lighting.sun_dir));
        particles.extend(p.holos.draws(&p.gpu.holo, w, &camera));
        particles.extend(p.vehicle_vis.campaign_glows(w, &camera));
        if let Some(weather) = p.weather.as_deref_mut() {
            // Sized to the cell (B13), not the window.
            particles.extend(weather.frame_draws_sized(
                r,
                &camera,
                tracked_velocity(w),
                [cell[2], cell[3]],
            ));
        }
        p.fx.submit_walk(r, &camera, w, particles);
        let mut draws = p.gpu.view_draws_with_hidden_instances(
            &w.sim,
            w.player,
            w.campaign.as_ref().map(|c| c.hidden_instances()),
        );
        draws.append(&mut extra);
        if let Some(vis) = p.soldier_vis.as_deref() {
            let (arms, gun) = vis.first_person_draws(w, &camera);
            skinned.extend(arms);
            draws.extend(gun);
        }
        // The spawn screens' soldiers, in this view's UI rectangle (as view 0's).
        if let Some(vis) = p.soldier_vis.as_deref() {
            let (fov_y, ui_aspect) = game::split_layout::ui_view(camera.fov_y, cell);
            let preview = &mut p.previews[i - 1];
            preview.set_view_aspect(Some(ui_aspect));
            let deka = &mut *p.deka_vis;
            let (body, gun) = preview.draws_live(
                r,
                vis,
                page,
                &Camera { fov_y, ..camera },
                p.preview_dt,
                |r, team, class, model, dt| {
                    let c = w
                        .sim
                        .classes
                        .get(usize::from(team))?
                        .get(usize::from(class))?
                        .droideka
                        .as_deref()?;
                    Some(deka.preview_draws(r, team, class, c, model, dt))
                },
            );
            skinned.extend(body);
            draws.extend(gun);
        }
        if let Some(quad) = p.fx_quad {
            draws.extend(p.fx.draws(quad, p.fx_sprite, &camera));
            let slot = w.sim.units[w.player.index as usize]
                .spec
                .player_slot
                .unwrap_or(0);
            draws.extend(game::squad_vis::squad_draws(
                &w.sim,
                slot,
                quad,
                p.squad_icons.0,
                p.squad_icons.1,
                &camera,
            ));
        }
        // The look preview of this player's own blend goes back before the world's (in a LAN
        // round with the predicted pose, `LocalView::draw_out`).
        match lan_view.as_deref_mut() {
            Some(v) => v.draw_out(w),
            None => w.blend_out(),
        }
        Built {
            camera,
            draws,
            skinned,
        }
    });
    p.lan_view = lan_view;
    p.walk.blend_out();
    built
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #194 B20 review: a LAN round maps each walk local to its own seat in whatever
    /// order the session adopted them, and a seat the session left out keeps its pad aside: it
    /// never takes a waiting player's seat (the split's offer loop skips `left_out`).
    #[test]
    fn a_lan_round_seats_each_local_by_seat_and_sets_left_out_pads_aside() {
        let mut party = input::owners::PadOwners::<u8>::default();
        party.join_pad(10); // seat 0
        party.join_pad(11); // seat 1: refused by the host
        party.join_pad(12); // seat 2
        party.join_pad(13); // seat 3
                            // The walk adopted seat 3's slot before seat 2's.
        let lan = [0, 3, 2];
        let mut seats = game::local_seats::Seats::from_seats(&party, &lan);
        assert_eq!(
            (0..3).map(|k| seats.seat_number(k)).collect::<Vec<_>>(),
            lan
        );
        assert_eq!(
            (0..3).map(|k| seats.pad_of(k)).collect::<Vec<_>>(),
            [Some(10), Some(13), Some(12)],
            "each view keeps its own seat's pad"
        );
        let left_out = left_out_pads(&party, &lan);
        assert_eq!(left_out, vec![11]);
        // Seat 3's pad drops: the refused pad's press is not offered (the window skips it), and
        // the same pad coming back still finds its own seat.
        assert_eq!(seats.gone(13), Some(1));
        assert!(seats.is_waiting(1));
        assert!(left_out.contains(&11), "the offer loop skips this pad");
        assert_eq!(seats.offer_with(13, |_| true), Some(1));
        // The round's end hands the left-out seat back to the Players screen.
        let mut back = seats.party_owners();
        assert!(!back.is_seated(1));
        back.take_seat_from(&party, 1);
        assert_eq!(back.pad_of(1), Some(11));
        assert_eq!(back.pad_of(3), Some(13));
    }
    use core::sim::buttons;
    use game::viewpoint::Viewpoint;

    /// A hosted LAN session on a free port, for the window's LAN branches (`App.netplay`).
    fn lan_host() -> super::super::lan::NetplaySession {
        let mut args = super::super::lan::LanArgs::parse(&[], true).expect("defaults parse");
        args.port = 0;
        let profile =
            game::netplay::PlayerProfile::from_profile(&game::profile::Profile::default());
        super::super::lan::NetplaySession::host(args, profile, 4, true).expect("hosting")
    }

    fn state(n: usize, keyboard: bool) -> SplitState {
        SplitState {
            seats: game::local_seats::Seats::new(n, keyboard),
            inputs: (0..n).map(|_| PlayerInput::new(Viewpoint::Third)).collect(),
            pads: Vec::new(),
            looks: Vec::new(),
            presence: game::local_seats::Presence::new(n),
            away: vec![None; n],
            score_holds: vec![Default::default(); n],
            stats: vec![None; n],
            menu_prev: vec![0; n],
            left_out: Vec::new(),
            lan: false,
            picks: Vec::new(),
        }
    }

    /// Each player's pad lands in its own slot, the first player's is the frame's own, and each
    /// player's viewpoint is stamped on their pad alone.
    #[test]
    fn each_players_pad_goes_to_its_own_slot_with_its_own_viewpoint() {
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let mut s = state(2, false);
        s.inputs[1].viewpoint = Viewpoint::First;
        let fire = PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        };
        let jump = PadState {
            buttons: buttons::JUMP,
            ..Default::default()
        };
        s.pads = vec![PadState::default(), jump];
        let pads = s.finish_pads(fire, &walk);
        assert_eq!(pads.len(), 2);
        assert!(pads[0].pressed(buttons::FIRE) && !pads[0].pressed(buttons::JUMP));
        assert!(pads[1].pressed(buttons::JUMP) && !pads[1].pressed(buttons::FIRE));
        assert!(
            pads[1].pressed(buttons::FIRST_PERSON),
            "player 1 is in first person"
        );
        assert!(!pads[0].pressed(buttons::FIRST_PERSON), "player 0 is not");
    }

    /// Seat profiles for tests: `names[i]` on seat `i` (seat 0 is the host's, never read).
    fn seat_profiles(names: [Option<&str>; 4]) -> [Option<game::profile::Profile>; 4] {
        names.map(|n| {
            n.map(|n| {
                let mut p = game::profile::Profile::default();
                p.name = n.to_owned();
                p
            })
        })
    }

    /// Issue #194: a returning pad that could be either of two waiting players (identical
    /// models) is seated nowhere until its holder picks: the A that opened the question does not
    /// answer it, a fresh d-pad press moves the pick, a fresh A takes that seat and nobody else's.
    #[test]
    fn a_pad_pick_takes_only_the_seat_its_holder_picks() {
        let mut seats = game::local_seats::Seats::<u8>::new(2, false);
        seats.offer(1);
        seats.offer(2);
        seats.gone(1);
        seats.gone(2);
        let Some(game::local_seats::Offered::Choose(c)) = seats.offer_alike(1, |_| true, |_| true)
        else {
            panic!("two identical pads wait: ask");
        };
        let mut pick = PadPick::new(c, PICK_TAKE);
        assert!(pick.open());
        assert_eq!(
            pick.step(&mut seats, 1, PICK_TAKE),
            None,
            "the opening A still held"
        );
        assert_eq!(pick.step(&mut seats, 1, 0), None);
        assert_eq!(pick.step(&mut seats, 1, PICK_RIGHT), None);
        assert_eq!(pick.choice.seat(), Some(1));
        assert_eq!(
            pick.step(&mut seats, 1, PICK_RIGHT),
            None,
            "held, not pressed again"
        );
        assert_eq!(pick.choice.seat(), Some(1));
        assert!(seats.is_waiting(0) && seats.is_waiting(1));
        assert_eq!(pick.step(&mut seats, 1, PICK_TAKE), Some(1));
        assert_eq!((seats.player_of(1), seats.is_waiting(0)), (Some(1), true));
    }

    /// Finding pick-ignores-second-identical-pad-silently: three players' identical pads drop;
    /// the first comes back and its question opens. The second comes back while it is open and
    /// joins it: its own d-pad moves the shared pick and its own A takes the seat shown, for
    /// itself, never for the first pad. The first pad then answers what is left, and the last
    /// pad back goes to the last seat by itself. A pad of another model opens its own question.
    #[test]
    fn a_second_identical_pad_answers_the_open_pick_too() {
        let mut seats = game::local_seats::Seats::<u8>::new(4, false);
        for pad in [1, 2, 3, 9] {
            seats.offer(pad);
        }
        for pad in [1, 2, 3] {
            seats.gone(pad);
        }
        let alike = |id: u8| id != 9;
        let Some(game::local_seats::Offered::Choose(c)) = seats.offer_alike(1, |_| true, alike)
        else {
            panic!("three identical pads wait: ask");
        };
        assert_eq!(c.seats, [0, 1, 2]);
        let mut pick = PadPick::new(c, PICK_TAKE);
        // The second identical pad (new id 4) presses while the question is open: it joins.
        let Some(game::local_seats::Offered::Choose(c2)) = seats.offer_alike(4, |_| true, alike)
        else {
            panic!("still ambiguous");
        };
        assert!(pick.join(&c2, PICK_TAKE));
        assert!(pick.has(4) && pick.has(1));
        assert_eq!(pick.pads(), [1, 4]);
        // Its held A does not count; its fresh d-pad press moves the pick, its A takes it.
        assert_eq!(pick.step(&mut seats, 4, PICK_TAKE), None);
        assert_eq!(pick.step(&mut seats, 4, 0), None);
        assert_eq!(pick.step(&mut seats, 4, PICK_RIGHT), None);
        assert_eq!(pick.choice.seat(), Some(1));
        assert_eq!(pick.step(&mut seats, 4, PICK_TAKE), Some(1));
        assert_eq!(
            seats.player_of(4),
            Some(1),
            "the pad that pressed is seated"
        );
        assert_eq!(seats.player_of(1), None, "the opener is not");
        assert!(!pick.has(4), "a seated pad answers no more");
        assert!(pick.open(), "two seats still wait and pad 1 is left");
        // The opener answers what is left.
        assert_eq!(pick.step(&mut seats, 1, 0), None);
        assert_eq!(pick.choice.seats, [0, 2]);
        assert_eq!(pick.choice.seat(), Some(0));
        assert_eq!(pick.step(&mut seats, 1, PICK_TAKE), Some(0));
        assert_eq!(seats.player_of(1), Some(0));
        assert!(!pick.open());
        // The last pad back has one seat left: it takes it by itself.
        assert_eq!(
            seats.offer_alike(5, |_| false, alike),
            Some(game::local_seats::Offered::Seat(2))
        );
        // A pad of another model's question shares no waiting player: it is not joined.
        let other = game::local_seats::SeatChoice {
            pad: 7u8,
            seats: vec![3],
            at: 0,
        };
        let mut open = PadPick::new(
            game::local_seats::SeatChoice {
                pad: 1u8,
                seats: vec![0, 1],
                at: 0,
            },
            0,
        );
        assert!(!open.join(&other, 0));
        assert!(!open.has(7));
        // A pad that went: the next one answers, and the lines name its model.
        assert!(open.join(
            &game::local_seats::SeatChoice {
                pad: 6,
                seats: vec![0, 1],
                at: 0,
            },
            0
        ));
        assert!(open.retain(|id| id != 1));
        assert_eq!(open.choice.pad, 6);
        assert!(!open.retain(|_| false));
    }

    /// Finding split-controls-editor-and-pick-line-host-icons: P2's Controls editor and the
    /// lost-controller pick line in P2's cell name buttons by P2's own icon choice, not the
    /// host's; P1's follow the host's.
    #[test]
    fn the_editor_and_the_pick_line_use_the_players_own_icons() {
        use game::input_icons::{Device, IconChoice};
        use ui::input_icons::IconSet;
        let mut host = game::profile::Profile::default();
        host.settings.input_icons = IconChoice::Auto;
        let mut profiles = seat_profiles([None, Some("Leia"), None, None]);
        let leia = &mut profiles[1].as_mut().unwrap().settings.input_icons;
        *leia = IconChoice::Ds4;
        assert_ne!(*leia, IconChoice::Auto, "the test needs a forced choice");
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(20);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        let xbox = |_: usize| Some(input::PadFamily::XboxOne);
        let fallback = Device::Pad(input::PadFamily::XboxOne);
        let p2 = seat_icon_set(&host, &profiles, &seats, 1, xbox, fallback);
        let want =
            game::input_icons::set_for(IconChoice::Ds4, Device::Pad(input::PadFamily::XboxOne));
        assert_eq!(p2, Some(want), "P2's editor: P2's own choice");
        assert_eq!(
            seat_icon_set(&host, &profiles, &seats, 0, xbox, fallback),
            Some(IconSet::XboxOne),
            "P1's editor: the host's Auto on P1's Xbox pad"
        );
        assert_eq!(
            seat_icon_set(&host, &profiles, &seats, 3, xbox, fallback),
            None,
            "nobody on seat 4"
        );
        // Both pads drop; the pick line in each waiting cell follows that player's choice.
        let mut seats = seats;
        seats.gone(10);
        seats.gone(20);
        let pick = PadPick::new(
            game::local_seats::SeatChoice {
                pad: 10u8,
                seats: vec![0, 1],
                at: 1,
            },
            0,
        );
        let line = |i: usize, name: &str| {
            pick_line_for(
                &pick,
                &seats,
                &host,
                &profiles,
                Some(input::PadFamily::XboxOne),
                fallback,
                i,
                name,
            )
        };
        assert_eq!(
            line(1, "Leia"),
            Some(game::split_ui::pick_line(want, true, "Leia"))
        );
        assert_eq!(
            line(0, "Han"),
            Some(game::split_ui::pick_line(IconSet::XboxOne, false, "Han"))
        );
        assert_ne!(
            line(1, "Leia"),
            Some(game::split_ui::pick_line(IconSet::XboxOne, true, "Leia")),
            "P2's line names PlayStation buttons, not the host's Xbox ones"
        );
        assert_eq!(line(2, "Cy"), None, "not a waiting player of the pick");
    }

    #[test]
    fn the_notice_names_the_player() {
        let none = seat_profiles([None; 4]);
        assert_eq!(seat_name(1, "Ann", &none), "Player 2", "no seat profile");
        assert_eq!(seat_name(0, "Ann", &none), "Ann");
        assert_eq!(seat_name(0, "  ", &none), "Player 1");
        let han = seat_profiles([None, Some("Han"), Some("  "), None]);
        assert_eq!(seat_name(1, "Ann", &han), "Han", "P2 by their own profile");
        assert_eq!(seat_name(2, "Ann", &han), "Player 3", "a refused name");
        let n = lost_notice("Han".into());
        assert_eq!(
            [n.notice.as_str(), n.name.as_str(), n.action.as_str()],
            [
                "Controller disconnected",
                "Han",
                "Reconnect and press any button"
            ]
        );
    }

    /// P1 pad, P2 keyboard, P3 pad, P4 pad; the keyboard leaves, so the battle plays seats P1,
    /// P3 and P4 as local slots 0, 1 and 2. Each is named by their own seat's profile, and a
    /// seat without one by the number it had on the Players screen.
    #[test]
    fn a_battle_of_seats_one_three_four_names_each_by_their_seat() {
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(1);
        o.join_keyboard();
        o.join_pad(3);
        o.join_pad(4);
        o.leave(1);
        let seats = game::local_seats::Seats::from_owners(o, 3);
        let profiles = seat_profiles([None, None, Some("Leia"), None]);
        let names = battle_names(&seats, &[0, 1, 2], "Ann", &profiles);
        assert_eq!(
            names,
            vec![
                (0, "Ann".to_string()),
                (1, "Leia".to_string()),
                (2, "Player 4".to_string())
            ]
        );
        // P1 left instead: the first local slot is P2 and does not take P1's profile name.
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_keyboard();
        o.join_pad(2);
        o.join_pad(3);
        o.leave(0);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        let none = seat_profiles([None; 4]);
        assert_eq!(
            battle_names(&seats, &[0, 1], "Ann", &none),
            vec![(0, "Player 2".to_string()), (1, "Player 3".to_string())]
        );
        // Even with a profile on seat P2: local player 0 plays on the host's profile, so it is
        // not named by P2's.
        let p2 = seat_profiles([None, Some("Han"), Some("Leia"), None]);
        assert_eq!(
            battle_names(&seats, &[0, 1], "Ann", &p2),
            vec![(0, "Player 2".to_string()), (1, "Leia".to_string())]
        );
        // The developer flag's seats are numbered by slot.
        let seats = game::local_seats::Seats::<u8>::new(2, true);
        assert_eq!(
            battle_names(&seats, &[0, 1], "Ann", &none),
            vec![(0, "Ann".to_string()), (1, "Player 2".to_string())]
        );
    }

    /// Issue #194 B4, in the game's per-tick order: the hook notes P1's medals first, then P2's
    /// feed scores P2's points (War Hero), then the second pass (`note_split_medals`) runs.
    /// P1's view gets "Han earned ..." on that tick, so the next frame's rebuilt seen maps
    /// (`TickState::new`) taking in the new counts loses nothing; and nothing repeats after.
    #[test]
    fn a_later_players_medal_reaches_the_first_view_on_the_same_tick() {
        use game::event_feed::{EventFeed, Texts};
        let mut w = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let opening = [0; 9];
        let p2 = w.add_local_player(1, Some(1)).expect("slot 1 is free");
        w.sim.install_medal_career(w.player, opening);
        w.sim.install_slot_medal_career(1, p2, opening);
        let mut names = game::player_names::PlayerNames::default();
        names.set([(0, "Luke"), (1, "Han")]);
        let new_feed = || {
            let mut f = EventFeed::new(Texts::builtin(), ["all", "imp"], &[]);
            f.set_player_names(names.iter());
            f
        };
        let mut feed = new_feed();
        let mut feeds = vec![new_feed()];
        let mut seen0 = EventFeed::medals_seen(&w, opening);
        let mut views = vec![seen0.clone()];
        // The tick: the hook's note for P1, then P2 scores, then the second pass.
        feed.note_medals(&w.sim, w.player, w.local_slot, &mut seen0);
        w.sim.score_medal_points(w.local_unit(1), 36);
        note_split_medals(&w, &mut feed, &mut seen0, &mut feeds, &mut views);
        let said =
            |f: &mut EventFeed| -> Vec<String> { f.take().into_iter().map(|m| m.text).collect() };
        let p1 = said(&mut feed);
        assert!(
            p1.iter().any(|t| t.starts_with("Han earned ")),
            "P1 sees P2's medal on the tick it was earned: {p1:?}"
        );
        let p2_lines = said(&mut feeds[0]);
        assert!(
            !p2_lines.is_empty() && p2_lines.iter().all(|t| !t.contains("earned")),
            "P2 sees their own medal line: {p2_lines:?}"
        );
        // Next frame: the maps are rebuilt from the counts now; a further pass says nothing.
        let mut seen0 = EventFeed::medals_seen(&w, opening);
        let mut views = vec![seen0.clone()];
        note_split_medals(&w, &mut feed, &mut seen0, &mut feeds, &mut views);
        assert!(
            feed.take().is_empty() && feeds[0].take().is_empty(),
            "once only"
        );
    }

    /// Issue #194 B4: the names are keyed by each local player's sim slot
    /// (`WalkSim::local_slot_of`), never by the local index: a battle whose second local player
    /// sits on slot 3 names slot 3.
    #[test]
    fn battle_names_key_each_player_by_their_slot() {
        let seats = game::local_seats::Seats::<u8>::new(2, true);
        let profiles = seat_profiles([None, Some("Han"), None, None]);
        assert_eq!(
            battle_names(&seats, &[2, 3], "Ann", &profiles),
            vec![(2, "Ann".to_string()), (3, "Han".to_string())]
        );
        // From a real walk: the slots come from `local_slot_of`.
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(2, Some(1)).expect("slot 2 is free");
        let slots: Vec<u8> = (0..walk.local_count())
            .map(|k| walk.local_slot_of(k))
            .collect();
        assert_eq!(slots, [0, 2]);
        assert_eq!(
            battle_names(&seats, &slots, "Ann", &profiles),
            vec![(0, "Ann".to_string()), (2, "Han".to_string())]
        );
    }

    /// The keyboard on the second seat: its mouse look goes back to the keyboard, not the
    /// first player's look-less gamepad pad, so one push of the mouse turns once.
    #[test]
    fn the_keyboard_on_a_later_seat_gives_back_its_own_mouse_look() {
        let mut kbm = game::kbm::Kbm::default();
        kbm.click_in();
        kbm.sync_grab_with(true, |_| Ok::<_, ()>(())).unwrap();
        kbm.motion(40.0, 0.0);
        let gamepad = PadState::default();
        let keys = kbm.pad(PadState::default());
        assert_ne!(keys.mx, 0, "the mouse turns the keyboard player");
        let pads = [gamepad, keys];
        kbm.consumed(&keyboard_pad(Some(1), &pads, gamepad), 1);
        assert_eq!(kbm.pad(PadState::default()).mx, 0, "the turn is spent");
        assert_eq!(keyboard_pad(Some(0), &pads, keys), keys);
        assert_eq!(keyboard_pad(None, &pads, keys), PadState::default());
    }

    /// A player whose pad has not been sampled yet drives nothing.
    #[test]
    fn start_on_any_seated_pad_opens_the_menu_as_that_player() {
        let mut prev = Vec::new();
        // Both pads busy (sticks moving: either may be the active pad); P2 presses Start.
        assert_eq!(seat_start_edge(&mut prev, &[false, false]), None);
        assert_eq!(seat_start_edge(&mut prev, &[false, true]), Some(1));
        // Held: once only.
        assert_eq!(seat_start_edge(&mut prev, &[false, true]), None);
        // P1 next, while P2 still holds.
        assert_eq!(seat_start_edge(&mut prev, &[true, true]), Some(0));
        // A seat joining mid-battle starts released.
        assert_eq!(seat_start_edge(&mut prev, &[true, true, true]), Some(2));
    }

    #[test]
    fn a_menu_whose_owner_lost_their_pad_passes_to_someone_who_can_drive_it() {
        // P2 (pad) opened it and keeps it while their pad is there.
        assert_eq!(menu_driver(1, 3, None, |_| true), 1);
        // P2's pad is gone: the lowest seat with a pad.
        assert_eq!(menu_driver(1, 3, None, |i| i != 1), 0);
        assert_eq!(menu_driver(1, 3, None, |i| i == 2), 2);
        // Nobody's pad: the keyboard's seat (P1 with nobody on it), which Esc drives.
        assert_eq!(menu_driver(1, 3, None, |_| false), 0);
        assert_eq!(menu_driver(1, 3, Some(2), |_| false), 2);
        // A keyboard owner never loses it.
        assert_eq!(menu_driver(2, 3, Some(2), |_| false), 2);
    }

    #[test]
    fn a_player_with_no_pad_yet_gets_a_neutral_pad() {
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let s = state(2, false);
        let pads = s.finish_pads(PadState::default(), &walk);
        assert_eq!(pads[1], PadState::default());
    }

    /// Issue #194 B3: a freshly seated split battle gives P2 their own seat profile (career,
    /// assist, viewpoint) and nothing of it to P1; the round lands in P2's seat profile, never
    /// in the host's `App.profile`.
    #[test]
    fn a_seat_profile_reaches_its_player_and_takes_its_round() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let mut han = game::profile::Profile::default().into_seat();
        han.name = "Han".into();
        han.career.medals = [2; 9];
        han.settings.viewpoint = Viewpoint::First;
        app.split_profiles[1] = Some(han);
        app.split = Some(state(2, false));
        app.install_seat_profiles(&mut walk);
        assert_eq!(walk.sim.slot_medal_counts(1), Some([2; 9]));
        assert!(walk.netplay.profiles.contains_key(&1));
        assert!(
            !walk.netplay.profiles.contains_key(&0),
            "P1 keeps the host path"
        );
        assert!(walk.sim.slot_assist.get(1).copied().flatten().is_some());
        let split = app.split.as_ref().unwrap();
        assert_eq!(split.inputs[1].viewpoint, Viewpoint::First);
        assert_eq!(split.inputs[0].viewpoint, Viewpoint::Third);
        let seats = seat_numbers(app.split.as_ref(), walk.local_count());
        let i = walk.local_unit(1).index as usize;
        walk.sim.units[i].kills = 4;
        super::super::career::record_split_rounds(&mut app.split_profiles, &mut walk, &[], &seats);
        let han = app.split_profiles[1].as_ref().unwrap();
        assert_eq!((han.career.rounds, han.career.kills), (1, 4));
        assert_eq!(app.profile.career.rounds, 0);
    }

    /// Issue #194 B22: while the first player is away the stand-in bot fires from their unit
    /// index; the feed still scores the round but the bot's shots and hits never reach P1's
    /// profile: P1 keeps the 3 shots they fired. P1's slot has no medal career while away (the
    /// bot's soldier has no player slot), so no class kill or medal of the bot's is credited.
    #[test]
    fn an_away_first_player_keeps_none_of_the_bots_shots() {
        use core::sim::Event;
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        walk.sim.install_medal_career(walk.player, [0; 9]);
        let mut feed = game::event_feed::EventFeed::default();
        let shoot = |walk: &mut game::walk::WalkSim, n: usize| {
            walk.sim.tick += 1;
            walk.sim.events.clear();
            for _ in 0..n {
                walk.sim.events.push(Event::Shot {
                    unit: walk.player,
                    weapon_class: 0,
                    origin: glam::Vec3::ZERO,
                    dir: glam::Vec3::Z,
                });
            }
        };
        shoot(&mut walk, 3);
        feed.tick(&walk.sim, walk.player);
        assert_eq!(feed.battle_stats().shots, 3);
        walk.suspend_local_player(0).expect("P1 drops");
        assert_eq!(walk.sim.slot_medal_counts(0), None, "no career to credit");
        for _ in 0..2 {
            shoot(&mut walk, 5);
            feed.tick_away(&walk.sim, walk.player);
        }
        let stats = feed.battle_stats();
        assert_eq!((stats.shots, stats.hits, stats.captures), (3, 0, 0));
        let mut p1 = game::profile::Profile::default();
        walk.record_battle(&mut p1, stats);
        assert_eq!(p1.career.shots, 3);
        // The same frames ticked normally would have counted the bot's.
        let mut plain = game::event_feed::EventFeed::default();
        shoot(&mut walk, 5);
        plain.tick(&walk.sim, walk.player);
        assert_eq!(plain.battle_stats().shots, 5);
    }

    /// Issue #194 B22: P2 in a starfighter flies with their own seat profile's flyer layout (two
    /// buttons swapped), not P1's; P1's own table (the host profile's) is untouched. A layout
    /// edit in battle rebuilds it.
    #[test]
    fn split_seat_vehicle_layout() {
        use ui::menu::ControlContext::Starfighter;
        let host = game::profile::Profile::default();
        let defaults: Vec<gilrs::Button> =
            game::control_layout::gamepad_defaults(Starfighter, &host.settings.controller_bindings)
                .into_iter()
                .map(|b| b.expect("a default for every flyer action"))
                .collect();
        let mut swapped = defaults.clone();
        swapped.swap(0, 1);
        let mut leia = game::profile::Profile::default();
        leia.name = "Leia".into();
        leia.settings.gamepad_layouts.starfighter = swapped.clone();
        let mut profiles: [Option<game::profile::Profile>; 4] = Default::default();
        profiles[1] = Some(leia);
        let s = state(2, false);
        let mut input = PlayerInput::new(Viewpoint::Third);
        input.follow_context(
            Some(Starfighter),
            seat_settings(&host, &profiles, &s.seats, 1),
        );
        let own = game::control_layout::gamepad(Starfighter, &swapped).expect("a valid layout");
        assert_eq!(input.pad_map.as_ref(), Some(&own), "P2's own flyer layout");
        // P1 plays on the host's table, which knows nothing of P2's.
        let p1 = seat_settings(&host, &profiles, &s.seats, 0);
        assert!(p1.gamepad_layouts.starfighter.is_empty());
        assert_ne!(
            game::control_layout::gamepad(Starfighter, &defaults).as_ref(),
            Some(&own)
        );
        // An edit to P2's layout in battle (the Controls editor) is picked up on the next sample.
        profiles[1]
            .as_mut()
            .unwrap()
            .settings
            .gamepad_layouts
            .starfighter = defaults.clone();
        input.relayout();
        input.follow_context(
            Some(Starfighter),
            seat_settings(&host, &profiles, &s.seats, 1),
        );
        assert_eq!(
            input.pad_map,
            game::control_layout::gamepad(Starfighter, &defaults)
        );
    }

    /// Issue #194 B22: P1 on team 0 and P2 on team 1, team 0 wins: P1's cell shows VICTORY and
    /// P2's DEFEAT (the walk's shared round end carries P1's result).
    #[test]
    fn split_banner_per_team() {
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let p1 = walk.player.index as usize;
        walk.sim.units[p1].team = 0;
        assert_eq!(walk.with_local(1, |w| w.viewer_team()), 1);
        walk.sim.outcome = Some(0);
        walk.follow_round_end(0.0, Default::default(), None, false);
        let mut feed = game::event_feed::EventFeed::default();
        let m0 = super::super::hud_model(&walk, &mut feed, 16.0 / 9.0);
        assert_eq!(m0.round_end.as_ref().map(|r| r.result), Some(0));
        let m1 = walk.with_local(1, |w| {
            let mut m = super::super::hud_model(w, &mut feed, 16.0 / 9.0);
            own_banner(w, m.round_end.as_mut().expect("the round ended"));
            m
        });
        assert_eq!(m1.round_end.as_ref().map(|r| r.result), Some(1));
        // An away first player's side is the one they left on, whoever holds the unit now.
        walk.suspend_local_player(0).expect("P1 drops");
        walk.sim.units[p1].team = 1;
        assert_eq!(walk.viewer_team(), 0);
    }

    /// Issue #194 B22: the Game Menu says who paused by name ("Paused by Leia"), the name the
    /// feed and the lost notice give; the keyboard with nobody on it says so. Local player 0 on a
    /// seat other than P1 plays on the host's profile and is "Player N" there (`local_name`).
    #[test]
    fn paused_by_names_the_opener() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Han".into();
        app.split_profiles = seat_profiles([None, Some("Leia"), None, None]);
        assert_eq!(
            app.paused_by_line(Some(1), Some(1)),
            None,
            "not a split battle"
        );
        app.split = Some(state(2, false));
        assert_eq!(
            app.paused_by_line(Some(1), Some(1)).as_deref(),
            Some("Paused by Leia")
        );
        assert_eq!(
            app.paused_by_line(Some(0), Some(0)).as_deref(),
            Some("Paused by Han")
        );
        assert_eq!(app.paused_by_line(None, None), None, "no menu");
        // Leia opened it and her pad dropped: the menu passed to P1, and the line names Han.
        assert_eq!(
            app.paused_by_line(Some(1), Some(0)).as_deref(),
            Some("Paused by Han")
        );
        // The Splitscreen page's rows name each player as the pause line does.
        let rows = |app: &App| app.split_seat_names(&app.split.as_ref().expect("split").seats);
        assert_eq!(
            rows(&app)[..2],
            [Some("Han".to_string()), Some("Leia".to_string())]
        );
        // A session name (the feed's) wins on both.
        app.names.set([(1, "Chewie")]);
        assert_eq!(
            app.paused_by_line(Some(1), Some(1)).as_deref(),
            Some("Paused by Chewie")
        );
        assert_eq!(rows(&app)[1].as_deref(), Some("Chewie"));
        app.names.clear();
        app.pause_opened_by = Some((0, true));
        assert_eq!(
            app.paused_by_line(Some(0), Some(0)).as_deref(),
            Some("Paused (keyboard)")
        );
        // A LAN round never pauses: the line names whose menu it is and that the battle runs on.
        app.netplay = Some(lan_host());
        app.pause_opened_by = Some((1, false));
        let lan = app.paused_by_line(Some(1), Some(1));
        assert_eq!(lan.as_deref(), Some("Leia's menu (battle runs on)"));
        assert!(!lan.unwrap().contains("Paused"));
        assert_eq!(
            app.paused_by_line(Some(1), Some(0)).as_deref(),
            Some("Leia's menu (battle runs on)"),
            "never passed on in a LAN round"
        );
        app.pause_opened_by = Some((0, true));
        assert_eq!(
            app.paused_by_line(Some(0), Some(0)).as_deref(),
            Some("Keyboard menu (battle runs on)")
        );
        app.netplay = None;
        app.pause_opened_by = None;
        // Local player 0 on seat P2 (P1 left the Players screen): "Player 2", not P2's profile.
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_keyboard();
        o.join_pad(2);
        o.join_pad(3);
        o.leave(0);
        let seats = game::local_seats::Seats::from_owners(o, 2);
        assert_eq!(
            local_name(&seats, 0, "Han", &app.split_profiles),
            "Player 2"
        );
    }

    /// Issue #194 B4: each view's scoreboard names the other local player by their profile,
    /// never "Player N": view 0 lists P2 as "Han", view 1 (P2's own feed, under
    /// `with_local(1)`) lists P1 as "Luke", and each view marks its own row.
    #[test]
    fn each_views_scoreboard_names_the_other_player_by_profile() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Luke".into();
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let mut han = game::profile::Profile::default().into_seat();
        han.name = "Han".into();
        app.split_profiles[1] = Some(han);
        app.split = Some(state(2, false));
        app.split_feeds = vec![game::event_feed::EventFeed::default()];
        app.name_split_players(&walk);
        assert_eq!(app.names.get(0), Some("Luke"));
        assert_eq!(app.names.get(1), Some("Han"));
        let titles = || [String::from("A"), String::from("B")];
        let rows = |b: &ui::scoreboard::Scoreboard| -> Vec<(String, bool)> {
            b.teams
                .iter()
                .flat_map(|t| t.rows.iter())
                .filter(|r| r.human)
                .map(|r| (r.name.clone(), r.me))
                .collect()
        };
        let view0 = super::super::events::scoreboard_of(&walk, &app.feed, titles(), None);
        let mut view0 = rows(&view0);
        view0.sort();
        assert_eq!(
            view0,
            [("Han".to_string(), false), ("Luke".to_string(), true)]
        );
        let feed1 = &app.split_feeds[0];
        let view1 = walk.with_local(1, |w| {
            super::super::events::scoreboard_of(w, feed1, titles(), None)
        });
        let mut view1 = rows(&view1);
        view1.sort();
        assert_eq!(
            view1,
            [("Han".to_string(), true), ("Luke".to_string(), false)]
        );
        // A rename reaches every feed once its generation moves; nothing is copied otherwise.
        let fed = app.names_fed;
        app.feed_names();
        assert_eq!(app.names_fed, fed);
        app.split_profiles[1].as_mut().unwrap().name = "Solo".into();
        app.name_split_players(&walk);
        let h = walk.local_unit(1);
        assert_eq!(app.split_feeds[0].unit_name(&walk.sim, h), "Solo");
        assert_eq!(app.feed.unit_name(&walk.sim, h), "Solo");
    }

    /// Issue #194 B9: each view's round-end stats pages are its own player's. View 1's pages
    /// (`split_stats_pages`, under `with_local(1)` with P2's feed) mark P2 ("Han") as "me", carry
    /// P2's own career, and count P2's "medals this match" from P2's slot against P2's opening
    /// counts; view 0's mark P1 ("Luke") with P1's career and no medal. Each lists the other by
    /// profile name, never "Player N". An away player gets no pages.
    #[test]
    fn each_views_stats_pages_are_its_own_players() {
        use game::event_feed::EventFeed;
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Luke".into();
        app.profile.career.kills = 7;
        let mut walk = game::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let p2 = walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        let mut han = game::profile::Profile::default().into_seat();
        han.name = "Han".into();
        han.career.kills = 42;
        app.split_profiles[1] = Some(han);
        app.split = Some(state(2, false));
        app.split_feeds = vec![EventFeed::default()];
        // P2 comes in with one of every medal already; P1 with none.
        walk.sim.install_medal_career(walk.player, [0; 9]);
        walk.sim.install_slot_medal_career(1, p2, [1; 9]);
        app.feed.set_opening_medals(&walk);
        let feed1 = &mut app.split_feeds[0];
        walk.with_local(1, |w| feed1.set_opening_medals(w));
        app.name_split_players(&walk);
        // P2 earns War Hero this match.
        walk.sim.score_medal_points(walk.local_unit(1), 36);
        let earned = walk.sim.slot_medal_counts(1).expect("P2's career");
        assert!(
            earned.iter().sum::<u32>() > 9,
            "P2 earned a medal: {earned:?}"
        );

        let me = |s: &ui::stats::StatsScreens| s.player_row().cloned().expect("a me row");
        let humans = |s: &ui::stats::StatsScreens| -> Vec<(String, bool)> {
            let mut v: Vec<(String, bool)> = s
                .lists
                .iter()
                .flat_map(|l| l.rows.iter())
                .filter(|r| r.human)
                .map(|r| (r.name.clone(), r.me))
                .collect();
            v.sort();
            v
        };
        let kills = |s: &ui::stats::StatsScreens| s.career.values[2].1.clone();

        let view0 = super::super::stats_pages(&walk, &app.feed, &app.profile.career);
        let seats = seat_numbers(app.split.as_ref(), walk.local_count());
        let pages = split_stats_pages(&mut walk, &app.split_feeds, &app.split_profiles, &seats);
        assert_eq!(pages.len(), 2);
        assert!(
            pages[0].is_none(),
            "view 0's pages are made as in single player"
        );
        let view1 = pages[1].clone().expect("P2's pages");

        assert_eq!(me(&view0).name, "Luke");
        assert_eq!(kills(&view0), "7", "P1's career");
        assert!(me(&view0).medals.is_empty(), "P1 earned nothing");
        assert_eq!(
            humans(&view0),
            [("Han".to_string(), false), ("Luke".to_string(), true)]
        );

        assert_eq!(me(&view1).name, "Han");
        assert_eq!(kills(&view1), "42", "P2's own career");
        let medals = me(&view1).medals;
        assert!(
            medals.iter().any(|(n, _)| n == "War Hero"),
            "P2's War Hero this match: {medals:?}"
        );
        assert!(
            medals.iter().all(|&(_, n)| n == 1),
            "counted from P2's opening counts (1 of each), not from 0: {medals:?}"
        );
        assert_eq!(
            humans(&view1),
            [("Han".to_string(), true), ("Luke".to_string(), false)]
        );
        assert!(
            view0
                .lists
                .iter()
                .chain(&view1.lists)
                .flat_map(|l| &l.rows)
                .all(|r| !r.name.starts_with("Player ")),
            "nobody is \"Player N\""
        );

        // An away player gets no pages, and gets their own once back while the pages are up;
        // pages already made are kept.
        let away = walk.suspend_local_player(1).expect("P2 suspended");
        let mut pages = split_stats_pages(&mut walk, &app.split_feeds, &app.split_profiles, &seats);
        assert!(pages[1].is_none());
        fill_split_stats_pages(
            &mut walk,
            &app.split_feeds,
            &app.split_profiles,
            &seats,
            &mut pages,
        );
        assert!(pages[1].is_none(), "still away");
        walk.resume_local_player(1, away).expect("P2 back");
        fill_split_stats_pages(
            &mut walk,
            &app.split_feeds,
            &app.split_profiles,
            &seats,
            &mut pages,
        );
        let back = pages[1].as_mut().expect("P2's pages once back");
        assert_eq!(me(back).name, "Han");
        back.page = ui::stats::Page::Career;
        fill_split_stats_pages(
            &mut walk,
            &app.split_feeds,
            &app.split_profiles,
            &seats,
            &mut pages,
        );
        assert_eq!(
            pages[1].as_ref().unwrap().page,
            ui::stats::Page::Career,
            "made pages are kept"
        );
    }

    /// Issue #194 B9: each player's scoreboard is held on their own pad. P2 holding their board
    /// button shows P2's board only; a frame whose pads were not sampled (paused, round end)
    /// releases every hold.
    #[test]
    fn each_players_board_is_held_on_their_own_pad() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        let mut split = state(3, false);
        let select = PadState {
            buttons: buttons::SPAWN_CLASS_PREV,
            ..Default::default()
        };
        split.pads = vec![PadState::default(), select, PadState::default()];
        app.split = Some(split);
        for _ in 0..30 {
            app.step_split_score_holds(true, 1.0 / 60.0);
        }
        let shown = |app: &App| -> Vec<bool> {
            app.split
                .as_ref()
                .unwrap()
                .score_holds
                .iter()
                .map(|h| h.shown())
                .collect()
        };
        assert_eq!(shown(&app), [false, true, false]);
        app.step_split_score_holds(false, 1.0 / 60.0);
        assert_eq!(shown(&app), [false, false, false]);
    }

    /// Issue #194 B21: two teams with a post each on flat ground, `classes` soldier classes a
    /// side, a first player (team 0) and P2 (team 1), both on the field.
    fn posts_two_locals(classes: usize) -> game::walk::WalkSim {
        use glam::Vec3;
        let flat = core::heightfield::Heightfield {
            size: 33,
            unit: 8.0,
            offset_x: 128.0,
            offset_z: 128.0,
            heights: vec![0.0; 33 * 33],
        };
        let mut w = game::walk::WalkSim::new(7, Vec3::ZERO, Some(flat));
        for (x, team) in [(0.0f32, 0u8), (-300.0, 1)] {
            let pos = Vec3::new(x, 0.0, 0.0);
            w.sim.add_cp(
                core::cp::CpDef {
                    name: format!("cp{x}"),
                    pos,
                    capture: core::region::Region::sphere(pos, 8.0),
                    control: None,
                    value_bleed: 1,
                    value_atk: [1, 1],
                    value_def: [1, 1],
                    spawn_nodes: vec![(pos + Vec3::new(0.0, 0.0, 1.0), 0.0)],
                    capture_time: 10.0,
                    neutralize_time: 10.0,
                },
                Some(team),
            );
        }
        let trooper = core::unit::UnitClass {
            consts: core::soldier::SoldierConstants::default(),
            max_health: 300.0,
            health_type: core::weapon::HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            jet: false,
            capture_posts: true,
            drops: core::sim::DropTable::default(),
            chunk: None,
            min: 0,
            max: None,
            unlock_points: 0,
            droideka: None,
        };
        for t in 0..2 {
            w.teams[t].classes = (0..classes)
                .map(|c| game::walk::ClassVisual {
                    name: format!("class{c}"),
                    label: format!("Class {c}"),
                    ..Default::default()
                })
                .collect();
            w.sim.classes[t] = vec![trooper.clone(); classes];
        }
        w.add_local_player(1, Some(1)).expect("slot 1 is free");
        for slot in 0..2 {
            w.sim.open_spawn_screens(slot, false);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
            tap(&mut w, slot, buttons::SPAWN_ACCEPT);
        }
        for i in 0..2 {
            assert!(w.sim.unit(w.local_unit(i)).is_some_and(|u| u.alive));
        }
        w
    }

    fn tap(w: &mut game::walk::WalkSim, slot: usize, bit: u32) {
        let mut p = [PadState::default(); 2];
        p[slot].buttons = bit;
        w.step_tick(&p);
        w.step_tick(&[PadState::default(); 2]);
    }

    /// Each view's HUD model as the window builds them: view 0's from the round's feed, the
    /// others' from [`further_view_models`].
    fn view_models(
        walk: &mut game::walk::WalkSim,
        feed: &mut game::event_feed::EventFeed,
        feeds: &mut [game::event_feed::EventFeed],
    ) -> Vec<(Option<ui::hud::HudModel>, bool)> {
        let away: Vec<bool> = (0..walk.local_count())
            .map(|i| walk.is_suspended(i))
            .collect();
        let first = (
            Some(super::super::hud_model(walk, feed, 16.0 / 9.0)),
            super::super::spawn_screen_up(walk),
        );
        let mut out = vec![first];
        out.extend(further_view_models(walk, feeds, &away, 16.0 / 9.0, &[]));
        out
    }

    /// Issue #194 B21: a split space round builds a HUD model for every view, each of its own
    /// player, and installs no medal career for any local player, the first included (a space
    /// round keeps none, `Sim::install_slot_medal_career`).
    #[test]
    fn a_split_space_round_has_a_hud_per_view_and_no_medal_careers() {
        use game::event_feed::EventFeed;
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        let mut walk = posts_two_locals(1);
        walk.sim.space_assault = Some(core::sim::SpaceAssault::new(10));
        assert!(walk.space_battle());
        let mut han = game::profile::Profile::default().into_seat();
        han.name = "Han".into();
        han.career.medals = [2; 9];
        app.split_profiles[1] = Some(han);
        app.split = Some(state(2, false));
        // The window's installs: the host's career on P1, the seat profiles on the others.
        walk.sim.install_medal_career(walk.player, [3; 9]);
        app.install_seat_profiles(&mut walk);
        for slot in 0..2 {
            assert_eq!(walk.sim.slot_medal_counts(slot), None, "slot {slot}");
        }
        assert!(walk.netplay.profiles.contains_key(&1), "P2's profile is in");
        // P2 is hurt; P1 is not.
        let p2 = walk.local_unit(1).index as usize;
        walk.sim.units[p2].health.cur = 120.0;
        let mut feed = EventFeed::default();
        let mut feeds = vec![EventFeed::default()];
        let models = view_models(&mut walk, &mut feed, &mut feeds);
        assert_eq!(models.len(), 2);
        let hp: Vec<f32> = models
            .iter()
            .map(|(m, _)| m.as_ref().expect("a HUD model for every view").hp)
            .collect();
        assert_eq!(hp, [300.0, 120.0], "each view shows its own player");
        assert!(models.iter().all(|(_, spawn)| !spawn));
        // P2 takes off in a starfighter, damaged: view 1 shows its ship's health, view 0 none.
        {
            use core::vehicle::{flyer::FlyerParams, SeatKind, VehicleKind, VehicleSpec};
            walk.sim.flyer_params = vec![Some(FlyerParams::default())];
            let at = walk.sim.unit(walk.local_unit(1)).unwrap().soldier.pos;
            let ship = walk.sim.spawn_vehicle(&VehicleSpec {
                kind: VehicleKind::Flyer,
                class: 0,
                home_team: None,
                neutral_when_empty: false,
                spawn_pad: None,
                pos: at + glam::Vec3::new(-6.0, 0.0, 0.0),
                yaw: 0.0,
                max_health: 100.0,
                health_type: core::weapon::HealthType::Vehicle,
                seats: vec![SeatKind::Pilot],
            });
            assert!(walk.sim.seat_unit(walk.local_unit(1), ship, 0));
            walk.sim.vehicle_mut(ship).unwrap().pos.y = 30.0;
            walk.sim.vehicle_mut(ship).unwrap().health.cur = 40.0;
            let models = view_models(&mut walk, &mut feed, &mut feeds);
            let seat: Vec<Option<(f32, f32)>> = models
                .iter()
                .map(|(m, _)| m.as_ref().expect("a HUD model").seat_health)
                .collect();
            assert_eq!(seat, [None, Some((40.0, 100.0))], "P2's ship on P2's view");
        }
        // Unchanged a step later: the space round keeps no careers.
        walk.step_tick(&[PadState::default(); 2]);
        for slot in 0..2 {
            assert_eq!(walk.sim.slot_medal_counts(slot), None, "slot {slot}");
        }
    }

    /// Issue #194 B21: P2 dies and opens the class screen on their spawn page, then moves its
    /// class row with their own pad: view 1's spawn page moves, P1's stays.
    #[test]
    fn p2s_class_row_moves_on_p2s_pad_only() {
        use game::event_feed::EventFeed;
        let mut walk = posts_two_locals(3);
        let mut s = state(2, false);
        for slot in 0..2 {
            walk.sim.open_spawn_screens(slot, false);
            tap(&mut walk, slot, buttons::SPAWN_ACCEPT);
        }
        let (mut feed, mut feeds) = (EventFeed::default(), vec![EventFeed::default()]);
        // Each view's spawn page as its HUD shows it: whether the class screen is up, and the
        // class row's selection.
        let row = |walk: &mut game::walk::WalkSim,
                   feed: &mut EventFeed,
                   feeds: &mut [EventFeed]|
         -> Vec<(bool, usize)> {
            let models = view_models(walk, feed, feeds);
            assert!(models
                .iter()
                .all(|(m, up)| *up && m.as_ref().unwrap().spawn.is_some()));
            let f1 = &feeds[0];
            [
                super::super::spawn_model(walk, feed),
                walk.with_local(1, |w| super::super::spawn_model(w, f1)),
            ]
            .into_iter()
            .map(|m| {
                let m = m.expect("a spawn page");
                let class = matches!(m.screen, Some(ui::spawn::SpawnScreen::Class));
                (class, m.selected_class)
            })
            .collect()
        };
        let before = row(&mut walk, &mut feed, &mut feeds);
        assert_eq!(before, [(true, 0), (true, 0)], "both on the class screen");
        // P2's pad holds next class for a few frames, then lets go; P1's pad does nothing.
        s.pads = vec![
            PadState::default(),
            PadState {
                buttons: buttons::SPAWN_CLASS_NEXT | buttons::SWITCH_WEAPON,
                ..Default::default()
            },
        ];
        for _ in 0..3 {
            let pads = s.finish_pads(PadState::default(), &walk);
            walk.step_tick(&pads);
        }
        s.pads = vec![PadState::default(); 2];
        let pads = s.finish_pads(PadState::default(), &walk);
        walk.step_tick(&pads);
        let after = row(&mut walk, &mut feed, &mut feeds);
        assert_eq!(after[0], before[0], "P1's page did not move");
        assert_eq!(after[1], (true, 1), "P2's class row moved one step");
        assert_eq!((walk.sim.spawn_choice[0], walk.sim.spawn_choice[1]), (0, 1));
    }

    /// Two-flag CTF with P1 (team 0) and P2 (team 1) on opposite sides: each view colours its
    /// own side's flag as the friend's and the other's as the enemy's. The one `VehicleVisuals`
    /// serves every cell, so each view sets its viewer ([`set_ctf_viewer`]) before it draws.
    #[test]
    fn each_view_colours_the_ctf_flags_for_its_own_side() {
        use game::point_light::Relation;
        let mut walk = posts_two_locals(1);
        walk.sim.ctf = Some(core::sim::CtfState::new_two_flag(
            5,
            glam::Vec3::ZERO,
            glam::Vec3::ZERO,
            glam::Vec3::X * 50.0,
            glam::Vec3::X * 50.0,
        ));
        let team = |w: &game::walk::WalkSim| w.sim.unit(w.player).map(|u| u.team);
        assert_eq!(team(&walk), Some(0));
        assert_eq!(walk.with_local(1, |w| team(w)), Some(1));
        let rgb = |rel| {
            Some(
                game::holo::relation_rgb(rel, false)
                    .map(|x| render::srgb_decode(f32::from(x) / 255.0)),
            )
        };
        let (friend, enemy) = (rgb(Relation::Friend), rgb(Relation::Enemy));
        let mut vis = game::viewer::VehicleVisuals::default();
        // The frame's order: view 0 first, then each further view in its cell.
        set_ctf_viewer(&mut vis, &walk);
        assert_eq!(
            vis.ctf_flag_tints(&walk.sim),
            [friend, enemy],
            "view 0: team 0's flag is P1's"
        );
        let view1 = walk.with_local(1, |w| {
            set_ctf_viewer(&mut vis, w);
            vis.ctf_flag_tints(&w.sim)
        });
        assert_eq!(view1, [enemy, friend], "view 1: team 1's flag is P2's");
        set_ctf_viewer(&mut vis, &walk);
        assert_eq!(
            vis.ctf_flag_tints(&walk.sim),
            [friend, enemy],
            "view 0 again next frame"
        );
    }

    /// Issue #194 B21: P2 dies. View 1 shows P2's death camera and then P2's spawn screens, while
    /// view 0 plays on: no death camera, no spawn page, and P1 walks.
    #[test]
    fn p2s_death_shows_in_view_1_only() {
        use game::event_feed::EventFeed;
        let mut walk = posts_two_locals(1);
        let (mut feed, mut feeds) = (EventFeed::default(), vec![EventFeed::default()]);
        let cell: Cell = [0, 0, 640, 720];
        let foot = game::render_blend::view_camera(&mut walk, 1, cell, &PadState::default(), None);
        let h = walk.local_unit(1);
        walk.sim.kill_environment(h, core::sim::DeathCause::Water);
        let go = [
            PadState {
                ly: 20000,
                ..Default::default()
            },
            PadState::default(),
        ];
        let p1_at = |w: &game::walk::WalkSim| w.sim.unit(w.local_unit(0)).unwrap().soldier.pos;
        let start = p1_at(&walk);
        for _ in 0..3 {
            walk.step_tick(&go);
        }
        assert!(
            walk.with_local(1, |w| w.death_cam.is_some()),
            "P2's death camera"
        );
        assert!(walk.death_cam.is_none(), "P1 plays on");
        let dead = game::render_blend::view_camera(&mut walk, 1, cell, &PadState::default(), None);
        assert_ne!(dead.position, foot.position, "view 1 films the death");
        let models = view_models(&mut walk, &mut feed, &mut feeds);
        assert!(models[0]
            .0
            .as_ref()
            .is_some_and(|m| !m.dead && m.spawn.is_none()));
        assert!(models[1].0.as_ref().is_some_and(|m| m.dead));
        // The respawn timer runs out: P2's spawn screens come up in view 1 alone.
        let mut up = false;
        for _ in 0..900 {
            walk.step_tick(&go);
            if view_models(&mut walk, &mut feed, &mut feeds)[1].1 {
                up = true;
                break;
            }
        }
        assert!(up, "P2's spawn screens came up");
        let models = view_models(&mut walk, &mut feed, &mut feeds);
        assert!(models[1].0.as_ref().is_some_and(|m| m.spawn.is_some()));
        assert!(!models[0].1, "no spawn screens in view 0");
        assert!(models[0]
            .0
            .as_ref()
            .is_some_and(|m| !m.dead && m.spawn.is_none()));
        assert!(walk.sim.unit(walk.local_unit(0)).is_some_and(|u| u.alive));
        assert!(
            (p1_at(&walk) - start).length() > 1.0,
            "P1 walked on throughout"
        );
    }

    /// Issue #194 B21: P2 opens the Game Menu and P2's pad drops. The menu passes to P1
    /// ([`menu_driver`]) and its title then names P1 ("Paused by Han"); [`apply_presence`] run
    /// afterwards suspends P2 alone, and P1 plays on. This pins the hand-off and the
    /// suspension's result only. That the window never calls `apply_presence` while the menu is
    /// up rests on `events.rs`'s per-frame branch order (the pad-sampling branches that seat
    /// pads are skipped while `self.paused` or a popup is up); this test does not drive that
    /// path, so a change there must keep the rule by hand.
    #[test]
    fn a_lost_pad_under_p2s_menu_passes_it_on_then_suspends_p2_alone() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Han".into();
        app.split_profiles = seat_profiles([None, Some("Leia"), None, None]);
        app.split = Some(state(2, false));
        let mut walk = posts_two_locals(1);
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(20);
        let mut seats = game::local_seats::Seats::from_owners(o, 2);
        let mut presence = game::local_seats::Presence::new(2);
        let mut away: Vec<Option<game::walk::Suspended>> = vec![None; 2];
        let profiles: [Option<game::profile::Profile>; 4] = Default::default();
        let connected = |seats: &game::local_seats::Seats<u8>, i: usize| seats.pad_of(i).is_some();
        // P2 (local 1) opens the menu and drives it.
        let owner = 1;
        let driver = menu_driver(owner, 2, None, |i| connected(&seats, i));
        assert_eq!(driver, 1);
        assert_eq!(
            app.paused_by_line(Some(owner), Some(driver)).as_deref(),
            Some("Paused by Leia")
        );
        // P2's pad drops while the menu is up: the menu passes to P1, whose name the title now
        // gives; nobody is suspended yet.
        assert_eq!(seats.gone(20), Some(1));
        let driver = menu_driver(owner, 2, None, |i| connected(&seats, i));
        assert_eq!(driver, 0);
        assert_eq!(
            app.paused_by_line(Some(owner), Some(driver)).as_deref(),
            Some("Paused by Han")
        );
        assert!(!walk.is_suspended(1));
        // The game resumes: the next sampled frame suspends P2, and only P2.
        let back = apply_presence(&seats, &mut presence, &mut away, &mut walk, &profiles);
        assert!(back.is_empty());
        assert!(walk.is_suspended(1) && !walk.is_suspended(0));
        assert!(away[1].is_some() && away[0].is_none());
        let start = walk.sim.unit(walk.local_unit(0)).unwrap().soldier.pos;
        let go = PadState {
            ly: 20000,
            ..Default::default()
        };
        for _ in 0..60 {
            walk.step_tick(&[go, PadState::default()]);
        }
        let now = walk.sim.unit(walk.local_unit(0)).unwrap().soldier.pos;
        assert!((now - start).length() > 1.0, "P1 plays on");
        // A second pass changes nothing; the pad coming back seats P2 behind the spawn screens.
        assert!(apply_presence(&seats, &mut presence, &mut away, &mut walk, &profiles).is_empty());
        assert_eq!(seats.offer(20), Some(1));
        let back = apply_presence(&seats, &mut presence, &mut away, &mut walk, &profiles);
        assert_eq!(back, [1]);
        assert!(!walk.is_suspended(1) && away[1].is_none());
        assert_ne!(walk.sim.spawn_stage[1], core::sim::SpawnStage::Accepted);
    }

    /// Finding split-round-end-pads-never-seated-offline: an offline round's end holds the game
    /// as a popup does (`App::round_end_held`), yet its frames still take the round-end branch
    /// ([`frame_pads`]), which seats and drops split pads. P2's pad drops before the end; the
    /// stats come up without P2's pages; P2's pad comes back during the stats and P2 is seated,
    /// gets their own pages and is no longer away. Then P1 leaves their pages and the round waits
    /// for P2; P2's pad goes again, P2 is away, loses their pages and is not waited for, so the
    /// stats end without the idle timeout.
    #[test]
    fn an_offline_round_end_still_seats_and_drops_split_pads() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Han".into();
        app.split_profiles = seat_profiles([None, Some("Leia"), None, None]);
        app.split = Some(state(2, false));
        app.split_feeds = vec![game::event_feed::EventFeed::default()];
        app.walk = Some((posts_two_locals(1), render::MeshHandle::default()));
        let mut o = input::owners::PadOwners::<u8>::default();
        o.join_pad(10);
        o.join_pad(20);
        let mut seats = game::local_seats::Seats::from_owners(o, 2);
        let mut presence = game::local_seats::Presence::new(2);
        let mut away: Vec<Option<game::walk::Suspended>> = vec![None; 2];
        let profiles = app.split_profiles.clone();
        let seat_nums = [0usize, 1];
        // P2's pad drops while the round runs: P2 is away.
        assert_eq!(seats.gone(20), Some(1));
        {
            let walk = &mut app.walk.as_mut().unwrap().0;
            apply_presence(&seats, &mut presence, &mut away, walk, &profiles);
            assert!(walk.is_suspended(1));
            walk.sim.outcome = Some(0);
        }
        // The round has its result: offline that holds the game, and the frame's pads still go
        // to the round-end branch, never to the popup's (which seats nothing).
        assert!(app.round_end_held());
        assert_eq!(
            frame_pads(false, false, app.round_end_held(), true),
            FramePads::RoundEnd
        );
        assert_eq!(frame_pads(false, false, true, false), FramePads::Popup);
        assert_eq!(frame_pads(true, false, true, true), FramePads::Paused);
        assert_eq!(frame_pads(false, false, false, false), FramePads::Play);
        // The stats come up, as the frame makes them; P2 is away and has none.
        let mut pages = vec![None; 2];
        {
            let walk = &mut app.walk.as_mut().unwrap().0;
            let mut r = game::round_end::RoundEnd::start(
                game::round_end::Result::Victory,
                &[],
                1,
                None,
                None,
            );
            r.phase = game::round_end::Phase::Stats;
            r.stats = Some(super::super::stats_pages(
                walk,
                &app.feed,
                &app.profile.career,
            ));
            r.hold_for_players(2);
            walk.round_end = Some(r);
            fill_split_stats_pages(walk, &app.split_feeds, &profiles, &seat_nums, &mut pages);
        }
        assert!(pages[1].is_none(), "away: no pages");
        assert_eq!(app.away_flags(), [false, true]);
        // P2's pad comes back during the stats: seated, back, with their own pages.
        assert_eq!(seats.offer(20), Some(1));
        {
            let walk = &mut app.walk.as_mut().unwrap().0;
            let back = apply_presence(&seats, &mut presence, &mut away, walk, &profiles);
            assert_eq!(back, [1]);
            fill_split_stats_pages(walk, &app.split_feeds, &profiles, &seat_nums, &mut pages);
        }
        assert!(pages[1].is_some(), "P2's own pages once back");
        assert_eq!(app.away_flags(), [false, false]);
        // P1 leaves their pages: P1's cell waits for P2 by name.
        let quit = ui::stats::Pad {
            square: true,
            ..Default::default()
        };
        {
            let r = app.walk.as_mut().unwrap().0.round_end.as_mut().unwrap();
            r.step(0.0, quit);
            assert!(r.has_left(0) && !r.done());
        }
        let cells: Vec<game::split_layout::Cell> = vec![[0, 0, 640, 720], [640, 0, 640, 720]];
        let here = app.away_flags();
        assert_eq!(
            app.split_waiting(&cells, &here),
            [(cells[0], "Waiting for Leia".to_string())]
        );
        // P2's pad goes during the stats: P2 is away, loses their pages and is not waited for.
        assert_eq!(seats.gone(20), Some(1));
        {
            let walk = &mut app.walk.as_mut().unwrap().0;
            apply_presence(&seats, &mut presence, &mut away, walk, &profiles);
            fill_split_stats_pages(walk, &app.split_feeds, &profiles, &seat_nums, &mut pages);
        }
        let gone = app.away_flags();
        assert_eq!(gone, [false, true]);
        assert!(app.split_waiting(&cells, &gone).is_empty());
        assert!(pages[1].is_none(), "away: their pages go");
        let r = app.walk.as_mut().unwrap().0.round_end.as_mut().unwrap();
        r.step_views(&[ui::stats::Pad::default()], &mut pages[1..2], false);
        assert!(r.done(), "the stats end without waiting for P2");
    }

    /// A LAN split round (findings lan-menu-handoff-freezes-other-player and
    /// lan-free-keyboard-esc-covers-p1): the round runs on under the Game Menu, so it is never
    /// passed to another player. P2's pad drops under P2's menu: the menu closes (offline it
    /// passes to P1). With nobody on the keyboard, Esc opens nothing while a seated pad is
    /// connected; once every pad is gone it opens, and it closes again when a pad is back.
    #[test]
    fn a_lan_menu_closes_when_its_openers_pad_goes_and_is_never_passed_on() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.split = Some(state(2, false));
        let open = |app: &mut App, owner: usize, by_keys: bool| {
            app.paused = true;
            app.pause_owner = Some(owner);
            app.pause_opened_by = Some((owner, by_keys));
        };
        // Offline: P2's pad drops, the menu passes to P1 and stays open.
        open(&mut app, 1, false);
        app.settle_split_menu(None, &[true, false]);
        assert!(app.paused);
        assert_eq!(app.pause_owner, Some(0));
        // LAN: the same drop closes the menu; nobody else is handed it.
        app.netplay = Some(lan_host());
        open(&mut app, 1, false);
        app.settle_split_menu(None, &[true, true]);
        assert!(
            app.paused && app.pause_owner == Some(1),
            "P2's pad is still here"
        );
        app.settle_split_menu(None, &[true, false]);
        assert!(!app.paused, "closed, not passed on");
        assert_eq!((app.pause_owner, app.pause_opened_by), (None, None));
        assert_eq!(app.lan_menu_view(), None, "no cell holds a menu");
        // A keyboard player's menu never closes for a pad.
        open(&mut app, 0, false);
        app.settle_split_menu(Some(0), &[false, false]);
        assert!(app.paused);
        // The free keyboard: no Esc while either pad plays; with both gone it may open, and a
        // pad coming back closes it.
        assert!(!lan_free_keyboard_opens(2, |i| i == 1));
        assert!(lan_free_keyboard_opens(2, |_| false));
        open(&mut app, 0, true);
        app.settle_split_menu(None, &[false, false]);
        assert!(app.paused, "the keyboard drives it while every pad is gone");
        app.settle_split_menu(None, &[false, true]);
        assert!(!app.paused, "a pad is back: it does not cover P1");
        // With no pads known (no gilrs) the keyboard's Esc opens as the first seat, as before.
        assert_eq!(app.pause_opener(true), Some(0));
    }

    /// Finding esc-menu-p1-away-passes-to-p2-keyboard-locked-out: an offline split round on pads
    /// only, P1 away. Esc opens the Game Menu as the lowest seat still here (P2), not away P1;
    /// the keyboard drives and closes it whichever pad drives it too, and the title keeps
    /// "Paused (keyboard)" rather than naming a pad player who never opened it.
    #[test]
    fn esc_with_p1_away_opens_a_keyboard_menu_the_keyboard_keeps() {
        let mut app = App::new(
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile.name = "Han".into();
        app.split_profiles = seat_profiles([None, Some("Leia"), None, None]);
        app.split = Some(state(2, false));
        let mut walk = posts_two_locals(1);
        walk.suspend_local_player(0).expect("P1 drops");
        app.walk = Some((walk, render::MeshHandle::default()));
        assert!(app.split_on());
        assert_eq!(app.split.as_ref().unwrap().seats.keyboard_player(), None);
        // The opener: P2, the lowest seat not away.
        assert_eq!(app.pause_opener(true), Some(1));
        assert_eq!(free_keyboard_opener(2, |_| false, &[true, false]), 1);
        assert_eq!(
            free_keyboard_opener(2, |i| i == 1, &[false, false]),
            1,
            "pad first"
        );
        assert_eq!(
            free_keyboard_opener(2, |_| false, &[true, true]),
            0,
            "nobody here"
        );
        let open = |app: &mut App, owner: usize| {
            app.paused = true;
            app.pause_owner = Some(owner);
            app.pause_opened_by = Some((owner, true));
        };
        open(&mut app, 1);
        app.settle_split_menu(None, &[false, true]);
        assert_eq!(app.pause_owner, Some(1));
        assert!(
            app.pause_keyboard_drives(),
            "the keyboard opened it and drives it"
        );
        assert_eq!(
            app.paused_by_line(Some(1), app.pause_owner).as_deref(),
            Some("Paused (keyboard)")
        );
        // A keyboard menu opened as P1 (the probe's path) passes its pad side to P2 when P1's
        // pad is gone; the keyboard still drives and closes it, and the title stays the keyboard's.
        open(&mut app, 0);
        app.settle_split_menu(None, &[false, true]);
        assert_eq!(app.pause_owner, Some(1), "P2's pad drives it too");
        assert!(app.pause_keyboard_drives());
        assert_eq!(
            app.paused_by_line(Some(0), app.pause_owner).as_deref(),
            Some("Paused (keyboard)")
        );
        // A pad player's menu is still not the free keyboard's.
        app.pause_opened_by = Some((1, false));
        app.pause_owner = Some(1);
        assert!(!app.pause_keyboard_drives());
        assert_eq!(
            app.paused_by_line(Some(1), Some(1)).as_deref(),
            Some("Paused by Leia")
        );
    }
}
