//! Measured BF2 stance boundaries from re-soldier-ground.md §2.3.
use super::*;

#[test]
fn fc_bf2_stand_fall_waits_until_after_the_measured_one_second() {
    let c = SoldierConstants::default();
    for stance in [Stance::Stand, Stance::Sprint] {
        // Start at the PS2 trace's reset row: airborne, clock zero.
        let mut s = Soldier::new(Vec3::Y * 100.0);
        s.grounded = false;
        s.stance = stance;
        s.vel.z = -c.max_speed;
        let pad = PadState {
            ly: i16::MAX,
            buttons: if stance == Stance::Sprint {
                buttons::SPRINT
            } else {
                0
            },
            ..Default::default()
        };
        for tick in 1..=31 {
            s.step(DT, &pad, &c, GroundRules::Bf2);
            s.resolve_ground(
                DT,
                crate::collision::NO_GROUND,
                Vec3::Y,
                0.0,
                GroundRules::Bf2,
            );
            assert_eq!(
                s.stance,
                if tick <= 30 { stance } else { Stance::Fall },
                "{stance:?} tick {tick}, clock {}",
                s.air_time
            );
        }
        // A real overrun still falls; a touch-down resets the elapsed clock.
        let mut over = Soldier::new(Vec3::Y * 100.0);
        over.grounded = false;
        over.stance = stance;
        over.vel.z = -c.max_speed;
        over.air_time = 1.001;
        over.resolve_ground(
            DT,
            crate::collision::NO_GROUND,
            Vec3::Y,
            0.0,
            GroundRules::Bf2,
        );
        assert_eq!(over.stance, Stance::Fall);
        s.pos = Vec3::ZERO;
        s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, GroundRules::Bf2);
        assert_eq!(s.air_time, 0.0);
    }
}
