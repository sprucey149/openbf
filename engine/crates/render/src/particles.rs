//! Particle effects (card FXP2, `analysis/re-fx.md` §6, [O]): the shapes the
//! PS2 draws a particle as, built per view, and the one far-to-near list the
//! particles share with the laser bolts (re-bolt §3.4: the PS2 sorts bolts
//! WITH the particle effects).
//!
//! The game layer (`game::fx`) runs the emitters and hands over each drawn
//! particle as a [`ParticleDraw`]: its shape in world units (the size
//! multiplier, the rotation mapping and the atlas cell already applied), its
//! vertex bytes as the PS2 computes them, its sheet and blend. This file
//! turns them into triangles for one camera:
//! - [`ParticleShape::Sprite`] — PARTICLE / ANIMATED: a square parallel to
//!   the screen, corners at `pos ± R(angle)·(±half, ±half)` in the camera's
//!   (right, up) plane, (0,0) at the top left (§6.1: VU1 expands it at the
//!   particle's depth);
//! - [`ParticleShape::Quad`] — BILLBOARD: the world-axis plane `pos ± A ± B`,
//!   (0,0) at `pos − A + B` (§6.4);
//! - [`ParticleShape::Mesh`] — a model already placed in the world (the
//!   muzzle flash, card FXMF), its triangles as given;
//! - [`ParticleShape::Ribbon`] — SPARK / STREAK: a camera-facing strip
//!   through points, each with its half-width and U; the side vector
//!   `normalize(((P − prev) + (next − P)) × (P − eye))`, camera up when that
//!   is zero (§6.5, B1 `FUN_0047df68`), `P + side` at V 0, `P − side` at V 1;
//! - [`ParticleShape::Beam`] — a command post hologram's column (card CPHOL):
//!   the "lightbeam" renderer's eye-facing fan, `crate::light_beam`.
//!
//! GS state (§6.7): NORMAL = ALPHA 0x44 (source-over), ADDITIVE = 0x48
//! (Cs·As + Cd); TEST 0x51001 — depth tested, never written, no alpha
//! cutout; PRIM without FGE — never fogged; vertex bytes on the GS scale
//! (`(b + 1) >> 1`, 128 = ×1.0, no overbright). They ride the bolts'
//! pipelines, which are exactly that.
//!
//! **Order.** Every item — a bolt, a particle — has a key point; the view
//! draws them farthest first (the PS2 heap-sorts its PARTICLE batches
//! back-to-front, §6.7). Items with the same distance keep their order, so
//! the game keys the unsorted types (every type but PARTICLE draws in spawn
//! order on the PS2) by their effect's origin. Runs of one sheet and blend
//! merge into one draw.

use crate::bolt::{gs_unit, BoltDraw, BoltFrame, BoltRun};
use crate::grass::GrassVertex;
use crate::{Camera, Renderer, TextureHandle};
use glam::Vec3;

mod trail;

/// A particle's shape in world units (see the module doc).
#[derive(Debug, Clone, PartialEq)]
pub enum ParticleShape {
    Sprite {
        pos: Vec3,
        /// Half the square's edge.
        half: f32,
        /// Counter-clockwise on screen, radians.
        angle: f32,
        /// The atlas cell: (u0, v0, u1, v1).
        uv: [f32; 4],
    },
    Quad {
        pos: Vec3,
        a: Vec3,
        b: Vec3,
    },
    Ribbon {
        /// (point, half-width, U).
        points: Vec<(Vec3, f32, f32)>,
    },
    /// Shared native modes 4/5: rounded head, tapered body and the
    /// nose-on mirrored cap, using the laser ribbon's UVs and alpha.
    CappedRibbon {
        head: Vec3,
        head_half: f32,
        tail: Vec3,
        tail_half: f32,
    },
    /// Swept blade sections: origin, tip, authored half-width and color.
    Trail {
        sections: Vec<(Vec3, Vec3, f32, [u8; 4])>,
        length: f32,
    },
    /// A model drawn as world-space triangles (three per triangle, its UVs):
    /// the muzzle flash (card FXMF, `analysis/re-muzzle-flash.md` §4 — the
    /// same GS state: ADDITIVE, depth tested, no depth write, unfogged,
    /// vertex colour × texel).
    Mesh {
        tris: Vec<(Vec3, [f32; 2])>,
    },
    SolidMesh {
        tris: Vec<(Vec3, [f32; 2])>,
    },
    CutoutMesh {
        tris: Vec<(Vec3, [f32; 2])>,
    },
    CutoutQuad {
        pos: Vec3,
        a: Vec3,
        b: Vec3,
    },
    Shaded {
        tris: Vec<(Vec3, [f32; 2], [u8; 4])>,
    },
    /// Procedural untextured geometry with per-vertex alpha, such as a
    /// hologram's axial flare. Unlike a textured effect missing its sheet,
    /// this deliberately uses the renderer's white fallback texture.
    UntexturedShaded {
        tris: Vec<(Vec3, [f32; 2], [u8; 4])>,
    },
    /// One weather cell's rain streaks (card WTHR, `analysis/re-weather.md`
    /// §3.1, §3.4): the PS2 draws each as ONE untextured GS line, a pixel of
    /// its 512-wide frame, Gouraud from the head's alpha to 0 at the tail.
    /// Here a line is a camera-facing strip `width` metres wide per metre of
    /// view depth (the PS2's pixel as an angle, so the look holds at any
    /// resolution). The draw's `color` is not read: a streak's GS bytes are
    /// not the particle path's (`rgb` is the displayed colour, 0..1; a
    /// line's alpha is the blend factor) and it needs no texture.
    Streaks {
        /// (head, tail, the head's alpha).
        lines: Vec<(Vec3, Vec3, f32)>,
        rgb: [f32; 3],
        width: f32,
    },
    /// A light beam (card CPHOL, `crate::light_beam`): untextured, additive,
    /// its fan and its alphas built for the view; the draw's `color` is the
    /// beam's bytes.
    Beam(crate::light_beam::Beam),
}

/// One drawn particle (or one STREAK).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleDraw {
    pub shape: ParticleShape,
    /// Where the view measures its distance from (see the module doc).
    pub key: Vec3,
    /// RGBA bytes, `(int)(c·255)`, before the GS halving.
    pub color: [u8; 4],
    pub texture: Option<TextureHandle>,
    /// ADDITIVE (0x48); else NORMAL (0x44).
    pub additive: bool,
}

/// A sprite's four corners and UVs as a triangle list, in the screen plane
/// `(right, up)`.
fn sprite(
    out: &mut Vec<GrassVertex>,
    pos: Vec3,
    (half, angle): (f32, f32),
    uv: [f32; 4],
    (right, up): (Vec3, Vec3),
    color: [f32; 4],
) {
    let (s, c) = angle.sin_cos();
    // R(angle) applied to the screen-plane offsets (x right, y up).
    let at =
        |x: f32, y: f32| pos + right * (half * (c * x - s * y)) + up * (half * (s * x + c * y));
    let tl = vtx(at(-1.0, 1.0), [uv[0], uv[1]], color);
    let tr = vtx(at(1.0, 1.0), [uv[2], uv[1]], color);
    let bl = vtx(at(-1.0, -1.0), [uv[0], uv[3]], color);
    let br = vtx(at(1.0, -1.0), [uv[2], uv[3]], color);
    out.extend([tl, tr, bl, bl, tr, br]);
}

fn vtx(p: Vec3, uv: [f32; 2], color: [f32; 4]) -> GrassVertex {
    GrassVertex {
        position: p.to_array(),
        uv,
        color,
        up: 0.0,
    }
}

/// A ribbon's side vectors, one per point (see the module doc).
pub fn ribbon_sides(points: &[(Vec3, f32, f32)], eye: Vec3, cam_up: Vec3) -> Vec<Vec3> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let p = points[i].0;
            let prev = if i > 0 { points[i - 1].0 } else { p };
            let next = if i + 1 < n { points[i + 1].0 } else { p };
            let side = ((p - prev) + (next - p)).cross(p - eye);
            let side = if side.length_squared() > 1e-12 {
                side.normalize()
            } else {
                cam_up
            };
            side * points[i].1
        })
        .collect()
}

/// The triangles of one particle for this view.
#[cfg(test)]
fn particle_vertices(d: &ParticleDraw, camera: &Camera) -> Vec<GrassVertex> {
    let mut out = Vec::new();
    append_particle_vertices(&mut out, d, camera);
    out
}

/// Append directly to the frame buffer: dense weather otherwise allocates and
/// copies a six-vertex temporary for every flake on every frame.
fn append_particle_vertices(out: &mut Vec<GrassVertex>, d: &ParticleDraw, camera: &Camera) {
    let right = camera.right();
    let up = right.cross(camera.forward()).normalize_or_zero();
    let color = [
        gs_unit(d.color[0]),
        gs_unit(d.color[1]),
        gs_unit(d.color[2]),
        gs_unit(d.color[3]),
    ];
    match &d.shape {
        ParticleShape::Sprite {
            pos,
            half,
            angle,
            uv,
        } => sprite(out, *pos, (*half, *angle), *uv, (right, up), color),
        ParticleShape::Quad { pos, a, b } | ParticleShape::CutoutQuad { pos, a, b } => {
            let v = [
                vtx(*pos - *a + *b, [0.0, 0.0], color),
                vtx(*pos + *a + *b, [1.0, 0.0], color),
                vtx(*pos - *a - *b, [0.0, 1.0], color),
                vtx(*pos + *a - *b, [1.0, 1.0], color),
            ];
            out.extend([v[0], v[1], v[2], v[2], v[1], v[3]]);
        }
        ParticleShape::Mesh { tris }
        | ParticleShape::SolidMesh { tris }
        | ParticleShape::CutoutMesh { tris } => {
            out.extend(tris.iter().map(|(p, uv)| vtx(*p, *uv, color)));
        }
        ParticleShape::Shaded { tris } | ParticleShape::UntexturedShaded { tris } => {
            out.extend(tris.iter().map(|(p, uv, c)| vtx(*p, *uv, c.map(gs_unit))));
        }
        ParticleShape::Streaks { lines, rgb, width } => {
            let (eye, fwd) = (camera.position, camera.forward());
            for (head, tail, alpha) in lines {
                let side = (*tail - *head).cross(*head - eye);
                let side = if side.length_squared() > 1e-12 {
                    side.normalize()
                } else {
                    up
                };
                // Half the strip's width at each end's own depth.
                let half = |p: Vec3| side * (0.5 * width * (p - eye).dot(fwd).max(camera.near));
                let (sh, st) = (half(*head), half(*tail));
                let ch = [rgb[0], rgb[1], rgb[2], *alpha];
                let ct = [rgb[0], rgb[1], rgb[2], 0.0];
                let a = vtx(*head + sh, [0.0, 0.0], ch);
                let b = vtx(*head - sh, [0.0, 1.0], ch);
                let c = vtx(*tail + st, [1.0, 0.0], ct);
                let e = vtx(*tail - st, [1.0, 1.0], ct);
                out.extend([a, b, c, c, b, e]);
            }
        }
        ParticleShape::Beam(b) => out.extend(crate::light_beam::vertices(b, d.color, camera)),
        ParticleShape::Ribbon { points } => {
            let sides = ribbon_sides(points, camera.position, up);
            for i in 1..points.len() {
                let (p0, _, u0) = points[i - 1];
                let (p1, _, u1) = points[i];
                let a = vtx(p0 + sides[i - 1], [u0, 0.0], color);
                let b = vtx(p0 - sides[i - 1], [u0, 1.0], color);
                let c = vtx(p1 + sides[i], [u1, 0.0], color);
                let e = vtx(p1 - sides[i], [u1, 1.0], color);
                out.extend([a, b, c, c, b, e]);
            }
        }
        ParticleShape::CappedRibbon {
            head,
            head_half,
            tail,
            tail_half,
        } => {
            out.extend(
                crate::bolt::ribbon(
                    *head,
                    *head_half,
                    *tail,
                    *tail_half,
                    camera.position,
                    right,
                    d.color[3],
                )
                .into_iter()
                .map(|v| {
                    vtx(
                        v.pos,
                        v.uv,
                        [color[0], color[1], color[2], gs_unit(v.alpha)],
                    )
                }),
            );
        }
        ParticleShape::Trail { sections, length } => {
            out.extend(trail::vertices(sections, *length, camera));
        }
    }
}

/// One thing to draw in the shared list: a bolt (its two ribbons) or a
/// particle.
#[derive(Clone, Copy)]
enum Item<'a> {
    Bolt(&'a BoltDraw),
    Particle(&'a ParticleDraw),
}

/// Exactly `items.sort_by(|a, b| b.0.total_cmp(&a.0))`, stable and for every
/// float, in linear time: dense weather sorts thousands of items a frame, and
/// this comparison sort was the largest single CPU cost of a Hoth frame.
fn sort_far_first<T: Copy>(items: &mut Vec<(f32, T)>) {
    const BITS: u32 = 11;
    const PASSES: u32 = 3;
    const BUCKETS: usize = 1 << BITS;
    // Below this the three bucket tables cost more than the sort.
    if items.len() < 256 {
        items.sort_by(|a, b| b.0.total_cmp(&a.0));
        return;
    }
    // `f32::total_cmp`'s order as a u32, then reversed: ascending is far first.
    let key = |d: f32| {
        let b = d.to_bits() as i32;
        let total = b ^ (((b >> 31) as u32) >> 1) as i32;
        !((total as u32) ^ 0x8000_0000)
    };
    let digit = |d: f32, pass: u32| (key(d) >> (pass * BITS)) as usize & (BUCKETS - 1);
    let mut counts = [[0u32; BUCKETS]; PASSES as usize];
    for item in items.iter() {
        for pass in 0..PASSES {
            counts[pass as usize][digit(item.0, pass)] += 1;
        }
    }
    let mut spare = items.clone();
    for pass in 0..PASSES {
        let mut at = 0u32;
        for count in &mut counts[pass as usize] {
            at += std::mem::replace(count, at);
        }
        let starts = &mut counts[pass as usize];
        for item in items.iter() {
            let slot = &mut starts[digit(item.0, pass)];
            spare[*slot as usize] = *item;
            *slot += 1;
        }
        std::mem::swap(items, &mut spare);
    }
}

/// The view's bolts and particles, farthest first, merged into runs.
pub(crate) fn build(bolts: &[BoltDraw], particles: &[ParticleDraw], camera: &Camera) -> BoltFrame {
    let eye = camera.position;
    let mut items: Vec<(f32, Item)> = bolts
        .iter()
        .map(|b| (b.head.distance_squared(eye), Item::Bolt(b)))
        .chain(
            particles
                .iter()
                .map(|p| (p.key.distance_squared(eye), Item::Particle(p))),
        )
        .collect();
    // Stable: equal keys keep their order (spawn order within an effect).
    sort_far_first(&mut items);
    let mut out = BoltFrame::default();
    let push = |out: &mut BoltFrame, first, tex, additive, solid, cutout| {
        let end = out.vertices.len() as u32;
        if first == end {
            return;
        }
        match out.runs.last_mut() {
            Some(r)
                if r.texture == tex
                    && r.glow == additive
                    && r.solid == solid
                    && r.cutout == cutout
                    && r.vertices.end == first =>
            {
                r.vertices.end = end;
            }
            _ => out.runs.push(BoltRun {
                texture: tex,
                vertices: first..end,
                glow: additive,
                solid,
                cutout,
            }),
        }
    };
    for (_, item) in items {
        match item {
            Item::Bolt(b) => {
                for (verts, tex, glow) in crate::bolt::bolt_vertices(b, camera) {
                    let first = out.vertices.len() as u32;
                    out.vertices.extend(verts);
                    push(&mut out, first, tex, glow, false, false);
                }
            }
            Item::Particle(p) => {
                // A missing sheet draws nothing (the PS2 samples VRAM block
                // 0, re-fx §6.9 [H] — PROVISIONAL).
                if p.texture.is_some()
                    || matches!(
                        p.shape,
                        ParticleShape::Streaks { .. }
                            | ParticleShape::Beam(_)
                            | ParticleShape::Trail { .. }
                            | ParticleShape::UntexturedShaded { .. }
                    )
                {
                    let first = out.vertices.len() as u32;
                    append_particle_vertices(&mut out.vertices, p, camera);
                    push(
                        &mut out,
                        first,
                        p.texture,
                        p.additive,
                        !p.additive && matches!(p.shape, ParticleShape::SolidMesh { .. }),
                        !p.additive
                            && matches!(
                                p.shape,
                                ParticleShape::CutoutQuad { .. } | ParticleShape::CutoutMesh { .. }
                            ),
                    );
                }
            }
        }
    }
    out
}

/// This frame's particles, set by [`Renderer::set_particles`].
#[derive(Default)]
pub(crate) struct ParticleState {
    pub particles: Vec<ParticleDraw>,
}

impl Renderer {
    /// Replace the particles drawn from the next frame on (none until this
    /// is called). Texture handles must be this renderer's.
    pub fn set_particles(&mut self, particles: Vec<ParticleDraw>) {
        self.particles.particles = particles;
    }

    /// Add to the particles already set (a still render's holograms beside
    /// its weather).
    pub fn add_particles(&mut self, particles: Vec<ParticleDraw>) {
        self.particles.particles.extend(particles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(shape: ParticleShape, key: Vec3) -> ParticleDraw {
        ParticleDraw {
            shape,
            key,
            color: [255, 255, 255, 255],
            texture: Some(TextureHandle(0)),
            additive: true,
        }
    }

    #[test]
    fn a_capped_ribbon_keeps_authored_half_width_and_full_white() {
        let d = draw(
            ParticleShape::CappedRibbon {
                head: Vec3::new(0.0, 1.0, -5.0),
                head_half: 0.064,
                tail: Vec3::new(0.0, -0.04, -5.0),
                tail_half: 0.08,
            },
            Vec3::ZERO,
        );
        let vertices = particle_vertices(&d, &Camera::default());
        assert_eq!(vertices.len(), 15, "cap plus body fan");
        assert!(vertices.iter().all(|v| v.color == [1.0; 4]));
        assert!(vertices
            .iter()
            .any(|v| v.uv[0] == 0.0 && v.position[1] > 1.0));
        let tail: Vec<_> = vertices.iter().filter(|v| v.uv[0] > 0.99).collect();
        assert_eq!(tail.len(), 4);
        assert!(tail
            .iter()
            .all(|v| (v.position[0].abs() - 0.08).abs() < 1e-6));
        assert!(tail.iter().all(|v| (v.position[1] + 0.04).abs() < 1e-6));
    }

    /// Camera at the origin looking down −Z: right = +X, up = +Y. An
    /// unrotated sprite's corners sit at ±half, (0,0) top left; at 90° the
    /// top-left corner has turned counter-clockwise to the bottom left.
    #[test]
    fn a_sprite_faces_the_screen_and_turns_counter_clockwise() {
        let cam = Camera::default();
        let d = draw(
            ParticleShape::Sprite {
                pos: Vec3::new(0.0, 0.0, -5.0),
                half: 0.5,
                angle: 0.0,
                uv: [0.0, 0.0, 1.0, 1.0],
            },
            Vec3::ZERO,
        );
        let v = particle_vertices(&d, &cam);
        assert_eq!(v.len(), 6);
        let tl = Vec3::from(v[0].position);
        assert!(tl.abs_diff_eq(Vec3::new(-0.5, 0.5, -5.0), 1e-6), "{tl}");
        assert_eq!(v[0].uv, [0.0, 0.0]);
        assert_eq!(v[5].uv, [1.0, 1.0]);
        let d = ParticleDraw {
            shape: ParticleShape::Sprite {
                pos: Vec3::new(0.0, 0.0, -5.0),
                half: 0.5,
                angle: std::f32::consts::FRAC_PI_2,
                uv: [0.0, 0.0, 1.0, 1.0],
            },
            ..d
        };
        let tl = Vec3::from(particle_vertices(&d, &cam)[0].position);
        assert!(tl.abs_diff_eq(Vec3::new(-0.5, -0.5, -5.0), 1e-6), "{tl}");
    }

    /// A ribbon's side is perpendicular to its run and to the eye ray, at
    /// the point's half-width; the GS scale maps 255 to 1.0.
    #[test]
    fn a_ribbon_faces_the_eye() {
        let cam = Camera::default();
        let pts = vec![
            (Vec3::new(-1.0, 0.0, -5.0), 0.1, 0.0),
            (Vec3::new(0.0, 0.0, -5.0), 0.1, 0.5),
            (Vec3::new(1.0, 0.0, -5.0), 0.1, 1.0),
        ];
        let sides = ribbon_sides(&pts, cam.position, Vec3::Y);
        for s in &sides {
            assert!((s.y.abs() - 0.1).abs() < 1e-6 && s.x.abs() < 1e-6, "{s}");
        }
        let v = particle_vertices(
            &draw(ParticleShape::Ribbon { points: pts }, Vec3::ZERO),
            &cam,
        );
        assert_eq!(v.len(), 12, "two segments");
        assert_eq!(v[0].color, [1.0; 4]);
    }

    /// Card WTHR: a streak is a strip from its head (its alpha) to its tail
    /// (alpha 0), `width` per metre of depth wide, in the displayed colour —
    /// and it draws without a sheet.
    #[test]
    fn a_streak_fades_to_its_tail_and_needs_no_sheet() {
        let cam = Camera::default();
        let d = ParticleDraw {
            texture: None,
            additive: false,
            ..draw(
                ParticleShape::Streaks {
                    lines: vec![(Vec3::new(0.0, 0.0, -4.0), Vec3::new(0.0, 0.5, -8.0), 0.8)],
                    rgb: [0.25, 0.5, 0.75],
                    width: 0.01,
                },
                Vec3::new(0.0, 0.0, -5.0),
            )
        };
        let v = particle_vertices(&d, &cam);
        assert_eq!(v.len(), 6);
        assert_eq!(v[0].color, [0.25, 0.5, 0.75, 0.8]);
        assert_eq!(v[2].color[3], 0.0);
        let w =
            |a: usize, b: usize| (Vec3::from(v[a].position) - Vec3::from(v[b].position)).length();
        assert!((w(0, 1) - 0.04).abs() < 1e-6, "4 m deep: {}", w(0, 1));
        assert!((w(2, 5) - 0.08).abs() < 1e-6, "8 m deep: {}", w(2, 5));
        let f = build(&[], &[d], &cam);
        assert_eq!(f.runs.len(), 1);
        assert_eq!((f.runs[0].texture, f.runs[0].glow), (None, false));
    }

    /// A mesh draws its triangles as given, whatever the view, at the
    /// GS-scaled colour (the muzzle flash's (80, 80, 255) → 40/128, 1.0).
    #[test]
    fn a_mesh_keeps_its_world_triangles() {
        let tris = vec![
            (Vec3::new(1.0, 2.0, 3.0), [0.0, 0.0]),
            (Vec3::new(1.0, 3.0, 3.0), [1.0, 0.0]),
            (Vec3::new(2.0, 2.0, 3.0), [0.0, 1.0]),
        ];
        let d = ParticleDraw {
            color: [80, 80, 255, 255],
            ..draw(ParticleShape::Mesh { tris: tris.clone() }, Vec3::ZERO)
        };
        let v = particle_vertices(&d, &Camera::default());
        assert_eq!(v.len(), 3);
        for (a, (p, uv)) in v.iter().zip(&tris) {
            assert_eq!((Vec3::from(a.position), a.uv), (*p, *uv));
            assert_eq!(a.color, [40.0 / 128.0, 40.0 / 128.0, 1.0, 1.0]);
        }
    }

    #[test]
    fn shaded_triangles_carry_their_own_vertex_bytes() {
        let tris = vec![
            (Vec3::new(1.0, 2.0, 3.0), [0.0, 0.0], [255, 0, 128, 0]),
            (Vec3::new(1.0, 3.0, 3.0), [0.5, 0.33], [64, 64, 64, 127]),
            (Vec3::new(2.0, 2.0, 3.0), [1.0, 1.0], [0, 255, 0, 255]),
        ];
        let d = draw(ParticleShape::Shaded { tris: tris.clone() }, Vec3::ZERO);
        let v = particle_vertices(&d, &Camera::default());
        assert_eq!(v.len(), 3);
        for (a, (p, uv, c)) in v.iter().zip(&tris) {
            assert_eq!((Vec3::from(a.position), a.uv), (*p, *uv));
            assert_eq!(a.color, c.map(gs_unit));
        }
        assert_eq!(v[1].color, [0.25, 0.25, 0.25, 0.5]);
    }

    #[test]
    fn procedural_shaded_geometry_draws_without_a_sheet() {
        let tris = vec![(Vec3::new(0.0, 0.0, -3.0), [0.0; 2], [128; 4]); 3];
        let mut d = draw(
            ParticleShape::UntexturedShaded { tris: tris.clone() },
            Vec3::ZERO,
        );
        d.texture = None;
        let frame = build(&[], &[d.clone()], &Camera::default());
        assert_eq!(frame.vertices.len(), 3);
        assert_eq!(frame.runs[0].texture, None);
        assert!(frame.runs[0].glow);
        d.shape = ParticleShape::Shaded { tris };
        assert!(
            build(&[], &[d], &Camera::default()).vertices.is_empty(),
            "missing authored sheets still skip textured effects"
        );
    }

    #[test]
    fn a_beam_appends_without_replacing_preceding_particle_vertices() {
        let cam = Camera::default();
        let far = draw(
            ParticleShape::Quad {
                pos: Vec3::new(0.0, 0.0, -10.0),
                a: Vec3::X,
                b: Vec3::Y,
            },
            Vec3::new(0.0, 0.0, -10.0),
        );
        let mut beam = draw(
            ParticleShape::Beam(crate::light_beam::Beam {
                base: Vec3::new(0.0, 0.0, -5.0),
                axis: Vec3::Y,
                length: 3.0,
                r0: 1.0,
                r1: 0.5,
                soft: 0.2,
                visible: 100.0,
                view: None,
                gs_rgb: 1.0,
                top: 0.25,
                cross: None,
            }),
            Vec3::new(0.0, 0.0, -5.0),
        );
        beam.texture = None;
        let frame = build(&[], &[far, beam], &cam);
        assert_eq!(frame.runs.len(), 2);
        assert_eq!(frame.runs[0].vertices, 0..6);
        assert_eq!(frame.vertices[0].position, [-1.0, 1.0, -10.0]);
        assert_eq!(frame.runs[1].vertices.start, 6);
        assert!(frame.runs[1].vertices.end > 6);
        assert_eq!(frame.runs[1].vertices.end as usize, frame.vertices.len());
    }

    /// The linear sort orders every float and every tie as the stable
    /// comparison sort does: signed zeros, infinities and NaNs of both signs.
    #[test]
    fn the_far_first_sort_is_the_stable_total_order_sort() {
        let mut seed = 0x1234_5678u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let special = [
            0.0,
            -0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            -f32::NAN,
            f32::MIN_POSITIVE,
            1e-42,
            1.0,
            4.0,
        ];
        for n in [0, 3, 255, 256, 257, 5000] {
            let mut items: Vec<(f32, u32)> = (0..n)
                .map(|i| {
                    let r = next();
                    let d = match r % 4 {
                        0 => special[(r >> 8) as usize % special.len()],
                        1 => f32::from_bits(r),
                        2 => (r >> 20) as f32,
                        _ => (r as f32 / u32::MAX as f32) * 40_000.0,
                    };
                    (d, i)
                })
                .collect();
            let mut expected = items.clone();
            expected.sort_by(|a, b| b.0.total_cmp(&a.0));
            sort_far_first(&mut items);
            let bits =
                |v: &[(f32, u32)]| v.iter().map(|(d, i)| (d.to_bits(), *i)).collect::<Vec<_>>();
            assert_eq!(bits(&items), bits(&expected), "{n} items");
        }
    }

    /// Far first; equal keys keep their order; a run of one sheet and blend
    /// is one draw; a particle with no sheet draws nothing.
    #[test]
    fn items_sort_far_to_near_and_merge() {
        let cam = Camera::default();
        let q = |z: f32| ParticleShape::Quad {
            pos: Vec3::new(0.0, 0.0, z),
            a: Vec3::X,
            b: Vec3::Y,
        };
        let mut near = draw(q(-2.0), Vec3::new(0.0, 0.0, -2.0));
        near.additive = false;
        let far1 = draw(q(-10.0), Vec3::new(0.0, 0.0, -10.0));
        let far2 = draw(q(-10.5), Vec3::new(0.0, 0.0, -10.0));
        let mut none = draw(q(-3.0), Vec3::new(0.0, 0.0, -3.0));
        none.texture = None;
        let f = build(&[], &[near, far1, none, far2], &cam);
        assert_eq!(f.runs.len(), 2, "{:?}", f.runs);
        assert!(f.runs[0].glow && !f.runs[1].glow);
        assert_eq!(f.runs[0].vertices, 0..12, "the two far quads, merged");
        assert_eq!(f.vertices[0].position[2], -10.0, "spawn order kept");
        assert_eq!(f.vertices.len(), 18);
    }
}
