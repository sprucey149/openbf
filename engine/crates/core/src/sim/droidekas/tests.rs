use std::sync::Arc;

use glam::Vec3;

use crate::droideka::{DroidekaClass, Order, IDLE, IDLE_TO_ROLL, ROLLING, ROLL_TO_IDLE};
use crate::sim::tests::flat_ground;
use crate::sim::{buttons, Event, PadState, Sim};
use crate::unit::{Controller, UnitHandle, UnitSpec};
use crate::weapon::{ExplosionClass, HealthType};

fn class() -> Arc<DroidekaClass> {
    Arc::new(DroidekaClass::default())
}

fn deka(spec: UnitSpec) -> UnitSpec {
    UnitSpec {
        max_health: 550.0,
        health_type: HealthType::Droid,
        droideka: Some(class()),
        ..spec
    }
}

fn sim() -> Sim {
    let mut s = Sim::empty(11);
    s.heightfield = Some(flat_ground());
    s
}

fn pad(buttons: u32, ly: f32) -> PadState {
    PadState {
        buttons,
        ly: PadState::quantize(ly),
        ..Default::default()
    }
}

fn state(s: &Sim, h: UnitHandle) -> u8 {
    s.units[h.index as usize].droideka.as_ref().unwrap().state
}

fn shots(s: &Sim) -> usize {
    s.events
        .iter()
        .filter(|e| matches!(e, Event::Shot { .. }))
        .count()
}

fn cues(s: &Sim) -> Vec<crate::droideka::Cue> {
    s.events
        .iter()
        .filter_map(|e| match e {
            Event::Droideka { cue, .. } => Some(*cue),
            _ => None,
        })
        .collect()
}

#[test]
fn a_bot_droideka_folds_at_spawn_and_rolls() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::trooper(1, Vec3::ZERO))).unwrap();
    s.step(&[]);
    assert_eq!(state(&s, h), IDLE_TO_ROLL);
    assert_eq!(cues(&s), vec![crate::droideka::Cue::Fold]);
    for _ in 0..20 {
        s.step(&[]);
    }
    assert_eq!(state(&s, h), ROLLING);
    assert!(s.units[h.index as usize].soldier.grounded);
}

#[test]
fn a_player_walks_fires_and_cannot_fire_while_folding() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    let mut fired = 0;
    for _ in 0..60 {
        s.step(&[pad(buttons::FIRE, 1.0)]);
        fired += shots(&s);
    }
    assert!(fired > 5, "{fired}");
    let z = s.units[h.index as usize].soldier.pos.z;
    assert!(z < -3.0, "walked forward: {z}");
    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
    assert_eq!(state(&s, h), IDLE_TO_ROLL);
    let mut fired = 0;
    for _ in 0..60 {
        s.step(&[pad(buttons::FIRE, 1.0)]);
        fired += shots(&s);
    }
    assert_eq!(state(&s, h), ROLLING);
    assert_eq!(fired, 0);
    let x0 = s.units[h.index as usize].soldier.pos;
    for _ in 0..30 {
        s.step(&[pad(0, 1.0)]);
    }
    let rolled = (s.units[h.index as usize].soldier.pos - x0).length();
    assert!(rolled > 6.0, "{rolled}");
    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
    assert_eq!(state(&s, h), ROLL_TO_IDLE);
    for _ in 0..55 {
        s.step(&[pad(0, 0.0)]);
    }
    assert_eq!(state(&s, h), IDLE);
}

fn target(s: &mut Sim, shield: bool) -> UnitHandle {
    let h = s
        .spawn(&deka(UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0))))
        .unwrap();
    let d = s.units[h.index as usize].droideka.as_mut().unwrap();
    d.order = Order::Deploy;
    d.shield.on = shield;
    h
}

fn shoot(s: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        s.step(&[PadState {
            buttons: buttons::FIRE,
            ..Default::default()
        }]);
    }
}

#[test]
fn a_raised_shield_takes_the_bolts_and_the_body_takes_none() {
    let mut s = sim();
    s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let t = target(&mut s, true);
    shoot(&mut s, 60);
    let u = &s.units[t.index as usize];
    let d = u.droideka.as_ref().unwrap();
    assert!(d.shield.points < 3000.0, "{}", d.shield.points);
    assert_eq!(u.health.cur, 550.0);
    assert!(u.last_hit.is_some());
}

#[test]
fn a_lowered_shield_lets_the_bolts_reach_the_body() {
    let mut s = sim();
    s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let t = target(&mut s, false);
    shoot(&mut s, 60);
    let u = &s.units[t.index as usize];
    assert_eq!(u.droideka.as_ref().unwrap().shield.points, 3000.0);
    assert!(u.health.cur < 550.0, "{}", u.health.cur);
}

#[test]
fn a_blast_spends_the_shield_before_the_health_and_does_not_throw_it() {
    let mut s = sim();
    let t = target(&mut s, true);
    s.units[t.index as usize]
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .points = 40.0;
    let e = ExplosionClass {
        damage: 100.0,
        damage_outer: 4.0,
        push: 20.0,
        ..Default::default()
    };
    let at = s.units[t.index as usize].soldier.pos + Vec3::Y;
    s.explode(&e, at, None, Some(0));
    let u = &s.units[t.index as usize];
    assert_eq!(u.droideka.as_ref().unwrap().shield.points, 0.0);
    assert!(
        u.health.cur < 550.0 && u.health.cur > 450.0,
        "{}",
        u.health.cur
    );
    assert!(!u.soldier.stance.knocked());
}

#[test]
fn a_dead_droideka_bursts_with_no_corpse_and_respawns_whole() {
    let mut s = sim();
    let t = target(&mut s, true);
    let i = t.index as usize;
    s.units[i].health.cur = -10.0;
    s.unit_killed(t, None);
    s.step(&[]);
    assert!(s
        .events
        .iter()
        .any(|e| matches!(e, Event::DroidBurst { unit, .. } if *unit == t)));
    assert!(!s.units[i].body && s.units[i].burst);
    let fresh = s.units[i].spec.build(t.gen + 1, &s.weapon_classes);
    let d = fresh.droideka.as_ref().unwrap();
    assert_eq!(d.state, IDLE);
    assert_eq!(d.shield.points, 3000.0);
    assert!(!d.shield.on);
}

#[test]
fn deep_water_wears_the_droideka_down() {
    let mut s = sim();
    s.water_level = Some(2.0);
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    for _ in 0..45 {
        s.step(&[pad(0, 0.0)]);
    }
    let hp = s.units[h.index as usize].health.cur;
    assert!((550.0 - 10.0 * 16.0..550.0).contains(&hp), "{hp}");
}

#[test]
fn droideka_runs_replay_to_the_same_hash() {
    let run = |turn: f32| {
        let mut s = sim();
        s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
        s.spawn(&deka(UnitSpec {
            controller: Controller::Idle,
            ..UnitSpec::trooper(1, Vec3::new(5.0, 0.0, -10.0))
        }))
        .unwrap();
        for i in 0..240u32 {
            let mut p = pad(if i % 40 == 0 { buttons::ROLL_STANCE } else { 0 }, 0.8);
            p.rx = PadState::quantize(turn);
            if i % 55 == 0 {
                p.buttons |= buttons::SECONDARY_FIRE | buttons::JUMP;
            }
            s.step(&[p]);
        }
        s.hash()
    };
    assert_eq!(run(0.3), run(0.3));
    assert_ne!(run(0.3), run(-0.3));
}

fn bot_and_foe(dist: f32) -> (Sim, UnitHandle) {
    let mut s = sim();
    s.spawn(&UnitSpec::player(0, Vec3::new(0.0, 0.0, -dist)))
        .unwrap();
    for _ in 0..crate::ai::dfocus::UNTARGETED {
        s.step(&[PadState::default()]);
    }
    let h = s
        .spawn(&deka(UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(1, Vec3::ZERO)
        }))
        .unwrap();
    (s, h)
}

#[test]
fn a_bot_droideka_stays_rolled_with_a_far_enemy_in_sight() {
    let (mut s, h) = bot_and_foe(30.0);
    for _ in 0..90 {
        s.step(&[PadState::default()]);
    }
    let u = &s.units[h.index as usize];
    println!("{:?} state {}", u.ai.behaviour, state(&s, h));
    assert_ne!(u.ai.behaviour, crate::ai::Behaviour::Fight);
    assert_eq!(state(&s, h), ROLLING);
}

#[test]
fn a_bot_droideka_deploys_raises_its_shield_and_fires_at_a_near_enemy() {
    let (mut s, h) = bot_and_foe(8.0);
    let mut fired = 0;
    let mut shield = false;
    let mut states = Vec::new();
    for _ in 0..150 {
        s.step(&[PadState::default()]);
        s.units[0].health.cur = s.units[0].health.max;
        let d = s.units[h.index as usize].droideka.as_ref().unwrap();
        shield |= d.shield.on;
        if states.last() != Some(&d.state) {
            states.push(d.state);
        }
        fired += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == h))
            .count();
    }
    let u = &s.units[h.index as usize];
    println!(
        "{:?} states {states:?} fired {fired} shield {shield}",
        u.ai.behaviour
    );
    assert_eq!(u.ai.behaviour, crate::ai::Behaviour::Fight);
    assert_eq!(states, [IDLE], "it stays deployed");
    assert!(shield);
    assert!(fired > 0);
}

#[test]
fn a_bot_droideka_rolls_again_once_its_fight_is_over() {
    let (mut s, h) = bot_and_foe(8.0);
    let mut fought = false;
    for _ in 0..300 {
        s.step(&[PadState::default()]);
        fought |= s.units[h.index as usize].ai.behaviour == crate::ai::Behaviour::Fight;
    }
    assert!(fought);
    assert!(!s.units[0].alive || s.units[0].health.cur <= 0.0);
    assert!(
        matches!(state(&s, h), IDLE_TO_ROLL | ROLLING),
        "{}",
        state(&s, h)
    );
}

#[test]
fn audit_droideka_look_preview_matches_each_form_without_zoom() {
    for rolling in [false, true] {
        for boost in [false, true] {
            for mouse in [false, true] {
                let mut s = sim();
                let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
                if rolling {
                    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
                    for _ in 0..60 {
                        s.step(&[pad(0, 1.0)]);
                    }
                    assert_eq!(state(&s, h), ROLLING);
                }
                let p = PadState {
                    rx: PadState::quantize(0.75),
                    ry: PadState::quantize(-0.6),
                    ly: PadState::quantize(1.0),
                    buttons: if boost { buttons::SPRINT } else { 0 },
                    mx: if mouse {
                        PadState::quantize_look(0.2)
                    } else {
                        0
                    },
                    my: if mouse {
                        PadState::quantize_look(0.1)
                    } else {
                        0
                    },
                    ..Default::default()
                };
                let Some(crate::sim::LookPreview::Foot { yaw, pitch, .. }) = s.look_preview(h, &p)
                else {
                    panic!("droideka look preview absent");
                };
                s.step(&[p]);
                let u = s.unit(h).unwrap();
                assert_eq!(
                    (yaw, pitch),
                    (u.soldier.yaw, u.soldier.pitch),
                    "rolling={rolling} boost={boost} mouse={mouse}"
                );
                s.step(&[PadState {
                    buttons: buttons::ZOOM,
                    ..Default::default()
                }]);
                assert!(
                    s.unit(h).unwrap().zoom.is_none(),
                    "droidekas deliberately cannot zoom"
                );
            }
        }
    }
}

#[test]
fn audit_droideka_zoom_press_is_ignored_in_walk_and_ball() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    for rolling in [false, true] {
        if rolling {
            s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
            for _ in 0..60 {
                s.step(&[pad(0, 0.0)]);
            }
            assert_eq!(state(&s, h), ROLLING);
        }
        for _ in 0..3 {
            s.step(&[pad(buttons::ZOOM, 0.0)]);
            assert!(s.unit(h).unwrap().zoom.is_none());
            s.step(&[pad(0, 0.0)]);
        }
    }
}

/// Rolled up on flat ground, ready to move.
fn rolled(s: &mut Sim) -> UnitHandle {
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
    for _ in 0..30 {
        s.step(&[pad(0, 0.0)]);
    }
    assert_eq!(state(s, h), ROLLING);
    h
}

fn deka_of(s: &Sim, h: UnitHandle) -> &crate::droideka::Droideka {
    s.unit(h).unwrap().droideka.as_ref().unwrap()
}

fn set_bar(s: &mut Sim, h: UnitHandle, points: f32) {
    s.units[h.index as usize]
        .droideka
        .as_mut()
        .unwrap()
        .shield
        .points = points;
}

/// The maintainer, 2026-11-01: standing never sprints, the press does nothing.
#[test]
fn standing_form_ignores_the_sprint_button() {
    let run = |sprint: u32| {
        let mut s = sim();
        let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
        for _ in 0..90 {
            s.step(&[pad(sprint, 1.0)]);
        }
        (s.unit(h).unwrap().soldier.pos, bar(&s, h))
    };
    let plain = run(0);
    let held = run(buttons::SPRINT);
    assert_eq!(plain, held);
}

#[test]
fn a_ball_sprints_at_the_class_scale_only_while_held_with_the_stick_forward() {
    let mut s = sim();
    let h = rolled(&mut s);
    let c = class();
    for _ in 0..15 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    let d = deka_of(&s, h);
    assert!(d.boosting);
    assert_eq!(d.omega, c.ball_omega_cap() * c.ball_boost_scale);
    // Released: back to the plain cap, at once.
    s.step(&[pad(0, 1.0)]);
    let d = deka_of(&s, h);
    assert!(!d.boosting);
    assert_eq!(d.omega, c.ball_omega_cap());
    // Half stick is no sprint.
    s.step(&[pad(buttons::SPRINT, 0.5)]);
    assert!(!deka_of(&s, h).boosting);
    // Backward is no sprint.
    s.step(&[pad(buttons::SPRINT, -1.0)]);
    assert!(!deka_of(&s, h).boosting);
}

/// A full bar lasts `EnergyBar / EnergyDrainSprint`, 3.3 s, then the ball drops
/// to its plain speed and waits for the recover share, the button still held.
#[test]
fn the_sprint_drains_the_bar_to_empty_stops_and_recovers() {
    let mut s = sim();
    let h = rolled(&mut s);
    let c = class();
    let dt = crate::TICK_DT;
    assert_eq!(c.ball_sprint_drain(), 900.0);
    assert_eq!(c.ball_sprint_recover(), 600.0);
    s.step(&[pad(buttons::SPRINT, 1.0)]);
    let start = bar(&s, h).0;
    for _ in 0..30 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    // No recharge under the drain.
    let want = start - 900.0 * 30.0 * dt;
    assert!((bar(&s, h).0 - want).abs() < 0.5, "{}", bar(&s, h).0);

    let mut ticks = 31u32;
    while bar(&s, h).0 > 0.0 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        ticks += 1;
        assert!(ticks < 200, "the bar never emptied");
    }
    let secs = ticks as f32 * dt;
    assert!((secs - 10.0 / 3.0).abs() < 0.1, "{secs}");
    let d = deka_of(&s, h);
    assert!(!d.boosting && d.sprint_spent);

    // Held: normal recovery at AddShieldOff, no sprint until 600, then again.
    let mut waited = 0u32;
    while deka_of(&s, h).sprint_spent {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        assert!(!deka_of(&s, h).boosting, "sprinted while spent");
        assert!(deka_of(&s, h).omega <= c.ball_omega_cap());
        waited += 1;
        assert!(waited < 200, "never recovered");
    }
    let secs = waited as f32 * dt;
    assert!((secs - 2.0).abs() < 0.1, "{secs}");
    s.step(&[pad(buttons::SPRINT, 1.0)]);
    assert!(deka_of(&s, h).boosting, "sprints again once recovered");

    // Let go: recharge resumes on the next tick, at the same rate as ever.
    s.step(&[pad(0, 1.0)]);
    let before = bar(&s, h).0;
    for _ in 0..30 {
        s.step(&[pad(0, 1.0)]);
    }
    let want = before + 300.0 * 30.0 * dt;
    assert!((bar(&s, h).0 - want).abs() < 0.5, "{}", bar(&s, h).0);
}

/// Unrolling mid-sprint ends the sprint and keeps what the bar holds: leaving
/// the ball refills nothing but normal recharge, and a raised shield has only
/// that left to absorb.
#[test]
fn unrolling_mid_sprint_ends_it_and_the_shield_gets_the_same_bar() {
    let mut s = sim();
    let h = rolled(&mut s);
    for _ in 0..60 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    let left = bar(&s, h).0;
    assert!(left < 2000.0, "{left}");
    s.step(&[pad(buttons::SPRINT | buttons::ROLL_STANCE, 1.0)]);
    assert_eq!(state(&s, h), ROLL_TO_IDLE);
    assert!(!deka_of(&s, h).boosting);
    let c = class();
    let unroll = (c.state_time(ROLL_TO_IDLE) / crate::TICK_DT) as usize + 2;
    let mut ticks = 1usize;
    for _ in 0..unroll {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        ticks += 1;
        assert!(!deka_of(&s, h).boosting);
    }
    assert_eq!(state(&s, h), crate::droideka::IDLE_TO_LEFT);
    let want = left + 300.0 * ticks as f32 * crate::TICK_DT;
    assert!((bar(&s, h).0 - want).abs() < 2.0, "{}", bar(&s, h).0);
    // The same pool is what the shield now spends: a hit shows on the bar.
    s.step(&[pad(buttons::SECONDARY_FIRE, 0.0)]);
    let d = s.units[h.index as usize].droideka.as_mut().unwrap();
    assert!(d.shield.on);
    assert_eq!(d.shield.absorb(100.0, 1.0), 0.0);
    let held = bar(&s, h).0;
    assert!(held < want, "{held}");
}

/// Sprinted empty, the lockout survives an unroll and a roll back up; only the
/// recharge clears it. A shield raised on the way spends the same bar.
#[test]
fn folding_and_unfolding_does_not_clear_a_spent_bar() {
    let mut s = sim();
    let h = rolled(&mut s);
    set_bar(&mut s, h, 30.0);
    for _ in 0..5 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    assert!(deka_of(&s, h).sprint_spent);
    // Out and back in. The unroll's own recharge could clear it fairly, so
    // the bar is held under the recover share to test the form change alone.
    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
    while state(&s, h) != IDLE {
        s.step(&[pad(0, 0.0)]);
    }
    set_bar(&mut s, h, 100.0);
    s.step(&[pad(buttons::ROLL_STANCE, 0.0)]);
    while state(&s, h) != ROLLING {
        s.step(&[pad(0, 0.0)]);
    }
    assert!(deka_of(&s, h).sprint_spent);
    s.step(&[pad(buttons::SPRINT, 1.0)]);
    assert!(!deka_of(&s, h).boosting, "a re-roll cleared the lockout");
}

/// Rolling up on a bar the shield emptied is the same lockout as sprinting it
/// empty, with the button held all the way through.
#[test]
fn rolling_up_on_an_empty_bar_gives_no_sprint_and_a_held_button_waits_for_the_ball() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    s.step(&[pad(buttons::SPRINT, 0.0)]);
    set_bar(&mut s, h, 0.0);
    s.step(&[pad(buttons::SPRINT, 0.0)]);
    assert!(deka_of(&s, h).sprint_spent, "empty standing counts");
    s.step(&[pad(buttons::SPRINT | buttons::ROLL_STANCE, 1.0)]);
    while state(&s, h) != ROLLING {
        assert!(!deka_of(&s, h).boosting, "no sprint in the fold");
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    assert!(!deka_of(&s, h).boosting);
    while deka_of(&s, h).sprint_spent {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        assert!(!deka_of(&s, h).boosting);
    }
    s.step(&[pad(buttons::SPRINT, 1.0)]);
    assert!(deka_of(&s, h).boosting, "held through the fold, it starts");
}

/// A bar part used by the shield sprints for what is left, no more.
#[test]
fn a_part_bar_sprints_for_what_it_holds() {
    let mut s = sim();
    let h = rolled(&mut s);
    set_bar(&mut s, h, 900.0);
    let mut ticks = 0;
    while bar(&s, h).0 > 0.0 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        ticks += 1;
        assert!(ticks < 60);
    }
    assert!((ticks as f32 * crate::TICK_DT - 1.0).abs() < 0.1, "{ticks}");
}

#[test]
fn the_stamina_cheat_keeps_the_bar_through_a_sprint() {
    let mut s = sim();
    let h = rolled(&mut s);
    s.cheats.infinite_stamina = true;
    for _ in 0..200 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    assert!(deka_of(&s, h).boosting);
    assert_eq!(bar(&s, h).0, 3000.0);
}

/// Bots do not sprint: their input never carries the button, even if a pad did.
#[test]
fn a_bot_droideka_gets_no_sprint() {
    let mut s = sim();
    let h = s
        .spawn(&deka(UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::player(0, Vec3::ZERO)
        }))
        .unwrap();
    let input = s
        .droideka_input(h.index as usize, &pad(buttons::SPRINT, 1.0))
        .unwrap();
    assert!(!input.boost);
}

/// The sprint in a replay: the same pads give the same hash on every tick, and
/// the hash sees the sprint, the bar and the lockout.
#[test]
fn a_sprint_replay_hashes_the_same_and_the_hash_sees_the_lockout() {
    let script = |t: u32| -> PadState {
        match t {
            0 => pad(buttons::ROLL_STANCE, 0.0),
            1..=30 => pad(0, 0.0),
            31..=200 => pad(buttons::SPRINT, 1.0),
            201 => pad(buttons::ROLL_STANCE, 0.0),
            _ => pad(buttons::SPRINT, 1.0),
        }
    };
    let run = || {
        let mut s = sim();
        s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
        (0..320u32)
            .map(|t| {
                s.step(&[script(t)]);
                s.hash()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());

    let one = |spent: bool| {
        let mut s = sim();
        let h = rolled(&mut s);
        s.units[h.index as usize]
            .droideka
            .as_mut()
            .unwrap()
            .sprint_spent = spent;
        s.hash()
    };
    assert_ne!(one(false), one(true));
}

/// The soldier's own sprint and energy stay out of it.
#[test]
fn a_sprinting_ball_does_not_leave_the_soldier_energy_or_stance_changed() {
    let mut s = sim();
    let h = rolled(&mut s);
    let before = s.unit(h).unwrap().soldier.energy;
    for _ in 0..60 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
    }
    let u = s.unit(h).unwrap();
    assert_eq!(u.soldier.energy, before);
    assert_ne!(u.soldier.stance, crate::soldier::Stance::Sprint);
}

/// The sprint's speed against a wall across its path: the ball stops at the
/// wall, at twice the speed as at one.
#[test]
fn a_sprinting_ball_does_not_pass_through_a_wall() {
    use crate::collision::{trimesh, Body, CollisionWorld, Shape};
    let mut s = sim();
    let z = -20.0;
    s.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::IDENTITY,
            Shape::Mesh(
                trimesh(
                    &[
                        [-50.0, -5.0, z],
                        [50.0, -5.0, z],
                        [50.0, 20.0, z],
                        [-50.0, 20.0, z],
                    ],
                    &[0, 1, 2, 0, 2, 3],
                )
                .unwrap(),
            ),
        )],
        Vec::new(),
    );
    let h = rolled(&mut s);
    let mut boosted = false;
    for _ in 0..90 {
        s.step(&[pad(buttons::SPRINT, 1.0)]);
        boosted |= deka_of(&s, h).boosting;
        let at = s.unit(h).unwrap().soldier.pos.z;
        assert!(at > z, "through the wall at {at}");
    }
    assert!(boosted);
    assert!(
        s.unit(h).unwrap().soldier.pos.z < z + 3.0,
        "never reached it"
    );
}

/// What a sprint covers on open ground, a full bar, against the plain roll.
#[test]
fn a_full_bar_sprint_covers_twice_the_plain_ground() {
    let run = |sprint: u32| {
        let mut s = sim();
        let h = rolled(&mut s);
        let from = s.unit(h).unwrap().soldier.pos;
        for _ in 0..100 {
            s.step(&[pad(sprint, 1.0)]);
        }
        s.unit(h).unwrap().soldier.pos.distance(from)
    };
    let (plain, sprint) = (run(0), run(buttons::SPRINT));
    println!("100 ticks: plain {plain} m, sprint {sprint} m");
    assert!(
        sprint > 1.8 * plain && sprint < 2.1 * plain,
        "{plain} {sprint}"
    );
}

fn bar(s: &Sim, h: UnitHandle) -> (f32, f32) {
    s.unit(h).unwrap().stamina_bar()
}

/// The maintainer: "The droideka stamina bar is how much shield is left.
/// removing the shield recharges it." BF2's numbers: 3000, -100/s up once it
/// is under full, +300/s down, and a full shield raised does not drain.
#[test]
fn the_stamina_bar_is_the_shield_and_lowering_it_recharges() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    s.step(&[pad(0, 0.0)]);
    assert_eq!(bar(&s, h), (3000.0, 3000.0));
    // Raised at full: no drain.
    s.step(&[pad(buttons::SECONDARY_FIRE, 0.0)]);
    for _ in 0..30 {
        s.step(&[pad(0, 0.0)]);
    }
    assert_eq!(bar(&s, h), (3000.0, 3000.0));
    // A hit takes the bar down by its damage, then it drains at 100/s.
    let d = s.units[h.index as usize].droideka.as_mut().unwrap();
    assert!(d.shield.on);
    assert_eq!(d.shield.absorb(1000.0, 1.0), 0.0);
    assert_eq!(bar(&s, h), (2000.0, 3000.0));
    let dt = crate::TICK_DT;
    for _ in 0..30 {
        s.step(&[pad(0, 0.0)]);
    }
    let up = 2000.0 - 100.0 * 30.0 * dt;
    assert!((bar(&s, h).0 - up).abs() < 0.5, "{}", bar(&s, h).0);
    // Lowered: 300/s up to the cap.
    s.step(&[pad(buttons::SECONDARY_FIRE, 0.0)]);
    let low = bar(&s, h).0;
    assert!(!s.unit(h).unwrap().droideka.as_ref().unwrap().shield.on);
    for _ in 0..30 {
        s.step(&[pad(0, 0.0)]);
    }
    let down = low + 300.0 * 30.0 * dt;
    assert!((bar(&s, h).0 - down).abs() < 0.5, "{}", bar(&s, h).0);
    for _ in 0..(10.0 / dt) as usize {
        s.step(&[pad(0, 0.0)]);
    }
    assert_eq!(bar(&s, h), (3000.0, 3000.0));
}

/// A bolt on the raised shield shows on the bar.
#[test]
fn a_bolt_on_the_raised_shield_lowers_the_bar() {
    let mut s = sim();
    s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    let t = target(&mut s, true);
    shoot(&mut s, 60);
    let (cur, max) = bar(&s, t);
    assert_eq!(max, 3000.0);
    assert!(cur < 3000.0, "{cur}");
    assert_eq!(
        cur,
        s.units[t.index as usize]
            .droideka
            .as_ref()
            .unwrap()
            .shield
            .points
    );
}

/// A soldier's bar stays his energy.
#[test]
fn a_soldiers_stamina_bar_is_his_energy() {
    let mut s = sim();
    let h = s.spawn(&UnitSpec::player(0, Vec3::ZERO)).unwrap();
    s.units[h.index as usize].soldier.energy = 37.0;
    let u = s.unit(h).unwrap();
    assert_eq!(u.stamina_bar(), (37.0, u.consts.energy_bar));
}

/// The press edge reads the droideka's own previous buttons: a droideka never
/// runs the soldier's step, so the soldier's stay at 0 and a held trigger would
/// read as a press every tick.
#[test]
fn a_held_trigger_on_an_empty_slot_clicks_once_for_a_human_droideka() {
    use crate::unit::SlotSpec;
    use crate::weapon::WeaponClass;
    let mut s = sim();
    let gun = s.add_weapon_class(WeaponClass::bf1_rifle());
    let spare = s.add_weapon_class(WeaponClass::bf1_rifle());
    let slot = |slot, channel, weapon_class| SlotSpec {
        slot,
        channel,
        weapon_class,
        clips: 0,
    };
    let h = s
        .spawn(&deka(UnitSpec {
            slots: Arc::new([slot(1, 0, gun), slot(2, 1, spare)]),
            ..UnitSpec::player(0, Vec3::ZERO)
        }))
        .unwrap();
    let i = h.index as usize;
    let k = s.units[i].cur1.unwrap() as usize;
    s.units[i].held[k].state.clip = 0;
    s.units[i].held[k].state.spare = 0;
    s.step(&[pad(0, 0.0)]);
    let mut clicks = 0;
    for _ in 0..10 {
        s.step(&[pad(buttons::SECONDARY_FIRE, 0.0)]);
        clicks += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::FireEmpty { .. }))
            .count();
    }
    assert_eq!(clicks, 1);
}

#[test]
fn shield_debt_is_not_extra_damage_or_erased_by_health_lending() {
    let mut s = sim();
    let h = s.spawn(&deka(UnitSpec::player(0, Vec3::ZERO))).unwrap();
    let u = s.unit_mut(h).unwrap();
    let desc = crate::weapon::DamageDesc {
        damage: 25.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    for on in [false, true] {
        u.droideka.as_mut().unwrap().shield = crate::droideka::Shield { points: -900.0, on };
        let before = u.health.cur;
        let taken = u.lend_droideka_shield(|health| health.take_damage(&desc, 1.0, false, 1.0));
        assert_eq!(taken, 25.0);
        assert_eq!(u.health.cur, before - 25.0);
        assert_eq!(u.droideka.as_ref().unwrap().shield.points, -900.0);
        assert_eq!(u.health.shield, 0.0);
    }
}
