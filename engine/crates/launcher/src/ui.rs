//! Native egui layout matching the minimalist `740×450` OpenBF launcher design.
//!
//! Pure UI state and rendering logic live here so the windowing/wgpu host in
//! `main.rs` stays thin and the visual layout can be tested independently.

use crate::config::{GraphicsApi, LauncherConfig, Platform, SourceConfig};
use crate::mods_worker::{ModsSnapshot, ModsWorker};
use crate::paths::{open_folder, LauncherPaths};
use crate::runner::{is_error_line, BuildHandle, BuildOutcome, BuildProgress};
use crate::ui_mods::card_text;
use egui::{
    Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Id,
    Layout, Margin, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use extensions::profile::{ExtensionProfile, StartupErrorReport, StartupOverrides};
use extensions::registry::ExtensionRegistry;
use extensions::ExtensionManifest;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const WINDOW_WIDTH: f32 = 740.0;
pub const WINDOW_HEIGHT: f32 = 450.0;
const SIDEBAR_WIDTH: f32 = 146.0;
const TITLEBAR_HEIGHT: f32 = 32.0;

// Palette matching the launcher mockup.
pub(crate) const BG_MAIN: Color32 = Color32::WHITE;
pub(crate) const BG_SIDEBAR: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf3);
pub(crate) const BORDER_SIDEBAR: Color32 = Color32::from_rgb(0xde, 0xde, 0xdb);
const NAV_HOVER: Color32 = Color32::from_rgb(0xec, 0xec, 0xea);
const NAV_ACTIVE: Color32 = Color32::from_rgb(0xed, 0xed, 0xeb);
const NAV_BAR: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
pub(crate) const TEXT_PRIMARY: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
const TEXT_ACTIVE: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
pub(crate) const TEXT_NAV: Color32 = Color32::from_rgb(0x4b, 0x4b, 0x4b);
pub(crate) const TEXT_MUTED: Color32 = Color32::from_rgb(0x77, 0x77, 0x77);
pub(crate) const INPUT_BORDER: Color32 = Color32::from_rgb(0xcc, 0xcc, 0xcc);
const MENU_BORDER: Color32 = Color32::from_rgb(0xbf, 0xbf, 0xbc);
const MENU_HOVER: Color32 = Color32::from_rgb(0xeb, 0xeb, 0xe8);
pub(crate) const BTN_BG: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf3);
pub(crate) const BTN_HOVER: Color32 = Color32::from_rgb(0xea, 0xea, 0xe7);
pub(crate) const BTN_BORDER: Color32 = Color32::from_rgb(0xc7, 0xc7, 0xc5);
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xc7, 0x42, 0x42);
pub(crate) const DOT_MISSING: Color32 = Color32::from_rgb(0xc5, 0x45, 0x45);
pub(crate) const DOT_BUILDING: Color32 = Color32::from_rgb(0xd4, 0x9b, 0x1a);
pub(crate) const DOT_READY: Color32 = Color32::from_rgb(0x32, 0x9b, 0x50);

/// Status indicator state for Battlefront I / Battlefront II.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DotState {
    Missing,
    Building,
    Ready,
}

/// Drop shadow under the dropdown menus.
const POPUP_SHADOW: egui::epaint::Shadow = egui::epaint::Shadow {
    offset: [0, 3],
    blur: 6,
    spread: 0,
    color: Color32::from_black_alpha(20),
};

pub(crate) fn regular_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub(crate) fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Active view in the right-hand pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Setup,
    Mods,
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
    pub page: Option<Page>,
    pub game_ready: bool,
    pub built_has_bf1: bool,
    pub extension_catalog: Vec<ExtensionManifest>,
    pub extension_profile: ExtensionProfile,
    pub mods_worker: ModsWorker,
    pub mods_snapshot: ModsSnapshot,
    pub(crate) last_synced_mod_gen: u64,
    pub startup_error: Option<StartupErrorReport>,
    pub launch_force_desktop: bool,
    bf1_present: bool,
    bf2_present: bool,
    bf1_cleared: bool,
    bf2_cleared: bool,
    building_with_bf1: bool,
    can_build: bool,
    build: Option<BuildHandle>,
    last_build: Option<BuildProgress>,
    toast: Option<(String, Instant)>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl LauncherApp {
    pub fn new(paths: LauncherPaths, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let config = LauncherConfig::load(&paths.config_path);
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let mods_worker = ModsWorker::new(Arc::clone(&wake));
        let mut app = Self {
            paths,
            config,
            page: None,
            game_ready: false,
            built_has_bf1: false,
            extension_catalog: Vec::new(),
            extension_profile: ExtensionProfile::default(),
            mods_worker,
            mods_snapshot: ModsSnapshot::default(),
            last_synced_mod_gen: 0,
            startup_error: None,
            launch_force_desktop: false,
            bf1_present: false,
            bf2_present: false,
            bf1_cleared: false,
            bf2_cleared: false,
            building_with_bf1: false,
            can_build: false,
            build: None,
            last_build: None,
            toast: None,
            wake,
        };
        app.refresh_fs_state();
        app
    }

    /// Re-check cached filesystem flags (called at startup, on source changes,
    /// on build completion, and when the window regains focus — never per frame).
    pub fn refresh_fs_state(&mut self) {
        let built = self.paths.built_status();
        self.game_ready = built.ready;
        self.built_has_bf1 = built.has_bf1;
        self.bf1_present = self.config.bf1.is_present();
        self.bf2_present = self.config.bf2.is_present();
        self.can_build = self.paths.can_build();
        self.extension_catalog = self.paths.probe_bundled_extensions();
        self.extension_profile = ExtensionProfile::load(&self.paths.extensions_profile_path());
        self.startup_error = StartupErrorReport::load(&self.paths.startup_error_path());
        self.save_stock_sources();
        self.trigger_mod_scan();
    }

    fn save_stock_sources(&self) {
        if let Err(e) = self.config.stock_sources().save(&self.paths.data_dir) {
            eprintln!("launcher: writing stock_sources.ron: {e}");
        }
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
        self.save_stock_sources();
    }

    pub(crate) fn save_extension_profile(&mut self) {
        if let Err(e) = self
            .extension_profile
            .save(&self.paths.extensions_profile_path())
        {
            self.show_toast(format!("Could not save extension settings: {e}"));
        }
    }

    fn clear_startup_error(&mut self) {
        StartupErrorReport::clear(&self.paths.startup_error_path());
        self.startup_error = None;
    }

    pub fn set_extension_enabled(&mut self, id: &str, enabled: bool) {
        self.extension_profile.set_enabled(id, enabled);
        self.save_extension_profile();
    }

    pub fn resolve_extensions(
        &self,
    ) -> Result<extensions::registry::ActivationPlan, Vec<extensions::registry::ResolutionError>>
    {
        ExtensionRegistry::new(self.extension_catalog.iter().cloned())
            .resolve(&self.extension_profile, &StartupOverrides::default())
    }

    pub(crate) fn show_toast(&mut self, msg: impl Into<String>) {
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
            self.building_with_bf1 = false;
            if succeeded {
                self.bf1_cleared = false;
                self.bf2_cleared = false;
            }
            self.refresh_fs_state();
        }
    }

    /// Status dot state for Battlefront I:
    /// - Yellow while a build including BF1 is running.
    /// - Red while a BF2-only build is running, when BF1 was cleared, or when not set up.
    /// - Green when the built game includes BF1.
    pub fn bf1_dot_state(&self) -> DotState {
        if self.build.is_some() {
            if self.building_with_bf1 {
                DotState::Building
            } else {
                DotState::Missing
            }
        } else if self.built_has_bf1 && !self.bf1_cleared {
            DotState::Ready
        } else {
            DotState::Missing
        }
    }

    /// Status dot state for Battlefront II:
    /// - Yellow while building.
    /// - Green when the built game is set up and ready.
    /// - Red otherwise.
    pub fn bf2_dot_state(&self) -> DotState {
        if self.build.is_some() {
            DotState::Building
        } else if self.game_ready && !self.bf2_cleared {
            DotState::Ready
        } else {
            DotState::Missing
        }
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
        v.popup_shadow = POPUP_SHADOW;
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
        self.poll_mods(Some(ctx));
        let building = self.build.is_some();
        let mod_block = self.play_blocking_reason();

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

                let play_enabled = self.game_ready && !building && mod_block.is_none();
                if sidebar_nav_button(ui, "Play", false, play_enabled) {
                    self.launch_force_desktop = false;
                    actions.launch_and_close = true;
                }
                if sidebar_nav_button(ui, "Setup", self.page == Some(Page::Setup), true) {
                    self.page = Some(Page::Setup);
                }
                if sidebar_nav_button(ui, "Mods", self.page == Some(Page::Mods), true) {
                    self.page = Some(Page::Mods);
                }
                if sidebar_nav_button(ui, "Options", self.page == Some(Page::Options), true) {
                    self.page = Some(Page::Options);
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
                if self.startup_error.is_some() {
                    self.draw_startup_error_banner(ui, building, &mut actions);
                    ui.add_space(14.0);
                }
                if let Some(reason) = &mod_block {
                    draw_notice_box(
                        ui,
                        Color32::from_rgb(0xff, 0xfb, 0xeb),
                        DOT_BUILDING,
                        TEXT_PRIMARY,
                        &format!("Play paused: {reason}"),
                    );
                    ui.add_space(10.0);
                }
                match self.page {
                    Some(Page::Setup) => self.draw_setup_page(ui, building),
                    Some(Page::Mods) => self.draw_mods_page(ui),
                    Some(Page::Options) => self.draw_options_page(ui, building),
                    None => {}
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
        let bf1_changed = source_row(
            ui,
            "Battlefront I",
            "platform_bf1",
            &mut self.config.bf1,
            !building,
        );
        if bf1_changed {
            self.bf1_cleared = !self.config.bf1.has_any();
            changed = true;
        }
        ui.add_space(21.0);
        let bf2_changed = source_row(
            ui,
            "Battlefront II",
            "platform_bf2",
            &mut self.config.bf2,
            !building,
        );
        if bf2_changed {
            self.bf2_cleared = !self.config.bf2.has_any();
            changed = true;
        }
        if changed {
            self.save_config();
        }

        ui.add_space(24.0);

        let bf1_ok = !self.config.bf1.has_any() || self.bf1_present;
        let sources_ok = self.bf2_present && bf1_ok;
        let can_build = self.can_build;
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
                if primary_button(ui, label, Vec2::new(112.0, 32.0), sources_ok && can_build) {
                    let wake = Arc::clone(&self.wake);
                    let with_bf1 = self.config.bf1.has_any();
                    let handle = BuildHandle::start(&self.paths, &self.config, move || wake());
                    self.building_with_bf1 = with_bf1;
                    self.last_build = Some(handle.snapshot());
                    self.build = Some(handle);
                }
                if !can_build {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("bake or game binary not found")
                            .font(regular_font(12.5))
                            .color(DOT_MISSING),
                    );
                } else if self.bf1_present && !self.bf2_present {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Battlefront II is required to build")
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
            let fill_color = outcome_color(snap.outcome.as_ref(), NAV_BAR);
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
            let status_color = outcome_color(snap.outcome.as_ref(), TEXT_PRIMARY);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(status_text)
                        .font(regular_font(12.5))
                        .color(status_color),
                )
                .truncate(),
            );
            if !snap.lines.is_empty() {
                ui.add_space(6.0);
                let console_h = ui.available_height().max(48.0);
                draw_fading_console(ui, bar_width, console_h, &snap.lines);
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
                dropdown_picker(ui, "graphics_api", 150.0, &labels, cur_idx, None, true)
            {
                self.config.graphics_api = supported[new_idx];
                changed = true;
            }
        });

        ui.add_space(21.0);

        // Optional DLC Heroes folder row.
        ui.horizontal(|ui| {
            row_label(ui, "DLC Heroes", 104.0);
            let action = path_field(
                ui,
                &self.config.dlc_display_name(),
                self.config.dlc_dir.is_some(),
                !building,
            );
            match action {
                Some(PathAction::Browse) => {
                    if let Some(dir) = pick_folder("Select DLC_XBOX folder") {
                        self.config.dlc_dir = Some(dir);
                        changed = true;
                    }
                }
                Some(PathAction::Clear) => {
                    self.config.dlc_dir = None;
                    changed = true;
                }
                None => {}
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

        // Instant Action map folders row.
        ui.horizontal(|ui| {
            row_label(ui, "Map folders", 104.0);
            let enabled = self.config.group_maps_by_source;
            if extension_toggle(ui, "group_maps_by_source", enabled, true) {
                self.config.group_maps_by_source = !enabled;
                changed = true;
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(112.0);
            ui.label(
                egui::RichText::new(
                    "Group Instant Action maps by Battlefront 1, Battlefront 2 and each mod",
                )
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

    fn draw_startup_error_banner(
        &mut self,
        ui: &mut egui::Ui,
        building: bool,
        actions: &mut HostActions,
    ) {
        let Some(report) = self.startup_error.clone() else {
            return;
        };
        let mut clear_error = false;
        let width = ui.available_width();
        Frame::new()
            .fill(Color32::from_rgb(0xfd, 0xf3, 0xf2))
            .stroke(Stroke::new(1.0, DOT_MISSING))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width((width - 24.0).max(160.0));
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);
                ui.label(
                    egui::RichText::new(format!("{} failed to start", report.extension_name))
                        .font(bold_font(13.5))
                        .color(DOT_MISSING),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&report.summary)
                            .font(regular_font(12.5))
                            .color(TEXT_PRIMARY),
                    )
                    .wrap(),
                );
                if !report.details.trim().is_empty() {
                    let details = truncate_banner_text(&report.details, 180);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{details} (see openbf_data/logs/startup_error.log)"
                            ))
                            .font(regular_font(11.5))
                            .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if report.can_retry_desktop
                        && primary_button(
                            ui,
                            "Play in Desktop Mode",
                            Vec2::new(156.0, 28.0),
                            self.game_ready && !building,
                        )
                    {
                        clear_error = true;
                        self.launch_force_desktop = true;
                        actions.launch_and_close = true;
                    }
                    if self.extension_profile.is_enabled(&report.extension_id)
                        && flat_button(
                            ui,
                            &format!("Disable {}", report.extension_name),
                            Vec2::new(144.0, 28.0),
                            true,
                        )
                    {
                        self.set_extension_enabled(&report.extension_id, false);
                        clear_error = true;
                        self.show_toast(format!("Disabled {}", report.extension_name));
                    }
                    if flat_button(ui, "Dismiss", Vec2::new(76.0, 28.0), true) {
                        clear_error = true;
                    }
                });
            });
        if clear_error {
            self.clear_startup_error();
        }
    }

    fn draw_status_dots(&self, ui: &mut egui::Ui) {
        let content_rect = ui.max_rect();
        let bf1_state = self.bf1_dot_state();
        let bf2_state = self.bf2_dot_state();

        // Anchor directly to bottom-right of the main content area
        // (matching `.status { right: 32px; bottom: 28px; gap: 12px }`).
        let right_x = content_rect.right();
        let row2_y = content_rect.bottom() - 8.0;
        let row1_y = row2_y - 28.0;

        paint_status_row(ui.painter(), right_x, row1_y, "Battlefront I", bf1_state);
        paint_status_row(ui.painter(), right_x, row2_y, "Battlefront II", bf2_state);
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
    state: DotState,
) {
    let dot_radius = 4.5;
    let dot_center = Pos2::new(right_x - dot_radius, center_y);
    let dot_color = match state {
        DotState::Missing => DOT_MISSING,
        DotState::Building => DOT_BUILDING,
        DotState::Ready => DOT_READY,
    };
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

        let labels = Platform::ALL.map(Platform::label);
        let cur_idx = Platform::ALL
            .iter()
            .position(|p| *p == source.platform)
            .unwrap_or(0);
        let title: Vec<(&str, bool)> = source
            .title_parts()
            .into_iter()
            .map(|(p, shown)| (p.label(), shown))
            .collect();
        if let Some(new_idx) =
            dropdown_picker(ui, id_salt, 150.0, &labels, cur_idx, Some(&title), enabled)
        {
            let next = Platform::ALL[new_idx];
            if next != source.platform {
                source.platform = next;
                changed = true;
            }
        }

        let shown = source.platform;
        let action = path_field(
            ui,
            &source.display_name(),
            source.active_path().is_some(),
            enabled,
        );
        match action {
            Some(PathAction::Browse) => {
                let picked = if shown.picks_folder() {
                    pick_folder(&format!("Select {label} (PC) folder"))
                } else {
                    pick_disc_image(&format!("Select {label} ({}) disc image", shown.label()))
                };
                if let Some(path) = picked {
                    source.set_path(shown, path);
                    changed = true;
                }
            }
            Some(PathAction::Clear) => {
                source.clear(shown);
                changed = true;
            }
            None => {}
        }
    });
    changed
}

/// A button pressed beside a [`path_field`].
enum PathAction {
    Browse,
    Clear,
}

/// The read-only path box with its Browse button and, once a path is
/// `selected`, a Clear button.
fn path_field(ui: &mut egui::Ui, text: &str, selected: bool, enabled: bool) -> Option<PathAction> {
    let clear_w = if selected { 64.0 } else { 0.0 };
    let field_w = (ui.available_width() - 69.0 - clear_w - 8.0).max(120.0);
    readonly_path_box(ui, text, selected, field_w);
    if flat_button(ui, "Browse", Vec2::new(69.0, 32.0), enabled) {
        Some(PathAction::Browse)
    } else if selected && flat_button(ui, "Clear", Vec2::new(56.0, 32.0), enabled) {
        Some(PathAction::Clear)
    } else {
        None
    }
}

/// Red for a failed build, green for a finished one, `running` otherwise.
fn outcome_color(outcome: Option<&BuildOutcome>, running: Color32) -> Color32 {
    match outcome {
        Some(BuildOutcome::Failed(_)) => DOT_MISSING,
        Some(BuildOutcome::Succeeded) => DOT_READY,
        _ => running,
    }
}

/// `BG_MAIN` at `alpha`.
fn bg_alpha(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(BG_MAIN.r(), BG_MAIN.g(), BG_MAIN.b(), alpha)
}

/// Fill `rect` with a gradient, one colour per corner (top-left, top-right,
/// bottom-right, bottom-left).
fn paint_gradient(ui: &egui::Ui, rect: Rect, [tl, tr, br, bl]: [Color32; 4]) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), tl);
    mesh.colored_vertex(rect.right_top(), tr);
    mesh.colored_vertex(rect.right_bottom(), br);
    mesh.colored_vertex(rect.left_bottom(), bl);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// Custom 32px-tall white dropdown matching the mockup's `.platform-picker`.
/// `title` replaces the closed box's text with parts joined by " + ", the
/// flagged ones bold, cut with "…" when they do not fit; `None` shows the
/// selected option.
fn dropdown_picker(
    ui: &mut egui::Ui,
    id_salt: &str,
    width: f32,
    options: &[&str],
    selected: usize,
    title: Option<&[(&str, bool)]>,
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
    let current = [(options.get(selected).copied().unwrap_or(""), false)];
    let galley = ui
        .painter()
        .layout_job(picker_title(title.unwrap_or(&current), width - 32.0, fg));
    ui.painter().galley(
        Pos2::new(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
        galley,
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
                    .shadow(POPUP_SHADOW)
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

/// One line of `parts` joined by " + ", the flagged ones bold, cut with "…"
/// at `max_width`.
fn picker_title(parts: &[(&str, bool)], max_width: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (idx, (text, bold)) in parts.iter().enumerate() {
        if idx > 0 {
            job.append(
                " + ",
                0.0,
                egui::TextFormat::simple(regular_font(13.0), color),
            );
        }
        let font = if *bold {
            bold_font(13.0)
        } else {
            regular_font(13.0)
        };
        job.append(text, 0.0, egui::TextFormat::simple(font, color));
    }
    job.wrap = egui::text::TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    job
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

pub(crate) fn flat_button(ui: &mut egui::Ui, label: &str, size: Vec2, enabled: bool) -> bool {
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

fn draw_fading_console(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    lines: &std::collections::VecDeque<String>,
) {
    // Borderless, backgroundless scroll area that merges directly into BG_MAIN.
    let scroll_out = ui.scope(|ui| {
        ui.set_max_width(width);
        let style = ui.style_mut();
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 4.0,
            bar_inner_margin: 2.0,
            bar_outer_margin: 0.0,
            floating: true,
            floating_width: 4.0,
            floating_allocated_width: 0.0,
            foreground_color: false,
            dormant_background_opacity: 0.0,
            active_background_opacity: 0.0,
            interact_background_opacity: 0.0,
            dormant_handle_opacity: 0.18,
            active_handle_opacity: 0.45,
            interact_handle_opacity: 0.65,
            ..style.spacing.scroll
        };
        egui::ScrollArea::vertical()
            .id_salt("setup_console")
            .max_width(width)
            .max_height(height)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 2.0;
                // Add top breathing room so the newest lines still read clearly
                // once content scrolls into the top fade zone.
                ui.add_space(10.0);
                for line in lines {
                    let color = if line.starts_with("== [") {
                        TEXT_PRIMARY
                    } else if is_error_line(line) {
                        DOT_MISSING
                    } else {
                        TEXT_MUTED
                    };
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(line)
                                .font(regular_font(11.5))
                                .color(color),
                        )
                        .wrap(),
                    );
                }
            })
    });

    // Smooth vertical white fade over the top edge once lines scroll above it.
    let rect = scroll_out.inner.inner_rect;
    let content_h = scroll_out.inner.content_size.y;
    let max_offset = (content_h - rect.height()).max(0.0);
    let offset_y = scroll_out.inner.state.offset.y;
    if offset_y > 0.5 || content_h > rect.height() {
        let fade_h = 28.0_f32.min(rect.height() * 0.45);
        let fade_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), fade_h));
        let (top, bot) = (bg_alpha(255), bg_alpha(0));
        paint_gradient(ui, fade_rect, [top, top, bot, bot]);
    }

    // Small clickable text at bottom-right to jump back to the latest line when scrolled up.
    if max_offset > 1.0 && offset_y < max_offset - 4.0 {
        let label = "↓ Scroll to bottom";
        let font = regular_font(11.0);
        let galley = ui
            .painter()
            .layout_no_wrap(label.to_string(), font.clone(), TEXT_MUTED);
        let pad = Vec2::new(6.0, 2.0);
        let btn_size = galley.size() + pad * 2.0;
        let btn_rect = Rect::from_min_size(
            Pos2::new(
                rect.right() - btn_size.x - 8.0,
                rect.bottom() - btn_size.y - 2.0,
            ),
            btn_size,
        );
        let resp = ui.interact(
            btn_rect,
            Id::new("setup_console_scroll_bottom"),
            Sense::click(),
        );
        let hovered = resp.hovered();
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // Soft seamless backing in BG_MAIN so the label stays readable over log lines.
        let backing = bg_alpha(235);
        let fade_left = Rect::from_min_size(
            Pos2::new(btn_rect.left() - 16.0, btn_rect.top()),
            Vec2::new(16.0, btn_rect.height()),
        );
        let clear = bg_alpha(0);
        paint_gradient(ui, fade_left, [clear, backing, backing, clear]);
        ui.painter()
            .rect_filled(btn_rect, CornerRadius::ZERO, backing);

        let color = if hovered { TEXT_PRIMARY } else { TEXT_MUTED };
        ui.painter()
            .text(btn_rect.center(), Align2::CENTER_CENTER, label, font, color);

        if resp.clicked() {
            let mut state = scroll_out.inner.state;
            state.offset.y = max_offset;
            state.store(ui.ctx(), scroll_out.inner.id);
            ui.ctx().request_repaint();
        }
    }
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

fn truncate_banner_text(s: &str, max_chars: usize) -> String {
    let single_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

pub(crate) fn draw_notice_box(
    ui: &mut egui::Ui,
    fill: Color32,
    border: Color32,
    text_color: Color32,
    text: &str,
) {
    let width = ui.available_width();
    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .inner_margin(Margin::symmetric(10, 7))
        .show(ui, |ui| {
            // 10px left + 10px right inner margin + 1px left + 1px right stroke = 22px.
            ui.set_width((width - 22.0).max(140.0));
            card_text(
                ui,
                egui::Label::new(
                    egui::RichText::new(text)
                        .font(regular_font(12.0))
                        .color(text_color),
                )
                .wrap(),
            );
        });
}

pub(crate) fn extension_badge(ui: &mut egui::Ui, text: &str, small: bool) {
    let (pad_x, pad_y, font_size, color) = if small {
        (5, 1, 10.0, TEXT_MUTED)
    } else {
        (6, 2, 11.0, TEXT_NAV)
    };
    Frame::new()
        .fill(BG_SIDEBAR)
        .stroke(Stroke::new(1.0, BORDER_SIDEBAR))
        .inner_margin(Margin::symmetric(pad_x, pad_y))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(text)
                        .font(regular_font(font_size))
                        .color(color),
                )
                .wrap_mode(egui::TextWrapMode::Extend)
                .selectable(false),
            );
        });
}

/// Lay out a list of `(label, small)` badges into neat horizontal rows that
/// wrap whole pills to the next line rather than squeezing individual words or
/// characters into vertical columns.
pub(crate) fn draw_badge_flow(ui: &mut egui::Ui, badges: &[(&str, bool)], max_width: f32) {
    if badges.is_empty() {
        return;
    }
    let gap_x = 6.0;
    let gap_y = 6.0;
    let widths: Vec<f32> = badges
        .iter()
        .map(|&(text, small)| {
            let (pad_x, font_size) = if small { (5.0, 10.0) } else { (6.0, 11.0) };
            let text_w = ui.fonts_mut(|f| {
                f.layout_no_wrap(text.to_owned(), regular_font(font_size), TEXT_NAV)
                    .size()
                    .x
            });
            text_w + pad_x * 2.0 + 4.0
        })
        .collect();

    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_row: Vec<usize> = Vec::new();
    let mut cur_w = 0.0f32;
    for (i, &w) in widths.iter().enumerate() {
        let add = if cur_row.is_empty() { w } else { gap_x + w };
        if !cur_row.is_empty() && cur_w + add > max_width {
            rows.push(cur_row);
            cur_row = vec![i];
            cur_w = w;
        } else {
            cur_row.push(i);
            cur_w += add;
        }
    }
    if !cur_row.is_empty() {
        rows.push(cur_row);
    }

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = gap_y;
        for row in rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap_x;
                for i in row {
                    let (text, small) = badges[i];
                    extension_badge(ui, text, small);
                }
            });
        }
    });
}

pub(crate) fn extension_toggle(
    ui: &mut egui::Ui,
    id_salt: &str,
    enabled: bool,
    interactive: bool,
) -> bool {
    let _id = ui.make_persistent_id(("ext_toggle", id_salt));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 18.0), Sense::click());
    let hovered = interactive && resp.hovered();
    let track_fill = if !interactive {
        Color32::from_rgb(0xe8, 0xe8, 0xe5)
    } else if enabled {
        DOT_READY
    } else if hovered {
        Color32::from_rgb(0xc8, 0xc8, 0xc4)
    } else {
        Color32::from_rgb(0xd8, 0xd8, 0xd4)
    };
    let track_border = if !interactive {
        BORDER_SIDEBAR
    } else if enabled {
        DOT_READY
    } else {
        BTN_BORDER
    };
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        track_fill,
        Stroke::new(1.0, track_border),
        StrokeKind::Inside,
    );
    let knob_x = if enabled {
        rect.right() - 16.0
    } else {
        rect.left() + 2.0
    };
    let knob_rect = Rect::from_min_size(Pos2::new(knob_x, rect.top() + 2.0), Vec2::new(14.0, 14.0));
    ui.painter().rect_filled(
        knob_rect,
        CornerRadius::ZERO,
        if interactive { BG_MAIN } else { BG_SIDEBAR },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    interactive && resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use extensions::{ExtensionOrigin, OPENBF_VR_ID};

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

        // Exercise the fading setup console with scrollable lines.
        app.last_build = Some(BuildProgress {
            running: true,
            step_index: 2,
            step_total: crate::pipeline::STEP_COUNT,
            step_label: "the discs".to_string(),
            lines: (0..25).map(|i| format!("line {i}")).collect(),
            outcome: None,
        });

        assert_eq!(app.page, None);
        for page in [
            None,
            Some(Page::Setup),
            Some(Page::Mods),
            Some(Page::Options),
        ] {
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
    fn vr_extension_defaults_disabled_and_persists_toggle() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_mods_persist_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut app = LauncherApp::new(paths.clone(), || {});

        assert!(app
            .extension_catalog
            .iter()
            .any(|m| m.id == OPENBF_VR_ID && m.origin == ExtensionOrigin::Bundled));
        assert!(!app.extension_profile.is_enabled(OPENBF_VR_ID));

        app.set_extension_enabled(OPENBF_VR_ID, true);
        assert!(app.extension_profile.is_enabled(OPENBF_VR_ID));

        let reloaded = LauncherApp::new(paths, || {});
        assert!(reloaded.extension_profile.is_enabled(OPENBF_VR_ID));

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn collect_shape_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                out.push((
                    t.galley.text().to_owned(),
                    Rect::from_min_size(t.pos, t.galley.size()),
                ));
            }
            egui::Shape::Vec(items) => {
                for s in items {
                    collect_shape_texts(s, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn mods_cards_start_collapsed_and_expand_when_clicked() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_mods_expand_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut app = LauncherApp::new(paths, || {});
        app.page = Some(Page::Mods);

        let ctx = egui::Context::default();
        LauncherApp::configure_style(&ctx);
        let base_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };

        let _ = ctx.run(base_input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let out = ctx.run(base_input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            collect_shape_texts(&clipped.shape, &mut texts);
        }
        let vr_header_rect = texts
            .iter()
            .find(|(text, _)| text == "OpenBF VR")
            .expect("OpenBF VR header")
            .1;
        assert!(texts.iter().any(|(t, _)| t == "Bundled"));
        assert!(!texts.iter().any(|(t, _)| t == "Engine Extension"));
        assert!(!texts.iter().any(|(t, _)| t.contains("Capabilities")));
        assert!(!texts
            .iter()
            .any(|(t, _)| t.contains("Active display backend")));
        assert!(!texts
            .iter()
            .any(|(t, _)| t.contains("OpenXR stereo headset")));

        // Click the header label to expand the card.
        let pos = vr_header_rect.center();
        let mut hover = base_input.clone();
        hover.events = vec![egui::Event::PointerMoved(pos)];
        let _ = ctx.run(hover, |ctx| {
            let _ = app.ui(ctx);
        });
        let mut down = base_input.clone();
        down.events = vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }];
        let _ = ctx.run(down, |ctx| {
            let _ = app.ui(ctx);
        });
        let mut up = base_input.clone();
        up.events = vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }];
        let out_expanded = ctx.run(up, |ctx| {
            let _ = app.ui(ctx);
        });
        let mut expanded_texts = Vec::new();
        for clipped in &out_expanded.shapes {
            collect_shape_texts(&clipped.shape, &mut expanded_texts);
        }
        assert!(
            expanded_texts
                .iter()
                .any(|(t, _)| t.contains("OpenXR stereo headset")),
            "expected expanded description at {pos:?}; got {expanded_texts:#?}"
        );
        assert!(expanded_texts.iter().any(|(t, _)| t == "Engine Extension"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_ui_renders_startup_error_recovery_banner_without_panic() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_error_banner_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let report = StartupErrorReport::new(
            OPENBF_VR_ID,
            "OpenBF VR",
            "No compatible OpenXR runtime found.",
            "xrEnumerateInstanceExtensionProperties returned ERROR_RUNTIME_UNAVAILABLE",
            true,
        );
        report.save(&paths.startup_error_path()).unwrap();

        let mut app = LauncherApp::new(paths, || {});
        assert!(app.startup_error.is_some());
        app.game_ready = true;
        app.set_extension_enabled(OPENBF_VR_ID, true);

        let ctx = egui::Context::default();
        LauncherApp::configure_style(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            let _ = app.ui(ctx);
        });
        assert!(!out.shapes.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_dots_reflect_missing_cleared_building_and_ready() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_dots_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = LauncherPaths::from_exe(&dir.join("openbf-launcher"), None);
        let mut app = LauncherApp::new(paths, || {});

        // Fresh install: both red.
        assert_eq!(app.bf1_dot_state(), DotState::Missing);
        assert_eq!(app.bf2_dot_state(), DotState::Missing);

        // BF2-only built game: BF1 red, BF2 green.
        app.game_ready = true;
        assert_eq!(app.bf1_dot_state(), DotState::Missing);
        assert_eq!(app.bf2_dot_state(), DotState::Ready);

        // Full BF1+BF2 built game: both green.
        app.built_has_bf1 = true;
        assert_eq!(app.bf1_dot_state(), DotState::Ready);
        assert_eq!(app.bf2_dot_state(), DotState::Ready);

        // Clearing BF1 immediately turns BF1 red while BF2 stays green.
        app.bf1_cleared = true;
        assert_eq!(app.bf1_dot_state(), DotState::Missing);
        assert_eq!(app.bf2_dot_state(), DotState::Ready);

        // Clearing BF2 immediately turns BF2 red as well.
        app.bf2_cleared = true;
        assert_eq!(app.bf2_dot_state(), DotState::Missing);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
