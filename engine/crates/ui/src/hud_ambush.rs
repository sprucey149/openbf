//! Where the shot came from — the PS2's hit-direction chevrons (card HATK,
//! `analysis/re-hud-attacker.md`): when the player's unit is hit, the
//! `ambush_icon` chevron flashes at the screen edge toward the attacker.
//! [`Ambush`] holds the four edges' timers inside [`crate::hud_tree::Hud`];
//! the game hands it each hit's direction in the viewer's own frame
//! ([`edge`]), the HUD's advance fades it, and the draw lays it over
//! [`crate::hud_draw`]'s frame.
//!
//! The PS2's rule (§1, §3 [V][O], B2 `FUN_00391680` / B1 `FUN_002d8028`):
//! the shot's travel `v` in the victim's (left, up, forward) rows, s = v0 +
//! v2, d = v2 − v0: s ≥ 0 and d < 0 → the right edge; s ≥ 0 and d ≥ 0 → the
//! bottom when v1 < 0, else the top; s < 0 and d ≥ 0 → the left edge; s < 0
//! and d < 0 (the attacker ahead) → nothing. The edge's timer goes to 1.0
//! and falls 1.0 a second (`FUN_003915a8`, DAT_005ff308); the alpha is the
//! timer. The four edges are independent; a new hit restarts its edge.
//!
//! Places (§7 [M], PROVISIONAL as the doc says: measured on PS2 frames, not
//! derived from the code): on a 640 × 480 frame the side chevrons are
//! [`SIDE`] (17 px thick, 92 long, centred at y 241, the apex 33 px in from
//! the edge); the bottom one is [`END`] (13 × 113, centred, the apex 23 px
//! above the bottom); `[D]` the top one mirrors the bottom (the PS2's
//! records place them symmetrically, 0.0 and 1.0 of the viewport).
//! `[ours]` Hor+: sizes scale with the window height, the side chevrons keep
//! their offset from the window's left / right edges, the end ones centre.

use crate::hud_draw::{color, Ctx, Frame, Quad, Screen, VAnchor};
use glam::{Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

pub const SHEET: &str = "ambush_icon";
/// The timer falls this much a second (DAT_005ff308 = 1.0, §1 [V]).
pub const FADE: f32 = 1.0;

/// A chevron's visible size on the PS2's 640 × 480 frame: (along the edge,
/// across it) and the apex's distance in from the edge (§7 [M]).
pub struct Place {
    pub long: f32,
    pub thick: f32,
    pub inset: f32,
}
pub const SIDE: Place = Place {
    long: 92.0,
    thick: 17.0,
    inset: 33.0,
};
pub const END: Place = Place {
    long: 113.0,
    thick: 13.0,
    inset: 23.0,
};
/// The side chevrons' centre height on the 480-line frame (§7 [M]).
pub const SIDE_Y: f32 = 241.0;

/// The sheet's visible texels (`ambush_icon`, 64 × 32): columns 10..=53,
/// rows 3..=28; the chevron points down (its apex is row 28's).
const VIS_U: [f32; 2] = [10.0 / 64.0, 54.0 / 64.0];
const VIS_V: [f32; 2] = [3.0 / 32.0, 29.0 / 32.0];

/// An edge, in the PS2's sprite order (`FUN_003930b8`, §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Right = 0,
    Bottom = 1,
    Left = 2,
    Top = 3,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Right, Edge::Bottom, Edge::Left, Edge::Top];

    /// The sheet's turn: its apex (down at 0) toward the edge (the
    /// records' angles, §1: right −π/2, left +π/2, top π).
    fn turn(self) -> f32 {
        match self {
            Edge::Right => -FRAC_PI_2,
            Edge::Bottom => 0.0,
            Edge::Left => FRAC_PI_2,
            Edge::Top => PI,
        }
    }
}

/// The edge a hit lights: `v` is the way the shot travelled in the victim's
/// frame (x left, y up, z forward). `None` from the front, and `[ours]` for
/// a hit with no direction (a blast centred on the victim).
pub fn edge(v: Vec3) -> Option<Edge> {
    if v == Vec3::ZERO {
        return None;
    }
    let s = v.x + v.z;
    let d = v.z - v.x;
    match (s >= 0.0, d >= 0.0) {
        (true, false) => Some(Edge::Right),
        (true, true) if v.y < 0.0 => Some(Edge::Bottom),
        (true, true) => Some(Edge::Top),
        (false, true) => Some(Edge::Left),
        (false, false) => None,
    }
}

/// The four edges' timers: above 0 shown, at that alpha.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ambush {
    pub timers: [f32; 4],
}

impl Ambush {
    /// A hit on `e`: shown at full, its fade restarted.
    pub fn hit(&mut self, e: Edge) {
        self.timers[e as usize] = 1.0;
    }

    pub fn advance(&mut self, dt: f32) {
        for t in &mut self.timers {
            if *t > 0.0 {
                *t = (*t - dt * FADE).max(0.0);
            }
        }
    }

    pub fn alpha(&self, e: Edge) -> f32 {
        self.timers[e as usize].clamp(0.0, 1.0)
    }
}

/// The chevron's quad for `e` in a `size` window: the transform that puts
/// its apex on the screen and turns it, and the sheet's full rect in window
/// pixels about the apex (the visible texels sized as [`Place`]).
///
/// In a split cell ([`Frame::cell`]) the chevrons are sized by the frame's
/// box, the side ones at the middle of the cell's height, the top and
/// bottom ones at the cell's own edges.
fn placement(e: Edge, f: &Frame) -> (Mat4, [f32; 4]) {
    let size = f.size;
    let k = f.box_h / 480.0;
    let side_y = {
        let y = SIDE_Y * k;
        let dy = f.shift_y(VAnchor::Centre);
        if dy == 0.0 {
            y
        } else {
            y + dy
        }
    };
    let p = match e {
        Edge::Left | Edge::Right => &SIDE,
        Edge::Top | Edge::Bottom => &END,
    };
    let apex = match e {
        Edge::Left => [p.inset * k, side_y],
        Edge::Right => [size[0] - p.inset * k, side_y],
        Edge::Bottom => [size[0] * 0.5, size[1] - p.inset * k],
        Edge::Top => [size[0] * 0.5, p.inset * k],
    };
    // The whole sheet from its visible part: across the edge from the
    // apex's row, along it centred.
    let w = p.long * k / (VIS_U[1] - VIS_U[0]);
    let h = p.thick * k / (VIS_V[1] - VIS_V[0]);
    let rect = [-0.5 * w, -VIS_V[1] * h, 0.5 * w, (1.0 - VIS_V[1]) * h];
    let m =
        Mat4::from_translation(Vec3::new(apex[0], apex[1], 0.0)) * Mat4::from_rotation_z(e.turn());
    (m, rect)
}

impl Ctx<'_> {
    pub(crate) fn ambush(&mut self, a: &Ambush, f: &Frame) {
        let s = Screen {
            origin: [0.0, 0.0],
            scale: [1.0, 1.0],
        };
        for e in Edge::ALL {
            let alpha = a.alpha(e);
            if alpha <= 0.0 {
                continue;
            }
            let (m, rect) = placement(e, f);
            let first = self.out.verts.len();
            self.quad(
                usize::MAX,
                &m,
                &s,
                &Quad {
                    rect,
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: color([255.0; 3], alpha),
                    texture: SHEET,
                    additive: false,
                    shadow: false,
                    boxed: false,
                },
            );
            let b = crate::hud_draw::vertex_bounds(&self.out.verts[first..]);
            self.out.ambush.push((e, b, alpha));
        }
    }
}

/// The visible chevron's window box for `e` (the sheet's drawn texels).
pub fn visible_box(e: Edge, size: [f32; 2]) -> [f32; 4] {
    visible_box_in(e, &Frame::window(size))
}

/// [`visible_box`] in the screen `f`.
pub fn visible_box_in(e: Edge, f: &Frame) -> [f32; 4] {
    let (m, rect) = placement(e, f);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let corners = [
        (
            lerp(rect[0], rect[2], VIS_U[0]),
            lerp(rect[1], rect[3], VIS_V[0]),
        ),
        (
            lerp(rect[0], rect[2], VIS_U[1]),
            lerp(rect[1], rect[3], VIS_V[1]),
        ),
    ];
    let [a, b] = corners.map(|(x, y)| m.transform_point3(Vec3::new(x, y, 0.0)));
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_draw::{draw, HudArt, HudFrame};
    use crate::hud_tree::Hud;
    use manifest::hud::HudLayout;

    /// A shot travelling at `deg` degrees from the victim's forward (a
    /// positive turn toward his left), with `up` of vertical.
    fn shot(deg: f32, up: f32) -> Vec3 {
        let (s, c) = deg.to_radians().sin_cos();
        Vec3::new(s, up, c)
    }

    /// §1 / §3: the attacker's side lights its edge; from behind the bottom
    /// (level or down) or the top (travelling up); from the front nothing;
    /// the quadrants meet at 45°.
    #[test]
    fn the_shot_lights_the_edge_toward_the_attacker() {
        // A shot travelling toward the victim's left came from his right.
        assert_eq!(edge(shot(90.0, 0.0)), Some(Edge::Right));
        assert_eq!(edge(shot(-90.0, 0.0)), Some(Edge::Left));
        // Travelling forward: the attacker is behind.
        assert_eq!(edge(shot(0.0, -0.1)), Some(Edge::Bottom));
        assert_eq!(edge(shot(0.0, 0.2)), Some(Edge::Top));
        // Travelling back at the victim: the attacker is ahead, on screen.
        assert_eq!(edge(shot(180.0, 0.0)), None);
        assert_eq!(edge(shot(180.0, -0.5)), None);
        // The 45° lines.
        assert_eq!(edge(shot(44.0, -0.1)), Some(Edge::Bottom));
        assert_eq!(edge(shot(46.0, -0.1)), Some(Edge::Right));
        assert_eq!(edge(shot(-44.0, -0.1)), Some(Edge::Bottom));
        assert_eq!(edge(shot(-46.0, -0.1)), Some(Edge::Left));
        assert_eq!(edge(shot(134.0, 0.0)), Some(Edge::Right));
        assert_eq!(edge(shot(136.0, 0.0)), None);
        assert_eq!(edge(shot(-134.0, 0.0)), Some(Edge::Left));
        assert_eq!(edge(shot(-136.0, 0.0)), None);
        // Exactly level from behind: the code's `v1 < 0` test says top.
        assert_eq!(edge(shot(0.0, 0.0)), Some(Edge::Top));
        assert_eq!(edge(Vec3::ZERO), None);
    }

    /// Full on a hit, a linear 1.0 s fade, the edges apart, a new hit
    /// restarts its own edge only.
    #[test]
    fn a_hit_fades_out_in_a_second() {
        let mut a = Ambush::default();
        a.hit(Edge::Left);
        assert_eq!(a.alpha(Edge::Left), 1.0);
        a.advance(0.25);
        assert!((a.alpha(Edge::Left) - 0.75).abs() < 1e-6);
        a.hit(Edge::Top);
        a.advance(0.5);
        assert!((a.alpha(Edge::Left) - 0.25).abs() < 1e-6);
        assert!((a.alpha(Edge::Top) - 0.5).abs() < 1e-6);
        a.hit(Edge::Left);
        assert_eq!(a.alpha(Edge::Left), 1.0);
        a.advance(1.0);
        assert_eq!(a.timers, [0.0; 4]);
        assert_eq!(a.alpha(Edge::Right), 0.0);
    }

    fn frame(a: Ambush, size: [f32; 2]) -> HudFrame {
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        let mut hud = Hud::new(&none, &none);
        hud.ambush = a;
        draw(&hud, &HudArt::default(), size)
    }

    /// §7: the visible chevrons sit where the PS2's frames show them on
    /// 640 × 480, pointing out at their edges.
    #[test]
    fn the_chevrons_sit_where_the_ps2_draws_them() {
        let size = [640.0, 480.0];
        let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01);
        let l = visible_box(Edge::Left, size);
        assert!(near(l, [33.0, 195.0, 50.0, 287.0]), "{l:?}");
        let r = visible_box(Edge::Right, size);
        assert!(near(r, [590.0, 195.0, 607.0, 287.0]), "{r:?}");
        let b = visible_box(Edge::Bottom, size);
        assert!(near(b, [263.5, 444.0, 376.5, 457.0]), "{b:?}");
        let t = visible_box(Edge::Top, size);
        assert!(near(t, [263.5, 23.0, 376.5, 36.0]), "{t:?}");
        // The sheet's apex (v at the visible bottom row) is the vertex side
        // nearest the edge: the left chevron's v = 1 corners are left of its
        // v = 0 ones.
        let mut a = Ambush::default();
        a.hit(Edge::Left);
        let f = frame(a, size);
        assert_eq!(f.verts.len(), 4);
        let x = |v: f32| f.verts.iter().find(|q| q.uv[1] == v).unwrap().pos[0];
        assert!(x(1.0) < x(0.0));
        assert_eq!(f.ambush.len(), 1);
        assert_eq!(f.ambush[0].0, Edge::Left);
        assert!(frame(Ambush::default(), size).verts.is_empty());
    }

    /// Hor+: a wide window keeps the side chevrons' offset from its own
    /// edges and the end ones centred, sizes by the height.
    #[test]
    fn a_wide_window_anchors_the_sides_to_its_edges() {
        let n = [640.0, 480.0];
        let w = [1920.0, 720.0];
        let (ln, lw) = (visible_box(Edge::Left, n), visible_box(Edge::Left, w));
        assert!((ln[0] * 1.5 - lw[0]).abs() < 1e-3);
        let (rn, rw) = (visible_box(Edge::Right, n), visible_box(Edge::Right, w));
        assert!(((640.0 - rn[2]) * 1.5 - (1920.0 - rw[2])).abs() < 1e-3);
        assert!(((rn[3] - rn[1]) * 1.5 - (rw[3] - rw[1])).abs() < 1e-3);
        let bw = visible_box(Edge::Bottom, w);
        assert!(((bw[0] + bw[2]) * 0.5 - 960.0).abs() < 1e-3);
    }
}
