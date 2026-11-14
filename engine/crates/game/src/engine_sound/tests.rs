use super::*;
use manifest::EngineGraph;

type Points = &'static [(f32, f32)];

fn layer(
    delay: f32,
    speed: Points,
    acceleration: Points,
    on: [Points; 2],
    off: [Points; 2],
) -> EngineLayer {
    let g = |parameter: &str, target: &str, points: Points| EngineGraph {
        parameter: parameter.into(),
        target: target.into(),
        points: points.to_vec(),
    };
    EngineLayer {
        sound: 0,
        delay,
        deviation: 0.0,
        graphs: vec![
            g("speed", "Pitch", speed),
            g("acceleration", "Gain", acceleration),
            g("turnontime", "Gain", on[0]),
            g("turnofftime", "Gain", off[0]),
            g("turnontime", "Pitch", on[1]),
            g("turnofftime", "Pitch", off[1]),
        ],
    }
}

/// BF1 NAB.LVL `cis_hover_aat_engine_parameterized` (re-vehicle-sounds.md
/// §1; `lead_tmp/vsnd/param.py bf1 NAB --sub nab1cw aat_engine`).
fn aat() -> Vec<EngineLayer> {
    let on: [Points; 2] = [&[(0.0, 0.0), (2.0, 1.0)], &[(0.0, 0.5), (3.0, 1.0)]];
    let off: [Points; 2] = [
        &[(0.0, 1.0), (3.0, 0.0)],
        &[(0.0, 1.0), (2.0, 0.6), (3.0, 0.3)],
    ];
    vec![
        layer(
            0.0,
            &[(0.0, 0.8), (0.5, 0.9), (1.0, 1.0)],
            &[(-1.0, 1.0), (0.0, 0.7), (1.0, 1.0)],
            on,
            off,
        ),
        layer(
            0.0,
            &[(0.0, 0.85), (0.5, 0.9), (1.0, 1.0)],
            &[(-1.0, 0.6), (0.0, 0.6), (0.5, 0.9), (1.0, 1.0)],
            on,
            off,
        ),
        layer(
            0.0,
            &[(0.0, 0.6), (0.5, 0.8), (1.0, 1.0)],
            &[(-1.0, 0.8), (0.0, 0.8), (0.5, 0.8), (1.0, 1.0)],
            on,
            off,
        ),
    ]
}

/// BF2 UTA.LVL `atst_engine_parameterized`, in the record's order.
fn atst() -> Vec<EngineLayer> {
    let on: [Points; 2] = [&[(0.0, 0.0), (1.0, 1.0)], &[(0.0, 0.7), (1.0, 1.0)]];
    let off: [Points; 2] = [&[(0.0, 1.0), (1.0, 0.0)], &[(0.0, 1.0), (1.0, 0.1)]];
    let pitch: Points = &[(-0.1, 1.0), (0.0, 0.8), (0.5, 0.9), (1.0, 1.0)];
    vec![
        layer(0.8, pitch, &[(-0.1, 1.0), (0.0, 8.0), (1.0, 1.0)], on, off),
        layer(
            0.8,
            &[(0.0, 0.9), (1.0, 1.0)],
            &[(0.0, 1.0), (1.0, 0.8)],
            on,
            off,
        ),
        layer(0.8, pitch, &[(-0.1, 0.7), (0.0, 0.5), (1.0, 0.7)], on, off),
    ]
}

const DT: f32 = 1.0 / 30.0;
const HOVER: Drive = Drive::Hover {
    forward: 12.0,
    reverse: 10.0,
};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn all(layers: &[EngineLayer], i: &Inputs) -> Vec<(f32, f32)> {
    layers.iter().map(|l| factors(l, i)).collect()
}

#[test]
fn a_tie_crosses_its_shift_points_between_gears_on_every_thrust() {
    let p = core::vehicle::flyer::FlyerParams {
        bf2: true,
        min_speed: 35.0,
        mid_speed: 70.0,
        max_speed: 95.0,
        boost_speed: 150.0,
        ..core::vehicle::flyer::FlyerParams::default()
    };
    let divisor = flyer_speed_divisor(&p, false);
    assert_eq!(divisor, 150.0);
    let up = BoostEntry::parse("imp_fly_TIE_shift_up_high_property 0.69 1").unwrap();
    let down = BoostEntry::parse("imp_fly_TIE_shift_down_property 0.69 0").unwrap();
    let (max, boost) = (95.0 / divisor, 150.0 / divisor);
    for _ in 0..3 {
        assert!(up.crossed(max, boost) && !down.crossed(max, boost));
        assert!(down.crossed(boost, max) && !up.crossed(boost, max));
    }
    let no_boost = core::vehicle::flyer::FlyerParams {
        boost_speed: 0.0,
        ..p
    };
    assert_eq!(flyer_speed_divisor(&no_boost, false), 95.0);
    let bf1 = core::vehicle::flyer::FlyerParams { bf2: false, ..p };
    assert_eq!(flyer_speed_divisor(&bf1, false), 95.0);
}

#[test]
fn a_bf2_flyers_engine_speed_clamps_at_one() {
    let mut e = Instance::default();
    let drive = Drive::Flyer {
        max_speed: 100.0,
        acceleration: 25.0,
        clamped: true,
    };
    let m = Motion {
        piloted: true,
        speed: 180.0,
        health: 1.0,
        phase: Some(core::vehicle::flyer::FlightPhase::Flying),
        ..Motion::default()
    };
    e.update(0.1, drive, 3.0, &m, &[], || 0.5);
    assert_eq!(e.inputs.speed, 1.0);
}

#[test]
fn a_boost_entry_fires_only_on_its_own_crossing() {
    let up = BoostEntry::parse("imp_fly_TIE_shift_up_property 0.53 1").unwrap();
    let down = BoostEntry::parse("imp_fly_TIE_shift_down_property 0.69 0").unwrap();
    assert_eq!(up.sound, "imp_fly_TIE_shift_up_property");
    assert!(up.rising && !down.rising);
    assert!(up.crossed(0.53, 0.54) && !up.crossed(0.54, 0.6) && !up.crossed(0.6, 0.5));
    assert!(down.crossed(0.69, 0.6) && !down.crossed(0.6, 0.5) && !down.crossed(0.6, 0.7));
    assert!(BoostEntry::parse("imp_fly_TIE_shift_up_property").is_none());
}

#[test]
fn a_graph_is_linear_between_its_points_and_flat_outside() {
    let g = [(0.0, 0.8), (0.5, 0.9), (1.0, 1.0)];
    for (x, y) in [
        (-3.0, 0.8),
        (0.0, 0.8),
        (0.25, 0.85),
        (0.5, 0.9),
        (0.9, 0.98),
        (1.0, 1.0),
        (7.0, 1.0),
    ] {
        assert!(near(graph(&g, x), y), "{x}: {}", graph(&g, x));
    }
    assert_eq!(graph(&[], 0.3), 1.0);
}

/// The AAT long after its turn-on: at rest the PS2's voices read pitch 0.80
/// / 0.85 / 0.60 and gain 0.63 / 0.54 / 0.72 = 0.9 x 0.7 / 0.6 / 0.8 (RAM,
/// §6.1); at full speed every pitch factor is 1.
#[test]
fn the_aat_at_rest_and_at_full_speed() {
    let rest = Inputs {
        health: 1.0,
        timer: 200.0,
        ..Inputs::default()
    };
    let got = all(&aat(), &rest);
    for (g, w) in got.iter().zip([(0.7, 0.8), (0.6, 0.85), (0.8, 0.6)]) {
        assert!(near(g.0, w.0) && near(g.1, w.1), "{got:?}");
    }
    let full = Inputs { speed: 1.0, ..rest };
    assert!(all(&aat(), &full).iter().all(|f| near(f.1, 1.0)));
    // A gain graph on a slope.
    let moving = Inputs {
        acceleration: 0.5,
        ..full
    };
    let got = all(&aat(), &moving);
    assert!(
        near(got[0].0, 0.85) && near(got[1].0, 0.9) && near(got[2].0, 0.8),
        "{got:?}"
    );
}

/// Graphs on one target multiply: 1 s into the turn-on the gain factor is
/// the acceleration graph x the fade (0.7 x 0.5), the pitch the speed graph
/// x the rise (0.8 x 2/3); the run-down reads the other pair.
#[test]
fn graphs_on_one_target_multiply() {
    let on = Inputs {
        timer: 1.0,
        ..Inputs::default()
    };
    let (g, p) = factors(&aat()[0], &on);
    assert!(
        near(g, 0.7 * 0.5) && near(p, 0.8 * (0.5 + 0.5 / 3.0)),
        "{g} {p}"
    );
    let off = Inputs {
        turning_off: true,
        ..on
    };
    let (g, p) = factors(&aat()[0], &off);
    assert!(near(g, 0.7 * (2.0 / 3.0)) && near(p, 0.8 * 0.8), "{g} {p}");
}

fn motion(piloted: bool, forward: f32, throttle: f32) -> Motion {
    Motion {
        phase: None,
        piloted,
        forward,
        speed: forward.abs(),
        throttle,
        health: 1.0,
    }
}

/// A hover's speed is |forward speed| over `ForwardSpeed`, or over
/// `ReverseSpeed` whenever the throttle is not forward (§6.3: 12.024 m/s ->
/// 1.0020, 11.885 m/s coasting -> 1.1885, -9.729 -> 0.9728).
#[test]
fn a_hovers_speed_is_normalised_by_the_throttles_side() {
    let layers = aat();
    let mut e = Instance::default();
    for (forward, throttle, want) in [
        (12.024, 1.0, 1.002),
        (11.885, 0.0, 1.1885),
        (-9.729, -1.0, 0.9729),
    ] {
        let m = motion(true, forward, throttle);
        e.update(DT, HOVER, 3.0, &m, &layers, || 0.5);
        assert!((e.inputs.speed - want).abs() < 1e-3, "{}", e.inputs.speed);
    }
}

/// The filtered acceleration moves 1 % of the way per update: a step in
/// speed leaves x0.99 per update after it (§6.4: x0.860 = 0.99^15 in 0.5 s).
#[test]
fn the_acceleration_filter_keeps_99_percent_per_update() {
    let layers = aat();
    let mut e = Instance::default();
    let m = motion(true, 12.0, 1.0);
    e.update(DT, HOVER, 3.0, &m, &layers, || 0.5);
    let a0 = e.inputs.acceleration;
    // One update from rest to full speed: (1 - 0) / dt x 0.01.
    assert!(near(a0, 0.3), "{a0}");
    for _ in 0..15 {
        e.update(DT, HOVER, 3.0, &m, &layers, || 0.5);
    }
    let kept = e.inputs.acceleration / a0;
    assert!((kept - 0.99f32.powi(15)).abs() < 1e-4, "{kept}");
}

/// On with a pilot, layers at once (delay 0); the run-down from the update
/// the seat empties, the stop at the first update whose timer has reached
/// `TurnOffTime` (PS2: timer 3.003, §6.5); a pilot inside the run-down
/// turns it on again from 0.
#[test]
fn on_with_a_pilot_and_off_after_the_run_down() {
    let layers = aat();
    let mut e = Instance::default();
    let (pilot, empty) = (motion(true, 0.0, 0.0), motion(false, 0.0, 0.0));
    e.update(DT, HOVER, 3.0, &empty, &layers, || 0.5);
    assert!(!e.on && e.started == [false; 3]);
    e.update(DT, HOVER, 3.0, &pilot, &layers, || 0.5);
    assert!(e.on && !e.turning_off && e.started == [true; 3]);
    assert_eq!(e.inputs.timer, 0.0);
    for _ in 0..90 {
        e.update(DT, HOVER, 3.0, &pilot, &layers, || 0.5);
    }
    // The run-down: timer from 0, the gain falling along the graph.
    e.update(DT, HOVER, 3.0, &empty, &layers, || 0.5);
    assert!(e.on && e.turning_off && e.inputs.timer == 0.0);
    let mut updates = 1;
    let mut last = e.inputs;
    while e.on {
        last = e.inputs;
        e.update(DT, HOVER, 3.0, &empty, &layers, || 0.5);
        updates += 1;
        assert!(updates < 200);
    }
    // 90 or 91 updates sent (float sums of 1 / 30), then the stop.
    assert!((91..=92).contains(&updates), "{updates}");
    assert!(
        last.timer < 3.0 && last.timer + DT >= 3.0 - 1e-4,
        "{last:?}"
    );
    assert!(factors(&layers[0], &last).0 < 0.02);
    assert!(e.started == [false; 3]);
    // Boarded again during a run-down.
    e.update(DT, HOVER, 3.0, &pilot, &layers, || 0.5);
    for _ in 0..30 {
        e.update(DT, HOVER, 3.0, &empty, &layers, || 0.5);
    }
    assert!(e.on && e.turning_off && e.inputs.timer > 0.9);
    e.update(DT, HOVER, 3.0, &pilot, &layers, || 0.5);
    assert!(e.on && !e.turning_off && e.inputs.timer == 0.0);
    // `TurnOffTime` 0 (the droideka's): off in the update the seat empties.
    e.update(DT, HOVER, 0.0, &empty, &layers, || 0.5);
    assert!(!e.on);
}

/// A walker sends m/s and `previous - speed / dt`: walking at 5.58 m/s the
/// pitch factors are 1 and each gain graph is read at its left end (BF2
/// RAM, §6.7); its layers start 0.8 s after the turn-on.
#[test]
fn a_walker_reads_its_gain_graphs_at_the_left_end() {
    let layers = atst();
    let mut e = Instance::default();
    let m = Motion {
        piloted: true,
        speed: 5.58,
        health: 1.0,
        ..Motion::default()
    };
    let mut first = None;
    for n in 0..120 {
        e.update(DT, Drive::Walker, 3.0, &m, &layers, || 0.5);
        if first.is_none() && e.started[0] {
            first = Some(n);
        }
    }
    assert!(e.started == [true; 3]);
    assert!((24..=25).contains(&first.unwrap()), "{first:?}");
    assert!(near(e.inputs.speed, 5.58) && e.inputs.acceleration < -100.0);
    let got = all(&layers, &e.inputs);
    for (g, w) in got.iter().zip([1.0, 1.0, 0.7]) {
        assert!(near(g.0, w) && near(g.1, 1.0), "{got:?}");
    }
    // Standing: acceleration 0, the graphs' own points at 0.
    let still = Motion { speed: 0.0, ..m };
    e.update(DT, Drive::Walker, 3.0, &still, &layers, || 0.5);
    e.update(DT, Drive::Walker, 3.0, &still, &layers, || 0.5);
    let got = all(&layers, &e.inputs);
    assert!(
        near(got[0].0, 8.0) && near(got[2].0, 0.5) && near(got[0].1, 0.8),
        "{got:?}"
    );
}

const VSND: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts/lead_tmp/vsnd");

/// The PS2's own numbers through the port's code (card VSND's referee): the
/// BAKED NAB1 records and [`factors`] against the multipliers in the PS2's
/// memory, per layer per sampled update of two live sessions
/// (`lead_tmp/vsnd/live{1,2}.csv`: stored speed, acceleration, health,
/// timer and flags of every hover instance), with the fixed rule of
/// `lead_tmp/vsnd/verify/strict_replay.py` - the timer sent is the sampled
/// one less the update's 1 / 30. Data-gated; `VSND_BAKED` names another
/// baked root.
#[test]
fn the_ps2s_multipliers_replay_through_the_baked_records() {
    let root = std::env::var_os("VSND_BAKED")
        .map_or_else(crate::assets::baked_root, std::path::PathBuf::from);
    let Some(lib) = crate::sounds::SoundLib::load(&root.join("bf1_nab1")) else {
        eprintln!("SKIP: bf1_nab1 sounds not baked");
        return;
    };
    let names = [
        "cis_hover_aat_engine_parameterized",
        "cis_hover_stap_engine_parameterized",
        "interceptorTank_engine_parameterized",
    ];
    let mut by_sound = HashMap::new();
    for (k, n) in names.iter().enumerate() {
        let Some(rec) = lib.engine(fnv1a_lowercase(n), &[fnv1a_lowercase("nab1cw")]) else {
            eprintln!("SKIP: {n} not in the bake (rebake sounds)");
            return;
        };
        for l in &rec.layers {
            by_sound.insert(format!("{:08X}", l.sound), (k, l.clone()));
        }
    }
    let (mut rows, mut ok) = ([0usize; 3], [0usize; 3]);
    for file in ["live1.csv", "live2.csv"] {
        let Ok(text) = pak::read_to_string(format!("{VSND}/{file}")) else {
            eprintln!("SKIP: {VSND}/{file} not present");
            return;
        };
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
        let (c_flags, c_timer, c_health, c_speed, c_accel, c_prop, c_g, c_p) = (
            col("flags"),
            col("timer"),
            col("health"),
            col("stored"),
            col("accel"),
            col("prop"),
            col("multG"),
            col("multP"),
        );
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            let Some((k, l)) = by_sound.get(f[c_prop]) else {
                continue;
            };
            let flags = u32::from_str_radix(f[c_flags], 16).unwrap();
            if flags & 2 == 0 {
                continue;
            }
            // No hover winds up or down (flags 4, 8).
            assert_eq!(flags & 0xc, 0, "{line}");
            let num = |c: usize| f[c].parse::<f32>().unwrap();
            let i = Inputs {
                speed: num(c_speed),
                acceleration: num(c_accel),
                health: num(c_health),
                timer: (num(c_timer) - 1.0 / 30.0).max(0.0),
                turning_off: flags & 0x10 != 0,
                wind: Wind::None,
            };
            let (g, p) = factors(l, &i);
            rows[*k] += 1;
            ok[*k] += usize::from((g - num(c_g)).abs().max((p - num(c_p)).abs()) <= 0.002);
        }
    }
    let (n, good) = (rows.iter().sum::<usize>(), ok.iter().sum::<usize>());
    eprintln!(
        "VSND replay: AAT {}/{}, STAP {}/{}, fighter tank {}/{}; {good} of {n} layer-ticks within 0.002",
        ok[0], rows[0], ok[1], rows[1], ok[2], rows[2]
    );
    assert_eq!(n, 19_419);
    assert!(good * 100 >= n * 99, "{good} of {n}");
}

/// BF2 Utapau's baked `atst_engine_parameterized`, layers in the layered
/// record's order: a walker at 5.58 m/s reads gain factors 0.7 / 1.0 / 1.0
/// and pitch factor 1 (RAM, §6.7). Data-gated like the replay.
#[test]
fn utapaus_baked_walker_record_reads_the_rams_factors() {
    let root = std::env::var_os("VSND_BAKED")
        .map_or_else(crate::assets::baked_root, std::path::PathBuf::from);
    let lib = crate::sounds::SoundLib::load(&root.join("bf2_uta1"));
    let rec = lib.as_ref().and_then(|l| {
        l.engine(
            fnv1a_lowercase("atst_engine_parameterized"),
            &[fnv1a_lowercase("uta1cw")],
        )
    });
    let Some(rec) = rec else {
        eprintln!("SKIP: bf2_uta1 engines not baked");
        return;
    };
    let i = Inputs {
        speed: 5.58,
        acceleration: 5.58 - 5.58 * 30.0,
        health: 1.0,
        timer: 50.0,
        turning_off: false,
        wind: Wind::None,
    };
    let got = all(&rec.layers, &i);
    for (g, w) in got.iter().zip([0.7, 1.0, 1.0]) {
        assert!(near(g.0, w) && near(g.1, 1.0), "{got:?}");
    }
    assert!(rec.layers.iter().all(|l| l.delay == 0.8));
}

/// Card JETP2: the baked `rep_inf_Jetpack_engine_parameterized` (one layer,
/// BF2 KAS; BF1's is the same): speed -> pitch 0.9..1.0, acceleration ->
/// gain 0.4 at 0, 0.5 under -0.1.
fn jet_record() -> EngineSound {
    EngineSound {
        name_hash: fnv1a_lowercase("rep_inf_Jetpack_engine_parameterized"),
        level: 0,
        layers: vec![EngineLayer {
            sound: fnv1a_lowercase("rep_inf_Jetpack_engine_parameterized"),
            delay: 0.0,
            deviation: 0.0,
            graphs: vec![
                EngineGraph {
                    parameter: "speed".into(),
                    target: "Pitch".into(),
                    points: vec![(0.0, 0.9), (1.0, 1.0)],
                },
                EngineGraph {
                    parameter: "acceleration".into(),
                    target: "Gain".into(),
                    points: vec![(-0.1, 0.5), (0.0, 0.4), (0.1, 0.5), (1.0, 1.0)],
                },
            ],
        }],
    }
}

fn jet_look(turn_off_time: f32) -> crate::roster::JetLook {
    crate::roster::JetLook {
        effect: Some("rep_sfx_jetpack".into()),
        engine: Some("rep_inf_Jetpack_engine_parameterized".into()),
        turn_on: Some("rep_weap_jetpack_turnon".into()),
        turning_off: None,
        turn_off: Some("rep_weap_jetpack_turnoff".into()),
        turn_off_time,
    }
}

/// One tick of one soldier: (jetting, speed) -> the loop's voices and the
/// one-shots.
fn jet_tick(
    e: &mut EngineSounds,
    look: &crate::roster::JetLook,
    rec: &EngineSound,
    jetting: bool,
    speed: f32,
) -> (Vec<Voice>, Vec<JetShotKind>) {
    let m = JetMotion {
        unit: 3,
        gen: 1,
        jetting,
        pos: Vec3::new(1.0, 2.0, 3.0),
        vel: Vec3::X * speed,
        health: 1.0,
        look,
    };
    let mut out = Vec::new();
    let record = |h: u32, _: &[u32]| (h == rec.name_hash).then_some(rec);
    e.tick_jets(std::iter::once(m), &record, &mut out);
    let kinds = e.take_jet_shots().into_iter().map(|s| s.kind).collect();
    (out, kinds)
}

/// Turn-on at the jet's start, the loop while it is on, the turn-off at
/// every end (a tap, empty fuel or a landing: all leave the jet stance, and
/// the PS2 runs the turn-off on any update outside it - B1:117778-117783,
/// `FUN_00366318`), with no run-down: every jet class's `TurnOffTime` is 0.
#[test]
fn a_jet_turns_on_loops_and_turns_off_at_every_end() {
    let rec = jet_record();
    let look = jet_look(0.0);
    let mut e = EngineSounds::default();
    let (v, s) = jet_tick(&mut e, &look, &rec, false, 0.0);
    assert!(v.is_empty() && s.is_empty(), "standing: nothing");
    for flight in 0..3 {
        let (v, s) = jet_tick(&mut e, &look, &rec, true, 0.0);
        assert_eq!(s, [JetShotKind::TurnOn], "flight {flight}");
        assert_eq!(v.len(), 1);
        assert!(v[0].soldier && v[0].vehicle == 3);
        assert_eq!(v[0].pos, Vec3::new(1.0, 2.0, 3.0));
        // At rest: pitch 0.9, gain 0.4 (acceleration 0 - 0 / dt).
        assert!(near(v[0].pitch_factor, 0.9) && near(v[0].gain_factor, 0.4));
        for _ in 0..20 {
            let (v, s) = jet_tick(&mut e, &look, &rec, true, 8.0);
            assert!(s.is_empty());
            assert_eq!(v.len(), 1, "the loop runs while he jets");
            // |v| 8 m/s: pitch 1.0; previous - speed / dt is about -232:
            // the gain graph's left end, 0.5 (the walker's slip).
            assert!(near(v[0].pitch_factor, 1.0) && near(v[0].gain_factor, 0.5));
            assert!(near(v[0].speed, 8.0));
        }
        // The end (whatever ended it): turn-off at once, the loop gone.
        let (v, s) = jet_tick(&mut e, &look, &rec, false, 8.0);
        assert_eq!(s, [JetShotKind::TurnOff], "flight {flight}");
        assert!(v.is_empty());
        assert!(!e.jet(3).unwrap().on);
        let (v, s) = jet_tick(&mut e, &look, &rec, false, 0.0);
        assert!(v.is_empty() && s.is_empty());
    }
}

/// With a `TurnOffTime` (none of the disc's jet classes has one): the loop
/// runs down that long, then `TurnOffSound`; jetting again inside it turns
/// on again.
#[test]
fn a_run_down_ends_with_the_turn_off_and_a_restart_turns_on() {
    let rec = jet_record();
    let look = crate::roster::JetLook {
        turning_off: Some("x_turning_off".into()),
        ..jet_look(1.0)
    };
    let mut e = EngineSounds::default();
    assert_eq!(
        jet_tick(&mut e, &look, &rec, true, 5.0).1,
        [JetShotKind::TurnOn]
    );
    let (v, s) = jet_tick(&mut e, &look, &rec, false, 5.0);
    assert_eq!(s, [JetShotKind::TurningOff]);
    assert_eq!(v.len(), 1, "running down");
    assert_eq!(
        jet_tick(&mut e, &look, &rec, true, 5.0).1,
        [JetShotKind::TurnOn]
    );
    assert_eq!(
        jet_tick(&mut e, &look, &rec, false, 5.0).1,
        [JetShotKind::TurningOff]
    );
    let mut ends = Vec::new();
    for t in 0..40 {
        let (v, s) = jet_tick(&mut e, &look, &rec, false, 5.0);
        if !s.is_empty() {
            assert!(v.is_empty());
            ends.push((t, s));
        }
    }
    // Timer 0 at the run-down's start, +1/30 an update: 1.0 s reached on
    // the 30th update after it.
    assert_eq!(ends.len(), 1, "{ends:?}");
    assert_eq!(ends[0].1, [JetShotKind::TurnOff]);
    assert!((28..=30).contains(&ends[0].0), "{ends:?}");
}

/// A soldier who respawns (a new generation) starts with his engine off;
/// a class without jet sounds is skipped.
#[test]
fn a_new_life_starts_off_and_a_jetless_class_has_no_engine() {
    let rec = jet_record();
    let look = jet_look(0.0);
    let mut e = EngineSounds::default();
    jet_tick(&mut e, &look, &rec, true, 0.0);
    let m = JetMotion {
        unit: 3,
        gen: 2,
        jetting: false,
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        health: 1.0,
        look: &look,
    };
    let record = |_: u32, _: &[u32]| Some(&rec);
    let mut out = Vec::new();
    e.tick_jets(std::iter::once(m), &record, &mut out);
    assert!(out.is_empty() && e.take_jet_shots().is_empty());
    let none = crate::roster::JetLook::default();
    let (v, s) = jet_tick(&mut EngineSounds::default(), &none, &rec, true, 0.0);
    assert!(v.is_empty() && s.is_empty());
}

/// The baked jet classes of both games name the engine block and its
/// sounds the map banks hold (re-jetpack.md §2.2): data-gated.
#[test]
fn baked_jet_classes_name_baked_sounds() {
    let root = crate::assets::baked_root();
    if !root.join("classes").exists() {
        eprintln!("SKIP: no class bake");
        return;
    }
    let mut rows = 0;
    for (game, map, classes, rifleman) in [
        (
            "bf2",
            "bf2_kas2",
            &["rep_inf_ep3_jettrooper", "imp_inf_dark_trooper"][..],
            "rep_inf_ep3_rifleman",
        ),
        (
            "bf1",
            "bf1_kas2",
            &["rep_inf_jet_trooper", "imp_inf_dark_trooper"][..],
            "rep_inf_clone_trooper",
        ),
    ] {
        let db = crate::roster::load_classdb(&root.join("classes"), game, &["rep", "imp"]).unwrap();
        let Some(lib) = crate::sounds::SoundLib::load(&root.join(map)) else {
            eprintln!("SKIP: no {map} sounds");
            continue;
        };
        for class in classes {
            let j = crate::roster::jet_look(&db, class);
            assert_eq!(j.turn_off_time, 0.0, "{class}");
            assert_eq!(j.turn_on.as_deref(), Some("rep_weap_jetpack_turnon"));
            assert_eq!(j.turn_off.as_deref(), Some("rep_weap_jetpack_turnoff"));
            for n in [&j.turn_on, &j.turn_off].into_iter().flatten() {
                assert!(lib.has(n), "{map} {class}: {n}");
            }
            let engine = j.engine.as_deref().unwrap();
            assert_eq!(engine, "rep_inf_Jetpack_engine_parameterized");
            let rec = lib
                .engine(fnv1a_lowercase(engine), &[])
                .expect("the record");
            assert_eq!(rec.layers.len(), 1);
            assert!(lib.has(engine), "the layer's loop");
            assert!(j.effect.is_some(), "{class}");
            rows += 1;
        }
        if db.resolve(rifleman).is_some() {
            assert_eq!(crate::roster::jet_look(&db, rifleman), Default::default());
        }
    }
    println!("{rows} jet classes checked");
}

/// A snowspeeder's take-off and landing: its loop runs on with no pilot in the air, the
/// time graphs follow the wind, and only a landed craft runs down.
#[test]
fn a_flyer_winds_up_at_takeoff_and_stops_only_when_landed() {
    let drive = Drive::Flyer {
        max_speed: 50.0,
        acceleration: 25.0,
        clamped: false,
    };
    let g = |parameter: &str, points: Points| EngineGraph {
        parameter: parameter.into(),
        target: "Gain".into(),
        points: points.to_vec(),
    };
    let up: Points = &[(0.0, 0.25), (1.0, 1.0)];
    let down: Points = &[(0.0, 1.0), (1.0, 0.5)];
    let layers = vec![EngineLayer {
        sound: 0,
        delay: 0.0,
        deviation: 0.0,
        graphs: vec![g("winduptime", up), g("winddowntime", down)],
    }];
    let mut e = Instance::default();
    let mut step = |piloted: bool, speed: f32, phase: FlightPhase| {
        let m = Motion {
            piloted,
            speed,
            health: 1.0,
            phase: Some(phase),
            ..Motion::default()
        };
        e.update(0.1, drive, 3.0, &m, &layers, || 0.5);
        (e.on, e.wind, e.wind_began, e.inputs, e.turning_off)
    };
    let (on, wind, began, ..) = step(true, 0.0, FlightPhase::Landed);
    assert!(on && wind == Wind::None && !began);
    let (.., began, i, _) = step(true, 0.0, FlightPhase::Takeoff);
    assert!(began && i.wind == Wind::Up && i.timer == 0.0);
    assert_eq!(factors(&layers[0], &i).0, 0.25);
    let (.., began, _, _) = step(true, 0.0, FlightPhase::Takeoff);
    assert!(!began);
    // In flight: no pilot and the loop runs; the speed reads against MaxSpeed.
    let (on, .., i, off) = step(false, 25.0, FlightPhase::Flying);
    assert!(on && !off && i.wind == Wind::Up);
    assert_eq!(i.speed, 0.5);
    assert!((i.acceleration - 0.5 / 0.1 / 25.0).abs() < 1e-5);
    let (.., began, i, _) = step(true, 25.0, FlightPhase::Landing);
    assert!(began && i.wind == Wind::Down);
    assert_eq!(factors(&layers[0], &i).0, 1.0);
    let (.., off) = step(false, 0.0, FlightPhase::Landed);
    assert!(off, "a landed craft with no pilot runs down");
}

/// The baked Hoth snowspeeder: its class names a loop record the battle resolves, and
/// its takeoff and landing are sounds the battle can play. Data-gated like the replay.
#[test]
fn hoths_baked_snowspeeder_has_its_loop_and_its_wind_shots() {
    let root = std::env::var_os("VSND_BAKED")
        .map_or_else(crate::assets::baked_root, std::path::PathBuf::from);
    let Some(lib) = crate::sounds::SoundLib::load(&root.join("bf1_hot1")) else {
        eprintln!("SKIP: bf1_hot1 not baked");
        return;
    };
    let rec = lib
        .engine(
            fnv1a_lowercase("all_fly_snowspeeder_engine_parameterized"),
            &[
                fnv1a_lowercase("hot"),
                fnv1a_lowercase("hot1cw"),
                fnv1a_lowercase("hot1gcw"),
            ],
        )
        .expect("the loop's record");
    assert!(!rec.layers.is_empty());
    let params: Vec<&str> = rec
        .layers
        .iter()
        .flat_map(|l| l.graphs.iter().map(|g| g.parameter.as_str()))
        .collect();
    eprintln!("{} layers, graphs {params:?}", rec.layers.len());
    for name in ["all_fly_snowspeeder_takeoff", "all_fly_snowspeeder_land"] {
        assert!(lib.has(name), "{name}");
    }
}

#[test]
#[ignore = "requires owned BF2 Space Felucia classes and sound banks"]
fn stock_arc170_maneuvers_spend_energy_and_play_one_authored_sound_each() {
    use core::sim::{buttons, PadState};
    use core::vehicle::{SeatKind, VehicleSpec};

    let root = std::env::var_os("STUNT_BAKED")
        .map_or_else(crate::assets::baked_root, std::path::PathBuf::from);
    let map = root.join("bf2_spa7");
    let lib = crate::sounds::SoundLib::load(&map).expect("Space Felucia sound banks");
    let db = crate::roster::load_classdb(&root.join("classes"), "bf2", &["rep"]).unwrap();
    let class = "rep_fly_arc170fighter_sc";
    let params = crate::vehicle_pads::flyer_params(&db, class, true, 1.0).unwrap();
    let engine = EngineClass::from_db(&db, class).unwrap();
    assert_eq!(params.trick_flip_speed, 5.0);
    assert_eq!(params.trick_roll_speed, 5.0);
    assert_eq!(params.trick_flip_drain, 30.0);
    assert_eq!(params.trick_drain, 15.0);
    for (kind, expected_sound, button, axis, energy_cost) in [
        (
            ManeuverKind::SideRoll,
            "rep_fly_arc170_roll",
            buttons::ROLL_STANCE,
            i16::MAX,
            params.trick_drain,
        ),
        (
            ManeuverKind::Reversal,
            "rep_fly_arc170_flip",
            buttons::FLYER_REVERSE,
            0,
            params.trick_flip_drain,
        ),
    ] {
        let mut walk = WalkSim::new(42, Vec3::ZERO, None);
        walk.sim.flyer_params = vec![Some(params.clone())];
        walk.vehicle_engines = vec![Some(engine.clone())];
        let flyer = walk.sim.spawn_vehicle(&VehicleSpec {
            kind: VehicleKind::Flyer,
            class: 0,
            home_team: None,
            neutral_when_empty: false,
            spawn_pad: None,
            pos: Vec3::Y * 75.0,
            yaw: 0.0,
            max_health: 1000.0,
            health_type: core::weapon::HealthType::Vehicle,
            seats: vec![SeatKind::Pilot],
        });
        assert!(walk.sim.seat_unit(walk.player, flyer, 0));
        let v = walk.sim.vehicle_mut(flyer).unwrap();
        v.flyer.phase = FlightPhase::Flying;
        v.flyer.lift = 1.0;
        v.flyer.initialized = true;
        v.flyer.energy = params.energy_max;
        v.vel = Vec3::Z * params.mid_speed;
        let mut mixer = EngineSounds::new(&map, &walk);
        let mut voices = Vec::new();

        let mut shots = Vec::new();
        for i in 0..120 {
            walk.sim.step(&[if i == 0 {
                PadState {
                    buttons: button,
                    rx: axis,
                    ..PadState::default()
                }
            } else {
                PadState::default()
            }]);
            if i == 0 {
                let v = walk.sim.vehicle(flyer).unwrap();
                assert_eq!(v.flyer.energy, params.energy_max - energy_cost);
                assert_eq!(v.flyer.active_maneuver(), Some(kind));
            }
            mixer.tick(&walk, |hash, levels| lib.engine(hash, levels), &mut voices);
            shots.extend(
                mixer
                    .take_jet_shots()
                    .into_iter()
                    .filter(|s| matches!(s.kind, JetShotKind::Maneuver(_))),
            );
        }
        assert_eq!(shots.len(), 1, "{kind:?}: {shots:?}");
        assert_eq!(shots[0].kind, JetShotKind::Maneuver(kind));
        assert_eq!(shots[0].name, expected_sound);
        assert!(lib.has(expected_sound));

        let v = walk.sim.vehicle(flyer).unwrap();
        assert!(v.flyer.active_maneuver().is_none());
        assert!((v.rot * Vec3::Y).y > 0.9, "{kind:?}: exits upright");
        let heading = if kind == ManeuverKind::SideRoll {
            1.0
        } else {
            -1.0
        };
        assert!((v.rot * Vec3::Z).z * heading > 0.9, "{kind:?}: {v:?}");
    }
}

/// Card VSN2 (re-vehicle-sounds §3.3, §3.5, §4): a vehicle class with the
/// two one-shots and no `EngineSound` record (the kaadu, the tauntaun)
/// still runs its instance, crying at boarding and at the run-down's end.
#[test]
fn a_recordless_walker_cries_at_boarding_and_after_its_run_down() {
    use core::vehicle::{SeatKind, VehicleSpec};
    let mut walk = WalkSim::new(42, Vec3::ZERO, None);
    walk.vehicle_engines = vec![Some(EngineClass {
        record: String::new(),
        turn_on: Some("tauntaunstart".into()),
        turning_off: None,
        turn_off: Some("tauntaunstop".into()),
        turn_off_time: 3.0,
        takeoff: None,
        land: None,
        roll: None,
        reversal: None,
        boost: Vec::new(),
    })];
    let mount = walk.sim.spawn_vehicle(&VehicleSpec {
        kind: VehicleKind::Walker,
        class: 0,
        home_team: None,
        neutral_when_empty: false,
        spawn_pad: None,
        pos: Vec3::ZERO,
        yaw: 0.0,
        max_health: 400.0,
        health_type: core::weapon::HealthType::Vehicle,
        seats: vec![SeatKind::Pilot],
    });
    let mut mixer = EngineSounds::default();
    let mut voices = Vec::new();
    let mut tick = |walk: &WalkSim, mixer: &mut EngineSounds| {
        mixer.tick(walk, |_, _| None, &mut voices);
        mixer
            .take_jet_shots()
            .into_iter()
            .map(|s| (s.kind, s.name))
            .collect::<Vec<_>>()
    };
    assert!(tick(&walk, &mut mixer).is_empty(), "empty: off, silent");
    assert!(walk.sim.seat_unit(walk.player, mount, 0));
    assert_eq!(
        tick(&walk, &mut mixer),
        [(JetShotKind::TurnOn, "tauntaunstart".to_owned())]
    );
    assert!(tick(&walk, &mut mixer).is_empty());
    walk.sim.vehicle_mut(mount).unwrap().seats[0].occupant = None;
    let mut cries = Vec::new();
    for t in 0..4 * core::TICK_HZ {
        for s in tick(&walk, &mut mixer) {
            cries.push((t, s));
        }
    }
    assert_eq!(cries.len(), 1, "{cries:?}");
    assert_eq!(cries[0].1, (JetShotKind::TurnOff, "tauntaunstop".to_owned()));
    let at = cries[0].0 as f32 * core::TICK_DT;
    assert!((at - 3.0).abs() <= 2.0 * core::TICK_DT, "{at}");
}

/// The baked tauntaun and kaadu classes name their cries (data-gated).
#[test]
fn baked_animal_mounts_name_their_cries() {
    let classes = crate::assets::baked_root().join("classes");
    for (game, side, class, on, off) in [
        ("bf2", "all", "all_walk_tauntaun", "tauntaunstart", "tauntaunstop"),
        ("bf1", "all", "all_walk_tauntaun", "tauntaunstart", "tauntaunstop"),
    ] {
        let Ok(db) = crate::roster::load_classdb(&classes, game, &[side]) else {
            eprintln!("SKIP: {game} classes not baked");
            continue;
        };
        let e = EngineClass::from_db(&db, class).expect("an instance with no record");
        assert!(e.record.is_empty(), "{game}");
        assert_eq!(
            (e.turn_on.as_deref(), e.turn_off.as_deref()),
            (Some(on), Some(off)),
            "{game}"
        );
    }
}
