//! Issue #103: refused roll input must not cancel sprint bookkeeping.
use super::*;

#[test]
fn fc_refused_roll_each_tick_does_not_skip_sprint_drain_or_empty_exit() {
    for rules in [GroundRules::Bf1, GroundRules::Bf2] {
        let mut c = SoldierConstants {
            roll_speed_factor: 0.0,
            energy_restore: 0.0,
            energy_restore_idle: 0.0,
            ..Default::default()
        };
        let mut s = Soldier::new(Vec3::ZERO);
        let mut pad = full_fwd();
        pad.buttons = buttons::SPRINT;
        for _ in 0..30 {
            s.step(DT, &pad, &c, rules);
            s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
        }
        assert_eq!(s.stance, Stance::Sprint);
        // Class-disabled rolling refuses while the sprint drains naturally.
        s.energy = 2.0;
        assert!(!s.energy_depleted);
        pad.buttons |= buttons::ROLL_STANCE;
        let mut control = s.clone();
        let mut drained = 0;
        for tick in 0..12 {
            // Inject a new press edge every tick, stronger than merely holding.
            s.prev_buttons &= !buttons::ROLL_STANCE;
            let before = s.energy;
            if before <= 0.0 {
                // Once empty, enable rolling: depletion must still refuse it,
                // and that refusal must not skip the sprint-to-stand exit.
                c.roll_speed_factor = SoldierConstants::default().roll_speed_factor;
            }
            s.step(DT, &pad, &c, rules);
            s.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
            let mut no_roll = pad;
            no_roll.buttons &= !buttons::ROLL_STANCE;
            control.step(DT, &no_roll, &c, rules);
            control.resolve_ground(DT, 0.0, Vec3::Y, 0.0, rules);
            assert_eq!(s.stance, control.stance, "{rules:?} tick {tick}");
            assert_eq!(s.energy, control.energy, "{rules:?} tick {tick}");
            if before > 0.0 {
                drained += 1;
                assert_eq!(s.energy, before - c.energy_drain_sprint * DT);
            } else {
                assert_eq!(s.stance, Stance::Stand);
                assert_eq!(s.energy, before, "a refused roll spends nothing");
            }
            if s.energy <= 0.0 {
                assert!(s.energy_depleted);
            }
        }
        assert!(drained > 1);
        assert_eq!(s.stance, Stance::Stand);
    }
}
