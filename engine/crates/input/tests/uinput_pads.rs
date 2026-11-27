//! Virtual pads through the kernel's uinput: xpad-style controllers the game
//! reads exactly as it reads real ones (gilrs opens their event nodes), and a
//! counter of the force-feedback traffic gilrs sends them.
//!
//! Ignored by default: they need write access to `/dev/uinput` and a seat
//! that grants the new event nodes to the user. Run them one at a time:
//! `cargo test -p input --test uinput_pads -- --ignored --test-threads=1 --nocapture`.
#![cfg(target_os = "linux")]

use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const EV_FF: u16 = 0x15;
const EV_UINPUT: u16 = 0x0101;
const UI_FF_UPLOAD: u16 = 1;
const UI_FF_ERASE: u16 = 2;
/// FF_RUMBLE, FF_PERIODIC, FF_SQUARE, FF_TRIANGLE, FF_SINE, FF_GAIN.
const FF_BITS: [u16; 6] = [0x50, 0x51, 0x58, 0x59, 0x5a, 0x60];
const BUS_USB: u16 = 0x03;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

/// The keys the Linux xpad driver gives an Xbox 360-class pad.
const XPAD_KEYS: [u16; 11] = [
    0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e,
];

const fn ioc(dir: u64, nr: u64, size: u64) -> u64 {
    (dir << 30) | (size << 16) | ((b'U' as u64) << 8) | nr
}
const UI_DEV_CREATE: u64 = ioc(0, 1, 0);
const UI_DEV_DESTROY: u64 = ioc(0, 2, 0);
const UI_DEV_SETUP: u64 = ioc(1, 3, std::mem::size_of::<UinputSetup>() as u64);
const UI_ABS_SETUP: u64 = ioc(1, 4, std::mem::size_of::<UinputAbsSetup>() as u64);
const UI_SET_EVBIT: u64 = ioc(1, 100, 4);
const UI_SET_KEYBIT: u64 = ioc(1, 101, 4);
const UI_SET_ABSBIT: u64 = ioc(1, 103, 4);
const UI_SET_FFBIT: u64 = ioc(1, 107, 4);
const UI_BEGIN_FF_UPLOAD: u64 = ioc(3, 200, std::mem::size_of::<UinputFfUpload>() as u64);
const UI_END_FF_UPLOAD: u64 = ioc(1, 201, std::mem::size_of::<UinputFfUpload>() as u64);
const UI_BEGIN_FF_ERASE: u64 = ioc(3, 202, std::mem::size_of::<UinputFfErase>() as u64);
const UI_END_FF_ERASE: u64 = ioc(1, 203, std::mem::size_of::<UinputFfErase>() as u64);

#[repr(C)]
struct UinputSetup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

/// `struct ff_effect`: 48 bytes on 64-bit Linux (the union holds a pointer).
#[repr(C)]
struct FfEffect {
    head: [u16; 7],
    u: [u64; 4],
}

#[repr(C)]
struct UinputFfUpload {
    request_id: u32,
    retval: i32,
    effect: FfEffect,
    old: FfEffect,
}

#[repr(C)]
struct UinputFfErase {
    request_id: u32,
    retval: i32,
    effect_id: u32,
}

#[repr(C)]
struct InputEvent {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

/// Force-feedback traffic one virtual pad received: effect uploads
/// (`EVIOCSFF`) and play requests (`EV_FF` writes). A memoryless kernel
/// driver (xpad, hid-playstation) turns each into an output report.
#[derive(Default)]
struct FfCount {
    uploads: AtomicU64,
    plays: AtomicU64,
    /// Every upload's rumble (strong, weak, length ms) and every play, in order.
    log: std::sync::Mutex<Vec<(Instant, FfOp)>>,
}

#[derive(Clone, Copy, Debug)]
enum FfOp {
    Upload { strong: u16, weak: u16, ms: u16 },
    Play,
}

impl FfCount {
    /// What the motors did between `from` and `to`, as the kernel plays it:
    /// a play starts the last uploaded rumble for its length; an upload while
    /// playing restarts it. Returns (milliseconds with a motor on, separate
    /// runs of a motor on, strong and weak motor output summed over the
    /// milliseconds as full-scale milliseconds).
    fn motor_timeline(&self, from: Instant, to: Instant) -> (u64, u32, u64, u64) {
        let log = self.log.lock().unwrap();
        let (mut on_ms, mut runs, mut strong_sum, mut weak_sum) = (0u64, 0u32, 0u64, 0u64);
        let mut current: Option<(u16, u16, u16)> = None;
        let mut playing_until: Option<Instant> = None;
        let mut on = false;
        let step = Duration::from_millis(1);
        let mut ops = log.iter().peekable();
        let mut t = from - Duration::from_millis(500);
        while t < to {
            while let Some(&&(at, op)) = ops.peek() {
                if at > t {
                    break;
                }
                ops.next();
                match op {
                    FfOp::Upload { strong, weak, ms } => {
                        current = Some((strong, weak, ms));
                        if playing_until.is_some_and(|u| u > at) {
                            playing_until = Some(at + Duration::from_millis(ms.into()));
                        }
                    }
                    FfOp::Play => {
                        if let Some((_, _, ms)) = current {
                            playing_until = Some(at + Duration::from_millis(ms.into()));
                        }
                    }
                }
            }
            let now_on = playing_until.is_some_and(|u| u > t)
                && current.is_some_and(|(s, w, _)| s != 0 || w != 0);
            if t >= from {
                if now_on {
                    let (s, w, _) = current.unwrap();
                    strong_sum += u64::from(s);
                    weak_sum += u64::from(w);
                    on_ms += 1;
                    if !on {
                        runs += 1;
                    }
                }
                on = now_on;
            }
            t += step;
        }
        (on_ms, runs, strong_sum / 65535, weak_sum / 65535)
    }
}

impl FfCount {
    fn total(&self) -> u64 {
        self.uploads.load(Ordering::Relaxed) + self.plays.load(Ordering::Relaxed)
    }
}

/// xpad's axes: sticks, analog triggers on ABS_Z / ABS_RZ, the d-pad on hat 0.
const XPAD_AXES: [(u16, i32, i32, i32, i32); 8] = [
    (ABS_X, -32768, 32767, 16, 128),
    (ABS_Y, -32768, 32767, 16, 128),
    (ABS_RX, -32768, 32767, 16, 128),
    (ABS_RY, -32768, 32767, 16, 128),
    (ABS_Z, 0, 255, 0, 0),
    (ABS_RZ, 0, 255, 0, 0),
    (ABS_HAT0X, -1, 1, 0, 0),
    (ABS_HAT0Y, -1, 1, 0, 0),
];

/// One virtual pad. Its force-feedback requests are answered on a thread
/// while it lives (an unanswered upload blocks the uploader).
struct VirtualPad {
    fd: c_int,
    ff: Arc<FfCount>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl VirtualPad {
    /// An xpad-style pad: the driver's eleven keys, sticks, analog triggers
    /// on ABS_Z / ABS_RZ, the d-pad on hat 0, and rumble.
    fn xpad(name: &str, vendor: u16, product: u16, version: u16) -> VirtualPad {
        VirtualPad::with_axes(name, vendor, product, version, &XPAD_AXES)
    }

    /// The xpad keys and rumble with these axes (code, min, max, fuzz, flat).
    fn with_axes(
        name: &str,
        vendor: u16,
        product: u16,
        version: u16,
        axes: &[(u16, i32, i32, i32, i32)],
    ) -> VirtualPad {
        unsafe {
            let fd = libc::open(
                c"/dev/uinput".as_ptr(),
                libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC,
            );
            assert!(
                fd >= 0,
                "open /dev/uinput: {}",
                std::io::Error::last_os_error()
            );
            let set = |req: u64, v: u16| {
                assert_eq!(
                    libc::ioctl(fd, req as _, c_int::from(v)),
                    0,
                    "ioctl {req:x} {v:x}"
                );
            };
            for ev in [EV_KEY, EV_ABS, EV_FF] {
                set(UI_SET_EVBIT, ev);
            }
            for key in XPAD_KEYS {
                set(UI_SET_KEYBIT, key);
            }
            // What a memoryless driver advertises: rumble, the periodic
            // waveforms it emulates on the motors, and gain.
            for bit in FF_BITS {
                set(UI_SET_FFBIT, bit);
            }
            for &(code, min, max, fuzz, flat) in axes {
                set(UI_SET_ABSBIT, code);
                let abs = UinputAbsSetup {
                    code,
                    value: 0,
                    minimum: min,
                    maximum: max,
                    fuzz,
                    flat,
                    resolution: 0,
                };
                assert_eq!(libc::ioctl(fd, UI_ABS_SETUP as _, &abs), 0);
            }
            let mut setup = UinputSetup {
                bustype: BUS_USB,
                vendor,
                product,
                version,
                name: [0; 80],
                ff_effects_max: 16,
            };
            setup.name[..name.len()].copy_from_slice(name.as_bytes());
            assert_eq!(libc::ioctl(fd, UI_DEV_SETUP as _, &setup), 0);
            assert_eq!(libc::ioctl(fd, UI_DEV_CREATE as _), 0);

            let ff = Arc::new(FfCount::default());
            let stop = Arc::new(AtomicBool::new(false));
            let thread = {
                let (ff, stop) = (ff.clone(), stop.clone());
                std::thread::spawn(move || serve_ff(fd, &ff, &stop))
            };
            VirtualPad {
                fd,
                ff,
                stop,
                thread: Some(thread),
            }
        }
    }

    fn emit(&self, type_: u16, code: u16, value: i32) {
        let ev = InputEvent {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        let n = unsafe {
            libc::write(
                self.fd,
                &ev as *const InputEvent as *const libc::c_void,
                std::mem::size_of::<InputEvent>(),
            )
        };
        assert_eq!(n as usize, std::mem::size_of::<InputEvent>());
    }

    /// Set one axis and send the report.
    fn axis(&self, code: u16, value: i32) {
        self.emit(EV_ABS, code, value);
        self.emit(EV_SYN, 0, 0);
    }
}

impl Drop for VirtualPad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        unsafe {
            libc::ioctl(self.fd, UI_DEV_DESTROY as _);
            libc::close(self.fd);
        }
    }
}

fn serve_ff(fd: c_int, ff: &FfCount, stop: &AtomicBool) {
    let size = std::mem::size_of::<InputEvent>();
    while !stop.load(Ordering::Relaxed) {
        let mut ev: InputEvent = unsafe { std::mem::zeroed() };
        let n = unsafe { libc::read(fd, &mut ev as *mut InputEvent as *mut libc::c_void, size) };
        if n as usize != size {
            std::thread::sleep(Duration::from_micros(500));
            continue;
        }
        match (ev.type_, ev.code) {
            (EV_UINPUT, UI_FF_UPLOAD) => unsafe {
                let mut up: UinputFfUpload = std::mem::zeroed();
                up.request_id = ev.value as u32;
                if libc::ioctl(fd, UI_BEGIN_FF_UPLOAD as _, &mut up) == 0 {
                    up.retval = 0;
                    libc::ioctl(fd, UI_END_FF_UPLOAD as _, &up);
                    ff.uploads.fetch_add(1, Ordering::Relaxed);
                    // head: type, id, direction, trigger (2), replay length, delay;
                    // the rumble union starts with strong then weak.
                    ff.log.lock().unwrap().push((
                        Instant::now(),
                        FfOp::Upload {
                            strong: up.effect.u[0] as u16,
                            weak: (up.effect.u[0] >> 16) as u16,
                            ms: up.effect.head[5],
                        },
                    ));
                }
            },
            (EV_UINPUT, UI_FF_ERASE) => unsafe {
                let mut er: UinputFfErase = std::mem::zeroed();
                er.request_id = ev.value as u32;
                if libc::ioctl(fd, UI_BEGIN_FF_ERASE as _, &mut er) == 0 {
                    er.retval = 0;
                    libc::ioctl(fd, UI_END_FF_ERASE as _, &er);
                }
            },
            (EV_FF, _) if ev.value != 0 => {
                ff.plays.fetch_add(1, Ordering::Relaxed);
                ff.log.lock().unwrap().push((Instant::now(), FfOp::Play));
            }
            _ => {}
        }
    }
}

/// Pump gilrs until `n` pads with this name are connected.
fn wait_for(g: &mut gilrs::Gilrs, name: &str, n: usize) -> Vec<gilrs::GamepadId> {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        while g.next_event().is_some() {}
        let ids: Vec<_> = g
            .gamepads()
            .filter(|(_, p)| p.os_name() == name)
            .map(|(id, _)| id)
            .collect();
        if ids.len() >= n {
            return ids;
        }
        assert!(
            Instant::now() < end,
            "only {} of {n} virtual pads '{name}' reached gilrs (needs the seat's ACL on new \
             event nodes)",
            ids.len()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn pump_for(g: &mut gilrs::Gilrs, d: Duration) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        while g.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Issue: a GameSir G7 Pro dongle re-enumerating as "Xbox 360 Controller for
/// Windows" (3537:0575, xpad's "Generic X-Box pad") had no SDL line, so its
/// triggers and d-pad mapped to nothing. Its own line now maps it, and any
/// other xpad pad without a line still gets triggers and d-pad.
#[test]
#[ignore = "needs /dev/uinput and a seat ACL on new event nodes"]
fn xpad_pads_without_a_db_line_map_triggers_and_dpad() {
    let known = VirtualPad::xpad("Generic X-Box pad", 0x3537, 0x0575, 0x0146);
    let unknown = VirtualPad::xpad("Generic X-Box pad", 0x1209, 0xf00d, 0x0001);
    std::thread::sleep(Duration::from_millis(300));
    let mut g = input::open_gilrs().unwrap();
    let ids = wait_for(&mut g, "Generic X-Box pad", 2);
    let map = input::Mapping::ps2_default();
    let by_vendor = |g: &gilrs::Gilrs, v: u16| {
        *ids.iter()
            .find(|&&id| g.gamepad(id).vendor_id() == Some(v))
            .unwrap()
    };
    let (k, u) = (by_vendor(&g, 0x3537), by_vendor(&g, 0x1209));
    assert_eq!(
        input::guid(&g.gamepad(k)),
        "03000000373500007505000046010000"
    );
    assert_eq!(
        g.gamepad(k).mapping_source(),
        gilrs::MappingSource::SdlMappings
    );
    assert_eq!(g.gamepad(u).mapping_source(), gilrs::MappingSource::Driver);
    for id in [k, u] {
        let pad = g.gamepad(id);
        assert_eq!(
            input::unresolved(&pad, &map),
            Vec::<String>::new(),
            "{}",
            input::guid(&pad)
        );
        // At rest, before any trigger report, nothing reads as held.
        for b in [gilrs::Button::RightTrigger2, gilrs::Button::LeftTrigger2] {
            assert!(!input::is_down(&pad, b), "{b:?} at rest");
        }
    }

    for (pad, id) in [(&known, k), (&unknown, u)] {
        let down = |g: &gilrs::Gilrs, b| input::is_down(&g.gamepad(id), b);
        use gilrs::Button as B;
        pad.axis(ABS_RZ, 255);
        pump_for(&mut g, Duration::from_millis(60));
        assert!(down(&g, B::RightTrigger2) && !down(&g, B::LeftTrigger2));
        pad.axis(ABS_RZ, 0);
        pad.axis(ABS_Z, 200);
        pump_for(&mut g, Duration::from_millis(60));
        assert!(!down(&g, B::RightTrigger2) && down(&g, B::LeftTrigger2));
        pad.axis(ABS_Z, 60);
        pump_for(&mut g, Duration::from_millis(60));
        assert!(!down(&g, B::LeftTrigger2), "a light touch is not a press");
        for (code, value, arm) in [
            (ABS_HAT0Y, -1, B::DPadUp),
            (ABS_HAT0Y, 1, B::DPadDown),
            (ABS_HAT0X, -1, B::DPadLeft),
            (ABS_HAT0X, 1, B::DPadRight),
        ] {
            pad.axis(code, value);
            pump_for(&mut g, Duration::from_millis(60));
            let held: Vec<_> = [B::DPadUp, B::DPadDown, B::DPadLeft, B::DPadRight]
                .into_iter()
                .filter(|&b| down(&g, b))
                .collect();
            assert_eq!(held, [arm], "hat {code:x}={value}");
            pad.axis(code, 0);
        }
        // The full sampled pad carries FIRE from the trigger.
        pad.axis(ABS_RZ, 255);
        pump_for(&mut g, Duration::from_millis(60));
        let (state, _) =
            input::pad_from_gamepad_with_arms(&g.gamepad(id), &map, 0, 1.0, [input::STICK_FULL; 2]);
        assert!(state.pressed(core::sim::buttons::FIRE));
        pad.axis(ABS_RZ, 0);
    }
}

/// A generic HID pad with no SDL line and its right stick on ABS_Z / ABS_RZ
/// (no ABS_RX / ABS_RY), centred: the Z axes are not triggers, so neither
/// trigger resolves and FIRE never reads held, centred or pushed.
#[test]
#[ignore = "needs /dev/uinput and a seat ACL on new event nodes"]
fn hid_pad_with_its_right_stick_on_z_never_fires() {
    let name = "Generic HID pad";
    let pad = VirtualPad::with_axes(
        name,
        0x1209,
        0xf00e,
        0x0001,
        &[
            (ABS_X, -32768, 32767, 16, 128),
            (ABS_Y, -32768, 32767, 16, 128),
            (ABS_Z, -32768, 32767, 16, 128),
            (ABS_RZ, -32768, 32767, 16, 128),
            (ABS_HAT0X, -1, 1, 0, 0),
            (ABS_HAT0Y, -1, 1, 0, 0),
        ],
    );
    std::thread::sleep(Duration::from_millis(300));
    let mut g = input::open_gilrs().unwrap();
    let id = wait_for(&mut g, name, 1)[0];
    assert_eq!(g.gamepad(id).mapping_source(), gilrs::MappingSource::Driver);
    let map = input::Mapping::ps2_default();
    let missing = input::unresolved(&g.gamepad(id), &map);
    assert!(
        missing.iter().any(|m| m == "RightTrigger2"),
        "the Z stick is not a trigger: {missing:?}"
    );
    use gilrs::Button as B;
    for (z, rz) in [(0, 0), (32767, 32767), (-32768, -32768)] {
        pad.axis(ABS_Z, z);
        pad.axis(ABS_RZ, rz);
        pump_for(&mut g, Duration::from_millis(60));
        let p = g.gamepad(id);
        for b in [B::RightTrigger2, B::LeftTrigger2] {
            assert!(!input::is_down(&p, b), "{b:?} with Z={z} Rz={rz}");
        }
        let (state, _) =
            input::pad_from_gamepad_with_arms(&p, &map, 0, 1.0, [input::STICK_FULL; 2]);
        assert!(!state.pressed(core::sim::buttons::FIRE), "FIRE with Z={z}");
    }
}

/// Force-feedback reports per second for four pads, idle and while every
/// player fires a repeater (the firing cue: 0.42 / 55 ms and 0.20 / 45 ms,
/// eight pulses a second), and the motor timeline of one of each pulse the
/// game sends. Prints the counts and timelines (compare them with and without
/// the vendored gilrs change); asserts that an idle pad gets no traffic, that
/// a firing pad still gets its pulses and that each pulse plays unbroken.
#[test]
#[ignore = "needs /dev/uinput and a seat ACL on new event nodes"]
fn rumble_reports_per_second_four_pads() {
    let pads: Vec<_> = (0..4)
        .map(|i| VirtualPad::xpad("openbf ff probe", 0x1209, 0xf0f0 + i, 0x0001))
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let mut g = input::open_gilrs().unwrap();
    let ids = wait_for(&mut g, "openbf ff probe", 4);
    assert!(ids.iter().all(|&id| g.gamepad(id).is_ff_supported()));
    pump_for(&mut g, Duration::from_millis(300));

    let window = Duration::from_secs(3);
    let snapshot = |pads: &[VirtualPad]| pads.iter().map(|p| p.ff.total()).collect::<Vec<_>>();
    let rate = |before: &[u64], after: &[u64]| {
        before
            .iter()
            .zip(after)
            .map(|(b, a)| (a - b) as f64 / window.as_secs_f64())
            .collect::<Vec<_>>()
    };

    let before = snapshot(&pads);
    pump_for(&mut g, window);
    let idle = rate(&before, &snapshot(&pads));

    let pulse = [
        input::RumbleMotor {
            magnitude: 0.42,
            duration_ms: 55,
            fade_ms: 0,
        },
        input::RumbleMotor {
            magnitude: 0.20,
            duration_ms: 45,
            fade_ms: 0,
        },
    ];
    let mut effects: Vec<Option<gilrs::ff::Effect>> = vec![None; 4];
    let before = snapshot(&pads);
    let start = Instant::now();
    let end = start + window;
    let mut next = Instant::now();
    while Instant::now() < end {
        if Instant::now() >= next {
            for (e, &id) in effects.iter_mut().zip(&ids) {
                *e = input::rumble(&mut g, id, pulse, 1.0);
            }
            next += Duration::from_millis(125);
        }
        while g.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(2));
    }
    let firing = rate(&before, &snapshot(&pads));
    // Leave the last pulse 300 ms to finish before reading its timeline.
    let timeline: Vec<_> = {
        pump_for(&mut g, Duration::from_millis(300));
        pads.iter()
            .map(|p| p.ff.motor_timeline(start, end + Duration::from_millis(200)))
            .collect()
    };
    drop(effects);
    pump_for(&mut g, Duration::from_millis(300));
    let before = snapshot(&pads);
    pump_for(&mut g, window);
    let after = rate(&before, &snapshot(&pads));

    let fmt = |v: &[f64]| {
        v.iter()
            .map(|r| format!("{r:.1}"))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    println!("ff reports/s per pad (uploads + plays), 4 pads:");
    println!("  idle:               {}", fmt(&idle));
    println!("  all firing (8 Hz):  {}", fmt(&firing));
    println!("  idle after firing:  {}", fmt(&after));
    println!("  motor (on ms, runs, strong ms, weak ms) while firing: {timeline:?}");

    // One of each pulse the game sends, alone on pad 0, read 1.2 s each.
    let m = |magnitude, duration_ms, fade_ms| input::RumbleMotor {
        magnitude,
        duration_ms,
        fade_ms,
    };
    for (what, pulse) in [
        ("fire cue", [m(0.42, 55, 0), m(0.20, 45, 0)]),
        ("hit", [m(1.0, 250, 0), m(0.35, 208, 0)]),
        ("flyer impact", [m(1.0, 600, 0), m(0.28, 300, 300)]),
        ("preview", [m(1.0, 280, 0), m(1.0, 280, 0)]),
    ] {
        let start = Instant::now();
        let effect = input::rumble(&mut g, ids[0], pulse, 1.0);
        pump_for(&mut g, Duration::from_millis(1200));
        drop(effect);
        let t = pads[0]
            .ff
            .motor_timeline(start, start + Duration::from_millis(1200));
        println!("  {what}: (on ms, runs, strong ms, weak ms) {t:?}");
        assert_eq!(t.1, 1, "{what} plays as one unbroken run");
    }
    assert!(
        firing.iter().all(|&r| r >= 8.0),
        "every pulse reaches its pad"
    );
    assert!(
        idle.iter().chain(&after).all(|&r| r == 0.0),
        "an idle pad gets no force-feedback traffic"
    );
}
