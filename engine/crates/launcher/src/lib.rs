//! Standalone native launcher and setup utility for OpenBF.
//!
//! Kept completely separate from `game` and `core`: players can still run
//! `openbf` directly without ever opening the launcher, and the game runtime
//! never links build or disc-reading code. Configuration is stored beside the
//! binaries in `openbf_data/launcher.ron`, matching the game's portable data
//! directory convention.

pub mod config;
pub mod paths;
pub mod pipeline;
pub mod runner;
pub mod ui;
