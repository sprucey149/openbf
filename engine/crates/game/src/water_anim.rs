//! The water MOVES (queue item OC): Kamino's FFT ocean and the flat-water
//! bob. The sim separately queries the layer plus flat-water bob at fixed ticks.
//!
//! ## What is the PS2's (`analysis/re-water.md`, [V] unless marked)
//!
//! 1. **The bob.** Every flat-water map lifts its whole plane by
//!    `0.06 · sin(1.885 · t)` metres, recomputed every frame (§2, B1
//!    538346–538347), and the flat cull places each tile at `height + bob`
//!    (§ citation 6, 539266). It is one rigid lift of the whole surface, so it
//!    rides the water draws' model matrix and touches no vertex. BF2 gates it
//!    on `OscillationEnable` — 1 at the level reset, and BF2 FEL1 authors 0,
//!    so FEL1's water is still (`re-water-flags.md` §7 `[O]`); BF1 has no
//!    gate ([`oscillates`]).
//! 2. **The ocean.** A map that authors `OceanEnable` (both KAM1s and nothing
//!    else, §5) draws a Tessendorf height field instead of the flat plane: a
//!    16×16 grid (`PatchDivisions`) per 160 m WATR cell; the Phillips
//!    spectrum `FUN_00477b98` 539609–539636 — `A = PhillipsConstant`,
//!    `L = 0.1019 · V²` with `V = WindSpeed`, `cos²` against `WindDirection`,
//!    damping `l = L / 75`; the dispersion `ω = √(g·k)` (539602); and an
//!    UNNORMALISED inverse 2-D transform every frame (`FUN_004cea50` 539584)
//!    whose output is the mesh (539585) (§3, amended by §3a). The grid spans ONE
//!    cell, so every wet cell draws the same patch, at ONE resolution (4):
//!    the port rebuilds that patch every frame and instances it over the
//!    cells ([`WaterMotion`]).
//!
//! 3. **The amplitude is the raw transform's** (§3a [V] [O], queue OCB).
//!    `FUN_004cea50`'s `1/(N·M)` divide is gated on its fourth argument and
//!    the water passes zero (`00477b00: daddu a3,zero,zero`; BF2 `00562598`),
//!    nothing between the spectrum and the vertex scales `h̃0`, and the VU
//!    cannot: the packet is a unit cube `y = (h − min)/(max − min)` plus one
//!    per-tile world box ([D] — that VU program is unread; a PCSX2 GS dump of
//!    KAM1 would close it). So KAM1's sea is metres, not centimetres:
//!    **19.2 m peak to trough and 4.3 m rms** at the median of 2000 frames
//!    here, against the 7.5 cm / 1.6 cm the `1/(N·M)` gave. The disc's own
//!    foam test — a second difference over 3.0 — fires on 13 % of the points
//!    at this scale and on none at all at the old one, an independent check
//!    on the sign of the change (§3a).
//!
//! 4. **The ocean draws at ONE resolution** (§3b [V] [O], queue OCL). Its
//!    per-frame mesh writer builds the patch TWICE — stride 1 into
//!    `DAT_005649c0` and stride `LODDecimation` into `DAT_005649c4`
//!    (`FUN_0048ac90` 550796–550797; BF2 708144–708145) — but only the
//!    stride-1 handle ever reaches the DMA-call draw primitive
//!    `FUN_0039b488` (387615–387616). The decimated handle is read by
//!    exactly ONE instruction in each ELF (`0048baac` / `005760dc`), both
//!    inside the FLAT-water renderer, which the dispatcher (`FUN_0048b1e0`
//!    550906) runs only when `OceanEnable == 0`. So the disc builds a
//!    decimated ocean mesh every frame and throws it away, and its ocean is
//!    full `PatchDivisions` at every distance: **no mesh join, and no
//!    seam**. Its two PASSES are two RANGES, not two resolutions (§3b (c)) —
//!    they differ in the material record they bind and in the range they
//!    set, and both draw every tile with the same stride-1 mesh. The port
//!    therefore builds and draws ONE patch ([`WaterMotion::step`]), and does
//!    not copy the mesh the disc discards.
//!
//! 5. **The foam** (§3 cit. 5 [V]; §3a [V] [O], queue OCF). Each tile is
//!    drawn twice: the base `MainTexture` OPAQUE and Z-written (record
//!    `0x69c3c0`/`0x3d0` = {0x4100, TEST 0x50000, prim 0x18}, no ABE bit),
//!    then `FoamTexture` blended over it (`0x69c3e0` = {0x100144, TEST
//!    0x50000, prim 0x58}: ALPHA 0x44, `Cs·As + Cd·(1−As)`). The foam's `As`
//!    is its sheet's alpha × the VERTEX alpha (TCC=1, TFX=MODULATE —
//!    `re-water-alpha.md` §0 [O]), and that vertex alpha is a byte per mesh
//!    point the EE keeps (`DAT_00564958`) and packs as `(b + 1) >> 1` on the
//!    GS's 0..0x80 scale (`FUN_0048a6c8` 550727, 550747). The mesh writer
//!    `FUN_00477cd0` steps every byte each frame from that frame's RAW
//!    heights (539941–539985): **+6 where `(h − h₋₁) − (h₊₁ − h)` exceeds
//!    3.0 along either axis of the periodic grid (the ELF word `0x40400000`
//!    at `004782a4`), else −4**, saturating at 255 and at 0. So foam gathers
//!    on the sharp crests and fades off them ([`Foam`]); here it rides a
//!    COPY of the patch ([`foam_patch`]), because the base shares the vertex
//!    colour and must stay opaque. **The rate is [D]:** the water tick runs
//!    once per update (§2, B1 135276) and the PS2 delivers one update per two
//!    NTSC vblanks (`analysis/a5-tickrate.md` [V]) — the audit
//!    `core::TICK_HZ` rests on — so the foam steps on a 30 Hz grid of the
//!    WATER clock ([`FOAM_RATE`]), each step at its own `k / 30` s, and the
//!    foam at a given `t` is the same at 60 Hz, at 144 Hz and in a capture.
//!
//! ## What is ours — PROVISIONAL, each for a stated reason
//!
//! - **The random draws.** Tessendorf's `h̃0(k) = (ξr + iξi)·√(P(k)/2)` needs
//!   Gaussian numbers; the ELF's generator and seed are not in `analysis/`,
//!   so these come from [`OCEAN_SEED`] through splitmix64 and Box–Muller. The
//!   SPECTRUM is the disc's; the particular sea it draws is ours.
//! - **`g = 9.81`** in `ω = √(g·k)`: §3 names the formula, and `0.1019 ≈ 1/g`
//!   in `L`, but not the constant 539602 multiplies by.
//! - **The spectrum's three inputs are constants here** ([`Spectrum::KAM1`]),
//!   not manifest fields: the bake does not carry `PhillipsConstant`,
//!   `WindSpeed` or `WindDirection` (`manifest::ManifestWaterConfig`), and
//!   adding them is a [REBAKE]. Both KAM1s author the same three — read off
//!   both discs on 2026-09-10 with `lead_tmp/s17/re/fxwater.py`, inside
//!   `Effect("Water")`: `WindDirection(0.2, 1)`, `WindSpeed(25)`,
//!   `PhillipsConstant(1e-05)`. The pair is taken as world (x, z).
//! - **Vertical displacement only**, normals by central differences over the
//!   periodic grid. §3 names no horizontal ("choppy") term.
//! - **The inverse transform is our radix-2 FFT** (rows, then columns); the
//!   ELF's butterfly is not read (§3), and any correct one gives the same
//!   numbers up to rounding.
//! - **Where the patches are drawn** (card KSEA, `analysis/re-kamino.md` §2
//!   [O]). There is no far ocean mesh (item 4 above), so the port draws one
//!   patch over every cell that reaches inside [`WaterRanges::far`] —
//!   `viewer::water_look`'s `0.8 · FarSceneRange`, 640 m on both KAM1s (the
//!   flat pass's own fraction is 0.9; `FAR_WATER_FRACTION` /
//!   `OCEAN_FAR_FRACTION` in `viewer/look.rs`, both fractions re-read as ELF
//!   words by §3b (c)) — and NOTHING past them: the PS2 draws no water past
//!   its far pass, and the frame from the sea's edge to the horizon is its
//!   clear colour, black (PCSX2, §2.1: `FarSceneRange` 800 → 400 pulls the
//!   edge from the 160 m tile row ending at 681 m to the one ending at
//!   361 m). A cell is drawn WHOLE, as the PS2's are — the edge is a tile
//!   boundary between `far` and `far + cell` ([`cells_in_range`]). What is
//!   ours: the cell's distance is RADIAL from the eye (as every cull in the
//!   port, `viewer::far_cull`), where the PS2's far pass tests its frustum
//!   [D from frames; the tile test is unread] — the two agree down the view
//!   axis, and the radial edge does not move as the camera turns. Until KSEA
//!   the port drew its own still plane from there to the horizon.
//! - **The ocean does not bob.** Citation 6 is the FLAT cull. Whether the
//!   ocean path `FUN_0048b280` lifts its tiles by the same term, and what the
//!   ocean-mode `DAT_00564900` (the field's minimum height, §2 539732–539744)
//!   does to a tile's placement, is not in `analysis/`. The surface here is
//!   `LAYR.height + h(x, t)`.
//! - **The per-vertex ripple on flat water** is not drawn. The PS2 gives every
//!   flat-water vertex a random phase nibble and animates it on VU1 (§3 [V
//!   words, D use]); the microcode is unread (HOLD FOR FABLE 21).
//! - **The foam's start — a deliberate deviation.** The disc clears ONE byte
//!   of its `(N+1)·(M+1)`-byte map (`FUN_004266d8(map, 0, 1)` 539465; ELF
//!   `004776ec: addiu a2,zero,1`), so the rest starts as heap garbage and
//!   washes out under the −4 decay within 64 frames (§3a [V] [O]). [`Foam`]
//!   zeroes all of it, and its first step pre-runs [`FOAM_WARMUP`] steps
//!   ending at that `t` — as it does when the clock runs backwards or jumps
//!   further than that — so the first frame drawn shows the sea's settled
//!   foam, not a build-up the PS2's player never saw either (its first
//!   frames pass under the load screen). The count is ours.
//! - **The foam layer's UVs and scroll.** The disc authors `FoamTile(5,5)`
//!   against the base's `Tile(4,4)` (§5) and builds the foam's texture
//!   matrix from it unscaled, with the base's scroll offset (§3d): the foam
//!   copy shares the base's mesh UVs and scroll, and its sheet's `uv_scale`
//!   (`viewer::scene::foam_uv_scale`, 40 / 32 m) gives it its own tiling.
//! - **The ocean's vertex RGB** is `DAT_0069c420..422` halved on the disc
//!   (§3a (c)); what writes those bytes is unread, so both layers keep white
//!   and `viewer::water_look`'s tint does the colouring.
//!
//! ## The clock, and what the goldens show
//!
//! [`clock`] is the renderer's unwrapped water clock (`render::Renderer::water_time`, which the game
//! loop advances) unless `OPENBF_WATER_TIME=<seconds>` pins it, so a capture
//! of moving water is reproducible. Nothing moves until a caller steps the
//! water (`viewer::SceneGpu::animate_water`): `upload_scene` builds the still
//! water it always built. The gallery's sheets and the walkabout (the goldens)
//! step it once per eye at the clock's 0 s (`gallery`'s `step_water`, card
//! KSEA): a flat map shows its plane unmoved, because the bob is 0 at t = 0,
//! but an ocean map shows the t = 0 sea — a Tessendorf sea at t = 0 is
//! `Σ (h̃0(k) + conj h̃0(−k)) e^{ik·x}`, which is a sea, not a plane. The foam
//! is the same: it lives only in a stepped ocean's foam copy, and the still
//! foam draw carries none (alpha 0, `viewer::upload_scene`).
//!
//! An UNSTEPPED ocean is therefore a flat plane — the baked lattice, which
//! `upload_scene` cuts per fragment at the same [`WaterRanges::far`] (it has
//! no eye to pick cells by), so no path draws water past the far pass. The
//! first step takes that plane out of the draw list for good.

use std::f32::consts::TAU;
use std::ops::{Add, Mul, Range, Sub};
use std::sync::OnceLock;

use anyhow::Result;
use glam::{Mat4, Vec3};
use render::{DrawCmd, MeshData, MeshHandle, Renderer, Vertex};

use crate::assets::CpuMesh;

/// Height of the flat-water bob, metres (`re-water.md` §2 [V]).
pub const BOB_AMPLITUDE: f32 = 0.06;
/// Its rate, radians per second (§2 [V]) — a period of 2π/1.885 = 3.333 s.
pub const BOB_RATE: f32 = 1.885;

/// The flat-water plane's lift at `t` seconds.
pub fn bob(t: f32) -> f32 {
    BOB_AMPLITUDE * (BOB_RATE * t).sin()
}

/// Does this flat water bob: the manifest's `oscillation_enable`, which the
/// bake writes as the byte the PS2 reads (1 unless authored 0, module header
/// item 1). A manifest with no `config` bobs, as every water did before.
pub fn oscillates(cfg: Option<&manifest::ManifestWaterConfig>) -> bool {
    cfg.is_none_or(|c| c.oscillation_enable)
}

/// The flat-water plane's lift at `t` seconds: [`bob`] where it
/// [`oscillates`], else 0.
pub fn flat_lift(oscillates: bool, t: f32) -> f32 {
    if oscillates {
        bob(t)
    } else {
        0.0
    }
}

/// The water's clock: `OPENBF_WATER_TIME` when it is set, else
/// `renderer_time`.
pub fn clock(renderer_time: f64) -> f64 {
    pinned_time().unwrap_or(renderer_time)
}

fn pinned_time() -> Option<f64> {
    static PINNED: OnceLock<Option<f64>> = OnceLock::new();
    *PINNED.get_or_init(|| {
        let v = std::env::var("OPENBF_WATER_TIME").ok()?;
        match v.trim().parse::<f64>() {
            Ok(t) if t.is_finite() => Some(t),
            _ => {
                eprintln!("OPENBF_WATER_TIME={v:?} is not a number of seconds; ignored");
                None
            }
        }
    })
}

/// A complex number — as much of one as the ocean needs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };

    pub fn new(re: f32, im: f32) -> Complex {
        Complex { re, im }
    }

    /// `e^{i·a}`.
    pub fn cis(a: f32) -> Complex {
        Complex::new(a.cos(), a.sin())
    }

    pub fn conj(self) -> Complex {
        Complex::new(self.re, -self.im)
    }

    pub fn scale(self, s: f32) -> Complex {
        Complex::new(self.re * s, self.im * s)
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        Complex::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

/// §3's `L = 0.1019 · V²`: the largest wave a wind of `V` m/s raises.
const L_PER_V2: f32 = 0.1019;
/// §3's damping of the short waves, `l = L / 75`.
const DAMPING_DIVISOR: f32 = 75.0;
/// PROVISIONAL (module header): `g` in `ω = √(g·k)`.
pub const GRAVITY: f32 = 9.81;
/// PROVISIONAL (module header): our seed for the ocean's Gaussian draws.
pub const OCEAN_SEED: u64 = 0x5eed_0c3a;

/// The Phillips spectrum's authored inputs (`re-water.md` §3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spectrum {
    /// `PhillipsConstant`, the spectrum's `A`.
    pub a: f32,
    /// `WindSpeed`, metres per second.
    pub wind_speed: f32,
    /// `WindDirection` as world (x, z); normalised where it is used.
    pub wind_direction: [f32; 2],
}

impl Spectrum {
    /// Both KAM1s' `Effect("Water")` values (module header), re-read off both
    /// discs on 2026-09-17 by `re-water.md` §3a along with the 160 m `WATR`
    /// cell. The three are [V]; what the sea they raise still owes to us is
    /// [`OCEAN_SEED`] (the ELF's own generator and seed are unread) and the
    /// [D] dequantisation the amplitude rests on (module header).
    pub const KAM1: Spectrum = Spectrum {
        a: 1.0e-5,
        wind_speed: 25.0,
        wind_direction: [0.2, 1.0],
    };

    /// `L`, metres.
    pub fn largest_wave(&self) -> f32 {
        L_PER_V2 * self.wind_speed * self.wind_speed
    }

    /// `P(k) = A · e^{−1/(kL)²} / k⁴ · (k̂·ŵ)² · e^{−k²l²}`, and 0 at `k = 0`
    /// (and for a zero wind direction, which leaves `k̂·ŵ` undefined).
    pub fn phillips(&self, k: [f32; 2]) -> f32 {
        let k2 = k[0] * k[0] + k[1] * k[1];
        let [wx, wz] = self.wind_direction;
        let w = (wx * wx + wz * wz).sqrt();
        if k2 == 0.0 || w == 0.0 {
            return 0.0;
        }
        let l = self.largest_wave();
        let cos = (k[0] * wx + k[1] * wz) / (k2.sqrt() * w);
        let damp = l / DAMPING_DIVISOR;
        self.a * (-1.0 / (k2 * l * l)).exp() / (k2 * k2) * cos * cos * (-k2 * damp * damp).exp()
    }
}

/// One ocean patch: an `n × n` height field over a `size`-metre square,
/// periodic in both axes so that neighbouring cells meet. Row-major `[z][x]`
/// throughout; in the spectrum, array index `i` is the wave number `i − n/2`.
pub struct Ocean {
    n: usize,
    size: f32,
    /// `h̃0(k)`.
    h0: Vec<Complex>,
    /// `conj(h̃0(−k))`.
    h0_neg_conj: Vec<Complex>,
    /// `ω(k)`, radians per second.
    omega: Vec<f32>,
    /// `e^{i·2π·j/n}` for `j` in `0..n`.
    twiddle: Vec<Complex>,
}

impl Ocean {
    /// `n` must be a power of two, at least 2 (the FFT's radix, and the
    /// spectrum is centred on `n/2`).
    pub fn new(n: usize, size: f32, spectrum: &Spectrum, seed: u64) -> Ocean {
        assert!(
            n >= 2 && n.is_power_of_two(),
            "ocean grid {n} is not a power of two"
        );
        let half = (n / 2) as isize;
        let wave = |i: usize| TAU * (i as isize - half) as f32 / size;
        let mut gauss = Gauss(seed);
        let mut h0 = Vec::with_capacity(n * n);
        let mut omega = Vec::with_capacity(n * n);
        for iz in 0..n {
            for ix in 0..n {
                let k = [wave(ix), wave(iz)];
                let amp = (spectrum.phillips(k) / 2.0).sqrt();
                let (a, b) = gauss.pair();
                h0.push(Complex::new(a * amp, b * amp));
                omega.push((GRAVITY * k[0].hypot(k[1])).sqrt());
            }
        }
        // −k of index i is index n − i; the Nyquist index 0 is its own mirror.
        let neg = |i: usize| (n - i) % n;
        let h0_neg_conj = (0..n * n)
            .map(|j| h0[neg(j / n) * n + neg(j % n)].conj())
            .collect();
        let twiddle = (0..n)
            .map(|j| Complex::cis(TAU * j as f32 / n as f32))
            .collect();
        Ocean {
            n,
            size,
            h0,
            h0_neg_conj,
            omega,
            twiddle,
        }
    }

    pub fn n(&self) -> usize {
        self.n
    }

    pub fn size(&self) -> f32 {
        self.size
    }

    /// `h̃(k, t) = h̃0(k)·e^{iωt} + conj(h̃0(−k))·e^{−iωt}` — Hermitian by
    /// construction, which is what makes the field real.
    pub fn spectrum(&self, t: f64) -> Vec<Complex> {
        (0..self.h0.len())
            .map(|j| {
                // Reduce each mode's phase in f64 before converting the angle.
                // The modes have no common material-clock wrap period.
                let w = (f64::from(self.omega[j]) * t).rem_euclid(std::f64::consts::TAU) as f32;
                self.h0[j] * Complex::cis(w) + self.h0_neg_conj[j] * Complex::cis(-w)
            })
            .collect()
    }

    /// The inverse transform of a spectrum laid out as [`Ocean::spectrum`]
    /// lays it out: `Σ_k s(k)·e^{ik·x}` at the `n × n` sample points, with NO
    /// `1/n²` — the PS2's is unnormalised here (`re-water.md` §3a [V] [O]:
    /// `FUN_004cea50` divides by `N·M` only when its fourth argument is
    /// non-zero, and the water's call site passes zero — `00477b00: daddu
    /// a3,zero,zero` before the `jal`, BF2 `00562598` the same). That factor
    /// of `n²` IS the sea's scale: see [`Ocean::rms`]. The `−n/2` offset of
    /// the wave numbers folds into the plain transform as a `(−1)^(x+z)`
    /// factor, which is a sign and not a normalisation.
    pub fn inverse(&self, spec: &[Complex]) -> Vec<Complex> {
        let n = self.n;
        let mut a = spec.to_vec();
        for row in a.chunks_exact_mut(n) {
            ifft(row, &self.twiddle);
        }
        let mut col = vec![Complex::ZERO; n];
        for x in 0..n {
            for (z, c) in col.iter_mut().enumerate() {
                *c = a[z * n + x];
            }
            ifft(&mut col, &self.twiddle);
            for (z, c) in col.iter().enumerate() {
                a[z * n + x] = *c;
            }
        }
        for (i, c) in a.iter_mut().enumerate() {
            if !(i / n + i % n).is_multiple_of(2) {
                *c = c.scale(-1.0);
            }
        }
        a
    }

    /// The complex field at `t`; its imaginary part is rounding.
    pub fn field(&self, t: f64) -> Vec<Complex> {
        self.inverse(&self.spectrum(t))
    }

    /// The heights at `t`, metres, `[z][x]`.
    pub fn heights(&self, t: f64) -> Vec<f32> {
        self.field(t).iter().map(|c| c.re).collect()
    }

    /// Root-mean-square height at `t`, metres. On KAM1 this is 4.3 m at the
    /// median of 2000 frames (§3a and the module header): a real sea, which
    /// is the whole of what dropping [`Ocean::inverse`]'s `1/n²` changed.
    pub fn rms(&self, t: f64) -> f32 {
        let h = self.heights(t);
        (h.iter().map(|v| v * v).sum::<f32>() / h.len() as f32).sqrt()
    }

    /// A height no sample of this sea ever exceeds at any `t`, metres:
    /// `Σ_k (|h̃0(k)| + |h̃0(−k)|)`, the triangle inequality on
    /// [`Ocean::inverse`] — so it carries that transform's scale and drops
    /// its `1/n²` with it. Loose by construction (every mode at its crest at
    /// once): on KAM1 it is 37 m against a worst sample of 17.2 m over 2000
    /// frames of this sea (23.5 m over §3a's forty).
    pub fn bound(&self) -> f32 {
        let abs = |c: &Complex| c.re.hypot(c.im);
        self.h0.iter().map(abs).sum::<f32>() + self.h0_neg_conj.iter().map(abs).sum::<f32>()
    }

    /// The `(n+1)²` patch at `t` — [`Ocean::patch`] at full resolution.
    pub fn patch_vertices(
        &self,
        t: f64,
        uv0: [f32; 2],
        metres_per_repeat: [f32; 2],
    ) -> Vec<Vertex> {
        self.patch(&self.heights(t), 1, uv0, metres_per_repeat)
    }

    /// A patch of `heights` ([`Ocean::heights`]) sampled at every `every`-th
    /// point — `(n/every + 1)²` vertices over `[0, size]²`, y the height,
    /// normals by central differences on the FULL grid, UVs `uv0 + local /
    /// metres_per_repeat`. Row and column `n/every` repeat 0, so a patch tiles
    /// without a seam; a decimated patch's vertices are exactly the full
    /// patch's at the points they share.
    ///
    /// Nothing DRAWS a decimated patch: the renderer only ever asks for
    /// `every = 1` (module header item 4). `every` stays because the
    /// sampling property above is what makes it safe to say so — see the
    /// test `the_far_patch_is_the_near_patch_at_every_other_point`.
    pub fn patch(
        &self,
        heights: &[f32],
        every: usize,
        uv0: [f32; 2],
        metres_per_repeat: [f32; 2],
    ) -> Vec<Vertex> {
        let n = self.n;
        let m = n / every;
        let at = |x: usize, z: usize| heights[(z % n) * n + (x % n)];
        let step = self.size / n as f32;
        let mut out = Vec::with_capacity((m + 1) * (m + 1));
        for gz in 0..=m {
            for gx in 0..=m {
                let (x, z) = (gx * every, gz * every);
                let dx = (at(x + 1, z) - at(x + n - 1, z)) / (2.0 * step);
                let dz = (at(x, z + 1) - at(x, z + n - 1)) / (2.0 * step);
                let (lx, lz) = (x as f32 * step, z as f32 * step);
                out.push(Vertex {
                    position: [lx, at(x, z), lz],
                    normal: Vec3::new(-dx, 1.0, -dz).normalize().to_array(),
                    uv: [
                        uv0[0] + lx / metres_per_repeat[0],
                        uv0[1] + lz / metres_per_repeat[1],
                    ],
                    color: [255; 4],
                });
            }
        }
        out
    }

    /// The triangles of [`Ocean::patch`] at `every`, two per quad in the
    /// bake's own order (`bake::water_glb`: `[a, c, b, b, c, d]`).
    pub fn patch_indices(&self, every: usize) -> Vec<u32> {
        let m = self.n / every;
        let stride = m + 1;
        let mut out = Vec::with_capacity(m * m * 6);
        for z in 0..m {
            for x in 0..m {
                let a = (z * stride + x) as u32;
                let (b, c) = (a + 1, a + stride as u32);
                out.extend_from_slice(&[a, c, b, b, c, c + 1]);
            }
        }
        out
    }
}

/// What a foam byte gains in a step where the crest test fires, and loses in
/// one where it does not (`FUN_00477cd0` 539955–539971, module header item 5).
pub const FOAM_GAIN: u8 = 6;
pub const FOAM_DECAY: u8 = 4;
/// The crest test's threshold, metres: `(h − h₋₁) − (h₊₁ − h)` over it along
/// either axis (the ELF word `0x40400000` = 3.0 at `004782a4`).
pub const FOAM_CREST: f32 = 3.0;
/// Foam steps per second of the water clock: the disc steps once per frame
/// and renders one frame per two NTSC vblanks — the audit `core::TICK_HZ`
/// rests on (module header item 5, [D]).
pub const FOAM_RATE: f32 = core::TICK_HZ as f32;
/// PROVISIONAL (module header): how many steps a cold start runs before the
/// first frame it draws — 6.4 s of sea, the fewest of those tried (96, 128,
/// 160, 192) whose foam matched a 2000-step run's byte for byte at all 1,200
/// times sampled on KAM1 (every 5th step of 200 s; 128 missed 69 of them).
pub const FOAM_WARMUP: u32 = 192;

/// The ocean's foam: one byte per point of the `(n+1)²` patch, in
/// [`Ocean::patch`]'s own order, stepped by the disc's rule (module header
/// item 5). Row and column `n` are row and column 0 again, as on the disc,
/// whose map is `(N+1)·(M+1)` bytes read through the same periodic indices.
#[derive(Debug, Clone, PartialEq)]
pub struct Foam {
    n: usize,
    bytes: Vec<u8>,
    /// The last step taken, in steps of `1 / FOAM_RATE` s; `None` before the
    /// first.
    last: Option<i64>,
}

impl Foam {
    /// Zeroed — ALL of it, where the disc clears one byte (module header).
    pub fn new(n: usize) -> Foam {
        Foam {
            n,
            bytes: vec![0; (n + 1) * (n + 1)],
            last: None,
        }
    }

    /// The bytes, `(n+1)²`, row-major `[z][x]`.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// One step of the disc's rule over `heights` ([`Ocean::heights`]: `n²`
    /// RAW heights, `[z][x]`). The differences are grouped as the disc groups
    /// them, `(h − left) − (right − h)`, and the test is strict.
    pub fn step(&mut self, heights: &[f32]) {
        let n = self.n;
        assert_eq!(
            heights.len(),
            n * n,
            "foam of {n}² points over {} heights",
            heights.len()
        );
        let at = |x: usize, z: usize| heights[(z % n) * n + (x % n)];
        for gz in 0..=n {
            for gx in 0..=n {
                let h = at(gx, gz);
                let along_x = (h - at(gx + n - 1, gz)) - (at(gx + 1, gz) - h);
                let along_z = (h - at(gx, gz + n - 1)) - (at(gx, gz + 1) - h);
                let b = &mut self.bytes[gz * (n + 1) + gx];
                *b = if along_x > FOAM_CREST || along_z > FOAM_CREST {
                    b.saturating_add(FOAM_GAIN)
                } else {
                    b.saturating_sub(FOAM_DECAY)
                };
            }
        }
    }

    /// Step `ocean`'s foam up to the water clock's `t`: every step `k` with
    /// `k / FOAM_RATE ≤ t` not yet taken, each over the heights at its own
    /// `k / FOAM_RATE`, so the result does not depend on how often this is
    /// called. The first call — and any call whose clock ran backwards or
    /// jumped more than [`FOAM_WARMUP`] steps — clears the map and runs the
    /// `FOAM_WARMUP` steps ending at `t` (module header). Returns the steps
    /// taken; a non-finite `t` takes none.
    pub fn advance(&mut self, ocean: &Ocean, t: f64) -> u32 {
        if !t.is_finite() {
            return 0;
        }
        let k = (t * f64::from(FOAM_RATE)).floor() as i64;
        let warmup = i64::from(FOAM_WARMUP);
        let from = match self.last {
            Some(last) if last <= k && k.saturating_sub(last) <= warmup => last,
            _ => {
                self.bytes.fill(0);
                k.saturating_sub(warmup)
            }
        };
        for s in from + 1..=k {
            self.step(&ocean.heights(s as f64 / f64::from(FOAM_RATE)));
        }
        self.last = Some(k);
        (k - from) as u32
    }
}

/// The vertex alpha a foam byte gives the foam layer: the disc packs
/// `(b + 1) >> 1`, the GS's 0..0x80 scale on which 0x80 is 1.0
/// (`FUN_0048a6c8` 550727, 550747), and the port's 0..255 is that × 255/128,
/// rounded — so 0 stays 0 and 255 stays 255.
pub fn foam_alpha(byte: u8) -> u8 {
    let gs = (u32::from(byte) + 1) >> 1;
    ((gs * 255 + 64) / 128) as u8
}

/// The foam layer's copy of a full-resolution patch ([`Ocean::patch`] at
/// `every` = 1): the same vertices with each one's alpha from its foam byte.
/// A COPY rather than a second build, so the two layers cannot part.
pub fn foam_patch(patch: &[Vertex], foam: &Foam) -> Vec<Vertex> {
    assert_eq!(
        patch.len(),
        foam.bytes.len(),
        "foam and patch disagree on the grid"
    );
    patch
        .iter()
        .zip(&foam.bytes)
        .map(|(v, &b)| {
            let mut v = *v;
            v.color[3] = foam_alpha(b);
            v
        })
        .collect()
}

/// In-place inverse DFT of a power-of-two `a` (no `1/n`): `A[x] = Σ_i a[i]
/// e^{+i·2π·i·x/n}`, radix-2, decimation in time. `tw[j] = e^{i·2π·j/n}`.
fn ifft(a: &mut [Complex], tw: &[Complex]) {
    let n = a.len();
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            a.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let (half, stride) = (len / 2, n / len);
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let u = a[start + k];
                let v = a[start + k + half] * tw[k * stride];
                a[start + k] = u + v;
                a[start + k + half] = u - v;
            }
        }
        len *= 2;
    }
}

/// The WATR cells a baked water mesh covers, read back off its lattice:
/// `bake::water_glb` emits two triangles per wet cell, `[a, c, b, b, c, d]`
/// over the cell's four corners.
#[derive(Debug, Clone, PartialEq)]
pub struct Lattice {
    /// Metres per cell side.
    pub cell: f32,
    /// The plane's height, `LAYR.height`.
    pub height: f32,
    /// Each wet cell's min-x, min-z corner, in the bake's order.
    pub origins: Vec<[f32; 2]>,
}

/// `None` for anything that is not exactly that lattice — a `CPAT` patch's
/// own triangles, cells of more than one size, a surface off the plane — in
/// which case the caller keeps the still mesh.
pub fn lattice_cells(positions: &[[f32; 3]], indices: &[u32]) -> Option<Lattice> {
    if indices.is_empty() || !indices.len().is_multiple_of(6) {
        return None;
    }
    let height = positions.get(indices[0] as usize)?[1];
    let mut cell: Option<f32> = None;
    let mut origins = Vec::with_capacity(indices.len() / 6);
    for quad in indices.as_chunks::<6>().0 {
        let p: Vec<[f32; 3]> = quad
            .iter()
            .map(|&i| positions.get(i as usize).copied())
            .collect::<Option<_>>()?;
        let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for v in &p {
            (x0, x1) = (x0.min(v[0]), x1.max(v[0]));
            (z0, z1) = (z0.min(v[2]), z1.max(v[2]));
        }
        let side = x1 - x0;
        let eps = 1e-4 * side.max(1.0);
        if side <= 0.0 || (z1 - z0 - side).abs() > eps {
            return None;
        }
        let near = |a: f32, b: f32| (a - b).abs() <= eps;
        let mut corners = [false; 4];
        for v in &p {
            if (v[1] - height).abs() > 1e-3 {
                return None;
            }
            let cx = match (near(v[0], x0), near(v[0], x1)) {
                (true, _) => 0,
                (_, true) => 1,
                _ => return None,
            };
            let cz = match (near(v[2], z0), near(v[2], z1)) {
                (true, _) => 0,
                (_, true) => 2,
                _ => return None,
            };
            corners[cx + cz] = true;
        }
        if corners.contains(&false) {
            return None;
        }
        match cell {
            None => cell = Some(side),
            Some(c) if (c - side).abs() <= eps => {}
            Some(_) => return None,
        }
        origins.push([x0, z0]);
    }
    Some(Lattice {
        cell: cell?,
        height,
        origins,
    })
}

/// The UV every cell's corner shares, when one does: the bake's UV is
/// `world / metres_per_repeat`, so one instanced patch paints every cell
/// exactly as the still mesh did only if the cells start at the same point
/// of the texture. True whenever `cell / metres_per_repeat` is whole (it is 4
/// on both KAM1s — `Tile(4, 4)` over `INFO5 = 1`).
pub fn shared_uv_origin(origins: &[[f32; 2]], metres_per_repeat: [f32; 2]) -> Option<[f32; 2]> {
    let uv = |o: [f32; 2]| [o[0] / metres_per_repeat[0], o[1] / metres_per_repeat[1]];
    let first = uv(*origins.first()?);
    let uv0 = [first[0].rem_euclid(1.0), first[1].rem_euclid(1.0)];
    let whole = |a: f32| (a - a.round()).abs() <= 1e-3;
    origins
        .iter()
        .all(|&o| {
            let u = uv(o);
            whole(u[0] - uv0[0]) && whole(u[1] - uv0[1])
        })
        .then_some(uv0)
}

/// Where an ocean's patches are drawn, metres from the eye (module header):
/// out to `far`, at full `PatchDivisions` the whole way — the disc's two
/// passes are two RANGES over one mesh, not two resolutions (§3b (c)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterRanges {
    /// `NearSceneRange.b` — the disc's boundary between the near pass and
    /// the far one. NOTHING in the port reads it today: the passes differ on
    /// the PS2 in the material record they bind (`0x69c3c0` near, `0x69c3d0`
    /// far, both plus the foam record) and the port draws one pass with one
    /// material, so the split has nothing to select yet. It is kept here,
    /// not deleted, because it is the input the two-pass work needs and the
    /// value is already computed; it stopped picking a MESH when §3b showed
    /// there is only one.
    pub near: f32,
    /// `viewer::WaterLook::far` — past it the PS2 draws no water.
    pub far: f32,
}

pub fn flat_display_gain(m: &manifest::Manifest) -> f32 {
    if m.source_game == "bf2"
        && m.hdr.is_some()
        && m.water
            .as_ref()
            .and_then(|w| w.config.as_ref())
            .is_some_and(|c| !c.ocean)
    {
        255.0 / 128.0
    } else {
        1.0
    }
}

pub fn flat_far_diffuse(
    mesh: &CpuMesh,
    water: &manifest::ManifestWater,
    lighting: &manifest::ManifestLighting,
) -> Option<render::WaterDiffuse> {
    let cfg = water.config.as_ref()?;
    if cfg.ocean
        || cfg.disable_low_res
        || !water.vertex_diffuse
        || water.patches != 0
        || lighting
            .far_scene_range
            .is_some_and(|f| !f.is_finite() || f <= 0.0)
    {
        return None;
    }
    let wet = water.wet.as_ref()?;
    let [near, far] = lighting.near_scene_range.map(|r| [r[1], r[2]])?;
    let patch = cfg.patch_divisions?;
    let decimation = cfg.lod_decimation?.min(patch[0]).min(patch[1]).max(1.0);
    if !wet.cell_size.is_finite()
        || wet.cell_size <= 0.0
        || !near.is_finite()
        || !far.is_finite()
        || near < 0.0
        || far <= near
    {
        return None;
    }
    let first = *mesh.positions.first()?;
    let stride = mesh.positions.iter().position(|p| p[2] != first[2])?;
    if stride < 2 {
        return None;
    }
    let nx = stride - 1;
    let dz = mesh.positions.get(stride)?[2] - first[2];
    if !dz.is_finite() || dz <= 0.0 {
        return None;
    }
    let nz = (wet.cell_size / dz).round() as usize;
    if nx > 64 || !(1..=64).contains(&nz) {
        return None;
    }
    let divisions = [nx as f32 / decimation, nz as f32 / decimation];
    if divisions
        .iter()
        .any(|d| !d.is_finite() || *d < 1.0 || d.fract() != 0.0)
    {
        return None;
    }
    let dims = divisions.map(|d| d as usize);
    if dims[0] * dims[1] > 64 || !nx.is_multiple_of(dims[0]) || !nz.is_multiple_of(dims[1]) {
        return None;
    }
    let mut colors = [[0u32; 4]; 16];
    for z in 0..dims[1] {
        for x in 0..dims[0] {
            let ordinal = x * dims[1] + z;
            let source = (ordinal % nz) * stride + ordinal / nz;
            let color = *mesh.colors.get(source)?;
            let index = z * dims[0] + x;
            colors[index / 4][index % 4] = u32::from_le_bytes(color);
        }
    }
    Some(render::WaterDiffuse {
        origin: [first[0], first[2]],
        step: [wet.cell_size / divisions[0], wet.cell_size / divisions[1]],
        dims: dims.map(|d| d as u32),
        range: [near, far],
        colors,
    })
}

/// A map's water, able to move. Built by `viewer::upload_scene` from the
/// water draws it has just pushed, and stepped by
/// `viewer::SceneGpu::animate_water`.
pub struct WaterMotion {
    /// The water's draws in `SceneGpu::draws`.
    range: Range<usize>,
    kind: Kind,
}

enum Kind {
    /// An ocean this module could not lay out; it keeps its still mesh.
    Still,
    /// Flat water: the bob (where it [`oscillates`]) on each draw's model
    /// matrix (`base` is the matrix `upload_scene` gave it).
    Flat {
        base: Vec<Mat4>,
        bobs: bool,
    },
    Ocean(Box<OceanDraw>),
}

/// The ocean's patch and the cells it is instanced over.
struct OceanDraw {
    ocean: Ocean,
    lattice: Lattice,
    ranges: WaterRanges,
    uv0: [f32; 2],
    metres_per_repeat: [f32; 2],
    /// The one patch, uploaded at the first step. ONE resolution is the
    /// guard on the seam OCL closed: with no second mesh there is nothing to
    /// pick between, so a crack at a range boundary is not representable
    /// (module header item 4). [`OceanDraw::foam_mesh`] does not reopen it:
    /// it is written every frame as a [`foam_patch`] COPY of these vertices.
    mesh: Option<MeshHandle>,
    /// The foam layer's copy of the patch, carrying [`OceanDraw::foam`] in
    /// its alpha — its own buffer because the base shares the vertex colour
    /// and must stay opaque (module header item 5). Uploaded after `mesh`,
    /// and the water pass orders its runs by mesh handle
    /// (`render::frame::draw_key`), so the foam composites over a base already
    /// drawn, as the disc draws it.
    foam_mesh: Option<MeshHandle>,
    foam: Foam,
    /// The still water draws as `upload_scene` pushed them — one patch draw
    /// per template per cell. The first step takes them out of the draw
    /// list; they stay here as the patches' material.
    templates: Vec<DrawCmd>,
    /// Which templates are the foam layer: `upload_scene` pushes each
    /// submesh's base draw first and gives the foam its own texture handle,
    /// so a template whose texture is not the first one's is foam.
    is_foam: Vec<bool>,
}

impl WaterMotion {
    /// `range` is where `draws` holds the water, `mesh` the baked water mesh,
    /// `cfg` its `fx__` config and `ranges` where an ocean's patches reach.
    pub fn new(
        range: Range<usize>,
        draws: &[DrawCmd],
        mesh: &CpuMesh,
        cfg: Option<&manifest::ManifestWaterConfig>,
        ranges: WaterRanges,
    ) -> WaterMotion {
        let kind = match cfg {
            Some(c) if c.ocean => match OceanDraw::new(mesh, c, ranges, &draws[range.clone()]) {
                Some(o) => Kind::Ocean(Box::new(o)),
                None => Kind::Still,
            },
            _ => Kind::Flat {
                base: draws[range.clone()].iter().map(|d| d.model).collect(),
                bobs: oscillates(cfg),
            },
        };
        WaterMotion { range, kind }
    }

    /// Move the water to `t` seconds, seen from `eye`. On an ocean the first
    /// step uploads the patch and takes the still plane out of the draw list;
    /// every step re-uploads the patch, advances the foam, and re-picks the
    /// cells that reach inside the far range — the ocean's only draws.
    pub fn step(
        &mut self,
        r: &mut Renderer,
        draws: &mut Vec<DrawCmd>,
        t: f64,
        eye: Vec3,
    ) -> Result<()> {
        match &mut self.kind {
            Kind::Still => {}
            Kind::Flat { base, bobs } => {
                let lift = Mat4::from_translation(Vec3::new(0.0, flat_lift(*bobs, t as f32), 0.0));
                for (d, m) in draws[self.range.clone()].iter_mut().zip(base.iter()) {
                    d.model = lift * *m;
                }
            }
            Kind::Ocean(o) => {
                let h = o.ocean.heights(t);
                // ONE patch, at full `PatchDivisions`, for every distance —
                // the disc builds a `LODDecimation` one too and never issues
                // it (module header item 4).
                let verts = o.ocean.patch(&h, 1, o.uv0, o.metres_per_repeat);
                let mesh = match o.mesh {
                    Some(m) => {
                        r.update_vertices(m, &verts)?;
                        m
                    }
                    None => {
                        let m = upload_dynamic(r, &verts, &o.ocean.patch_indices(1))?;
                        o.mesh = Some(m);
                        m
                    }
                };
                // The foam layer, when the viewer drew one (module header
                // item 5): advanced on its own 30 Hz clock, written after the
                // base so its buffer sorts after it.
                let foam_mesh = match o.is_foam.contains(&true) {
                    false => mesh,
                    true => {
                        o.foam.advance(&o.ocean, t);
                        let fv = foam_patch(&verts, &o.foam);
                        match o.foam_mesh {
                            Some(m) => {
                                r.update_vertices(m, &fv)?;
                                m
                            }
                            None => {
                                let m = upload_dynamic(r, &fv, &o.ocean.patch_indices(1))?;
                                o.foam_mesh = Some(m);
                                m
                            }
                        }
                    }
                };
                // The patches REPLACE whatever the range held — the still
                // plane on the first step, the last frame's patches after:
                // the PS2 draws no water past its far pass (module header).
                // A patch is drawn whole, so it drops the per-fragment cull
                // `upload_scene` gave the still plane.
                let cells = cell_draws(
                    cells_in_range(&o.lattice, eye, o.ranges.far),
                    o.lattice.height,
                    &o.templates,
                    &o.is_foam,
                    [mesh, foam_mesh],
                    |mesh, draw| DrawCmd { mesh, ..draw },
                );
                let start = self.range.start;
                draws.splice(self.range.clone(), cells.iter().copied());
                self.range = start..start + cells.len();
            }
        }
        Ok(())
    }

    /// True on an ocean map whose patch this module laid out.
    pub fn is_ocean(&self) -> bool {
        matches!(self.kind, Kind::Ocean(_))
    }

    /// Where the water's draws are in `SceneGpu::draws` right now — on a
    /// stepped ocean, this frame's patches and nothing else.
    pub fn draw_range(&self) -> Range<usize> {
        self.range.clone()
    }
}

/// Route every selected cell's base and foam templates to their own patch.
/// The mesh type and draw builder let CPU callers use distinct symbolic handles
/// without uploading a GPU mesh. Production supplies the uploaded mesh handles.
pub fn cell_draws<M: Copy, D>(
    cells: impl IntoIterator<Item = [f32; 2]>,
    height: f32,
    templates: &[DrawCmd],
    is_foam: &[bool],
    meshes: [M; 2],
    draw: impl Fn(M, DrawCmd) -> D,
) -> Vec<D> {
    assert_eq!(templates.len(), is_foam.len());
    let mut draws = Vec::new();
    for c in cells {
        let at = Mat4::from_translation(Vec3::new(c[0], height, c[1]));
        for (tpl, &foam) in templates.iter().zip(is_foam) {
            draws.push(draw(
                meshes[usize::from(foam)],
                DrawCmd {
                    model: at * tpl.model,
                    ..*tpl
                }
                .with_cull(None),
            ));
        }
    }
    draws
}

/// The wet cells an ocean draws from `eye`: every cell of `lattice` whose
/// nearest point is within `far` metres, WHOLE — so the sea ends on a cell
/// boundary between `far` and `far + cell`, as the PS2's does
/// (`analysis/re-kamino.md` §2.1; module header). Min corners, in the
/// lattice's order.
pub fn cells_in_range(
    lattice: &Lattice,
    eye: Vec3,
    far: f32,
) -> impl Iterator<Item = [f32; 2]> + '_ {
    lattice
        .origins
        .iter()
        .copied()
        .filter(move |&c| distance_to_cell(eye, c, lattice.cell, lattice.height) <= far)
}

/// Metres from `eye` to the nearest point of the cell whose min corner is
/// `origin`, on the plane at `y` — a radial distance, as the port's far cull
/// takes it (`viewer::far_cull`), where the PS2 clips by frustum planes.
fn distance_to_cell(eye: Vec3, origin: [f32; 2], cell: f32, y: f32) -> f32 {
    let dx = (origin[0] - eye.x).max(eye.x - (origin[0] + cell)).max(0.0);
    let dz = (origin[1] - eye.z).max(eye.z - (origin[1] + cell)).max(0.0);
    Vec3::new(dx, eye.y - y, dz).length()
}

impl OceanDraw {
    fn new(
        mesh: &CpuMesh,
        c: &manifest::ManifestWaterConfig,
        ranges: WaterRanges,
        templates: &[DrawCmd],
    ) -> Option<OceanDraw> {
        let fail = |why: &str| {
            eprintln!("water: the ocean stays still — {why}");
            None
        };
        let Some(lattice) = lattice_cells(&mesh.positions, &mesh.indices) else {
            return fail("its baked mesh is not a plain lattice of cells");
        };
        let Some(metres_per_repeat) = c.metres_per_repeat else {
            return fail("the bake recorded no metres_per_repeat");
        };
        let Some(uv0) = shared_uv_origin(&lattice.origins, metres_per_repeat) else {
            return fail("its cells do not start at one point of the texture");
        };
        // `PatchDivisions` (§5: 16, 16 on both KAM1s); `INFO5` = 1 there, so
        // the grid is not multiplied (`bake::water_glb`'s header).
        let whole = |v: f32| v >= 1.0 && v.fract() == 0.0;
        let n = match c.patch_divisions {
            Some([a, b]) if a == b && whole(a) && (a as usize).is_power_of_two() && a >= 2.0 => {
                a as usize
            }
            other => {
                return fail(&format!(
                    "PatchDivisions {other:?} is not a power-of-two square"
                ))
            }
        };
        // `c.lod_decimation` is NOT read here any more, and there is no
        // validation of it left to fail on: it used to reject a value that
        // did not divide `n`, which mattered only because the far patch was
        // built at that stride. §3b read the ocean path and the decimated
        // mesh is never issued, so the field gates nothing on this side and
        // an unusable value can no longer break anything. The bake still
        // carries it (`bake::water_glb` → `manifest::ManifestWaterConfig`)
        // because it IS the disc's authored key and the flat-water far pass
        // is the place that will want it.
        let base_texture = templates.first().and_then(|d| d.texture);
        Some(OceanDraw {
            ocean: Ocean::new(n, lattice.cell, &Spectrum::KAM1, OCEAN_SEED),
            lattice,
            ranges,
            uv0,
            metres_per_repeat,
            mesh: None,
            foam_mesh: None,
            foam: Foam::new(n),
            templates: templates.to_vec(),
            is_foam: templates
                .iter()
                .map(|d| d.texture != base_texture)
                .collect(),
        })
    }
}

/// `Renderer::add_dynamic_mesh` takes the SoA `MeshData`; the vertices here
/// are built interleaved.
fn upload_dynamic(r: &mut Renderer, verts: &[Vertex], indices: &[u32]) -> Result<MeshHandle> {
    let positions: Vec<[f32; 3]> = verts.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = verts.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = verts.iter().map(|v| v.uv).collect();
    let colors: Vec<[u8; 4]> = verts.iter().map(|v| v.color).collect();
    r.add_dynamic_mesh(&MeshData {
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        colors: Some(&colors),
        indices,
    })
    .map_err(|e| anyhow::anyhow!("water mesh: {e}"))
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Standard normal pairs: splitmix64 uniforms through Box–Muller.
struct Gauss(u64);

impl Gauss {
    /// Uniform on the open interval (0, 1).
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(1);
        ((splitmix64(self.0) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn pair(&mut self) -> (f32, f32) {
        let (u1, u2) = (self.uniform(), self.uniform());
        let r = (-2.0 * u1.ln()).sqrt();
        let a = std::f64::consts::TAU * u2;
        ((r * a.cos()) as f32, (r * a.sin()) as f32)
    }
}
