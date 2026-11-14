//! Issue #168: the drawn view's smoothness, LAN against single player. A
//! steady input (the same mouse motion every frame, or a held stick) must
//! turn the drawn aim and the camera by the same step every frame (the
//! stick's step in proportion to the frame's time), on the host and on a
//! guest, at 60 and 144 Hz, with and without frame-time jitter; running
//! forward must move the drawn soldier at an even speed.

use super::tests::setup;
use super::*;
use crate::walk::WalkSim;
use core::sim::{buttons, PadState};
use glam::Vec3;
use net::{ClientConfig, ClientCore, HostConfig, HostCore, LinkParams, LinkSim, Micros};

/// Seconds of play before the measurement (the round starts, the soldier
/// stands).
const WARM_S: f64 = 3.0;
/// When the guest joins (s).
const GUEST_AT_S: f64 = 1.0;
/// Seconds measured.
const MEASURE_S: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Input {
    /// This many radians of mouse motion every frame.
    Mouse(f32),
    /// The right stick held at this push.
    Stick(f32),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    pub hz: f64,
    /// Frame-time jitter: ±30% and a 40 ms frame now and then.
    pub jitter: bool,
    pub input: Input,
    /// Hold forward too.
    pub run: bool,
    /// The host's network thread polls late now and then (a busy CPU).
    pub net_stalls: bool,
    /// The frame clock's offset from the host's tick clock (µs). 0 puts
    /// frames on the tick closes at 60 Hz: the host player's inputs race
    /// the closes, so his queue starves and slips several times a second.
    pub phase_us: f64,
}

/// One drawn frame.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Sample {
    pub dt: f32,
    pub yaw: f32,
    pub pos: Vec3,
    pub cam_yaw: f32,
    pub cam_pos: Vec3,
    /// The other human's drawn position, when there is one.
    pub other: Option<Vec3>,
    /// The drawn view was the local player's own (seated, alive).
    pub ok: bool,
}

/// `SMOOTH_TRACE=from..to`: print those measured frames' state.
fn trace(frame: usize) -> bool {
    std::env::var("SMOOTH_TRACE").ok().is_some_and(|v| {
        let (a, b) = v.split_once("..").unwrap_or(("0", "0"));
        (a.parse().unwrap_or(0)..b.parse().unwrap_or(0)).contains(&frame)
    })
}

struct Lcg(u64);
impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The fixture battle with a command post per side, so a joining guest's
/// spawn screens have a post to accept.
fn fixture(setup: &RoundSetup, local: u8) -> anyhow::Result<WalkSim> {
    let mut walk = WalkSim::new(setup.seed, Vec3::ZERO, None);
    walk.spawn_dummy_targets();
    walk.local_slot = local;
    for (team, x) in [(0u8, 30.0f32), (1, -30.0)] {
        let at = Vec3::new(x, 0.0, 10.0);
        let (capture_time, neutralize_time) = core::cp::CpDef::class_times(true);
        walk.sim.add_cp(
            core::cp::CpDef {
                name: format!("cp{team}"),
                pos: at,
                capture: core::region::Region::sphere(at, 4.0),
                control: None,
                value_bleed: 10,
                value_atk: [10; 2],
                value_def: [10; 2],
                spawn_nodes: vec![(at, 0.0)],
                capture_time,
                neutralize_time,
            },
            Some(team),
        );
    }
    install_humans(&mut walk, setup)?;
    Ok(walk)
}

/// Frame times (s) for `secs` of play.
fn frame_times(case: &Case, secs: f64, seed: u64) -> Vec<f64> {
    let mut rng = Lcg(seed);
    let base = 1.0 / case.hz;
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < secs {
        let mut dt = base;
        if case.jitter {
            dt *= 1.0 + 0.6 * (rng.unit() - 0.5);
            if rng.unit() < 0.02 {
                dt = 0.040;
            }
        }
        out.push(dt);
        t += dt;
    }
    out
}

fn pad_of(case: &Case, warm: bool, frame: usize) -> PadState {
    let mut p = PadState::default();
    if warm {
        // The side, a post (pushed to one), the class: each screen's accept.
        match frame % 40 {
            0..10 => p.buttons |= buttons::SPAWN_POST_RIGHT,
            20..30 => p.buttons |= buttons::SPAWN_ACCEPT,
            _ => {}
        }
        return p;
    }
    match case.input {
        Input::Mouse(rad) => {
            p.mx = PadState::quantize_look(f64::from(rad));
            p.buttons |= buttons::MOUSE_AIM;
        }
        Input::Stick(push) => p.rx = PadState::quantize(push),
    }
    if case.run {
        p.ly = PadState::quantize(1.0);
    }
    p
}

fn sample(walk: &WalkSim, dt: f32) -> Sample {
    let i = walk.player.index as usize;
    let u = &walk.sim.units[i];
    let cam = walk.camera();
    Sample {
        dt,
        yaw: u.soldier.yaw,
        pos: u.soldier.pos,
        cam_yaw: cam.yaw,
        cam_pos: cam.position,
        other: None,
        ok: u.alive && u.mount.is_none(),
    }
}

/// The other human slot's drawn position (slots 0 and 1).
fn other_of(walk: &WalkSim, local: u8) -> Option<Vec3> {
    let h = walk
        .sim
        .players
        .get(usize::from(1 - local.min(1)))
        .copied()
        .flatten()?;
    walk.sim.unit(h).filter(|u| u.alive).map(|u| u.soldier.pos)
}

/// Single player, as the windowed loop draws it: the mouse's motion waits
/// in the look until a tick takes it (`Kbm::consumed`), the stick is the
/// frame's pad.
pub(super) fn run_single_player(case: &Case) -> Vec<Sample> {
    let s = setup();
    let mut walk = fixture(&s, 0).unwrap();
    walk.blend.on = true;
    let warm = frame_times(case, WARM_S, 1);
    let measure = frame_times(case, MEASURE_S, 2);
    let mut pending = 0.0f64;
    let mut mouse_aim = false;
    let mut out = Vec::new();
    for (k, dt) in warm.iter().chain(measure.iter()).enumerate() {
        let warming = k < warm.len();
        let frame_pad = pad_of(case, warming, k);
        pending += f64::from(frame_pad.mx) * f64::from(core::sim::LOOK_UNIT);
        mouse_aim |= frame_pad.mouse_aim();
        let pad = PadState {
            mx: PadState::quantize_look(pending),
            ..frame_pad
        };
        let pad = walk.blend.stamp(pad);
        let ticks = walk.advance_with(*dt as f32, std::slice::from_ref(&pad), |_| {});
        if ticks > 0 {
            pending -= f64::from(pad.mx) * f64::from(core::sim::LOOK_UNIT);
        }
        walk.blend.consumed(ticks);
        let mut look = PadState {
            mx: PadState::quantize_look(pending),
            buttons: if mouse_aim { buttons::MOUSE_AIM } else { 0 },
            ..PadState::default()
        };
        if !look.has_look() && !look.mouse_aim() {
            look = PadState {
                mx: 0,
                my: 0,
                ..pad
            };
        }
        crate::render_blend::blend_in_looking(&mut walk, &look);
        if !warming {
            out.push(sample(&walk, *dt as f32));
        }
        walk.blend_out();
    }
    out
}

/// One LAN peer's game loop state.
struct Seat {
    peer: Peer,
    walk: Option<WalkSim>,
    view: LocalView,
    times: Vec<f64>,
    next: usize,
    at_us: f64,
    warm_frames: usize,
    samples: Vec<Sample>,
}

impl Seat {
    fn new(peer: Peer, case: &Case, seed: u64, warm_s: f64) -> Self {
        let phase = case.phase_us;
        let warm = frame_times(case, warm_s, seed);
        let warm_frames = warm.len();
        let mut times = warm;
        times.extend(frame_times(case, MEASURE_S, seed + 1));
        Self {
            peer,
            walk: None,
            view: LocalView::new(),
            times,
            next: 0,
            at_us: phase,
            warm_frames,
            samples: Vec::new(),
        }
    }

    fn done(&self) -> bool {
        self.next >= self.times.len()
    }

    /// The frames due by `now_us`, as the windowed loop runs them.
    fn frames<L: Link>(&mut self, now_us: u64, link: &mut L, case: &Case, slot: u8) {
        while !self.done() && self.at_us <= now_us as f64 {
            let dt = self.times[self.next];
            let k = self.next;
            self.next += 1;
            self.at_us += dt * 1e6;
            for n in self.peer.poll(link) {
                if let Notice::Load { setup, .. } = n {
                    let mut w = fixture(&setup, slot).unwrap();
                    w.blend.on = true;
                    self.peer.round_built(link, &mut w);
                    self.walk = Some(w);
                }
            }
            let Some(walk) = self.walk.as_mut() else {
                continue;
            };
            let warming = k < self.warm_frames;
            let pad = walk.blend.stamp(pad_of(case, warming, k));
            let report = self.view.frame(
                &mut self.peer,
                link,
                dt as f32,
                pad,
                walk,
                None,
                |_, _, _, _| {},
            );
            walk.blend.consumed(report.ticks.max(1));
            let hash = walk.sim.hash();
            let conf = walk.sim.units[walk.player.index as usize].soldier.yaw;
            self.view.draw_in(walk);
            if trace(k.saturating_sub(self.warm_frames)) && !warming {
                println!(
                    "{} f{} at{:.1} rate{:.4} slack{:?} pacing{:?} t{} st{} frac{:.3} other{:?} pend{} open{} share{:.3} conf{:.5} drawn{:.5} depth{} ease{:?} corr{}",
                    self.peer.local_slot,
                    k - self.warm_frames,
                    self.at_us / 1e3,
                    self.peer.pacer().dilation(),
                    link.slack_us(),
                    link.pacing(),
                    walk.sim.tick,
                    report.ticks,
                    self.view.fraction(),
                    other_of(walk, self.peer.local_slot),
                    self.peer.pending_inputs().len(),
                    self.peer.pacer().pending().len(),
                    self.view.share(),
                    conf,
                    walk.sim.units[walk.player.index as usize].soldier.yaw,
                    self.view.predictor.depth(),
                    self.view.predictor.ease().now(),
                    self.view.predictor.stats.corrections,
                );
            }
            if !warming {
                let mut s = sample(walk, dt as f32);
                s.ok &= walk.sim.players.get(usize::from(self.peer.local_slot))
                    == Some(&Some(walk.player));
                s.other = other_of(walk, self.peer.local_slot);
                self.samples.push(s);
            }
            self.view.draw_out(walk);
            assert_eq!(walk.sim.hash(), hash, "drawing reached the sim");
        }
    }
}

/// The host alone over a bare `HostCore`: its network thread closes ticks
/// on its own millisecond clock, the game renders on its frame clock.
pub(super) fn run_host(case: &Case) -> Run {
    let mut core = HostCore::new(HostConfig::new("test-build", "Host"));
    let mut seat = Seat::new(Peer::host(), case, 11, WARM_S);
    let bytes = seat.peer.host_load(&mut core, &setup());
    let mut w = fixture(&RoundSetup::decode(&bytes).unwrap(), 0).unwrap();
    w.blend.on = true;
    seat.peer.round_built(&mut core, &mut w);
    seat.walk = Some(w);
    let mut rng = Lcg(99);
    let mut now = 1_000u64;
    let mut stall_until = 0u64;
    while !seat.done() {
        if case.net_stalls && now >= stall_until && rng.unit() < 0.01 {
            stall_until = now + 4_000 + (rng.unit() * 12_000.0) as u64;
        }
        if now >= stall_until {
            core.update(Micros(now));
        }
        seat.frames(now, &mut core, case, 0);
        now += 1_000;
    }
    Run {
        samples: seat.samples,
        starved: core.pacing(0).map_or(0, |p| p.1),
    }
}

/// A run's drawn frames and the host's starved closes of the slot drawn.
pub(super) struct Run {
    pub samples: Vec<Sample>,
    pub starved: u32,
}

/// A host and a guest on a 1 ms LAN (the maintainer's two windows): the
/// guest's samples.
pub(super) fn run_guest(case: &Case) -> (Run, Run) {
    let link = LinkParams {
        latency: Micros(500),
        jitter: Micros(200),
        loss: 0.0,
        duplicate: 0.0,
        reorder: 0.0,
    };
    let (ha, ga) = (virtual_net::addr(0), virtual_net::addr(1));
    let mut net = LinkSim::new(5);
    net.set_default(link);
    net.add_endpoint(ha);
    net.add_endpoint(ga);
    let mut host_core = HostCore::new(HostConfig::new("test-build", "Host"));
    let mut host = Seat::new(Peer::host(), case, 21, WARM_S + GUEST_AT_S);
    let bytes = host.peer.host_load(&mut host_core, &setup());
    let mut w = fixture(&RoundSetup::decode(&bytes).unwrap(), 0).unwrap();
    w.blend.on = true;
    host.peer.round_built(&mut host_core, &mut w);
    host.walk = Some(w);
    let mut config = ClientConfig::new("test-build", "Guest", 42);
    config.conn_id = 1001;
    let mut guest_core = ClientCore::new(config, ha, Micros(1_000));
    let mut guest = Seat::new(Peer::client(), case, 31, WARM_S);
    // The guest joins a second in and renders on from there; the host
    // measures from the guest's start too (the other human runs then).
    guest.at_us += GUEST_AT_S * 1e6;
    let mut now = 1_000u64;
    let mut rng = Lcg(77);
    let mut stall_until = 0u64;
    while !guest.done() {
        if case.net_stalls && now >= stall_until && rng.unit() < 0.01 {
            stall_until = now + 4_000 + (rng.unit() * 12_000.0) as u64;
        }
        let net_up = now >= stall_until;
        net.advance(Micros(now));
        while let Some((from, b)) = net_up.then(|| net.recv(ha)).flatten() {
            host_core.handle_datagram(Micros(now), from, &b);
        }
        while let Some((from, b)) = net.recv(ga) {
            guest_core.handle_datagram(Micros(now), from, &b);
        }
        // The host's own frames keep going past its schedule: repeat it.
        if host.done() {
            host.next = host.warm_frames;
        }
        host.frames(now, &mut host_core, case, 0);
        if now as f64 >= GUEST_AT_S * 1e6 {
            let slot = guest.peer.local_slot;
            guest.frames(now, &mut guest_core, case, slot);
        }
        if net_up {
            for (to, b) in host_core.update(Micros(now)) {
                net.send(Micros(now), ha, to, &b);
            }
        }
        if now as f64 >= GUEST_AT_S * 1e6 {
            for (to, b) in guest_core.update(Micros(now)) {
                net.send(Micros(now), ga, to, &b);
            }
        }
        now += 1_000;
    }
    let run = |seat: Seat, slot: u8| Run {
        samples: seat.samples,
        starved: host_core.pacing(slot).map_or(0, |p| p.1),
    };
    (run(guest, 1), run(host, 0))
}

/// Per-frame steadiness of a series: each frame's step over its weight
/// (1 for mouse motion per frame, the frame time for a rate), against the
/// median.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Steadiness {
    pub frames: usize,
    pub median: f64,
    /// The largest |step − median| / median.
    pub max_dev: f64,
    /// The 99th percentile of it.
    pub p99_dev: f64,
    /// Frames off by more than 10%.
    pub over_10pct: usize,
    /// Steps against the motion.
    pub reversals: usize,
}

pub(super) fn steadiness(steps: &[(f64, f64)]) -> Steadiness {
    let r: Vec<f64> = steps.iter().map(|(d, w)| d / w).collect();
    if r.is_empty() {
        return Steadiness::default();
    }
    let mut sorted = r.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let mut devs: Vec<f64> = r
        .iter()
        .map(|x| (x - median).abs() / median.abs().max(1e-12))
        .collect();
    let reversals = r
        .iter()
        .filter(|x| x.signum() != median.signum() && x.abs() > 1e-9)
        .count();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Steadiness {
        frames: r.len(),
        median,
        max_dev: *devs.last().unwrap(),
        p99_dev: devs[(devs.len() * 99 / 100).min(devs.len() - 1)],
        over_10pct: devs.iter().filter(|d| **d > 0.10).count(),
        reversals,
    }
}

fn wrap(a: f32) -> f64 {
    let mut a = f64::from(a);
    while a > std::f64::consts::PI {
        a -= std::f64::consts::TAU;
    }
    while a < -std::f64::consts::PI {
        a += std::f64::consts::TAU;
    }
    a
}

/// What a run's drawn view did.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Report {
    pub yaw: Steadiness,
    pub cam_yaw: Steadiness,
    /// Drawn speed (m per s), when running.
    pub speed: Steadiness,
    /// The camera's motion: per frame for mouse motion standing, per
    /// second otherwise.
    pub cam_move: Steadiness,
    /// The other human's drawn speed, when it runs.
    pub other_speed: Steadiness,
    pub starved: u32,
}

/// Measured frames skipped first: the run and the turn ramp up.
const RAMP_S: f32 = 0.5;

pub(super) fn report(case: &Case, run: &Run) -> Report {
    let weight = |s: &Sample| match case.input {
        Input::Mouse(_) => 1.0,
        Input::Stick(_) => f64::from(s.dt),
    };
    let mut t = 0.0;
    let samples: Vec<&Sample> = run
        .samples
        .iter()
        .skip_while(|s| {
            t += s.dt;
            t < RAMP_S
        })
        .collect();
    let (mut yaw, mut cam_yaw, mut speed, mut cam_move, mut other) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for w in samples.windows(2) {
        let (a, b) = (w[0], w[1]);
        if !(a.ok && b.ok) {
            continue;
        }
        let dt = f64::from(b.dt);
        yaw.push((wrap(b.yaw - a.yaw), weight(b)));
        cam_yaw.push((wrap(b.cam_yaw - a.cam_yaw), weight(b)));
        let moved = f64::from(b.cam_pos.distance(a.cam_pos));
        if case.run {
            speed.push((f64::from(b.pos.distance(a.pos)), dt));
            cam_move.push((moved, dt));
            if let (Some(p), Some(q)) = (a.other, b.other) {
                other.push((f64::from(q.distance(p)), dt));
            }
        } else {
            cam_move.push((moved, weight(b)));
        }
    }
    Report {
        yaw: steadiness(&yaw),
        cam_yaw: steadiness(&cam_yaw),
        speed: steadiness(&speed),
        cam_move: steadiness(&cam_move),
        other_speed: steadiness(&other),
        starved: run.starved,
    }
}

fn show(name: &str, r: &Report) {
    let f = |s: &Steadiness| {
        format!(
            "n={:4} median={:8.4} max={:6.3} p99={:6.3} >10%={:4} rev={:3}",
            s.frames, s.median, s.max_dev, s.p99_dev, s.over_10pct, s.reversals
        )
    };
    println!("{name:<38} starved {:3}", r.starved);
    println!("{:<6} yaw [{}]", "", f(&r.yaw));
    println!("{:<6} cam [{}]", "", f(&r.cam_yaw));
    println!("{:<6} cmv [{}]", "", f(&r.cam_move));
    if r.speed.frames > 0 {
        println!("{:<6} spd [{}]", "", f(&r.speed));
    }
    if r.other_speed.frames > 0 {
        println!("{:<6} oth [{}]", "", f(&r.other_speed));
    }
}

/// Every combination, off the tick clock's phase, plus the 60 Hz frame
/// clock on it (`Case::phase_us`).
fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for hz in [60.0, 144.0] {
        for jitter in [false, true] {
            for input in [Input::Mouse(0.004), Input::Stick(0.6)] {
                for run in [false, true] {
                    out.push(Case {
                        hz,
                        jitter,
                        input,
                        run,
                        net_stalls: jitter,
                        phase_us: 5_000.0,
                    });
                }
            }
        }
    }
    for input in [Input::Mouse(0.004), Input::Stick(0.6)] {
        for run in [false, true] {
            out.push(Case {
                hz: 60.0,
                jitter: false,
                input,
                run,
                net_stalls: false,
                phase_us: 0.0,
            });
        }
    }
    out
}

fn case_name(case: &Case) -> String {
    format!(
        "{}Hz {}{} {:?} {}",
        case.hz,
        if case.jitter { "busy" } else { "even" },
        if case.phase_us == 0.0 {
            " on-close"
        } else {
            ""
        },
        case.input,
        if case.run { "run" } else { "stand" }
    )
}

/// The measurement table (run with `--ignored --nocapture`); trace frames
/// with `SMOOTH_TRACE=from..to`.
#[test]
#[ignore]
fn lan_view_smoothness_table() {
    for case in cases() {
        let name = case_name(&case);
        let sp = Run {
            samples: run_single_player(&case),
            starved: 0,
        };
        show(&format!("SP    {name}"), &report(&case, &sp));
        show(&format!("HOST  {name}"), &report(&case, &run_host(&case)));
        let (g, h) = run_guest(&case);
        show(&format!("GUEST {name}"), &report(&case, &g));
        show(&format!("HOST+ {name}"), &report(&case, &h));
    }
}

/// Issue #168 (the maintainer's playtest: "panning around with the camera
/// is really jittery ... tiny steps from one frame to the next", host and
/// guest): the drawn aim, camera and soldier on a LAN peer are as steady as
/// single player's, for every case.
///
/// Before the fixes, on this harness: the mouse turn stepped back and forth
/// (deviation up to 5.3x a frame's turn, 117 reversals in 6 s on the host
/// whose queue starved), a held stick lost its open tick's share at every
/// sent input (up to 5.5x), and a running soldier and his camera moved at
/// 0..2x their speed every frame (the drawn pose switched between the input
/// clock and the confirmed ticks).
#[test]
fn lan_view_is_as_steady_as_single_player() {
    let mut failures = Vec::new();
    for case in cases() {
        let sp = report(
            &case,
            &Run {
                samples: run_single_player(&case),
                starved: 0,
            },
        );
        let (g, h) = run_guest(&case);
        for (who, run) in [("host", run_host(&case)), ("guest", g), ("host+guest", h)] {
            let r = report(&case, &run);
            let name = format!("{who} {}", case_name(&case));
            let mut check = |what: &str, ok: bool, s: &Steadiness| {
                if !ok {
                    failures.push(format!("{name}: {what} {s:?} (starved {})", r.starved));
                }
            };
            assert!(
                r.yaw.frames > 200,
                "{name}: {} frames measured",
                r.yaw.frames
            );
            check(
                "reversed",
                r.yaw.reversals == 0 && r.cam_yaw.reversals == 0,
                &r.yaw,
            );
            let on_close = case.phase_us == 0.0;
            match case.input {
                // The same turn every frame, exactly.
                Input::Mouse(_) => {
                    check("mouse turn", r.yaw.max_dev < 0.01, &r.yaw);
                    check("camera turn", r.cam_yaw.max_dev < 0.01, &r.cam_yaw);
                }
                // As even as single player's (a starving queue's slips are
                // eased in, real turns the sim made).
                Input::Stick(_) => {
                    let room = if on_close { 0.5 } else { 0.1 };
                    check("stick turn", r.yaw.p99_dev <= sp.yaw.p99_dev + room, &r.yaw);
                    check(
                        "camera turn",
                        r.cam_yaw.p99_dev <= sp.cam_yaw.p99_dev + room,
                        &r.cam_yaw,
                    );
                }
            }
            if case.run {
                let room = if on_close || case.jitter { 0.2 } else { 0.05 };
                check(
                    "speed",
                    r.speed.p99_dev <= sp.speed.p99_dev + room,
                    &r.speed,
                );
                check(
                    "camera motion",
                    r.cam_move.p99_dev <= sp.cam_move.p99_dev + room,
                    &r.cam_move,
                );
                // The other human, drawn from the confirmed ticks, on a
                // steady frame clock.
                if !case.jitter && r.other_speed.frames > 0 {
                    check(
                        "other's speed",
                        r.other_speed.max_dev < 0.05,
                        &r.other_speed,
                    );
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The inputs a peer still previews and predicts are exactly those no
/// stepped tick took, through the host queue's starved and slipped closes:
/// every mouse count sent is either in a stepped bundle's pad or pending,
/// never both or neither (the host used to drop one per bundle, so a
/// starved close lost the aim of an input its tick never stepped).
#[test]
fn pending_inputs_are_exactly_the_ones_no_tick_took() {
    let mut core = HostCore::new(HostConfig::new("test-build", "Host"));
    let mut peer = Peer::host();
    let bytes = peer.host_load(&mut core, &setup());
    let mut walk = fixture(&RoundSetup::decode(&bytes).unwrap(), 0).unwrap();
    peer.round_built(&mut core, &mut walk);
    let (mut sent, mut stepped) = (0i64, 0i64);
    let mut now = 1_000u64;
    for frame in 0..600u32 {
        // Now and then the tick clock runs ahead of the inputs (a late
        // frame): its closes starve, then slip.
        let ahead = if frame % 40 == 20 { 45_000 } else { 0 };
        core.update(Micros(now + ahead));
        peer.poll(&mut core);
        let pad = PadState {
            mx: 100,
            buttons: buttons::MOUSE_AIM,
            ..PadState::default()
        };
        let report = peer.frame(&mut core, 1.0 / 60.0, pad, &mut walk, None, |_, b, _| {
            stepped += b.pad(0).map_or(0, |p| i64::from(p.mx));
        });
        sent += report
            .submitted
            .iter()
            .map(|p| i64::from(p.mx))
            .sum::<i64>();
        let pending: i64 = peer.pending_inputs().iter().map(|p| i64::from(p.mx)).sum();
        // (Pending inputs show once a bundle holds the slot.)
        if peer.stats.ticks_stepped > 0 {
            assert_eq!(stepped + pending, sent, "frame {frame}");
        }
        now += 16_667;
    }
    assert!(sent > 0);
    let starved = core.pacing(0).map_or(0, |p| p.1);
    assert!(starved >= 5, "the queue starved {starved} times");
}

/// The drawn aim turns the same every frame through the host queue's
/// starved and slipped closes: a starved tick took none of the inputs and
/// a slipped one two, so the ticks disagree with the old prediction about
/// the aim at that tick, but never about the aim after an input (the
/// correction is keyed by input; keyed by tick it eased a phantom aim error
/// in and out at every slip).
#[test]
fn the_drawn_aim_rides_through_starved_and_slipped_closes() {
    let mut core = HostCore::new(HostConfig::new("test-build", "Host"));
    let mut peer = Peer::host();
    let bytes = peer.host_load(&mut core, &setup());
    let mut walk = fixture(&RoundSetup::decode(&bytes).unwrap(), 0).unwrap();
    walk.blend.on = true;
    peer.round_built(&mut core, &mut walk);
    let mut view = LocalView::new();
    let mut now = 1_000u64;
    let mut yaws = Vec::new();
    for frame in 0..400u32 {
        let ahead = if frame % 40 == 20 { 45_000 } else { 0 };
        core.update(Micros(now + ahead));
        peer.poll(&mut core);
        let pad = PadState {
            mx: PadState::quantize_look(0.004),
            buttons: buttons::MOUSE_AIM,
            ..PadState::default()
        };
        view.frame(
            &mut peer,
            &mut core,
            1.0 / 60.0,
            pad,
            &mut walk,
            None,
            |_, _, _, _| {},
        );
        view.draw_in(&mut walk);
        yaws.push(walk.sim.units[walk.player.index as usize].soldier.yaw);
        view.draw_out(&mut walk);
        now += 16_667;
    }
    let steps: Vec<(f64, f64)> = yaws[60..]
        .windows(2)
        .map(|w| (wrap(w[1] - w[0]), 1.0))
        .collect();
    let s = steadiness(&steps);
    let starved = core.pacing(0).map_or(0, |p| p.1);
    assert!(starved >= 5, "the queue starved {starved} times");
    assert!(s.max_dev < 0.01 && s.reversals == 0, "{s:?}");
}
