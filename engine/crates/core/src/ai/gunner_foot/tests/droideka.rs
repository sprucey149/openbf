use super::*;

#[test]
fn droideka_gunner_uses_the_class_pitch_rate_and_soldiers_keep_theirs() {
    use crate::droideka::DroidekaClass;
    use crate::unit::UnitSpec;
    use std::sync::Arc;
    for deka_rate in [None, Some(0.6), Some(12.0)] {
        for pitch in [-0.007, 0.007] {
            let mut s = Sim::empty(5);
            let h = s
                .spawn(&UnitSpec {
                    droideka: deka_rate.map(|rate| {
                        Arc::new(DroidekaClass {
                            max_pitch_speed: rate,
                            ..Default::default()
                        })
                    }),
                    ..UnitSpec::trooper(0, Vec3::ZERO)
                })
                .unwrap();
            s.unit_mut(h).unwrap().soldier.pitch = pitch;
            let u = s.unit(h).unwrap();
            let tk = Tick {
                rules: crate::ai::UnitRules::for_unit(&s, u),
                me: h,
                u,
                eye: u.ai_eye(),
                wc: &s.weapon_classes[u.weapon_class],
                tier: s.unit_tier(u),
                skill: s.skill_of(u),
            };
            let mut st = u.ai;
            let mut p = PadState::default();
            run(
                &s,
                &tk,
                &mut st,
                &AimRequest::Travel(Vec3::NEG_Z * 20.0),
                &mut p,
                &mut NavTick::default(),
            )
            .unwrap();
            let rate = deka_rate.unwrap_or(u.consts.max_pitch_speed);
            assert_eq!(
                p.ry,
                PadState::quantize(-pitch / (rate * crate::TICK_DT)),
                "deka rate {deka_rate:?}, pitch {pitch}"
            );
            let after = pitch + PadState::axis(p.ry) * rate * crate::TICK_DT;
            assert!(after.abs() < 1e-5, "one-tick leveling left {after}");
        }
    }
}
