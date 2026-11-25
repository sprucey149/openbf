//! Weather: the map's precipitation and its wind (card WTHR,
//! `analysis/re-weather.md` §2–§6, [O]) — rain on both Kaminos and BF1 Yavin
//! 2, snow on the Hoths and BF1 Rhen Var, ash on BF2 Mygeeto, embers on BF2
//! Mustafar, pollen on BF2 Felucia.
//!
//! The PS2's rule, kept whole:
//! - **N particles live in ONE unit cube**, tiled over the world in 5 m
//!   cells: a particle at unit position `u` stands at `(cell + u)·5 m` in
//!   every cell near the camera ([`TILE`]). Each falls at `Velocity` plus the
//!   wind plus its own offset; twice a second every offset, alpha and spin
//!   steps toward a new random extreme ([`Weather::update`]).
//! - **The wind** wanders: a glide to a new target near the base over
//!   `1/rate` s, then a hold of `0.5/rate` s ([`Wind`]). Nothing else reads it.
//! - **The cells** drawn are those of the view frustum's index box nearer
//!   than `Range` along the view axis, each with an alpha that fades with
//!   distance ([`cell_walk`]). With a `rainshadow…` region in view the walk
//!   carries a pseudo-distance instead: a cell overlapping a region is shut
//!   (dry) and the walk reaches 2.5 × further through shut cells — the rain
//!   stays out of halls and is still seen through their doorways.
//! - **A streak** is a line from the drop (its alpha) to where it was
//!   `StreakLength/30` s ago as the moving player sees it (alpha 0); its
//!   colour is `Color` applied twice ([`streak_rgb`]). **A flake** is a
//!   camera-facing textured square of edge 2.83 × ⌊`ParticleSize`·256⌋/256,
//!   spun by its phase, at its own alpha — it takes NO distance fade.
//! - **Splashes**: `GroundEffectsPerSec` per 0.65·spread² m² of the view's
//!   footprint, dropped onto the ground, none within 1 m of a rain shadow
//!   ([`Weather::splashes`]).
//!
//! Viewer-side only: its randoms are a stream of its own ([`Lcg`], never the
//! sim's) and it runs on the frame's dt, so the sim hash cannot move.
//!
//! Port choices (each one line in `FEEL_NOTES.md` where it is a look):
//! - a streak is the PS2's one pixel of 512 as an ANGLE (never under one
//!   output pixel), so the rain looks the same at any resolution;
//! - the 2-pixel distance `dpx` keeps the PS2's 4:3 horizontal ([`PS2_ASPECT`])
//!   whatever the window's shape, so a wide window does not dim the rain;
//! - which regions are "in view" and which rim cells are in the frustum are
//!   the port's own box tests (re-weather.md §11: read for shape only);
//! - the second writer of the "in view" flags (the splash loop's own box,
//!   §5) is left out: the flags are the last drawn view's.

use crate::fx::emitter::Lcg;
use core::fmath::FloatExt;
use glam::Vec3;
use manifest::{Manifest, ManifestPrecipitation, ManifestRegion, ManifestWind};
use render::{Camera, ParticleDraw, ParticleShape, Renderer, TextureHandle};
use std::path::{Path, PathBuf};

/// The cell's edge, metres (the object's +0x44: 5.0 on all nine maps, no key).
pub const TILE: f32 = 5.0;
/// The re-roll clock's rate and its threshold (+0x8c; `DAT_006013a0`).
const REROLL_RATE: f32 = 0.4;
const REROLL_AT: f32 = 0.2;
/// With a rain shadow in view the walk reaches this much further, and an open
/// cell costs this many steps (§4.2).
const SHADOW_REACH: f32 = 2.5;
/// Where the far fade starts, as a share of the far distance (§3.3).
const FADE_FROM: f32 = 0.66;
/// The PS2 frame's tan(fov_h/2) over tan(fov_v/2), from the walk states'
/// cameras (BF2 0.63707 / 0.488378; BF1's 0.4452 / 0.34131 is the same).
pub const PS2_ASPECT: f32 = 0.637_07 / 0.488_378;
/// The PS2 frame's width in pixels: a streak is one of them.
const PS2_WIDTH: f32 = 512.0;
/// The seed of a STILL's weather (the gallery's map sheets, `pose_render`):
/// the setup's particles with no time run, the same picture every run.
pub const STILL_SEED: u32 = 0x5eed_f00d;
/// Splashes tried per update at most (a long frame must not spiral).
const SPLASH_CAP: usize = 64;

/// The effect's numbers as the PS2 holds them.
#[derive(Debug, Clone, PartialEq)]
pub struct Precip {
    /// Lines (rain); else textured squares.
    pub streaks: bool,
    pub texture: Option<String>,
    pub particle_size: f32,
    pub range: f32,
    pub color: [u8; 4],
    pub alpha: [f32; 2],
    pub count: usize,
    /// Fall speed, m/s, down positive.
    pub velocity: f32,
    pub velocity_range: f32,
    /// `StreakLength` / 30: seconds of travel.
    pub streak_seconds: f32,
    pub cross_scale: f32,
    pub axial_scale: f32,
    pub rotation: f32,
    pub rotation_range: f32,
    pub ground_effect: Option<String>,
    /// Splashes a second per view (BF2 divides the key by the view count at
    /// parse, BF1 does not: [`Weather::splashes`]' `views`).
    pub ground_per_sec: f32,
    pub ground_spread: f32,
}

impl Precip {
    pub fn from_manifest(m: &ManifestPrecipitation) -> Precip {
        Precip {
            streaks: m.streaks,
            texture: m.texture.clone(),
            particle_size: m.particle_size,
            range: m.range,
            color: m.color,
            alpha: m.alpha_min_max,
            // (int)(density + density range), §2.2.
            count: (m.particle_density + m.particle_density_range).fmax(0.0) as usize,
            velocity: m.velocity,
            velocity_range: m.velocity_range,
            streak_seconds: m.streak_length * 0.033_333_335,
            cross_scale: m.camera_cross_velocity_scale,
            axial_scale: m.camera_axial_velocity_scale,
            rotation: m.rotation,
            rotation_range: m.rotation_range,
            ground_effect: m.ground_effect.clone(),
            ground_per_sec: m.ground_effects_per_sec,
            ground_spread: m.ground_effect_spread,
        }
    }

    /// The distance at which `ParticleSize` spans 2 pixels of the PS2's 512:
    /// `ParticleSize·128 / tan(fov_h/2)` (§3.3), on the PS2's 4:3 horizontal.
    pub fn dpx(&self, tan_v: f32) -> f32 {
        self.particle_size * 128.0 / (tan_v * PS2_ASPECT)
    }

    /// A flake's edge in metres: 2.83 × ⌊`ParticleSize`·256⌋/256 (§3.4).
    pub fn flake_edge(&self) -> f32 {
        2.0 * std::f32::consts::SQRT_2 * (self.particle_size * 256.0).floor() / 256.0
    }
}

/// The GS halving of a colour byte: `(c + 1) >> 1`, 128 = ×1.0.
fn gs_half(b: u8) -> u32 {
    (u32::from(b) + 1) >> 1
}

/// A streak's displayed colour bytes: `Color` applied twice — the vertex
/// byte times the cell's, ⌊h·h / 128⌋, shown untextured as is (§3.4).
/// Kamino's (216, 220, 228) draws as (91, 94, 101).
pub fn streak_rgb(color: [u8; 4]) -> [u8; 3] {
    [0, 1, 2].map(|i| {
        let h = gs_half(color[i]);
        (h * h / 128) as u8
    })
}

/// A streak head's blend factor: the particle's alpha byte times the cell's
/// (`Color`.a × the cell's alpha), both halved, ⌊·/128⌋ of 128 (§3.4).
pub fn streak_alpha(color_a: u8, particle: f32, cell: f32) -> f32 {
    let hv = gs_half((particle.clamp(0.0, 1.0) * 255.0) as u8);
    let hc = gs_half((f32::from(color_a) * cell.clamp(0.0, 1.0)) as u8);
    (hv * hc / 128) as f32 / 128.0
}

/// One particle of the cube.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// In the unit cube.
    pub pos: Vec3,
    /// Its own velocity offset, m/s.
    pub offset: Vec3,
    pub alpha: f32,
    /// Quads: turns per second, and the phase.
    pub spin: f32,
    pub phase: f32,
}

/// The wandering wind vector (§6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wind {
    pub base: Vec3,
    pub range: f32,
    pub rate: f32,
    pub clock: f32,
    pub from: Vec3,
    pub target: Vec3,
    pub current: Vec3,
}

impl Wind {
    /// No wind: every map but the Kaminos and the Hoths.
    pub fn still() -> Wind {
        Wind::new(Vec3::ZERO, 0.0, 0.0)
    }

    pub fn new(base: Vec3, range: f32, rate: f32) -> Wind {
        Wind {
            base,
            range,
            rate,
            clock: 0.0,
            from: base,
            target: base,
            current: base,
        }
    }

    pub fn from_manifest(m: &ManifestWind) -> Wind {
        Wind::new(
            Vec3::new(m.velocity[0], 0.0, m.velocity[1]),
            m.velocity_range,
            m.velocity_change_rate,
        )
    }

    /// One update (the PS2's 0x461ff8, read from the disassembly).
    pub fn update(&mut self, dt: f32, rng: &mut Lcg) {
        let t = self.clock + self.rate * dt;
        self.clock = t;
        if t < 1.0 {
            self.current = self.from + (self.target - self.from) * t;
            return;
        }
        self.from = self.target;
        self.current = self.target;
        if t > 1.5 {
            let (l, r) = (self.base.length(), self.range);
            let a = rng.float() * 2.0 - 1.0;
            let b = rng.float() * 2.0 - 1.0;
            self.target = if l <= 0.01 * r {
                self.base + Vec3::new(a, 0.0, b) * r
            } else {
                self.base
                    + self.base * (r / l) * a
                    + Vec3::new(self.base.z, 0.0, -self.base.x) * (r * r / ((r + l) * l)) * b
            };
            self.clock = 0.0;
        }
    }
}

/// A rain-shadow region's shape as the PS2's init sorts it
/// (`analysis/re-cp-capture.md` §5; the kinds of re-weather.md §4.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowKind {
    /// Kind 1.
    Sphere,
    /// Kind 2: a box with a real rotation (its rows are the axes).
    Box([Vec3; 3]),
    /// Kind 5: an axis-aligned box.
    AaBox,
    /// Kinds 6 / 7 / 8: a cylinder about world X / Y / Z; `size` is then
    /// (radius, half-height, radius).
    Cylinder(usize),
    /// Kinds 3, 4 and the rest: never overlaps a cell here (a tilted
    /// cylinder is false on the PS2; no disc authors a plane).
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub kind: ShadowKind,
    pub center: Vec3,
    pub size: Vec3,
    /// Set by the last drawn view ([`Weather::look`]).
    pub in_view: bool,
}

impl Shadow {
    /// A `regn` whose name begins with `rainshadow` (§4.1), sorted into its
    /// kind as the engine's region init does.
    pub fn from_region(r: &ManifestRegion) -> Option<Shadow> {
        if !r.name.to_ascii_lowercase().starts_with("rainshadow") {
            return None;
        }
        let rows = r.rotation.map(Vec3::from);
        let len = rows.map(Vec3::length);
        let size = Vec3::from(r.size) * Vec3::from(len);
        // The one world axis a row lies along, if it has exactly one
        // component over 0.02 (the box's rule; taken for the cylinder's
        // "single non-zero component" too — PROVISIONAL).
        let lone = |v: Vec3| {
            let big: Vec<usize> = (0..3).filter(|&i| v[i].abs() > 0.02).collect();
            (big.len() == 1).then(|| big[0])
        };
        let (kind, size) = match r.shape.to_ascii_lowercase().as_str() {
            "sphere" => (ShadowKind::Sphere, Vec3::splat(Vec3::from(r.size).length())),
            "box" => match (lone(rows[0]), lone(rows[1]), lone(rows[2])) {
                (Some(a), Some(b), Some(c)) => {
                    let mut s = Vec3::ZERO;
                    s[a] = size.x;
                    s[b] = size.y;
                    s[c] = size.z;
                    (ShadowKind::AaBox, s)
                }
                _ => (ShadowKind::Box(rows.map(Vec3::normalize_or_zero)), size),
            },
            "cylinder" => {
                let rad = (size.x * size.x + size.z * size.z).sqrt();
                let kind = lone(rows[1]).map_or(ShadowKind::Other, ShadowKind::Cylinder);
                (kind, Vec3::new(rad, size.y, rad))
            }
            _ => (ShadowKind::Other, size),
        };
        Some(Shadow {
            kind,
            center: Vec3::from(r.translation),
            size,
            in_view: false,
        })
    }

    /// Does the cell of half-edge `half` at `c` overlap the region — the
    /// PS2's `FUN_00537208`, quirks kept (§4.2): the upright cylinder's
    /// corner test squares a negative gap, and a ROTATED box is no overlap
    /// test at all (each side's projected half-size alone against the gap),
    /// true only for cells round the region's centre.
    pub fn overlaps_cell(&self, c: Vec3, half: f32) -> bool {
        let delta = c - self.center;
        let d = delta.abs();
        match self.kind {
            ShadowKind::AaBox => d.cmplt(Vec3::splat(half) + self.size).all(),
            ShadowKind::Cylinder(axis) => {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let r = self.size.x;
                if d[axis] >= half + self.size.y || d[u] >= half + r || d[v] >= half + r {
                    return false;
                }
                (d[u] < half && d[v] < half)
                    || (d[u] - half) * (d[u] - half) + (d[v] - half) * (d[v] - half) < r * r
            }
            ShadowKind::Box(rows) => {
                (0..3).all(|a| {
                    let reach: f32 = (0..3).map(|i| rows[i][a].abs() * self.size[i]).sum();
                    d[a] < reach
                }) && (0..3).all(|i| {
                    let reach = half * (rows[i].x.abs() + rows[i].y.abs() + rows[i].z.abs());
                    delta.dot(rows[i]).abs() < reach
                })
            }
            ShadowKind::Sphere => {
                let gap = (d - Vec3::splat(half)).max(Vec3::ZERO);
                gap.length_squared() < self.size.x * self.size.x
            }
            ShadowKind::Other => false,
        }
    }

    /// Does a sphere of radius `r` at `p` touch the region (the splash's
    /// test, §5; the shapes grown by `r` — [D], the PS2's `FUN_00536f10` was
    /// not read case by case).
    pub fn touches_sphere(&self, p: Vec3, r: f32) -> bool {
        let delta = p - self.center;
        let d = delta.abs();
        match self.kind {
            ShadowKind::AaBox => d.cmplt(self.size + Vec3::splat(r)).all(),
            ShadowKind::Cylinder(axis) => {
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let reach = self.size.x + r;
                d[axis] < self.size.y + r && d[u] * d[u] + d[v] * d[v] < reach * reach
            }
            ShadowKind::Box(rows) => (0..3).all(|i| delta.dot(rows[i]).abs() < self.size[i] + r),
            ShadowKind::Sphere => delta.length() < self.size.x + r,
            ShadowKind::Other => false,
        }
    }

    /// The half-extents of the region's world-axis bounding box.
    fn bounds(&self) -> Vec3 {
        match self.kind {
            ShadowKind::Cylinder(axis) => {
                let mut b = Vec3::splat(self.size.x);
                b[axis] = self.size.y;
                b
            }
            ShadowKind::Box(rows) => Vec3::from(std::array::from_fn::<f32, 3, _>(|a| {
                (0..3).map(|i| rows[i][a].abs() * self.size[i]).sum()
            })),
            _ => self.size,
        }
    }
}

/// The camera as the weather reads it: the PS2's rows (it looks down −`z`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub pos: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
    pub near: f32,
    pub zoom: f32,
    pub tan_h: f32,
    pub tan_v: f32,
}

impl View {
    /// The port's camera at a window of `aspect`. A zoomed view is a
    /// narrower `fov_y` here, so `zoom` stays 1 (the PS2's far gains
    /// near·(zoom − 1), under half a metre).
    pub fn from_camera(c: &Camera, aspect: f32) -> View {
        let (x, f) = (c.right(), c.forward());
        let (sin, cos) = core::fmath::sin_cos(c.fov_y * 0.5);
        let tan_v = sin / cos;
        View {
            pos: c.position,
            x,
            y: x.cross(f).normalize_or_zero(),
            z: -f,
            near: c.near,
            zoom: 1.0,
            tan_h: tan_v * aspect,
            tan_v,
        }
    }

    /// The world-axis bounds of the frustum between `near` and `far` (its
    /// apex and both planes' corners).
    fn bounds(&self, near: f32, far: f32) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = (self.pos, self.pos);
        for d in [near, far] {
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    let p = self.pos - self.z * d
                        + self.x * (sx * self.tan_h * d)
                        + self.y * (sy * self.tan_v * d);
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        (lo, hi)
    }

    /// Is the cube of half-edge `half` at `c` not wholly outside the frustum
    /// (its bounding sphere against each plane — the test the model scored
    /// against the PS2 frames used).
    fn sees_cell(&self, c: Vec3, half: f32, near: f32, far: f32) -> bool {
        let r = half * 3f32.sqrt();
        let v = c - self.pos;
        let d = -v.dot(self.z);
        if d < near - r || d > far + r {
            return false;
        }
        let side =
            |along: f32, tan: f32| along.abs() <= d.fmax(0.0) * tan + r * (1.0 + tan * tan).sqrt();
        side(v.dot(self.x), self.tan_h) && side(v.dot(self.y), self.tan_v)
    }

    /// Is the world-axis box (`c` ± `half`) not wholly outside one plane of
    /// the frustum.
    fn sees_box(&self, c: Vec3, half: Vec3, near: f32, far: f32) -> bool {
        // (inward normal, a point on the plane)
        let fwd = -self.z;
        let planes = [
            (fwd, self.pos + fwd * near),
            (-fwd, self.pos + fwd * far),
            ((fwd * self.tan_h + self.x).normalize_or_zero(), self.pos),
            ((fwd * self.tan_h - self.x).normalize_or_zero(), self.pos),
            ((fwd * self.tan_v + self.y).normalize_or_zero(), self.pos),
            ((fwd * self.tan_v - self.y).normalize_or_zero(), self.pos),
        ];
        planes.iter().all(|(n, p)| {
            // The box's farthest corner along the normal.
            (c - *p).dot(*n) + half.dot(n.abs()) >= 0.0
        })
    }
}

/// What one view draws: the cells (index, alpha) and the numbers behind them.
#[derive(Debug, Clone, PartialEq)]
pub struct CellWalk {
    /// The far distance used: `Range`, × 2.5 with a rain shadow in view.
    pub far: f32,
    pub fade_start: f32,
    pub dpx: f32,
    pub cells: Vec<([i32; 3], f32)>,
}

/// A cell's centre.
pub fn cell_centre(index: [i32; 3]) -> Vec3 {
    (Vec3::new(index[0] as f32, index[1] as f32, index[2] as f32) + 0.5) * TILE
}

/// The cells one view draws and each one's alpha (§3.3; with a region
/// flagged `in_view`, §4.2's walk).
pub fn cell_walk(p: &Precip, view: &View, shadows: &[Shadow]) -> CellWalk {
    let far0 = p.range + view.near * (view.zoom - 1.0);
    let active = shadows.iter().any(|s| s.in_view);
    let far = if active { far0 * SHADOW_REACH } else { far0 };
    let fade_start = far * FADE_FROM;
    let dpx = p.dpx(view.tan_v);
    let half = TILE * 0.5;
    let index = |v: Vec3| (v / TILE).floor().as_ivec3();
    let mut cells = Vec::new();
    if !active {
        let (lo, hi) = view.bounds(view.near * view.zoom, far);
        let (lo, hi) = (index(lo), index(hi));
        for ix in lo.x..=hi.x {
            for iy in lo.y..=hi.y {
                for iz in lo.z..=hi.z {
                    let i = [ix, iy, iz];
                    let c = cell_centre(i);
                    let d = (view.pos - c).dot(view.z);
                    if d >= far || !view.sees_cell(c, half, view.near * view.zoom, far) {
                        continue;
                    }
                    let mut a = if d > dpx { dpx / d } else { 1.0 };
                    if d > fade_start {
                        a *= (p.range - d) / (far - fade_start);
                    }
                    if a > 0.0 {
                        cells.push((i, a));
                    }
                }
            }
        }
        return CellWalk {
            far,
            fade_start,
            dpx,
            cells,
        };
    }
    // The rain-shadow walk: near side first on each axis, a pseudo-distance
    // carried from cell to cell — an open step costs 2.5, a shut one 1.
    let (lo, hi) = view.bounds(view.near, far);
    let (lo, hi) = (index(lo), index(hi));
    let axis = |lo: i32, hi: i32, z: f32| -> Vec<i32> {
        if z > 0.0 {
            (lo..=hi).rev().collect()
        } else {
            (lo..=hi).collect()
        }
    };
    let xs = axis(lo.x, hi.x, view.z.x);
    let ys = axis(lo.y, hi.y, view.z.y);
    let zs = axis(lo.z, hi.z, view.z.z);
    let step = view.z.abs() * TILE;
    let d0 = (view.pos - cell_centre([xs[0], ys[0], zs[0]])).dot(view.z);
    for (jy, &iy) in ys.iter().enumerate() {
        let dy = d0 + jy as f32 * step.y;
        // (v, open) of the row before, per z step.
        let mut up_row: Vec<(f32, bool)> = (0..zs.len())
            .map(|k| (dy + SHADOW_REACH * (k as f32 * step.z - step.x), true))
            .collect();
        for (jx, &ix) in xs.iter().enumerate() {
            let mut prev = (dy + SHADOW_REACH * (jx as f32 * step.x - step.z), true);
            let mut any = false;
            let mut row = Vec::with_capacity(zs.len());
            for (k, &iz) in zs.iter().enumerate() {
                let up = up_row[k];
                let v = match (prev.1, up.1) {
                    (true, true) => {
                        (up.0 + SHADOW_REACH * step.x).fmax(prev.0 + SHADOW_REACH * step.z)
                    }
                    (true, false) => up.0 + step.x,
                    (false, true) => prev.0 + step.z,
                    (false, false) => (up.0 + step.x).fmin(prev.0 + step.z),
                };
                let mut open = true;
                if v < far {
                    any = true;
                    let i = [ix, iy, iz];
                    let c = cell_centre(i);
                    if view.sees_cell(c, half, view.near, far) {
                        if shadows
                            .iter()
                            .any(|s| s.in_view && s.overlaps_cell(c, half))
                        {
                            open = false;
                        } else {
                            let mut a = if v > dpx { dpx / v } else { 1.0 };
                            if v > fade_start {
                                a *= (far - v) / (far - fade_start);
                            }
                            if a > 0.0 {
                                cells.push((i, a));
                            }
                        }
                    }
                }
                row.push((v, open));
                prev = (v, open);
            }
            up_row = row;
            if !any {
                break;
            }
        }
    }
    CellWalk {
        far,
        fade_start,
        dpx,
        cells,
    }
}

/// The streak's common vector, head to tail: −(fall − camvel)·S, shortened to
/// one tile (§3.1). `camvel` is the velocity of what the camera tracks.
pub fn streak_vector(p: &Precip, wind: Vec3, camvel: Vec3, view: &View) -> Vec3 {
    let camvel = if p.cross_scale != 1.0 || p.axial_scale != 1.0 {
        view.x * (camvel.dot(view.x) * p.cross_scale)
            + view.y * (camvel.dot(view.y) * p.cross_scale)
            + view.z * (camvel.dot(view.z) * p.axial_scale)
    } else {
        camvel
    };
    let fall = Vec3::new(wind.x, -p.velocity, wind.z);
    let s = -(fall - camvel) * p.streak_seconds;
    let l = s.length();
    if l > TILE {
        s * (TILE / l)
    } else {
        s
    }
}

/// The footprint box the splashes fall in: the world-axis bounds of the
/// camera and the four far corners of a 60° × 46.8° view out to spread + 3 m
/// (§5) — (min, max).
pub fn splash_box(spread: f32, view: &View) -> (Vec3, Vec3) {
    let d = spread + 3.0;
    let (mut lo, mut hi) = (view.pos, view.pos);
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            let p = view.pos - view.z * d + view.x * (sx * 0.577 * d) + view.y * (sy * 0.433 * d);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    (lo, hi)
}

/// Seconds between two splashes in a footprint of `dx` × `dz` metres:
/// 0.65·spread² / (rate·dx·dz) (§5).
pub fn splash_interval(spread: f32, rate: f32, dx: f32, dz: f32) -> f32 {
    0.65 * spread * spread / (rate * dx * dz)
}

/// A map's live weather.
#[derive(Debug, Clone)]
pub struct Weather {
    pub precip: Precip,
    pub particles: Vec<Particle>,
    pub wind: Wind,
    pub shadows: Vec<Shadow>,
    /// The re-roll clock.
    clock: f32,
    splash_timer: f32,
    /// The split views' own splash clocks, by local player index ([`Weather::view_splashes`]);
    /// the single view keeps `splash_timer`.
    view_timers: Vec<f32>,
    rng: Lcg,
    /// Where the flake's sheet is looked for, map first; and the sheet once
    /// asked for (`Some(None)`: in no layer).
    texture_dirs: Vec<PathBuf>,
    sheet: Option<Option<TextureHandle>>,
}

impl Weather {
    /// The map's weather, `None` on a map with no precipitation. `seed`
    /// starts the module's own random stream.
    pub fn from_manifest(m: &Manifest, seed: u32) -> Option<Weather> {
        let precip = Precip::from_manifest(m.precipitation.as_ref()?);
        let wind = m.wind.as_ref().map_or(Wind::still(), Wind::from_manifest);
        let shadows = m.regions.iter().filter_map(Shadow::from_region).collect();
        Some(Weather::new(precip, wind, shadows, seed))
    }

    /// The setup (§2.2): each particle's position, velocity offset and
    /// alpha; a Quad's spin rate and phase.
    pub fn new(precip: Precip, wind: Wind, shadows: Vec<Shadow>, seed: u32) -> Weather {
        let mut rng = Lcg(seed);
        let particles = (0..precip.count)
            .map(|_| {
                let pos = Vec3::new(rng.float(), rng.float(), rng.float());
                let mut o = || (rng.float() - 0.5) * 2.0 * precip.velocity_range;
                let offset = Vec3::new(o(), o(), o());
                let alpha = precip.alpha[0] + (precip.alpha[1] - precip.alpha[0]) * rng.float();
                let (spin, phase) = if precip.streaks {
                    (0.0, 0.0)
                } else {
                    (
                        precip.rotation + precip.rotation_range * (rng.float() - 0.5),
                        rng.float(),
                    )
                };
                Particle {
                    pos,
                    offset,
                    alpha,
                    spin,
                    phase,
                }
            })
            .collect();
        Weather {
            precip,
            particles,
            wind,
            shadows,
            clock: 0.0,
            splash_timer: 0.0,
            view_timers: Vec::new(),
            rng,
            texture_dirs: Vec::new(),
            sheet: None,
        }
    }

    /// [`Weather::from_manifest`] for the map baked at `map_dir`: the
    /// flake's sheet resolves map → common (`fx_snow` is in COMMON.LVL /
    /// INGAME.LVL, the others in their map's LVL).
    pub fn load(m: &Manifest, assets_dir: &Path, map_dir: &Path, seed: u32) -> Option<Weather> {
        let mut w = Weather::from_manifest(m, seed)?;
        let common = format!("common_{}", m.source_game.to_ascii_lowercase());
        w.texture_dirs = vec![
            map_dir.join("textures"),
            assets_dir.join(common).join("textures"),
        ];
        Some(w)
    }

    /// The flake's sheet, uploaded on the first call (rain has none).
    pub fn sheet(&mut self, r: &mut Renderer) -> Option<TextureHandle> {
        if let Some(t) = self.sheet {
            return t;
        }
        let t = self.precip.texture.clone().and_then(|name| {
            let name = name.to_ascii_lowercase();
            let t = self.texture_dirs.iter().find_map(|d| {
                crate::viewer::upload_texture_file(r, d, &name).unwrap_or_else(|e| {
                    eprintln!("weather texture {name}: {e:#}");
                    None
                })
            });
            if t.is_none() {
                eprintln!("weather texture {name}: in no layer (the flakes draw as nothing)");
            }
            t
        });
        self.sheet = Some(t);
        t
    }

    /// Hand the renderer a still view's weather — the map's precipitation at
    /// its setup state ([`STILL_SEED`]) and nothing else in the particle
    /// list; no particles on a map without any.
    pub fn submit_still(
        r: &mut Renderer,
        m: &Manifest,
        assets_dir: &Path,
        map_dir: &Path,
        camera: &Camera,
    ) {
        let draws = Weather::load(m, assets_dir, map_dir, STILL_SEED)
            .map(|mut w| w.frame_draws(r, camera, Vec3::ZERO))
            .unwrap_or_default();
        r.set_particles(draws);
    }

    /// [`Weather::draws`] for the renderer's own frame: its aspect, its
    /// height and the flake's sheet.
    pub fn frame_draws(
        &mut self,
        r: &mut Renderer,
        camera: &Camera,
        camvel: Vec3,
    ) -> Vec<ParticleDraw> {
        let (w, h) = r.size();
        self.frame_draws_sized(r, camera, camvel, [w, h])
    }

    /// [`Weather::frame_draws`] for a view of `size` pixels (issue #194 B13: a splitscreen
    /// cell, whose aspect and height are not the window's). Its [`Weather::look`] flags the rain
    /// shadows that view sees.
    pub fn frame_draws_sized(
        &mut self,
        r: &mut Renderer,
        camera: &Camera,
        camvel: Vec3,
        size: [u32; 2],
    ) -> Vec<ParticleDraw> {
        let [w, h] = size;
        let view = View::from_camera(camera, w as f32 / h.max(1) as f32);
        let sheet = self.sheet(r);
        self.draws(&view, camvel, sheet, h as f32)
    }

    /// One frame (§2.3, §6): the wind, the re-roll clock, the fall.
    pub fn update(&mut self, dt: f32) {
        self.wind.update(dt, &mut self.rng);
        let p = &self.precip;
        self.clock += REROLL_RATE * dt;
        if self.clock > REROLL_AT {
            // Every 0.5 s each particle steps toward a new random extreme.
            let c = self.clock.fmin(1.0);
            let rng = &mut self.rng;
            let mut sign = || {
                if rng.step() & 0x8000_0000 != 0 {
                    1.0
                } else {
                    -1.0
                }
            };
            for q in &mut self.particles {
                let to = Vec3::new(sign(), sign(), sign()) * p.velocity_range;
                q.offset += (to - q.offset) * c;
                let a = if sign() > 0.0 { p.alpha[1] } else { p.alpha[0] };
                q.alpha += (a - q.alpha) * c;
                if !p.streaks {
                    let s = p.rotation + sign() * p.rotation_range * 0.5;
                    q.spin += (s - q.spin) * c;
                }
            }
            self.clock = c - REROLL_AT;
        }
        let fall = Vec3::new(self.wind.current.x, -p.velocity, self.wind.current.z);
        for q in &mut self.particles {
            let moved = q.pos + (fall + q.offset) * (dt / TILE);
            q.pos = moved - moved.floor();
            if !p.streaks {
                let ph = q.phase + q.spin * dt;
                q.phase = ph - ph.trunc();
            }
        }
    }

    /// Flag the rain shadows this view sees (their bounding boxes against
    /// the view's frustum out to `Range`) and walk the cells.
    pub fn look(&mut self, view: &View) -> CellWalk {
        let far = self.precip.range + view.near * (view.zoom - 1.0);
        for s in &mut self.shadows {
            s.in_view = view.sees_box(s.center, s.bounds(), view.near * view.zoom, far);
        }
        cell_walk(&self.precip, view, &self.shadows)
    }

    /// This frame's draws for one view: per drawn cell one batch of streaks,
    /// or one sprite per flake. `camvel` is the velocity of what the camera
    /// tracks; `texture` the flake's sheet; `height_px` the output's height
    /// (a streak is never thinner than one pixel of it).
    pub fn draws(
        &mut self,
        view: &View,
        camvel: Vec3,
        texture: Option<TextureHandle>,
        height_px: f32,
    ) -> Vec<ParticleDraw> {
        let walk = self.look(view);
        let p = &self.precip;
        let mut out = Vec::new();
        if p.streaks {
            let streak = streak_vector(p, self.wind.current, camvel, view);
            let rgb = streak_rgb(p.color).map(|b| f32::from(b) / 255.0);
            // The PS2's pixel as an angle; at least one of the output's.
            let width = (2.0 * view.tan_v * PS2_ASPECT / PS2_WIDTH)
                .fmax(2.0 * view.tan_v / height_px.fmax(1.0));
            for (index, cell_alpha) in &walk.cells {
                let c = cell_centre(*index);
                let lines = self
                    .particles
                    .iter()
                    .map(|q| {
                        let head = c + (q.pos - 0.5) * TILE;
                        let tail = head + streak - q.offset * p.streak_seconds;
                        (head, tail, streak_alpha(p.color[3], q.alpha, *cell_alpha))
                    })
                    .collect();
                out.push(ParticleDraw {
                    shape: ParticleShape::Streaks { lines, rgb, width },
                    key: c,
                    color: p.color,
                    texture: None,
                    additive: false,
                });
            }
            return out;
        }
        // A flake takes neither the cell's alpha nor `Color`.a (§3.4): its
        // own alpha in every drawn cell.
        let half = p.flake_edge() * 0.5;
        for (index, _) in &walk.cells {
            let c = cell_centre(*index);
            for q in &self.particles {
                let pos = c + (q.pos - 0.5) * TILE;
                out.push(ParticleDraw {
                    shape: ParticleShape::Sprite {
                        pos,
                        half,
                        angle: std::f32::consts::PI * (2.0 * q.phase - 1.0),
                        uv: [0.0, 0.0, 1.0, 1.0],
                    },
                    key: pos,
                    color: [
                        p.color[0],
                        p.color[1],
                        p.color[2],
                        (q.alpha.clamp(0.0, 1.0) * 255.0) as u8,
                    ],
                    texture,
                    additive: false,
                });
            }
        }
        out
    }

    /// The splashes of one frame for several views (issue #194 B13, splitscreen): for each view
    /// in order, its [`Weather::look`] and then its [`Weather::splashes`] with the rate divided
    /// among the views, so each view's splashes read that view's own rain shadows (never the
    /// last view's). Each view is keyed by its local player index and runs on its own splash
    /// clock against its own interval, so a view whose footprint shrinks (looking down) still
    /// gets its share rather than having the others drain a shared clock. `ground` as for
    /// [`Weather::splashes`]. The single view keeps calling [`Weather::splashes`] alone, on the
    /// one-view clock, reading its last frame's look.
    pub fn view_splashes(
        &mut self,
        dt: f32,
        views: &[(usize, View)],
        ground: &mut dyn FnMut(Vec3) -> Option<Vec3>,
    ) -> Vec<Vec3> {
        let n = u32::try_from(views.len()).unwrap_or(u32::MAX);
        let mut out = Vec::new();
        for (slot, view) in views {
            if self.view_timers.len() <= *slot {
                self.view_timers.resize(slot + 1, 0.0);
            }
            std::mem::swap(&mut self.splash_timer, &mut self.view_timers[*slot]);
            self.look(view);
            out.extend(self.splashes(dt, view, n, ground));
            std::mem::swap(&mut self.splash_timer, &mut self.view_timers[*slot]);
        }
        out
    }

    /// The splashes of one frame (§5): where each lands. `views` is BF2's
    /// divisor of the rate (1 in BF1 whatever the split); `ground` drops a
    /// point given at the ray's top (50 m over the camera) straight down
    /// onto terrain or buildings. The rain shadows read are the last
    /// [`Weather::look`]'s.
    pub fn splashes(
        &mut self,
        dt: f32,
        view: &View,
        views: u32,
        ground: &mut dyn FnMut(Vec3) -> Option<Vec3>,
    ) -> Vec<Vec3> {
        let p = &self.precip;
        let mut out = Vec::new();
        if p.ground_effect.is_none() {
            return out;
        }
        let (lo, hi) = splash_box(p.ground_spread, view);
        let rate = p.ground_per_sec / views.max(1) as f32;
        let interval = splash_interval(p.ground_spread, rate, hi.x - lo.x, hi.z - lo.z);
        if !(interval > 0.0 && interval.is_finite()) {
            return out;
        }
        self.splash_timer += dt;
        let mut tries = 0;
        while self.splash_timer > interval {
            self.splash_timer -= interval;
            tries += 1;
            if tries > SPLASH_CAP {
                self.splash_timer %= interval;
                break;
            }
            let x = lo.x + (hi.x - lo.x) * self.rng.float();
            let z = lo.z + (hi.z - lo.z) * self.rng.float();
            let Some(hit) = ground(Vec3::new(x, view.pos.y + 50.0, z)) else {
                continue;
            };
            if self
                .shadows
                .iter()
                .any(|s| s.in_view && s.touches_sphere(hit, 1.0))
            {
                continue;
            }
            out.push(hit);
        }
        out
    }
}

#[cfg(test)]
mod split_tests {
    //! Issue #194 B13: rain per splitscreen view.
    use super::*;

    fn rain() -> Precip {
        Precip {
            streaks: true,
            texture: None,
            particle_size: 0.02,
            range: 30.0,
            color: [180, 180, 200, 255],
            alpha: [0.0, 1.0],
            count: 16,
            velocity: 20.0,
            velocity_range: 1.0,
            streak_seconds: 0.1,
            cross_scale: 1.0,
            axial_scale: 1.0,
            rotation: 0.0,
            rotation_range: 0.0,
            ground_effect: Some("com_sfx_rainsplash".into()),
            ground_per_sec: 7.0,
            ground_spread: 8.0,
        }
    }

    /// A level view 10 m up at `x`, looking down -z.
    fn level(x: f32) -> View {
        View {
            pos: Vec3::new(x, 10.0, 0.0),
            x: Vec3::X,
            y: Vec3::Y,
            z: Vec3::Z,
            near: 0.5,
            zoom: 1.0,
            tan_h: 0.637,
            tan_v: 0.488,
        }
    }

    /// A roof over the far half of the view at x = 0's splash footprint (z < -6.5).
    fn roof() -> Shadow {
        Shadow {
            kind: ShadowKind::AaBox,
            center: Vec3::new(0.0, 0.0, -11.0),
            size: Vec3::new(50.0, 20.0, 4.5),
            in_view: false,
        }
    }

    fn flat(top: Vec3) -> Option<Vec3> {
        Some(Vec3::new(top.x, 0.0, top.z))
    }

    fn under_roof(hits: &[Vec3]) -> usize {
        hits.iter()
            .filter(|h| h.x.abs() < 100.0 && h.z < -11.0 + 4.5 + 1.0 - 1e-4)
            .count()
    }

    /// P1 stands 1 km away and cannot see the roof; P2 stands by it. Each view looks right
    /// before its own splashes, so none of P2's lands under the roof, whatever P1 saw.
    #[test]
    fn each_views_splashes_read_its_own_look() {
        let (far, near) = (level(1000.0), level(0.0));
        let mut w = Weather::new(rain(), Wind::still(), vec![roof()], 9);
        w.look(&near);
        assert!(w.shadows[0].in_view, "P2 sees the roof");
        w.look(&far);
        assert!(!w.shadows[0].in_view, "P1 does not");
        let mut landed = Vec::new();
        for _ in 0..600 {
            landed.extend(w.view_splashes(1.0 / 60.0, &[(0, far), (1, near)], &mut flat));
        }
        let p2 = landed.iter().filter(|h| h.x.abs() < 100.0).count();
        let p1 = landed.len() - p2;
        assert!(p1 > 0 && p2 > 0, "{p1} {p2}");
        assert_eq!(under_roof(&landed), 0, "P2's splashes read P2's look");
        // The control: every look first leaves P1's flags for P2's splashes, and they fall
        // under the roof.
        let mut w = Weather::new(rain(), Wind::still(), vec![roof()], 9);
        let mut wrong = Vec::new();
        for _ in 0..600 {
            w.look(&near);
            w.look(&far);
            wrong.extend(w.splashes(1.0 / 60.0, &near, 2, &mut flat));
        }
        assert!(under_roof(&wrong) > 0, "the control discriminates");
    }

    /// Two views share one view's splash rate between them.
    #[test]
    fn two_views_share_one_rate() {
        let views = [(0, level(1000.0)), (1, level(-1000.0))];
        let mut one = Weather::new(rain(), Wind::still(), Vec::new(), 9);
        let mut two = Weather::new(rain(), Wind::still(), Vec::new(), 9);
        let (mut a, mut b) = (0usize, 0usize);
        for _ in 0..600 {
            a += one.splashes(1.0 / 60.0, &views[0].1, 1, &mut flat).len();
            b += two.view_splashes(1.0 / 60.0, &views, &mut flat).len();
        }
        assert!(a > 100);
        assert!((a as f32 - b as f32).abs() <= 3.0, "{a} {b}");
    }

    /// A view looking straight down at `x`, 10 m up: its splash footprint is shallower than a
    /// level view's, so its interval is longer.
    fn down(x: f32) -> View {
        View {
            pos: Vec3::new(x, 10.0, 0.0),
            x: Vec3::X,
            y: Vec3::NEG_Z,
            z: Vec3::Y,
            ..level(x)
        }
    }

    /// P1 looks down (longer interval), P2 looks level: each view's splashes come at its own
    /// footprint's rate, not drained by the other view through a shared clock.
    #[test]
    fn a_view_looking_down_keeps_its_share() {
        let (p1, p2) = (down(1000.0), level(-1000.0));
        let (dt, frames) = (1.0 / 240.0, 2400);
        let expect = |v: &View| {
            let p = rain();
            let (lo, hi) = splash_box(p.ground_spread, v);
            let i = splash_interval(
                p.ground_spread,
                p.ground_per_sec / 2.0,
                hi.x - lo.x,
                hi.z - lo.z,
            );
            dt * frames as f32 / i
        };
        let (e1, e2) = (expect(&p1), expect(&p2));
        assert!(e1 < e2, "down's footprint is smaller: {e1} {e2}");
        let mut w = Weather::new(rain(), Wind::still(), Vec::new(), 9);
        let mut landed = Vec::new();
        for _ in 0..frames {
            landed.extend(w.view_splashes(dt, &[(0, p1), (1, p2)], &mut flat));
        }
        let a = landed.iter().filter(|h| h.x > 0.0).count() as f32;
        let b = landed.len() as f32 - a;
        assert!(
            (a - e1).abs() <= 2.0 && (b - e2).abs() <= 2.0,
            "{a}/{e1} {b}/{e2}"
        );
        // The control: one clock shared by both views starves the one looking down.
        let mut w = Weather::new(rain(), Wind::still(), Vec::new(), 9);
        let mut shared = Vec::new();
        for _ in 0..frames {
            shared.extend(w.splashes(dt, &p1, 2, &mut flat));
            shared.extend(w.splashes(dt, &p2, 2, &mut flat));
        }
        let a = shared.iter().filter(|h| h.x > 0.0).count() as f32;
        assert!(a < e1 * 0.5, "the control discriminates: {a}/{e1}");
    }

    /// `frame_draws` is `frame_draws_sized` at the renderer's size, draw for draw.
    #[test]
    #[ignore = "requires a headless GPU adapter"]
    fn frame_draws_is_frame_draws_sized_at_the_renderer_size() {
        let mut r = render::Renderer::new_headless(640, 360).expect("headless GPU");
        let camera = Camera {
            position: Vec3::new(0.0, 10.0, 0.0),
            ..Camera::default()
        };
        let vel = Vec3::new(1.0, 0.0, 2.0);
        let mut a = Weather::new(rain(), Wind::still(), vec![roof()], 5);
        let mut b = Weather::new(rain(), Wind::still(), vec![roof()], 5);
        let da = a.frame_draws(&mut r, &camera, vel);
        let db = b.frame_draws_sized(&mut r, &camera, vel, [640, 360]);
        assert!(!da.is_empty());
        assert_eq!(da, db);
        assert_eq!(
            a.shadows.iter().map(|s| s.in_view).collect::<Vec<_>>(),
            b.shadows.iter().map(|s| s.in_view).collect::<Vec<_>>()
        );
        // A tall cell is not the window: other draws.
        let mut c = Weather::new(rain(), Wind::still(), vec![roof()], 5);
        assert_ne!(da, c.frame_draws_sized(&mut r, &camera, vel, [320, 360]));
    }
}
