//! The Players screen in the window (issue #194, `analysis/plan/SPLITSCREEN.md` sections 3, 15
//! and 18.1 item 1): each connected pad is heard on its own (the merged menu edges follow one
//! pad), with its own arms' hold repeat, seated in join order by `game::local_seats::Party`
//! with a profile picker per seat, and drawn by `ui::shell_ps2::players`.

use super::{App, FrontShell};
use game::local_seats::{PartyEnv, PartyFrame, PartyPress, PartyStep, SeatProfile};
use ui::shell_ps2::players::{PlayerCell, PlayersHit, PlayersView};

/// The buttons the screen reads, as held bits.
const SOUTH: u16 = 1;
const START: u16 = 2;
const EAST: u16 = 4;
const NORTH: u16 = 8;
const WEST: u16 = 16;
const L1: u16 = 32;
const R1: u16 = 64;
/// The arms (D-pad or left stick), in the menus' order: up, down, left, right.
const ARMS_SHIFT: u16 = 7;
/// Select (Share, Create, View, Back): held, it empties a lost seat (`PartyPress::remove`).
const SELECT: u16 = 1 << (ARMS_SHIFT + 4);

/// One pad's held buttons last frame and its arms' hold repeat.
#[derive(Debug, Clone, Copy)]
pub(super) struct PadHeld {
    id: gilrs::GamepadId,
    bits: u16,
    repeat: ui::shell_feel::HoldRepeat,
    remove: game::local_seats::RemoveHold,
}

fn held(pad: &gilrs::Gamepad<'_>, arms_before: u32) -> u16 {
    use gilrs::Button;
    let mut bits = [
        (Button::South, SOUTH),
        (Button::Start, START),
        (Button::East, EAST),
        (Button::North, NORTH),
        (Button::West, WEST),
        (Button::LeftTrigger, L1),
        (Button::RightTrigger, R1),
        (Button::Select, SELECT),
        (Button::DPadUp, 1 << ARMS_SHIFT),
        (Button::DPadDown, 2 << ARMS_SHIFT),
        (Button::DPadLeft, 4 << ARMS_SHIFT),
        (Button::DPadRight, 8 << ARMS_SHIFT),
    ]
    .iter()
    .filter(|(b, _)| pad.is_pressed(*b))
    .fold(0, |bits, (_, bit)| bits | bit);
    let stick = input::stick_arms(
        pad.value(gilrs::Axis::LeftStickX),
        pad.value(gilrs::Axis::LeftStickY),
        arms_before,
    );
    bits |= (stick as u16 & 15) << ARMS_SHIFT;
    bits
}

/// A press from fresh bits.
fn press(fresh: u16) -> PartyPress {
    let arm = |k: u16| fresh & (k << ARMS_SHIFT) != 0;
    PartyPress {
        join: fresh & SOUTH != 0,
        start: fresh & START != 0,
        back: fresh & EAST != 0,
        west: fresh & WEST != 0,
        north: fresh & NORTH != 0,
        up: arm(1),
        down: arm(2),
        left: arm(4),
        right: arm(8),
        l1: fresh & L1 != 0,
        r1: fresh & R1 != 0,
        remove: false,
    }
}

/// A pad's kind, as its region names it.
fn device_name(family: input::PadFamily) -> &'static str {
    match family {
        input::PadFamily::Ds2 => "DualShock 2",
        input::PadFamily::Ds4 => "DualShock 4",
        input::PadFamily::Ds5 => "DualSense",
        input::PadFamily::XboxOriginal | input::PadFamily::XboxOne => "Xbox controller",
    }
}

/// A profile's career on one line of its Ready card.
fn career_line(c: &game::profile::Career) -> String {
    format!(
        "{} kills   {} points   {} rounds",
        c.kills, c.points, c.rounds
    )
}

impl App {
    /// Every frame on the shell: which pads are connected (the Splitscreen row is live while one
    /// is), and each pad's fresh presses since last frame, its held arms repeating. The held
    /// state is tracked on every page, so the A that opened the Players screen is not also a
    /// join.
    pub(super) fn poll_party_pads(
        &mut self,
        front: &mut FrontShell,
        dt: f32,
    ) -> (Vec<gilrs::GamepadId>, Vec<(gilrs::GamepadId, PartyPress)>) {
        let Some(g) = self.gilrs.as_ref() else {
            front.party_held.clear();
            front.menus.set_splitscreen_available(false);
            return (Vec::new(), Vec::new());
        };
        let ids = input::usable_ids(g);
        front.menus.set_splitscreen_available(!ids.is_empty());
        let mut presses = Vec::new();
        let mut now = Vec::with_capacity(ids.len());
        for &id in &ids {
            let before = front.party_held.iter().find(|p| p.id == id).copied();
            let arms_before = before.map_or(0, |p| u32::from(p.bits >> ARMS_SHIFT) & 15);
            let bits = held(&g.gamepad(id), arms_before);
            let was = before.map_or(bits, |p| p.bits);
            let mut repeat = before.map_or_else(Default::default, |p| p.repeat);
            let again = repeat.step(u32::from(bits >> ARMS_SHIFT) & 15, dt) as u16;
            let fresh = (bits & !was) | (again << ARMS_SHIFT);
            let mut remove = before.map_or_else(Default::default, |p| p.remove);
            let removes = remove.step(bits & SELECT != 0, dt);
            if fresh != 0 || removes {
                presses.push((
                    id,
                    PartyPress {
                        remove: removes,
                        ..press(fresh)
                    },
                ));
            }
            now.push(PadHeld {
                id,
                bits,
                repeat,
                remove,
            });
        }
        front.party_held = now;
        (ids, presses)
    }

    /// One frame of the Players screen: seat, choose, unseat, leave or continue, then fill its
    /// regions.
    pub(super) fn step_players(
        &mut self,
        front: &mut FrontShell,
        connected: Vec<gilrs::GamepadId>,
        pads: Vec<(gilrs::GamepadId, PartyPress)>,
        pointer: Option<([f32; 2], bool)>,
        wheel: f32,
        dt: f32,
    ) {
        front.menus.tick(dt);
        let mut keys = self.key_menu_edges;
        // The keyboard's held arrows repeat as a pad's arms do.
        keys |= front.party_key_repeat.step(self.prev_key_menu & 15, dt);
        // Tab held: the keyboard's way to empty a lost seat, as a pad's Select.
        let key_remove = front
            .party_key_remove
            .step(self.prev_key_menu & game::kbm::MENU_MAP != 0, dt);
        let size = self.renderer.as_ref().map(|r| {
            let (w, h) = r.size();
            [w as f32, h as f32]
        });
        let hit = match (pointer, size) {
            (Some((point, click)), Some(size)) => {
                ui::shell_ps2::players::hit(&front.menus, point, size).map(|h| (h, click))
            }
            _ => None,
        };
        let typed = std::mem::take(&mut front.name_input);
        // Each pad's model, the waiting seats' lost pads too (issue #194: identical pads).
        let models = self.gilrs.as_ref().map_or_else(Vec::new, |g| {
            let owners = front.party.owners();
            let lost = (0..input::owners::MAX_LOCAL_PLAYERS).filter_map(|i| owners.lost_pad(i));
            connected
                .iter()
                .copied()
                .chain(lost)
                .map(|id| (id, g.gamepad(id).uuid()))
                .collect()
        });
        let frame = PartyFrame {
            models,
            connected,
            pads,
            keyboard: PartyPress {
                join: keys & 16 != 0,
                start: keys & game::kbm::MENU_SQUARE != 0,
                back: keys & 32 != 0,
                up: keys & 1 != 0,
                down: keys & 2 != 0,
                left: keys & 4 != 0,
                right: keys & 8 != 0,
                l1: keys & game::kbm::MENU_L1 != 0,
                r1: keys & game::kbm::MENU_R1 != 0,
                remove: key_remove,
                ..Default::default()
            },
            typed: typed.text,
            backspaces: typed.backspaces,
            click_continue: matches!(hit, Some((PlayersHit::Continue, true))),
            pointer: match hit {
                Some((PlayersHit::Picker(seat, target), click)) => Some((seat, target, click)),
                _ => None,
            },
            wheel,
        };
        let env = PartyEnv {
            store: self.profile_store.as_ref(),
            host_id: self.profile.id(),
            difficulty: self.profile.settings.difficulty,
        };
        let out = front.party.step(&frame, &env);
        for cue in out.cues {
            front.menus.cues.play(cue);
        }
        if let Some(profile) = out.host {
            self.apply_host_profile(front, profile);
        }
        match out.step {
            PartyStep::Leave => front.menus.leave_players(),
            PartyStep::Continue => front.menus.players_continue(),
            PartyStep::Stay => {}
        }
        self.fill_players_view(front);
    }

    /// The regions as the seats stand.
    pub(super) fn fill_players_view(&self, front: &mut FrontShell) {
        use game::input_icons::{set_for, Device, IconChoice};
        let owners = front.party.owners();
        // The open quarters' join glyphs: the connected pads nobody holds, when one family.
        let waiting: Vec<ui::input_icons::IconSet> = self
            .gilrs
            .as_ref()
            .map(|g| {
                input::usable_ids(g)
                    .into_iter()
                    .filter(|id| owners.player_of(*id).is_none())
                    .map(|id| {
                        set_for(
                            IconChoice::Auto,
                            Device::Pad(input::pad_family(&g.gamepad(id))),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The seated, connected pads' sets: the Select glyph a lost seat's notice shows.
        let seated_sets: Vec<ui::input_icons::IconSet> = self
            .gilrs
            .as_ref()
            .map(|g| {
                input::usable_ids(g)
                    .into_iter()
                    .filter(|id| owners.player_of(*id).is_some())
                    .map(|id| {
                        set_for(
                            IconChoice::Auto,
                            Device::Pad(input::pad_family(&g.gamepad(id))),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let party = &front.party;
        let mut view = PlayersView {
            // The party's own verdict, so Continue is never shown live and then refused.
            blocked: !party.can_start(),
            twice: std::array::from_fn(|i| party.shares_profile(i)),
            removable: std::array::from_fn(|i| party.removable(i)),
            remove_icons: seated_sets
                .first()
                .copied()
                .filter(|first| seated_sets.iter().all(|s| s == first)),
            keyboard_free: owners.keyboard_player().is_none(),
            join_icons: waiting
                .first()
                .copied()
                .filter(|first| waiting.iter().all(|s| s == first)),
            ..Default::default()
        };
        for (i, cell) in view.cells.iter_mut().enumerate() {
            if !owners.is_seated(i) {
                continue;
            }
            let keyboard = owners.keyboard_player() == Some(i);
            let family = owners.pad_of(i).and_then(|id| {
                self.gilrs
                    .as_ref()
                    .map(|g| input::pad_family(&g.gamepad(id)))
            });
            let device = match (keyboard, owners.pad_of(i), family) {
                (true, _, _) => front.menus.players_texts.keyboard.clone(),
                (false, Some(_), Some(f)) => device_name(f).to_string(),
                (false, Some(_), None) => "Controller".to_string(),
                (false, None, _) => String::new(),
            };
            let icons = match (keyboard, family) {
                (true, _) => ui::input_icons::IconSet::KeyboardMouse,
                (false, Some(f)) => set_for(IconChoice::Auto, Device::Pad(f)),
                (false, None) => ui::input_icons::IconSet::Ds2,
            };
            let lost = owners.is_lost(i);
            let rechoose = front.party.back_rechooses(i, self.profile_store.is_some());
            let seated = |name: String, career: &game::profile::Career| PlayerCell::Seated {
                name,
                device: device.clone(),
                icons,
                career: career_line(career),
                lost,
                rechoose,
            };
            *cell = match front.party.profile(i) {
                SeatProfile::Open => PlayerCell::Open,
                SeatProfile::Picking(picker) => PlayerCell::Picking {
                    picker: picker.clone(),
                    device: device.clone(),
                    icons,
                    lost,
                },
                SeatProfile::Host => seated(
                    super::super::split::seat_name(0, &self.profile.name, &self.split_profiles),
                    &self.profile.career,
                ),
                SeatProfile::Ready(p) => seated(p.name.clone(), &p.career),
                SeatProfile::Lent(_) => match self.split_profiles[i].as_ref() {
                    Some(p) => seated(p.name.clone(), &p.career),
                    None => seated(format!("Player {}", i + 1), &Default::default()),
                },
            };
        }
        front.menus.players = view;
    }
}
