//! The maintainer, 2026-10-30: "The auto turrets: Can those not be destroyed
//! yet? They don't have health and attacking them doesn't do anything. They
//! should have health and be attackable like the original games had."
//!
//! A real map (BF2 Naboo), the real sniper's dispenser, the real weapons: a
//! player lays his reticle on an enemy's auto turret and fires as he would in
//! play. See `analysis/re-portable-turret.md` section 10.

use super::turret_damage::{spawn, world, World};
use core::autoturret::TurretEnd;
use core::sim::{buttons, AimTarget, Event, PadState};
use core::soldier::Stance;
use core::unit::{Controller, UnitHandle};
use glam::Vec3;

/// Flat open ground on the map: 40 m clear along +X at chest and head height.
pub(super) fn open_ground(w: &World) -> Option<Vec3> {
    let sim = &w.sim;
    for x in (-250..250).step_by(10) {
        for z in (-250..250).step_by(10) {
            let p = Vec3::new(x as f32, 500.0, z as f32);
            let at = |dx: f32| {
                let q = p + Vec3::X * dx;
                q.with_y(sim.spawn_ground(q))
            };
            let base = at(0.0);
            if base.y < -100.0 || (0..=8).any(|k| (at(k as f32 * 5.0).y - base.y).abs() > 0.15) {
                continue;
            }
            let up = Vec3::Y * 1.5;
            if core::perception::los(sim, base + up, at(40.0) + up)
                && core::perception::los(sim, base + up + Vec3::Y, at(40.0) + up + Vec3::Y)
            {
                return Some(base);
            }
        }
    }
    None
}

pub(super) struct Scene {
    pub(super) w: World,
    pub(super) me: UnitHandle,
    pub(super) centre: Vec3,
    pub(super) id: u64,
    pub(super) at: Vec3,
}

/// `class` of team 0 in player slot 0 at `dist` m from a landed turret of a
/// team 1 sniper (player slot 1, sent away after the throw).
fn scene(class: &str, dist: f32) -> Option<Scene> {
    scene_on("bf2_nab2", ["rep", "cis"], "cis_inf_sniper", class, dist)
}

/// `scene` on any map and sides; `sniper` is team 1's dropper.
pub(super) fn scene_on(
    map: &str,
    sides: [&str; 2],
    sniper: &str,
    class: &str,
    dist: f32,
) -> Option<Scene> {
    let mut w = world(map, sides)?;
    // Coruscant is all floors under a roof: its round hall's, as the captures use.
    let p = if map == "bf2_cor1" {
        Vec3::new(-48.0, 3.0, -108.0)
    } else {
        open_ground(&w).unwrap_or_else(|| panic!("no open ground on {map}"))
    };
    let me = spawn(&mut w, class, 0, p, Controller::Player { slot: 0 });
    let far = p + Vec3::X * dist;
    let foe = spawn(&mut w, sniper, 1, far, Controller::Player { slot: 1 });
    let weapons = w.sim.weapon_classes.clone();
    let u = w.sim.unit_mut(foe).unwrap();
    let k = u
        .held
        .iter()
        .position(|h| weapons[h.class].turret.is_some())
        .expect("the sniper's dispenser");
    let hslot = u.held[k].slot;
    for h in &mut u.held {
        h.state.active = h.slot == hslot;
    }
    u.cur1 = Some(k as u8);
    let pad = |b: u32| PadState {
        buttons: b,
        ..Default::default()
    };
    w.sim.step(&[pad(0), pad(buttons::SECONDARY_FIRE)]);
    for _ in 0..240 {
        w.sim.step(&[pad(0), pad(0)]);
        if w.sim.turrets.first().is_some_and(|t| t.landed) {
            break;
        }
    }
    let t = w.sim.turrets.first().expect("placed");
    assert!(t.landed, "landed");
    let (centre, id) = (t.centre(), t.id);
    // The thrower goes away; only the turret is left to shoot at.
    let u = w.sim.unit_mut(foe).unwrap();
    u.soldier.pos = p + Vec3::X * 200.0;
    // The player outlives the turret's own gun.
    let u = w.sim.unit_mut(me).unwrap();
    u.health.max = 1e7;
    u.health.cur = 1e7;
    Some(Scene {
        w,
        me,
        centre,
        id,
        at: p,
    })
}

/// Lay the reticle's ray (what the player sees) on `at`.
pub(super) fn lay_reticle(w: &mut World, me: UnitHandle, at: Vec3) {
    let yaw = |d: Vec3| (-d.x).atan2(-d.z);
    for _ in 0..6 {
        let (rules, views) = (w.sim.ground_rules, w.sim.views());
        let (a, d) = w.sim.unit(me).unwrap().aim_ray(rules, views);
        let want = (at - a).normalize();
        let pi = std::f32::consts::PI;
        let s = w.sim.unit_mut(me).unwrap();
        s.soldier.pitch += want.y.asin() - d.y.asin();
        s.soldier.yaw += (yaw(want) - yaw(d) + pi).rem_euclid(2.0 * pi) - pi;
    }
}

/// Hold FIRE with the reticle on the turret until it is gone or `secs` pass.
/// The health seen each tick, and the end if it came.
fn fire_on_it(s: &mut Scene, secs: f32) -> (Vec<f32>, Option<TurretEnd>) {
    let mut seen = Vec::new();
    for tick in 0..(secs * core::TICK_HZ as f32) as u32 {
        lay_reticle(&mut s.w, s.me, s.centre);
        let b = if tick % 2 == 0 { buttons::FIRE } else { 0 };
        let pad = PadState {
            buttons: b,
            ..Default::default()
        };
        s.w.sim.step(&[pad, PadState::default()]);
        for e in &s.w.sim.events {
            if let Event::TurretGone { end, .. } = e {
                return (seen, Some(*end));
            }
        }
        if let Some(t) = s.w.sim.turrets.iter().find(|t| t.id == s.id) {
            seen.push(t.health);
        }
    }
    (seen, None)
}

/// The maintainer's report, as played: the reticle dead on the turret, a
/// rifle's bolts. 80 at building scale 0.1 = 8 a bolt on its 30 health
/// (ours, `normalize::SHARED_HEALTH`; the disc's 50 took 7).
#[test]
fn a_rifle_on_the_reticle_destroys_an_enemy_auto_turret() {
    let Some(mut s) = scene("rep_inf_ep3_rifleman", 15.0) else {
        return;
    };
    let (seen, end) = fire_on_it(&mut s, 10.0);
    let low = seen.iter().copied().fold(f32::MAX, f32::min);
    assert_eq!(end, Some(TurretEnd::Destroyed), "health only got to {low}");
}

/// A rocket at the reticle: one is enough.
#[test]
fn a_rocket_on_the_reticle_destroys_an_enemy_auto_turret() {
    let Some(mut s) = scene("rep_inf_ep3_rocketeer", 20.0) else {
        return;
    };
    let (_, end) = fire_on_it(&mut s, 6.0);
    assert_eq!(end, Some(TurretEnd::Destroyed));
}

/// Crouched, as a soldier fights: the fire point is well under the reticle's
/// ray, and a bolt aimed at the ground behind the turret passes under its
/// 0.25 m sphere unless the ray stops on it (what the maintainer saw: 8 HP
/// from a whole clip at 6-30 m, no sign of a hit).
#[test]
fn crouched_on_the_reticle_a_rifle_still_destroys_it() {
    for dist in [6.0, 15.0, 30.0] {
        let Some(mut s) = scene("rep_inf_ep3_rifleman", dist) else {
            return;
        };
        s.w.sim.unit_mut(s.me).unwrap().soldier.stance = Stance::Crouch;
        let (seen, end) = fire_on_it(&mut s, 6.0);
        let low = seen.iter().copied().fold(f32::MAX, f32::min);
        assert_eq!(end, Some(TurretEnd::Destroyed), "at {dist} m: {low} left");
    }
}

/// A rocket from beyond 30 m, crouched: the launcher's flat flight stops on it.
#[test]
fn a_far_crouched_rocket_destroys_it() {
    let Some(mut s) = scene("rep_inf_ep3_rocketeer", 38.0) else {
        return;
    };
    s.w.sim.unit_mut(s.me).unwrap().soldier.stance = Stance::Crouch;
    let (_, end) = fire_on_it(&mut s, 6.0);
    assert_eq!(end, Some(TurretEnd::Destroyed));
}

/// A team 0 bot of `class` where the player stood, the only soldier about,
/// for up to 30 s: how the turret ended, and whether it ever was his target.
fn bot_at_it(class: &str) -> Option<(Option<TurretEnd>, bool)> {
    let mut s = scene(class, 20.0)?;
    let bot = spawn(&mut s.w, class, 0, s.at, Controller::Ai);
    s.w.sim.unit_mut(s.me).unwrap().soldier.pos = s.at + Vec3::X * 400.0;
    let b = s.w.sim.unit_mut(bot).unwrap();
    b.health.max = 1e7;
    b.health.cur = 1e7;
    b.soldier.yaw = -std::f32::consts::FRAC_PI_2;
    let (mut end, mut taken) = (None, false);
    for _ in 0..30 * core::TICK_HZ {
        s.w.sim.step(&[PadState::default(), PadState::default()]);
        let target = s.w.sim.unit(bot).and_then(|u| u.ai.target);
        taken |= target.and_then(|t| t.turret()) == Some(s.id);
        end = end.or(s.w.sim.events.iter().find_map(|e| match e {
            Event::TurretGone { end, .. } => Some(*end),
            _ => None,
        }));
        if end.is_some() {
            break;
        }
    }
    Some((end, taken))
}

/// The original's rule, by the weapon's `AITarget*` mask (B2 `FUN_002aaba0`
/// 230622, the disc's data): the rocket launcher authors `AITargetBuilding 1`,
/// so its bot takes the turret and destroys it; the rifle, pistol and grenade
/// author it 0, so a rifleman walks by.
#[test]
fn a_rocketeer_bot_shoots_it_down_and_a_rifleman_bot_does_not_try() {
    let Some((end, taken)) = bot_at_it("rep_inf_ep3_rocketeer") else {
        return;
    };
    assert!(taken, "the rocketeer never took it");
    assert_eq!(end, Some(TurretEnd::Destroyed));
    let (end, taken) = bot_at_it("rep_inf_ep3_rifleman").unwrap();
    assert!(!taken, "a rifle's mask refuses a building");
    assert_ne!(end, Some(TurretEnd::Destroyed));
}

/// Its own team's fusion cutter mends a hurt one (the amount table by health
/// type); it never has a wreck to rebuild, it is gone (`FUN_0022d1e0`).
#[test]
fn a_friends_cutter_mends_a_hurt_one() {
    let Some(mut s) = scene("cis_inf_engineer", 2.0) else {
        return;
    };
    // The engineer is of the turret's team.
    let me = s.me;
    s.w.sim.unit_mut(me).unwrap().team = 1;
    let weapons = s.w.sim.weapon_classes.clone();
    let u = s.w.sim.unit_mut(me).unwrap();
    let k = u
        .held
        .iter()
        .position(|h| weapons[h.class].repair.is_some())
        .expect("the engineer's cutter");
    let slot = u.held[k].slot;
    for h in &mut u.held {
        h.state.active = h.slot == slot;
    }
    if u.held[k].channel == 1 {
        u.cur1 = Some(k as u8);
    } else {
        u.weapon_class = u.held[k].class;
        u.weapon_slot = slot;
    }
    let t = s.w.sim.turrets.iter_mut().find(|t| t.id == s.id).unwrap();
    t.health = 10.0;
    let channel1 = s.w.sim.unit(me).unwrap().cur1 == Some(k as u8);
    let button = if channel1 {
        buttons::SECONDARY_FIRE
    } else {
        buttons::FIRE
    };
    let mut best = 10.0f32;
    for _ in 0..3 * core::TICK_HZ {
        lay_reticle(&mut s.w, me, s.centre);
        let pad = PadState {
            buttons: button,
            ..Default::default()
        };
        s.w.sim.step(&[pad, PadState::default()]);
        if let Some(t) = s.w.sim.turrets.iter().find(|t| t.id == s.id) {
            best = best.max(t.health);
        }
    }
    assert!(best > 10.0, "the cutter did nothing: {best}");
    assert!(best <= 30.0, "past its MaxHealth (ours, 30): {best}");
}

/// The class sweep: BF2 Hoth's placed self-piloted tripod guns are armed
/// buildings with authored health and fall as any other (the 1-flag layer; no
/// conquest world places one, and card VSELF is what would make them fire).
#[test]
fn a_placed_tripod_gun_has_health_and_falls() {
    let Some(mut w) = super::turret_damage::world_mode("bf2_hot1", ["all", "imp"], "ctf") else {
        return;
    };
    let all = super::turret_damage::turrets(&w, "tur_bldg_chaingun_tripod");
    assert!(!all.is_empty(), "no tripod in Hoth's 1-flag layer");
    let vh = all[0];
    let v = w.sim.vehicle(vh).unwrap().clone();
    assert!(v.health.max > 0.0, "{}", v.health.max);
    assert!(w.sim.vehicle_sphere(vh).is_some());
    w.sim.vehicle_mut(vh).unwrap().health.cur = 1.0;
    let e = core::weapon::ExplosionClass {
        damage: 100.0,
        damage_inner: 4.0,
        damage_outer: 4.0,
        ..Default::default()
    };
    let (c, _) = w.sim.vehicle_sphere(vh).unwrap();
    w.sim.explode(&e, c, None, Some(0));
    w.sim.step(&[]);
    assert!(w.sim.vehicle(vh).is_none(), "still standing");
}

/// The reticle picks it as the PS2's acquisition does (its name and health on
/// the HUD, the enemy colour).
#[test]
fn the_reticle_picks_an_enemy_auto_turret() {
    let Some(mut s) = scene("rep_inf_ep3_rifleman", 15.0) else {
        return;
    };
    lay_reticle(&mut s.w, s.me, s.centre);
    s.w.sim.step(&[PadState::default(), PadState::default()]);
    assert_eq!(
        s.w.sim.reticle_pick[0],
        Some((AimTarget::Turret(s.id), -1)),
        "the pick"
    );
    let feed = game::event_feed::EventFeed::default();
    let t = feed.target(&s.w.sim, 0).expect("the HUD's target");
    assert_eq!(t.name, "Auto Turret");
    assert_eq!((t.health_fraction, t.relation), (1.0, -1));
}

/// A real land mine's blast a step away; its owner's rule is the shared blast
/// (the thermal detonator's goes the same way, `core` `turrets::tests`).
#[test]
fn a_mine_blast_destroys_it() {
    let Some(mut s) = scene("cis_inf_engineer", 15.0) else {
        return;
    };
    let at = s.centre + Vec3::X;
    let weapons = s.w.sim.weapon_classes.clone();
    let mine = weapons
        .iter()
        .find_map(|w| w.mine.as_ref())
        .expect("the engineer's mines");
    let blast = mine
        .item
        .explosion(core::mine::Detonation::Trigger)
        .expect("its blast")
        .clone();
    s.w.sim.explode(&blast, at, Some(s.me), Some(0));
    let left =
        s.w.sim
            .turrets
            .iter()
            .find(|t| t.id == s.id)
            .map(|t| t.health);
    assert!(left.is_some_and(|h| h <= 0.0), "a mine left it at {left:?}");
}

/// The maintainer, on remote droids: "have its name and health show up on the
/// hud and the reticle change color if you go over it". The discs' name on
/// both games; no magnet.
#[test]
fn the_reticle_reads_an_enemy_remote_droid() {
    let Some(mut s) = scene("rep_inf_ep3_rifleman", 15.0) else {
        return;
    };
    let Some(k) =
        s.w.sim
            .weapon_classes
            .iter()
            .position(|w| w.remote.is_some())
    else {
        eprintln!("SKIP: no remote weapon on this mission");
        return;
    };
    let remote = s.w.sim.weapon_classes[k].remote.clone().unwrap();
    s.w.sim.turrets.clear();
    let mut d = core::remote::Droid::new(
        90,
        s.me,
        1,
        k,
        &remote,
        s.centre,
        glam::Vec3::ZERO,
        0.0,
        false,
    );
    d.controlled = false;
    d.health = remote.droid.max_health * 0.5;
    s.w.sim.droids.push(d);
    lay_reticle(&mut s.w, s.me, s.centre);
    s.w.sim.step(&[PadState::default(), PadState::default()]);
    assert_eq!(s.w.sim.reticle_pick[0], Some((AimTarget::Droid(90), -1)));
    assert_eq!(s.w.sim.magnet[0], None);
    let feed = game::event_feed::EventFeed::default();
    let t = feed.target(&s.w.sim, 0).expect("the HUD's target");
    assert_eq!(t.name, "Recon Droid");
    assert_eq!(t.relation, -1);
    assert!(
        t.health_fraction > 0.4 && t.health_fraction <= 0.5,
        "{}",
        t.health_fraction
    );
    let root = game::assets::baked_root();
    for (game_id, want) in [("bf2", "Recon Droid"), ("bf1", "Recon Droid")] {
        let texts = game::event_feed::Texts::load(&root, game_id);
        for side in ["all", "imp", "rep", "cis"] {
            let name = texts.weapon_name(&format!("{side}_weap_inf_remotedroid"));
            assert_eq!(name.as_deref(), Some(want), "{game_id} {side}");
        }
    }
}
