//! Card CHEAT2 on a real weapon: BF1's stormtrooper rifle off the disc (a
//! 55-round clip, 220 in reserve), held on FIRE for 100 s by the player
//! through the sim, with `ammo`, `clip`, both and neither.

use core::sim::{buttons, Cheats, PadState, Sim};
use core::unit::Controller;
use game::roster::ClassRoster;
use glam::Vec3;

/// One tick after the step: the clip, the reserve, reloading, shots so far.
#[derive(Debug, Clone, Copy)]
struct Tick {
    clip: u32,
    spare: u32,
    reloading: bool,
    shots: u64,
}

fn hold_fire(cheats: Cheats) -> Option<Vec<Tick>> {
    let db = super::weapon_classdb::db(
        &[
            "bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL",
            "bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL",
        ],
        "cheat2",
    )?;
    let roster = ClassRoster::build(&db, "imp_inf_storm_trooper", 1, None).unwrap();
    let mut sim = Sim::empty(1);
    sim.heightfield = Some(core::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    let class = roster.register(&mut sim, &mut Vec::new());
    let spec = class.spec(0, Vec3::ZERO, 0.0, Controller::Player { slot: 0 }, 0, 0);
    let me = sim.spawn(&spec).unwrap();
    sim.players[0] = Some(me);
    sim.cheats = cheats;
    let fire = PadState {
        buttons: buttons::FIRE,
        ..Default::default()
    };
    let w = &sim.unit(me).unwrap().weapon;
    assert_eq!((w.clip, w.spare), (55, 220));
    let mut ticks = Vec::new();
    for _ in 0..100 * core::TICK_HZ {
        sim.step(&[fire]);
        let w = &sim.unit(me).expect("alive").weapon;
        ticks.push(Tick {
            clip: w.clip,
            spare: w.spare,
            reloading: w.reloading(),
            shots: w.shots_fired,
        });
    }
    Some(ticks)
}

/// The ticks on which a shot left.
fn shot_ticks(t: &[Tick]) -> Vec<usize> {
    let mut prev = 0;
    let mut out = Vec::new();
    for (i, k) in t.iter().enumerate() {
        if k.shots > prev {
            out.push(i);
        }
        prev = k.shots;
    }
    out
}

/// The longest wait between two shots.
fn longest_gap(t: &[Tick]) -> usize {
    let s = shot_ticks(t);
    s.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0)
}

#[test]
fn infinite_ammo_no_reload_and_both_on_the_stormtrooper_rifle() {
    let Some(plain) = hold_fire(Cheats::default()) else {
        return;
    };
    // No cheat: the clip empties, reloads from the reserve, and all 275
    // rounds go.
    let last = plain.last().unwrap();
    assert_eq!((last.clip, last.spare, last.shots), (0, 0, 275));
    assert!(plain.iter().any(|t| t.reloading));
    let first = plain.iter().position(|t| t.reloading).unwrap();
    let steady = longest_gap(&plain[..first]) + 1;
    let reload_gap = longest_gap(&plain);
    assert!(reload_gap > steady + 10, "{reload_gap} vs {steady}");

    // `ammo`: the reserve never moves; the clip still empties and reloads
    // (for free), so more than 275 rounds go.
    let ammo = hold_fire(Cheats {
        infinite_ammo: true,
        ..Default::default()
    })
    .unwrap();
    for (i, t) in ammo.iter().enumerate() {
        assert_eq!(t.spare, 220, "tick {i}");
    }
    assert!(ammo.iter().any(|t| t.clip == 0));
    assert!(ammo.iter().any(|t| t.reloading));
    let first_reload = ammo.iter().position(|t| t.reloading).unwrap();
    let refilled = ammo[first_reload..]
        .iter()
        .position(|t| !t.reloading)
        .unwrap();
    assert!(
        ammo[first_reload + refilled].clip >= 54,
        "full clip after it"
    );
    assert!(ammo.last().unwrap().shots > 275);
    assert!(
        longest_gap(&ammo).abs_diff(reload_gap) <= 1,
        "the same reload pause"
    );

    // `clip`: never a reload; the clip stays full while the reserve pays,
    // then empties; 275 rounds in one burst.
    let clip = hold_fire(Cheats {
        no_reload: true,
        ..Default::default()
    })
    .unwrap();
    let mut prev_total = 275;
    for (i, t) in clip.iter().enumerate() {
        assert!(!t.reloading, "tick {i}");
        if t.spare > 0 {
            assert_eq!(t.clip, 55, "tick {i}");
        }
        let total = t.clip + t.spare;
        assert_eq!(u64::from(275 - total), t.shots, "tick {i}");
        assert!(total <= prev_total);
        prev_total = total;
    }
    let last = clip.last().unwrap();
    assert_eq!((last.clip, last.spare, last.shots), (0, 0, 275));
    assert!(longest_gap(&clip) <= steady, "without stopping");

    // Both: nothing is drawn and nothing reloads.
    let both = hold_fire(Cheats {
        infinite_ammo: true,
        no_reload: true,
        ..Default::default()
    })
    .unwrap();
    for (i, t) in both.iter().enumerate() {
        assert_eq!((t.clip, t.spare, t.reloading), (55, 220, false), "tick {i}");
    }
    assert!(both.last().unwrap().shots > 275);
    assert!(longest_gap(&both) <= steady);
}
