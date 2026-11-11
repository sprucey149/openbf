//! Hit registration at range (playtest 2026-10-28, BF1 Yavin 4: Arena):
//! "my target was directly on the guy and I'd see the bullet hit the guy but
//! nothing would happen [...] dead center of the guys body wouldn't do
//! anything, I'd have to aim for basically the shoulders to head".
//!
//! Each shot puts the reticle's ray on the enemy's DRAWN body (its skinned
//! pose's ribcage bone, or the belly between ribcage and pelvis) as the
//! window drew it - blended between the last two ticks, `render_blend` -
//! and the press is taken by the next tick, as in the windowed game. Two
//! causes of misses were found and are pinned here:
//! - past the reticle ray's 65.5 m reach an unscoped shot climbed over the
//!   reticle (0.35 m at 100 m, 0.65 m at 150 m) and flew over the head of a
//!   soldier aimed at in the chest (`Sim::aim_hit`'s pick distance, ours);
//! - a runner's drawn body is 1-2 ticks behind the one the bolt met (up to
//!   0.6 m at a run, past the torso prims' half width): a human's bolt now
//!   meets the soldiers where he saw them (`core::unit::HUMAN_SHOT_LAG`).
//!
//! Real classes and the real map from the bake; each case skips without it.

use core::sim::{buttons, Event, PadState};
use core::soldier::Stance;
use core::unit::{Controller, UnitHandle};
use glam::Vec3;

struct Range {
    walk: game::walk::WalkSim,
    vis: game::soldier_vis::SoldierVisuals,
    enemy: UnitHandle,
    /// The way from the shooter to the enemy, level.
    dir: Vec3,
}

/// `shooter` (team 0, side `sides[0]`) and an idle `target` (team 1) on
/// `map`, `dist` m apart on open ground, with a clear line from the
/// shooter's eye to the target's feet, chest and head, and level, clear
/// ground 4 m to either side of the target (where a runner crosses).
/// `None` without the bake.
fn range(
    map: &str,
    game: &'static str,
    sides: [&'static str; 2],
    shooter: &str,
    target: &str,
    dist: f32,
) -> Option<Range> {
    let root = game::assets::baked_root();
    let mut scene = game::assets::load_scene(&root.join(map)).ok()?;
    scene.apply_mode_layer("con");
    let db = game::roster::load_classdb(&root.join("classes"), game, &sides).ok()?;
    let mut sim = core::sim::Sim::empty(42);
    sim.ground_rules = core::soldier::GroundRules::for_game(game);
    scene.install_collision(&mut sim);
    let mut names = Vec::new();
    let team = |sim: &mut core::sim::Sim, names: &mut Vec<_>, class: &str, side: &str| {
        let roster = game::roster::ClassRoster::build(&db, class, 1, None).unwrap();
        let uc = roster.register(sim, names);
        let visual = game::walk::ClassVisual::from_roster(&roster);
        let faction = game::roster::FactionRoster {
            faction: side.into(),
            classes: vec![roster.clone()],
        };
        game::walk::TeamTemplate::from_roster(&faction, &uc, vec![visual])
    };
    let t0 = team(&mut sim, &mut names, shooter, sides[0]);
    let t1 = team(&mut sim, &mut names, target, sides[1]);
    let layout = game::layout::MapLayout::from_manifest(&scene.manifest);
    let at = layout.cps.first().map_or(Vec3::ZERO, |c| c.pos);
    let mut walk =
        game::walk::WalkSim::with_teams(sim, scene.heightfield(), at, [t0, t1.clone()], None);
    // The shots are aimed by hand: hit registration, not the assist (a
    // sniper has a magnet since `sniperassist`; `sniper_assist` tests it).
    walk.sim.assist_intensity = 0.0;
    let vis = game::soldier_vis::SoldierVisuals::load_cpu(&root, game, &walk.teams);
    let pi = walk.player.index as usize;
    let mut h = walk
        .sim
        .spawn(&t1.spec(1, at, 0.0, Controller::Idle, 0))
        .unwrap();
    let settle = |walk: &mut game::walk::WalkSim| {
        for _ in 0..40 {
            walk.step_tick(&[PadState::default()]);
        }
    };
    let tries = layout.cps.iter().flat_map(|c| {
        [
            Vec3::new(8.0, 1.0, 8.0),
            Vec3::new(-8.0, 1.0, -8.0),
            Vec3::new(30.0, 1.0, -30.0),
            Vec3::new(-30.0, 1.0, 30.0),
            Vec3::new(60.0, 1.0, 0.0),
            Vec3::new(-60.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 60.0),
            Vec3::new(0.0, 1.0, -60.0),
        ]
        .into_iter()
        .flat_map(move |off| (0..16).map(move |k| (c.pos + off, k)))
    });
    for (base, k) in tries {
        let a = k as f32 * std::f32::consts::FRAC_PI_8;
        let dir = Vec3::new(a.sin(), 0.0, a.cos());
        if walk.sim.unit(h).is_none() {
            h = walk
                .sim
                .spawn(&t1.spec(1, at, 0.0, Controller::Idle, 0))
                .unwrap();
        }
        if !walk.sim.units[pi].alive {
            break;
        }
        let u = walk.sim.unit_mut(h).unwrap();
        u.soldier.pos = base + dir * dist + Vec3::Y * 0.5;
        u.soldier.vel = Vec3::ZERO;
        settle(&mut walk);
        let Some(e) = walk.sim.unit(h).map(|u| u.soldier.pos) else {
            continue;
        };
        let p = &mut walk.sim.units[pi].soldier;
        p.pos = e - dir * dist + Vec3::Y * 0.5;
        p.vel = Vec3::ZERO;
        settle(&mut walk);
        if !walk.sim.units[pi].alive || walk.sim.unit(h).is_none() {
            continue;
        }
        let p = walk.sim.units[pi].soldier.pos;
        let side = Vec3::Y.cross(dir);
        let clear = [-4.0, -2.0, 0.0, 2.0, 4.0].iter().all(|&s| {
            let at = e + side * s;
            let level = (walk.sim.ground_under(at + Vec3::Y, false) - e.y).abs() < 0.3;
            level
                && [0.3, 1.0, 1.7].iter().all(|&y| {
                    core::perception::los_seat(
                        &walk.sim,
                        p + Vec3::Y * 2.1,
                        at + Vec3::Y * y,
                        core::collision::HullFilter::All,
                        [None, None],
                    )
                })
        });
        if (e.y - p.y).abs() < 1.5 && clear {
            let d = e - p;
            let dir = Vec3::new(d.x, 0.0, d.z).normalize();
            let mut r = Range {
                walk,
                vis,
                enemy: h,
                dir,
            };
            r.pose(0.1);
            return Some(r);
        }
    }
    panic!("{map}: no open lane of {dist} m near a post");
}

impl Range {
    fn player(&self) -> usize {
        self.walk.player.index as usize
    }

    /// Step the visuals `dt` with the sim as it stands.
    fn pose(&mut self, dt: f32) {
        self.vis.update(&self.walk, dt);
    }

    /// The enemy's drawn body: world triangles and bones.
    #[allow(clippy::type_complexity)]
    fn drawn(&self) -> (Vec<[Vec3; 3]>, Vec<(String, Vec3)>) {
        self.vis
            .drawn_body(&self.walk, self.enemy.index as usize)
            .expect("the enemy is drawn")
    }

    /// The enemy's drawn bone whose name contains `part`.
    fn bone(&self, part: &str) -> Vec3 {
        self.drawn()
            .1
            .into_iter()
            .find(|(n, _)| n.to_ascii_lowercase().contains(part))
            .map(|(_, p)| p)
            .unwrap_or_else(|| panic!("no {part} bone"))
    }

    /// Turn the player's reticle ray onto `p`.
    fn aim_at(&mut self, p: Vec3) {
        let pi = self.player();
        let w = &mut self.walk;
        for _ in 0..3 {
            let o = w.sim.units[pi].aim_ray(w.sim.ground_rules, 1).0;
            let d = p - o;
            let me = &mut w.sim.units[pi].soldier;
            me.pitch = d.y.atan2(Vec3::new(d.x, 0.0, d.z).length());
            me.yaw = (-d.x).atan2(-d.z);
        }
    }

    /// One tick with `b` held; the events it raised.
    fn tick(&mut self, b: u32) -> Vec<Event> {
        self.walk.step_tick(&[PadState {
            buttons: b,
            ..Default::default()
        }]);
        self.pose(core::TICK_DT);
        self.walk.sim.events.clone()
    }

    /// Press ZOOM until the view is the scope's.
    fn scope_in(&mut self) {
        for _ in 0..3 {
            self.tick(buttons::ZOOM);
            for _ in 0..15 {
                self.tick(0);
            }
            if self.walk.sim.unit(self.walk.player).unwrap().scoped() {
                return;
            }
        }
        panic!("no scope");
    }
}

/// Where a shot aims on the drawn body.
#[derive(Clone, Copy, Debug)]
enum Aim {
    /// The drawn ribcage bone: the chest's centre.
    Chest,
    /// Midway between the ribcage and the pelvis: the belly.
    Belly,
    /// 0.7 m beside the chest, the way he runs.
    Beside,
    /// 0.45 m over the head bone.
    Over,
}

/// One shot at the enemy, standing or running across the lane at `run` x
/// his class's MaxSpeed: the reticle on his drawn body as a frame `alpha`
/// of the way from the tick before the last to the last drew it, plus, if
/// `lead`, his motion over the bolt's flight (the player's own lead); the
/// press is taken by the next tick. The hit's damage, if it hit.
fn shot(r: &mut Range, run: f32, alpha: f32, aim: Aim, lead: bool) -> Option<f32> {
    let side = Vec3::Y.cross(r.dir).normalize();
    let run = r.walk.sim.unit(r.enemy).unwrap().consts.max_speed * run;
    let start = r.walk.sim.unit(r.enemy).unwrap().soldier.pos;
    // A full clip: the reload is not what is tested.
    let pi = r.player();
    let w = &mut r.walk.sim.units[pi].weapon;
    (w.clip, w.reload_timer) = (w.clip.max(50), 0.0);
    // Run up from behind the spot so the shot finds him on it. The run-up
    // covers most weapons' shot delay; a longer one (the sniper rifle's
    // second) passes first.
    const RUN_UP: u32 = 20;
    for _ in 0..120 {
        let w = &r.walk.sim.units[pi].weapon;
        if w.refire_time - w.refire_accum <= RUN_UP as f32 * core::TICK_DT {
            break;
        }
        r.tick(0);
    }
    let u = r.walk.sim.unit_mut(r.enemy).unwrap();
    if run > 0.0 {
        u.soldier.pos = start - side * (run * (RUN_UP + 1) as f32 * core::TICK_DT);
        u.controller = Controller::Pacer {
            vel: side * run,
            period: 0,
        };
    }
    u.health.cur = 1.0e6;
    for _ in 0..RUN_UP {
        r.tick(0);
    }
    let prev = r.walk.sim.unit(r.enemy).unwrap().soldier.pos;
    r.tick(0);
    let cur = r.walk.sim.unit(r.enemy).unwrap().soldier.pos;
    let rib = r.bone("ribcage");
    let part = match aim {
        Aim::Chest => rib,
        Aim::Belly => (rib + r.bone("pelvis")) * 0.5,
        Aim::Beside => rib + side * 0.7,
        Aim::Over => r.bone("head") + Vec3::Y * 0.45,
    } - cur;
    let me = r.walk.sim.unit(r.walk.player).unwrap();
    let v = r.walk.sim.weapon_classes[me.weapon_class].ordnance.velocity;
    let flight = (cur - me.soldier.pos).length() / v;
    let led = if lead { run * flight } else { 0.0 };
    r.aim_at(prev.lerp(cur, alpha) + part + side * led);
    let (mut got, mut fired) = (None, false);
    for k in 0..60 {
        let mut done = false;
        for e in r.tick(if k == 0 { buttons::FIRE } else { 0 }) {
            match e {
                Event::Hit { victim, damage, .. } if victim == r.enemy => {
                    got = Some(damage);
                    done = true;
                }
                Event::Shot { unit, .. } if unit == r.walk.player => fired = true,
                Event::BoltExpired { .. } => done = true,
                _ => {}
            }
        }
        if done {
            break;
        }
    }
    assert!(fired, "the shot fired");
    let u = r.walk.sim.unit_mut(r.enemy).unwrap();
    u.controller = Controller::Idle;
    u.soldier.pos = start;
    u.soldier.vel = Vec3::ZERO;
    for _ in 0..5 {
        r.tick(0);
    }
    got
}

const SIDES: [&str; 2] = ["all", "imp"];
const MAP: &str = "bf1_yav2";
const TROOPER: &str = "imp_inf_storm_trooper";

/// The 60 fps window's frames: one just after a tick, one half way.
const FRAMES: [f32; 2] = [0.0, 0.5];

/// A swing's frames: the drawn aim is that share into the next tick's turn.
const SWING_FRAMES: [f32; 4] = [0.0, 0.25, 0.5, 0.75];

/// A standing and a crouched soldier, shot in the drawn chest and belly:
/// the sniper rifle unscoped and scoped at 30-150 m, the rifle at 30 and
/// 60 m (past that its spread is the weapon's). Before the fix an unscoped
/// shot past 110 m flew over the head (0 of 6 at 120 and 150 m).
#[test]
fn chest_and_belly_shots_register_at_range() {
    for (shooter, scoped, dists) in [
        (
            "all_inf_marksman",
            false,
            &[30.0, 60.0, 100.0, 120.0, 150.0][..],
        ),
        ("all_inf_marksman", true, &[30.0, 60.0, 100.0, 150.0][..]),
        ("all_inf_soldier", false, &[30.0, 60.0][..]),
    ] {
        for &dist in dists {
            let Some(mut r) = range(MAP, "bf1", SIDES, shooter, TROOPER, dist) else {
                eprintln!("skip: no bake");
                return;
            };
            if scoped {
                r.scope_in();
            }
            for stance in [Stance::Stand, Stance::Crouch] {
                r.walk.sim.unit_mut(r.enemy).unwrap().soldier.stance = stance;
                for _ in 0..20 {
                    r.tick(0);
                }
                for aim in [Aim::Chest, Aim::Belly] {
                    let hit = shot(&mut r, 0.0, 0.5, aim, false);
                    assert!(
                        hit.is_some(),
                        "{shooter} scoped {scoped} at {dist} m, {stance:?} {aim:?}: missed"
                    );
                }
            }
        }
    }
}

/// A trooper crossing at his run speed, the sniper's reticle on his drawn
/// chest and belly with NO lead (a 2000 m/s round is instant to a player),
/// in four frames of a tick: every shot hits. Before the fix the frame
/// just after a tick (the drawn body 2 ticks behind the bolt's) missed at
/// 70 m and unscoped shots past 110 m flew over him; a round that met him
/// in its second tick (past 66.7 m) met him a tick further on than drawn.
#[test]
fn shots_at_a_runner_register_at_range() {
    for (scoped, dists) in [
        (false, &[30.0, 70.0, 75.0, 90.0, 120.0, 150.0][..]),
        (true, &[30.0, 70.0, 75.0, 90.0, 150.0][..]),
    ] {
        for &dist in dists {
            let Some(mut r) = range(MAP, "bf1", SIDES, "all_inf_marksman", TROOPER, dist) else {
                eprintln!("skip: no bake");
                return;
            };
            if scoped {
                r.scope_in();
            }
            let mut misses = Vec::new();
            for aim in [Aim::Chest, Aim::Belly] {
                for alpha in [0.0, 0.25, 0.5, 0.75] {
                    if shot(&mut r, 1.0, alpha, aim, false).is_none() {
                        misses.push(format!("{aim:?} frame {alpha}"));
                    }
                }
            }
            assert!(
                misses.is_empty(),
                "scoped {scoped} at {dist} m: missed {misses:?}"
            );
        }
    }
}

/// Shots beside the drawn body and over the head still miss, standing and
/// running, near and far.
#[test]
fn shots_beside_the_body_miss() {
    for dist in [30.0, 100.0, 150.0] {
        let Some(mut r) = range(MAP, "bf1", SIDES, "all_inf_marksman", TROOPER, dist) else {
            eprintln!("skip: no bake");
            return;
        };
        for run in [0.0, 1.0] {
            for aim in [Aim::Beside, Aim::Over] {
                for alpha in FRAMES {
                    assert_eq!(
                        shot(&mut r, run, alpha, aim, false),
                        None,
                        "at {dist} m, run {run}, {aim:?}, frame {alpha}: hit"
                    );
                }
            }
        }
    }
}

/// Möller-Trumbore, the nearest t.
fn ray_tri(o: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// The probe behind the finding that the prims are not the cause (not a
/// check): parallel rays along the lane on a 5 cm grid across each Yavin
/// class's drawn body, standing and crouched; `#` drawn and hit, `D` drawn
/// but no prim (the arms and the rifle held out), `P` a prim where nothing
/// is drawn. The drawn torso is inside the prims for every class.
#[test]
#[ignore = "probe: prints the census"]
fn probe_drawn_body_against_the_prims() {
    const ALL: [&str; 5] = [
        "all_inf_soldier",
        "all_inf_vanguard",
        "all_inf_pilot",
        "all_inf_marksman",
        "all_inf_smuggler",
    ];
    const IMP: [&str; 5] = [
        TROOPER,
        "imp_inf_shock_trooper",
        "imp_inf_pilotatst",
        "imp_inf_scout_trooper",
        "imp_inf_dark_trooper",
    ];
    for (shooter, targets, sides) in [
        ("all_inf_soldier", IMP, SIDES),
        (TROOPER, ALL, ["imp", "all"]),
    ] {
        for target in targets {
            let Some(mut r) = range(MAP, "bf1", sides, shooter, target, 30.0) else {
                eprintln!("skip: no bake");
                return;
            };
            for stance in [Stance::Stand, Stance::Crouch] {
                r.walk.sim.unit_mut(r.enemy).unwrap().soldier.stance = stance;
                for _ in 0..20 {
                    r.tick(0);
                }
                let (tris, _) = r.drawn();
                let u = r.walk.sim.unit(r.enemy).unwrap();
                let (prims, feet) = (u.bolt_prims(), u.soldier.pos);
                let side = Vec3::Y.cross(r.dir).normalize();
                let mut n = [0; 3];
                for iy in (0..23).rev() {
                    let mut row = String::new();
                    for ix in -10..=10 {
                        let o = feet + Vec3::Y * (iy as f32 * 0.1) + side * (ix as f32 * 0.1)
                            - r.dir * 20.0;
                        let dm = tris.iter().any(|t| ray_tri(o, r.dir, t).is_some());
                        let sm = prims.hit(o, r.dir, 40.0).is_some();
                        let (c, k) = match (dm, sm) {
                            (true, true) => ('#', 0),
                            (true, false) => ('D', 1),
                            (false, true) => ('P', 2),
                            _ => ('.', 3),
                        };
                        if k < 3 {
                            n[k] += 1;
                        }
                        row.push(c);
                    }
                    println!("{:4.1} {row}", iy as f32 * 0.1);
                }
                println!(
                    "{target} {stance:?}: both {}, drawn only {}, prims only {}",
                    n[0], n[1], n[2]
                );
            }
        }
    }
}

/// The probe behind the before / after table (not a check): hit (its
/// damage) or miss, per shooter, distance, standing / running, chest /
/// belly and frame.
#[test]
#[ignore = "probe: prints hit or miss"]
fn probe_table() {
    for (shooter, scoped) in [
        ("all_inf_marksman", false),
        ("all_inf_marksman", true),
        ("all_inf_soldier", false),
    ] {
        for dist in [40.0, 70.0, 85.0, 100.0, 120.0, 150.0] {
            let Some(mut r) = range(MAP, "bf1", SIDES, shooter, TROOPER, dist) else {
                eprintln!("skip: no bake");
                return;
            };
            if scoped {
                r.scope_in();
            }
            let mut out = String::new();
            for (run, label) in [(0.0, "still"), (1.0, "run")] {
                for aim in [Aim::Chest, Aim::Belly] {
                    let hits: Vec<String> = [0.0, 0.5, 0.99]
                        .iter()
                        .map(|&a| {
                            shot(&mut r, run, a, aim, false)
                                .map_or("-".into(), |d| format!("{d:.0}"))
                        })
                        .collect();
                    out += &format!(" {label}/{aim:?}:{}", hits.join(","));
                }
            }
            println!("{shooter} scoped {scoped} at {dist} m:{out}");
        }
    }
}

/// One shot from a controller swinging the reticle across a standing enemy
/// (2026-10-29): the right stick held at `rx` (the pad's value), the frame
/// `alpha` of the way into the next tick (the window's blend on), the press
/// in the frame whose DRAWN reticle (`render_blend::blend_in_looking` with
/// the held pad, as the windowed game draws it) is on `at` + the drawn
/// ribcage, stamped as the window stamps it (`Blend::stamp`: the press
/// fires at the share of the turn drawn). Hit or miss.
fn swing_shot(r: &mut Range, rx: f32, alpha: f32, at: Vec3) -> bool {
    const DT: f32 = core::TICK_DT;
    let pi = r.player();
    let w = &mut r.walk.sim.units[pi].weapon;
    (w.clip, w.reload_timer) = (w.clip.max(50), 0.0);
    r.walk.blend.on = true;
    let pad = |b: u32| PadState {
        buttons: b,
        rx: PadState::quantize(rx),
        ..Default::default()
    };
    // Past the last press's 1 s pull timer.
    for _ in 0..35 {
        r.tick(0);
    }
    // The frame `alpha` into the tick: the accumulator's left-over.
    let lag = (alpha * DT - r.walk.accum()).rem_euclid(DT);
    r.walk.advance(lag, &[PadState::default()]);
    let target = r.bone("ribcage") + at;
    // Turn back by the drawn offset until the frame after a stick tick
    // draws the reticle on the target.
    let mut back = 0.0;
    let mut err = f32::MAX;
    for _ in 0..8 {
        r.aim_at(target);
        let want = r.walk.sim.units[pi].soldier.yaw;
        r.walk.sim.units[pi].soldier.yaw = want + back;
        r.walk.advance(DT, &[pad(0)]);
        r.pose(DT);
        game::render_blend::blend_in_looking(&mut r.walk, &pad(0));
        let drawn = r.walk.sim.units[pi].soldier.yaw;
        r.walk.blend_out();
        err = drawn - want;
        if err.abs() < 1e-4 {
            break;
        }
        back -= err;
    }
    // 0.1 mrad: 8 mm at 80 m.
    assert!(
        err.abs() < 1e-4,
        "rx {rx} frame {alpha}: drawn off by {err}"
    );
    // The drawn frame's trigger is up (the window stamps every frame).
    r.walk.blend.stamp(pad(0));
    let mut b = buttons::FIRE;
    let (mut got, mut fired) = (false, false);
    for _ in 0..30 {
        let p = r.walk.blend.stamp(pad(b));
        let ticks = r.walk.advance(DT, &[p]);
        r.walk.blend.consumed(ticks);
        r.pose(DT);
        b = 0;
        let mut done = false;
        for e in r.walk.sim.events.clone() {
            match e {
                Event::Hit { victim, .. } if victim == r.enemy => {
                    got = true;
                    done = true;
                }
                Event::Shot { unit, .. } if unit == r.walk.player => fired = true,
                Event::BoltExpired { .. } => done = true,
                _ => {}
            }
        }
        if done {
            break;
        }
    }
    assert!(fired, "the shot fired");
    got
}

/// A controller's reticle is drawn where the next tick fires (the stick's
/// preview, `Sim::look_preview`; 2026-10-29): the marksman scoped at x2 and
/// x8 swings across a standing trooper at 40 and 80 m at a pad stick of
/// +-0.2 and +-0.5, assist off and on, and presses when the drawn reticle
/// meets the drawn ribcage, in frames 0, 1/4, 1/2 and 3/4 into the next
/// tick's turn (the press fires at that share): every shot hits. Before,
/// the shot left 1-2 ticks of turn past the drawn reticle (80 m x8 at 0.2:
/// 1 hit in 4).
#[test]
fn a_swinging_controller_hits_where_the_reticle_is_drawn() {
    for dist in [40.0, 80.0] {
        let Some(mut r) = range(MAP, "bf1", SIDES, "all_inf_marksman", TROOPER, dist) else {
            eprintln!("skip: no bake");
            return;
        };
        r.walk.sim.unit_mut(r.enemy).unwrap().health.cur = 1.0e6;
        r.scope_in();
        for step in 0..2 {
            if step == 1 {
                r.tick(buttons::ZOOM);
                for _ in 0..15 {
                    r.tick(0);
                }
            }
            let z = r.walk.sim.unit(r.walk.player).unwrap().zoom.unwrap().value;
            let mut misses = Vec::new();
            for intensity in [0.0, 1.0] {
                r.walk.sim.assist_intensity = intensity;
                for rx in [0.2, -0.2, 0.5, -0.5] {
                    for alpha in SWING_FRAMES {
                        r.walk.sim.unit_mut(r.enemy).unwrap().health.cur = 1.0e6;
                        if !swing_shot(&mut r, rx, alpha, Vec3::ZERO) {
                            misses.push(format!("assist {intensity} rx {rx} frame {alpha}"));
                        }
                    }
                }
            }
            r.walk.sim.assist_intensity = 0.0;
            assert!(misses.is_empty(), "{dist} m x{z}: missed {misses:?}");
        }
    }
}

/// The control: the same swing pressed with the drawn reticle 0.7 m beside
/// the drawn chest still misses (assist off: its pull would draw the shot
/// in).
#[test]
fn a_swinging_controller_beside_the_body_still_misses() {
    for dist in [40.0, 80.0] {
        let Some(mut r) = range(MAP, "bf1", SIDES, "all_inf_marksman", TROOPER, dist) else {
            eprintln!("skip: no bake");
            return;
        };
        r.walk.sim.unit_mut(r.enemy).unwrap().health.cur = 1.0e6;
        r.scope_in();
        let side = Vec3::Y.cross(r.dir).normalize() * 0.7;
        for rx in [0.2, -0.2, 0.5, -0.5] {
            for alpha in SWING_FRAMES {
                for at in [side, -side] {
                    assert!(
                        !swing_shot(&mut r, rx, alpha, at),
                        "{dist} m rx {rx} frame {alpha} beside {at}: hit"
                    );
                }
            }
        }
    }
}
