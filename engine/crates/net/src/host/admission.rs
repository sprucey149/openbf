//! Who may play: `Hello` checks, slot choice, kicks, bans, leaves and
//! slot reservations (rules in the `host` module header).

use super::{HostCore, LeaveReason, NetEvent, Phase, Player, CLOSE_LINGER};
use crate::msg::{
    local_token, unique_name, valid_name, LocalHello, Msg, RejectReason, MAX_LOCALS, MAX_NAME_CHARS,
};
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
    pub locals: Vec<LocalHello>,
}

impl HostCore {
    /// Kicks a player; `ban` refuses its token and address for the session.
    pub fn kick(&mut self, slot: u8, ban: bool) {
        self.kick_with(slot, ban, String::new(), LeaveReason::Kicked);
    }

    /// [`HostCore::kick`] with the reject `detail` the guest shows and the
    /// leave `reason` the host game hears.
    pub fn kick_with(&mut self, slot: u8, ban: bool, detail: String, reason: LeaveReason) {
        if self.players.get(&slot).is_none_or(|p| p.host_local) {
            return;
        }
        // A connected slot's whole machine goes with it, and a reserved
        // slot's reserved siblings (module rules).
        let conn = self.players.get(&slot).and_then(|p| p.conn);
        let slots = if conn.is_some() {
            self.machine_slots(slot)
        } else {
            let machine = self.players[&slot].machine;
            self.players
                .iter()
                .filter(|(s, p)| {
                    **s == slot || (!p.host_local && p.conn.is_none() && p.machine == machine)
                })
                .map(|(s, _)| *s)
                .collect()
        };
        if let Some(id) = conn {
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
        for s in slots {
            let Some(p) = self.players.remove(&s) else {
                continue;
            };
            if ban {
                self.banned_tokens.insert(p.token);
            }
            self.round.remove_human(s, p.active);
            self.events
                .push_back(NetEvent::PlayerLeft { slot: s, reason });
        }
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
        // The machine waits as one (module rules).
        for s in self.conn_slots(id) {
            let active = self.players.get(&s).is_some_and(|p| p.active);
            self.round.remove_human(s, active);
            self.round.late.retain(|x| *x != s);
            self.round.late_locals.retain(|x| *x != s);
            if let Some(p) = self.players.get_mut(&s) {
                p.catching_up = false;
                p.loaded = true;
            }
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
            locals,
        } = hello;
        let addr = self.peers[&id].conn.addr;
        // Local 0 (seat 0) first, then the machine's other players by seat
        // (module rules).
        let mut wanted = vec![(0, name, profile)];
        wanted.extend(locals.into_iter().map(|l| (l.seat, l.name, l.profile)));
        wanted.truncate(MAX_LOCALS);
        let seats: Vec<u8> = wanted.iter().map(|(seat, _, _)| *seat).collect();
        let refuse = |reason, detail: String| (reason, detail);
        let banned = seats
            .iter()
            .any(|seat| self.banned_tokens.contains(&local_token(token, *seat)));
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
        } else if banned || self.banned_ips.contains(&addr.ip()) {
            Err(refuse(RejectReason::Banned, String::new()))
        } else if !wanted.iter().all(|(_, n, _)| valid_name(n)) {
            Err(refuse(RejectReason::BadName, String::new()))
        } else if self.round.is_loading() {
            Err(refuse(RejectReason::Busy, String::new()))
        } else {
            self.pick_slots(token, &seats).ok_or_else(|| {
                refuse(
                    RejectReason::Full,
                    full_detail(self.room_for(token, &seats), seats.len()),
                )
            })
        };
        let picked = match verdict {
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
        let slots: Vec<u8> = picked.iter().map(|(s, _)| *s).collect();
        // The same token still connected: the old link is replaced, and any
        // slot it held that this `Hello` does not take back is released.
        let mut old_ids: Vec<u32> = slots
            .iter()
            .filter_map(|s| self.players.get(s).and_then(|p| p.conn))
            .filter(|old| *old != id)
            .collect();
        old_ids.sort_unstable();
        old_ids.dedup();
        for old in old_ids {
            let held = self.conn_slots(old);
            self.peers.remove(&old);
            for s in held {
                // A seat this `Hello` takes back reconnects; the others left.
                let reclaimed = slots.contains(&s);
                self.events.push_back(NetEvent::PlayerLeft {
                    slot: s,
                    reason: if reclaimed {
                        LeaveReason::Replaced
                    } else {
                        LeaveReason::Left
                    },
                });
                if reclaimed {
                    continue;
                }
                if let Some(p) = self.players.get_mut(&s).filter(|p| p.conn == Some(old)) {
                    p.conn = None;
                    p.reserved_until = Some(self.now + RESERVE_FOR);
                    let active = p.active;
                    self.round.remove_human(s, active);
                }
            }
        }
        let target = self.config.queue_target;
        let mut joined = Vec::with_capacity(slots.len());
        for ((slot, reconnect), (seat, name, profile)) in picked.into_iter().zip(wanted) {
            // LAN §5b: names are unique in join order ("Han", "Han_1",
            // ...); a reconnect keeps the name its slot already has while
            // it is still the name this player asks for.
            let name = match self.players.get(&slot) {
                Some(p) if reconnect && same_name(&p.name, &name) => p.name.clone(),
                _ => unique_name(&name, |n| {
                    self.players
                        .iter()
                        .any(|(s, p)| *s != slot && p.name.eq_ignore_ascii_case(n))
                }),
            };
            let p = self.players.entry(slot).or_insert_with(|| Player {
                name: String::new(),
                token: 0,
                profile: Vec::new(),
                conn: None,
                machine: 0,
                host_local: false,
                queue: SlotQueue::new(target),
                reserved_until: None,
                active: false,
                loaded: false,
                seen_tick: 0,
                catching_up: false,
                away: false,
            });
            p.name.clone_from(&name);
            p.token = local_token(token, seat);
            p.machine = token;
            p.profile = profile.clone();
            p.conn = Some(id);
            p.reserved_until = None;
            // A new link tells its own lost pads again (`LocalAway`).
            p.away = false;
            p.queue.reset_stream();
            joined.push((slot, name, profile, reconnect));
        }
        let peer = self.peers.get_mut(&id).expect("peer");
        peer.phase = Phase::Joined(slots[0]);
        peer.extra = slots[1..].to_vec();
        peer.conn.send_reliable(
            &Msg::Welcome {
                slot: slots[0],
                max_players: self.config.max_players,
                extra: slots[1..].to_vec(),
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
        for (slot, name, profile, reconnect) in joined {
            self.events.push_back(NetEvent::PlayerJoined {
                slot,
                machine: slots[0],
                name,
                content: content.clone(),
                profile,
                reconnect,
            });
        }
        if self.round.is_running() {
            self.start_stream(id, true);
        }
    }

    /// The slots for a machine's local players in `seats`, all or none
    /// (module rules): each seat's old slot for its derived token
    /// (connected or reserved), the rest the lowest free ones. Each comes
    /// with whether it was reclaimed.
    fn pick_slots(&self, token: u64, seats: &[u8]) -> Option<Vec<(u8, bool)>> {
        let mut picked: Vec<Option<(u8, bool)>> = seats
            .iter()
            .map(|seat| {
                let t = local_token(token, *seat);
                self.players
                    .iter()
                    .find(|(_, p)| !p.host_local && p.token == t)
                    .map(|(s, _)| (*s, true))
            })
            .collect();
        let mut free = (1..self.config.max_players).filter(|s| !self.players.contains_key(s));
        for x in picked.iter_mut().filter(|x| x.is_none()) {
            *x = Some((free.next()?, false));
        }
        picked.into_iter().collect()
    }

    /// How many of a machine's `seats` [`Self::pick_slots`] could place:
    /// those reclaiming their own old slot, plus the free ones.
    fn room_for(&self, token: u64, seats: &[u8]) -> usize {
        let reclaimed = seats
            .iter()
            .filter(|seat| {
                let t = local_token(token, **seat);
                self.players.values().any(|p| !p.host_local && p.token == t)
            })
            .count();
        let free = (1..self.config.max_players)
            .filter(|s| !self.players.contains_key(s))
            .count();
        reclaimed + free
    }

    pub(super) fn drop_peer(&mut self, id: u32, reason: LeaveReason) {
        let slots = self.conn_slots(id);
        if self.peers.remove(&id).is_none() {
            return;
        }
        for slot in slots {
            let Some(p) = self.players.get_mut(&slot) else {
                continue;
            };
            if p.conn != Some(id) {
                continue;
            }
            p.conn = None;
            p.reserved_until = Some(self.now + RESERVE_FOR);
            let active = p.active;
            self.round.remove_human(slot, active);
            self.events.push_back(NetEvent::PlayerLeft { slot, reason });
        }
        self.check_all_loaded();
    }
}

/// Whether a reclaimed slot's `old` name is still the name `asked` for: the
/// same name, or `asked` as [`unique_name`] makes it unique (`asked_1`, ...;
/// LAN §5b).
fn same_name(old: &str, asked: &str) -> bool {
    let asked = asked.trim();
    if old.is_empty() {
        return false;
    }
    if old.eq_ignore_ascii_case(asked) {
        return true;
    }
    let Some((_, n)) = old.rsplit_once('_') else {
        return false;
    };
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // The candidate `unique_name` would build for this suffix.
    let suffix = format!("_{n}");
    let room = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
    let stem: String = asked.chars().take(room).collect();
    old.eq_ignore_ascii_case(&format!("{}{suffix}", stem.trim_end()))
}

/// A `Full` refusal's detail: "room R of W", the slots there are for a
/// machine bringing W players (all or none), so the guest can say how
/// many fewer could join ([`parse_full_detail`]).
pub fn full_detail(room: usize, wanted: usize) -> String {
    format!("room {} of {wanted}", room.min(wanted))
}

/// The `(room, wanted)` of a [`full_detail`]; `None` for any other text
/// (an empty detail).
pub fn parse_full_detail(detail: &str) -> Option<(usize, usize)> {
    let (room, wanted) = detail.strip_prefix("room ")?.split_once(" of ")?;
    Some((room.trim().parse().ok()?, wanted.trim().parse().ok()?))
}
