//! `[ours]` The LAN session's diagnostic log lines (issue #189): what a
//! player's log file needs to explain a guest that stays on the loading
//! screen or drops, without the other machine's log.
//!
//! Rules:
//! - Every line starts with the session time, `t=<s>s`, counted from the
//!   session's start, so two machines' logs line up by their own events
//!   (each log's first line holds the wall clock).
//! - The host logs each guest's join with its OS and CPU, each silence the
//!   network core reports (`Trouble`, 1.5 s with nothing heard) and its end,
//!   each guest's `Loaded`, the round start with the guests that were not
//!   loaded, and each leave with how long that guest had been silent and
//!   what the round was doing.
//! - A guest logs its own loading (began, built and `Loaded` sent, the
//!   start heard) and each silence from the host.
//! - Log only: nothing here reaches the HUD, the sim or the wire.

use net::host::LeaveReason;
use std::collections::{BTreeMap, BTreeSet};

/// The session's diagnostic state (module rules). Times are seconds since
/// the session began; the caller passes the clock.
#[derive(Debug, Default)]
pub struct Diag {
    /// Silent guests, by slot: since when.
    silent: BTreeMap<u8, f64>,
    /// The host has been silent since then (a guest's view).
    host_silent: Option<f64>,
    /// The round loading: its id, since when, and (the host) the guests it
    /// waits for and those that reported `Loaded`.
    round: Option<Round>,
}

#[derive(Debug)]
struct Round {
    id: u32,
    since: f64,
    joining: bool,
    waiting: BTreeSet<u8>,
    loaded: BTreeSet<u8>,
    started: bool,
}

fn stamp(t: f64) -> String {
    format!("t={t:.1}s")
}

impl Diag {
    /// A guest joined: `target` is its `os-arch` (empty from a build that
    /// does not send it).
    pub fn joined(
        &mut self,
        t: f64,
        slot: u8,
        name: &str,
        target: &str,
        reconnect: bool,
    ) -> String {
        self.silent.remove(&slot);
        let target = if target.is_empty() {
            "unknown OS"
        } else {
            target
        };
        format!(
            "{} {name} {} (slot {slot}, {target})",
            stamp(t),
            if reconnect { "reconnected" } else { "joined" }
        )
    }

    /// A round began loading here: `guests` are the other humans the host
    /// waits for (empty on a guest).
    pub fn load_began(
        &mut self,
        t: f64,
        id: u32,
        map: &str,
        joining: bool,
        guests: impl IntoIterator<Item = u8>,
    ) -> String {
        let waiting: BTreeSet<u8> = guests.into_iter().collect();
        let line = format!(
            "{} loading round {id} ({map}{}){}",
            stamp(t),
            if joining { ", joining mid-round" } else { "" },
            if waiting.is_empty() {
                String::new()
            } else {
                format!(", waiting for {} guest(s)", waiting.len())
            }
        );
        self.round = Some(Round {
            id,
            since: t,
            joining,
            waiting,
            loaded: BTreeSet::new(),
            started: false,
        });
        line
    }

    /// This machine built the round (a guest's `Loaded` or `Built` went out).
    pub fn built(&mut self, t: f64) -> Option<String> {
        let r = self.round.as_ref()?;
        Some(format!(
            "{} built round {} in {:.1} s{}",
            stamp(t),
            r.id,
            t - r.since,
            if r.joining { ", catching up" } else { "" }
        ))
    }

    /// The host heard guest `slot`'s `Loaded` for the round loading.
    pub fn guest_loaded(&mut self, t: f64, slot: u8, name: &str) -> Option<String> {
        let r = self.round.as_mut()?;
        r.loaded.insert(slot);
        Some(format!(
            "{} {name} loaded round {} in {:.1} s",
            stamp(t),
            r.id,
            t - r.since
        ))
    }

    /// The round started here. On the host, `name_of` names the guests that
    /// had not reported `Loaded`.
    pub fn started(&mut self, t: f64, name_of: impl Fn(u8) -> String) -> Option<String> {
        let r = self.round.as_mut()?;
        if r.started {
            return None;
        }
        r.started = true;
        let late: Vec<String> = r
            .waiting
            .iter()
            .filter(|s| !r.loaded.contains(s))
            .map(|&s| match self.silent.get(&s) {
                Some(&since) => format!("{} (silent {:.1} s)", name_of(s), t - since),
                None => name_of(s),
            })
            .collect();
        Some(format!(
            "{} round {} starts {:.1} s after loading began{}",
            stamp(t),
            r.id,
            t - r.since,
            if late.is_empty() {
                String::new()
            } else {
                format!("; not loaded: {}", late.join(", "))
            }
        ))
    }

    /// What the round is doing for guest `slot`, for a silence's line.
    fn round_state(&self, slot: u8) -> String {
        match &self.round {
            None => "in the lobby".to_owned(),
            Some(r) if !r.started && r.waiting.contains(&slot) => format!(
                "loading round {}, {}",
                r.id,
                if r.loaded.contains(&slot) {
                    "loaded"
                } else {
                    "not loaded yet"
                }
            ),
            Some(r) if !r.started => format!("loading round {}", r.id),
            Some(r) => format!("round {} in play", r.id),
        }
    }

    /// The network core's `Trouble` for guest `slot`: nothing heard for
    /// 1.5 s (`on`), or heard again.
    pub fn guest_trouble(&mut self, t: f64, slot: u8, name: &str, on: bool) -> Option<String> {
        if on {
            self.silent.insert(slot, t - silence_before_trouble());
            return Some(format!(
                "{} {name} silent for {:.1} s ({})",
                stamp(t),
                silence_before_trouble(),
                self.round_state(slot)
            ));
        }
        let since = self.silent.remove(&slot)?;
        Some(format!(
            "{} {name} heard again after {:.1} s of silence",
            stamp(t),
            t - since
        ))
    }

    /// Guest `slot` left.
    pub fn left(&mut self, t: f64, slot: u8, name: &str, reason: LeaveReason) -> String {
        let state = self.round_state(slot);
        let silent = self.silent.remove(&slot);
        if let Some(r) = self.round.as_mut() {
            r.waiting.remove(&slot);
        }
        format!(
            "{} {name} left: {reason:?}{} ({state})",
            stamp(t),
            silent.map_or_else(String::new, |since| format!(
                ", nothing heard for {:.1} s",
                t - since
            ))
        )
    }

    /// The client core's `Trouble`: nothing heard from the host for 1.5 s
    /// (`on`), or heard again.
    pub fn host_trouble(&mut self, t: f64, on: bool) -> Option<String> {
        if on {
            self.host_silent = Some(t - silence_before_trouble());
            let state = match &self.round {
                None => "in the lobby".to_owned(),
                Some(r) if !r.started => format!("loading round {}", r.id),
                Some(r) => format!("round {} in play", r.id),
            };
            return Some(format!(
                "{} nothing heard from the host for {:.1} s ({state})",
                stamp(t),
                silence_before_trouble()
            ));
        }
        let since = self.host_silent.take()?;
        Some(format!(
            "{} host heard again after {:.1} s of silence",
            stamp(t),
            t - since
        ))
    }
}

/// The silence the network core waits before `Trouble` (seconds).
fn silence_before_trouble() -> f64 {
    net::TROUBLE_AFTER.as_millis_f64() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(s: u8) -> String {
        format!("P{s}")
    }

    /// The fourth playtest's Mac guest: never loaded, silent, timed out.
    /// The host's lines name the silence and the loading it interrupted.
    #[test]
    fn a_silent_loading_guest_is_named_at_the_start_and_the_leave() {
        let mut d = Diag::default();
        assert_eq!(
            d.joined(1.0, 2, "TAB", "macos-aarch64", false),
            "t=1.0s TAB joined (slot 2, macos-aarch64)"
        );
        d.load_began(10.0, 1, "bf2_mus1 con", false, [1, 2]);
        assert_eq!(
            d.guest_loaded(13.0, 1, "P1").as_deref(),
            Some("t=13.0s P1 loaded round 1 in 3.0 s")
        );
        assert_eq!(
            d.guest_trouble(12.0, 2, "TAB", true).as_deref(),
            Some("t=12.0s TAB silent for 1.5 s (loading round 1, not loaded yet)")
        );
        assert_eq!(
            d.started(67.0, names).as_deref(),
            Some(
                "t=67.0s round 1 starts 57.0 s after loading began; not loaded: P2 (silent 56.5 s)"
            )
        );
        assert_eq!(d.started(68.0, names), None, "one start line per round");
        assert_eq!(
            d.left(70.5, 2, "TAB", LeaveReason::Timeout),
            "t=70.5s TAB left: Timeout, nothing heard for 60.0 s (round 1 in play)"
        );
    }

    #[test]
    fn a_silence_that_ends_says_how_long_it_lasted() {
        let mut d = Diag::default();
        assert!(
            d.guest_trouble(5.0, 1, "A", false).is_none(),
            "no end without a start"
        );
        d.guest_trouble(5.0, 1, "A", true);
        assert_eq!(
            d.guest_trouble(8.5, 1, "A", false).as_deref(),
            Some("t=8.5s A heard again after 5.0 s of silence")
        );
        assert_eq!(
            d.host_trouble(2.0, true).as_deref(),
            Some("t=2.0s nothing heard from the host for 1.5 s (in the lobby)")
        );
        assert_eq!(
            d.host_trouble(3.5, false).as_deref(),
            Some("t=3.5s host heard again after 3.0 s of silence")
        );
    }

    #[test]
    fn a_guest_logs_its_own_build() {
        let mut d = Diag::default();
        assert!(d.built(1.0).is_none(), "nothing loading");
        d.load_began(4.0, 3, "bf2_dea1 con", true, []);
        assert_eq!(
            d.built(9.3).as_deref(),
            Some("t=9.3s built round 3 in 5.3 s, catching up")
        );
        assert_eq!(
            d.started(10.0, names).as_deref(),
            Some("t=10.0s round 3 starts 6.0 s after loading began")
        );
    }
}
