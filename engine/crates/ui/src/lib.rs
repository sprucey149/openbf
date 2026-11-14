//! In-game UI. The HUD is BF2's own, interpreted from its baked tree
//! ([`hud_tree`] the elements and their events, [`hud_draw`] their layout into
//! triangles for `render::hud`; [`hud_map`], [`hud_squad`] and
//! [`hud_capture`] the minimap, the squad-order cross and the capture
//! display, [`hud_ambush`] the hit-direction chevrons, drawn from code as
//! the PS2 draws them), fed by a
//! [`hud::HudModel`]; the spawn screens are drawn into the same frame as the
//! PS2's shell draws them ([`spawn_draw`]), and so are the BF2 shell's
//! title and stack menus ([`shell_ps2`], their clicks, focus pulse and
//! entrance in [`shell_feel`]) and its Instant Action screens ([`shell`],
//! [`shell_ia`]). The in-game menus share those assets ([`pause_draw`]).
//! Fallback panels and the HUD lines BF2's tree does not carry yet use egui.
//! Both halves are pure functions of the model so they can be tested headless,
//! independently of the renderer; [`overlay`] is the egui-wgpu / egui-winit
//! glue that paints egui into the game's frame through
//! `render::Renderer::render_with_overlay`.
//!
//! Layout rule (PLAN §Presentation, widescreen-native): every element is
//! anchored to an edge/corner in logical points — nothing is authored for a
//! fixed aspect or stretched.

pub use egui;

pub mod cursor;
pub mod hud;
pub mod hud_ambush;
pub mod hud_capture;
pub mod hud_draw;
pub mod hud_flag;
pub mod hud_map;
pub mod hud_scope;
pub mod hud_space;
pub mod hud_squad;
pub mod hud_tree;
pub mod input_icons;
pub mod loading;
pub mod lobby;
pub mod menu;
pub mod overlay;
pub mod pause_draw;
pub mod scoreboard;
pub mod shell;
pub mod shell_feel;
pub mod shell_ia;
pub mod shell_ps2;
pub mod spawn;
pub mod spawn_draw;
pub mod stats;
pub mod stats_draw;

pub mod match_length;
pub mod population;
