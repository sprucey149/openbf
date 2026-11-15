//! Playtest 2026-10-29, BF2 Geonosis, after the cannon's aimer fix: walking
//! round the Geonosian cannon with a bot in it, "the deflections are small
//! and fast like a vibration".
//!
//! A bot's seat gun followed its aim point with a one-tick (deadbeat) stick
//! and an aimer at its full rate, and that point moves in a step each time
//! the bot's aim block thinks (every 5th tick, `core::unit::AimBlock`): the
//! gun held still 4 ticks and jumped on the 5th. The PS2 hands a seated
//! bot's wanted direction through the AI's aim tracker first
//! (re-turret-draw 2.3, re-ai-difficulty 4.2), as a bot on foot's.
//!
//! Each case: a bot of the other side in a gun seat and a human standing in
//! front of it for 4 s, then walking. Measured from the 2nd second (after the
//! acquisition swing): the barrel's turn each tick and the "hold then jump"
//! count (a tick that turns over [`JUMP`] right after one under [`HOLD`]),
//! and the hits per shot.

use super::turret_damage::{spawn, turrets, world, World};
use core::sim::{Event, PadState};
use core::unit::{Controller, UnitHandle};
use core::vehicle::{VehicleHandle, VehicleKind};
use glam::Vec3;

/// A tick that turns less than this (deg) holds.
const HOLD: f32 = 0.05;
/// A tick that turns more than this (deg) right after a hold jumps.
const JUMP: f32 = 0.25;

struct Track {
    /// The worst turn of one tick (deg), standing and walking.
    still_worst: f32,
    walk_worst: f32,
    /// Hold-then-jump ticks, standing and walking.
    still_jumps: u32,
    walk_jumps: u32,
    /// The worst change of the turn from one tick to the next (deg).
    still_jerk: f32,
    shots: u64,
    hits: u32,
}

/// Run `gunner` in `vh`'s seat `seat` against a human standing `r` m out
/// along the gun's rest barrel for 120 ticks, then walking: round the gun
/// (`circle`) or across its front at 2 m/s.
fn track(
    w: &mut World,
    vh: VehicleHandle,
    gunner: UnitHandle,
    (human, team): (&str, u8),
    r: f32,
    circle: bool,
    ticks: u32,
) -> Track {
    let s = w.sim.seat_look(gunner).expect("a gun seat");
    let centre = if circle {
        w.sim.vehicle(vh).unwrap().pos
    } else {
        s.muzzle
    };
    let fwd = Vec3::new(s.barrel.x, 0.0, s.barrel.z).normalize();
    let side = fwd.cross(Vec3::Y);
    let a0 = fwd.x.atan2(fwd.z);
    let at = |k: u32, w: &World| {
        let t = k.saturating_sub(120) as f32 * core::TICK_DT;
        let mut p = if circle {
            let a = a0 + 4.0 / r * t;
            centre + Vec3::new(a.sin(), 0.0, a.cos()) * r
        } else {
            // Back and forth across the front, 8 m each way.
            let x = 2.0 * t;
            let x = (x % 16.0 - 8.0).abs() - 4.0;
            centre + fwd * r + side * x
        };
        p.y = w.sim.spawn_ground(p);
        p
    };
    let p0 = at(0, w);
    let me = spawn(w, human, team, p0, Controller::Player { slot: 0 });
    w.sim.cheats.invincible = true;
    let shots0: u64 = gun_shots(w, vh);
    let mut out = Track {
        still_worst: 0.0,
        walk_worst: 0.0,
        still_jumps: 0,
        walk_jumps: 0,
        still_jerk: 0.0,
        shots: 0,
        hits: 0,
    };
    let (mut prev, mut last) = (None::<Vec3>, 0.0f32);
    let mut series = Vec::new();
    for k in 0..ticks {
        let p = at(k, w);
        if let Some(u) = w.sim.unit_mut(me) {
            u.soldier.pos = p;
            u.soldier.vel = Vec3::ZERO;
        }
        w.sim.step(&[PadState::default()]);
        out.hits += w
            .sim
            .events
            .iter()
            .filter(|e| matches!(e, Event::Hit { shooter, victim, .. } if *shooter == gunner && *victim == me))
            .count() as u32;
        // A long run ends when the gunner leaves (his wait or the fight is over).
        let Some(b) = w.sim.seat_look(gunner).map(|s| s.barrel) else {
            assert!(k >= 600, "the gunner left at tick {k}");
            eprintln!("  the gunner left at tick {k}");
            break;
        };
        if let Some(q) = prev {
            let d = q.angle_between(b).to_degrees();
            series.push(d);
            if k >= 70 {
                let jump = d > JUMP && last < HOLD;
                if k < 120 {
                    out.still_worst = out.still_worst.max(d);
                    out.still_jumps += u32::from(jump);
                    out.still_jerk = out.still_jerk.max((d - last).abs());
                } else if k >= 130 {
                    out.walk_worst = out.walk_worst.max(d);
                    out.walk_jumps += u32::from(jump);
                }
            }
            last = d;
        }
        prev = Some(b);
    }
    out.shots = gun_shots(w, vh) - shots0;
    let row = |a: usize, b: usize| {
        series[a.min(series.len())..b.min(series.len())]
            .iter()
            .map(|d| format!("{d:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    eprintln!("  standing turns (deg/tick): {}", row(80, 110));
    eprintln!("  walking turns (deg/tick):  {}", row(200, 230));
    out
}

fn gun_shots(w: &World, vh: VehicleHandle) -> u64 {
    w.sim
        .vehicle(vh)
        .unwrap()
        .guns
        .iter()
        .flatten()
        .flat_map(|g| g.weapons.iter())
        .map(|w| w.shots_fired)
        .sum()
}

fn report(name: &str, t: &Track) {
    eprintln!(
        "{name}: standing worst {:.2} deg/tick, jerk {:.2}, {} hold-jumps; walking worst {:.2}, {} \
         hold-jumps; {} hits / {} shots",
        t.still_worst, t.still_jerk, t.still_jumps, t.walk_worst, t.walk_jumps, t.hits, t.shots
    );
}

/// A seated bot gunner of `team` in `vh`'s first non-pilot gun seat.
fn man(w: &mut World, vh: VehicleHandle, class: &str, team: u8) -> Option<UnitHandle> {
    let pos = w.sim.vehicle(vh)?.pos;
    let n = w.sim.vehicle(vh)?.seats.len();
    let bot = spawn(w, class, team, pos + Vec3::Y * 50.0, Controller::Ai);
    // A Normal bot (the helper spawns skill 0, read as level 1).
    w.sim.unit_mut(bot)?.skill = core::skill::Difficulty::Normal.base_skill();
    for seat in 0..n as u8 {
        if !w.sim.seat_unit(bot, vh, seat) {
            continue;
        }
        if w.sim.seat_look(bot).is_some_and(|s| !s.gun.pilot) {
            return Some(bot);
        }
        w.sim.unseat_unit(bot);
    }
    None
}

/// The bound every bot gun meets: on a standing man no tick turns the
/// barrel over 0.5 deg nor changes its turn by over 0.3 deg from the tick
/// before, and at a standing or walking man the turn never holds and then
/// jumps (before the fix, the cannon: 1.97 deg after 4 still ticks).
fn smooth(name: &str, t: &Track) {
    report(name, t);
    assert!(t.shots > 0, "{name}: the gunner fired");
    assert!(
        t.still_worst < 0.5,
        "{name}: a tick turned {} deg at a standing man",
        t.still_worst
    );
    assert!(
        t.still_jerk < 0.3,
        "{name}: the turn changed {} deg in a tick at a standing man",
        t.still_jerk
    );
    assert_eq!(
        t.still_jumps, 0,
        "{name}: the gun holds and jumps at a standing man"
    );
    assert_eq!(
        t.walk_jumps, 0,
        "{name}: the gun holds and jumps at a walking man"
    );
}

/// A few of the gunner's side far off, so the side is not wiped out.
fn far_friends(w: &mut World, at: Vec3, class: &str, team: u8) {
    for k in 0..4 {
        let far = at + Vec3::new(400.0 + 3.0 * k as f32, 300.0, 0.0);
        spawn(w, class, team, far, Controller::Ai);
    }
}

/// One case: the world, the gun, its gunner, the human (class, team), his
/// range, whether he circles, and a name.
type Case = (
    World,
    VehicleHandle,
    UnitHandle,
    (&'static str, u8),
    f32,
    bool,
    String,
);

/// The playtest's cannon, a man `r` m off, circling it or walking across.
fn geo_cannon(r: f32, circle: bool) -> Option<Case> {
    let mut w = world("bf2_geo1", ["rep", "cis"])?;
    let near = Vec3::new(142.28, -63.11, -0.63);
    let vh = turrets(&w, "tur_bldg_geoturret")
        .into_iter()
        .find(|&h| w.sim.vehicle(h).unwrap().pos.distance(near) < 1.0)
        .expect("the playtest's cannon");
    let at = w.sim.vehicle(vh).unwrap().pos;
    far_friends(&mut w, at, "cis_inf_rifleman", 1);
    let g = man(&mut w, vh, "cis_inf_rifleman", 1).expect("its gunner");
    let jet = ("rep_inf_ep2_jettrooper", 0);
    Some((w, vh, g, jet, r, circle, format!("geo1 cannon at {r} m")))
}

/// A Hoth dish turret, a man 20 m in front.
fn hoth_turret() -> Option<Case> {
    let mut w = world("bf2_hot1", ["all", "imp"])?;
    let (vh, g) = turrets(&w, "tur_bldg_hoth_dishturret")
        .into_iter()
        .find_map(|h| man(&mut w, h, "imp_inf_rifleman", 1).map(|g| (h, g)))
        .expect("a manned dish turret");
    let at = w.sim.vehicle(vh).unwrap().pos;
    far_friends(&mut w, at, "imp_inf_rifleman", 1);
    let name = format!(
        "hot1 {}",
        w.classes[w.sim.vehicle(vh).unwrap().class as usize]
    );
    Some((w, vh, g, ("all_inf_rifleman", 0), 20.0, false, name))
}

/// The first republic pad vehicle on Geonosis with a gunner's seat (the
/// fighter tank), a still driver, a man 15 m in front.
fn geo_vehicle() -> Option<Case> {
    let mut w = world("bf2_geo1", ["rep", "cis"])?;
    // The pads spawn their vehicles on the first tick.
    w.sim.step(&[]);
    let list: Vec<(VehicleHandle, String)> = w
        .sim
        .alive_vehicles()
        .filter(|(_, v)| v.kind != VehicleKind::Turret)
        .filter(|(_, v)| w.classes[v.class as usize].starts_with("rep_"))
        .map(|(h, v)| (h, w.classes[v.class as usize].clone()))
        .collect();
    let (vh, name, g) = list
        .iter()
        .find_map(|(h, n)| {
            // A driver who sits still, so the gunner stays a gunner.
            let pos = w.sim.vehicle(*h)?.pos + Vec3::Y * 60.0;
            let d = spawn(&mut w, "rep_inf_ep2_rifleman", 0, pos, Controller::Idle);
            if !w.sim.seat_unit(d, *h, 0) {
                return None;
            }
            man(&mut w, *h, "rep_inf_ep2_rifleman", 0).map(|g| (*h, n.clone(), g))
        })
        .expect("a vehicle with a gunner's seat");
    let at = w.sim.vehicle(vh).unwrap().pos;
    far_friends(&mut w, at, "rep_inf_ep2_rifleman", 0);
    let name = format!("geo1 {name} gunner");
    Some((w, vh, g, ("cis_inf_rifleman", 1), 15.0, false, name))
}

fn run(case: Option<Case>, ticks: u32) -> Option<(String, Track)> {
    let (mut w, vh, g, human, r, circle, name) = case?;
    Some((name, track(&mut w, vh, g, human, r, circle, ticks)))
}

#[test]
fn bot_guns_turn_smoothly_on_geonosis() {
    if let Some((name, t)) = run(geo_cannon(5.0, true), 600) {
        smooth(&name, &t);
    }
}

#[test]
fn bot_guns_turn_smoothly_hoth_turret() {
    if let Some((name, t)) = run(hoth_turret(), 600) {
        smooth(&name, &t);
    }
}

#[test]
fn bot_guns_turn_smoothly_vehicle_gun() {
    if let Some((name, t)) = run(geo_vehicle(), 600) {
        smooth(&name, &t);
    }
}

/// The smoothing keeps the bots' aim: over the three guns, the cannon at
/// three ranges, and 60 s each, the hits per shot. Before (the step-hold
/// aim, the same scenes): 113 hits / 219 shots = 0.52; at 12 m, 25 m, the
/// dish and the tank 85 / 191, now 84 / 191. At 5 m round the cannon the
/// gun used to fire only when a think's jump happened to meet his walk (28
/// shots); a barrel on last tick's point now counts (`gunner::aim`), so it
/// fires as at any range.
#[test]
fn bot_guns_keep_their_hit_rate() {
    let (mut hits, mut shots) = (0u32, 0u64);
    let cases = [
        geo_cannon(5.0, true),
        geo_cannon(12.0, false),
        geo_cannon(25.0, false),
        hoth_turret(),
        geo_vehicle(),
    ];
    for case in cases {
        let Some((name, t)) = run(case, 1800) else {
            return;
        };
        report(&name, &t);
        if name == "geo1 cannon at 5 m" {
            assert!(t.shots >= 40, "{name}: it fires at a man walking round it");
        }
        hits += t.hits;
        shots += t.shots;
    }
    let rate = hits as f32 / shots.max(1) as f32;
    eprintln!("bot guns: {hits} hits / {shots} shots = {rate:.3}");
    assert!(shots >= 200, "the guns fire: {shots}");
    assert!(rate > 0.45, "hits per shot {rate}");
}
