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
    State, ERAS, GROUPS,
};

/// How long the host waits for every player's `Ready` after a round.
pub const READY_WAIT_SECS: f32 = 60.0;

/// The discovery reply's blob: what Join Game lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInfo {
    pub name: String,
    /// The map in play or next (its label), empty in the lobby.
    pub map: String,
    /// The mode in play ("Conquest"), empty in the lobby.
    pub mode: String,
    pub players: u8,
    pub max_players: u8,
    pub protocol: u16,
    pub build: String,
    /// A random number per hosting session: one host heard at several
    /// addresses (its LAN and its VPN one) is listed once.
    pub session: u64,
}

/// The part of [`HostInfo`] every build sends ([`HostInfo::decode_any`]).
#[derive(Deserialize)]
struct HostInfoCore {
    name: String,
    protocol: u16,
    build: String,
}

impl HostInfo {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("host info encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// The blob of any openbf build: the whole info when it is this
    /// build's, else just the name, protocol and build every build sends
    /// (so another version's host is listed as one, not dropped).
    pub fn decode_any(bytes: &[u8]) -> Option<Self> {
        Self::decode(bytes).or_else(|| {
            let c: HostInfoCore = serde_json::from_slice(bytes).ok()?;
            Some(Self {
                name: c.name,
                protocol: c.protocol,
                build: c.build,
                ..Default::default()
            })
        })
    }

    /// A discovery answer's info; the protocol is the answer's own (a host
    /// on another version answers in its version). An unreadable blob is
    /// an unnamed host of that protocol.
    pub fn of_found(h: &net::FoundHost) -> Self {
        let mut info = Self::decode_any(&h.info).unwrap_or_default();
        info.protocol = h.protocol;
        info
    }

    /// Why this build cannot join it, if it cannot.
    pub fn refusal(&self, build: &str) -> Option<String> {
        if self.protocol != net::PROTOCOL || self.build != build {
            Some("different version".into())
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

/// Diagnostic explaining when a guest's active gameplay-affecting content mods
/// or override stack differ from the host (or cannot be verified).
pub fn mod_mismatch_reason(host: &ContentManifest, guest: &ContentManifest) -> Option<String> {
    if host.override_digest != guest.override_digest {
        return Some(
            "Active gameplay-affecting mods or load order differ from host (match mods and load order in Mods tab).".into(),
        );
    }
    None
}

/// Conservative check for a guest's decoded [`ContentManifest`]: rejects
/// unreadable manifests whenever the host has active gameplay overrides.
pub fn guest_mod_mismatch(
    host: &ContentManifest,
    guest: Option<&ContentManifest>,
) -> Option<String> {
    match guest {
        Some(g) => mod_mismatch_reason(host, g),
        None if !host.override_digest.is_empty() => Some(
            "Could not verify your active mods against the host's gameplay-affecting mod stack."
                .into(),
        ),
        None => None,
    }
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
        R::Full => match net::host::parse_full_detail(detail) {
            // A machine bringing several players is let in all or none.
            Some((room, wanted)) if room > 0 && wanted > room => format!(
                "The game has room for {room} more player{}; {wanted} are on this computer. \
                 Join with fewer players",
                if room == 1 { "" } else { "s" }
            ),
            _ => "The game is full".into(),
        },
        R::Busy => "The host is loading a battle; try again in a moment".into(),
        R::Banned => "The host removed you from this game".into(),
        R::BadName => "The host refused your player name (Options → Game Options)".into(),
        R::Kicked if !detail.is_empty() => detail.to_owned(),
        R::Kicked => "The host removed you from the game".into(),
    }
}

/// A further player of the host's own machine the host core refused
/// (`HostCore::add_local`), in [`reject_text`]'s words: the lobby line for
/// that player.
pub fn local_refusal_text(name: &str, reason: net::RejectReason) -> String {
    use net::RejectReason as R;
    match reason {
        R::Full => format!("{name} could not join: the game is full"),
        R::Busy => {
            format!("{name} could not join: the host is loading a battle; try again in a moment")
        }
        R::BadName => format!(
            "{name} could not join: the player name is not allowed (Options → Game Options)"
        ),
        // The core refuses a host local only for the three above.
        R::Version | R::Banned | R::Kicked => format!("{name} could not join this game"),
    }
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideKey {
    Era(u8),
    CustomEra(String),
    Mixed(u8),
}

impl SideKey {
    fn of(s: &Side) -> Self {
        match s {
            Side::Era(Era::Custom(code)) => SideKey::CustomEra(code.clone()),
            Side::Era(e) => SideKey::Era(ERAS.iter().position(|x| x == e).unwrap_or(0) as u8),
            Side::Mixed(i) => SideKey::Mixed(*i as u8),
        }
    }

    fn side(&self) -> Option<Side> {
        match self {
            SideKey::Era(i) => ERAS.get(usize::from(*i)).map(|e| Side::Era(e.clone())),
            SideKey::CustomEra(code) => Era::from_code(code).map(Side::Era),
            SideKey::Mixed(i) => {
                (usize::from(*i) < ui::shell::MIXED.len()).then_some(Side::Mixed(usize::from(*i)))
            }
        }
    }
}

fn mode_of(subst: &str) -> Option<Mode> {
    Mode::from_subst(subst)
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

/// One player in the roster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub slot: u8,
    /// Its machine (splitscreen, issue #194 B19): the first slot of the
    /// machine it plays on, 0 for the host's own players.
    #[serde(default)]
    pub machine: u8,
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

/// A cursor on the wire: its row and the first row its view rests on.
fn cur(c: ListCursor) -> (u16, u16) {
    (c.at as u16, c.view.target().round() as u16)
}

fn uncur((at, top): (u16, u16)) -> ListCursor {
    ListCursor {
        at: usize::from(at),
        view: ui::scroll::ScrollView::at(f32::from(top)),
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
                top: c.view.target().round() as u16,
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
                MapPick::Folder(_) | MapPick::All => None,
            },
            picked_mode: match &ia.picked_mode {
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
            side: SideKey::of(&e.side),
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
            MapPick::Folder(_) | MapPick::All => None,
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
