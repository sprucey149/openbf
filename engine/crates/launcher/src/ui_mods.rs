//! Launcher Mods tab UI: bundled engine extensions + filesystem-discovered
//! legacy Battlefront II content mods with manual priority load order.
//!
//! Filesystem `openbf_data/mods/` is the sole source of truth — there is
//! intentionally no Import Mod button or wizard.

use crate::config::GraphicsApi;
use crate::mods_worker::{DiscoveredModState, ModsEnv};
use crate::paths::open_folder;
use crate::ui::{
    bold_font, draw_badge_flow, draw_notice_box, extension_badge, extension_toggle, flat_button,
    regular_font, LauncherApp, BG_MAIN, BG_SIDEBAR, DOT_BUILDING, DOT_MISSING, DOT_READY,
    INPUT_BORDER, TEXT_MUTED, TEXT_PRIMARY,
};
use egui::text_selection::LabelSelectionState;
use egui::{Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, Sense, Stroke, Vec2};
use extensions::mods::{resolve_content_mods, DiscoveredMod, ModCacheStatus, OpStatus, ResourceOp};
use extensions::{ExtensionManifest, ExtensionOrigin, OPENBF_VR_ID};
use std::collections::BTreeSet;
use std::time::Duration;

impl LauncherApp {
    pub(crate) fn mods_env(&self) -> ModsEnv {
        ModsEnv {
            data_dir: self.paths.data_dir.clone(),
            stock: self.paths.stock_assets(),
        }
    }

    pub fn trigger_mod_scan(&self) {
        self.mods_worker.trigger_scan(self.mods_env());
    }

    fn all_visible_mod_ids(&self, folder_ids: &[String]) -> Vec<String> {
        let mut ids: Vec<String> = folder_ids.to_vec();
        for ext in &self.extension_catalog {
            if !ids.iter().any(|existing| existing == &ext.id) {
                ids.push(ext.id.clone());
            }
        }
        ids
    }

    pub(crate) fn poll_mods(&mut self, ctx: Option<&egui::Context>) {
        let snap = self.mods_worker.snapshot();
        if snap.generation != self.last_synced_mod_gen {
            self.last_synced_mod_gen = snap.generation;
            let visible_ids = self.all_visible_mod_ids(&snap.folder_ids);
            let prev_order = self.extension_profile.order.clone();
            self.extension_profile.reconcile_order(&visible_ids);
            if self.extension_profile.order != prev_order {
                self.save_extension_profile();
            }
            self.mods_snapshot = snap.clone();
            self.queue_needed_conversions();
        } else {
            self.mods_snapshot = snap.clone();
        }

        if let Some(c) = ctx {
            if snap.scanning || snap.inspecting_id.is_some() || snap.converting_id.is_some() {
                c.request_repaint_after(Duration::from_millis(80));
            }
        }
    }

    fn queue_needed_conversions(&self) {
        let ordered_ids = self
            .extension_profile
            .ordered_visible(&self.mods_snapshot.folder_ids);
        let to_inspect: Vec<String> = ordered_ids
            .iter()
            .filter(|id| {
                self.extension_profile.is_enabled(id)
                    && self
                        .mods_snapshot
                        .mods
                        .get(**id)
                        .is_some_and(|s| !s.inspected && !s.inspecting)
            })
            .map(|id| (*id).to_owned())
            .collect();
        if !to_inspect.is_empty() {
            self.mods_worker.queue_inspect(self.mods_env(), to_inspect);
        }
        let to_convert: Vec<String> = ordered_ids
            .into_iter()
            .filter(|id| {
                self.extension_profile.is_enabled(id)
                    && self.mods_snapshot.mods.get(*id).is_some_and(|s| {
                        s.inspected
                            && !s.inspecting
                            && s.discovered.compatibility.is_playable()
                            && matches!(
                                s.cache_status,
                                ModCacheStatus::NeedsConversion | ModCacheStatus::Stale { .. }
                            )
                    })
            })
            .map(str::to_owned)
            .collect();
        if !to_convert.is_empty() {
            self.mods_worker
                .queue_conversions(self.paths.bake_bin(), self.mods_env(), to_convert);
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
        let visible_ids = self.all_visible_mod_ids(&self.mods_snapshot.folder_ids);
        if self.extension_profile.move_up(id, &visible_ids) {
            self.save_extension_profile();
        }
    }

    pub fn move_mod_down(&mut self, id: &str) {
        let visible_ids = self.all_visible_mod_ids(&self.mods_snapshot.folder_ids);
        if self.extension_profile.move_down(id, &visible_ids) {
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
        if self.mods_snapshot.scanning && self.mods_snapshot.generation == 0 {
            return Some("Scanning mods...".into());
        }
        for id in &self.mods_snapshot.folder_ids {
            if !self.extension_profile.is_enabled(id) {
                continue;
            }
            let Some(state) = self.mods_snapshot.mods.get(id) else {
                continue;
            };
            if !state.inspected || state.inspecting {
                return Some(format!(
                    "Inspecting mod `{}`...",
                    state.discovered.display_name
                ));
            }
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
        let snap = self.mods_snapshot.clone();
        let initial_loading = snap.scanning && snap.generation == 0;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Mods")
                    .font(bold_font(17.0))
                    .color(TEXT_PRIMARY),
            );
            if snap.scanning {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(if initial_loading {
                        "Loading..."
                    } else {
                        "Refreshing..."
                    })
                    .font(regular_font(12.0))
                    .color(TEXT_MUTED),
                );
            }
        });
        ui.add_space(12.0);

        let catalog = self.extension_catalog.clone();
        let ext_resolution = self.resolve_extensions();
        let dx12_with_vr = self.extension_profile.is_enabled(OPENBF_VR_ID)
            && self.config.graphics_api == GraphicsApi::Dx12;

        let discovered_vec: Vec<DiscoveredMod> =
            snap.mods.values().map(|s| s.discovered.clone()).collect();
        let content_resolution = resolve_content_mods(&discovered_vec, &self.extension_profile);

        let footer_h = 44.0;
        let scroll_h = (ui.available_height() - footer_h).max(96.0);
        let width = ui.available_width();
        // Fixed inner column width computed once from the scroll viewport so
        // Frame stroke/margins never cause successive cards to widen by +2px each.
        let list_w = (width - 14.0).max(160.0);
        let card_inner_w = (list_w - 26.0).max(160.0);
        egui::ScrollArea::vertical()
            .id_salt("mods_list")
            .max_width(width)
            .max_height(scroll_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(list_w);
                ui.set_max_width(list_w);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
                scroll_while_dragging(ui);

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

                if let Err(errs) = &content_resolution {
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

                let all_visible = self.all_visible_mod_ids(&snap.folder_ids);
                let ordered_all: Vec<String> = self
                    .extension_profile
                    .ordered_visible(&all_visible)
                    .into_iter()
                    .map(str::to_owned)
                    .collect();

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

                let total = ordered_all.len();
                for (idx, item_id) in ordered_all.iter().enumerate() {
                    if let Some(state) = snap.mods.get(item_id) {
                        self.draw_content_mod_card(
                            ui,
                            state,
                            idx,
                            total,
                            conflict_ids.contains(item_id),
                            card_inner_w,
                        );
                    } else if let Some(manifest) = catalog.iter().find(|m| m.id == *item_id) {
                        self.draw_extension_card(ui, manifest, idx, total, card_inner_w);
                    }
                }

                if initial_loading {
                    draw_notice_box(
                        ui,
                        BG_SIDEBAR,
                        INPUT_BORDER,
                        TEXT_MUTED,
                        "Loading mods...",
                    );
                } else if snap.folder_ids.is_empty() && !snap.scanning {
                    draw_notice_box(
                        ui,
                        BG_SIDEBAR,
                        INPUT_BORDER,
                        TEXT_MUTED,
                        &format!(
                            "No legacy mods found in {}. Copy an extracted Battlefront II mod folder (addon map, side/FPM pack, or loose .lvl files) into mods/ and click Refresh.",
                            mods_dir.display()
                        ),
                    );
                }
            });

        // Bottom-left action controls opposite the bottom-right BF1/BF2 status indicators.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if flat_button(ui, "Open Mods Folder", Vec2::new(124.0, 26.0), true) {
                    if let Err(e) = open_folder(&mods_dir) {
                        self.show_toast(format!("Could not open mods folder: {e}"));
                    }
                }
                if flat_button(ui, "Refresh", Vec2::new(72.0, 26.0), true) {
                    self.refresh_fs_state();
                }
            });
        });
    }

    fn draw_content_mod_card(
        &mut self,
        ui: &mut egui::Ui,
        state: &DiscoveredModState,
        idx: usize,
        total: usize,
        has_override_conflict: bool,
        card_w: f32,
    ) {
        let info = &state.discovered;
        let playable = info.compatibility.is_playable();
        let enabled = playable && self.extension_profile.is_enabled(&info.id);
        let expand_id = ui.make_persistent_id(("content_mod_expanded", &info.id));
        let mut expanded = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
        let mut action = None;
        let changes_gameplay = info.changes_gameplay();

        let border_color = if !playable || matches!(state.cache_status, ModCacheStatus::Failed(_)) {
            DOT_MISSING
        } else if enabled {
            if state.inspecting || !state.inspected {
                DOT_BUILDING
            } else {
                match state.cache_status {
                    ModCacheStatus::Ready => DOT_READY,
                    ModCacheStatus::Converting
                    | ModCacheStatus::NeedsConversion
                    | ModCacheStatus::Stale { .. } => DOT_BUILDING,
                    _ => INPUT_BORDER,
                }
            }
        } else {
            INPUT_BORDER
        };

        Frame::new()
            .fill(BG_MAIN)
            .stroke(Stroke::new(1.0_f32, border_color))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(card_w);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);

                ui.horizontal(|ui| {
                    let status = content_status_badge(playable, enabled, state);
                    draw_card_header(
                        ui,
                        card_w,
                        status.as_ref().map(|(s, _)| s.as_str()),
                        expand_id,
                        &mut expanded,
                        &info.display_name,
                        info.version.as_deref(),
                        None,
                    );

                    action = draw_card_right_controls(
                        ui,
                        &info.id,
                        enabled,
                        playable,
                        idx,
                        total,
                        status.as_ref().map(|(s, c)| (s.as_str(), *c)),
                    );
                });

                if matches!(state.cache_status, ModCacheStatus::Converting) {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_secs(1));
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
                    ui.horizontal(|ui| {
                        let elapsed = state.convert_started_at.map_or_else(String::new, |t| {
                            let secs = t.elapsed().as_secs();
                            if secs >= 60 {
                                format!(" ({}m {:02}s)", secs / 60, secs % 60)
                            } else {
                                format!(" ({secs}s)")
                            }
                        });
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new(format!("{}{elapsed}", state.progress_msg))
                                    .font(regular_font(11.0))
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if flat_button(ui, "Cancel", Vec2::new(62.0, 22.0), true) {
                                self.mods_worker.cancel_conversion();
                            }
                        });
                    });
                }

                if let ModCacheStatus::Failed(err) = &state.cache_status {
                    ui.horizontal(|ui| {
                        card_text(
                            ui,
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
                                    self.mods_env(),
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
                    card_text(
                        ui,
                        egui::Label::new(
                            egui::RichText::new(reason)
                                .font(regular_font(11.5))
                                .color(DOT_MISSING),
                        )
                        .wrap(),
                    );
                }

                if expanded {
                    if !state.inspected && !state.inspecting {
                        self.mods_worker
                            .queue_inspect(self.mods_env(), vec![info.id.clone()]);
                    }
                    if !info.description.is_empty() {
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new(&info.description)
                                    .font(regular_font(12.5))
                                    .color(TEXT_PRIMARY),
                            )
                            .wrap(),
                        );
                    }
                    if state.inspecting || !state.inspected {
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new("Inspecting LVL files...")
                                    .font(regular_font(11.5))
                                    .color(TEXT_MUTED),
                            ),
                        );
                    }
                    let mut badges: Vec<(&str, bool)> = vec![(info.mod_type.label(), true)];
                    if let Some(leg) = &info.legacy_addon_id {
                        if !leg.eq_ignore_ascii_case(&info.display_name)
                            && !leg.eq_ignore_ascii_case(&info.folder_name)
                        {
                            badges.push((leg.as_str(), false));
                        }
                    }
                    if has_override_conflict && enabled {
                        badges.push(("Override", true));
                    }
                    if changes_gameplay {
                        badges.push(("Gameplay", false));
                    }
                    for cap in &info.capabilities {
                        badges.push((cap.label(), false));
                    }
                    draw_badge_flow(ui, &badges, card_w);
                    let mut meta_parts = vec![
                        format!("Folder: {}", info.folder_name),
                        format!("ID: {}", info.id),
                        format!("Namespace: {}", info.namespace),
                    ];
                    if let Some(author) = &info.author {
                        meta_parts.push(format!("Author: {author}"));
                    }
                    card_text(
                        ui,
                        egui::Label::new(
                            egui::RichText::new(meta_parts.join("  |  "))
                                .font(regular_font(11.5))
                                .color(TEXT_MUTED),
                        )
                        .wrap(),
                    );
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
                        let mut era_parts = vec![format!("CW: {cw}"), format!("GCW: {gcw}")];
                        for (era, modes) in &map.modes_custom_eras {
                            if !modes.is_empty() {
                                let label = map
                                    .era_overrides
                                    .get(era)
                                    .and_then(|ov| ov.name.as_deref())
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| era.to_ascii_uppercase());
                                era_parts.push(format!("{label}: {}", modes.join(", ")));
                            }
                        }
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "Map: {} [{}] ({})",
                                    map.display_name,
                                    map.legacy_code,
                                    era_parts.join(" | ")
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
                            card_text(
                                ui,
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
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "Custom sides: {}",
                                    info.custom_sides.join(", ")
                                ))
                                .font(regular_font(11.0))
                                .color(TEXT_MUTED),
                            ),
                        );
                    }
                    if !info.resource_ops.is_empty() {
                        draw_resource_ops(ui, &info.resource_ops);
                    }
                    if !info.declared_overrides.is_empty() {
                        card_text(
                            ui,
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
                    for note in &state.cache_unsupported {
                        card_text(
                            ui,
                            egui::Label::new(
                                egui::RichText::new(format!("Not converted: {note}"))
                                    .font(regular_font(11.0))
                                    .color(TEXT_MUTED),
                            )
                            .wrap(),
                        );
                    }
                    let mut shown_warnings: BTreeSet<&str> = BTreeSet::new();
                    for w in info.warnings.iter().chain(&state.cache_warnings) {
                        if !shown_warnings.insert(w.as_str()) {
                            continue;
                        }
                        card_text(
                            ui,
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
        match action {
            Some(CardControlAction::Toggle) => self.set_content_mod_enabled(&info.id, !enabled),
            Some(CardControlAction::MoveUp) => self.move_mod_up(&info.id),
            Some(CardControlAction::MoveDown) => self.move_mod_down(&info.id),
            None => {}
        }
    }

    pub(crate) fn draw_extension_card(
        &mut self,
        ui: &mut egui::Ui,
        manifest: &ExtensionManifest,
        idx: usize,
        total: usize,
        card_w: f32,
    ) {
        let enabled = manifest.supported && self.extension_profile.is_enabled(&manifest.id);
        let expand_id = ui.make_persistent_id(("ext_expanded", &manifest.id));
        let mut expanded = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(expand_id).unwrap_or(false));
        let mut action = None;
        Frame::new()
            .fill(BG_MAIN)
            .stroke(Stroke::new(
                1.0_f32,
                if enabled { DOT_READY } else { INPUT_BORDER },
            ))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(card_w);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 5.0);

                ui.horizontal(|ui| {
                    let status = (!manifest.supported).then_some(("Unsupported", DOT_MISSING));
                    draw_card_header(
                        ui,
                        card_w,
                        status.map(|(s, _)| s),
                        expand_id,
                        &mut expanded,
                        &manifest.name,
                        Some(&manifest.version),
                        (manifest.origin == ExtensionOrigin::Bundled).then(|| manifest.origin.label()),
                    );

                    action = draw_card_right_controls(
                        ui,
                        &manifest.id,
                        enabled,
                        manifest.supported,
                        idx,
                        total,
                        status,
                    );
                });

                if expanded {
                    card_text(
                        ui,
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
                        card_text(
                            ui,
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
                    card_text(
                        ui,
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
        match action {
            Some(CardControlAction::Toggle) => self.set_extension_enabled(&manifest.id, !enabled),
            Some(CardControlAction::MoveUp) => self.move_mod_up(&manifest.id),
            Some(CardControlAction::MoveDown) => self.move_mod_down(&manifest.id),
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
enum CardControlAction {
    Toggle,
    MoveUp,
    MoveDown,
}

fn scroll_while_dragging(ui: &egui::Ui) {
    if ui.ctx().dragged_id().is_some() && ui.rect_contains_pointer(ui.clip_rect()) {
        let delta = ui.input(|i| i.smooth_scroll_delta);
        ui.scroll_with_delta_animation(delta, egui::style::ScrollAnimation::none());
    }
}

/// A card's selectable text. egui's own label joins the text selection only
/// while it is on screen, and the selection is dropped when either end goes
/// missing, so scrolling would lose it. This one takes part wherever it is.
pub(crate) fn card_text(ui: &mut egui::Ui, label: egui::Label) {
    let (pos, galley, response) = label.layout_in_ui(ui);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), galley.text())
    });
    let color = ui.style().visuals.text_color();
    LabelSelectionState::label_text_selection(ui, &response, pos, galley, color, Stroke::NONE);
}

fn draw_resource_ops(ui: &mut egui::Ui, ops: &[ResourceOp]) {
    let active = ops.iter().filter(|o| o.is_active()).count();
    let identical = ops
        .iter()
        .filter(|o| o.status == OpStatus::Identical)
        .count();
    let ambiguous = ops.iter().filter(|o| o.status.is_ambiguous()).count();
    let mut parts = Vec::new();
    if active > 0 {
        parts.push(format!("{active} active"));
    }
    if ambiguous > 0 {
        parts.push(format!("{ambiguous} unclear (kept stock)"));
    }
    if identical > 0 {
        parts.push(format!("{identical} identical to stock"));
    }
    let other = ops.len().saturating_sub(active + identical + ambiguous);
    if other > 0 {
        parts.push(format!("{other} inactive/unresolved"));
    }
    card_text(
        ui,
        egui::Label::new(
            egui::RichText::new(format!("Stock replacements: {}", parts.join(", ")))
                .font(bold_font(11.5))
                .color(TEXT_PRIMARY),
        ),
    );
    for op in ops {
        if op.status == OpStatus::Identical && !op.selected_by_user {
            continue;
        }
        draw_one_resource_op(ui, op);
    }
}

fn draw_one_resource_op(ui: &mut egui::Ui, op: &ResourceOp) {
    let status_color = match &op.status {
        OpStatus::Active => DOT_READY,
        OpStatus::Ambiguous => DOT_BUILDING,
        OpStatus::Identical | OpStatus::Inactive(_) => TEXT_MUTED,
        OpStatus::Unresolved(_) | OpStatus::Unsupported(_) => DOT_MISSING,
    };
    ui.horizontal_wrapped(|ui| {
        let dest = op
            .target
            .as_ref()
            .map(|t| t.label())
            .unwrap_or_else(|| op.status.label().to_owned());
        card_text(
            ui,
            egui::Label::new(
                egui::RichText::new(format!("  {} -> {dest}", op.source))
                    .font(regular_font(11.0))
                    .color(TEXT_PRIMARY),
            ),
        );
        card_text(
            ui,
            egui::Label::new(
                egui::RichText::new(format!("[{}]", op.status.label()))
                    .font(regular_font(10.5))
                    .color(status_color),
            ),
        );
        if op.is_active() && op.changes_gameplay() {
            extension_badge(ui, "Gameplay", false);
        }
    });
    if let Some(reason) = op.status.reason() {
        card_text(
            ui,
            egui::Label::new(
                egui::RichText::new(format!("    {reason}"))
                    .font(regular_font(10.5))
                    .color(TEXT_MUTED),
            )
            .wrap(),
        );
    }
}

/// The left part of a card's header row: chevron, title, version and an
/// optional badge, clicked to expand or collapse. Width is left for the
/// right-hand controls (#N, up/down arrows, toggle and `status`) so long
/// titles truncate cleanly instead of colliding.
#[allow(clippy::too_many_arguments)]
fn draw_card_header(
    ui: &mut egui::Ui,
    card_w: f32,
    status: Option<&str>,
    expand_id: egui::Id,
    expanded: &mut bool,
    title: &str,
    version: Option<&str>,
    badge: Option<&str>,
) {
    let status_w = status
        .map(|s| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(s.to_owned(), regular_font(12.0), TEXT_MUTED)
                    .size()
                    .x
            }) + 12.0
        })
        .unwrap_or(0.0);
    let left_w = (card_w - (154.0 + status_w)).max(80.0);
    let header_alloc = ui.allocate_ui_with_layout(
        Vec2::new(left_w, 22.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_max_width(left_w);
            ui.horizontal(|ui| {
                draw_expand_chevron(ui, *expanded);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(title)
                            .font(bold_font(14.0))
                            .color(TEXT_PRIMARY),
                    )
                    .truncate()
                    .selectable(false),
                );
                if let Some(ver) = version {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("v{ver}"))
                                .font(regular_font(12.0))
                                .color(TEXT_MUTED),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend)
                        .selectable(false),
                    );
                }
                if let Some(label) = badge {
                    extension_badge(ui, label, false);
                }
            });
        },
    );
    let header_resp = ui.interact(
        header_alloc.response.rect,
        expand_id.with("header"),
        Sense::click(),
    );
    if header_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if header_resp.clicked() {
        *expanded = !*expanded;
    }
}

fn draw_expand_chevron(ui: &mut egui::Ui, expanded: bool) {
    let (chev_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 16.0), Sense::hover());
    let cx = chev_rect.center().x;
    let cy = chev_rect.center().y;
    let chev_stroke = Stroke::new(1.3_f32, Color32::from_rgb(0x55, 0x55, 0x55));
    if expanded {
        ui.painter().line_segment(
            [Pos2::new(cx - 3.5, cy - 1.5), Pos2::new(cx, cy + 2.0)],
            chev_stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(cx, cy + 2.0), Pos2::new(cx + 3.5, cy - 1.5)],
            chev_stroke,
        );
    } else {
        ui.painter().line_segment(
            [Pos2::new(cx - 1.5, cy - 3.5), Pos2::new(cx + 2.0, cy)],
            chev_stroke,
        );
        ui.painter().line_segment(
            [Pos2::new(cx + 2.0, cy), Pos2::new(cx - 1.5, cy + 3.5)],
            chev_stroke,
        );
    }
}

fn draw_card_right_controls(
    ui: &mut egui::Ui,
    id_salt: &str,
    enabled: bool,
    interactive: bool,
    idx: usize,
    total: usize,
    status: Option<(&str, Color32)>,
) -> Option<CardControlAction> {
    let mut action = None;
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if extension_toggle(ui, id_salt, enabled, interactive) {
            action = Some(CardControlAction::Toggle);
        }
        if order_arrow_button(ui, false, idx + 1 < total) {
            action = Some(CardControlAction::MoveDown);
        }
        if order_arrow_button(ui, true, idx > 0) {
            action = Some(CardControlAction::MoveUp);
        }
        // Fixed-width right-aligned order slot so #1..#99 align vertically
        // regardless of proportional digit width.
        ui.allocate_ui_with_layout(
            Vec2::new(28.0, 22.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("#{}", idx + 1))
                            .font(bold_font(12.0))
                            .color(TEXT_MUTED),
                    )
                    .wrap_mode(egui::TextWrapMode::Extend)
                    .selectable(false),
                );
            },
        );
        if let Some((status_text, status_color)) = status {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(status_text)
                        .font(regular_font(12.0))
                        .color(status_color),
                )
                .wrap_mode(egui::TextWrapMode::Extend)
                .selectable(false),
            );
        }
    });
    action
}

fn content_status_badge(
    playable: bool,
    enabled: bool,
    state: &DiscoveredModState,
) -> Option<(String, Color32)> {
    if !playable {
        return Some((state.discovered.compatibility.label().into(), DOT_MISSING));
    }
    if state.inspecting {
        return Some(("Inspecting...".into(), DOT_BUILDING));
    }
    if !enabled {
        return None;
    }
    if !state.inspected {
        return Some(("Inspecting...".into(), DOT_BUILDING));
    }
    match &state.cache_status {
        ModCacheStatus::Ready => None,
        ModCacheStatus::Converting => {
            Some((format!("Converting {}%", state.progress_pct), DOT_BUILDING))
        }
        ModCacheStatus::NeedsConversion => Some(("Pending".into(), DOT_BUILDING)),
        ModCacheStatus::Stale { .. } => Some(("Rebuild".into(), DOT_BUILDING)),
        ModCacheStatus::Failed(_) => Some(("Failed".into(), DOT_MISSING)),
        ModCacheStatus::NotApplicable => Some(("Unsupported".into(), DOT_MISSING)),
    }
}

fn order_arrow_button(ui: &mut egui::Ui, up: bool, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, 22.0), Sense::click());
    let hovered = enabled && resp.hovered();
    let color = if !enabled {
        Color32::from_rgb(0xcc, 0xcc, 0xcc)
    } else if hovered {
        TEXT_PRIMARY
    } else {
        Color32::from_rgb(0x7a, 0x7a, 0x7a)
    };
    let c = rect.center();
    let tip_y = if up { c.y - 2.5 } else { c.y + 2.5 };
    let base_y = if up { c.y + 2.0 } else { c.y - 2.0 };
    let stroke = Stroke::new(1.6_f32, color);
    ui.painter().line_segment(
        [Pos2::new(c.x - 4.0, base_y), Pos2::new(c.x, tip_y)],
        stroke,
    );
    ui.painter().line_segment(
        [Pos2::new(c.x, tip_y), Pos2::new(c.x + 4.0, base_y)],
        stroke,
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
    use extensions::mods::TargetSelections;

    fn collect_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                Rect::from_min_size(t.pos, t.galley.size()),
            )),
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

    fn synthetic_ucfb(tag: &[u8; 4], name: &str, payload: &[u8]) -> Vec<u8> {
        let mut name_body = name.as_bytes().to_vec();
        name_body.push(0);
        let mut inner = Vec::new();
        inner.extend_from_slice(b"NAME");
        inner.extend_from_slice(&(name_body.len() as u32).to_le_bytes());
        inner.extend_from_slice(&name_body);
        while inner.len() % 4 != 0 {
            inner.push(0);
        }
        inner.extend_from_slice(b"BODY");
        inner.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        inner.extend_from_slice(payload);
        while inner.len() % 4 != 0 {
            inner.push(0);
        }
        let mut chunk = Vec::new();
        chunk.extend_from_slice(tag);
        chunk.extend_from_slice(&(inner.len() as u32).to_le_bytes());
        chunk.extend_from_slice(&inner);
        while chunk.len() % 4 != 0 {
            chunk.push(0);
        }
        let mut ucfb = Vec::new();
        ucfb.extend_from_slice(b"ucfb");
        ucfb.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        ucfb.extend_from_slice(&chunk);
        ucfb
    }

    fn write_synthetic_catalog(checkout_root: &std::path::Path, entries: &[(&str, Vec<u8>)]) {
        use extensions::mods::catalog::{
            CatalogLvl, SourceGame, SourcePlatform, StockCatalog, CATALOG_DIR,
            CATALOG_GENERATOR_VERSION, CATALOG_SCHEMA_VERSION,
        };
        let lvls = entries
            .iter()
            .map(|(path, bytes)| CatalogLvl {
                path: (*path).into(),
                facts: mungefs::inventory::scan_bytes(bytes),
                outputs: vec!["classes/bf2_all.ron".into()],
            })
            .collect();
        let cat = StockCatalog {
            schema_version: CATALOG_SCHEMA_VERSION,
            generator_version: CATALOG_GENERATOR_VERSION,
            game: SourceGame::Bf2,
            platform: SourcePlatform::Pc,
            lvls,
        };
        let dir = checkout_root
            .join("engine")
            .join("assets_baked")
            .join(CATALOG_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bf2_pc.ron"), ron::to_string(&cat).unwrap()).unwrap();
    }

    #[test]
    fn auto_discovers_mods_without_import_and_persists_load_order() {
        let dir =
            std::env::temp_dir().join(format!("openbf_launcher_ui_mods_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_synthetic_catalog(
            &dir,
            &[(
                "side/all.lvl",
                synthetic_ucfb(b"tex_", "all_trooper", b"stock"),
            )],
        );
        let mut paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        paths.checkout_root = Some(dir.clone());
        let mods = paths.mods_dir();
        for (name, bytes) in [
            ("AlphaMod", synthetic_ucfb(b"tex_", "all_trooper", b"alpha")),
            ("BetaMod", synthetic_ucfb(b"tex_", "all_trooper", b"beta")),
        ] {
            let side = mods.join(name).join("data/_LVL_PC/SIDE");
            std::fs::create_dir_all(&side).unwrap();
            std::fs::write(side.join("all.lvl"), bytes).unwrap();
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
        let open_rect = rendered
            .iter()
            .find(|(t, _)| t == "Open Mods Folder")
            .expect("Open Mods Folder")
            .1;
        let refresh_rect = rendered
            .iter()
            .find(|(t, _)| t == "Refresh")
            .expect("Refresh")
            .1;
        let bf2_rect = rendered
            .iter()
            .find(|(t, _)| t == "Battlefront II")
            .expect("Battlefront II")
            .1;
        // Footer buttons sit at the bottom-left opposite the bottom-right status indicators.
        assert!(open_rect.center().y > WINDOW_HEIGHT - 80.0);
        assert!(refresh_rect.center().y > WINDOW_HEIGHT - 80.0);
        assert!(open_rect.right() < bf2_rect.left());
        assert!(refresh_rect.right() < bf2_rect.left());

        assert!(!rendered.iter().any(|(t, _)| t.contains("Import Mod")));
        assert!(!rendered.iter().any(|(t, _)| t == "Engine Extensions"));
        assert!(!rendered.iter().any(|(t, _)| t == "Content Mods"));
        assert!(!rendered.iter().any(|(t, _)| t.contains("Higher in list")));
        assert!(!rendered.iter().any(|(t, _)| t == "Ready"));
        assert!(!rendered.iter().any(|(t, _)| t == "Enabled"));
        assert!(!rendered.iter().any(|(t, _)| t == "Disabled"));
        assert!(!rendered.iter().any(|(t, _)| t.contains("Folder: AlphaMod")));

        let alpha_rect = rendered
            .iter()
            .find(|(t, _)| t == "AlphaMod")
            .expect("AlphaMod")
            .1;
        let vr_rect = rendered
            .iter()
            .find(|(t, _)| t == "OpenBF VR")
            .expect("OpenBF VR")
            .1;
        let order1_rect = rendered.iter().find(|(t, _)| t == "#1").expect("#1").1;
        let order3_rect = rendered.iter().find(|(t, _)| t == "#3").expect("#3").1;
        assert!(rendered.iter().any(|(t, _)| t == "#2"));
        assert!(
            order1_rect.left() > alpha_rect.right(),
            "expected #1 right of title: {order1_rect:?} vs {alpha_rect:?}"
        );
        assert!(
            order3_rect.left() > vr_rect.right(),
            "expected #3 right of OpenBF VR title: {order3_rect:?} vs {vr_rect:?}"
        );
        assert!(rendered.iter().any(|(t, _)| t == "BetaMod"));

        // Clicking a content mod header expands its details just like OpenBF VR.
        let click_pos = alpha_rect.center();
        for pressed in [None, Some(true), Some(false)] {
            let mut ev = input.clone();
            ev.events = match pressed {
                None => vec![egui::Event::PointerMoved(click_pos)],
                Some(p) => vec![egui::Event::PointerButton {
                    pos: click_pos,
                    button: egui::PointerButton::Primary,
                    pressed: p,
                    modifiers: egui::Modifiers::NONE,
                }],
            };
            let out_click = ctx.run(ev, |ctx| {
                let _ = app.ui(ctx);
            });
            if pressed == Some(false) {
                let mut expanded_texts = Vec::new();
                for clipped in &out_click.shapes {
                    collect_texts(&clipped.shape, &mut expanded_texts);
                }
                assert!(
                    expanded_texts
                        .iter()
                        .any(|(t, _)| t.contains("Folder: AlphaMod")),
                    "expected expanded metadata for AlphaMod; got {expanded_texts:#?}"
                );
            }
        }

        // Reorder BetaMod and OpenBF VR, toggle BetaMod disabled, verify unified & content order persist.
        app.move_mod_up("content.betamod");
        app.move_mod_up(OPENBF_VR_ID);
        app.set_content_mod_enabled("content.betamod", false);
        assert_eq!(
            app.extension_profile
                .ordered_visible(&app.all_visible_mod_ids(&app.mods_snapshot.folder_ids)),
            vec!["content.betamod", OPENBF_VR_ID, "content.alphamod"]
        );
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
                .ordered_visible(&reloaded.all_visible_mod_ids(&reloaded.mods_snapshot.folder_ids)),
            vec!["content.betamod", OPENBF_VR_ID, "content.alphamod"]
        );
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
        // Step past OpenBF VR (#2) and BetaMod (#1) so AlphaMod sits above its dependency.
        reloaded.move_mod_up("content.alphamod");
        reloaded.move_mod_up("content.alphamod");
        // Cache is still pending conversion (or failed without bake), so fix dependency error is cleared:
        let plan = resolve_content_mods(
            &reloaded
                .mods_snapshot
                .mods
                .values()
                .map(|s| s.discovered.clone())
                .collect::<Vec<_>>(),
            &reloaded.extension_profile,
        )
        .expect("resolved once dependency order is fixed");
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].resource_path, "bf2 pc:side/all.lvl");
        assert_eq!(plan.conflicts[0].winner_id, "content.alphamod");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_wheel_scrolls_the_list_while_text_is_being_selected() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_drag_scroll_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        write_synthetic_catalog(
            &dir,
            &[(
                "side/all.lvl",
                synthetic_ucfb(b"tex_", "all_trooper", b"stock"),
            )],
        );
        let mut paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        paths.checkout_root = Some(dir.clone());
        for i in 0..24 {
            let side = paths
                .mods_dir()
                .join(format!("Mod{i:02}"))
                .join("data/_LVL_PC/SIDE");
            std::fs::create_dir_all(&side).unwrap();
            std::fs::write(
                side.join("all.lvl"),
                synthetic_ucfb(b"tex_", "all_trooper", b"modded"),
            )
            .unwrap();
        }

        let mut app = LauncherApp::new(paths, || {});
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

        let mut time = 0.0;
        let mut frame = |app: &mut LauncherApp, events: Vec<egui::Event>| {
            time += 1.0 / 60.0;
            let mut raw = input.clone();
            raw.time = Some(time);
            raw.events = events;
            let out = ctx.run(raw, |ctx| {
                let _ = app.ui(ctx);
            });
            let mut texts = Vec::new();
            for clipped in &out.shapes {
                collect_texts(&clipped.shape, &mut texts);
            }
            texts
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let find = |texts: &[(String, Rect)], needle: &str| {
            texts
                .iter()
                .find(|(t, _)| t.contains(needle))
                .map(|(_, r)| *r)
        };

        let title = find(&frame(&mut app, Vec::new()), "Mod00")
            .expect("Mod00")
            .center();
        frame(&mut app, vec![egui::Event::PointerMoved(title)]);
        frame(&mut app, vec![button(title, true)]);
        frame(&mut app, vec![button(title, false)]);
        let before = find(&frame(&mut app, Vec::new()), "Folder: Mod00").expect("expanded card");

        let start = before.left_center() + Vec2::new(4.0, 0.0);
        let end = start + Vec2::new(60.0, 0.0);
        frame(&mut app, vec![egui::Event::PointerMoved(start)]);
        frame(&mut app, vec![button(start, true)]);
        frame(&mut app, vec![egui::Event::PointerMoved(end)]);
        assert!(ctx.dragged_id().is_some(), "selecting text is a drag");
        frame(
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -40.0),
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let mut texts = Vec::new();
        for _ in 0..30 {
            texts = frame(&mut app, Vec::new());
        }
        let after = find(&texts, "Folder: Mod00").expect("still on screen");
        assert!(
            (before.top() - after.top() - 40.0).abs() < 1.0,
            "{before:?} -> {after:?}"
        );

        // Scroll the line the selection began on out of view, then finish the
        // drag on text further down and copy.
        frame(
            &mut app,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -400.0),
                modifiers: egui::Modifiers::NONE,
            }],
        );
        for _ in 0..30 {
            texts = frame(&mut app, Vec::new());
        }
        let gone = find(&texts, "Folder: Mod00").map_or(f32::MIN, |r| r.bottom());
        assert!(gone < end.y - 100.0, "the first line is still in view");
        let selection = ctx.plugin::<egui::text_selection::LabelSelectionState>();
        assert!(selection.lock().has_selection(), "dropped while scrolling");
        frame(&mut app, vec![button(end, false)]);
        let mut raw = input.clone();
        raw.time = Some(100.0);
        raw.events = vec![egui::Event::Copy];
        let out = ctx.run(raw, |ctx| {
            let _ = app.ui(ctx);
        });
        let copied: String = out
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(copied.contains("Namespace: mod_mod00"), "{copied:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unclear_target_stays_stock_and_offers_no_picker() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_unclear_target_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let stock_xwing = synthetic_ucfb(b"tex_", "xwing_cockpit", b"stock");
        write_synthetic_catalog(
            &dir,
            &[
                ("fpm/imp/allxwing.lvl", stock_xwing.clone()),
                ("fpm/rep/allxwing.lvl", stock_xwing),
            ],
        );
        let mut paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        paths.checkout_root = Some(dir.clone());
        let loose_mod = paths.mods_dir().join("LooseXWing");
        std::fs::create_dir_all(&loose_mod).unwrap();
        std::fs::write(
            loose_mod.join("allxwing.lvl"),
            synthetic_ucfb(b"tex_", "xwing_cockpit", b"modded"),
        )
        .unwrap();

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

        let found = &app.mods_snapshot.mods["content.loosexwing"].discovered;
        assert_eq!(found.ambiguous_ops().count(), 1);
        assert_eq!(found.active_ops().count(), 0);

        let out = ctx.run(input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut rendered = Vec::new();
        for clipped in &out.shapes {
            collect_texts(&clipped.shape, &mut rendered);
        }
        assert!(rendered.iter().any(|(t, _)| t == "LooseXWing"));
        assert!(!rendered.iter().any(|(t, _)| t.contains("Choose target")));
        assert!(!TargetSelections::path(&paths.data_dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collapsed_headers_only_show_bundled_tag_and_expanded_badges_wrap_cleanly() {
        use extensions::mods::replace::Capability;
        use extensions::mods::{
            compute_mod_fingerprint, list_mod_folders, mod_cache_path, ModCacheMeta,
            MOD_CACHE_SCHEMA_VERSION, MOD_CONVERTER_ID,
        };

        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_badge_layout_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let stock_all = synthetic_ucfb(b"tex_", "all_inf", b"stock");
        write_synthetic_catalog(&dir, &[("side/all.lvl", stock_all)]);
        let mut paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        paths.checkout_root = Some(dir.clone());
        let long_name = "SWBF2 Improved Sides mod 1.0";
        let mod_dir = paths.data_dir.join("mods").join(long_name);
        std::fs::create_dir_all(mod_dir.join("side")).unwrap();
        std::fs::write(mod_dir.join("side").join("all.lvl"), b"placeholder").unwrap();

        let entry = list_mod_folders(&paths.data_dir.join("mods"))
            .unwrap()
            .remove(0);
        let mod_id = entry.mod_id;
        let namespace = entry.namespace;
        let fp = compute_mod_fingerprint(&mod_dir).unwrap();
        let cache_dir = mod_cache_path(&paths.data_dir, &namespace);
        std::fs::create_dir_all(&cache_dir).unwrap();
        let catalog_digest = extensions::mods::catalog::StockCatalogs::load_dir(
            &dir.join("engine").join("assets_baked"),
        )
        .digest;
        let mut meta = ModCacheMeta {
            schema_version: MOD_CACHE_SCHEMA_VERSION,
            converter: MOD_CONVERTER_ID.into(),
            mod_id: mod_id.clone(),
            namespace: namespace.clone(),
            source_fingerprint: fp.digest,
            display_name: long_name.into(),
            version: None,
            author: None,
            description: "Test long mod card.".into(),
            mod_type: extensions::mods::ModContentType::LegacySideOverhaul,
            legacy_addon_id: Some(long_name.into()),
            maps: Vec::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            declared_overrides: Vec::new(),
            has_locl: false,
            resource_ops: vec![extensions::mods::ResourceOp {
                source: "side/all.lvl".into(),
                source_sha256: "abc".into(),
                source_size: 11,
                detected_game: Some(extensions::mods::catalog::SourceGame::Bf2),
                detected_platform: Some(extensions::mods::catalog::SourcePlatform::Pc),
                target: Some(extensions::mods::catalog::TargetRef {
                    game: extensions::mods::catalog::SourceGame::Bf2,
                    platform: extensions::mods::catalog::SourcePlatform::Pc,
                    path: "side/all.lvl".into(),
                }),
                mode: extensions::mods::replace::ReplacementMode::Full,
                status: OpStatus::Active,
                candidates: Vec::new(),
                evidence: Vec::new(),
                changes: Vec::new(),
                gameplay: vec![extensions::mods::replace::GameplayChange {
                    resource: "entc test".into(),
                    detail: "1 changed".into(),
                }],
                capabilities: vec![
                    Capability::SideReplacement,
                    Capability::ShellSideReplacement,
                    Capability::FirstPerson,
                    Capability::Textures,
                    Capability::Models,
                    Capability::Animation,
                    Capability::GameplayClasses,
                ],
                selected_by_user: false,
            }],
            replacement_key: "cached-key".into(),
            replacement_outputs: Vec::new(),
            catalog_digest: catalog_digest.clone(),
            target_choices: std::collections::BTreeMap::new(),
            warnings: Vec::new(),
            unsupported: Vec::new(),
            custom_modes: std::collections::BTreeMap::new(),
            has_shell_icons: false,
        };
        meta.replacement_key =
            meta.expected_replacement_key(&catalog_digest, &std::collections::BTreeMap::new());
        meta.save_atomic(&cache_dir).unwrap();

        let mut app = LauncherApp::new(paths, || {});
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
        app.extension_profile.order = vec![OPENBF_VR_ID.into(), mod_id.clone()];
        app.extension_profile.set_enabled(&mod_id, true);

        // 1. Collapsed view: only "Bundled" badge appears; title never overlaps #2.
        let out = ctx.run(input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut rendered = Vec::new();
        for clipped in &out.shapes {
            collect_texts(&clipped.shape, &mut rendered);
        }
        assert!(rendered.iter().any(|(t, _)| t == "Bundled"));
        assert!(!rendered.iter().any(|(t, _)| t == "Gameplay"));
        assert!(!rendered.iter().any(|(t, _)| t == "Side Overhaul"));
        let title_rect = rendered
            .iter()
            .find(|(t, _)| t.starts_with("SWBF2 Improved Sides"))
            .expect("title")
            .1;
        let order2_rect = rendered.iter().find(|(t, _)| t == "#2").expect("#2").1;
        assert!(
            title_rect.right() < order2_rect.left(),
            "collapsed title overlapped #2: {title_rect:?} vs {order2_rect:?}"
        );

        // 2. Click header to expand: all capability badges render on single lines (no vertical squish).
        let click_pos = title_rect.center();
        for pressed in [None, Some(true), Some(false)] {
            let mut ev = input.clone();
            ev.events = match pressed {
                None => vec![egui::Event::PointerMoved(click_pos)],
                Some(p) => vec![egui::Event::PointerButton {
                    pos: click_pos,
                    button: egui::PointerButton::Primary,
                    pressed: p,
                    modifiers: egui::Modifiers::NONE,
                }],
            };
            let _ = ctx.run(ev, |ctx| {
                let _ = app.ui(ctx);
            });
        }
        let out_expanded = ctx.run(input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut expanded = Vec::new();
        for clipped in &out_expanded.shapes {
            collect_texts(&clipped.shape, &mut expanded);
        }
        for label in [
            "Side Overhaul",
            "Gameplay",
            "Side replacement",
            "Menu side replacement",
            "First person",
            "Textures",
            "Models",
            "Animation",
            "Gameplay classes",
        ] {
            let rect = expanded
                .iter()
                .find(|(t, _)| t == label)
                .unwrap_or_else(|| {
                    panic!(
                        "missing expanded badge `{label}` (click={click_pos:?}, title={title_rect:?}, got={:?})",
                        expanded.iter().map(|(t, r)| (t.as_str(), *r)).collect::<Vec<_>>()
                    )
                })
                .1;
            assert!(
                rect.height() < 24.0,
                "badge `{label}` wrapped vertically: {rect:?}"
            );
            assert!(
                rect.right() < WINDOW_WIDTH,
                "badge `{label}` overflowed window: {rect:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loading_state_hides_empty_message_and_cards_keep_fixed_width_alignment() {
        let dir = std::env::temp_dir().join(format!(
            "openbf_launcher_align_and_load_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let paths =
            LauncherPaths::from_exe(&dir.join(format!("openbf-launcher{EXE_SUFFIX}")), None);
        for name in ["ModAlpha", "ModBeta", "ModGamma", "ModDelta"] {
            let side = paths.data_dir.join("mods").join(name).join("side");
            std::fs::create_dir_all(&side).unwrap();
            let f = std::fs::File::create(side.join("all.lvl")).unwrap();
            // > 8 MiB sparse file so listing is instant & metadata-only
            f.set_len(12 * 1024 * 1024).unwrap();
        }
        let mut app = LauncherApp::new(paths, || {});
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

        // 1. Before initial scan completes, show "Loading mods..." instead of "No legacy mods found".
        app.mods_snapshot.scanning = true;
        app.mods_snapshot.generation = 0;
        let loading_out = ctx.run(input.clone(), |ctx| {
            // Render draw_mods_page directly without polling the worker so scanning=true is tested deterministically.
            egui::CentralPanel::default().show(ctx, |ui| {
                app.draw_mods_page(ui);
            });
        });
        let mut loading_texts = Vec::new();
        for clipped in &loading_out.shapes {
            collect_texts(&clipped.shape, &mut loading_texts);
        }
        assert!(
            loading_texts.iter().any(|(t, _)| t == "Loading mods..."),
            "expected Loading mods... notice; got {loading_texts:?}"
        );
        assert!(
            !loading_texts
                .iter()
                .any(|(t, _)| t.contains("No legacy mods found")),
            "must not show No legacy mods found while scanning"
        );

        // 2. Once scanned, all 5 cards (#1..#5) must have identical right-edge alignment (zero +2px drift).
        wait_for_scan(&mut app, &ctx, &input);
        let out = ctx.run(input.clone(), |ctx| {
            let _ = app.ui(ctx);
        });
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            collect_texts(&clipped.shape, &mut texts);
        }
        let order_rights: Vec<f32> = ["#1", "#2", "#3", "#4", "#5"]
            .iter()
            .map(|label| {
                texts
                    .iter()
                    .find(|(t, _)| t == label)
                    .unwrap_or_else(|| panic!("missing order label {label}"))
                    .1
                    .right()
            })
            .collect();
        let min_r = order_rights.iter().copied().fold(f32::INFINITY, f32::min);
        let max_r = order_rights
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (max_r - min_r) < 0.5,
            "order labels drifted horizontally across cards: {order_rights:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
