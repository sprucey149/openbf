//! Recent games (issue #168): the LAN games this computer joined, kept in
//! `openbf_data/recent_games.ron` beside the profiles, and how Join Game
//! matches them to the hosts that answer.
//!
//! Rules:
//! - Per computer, not per profile: every profile on this install sees the
//!   same list. Newest first, one entry per address (letter case ignored),
//!   at most [`RECENT_GAMES`]. An entry is written when the host accepts
//!   the player (its welcome), never for an address that only resolved.
//!   An entry stays until the player forgets it.
//! - The file is written atomically (a temporary file, then a rename). A
//!   file that cannot be read is set aside as `.ron.bad` and the list starts
//!   empty; it never touches a profile.
//! - DEV-PHASE IMPORT, slated for removal at v1 with the profile's
//!   `recent_hosts`: when the file does not exist yet, the active profile's
//!   old address list seeds it once.
//! - Join Game asks each recent game's address by unicast (tunnels carry no
//!   broadcast). [`merge`] puts a host's answer on the recent game whose
//!   resolved address it came from; the other hosts are listed once each
//!   under "Games on this network", one row per hosting session however
//!   many of its addresses answered.
//! - Nothing here touches a sim.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// How many recent games this computer keeps.
pub const RECENT_GAMES: usize = 8;

/// The file's name in the data folder.
pub const FILE_NAME: &str = "recent_games.ron";

/// One game joined.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentGame {
    /// The address as the player joined it (typed text or `ip:port`).
    pub addr: String,
    /// The host's player name ("" unknown).
    pub name: String,
    /// The last map played there (its label, "" none yet).
    pub map: String,
    /// That map's mode ("Conquest", "" none yet).
    pub mode: String,
    /// When it was last joined (seconds since 1970; 0 unknown).
    pub last_joined_unix: u64,
}

/// This computer's recent games and where they are kept (`None`: nowhere,
/// as in tests).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentGames {
    pub games: Vec<RecentGame>,
    path: Option<PathBuf>,
}

/// Seconds since 1970 now.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "2 days ago" for a join at `then` seen at `now` ("" unknown).
pub fn ago_text(now: u64, then: u64) -> String {
    if then == 0 || then > now {
        return String::new();
    }
    let d = now - then;
    let (n, unit) = match d {
        0..60 => return "just now".into(),
        60..3_600 => (d / 60, "minute"),
        3_600..86_400 => (d / 3_600, "hour"),
        _ => (d / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

impl RecentGames {
    /// An empty list kept nowhere.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// The list in `path`; when the file does not exist yet, seeded once
    /// from `legacy` (the profile's old addresses, module rules) and saved.
    pub fn load_or_import(path: &Path, legacy: &[String]) -> Self {
        let mut r = Self {
            games: Vec::new(),
            path: Some(path.to_owned()),
        };
        match std::fs::read_to_string(path) {
            Ok(txt) => match ron::from_str::<Vec<RecentGame>>(&txt) {
                Ok(g) => {
                    r.games = g;
                    r.games.truncate(RECENT_GAMES);
                }
                Err(e) => {
                    eprintln!(
                        "recent games {} unreadable ({e}); starting empty",
                        path.display()
                    );
                    let _ = std::fs::rename(path, path.with_extension("ron.bad"));
                }
            },
            Err(_) if !legacy.is_empty() => {
                for a in legacy.iter().rev() {
                    r.remember(a, "", 0);
                }
                r.save();
            }
            Err(_) => {}
        }
        r
    }

    /// Writes the file (module rules); a failure is said, never fatal.
    pub fn save(&self) {
        if let Some(p) = &self.path {
            if let Err(e) = crate::profile::write_ron(&self.games, p) {
                eprintln!("recent games save failed: {e:#}");
            }
        }
    }

    /// Puts `addr` first, joined at `now`; a known entry keeps its details
    /// unless `name` gives a new one.
    pub fn remember(&mut self, addr: &str, name: &str, now: u64) {
        let addr = addr.trim();
        if addr.is_empty() {
            return;
        }
        let old = self
            .games
            .iter()
            .position(|g| g.addr.eq_ignore_ascii_case(addr))
            .map(|i| self.games.remove(i));
        let mut g = old.unwrap_or_default();
        g.addr = addr.to_owned();
        if !name.is_empty() {
            g.name = name.to_owned();
        }
        g.last_joined_unix = now;
        self.games.insert(0, g);
        self.games.truncate(RECENT_GAMES);
    }

    /// Fills in what the session learned (the host's name, the map and mode
    /// played; empty fields are left alone). True when anything changed.
    pub fn update(&mut self, addr: &str, name: &str, map: &str, mode: &str) -> bool {
        let Some(g) = self
            .games
            .iter_mut()
            .find(|g| g.addr.eq_ignore_ascii_case(addr.trim()))
        else {
            return false;
        };
        let mut changed = false;
        for (field, v) in [(&mut g.name, name), (&mut g.map, map), (&mut g.mode, mode)] {
            if !v.is_empty() && field != v {
                *field = v.to_owned();
                changed = true;
            }
        }
        changed
    }

    /// Forgets entry `i` (out of range: nothing).
    pub fn forget(&mut self, i: usize) -> bool {
        if i < self.games.len() {
            self.games.remove(i);
            true
        } else {
            false
        }
    }

    /// Join Game's rows (no answers yet).
    pub fn rows(&self, now: u64) -> Vec<ui::lobby::RecentRow> {
        self.games
            .iter()
            .map(|g| ui::lobby::RecentRow::new(&g.addr, &g.name, ago_text(now, g.last_joined_unix)))
            .collect()
    }
}

/// A host the browser heard, as Join Game lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    pub addr: SocketAddr,
    /// Its hosting session ([`super::HostInfo::session`]; 0 unknown).
    pub session: u64,
    pub row: ui::lobby::FoundRow,
}

/// Splits the hosts heard (module rules): the answer of each recent game
/// (by its resolved address, `None`: not resolved or not IPv4), and the
/// other hosts, one per session, the quickest answer of each.
pub fn merge(
    heard: Vec<Heard>,
    recent: &[Option<SocketAddr>],
) -> (Vec<ui::lobby::FoundRow>, Vec<Option<ui::lobby::FoundRow>>) {
    let mut claimed_sessions = Vec::new();
    let mut claimed_addrs = Vec::new();
    let live = recent
        .iter()
        .map(|a| {
            let h = heard.iter().find(|h| Some(h.addr) == *a)?;
            claimed_addrs.push(h.addr);
            if h.session != 0 {
                claimed_sessions.push(h.session);
            }
            Some(h.row.clone())
        })
        .collect();
    let mut rest: Vec<Heard> = heard
        .into_iter()
        .filter(|h| !claimed_addrs.contains(&h.addr) && !claimed_sessions.contains(&h.session))
        .collect();
    let mut out: Vec<Heard> = Vec::new();
    rest.sort_by_key(|h| h.row.ping_ms);
    for h in rest {
        if h.session == 0 || !out.iter().any(|o| o.session == h.session) {
            out.push(h);
        }
    }
    out.sort_by_key(|h| h.addr);
    (out.into_iter().map(|h| h.row).collect(), live)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(addr: &str, ping_ms: u32) -> ui::lobby::FoundRow {
        ui::lobby::FoundRow {
            addr: addr.into(),
            name: "Ann".into(),
            map: String::new(),
            mode: String::new(),
            players: "1/8".into(),
            ping_ms,
            refusal: None,
        }
    }

    fn heard(addr: &str, session: u64, ping_ms: u32) -> Heard {
        Heard {
            addr: addr.parse().unwrap(),
            session,
            row: row(addr, ping_ms),
        }
    }

    /// Newest first, once per address (case ignored), at most eight; a
    /// rejoin keeps the name learned before; forget and update.
    #[test]
    fn recent_games_remember_forget_and_update() {
        let mut r = RecentGames::in_memory();
        for i in 0..10 {
            r.remember(&format!("10.0.0.{i}"), "", 100 + i);
        }
        assert_eq!(r.games.len(), RECENT_GAMES);
        assert_eq!(r.games[0].addr, "10.0.0.9");
        r.remember("ann-pc", "Ann", 200);
        r.remember(" ANN-PC ", "", 300);
        assert_eq!(r.games[0].addr, "ANN-PC");
        assert_eq!(r.games[0].name, "Ann", "the name is kept");
        assert_eq!(r.games[0].last_joined_unix, 300);
        assert_eq!(
            r.games
                .iter()
                .filter(|g| g.addr.eq_ignore_ascii_case("ann-pc"))
                .count(),
            1
        );
        r.remember("", "x", 1);
        assert_eq!(r.games[0].addr, "ANN-PC");
        assert!(r.update("ann-pc", "Ann B", "Hoth: Echo Base", "Conquest"));
        assert!(!r.update("ann-pc", "", "Hoth: Echo Base", ""), "no change");
        assert!(!r.update("nobody", "x", "", ""));
        assert_eq!(r.games[0].name, "Ann B");
        assert_eq!(r.games[0].mode, "Conquest");
        assert!(r.forget(0));
        assert!(!r.forget(99));
        assert_eq!(r.games[0].addr, "10.0.0.9");
        assert_eq!(ago_text(1_000, 0), "");
        assert_eq!(ago_text(1_000, 990), "just now");
        assert_eq!(ago_text(10_000, 10_000 - 120), "2 minutes ago");
        assert_eq!(ago_text(10_000, 10_000 - 3_600), "1 hour ago");
        assert_eq!(ago_text(1_000_000, 1_000_000 - 2 * 86_400), "2 days ago");
    }

    /// The file round trips; a missing file is seeded once from the old
    /// profile list, then the file wins; an unreadable file is set aside.
    #[test]
    fn recent_games_file_round_trips_imports_once_and_survives_corruption() {
        let dir = std::env::temp_dir().join(format!("openbf_recent_games_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE_NAME);
        let legacy = vec!["studio-pc:40000".to_owned(), "10.0.0.2".to_owned()];
        let r = RecentGames::load_or_import(&path, &legacy);
        assert_eq!(
            r.games.iter().map(|g| g.addr.as_str()).collect::<Vec<_>>(),
            ["studio-pc:40000", "10.0.0.2"],
            "imported, order kept"
        );
        assert!(path.exists(), "the import is saved");
        let mut r = RecentGames::load_or_import(&path, &[]);
        r.remember("ann-pc", "Ann", 1_700_000_000);
        r.update("ann-pc", "", "Kashyyyk: Docks", "Conquest");
        r.forget(2);
        r.save();
        let back = RecentGames::load_or_import(&path, &["ignored".to_owned()]);
        assert_eq!(back, r, "round trip; the old list is not read again");
        assert_eq!(back.games[0].map, "Kashyyyk: Docks");
        std::fs::write(&path, "not ron").unwrap();
        let bad = RecentGames::load_or_import(&path, &legacy);
        assert!(
            bad.games.is_empty(),
            "an unreadable file is not re-imported"
        );
        assert!(path.with_extension("ron.bad").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A recent game's address answer goes on its row; the same session
    /// heard at another address (its LAN one) is not listed again; the
    /// other hosts are listed once per session, the quickest answer.
    #[test]
    fn recent_games_merge_answers_and_sessions() {
        let heard = vec![
            heard("100.64.0.9:37450", 7, 40),
            heard("192.168.1.9:37450", 7, 2),
            heard("192.168.1.20:37450", 9, 5),
            heard("10.0.0.20:37450", 9, 3),
            heard("192.168.1.30:37450", 0, 4),
            heard("192.168.1.31:37450", 0, 4),
        ];
        let recent = [
            Some("100.64.0.9:37450".parse().unwrap()),
            None,
            Some("100.64.0.99:37450".parse().unwrap()),
        ];
        let (found, live) = merge(heard, &recent);
        assert_eq!(live.len(), 3);
        assert_eq!(live[0].as_ref().unwrap().addr, "100.64.0.9:37450");
        assert!(live[1].is_none() && live[2].is_none());
        let addrs: Vec<&str> = found.iter().map(|f| f.addr.as_str()).collect();
        assert_eq!(
            addrs,
            [
                "10.0.0.20:37450",
                "192.168.1.30:37450",
                "192.168.1.31:37450"
            ],
            "session 7 is the recent game's; session 9 once, its quickest"
        );
    }
}
