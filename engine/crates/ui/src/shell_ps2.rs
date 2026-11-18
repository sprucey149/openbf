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

/// Maximum mission rows shown at once on [`Page::RiseOfTheEmpire`] so the
/// stack and note fit within the 448-unit shell height.
const CAMPAIGN_WINDOW: usize = 6;

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
    LaunchCampaignMission { index: usize },
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
        self.campaign_selected = self.campaign_selected.min(self.visible_campaign_count() - 1);
        self.rebuild_campaign_stacks();
    }

    /// Highlight mission `index` on [`Page::RiseOfTheEmpire`].
    pub fn select_campaign_mission(&mut self, index: usize) {
        let count = self.visible_campaign_count();
        self.campaign_selected = index.min(count - 1);
        self.campaign_focus_toggle = false;
        self.rebuild_campaign_stacks();
    }

    fn rebuild_campaign_stacks(&mut self) {
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
                "Show Unlocked Only".into()
            } else {
                "View All Missions".into()
            },
            enabled: true,
        });
        let focus = if self.campaign_focus_toggle {
            rows.len() - 1
        } else {
            self.campaign_selected - win_start
        };
        let note = if self.campaign_focus_toggle {
            format!("Showing {count} of {} missions", crate::campaign::RISE_OF_THE_EMPIRE.len())
        } else {
            let e = &crate::campaign::RISE_OF_THE_EMPIRE[self.campaign_selected];
            let planet = &self.campaign_planets[self.campaign_selected];
            format!(
                "{} ({}/{count}) \u{2014} {planet}",
                e.era.to_ascii_uppercase(),
                self.campaign_selected + 1
            )
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

    pub fn stack(&self) -> Option<&Stack> {
        match self.page {
            Page::Title | Page::CampaignBattle => None,
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
            Page::Title | Page::CampaignBattle => None,
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

    fn step_campaign(&mut self, input: MenuInput) -> Option<ShellCommand> {
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
            self.campaign_selected = self.campaign_selected.min(self.visible_campaign_count() - 1);
            self.rebuild_campaign_stacks();
        } else {
            self.go(Page::CampaignBattle);
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
            Page::CampaignBattle | Page::CampaignMenu | Page::CampaignSaves => "",
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
            let grey = ([UNFOCUSED_BAR; 3], tint::UNSELECTED.1);
            let (bar, style) = if !row.enabled {
                (grey, tint::DIMMED)
            } else if k == stack.focus {
                (([255.0; 3], pulse), (tint::SELECTED.0, pulse))
            } else {
                (grey, tint::UNSELECTED)
            };
            let row = FlashyRow {
                rect: *r,
                pieces: crate::spawn_draw::item_pieces(k + 1 == stack.rows.len()),
                bar,
                text: &row.label,
                style,
                font: &ITEM,
            };
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
mod tests {
    use super::*;
    const BUTTON_FONT: &str = "gamefont_large";
    use crate::typography::tint::SELECTED as SELECTED_TEXT;

    #[test]
    fn shell_draw_and_pointer_share_menu_metrics_at_all_capture_sizes() {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use manifest::hud::{HudFont, HudGlyph, HudLayout};
        let legacy = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["legacy".into()],
            glyphs: (32..127)
                .map(|code| HudGlyph {
                    code,
                    page: 0,
                    advance: 4,
                    x0: 0,
                    x1: 4,
                    y0: 3,
                    y1: 18,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut menu = legacy.clone();
        menu.name = "menu_gamefont_large".into();
        menu.pages = vec!["menu".into()];
        for g in &mut menu.glyphs {
            g.advance = 12;
            g.x1 = 12;
        }
        let mut art = HudArt::default();
        art.fonts.insert(legacy.name.clone(), legacy);
        art.fonts.insert(menu.name.clone(), menu);
        let mut menus = ShellMenus::new(&|_, fallback| fallback.into());
        menus.page = Page::Main;
        menus.on_page = 5.0;
        let empty = HudLayout {
            name: "empty".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.shell = Some((menus.clone(), false));
        let layout = stack_layout(art.menu_font(BUTTON_FONT).unwrap(), menus.stack().unwrap());
        for size in [
            [1920.0, 1080.0],
            [2560.0, 1440.0],
            [3840.0, 2160.0],
            [640.0, 480.0],
        ] {
            let frame = crate::hud_draw::draw(&hud, &art, size);
            assert!(frame.batches.iter().any(|b| b.texture == "menu"));
            assert!(!frame.batches.iter().any(|b| b.texture == "legacy"));
            let [x0, y0, x1, y1] = layout.rows[0];
            let screen = crate::spawn_draw::screen_at(size, crate::spawn_draw::Anchor::Centre);
            let point = screen.map(glam::vec3(x1 - 1.0, (y0 + y1) * 0.5, 0.0));
            assert_eq!(
                row_at(art.menu_font(BUTTON_FONT).unwrap(), &menus, point, size),
                Some(0)
            );
            // The edge of this wider row is outside the old, narrow font's bar.
            assert!(x1 > x0);
            assert_eq!(row_at(&art.fonts[BUTTON_FONT], &menus, point, size), None);
        }
        hud.shell = None;
        let ctx = Ctx {
            hud: &hud,
            art: &art,
            out: Default::default(),
            clip: None,
        };
        let face = ctx.face(crate::typography::ITEM.face).unwrap();
        assert_eq!(face.name, "menu_gamefont_large");
    }

    fn menus() -> ShellMenus {
        ShellMenus::new(&|_, e| e.to_owned())
    }

    fn press(f: impl Fn(&mut MenuInput)) -> MenuInput {
        let mut i = MenuInput::default();
        f(&mut i);
        i
    }

    /// Acceptance 2 of card BSHLA, updated for issue #168: Multiplayer sits
    /// in its PS2 slot and is live; the dimmed Splitscreen is skipped both
    /// ways (§3.3).
    #[test]
    fn main_menu_skips_the_disabled_splitscreen() {
        let mut m = menus();
        assert_eq!(m.step(MenuInput::default(), true, 0.1), None);
        assert_eq!(m.page, Page::Main);
        let ids: Vec<_> = m.main.rows.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            [
                RowId::Singleplayer,
                RowId::Multiplayer,
                RowId::Splitscreen,
                RowId::Options,
                RowId::QuitGame
            ]
        );
        assert!(m.main.rows[1].enabled, "Multiplayer is live (issue #168)");
        assert!(!m.main.rows[2].enabled, "Splitscreen is disabled");
        assert_eq!(m.main.focus, 0);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::Multiplayer);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(
            m.main.rows[m.main.focus].id,
            RowId::Options,
            "Down skips Splitscreen"
        );
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::Multiplayer, "and Up");
        m.step(press(|i| i.up = true), false, 0.1);
        // Wrapping: Up from Singleplayer lands on Quit Game, Down from Quit
        // Game on Singleplayer.
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::QuitGame);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.main.rows[m.main.focus].id, RowId::Singleplayer);
        // A confirm never lands on a disabled row.
        m.main.focus = 2;
        assert_eq!(m.step(press(|i| i.confirm = true), false, 0.1), None);
        assert_eq!(m.page, Page::Main);
    }

    /// Playtest A5: the main menu's bottom row is Quit Game, by pad (the
    /// wrap) or by mouse, and it quits.
    #[test]
    fn main_menu_quit_game_is_the_last_row_and_quits() {
        use crate::shell_feel::MenuSound as S;
        use manifest::hud::{HudFont, HudGlyph};
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.1);
        m.cues.take();
        m.step(press(|i| i.up = true), false, 0.2);
        let row = &m.main.rows[m.main.focus];
        assert_eq!((row.id, row.label.as_str()), (RowId::QuitGame, "Quit Game"));
        assert_eq!(m.main.focus, m.main.rows.len() - 1, "the bottom row");
        m.cues.take();
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.2),
            Some(ShellCommand::Quit)
        );
        assert_eq!(m.cues.take(), [S::Accept]);
        // The mouse: the row's bar is a hit, and a click on it quits.
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut m = menus();
        m.page = Page::Main;
        m.on_page = 2.0;
        let [x0, y0, x1, y1] = stack_layout(&font, m.stack().unwrap()).rows[4];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(4));
        assert!(m.focus_row(4));
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.2),
            Some(ShellCommand::Quit)
        );
    }

    #[test]
    fn mouse_hit_uses_the_visible_row_layout_and_skips_disabled_rows() {
        use manifest::hud::{HudFont, HudGlyph};
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut m = menus();
        m.page = Page::Main;
        m.on_page = 2.0;
        let stack = m.stack().unwrap();
        let layout = stack_layout(&font, stack);
        let [x0, y0, x1, y1] = layout.rows[3];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), Some(3));
        let [x0, y0, x1, y1] = layout.rows[2];
        let p = [
            ((x0 + x1) * 0.5) * 1.25,
            ((y0 + y1) * 0.5) * (480.0 / 448.0),
        ];
        assert_eq!(row_at(&font, &m, p, [640.0, 480.0]), None);
    }

    /// `m` as the HUD pass lays it out in a 640 x 480 window, with a font
    /// of 10-unit glyphs.
    fn frame(m: ShellMenus) -> crate::hud_draw::HudFrame {
        use crate::hud_draw::HudArt;
        use crate::hud_tree::Hud;
        use manifest::hud::{HudFont, HudGlyph, HudLayout};
        let font = HudFont {
            name: BUTTON_FONT.into(),
            line_height: 22,
            pages: vec!["page0".into()],
            glyphs: (32u8..127)
                .map(|code| HudGlyph {
                    code: u16::from(code),
                    page: 0,
                    advance: 10,
                    x0: 0,
                    x1: 10,
                    y0: 2,
                    y1: 14,
                    uv: [0.0, 0.1, 0.0, 0.1],
                })
                .collect(),
        };
        let mut art = HudArt::default();
        art.fonts.insert(BUTTON_FONT.into(), font);
        let empty = HudLayout {
            name: "t".into(),
            nodes: Vec::new(),
        };
        let mut hud = Hud::new(&empty, &empty);
        hud.shell = Some((m, false));
        crate::hud_draw::draw(&hud, &art, [640.0, 480.0])
    }

    /// The main menu as the HUD pass lays it out: the dimmed rows' labels
    /// are drawn grey 110 (§3.3), every glyph of both of them.
    #[test]
    fn the_disabled_rows_are_drawn_dimmed() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 5.0);
        m.step(MenuInput::default(), false, 5.0);
        let f = frame(m);
        let grey = DIMMED_GREY / 255.0;
        let dimmed = f
            .verts
            .iter()
            .filter(|v| v.color[..3].iter().all(|c| (c - grey).abs() < 1e-4))
            .count();
        let glyphs = "Splitscreen".len();
        assert_eq!(dimmed, 4 * glyphs, "Splitscreen dimmed");
        let sheets: Vec<&str> = f.batches.iter().map(|b| b.texture.as_str()).collect();
        for p in [
            "logo",
            "bf2_buttons_topleft",
            "bf2_buttons_botright",
            "btna",
            "btnb",
        ] {
            assert!(sheets.contains(&p), "{p}: {sheets:?}");
        }
    }

    #[test]
    fn the_flow_reaches_instant_action_and_back() {
        let mut m = menus();
        let confirm = press(|i| i.confirm = true);
        // Only Start leaves the title: Cross does nothing there.
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Title);
        m.step(MenuInput::default(), true, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.movie(), MOVIE_SUB);
        // Rise of the Empire is live; down skips disabled Galactic Conquest to Instant Action.
        assert_eq!(
            m.singleplayer.rows[m.singleplayer.focus].id,
            RowId::RiseOfTheEmpire
        );
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(
            m.singleplayer.rows[m.singleplayer.focus].id,
            RowId::InstantAction
        );
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::InstantAction)
        );
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::Main);
        // Down: Multiplayer (issue #168), then Options past Splitscreen.
        m.step(press(|i| i.down = true), false, 0.1);
        m.step(press(|i| i.down = true), false, 0.1);
        m.step(confirm, false, 0.1);
        assert_eq!(m.page, Page::Options);
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::Settings(crate::menu::SettingsPage::Game))
        );
        m.step(press(|i| i.back = true), false, 0.1);
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::Title);
        assert_eq!(m.movie(), MOVIE_MAIN);
        assert_eq!(
            m.step(press(|i| i.back = true), false, 0.1),
            Some(ShellCommand::Quit)
        );
    }

    #[test]
    fn controls_are_reachable_from_options() {
        let mut m = menus();
        m.page = Page::Options;
        m.options.focus = m
            .options
            .rows
            .iter()
            .position(|row| row.id == RowId::Controls)
            .unwrap();
        assert!(m.options.rows[m.options.focus].enabled);
        assert_eq!(
            m.step(press(|i| i.confirm = true), false, 0.1),
            Some(ShellCommand::Settings(crate::menu::SettingsPage::Controls))
        );
    }

    #[test]
    fn the_prompt_pulses_between_its_alphas() {
        let mut m = menus();
        let mut seen = (f32::MAX, f32::MIN);
        for _ in 0..100 {
            m.step(MenuInput::default(), false, 0.05);
            let a = m.prompt_alpha();
            seen = (seen.0.min(a), seen.1.max(a));
        }
        assert!(
            (seen.0 - 0.4).abs() < 0.03 && (seen.1 - 1.0).abs() < 0.03,
            "{seen:?}"
        );
    }

    /// re-bf2-menus §4.1: Start on the title accepts; a stack move clicks
    /// even through the wrap; Cross accepts; Back plays the exit click.
    #[test]
    fn the_stacks_ask_for_the_ps2_clicks() {
        use crate::shell_feel::MenuSound as S;
        let mut m = menus();
        m.step(press(|i| i.confirm = true), false, 0.2);
        assert!(m.cues.take().is_empty(), "Cross does nothing on the title");
        m.step(MenuInput::default(), true, 0.2);
        assert_eq!(m.cues.take(), [S::Accept]);
        // Multiplayer, Options, Quit Game, Singleplayer (the wrap),
        // Multiplayer, Options.
        for _ in 0..6 {
            m.step(press(|i| i.down = true), false, 0.2);
        }
        assert_eq!(m.cues.take(), [S::Select; 6], "every press, the wrap too");
        m.step(press(|i| i.confirm = true), false, 0.2);
        assert_eq!(m.page, Page::Options);
        assert_eq!(m.cues.take(), [S::Accept]);
        m.step(press(|i| i.back = true), false, 0.2);
        assert_eq!(m.cues.take(), [S::Exit]);
        // Presses faster than 0.1 s apart: one click.
        m.step(press(|i| i.down = true), false, 0.02);
        m.step(press(|i| i.down = true), false, 0.02);
        assert_eq!(m.cues.take(), [S::Select]);
        // Back from a page of the port's own: the stack's re-Enter.
        m.on_page = 3.0;
        m.uncover();
        assert_eq!((m.cues.take(), m.on_page), (vec![S::Exit], 0.0));
    }

    /// re-bf2-menus §5: the pulse's phase runs on through a focus move, and
    /// the focused row's label and bar share it; the others sit at 0.6 with
    /// a grey 150 bar. It steps at 30 Hz (menuref §9.4.1).
    #[test]
    fn the_focused_row_pulses_and_keeps_its_phase_across_moves() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.0);
        m.step(MenuInput::default(), false, 0.15);
        let before = m.pulse.alpha();
        // 0.15 s = 4 whole updates of 2/30 from 0.5.
        assert!((before - (0.5 + 4.0 * 2.0 / 30.0)).abs() < 1e-4, "{before}");
        m.step(press(|i| i.down = true), false, 0.05);
        assert!(
            (m.pulse.alpha() - 0.9).abs() < 1e-4,
            "not reset by the move"
        );
        m.on_page = 5.0;
        let a = m.pulse.alpha();
        let f = frame(m);
        let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
        let focused = |v: &&crate::hud_draw::HudVertex| {
            (v.color[0] - r).abs() < 1e-4
                && (v.color[1] - g).abs() < 1e-4
                && (v.color[2] - b).abs() < 1e-4
        };
        let label: Vec<_> = f.verts.iter().filter(focused).collect();
        // Down from Singleplayer: Multiplayer (issue #168).
        assert_eq!(label.len(), 4 * "Multiplayer".len());
        assert!(label.iter().all(|v| (v.color[3] - a).abs() < 1e-4));
        let white_bar = f
            .verts
            .iter()
            .filter(|v| v.color == [1.0, 1.0, 1.0, a])
            .count();
        assert_eq!(white_bar, 3 * 4, "the focused bar's three pieces");
        let grey = UNFOCUSED_BAR / 255.0;
        let grey_bars = f
            .verts
            .iter()
            .filter(|v| v.color == [grey, grey, grey, 0.6])
            .count();
        assert_eq!(grey_bars, 4 * 3 * 4, "the other four rows' bars");
    }

    /// re-bf2-menus §5, menuref §9.4.5: on entering a page nothing of row 1
    /// shows until its 0.1 s delay, then its bar wipes out before its text
    /// appears.
    #[test]
    fn a_page_entry_replays_the_flashy_entrance() {
        let mut m = menus();
        m.step(MenuInput::default(), true, 0.0);
        let bar_right = |m: &ShellMenus| {
            let f = frame(m.clone());
            f.verts
                .iter()
                .filter(|v| v.color[..3] == [1.0, 1.0, 1.0] && v.color[3] == m.pulse.alpha())
                .map(|v| v.pos[0])
                .fold(f32::MIN, f32::max)
        };
        m.on_page = 0.05;
        assert_eq!(bar_right(&m), f32::MIN, "row 1 not started");
        m.on_page = 0.1 + 0.05;
        let half = bar_right(&m);
        m.on_page = 0.1 + 0.1;
        let full = bar_right(&m);
        assert!(half > f32::MIN && half < full, "{half} {full}");
        let text = |m: &ShellMenus| {
            let [r, g, b] = SELECTED_TEXT.0.map(|c| c / 255.0);
            frame(m.clone())
                .verts
                .iter()
                .filter(|v| v.color[..3] == [r, g, b])
                .count()
        };
        m.on_page = 0.1 + 0.15;
        assert_eq!(text(&m), 0, "the square sweeps before the text");
        m.on_page = 0.1 + 0.3;
        assert_eq!(text(&m), 4 * "Singleplayer".len());
    }

    /// menuref "Singleplayer": coming back to Singleplayer (from Instant
    /// Action, or from the Main Menu) focuses its first row, not the one
    /// left.
    #[test]
    fn singleplayer_is_always_entered_on_its_first_row() {
        let mut m = menus();
        // As if Training were shown: the first row live.
        m.singleplayer.rows[0].enabled = true;
        m.step(MenuInput::default(), true, 0.1);
        m.step(press(|i| i.confirm = true), false, 0.1);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.singleplayer.focus, 0);
        m.singleplayer.focus = 2;
        // Instant Action opens over it and closes: back on the first row.
        m.uncover();
        assert_eq!(m.singleplayer.focus, 0);
        m.singleplayer.focus = 2;
        m.step(press(|i| i.back = true), false, 0.1);
        m.step(press(|i| i.confirm = true), false, 0.1);
        assert_eq!(m.page, Page::Singleplayer);
        assert_eq!(m.singleplayer.focus, 0);
    }

    /// Issue #168: Main → Multiplayer offers LAN Host Game and Join Game;
    /// Online is shown dimmed with its note, never focused; Back returns to
    /// the main menu.
    #[test]
    fn multiplayer_offers_lan_host_and_join_and_a_dimmed_online_row() {
        let mut m = menus();
        let confirm = press(|i| i.confirm = true);
        m.step(MenuInput::default(), true, 0.1);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Multiplayer);
        assert_eq!(m.movie(), MOVIE_SUB);
        let rows: Vec<_> = m
            .multiplayer
            .rows
            .iter()
            .map(|r| (r.id, r.label.as_str(), r.enabled))
            .collect();
        assert_eq!(
            rows,
            [
                (RowId::LanHost, "LAN: Host Game", true),
                (RowId::LanJoin, "LAN: Join Game", true),
                (RowId::Online, "Online", false),
            ]
        );
        assert_eq!(
            m.multiplayer.note.as_deref(),
            Some("Online play needs a dedicated server \u{2014} not available yet")
        );
        assert_eq!(m.step(confirm, false, 0.1), Some(ShellCommand::HostGame));
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), Some(ShellCommand::JoinGame));
        // Down from Join wraps past the dimmed Online row to Host.
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.multiplayer.rows[m.multiplayer.focus].id, RowId::LanHost);
        assert!(!m.focus_row(2), "Online cannot take the pointer's focus");
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::Main);
    }

    /// The Online note is drawn under the Multiplayer stack once revealed.
    #[test]
    fn the_online_note_is_drawn_on_the_multiplayer_page() {
        let mut m = menus();
        m.page = Page::Multiplayer;
        m.step(MenuInput::default(), false, 5.0);
        let f = frame(m);
        let note = ([DIMMED_GREY + 60.0; 3].map(|c| c / 255.0), ());
        let lit = f
            .verts
            .iter()
            .filter(|v| {
                v.color[..3]
                    .iter()
                    .zip(note.0)
                    .all(|(c, n)| (c - n).abs() < 1e-4)
            })
            .count();
        assert!(lit >= 4 * "Online".len(), "note glyphs drawn: {lit}");
    }

    #[test]
    fn rise_of_the_empire_page_lists_unlocked_missions_toggles_all_and_launches() {
        let mut m = menus();
        let confirm = press(|i| i.confirm = true);
        m.step(MenuInput::default(), true, 0.1);
        m.step(confirm, false, 0.1);
        assert_eq!(m.page, Page::Singleplayer);

        // First visit prompts for Geonosis training; Skip unlocks Turn 1 and opens New Game / Load.
        m.step(confirm, false, 0.1);
        assert_eq!(m.page, Page::AskTraining);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert!(m.training_prompted());
        assert_eq!(m.campaign_unlocked, 2);
        assert_eq!(m.page, Page::RiseNewLoad);

        // New Game emits StartCampaignNewGame targeting Turn 1 (Mygeeto).
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::StartCampaignNewGame)
        );
        assert_eq!(m.campaign_selected, 1);

        // Load opens the save list without the load question (`SkipPromptSave`); Back returns.
        m.open_page(Page::RiseNewLoad);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::CampaignSaves);
        assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::List);
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::RiseNewLoad);

        // Open CampaignBattle directly (as LoadCampaign / crawl completion does), check 3D planet movie == ""
        m.select_campaign_mission(1);
        m.open_page(Page::CampaignBattle);
        assert_eq!(m.page, Page::CampaignBattle);
        assert_eq!(m.movie(), "");

        // Esc / Back / Start from CampaignBattle opens Game Menu (CampaignMenu) over the planet.
        assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
        assert_eq!(m.page, Page::CampaignMenu);
        assert_eq!(m.movie(), "");

        // Row 0 = Resume Game -> back to CampaignBattle.
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::CampaignBattle);

        // Re-open Game Menu -> Options -> Back returns to Game Menu (not Main).
        m.step(MenuInput::default(), true, 0.1);
        assert_eq!(m.page, Page::CampaignMenu);
        m.step(press(|i| i.down = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Options);
        assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
        assert_eq!(m.page, Page::CampaignMenu);

        // Save Game opens the save list; Load Game asks the load question first.
        m.focus_row(2);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::CampaignSaves);
        assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::List);
        m.step(press(|i| i.back = true), false, 0.1);
        assert_eq!(m.page, Page::CampaignMenu);
        m.focus_row(3);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::ConfirmLoad);
        m.step(press(|i| i.up = true), false, 0.1);
        m.step(confirm, false, 0.1);
        assert_eq!(m.page, Page::CampaignMenu);

        // Quit opens Yes/No confirmation defaulting to No; Back cancels, Yes returns to Singleplayer.
        m.focus_row(4);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert!(m.campaign_quit_confirm);
        assert_eq!(m.campaign_quit_popup.focus, 1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert!(!m.campaign_quit_confirm);
        assert_eq!(m.page, Page::CampaignMenu);

        // Yes asks the save question (`QuitRequested`), then leaves.
        m.focus_row(4);
        m.step(confirm, false, 0.1);
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::CampaignSaves);
        assert_eq!(m.campaign_saves.step, campaign_saves::SaveStep::Prompt);
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.step(confirm, false, 0.1), None);
        assert_eq!(m.page, Page::Singleplayer);

        // Attack from CampaignBattle launches the selected mission.
        m.set_campaign_unlocked(18);
        m.select_campaign_mission(17);
        m.open_page(Page::CampaignBattle);
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::LaunchCampaignMission { index: 17 })
        );

        // Once prompted, Training appears as Singleplayer's first row and launches mission 0 directly.
        m.open_page(Page::Singleplayer);
        assert_eq!(m.singleplayer.rows[0].id, RowId::Training);
        assert_eq!(
            m.step(confirm, false, 0.1),
            Some(ShellCommand::LaunchCampaignMission { index: 0 })
        );
    }

    #[test]
    fn keyboard_enter_sets_confirm_and_start_without_hijacking_campaign_menu() {
        let mut m = menus();
        m.set_campaign_unlocked(2);
        m.select_campaign_mission(1);
        m.open_page(Page::CampaignBattle);

        // On desktop, KeyCode::Enter sets both confirm and start in menu_bits().
        let enter = press(|i| i.confirm = true);
        assert_eq!(
            m.step(enter, true, 0.1),
            Some(ShellCommand::LaunchCampaignMission { index: 1 })
        );
        assert_eq!(m.page, Page::CampaignBattle);

        // Esc / controller Start (back/start without confirm) still opens Game Menu.
        assert_eq!(m.step(press(|i| i.back = true), false, 0.1), None);
        assert_eq!(m.page, Page::CampaignMenu);

        // Inside Game Menu, Enter on Quit opens confirmation -> Up -> Yes -> the save question.
        m.focus_row(4);
        assert_eq!(m.step(enter, true, 0.1), None);
        assert!(m.campaign_quit_confirm);
        assert_eq!(m.page, Page::CampaignMenu);
        m.step(press(|i| i.up = true), false, 0.1);
        assert_eq!(m.step(enter, true, 0.1), None);
        assert_eq!(m.page, Page::CampaignSaves);
    }
}
