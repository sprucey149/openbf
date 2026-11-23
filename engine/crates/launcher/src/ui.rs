//! Native egui layout matching the minimalist `740×450` OpenBF launcher design.
//!
//! Pure UI state and rendering logic live here so the windowing/wgpu host in
//! `main.rs` stays thin and the visual layout can be tested independently.

use crate::config::{GraphicsApi, LauncherConfig, Platform, SourceConfig};
use crate::paths::{open_folder, LauncherPaths};
use crate::runner::{BuildHandle, BuildOutcome, BuildProgress};
use egui::{
    Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Id,
    Layout, Margin, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const WINDOW_WIDTH: f32 = 740.0;
pub const WINDOW_HEIGHT: f32 = 450.0;
const SIDEBAR_WIDTH: f32 = 146.0;
const TITLEBAR_HEIGHT: f32 = 32.0;

// Palette matching the launcher mockup.
const BG_MAIN: Color32 = Color32::WHITE;
const BG_SIDEBAR: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf3);
const BORDER_SIDEBAR: Color32 = Color32::from_rgb(0xde, 0xde, 0xdb);
const NAV_HOVER: Color32 = Color32::from_rgb(0xec, 0xec, 0xea);
const NAV_ACTIVE: Color32 = Color32::from_rgb(0xed, 0xed, 0xeb);
const NAV_BAR: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
const TEXT_ACTIVE: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
const TEXT_NAV: Color32 = Color32::from_rgb(0x4b, 0x4b, 0x4b);
const TEXT_MUTED: Color32 = Color32::from_rgb(0x77, 0x77, 0x77);
const INPUT_BORDER: Color32 = Color32::from_rgb(0xcc, 0xcc, 0xcc);
const MENU_BORDER: Color32 = Color32::from_rgb(0xbf, 0xbf, 0xbc);
const MENU_HOVER: Color32 = Color32::from_rgb(0xeb, 0xeb, 0xe8);
const BTN_BG: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf3);
const BTN_HOVER: Color32 = Color32::from_rgb(0xea, 0xea, 0xe7);
const BTN_BORDER: Color32 = Color32::from_rgb(0xc7, 0xc7, 0xc5);
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xc7, 0x42, 0x42);
const DOT_MISSING: Color32 = Color32::from_rgb(0xc5, 0x45, 0x45);
const DOT_READY: Color32 = Color32::from_rgb(0x32, 0x9b, 0x50);

fn regular_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Active view in the right-hand pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Setup,
    Options,
}

/// Actions requested by the UI for the window host to execute.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostActions {
    pub drag_window: bool,
    pub minimize: bool,
    pub close: bool,
    pub launch_and_close: bool,
}

/// Top-level launcher application state.
pub struct LauncherApp {
    pub paths: LauncherPaths,
    pub config: LauncherConfig,
    pub page: Page,
    pub game_ready: bool,
    bf1_present: bool,
    bf2_present: bool,
    has_pack_script: bool,
    build: Option<BuildHandle>,
    last_build: Option<BuildProgress>,
    toast: Option<(String, Instant)>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl LauncherApp {
    pub fn new(paths: LauncherPaths, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let config = LauncherConfig::load(&paths.config_path);
        let mut app = Self {
            paths,
            config,
            page: Page::Setup,
            game_ready: false,
            bf1_present: false,
            bf2_present: false,
            has_pack_script: false,
            build: None,
            last_build: None,
            toast: None,
            wake: Arc::new(wake),
        };
        app.refresh_fs_state();
        app
    }

    /// Re-check cached filesystem flags (called at startup, on source changes,
    /// on build completion, and when the window regains focus — never per frame).
    pub fn refresh_fs_state(&mut self) {
        self.game_ready = self.paths.is_game_ready();
        self.bf1_present = self.config.bf1.is_present();
        self.bf2_present = self.config.bf2.is_present();
        self.has_pack_script = self.paths.pack_script().is_some();
    }

    pub fn is_building(&self) -> bool {
        self.build.is_some()
    }

    pub fn cancel_build_if_running(&self) {
        if let Some(handle) = &self.build {
            handle.cancel();
        }
    }

    fn save_config(&mut self) {
        self.bf1_present = self.config.bf1.is_present();
        self.bf2_present = self.config.bf2.is_present();
        if let Err(e) = self.config.save(&self.paths.config_path) {
            self.show_toast(format!("Could not save settings: {e}"));
        }
    }

    fn show_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now() + Duration::from_secs(3)));
    }

    fn poll_build(&mut self) {
        let Some(handle) = &self.build else {
            return;
        };
        let snap = handle.snapshot();
        let finished = !snap.running;
        let succeeded = matches!(snap.outcome, Some(BuildOutcome::Succeeded));
        self.last_build = Some(snap);
        if finished {
            self.build = None;
            if succeeded {
                self.refresh_fs_state();
            }
        }
    }

    /// Whether a game's status indicator should show green.
    ///
    /// Green when the user selected a valid source on disk, OR when the game is
    /// already built and ready to play and no new path has replaced it.
    fn source_status_ready(&self, present: bool, source: &SourceConfig) -> bool {
        present || (self.game_ready && source.path.is_none())
    }

    /// Install system regular/bold fonts and apply our light theme visuals once.
    pub fn configure_style(ctx: &egui::Context) {
        install_system_fonts(ctx);

        let mut style = (*ctx.style()).clone();
        let mut v = egui::Visuals::light();
        v.panel_fill = BG_MAIN;
        v.window_fill = BG_MAIN;
        v.override_text_color = Some(TEXT_PRIMARY);
        v.widgets.noninteractive.bg_fill = BG_MAIN;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, INPUT_BORDER);
        v.widgets.noninteractive.corner_radius = CornerRadius::ZERO;
        v.widgets.inactive.bg_fill = BTN_BG;
        v.widgets.inactive.weak_bg_fill = BTN_BG;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, BTN_BORDER);
        v.widgets.inactive.corner_radius = CornerRadius::ZERO;
        v.widgets.hovered.bg_fill = BTN_HOVER;
        v.widgets.hovered.weak_bg_fill = BTN_HOVER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, BTN_BORDER);
        v.widgets.hovered.corner_radius = CornerRadius::ZERO;
        v.widgets.active.bg_fill = NAV_ACTIVE;
        v.widgets.active.weak_bg_fill = NAV_ACTIVE;
        v.widgets.active.bg_stroke = Stroke::new(1.0, NAV_BAR);
        v.widgets.active.corner_radius = CornerRadius::ZERO;
        v.selection.bg_fill = MENU_HOVER;
        v.selection.stroke = Stroke::new(1.0, NAV_BAR);
        v.popup_shadow = egui::epaint::Shadow {
            offset: [0, 3],
            blur: 6,
            spread: 0,
            color: Color32::from_black_alpha(20),
        };
        style.visuals = v;
        style.spacing.item_spacing = Vec2::new(8.0, 0.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, regular_font(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, regular_font(13.0));
        ctx.set_style(style);
    }

    /// Render one frame of the launcher UI.
    pub fn ui(&mut self, ctx: &egui::Context) -> HostActions {
        let mut actions = HostActions::default();
        self.poll_build();
        let building = self.build.is_some();

        if let Some((_, until)) = &self.toast {
            let now = Instant::now();
            if now >= *until {
                self.toast = None;
            } else {
                ctx.request_repaint_after(until.saturating_duration_since(now));
            }
        }

        // Left sidebar (146px).
        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(Frame::new().fill(BG_SIDEBAR).inner_margin(Margin {
                left: 0,
                right: 0,
                top: 22,
                bottom: 17,
            }))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let rect = ui.max_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(rect.right(), rect.top() - 22.0),
                        Pos2::new(rect.right(), rect.bottom() + 17.0),
                    ],
                    Stroke::new(1.0, BORDER_SIDEBAR),
                );

                // `openbf` bold wordmark (margin: 0 23px 25px, 16px bold).
                let (brand_rect, _) =
                    ui.allocate_exact_size(Vec2::new(SIDEBAR_WIDTH, 20.0), Sense::hover());
                ui.painter().text(
                    Pos2::new(brand_rect.left() + 23.0, brand_rect.center().y),
                    Align2::LEFT_CENTER,
                    "openbf",
                    bold_font(16.0),
                    TEXT_PRIMARY,
                );
                ui.add_space(25.0);

                let play_enabled = self.game_ready && !building;
                if sidebar_nav_button(ui, "Play", false, play_enabled) {
                    actions.launch_and_close = true;
                }
                if sidebar_nav_button(ui, "Setup", self.page == Page::Setup, true) {
                    self.page = Page::Setup;
                }
                if sidebar_nav_button(ui, "Options", self.page == Page::Options, true) {
                    self.page = Page::Options;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    if sidebar_nav_button(ui, "Exit", false, true) {
                        actions.close = true;
                    }
                });
            });

        // Main right-hand pane.
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG_MAIN).inner_margin(Margin {
                left: 32,
                right: 32,
                top: 44,
                bottom: 28,
            }))
            .show(ctx, |ui| {
                match self.page {
                    Page::Setup => self.draw_setup_page(ui, building),
                    Page::Options => self.draw_options_page(ui, building),
                }
                self.draw_status_dots(ui);
            });

        // Frameless drag strip + minimize/close controls painted on top.
        self.draw_window_chrome(ctx, &mut actions);

        if let Some((msg, _)) = &self.toast {
            draw_toast(ctx, msg);
        }

        actions
    }

    fn draw_setup_page(&mut self, ui: &mut egui::Ui, building: bool) {
        ui.label(
            egui::RichText::new("Setup")
                .font(bold_font(17.0))
                .color(TEXT_PRIMARY),
        );
        ui.add_space(30.0);

        let mut changed = false;
        changed |= source_row(
            ui,
            "Battlefront I",
            "platform_bf1",
            &mut self.config.bf1,
            !building,
        );
        ui.add_space(21.0);
        changed |= source_row(
            ui,
            "Battlefront II",
            "platform_bf2",
            &mut self.config.bf2,
            !building,
        );
        if changed {
            self.save_config();
        }

        ui.add_space(24.0);

        let sources_ok = self.bf1_present && self.bf2_present;
        let has_pack = self.has_pack_script;
        let has_build_panel = self.last_build.is_some();
        ui.horizontal(|ui| {
            if building {
                if flat_button(ui, "Cancel", Vec2::new(104.0, 32.0), true) {
                    self.cancel_build_if_running();
                }
            } else {
                let label = if self.game_ready {
                    "Rebuild game"
                } else {
                    "Build game"
                };
                if primary_button(ui, label, Vec2::new(112.0, 32.0), sources_ok && has_pack) {
                    let wake = Arc::clone(&self.wake);
                    match BuildHandle::start(&self.paths, &self.config, move || wake()) {
                        Ok(handle) => {
                            self.last_build = Some(handle.snapshot());
                            self.build = Some(handle);
                        }
                        Err(e) => self.show_toast(format!("{e}")),
                    }
                }
                if !has_pack {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("scripts/pack.py not found")
                            .font(regular_font(12.5))
                            .color(DOT_MISSING),
                    );
                } else if self.game_ready && !has_build_panel {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Ready to play")
                            .font(regular_font(12.5))
                            .color(DOT_READY),
                    );
                }
            }
        });

        if let Some(snap) = &self.last_build {
            ui.add_space(18.0);
            let frac = if snap.step_total > 0 {
                snap.step_index as f32 / snap.step_total as f32
            } else {
                0.0
            };
            let bar_width = (ui.available_width() - 150.0).max(200.0);
            let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 6.0), Sense::hover());
            ui.painter().rect_filled(
                bar_rect,
                CornerRadius::ZERO,
                Color32::from_rgb(0xe8, 0xe8, 0xe5),
            );
            let fill_color = match &snap.outcome {
                Some(BuildOutcome::Failed(_)) => DOT_MISSING,
                Some(BuildOutcome::Succeeded) => DOT_READY,
                _ => NAV_BAR,
            };
            let fill_rect = Rect::from_min_size(
                bar_rect.min,
                Vec2::new(bar_rect.width() * frac.clamp(0.0, 1.0), bar_rect.height()),
            );
            ui.painter()
                .rect_filled(fill_rect, CornerRadius::ZERO, fill_color);

            ui.add_space(8.0);
            let status_text = match &snap.outcome {
                Some(BuildOutcome::Succeeded) => "Setup complete — ready to play.".to_string(),
                Some(BuildOutcome::Cancelled) => "Setup cancelled.".to_string(),
                Some(BuildOutcome::Failed(err)) => format!("Failed: {err}"),
                None => {
                    if snap.step_index > 0 {
                        format!(
                            "Step {}/{}: {}",
                            snap.step_index, snap.step_total, snap.step_label
                        )
                    } else {
                        snap.step_label.clone()
                    }
                }
            };
            let status_color = match &snap.outcome {
                Some(BuildOutcome::Failed(_)) => DOT_MISSING,
                Some(BuildOutcome::Succeeded) => DOT_READY,
                _ => TEXT_PRIMARY,
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(status_text)
                        .font(regular_font(12.5))
                        .color(status_color),
                )
                .truncate(),
            );
            if snap.running && !snap.last_line.is_empty() {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&snap.last_line)
                            .font(regular_font(11.5))
                            .color(TEXT_MUTED),
                    )
                    .truncate(),
                );
            }
        }
    }

    fn draw_options_page(&mut self, ui: &mut egui::Ui, building: bool) {
        ui.label(
            egui::RichText::new("Options")
                .font(bold_font(17.0))
                .color(TEXT_PRIMARY),
        );
        ui.add_space(30.0);

        let mut changed = false;

        // Graphics API row.
        ui.horizontal(|ui| {
            row_label(ui, "Graphics API", 104.0);
            let supported = GraphicsApi::supported_on_host();
            let labels: Vec<&str> = supported.iter().map(|a| a.label()).collect();
            let cur_idx = supported
                .iter()
                .position(|a| *a == self.config.graphics_api)
                .unwrap_or(0);
            if let Some(new_idx) =
                dropdown_picker(ui, "graphics_api", 150.0, &labels, cur_idx, true)
            {
                self.config.graphics_api = supported[new_idx];
                changed = true;
            }
        });

        ui.add_space(21.0);

        // Optional DLC Heroes folder row.
        let dlc_selected = self.config.dlc_dir.is_some();
        ui.horizontal(|ui| {
            row_label(ui, "DLC Heroes", 104.0);
            let clear_w = if dlc_selected { 64.0 } else { 0.0 };
            let field_w = (ui.available_width() - 69.0 - clear_w - 8.0).max(120.0);
            {
                let dlc_text = self.config.dlc_display_name();
                readonly_path_box(ui, &dlc_text, dlc_selected, field_w);
            }
            if flat_button(ui, "Browse", Vec2::new(69.0, 32.0), !building) {
                if let Some(dir) = pick_folder("Select DLC_XBOX folder") {
                    self.config.dlc_dir = Some(dir);
                    changed = true;
                }
            }
            if dlc_selected && flat_button(ui, "Clear", Vec2::new(56.0, 32.0), !building) {
                self.config.dlc_dir = None;
                changed = true;
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(112.0);
            ui.label(
                egui::RichText::new("Optional DLC_XBOX folder (Kit Fisto & Asajj Ventress)")
                    .font(regular_font(12.0))
                    .color(TEXT_MUTED),
            );
        });

        ui.add_space(21.0);

        // Open logs folder row.
        ui.horizontal(|ui| {
            row_label(ui, "Logs", 104.0);
            if flat_button(ui, "Open logs folder", Vec2::new(136.0, 32.0), true) {
                if let Err(e) = open_folder(&self.paths.logs_dir) {
                    self.show_toast(format!("{e}"));
                }
            }
        });

        if changed {
            self.save_config();
        }
    }

    fn draw_status_dots(&self, ui: &mut egui::Ui) {
        let content_rect = ui.max_rect();
        let bf1_ready = self.source_status_ready(self.bf1_present, &self.config.bf1);
        let bf2_ready = self.source_status_ready(self.bf2_present, &self.config.bf2);

        // Anchor directly to bottom-right of the main content area
        // (matching `.status { right: 32px; bottom: 28px; gap: 12px }`).
        let right_x = content_rect.right();
        let row2_y = content_rect.bottom() - 8.0;
        let row1_y = row2_y - 28.0;

        paint_status_row(ui.painter(), right_x, row1_y, "Battlefront I", bf1_ready);
        paint_status_row(ui.painter(), right_x, row2_y, "Battlefront II", bf2_ready);
    }

    fn draw_window_chrome(&self, ctx: &egui::Context, actions: &mut HostActions) {
        let screen = ctx.viewport_rect();
        egui::Area::new(Id::new("window_chrome"))
            .fixed_pos(Pos2::new(SIDEBAR_WIDTH, 0.0))
            .order(Order::Foreground)
            .show(ctx, |ui| {
                let total_w = (screen.width() - SIDEBAR_WIDTH).max(88.0);
                let drag_w = (total_w - 88.0).max(0.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let (_, drag_resp) = ui.allocate_exact_size(
                        Vec2::new(drag_w, TITLEBAR_HEIGHT),
                        Sense::click_and_drag(),
                    );
                    if drag_resp.drag_started() {
                        actions.drag_window = true;
                    }
                    if chrome_button(ui, ChromeIcon::Minimize) {
                        actions.minimize = true;
                    }
                    if chrome_button(ui, ChromeIcon::Close) {
                        actions.close = true;
                    }
                });
            });
    }
}

fn paint_status_row(
    painter: &egui::Painter,
    right_x: f32,
    center_y: f32,
    label: &str,
    ready: bool,
) {
    let dot_radius = 4.5;
    let dot_center = Pos2::new(right_x - dot_radius, center_y);
    let dot_color = if ready { DOT_READY } else { DOT_MISSING };
    painter.circle_filled(dot_center, dot_radius, dot_color);

    // 11px gap between label and dot, text on the LEFT of the dot.
    let text_right = right_x - dot_radius * 2.0 - 11.0;
    painter.text(
        Pos2::new(text_right, center_y),
        Align2::RIGHT_CENTER,
        label,
        regular_font(13.0),
        TEXT_PRIMARY,
    );
}

fn row_label(ui: &mut egui::Ui, text: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        bold_font(13.0),
        TEXT_PRIMARY,
    );
}

fn sidebar_nav_button(ui: &mut egui::Ui, label: &str, active: bool, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(SIDEBAR_WIDTH, 41.0), Sense::click());
    let hovered = enabled && resp.hovered();
    let bg = if active {
        NAV_ACTIVE
    } else if hovered {
        NAV_HOVER
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    }
    if active {
        let bar = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
        ui.painter().rect_filled(bar, CornerRadius::ZERO, NAV_BAR);
    }
    let text_color = if !enabled {
        Color32::from_rgba_unmultiplied(0x4b, 0x4b, 0x4b, 97)
    } else if active {
        TEXT_ACTIVE
    } else if hovered {
        Color32::from_rgb(0x20, 0x20, 0x20)
    } else {
        TEXT_NAV
    };
    let font = if active {
        bold_font(13.0)
    } else {
        regular_font(13.0)
    };
    ui.painter().text(
        Pos2::new(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        text_color,
    );
    if enabled && hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

fn source_row(
    ui: &mut egui::Ui,
    label: &str,
    id_salt: &str,
    source: &mut SourceConfig,
    enabled: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        row_label(ui, label, 104.0);

        let labels: [&str; 3] = [
            Platform::Pc.label(),
            Platform::Ps2.label(),
            Platform::Xbox.label(),
        ];
        let cur_idx = Platform::ALL
            .iter()
            .position(|p| *p == source.platform)
            .unwrap_or(0);
        if let Some(new_idx) = dropdown_picker(ui, id_salt, 100.0, &labels, cur_idx, enabled) {
            let next = Platform::ALL[new_idx];
            if next != source.platform {
                source.platform = next;
                source.path = None;
                changed = true;
            }
        }

        let field_w = (ui.available_width() - 69.0 - 8.0).max(120.0);
        let text = source.display_name();
        readonly_path_box(ui, &text, source.path.is_some(), field_w);

        if flat_button(ui, "Browse", Vec2::new(69.0, 32.0), enabled) {
            let picked = if source.platform.picks_folder() {
                pick_folder(&format!("Select {label} (PC) folder"))
            } else {
                pick_disc_image(&format!(
                    "Select {label} ({}) disc image",
                    source.platform.label()
                ))
            };
            if let Some(path) = picked {
                source.path = Some(path);
                changed = true;
            }
        }
    });
    changed
}

/// Custom 32px-tall white dropdown matching the mockup's `.platform-picker`.
fn dropdown_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    width: f32,
    options: &[&str],
    selected: usize,
    enabled: bool,
) -> Option<usize> {
    let id = ui.make_persistent_id(id_salt);
    let open_id = id.with("open");
    let mut is_open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
    if !enabled {
        is_open = false;
    }

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
    let hovered = enabled && resp.hovered();
    let bg = if hovered {
        Color32::from_rgb(0xf8, 0xf8, 0xf7)
    } else {
        BG_MAIN
    };
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        bg,
        Stroke::new(1.0, INPUT_BORDER),
        StrokeKind::Inside,
    );
    let fg = if enabled { TEXT_PRIMARY } else { TEXT_MUTED };
    let current = options.get(selected).copied().unwrap_or("");
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        current,
        regular_font(13.0),
        fg,
    );

    // Crisp downward chevron matching the mockup's SVG.
    let cx = rect.right() - 12.0;
    let cy = rect.center().y;
    let chevron = Stroke::new(1.3, Color32::from_rgb(0x55, 0x55, 0x55));
    ui.painter().line_segment(
        [Pos2::new(cx - 3.5, cy - 1.5), Pos2::new(cx, cy + 2.0)],
        chevron,
    );
    ui.painter().line_segment(
        [Pos2::new(cx, cy + 2.0), Pos2::new(cx + 3.5, cy - 1.5)],
        chevron,
    );

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if enabled && resp.clicked() {
        is_open = !is_open;
    }

    let mut picked = None;
    if is_open {
        let menu_pos = Pos2::new(rect.left(), rect.bottom() - 1.0);
        let menu_w = width.max(110.0);
        let area_resp = egui::Area::new(id.with("menu"))
            .fixed_pos(menu_pos)
            .order(Order::Foreground)
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(BG_MAIN)
                    .stroke(Stroke::new(1.0, MENU_BORDER))
                    .inner_margin(Margin::symmetric(0, 2))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 3],
                        blur: 6,
                        spread: 0,
                        color: Color32::from_black_alpha(20),
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for (idx, opt) in options.iter().enumerate() {
                            let (opt_rect, opt_resp) =
                                ui.allocate_exact_size(Vec2::new(menu_w, 29.0), Sense::click());
                            if opt_resp.hovered() {
                                ui.painter()
                                    .rect_filled(opt_rect, CornerRadius::ZERO, MENU_HOVER);
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            let font = if idx == selected {
                                bold_font(13.0)
                            } else {
                                regular_font(13.0)
                            };
                            ui.painter().text(
                                Pos2::new(opt_rect.left() + 8.0, opt_rect.center().y),
                                Align2::LEFT_CENTER,
                                *opt,
                                font,
                                TEXT_PRIMARY,
                            );
                            if opt_resp.clicked() {
                                picked = Some(idx);
                            }
                        }
                    });
            });

        let clicked_outside = ui.ctx().input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rect.contains(p) && !area_resp.response.rect.contains(p))
        });
        let esc = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
        if picked.is_some() || clicked_outside || esc {
            is_open = false;
        }
    }

    ui.ctx().data_mut(|d| d.insert_temp(open_id, is_open));
    picked
}

fn readonly_path_box(ui: &mut egui::Ui, text: &str, selected: bool, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        BG_MAIN,
        Stroke::new(1.0, INPUT_BORDER),
        StrokeKind::Inside,
    );
    let color = if selected {
        Color32::from_rgb(0x22, 0x22, 0x22)
    } else {
        TEXT_MUTED
    };
    let clip = rect.shrink2(Vec2::new(8.0, 0.0));
    let painter = ui.painter().with_clip_rect(clip);
    painter.text(
        Pos2::new(clip.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        regular_font(13.0),
        color,
    );
}

fn flat_button(ui: &mut egui::Ui, label: &str, size: Vec2, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = enabled && resp.hovered();
    let bg = if hovered { BTN_HOVER } else { BTN_BG };
    let text_color = if enabled {
        Color32::from_rgb(0x22, 0x22, 0x22)
    } else {
        Color32::from_rgba_unmultiplied(0x22, 0x22, 0x22, 100)
    };
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        bg,
        Stroke::new(1.0, BTN_BORDER),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        regular_font(13.0),
        text_color,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

fn primary_button(ui: &mut egui::Ui, label: &str, size: Vec2, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = enabled && resp.hovered();
    let bg = if !enabled {
        Color32::from_rgb(0xdc, 0xdc, 0xd9)
    } else if hovered {
        Color32::from_rgb(0x1f, 0x1f, 0x1f)
    } else {
        NAV_BAR
    };
    let text_color = if enabled {
        Color32::WHITE
    } else {
        Color32::from_rgb(0x88, 0x88, 0x88)
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        regular_font(13.0),
        text_color,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

#[derive(Clone, Copy)]
enum ChromeIcon {
    Minimize,
    Close,
}

fn chrome_button(ui: &mut egui::Ui, icon: ChromeIcon) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(44.0, TITLEBAR_HEIGHT), Sense::click());
    let hovered = resp.hovered();
    let (bg, fg) = match (icon, hovered) {
        (ChromeIcon::Close, true) => (CLOSE_HOVER, Color32::WHITE),
        (ChromeIcon::Minimize, true) => (
            Color32::from_rgb(0xe9, 0xe9, 0xe7),
            Color32::from_rgb(0x34, 0x34, 0x34),
        ),
        (_, false) => (Color32::TRANSPARENT, Color32::from_rgb(0x34, 0x34, 0x34)),
    };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    }
    let c = rect.center();
    let stroke = Stroke::new(1.35, fg);
    match icon {
        ChromeIcon::Minimize => {
            ui.painter().line_segment(
                [Pos2::new(c.x - 4.0, c.y), Pos2::new(c.x + 4.0, c.y)],
                stroke,
            );
        }
        ChromeIcon::Close => {
            ui.painter().line_segment(
                [
                    Pos2::new(c.x - 4.0, c.y - 4.0),
                    Pos2::new(c.x + 4.0, c.y + 4.0),
                ],
                stroke,
            );
            ui.painter().line_segment(
                [
                    Pos2::new(c.x + 4.0, c.y - 4.0),
                    Pos2::new(c.x - 4.0, c.y + 4.0),
                ],
                stroke,
            );
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn draw_toast(ctx: &egui::Context, msg: &str) {
    let screen = ctx.viewport_rect();
    egui::Area::new(Id::new("launcher_toast"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -22.0))
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            Frame::new()
                .fill(NAV_BAR)
                .inner_margin(Margin::symmetric(13, 8))
                .show(ui, |ui| {
                    ui.set_max_width(screen.width() - 48.0);
                    ui.label(
                        egui::RichText::new(msg)
                            .font(regular_font(12.0))
                            .color(Color32::WHITE),
                    );
                });
        });
}

/// Load native OS regular and bold sans-serif fonts so headings and labels have
/// real bold weight instead of egui's light built-in fallback.
fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let regular = load_first_font(REGULAR_FONT_CANDIDATES);
    let bold = load_first_font(BOLD_FONT_CANDIDATES);

    if let Some(bytes) = regular {
        fonts.font_data.insert(
            "sys_regular".to_owned(),
            Arc::new(FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "sys_regular".to_owned());
    }

    let mut bold_chain = Vec::new();
    if let Some(bytes) = bold {
        fonts
            .font_data
            .insert("sys_bold".to_owned(), Arc::new(FontData::from_owned(bytes)));
        bold_chain.push("sys_bold".to_owned());
    }
    if let Some(prop) = fonts.families.get(&FontFamily::Proportional) {
        bold_chain.extend(prop.iter().cloned());
    }
    fonts
        .families
        .insert(FontFamily::Name("bold".into()), bold_chain);

    ctx.set_fonts(fonts);
}

#[cfg(target_os = "windows")]
const REGULAR_FONT_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
];
#[cfg(target_os = "windows")]
const BOLD_FONT_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\segoeuib.ttf",
    r"C:\Windows\Fonts\arialbd.ttf",
];

#[cfg(target_os = "macos")]
const REGULAR_FONT_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/SFNS.ttf",
];
#[cfg(target_os = "macos")]
const BOLD_FONT_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/SFNS.ttf",
];

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const REGULAR_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/noto/NotoSans-Regular.ttf",
];
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const BOLD_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/noto/NotoSans-Bold.ttf",
];

fn load_first_font(candidates: &[&str]) -> Option<Vec<u8>> {
    candidates.iter().find_map(|p| std::fs::read(p).ok())
}

fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

fn pick_disc_image(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Disc images", &["iso", "xiso"])
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_ui_renders_setup_and_options_without_panic() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_ui_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut app = LauncherApp::new(paths, || {});

        let ctx = egui::Context::default();
        LauncherApp::configure_style(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };

        for page in [Page::Setup, Page::Options] {
            app.page = page;
            let out = ctx.run(input.clone(), |ctx| {
                let actions = app.ui(ctx);
                assert!(!actions.close);
                assert!(!actions.launch_and_close);
                assert!(!actions.minimize);
                assert!(!actions.drag_window);
            });
            assert!(!out.shapes.is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_dots_reflect_built_game_when_no_override_selected() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_dots_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut app = LauncherApp::new(paths, || {});

        assert!(!app.source_status_ready(app.bf1_present, &app.config.bf1));
        assert!(!app.source_status_ready(app.bf2_present, &app.config.bf2));

        app.game_ready = true;
        assert!(app.source_status_ready(app.bf1_present, &app.config.bf1));
        assert!(app.source_status_ready(app.bf2_present, &app.config.bf2));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
