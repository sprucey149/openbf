//! BF2's PS2 Instant Action screen (`ifs_missionselect`) and its Session
//! Options (`ifs_instant_options_overview` / `ifs_instant_options`), the
//! model: the map list, the five states and every press, the playlist, the
//! options and the launch queue (`analysis/re-bf2-menus.md` §1, §2, §6, §7).
//! Drawn by the HUD pass in `crate::shell_ia`; the game builds the catalog
//! from its bake and plays the playlist it is handed.
//!
//! `[ours]` (§7): the map list holds both games' maps, each "<name> (bf1)" /
//! "<name> (bf2)"; rows, modes, eras and option groups the port cannot run
//! are dimmed (grey 110, never focused) - the PS2 has that style for button
//! stacks only, its list boxes have none; the era list ends with the port's
//! cross-era pairs.

use crate::menu::MenuInput;
use crate::shell_feel::MenuSound;

/// `ScriptCB_GetMaxMissionQueue()` = 128 (§1.3, B2 357683).
pub const MAX_MISSIONS: usize = 128;
/// Rows shown by the Planet, Mode and Playlist lists, and by the Era list
/// (`clamp(floor((403.2 - 240) / 17), 4, 16)` = 9, Era half that; §1.2).
pub const LIST_ROWS: usize = 9;
pub const ERA_ROWS: usize = 4;
/// L1 / R1 move a 9-row list 8 rows (menuref "Instant Action": 0 -> 8 ->
/// 16 -> 24 on the Planet list, `ia_list_page`).
pub const LIST_PAGE: usize = 8;
/// Rows shown on an options page (§2.2).
pub const OPTION_ROWS: usize = 5;
/// `iMaxBots`: AI units per team 0..16 (menuref §9.1.1 [M], `so_global`:
/// Right and R1 at 16 stay 16; the ELF image's 16, not the 25 of
/// `ScriptCB_GetNetGameDefaults`' non-network branch).
pub const MAX_BOTS: i32 = 16;
/// The modes the port's sim can run (§7 dims the rest).
pub const PORT_MODES: [Mode; 4] = [Mode::Con, Mode::Eli, Mode::Assault, Mode::OneFlag];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Game {
    Bf1,
    Bf2,
}

impl Game {
    pub fn tag(self) -> &'static str {
        match self {
            Game::Bf1 => "bf1",
            Game::Bf2 => "bf2",
        }
    }
}

/// `gMapModes` in screen order (con, ctf, 1flag, tdm, hunt, ass, eli; §6);
/// no map has tdm, so it is left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Con,
    Ctf,
    OneFlag,
    Hunt,
    Assault,
    Eli,
}

pub const MODES: [Mode; 6] = [
    Mode::Con,
    Mode::Ctf,
    Mode::OneFlag,
    Mode::Hunt,
    Mode::Assault,
    Mode::Eli,
];

impl Mode {
    /// The mission name's suffix (`hot1g_con`), the port's mission files'
    /// too (`bf2_hot1_con.ron`).
    pub fn subst(self) -> &'static str {
        match self {
            Mode::Con => "con",
            Mode::Ctf => "ctf",
            Mode::OneFlag => "1flag",
            Mode::Hunt => "hunt",
            Mode::Assault => "ass",
            Mode::Eli => "eli",
        }
    }

    /// Name and description keys with the disc's English; eli shows as
    /// Assault (`modename.name.assault`, §6).
    fn text(self) -> ((&'static str, &'static str), (&'static str, &'static str)) {
        match self {
            Mode::Con => (
                ("modename.name.con", "Conquest"),
                (
                    "modename.description.con",
                    "Conquest\r\nDefend your command posts while attempting to capture the enemy's.",
                ),
            ),
            Mode::Ctf => (
                ("modename.name.ctf", "2-flag CTF"),
                (
                    "modename.description.ctf",
                    "Capture The Flag (2 Flag)\r\nDefend your team flag while attempting to steal the enemy's.",
                ),
            ),
            Mode::OneFlag => (
                ("modename.name.1flag", "1-flag CTF"),
                (
                    "modename.description.1flag",
                    "Capture The Flag (1 Flag)\r\nBoth teams attempt to take a neutral flag and return it to the enemy base.",
                ),
            ),
            Mode::Hunt => (
                ("modename.name.hunt", "Hunt"),
                (
                    "modename.description.hunt",
                    "Hunt\r\nEliminate as many of the indigenous species as possible before time runs out.",
                ),
            ),
            Mode::Assault => (
                ("modename.name.assault", "Assault"),
                (
                    "modename.description.assault",
                    "Assault\r\nDestroy enemy critical systems (18 points), frigates (18 points), and fighters (3 points).",
                ),
            ),
            Mode::Eli => (
                ("modename.name.assault", "Assault"),
                (
                    "modename.description.elimination",
                    "Assault\r\nHeroes and villains clash sabers and exchange fire in the ultimate duel of the fates.",
                ),
            ),
        }
    }

    /// The row's icon (SHELL.LVL, §6; eli draws Assault's).
    pub fn icon(self) -> &'static str {
        match self {
            Mode::Con => "mode_icon_con",
            Mode::Ctf => "mode_icon_2ctf",
            Mode::OneFlag => "mode_icon_ctf",
            Mode::Hunt => "mode_icon_hunt",
            Mode::Assault | Mode::Eli => "mode_icon_ass",
        }
    }
}

/// The Era list's pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Era(Era),
    /// `[ours]` an index into [`MIXED`].
    Mixed(usize),
}

impl Side {
    pub fn armies(self) -> [&'static str; 2] {
        match self {
            Side::Era(e) => e.armies(),
            Side::Mixed(i) => MIXED[i],
        }
    }
}

/// One playlist row (`gPickedMapList` minus "Remove All").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// An index into [`InstantAction::maps`].
    pub map: usize,
    pub mode: Mode,
    pub side: Side,
}

/// What LAUNCH hands the game, one per playlist row in order (§1.4, §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub key: String,
    pub game: Game,
    pub mode: Mode,
    pub armies: [String; 2],
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapPick {
    One(usize),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModePick {
    One(Mode),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePick {
    One(Side),
    AllEras,
}

/// `ifs_ms_state` (§1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Map,
    Mode,
    Era,
    Command,
    Playlist,
}

/// The command stack, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Launch,
    Add,
    Remove,
    Options,
}

pub const COMMANDS: [Cmd; 4] = [Cmd::Launch, Cmd::Add, Cmd::Remove, Cmd::Options];

impl Cmd {
    fn text(self) -> ((&'static str, &'static str), (&'static str, &'static str)) {
        match self {
            Cmd::Launch => (
                ("ifs.missionselect.buttons.text.launch", "Launch"),
                (
                    "ifs.missionselect.buttons.help.launch",
                    "Launch the current playlist.",
                ),
            ),
            Cmd::Add => (
                ("ifs.missionselect.buttons.text.add", "Add"),
                (
                    "ifs.missionselect.buttons.help.add",
                    "Add a map to the playlist.",
                ),
            ),
            Cmd::Remove => (
                ("ifs.missionselect.buttons.text.remove", "Remove"),
                (
                    "ifs.missionselect.buttons.help.remove",
                    "Remove a map from the playlist.",
                ),
            ),
            Cmd::Options => (
                ("ifs.missionselect.buttons.text.options", "Options"),
                (
                    "ifs.missionselect.buttons.help.options",
                    "Modify the session options.",
                ),
            ),
        }
    }
}

/// A list's focused row and its first shown row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListCursor {
    pub at: usize,
    pub top: usize,
}

impl ListCursor {
    /// Move by `step` rows over `live` (each row's enabled flag), no wrap;
    /// `[ours]` dimmed rows are skipped (§7); `shown` rows are visible.
    fn step(&mut self, live: &[bool], step: isize, shown: usize) -> bool {
        let n = live.len() as isize;
        if n == 0 {
            return false;
        }
        let target = (self.at as isize + step).clamp(0, n - 1);
        let dir = step.signum();
        // The nearest live row from the target on, else back towards here.
        let mut pick = None;
        let mut k = target;
        while (0..n).contains(&k) && dir != 0 {
            if live[k as usize] {
                pick = Some(k);
                break;
            }
            k += dir;
        }
        if pick.is_none() {
            let mut k = target;
            while k != self.at as isize && dir != 0 {
                if live[k as usize] {
                    pick = Some(k);
                    break;
                }
                k -= dir;
            }
        }
        let moved = matches!(pick, Some(p) if p as usize != self.at);
        if let Some(p) = pick {
            self.at = p as usize;
        }
        self.show(n as usize, shown);
        moved
    }

    /// The first live row (or 0), shown.
    fn first(live: &[bool], shown: usize) -> ListCursor {
        let mut c = ListCursor {
            at: live.iter().position(|l| *l).unwrap_or(0),
            top: 0,
        };
        c.show(live.len(), shown);
        c
    }

    pub(crate) fn show(&mut self, n: usize, shown: usize) {
        if self.at < self.top {
            self.top = self.at;
        }
        if self.at >= self.top + shown {
            self.top = self.at + 1 - shown;
        }
        self.top = self.top.min(n.saturating_sub(shown));
    }
    /// Scroll `top` by `delta` rows without moving `at`.
    /// `delta` > 0 scrolls down (higher top, showing later items).
    /// `delta` < 0 scrolls up (lower top, showing earlier items).
    pub fn scroll(&mut self, total_rows: usize, shown: usize, delta: isize) -> bool {
        let max_top = total_rows.saturating_sub(shown);
        let new_top = (self.top as isize + delta).clamp(0, max_top as isize) as usize;
        if new_top != self.top {
            self.top = new_top;
            true
        } else {
            false
        }
    }
}

/// A list row as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    pub text: String,
    pub enabled: bool,
    /// The right-edge icon (a mode's, an era's) and the one left of it (a
    /// playlist row's era icon).
    pub icon: Option<&'static str>,
    pub icon2: Option<&'static str>,
}

/// The page up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Missions,
    /// "Session Options": the focused button.
    Overview(usize),
    Group(Group, ListCursor),
}

/// A frame's presses: the menu edges plus L1 / R1.
#[derive(Debug, Clone, Copy, Default)]
pub struct IaInput {
    pub menu: MenuInput,
    pub l1: bool,
    pub r1: bool,
}

/// A visible target on the Instant Action screen, in menu model coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerTarget {
    Command(usize),
    Map(usize),
    Mode(usize),
    Era(usize),
    Playlist(usize),
    Overview(usize),
    Option(usize, bool),
    ScrollLeft(isize),
    ScrollRight(isize),
    ScrollOption(isize),
}

/// What a press asks the game to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IaCommand {
    /// Triangle in the command state: back to Singleplayer.
    Back,
    /// LAUNCH: the playlist, in order (`bRandomOrder` is MP-only, §1.4).
    Launch(Vec<Launch>),
    /// An options page was left (`push_prefs`): store what must outlive
    /// the session (the AI difficulty is the profile's).
    Prefs,
}

/// The Instant Action screen and its Session Options.
#[derive(Debug, Clone, PartialEq)]
pub struct InstantAction {
    pub maps: Vec<MapEntry>,
    pub texts: IaTexts,
    pub state: State,
    /// The command stack's focus (kept while a list has it, f11).
    pub command: usize,
    pub map_list: ListCursor,
    pub mode_list: ListCursor,
    pub era_list: ListCursor,
    pub play_list: ListCursor,
    pub picked_map: MapPick,
    pub picked_mode: ModePick,
    pub playlist: Vec<Entry>,
    pub options: SessionOptions,
    pub page: Page,
    /// "Selected maplist is full." is up (`Popup_Ok`, §1.4).
    pub full: bool,
    /// Seconds on the current page (the drawing's clock).
    pub on_page: f32,
    /// The clicks the presses asked for, for the game to play
    /// ([`InstantAction::take_clicks`]).
    clicks: Vec<MenuSound>,
}

impl InstantAction {
    /// Put the mouse focus on a visible, enabled row. Keyboard/controller
    /// input still uses the same cursors and confirmation path.
    /// Scroll the active or left list (Planet / Mode / Era) by `delta` rows.
    pub fn scroll_left(&mut self, delta: isize) -> bool {
        let (len, shown) = match self.state {
            State::Mode => (self.mode_rows().len(), LIST_ROWS),
            State::Era => (self.side_rows().len(), ERA_ROWS),
            _ => (self.map_rows().len(), LIST_ROWS),
        };
        let moved = match self.state {
            State::Mode => self.mode_list.scroll(len, shown, delta),
            State::Era => self.era_list.scroll(len, shown, delta),
            _ => self.map_list.scroll(len, shown, delta),
        };
        if moved {
            self.click(MenuSound::Select);
        }
        moved
    }

    /// Scroll the playlist by `delta` rows.
    pub fn scroll_right(&mut self, delta: isize) -> bool {
        let moved = self
            .play_list
            .scroll(self.playlist.len() + 1, LIST_ROWS, delta);
        if moved {
            self.click(MenuSound::Select);
        }
        moved
    }

    /// Scroll the session options group list by `delta` rows.
    pub fn scroll_option(&mut self, delta: isize) -> bool {
        if let Page::Group(g, mut cur) = self.page {
            let moved = cur.scroll(self.options.rows(g).len(), OPTION_ROWS, delta);
            if moved {
                self.page = Page::Group(g, cur);
                self.click(MenuSound::Select);
            }
            moved
        } else {
            false
        }
    }

    /// Scroll at a pointer location (window coordinates `point` in window `size`).
    pub fn scroll_pointer(
        &mut self,
        point: Option<[f32; 2]>,
        size: Option<[f32; 2]>,
        delta: isize,
    ) -> bool {
        if delta == 0 {
            return false;
        }
        match self.page {
            Page::Missions => {
                if let (Some(point), Some(size)) = (point, size) {
                    if size[0] > 0.0 && size[1] > 0.0 {
                        let box_w = 4.0 / 3.0 * size[1];
                        let x = (point[0] - (size[0] - box_w) * 0.5) / (box_w / 512.0);
                        if (crate::shell_ia::RIGHT_WINDOW[0]..=crate::shell_ia::RIGHT_WINDOW[2])
                            .contains(&x)
                        {
                            return self.scroll_right(delta);
                        }
                        if (crate::shell_ia::LEFT_WINDOW[0]..=crate::shell_ia::LEFT_WINDOW[2])
                            .contains(&x)
                        {
                            return self.scroll_left(delta);
                        }
                    }
                }
                if self.state == State::Playlist {
                    self.scroll_right(delta)
                } else {
                    self.scroll_left(delta)
                }
            }
            Page::Group(..) => self.scroll_option(delta),
            _ => false,
        }
    }

    pub fn focus_pointer(&mut self, target: PointerTarget) -> bool {
        match target {
            PointerTarget::Command(k) if COMMANDS.get(k).is_some_and(|c| self.command_live(*c)) => {
                self.command = k;
                true
            }
            PointerTarget::Map(k) if self.map_rows().get(k).is_some_and(|r| r.enabled) => {
                self.state = State::Map;
                self.map_list.at = k;
                self.map_list.show(self.map_rows().len(), LIST_ROWS);
                true
            }
            PointerTarget::Mode(k) if self.mode_rows().get(k).is_some_and(|r| r.enabled) => {
                self.state = State::Mode;
                self.mode_list.at = k;
                self.mode_list.show(self.mode_rows().len(), LIST_ROWS);
                true
            }
            PointerTarget::Era(k) if self.side_rows().get(k).is_some_and(|r| r.enabled) => {
                self.state = State::Era;
                self.era_list.at = k;
                self.era_list.show(self.side_rows().len(), ERA_ROWS);
                true
            }
            PointerTarget::Playlist(k) if k <= self.playlist.len() => {
                self.state = State::Playlist;
                self.play_list.at = k;
                self.play_list.show(self.playlist.len() + 1, LIST_ROWS);
                true
            }
            PointerTarget::Overview(k)
                if self.page == Page::Overview(k)
                    || (self.overview().get(k).is_some_and(|(_, live)| *live)) =>
            {
                self.page = Page::Overview(k);
                true
            }
            PointerTarget::Option(k, _) => {
                if let Page::Group(g, mut cur) = self.page {
                    if k < self.options.rows(g).len() {
                        cur.at = k;
                        cur.show(self.options.rows(g).len(), OPTION_ROWS);
                        self.page = Page::Group(g, cur);
                        return true;
                    }
                }
                false
            }
            PointerTarget::ScrollLeft(_)
            | PointerTarget::ScrollRight(_)
            | PointerTarget::ScrollOption(_) => true,
            _ => false,
        }
    }
    /// The screen's first Enter (§1.3): an empty playlist, the command
    /// state, focus on the first live button (ADD).
    pub fn new(maps: Vec<MapEntry>, texts: IaTexts, options: SessionOptions) -> InstantAction {
        let mut ia = InstantAction {
            maps,
            texts,
            state: State::Command,
            command: 0,
            map_list: ListCursor::default(),
            mode_list: ListCursor::default(),
            era_list: ListCursor::default(),
            play_list: ListCursor::default(),
            picked_map: MapPick::All,
            picked_mode: ModePick::All,
            playlist: Vec::new(),
            options,
            page: Page::Missions,
            full: false,
            on_page: 0.0,
            clicks: Vec::new(),
        };
        ia.enter();
        ia
    }

    /// Every later Enter: the command state, the playlist and the session
    /// options kept (f24; menuref §9.1.5).
    pub fn enter(&mut self) {
        self.state = State::Command;
        self.page = Page::Missions;
        self.full = false;
        self.on_page = 0.0;
        self.command = self.first_command();
        let live = self.live(&self.map_rows());
        self.map_list = ListCursor::first(&live, LIST_ROWS);
    }

    // ---- the lists -------------------------------------------------------

    fn live(&self, rows: &[ListRow]) -> Vec<bool> {
        rows.iter().map(|r| r.enabled).collect()
    }

    /// Whether `map` with `mode` and `side` would add anything.
    fn adds(&self, map: MapPick, mode: ModePick, side: SidePick) -> bool {
        !self.expand(map, mode, side).is_empty()
    }

    /// The Planet list: every map, then "All Maps".
    pub fn map_picks(&self) -> Vec<MapPick> {
        (0..self.maps.len())
            .map(MapPick::One)
            .chain(std::iter::once(MapPick::All))
            .collect()
    }

    pub fn map_rows(&self) -> Vec<ListRow> {
        self.map_picks()
            .into_iter()
            .map(|p| ListRow {
                text: match p {
                    MapPick::One(i) => self.maps[i].label.clone(),
                    MapPick::All => self.texts.all_maps.clone(),
                },
                enabled: self.adds(p, ModePick::All, SidePick::AllEras)
                    || self.mixed_adds(p, ModePick::All),
                icon: None,
                icon2: None,
            })
            .collect()
    }

    fn mixed_adds(&self, map: MapPick, mode: ModePick) -> bool {
        (0..MIXED.len()).any(|i| self.adds(map, mode, SidePick::One(Side::Mixed(i))))
    }

    /// The modes `map` has, in `gMapModes` order (All Maps: any map's).
    fn modes_of(&self, map: MapPick) -> Vec<Mode> {
        match map {
            MapPick::One(i) => self.maps[i].all_modes(),
            MapPick::All => MODES
                .into_iter()
                .filter(|m| self.maps.iter().any(|e| e.all_modes().contains(m)))
                .collect(),
        }
    }

    /// The Mode list: the map's modes, then "All Modes" when it has more
    /// than one (f2 pc 123-167).
    pub fn mode_picks(&self) -> Vec<ModePick> {
        let modes = self.modes_of(self.picked_map);
        let many = modes.len() > 1;
        modes
            .into_iter()
            .map(ModePick::One)
            .chain(many.then_some(ModePick::All))
            .collect()
    }

    pub fn mode_rows(&self) -> Vec<ListRow> {
        let picks = self.mode_picks();
        let live_modes = picks
            .iter()
            .filter(|p| matches!(p, ModePick::One(_)) && self.mode_live(**p))
            .count();
        picks
            .iter()
            .map(|&p| match p {
                ModePick::One(m) => ListRow {
                    text: self.texts.mode(m).0.to_owned(),
                    enabled: self.mode_live(p),
                    icon: Some(m.icon()),
                    icon2: None,
                },
                // §7: dimmed when only one mode is live.
                ModePick::All => ListRow {
                    text: self.texts.all_modes.clone(),
                    enabled: live_modes > 1,
                    icon: None,
                    icon2: None,
                },
            })
            .collect()
    }

    fn mode_live(&self, mode: ModePick) -> bool {
        self.adds(self.picked_map, mode, SidePick::AllEras)
            || self.mixed_adds(self.picked_map, mode)
    }

    /// The Era list: the eras that have the picked mode (any mode after All
    /// Modes) and "All Eras" when more than one (f3 pc 87-143). The `[ours]`
    /// cross-era pairs ([`MIXED`]) are not listed for now (owner's call,
    /// 2026-10-28: Clone Wars and Galactic Civil War only).
    pub fn side_picks(&self) -> Vec<SidePick> {
        let has = |e: Era| {
            let maps: Vec<&MapEntry> = match self.picked_map {
                MapPick::One(i) => vec![&self.maps[i]],
                MapPick::All => self.maps.iter().collect(),
            };
            maps.iter().any(|m| match self.picked_mode {
                ModePick::One(mode) => m.modes[e.index()].contains(&mode),
                ModePick::All => m.has_era(e),
            })
        };
        let eras: Vec<Era> = ERAS.into_iter().filter(|e| has(*e)).collect();
        let many = eras.len() > 1;
        eras.into_iter()
            .map(|e| SidePick::One(Side::Era(e)))
            .chain(many.then_some(SidePick::AllEras))
            .collect()
    }

    pub fn side_rows(&self) -> Vec<ListRow> {
        let picks = self.side_picks();
        let live = |p: SidePick| self.adds(self.picked_map, self.picked_mode, p);
        let live_eras = picks
            .iter()
            .filter(|p| matches!(p, SidePick::One(Side::Era(_))) && live(**p))
            .count();
        picks
            .iter()
            .map(|&p| match p {
                SidePick::One(s) => ListRow {
                    text: self.texts.side(s),
                    enabled: live(p),
                    icon: match s {
                        Side::Era(e) => Some(e.icon()),
                        Side::Mixed(_) => None,
                    },
                    icon2: None,
                },
                // `[ours]` like All Modes: dimmed with one live era.
                SidePick::AllEras => ListRow {
                    text: self.texts.all_eras.clone(),
                    enabled: live_eras > 1,
                    icon: None,
                    icon2: None,
                },
            })
            .collect()
    }

    /// The Playlist: "Remove All", then the rows (map name, mode icon, era
    /// icon left of it; f3).
    pub fn play_rows(&self) -> Vec<ListRow> {
        std::iter::once(ListRow {
            text: self.texts.remove_all.clone(),
            enabled: true,
            icon: None,
            icon2: None,
        })
        .chain(self.playlist.iter().map(|e| ListRow {
            text: self.maps[e.map].label.clone(),
            enabled: true,
            icon: Some(e.mode.icon()),
            icon2: match e.side {
                Side::Era(era) => Some(era.icon()),
                Side::Mixed(_) => None,
            },
        }))
        .collect()
    }

    /// The rows `fnAddMap1..4` would add (§1.4): maps in list order, eras
    /// outer, modes inner; `[ours]` only what the port can run.
    pub fn expand(&self, map: MapPick, mode: ModePick, side: SidePick) -> Vec<Entry> {
        let maps: Vec<usize> = match map {
            MapPick::One(i) => vec![i],
            MapPick::All => (0..self.maps.len()).collect(),
        };
        let sides: Vec<Side> = match side {
            SidePick::One(s) => vec![s],
            SidePick::AllEras => ERAS.into_iter().map(Side::Era).collect(),
        };
        let modes: Vec<Mode> = match mode {
            ModePick::One(m) => vec![m],
            ModePick::All => MODES.to_vec(),
        };
        let mut out = Vec::new();
        for &i in &maps {
            let m = &self.maps[i];
            for &s in &sides {
                if let Side::Era(e) = s {
                    if !m.has_era(e) {
                        continue;
                    }
                }
                for &mode in &modes {
                    if m.runnable(mode, s) {
                        out.push(Entry {
                            map: i,
                            mode,
                            side: s,
                        });
                    }
                }
            }
        }
        out
    }

    // ---- the command stack ----------------------------------------------

    /// f11: LAUNCH is live with a map in the playlist; in a list state all
    /// four are dimmed.
    pub fn command_live(&self, c: Cmd) -> bool {
        self.state == State::Command && (c != Cmd::Launch || !self.playlist.is_empty())
    }

    fn first_command(&self) -> usize {
        let launch = !self.playlist.is_empty();
        if launch {
            0
        } else {
            1
        }
    }

    fn focus_command(&mut self, c: Cmd) {
        self.command = COMMANDS.iter().position(|x| *x == c).unwrap_or(1);
    }

    /// The info box's text now (f9).
    pub fn info(&self) -> String {
        let at = |c: &ListCursor| c.at;
        match self.state {
            State::Command => self.texts.command(COMMANDS[self.command]).1.to_owned(),
            State::Map => match self.map_picks().get(at(&self.map_list)) {
                Some(MapPick::One(i)) => self.maps[*i].description.clone(),
                _ => self.texts.all_maps_desc.clone(),
            },
            State::Mode => match self.mode_picks().get(at(&self.mode_list)) {
                Some(ModePick::One(m)) => self.texts.mode(*m).1.to_owned(),
                _ => self.texts.all_modes_desc.clone(),
            },
            State::Era => match self.side_picks().get(at(&self.era_list)) {
                Some(SidePick::One(s)) => self.texts.side(*s),
                _ => self.texts.all_eras.clone(),
            },
            State::Playlist => match at(&self.play_list) {
                0 => String::new(),
                i => self
                    .playlist
                    .get(i - 1)
                    .map_or_else(String::new, |e| self.maps[e.map].label.clone()),
            },
        }
    }

    /// The overview's buttons and whether each is live.
    pub fn overview(&self) -> Vec<(Group, bool)> {
        GROUPS.iter().map(|g| (*g, g.honoured())).collect()
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.on_page = 0.0;
    }

    fn click(&mut self, s: MenuSound) {
        self.clicks.push(s);
    }

    /// The clicks asked for since the last call, oldest first. The table is
    /// menuref's "Sound table": a stack move clicks select (the wrap too); a
    /// list move only when the cursor moves; a Session Options value step
    /// select always, even at a clamp; Cross accept, on "Remove All" error;
    /// Triangle one exit click (the command state's is the Singleplayer
    /// stack's re-Enter, not here); Cross on a group page nothing.
    pub fn take_clicks(&mut self) -> Vec<MenuSound> {
        std::mem::take(&mut self.clicks)
    }

    /// `fnAddMap` (§1.4): the rows appended until the list holds
    /// [`MAX_MISSIONS`]; one more opens "Selected maplist is full.".
    pub fn add(&mut self, map: MapPick, mode: ModePick, side: SidePick) {
        for e in self.expand(map, mode, side) {
            if self.playlist.len() >= MAX_MISSIONS {
                self.full = true;
                break;
            }
            self.playlist.push(e);
            let n = self.playlist.len() + 1;
            self.play_list.at = n - 1;
            self.play_list.show(n, LIST_ROWS);
        }
    }

    /// The playlist as LAUNCH hands it over, in order.
    pub fn launch(&self) -> Vec<Launch> {
        self.playlist
            .iter()
            .map(|e| {
                let m = &self.maps[e.map];
                Launch {
                    key: m.key.clone(),
                    game: m.game,
                    mode: e.mode,
                    armies: e.side.armies().map(str::to_owned),
                    label: m.label.clone(),
                }
            })
            .collect()
    }
}

mod catalog;
pub use catalog::*;
mod options;
pub use options::*;
mod texts;
pub use texts::*;
mod keys;

#[cfg(test)]
mod tests;
