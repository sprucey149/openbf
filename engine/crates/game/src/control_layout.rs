//! Action bindings for the three gameplay contexts. The legacy layouts remain
//! the fallback for older profiles and for the separate spawn screen controls.

use crate::kbm::{Bind, Bindings, Trigger};
use core::sim::buttons as b;
use gilrs::Button;
use serde::{Deserialize, Serialize};
use ui::input_icons::IconSet;
use ui::menu::ControlContext;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layouts<T> {
    pub infantry: Vec<T>,
    pub vehicle: Vec<T>,
    pub starfighter: Vec<T>,
}

impl<T> Default for Layouts<T> {
    fn default() -> Self {
        Self {
            infantry: Vec::new(),
            vehicle: Vec::new(),
            starfighter: Vec::new(),
        }
    }
}

impl<T> Layouts<T> {
    pub fn get(&self, context: ControlContext) -> &[T] {
        match context {
            ControlContext::Infantry => &self.infantry,
            ControlContext::Vehicle => &self.vehicle,
            ControlContext::Starfighter => &self.starfighter,
        }
    }

    pub fn set(&mut self, context: ControlContext, layout: Vec<T>) {
        *match context {
            ControlContext::Infantry => &mut self.infantry,
            ControlContext::Vehicle => &mut self.vehicle,
            ControlContext::Starfighter => &mut self.starfighter,
        } = layout;
    }
}

#[derive(Clone, Copy)]
pub struct Action {
    pub name: &'static str,
    pub bind: Bind,
}

const fn action(name: &'static str, bits: u32) -> Action {
    Action {
        name,
        bind: Bind { bits, walk: (0, 0) },
    }
}

const fn direction(name: &'static str, x: i8, y: i8) -> Action {
    Action {
        name,
        bind: Bind {
            bits: 0,
            walk: (x, y),
        },
    }
}

const FIRE: Action = action("Fire", b::FIRE);
const SECONDARY: Action = action("Secondary fire", b::SECONDARY_FIRE);
const ZOOM: Action = action("Zoom", b::ZOOM);
const VIEW: Action = action("Viewpoint", b::VIEW_TOGGLE);

const INFANTRY: &[Action] = &[
    FIRE,
    SECONDARY,
    ZOOM,
    action("Jump", b::JUMP),
    action("Roll / Stance", b::ROLL_STANCE),
    action("Sprint", b::SPRINT),
    action("Reload", b::RELOAD),
    action("Use / Enter vehicle", b::ACTION),
    action("Next primary weapon", b::SWITCH_WEAPON),
    action("Next secondary weapon", b::SWITCH_SECONDARY),
    action("Squad move out", b::SQUAD_MOVE_OUT),
    action("Squad hold", b::SQUAD_HOLD),
    action(
        "Squad at ease / Accept hero",
        b::SQUAD_AT_EASE | b::HERO_ACCEPT,
    ),
    action(
        "Squad follow / Decline hero",
        b::SQUAD_FOLLOW | b::HERO_DECLINE,
    ),
];

const VEHICLE: &[Action] = &[
    FIRE,
    SECONDARY,
    ZOOM,
    action("Boost", b::SPRINT),
    action("Jump", b::JUMP),
    action("Exit vehicle", b::ACTION),
];

const STARFIGHTER: &[Action] = &[
    FIRE,
    SECONDARY,
    ZOOM,
    action("Boost", b::SPRINT),
    action("Take off / Land", b::JUMP),
    action("Exit starfighter", b::ACTION),
];

pub fn actions(context: ControlContext, keyboard: bool) -> Vec<Action> {
    let contextual = match context {
        ControlContext::Infantry => INFANTRY,
        ControlContext::Vehicle => VEHICLE,
        ControlContext::Starfighter => STARFIGHTER,
    };
    let mut out = Vec::new();
    if keyboard {
        let movement = match context {
            ControlContext::Infantry => {
                ["Move forward", "Move backward", "Move left", "Move right"]
            }
            ControlContext::Vehicle => [
                "Drive forward",
                "Drive backward",
                "Strafe left",
                "Strafe right",
            ],
            ControlContext::Starfighter => [
                "Increase throttle",
                "Decrease throttle",
                "Roll left",
                "Roll right",
            ],
        };
        out.extend(
            movement
                .into_iter()
                .zip([(0, 1), (0, -1), (-1, 0), (1, 0)])
                .map(|(name, (x, y))| direction(name, x, y)),
        );
    }
    out.extend_from_slice(contextual);
    if keyboard {
        out.push(VIEW);
    }
    if context == ControlContext::Starfighter {
        if keyboard {
            out.push(action("Stunt", b::FLYER_TRICK));
            out.push(action("Reverse direction", b::FLYER_REVERSE));
        } else {
            out.push(action("Stunt / Reverse direction", b::ROLL_STANCE));
        }
    }
    out
}

pub fn wheel(trigger: Trigger) -> bool {
    matches!(trigger, Trigger::WheelUp | Trigger::WheelDown)
}

pub fn accepts_trigger(action: Action, trigger: Trigger) -> bool {
    !matches!(trigger, Trigger::Key(winit::keyboard::KeyCode::Escape))
        && (!wheel(trigger) || action.bind.walk == (0, 0))
}

fn valid<T: PartialEq>(
    items: &[T],
    actions: &[Action],
    accepts: impl Fn(Action, &T) -> bool,
) -> bool {
    items.len() == actions.len()
        && items
            .iter()
            .zip(actions)
            .enumerate()
            .all(|(i, (trigger, action))| {
                !items[..i].contains(trigger) && accepts(*action, trigger)
            })
}

pub fn keyboard(context: ControlContext, items: &[Trigger]) -> Option<Bindings> {
    let actions = actions(context, true);
    valid(items, &actions, |a, t| accepts_trigger(a, *t)).then(|| Bindings {
        table: items
            .iter()
            .copied()
            .zip(actions.into_iter().map(|a| a.bind))
            .collect(),
    })
}

pub fn gamepad(context: ControlContext, items: &[Button]) -> Option<input::Mapping> {
    let actions = actions(context, false);
    valid(items, &actions, |_, t| {
        !matches!(t, Button::Unknown | Button::Start)
    })
    .then(|| input::Mapping {
        table: items
            .iter()
            .copied()
            .zip(actions.into_iter().map(|a| a.bind.bits))
            .collect(),
    })
}

/// Each action's first trigger in the table `context` plays without a
/// layout ([`fallback`]): the Controls page's defaults.
pub fn keyboard_defaults(context: ControlContext, legacy: &Bindings) -> Vec<Option<Trigger>> {
    let legacy = &fallback(Some(context), legacy);
    let mut used = Vec::new();
    actions(context, true)
        .into_iter()
        .map(|a| {
            let trigger = legacy.table.iter().find_map(|&(t, b)| {
                let matches = if a.bind.walk == (0, 0) {
                    // Use the action's primary bit to resolve a bundled hero response.
                    b.bits & a.bind.bits & a.bind.bits.wrapping_neg() != 0
                } else {
                    b.walk == a.bind.walk
                };
                (matches && !used.contains(&t) && accepts_trigger(a, t)).then_some(t)
            });
            used.extend(trigger);
            trigger
        })
        .collect()
}

/// The table a context without a valid layout of its own plays, and that its
/// Controls page starts from: the profile's legacy table, except that the
/// stock one gives vehicles and starfighters [`Bindings::vehicle_default`].
/// The 2026-11-14 defaults moved the on-foot mouse buttons; the vehicles'
/// controls are their own contributor's area and stay as they were. A
/// legacy table the player changed plays as saved everywhere.
pub fn fallback(context: Option<ControlContext>, legacy: &Bindings) -> Bindings {
    match context {
        Some(ControlContext::Vehicle | ControlContext::Starfighter)
            if *legacy == Bindings::pc_default() =>
        {
            Bindings::vehicle_default()
        }
        _ => legacy.clone(),
    }
}

/// The keyboard table context `context` plays: its own valid layout, else
/// [`fallback`] of the profile's legacy table. `App::refresh_context_bindings`
/// installs it on every context change.
pub fn context_bindings(
    context: Option<ControlContext>,
    settings: &crate::profile::Settings,
) -> Bindings {
    context
        .and_then(|c| keyboard(c, settings.keyboard_layouts.get(c)))
        .unwrap_or_else(|| fallback(context, &settings.keyboard_bindings))
}

/// The context's factor on mouse look: [`crate::kbm::ON_FOOT_LOOK_SCALE`] on
/// foot and on the spawn screens, 1.0 in a vehicle (turrets included) or a
/// starfighter, whose look keeps the feel it had before 2026-11-14.
pub fn look_scale(context: Option<ControlContext>) -> f64 {
    match context {
        Some(ControlContext::Vehicle | ControlContext::Starfighter) => 1.0,
        _ => crate::kbm::ON_FOOT_LOOK_SCALE,
    }
}

pub fn gamepad_defaults(context: ControlContext, legacy: &input::Mapping) -> Vec<Option<Button>> {
    let mut used = Vec::new();
    actions(context, false)
        .into_iter()
        .map(|a| {
            let button = legacy.table.iter().find_map(|&(t, bits)| {
                (bits & a.bind.bits & a.bind.bits.wrapping_neg() != 0 && !used.contains(&t))
                    .then_some(t)
            });
            used.extend(button);
            button
        })
        .collect()
}

pub fn current(walk: &crate::walk::WalkSim) -> Option<ControlContext> {
    let unit = walk.sim.unit(walk.player)?;
    let slot = usize::from(unit.spec.player_slot?);
    if !unit.alive
        || walk
            .sim
            .spawn_stage
            .get(slot)
            .is_some_and(|stage| *stage != core::sim::SpawnStage::Accepted)
    {
        return None;
    }
    Some(if walk.player_pilots_flyer() {
        ControlContext::Starfighter
    } else if unit.mount.is_some() {
        ControlContext::Vehicle
    } else {
        ControlContext::Infantry
    })
}

/// Local player `i`'s context ([`current`] read through their own unit; 0 is the first
/// player). An away player, or a seat the walk does not have, has none.
pub fn current_of(walk: &mut crate::walk::WalkSim, i: usize) -> Option<ControlContext> {
    if i >= walk.local_count() || walk.is_suspended(i) {
        return None;
    }
    walk.with_local(i, |w| current(w))
}

pub fn keyboard_name(t: Trigger) -> String {
    use winit::event::MouseButton;
    match t {
        Trigger::WheelUp => "Mouse wheel up".into(),
        Trigger::WheelDown => "Mouse wheel down".into(),
        Trigger::Mouse(b) => match b {
            MouseButton::Left => "Left mouse".into(),
            MouseButton::Right => "Right mouse".into(),
            MouseButton::Middle => "Middle mouse".into(),
            MouseButton::Back => "Mouse back".into(),
            MouseButton::Forward => "Mouse forward".into(),
            MouseButton::Other(i) => format!("Mouse {i}"),
        },
        Trigger::Key(k) => {
            let name = format!("{k:?}");
            name.strip_prefix("Key")
                .or_else(|| name.strip_prefix("Digit"))
                .unwrap_or(&name)
                .into()
        }
    }
}

pub fn gamepad_name(button: Button, set: IconSet) -> String {
    crate::input_icons::pad_button(button).map_or_else(
        || format!("{button:?}"),
        |b| ui::input_icons::pad_name(set, b).to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;

    /// Each local player's context is read from their own unit: the first player in a
    /// vehicle leaves the second, on foot, on the infantry table.
    #[test]
    fn each_local_player_has_their_own_context() {
        let mut walk = crate::walk::WalkSim::new(7, glam::Vec3::ZERO, None);
        let second = walk.add_local_player(1, Some(1)).expect("slot 1 is free");
        for (u, slot) in [(walk.player, 0usize), (second, 1)] {
            walk.sim.units[u.index as usize].alive = true;
            walk.sim.spawn_stage[slot] = core::sim::SpawnStage::Accepted;
        }
        let v = walk.sim.spawn_vehicle(&core::vehicle::VehicleSpec {
            kind: core::vehicle::VehicleKind::Hover,
            class: 0,
            home_team: Some(0),
            neutral_when_empty: false,
            spawn_pad: None,
            pos: glam::Vec3::ZERO,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![core::vehicle::SeatKind::Pilot],
        });
        walk.sim.units[walk.player.index as usize].mount = Some(core::vehicle::Mount {
            vehicle: v,
            seat: 0,
        });
        assert_eq!(current_of(&mut walk, 0), Some(ControlContext::Vehicle));
        assert_eq!(current_of(&mut walk, 1), Some(ControlContext::Infantry));
        assert_eq!(current_of(&mut walk, 2), None, "no third seat");
        assert_eq!(
            current(&walk),
            Some(ControlContext::Vehicle),
            "the walk is put back"
        );
    }

    #[test]
    fn the_stock_contexts_fit_each_device_and_keep_their_actions() {
        let key = Bindings::default();
        let pad = input::Mapping::default();
        for context in [
            ControlContext::Infantry,
            ControlContext::Vehicle,
            ControlContext::Starfighter,
        ] {
            let keys: Option<Vec<_>> = keyboard_defaults(context, &key).into_iter().collect();
            let buttons: Option<Vec<_>> = gamepad_defaults(context, &pad).into_iter().collect();
            let buttons = buttons.expect("the stock pad can fill every action");
            assert!(buttons.len() <= 15);
            assert!(buttons
                .iter()
                .all(|button| crate::input_icons::pad_button(*button).is_some()));
            let keys = keyboard(context, &keys.expect("stock keyboard complete")).unwrap();
            let pad = gamepad(context, &buttons).unwrap();
            for bit in [b::FIRE, b::SECONDARY_FIRE, b::ACTION, b::JUMP] {
                assert!(keys.table.iter().any(|(_, bind)| bind.bits & bit != 0));
                assert!(pad.table.iter().any(|(_, bits)| bits & bit != 0));
            }
            if context == ControlContext::Infantry {
                for (button, bit) in [
                    (Button::DPadLeft, b::HERO_ACCEPT),
                    (Button::DPadRight, b::HERO_DECLINE),
                ] {
                    assert!(pad
                        .table
                        .iter()
                        .any(|&(key, bits)| key == button && bits & bit != 0));
                }
            }
        }
    }

    fn names(context: ControlContext, legacy: &Bindings) -> Vec<(&'static str, String)> {
        actions(context, true)
            .into_iter()
            .zip(keyboard_defaults(context, legacy))
            .map(|(a, t)| (a.name, t.map_or_else(|| "-".into(), keyboard_name)))
            .collect()
    }

    /// The stock keyboard as the Controls page shows it (the maintainer,
    /// 2026-11-14, after the LAN playtest): on foot Right mouse zooms, the
    /// wheel switches weapons and Left Ctrl rolls, and every shown key works
    /// on a three-button mouse; vehicles and starfighters keep theirs.
    #[test]
    fn the_controls_page_shows_the_stock_keyboard_per_context() {
        let stock = Bindings::default();
        let infantry = [
            ("Move forward", "W"),
            ("Move backward", "S"),
            ("Move left", "A"),
            ("Move right", "D"),
            ("Fire", "Left mouse"),
            ("Secondary fire", "Middle mouse"),
            ("Zoom", "Right mouse"),
            ("Jump", "Space"),
            ("Roll / Stance", "ControlLeft"),
            ("Sprint", "ShiftLeft"),
            ("Reload", "R"),
            ("Use / Enter vehicle", "E"),
            ("Next primary weapon", "Mouse wheel down"),
            ("Next secondary weapon", "G"),
            ("Squad move out", "ArrowUp"),
            ("Squad hold", "ArrowDown"),
            ("Squad at ease / Accept hero", "ArrowLeft"),
            ("Squad follow / Decline hero", "ArrowRight"),
            ("Viewpoint", "V"),
        ];
        let vehicle = [
            ("Drive forward", "W"),
            ("Drive backward", "S"),
            ("Strafe left", "A"),
            ("Strafe right", "D"),
            ("Fire", "Left mouse"),
            ("Secondary fire", "Right mouse"),
            ("Zoom", "Middle mouse"),
            ("Boost", "ShiftLeft"),
            ("Jump", "Space"),
            ("Exit vehicle", "E"),
            ("Viewpoint", "V"),
        ];
        let starfighter = [
            ("Increase throttle", "W"),
            ("Decrease throttle", "S"),
            ("Roll left", "A"),
            ("Roll right", "D"),
            ("Fire", "Left mouse"),
            ("Secondary fire", "Right mouse"),
            ("Zoom", "Middle mouse"),
            ("Boost", "ShiftLeft"),
            ("Take off / Land", "Space"),
            ("Exit starfighter", "E"),
            ("Viewpoint", "V"),
            ("Stunt", "AltLeft"),
            ("Reverse direction", "CapsLock"),
        ];
        for (context, want) in [
            (ControlContext::Infantry, &infantry[..]),
            (ControlContext::Vehicle, &vehicle[..]),
            (ControlContext::Starfighter, &starfighter[..]),
        ] {
            let got = names(context, &fallback(Some(context), &stock));
            let want: Vec<_> = want.iter().map(|&(a, k)| (a, k.to_owned())).collect();
            assert_eq!(got, want, "{context:?}");
        }
    }

    /// 2026-11-14: with the stock table, a vehicle's or a starfighter's
    /// mouse buttons press what they pressed before the new infantry
    /// defaults, and on foot (or on a spawn screen) the new ones play.
    #[test]
    fn the_stock_table_leaves_the_vehicle_mouse_buttons_as_they_were() {
        use crate::kbm::Kbm;
        use core::sim::PadState;
        use winit::event::MouseButton as M;
        let press = |context: Option<ControlContext>, t: Trigger| {
            let mut k = Kbm::default();
            k.set_bindings(&fallback(context, &Bindings::default()));
            k.press(t, true);
            k.pad(PadState::default()).buttons
        };
        for context in [ControlContext::Vehicle, ControlContext::Starfighter] {
            let c = Some(context);
            assert_eq!(press(c, Trigger::Mouse(M::Right)), b::SECONDARY_FIRE);
            assert_eq!(press(c, Trigger::Mouse(M::Middle)), b::ZOOM);
            assert_eq!(press(c, Trigger::Key(KeyCode::KeyZ)), b::ZOOM);
            assert_eq!(press(c, Trigger::Mouse(M::Back)), 0);
            assert_eq!(press(c, Trigger::Mouse(M::Forward)), 0);
            assert_eq!(fallback(c, &Bindings::default()), Bindings::pc_default_v0());
        }
        for c in [Some(ControlContext::Infantry), None] {
            assert_eq!(press(c, Trigger::Mouse(M::Right)), b::ZOOM);
            assert_eq!(press(c, Trigger::Mouse(M::Back)), b::SECONDARY_FIRE);
            assert_eq!(press(c, Trigger::Mouse(M::Middle)), b::SECONDARY_FIRE);
        }
        // A legacy table the player changed plays as saved in every context.
        let mut mine = Bindings::pc_default_v0();
        assert!(mine.remap(0, Trigger::Key(KeyCode::KeyP)));
        assert!(mine.is_valid() && Bindings::pc_default().is_valid());
        assert_eq!(fallback(Some(ControlContext::Vehicle), &mine), mine);
    }

    /// The layout pages' stock triggers (2026-11-14, the maintainer's new
    /// keyboard and mouse defaults after the LAN playtest): on foot the
    /// playtester's remap with Middle mouse and G in place of the thumb
    /// buttons; vehicles and starfighters keep theirs.
    #[test]
    fn the_stock_layouts_show_the_new_infantry_triggers_and_unchanged_vehicle_ones() {
        use winit::event::MouseButton as M;
        use KeyCode as K;
        use Trigger::{Key, Mouse};
        let stock = |context| -> Vec<Trigger> {
            keyboard_defaults(context, &Bindings::default())
                .into_iter()
                .map(|t| t.expect("every action has a stock trigger"))
                .collect()
        };
        let wasd = [Key(K::KeyW), Key(K::KeyS), Key(K::KeyA), Key(K::KeyD)];
        let infantry = [
            Mouse(M::Left),
            Mouse(M::Middle),
            Mouse(M::Right),
            Key(K::Space),
            Key(K::ControlLeft),
            Key(K::ShiftLeft),
            Key(K::KeyR),
            Key(K::KeyE),
            Trigger::WheelDown,
            Key(K::KeyG),
            Key(K::ArrowUp),
            Key(K::ArrowDown),
            Key(K::ArrowLeft),
            Key(K::ArrowRight),
            Key(K::KeyV),
        ];
        assert_eq!(
            stock(ControlContext::Infantry),
            [&wasd[..], &infantry].concat()
        );
        let vehicle = [
            Mouse(M::Left),
            Mouse(M::Right),
            Mouse(M::Middle),
            Key(K::ShiftLeft),
            Key(K::Space),
            Key(K::KeyE),
            Key(K::KeyV),
        ];
        assert_eq!(
            stock(ControlContext::Vehicle),
            [&wasd[..], &vehicle].concat()
        );
        let flight = [Key(K::AltLeft), Key(K::CapsLock)];
        assert_eq!(
            stock(ControlContext::Starfighter),
            [&wasd[..], &vehicle, &flight].concat()
        );
    }

    /// What `App::refresh_context_bindings` installs per context, and the
    /// look factor with it: the stock profile plays the new table on foot and
    /// the old one in vehicles and starfighters; a saved infantry layout plays
    /// on foot only; a settings change (`Kbm::follow`) leaves a vehicle's
    /// table alone.
    #[test]
    fn each_context_plays_its_table_and_look_factor() {
        use crate::kbm::{Kbm, ON_FOOT_LOOK_SCALE};
        use crate::profile::Settings;
        use winit::event::MouseButton as M;
        let stock = Settings::default();
        assert_eq!(
            context_bindings(Some(ControlContext::Infantry), &stock),
            Bindings::pc_default()
        );
        assert_eq!(context_bindings(None, &stock), Bindings::pc_default());
        for c in [ControlContext::Vehicle, ControlContext::Starfighter] {
            assert_eq!(context_bindings(Some(c), &stock), Bindings::pc_default_v0());
            assert_eq!(look_scale(Some(c)), 1.0);
        }
        assert_eq!(
            look_scale(Some(ControlContext::Infantry)),
            ON_FOOT_LOOK_SCALE
        );
        assert_eq!(look_scale(None), ON_FOOT_LOOK_SCALE);

        let mut mine = stock.clone();
        let mut layout: Vec<Trigger> =
            keyboard_defaults(ControlContext::Infantry, &Bindings::default())
                .into_iter()
                .flatten()
                .collect();
        layout[5] = Trigger::Mouse(M::Back);
        mine.keyboard_layouts
            .set(ControlContext::Infantry, layout.clone());
        let on_foot = context_bindings(Some(ControlContext::Infantry), &mine);
        assert_eq!(
            on_foot,
            keyboard(ControlContext::Infantry, &layout).unwrap()
        );
        assert_eq!(
            context_bindings(Some(ControlContext::Vehicle), &mine),
            Bindings::pc_default_v0()
        );

        let mut k = Kbm::default();
        let c = Some(ControlContext::Vehicle);
        k.swap_context(&context_bindings(c, &mine), look_scale(c));
        k.follow(&mine);
        assert_eq!(k.bindings, Bindings::pc_default_v0());
        assert_eq!(k.look_scale, 1.0);
    }

    #[test]
    fn a_remapped_hero_key_keeps_its_action_in_infantry() {
        let key = Bindings::default();
        let mut keys: Vec<_> = keyboard_defaults(ControlContext::Infantry, &key)
            .into_iter()
            .flatten()
            .collect();
        let row = actions(ControlContext::Infantry, true)
            .iter()
            .position(|a| a.bind.bits & b::HERO_ACCEPT != 0)
            .unwrap();
        keys[row] = Trigger::Key(KeyCode::KeyY);
        let mut layouts = Layouts::default();
        layouts.set(ControlContext::Infantry, keys);
        assert!(layouts.get(ControlContext::Vehicle).is_empty());
        let mapped = keyboard(
            ControlContext::Infantry,
            layouts.get(ControlContext::Infantry),
        )
        .unwrap();
        assert!(mapped.table.iter().any(|&(key, bind)| {
            key == Trigger::Key(KeyCode::KeyY) && bind.bits & b::HERO_ACCEPT != 0
        }));
        assert!(mapped
            .table
            .iter()
            .all(|(_, bind)| bind.bits & b::SPAWN_ACCEPT == 0));
    }
}
