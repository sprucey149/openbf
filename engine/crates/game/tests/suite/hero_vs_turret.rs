//! The maintainer, 2026-10-30: "for auto turrets: yes I was talking about the
//! one the sniper drops. Make sure lightsabers and heroes can destroy them too.
//! I noticed the issue when I was darth maul and couldn't kill it."
//!
//! A real map and the real sniper's dispenser, the real hero's real attacks
//! through the pad, the reticle on the turret.

use super::auto_turret_damage::{lay_reticle, scene_on, Scene};
use super::turret_damage::spawn;
use core::autoturret::TurretEnd;
use core::sim::{buttons, Event, PadState};
use core::unit::Controller;

const MAP: &str = "bf2_cor1";

fn cis_hero(class: &str, dist: f32) -> Option<Scene> {
    scene_on(MAP, ["cis", "rep"], "rep_inf_ep3_sniper", class, dist)
}

fn pad(b: u32) -> [PadState; 2] {
    [
        PadState {
            buttons: b,
            ..Default::default()
        },
        PadState::default(),
    ]
}

/// Step `buttons(tick)` for `secs` with the reticle on the turret. Its hits by
/// the player, and how it ended.
fn attack(s: &mut Scene, secs: f32, press: impl Fn(u32) -> u32) -> (usize, Option<TurretEnd>) {
    let mut hits = 0;
    for tick in 0..(secs * core::TICK_HZ as f32) as u32 {
        lay_reticle(&mut s.w, s.me, s.centre);
        s.w.sim.step(&pad(press(tick)));
        let u = s.w.sim.unit_mut(s.me).unwrap();
        u.soldier.energy = u.consts.energy_bar;
        for e in &s.w.sim.events {
            match e {
                Event::TurretHit {
                    shooter, turret, ..
                } if *shooter == s.me && *turret == s.id => hits += 1,
                Event::TurretGone { end, .. } => return (hits, Some(*end)),
                _ => {}
            }
        }
    }
    (hits, None)
}

fn select(s: &mut Scene, want: impl Fn(&core::weapon::WeaponClass) -> bool) {
    let classes = s.w.sim.weapon_classes.clone();
    let u = s.w.sim.unit_mut(s.me).unwrap();
    let k = u
        .held
        .iter()
        .position(|h| want(&classes[h.class]))
        .expect("the hero's weapon");
    let slot = u.held[k].slot;
    for h in &mut u.held {
        h.state.active = h.slot == slot;
    }
    u.cur1 = Some(k as u8);
}

#[test]
fn darth_mauls_blade_destroys_the_snipers_turret() {
    let Some(mut s) = cis_hero("cis_hero_darthmaul", 1.2) else {
        return;
    };
    let (hits, end) = attack(
        &mut s,
        12.0,
        |t| if t % 12 == 0 { buttons::FIRE } else { 0 },
    );
    assert!(hits > 0, "no blade hit");
    assert_eq!(end, Some(TurretEnd::Destroyed));
}

/// Standing level with his head up, not looking down at it.
#[test]
fn a_swing_cuts_it_without_aiming_down_at_it() {
    let Some(mut s) = cis_hero("cis_hero_darthmaul", 1.2) else {
        return;
    };
    lay_reticle(&mut s.w, s.me, s.centre);
    s.w.sim.unit_mut(s.me).unwrap().soldier.pitch = 0.0;
    let mut gone = false;
    for t in 0..(12 * core::TICK_HZ) {
        s.w.sim
            .step(&pad(if t % 12 == 0 { buttons::FIRE } else { 0 }));
        gone |=
            s.w.sim
                .events
                .iter()
                .any(|e| matches!(e, Event::TurretGone { .. }));
    }
    assert!(gone);
}

#[test]
fn a_thrown_saber_turns_back_off_the_turret_and_hurts_it() {
    let Some(mut s) = cis_hero("cis_hero_darthmaul", 10.0) else {
        return;
    };
    select(&mut s, |c| c.saber_throw.is_some());
    let mut far = 0.0f32;
    let mut hit = false;
    for t in 0..(3 * core::TICK_HZ) {
        lay_reticle(&mut s.w, s.me, s.centre);
        s.w.sim
            .step(&pad(if t < 4 { buttons::SECONDARY_FIRE } else { 0 }));
        let u = s.w.sim.unit(s.me).unwrap();
        if let Some(f) = u.held.iter().find_map(|h| h.state.saber_flight.as_ref()) {
            far = far.max(f.pos.distance(u.soldier.pos));
        }
        hit |= s.w.sim.events.iter().any(|e| {
            matches!(e, Event::TurretHit { shooter, .. } if *shooter == s.me)
                || matches!(
                    e,
                    Event::TurretGone {
                        end: TurretEnd::Destroyed,
                        ..
                    }
                )
        });
    }
    assert!(hit, "the throw did not hurt it");
    assert!(
        (5.0..12.5).contains(&far),
        "turned back at the turret: {far}"
    );
    let u = s.w.sim.unit(s.me).unwrap();
    assert!(
        u.held.iter().all(|h| h.state.saber_flight.is_none()),
        "home"
    );
}

#[test]
fn mauls_thrown_saber_can_cut_a_soldier_again_after_the_turret_turns_it_back() {
    let Some(mut s) = cis_hero("cis_hero_darthmaul", 10.0) else {
        return;
    };
    select(&mut s, |c| c.saber_throw.is_some());
    let at = s.at.lerp(s.centre.with_y(s.at.y), 0.5);
    let victim = spawn(&mut s.w, "rep_inf_ep3_rifleman", 1, at, Controller::Idle);
    let soldier = s.w.sim.unit_mut(victim).unwrap();
    soldier.health.max = 10_000.0;
    soldier.health.cur = soldier.health.max;
    let mut hp = soldier.health.cur;
    let mut hits = [0; 2];
    let mut launches = 0;
    let mut stopped = false;
    for t in 0..(3 * core::TICK_HZ) {
        lay_reticle(&mut s.w, s.me, s.centre);
        s.w.sim
            .step(&pad(if t < 4 { buttons::SECONDARY_FIRE } else { 0 }));
        let after = s.w.sim.unit(victim).unwrap().health.cur;
        if after < hp {
            hits[usize::from(stopped)] += 1;
        }
        hp = after;
        if s.w.sim.events.iter().any(|e| {
            matches!(e, Event::TurretHit { shooter, turret, .. } if *shooter == s.me && *turret == s.id)
        }) {
            assert!(!stopped, "the turret takes only the outgoing contact");
            let flight = s.w.sim.unit(s.me).unwrap().held.iter()
                .find_map(|h| h.state.saber_flight.as_ref()).unwrap();
            assert!(flight.returning, "the turret turns the blade back");
            stopped = true;
        }
        launches +=
            s.w.sim
                .events
                .iter()
                .filter(|e| {
                    matches!(e, Event::Ability { unit, weapon_class, .. } if *unit == s.me
                && s.w.sim.weapon_classes[*weapon_class as usize].saber_throw.is_some())
                })
                .count();
    }
    assert!(stopped, "the throw reached the turret");
    assert_eq!(launches, 1);
    assert_eq!(hits, [1, 1]);
    assert!(s
        .w
        .sim
        .unit(s.me)
        .unwrap()
        .held
        .iter()
        .all(|h| h.state.saber_flight.is_none()));
}

#[test]
fn dookus_lightning_destroys_it() {
    let Some(mut s) = cis_hero("cis_hero_countdooku", 6.0) else {
        return;
    };
    select(&mut s, |c| {
        c.force.is_some() && c.name.contains("lightning")
    });
    let (hits, end) = attack(&mut s, 12.0, |t| {
        if t % 30 < 12 {
            buttons::SECONDARY_FIRE
        } else {
            0
        }
    });
    assert!(hits > 0, "no lightning hit");
    assert_eq!(end, Some(TurretEnd::Destroyed));
}

#[test]
fn a_force_push_with_authored_damage_reaches_it_as_its_data_says() {
    let Some(mut s) = cis_hero("cis_hero_darthmaul", 5.0) else {
        return;
    };
    select(&mut s, |c| c.force.is_some() && c.name.contains("push"));
    let cls =
        s.w.sim.unit(s.me).unwrap().held[s.w.sim.unit(s.me).unwrap().cur1.unwrap() as usize].class;
    let authored = s.w.sim.weapon_classes[cls].ordnance.charged(0.0).damage;
    let (hits, _) = attack(&mut s, 4.0, |t| {
        if t % 30 < 12 {
            buttons::SECONDARY_FIRE
        } else {
            0
        }
    });
    assert_eq!(hits > 0, authored > 0.0, "authored {authored}");
}

#[test]
fn maces_jump_attack_destroys_it() {
    let Some(mut s) = scene_on(
        MAP,
        ["rep", "cis"],
        "cis_inf_sniper",
        "rep_hero_macewindu",
        1.5,
    ) else {
        return;
    };
    let (hits, end) = attack(&mut s, 12.0, |t| match t % 60 {
        0 => buttons::JUMP,
        12 | 13 => buttons::FIRE,
        _ => 0,
    });
    assert!(hits > 0, "no jump attack hit");
    assert_eq!(end, Some(TurretEnd::Destroyed));
}
