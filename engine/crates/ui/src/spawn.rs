//! Spawn screen: what a player off the field looks at - dead, or behind the
//! spawn screens (card SSELA: side, spawn map, class; [`SpawnScreen`]). The
//! PS2's own side, map and class screens are `crate::spawn_draw`'s (card
//! SSELB), painted by the HUD pass; this egui panel is the battle without
//! spawn screens and the fallback where no HUD pass draws.
//!
//! Pure presentation — `SpawnModel` is filled by the game layer from the sim's
//! command posts and `Sim.waves[team]`.
//!
//! Both selections are SIM state, moved by pad bits the input log carries,
//! so they reproduce under replay like any other:
//! - the CLASS row: `selected_class` is `Sim::spawn_choice[slot]`
//!   (`buttons::SPAWN_CLASS_NEXT`/`PREV`), the class the respawn is built from;
//! - the POST: `selected` is the slot's spawn-map pick through keep-or-nearest
//!   (`Sim::spawn_post_selected`, card SPSEL; `buttons::SPAWN_POST_*` push it
//!   by direction on the map, the PS2's rule), the post the respawn uses.
//!   `None` until the player first pushes: the respawn then draws from every
//!   post's nodes.

use crate::input_icons::Prompt;
use egui::{Align2, Color32, FontId, Frame, RichText, Vec2};

use crate::hud::TEAM_COLORS;

#[derive(Debug, Clone, Default)]
pub struct SpawnPost {
    pub name: String,
    /// Enemies are neutralizing this post right now.
    pub contested: bool,
    /// Metres from where the player died (ordering aid, not a game rule).
    pub distance: f32,
}

/// What every spawn screen leaves of the backdrop's brightness: 0.51 behind
/// the respawn map on Hoth (the snow's mean colour 194 -> 98, within 0.3 s;
/// `analysis/re-spawn-flow.md` §6 item 3 [L]). The level start's screens read
/// the same (the snow at about 110 under the side, map and class screens of
/// `lead_tmp/ssel/t1_hot1`, [D]); the class line-up stands undimmed in front
/// (card SSELB slice 2).
pub const BACKDROP_DIM: f32 = 0.51;

/// The black veil's alpha that leaves [`BACKDROP_DIM`] of a displayed value:
/// the overlay blends in linear light (an sRGB target), so the factor is
/// taken through the sRGB curve at the PS2's own measured level, 194 -> 98.
fn backdrop_veil_alpha() -> u8 {
    fn lin(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let c = 194.0 / 255.0;
    let keep = lin(c * BACKDROP_DIM) / lin(c);
    ((1.0 - keep) * 255.0).round() as u8
}

/// Which of the sim's spawn screens is up (card SSELA, `Sim::spawn_stage`),
/// when the battle runs them. Shared state for the HUD-pass screens and
/// the egui fallback panel.
#[derive(Debug, Clone)]
pub enum SpawnScreen {
    /// The side select: team 1, team 2, Auto Assign, and the highlighted row.
    Side { rows: [String; 3], selected: usize },
    /// The spawn map: the post list below is the pick; X takes it.
    Map,
    /// The class select: the class row; X spawns, Triangle goes back.
    Class,
    /// Dead, the respawn timer still running.
    Waiting,
}

#[derive(Debug, Clone, Default)]
pub struct SpawnModel {
    pub team_names: [String; 2],
    pub player_team: u8,
    pub tickets: [u32; 2],
    /// Posts this team can spawn at, in map order.
    pub posts: Vec<SpawnPost>,
    /// The sim's pick as an index into `posts` (clamped when drawing);
    /// `None` = no pick yet, any post.
    pub selected: Option<usize>,
    /// The player's soldier classes, in the team's table order — display
    /// labels from `game::walk::TeamTemplate::classes`. Empty = the team has
    /// no class table and no row is drawn.
    pub classes: Vec<String>,
    /// `Sim::spawn_choice[slot]` — the class the next respawn takes (clamped
    /// when drawing).
    pub selected_class: usize,
    /// Seconds until this player's spawn slot opens; None = slot is open.
    pub countdown: Option<f32>,
    /// The spawn screen up (card SSELA); `None` = the battle runs no spawn
    /// screens (the respawn is automatic).
    pub screen: Option<SpawnScreen>,
    /// The PS2's own screen for `screen`, drawn by the HUD pass
    /// (`crate::spawn_draw`, card SSELB); while it is `Some` this panel
    /// draws nothing. The game clears it where no HUD pass draws.
    pub page: Option<crate::spawn_draw::SpawnPage>,
    /// The prompts' names on this panel (`crate::input_icons`).
    pub icons: crate::input_icons::Icons,
}

/// The side select (card SSELA placeholder): the three rows, the highlighted
/// one marked.
fn draw_side(ctx: &egui::Context, rows: &[String; 3], selected: usize) {
    egui::Area::new(egui::Id::new("spawn_side"))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(215))
                .inner_margin(egui::Margin::symmetric(28, 20))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("SELECT A TEAM")
                                .font(FontId::proportional(26.0))
                                .color(Color32::WHITE)
                                .strong(),
                        );
                        ui.add_space(10.0);
                        for (i, r) in rows.iter().enumerate() {
                            let color = match i {
                                0 | 1 if i == selected => TEAM_COLORS[i],
                                _ if i == selected => Color32::WHITE,
                                _ => Color32::from_gray(150),
                            };
                            let mark = if i == selected { "▶ " } else { "   " };
                            ui.label(
                                RichText::new(format!("{mark}{}", r.to_uppercase()))
                                    .size(19.0)
                                    .color(color)
                                    .strong(),
                            );
                        }
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("D-pad up / down  ·  X select")
                                .size(12.0)
                                .color(Color32::GRAY),
                        );
                    });
                });
        });
}

/// Draw the spawn screen over the frame. Call instead of the normal death
/// banner while the player is dead and the round is still running.
pub fn draw_spawn(ctx: &egui::Context, m: &SpawnModel) {
    if m.page.is_some() {
        return;
    }
    let team = usize::from(m.player_team.min(1));
    let title = match &m.screen {
        Some(SpawnScreen::Class) => "SELECT CLASS",
        _ => "SELECT SPAWN POINT",
    };
    let accent = TEAM_COLORS[team];

    // Dim the backdrop behind the screen to the PS2's 0.51.
    egui::Area::new(egui::Id::new("spawn_dim"))
        .anchor(Align2::LEFT_TOP, Vec2::ZERO)
        .show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.painter()
                .rect_filled(r, 0.0, Color32::from_black_alpha(backdrop_veil_alpha()));
            ui.allocate_space(r.size());
        });
    if let Some(SpawnScreen::Side { rows, selected }) = &m.screen {
        draw_side(ctx, rows, *selected);
        return;
    }

    egui::Area::new(egui::Id::new("spawn_panel"))
        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(215))
                .inner_margin(egui::Margin::symmetric(28, 20))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(title)
                                .font(FontId::proportional(26.0))
                                .color(accent)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!(
                                "{}  ·  {} reinforcements",
                                m.team_names[team].to_uppercase(),
                                m.tickets[team]
                            ))
                            .size(13.0)
                            .color(Color32::LIGHT_GRAY),
                        );
                    });
                    ui.add_space(12.0);

                    // The class row belongs to class select or the combined legacy panel.
                    if !m.classes.is_empty() && matches!(m.screen, None | Some(SpawnScreen::Class))
                    {
                        let pick = m.selected_class.min(m.classes.len() - 1);
                        ui.vertical_centered(|ui| {
                            ui.horizontal(|ui| {
                                for (i, c) in m.classes.iter().enumerate() {
                                    let picked = i == pick;
                                    ui.label(
                                        RichText::new(c.to_uppercase())
                                            .size(if picked { 16.0 } else { 14.0 })
                                            .color(if picked {
                                                accent
                                            } else {
                                                Color32::from_gray(150)
                                            })
                                            .strong(),
                                    );
                                    if i + 1 < m.classes.len() {
                                        ui.label(
                                            RichText::new("·").size(14.0).color(Color32::DARK_GRAY),
                                        );
                                    }
                                }
                            });
                            ui.label(
                                RichText::new(if m.screen.is_some() {
                                    format!(
                                        "On the class screen: {} left / right or {}",
                                        m.icons.text(Prompt::DPad),
                                        m.icons.class_keys()
                                    )
                                } else {
                                    format!("{} to change class", m.icons.class_keys())
                                })
                                .size(12.0)
                                .color(Color32::GRAY),
                            );
                        });
                        ui.add_space(10.0);
                    }

                    if m.posts.is_empty() {
                        ui.label(
                            RichText::new("no friendly command post — spawning at the fallback")
                                .size(15.0)
                                .color(Color32::from_rgb(230, 180, 80)),
                        );
                    }
                    let sel = m.selected.map(|s| s.min(m.posts.len().saturating_sub(1)));
                    if sel.is_none() && !m.posts.is_empty() {
                        ui.label(
                            RichText::new("ANY POST — push the D-pad toward one to pick it")
                                .size(13.0)
                                .color(Color32::from_gray(170)),
                        );
                    }
                    for (i, p) in m.posts.iter().enumerate() {
                        let picked = Some(i) == sel;
                        let (mark, name_color) = if picked {
                            ("▶", accent)
                        } else {
                            ("  ", Color32::from_gray(210))
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(mark).size(16.0).color(accent));
                            ui.label(
                                RichText::new(p.name.to_uppercase())
                                    .size(17.0)
                                    .color(name_color)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!("{:.0} m", p.distance))
                                    .size(13.0)
                                    .color(Color32::GRAY),
                            );
                            if p.contested {
                                ui.label(
                                    RichText::new("UNDER ATTACK")
                                        .size(13.0)
                                        .color(Color32::from_rgb(240, 90, 60))
                                        .strong(),
                                );
                            }
                        });
                    }

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);
                    let prompt = match &m.screen {
                        Some(SpawnScreen::Map) => Some(format!(
                            "{}  SELECT COMMAND POST",
                            m.icons.symbol(Prompt::SpawnAccept)
                        )),
                        Some(SpawnScreen::Class) => Some(format!(
                            "{}  SELECT CHARACTER    {}  BACK",
                            m.icons.symbol(Prompt::SpawnAccept),
                            m.icons.symbol(Prompt::SpawnBack)
                        )),
                        _ => None,
                    };
                    ui.vertical_centered(|ui| match (prompt, m.countdown) {
                        (Some(p), _) => ui.label(
                            RichText::new(p)
                                .font(FontId::proportional(22.0))
                                .color(accent)
                                .strong(),
                        ),
                        (None, Some(s)) => ui.label(
                            RichText::new(format!("SPAWNING IN {}", s.ceil().max(1.0) as u32))
                                .font(FontId::proportional(22.0))
                                .color(Color32::WHITE)
                                .strong(),
                        ),
                        (None, None) => ui.label(
                            RichText::new("SPAWNING…")
                                .font(FontId::proportional(22.0))
                                .color(accent)
                                .strong(),
                        ),
                    });
                    ui.add_space(4.0);
                    ui.vertical_centered(|ui| {
                        // The pick goes by direction on the map (the PS2's
                        // spawn map), not down the list: up = north (-z).
                        ui.label(
                            RichText::new(format!(
                                "{}: pick the post that way on the map",
                                m.icons.text(Prompt::DPad)
                            ))
                            .size(12.0)
                            .color(Color32::GRAY),
                        );
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> SpawnModel {
        SpawnModel {
            team_names: ["all".into(), "imp".into()],
            player_team: 0,
            tickets: [147, 152],
            posts: vec![
                SpawnPost {
                    name: "cp1".into(),
                    contested: false,
                    distance: 42.0,
                },
                SpawnPost {
                    name: "cp3".into(),
                    contested: true,
                    distance: 118.0,
                },
            ],
            selected: Some(1),
            classes: vec![
                "Trooper".into(),
                "Assault Gunner".into(),
                "Pilot".into(),
                "Rebel Scout".into(),
                "Repair".into(),
            ],
            selected_class: 2,
            countdown: Some(3.4),
            screen: None,
            page: None,
            icons: Default::default(),
        }
    }

    /// The veil leaves the PS2's 0.51 of the snow's displayed level (194 ->
    /// 98, re-spawn-flow §6 item 3) once the linear-light blend is done.
    #[test]
    fn the_veil_dims_the_measured_level_to_0_51() {
        let to_lin = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let to_srgb = |l: f32| {
            if l <= 0.0031308 {
                l * 12.92
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            }
        };
        let keep = 1.0 - f32::from(backdrop_veil_alpha()) / 255.0;
        let shown = to_srgb(to_lin(194.0 / 255.0) * keep) * 255.0;
        assert!((shown - 98.0).abs() <= 1.5, "194 -> {shown}");
    }

    /// Lays out headless at two aspects, in every state the game can hand it.
    #[test]
    fn spawn_screen_lays_out_headless() {
        for (w, h) in [(1280.0, 720.0), (2560.0, 1080.0)] {
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w, h))),
                ..Default::default()
            };
            // Anchored areas size on frame 1 and lay out on frame 2.
            let _ = ctx.run(input.clone(), |ctx| draw_spawn(ctx, &model()));
            let out = ctx.run(input, |ctx| draw_spawn(ctx, &model()));
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            let indices: usize = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.indices.len(),
                    _ => 0,
                })
                .sum();
            assert!(indices > 300, "{w}x{h}: {indices} indices");
        }
    }

    #[test]
    fn degenerate_models_do_not_panic() {
        let ctx = egui::Context::default();
        // No posts, no countdown, selection past the end.
        let m = SpawnModel {
            posts: Vec::new(),
            selected: Some(7),
            countdown: None,
            ..model()
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw_spawn(ctx, &m));
        // Selection past the end with posts present clamps to the last.
        let m = SpawnModel {
            selected: Some(99),
            ..model()
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw_spawn(ctx, &m));
        // No pick yet.
        let m = SpawnModel {
            selected: None,
            ..model()
        };
        let _ = ctx.run(egui::RawInput::default(), |ctx| draw_spawn(ctx, &m));
    }

    /// Card SSELA: every spawn screen the sim can be on lays out, and the
    /// side select (its own panel) draws something.
    #[test]
    fn every_spawn_screen_lays_out() {
        let rows = || ["all".to_owned(), "imp".to_owned(), "Auto Assign".to_owned()];
        for screen in [
            SpawnScreen::Side {
                rows: rows(),
                selected: 2,
            },
            SpawnScreen::Side {
                rows: rows(),
                selected: 9,
            },
            SpawnScreen::Map,
            SpawnScreen::Class,
            SpawnScreen::Waiting,
        ] {
            let ctx = egui::Context::default();
            let m = SpawnModel {
                screen: Some(screen.clone()),
                ..model()
            };
            let _ = ctx.run(egui::RawInput::default(), |ctx| draw_spawn(ctx, &m));
            let out = ctx.run(egui::RawInput::default(), |ctx| draw_spawn(ctx, &m));
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            assert!(!prims.is_empty(), "{screen:?} drew nothing");
        }
    }

    #[test]
    fn the_post_screen_does_not_draw_the_class_row() {
        let draws_class = |screen| {
            let ctx = egui::Context::default();
            let model = SpawnModel {
                screen: Some(screen),
                ..model()
            };
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280.0, 720.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input.clone(), |ctx| draw_spawn(ctx, &model));
            ctx.run(input, |ctx| draw_spawn(ctx, &model))
                .shapes
                .iter()
                .any(|clipped| {
                    matches!(&clipped.shape, egui::Shape::Text(text) if text.galley.text() == "ASSAULT GUNNER")
                })
        };
        assert!(draws_class(SpawnScreen::Class));
        assert!(!draws_class(SpawnScreen::Map));
    }

    /// The class row draws for every table size the game can hand it — none
    /// (a team with no class table: no row at all), one, six, and a choice
    /// past the end (it clamps).
    #[test]
    fn the_class_row_renders_at_every_table_size() {
        let count = |m: &SpawnModel| {
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280.0, 720.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input.clone(), |ctx| draw_spawn(ctx, m));
            let out = ctx.run(input, |ctx| draw_spawn(ctx, m));
            ctx.tessellate(out.shapes, out.pixels_per_point)
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.indices.len(),
                    _ => 0,
                })
                .sum::<usize>()
        };
        let none = count(&SpawnModel {
            classes: Vec::new(),
            selected_class: 0,
            ..model()
        });
        assert!(none > 300, "{none} indices with no class row");
        let six = count(&SpawnModel {
            classes: (0..6).map(|i| format!("class {i}")).collect(),
            selected_class: 5,
            ..model()
        });
        assert!(six > none, "the class row draws something: {none} -> {six}");
        // One class, and a choice past the end of the list.
        let one = count(&SpawnModel {
            classes: vec!["Trooper".into()],
            selected_class: 9,
            ..model()
        });
        assert!(one > none);
    }
}
