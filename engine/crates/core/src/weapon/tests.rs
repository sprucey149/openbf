//! Reference vectors from re-blaster-damage.md §9 (+ the state-machine
//! behaviors read from the decomp bodies).
use super::*;
use std::collections::HashMap;

mod ammo;
mod dispenser_bounds;

#[test]
fn shot_pattern_advances_wraps_and_participates_in_replay_hash() {
    let class = WeaponClass {
        shot_tilts: vec![
            (-0.1 * DEG, -0.8 * DEG),
            (0.8 * DEG, 0.0),
            (-0.1 * DEG, 0.8 * DEG),
        ],
        shots_per_salvo: 3,
        ..WeaponClass::default()
    };
    let mut state = WeaponState::new(&class, true, 0);
    let mut shots = Vec::new();
    state.update(
        DT30,
        &class,
        fi(true, false),
        &mut GameLcg::new(7),
        &mut shots,
    );
    assert_eq!(
        shots.iter().map(|s| s.tilt).collect::<Vec<_>>(),
        class.shot_tilts
    );
    assert_eq!(state.tilt_index, 0);
    let bytes = |state: &WeaponState| {
        let mut out = Vec::new();
        state.hash_into(&mut |b| out.extend_from_slice(b));
        out
    };
    let initial = bytes(&state);
    state.tilt_index = 1;
    assert_ne!(bytes(&state), initial);
}

const DT30: f32 = 1.0 / 30.0;

/// `cheat`: both ammo cheats (nothing drawn, no reload).
fn fi(held: bool, cheat: bool) -> FireInput {
    FireInput {
        held,
        blocked: false,
        infinite_ammo: cheat,
        no_reload: cheat,
        ai_dispersion: None,
    }
}

fn run(w: &mut WeaponState, c: &WeaponClass, dt: f32, ticks: usize, held: bool) -> Vec<f32> {
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    let mut times = Vec::new();
    let mut t = 0.0f32;
    for _ in 0..ticks {
        shots.clear();
        w.update(dt, c, fi(held, false), &mut lcg, &mut shots);
        for _ in &shots {
            times.push(t);
        }
        t += dt;
    }
    times
}

/// §9.1 BF1 bloom: sₙ = −1.6 + 0.14(n−1) deg at shot n; first widened
/// shot n = 13; cap 6.0. Exact with dt = 0.05 (4 updates per 0.2 s).
/// Card SPRD (OURS) puts the floor `tight_hold` = 1.5 s × 0.7°/s = 1.05°
/// lower: sₙ = −2.65 + 0.14(n−1), first widened shot n = 20.
#[test]
fn v1_bf1_spread_bloom_sequence() {
    let c = WeaponClass::bf1_rifle();
    let hold = c.tight_hold(true, 1.0) / DEG;
    assert!((hold - 1.05).abs() < 1e-4, "tight hold {hold} deg");
    let mut w = WeaponState::new(&c, true, 6);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    let mut states = Vec::new();
    for _ in 0..(60 * 20) {
        shots.clear();
        // State sampled at fire time = what the shot used.
        w.update(0.05, &c, fi(true, true), &mut lcg, &mut shots);
        for s in &shots {
            states.push(s.pitch_spread);
        }
    }
    assert!(states.len() >= 68, "{} shots", states.len());
    // Below the cap the sampled state follows the closed form exactly.
    for (i, &disp) in states.iter().enumerate().take(55) {
        let n = i as f32 + 1.0;
        let s_n = -1.6 - hold + 0.14 * (n - 1.0);
        let want = (s_n.max(0.0) + 0.25) * DEG;
        assert!(
            (disp - want).abs() < 2e-3 * DEG,
            "shot {n}: dispersion {} deg, want {} deg",
            disp / DEG,
            want / DEG
        );
    }
    // Shot 19 still at the floor, shot 20 first widened (the PS2's: 12, 13).
    assert!((states[18] - 0.25 * DEG).abs() < 1e-3 * DEG);
    assert!(states[19] > 0.25 * DEG && states[19] < 0.36 * DEG);
    // At the cap: growth is clamped to SpreadLimit AFTER the shot and
    // recovery runs before the next one, so the state sampled at fire
    // time plateaus at 6.0 − 4.8×0.2 = 5.04 (dispersion 5.29°).
    for &disp in &states[57..68] {
        assert!(
            (disp - 5.29 * DEG).abs() < 2e-3 * DEG,
            "plateau {}",
            disp / DEG
        );
    }
    // A salvo the refire branch started with no dt left still fires on
    // the first released update (only holstering cancels it) — let it
    // resolve, then measure recovery to −1.6 at 4.8°/s (from the cap
    // itself it would be 7.6/4.8 = 1.583 s).
    w.update(0.05, &c, fi(false, true), &mut lcg, &mut shots);
    let start = w.spread;
    assert!(
        start > 5.0 * DEG && start <= 6.0 * DEG,
        "end state {}",
        start / DEG
    );
    let mut t = 0.0f32;
    while w.spread > -(1.6 + hold) * DEG + 1e-6 {
        w.update(0.05, &c, fi(false, true), &mut lcg, &mut shots);
        t += 0.05;
    }
    let want = (start + (1.6 + hold) * DEG) / (4.8 * DEG);
    assert!((t - want).abs() < 0.06, "recovery took {t}, want {want}");
}

/// §9.2 BF2 bloom: sₙ = −1.6 + 0.24(n−1); first widened shot n = 8; cap 5.0.
#[test]
fn v2_bf2_spread_bloom() {
    let c = WeaponClass::bf2_rifle();
    let mut w = WeaponState::new(&c, true, 6);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    let mut states = Vec::new();
    for _ in 0..(50 * 4) {
        shots.clear();
        w.update(0.05, &c, fi(true, true), &mut lcg, &mut shots);
        for s in &shots {
            states.push(s.yaw_spread);
        }
    }
    // Card SPRD (OURS): the floor is 1.5 s × 1.2°/s = 1.8° lower, sₙ =
    // −3.4 + 0.24(n−1): shot 15 at the floor, shot 16 the first widened.
    assert!((c.tight_hold(true, 1.0) / DEG - 1.8).abs() < 1e-4);
    assert!(
        (states[14] - 0.25 * DEG).abs() < 1e-3 * DEG,
        "shot 15 at floor"
    );
    assert!(
        (states[15] - 0.45 * DEG).abs() < 2e-3 * DEG,
        "shot 16 widened: {}",
        states[15] / DEG
    );
    // Cap: 5.0 − 4.8×0.2 = 4.04 sampled at fire time → dispersion 4.29°.
    assert!(
        (states[43] - 4.29 * DEG).abs() < 2e-3 * DEG,
        "capped at 4.29: {}",
        states[43] / DEG
    );
}

/// Card SPRD's acceptance (OURS, the maintainer 2026-10-04): held fire keeps its
/// tight spread 1-2 s longer than the PS2's rule before it opens, for the
/// player and for a bot, then opens at the PS2's own rate. The PS2's first
/// widened shot is the closed form of §9.1 / §9.2 (a human's sₙ = −1.6 +
/// g(n−1), a bot's g(n−1), g = SpreadPerShot − 0.2 s × SpreadRecoverRate).
#[test]
fn held_fire_stays_tight_one_to_two_seconds_longer() {
    for (c, g) in [
        (WeaponClass::bf1_rifle(), 0.14f32),
        (WeaponClass::bf2_rifle(), 0.24),
    ] {
        for human in [true, false] {
            let mut w = WeaponState::new(&c, human, 6);
            let mut lcg = GameLcg::new(1);
            let (mut shots, mut states) = (Vec::new(), Vec::new());
            for _ in 0..400 {
                shots.clear();
                w.update(0.05, &c, fi(true, true), &mut lcg, &mut shots);
                states.extend(shots.iter().map(|s| s.pitch_spread));
            }
            let tight = c.pitch_spread + 1e-3 * DEG;
            let first = states.iter().position(|&s| s > tight).unwrap();
            // The PS2's: the first n (0-based) with floor + g·n > 0.
            let floor = if human { -1.6f32 } else { 0.0 };
            let ps2 = (0..).find(|&n| floor + g * n as f32 > 1e-3).unwrap();
            let longer = (first - ps2) as f32 * 0.2;
            assert!(
                (1.0..=2.0).contains(&longer),
                "human {human}: first widened shot {first} (PS2 {ps2}), {longer} s longer"
            );
            // ... and from there it opens shot by shot as the PS2's does.
            let step = (states[first + 5] - states[first + 4]) / DEG;
            assert!((step - g).abs() < 2e-3, "growth {step} deg / shot");
        }
    }
}

/// §9.3 cadence: first shot on press, then every ShotDelay; 15 shots in
/// [0, 2.967 s) at 30 Hz; two shots in one 0.4 s update.
#[test]
fn v3_refire_cadence_is_drift_free() {
    let c = WeaponClass::bf1_rifle();
    let mut w = WeaponState::new(&c, true, 6);
    let times = run(&mut w, &c, DT30, 89, true);
    assert_eq!(times.len(), 15, "{times:?}");
    assert_eq!(times[0], 0.0, "first shot immediate on press");
    for (k, t) in times.iter().enumerate() {
        assert!((t - 0.2 * k as f32).abs() < DT30 + 1e-4, "shot {k} at {t}");
    }
    // Big dt: catch-up shots inside one update.
    let mut w = WeaponState::new(&c, true, 6);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    w.update(0.4, &c, fi(true, false), &mut lcg, &mut shots);
    // Trigger shot + one refire period; the salvo started by the refire
    // branch with no dt left fires at the start of the NEXT update (the
    // decomp loop breaks on dt_left <= eps right after starting it).
    assert_eq!(shots.len(), 2, "trigger shot + 1 refire period in 0.4 s");
    shots.clear();
    w.update(0.001, &c, fi(true, false), &mut lcg, &mut shots);
    assert_eq!(shots.len(), 1, "pending salvo fires immediately");
    assert_eq!(w.clip, 47);
}

/// §9.4 BF2 rifle damage vs distance (d = 160·age).
#[test]
fn v4_bf2_damage_falloff_by_age() {
    let o = WeaponClass::bf2_rifle().ordnance;
    let at = |d: f32| o.damage_desc(d / 160.0, false).damage;
    for (d, want) in [
        (10.0, 80.0),
        (64.0, 80.0),
        (72.0, 72.5),
        (80.0, 65.0),
        (88.0, 57.5),
        (96.0, 50.0),
        (120.0, 50.0),
    ] {
        assert!((at(d) - want).abs() < 1e-3, "d={d}: {} != {want}", at(d));
    }
    assert_eq!(o.max_range(), 128.0);
    // Team bonus: +25% at close range.
    assert!((o.damage_desc(0.1, true).damage - 100.0).abs() < 1e-4);
    // BF1: flat at any age.
    let b = WeaponClass::bf1_rifle().ordnance;
    assert_eq!(b.damage_desc(1.4, false).damage, 60.0);
    assert_eq!(b.max_range(), 120.0);
}

/// §9.6 TTK: BF1 rifle vs 300 HP person = 5 shots; building type takes 6/shot.
#[test]
fn v6_ttk_bf1_rifle_vs_trooper() {
    let o = WeaponClass::bf1_rifle().ordnance;
    let mut h = Health::new(300.0, HealthType::Person);
    let mut n = 0;
    while h.alive() {
        h.take_damage(&o.damage_desc(0.25, false), 1.0, false, 1.0);
        n += 1;
    }
    assert_eq!(n, 5);
    let mut b = Health::new(300.0, HealthType::Building);
    let applied = b.take_damage(&o.damage_desc(0.25, false), 1.0, false, 1.0);
    assert!((applied - 6.0).abs() < 1e-5);
    // Vehicles are immune to the BF1 rifle (VehicleScale 0).
    let mut v = Health::new(1000.0, HealthType::Vehicle);
    assert_eq!(
        v.take_damage(&o.damage_desc(0.25, false), 1.0, false, 1.0),
        0.0
    );
    // Friendly fire scale applies before the type scale.
    let mut f = Health::new(300.0, HealthType::Person);
    assert!((f.take_damage(&o.damage_desc(0.0, false), 1.0, true, 0.5) - 30.0).abs() < 1e-5);
}

/// §9.7 triangular dispersion is seed-reproducible and bounded; the
/// per-shot LCG consumption is exactly 8 steps (4 pitch + 4 yaw).
#[test]
fn v7_shot_direction_reproducible_and_bounded() {
    let mut a = GameLcg::new(0xBEEF);
    let mut b = GameLcg::new(0xBEEF);
    let d1 = shot_direction(&mut a, Vec3::NEG_Z, Vec3::X, Vec3::Y, 0.1, 0.1, (0.0, 0.0));
    let d2 = shot_direction(&mut b, Vec3::NEG_Z, Vec3::X, Vec3::Y, 0.1, 0.1, (0.0, 0.0));
    assert_eq!(d1, d2);
    assert!((d1.length() - 1.0).abs() < 1e-6);
    // Steps consumed: 8.
    let mut c = GameLcg::new(0xBEEF);
    for _ in 0..8 {
        c.step();
    }
    assert_eq!(a.state(), c.state());
    // Zero spread → exactly forward; offsets never exceed the half-angle.
    let mut z = GameLcg::new(3);
    let d = shot_direction(&mut z, Vec3::NEG_Z, Vec3::X, Vec3::Y, 0.0, 0.0, (0.0, 0.0));
    assert!((d - Vec3::NEG_Z).length() < 1e-6);
    let mut g = GameLcg::new(99);
    for _ in 0..500 {
        let d = shot_direction(&mut g, Vec3::NEG_Z, Vec3::X, Vec3::Y, 0.05, 0.1, (0.0, 0.0));
        let ang = d.angle_between(Vec3::NEG_Z);
        assert!(
            ang <= (0.05f32.powi(2) + 0.1f32.powi(2)).sqrt() + 1e-4,
            "{ang}"
        );
    }
}

/// TriggerSingle: one salvo per fresh press, at most one per ShotDelay.
#[test]
fn trigger_single_fires_once_per_press_rate_limited() {
    let mut c = WeaponClass::bf1_rifle();
    c.trigger_single = true;
    let mut w = WeaponState::new(&c, true, 6);
    // Held for 1 s: exactly one shot.
    let times = run(&mut w, &c, DT30, 30, true);
    assert_eq!(times.len(), 1);
    // Tap-tap-tap every 2 ticks (0.067 s): shots only every ≥ 0.2 s.
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    let mut fired = 0;
    for i in 0..60 {
        shots.clear();
        w.update(DT30, &c, fi(i % 2 == 0, false), &mut lcg, &mut shots);
        fired += shots.len();
    }
    assert!((9..=10).contains(&fired), "2 s of tapping -> {fired} shots");
}

/// Card CHEAT2: `ammo` keeps the reserve and reloads for free (even from an
/// empty reserve); `clip` keeps the clip full while the reserve pays, then
/// empties it with no reload.
#[test]
fn ammo_and_clip_cheats_split() {
    let c = WeaponClass::bf1_rifle();
    let input = |infinite_ammo, no_reload| FireInput {
        held: true,
        infinite_ammo,
        no_reload,
        ..Default::default()
    };
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    // `ammo`, from a 50-round clip and no reserve.
    let mut w = WeaponState::new(&c, true, 0);
    let mut reloaded = false;
    for _ in 0..30 * 20 {
        w.update(DT30, &c, input(true, false), &mut lcg, &mut shots);
        assert_eq!(w.spare, 0);
        reloaded |= w.reloading();
    }
    assert!(reloaded && shots.len() > 60, "{} shots", shots.len());
    // `clip`, 50 + 100.
    let mut w = WeaponState::new(&c, true, 2);
    shots.clear();
    for _ in 0..30 * 40 {
        w.update(DT30, &c, input(false, true), &mut lcg, &mut shots);
        assert!(!w.reloading());
        if w.spare > 0 {
            assert_eq!(w.clip, 50);
        }
    }
    assert_eq!((w.clip, w.spare, shots.len()), (0, 0, 150));
}

/// Ammo: 50-round clip empties after 50 shots, auto-reload takes 1.5 s,
/// then firing resumes; infinite-ammo cheat never consumes.
#[test]
fn clip_reload_and_infinite_ammo_cheat() {
    let c = WeaponClass::bf1_rifle();
    let mut w = WeaponState::new(&c, true, 6);
    assert_eq!((w.clip, w.spare), (50, 300));
    let times = run(&mut w, &c, DT30, 30 * 13, true);
    assert!(times.len() >= 50 + 8, "{} shots", times.len());
    assert!((times[49] - 9.8).abs() < 0.05);
    assert!(
        times[50] >= 11.5 && times[50] < 11.6,
        "resume at {}",
        times[50]
    );
    assert_eq!(w.spare, 250);

    let mut w2 = WeaponState::new(&c, true, 0);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    for _ in 0..(30 * 12) {
        w2.update(DT30, &c, fi(true, true), &mut lcg, &mut shots);
    }
    assert_eq!(w2.clip, 50, "cheat leaves the clip full");
    assert_eq!(shots.len(), 60);
}

/// Human vs AI spread floor; posture lerp Still→Move by move magnitude.
#[test]
fn spread_floor_and_posture_lerp() {
    let mut c = WeaponClass::bf1_rifle();
    c.posture_spread[1] = [1.0 * DEG, 3.0 * DEG, 0.5 * DEG];
    let h = WeaponState::new(&c, true, 0);
    let ai = WeaponState::new(&c, false, 0);
    // The PS2's floors, each less the port's tight hold (card SPRD).
    assert!((h.spread + 1.6 * DEG + c.tight_hold(true, 1.0)).abs() < 1e-7);
    assert!((ai.spread + c.tight_hold(false, 1.0)).abs() < 1e-7);
    let mut w = h.clone();
    w.set_posture(&c, 1, 0.5, false);
    assert!((w.posture_base - 2.0 * DEG).abs() < 1e-7);
    w.set_posture(&c, 1, 0.0, true);
    assert!((w.posture_base - 0.5 * DEG).abs() < 1e-7);
    w.set_posture(&c, 7, 1.0, false); // stance > 2 → stand row (all 0)
    assert_eq!(w.posture_base, 0.0);
}

/// Heat model: HeatPerShot 0.5 → shots at 0/0.2/0.4 reach heat 1.3 →
/// overheated; the 4th waits until heat < HeatThreshold (0.3) at
/// 0.5/s = 2.0 s later.
#[test]
fn heat_overheats_and_recovers() {
    let mut c = WeaponClass::bf1_rifle();
    c.heat_per_shot = 0.5;
    c.heat_recover_rate = 0.5;
    c.heat_threshold = 0.3;
    let mut w = WeaponState::new(&c, true, 6);
    let times = run(&mut w, &c, DT30, 100, true);
    // 0 / 0.2 / 0.4 → heat 1.3, overheated; ~2.43 once heat < 0.3
    // (→ 0.8); 2.63 (→ 1.2, overheated again).
    assert_eq!(times.len(), 5, "{times:?}");
    assert!((times[2] - 0.4).abs() < DT30);
    assert!(
        times[3] - times[2] > 1.9 && times[3] - times[2] < 2.2,
        "overheat gap {times:?}"
    );
    assert!((times[4] - times[3] - 0.2).abs() < DT30);
}

/// LEAD ADDENDUM 4b's synthetic subject: a class that passes all three
/// gates of [`WeaponClass::ai_distance_table`].
fn ai_sniper_class() -> WeaponClass {
    WeaponClass {
        trigger_single: true,
        zoom_max: 8.0,
        yaw_spread: 0.0,
        ..WeaponClass::default()
    }
}

/// ADDENDUM 4b (iii): a STEP function on the SQUARED distance with strict
/// `<` and no interpolation (`re-ai-sniper.md` §2, B2:323373-323398).
#[test]
fn ai_sniper_table_steps_on_squared_distance() {
    assert_eq!(ai_sniper_yaw(0.0), 0.0997);
    assert_eq!(ai_sniper_yaw(399.9), 0.0997);
    assert_eq!(ai_sniper_yaw(400.0), 0.0499);
    assert_eq!(ai_sniper_yaw(1599.9), 0.0499);
    assert_eq!(ai_sniper_yaw(1600.0), 0.0333);
    assert_eq!(ai_sniper_yaw(3600.0), 0.025);
    assert_eq!(ai_sniper_yaw(6400.0), 0.02);
    assert_eq!(ai_sniper_yaw(10000.0), 0.0167);
    assert_eq!(ai_sniper_yaw(14400.0), 0.0143);
    assert_eq!(ai_sniper_yaw(1.0e9), AI_SNIPER_DISPERSION_FAR);
    // The edges are 20/40/60/80/100/120 m.
    for (m, want) in [
        (19.9f32, 0.0997),
        (20.0, 0.0499),
        (59.9, 0.0333),
        (119.9, 0.0167),
        (120.0, 0.0143),
    ] {
        assert_eq!(ai_sniper_yaw(m * m), want, "{m} m");
    }
}

/// ADDENDUM 4b (ii): the class half of the PS2 predicate — all three
/// gates, and `>=` on `ZoomMax` (`imp_weap_inf_sniperrifle` ships 6.0).
#[test]
fn ai_distance_table_needs_all_three_class_gates() {
    assert!(ai_sniper_class().ai_distance_table());
    assert!(WeaponClass {
        zoom_max: 6.0,
        ..ai_sniper_class()
    }
    .ai_distance_table());
    assert!(!WeaponClass {
        zoom_max: 5.99,
        ..ai_sniper_class()
    }
    .ai_distance_table());
    assert!(!WeaponClass {
        yaw_spread: 0.25 * DEG,
        ..ai_sniper_class()
    }
    .ai_distance_table());
    assert!(!WeaponClass {
        trigger_single: false,
        ..ai_sniper_class()
    }
    .ai_distance_table());
    // The ctor default keeps every ordinary class out.
    assert_eq!(WeaponClass::default().zoom_max, 1.0);
    assert!(!WeaponClass::default().ai_distance_table());
    assert!(!WeaponClass::bf1_rifle().ai_distance_table());
}

/// The loaders coerce a written `ZoomMax 0` to 1.0 (`re-ai-sniper.md` §2
/// key table, B2:320352-320361 / B1:209974-209983).
#[test]
fn zoom_max_loads_with_the_zero_coercion() {
    let zm = |v: &str| {
        WeaponClass::from_props("w", &src(&[("ZoomMax", v)], &[]), OrdnanceClass::default())
            .zoom_max
    };
    assert_eq!(zm("8.0"), 8.0);
    assert_eq!(zm("6.0"), 6.0);
    assert_eq!(zm("0"), 1.0);
    assert_eq!(
        WeaponClass::from_props("w", &src(&[], &[]), OrdnanceClass::default()).zoom_max,
        1.0
    );
}

/// Card CLSE2 (`analysis/re-soldier-ai.md` 18.4): the three AIBubble keys
/// and the constructor's 1.0 / 20 / 10.
#[test]
fn the_ai_bubble_keys_load_with_their_defaults() {
    let w = |pairs: &[(&str, &str)]| {
        let w = WeaponClass::from_props("w", &src(pairs, &[]), OrdnanceClass::default());
        (w.ai_bubble_size, w.ai_bubble_dist, w.ai_bubble_clamp)
    };
    assert_eq!(w(&[]), (1.0, 20.0, 10.0));
    assert_eq!(w(&[("AIBubbleSizeMultiplier", "1.25")]), (1.25, 20.0, 10.0));
    assert_eq!(
        w(&[
            ("AIBubbleSizeMultiplier", "0.6"),
            ("AIBubbleScaleDistDivider", "100"),
            ("AIBubbleScaleClamp", "3.0"),
        ]),
        (0.6, 100.0, 3.0)
    );
}

/// ADDENDUM 4b (i) and (5): the table value enters the `YawSpread` SLOT
/// scaled by the skill's `dispersion_mult`, ON TOP of
/// `max(spread, 0) + postureBase` — which the PS2 adds on every branch
/// (B2:323416). Pitch is untouched.
#[test]
fn ai_sniper_dispersion_reaches_the_weapon_yaw_slot() {
    let c = ai_sniper_class();
    // 30 m → d² 900, inside the 40 m band.
    let table = ai_sniper_yaw(30.0 * 30.0);
    assert_eq!(table, 0.0499);
    for (level, mult) in [
        (4u8, 1.684_210_5_f32),
        (8, 1.263_157_9),
        (17, 0.315_789_46),
        (20, 0.0),
    ] {
        let s = crate::skill::Skill::from_level(level);
        assert!(
            (s.dispersion_mult - mult).abs() < 1e-6,
            "skill {level} mult {}",
            s.dispersion_mult
        );
        let want = table * s.dispersion_mult;
        let mut w = WeaponState::new(&c, false, 0);
        w.spread = 0.02;
        w.posture_base = 0.01;
        let mut lcg = GameLcg::new(1);
        let mut shots = Vec::new();
        w.update(
            DT30,
            &c,
            FireInput {
                held: false,
                blocked: false,
                infinite_ammo: true,
                no_reload: true,
                ai_dispersion: Some(want),
            },
            &mut lcg,
            &mut shots,
        );
        // `update` is what copies the AI's value onto the instance, as
        // the PS2's aim tick writes `weapon+0x138`.
        assert_eq!(w.ai_dispersion, Some(want));
        assert_eq!(w.yaw_spread(&c), w.spread.max(0.0) + w.posture_base + want);
        assert_eq!(
            w.pitch_spread(&c),
            w.spread.max(0.0) + w.posture_base + c.pitch_spread
        );
    }
    // No AI value (a human, or a bot with no target): the class
    // `YawSpread`, exactly as before Phase C.
    let c2 = WeaponClass {
        yaw_spread: 0.25 * DEG,
        ..WeaponClass::default()
    };
    let mut w = WeaponState::new(&c2, false, 0);
    let mut lcg = GameLcg::new(1);
    let mut shots = Vec::new();
    w.update(DT30, &c2, fi(false, true), &mut lcg, &mut shots);
    assert_eq!(w.ai_dispersion, None);
    assert_eq!(w.yaw_spread(&c2), c2.yaw_spread);
}

struct MapSrc(HashMap<String, String>, HashMap<u32, String>);
impl PropSource for MapSrc {
    fn prop(&self, key: &str) -> Option<&str> {
        self.0.get(&key.to_ascii_lowercase()).map(String::as_str)
    }
    fn prop_hash(&self, hash: u32) -> Option<&str> {
        self.1.get(&hash).map(String::as_str)
    }
}
fn src(pairs: &[(&str, &str)], hashed: &[(u32, &str)]) -> MapSrc {
    MapSrc(
        pairs
            .iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
            .collect(),
        hashed.iter().map(|(h, v)| (*h, v.to_string())).collect(),
    )
}

/// ODF strings (report §8 dumps) parse to the same structs as the fixtures.
#[test]
fn from_props_reproduces_rifle_fixtures() {
    let ord = OrdnanceClass::from_props(&src(
        &[
            ("Velocity", "80.0"),
            ("LifeSpan", "1.5"),
            ("Gravity", "1.0"),
            ("MaxDamage", "60.0"),
            ("PersonScale", "1.0"),
            ("AnimalScale", "1.0"),
            ("DroidScale", "1.0"),
            ("VehicleScale", "0.0"),
            ("BuildingScale", "0.1"),
            ("ShieldScale", "1.0"),
            ("OrdnanceSound", "com_weap_inf_ord_hum_sm"),
            ("CollisionSound", "imp_weap_ord_exp"),
        ],
        &[],
    ));
    let w = WeaponClass::from_props(
        "all_weap_inf_rifle",
        &src(
            &[
                ("OrdnanceName", "all_weap_inf_rifle_ord"),
                ("ShotDelay", "0.2"),
                ("RoundsPerClip", "50"),
                ("ReloadTime", "1.5"),
                ("YawSpread", "0.25"),
                ("PitchSpread", "0.25"),
                ("SpreadPerShot", "1.1"),
                ("SpreadRecoverRate", "4.8"),
                ("SpreadThreshold", "1.6"),
                ("SpreadLimit", "6.0"),
                ("LockOnRange", "64.0"),
                // The AI's range triple (card FGHT).
                ("MinRange", "0"),
                ("OptimalRange", "16"),
                ("MaxRange", "48"),
                ("TargetEnemy", "1"),
                ("TargetNeutral", "0"),
                ("TargetFriendly", "0"),
                ("TargetPerson", "1"),
                ("TargetAnimal", "1"),
                ("TargetDroid", "1"),
                ("TargetVehicle", "0"),
                ("TargetBuilding", "0"),
                ("LockOnAngle", "1.0"),
                ("TriggerSingle", "0"),
                // Both rifles ship ZoomMin/ZoomMax 2.5 (`re-ai-sniper.md`
                // §3).
                ("ZoomMin", "2.5"),
                ("ZoomMax", "2.5"),
                ("StandStillSpread", "0.0"),
                ("FireSound", "all_weap_inf_rifle_fire"),
                ("ReloadSound", "all_weap_inf_reload_med"),
                ("FireEmptySound", "com_weap_inf_ammo_empty"),
                ("WeaponChangeSound", "all_weap_inf_equip_med"),
            ],
            &[],
        ),
        ord,
    );
    // The laser look and the muzzle flash are replayed in PROP order by
    // the class loader (`game::roster::laser_look` / `flash_look`), not
    // by `from_props`.
    let mut want = WeaponClass::bf1_rifle();
    want.ordnance.laser = None;
    want.ordnance.impact = None;
    want.flash = None;
    assert_eq!(w, want);

    let ord2 = OrdnanceClass::from_props(&src(
        &[
            ("Velocity", "160.0"),
            ("LifeSpan", "0.8"),
            ("MaxDamage", "80.0"),
            ("PersonScale", "1.0"),
            ("AnimalScale", "1.0"),
            ("DroidScale", "1.0"),
            ("VehicleScale", "0.1"),
            ("BuildingScale", "0.1"),
            ("ShieldScale", "1.0"),
            ("OrdnanceSound", "com_weap_inf_ord_hum_sm"),
            ("CollisionSound", "imp_weap_ord_exp"),
        ],
        &[
            (HASH_FALLOFF_START, "0.4"),
            (HASH_FALLOFF_DURATION, "0.2"),
            (HASH_FALLOFF_END_DAMAGE, "50.0"),
            (HASH_TEAM_BONUS_SCALE, "0.25"),
        ],
    ));
    let w2 = WeaponClass::from_props(
        "com_weap_inf_rifle",
        &src(
            &[
                ("ShotDelay", "0.2"),
                ("RoundsPerClip", "50"),
                ("ReloadTime", "1.5"),
                ("YawSpread", "0.25"),
                ("PitchSpread", "0.25"),
                ("SpreadPerShot", "1.2"),
                ("SpreadRecoverRate", "4.8"),
                ("SpreadThreshold", "1.6"),
                ("SpreadLimit", "5.0"),
                ("OrdnanceName", "com_weap_inf_rifle_ord"),
                ("LockOnRange", "40.0"),
                ("MinRange", "0"),
                ("OptimalRange", "24"),
                ("MaxRange", "64"),
                ("TargetEnemy", "1"),
                ("TargetNeutral", "0"),
                ("TargetFriendly", "0"),
                ("TargetPerson", "1"),
                ("TargetAnimal", "1"),
                ("TargetDroid", "1"),
                ("TargetVehicle", "0"),
                ("TargetBuilding", "0"),
                ("LockOnAngle", "1.0"),
                ("ZoomMin", "2.5"),
                ("ZoomMax", "2.5"),
                ("FireSound", "rep_weap_inf_rifle_fire"),
                ("ReloadSound", "com_weap_inf_reload_med"),
                ("FireEmptySound", "com_weap_inf_ammo_empty"),
                ("WeaponChangeSound", "com_weap_inf_equip_med"),
                ("ScoreForMedalsType", "1"),
            ],
            &[],
        ),
        ord2,
    );
    // The laser look and the muzzle flash are replayed in PROP order by
    // the class loader (`game::roster::laser_look` / `flash_look`), not
    // by `from_props`.
    let mut want = WeaponClass::bf2_rifle();
    want.ordnance.laser = None;
    want.ordnance.impact = None;
    want.flash = None;
    assert_eq!(w2, want);

    // Legacy aliases + pairs + clamps.
    let w3 = WeaponClass::from_props(
        "x",
        &src(
            &[
                ("MaxSpread", "2.0"),
                ("ShotDelay", "0.001 0.3"),
                ("SalvoDelay", "0.1 0.2"),
                ("RoundsPerClip", "0"),
            ],
            &[],
        ),
        OrdnanceClass::default(),
    );
    assert_eq!((w3.pitch_spread, w3.yaw_spread), (0.05, 0.1));
    assert_eq!(w3.shot_delay, [0.01, 0.3]);
    assert_eq!(w3.salvo_delay, [0.1, 0.2]);
    assert!(w3.infinite_ammo());
    assert_eq!(HealthType::parse("Person"), Some(HealthType::Person));
}

/// TargetAll precedes per-kind overrides; AI inherits the resulting mask
/// before its own overrides. Missing properties retain the defaults.
#[test]
fn optional_target_properties_keep_inheritance_and_override_order() {
    let defaults = WeaponClass::from_props("w", &src(&[], &[]), OrdnanceClass::default());
    let w = WeaponClass::from_props(
        "w",
        &src(
            &[
                ("TargetEnemy", "0"),
                ("TargetNeutral", "2"),
                ("TargetAll", "0"),
                ("TargetPerson", "3"),
                ("TargetVehicle", "0"),
                ("AITargetPerson", "0"),
                ("AITargetBuilding", "-2"),
            ],
            &[],
        ),
        OrdnanceClass::default(),
    );
    assert_eq!(w.target_team, (defaults.target_team & !1) | 2);
    assert_eq!(w.target_kind, 1);
    assert_eq!(w.ai_target_kind, 1 << 4);
    assert_eq!(defaults.ai_target_kind, defaults.target_kind);
}

/// BF2 FUN_001dfb38 (B2:91346-91356) and BF1 FUN_001bc190 (B1:64364-64379):
/// a shield is charged only for a positive shield-scaled hit, so a weapon whose
/// ShieldScale is 0 leaves the shield and goes straight to health.
#[test]
fn a_shield_scale_of_0_leaves_the_shield_and_hurts_health() {
    let mut h = Health::new(100.0, HealthType::Person);
    h.shield = 50.0;
    h.shield_max = 50.0;
    let desc = DamageDesc {
        damage: 30.0,
        scales: [1.0; 7],
        shield_scale: 0.0,
    };
    assert_eq!(h.take_damage(&desc, 1.0, false, 1.0), 30.0);
    assert_eq!((h.cur, h.shield), (70.0, 50.0));
    let charged = DamageDesc {
        shield_scale: 1.0,
        ..desc
    };
    assert_eq!(h.take_damage(&charged, 1.0, false, 1.0), 0.0);
    assert_eq!((h.cur, h.shield), (70.0, 20.0));
}

/// Issue #29 (`analysis/re-weapon-charge.md` §4): an ordnance's charge points
/// blend by the charge's place in its segment - BF2's Hoth dish turret
/// (Damage 1000 / 1250 / 1500 / 3000 at 0 / 0.5 / 0.75 / 2.0) reaches 1800 at
/// a full charge - and an emitter's damage runs Damage -> MaxDamage by the
/// raw charge.
#[test]
fn ordnance_charge_points_blend_by_the_charge() {
    let knot = |at: f32, damage: f32| OrdKnot {
        at,
        velocity: 600.0,
        life_span: 1.0,
        damage,
        scales: [1.0; 7],
        shield_scale: 1.0,
    };
    let dish = OrdnanceClass {
        velocity: 600.0,
        life_span: 1.0,
        max_damage: 1000.0,
        knots: vec![knot(0.5, 1250.0), knot(0.75, 1500.0), knot(2.0, 3000.0)],
        ..Default::default()
    };
    let at = |c: f32| dish.charged(c).damage;
    assert_eq!((at(0.0), at(0.25), at(0.5)), (1000.0, 1125.0, 1250.0));
    assert!((at(0.6) - 1350.0).abs() < 1e-3, "{}", at(0.6));
    assert!((at(1.0) - 1800.0).abs() < 1e-3, "{}", at(1.0));
    assert!((dish.damage_desc_charged(0.0, false, 1.0).damage - 1800.0).abs() < 1e-3);
    let arc = OrdnanceClass {
        max_damage: 325.0,
        min_damage: Some(300.0),
        ..Default::default()
    };
    assert!((arc.charged(0.4).damage - 310.0).abs() < 1e-3);
    assert_eq!(arc.charged(1.0).damage, 325.0);
    // No points, no emitter: the class's own numbers at any charge.
    let rifle = WeaponClass::bf1_rifle().ordnance;
    assert_eq!(rifle.charged(0.3).damage, rifle.max_damage);
}

/// A disc sniper as the loader parses it (BF1's: LockOnRange 0, LockOnAngle
/// 1, MaxRange 128, a 2000 m/s bolt for 0.15 s), its round's VehicleScale
/// `vehicle_scale` (BF1 0, BF2 0.1).
pub(crate) fn disc_sniper(vehicle_scale: f32) -> WeaponClass {
    let mut w = WeaponClass {
        name: "all_weap_inf_sniperrifle".into(),
        trigger_single: true,
        zoom_max: 8.0,
        zoom_min: 2.0,
        zoom_turn_div: [2.0, 8.0],
        zoom_first_person: true,
        lock_on_range: 0.0,
        ai_range: [20.0, 64.0, 128.0],
        extreme_range: 300.0,
        yaw_spread: 0.0,
        pitch_spread: 0.0,
        ..WeaponClass::bf1_rifle()
    };
    w.ordnance.velocity = 2000.0;
    w.ordnance.life_span = 0.15;
    w.ordnance.scales[3] = vehicle_scale;
    w
}

/// OURS (sniper aim assist): a sniper rifle gets its MaxRange as the reach,
/// keeps its 1° cone, and takes vehicles only when its round hurts them.
#[test]
fn a_sniper_rifle_gets_a_narrow_magnet_to_its_max_range() {
    let mut bf1 = disc_sniper(0.0);
    assert!(bf1.is_sniper_rifle());
    bf1.give_sniper_assist();
    assert_eq!(
        (bf1.lock_on_range, bf1.lock_on_angle, bf1.extreme_range),
        (128.0, DEG, 300.0)
    );
    assert_eq!((bf1.target_team, bf1.target_kind & 0x1f), (1, 0b111));
    let mut bf2 = disc_sniper(0.1);
    bf2.give_sniper_assist();
    assert_eq!(bf2.target_kind & 0x1f, 0b1111);
    // The AI's kinds are its own.
    assert_eq!(bf2.ai_target_kind, disc_sniper(0.1).ai_target_kind);
    // No MaxRange, no LockOnAngle, a short bolt: the fallbacks, and the
    // cone at least as long as the reach.
    let mut bare = WeaponClass {
        ai_range: [0.0; 3],
        lock_on_angle: 0.0,
        extreme_range: 22.5,
        ..disc_sniper(0.0)
    };
    bare.give_sniper_assist();
    assert_eq!(
        (bare.lock_on_range, bare.lock_on_angle, bare.extreme_range),
        (
            SNIPER_LOCK_ON_RANGE,
            SNIPER_LOCK_ON_ANGLE,
            SNIPER_LOCK_ON_RANGE
        )
    );
}

/// Every other weapon is left as parsed: the rifles, a scope weapon outside
/// the sniper table (the bowcaster: ZoomMax 5), an unscoped one in it, and a
/// sniper that authors its own LockOnRange.
#[test]
fn give_sniper_assist_leaves_other_weapons_alone() {
    let bowcaster = WeaponClass {
        zoom_max: 5.0,
        ..disc_sniper(0.0)
    };
    let unscoped = WeaponClass {
        zoom_first_person: false,
        ..disc_sniper(0.0)
    };
    let authored = WeaponClass {
        lock_on_range: 40.0,
        ..disc_sniper(0.0)
    };
    for w in [
        WeaponClass::bf1_rifle(),
        WeaponClass::bf2_rifle(),
        bowcaster,
        unscoped,
        authored,
    ] {
        let mut after = w.clone();
        after.give_sniper_assist();
        assert_eq!(after, w, "{}", w.name);
    }
}

/// One update of a pressed-or-not trigger: whether it clicked, and its shots.
fn press_update(w: &mut WeaponState, c: &WeaponClass, input: FireInput) -> (bool, usize) {
    let mut shots = Vec::new();
    w.update(DT30, c, input, &mut GameLcg::new(7), &mut shots);
    (w.press_clicks(c, input, !shots.is_empty()), shots.len())
}

/// A rifle at 0.5 heat a shot, shot until it overheats and its cycle ends.
fn overheated_rifle() -> (WeaponClass, WeaponState) {
    let mut c = WeaponClass::bf1_rifle();
    c.heat_per_shot = 0.5;
    c.heat_recover_rate = 0.1;
    c.heat_threshold = 0.3;
    let mut w = WeaponState::new(&c, true, 6);
    run(&mut w, &c, DT30, 20, true);
    run(&mut w, &c, DT30, 12, false);
    assert!(w.overheated && w.state == FireState::Idle);
    (c, w)
}

/// §7.3: each press on an overheated weapon clicks once, however long it is
/// held, and the press edge is the only limit on how often.
#[test]
fn an_overheated_weapon_clicks_each_press_once() {
    let (c, mut w) = overheated_rifle();
    for _ in 0..3 {
        assert_eq!(press_update(&mut w, &c, fi(true, false)), (true, 0));
        for _ in 0..14 {
            assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 0));
        }
        assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
        assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
    }
}

/// A cool weapon's press fires, and a press that cannot fire for a reason of
/// the soldier's, or on a weapon not selected, is no refusal of the weapon's.
#[test]
fn only_a_press_the_weapon_refuses_clicks() {
    let mut c = WeaponClass::bf1_rifle();
    c.heat_per_shot = 0.1;
    let mut w = WeaponState::new(&c, true, 6);
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 1));

    let (c, mut w) = overheated_rifle();
    let blocked = FireInput {
        blocked: true,
        ..fi(true, false)
    };
    assert_eq!(press_update(&mut w, &c, blocked), (false, 0));

    let (c, mut w) = overheated_rifle();
    w.active = false;
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 0));
}

/// The PS2 stays in its fire state after the overheating shot (0.17 s on the
/// pistol, measured live in both games), and a press there does not click.
#[test]
fn a_press_in_the_overheating_shots_cycle_does_not_click() {
    let mut c = WeaponClass::bf1_rifle();
    c.heat_per_shot = 1.0;
    c.heat_recover_rate = 0.1;
    c.heat_threshold = 0.3;
    let mut w = WeaponState::new(&c, true, 6);
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 1));
    assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
    assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 0));
    assert!(w.overheated && w.state == FireState::Firing);
}

/// The update the heat clears on answers the press with a shot, not a click:
/// the heat is updated before the state machine reads it.
#[test]
fn the_press_a_weapon_cools_on_fires_without_a_click() {
    let (c, mut w) = overheated_rifle();
    w.heat = c.heat_threshold + 0.001;
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 1));
    assert!(!w.overheated);
}

/// A press more than 1 s into a fire state clicks (`1.0 < weapon+0xd0`); one
/// sooner does not, and the shot that ends a long refire does not.
#[test]
fn a_long_refire_clicks_only_past_one_second() {
    let mut c = WeaponClass::bf1_rifle();
    c.shot_delay = [2.0; 2];
    let mut w = WeaponState::new(&c, true, 6);
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 1));
    assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
    assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
    // Under a second in.
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 0));
    run(&mut w, &c, DT30, 30, false);
    // Past a second, inside the two-second refire.
    assert!(w.state == FireState::Firing && w.refire_accum > 1.0);
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (true, 0));
}

/// The same refusal for an empty weapon and for one reloading. The empty
/// weapon clicks every press, though each starts a cycle the port runs out.
#[test]
fn an_empty_and_a_reloading_weapon_click() {
    let c = WeaponClass::bf1_rifle();
    let mut w = WeaponState::new(&c, true, 0);
    w.clip = 0;
    w.spare = 0;
    for _ in 0..3 {
        assert_eq!(press_update(&mut w, &c, fi(true, false)), (true, 0));
        assert_eq!(press_update(&mut w, &c, fi(false, false)), (false, 0));
        run(&mut w, &c, DT30, 12, false);
    }

    let mut w = WeaponState::new(&c, true, 6);
    w.clip = 1;
    w.start_reload_as(&c, false);
    assert!(w.reloading());
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (true, 0));
}

/// A press that fires a weapon's last round is not refused, though a cycle
/// shorter than the update ends in the reload that shot started.
#[test]
fn the_press_that_empties_a_fast_weapon_does_not_click() {
    let mut c = WeaponClass::bf1_rifle();
    c.shot_delay = [0.0; 2];
    c.trigger_single = true;
    let mut w = WeaponState::new(&c, true, 6);
    w.clip = c.ammo_per_shot;
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (false, 1));
    assert!(w.reloading() && w.state == FireState::Idle);
}

/// A thrown weapon with nothing in the hand stays idle on a press and clicks.
#[test]
fn an_empty_thrown_weapon_stays_idle_and_clicks() {
    let mut c = WeaponClass::bf1_rifle();
    c.thrown = Some(Thrown {
        shot_elevate: 0.21,
        hide_on_fire: true,
        hand: None,
    });
    let mut w = WeaponState::new(&c, true, 0);
    w.clip = 0;
    w.spare = 0;
    assert_eq!(press_update(&mut w, &c, fi(true, false)), (true, 0));
    assert_eq!(w.state, FireState::Idle);
}
