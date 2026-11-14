//! The multiplayer lobby's data (LAN §4.1-§4.3, §4.8): what the host shows
//! its guests ([`LobbyState`], the guests draw the host's Instant Action
//! screen read-only from it), the discovery blob ([`HostInfo`]), the host's
//! notices to one guest ([`GameNotice`]), the content restriction of the
//! host's map list and the texts a guest is refused or benched with.
//!
//! Rules:
//! - Maps travel by **key**, never by a local index: the host's offered
//!   list ([`LobbyState::maps`], already the intersection of everyone's
//!   content) and its playlist rows. A guest builds its screen's map list
//!   from its own catalog in the host's order, so every list index, cursor
//!   and pick lines up; a key it lacks is dropped from its copy.
//! - The host's list is the intersection ([`super::ContentManifest::intersect`]):
//!   [`restrict`] rebuilds it from the full catalog whenever the
//!   intersection changes, keeps the playlist rows whose key survives and
//!   names the rest; the addon heroes switch is greyed and off unless every
//!   player has them.
//! - Everything is JSON (`serde_json`), like the round setup.
//! - Nothing here touches a sim.

use super::ContentManifest;
use serde::{Deserialize, Serialize};
use ui::shell::{
    Entry, Era, Group, InstantAction, ListCursor, MapEntry, MapPick, Mode, ModePick, Page, Side,
    State, ERAS, GROUPS, MODES,
};

/// How many recent hosts the profile keeps.
pub const RECENT_HOSTS: usize = 8;
/// How long the host waits for every player's `Ready` after a round.
pub const READY_WAIT_SECS: f32 = 60.0;

/// The discovery reply's blob: what Join Game lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub name: String,
    /// The map in play or next (its label), empty in the lobby.
    pub map: String,
    pub players: u8,
    pub max_players: u8,
    pub protocol: u16,
    pub build: String,
}

impl HostInfo {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("host info encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// Why this build cannot join it, if it cannot.
    pub fn refusal(&self, build: &str) -> Option<String> {
        if self.protocol != net::PROTOCOL || self.build != build {
            Some("another openbf build".into())
        } else if self.players >= self.max_players {
            Some("full".into())
        } else {
            None
        }
    }
}

/// A host's notice to one guest (a `Msg::Notice`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameNotice {
    /// The guest's built round differs from the host's (§4.3): it waits in
    /// the lobby; `groups` are the host's static digest groups, so the
    /// guest can name the first that differs.
    DigestMismatch {
        map: String,
        groups: Vec<(String, u64)>,
    },
    /// Back to the lobby with this line (no copy of the map in play...).
    BackToLobby(String),
}

impl GameNotice {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("notice encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// "Your copy of <map> plays differently from the host's (<group>)": the
/// first group whose digest differs, by the host's order.
pub fn digest_mismatch_text(map: &str, host: &[(String, u64)], ours: &[(&str, u64)]) -> String {
    let group = host
        .iter()
        .find(|(name, d)| {
            ours.iter()
                .find(|(n, _)| n == name)
                .is_none_or(|(_, o)| o != d)
        })
        .map_or("unknown", |(name, _)| name.as_str());
    format!("Your copy of {map} plays differently from the host's ({group})")
}

/// A refused or ended connection as the player reads it.
pub fn reject_text(reason: net::RejectReason, detail: &str, ours: &str) -> String {
    use net::RejectReason as R;
    match reason {
        R::Version => {
            // The cores say "<theirs>, host <the host's>".
            let host = detail
                .rsplit_once(", host ")
                .map_or("unknown", |(_, h)| h)
                .trim();
            if detail.starts_with("protocol") {
                format!(
                    "The host runs a different openbf build (host: protocol {host}, you: protocol {})",
                    net::PROTOCOL
                )
            } else {
                format!("The host runs a different openbf build (host: {host}, you: {ours})")
            }
        }
        R::Full => "The game is full".into(),
        R::Busy => "The host is loading a battle; try again in a moment".into(),
        R::Banned => "The host removed you from this game".into(),
        R::BadName => "The host refused your player name (Options → Game Options)".into(),
        R::Kicked if !detail.is_empty() => detail.to_owned(),
        R::Kicked => "The host removed you from the game".into(),
    }
}

/// Puts `addr` first in the recent hosts (once), keeping [`RECENT_HOSTS`].
pub fn remember_host(recent: &mut Vec<String>, addr: &str) {
    let addr = addr.trim();
    if addr.is_empty() {
        return;
    }
    recent.retain(|a| !a.eq_ignore_ascii_case(addr));
    recent.insert(0, addr.to_owned());
    recent.truncate(RECENT_HOSTS);
}

/// The player name to use: the profile's when it is valid
/// (`net::msg::valid_name`), else the OS user's, else "Player".
pub fn player_name(profile_name: &str) -> String {
    let fit = |s: &str| -> Option<String> {
        let s: String = s.trim().chars().take(net::msg::MAX_NAME_CHARS).collect();
        net::msg::valid_name(&s).then_some(s)
    };
    fit(profile_name)
        .or_else(|| std::env::var("USER").ok().as_deref().and_then(fit))
        .or_else(|| std::env::var("USERNAME").ok().as_deref().and_then(fit))
        .unwrap_or_else(|| "Player".into())
}

/// A playlist row by key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyEntry {
    pub map: String,
    /// [`Mode::subst`].
    pub mode: String,
    pub side: SideKey,
}

/// [`Side`] as data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideKey {
    Era(u8),
    Mixed(u8),
}

impl SideKey {
    fn of(s: Side) -> Self {
        match s {
            Side::Era(e) => SideKey::Era(ERAS.iter().position(|x| *x == e).unwrap_or(0) as u8),
            Side::Mixed(i) => SideKey::Mixed(i as u8),
        }
    }

    fn side(self) -> Option<Side> {
        match self {
            SideKey::Era(i) => ERAS.get(usize::from(i)).map(|e: &Era| Side::Era(*e)),
            SideKey::Mixed(i) => {
                (usize::from(i) < ui::shell::MIXED.len()).then_some(Side::Mixed(usize::from(i)))
            }
        }
    }
}

fn mode_of(subst: &str) -> Option<Mode> {
    MODES.into_iter().find(|m| m.subst() == subst)
}

/// The host's page as data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageKey {
    Missions,
    Overview(u8),
    Group { group: u8, at: u16, top: u16 },
}

/// Where the session is (what the guests' screens say).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyPhase {
    /// The host is on its setup screens.
    #[default]
    Choosing,
    /// A round is in play (its map's label).
    InRound(String),
    /// The round's results: the slots that confirmed their last page.
    Results { ready: Vec<u8> },
}

fn awards_default() -> bool {
    true
}

/// One player in the roster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub slot: u8,
    pub name: String,
    /// `None` for the host.
    pub ping_ms: Option<u32>,
    pub addon: bool,
    /// The side a player starts on: `None` = the side select (Instant
    /// Action's).
    pub team: Option<u8>,
}

/// The session options a guest's screen shows ([`super::RoundOptions`]
/// plus the setup-only fields).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyOptions {
    pub round: super::RoundOptions,
    pub num_bots: i32,
    pub difficulty: i32,
    pub dlc_heroes_installed: bool,
}

/// The host's shared setup (LAN §4.2; module rules).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyState {
    pub phase: LobbyPhase,
    /// The maps offered, by key, in the host's list order.
    pub maps: Vec<String>,
    pub playlist: Vec<LobbyEntry>,
    pub page: PageKey,
    /// [`State`] as its index (Map, Mode, Era, Command, Playlist).
    pub state: u8,
    pub command: u8,
    /// The four lists' cursors (at, top): map, mode, era, playlist.
    pub lists: [(u16, u16); 4],
    pub picked_map: Option<String>,
    pub picked_mode: Option<String>,
    pub options: LobbyOptions,
    pub cheat_policy: u32,
    pub lag_comp: bool,
    /// The award policy (LAN §5b; `RoundSetup::awards`).
    #[serde(default = "awards_default")]
    pub awards: bool,
    pub max_players: u8,
    pub roster: Vec<LobbyPlayer>,
    /// The latest lobby message (an entry removed, a player benched).
    pub message: Option<String>,
}

const STATES: [State; 5] = [
    State::Map,
    State::Mode,
    State::Era,
    State::Command,
    State::Playlist,
];

fn cur(c: ListCursor) -> (u16, u16) {
    (c.at as u16, c.top as u16)
}

fn uncur((at, top): (u16, u16)) -> ListCursor {
    ListCursor {
        at: usize::from(at),
        top: usize::from(top),
    }
}

impl LobbyState {
    /// The host's screen `ia` as the guests see it (the rest is the
    /// caller's: phase, policy, roster, message).
    pub fn from_ia(ia: &InstantAction) -> Self {
        let key = |i: usize| ia.maps[i].key.clone();
        let page = match ia.page {
            Page::Missions => PageKey::Missions,
            Page::Overview(k) => PageKey::Overview(k as u8),
            Page::Group(g, c) => PageKey::Group {
                group: GROUPS.iter().position(|x| *x == g).unwrap_or(0) as u8,
                at: c.at as u16,
                top: c.top as u16,
            },
        };
        let o = &ia.options;
        Self {
            phase: LobbyPhase::Choosing,
            maps: ia.maps.iter().map(|m| m.key.clone()).collect(),
            playlist: playlist_keys(ia),
            page,
            state: STATES.iter().position(|s| *s == ia.state).unwrap_or(3) as u8,
            command: ia.command as u8,
            lists: [
                cur(ia.map_list),
                cur(ia.mode_list),
                cur(ia.era_list),
                cur(ia.play_list),
            ],
            picked_map: match ia.picked_map {
                MapPick::One(i) => Some(key(i)),
                MapPick::All => None,
            },
            picked_mode: match ia.picked_mode {
                ModePick::One(m) => Some(m.subst().to_owned()),
                ModePick::All => None,
            },
            options: LobbyOptions {
                round: super::RoundOptions::from_session(o),
                num_bots: o.num_bots,
                difficulty: o.difficulty,
                dlc_heroes_installed: o.dlc_heroes_installed,
            },
            cheat_policy: 0,
            lag_comp: true,
            awards: true,
            max_players: 0,
            roster: Vec::new(),
            message: None,
        }
    }

    /// Draws the host's screen into a guest's `ia`: its map list becomes
    /// `full` (the guest's own catalog) in the host's order; returns the
    /// host's keys this copy lacks.
    pub fn apply_to(&self, ia: &mut InstantAction, full: &[MapEntry]) -> Vec<String> {
        let mut missing = Vec::new();
        ia.maps = self
            .maps
            .iter()
            .filter_map(|k| {
                let m = full.iter().find(|m| m.key == *k && m.baked).cloned();
                if m.is_none() {
                    missing.push(k.clone());
                }
                m
            })
            .collect();
        ia.playlist = entries_from_keys(&ia.maps, &self.playlist).0;
        ia.page = match self.page {
            PageKey::Missions => Page::Missions,
            PageKey::Overview(k) => Page::Overview(usize::from(k)),
            PageKey::Group { group, at, top } => Page::Group(
                GROUPS
                    .get(usize::from(group))
                    .copied()
                    .unwrap_or(Group::Global),
                uncur((at, top)),
            ),
        };
        ia.state = STATES
            .get(usize::from(self.state))
            .copied()
            .unwrap_or(State::Command);
        ia.command = usize::from(self.command).min(ui::shell::COMMANDS.len() - 1);
        ia.map_list = uncur(self.lists[0]);
        ia.mode_list = uncur(self.lists[1]);
        ia.era_list = uncur(self.lists[2]);
        ia.play_list = uncur(self.lists[3]);
        ia.picked_map = self
            .picked_map
            .as_ref()
            .and_then(|k| ia.maps.iter().position(|m| m.key == *k))
            .map_or(MapPick::All, MapPick::One);
        ia.picked_mode = self
            .picked_mode
            .as_deref()
            .and_then(mode_of)
            .map_or(ModePick::All, ModePick::One);
        let o = &self.options;
        ia.options = o.round.to_session(ia.options);
        ia.options.num_bots = o.num_bots;
        ia.options.difficulty = o.difficulty;
        ia.options.dlc_heroes_installed = o.dlc_heroes_installed;
        ia.full = false;
        missing
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("lobby encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// `ia`'s playlist by key.
pub fn playlist_keys(ia: &InstantAction) -> Vec<LobbyEntry> {
    ia.playlist
        .iter()
        .map(|e| LobbyEntry {
            map: ia.maps[e.map].key.clone(),
            mode: e.mode.subst().to_owned(),
            side: SideKey::of(e.side),
        })
        .collect()
}

/// Playlist rows by key as rows of `maps`; the rows whose map (or mode,
/// side) is not there come back second.
pub fn entries_from_keys(maps: &[MapEntry], rows: &[LobbyEntry]) -> (Vec<Entry>, Vec<LobbyEntry>) {
    let mut kept = Vec::new();
    let mut lost = Vec::new();
    for r in rows {
        let found = maps.iter().position(|m| m.key == r.map).and_then(|map| {
            Some(Entry {
                map,
                mode: mode_of(&r.mode)?,
                side: r.side.side()?,
            })
        });
        match found {
            Some(e) => kept.push(e),
            None => lost.push(r.clone()),
        }
    }
    (kept, lost)
}

/// The host's screen limited to what everyone can play (module rules):
/// `full` is the host's whole catalog, `common` the intersection. Returns
/// the lobby lines for what was removed or switched off.
pub fn restrict(
    ia: &mut InstantAction,
    full: &[MapEntry],
    common: &ContentManifest,
    addon_here: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let rows = playlist_keys(ia);
    let label = |key: &str| {
        full.iter()
            .find(|m| m.key == key)
            .map_or_else(|| key.to_owned(), |m| m.label.clone())
    };
    let maps: Vec<MapEntry> = full
        .iter()
        .filter(|m| common.maps.contains(&m.key))
        .cloned()
        .collect();
    if maps
        .iter()
        .map(|m| &m.key)
        .ne(ia.maps.iter().map(|m| &m.key))
    {
        let picked = match ia.picked_map {
            MapPick::One(i) => ia.maps.get(i).map(|m| m.key.clone()),
            MapPick::All => None,
        };
        ia.maps = maps;
        let (kept, lost) = entries_from_keys(&ia.maps, &rows);
        ia.playlist = kept;
        for r in lost {
            lines.push(format!(
                "{} was removed from the playlist: not every player has it",
                label(&r.map)
            ));
        }
        ia.picked_map = picked
            .and_then(|k| ia.maps.iter().position(|m| m.key == k))
            .map_or(MapPick::All, MapPick::One);
        ia.enter();
        ia.play_list = ListCursor::default();
    }
    let addon = addon_here && common.addon_heroes;
    if ia.options.dlc_heroes_installed != addon {
        ia.options.dlc_heroes_installed = addon;
        if !addon && ia.options.dlc_heroes {
            lines.push("The addon heroes were switched off: not every player has them".into());
        }
        ia.options.dlc_heroes = addon && ia.options.dlc_heroes;
    }
    if !addon {
        ia.options.dlc_heroes = false;
    }
    lines
}

/// The session token this game process joins every host with (LAN §4.5):
/// one for the process, so a guest that leaves, drops or times out and
/// joins again gets its slot and name back for the host's 90 s.
pub fn session_token() -> u64 {
    static TOKEN: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *TOKEN.get_or_init(net::fresh_u64)
}

/// The host's next launched playlist row that every connected player can
/// play now (`common`, the intersection of the current roster's content;
/// LAN §4.2): rows a player who joined since Launch lacks are dropped with
/// a lobby line each, and the launched session's addon heroes switch off
/// unless everyone (the host included, `addon_here`) has them. `None`: the
/// playlist is over.
pub fn next_playable_row(
    playlist: &mut std::collections::VecDeque<ui::shell::Launch>,
    session: &mut ui::shell::SessionOptions,
    common: &ContentManifest,
    addon_here: bool,
    lines: &mut Vec<String>,
) -> Option<ui::shell::Launch> {
    let addon = addon_here && common.addon_heroes;
    if session.dlc_heroes && !addon {
        lines.push("The addon heroes were switched off: not every player has them".into());
    }
    session.dlc_heroes &= addon;
    session.dlc_heroes_installed &= addon;
    while let Some(row) = playlist.pop_front() {
        if common.maps.contains(&row.key) {
            return Some(row);
        }
        lines.push(format!(
            "{} was removed from the playlist: not every player has it",
            row.label
        ));
    }
    None
}

#[cfg(test)]
mod tests;
