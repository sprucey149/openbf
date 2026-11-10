//! Print how each connected controller resolves through `input::open_gilrs`
//! (card PADM) — the same log line the game prints at startup, without a
//! window: `cargo run --release -p input --example pads`.
//!
//! `--live [SECONDS]` (#142, #146) then reads the pads for that long (10 by
//! default) and prints each stick as it moves, with the running peak of each
//! axis: the line to paste when a pad walks slowly or will not sprint. The
//! game takes a left-stick peak down to `input::STICK_FULL_MIN` as a full
//! push, each axis on its own; under that the axis is out of its range.

use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let g = match input::open_gilrs() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gilrs unavailable: {e}");
            std::process::exit(2);
        }
    };
    let warning = input::log_pads(&g);
    let live = args.first().is_some_and(|a| a == "--live");
    if !live {
        if let Some(w) = warning {
            eprintln!("(the game shows this on screen)\n{w}");
            std::process::exit(1);
        }
        return;
    }
    let seconds: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    live_sticks(g, seconds);
}

/// Pump events for `seconds`, printing every stick move and its peak so far.
fn live_sticks(mut g: gilrs::Gilrs, seconds: f32) {
    use gilrs::{Axis, EventType};
    use std::collections::HashMap;
    println!(
        "reading sticks for {seconds} s: move each stick to its edge, all round \
         (a full push reads 1.000; the game takes a left peak of {} or more as full)",
        input::STICK_FULL_MIN
    );
    let mut peaks: HashMap<gilrs::GamepadId, [f32; 4]> = HashMap::new();
    let mut last_line = Instant::now() - Duration::from_secs(1);
    let end = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < end {
        while let Some(ev) = g.next_event() {
            if let EventType::AxisChanged(axis, _, _) = ev.event {
                if !matches!(
                    axis,
                    Axis::LeftStickX | Axis::LeftStickY | Axis::RightStickX | Axis::RightStickY
                ) {
                    continue;
                }
                let pad = g.gamepad(ev.id);
                let v = [
                    pad.value(Axis::LeftStickX),
                    pad.value(Axis::LeftStickY),
                    pad.value(Axis::RightStickX),
                    pad.value(Axis::RightStickY),
                ];
                let peak = peaks.entry(ev.id).or_default();
                for (p, v) in peak.iter_mut().zip(v) {
                    *p = p.max(v.abs());
                }
                if last_line.elapsed() >= Duration::from_millis(100) {
                    last_line = Instant::now();
                    println!(
                        "{}: left ({:+.3}, {:+.3}) right ({:+.3}, {:+.3})  peaks left ({:.3}, {:.3}) right ({:.3}, {:.3})",
                        pad.name(),
                        v[0],
                        v[1],
                        v[2],
                        v[3],
                        peak[0],
                        peak[1],
                        peak[2],
                        peak[3]
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    if peaks.is_empty() {
        println!("no stick moved");
    }
    for (id, p) in &peaks {
        let mut r = input::StickRange::default();
        r.feed(p[0], p[1]);
        let full = r.full();
        println!(
            "{}: peaks left ({:.3}, {:.3}) right ({:.3}, {:.3}); the game scales the walk by ({:.3}, {:.3})",
            g.gamepad(*id).name(),
            p[0],
            p[1],
            p[2],
            p[3],
            full[0],
            full[1]
        );
    }
}
