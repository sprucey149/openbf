//! Sounds for changes the spawn picker accepted, including its pointer picks.

use core::sim::{SideChoice, SpawnStage};
use core::soldier::GroundRules;
use core::unit::SpawnPost;
use game::walk::WalkSim;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Selection {
    Side(SideChoice),
    Post(Option<SpawnPost>),
    Class(SpawnStage, Option<usize>),
    Closed,
}

impl Selection {
    pub(super) fn read(walk: &WalkSim) -> Self {
        let sim = &walk.sim;
        let stage = sim.spawn_stage.first().copied().unwrap_or_default();
        if !sim.spawn_screens && stage != SpawnStage::Alive {
            return Self::Closed;
        }
        match stage {
            SpawnStage::Side => Self::Side(sim.side_choice[0]),
            SpawnStage::Map => Self::Post(sim.spawn_post_choice[0]),
            SpawnStage::Class | SpawnStage::Alive => {
                Self::Class(stage, sim.spawn_class_choice(0, walk.player_team()))
            }
            SpawnStage::Accepted => Self::Closed,
        }
    }

    /// The BF1 team toggle uses its unit-change cue. BF2's team page uses
    /// the shell cursor cue (analysis/re-ambience-music.md §6).
    pub(super) fn movement_cue(self, next: Self, rules: GroundRules) -> Option<&'static str> {
        if self == next {
            return None;
        }
        match (self, next) {
            (Self::Side(_), Self::Side(_)) if rules == GroundRules::Bf1 => {
                Some("shell_select_unit")
            }
            (Self::Side(_), Self::Side(_)) | (Self::Post(_), Self::Post(Some(_))) => {
                Some("shell_select_change")
            }
            (Self::Class(stage, _), Self::Class(next_stage, Some(_))) if stage == next_stage => {
                Some("shell_select_unit")
            }
            _ => None,
        }
    }
}

/// A developer notice belongs above the spawn and pause overlays.
pub(super) fn draw_snapshot_notice(ctx: &ui::egui::Context, saved: bool) {
    use ui::egui::{Align2, Area, Frame, Id, Order};
    Area::new(Id::new("snapshot_notice"))
        .anchor(Align2::CENTER_TOP, [0.0, 20.0])
        .order(Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(if saved {
                    "Snapshot saved"
                } else {
                    "Snapshot save failed"
                });
            });
        });
}
