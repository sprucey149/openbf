use super::*;
use core::fmath::FloatExt;

#[derive(Debug, Clone)]
pub(super) struct ComboPose {
    name: String,
    upper: Option<Slot>,
    lower: Option<Slot>,
    seconds: f32,
    force: bool,
    restart: bool,
    legs: bool,
    transition: [Option<f32>; 2],
}

impl ComboPose {
    pub(super) fn forced(&self) -> bool {
        self.force
    }

    pub(super) fn restarting(&self) -> bool {
        self.restart
    }
}

/// Force Choke's hold and its end (issue #173). The hold itself is drawn by
/// [`Animator::status_pose`]; when it ends, the victim drops back to his feet
/// with `landhard`. BF2 sends a soldier leaving the choke state (0xe) for
/// stand, crouch or sprint to simple clip 7, `landhard` (`FUN_0027bae0`
/// B2:200286-200343); BF1 lends the hold, and its own `landhard` closes it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ChokeRelease {
    /// This update holds him ([`Animator::choke_hold`]).
    held: bool,
    /// The last update held him.
    was_held: bool,
    /// The landing after a hold: (seconds played, its clip's length).
    landing: Option<(f32, f32)>,
}

/// A release landing ends early once he moves faster than this (m/s).
/// PROVISIONAL: the stand-up out of the choke state locks the jump, crouch,
/// sprint and the triggers for 0.5 s but leaves walking free (B2
/// `FUN_00246850`, flags 0x1c00; the sim's `Soldier::choke_release_lock`),
/// so a victim can walk off during the landing and is let go to his run here.
/// measure: the PS2 animator's end of simple clip 7 while the soldier walks
/// off under the release lock.
const RELEASE_INTERRUPT_SPEED: f32 = 1.0;

impl Animator {
    pub fn hero_jump(&mut self, jump: Option<(u8, u32)>) {
        self.hero_jump = jump;
    }

    /// Whether Force Choke holds this soldier this update. Set before
    /// [`Animator::step`]; the hold's pose itself is [`Animator::status_pose`].
    pub fn choke_hold(&mut self, held: bool) {
        self.choke.held = held;
    }

    /// The hard landing that ends a choke: from the first update after the
    /// hold, while he is on his feet (stand, crouch or a BF2 sprint), for
    /// the length of his set's `landhard` (BF1 frames / 30, BF2
    /// (frames - 1) / 30). Changing the body to it crossfades the held
    /// pose into the landing over that clip's blend-in.
    pub(super) fn choke_release(
        &mut self,
        rig: &Rig,
        clips: &[Clip],
        i: &AnimInput,
        stance: Result<usize, Special>,
        dt: f32,
    ) -> Result<usize, Special> {
        let c = &mut self.choke;
        let released = c.was_held && !c.held;
        c.was_held = c.held;
        if c.held {
            c.landing = None;
            return stance;
        }
        let on_feet = matches!(stance, Ok(0 | 1) | Err(Special::Sprint));
        if released && on_feet {
            let lower = rig
                .body_slots(i.set, Body::Special(Special::Landing(true)), false)
                .1;
            c.landing = lower.and_then(|slot| {
                let frames = clips.get(slot.clip)?.frames as f32;
                let length = match rig.game {
                    Game::Bf1 => frames / 30.0,
                    Game::Bf2 => (frames - 1.0) / 30.0,
                };
                (length > 0.0).then_some((0.0, length))
            });
        }
        match &mut c.landing {
            Some((played, length))
                if on_feet && *played < *length && i.speed() <= RELEASE_INTERRUPT_SPEED =>
            {
                *played += dt.fmax(0.0);
                Err(Special::Landing(true))
            }
            _ => {
                c.landing = None;
                stance
            }
        }
    }

    /// The choke victim's pose, `seconds` into the hold: the whole body
    /// from one clip, looped at 30 frames a second over (frames - 1) / 30,
    /// with its root translation kept. BF2 draws a soldier in the choke state
    /// (0xe) from the class's named clip on both players and overwrites the
    /// pose with it, with no blend and no overlay (`FUN_0027caf0`
    /// B2:200931-200955); `human_choking` raises `bone_root` 0.3 m and more
    /// above the standing clips', which is the lift.
    pub fn status_pose(
        &mut self,
        clips: &[Clip],
        skel: &Skeleton,
        family: &str,
        seconds: f32,
    ) -> bool {
        let own = format!("{family}_choking");
        // The family's own clip first, whatever the clips' order, then BF2's
        // `human_choking` (the bake lends it to BF1 soldiers). A bake older
        // than that lend has neither on a BF1 body: the thrown flail loop
        // stands in, so the victim struggles rather than freezes.
        let clip = [own.as_str(), "human_choking"]
            .iter()
            .find_map(|name| {
                clips.iter().find(|c| {
                    c.name.eq_ignore_ascii_case(name)
                        || c.name.eq_ignore_ascii_case(&format!("{name}_full"))
                })
            })
            .or_else(|| clips.iter().find(|c| c.name.contains("flail")));
        let Some(clip) = clip else {
            return false;
        };
        let period = clip.frames.saturating_sub(1).max(1) as f32;
        self.kept = Some(clip.sample(skel, (seconds * 30.0).rem_euclid(period), false));
        self.combo_override = true;
        self.combo_aim = false;
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn combo_pose(
        &mut self,
        clips: &[Clip],
        class: &ClassAnim,
        weapon: &str,
        animation: &str,
        seconds: f32,
        looping: bool,
        force: bool,
    ) -> bool {
        let (primary, fallback) = class.bank_pair_of(weapon);
        let banks: [&str; 2] = [primary, fallback.unwrap_or(primary)];
        let banks = if fallback.is_some() { &banks[..] } else { &banks[..1] };
        let slot = |layer: &str| {
            let clip =
                crate::hero_blades::clip_for_banks(clips, &class.family, banks, animation, layer)?;
            let descriptor = if layer == "upper" {
                class.custom.upper(animation)
            } else {
                class.custom.lower(animation)
            };
            Some(Slot {
                clip: clips.iter().position(|c| std::ptr::eq(c, clip))?,
                looping: descriptor.looping.unwrap_or(looping),
                blend_in: descriptor.blend_in,
                blend_out: descriptor.blend_out,
                aim: descriptor.aim,
                pair: None,
            })
        };
        let (upper, lower) = (slot("upper"), slot("lower"));
        if upper.is_none() && lower.is_none() {
            self.combo = None;
            return false;
        }
        let legs = self
            .combo
            .as_ref()
            .filter(|old| {
                old.lower.map(|s| s.clip) == lower.map(|s| s.clip) && seconds >= old.seconds
            })
            .is_none_or(|old| old.legs);
        let transition = [true, false].map(|upper| {
            self.combo
                .as_ref()
                .and_then(|old| class.custom.blend_to(&old.name, animation, upper))
        });
        let restart = self.combo.as_ref().is_some_and(|old| seconds < old.seconds);
        self.combo = Some(ComboPose {
            name: animation.into(),
            upper,
            lower,
            seconds,
            force,
            restart,
            legs,
            transition,
        });
        true
    }

    pub fn clear_combo_pose(&mut self) {
        self.combo = None;
    }

    /// A combo pose naming a clip at or past `clips` (kept from another
    /// body, issue #168) is no combo pose.
    pub(super) fn drop_stale_combo(&mut self, clips: usize) {
        if self.combo.as_ref().is_some_and(|c| {
            [c.upper, c.lower]
                .into_iter()
                .flatten()
                .any(|s| s.clip >= clips)
        }) {
            self.combo = None;
        }
    }

    pub(super) fn combo_slots(
        &mut self,
        rig: &Rig,
        input: &AnimInput,
    ) -> (Option<Slot>, Option<Slot>) {
        let Some(combo) = &mut self.combo else {
            return (None, None);
        };
        if let Some(lower) = combo.lower {
            // A slot from another body has no root speed here.
            let speed = rig
                .root
                .get(lower.clip)
                .copied()
                .flatten()
                .unwrap_or([0.0; 2]);
            let scale = if input.root_scale > 0.0 {
                input.root_scale
            } else {
                1.0
            };
            let residual = input.vel / scale - glam::Vec2::from_array(speed);
            combo.legs &= residual.length_squared() <= 0.09;
        }
        (
            combo.upper,
            combo.lower.filter(|_| combo.force || combo.legs),
        )
    }

    pub(super) fn combo_clock(&mut self, clips: &[Clip], upper: bool, lower: bool) {
        let Some(combo) = &mut self.combo else { return };
        combo.restart = false;
        for (player, active) in [(&mut self.upper, upper), (&mut self.lower, lower)] {
            if active {
                let Some(slot) = player.slot else { continue };
                let Some(c) = clips.get(slot.clip) else {
                    continue;
                };
                let phase = combo.seconds * 30.0 / c.frames.saturating_sub(1).max(1) as f32;
                player.t = if slot.looping {
                    phase.rem_euclid(1.0)
                } else {
                    phase.clamp(0.0, 1.0)
                };
            }
        }
    }

    pub(super) fn combo_transition(&mut self, upper: bool, old: Option<usize>) {
        let player = if upper {
            &mut self.upper
        } else {
            &mut self.lower
        };
        if old != player.slot.map(|s| s.clip) {
            if let Some(time) = self
                .combo
                .as_ref()
                .and_then(|c| c.transition[usize::from(!upper)])
            {
                player.blend = time;
            }
        }
    }
}
