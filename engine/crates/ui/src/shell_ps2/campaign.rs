//! Rise of the Empire's mission list and its navigation, the logic side of
//! [`super::Page::RiseOfTheEmpire`] / [`super::Page::CampaignBattle`].
//!
//! The list shows a window of at most [`CAMPAIGN_WINDOW`] mission rows plus
//! the "view all" toggle. Focus moves between missions and the toggle, which
//! swaps between unlocked-only and the whole 18-mission catalog; the note
//! under the window names the highlighted mission or the shown/total count.
//!
//! The fields these methods touch stay on [`ShellMenus`] so the game's
//! `menus.campaign_unlocked` / `menus.campaign_selected` reads are unchanged;
//! this module owns the behaviour. The list's new text keys live in the
//! texts section below and are resolved once through `text(key, English)` in
//! [`ShellMenus::new`], so rebuilding on a profile change needs no closure.

use super::{Page, RowId, ShellCommand, ShellMenus, Stack, StackRow};
use crate::menu::MenuInput;
use crate::shell_feel::MenuSound;

/// Maximum mission rows shown at once so the stack and note fit within the
/// 448-unit shell height.
const CAMPAIGN_WINDOW: usize = 6;

// The campaign list's localizable texts, each `(key, English fallback)`.
// The English strings are the exact ones the screen showed before these were
// keyed; `text(key, English)` resolves them once in `ShellMenus::new`.
/// The toggle's label while only unlocked missions are listed. This reuses
/// the key the toggle row already carries ([`RowId::CampaignToggleAll`]).
pub(super) const TEXT_VIEW_ALL: (&str, &str) = ("openbf.campaign.toggle_all", "View All Missions");
/// The toggle's label while every mission is listed.
pub(super) const TEXT_SHOW_UNLOCKED: (&str, &str) =
    ("openbf.campaign.show_unlocked", "Show Unlocked Only");
/// The note while the toggle has focus. `{count}` is the shown missions and
/// `{total}` the catalog's length.
pub(super) const TEXT_SHOWING: (&str, &str) = (
    "openbf.campaign.showing",
    "Showing {count} of {total} missions",
);
/// The note while a mission has focus. `{era}` is uppercased, `{index}` is
/// the mission's 1-based number, `{count}` the shown missions and `{planet}`
/// the planet's localized name.
pub(super) const TEXT_MISSION_NOTE: (&str, &str) = (
    "openbf.campaign.mission_note",
    "{era} ({index}/{count}) \u{2014} {planet}",
);

impl ShellMenus {
    fn visible_campaign_count(&self) -> usize {
        let total = crate::campaign::RISE_OF_THE_EMPIRE.len();
        if self.campaign_view_all {
            total
        } else {
            (self.campaign_unlocked as usize).clamp(1, total)
        }
    }

    /// Update the number of unlocked campaign missions (`1..=18`) from the
    /// active profile.
    pub fn set_campaign_unlocked(&mut self, unlocked: u8) {
        let total = crate::campaign::RISE_OF_THE_EMPIRE.len();
        self.campaign_unlocked = unlocked.clamp(1, total as u8);
        self.campaign_selected = self
            .campaign_selected
            .min(self.visible_campaign_count() - 1);
        self.rebuild_campaign_stacks();
    }

    /// Highlight mission `index` on [`Page::RiseOfTheEmpire`].
    pub fn select_campaign_mission(&mut self, index: usize) {
        let count = self.visible_campaign_count();
        self.campaign_selected = index.min(count - 1);
        self.campaign_focus_toggle = false;
        self.rebuild_campaign_stacks();
    }

    pub(super) fn rebuild_campaign_stacks(&mut self) {
        let count = self.visible_campaign_count();
        self.campaign_selected = self.campaign_selected.min(count - 1);
        let win = count.min(CAMPAIGN_WINDOW);
        let win_start = if count <= CAMPAIGN_WINDOW {
            0
        } else {
            self.campaign_selected
                .saturating_sub(CAMPAIGN_WINDOW / 2)
                .min(count - win)
        };
        let mut rows = Vec::with_capacity(win + 1);
        for i in win_start..win_start + win {
            let entry = &crate::campaign::RISE_OF_THE_EMPIRE[i];
            rows.push(StackRow {
                id: RowId::CampaignMission(i as u8),
                label: entry.title.to_owned(),
                enabled: true,
            });
        }
        rows.push(StackRow {
            id: RowId::CampaignToggleAll,
            label: if self.campaign_view_all {
                self.campaign_show_unlocked.clone()
            } else {
                self.campaign_view_all_text.clone()
            },
            enabled: true,
        });
        let focus = if self.campaign_focus_toggle {
            rows.len() - 1
        } else {
            self.campaign_selected - win_start
        };
        let note = if self.campaign_focus_toggle {
            self.campaign_showing_fmt
                .replace("{count}", &count.to_string())
                .replace(
                    "{total}",
                    &crate::campaign::RISE_OF_THE_EMPIRE.len().to_string(),
                )
        } else {
            let e = &crate::campaign::RISE_OF_THE_EMPIRE[self.campaign_selected];
            let planet = &self.campaign_planets[self.campaign_selected];
            self.campaign_mission_note_fmt
                .replace("{era}", &e.era.to_ascii_uppercase())
                .replace("{index}", &(self.campaign_selected + 1).to_string())
                .replace("{count}", &count.to_string())
                .replace("{planet}", planet)
        };
        self.campaign = Stack {
            title: self.campaign_title.clone(),
            rows,
            focus,
            left: true,
            y_add: 0.0,
            note: Some(note),
        };
        let selected = &crate::campaign::RISE_OF_THE_EMPIRE[self.campaign_selected];
        self.campaign_battle = Stack {
            title: selected.title.to_owned(),
            rows: vec![
                StackRow {
                    id: RowId::CampaignPlay,
                    label: self.campaign_launch_label.clone(),
                    enabled: true,
                },
                StackRow {
                    id: RowId::CampaignBack,
                    label: self.back.clone(),
                    enabled: true,
                },
            ],
            focus: 0,
            left: true,
            y_add: 0.0,
            note: Some(self.campaign_descs[self.campaign_selected].clone()),
        };
    }

    pub(super) fn step_campaign(&mut self, input: MenuInput) -> Option<ShellCommand> {
        let count = self.visible_campaign_count();
        let total_slots = count + 1; // missions + toggle row
        let cur = if self.campaign_focus_toggle {
            count
        } else {
            self.campaign_selected.min(count - 1)
        };
        let mut next = cur;
        if input.up {
            next = (cur + total_slots - 1) % total_slots;
        }
        if input.down {
            next = (cur + 1) % total_slots;
        }
        if next != cur {
            if next == count {
                self.campaign_focus_toggle = true;
            } else {
                self.campaign_focus_toggle = false;
                self.campaign_selected = next;
            }
            self.rebuild_campaign_stacks();
            self.cues.play(MenuSound::Select);
        }
        if !input.confirm {
            return None;
        }
        self.cues.play(MenuSound::Accept);
        if self.campaign_focus_toggle {
            self.campaign_view_all = !self.campaign_view_all;
            self.campaign_selected = self
                .campaign_selected
                .min(self.visible_campaign_count() - 1);
            self.rebuild_campaign_stacks();
        } else {
            self.go(Page::CampaignBattle);
        }
        None
    }
}
