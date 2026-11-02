use super::*;
use crate::ai::think;

mod radius_rows;

/// Card CLSE2: the radius formula against the PS2's own RAM - 80 rows
/// at rest (69 standing, 11 crouched), each within 1e-4 m.
#[test]
fn the_radius_reproduces_the_ps2s_ram_rows() {
    let (mut crouched, mut capped) = (0, 0);
    for &(size, word, err, vis, cap, ps2) in &radius_rows::ROWS {
        let stance = match word {
            0 => Stance::Stand,
            1 => Stance::Crouch,
            w => panic!("stance word {w}"),
        };
        crouched += usize::from(word == 1);
        capped += usize::from(err * stance_factor(stance) > cap);
        let r = radius(2.0 * size, stance_factor(stance), err, 0.0, vis, cap);
        assert!(
            (r - ps2).abs() < 1e-4,
            "size {size} stance {word} err {err} vis {vis}: {r} against the PS2's {ps2}"
        );
    }
    assert_eq!((radius_rows::ROWS.len(), crouched), (80, 11));
    assert!(capped > 0, "the cap is exercised");
    // The arms no row at rest shows (the ELF's jump table, 18.2), and the
    // speed term on top.
    assert_eq!(stance_factor(Stance::Sprint), 2.0);
    assert_eq!(stance_factor(Stance::Jump), 2.0);
    assert_eq!(stance_factor(Stance::Prone), 2.0);
    assert_eq!(stance_factor(Stance::Roll), 3.0);
    assert!((radius(1.0, 1.0, 0.5, 1.0, 1.0, 9.0) - 0.8).abs() < 1e-6);
}

/// Two still bots 20 m apart in a round of `rules`, the shooter's
/// direction set to +X and the target's error to 0.8 m.
fn aim_pair(rules: crate::soldier::GroundRules) -> (Sim, UnitHandle, UnitHandle) {
    use crate::unit::{Controller, UnitSpec};
    let mut s = Sim::empty(5);
    s.ground_rules = rules;
    let mut bot = |team: u8, z: f32| {
        s.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, Vec3::new(0.0, 0.0, z))
        })
        .unwrap()
    };
    let (a, t) = (bot(0, 0.0), bot(1, -20.0));
    s.units[a.index as usize].aim.dir = Vec3::X;
    s.units[t.index as usize].aim.err = 0.8;
    (s, a, t)
}

/// The aim point's offset from the target's centre, for `aim_pair`, over
/// the bot-against-bot and range factors (0.3 x (d / 20 + 0.5)): the
/// radius along the shooter's direction.
fn aim_offset(s: &Sim, a: UnitHandle, t: UnitHandle) -> Vec3 {
    let e = s.unit(t).unwrap();
    let eye = s.unit(a).unwrap().muzzle();
    let c = e.capsule().center();
    let st = AiState::default();
    let off = seen_aim(s, a, &st, eye, e, 1e6, &WeaponClass::default()) - c;
    off / (ERR_BOT_SCALE * ((c - eye).length() / 20.0 + 0.5))
}

/// Card CLSE2: a BF2 bot's aim point is off by the TARGET's own error,
/// along the SHOOTER's own direction, times the target's model size and
/// stance - and the shooter's own block's error is nothing to it.
#[test]
fn a_bf2_bot_aims_off_by_its_targets_own_error() {
    let (mut s, a, t) = aim_pair(crate::soldier::GroundRules::Bf2);
    let (ai, ti) = (a.index as usize, t.index as usize);
    let base = aim_offset(&s, a, t);
    assert!((base - Vec3::X * 0.8).length() < 1e-5, "{base}");
    // The shooter's own error is not asked.
    s.units[ai].aim.err = 2.0;
    assert_eq!(aim_offset(&s, a, t), base);
    // The target's is.
    s.units[ti].aim.err = 0.4;
    assert!((aim_offset(&s, a, t) - base * 0.5).length() < 1e-5);
    s.units[ti].aim.err = 0.8;
    // Its model: the super battle droid's 0.48.
    s.units[ti].consts.aim_size = 0.48;
    assert!((aim_offset(&s, a, t) - base * 0.48).length() < 1e-5);
    s.units[ti].consts.aim_size = 1.0;
    // Its stance, inside the cap (1.36 m at skill 17): crouched 1.5.
    s.units[ti].soldier.stance = Stance::Crouch;
    assert!((aim_offset(&s, a, t) - base * 1.5).length() < 1e-5);
    s.units[ti].soldier.stance = Stance::Roll;
    let cap = crate::skill::Skill::from_level(17).err_cap;
    assert!((aim_offset(&s, a, t) - Vec3::X * cap).length() < 1e-5);
    s.units[ti].soldier.stance = Stance::Stand;
    // The shooter's direction: turned, the offset turns with it; and it
    // never points under the aim point.
    s.units[ai].aim.dir = Vec3::new(0.0, -0.6, 0.8);
    let turned = aim_offset(&s, a, t);
    assert!(
        (turned - Vec3::new(0.0, 0.0, 0.8) * 0.8).length() < 1e-5,
        "{turned}"
    );
}

/// A BF1 round keeps D10's model (PROVISIONAL, card CLSE2B): the offset
/// is the SHOOTER's tracked error, whole - no factor by target or range,
/// nothing of the blocks - and the sim thinks no block.
#[test]
fn a_bf1_bot_keeps_its_own_tracked_error() {
    let (mut s, a, t) = aim_pair(crate::soldier::GroundRules::Bf1);
    let skill = crate::skill::Skill::from_level(17);
    let off = |s: &Sim, err: f32| {
        let st = AiState {
            err,
            ..AiState::default()
        };
        let e = s.unit(t).unwrap();
        let eye = s.unit(a).unwrap().muzzle();
        let wc = WeaponClass::default();
        seen_aim(s, a, &st, eye, e, 1e6, &wc) - e.capsule().center()
    };
    let dir = bf1_error_dir(&s, a);
    assert!((off(&s, 0.7) - dir * 0.7).length() < 1e-4);
    assert!(
        (off(&s, 9.0) - dir * skill.err_cap).length() < 1e-4,
        "capped"
    );
    s.units[t.index as usize].aim.err = 2.0;
    s.units[t.index as usize].consts.aim_size = 0.48;
    s.units[t.index as usize].is_player = true;
    assert!(
        (off(&s, 0.7) - dir * 0.7).length() < 1e-4,
        "nothing of BF2's"
    );
    let before = s.unit(a).unwrap().aim;
    for _ in 0..10 {
        s.step(&[]);
    }
    assert_eq!(s.unit(a).unwrap().aim, before);
}

/// A BF1 round (D10): the shooter's tracked error DECAYS toward the skill's floor on a
/// still target and GROWS when the target moves, and it is clamped into
/// `[floor, 2.0]` with a target and `[1.5, 2.0]` without one (LEAD
/// ADDENDUM 4a).
#[test]
fn the_tracked_error_decays_to_the_floor_and_grows_with_movement() {
    use crate::skill::{Difficulty, Skill};
    use crate::unit::{Controller, UnitSpec};
    // A bot's skill is the TIER's (`core::skill_for`), so the level is
    // chosen by the difficulty: Easy 14, Normal 17.
    let build = |d: Difficulty| {
        let mut s = crate::sim::Sim::empty(5);
        s.ground_rules = crate::soldier::GroundRules::Bf1;
        s.difficulty = d;
        s.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        });
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -20.0)))
            .unwrap();
        let level = s.unit(a).unwrap().skill;
        assert_eq!(level, d.base_skill());
        (s, a, t, level)
    };
    // A still target, five seconds: the error decays to the floor.
    for d in [Difficulty::Easy, Difficulty::Normal] {
        let (s, a, _, level) = build(d);
        let mut st = AiState::default();
        assert_eq!(st.err, ERR_NO_TARGET, "starts at the no-target floor");
        let mut sim = s;
        for t in 0..150u64 {
            sim.tick = t;
            think(&sim, a, &mut st);
        }
        let floor = Skill::from_level(level).err_floor;
        assert!(
            (st.err - floor).abs() < 1e-3,
            "skill {level} decayed to {} not {floor}",
            st.err
        );
    }
    // The floors themselves, as ADDENDUM 4a rules them: 1.158 m at
    // skill 8, 0.589 m at 17 (the brief's "1 m / 0" text is superseded).
    assert!((Skill::from_level(8).err_floor - 1.157_894_7).abs() < 1e-6);
    assert!((Skill::from_level(17).err_floor - 0.589_473_6).abs() < 1e-6);

    // A target moving at full speed GROWS the error above the floor.
    let (mut sim, a, t, _) = build(Difficulty::Normal);
    let mut st = AiState::default();
    let speed = sim.unit(t).unwrap().consts.max_speed;
    for k in 0..30u64 {
        sim.tick = k * THINK_PERIOD;
        // Walk the target sideways at its own top speed between thinks.
        let step = speed * THINK_PERIOD as f32 / crate::TICK_HZ as f32;
        let u = sim.unit_mut(t).unwrap();
        u.soldier.pos.x += step;
        think(&sim, a, &mut st);
    }
    let floor = Skill::from_level(17).err_floor;
    assert!(
        st.err > floor + 0.05,
        "moving target grew the error: {}",
        st.err
    );
    assert!(st.err <= ERR_MAX);

    // With no target at all the error is held at the PS2's 1.5 m no-target
    // floor, never reset and never below it.
    let (mut sim, a, t, _) = build(Difficulty::Normal);
    sim.units[t.index as usize].alive = false;
    let mut st = AiState {
        err: 0.2,
        ..Default::default()
    };
    think(&sim, a, &mut st);
    assert_eq!(st.err, ERR_NO_TARGET);
}

/// A BF1 round (D10): the aim point is offset by at most the effective radius, in a
/// direction that is constant within a think and different across thinks.
#[test]
fn the_error_offset_is_bounded_and_re_drawn_once_per_think() {
    let r = |tick: u64| unit_dir(3, tick / THINK_PERIOD, 1, SALT_ERR_DIR);
    assert_eq!(r(0), r(THINK_PERIOD - 1), "constant within a think");
    assert_ne!(r(0), r(THINK_PERIOD), "re-drawn next think");
    assert_ne!(r(0), unit_dir(4, 0, 1, SALT_ERR_DIR), "per unit");
    for t in 0..40u64 {
        assert!((r(t).length() - 1.0).abs() < 1e-5, "unit vector");
    }
    // The Elite tier's cap overrides the level's, so the offset a
    // skill-20 Elite bot can carry is 0.5 m and a Hard one 1.2 m.
    use crate::skill::Difficulty;
    assert_eq!(Difficulty::Elite.skill_at(20).err_cap, 0.5);
    assert!((Difficulty::Hard.skill_at(20).err_cap - 1.2).abs() < 1e-6);
}

/// D9: the aim tracker steers the aim direction instead of snapping it,
/// slews when the target is more than 30° off, and is idle with no target.
#[test]
fn the_aim_tracker_steers_and_slews() {
    use crate::unit::{Controller, UnitSpec};
    let build = |enemy_at: Option<Vec3>| {
        let mut s = crate::sim::Sim::empty(5);
        s.heightfield = Some(crate::heightfield::Heightfield {
            size: 2,
            unit: 4000.0,
            offset_x: 2000.0,
            offset_z: 2000.0,
            heights: vec![0.0; 4],
        });
        let a = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        if let Some(p) = enemy_at {
            s.spawn(&UnitSpec::trooper(1, p)).unwrap();
        }
        (s, a)
    };
    // A target straight ahead: the tracker starts at the unit's facing and
    // stays a unit vector while it closes on the aim point.
    let (sim, a) = build(Some(Vec3::new(0.0, 0.0, -20.0)));
    let mut st = AiState::default();
    think(&sim, a, &mut st);
    assert!((st.aim_dir.length() - 1.0).abs() < 1e-5, "unit vector");
    let first = st.aim_dir;
    let mut sim = sim;
    for t in 1..20u64 {
        sim.tick = t;
        think(&sim, a, &mut st);
    }
    assert!((st.aim_dir.length() - 1.0).abs() < 1e-5);
    assert_ne!(st.aim_dir, first, "the tracker moved");

    // A target 90° off the facing: the first tick SLEWS 30° toward it and
    // zeroes the rate, then takes the one tracker step of the same call
    // (at most `aim_accel`·dt² further, under 1°).
    // 60° off the facing — inside the 77.5° cone, so it IS perceived,
    // and more than the tracker's 30° slew step.
    let (sim, a) = build(Some(Vec3::new(-17.32, 0.0, -10.0)));
    let mut st = AiState::default();
    assert!(sim.unit(a).is_some());
    let facing = sim.unit(a).unwrap().aim_frame().0;
    think(&sim, a, &mut st);
    assert!(st.target.is_some(), "60 deg off is inside the cone");
    let deg = st.aim_dir.dot(facing).clamp(-1.0, 1.0).acos().to_degrees();
    assert!(
        (30.0..31.0).contains(&deg),
        "slewed 30 deg plus one step: {deg}"
    );
    let dt = crate::TICK_DT;
    assert!(
        st.aim_vel.length() <= 14.0 * dt + 1e-4,
        "the slew zeroed the rate before the step: {}",
        st.aim_vel.length()
    );

    // With NO target the tracker is idle and the travel facing is
    // untouched.
    let (sim, a) = build(None);
    let mut st = AiState::default();
    think(&sim, a, &mut st);
    assert_eq!(st.aim_dir, Vec3::ZERO);
    assert_eq!(st.aim_vel, Vec3::ZERO);
}

/// D9's tracker law is the PS2's (B2 `FUN_002a83e0`, B1 `FUN_00236700`):
/// the "reach it next tick" acceleration is clamped and the damping is
/// subtracted after the clamp. The port's earlier law clamped the sum
/// `(dir − ad)/dt − av·damping`; it lagged a turning target by about 1.1°
/// at skill 20 and took over 2 s to settle a 10° step. The PS2's lags under
/// 0.1° and settles the step inside 0.5° in 20 ticks at skill 17 and up.
#[test]
fn the_aim_tracker_clamps_the_reach_then_damps() {
    let dt = crate::TICK_DT;
    let at = |ang: f32| Vec3::new(-ang.sin(), 0.0, -ang.cos());
    let angle = |a: Vec3, b: Vec3| a.dot(b).clamp(-1.0, 1.0).acos().to_degrees();
    for level in [17u8, 20] {
        let k = crate::skill::Skill::from_level(level);
        let (accel, damping) = k.tracker_for(true);
        // A target swinging round at 0.4 rad/s: the lag after 5 s.
        let (mut ad, mut av) = (at(0.0), Vec3::ZERO);
        let mut lag = 0.0f32;
        for t in 0..300 {
            let want = at(0.4 * t as f32 * dt);
            (ad, av) = track_step(ad, av, want, accel, damping, dt);
            if t >= 150 {
                lag = lag.max(angle(ad, want));
            }
        }
        assert!(
            lag < 0.1,
            "skill {level}: lag {lag} deg on a turning target"
        );
        // A 10° step: inside 0.5° from tick 20 on (the old law took 73).
        let want = at(10f32.to_radians());
        let (mut ad, mut av) = (at(0.0), Vec3::ZERO);
        for t in 0..90 {
            (ad, av) = track_step(ad, av, want, accel, damping, dt);
            if t >= 20 {
                let off = angle(ad, want);
                assert!(off < 0.5, "skill {level}: {off} deg off at tick {t}");
            }
        }
    }
    // The damping is outside the clamp: far from the target at speed, the
    // step is the clamped reach minus `av·damping`.
    let (accel, damping) = (2.0, 1.5);
    let av = Vec3::new(-1.0, 0.0, 0.0);
    let (_, next) = track_step(at(0.0), av, at(0.4), accel, damping, dt);
    let reach = ((at(0.4) - at(0.0)) / dt - av) / dt;
    let want = av + (reach.normalize() * accel - av * damping) * dt;
    assert!((next - want).length() < 1e-5, "{next} vs {want}");
}

/// Accuracy against range under the Phase C aim model (D9/D10): the
/// tracked-position error is a distance at the target and the weapon's
/// dispersion is an angle, so the hit rate falls with range and never
/// grows.
#[test]
fn aim_error_scales_misses_with_range() {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let rate_of = |dist: f32, human: bool| {
        let mut s = Sim::empty(5);
        s.cheats.infinite_ammo = true;
        let shooter = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        // Immortal dummy target (health topped up every tick).
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -dist)))
            .unwrap();
        // Card CLSE: the error is whole against a human's unit.
        s.units[t.index as usize].is_player = human;
        let (mut shots, mut hits) = (0, 0);
        // (1800 ticks: card CLSE4's pause is 1.7 s of every burst.)
        for _ in 0..1800 {
            s.step(&[PadState::default()]);
            s.unit_mut(t).unwrap().health.cur = 300.0;
            shots += s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
                .count();
            hits += s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Hit { .. }))
                .count();
        }
        assert!(shots > 20, "AI fired {shots} shots at {dist} m");
        hits as f32 / shots as f32
    };
    // Seed 5, Normal (skill 17), a still dummy and 300 ticks per range.
    // D10's positional aim error and D11's burst pauses interact with
    // weapon spread; the small shot sample needs deliberately wide bands.
    let rate = |dist: f32| rate_of(dist, true);
    let rates: Vec<f32> = [4.0, 8.0, 16.0, 24.0, 40.0]
        .iter()
        .map(|&d| rate(d))
        .collect();
    let bots: Vec<f32> = [4.0, 8.0, 16.0, 24.0, 40.0]
        .iter()
        .map(|&d| rate_of(d, false))
        .collect();
    // Allow about one shot's variation between adjacent range samples.
    for w in rates.windows(2) {
        assert!(
            w[1] <= w[0] + 0.04,
            "hit rate must not grow with range: {rates:?}"
        );
    }
    assert!(rates[4] < rates[0], "and it falls over the whole span");
    // SPRD (OURS): AI bursts stay tight (`TIGHT_HOLD_SECS_AI`). CLSE:
    // BF2 scales error by d / 20 + 0.5 against humans, and by another
    // 0.3 against bots (`analysis/re-soldier-ai.md` 17). These assertions
    // protect close/far accuracy and the human/bot distinction. The TPOS
    // shooter stands still; there is no strafe on a burst.
    // (0.5: the PS2's bolt prims, issue #41, are slimmer than the capsule
    // the band was first set on.)
    assert!(rates[1] > 0.5 && rates[1] < 0.95, "8 m rate {}", rates[1]);
    assert!(rates[4] < 0.35, "40 m rate {}", rates[4]);
    for (b, h) in bots.iter().zip(&rates) {
        assert!(
            *b + 0.04 >= *h,
            "a bot is hit no less than a human: {bots:?} {rates:?}"
        );
    }
    assert!(bots[1] > 0.85, "8 m rate at a bot {}", bots[1]);
    assert!(bots[4] <= bots[0] + 0.04, "and no better far off: {bots:?}");
}

/// Card CLSE4 (`analysis/re-soldier-ai.md` 20.3): the burst and the pause
/// are the weapon class's Barrage keys under each game's own rule. With
/// the class defaults 3 / 6 / 2.0 s a BF2 bot fires 3 at the lowest
/// level and 3..5 at the highest and waits (1.5 − 0.8·t) × 2.0 s; a BF1
/// bot fires 3..6 at any level and waits 2.0 s. A class that writes the
/// keys (BF2's flyer chaingun: 10 / 20 / 0.5) is fired by its own.
#[test]
fn the_burst_and_pause_are_the_weapon_classes_under_each_games_rule() {
    use crate::skill::Skill;
    use crate::soldier::GroundRules::{Bf1, Bf2};
    let me = UnitHandle { index: 3, gen: 1 };
    let rifle = crate::weapon::WeaponClass::default();
    assert_eq!((rifle.barrage, rifle.barrage_delay), ([3, 6], 2.0));
    let lens = |rules, wc: &crate::weapon::WeaponClass, skill: &Skill| {
        let mut seen = std::collections::BTreeSet::new();
        for t in 0..2048 {
            seen.insert(burst_len(rules, wc, me, t, skill));
        }
        seen.into_iter().collect::<Vec<u8>>()
    };
    let (s1, s20) = (Skill::from_level(1), Skill::from_level(20));
    assert_eq!(lens(Bf2, &rifle, &s1), [3], "BF2, the lowest level");
    assert_eq!(lens(Bf2, &rifle, &s20), [3, 4, 5], "BF2, the highest");
    assert_eq!(
        lens(Bf1, &rifle, &s1),
        [3, 4, 5, 6],
        "BF1 has no skill term"
    );
    assert_eq!(lens(Bf1, &rifle, &s20), [3, 4, 5, 6]);
    // The pause: 1.5 and 0.7 x 2.0 s x 30 Hz, and BF1's flat 2.0 s.
    assert_eq!(pause_ticks(Bf2, &rifle, &s1), 90);
    assert_eq!(pause_ticks(Bf2, &rifle, &s20), 42);
    assert_eq!(pause_ticks(Bf1, &rifle, &s1), 60);
    assert_eq!(pause_ticks(Bf1, &rifle, &s20), 60);
    // A class's own keys.
    let chaingun = crate::weapon::WeaponClass {
        barrage: [10, 20],
        barrage_delay: 0.5,
        ..crate::weapon::WeaponClass::default()
    };
    assert_eq!(lens(Bf2, &chaingun, &s1), [10]);
    assert_eq!(
        lens(Bf2, &chaingun, &s20),
        (10..20).collect::<Vec<u8>>(),
        "the maximum is never drawn"
    );
    assert_eq!(lens(Bf1, &chaingun, &s1), (10..=20).collect::<Vec<u8>>());
    assert_eq!(pause_ticks(Bf2, &chaingun, &s20), 11); // 0.7 × 0.5 × 30 = 10.5
    assert_eq!(pause_ticks(Bf1, &chaingun, &s20), 15);
    // BF2's floor of 2, whatever a class writes.
    let one = crate::weapon::WeaponClass {
        barrage: [1, 1],
        ..crate::weapon::WeaponClass::default()
    };
    assert_eq!(lens(Bf2, &one, &s20), [2]);
    assert_eq!(lens(Bf1, &one, &s20), [1]);
}

/// Card CLSE4 (`analysis/re-soldier-ai.md` 20.2): the shots of a standing
/// bot at a standing enemy `d` metres off over 20 s, its weapon's
/// `MaxRange` set to `max`; and the least and the most the two were
/// apart (the shooter shuffles on its spot).
fn shots_at(rules: crate::soldier::GroundRules, d: f32, max: f32) -> (usize, f32, f32) {
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let mut s = Sim::empty(5);
    s.ground_rules = rules;
    s.cheats.infinite_ammo = true;
    s.weapon_classes[0].ai_range = [0.0, max * 0.5, max];
    let shooter = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let t = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -d)))
        .unwrap();
    let (mut shots, mut near, mut far) = (0, f32::MAX, 0.0f32);
    for _ in 0..600 {
        s.step(&[PadState::default()]);
        s.unit_mut(t).unwrap().health.cur = 300.0;
        let apart = s.unit(shooter).unwrap().soldier.pos - s.unit(t).unwrap().soldier.pos;
        near = near.min(apart.length());
        far = far.max(apart.length());
        shots += s
            .events
            .iter()
            .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
            .count();
    }
    (shots, near, far)
}

/// Card CLSE4: a bot fires a target only strictly inside its weapon's
/// `MaxRange` - a pistol's 24 m: at 23 m it fires, at 25 m it does not,
/// in both games; the same 25 m under a rifle's 64 m is fired at (the
/// bolt reaches either way).
#[test]
fn a_bot_fires_only_inside_its_weapons_max_range() {
    for rules in [
        crate::soldier::GroundRules::Bf2,
        crate::soldier::GroundRules::Bf1,
    ] {
        let (shots, _, far) = shots_at(rules, 23.0, 24.0);
        assert!(
            shots > 10 && far < 23.5,
            "{rules:?}: 23 m of 24: {shots} {far}"
        );
        let (shots, near, _) = shots_at(rules, 25.0, 24.0);
        assert!(near > 24.5, "{rules:?}: it stays beyond the range: {near}");
        assert_eq!(shots, 0, "{rules:?}: 25 m of 24");
        let (shots, _, _) = shots_at(rules, 25.0, 64.0);
        assert!(shots > 10, "{rules:?}: 25 m of 64: {shots}");
    }
}

/// Card STTAP's fixture: a bot with a single-shot weapon 8 m from a
/// standing target for `ticks`. Returns the ticks it fired on and every
/// value its weapon's trigger byte took.
fn tap_run(
    rules: crate::soldier::GroundRules,
    skill: u8,
    ticks: u32,
    class: fn(&mut crate::weapon::WeaponClass),
) -> (Vec<u64>, Vec<i32>) {
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let mut s = Sim::empty(5);
    s.ground_rules = rules;
    s.cheats.infinite_ammo = true;
    s.weapon_classes[0].trigger_single = true;
    class(&mut s.weapon_classes[0]);
    let shooter = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    s.unit_mut(shooter).unwrap().skill = skill;
    let t = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -8.0)))
        .unwrap();
    let (mut shots, mut cmds) = (Vec::new(), Vec::new());
    for _ in 0..ticks {
        s.step(&[PadState::default()]);
        s.unit_mut(t).unwrap().health.cur = 300.0;
        cmds.push(s.unit(shooter).unwrap().weapon.fire_cmd);
        if s.events
            .iter()
            .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
        {
            shots.push(s.tick);
        }
    }
    (shots, cmds)
}

/// Card STTAP: a BF2 bot TAPS a single-shot weapon (`FUN_002baf70`) — the
/// trigger is down one tick at a time and comes back after `U(0.2, 2.0) ×
/// tap_x` s, so the weapon keeps firing where a held trigger fired once.
#[test]
fn a_bf2_bot_taps_a_single_shot_weapon() {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::soldier::GroundRules::Bf2;
    // A rocket launcher's pace (one shot a second), the worst skill: the
    // re-arm is 0.18–1.8 s.
    let (shots, cmds) = tap_run(Bf2, 1, 300, |c| c.shot_delay = [1.0, 1.0]);
    assert!(shots.len() >= 5, "skill 1 fired {} in 10 s", shots.len());
    assert!(cmds.iter().all(|&c| c <= 1), "a press lasts one tick");
    // The best skill re-arms in 0.02–0.2 s: a press every 2–8 ticks, and
    // a weapon that is ready every 6 fires on most of them.
    let (shots, cmds) = tap_run(Bf2, 20, 300, |_| {});
    assert!(cmds.iter().all(|&c| c <= 1), "a press lasts one tick");
    let presses: Vec<usize> = (0..cmds.len()).filter(|&i| cmds[i] == 1).collect();
    let gaps: Vec<usize> = presses.windows(2).map(|w| w[1] - w[0]).collect();
    let inner = &gaps[1..];
    assert!(
        inner.iter().all(|g| (2..=8).contains(g)),
        "press gaps {gaps:?}"
    );
    assert!(shots.len() >= 25, "skill 20 fired {}", shots.len());
}

/// Card STTAP: a BF2 sniper (TriggerSingle and ZoomMax ≥ 6) presses once
/// per `U(2, 3)` s (`FUN_002baa38`).
#[test]
fn a_bf2_sniper_fires_once_per_two_to_three_seconds() {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::soldier::GroundRules::Bf2;
    let (shots, _) = tap_run(Bf2, 20, 900, |c| c.zoom_max = 8.0);
    let gaps: Vec<u64> = shots.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(shots.len() >= 9, "fired {}", shots.len());
    assert!(gaps.iter().all(|g| (60..=90).contains(g)), "gaps {gaps:?}");
    assert!(gaps.iter().any(|&g| g > 62) && gaps.iter().any(|&g| g < 88));
    // ZoomMax 5.99 is not a sniper: it taps.
    let (shots, _) = tap_run(Bf2, 20, 900, |c| c.zoom_max = 5.99);
    assert!(shots.len() > 60, "fired {}", shots.len());
}

/// Card STTAP: a BF1 bot's trigger byte runs 1 → 2 → −1 → −2 → 1
/// (`FUN_0023cc20`), one fresh press every fourth tick; a BF1 sniper also
/// waits `FUN_0023c698`'s `U(2, 3)` s.
#[test]
fn a_bf1_bot_presses_a_single_shot_weapon_every_fourth_tick() {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::soldier::GroundRules::Bf1;
    let (shots, cmds) = tap_run(Bf1, 20, 300, |c| c.shot_delay = [0.1, 0.1]);
    // The weapon's own byte reads the pad: down, down, up, up. From the
    // second think on: in the first the error is still near its 1.5 m
    // no-target floor, its point swings over 2° a tick at 8 m, and the
    // gun, which follows it inside a tick (the PS2's tracker), can be off
    // the aim tolerance for a tick.
    let first = (20..cmds.len()).find(|&i| cmds[i] == 1).expect("a press");
    for (i, w) in cmds[first..].as_chunks::<4>().0.iter().take(40).enumerate() {
        assert_eq!(*w, [1, 2, -1, 0], "cycle {i}");
    }
    // Every shot is a fresh press, so none comes sooner than the fourth
    // tick; a tick off the aim tolerance may stretch a gap.
    let shots: Vec<u64> = shots.into_iter().filter(|&t| t > first as u64).collect();
    let gaps: Vec<u64> = shots.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.iter().all(|&g| g >= 4), "gaps {gaps:?}");
    let on_beat = gaps.iter().filter(|&&g| g == 4).count();
    assert!(on_beat * 10 >= gaps.len() * 9, "gaps {gaps:?}");
    assert!(
        shots.len() >= 55,
        "fired {} after the first think",
        shots.len()
    );

    let (shots, cmds) = tap_run(Bf1, 20, 900, |c| c.zoom_max = 6.0);
    let gaps: Vec<u64> = shots.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(shots.len() >= 9, "fired {}", shots.len());
    assert!(gaps.iter().all(|g| (60..=92).contains(g)), "gaps {gaps:?}");
    assert!(cmds.iter().all(|&c| c <= 1), "a sniper's press is one tick");
}

/// D11: a bot fires in BURSTS separated by the skill's pause, where
/// before it held the trigger down for as long as it was aligned.
/// Card FFLN: a bot does not fire through a friend. One bot, its enemy
/// 14 m ahead, and a friend standing on the line between them or 4 m off
/// it. On the line: no shot leaves while the friend is in the cone, no
/// bolt of the bot's lands on the friend, the waiting is counted, and the
/// bot does not stay mute — it steps out of the line and fires (a bot
/// with no Tactician's position, as here: card TPOS keeps the step for
/// them). Off the line: nothing is withheld.
#[test]
fn a_bot_does_not_fire_through_a_friend() {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let run = |friend_x: f32| {
        let mut s = Sim::empty(5);
        s.cheats.infinite_ammo = true;
        let shooter = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let enemy = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -14.0)))
            .unwrap();
        let friend = s
            .spawn(&UnitSpec::trooper(0, Vec3::new(friend_x, 0.0, -5.0)))
            .unwrap();
        let (mut shots, mut friend_hits, mut through) = (0u32, 0u32, 0u32);
        for _ in 0..900 {
            // What the bot's think is about to see.
            let u = s.unit(shooter).unwrap();
            let wc = &s.weapon_classes[u.weapon_class];
            // A salvo the refire branch has already started leaves at
            // the next update whatever the trigger does (weapon.rs: only
            // holstering cancels it, as on the PS2) - not a shot the
            // think let go, so it is not counted (card SPRD: the bot's
            // sweeping aim met the friend on such a tick).
            let queued = u.weapon.salvo_counter > 0;
            let blocked = !queued
                && friend_in_line(
                    &s,
                    shooter,
                    u,
                    u.muzzle(),
                    14.0,
                    LINE_SPREAD + u.weapon.yaw_spread(wc).max(u.weapon.pitch_spread(wc)),
                    None,
                )
                .is_some();
            s.step(&[PadState::default()]);
            s.unit_mut(enemy).unwrap().health.cur = 300.0;
            for e in &s.events {
                match *e {
                    Event::Shot { unit, .. } if unit == shooter => {
                        shots += 1;
                        through += u32::from(blocked);
                    }
                    Event::Hit {
                        shooter: a, victim, ..
                    } if a == shooter && victim == friend => friend_hits += 1,
                    _ => {}
                }
            }
        }
        (shots, friend_hits, through, s.nav_stats.line_held)
    };
    let (shots, friend_hits, through, held) = run(0.0);
    assert_eq!(friend_hits, 0, "bolts into the friend");
    assert_eq!(through, 0, "shots that left with the friend in the cone");
    assert!(held > 0, "the wait is counted");
    assert!(shots > 20, "stepped aside and fired: {shots} shots");
    let (shots, friend_hits, _, held) = run(4.0);
    assert_eq!((friend_hits, held), (0, 0), "a friend off the line");
    assert!(shots > 20, "{shots} shots");
}

/// Card TKFIX (OURS): a charge weapon (the bowcaster) shoots on the
/// RELEASE, so a friend who steps into the line while the bot charges is
/// covered at the release, not only at the press. Each time the bot has
/// held its trigger four ticks, a friend is put on the line 5 m ahead and
/// kept there for a second: no release leaves with him in the cone, no bolt lands on
/// him, and the bot still fires once he has gone.
#[test]
fn a_charged_shot_is_not_released_into_a_friend() {
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let mut s = Sim::empty(5);
    s.cheats.infinite_ammo = true;
    s.weapon_classes[0].max_pressed_time = 1.0;
    let shooter = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -14.0)))
        .unwrap();
    let away = Vec3::new(8.0, 0.0, -5.0);
    let friend = s.spawn(&UnitSpec::trooper(0, away)).unwrap();
    let (mut releases, mut through, mut friend_hits) = (0u32, 0u32, 0u32);
    let (mut stay, mut held) = (0u32, 0u32);
    for _ in 0..1800 {
        let u = s.unit(shooter).unwrap();
        held = if u.weapon.fire_cmd > 0 { held + 1 } else { 0 };
        // A few ticks into the charge: the last release's bolt is gone.
        if held == 4 && stay == 0 {
            // On the line, in front of wherever the bot now stands.
            let (dir, _, _) = u.aim_frame();
            let at = u.soldier.pos + dir.with_y(0.0).normalize_or_zero() * 5.0;
            s.unit_mut(friend).unwrap().soldier.pos = at;
            stay = 30;
        } else if stay > 0 {
            stay -= 1;
            if stay == 0 {
                s.unit_mut(friend).unwrap().soldier.pos = away;
            }
        }
        s.step(&[PadState::default()]);
        s.unit_mut(enemy).unwrap().health.cur = 300.0;
        s.unit_mut(friend).unwrap().soldier.vel = Vec3::ZERO;
        let u = s.unit(shooter).unwrap();
        if u.weapon.fire_cmd == -1 {
            releases += 1;
            let wc = &s.weapon_classes[u.weapon_class];
            let blocked = friend_in_line(
                &s,
                shooter,
                u,
                u.muzzle(),
                14.0,
                LINE_SPREAD + u.weapon.yaw_spread(wc).max(u.weapon.pitch_spread(wc)),
                None,
            )
            .is_some();
            through += u32::from(blocked && stay > 0);
        }
        for e in &s.events {
            if let Event::Hit {
                shooter: a, victim, ..
            } = *e
            {
                friend_hits += u32::from(a == shooter && victim == friend);
            }
        }
    }
    assert_eq!(friend_hits, 0, "charged bolts into the friend");
    assert_eq!(through, 0, "releases with the friend in the cone");
    assert!(releases > 4, "the bot still fires: {releases} releases");
}

/// Card HVYW (OURS): a missile's line of fire is cleared by what the
/// shooter walks in the wind-up and what the friend walks until the
/// rocket is where he is. A friend 2 m off the line 5 m ahead: standing,
/// he is in nobody's line; walking at 6 m/s he is in a rocket's (a
/// quarter-second wind-up, 100 m/s: 0.75 + 6 x 0.3 = 2.55 m) and in no
/// rifle's; and a shooter who walks at 6 m/s has him in the rocket's
/// (0.75 + 6 x 0.25 = 2.25 m). 30 m out the flight counts: 3.5 m off
/// and walking he is in it (0.75 + 6 x 0.55 = 4.05 m).
#[test]
fn a_missiles_line_is_cleared_by_what_is_walked_before_it_gets_there() {
    use crate::sim::Sim;
    use crate::unit::UnitSpec;
    let rocket = Some((0.25, 100.0));
    let ask = |at: Vec3, friend_v: f32, own_v: f32, late| {
        let mut s = Sim::empty(5);
        let shooter = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        let friend = s.spawn(&UnitSpec::trooper(0, at)).unwrap();
        s.units[friend.index as usize].soldier.vel = Vec3::X * friend_v;
        s.units[shooter.index as usize].soldier.vel = Vec3::X * own_v;
        let u = s.unit(shooter).unwrap();
        friend_in_line(&s, shooter, u, u.muzzle(), 40.0, 0.0, late)
    };
    let near = Vec3::new(2.0, 0.0, -5.0);
    assert_eq!(ask(near, 0.0, 0.0, None), None);
    assert_eq!(ask(near, 0.0, 0.0, rocket), None, "a standing friend");
    assert_eq!(ask(near, 6.0, 0.0, None), None, "a rifle's line");
    assert_eq!(ask(near, 6.0, 0.0, rocket), Some(-1), "he walks");
    assert_eq!(ask(near, 0.0, 6.0, rocket), Some(-1), "the shooter walks");
    // Just outside each: 2.6 m off for the friend's walk (2.55), 2.3 m
    // for the shooter's (2.25).
    assert_eq!(ask(Vec3::new(2.6, 0.0, -5.0), 6.0, 0.0, rocket), None);
    assert_eq!(ask(Vec3::new(2.3, 0.0, -5.0), 0.0, 6.0, rocket), None);
    // The flight: 30 m out, 3.5 m off (0.75 + 6 x 0.55 = 4.05), and not
    // with the wind-up alone (0.75 + 1.5 = 2.25).
    let far = Vec3::new(3.5, 0.0, -30.0);
    assert_eq!(ask(far, 6.0, 0.0, rocket), Some(-1));
    assert_eq!(ask(far, 6.0, 0.0, Some((0.25, 1e6))), None);
    assert_eq!(ask(far, 6.0, 0.0, None), None);
}

/// Card RKEX (OURS): a weapon whose ordnance explodes (Damage radius 4)
/// holds its shot while a friend on foot stands within 4 + 1 m of the
/// target - 3 m off it he is, 5.5 m off he is not, nor is an enemy
/// there; a rifle (no explosion) never asks.
#[test]
fn an_exploding_shot_waits_for_a_friend_beside_its_target() {
    use crate::sim::Sim;
    use crate::unit::UnitSpec;
    use crate::weapon::ExplosionClass;
    let target = Vec3::new(0.0, 1.0, -20.0);
    let ask = |at: Vec3, team: u8, explodes: bool| {
        let mut s = Sim::empty(5);
        let shooter = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        let f = s.spawn(&UnitSpec::trooper(team, at)).unwrap();
        // No ground in `Sim::empty`: stand him at y 0.
        s.units[f.index as usize].soldier.pos = at;
        let mut wc = s.weapon_classes[0].clone();
        if explodes {
            wc.ordnance.explosion = Some(ExplosionClass {
                damage: 400.0,
                damage_inner: 2.0,
                damage_outer: 4.0,
                ..ExplosionClass::default()
            });
        }
        let u = s.unit(shooter).unwrap();
        friend_in_blast(&s, shooter, u, target, &wc)
    };
    let beside = Vec3::new(3.0, 0.0, -20.0);
    assert_eq!(ask(beside, 0, true), Some(-1), "a friend beside it");
    assert_eq!(
        ask(Vec3::new(-3.0, 0.0, -20.0), 0, true),
        Some(1),
        "the other side"
    );
    assert_eq!(ask(Vec3::new(5.5, 0.0, -20.0), 0, true), None, "outside");
    assert_eq!(ask(beside, 1, true), None, "an enemy");
    assert_eq!(ask(beside, 0, false), None, "a rifle");
}

/// Card TEAM (OURS): any shot's line is cleared by one tick of walking
/// (the sim fires after it moves the units) and by its flight. A friend
/// 1.5 m ahead and 0.9 m off the line is in no standing rifleman's line
/// (0.75 m); walking at 6 m/s he is (0.75 + 6 x (1/30 + 1.5/v)), and so
/// he is to a shooter walking at 6 m/s (0.75 + 6/30); 1.2 m off he is in
/// neither.
#[test]
fn a_shots_line_is_cleared_by_a_ticks_walk_and_its_flight() {
    use crate::sim::Sim;
    use crate::unit::UnitSpec;
    let ask = |at: Vec3, friend_v: f32, own_v: f32| {
        let mut s = Sim::empty(5);
        let shooter = s.spawn(&UnitSpec::trooper(0, Vec3::ZERO)).unwrap();
        let friend = s.spawn(&UnitSpec::trooper(0, at)).unwrap();
        s.units[friend.index as usize].soldier.vel = Vec3::X * friend_v;
        s.units[shooter.index as usize].soldier.vel = Vec3::X * own_v;
        let u = s.unit(shooter).unwrap();
        let wc = &s.weapon_classes[u.weapon_class];
        assert_ne!(wc.ordnance.kind, crate::weapon::OrdKind::Missile);
        assert!(wc.ordnance.velocity > 20.0, "a bolt");
        let late = shot_late(wc);
        assert_eq!(late.0, 1.0 / crate::TICK_HZ as f32);
        friend_in_line(&s, shooter, u, u.muzzle(), 40.0, 0.0, Some(late))
    };
    let near = Vec3::new(0.9, 0.0, -1.5);
    assert_eq!(ask(near, 0.0, 0.0), None, "a standing friend");
    assert_eq!(ask(near, 6.0, 0.0), Some(-1), "he walks");
    assert_eq!(ask(near, 0.0, 6.0), Some(-1), "the shooter walks");
    assert_eq!(ask(Vec3::new(1.2, 0.0, -1.5), 6.0, 0.0), None);
    assert_eq!(ask(Vec3::new(1.2, 0.0, -1.5), 0.0, 6.0), None);
}

/// The whole trigger, both games: a standing bot's shots at a standing
/// enemy 8 m off come in bursts of the class's bounds with the class's
/// pause between them, counted from a burst's last shot (card CLSE4).
#[test]
fn the_ai_fires_in_bursts_with_a_pause_between_them() {
    for rules in [
        crate::soldier::GroundRules::Bf2,
        crate::soldier::GroundRules::Bf1,
    ] {
        bursts_with_a_pause(rules);
    }
}

/// Card BRSTU (`analysis/re-soldier-ai.md` 23.2): a hand weapon with
/// unlimited rounds (the repeater's kind: `RoundsPerClip` 0, heat a
/// shot) is not held to the class's 3-6 shots - a bot on foot fires it
/// until its heat is a shot short of full, and it never overheats.
#[test]
fn an_unlimited_hand_weapon_bursts_on_its_heat() {
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    for rules in [
        crate::soldier::GroundRules::Bf1,
        crate::soldier::GroundRules::Bf2,
    ] {
        let mut s = Sim::empty(5);
        s.ground_rules = rules;
        s.weapon_classes[0] = WeaponClass {
            rounds_per_clip: 0,
            heat_per_shot: 0.1,
            heat_recover_rate: 0.4,
            heat_threshold: 0.5,
            ..WeaponClass::bf1_rifle()
        };
        let shooter = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -8.0)))
            .unwrap();
        let (mut shots, mut peak) = (Vec::new(), 0.0f32);
        for _ in 0..1800 {
            s.step(&[PadState::default()]);
            s.unit_mut(t).unwrap().health.cur = 300.0;
            let u = s.unit(shooter).unwrap();
            assert!(!u.weapon.overheated, "{rules:?}: overheated at {}", s.tick);
            peak = peak.max(u.weapon.heat);
            if s.events
                .iter()
                .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
            {
                shots.push(s.tick);
            }
        }
        let mut runs = vec![1u32];
        for w in shots.windows(2) {
            if w[1] - w[0] > 7 {
                runs.push(1);
            } else {
                *runs.last_mut().unwrap() += 1;
            }
        }
        assert!(runs.len() >= 4, "{rules:?}: {runs:?}");
        assert!(
            runs[..runs.len() - 1].iter().all(|&n| n > 6),
            "{rules:?}: {runs:?}"
        );
        assert!(peak > 0.85, "{rules:?}: peak heat {peak}");
    }
}

fn bursts_with_a_pause(rules: crate::soldier::GroundRules) {
    // Card FGHT: a shooter that keeps its place.
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    let mut s = Sim::empty(5);
    s.ground_rules = rules;
    s.cheats.infinite_ammo = true;
    let shooter = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    let t = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -8.0)))
        .unwrap();
    let mut shot_ticks = Vec::new();
    for _ in 0..2700 {
        s.step(&[PadState::default()]);
        s.unit_mut(t).unwrap().health.cur = 300.0;
        if s.events
            .iter()
            .any(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
        {
            shot_ticks.push(s.tick);
        }
    }
    assert!(shot_ticks.len() > 20, "fired {} shots", shot_ticks.len());
    // Split the shots into runs at every gap longer than the weapon's own
    // refire period (ShotDelay 0.2 s = 6 ticks).
    let wc = &s.weapon_classes[0];
    let refire = (wc.shot_delay[0] * crate::TICK_HZ as f32).round() as u64;
    let mut runs = vec![1u32];
    for w in shot_ticks.windows(2) {
        if w[1] - w[0] > refire + 1 {
            runs.push(1);
        } else {
            *runs.last_mut().unwrap() += 1;
        }
    }
    // Drop the first and last runs: they are truncated by the window.
    let inner = &runs[1..runs.len() - 1];
    assert!(
        inner.len() >= 10,
        "{rules:?}: only {} whole bursts: {runs:?}",
        inner.len()
    );
    // The bounds are the class's 3 / 6: BF2 never draws the maximum,
    // BF1 does.
    let [min, max] = wc.barrage.map(u32::from);
    let top = match rules {
        crate::soldier::GroundRules::Bf2 => max - 1,
        crate::soldier::GroundRules::Bf1 => max,
    };
    for &n in inner {
        assert!(n <= top, "{rules:?}: burst of {n}: {runs:?}");
    }
    // A run SHORTER than the minimum is legitimate and rare: the counter
    // only counts shots that were actually emitted, so a bot that loses
    // its alignment mid-burst stops early and its remaining shots come
    // after the next gap.
    let short = inner.iter().filter(|&&n| n < min).count();
    assert!(
        short * 10 <= inner.len(),
        "{rules:?}: {short} of {} runs are short: {runs:?}",
        inner.len()
    );
    // The gaps between bursts are the class's pause from the last shot:
    // none shorter, and the usual one no longer than the next tick the
    // weapon and the aim allow.
    let skill = s.difficulty.skill_at(s.unit(shooter).unwrap().skill);
    let pause = pause_ticks(rules, wc, &skill);
    assert!(pause >= 42, "{rules:?}: 2.0 s by the skill: {pause}");
    let gaps: Vec<u64> = shot_ticks.windows(2).map(|w| w[1] - w[0]).collect();
    let mut long: Vec<u64> = gaps.iter().copied().filter(|g| *g > refire + 1).collect();
    long.sort_unstable();
    // (A reload inside a burst is a gap too, and is not a pause.)
    let under = long.iter().filter(|g| **g < pause).count();
    assert!(
        under * 10 <= long.len(),
        "{rules:?}: pauses {long:?} of {pause}"
    );
    let median = long[long.len() / 2];
    assert!(
        median <= pause + refire,
        "{rules:?}: pauses {long:?} of {pause}"
    );
}

/// D11's charge weapons: the bot HOLDS the trigger for
/// `U(0.15, charge_hi) × MaxPressedTime` and the weapon fires on the
/// RELEASE (`fire_predicate`: `fire_cmd == -1`), so the hold length is
/// the drawn window and a high-skill bot (charge_hi = t = 1.0) may hold
/// far longer than a low one (t = 0.158 at skill 4). No roster soldier
/// carries a charge weapon today — this class is synthetic.
#[test]
fn a_charge_weapon_is_held_then_released_inside_the_skill_window() {
    use crate::sim::Sim;
    use crate::unit::{Controller, UnitSpec};
    const MAX_PRESSED: f32 = 1.0;
    // Ticks the pad had held FIRE when the weapon saw the release edge.
    let holds = |skill: u8| {
        let mut s = Sim::empty(5);
        s.cheats.infinite_ammo = true;
        s.weapon_classes[0].max_pressed_time = MAX_PRESSED;
        let shooter = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        s.unit_mut(shooter).unwrap().skill = skill;
        let t = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -8.0)))
            .unwrap();
        let (mut out, mut held) = (Vec::new(), 0i32);
        for _ in 0..1200 {
            s.step(&[PadState::default()]);
            s.unit_mut(t).unwrap().health.cur = 300.0;
            let cmd = s.unit(shooter).unwrap().weapon.fire_cmd;
            if cmd == -1 {
                out.push(held);
            }
            held = cmd.max(0);
        }
        assert!(out.len() > 4, "skill {skill}: {} releases", out.len());
        out
    };
    let lo = holds(4);
    let hi = holds(20);
    let ticks = |f: f32| (f * MAX_PRESSED * crate::TICK_HZ as f32).round() as i32;
    let (floor, ceil4) = (ticks(CHARGE_MIN), ticks(0.15789474));
    let ceil20 = ticks(1.0);
    // Skill 4's window is [0.15, 0.158] × 1 s — 5 ticks, near-constant.
    assert!(
        lo.iter().all(|&h| (floor..=ceil4).contains(&h)),
        "skill 4 holds outside [{floor}, {ceil4}]: {lo:?}"
    );
    // Skill 20's is the whole [0.15, 1.0] × 1 s.
    assert!(
        hi.iter().all(|&h| (floor..=ceil20).contains(&h)),
        "skill 20 holds outside [{floor}, {ceil20}]: {hi:?}"
    );
    let mean = |v: &[i32]| v.iter().sum::<i32>() as f32 / v.len() as f32;
    assert!(
        mean(&hi) > mean(&lo) * 2.0,
        "skill 20 must charge far longer: {} vs {}",
        mean(&hi),
        mean(&lo)
    );
    // ... and the spread of the window is what separates them.
    assert!(
        hi.iter().max() > lo.iter().max(),
        "skill 20 never used the top of its window: {hi:?}"
    );
}

#[test]
fn yaw_toward_matches_the_aim_frame() {
    // Facing -Z is yaw 0; facing -X (left) is +90°.
    assert!((yaw_toward(Vec3::NEG_Z)).abs() < 1e-6);
    assert!((yaw_toward(Vec3::NEG_X) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert!((yaw_toward(Vec3::X) + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert!((wrap(4.0) - (4.0 - std::f32::consts::TAU)).abs() < 1e-6);
}

/// Card CLSE (`analysis/re-soldier-ai.md` 17.2): BF2's error vector is
/// 0.3 of itself against a bot and whole against a human, times
/// d / 20 + 0.5 (at most 10), never below the aim point, and nothing for
/// a bot that rides with a human (any seat; the target's vehicle too).
#[test]
fn bf2_scales_the_error_vector_by_target_and_range() {
    use crate::unit::{Controller, UnitSpec};
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::HealthType;
    let mut s = crate::sim::Sim::empty(5);
    let bot = |s: &mut crate::sim::Sim, team: u8, z: f32| {
        s.spawn(&UnitSpec {
            controller: Controller::Ai,
            ..UnitSpec::trooper(team, Vec3::new(0.0, 0.0, z))
        })
        .unwrap()
    };
    let (a, t, mate) = (
        bot(&mut s, 0, 0.0),
        bot(&mut s, 1, -20.0),
        bot(&mut s, 0, 4.0),
    );
    let off = Vec3::new(0.6, 0.4, -0.3);
    let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-6;
    let vec = |s: &crate::sim::Sim, off: Vec3, d: f32| {
        error_vector(s, a, s.unit(t).unwrap(), off, d, &WeaponClass::default())
    };
    // Bot against bot: 0.3, and the range factor 0.75 / 1.5 / 2.5 / 10.
    assert!(close(vec(&s, off, 5.0), off * 0.3 * 0.75));
    assert!(close(vec(&s, off, 20.0), off * 0.3 * 1.5));
    assert!(close(vec(&s, off, 40.0), off * 0.3 * 2.5));
    assert!(close(vec(&s, off, 400.0), off * 0.3 * 10.0));
    // Never below the aim point.
    let down = Vec3::new(0.6, -0.4, -0.3);
    assert!(close(
        vec(&s, down, 20.0),
        Vec3::new(0.6, 0.0, -0.3) * 0.3 * 1.5
    ));
    // A human target keeps the vector whole (the range factor stays),
    // and it too is never aimed under.
    s.units[t.index as usize].is_player = true;
    assert!(close(vec(&s, off, 20.0), off * 1.5));
    assert!(close(vec(&s, off, 5.0), off * 0.75));
    assert!(close(vec(&s, down, 20.0), Vec3::new(0.6, 0.0, -0.3) * 1.5));
    s.units[t.index as usize].is_player = false;
    // Card CLSE2: the weapon class's three AIBubble keys - the vector's
    // factor, the range divider and the range factor's cap.
    let keyed = |size: f32, dist: f32, clamp: f32, d: f32| {
        let wc = WeaponClass {
            ai_bubble_size: size,
            ai_bubble_dist: dist,
            ai_bubble_clamp: clamp,
            ..WeaponClass::default()
        };
        error_vector(&s, a, s.unit(t).unwrap(), off, d, &wc)
    };
    assert!(close(keyed(1.25, 20.0, 10.0, 20.0), off * 1.25 * 0.3 * 1.5));
    assert!(close(keyed(0.6, 20.0, 10.0, 20.0), off * 0.6 * 0.3 * 1.5));
    assert!(close(keyed(1.0, 100.0, 10.0, 50.0), off * 0.3 * 1.0));
    assert!(close(keyed(1.0, 100.0, 3.0, 400.0), off * 0.3 * 3.0));
    // A bot in a vehicle a human rides aims with no error; alone in it,
    // with its own.
    let v = s.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Hover,
        class: 0,
        home_team: None,
        neutral_when_empty: true,
        spawn_pad: None,
        pos: Vec3::new(12.0, 2.0, 0.0),
        yaw: 0.0,
        max_health: 1000.0,
        health_type: HealthType::Vehicle,
        seats: vec![SeatKind::Pilot, SeatKind::Gunner(0)],
    });
    assert!(s.enter_vehicle(a, v, 1));
    assert!(close(vec(&s, off, 20.0), off * 0.3 * 1.5), "alone in it");
    assert!(s.enter_vehicle(mate, v, 0));
    assert!(close(vec(&s, off, 20.0), off * 0.3 * 1.5), "a bot drives");
    s.units[mate.index as usize].is_player = true;
    assert_eq!(vec(&s, off, 20.0), Vec3::ZERO, "a human drives");
    // Every seat is asked, not the first: the bot drives, the human is
    // its gunner.
    s.exit_vehicle(a);
    s.exit_vehicle(mate);
    assert!(s.enter_vehicle(a, v, 0));
    assert!(s.enter_vehicle(mate, v, 1));
    assert_eq!(vec(&s, off, 20.0), Vec3::ZERO, "a human is the gunner");
    s.exit_vehicle(a);
    // The TARGET's vehicle is asked too: a bot that rides with a human
    // is aimed at with the whole vector.
    assert!(close(vec(&s, off, 20.0), off * 0.3 * 1.5));
    let seated = |s: &crate::sim::Sim, d: f32| {
        error_vector(s, a, s.unit(mate).unwrap(), off, d, &WeaponClass::default())
    };
    assert!(close(seated(&s, 20.0), off * 1.5), "mate is the human");
    s.exit_vehicle(mate);
    s.units[mate.index as usize].is_player = false;
    s.units[t.index as usize].is_player = true;
    s.units[t.index as usize].team = 0;
    assert!(s.enter_vehicle(t, v, 0));
    assert!(s.enter_vehicle(mate, v, 1));
    assert!(close(seated(&s, 20.0), off * 1.5), "a bot beside a human");
}

/// RKMAR: fixed capsule-centre distances from the predicted blast point,
/// independent of the policy's margin. Both lateral sides and both teams shoot.
fn rkmar_blast_clearance(rules: crate::soldier::GroundRules) {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::unit::UnitSpec;
    use crate::vehicle::{SeatKind, VehicleKind, VehicleSpec};
    use crate::weapon::ExplosionClass;
    use crate::weapon::HealthType;
    for team in [0, 1] {
        for side in [-1.0, 1.0] {
            for (distance, other_team, dead, mounted, explosive, held) in [
                (3.0, team, false, false, true, true),
                (4.5, team, false, false, true, true),
                (5.5, team, false, false, true, false),
                (4.5, 1 - team, false, false, true, false),
                (4.5, team, true, false, true, false),
                (4.5, team, false, true, true, false),
                (4.5, team, false, false, false, false),
            ] {
                let mut s = Sim::empty(5);
                s.ground_rules = rules;
                let shooter = s
                    .spawn(&UnitSpec::trooper(team, Vec3::new(0.0, 0.0, 20.0)))
                    .unwrap();
                s.units[shooter.index as usize].soldier.pos = Vec3::new(0.0, 0.0, 20.0);
                s.collision = CollisionWorld::new(
                    vec![Body::new(
                        glam::Mat4::from_translation(Vec3::new(0.0, 1.5, -3.0)),
                        Shape::Box {
                            half: Vec3::new(20.0, 4.0, 0.5),
                        },
                    )],
                    vec![],
                );
                if explosive {
                    s.weapon_classes[0].ordnance.explosion = Some(ExplosionClass {
                        damage: 400.0,
                        damage_inner: 2.0,
                        damage_outer: 4.0,
                        ..ExplosionClass::default()
                    });
                }
                let u = s.unit(shooter).unwrap();
                let muzzle = u.muzzle();
                let blast = Vec3::new(muzzle.x, muzzle.y, -2.3);
                let other = s.spawn(&UnitSpec::trooper(other_team, Vec3::ZERO)).unwrap();
                let offset =
                    s.unit(other).unwrap().capsule().center() - s.unit(other).unwrap().soldier.pos;
                let pos = blast + Vec3::X * side * distance - offset;
                s.units[other.index as usize].soldier.pos = pos;
                if mounted {
                    let vehicle = s.spawn_vehicle(&VehicleSpec {
                        kind: VehicleKind::Hover,
                        class: 0,
                        home_team: None,
                        neutral_when_empty: true,
                        spawn_pad: None,
                        pos,
                        yaw: 0.0,
                        max_health: 1000.0,
                        health_type: HealthType::Vehicle,
                        seats: vec![SeatKind::Pilot],
                    });
                    assert!(s.enter_vehicle(other, vehicle, 0));
                }
                // Keep the legal mounted soldier at the measured seat position.
                let centre = s.units[other.index as usize].capsule().center();
                assert!(((centre - blast).length() - distance).abs() < 1e-4);
                if dead {
                    s.units[other.index as usize].alive = false;
                }
                let u = s.unit(shooter).unwrap();
                let bolt = crate::projectile::Bolt::spawn(
                    muzzle,
                    Vec3::NEG_Z,
                    s.weapon_classes[0].ordnance.velocity,
                    s.weapon_classes[0].ordnance.life_span,
                    crate::projectile::Shooter {
                        owner: shooter,
                        team,
                        weapon_class: 0,
                        is_player: false,
                    },
                    None,
                    None,
                    &s.collision,
                    crate::collision::HullFilter::All,
                    s.alive_units()
                        .filter(|(h, _)| *h != shooter)
                        .map(|(h, o)| (h, o.capsule())),
                );
                assert!(matches!(bolt.hit, crate::projectile::Hit::World));
                assert!((bolt.end - Vec3::new(muzzle.x, muzzle.y, -2.5)).length() < 1e-4);
                let aim = friend_in_blast(&s, shooter, u, blast, &s.weapon_classes[0]);
                assert_eq!(aim, if held { Some(if side > 0.0 { -1 } else { 1 }) } else { None },
                        "aim {rules:?} team={team} side={side} d={distance} dead={dead} mounted={mounted} explosive={explosive}");
                assert_eq!(release_blasts_own_side(&s, shooter.index as usize), held,
                        "release {rules:?} team={team} side={side} d={distance} dead={dead} mounted={mounted} explosive={explosive}");
            }
        }
    }
}

#[test]
fn rkmar_bf1_blast_clearance_at_aim_and_release() {
    rkmar_blast_clearance(crate::soldier::GroundRules::Bf1);
}

#[test]
fn rkmar_bf2_blast_clearance_at_aim_and_release() {
    rkmar_blast_clearance(crate::soldier::GroundRules::Bf2);
}

/// [ours] A bot's error direction turns a little every tick instead of
/// jumping once a think (playtest 2026-10-29, the Geonosis cannon's
/// vibration): BF2's block direction reaches each think's `dir` on the tick
/// before the next think, and BF1's drawn direction the tick before the
/// next draw; no tick turns it more than a think's turn over the think's
/// ticks, plus rounding.
#[test]
fn the_error_direction_turns_every_tick_not_once_a_think() {
    use crate::unit::AimBlock;
    for rules in [
        crate::soldier::GroundRules::Bf2,
        crate::soldier::GroundRules::Bf1,
    ] {
        let (mut s, a, _) = aim_pair(rules);
        let ai = a.index as usize;
        s.units[ai].aim = AimBlock::default();
        let mut prev: Option<Vec3> = None;
        let mut worst = 0.0f32;
        let mut moved = 0;
        for tick in 0..60u64 {
            s.tick = tick;
            let p = AimBlock::PERIOD;
            if rules == crate::soldier::GroundRules::Bf2
                && (tick + a.index as u64).is_multiple_of(p)
            {
                let k = tick as f32;
                let jolt = Vec3::new((k * 0.7).sin(), (k * 1.3).cos(), (k * 0.4).sin());
                s.units[ai]
                    .aim
                    .think(p as f32 * crate::TICK_DT, Vec3::ZERO, 5.0, 1.0, 0.4, jolt);
            }
            let d = match rules {
                crate::soldier::GroundRules::Bf2 => error_dir(&s, a, &s.units[ai]),
                _ => bf1_error_dir(&s, a),
            };
            assert!((d.length() - 1.0).abs() < 1e-4, "{rules:?}: a unit vector");
            if let (Some(q), true) = (prev, tick >= 10) {
                let turn = q.angle_between(d);
                worst = worst.max(turn);
                moved += usize::from(turn > 1e-4);
            }
            if rules == crate::soldier::GroundRules::Bf2
                && (tick + 1 + a.index as u64).is_multiple_of(p)
                && tick >= 10
            {
                let b = &s.units[ai].aim;
                assert!(
                    d.distance(b.dir) < 1e-4,
                    "BF2: at this think's dir before the next"
                );
            }
            prev = Some(d);
        }
        let limit = match rules {
            // A think turns the block 2 x 5/30 rad: a fifth of it a tick.
            crate::soldier::GroundRules::Bf2 => 2.0 * 5.0 / 30.0 / 5.0 + 1e-3,
            // Two random directions at most half a turn apart, over 10 ticks.
            _ => std::f32::consts::PI / THINK_PERIOD as f32 + 1e-3,
        };
        assert!(worst <= limit, "{rules:?}: a tick turned {worst} rad");
        assert_eq!(moved, 50, "{rules:?}: it turns on every tick");
    }
}

/// Issue #29: the mortar in a bot's hands on the PS2's low-arc solution
/// (`analysis/re-ordnance-kinds.md` §6, re-grenades §7.3) — the shell
/// clears a 1 m wall 10 m out and comes down on the enemy behind it. A
/// flat `seen_aim` shot at the same enemy falls short of him, so the scene
/// fails without the lobbed aim.
#[test]
fn a_bot_lobs_a_mortar_shell_over_a_wall() {
    use crate::collision::{Body, CollisionWorld, Shape};
    use crate::heightfield::Heightfield;
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    use crate::weapon::{ExplosionClass, OrdKind, OrdnanceClass, Thrown, WeaponClass};
    let _stand = crate::ai::tests::all_stand();
    let mut s = Sim::empty(7);
    s.ground_rules = crate::soldier::GroundRules::Bf1;
    s.cheats.infinite_ammo = true;
    s.heightfield = Some(Heightfield {
        size: 2,
        unit: 400.0,
        offset_x: 200.0,
        offset_z: 200.0,
        heights: vec![0.0; 4],
    });
    // A waist-high wall across the way: a flat shot strikes it, the arc
    // clears it. 1 m is well under the shell's line at 10 m.
    s.collision = CollisionWorld::new(
        vec![Body::new(
            glam::Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)),
            Shape::Box {
                half: Vec3::new(10.0, 1.0, 0.25),
            },
        )],
        Vec::new(),
    );
    // The BF1 mortar launcher: a `grenade` class, ShotElevate 0.1,
    // MaxPressedTime 0.75, its `_ord1` shell 40 m/s at Gravity 1.75 x 9.8
    // and a 4 m blast (re-ordnance-kinds §6).
    let mut mortar = WeaponClass::bf1_rifle();
    mortar.trigger_single = true;
    mortar.max_pressed_time = 0.75;
    (
        mortar.spread_per_shot,
        mortar.posture_spread,
        mortar.spread_threshold,
        mortar.spread_limit,
    ) = (0.0, [[0.0; 3]; 3], 0.0, 0.0);
    mortar.shot_delay = [1.4, 1.4];
    mortar.ai_range = [10.0, 32.0, 48.0];
    mortar.thrown = Some(Thrown {
        shot_elevate: 0.1,
        hide_on_fire: true,
        hand: None,
    });
    mortar.ordnance = OrdnanceClass {
        velocity: 40.0,
        life_span: 3.0,
        max_damage: 150.0,
        kind: OrdKind::Shell,
        gravity: 1.75,
        explosion: Some(ExplosionClass {
            damage: 150.0,
            damage_inner: 1.0,
            damage_outer: 4.0,
            ..ExplosionClass::default()
        }),
        ..OrdnanceClass::default()
    };
    let wc = s.add_weapon_class(mortar);
    // Skill 4 draws the charge floor (0.15 x 0.75 s): a tap of the shell
    // stage, not the 0.9 hold that swaps in the sticky (issue #29).
    let shooter = s
        .spawn(&UnitSpec {
            controller: Controller::Ai,
            weapon_class: wc,
            ..UnitSpec::trooper(0, Vec3::ZERO)
        })
        .unwrap();
    s.unit_mut(shooter).unwrap().skill = 4;
    // The enemy 24 m out, behind the wall, with health enough to take the
    // blast and be hurt by it more than once.
    let enemy = s
        .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -24.0)))
        .unwrap();
    s.units[enemy.index as usize].health.max = 1e9;
    s.units[enemy.index as usize].health.cur = 1e9;
    let (mut shots, mut hurt) = (Vec::new(), 0.0f32);
    for _ in 0..900 {
        s.step(&[]);
        for e in &s.events {
            match *e {
                Event::Shot { unit, dir, .. } if unit == shooter => shots.push(dir),
                Event::Hit { victim, damage, .. } if victim == enemy => hurt += damage,
                _ => {}
            }
        }
    }
    assert!(
        shots.len() >= 2,
        "the bot fired the mortar: {}",
        shots.len()
    );
    let dir = shots[0];
    assert!(dir.y > 0.03, "the shell is lobbed, not fired flat: {dir}");
    assert!(
        hurt > 0.0,
        "the shell came down on the enemy behind the wall: {hurt} damage"
    );
}

#[test]
fn a_cone_bot_holds_fire_outside_the_cone_length() {
    let _style = crate::ai::tests::all_stand();
    use crate::sim::{Event, Sim};
    use crate::unit::{Controller, UnitSpec};
    use crate::weapon::{test_props::List, EmitterOrdnance};
    for (distance, should_fire) in [(8.0, true), (13.0, false)] {
        let mut s = Sim::empty(5);
        s.cheats.infinite_ammo = true;
        s.weapon_classes[0].emitter = Some(EmitterOrdnance::from_props(
            &List(&[(2_470_609_998, "10"), (2_750_430_025, "50")]),
            true,
        ));
        let shooter = s
            .spawn(&UnitSpec {
                controller: Controller::Ai,
                ..UnitSpec::trooper(0, Vec3::ZERO)
            })
            .unwrap();
        let target = s
            .spawn(&UnitSpec::trooper(1, Vec3::new(0.0, 0.0, -distance)))
            .unwrap();
        let mut shots = 0;
        for _ in 0..300 {
            s.step(&[PadState::default()]);
            s.unit_mut(target).unwrap().health.cur = 300.0;
            shots += s
                .events
                .iter()
                .filter(|e| matches!(e, Event::Shot { unit, .. } if *unit == shooter))
                .count();
        }
        assert_eq!(shots > 0, should_fire, "distance={distance}, shots={shots}");
    }
}
