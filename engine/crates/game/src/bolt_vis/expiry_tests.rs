use super::*;
use core::collision::HullFilter;
use core::projectile::{Bolt, Shooter};

fn expiry(fade: f32) -> (WalkSim, BoltVis, Vec3) {
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    let mut w = core::weapon::WeaponClass::bf1_rifle();
    w.ordnance.laser.as_mut().unwrap().fade_out_rate = fade;
    let class = walk.sim.add_weapon_class(w);
    let b = Bolt::fire(
        Vec3::Y * 30.0,
        Vec3::Z,
        2000.0,
        0.15,
        Shooter {
            owner: walk.player,
            team: 0,
            weapon_class: class,
            is_player: true,
        },
        None,
        None,
        &walk.sim.collision,
        HullFilter::Static,
    );
    let end = b.end;
    walk.sim.bolts.push(b);
    let mut vis = BoltVis::default();
    vis.track(&walk);
    (walk, vis, end)
}

fn run_to_expiry(walk: &mut WalkSim, vis: &mut BoltVis) {
    for _ in 0..6 {
        walk.sim.step(&[]);
        vis.update(walk, core::TICK_DT);
        if walk.sim.bolts.is_empty() {
            return;
        }
    }
    panic!("fixture never expired");
}

#[test]
fn air_expiry_sniper_fades_at_its_end_for_one_second() {
    let (mut walk, mut vis, end) = expiry(1.0);
    run_to_expiry(&mut walk, &mut vis);
    assert_eq!(vis.remnant_count(), 1);
    assert!(vis.remnants[0].head.abs_diff_eq(end, 1e-4));
    assert!(!vis.remnants[0].terrain);
    vis.age(&walk, 0.5);
    assert_eq!(vis.remnants[0].fade, 0.5);
    vis.age(&walk, 0.5);
    assert_eq!(vis.remnant_count(), 0);
}

#[test]
fn air_expiry_rifle_stays_two_updates() {
    let (mut walk, mut vis, _) = expiry(0.0);
    run_to_expiry(&mut walk, &mut vis);
    assert_eq!(vis.remnant_count(), 1);
    vis.age(&walk, core::TICK_DT);
    assert_eq!(vis.remnant_count(), 1);
    vis.age(&walk, core::TICK_DT);
    assert_eq!(vis.remnant_count(), 0);
}

#[test]
fn removing_a_round_before_air_expiry_leaves_no_remnant() {
    let (mut walk, mut vis, _) = expiry(1.0);
    walk.sim.bolts.clear();
    vis.track(&walk);
    assert_eq!(vis.remnant_count(), 0);
}

#[test]
fn removal_on_the_last_step_without_an_expiry_event_leaves_no_remnant() {
    let (mut walk, mut vis, end) = expiry(1.0);
    let b = &mut walk.sim.bolts[0];
    b.remaining = core::TICK_DT * 0.5;
    b.pos = end - b.vel * b.remaining;
    vis.track(&walk);
    walk.sim.bolts.clear();
    vis.track(&walk);
    assert_eq!(vis.remnant_count(), 0);
}
