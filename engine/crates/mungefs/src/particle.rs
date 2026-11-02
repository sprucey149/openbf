//! `fx__` particle effects: the `ParticleEmitter` trees (card FXP1,
//! `analysis/re-fx.md` §1–§4, [O]).
//!
//! A particle effect is one `fx__` chunk: a `NAME` child holding
//! `fnv1a_lowercase(effect name)` (the PS2's "fnv20", FNV-1a over
//! `byte | 0x20`, so `_` hashes as 0x7f — [`crate::fnv1a_lowercase`] is that
//! hash, and an ODF value names an effect by the same one) followed by a
//! munged config tree ([`crate::config`]) whose root records are
//! `ParticleEmitter("<label>")`. The same tag also carries the map
//! environment effects (`Effect("Water" | "Wind" | …)`, `Shield`, …): a chunk
//! with no `ParticleEmitter` root is not a particle effect and
//! [`Effect::parse`] returns `None` for it (the water half is
//! [`crate::fx`]; the weather effects are cards WTHR / WTHL).
//!
//! ## What is read, and how (re-fx §2–§3, the B1 parsers at B1 246942–248420)
//!
//! Every key of the parsers' switches is decoded; the 63 the discs use are
//! all named in [`key`] (7 by the hash alone — Spawner shape type 4 and two of
//! its X/Y/Z families, 3 emitters on BF1 HOT1). A key the parsers do not know
//! lands in [`Emitter::unknown`] (none on either disc — `tests/suite/fx_lvl.rs`).
//!
//! - A **range** `Key(a, b)` is `{base: min(a,b)·k, span: |b−a|·k}` (B1
//!   `FUN_002ba1f0`, B2 `FUN_0037b7f0`); a one-argument record is `{a·k, 0}`.
//!   B1 keeps a third word, `random = a≠b`, which is exactly `span ≠ 0`
//!   ([`Range::random`]). k = 1/255 (the f32 0x3b808081) for colour keys and
//!   1 for everything else — the ONLY unit conversion at parse (§3).
//! - **Indexed** keys `Key(i, a, b)` (the Spawner's Size / colours /
//!   rotations, the Transformer's Size / Rotation / Color tracks) take their
//!   pair from slot 1. The PS2 writes index i to `field + i·stride`, which for
//!   i ≠ 0 lands in the NEXT field; every index on both discs is 0 (§1), so
//!   only i = 0 is stored and any other goes to `unknown`.
//! - **Defaults** are the constructors' (B1 `FUN_002b1b38` / `FUN_002b6b30` /
//!   `FUN_002b6108` / `FUN_002b92b0`, B2 `FUN_003742a0` / `FUN_00378890` /
//!   `FUN_00377b28` — the two games' values are identical, read 2026-09-30).
//! - **Continuous emitters** (§3, B1 247225–247230, B2 376077–376082):
//!   `MaxParticles(-1, -1)` with `BurstDelay(0, 0)` becomes BurstDelay
//!   `{0.1, 0.1}` — a burst every 0.1–0.2 s. Applied to every emitter,
//!   nested ones included (the rule runs at the end of each recursive parse).
//! - **BF2's split-screen budget is NOT applied.** BF2 halves MaxParticles
//!   when two or more local viewports are open and multiplies BurstDelay by
//!   the viewport count (§3, §8); that is a PS2 fill-rate budget, and this
//!   port runs every viewport at the single-player density (a modern-port
//!   call, PLAN §How the project runs: faithful content, no hardware
//!   limits). The halving is B2 376186–376214 if it is ever wanted.
//! - **Per game** ([`Game`]): the parse is the same; the game changes what a
//!   RUNTIME does with two keys — BF1 never reads `FrameSize` (a fixed 4×4
//!   atlas, §6.3) nor parses `Looping`; BF2 reads `FrameSize` as the atlas
//!   cell edge in texels and `Looping` (default on). Both are stored as
//!   authored and the consumer applies the game's rule.
//! - Shared slots, kept as the PS2 keeps them: `Texture` and `Model` write
//!   the same word (Geometry +0x04), and a second nested `ParticleEmitter`
//!   in one scope replaces the first (one pointer) — the last record wins.

use crate::config::{parse_tree, ConfigNode};
use crate::{Chunk, UcfbError};

/// `fnv1a_lowercase` of every key the particle parsers switch on.
pub mod key {
    pub const PARTICLE_EMITTER: u32 = 0xcebb_215f;
    // ParticleEmitter scope (§2.1)
    pub const START_DELAY: u32 = 0x826c_d95e;
    pub const BURST_DELAY: u32 = 0x78fc_3314;
    pub const BURST_COUNT: u32 = 0x33cb_b418;
    pub const MAX_PARTICLES: u32 = 0x865d_5c88;
    pub const RED: u32 = 0x40f4_80dc;
    pub const GREEN: u32 = 0x011d_ecbc;
    pub const BLUE: u32 = 0x82fb_f5cd;
    pub const ALPHA: u32 = 0x5d8b_6dab;
    pub const HUE: u32 = 0xe38d_6359;
    pub const SATURATION: u32 = 0xf5a2_e289;
    pub const VALUE: u32 = 0x425e_d3ca;
    pub const SIZE: u32 = 0x23a0_d95c;
    pub const SPAWNER: u32 = 0x4936_726f;
    pub const TRANSFORMER: u32 = 0xf177_3bf8;
    pub const GEOMETRY: u32 = 0x2a6d_8591;
    pub const MIN_LOD_DIST: u32 = 0x6245_ae9e;
    pub const MAX_LOD_DIST: u32 = 0xb9d6_1dcc;
    pub const BOUNDING_RADIUS: u32 = 0xcbbf_df2b;
    pub const NO_REGISTER_STEP: u32 = 0xa7fc_4de1;
    pub const WORLD_ALIGNED: u32 = 0x33bb_ef79;
    pub const SOUND_NAME: u32 = 0x7a5f_f18f;
    // Spawner (§2.2)
    pub const SPREAD: u32 = 0x494a_00ce;
    pub const VORTEX: u32 = 0xd876_0e19;
    pub const CIRCLE: u32 = 0x2821_7089;
    /// Spawner shape type 4 — the name is not cracked (re-fx §2).
    pub const SHAPE4: u32 = 0xc091_789a;
    pub const OFFSET: u32 = 0x14c8_d3ca;
    pub const POSITION_X: u32 = 0x57d8_7d76;
    pub const POSITION_Y: u32 = 0x58d8_7f09;
    pub const POSITION_Z: u32 = 0x55d8_7a50;
    pub const START_X: u32 = 0xe9b8_52e5;
    pub const START_Y: u32 = 0xe8b8_5152;
    pub const START_Z: u32 = 0xe7b8_4fbf;
    /// The two unnamed X/Y/Z families of the Vortex / type-4 shapes.
    pub const SHAPE_A_X: u32 = 0xf102_de8a;
    pub const SHAPE_A_Y: u32 = 0xf202_e01d;
    pub const SHAPE_A_Z: u32 = 0xef02_db64;
    pub const SHAPE_B_X: u32 = 0xaa11_aa04;
    pub const SHAPE_B_Y: u32 = 0xab11_ab97;
    pub const SHAPE_B_Z: u32 = 0xac11_ad2a;
    pub const POSITION_SCALE: u32 = 0x9ff3_63c0;
    pub const VELOCITY_SCALE: u32 = 0x983c_4168;
    pub const INHERIT_VELOCITY_FACTOR: u32 = 0xb9a4_157c;
    pub const FADE_IN_TIME: u32 = 0x5083_ecd9;
    pub const START_ROTATION: u32 = 0x3d4c_ef81;
    pub const ROTATION_VELOCITY: u32 = 0x164b_5138;
    // Transformer (§2.3)
    pub const POSITION: u32 = 0x934f_4e0a;
    pub const ROTATION: u32 = 0x21ac_415f;
    pub const COLOR: u32 = 0x3d7e_6258;
    pub const LIFE_TIME: u32 = 0x5212_abee;
    pub const NEXT: u32 = 0x5cb6_8de8;
    pub const ACCELERATE: u32 = 0x9d9f_16f0;
    pub const REACH: u32 = 0x4cfd_5078;
    pub const SCALE: u32 = 0x8297_1c71;
    pub const MOVE: u32 = 0x184b_0014;
    // Geometry (§2.4)
    pub const TEXTURE: u32 = 0x3c64_68f4;
    pub const MODEL: u32 = 0xb08b_665a;
    pub const BLEND_MODE: u32 = 0xfa78_4eab;
    pub const TYPE: u32 = 0x5127_f14d;
    pub const TIME_PER_FRAME: u32 = 0x4134_95b0;
    pub const FRAME_SIZE: u32 = 0xfaf5_9ea7;
    pub const TOTAL_FRAMES: u32 = 0x5579_9c97;
    pub const BLUR_VALUE: u32 = 0xfc1f_d229;
    pub const BLUR_RES: u32 = 0x7971_e4d6;
    pub const SPARK_LENGTH: u32 = 0x530a_aa4e;
    pub const LOOPING: u32 = 0x7593_992d;
    // Values of BlendMode / Type (hashed strings)
    pub const ADDITIVE: u32 = 0x06a6_a0a5;
    pub const BLUR: u32 = 0x6bfb_d198;
    pub const PARTICLE: u32 = 0x8a87_43bf;
    pub const STREAK: u32 = 0x596f_63f3;
    pub const EMITTER: u32 = 0x576b_09cd;
    pub const ANIMATED: u32 = 0x6f35_dfbc;
    pub const BILLBOARD: u32 = 0xe506_fa64;
    pub const SPARK: u32 = 0x3f2f_216a;
}

/// 1/255 exactly as the ELFs hold it (`lui 0x3b80 / ori 0x8081`).
pub const COLOR_SCALE: f32 = f32::from_bits(0x3b80_8081);

/// `a·b` as the EE FPU computes it: rounded toward zero, not to nearest.
/// The product of two f32s is exact in f64, so truncating that is the PS2's
/// answer; IEEE's differs by one ulp on e.g. 249 × 1/255, and with this the
/// parsed values equal the PS2's RAM bit for bit (`tests/suite/fx_lvl.rs`).
pub fn ps2_mul(a: f32, b: f32) -> f32 {
    toward_zero(a as f64 * b as f64)
}

/// `a − b` as the EE FPU computes it: the smaller operand is aligned to the
/// larger's exponent keeping ONE guard bit (the rest truncated), then the
/// sum is rounded toward zero. Fitted on the RAM vector's 20 random colour
/// spans (`lead_tmp/fxp1`): one guard bit matches 20/20, exact-then-truncate
/// 8/20, two or three guard bits 17/20.
pub fn ps2_sub(a: f32, b: f32) -> f32 {
    let (a, b) = (a as f64, -(b as f64));
    let (big, small) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
    if big == 0.0 || small == 0.0 {
        return toward_zero(big + small);
    }
    // The ulp of `big`'s binade with a 24-bit significand, halved (the
    // guard bit): 2^(E − 25) for big in [2^(E−1), 2^E).
    let exp = ((big as f32).to_bits() >> 23 & 0xff) as i32;
    let u = 2f64.powi(exp - 151);
    toward_zero(big + (small / u).trunc() * u)
}

fn toward_zero(x: f64) -> f32 {
    let r = x as f32;
    if r != 0.0 && r.is_finite() && (r as f64).abs() > x.abs() {
        f32::from_bits(r.to_bits() - 1)
    } else {
        r
    }
}

/// Which disc an effect came from — it decides the runtime's reading of
/// `FrameSize` and `Looping` (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Bf1,
    Bf2,
}

/// A sampled value: `base + span·rand()` (re-fx §2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Range {
    pub base: f32,
    pub span: f32,
}

impl Range {
    pub const fn fixed(v: f32) -> Range {
        Range { base: v, span: 0.0 }
    }
    pub const fn new(base: f32, span: f32) -> Range {
        Range { base, span }
    }
    /// B1's third word: whether the value is drawn at random.
    pub fn random(&self) -> bool {
        self.span != 0.0
    }

    /// B1 `FUN_002ba1f0` / B2 `FUN_0037b7f0`: the pair starting at slot
    /// `at`, scaled by `k`, in the EE FPU's arithmetic ([`ps2_mul`]).
    fn from_slots(n: &ConfigNode, at: usize, k: f32) -> Range {
        let a = ps2_mul(n.f32(at).unwrap_or(0.0), k);
        match n.f32(at + 1) {
            Some(b) => {
                let b = ps2_mul(b, k);
                if a == b {
                    Range::fixed(a)
                } else {
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    Range::new(lo, ps2_sub(hi, lo))
                }
            }
            None => Range::fixed(a),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Rgb,
    Hsv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// GS ALPHA 0x44, `(Cs − Cd)·As + Cd` — also any unknown value.
    Normal,
    /// 0x48, `Cs·As + Cd`.
    Additive,
    /// Two passes over a screen copy (its microprogram is re-fx §11 [H]).
    Blur,
}

/// Geometry `Type` (§2.4, codes 0–6 in the PS2's order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryType {
    /// Also any unknown or missing Type.
    Particle,
    Streak,
    Emitter,
    Animated,
    Geometry,
    Billboard,
    Spark,
}

/// Spawner shape (`S+0x6c`, §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// No shape record: the direction is 0 (§5.4).
    None,
    Spread,
    Vortex,
    Circle,
    /// Type 4 (0xc091789a), name uncracked.
    Type4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spawner {
    pub shape: Shape,
    /// The shape block: nine ranges in the PS2's slot order (+0, +0xc, …
    /// +0x60). Spread and Circle author PositionX/Y/Z into slots 0/1/2.
    /// Vortex and type 4 fill all nine: 0 StartX, 1 A_X, 2 B_X, 3 StartY,
    /// 4 B_Y, 5 A_Y, 6 StartZ, 7 B_Z, 8 A_Z (B1 `FUN_002bbbc0`, key names in
    /// [`key`]). The defaults (ctor) are those of the union.
    pub shape_ranges: [Range; 9],
    /// `Offset { PositionX/Y/Z }`.
    pub offset: [Range; 3],
    pub position_scale: Range,
    pub velocity_scale: Range,
    pub inherit_velocity_factor: Range,
    pub size: Range,
    /// Red/Green/Blue/Alpha or Hue/Saturation/Value/Alpha, ×1/255.
    pub color: [Range; 4],
    /// Set by the last Red (RGB) or Hue (HSV) record — the mode the runtime
    /// uses (§5.8).
    pub color_mode: ColorMode,
    /// An EMITTER fade-in over effect time, not per particle (§5.6).
    pub fade_in_time: f32,
    /// As authored (no unit conversion, §3).
    pub start_rotation: Range,
    pub rotation_velocity: Range,
}

impl Default for Spawner {
    /// B1 `FUN_002b6b30` = B2 `FUN_00378890`.
    fn default() -> Spawner {
        let r = Range::new;
        let f = Range::fixed;
        Spawner {
            shape: Shape::None,
            shape_ranges: [
                r(0.0, 1.0),
                f(f32::from_bits(0x3d07_2b02)),
                f(1.5),
                r(0.0, 1.0),
                f(0.0),
                f(f32::from_bits(0x3d95_8106)),
                r(0.0, 1.0),
                f(f32::from_bits(0x3f99_999a)),
                f(f32::from_bits(0x3dbe_76c9)),
            ],
            offset: [f(0.0); 3],
            position_scale: f(5.0),
            velocity_scale: f(f32::from_bits(0x3c23_d70a)),
            inherit_velocity_factor: f(0.0),
            size: f(f32::from_bits(0x3e99_999a)),
            color: [
                r(f32::from_bits(0x3f66_6666), f32::from_bits(0x3dcc_ccd0)),
                r(f32::from_bits(0x3f66_6666), f32::from_bits(0x3dcc_ccd0)),
                r(f32::from_bits(0x3f66_6666), f32::from_bits(0x3dcc_ccd0)),
                f(0.5),
            ],
            color_mode: ColorMode::Rgb,
            fade_in_time: 0.0,
            start_rotation: r(0.0, 360.0),
            rotation_velocity: r(-50.0, 100.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionOp {
    None,
    /// vel += A·s·min(u,1) — A is the total change over the node (§5.7).
    Accelerate,
    /// vel = lerp(vel, A·s, u).
    Reach,
    /// vel ·= lerp(1, k, u), k = `value[0]`.
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarOp {
    None,
    /// Adds min(u, 1) — the authored value is IGNORED (§5.7, asm).
    Move,
    Reach,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorOp {
    None,
    /// c = clamp01(c + D·min(u,1)).
    Move,
    /// c = lerp(c, D, u).
    Reach,
}

/// One node of a Transformer chain. A chain is the node and its `Next()`
/// successors in order; each runs for its `lifetime` (default 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node<Op, V> {
    pub lifetime: f32,
    pub op: Op,
    pub value: V,
}

pub type PositionNode = Node<PositionOp, [f32; 3]>;
pub type ScalarNode = Node<ScalarOp, f32>;
/// `value` ×1/255.
pub type ColorNode = Node<ColorOp, [f32; 4]>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transformer {
    /// Every particle's life (`[7]`, 0 when unauthored — the ctor leaves the
    /// word unwritten; every disc emitter authors it).
    pub lifetime: f32,
    pub position: Vec<PositionNode>,
    pub size: Vec<ScalarNode>,
    /// Acts on the rotation VELOCITY (§5.7). No disc authors one.
    pub rotation: Vec<ScalarNode>,
    pub color: Vec<ColorNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    pub blend: BlendMode,
    pub kind: GeometryType,
    /// The texture name (`Texture`) — shares its slot with `model`.
    pub texture: Option<String>,
    /// The model name (`Model`, drawn by Type GEOMETRY).
    pub model: Option<String>,
    /// Type EMITTER: the per-particle child emitter.
    pub child: Option<Box<Emitter>>,
    /// ANIMATED, seconds per frame (default 2.0).
    pub time_per_frame: f32,
    /// ANIMATED, as authored (default 32). BF1 never reads it; BF2: the
    /// atlas cell edge in texels.
    pub frame_size: f32,
    /// ANIMATED (default 4).
    pub total_frames: f32,
    pub blur_value: f32,
    pub blur_res: i32,
    pub spark_length: f32,
    /// ANIMATED, BF2 only (default on); BF1 does not parse the key.
    pub looping: bool,
}

impl Default for Geometry {
    /// B1 `FUN_002b6108` / B2 `FUN_00377b28`.
    fn default() -> Geometry {
        Geometry {
            blend: BlendMode::Normal,
            kind: GeometryType::Particle,
            texture: None,
            model: None,
            child: None,
            time_per_frame: 2.0,
            frame_size: 32.0,
            total_frames: 4.0,
            blur_value: 0.0,
            blur_res: 0,
            spark_length: 0.0,
            looping: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    /// `ParticleEmitter("<label>")`.
    pub label: String,
    pub start_delay: Range,
    pub burst_delay: Range,
    pub burst_count: Range,
    /// A LIFETIME budget, −1 = forever (§5.2).
    pub max_particles: Range,
    /// Emitter-level colour (×1/255), multiplied into every particle after
    /// the Transformer (§5.8), in the SPAWNER's colour mode.
    pub color: [Range; 4],
    /// The emitter-level mode word (Red → RGB, Hue → HSV): never read at
    /// runtime (§5.8), kept because the PS2 stores it.
    pub color_mode: ColorMode,
    pub size: Range,
    pub spawner: Spawner,
    pub transformer: Transformer,
    pub geometry: Geometry,
    /// Parsed and never read by either ELF (§6.8).
    pub min_lod_dist: f32,
    pub max_lod_dist: f32,
    /// Picks the far-fade class (> 20 / > 5 / else, §3, §6.8).
    pub bounding_radius: f32,
    pub no_register_step: bool,
    pub world_aligned: bool,
    /// A nested `ParticleEmitter`: a sibling instance of the same effect.
    pub sibling: Option<Box<Emitter>>,
    pub sound_name: Option<String>,
    /// Keys no parser switch knows, and indexed records with i ≠ 0, in
    /// order (the scope they were in is not kept). Empty on both discs.
    pub unknown: Vec<u32>,
}

impl Default for Emitter {
    /// B1 `FUN_002b1b38` / B2 `FUN_003742a0`.
    fn default() -> Emitter {
        Emitter {
            label: String::new(),
            start_delay: Range::default(),
            burst_delay: Range::default(),
            burst_count: Range::default(),
            max_particles: Range::default(),
            color: [Range::fixed(1.0); 4],
            color_mode: ColorMode::Rgb,
            size: Range::fixed(1.0),
            spawner: Spawner::default(),
            transformer: Transformer::default(),
            geometry: Geometry::default(),
            min_lod_dist: 10.0,
            max_lod_dist: 50.0,
            bounding_radius: 5.0,
            no_register_step: false,
            world_aligned: false,
            sibling: None,
            sound_name: None,
            unknown: Vec::new(),
        }
    }
}

impl Emitter {
    /// This emitter and every emitter under it (siblings and Geometry
    /// children), depth first: self, geometry child tree, sibling tree —
    /// the census's count (re-fx §1).
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Emitter>) {
        out.push(self);
        if let Some(c) = &self.geometry.child {
            c.walk(out);
        }
        if let Some(s) = &self.sibling {
            s.walk(out);
        }
    }

    fn parse(n: &ConfigNode) -> Emitter {
        let mut e = Emitter {
            label: n.str_arg(0).unwrap_or_default().to_owned(),
            ..Emitter::default()
        };
        for k in &n.children {
            let r = |s: f32| Range::from_slots(k, 0, s);
            match k.hash {
                key::START_DELAY => e.start_delay = r(1.0),
                key::BURST_DELAY => e.burst_delay = r(1.0),
                key::BURST_COUNT => e.burst_count = r(1.0),
                key::MAX_PARTICLES => e.max_particles = r(1.0),
                key::SIZE => e.size = r(1.0),
                key::RED => {
                    e.color_mode = ColorMode::Rgb;
                    e.color[0] = r(COLOR_SCALE);
                }
                key::HUE => {
                    e.color_mode = ColorMode::Hsv;
                    e.color[0] = r(COLOR_SCALE);
                }
                key::GREEN | key::SATURATION => e.color[1] = r(COLOR_SCALE),
                key::BLUE | key::VALUE => e.color[2] = r(COLOR_SCALE),
                key::ALPHA => e.color[3] = r(COLOR_SCALE),
                key::SPAWNER => parse_spawner(&mut e.spawner, k, &mut e.unknown),
                key::TRANSFORMER => parse_transformer(&mut e.transformer, k, &mut e.unknown),
                key::GEOMETRY => parse_geometry(&mut e.geometry, k, &mut e.unknown),
                key::MIN_LOD_DIST => e.min_lod_dist = k.f32(0).unwrap_or(0.0),
                key::MAX_LOD_DIST => e.max_lod_dist = k.f32(0).unwrap_or(0.0),
                key::BOUNDING_RADIUS => e.bounding_radius = k.f32(0).unwrap_or(0.0),
                // Presence sets the flag; the argument is not read.
                key::NO_REGISTER_STEP => e.no_register_step = true,
                key::WORLD_ALIGNED => e.world_aligned = true,
                key::PARTICLE_EMITTER => e.sibling = Some(Box::new(Emitter::parse(k))),
                // Only a one-argument record is taken (B1 checks its argument count).
                key::SOUND_NAME if k.values.len() == 1 => {
                    e.sound_name = k.str_arg(0).map(str::to_owned);
                }
                key::SOUND_NAME => {}
                h => e.unknown.push(h),
            }
        }
        // §3: a continuous emitter bursts every 0.1–0.2 s. The test is the
        // ELF's, sum for sum (B1 247225–247230).
        let (m, b) = (e.max_particles, e.burst_delay);
        if m.base == -1.0 && m.base + m.span == -1.0 && b.base + b.span == 0.0 && b.base == 0.0 {
            let tenth = f32::from_bits(0x3dcc_cccd);
            e.burst_delay = Range::new(tenth, tenth);
        }
        e
    }
}

/// An indexed record's range: the pair after the index, or `None` (and the
/// key noted) for an index other than 0.
fn indexed(k: &ConfigNode, scale: f32, unknown: &mut Vec<u32>) -> Option<Range> {
    if k.f32(0).unwrap_or(0.0) as i32 != 0 {
        unknown.push(k.hash);
        return None;
    }
    Some(Range::from_slots(k, 1, scale))
}

/// `Spread/Circle/Offset { PositionX/Y/Z }` (B1 `FUN_002bb9d0`).
fn parse_xyz(out: &mut [Range], n: &ConfigNode, unknown: &mut Vec<u32>) {
    for k in &n.children {
        let slot = match k.hash {
            key::POSITION_X => 0,
            key::POSITION_Y => 1,
            key::POSITION_Z => 2,
            h => {
                unknown.push(h);
                continue;
            }
        };
        out[slot] = Range::from_slots(k, 0, 1.0);
    }
}

/// The Vortex / type-4 block (B1 `FUN_002bbbc0`).
fn parse_shape9(out: &mut [Range; 9], n: &ConfigNode, unknown: &mut Vec<u32>) {
    for k in &n.children {
        let slot = match k.hash {
            key::START_X => 0,
            key::SHAPE_A_X => 1,
            key::SHAPE_B_X => 2,
            key::START_Y => 3,
            key::SHAPE_B_Y => 4,
            key::SHAPE_A_Y => 5,
            key::START_Z => 6,
            key::SHAPE_B_Z => 7,
            key::SHAPE_A_Z => 8,
            h => {
                unknown.push(h);
                continue;
            }
        };
        out[slot] = Range::from_slots(k, 0, 1.0);
    }
}

/// B1 `FUN_002baab0` / B2 `FUN_0037bf00`.
fn parse_spawner(s: &mut Spawner, n: &ConfigNode, unknown: &mut Vec<u32>) {
    for k in &n.children {
        let r = |sc: f32| Range::from_slots(k, 0, sc);
        match k.hash {
            key::SPREAD => {
                s.shape = Shape::Spread;
                parse_xyz(&mut s.shape_ranges[..3], k, unknown);
            }
            key::CIRCLE => {
                s.shape = Shape::Circle;
                parse_xyz(&mut s.shape_ranges[..3], k, unknown);
            }
            key::VORTEX => {
                s.shape = Shape::Vortex;
                parse_shape9(&mut s.shape_ranges, k, unknown);
            }
            key::SHAPE4 => {
                s.shape = Shape::Type4;
                parse_shape9(&mut s.shape_ranges, k, unknown);
            }
            key::OFFSET => parse_xyz(&mut s.offset, k, unknown),
            key::POSITION_SCALE => s.position_scale = r(1.0),
            key::VELOCITY_SCALE => s.velocity_scale = r(1.0),
            key::INHERIT_VELOCITY_FACTOR => s.inherit_velocity_factor = r(1.0),
            key::FADE_IN_TIME => s.fade_in_time = k.f32(0).unwrap_or(0.0),
            key::SIZE => {
                if let Some(v) = indexed(k, 1.0, unknown) {
                    s.size = v;
                }
            }
            key::START_ROTATION => {
                if let Some(v) = indexed(k, 1.0, unknown) {
                    s.start_rotation = v;
                }
            }
            key::ROTATION_VELOCITY => {
                if let Some(v) = indexed(k, 1.0, unknown) {
                    s.rotation_velocity = v;
                }
            }
            h @ (key::RED
            | key::HUE
            | key::GREEN
            | key::SATURATION
            | key::BLUE
            | key::VALUE
            | key::ALPHA) => {
                // The mode word is written before the index is looked at.
                match h {
                    key::RED => s.color_mode = ColorMode::Rgb,
                    key::HUE => s.color_mode = ColorMode::Hsv,
                    _ => {}
                }
                let ch = match h {
                    key::RED | key::HUE => 0,
                    key::GREEN | key::SATURATION => 1,
                    key::BLUE | key::VALUE => 2,
                    _ => 3,
                };
                if let Some(v) = indexed(k, COLOR_SCALE, unknown) {
                    s.color[ch] = v;
                }
            }
            h => unknown.push(h),
        }
    }
}

/// The node's `LifeTime` / `Next` records, shared by every chain kind;
/// anything else goes to `op`. Returns the chain (this node + successors).
fn parse_chain<Op: Copy, V: Copy>(
    n: &ConfigNode,
    empty: Node<Op, V>,
    op: &mut dyn FnMut(&mut Node<Op, V>, &ConfigNode) -> bool,
    unknown: &mut Vec<u32>,
) -> Vec<Node<Op, V>> {
    let mut node = empty;
    let mut next: Vec<Node<Op, V>> = Vec::new();
    for k in &n.children {
        match k.hash {
            key::LIFE_TIME => node.lifetime = k.f32(0).unwrap_or(0.0),
            key::NEXT => next = parse_chain(k, empty, op, unknown),
            h => {
                if !op(&mut node, k) {
                    unknown.push(h);
                }
            }
        }
    }
    let mut out = vec![node];
    out.extend(next);
    out
}

/// B1 `FUN_002bb090` + the chain parsers `FUN_002bbe80` (Position),
/// `FUN_002bc1a0` / `FUN_002bc448` (Size / Rotation, op `FUN_002bca10`),
/// `FUN_002bc6f0` (Color).
fn parse_transformer(t: &mut Transformer, n: &ConfigNode, unknown: &mut Vec<u32>) {
    for k in &n.children {
        match k.hash {
            key::LIFE_TIME => t.lifetime = k.f32(0).unwrap_or(0.0),
            key::POSITION => {
                let empty = Node {
                    lifetime: 1.0,
                    op: PositionOp::None,
                    value: [0.0; 3],
                };
                t.position = parse_chain(
                    k,
                    empty,
                    &mut |nd, r| {
                        let v = [r.f32(0), r.f32(1), r.f32(2)].map(|x| x.unwrap_or(0.0));
                        match r.hash {
                            key::ACCELERATE => (nd.op, nd.value) = (PositionOp::Accelerate, v),
                            key::REACH => (nd.op, nd.value) = (PositionOp::Reach, v),
                            // Scale writes the first word only.
                            key::SCALE => (nd.op, nd.value[0]) = (PositionOp::Scale, v[0]),
                            _ => return false,
                        }
                        true
                    },
                    unknown,
                );
            }
            h @ (key::SIZE | key::ROTATION) => {
                if k.f32(0).unwrap_or(0.0) as i32 != 0 {
                    unknown.push(h);
                    continue;
                }
                let empty = Node {
                    lifetime: 1.0,
                    op: ScalarOp::None,
                    value: 0.0,
                };
                let chain = parse_chain(
                    k,
                    empty,
                    &mut |nd, r| {
                        nd.op = match r.hash {
                            key::MOVE => ScalarOp::Move,
                            key::REACH => ScalarOp::Reach,
                            key::SCALE => ScalarOp::Scale,
                            _ => return false,
                        };
                        nd.value = r.f32(0).unwrap_or(0.0);
                        true
                    },
                    unknown,
                );
                if h == key::SIZE {
                    t.size = chain;
                } else {
                    t.rotation = chain;
                }
            }
            key::COLOR => {
                if k.f32(0).unwrap_or(0.0) as i32 != 0 {
                    unknown.push(k.hash);
                    continue;
                }
                let empty = Node {
                    lifetime: 1.0,
                    op: ColorOp::None,
                    value: [0.0; 4],
                };
                t.color = parse_chain(
                    k,
                    empty,
                    &mut |nd, r| {
                        nd.op = match r.hash {
                            key::MOVE => ColorOp::Move,
                            key::REACH => ColorOp::Reach,
                            _ => return false,
                        };
                        nd.value =
                            [0, 1, 2, 3].map(|i| ps2_mul(r.f32(i).unwrap_or(0.0), COLOR_SCALE));
                        true
                    },
                    unknown,
                );
            }
            h => unknown.push(h),
        }
    }
}

/// B1 `FUN_002bb548` / B2 `FUN_0037c4d8`.
fn parse_geometry(g: &mut Geometry, n: &ConfigNode, unknown: &mut Vec<u32>) {
    let word = |k: &ConfigNode| k.str_arg(0).map(crate::fnv1a_lowercase);
    for k in &n.children {
        let f = || k.f32(0).unwrap_or(0.0);
        match k.hash {
            key::TEXTURE => {
                g.texture = k.str_arg(0).map(str::to_owned);
                g.model = None;
            }
            key::MODEL => {
                g.model = k.str_arg(0).map(str::to_owned);
                g.texture = None;
            }
            key::BLEND_MODE => {
                g.blend = match word(k) {
                    Some(key::ADDITIVE) => BlendMode::Additive,
                    Some(key::BLUR) => BlendMode::Blur,
                    _ => BlendMode::Normal,
                }
            }
            key::TYPE => {
                g.kind = match word(k) {
                    Some(key::STREAK) => GeometryType::Streak,
                    Some(key::EMITTER) => GeometryType::Emitter,
                    Some(key::ANIMATED) => GeometryType::Animated,
                    Some(key::GEOMETRY) => GeometryType::Geometry,
                    Some(key::BILLBOARD) => GeometryType::Billboard,
                    Some(key::SPARK) => GeometryType::Spark,
                    _ => GeometryType::Particle,
                }
            }
            key::PARTICLE_EMITTER => g.child = Some(Box::new(Emitter::parse(k))),
            key::TIME_PER_FRAME => g.time_per_frame = f(),
            key::FRAME_SIZE => g.frame_size = f(),
            key::TOTAL_FRAMES => g.total_frames = f(),
            key::BLUR_VALUE => g.blur_value = f(),
            key::BLUR_RES => g.blur_res = f() as i32,
            key::SPARK_LENGTH => g.spark_length = f(),
            key::LOOPING => g.looping = f() != 0.0,
            h => unknown.push(h),
        }
    }
}

/// One `fx__` particle effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// `fnv1a_lowercase(effect name)` from the chunk's `NAME`.
    pub name_hash: u32,
    /// The root `ParticleEmitter` records, in file order.
    pub emitters: Vec<Emitter>,
}

impl Effect {
    /// `Ok(None)` for an `fx__` with no `ParticleEmitter` root (an
    /// environment `Effect(…)`, a `Shield`, …) or no 4-byte `NAME`.
    pub fn parse(c: &Chunk<'_>) -> Result<Option<Effect>, UcfbError> {
        let Some(name) = c.child(b"NAME").filter(|n| n.payload.len() == 4) else {
            return Ok(None);
        };
        let name_hash = u32::from_le_bytes(name.payload.try_into().unwrap());
        let emitters: Vec<Emitter> = parse_tree(c)?
            .iter()
            .filter(|n| n.hash == key::PARTICLE_EMITTER)
            .map(Emitter::parse)
            .collect();
        if emitters.is_empty() {
            return Ok(None);
        }
        Ok(Some(Effect {
            name_hash,
            emitters,
        }))
    }

    /// Every emitter of the effect, depth first (re-fx §1's count).
    pub fn all_emitters(&self) -> Vec<&Emitter> {
        let mut out = Vec::new();
        for e in &self.emitters {
            e.walk(&mut out);
        }
        out
    }

    /// Every texture and model name the effect's emitters draw.
    pub fn resources(&self) -> (Vec<&str>, Vec<&str>) {
        let (mut tex, mut mdl) = (Vec::new(), Vec::new());
        for e in self.all_emitters() {
            if let Some(t) = e.geometry.texture.as_deref().filter(|t| !t.is_empty()) {
                tex.push(t);
            }
            if let Some(m) = e.geometry.model.as_deref().filter(|m| !m.is_empty()) {
                mdl.push(m);
            }
        }
        (tex, mdl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_chunk;

    /// Every named key's constant is its name's hash, so a typo cannot pass.
    #[test]
    fn key_constants_are_their_names_hashes() {
        let h = crate::fnv1a_lowercase;
        for (name, v) in [
            ("ParticleEmitter", key::PARTICLE_EMITTER),
            ("StartDelay", key::START_DELAY),
            ("BurstDelay", key::BURST_DELAY),
            ("BurstCount", key::BURST_COUNT),
            ("MaxParticles", key::MAX_PARTICLES),
            ("Red", key::RED),
            ("Green", key::GREEN),
            ("Blue", key::BLUE),
            ("Alpha", key::ALPHA),
            ("Hue", key::HUE),
            ("Saturation", key::SATURATION),
            ("Value", key::VALUE),
            ("Size", key::SIZE),
            ("Spawner", key::SPAWNER),
            ("Transformer", key::TRANSFORMER),
            ("Geometry", key::GEOMETRY),
            ("MinLodDist", key::MIN_LOD_DIST),
            ("MaxLodDist", key::MAX_LOD_DIST),
            ("BoundingRadius", key::BOUNDING_RADIUS),
            ("NoRegisterStep", key::NO_REGISTER_STEP),
            ("WorldAligned", key::WORLD_ALIGNED),
            ("SoundName", key::SOUND_NAME),
            ("Spread", key::SPREAD),
            ("Vortex", key::VORTEX),
            ("Circle", key::CIRCLE),
            ("Offset", key::OFFSET),
            ("PositionX", key::POSITION_X),
            ("PositionY", key::POSITION_Y),
            ("PositionZ", key::POSITION_Z),
            ("StartX", key::START_X),
            ("StartY", key::START_Y),
            ("StartZ", key::START_Z),
            ("PositionScale", key::POSITION_SCALE),
            ("VelocityScale", key::VELOCITY_SCALE),
            ("InheritVelocityFactor", key::INHERIT_VELOCITY_FACTOR),
            ("FadeInTime", key::FADE_IN_TIME),
            ("StartRotation", key::START_ROTATION),
            ("RotationVelocity", key::ROTATION_VELOCITY),
            ("Position", key::POSITION),
            ("Rotation", key::ROTATION),
            ("Color", key::COLOR),
            ("LifeTime", key::LIFE_TIME),
            ("Next", key::NEXT),
            ("Accelerate", key::ACCELERATE),
            ("Reach", key::REACH),
            ("Scale", key::SCALE),
            ("Move", key::MOVE),
            ("Texture", key::TEXTURE),
            ("Model", key::MODEL),
            ("BlendMode", key::BLEND_MODE),
            ("Type", key::TYPE),
            ("TimePerFrame", key::TIME_PER_FRAME),
            ("FrameSize", key::FRAME_SIZE),
            ("TotalFrames", key::TOTAL_FRAMES),
            ("BlurValue", key::BLUR_VALUE),
            ("BlurRes", key::BLUR_RES),
            ("SparkLength", key::SPARK_LENGTH),
            ("Looping", key::LOOPING),
            ("ADDITIVE", key::ADDITIVE),
            ("BLUR", key::BLUR),
            ("PARTICLE", key::PARTICLE),
            ("STREAK", key::STREAK),
            ("EMITTER", key::EMITTER),
            ("ANIMATED", key::ANIMATED),
            ("GEOMETRY", key::GEOMETRY),
            ("BILLBOARD", key::BILLBOARD),
            ("SPARK", key::SPARK),
        ] {
            assert_eq!(h(name), v, "{name}");
        }
        assert_eq!(COLOR_SCALE, 1.0 / 255.0);
    }

    /// The effect-name hash is fnv20: `_` hashes as 0x7f (re-fx §1:
    /// `ord_explosion` = 0xc42b8278, plain lowercase FNV 0x387ac758).
    #[test]
    fn effect_names_hash_as_fnv20() {
        assert_eq!(crate::fnv1a_lowercase("ord_explosion"), 0xc42b_8278);
    }

    fn data(hash: u32, nums: &[f32], strs: &[&str]) -> Vec<u8> {
        let argc = nums.len() + strs.len();
        let mut p = hash.to_le_bytes().to_vec();
        p.push(argc as u8);
        for n in nums {
            p.extend(n.to_bits().to_le_bytes());
        }
        let mut blob = Vec::new();
        for s in strs {
            p.extend(((4 * argc + blob.len()) as u32).to_le_bytes());
            blob.extend(s.as_bytes());
            blob.push(0);
        }
        p.extend((blob.len() as u32).to_le_bytes());
        p.extend(blob);
        build_chunk(b"DATA", &p)
    }
    fn scop(body: Vec<u8>) -> Vec<u8> {
        build_chunk(b"SCOP", &body)
    }

    #[test]
    fn ranges_defaults_and_the_continuous_rule() {
        let mut spawner = data(key::SIZE, &[0.0, 0.6, 0.3], &[]);
        spawner.extend(data(key::HUE, &[0.0, 255.0, 0.0], &[]));
        let mut body = data(key::MAX_PARTICLES, &[-1.0, -1.0], &[]);
        body.extend(data(key::BURST_DELAY, &[0.0, 0.0], &[]));
        body.extend(data(key::SPAWNER, &[], &[]));
        body.extend(scop(spawner));
        let mut root = data(key::PARTICLE_EMITTER, &[], &["Smoke"]);
        root.extend(scop(body));
        let mut fx = build_chunk(b"NAME", &0x1234_5678u32.to_le_bytes());
        fx.extend(root);
        let buf = build_chunk(b"fx__", &fx);
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let e = Effect::parse(&c).unwrap().unwrap();
        assert_eq!(e.name_hash, 0x1234_5678);
        let em = &e.emitters[0];
        assert_eq!(em.label, "Smoke");
        assert_eq!(em.burst_delay, Range::new(0.1, 0.1));
        assert!(em.burst_delay.random());
        assert_eq!(em.max_particles, Range::fixed(-1.0));
        // (0.6, 0.3) is stored low-first.
        assert_eq!(em.spawner.size.base, 0.3);
        assert!((em.spawner.size.span - 0.3).abs() < 1e-6);
        assert_eq!(em.spawner.color_mode, ColorMode::Hsv);
        assert_eq!(
            em.spawner.color[0],
            Range::new(0.0, ps2_mul(255.0, COLOR_SCALE))
        );
        // Untouched: the ctors'.
        assert_eq!(em.spawner.position_scale, Range::fixed(5.0));
        assert_eq!(em.size, Range::fixed(1.0));
        assert_eq!(em.geometry.total_frames, 4.0);
        assert!(em.unknown.is_empty());
    }
}
