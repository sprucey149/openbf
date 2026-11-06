//! The PS2's particle emitter runtime (card FXP2, `analysis/re-fx.md` §5,
//! [O]; the BF2 differences §8): an effect's emitters, their bursts and
//! lifetime budget, the spawner (BF1's 20-variant cache), and each
//! particle's closed-form Transformer at its age. Visual only.
//!
//! **Stateless like the PS2.** The PS2 rebuilds every live particle each
//! frame from its burst's history (§5.1): the port keeps each instance's
//! live BURSTS (time, first index, count, the emitter's frame then) and
//! re-evaluates every particle at `age = now − burst time` — the same closed
//! forms, so no integration error and no per-particle state.
//!
//! - **Instance** (B1 `FUN_002b1e40`): MaxParticles, StartDelay, the emitter
//!   R|H G|S B|V A multipliers and Size are sampled once; the first burst is
//!   at StartDelay.
//! - **Bursts** (§5.2): per burst BurstDelay (the time to the next one) and
//!   BurstCount, rounded `(int)(x + 0.5)` and clamped to what MaxParticles
//!   has left — a LIFETIME budget (−1 = forever): spawning stops once it is
//!   spent and the instance dies when the last burst's particles retire. A
//!   burst retires once `burst time + Transformer.LifeTime < now`.
//! - **Spawner** (§5.3–§5.4): BF1 draws every particle from one of 20
//!   variants per emitter template, slot `(idx + instance offset) % 20`,
//!   each filled once and stored as 16-bit integers (position / velocity
//!   ×100, size ×1000, StartRotation ×360, RotationVelocity ×100, colour
//!   bytes; InheritVelocityFactor unquantised); BF2 seeds every particle on
//!   its own and keeps floats. Spread → dir = (X, Y, Z); Circle → the same,
//!   normalised (zero → +Y); pos = Offset + dir·PositionScale, vel =
//!   dir·VelocityScale; into the world by the burst's frame (rotation +
//!   translation, velocity by rotation), `vel += frame velocity ·
//!   InheritVelocityFactor`, size ×= the instance's Size.
//!   NoRegisterStep spawns every burst in the CURRENT frame (the particles
//!   ride with the emitter), WorldAligned drops the rotation (§5.5).
//! - **FadeInTime** (§5.6): a burst emitted before it has its alpha ×
//!   `burst time / FadeInTime` — an emitter fade-in.
//! - **Transformer** (§5.7): each chain node runs its LifeTime with u = t/L
//!   and hands on to Next with t − L. Position (trapezoid, h = min(u,1)·L/2):
//!   Accelerate vel += A·s·min(u,1), Reach vel → A·s, Scale vel ·= lerp(1, k,
//!   u); after the last node the particle coasts. Size / Rotation: Reach,
//!   Scale, and **Move adds u (1 once a non-zero Move node is done) — its
//!   value is ignored**; the Rotation chain acts on the rotation VELOCITY.
//!   Colour: Move clamp01(c + D·min(u,1)), Reach lerp; four channels.
//!   s = the instance's Size. Then × the instance colour (§5.8).
//! - **ANIMATED frame** (§5.9, §8): BF1 `f = (int)(age / TimePerFrame)`,
//!   `while TotalFrames < f { f −= TotalFrames }` (frame N reachable, the
//!   original's off-by-one); BF2 `f %= TotalFrames` with Looping, else a
//!   frame past the end is not drawn.
//!
//! RNG (§5.2): the PS2's LCG `s' = s·0x19660d + 0x3c6ef35f` and its float
//! from two steps. The seeds themselves are the PS2's per-session randoms,
//! so the port's are its own.
//!
//! **Not modelled** (each named where it would go): the Vortex / type-4
//! shapes (3 BF1 HOT1 emitters, dir 0 here — their table-sine form is [D]
//! only, PROVISIONAL); the emitter-path history's 0.1 s node spacing (a
//! burst takes the frame of the moment it is emitted); the "phantom" burst
//! the PS2 draws one BurstDelay early (AGENT_A 1.10, an artifact of its
//! stateless loop); the per-instance 127-per-frame and BF2's split-screen
//! halving — PS2 budgets the port lifts (FEEL_NOTES). BF2's clamp of
//! MaxParticles to 128 is kept (it shapes how long an effect runs).

use glam::{Mat3, Vec3};
use manifest::{
    FxColorNode, FxColorOp, FxEmitter, FxPositionNode, FxPositionOp, FxRange, FxScalarNode,
    FxScalarOp, FxShape, FxSpawner, FxType,
};

/// Which ELF's rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Bf1,
    Bf2,
}

/// Variants per BF1 spawner (B1 245486).
pub const VARIANTS: u32 = 20;
/// Every particle seed is xored with this (B1 245490, B2 374664).
pub const SEED_XOR: u32 = 0xc71c_2830;
/// BF2's per-instance cap on MaxParticles (B2 371576).
pub const BF2_MAX_PARTICLES: i32 = 128;
/// The port's safety cap on bursts one update may emit (a zero-delay
/// forever emitter would otherwise never stop).
const MAX_BURSTS_PER_UPDATE: usize = 256;
/// The port's safety cap on live particles per instance (the PS2's 127 per
/// frame is lifted, see the module doc).
pub const MAX_LIVE: usize = 2048;

/// The PS2's LCG (re-fx §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lcg(pub u32);

impl Lcg {
    pub fn step(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
        self.0
    }

    /// A float in [0, 1) from two steps.
    pub fn float(&mut self) -> f32 {
        let s1 = self.step();
        let s2 = self.step();
        f32::from_bits((((s1 >> 16) | (s2 & 0xffff_0000)) >> 9) | 0x3f80_0000) - 1.0
    }

    /// `base + span·rand`, drawing only when the range is random.
    pub fn range(&mut self, r: FxRange) -> f32 {
        if r.1 != 0.0 {
            r.0 + r.1 * self.float()
        } else {
            r.0
        }
    }
}

/// One spawner draw, in the emitter's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Variant {
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub rgba: [f32; 4],
    pub rot: f32,
    pub rotvel: f32,
    pub ivf: f32,
}

/// `x` through a 16-bit store at `scale` (EE `cvt.w.s` truncates, the
/// halfword wraps).
fn q16(x: f32, scale: f32) -> f32 {
    f32::from((x * scale) as i32 as i16) / scale
}

/// A colour channel through its byte.
fn q8(c: f32) -> f32 {
    f32::from((c * 255.0) as i32 as u8) / 255.0
}

/// One spawner variant (B1 `FUN_002b7ef0`, sampling order AGENT_A 2.4).
/// `quantise` = BF1's cache.
pub fn sample_variant(sp: &FxSpawner, rng: &mut Lcg, quantise: bool) -> Variant {
    let mut dir = Vec3::new(
        rng.range(sp.shape_ranges[0]),
        rng.range(sp.shape_ranges[1]),
        rng.range(sp.shape_ranges[2]),
    );
    match sp.shape {
        FxShape::Spread => {}
        FxShape::Circle => {
            dir = if dir.length_squared() > 0.0 {
                dir.normalize()
            } else {
                Vec3::Y
            }
        }
        // PROVISIONAL (module doc).
        FxShape::None | FxShape::Vortex | FxShape::Type4 => dir = Vec3::ZERO,
    }
    let offset = Vec3::new(
        rng.range(sp.offset[0]),
        rng.range(sp.offset[1]),
        rng.range(sp.offset[2]),
    );
    let ps = rng.range(sp.position_scale);
    let vs = rng.range(sp.velocity_scale);
    let size = rng.range(sp.size);
    let rgba = [
        rng.range(sp.color[0]),
        rng.range(sp.color[1]),
        rng.range(sp.color[2]),
        rng.range(sp.color[3]),
    ];
    let rot = rng.range(sp.start_rotation);
    let rotvel = rng.range(sp.rotation_velocity);
    let ivf = rng.range(sp.inherit_velocity_factor);
    let (pos, vel) = (offset + dir * ps, dir * vs);
    if quantise {
        Variant {
            pos: Vec3::new(q16(pos.x, 100.0), q16(pos.y, 100.0), q16(pos.z, 100.0)),
            vel: Vec3::new(q16(vel.x, 100.0), q16(vel.y, 100.0), q16(vel.z, 100.0)),
            size: q16(size, 1000.0),
            rgba: rgba.map(q8),
            rot: q16(rot, 360.0),
            rotvel: q16(rotvel, 100.0),
            ivf,
        }
    } else {
        Variant {
            pos,
            vel,
            size,
            rgba,
            rot,
            rotvel,
            ivf,
        }
    }
}

/// BF1's 20 variants of one spawner from its construction seed.
pub fn variant_cache(sp: &FxSpawner, spawner_seed: u32) -> Vec<Variant> {
    (0..VARIANTS)
        .map(|slot| {
            let mut rng = Lcg((slot * 10).wrapping_add(spawner_seed) ^ SEED_XOR);
            sample_variant(sp, &mut rng, true)
        })
        .collect()
}

/// BF2's per-particle seed (B2 `FUN_00379518` 374664).
pub fn bf2_seed(idx: u32, r: u32) -> u32 {
    let s = (u64::from(idx) * 0x4001_8fc9 + u64::from(r) * 0x4000_029d) % 0x4000_0003;
    s as u32 ^ SEED_XOR
}

fn lerp(a: f32, b: f32, u: f32) -> f32 {
    a * (1.0 - u) + b * u
}

/// Progress through a node of life `l` at local time `t` (0 when l ≤ 0).
fn progress(t: f32, l: f32) -> f32 {
    if l > 0.0 {
        t * (1.0 / l)
    } else {
        0.0
    }
}

/// The Position chain at local time `t` (B1 `FUN_002b9610`), s = the
/// instance's Size. Without a chain: `pos + vel·t`.
pub fn position_chain(
    nodes: &[FxPositionNode],
    mut pos: Vec3,
    mut vel: Vec3,
    mut t: f32,
    s: f32,
) -> (Vec3, Vec3) {
    if nodes.is_empty() {
        return (pos + vel * t, vel);
    }
    for (i, n) in nodes.iter().enumerate() {
        let l = n.lifetime;
        let u = progress(t, l);
        let um = u.min(1.0);
        let h = um * l * 0.5;
        let a = Vec3::from(n.value);
        pos += vel * h;
        vel = match n.op {
            FxPositionOp::None => vel,
            FxPositionOp::Accelerate => vel + a * s * um,
            FxPositionOp::Reach => {
                if u >= 1.0 {
                    a * s
                } else {
                    vel * (1.0 - u) + a * s * u
                }
            }
            FxPositionOp::Scale => {
                vel * if u >= 1.0 {
                    n.value[0]
                } else {
                    lerp(1.0, n.value[0], u)
                }
            }
        };
        pos += vel * h;
        if u > 1.0 {
            if i + 1 < nodes.len() {
                t -= l;
                continue;
            }
            pos += vel * (t - l);
        }
        break;
    }
    (pos, vel)
}

/// One scalar op (B1 `FUN_002b98a8`, asm): Move adds u — 1 once a
/// non-zero node is done — and ignores its value.
fn scalar_op(v: f32, n: &FxScalarNode, u: f32) -> f32 {
    match n.op {
        FxScalarOp::None => v,
        FxScalarOp::Move => {
            if u > 1.0 && n.value != 0.0 {
                v + 1.0
            } else {
                v + u
            }
        }
        FxScalarOp::Reach => {
            if u >= 1.0 {
                n.value
            } else {
                lerp(v, n.value, u)
            }
        }
        FxScalarOp::Scale => {
            v * if u >= 1.0 {
                n.value
            } else {
                lerp(1.0, n.value, u)
            }
        }
    }
}

/// The Size chain at local time `t`; the end value holds.
pub fn size_chain(nodes: &[FxScalarNode], mut v: f32, mut t: f32) -> f32 {
    for (i, n) in nodes.iter().enumerate() {
        let u = progress(t, n.lifetime);
        v = scalar_op(v, n, u);
        if u > 1.0 && i + 1 < nodes.len() {
            t -= n.lifetime;
            continue;
        }
        break;
    }
    v
}

/// The Rotation chain (B1 `FUN_002b9a48`): the op turns the rotation
/// velocity; a completed node adds `rotvel_end·L`, the current one
/// `rotvel·t`. Without a chain: `rot + rotvel·t`.
pub fn rotation_chain(nodes: &[FxScalarNode], mut rot: f32, mut rotvel: f32, mut t: f32) -> f32 {
    if nodes.is_empty() {
        return rot + rotvel * t;
    }
    for (i, n) in nodes.iter().enumerate() {
        let u = progress(t, n.lifetime);
        rotvel = scalar_op(rotvel, n, u);
        if u > 1.0 && i + 1 < nodes.len() {
            rot += rotvel * n.lifetime;
            t -= n.lifetime;
            continue;
        }
        rot += rotvel * t;
        break;
    }
    rot
}

/// The Color chain (B1 `FUN_002b9b20`) on all four channels.
pub fn color_chain(nodes: &[FxColorNode], mut c: [f32; 4], mut t: f32) -> [f32; 4] {
    for (i, n) in nodes.iter().enumerate() {
        let u = progress(t, n.lifetime);
        for (k, ch) in c.iter_mut().enumerate() {
            let d = n.value[k];
            *ch = match n.op {
                FxColorOp::None => *ch,
                FxColorOp::Move => (*ch + d * u.min(1.0)).clamp(0.0, 1.0),
                FxColorOp::Reach => {
                    if u >= 1.0 {
                        d
                    } else {
                        lerp(*ch, d, u)
                    }
                }
            };
        }
        if u > 1.0 && i + 1 < nodes.len() {
            t -= n.lifetime;
            continue;
        }
        break;
    }
    c
}

/// The ANIMATED frame at `age`; `None` = not drawn (BF2 without Looping).
pub fn animated_frame(
    era: Era,
    age: f32,
    time_per_frame: f32,
    total: f32,
    looping: bool,
) -> Option<u32> {
    let f = if time_per_frame > 0.0 {
        (age / time_per_frame) as i32
    } else {
        0
    };
    let n = total as i32;
    match era {
        Era::Bf1 => {
            let mut f = f;
            if n > 0 {
                while n < f {
                    f -= n;
                }
            }
            Some(f.max(0) as u32)
        }
        Era::Bf2 => {
            if n <= 0 {
                Some(0)
            } else if looping {
                Some(f.rem_euclid(n) as u32)
            } else if f >= n {
                None
            } else {
                Some(f.max(0) as u32)
            }
        }
    }
}

/// An emitter's frame: where it is, how it is turned, how fast it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub rot: Mat3,
    pub pos: Vec3,
    pub vel: Vec3,
}

impl Frame {
    pub fn at(pos: Vec3) -> Frame {
        Frame {
            rot: Mat3::IDENTITY,
            pos,
            vel: Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Burst {
    time: f32,
    first: u32,
    count: u32,
    frame: Frame,
}

/// One evaluated particle, in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// Rotation of the emitter node at this particle’s birth.
    pub basis: Mat3,
    pub pos: Vec3,
    pub vel: Vec3,
    pub size: f32,
    pub rot: f32,
    /// RGBA, or HSVA when `hsv`.
    pub rgba: [f32; 4],
    pub hsv: bool,
    pub age: f32,
    /// The spawn index (the density thinning keys on `idx & 3`).
    pub idx: u32,
    /// ANIMATED only.
    pub frame: Option<u32>,
}

/// One emitter template's live instance.
#[derive(Debug, Clone)]
pub struct Instance {
    /// −1 = forever.
    max: i32,
    next_burst: f32,
    emitted: u32,
    retired: u32,
    bursts: std::collections::VecDeque<Burst>,
    rng: Lcg,
    /// The emitter-level multipliers (§5.8).
    pub rgba: [f32; 4],
    pub size: f32,
    pub inherit: f32,
    /// BF1: the cache offset; BF2: the per-particle seed's instance term.
    offset: u32,
    dead: bool,
}

impl Instance {
    /// A fresh instance of `em` (B1 `FUN_002b1e40`); `seed` is its random.
    pub fn new(em: &FxEmitter, era: Era, seed: u32, offset: u32) -> Instance {
        let mut rng = Lcg(seed ^ SEED_XOR);
        let mut max = rng.range(em.max_particles) as i32 as i16 as i32;
        if era == Era::Bf2 && max != -1 {
            max = max.min(BF2_MAX_PARTICLES);
        }
        let start = rng.range(em.start_delay);
        let rgba = [
            rng.range(em.color[0]),
            rng.range(em.color[1]),
            rng.range(em.color[2]),
            rng.range(em.color[3]),
        ];
        let size = rng.range(em.size);
        Instance {
            max,
            next_burst: start,
            emitted: 0,
            retired: 0,
            bursts: Default::default(),
            rng,
            rgba,
            size,
            inherit: 0.0,
            offset,
            dead: max == 0,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Particles emitted so far.
    pub fn emitted(&self) -> u32 {
        self.emitted
    }

    pub(super) fn child_offset(&self, idx: u32) -> u32 {
        self.offset.wrapping_add(idx.wrapping_mul(10))
    }

    pub(super) fn set_offset(&mut self, offset: u32) {
        self.offset = offset;
    }

    pub(super) fn translate(&mut self, delta: Vec3) {
        for burst in &mut self.bursts {
            burst.frame.pos += delta;
        }
    }

    fn budget_left(&self) -> Option<u32> {
        (self.max >= 0).then(|| (self.max as u32).saturating_sub(self.emitted))
    }

    /// Advance to effect time `t`: emit the bursts due (unless `stopped`),
    /// retire the spent ones, and die when nothing is left to draw.
    pub fn update(&mut self, em: &FxEmitter, t: f32, frame: Frame, stopped: bool) {
        self.update_path(em, t, stopped, |_| frame);
    }

    pub(super) fn update_path(
        &mut self,
        em: &FxEmitter,
        t: f32,
        stopped: bool,
        mut frame_at: impl FnMut(f32) -> Frame,
    ) {
        if self.dead {
            return;
        }
        let mut n_bursts = 0;
        while !stopped
            && self.next_burst <= t
            && self.budget_left() != Some(0)
            && n_bursts < MAX_BURSTS_PER_UPDATE
        {
            let delay = self.rng.range(em.burst_delay);
            let mut n = (self.rng.range(em.burst_count) + 0.5) as i32;
            n = n.max(0);
            if let Some(left) = self.budget_left() {
                n = n.min(left as i32);
            }
            let live: usize = self.bursts.iter().map(|b| b.count as usize).sum();
            if live + (n as usize) <= MAX_LIVE {
                self.bursts.push_back(Burst {
                    time: self.next_burst,
                    first: self.emitted,
                    count: n as u32,
                    frame: frame_at(self.next_burst),
                });
            }
            self.emitted += n as u32;
            self.next_burst += if delay > 0.0 { delay } else { 0.0 };
            n_bursts += 1;
            if delay <= 0.0 && self.max < 0 {
                // A zero-delay forever emitter: one burst per update.
                self.next_burst = t.next_up();
            }
        }
        let life = em.transformer.lifetime;
        while let Some(b) = self.bursts.front() {
            if b.time + life < t {
                self.retired += b.count;
                self.bursts.pop_front();
            } else {
                break;
            }
        }
        let spent = self.max >= 0 && self.retired >= self.max as u32;
        let over = (stopped || self.budget_left() == Some(0)) && self.bursts.is_empty();
        self.dead = spent || over;
    }

    /// Every live particle at effect time `t` (see the module doc). `keep`
    /// is the density thinning (`idx & 3` → drawn?); `now` the emitter's
    /// current frame (NoRegisterStep); `cache` BF1's variants.
    pub fn particles(
        &self,
        em: &FxEmitter,
        era: Era,
        t: f32,
        now: Frame,
        cache: &[Variant],
        keep: impl Fn(u32) -> bool,
    ) -> Vec<Particle> {
        let streak = em.geometry.kind == FxType::Streak;
        let mut out = Vec::new();
        for b in &self.bursts {
            let age = t - b.time;
            if age < 0.0 || age >= em.transformer.lifetime {
                continue;
            }
            for idx in b.first..b.first + b.count {
                if !streak && !keep(idx & 3) {
                    continue;
                }
                out.extend(self.particle(em, era, b, idx, age, t, now, cache));
            }
        }
        out
    }

    pub(super) fn particle_at(
        &self,
        em: &FxEmitter,
        era: Era,
        idx: u32,
        age: f32,
        now: Frame,
        cache: &[Variant],
    ) -> Option<Particle> {
        if age < 0.0 || age >= em.transformer.lifetime {
            return None;
        }
        let b = self
            .bursts
            .iter()
            .find(|b| (b.first..b.first + b.count).contains(&idx))?;
        self.particle(em, era, b, idx, age, b.time + age, now, cache)
    }

    #[allow(clippy::too_many_arguments)]
    fn particle(
        &self,
        em: &FxEmitter,
        era: Era,
        b: &Burst,
        idx: u32,
        age: f32,
        t: f32,
        now: Frame,
        cache: &[Variant],
    ) -> Option<Particle> {
        let tr = &em.transformer;
        let g = &em.geometry;
        let frame = if em.no_register_step { now } else { b.frame };
        let rot = if em.world_aligned {
            Mat3::IDENTITY
        } else {
            frame.rot
        };
        let v = match era {
            Era::Bf1 if !cache.is_empty() => {
                cache[((idx.wrapping_add(self.offset)) % VARIANTS) as usize]
            }
            _ => sample_variant(&em.spawner, &mut Lcg(bf2_seed(idx, self.offset)), false),
        };
        let pos = rot * v.pos + frame.pos;
        let vel = rot * v.vel + frame.vel * (v.ivf + self.inherit);
        let mut rgba = v.rgba;
        if em.spawner.fade_in_time > 0.0 && b.time < em.spawner.fade_in_time {
            rgba[3] *= b.time / em.spawner.fade_in_time;
        }
        let mut rot0 = v.rot;
        if g.kind == FxType::Streak {
            rot0 *= t;
        }
        let s = self.size;
        let (pos, vel) = position_chain(&tr.position, pos, vel, age, s);
        let size = size_chain(&tr.size, v.size * s, age);
        let basis = rot;
        let rot = rotation_chain(&tr.rotation, rot0, v.rotvel, age);
        let mut rgba = color_chain(&tr.color, rgba, age);
        for (c, m) in rgba.iter_mut().zip(self.rgba) {
            *c *= m;
        }
        let frame = if g.kind == FxType::Animated {
            Some(animated_frame(
                era,
                age,
                g.time_per_frame,
                g.total_frames,
                g.looping,
            )?)
        } else {
            None
        };
        Some(Particle {
            basis,
            pos,
            vel,
            size,
            rot,
            rgba,
            hsv: em.spawner.hsv,
            age,
            idx,
            frame,
        })
    }
}

/// HSV → RGB, 6 sectors, H in [0, 1) (B1 `FUN_002b62a8`): S = 0 is grey,
/// sector ≥ 5 (H ≥ 1 included, f = 6H − sector) is (V, p, q).
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    if s == 0.0 {
        return [v, v, v];
    }
    let h6 = h * 6.0;
    let i = (h6 as i32).max(0);
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduling_emitter(delay: f32) -> FxEmitter {
        let zero = FxRange(0.0, 0.0);
        let one = FxRange(1.0, 0.0);
        FxEmitter {
            label: "synthetic scheduler".into(),
            start_delay: zero,
            burst_delay: FxRange(delay, 0.0),
            burst_count: one,
            max_particles: FxRange(-1.0, 0.0),
            color: [one; 4],
            hsv: false,
            size: one,
            spawner: FxSpawner {
                shape: FxShape::None,
                shape_ranges: [zero; 9],
                offset: [zero; 3],
                position_scale: one,
                velocity_scale: one,
                inherit_velocity_factor: zero,
                size: one,
                color: [one; 4],
                hsv: false,
                fade_in_time: 0.0,
                start_rotation: zero,
                rotation_velocity: zero,
            },
            transformer: manifest::FxTransformer {
                lifetime: 1.0,
                position: vec![],
                size: vec![],
                rotation: vec![],
                color: vec![],
            },
            geometry: manifest::FxGeometry {
                blend: manifest::FxBlend::Normal,
                kind: FxType::Particle,
                texture: None,
                model: None,
                child: None,
                time_per_frame: 0.0,
                frame_size: 1.0,
                total_frames: 1.0,
                blur_value: 0.0,
                blur_res: 0,
                spark_length: 0.0,
                looping: false,
            },
            min_lod_dist: 0.0,
            max_lod_dist: 0.0,
            bounding_radius: 0.0,
            no_register_step: false,
            world_aligned: false,
            sibling: None,
            sound_name: None,
        }
    }

    #[test]
    fn nonpositive_forever_delays_emit_once_at_large_times() {
        for delay in [0.0, -1.0] {
            let em = scheduling_emitter(delay);
            let mut instance = Instance::new(&em, Era::Bf2, 1, 0);
            for (i, t) in [0.0, 1.0, 2.0, 4.0, 1_000_000.0].into_iter().enumerate() {
                instance.update(&em, t, Frame::at(Vec3::ZERO), false);
                assert_eq!(instance.emitted(), i as u32 + 1, "delay {delay}, time {t}");
                assert!(instance.next_burst > t);
            }
            let emitted = instance.emitted();
            instance.update(&em, 2_000_000.0, Frame::at(Vec3::ZERO), true);
            assert_eq!(instance.emitted(), emitted);
        }
        // Ordinary positive delays still catch up every due burst.
        let em = scheduling_emitter(0.5);
        let mut instance = Instance::new(&em, Era::Bf2, 1, 0);
        instance.update(&em, 2.0, Frame::at(Vec3::ZERO), false);
        assert_eq!(instance.emitted(), 5);
        assert_eq!(instance.next_burst, 2.5);
    }

    #[test]
    fn catch_up_bursts_take_the_frame_at_their_own_birth_time() {
        let mut em = scheduling_emitter(0.5);
        em.max_particles = FxRange(5.0, 0.0);
        em.transformer.lifetime = 10.0;
        let mut instance = Instance::new(&em, Era::Bf2, 1, 0);
        instance.update_path(&em, 2.0, false, |time| Frame::at(Vec3::X * time));
        let particles =
            instance.particles(&em, Era::Bf2, 2.0, Frame::at(Vec3::X * 2.0), &[], |_| true);
        assert_eq!(particles.len(), 5);
        for (i, particle) in particles.iter().enumerate() {
            assert!(particle.pos.abs_diff_eq(Vec3::X * (i as f32 * 0.5), 0.0001));
        }
    }

    fn pnode(lifetime: f32, op: FxPositionOp, value: [f32; 3]) -> FxPositionNode {
        FxPositionNode {
            lifetime,
            op,
            value,
        }
    }

    fn snode(lifetime: f32, op: FxScalarOp, value: f32) -> FxScalarNode {
        FxScalarNode {
            lifetime,
            op,
            value,
        }
    }

    /// The LCG and its float (§5.2), hand computed: s = 1 → 0x3d5f5c6c →
    /// 0x0f8e0afb; bits ((0x3d5f | 0x0f8e0000) >> 9) | 0x3f800000.
    #[test]
    fn the_lcg_and_its_float() {
        let mut r = Lcg(1);
        assert_eq!(r.step(), 0x3c6e_f35f_u32.wrapping_add(0x0019_660d));
        let mut r = Lcg(1);
        let s1 = 0x0019_660du32.wrapping_add(0x3c6e_f35f);
        let s2 = s1.wrapping_mul(0x0019_660d).wrapping_add(0x3c6e_f35f);
        let want = f32::from_bits((((s1 >> 16) | (s2 & 0xffff_0000)) >> 9) | 0x3f80_0000) - 1.0;
        assert_eq!(r.float(), want);
        assert!((0.0..1.0).contains(&want));
        // A fixed range draws nothing.
        let mut r = Lcg(7);
        assert_eq!(r.range(FxRange(2.5, 0.0)), 2.5);
        assert_eq!(r, Lcg(7));
    }

    /// Accelerate over L = 0.5 with A = (0, −10, 0), s = 2: the TOTAL dv is
    /// A·s = −20; at t = 0.25 (u = 0.5) h = 0.125, v = −10, pos = −1.25;
    /// at t = 1 the node is done (h = 0.25: pos = 0·0.25 + (−20)·0.25 = −5
    /// over the node) and the particle coasts 0.5 s more: pos = −15.
    #[test]
    fn accelerate_is_a_total_change_then_a_coast() {
        let n = [pnode(0.5, FxPositionOp::Accelerate, [0.0, -10.0, 0.0])];
        let (p, v) = position_chain(&n, Vec3::ZERO, Vec3::ZERO, 0.25, 2.0);
        assert!(
            (v.y + 10.0).abs() < 1e-6 && (p.y + 1.25).abs() < 1e-6,
            "{p} {v}"
        );
        let (p, v) = position_chain(&n, Vec3::ZERO, Vec3::ZERO, 1.0, 2.0);
        assert!(
            (v.y + 20.0).abs() < 1e-6 && (p.y + 15.0).abs() < 1e-5,
            "{p} {v}"
        );
        // No chain: pos + vel·t.
        let (p, _) = position_chain(&[], Vec3::ZERO, Vec3::X, 3.0, 1.0);
        assert_eq!(p, Vec3::new(3.0, 0.0, 0.0));
    }

    /// Reach lerps the velocity to A·s; Scale multiplies by lerp(1, k, u)
    /// and ignores s; a node hands on to Next with t − L.
    #[test]
    fn reach_scale_and_next() {
        let v0 = Vec3::new(4.0, 0.0, 0.0);
        let n = [pnode(1.0, FxPositionOp::Reach, [0.0, 1.0, 0.0])];
        let (_, v) = position_chain(&n, Vec3::ZERO, v0, 0.5, 2.0);
        assert!(v.abs_diff_eq(Vec3::new(2.0, 1.0, 0.0), 1e-6), "{v}");
        let n = [pnode(2.0, FxPositionOp::Scale, [0.0, 0.0, 0.0])];
        let (p, v) = position_chain(&n, Vec3::ZERO, v0, 1.0, 5.0);
        // u = 0.5: v = 4·0.5 = 2; h = 0.5: pos = 4·0.5 + 2·0.5 = 3.
        assert!(v.abs_diff_eq(Vec3::new(2.0, 0.0, 0.0), 1e-6), "{v}");
        assert!(p.abs_diff_eq(Vec3::new(3.0, 0.0, 0.0), 1e-6), "{p}");
        // Two nodes: None for 1 s (coasting at 4), then Scale to 0 over 1 s:
        // at t = 1.5, pos = 4 + (4·0.25 + 2·0.25) = 5.5.
        let n = [
            pnode(1.0, FxPositionOp::None, [0.0; 3]),
            pnode(1.0, FxPositionOp::Scale, [0.0; 3]),
        ];
        let (p, _) = position_chain(&n, Vec3::ZERO, v0, 1.5, 1.0);
        assert!((p.x - 5.5).abs() < 1e-6, "{p}");
    }

    /// Size: Scale ×3 over 0.1 s (ord_explosion's Flare) is ×2 halfway and
    /// holds ×3 after; Move ignores its value — +u, then +1 once a non-zero
    /// node is done; Reach lerps.
    #[test]
    fn size_ops_and_the_move_quirk() {
        let n = [snode(0.1, FxScalarOp::Scale, 3.0)];
        assert!((size_chain(&n, 0.1, 0.05) - 0.2).abs() < 1e-6);
        assert!((size_chain(&n, 0.1, 0.5) - 0.3).abs() < 1e-6);
        let n = [snode(2.0, FxScalarOp::Move, 7.0)];
        assert!(
            (size_chain(&n, 1.0, 1.0) - 1.5).abs() < 1e-6,
            "value 7 ignored"
        );
        assert!(
            (size_chain(&n, 1.0, 5.0) - 2.0).abs() < 1e-6,
            "+1 once done"
        );
        let n = [snode(2.0, FxScalarOp::Move, 0.0)];
        assert!(
            (size_chain(&n, 1.0, 5.0) - 3.5).abs() < 1e-6,
            "Move(0) grows by u"
        );
        let n = [snode(1.0, FxScalarOp::Reach, 3.0)];
        assert!((size_chain(&n, 1.0, 0.25) - 1.5).abs() < 1e-6);
    }

    /// Rotation: the chain acts on the rotation velocity (Scale 0 over 1 s
    /// from 10/s: at 0.5 s rotvel 5, rot = 5·0.5 = 2.5 — the rectangle rule).
    #[test]
    fn rotation_chain_turns_the_velocity() {
        let n = [snode(1.0, FxScalarOp::Scale, 0.0)];
        assert!((rotation_chain(&n, 0.0, 10.0, 0.5) - 2.5).abs() < 1e-6);
        assert_eq!(rotation_chain(&[], 1.0, 10.0, 0.5), 6.0);
    }

    /// Colour: Move clamps to [0, 1] and is a total change; Reach lerps
    /// unclamped; ord_explosion's Sparks go white in 0.01 s, then orange by
    /// 0.2 s.
    #[test]
    fn colour_move_clamps_and_reach_lerps() {
        let mv = [FxColorNode {
            lifetime: 1.0,
            op: FxColorOp::Move,
            value: [1.0, -1.0, 0.5, 0.0],
        }];
        let c = color_chain(&mv, [0.5, 0.5, 0.5, 1.0], 2.0);
        assert_eq!(c, [1.0, 0.0, 1.0, 1.0]);
        let c = color_chain(&mv, [0.5, 0.5, 0.5, 1.0], 0.25);
        assert!((c[0] - 0.75).abs() < 1e-6 && (c[2] - 0.625).abs() < 1e-6);
        let sparks = [
            FxColorNode {
                lifetime: 0.01,
                op: FxColorOp::Reach,
                value: [1.0, 1.0, 1.0, 1.0],
            },
            FxColorNode {
                lifetime: 0.19,
                op: FxColorOp::Reach,
                value: [1.0, 128.0 / 255.0, 0.0, 1.0],
            },
        ];
        let c = color_chain(&sparks, [1.0, 0.72, 0.07, 0.0], 0.005);
        assert!((c[3] - 0.5).abs() < 1e-5, "{c:?}");
        let c = color_chain(&sparks, [1.0, 0.72, 0.07, 0.0], 0.105);
        assert!((c[1] - lerp(1.0, 128.0 / 255.0, 0.5)).abs() < 1e-5, "{c:?}");
    }

    /// BF1's frame counter runs 0…N with frame 0 once (strict `<`); BF2
    /// wraps, or stops drawing without Looping.
    #[test]
    fn animated_frames_wrap_like_the_ps2() {
        let f = |age| animated_frame(Era::Bf1, age, 0.015, 16.0, true);
        assert_eq!(f(0.0), Some(0));
        assert_eq!(f(0.015 * 16.5), Some(16), "frame N is reachable");
        assert_eq!(f(0.015 * 17.5), Some(1), "17 − 16");
        let g = |age, l| animated_frame(Era::Bf2, age, 0.1, 4.0, l);
        assert_eq!(g(0.45, true), Some(0));
        assert_eq!(g(0.45, false), None);
        assert_eq!(g(0.35, false), Some(3));
    }

    /// BF1's cache quantises: StartRotation ×360 in an i16 wraps past
    /// ±91.02; sizes to 1/1000.
    #[test]
    fn the_cache_quantises_to_16_bits() {
        assert_eq!(q16(90.0, 360.0), 90.0);
        let w = q16(100.0, 360.0);
        assert!((w - (100.0 - 65536.0 / 360.0)).abs() < 1e-2, "{w}");
        assert_eq!(q16(0.30049, 1000.0), 0.3);
        assert_eq!(q8(0.5), 127.0 / 255.0);
    }

    /// HSV: the six sectors and grey.
    #[test]
    fn hsv_sectors() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), [1.0, 0.0, 0.0]);
        assert_eq!(hsv_to_rgb(0.5, 1.0, 1.0), [0.0, 1.0, 1.0]);
        assert_eq!(hsv_to_rgb(0.3, 0.0, 0.4), [0.4; 3]);
        // H = 1: sector 6 takes sector 5's (V, p, q) with f = 0, so q = V.
        assert_eq!(hsv_to_rgb(1.0, 1.0, 1.0), [1.0, 0.0, 1.0]);
    }
}
