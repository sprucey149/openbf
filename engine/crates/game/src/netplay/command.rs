//! In-round game commands (host → every peer, inside a bundle's
//! `SessionCommand::Game`) and guest requests (guest → host, a
//! `Msg::Request`), LAN §4.4 and §5.
//!
//! Rules:
//! - A [`GameCommand`] is applied by every peer before the same tick's step
//!   (`super::apply`). Bytes that do not decode are ignored everywhere alike.
//! - Cheats are bits in [`core::sim::Cheats::entries`] order ([`cheat_bits`]).
//! - The host's own commands are not limited by the cheat policy: the host
//!   can switch any player's cheats or everyone's. A guest's
//!   [`GameRequest`] is checked by [`validate_request`]: only for the
//!   requesting slot, and turning a cheat on only when the policy allows it
//!   (turning one off is always allowed).
//! - A new policy clears every disallowed cheat on every slot on the tick it
//!   applies.
//! - A guest's round reports ([`GameRequest::Built`], [`GameRequest::LeftRound`])
//!   are no commands: [`super::Peer`] answers them on the host, and
//!   [`validate_request`] refuses them ([`Refusal::NotACommand`]).
//! - The pause menu's sim settings (team size, reinforcements, career awards)
//!   are host-only commands in multiplayer.
//! - The award policy (LAN §5b) works like the cheat policy: the host's
//!   [`GameCommand::SetAwardPolicy`] switches every slot's earned awards off,
//!   or back to each player's own preference.

use core::sim::Cheats;
use serde::{Deserialize, Serialize};

/// `slot` value meaning "every human slot".
pub const ALL_SLOTS: u8 = 255;
/// How many cheats [`Cheats::entries`] lists.
pub const CHEAT_COUNT: u8 = 10;
/// Every cheat bit.
pub const ALL_CHEATS: u32 = (1 << CHEAT_COUNT) - 1;

/// A cheat set as bits in [`Cheats::entries`] order.
pub fn cheat_bits(c: &Cheats) -> u32 {
    c.entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.2)
        .fold(0, |b, (i, _)| b | 1 << i)
}

/// The cheat set of `bits` (bits past [`CHEAT_COUNT`] ignored).
pub fn cheats_from_bits(bits: u32) -> Cheats {
    let mut c = Cheats::default();
    for (i, e) in Cheats::default().entries().iter().enumerate() {
        if bits & (1 << i) != 0 {
            c.toggle(e.0);
        }
    }
    c
}

/// A cheat's bit index by its registry id ("invincible", "ammo", ...).
pub fn cheat_index(id: &str) -> Option<u8> {
    Cheats::default()
        .entries()
        .iter()
        .position(|e| e.0 == id)
        .map(|i| i as u8)
}

/// A host command every peer applies on the same tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameCommand {
    /// Replace `slot`'s cheats ([`ALL_SLOTS`]: every human's).
    SetSlotCheats { slot: u8, bits: u32 },
    /// Switch one cheat of `slot` ([`ALL_SLOTS`]: every human's).
    SetSlotCheat { slot: u8, id: u8, on: bool },
    /// The cheats guests may ask for; disallowed ones are cleared everywhere.
    SetCheatPolicy { allowed: u32 },
    /// The pause menu's Team size (`WalkSim::set_team_size_percent`).
    SetTeamSize { pct: u32 },
    /// The pause menu's Reinforcements (`WalkSim::reinforcement_pct`).
    SetReinforcements { pct: u32 },
    /// A slot's career award bonuses ([`ALL_SLOTS`]: every human's): the
    /// player's own preference, applied while the award policy allows.
    SetCareerAwards { slot: u8, on: bool },
    /// The host's award policy: off, no slot's earned awards apply; on,
    /// each slot's own preference does.
    SetAwardPolicy { on: bool },
}

impl GameCommand {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("game command encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// A guest's ask of the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameRequest {
    /// The pause menu's Cheats row or a Morse word, for the asker's slot.
    CheatToggle { id: u8, on: bool },
    /// A mid-round joiner built round `round_id` (LAN §4.3, §4.5): its
    /// static digest and tick-0 `Sim::hash`, checked as a round-start
    /// guest's `Loaded` is (a joiner sends no `Loaded`: the host would take
    /// it as live and its catch-up as falling behind).
    Built {
        round_id: u32,
        static_digest: u64,
        hash0: u64,
    },
    /// The guest left round `round_id` for the lobby on its own: the host
    /// benches it so neither the load barrier nor a seat waits for it.
    LeftRound { round_id: u32, why: LeftWhy },
}

/// Why a guest left a round on its own ([`GameRequest::LeftRound`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeftWhy {
    /// The round could not be built here (a missing map or file).
    LoadFailed,
    /// A second desync within [`super::peer::DESYNC_WINDOW_S`] (LAN §4.7).
    Desynced,
}

impl GameRequest {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("game request encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// Why the host refused a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Refusal {
    /// The bytes are no request.
    Malformed,
    /// No such cheat.
    UnknownCheat,
    /// The cheat policy does not allow it.
    NotAllowed,
    /// A round report, not a command ([`GameRequest::Built`],
    /// [`GameRequest::LeftRound`]).
    NotACommand,
}

/// The command a guest's request becomes, or why not (module rules).
pub fn validate_request(slot: u8, req: &GameRequest, policy: u32) -> Result<GameCommand, Refusal> {
    match *req {
        GameRequest::CheatToggle { id, on } => {
            if id >= CHEAT_COUNT {
                return Err(Refusal::UnknownCheat);
            }
            if on && policy & (1 << id) == 0 {
                return Err(Refusal::NotAllowed);
            }
            Ok(GameCommand::SetSlotCheat { slot, id, on })
        }
        GameRequest::Built { .. } | GameRequest::LeftRound { .. } => Err(Refusal::NotACommand),
    }
}

/// [`validate_request`] from the request's bytes.
pub fn validate_request_bytes(slot: u8, bytes: &[u8], policy: u32) -> Result<GameCommand, Refusal> {
    let req = GameRequest::decode(bytes).ok_or(Refusal::Malformed)?;
    validate_request(slot, &req, policy)
}
