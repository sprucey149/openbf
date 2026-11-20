//! The BF2 PS2 shell's front screens (card BSHLA, `analysis/re-bf2-shell.md`
//! §2-§4, §9): Press START, the Main Menu, Singleplayer and Options, each
//! the PS2's vertical-button stack over the background movie, painted by the
//! HUD pass (`crate::hud_draw`) in the shell's 512 × 448 screen.
//!
//! - **Title** (`ifs_start`): `logo` at the top, the `shell_main` movie and
//!   "Press START Button To Begin" pulsing; only Start advances (§2 row 3;
//!   the keyboard's Enter is the port's Start). The windowed shell opens
//!   the profile picker here (§9.3). The legal / logo screens are skipped.
//! - **Main Menu** (`ifs_main`): logo + stack 10 units lower than the
//!   others: Singleplayer, Multiplayer (issue #168: LAN play), Splitscreen
//!   disabled (until card SSCR; the PS2's own dimmed style: label grey 110,
//!   focus skips it, §3.3), Options,
//!   then `[ours]` **Quit Game**, which exits. The PC's `ifs_main` ends in a
//!   quit row ("Exit To Windows") that the PS2 hides (`re-bf2-menus.md`,
//!   ifs_main list order); the port shows it with its own label.
//! - **Singleplayer** (`ifs_sp`, left-justified, `shell_sub_left`): Rise of
//!   the Empire and Galactic Conquest disabled (no port mode yet), Instant
//!   Action live; Training and Space Overview hidden (§9.5).
//! - `[ours]` **Multiplayer** (issue #168, `LAN_MULTIPLAYER.md` §4): LAN
//!   Host Game and LAN Join Game live, Online dimmed with
//!   [`TEXT_ONLINE_NOTE`] under the stack (no dedicated server exists).
//!
//! The stack's geometry is the frames' [M] rule (§3.2), not the Lua's
//! left-justify formula (which does not reproduce them, §3.2 [H]): labels
//! 1.087 × the font's advances across, the bar 1.087 × the widest label +
//! [`BAR_PAD`], centred stacks about x [`CENTRE_X`], left-justified ones from
//! x [`LEFT_X`]; rows `font height + 6` ([`crate::spawn_draw::ROW_H`]) every
//! `+ 4` ([`crate::spawn_draw::ROW_PITCH`]); the stack's middle
//! [`CENTRE_LIFT`] above its container's y (0.6 of the screen, + 10 on the
//! main menu). Focus moves wrap last <-> first and skip disabled rows (§3.2,
//! §3.3).
//!
//! How it sounds and moves (`analysis/re-bf2-menus.md` §4.1, §5;
//! [`crate::shell_feel`]): every stack move clicks (the stacks wrap), Cross
//! on a row plays accept, Back plays the exit click of the screen it returns
//! to, Start on the title accept. The focused row's bar and label pulse
//! together ([`Pulse::BUTTON`], 0.734 s, one phase kept across moves); the
//! others sit at alpha 0.6 with a grey 150 bar. Every page entry (forward
//! or back) replays the flashy entrance: the title at once, row k
//! [`ROW_REVEAL`] × k s later. Singleplayer is always entered on its first
//! live row (the PS2's Training, menuref "Singleplayer").
//!
//! `[ours]` Hor+: the movie, the logo and centred stacks keep the 4:3 box at
//! the window's centre (the movie pillarboxed, never stretched: §9.9), a
//! left-justified stack keeps its offset from the box's left edge, the help
//! lines their offsets from the window's edges.

mod campaign;
pub mod campaign_saves;
pub mod profiles;

use crate::hud_draw::Ctx;
use crate::hud_tree::HAlign;
use crate::input_icons::Prompt;
use crate::menu::MenuInput;
use crate::shell_feel::{flashy, Cues, MenuSound, Pulse};
use crate::spawn_draw::{ROW_H, ROW_PITCH};
use crate::typography::{tint, ITEM, TITLE};
use manifest::hud::HudFont;

/// `gDefaultButtonScreenRelativeY` 0.6 of the 448-line screen (§3.1 [V]).
pub const CONTAINER_Y: f32 = 0.6 * 448.0;
/// The main menu's container sits 10 units lower (§3.2 [V]).
pub const MAIN_Y_ADD: f32 = 10.0;
/// The stack's middle stands this far above its container's y [M]: title
/// tops at 206 / 178 / 143.5 px of 480 on the main menu / Singleplayer /
/// Options frames (4, 5, 7 rows) - one offset fits all three to 0.2 units.
pub const CENTRE_LIFT: f32 = 8.7;
/// A centred stack's middle across [M] (main menu bar 143.75..501.25 px of
/// 640 = 115..401 units).
pub const CENTRE_X: f32 = 258.0;
/// A left-justified stack's bar left edge [M] (43 px on both the
/// Singleplayer and the Options frame: the safe area's 25.6 + 8.8).
pub const LEFT_X: f32 = 34.4;
/// The bar's outer width beyond 1.087 × its widest label's advances [M]
/// (§3.2: main menu, Singleplayer and Options agree within 0.9 units).
pub const BAR_PAD: f32 = 134.2;
/// A dimmed button's label grey (`ShowHideVerticalButtons` f7 pc 266-289
/// [V]: 110 instead of 255).
pub use crate::typography::tint::DIMMED_GREY;
/// An unfocused row's bar tint (`FUN_00381858` B2 380175-380178: 150 grey
/// unless the text style is `selected`, re-bf2-menus §5 [V]).
pub const UNFOCUSED_BAR: f32 = 150.0;
/// Seconds between one row's flashy start and the next's (`button` main.f7
/// pc 188, re-bf2-menus §5 [V]; menuref §9.4.5 [M]: ~0.1 s on the PS2).
pub const ROW_REVEAL: f32 = 0.1;
/// The flashy sweep's square (unbaked: drawn untextured until it is, §8).
pub const CURSOR_SHEET: &str = "cursor";
/// The logo: 512 × 256, centred, 38 units above the top of the safe area
/// (22.4, §3.5): §3.4 [V] says "above the top edge", and the frames put it
/// from the safe area's [M] (ink rows 36-147 px of 480 on
/// `title_pressstart.png`; from the screen's edge they land 24 px higher).
pub const LOGO: [f32; 4] = [0.0, 22.4 - 38.0, 512.0, 22.4 - 38.0 + 256.0];
/// The prompt's run top [M] (`title_pressstart.png`: its ink rows 427-441
/// px, a row's ink starts 4.5 units below its run's top), centred.
pub const PROMPT_TOP: f32 = 394.0;
/// The prompt's alpha swings between these at [`PROMPT_RATE`] a second
/// (`ifs_start` main.f2 pc 45-64 [V]).
pub const PROMPT_ALPHA: [f32; 2] = [0.4, 1.0];
pub const PROMPT_RATE: f32 = 0.5;
/// The texture name the movie's current frame is drawn from.
pub const MOVIE_SHEET: &str = "#shell_movie";

/// The movie properties (`manifest::shell`).
pub const MOVIE_MAIN: &str = "shell_main";
pub const MOVIE_SUB: &str = "shell_sub_left";
/// The music every front screen plays (`ifs_start` main.f2 sets
/// `shell_soundtrack`, §2 [V]).
pub const MUSIC: &str = "shell_soundtrack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Title,
    Main,
    Singleplayer,
    AskTraining,
    RiseNewLoad,
    RiseOfTheEmpire,
    CampaignBattle,
    CampaignMenu,
    CampaignSaves,
    Options,
    /// `[ours]` issue #168.
    Multiplayer,
    /// `[ours]` the end-of-campaign credits crawl. The game draws the crawl
    /// (no stack, no shell movie); any skip input leaves for Singleplayer.
    Credits,
}

/// Every button the port shows; the label is the disc's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowId {
    Singleplayer,
    Multiplayer,
    Splitscreen,
    Options,
    QuitGame,
    Training,
    RiseOfTheEmpire,
    GalacticConquest,
    InstantAction,
    AskTrainingPlay,
    AskTrainingSkip,
    CampaignNewGame,
    CampaignLoad,
    CampaignMission(u8),
    CampaignToggleAll,
    CampaignPlay,
    CampaignBack,
    CampaignResume,
    CampaignOptions,
    CampaignSave,
    CampaignLoadMenu,
    CampaignQuit,
    CampaignQuitYes,
    CampaignQuitNo,
    SaveAsNew,
    SaveSlot(u8),
    NoSavedGames,
    SaveContinue,
    SaveConfirm,
    LoadCancel,
    LoadContinue,
    SaveYes,
    SaveNo,
    SaveOk,
    GameOptions,
    AudioOptions,
    GraphicsOptions,
    Controls,
    /// `[ours]` Multiplayer's rows (issue #168).
    LanHost,
    LanJoin,
    Online,
}

impl RowId {
    /// The disc's text key and its English fallback (`re-bf2-shell` §2,
    /// `bshl/keys_resolved.tsv`).
    pub fn key(self) -> (&'static str, &'static str) {
        match self {
            RowId::Singleplayer => ("ifs.main.sp", "Singleplayer"),
            RowId::Multiplayer => ("ifs.main.mp", "Multiplayer"),
            RowId::Splitscreen => ("ifs.main.split", "Splitscreen"),
            RowId::Options => ("ifs.main.options", "Options"),
            // Our own key: the disc's `common.quit2windows` reads "Exit To Windows".
            RowId::QuitGame => ("ifs.main.quitgame", "Quit Game"),
            RowId::Training => ("ifs.sp.training", "Training"),
            RowId::RiseOfTheEmpire => ("ifs.sp.riseempire", "Rise of the Empire"),
            RowId::GalacticConquest => ("ifs.sp.meta", "Galactic Conquest"),
            RowId::InstantAction => ("ifs.sp.ia", "Instant Action"),
            RowId::AskTrainingPlay => ("ifs.sp.play", "Play"),
            RowId::AskTrainingSkip => ("ifs.sp.skip", "Skip"),
            RowId::CampaignNewGame => ("ifs.meta.load.new", "New Game"),
            RowId::CampaignLoad => ("ifs.meta.load.btnload", "Load"),
            RowId::CampaignMission(i) => (
                "openbf.campaign.mission",
                crate::campaign::RISE_OF_THE_EMPIRE[i as usize].title,
            ),
            RowId::CampaignToggleAll => ("openbf.campaign.toggle_all", "View All Missions"),
            RowId::CampaignPlay => ("ifs.freeform.attack", "Attack"),
            RowId::CampaignBack => ("common.back", "Back"),
            RowId::CampaignResume => ("common.resume", "Resume Game"),
            RowId::CampaignOptions => ("ifs.main.options", "Options"),
            RowId::CampaignSave => ("common.save", "Save Game"),
            RowId::CampaignLoadMenu => ("common.load", "Load Game"),
            RowId::CampaignQuit => ("common.quit", "Quit"),
            RowId::CampaignQuitYes => ("common.yes", "Yes"),
            RowId::CampaignQuitNo => ("common.no", "No"),
            RowId::SaveAsNew => ("ifs.meta.load.saveasnew", "< Save as New >"),
            RowId::SaveSlot(_) => ("ifs.meta.load.save", "Save Game"),
            RowId::NoSavedGames => ("ifs.meta.load.nogames", "No Saved Games Available"),
            RowId::SaveContinue => ("ifs.loadsave_ps2.continuenosave", "Continue Without Saving"),
            RowId::SaveConfirm => ("ifs.loadsave_ps2.save", "Save"),
            RowId::LoadCancel => ("ifs.loadsave_ps2.cancel", "Cancel"),
            RowId::LoadContinue => ("ifs.loadsave_ps2.continue", "Continue"),
            RowId::SaveYes => ("common.yes", "Yes"),
            RowId::SaveNo => ("common.no", "No"),
            RowId::SaveOk => ("common.ok", "OK"),
            RowId::GameOptions => ("ifs.GameOpt.title", "Game Options"),
            RowId::AudioOptions => ("ifs.SoundOpt.title", "Audio Options"),
            RowId::GraphicsOptions => ("openbf.graphicsoptions", "Graphics Options"),
            RowId::Controls => ("ifs.controls.Vehicle_Unit", "Controls"),
            // Our own keys (issue #168).
            RowId::LanHost => ("openbf.mp.lanhost", "LAN: Host Game"),
            RowId::LanJoin => ("openbf.mp.lanjoin", "LAN: Join Game"),
            RowId::Online => ("openbf.mp.online", "Online"),
        }
    }
}

/// The other texts the screens show: (key, English).
pub const TEXT_PROMPT: (&str, &str) = ("ifs.start.press_start", "Press START Button To Begin");
pub const TEXT_MAIN: (&str, &str) = ("ifs.main.title", "Main Menu");
pub const TEXT_BACK: (&str, &str) = ("common.back", "Back");
pub const TEXT_SELECT: (&str, &str) = ("common.select", "Select");
/// Why the Multiplayer page's Online row is dimmed (issue #168).
pub const TEXT_ONLINE_NOTE: (&str, &str) = (
    "openbf.mp.online_note",
    "Online play needs a dedicated server \u{2014} not available yet",
);

#[derive(Debug, Clone, PartialEq)]
pub struct StackRow {
    pub id: RowId,
    pub label: String,
    /// `false` = the PS2's `bDimmed`: drawn grey, never focused.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub title: String,
    pub rows: Vec<StackRow>,
    pub focus: usize,
    pub left: bool,
    /// The container's extra y (the main menu's 10).
    pub y_add: f32,
    /// `[ours]` a line drawn under the stack (Multiplayer's Online note).
    pub note: Option<String>,
}

impl Stack {
    /// The next enabled row from `focus` going `step` (+1 down, -1 up),
    /// wrapping (`FollowButtonLink`, §3.3); `focus` when none other is.
    pub fn follow(&self, step: isize) -> usize {
        let n = self.rows.len() as isize;
        let mut k = self.focus as isize;
        for _ in 0..n {
            k = (k + step).rem_euclid(n);
            if self.rows[k as usize].enabled {
                return k as usize;
            }
        }
        self.focus
    }

    /// The first enabled row (`TagOfFirst` skips dimmed ones).
    pub fn first(&self) -> usize {
        self.rows.iter().position(|r| r.enabled).unwrap_or(0)
    }
}

/// OURS: the save list's Delete help text one help line above Select. The
/// disc puts it 40 above the screen's bottom, right-justified
/// (`ifs_campaign_load` main.f36 pc 113-121). measure: the PS2's help line
/// heights on the save list's frame.
const DELETE_HELP_LIFT: f32 = 25.0;

/// What a step asks the game to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCommand {
    /// Singleplayer → Instant Action: today's IA setup (until card BSHLB).
    InstantAction,
    /// Rise of the Empire → New Game: plays opening crawl (`crawl1`) and opens Turn 1 planet briefing.
    StartCampaignNewGame,
    /// The save screen's name keyboard, for [`campaign_saves::CampaignSaves::pending`].
    EditCampaignSaveName,
    /// Write [`campaign_saves::CampaignSaves::pending`] with the current turn.
    WriteCampaignSave,
    /// Read the focused save ([`campaign_saves::CampaignSaves::selected_name`]).
    LoadCampaignSave,
    /// Delete the focused save.
    DeleteCampaignSave,
    /// Rise of the Empire → Launch mission index (`0..18`).
    LaunchCampaignMission {
        index: usize,
    },
    Settings(crate::menu::SettingsPage),
    /// Multiplayer → LAN: Host Game / Join Game (issue #168).
    HostGame,
    JoinGame,
    /// Back on the title screen, or the main menu's Quit Game row (`[ours]`:
    /// the PS2 has no way out).
    Quit,
}

/// The shell's screens and where each one's focus is.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellMenus {
    pub page: Page,
    pub main: Stack,
    pub singleplayer: Stack,
    pub ask_training: Stack,
    pub rise_new_load: Stack,
    pub campaign: Stack,
    pub campaign_battle: Stack,
    pub campaign_menu: Stack,
    pub campaign_quit_popup: Stack,
    pub campaign_saves: campaign_saves::CampaignSaves,
    pub options: Stack,
    pub multiplayer: Stack,
    pub campaign_unlocked: u8,
    pub campaign_selected: usize,
    pub campaign_view_all: bool,
    pub campaign_focus_toggle: bool,
    pub campaign_quit_confirm: bool,
    pub campaign_pause_options: bool,
    pub training_prompted: bool,
    ask_training_prompt: String,
    campaign_quit_prompt: String,
    training_label: String,
    campaign_title: String,
    campaign_launch_label: String,
    campaign_planets: Vec<String>,
    campaign_descs: Vec<String>,
    /// The campaign list's localizable texts, looked up once in [`Self::new`]
    /// through `text(key, English)`. They live here so the list can be
    /// rebuilt on a profile change without the lookup closure.
    campaign_show_unlocked: String,
    campaign_view_all_text: String,
    campaign_showing_fmt: String,
    campaign_mission_note_fmt: String,
    pub prompt: String,
    pub back: String,
    pub select: String,
    /// Seconds since the page was entered (the reveal) and since the shell
    /// started (the prompt's pulse).
    pub on_page: f32,
    pub clock: f32,
    /// A page of the port's own is up over the screen (the Instant Action
    /// setup, the settings): only the movie draws.
    pub covered: bool,
    /// The clicks asked for, for the game to play.
    pub cues: Cues,
    /// The focused button's pulse (one phase, shared by every screen; a
    /// list's cursor box does not pulse, menuref §9.1.9).
    pub pulse: Pulse,
}

impl ShellMenus {
    /// The screens, their texts looked up by `text(key, English)`.
    pub fn new(text: &dyn Fn(&str, &str) -> String) -> ShellMenus {
        let row = |id: RowId, enabled: bool| {
            let (k, e) = id.key();
            StackRow {
                id,
                label: text(k, e),
                enabled,
            }
        };
        let stack = |title: String, rows: Vec<StackRow>, left: bool, y_add: f32| {
            let mut s = Stack {
                title,
                rows,
                focus: 0,
                left,
                y_add,
                note: None,
            };
            s.focus = s.first();
            s
        };
        let (k, e) = RowId::Singleplayer.key();
        let sp_title = text(k, e);
        let (k, e) = RowId::RiseOfTheEmpire.key();
        let campaign_title = text(k, e);
        let (k, e) = RowId::Options.key();
        let opt_title = text(k, e);
        let (k, e) = RowId::Multiplayer.key();
        let mut multiplayer = stack(
            text(k, e),
            vec![
                row(RowId::LanHost, true),
                row(RowId::LanJoin, true),
                row(RowId::Online, false),
            ],
            true,
            0.0,
        );
        multiplayer.note = Some(text(TEXT_ONLINE_NOTE.0, TEXT_ONLINE_NOTE.1));
        let campaign_planets = crate::campaign::RISE_OF_THE_EMPIRE
            .iter()
            .map(|e| text(e.planet_key, e.title))
            .collect();
        let campaign_descs = crate::campaign::RISE_OF_THE_EMPIRE
            .iter()
            .map(|e| {
                let raw = text(e.desc_key, e.desc_fallback);
                let joined = raw.split_whitespace().collect::<Vec<_>>().join(" ");
                if joined.is_empty() {
                    e.desc_fallback.to_owned()
                } else {
                    joined
                }
            })
            .collect();
        let campaign_show_unlocked = text(
            campaign::TEXT_SHOW_UNLOCKED.0,
            campaign::TEXT_SHOW_UNLOCKED.1,
        );
        let campaign_view_all_text = text(campaign::TEXT_VIEW_ALL.0, campaign::TEXT_VIEW_ALL.1);
        let campaign_showing_fmt = text(campaign::TEXT_SHOWING.0, campaign::TEXT_SHOWING.1);
        let campaign_mission_note_fmt =
            text(campaign::TEXT_MISSION_NOTE.0, campaign::TEXT_MISSION_NOTE.1);
        let back_label = text(TEXT_BACK.0, TEXT_BACK.1);
        let training_label = text("ifs.sp.training", "Training");
        let ask_training_prompt = text(
            "ifs.sp.asktraining",
            "Do you want to play the training mission first?",
        );
        let campaign_quit_prompt = text(
            "ifs.pause.warn_quit",
            "Leaving the game now will end this session. Are you sure you want to leave?",
        );
        let launch_label = text("ifs.freeform.attack", "Attack");
        let pause_title = text("game.pause.title", "Game Menu");
        let mut menus = ShellMenus {
            page: Page::Title,
            main: stack(
                text(TEXT_MAIN.0, TEXT_MAIN.1),
                vec![
                    row(RowId::Singleplayer, true),
                    row(RowId::Multiplayer, true),
                    row(RowId::Splitscreen, false),
                    row(RowId::Options, true),
                    row(RowId::QuitGame, true),
                ],
                false,
                MAIN_Y_ADD,
            ),
            singleplayer: stack(
                sp_title,
                vec![
                    row(RowId::RiseOfTheEmpire, true),
                    row(RowId::GalacticConquest, false),
                    row(RowId::InstantAction, true),
                ],
                true,
                0.0,
            ),
            ask_training: stack(
                String::new(),
                vec![
                    row(RowId::AskTrainingPlay, true),
                    row(RowId::AskTrainingSkip, true),
                ],
                false,
                -32.0,
            ),
            rise_new_load: stack(
                campaign_title.clone(),
                vec![
                    row(RowId::CampaignNewGame, true),
                    row(RowId::CampaignLoad, true),
                ],
                true,
                -44.8,
            ),
            campaign: stack(campaign_title.clone(), Vec::new(), true, 0.0),
            campaign_battle: stack(
                String::new(),
                vec![
                    StackRow {
                        id: RowId::CampaignPlay,
                        label: launch_label.clone(),
                        enabled: true,
                    },
                    StackRow {
                        id: RowId::CampaignBack,
                        label: back_label.clone(),
                        enabled: true,
                    },
                ],
                true,
                0.0,
            ),
            campaign_menu: stack(
                pause_title,
                vec![
                    row(RowId::CampaignResume, true),
                    row(RowId::CampaignOptions, true),
                    row(RowId::CampaignSave, true),
                    row(RowId::CampaignLoadMenu, true),
                    row(RowId::CampaignQuit, true),
                ],
                false,
                0.0,
            ),
            campaign_quit_popup: stack(
                String::new(),
                vec![
                    row(RowId::CampaignQuitYes, true),
                    row(RowId::CampaignQuitNo, true),
                ],
                false,
                -32.0,
            ),
            campaign_saves: campaign_saves::CampaignSaves::new(text),
            options: stack(
                opt_title,
                vec![
                    row(RowId::GameOptions, true),
                    row(RowId::AudioOptions, true),
                    row(RowId::GraphicsOptions, true),
                    row(RowId::Controls, true),
                ],
                true,
                0.0,
            ),
            multiplayer,
            campaign_unlocked: 1,
            campaign_selected: 0,
            campaign_view_all: false,
            campaign_focus_toggle: false,
            campaign_quit_confirm: false,
            campaign_pause_options: false,
            training_prompted: false,
            ask_training_prompt,
            campaign_quit_prompt,
            training_label,
            campaign_title,
            campaign_launch_label: launch_label,
            campaign_planets,
            campaign_descs,
            campaign_show_unlocked,
            campaign_view_all_text,
            campaign_showing_fmt,
            campaign_mission_note_fmt,
            prompt: text(TEXT_PROMPT.0, TEXT_PROMPT.1),
            back: back_label,
            select: text(TEXT_SELECT.0, TEXT_SELECT.1),
            on_page: 0.0,
            clock: 0.0,
            covered: false,
            cues: Cues::default(),
            pulse: Pulse::BUTTON,
        };
        menus.rebuild_campaign_stacks();
        menus
    }

    pub fn training_prompted(&self) -> bool {
        self.training_prompted
    }

    pub fn set_training_prompted(&mut self, prompted: bool) {
        if self.training_prompted == prompted {
            return;
        }
        self.training_prompted = prompted;
        let has_training = self
            .singleplayer
            .rows
            .first()
            .is_some_and(|r| r.id == RowId::Training);
        if prompted && !has_training {
            self.singleplayer.rows.insert(
                0,
                StackRow {
                    id: RowId::Training,
                    label: self.training_label.clone(),
                    enabled: true,
                },
            );
            self.singleplayer.focus = self.singleplayer.first();
        } else if !prompted && has_training {
            self.singleplayer.rows.remove(0);
            self.singleplayer.focus = self.singleplayer.first();
        }
    }

    pub fn stack(&self) -> Option<&Stack> {
        match self.page {
            Page::Title | Page::CampaignBattle | Page::Credits => None,
            Page::Main => Some(&self.main),
            Page::Singleplayer => Some(&self.singleplayer),
            Page::AskTraining => Some(&self.ask_training),
            Page::RiseNewLoad => Some(&self.rise_new_load),
            Page::RiseOfTheEmpire => Some(&self.campaign),
            Page::CampaignMenu => Some(if self.campaign_quit_confirm {
                &self.campaign_quit_popup
            } else {
                &self.campaign_menu
            }),
            Page::CampaignSaves => Some(&self.campaign_saves.stack),
            Page::Options => Some(&self.options),
            Page::Multiplayer => Some(&self.multiplayer),
        }
    }

    fn stack_mut(&mut self) -> Option<&mut Stack> {
        match self.page {
            Page::Title | Page::CampaignBattle | Page::Credits => None,
            Page::Main => Some(&mut self.main),
            Page::Singleplayer => Some(&mut self.singleplayer),
            Page::AskTraining => Some(&mut self.ask_training),
            Page::RiseNewLoad => Some(&mut self.rise_new_load),
            Page::RiseOfTheEmpire => Some(&mut self.campaign),
            Page::CampaignMenu => Some(if self.campaign_quit_confirm {
                &mut self.campaign_quit_popup
            } else {
                &mut self.campaign_menu
            }),
            Page::CampaignSaves => Some(&mut self.campaign_saves.stack),
            Page::Options => Some(&mut self.options),
            Page::Multiplayer => Some(&mut self.multiplayer),
        }
    }

    /// Move focus to an enabled row; disabled and missing rows are ignored.
    pub fn focus_row(&mut self, row: usize) -> bool {
        let Some(stack) = self.stack_mut() else {
            return false;
        };
        if !stack.rows.get(row).is_some_and(|r| r.enabled) {
            return false;
        }
        let id = stack.rows[row].id;
        stack.focus = row;
        if self.page == Page::CampaignSaves {
            let s = &mut self.campaign_saves;
            if s.step == campaign_saves::SaveStep::List {
                let first = usize::from(s.mode == campaign_saves::SaveMode::Save);
                s.selected = match id {
                    RowId::SaveSlot(i) => first + i as usize,
                    _ => 0,
                };
                s.rebuild();
            }
        }
        if self.page == Page::RiseOfTheEmpire {
            match id {
                RowId::CampaignMission(i) => {
                    self.campaign_selected = i as usize;
                    self.campaign_focus_toggle = false;
                    self.rebuild_campaign_stacks();
                }
                RowId::CampaignToggleAll => {
                    self.campaign_focus_toggle = true;
                    self.rebuild_campaign_stacks();
                }
                _ => {}
            }
        }
        true
    }

    /// Open `page`, resetting reveal animation and focus as needed.
    pub fn open_page(&mut self, page: Page) {
        self.go(page);
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.on_page = 0.0;
        if page != Page::CampaignMenu {
            self.campaign_quit_confirm = false;
        }
        // Back on Singleplayer the focus is its first row (Training on the
        // PS2) whatever was left (menuref "Singleplayer", `ia_enter_back`).
        if page == Page::Main {
            self.campaign_pause_options = false;
        } else if page == Page::Singleplayer {
            self.campaign_pause_options = false;
            self.singleplayer.focus = self.singleplayer.first();
        } else if page == Page::AskTraining {
            self.ask_training.focus = self.ask_training.first();
        } else if page == Page::RiseNewLoad {
            self.rise_new_load.focus = self.rise_new_load.first();
        } else if page == Page::CampaignMenu {
            self.campaign_menu.note = None;
        } else if matches!(page, Page::RiseOfTheEmpire | Page::CampaignBattle) {
            self.rebuild_campaign_stacks();
        }
    }

    /// The clocks only: a frame of `dt` while a page of the port's own
    /// covers the screen (its pulses and click limit keep running).
    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        self.on_page += dt;
        self.cues.tick(dt);
        self.pulse.advance(dt);
    }

    /// The page over the screen closed: this screen is entered again, so it
    /// plays its exit click and its entrance (template Enter main.f10 pc 48,
    /// re-bf2-menus §4.1 [V]).
    pub fn uncover(&mut self) {
        self.go(self.page);
        self.cues.play(MenuSound::Exit);
    }

    /// One frame: `dt` seconds, the menu edges and Start's.
    pub fn step(&mut self, input: MenuInput, start: bool, dt: f32) -> Option<ShellCommand> {
        self.tick(dt);
        if self.page == Page::Title {
            if start {
                // `ifs_start` main.f8 pc 23 (re-bf2-menus §4.1 [V]).
                self.cues.play(MenuSound::Accept);
                self.go(Page::Main);
            } else if input.back {
                return Some(ShellCommand::Quit);
            }
            return None;
        }
        if self.page == Page::Credits {
            // The crawl has no stack: the game draws it, and any skip input
            // (Cross, Back/Triangle or Start) leaves for the Singleplayer
            // screen that queued it. This is the page's only navigation.
            if input.back || input.confirm || start {
                self.cues.play(MenuSound::Exit);
                self.go(Page::Singleplayer);
            }
            return None;
        }
        if self.page == Page::CampaignBattle {
            if input.confirm {
                self.cues.play(MenuSound::Accept);
                return Some(ShellCommand::LaunchCampaignMission {
                    index: self.campaign_selected,
                });
            }
            if input.back || start {
                self.campaign_quit_confirm = false;
                self.campaign_menu.focus = self.campaign_menu.first();
                self.cues.play(MenuSound::Select);
                self.go(Page::CampaignMenu);
                return None;
            }
            return None;
        }
        if self.page == Page::CampaignSaves {
            return self.step_campaign_saves(input);
        }
        if self.page == Page::CampaignMenu {
            if self.campaign_quit_confirm {
                if input.back {
                    self.campaign_quit_confirm = false;
                    self.on_page = 0.0;
                    self.cues.play(MenuSound::Exit);
                    return None;
                }
            } else if input.back || (start && !input.confirm) {
                self.cues.play(MenuSound::Exit);
                self.go(Page::CampaignBattle);
                return None;
            }
        }
        if input.back {
            let to = match self.page {
                Page::Main => Page::Title,
                Page::AskTraining | Page::RiseNewLoad => Page::Singleplayer,
                Page::RiseOfTheEmpire => Page::RiseNewLoad,
                Page::Options if self.campaign_pause_options => {
                    self.campaign_pause_options = false;
                    Page::CampaignMenu
                }
                _ => Page::Main,
            };
            // The screen underneath plays its exit click on re-Enter.
            self.cues.play(MenuSound::Exit);
            self.go(to);
            return None;
        }
        if self.page == Page::RiseOfTheEmpire {
            return self.step_campaign(input);
        }
        let s = self.stack_mut()?;
        // `FollowButtonLink` clicks whenever it finds a row; links wrap.
        let mut moved = false;
        if input.up {
            s.focus = s.follow(-1);
            moved = true;
        }
        if input.down {
            s.focus = s.follow(1);
            moved = true;
        }
        let row = s.rows.get(s.focus).filter(|r| r.enabled).map(|r| r.id);
        if moved && row.is_some() {
            self.cues.play(MenuSound::Select);
        }
        if !input.confirm {
            return None;
        }
        let row = row?;
        self.cues.play(MenuSound::Accept);
        match row {
            RowId::Singleplayer => self.go(Page::Singleplayer),
            RowId::Training => {
                return Some(ShellCommand::LaunchCampaignMission { index: 0 });
            }
            RowId::RiseOfTheEmpire => {
                if !self.training_prompted {
                    self.go(Page::AskTraining);
                } else {
                    self.go(Page::RiseNewLoad);
                }
            }
            RowId::AskTrainingPlay => {
                self.set_training_prompted(true);
                return Some(ShellCommand::LaunchCampaignMission { index: 0 });
            }
            RowId::AskTrainingSkip => {
                self.set_training_prompted(true);
                if self.campaign_unlocked < 2 {
                    self.set_campaign_unlocked(2);
                }
                self.go(Page::RiseNewLoad);
            }
            RowId::CampaignNewGame => {
                if self.campaign_unlocked < 2 {
                    self.set_campaign_unlocked(2);
                }
                self.select_campaign_mission(1);
                return Some(ShellCommand::StartCampaignNewGame);
            }
            RowId::CampaignLoad => self.open_campaign_saves(
                campaign_saves::SaveMode::Load,
                campaign_saves::SaveReturn::NewLoad,
                false,
            ),
            RowId::CampaignResume => self.go(Page::CampaignBattle),
            RowId::CampaignOptions => {
                self.campaign_pause_options = true;
                self.go(Page::Options);
            }
            RowId::CampaignSave => self.open_campaign_saves(
                campaign_saves::SaveMode::Save,
                campaign_saves::SaveReturn::Menu,
                false,
            ),
            RowId::CampaignLoadMenu => self.open_campaign_saves(
                campaign_saves::SaveMode::Load,
                campaign_saves::SaveReturn::Menu,
                true,
            ),
            RowId::CampaignQuit => {
                self.campaign_quit_confirm = true;
                self.campaign_quit_popup.focus = 1;
                self.on_page = 0.0;
            }
            RowId::CampaignQuitYes => {
                self.campaign_quit_confirm = false;
                self.open_campaign_saves(
                    campaign_saves::SaveMode::Save,
                    campaign_saves::SaveReturn::Quit,
                    true,
                );
            }
            RowId::CampaignQuitNo => {
                self.campaign_quit_confirm = false;
                self.on_page = 0.0;
            }
            RowId::CampaignPlay => {
                return Some(ShellCommand::LaunchCampaignMission {
                    index: self.campaign_selected,
                })
            }
            RowId::CampaignBack => {
                self.cues.play(MenuSound::Exit);
                self.go(Page::RiseOfTheEmpire);
            }
            RowId::Options => self.go(Page::Options),
            RowId::Multiplayer => self.go(Page::Multiplayer),
            RowId::LanHost => return Some(ShellCommand::HostGame),
            RowId::LanJoin => return Some(ShellCommand::JoinGame),
            RowId::InstantAction => return Some(ShellCommand::InstantAction),
            RowId::GameOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Game))
            }
            RowId::AudioOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Audio))
            }
            RowId::GraphicsOptions => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Graphics))
            }
            RowId::Controls => {
                return Some(ShellCommand::Settings(crate::menu::SettingsPage::Controls))
            }
            RowId::QuitGame => return Some(ShellCommand::Quit),
            _ => {}
        }
        None
    }

    /// Enter the main stack after a profile is selected.
    pub fn profile_selected(&mut self) {
        self.main.focus = self.main.first();
        self.go(Page::Main);
    }

    pub fn profile_cancelled(&mut self) {
        self.go(Page::Title);
    }

    /// The page stands over the campaign map's planet: the planet screen,
    /// its pause menu, and the save screen opened from them
    /// (`ifs_campaign_load` has no movie, main pc 62-64).
    pub fn over_planet(&self) -> bool {
        match self.page {
            Page::CampaignBattle | Page::CampaignMenu => true,
            Page::CampaignSaves => {
                self.campaign_saves.back_to != campaign_saves::SaveReturn::NewLoad
            }
            _ => false,
        }
    }

    /// The movie property the page plays (§4). The save screen sets none and
    /// keeps the one playing (`ifs_campaign_load` main pc 62-64,
    /// `analysis/re-bf2-menus.md` "A screen that sets no `movieBackground`"):
    /// Rise of the Empire's `shell_sub_left`, or the planet screen's none.
    pub fn movie(&self) -> &'static str {
        match self.page {
            Page::Title | Page::Main => MOVIE_MAIN,
            Page::RiseOfTheEmpire => {
                if self.campaign_focus_toggle {
                    MOVIE_SUB
                } else {
                    crate::campaign::RISE_OF_THE_EMPIRE[self.campaign_selected].transition_movie
                }
            }
            Page::CampaignSaves if !self.over_planet() => MOVIE_SUB,
            Page::CampaignBattle | Page::CampaignMenu | Page::CampaignSaves | Page::Credits => "",
            Page::Singleplayer
            | Page::AskTraining
            | Page::RiseNewLoad
            | Page::Options
            | Page::Multiplayer => MOVIE_SUB,
        }
    }

    /// The prompt's alpha now: a triangle between [`PROMPT_ALPHA`] at
    /// [`PROMPT_RATE`] a second, starting at the top.
    pub fn prompt_alpha(&self) -> f32 {
        let [lo, hi] = PROMPT_ALPHA;
        let span = hi - lo;
        let t = (self.clock * PROMPT_RATE).rem_euclid(2.0 * span);
        if t < span {
            hi - t
        } else {
            lo + (t - span)
        }
    }
}

/// A stack's boxes in shell units: the title bar and one per row.
#[derive(Debug, Clone, PartialEq)]
pub struct StackLayout {
    pub title: [f32; 4],
    pub rows: Vec<[f32; 4]>,
}

/// Lay a stack out (the [M] rule of the module doc).
pub fn stack_layout(font: &HudFont, stack: &Stack) -> StackLayout {
    let widest = std::iter::once(stack.title.as_str())
        .chain(stack.rows.iter().map(|r| r.label.as_str()))
        .map(|t| crate::typography::width(font, &ITEM, t))
        .fold(0.0, f32::max);
    let w = widest + BAR_PAD;
    let x0 = if stack.left {
        LEFT_X
    } else {
        CENTRE_X - w * 0.5
    };
    let bars = stack.rows.len() + 1;
    let height = bars as f32 * ROW_PITCH - (ROW_PITCH - ROW_H);
    let top = CONTAINER_Y + stack.y_add - CENTRE_LIFT - height * 0.5;
    let bar = |k: usize| {
        let y = top + ROW_PITCH * k as f32;
        [x0, y, x0 + w, y + ROW_H]
    };
    StackLayout {
        title: bar(0),
        rows: (1..bars).map(bar).collect(),
    }
}

/// Find an enabled, revealed shell row beneath a window-space pointer.
pub fn row_at(
    font: &HudFont,
    menus: &ShellMenus,
    point: [f32; 2],
    size: [f32; 2],
) -> Option<usize> {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return None;
    }
    let stack = menus.stack()?;
    let box_w = 4.0 / 3.0 * size[1];
    let origin_x = if stack.left {
        0.0
    } else {
        (size[0] - box_w) * 0.5
    };
    let x = (point[0] - origin_x) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    stack
        .rows
        .iter()
        .zip(stack_layout(font, stack).rows)
        .enumerate()
        .find_map(|(i, (row, [x0, y0, x1, y1]))| {
            (row.enabled
                && menus.on_page >= ROW_REVEAL * (i + 1) as f32
                && x >= x0
                && x <= x1
                && y >= y0
                && y <= y1)
                .then_some(i)
        })
}

/// Hit-test the title page's visible Start prompt.
pub fn prompt_at(font: &HudFont, menus: &ShellMenus, point: [f32; 2], size: [f32; 2]) -> bool {
    if size[0] <= 0.0 || size[1] <= 0.0 || menus.page != Page::Title {
        return false;
    }
    let box_w = 4.0 / 3.0 * size[1];
    let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
    let y = point[1] / (size[1] / 448.0);
    let width = crate::typography::width(font, &ITEM, &menus.prompt);
    let x0 = 256.0 - width * 0.5;
    x >= x0 - 6.0
        && x <= x0 + width + 6.0
        && y >= PROMPT_TOP - 4.0
        && y <= PROMPT_TOP + f32::from(font.line_height) + 4.0
}

impl Ctx<'_> {
    /// The shell's page for a `size` window: the movie (its frame is the
    /// [`MOVIE_SHEET`] texture; `movie` false draws none), then the logo
    /// or the stack and the help lines.
    pub(crate) fn shell_page(&mut self, m: &ShellMenus, movie: bool, size: [f32; 2]) {
        use crate::spawn_draw::{screen_at, Anchor};
        let centre = screen_at(size, Anchor::Centre);
        let full = [0.0, 0.0, 1.0, 1.0];
        if movie {
            self.sheet(&centre, [0.0, 0.0, 512.0, 448.0], full, MOVIE_SHEET, 1.0);
        }
        if m.covered {
            return;
        }
        if matches!(m.page, Page::Title | Page::Main) {
            self.sheet(&centre, LOGO, full, "logo", 1.0);
        }
        if m.page == Page::CampaignBattle {
            let entry = &crate::campaign::RISE_OF_THE_EMPIRE[m.campaign_selected];
            let title = m
                .campaign_planets
                .get(m.campaign_selected)
                .map_or(entry.title, String::as_str);
            let desc = m
                .campaign_descs
                .get(m.campaign_selected)
                .map_or(entry.desc_fallback, String::as_str);
            self.bordered(&centre, [36.0, 304.0, 476.0, 396.0]);
            self.text(
                &centre,
                &crate::typography::BODY,
                title,
                (52.0, 316.0),
                HAlign::Left,
                tint::TITLE,
            );
            for (i, line) in self
                .wrapped(&crate::typography::LIST, desc, 408.0)
                .iter()
                .take(3)
                .enumerate()
            {
                self.text(
                    &centre,
                    &crate::typography::LIST,
                    line,
                    (52.0, 340.0 + i as f32 * 16.0),
                    HAlign::Left,
                    tint::BODY,
                );
            }
            self.help(size, true, Prompt::Accept, &m.campaign_launch_label);
            return;
        }
        if m.page == Page::CampaignSaves {
            self.campaign_saves_page(m, size);
            return;
        }
        let Some(stack) = m.stack() else {
            let (rgb, _) = tint::UNSELECTED;
            let at = (256.0, PROMPT_TOP);
            let style = (rgb, m.prompt_alpha());
            self.text(&centre, &ITEM, &m.prompt, at, HAlign::Center, style);
            return;
        };
        let Some(font) = self.face(ITEM.face) else {
            return;
        };
        let lay = stack_layout(font, stack);
        let s = if stack.left {
            screen_at(size, Anchor::Left)
        } else {
            centre
        };
        if matches!(m.page, Page::AskTraining)
            || (m.page == Page::CampaignMenu && m.campaign_quit_confirm)
        {
            let prompt = if m.page == Page::AskTraining {
                &m.ask_training_prompt
            } else {
                &m.campaign_quit_prompt
            };
            self.bordered(&centre, [118.0, 144.0, 394.0, 276.0]);
            for (i, line) in self
                .wrapped(&crate::typography::BODY, prompt, 248.0)
                .iter()
                .take(5)
                .enumerate()
            {
                self.text(
                    &centre,
                    &crate::typography::BODY,
                    line,
                    (256.0, 160.0 + i as f32 * 18.0),
                    HAlign::Center,
                    tint::BODY,
                );
            }
        }
        if !stack.title.is_empty() {
            let title = FlashyRow {
                rect: lay.title,
                pieces: crate::spawn_draw::TITLE_PIECES,
                bar: ([255.0; 3], crate::spawn_draw::PIECE_ALPHA),
                text: &stack.title,
                style: tint::TITLE,
                font: &TITLE,
            };
            self.flashy_row(&s, &title, m.on_page);
        }
        let pulse = m.pulse.alpha();
        for (k, (row, r)) in stack.rows.iter().zip(&lay.rows).enumerate() {
            let row = Self::flashy_row_from_stack(
                row,
                *r,
                k,
                stack.rows.len(),
                stack.focus,
                pulse,
                &ITEM,
            );
            let t = m.on_page - ROW_REVEAL * (k + 1) as f32;
            self.flashy_row(&s, &row, t);
        }
        if let (Some(note), Some(last)) = (&stack.note, lay.rows.last()) {
            let t = m.on_page - ROW_REVEAL * (stack.rows.len() + 1) as f32;
            let a = flashy(t).text_a;
            if a > 0.0 {
                let at = (last[0] + 4.0, last[3] + ROW_PITCH - ROW_H + 6.0);
                let style = (tint::NOTE.0, tint::NOTE.1 * a);
                self.text(&s, &ITEM, note, at, HAlign::Left, style);
            }
        }
        if m.page != Page::AskTraining {
            self.help(size, false, Prompt::Back, &m.back);
        }
        self.help(size, true, Prompt::Accept, &m.select);
    }

    /// Build a stack row's [`FlashyRow`] at its laid-out `rect`. The bar and
    /// text style come from the row's enabled/focus state and the pulse
    /// `alpha`; the font is the label's role ([`ITEM`] on a shell stack,
    /// [`crate::typography::BODY`] on a save popup's buttons). Shared by the
    /// shell stacks here and the campaign save popups
    /// ([`crate::shell_ps2::campaign_saves`]), so the two draw the same rows.
    pub(crate) fn flashy_row_from_stack<'a>(
        row: &'a StackRow,
        rect: [f32; 4],
        k: usize,
        n: usize,
        focus: usize,
        alpha: f32,
        font: &'static crate::typography::TextStyle,
    ) -> FlashyRow<'a> {
        let grey = ([UNFOCUSED_BAR; 3], tint::UNSELECTED.1);
        let (bar, style) = if !row.enabled {
            (grey, tint::DIMMED)
        } else if k == focus {
            (([255.0; 3], alpha), (tint::SELECTED.0, alpha))
        } else {
            (grey, tint::UNSELECTED)
        };
        FlashyRow {
            rect,
            pieces: crate::spawn_draw::item_pieces(k + 1 == n),
            bar,
            text: &row.label,
            style,
            font,
        }
    }

    /// One stack row (or the title) `t` seconds into its flashy entrance
    /// (re-bf2-menus §5 [V]): the bar's middle wipes out, a `cursor` square
    /// sweeps across the text box, the text pops up about its centre line
    /// and fades in.
    pub(crate) fn flashy_row(&mut self, s: &crate::hud_draw::Screen, row: &FlashyRow<'_>, t: f32) {
        let f = flashy(t);
        let [x0, y0, x1, y1] = row.rect;
        let Some(out) = f.bar else {
            return;
        };
        let caps = 2.0 * (y1 - y0);
        let right = x0 + caps + (x1 - x0 - caps).max(0.0) * out;
        self.bar_tinted(s, [x0, y0, right, y1], row.pieces, row.bar);
        let style = row.font;
        let lh = style.size;
        // Text wider than its bar ends in an ellipsis, never a smaller size.
        let text = self.ellipsized(style, row.text, x1 - x0 - 2.0 * (y1 - y0));
        let (cx, top) = ((x0 + x1) * 0.5, y0 + (y1 - y0 - lh) * 0.5);
        if let Some(across) = f.square {
            let w = self.text_width(style, &text);
            let x = cx - w * 0.5 + w * across;
            let rect = [x, top, x + lh, top + lh];
            let full = [0.0, 0.0, 1.0, 1.0];
            self.sheet(s, rect, full, CURSOR_SHEET, row.style.1);
        }
        if f.text_h <= 0.0 {
            return;
        }
        let at = (cx, top + lh * (1.0 - f.text_h) * 0.5);
        let tint = (row.style.0, row.style.1 * f.text_a);
        self.text_popped(s, style, &text, at, HAlign::Center, tint, f.text_h);
    }
}

/// A row as [`Ctx::flashy_row`] draws it.
pub(crate) struct FlashyRow<'a> {
    pub(crate) rect: [f32; 4],
    pub(crate) pieces: [&'static str; 3],
    /// The bar's tint and alpha.
    pub(crate) bar: ([f32; 3], f32),
    pub(crate) text: &'a str,
    pub(crate) style: ([f32; 3], f32),
    /// The label's role: [`TITLE`] on a title bar, [`ITEM`] on a stack's
    /// row, [`crate::typography::BODY`] on a popup's (`gPopupButtonFont`).
    pub(crate) font: &'static crate::typography::TextStyle,
}

#[cfg(test)]
mod tests;
