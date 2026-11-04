//! The bake crate's integration tests, linked as ONE test binary (card WEARY, SSD wear): every file directly under
//! `tests/` is its own binary that links the crate and its dependencies, so the files live here as modules instead.
//! A test's id moves from `<file> <name>` to `suite <file>::<name>`. A new test file goes in this directory with a
//! `mod` line below.

mod anim_sets;
mod bake_hot1_layers;
mod bake_tat1;
mod bake_tat2;
mod bake_vehicles;
mod beep;
mod combo;
mod hud_alpha;
mod side_steps;
mod sky_glow_alpha;
mod sound_repairs;
mod water_patch_cells;
mod water_sheet_alpha;
