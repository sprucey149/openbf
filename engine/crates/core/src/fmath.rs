//! The sim's transcendental functions, on one implementation everywhere.
//!
//! `f32::sin_cos`/`atan2`/`asin` in `std` call the *host* libm — glibc here,
//! a different one on Windows, macOS or wasm — and those libms disagree in
//! the last bits. A replay is a bit-exact contract (PLAN §Determinism), so a
//! log recorded on one box has to reproduce on every other one; routing the
//! sim's transcendentals through the `libm` crate (a pure-Rust port of MUSL's
//! libm, no platform code) is what makes that true. `core::fmath` is the
//! single door.
//!
//! What deliberately stays on `std`:
//! - `sqrt`: IEEE-754 requires it to be correctly rounded, so every platform
//!   already agrees bit-for-bit, and `f32::sqrt` is one hardware instruction.
//!   That covers glam's `length`/`normalize`/`distance` too — sqrt-only.
//! - `%`, `abs`, `clamp`, `to_radians` and plain `+ - * /`: each is exact or
//!   a single correctly-rounded operation, pinned by IEEE-754 itself.
//!
//! The rule is enforced, not just documented: `no_std_transcendentals_in_core`
//! below re-reads `core/src` and fails if any file other than this one calls
//! a transcendental method, primitive path/function value or denied glam
//! helper outside `#[cfg(test)]`, and
//! `no_std_transcendentals_in_game_sim_inputs` does the same over `game/src`,
//! where the load path builds sim inputs, all but its presentation files
//! and the exact, counted drawing expressions in mixed files.
//! `no_wall_clock_in_core` walks the same files for `Instant`, `SystemTime`
//! and `std::time`, allowing only two diagnostic timers.

/// Sine and cosine of `x` radians together, as `(sin, cos)`.
#[inline]
pub fn sin_cos(x: f32) -> (f32, f32) {
    (libm::sinf(x), libm::cosf(x))
}

/// The angle in radians of the vector `(x, y)`, in `[-pi, pi]`.
#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

/// Arcsine of `x` in radians. NaN outside `[-1, 1]` — clamp before calling.
#[inline]
pub fn asin(x: f32) -> f32 {
    libm::asinf(x)
}

/// e^x.
#[inline]
pub fn exp(x: f32) -> f32 {
    libm::expf(x)
}

#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    libm::powf(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed-magnitude f32 bits mapped to a monotonic integer, so that
    /// adjacent floats are adjacent ordinals and subtracting two of them
    /// counts the representable floats between the pair.
    fn ordinal(x: f32) -> i64 {
        let bits = x.to_bits();
        if bits & 0x8000_0000 != 0 {
            -i64::from(bits & 0x7fff_ffff)
        } else {
            i64::from(bits)
        }
    }

    /// Distance between two f32s in units in the last place.
    fn ulps(a: f32, b: f32) -> u64 {
        assert!(a.is_finite() && b.is_finite(), "{a} vs {b}");
        (ordinal(a) - ordinal(b)).unsigned_abs()
    }

    /// How far `fmath` (the `libm` crate) can land from `std` (the host
    /// libm, glibc on the dev box) over a dense sweep. Both are sub-ulp
    /// implementations of the same functions, but they are different code,
    /// so the sim hash is allowed to move when a call site is ported — this
    /// test is the record of by how much.
    ///
    /// Measured on the dev box (x86_64, glibc 2.43): max **1 ulp** for all
    /// four functions — not 0, so porting a call site is allowed to move the
    /// sim hash. What actually moved when P9(b) landed: nothing short and
    /// scripted — the `--sim-hash 600` probe and both walk-replay logs came
    /// back bit-identical, because those inputs land where the two libms
    /// agree — but a full 60 000-tick 16-v-16 AI battle does diverge, since
    /// 1 ulp of aim fed back through hit/miss decisions is chaotic. A
    /// std-backed build of this module reproduces the pre-P9(b) battle
    /// results exactly, so that divergence is the libm swap and nothing
    /// else. See the P9(b) Log.
    ///
    /// The assertion is held at 2 ulp: it is a claim about the two
    /// implementations being interchangeable, not about this machine's libc,
    /// and the sweep prints the measured numbers when they move.
    #[test]
    fn fmath_within_2ulp_of_std() {
        let mut worst = [0u64; 4]; // sin, cos, atan2, asin
        let mut at = [0f32; 4];

        // sin/cos over +-8pi, plus the small-angle and large-argument ends
        // the sim actually sees (yaw wraps to +-pi, pitch is tiny).
        let n = 2_000_000;
        for i in 0..=n {
            let x = (i as f32 / n as f32 - 0.5) * 2.0 * 8.0 * std::f32::consts::PI;
            let (s, c) = sin_cos(x);
            let (rs, rc) = x.sin_cos();
            if ulps(s, rs) > worst[0] {
                worst[0] = ulps(s, rs);
                at[0] = x;
            }
            if ulps(c, rc) > worst[1] {
                worst[1] = ulps(c, rc);
                at[1] = x;
            }
        }

        // atan2 over a full-circle grid of directions, including the axes.
        let g = 1200;
        for iy in 0..=g {
            for ix in 0..=g {
                let y = iy as f32 / g as f32 * 2.0 - 1.0;
                let x = ix as f32 / g as f32 * 2.0 - 1.0;
                if x == 0.0 && y == 0.0 {
                    continue;
                }
                let d = ulps(atan2(y, x), y.atan2(x));
                if d > worst[2] {
                    worst[2] = d;
                    at[2] = y;
                }
            }
        }

        // asin over its whole domain (the sim feeds it a clamped unit-vector
        // component, so the endpoints matter).
        let n = 2_000_000;
        for i in 0..=n {
            let x = (i as f32 / n as f32) * 2.0 - 1.0;
            let d = ulps(asin(x), x.asin());
            if d > worst[3] {
                worst[3] = d;
                at[3] = x;
            }
        }

        println!(
            "fmath vs std, max ulp: sin {} (x={}), cos {} (x={}), \
             atan2 {} (y={}), asin {} (x={})",
            worst[0], at[0], worst[1], at[1], worst[2], at[2], worst[3], at[3]
        );
        for (name, (&w, &x)) in ["sin", "cos", "atan2", "asin"]
            .iter()
            .zip(worst.iter().zip(at.iter()))
        {
            assert!(w <= 2, "{name}: {w} ulp from std at {x}");
        }
    }

    /// Every `std` float method that routes to the host libm, written as
    /// the text of the call. Method and primitive path forms (including
    /// function values such as `.map(f32::cos)`) are checked separately.
    /// `sqrt`, arithmetic, `abs`, `clamp` and `to_radians` remain allowed.
    const BANNED: &[&str] = &[
        // trigonometry
        ".sin_cos(",
        ".sin(",
        ".cos(",
        ".tan(",
        ".asin(",
        ".acos(",
        ".atan(",
        ".atan2(",
        // hyperbolics (std builds these out of `ln`/`ln_1p`, so they carry
        // the host libm with them just the same)
        ".sinh(",
        ".cosh(",
        ".tanh(",
        ".asinh(",
        ".acosh(",
        ".atanh(",
        // exponentials and logarithms
        ".exp(",
        ".exp2(",
        ".exp_m1(",
        ".ln(",
        ".ln_1p(",
        ".log(",
        ".log2(",
        ".log10(",
        // powers and roots
        ".powf(",
        ".powi(",
        ".cbrt(",
        ".hypot(",
    ];

    // Glam helpers that can hide trigonometry. Matrix/quaternion conversion
    // from existing components (`from_quat`, `from_mat3`, and
    // `from_rotation_translation`) uses arithmetic/sqrt and is allowed.
    const GLAM_BANNED: &[&str] = &[
        "from_rotation_x",
        "from_rotation_y",
        "from_rotation_z",
        "from_axis_angle",
        "from_scaled_axis",
        "from_euler",
        "from_rotation_arc",
        "from_rotation_arc_colinear",
        "from_rotation_arc_2d",
        "from_angle",
        "from_scale_angle",
        "from_scale_angle_translation",
        "angle_between",
        "angle_to",
        "to_angle",
        "to_axis_angle",
        "to_scaled_axis",
        "to_euler",
        "slerp",
        "rotate_towards",
        "perspective_rh",
        "perspective_lh",
        "perspective_rh_gl",
        "perspective_infinite_rh",
        "perspective_infinite_lh",
        "perspective_infinite_reverse_rh",
        "perspective_infinite_reverse_lh",
    ];

    fn ident(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    /// Primitive path matches end at an identifier boundary, so `cosh`
    /// does not also match `cos`. Whitespace cannot hide a path or call.
    fn path_call(line: &str, path: &str) -> bool {
        line.match_indices(path).any(|(i, _)| {
            (path.starts_with("::") || !line[..i].chars().next_back().is_some_and(ident))
                && !line[i + path.len()..].chars().next().is_some_and(ident)
        })
    }

    /// `line` without its `//` comment and its spacing, bar one space between
    /// two identifiers (`return f32::sin`): what the scans match against.
    fn compact(line: &str) -> String {
        let mut compact = String::new();
        let mut chars = line.split("//").next().unwrap_or("").chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_whitespace() {
                // Preserve identifier separation (`return f32::sin`),
                // while discarding spacing around `.` and `::`.
                if compact.chars().next_back().is_some_and(ident)
                    && chars.peek().copied().is_some_and(ident)
                {
                    compact.push(' ');
                }
            } else {
                compact.push(c);
            }
        }
        compact
    }

    /// One diagnostic per offending line, retaining the original line number.
    fn banned_calls_in(name: &str, code: &str) -> Vec<String> {
        code.lines()
            .enumerate()
            .filter_map(|(n, line)| {
                let compact = compact(line);
                let primitive = BANNED.iter().any(|pat| {
                    let method = pat.trim_start_matches('.').trim_end_matches('(');
                    compact.contains(pat)
                        || ["f32", "f64"]
                            .iter()
                            .any(|ty| path_call(&compact, &format!("{ty}::{method}")))
                });
                let glam = GLAM_BANNED.iter().any(|method| {
                    compact.contains(&format!(".{method}("))
                        || path_call(&compact, &format!("::{method}"))
                });
                (primitive || glam).then(|| format!("{name}:{}: {}", n + 1, line.trim()))
            })
            .collect()
    }

    /// A guard nobody has watched fail is a guard nobody has tested. Feed
    /// the scan one call per banned pattern and check each is caught
    /// exactly ONCE — which is also what proves the rows do not shadow each
    /// other — then check the shapes the module docs allow stay clean.
    #[test]
    fn the_referee_catches_every_banned_call() {
        for pat in BANNED {
            let name = pat.trim_start_matches('.').trim_end_matches('(');
            let hits = banned_calls_in("probe.rs", &format!("let y = a.{name}(b);\n"));
            assert_eq!(hits.len(), 1, "{pat} is not caught exactly once: {hits:?}");
        }
    }

    #[test]
    fn the_referee_catches_primitive_paths_and_function_values() {
        for pat in BANNED {
            let method = pat.trim_start_matches('.').trim_end_matches('(');
            for ty in ["f32", "f64"] {
                for probe in [
                    format!("let y = {ty} :: {method} (x);"),
                    format!("let y = xs.map({ty}::{method});"),
                    format!("return {ty}::{method}(x);"),
                ] {
                    assert_eq!(banned_calls_in("probe.rs", &probe).len(), 1, "{probe}");
                }
            }
        }
    }

    #[test]
    fn the_referee_catches_glam_helpers() {
        for method in GLAM_BANNED {
            for probe in [
                format!("let y = glam::Quat::{method}(x);"),
                format!("let y = q . {method} (x);"),
            ] {
                assert_eq!(banned_calls_in("probe.rs", &probe).len(), 1, "{probe}");
            }
        }
    }

    #[test]
    fn the_referee_allows_deterministic_arithmetic() {
        for allowed in [
            "let y = a.sqrt();",
            "let y = fmath::sin(x) + fmath::cos(y);",
            "let y = f32::sqrt(x) + myf32::sin(x);",
            "let y = Mat4::from_rotation_translation(q, t);",
            "let y = Quat::from_mat3(m); // q.slerp(other, t)",
            "let y = a.abs().floor().clamp(0.0, 1.0).to_radians();",
            "let y = v.arctan(w) + q.mylog(r) + run_log(s) + demo_log(t);",
        ] {
            assert!(
                banned_calls_in("probe.rs", allowed).is_empty(),
                "false positive on: {allowed}"
            );
        }
    }

    // Exact rendering expressions in mixed files, not whole-file exemptions.
    // Walk draws placeholder boxes/bolts; skin blends drawn bones; soldier_vis
    // builds draw matrices; holo/spawn_preview build UI models; event_feed's
    // ambush_edge returns a HUD chevron. Blend restores tick poses before stepping.
    // None of these expressions supplies a simulation input. Each exemption is
    // consumed exactly once: moving, changing or duplicating one needs review.
    const DRAW_CALLS: &[(&str, &str)] = &[
        ("walk.rs", "* Mat4::from_rotation_y(s.yaw)"),
        ("walk.rs", "* Mat4::from_rotation_y(u.soldier.yaw)"),
        (
            "walk.rs",
            "let rot = Quat::from_rotation_arc(Vec3::Z, dir);",
        ),
        (
            "skin.rs",
            "rotation: self.rotation.slerp(other.rotation, t),",
        ),
        // Kit Fisto's tentacle sway: a pose nothing reads back.
        ("skin.rs", "let x = reach * (t * 3.1 + phase).sin();"),
        (
            "skin.rs",
            "let y = reach * 0.6 * (t * 1.9 + phase * 1.3).sin();",
        ),
        (
            "skin.rs",
            "bone.rotation *= Quat::from_euler(glam::EulerRot::XYZ, x, y, 0.0);",
        ),
        (
            "soldier_vis.rs",
            "Mat4::from_translation(s.pos) * Mat4::from_rotation_y(s.yaw + std::f32::consts::PI)",
        ),
        ("holo.rs", "* Mat4::from_rotation_y(-h.spin * c.rotate)"),
        ("holo.rs", "* Mat4::from_rotation_x(-h.tilt)"),
        (
            "event_feed.rs",
            "Quat::from_rotation_y(me.soldier.yaw + std::f32::consts::PI),",
        ),
        ("spawn_preview.rs", ") * Mat4::from_rotation_y(yaw)"),
        ("render_blend.rs", "o.rot = p.rot.slerp(o.rot, t);"),
        (
            "render_blend.rs",
            "out.rot = p.rot.slerp(filtered_walker_heading(rot, c.rot), t);",
        ),
        (
            "render_blend.rs",
            "if previous.angle_between(current) >= WALKER_HEADING_SNAP_RAD {",
        ),
        (
            "render_blend.rs",
            "previous.slerp(current, WALKER_HEADING_BLEND).normalize()",
        ),
    ];

    /// Every `.rs` file under `dir` as `(path under dir, non-test code)`,
    /// down into the module directories a split leaves (`collision/`: a flat
    /// walk would let every item a split moved slip past the scan without a
    /// word), skipping the files `skip` names by their path under `dir` (a
    /// name ending in `/` skips that whole directory) and each file's
    /// trailing `#[cfg(test)] mod tests` — inline, or `mod tests;` in
    /// `tests.rs`.
    fn non_test_sources(dir: &std::path::Path, skip: &[&str]) -> Vec<(String, String)> {
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        let mut dirs = vec![dir.to_path_buf()];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).expect("source dir readable") {
                let p = e.expect("dir entry").path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") {
                    files.push(p);
                }
            }
        }
        files.sort();
        assert!(
            files.len() > 10,
            "referee scanned almost nothing: {files:?}"
        );

        let mut sources = Vec::new();
        for path in &files {
            let name = path
                .strip_prefix(dir)
                .expect("under the scanned dir")
                .to_string_lossy()
                .replace('\\', "/");
            // A `<module>/tests.rs` is the test module its parent declares
            // as `#[cfg(test)] mod tests;` (checked below): test code.
            if name.ends_with("/tests.rs")
                || skip
                    .iter()
                    .any(|s| name == *s || (s.ends_with('/') && name.starts_with(s)))
            {
                continue;
            }
            let text = std::fs::read_to_string(path)
                .expect("read source")
                .replace("\r\n", "\n");

            // The LAST `#[cfg(test)]` of a file is its trailing `mod tests`,
            // and cutting there drops the test code. One before it is a test
            // SEAM in the code (card FGHT: `ai/behave.rs` `style`): one
            // braced statement, skipped to its closing brace. Anything else
            // would leave the referee blind without saying so — fail loudly.
            let cuts: Vec<usize> = text.match_indices("#[cfg(test)]").map(|(i, _)| i).collect();
            let code = match cuts.split_last() {
                Some((&last, seams)) => {
                    assert!(
                        text[last..].starts_with("#[cfg(test)]\nmod tests {")
                            || text[last..].trim_end() == "#[cfg(test)]\nmod tests;"
                            // shares its fixtures with other test modules
                            || text[last..].trim_end() == "#[cfg(test)]\npub(crate) mod tests;"
                            || text[last..].trim_end() == "#[cfg(test)]\npub(super) mod tests;",
                        "{name}: the last #[cfg(test)] is not the trailing `mod tests` \
                         — teach the referee to skip it"
                    );
                    let (mut code, mut at) = (String::new(), 0);
                    for &i in seams {
                        let open = i + text[i..last].find('{').expect("a seam gates a block");
                        let mut depth = 0i32;
                        let close = text[open..last]
                            .char_indices()
                            .find(|&(_, c)| {
                                depth += i32::from(c == '{') - i32::from(c == '}');
                                depth == 0
                            })
                            .map(|(k, _)| open + k + 1);
                        let close = close.unwrap_or_else(|| panic!("{name}: a seam's block"));
                        code.push_str(&text[at..i]);
                        at = close;
                    }
                    code.push_str(&text[at..last]);
                    code
                }
                None => text.clone(),
            };
            sources.push((name, code));
        }
        sources
    }

    /// The banned calls in the non-test code under `dir`
    /// ([`non_test_sources`]), less the exact [`DRAW_CALLS`] exceptions.
    fn offenders_under(dir: &std::path::Path, skip: &[&str]) -> Vec<String> {
        let mut offenders = Vec::new();
        for (name, code) in non_test_sources(dir, skip) {
            let mut hits = banned_calls_in(&name, &code);
            for &(file, expression) in DRAW_CALLS.iter().filter(|(file, _)| *file == name) {
                let suffix = format!(": {expression}");
                let count = hits.iter().filter(|h| h.ends_with(&suffix)).count();
                assert_eq!(
                    count, 1,
                    "{file}: stale or duplicated drawing exception: {expression}"
                );
                hits.retain(|h| !h.ends_with(&suffix));
            }
            offenders.extend(hits);
        }
        offenders
    }

    /// The sim must not reach the host libm behind `fmath`'s back: a source
    /// scan of `core/src` for method/path calls and glam helpers outside this file
    /// and outside `#[cfg(test)]`.
    #[test]
    fn no_std_transcendentals_in_core() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let offenders = offenders_under(dir, &["fmath.rs"]);
        assert!(
            offenders.is_empty(),
            "the sim's transcendentals must go through core::fmath (the libm \
             crate), not the host libm — a replay has to reproduce bit-for-bit \
             on every platform. Offending lines:\n{}",
            offenders.join("\n")
        );
    }

    /// The same rule one crate out: `game` builds sim inputs at load time
    /// (a spawn node's yaw in `layout.rs` reaches `Sim::hash` through every
    /// respawn — queue DET1), so `game/src` is scanned too, all but the
    /// files that only draw, play sound or animate what the sim already
    /// decided. A file joins this list only if nothing it computes can
    /// reach a `Sim`.
    #[test]
    fn no_std_transcendentals_in_game_sim_inputs() {
        const PRESENTATION: &[&str] = &[
            "viewer.rs",
            "viewer/",
            "gallery.rs",
            "sounds.rs",
            // The same presentation-only mixer math extracted from sounds.rs.
            "sounds/mixing.rs",
            "water_anim.rs",
            // Effect triangles and cosmetic debris never write simulation state.
            "fx/draw.rs",
            "fx/chunks.rs",
            // The round renderer: it reads `Sim::bolts` and returns draws
            // (glyphs, ribbons, a missile's model matrix); no draw is read back.
            "bolt_vis.rs",
            "fx/force.rs",
            // Which soldiers draw and at what detail: a per-frame draw list; the
            // simulation keeps every soldier and never reads the choice.
            "soldier_vis/lod.rs",
            // The soldier animator poses what the sim decided; it holds no
            // `Sim` and nothing reads a pose back (card ANIML).
            "animator.rs",
            "animator/",
            // Keeps the display awake while a round runs; it reads no `Sim`.
            "main/windowed/idle.rs",
        ];
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../game/src"));
        let offenders = offenders_under(dir, PRESENTATION);
        assert!(
            offenders.is_empty(),
            "game/src computes sim inputs outside the presentation files: \
             route these through core::fmath (or, if the file truly only \
             draws, add it to PRESENTATION with the reason). Offending \
             lines:\n{}",
            offenders.join("\n")
        );
    }

    /// The only files under `core/src` whose non-test code may read the wall
    /// clock, each with why that cannot move the sim.
    const WALL_CLOCK_ALLOWED: &[(&str, &str)] = &[
        (
            "sim/step.rs",
            "`ai_wall_ms`, `tick_cost`, `think_hist` (via `NavTick::think_ns`): diagnostic timing, never hashed or read by the sim",
        ),
        (
            "ai/tactic.rs",
            "`TableStats.secs`: diagnostic timing, never hashed or read by the sim",
        ),
    ];

    /// One diagnostic per non-comment line that names `Instant`,
    /// `SystemTime` (as whole identifiers) or the `std::time` path.
    fn wall_clock_in(name: &str, code: &str) -> Vec<String> {
        code.lines()
            .enumerate()
            .filter_map(|(n, line)| {
                let compact = compact(line);
                ["Instant", "SystemTime", "std::time"]
                    .iter()
                    .any(|w| path_call(&compact, w))
                    .then(|| format!("{name}:{}: {}", n + 1, line.trim()))
            })
            .collect()
    }

    /// The wall-clock scan, watched failing and passing on probes.
    #[test]
    fn the_wall_clock_referee_catches_clock_reads() {
        for probe in [
            "let t0 = std::time::Instant::now();",
            "let t0 = std :: time :: Instant :: now();",
            "use std::time;",
            "let t = Instant::now();",
            "let t = SystemTime::now();",
            "fn f(t: Instant) {}",
        ] {
            assert_eq!(wall_clock_in("probe.rs", probe).len(), 1, "{probe}");
        }
        for allowed in [
            "// the Instant Action option, std::time",
            "let instant_kill = mystd::time_left + InstantAction;",
            "let s = \"x\"; // Instant::now()",
        ] {
            assert!(
                wall_clock_in("probe.rs", allowed).is_empty(),
                "false positive on: {allowed}"
            );
        }
    }

    /// `core` is deterministic: its non-test code reads no wall clock, bar
    /// the two diagnostic timers in [`WALL_CLOCK_ALLOWED`]. An allowlisted
    /// file that no longer reads the clock is a stale entry and fails too.
    #[test]
    fn no_wall_clock_in_core() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut offenders = Vec::new();
        let mut used = vec![false; WALL_CLOCK_ALLOWED.len()];
        // This file is skipped as in `no_std_transcendentals_in_core`: its
        // tests quote the probes, and its non-test code is the libm wrappers.
        for (name, code) in non_test_sources(dir, &["fmath.rs"]) {
            let hits = wall_clock_in(&name, &code);
            match WALL_CLOCK_ALLOWED.iter().position(|(f, _)| *f == name) {
                Some(k) => used[k] |= !hits.is_empty(),
                None => offenders.extend(hits),
            }
        }
        for ((file, _), used) in WALL_CLOCK_ALLOWED.iter().zip(used) {
            assert!(used, "{file}: stale wall-clock allowance, drop it");
        }
        assert!(
            offenders.is_empty(),
            "core reads the wall clock outside its diagnostic allowlist — the \
             sim must replay bit-for-bit, so time comes from `Sim::tick`. \
             Offending lines:\n{}",
            offenders.join("\n")
        );
    }
}
