//! Who may play: `Hello` checks, slot choice, kicks, bans, leaves and
//! slot reservations (rules in the `host` module header).

use super::{HostCore, LeaveReason, NetEvent, Phase, Player, CLOSE_LINGER};
use crate::msg::{unique_name, valid_name, Msg, RejectReason};
use crate::packet::PROTOCOL;
use crate::scheduler::SlotQueue;
use crate::RESERVE_FOR;

/// A decoded `Hello`.
pub(super) struct Hello {
    pub protocol: u16,
    pub build: String,
    pub name: String,
    pub token: u64,
    pub content: Vec<u8>,
    pub profile: Vec<u8>,
}

impl HostCore {
    /// Kicks a player; `ban` refuses its token and address for the session.
    pub fn kick(&mut self, slot: u8, ban: bool) {
        self.kick_with(slot, ban, String::new(), LeaveReason::Kicked);
    }

    /// [`HostCore::kick`] with the reject `detail` the guest shows and the
    /// leave `reason` the host game hears.
    pub fn kick_with(&mut self, slot: u8, ban: bool, detail: String, reason: LeaveReason) {
        if slot == 0 {
            return;
        }
        let Some(p) = self.players.remove(&slot) else {
            return;
        };
        if ban {
            self.banned_tokens.insert(p.token);
        }
        if let Some(id) = p.conn {
            if let Some(peer) = self.peers.get_mut(&id) {
                if ban {
                    self.banned_ips.insert(peer.conn.addr.ip());
                }
                let reject = if ban {
                    RejectReason::Banned
                } else {
                    RejectReason::Kicked
                };
                peer.conn.send_reliable(
                    &Msg::Reject {
                        reason: reject,
                        detail,
                    }
                    .encode(),
                );
                peer.phase = Phase::Closing(self.now + CLOSE_LINGER);
            }
        }
        self.round.remove_human(slot, p.active);
        self.events.push_back(NetEvent::PlayerLeft { slot, reason });
    }

    /// Takes a connected guest out of the current round without dropping
    /// its connection (LAN §4.3: a copy that plays differently waits in the
    /// lobby): a bot takes its place, no more bundles go to it, and the next
    /// `load_round` seats it again. False when `slot` is no connected guest.
    pub fn bench(&mut self, slot: u8) -> bool {
        let Some(id) = self
            .players
            .get(&slot)
            .filter(|_| slot != 0)
            .and_then(|p| p.conn)
        else {
            return false;
        };
        let active = self.players.get(&slot).is_some_and(|p| p.active);
        self.round.remove_human(slot, active);
        self.round.late.retain(|s| *s != slot);
        if let Some(p) = self.players.get_mut(&slot) {
            p.catching_up = false;
            p.loaded = true;
        }
        if let Some(peer) = self.peers.get_mut(&id) {
            peer.stream = 0;
            peer.stream_round = 0;
            peer.sent_through = 0;
        }
        self.check_all_loaded();
        true
    }

    pub(super) fn admit(&mut self, id: u32, hello: Hello) {
        let Hello {
            protocol,
            build,
            name,
            token,
            content,
            profile,
        } = hello;
        let addr = self.peers[&id].conn.addr;
        let refuse = |reason, detail: String| (reason, detail);
        let verdict = if protocol != PROTOCOL {
            Err(refuse(
                RejectReason::Version,
                format!("protocol {protocol}, host {PROTOCOL}"),
            ))
        } else if build != self.config.build {
            Err(refuse(
                RejectReason::Version,
                format!("build {build}, host {}", self.config.build),
            ))
        } else if self.banned_tokens.contains(&token) || self.banned_ips.contains(&addr.ip()) {
            Err(refuse(RejectReason::Banned, String::new()))
        } else if !valid_name(&name) {
            Err(refuse(RejectReason::BadName, String::new()))
        } else if self.round.is_loading() {
            Err(refuse(RejectReason::Busy, String::new()))
        } else {
            self.pick_slot(token)
                .ok_or_else(|| refuse(RejectReason::Full, String::new()))
        };
        let (slot, reconnect) = match verdict {
            Ok(x) => x,
            Err((reason, detail)) => {
                let peer = self.peers.get_mut(&id).expect("peer");
                peer.conn
                    .send_reliable(&Msg::Reject { reason, detail }.encode());
                peer.phase = Phase::Closing(self.now + CLOSE_LINGER);
                self.events.push_back(NetEvent::Refused { addr, reason });
                return;
            }
        };
        // The same token still connected: the old link is replaced.
        if let Some(old) = self.players.get(&slot).and_then(|p| p.conn) {
            if old != id {
                self.peers.remove(&old);
                self.events.push_back(NetEvent::PlayerLeft {
                    slot,
                    reason: LeaveReason::Replaced,
                });
            }
        }
        // LAN §5b: names are unique in join order ("the maintainer", "playtest_1", ...);
        // a reconnect keeps the name its slot already has.
        let name = match self.players.get(&slot) {
            Some(p) if reconnect && !p.name.is_empty() => p.name.clone(),
            _ => unique_name(&name, |n| {
                self.players
                    .iter()
                    .any(|(s, p)| *s != slot && p.name.eq_ignore_ascii_case(n))
            }),
        };
        let target = self.config.queue_target;
        let p = self.players.entry(slot).or_insert_with(|| Player {
            name: String::new(),
            token,
            profile: Vec::new(),
            conn: None,
            queue: SlotQueue::new(target),
            reserved_until: None,
            active: false,
            loaded: false,
            seen_tick: 0,
            catching_up: false,
        });
        p.name.clone_from(&name);
        p.token = token;
        p.profile = profile.clone();
        p.conn = Some(id);
        p.reserved_until = None;
        p.queue.reset_stream();
        let peer = self.peers.get_mut(&id).expect("peer");
        peer.phase = Phase::Joined(slot);
        peer.conn.send_reliable(
            &Msg::Welcome {
                slot,
                max_players: self.config.max_players,
            }
            .encode(),
        );
        if self.lobby_version != 0 {
            peer.conn.send_reliable(
                &Msg::Lobby {
                    version: self.lobby_version,
                    state: self.lobby.clone(),
                }
                .encode(),
            );
        }
        self.events.push_back(NetEvent::PlayerJoined {
            slot,
            name,
            content,
            profile,
            reconnect,
        });
        if self.round.is_running() {
            self.start_stream(id, true);
        }
    }

    /// The slot for a token: its old one (connected or reserved), else the
    /// lowest free one.
    fn pick_slot(&self, token: u64) -> Option<(u8, bool)> {
        if let Some((s, _)) = self
            .players
            .iter()
            .find(|(s, p)| **s != 0 && p.token == token)
        {
            return Some((*s, true));
        }
        (1..self.config.max_players)
            .find(|s| !self.players.contains_key(s))
            .map(|s| (s, false))
    }

    pub(super) fn drop_peer(&mut self, id: u32, reason: LeaveReason) {
        let Some(peer) = self.peers.remove(&id) else {
            return;
        };
        let Phase::Joined(slot) = peer.phase else {
            return;
        };
        let Some(p) = self.players.get_mut(&slot) else {
            return;
        };
        if p.conn != Some(id) {
            return;
        }
        p.conn = None;
        p.reserved_until = Some(self.now + RESERVE_FOR);
        let active = p.active;
        self.round.remove_human(slot, active);
        self.events.push_back(NetEvent::PlayerLeft { slot, reason });
        self.check_all_loaded();
    }
}
