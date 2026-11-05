use super::*;
use crate::weapon::OrdKnot;

fn simulation(bf2: bool) -> Sim {
    let mut s = Sim::empty(82);
    s.ground_rules = if bf2 {
        crate::soldier::GroundRules::Bf2
    } else {
        crate::soldier::GroundRules::Bf1
    };
    s
}

fn charged(bf2: bool, root_life: f32, full_life: f32) -> WeaponClass {
    let mut w = thermal(bf2);
    w.ordnance.velocity = 5.0;
    w.ordnance.life_span = root_life;
    w.ordnance.knots.push(OrdKnot {
        at: 1.0,
        velocity: 25.0,
        life_span: full_life,
        damage: 0.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    });
    w.ordnance.sticky.as_mut().unwrap().explosion = None;
    w
}

fn shot(charge: f32) -> ShotSpec {
    ShotSpec {
        charge,
        stage: 0,
        tilt: (0.0, 0.0),
        pitch_spread: 0.0,
        yaw_spread: 0.0,
    }
}

#[test]
fn sticky_launch_uses_the_shots_charge_for_speed_and_life() {
    for bf2 in [false, true] {
        for (charge, speed, life) in [
            (0.0, 5.0, 0.25),
            (0.25, 10.0, 0.5),
            (0.5, 15.0, 0.75),
            (1.0, 25.0, 1.25),
        ] {
            let mut s = simulation(bf2);
            let wc = s.add_weapon_class(charged(bf2, 0.25, 1.25));
            let owner = s
                .spawn(&UnitSpec {
                    weapon_class: wc,
                    ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
                })
                .unwrap();
            s.unit_mut(owner).unwrap().soldier.pitch = 0.0;
            s.soldier_shots(owner.index as usize, None, &[shot(charge)]);
            let g = &s.stickies[0];
            let dir = sticky::throw_dir(Vec3::NEG_Z, 0.21);
            assert_eq!(g.vel, dir * speed, "era={bf2} charge={charge}");
            assert_eq!(g.life, life);
            assert!(!g.armed);
        }
    }
}

#[test]
fn unknotted_sticky_launch_preserves_the_original_instance_bits() {
    for bf2 in [false, true] {
        for charge in [0.0, 0.25, 1.0] {
            let mut s = simulation(bf2);
            let w = thermal(bf2);
            let wc = s.add_weapon_class(w.clone());
            let owner = s
                .spawn(&UnitSpec {
                    weapon_class: wc,
                    ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
                })
                .unwrap();
            s.unit_mut(owner).unwrap().soldier.pitch = 0.0;
            let u = s.unit(owner).unwrap();
            let expected = Sticky::new(
                u.launch_origin(w.thrown.unwrap().release_hand(1.0)),
                sticky::throw_dir(Vec3::NEG_Z, 0.21) * w.ordnance.velocity,
                w.ordnance.sticky.as_ref().unwrap(),
                w.ordnance.life_span,
                owner,
                0,
                wc,
            );
            s.soldier_shots(owner.index as usize, None, &[shot(charge)]);
            let bytes = |g: &Sticky| {
                let mut bytes = Vec::new();
                g.hash_into(&mut |part| bytes.extend_from_slice(part));
                bytes
            };
            assert_eq!(bytes(&s.stickies[0]), bytes(&expected));
        }
    }
}

/// Authored fixture: the knot changes fuse length in either direction.
/// Real sweep contacts exercise rest, water and a moving soldier anchor;
/// warning position and water's bounce estimate retain their existing rules.
#[test]
fn blended_sticky_fuse_preserves_existing_contact_warning_rules() {
    for bf2 in [false, true] {
        for (root_life, full_life) in [(0.25, 1.25), (1.25, 0.25)] {
            for contact in [Contact::Rest, Contact::Water, Contact::Stuck] {
                let mut s = simulation(bf2);
                s.heightfield = Some(flat_ground());
                let mut w = charged(bf2, root_life, full_life);
                w.ordnance.sticky.as_mut().unwrap().stick_mask = if contact == Contact::Stuck {
                    u32::MAX
                } else {
                    0
                };
                let wc = s.add_weapon_class(w);
                let owner = s
                    .spawn(&UnitSpec {
                        weapon_class: wc,
                        ..UnitSpec::player_in_slot(0, 0, Vec3::ZERO)
                    })
                    .unwrap();
                let target = s
                    .spawn(&UnitSpec::trooper(1, Vec3::new(20.0, 0.0, 0.0)))
                    .unwrap();
                s.soldier_shots(owner.index as usize, None, &[shot(0.5)]);
                let mut g = s.stickies.pop().unwrap();
                assert_eq!(g.life, 0.75);
                // Unarmed flight never spends the blended fuse.
                g.fly(TICK_DT, false);
                assert!(!g.burn(TICK_DT));
                assert_eq!(g.life, 0.75);
                if contact == Contact::Stuck {
                    g.pos = Vec3::new(20.0, 0.9, 1.0);
                    g.vel = Vec3::new(0.0, 0.0, -40.0);
                } else {
                    g.pos = Vec3::new(10.0, 0.2, 0.0);
                    g.vel = Vec3::new(0.0, -10.0, 0.0);
                    g.bounces = sticky::REST_BOUNCES;
                    if contact == Contact::Water {
                        s.water_level = Some(0.15);
                        // Isolate water arming from a later bottom contact.
                        s.heightfield = None;
                    }
                }
                s.tick = 10;
                assert!(s.tick_sticky(&mut g, TICK_DT));
                assert!(g.armed, "{contact:?}");
                assert_eq!(g.life, 0.75);
                if contact == Contact::Stuck {
                    assert!(
                        matches!(g.anchor, Some(Anchor::Unit { handle, .. }) if handle == target)
                    );
                } else if contact == Contact::Rest {
                    assert!(!g.moving);
                }
                let warning_life = if contact == Contact::Water {
                    0.75 + super::super::HAZARD_BOUNCE_TIME
                } else {
                    0.75
                };
                let deadline = s.tick + (warning_life * crate::TICK_HZ as f32).ceil() as u64;
                assert_eq!(
                    s.hazards[0].until, deadline,
                    "{contact:?}: rest/stuck use the instance fuse; water keeps its estimate"
                );
                let since = s.hazards[0].since;
                let warning_pos = s.hazards[0].pos;
                let mut elapsed = 0;
                loop {
                    s.tick += 1;
                    elapsed += 1;
                    if contact == Contact::Stuck {
                        s.unit_mut(target).unwrap().soldier.pos.x += 0.1;
                    }
                    if !s.tick_sticky(&mut g, TICK_DT) {
                        break;
                    }
                    assert!(elapsed < 30, "fuse must expire");
                    assert_eq!(s.hazards[0].since, since);
                    assert_eq!(s.hazards[0].until, deadline);
                    assert_eq!(s.hazards[0].pos, warning_pos);
                    if contact == Contact::Stuck {
                        assert!(g.pos.x > warning_pos.x, "the anchor must actually move");
                    }
                }
                assert!((elapsed as f32 * TICK_DT - 0.75).abs() <= TICK_DT);
                assert!(s
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::StickyBlast { .. })));
                assert_eq!(s.hazards[0].until, s.tick);
            }
        }
    }
}
