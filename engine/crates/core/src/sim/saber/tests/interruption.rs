use super::*;

fn moving_attack() -> (Sim, UnitHandle) {
    let mut sim = Sim::empty(712);
    sim.heightfield = Some(flat_ground());
    sim.ground_rules = crate::soldier::GroundRules::Bf2;
    sim.weapon_classes = vec![saber(program(vec![
        field(K::Duration, vec![V::Number(1.0)], vec![]),
        field(K::Posture, vec![text("Stand")], vec![]),
        field(K::Animation, vec![text("swing")], vec![]),
        field(K::Gravity, vec![V::Number(2.0)], vec![]),
        field(K::EnergyRestoreRate, vec![V::Number(-20.0)], vec![]),
        field(K::TurnFactor, vec![V::Number(0.5)], vec![]),
        field(
            K::AnimatedMove,
            vec![],
            vec![field(K::VelocityZ, vec![V::Number(12.0)], vec![])],
        ),
    ]))]
    .into();
    let owner = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    sim.step(&[PadState::default()]);
    let state = sim.unit(owner).unwrap().weapon.melee.as_ref().unwrap();
    assert_eq!(state.runtime.state, 1);
    assert!(state.movement_active && state.gravity.is_some());
    assert!(sim.unit(owner).unwrap().soldier.vel.length_squared() > 0.0);
    assert!(
        sim.unit(owner).unwrap().soldier.grounded,
        "legal ground-knock control"
    );
    (sim, owner)
}

#[test]
fn a_real_knock_interrupts_combo_motion_and_gravity_before_tumble_moves() {
    let (mut sim, owner) = moving_attack();
    sim.unit_mut(owner).unwrap().soldier.knock(
        Vec3::X,
        10.0,
        crate::soldier::GroundRules::Bf2,
        0.5,
    );
    assert_eq!(sim.unit(owner).unwrap().soldier.stance, Stance::Tumble);
    assert!(sim.unit(owner).unwrap().soldier.no_fire());
    sim.step(&[PadState::default()]);
    let unit = sim.unit(owner).unwrap();
    let state = unit.weapon.melee.as_ref().unwrap();
    assert_eq!(
        state.runtime.state, 0,
        "native Tumble posture interrupt must execute"
    );
    assert!(state.movement.is_none() && state.gravity.is_none());
    assert_eq!(
        unit.soldier.vel.y, 0.0,
        "stale zero-g must not launch a tumbling soldier"
    );
}

#[test]
fn normal_getup_and_its_fire_lock_do_not_apply_authored_attack_constants() {
    fn recovery() -> (Sim, UnitHandle) {
        let (mut sim, owner) = moving_attack();
        sim.unit_mut(owner).unwrap().soldier.knock(
            Vec3::X,
            10.0,
            crate::soldier::GroundRules::Bf2,
            0.5,
        );
        let mut getup = false;
        let mut locked = false;
        for _ in 0..240 {
            sim.step(&[PadState::default()]);
            let unit = sim.unit(owner).unwrap();
            getup |= unit.soldier.stance == Stance::GetUp;
            if unit.soldier.stance == Stance::Stand && unit.soldier.fire_lock {
                locked = true;
                break;
            }
        }
        assert!(
            getup && locked,
            "the production knock/get-up/fire-lock path must run"
        );
        (sim, owner)
    }
    let (mut sim, owner) = recovery();
    let (mut control, control_owner) = recovery();
    assert_eq!(sim.hash(), control.hash());
    assert_eq!(owner, control_owner);
    control.step(&[PadState::default()]);
    sim.step(&[PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }]);
    let unit = sim.unit(owner).unwrap();
    let ordinary = control.unit(owner).unwrap();
    assert!(unit.soldier.no_fire());
    assert_eq!(unit.soldier.pos, ordinary.soldier.pos);
    assert_eq!(unit.soldier.vel, ordinary.soldier.vel);
    assert_eq!(unit.soldier.energy, ordinary.soldier.energy);
    let mut constants = unit.consts;
    sim.saber_constants(owner.index as usize, &mut constants);
    assert_eq!(constants.max_turn_speed, unit.consts.max_turn_speed);
    assert!(constants.motion_target.is_none());
    assert_eq!(constants.energy_restore, unit.consts.energy_restore);
}

#[test]
fn derived_combo_outputs_are_recomputed_before_the_next_live_consumer() {
    for knocked in [false, true] {
        let (mut ordinary, owner) = moving_attack();
        let (mut changed, changed_owner) = moving_attack();
        assert_eq!(owner, changed_owner);
        if knocked {
            ordinary.unit_mut(owner).unwrap().soldier.knock(
                Vec3::X,
                10.0,
                crate::soldier::GroundRules::Bf2,
                0.5,
            );
        }
        if knocked {
            changed.unit_mut(owner).unwrap().soldier.knock(
                Vec3::X,
                10.0,
                crate::soldier::GroundRules::Bf2,
                0.5,
            );
        }
        assert_eq!(ordinary.hash(), changed.hash());
        let state = changed
            .unit_mut(owner)
            .unwrap()
            .weapon
            .melee
            .as_mut()
            .unwrap();
        state.animation = None;
        state.animation_loop = !state.animation_loop;
        state.hidden_edges = u8::MAX;
        state.aligned_to_thrust = !state.aligned_to_thrust;
        state.deflect = Some(combo::Deflect {
            min_angle: -180.0,
            max_angle: 180.0,
            energy_cost: 0.0,
        });
        state.movement = Some(combo::Movement {
            velocity: [200.0, -300.0],
        });
        state.gravity = Some(combo::Gravity {
            factor: -10.0,
            target: None,
            impulse: false,
            zero_at_target: false,
        });
        state.restore_rate = Some(-100.0);
        state.turn_factor = 10.0;
        state.attacks.clear();
        assert_eq!(
            ordinary.hash(),
            changed.hash(),
            "these outputs are derived, not independent history"
        );
        ordinary.step(&[PadState::default()]);
        changed.step(&[PadState::default()]);
        assert_eq!(
            ordinary.hash(),
            changed.hash(),
            "derived outputs escaped recomputation (knocked={knocked})"
        );
    }
}
