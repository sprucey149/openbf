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
    drop(w); // same owner drop used by window close; joins, never detaches
    assert!(ended.load(Ordering::SeqCst));
    let mut next = Worker::spawn(|| Ok(42)).unwrap();
    assert_eq!(await_worker(&mut next).unwrap(), 42);
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
            super::super::load_and_upload(a.renderer.as_mut().unwrap(), &root.join(key), "con")
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
        receipts.push(serde_json::json!({"map":key,"cpu_parse_ms":cpu_ms,"ready_wall_ms":ready_ms,"frames_while_pending":frames_pending,"upload_setup_ms":event_boundary_ms,"initial_hash":format!("{hash:016x}")}));
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
