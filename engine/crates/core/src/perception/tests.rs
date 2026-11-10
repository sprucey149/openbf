use super::*;
use crate::ai::{think, AiState, THINK_PERIOD};
use crate::collision::{Body, Shape};
use crate::heightfield::Heightfield;
use crate::sim::{buttons, Sim};
use crate::skill::Difficulty;
use crate::unit::{Controller, UnitSpec};
use glam::Mat4;

/// Flat ground everywhere (without a heightfield units spawn at
/// `NO_GROUND` and fall away from anything placed for them).
fn ground() -> Heightfield {
    Heightfield {
        size: 2,
        unit: 4000.0,
        offset_x: 2000.0,
        offset_z: 2000.0,
        heights: vec![0.0; 4],
    }
}

/// An AI soldier at the origin facing −Z, on flat ground.
fn watcher(seed: u32) -> (Sim, crate::unit::UnitHandle) {
    let mut s = Sim::empty(seed);
    s.heightfield = Some(ground());
    let a = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    (s, a)
}

fn enemy_at(s: &mut Sim, p: Vec3) -> crate::unit::UnitHandle {
    s.spawn(&UnitSpec::trooper(1, p)).unwrap()
}

/// Normal's columns (Easy's are the same): view ×1.0, 9 reaction ticks.
fn normal() -> Tier {
    Difficulty::Normal.tier()
}

/// Line of sight is blocked by a WORLD model, not only by terrain: an
/// enemy on the far side of a wall body is invisible, and visible the
/// moment the wall is gone. Before this the AI shot through every
/// building on the map. (Phase C moved this test with `ai::visible` →
/// `perception::los`, brief D5.)
#[test]
fn a_wall_body_blocks_line_of_sight() {
    let build = |wall: bool| {
        let (mut s, a) = watcher(11);
        let b = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
        if wall {
            // Halfway between them, y in (0, 10): taller and wider than
            // either soldier, so nothing sees over or round it.
            s.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
                Shape::Box {
                    half: Vec3::new(5.0, 5.0, 0.5),
                },
            ));
        }
        (s, a, b)
    };
    let eye_and_target = |s: &Sim, a, b| {
        (
            s.unit(a).unwrap().muzzle(),
            s.unit(b).unwrap().capsule().center(),
        )
    };

    let (s, a, b) = build(true);
    let (eye, tgt) = eye_and_target(&s, a, b);
    assert!(!los(&s, eye, tgt), "the wall blocks");
    assert!(!los(&s, tgt, eye), "and blocks looking back");
    // The terrain half alone still says clear — it is the world ray that
    // is doing the work here, and the two halves are what the sweep's
    // `BLOCKED terrain|world|both` column reads.
    assert!(los_terrain(&s, eye, tgt), "terrain is clear");
    assert!(!los_world(&s, eye, tgt), "the world body is not");

    let (s, a, b) = build(false);
    let (eye, tgt) = eye_and_target(&s, a, b);
    assert!(los(&s, eye, tgt), "no wall, clear");
    // A wall PAST the target does not block it.
    let (mut s, a, b) = build(false);
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, -30.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    let (eye, tgt) = eye_and_target(&s, a, b);
    assert!(los(&s, eye, tgt), "the wall is behind the target");

    // ... and the AI acts on it: it targets the enemy it can see and
    // never the one behind the wall.
    let targeted = |wall: bool| {
        let (s, a, _) = build(wall);
        let mut st = AiState::default();
        think(&s, a, &mut st);
        st.target
    };
    assert!(targeted(false).is_some(), "targets a visible enemy");
    assert_eq!(targeted(true), None, "never targets through a wall");
}

/// The cone is the PS2's (RE §9.1): a 77.5° half-angle about the aim
/// direction, with BF2's 10 m omni bubble that BF1 does not have.
#[test]
fn the_view_cone_is_77_5_degrees_over_bf2s_ten_metre_omni() {
    let view = Vec3::new(0.0, 0.0, -1.0);
    let at = |deg: f32, dist: f32| {
        let r = deg.to_radians();
        Vec3::new(r.sin() * dist, 0.0, -r.cos() * dist)
    };
    for &omni in &[0.0f32, 10.0] {
        for &(deg, inside) in &[
            (0.0, true),
            (60.0, true),
            (77.0, true),
            (78.0, false),
            (120.0, false),
        ] {
            let off = at(deg, 20.0);
            assert_eq!(
                in_cone(view, off, off.dot(off), omni),
                inside,
                "{deg}° at 20 m, omni {omni}"
            );
        }
    }
    // Behind the unit at 5 m: BF2 sees it (inside the bubble), BF1 does
    // not (RE §9.1 — BF1 has the same cone with NO omni).
    let back = at(180.0, 5.0);
    let d2 = back.dot(back);
    assert!(in_cone(view, back, d2, 10.0), "BF2's 10 m omni");
    assert!(!in_cone(view, back, d2, 0.0), "BF1 has no omni");
    // The bubble ends exactly at its radius.
    let just_out = at(180.0, 10.5);
    assert!(!in_cone(view, just_out, just_out.dot(just_out), 10.0));
}

/// The view distance is `(20·bound_radius + 30) × mult × vis` with the
/// bound radius half the capsule height (RE §5.1; PROVISIONAL — the PS2's
/// bound is the model's).
/// trace: PS2 model bounds and stance-dependent sight ranges before replacing the capsule-
/// height bound approximation.
#[test]
fn view_distance_by_stance_and_tier() {
    let close = |a: f32, b: f32| assert!((a - b).abs() < 1e-3, "{a} vs {b}");
    close(view_distance(Stance::Stand, 1.0), 49.0);
    close(view_distance(Stance::Crouch, 1.0), 46.0);
    close(view_distance(Stance::Prone, 1.0), 35.5);
    // The tiers scale it: Easy and Normal 1.0, Hard 1.25, Elite 1.5.
    for (d, mult) in [
        (Difficulty::Easy, 1.0),
        (Difficulty::Normal, 1.0),
        (Difficulty::Hard, 1.25),
        (Difficulty::Elite, 1.5),
    ] {
        close(
            view_distance(Stance::Stand, d.tier().view_mult),
            49.0 * mult,
        );
    }
    // ... and an enemy just past it is not a contact.
    let (mut s, a) = watcher(3);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -50.0));
    let mut st = AiState::default();
    assert_eq!(scan(&s, a, &mut st, normal()), (0, 0), "50 m > 49 m");
    let (mut s, a) = watcher(3);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -48.0));
    let mut st = AiState::default();
    assert_eq!(scan(&s, a, &mut st, normal()).0, 1, "48 m < 49 m");
}

/// Card MAIS: the mission's `SetAIViewMultiplier` shortens every bot's
/// sight, on top of its tier's: Dagobah's 0.35 at Normal is 17 m, so an
/// enemy 30 m off is not seen; at Hard (x 1.25, 21 m) one 20 m off is.
#[test]
fn the_missions_view_multiplier_shortens_every_bots_sight() {
    let seen = |s: &Sim, a| {
        let mut st = AiState::default();
        think(s, a, &mut st);
        st.contacts.iter().flatten().any(|c| c.seen_now)
    };
    let (mut s, a) = watcher(3);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -30.0));
    assert!(seen(&s, a), "30 m < 49 m");
    s.mission_ai.set_view_mult(0.35);
    assert!((s.ai_tier(0).view_mult - 0.35).abs() < 1e-6);
    assert!(!seen(&s, a), "30 m > 17 m");
    let (mut s, a) = watcher(3);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    s.mission_ai.set_view_mult(0.35);
    assert!(!seen(&s, a), "20 m > 17 m at Normal");
    s.difficulty = Difficulty::Hard;
    assert!((s.ai_tier(0).view_mult - 1.25 * 0.35).abs() < 1e-6);
    assert!(seen(&s, a), "20 m < 21 m at Hard");
}

/// OURS (2026-10-29, long-range bot sniping, tied to the tiers by the
/// maintainer): a bot standing still with a scoped rifle in hand sees a
/// standing enemy out to its tier's share of the rifle's `MaxRange` (128
/// m), times the mission's view multiplier: Easy none (the plain 49 m),
/// Normal 0.75 (96 m), Hard and Elite all of it; a crouching one as much
/// nearer as the plain sight is. Not with the rifle slung, not on the
/// move; a proxy sees as Hard does on any tier.
/// measure: stationary and moving scoped-sniper detection distances by tier and mission view
/// multiplier before replacing this deliberate port rule.
#[test]
fn a_still_sniper_sees_out_to_its_tiers_share_of_its_rifles_range() {
    let rifle = |s: &mut Sim| {
        s.add_weapon_class(crate::weapon::WeaponClass {
            trigger_single: true,
            zoom_max: 8.0,
            yaw_spread: 0.0,
            ai_range: [20.0, 64.0, 128.0],
            ..crate::weapon::WeaponClass::bf2_rifle()
        })
    };
    let seen = |s: &Sim, a| {
        let mut st = AiState::default();
        scan(s, a, &mut st, s.unit_tier(s.unit(a).unwrap()));
        st.contacts.iter().flatten().any(|c| c.seen_now)
    };
    let scene = |d: Difficulty, at: f32, stance: Stance| {
        let (mut s, a) = watcher(3);
        s.difficulty = d;
        let w = rifle(&mut s);
        s.units[a.index as usize].weapon_class = w;
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -at));
        s.units[e.index as usize].soldier.stance = stance;
        (s, a)
    };
    let sees = |d, at| {
        let (s, a) = scene(d, at, Stance::Stand);
        seen(&s, a)
    };
    // Per tier, a standing enemy at 48, 80, 110 and 120 m.
    for (d, want) in [
        (Difficulty::Easy, [true, false, false, false]),
        (Difficulty::Normal, [true, true, false, false]),
        (Difficulty::Hard, [true, true, true, true]),
        (Difficulty::Elite, [true, true, true, true]),
    ] {
        let got = [48.0, 80.0, 110.0, 120.0].map(|at| sees(d, at));
        assert_eq!(got, want, "{d:?}");
    }
    assert!(sees(Difficulty::Normal, 94.0), "Normal: 94 m < 96 m");
    assert!(!sees(Difficulty::Hard, 130.0), "Hard: 130 m > 128 m");
    // Crouching at Hard: 128 x 46 / 49 = 120 m.
    let (s, a) = scene(Difficulty::Hard, 115.0, Stance::Crouch);
    assert!(seen(&s, a), "a crouching enemy at 115 m");
    let (s, a) = scene(Difficulty::Hard, 125.0, Stance::Crouch);
    assert!(!seen(&s, a), "but not at 125 m");
    // Walking, it sees as anyone does.
    let (mut s, a) = scene(Difficulty::Hard, 80.0, Stance::Stand);
    s.units[a.index as usize].soldier.vel = Vec3::new(3.0, 0.0, 0.0);
    assert!(!seen(&s, a), "not on the move");
    // The rifle slung (a plain rifle in hand).
    let (mut s, a) = scene(Difficulty::Hard, 80.0, Stance::Stand);
    s.units[a.index as usize].weapon_class = 0;
    assert!(!seen(&s, a), "not with a plain rifle");
    // The mission's view multiplier shortens the scope too: 128 x 0.5.
    let (mut s, a) = scene(Difficulty::Hard, 70.0, Stance::Stand);
    s.mission_ai.set_view_mult(0.5);
    assert!(!seen(&s, a), "70 m > 64 m at x 0.5");
    let (mut s, a) = scene(Difficulty::Hard, 60.0, Stance::Stand);
    s.mission_ai.set_view_mult(0.5);
    assert!(seen(&s, a), "60 m < 64 m at x 0.5");
    // A proxy at Easy sees as Hard does.
    let (mut s, a) = scene(Difficulty::Easy, 120.0, Stance::Stand);
    s.units[a.index as usize].proxy_human = Some(crate::ai::proxy::Strength::Average.profile());
    assert!(seen(&s, a), "a proxy's scope at Easy");
}

/// A contact is kept for MEMORY_TICKS after the last sighting and cleared
/// on the think after that.
#[test]
fn memory_expires_three_seconds_after_the_last_sighting() {
    let (mut s, a) = watcher(5);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let mut st = AiState::default();
    scan(&s, a, &mut st, normal());
    let c = contact_of(&st, e).expect("seen");
    assert!(c.seen_now && !c.heard && c.first_seen == 0);
    // Take the sight away (a wall between them) and step the clock.
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    s.tick = MEMORY_TICKS;
    scan(&s, a, &mut st, normal());
    let c = contact_of(&st, e).expect("still remembered at 90 ticks");
    assert!(!c.seen_now, "not seen, only remembered");
    assert_eq!(c.last_pos, Vec3::new(0.0, 0.0, -20.0) + Vec3::Y * 0.95);
    s.tick = MEMORY_TICKS + 1;
    scan(&s, a, &mut st, normal());
    assert_eq!(contact_of(&st, e), None, "forgotten at 91 ticks");
}

/// Six slots: a seventh enemy takes the slot whose record is oldest.
#[test]
fn a_seventh_contact_evicts_the_oldest() {
    let (mut s, a) = watcher(9);
    let mut seen = Vec::new();
    for i in 0..CONTACTS {
        seen.push(enemy_at(&mut s, Vec3::new(i as f32 * 2.0, 0.0, -20.0)));
    }
    let mut st = AiState::default();
    assert_eq!(scan(&s, a, &mut st, normal()).0, CONTACTS as u8);
    // Age the first contact only: hide it behind a wall, keep the rest.
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(0.9, 5.0, 0.5),
        },
    ));
    s.tick = 30;
    scan(&s, a, &mut st, normal());
    assert!(!contact_of(&st, seen[0]).unwrap().seen_now, "hidden");
    // A seventh enemy arrives; the memory is full, so the oldest goes.
    let newcomer = enemy_at(&mut s, Vec3::new(-4.0, 0.0, -18.0));
    s.tick = 60;
    scan(&s, a, &mut st, normal());
    assert_eq!(contact_of(&st, seen[0]), None, "the oldest was evicted");
    assert!(contact_of(&st, newcomer).is_some(), "the seventh is held");
    assert_eq!(st.contacts.iter().flatten().count(), CONTACTS);
}

/// A first sighting is not a shot: the tier's reaction time must pass
/// first (`reaction_ticks`, PROVISIONAL). Easy and Normal are 9 ticks (0.3 s).
/// measure: first-visible-target to first-shot ticks in controlled PS2 encounters by difficulty
/// before replacing the reaction delays.
#[test]
fn a_first_sighting_only_fires_after_the_reaction_time() {
    let (mut s, a) = watcher(13);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let r = normal().reaction_ticks;
    assert_eq!(r, 9);
    let mut st = AiState::default();
    // Tick 0: seen for the first time, no shot.
    let (pad, _) = think(&s, a, &mut st);
    assert!(st.target.is_some(), "target acquired");
    assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot on sight");
    let c = contact_of(&st, st.target.unwrap()).unwrap();
    assert!(!fireable(&c, 0, r) && !fireable(&c, r - 1, r));
    assert!(fireable(&c, r, r), "fireable once the reaction has passed");
    assert!(THINK_PERIOD > r, "the reaction is inside one think period");
    // ... and the think agrees: nothing before tick r, and the trigger
    // does go down after it (the aim tracker's slew, brief D9, still costs
    // a few ticks of alignment).
    for t in 1..r {
        s.tick = t;
        let (pad, _) = think(&s, a, &mut st);
        assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot at tick {t}");
    }
    let mut fired = None;
    for t in r..r + 60 {
        s.tick = t;
        let (pad, _) = think(&s, a, &mut st);
        if pad.buttons & buttons::FIRE != 0 {
            fired = Some(t);
            break;
        }
    }
    assert!(fired.is_some(), "fires once the reaction has passed");
}

/// Hearing (design decision 2): a shot inside HEAR_RADIUS is a contact
/// with no line of sight at all; one outside it is not; and being hit
/// records the shooter at any range.
#[test]
fn hearing_records_a_shooter_behind_a_wall() {
    let heard_at = |dist: f32| {
        let (mut s, a) = watcher(17);
        // Behind the listener AND behind a wall: neither the cone nor
        // the LOS ray can find this one.
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, dist));
        s.collision.push_body(Body::new(
            Mat4::from_translation(Vec3::new(0.0, 5.0, dist / 2.0)),
            Shape::Box {
                half: Vec3::new(5.0, 5.0, 0.5),
            },
        ));
        s.units[e.index as usize].last_shot_tick = 0;
        s.tick = 1;
        let mut st = AiState::default();
        let counts = scan(&s, a, &mut st, normal());
        (counts, contact_of(&st, e))
    };
    let ((held, heard), c) = heard_at(25.0);
    assert_eq!((held, heard), (1, 1), "a shot at 25 m is heard");
    let c = c.expect("heard contact");
    assert!(c.heard && !c.seen_now, "heard, not seen");
    assert_eq!(c.last_pos, Vec3::new(0.0, 0.0, 25.0), "the shooter's feet");
    let ((held, heard), c) = heard_at(35.0);
    assert_eq!((held, heard), (0, 0), "a shot at 35 m is not");
    assert_eq!(c, None);

    // Being HIT records the shooter at any range.
    let (mut s, a) = watcher(19);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 80.0));
    s.units[a.index as usize].last_hit = Some((e, 0));
    s.tick = 1;
    let mut st = AiState::default();
    assert_eq!(scan(&s, a, &mut st, normal()), (1, 1), "hit from 80 m");
    assert!(contact_of(&st, e).unwrap().heard);
}

/// Card TEAM: a scene for passing contacts on - the watcher at the
/// origin facing -Z, an enemy 30 m behind it beyond a wall (it can
/// neither see nor hear it), and a friend at `friend` whose last think
/// saw the enemy (`seen` false: only remembered it).
fn told_scene(friend: Vec3, seen: bool) -> (Sim, UnitHandle, UnitHandle, UnitHandle) {
    let (mut s, a) = watcher(31);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 30.0));
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, 15.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    let f = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, friend)
        })
        .unwrap();
    s.units[f.index as usize].ai.contacts[0] = Some(Contact {
        handle: e.into(),
        first_seen: 0,
        last_seen: 0,
        last_pos: Vec3::new(0.0, 0.9, 30.0),
        seen_now: seen,
        heard: false,
        told: false,
    });
    s.tick = 5;
    (s, a, e, f)
}

/// Card TEAM (bot design 4.1): what a friend within 20 m sees is heard -
/// at the friend's point, its reaction counted from now - and nothing is
/// passed on from 20.5 m or from a memory.

#[test]
fn a_friend_within_twenty_metres_passes_on_what_it_sees() {
    let told = |friend: Vec3, seen: bool, tier: Tier| {
        let (s, a, e, _) = told_scene(friend, seen);
        let mut st = AiState::default();
        assert_eq!(scan(&s, a, &mut st, tier), (0, 0), "not seen, not heard");
        let n = tell(&s, a, &mut st);
        (n, contact_of(&st, e))
    };
    let (n, c) = told(Vec3::new(15.0, 0.0, 0.0), true, normal());
    let c = c.expect("told");
    assert_eq!(n, 1);
    assert!(c.heard && c.told && !c.seen_now, "heard, told, not seen");
    assert_eq!(c.last_pos, Vec3::new(0.0, 0.9, 30.0), "the friend's point");
    assert_eq!(c.first_seen, 5, "the reaction counts from the telling");
    assert_eq!(
        told(Vec3::new(19.5, 0.0, 0.0), true, normal()).0,
        1,
        "19.5 m"
    );
    assert_eq!(
        told(Vec3::new(20.5, 0.0, 0.0), true, normal()),
        (0, None),
        "20.5 m"
    );
    assert_eq!(
        told(Vec3::new(15.0, 0.0, 0.0), false, normal()),
        (0, None),
        "a memory"
    );
    // Every tier tells (the design's column was every tier but Easy).
    for d in Difficulty::ALL {
        assert_eq!(
            told(Vec3::new(15.0, 0.0, 0.0), true, d.tier()).0,
            1,
            "{d:?}"
        );
    }
}

/// Card TEAM: a friend's telling never overwrites the unit's own record
/// of the enemy (its own memory lapses as it would untold), and sight
/// clears the told mark.
#[test]
fn a_told_record_never_overwrites_the_units_own() {
    let (mut s, a, e, _) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
    let own = Contact {
        handle: e.into(),
        first_seen: 1,
        last_seen: 2,
        last_pos: Vec3::new(3.0, 0.0, 28.0),
        seen_now: false,
        heard: true,
        told: false,
    };
    let mut st = AiState::default();
    st.contacts[0] = Some(own);
    assert_eq!(tell(&s, a, &mut st), 0, "its own record stands");
    assert_eq!(contact_of(&st, e), Some(own));
    // Its own memory lapses; then the friend's sight is told.
    s.tick = 2 + MEMORY_TICKS + 1;
    scan(&s, a, &mut st, normal());
    assert_eq!(contact_of(&st, e), None, "forgotten");
    assert_eq!(tell(&s, a, &mut st), 1);
    assert!(contact_of(&st, e).is_some_and(|c| c.told));
    // Seen, it is the unit's own again.
    see(&mut st, e.into(), s.tick + 1, Vec3::new(0.0, 0.9, 30.0));
    assert!(contact_of(&st, e).is_some_and(|c| c.seen_now && !c.told && !c.heard));
}

/// Card TEAM: a told enemy keeps nobody on a position. A bot hiding at a
/// covered position (`COVER`), or holding one with time left on it,
/// whose target only a friend sees lets it go - after `HOLD_CLOSE`, or
/// once it has been quiet for `HOLD_QUIET`; the same bot with a record
/// of its OWN (the control: told by hand each tick, as its ears would)
/// keeps it the whole 10 s.
#[test]
fn a_told_enemy_keeps_nobody_on_a_position() {
    use crate::ai::{FightState, HoldState, HOLD_CLOSE};
    let kept = |own: bool, cover: bool| {
        let (mut s, a, e, f) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
        let mut st = AiState {
            hold: Some(HoldState {
                until: 5 + if cover {
                    HOLD_CLOSE
                } else {
                    20 * crate::TICK_HZ as u64
                },
                last_contact: 5,
                ..HoldState::default()
            }),
            ..AiState::default()
        };
        st.fight.mode = if cover { FightState::COVER } else { 0 };
        if cover {
            // (At COVER a bot holds a position of the table, where it
            // stands: card TPOSR runs the Tactician on its own record.)
            use crate::ai::tactic::{Position, PositionTable, Source};
            let q = Position {
                pos: s.units[a.index as usize].soldier.pos,
                source: Source::Edge,
                facing: glam::Vec2::ZERO,
                stance: 0,
                stance2: 0,
                hub: u16::MAX,
                low: 0,
                corner: 0,
                shut: 0,
                tall: 0,
            };
            s.positions = PositionTable::index(vec![q], Default::default());
            st.tpos.held = Some(0);
        }
        for t in 5..5 + 10 * crate::TICK_HZ as u64 {
            s.tick = t;
            if let Some(c) = s.units[f.index as usize].ai.contacts[0].as_mut() {
                c.seen_now = true;
                c.last_seen = t;
            }
            if own {
                st.contacts[0] = Some(Contact {
                    handle: e.into(),
                    first_seen: 5,
                    last_seen: t,
                    last_pos: Vec3::new(0.0, 0.9, 30.0),
                    seen_now: false,
                    heard: true,
                    told: false,
                });
            }
            think(&s, a, &mut st);
        }
        assert_eq!(
            st.target,
            Some(e.into()),
            "own {own}: the enemy is the target"
        );
        st.hold.is_some_and(|h| s.tick < h.until)
    };
    for cover in [true, false] {
        assert!(kept(true, cover), "cover {cover}: its own record keeps it");
        assert!(!kept(false, cover), "cover {cover}: a friend's does not");
    }
}

/// Card TEAM: a told enemy becomes the target and is aimed at, and is
/// never fired at while the bot cannot see it, however long it is told.
#[test]
fn a_told_enemy_is_aimed_at_and_never_fired_at_unseen() {
    let (mut s, a, e, f) = told_scene(Vec3::new(15.0, 0.0, 0.0), true);
    let mut st = AiState::default();
    for t in 5..5 + 4 * THINK_PERIOD * 10 {
        s.tick = t;
        if let Some(c) = s.units[f.index as usize].ai.contacts[0].as_mut() {
            c.seen_now = true;
            c.last_seen = t;
        }
        let (pad, _) = think(&s, a, &mut st);
        assert_eq!(pad.buttons & buttons::FIRE, 0, "no shot at tick {t}");
    }
    assert_eq!(st.target, Some(e.into()), "the told enemy is the target");
    let c = contact_of(&st, e).expect("still held");
    assert!(c.heard && !c.seen_now);
}

/// Target choice (decision 3): the current target is kept until a rival
/// beats it by more than HYSTERESIS, and never before RETARGET_TICKS.
#[test]
fn hysteresis_and_the_retarget_interval() {
    // Two enemies in view; the score is the distance term alone (no
    // humans, both seen), so the closer one wins by
    // 100·(d₂ − d₁)/view_dist points.
    let scale = view_distance(Stance::Stand, 1.0);
    let gap = |points: f32| points * scale / 100.0;
    let build = |second: f32| {
        let (mut s, a) = watcher(23);
        let far = enemy_at(&mut s, Vec3::new(0.0, 0.0, -30.0));
        let near = enemy_at(&mut s, Vec3::new(0.0, 0.0, -(30.0 - second)));
        (s, a, far, near)
    };
    // Start with the FAR one as the target, at tick 0.
    let run = |second: f32, tick: u64| {
        let (mut s, a, far, near) = build(second);
        s.tick = tick;
        let mut st = AiState {
            target: Some(far.into()),
            last_switch: 0,
            ..Default::default()
        };
        scan(&s, a, &mut st, normal());
        // The far one must still be a contact for the "keeps" case to
        // mean anything.
        assert!(contact_of(&st, far).is_some());
        choose_target(&s, a, &mut st, normal());
        (st.target == Some(near.into()), far, near)
    };
    let (switched, ..) = run(gap(9.0), RETARGET_TICKS);
    assert!(!switched, "+9 points does not beat +10 hysteresis");
    let (switched, ..) = run(gap(11.0), RETARGET_TICKS);
    assert!(switched, "+11 points does");
    let (switched, ..) = run(gap(11.0), RETARGET_TICKS - 1);
    assert!(!switched, "... but never before the 1.5 s interval");
    // A target that left the memory is dropped at once, interval or not.
    let (mut s, a, far, near) = build(gap(11.0));
    s.units[far.index as usize].alive = false;
    let mut st = AiState {
        target: Some(far.into()),
        last_switch: 0,
        ..Default::default()
    };
    scan(&s, a, &mut st, normal());
    choose_target(&s, a, &mut st, normal());
    assert_eq!(
        st.target,
        Some(near.into()),
        "a lost target is replaced at once"
    );
}

/// The PRESSURE weight (decision 3): at Hard a human is worth ×1.3, so a
/// farther human outscores a nearer bot; at Normal (×1.0) it does not.
#[test]
fn the_pressure_weight_prefers_a_farther_human_at_elite() {
    let pick = |tier: Tier| {
        let mut s = Sim::empty(29);
        s.heightfield = Some(ground());
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let bot = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(4.0, 0.0, -10.0)))
            .unwrap();
        let human = s
            .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -30.0)))
            .unwrap();
        let mut st = AiState::default();
        scan(&s, a, &mut st, tier);
        assert_eq!(st.contacts.iter().flatten().count(), 2, "both in view");
        choose_target(&s, a, &mut st, tier);
        (st.target, bot, human)
    };
    let (t, bot, _) = pick(normal());
    assert_eq!(t, Some(bot.into()), "Normal takes the nearer bot");
    let (t, _, human) = pick(Difficulty::Hard.tier());
    assert_eq!(t, Some(human.into()), "Hard leans on the human");
}

/// `[ours]` (the maintainer 2026-10-29): no bot takes a human as its
/// target in his first second alive; a bot still may.
#[test]
fn a_human_is_nobodys_target_in_his_first_second() {
    let mut s = Sim::empty(31);
    s.heightfield = Some(ground());
    let a = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let human = s
        .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -10.0)))
        .unwrap();
    crate::ai::dfocus::upkeep(&mut s);
    let born = s.tick;
    let mut st = AiState::default();
    scan(&s, a, &mut st, normal());
    assert_eq!(st.contacts.iter().flatten().count(), 1, "seen");
    choose_target(&s, a, &mut st, normal());
    assert_eq!(st.target, None, "just spawned");
    s.tick = born + crate::ai::dfocus::UNTARGETED - 1;
    choose_target(&s, a, &mut st, normal());
    assert_eq!(st.target, None, "still in his first second");
    s.tick = born + crate::ai::dfocus::UNTARGETED;
    choose_target(&s, a, &mut st, normal());
    assert_eq!(st.target, Some(human.into()), "fair game after a second");
    let bot = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -5.0)))
        .unwrap();
    crate::ai::dfocus::upkeep(&mut s);
    let mut st = AiState::default();
    scan(&s, a, &mut st, normal());
    choose_target(&s, a, &mut st, normal());
    assert_eq!(st.target, Some(bot.into()), "a new bot is a target at once");
}

/// A unit that LOSES sight aims at the last known position and does not
/// fire at it (brief D7).
#[test]
fn a_remembered_target_is_aimed_at_but_never_shot_at() {
    let (mut s, a) = watcher(31);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let mut st = AiState::default();
    let mut fired = false;
    for t in 0..60u64 {
        s.tick = t;
        let (pad, _) = think(&s, a, &mut st);
        fired |= pad.buttons & buttons::FIRE != 0;
    }
    assert!(fired, "shoots what it sees");
    // The enemy steps behind a wall: the memory keeps it, the trigger
    // does not.
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    s.tick += THINK_PERIOD;
    let (pad, _) = think(&s, a, &mut st);
    assert_eq!(st.target, Some(e.into()), "still the target");
    let c = contact_of(&st, e).unwrap();
    assert!(!c.seen_now, "out of sight");
    assert_eq!(pad.buttons & buttons::FIRE, 0, "never fires at a memory");
}

/// `PadState` is not part of this module's contract, but the fairness
/// rule is: every FIRE the AI presses is at a contact it holds.
#[test]
fn every_ai_shot_is_at_a_held_contact() {
    let (mut s, a) = watcher(37);
    enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let mut st = AiState::default();
    for t in 0..90u64 {
        s.tick = t;
        let (pad, _) = think(&s, a, &mut st);
        if pad.pressed(buttons::FIRE) {
            let target = st.target.expect("fired with no target");
            assert!(contact_of(&st, target).is_some(), "fired at tick {t}");
        }
    }
}

/// An AI soldier facing −Z on a platform 5 m over flat ground: a box floor
/// 60 m square and half a metre thick, its top at y = 5, pushed before the
/// spawn so the unit stands on it.
fn platform_watcher(seed: u32) -> (Sim, crate::unit::UnitHandle) {
    let mut s = Sim::empty(seed);
    s.heightfield = Some(ground());
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 4.75, 0.0)),
        Shape::Box {
            half: Vec3::new(30.0, 0.25, 30.0),
        },
    ));
    let a = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(0.0, 5.0, 0.0))
        })
        .unwrap();
    (s, a)
}

/// Playtest C4 (BF1 Kashyyyk Docks): a sniper on a platform aimed straight
/// down through its floor at the fight beneath. A floor is the same ORDNANCE
/// body a bolt flies through, so it blocks the sight ray: the trooper
/// shooting under the platform is HEARD, never seen, never fired on, and the
/// gun stays level — the memory is faced, not pitched at. Control: the same
/// enemy on the platform is seen and fired on.
#[test]
fn a_floor_body_blocks_sight_and_aim_downward() {
    let (mut s, a) = platform_watcher(43);
    let eye = s.unit(a).unwrap().ai_eye();
    assert!((eye.y - 5.0) > 1.0, "stands on the platform: eye {eye}");
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -3.0));
    assert!(!los(&s, eye, Vec3::new(0.0, 1.4, -3.0)), "the floor blocks");
    let mut st = AiState::default();
    for t in 0..90u64 {
        s.tick = t;
        s.units[e.index as usize].last_shot_tick = t;
        let (pad, _) = think(&s, a, &mut st);
        assert_eq!(
            pad.buttons & buttons::FIRE,
            0,
            "fired through the floor at tick {t}"
        );
        assert!(
            pad.ry >= 0,
            "pitched down through the floor at tick {t}: ry {}",
            pad.ry
        );
        assert_eq!(st.aim_dir, Vec3::ZERO, "tracking a point it cannot see");
    }
    let c = contact_of(&st, e).expect("the shooter is heard");
    assert!(c.heard && !c.seen_now, "heard, not seen");
    assert_eq!(st.target, Some(e.into()), "it is the target all the same");
    assert!(st.memory_blocked, "the memory is behind the floor");

    // Control: on the platform it is seen, aimed at and shot.
    let (mut s, a) = platform_watcher(43);
    let e = enemy_at(&mut s, Vec3::new(0.0, 5.0, -20.0));
    let mut st = AiState::default();
    let mut fired = false;
    for t in 0..90u64 {
        s.tick = t;
        let (pad, _) = think(&s, a, &mut st);
        fired |= pad.buttons & buttons::FIRE != 0;
    }
    assert!(contact_of(&st, e).is_some_and(|c| c.seen_now), "seen");
    assert!(!st.memory_blocked);
    assert!(fired, "fires at what it sees on the platform");
}

/// A shot heard from behind a wall is a direction to face, not a point to
/// pitch at: the bot turns toward it with the tracker idle and the gun
/// level. With the wall gone the remembered point is tracked as before.
#[test]
fn a_heard_shooter_behind_geometry_is_faced_not_aimed_at() {
    let run = |wall: bool| {
        let (mut s, a) = watcher(47);
        // Behind the listener: the cone never sees it, so it is a memory
        // with or without the wall.
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 25.0));
        if wall {
            s.collision.push_body(Body::new(
                Mat4::from_translation(Vec3::new(0.0, 5.0, 12.5)),
                Shape::Box {
                    half: Vec3::new(5.0, 5.0, 0.5),
                },
            ));
        }
        let mut st = AiState::default();
        s.units[e.index as usize].last_shot_tick = 0;
        s.tick = 1;
        let (pad, _) = think(&s, a, &mut st);
        let c = contact_of(&st, e).expect("heard");
        assert!(c.heard && !c.seen_now);
        assert_eq!(st.target, Some(e.into()));
        (st.memory_blocked, st.aim_dir, pad)
    };
    let (blocked, aim, pad) = run(true);
    assert!(blocked, "the wall is between");
    assert_eq!(aim, Vec3::ZERO, "faced level, not tracked");
    assert_ne!(pad.rx, 0, "turns toward the shot");
    assert_eq!(pad.ry, 0, "the gun stays level");
    let (blocked, aim, _) = run(false);
    assert!(!blocked);
    assert_ne!(aim, Vec3::ZERO, "the open point is tracked");
}

/// A space round's foot bots (a hangar fight in Space Assault) aim as they
/// did: the heard shooter behind the wall is tracked by the memory arm and
/// `memory_blocked` stays clear, so their state and hashes do not move.
#[test]
fn a_space_round_keeps_the_memory_aim_through_geometry() {
    let (mut s, a) = watcher(47);
    s.space_assault = Some(crate::sim::SpaceAssault::new(10));
    assert!(s.space_round());
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 25.0));
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, 12.5)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    let mut st = AiState::default();
    s.units[e.index as usize].last_shot_tick = 0;
    s.tick = 1;
    think(&s, a, &mut st);
    let c = contact_of(&st, e).expect("heard");
    assert!(c.heard && !c.seen_now);
    assert_eq!(st.target, Some(e.into()));
    assert!(!st.memory_blocked, "space: no sight gate");
    assert_ne!(
        st.aim_dir,
        Vec3::ZERO,
        "space: the remembered point is tracked"
    );
}

/// `Sim::space_round`: a space One-Flag round (the space flag) is a space
/// round too; a ground One-Flag round is not.
#[test]
fn a_space_one_flag_round_is_a_space_round() {
    use crate::sim::CtfState;
    let (goal0, goal1) = (Vec3::new(-50.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0));
    let mut s = Sim::empty(1);
    assert!(!s.space_round());
    s.install_ctf(CtfState::new_space_one_flag(5, Vec3::ZERO, goal0, goal1));
    assert!(s.space_round());
    let mut s = Sim::empty(1);
    s.install_ctf(CtfState::new_ground_one_flag(5, Vec3::ZERO, goal0, goal1));
    assert!(!s.space_round());
}

/// The whole state a scan writes is hashed: two sims whose units carry
/// different memories must not hash alike.
#[test]
fn the_contact_memory_is_part_of_the_hash() {
    let (mut s, a) = watcher(41);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let base = s.hash();
    let mut st = AiState::default();
    scan(&s, a, &mut st, normal());
    s.units[a.index as usize].ai = st;
    let with_contact = s.hash();
    assert_ne!(base, with_contact, "a contact is state");
    // Every field of the record, one at a time (`Sim` is not `Clone`, so
    // each probe puts the slot back).
    let probe = |s: &mut Sim, name: &str, f: &dyn Fn(&mut Contact)| {
        let before = s.units[a.index as usize].ai.contacts[0];
        f(s.units[a.index as usize].ai.contacts[0].as_mut().unwrap());
        assert_ne!(s.hash(), with_contact, "{name} is state");
        s.units[a.index as usize].ai.contacts[0] = before;
        assert_eq!(s.hash(), with_contact, "{name} restored");
    };
    probe(&mut s, "first_seen", &|c| c.first_seen += 1);
    probe(&mut s, "last_seen", &|c| c.last_seen += 1);
    probe(&mut s, "last_pos", &|c| c.last_pos.x += 1.0);
    probe(&mut s, "seen_now", &|c| c.seen_now = !c.seen_now);
    probe(&mut s, "heard", &|c| c.heard = !c.heard);
    probe(&mut s, "handle", &|c| {
        if let Foe::Unit(h) = &mut c.handle {
            h.gen += 1
        }
    });
    // ... the empty slot's sentinel: dropping a contact must move it too.
    let keep = s.units[a.index as usize].ai.contacts[0];
    s.units[a.index as usize].ai.contacts[0] = None;
    assert_ne!(s.hash(), with_contact, "an empty slot is state");
    s.units[a.index as usize].ai.contacts[0] = keep;
    // ... and the two `Unit` fields the hearing rule writes.
    s.units[a.index as usize].last_shot_tick = 3;
    assert_ne!(s.hash(), with_contact, "last_shot_tick is state");
    s.units[a.index as usize].last_shot_tick = u64::MAX;
    s.units[a.index as usize].last_hit = Some((e, 1));
    assert_ne!(s.hash(), with_contact, "last_hit is state");
    s.units[a.index as usize].last_hit = None;
    assert_eq!(s.hash(), with_contact, "both restored");
    // ... and the target / last_switch pair.
    s.units[a.index as usize].ai.target = Some(e.into());
    let with_target = s.hash();
    assert_ne!(with_target, with_contact, "the target is state");
    s.units[a.index as usize].ai.last_switch = 7;
    assert_ne!(s.hash(), with_target, "last_switch is state");
}
#[test]
fn audit_whole_brain_preferences_respect_spawn_grace() {
    for tier in [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Elite,
    ] {
        for proxy in [None, Some(crate::ai::proxy::Strength::Good.profile())] {
            let mut sim = Sim::empty(31);
            sim.heightfield = Some(crate::heightfield::Heightfield {
                size: 2,
                unit: 4000.0,
                offset_x: 2000.0,
                offset_z: 2000.0,
                heights: vec![0.0; 4],
            });
            sim.team_difficulty[0] = Some(tier);
            let bot = sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(0, Vec3::ZERO)
                })
                .unwrap();
            sim.unit_mut(bot).unwrap().proxy_human = proxy;
            let human = sim
                .spawn(&UnitSpec::player_in_slot(0, 1, Vec3::new(0.0, 0.0, -15.0)))
                .unwrap();
            crate::ai::dfocus::upkeep(&mut sim);
            for tick in [
                0,
                crate::ai::dfocus::UNTARGETED - 1,
                crate::ai::dfocus::UNTARGETED,
            ] {
                sim.tick = tick;
                let mut st = AiState::default();
                crate::ai::think(&sim, bot, &mut st);
                assert_eq!(
                    st.target,
                    (tick == crate::ai::dfocus::UNTARGETED).then_some(human.into()),
                    "tier={tier:?} proxy={proxy:?} tick={tick}"
                );
            }
        }
    }
}

/// Playtest C1: the search's length, by distance and by whether the bot
/// already held a record of the shooter.
#[test]
fn the_hit_search_grows_with_distance_and_shrinks_for_a_known_shooter() {
    assert_eq!(search_ticks(20.0, false), 21);
    assert_eq!(search_ticks(40.0, false), 33);
    assert_eq!(search_ticks(60.0, false), 45);
    assert_eq!(search_ticks(200.0, false), SEARCH_CAP);
    const {
        assert!(
            SEARCH_CAP + 9 < MEMORY_TICKS,
            "the record outlives its search"
        )
    };
    // Hit from `dist` m behind (outside the cone and BF2's bubble);
    // `known`: a shot of his was heard at the think before.
    let hit = |dist: f32, known: bool| {
        let (mut s, a) = watcher(41);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, dist));
        let mut st = AiState::default();
        if known {
            hear(&mut st, Foe::Unit(e), 0, Vec3::new(0.0, 0.0, dist), false);
        }
        s.units[a.index as usize].last_hit = Some((e, 4));
        s.tick = 5;
        scan(&s, a, &mut st, normal());
        let eye = s.unit(a).unwrap().ai_eye();
        let d = (Vec3::new(0.0, 0.0, dist) - eye).length();
        let c = contact_of(&st, e).expect("the shooter is held");
        assert!(c.heard && !c.seen_now);
        assert_eq!(
            c.first_seen,
            5 + search_ticks(d, known),
            "{dist} m known={known}"
        );
        assert!(!fireable(&c, 5 + search_ticks(d, known) + 8, 9));
        c.first_seen
    };
    assert!(hit(15.0, false) < hit(45.0, false), "farther is longer");
    assert!(
        hit(45.0, true) < hit(45.0, false),
        "a known shooter is quicker"
    );
}

/// Playtest C1: a shooter the bot sees as it is hit, or saw with its own
/// eyes at its last think, costs no search; nor does another hit lengthen
/// a search under way.
#[test]
fn a_hit_from_a_shooter_in_sight_costs_no_search() {
    // In front, in sight.
    let (mut s, a) = watcher(43);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    s.units[a.index as usize].last_hit = Some((e, 0));
    s.tick = 1;
    let mut st = AiState::default();
    scan(&s, a, &mut st, normal());
    let c = contact_of(&st, e).unwrap();
    assert!(c.seen_now);
    assert_eq!(c.first_seen, 1);
    // Seen at tick 1, behind a wall by tick 6 and hitting the bot.
    s.collision.push_body(Body::new(
        Mat4::from_translation(Vec3::new(0.0, 5.0, -10.0)),
        Shape::Box {
            half: Vec3::new(5.0, 5.0, 0.5),
        },
    ));
    s.units[a.index as usize].last_hit = Some((e, 5));
    s.tick = 6;
    scan(&s, a, &mut st, normal());
    let c = contact_of(&st, e).unwrap();
    assert!(c.heard && !c.seen_now);
    assert_eq!(c.first_seen, 1, "seen at the last think: no search");

    // Behind: the first hit starts a search, a second does not move it.
    let (mut s, a) = watcher(47);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 40.0));
    let mut st = AiState::default();
    s.units[a.index as usize].last_hit = Some((e, 0));
    s.tick = 1;
    scan(&s, a, &mut st, normal());
    let pending = contact_of(&st, e).unwrap().first_seen;
    assert!(pending > 1 + 20);
    s.units[a.index as usize].last_hit = Some((e, 10));
    s.tick = 11;
    scan(&s, a, &mut st, normal());
    assert_eq!(contact_of(&st, e).unwrap().first_seen, pending);
}

/// Playtest C1: space maps and proxies keep the hit rule they had - the
/// reaction counts from the hearing, and there is no flinch.
#[test]
fn space_maps_and_proxies_keep_the_old_hit_rule() {
    let cases: [fn(&mut Sim, UnitHandle); 3] = [
        |s, _| {
            s.landing_regions.push(
                crate::region::Region::from_rows(
                    "box",
                    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    [0.0, 500.0, 0.0],
                    [1.0, 1.0, 1.0],
                )
                .unwrap(),
            )
        },
        |s, _| s.space_assault = Some(crate::sim::SpaceAssault::new(10)),
        |s, a| {
            s.units[a.index as usize].proxy_human =
                Some(crate::ai::proxy::Strength::Average.profile())
        },
    ];
    for (i, setup) in cases.iter().enumerate() {
        let (mut s, a) = watcher(53);
        let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, 40.0));
        setup(&mut s, a);
        s.units[a.index as usize].last_hit = Some((e, 0));
        s.tick = 1;
        let mut st = AiState::default();
        think(&s, a, &mut st);
        let c = contact_of(&st, e).expect("the shooter is held");
        assert_eq!(c.first_seen, 1, "case {i}: no search");
        assert_eq!(st.flinch_until, 0, "case {i}: no flinch");
    }
}

/// Playtest C1: a hit makes a bot on foot hold a shot it would have fired
/// for FLINCH_TICKS, at most once per FLINCH_GAP, and hits every tick do
/// not stop it shooting back.
#[test]
fn a_hit_flinch_holds_fire_briefly_and_not_too_often() {
    use crate::ai::{FLINCH_GAP, FLINCH_TICKS};
    let (mut s, a) = watcher(59);
    let e = enemy_at(&mut s, Vec3::new(0.0, 0.0, -20.0));
    let mut st = AiState::default();
    let fire = |s: &mut Sim, st: &mut AiState, t: u64, hit: Option<u64>| {
        s.tick = t;
        s.units[a.index as usize].last_hit = hit.map(|h| (e, h));
        think(s, a, st).0.buttons & buttons::FIRE != 0
    };
    // Find a tick where the unhit bot fires.
    let mut t = 1;
    loop {
        let mut probe = st;
        if fire(&mut s, &mut probe, t, None) {
            break;
        }
        fire(&mut s, &mut st, t, None);
        t += 1;
        assert!(t < 120, "the bot never fired");
    }
    // Hit on the tick before it: that shot and the flinch's are held.
    let mut hit = st;
    for k in 0..FLINCH_TICKS {
        assert!(
            !fire(&mut s, &mut hit, t + k, Some(t - 1)),
            "tick {k} of the flinch"
        );
    }
    assert_eq!(hit.flinch_until, t + FLINCH_TICKS);
    // Another hit inside the gap is no flinch.
    fire(
        &mut s,
        &mut hit,
        t + FLINCH_TICKS,
        Some(t + FLINCH_TICKS - 1),
    );
    assert_eq!(hit.flinch_until, t + FLINCH_TICKS, "inside the gap");
    // Hit every tick: it still shoots back.
    let mut shots = 0;
    for k in FLINCH_TICKS + 1..FLINCH_TICKS + 1 + 4 * FLINCH_GAP {
        shots += u32::from(fire(&mut s, &mut hit, t + k, Some(t + k - 1)));
    }
    assert!(shots > 0, "steady fire never locks it out");
    assert!(
        hit.flinch_until > t + FLINCH_TICKS,
        "it flinched again after the gap"
    );
}

/// Playtest C1, the whole bot: hit from 40 m off its side, just out of
/// its view cone, it turns, finds the shooter and fires no sooner than the
/// search and its reaction allow - under the old rule (a space map's) the
/// turn alone held it. From straight behind the 180 degree turn (2 rad/s)
/// takes about as long as the search, so the change is mostly at the side.
#[test]
fn a_far_hit_from_an_unseen_shooter_waits_out_the_search() {
    let first_shot = |at: Vec3, space: bool, every_tick: bool| {
        let (mut s, a) = watcher(61);
        let e = enemy_at(&mut s, at);
        if space {
            s.space_assault = Some(crate::sim::SpaceAssault::new(10));
        }
        for _ in 0..5 {
            s.step(&[]);
        }
        let hit = s.tick - 1;
        s.units[a.index as usize].last_hit = Some((e, hit));
        let before = s.unit(a).unwrap().last_shot_tick;
        for _ in 0..200 {
            if every_tick {
                let t = s.tick - 1;
                s.units[a.index as usize].last_hit = Some((e, t));
            }
            s.step(&[]);
            s.unit_mut(e).unwrap().health.cur = 300.0;
            let shot = s.unit(a).unwrap().last_shot_tick;
            if shot != before {
                return shot - hit;
            }
        }
        panic!("{at} space={space} every_tick={every_tick}: never fired");
    };
    let r = normal().reaction_ticks;
    let side = Vec3::new(40.0, 0.0, 0.0);
    let search = search_ticks(39.0, false);
    let (old, new) = (
        first_shot(side, true, false),
        first_shot(side, false, false),
    );
    assert!(
        old < search + r,
        "the old rule answers after the turn ({old} ticks)"
    );
    assert!(new >= search + r, "{new} ticks");
    assert!(new <= SEARCH_CAP + r + 30, "{new} ticks");
    // Hit every tick: the search is not lengthened, and it still fires.
    let sustained = first_shot(side, false, true);
    assert!(sustained <= SEARCH_CAP + r + 60, "{sustained} ticks");
    // Behind: the turn is the longer part either way.
    let behind = Vec3::new(0.0, 0.0, 40.0);
    let (old_b, new_b) = (
        first_shot(behind, true, false),
        first_shot(behind, false, false),
    );
    assert!(
        new_b >= old_b && new_b <= SEARCH_CAP + r + 60,
        "{old_b} -> {new_b}"
    );
    eprintln!("C1 side {old} -> {new} (sustained {sustained}), behind {old_b} -> {new_b}");
}
