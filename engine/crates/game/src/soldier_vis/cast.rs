//! One-shot SoldierAnimation poses use the authored BF2 clip length.

use crate::{
    animator::{aim::AimKind, ClassAnim},
    skin::Clip,
};
use core::fmath::FloatExt;
use glam::Vec2;

pub(super) fn animation<'a>(
    power: &'a core::weapon::WeaponClass,
    primary: &'a core::weapon::WeaponClass,
) -> Option<&'a str> {
    // A melee throw plays the melee program's ThrowAnimation (the weapon's
    // getter 0x33c458; the release is InitialSalvoDelay into it, 0x3450c0).
    if power.saber_throw.is_some() {
        return primary
            .saber
            .as_ref()?
            .combo
            .fields
            .iter()
            .rfind(|field| field.keyword == manifest::combo::ComboKeyword::ThrowAnimation)?
            .text(0);
    }
    // Area-effect SoldierAnimation names the victim pose (0x32d198),
    // while the caster uses the melee program's offhand fire animation.
    let own = match power.force.as_ref() {
        Some(force) => matches!(force.kind, core::hero_weapon::ForceKind::Emitter(_))
            .then_some(force.animation.as_deref())
            .flatten(),
        None if offhand_projectile(power, primary) => None,
        None => return None,
    };
    own.or_else(|| {
        // PS2 0x309d10 maps OffhandAnimation FIRE to the first fire slot.
        primary
            .saber
            .as_ref()?
            .combo
            .fields
            .iter()
            .find(|field| {
                field.keyword == manifest::combo::ComboKeyword::OffhandAnimation
                    && field
                        .text(1)
                        .is_some_and(|s| s.eq_ignore_ascii_case("FIRE"))
            })?
            .text(0)
    })
}

/// A melee hero's offhand power that throws a modelled round: Kit Fisto's
/// Force ball, a cannon (`force_pull`'s child) firing a missile. The soldier
/// animator takes the melee program's offhand FIRE pose for whatever offhand
/// weapon fires (getter 0x33c438, called by 0x239bb0, B2:153915), not only
/// for the emitter powers, so the ball's throw is Kit's `stand_useforce`
/// (`fisto_sabre_stand_useforce_full`, 40 frames), started by its shot.
pub(super) fn offhand_projectile(
    power: &core::weapon::WeaponClass,
    primary: &core::weapon::WeaponClass,
) -> bool {
    power.force.is_none()
        && power.saber.is_none()
        && power.saber_throw.is_none()
        && power.ordnance.missile.is_some()
        && primary.saber.is_some()
}

#[derive(Clone, Default)]
pub(super) struct CastClock {
    seen: Option<u64>,
    firing: Option<usize>,
    active: Option<(usize, f32)>,
    legs: Legs,
}

#[derive(Clone, Default)]
struct Legs {
    animation: Option<String>,
    joined: bool,
}

impl Legs {
    fn update(&mut self, name: &str, velocity: Vec2, root: Option<Vec2>) -> bool {
        if self.animation.as_deref() != Some(name) {
            self.animation = Some(name.into());
            self.joined = true;
        }
        // Native named lower action: 0x27dfe0, B2:201866-201881.
        self.joined &= root.is_some_and(|r| (velocity - r).length_squared() <= 0.09);
        self.joined
    }
}

pub(super) struct Frame<'a> {
    pub animation: &'a str,
    pub seconds: f32,
    pub legs: bool,
    pub aim: (AimKind, f32),
}

impl CastClock {
    pub fn firing(&mut self, tick: u64, weapon: Option<usize>) {
        if let Some(weapon) = weapon.filter(|w| self.firing != Some(*w)) {
            self.clear();
            self.seen = None;
            self.start(tick, weapon);
        }
        self.firing = weapon;
    }

    pub fn start(&mut self, tick: u64, weapon: usize) {
        if self.seen != Some(tick) {
            self.seen = Some(tick);
            if self.active.is_none_or(|(old, _)| old != weapon) {
                self.active = Some((weapon, 0.0));
            }
        }
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.legs = Legs::default();
    }

    pub fn advance(&mut self, dt: f32) {
        if let Some((_, age)) = &mut self.active {
            *age += dt;
        }
    }

    pub fn pose(
        &mut self,
        clips: &[Clip],
        class: &ClassAnim,
        weapon: &str,
        animation: &str,
        holding: bool,
    ) -> Option<(usize, f32)> {
        let (source, age) = self.active?;
        let (primary, fallback) = class.bank_pair_of(weapon);
        let banks: [&str; 2] = [primary, fallback.unwrap_or(primary)];
        let banks = if fallback.is_some() {
            &banks[..]
        } else {
            &banks[..1]
        };
        let end = ["upper", "lower"]
            .into_iter()
            .filter_map(|layer| {
                crate::hero_blades::clip_for_banks(clips, &class.family, banks, animation, layer)
            })
            .map(|clip| clip.frames.saturating_sub(1) as f32 / 30.0)
            .reduce(core::fmath::max)?;
        if age <= end || holding && class.custom.upper(animation).final_frame {
            Some((source, age.fmin(end)))
        } else {
            self.clear();
            None
        }
    }

    /// The frame's animation and clock with no state change (tests).
    #[cfg(test)]
    pub fn clone_frame(
        &mut self,
        u: &core::unit::Unit,
        weapons: &[core::weapon::WeaponClass],
        clips: &[Clip],
        class: &ClassAnim,
    ) -> Option<(String, f32)> {
        self.frame_inner(u, weapons, clips, class)
            .map(|f| (f.animation.to_owned(), f.seconds))
    }

    pub fn weapon(&self) -> Option<usize> {
        self.active.map(|(weapon, _)| weapon)
    }

    pub fn frame<'a>(
        &mut self,
        u: &core::unit::Unit,
        weapons: &'a [core::weapon::WeaponClass],
        clips: &[Clip],
        class: &ClassAnim,
    ) -> Option<Frame<'a>> {
        let frame = self.frame_inner(u, weapons, clips, class);
        if frame.is_none() {
            self.legs = Legs::default();
        }
        frame
    }

    fn frame_inner<'a>(
        &mut self,
        u: &core::unit::Unit,
        weapons: &'a [core::weapon::WeaponClass],
        clips: &[Clip],
        class: &ClassAnim,
    ) -> Option<Frame<'a>> {
        if !u.alive || u.mount.is_some() {
            self.clear();
            return None;
        }
        let primary = &weapons[u.weapon_class];
        let active = u.active_weapon();
        let held = active
            .2
            .force_channel
            .as_ref()
            .filter(|_| active.2.state == core::weapon::FireState::Firing);
        let (power, seconds, channel) = match held {
            Some(channel) => (&weapons[active.0], channel.elapsed, true),
            None => (&weapons[self.weapon()?], 0.0, false),
        };
        let name = animation(power, primary)?;
        let seconds = if channel {
            seconds
        } else {
            let holding = active.0 == self.weapon()?
                && active.2.fire_cmd > 0
                && active.2.state == core::weapon::FireState::Firing;
            self.pose(clips, class, &primary.name, name, holding)?.1
        };
        let saber = primary.saber.as_ref()?;
        let root = saber
            .clips
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .map(|c| Vec2::from(c.root_speed));
        let scale = if class.root_scale > 0.0 {
            class.root_scale
        } else {
            1.0
        };
        let velocity = crate::animator::body_vel(u.soldier.vel, u.soldier.yaw) / scale;
        let legs = self.legs.update(name, velocity, root);
        let descriptor = class.custom.upper(name);
        Some(Frame {
            animation: name,
            seconds,
            legs,
            aim: (descriptor.aim, descriptor.blend_in),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_offhand_legs_drop_at_native_velocity_window_and_rejoin_only_on_new_action() {
        let mut legs = Legs::default();
        assert!(legs.update("force", Vec2::ZERO, Some(Vec2::ZERO)));
        assert!(legs.update("force", Vec2::new(0.3, 0.0), Some(Vec2::ZERO)));
        assert!(!legs.update("force", Vec2::new(0.31, 0.0), Some(Vec2::ZERO)));
        assert!(!legs.update("force", Vec2::ZERO, Some(Vec2::ZERO)));
        assert!(legs.update("other", Vec2::new(2.0, 3.0), Some(Vec2::new(2.0, 3.0))));
        assert!(!legs.update("other", Vec2::ZERO, Some(Vec2::new(2.0, 3.0))));
    }

    #[test]
    fn emitter_pose_ends_at_the_authored_clip_and_retained_events_do_not_restart_it() {
        let mut clock = CastClock::default();
        let class = ClassAnim {
            family: "human".into(),
            ..Default::default()
        };
        let clips = [Clip {
            name: "human_rifle_push_full".into(),
            fps: 30.0,
            frames: 16,
            tracks: Vec::new(),
        }];
        clock.start(4, 7);
        clock.advance(0.5);
        clock.start(4, 7);
        clock.start(5, 7);
        assert_eq!(
            clock.pose(&clips, &class, "anything", "push", false),
            Some((7, 0.5))
        );
        clock.advance(1.0 / 30.0);
        assert!(clock
            .pose(&clips, &class, "anything", "push", false)
            .is_none());
        clock.start(6, 7);
        assert_eq!(
            clock.pose(&clips, &class, "anything", "push", false),
            Some((7, 0.0))
        );
    }

    #[test]
    fn final_frame_cast_holds_without_restarting_and_releases() {
        use manifest::combo::{ComboDef, ComboField, ComboKeyword as K, ComboValue as V};
        let mut class = ClassAnim {
            family: "human".into(),
            ..Default::default()
        };
        class.custom.register(&ComboDef {
            name_hash: 1,
            fields: vec![ComboField {
                keyword: K::Animation,
                args: vec![V::Text("cast".into())],
                scoped: true,
                children: vec![ComboField {
                    keyword: K::Loop,
                    args: vec![V::Text("FinalFrame".into())],
                    scoped: false,
                    children: vec![],
                }],
            }],
        });
        let clips = [Clip {
            name: "human_rifle_cast_full".into(),
            fps: 30.0,
            frames: 31,
            tracks: vec![],
        }];
        let mut clock = CastClock::default();
        clock.start(1, 2);
        for tick in 2..200 {
            clock.advance(1.0 / 30.0);
            clock.start(tick, 2);
            let (_, age) = clock.pose(&clips, &class, "weapon", "cast", true).unwrap();
            assert!((age - ((tick - 1) as f32 / 30.0).min(1.0)).abs() < 0.0001);
        }
        assert!(clock
            .pose(&clips, &class, "weapon", "cast", false)
            .is_none());
        clock.start(200, 2);
        assert_eq!(
            clock.pose(&clips, &class, "weapon", "cast", true),
            Some((2, 0.0))
        );
    }

    fn melee_primary() -> core::weapon::WeaponClass {
        use manifest::combo::{ComboDef, ComboField, ComboKeyword as K, ComboValue as V};
        let mut primary = core::weapon::WeaponClass::default();
        primary.saber = Some(Box::new(core::hero_weapon::SaberClass {
            combo: ComboDef {
                name_hash: 0,
                fields: vec![ComboField {
                    keyword: K::OffhandAnimation,
                    args: vec![V::Text("stand_useforce".into()), V::Text("FIRE".into())],
                    scoped: false,
                    children: vec![],
                }],
            },
            blades: vec![],
            throw_edges: vec![],
            deflect_edges: vec![],
            weapon_bind: glam::Mat4::IDENTITY,
            throw_bindings: vec![],
            clips: vec![],
            state_seconds: vec![],
            damage: 0.0,
            hit_sound: None,
            deflect_sound: None,
            hit_effect: None,
            deflect_effect: None,
        }));
        primary
    }

    /// Kit Fisto's Force ball (a cannon firing a missile) throws with his
    /// offhand FIRE pose, the whole authored clip (40 frames: 1.3 s), as an
    /// emitter power does; a gun in a soldier's hands, or a melee hero's
    /// buff with no round, plays no cast.
    #[test]
    fn a_melee_heros_thrown_ball_plays_the_offhand_fire_pose_to_its_end() {
        let primary = melee_primary();
        let mut ball = core::weapon::WeaponClass::default();
        assert_eq!(animation(&ball, &primary), None, "no round, no cast");
        ball.ordnance.missile = Some(core::missile::MissileClass::default());
        assert!(offhand_projectile(&ball, &primary));
        assert_eq!(animation(&ball, &primary), Some("stand_useforce"));
        let rifle = core::weapon::WeaponClass::default();
        assert!(!offhand_projectile(&ball, &rifle));
        assert_eq!(animation(&ball, &rifle), None);

        let class = ClassAnim {
            family: "fisto".into(),
            ..Default::default()
        };
        let clips = [Clip {
            name: "fisto_rifle_stand_useforce_full".into(),
            fps: 30.0,
            frames: 40,
            tracks: vec![],
        }];
        let mut clock = CastClock::default();
        clock.start(1, 3);
        let mut last = 0.0;
        for tick in 2..60u64 {
            clock.advance(1.0 / 30.0);
            match clock.pose(&clips, &class, "saber", "stand_useforce", false) {
                Some((3, age)) => last = age,
                other => {
                    assert!(other.is_none());
                    break;
                }
            }
            let _ = tick;
        }
        // Within a frame of its last (39 / 30 s): the clock's steps are
        // summed floats.
        assert!(
            (last - 39.0 / 30.0).abs() <= 1.0 / 30.0 + 1e-4,
            "played to its last frame: {last}"
        );
    }
}
