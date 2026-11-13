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

pub fn keyboard_defaults(context: ControlContext, legacy: &Bindings) -> Vec<Option<Trigger>> {
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
