//! The command-post capture icon — the PS2's `CaptureDisplay` (card CPBAR,
//! `analysis/re-capture-display.md`): [`CaptureIcon`] is what the game feeds
//! per frame (`game::capture_feed` makes it from the post the player stands
//! in), and the draw lays it over [`crate::hud_draw`]'s frame.
//!
//! The PS2's element (§3.3 [V][O]): the post symbol `hud_flag_timer`
//! (64 × 64, white, its alpha the ring and spokes) as one picture cut by a
//! horizontal line at row `split` × 64: the texture's rows above the line in
//! the top colour, the rows below in the bottom colour. The group hangs at
//! (0.95, 0.5) of the frame (§4 [V]); on the 512 × 448 frame the picture is
//! [`BOX`] (§4 [D][O], measured on both games' frames to under a pixel;
//! PROVISIONAL as the doc says: ELF and RAM constants combined).
//!
//! `[ours]` Hor+: the picture keeps its 4:3 offset from the window's right
//! edge, as the squad cross does. `[ours]` [`Mark`]: a pointer beside the
//! line that says which way it moves (the maintainer's "which way", which the PS2 does
//! not answer; FEEL_NOTES).

use crate::hud_draw::{color, Ctx, Frame, Quad, Screen, VAnchor};
use crate::hud_tree::{FRAME_H, FRAME_W};
use glam::{Mat4, Vec3};

/// The picture on the PS2's 512 × 448 frame: x0, y0, x1, y1 (§4: its right
/// edge at 0.95 × 512, 64 units at 1.0221 frame pixels across, its top 30
/// rows above mid height).
pub const BOX: [f32; 4] = [420.99, 194.0, 486.40, 258.0];
pub const SHEET: &str = "hud_flag_timer";
/// `[ours]` the direction mark: the minimap's north pointer (it points up),
/// [`MARK_SIZE`] frame pixels square, its centre [`MARK_GAP`] left of the
/// picture's visible left edge, on the line.
pub const MARK_SHEET: &str = "hud_map_northpointer";
pub const MARK_SIZE: f32 = 14.0;
pub const MARK_GAP: f32 = 4.0;
/// The texture's visible columns start this far into the 64 (§4 [M]: the
/// alpha covers columns 5..59).
const VISIBLE_LEFT: f32 = 5.0 / 64.0;

/// A colour of the PS2's team table as the viewer sees it (§3.2): the
/// minimap's and the event lines' colours (`crate::hud::message_color`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Team index 0: nobody's.
    Neutral,
    /// The viewer's own team.
    Own,
    Enemy,
}

impl Side {
    /// BF2's table on every map (the port's HUD is BF2's).
    pub fn rgb(self) -> [u8; 3] {
        use crate::hud::message_color::{ENEMY, NEUTRAL, OWN};
        match self {
            Side::Neutral => NEUTRAL,
            Side::Own => OWN,
            Side::Enemy => ENEMY,
        }
    }
}

/// `[ours]` which way the line moves this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Up,
    Down,
}

/// What the icon shows this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureIcon {
    /// The line, 0 (top) .. 1 (bottom) of the picture.
    pub split: f32,
    pub top: Side,
    pub bottom: Side,
    /// `[ours]` `None` = the line holds (or is frozen).
    pub mark: Option<Mark>,
}

/// A drawn part of the icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Top,
    Bottom,
    Mark,
}

/// Frame pixels → window pixels for a `size` window: the 4:3 picture keeps
/// its place at the right edge.
/// In a split cell ([`Frame::cell`]) the box is the frame's, at the cell's
/// top-right, so the picture stays under the minimap, flags and timer column
/// (all `VAnchor::Top`).
pub(crate) fn screen(f: &Frame) -> Screen {
    let box_w = 4.0 / 3.0 * f.box_h;
    Screen {
        origin: [f.size[0] - box_w, f.shift_y(VAnchor::Top)],
        scale: [box_w / FRAME_W, f.box_h / FRAME_H],
    }
}

/// The picture's window box in a `size` window.
pub fn picture(size: [f32; 2]) -> [f32; 4] {
    picture_in(&Frame::window(size))
}

/// [`picture`] in the screen `f`.
pub fn picture_in(f: &Frame) -> [f32; 4] {
    let s = screen(f);
    let a = s.map(Vec3::new(BOX[0], BOX[1], 0.0));
    let b = s.map(Vec3::new(BOX[2], BOX[3], 0.0));
    [a[0], a[1], b[0], b[1]]
}

impl Ctx<'_> {
    fn capture_part(&mut self, part: Part, rgb: [u8; 3], draw: impl FnOnce(&mut Self)) {
        let first = self.out.verts.len();
        draw(self);
        // The mark's shadow copy is black.
        let b = crate::hud_draw::vertex_bounds(
            self.out.verts[first..]
                .iter()
                .filter(|v| v.color[..3] != [0.0; 3]),
        );
        if b[0] <= b[2] {
            self.out.capture.push((part, b, rgb));
        }
    }

    pub(crate) fn capture(&mut self, icon: &CaptureIcon, f: &Frame) {
        let s = screen(f);
        let none = usize::MAX;
        let f = icon.split.clamp(0.0, 1.0);
        let [x0, y0, x1, y1] = BOX;
        let y = y0 + f * (y1 - y0);
        let sheet = |rect: [f32; 4], v: [f32; 2], rgb: [u8; 3]| Quad {
            rect,
            uv: [0.0, v[0], 1.0, v[1]],
            color: color(rgb.map(f32::from), 1.0),
            texture: SHEET,
            additive: false,
            shadow: false,
            boxed: false,
        };
        // The texture's rows 0..f in the top colour, f..1 in the bottom's
        // (§3.3); an empty part draws nothing.
        for (part, side, rect, v) in [
            (Part::Top, icon.top, [x0, y0, x1, y], [0.0, f]),
            (Part::Bottom, icon.bottom, [x0, y, x1, y1], [f, 1.0]),
        ] {
            if rect[3] - rect[1] <= 0.0 {
                continue;
            }
            let rgb = side.rgb();
            self.capture_part(part, rgb, |c| {
                c.quad(none, &Mat4::IDENTITY, &s, &sheet(rect, v, rgb));
            });
        }
        let Some(mark) = icon.mark else {
            return;
        };
        let cx = x0 + VISIBLE_LEFT * (x1 - x0) - MARK_GAP - MARK_SIZE * 0.5;
        let turn = match mark {
            Mark::Up => 0.0,
            Mark::Down => std::f32::consts::PI,
        };
        let m = Mat4::from_translation(Vec3::new(cx, y, 0.0)) * Mat4::from_rotation_z(turn);
        let h = MARK_SIZE * 0.5;
        let white = [255u8; 3];
        self.capture_part(Part::Mark, white, |c| {
            c.quad(
                none,
                &m,
                &s,
                &Quad {
                    rect: [-h, -h, h, h],
                    uv: [0.0, 0.0, 1.0, 1.0],
                    color: color([255.0; 3], 1.0),
                    texture: MARK_SHEET,
                    additive: false,
                    shadow: true,
                    boxed: false,
                },
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_draw::{draw, HudArt, HudFrame};
    use crate::hud_tree::Hud;
    use manifest::hud::HudLayout;

    fn frame(icon: Option<CaptureIcon>, size: [f32; 2]) -> HudFrame {
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        let mut hud = Hud::new(&none, &none);
        hud.capture = icon;
        draw(&hud, &HudArt::default(), size)
    }

    fn part(f: &HudFrame, p: Part) -> Option<([f32; 4], [u8; 3])> {
        f.capture.iter().find(|(q, ..)| *q == p).map(|x| (x.1, x.2))
    }

    fn icon(split: f32, mark: Option<Mark>) -> Option<CaptureIcon> {
        Some(CaptureIcon {
            split,
            top: Side::Neutral,
            bottom: Side::Own,
            mark,
        })
    }

    /// §4: at 640 × 480 the visible symbol (columns 5..59, rows 4..58 of the
    /// 64) is x 532.6..602.9, y 212.1..271.1 — within 2 px of the PS2's
    /// frames' box x 533..603, y 212..272.
    #[test]
    fn the_symbol_sits_where_the_ps2_draws_it() {
        let [x0, y0, x1, y1] = picture([640.0, 480.0]);
        let (w, h) = (x1 - x0, y1 - y0);
        let vis = [
            x0 + w * 5.0 / 64.0,
            y0 + h * 4.0 / 64.0,
            x0 + w * 60.0 / 64.0,
            y0 + h * 59.0 / 64.0,
        ];
        for (got, want) in vis.into_iter().zip([533.0, 212.0, 603.0, 272.0]) {
            assert!((got - want).abs() <= 2.0, "{vis:?}");
        }
        assert!((vis[0] - 532.6).abs() < 0.1 && (vis[2] - 602.9).abs() < 0.1);
        assert!((vis[1] - 212.1).abs() < 0.1 && (vis[3] - 271.1).abs() < 0.1);
    }

    /// One picture cut at the line: the top part the texture's rows 0..f in
    /// the top colour, the bottom part rows f..1 in the bottom's.
    #[test]
    fn the_line_cuts_one_picture_in_two() {
        let size = [640.0, 480.0];
        let p = picture(size);
        let f = frame(icon(0.25, None), size);
        let (top, trgb) = part(&f, Part::Top).unwrap();
        let (bot, brgb) = part(&f, Part::Bottom).unwrap();
        assert_eq!(trgb, Side::Neutral.rgb());
        assert_eq!(brgb, Side::Own.rgb());
        let line = p[1] + 0.25 * (p[3] - p[1]);
        assert!((top[1] - p[1]).abs() < 1e-3 && (top[3] - line).abs() < 1e-3);
        assert!((bot[1] - line).abs() < 1e-3 && (bot[3] - p[3]).abs() < 1e-3);
        assert_eq!((top[0], top[2]), (p[0], p[2]));
        // The texture rows follow the line.
        let vs: Vec<f32> = f.verts.iter().map(|v| v.uv[1]).collect();
        assert_eq!(vs, [0.0, 0.0, 0.25, 0.25, 0.25, 0.25, 1.0, 1.0]);
        // All grey or all filled: one part.
        assert!(part(&frame(icon(1.0, None), size), Part::Bottom).is_none());
        assert!(part(&frame(icon(0.0, None), size), Part::Top).is_none());
        assert!(frame(None, size).verts.is_empty());
    }

    /// `[ours]` the mark: left of the symbol, on the line, pointing the
    /// way the line goes.
    #[test]
    fn the_mark_sits_on_the_line() {
        let size = [640.0, 480.0];
        let p = picture(size);
        for (mark, turned) in [(Mark::Up, false), (Mark::Down, true)] {
            let f = frame(icon(0.5, Some(mark)), size);
            let (b, _) = part(&f, Part::Mark).unwrap();
            assert!(
                b[2] < p[0] + (p[2] - p[0]) * 5.0 / 64.0,
                "left of the symbol"
            );
            let mid = (b[1] + b[3]) * 0.5;
            assert!((mid - (p[1] + p[3]) * 0.5).abs() < 1e-3);
            // The pointer's texture top (v 0) is up for Up, down for Down.
            let v0 = f
                .verts
                .iter()
                .rev()
                .take(4)
                .find(|v| v.uv[1] == 0.0)
                .unwrap();
            assert_eq!(v0.pos[1] > mid, turned);
        }
        assert!(part(&frame(icon(0.5, None), size), Part::Mark).is_none());
    }

    /// Hor+: the picture keeps its distance from the right edge, not
    /// stretched.
    #[test]
    fn a_wide_window_keeps_the_right_edge_offset() {
        let n = picture([640.0, 480.0]);
        let w = picture([1280.0, 720.0]);
        assert!(((640.0 - n[2]) * 1.5 - (1280.0 - w[2])).abs() < 1e-3);
        assert!(((n[2] - n[0]) * 1.5 - (w[2] - w[0])).abs() < 1e-3);
        assert!(((n[3] - n[1]) * 1.5 - (w[3] - w[1])).abs() < 1e-3);
        assert!((n[1] * 1.5 - w[1]).abs() < 1e-3);
    }
}
