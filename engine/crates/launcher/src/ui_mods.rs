//! Launcher Mods tab UI: bundled engine extensions + filesystem-discovered
//! legacy Battlefront II content mods with manual priority load order.
//!
//! Filesystem `openbf_data/mods/` is the sole source of truth — there is
//! intentionally no Import Mod button or wizard.

use crate::config::GraphicsApi;
use crate::mods_worker::DiscoveredModState;
use crate::paths::open_folder;
use crate::ui::{
    bold_font, draw_notice_box, extension_badge, extension_toggle, flat_button, regular_font,
    LauncherApp, BG_MAIN, BG_SIDEBAR, BORDER_SIDEBAR, BTN_BORDER, DOT_BUILDING, DOT_MISSING,
    DOT_READY, INPUT_BORDER, TEXT_MUTED, TEXT_PRIMARY,
};
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, Sense, Stroke, StrokeKind,
    Vec2,
};
use extensions::mods::{resolve_content_mods, DiscoveredMod, ModCacheStatus};
use extensions::{ExtensionManifest, ExtensionOrigin, OPENBF_VR_ID};
use std::collections::BTreeSet;
use std::time::Duration;

impl LauncherApp {
    pub fn trigger_mod_scan(&self) {
        self.mods_worker.trigger_scan(self.paths.data_dir.clone());
    }

    pub(crate) fn poll_mods(&mut self, ctx: Option<&egui::Context>) {
        let snap = self.mods_worker.snapshot();
        if snap.generation != self.last_synced_mod_gen {
            self.last_synced_mod_gen = snap.generation;
            let prev_order = self.extension_profile.order.clone();
            self.extension_profile.reconcile_order(&snap.folder_ids);
            if self.extension_profile.order != prev_order {
                self.save_extension_profile();
            }
            self.mods_snapshot = snap.clone();
            self.queue_needed_conversions();
        } else {
            self.mods_snapshot = snap.clone();
        }

        if let Some(c) = ctx {
            if snap.scanning || snap.converting_id.is_some() {
                c.request_repaint_after(Duration::from_millis(80));
            }
        }
    }

    fn queue_needed_conversions(&self) {
        let ordered_ids = self
            .extension_profile
            .ordered_visible(&self.mods_snapshot.folder_ids);
        let to_convert: Vec<String> = ordered_ids
            .into_iter()
            .filter(|id| {
                self.extension_profile.is_enabled(id)
                    && self.mods_snapshot.mods.get(*id).is_some_and(|s| {
                        s.discovered.compatibility.is_playable()
                            && matches!(
                                s.cache_status,
                                ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. }
                            )
                    })
            })
            .map(str::to_owned)
            .collect();
        if !to_convert.is_empty() {
            self.mods_worker.queue_conversions(
                self.paths.bake_bin(),
                self.paths.data_dir.clone(),
                to_convert,
            );
        }
    }

    pub fn set_content_mod_enabled(&mut self, id: &str, enabled: bool) {
        self.extension_profile.set_enabled(id, enabled);
        self.save_extension_profile();
        if enabled {
            self.queue_needed_conversions();
        }
    }

    pub fn move_mod_up(&mut self, id: &str) {
        if self
            .extension_profile
            .move_up(id, &self.mods_snapshot.folder_ids)
        {
            self.save_extension_profile();
        }
    }

    pub fn move_mod_down(&mut self, id: &str) {
        if self
            .extension_profile
            .move_down(id, &self.mods_snapshot.folder_ids)
        {
            self.save_extension_profile();
        }
    }

    /// Blocking reason preventing `Play` from launching when enabled content mods
    /// or extensions are unready or misordered.
    pub fn play_blocking_reason(&self) -> Option<String> {
        if let Err(errs) = self.resolve_extensions() {
            if let Some(first) = errs.first() {
                return Some(first.to_string());
            }
        }
        let discovered: Vec<DiscoveredMod> = self
            .mods_snapshot
            .mods
            .values()
            .map(|s| s.discovered.clone())
            .collect();
        if let Err(errs) = resolve_content_mods(&discovered, &self.extension_profile) {
            if let Some(first) = errs.first() {
                return Some(first.to_string());
            }
        }
        for id in &self.mods_snapshot.folder_ids {
            if !self.extension_profile.is_enabled(id) {
                continue;
            }
            let Some(state) = self.mods_snapshot.mods.get(id) else {
                continue;
            };
            match &state.cache_status {
                ModCacheStatus::Ready | ModCacheStatus::NotApplicable => {}
                ModCacheStatus::Converting => {
                    return Some(format!(
                        "Preparing mod `{}` ({}%)...",
                        state.discovered.display_name, state.progress_pct
                    ));
                }
                ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. } => {
                    return Some(format!(
                        "Waiting to convert `{}`...",
                        state.discovered.display_name
                    ));
                }
                ModCacheStatus::Failed(err) => {
                    return Some(format!(
                        "Enabled mod `{}` failed to prepare: {err}",
                        state.discovered.display_name
                    ));
                }
            }
        }
        None
    }

    pub(crate) fn draw_mods_page(&mut self, ui: &mut egui::Ui) {
        let mods_dir = self.paths.mods_dir();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Mods")
                    .font(bold_font(17.0))
                    .color(TEXT_PRIMARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if flat_button(ui, "Refresh", Vec2::new(72.0, 24.0), true) {
                    self.refresh_fs_state();
                }
                if flat_button(ui, "Open Mods Folder", Vec2::new(124.0, 24.0), true) {
                    if let Err(e) = open_folder(&mods_dir) {
                        self.show_toast(format!("Could not open mods folder: {e}"));
                    }
                }
            });
        });
        ui.add_space(12.0);

        let catalog = self.extension_catalog.clone();
        let ext_resolution = self.resolve_extensions();
        let dx12_with_vr = self.extension_profile.is_enabled(OPENBF_VR_ID)
            && self.config.graphics_api == GraphicsApi::Dx12;

        let snap = self.mods_snapshot.clone();
        let discovered_vec: Vec<DiscoveredMod> =
            snap.mods.values().map(|s| s.discovered.clone()).collect();
        let content_resolution = resolve_content_mods(&discovered_vec, &self.extension_profile);

        let scroll_h = (ui.available_height() - 54.0).max(96.0);
        let width = ui.available_width();
        egui::ScrollArea::vertical()
            .id_salt("mods_list")
            .max_width(width)
            .max_height(scroll_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width((width - 8.0).max(160.0));
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                match &ext_resolution {
                    Ok(plan) => {
                        for warning in &plan.warnings {
                            draw_notice_box(
                                ui,
                                Color32::from_rgb(0xff, 0xfb, 0xeb),
                                DOT_BUILDING,
                                DOT_BUILDING,
                                warning,
                            );
                        }
                    }
                    Err(errs) => {
                        for err in errs {
                            draw_notice_box(
                                ui,
                                Color32::from_rgb(0xfd, 0xf3, 0xf2),
                                DOT_MISSING,
                                DOT_MISSING,
                                &format!("Extension conflict: {err}"),
                            );
                        }
                    }
                }

                if dx12_with_vr {
                    draw_notice_box(
                        ui,
                        Color32::from_rgb(0xff, 0xfb, 0xeb),
                        DOT_BUILDING,
                        TEXT_PRIMARY,
                        "Options -> Graphics API is Direct3D 12. OpenBF VR requires Vulkan for OpenXR interop and overrides Dx12 while active.",
                    );
                }

                if let Some(scan_err) = &snap.scan_error {
                    draw_notice_box(
                        ui,
                        Color32::from_rgb(0xfd, 0xf3, 0xf2),
                        DOT_MISSING,
                        DOT_MISSING,
                        &format!("Mod scan error: {scan_err}"),
                    );
                }

                match &content_resolution {
                    Ok(plan) => {
                        if !plan.conflicts.is_empty() {
                            let summary: Vec<String> = plan
                                .conflicts
                                .iter()
                                .map(|c| {
                                    format!(
                                        "{}: `{}` overrides {}",
                                        c.resource_path,
                                        c.winner_name,
                                        c.overridden_names.join(", ")
                                    )
                                })
                                .collect();
                            draw_notice_box(
                                ui,
                                Color32::from_rgb(0xff, 0xfb, 0xeb),
                                DOT_BUILDING,
                                TEXT_PRIMARY,
                                &format!(
                                    "Override priority active (higher in list wins): {}",
                                    summary.join("; ")
                                ),
                            );
                        }
                    }
                    Err(errs) => {
                        for err in errs {
                            draw_notice_box(
                                ui,
                                Color32::from_rgb(0xfd, 0xf3, 0xf2),
                                DOT_MISSING,
                                DOT_MISSING,
                                &err.to_string(),
                            );
                        }
                    }
                }

                // Engine Extensions section (capability-based, not user-ordered).
                ui.label(
                    egui::RichText::new("Engine Extensions")
                        .font(bold_font(12.5))
                        .color(TEXT_MUTED),
                );
                if catalog.is_empty() {
                    ui.label(
                        egui::RichText::new("No bundled engine extensions found.")
                            .font(regular_font(12.0))
                            .color(TEXT_MUTED),
                    );
                } else {
                    for manifest in &catalog {
                        if let Some(next_enabled) = self.draw_extension_card(ui, manifest) {
                            self.set_extension_enabled(&manifest.id, next_enabled);
                        }
                    }
                }

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Content Mods")
                            .font(bold_font(12.5))
                            .color(TEXT_MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Higher in list = higher override priority")
                                .font(regular_font(11.0))
                                .color(TEXT_MUTED),
                        );
                    });
                });

                let ordered_ids: Vec<String> = self
                    .extension_profile
                    .ordered_visible(&snap.folder_ids)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();

                if ordered_ids.is_empty() {
                    draw_notice_box(
                        ui,
                        BG_SIDEBAR,
                        INPUT_BORDER,
                        TEXT_MUTED,
                        &format!(
                            "No legacy mods found in {}. Copy a Battlefront II addon folder (such as ABR/) into mods/ and click Refresh.",
                            mods_dir.display()
                        ),
                    );
                } else {
                    let conflict_ids: BTreeSet<String> = content_resolution
                        .as_ref()
                        .map(|p| {
                            p.conflicts
                                .iter()
                                .flat_map(|c| {
                                    std::iter::once(c.winner_id.clone())
                                        .chain(c.overridden_ids.iter().cloned())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let total = ordered_ids.len();
                    for (idx, mod_id) in ordered_ids.iter().enumerate() {
                        if let Some(state) = snap.mods.get(mod_id) {
                            self.draw_content_mod_card(
                                ui,
                                state,
                                idx,
                                total,
                                conflict_ids.contains(mod_id),
                            );
                        }
                    }
                }
            });
    }

    fn draw_content_mod_card(
        &mut self,
        ui: &mut egui::Ui,
        state: &DiscoveredModState,
        idx: usize,
        total: usize,
        has_override_conflict: bool,
    ) {
        let info = &state.discovered;
        let playable = info.compatibility.is_playable();
        let enabled = playable && self.extension_profile.is_enabled(&info.id);
        let expand_id = ui.make_persistent_id(("content_mod_expanded", &info.id));
        let mut expanded = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
        let card_w = (ui.available_width() - 24.0).max(160.0);

        let border_color = if !playable || matches!(state.cache_status, ModCacheStatus::Failed(_)) {
            DOT_MISSING
        } else if enabled {
            match state.cache_status {
                ModCacheStatus::Ready => DOT_READY,
                ModCacheStatus::Converting
                | ModCacheStatus::NeedsConversion
                | ModCacheStatus::Stale { .. } => DOT_BUILDING,
                _ => INPUT_BORDER,
            }
        } else {
            INPUT_BORDER
        };

        Frame::new()
            .fill(BG_MAIN)
            .stroke(Stroke::new(1.0, border_color))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(card_w);
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 5.0);

                ui.horizontal(|ui| {
                    extension_badge(ui, &format!("#{}", idx + 1), true);

                    if small_order_button(ui, "^", idx > 0) {
                        self.move_mod_up(&info.id);
                    }
                    if small_order_button(ui, "v", idx + 1 < total) {
                        self.move_mod_down(&info.id);
                    }

                    let header_resp = ui
                        .scope_builder(
                            egui::UiBuilder::new()
                                .id_salt(expand_id.with("header"))
                                .sense(Sense::click()),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&info.display_name)
                                                .font(bold_font(13.5))
                                                .color(TEXT_PRIMARY),
                                        )
                                        .selectable(false),
                                    );
                                    if let Some(leg) = &info.legacy_addon_id {
                                        extension_badge(ui, leg, true);
                                    }
                                    extension_badge(ui, info.mod_type.label(), true);
                                    if has_override_conflict && enabled {
                                        extension_badge(ui, "Override", true);
                                    }
                                });
                            },
                        )
                        .response;
                    if header_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if header_resp.clicked() {
                        expanded = !expanded;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if extension_toggle(ui, &info.id, enabled, playable) {
                            self.set_content_mod_enabled(&info.id, !enabled);
                        }
                        let (status_text, status_color) =
                            content_status_badge(playable, enabled, state);
                        ui.label(
                            egui::RichText::new(status_text)
                                .font(regular_font(11.5))
                                .color(status_color),
                        );
                    });
                });

                if matches!(state.cache_status, ModCacheStatus::Converting) {
                    let frac = (state.progress_pct as f32 / 100.0).clamp(0.0, 1.0);
                    let (bar_rect, _) =
                        ui.allocate_exact_size(Vec2::new(card_w, 6.0), Sense::hover());
                    ui.painter()
                        .rect_filled(bar_rect, CornerRadius::ZERO, BG_SIDEBAR);
                    let fill_rect = Rect::from_min_size(
                        bar_rect.min,
                        Vec2::new(bar_rect.width() * frac, bar_rect.height()),
                    );
                    ui.painter()
                        .rect_filled(fill_rect, CornerRadius::ZERO, DOT_BUILDING);
                    ui.label(
                        egui::RichText::new(&state.progress_msg)
                            .font(regular_font(11.0))
                            .color(TEXT_MUTED),
                    );
                }

                if let ModCacheStatus::Failed(err) = &state.cache_status {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("Conversion failed: {err}"))
                                    .font(regular_font(11.5))
                                    .color(DOT_MISSING),
                            )
                            .wrap(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if flat_button(ui, "Retry", Vec2::new(56.0, 22.0), true) {
                                self.mods_worker.retry_conversion(
                                    self.paths.bake_bin(),
                                    self.paths.data_dir.clone(),
                                    info.id.clone(),
                                );
                            }
                        });
                    });
                } else if !playable {
                    let reason = info
                        .compatibility
                        .reason()
                        .unwrap_or("Mod layout is invalid or unsupported.");
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(reason)
                                .font(regular_font(11.5))
                                .color(DOT_MISSING),
                        )
                        .wrap(),
                    );
                }

                if expanded {
                    ui.add_space(2.0);
                    let mut meta_parts = vec![
                        format!("Folder: {}", info.folder_name),
                        format!("ID: {}", info.id),
                        format!("Namespace: {}", info.namespace),
                    ];
                    if let Some(ver) = &info.version {
                        meta_parts.push(format!("v{ver}"));
                    }
                    if let Some(author) = &info.author {
                        meta_parts.push(format!("Author: {author}"));
                    }
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(meta_parts.join("  |  "))
                                .font(regular_font(11.0))
                                .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
                    if !info.description.is_empty() {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&info.description)
                                    .font(regular_font(12.0))
                                    .color(TEXT_PRIMARY),
                            )
                            .wrap(),
                        );
                    }
                    for map in &info.maps {
                        let cw = if map.modes_cw.is_empty() {
                            "-".to_owned()
                        } else {
                            map.modes_cw.join(", ")
                        };
                        let gcw = if map.modes_gcw.is_empty() {
                            "-".to_owned()
                        } else {
                            map.modes_gcw.join(", ")
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "Map: {} [{}] (CW: {cw} | GCW: {gcw})",
                                    map.display_name, map.legacy_code
                                ))
                                .font(regular_font(11.5))
                                .color(TEXT_PRIMARY),
                            )
                            .wrap(),
                        );
                        for mission in &map.missions {
                            let status = if mission.supported {
                                "supported".to_owned()
                            } else {
                                format!(
                                    "skipped: {}",
                                    mission
                                        .unsupported_reason
                                        .as_deref()
                                        .unwrap_or("unsupported")
                                )
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "  - {} ({}/{}): {status}",
                                        mission.script_name, mission.era, mission.mode
                                    ))
                                    .font(regular_font(11.0))
                                    .color(
                                        if mission.supported {
                                            TEXT_MUTED
                                        } else {
                                            DOT_BUILDING
                                        },
                                    ),
                                )
                                .wrap(),
                            );
                        }
                    }
                    if !info.custom_sides.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "Custom sides: {}",
                                info.custom_sides.join(", ")
                            ))
                            .font(regular_font(11.0))
                            .color(TEXT_MUTED),
                        );
                    }
                    if !info.declared_overrides.is_empty() {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "Stock overrides: {}",
                                    info.declared_overrides.join(", ")
                                ))
                                .font(regular_font(11.0))
                                .color(DOT_BUILDING),
                            )
                            .wrap(),
                        );
                    }
                    for w in &info.warnings {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("Warning: {w}"))
                                    .font(regular_font(11.0))
                                    .color(DOT_BUILDING),
                            )
                            .wrap(),
                        );
                    }
                }
            });
        ui.ctx().data_mut(|d| d.insert_temp(expand_id, expanded));
    }

    pub(crate) fn draw_extension_card(
        &self,
        ui: &mut egui::Ui,
        manifest: &ExtensionManifest,
    ) -> Option<bool> {
        let enabled = manifest.supported && self.extension_profile.is_enabled(&manifest.id);
        let expand_id = ui.make_persistent_id(("ext_expanded", &manifest.id));
        let mut expanded = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
        let mut toggled = None;
        let card_w = (ui.available_width() - 24.0).max(160.0);
        Frame::new()
            .fill(BG_MAIN)
            .stroke(Stroke::new(
                1.0,
                if enabled { DOT_READY } else { INPUT_BORDER },
            ))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(card_w);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);

                ui.horizontal(|ui| {
                    let header_resp = ui
                        .scope_builder(
                            egui::UiBuilder::new()
                                .id_salt(expand_id.with("header"))
                                .sense(Sense::click()),
                            |ui| {
                                ui.horizontal(|ui| {
                                    let (chev_rect, _) = ui
                                        .allocate_exact_size(Vec2::new(10.0, 16.0), Sense::hover());
                                    let cx = chev_rect.center().x;
                                    let cy = chev_rect.center().y;
                                    let chev_stroke =
                                        Stroke::new(1.3, Color32::from_rgb(0x55, 0x55, 0x55));
                                    if expanded {
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(cx - 3.5, cy - 1.5),
                                                Pos2::new(cx, cy + 2.0),
                                            ],
                                            chev_stroke,
                                        );
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(cx, cy + 2.0),
                                                Pos2::new(cx + 3.5, cy - 1.5),
                                            ],
                                            chev_stroke,
                                        );
                                    } else {
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(cx - 1.5, cy - 3.5),
                                                Pos2::new(cx + 2.0, cy),
                                            ],
                                            chev_stroke,
                                        );
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(cx + 2.0, cy),
                                                Pos2::new(cx - 1.5, cy + 3.5),
                                            ],
                                            chev_stroke,
                                        );
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&manifest.name)
                                                .font(bold_font(14.0))
                                                .color(TEXT_PRIMARY),
                                        )
                                        .selectable(false),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!("v{}", manifest.version))
                                                .font(regular_font(12.0))
                                                .color(TEXT_MUTED),
                                        )
                                        .selectable(false),
                                    );
                                    if manifest.origin == ExtensionOrigin::Bundled {
                                        extension_badge(ui, manifest.origin.label(), false);
                                    }
                                });
                            },
                        )
                        .response;
                    if header_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if header_resp.clicked() {
                        expanded = !expanded;
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let status = if !manifest.supported {
                            "Unsupported"
                        } else if enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        };
                        let status_color = if !manifest.supported {
                            DOT_MISSING
                        } else if enabled {
                            DOT_READY
                        } else {
                            TEXT_MUTED
                        };
                        if extension_toggle(ui, &manifest.id, enabled, manifest.supported) {
                            toggled = Some(!enabled);
                        }
                        ui.label(
                            egui::RichText::new(status)
                                .font(regular_font(12.0))
                                .color(status_color),
                        );
                    });
                });

                if expanded {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&manifest.description)
                                .font(regular_font(12.5))
                                .color(TEXT_PRIMARY),
                        )
                        .wrap(),
                    );
                    ui.horizontal(|ui| {
                        extension_badge(ui, manifest.kind.label(), true);
                    });

                    if !manifest.dependencies.is_empty() || !manifest.conflicts.is_empty() {
                        let mut rel = Vec::new();
                        if !manifest.dependencies.is_empty() {
                            rel.push(format!("Requires: {}", manifest.dependencies.join(", ")));
                        }
                        if !manifest.conflicts.is_empty() {
                            rel.push(format!("Conflicts: {}", manifest.conflicts.join(", ")));
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(rel.join("  |  "))
                                    .font(regular_font(11.5))
                                    .color(TEXT_MUTED),
                            )
                            .wrap(),
                        );
                    }
                }

                if !manifest.supported {
                    let reason = manifest
                        .unsupported_reason
                        .as_deref()
                        .unwrap_or("Not supported on this platform or build.");
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("Unavailable: {reason}"))
                                .font(regular_font(11.5))
                                .color(DOT_MISSING),
                        )
                        .wrap(),
                    );
                }
            });
        ui.ctx().data_mut(|d| d.insert_temp(expand_id, expanded));
        toggled
    }
}

fn content_status_badge(
    playable: bool,
    enabled: bool,
    state: &DiscoveredModState,
) -> (String, Color32) {
    if !playable {
        return (state.discovered.compatibility.label().into(), DOT_MISSING);
    }
    if !enabled {
        return ("Disabled".into(), TEXT_MUTED);
    }
    match &state.cache_status {
        ModCacheStatus::Ready => ("Ready".into(), DOT_READY),
        ModCacheStatus::Converting => (format!("Converting {}%", state.progress_pct), DOT_BUILDING),
        ModCacheStatus::NeedsConversion => ("Pending".into(), DOT_BUILDING),
        ModCacheStatus::Stale { .. } => ("Rebuild".into(), DOT_BUILDING),
        ModCacheStatus::Failed(_) => ("Failed".into(), DOT_MISSING),
        ModCacheStatus::NotApplicable => ("Unsupported".into(), DOT_MISSING),
    }
}

fn small_order_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::click());
    let hovered = enabled && resp.hovered();
    let bg = if !enabled {
        BG_SIDEBAR
    } else if hovered {
        Color32::from_rgb(0xea, 0xea, 0xe7)
    } else {
        BG_MAIN
    };
    ui.painter().rect(
        rect,
        CornerRadius::ZERO,
        bg,
        Stroke::new(1.0, if enabled { BTN_BORDER } else { BORDER_SIDEBAR }),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        regular_font(11.0),
        if enabled { TEXT_PRIMARY } else { TEXT_MUTED },
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    enabled && resp.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{LauncherPaths, EXE_SUFFIX};
    use crate::ui::{Page, WINDOW_HEIGHT, WINDOW_WIDTH};
    use egui::{Pos2, Rect, Vec2};

    fn collect_texts(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(items) => {
                for s in items {
                    collect_texts(s, out);
                }
            }
            _ => {}
        }
    }

    fn wait_for_scan(app: &mut LauncherApp, ctx: &egui::Context, input: &egui::RawInput) {
        for _ in 0..50 {
            let _ = ctx.run(input.clone(), |ctx| {
                let _ = app.ui(ctx);
            });
            if !app.mods_snapshot.scanning && app.last_synced_mod_gen > 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for background mod scan");
    }

    #[test]
    fn auto_discovers_mods_without_import_and_persists_load_order() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_ui_mods_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        let mods = paths.mods_dir();
        for name in ["AlphaMod", "BetaMod"] {
            let side = mods.join(name).join("data/_LVL_PC/SIDE");
            std::fs::create_dir_all(&side).unwrap();
            std::fs::write(side.join("all.lvl"), b"ucfb").unwrap();
        }

        let mut app = LauncherApp::new(paths.clone(), || {});
        app.page = Some(Page::Mods);
        let ctx = egui::Context::default();
        LauncherApp::configure_style(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };
        wait_for_scan(&mut app, &ctx, &input);

        let out = ctx.run(input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut rendered = Vec::new();
        for clipped in &out.shapes {
            collect_texts(&clipped.shape, &mut rendered);
        }
        assert!(rendered.iter().any(|t| t == "Open Mods Folder"));
        assert!(rendered.iter().any(|t| t == "Refresh"));
        assert!(!rendered.iter().any(|t| t.contains("Import Mod")));
        assert!(rendered.iter().any(|t| t == "AlphaMod"));
        assert!(rendered.iter().any(|t| t == "BetaMod"));

        // Reorder BetaMod above AlphaMod, toggle BetaMod disabled, verify order persists across restart.
        app.move_mod_up("content.betamod");
        app.set_content_mod_enabled("content.betamod", false);
        assert_eq!(
            app.extension_profile
                .ordered_visible(&app.mods_snapshot.folder_ids),
            vec!["content.betamod", "content.alphamod"]
        );

        let mut reloaded = LauncherApp::new(paths.clone(), || {});
        wait_for_scan(&mut reloaded, &ctx, &input);
        assert_eq!(
            reloaded
                .extension_profile
                .ordered_visible(&reloaded.mods_snapshot.folder_ids),
            vec!["content.betamod", "content.alphamod"]
        );
        assert!(!reloaded.extension_profile.is_enabled("content.betamod"));

        // VerifyPlay blocking behavior: explicit dependency order violation blocks Play.
        std::fs::write(
            mods.join("AlphaMod").join("openbf_mod.ron"),
            "(dependencies: [\"content.betamod\"])",
        )
        .unwrap();
        reloaded.refresh_fs_state();
        let prev_gen = reloaded.last_synced_mod_gen;
        for _ in 0..50 {
            let _ = ctx.run(input.clone(), |ctx| {
                let _ = reloaded.ui(ctx);
            });
            if reloaded.last_synced_mod_gen > prev_gen && !reloaded.mods_snapshot.scanning {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        reloaded.set_content_mod_enabled("content.alphamod", true);
        reloaded
            .extension_profile
            .set_enabled("content.betamod", true);
        reloaded.save_extension_profile();
        // BetaMod is above AlphaMod, but AlphaMod depends on BetaMod -> order error blocks Play!
        assert!(reloaded.play_blocking_reason().is_some());
        reloaded.move_mod_up("content.alphamod");
        // Cache is still pending conversion (or failed without bake), so fix dependency error is cleared:
        assert!(resolve_content_mods(
            &reloaded
                .mods_snapshot
                .mods
                .values()
                .map(|s| s.discovered.clone())
                .collect::<Vec<_>>(),
            &reloaded.extension_profile,
        )
        .is_ok());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
