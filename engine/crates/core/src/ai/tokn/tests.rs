use super::*;
use crate::sim::{Event, PadState};
use crate::unit::UnitSpec;

/// A flat field: a human of team 0 at the origin and enemy bots (team 1)
/// at `dists` metres north of him, each with him as its target.
fn field(d: Difficulty, dists: &[f32]) -> (Sim, UnitHandle, Vec<UnitHandle>) {
    let (mut sim, _) = crate::ai::tests::l_sim();
    sim.units.clear();
    sim.difficulty = d;
    let human = sim.spawn(&UnitSpec::player(0, Vec3::ZERO)).expect("human");
    let bots = dists
        .iter()
        .map(|&z| {
            let h = sim
                .spawn(&UnitSpec {
                    controller: Controller::Ai,
                    ..UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -z))
                })
                .expect("bot");
            sim.unit_mut(h).expect("alive").ai.target = Some(human.into());
            h
        })
        .collect();
    (sim, human, bots)
}

fn holders(sim: &Sim) -> Vec<UnitHandle> {
    let mut v: Vec<UnitHandle> = sim.tokens.iter().map(|t| t.bot).collect();
    v.sort_by_key(|h| h.index);
    v
}

/// The user's table: Easy 2, Normal 3, Hard 5, Elite 7; each tier at
/// least as many tokens, no longer a hold and no wider a miss than the one
/// below it, and Elite's strictly shorter and narrower than Hard's.
#[test]
fn the_table_is_the_users_and_each_tier_is_harder() {
    let n: Vec<Option<u8>> = Difficulty::ALL.iter().map(|&d| budget(d).tokens).collect();
    assert_eq!(n, vec![Some(2), Some(3), Some(5), Some(7)]);
    let (h, e) = (budget(Difficulty::Hard), budget(Difficulty::Elite));
    assert!(e.hold < h.hold && e.miss < h.miss && e.hold > 0 && e.miss > 0.0);
    for w in BUDGETS.windows(2) {
        let (a, b) = (w[0].1, w[1].1);
        if let Some(nb) = b.tokens {
            assert!(nb >= a.tokens.unwrap_or(u8::MAX));
            assert!(b.hold <= a.hold && b.miss <= a.miss);
        }
    }
}

/// Normal: the three best placed of four shooters get the tokens; Elite's
/// seven cover all four, and so bar nobody.
#[test]
fn the_best_placed_shooters_get_the_tokens() {
    let (mut sim, human, bots) = field(Difficulty::Normal, &[40.0, 10.0, 20.0, 30.0]);
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![bots[1], bots[2], bots[3]]);
    assert!(bars(&sim, bots[0], human));
    assert!(!bars(&sim, bots[1], human));
    let (mut sim, human, bots) = field(Difficulty::Elite, &[40.0, 10.0, 20.0, 30.0]);
    deal(&mut sim);
    assert_eq!(holders(&sim), bots);
    assert!(bots.iter().all(|&b| !bars(&sim, b, human)));
}

/// A held token goes, at the end of its hold, to the best-placed bot
/// waiting; with nobody waiting the holder keeps it; a holder that turns to
/// another target gives it up at once.
#[test]
fn tokens_rotate_after_the_hold() {
    let (mut sim, human, bots) = field(Difficulty::Easy, &[10.0, 20.0, 30.0]);
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![bots[0], bots[1]]);
    let hold = budget(Difficulty::Easy).hold;
    sim.tick += hold - 1;
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![bots[0], bots[1]], "held for the hold");
    sim.tick += 1;
    deal(&mut sim);
    let h = holders(&sim);
    assert!(
        h.contains(&bots[2]) && h.len() == 2,
        "then to the one waiting: {h:?}"
    );
    // It turns away: the first two take theirs back, and alone keep them.
    sim.unit_mut(bots[2]).expect("alive").ai.target = None;
    deal(&mut sim);
    assert_eq!(holders(&sim), vec![bots[0], bots[1]]);
    sim.tick += 3 * hold;
    deal(&mut sim);
    assert_eq!(
        holders(&sim),
        vec![bots[0], bots[1]],
        "nobody waiting: kept"
    );
    assert!(sim.tokens.iter().all(|t| t.human == human));
}

/// Bot-vs-bot fire and a friendly bot are never governed.
#[test]
fn only_an_enemy_bots_fire_at_a_human_is_governed() {
    let (mut sim, human, bots) = field(Difficulty::Easy, &[10.0, 20.0, 30.0]);
    let friend = sim
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::new(5.0, 0.0, 0.0))
        })
        .expect("friend");
    deal(&mut sim);
    assert!(!bars(&sim, bots[2], friend), "bot on bot");
    assert!(!bars(&sim, friend, human), "a team mate");
    assert!(bars(&sim, bots[2], human));
}

/// A bot without a token aims beside him by its tier's miss, a little high.
#[test]
fn the_near_miss_is_the_tiers_width() {
    let (mut sim, human, bots) = field(Difficulty::Easy, &[10.0, 20.0, 30.0]);
    deal(&mut sim);
    let eye = Vec3::new(0.0, 1.5, -30.0);
    let off = miss_offset(&sim, bots[2], human, eye, Vec3::Y).expect("barred");
    assert!(
        (off.x.abs() - budget(Difficulty::Easy).miss).abs() < 1e-4,
        "{off}"
    );
    assert!(off.y > 0.0 && off.z.abs() < 1e-4, "{off}");
    assert_eq!(miss_offset(&sim, bots[0], human, eye, Vec3::Y), None);
}

/// In play, at Easy, against a human who stands and never fires back: the
/// token holders do the hurting; the others fire too, beside him, and what
/// of theirs meets him anyway (a stray) is real but rare.
#[test]
fn in_play_the_token_holders_do_the_hurting() {
    let (mut sim, human, _) = field(Difficulty::Easy, &[15.0, 18.0, 21.0, 24.0, 27.0]);
    let (mut hits, mut strays) = (0, 0);
    for _ in 0..900 {
        sim.step(&[PadState::default()]);
        for e in &sim.events {
            if let Event::Hit {
                shooter,
                victim,
                damage,
                ..
            } = *e
            {
                if victim == human && damage > 0.0 {
                    if holds(&sim, shooter, human) {
                        hits += 1;
                    } else {
                        strays += 1;
                    }
                }
            }
        }
        if let Some(u) = sim.unit_mut(human) {
            u.health.cur = u.health.max;
        }
    }
    assert!(hits > 0, "the holders hit");
    assert!(strays * 5 <= hits, "strays {strays} of holders' {hits}");
    assert!(
        sim.nav_stats.tokn_aimed_off > 0,
        "the others fire beside him"
    );
}

/// A bolt from `owner` 2 m in front of the human's chest, flying at him.
fn shot(owner: UnitHandle, hit: crate::projectile::Hit) -> crate::projectile::Bolt {
    let pos = Vec3::new(0.0, 1.2, -2.0);
    let vel = Vec3::new(0.0, 0.0, 300.0);
    crate::projectile::Bolt {
        tracked_origin: false,
        visual_shot: None,
        pos,
        vel,
        remaining: if hit == crate::projectile::Hit::None {
            1.0
        } else {
            0.001
        },
        age: 0.05,
        owner,
        team: 1,
        weapon_class: 0,
        hit,
        end: pos + vel,
        shooter_is_player: false,
        hulls: crate::collision::HullFilter::All,
        hit_mult: 1.0,
        gravity: 0.0,
        charge: 1.0,
        missile: None,
        fat: None,
    }
}

/// No fire passes through a soldier: a bolt of a bot without a token that
/// meets the human anyway (the sweep's or the spawn ray's) hurts him like
/// the holder's, and is counted as a stray.
#[test]
fn a_stray_bolt_without_a_token_still_hurts() {
    use crate::projectile::{Hit, Target};
    for (who, preset) in [(2, false), (2, true), (0, false)] {
        let (mut sim, human, bots) = field(Difficulty::Easy, &[10.0, 20.0, 30.0]);
        deal(&mut sim);
        let hit = if preset {
            Hit::Target(Target::Unit(human))
        } else {
            Hit::None
        };
        sim.bolts.push(shot(bots[who], hit));
        let full = sim.unit(human).expect("alive").health.cur;
        sim.step(&[PadState::default()]);
        let took = full - sim.unit(human).expect("alive").health.cur;
        assert!(took > 0.0, "{who} {preset:?}");
        assert_eq!(sim.nav_stats.tokn_stray, u64::from(who == 2));
    }
}

/// A human in a vehicle gets token holders at every tier: the line to him
/// passes his own hull. At Easy a bot with a launcher shooting at his tank
/// holds one of his tokens and is not made to miss.
#[test]
fn a_human_in_a_vehicle_gets_token_holders() {
    use crate::ai::arsenal::tests::{heavy_scene, launcher, manned, rifle};
    use crate::collision::{HullPart, Layers, Shape};
    use crate::vehicle::VehicleKind;
    let (mut sim, bot, _) = heavy_scene(launcher(), rifle());
    sim.difficulty = Difficulty::Easy;
    // A hull round the pilot's seat: a 4 x 4 x 6 m box on the vehicle.
    sim.hull_parts = vec![Some(std::sync::Arc::from(vec![HullPart::new(
        glam::Mat4::IDENTITY,
        Shape::Box {
            half: Vec3::new(2.0, 2.0, 3.0),
        },
        Layers::ALL,
    )]))];
    let (_, rider) = manned(&mut sim, VehicleKind::Hover, Vec3::new(0.0, 1.5, -40.0));
    sim.sync_hulls();
    sim.units[rider.index as usize].is_player = true;
    let (eye, c) = (
        sim.unit(bot).expect("alive").muzzle(),
        sim.unit(rider).expect("alive").capsule().center(),
    );
    assert!(
        !crate::perception::los(&sim, eye, c),
        "the hull is in the way"
    );
    sim.unit_mut(bot).expect("alive").ai.target = Some(rider.into());
    deal(&mut sim);
    assert!(holds(&sim, bot, rider));
    assert!(!bars(&sim, bot, rider));
}
