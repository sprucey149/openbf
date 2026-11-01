use super::*;

#[derive(Debug, Clone)]
pub(super) struct ComboPose {
    name: String,
    upper: Option<Slot>,
    lower: Option<Slot>,
    seconds: f32,
    force: bool,
    legs: bool,
    transition: [Option<f32>; 2],
}

impl ComboPose {
    pub(super) fn forced(&self) -> bool {
        self.force
    }
}

impl Animator {
    pub fn hero_jump(&mut self, jump: Option<(u8, u32)>) {
        self.hero_jump = jump;
    }

    pub fn status_pose(
        &mut self,
        clips: &[Clip],
        skel: &Skeleton,
        family: &str,
        seconds: f32,
    ) -> bool {
        let own = format!("{family}_choking");
        // BF1 soldier assets lack authored `human_choking` clips because BF1 had no
        // playable heroes or Force powers. Fall back to airborne struggling (`flail`)
        // so victims kick and struggle in the hold rather than freezing motionless.
        let clip = clips
            .iter()
            .find(|c| {
                [own.as_str(), "human_choking"].iter().any(|name| {
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
        let bank = class.bank_of(weapon);
        let bank = bank.split_once('_').map_or(bank, |(_, b)| b);
        let slot = |layer: &str| {
            let clip = crate::hero_blades::clip_for(clips, &class.family, bank, animation, layer)?;
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
        self.combo = Some(ComboPose {
            name: animation.into(),
            upper,
            lower,
            seconds,
            force,
            legs,
            transition,
        });
        true
    }

    pub fn clear_combo_pose(&mut self) {
        self.combo = None;
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
            let speed = rig.root[lower.clip].unwrap_or([0.0; 2]);
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
        let Some(combo) = &self.combo else { return };
        for (player, active) in [(&mut self.upper, upper), (&mut self.lower, lower)] {
            if active {
                let Some(slot) = player.slot else { continue };
                let phase =
                    combo.seconds * 30.0 / clips[slot.clip].frames.saturating_sub(1).max(1) as f32;
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
