//! Card TURR's scenes on flat ground: a building turret at the origin facing
//! +Z (a gun on a full-circle mount, BF2's dish-turret limits), one bot of
//! team 0 near it, three idle team-mates far off (so the team is not in the
//! endgame) and an idle enemy that does not die. One scene a rule: the offer
//! (attack and defence clauses, with their controls), the post's cap, each
//! of the five exits, the turret closed after leaving and after a walk that
//! did not reach it. STCHG adds charge timing, ownership and lifecycle controls.

use super::*;
use crate::cp::CpDef;
use crate::region::Region;
use crate::sim::Sim;
use crate::unit::{Controller, UnitSpec};
use crate::vehicle::gun::{Aimer, GunWeapon, Slew, Turn};
use crate::vehicle::{SeatKind, SeatSetup, VehicleSpec};
use crate::weapon::{DamageDesc, HealthType, WeaponClass};

const DEG: f32 = std::f32::consts::PI / 180.0;
const HZ: u64 = crate::TICK_HZ as u64;

/// The turret's gun: class `weapon`, pivot 3 m up; a full-circle yaw unless
/// `yaw` limits it (degrees, the PS2's terms).
fn gun_setup(weapon: usize, yaw: Option<[f32; 2]>) -> GunSetup {
    GunSetup {
        weapons: vec![GunWeapon {
            class: weapon,
            infinite: true,
            reserve_clips: 0,
            fire_points: Vec::new(),
            fire_limits: Vec::new(),
        }],
        fire_point: Vec3::new(0.0, 3.0, 2.0),
        pivot: Vec3::new(0.0, 3.0, 0.0),
        eye: Vec3::new(0.0, 0.5, -1.0),
        aimer: Aimer {
            yaw: [0.0, 0.0],
            pitch: [-15.0 * DEG, 40.0 * DEG],
            ..Aimer::default()
        },
        pilot: false,
        turn: Some(Turn::from_class(
            Some(yaw.unwrap_or([-180.0, 180.0])),
            Some([-10.0, 35.0]),
            (1.0, 1.0),
            (10.0, 10.0),
            Slew::Linear,
        )),
        framed: true,
        aimer_bind: glam::Quat::IDENTITY,
        weapon_aimer: None,
        gunner: None,
    }
}

struct Scene {
    s: Sim,
    v: VehicleHandle,
    bot: UnitHandle,
    enemy: UnitHandle,
}

fn turret_at(s: &mut Sim, pos: Vec3) -> VehicleHandle {
    s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Turret,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos,
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Building,
        seats: vec![SeatKind::Gunner(0)],
    })
}

fn bot_at(s: &mut Sim, pos: Vec3) -> UnitHandle {
    s.spawn(&UnitSpec {
        controller: Controller::Ai,
        ..UnitSpec::trooper(0, pos)
    })
    .unwrap()
}

/// The scene: the bot at `bot`, the enemy at `enemy` (both on the ground),
/// the turret's gun firing class `weapon` (0 the rifle, 1 a charge rifle) over `yaw`.
fn scene(bot: Vec3, enemy: Vec3, weapon: usize, yaw: Option<[f32; 2]>) -> Scene {
    scene_with(bot, enemy, gun_setup(weapon, yaw))
}

/// [`scene`] with the turret's gun `g`.
fn scene_with(bot: Vec3, enemy: Vec3, g: GunSetup) -> Scene {
    scene_classes(bot, enemy, g, &[])
}

/// [`scene_with`], the weapon classes `more` added from index 2 on.
fn scene_classes(bot: Vec3, enemy: Vec3, g: GunSetup, more: &[WeaponClass]) -> Scene {
    let mut s = Sim::empty(5);
    s.heightfield = Some(crate::heightfield::Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    });
    // Class 0 is `Sim::empty`'s rifle.
    let charge = s.add_weapon_class(WeaponClass {
        max_pressed_time: 1.0,
        ..WeaponClass::bf1_rifle()
    });
    assert_eq!(charge, 1);
    for wc in more {
        s.add_weapon_class(wc.clone());
    }
    s.seat_setup = vec![Some(SeatSetup {
        radius: 3.0,
        body_active: vec![],
        boardable: vec![true],
        exit_drop: 15.0,
        ai_size: 1,
        guns: vec![Some(g)],
    })];
    let v = turret_at(&mut s, Vec3::ZERO);
    let b = bot_at(&mut s, bot);
    for k in 0..3 {
        s.spawn(&UnitSpec::trooper(
            0,
            Vec3::new(600.0 + 5.0 * k as f32, 0.0, 600.0),
        ))
        .unwrap();
    }
    let e = s
        .spawn(&UnitSpec {
            max_health: 1.0e9,
            ..UnitSpec::trooper(1, enemy)
        })
        .unwrap();
    Scene {
        s,
        v,
        bot: b,
        enemy: e,
    }
}

/// A team-0 post 20 m from the turret and an enemy post 220 m off - its
/// neighbour (no graph: any other post is one).
fn defended(sc: &mut Scene) {
    let post = |name: &str, pos: Vec3| CpDef {
        name: name.into(),
        pos,
        capture: Region::sphere(pos, 8.0),
        control: None,
        value_bleed: 100,
        value_atk: [100, 100],
        value_def: [100, 100],
        spawn_nodes: vec![],
        capture_time: 10.0,
        neutralize_time: 12.0,
    };
    sc.s.add_cp(post("ours", Vec3::new(0.0, 0.0, 20.0)), Some(0));
    sc.s.add_cp(post("theirs", Vec3::new(0.0, 0.0, 220.0)), Some(1));
}

/// A team-mate's memory holds `e` where it stands: the first of the idle
/// team-mates, who never think, so the record is never aged out.
fn tell(s: &mut Sim, e: UnitHandle) {
    let at = s.unit(e).unwrap().capsule().center();
    let tick = s.tick;
    let mate = s
        .units
        .iter()
        .position(|u| u.alive && u.team == 0 && u.controller == Controller::Idle)
        .unwrap();
    s.units[mate].ai.contacts[0] = Some(crate::perception::Contact {
        handle: e.into(),
        first_seen: tick,
        last_seen: tick,
        last_pos: at,
        seen_now: true,
        heard: false,
        told: false,
    });
}

fn seated(s: &Sim, b: UnitHandle, v: VehicleHandle) -> bool {
    s.unit(b)
        .and_then(|u| u.mount)
        .is_some_and(|m| m.vehicle == v)
}

/// Step until `f` holds, at most `ticks`; the tick it held at.
fn until(s: &mut Sim, ticks: u64, mut f: impl FnMut(&Sim) -> bool) -> Option<u64> {
    for _ in 0..ticks {
        s.step(&[]);
        if f(s) {
            return Some(s.tick);
        }
    }
    None
}

/// The attack clause: an enemy the bot sees 30 m behind the turret, inside
/// its arc and range - the bot is offered it, boards, turns it round and
/// fires. Controls: the same enemy out of sight and range (130 m), and a
/// turret whose arc looks the other way, are never offered.
#[test]
fn a_bot_mans_a_turret_that_can_hit_an_enemy_the_team_knows_of() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    let (b, v) = (sc.bot, sc.v);
    let at = until(&mut sc.s, 10 * HZ, |s| seated(s, b, v)).expect("never manned");
    assert!(at < 8 * HZ, "manned at tick {at}");
    assert_eq!(sc.s.nav_stats.man_offers, 1);
    assert_eq!(sc.s.nav_stats.man_boards, 1);
    let shot = until(&mut sc.s, 10 * HZ, |s| s.nav_stats.seat_shots > 0);
    assert!(shot.is_some(), "the gunner never fired");
    assert!(seated(&sc.s, b, v), "left with a target to hit");
    assert_eq!(sc.s.unit(b).unwrap().ai.behaviour, Behaviour::Man);

    for (enemy, yaw) in [
        (Vec3::new(4.0, 0.0, -130.0), None),
        (Vec3::new(4.0, 0.0, -30.0), Some([-60.0, 60.0])),
    ] {
        let mut sc = scene(Vec3::new(3.0, 0.0, 10.0), enemy, 0, yaw);
        let (b, v) = (sc.bot, sc.v);
        assert_eq!(until(&mut sc.s, 10 * HZ, |s| seated(s, b, v)), None);
        assert_eq!(sc.s.nav_stats.man_offers, 0, "{enemy} {yaw:?}");
    }
    // Known to the team but out of reach: 70 m off, past the 49 m a gunner
    // sees a standing soldier at (the rifle reaches 120 m); or behind a wall.
    for wall in [false, true] {
        let enemy = if wall {
            Vec3::new(4.0, 0.0, 30.0)
        } else {
            Vec3::new(4.0, 0.0, 70.0)
        };
        let mut sc = scene(Vec3::new(3.0, 0.0, -150.0), enemy, 0, None);
        if wall {
            use crate::collision::{Body, CollisionWorld, Shape};
            sc.s.collision = CollisionWorld::new(
                vec![Body::new(
                    glam::Mat4::from_translation(Vec3::new(0.0, 4.0, 15.0)),
                    Shape::Box {
                        half: Vec3::new(10.0, 4.0, 0.25),
                    },
                )],
                Vec::new(),
            );
        }
        tell(&mut sc.s, sc.enemy);
        let g = seat_gun(&sc.s, sc.v, 0).unwrap().clone();
        let k = known(&sc.s, 0);
        assert_eq!(k.len(), 1);
        assert_eq!(hittable(&sc.s, sc.v, &g, 0, &k), None, "wall {wall}");
        // The same enemy 30 m off in the open: hittable.
        if !wall {
            sc.s.units[sc.enemy.index as usize].soldier.pos = Vec3::new(4.0, 0.0, 30.0);
            tell(&mut sc.s, sc.enemy);
            assert!(hittable(&sc.s, sc.v, &g, 0, &known(&sc.s, 0)).is_some());
        }
    }
}

/// The defence clause: the turret, whose arc is the front 120 degrees,
/// stands 20 m from a front-line post of the team, and a team-mate knows of
/// an enemy behind it, 46 m from the post - nothing it can hit, but it is
/// offered and manned. Not with that enemy 70 m from the post, nor beside a
/// post of the enemy's, nor when the other post is the team's own too (not
/// a front line).
#[test]
fn a_turret_by_a_front_line_post_with_an_enemy_near_is_manned() {
    let front = Some([-60.0, 60.0]);
    let at = Vec3::new(0.0, 0.0, 20.0);
    // (enemy, post 0's owner, post 1's owner, post 0's place, manned)
    for (enemy, owner, other, post, manned) in [
        (Vec3::new(5.0, 0.0, -25.0), 0, 1, at, true),
        (Vec3::new(5.0, 0.0, -50.0), 0, 1, at, false),
        (Vec3::new(5.0, 0.0, -25.0), 1, 1, at, false),
        (Vec3::new(5.0, 0.0, -25.0), 0, 0, at, false),
        // The post 45 m off the turret's side, the enemy 14 m from it,
        // outside the arc.
        (
            Vec3::new(55.0, 0.0, -10.0),
            0,
            1,
            Vec3::new(45.0, 0.0, 0.0),
            false,
        ),
    ] {
        let mut sc = scene(Vec3::new(3.0, 0.0, 10.0), enemy, 0, front);
        defended(&mut sc);
        sc.s.cps[0].owner = Some(owner);
        sc.s.cps[1].owner = Some(other);
        sc.s.cps[0].def.pos = post;
        tell(&mut sc.s, sc.enemy);
        let (b, v) = (sc.bot, sc.v);
        let at = until(&mut sc.s, 10 * HZ, |s| seated(s, b, v));
        assert_eq!(at.is_some(), manned, "{enemy} {owner} {other} {post}");
        assert_eq!(sc.s.nav_stats.man_defence_offers, u64::from(manned));
    }
}

/// The route graph's neighbours (`l_graph`: hubs 0 - 1 - 2 in a row): with a
/// post at each hub, the middle one is the only neighbour of the first, so
/// the first is on the front line when the middle is the enemy's and not
/// when only the far one is.
#[test]
fn a_front_line_post_has_an_enemy_neighbour_on_the_route_graph() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(400.0, 0.0, 0.0),
        0,
        None,
    );
    sc.s.nav = Some(crate::ai::tests::l_graph());
    defended(&mut sc);
    defended(&mut sc);
    sc.s.cps.truncate(3);
    for (k, cp) in sc.s.cps.iter_mut().enumerate() {
        cp.nav_hub = Some(k as u16);
    }
    for (owners, front) in [([0, 1, 0], true), ([0, 0, 1], false), ([0, 0, 0], false)] {
        for (cp, o) in sc.s.cps.iter_mut().zip(owners) {
            cp.owner = Some(o);
        }
        assert_eq!(front_line(&sc.s, 0, 0), front, "{owners:?}");
    }
    // The middle post is the far one's neighbour too.
    for (cp, o) in sc.s.cps.iter_mut().zip([1, 0, 0]) {
        cp.owner = Some(o);
    }
    assert!(front_line(&sc.s, 0, 1));
    assert!(!front_line(&sc.s, 0, 2));
}

/// A post keeps at most two of its turrets manned or claimed: three turrets
/// beside one post, three free bots, an enemy all three can hit - two
/// claims.
#[test]
fn a_post_takes_two_gunners() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    defended(&mut sc);
    for x in [-8.0, 8.0] {
        turret_at(&mut sc.s, Vec3::new(x, 0.0, 0.0));
        bot_at(&mut sc.s, Vec3::new(x + 3.0, 0.0, 10.0));
    }
    until(&mut sc.s, 3 * HZ, |_| false);
    let claimed =
        sc.s.alive_units()
            .filter(|(_, u)| u.mount.is_some() || u.ai.ride.is_some_and(|r| r.board.is_some()))
            .count();
    assert_eq!(
        claimed, MAN_POST_CAP,
        "{} offers",
        sc.s.nav_stats.man_offers
    );
}

/// Seat the scene's bot by hand at tick 1.
fn sit(sc: &mut Scene) {
    sc.s.step(&[]);
    assert!(sc.s.enter_vehicle(sc.bot, sc.v, 0));
}

/// Exit 1: a gunner at a defended post (an enemy its team knows of behind
/// the turret's 120-degree arc, 46 m from the post) with nothing it can hit
/// gets out 10 s after it sat down, by rule 1 - then the turret is closed
/// to it for 20 s and offered to it again after.
#[test]
fn exit_1_ten_seconds_with_nothing_to_hit_and_closed_for_twenty() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(5.0, 0.0, -25.0),
        0,
        Some([-60.0, 60.0]),
    );
    defended(&mut sc);
    tell(&mut sc.s, sc.enemy);
    sit(&mut sc);
    let (b, v) = (sc.bot, sc.v);
    let sat = sc.s.tick;
    let out = until(&mut sc.s, 15 * HZ, |s| !seated(s, b, v)).expect("never left");
    assert!(out - sat >= MAN_NO_TARGET, "left after {} ticks", out - sat);
    assert!(out - sat <= MAN_NO_TARGET + THINK_PERIOD + 1);
    assert_eq!(
        sc.s.nav_stats.man_codes[1], 1,
        "{:?}",
        sc.s.nav_stats.man_codes
    );
    // Closed to it for 20 s: no claim, though the post is still defended
    // and the bot is held 10 m from the turret (it would walk off to post 1).
    let claim = |s: &Sim| {
        s.unit(b)
            .unwrap()
            .ai
            .ride
            .is_some_and(|r| r.board == Some(v))
    };
    let hold = |s: &mut Sim| {
        let u = &mut s.units[b.index as usize].soldier;
        (u.pos, u.vel) = (Vec3::new(3.0, 0.0, 10.0), Vec3::ZERO);
    };
    for _ in 0..(MAN_LEFT_CLOSED - 2 * MAN_OFFER_PERIOD) {
        hold(&mut sc.s);
        sc.s.step(&[]);
        assert!(
            !claim(&sc.s) && !seated(&sc.s, b, v),
            "re-claimed at {}",
            sc.s.tick
        );
    }
    let again = (0..4 * MAN_OFFER_PERIOD).any(|_| {
        hold(&mut sc.s);
        sc.s.step(&[]);
        claim(&sc.s) || seated(&sc.s, b, v)
    });
    assert!(again, "never offered again");
}

/// Exit 2: a depleted gun with a reachable enemy sits 45 s and exits by
/// rule 2; the otherwise equal loaded gun fires and stays.
#[test]
fn exit_2_forty_five_seconds_with_no_shot_in_twenty() {
    for leaves in [true, false] {
        let mut gun = gun_setup(2, None);
        gun.weapons[0].infinite = !leaves;
        gun.weapons[0].reserve_clips = 0;
        let mut sc = scene_classes(
            Vec3::new(3.0, 0.0, 10.0),
            Vec3::new(4.0, 0.0, 30.0),
            gun,
            &[WeaponClass {
                rounds_per_clip: 3,
                ..WeaponClass::bf1_rifle()
            }],
        );
        sit(&mut sc);
        let sat = sc.s.tick;
        // Let the real weapon consume its finite clip; no injected silence.
        if leaves {
            let empty = until(&mut sc.s, 20 * HZ, |s| {
                s.vehicle(sc.v).unwrap().guns[0].as_ref().is_some_and(|g| {
                    let w = &g.weapons[0];
                    w.clip == 0 && w.spare == 0 && w.shots_fired > 0
                })
            });
            assert!(empty.is_some(), "finite gun must actually deplete");
        }
        let (b, v) = (sc.bot, sc.v);
        let out = until(&mut sc.s, 50 * HZ, |s| !seated(s, b, v));
        if leaves {
            let out = out.expect("never left");
            assert!(out - sat >= MAN_LONG, "left after {} ticks", out - sat);
            assert!(out - sc.s.unit(b).unwrap().last_shot_tick >= MAN_NO_SHOT);
            assert_eq!(
                sc.s.nav_stats.man_codes[2], 1,
                "{:?}",
                sc.s.nav_stats.man_codes
            );
        } else {
            assert_eq!(out, None, "{:?}", sc.s.nav_stats.man_codes);
            assert!(sc.s.nav_stats.seat_shots > 0);
        }
    }
}

/// Exit 3: hit from behind a turret whose arc is the front 120 degrees ->
/// out at the next think; hit from inside the arc -> stays. A turret low on
/// health is not left (no bail-out since 2026-10-28): at 5 % it stays.
#[test]
fn exit_3_hit_from_outside_every_arc_and_no_low_health_bail_out() {
    let hit = DamageDesc {
        damage: 10.0,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    for (from_behind, leaves) in [(true, true), (false, false)] {
        let mut sc = scene(
            Vec3::new(3.0, 0.0, 10.0),
            Vec3::new(4.0, 0.0, 30.0),
            0,
            Some([-60.0, 60.0]),
        );
        let behind =
            sc.s.spawn(&UnitSpec {
                max_health: 1.0e9,
                ..UnitSpec::trooper(1, Vec3::new(-3.0, 0.0, -25.0))
            })
            .unwrap();
        sit(&mut sc);
        until(&mut sc.s, 2 * HZ, |_| false);
        let who = if from_behind { behind } else { sc.enemy };
        sc.s.damage_vehicle(sc.v, &hit, 1.0, Some((who, 1)));
        let (b, v) = (sc.bot, sc.v);
        let out = until(&mut sc.s, 2 * HZ, |s| !seated(s, b, v));
        assert_eq!(out.is_some(), leaves, "from behind {from_behind}");
        assert_eq!(sc.s.nav_stats.man_codes[3], u64::from(leaves));
    }
    for (share, leaves) in [(0.05, false), (0.35, false)] {
        let mut sc = scene(
            Vec3::new(3.0, 0.0, 10.0),
            Vec3::new(4.0, 0.0, 30.0),
            0,
            None,
        );
        sit(&mut sc);
        sc.s.vehicles[sc.v.index as usize].health.cur = share * 1000.0;
        let (b, v) = (sc.bot, sc.v);
        let out = until(&mut sc.s, 2 * HZ, |s| !seated(s, b, v));
        assert_eq!(out.is_some(), leaves, "at {share}");
        assert_eq!(sc.s.nav_stats.man_codes[3], u64::from(leaves));
    }
}

/// Exit 4: the endgame - a team at 10 tickets, or the gunner's own team down
/// to three - puts a gunner with a target out at its next think, and no
/// turret is offered then.
#[test]
fn exit_4_the_endgame_empties_the_turrets() {
    for tickets in [true, false] {
        let mut sc = scene(
            Vec3::new(3.0, 0.0, 10.0),
            Vec3::new(4.0, 0.0, 30.0),
            0,
            None,
        );
        sit(&mut sc);
        until(&mut sc.s, 2 * HZ, |_| false);
        let (b, v) = (sc.bot, sc.v);
        assert!(seated(&sc.s, b, v));
        if tickets {
            sc.s.battle.teams[1].reinforcements = HOLD_ENDGAME as f32;
        } else {
            let mate =
                sc.s.alive_units()
                    .find(|(h, u)| u.team == 0 && *h != b)
                    .map(|(h, _)| h)
                    .unwrap();
            sc.s.units[mate.index as usize].alive = false;
        }
        let out = until(&mut sc.s, THINK_PERIOD + 2, |s| !seated(s, b, v));
        assert!(out.is_some(), "tickets {tickets}: still seated");
        assert_eq!(sc.s.nav_stats.man_codes[4], 1);
    }
    // ...and the attack scene's offer is not made.
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    sc.s.battle.teams[0].reinforcements = HOLD_ENDGAME as f32;
    let (b, v) = (sc.bot, sc.v);
    assert_eq!(until(&mut sc.s, 10 * HZ, |s| seated(s, b, v)), None);
    assert_eq!(sc.s.nav_stats.man_offers, 0);
}

/// Exit 5, the purpose lost for 10 s: the purpose clock a think keeps is
/// never older than exit 1's (a target it can hit IS a purpose), so in the
/// sim exit 1 holds first and rule 5 cannot be isolated in a scene. The
/// rule itself, on clocks set by hand: a target it could hit 1 s ago, the
/// purpose 10 s ago -> 5; 9 s ago -> none.
#[test]
fn exit_5_the_purpose_lost_for_ten_seconds() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(4.0, 0.0, 30.0),
        0,
        None,
    );
    sit(&mut sc);
    sc.s.tick = 1000;
    let g = seat_gun(&sc.s, sc.v, 0).unwrap().clone();
    for (ago, rule) in [(MAN_PURPOSE_LOST, Some(5)), (MAN_PURPOSE_LOST - HZ, None)] {
        let man = ManState {
            seated: 900,
            hittable: 970,
            purpose: 1000 - ago,
            look: None,
            aim: None,
        };
        assert_eq!(exit_rule(&sc.s, sc.v, &g, 0, &man, 990), rule, "{ago}");
    }
}

/// The walk has an outcome: a turret inside a closed ring of 1.5 m walls
/// 24 m across (wider than the 10 m board reach) is offered, never reached,
/// and then closed to the bot for 60 s.
#[test]
fn a_turret_the_walk_cannot_reach_is_closed_for_sixty_seconds() {
    use crate::collision::{Body, CollisionWorld, Shape};
    let mut sc = scene(
        Vec3::new(3.0, 0.0, -20.0),
        Vec3::new(10.0, 0.0, -45.0),
        0,
        None,
    );
    let wall =
        |c: Vec3, half: Vec3| Body::new(glam::Mat4::from_translation(c), Shape::Box { half });
    sc.s.collision = CollisionWorld::new(
        vec![
            wall(Vec3::new(0.0, 0.75, 12.0), Vec3::new(12.5, 0.75, 0.25)),
            wall(Vec3::new(0.0, 0.75, -12.0), Vec3::new(12.5, 0.75, 0.25)),
            wall(Vec3::new(12.0, 0.75, 0.0), Vec3::new(0.25, 0.75, 12.5)),
            wall(Vec3::new(-12.0, 0.75, 0.0), Vec3::new(0.25, 0.75, 12.5)),
        ],
        Vec::new(),
    );
    let (b, v) = (sc.bot, sc.v);
    let claim = move |s: &Sim| {
        s.unit(b)
            .unwrap()
            .ai
            .ride
            .is_some_and(|r| r.board == Some(v))
    };
    assert!(until(&mut sc.s, 5 * HZ, claim).is_some(), "never offered");
    let dropped = until(&mut sc.s, 40 * HZ, |s| !claim(s)).expect("never gave up");
    assert!(!seated(&sc.s, b, v));
    let r = sc.s.unit(b).unwrap().ai.ride.unwrap();
    assert_eq!(r.closed, Some(v));
    // (Closed at the think, the tick before the step's count.)
    assert_eq!(r.closed_until, dropped - 1 + MAN_UNREACHED_CLOSED);
    assert_eq!(sc.s.nav_stats.man_codes[6], 1);
    for _ in 0..(MAN_UNREACHED_CLOSED - 2 * MAN_OFFER_PERIOD) {
        sc.s.step(&[]);
        assert!(!claim(&sc.s), "claimed again at {}", sc.s.tick);
    }
}

/// The offer goes to the NEAREST free bot within 50 m: of bots 10 m and 30 m
/// off the near one takes it; one 55 m off alone is offered nothing.
#[test]
fn the_nearest_free_bot_within_fifty_metres_is_offered_the_turret() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 30.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    let near = bot_at(&mut sc.s, Vec3::new(-3.0, 0.0, 10.0));
    tell(&mut sc.s, sc.enemy);
    until(&mut sc.s, 2, |_| false);
    let board = |s: &Sim, h: UnitHandle| s.unit(h).unwrap().ai.ride.and_then(|r| r.board);
    assert_eq!(board(&sc.s, near), Some(sc.v));
    assert_eq!(board(&sc.s, sc.bot), None);

    let mut sc = scene(
        Vec3::new(3.0, 0.0, 55.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    tell(&mut sc.s, sc.enemy);
    until(&mut sc.s, 2 * MAN_OFFER_PERIOD, |_| false);
    assert_eq!(sc.s.nav_stats.man_offers, 0);
}

/// A gunner that has not seen the enemy itself turns the turret to the one
/// its team knows of (behind it) and fires once it sees him.
#[test]
fn a_gunner_turns_to_the_enemy_its_team_knows_of() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    tell(&mut sc.s, sc.enemy);
    // (Facing +z, away from the enemy, so it never sees him on foot.)
    sc.s.units[sc.bot.index as usize].soldier.yaw = std::f32::consts::PI;
    sit(&mut sc);
    assert!(sc
        .s
        .unit(sc.bot)
        .unwrap()
        .ai
        .contacts
        .iter()
        .all(Option::is_none));
    let shot = until(&mut sc.s, 10 * HZ, |s| s.nav_stats.seat_shots > 0);
    assert!(shot.is_some(), "never turned to fire");
}

/// Card TURR: a bot walking to its turret does not stop to fight an enemy
/// inside 25 m on the way (a boarder of a vehicle does) - it walks on and
/// mans the turret, 0.4 m past the board reach here. (The posts give the
/// bot somewhere to look while it walks: with none, no route and nobody
/// seen, `think` idles it.)
#[test]
fn a_bot_walking_to_its_turret_does_not_stand_to_fight_on_the_way() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 10.0),
        Vec3::new(5.0, 0.0, 25.0),
        0,
        None,
    );
    defended(&mut sc);
    sc.s.units[sc.bot.index as usize].soldier.yaw = std::f32::consts::PI;
    let (b, v) = (sc.bot, sc.v);
    let saw = until(&mut sc.s, HZ, |s| s.unit(b).unwrap().ai.target.is_some());
    assert!(saw.is_some(), "the bot does not see the enemy 15 m off");
    assert!(until(&mut sc.s, 5 * HZ, |s| seated(s, b, v)).is_some());
}

/// BF2 Mygeeto's recoilless turret (`tur_bldg_recoilless_lg`, the rifle in
/// place of its rocket): the pivot `aimer_y` at the base, `hp_fire1` 3.39 m
/// over it and 1.28 m to the side, the camera's eye 8.3 m behind and 4.2 m
/// up (`EyePointOffset 1.0 4.2 -8.3`), `AimerYawLimits 0 0`,
/// `AimerPitchLimits -30 20`, `PitchLimits -30 20`, turned at 1.5 rad/s.
fn recoilless() -> GunSetup {
    let mut g = gun_setup(0, None);
    g.fire_point = Vec3::new(1.281, 3.388, 3.372);
    g.pivot = Vec3::ZERO;
    g.eye = Vec3::new(1.0, 4.2, -8.3);
    g.aimer.pitch = [-30.0 * DEG, 20.0 * DEG];
    g.turn = Some(Turn::from_class(
        Some([-180.0, 180.0]),
        Some([-30.0, 20.0]),
        (1.5, 1.0),
        (10.0, 10.0),
        Slew::Linear,
    ));
    g
}

/// The turret's own building as the bake builds it: a map body of the
/// turret's instance (7) round `centre`.
fn building(sc: &mut Scene, centre: Vec3, half: Vec3) {
    use crate::collision::{Body, CollisionWorld, Shape};
    let mut b = Body::new(glam::Mat4::from_translation(centre), Shape::Box { half });
    b.instance = Some(7);
    sc.s.collision = CollisionWorld::new(vec![b], Vec::new());
    sc.s.vehicle_mut(sc.v).unwrap().instance = Some(7);
}

/// Hits on `e` by anyone, this tick.
fn hits_on(s: &Sim, e: UnitHandle) -> usize {
    s.events
        .iter()
        .filter(|ev| matches!(ev, crate::sim::Event::Hit { victim, .. } if *victim == e))
        .count()
}

/// Card TAIM, merged into TURR: a bot in Mygeeto's recoilless turret, its
/// building round the camera's eye, puts a bolt on a soldier 40 m in front
/// within 5 s. On the branch before (33016bf) the barrel never came within
/// 7 deg of a seen target on that map (`lead_tmp/turr/myg_dbg.txt`): the
/// camera ray hit the building at the eye, the aimer turned the pivot's
/// line onto the ray's point (3.4 m under the barrel), the muzzle was turned
/// onto the pivot's axis and the look was steered from the eye.
#[test]
fn a_bot_in_a_recoilless_turret_hits_a_soldier_forty_metres_out() {
    let mut sc = scene_with(
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(2.0, 0.0, 40.0),
        recoilless(),
    );
    building(&mut sc, Vec3::new(0.0, 3.0, -5.0), Vec3::new(3.0, 3.0, 6.0));
    sit(&mut sc);
    let e = sc.enemy;
    // 10 s: the PS2's bolt prims (issue #41) are slimmer than the capsule
    // the 5 s was set on; the first hit lands near 9 s.
    let hit = until(&mut sc.s, 10 * HZ, |s| hits_on(s, e) > 0);
    assert!(hit.is_some(), "no bolt hit the soldier in 10 s");
}

/// A gunner keeps to its turret's arc (OURS, card TURR): an enemy it holds
/// in memory 12 m behind a turret whose arc is the front 120 degrees, and
/// one it sees 35 m in front - it takes the one in front and hits him. On
/// foot's choice (the nearer) left Kashyyyk's gunners holding the look at
/// the limit of the arc (1 shot in 29 spells over 8 seeds).
#[test]
fn a_gunner_takes_the_target_inside_its_arc() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(2.0, 0.0, 35.0),
        0,
        Some([-60.0, 60.0]),
    );
    let behind =
        sc.s.spawn(&UnitSpec {
            max_health: 1.0e9,
            ..UnitSpec::trooper(1, Vec3::new(1.0, 0.0, -12.0))
        })
        .unwrap();
    sit(&mut sc);
    let (b, e) = (sc.bot, sc.enemy);
    let mut hit = false;
    for k in 0..5 * HZ {
        // The one behind stays fresh in the gunner's memory, unseen.
        let at = sc.s.unit(behind).unwrap().capsule().center();
        let tick = sc.s.tick;
        let st = &mut sc.s.units[b.index as usize].ai;
        st.contacts[3] = Some(crate::perception::Contact {
            handle: behind.into(),
            first_seen: 0,
            last_seen: tick,
            last_pos: at,
            seen_now: false,
            heard: false,
            told: false,
        });
        sc.s.step(&[]);
        // From its first think in the seat (it may have seen him on foot).
        if k > THINK_PERIOD {
            assert_ne!(
                sc.s.unit(b).unwrap().ai.target,
                Some(behind.into()),
                "tick {k}"
            );
        }
        hit |= hits_on(&sc.s, e) > 0;
    }
    assert!(hit, "the soldier in front was never hit");
}

/// A gunner sees past its own building (card TURR): its muzzle inside the
/// turret's map body, an enemy 30 m in front is a contact at the first
/// think; the same body as a stranger's (instance 8) hides him.
#[test]
fn a_gunner_sees_out_of_its_own_building() {
    for own in [true, false] {
        let mut sc = scene(
            Vec3::new(3.0, 0.0, -3.0),
            Vec3::new(2.0, 0.0, 30.0),
            0,
            None,
        );
        building(&mut sc, Vec3::new(0.0, 3.0, 1.0), Vec3::new(2.0, 2.0, 2.0));
        if !own {
            sc.s.vehicle_mut(sc.v).unwrap().instance = Some(8);
        }
        sit(&mut sc);
        let (b, e) = (sc.bot, sc.enemy);
        let seen = until(&mut sc.s, 2 * HZ, |s| {
            s.unit(b)
                .unwrap()
                .ai
                .contacts
                .iter()
                .flatten()
                .any(|c| c.handle == e && c.seen_now)
        });
        assert_eq!(seen.is_some(), own, "own building {own}");
    }
}

/// Kashyyyk's case (card TURR): the recoilless turret on a 20 m tower whose
/// deck runs 2 m out in front of it, a soldier on the ground 30 m out, ahead
/// or 45 deg to the side - from the camera's eye, 8.3 m behind, the line to
/// him crosses the deck; from the muzzle it does not. The gunner looks from
/// its muzzle and hits him within 5 s.
#[test]
fn a_gunner_on_a_tower_hits_a_soldier_below() {
    for at in [Vec3::new(2.0, 0.0, 30.0), Vec3::new(18.0, 0.0, 18.0)] {
        tower_shot(at);
    }
}

fn tower_shot(at: Vec3) {
    use crate::collision::{Body, CollisionWorld, Shape};
    let mut sc = scene_with(Vec3::new(3.0, 0.0, -3.0), at, recoilless());
    sc.s.vehicle_mut(sc.v).unwrap().pos = Vec3::new(0.0, 20.0, 0.0);
    sc.s.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 19.5, -4.0)),
            Shape::Box {
                half: Vec3::new(6.0, 0.5, 6.0),
            },
        )],
        Vec::new(),
    );
    let eye = Vec3::new(1.0, 24.2, -8.3);
    let him = sc.s.unit(sc.enemy).unwrap().capsule().center();
    assert!(
        !crate::perception::los(&sc.s, eye, him),
        "the deck hides {at} from the eye"
    );
    sit(&mut sc);
    let e = sc.enemy;
    // 10 s: the PS2's bolt prims (issue #41) are slimmer than the capsule
    // the 5 s was set on; the first hit lands near 9 s.
    let hit = until(&mut sc.s, 10 * HZ, |s| hits_on(s, e) > 0);
    assert!(hit.is_some(), "no bolt hit the soldier at {at} in 10 s");
}

/// Card TBRD: a bot boards a building turret from the PS2's board-agent
/// reach (its body sphere within the turret's radius, 3 m, plus 1.5 m in BF2
/// and 0.5 m in BF1, 3-D), not from the player's radius plus 5 m and the 2 m
/// slack: its last tick on foot is at most 5.5 m (BF2) / 4.5 m (BF1) from
/// the turret's origin, where it used to be about 10 m.
#[test]
fn a_bot_boards_a_building_turret_only_once_it_has_walked_up_to_it() {
    for (rules, pad) in [
        (crate::soldier::GroundRules::Bf2, crate::ai::BOARD_PAD_BF2),
        (crate::soldier::GroundRules::Bf1, crate::ai::BOARD_PAD_BF1),
    ] {
        let mut sc = scene(
            Vec3::new(3.0, 0.0, 12.0),
            Vec3::new(4.0, 0.0, -30.0),
            0,
            None,
        );
        sc.s.ground_rules = rules;
        let (b, v) = (sc.bot, sc.v);
        let mut last = None;
        let at = until(&mut sc.s, 15 * HZ, |s| {
            let u = s.unit(b).unwrap();
            if u.mount.is_none() {
                let c = u.capsule();
                last = Some((u.soldier.pos, c.center(), c.radius.max(c.height * 0.5)));
            }
            seated(s, b, v)
        });
        assert!(at.is_some(), "{rules:?}: never manned");
        let (feet, centre, rs) = last.unwrap();
        let gap = centre.length() - 3.0 - rs;
        assert!(
            gap <= pad + 0.01,
            "{rules:?}: boarded {gap} m off the sphere"
        );
        let xz = feet.x.hypot(feet.z);
        assert!(xz <= 4.0 + pad, "{rules:?}: boarded from {xz} m");
    }
}

/// Card TBRD: a building turret 4 m over the walk (a balcony gun) is not
/// boarded from below - the bot that walks under it gives its claim up at
/// once (the walk did not reach it, `man_codes[6]`), long before the claim's
/// 30 s clock; it was seated from the ground before the card.
#[test]
fn a_turret_on_another_floor_is_given_up_not_boarded_from_below() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, 12.0),
        Vec3::new(4.0, 0.0, -30.0),
        0,
        None,
    );
    sc.s.vehicle_mut(sc.v).unwrap().pos.y = 4.0;
    let (b, v) = (sc.bot, sc.v);
    let gave_up = until(&mut sc.s, 20 * HZ, |s| s.nav_stats.man_codes[6] > 0);
    assert!(gave_up.is_some(), "the claim was never given up");
    assert!(
        sc.s.nav_stats.man_offers >= 1,
        "no offer: the scene tests nothing"
    );
    assert!(!seated(&sc.s, b, v), "boarded from the floor below");
    assert_eq!(sc.s.nav_stats.man_boards, 0);
}

/// Card BRSTU's scene: the bot seated at the turret, an immortal enemy 35 m
/// in front, the gun firing `wc` with `infinite` rounds (else 5 spare
/// clips) under `rules`.
fn seat_gun_scene(wc: WeaponClass, infinite: bool, rules: crate::soldier::GroundRules) -> Scene {
    let mut g = gun_setup(2, None);
    g.weapons[0].infinite = infinite;
    g.weapons[0].reserve_clips = if infinite { 0 } else { 5 };
    let mut sc = scene_classes(
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(2.0, 0.0, 35.0),
        g,
        &[wc],
    );
    sc.s.ground_rules = rules;
    sit(&mut sc);
    sc
}

/// One tick of the seat gun: (shots fired this tick, heat, overheated).
type GunTick = (u64, f32, bool);

/// Step `ticks` and read the seat gun each tick.
fn gun_ticks(sc: &mut Scene, ticks: u64) -> Vec<GunTick> {
    // (The gun's state is made on its first tick in a seat.)
    let seat = |s: &Sim| {
        let g = s
            .vehicle(sc.v)
            .unwrap()
            .guns
            .first()
            .and_then(Option::as_ref);
        g.map_or((0, 0.0, false), |g| {
            let w = &g.weapons[0];
            (w.shots_fired, w.heat, w.overheated)
        })
    };
    let mut last = seat(&sc.s).0;
    (0..ticks)
        .map(|_| {
            sc.s.step(&[]);
            let (n, heat, hot) = seat(&sc.s);
            let shots = n - last;
            last = n;
            (shots, heat, hot)
        })
        .collect()
}

/// Runs of shooting ticks: each `(first tick, last tick, shots)`, a run
/// broken by a gap of more than `gap` ticks.
fn shot_runs(t: &[GunTick], gap: usize) -> Vec<(usize, usize, u64)> {
    let mut runs: Vec<(usize, usize, u64)> = Vec::new();
    for (k, &(n, _, _)) in t.iter().enumerate() {
        if n == 0 {
            continue;
        }
        match runs.last_mut() {
            Some(r) if k - r.1 <= gap => {
                r.1 = k;
                r.2 += n;
            }
            _ => runs.push((k, k, n)),
        }
    }
    runs
}

/// A heat gun: 0.1 a shot, 5 shots a second, cooling 0.4 a second.
fn heat_gun() -> WeaponClass {
    WeaponClass {
        heat_per_shot: 0.1,
        heat_recover_rate: 0.4,
        heat_threshold: 0.5,
        shot_delay: [0.2, 0.2],
        rounds_per_clip: 0,
        ..WeaponClass::bf1_rifle()
    }
}

/// Card BRSTU (`analysis/re-soldier-ai.md` 23.2): a bot at a turret whose
/// gun has unlimited rounds and heat fires each burst until the heat is one
/// shot short of full - far past the class's 3-6 shots - and never
/// overheats; between bursts it waits the class's pause, counted from the
/// burst's end (the cooled gun affords the next burst at once).
#[test]
fn a_heat_gun_turret_bursts_until_one_shot_short_of_full_and_never_overheats() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let mut sc = seat_gun_scene(heat_gun(), true, rules);
        let t = gun_ticks(&mut sc, 20 * HZ);
        assert!(
            t.iter().all(|&(_, _, hot)| !hot),
            "{rules:?}: the gun overheated"
        );
        let runs = shot_runs(&t, 7);
        assert!(runs.len() >= 3, "{rules:?}: {runs:?}");
        let u = sc.s.unit(sc.bot).unwrap();
        let skill = sc.s.difficulty_of(u.team).skill_at(u.skill);
        let factor = match rules {
            crate::soldier::GroundRules::Bf2 => skill.pause_mult,
            crate::soldier::GroundRules::Bf1 => 1.0,
        };
        let pause = (factor * 2.0 * HZ as f32).round() as usize;
        // Every whole burst (not the last, cut by the test's end).
        for w in runs.windows(2) {
            let (a, b) = (w[0], w[1]);
            // The burst's last heat is within a shot of full.
            let peak = t[a.0..=a.1].iter().map(|x| x.1).fold(0.0, f32::max);
            assert!(peak > 0.85 && peak < 1.0, "{rules:?}: peak {peak} in {a:?}");
            assert!(a.2 > 6, "{rules:?}: a burst of {} shots: {runs:?}", a.2);
            // The pause: the class's, from the burst's end (the end test
            // runs on the tick after its last shot).
            let gap = b.0 - a.1;
            assert!(
                gap >= pause && gap <= pause + 3,
                "{rules:?}: gap {gap} ticks, pause {pause}: {runs:?}"
            );
        }
    }
}

/// Card BRSTU: an unlimited gun with no heat fires for as long as it has a
/// target - one burst over the 20 s.
#[test]
fn a_gun_with_no_heat_fires_for_as_long_as_it_has_a_target() {
    let wc = WeaponClass {
        rounds_per_clip: 0,
        ..heat_gun()
    };
    let wc = WeaponClass {
        heat_per_shot: 0.0,
        ..wc
    };
    let mut sc = seat_gun_scene(wc, true, crate::soldier::GroundRules::Bf2);
    let t = gun_ticks(&mut sc, 20 * HZ);
    let runs = shot_runs(&t, 7);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert!(runs[0].2 > 80, "{runs:?}");
}

/// Card BRSTU: a seat weapon with COUNTED rounds (its class's clip and the
/// slot's 5 spare clips) keeps CLSE4's rule - bursts of its class's own
/// Barrage keys (1 shot, 3 s: mutation GN1, the default class's 3-6 / 2 s,
/// fails) - and a single-shot seat gun is tapped, a shot a press.
#[test]
fn a_counted_seat_weapon_stops_after_n_with_its_classes_pause() {
    let wc = WeaponClass {
        barrage: [1, 1],
        barrage_delay: 3.0,
        ..WeaponClass::bf1_rifle()
    };
    let mut sc = seat_gun_scene(wc, false, crate::soldier::GroundRules::Bf1);
    let t = gun_ticks(&mut sc, 20 * HZ);
    let runs = shot_runs(&t, 7);
    assert!(runs.len() >= 4, "{runs:?}");
    for w in runs.windows(2) {
        assert_eq!(w[0].2, 1, "{runs:?}");
        let gap = w[1].0 - w[0].1;
        assert!(
            (3 * HZ as usize..=3 * HZ as usize + 3).contains(&gap),
            "gap {gap}: {runs:?}"
        );
    }
}

/// Card BRSTU: a `TriggerSingle` seat gun fires on a fresh press only, so
/// the bot taps it (on foot's STTAP rule): a held trigger would fire once.
#[test]
fn a_single_shot_seat_gun_is_tapped() {
    let wc = WeaponClass {
        trigger_single: true,
        ..heat_gun()
    };
    let wc = WeaponClass {
        heat_per_shot: 0.0,
        ..wc
    };
    let mut sc = seat_gun_scene(wc, true, crate::soldier::GroundRules::Bf2);
    let t = gun_ticks(&mut sc, 20 * HZ);
    let shots: u64 = t.iter().map(|x| x.0).sum();
    assert!(shots >= 10, "{shots} shots in 20 s");
}

/// Card BRSTU: a burst starts only when n shots' heat fits in what is left
/// (the PS2's state 2). A slow-cooling gun (0.05 a second) is still hot when
/// the 2 s pause ends, so the bot waits past it, and every burst it starts
/// has room for at least the class's 3 shots - never the one or two a
/// start at the pause's end would leave before the end test.
#[test]
fn a_heat_burst_waits_until_its_shots_fit() {
    let wc = WeaponClass {
        heat_recover_rate: 0.05,
        ..heat_gun()
    };
    let mut sc = seat_gun_scene(wc, true, crate::soldier::GroundRules::Bf1);
    let t = gun_ticks(&mut sc, 60 * HZ);
    assert!(t.iter().all(|&(_, _, hot)| !hot), "the gun overheated");
    let runs = shot_runs(&t, 7);
    assert!(runs.len() >= 4, "{runs:?}");
    for w in runs.windows(2) {
        let b = w[1];
        assert!(b.2 >= 3, "a burst of {} shots: {runs:?}", b.2);
        // The heat the tick before the burst: room for 3 shots' 0.3.
        let before = t[b.0 - 1].1;
        assert!(before < 0.7, "started at heat {before}: {runs:?}");
    }
    let waits = runs
        .windows(2)
        .filter(|w| w[1].0 - w[0].1 > 3 * HZ as usize)
        .count();
    assert!(waits > 0, "no burst waited past the 2 s pause: {runs:?}");
}

/// Card BRSTU: BF1 Hoth's turret cannon (0.25 heat a shot, two salvos a
/// trigger, cooling 0.4 a second): a drawn n of 4 costs exactly a full
/// meter, which never fits - the bot draws again while it waits, so it keeps
/// firing burst after burst. With n kept until the want drops (the PS2's
/// state 2) the gunner stood with a target and a cold gun for the rest of
/// the round (53 % of its target time in a sweep).
#[test]
fn a_burst_whose_n_costs_a_full_meter_is_drawn_again() {
    let wc = WeaponClass {
        heat_per_shot: 0.25,
        heat_recover_rate: 0.4,
        heat_threshold: 0.25,
        shot_delay: [0.4, 0.4],
        salvo_count: 2,
        salvo_delay: [0.15, 0.15],
        initial_salvo_delay: 0.0,
        rounds_per_clip: 0,
        ..WeaponClass::bf1_rifle()
    };
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let mut sc = seat_gun_scene(wc.clone(), true, rules);
        let t = gun_ticks(&mut sc, 60 * HZ);
        let runs = shot_runs(&t, 15);
        assert!(
            runs.len() >= 12,
            "{rules:?}: {} bursts in 60 s: {runs:?}",
            runs.len()
        );
        let longest = runs.windows(2).map(|w| w[1].0 - w[0].1).max().unwrap_or(0);
        assert!(
            longest < 4 * HZ as usize,
            "{rules:?}: a {longest}-tick wait: {runs:?}"
        );
    }
}

/// STCHG: actual seat dispatch from an idle weapon, both source rules and
/// both TriggerSingle values. Every cycle must use the foot skill draw,
/// keep its deadline, hold FIRE, and produce an owned shot on release.
#[test]
fn seat_charge_holds_then_releases_on_its_drawn_deadline() {
    use crate::ai::draw::{unit_unit, SALT_CHARGE};
    use crate::soldier::GroundRules::{Bf1, Bf2};
    for (rules, duration, level) in [(Bf1, 1.5, 1), (Bf2, 2.0, 20)] {
        for single in [true, false] {
            let wc = WeaponClass {
                max_pressed_time: duration,
                trigger_single: single,
                rounds_per_clip: 0,
                heat_per_shot: 0.0,
                ..WeaponClass::bf1_rifle()
            };
            let mut sc = seat_gun_scene(wc, true, rules);
            sc.s.units[sc.bot.index as usize].skill = level;
            assert_eq!(sc.s.unit(sc.bot).unwrap().ai.charge_until, 0);
            assert!(sc
                .s
                .vehicle(sc.v)
                .unwrap()
                .guns
                .first()
                .is_none_or(Option::is_none));
            let mut deadline = 0;
            let mut starts = 0;
            let mut releases = 0;
            let mut shots = 0;
            for _ in 0..10 * HZ {
                let tick = sc.s.tick;
                sc.s.step(&[]);
                let u = sc.s.unit(sc.bot).unwrap();
                assert!(u.mount.is_some());
                let until = u.ai.charge_until;
                let w = &sc.s.vehicle(sc.v).unwrap().guns[0]
                    .as_ref()
                    .unwrap()
                    .weapons[0];
                if starts == 0 && until == 0 {
                    assert_eq!(w.shots_fired, 0, "no carried foot FIRE may count");
                    assert!(w.fire_cmd <= 0, "fresh seat weapon must remain idle");
                }
                if deadline == 0 && until != 0 {
                    let skill = sc.s.difficulty_of(u.team).skill_at(level);
                    let draw = unit_unit(sc.bot.index, tick, sc.bot.gen, SALT_CHARGE);
                    let fraction = crate::ai::CHARGE_MIN
                        + (skill.charge_hi - crate::ai::CHARGE_MIN).max(0.0) * draw;
                    let ticks = (fraction * duration * crate::TICK_HZ as f32)
                        .round()
                        .max(1.0) as u64;
                    assert_eq!(until, tick + ticks, "skill/class duration draw");
                    deadline = until;
                    starts += 1;
                    eprintln!("STCHG {rules:?} single={single} start={tick} deadline={until} FIRE={} fire_cmd={}", w.fire_cmd > 0, w.fire_cmd);
                }
                if deadline > tick {
                    assert_eq!(until, deadline, "uninterrupted charge deadline changed");
                    assert!(w.fire_cmd > 0, "charge must hold FIRE");
                } else if deadline != 0 {
                    assert_eq!(tick, deadline, "release tick");
                    assert_eq!(until, 0);
                    assert_eq!(w.fire_cmd, -1, "release edge");
                    assert_eq!(w.shots_fired, shots + 1, "release must fire");
                    assert!(
                        sc.s.events.iter().any(|e| matches!(e,
                            crate::sim::Event::Shot { unit, weapon_class: 2, .. } if *unit == sc.bot
                        )),
                        "release shot owner/class"
                    );
                    eprintln!("STCHG {rules:?} single={single} shot={tick} owner={:?} class=2 fire_cmd={}", sc.bot, w.fire_cmd);
                    releases += 1;
                    deadline = 0;
                }
                shots = w.shots_fired;
            }
            assert!(
                starts >= 2 && releases >= 2,
                "must charge, release and repeat: {rules:?} single={single}"
            );
        }
    }
}

/// The otherwise equal noncharge tap callers remain live in both rules.
#[test]
fn seat_charge_uncharged_tap_controls() {
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let mut sc = seat_gun_scene(
            WeaponClass {
                trigger_single: true,
                rounds_per_clip: 0,
                heat_per_shot: 0.0,
                ..WeaponClass::bf1_rifle()
            },
            true,
            rules,
        );
        let trace = gun_ticks(&mut sc, 10 * HZ);
        let shots: u64 = trace.iter().map(|t| t.0).sum();
        eprintln!("STCHG tap {rules:?} shots={shots}");
        assert!(shots >= 2);
    }
}

/// Entry, exit/reboard, a different seat, and ai_use_pass must discard the
/// previous weapon's deadline. Failed entry leaves the live tenure intact.
#[test]
fn seat_charge_lifecycle_resets_at_successful_boundaries() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(2.0, 0.0, 35.0),
        1,
        None,
    );
    // Keep autonomous boarding closed while the foot weapon really charges.
    sc.s.seat_setup[0].as_mut().unwrap().boardable[0] = false;
    sc.s.weapon_classes[0].max_pressed_time = 2.0;
    sc.s.units[sc.bot.index as usize].soldier.yaw = std::f32::consts::PI;
    let bot = sc.bot;
    let charging = |s: &Sim| s.unit(bot).unwrap().ai.charge_until > s.tick + 2;
    assert!(
        until(&mut sc.s, 10 * HZ, charging).is_some(),
        "real foot charge"
    );
    assert!(sc.s.unit(bot).unwrap().weapon.fire_cmd > 0);
    sc.s.seat_setup[0].as_mut().unwrap().boardable[0] = true;
    assert!(sc.s.enter_vehicle(bot, sc.v, 0));
    assert_eq!(sc.s.unit(bot).unwrap().ai.charge_until, 0, "foot to seat");
    assert!(
        until(&mut sc.s, 10 * HZ, charging).is_some(),
        "real seat charge"
    );
    let deadline = sc.s.unit(bot).unwrap().ai.charge_until;
    assert!(!sc.s.enter_vehicle(bot, sc.v, 0));
    assert_eq!(
        sc.s.unit(bot).unwrap().ai.charge_until,
        deadline,
        "refused entry"
    );
    sc.s.exit_vehicle(bot);
    assert_eq!(sc.s.unit(bot).unwrap().ai.charge_until, 0, "exit");
    assert!(sc.s.enter_vehicle(bot, sc.v, 0));
    assert_eq!(sc.s.unit(bot).unwrap().ai.charge_until, 0, "reboard");
    sc.s.exit_vehicle(bot);
    sc.s.seat_setup[0].as_mut().unwrap().boardable[0] = false;
    sc.s.units[bot.index as usize].soldier.pos = Vec3::new(3.0, 0.0, -3.0);
    sc.s.units[bot.index as usize].soldier.yaw = std::f32::consts::PI;
    assert!(
        until(&mut sc.s, 10 * HZ, charging).is_some(),
        "foot charge before AI boarding"
    );
    sc.s.seat_setup[0].as_mut().unwrap().boardable[0] = true;
    let ai = &mut sc.s.units[bot.index as usize].ai;
    ai.ride = Some(crate::ai::RideState {
        board: Some(sc.v),
        use_req: true,
        ..Default::default()
    });
    sc.s.ai_use_pass();
    assert!(
        sc.s.unit(bot).unwrap().mount.is_some(),
        "AI public boarding path"
    );
    assert_eq!(
        sc.s.unit(bot).unwrap().ai.charge_until,
        0,
        "local AI writeback"
    );
}

/// STCHG's preserved deadline also reaches the ordinary mobile-gunner
/// dispatch, with an occupied pilot seat, through real simulation steps.
#[test]
fn seat_charge_mobile_gunner_dispatch_and_seat_switch() {
    let mut sc = seat_gun_scene(
        WeaponClass {
            max_pressed_time: 2.0,
            trigger_single: true,
            rounds_per_clip: 0,
            heat_per_shot: 0.0,
            ..WeaponClass::bf1_rifle()
        },
        true,
        crate::soldier::GroundRules::Bf2,
    );
    sc.s.exit_vehicle(sc.bot);
    let setup = sc.s.seat_setup[0].as_mut().unwrap();
    let gun = setup.guns[0].clone();
    setup.guns = vec![None, gun];
    setup.boardable = vec![true, true];
    let pilot = sc.s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
    let v = sc.s.vehicle_mut(sc.v).unwrap();
    v.kind = VehicleKind::Hover;
    v.seats = vec![
        crate::vehicle::Seat {
            kind: SeatKind::Pilot,
            occupant: None,
        },
        crate::vehicle::Seat {
            kind: SeatKind::Gunner(0),
            occupant: None,
        },
    ];
    v.guns.clear();
    assert!(sc.s.enter_vehicle(pilot, sc.v, 0));
    sc.s.units[sc.bot.index as usize].ai.charge_until = 999;
    assert!(sc.s.enter_vehicle(sc.bot, sc.v, 1));
    assert_eq!(
        sc.s.unit(sc.bot).unwrap().ai.charge_until,
        0,
        "different seat's tenure"
    );
    let mut held = false;
    let mut release = false;
    for _ in 0..10 * HZ {
        sc.s.step(&[]);
        assert_eq!(sc.s.unit(sc.bot).unwrap().mount.unwrap().seat, 1);
        let w = &sc.s.vehicle(sc.v).unwrap().guns[1]
            .as_ref()
            .unwrap()
            .weapons[0];
        held |= w.fire_cmd > 0;
        release |= held && w.fire_cmd == -1 && w.shots_fired > 0;
    }
    assert!(
        held && release,
        "mobile gunner must hold and fire on release"
    );
}

/// Losing the visible target uses the foot policy: release immediately,
/// clearing the deadline, even though this can fire a release weapon.
#[test]
fn seat_charge_lost_target_uses_foot_release_policy() {
    let mut sc = seat_gun_scene(
        WeaponClass {
            max_pressed_time: 2.0,
            trigger_single: true,
            rounds_per_clip: 0,
            heat_per_shot: 0.0,
            ..WeaponClass::bf1_rifle()
        },
        true,
        crate::soldier::GroundRules::Bf2,
    );
    let bot = sc.bot;
    assert!(
        until(&mut sc.s, 5 * HZ, |s| s.unit(bot).unwrap().ai.charge_until
            > s.tick + 2)
        .is_some()
    );
    let mut st = sc.s.unit(bot).unwrap().ai;
    st.target = None;
    let mut pad = crate::sim::PadState::default();
    crate::ai::gunner::aim(
        &sc.s,
        bot,
        &mut st,
        &mut pad,
        crate::ai::gunner::Look::Seat,
        None,
    );
    assert_eq!(st.charge_until, 0);
    assert_eq!(pad.buttons & crate::sim::buttons::FIRE, 0);
}

#[test]
fn audit_elite_preferences_keep_the_turret_arc() {
    let mut sc = scene(
        Vec3::new(3.0, 0.0, -3.0),
        Vec3::new(2.0, 0.0, 35.0),
        0,
        Some([-20.0, 20.0]),
    );
    sc.s.team_difficulty[0] = Some(crate::skill::Difficulty::Elite);
    sit(&mut sc);
    let outside =
        sc.s.spawn(&UnitSpec::trooper(1, Vec3::new(15.0, 0.0, 20.0)))
            .unwrap();
    sc.s.unit_mut(outside).unwrap().ai.target = Some(sc.bot.into());
    let mut st = crate::ai::AiState::default();
    crate::ai::think(&sc.s, sc.bot, &mut st);
    assert!(
        st.contacts
            .iter()
            .flatten()
            .any(|c| c.handle == outside && c.seen_now),
        "positive control: outside-arc threat is perceived"
    );
    assert_eq!(
        st.target,
        Some(sc.enemy.into()),
        "preference must not restore an out-of-arc threat"
    );
}
