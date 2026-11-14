//! Emitter arcs (card UORDE, `analysis/re-ordnance-kinds.md` §8; the PS2
//! object in `analysis/re-weather.md` §8.2–§8.3): the lightning an emitter
//! ordnance's `LightningEffect` draws between a shot's links.
//!
//! The PS2 spawns one `LightningBoltEffectObject` per link and rebuilds its
//! ribbon from two private LCG streams every frame. The port keeps the same
//! record constants (both discs author one `LightningBolt` for the arc
//! casters — `arccaster_lightning` in BF1, `com_sfx_lightning` in BF2 — with
//! the values in [`MAIN`] and [`CHILD`]) and the same `strip` shape, but
//! derives the seeds from the SIM TICK and the link's index, never a wall
//! clock or a thread-local RNG: a replay, a pause and a second view draw the
//! same arc.
//!
//! Lifetime: the record's `FadeTime` 0.6 s = 18 ticks at 30 Hz. The instance
//! counts down from `FadeTime` and the strip is at `min(1, 2·life/FadeTime)`
//! of its alpha (full for the first 0.3 s, then linear to 0). The shape
//! drifts by `InterpolationSpeed` 5 shapes a second (§8.2): at age `t` the
//! pair of seeds is draw `floor(5t)` and the next, blended by `frac(5t)`.
//! Child strips take fresh seeds each render on the PS2 (they flicker); here
//! they are a function of the tick, so they flicker at the sim's 30 Hz.
//!
//! Geometry is the PS2's ribbon: per point `P ± side`, the half-width in
//! metres, `side = normalize(T × (P − eye))·w` (T is the summed neighbouring
//! segments), U along the axis in `TextureSize` units and V 0/1 across.
//! Endpoint vertices have alpha 0; the inner ones carry the record's colour.
//! The port draws it through [`render::ParticleShape::Shaded`] — the existing
//! per-vertex-colour FX draw — under the additive blend the PS2's state
//! record uses (0x48: Cs·As + Cd, depth tested, no depth write, unfogged).

use super::emitter::{Lcg, SEED_XOR};
use core::fmath::FloatExt;
use glam::Vec3;
use render::{Camera, ParticleDraw, ParticleShape, TextureHandle};

/// The record both games author for the arc caster effects (BF1 COMMON.LVL
/// and BF2 INGAME.LVL hold the same values; the class name is the effect's
/// own `Texture`).
const WIDTH: f32 = 0.5;
const FADE_TIME: f32 = 0.6;
const SPREAD_FACTOR: f32 = 0.8;
const BREAK_DISTANCE: f32 = 1.0;
const INTERPOLATION_SPEED: f32 = 5.0;
const TEXTURE_SIZE: f32 = 3.5;
const COLOR: [u8; 4] = [130, 170, 255, 250];
const BRANCH_FACTOR: f32 = 0.15;
const MAX_BRANCHES: usize = 3;
/// The record leaves `BranchLength` and `BranchSpreadFactor` at the ctor
/// defaults (4 m, 0.8).
const BRANCH_LENGTH: f32 = 4.0;
const BRANCH_SPREAD_FACTOR: f32 = 0.8;
const NUM_CHILDREN: usize = 3;
const CHILD_WIDTH: f32 = 0.4;
const CHILD_BREAK_DISTANCE: f32 = 1.5;
const CHILD_TEXTURE_SIZE: f32 = 2.0;
const CHILD_SPREAD_FACTOR: f32 = 1.0;
const CHILD_COLOR: [u8; 4] = [140, 180, 255, 100];

/// The strip's lifetime in sim ticks (18 at 30 Hz).
const LIFE_TICKS: u64 = (FADE_TIME * core::TICK_HZ as f32).round() as u64;
/// A strip that would take 101 points or more is dropped whole (§8.3).
const MAX_SEGMENTS: i32 = 100;

/// One strip's draw rules (the PS2 `strip` arguments).
#[derive(Clone, Copy, Debug)]
struct Params {
    /// Half-width at the start (m).
    width: f32,
    spread: f32,
    break_distance: f32,
    texture_size: f32,
    color: [u8; 4],
    /// The A→B shape blend, 0..1.
    phase: f32,
    /// May this strip grow branches? (Branches themselves never do.)
    branches: bool,
}

const MAIN: Params = Params {
    width: WIDTH,
    spread: SPREAD_FACTOR,
    break_distance: BREAK_DISTANCE,
    texture_size: TEXTURE_SIZE,
    color: COLOR,
    phase: 0.0,
    branches: true,
};

const CHILD: Params = Params {
    width: CHILD_WIDTH,
    spread: CHILD_SPREAD_FACTOR,
    break_distance: CHILD_BREAK_DISTANCE,
    texture_size: CHILD_TEXTURE_SIZE,
    color: CHILD_COLOR,
    phase: 0.0,
    branches: false,
};

/// A branch the main strip grew: from an inner point along `dir` for
/// [`BRANCH_LENGTH`], with its own seed pair.
#[derive(Clone, Copy, Debug)]
struct Branch {
    point: Vec3,
    width: f32,
    dir: Vec3,
    seed_a: u32,
    seed_b: u32,
}

/// One strip's points and branches, before the view turns them into
/// camera-facing triangles. A point is (position, half-width, the record's
/// alpha byte — 0 at both ends).
#[derive(Clone, Debug)]
struct Shape {
    pts: Vec<(Vec3, f32, u8)>,
    branches: Vec<Branch>,
}

/// One emitter link's arc: where it runs, the tick it appeared and its
/// deterministic seed stream.
#[derive(Clone, Debug)]
pub struct Arc {
    from: Vec3,
    to: Vec3,
    /// The `LightningEffect` class, lowercase (its `Texture`).
    class: String,
    born: u64,
    /// The instance's one engine-LCG stream: draw `k` is `k + 1` steps in.
    stream: u32,
    /// The link's index among this tick's arcs (the child flicker seed).
    link: u32,
}

impl Arc {
    /// The arc `from → to` for effect `class`, drawn on sim tick `born`;
    /// `link` is its index among the tick's arcs.
    pub fn new(from: Vec3, to: Vec3, class: &str, born: u64, link: u32) -> Arc {
        // The PS2's two raw draws at creation come from the engine LCG. The
        // port seeds that stream from (tick, link) only, so an arc is a pure
        // function of when and where it was drawn.
        let t = born as u32 ^ (born >> 32) as u32;
        let mut rng = Lcg(0x2f8a_6c1d ^ t ^ link.wrapping_mul(0x9e37_79b9));
        Arc {
            from,
            to,
            class: class.to_ascii_lowercase(),
            born,
            stream: rng.step(),
            link,
        }
    }

    /// The effect class whose texture the strip samples.
    pub fn class(&self) -> &str {
        &self.class
    }

    /// Still inside the record's `FadeTime`?
    pub fn alive(&self, tick: u64) -> bool {
        tick.saturating_sub(self.born) < LIFE_TICKS
    }

    /// Draw `k` of this arc's seed stream (0-based).
    fn draw(&self, k: u32) -> u32 {
        let mut rng = Lcg(self.stream);
        let mut s = rng.step();
        for _ in 0..k {
            s = rng.step();
        }
        s
    }

    /// The two fresh draws a child strip takes this view (the PS2 redraws
    /// them every frame; the port ties them to the tick).
    fn child_draws(&self, tick: u64, child: u32) -> (u32, u32) {
        let t = tick as u32 ^ (tick >> 32) as u32;
        let mut rng = Lcg(self.stream
            ^ t.wrapping_mul(0x9e37_79b9)
            ^ self.link.wrapping_mul(0x85eb_ca6b)
            ^ child.wrapping_mul(0xc2b2_ae35));
        (rng.step(), rng.step())
    }

    /// This view's draws. `texture` is the class's sheet; with none the
    /// strip draws nothing (the particle path's missing-sheet rule).
    pub fn draws(
        &self,
        tick: u64,
        camera: &Camera,
        texture: Option<TextureHandle>,
        out: &mut Vec<ParticleDraw>,
    ) {
        let age = tick.saturating_sub(self.born);
        if age >= LIFE_TICKS {
            return;
        }
        let t = age as f32 * core::TICK_DT;
        let fade = (2.0 * (FADE_TIME - t) / FADE_TIME).clamp(0.0, 1.0);
        let phase_total = t * INTERPOLATION_SPEED;
        let step = phase_total as u32;
        let phase = phase_total - step as f32;
        let up = camera.right().cross(camera.forward()).normalize_or_zero();
        let mut view = View {
            eye: camera.position,
            up,
            texture,
            fade,
            key: self.from,
            out,
        };
        strip(
            self.from,
            self.to,
            Params { phase, ..MAIN },
            self.draw(step),
            self.draw(step + 1),
            &mut view,
        );
        for child in 0..NUM_CHILDREN {
            let (a, b) = self.child_draws(tick, child as u32);
            strip(self.from, self.to, CHILD, a, b, &mut view);
        }
    }
}

fn strip(from: Vec3, to: Vec3, params: Params, seed_a: u32, seed_b: u32, view: &mut View<'_>) {
    let Some(shape) = shape(from, to, params, seed_a, seed_b) else {
        return;
    };
    let len = (to - from).length();
    let seg = len / (shape.pts.len() - 1) as f32;
    let tris = triangles(
        &shape.pts,
        seg,
        params.texture_size,
        params.color,
        view.fade,
        view.eye,
        view.up,
    );
    view.out.push(ParticleDraw {
        shape: ParticleShape::Shaded { tris },
        key: view.key,
        color: [255; 4],
        texture: view.texture,
        additive: true,
    });
    // The PS2 draws the branches last-first.
    for b in shape.branches.into_iter().rev() {
        strip(
            b.point,
            b.point + b.dir * BRANCH_LENGTH,
            Params {
                width: b.width,
                phase: 0.0,
                branches: false,
                ..params
            },
            b.seed_a,
            b.seed_b,
            view,
        );
    }
}

/// The view-dependent data one arc's strips share.
struct View<'a> {
    eye: Vec3,
    up: Vec3,
    texture: Option<TextureHandle>,
    fade: f32,
    key: Vec3,
    out: &'a mut Vec<ParticleDraw>,
}

/// The PS2 `strip` (`FUN_00459c50` / B1 `FUN_00376a98`): the points and the
/// branches it grew. `None` for a degenerate or over-budget strip.
fn shape(from: Vec3, to: Vec3, p: Params, seed_a: u32, seed_b: u32) -> Option<Shape> {
    let len = (to - from).length();
    if len < 1e-3 {
        return None;
    }
    let d = (to - from) / len;
    let n = ((len / p.break_distance) as i32).max(3);
    if n > MAX_SEGMENTS {
        return None;
    }
    let n = n as usize;
    let seg = len / n as f32;
    let mut a = Lcg(seed_a ^ SEED_XOR);
    let mut b = Lcg(seed_b ^ SEED_XOR);
    let mut pts = Vec::with_capacity(n + 1);
    let mut branches = Vec::new();
    let mut w = p.width;
    // Both ends fade out; every inner point carries the record's colour.
    pts.push((from, w, 0));
    for i in 0..n - 1 {
        let c = from + d * (seg * (i + 1) as f32);
        let mut oa = Vec3::new(
            dither(&mut a, p.spread),
            dither(&mut a, p.spread),
            dither(&mut a, p.spread),
        );
        oa -= d * oa.dot(d);
        let mut ob = Vec3::new(
            dither(&mut b, p.spread),
            dither(&mut b, p.spread),
            dither(&mut b, p.spread),
        );
        ob -= d * ob.dot(d);
        let off = oa + (ob - oa) * p.phase;
        let point = c + off;
        pts.push((point, w, p.color[3]));
        let roll = a.float();
        // The B stream advances in step even when no branch is taken.
        let _ = b.float();
        if p.branches && roll < BRANCH_FACTOR && branches.len() < MAX_BRANCHES {
            let dir = (off + d * BRANCH_SPREAD_FACTOR).normalize_or_zero();
            if dir.length_squared() > 0.0 {
                branches.push(Branch {
                    point,
                    width: w,
                    dir,
                    seed_a: seed_a.wrapping_add(i as u32 + 1),
                    seed_b: seed_b.wrapping_add(i as u32 + 1),
                });
            }
        }
        w -= p.width / (n as f32 + 1.0);
    }
    pts.push((to, w.fmax(0.0), 0));
    Some(Shape { pts, branches })
}

fn dither(rng: &mut Lcg, spread: f32) -> f32 {
    (rng.float() - 0.5) * spread
}

/// The PS2 ribbon's triangles for this eye: `P ± side` per point, with the
/// point's own alpha and the axis U in `TextureSize` units.
fn triangles(
    pts: &[(Vec3, f32, u8)],
    seg: f32,
    texture_size: f32,
    color: [u8; 4],
    fade: f32,
    eye: Vec3,
    up: Vec3,
) -> Vec<(Vec3, [f32; 2], [u8; 4])> {
    let n = pts.len();
    let sides: Vec<Vec3> = (0..n)
        .map(|i| {
            let p = pts[i].0;
            let prev = if i > 0 { pts[i - 1].0 } else { p };
            let next = if i + 1 < n { pts[i + 1].0 } else { p };
            let mut side = ((p - prev) + (next - p)).cross(p - eye);
            if side.length_squared() > 1e-12 {
                side = side.normalize();
            } else {
                side = up;
            }
            side * pts[i].1
        })
        .collect();
    let tex = texture_size.fmax(1e-6);
    let rgb = [color[0], color[1], color[2]];
    let alpha = |i: usize| -> u8 { (f32::from(pts[i].2) * fade) as u8 };
    let mut tris = Vec::with_capacity((n - 1) * 6);
    for i in 1..n {
        let (p0, _, _) = pts[i - 1];
        let (p1, _, _) = pts[i];
        let u0 = (i - 1) as f32 * seg / tex;
        let u1 = i as f32 * seg / tex;
        let c0 = [rgb[0], rgb[1], rgb[2], alpha(i - 1)];
        let c1 = [rgb[0], rgb[1], rgb[2], alpha(i)];
        let v = [
            (p0 + sides[i - 1], [u0, 0.0], c0),
            (p0 - sides[i - 1], [u0, 1.0], c0),
            (p1 + sides[i], [u1, 0.0], c1),
            (p1 - sides[i], [u1, 1.0], c1),
        ];
        tris.extend([v[0], v[1], v[2], v[2], v[1], v[3]]);
    }
    tris
}

/// The two arc classes the discs author (`data` is the classdb bake; see the
/// module doc). Used to preload the sheet at the battle's load.
pub const TEXTURES: [&str; 2] = ["arccaster_lightning", "com_sfx_lightning"];

#[cfg(test)]
mod tests {
    use super::*;

    fn arc() -> Arc {
        Arc::new(
            Vec3::ZERO,
            Vec3::new(6.0, 0.0, 0.0),
            "arccaster_lightning",
            10,
            0,
        )
    }

    fn draws(arc: &Arc, tick: u64) -> Vec<ParticleDraw> {
        let mut out = Vec::new();
        // The real view passes the class's sheet; its handle is opaque
        // outside `render`, so the unit test only checks what it is handed.
        arc.draws(tick, &Camera::default(), None, &mut out);
        out
    }

    fn tris(d: &ParticleDraw) -> &[(Vec3, [f32; 2], [u8; 4])] {
        match &d.shape {
            ParticleShape::Shaded { tris } => tris,
            other => panic!("not a shaded strip: {other:?}"),
        }
    }

    /// `n = int(len / BreakDistance)` segments, `n - 1` inner points, the
    /// half-width tapering by Width/(n+1); both ends alpha 0, the inside the
    /// record's colour.
    #[test]
    fn a_strip_taper_and_fades_at_its_ends() {
        let p = Params {
            branches: false,
            ..MAIN
        };
        let s = shape(Vec3::ZERO, Vec3::new(6.0, 0.0, 0.0), p, 1, 2).unwrap();
        assert_eq!(s.pts.len(), 7, "six segments");
        assert_eq!(s.pts[0].1, WIDTH);
        assert_eq!(s.pts[1].1, WIDTH, "the first inner point keeps the full W");
        assert!((s.pts[2].1 - (WIDTH - WIDTH / 7.0)).abs() < 1e-6);
        assert!((s.pts[6].1 - 2.0 * WIDTH / 7.0).abs() < 1e-6);
        assert_eq!(s.pts[0].2, 0);
        assert_eq!(s.pts[6].2, 0);
        assert!(s.pts[1..6].iter().all(|p| p.2 == COLOR[3]));
        // The jitter is perpendicular to the axis and inside ±Spread/2.
        for (i, (point, _, _)) in s.pts[1..6].iter().enumerate() {
            assert!((point.x - (i + 1) as f32).abs() < 1e-4);
            assert!(point.y.abs() <= SPREAD_FACTOR / 2.0 + 1e-6);
            assert!(point.z.abs() <= SPREAD_FACTOR / 2.0 + 1e-6);
        }
    }

    /// One draw per strip, all additive, keyed at the arc's origin — the
    /// shared particle list's `Shaded` mechanism.
    #[test]
    fn draws_are_additive_textured_shaded_strips() {
        let d = draws(&arc(), 10);
        assert!(!d.is_empty());
        assert!(d
            .iter()
            .all(|p| p.additive && p.texture.is_none() && p.key == Vec3::ZERO));
        let main = tris(&d[0]);
        assert_eq!(main.len(), 6 * 6, "six segments, two triangles each");
        assert_eq!(main[0].2[3], 0, "the start fades out");
        assert_eq!(main[2].2[3], COLOR[3]);
        assert_eq!(main.last().unwrap().2[3], 0, "the end fades out");
        assert_eq!(main[0].2[..3], COLOR[..3]);
    }

    /// The record's `FadeTime` is 18 ticks: full for the first nine, then
    /// linear to 0; the 19th draws nothing.
    #[test]
    fn an_arc_lives_eighteen_ticks_and_fades_out() {
        let a = arc();
        assert!(a.alive(10));
        assert!(a.alive(27));
        assert!(!a.alive(28));
        let alpha = |tick| tris(&draws(&a, tick)[0])[2].2[3];
        assert_eq!(alpha(10), COLOR[3]);
        assert_eq!(alpha(19), COLOR[3], "0.3 s: the fade starts");
        let half = alpha(24);
        assert!(half > 0 && half < COLOR[3], "{half}");
        assert!(draws(&a, 28).is_empty());
    }

    /// The same (tick, link) draws the same arc; another tick or link draws
    /// another shape.
    #[test]
    fn the_shape_is_a_function_of_tick_and_link() {
        let a = arc();
        let same = Arc::new(
            Vec3::ZERO,
            Vec3::new(6.0, 0.0, 0.0),
            "arccaster_lightning",
            10,
            0,
        );
        assert_eq!(draws(&a, 12), draws(&same, 12));
        let other_link = Arc::new(
            Vec3::ZERO,
            Vec3::new(6.0, 0.0, 0.0),
            "arccaster_lightning",
            10,
            1,
        );
        assert_ne!(draws(&a, 12), draws(&other_link, 12));
        let other_tick = Arc::new(
            Vec3::ZERO,
            Vec3::new(6.0, 0.0, 0.0),
            "arccaster_lightning",
            11,
            0,
        );
        assert_ne!(draws(&a, 12), draws(&other_tick, 13));
    }

    /// The record's caps: at most three branches, and a strip over 100
    /// segments is dropped whole.
    #[test]
    fn branches_are_capped_and_a_long_strip_is_dropped() {
        let mut saw = false;
        for seed in 0..64 {
            let s = shape(
                Vec3::ZERO,
                Vec3::new(30.0, 0.0, 0.0),
                MAIN,
                seed,
                seed ^ 0x55aa_55aa,
            )
            .unwrap();
            saw |= !s.branches.is_empty();
            assert!(s.branches.len() <= MAX_BRANCHES);
        }
        assert!(saw, "no seed in 64 grew a branch");
        assert!(shape(Vec3::ZERO, Vec3::new(101.0, 0.0, 0.0), MAIN, 1, 2).is_none());
    }
}
