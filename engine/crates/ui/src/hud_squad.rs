//! BF1's squad order cross — the `SquadCommandGroup` element (card SQHUD,
//! `analysis/re-squad-orders.md` §2 and §2b): [`SquadCross`] is what the game
//! feeds per frame, and the draw lays it over [`crate::hud_draw`]'s frame.
//!
//! The PS2's element (§2b [V]): the `squad_cmd_cross` sheet as a 64 × 64 rect
//! at scale 0.5 on the group's origin; per arm a `squad_cmd_highlight` rect
//! of 32 × 32 hung by its bottom edge, at scale 0.5, 16 from the origin and
//! turned so its point is at the origin (up π, down 0, right −π/2, left π/2);
//! per arm a `gamefont_tiny` label 25 from the origin (up: centred, its
//! bottom there; down: centred, its top; right: its left edge, centred on
//! the row; left: its right edge). A press shows the four labels of the
//! order's kind, the chosen arm's label at alpha 255 with its highlight, the
//! others at 127 without, and sets a timer to 1.0 that falls 0.5 a second;
//! the group's alpha is the timer and it hides below 0.
//!
//! Where it sits is measured ([L], three PS2 frames, §2b): [`CENTRE`],
//! vertical sizes at [`Y_UNIT`], the labels' rows by [`LINE`] and [`DROP`].
//! The labels carry the Shadow style's spacing and shadow copy.
//!
//! `[ours]`: the count of units that took the order, drawn beside the chosen
//! arm's label in its font and alpha (the maintainer, INBOX 2026-09-30; FEEL_NOTES);
//! Hor+ — the cross keeps its 4:3 offset from the window's right edge. A BF2
//! battle shows the same cross (the BF2 disc ships neither sheet).

use crate::hud_draw::{color, run_width, Ctx, Quad, Run, Screen};
use crate::hud_tree::{HAlign, FRAME_H, FRAME_W};
use glam::{Mat4, Vec3};

/// The cross's centre on the PS2's 512 × 448 frame [L]. The element's own
/// rule — the scene's safe rect (0.05, 0.05, 0.9, 0.9 [M]) anchored at (1, 1),
/// then (−120, −50) — gives x 366.4 at one pixel per unit; with the plain scene
/// matrix's 1.0221 px per unit (`analysis/re-capture-display.md` §4) it gives
/// x = 0.95 · 512 − 120 · 1.0221 = 363.75 [D]. y (378.9 by the old rule) is
/// still not traced [H].
pub const CENTRE: [f32; 2] = [363.8, 376.6];
/// Frame rows per element unit down the screen [L]: the cross is 32 units
/// wide and 29.9 rows tall, the down arm 20.6 rows of picture long.
pub const Y_UNIT: f32 = 448.0 / 480.0;
/// The cross sheet's side: the 64 × 64 rect at scale 0.5 [V].
pub const CROSS: f32 = 32.0;
/// A highlight's side at its scale 0.5, and how far its hung edge sits from
/// the centre [V].
pub const ARM: f32 = 16.0;
/// A label's anchor from the centre [V].
pub const LABEL: f32 = 25.0;
/// The timer's fall per second [V] (1.0 → 0: 2 s).
pub const FADE_PER_SECOND: f32 = 0.5;
/// An unchosen label's alpha [V] (0x7f).
pub const DIM: f32 = 127.0 / 255.0;
/// A label's row: its height and how far below its top the font's glyph
/// rows start [L] (PROVISIONAL: BF1's text element is not read; the font's
/// own line is 18).
pub const LINE: f32 = 23.2;
pub const DROP: f32 = 3.8;
/// `[ours]` the gap between the chosen label and its count.
pub const COUNT_GAP: f32 = 6.0;
/// BF1's own `gamefont_tiny` as `bake fonts` names it; BF2's stands in for a
/// bake without it.
pub const FONT: &str = "bf1_gamefont_tiny";
const FONT_FALLBACK: &str = "gamefont_tiny";
pub const CROSS_SHEET: &str = "squad_cmd_cross";
pub const HIGHLIGHT_SHEET: &str = "squad_cmd_highlight";

/// The arms in the PS2's order: up, down, right, left.
pub const UP: u8 = 0;
pub const DOWN: u8 = 1;
pub const RIGHT: u8 = 2;
pub const LEFT: u8 = 3;

/// Per arm: the highlight's place and turn [V] (`DAT_00518888`).
const HIGHLIGHTS: [(f32, f32, f32); 4] = [
    (0.0, -ARM, std::f32::consts::PI),
    (0.0, ARM, 0.0),
    (ARM, 0.0, -std::f32::consts::FRAC_PI_2),
    (-ARM, 0.0, std::f32::consts::FRAC_PI_2),
];

/// How a label hangs on its anchor down the screen.
#[derive(Clone, Copy)]
enum Hang {
    Top,
    Centre,
    Bottom,
}

/// Per arm: the label's anchor and alignment [V] (`DAT_00518848`).
const LABELS: [(f32, f32, HAlign, Hang); 4] = [
    (0.0, -LABEL, HAlign::Center, Hang::Bottom),
    (0.0, LABEL, HAlign::Center, Hang::Top),
    (LABEL, 0.0, HAlign::Left, Hang::Centre),
    (-LABEL, 0.0, HAlign::Right, Hang::Centre),
];

/// What the cross shows this frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SquadCross {
    /// The four order names: up, down, right, left.
    pub labels: [String; 4],
    /// The chosen arm ([`UP`] … [`LEFT`]).
    pub arm: u8,
    /// `[ours]` how many units took the order; `None` draws no number.
    pub count: Option<u32>,
    /// The PS2's timer: 1.0 at the press, falling [`FADE_PER_SECOND`].
    pub timer: f32,
}

impl SquadCross {
    /// The timer `seconds` after the press; `None` once the group is hidden.
    pub fn timer_at(seconds: f32) -> Option<f32> {
        let t = 1.0 - FADE_PER_SECOND * seconds;
        (t >= 0.0).then_some(t)
    }

    /// The group's alpha: the timer as the PS2's byte.
    pub fn alpha(&self) -> f32 {
        (self.timer.clamp(0.0, 1.0) * 255.0).floor() / 255.0
    }

    /// Arm `arm`'s label alpha.
    pub fn label_alpha(&self, arm: u8) -> f32 {
        self.alpha() * if arm == self.arm { 1.0 } else { DIM }
    }
}

/// A drawn part of the cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Cross,
    Highlight,
    /// An arm's label.
    Label(u8),
    Count,
}

/// Element units → window pixels for a `size` window: the cross's centre is
/// the origin, and the 4:3 picture keeps its place at the right edge.
pub(crate) fn screen(size: [f32; 2]) -> Screen {
    let box_w = 4.0 / 3.0 * size[1];
    Screen {
        origin: [
            size[0] - box_w + CENTRE[0] / FRAME_W * box_w,
            CENTRE[1] / FRAME_H * size[1],
        ],
        scale: [box_w / FRAME_W, size[1] / FRAME_H * Y_UNIT],
    }
}

/// Where the cross's centre and arm `arm`'s label anchor land in a `size`
/// window.
pub fn centre(size: [f32; 2]) -> [f32; 2] {
    screen(size).map(Vec3::ZERO)
}

pub fn label_anchor(arm: u8, size: [f32; 2]) -> [f32; 2] {
    let (x, y, ..) = LABELS[usize::from(arm.min(3))];
    screen(size).map(Vec3::new(x, y, 0.0))
}

/// A label's glyph run: white, the Shadow style.
fn label(text: &str, at: (f32, f32), h_align: HAlign, alpha: f32) -> Run<'_> {
    Run {
        text,
        at,
        h_align,
        scale: [1.0, 1.0],
        color: color([255.0; 3], alpha),
        additive: false,
        shadow: true,
    }
}

impl Ctx<'_> {
    /// Draw one part and note its window box.
    fn squad_part(&mut self, part: Part, alpha: f32, draw: impl FnOnce(&mut Self)) {
        let first = self.out.verts.len();
        let first_box = self.out.boxes.len();
        draw(self);
        let b = if self.out.boxes.len() > first_box {
            self.out.boxes[first_box..].iter().fold(
                [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
                |b, (_, bx)| {
                    [
                        b[0].min(bx[0]),
                        b[1].min(bx[1]),
                        b[2].max(bx[2]),
                        b[3].max(bx[3]),
                    ]
                },
            )
        } else {
            // A label's shadow copy is black.
            crate::hud_draw::vertex_bounds(
                self.out.verts[first..]
                    .iter()
                    .filter(|v| v.color[..3] != [0.0; 3]),
            )
        };
        if b[0] <= b[2] {
            self.out.squad.push((part, b, alpha));
        }
    }

    pub(crate) fn squad(&mut self, sq: &SquadCross, size: [f32; 2]) {
        let alpha = sq.alpha();
        if sq.timer < 0.0 || alpha <= 0.0 {
            return;
        }
        let s = screen(size);
        let none = usize::MAX;
        let sheet = |rect: [f32; 4], texture: &'static str| Quad {
            rect,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: color([255.0; 3], alpha),
            texture,
            additive: false,
            shadow: false,
            boxed: false,
        };
        let h = CROSS * 0.5;
        self.squad_part(Part::Cross, alpha, |c| {
            c.quad(
                none,
                &Mat4::IDENTITY,
                &s,
                &sheet([-h, -h, h, h], CROSS_SHEET),
            );
        });
        let arm = usize::from(sq.arm.min(3));
        let (x, y, turn) = HIGHLIGHTS[arm];
        let m = Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(turn);
        self.squad_part(Part::Highlight, alpha, |c| {
            c.quad(
                none,
                &m,
                &s,
                &sheet([-ARM * 0.5, -ARM, ARM * 0.5, 0.0], HIGHLIGHT_SHEET),
            );
        });

        let art = self.art;
        let Some(font) = art.fonts.get(FONT).or_else(|| art.fonts.get(FONT_FALLBACK)) else {
            return;
        };
        for (k, (x, y, h_align, hang)) in LABELS.into_iter().enumerate() {
            let top = y + DROP
                - match hang {
                    Hang::Top => 0.0,
                    Hang::Centre => LINE * 0.5,
                    Hang::Bottom => LINE,
                };
            let a = sq.label_alpha(k as u8);
            let text = sq.labels[k].as_str();
            self.squad_part(Part::Label(k as u8), a, |c| {
                c.run(
                    none,
                    &Mat4::IDENTITY,
                    &s,
                    font,
                    &label(text, (x, top), h_align, a),
                );
            });
            let (Some(n), true) = (sq.count, k == arm) else {
                continue;
            };
            // The number sits past the label's outer end: after it, or
            // before the left arm's.
            let w = run_width(font, text, 1.0, true);
            let (at, side) = match h_align {
                HAlign::Left => (x + w + COUNT_GAP, HAlign::Left),
                HAlign::Center => (x + w * 0.5 + COUNT_GAP, HAlign::Left),
                HAlign::Right => (x - w - COUNT_GAP, HAlign::Right),
            };
            let number = n.to_string();
            self.squad_part(Part::Count, a, |c| {
                c.run(
                    none,
                    &Mat4::IDENTITY,
                    &s,
                    font,
                    &label(&number, (at, top), side, a),
                );
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_draw::{draw, HudArt};
    use crate::hud_tree::Hud;
    use manifest::hud::{HudFont, HudGlyph, HudLayout};
    use std::collections::HashMap;

    /// A font of 9 × 13 capitals in rows 5..18, like BF1's tiny one.
    fn art() -> HudArt {
        let glyphs = (b' '..=b'Z')
            .map(|c| HudGlyph {
                code: u16::from(c),
                page: 0,
                advance: 9,
                x0: 0,
                x1: 7,
                y0: 5,
                y1: 18,
                uv: [0.0, 0.1, 0.0, 0.1],
            })
            .collect();
        let font = HudFont {
            name: FONT.into(),
            line_height: 18,
            pages: vec!["page".into()],
            glyphs,
        };
        HudArt {
            fonts: HashMap::from([(FONT.to_string(), font)]),
            ..Default::default()
        }
    }

    fn cross(arm: u8, count: Option<u32>, seconds: f32) -> Option<SquadCross> {
        Some(SquadCross {
            labels: ["MOVE OUT", "HOLD POSITION", "FOLLOW ME", "AT EASE"].map(str::to_owned),
            arm,
            count,
            timer: SquadCross::timer_at(seconds)?,
        })
    }

    fn frame(sq: Option<SquadCross>, size: [f32; 2]) -> crate::hud_draw::HudFrame {
        let none = HudLayout {
            name: "none".into(),
            nodes: vec![],
        };
        let mut hud = Hud::new(&none, &none);
        hud.squad = sq;
        draw(&hud, &art(), size)
    }

    fn part(f: &crate::hud_draw::HudFrame, p: Part) -> Option<([f32; 4], f32)> {
        f.squad.iter().find(|(q, ..)| *q == p).map(|x| (x.1, x.2))
    }

    /// The PS2's fade: the chosen arm at 255, the others at 127, all times
    /// the timer's byte; hidden after 2 s.
    #[test]
    fn the_alpha_is_the_timers() {
        for (t, group) in [(0.0, 255.0), (1.0, 127.0), (1.9, 12.0)] {
            let f = frame(cross(RIGHT, None, t), [640.0, 480.0]);
            let a = group / 255.0;
            assert!((part(&f, Part::Cross).unwrap().1 - a).abs() < 1.0e-6, "{t}");
            assert!((part(&f, Part::Label(RIGHT)).unwrap().1 - a).abs() < 1.0e-6);
            assert!((part(&f, Part::Label(UP)).unwrap().1 - a * DIM).abs() < 1.0e-6);
            assert!(part(&f, Part::Count).is_none());
        }
        // At 2.0 s the timer is 0: nothing; after it the group is hidden.
        assert!(frame(cross(RIGHT, None, 2.0), [640.0, 480.0])
            .verts
            .is_empty());
        assert!(cross(RIGHT, None, 2.01).is_none());
    }

    /// The cross is 32 × 32 units on the centre, each highlight 16 × 16 with
    /// its point at the centre, each label 25 units out.
    #[test]
    fn the_parts_sit_by_the_ps2s_tables() {
        let size = [640.0, 480.0];
        let (kx, ky) = (640.0 / 512.0, 480.0 / 448.0 * Y_UNIT);
        let c = centre(size);
        assert!((c[0] - CENTRE[0] * 1.25).abs() < 1.0e-3);
        assert!((c[1] - CENTRE[1] * 480.0 / 448.0).abs() < 1.0e-3);
        let near = |got: [f32; 4], want: [f32; 4]| {
            for k in 0..4 {
                let w = c[k % 2] + want[k] * if k % 2 == 0 { kx } else { ky };
                assert!((got[k] - w).abs() < 1.0e-2, "{got:?} vs {want:?} [{k}]");
            }
        };
        let f = frame(cross(RIGHT, None, 0.0), size);
        near(part(&f, Part::Cross).unwrap().0, [-16.0, -16.0, 16.0, 16.0]);
        for (arm, want) in [
            (UP, [-8.0, -16.0, 8.0, 0.0]),
            (DOWN, [-8.0, 0.0, 8.0, 16.0]),
            (RIGHT, [0.0, -8.0, 16.0, 8.0]),
            (LEFT, [-16.0, -8.0, 0.0, 8.0]),
        ] {
            let f = frame(cross(arm, None, 0.0), size);
            near(part(&f, Part::Highlight).unwrap().0, want);
        }
        // Labels: 13 rows of capitals, 5 below the font's line top.
        let (w_up, w_right) = (8.0 * 10.0, 9.0 * 10.0);
        let rows = |top: f32| (top + DROP + 5.0, top + DROP + 18.0);
        let (t, b) = rows(-LABEL - LINE);
        near(
            part(&f, Part::Label(UP)).unwrap().0,
            [-w_up / 2.0, t, w_up / 2.0 - 3.0, b],
        );
        let (t, b) = rows(LABEL);
        let w_down = 13.0 * 10.0;
        near(
            part(&f, Part::Label(DOWN)).unwrap().0,
            [-w_down / 2.0, t, w_down / 2.0 - 3.0, b],
        );
        let (t, b) = rows(-LINE / 2.0);
        near(
            part(&f, Part::Label(RIGHT)).unwrap().0,
            [LABEL, t, LABEL + w_right - 3.0, b],
        );
        near(
            part(&f, Part::Label(LEFT)).unwrap().0,
            [-LABEL - 70.0, t, -LABEL - 3.0, b],
        );
    }

    /// `[ours]` the count: past the chosen label's outer end, in its alpha.
    #[test]
    fn the_count_sits_beside_the_chosen_label() {
        let size = [640.0, 480.0];
        for arm in [UP, DOWN, RIGHT, LEFT] {
            let f = frame(cross(arm, Some(2), 1.0), size);
            let (label, a) = part(&f, Part::Label(arm)).unwrap();
            let (count, ca) = part(&f, Part::Count).expect("a count");
            assert_eq!(a, ca);
            assert_eq!((label[1], label[3]), (count[1], count[3]), "one row");
            if arm == LEFT {
                assert!(count[2] < label[0], "before the left label");
            } else {
                assert!(count[0] > label[2], "after the label");
            }
        }
        assert!(part(&frame(cross(UP, None, 1.0), size), Part::Count).is_none());
    }

    /// Hor+: the cross keeps its distance from the right edge.
    #[test]
    fn a_wide_window_keeps_the_right_edge_offset() {
        let narrow = centre([640.0, 480.0]);
        let wide = centre([1280.0, 720.0]);
        assert!(((640.0 - narrow[0]) * 1.5 - (1280.0 - wide[0])).abs() < 1.0e-3);
        assert!((narrow[1] * 1.5 - wide[1]).abs() < 1.0e-3);
        let a = label_anchor(RIGHT, [1280.0, 720.0]);
        assert!((a[0] - wide[0] - LABEL * 960.0 / 512.0).abs() < 1.0e-3);
    }
}
