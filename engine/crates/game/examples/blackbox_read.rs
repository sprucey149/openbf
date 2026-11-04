//! Read a playtest black box snapshot (`game::blackbox`, card BLKB) and print
//! the digest to read first — DEV SCAFFOLDING, goes with the module.
//!
//! ```text
//! cargo run -p game --example blackbox_read -- [file] [view]
//! ```
//!
//! `file` defaults to `engine/artifacts/playtest/last.json`. Run it under the
//! memory cap, as every probe (`systemd-run --user --scope -p MemoryMax=8G`).
//!
//! With no view it prints: the header (the commit the BINARY was built from,
//! the checkout's HEAD beside it, the build time, and a warning when they do
//! not agree), the human's last 10 s, the focus unit — what the human was
//! looking at — with its brain and trail, every bot with a no-progress or
//! blocked-steering run over 3 s or a trail that went nowhere (who, where,
//! which body, its goal), every unit within 15 m of the human, the posts, the
//! last 30 s of events, and the sounds (card SNDB): a line for every tick
//! whose starts sum over full scale ("N starts, sum G: names") and the last
//! 10 s of starts - name, gain, pitch, metres, voices, take file. Right
//! after the header, the frame record (card HITCH, schema 6): the summary
//! line (frames, p50 / p99 / worst ms, frames over 25 ms past the first 5 s
//! and per minute), the five longest frames and the newest ten over 25 ms,
//! each with its phases, ticks run, AI and hook ms and the GPU objects made.
//!
//! Views:
//! - `--unit <id>`: that unit in every frame, its trail, its events and
//!   (card WHY, schema 9) its last reasons: why it entered each behaviour,
//!   why each walk failed (a troubled bot's line carries its last four);
//! - `--near x,y,z,r`: the units within `r` of a point in the last frame;
//! - `--events`: every event in the file;
//! - `--sounds`: every sound start in the file (60 s);
//! - `--frames`: every slow frame the record kept (up to 256), each GPU
//!   object made in it named;
//! - `--replay-spot [id]`: one line for a headless follow-up on a unit's spot
//!   (the focus unit by default): the map, seed, tick and place, and the
//!   `round_to_outcome` command that looks at it.

use game::blackbox::{Ev, EventRow, Frame, Snapshot};
use game::unit_probe::{Probe, Steer, Stop};
use std::fmt::Write as _;

const HZ: f32 = core::TICK_HZ as f32;

/// A human row's reticle / magnet code: `#N` a unit, `veh N` a vehicle
/// (-2 - N), `-` none.
fn pick_label(code: i32) -> String {
    match code {
        c if c >= 0 => format!("#{c}"),
        -1 => "-".into(),
        c => format!("veh {}", -2 - c),
    }
}

fn secs(snap: &Snapshot, tick: u64) -> f32 {
    (tick as f32 - snap.header.tick as f32) / HZ
}

fn class_name<'a>(snap: &'a Snapshot, p: &Probe) -> &'a str {
    snap.names.classes[usize::from(p.team).min(1)]
        .get(usize::from(p.class))
        .map_or("?", String::as_str)
}

fn post_name(snap: &Snapshot, cp: i32) -> String {
    usize::try_from(cp)
        .ok()
        .and_then(|i| snap.names.posts.get(i))
        .map_or_else(|| format!("cp{cp}"), |n| format!("cp{cp} {n}"))
}

fn body_name(snap: &Snapshot, body: i32) -> String {
    match u32::try_from(body).ok() {
        None => "-".into(),
        Some(b) => snap
            .names
            .bodies
            .get(&b)
            .map_or_else(|| format!("body {b}"), |n| format!("body {b} \"{n}\"")),
    }
}

/// What the unit is doing, from its brain fields.
fn doing(p: &Probe) -> String {
    if p.human {
        return "the human".into();
    }
    if p.veh >= 0 {
        return format!("rides vehicle {} seat {}", p.veh, p.seat);
    }
    let mut s = match (p.lost, p.board, p.ordered, p.hold) {
        (l, ..) if l > 0 => format!(
            "LOST ({}, {} jump tries)",
            if l == 2 { "confirmed" } else { "suspected" },
            p.lost_tries
        ),
        (_, b, ..) if b >= 0 => format!("boarding vehicle {b}"),
        (_, _, true, _) => "under a squad order".into(),
        (.., 2) => "holding a position".into(),
        _ => p.goal_label(),
    };
    // Schema 2: the hold itself, and where the steering walked this tick.
    let at = |v: [f32; 3]| format!("({:.1},{:.1},{:.1})", v[0], v[1], v[2]);
    let d = |v: [f32; 3]| glam::Vec3::from(v).distance(glam::Vec3::from(p.pos));
    if p.hold == 2 && p.hold_left > 0.0 {
        let _ = write!(
            s,
            " [hold goal {} {:.1} m off, spot {} {:.1} m off, {:.1} s left, strafe side {:+}, goal kept {:.1} s, blocked {} ticks]",
            at(p.hold_goal),
            d(p.hold_goal),
            at(p.hold_spot),
            d(p.hold_spot),
            p.hold_left,
            p.hold_side,
            p.hold_commit,
            p.hold_blocked
        );
    }
    // Card TPOS: the fight's mode and the Tactician's position.
    if p.fmode != 0 || p.tpos.is_some() {
        const MODES: [&str; 7] = ["none", "stand", "walk", "pause", "move", "TAKE", "COVER"];
        let _ = write!(
            s,
            " [fight {}",
            MODES.get(usize::from(p.fmode)).unwrap_or(&"?")
        );
        if let Some(i) = p.tpos {
            let _ = write!(
                s,
                " pos #{i} {} {:.1} m off, {}{}{}, hits {}, phase {:.1} s",
                at(p.tpos_at),
                d(p.tpos_at),
                if p.tpos_bits & 1 != 0 {
                    "covered"
                } else {
                    "open"
                },
                if p.tpos_bits & 2 != 0 {
                    " firing"
                } else {
                    " hidden"
                },
                match p.tpos_bits & 12 {
                    4 => " lean -1",
                    8 => " lean +1",
                    _ => "",
                },
                p.tpos_hits,
                p.tpos_phase
            );
        }
        s.push(']');
    }
    if p.board >= 0 && p.board_at != [0.0; 3] {
        let _ = write!(s, " [vehicle at {} {:.1} m]", at(p.board_at), d(p.board_at));
    }
    if p.walk > 0 {
        let _ = write!(
            s,
            " -> walks at {} {} {:.1} m",
            match p.walk {
                1 => "its return point",
                2 => "a point out of a hazard",
                3 => "its hold goal",
                4 => "its vehicle",
                core::ai::WalkAsk::MEND => "its health droid",
                _ => "its leg's point",
            },
            at(p.walk_at),
            d(p.walk_at)
        );
    }
    if p.cp >= 0 && p.cp != p.goal_cp && p.walk != core::ai::WalkAsk::MEND {
        let _ = write!(s, " (walks at cp{})", p.cp);
    }
    s
}

/// One line for a unit.
fn unit_line(snap: &Snapshot, p: &Probe) -> String {
    let mut s = format!(
        "#{:<3} t{} {:<22} ({:.1},{:.1},{:.1}) hp {:<4.0} {:.1} m/s {} | {}",
        p.id,
        p.team,
        class_name(snap, p),
        p.pos[0],
        p.pos[1],
        p.pos[2],
        p.hp,
        p.speed,
        if p.grounded { "ground" } else { "AIR" },
        doing(p),
    );
    if p.leg {
        let _ = write!(
            s,
            " | leg {}->{} dest {} arc {} {} wp {:.1} m",
            p.from, p.next, p.dest, p.arc, p.src, p.wp_d
        );
    }
    if !p.human {
        let _ = write!(s, " | steer {}", p.steer_label());
    }
    if p.stop != Stop::Open {
        let _ = write!(
            s,
            " STOP {:?} at {:.2} m {}",
            p.stop,
            p.stop_d,
            body_name(snap, p.body)
        );
    }
    // Schema 4: the behaviour that ran, and the going-round commitment.
    if snap.schema >= 4 && !p.human {
        let _ = write!(s, " | does {}", p.doing.tag());
        if p.recover > 0 {
            let _ = write!(s, " (recover {})", p.recover);
        }
        if p.round {
            let _ = write!(
                s,
                " | ROUND side {:+}{}, nearest {:.1} m, allowance {:.1} of {:.0} s{}",
                p.slide,
                if p.walk == 0 {
                    " (kept, not walking)"
                } else {
                    ""
                },
                p.round_near,
                f32::from(p.round_t) / HZ,
                f32::from(core::ai::ROUND_MAX) / HZ,
                if p.round_rim { ", at a RIM (none)" } else { "" }
            );
        } else if p.round_t > 0 {
            let _ = write!(
                s,
                " | went round for {:.1} of {:.0} s",
                f32::from(p.round_t) / HZ,
                f32::from(core::ai::ROUND_MAX) / HZ
            );
        }
    }
    if p.stuck > 0 || p.stall > 0 {
        let _ = write!(
            s,
            " | stuck {:.1} s stall {:.0} s",
            f32::from(p.stuck) / HZ,
            p.stall as f32 / HZ
        );
    }
    if p.target >= 0 {
        let _ = write!(s, " | enemy #{} at {:.0} m", p.target, p.target_d);
    } else if let Some(id) = p.target_droid {
        let _ = write!(s, " | enemy droid {id} at {:.0} m", p.target_d);
    }
    let _ = write!(s, " | locus {}{} level {:.1}", p.locus, p.hub, p.level);
    s
}

fn weapon_name(snap: &Snapshot, i: i32) -> &str {
    usize::try_from(i)
        .ok()
        .and_then(|i| snap.names.weapons.get(i))
        .map_or("?", String::as_str)
}

fn event_line(snap: &Snapshot, e: &EventRow) -> String {
    let at = format!("({:.1},{:.1},{:.1})", e.pos[0], e.pos[1], e.pos[2]);
    let what = match e.kind {
        Ev::Kill => format!(
            "#{} killed #{} with {} at {at}",
            e.a,
            e.b,
            usize::try_from(e.c)
                .ok()
                .and_then(|i| snap.names.weapons.get(i))
                .map_or("?", String::as_str)
        ),
        Ev::Spawn => format!("#{} spawned at {at}", e.a),
        Ev::Despawn => format!("#{} shed", e.a),
        Ev::Captured => format!("{} captured by team {}", post_name(snap, e.a), e.b),
        Ev::Neutralized => format!("{} neutralised by team {}", post_name(snap, e.a), e.b),
        Ev::Saved => format!("{} saved by team {}", post_name(snap, e.a), e.b),
        Ev::Defeated => format!("team {} defeated", e.a),
        Ev::TimeUp => format!("time up, winner {}", e.a),
        Ev::VehicleLost => format!(
            "vehicle {} ({}) destroyed by #{} at {at}",
            e.a,
            usize::try_from(e.c)
                .ok()
                .and_then(|i| snap.names.vehicles.get(i))
                .map_or("?", String::as_str),
            e.b
        ),
        Ev::Order => format!("player {} order {} to {} bots", e.a, e.b, e.c),
        Ev::Voice => format!("commander: {}", e.text),
        Ev::Stuck => format!(
            "#{} no progress for 3 s at {at}, in its way: {}",
            e.a,
            body_name(snap, e.b)
        ),
        Ev::Shot => format!("#{} fired {}", e.a, weapon_name(snap, e.c)),
        Ev::Hit => {
            let v = |i: usize| e.v.get(i).copied().unwrap_or(0.0);
            format!(
                "#{} hit #{} with {}: {} damage ({}), {} left, at {at}",
                e.a,
                e.b,
                weapon_name(snap, e.c),
                v(0),
                match (v(2), snap.schema) {
                    (a, _) if a > 0.0 => format!("direct, {a:.2} s flight"),
                    (a, s) if a == 0.0 && s >= 11 => "direct, struck in its first update".into(),
                    (_, s) if s >= 11 => "blast".into(),
                    // Schema < 11 wrote 0 for both (`core::sim::BLAST_AGE`).
                    _ => "blast, or a bolt that struck in its first update".into(),
                },
                v(1)
            )
        }
        Ev::VehicleHit => {
            let v = |i: usize| e.v.get(i).copied().unwrap_or(0.0);
            let who = if e.a < 0 {
                "a collision".to_string()
            } else {
                format!("#{} with {}", e.a, weapon_name(snap, e.c))
            };
            format!(
                "{who} hit vehicle {}: {} damage, {} left at the tick's end, at {at}",
                e.b,
                v(0),
                v(1)
            )
        }
        k => format!("#{} died: {k:?} at {at}", e.a),
    };
    format!("{:>7.1} s  {what}", secs(snap, e.tick))
}

/// A blocked-steering run: its length (s), the net metres moved over it, the
/// tick it ended at and the last row in it that named what was in the way.
struct Run {
    secs: f32,
    net: f32,
    end: u64,
    at: Probe,
}

/// A unit's longest run of frames in which the steering was going ROUND
/// something (slide, fan, back, ledge) rather than walking straight.
fn blocked_run(frames: &[Frame], id: u32, gen: u32) -> Option<Run> {
    let mut run: Vec<(u64, &Probe)> = Vec::new();
    let mut best: Option<Run> = None;
    let mut close = |run: &mut Vec<(u64, &Probe)>| {
        if let (Some((_, a)), Some((end, b))) = (run.first(), run.last()) {
            let secs = run.len() as f32 * 0.5;
            if best.as_ref().is_none_or(|r| secs > r.secs) {
                let named = run.iter().rev().find(|(_, p)| p.stop != Stop::Open);
                best = Some(Run {
                    secs,
                    net: glam::Vec3::from(a.pos).distance(glam::Vec3::from(b.pos)),
                    end: *end,
                    at: *named.map_or(*b, |(_, p)| *p),
                });
            }
        }
        run.clear();
    };
    for f in frames {
        match f.units.iter().find(|p| p.id == id && p.gen == gen) {
            Some(p)
                if !p.human
                    && p.veh < 0
                    && !matches!(p.steer, Steer::Fwd | Steer::Idle | Steer::Round) =>
            {
                run.push((f.tick, p))
            }
            _ => close(&mut run),
        }
    }
    close(&mut run);
    best
}

/// The frame record (card HITCH): the summary, the worst five, and the
/// newest ten slow frames (`all`: every one kept, each GPU object named).
fn frame_record(snap: &Snapshot, all: bool) -> String {
    let mut s = String::new();
    let Some(h) = &snap.hitches else {
        let why = match snap.schema < 6 {
            true => "schema < 6: written before card HITCH",
            false => "a headless round, or the record off",
        };
        let _ = writeln!(s, "FRAMES  not recorded ({why})");
        return s;
    };
    let _ = writeln!(s, "FRAMES  {}", h.summary());
    for w in &h.worst {
        let _ = writeln!(s, "  worst  {}", w.line());
    }
    if !h.made.is_empty() {
        let made: Vec<String> = h
            .made
            .iter()
            .map(|m| {
                format!(
                    "{} {:?} x{} ({:.1} MB, {:.1} ms)",
                    m.kind,
                    m.label,
                    m.count,
                    m.bytes as f64 / (1024.0 * 1024.0),
                    m.ms
                )
            })
            .collect();
        let _ = writeln!(s, "  made past {:.0} s: {}", h.settle_s, made.join(", "));
    }
    let from = if all {
        0
    } else {
        h.slow.len().saturating_sub(10)
    };
    if !h.slow.is_empty() {
        let _ = writeln!(
            s,
            "  slow (over {:.0} ms): {} kept, {}",
            h.limit_ms,
            h.slow.len(),
            if all { "all" } else { "the newest 10" }
        );
    }
    for r in &h.slow[from..] {
        let _ = writeln!(s, "  slow   {}", r.line());
        if all {
            for m in &r.made {
                let _ = writeln!(
                    s,
                    "           made {} {:?} {} B {:.2} ms",
                    m.kind, m.label, m.bytes, m.ms
                );
            }
        }
    }
    s
}

fn header(snap: &Snapshot) -> String {
    let h = &snap.header;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "SNAPSHOT  reason: {}  | schema {}",
        h.reason, snap.schema
    );
    let built = if h.meta.build_time > 0 {
        format!("{} UTC", utc(h.meta.build_time))
    } else {
        "unknown".into()
    };
    let dirty = |d: bool| if d { " +dirty" } else { "" };
    if h.meta.build_commit.is_empty() {
        // Schema 1: only the checkout's HEAD was recorded.
        let _ = writeln!(
            s,
            "  build: commit NOT RECORDED (schema 1), binary built {built}  | checkout HEAD {}{} when the battle started",
            h.meta.commit,
            dirty(h.meta.dirty)
        );
        match commit_time(&h.meta.commit) {
            Some(t) if h.meta.build_time > 0 && h.meta.build_time < t => {
                let _ = writeln!(
                    s,
                    "  WARNING: the binary was built BEFORE commit {} was made ({} UTC): it does not hold that commit's code",
                    h.meta.commit,
                    utc(t)
                );
            }
            Some(_) => {}
            None => {
                let _ = writeln!(
                    s,
                    "  note: commit {} not found here; the binary may be older than it",
                    h.meta.commit
                );
            }
        }
    } else {
        let _ = writeln!(
            s,
            "  build: commit {}{}, binary built {built}  (the snapshot belongs to THIS commit: if main has moved, say so)",
            h.meta.build_commit,
            dirty(h.meta.build_dirty)
        );
        let _ = writeln!(
            s,
            "  checkout HEAD {}{} when the battle started",
            h.meta.commit,
            dirty(h.meta.dirty)
        );
        if h.meta.build_commit != h.meta.commit {
            let _ = writeln!(
                s,
                "  WARNING: the binary ({}) is not the checkout's HEAD ({}): read the code at {}",
                h.meta.build_commit, h.meta.commit, h.meta.build_commit
            );
        }
        if h.meta.build_dirty {
            let _ = writeln!(
                s,
                "  WARNING: built from an edited tree: {} plus uncommitted source changes",
                h.meta.build_commit
            );
        }
    }
    if snap.schema < 4 {
        let _ = writeln!(
            s,
            "  note: schema {} — no behaviour and no going-round commitment in the unit rows (card OBST2, schema 4)",
            snap.schema
        );
    }
    if snap.schema < 2 {
        let _ = writeln!(
            s,
            "  note: schema 1 — each STOP was asked toward the leg's point, not where the steering walked (wrong for holders, boarders, lost bots; a standing bot may carry one); no hold fields"
        );
    }
    let _ = writeln!(
        s,
        "  map {} ({} {})  mode {}  seed {}  difficulty {}  factions {} v {}",
        h.meta.map,
        h.meta.source_game,
        snap.names.posts.len(),
        h.meta.mode,
        h.meta.seed,
        h.meta.difficulty,
        snap.names.factions[0],
        snap.names.factions[1]
    );
    if !h.meta.mission.is_empty() {
        let _ = writeln!(
            s,
            "  mission {}  the player on team {}",
            h.meta.mission,
            h.meta.player_team + 1
        );
    }
    let _ = writeln!(
        s,
        "  tick {} ({:.1} s)  window {}x{}  {:.0} fps (frame mean {:.1} / p99 {:.1} / max {:.1} ms)  human #{}  ring {:.2} MB",
        h.tick, h.time, h.window[0], h.window[1], h.fps, h.frame_ms[0], h.frame_ms[1], h.frame_ms[2], h.human, h.ring_mb
    );
    let _ = writeln!(
        s,
        "  holds {} unit frames ({:.0} s), {} human rows, {} world frames, {} events, {} trails",
        snap.frames.len(),
        snap.frames.len() as f32 * 0.5,
        snap.human.len(),
        snap.world.len(),
        snap.events.len(),
        snap.trails.len()
    );
    let env: Vec<String> = h.meta.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let _ = writeln!(s, "  env {}", env.join(" "));
    // Card DPRX (schema 10): the whole battle's tally; human_profile.py
    // prints the full profile.
    if let Some(m) = &snap.session {
        let mins = (m.to.saturating_sub(m.from)) as f32 / core::TICK_HZ as f32 / 60.0;
        let _ = writeln!(
            s,
            "  session ({} #{}, {:.1} min): kills {} deaths {} shots {} hits {} lives {} (scripts/human_profile.py for the profile)",
            m.subject,
            m.unit,
            mins,
            m.kills,
            m.deaths,
            m.shots,
            m.hits,
            m.lives.len()
        );
    }
    s
}

/// Seconds since the epoch as `YYYY-MM-DD hh:mm:ss` (UTC).
fn utc(t: u64) -> String {
    let (days, rem) = (t / 86_400, t % 86_400);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// When `commit` was made (seconds since the epoch), asked of git here.
fn commit_time(commit: &str) -> Option<u64> {
    let o = std::process::Command::new("git")
        .args(["show", "-s", "--format=%ct", commit])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    String::from_utf8_lossy(&o.stdout).trim().parse().ok()
}

fn buttons(b: u32) -> String {
    const NAMES: [&str; 24] = [
        "fire",
        "jump",
        "roll",
        "sprint",
        "zoom",
        "reload",
        "action",
        "switch",
        "secondary",
        "sq-follow",
        "sq-hold",
        "sq-at-ease",
        "sq-move-out",
        "class+",
        "class-",
        "switch2",
        "post-up",
        "post-down",
        "post-left",
        "post-right",
        "accept",
        "back",
        "respawn",
        "mouse-aim",
    ];
    let v: Vec<&str> = NAMES
        .iter()
        .enumerate()
        .filter(|(i, _)| b & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect();
    if v.is_empty() {
        "-".into()
    } else {
        v.join("+")
    }
}

/// The human, one line per half second over the last `span` seconds.
fn human(snap: &Snapshot, span: f32) -> String {
    let mut s = String::new();
    if snap.human.is_empty() {
        return "HUMAN  none (a headless round)\n".into();
    }
    let _ = writeln!(
        s,
        "HUMAN  last {span:.0} s (one line per 0.5 s; buttons = any pressed in it)"
    );
    let from = snap.header.tick.saturating_sub((span * HZ) as u64);
    let rows: Vec<_> = snap.human.iter().filter(|r| r.tick > from).collect();
    let mut i = 0;
    while i < rows.len() {
        // A half second of rows: the 0.5 s rows stand alone, the per-tick
        // ones are grouped.
        let bucket = rows[i].tick.div_ceil(15);
        let mut j = i;
        let (mut pressed, mut stick, mut mouse) = (0u32, glam::Vec2::ZERO, (0i64, 0i64));
        while j < rows.len() && rows[j].tick.div_ceil(15) == bucket {
            pressed |= rows[j].buttons;
            mouse.0 += i64::from(rows[j].mx);
            mouse.1 += i64::from(rows[j].my);
            stick += glam::Vec2::new(f32::from(rows[j].lx), f32::from(rows[j].ly)) / 32767.0;
            j += 1;
        }
        stick /= (j - i) as f32;
        let r = rows[j - 1];
        let _ = writeln!(
            s,
            "  {:>6.1} s  {} ({:.1},{:.1},{:.1}) yaw {:.2} pitch {:.2}  {:.1} m/s  stick ({:+.2},{:+.2}){} {}  hp {:.0}  {} {}/{}  reticle {}  magnet {}{}",
            secs(snap, r.tick),
            if r.alive { "alive" } else { "DEAD " },
            r.pos[0], r.pos[1], r.pos[2], r.yaw, r.pitch, r.speed, stick.x, stick.y,
            match mouse {
                (0, 0) => String::new(),
                (x, y) => format!(
                    " mouse ({:+.1},{:+.1}) deg",
                    (x as f32 * core::sim::LOOK_UNIT).to_degrees(),
                    (y as f32 * core::sim::LOOK_UNIT).to_degrees()
                ),
            },
            buttons(pressed), r.hp,
            snap.names.weapons.get(r.weapon as usize).map_or("?", String::as_str),
            r.clip, r.spare,
            pick_label(r.reticle),
            pick_label(r.magnet),
            if r.veh >= 0 { format!("  in vehicle {} seat {}", r.veh, r.seat) } else { String::new() },
        );
        i = j;
    }
    s
}

/// A trail's measure: metres walked, net metres, direction reversals (a
/// step of 5 cm or more turned over 90 degrees from the one before).
struct Walked {
    path: f32,
    net: f32,
    reversals: u32,
}

impl Walked {
    fn of(pts: &[[f32; 3]]) -> Walked {
        let pts: Vec<glam::Vec3> = pts.iter().map(|p| glam::Vec3::from(*p)).collect();
        let mut reversals = 0;
        let mut prev = glam::Vec3::ZERO;
        for w in pts.windows(2) {
            let d = w[1] - w[0];
            if d.length() < 0.05 {
                continue;
            }
            if prev != glam::Vec3::ZERO && d.dot(prev) < 0.0 {
                reversals += 1;
            }
            prev = d;
        }
        Walked {
            path: pts.windows(2).map(|w| w[0].distance(w[1])).sum(),
            net: pts
                .first()
                .zip(pts.last())
                .map_or(0.0, |(a, b)| a.distance(*b)),
            reversals,
        }
    }

    /// Over 3 m walked for under a quarter of it in net distance.
    fn nowhere(&self) -> bool {
        self.path > 3.0 && self.net < 0.25 * self.path
    }
}

/// The tick a trail's last point was taken at. Schema 2 carries it; in a
/// schema 1 file a trail of a unit that is not in the last frame ended at an
/// unrecorded earlier tick (`None`): the unit was dead.
fn trail_end(snap: &Snapshot, t: &game::blackbox::TrailOut) -> Option<u64> {
    if snap.schema >= 2 {
        return Some(t.tick);
    }
    snap.frames
        .last()
        .is_some_and(|f| f.units.iter().any(|p| p.id == t.id))
        .then_some(snap.header.trail_tick)
}

fn trail_ends(snap: &Snapshot, t: &game::blackbox::TrailOut) -> String {
    match trail_end(snap, t) {
        Some(e) if e == snap.header.trail_tick => format!("ends {:.1} s", secs(snap, e)),
        Some(e) => format!("ENDS {:.1} s: the unit died", secs(snap, e)),
        None => "ENDS EARLIER, tick not recorded: the unit died".into(),
    }
}

fn trail(snap: &Snapshot, id: u32) -> String {
    let Some(t) = snap.trails.iter().find(|t| t.id == id) else {
        return "  trail: none\n".into();
    };
    let pts: Vec<glam::Vec3> = t.pts.iter().map(|p| glam::Vec3::from(*p)).collect();
    let w = Walked::of(&t.pts);
    let (mut lo, mut hi) = (glam::Vec3::MAX, glam::Vec3::MIN);
    for p in &pts {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let mut s = format!(
        "  trail: {} points over {:.1} s ({}) — walked {:.1} m, net {:.1} m, {} reversals, box {:.1} x {:.1} x {:.1} m{}\n   ",
        pts.len(),
        pts.len() as f32 / 4.0,
        trail_ends(snap, t),
        w.path,
        w.net,
        w.reversals,
        hi.x - lo.x,
        hi.y - lo.y,
        hi.z - lo.z,
        if w.nowhere() { "  <- going nowhere" } else { "" },
    );
    // Every fourth point: one a second.
    for p in pts.iter().step_by(4) {
        let _ = write!(s, " ({:.1},{:.1},{:.1})", p.x, p.y, p.z);
    }
    s.push('\n');
    s
}

fn last_of(snap: &Snapshot, id: u32) -> Option<(&Frame, &Probe)> {
    snap.frames
        .iter()
        .rev()
        .find_map(|f| f.units.iter().find(|p| p.id == id).map(|p| (f, p)))
}

fn focus(snap: &Snapshot) -> String {
    let f = &snap.focus;
    if f.how == "none" && f.dist == 0.0 {
        return "FOCUS  none (no human)\n".into();
    }
    let mut s = format!(
        "FOCUS  unit {} (picked by: {})  eye ({:.1},{:.1},{:.1}) dir ({:.2},{:.2},{:.2})  point under the reticle {}\n",
        if f.unit >= 0 { format!("#{}", f.unit) } else { "none".into() },
        f.how, f.eye[0], f.eye[1], f.eye[2], f.dir[0], f.dir[1], f.dir[2],
        if f.dist >= 0.0 {
            format!("({:.1},{:.1},{:.1}) at {:.1} m", f.point[0], f.point[1], f.point[2], f.dist)
        } else {
            "none (sky)".into()
        },
    );
    if let Some((fr, p)) = u32::try_from(f.unit).ok().and_then(|id| last_of(snap, id)) {
        let _ = writeln!(s, "  {:.1} s  {}", secs(snap, fr.tick), unit_line(snap, p));
        s.push_str(&trail(snap, p.id));
    }
    s
}

fn troubled(snap: &Snapshot) -> String {
    let mut s = String::from(
        "TROUBLE  bots with a no-progress run or a blocked-steering run over 3 s (longest first), then every trail that went nowhere\n",
    );
    let mut rows: Vec<(f32, String)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for f in snap.frames.iter().rev() {
        for p in &f.units {
            if p.human || !seen.insert((p.id, p.gen)) {
                continue;
            }
            let stuck = f32::from(p.stuck) / HZ;
            let run = blocked_run(&snap.frames, p.id, p.gen).filter(|r| r.secs > 3.0);
            if stuck <= 3.0 && run.is_none() {
                continue;
            }
            let mut line = format!("  {}", unit_line(snap, p));
            if let Some(w) = whys(snap, p.id, 4) {
                let _ = write!(line, "\n      {w}");
            }
            if let Some(r) = &run {
                let _ = write!(
                    line,
                    "\n      steering blocked for {:.1} s (net {:.1} m), until {:.1} s, at ({:.1},{:.1},{:.1}): {:?} {}",
                    r.secs,
                    r.net,
                    secs(snap, r.end),
                    r.at.pos[0],
                    r.at.pos[1],
                    r.at.pos[2],
                    r.at.stop,
                    body_name(snap, r.at.body)
                );
            }
            rows.push((stuck.max(run.map_or(0.0, |r| r.secs)), line));
        }
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    if rows.is_empty() {
        s.push_str("  none\n");
    }
    for (_, l) in rows.iter().take(24) {
        let _ = writeln!(s, "{l}");
    }
    if rows.len() > 24 {
        let _ = writeln!(s, "  ... and {} more", rows.len() - 24);
    }
    // Every unit's trail, not only the focus unit's: a bot that steers `fwd`
    // back and forth has no blocked run and no stuck clock (card BLKF).
    let mut lost: Vec<(f32, String)> = Vec::new();
    for t in &snap.trails {
        let w = Walked::of(&t.pts);
        let p = last_of(snap, t.id).map(|(_, p)| p);
        if !w.nowhere() || p.is_some_and(|p| p.human || p.veh >= 0) {
            continue;
        }
        let mut line = format!(
            "  #{:<3} walked {:.1} m for {:.1} m net, {} reversals in {:.1} s ({})",
            t.id,
            w.path,
            w.net,
            w.reversals,
            t.pts.len() as f32 / 4.0,
            trail_ends(snap, t)
        );
        if let Some(p) = p {
            let _ = write!(line, "\n      {}", unit_line(snap, p));
        }
        lost.push((w.path / w.net.max(0.1), line));
    }
    lost.sort_by(|a, b| b.0.total_cmp(&a.0));
    let _ = writeln!(
        s,
        " going nowhere (trail: over 3 m walked, under a quarter of it net; worst first): {}",
        if lost.is_empty() { "none" } else { "" }
    );
    for (_, l) in &lost {
        let _ = writeln!(s, "{l}");
    }
    s
}

fn near(snap: &Snapshot, c: glam::Vec3, r: f32, title: &str) -> String {
    let mut s = format!("{title}\n");
    let Some(f) = snap.frames.last() else {
        return s + "  no frames\n";
    };
    let mut v: Vec<(f32, &Probe)> = f
        .units
        .iter()
        .map(|p| (glam::Vec3::from(p.pos).distance(c), p))
        .filter(|(d, _)| *d <= r)
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    if v.is_empty() {
        s.push_str("  none\n");
    }
    for (d, p) in v {
        let _ = writeln!(s, "  {d:>5.1} m  {}", unit_line(snap, p));
    }
    s
}

fn world(snap: &Snapshot) -> String {
    let Some(w) = snap.world.last() else {
        return "WORLD  no world frame yet\n".into();
    };
    let mut s = format!(
        "WORLD  at {:.1} s: tickets {}/{}  bleed {:.2}/{:.2} per s  alive {}/{}\n",
        secs(snap, w.tick),
        w.tickets[0],
        w.tickets[1],
        w.bleed[0],
        w.bleed[1],
        w.alive[0],
        w.alive[1]
    );
    for (i, p) in w.posts.iter().enumerate() {
        let _ = write!(
            s,
            "  {:<28} owner {:<2} capture {:.2} (team {}) neutral {:.2}",
            post_name(snap, i as i32),
            p.owner,
            p.cap,
            p.cap_team,
            p.neutral
        );
        // The running meter as the capture icon shows it (card CPBAR;
        // schema 5): its fraction, whose it is, which way it moves.
        if snap.schema >= 5 && p.times[0] > 0.0 && p.times[1] > 0.0 {
            let st = p.state();
            let inside = p.inside.map(u32::from);
            let (meter, frac, team) = match st.owner {
                None => ("capture", st.capture / st.capture_time, p.cap_team),
                Some(o) => ("neutralise", st.neutral / st.neutralize_time, o as i8),
            };
            let way = match game::capture_feed::meter_sign(&st, &inside) {
                1 => "rising",
                -1 => "falling",
                _ => "holding",
            };
            let contested = if inside.iter().all(|&n| n > 0) {
                ", contested"
            } else {
                ""
            };
            let _ = write!(
                s,
                " | {meter} {frac:.2} (team {team}) {way}{contested}, inside {}/{}",
                inside[0], inside[1]
            );
        }
        s.push('\n');
    }
    let idle_turret =
        |v: &game::blackbox::VehicleRow| v.kind == 3 && v.riders.iter().all(|r| *r < 0);
    for v in w.vehicles.iter().filter(|v| !idle_turret(v)) {
        let riders: Vec<String> = v
            .riders
            .iter()
            .filter(|r| **r >= 0)
            .map(|r| format!("#{r}"))
            .collect();
        let _ =
            writeln!(
            s,
            "  vehicle {:<3} {:<24} kind {} team {:<2} ({:.1},{:.1},{:.1}) hp {:.0} riders [{}]",
            v.id,
            snap.names.vehicles.get(v.class as usize).map_or("?", String::as_str),
            v.kind,
            v.team,
            v.pos[0],
            v.pos[1],
            v.pos[2],
            v.hp,
            riders.join(" ")
        );
    }
    let open = w.doors.iter().filter(|d| **d > 0.0).count();
    let _ = writeln!(
        s,
        "  empty turrets: {}   doors: {} of {} open",
        w.vehicles.iter().filter(|v| idle_turret(v)).count(),
        open,
        w.doors.len()
    );
    s
}

fn events(snap: &Snapshot, span: Option<f32>) -> String {
    let from = span.map_or(0, |s| snap.header.tick.saturating_sub((s * HZ) as u64));
    let mut s = match span {
        Some(x) => format!("EVENTS  last {x:.0} s\n"),
        None => "EVENTS  all\n".to_owned(),
    };
    let mut n = 0;
    for e in snap.events.iter().filter(|e| e.tick >= from) {
        let _ = writeln!(s, "  {}", event_line(snap, e));
        n += 1;
    }
    if n == 0 {
        s.push_str("  none\n");
    }
    s
}

/// Card WHY: unit `id`'s last `n` reasons as one line ("12.3 s fight:seen,
/// 14.0 s FAIL wall arc 31 in travel, ..."), `None` when the file has none
/// (before schema 9, or a unit that has changed nothing yet).
fn whys(snap: &Snapshot, id: u32, n: usize) -> Option<String> {
    let w = snap.why.iter().find(|w| w.id == id)?;
    let skip = w.rows.len().saturating_sub(n);
    let rows: Vec<String> = w.rows[skip..]
        .iter()
        .map(|r| {
            let t = secs(snap, r.tick);
            if r.fail {
                let arc = if r.arc >= 0 {
                    format!(" arc {}", r.arc)
                } else {
                    String::new()
                };
                format!("{t:.1} s FAIL {}{arc} in {}", r.why.tag(), r.doing.tag())
            } else {
                format!("{t:.1} s {}:{}", r.doing.tag(), r.why.tag())
            }
        })
        .collect();
    Some(format!("why (gen {}): {}", w.gen, rows.join(", ")))
}

fn unit_view(snap: &Snapshot, id: u32) -> String {
    let mut s = format!("UNIT #{id}  every frame, oldest first\n");
    for f in &snap.frames {
        if let Some(p) = f.units.iter().find(|p| p.id == id) {
            let _ = writeln!(s, "  {:>6.1} s  {}", secs(snap, f.tick), unit_line(snap, p));
        }
    }
    s.push_str(&trail(snap, id));
    match whys(snap, id, usize::MAX) {
        Some(w) => {
            let _ = writeln!(s, "  {w}");
        }
        None if snap.schema < 9 => s.push_str("  why: none (a file before schema 9)\n"),
        None => s.push_str("  why: none\n"),
    }
    s.push_str("  its events:\n");
    for e in &snap.events {
        let unit_event = !matches!(
            e.kind,
            Ev::Captured
                | Ev::Neutralized
                | Ev::Saved
                | Ev::Defeated
                | Ev::TimeUp
                | Ev::Voice
                | Ev::Order
        );
        if unit_event && (e.a == id as i32 || (e.kind == Ev::Kill && e.b == id as i32)) {
            let _ = writeln!(s, "  {}", event_line(snap, e));
        }
    }
    s
}

/// The line a headless follow-up starts from.
fn replay_spot(snap: &Snapshot, id: Option<u32>) -> String {
    let id = id.or_else(|| u32::try_from(snap.focus.unit).ok());
    let Some((f, p)) = id.and_then(|i| last_of(snap, i)) else {
        return "replay-spot: no unit (give an id: --replay-spot <id>)\n".into();
    };
    let h = &snap.header;
    format!(
        "spot map={} seed={} tick={} unit={} class={} pos={:.1},{:.1},{:.1} goal={} leg={}->{} stop={:?} {} build={}\n  \
         a headless round is not this round (no human, the map's own team sizes): look at the PLACE, not the tick —\n  \
         FLOOR_GRID={:.0},{:.0},8,1,{:.0} STUCK_CENSUS=1 ROUND_VERBOSE=1 cargo run --release -p game --example round_to_outcome -- 60000 {} {}\n  \
         scripts/sweep_seeds.py for the map's 8-seed table; BLACKBOX_ON=stuck on round_to_outcome writes a headless snapshot of the first 30 s stuck run\n",
        h.meta.map,
        h.meta.seed,
        f.tick,
        p.id,
        class_name(snap, p),
        p.pos[0],
        p.pos[1],
        p.pos[2],
        p.goal_label(),
        p.from,
        p.next,
        p.stop,
        body_name(snap, p.body),
        if h.meta.build_commit.is_empty() {
            &h.meta.commit
        } else {
            &h.meta.build_commit
        },
        p.pos[0],
        p.pos[2],
        p.pos[1] + 3.0,
        h.meta.seed,
        h.meta.map,
    )
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = None;
    let mut view = None;
    let mut it = args.iter().peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--unit" | "--near" => {
                let v = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{a} wants a value"))?;
                view = Some((a.clone(), Some(v.clone())));
            }
            "--events" | "--sounds" | "--frames" => view = Some((a.clone(), None)),
            "--replay-spot" => {
                let v = it.next_if(|n| n.parse::<u32>().is_ok()).cloned();
                view = Some((a.clone(), v));
            }
            f if !f.starts_with("--") => file = Some(std::path::PathBuf::from(f)),
            other => anyhow::bail!(
                "unknown view {other} (--unit <id>, --near x,y,z,r, --events, --sounds, --frames, --replay-spot [id])"
            ),
        }
    }
    let path = file.unwrap_or_else(|| {
        // Run from `engine/` or from the repository root.
        let d = game::blackbox::default_dir();
        if d.exists() {
            d.join("last.json")
        } else {
            std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../artifacts/playtest"
            ))
            .join("last.json")
        }
    });
    let snap = game::blackbox::read(&path)?;
    println!("{}  ({})", path.display(), {
        let b = std::fs::metadata(&path).map_or(0, |m| m.len());
        format!("{:.2} MB", b as f64 / (1024.0 * 1024.0))
    });
    print!("{}", header(&snap));
    let all_frames = view.as_ref().is_some_and(|v| v.0 == "--frames");
    print!("{}", frame_record(&snap, all_frames));
    match view.as_ref().map(|(k, v)| (k.as_str(), v.as_deref())) {
        Some(("--unit", Some(v))) => print!("{}", unit_view(&snap, v.parse()?)),
        Some(("--near", Some(v))) => {
            let a: Vec<f32> = v
                .split(',')
                .map(|x| x.trim().parse())
                .collect::<Result<_, _>>()?;
            let &[x, y, z, r] = a.as_slice() else {
                anyhow::bail!("--near x,y,z,r");
            };
            print!(
                "{}",
                near(
                    &snap,
                    glam::Vec3::new(x, y, z),
                    r,
                    &format!("NEAR ({x},{y},{z}) within {r} m, last frame")
                )
            );
        }
        Some(("--events", _)) => print!("{}", events(&snap, None)),
        Some(("--sounds", _)) => print!("{}", game::blackbox::sound_digest(&snap, None)),
        Some(("--frames", _)) => {}
        Some(("--replay-spot", v)) => {
            print!("{}", replay_spot(&snap, v.and_then(|x| x.parse().ok())))
        }
        _ => {
            print!("{}", human(&snap, 10.0));
            print!("{}", focus(&snap));
            print!("{}", troubled(&snap));
            let me = snap
                .frames
                .last()
                .and_then(|f| f.units.iter().find(|p| p.human))
                .map(|p| glam::Vec3::from(p.pos));
            match me {
                Some(c) => print!(
                    "{}",
                    near(&snap, c, 15.0, "NEAR THE HUMAN  within 15 m, last frame")
                ),
                None => println!("NEAR THE HUMAN  no human in the last frame"),
            }
            print!("{}", world(&snap));
            print!("{}", events(&snap, Some(30.0)));
            print!("{}", game::blackbox::sound_digest(&snap, Some(10.0)));
        }
    }
    Ok(())
}
