//! Rise of the Empire's saves: the shell's save screen
//! (`ui::shell_ps2::campaign_saves`) asks, the profile keeps them
//! (`game::profile::CampaignProgress::saves`), and the save name is typed on
//! the profile picker's keyboard (`ifs_campaign_load` main.f32 pc 129-173
//! pushes `ifs_vkeyboard`).

use super::profiles::{step_name_keyboard, FrameInput, NameKeyAction, NameKeys};
use super::{App, FrontShell};
use ui::{
    shell_feel::MenuSound,
    shell_ps2::{
        campaign_saves::NAME_LIMIT,
        profiles::{Page, Picker, Target},
        ShellCommand,
    },
};

/// A save's date as the list's second line shows it: "2026-11-15 12:35.58"
/// ([M] the PS2's Load Game frame: the card file's date, `datestr`,
/// `ifs_campaign_load` main.f1 pc 42-53), in local time. Empty for a save
/// from before saves had dates.
fn save_date(secs: u64) -> String {
    if secs == 0 {
        return String::new();
    }
    let [y, mo, d, h, mi, s] = local_time(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}.{s:02}")
}

/// Seconds since 1970 as the local year, month, day, hour, minute, second:
/// kernel32 turns the FILETIME (100 ns ticks since 1601, two u32s laid out
/// as one little-endian u64) into UTC, then into the local zone of that
/// date.
#[cfg(windows)]
fn local_time(secs: u64) -> [u32; 6] {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn FileTimeToSystemTime(file_time: *const u64, out: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            zone: *const std::ffi::c_void,
            utc: *const SystemTime,
            out: *mut SystemTime,
        ) -> i32;
    }
    // A corrupt save's timestamp must not overflow the 100 ns tick count:
    // fall back to the UTC civil date when it cannot be represented.
    let Some(ticks) = secs
        .checked_add(11_644_473_600)
        .and_then(|secs| secs.checked_mul(10_000_000))
    else {
        return utc_time(secs);
    };
    let (mut utc, mut local) = (SystemTime::default(), SystemTime::default());
    let ok = unsafe {
        FileTimeToSystemTime(&ticks, &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) != 0
    };
    if !ok {
        return utc_time(secs);
    }
    [
        local.year,
        local.month,
        local.day,
        local.hour,
        local.minute,
        local.second,
    ]
    .map(u32::from)
}

/// OURS: UTC off Windows. trace: the platform's local time zone.
#[cfg(not(windows))]
fn local_time(secs: u64) -> [u32; 6] {
    utc_time(secs)
}

/// Seconds since 1970 as the UTC civil date and time (days to a date by the
/// proleptic Gregorian calendar's 400-year cycle).
fn utc_time(secs: u64) -> [u32; 6] {
    let days = (secs / 86_400) as i64;
    let rest = (secs % 86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    [
        year as u32,
        month as u32,
        day as u32,
        rest / 3600,
        rest / 60 % 60,
        rest % 60,
    ]
}

impl App {
    /// The save screen's list as the profile holds it now.
    pub(super) fn sync_campaign_saves(&self, front: &mut FrontShell) {
        let current = front.menus.campaign_saves.current.clone();
        let list = self.profile.campaign.save_list(current.as_deref());
        let names = list.iter().map(|s| s.name.clone()).collect();
        let dates = list.iter().map(|s| save_date(s.saved)).collect();
        front.menus.set_campaign_saves(names, dates);
    }

    /// New Game clears the campaign held in memory, never a save
    /// (`ifs_freeform_rise_newload` main.f2 pc 12-23).
    pub(super) fn start_campaign_new_game(&mut self, front: &mut FrontShell) {
        if self.profile.campaign.unlocked < 2 {
            self.profile.campaign.unlocked = 2;
            if let Err(e) = self.profile.save() {
                eprintln!("campaign profile save failed: {e:#}");
            }
        }
        front.queue_campaign_new_game();
    }

    /// Act on one of the save screen's commands.
    pub(super) fn campaign_save_command(&mut self, front: &mut FrontShell, cmd: ShellCommand) {
        match cmd {
            ShellCommand::EditCampaignSaveName => {
                let name = front
                    .menus
                    .campaign_saves
                    .pending
                    .as_ref()
                    .and_then(|p| p.replace.clone())
                    .unwrap_or_else(|| self.profile.name.clone());
                let mut pick = Picker::new(Vec::new(), None);
                pick.name_title = Some(front.menus.campaign_saves.keyboard_title().to_owned());
                pick.resume_name(name.chars().take(NAME_LIMIT).collect());
                front.save_name = Some(pick);
            }
            ShellCommand::WriteCampaignSave => {
                let Some(write) = front.menus.campaign_saves.pending.clone() else {
                    return;
                };
                let turn = front.menus.campaign_selected as u8;
                let saved = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let before = self.profile.campaign.clone();
                self.profile.campaign.write_save(
                    write.replace.as_deref(),
                    &write.name,
                    turn,
                    saved,
                );
                let result = self.profile.save().map_err(|e| {
                    eprintln!("campaign save failed: {e:#}");
                    "Save failed".to_owned()
                });
                if result.is_err() {
                    self.profile.campaign = before;
                }
                front.menus.campaign_save_written(result);
                self.sync_campaign_saves(front);
            }
            ShellCommand::LoadCampaignSave => {
                let turn = front
                    .menus
                    .campaign_saves
                    .selected_name()
                    .and_then(|name| self.profile.campaign.save_turn(name));
                match turn {
                    Some(turn) => {
                        let unlocked = front.menus.campaign_unlocked.max(turn.saturating_add(1));
                        front.menus.set_campaign_unlocked(unlocked);
                        front.menus.campaign_save_loaded(usize::from(turn));
                        self.sync_campaign_saves(front);
                    }
                    None => front.menus.campaign_save_failed("Load failed".into()),
                }
            }
            ShellCommand::DeleteCampaignSave => {
                let Some(name) = front
                    .menus
                    .campaign_saves
                    .selected_name()
                    .map(str::to_owned)
                else {
                    return;
                };
                let before = self.profile.campaign.clone();
                self.profile.campaign.delete_save(&name);
                if let Err(e) = self.profile.save() {
                    eprintln!("campaign save delete failed: {e:#}");
                    self.profile.campaign = before;
                    front.menus.campaign_save_failed("Delete failed".into());
                    return;
                }
                self.sync_campaign_saves(front);
                front.menus.campaign_save_deleted();
            }
            _ => {}
        }
    }

    /// One frame of the save name keyboard (the profile picker's name page).
    pub(super) fn step_save_name(&mut self, front: &mut FrontShell, frame: FrameInput) {
        let Some(pick) = front.save_name.as_mut() else {
            return;
        };
        match step_name_keyboard(
            &mut front.menus,
            &mut front.name_input,
            pick,
            NameKeys {
                frame: &frame,
                name_limit: NAME_LIMIT,
                renderer: self.renderer.as_ref(),
                list: false,
            },
        ) {
            // The shared helper already applied the symbols toggle and the
            // key / symbols / backspace targets with this screen's cues.
            NameKeyAction::None | NameKeyAction::Symbols => {}
            NameKeyAction::Cancel => {
                front.save_name = None;
                front.menus.campaign_save_name_cancelled();
                front.menus.cues.play(MenuSound::Exit);
            }
            NameKeyAction::Target(Target::Accept) => {
                let Page::Name { text, .. } = &pick.page else {
                    return;
                };
                let name = text.trim_end_matches(' ').to_owned();
                if name.trim().is_empty() {
                    pick.message = Some(front.menus.campaign_saves.too_short().to_owned());
                    front.menus.cues.play(MenuSound::Error);
                    return;
                }
                front.save_name = None;
                front.menus.cues.play(MenuSound::Accept);
                if let Some(cmd) = front.menus.campaign_save_named(name) {
                    self.campaign_save_command(front, cmd);
                }
            }
            NameKeyAction::Target(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::profile::{CampaignSave, Store};
    use std::path::PathBuf;
    use ui::menu::MenuInput;
    use ui::shell_ps2::{
        campaign_saves::{SaveMode, SaveReturn, SaveStep},
        Page as ShellPage,
    };

    #[test]
    fn a_save_date_reads_as_the_ps2_list_writes_it() {
        assert_eq!(utc_time(0), [1970, 1, 1, 0, 0, 0]);
        assert_eq!(utc_time(1_709_251_199), [2024, 2, 29, 23, 59, 59]);
        assert_eq!(utc_time(1_794_746_158), [2026, 11, 15, 12, 35, 58]);
        assert_eq!(save_date(0), "");
        let text = save_date(1_794_746_158);
        assert_eq!(text.len(), "2026-11-15 12:35.58".len());
        assert_eq!(&text[4..5], "-");
        assert_eq!(&text[13..14], ":");
        assert_eq!(&text[16..17], ".");
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "openbf-campaign-saves-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        dir
    }

    fn app_with_saves(tag: &str) -> (App, PathBuf) {
        let dir = temp_dir(tag);
        let store = Store::new(dir.clone());
        let mut app = App::new(
            PathBuf::new(),
            PathBuf::new(),
            super::super::super::tests::setup(None),
            None,
            None,
            None,
            Default::default(),
        );
        app.profile = store
            .create("Pilot", core::skill::Difficulty::Normal)
            .unwrap();
        app.profile.campaign.unlocked = 6;
        app.profile.campaign.training_prompted = true;
        app.profile.campaign.write_save(None, "Pilot", 5, 100);
        app.profile.save().unwrap();
        app.profile_store = Some(store);
        app.front = Some(FrontShell::new(&dir));
        (app, dir)
    }

    fn reread(app: &App) -> Vec<CampaignSave> {
        let store = app.profile_store.as_ref().unwrap();
        let list = store.list().unwrap();
        store.select(list.names[0].0).unwrap().campaign.saves
    }

    fn step(app: &mut App, front: &mut FrontShell, f: impl Fn(&mut MenuInput)) {
        let mut input = MenuInput::default();
        f(&mut input);
        app.sync_campaign_saves(front);
        if let Some(cmd) = front.menus.step(input, false, 0.0) {
            app.campaign_save_command(front, cmd);
        }
    }

    #[test]
    fn training_and_new_game_keep_the_saved_campaign() {
        let (mut app, dir) = app_with_saves("keep");
        let saved = reread(&app);
        app.finish_campaign_round(0, true, true, false);
        assert_eq!(reread(&app), saved);
        app.finish_campaign_round(0, false, false, false);
        assert_eq!(reread(&app), saved);

        let mut front = app.front.take().unwrap();
        front.menus.open_page(ShellPage::RiseNewLoad);
        app.start_campaign_new_game(&mut front);
        assert_eq!(front.menus.campaign_selected, 1);
        assert_eq!(reread(&app), saved);
        assert_eq!(app.profile.campaign.unlocked, 6);

        front.menus.open_page(ShellPage::RiseNewLoad);
        step(&mut app, &mut front, |i| i.down = true);
        step(&mut app, &mut front, |i| i.confirm = true);
        assert_eq!(front.menus.page, ShellPage::CampaignSaves);
        assert_eq!(front.menus.campaign_saves.mode, SaveMode::Load);
        step(&mut app, &mut front, |i| i.confirm = true);
        assert_eq!(front.menus.page, ShellPage::CampaignBattle);
        assert_eq!(front.menus.campaign_selected, 5);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_finished_battle_asks_to_save_into_a_slot() {
        let (mut app, dir) = app_with_saves("prompt");
        app.finish_campaign_round(2, true, true, true);
        let mut front = app.front.take().unwrap();
        front.cutscenes.clear();
        assert_eq!(front.menus.page, ShellPage::CampaignSaves);
        assert_eq!(front.menus.campaign_saves.step, SaveStep::Prompt);
        assert_eq!(front.menus.campaign_saves.back_to, SaveReturn::Battle);
        assert_eq!(front.menus.campaign_selected, 3);

        step(&mut app, &mut front, |i| i.confirm = true);
        assert_eq!(front.menus.campaign_saves.step, SaveStep::List);
        step(&mut app, &mut front, |i| i.confirm = true);
        let pick = front.save_name.as_ref().expect("the name keyboard");
        assert!(matches!(&pick.page, Page::Name { text, .. } if text == "Pilot"));
        front.name_input.text = "2".into();
        app.step_save_name(
            &mut front,
            FrameInput {
                start: true,
                ..Default::default()
            },
        );
        assert!(front.save_name.is_none());
        assert_eq!(front.menus.page, ShellPage::CampaignBattle);
        let saves = reread(&app);
        assert_eq!(saves.len(), 2);
        assert!(saves.iter().any(|s| s.name == "Pilot" && s.turn == 5));
        assert!(saves.iter().any(|s| s.name == "Pilot2" && s.turn == 3));
        app.sync_campaign_saves(&mut front);
        assert_eq!(front.menus.campaign_saves.names, ["Pilot2", "Pilot"]);

        app.front = Some(front);
        app.finish_campaign_round(3, false, true, true);
        let mut front = app.front.take().unwrap();
        assert_eq!(front.menus.campaign_saves.step, SaveStep::Prompt);
        step(&mut app, &mut front, |i| i.up = true);
        step(&mut app, &mut front, |i| i.confirm = true);
        assert_eq!(front.menus.page, ShellPage::CampaignBattle);
        assert_eq!(reread(&app).len(), 2);

        app.front = Some(front);
        app.finish_campaign_round(3, false, false, true);
        let front = app.front.take().unwrap();
        assert_eq!(front.menus.page, ShellPage::CampaignBattle);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
