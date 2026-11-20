//! Rise of the Empire's save and load screen (`ifs_campaign_load`) with its
//! popups, as the disc's shell Lua runs them (`extracted/bf2/lua/shell/`;
//! `analysis/re-bf2-menus.md` "Rise of the Empire saves").
//!
//! - **Save prompt** (`Popup_LoadSave2`, `ifs_campaign_load` main.f13):
//!   "Would you like to save your progress?" over Continue Without Saving
//!   (B) and Save (A), focus Save (pc 63-65). `ifs_campaign_main`
//!   ApplyBattleResult (main.f8 pc 36-37) asks for it after every battle
//!   result, the planet screen's Enter shows it (`ifs_campaign_battle`
//!   main.f0 pc 97-99 -> `PromptSave`, main.f4 pc 6-21), and the pause
//!   menu's Quit asks it before leaving (`ifs_campaign_menu` main.f1 pc
//!   11-16).
//! - **Load prompt** (main.f8): "Are you sure you want to load? ..." over
//!   Cancel and Continue, focus Continue; the pause menu's Load Game shows
//!   it, the Rise of the Empire screen's Load skips it (`SkipPromptSave`,
//!   `ifs_freeform_rise_newload` main.f2 pc 28-34).
//! - **The list** (main.f4, f32): Save lists "< Save as New >" first, then
//!   the saves; Load the saves, or "No Saved Games Available". The save
//!   last loaded or written comes first, then the newest
//!   (`ScriptCB_GetSavedCampaignList` FUN_0018a300 sorts with
//!   FUN_00189ec8), and the focus starts on it (main.f4 pc 53-75).
//!   Save as New on a full list (more than `iMaxSaves` rows, 32: FUN_0018a300
//!   pushes 0x42000000) shows the list-full popup (main.f32 pc 95-121);
//!   any other row opens the name keyboard (pc 129-173), and a save with
//!   the same file first asks to overwrite (`ifs_saveop` main.f30). Delete
//!   (the PS2's Square, help text right-justified 40 above the bottom,
//!   main.f36 pc 113-123) asks first (main.f35); only in Save mode (main.f35
//!   pc 0-5, the help shown by main.f2 pc 18-30) and never on Save as New
//!   (main.f4 pc 76-90).
//! - **The look** (main.f0, f1, f36): a `NewButtonWindow` titled "Load
//!   Game" / "Save Game" over the movie already playing (no
//!   `movieBackground`), rows of an icon and two `gamefont_small` lines,
//!   the name and the date, the `listbox_cursor` box on the focused row;
//!   help "Load" / "Save" (main.f23 pc 99-162). The popups are sized by
//!   `gPopup_fnSetTitle_Internal` (`popups_common` main.f0) with their
//!   buttons in `gamefont_small` ([`popup_layout`]).
//! - Back on the list leaves (main.f33); Back on a Yes / No or Save prompt
//!   does nothing (`popup_yesno` main.f5, `popup_loadsave2` main.f4 are
//!   empty); Back on the list-full popup is its OK (`popup_ok_large` pc
//!   26-28).

use super::{Page, RowId, ShellCommand, ShellMenus, Stack, StackRow, BAR_PAD, ROW_REVEAL};
use crate::hud_draw::{Ctx, Screen};
use crate::hud_tree::HAlign;
use crate::input_icons::Prompt;
use crate::menu::MenuInput;
use crate::shell_feel::MenuSound;
use crate::spawn_draw::{screen_at, Anchor};
use crate::typography::{tint, BODY};
use manifest::hud::HudFont;

/// `iMaxSaves`: FUN_0018a300 returns 32.0 beside the list.
pub const MAX_SAVES: usize = 32;
/// A save's name on the PS2 keyboard (`ifs_campaign_load` main.f32 pc
/// 152-156: `MaxLen` 10 on the PS2).
pub const NAME_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveMode {
    Save,
    Load,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStep {
    Prompt,
    ConfirmLoad,
    List,
    Overwrite,
    ListFull,
    Delete,
}

/// The screen the save screen returns to when it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveReturn {
    /// The planet screen, after a battle's result.
    Battle,
    /// The planet's pause menu (its Save Game / Load Game).
    Menu,
    /// Singleplayer: the pause menu's Quit asked for it (`QuitRequested`,
    /// `ifs_campaign_menu` main.f3 clears the campaign and leaves).
    Quit,
    /// Rise of the Empire's New Game / Load.
    NewLoad,
}

/// A write the game is asked for: `replace` the save of that name (or none:
/// a new one) with one named `name`, as `ScriptCB_StartSaveCampaign
/// (filename1, filename2)` does (`ifs_campaign_load` main.f15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveWrite {
    pub replace: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SaveTexts {
    pub save_title: String,
    pub load_title: String,
    pub save_as_new: String,
    pub no_games: String,
    pub confirm_save: String,
    pub confirm_load: String,
    pub overwrite: String,
    pub list_full: String,
    pub confirm_delete: String,
    pub save: String,
    pub continue_no_save: String,
    pub load_continue: String,
    pub load_cancel: String,
    pub yes: String,
    pub no: String,
    pub ok: String,
    pub delete: String,
    pub keyboard_title: String,
    pub too_short: String,
    pub btn_load: String,
    pub btn_save: String,
}

impl SaveTexts {
    pub(super) fn new(text: &dyn Fn(&str, &str) -> String) -> Self {
        Self {
            save_title: text("ifs.meta.load.save", "Save Game"),
            load_title: text("ifs.meta.load.load", "Load Game"),
            save_as_new: text("ifs.meta.load.saveasnew", "< Save as New >"),
            no_games: text("ifs.meta.load.nogames", "No Saved Games Available"),
            confirm_save: text(
                "ifs.meta.load.confirm_save",
                "Would you like to save your progress?",
            ),
            confirm_load: text(
                "ifs.meta.load.confirm_load",
                "Are you sure you want to load? All unsaved game progress will be lost.",
            ),
            overwrite: text(
                "ifs.loadsave_ps2.save25",
                "Are you sure you wish to overwrite this Campaign save file?",
            ),
            list_full: text(
                "ifs.meta.load.listfull",
                "Save list is full. To create another save will require deleting an existing item. Or, you can overwrite an existing item.",
            ),
            confirm_delete: text(
                "ifs.meta.load.confirm_delete",
                "Do you really want to delete this saved game?",
            ),
            save: text("ifs.loadsave_ps2.save", "Save"),
            continue_no_save: text("ifs.loadsave_ps2.continuenosave", "Continue Without Saving"),
            load_continue: text("ifs.loadsave_ps2.continue", "Continue"),
            load_cancel: text("ifs.loadsave_ps2.cancel", "Cancel"),
            yes: text("common.yes", "Yes"),
            no: text("common.no", "No"),
            ok: text("common.ok", "OK"),
            delete: text("ifs.profile.delete", "Delete"),
            keyboard_title: text("ifs.vkeyboard.title_rote", "Enter Save Name:"),
            too_short: text("ifs.vkeyboard.tooshort", "Please enter at least one character"),
            btn_load: text("ifs.meta.load.btnload", "Load"),
            btn_save: text("ifs.meta.load.btnsave", "Save"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CampaignSaves {
    pub mode: SaveMode,
    pub step: SaveStep,
    pub back_to: SaveReturn,
    /// The saves' names in the list's order.
    pub names: Vec<String>,
    /// Their dates as the list shows them (beside [`Self::names`]).
    pub dates: Vec<String>,
    /// The save last loaded or written this session (`DAT_0066c438`, the
    /// list's `isCurrent`).
    pub current: Option<String>,
    /// The focused list row (Save: 0 is Save as New).
    pub selected: usize,
    /// The first row the list shows (`FirstShownIdx`): the focused row when
    /// the list opens (main.f4 pc 70-75). OURS: pulled back so the window
    /// stays full and a move scrolls one row past its edge. trace:
    /// `ListManager_fnFillContents` and `fnNavUp` / `fnNavDown`.
    pub first: usize,
    /// The save the keyboard or the overwrite prompt is for.
    pub pending: Option<SaveWrite>,
    /// `[ours]` a failed write or read, under the list.
    pub message: Option<String>,
    pub stack: Stack,
    pub(super) texts: SaveTexts,
}

impl CampaignSaves {
    pub(super) fn new(text: &dyn Fn(&str, &str) -> String) -> Self {
        Self {
            mode: SaveMode::Save,
            step: SaveStep::List,
            back_to: SaveReturn::Battle,
            names: Vec::new(),
            dates: Vec::new(),
            current: None,
            selected: 0,
            first: 0,
            pending: None,
            message: None,
            stack: Stack {
                title: String::new(),
                rows: Vec::new(),
                focus: 0,
                left: true,
                y_add: 0.0,
                note: None,
            },
            texts: SaveTexts::new(text),
        }
    }

    fn first_save_row(&self) -> usize {
        usize::from(self.mode == SaveMode::Save)
    }

    fn row_count(&self) -> usize {
        self.first_save_row() + self.names.len()
    }

    /// The save under the focused row, if it is one.
    pub fn selected_name(&self) -> Option<&str> {
        self.selected
            .checked_sub(self.first_save_row())
            .and_then(|i| self.names.get(i))
            .map(String::as_str)
    }

    /// The text the popup's box shows, `None` on the list.
    pub fn prompt(&self) -> Option<&str> {
        let t = &self.texts;
        Some(match self.step {
            SaveStep::Prompt => &t.confirm_save,
            SaveStep::ConfirmLoad => &t.confirm_load,
            SaveStep::Overwrite => &t.overwrite,
            SaveStep::ListFull => &t.list_full,
            SaveStep::Delete => &t.confirm_delete,
            SaveStep::List => return None,
        })
    }

    pub fn keyboard_title(&self) -> &str {
        &self.texts.keyboard_title
    }

    pub fn too_short(&self) -> &str {
        &self.texts.too_short
    }

    /// The Delete help line's label, while the Save list has a save
    /// focused (main.f4 pc 76-90; Load mode never deletes on the PS2, main.f35
    /// pc 0-5).
    pub fn delete_help(&self) -> Option<&str> {
        (self.step == SaveStep::List
            && self.mode == SaveMode::Save
            && self.selected_name().is_some())
        .then_some(self.texts.delete.as_str())
    }

    /// The accept help: "Load" or "Save" (main.f23 pc 99-162).
    pub fn accept_help(&self) -> &str {
        match self.mode {
            SaveMode::Load => &self.texts.btn_load,
            SaveMode::Save => &self.texts.btn_save,
        }
    }

    /// The window's title (`gButtonWindow_fnSetText`, main.f23 pc 99-146).
    pub fn title(&self) -> &str {
        match self.mode {
            SaveMode::Load => &self.texts.load_title,
            SaveMode::Save => &self.texts.save_title,
        }
    }

    /// The list stays on screen behind this step's popup: the save prompt,
    /// the load prompt and the delete question hide it (`ifs_campaign_load_SetVis`
    /// nil in main.f8, f13, f35), the list-full popup and the overwrite
    /// question do not (main.f32 pc 95-121, `ifs_saveop` main.f30).
    pub fn list_shown(&self) -> bool {
        matches!(
            self.step,
            SaveStep::List | SaveStep::ListFull | SaveStep::Overwrite
        )
    }

    /// The row the list's `k`-th visible line shows: "< Save as New >", a
    /// save's name and date, or none.
    pub fn row(&self, i: usize) -> Option<(&str, &str, bool)> {
        let first = self.first_save_row();
        if i < first {
            return Some((&self.texts.save_as_new, "", false));
        }
        let k = i - first;
        let name = self.names.get(k)?;
        let date = self.dates.get(k).map_or("", String::as_str);
        Some((name, date, true))
    }

    /// The text shown when the list is empty (`nogames`, main pc 65-77).
    pub fn no_games(&self) -> Option<&str> {
        (self.row_count() == 0).then_some(self.texts.no_games.as_str())
    }

    fn popup(&mut self, rows: Vec<(RowId, String)>, focus: usize) {
        self.stack = Stack {
            title: String::new(),
            rows: rows
                .into_iter()
                .map(|(id, label)| StackRow {
                    id,
                    label,
                    enabled: true,
                })
                .collect(),
            focus,
            left: false,
            y_add: -32.0,
            note: None,
        };
    }

    pub(super) fn rebuild(&mut self) {
        let t = self.texts.clone();
        match self.step {
            SaveStep::Prompt => self.popup(
                vec![
                    (RowId::SaveContinue, t.continue_no_save),
                    (RowId::SaveConfirm, t.save),
                ],
                1,
            ),
            SaveStep::ConfirmLoad => self.popup(
                vec![
                    (RowId::LoadCancel, t.load_cancel),
                    (RowId::LoadContinue, t.load_continue),
                ],
                1,
            ),
            SaveStep::Overwrite | SaveStep::Delete => {
                self.popup(vec![(RowId::SaveYes, t.yes), (RowId::SaveNo, t.no)], 1)
            }
            SaveStep::ListFull => self.popup(vec![(RowId::SaveOk, t.ok)], 0),
            SaveStep::List => {
                let count = self.row_count();
                let mut rows = Vec::new();
                if count == 0 {
                    rows.push(StackRow {
                        id: RowId::NoSavedGames,
                        label: t.no_games,
                        enabled: false,
                    });
                    self.selected = 0;
                } else {
                    self.selected = self.selected.min(count - 1);
                    let win = count.min(LIST_SHOWN);
                    if self.selected < self.first {
                        self.first = self.selected;
                    } else if self.selected >= self.first + win {
                        self.first = self.selected + 1 - win;
                    }
                    self.first = self.first.min(count - win);
                    let start = self.first;
                    for i in start..start + win {
                        let label = if i < self.first_save_row() {
                            t.save_as_new.clone()
                        } else {
                            self.names[i - self.first_save_row()].clone()
                        };
                        let id = if i < self.first_save_row() {
                            RowId::SaveAsNew
                        } else {
                            RowId::SaveSlot((i - self.first_save_row()) as u8)
                        };
                        rows.push(StackRow {
                            id,
                            label,
                            enabled: true,
                        });
                    }
                    self.stack.focus = self.selected - start;
                }
                let focus = if count == 0 { 0 } else { self.stack.focus };
                self.stack = Stack {
                    title: match self.mode {
                        SaveMode::Save => t.save_title,
                        SaveMode::Load => t.load_title,
                    },
                    rows,
                    focus,
                    left: true,
                    y_add: 0.0,
                    note: self.message.clone(),
                };
            }
        }
    }

    fn default_row(&self) -> usize {
        self.current
            .as_ref()
            .and_then(|c| self.names.iter().position(|n| n == c))
            .map_or(0, |i| i + self.first_save_row())
    }
}

impl ShellMenus {
    /// Open the save screen in `mode`, back to `back_to` when it closes;
    /// `prompt` first asks the save or the load question.
    pub fn open_campaign_saves(&mut self, mode: SaveMode, back_to: SaveReturn, prompt: bool) {
        let s = &mut self.campaign_saves;
        s.mode = mode;
        s.back_to = back_to;
        s.pending = None;
        s.message = None;
        s.step = match (prompt, mode) {
            (true, SaveMode::Save) => SaveStep::Prompt,
            (true, SaveMode::Load) => SaveStep::ConfirmLoad,
            (false, _) => SaveStep::List,
        };
        s.selected = s.default_row();
        s.first = s.selected;
        s.rebuild();
        self.go(Page::CampaignSaves);
    }

    /// The saves' names and dates in the list's order (the game sends them
    /// whenever the profile's saves change).
    pub fn set_campaign_saves(&mut self, names: Vec<String>, dates: Vec<String>) {
        let s = &mut self.campaign_saves;
        if s.names == names && s.dates == dates {
            return;
        }
        s.names = names;
        s.dates = dates;
        s.rebuild();
    }

    fn close_campaign_saves(&mut self) {
        self.campaign_saves.pending = None;
        let to = match self.campaign_saves.back_to {
            SaveReturn::Battle => Page::CampaignBattle,
            SaveReturn::Menu => Page::CampaignMenu,
            SaveReturn::Quit => Page::Singleplayer,
            SaveReturn::NewLoad => Page::RiseNewLoad,
        };
        if to == Page::CampaignMenu {
            self.campaign_menu.focus = self.campaign_menu.first();
        }
        self.go(to);
    }

    fn saves_step_to(&mut self, step: SaveStep) {
        self.campaign_saves.step = step;
        self.campaign_saves.rebuild();
        self.on_page = 0.0;
    }

    /// Square on the list: delete the focused save, after asking.
    pub fn campaign_save_delete(&mut self) {
        let s = &self.campaign_saves;
        if self.page != Page::CampaignSaves || s.delete_help().is_none() {
            return;
        }
        self.cues.play(MenuSound::Accept);
        self.saves_step_to(SaveStep::Delete);
    }

    /// The name keyboard's Accept: write the pending save, first asking
    /// when it would replace one.
    pub fn campaign_save_named(&mut self, name: String) -> Option<ShellCommand> {
        let s = &mut self.campaign_saves;
        let replace = s.pending.take().and_then(|p| p.replace).or_else(|| {
            s.names
                .iter()
                .find(|n| n.eq_ignore_ascii_case(&name))
                .cloned()
        });
        let overwrite = replace.is_some();
        s.pending = Some(SaveWrite { replace, name });
        if overwrite {
            self.saves_step_to(SaveStep::Overwrite);
            None
        } else {
            Some(ShellCommand::WriteCampaignSave)
        }
    }

    /// The keyboard closed without a name: back on the list.
    pub fn campaign_save_name_cancelled(&mut self) {
        self.campaign_saves.pending = None;
        self.saves_step_to(SaveStep::List);
    }

    /// The game wrote the pending save (`Err`: the reason it did not).
    pub fn campaign_save_written(&mut self, result: Result<(), String>) {
        let written = self.campaign_saves.pending.take();
        match result {
            Ok(()) => {
                self.campaign_saves.current = written.map(|w| w.name);
                self.close_campaign_saves();
            }
            Err(message) => {
                self.campaign_saves.message = Some(message);
                self.cues.play(MenuSound::Error);
                self.saves_step_to(SaveStep::List);
            }
        }
    }

    /// The game read the focused save: its turn opens on the planet screen.
    pub fn campaign_save_loaded(&mut self, turn: usize) {
        self.campaign_saves.current = self.campaign_saves.selected_name().map(str::to_owned);
        self.select_campaign_mission(turn);
        self.go(Page::CampaignBattle);
    }

    /// The game could not read or delete a save.
    pub fn campaign_save_failed(&mut self, message: String) {
        self.campaign_saves.message = Some(message);
        self.cues.play(MenuSound::Error);
        self.saves_step_to(SaveStep::List);
    }

    /// The game deleted the focused save (and sent the new names).
    pub fn campaign_save_deleted(&mut self) {
        let s = &mut self.campaign_saves;
        if s.current.as_ref().is_some_and(|c| !s.names.contains(c)) {
            s.current = None;
        }
        self.saves_step_to(SaveStep::List);
    }

    pub(super) fn step_campaign_saves(&mut self, input: MenuInput) -> Option<ShellCommand> {
        let step = self.campaign_saves.step;
        if input.back {
            match step {
                SaveStep::List => {
                    self.cues.play(MenuSound::Exit);
                    self.close_campaign_saves();
                    return None;
                }
                SaveStep::ListFull => {
                    self.cues.play(MenuSound::Accept);
                    self.saves_step_to(SaveStep::List);
                    return None;
                }
                _ => return None,
            }
        }
        if step == SaveStep::List {
            let count = self.campaign_saves.row_count();
            if count > 0 && (input.up || input.down) {
                let s = &mut self.campaign_saves;
                s.selected = if input.up {
                    (s.selected + count - 1) % count
                } else {
                    (s.selected + 1) % count
                };
                s.rebuild();
                self.cues.play(MenuSound::Select);
            }
            if !input.confirm {
                return None;
            }
            let s = &self.campaign_saves;
            if count == 0 {
                self.cues.play(MenuSound::Accept);
                self.close_campaign_saves();
                return None;
            }
            self.cues.play(MenuSound::Accept);
            return match s.mode {
                SaveMode::Load => Some(ShellCommand::LoadCampaignSave),
                SaveMode::Save => {
                    let replace = s.selected_name().map(str::to_owned);
                    if replace.is_none() && s.names.len() >= MAX_SAVES {
                        self.saves_step_to(SaveStep::ListFull);
                        return None;
                    }
                    self.campaign_saves.pending = Some(SaveWrite {
                        replace,
                        name: String::new(),
                    });
                    Some(ShellCommand::EditCampaignSaveName)
                }
            };
        }
        let s = &mut self.campaign_saves.stack;
        if input.up || input.down {
            let before = s.focus;
            s.focus = s.follow(if input.up { -1 } else { 1 });
            if s.focus != before {
                self.cues.play(MenuSound::Select);
            }
        }
        if !input.confirm {
            return None;
        }
        let row = s.rows.get(s.focus).map(|r| r.id)?;
        let accept = matches!(
            row,
            RowId::SaveConfirm | RowId::LoadContinue | RowId::SaveYes | RowId::SaveOk
        );
        self.cues.play(if accept {
            MenuSound::Accept
        } else {
            MenuSound::Cancel
        });
        match row {
            RowId::SaveConfirm | RowId::LoadContinue | RowId::SaveOk => {
                self.saves_step_to(SaveStep::List)
            }
            RowId::SaveContinue | RowId::LoadCancel => self.close_campaign_saves(),
            RowId::SaveYes if step == SaveStep::Overwrite => {
                return Some(ShellCommand::WriteCampaignSave)
            }
            RowId::SaveYes => return Some(ShellCommand::DeleteCampaignSave),
            RowId::SaveNo => {
                self.campaign_saves.pending = None;
                self.saves_step_to(SaveStep::List);
            }
            _ => {}
        }
        None
    }
}

/// The PS2's safe area: x, y, width, height (`analysis/re-bf2-shell.md`
/// §3.5).
const SAFE: [f32; 4] = [25.6, 22.4, 460.8, 403.2];
/// The shell's centre (`ScreenRelativeX` / `ScreenRelativeY` 0.5).
const MID: [f32; 2] = [256.0, 224.0];
/// `ScriptCB_GetFontHeight("gamefont_small")` (`analysis/re-bf2-menus.md`
/// §2.2).
const SMALL_H: f32 = 18.0;
/// A save's row: two lines and 4 (main.f36 pc 4-17).
pub const LIST_ROW: f32 = 2.0 * SMALL_H + 4.0;
/// The rows shown: `floor(safe height x 0.75 / row)` (main.f36 pc 18-28).
pub const LIST_SHOWN: usize = 7;
const WINDOW_W: f32 = 0.9 * SAFE[2];
const WINDOW_H: f32 = LIST_SHOWN as f32 * LIST_ROW + 30.0;
/// The `NewButtonWindow`: 0.9 x the safe width, the rows + 30 tall,
/// centred 10 above the middle (main.f36 pc 0-42).
pub const LIST_WINDOW: [f32; 4] = [
    MID[0] - WINDOW_W * 0.5,
    MID[1] - 10.0 - WINDOW_H * 0.5,
    MID[0] + WINDOW_W * 0.5,
    MID[1] - 10.0 + WINDOW_H * 0.5,
];
/// The rows: the window's width - 40, centred (main.f36 pc 44-48).
const LIST_X: [f32; 2] = [
    MID[0] - (WINDOW_W - 40.0) * 0.5,
    MID[0] + (WINDOW_W - 40.0) * 0.5,
];
/// OURS: the first row's top, the window's 30 spare units split evenly.
/// trace: `ListManager_fnInitList`'s `yTop`.
const LIST_TOP: f32 = LIST_WINDOW[1] + 15.0;
/// The type icon: 1.5 x the font's height square, 10 in, centred on the row
/// (main.f0 pc 10-29).
const ICON: [f32; 2] = [10.0, 1.5 * SMALL_H];
/// The name and date: 20 right of the icon, the name's line one font height
/// - 2 above the row's middle, the date's at it (main.f0 pc 30-71).
const TEXT_X: f32 = ICON[0] + ICON[1] + 10.0;
const NAME_Y: f32 = -SMALL_H + 2.0;
/// The type icon of a Rise of the Empire save (main.f1 pc 22-33).
pub const ROTE_ICON: &str = "rote_icon";
/// The `listbox_cursor` line around the focused row: the sheet's line sits 3
/// texels in across and 4 / 10 down / up of its 64 (`shell_ia`'s
/// `cursor_box`), drawn as a `NewButtonWindow` the row's width and its
/// height + 10 with 16-unit corners (`listmanager` main.f0 pc 149-154): 3 in,
/// 36 tall. [M] centred on the row (the PS2's Load Game frame: the line's
/// middle 0.5 units above the name and date's).
const CURSOR: [f32; 2] = [3.0, LIST_ROW + 10.0 - 14.0];
/// The bottom scroll arrow sits 10 higher (`fBottomArrowYOffset`, main pc
/// 16).
const ARROW_LIFT: f32 = 10.0;
/// `gButtonWidthPad`, `gPopupWidthPad` / `gPopupHeightPad`
/// (`analysis/re-bf2-shell.md` §3.1).
const BUTTON_PAD: f32 = 100.0;
const POPUP_PAD: f32 = 32.0;
/// A popup button: `gamefont_small` + `gButtonHeightPad` 6, every + 4
/// (`gButtonGutter`, `ifelem_button` main.f6 pc 30-42).
const POPUP_ROW: f32 = SMALL_H + 6.0;
const POPUP_PITCH: f32 = POPUP_ROW + 4.0;

/// A popup as `NewPopup` builds it.
struct PopupSpec {
    width: f32,
    height: f32,
    /// `ButtonHeightHint` (50 when unset, `popups_common` main.f0 pc 106-109).
    hint: f32,
    no_thin: bool,
    /// The text's top from the middle.
    y2: f32,
    /// The button layout's `yTop`.
    y_top: f32,
    /// `fMinWidth` = the buttons' width + 20 (`popup_loadsave2` main.f1 pc
    /// 38-39, `popup_yesno` main.f0 pc 33-34).
    fit_buttons: bool,
}

/// `Popup_LoadSave2` (`popup_loadsave2` main pc 0-18, 35-68).
const LOADSAVE2: PopupSpec = PopupSpec {
    width: 460.0,
    height: 280.0,
    hint: 95.0,
    no_thin: true,
    y2: -80.0,
    y_top: -10.0,
    fit_buttons: true,
};
/// `Popup_YesNo` (`popup_yesno` main pc 8-30; `Vertical_YesNoButtons_layout`
/// `popups_common` main pc 24-39).
const YESNO: PopupSpec = PopupSpec {
    width: 300.0,
    height: 180.0,
    hint: 75.0,
    no_thin: false,
    y2: -80.0,
    y_top: 1.0,
    fit_buttons: true,
};
/// `Popup_Ok_Large` (`popup_ok_large` main pc 0-14; `OkButton_layout`
/// `popups_common` main pc 0-11). OURS: its `gamefont_medium` text drawn in
/// `gamefont_small` (the menu face has no medium set). trace: the medium
/// set's metrics in the menu face.
const OK_LARGE: PopupSpec = PopupSpec {
    width: 400.0,
    height: 240.0,
    hint: 50.0,
    no_thin: false,
    y2: -110.0,
    y_top: 0.0,
    fit_buttons: false,
};

/// A popup laid out: its frame, its text's lines from `text_top`, centred,
/// and its buttons' bars, in shell units.
#[derive(Debug, Clone, PartialEq)]
pub struct PopupLayout {
    pub frame: [f32; 4],
    pub lines: Vec<String>,
    pub text_top: f32,
    pub buttons: Vec<[f32; 4]>,
}

impl CampaignSaves {
    fn popup_spec(&self) -> Option<&'static PopupSpec> {
        Some(match self.step {
            SaveStep::Prompt | SaveStep::ConfirmLoad => &LOADSAVE2,
            SaveStep::Overwrite | SaveStep::Delete => &YESNO,
            SaveStep::ListFull => &OK_LARGE,
            SaveStep::List => return None,
        })
    }

    /// The step's popup as `gPopup_fnSetTitle_Internal` sizes it
    /// (`popups_common` main.f0): the text box 0.75 x the width - 32, the
    /// width - 32 when the popup is never thin, the text is taller than 0.45 x
    /// the height or the box is under 1.5 x `gButtonWidthPad` (pc 0-94); at
    /// least `fMinWidth`; the frame the text + 32 across and the text + 32 +
    /// `ButtonHeightHint` down, its middle the text's top + half its height
    /// - 16; the buttons from the text's bottom + 14 (pc 95-135). Buttons are
    /// as wide as the widest label + the pad (`ShowHideVerticalButtons`
    /// f7 pc 36-64); button k sits at `yTop` + (k + 1) x height + k x gutter
    /// (f7 pc 240-265), the bar centred there ([M] the main menu's rows: row 0
    /// at the script's 237.8, its bar 224.3..252.3).
    /// OURS: the pad is the stacks' measured [`BAR_PAD`] (the popup's
    /// `HardWidthMax` from an earlier showing is not applied). measure: a PS2
    /// frame of the save prompt.
    pub fn popup_layout(&self, font: &HudFont) -> Option<PopupLayout> {
        let spec = self.popup_spec()?;
        let text = self.prompt()?;
        let style = &BODY;
        let label = self
            .stack
            .rows
            .iter()
            .map(|r| crate::typography::width(font, style, &r.label))
            .fold(0.0, f32::max);
        let button_w = label + BAR_PAD;
        let min_w = if spec.fit_buttons {
            button_w + 20.0
        } else {
            0.0
        };
        let measure = |room: f32| {
            let lines = crate::typography::wrap(font, style, text, room);
            let w = lines
                .iter()
                .map(|l| crate::typography::width(font, style, l))
                .fold(0.0, f32::max);
            let h = lines.len() as f32 * style.line;
            (lines, w, h)
        };
        let thin = (spec.width * 0.75 - POPUP_PAD).max(min_w);
        let (mut lines, mut text_w, mut text_h) = measure(thin);
        if spec.no_thin || text_h > 0.45 * spec.height || thin < 1.5 * BUTTON_PAD {
            (lines, text_w, text_h) = measure((spec.width - POPUP_PAD).max(min_w));
        }
        let w = text_w.max(min_w) + POPUP_PAD;
        let h = text_h + POPUP_PAD + spec.hint;
        let text_top = MID[1] + spec.y2;
        let mid = text_top + h * 0.5 - 16.0;
        let first = text_top + text_h + 14.0 + spec.y_top + POPUP_ROW * 0.5;
        let buttons = (0..self.stack.rows.len())
            .map(|k| {
                let y = first + POPUP_PITCH * k as f32;
                [
                    MID[0] - button_w * 0.5,
                    y,
                    MID[0] + button_w * 0.5,
                    y + POPUP_ROW,
                ]
            })
            .collect();
        Some(PopupLayout {
            frame: [
                MID[0] - w * 0.5,
                mid - h * 0.5,
                MID[0] + w * 0.5,
                mid + h * 0.5,
            ],
            lines,
            text_top,
            buttons,
        })
    }

    /// The list's `k`-th visible row's box.
    fn list_row(k: usize) -> [f32; 4] {
        let y = LIST_TOP + LIST_ROW * k as f32;
        [LIST_X[0], y, LIST_X[1], y + LIST_ROW]
    }
}

/// The save screen's row (its stack's index) under a window-space pointer:
/// a popup's button, or a visible list row. `font` is `gamefont_small`'s.
pub fn row_at(
    font: &HudFont,
    menus: &ShellMenus,
    point: [f32; 2],
    size: [f32; 2],
) -> Option<usize> {
    if size[0] <= 0.0 || size[1] <= 0.0 || menus.page != Page::CampaignSaves {
        return None;
    }
    let s = &menus.campaign_saves;
    let box_w = 4.0 / 3.0 * size[1];
    let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    let inside = |[x0, y0, x1, y1]: [f32; 4]| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    let rects: Vec<[f32; 4]> = match s.popup_layout(font) {
        Some(p) => p.buttons,
        None => (0..s.stack.rows.len())
            .map(CampaignSaves::list_row)
            .collect(),
    };
    rects
        .into_iter()
        .zip(&s.stack.rows)
        .position(|(r, row)| row.enabled && inside(r))
}

impl Ctx<'_> {
    /// The save screen: the list window (unless the step's popup hides it),
    /// the popup, and the help lines the list shows.
    pub(crate) fn campaign_saves_page(&mut self, m: &ShellMenus, size: [f32; 2]) {
        let s = &m.campaign_saves;
        let centre = screen_at(size, Anchor::Centre);
        if s.list_shown() {
            self.save_list(&centre, m);
        }
        let popup = self.face(BODY.face).and_then(|f| s.popup_layout(f));
        if let Some(p) = popup {
            self.bordered(&centre, p.frame);
            for (i, line) in p.lines.iter().enumerate() {
                self.text(
                    &centre,
                    &BODY,
                    line,
                    (MID[0], p.text_top + i as f32 * BODY.line),
                    HAlign::Center,
                    tint::BODY,
                );
            }
            let pulse = m.pulse.alpha();
            let n = s.stack.rows.len();
            for (k, (row, r)) in s.stack.rows.iter().zip(&p.buttons).enumerate() {
                let row = Ctx::flashy_row_from_stack(row, *r, k, n, s.stack.focus, pulse, &BODY);
                self.flashy_row(&centre, &row, m.on_page - ROW_REVEAL * (k + 1) as f32);
            }
        }
        if !s.list_shown() {
            return;
        }
        if s.step != SaveStep::Overwrite {
            self.help(size, false, Prompt::Back, &m.back);
        }
        self.help(size, true, Prompt::Accept, s.accept_help());
        if let Some(delete) = s.delete_help() {
            self.help_bottom(
                size,
                true,
                Prompt::Done,
                delete,
                size[1] - super::DELETE_HELP_LIFT * size[1] / 448.0,
            );
        }
    }

    /// The list window: its title bar, a row per visible save, the cursor
    /// on the focused one, the scroll arrows, "No Saved Games Available"
    /// when empty, the port's error note under it.
    fn save_list(&mut self, c: &Screen, m: &ShellMenus) {
        let s = &m.campaign_saves;
        self.window_box(c, LIST_WINDOW, Some(s.title()));
        if let Some(none) = s.no_games() {
            self.text(
                c,
                &BODY,
                none,
                (MID[0], MID[1]),
                HAlign::Center,
                tint::UNSELECTED,
            );
        }
        let count = s.row_count();
        let shown = count.min(LIST_SHOWN);
        let room = LIST_X[1] - LIST_X[0] - 20.0 - TEXT_X;
        for k in 0..shown {
            let i = s.first + k;
            let Some((name, date, icon)) = s.row(i) else {
                continue;
            };
            let [x0, y0, x1, y1] = CampaignSaves::list_row(k);
            let mid = (y0 + y1) * 0.5;
            let focused = i == s.selected;
            if focused {
                let [inset, h] = CURSOR;
                self.cursor_box(c, [x0 + inset, mid - h * 0.5, x1 - inset, mid + h * 0.5]);
            }
            if icon {
                let [ix, side] = ICON;
                self.sheet(
                    c,
                    [x0 + ix, mid - side * 0.5, x0 + ix + side, mid + side * 0.5],
                    [0.0, 0.0, 1.0, 1.0],
                    ROTE_ICON,
                    1.0,
                );
            }
            let style = if focused {
                tint::SELECTED
            } else {
                tint::UNSELECTED
            };
            for (line, y) in [(name, NAME_Y), (date, 0.0)] {
                let line = self.ellipsized(&BODY, line, room);
                self.text(c, &BODY, &line, (x0 + TEXT_X, mid + y), HAlign::Left, style);
            }
        }
        let bottom = LIST_TOP + LIST_ROW * shown as f32;
        self.more_arrows(
            c,
            MID[0],
            [LIST_TOP, bottom - ARROW_LIFT],
            s.first > 0,
            s.first + shown < count,
        );
        if let Some(note) = &s.message {
            let at = (MID[0], LIST_WINDOW[3] + 6.0);
            self.text(c, &BODY, note, at, HAlign::Center, tint::NOTE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menus(names: &[&str]) -> ShellMenus {
        let mut m = ShellMenus::new(&|_, english| english.into());
        m.set_campaign_saves(names.iter().map(|n| (*n).to_owned()).collect(), Vec::new());
        m
    }

    fn press(m: &mut ShellMenus, f: impl Fn(&mut MenuInput)) -> Option<ShellCommand> {
        let mut input = MenuInput::default();
        f(&mut input);
        m.step(input, false, 0.0)
    }

    #[test]
    fn the_battle_prompt_saves_as_new_or_continues_without_saving() {
        let mut m = menus(&[]);
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Battle, true);
        assert_eq!(m.page, Page::CampaignSaves);
        assert_eq!(m.campaign_saves.step, SaveStep::Prompt);
        let rows: Vec<_> = m.stack().unwrap().rows.iter().map(|r| r.id).collect();
        assert_eq!(rows, [RowId::SaveContinue, RowId::SaveConfirm]);
        assert_eq!(m.stack().unwrap().focus, 1);
        assert_eq!(press(&mut m, |i| i.back = true), None);
        assert_eq!(m.campaign_saves.step, SaveStep::Prompt);
        assert_eq!(press(&mut m, |i| i.confirm = true), None);
        assert_eq!(m.campaign_saves.step, SaveStep::List);
        assert_eq!(m.stack().unwrap().rows[0].label, "< Save as New >");
        assert_eq!(
            press(&mut m, |i| i.confirm = true),
            Some(ShellCommand::EditCampaignSaveName)
        );
        assert_eq!(
            m.campaign_save_named("Rex".into()),
            Some(ShellCommand::WriteCampaignSave)
        );
        assert_eq!(
            m.campaign_saves.pending,
            Some(SaveWrite {
                replace: None,
                name: "Rex".into()
            })
        );
        m.campaign_save_written(Ok(()));
        assert_eq!(m.page, Page::CampaignBattle);
        assert_eq!(m.campaign_saves.current.as_deref(), Some("Rex"));

        m.open_campaign_saves(SaveMode::Save, SaveReturn::Battle, true);
        press(&mut m, |i| i.up = true);
        assert_eq!(m.stack().unwrap().focus, 0);
        assert_eq!(press(&mut m, |i| i.confirm = true), None);
        assert_eq!(m.page, Page::CampaignBattle);
    }

    #[test]
    fn a_save_over_an_existing_one_asks_first_and_defaults_to_no() {
        let mut m = menus(&["Rex", "Cody"]);
        m.campaign_saves.current = Some("Cody".into());
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Menu, false);
        assert_eq!(m.campaign_saves.selected_name(), Some("Cody"));
        assert_eq!(
            press(&mut m, |i| i.confirm = true),
            Some(ShellCommand::EditCampaignSaveName)
        );
        assert_eq!(m.campaign_save_named("Cody2".into()), None);
        assert_eq!(m.campaign_saves.step, SaveStep::Overwrite);
        assert_eq!(m.stack().unwrap().focus, 1);
        assert_eq!(press(&mut m, |i| i.confirm = true), None);
        assert_eq!(m.campaign_saves.step, SaveStep::List);
        assert_eq!(m.campaign_saves.pending, None);

        press(&mut m, |i| i.confirm = true);
        m.campaign_save_named("Cody".into());
        press(&mut m, |i| i.up = true);
        assert_eq!(
            press(&mut m, |i| i.confirm = true),
            Some(ShellCommand::WriteCampaignSave)
        );
        assert_eq!(
            m.campaign_saves
                .pending
                .as_ref()
                .unwrap()
                .replace
                .as_deref(),
            Some("Cody")
        );

        let mut m = menus(&["Rex"]);
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Menu, false);
        press(&mut m, |i| i.confirm = true);
        assert_eq!(m.campaign_save_named("rex".into()), None);
        assert_eq!(m.campaign_saves.step, SaveStep::Overwrite);
    }

    #[test]
    fn save_as_new_on_a_full_list_shows_the_list_full_popup() {
        let names: Vec<String> = (0..MAX_SAVES).map(|i| format!("S{i}")).collect();
        let mut m = ShellMenus::new(&|_, english| english.into());
        m.set_campaign_saves(names, Vec::new());
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Battle, false);
        assert_eq!(press(&mut m, |i| i.confirm = true), None);
        assert_eq!(m.campaign_saves.step, SaveStep::ListFull);
        press(&mut m, |i| i.back = true);
        assert_eq!(m.campaign_saves.step, SaveStep::List);
        press(&mut m, |i| i.down = true);
        assert_eq!(
            press(&mut m, |i| i.confirm = true),
            Some(ShellCommand::EditCampaignSaveName)
        );
    }

    #[test]
    fn load_lists_saves_deletes_after_asking_and_shows_no_games() {
        let mut m = menus(&[]);
        m.open_campaign_saves(SaveMode::Load, SaveReturn::NewLoad, false);
        let rows = &m.stack().unwrap().rows;
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].enabled);
        assert_eq!(
            m.campaign_saves.no_games(),
            Some("No Saved Games Available")
        );
        assert_eq!(m.movie(), super::super::MOVIE_SUB);
        assert_eq!(press(&mut m, |i| i.confirm = true), None);
        assert_eq!(m.page, Page::RiseNewLoad);

        let mut m = menus(&["Rex", "Cody"]);
        m.open_campaign_saves(SaveMode::Load, SaveReturn::Menu, true);
        assert_eq!(m.campaign_saves.step, SaveStep::ConfirmLoad);
        assert_eq!(m.movie(), "");
        press(&mut m, |i| i.confirm = true);
        press(&mut m, |i| i.down = true);
        assert_eq!(m.campaign_saves.selected_name(), Some("Cody"));
        assert_eq!(m.campaign_saves.accept_help(), "Load");
        assert_eq!(m.campaign_saves.delete_help(), None);
        m.campaign_save_delete();
        assert_eq!(m.campaign_saves.step, SaveStep::List);

        let mut m = menus(&["Rex", "Cody"]);
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Menu, false);
        press(&mut m, |i| i.down = true);
        press(&mut m, |i| i.down = true);
        assert_eq!(m.campaign_saves.selected_name(), Some("Cody"));
        assert_eq!(m.campaign_saves.accept_help(), "Save");
        assert_eq!(m.campaign_saves.delete_help(), Some("Delete"));
        m.campaign_save_delete();
        assert_eq!(m.campaign_saves.step, SaveStep::Delete);
        assert!(!m.campaign_saves.list_shown());
        assert_eq!(press(&mut m, |i| i.back = true), None);
        assert_eq!(
            press(&mut m, |i| {
                i.up = true;
                i.confirm = true
            }),
            Some(ShellCommand::DeleteCampaignSave)
        );
        m.set_campaign_saves(vec!["Rex".into()], Vec::new());
        m.campaign_save_deleted();
        m.open_campaign_saves(SaveMode::Load, SaveReturn::Menu, false);
        assert_eq!(
            press(&mut m, |i| i.confirm = true),
            Some(ShellCommand::LoadCampaignSave)
        );
        assert_eq!(m.campaign_saves.selected_name(), Some("Rex"));
        m.set_campaign_unlocked(18);
        m.campaign_save_loaded(4);
        assert_eq!(m.page, Page::CampaignBattle);
        assert_eq!(m.campaign_selected, 4);
        assert_eq!(m.campaign_saves.current.as_deref(), Some("Rex"));
    }

    #[test]
    fn save_as_new_cannot_be_deleted() {
        let mut m = menus(&["Rex"]);
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Menu, false);
        assert_eq!(m.campaign_saves.delete_help(), None);
        m.campaign_save_delete();
        assert_eq!(m.campaign_saves.step, SaveStep::List);
    }

    fn small_font() -> HudFont {
        HudFont {
            name: "gamefont_small".into(),
            line_height: 18,
            pages: vec!["p".into()],
            glyphs: (32..127)
                .map(|code| manifest::hud::HudGlyph {
                    code,
                    page: 0,
                    advance: 9,
                    x0: 0,
                    x1: 9,
                    y0: 0,
                    y1: 18,
                    uv: [0.0; 4],
                })
                .collect(),
        }
    }

    fn window_point([x0, y0, x1, y1]: [f32; 4]) -> [f32; 2] {
        [(x0 + x1) * 0.5 * 2.5, (y0 + y1) * 0.5 * (960.0 / 448.0)]
    }

    #[test]
    fn the_list_window_is_the_scripts_button_window() {
        assert_eq!(LIST_ROW, 40.0);
        assert_eq!(LIST_SHOWN, (SAFE[3] * 0.75 / LIST_ROW).floor() as usize);
        let [x0, y0, x1, y1] = LIST_WINDOW;
        assert!((x1 - x0 - 414.72).abs() < 1e-3);
        assert_eq!(y1 - y0, 310.0);
        assert_eq!((x0 + x1) * 0.5, 256.0);
        assert_eq!((y0 + y1) * 0.5, 214.0);
        assert_eq!(CURSOR[1], 36.0);
        assert_eq!(TEXT_X, 47.0);
    }

    #[test]
    fn every_save_popup_keeps_its_buttons_inside_its_frame() {
        let font = small_font();
        let mut m = menus(&["Rex"]);
        for (mode, prompt, step) in [
            (SaveMode::Save, true, SaveStep::Prompt),
            (SaveMode::Load, true, SaveStep::ConfirmLoad),
            (SaveMode::Save, false, SaveStep::ListFull),
            (SaveMode::Save, false, SaveStep::Overwrite),
            (SaveMode::Save, false, SaveStep::Delete),
        ] {
            m.open_campaign_saves(mode, SaveReturn::Battle, prompt);
            m.saves_step_to(step);
            let p = m.campaign_saves.popup_layout(&font).unwrap();
            let [fx0, fy0, fx1, fy1] = p.frame;
            assert!(!p.buttons.is_empty());
            for [x0, y0, x1, y1] in &p.buttons {
                assert!(
                    *x0 > fx0 && *x1 < fx1,
                    "{step:?} {:?} {:?}",
                    p.frame,
                    p.buttons
                );
                assert!(
                    *y0 > fy0 && *y1 < fy1,
                    "{step:?} {:?} {:?}",
                    p.frame,
                    p.buttons
                );
            }
            let text_bottom = p.text_top + p.lines.len() as f32 * BODY.line;
            assert!(
                p.text_top > fy0 && text_bottom < p.buttons[0][1],
                "{step:?}"
            );
            let focus = m.campaign_saves.stack.focus;
            let point = window_point(p.buttons[focus]);
            assert_eq!(row_at(&font, &m, point, [1280.0, 960.0]), Some(focus));
        }
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Battle, true);
        let p = m.campaign_saves.popup_layout(&font).unwrap();
        let label = crate::typography::width(&font, &BODY, "Continue Without Saving");
        assert_eq!(p.frame[2] - p.frame[0], label + BAR_PAD + 20.0 + 32.0);
    }

    #[test]
    fn the_list_opens_on_the_current_save_and_scrolls_a_row_at_a_time() {
        let names: Vec<String> = (0..12).map(|i| format!("S{i}")).collect();
        let dates: Vec<String> = (0..12)
            .map(|i| format!("2026-11-{i:02} 12:00.00"))
            .collect();
        let mut m = ShellMenus::new(&|_, english| english.into());
        m.set_campaign_saves(names, dates);
        m.campaign_saves.current = Some("S3".into());
        m.open_campaign_saves(SaveMode::Load, SaveReturn::Menu, false);
        assert_eq!(m.campaign_saves.first, 3);
        assert_eq!(
            m.campaign_saves.row(3),
            Some(("S3", "2026-11-03 12:00.00", true))
        );
        for _ in 0..7 {
            press(&mut m, |i| i.down = true);
        }
        assert_eq!(m.campaign_saves.selected, 10);
        assert_eq!(m.campaign_saves.first, 4);
        let s = &m.campaign_saves;
        assert_eq!(s.stack.rows[s.stack.focus].id, RowId::SaveSlot(10));
        m.open_campaign_saves(SaveMode::Save, SaveReturn::Menu, false);
        assert_eq!(
            m.campaign_saves.row(0),
            Some(("< Save as New >", "", false))
        );
        let font = small_font();
        let point = window_point(CampaignSaves::list_row(1));
        assert_eq!(row_at(&font, &m, point, [1280.0, 960.0]), Some(1));
    }
}
