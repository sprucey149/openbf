//! Deterministic time-to-kill traces for the P4 Tier-4 artifact
//! (engine/artifacts/p4_combat/). Prints CSV to stdout:
//!   scenario,tick,time_s,event,damage,dist_m,cum_damage,victim_hp
//! Scenarios reproduce analysis/re-blaster-damage.md §8/§9 numbers inside the
//! full sim (fire cycle + bolt flight + TakeDamage), not the unit tests.
#![allow(clippy::disallowed_methods)] // f32::max/min: a harness, not the LAN sim (LAN_MULTIPLAYER.md §7)

use core::sim::{buttons, Event, PadState, Sim};
use core::unit::{Controller, UnitSpec};
use core::weapon::WeaponClass;
use glam::Vec3;

fn fire() -> PadState {
    PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    }
}

/// Human shooter vs a stationary trooper `dist` m ahead; fire held from t=0.
fn duel(name: &str, weapon: usize, dist: f32, victim_hp: f32) {
    let mut s = Sim::empty(1);
    if weapon == 1 {
        s.add_weapon_class(WeaponClass::bf2_rifle());
    }
    s.spawn(&UnitSpec {
        weapon_class: weapon,
        ..UnitSpec::player(0, Vec3::ZERO)
    })
    .unwrap();
    let t = s
        .spawn(&UnitSpec {
            max_health: victim_hp,
            ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -dist))
        })
        .unwrap();
    let speed = s.weapon_classes[weapon].ordnance.velocity;
    let mut cum = 0.0;
    for _ in 0..(30 * 12) {
        s.step(&[fire()]);
        let tick = s.tick - 1;
        let time = tick as f32 / 30.0;
        for e in &s.events {
            match e {
                Event::Shot { .. } => {
                    println!("{name},{tick},{time:.3},shot,,,{cum:.1},{:.1}", hp(&s, t))
                }
                Event::Hit { damage, age, .. } => {
                    cum += damage;
                    println!(
                        "{name},{tick},{time:.3},hit,{damage:.1},{:.1},{cum:.1},{:.1}",
                        age.max(0.0) * speed,
                        hp(&s, t)
                    );
                }
                Event::Kill { .. } => println!("{name},{tick},{time:.3},kill,,,{cum:.1},0.0"),
                _ => {}
            }
        }
        if s.unit(t).is_none() {
            break;
        }
    }
}

fn hp(s: &Sim, t: core::unit::UnitHandle) -> f32 {
    s.units[t.index as usize].health.cur.max(0.0)
}

/// Two AIs facing each other at `dist` m.
fn ai_duel(dist: f32, seed: u32) {
    let name = format!("ai_duel_{dist:.0}m_seed{seed}");
    let mut s = Sim::empty(seed);
    s.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(0, Vec3::ZERO)
    })
    .unwrap();
    s.spawn(&UnitSpec {
        controller: Controller::Ai,
        yaw: std::f32::consts::PI,
        ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -dist))
    })
    .unwrap();
    let (mut shots, mut hits) = (0, 0);
    for _ in 0..(30 * 20) {
        s.step(&[PadState::default()]);
        let tick = s.tick - 1;
        let time = tick as f32 / 30.0;
        for e in &s.events {
            match e {
                Event::Shot { .. } => shots += 1,
                Event::Hit { damage, age, .. } => {
                    hits += 1;
                    println!(
                        "{name},{tick},{time:.3},hit,{damage:.1},{:.1},,",
                        age.max(0.0) * 80.0
                    );
                }
                Event::Kill { victim, .. } => {
                    println!(
                        "{name},{tick},{time:.3},kill,,,,unit{} ({} shots / {} hits so far)",
                        victim.index, shots, hits
                    );
                    return;
                }
                _ => {}
            }
        }
    }
    println!("{name},,,no_kill_in_20s,,,,{shots} shots / {hits} hits");
}

fn main() {
    println!("scenario,tick,time_s,event,damage,dist_m,cum_damage,victim_hp");
    duel("bf1_rifle_vs_300hp_20m", 0, 20.0, 300.0);
    duel("bf1_rifle_vs_300hp_100m", 0, 100.0, 300.0);
    duel("bf2_rifle_vs_400hp_20m", 1, 20.0, 400.0);
    duel("bf2_rifle_vs_400hp_80m", 1, 80.0, 400.0);
    duel("bf2_rifle_vs_400hp_100m", 1, 100.0, 400.0);
    ai_duel(25.0, 21);
    ai_duel(40.0, 21);
}
