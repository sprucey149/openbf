//! The grass patches: the PS2's `grasspatch` sprite clusters
//! (analysis/re-foliage.md §7, [O]).
//!
//! A class holds ONE template of sprites (`game::viewer::grass` builds it as
//! B1 `FUN_001ae120` does); every patch of the class draws that template at
//! its own position and tilt with its own colour. Each view the PS2 rebuilds
//! the class's sprite buffer back to front in the patch's LOCAL frame from
//! the world camera's axes (B1 `FUN_002eca58`, B2 `FUN_003c6788`), one quad
//! per sprite (`FUN_002ec4c8` / `FUN_002ec2d8`), then draws it once per
//! visible patch (`FUN_002ed080`). This module does the same: the per-view
//! buffer on the CPU ([`sprite_quad`], a few hundred quads), the patches as
//! GPU instances of it.
//!
//! - A NORMAL sprite stands on its base point: right = the camera's right
//!   turned toward the look axis by a = ±0.05·(i mod 5) (odd i negative),
//!   up = `normalize(Y − 0.7·(Y·B)·B)` (B the camera's back axis) × the
//!   sprite's height factor; its top edge is sheared by
//!   `(sin(phase)·0.08 + lean)·width`.
//! - A TALL sprite (a fern frond) is a quad PERPENDICULAR to a leaned up
//!   `Y − L·lean` (L the horizontal look axis; lean = the class's tall lean,
//!   plus `sin(phase)·0.03` unless the class is `still`): side =
//!   `normalize(facing × up + (0, facing.y, 0))`, depth = `normalize(side ×
//!   up)`, both × the sprite's width, centred on its point.
//! - Vertex k takes atlas corner k: top-left, top-right, bottom-left,
//!   bottom-right (`FUN_002ec2d8`).
//! - The sway: `phase = phase₀ + t·0.8·(1 + 0.2·(i mod 5))` (`FUN_001af2e0`).
//! - A patch draws while `−bound ≤ depth ≤ D` (depth along the camera's look
//!   axis), its alpha × `min((D − depth)·0.05, 1)` — the last 20 m fade.
//!
use crate::{Camera, Renderer, TextureHandle};
use glam::{Mat4, Vec3};

/// One sprite of a class template, in the patch's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrassSprite {
    /// Base point (normal) or centre (tall).
    pub pos: Vec3,
    pub width: f32,
    pub height: f32,
    /// Normal sprites: the up axis's scale (+0x54: 1, or 0.8..1 when the
    /// class has 3 or ≥ 5 parts).
    pub up_scale: f32,
    /// Normal sprites: the static lean (+0x5c).
    pub lean: f32,
    /// The sway phase at t = 0.
    pub phase: f32,
    /// Its index in the template (the turn and the sway rate read it).
    pub index: u32,
    /// Tall sprites: the fixed facing (+0x20, unit).
    pub tall: Option<Vec3>,
    /// Atlas corners: top-left, top-right, bottom-left, bottom-right.
    pub uv: [[f32; 2]; 4],
    /// Grey, grey, grey, the class `Alpha`.
    pub color: [f32; 4],
}

/// One `grasspatch` class ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct GrassClass {
    pub texture: Option<TextureHandle>,
    pub sprites: Vec<GrassSprite>,
    /// The tall blades' lean (`0xf6582b9c`).
    pub tall_lean: f32,
    /// `0x604db82e`: tall blades do not sway.
    pub still: bool,
    /// The template's radius about its box centre plus a sprite width (+0x5c).
    pub bound: f32,
}

/// One placed patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrassPatch {
    pub class: u32,
    /// Local → world: columns right, up, forward, position.
    pub matrix: Mat4,
    /// The draw colour, 1.0 = unity (the FOLG tint; white for a world object).
    pub color: [f32; 4],
    /// D, metres: drawn up to this depth, fading over its last 20 m.
    pub distance: f32,
}

/// `DAT_00518a5c`: a normal sprite's sway shear.
const SWAY: f32 = 0.08;
/// `DAT_00518a60`: a tall blade's sway lean.
const TALL_SWAY: f32 = 0.03;
/// `DAT_005174b4`: the sway rate.
const RATE: f32 = 0.8;
/// The fade before D: alpha × (D − depth)·0.05.
const FADE: f32 = 0.05;

/// The camera's axes as the grass build reads them.
#[derive(Debug, Clone, Copy)]
pub struct ViewAxes {
    /// Screen right.
    pub right: Vec3,
    /// The look direction.
    pub look: Vec3,
}

impl ViewAxes {
    pub fn of(c: &Camera) -> ViewAxes {
        ViewAxes {
            right: c.right().normalize_or_zero(),
            look: c.forward().normalize_or_zero(),
        }
    }
}

/// `sin(phase)` of sprite `s` at `t` seconds. `t` is [`Renderer::time`],
/// which the caller wraps at [`crate::TIME_WRAP`]: 2000 s is not a whole
/// number of sway periods, so the blades jump phase once at each wrap.
pub fn sway(s: &GrassSprite, t: f32) -> f32 {
    let rate = RATE * (1.0 + 0.2 * (s.index % 5) as f32);
    (s.phase + t * rate).sin()
}

/// A sprite's four corners in its patch's frame, in atlas-corner order
/// (see the module doc).
pub fn sprite_quad(s: &GrassSprite, class: &GrassClass, v: &ViewAxes, t: f32) -> [Vec3; 4] {
    let back = -v.look;
    let (w, n) = match s.tall {
        None => {
            let a = (s.index % 5) as f32 * if s.index % 2 == 1 { -0.05 } else { 0.05 };
            let right = v.right * (1.0 - a.abs()) + v.look * a;
            let up = (Vec3::Y - back * (0.7 * back.y)).normalize_or_zero() * s.up_scale;
            let w = right * s.width;
            let top = s.pos + up * s.height - w * 0.5 + w * (sway(s, t) * SWAY + s.lean);
            return [top, top + w, s.pos - w * 0.5, s.pos + w * 0.5];
        }
        Some(facing) => {
            let level = Vec3::new(v.look.x, 0.0, v.look.z)
                .try_normalize()
                .unwrap_or(Vec3::X);
            let lean = if class.still {
                class.tall_lean
            } else {
                sway(s, t) * TALL_SWAY + class.tall_lean
            };
            let up = Vec3::Y - level * lean;
            let side = (facing.cross(up) + Vec3::new(0.0, facing.y, 0.0)).normalize_or_zero();
            let depth = side.cross(up).normalize_or_zero();
            (side * s.width, depth * s.width)
        }
    };
    let top = s.pos + n * 0.5 - w * 0.5;
    let bottom = s.pos - n * 0.5;
    [top, top + w, bottom - w * 0.5, bottom + w * 0.5]
}

/// A class's sprites in draw order for this view: far to near along the
/// look axis (the PS2's heap on the template position · the back axis).
pub fn draw_order(class: &GrassClass, v: &ViewAxes) -> Vec<usize> {
    let mut order: Vec<usize> = (0..class.sprites.len()).collect();
    order.sort_by(|&a, &b| {
        let (da, db) = (
            class.sprites[a].pos.dot(-v.look),
            class.sprites[b].pos.dot(-v.look),
        );
        da.total_cmp(&db)
    });
    order
}

/// A patch's alpha factor at `depth` along the look axis, `None` when it is
/// not drawn (`FUN_002ed080`).
pub fn patch_alpha(depth: f32, distance: f32, bound: f32) -> Option<f32> {
    (-bound <= depth && depth <= distance).then(|| ((distance - depth) * FADE).min(1.0))
}

/// One vertex of the per-view sprite buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GrassVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    /// A leaf quad's normal along its frame's up axis: +1 on the top pair,
    /// −1 on the bottom (`leaf.rs`; BF2 lights by it). 0 on grass.
    pub up: f32,
}

impl GrassVertex {
    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GrassVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

/// One patch instance: its matrix columns and colour × fade.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GrassInstance {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
    /// x: 1 = lit on the vertex `up` normals (BF2 leaves); y: 1 = skip the
    /// world's far fade (a leaf object carries its own). 0 on grass.
    pub params: [f32; 4],
}

impl GrassInstance {
    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
            4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GrassInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRS,
        }
    }
}

/// One instanced draw of the frame's grass: a class's vertex range and a run
/// of its patches.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GrassRun {
    pub texture: Option<TextureHandle>,
    pub vertices: std::ops::Range<u32>,
    pub instances: std::ops::Range<u32>,
}

/// The frame's grass, built on the CPU: the per-view sprite buffer of every
/// class with a visible patch, the visible patches far to near, and the runs
/// that draw them.
#[derive(Debug, Default)]
pub(crate) struct GrassFrame {
    pub vertices: Vec<GrassVertex>,
    pub instances: Vec<GrassInstance>,
    pub runs: Vec<GrassRun>,
}

/// What a frame's grass and leaves are filtered by.
#[derive(Clone, Copy)]
pub(crate) struct FoliageView<'a> {
    /// Patches wholly off screen are left out, when set. The PS2's own test is
    /// only depth along the look axis, so without this a patch at the eye's side
    /// still builds and draws.
    pub frustum: Option<&'a crate::frustum::Frustum>,
}

impl FoliageView<'_> {
    /// No filter: everything the depth test keeps, at its own distance.
    #[cfg(test)]
    pub(crate) const ALL: FoliageView<'static> = FoliageView { frustum: None };

    /// Whether a patch at `at`, reaching `bound` from its origin, can show. The
    /// margin is the template's: `bound` is measured from its box's centre, and
    /// the origin may be off that by as much again.
    pub(crate) fn may_show(&self, at: Vec3, bound: f32) -> bool {
        self.frustum.is_none_or(|f| {
            let r = Vec3::splat(bound * 2.0);
            f.intersects_aabb(at - r, at + r)
        })
    }
}

/// Build the frame's grass for `camera` at `t` seconds.
pub(crate) fn build(
    classes: &[GrassClass],
    patches: &[GrassPatch],
    camera: &Camera,
    t: f32,
    view: FoliageView<'_>,
) -> GrassFrame {
    let v = ViewAxes::of(camera);
    // Visible patches with their depth and alpha.
    let mut vis: Vec<(f32, usize, f32)> = patches
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let class = classes.get(p.class as usize)?;
            let at = p.matrix.w_axis.truncate();
            let depth = (at - camera.position).dot(v.look);
            let alpha = patch_alpha(depth, p.distance, class.bound)?;
            view.may_show(at, class.bound).then_some((depth, i, alpha))
        })
        .collect();
    let mut out = GrassFrame::default();
    if vis.is_empty() {
        return out;
    }
    // Far to near; a stable sort keeps equal depths in placement order.
    vis.sort_by(|a, b| b.0.total_cmp(&a.0));
    // Each drawn class's sprite buffer, once.
    let mut ranges: Vec<Option<std::ops::Range<u32>>> = vec![None; classes.len()];
    for &(_, i, _) in &vis {
        let c = patches[i].class as usize;
        if ranges[c].is_some() {
            continue;
        }
        let class = &classes[c];
        let first = out.vertices.len() as u32;
        for k in draw_order(class, &v) {
            let s = &class.sprites[k];
            let q = sprite_quad(s, class, &v, t);
            for corner in [0, 1, 2, 1, 3, 2] {
                out.vertices.push(GrassVertex {
                    position: q[corner].to_array(),
                    uv: s.uv[corner],
                    color: s.color,
                    up: 0.0,
                });
            }
        }
        ranges[c] = Some(first..out.vertices.len() as u32);
    }
    for &(_, i, a) in &vis {
        let p = &patches[i];
        let slot = out.instances.len() as u32;
        let mut color = p.color;
        color[3] *= a;
        out.instances.push(GrassInstance {
            model: p.matrix.to_cols_array_2d(),
            color,
            params: [0.0; 4],
        });
        let c = p.class as usize;
        match out.runs.last_mut() {
            Some(r) if Some(&r.vertices) == ranges[c].as_ref() => r.instances.end = slot + 1,
            _ => out.runs.push(GrassRun {
                texture: classes[c].texture,
                vertices: ranges[c].clone().unwrap(),
                instances: slot..slot + 1,
            }),
        }
    }
    out
}

/// The renderer's grass: the classes and patches of the loaded scene, and
/// the buffers the frame writes.
#[derive(Default)]
pub(crate) struct GrassState {
    pub classes: Vec<GrassClass>,
    pub patches: Vec<GrassPatch>,
    pub vertex_buf: Option<(wgpu::Buffer, usize)>,
    pub instance_buf: Option<(wgpu::Buffer, usize)>,
}

/// A buffer of at least `bytes`, grown to the next power of two.
pub(crate) fn ensure(
    device: &wgpu::Device,
    slot: &mut Option<(wgpu::Buffer, usize)>,
    bytes: usize,
    label: &str,
) {
    if slot.as_ref().is_some_and(|(_, n)| *n >= bytes) {
        return;
    }
    let size = bytes.next_power_of_two().max(256);
    *slot = Some((
        crate::resources::buffer(
            device,
            label,
            size as u64,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        ),
        size,
    ));
}

impl Renderer {
    /// Replace the scene's grass (none until this is called). Every patch's
    /// `class` indexes `classes`; a texture handle must be this renderer's.
    pub fn set_grass(&mut self, classes: Vec<GrassClass>, patches: Vec<GrassPatch>) {
        self.grass.classes = classes;
        self.grass.patches = patches;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite(tall: Option<Vec3>) -> GrassSprite {
        GrassSprite {
            pos: Vec3::new(1.0, 0.0, 2.0),
            width: 2.0,
            height: 3.0,
            up_scale: 1.0,
            lean: 0.0,
            phase: 0.0,
            index: 0,
            tall,
            uv: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
            color: [1.0; 4],
        }
    }

    fn class(s: GrassSprite) -> GrassClass {
        GrassClass {
            texture: None,
            sprites: vec![s],
            tall_lean: 0.5,
            still: true,
            bound: 5.0,
        }
    }

    /// A level camera looking down −Z: a normal sprite at phase 0 (sin 0)
    /// stands upright, `width` across screen-right, `height` tall, its base
    /// centred on its point.
    #[test]
    fn a_normal_sprite_faces_the_camera() {
        let s = sprite(None);
        let v = ViewAxes {
            right: Vec3::X,
            look: Vec3::NEG_Z,
        };
        let q = sprite_quad(&s, &class(s), &v, 0.0);
        let p = s.pos;
        assert!(q[2].abs_diff_eq(p - Vec3::X, 1e-6), "{q:?}");
        assert!(q[3].abs_diff_eq(p + Vec3::X, 1e-6));
        assert!(q[0].abs_diff_eq(p + Vec3::new(-1.0, 3.0, 0.0), 1e-6));
        assert!(q[1].abs_diff_eq(p + Vec3::new(1.0, 3.0, 0.0), 1e-6));
        // Sprite 3 turns toward the look axis by 0.15 (odd: −).
        let mut s3 = s;
        s3.index = 3;
        let q = sprite_quad(&s3, &class(s3), &v, 0.0);
        let w = q[3] - q[2];
        assert!(
            w.abs_diff_eq((Vec3::X * 0.85 - Vec3::NEG_Z * 0.15) * 2.0, 1e-6),
            "{w}"
        );
    }

    /// A still tall blade with no lean lies FLAT: both of its axes are
    /// horizontal, `width` long, centred on its point.
    #[test]
    fn a_tall_blade_lies_across_its_leaned_up() {
        let s = sprite(Some(Vec3::X));
        let mut c = class(s);
        c.tall_lean = 0.0;
        let v = ViewAxes {
            right: Vec3::X,
            look: Vec3::NEG_Z,
        };
        let q = sprite_quad(&s, &c, &v, 0.0);
        for k in q {
            assert!((k.y - s.pos.y).abs() < 1e-6, "{q:?}");
        }
        let centre = (q[0] + q[1] + q[2] + q[3]) / 4.0;
        assert!(centre.abs_diff_eq(s.pos, 1e-6));
        assert!(((q[1] - q[0]).length() - 2.0).abs() < 1e-6);
        assert!(((q[0] - q[2]).length() - 2.0).abs() < 1e-5);
        // With a lean the blade tilts: its normal leans toward the camera.
        c.tall_lean = 0.5;
        let q = sprite_quad(&s, &c, &v, 0.0);
        let n = (q[1] - q[0]).cross(q[2] - q[0]).normalize();
        assert!(n.y.abs() < 0.99 && n.y.abs() > 0.8, "{n}");
    }

    /// Drawn from `−bound` to D; full alpha until D − 20, then linear to 0.
    #[test]
    fn a_patch_fades_over_the_last_twenty_metres() {
        assert_eq!(patch_alpha(-6.0, 40.0, 5.0), None);
        assert_eq!(patch_alpha(-5.0, 40.0, 5.0), Some(1.0));
        assert_eq!(patch_alpha(20.0, 40.0, 5.0), Some(1.0));
        assert_eq!(patch_alpha(30.0, 40.0, 5.0), Some(0.5));
        assert_eq!(patch_alpha(40.0, 40.0, 5.0), Some(0.0));
        assert_eq!(patch_alpha(40.1, 40.0, 5.0), None);
    }

    /// The sway rate grows 20 % per step of i mod 5.
    #[test]
    fn the_sway_runs_at_the_ps2_s_rates() {
        let mut s = sprite(None);
        s.index = 7;
        let t = 1.0;
        assert!((sway(&s, t) - (t * 0.8 * 1.4).sin()).abs() < 1e-6);
    }

    /// A patch at the eye's side passes the depth test but is off screen, so
    /// the frustum drops it.
    #[test]
    fn the_frustum_drops_a_patch_out_at_the_side() {
        let classes = vec![GrassClass {
            texture: None,
            sprites: Vec::new(),
            tall_lean: 0.0,
            still: false,
            bound: 1.0,
        }];
        let at = |x: f32, z: f32| GrassPatch {
            class: 0,
            matrix: Mat4::from_translation(Vec3::new(x, 0.0, z)),
            color: [1.0; 4],
            distance: 100.0,
        };
        let patches = vec![at(0.0, -50.0), at(500.0, -50.0)];
        let cam = Camera::default();
        let drawn =
            |view: FoliageView<'_>| build(&classes, &patches, &cam, 0.0, view).instances.len();
        assert_eq!(drawn(FoliageView::ALL), 2);
        let vp = cam.view_proj(1.0);
        let frustum = crate::frustum::Frustum::from_view_proj(&vp);
        let view = FoliageView {
            frustum: Some(&frustum),
        };
        assert_eq!(drawn(view), 1, "the one out at the side is gone");
    }

    /// Patches draw far to near; runs merge consecutive patches of a class.
    #[test]
    fn the_frame_draws_far_to_near_in_class_runs() {
        let s = sprite(None);
        let classes = vec![class(s), class(s)];
        let at = |class: u32, z: f32| GrassPatch {
            class,
            matrix: Mat4::from_translation(Vec3::new(0.0, 0.0, z)),
            color: [1.0; 4],
            distance: 100.0,
        };
        // Camera at the origin looking down −Z: depth = −z.
        let patches = vec![
            at(0, -10.0),
            at(1, -30.0),
            at(0, -50.0),
            at(0, -60.0),
            at(0, 10.0),
        ];
        let f = build(
            &classes,
            &patches,
            &Camera::default(),
            0.0,
            FoliageView::ALL,
        );
        let z: Vec<f32> = f.instances.iter().map(|i| i.model[3][2]).collect();
        assert_eq!(
            z,
            vec![-60.0, -50.0, -30.0, -10.0],
            "the one behind is culled"
        );
        assert_eq!(f.runs.len(), 3);
        assert_eq!(f.runs[0].instances, 0..2);
        assert_eq!(
            f.vertices.len(),
            12,
            "one sprite per class, drawn classes only"
        );
    }
}
