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
//!   `f32::clamp` is two comparisons and two selects, so a signed zero or a
//!   NaN comes out the same on every CPU (measured, below). glam's vector
//!   `min`/`max`/`clamp`/`clamp_length*` under `scalar-math` are comparisons
//!   too and also stay.
//!
//! What does NOT stay on `std`: `f32::max`/`min` (and `f64`'s). LLVM lowers
//! them per CPU, and the CPUs disagree on a tie between `+0.0` and `-0.0` and
//! on which NaN comes back: x86's `maxss` returns the second operand on a tie,
//! AArch64's `fmaxnm` orders `-0.0` below `+0.0`. A weapon's spread decays
//! with `(spread - dt * rate).max(floor)`; on x86 that is `-0.0`, on Apple
//! Silicon `+0.0`, and the state hashes part within seconds
//! (analysis/plan/LAN_MULTIPLAYER.md §7). [`max`]/[`min`] and
//! [`FloatExt::fmax`]/[`FloatExt::fmin`] are the same choice written as plain
//! comparisons, giving bit-for-bit what x86 gives today, so no x86 hash moves.
//! Clippy refuses `f32::max`/`min` in `core` and `game` (`clippy.toml`).
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
//!
//! glam's vector code is the other half of the same contract: its default
//! SIMD paths add lanes in a different order on SSE2 and NEON, so the
//! workspace builds glam with `scalar-math` (engine/Cargo.toml,
//! analysis/plan/LAN_MULTIPLAYER.md §7). The assertion below refuses to
//! compile `core` without it: only the scalar `Quat` and `Vec4` drop the
//! 16-byte alignment their SIMD registers need.

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

/// Square a scalar with one deterministic multiplication.
#[inline]
pub fn square(x: f32) -> f32 {
    x * x
}

/// Larger of `a` and `b`, as x86-64 computes `f32::max` (`maxss` with NaN
/// handling), on every CPU: `a` when `a > b` or `b` is NaN, otherwise `b`.
/// So a tie (`+0.0` vs `-0.0`) gives `b`, a NaN operand gives the other one,
/// and two NaNs give `a`, its payload untouched. Measured on x86-64 against
/// `f32::max` in debug and release over every pair of signed zeros,
/// infinities, subnormals, extremes and NaN payloads (fmath tests).
#[inline]
pub fn max<F: FloatExt>(a: F, b: F) -> F {
    a.fmax(b)
}

/// Smaller of `a` and `b`, as x86-64 computes `f32::min`: `a` when `a < b`
/// or `b` is NaN, otherwise `b` (see [`max`]).
#[inline]
pub fn min<F: FloatExt>(a: F, b: F) -> F {
    a.fmin(b)
}

/// The NaN every hash feeds for any NaN: `f32::NAN`, so a NaN constant the
/// sim stores as a sentinel (a hover's missing leg, `Vec3::NAN`) hashes
/// exactly as before. A NaN made at run time is `0xffc0_0000` on x86 and
/// `0x7fc0_0000` on AArch64, so hashing raw bits would part the two over a
/// NaN both computed the same way.
pub const HASH_NAN_BITS: u32 = 0x7fc0_0000;

/// [`HASH_NAN_BITS`] for an `f64` (`f64::NAN`).
pub const HASH_NAN_BITS_F64: u64 = 0x7ff8_0000_0000_0000;

/// The sim's float operations that must not differ between CPUs, as methods
/// so a call site keeps its shape: `x.max(y)` becomes `x.fmax(y)`.
pub trait FloatExt: Copy {
    /// The float's bit pattern type.
    type Bits;
    /// [`max`]: x86's `f32::max` on every CPU.
    fn fmax(self, other: Self) -> Self;
    /// [`min`]: x86's `f32::min` on every CPU.
    fn fmin(self, other: Self) -> Self;
    /// `to_bits` for a hash or digest: any NaN as [`HASH_NAN_BITS`].
    fn hash_bits(self) -> Self::Bits;
}

macro_rules! float_ext {
    ($f:ty, $bits:ty, $nan:expr) => {
        impl FloatExt for $f {
            type Bits = $bits;
            #[inline]
            fn fmax(self, other: $f) -> $f {
                if self > other || other.is_nan() {
                    self
                } else {
                    other
                }
            }
            #[inline]
            fn fmin(self, other: $f) -> $f {
                if self < other || other.is_nan() {
                    self
                } else {
                    other
                }
            }
            #[inline]
            fn hash_bits(self) -> $bits {
                if self.is_nan() {
                    $nan
                } else {
                    self.to_bits()
                }
            }
        }
    };
}
float_ext!(f32, u32, HASH_NAN_BITS);
float_ext!(f64, u64, HASH_NAN_BITS_F64);

/// glam is built with `scalar-math`: every CPU runs the sim's vector maths
/// as the same plain f32 operations in the same order. Under the SIMD paths
/// (SSE2, NEON, wasm simd128, core-simd) `Quat` and `Vec4` are 16-byte
/// aligned; only the scalar types are aligned like an `f32`.
pub const GLAM_SCALAR_MATH: bool = std::mem::align_of::<glam::Quat>() == 4
    && std::mem::align_of::<glam::Vec4>() == 4
    && std::mem::align_of::<glam::Mat4>() == 4;

const _: () = assert!(
    GLAM_SCALAR_MATH,
    "glam must be built with its `scalar-math` feature (engine/Cargo.toml): its SIMD paths \
     round differently on x86 and ARM and LAN players desync (analysis/plan/LAN_MULTIPLAYER.md §7)"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// The referee for one math path on every CPU (LAN_MULTIPLAYER.md §7):
    /// `core` already refuses to compile without glam's `scalar-math`; this
    /// names the check in every test run, XDET's included.
    #[test]
    fn glam_runs_scalar_math_on_every_cpu() {
        use std::mem::align_of;
        assert_eq!(
            [
                align_of::<glam::Quat>(),
                align_of::<glam::Vec4>(),
                align_of::<glam::Mat4>()
            ],
            [4, 4, 4],
            "glam lost its `scalar-math` feature (engine/Cargo.toml): x86 and ARM players \
             would desync (analysis/plan/LAN_MULTIPLAYER.md §7)"
        );
    }

    /// Every float shape where CPUs can disagree on a max or min: both
    /// zeros, infinities, extremes, subnormals, equal pairs, and NaNs with
    /// either sign, quiet or signalling, with and without a payload.
    fn edges() -> Vec<f32> {
        let mut v = vec![
            0.0,
            -0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            1.0,
            -1.0,
            2.0,
            -2.0,
            0.5,
            1.5,
            f32::MIN_POSITIVE,
            -f32::MIN_POSITIVE,
            f32::from_bits(1),
            f32::from_bits(0x8000_0001),
            f32::MAX,
            f32::MIN,
        ];
        for bits in [
            0x7fc0_0000u32,
            0xffc0_0000,
            0x7f80_0001,
            0xff80_0001,
            0x7fc0_1234,
            0xffc0_4321,
            0x7fbf_ffff,
            0x7fff_ffff,
            0xffff_ffff,
        ] {
            v.push(f32::from_bits(bits));
        }
        v
    }

    /// `fmath::max`/`min` are x86's `f32::max`/`min`, bit for bit, over every
    /// pair of [`edges`] (625 pairs, NaN payloads included), so moving a call
    /// site onto them cannot move an x86 hash. On x86-64 only: on AArch64
    /// `f32::max` itself is the other answer (`fmaxnm`). The same model was
    /// measured on this box against `f32::max`/`min` compiled at opt-level 0
    /// and 3 (2026-11-13, rustc 1.97.1, LLVM 22): equal in every case.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn max_and_min_are_x86s_std_answers() {
        use std::hint::black_box;
        for a in edges() {
            for b in edges() {
                let (x, y) = (black_box(a), black_box(b));
                assert_eq!(max(a, b).to_bits(), x.max(y).to_bits(), "max({a:?}, {b:?})");
                assert_eq!(min(a, b).to_bits(), x.min(y).to_bits(), "min({a:?}, {b:?})");
                let (x, y) = (black_box(f64::from(a)), black_box(f64::from(b)));
                assert_eq!(
                    max(x, y).to_bits(),
                    x.max(y).to_bits(),
                    "f64 max({a:?}, {b:?})"
                );
                assert_eq!(
                    min(x, y).to_bits(),
                    x.min(y).to_bits(),
                    "f64 min({a:?}, {b:?})"
                );
            }
        }
    }

    /// The cross-CPU half (XDET runs it on x86 Linux, Windows and AArch64):
    /// `fmath::max`/`min`, `f32::clamp` and glam's vector `min`/`max`/`clamp`
    /// give the same bits for every pair of [`edges`] on every CPU. The
    /// digest was taken on x86-64; an ARM build that let LLVM pick `fmaxnm`
    /// again, or a glam that went back to `f32::max`, changes it.
    #[test]
    fn max_min_and_clamp_agree_on_every_cpu() {
        use glam::{Vec2, Vec3, Vec3A, Vec4};
        use std::hint::black_box;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: f32| {
            for b in v.to_bits().to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        let e = edges();
        for &a in &e {
            for &b in &e {
                let (x, y) = (black_box(a), black_box(b));
                eat(max(x, y));
                eat(min(x, y));
                eat(Vec2::splat(x).max(Vec2::splat(y)).x);
                eat(Vec2::splat(x).min(Vec2::splat(y)).x);
                eat(Vec3::splat(x).max(Vec3::splat(y)).y);
                eat(Vec3::splat(x).min(Vec3::splat(y)).y);
                eat(Vec3A::splat(x).max(Vec3A::splat(y)).z);
                eat(Vec3A::splat(x).min(Vec3A::splat(y)).z);
                eat(Vec4::splat(x).max(Vec4::splat(y)).w);
                eat(Vec4::splat(x).min(Vec4::splat(y)).w);
                for &c in &e {
                    // clamp's own contract: lo <= hi, neither NaN.
                    if y <= c {
                        let z = black_box(c);
                        eat(x.clamp(y, z));
                        eat(Vec3::splat(x).clamp(Vec3::splat(y), Vec3::splat(z)).x);
                    }
                }
            }
        }
        // The known answers behind the digest, written out.
        let bits = |v: f32| v.to_bits();
        assert_eq!(bits(max(0.0, -0.0)), bits(-0.0), "a tie gives the second");
        assert_eq!(bits(max(-0.0, 0.0)), bits(0.0));
        assert_eq!(bits(min(0.0, -0.0)), bits(-0.0));
        assert_eq!(bits(min(-0.0, 0.0)), bits(0.0));
        let (qnan, nnan) = (f32::from_bits(0x7fc0_1234), f32::from_bits(0xffc0_4321));
        assert_eq!(bits(max(qnan, 1.0)), bits(1.0), "a NaN gives the other");
        assert_eq!(bits(max(1.0, qnan)), bits(1.0));
        assert_eq!(
            bits(min(qnan, nnan)),
            0x7fc0_1234,
            "two NaNs give the first"
        );
        assert_eq!(bits(black_box(-0.0f32).clamp(0.0, 1.0)), bits(-0.0));
        assert_eq!(
            h, 0xc948_e8e3_6e50_0fd7,
            "max/min/clamp digest (taken on x86-64)"
        );
    }

    /// A NaN hashes as one pattern whichever CPU made it: x86's default NaN
    /// is `0xffc0_0000`, AArch64's `0x7fc0_0000`, and `f32::NAN` keeps its own.
    #[test]
    fn a_hashed_nan_has_one_pattern() {
        use std::hint::black_box;
        let zero = black_box(0.0f32);
        let made = zero / zero;
        assert!(made.is_nan());
        assert_eq!(f32::NAN.hash_bits(), f32::NAN.to_bits());
        assert_eq!(f64::NAN.hash_bits(), f64::NAN.to_bits());
        for nan in [made, f32::NAN, -f32::NAN, f32::from_bits(0x7f80_0001)] {
            assert_eq!(nan.hash_bits(), HASH_NAN_BITS, "{:#x}", nan.to_bits());
            assert_eq!(f64::from(nan).hash_bits(), HASH_NAN_BITS_F64);
        }
        for v in [0.0f32, -0.0, 1.5, f32::INFINITY, f32::from_bits(1)] {
            assert_eq!(v.hash_bits(), v.to_bits(), "{v:?}: only a NaN is rewritten");
        }
    }

    /// The referee for `f32::max`/`min` is clippy's type-aware
    /// `disallowed-methods` (a text scan cannot tell `a.max(b)` on a float
    /// from one on an integer). This keeps it in place: both crates' configs
    /// name all four methods, and only the crate roots' test-only
    /// `cfg_attr(test, allow(...))` may lift it in `src`.
    #[test]
    fn clippy_refuses_std_max_and_min() {
        let crates = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
        for krate in ["core", "game"] {
            let config = std::fs::read_to_string(crates.join(krate).join("clippy.toml"))
                .unwrap_or_else(|e| panic!("{krate}/clippy.toml: {e}"));
            for path in ["f32::max", "f32::min", "f64::max", "f64::min"] {
                assert!(
                    config.contains(&format!("{{ path = \"{path}\", reason = ")),
                    "{krate}/clippy.toml no longer disallows {path} (LAN_MULTIPLAYER.md §7)"
                );
            }
            let mut dirs = vec![crates.join(krate).join("src")];
            while let Some(d) = dirs.pop() {
                for e in std::fs::read_dir(&d).expect("source dir") {
                    let p = e.expect("dir entry").path();
                    if p.is_dir() {
                        dirs.push(p);
                        continue;
                    }
                    // This file quotes the attribute it looks for.
                    if p.extension().is_none_or(|e| e != "rs") || p.ends_with("src/fmath.rs") {
                        continue;
                    }
                    let text = std::fs::read_to_string(&p).expect("read source");
                    for line in text
                        .lines()
                        .filter(|l| l.contains("clippy::disallowed_methods"))
                    {
                        let root = p.ends_with("src/lib.rs") || p.ends_with("src/main.rs");
                        assert!(
                            root && line.trim()
                                == "#![cfg_attr(test, allow(clippy::disallowed_methods))]",
                            "{}: only test code may use f32::max/min: {line}",
                            p.display()
                        );
                    }
                }
            }
        }
    }

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
        ("render_blend.rs", "o.rot = prev.rot.slerp(o.rot, t);"),
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
            // `<name>_tests.rs` is the same convention for a named test module
            // (`#[cfg(test)] mod rider_health_tests;`).
            if name.ends_with("/tests.rs")
                || name.ends_with("_tests.rs")
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
            // SEAM in the code (card FGHT: `ai/brain/behaviours/fight.rs` `style`): one
            // braced statement, skipped to its closing brace. Anything else
            // would leave the referee blind without saying so — fail loudly.
            let cuts: Vec<usize> = text.match_indices("#[cfg(test)]").map(|(i, _)| i).collect();
            let code = match cuts.split_last() {
                Some((&last, seams)) => {
                    assert!(
                        text[last..].starts_with("#[cfg(test)]\nmod tests {")
                            // an inline module whose fixtures other tests share
                            || text[last..].starts_with("#[cfg(test)]\npub(crate) mod tests {")
                            || text[last..].trim_end() == "#[cfg(test)]\nmod tests;"
                            // the test module kept in a named file
                            || (text[last..].starts_with("#[cfg(test)]\n#[path = \"")
                                && text[last..].trim_end().ends_with("\"]\nmod tests;")
                                && text[last..].trim_end().lines().count() == 3)
                            // shares its fixtures with other test modules
                            || matches!(
                                text[last..].trim_end(),
                                "#[cfg(test)]\npub(crate) mod tests;" | "#[cfg(test)]\npub(super) mod tests;"
                            ),
                        "{name}: the last #[cfg(test)] is not the trailing `mod tests` \
                         — teach the referee to skip it"
                    );
                    let (mut code, mut at) = (String::new(), 0);
                    for &i in seams {
                        // A test-only `mod name;` declaration: skip to its `;`.
                        let semi = text[i..last].find(';');
                        let brace = text[i..last].find('{');
                        if let Some(s) = semi.filter(|&s| brace.is_none_or(|b| s < b)) {
                            code.push_str(&text[at..i]);
                            at = i + s + 1;
                            continue;
                        }
                        let open = i + brace.expect("a seam gates a block");
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
