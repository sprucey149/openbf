//! What a bot on foot is DOING: [`think`], one AI unit's tick as ONE priority
//! chain — perceive and retarget on the think period, then (every tick) the
//! chain that picks what walks the bot (the return to the level, the run from
//! a grenade, the held position, the vehicle it boards, the route leg) and
//! what it looks at — which hands exactly one [`AimRequest`] to the gunner
//! ([`super::gunner_foot`]) and exactly one [`MoveRequest`] to the mover
//! ([`super::mover`]). Card SPL11, `analysis/bot-design.md` 3: this file
//! writes no stick and no button.
//!
//! The gunner runs BETWEEN the two requests: the close hold, the fight and
//! the step out of a friend's line read its report
//! ([`super::gunner_foot::GunReport`]). Card TPOS deleted the strafe on a
//! new burst, and the step goes for a bot on a Tactician's position.
//!
//! Card TPOS: a fighter's position is the Tactician's when the map's table
//! has one for the fight (`super::tactic::fight`: walked to as a ring point,
//! `FightState::TAKE`, then its hide / fire cycle, `FightState::COVER`); a
//! hit there counts against it (`super::tactic::hit`) instead of card
//! REACT's row. FGHT's own style below is the fight when nothing passes.
//!
//! Card MOVR: the mover answers each request with an outcome
//! ([`super::Walk`]); a failed walk climbs [`brain::recover`]'s ladder, and the
//! link of the chain that ran is named in `AiState::behaviour`. The think's
//! phase is the unit's index.
//!
//! Card FGHT: a seen target inside the weapon's range is FOUGHT from a
//! position of the bot's own fixed style — it stands to shoot on a spot
//! clear of the other fighters, or circles on a ring round its target
//! (`fight_position`, `ring_point`, `stand_spot`; the PS2's attack modes 2
//! and 3, `analysis/re-soldier-ai.md` 4.4, 4.5).
//!
//! Card REACT: a bot on a position that is HIT answers by its tier's row —
//! it stays, strafes, rolls aside or moves to a new position (`react`; the
//! PS2's attacker ignores the hit, a named deviation); no such step goes
//! through a death region (`react_ok`), and a roll or a move ends in a hub
//! of the graph (`in_hub`).
//!
//! Here too, until their own cards replace them: the held position (card
//! HOLD: `spot_free`, `stand_ok`, `roll_clear`, the expired hold settled at a
//! hub, `hub_reached`; its shuffle and its hold at a hub went with card
//! TPOS),
//! the run from a grenade (`hazard_flee`) and the lost bot's jump
//! (`lost_jump`). The position, fight, reaction and dodge tests live in
//! `behave/position/tests.rs`.

//!
//! Split by responsibility: `brain` runs the priority chain; `position` owns
//! fight positions, reactions and escape probes; `position/tests.rs` holds
//! the tests.

#[cfg(doc)]
use crate::ai::gunner_foot::AimRequest;
#[cfg(doc)]
use crate::ai::mover::MoveRequest;
use crate::ai::{arsenal, man, ride, route, snipe, tactic};
#[cfg(doc)]
use crate::ai::{state::NavTick, PIN_FAILS};
use crate::unit::{Unit, UnitHandle};
use glam::Vec3;

mod brain;
mod position;
pub use brain::think;
pub(super) use position::lost_jump;

/// What one tick of a bot on foot reads once and every layer shares: the
/// unit, its AI eye, its weapon class, the difficulty tier and the unit's own
/// skill vector under the tier's overrides.
#[derive(Clone, Copy)]
pub(super) struct Tick<'a> {
    pub me: UnitHandle,
    pub u: &'a Unit,
    pub eye: Vec3,
    pub wc: &'a crate::weapon::WeaponClass,
    pub tier: crate::skill::Tier,
    pub skill: crate::skill::Skill,
}
