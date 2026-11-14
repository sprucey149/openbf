//! BF1 Geonosis: the AT-TE's 3rd seat stops at its authored arc (playtest
//! 2026-11-14: "for the 3rd seat, it allows looking 180 degrees even though
//! the weapon has a angle limit which is enforced on the PCSX2"). BF1
//! rep_walk_atte authors that seat's YawLimits 120 230; BF1's setter flags
//! such a rear arc (B1:138731-138752) and its tick clamps it in [0, 2π)
//! (B1:137276-137303), as BF2 does. The port flagged it on BF2 guns only.
//! Mutation: `yaw_positive` back on `Slew::Linear` only -> the turn wraps.

use super::seat_camera::{map, missions_of};

#[test]
fn bf1_atte_rear_gun_holds_its_120_to_230_degree_arc() {
    let root = game::assets::baked_root();
    let dir = root.join("bf1_geo1");
    let mission = missions_of("bf1_geo1").into_iter().next();
    let Some(m) = mission.and_then(|mission| map(&dir, &mission.factions())) else {
        eprintln!("SKIP: bf1_geo1 is not baked");
        return;
    };
    let Some(class) = m.build.classes.iter().position(|c| c == "rep_walk_atte") else {
        eprintln!("SKIP: no rep_walk_atte on bf1_geo1");
        return;
    };
    let turn = m.walk.sim.seat_setup[class].as_ref().unwrap().guns[2]
        .as_ref()
        .unwrap()
        .turn
        .unwrap();
    assert!(turn.yaw_positive && !turn.yaw_wrap, "{turn:?}");
    let (lo, hi) = (120f32.to_radians(), 230f32.to_radians());
    assert!((turn.yaw[0] - lo).abs() < 1e-4 && (turn.yaw[1] - hi).abs() < 1e-4);
    for dir in [1.0f32, -1.0] {
        let (mut look, mut rate) = (turn.rest(), (0.0, 0.0));
        for _ in 0..600 {
            (look, rate) = turn.step(look, rate, (dir, 0.0), 1.0 / 30.0);
            assert!(look.0 >= lo - 1e-4 && look.0 <= hi + 1e-4, "{look:?}");
        }
        let end = if dir > 0.0 { lo } else { hi };
        assert!((look.0 - end).abs() < 1e-4, "{look:?}");
    }
}
