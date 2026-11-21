//! A Text element's lines: the TextBox it is laid out in, the words broken
//! to the box's width, and the `{Name}` control tokens drawn as their
//! button.
//!
//! The PS2's Text layout (`FUN_005a7c00`'s loop over `FUN_005a74f0`'s
//! lines): the text object keeps the box at +0xa0 / +0xa2 (`FUN_005a5b98`,
//! the TextBox × 512, 448) and the alignment in +0xb8 bits 16..19
//! (`FUN_005a5bd0`). Each line's x is 0 (Left), box − line (Right) or
//! (box − line) / 2 (Center); the block's y is 0 (Top), box − lines × pitch
//! (Bottom) or half that (Center). With no box both sizes are 0, so a line
//! hangs on the element's origin as before. Every TextBreak in the shipped
//! layouts is "Word", so a boxed Text always breaks between words.
//! [H] the breaking itself (`FUN_005a74f0`) is read as greedy word wrap;
//! trace: `FUN_005a74f0` for its rule on a word wider than the box.

use super::{color, run_width, Ctx, Quad, Run, Screen};
use crate::hud_tree::{Elem, HAlign, VAlign};
use crate::input_icons::{control_token, pad_sheet, IconSet, Icons, PadButton, Prompt};
use glam::Mat4;
use manifest::hud::HudFont;

/// A run of text, or a control token's button sheet.
#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Text(&'a str),
    Icon(&'static str),
}

/// `FUN_005a73e0`: `{Name}` becomes the glyph `FUN_005a61b8` maps the
/// name's hash to, and a name with none becomes a space. `FUN_0042e940`
/// maps `GuiAccept` to 0x800b and `GuiCancel` to 0x8009, the pad's accept
/// and cancel, which [`Icons`] draws as the set's accept and back.
/// OURS: the glyph is drawn as that sheet. replace: the 0x80xx glyphs'
/// own art. The port maps gameplay tokens to its current action bindings;
/// trace: the remaining control names of `DAT_00600c18` (`FUN_0042e8e8`).
fn pieces<'a>(text: &'a str, icons: &'a Icons) -> Vec<Piece<'a>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        if open > 0 {
            out.push(Piece::Text(&rest[..open]));
        }
        let Some(close) = rest[open..].find('}') else {
            out.push(Piece::Text(&rest[open..]));
            return out;
        };
        let token = &rest[open + 1..open + close];
        out.push(match token.to_ascii_lowercase().as_str() {
            "guiaccept" => Piece::Icon(icons.sheet(Prompt::Accept)),
            "guicancel" => Piece::Icon(icons.sheet(Prompt::Back)),
            "map" | "btn_select" => {
                if icons.set == IconSet::KeyboardMouse {
                    Piece::Text("[Tab]")
                } else {
                    Piece::Icon(pad_sheet(icons.set, PadButton::Select))
                }
            }
            "btn_start" => {
                if icons.set == IconSet::KeyboardMouse {
                    Piece::Text("[Esc]")
                } else {
                    Piece::Icon(pad_sheet(icons.set, PadButton::Start))
                }
            }
            "btn_analog_l" => Piece::Text(if icons.set == IconSet::KeyboardMouse {
                "[WASD]"
            } else {
                "[Left stick]"
            }),
            "btn_analog_r" => Piece::Text(if icons.set == IconSet::KeyboardMouse {
                "[Mouse]"
            } else {
                "[Right stick]"
            }),
            _ => match control_token(token) {
                Some(k) if icons.set == IconSet::KeyboardMouse => Piece::Text(icons.keys[k].text()),
                Some(k) => Piece::Icon(pad_sheet(icons.set, icons.controls[k])),
                _ => Piece::Text(" "),
            },
        });
        rest = &rest[open + close + 1..];
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// The lines `text` breaks into at `width` (0: no box, one line per `\n`).
pub(super) fn wrap(text: &str, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width > 0.0 && !line.is_empty() && measure(&candidate) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

impl Ctx<'_> {
    pub(super) fn elem_text(&mut self, i: usize, e: &Elem, m: &Mat4, s: &Screen, alpha: f32) {
        let Some(font) = self.art.fonts.get(&e.font) else {
            return;
        };
        let height = f32::from(font.line_height) * e.text_scale[1];
        let measure = |t: &str| self.width(font, t, e.text_scale);
        let lines = wrap(&e.text, e.rect.w, &measure);
        let block = height * lines.len() as f32;
        let top = match e.v_align {
            VAlign::Top => 0.0,
            VAlign::Center => (e.rect.h - block) * 0.5,
            VAlign::Bottom => e.rect.h - block,
        };
        let x = match e.h_align {
            HAlign::Left => 0.0,
            HAlign::Center => e.rect.w * 0.5,
            HAlign::Right => e.rect.w,
        };
        let col = color(self.hud.drawn(i), alpha);
        for (k, line) in lines.iter().enumerate() {
            self.token_run(
                i,
                m,
                s,
                font,
                &Run {
                    text: line,
                    at: (x, top + height * k as f32),
                    h_align: e.h_align,
                    scale: e.text_scale,
                    color: col,
                    additive: e.additive,
                    shadow: false,
                    tracking: 0.0,
                },
            );
        }
    }

    /// A button sheet's size in a line of `font`: OURS the line's height and
    /// the sheet's own aspect. replace: the 0x80xx glyph's metrics.
    fn icon_size(&self, font: &HudFont, sheet: &str, ky: f32) -> (f32, f32) {
        let h = f32::from(font.line_height) * ky;
        (
            h * self.art.sheet_aspect.get(sheet).copied().unwrap_or(1.0),
            h,
        )
    }

    fn width(&self, font: &HudFont, text: &str, scale: [f32; 2]) -> f32 {
        pieces(text, &self.art.icons)
            .iter()
            .map(|p| match p {
                Piece::Text(t) => run_width(font, t, scale[0], false),
                Piece::Icon(sheet) => self.icon_size(font, sheet, scale[1]).0,
            })
            .sum()
    }

    /// [`Ctx::run`] with the control tokens drawn as their buttons.
    pub(crate) fn token_run(
        &mut self,
        i: usize,
        m: &Mat4,
        s: &Screen,
        font: &HudFont,
        r: &Run<'_>,
    ) {
        if !r.text.contains('{') {
            self.run(i, m, s, font, r);
            return;
        }
        let parts = pieces(r.text, &self.art.icons);
        let total: f32 = parts
            .iter()
            .map(|p| match p {
                Piece::Text(t) => run_width(font, t, r.scale[0], r.shadow),
                Piece::Icon(sheet) => self.icon_size(font, sheet, r.scale[1]).0,
            })
            .sum();
        let mut pen = r.at.0
            - match r.h_align {
                HAlign::Left => 0.0,
                HAlign::Center => total * 0.5,
                HAlign::Right => total,
            };
        for p in parts {
            match p {
                Piece::Text(t) => {
                    let run = Run {
                        text: t,
                        at: (pen, r.at.1),
                        h_align: HAlign::Left,
                        ..*r
                    };
                    self.run(i, m, s, font, &run);
                    pen += run_width(font, t, r.scale[0], r.shadow);
                }
                Piece::Icon(sheet) => {
                    let (w, h) = self.icon_size(font, sheet, r.scale[1]);
                    self.quad(
                        i,
                        m,
                        s,
                        &Quad {
                            rect: [pen, r.at.1, pen + w, r.at.1 + h],
                            uv: [0.0, 0.0, 1.0, 1.0],
                            color: [1.0, 1.0, 1.0, r.color[3]],
                            texture: sheet,
                            additive: r.additive,
                            shadow: false,
                            boxed: true,
                        },
                    );
                    pen += w;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_tokens_follow_device_and_bindings_without_leaking_braces() {
        for set in crate::input_icons::ALL_SETS {
            let mut icons = Icons::of(set);
            icons.controls[1] = PadButton::West;
            icons.keys[1] = crate::input_icons::KeyLabel::new("[G]");
            let parts = pieces("{btnr1} {btnl1} {Map}", &icons);
            assert_eq!(parts.len(), 5);
            assert!(parts
                .iter()
                .all(|p| !matches!(p, Piece::Text(t) if t.contains('{'))));
            if set == IconSet::KeyboardMouse {
                assert_eq!(parts[2], Piece::Text("[G]"));
                assert_eq!(parts[4], Piece::Text("[Tab]"));
            } else {
                assert_eq!(parts[2], Piece::Icon(pad_sheet(set, PadButton::West)));
                assert_eq!(parts[4], Piece::Icon(pad_sheet(set, PadButton::Select)));
            }
        }
        assert_eq!(
            pieces("broken {token", &Icons::default()),
            vec![Piece::Text("broken "), Piece::Text("{token")]
        );
    }

    #[test]
    fn a_control_token_is_its_button_and_an_unknown_one_a_space() {
        let icons = Icons::default();
        assert_eq!(
            pieces("Press {GuiAccept} to continue", &icons),
            vec![
                Piece::Text("Press "),
                Piece::Icon(icons.sheet(Prompt::Accept)),
                Piece::Text(" to continue"),
            ]
        );
        assert_eq!(
            pieces("{Nope}x", &icons),
            vec![Piece::Text(" "), Piece::Text("x")]
        );
    }

    #[test]
    fn words_break_at_the_box_width() {
        let measure = |t: &str| t.len() as f32;
        assert_eq!(wrap("aa bb cc", 5.0, &measure), vec!["aa bb", "cc"]);
        assert_eq!(wrap("aa bb cc", 0.0, &measure), vec!["aa bb cc"]);
        assert_eq!(wrap("a\nb", 0.0, &measure), vec!["a", "b"]);
    }
}
