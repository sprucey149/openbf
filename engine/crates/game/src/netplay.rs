//! LAN multiplayer, the game side (issue #168, `analysis/plan/LAN_MULTIPLAYER.md`
//! §4, §5, §9): what the host sends to build a round, the in-round game
//! commands, the deterministic game-layer writes around each lockstep tick,
//! and [`Peer`], the per-frame driver the windowed loop and the headless
//! soaks share. The `net` crate carries everything here as opaque bytes.
//!
//! Layout (each file's header holds its rules):
//! - [`dto`]: [`RoundSetup`], [`PlayerProfile`], [`RoundOptions`],
//!   [`EnvSwitches`].
//! - [`content`]: [`ContentManifest`] and the host's intersection, the build
//!   identity ([`build_id`]).
//! - [`command`]: [`GameCommand`], [`GameRequest`] and the host's checks.
//! - [`apply`]: [`install_humans`], [`apply`], [`gate`], [`score`], [`step`]
//!   and the round's per-slot state ([`RoundState`]).
//! - [`desync`]: the desync reports' rows, bundle log, file and
//!   comparison ([`DesyncReport`]).
//! - [`peer`], [`link`]: [`Peer`], [`Link`] (bare cores in virtual time or
//!   `NetSession`s), [`FrameReport`].
//! - [`meter`]: [`NetMeter`], the net meter's figures (the overlay and the
//!   black box).
//! - [`lobby`]: [`LobbyState`] (the host's setup screen as the guests draw
//!   it, maps by key), [`HostInfo`], [`GameNotice`], the content
//!   restriction and the refusal texts.
//! - [`predict`], [`view`]: local prediction ([`Predictor`]) and the
//!   per-frame step and drawn view ([`LocalView`]) of a windowed peer.
//! - [`virtual_net`]: several peers in one process on `net::LinkSim`, in
//!   virtual time (the `--net-soak` harness and the tests), and the
//!   scripted pads.
//!
//! Module rules:
//! - Every write into the sim comes from the round setup, a bundle or
//!   sim-derived state, on every peer alike; nothing reads a clock, the
//!   local profile or the environment once the round is built.
//! - Single player does not use this module and keeps its behaviour.

pub mod apply;
pub mod command;
pub mod content;
pub mod desync;
pub mod dto;
pub mod link;
pub mod lobby;
pub mod meter;
pub mod peer;
pub mod predict;
pub mod reach;
pub mod view;
pub mod virtual_net;

pub use apply::{apply, gate, install_humans, score, step, Applied, RoundState};
pub use command::{
    cheat_bits, cheat_index, cheats_from_bits, validate_request, validate_request_bytes,
    GameCommand, GameRequest, LeftWhy, Refusal, ALL_CHEATS, ALL_SLOTS, CHEAT_COUNT,
};
pub use content::{build_id, fnv64, target, ContentManifest};
pub use desync::DesyncReport;
pub use dto::{AssistSettings, EnvSwitches, HumanSeat, PlayerProfile, RoundOptions, RoundSetup};
pub use lobby::{GameNotice, HostInfo, LobbyPhase, LobbyPlayer, LobbyState};
pub use meter::{MeterPeer, NetMeter};
pub use peer::{
    FrameReport, Guest, Link, LinkEvent, Mode, Notice, Peer, PeerStats, DESYNC_WINDOW_S, HASH_EVERY,
};
pub use predict::{PredictMode, PredictStats, PredictedShot, Predictor};
pub use view::LocalView;

#[cfg(test)]
mod smooth_tests;
#[cfg(test)]
mod tests;
