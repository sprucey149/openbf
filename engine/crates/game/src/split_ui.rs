//! Each split view's own UI, painted into its own cell (issue #194, `analysis/plan/SPLITSCREEN.md`
//! section 7), and the Game Menu over the whole window. The window's frame
//! (`main/windowed/split.rs`, `render_views`) and the headless split captures
//! (`tests/split_capture.rs`) both paint through [`SplitUi::paint`], one call per view from that
//! view's overlay, so what the captures show is what the window draws.
//!
//! The layout rule is [`ui::hud_draw::Frame::cell`] (`SPLITSCREEN.md` 18.1 item 5): each view's
//! UI is laid out for and painted over its whole cell, never for the window. The HUD's items go
//! to the cell's edges; what the camera projects (the reticle, lock-ons, markers) lands on the
//! cell's 4:3 band ([`crate::split_layout::ui_rect`], the part of the widened field the unwidened
//! camera covers), and the spawn and stats pages stand centred on that band.
//!
//! The one `HudView` keeps each player's HUD state apart (`HudView::select_player`) but has one
//! vertex buffer, so it paints at most once per view's submit: a view paints its own HUD, or,
//! with the Game Menu open, the last view paints the menu over every cell (the menu takes the
//! HUD's place, as in single player) and the other views paint nothing.

use crate::hud_feed::HudView;
use crate::split_layout::Cell;
use ui::hud::HudModel;

/// A LAN split round's Game Menu line (issue #194): the round runs on under the menu, so it
/// names whose menu it is ("Han's menu") and says the battle goes on, never "Paused".
/// `who` is the opener's name, or `None` for the keyboard with nobody playing on it.
pub fn lan_menu_line(who: Option<&str>) -> String {
    match who {
        Some(name) => format!("{name}'s menu (battle runs on)"),
        None => "Keyboard menu (battle runs on)".to_string(),
    }
}

/// The line a player's cell shows once they have left their own round-end stats pages while
/// others still read theirs (issue #194): who the round waits for, by name.
pub fn waiting_line(names: &[String]) -> String {
    format!("Waiting for {}", names.join(", "))
}

/// `style`'s egui font (the menu face, `ui::typography`) in player cell `cell` (window pixels):
/// the role's size for this window times the cell's text scale
/// ([`crate::name_tags::cell_text_scale`]), so a cell's lines shrink with the cell as its round
/// timer does (issue #194 B22: the menu's own face, never egui's default).
pub fn cell_font(
    ctx: &ui::egui::Context,
    cell: Cell,
    style: &ui::typography::TextStyle,
) -> ui::egui::FontId {
    let ppp = ctx.pixels_per_point().max(0.1);
    let window_h = ctx.content_rect().height() * ppp;
    let mut font = ui::typography::egui_font(ctx, style);
    font.size = (font.size * crate::name_tags::cell_text_scale(cell, window_h, 1.0)).max(1.0);
    font
}

/// Paint [`waiting_line`]'s `text` on a dark band across the foot of player cell `cell` (window
/// pixels) in the egui overlay, over that cell's stats footer: the menu face's list role, in its
/// note tint.
pub fn draw_waiting_line(ctx: &ui::egui::Context, cell: Cell, text: &str) {
    use ui::egui::{Align2, Color32, LayerId, Order, Pos2, Rect};
    use ui::typography::{egui_color, tint, LIST};
    let ppp = ctx.pixels_per_point().max(0.1);
    let [x, y, w, h] = cell.map(|v| v as f32 / ppp);
    let font = cell_font(ctx, cell, &LIST);
    let band_h = font.size * 2.4;
    let band = Rect::from_min_size(Pos2::new(x, y + h - band_h), ui::egui::vec2(w, band_h));
    let painter = ctx.layer_painter(LayerId::new(
        Order::Foreground,
        ui::egui::Id::new("split_waiting"),
    ));
    painter.rect_filled(band, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));
    painter.text(
        band.center(),
        Align2::CENTER_CENTER,
        LIST.case.apply(text),
        font,
        egui_color(tint::NOTE),
    );
}

/// What a player's cell shows while their controller is gone: the notice, who it is (`name`,
/// the window's `App.names`) and how to come back (the pad rejoins on a deliberate press,
/// `local_seats::Seats::offer`). [`SplitUi::lost`] paints it in the cell's HUD pass
/// (`ui::hud_draw::draw_lost_cell`), in the menu face with the button glyphs.
pub fn lost_notice(name: String) -> ui::shell_ps2::players::LostCell {
    ui::shell_ps2::players::LostCell {
        notice: "Controller disconnected".to_string(),
        name,
        action: "Reconnect and press any button".to_string(),
        glyph: None,
    }
}

/// The acting line of a waiting player's lost-controller notice ([`lost_notice`]) while a
/// returning pad could be theirs or another waiting player's (identical models, issue #194):
/// `here` (the pick shows this player) puts the pad's A (in its icon set `set`) before "Play as"
/// their name, else its d-pad left/right, which moves the pick to them. The glyph's sheet, then
/// the label.
pub fn pick_line(set: ui::input_icons::IconSet, here: bool, name: &str) -> (&'static str, String) {
    use ui::input_icons::{Icons, Prompt};
    let prompt = if here { Prompt::Accept } else { Prompt::DPadLr };
    (Icons::of(set).sheet(prompt), format!("Play as {name}"))
}

/// The Game Menu of a split battle: one, opened and driven by `owner` (a local player index).
pub struct SplitPause<'a> {
    pub owner: usize,
    pub menu: &'a ui::menu::MenuModel,
    pub anim: &'a ui::pause_draw::Animation,
    /// A LAN round's Game Menu (issue #194 B20): the round runs on under it, so it covers only
    /// this view's cell (its opener's) and every other view keeps its HUD. `None`: the whole
    /// window, after the last view.
    pub in_view: Option<usize>,
}

/// Everything the views' UI draws from this frame.
pub struct SplitUi<'a> {
    /// Each local player's HUD model; `None` for a player who is away (their cell shows the
    /// lost-controller notice instead).
    pub models: &'a [Option<HudModel>],
    /// Each local player's prompt icons: their own pad's buttons, or the keyboard's for the
    /// keyboard-and-mouse player.
    pub icons: &'a [ui::input_icons::Icons],
    /// The fades' step (0 while paused).
    pub dt: f32,
    /// The `hud` cheat hides the battle HUD (the round's end screens still draw).
    pub hidden: bool,
    /// The native Game Menu, when it is open.
    pub pause: Option<SplitPause<'a>>,
    /// The whole window, which the Game Menu covers.
    pub window: [u32; 2],
    /// How many views this present has: the last one carries the Game Menu.
    pub views: usize,
    /// The HUD size setting's scale (1.0 Full; the host profile's
    /// `splitscreen.hud_size`, [`crate::profile::HudSize::k`]).
    pub hud_k: f32,
    /// Each view's lost-controller notice ([`lost_notice`]), `None` for a player who is here;
    /// a view with no HUD model and a notice paints the notice over its whole cell.
    pub lost: &'a [Option<ui::shell_ps2::players::LostCell>],
}

/// The screen view `cell`'s UI is laid out for: the whole cell, its top-left at 0, 0
/// ([`ui::hud_draw::Frame::cell`]). The window's pointer hit tests use the same frame.
pub fn cell_frame(cell: Cell, hud_k: f32) -> ui::hud_draw::Frame {
    ui::hud_draw::Frame::cell([cell[2] as f32, cell[3] as f32], hud_k)
}

impl SplitUi<'_> {
    /// Lay out and paint view `i`'s UI into `cell` of the window's image `view` (from that view's
    /// overlay, after its world). Returns the rectangle painted (the whole cell, or the window
    /// for the Game Menu; a LAN round's menu its opener's cell), if any.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        hv: &mut HudView,
        i: usize,
        cell: Cell,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) -> Option<Cell> {
        if let Some(p) = self
            .pause
            .as_ref()
            .filter(|p| p.in_view.is_none_or(|v| v == i))
        {
            if p.in_view.is_none() && i + 1 != self.views {
                return None;
            }
            let model = self
                .models
                .get(p.owner)
                .and_then(Option::as_ref)
                .or_else(|| self.models.iter().flatten().next())?;
            hv.select_player(p.owner);
            if let Some(icons) = self.icons.get(p.owner) {
                hv.set_icons(*icons);
            }
            let rect = match p.in_view {
                Some(_) => cell,
                None => [0, 0, self.window[0], self.window[1]],
            };
            hv.update_pause(model, p.menu, p.anim, [rect[2], rect[3]]);
            hv.paint_in(device, queue, encoder, view, rect);
            return Some(rect);
        }
        let Some(model) = self.models.get(i).and_then(Option::as_ref) else {
            let notice = self.lost.get(i)?.as_ref()?;
            hv.select_player(i);
            hv.update_lost(notice, [cell[2], cell[3]]);
            hv.paint_in(device, queue, encoder, view, cell);
            return Some(cell);
        };
        if !crate::round_end::hud_pass_drawn(model.round_end.is_some(), self.hidden) {
            return None;
        }
        hv.select_player(i);
        if let Some(icons) = self.icons.get(i) {
            hv.set_icons(*icons);
        }
        hv.update_in(model, self.dt, &cell_frame(cell, self.hud_k));
        hv.paint_in(device, queue, encoder, view, cell);
        Some(cell)
    }
}

/// The device whose icons local player `i`'s prompts show: the keyboard's for the keyboard
/// player, else their own pad's family, else (no pad yet) `fallback`.
pub fn seat_device(
    keyboard: bool,
    pad: Option<input::PadFamily>,
    fallback: crate::input_icons::Device,
) -> crate::input_icons::Device {
    match (keyboard, pad) {
        (true, _) => crate::input_icons::Device::KeyboardMouse,
        (false, Some(f)) => crate::input_icons::Device::Pad(f),
        (false, None) => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_icons::Device;

    /// Issue #194 (finding pick-line-spells-button-as-text): the waiting cells of a pad that could
    /// be either player's show that pad's own A glyph before "Play as" the player it takes, or its
    /// d-pad left/right glyph that moves the pick, never the button spelled in words.
    #[test]
    fn the_pick_line_names_the_pads_button_and_the_player() {
        use ui::input_icons::IconSet;
        use ui::input_icons::{pad_sheet, Icons, PadButton, Prompt};
        assert_eq!(
            pick_line(IconSet::Ds4, true, "Han"),
            (
                pad_sheet(IconSet::Ds4, PadButton::South),
                "Play as Han".into()
            )
        );
        assert_eq!(
            pick_line(IconSet::XboxOne, true, "Leia"),
            (
                pad_sheet(IconSet::XboxOne, PadButton::South),
                "Play as Leia".into()
            )
        );
        assert_eq!(
            pick_line(IconSet::XboxOne, false, "Leia"),
            (
                Icons::of(IconSet::XboxOne).sheet(Prompt::DPadLr),
                "Play as Leia".into()
            )
        );
        // The notice says how to come back without a glyph (any button does).
        let n = lost_notice("Han".into());
        assert_eq!((n.name.as_str(), n.glyph), ("Han", None));
    }

    #[test]
    fn only_the_keyboard_player_gets_keyboard_prompts() {
        let pad = Device::Pad(input::PadFamily::XboxOne);
        assert_eq!(
            seat_device(true, Some(input::PadFamily::Ds4), pad),
            Device::KeyboardMouse
        );
        assert_eq!(
            seat_device(false, Some(input::PadFamily::Ds4), Device::KeyboardMouse),
            Device::Pad(input::PadFamily::Ds4)
        );
        assert_eq!(seat_device(false, None, pad), pad);
    }
}
