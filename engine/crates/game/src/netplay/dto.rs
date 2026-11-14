//! The round's setup and each player's profile as the host sends them
//! (LAN §4.1, §4.3): everything a peer would otherwise read from its own
//! profile, shell or environment that changes the sim.
//!
//! Rules:
//! - Every peer, the host included, builds the round from the DECODED bytes
//!   of [`RoundSetup`], never from the struct it encoded, so a float that a
//!   text codec rounds is rounded the same way everywhere.
//! - The codec is JSON (`serde_json`): small, versioned by its field names,
//!   and decoded the same on every OS. A [`PlayerProfile`] travels inside an
//!   `AddHuman` command and must fit [`net::bundle::MAX_COMMAND_BYTES`].
//! - The gameplay environment switches (`OPENBF_DOORS_SOLID`,
//!   `OPENBF_PLNP_MISSION`, `OPENBF_FLYER_BOTS`) are read at round build, deep
//!   in asset loading. The host sends its values ([`EnvSwitches`]); a peer
//!   whose own differ refuses to build ([`EnvSwitches::check_local`]) rather
//!   than desync. `OPENBF_AUTOPILOT` only drives the local pad and is ignored.

use anyhow::{bail, Context, Result};
use core::sim::{AssistProfile, MedalCounts};
use serde::{Deserialize, Serialize};
use ui::shell::SessionOptions;

/// The shell's Session Options that change a round (`heroes::rules`, the
/// population, tickets, timers and score limits). The rest of
/// [`SessionOptions`] is the local menu's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundOptions {
    pub heroes: bool,
    pub ai_heroes: bool,
    pub hero_unlock: i32,
    pub hero_unlock_val: i32,
    pub hero_assign: i32,
    pub hero_respawn: i32,
    /// The addon heroes in Hero Assault: only when every player has them
    /// ([`super::ContentManifest::intersect`]).
    pub dlc_heroes: bool,
    pub con_numbots: i32,
    pub con_mult: i32,
    pub con_timer: i32,
    pub ctf_numbots: i32,
    pub ctf_score: i32,
    pub ctf_timer: i32,
    pub hun_timer: i32,
    pub hun_score: i32,
    pub ass_numbots: i32,
    pub ass_score: i32,
}

impl RoundOptions {
    pub fn from_session(s: &SessionOptions) -> Self {
        Self {
            heroes: s.heroes,
            ai_heroes: s.ai_heroes,
            hero_unlock: s.hero_unlock,
            hero_unlock_val: s.hero_unlock_val,
            hero_assign: s.hero_assign,
            hero_respawn: s.hero_respawn,
            dlc_heroes: s.dlc_heroes,
            con_numbots: s.con_numbots,
            con_mult: s.con_mult,
            con_timer: s.con_timer,
            ctf_numbots: s.ctf_numbots,
            ctf_score: s.ctf_score,
            ctf_timer: s.ctf_timer,
            hun_timer: s.hun_timer,
            hun_score: s.hun_score,
            ass_numbots: s.ass_numbots,
            ass_score: s.ass_score,
        }
    }

    /// `base` (a peer's own menu state) with every round field the host's.
    pub fn to_session(&self, base: SessionOptions) -> SessionOptions {
        SessionOptions {
            heroes: self.heroes,
            ai_heroes: self.ai_heroes,
            hero_unlock: self.hero_unlock,
            hero_unlock_val: self.hero_unlock_val,
            hero_assign: self.hero_assign,
            hero_respawn: self.hero_respawn,
            dlc_heroes: self.dlc_heroes,
            con_numbots: self.con_numbots,
            con_mult: self.con_mult,
            con_timer: self.con_timer,
            ctf_numbots: self.ctf_numbots,
            ctf_score: self.ctf_score,
            ctf_timer: self.ctf_timer,
            hun_timer: self.hun_timer,
            hun_score: self.hun_score,
            ass_numbots: self.ass_numbots,
            ass_score: self.ass_score,
            ..base
        }
    }
}

impl Default for RoundOptions {
    fn default() -> Self {
        Self::from_session(&SessionOptions::default())
    }
}

/// The environment switches the round build reads (module header).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvSwitches {
    /// `OPENBF_DOORS_SOLID=1`.
    pub doors_solid: bool,
    /// Not `OPENBF_PLNP_MISSION=0`.
    pub plnp_mission: bool,
    /// Not `OPENBF_FLYER_BOTS=0`.
    pub flyer_bots: bool,
}

impl Default for EnvSwitches {
    fn default() -> Self {
        Self {
            doors_solid: false,
            plnp_mission: true,
            flyer_bots: true,
        }
    }
}

impl EnvSwitches {
    /// This process's switches.
    pub fn local() -> Self {
        Self {
            doors_solid: std::env::var("OPENBF_DOORS_SOLID").as_deref() == Ok("1"),
            plnp_mission: std::env::var("OPENBF_PLNP_MISSION").as_deref() != Ok("0"),
            flyer_bots: std::env::var_os("OPENBF_FLYER_BOTS").is_none_or(|v| v != "0"),
        }
    }

    /// Refuse to build a round whose switches differ from this process's.
    pub fn check_local(&self) -> Result<()> {
        let mine = Self::local();
        let mut differ = Vec::new();
        if mine.doors_solid != self.doors_solid {
            differ.push(format!("OPENBF_DOORS_SOLID={}", u8::from(self.doors_solid)));
        }
        if mine.plnp_mission != self.plnp_mission {
            differ.push(format!(
                "OPENBF_PLNP_MISSION={}",
                u8::from(self.plnp_mission)
            ));
        }
        if mine.flyer_bots != self.flyer_bots {
            differ.push(format!("OPENBF_FLYER_BOTS={}", u8::from(self.flyer_bots)));
        }
        if !differ.is_empty() {
            bail!(
                "the host plays with {}; set the same here",
                differ.join(" ")
            );
        }
        Ok(())
    }
}

/// One player's aim-assist settings ([`AssistProfile`] as data).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssistSettings {
    pub intensity: f32,
    pub pad_look_sensitivity: f32,
    pub armed: bool,
    pub lead_on: bool,
}

impl Default for AssistSettings {
    fn default() -> Self {
        let s = crate::profile::Settings::default();
        Self {
            intensity: s.assist_intensity,
            pad_look_sensitivity: s.pad_look_sensitivity,
            armed: s.aim_assist,
            lead_on: true,
        }
    }
}

impl AssistSettings {
    /// The sim's profile; `space` keeps the stick as it is
    /// (`WalkSim::pad_look_for_battle`).
    pub fn to_sim(self, space: bool) -> AssistProfile {
        AssistProfile {
            intensity: crate::profile::clamp_assist_intensity(self.intensity),
            pad_look_sensitivity: if space {
                1.0
            } else {
                crate::profile::clamp_pad_look(self.pad_look_sensitivity)
            },
            armed: self.armed,
            lead_on: self.lead_on,
        }
    }
}

/// What a player's saved profile gives the sim (LAN §4.1): medal counts,
/// the class-unlock counters `unlocks::class_cost` reads, aim assist and the
/// award-bonus switch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerProfile {
    pub name: String,
    pub medals: MedalCounts,
    /// Career kills per [`crate::unlocks::UNLOCKABLES`] counter, in its order.
    pub unlock_kills: [u32; 8],
    pub assist: AssistSettings,
    /// `Settings::career_awards`: earned medals grant their bonuses.
    pub award_bonuses: bool,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            medals: MedalCounts::default(),
            unlock_kills: [0; 8],
            assist: AssistSettings::default(),
            award_bonuses: true,
        }
    }
}

impl PlayerProfile {
    pub fn from_profile(p: &crate::profile::Profile) -> Self {
        let s = &p.settings;
        Self {
            name: p.name.chars().take(net::msg::MAX_NAME_CHARS).collect(),
            medals: p.career.medals,
            unlock_kills: std::array::from_fn(|i| crate::unlocks::UNLOCKABLES[i].kills(&p.career)),
            assist: AssistSettings {
                intensity: s.assist_intensity,
                pad_look_sensitivity: s.pad_look_sensitivity,
                armed: s.aim_assist,
                lead_on: true,
            },
            award_bonuses: s.career_awards,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("profile encodes")
    }

    /// A malformed profile decodes as the default one: every peer decodes the
    /// same bytes, so they agree either way.
    pub fn decode_or_default(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }
}

/// A human in the round from its start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HumanSeat {
    pub slot: u8,
    pub name: String,
    /// `None`: the side select (Instant Action). Slot 0 always gets the side
    /// select, as the shell's player does.
    pub team: Option<u8>,
    /// Starting cheats ([`super::cheat_bits`] order).
    pub cheats: u32,
    pub profile: PlayerProfile,
}

fn awards_default() -> bool {
    true
}

/// Everything every peer needs to build the identical round (LAN §4.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundSetup {
    /// The baked map's key (its directory under the assets root).
    pub map: String,
    pub mode: String,
    /// The playlist row's armies, before the mission pick orders them.
    pub armies: [String; 2],
    pub seed: u32,
    pub difficulty: core::skill::Difficulty,
    pub options: RoundOptions,
    /// The host profile's `team_handicap_on`.
    pub team_handicap: bool,
    /// The cheats guests may ask for ([`super::cheat_bits`] order).
    pub cheat_policy: u32,
    /// The host's award policy (LAN §5b): each player's own earned awards
    /// apply; false: nobody's. On by default, as in single player.
    #[serde(default = "awards_default")]
    pub awards: bool,
    pub humans: Vec<HumanSeat>,
    /// The Instant Action playlist row this round plays.
    pub playlist_pos: u32,
    pub env: EnvSwitches,
}

impl RoundSetup {
    /// The shell's launch data as a round (`windowed.rs` `start_battle`):
    /// seed 42 + the host career's rounds, the host profile's tier.
    pub fn instant_action(
        map: &str,
        mode: &str,
        armies: [String; 2],
        session: &SessionOptions,
        host: &crate::profile::Profile,
    ) -> Self {
        Self {
            map: map.to_owned(),
            mode: mode.to_owned(),
            armies,
            seed: 42u32.wrapping_add(host.career.rounds),
            difficulty: host.settings.difficulty,
            options: RoundOptions::from_session(session),
            team_handicap: host.settings.team_handicap_on,
            cheat_policy: 0,
            awards: true,
            humans: Vec::new(),
            playlist_pos: 0,
            env: EnvSwitches::local(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("round setup encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let s: Self = serde_json::from_slice(bytes).context("round setup")?;
        if s.humans
            .iter()
            .any(|h| usize::from(h.slot) >= net::MAX_SLOTS)
        {
            bail!("round setup: a human slot is out of range");
        }
        Ok(s)
    }

    pub fn human(&self, slot: u8) -> Option<&HumanSeat> {
        self.humans.iter().find(|h| h.slot == slot)
    }
}
