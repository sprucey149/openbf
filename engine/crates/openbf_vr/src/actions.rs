//! OpenXR action bindings and independently tracked aim and grip poses.

use anyhow::Result;
use glam::{Quat, Vec2, Vec3};
use openxr as xr;

type Session = xr::Session<xr::Vulkan>;

const SWAPS: u32 = core::sim::buttons::SWITCH_WEAPON | core::sim::buttons::SWITCH_SECONDARY;
/// OURS: how long one weapon-swap button waits for the other before it counts
/// alone. measure: press both together, and swap and block, in a headset.
const CHORD_NS: i64 = 150_000_000;

/// The two weapon-swap buttons held together open the game menu. Either alone
/// waits [`CHORD_NS`] for the other, so the pair swaps nothing; a shorter tap
/// counts as it is let go.
#[derive(Clone, Copy, Default)]
struct Chord {
    since: Option<i64>,
    waiting: u32,
    together: bool,
}

impl Chord {
    /// The swap buttons play sees for `held` at `now`, and whether the pair
    /// is down.
    fn filter(&mut self, held: u32, now: i64) -> (u32, bool) {
        if held == SWAPS {
            *self = Self {
                together: true,
                ..Self::default()
            };
            return (0, true);
        }
        if held == 0 {
            let tapped = self.waiting;
            *self = Self::default();
            return (tapped, false);
        }
        if self.together {
            return (0, false);
        }
        let since = *self.since.get_or_insert(now);
        if now.saturating_sub(since) < CHORD_NS {
            self.waiting = held;
            (0, false)
        } else {
            self.waiting = 0;
            (held, false)
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub position: Vec3,
    pub orientation: Quat,
}

impl Pose {
    pub fn from_xr(pose: xr::Posef) -> Self {
        let p = pose.position;
        let q = pose.orientation;
        Self {
            position: Vec3::new(p.x, p.y, p.z),
            orientation: Quat::from_xyzw(q.x, q.y, q.z, q.w).normalize(),
        }
    }

    pub fn locate(
        space: &xr::Space,
        base: &xr::Space,
        time: xr::Time,
        required: xr::SpaceLocationFlags,
    ) -> Result<Option<Self>> {
        Ok(Self::from_location(space.locate(base, time)?, required))
    }

    fn from_location(sample: xr::SpaceLocation, required: xr::SpaceLocationFlags) -> Option<Self> {
        let valid = xr::SpaceLocationFlags::POSITION_VALID
            | xr::SpaceLocationFlags::ORIENTATION_VALID
            | required;
        sample
            .location_flags
            .contains(valid)
            .then(|| Self::from_xr(sample.pose))
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Hand {
    pub aim: Option<Pose>,
    pub grip: Option<Pose>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Input {
    pub hands: [Hand; 2],
    pub movement: Option<Vec2>,
    pub right_stick: Vec2,
    pub buttons: u32,
    pub pause: bool,
    pub stick_click: bool,
    pub support_grip: bool,
    pub left_velocity: Option<Vec3>,
}

impl Input {
    pub fn turn(self) -> f32 {
        let Vec2 { x, y } = self.right_stick;
        if x.abs() >= y.abs() {
            x
        } else {
            0.0
        }
    }

    pub fn battle_buttons(self) -> u32 {
        // Use a downward flick to reload without taking either grip button.
        let Vec2 { x, y } = self.right_stick;
        let reload = y < -0.7 && y < -x.abs();
        self.buttons | u32::from(reload) * core::sim::buttons::RELOAD
    }
}

pub(super) struct Actions {
    set: xr::ActionSet,
    hands: [xr::Path; 2],
    aim: xr::Action<xr::Posef>,
    grip: xr::Action<xr::Posef>,
    move_stick: xr::Action<xr::Vector2f>,
    turn_stick: xr::Action<xr::Vector2f>,
    fire: xr::Action<bool>,
    secondary: xr::Action<bool>,
    reload: xr::Action<bool>,
    jump: xr::Action<bool>,
    interact: xr::Action<bool>,
    switch: xr::Action<bool>,
    switch_secondary: xr::Action<bool>,
    sprint: xr::Action<bool>,
    pause: xr::Action<bool>,
    stick_click: xr::Action<bool>,
    support_grip: xr::Action<bool>,
    haptic: xr::Action<xr::Haptic>,
    chord: std::cell::Cell<Chord>,
}

impl Actions {
    pub fn new(instance: &xr::Instance, session: &Session) -> Result<Self> {
        let hands = [
            instance.string_to_path("/user/hand/left")?,
            instance.string_to_path("/user/hand/right")?,
        ];
        let set = instance.create_action_set("battle", "Battle controls", 0)?;
        let actions = Self {
            hands,
            aim: set.create_action("aim", "Aim pose", &hands)?,
            grip: set.create_action("grip", "Grip pose", &hands)?,
            move_stick: set.create_action("move", "Move", &[])?,
            turn_stick: set.create_action("turn", "Snap turn", &[])?,
            fire: set.create_action("fire", "Fire", &[])?,
            secondary: set.create_action("secondary", "Secondary fire", &[])?,
            reload: set.create_action("reload", "Reload", &[])?,
            jump: set.create_action("jump", "Jump", &[])?,
            interact: set.create_action("interact", "Use or interact", &[])?,
            switch: set.create_action("switch", "Switch primary weapon", &[])?,
            switch_secondary: set.create_action(
                "switch_secondary",
                "Switch secondary weapon",
                &[],
            )?,
            sprint: set.create_action("sprint", "Sprint", &[])?,
            pause: set.create_action("pause", "Open or close the game menu", &[])?,
            stick_click: set.create_action("stick_click", "Answer a hero offer", &[])?,
            support_grip: set.create_action(
                "support_grip",
                "Grip the weapon with the left hand",
                &[],
            )?,
            haptic: set.create_action("saber_contact", "Saber contact vibration", &hands)?,
            set,
            chord: Default::default(),
        };
        // All profiles have aim and grip; the simple profile also lets a
        // tracked device with only a select button fire the primary weapon.
        actions.suggest(
            instance,
            "khr/simple_controller",
            None,
            "select/click",
            None,
            None,
            false,
            None,
        )?;
        actions.suggest(
            instance,
            "oculus/touch_controller",
            Some("thumbstick"),
            "trigger/value",
            None,
            Some(["x/click", "y/click"]),
            true,
            Some("squeeze/value"),
        )?;
        actions.suggest(
            instance,
            "valve/index_controller",
            Some("thumbstick"),
            "trigger/value",
            None,
            Some(["a/click", "b/click"]),
            true,
            Some("squeeze/value"),
        )?;
        actions.suggest(
            instance,
            "microsoft/motion_controller",
            Some("thumbstick"),
            "trigger/value",
            Some("menu/click"),
            None,
            true,
            Some("squeeze/click"),
        )?;
        actions.suggest(
            instance,
            "htc/vive_controller",
            None,
            "trigger/click",
            Some("menu/click"),
            None,
            true,
            Some("squeeze/click"),
        )?;
        session.attach_action_sets(&[&actions.set])?;
        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    fn suggest(
        &self,
        instance: &xr::Instance,
        profile: &str,
        stick: Option<&str>,
        trigger: &str,
        reload: Option<&str>,
        left_face: Option<[&str; 2]>,
        secondary: bool,
        support_grip: Option<&str>,
    ) -> Result<()> {
        let profile = instance.string_to_path(&format!("/interaction_profiles/{profile}"))?;
        let path = |side, component| {
            instance.string_to_path(&format!("/user/hand/{side}/input/{component}"))
        };
        let mut bindings = vec![
            xr::Binding::new(&self.aim, path("left", "aim/pose")?),
            xr::Binding::new(&self.aim, path("right", "aim/pose")?),
            xr::Binding::new(&self.grip, path("left", "grip/pose")?),
            xr::Binding::new(&self.grip, path("right", "grip/pose")?),
            xr::Binding::new(&self.fire, path("right", trigger)?),
            xr::Binding::new(
                &self.haptic,
                instance.string_to_path("/user/hand/left/output/haptic")?,
            ),
            xr::Binding::new(
                &self.haptic,
                instance.string_to_path("/user/hand/right/output/haptic")?,
            ),
        ];
        if secondary {
            bindings.push(xr::Binding::new(&self.secondary, path("left", trigger)?));
        }
        if let Some(squeeze) = support_grip {
            bindings.push(xr::Binding::new(&self.support_grip, path("left", squeeze)?));
        }
        if let Some(reload) = reload {
            bindings.push(xr::Binding::new(&self.reload, path("right", reload)?));
        }
        if let Some([interact, secondary]) = left_face {
            bindings.extend([
                xr::Binding::new(&self.jump, path("right", "a/click")?),
                xr::Binding::new(&self.switch, path("right", "b/click")?),
                xr::Binding::new(&self.interact, path("left", interact)?),
                xr::Binding::new(&self.switch_secondary, path("left", secondary)?),
            ]);
        }
        if let Some(stick) = stick {
            bindings.push(xr::Binding::new(&self.move_stick, path("left", stick)?));
            bindings.push(xr::Binding::new(&self.turn_stick, path("right", stick)?));
            bindings.push(xr::Binding::new(
                &self.sprint,
                path("left", "thumbstick/click")?,
            ));
            bindings.push(xr::Binding::new(
                &self.stick_click,
                path("right", "thumbstick/click")?,
            ));
        }
        // With two swap buttons the pair opens the menu ([`Chord`]).
        if left_face.is_none() {
            bindings.push(xr::Binding::new(&self.pause, path("left", "menu/click")?));
        }
        match instance.suggest_interaction_profile_bindings(profile, &bindings) {
            // Runtimes may implement only some standard interaction profiles.
            Ok(()) | Err(xr::sys::Result::ERROR_PATH_UNSUPPORTED) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn spaces(&self, session: &Session) -> Result<[[xr::Space; 2]; 2]> {
        let hand = |side| -> Result<_> {
            Ok([
                self.aim.create_space(session, side, xr::Posef::IDENTITY)?,
                self.grip.create_space(session, side, xr::Posef::IDENTITY)?,
            ])
        };
        Ok([hand(self.hands[0])?, hand(self.hands[1])?])
    }

    pub fn saber_pulse(&self, session: &Session, hand: usize, blocked: bool, strength: f32) {
        // OURS: a cut has a longer, stronger pulse than a block.
        // measure: distinguish the pulses on Touch, Index and Vive controllers.
        let (nanos, gain) = if blocked {
            (28_000_000, 0.45)
        } else {
            (60_000_000, 0.9)
        };
        let pulse = xr::HapticVibration::new()
            .duration(xr::Duration::from_nanos(nanos))
            .frequency(xr::FREQUENCY_UNSPECIFIED)
            .amplitude((strength * gain).clamp(0.0, 1.0));
        let _ = self
            .haptic
            .apply_feedback(session, self.hands[hand], &pulse);
    }

    pub fn sample(
        &self,
        session: &Session,
        spaces: &[[xr::Space; 2]; 2],
        base: &xr::Space,
        time: xr::Time,
    ) -> Result<Input> {
        use core::sim::buttons;
        session.sync_actions(&[xr::ActiveActionSet::new(&self.set)])?;
        let mut hands = [Hand::default(); 2];
        let mut left_velocity = None;
        for (index, ((hand, side), [aim, grip])) in
            hands.iter_mut().zip(self.hands).zip(spaces).enumerate()
        {
            if self.aim.is_active(session, side)? {
                hand.aim =
                    Pose::locate(aim, base, time, xr::SpaceLocationFlags::ORIENTATION_TRACKED)?;
            }
            if self.grip.is_active(session, side)? {
                let required = xr::SpaceLocationFlags::ORIENTATION_TRACKED
                    | xr::SpaceLocationFlags::POSITION_TRACKED;
                if index == 0 {
                    let (location, motion) = grip.relate(base, time)?;
                    hand.grip = Pose::from_location(location, required);
                    if motion
                        .velocity_flags
                        .contains(xr::SpaceVelocityFlags::LINEAR_VALID)
                    {
                        let v = motion.linear_velocity;
                        let v = Vec3::new(v.x, v.y, v.z);
                        left_velocity = v.is_finite().then_some(v);
                    }
                } else {
                    hand.grip = Pose::locate(grip, base, time, required)?;
                }
            }
        }
        let stick = |action: &xr::Action<xr::Vector2f>| -> Result<Option<Vec2>> {
            let state = action.state(session, xr::Path::NULL)?;
            Ok(state
                .is_active
                .then(|| Vec2::new(state.current_state.x, state.current_state.y)))
        };
        let mut buttons = 0;
        for (action, bit) in [
            (&self.fire, buttons::FIRE),
            (&self.secondary, buttons::SECONDARY_FIRE),
            (&self.reload, buttons::RELOAD),
            (&self.jump, buttons::JUMP),
            (&self.interact, buttons::ACTION),
            (&self.switch, buttons::SWITCH_WEAPON),
            (&self.switch_secondary, buttons::SWITCH_SECONDARY),
            (&self.sprint, buttons::SPRINT),
        ] {
            let state = action.state(session, xr::Path::NULL)?;
            if state.is_active && state.current_state {
                buttons |= bit;
            }
        }
        let pause = self.pause.state(session, xr::Path::NULL)?;
        let support_grip = self.support_grip.state(session, xr::Path::NULL)?;
        let stick_click = self.stick_click.state(session, xr::Path::NULL)?;
        let mut chord = self.chord.get();
        let (swaps, paired) = chord.filter(buttons & SWAPS, time.as_nanos());
        self.chord.set(chord);
        buttons = buttons & !SWAPS | swaps;
        Ok(Input {
            hands,
            movement: stick(&self.move_stick)?,
            right_stick: stick(&self.turn_stick)?.unwrap_or_default(),
            buttons,
            pause: pause.is_active && pause.current_state || paired,
            stick_click: stick_click.is_active && stick_click.current_state,
            support_grip: support_grip.is_active && support_grip.current_state,
            left_velocity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sim::buttons::{SWITCH_SECONDARY as Y, SWITCH_WEAPON as B};

    #[test]
    fn the_swap_pair_pauses_without_swapping_and_either_alone_still_swaps() {
        let mut chord = Chord::default();
        assert_eq!(chord.filter(B, 0), (0, false));
        assert_eq!(chord.filter(B | Y, 1), (0, true));
        assert_eq!(chord.filter(Y, 2), (0, false));
        assert_eq!(chord.filter(Y, CHORD_NS * 3), (0, false));
        assert_eq!(chord.filter(0, CHORD_NS * 3), (0, false));

        assert_eq!(chord.filter(B, 0), (0, false));
        assert_eq!(chord.filter(0, 1), (B, false));
        assert_eq!(chord.filter(0, 2), (0, false));

        assert_eq!(chord.filter(Y, 0), (0, false));
        assert_eq!(chord.filter(Y, CHORD_NS), (Y, false));
        assert_eq!(chord.filter(0, CHORD_NS + 1), (0, false));
    }
}
