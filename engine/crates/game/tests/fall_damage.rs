use core::sim::{Event, PadState};
use game::event_feed::{EventFeed, Texts};
use glam::Vec3;

#[path = "../examples/support/walker_battle.rs"]
mod walker_battle;

#[test]
#[ignore = "requires the locally baked Kashyyyk map and classes"]
fn kashyyyk_recorded_platform_drop_loses_health_and_flashes_red() {
    let root = game::assets::baked_root();
    let (mut walk, _) = walker_battle::battle_on(&root, "bf2_kas2", "bf2_kas2_con_cw").unwrap();
    let player = walk.player;
    let u = walk.sim.unit_mut(player).unwrap();
    assert!(u.mount.is_none());
    u.soldier.pos = Vec3::new(30.02, 18.41, -98.15);
    u.soldier.vel = Vec3::new(6.9, -17.4, 0.6);
    u.soldier.grounded = false;
    let health_before = u.health.cur;
    let mut feed = EventFeed::new(Texts::builtin(), ["rep", "cis"], &[]);
    let mut hits = 0;
    let mut flashed = false;
    for _ in 0..120 {
        walk.step_tick(&[PadState::default()]);
        feed.tick(&walk.sim, player);
        for event in &walk.sim.events {
            if let Event::FallDamage {
                victim,
                damage,
                health_left,
            } = *event
            {
                if victim == player {
                    hits += 1;
                    assert!(damage > 0.0 && health_left > 0.0);
                    flashed |= feed.damage_flash(walk.sim.tick, player) > 0.0;
                    assert!(feed.take_ambush().is_empty());
                    let u = walk.sim.unit(player).unwrap();
                    assert!((u.soldier.pos.y - 16.76).abs() < 0.2, "{:?}", u.soldier.pos);
                    println!("Kashyyyk landing: {damage} damage, {health_left} health remaining, red flash {flashed}");
                }
            }
        }
    }
    let u = walk.sim.unit(player).unwrap();
    assert!(u.soldier.grounded);
    assert!(u.health.cur > health_before * 0.5 && u.health.cur < health_before);
    assert_eq!(hits, 1);
    assert!(flashed);
}
