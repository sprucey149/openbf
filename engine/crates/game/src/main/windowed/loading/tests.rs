use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

#[test]
fn skip_deadline_is_exact_and_expired_prompt_rearms() {
    let mut p = Playback::default();
    assert!(!p.prompt(Duration::ZERO));
    p.press(Duration::from_secs(1));
    assert!(p.prompt(Duration::from_millis(3999)));
    assert!(!p.prompt(Duration::from_secs(4)));
    p.press(Duration::from_secs(4));
    assert!(!p.done(Duration::from_secs(4), Duration::from_secs(10)));
    assert!(p.prompt(Duration::from_millis(6999)));
    p.press(Duration::from_millis(6999));
    assert!(p.done(Duration::from_millis(6999), Duration::from_secs(10)));
    assert!(!p.prompt(Duration::from_millis(6999)));
}

#[test]
fn launch_holds_repeat_motion_and_release_do_not_skip() {
    let mut e = Edges::<u8>::default();
    assert!(e.update(1, true, false).0); // shell launch
    let mut p = Playback::default();
    for (down, repeat) in [(true, false), (true, true), (false, false)] {
        if e.update(1, down, repeat).0 {
            p.press(Duration::ZERO);
        }
    }
    assert!(!p.prompt(Duration::ZERO));
    for b in [1, 99] {
        // a released controller/keyboard and an unbound button
        if e.update(b, true, false).0 {
            p.press(Duration::from_millis(20));
        }
    }
    assert!(p.done(Duration::from_millis(20), Duration::from_secs(4)));
    e.quarantine();
    assert_eq!(e.update(99, true, true), (false, true));
    assert_eq!(e.update(99, false, false), (false, true));
    assert_eq!(e.update(99, true, false), (true, false));
}

#[test]
fn playback_and_readiness_are_independent() {
    let mut p = Playback::default();
    assert!(!p.done(Duration::from_millis(999), Duration::from_secs(1)));
    assert!(p.done(Duration::from_secs(1), Duration::from_secs(1)));
    assert_eq!(
        p.time(Duration::from_millis(1500)),
        Duration::from_millis(1500)
    );
    p.press(Duration::ZERO);
    p.press(Duration::from_millis(1));
    assert!(p.done(Duration::from_millis(1), Duration::from_secs(4)));
    assert!(!p.can_enter(Duration::from_secs(10), Duration::from_secs(4), true, false));
    assert!(!p.can_enter(Duration::from_secs(10), Duration::from_secs(4), false, true));
    assert!(p.can_enter(Duration::from_millis(1), Duration::from_secs(4), true, true));
    assert_eq!(
        p.time(Duration::from_secs(10)),
        Duration::from_millis(1),
        "skip freezes playback while CPU preparation finishes"
    );
    // Readiness must still gate the transition after a skip (tested with a
    // worker blocked by a channel rather than a timing-dependent sleep).
    let (tx, rx) = mpsc::channel();
    let mut w = Worker::spawn(move || {
        rx.recv()?;
        Ok(7)
    })
    .unwrap();
    assert!(w.poll().is_none());
    tx.send(()).unwrap();
    assert_eq!(await_worker(&mut w).unwrap(), 7);
    assert!(w.poll().is_none());
}

fn await_worker<T: Send + 'static>(w: &mut Worker<T>) -> anyhow::Result<T> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(r) = w.poll() {
            return r;
        }
        assert!(Instant::now() < deadline, "worker timed out");
        std::thread::yield_now();
    }
}

#[test]
fn worker_failure_panic_close_and_next_round_are_owned() {
    let mut bad = Worker::<()>::spawn(|| anyhow::bail!("broken input")).unwrap();
    assert!(await_worker(&mut bad)
        .unwrap_err()
        .to_string()
        .contains("broken input"));
    let mut panic = Worker::<()>::spawn(|| panic!("test panic")).unwrap();
    assert!(await_worker(&mut panic)
        .unwrap_err()
        .to_string()
        .contains("panicked"));
    let ended = Arc::new(AtomicBool::new(false));
    let end = ended.clone();
    let w = Worker::spawn(move || {
        end.store(true, Ordering::SeqCst);
        Ok(())
    })
    .unwrap();
    drop(w); // an in-game round change's drop joins; only the close abandons
    assert!(ended.load(Ordering::SeqCst));
    let mut next = Worker::spawn(|| Ok(42)).unwrap();
    assert_eq!(await_worker(&mut next).unwrap(), 42);
}

#[test]
fn close_detaches_a_blocked_loading_worker() {
    let (release, wait) = mpsc::channel::<()>();
    let ended = Arc::new(AtomicBool::new(false));
    let end = ended.clone();
    let mut w = Worker::<()>::spawn(move || {
        let _ = wait.recv();
        end.store(true, Ordering::SeqCst);
        Ok(())
    })
    .unwrap();
    let started = Instant::now();
    w.abandon();
    drop(w);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(!ended.load(Ordering::SeqCst), "not joined");
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ended.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "the detached thread still ends");
        std::thread::yield_now();
    }
}

#[test]
fn bf2_locations_use_only_same_planet_loading_donors() {
    for (key, donor) in [
        ("bf2_end1", "bf1_end1"),
        ("bf2_geo1", "bf1_geo1"),
        ("bf2_hot1", "bf1_hot1"),
        ("bf2_kam1", "bf1_kam1"),
        ("bf2_kas2", "bf1_kas2"),
        ("bf2_nab2", "bf1_nab2"),
        ("bf2_yav1", "bf1_yav1"),
        ("tat2", "bf1_tat2"),
        ("bf2_tat2", "bf1_tat2"),
        ("bf2_tat3", "tat1"),
        ("bf2_spa1", "bf1_yav1"),
        ("bf2_spa3", "bf1_kas2"),
        ("bf2_spa8", "bf1_hot1"),
        ("bf2_spa9", "tat1"),
    ] {
        assert_eq!(bf1_donor(key), Some(donor));
    }
    for key in [
        "bf2_cor1", "bf2_dag1", "bf2_dea1", "bf2_fel1", "bf2_mus1", "bf2_myg1", "bf2_pol1",
        "bf2_tan1", "bf2_uta1", "bf2_spa6", "bf2_spa7",
    ] {
        assert_eq!(bf1_donor(key), None);
    }
    assert!(presentation(Path::new("/nonexistent/loading-assets"), "bf2_pol1").is_none());
}

fn app(root: PathBuf) -> super::super::App {
    super::super::App::new(
        root,
        game::assets::baked_root()
            .parent()
            .unwrap()
            .join("missions"),
        super::super::tests::setup(None),
        None,
        None,
        None,
        Default::default(),
    )
}

fn row(key: &str, mode: ui::shell::Mode) -> ui::shell::Launch {
    ui::shell::Launch {
        key: key.into(),
        game: if key.starts_with("bf1") {
            ui::shell::Game::Bf1
        } else {
            ui::shell::Game::Bf2
        },
        mode,
        armies: ["all".into(), "imp".into()],
        label: key.into(),
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "openbf-loading-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn image(&self, key: &str, name: &str) {
        let dir = self.0.join(key).join("load");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{name}.ktx2")),
            bake::ktx2w::write_rgba8(2, 2, &[vec![128; 16]]),
        )
        .unwrap();
    }

    fn map(&self, key: &str, names: &[&str], pool: bool) {
        let dir = self.0.join(key);
        std::fs::create_dir_all(dir.join("load")).unwrap();
        std::fs::write(
            dir.join("manifest.ron"),
            format!(
                "(map: \"{key}\", source_game: \"{}\")",
                if key.starts_with("bf1") { "bf1" } else { "bf2" }
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("load/load.ron"),
            format!(
                "(sequence: {names:?}, extras: [], style: {})",
                if pool { "Pool" } else { "Sequence" }
            ),
        )
        .unwrap();
        for name in names {
            self.image(key, name);
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn presentation_uses_only_authored_donors_and_the_destinations_own_still() {
    let f = Fixture::new();
    f.map("bf1_bes1", &["load_galacticmap_0", "bespin_ground"], false);
    f.map("bf1_hot1", &["load_galacticmap_0", "hoth_ground"], false);
    f.map("bf2_pol1", &["native_bf2_still"], true);
    assert_eq!(presentation(&f.0, "bf1_bes1").unwrap().sequence.len(), 2);
    let seq = presentation(&f.0, "bf2_hot1").unwrap();
    assert_eq!(seq.sequence[1].0, "hoth_ground");
    let pol = presentation(&f.0, "bf2_pol1").unwrap();
    assert_eq!(pol.sequence[0].0, "load_galacticmap_0");
    assert_eq!(pol.sequence[1].0, "native_bf2_still");
    assert!(matches!(pol.motion, Motion::Arrival));
    std::fs::write(f.0.join("bf1_hot1/load/hoth_ground.ktx2"), "corrupt").unwrap();
    let seq = presentation(&f.0, "bf2_hot1").unwrap();
    assert_eq!(
        seq.sequence.len(),
        1,
        "broken matching sequence falls back to galaxy only"
    );
    // Valid KTX header with a short base mip must not reach ColorImage's assert.
    let mut short = bake::ktx2w::write_rgba8(2, 2, &[vec![0; 16]]);
    short[20..24].copy_from_slice(&3u32.to_le_bytes());
    std::fs::write(f.0.join("bf1_bes1/load/load_galacticmap_0.ktx2"), short).unwrap();
    let pol = presentation(&f.0, "bf2_pol1").unwrap();
    assert_eq!(pol.sequence[0].0, "load_galacticmap_0");
    assert!(matches!(pol.motion, Motion::Arrival));
    std::fs::remove_file(f.0.join("bf1_hot1/load/load_galacticmap_0.ktx2")).unwrap();
    let pol = presentation(&f.0, "bf2_pol1").unwrap();
    assert_eq!(pol.sequence.len(), 1);
    assert_eq!(pol.sequence[0].0, "native_bf2_still");
    assert!(matches!(pol.motion, Motion::Still));
    f.image("common_bf2", "orbit_polismassa");
    let pol = presentation(&f.0, "bf2_pol1").unwrap();
    assert_eq!(pol.sequence[0].0, "orbit_polismassa");
    assert_eq!(pol.sequence[1].0, "native_bf2_still");
    assert!(matches!(pol.motion, Motion::Arrival));
    f.image("bf1_bes1", "load_galacticmap_0");
    let pol = presentation(&f.0, "bf2_pol1").unwrap();
    assert_eq!(pol.sequence.len(), 3);
    assert_eq!(pol.sequence[0].0, "load_galacticmap_0");
    assert_eq!(pol.sequence[1].0, "orbit_polismassa");
    assert_eq!(pol.sequence[2].0, "native_bf2_still");

    let mut worker = prepare(f.0.join("bf2_pol1"), "con".into()).unwrap();
    assert!(
        await_worker(&mut worker).is_err(),
        "discoverable but broken scene reports an error"
    );
}

#[test]
fn bf2_arrival_picks_era_appropriate_still_and_recovers_from_missing_preferred_art() {
    let f = Fixture::new();
    f.map("bf2_spa6", &["spa_1", "spa_6"], true);
    f.map("bf2_spa7", &["spa_1", "spa_7"], true);
    f.image("common_bf2", "orbit_mygeeto");
    f.image("common_bf2", "orbit_felucia");
    f.map("bf1_bes1", &["load_galacticmap_0", "bespin_ground"], false);

    let sequence = |key| {
        presentation(&f.0, key)
            .unwrap()
            .sequence
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        sequence("bf2_spa6"),
        ["load_galacticmap_0", "orbit_mygeeto", "spa_6"]
    );
    assert_eq!(
        sequence("bf2_spa7"),
        ["load_galacticmap_0", "orbit_felucia", "spa_7"]
    );
    std::fs::remove_file(f.0.join("bf2_spa6/load/spa_6.ktx2")).unwrap();
    assert_eq!(
        sequence("bf2_spa6"),
        ["load_galacticmap_0", "orbit_mygeeto", "spa_1"]
    );
}

#[test]
fn playlist_recovers_from_failed_worker_and_keeps_mode_and_armies() {
    let f = Fixture::new();
    f.map("bf1_bes1", &[], false);
    f.map("bf2_pol1", &[], false);
    let mut a = app(f.0.clone());
    let mut next = row("bf2_pol1", ui::shell::Mode::Hunt);
    next.armies = ["rep".into(), "cis".into()];
    a.start_playlist(
        vec![
            row("missing", ui::shell::Mode::Con),
            row("bf1_bes1", ui::shell::Mode::Con),
            next,
        ],
        Default::default(),
    );
    let first = a.loading.as_mut().unwrap();
    assert!(await_worker(&mut first.worker).is_err());
    a.finish_loading();
    a.loading_failed();
    assert!(matches!(a.screen, super::super::Screen::Loading));
    assert_eq!(a.mode, "hunt");
    assert_eq!(a.current.as_ref().unwrap().armies, ["rep", "cis"]);
    assert_eq!(a.loading.as_ref().unwrap().map_name, "bf2_pol1");
    a.finish_loading(); // close cleans up even before the new worker is ready
    a.loading_failed();
    assert!(matches!(a.screen, super::super::Screen::Shell));
    assert!(a.loading.is_none());
    assert!(a.current.is_none());
}

#[test]
fn raw_mouse_buttons_are_consumed_through_release_and_motion_is_not_a_press() {
    use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};
    let f = Fixture::new();
    f.map("bf1_bes1", &[], false);
    let mut a = app(f.0.clone());
    let event = |state| WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state,
        button: MouseButton::Other(31),
    };
    assert!(!a.loading_window_event(&event(ElementState::Pressed))); // launch
    assert!(a.begin_loading(row("bf1_bes1", ui::shell::Mode::Con)));
    assert!(a.loading_window_event(&event(ElementState::Pressed)));
    assert!(!a.loading.as_ref().unwrap().playback.prompt(Duration::ZERO));
    assert!(!a.loading_window_event(&WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: winit::dpi::PhysicalPosition::new(2.0, 3.0),
    }));
    assert!(a.loading_window_event(&event(ElementState::Released)));
    assert!(a.loading_window_event(&event(ElementState::Pressed)));
    assert!(a
        .loading
        .as_ref()
        .unwrap()
        .playback
        .prompt(a.loading.as_ref().unwrap().started.elapsed()));
    let second = WindowEvent::MouseInput {
        device_id: DeviceId::dummy(),
        state: ElementState::Pressed,
        button: MouseButton::Middle,
    };
    assert!(a.loading_window_event(&second));
    assert!(a
        .loading
        .as_ref()
        .unwrap()
        .playback
        .done(Duration::ZERO, Duration::from_secs(4)));
    a.finish_loading();
    a.init_shell();
    assert!(a.loading_window_event(&event(ElementState::Pressed)));
    assert!(a.loading_window_event(&event(ElementState::Released)));
    assert!(!a.loading_window_event(&event(ElementState::Pressed)));
    assert_eq!((a.keys_held, a.keys_tapped), (0, 0));
}

/// Real runtime halves without a window: render the same loading UI while its
/// owned worker parses, then run the actual event-thread upload/setup and check
/// the untouched synchronous scene + setup produces the same initial sim.
#[test]
#[ignore = "requires GPU, LOADING_ASSETS, LOADING_OUT and owned OPENBF_PROFILE"]
fn capture_loading_overlap_and_setup() {
    let root = PathBuf::from(std::env::var_os("LOADING_ASSETS").expect("LOADING_ASSETS"));
    let out = PathBuf::from(std::env::var_os("LOADING_OUT").expect("LOADING_OUT"));
    assert!(
        std::env::var_os("OPENBF_PROFILE").is_some(),
        "use an owned test profile"
    );
    std::fs::create_dir_all(&out).unwrap();
    let mut receipts = Vec::new();
    let context = ui::egui::Context::default();
    for key in [
        "bf2_end1", "bf2_geo1", "bf2_hot1", "bf2_kam1", "bf2_kas2", "bf2_nab2", "bf2_yav1", "tat2",
        "bf2_tat3", "bf2_spa1", "bf2_spa3", "bf2_spa8", "bf2_spa9",
    ] {
        let display = Display::load(&root, key, &context);
        assert!(
            display.sample(Duration::ZERO).is_some(),
            "{key} must play its planet's sequence"
        );
        assert_eq!(display.duration(), Duration::from_millis(13_500), "{key}");
    }
    for key in [
        "bf2_cor1", "bf2_dag1", "bf2_dea1", "bf2_fel1", "bf2_mus1", "bf2_myg1", "bf2_pol1",
        "bf2_tan1", "bf2_uta1", "bf2_spa6", "bf2_spa7",
    ] {
        let assets = presentation(&root, key).expect("owned arrival art");
        assert!(matches!(assets.motion, Motion::Arrival), "{key}");
        assert!(
            assets.sequence[0].0.starts_with("load_galacticmap"),
            "{key}"
        );
        assert_eq!(assets.sequence.len(), 3, "{key}");
        let orbit = bf2_orbit(key).unwrap_or("load_locale_tatooine_1");
        assert_eq!(assets.sequence[1].0, orbit, "{key}");
        let (_, still) = &assets.sequence[2];
        assert_eq!(still.width * 3, still.height * 4, "{key}");
        let display = Display::load(&root, key, &context);
        assert_eq!(display.duration(), Duration::from_millis(9_000), "{key}");
        assert_eq!(display.sample(Duration::ZERO).unwrap().step, 0);
        assert_eq!(
            display.sample(Duration::from_millis(4_500)).unwrap().step,
            1
        );
        assert_eq!(display.sample(display.duration()).unwrap().step, 2);
    }
    let mut coruscant = app(root.clone());
    coruscant.renderer = Some(render::Renderer::new_headless(640, 480).unwrap());
    let mut session = ui::shell::SessionOptions::default();
    for _ in 0..3 {
        session.adjust(ui::shell::Opt::ConMult, -1, 1);
    }
    assert_eq!(session.con_mult, 70);
    coruscant.start_playlist(vec![row("bf2_cor1", ui::shell::Mode::Con)], session);
    let loading = coruscant.loading.as_mut().unwrap();
    let prepared = await_worker(&mut loading.worker).unwrap();
    coruscant.finish_loading();
    coruscant.start_battle("bf2_cor1".into(), prepared).unwrap();
    let walk = &coruscant.walk.as_ref().unwrap().0;
    let tickets = walk.sim.battle.teams.map(|team| team.tickets());
    assert_eq!(tickets, [145, 145]);
    println!("Coruscant at {}%: {tickets:?}", walk.reinforcement_pct);
    drop(coruscant);
    for key in ["bf1_bes1", "bf2_hot1", "bf2_pol1"] {
        let mut a = app(root.clone());
        let mut r = render::Renderer::new_headless(1280, 720).unwrap();
        let mut overlay = ui::overlay::EguiOverlay::headless(r.device(), r.surface_format());
        let mut worker = prepare(root.join(key), "con".into()).unwrap();
        let start = Instant::now();
        let ls = presentation(&root, key).expect("owned BF1 loading art");
        if key == "bf2_pol1" {
            assert_eq!(ls.sequence[1].0, "orbit_polismassa");
            assert_eq!(ls.sequence[2].0, "pol_1");
        }
        if key == "bf2_hot1" {
            assert!(ls.sequence.iter().any(|(n, _)| n.contains("hoth")));
        }
        let display = Display::load(&root, key, overlay.ctx());
        assert!(
            display.sample(Duration::ZERO).is_some(),
            "{key} must animate"
        );
        let millis = if key == "bf2_pol1" { 9_000 } else { 13_500 };
        assert_eq!(display.duration(), Duration::from_millis(millis));
        let mut frames_pending = 0;
        let prepared = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
                let frame = display.sample(Duration::ZERO);
                display.draw(ctx, key, Duration::ZERO, frame.as_ref(), false);
            });
            r.render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, view, size| overlay.paint(&paint, dev, q, enc, view, size)),
            )
            .unwrap();
            frames_pending += 1;
        };
        let ready_ms = start.elapsed().as_secs_f64() * 1000.0;
        let cpu_ms = prepared.cpu_ms;
        // Keep a deterministic blocked worker for the skipped-waiting case;
        // actual parse can be too fast for any one headless frame to catch it.
        let (tx, rx) = mpsc::channel();
        let mut held = Worker::spawn(move || {
            rx.recv()?;
            Ok(())
        })
        .unwrap();
        let mut p = Playback::default();
        let mut images = Vec::new();
        for (name, at, press) in [
            ("initial", 0, false),
            ("armed", 10, true),
            ("expired", 3010, false),
            ("rearmed", 3020, true),
            ("skipped-waiting", 3030, true),
        ] {
            let t = Duration::from_millis(at);
            if press {
                p.press(t);
            }
            assert!(held.poll().is_none());
            // Freeze the animation in the prompt comparison so unrelated
            // moving pixels cannot make a missing prompt test pass.
            let time = Duration::ZERO;
            let frame = display.sample(time);
            let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
                display.draw(ctx, key, time, frame.as_ref(), p.prompt(t));
            });
            r.render_with_overlay(
                &render::Camera::default(),
                &[],
                [0.0, 0.0, 0.0, 1.0],
                Some(&mut |dev, q, enc, view, size| overlay.paint(&paint, dev, q, enc, view, size)),
            )
            .unwrap();
            let rgba = r.capture_rgba().unwrap().0;
            std::fs::write(
                out.join(format!("{key}-{name}.png")),
                render::encode_png(&rgba, 1280, 720).unwrap(),
            )
            .unwrap();
            images.push(rgba);
        }
        let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
            ui::loading::draw_loading_with_skip(ctx, None, key, 0, 1, true)
        });
        r.render_with_overlay(
            &render::Camera::default(),
            &[],
            [0.0, 0.0, 0.0, 1.0],
            Some(&mut |dev, q, enc, view, size| overlay.paint(&paint, dev, q, enc, view, size)),
        )
        .unwrap();
        let flat = r.capture_rgba().unwrap().0;
        assert!(
            flat.chunks_exact(4)
                .any(|p| p[0] != 0 || p[1] != 0 || p[2] != 0),
            "fallback frame must remain visible"
        );
        std::fs::write(
            out.join(format!("{key}-missing-art.png")),
            render::encode_png(&flat, 1280, 720).unwrap(),
        )
        .unwrap();
        assert_ne!(images[0], images[1], "armed prompt must draw");
        assert_ne!(
            images[2], images[3],
            "expired prompt must disappear, then rearm"
        );
        assert!(p.done(Duration::from_millis(3030), display.duration()));
        if display.authored.is_some() {
            for (name, ms) in [
                ("vertical", 300),
                ("horizontal", 2000),
                ("locked", 3300),
                ("zoom", 4000),
                ("locale", 4900),
                ("landsat", 9800),
                ("level", 13500),
            ] {
                let time = Duration::from_millis(ms);
                let frame = display.sample(time);
                let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
                    display.draw(ctx, key, time, frame.as_ref(), false);
                });
                r.render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.0, 0.0, 0.0, 1.0],
                    Some(&mut |dev, q, enc, view, size| {
                        overlay.paint(&paint, dev, q, enc, view, size)
                    }),
                )
                .unwrap();
                let rgba = r.capture_rgba().unwrap().0;
                std::fs::write(
                    out.join(format!("{key}-{name}.png")),
                    render::encode_png(&rgba, 1280, 720).unwrap(),
                )
                .unwrap();
            }
        }
        tx.send(()).unwrap();
        await_worker(&mut held).unwrap();
        a.renderer = Some(r);
        a.overlay = Some(overlay);
        a.mode = "con".into();
        a.current = Some(row(key, ui::shell::Mode::Con));
        let setup_start = Instant::now();
        a.start_battle(key.into(), prepared).unwrap();
        let event_boundary_ms = setup_start.elapsed().as_secs_f64() * 1000.0;
        let walk = &a.walk.as_ref().unwrap().0;
        assert!(matches!(a.screen, super::super::Screen::Battle));
        assert_eq!(walk.sim.tick, 0);
        assert_eq!(walk.sim.spawn_stage[0], core::sim::SpawnStage::Side);
        let hash = walk.sim.hash();
        a.renderer.as_mut().unwrap().clear_scene();
        let (direct_gpu, scene) =
            super::super::load_and_upload(a.renderer.as_mut().unwrap(), &root.join(key), "con", &[])
                .unwrap();
        assert_eq!(direct_gpu.draws.len(), a.gpu.as_ref().unwrap().draws.len());
        let mut control = super::super::make_walk(&scene, &a.setup).unwrap();
        a.cheats.install(&mut control.sim);
        control
            .sim
            .install_medal_career(control.player, a.profile.career.medals);
        control.sim.assist_intensity = a.profile.settings.assist_intensity;
        control.sim.assist_armed = a.profile.settings.aim_assist;
        control.sim.team_handicap_on = a.profile.settings.team_handicap_on;
        super::super::super::battle::open_spawn_screens(&mut control, a.setup.mission.as_ref());
        assert_eq!(
            control.sim.hash(),
            hash,
            "worker path must preserve initial gameplay state"
        );
        // The round's CPU side on the worker (LAN §10.5): the same tick-0
        // sim, and the event thread's share of the boundary measured again.
        let mut b = app(root.clone());
        let mut r = a.renderer.take().unwrap();
        r.clear_scene();
        b.renderer = Some(r);
        b.overlay = a.overlay.take();
        b.mode = "con".into();
        b.current = Some(row(key, ui::shell::Mode::Con));
        let job = b.round_job(key, "con", b.current.as_ref());
        let mut worker = prepare_round(root.join(key), "con".into(), Some(job)).unwrap();
        let mut frames_round = 0;
        let started = Instant::now();
        let prepared = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            let overlay = b.overlay.as_mut().unwrap();
            let paint = overlay.begin_headless([1280, 720], 1.0, |ctx| {
                let frame = display.sample(Duration::ZERO);
                display.draw(ctx, key, Duration::ZERO, frame.as_ref(), false);
            });
            b.renderer
                .as_mut()
                .unwrap()
                .render_with_overlay(
                    &render::Camera::default(),
                    &[],
                    [0.0, 0.0, 0.0, 1.0],
                    Some(&mut |dev, q, enc, view, size| {
                        overlay.paint(&paint, dev, q, enc, view, size)
                    }),
                )
                .unwrap();
            frames_round += 1;
        };
        let round_ready_ms = started.elapsed().as_secs_f64() * 1000.0;
        let laps = prepared.round.as_ref().map(|r| r.laps.clone());
        let boundary = Instant::now();
        b.start_battle(key.into(), prepared).unwrap();
        let worker_boundary_ms = boundary.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            b.walk.as_ref().unwrap().0.sim.hash(),
            hash,
            "the round built on the worker must equal the serial build"
        );
        println!(
            "{key}: event-thread boundary serial={event_boundary_ms:.1} ms \
             worker={worker_boundary_ms:.1} ms ({laps:?})"
        );
        receipts.push(serde_json::json!({"map":key,"cpu_parse_ms":cpu_ms,"ready_wall_ms":ready_ms,"frames_while_pending":frames_pending,"upload_setup_ms":event_boundary_ms,"initial_hash":format!("{hash:016x}"),"worker_ready_wall_ms":round_ready_ms,"worker_frames_while_pending":frames_round,"worker_upload_setup_ms":worker_boundary_ms,"worker_laps":laps}));
    }
    let (full_ms, full_hash) = hoth_team_screen_time(&root, false);
    let (skip_ms, skip_hash) = hoth_team_screen_time(&root, true);
    assert_eq!(skip_hash, full_hash, "skip must preserve the launch state");
    println!("Hoth team screen: full={full_ms:.1} ms skip={skip_ms:.1} ms");
    receipts.push(serde_json::json!({"map":"bf2_hot1", "full_team_screen_ms":full_ms, "skip_team_screen_ms":skip_ms}));
    std::fs::write(
        out.join("overlap.json"),
        serde_json::to_string_pretty(&receipts).unwrap(),
    )
    .unwrap();
}

fn hoth_team_screen_time(root: &Path, skip: bool) -> (f64, u64) {
    use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};

    let mut a = app(root.to_owned());
    let renderer = render::Renderer::new_headless(1280, 720).unwrap();
    a.overlay = Some(ui::overlay::EguiOverlay::headless(
        renderer.device(),
        renderer.surface_format(),
    ));
    a.renderer = Some(renderer);
    let start = Instant::now();
    assert!(a.begin_loading(row("bf2_hot1", ui::shell::Mode::Con)));
    assert!(a.loading.as_ref().unwrap().display.textures.len() > 1);
    a.redraw_loading().unwrap();
    if skip {
        for state in [
            ElementState::Pressed,
            ElementState::Released,
            ElementState::Pressed,
        ] {
            assert!(a.loading_window_event(&WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state,
                button: MouseButton::Other(31),
            }));
        }
        let loading = a.loading.as_ref().unwrap();
        assert!(loading
            .playback
            .done(loading.started.elapsed(), loading.display.duration()));
    }
    while matches!(a.screen, super::super::Screen::Loading) {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "Hoth launch timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
        a.redraw_loading().unwrap();
    }
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert!(matches!(a.screen, super::super::Screen::Battle));
    let sim = &a.walk.as_ref().unwrap().0.sim;
    assert_eq!(sim.spawn_stage[0], core::sim::SpawnStage::Side);
    assert_eq!(sim.tick, 0);
    (elapsed_ms, sim.hash())
}

#[test]
fn held_controller_must_release_and_unbound_button_can_confirm_skip() {
    let mut edges = Edges::<(u8, u32)>::default();
    let accept = (1, 10);
    let unbound = (1, 0xffff);
    edges.update(accept, true, false); // already held while entering loading
    let mut p = Playback::default();
    assert!(!edges.update(accept, true, false).0);
    assert!(!edges.update(accept, false, false).0);
    assert!(edges.update(accept, true, false).0);
    p.press(Duration::from_millis(5));
    assert!(p.prompt(Duration::from_millis(5)));
    assert!(!edges.update(accept, true, true).0);
    assert!(!p.done(Duration::from_millis(6), Duration::from_secs(4)));
    assert!(edges.update(unbound, true, false).0);
    p.press(Duration::from_millis(7));
    assert!(p.done(Duration::from_millis(7), Duration::from_secs(4)));
    edges.quarantine();
    assert_eq!(edges.update(accept, false, false), (false, true));
    assert_eq!(edges.update(unbound, false, false), (false, true));
    assert_eq!(edges.update(accept, true, false), (true, false));
}

#[test]
fn controller_hold_is_neutral_until_release_then_allows_new_input() {
    let mut blocked = true;
    assert!(suppress_pad_until_release(&mut blocked, true));
    assert!(suppress_pad_until_release(&mut blocked, true));
    assert!(!suppress_pad_until_release(&mut blocked, false));
    assert!(!suppress_pad_until_release(&mut blocked, true));
    let pads = input::usable_pads([
        (1u8, input::PadKind::Unified, Some((1, 2))),
        (2u8, input::PadKind::Raw, Some((1, 2))),
    ]);
    assert_eq!(
        pads,
        vec![(1, input::PadKind::Unified)],
        "one physical press must have one usable interface"
    );
}

/// Issue #168: the LAN loading gate. A round start never ends before its
/// presentation (the first VPN playtest's skip: it ended when the round was
/// built), holds the installed battle until the start and shows it then; a
/// peer the start went on without installs when built; a join enters when
/// built; single player is unchanged.
#[test]
fn lan_round_start_holds_until_the_hosts_start() {
    let now = Instant::now();
    let soon = now + Duration::from_millis(120);
    let start = |s| Some((false, s));
    // Single player: the presentation's end.
    assert_eq!(next(None, false, true, false, now), Next::Draw);
    assert_eq!(next(None, true, true, false, now), Next::Enter);
    // A round start built before its presentation's end keeps drawing.
    assert_eq!(next(start(None), false, true, false, now), Next::Draw);
    assert_eq!(next(start(None), true, true, false, now), Next::Hold);
    // Held: until the start's moment, then shown.
    assert_eq!(next(start(None), true, false, true, now), Next::Draw);
    assert_eq!(next(start(Some(soon)), true, false, true, now), Next::Draw);
    assert_eq!(next(start(Some(soon)), true, false, true, soon), Next::Show);
    // Started without it (the barrier's timeout): in as soon as built.
    assert_eq!(next(start(Some(now)), false, false, false, now), Next::Draw);
    assert_eq!(next(start(Some(now)), false, true, false, now), Next::Hold);
    // A join or a rejoin catches up: in when built, no hold.
    assert_eq!(
        next(Some((true, None)), false, true, false, now),
        Next::Enter
    );
    assert_eq!(
        next(Some((true, None)), false, false, false, now),
        Next::Draw
    );
}

/// A LAN guest's playback: no press prompts or skips; only the host's
/// skip ends it.
#[test]
fn lan_guest_playback_ignores_presses_until_the_host_skips() {
    let mut p = Playback::no_skip();
    for ms in [0, 1, 5, 20, 2_000] {
        p.press(Duration::from_millis(ms));
        assert!(!p.prompt(Duration::from_millis(ms)));
    }
    assert!(!p.skipped());
    assert!(!p.done(Duration::from_millis(2_000), Duration::from_secs(4)));
    assert!(p.done(Duration::from_secs(4), Duration::from_secs(4)));
    p.host_skipped(Duration::from_millis(2_100));
    assert!(p.skipped() && !p.prompt(Duration::from_millis(2_100)));
    assert!(p.done(Duration::from_millis(2_100), Duration::from_secs(4)));
    assert_eq!(p.time(Duration::from_secs(3)), Duration::from_millis(2_100));
}

/// The fixture's `bf1_bes1` with one still (a 1 s presentation).
fn one_still_fixture() -> Fixture {
    let f = Fixture::new();
    f.map("bf1_bes1", &[], false);
    std::fs::write(
        f.0.join("bf1_bes1/manifest.ron"),
        "(map: \"bf1_bes1\", source_game: \"bf1\", terrain_name: None, sky_name: None, \
         terrain_glb: None, models: [], textures: [], classes: [], geometries: [], \
         regions: [], instances: [])",
    )
    .unwrap();
    f
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.local_addr())
        .map(|a| a.port())
        .unwrap()
}

fn lan_host(port: u16) -> super::super::lan::NetplaySession {
    let args =
        super::super::lan::LanArgs::parse(&["--port".to_owned(), port.to_string()], true).unwrap();
    let mut profile = game::netplay::PlayerProfile::decode_or_default(&[]);
    profile.name = "Host".into();
    super::super::lan::NetplaySession::host(args, profile, 4, true).unwrap()
}

fn lan_setup(slots: &[u8]) -> game::netplay::RoundSetup {
    let mut s = game::netplay::RoundSetup::instant_action(
        "bf1_bes1",
        "con",
        ["all".into(), "imp".into()],
        &ui::shell::SessionOptions::default(),
        &game::profile::Profile::default(),
    );
    for slot in slots {
        s.humans.push(game::netplay::HumanSeat {
            slot: *slot,
            name: format!("P{slot}"),
            team: None,
            cheats: 0,
            profile: game::netplay::PlayerProfile::default(),
        });
    }
    s
}

fn click(
    button: winit::event::MouseButton,
    state: winit::event::ElementState,
) -> winit::event::WindowEvent {
    winit::event::WindowEvent::MouseInput {
        device_id: winit::event::DeviceId::dummy(),
        state,
        button,
    }
}

/// What one loading screen did ([`loading_wall`]).
struct Wall {
    ms: f64,
    duration: Duration,
    /// Prompted or skipped before the first fresh press (by the held or
    /// repeated Accept).
    early: bool,
    prompted: bool,
    /// The LAN host's skip went to every guest.
    skip_sent: bool,
}

/// The loading screen's wall time for one round on the one-still fixture,
/// `net` with a LAN host session that has started its round. Accept is
/// held from the screen before (the lobby's Launch), then pressed twice
/// fresh, then mashed: that skips in single player and on the LAN host.
fn loading_wall(net: bool) -> Wall {
    use winit::event::{ElementState, MouseButton};
    let f = one_still_fixture();
    let mut a = app(f.0.clone());
    if net {
        let mut n = lan_host(free_port());
        n.peer.host_load(&mut n.link, &lan_setup(&[0]));
        a.netplay = Some(n);
    }
    let accept = MouseButton::Left;
    // The lobby's Launch, still held as the loading screen opens.
    a.loading_window_event(&click(accept, ElementState::Pressed));
    assert!(a.begin_loading(row("bf1_bes1", ui::shell::Mode::Con)));
    let duration = a.loading.as_ref().unwrap().display.duration();
    let t = Instant::now();
    let (mut prompted, mut early, mut skip_sent) = (false, false, false);
    let mut frame = 0;
    while matches!(a.screen, super::super::Screen::Loading) && t.elapsed() < Duration::from_secs(10)
    {
        // The held Accept again (not fresh), its release, then fresh
        // presses and a mash of two buttons.
        let presses: &[(MouseButton, ElementState)] = match frame {
            0 => &[(accept, ElementState::Pressed)],
            1 => &[(accept, ElementState::Released)],
            2 | 3 => &[
                (accept, ElementState::Pressed),
                (accept, ElementState::Released),
            ],
            _ => &[
                (accept, ElementState::Pressed),
                (MouseButton::Right, ElementState::Pressed),
                (accept, ElementState::Released),
                (MouseButton::Right, ElementState::Released),
            ],
        };
        for (b, s) in presses {
            a.loading_window_event(&click(*b, *s));
        }
        if let Some(l) = a.loading.as_ref() {
            let e = l.started.elapsed();
            prompted |= l.playback.prompt(e) || l.playback.skipped();
            if frame < 2 {
                early |= prompted;
            }
        }
        a.redraw_loading().unwrap();
        skip_sent |= a
            .netplay
            .as_ref()
            .is_some_and(|n| n.peer.stats.loading_skips > 0);
        frame += 1;
        std::thread::sleep(Duration::from_millis(4));
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    if let Some(n) = a.netplay.take() {
        n.shutdown();
    }
    Wall {
        ms,
        duration,
        early,
        prompted,
        skip_sent,
    }
}

/// Issue #168: the LAN host skips the loading screen as single player
/// does (two fresh presses within the window) and its skip goes to every
/// guest. Accept held from the lobby, its repeat and its release never
/// count (the first VPN playtest's "skipped" screens).
#[test]
fn a_lan_hosts_two_fresh_presses_skip_for_everyone_and_a_held_accept_never_does() {
    for net in [false, true] {
        let w = loading_wall(net);
        assert!(!w.early, "net {net}: the held Accept neither prompts nor skips");
        assert!(w.prompted, "net {net}: the fresh presses prompt and skip");
        assert!(
            w.ms < w.duration.as_secs_f64() * 1000.0,
            "net {net}: skipped, {:.1} ms of {:?}",
            w.ms,
            w.duration
        );
        assert_eq!(w.skip_sent, net, "only the LAN host's skip goes out");
    }
}

/// Issue #168: a LAN guest's presses never prompt or skip its loading
/// screen, whatever Accept did; the host's skip ends it, early, over UDP
/// on this machine.
#[test]
fn a_lan_guests_presses_never_skip_but_the_hosts_skip_ends_its_loading() {
    use winit::event::{ElementState, MouseButton};
    let f = one_still_fixture();
    let port = free_port();
    let mut host = lan_host(port);
    let mut a = app(f.0.clone());
    let mut args = super::super::lan::LanArgs::parse(&[format!("127.0.0.1:{port}")], false).unwrap();
    args.assets = f.0.clone();
    let mut profile = game::netplay::PlayerProfile::decode_or_default(&[]);
    profile.name = "Guest".into();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    a.netplay = Some(super::super::lan::NetplaySession::join(args, profile, addr).unwrap());
    // Welcome, then the host's round: the guest takes its `LoadRound` and
    // opens the loading screen as `net_load` would.
    let t = Instant::now();
    let slot = loop {
        host.peer.poll(&mut host.link);
        let n = a.netplay.as_mut().unwrap();
        n.peer.poll(&mut n.link);
        if let Some(s) = host.peer.guests().keys().next() {
            break *s;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the guest joins");
        std::thread::sleep(Duration::from_millis(2));
    };
    host.peer.host_load(&mut host.link, &lan_setup(&[0, slot]));
    loop {
        host.peer.poll(&mut host.link);
        let n = a.netplay.as_mut().unwrap();
        n.peer.poll(&mut n.link);
        if n.peer.mode() == game::netplay::Mode::Loading {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the guest loads");
        std::thread::sleep(Duration::from_millis(2));
    }
    let accept = MouseButton::Left;
    a.loading_window_event(&click(accept, ElementState::Pressed));
    assert!(a.begin_loading(row("bf1_bes1", ui::shell::Mode::Con)));
    let duration = a.loading.as_ref().unwrap().display.duration();
    let t = Instant::now();
    let mut prompted = false;
    let mut skip_at = None;
    let mut frame = 0;
    while matches!(a.screen, super::super::Screen::Loading) && t.elapsed() < Duration::from_secs(10)
    {
        // Fresh presses and a mash on every frame.
        for (b, s) in [
            (accept, ElementState::Released),
            (accept, ElementState::Pressed),
            (MouseButton::Right, ElementState::Pressed),
            (accept, ElementState::Released),
            (MouseButton::Right, ElementState::Released),
        ] {
            a.loading_window_event(&click(b, s));
        }
        if let Some(l) = a.loading.as_ref() {
            let e = l.started.elapsed();
            prompted |= l.playback.prompt(e);
            assert!(
                skip_at.is_some() || !l.playback.skipped(),
                "frame {frame}: the guest's own presses skipped"
            );
        }
        // The host skips at 300 ms.
        if skip_at.is_none() && t.elapsed() >= Duration::from_millis(300) {
            assert!(host.peer.skip_loading(&mut host.link));
            skip_at = Some(t.elapsed());
        }
        host.peer.poll(&mut host.link);
        a.redraw_loading().unwrap();
        frame += 1;
        std::thread::sleep(Duration::from_millis(4));
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let heard = a.netplay.as_ref().unwrap().peer.stats.loading_skips;
    for n in [a.netplay.take().unwrap(), host] {
        n.shutdown();
    }
    assert!(!prompted, "a guest is never prompted");
    assert_eq!(heard, 1, "the guest heard the host's skip");
    let skip_at = skip_at.expect("the host skipped").as_secs_f64() * 1000.0;
    assert!(ms >= skip_at, "the guest's loading ended at the host's skip");
    assert!(
        ms < duration.as_secs_f64() * 1000.0,
        "the guest's loading lasted {ms:.1} ms of {duration:?}"
    );
}
