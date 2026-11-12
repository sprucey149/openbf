//! PS2 flyer BODY properties. Rates and angles are already radians; turret
//! sections must not override the pilot's flight controls. See re-flyer.md.

use super::{fill, float, h, section_scope, walker, Scope, FLYER_BASES};
use crate::classdb::ClassDb;
use core::vehicle::flyer::FlyerParams;
use glam::Vec3;
use manifest::ManifestHardpoint;

/// How FUN_00300230 stores an authored follower number.
#[derive(Clone, Copy)]
enum Convert {
    Plain,
    /// Stored as 1/x; an authored zero stores 0.0001 (re-flyer-ai.md).
    Inverse,
    Degrees,
}

impl Convert {
    fn apply(self, v: f32) -> f32 {
        match self {
            Convert::Plain => v,
            Convert::Inverse if v == 0.0 => 0.0001,
            Convert::Inverse => 1.0 / v,
            Convert::Degrees => v.to_radians(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlyerClass {
    motion: FlyerParams,
}

impl FlyerClass {
    pub fn from_db(db: &ClassDb, class: &str, bf2: bool) -> Option<Self> {
        let base = db.engine_base(class)?;
        if !FLYER_BASES.contains(&base) {
            return None;
        }
        let mut c = Self {
            motion: if bf2 {
                FlyerParams::bf2()
            } else {
                FlyerParams::default()
            },
        };
        for def in db.chain(class).into_iter().rev() {
            let mut scope = Scope::Body;
            for (key, value) in &def.props {
                if *key == h("FLYERSECTION") {
                    scope = section_scope(value);
                } else if !matches!(scope, Scope::Turret(_)) || !walker::turret_claims(*key) {
                    // The flyer setter handles flight keys before its base
                    // destructible setter sees CHUNKSECTION. A chunk does not
                    // close BODY or reset the selected mounted turret. Only
                    // properties the turret claims are intercepted (B2:124864).
                    c.set(*key, value);
                }
            }
        }
        c.motion.command_flyer = base == "commandflyer";
        Some(c)
    }

    /// The model-bound-derived rest height is resolved by the same reader
    /// that positions the pad spawn, including GeometryName/property order.
    pub fn params(&self, landed_height: f32) -> FlyerParams {
        FlyerParams {
            landed_height,
            ..self.motion.clone()
        }
    }

    fn set(&mut self, key: u32, value: &str) {
        let p = &mut self.motion;
        if key == h("Acceleration") {
            if let Some(number) = float(value).filter(|v| v.is_finite()) {
                p.acceleration = number;
            }
            return;
        }
        let scalars = [
            ("Acceleraton", &mut p.acceleration),
            ("MinSpeed", &mut p.min_speed),
            ("MidSpeed", &mut p.mid_speed),
            ("MaxSpeed", &mut p.max_speed),
            ("StrafeSpeed", &mut p.strafe_speed),
            ("PitchRate", &mut p.pitch_rate),
            ("TurnRate", &mut p.turn_rate),
            ("PitchFilter", &mut p.pitch_filter),
            ("PitchFilterDecel", &mut p.pitch_damp),
            ("TurnFilter", &mut p.turn_filter),
            ("TurnFilterDecel", &mut p.turn_damp),
            ("RollRate", &mut p.roll_rate),
            ("RollFilter", &mut p.roll_filter),
            ("ThrustPitchAngle", &mut p.thrust_pitch_angle),
            ("StrafeRollAngle", &mut p.strafe_roll_angle),
            ("BankAngle", &mut p.bank_angle),
            ("BankFilter", &mut p.bank_filter),
            ("LevelFilter", &mut p.level_filter),
            ("LevelFilterLanding", &mut p.level_damp),
            ("TakeoffHeight", &mut p.takeoff_height),
            ("TakeoffTime", &mut p.takeoff_time),
            ("TakeoffSpeed", &mut p.takeoff_speed),
            ("LandingTime", &mut p.landing_time),
            ("LandingSpeed", &mut p.landing_speed),
            ("LandedHeight", &mut p.landed_height),
            ("BoostSpeed", &mut p.boost_speed),
            ("BoostAcceleration", &mut p.boost_acceleration),
            ("EnergyBar", &mut p.energy_max),
            ("EnergyAutoRestore", &mut p.energy_auto_restore),
            ("EnergyBoostDrain", &mut p.energy_boost_drain),
            ("VelocityFilter", &mut p.velocity_filter),
        ];
        for (name, slot) in scalars {
            if key == h(name) {
                if let Some(number) =
                    float(value).filter(|v| v.is_finite() && (name != "EnergyBar" || *v >= 0.0))
                {
                    *slot = number;
                }
                return;
            }
        }
        // The path follower's setter FUN_00300230 (re-flyer-ai.md's table).
        let f = &mut p.follower;
        let number = float(value).filter(|v| v.is_finite());
        for (name, slot, convert) in [
            ("PathFollowerSpeed", &mut f.speed, Convert::Plain),
            ("PathFollowerTurnSpeed", &mut f.turn_speed, Convert::Plain),
            (
                "PathFollowerAcceleration",
                &mut f.inv_acceleration,
                Convert::Inverse,
            ),
            (
                "PathFollowerRollAcceleration",
                &mut f.roll_acceleration,
                Convert::Degrees,
            ),
            (
                "PathFollowerRollLookAhead",
                &mut f.roll_look_ahead,
                Convert::Plain,
            ),
            (
                "PathFollowerRollTolerance",
                &mut f.roll_tolerance,
                Convert::Degrees,
            ),
            ("PathFollowerRollSpeed", &mut f.roll_speed, Convert::Plain),
            (
                "PathFollowerRollAccelerationLag",
                &mut f.roll_lag_inv,
                Convert::Inverse,
            ),
            (
                "PathFollowerRollTurnFactor",
                &mut f.roll_turn_factor,
                Convert::Plain,
            ),
            (
                "PathFollowerRollSpeedFactor",
                &mut f.roll_speed_factor,
                Convert::Plain,
            ),
            (
                "PathFollowerRollDampening",
                &mut f.roll_dampening,
                Convert::Plain,
            ),
            (
                "PathFollowerMinBranchDist",
                &mut f.min_branch_dist,
                Convert::Plain,
            ),
        ] {
            if key == h(name) {
                if let Some(number) = number {
                    *slot = convert.apply(number);
                }
                return;
            }
        }
        if key == 0xe510_8b93 {
            f.formations = f.formations.saturating_add(1);
            return;
        }
        if key == h("PathFollowerClass") {
            f.class_id = h(value.trim().trim_matches('"'));
            return;
        }
        if key == h("PathFollowerFlyerVsFlyer") {
            if let Some(number) = number {
                f.flyer_vs_flyer = number != 0.0;
            }
            return;
        }
        if key == h("NeverCrashWhenUnpiloted") {
            p.never_crash_when_unpiloted = value.trim() == "1" || number.is_some_and(|v| v != 0.0);
            return;
        }
        if key == h("CollisionThreshold") {
            fill(&mut p.collision_threshold, value);
        } else if key == h("CollisionScale") {
            fill(&mut p.collision_scale, value);
        }
    }
}

#[derive(Clone, Debug)]
pub struct FlyerExhaust {
    pub effect: String,
    pub local: glam::Mat4,
    pub node: Option<String>,
    pub offset: Vec3,
    pub scale: [f32; 3],
}

/// The PS2 keeps at most eight thrust effects per class (`FUN_00211a40`,
/// B2:124203-124223); a later `ThrustEffect` is dropped.
const MAX_THRUST_EFFECTS: usize = 8;

pub(crate) fn flyer_exhaust(
    db: &ClassDb,
    class: &str,
    nodes: &[ManifestHardpoint],
) -> Vec<FlyerExhaust> {
    // `FUN_00211a40` reads these keys in ODF order, each applying to the most
    // recent `ThrustEffect` (B2:124203-125043).
    let (effect_key, point_key, offset_key) = (
        h("ThrustEffect"),
        h("ThrustAttachPoint"),
        h("ThrustAttachOffset"),
    );
    let (min_scale_key, max_scale_key, start_scale_key) = (
        h("ThrustEffectMinScale"),
        h("ThrustEffectMaxScale"),
        h("ThrustEffectScaleStart"),
    );
    let mut effects: Vec<(String, Option<String>, Vec3, [f32; 3])> = Vec::new();
    'definitions: for def in db.chain(class).into_iter().rev() {
        for (key, value) in &def.props {
            if *key == effect_key {
                if effects.len() == MAX_THRUST_EFFECTS {
                    break 'definitions;
                }
                effects.push((value.trim().to_owned(), None, Vec3::ZERO, [1.0, 1.0, 0.0]));
            } else if let Some((_, node, offset, scale)) = effects.last_mut() {
                let slot = match *key {
                    k if k == min_scale_key => Some(0),
                    k if k == max_scale_key => Some(1),
                    k if k == start_scale_key => Some(2),
                    _ => None,
                };
                if let Some(i) = slot {
                    if let Ok(v) = value.trim().parse::<f32>() {
                        scale[i] = v;
                    }
                } else if *key == point_key {
                    *node = Some(value.trim().to_owned());
                } else if *key == offset_key {
                    let mut values = value.split_whitespace().map(str::parse::<f32>);
                    if let (Some(Ok(x)), Some(Ok(y)), Some(Ok(z)), None) =
                        (values.next(), values.next(), values.next(), values.next())
                    {
                        *offset = Vec3::new(x, y, z);
                    }
                }
            }
        }
    }
    effects
        .into_iter()
        .map(|(effect, node, offset, scale)| {
            let base = node
                .as_deref()
                .and_then(|name| nodes.iter().find(|n| n.name.eq_ignore_ascii_case(name)))
                .map_or(glam::Mat4::IDENTITY, |n| {
                    crate::viewer::instance_matrix(
                        n.axes.unwrap_or(glam::Mat3::IDENTITY.to_cols_array_2d()),
                        n.pos,
                    )
                });
            FlyerExhaust {
                effect,
                local: base * glam::Mat4::from_translation(offset),
                node,
                offset,
                scale,
            }
        })
        .collect()
}
