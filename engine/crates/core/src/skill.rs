//! Difficulty tiers and the per-unit SKILL VECTOR (PLAN Log "PHASE C DESIGN —
//! perception, the skill vector and the four tiers", decisions 1 and 4).
//!
//! The originals keep an integer skill 1..=20 per unit and read every accuracy
//! parameter off curves in `t = (s − 1)/19` (`analysis/re-ai-difficulty.md`
//! §4.1, §7). We keep that scale, so a PS2 level is a NUMBER — the PS2's
//! non-human-team base skill is 14 at Normal and 17 at Elite (RE §2.1) — and
//! our four tiers set every bot's base skill on it.
//!
//! # The four tiers (the maintainer, 2026-10-28)
//!
//! "Normal (matching PS2 normal), Hard (matching ps2 elite), elite (step above
//! ps2 elite), Legendary (our highest difficulty level)" — and the same day,
//! after playing them: "Don't change difficulties under the hood, but just
//! rename the 4 difficulties to easy, normal, hard, elite. brand new player
//! starts on normal." So the rows below are those four, unchanged, under the
//! names Easy / Normal / Hard / Elite. Matching need not be exact (our Easy
//! and Normal may be a little dumber than the PS2's Normal and Elite), but
//! Hard and Elite are harder than anything the PS2 has.
//!
//! History, so an old note reads right: before 2026-10-28 the rows were
//! Easy 8 / Normal 17 / Hard 20 / Elite 20+extras. The first rename that day
//! dropped Easy 8, called the 17 / 20 / 20+extras rows Hard / Elite /
//! Legendary and added a new Normal row at 14 (the 17 row's columns at the PS2
//! Normal's base skill 14). The second rename (above) shifted every name down
//! one: today's Easy is that 14 row, Normal the 17 row (the old Normal, the
//! row every gate, sweep and headless run has always used), Hard the 20 row
//! and Elite the 20+extras row.
//!
//! # The tier table — PROVISIONAL, maintainer-owned
//! measure: repeat issue #86's seeded proxy checks after sim changes; tune only by assignment.
//!
//! | tier | base skill | t | view mult | reaction s (ticks) | pressure ×human | err cap | bleed handicap |
//! |---|---|---|---|---|---|---|---|
//! | Easy | 14 | 0.684 | 1.0 | 0.3 (9) | 0.8 | 1.52 m | — |
//! | Normal | 17 | 0.842 | 1.0 | 0.3 (9) | 1.0 | 1.36 m | — |
//! | Hard | 20 | 1.0 | 1.25 | 0.2 (6) | 1.3 | 1.2 m | — |
//! | Elite | 20 | 1.0 | 1.5 | 0.15 (5) | 1.6 | **0.9 m** | ×1.25 |
//!
//! [ours] PROVISIONAL (the maintainer, 2026-10-29): Hard and Elite bots also
//! go to a health droid under 35 % health (`mend`); Elite bots also prefer
//! threats (`picks`) and attack the nearest unheld post (`hunts`). These are
//! the human proxy's choices (`ai::proxy::{mend, pick, front}`).
//!
//! Card DRETIRE (the maintainer, 2026-10-28): the "human damage ×" column (D12: a
//! bot's bolt or blast into a human ×1.15 at Hard, ×1.3 at Elite) is gone,
//! every tier takes 1.0: "Dying faster from a few bullets is a cheap way to
//! do this. We should make the competition actually stiffer, not just give
//! the enemy better weapons." Its host option `human_damage_on` went with
//! it. The pressure column is a different lever (whom a bot picks as its
//! target, `perception`), not damage, and stays.
//!
//! # Humans change their own team's bots (card DTEAM, the maintainer 2026-10-28) [ours]
//!
//! "Enemy bots stronger than teammates: Yes but there is a floor/ceiling
//! [...] we don't want our teammates to just be so stupid they can't do
//! anything at all. [...] when users are even on each side, each side's bots
//! are even and then it comes down to the users playing better than the
//! other user." Only humans change bot skill; no map or side is balanced.
//!
//! A team's SURPLUS is its humans minus the other team's, never below 0, and
//! capped at [`SURPLUS_CAP`] (3): a 4th surplus human changes nothing more.
//! The other team's bots keep the host tier's row (Elite's extras included).
//! A team with surplus `s > 0` has its bots' skill lowered by the tier's
//! offset per surplus human, never below the tier's floor, and its bots play
//! the ROW that skill falls in (the highest tier, at most the host's, whose
//! base skill is at or below it), so Elite's extras (and Hard's columns past
//! one step) are off on the surplus team's own bots. Equal humans (0v0, 1v1,
//! 2v2 ...) give both teams the host row. [`team_skill`] is the rule:
//!
//! | host tier | per surplus human | floor | surplus 1 | surplus 2 | surplus 3+ |
//! |---|---|---|---|---|---|
//! | Easy (14) | 0 | 14 | 14 Easy | 14 Easy | 14 Easy |
//! | Normal (17) | −1 | 14 | 16 Easy row | 15 Easy row | 14 Easy row |
//! | Hard (20) | −2 | 11 | 18 Normal row | 16 Easy row | 14 Easy row |
//! | Elite (20+extras) | −1 | 11 | 19 Normal row | 18 Normal row | 17 Normal row |
//!
//! The floor 11 is the PS2's own teammates' skill (RE §2.1); Normal's floor
//! is one row down (Easy's 14), Easy (a new player's team should win) takes
//! no offset. OURS retune for issue #86 (2026-11-03): Hard/Elite previously
//! subtracted 3 per surplus human; current-code Good-proxy wins were only
//! 31/96 and 10/96. Hard subtracts 2; Elite subtracts 1. The
//! floor and cap are preserved. The final 1–16 check gives Hard 54/96
//! and Elite 25/96 wins at the original 0.8 m cap; seeds 17–32 also give 25/96.
//! Issue #86 review: the 0.9 m cap gives 13/48 training wins (seeds 1–8),
//! frozen before fresh confirmation. This changes only Elite's own aim cap.
//! PROVISIONAL; measure: both proxy sides, seeds 1–8 for selection and
//! fresh seeds 33+ for confirmation on the three difficulty-check maps after sim changes.
//! The Easy row differs from Normal's in more than the
//! base skill (pressure 0.8, no scope sight), so a surplus team on the Easy
//! row under a Normal host plays those columns too.
//!
//! The level is set at each bot's spawn ([`crate::sim::Sim::bot_skill`]), so
//! a human joining, leaving or changing team reaches a bot at its next life;
//! the row's columns are the team's and follow the head count at once.
//!
//! Card HOLD adds how quickly a bot runs from a grenade, as a chance per
//! landing (ours, PROVISIONAL — the maintainer's to tune; its shuffle and strafe went
//! with card TPOS):
//!
//! | tier | grenade (per landing) | grenade delay s |
//! |---|---|---|
//! | Easy | 60 % | 0.6–1.3 |
//! | Normal | 60 % | 0.6–1.3 |
//! | Hard | 85 % | 0–0.8 |
//! | Elite | 95 % | 0–0.8 |
//!
//! Card REACT adds what a bot on a position (a hub's, a fight's) does when
//! it is HIT (on a Tactician's position, card TPOS, the "moves" share
//! leaves it at once and two hits leave it) — one draw per hit, 1.5 s apart at least (ours, PROVISIONAL —
//! The maintainer's to tune; the PS2's attacker ignores the hit):
//!
//! | tier | stays | strafes 2.5 m | rolls aside | moves to a new position |
//! |---|---|---|---|---|
//! | Easy | 35 % | 30 % | 15 % | 20 % |
//! | Normal | 35 % | 30 % | 15 % | 20 % |
//! | Hard | 20 % | 30 % | 20 % | 30 % |
//! | Elite | 10 % | 30 % | 25 % | 35 % |
//!
//! Card TEAM / FLNK (ours): every tier passes on what it sees and
//! flanks; Hard and Elite also cover a capturer (`Tier::cover`).
//! No tier gives a bot the head (or vehicle critical-prim) hit multiplier
//! on a ground round (`Sim::hit_scale`, playtest 2026-11-05 B1); space
//! rounds keep it from Hard up.
//!
//! Only the SKILL SCALE and the curves below are the RE's. Every column of
//! that table is the lead's design placeholder: the base skills, the view
//! multipliers, the reaction floor (the PS2 has **no** reaction timer at all —
//! RE §4.3: its "reaction feel" is the 5-frame think cadence plus the error
//! floor), the pressure weight and the bleed handicap. They are measured by the
//! arena and moved only under a maintainer tuning assignment. Issue #86 is
//! that assignment: OURS Elite cap 0.9 m; measure: the Good-proxy
//! selection seeds 1–8 and confirmation seeds 9–32 on both sides of CHECK_MAPS.
//!
//! The PS2's own per-level bases, for scale (RE §2.1): BF2 non-human teams
//! 4 / 14 / 17 at Easy / Medium / Hard (shown Easy / Normal / Elite),
//! human-team teammates 6 / 11 / 11.

use crate::fmath::FloatExt;

/// The four host difficulty tiers (PLAN §Saves: host authority), ordered
/// easiest first (the arena's head-to-head takes the lower of two).
///
/// The default is [`Difficulty::Normal`] (skill 17): the row every gate,
/// sweep and headless run has always used, and where a brand-new profile
/// starts (the maintainer, 2026-10-28).
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
    /// A profile saved as "legendary" (the top tier's name for part of
    /// 2026-10-28) loads as Elite, the same row.
    #[serde(alias = "legendary")]
    Elite,
}

impl Difficulty {
    /// Every tier, in table order — the arena's row order.
    pub const ALL: [Difficulty; 4] = [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Elite,
    ];

    /// One byte for [`crate::sim::Sim::hash`]. Each ROW keeps its byte
    /// through both 2026-10-28 renames (skill 14 0, skill 17 1, skill 20 2,
    /// 20+extras 3), so a run's hash does not move with a name.
    pub fn bytes(self) -> [u8; 1] {
        [match self {
            Difficulty::Easy => 0,
            Difficulty::Normal => 1,
            Difficulty::Hard => 2,
            Difficulty::Elite => 3,
        }]
    }

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
            Difficulty::Elite => "elite",
        }
    }

    /// `easy|normal|hard|elite`, case-insensitive; `None` for anything
    /// else.
    pub fn parse(s: &str) -> Option<Difficulty> {
        match s.trim().to_ascii_lowercase().as_str() {
            "easy" => Some(Difficulty::Easy),
            "normal" => Some(Difficulty::Normal),
            "hard" => Some(Difficulty::Hard),
            "elite" => Some(Difficulty::Elite),
            _ => None,
        }
    }

    /// The tier's row skill: every bot of a team with no human surplus takes
    /// it at spawn (a surplus team's bots take less, [`team_skill`], card
    /// DTEAM). Easy and Normal are the PS2's Normal and Elite bases
    /// (RE §2.1).
    pub fn base_skill(self) -> u8 {
        match self {
            Difficulty::Easy => 14,
            Difficulty::Normal => 17,
            Difficulty::Hard | Difficulty::Elite => 20,
        }
    }

    /// The per-tier columns of the table above.
    pub fn tier(self) -> Tier {
        match self {
            // Normal's columns at the PS2 Normal's skill (the error cap is
            // the skill's own, 1.52 m at 14).
            Difficulty::Easy => Tier {
                view_mult: 1.0,
                reaction_ticks: 9,
                pressure: 0.8,
                err_cap: None,
                bleed_handicap: 1.0,
                react: [0.30, 0.15, 0.20],
                grenade: 0.60,
                grenade_delay: (18, 39),
                cover: false,
                scope_sight: 0.0,
                picks: false,
                mend: 0.0,
                hunts: false,
            },
            Difficulty::Normal => Tier {
                view_mult: 1.0,
                reaction_ticks: 9,
                pressure: 1.0,
                err_cap: None,
                bleed_handicap: 1.0,
                react: [0.30, 0.15, 0.20],
                grenade: 0.60,
                grenade_delay: (18, 39),
                cover: false,
                scope_sight: 0.75,
                picks: false,
                mend: 0.0,
                hunts: false,
            },
            Difficulty::Hard => Tier {
                view_mult: 1.25,
                reaction_ticks: 6,
                pressure: 1.3,
                err_cap: None,
                bleed_handicap: 1.0,
                react: [0.30, 0.20, 0.30],
                grenade: 0.85,
                grenade_delay: (0, 24),
                cover: true,
                scope_sight: 1.0,
                picks: false,
                mend: 0.35,
                hunts: false,
            },
            Difficulty::Elite => Tier {
                view_mult: 1.5,
                reaction_ticks: 5,
                pressure: 1.6,
                // OURS, issue #86: still tighter than Hard's authored-curve cap.
                // measure: seeded Good-proxy selection/confirmation in the module header.
                err_cap: Some(0.9),
                bleed_handicap: 1.25,
                react: [0.30, 0.25, 0.35],
                grenade: 0.95,
                grenade_delay: (0, 24),
                cover: true,
                scope_sight: 1.0,
                picks: true,
                mend: 0.35,
                hunts: true,
            },
        }
    }

    /// The skill vector a bot of this tier carries: [`Skill::from_level`] at
    /// [`Difficulty::base_skill`], with the tier's own `err_cap` override
    /// applied (Elite).
    pub fn skill(self) -> Skill {
        self.skill_at(self.base_skill())
    }

    /// The vector a bot of integer skill `s` carries UNDER THIS TIER: the
    /// level curve ([`Skill::from_level`]) with the tier's own overrides
    /// applied — today only Elite's tighter error cap. One place, so the
    /// think and `Difficulty::skill` cannot drift.
    pub fn skill_at(self, s: u8) -> Skill {
        let mut k = Skill::from_level(s);
        if let Some(cap) = self.tier().err_cap {
            k.err_cap = cap;
        }
        k
    }
}

/// Card DTEAM: the most surplus humans a team's bots answer to. A team's
/// 4th and later surplus human changes nothing more.
pub const SURPLUS_CAP: u8 = 3;

/// One team's bots under the DTEAM rule: the row whose columns they play and
/// the integer skill they spawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamSkill {
    pub tier: Difficulty,
    pub level: u8,
}

impl Difficulty {
    /// Card DTEAM: (skill lowered per surplus human, the floor it stops at).
    /// OURS, PROVISIONAL; measure: the module header's seeded Good-proxy check.
    pub fn ally_offset(self) -> (u8, u8) {
        match self {
            Difficulty::Easy => (0, 14),
            Difficulty::Normal => (1, 14),
            Difficulty::Hard => (2, 11),
            Difficulty::Elite => (1, 11),
        }
    }

    /// The row a bot of integer skill `level` plays under this host tier: the
    /// highest tier, at most this one, whose base skill is at or below
    /// `level` (Easy below every base).
    pub fn row_for(self, level: u8) -> Difficulty {
        Difficulty::ALL
            .into_iter()
            .rev()
            .filter(|&d| d <= self)
            .find(|&d| d.base_skill() <= level)
            .unwrap_or(Difficulty::Easy)
    }
}

/// Card DTEAM [ours]: each team's bots under host tier `tier` with
/// `humans[t]` humans on team t (module header). A team with no human
/// surplus takes the host row; a surplus team's bots lose the tier's offset
/// per surplus human (at most [`SURPLUS_CAP`] of them), never below its
/// floor, and play the row that skill falls in.
pub fn team_skill(tier: Difficulty, humans: [u8; 2]) -> [TeamSkill; 2] {
    let (per, floor) = tier.ally_offset();
    [0usize, 1].map(|t| {
        let surplus = humans[t].saturating_sub(humans[1 - t]).min(SURPLUS_CAP);
        if surplus == 0 {
            return TeamSkill {
                tier,
                level: tier.base_skill(),
            };
        }
        let level = tier
            .base_skill()
            .saturating_sub(per * surplus)
            .max(floor.min(tier.base_skill()));
        TeamSkill {
            tier: tier.row_for(level),
            level,
        }
    })
}

/// The per-tier columns that are NOT a function of the integer skill. All
/// PROVISIONAL (lead placeholders, no RE) — see the module header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tier {
    /// Multiplies the PS2's view-distance shape `(20·bound_radius + 30)`
    /// (RE §5.1). PROVISIONAL.
    pub view_mult: f32,
    /// Ticks between a contact's FIRST SIGHTING and the tick it may be fired
    /// at. OURS: the PS2 has no reaction timer (RE §4.3). PROVISIONAL.
    pub reaction_ticks: u64,
    /// Target-choice weight on a HUMAN target (per human, not per team —
    /// PLAN "AI decision"). PROVISIONAL.
    pub pressure: f32,
    /// Elite's tighter tracked-position error cap; `None` = the skill's own.
    pub err_cap: Option<f32>,
    /// Ticket-bleed multiplier against a team that HAS a human when the
    /// opposing team has none (the team handicap, a host option). 1.0 = none.
    /// PROVISIONAL.
    pub bleed_handicap: f32,
    /// (Card TPOS: card HOLD's `shuffle` and `strafe` chances went with the
    /// shuffle and the strafe on a burst: a fighter's cycle and its next
    /// position are the Tactician's, `crate::ai::tactic`.)
    /// Card REACT (the maintainer, 2026-10-07): what a bot on a position does about a
    /// HIT, one draw per hit outside the cooldown — the chances to STRAFE,
    /// to ROLL aside and to MOVE to a new position; the rest it stays
    /// (`crate::ai` `react`). OURS and PROVISIONAL; a named deviation, the
    /// PS2's attacker ignores the hit.
    pub react: [f32; 3],
    /// ...when a grenade lands within its warning radius, to run clear of it.
    pub grenade: f32,
    /// The grenade reaction's delay in ticks, drawn uniformly in this range.
    /// The SHAPE is the PS2's (re-grenades.md §7.4 — BF1: U(1.1, 2.1) s under
    /// difficulty 14, U(0.6, 1.3) s under 17, else U(0, 0.8) s); which tier
    /// takes which range is ours.
    pub grenade_delay: (u64, u64),
    /// Card TEAM (`analysis/bot-design.md` 4.3, section 7): while a friend
    /// captures a post, up to two others inside its region take guard
    /// positions facing the approach (the board's Cover role,
    /// `ai::team::board`). OURS. The design's column: the 20 and 20+extras
    /// rows, Hard and Elite (names they also had before 2026-10-28). (The
    /// design's other two team columns, telling a friend what it sees and
    /// flanking, were every tier but the old skill-8 Easy; with that row gone
    /// they are every tier's and no column:
    /// `perception::tell` and `ai::team`'s flank board.)
    pub cover: bool,
    /// OURS, PROVISIONAL (the maintainer, 2026-10-29: "Easy would be what it
    /// was before, Normal is farther sniper sight, Hard and Elite are the full
    /// fix"): the share of a scoped rifle's `MaxRange` a still sniper sees a
    /// standing soldier at (`perception::scope_reach`); 0 = off, the plain
    /// sight and the old perch, band and stay rules.
    pub scope_sight: f32,
    /// [ours] PROVISIONAL: prefer visible threats, then hurt enemies.
    pub picks: bool,
    /// [ours] PROVISIONAL: seek a health droid below this health share.
    pub mend: f32,
    /// [ours] PROVISIONAL: attack the nearest unheld post, never defend.
    pub hunts: bool,
}

/// [ours] PROVISIONAL: the share of its skill a bot on foot's aim tracker
/// reads (its acceleration clamp and damping, [`Skill::tracker_for`]),
/// every tier and both teams alike. The PS2's law swings onto a new target
/// in 7-20 ticks at skills 14-20; the port's old law took 70-90, about what
/// the PS2's does at skills 4-7, a third of the skill (simulated: 0.35 fits
/// the old swing at skills 11-20 best). Every other skill term is the bot's own.
pub const TRACKER_SHARE: f32 = 0.35;

/// Everything the AI reads off its integer skill, computed once per think.
///
/// Every curve here is the RE's (`analysis/re-ai-difficulty.md` §4.1 —
/// `FUN_002a2c80..0x2a32e8`, the BF2 skill-parameter getters; §4.4 for the
/// burst/pause/charge shapes), evaluated at `t = (s − 1)/19` over the
/// originals' 1..=20 scale. `lerp(a, b)` below means `a + (b − a)·t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skill {
    /// The integer level this vector came from (clamped 1..=20).
    pub level: u8,
    /// `(level − 1)/19`.
    pub t: f32,
    /// `weapon+0x134` (BF1 `+0x154`): the weapon's own spread growth per shot
    /// is multiplied by this. `lerp(3, 1)`, RE §4.1 / §4.2 step 2.
    pub spread_mult: f32,
    /// `weapon+0x138` (BF1 `+0x158`): scales the AI distance-dispersion table
    /// in `GetYawSpread`. `lerp(2, 0)`, RE §4.1.
    pub dispersion_mult: f32,
    /// Aim-direction acceleration clamp, CUBIC in `t` (RE §4.1
    /// `FUN_002a2eb0` — `lerp(0.8, 14.0)` over `t³`).
    pub aim_accel: f32,
    /// Aim-direction damping, `lerp(0.8, 2.4)` (RE §4.1 `FUN_002a2ee0`).
    pub aim_damping: f32,
    /// Tracked-position error decay, m/s (RE §4.1 `FUN_002a2dc0`,
    /// `lerp(0.1, 1.1)`; the RE's sign is negative because the PS2 ADDS it).
    pub err_decay: f32,
    /// Tracked-position error FLOOR, m: `max(0, lerp(1.6, 0.4))`, a FLOAT
    /// and continuous in skill — 1.6 m at skill 1, 0.968 at 11, 0.4 at 20
    /// (`analysis/re-ai-sniper.md` §1, lead-verified 2026-09-08:
    /// `FUN_002a2df8` has NO integer conversion, Ghidra's
    /// `(int)f * (uint)(0.0 < f)` is the EE `max.s` at 0x2a2e28, and
    /// `FUN_002ae7b8` consumes it as a float).
    pub err_floor: f32,
    /// Tracked-position error CAP, m: `max(0, lerp(2.2, 1.2))`, likewise a
    /// float — 2.2 m at skill 1, 1.36 at 17, 1.2 at 20 (same source,
    /// `FUN_002a2e38`/`FUN_002a2e78`). A tier may override it
    /// ([`Tier::err_cap`]). The flyer cap `max(0, lerp(4.0, 1.2))` has no
    /// input in the port (no flyer targets).
    pub err_cap: f32,
    pub err_cap_flyer: f32,
    /// Burst-length fraction: `n = max(2, min + U(0, max − min)·t)`
    /// (RE §4.1 `FUN_002a2c90`, §4.4 `FUN_002bac98`).
    pub burst_t: f32,
    /// Inter-burst pause multiplier `1.5 − 0.8·t` — the RE's
    /// `(1.6 − lerp(0.1, 0.9)) × weaponclass+0x21c` (§4.1 `FUN_002a2cb8`).
    pub pause_mult: f32,
    /// A single-shot weapon's re-arm scale: the trigger comes up for
    /// `U(0.2, 2.0) × tap_x` seconds after each press, `tap_x = max(0, 1 −
    /// lerp(0.1, 0.9))` (BF2 `FUN_002baf70` over `FUN_002a2cf0`,
    /// `analysis/re-round-stalls.md` §2 C4; a real `max.s` at 0x2bb018).
    pub tap_x: f32,
    /// Charge weapons release at charge `U(0.15, charge_hi) × MaxPressedTime`
    /// (RE §4.1 `FUN_002a2d28`/`FUN_002a2d40`, §4.4).
    pub charge_hi: f32,
    /// Chance to use the grenade class against the current target (RE §4.1
    /// `FUN_002a2d68`): BF2's roll inside the grenade gate — the bot's number
    /// plus floor(10 t), mod 100, at most floor(100 x this) (card GRAI,
    /// `ai::arsenal::grenade_forced`). BF1's gate has no skill term.
    pub grenade_chance: f32,
    /// Chance to dodge on being hit, `lerp(0, 0.2)`, cooldown 2 s (RE §4.1
    /// `FUN_002a3018`; the dodge is a movement re-plan, NOT a roll — §9.4).
    pub dodge_chance: f32,
}

impl Skill {
    /// The vector at integer skill `s` (clamped to the originals' 1..=20).
    pub fn from_level(s: u8) -> Skill {
        let level = s.clamp(1, 20);
        let t = f32::from(level - 1) / 19.0;
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Skill {
            level,
            t,
            spread_mult: lerp(3.0, 1.0),
            dispersion_mult: lerp(2.0, 0.0),
            // RE §4.1: the acceleration getter is the one CUBIC curve.
            aim_accel: 0.8 + 13.2 * t * t * t,
            aim_damping: lerp(0.8, 2.4),
            err_decay: lerp(0.1, 1.1),
            // FLOATS, clamped at zero, never truncated: HOLD 19(g) was
            // ruled by the lead on 2026-09-08 (`analysis/re-ai-sniper.md` §1,
            // brief_phase_c LEAD ADDENDUM 4a) — the `(int)` in the decompiler
            // output is the EE's `max.s` against zero, and the consumer takes
            // a float. Commit 1 of this phase shipped `as i32 as f32` and the
            // "1 m below skill 12, then 0" rows with it; both are wrong.
            err_floor: lerp(1.6, 0.4).fmax(0.0),
            err_cap: lerp(2.2, 1.2).fmax(0.0),
            err_cap_flyer: lerp(4.0, 1.2).fmax(0.0),
            burst_t: t,
            pause_mult: 1.5 - 0.8 * t,
            tap_x: (1.0 - lerp(0.1, 0.9)).fmax(0.0),
            charge_hi: t,
            grenade_chance: t,
            dodge_chance: lerp(0.0, 0.2),
        }
    }

    /// The skill a bot on foot's aim tracker reads: [`TRACKER_SHARE`] of its
    /// own, rounded, floored at 1.
    pub fn tracker_skill(self) -> Skill {
        Skill::from_level(((f32::from(self.level) * TRACKER_SHARE).round() as u8).max(1))
    }

    /// The aim tracker's `(A, B)` = (acceleration clamp, damping), HALVED for
    /// a weapon that is not a `WeaponCannon` (RE §4.2 step 4,
    /// `DAT_00699ec8`).
    ///
    /// The foot gunner currently passes `true` for every weapon class: its
    /// tracker does not yet classify non-cannon weapons. The non-cannon
    /// scaling remains available and tested.
    pub fn tracker_for(self, is_cannon: bool) -> (f32, f32) {
        if is_cannon {
            (self.aim_accel, self.aim_damping)
        } else {
            (self.aim_accel * 0.5, self.aim_damping * 0.5)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_four_tiers_yield_the_designs_vector() {
        // The design's expected values (PLAN "PHASE C DESIGN", D1's table,
        // its rows renamed on 2026-10-28), recomputed from the RE's own
        // formulas; Easy's row is the curves at skill 14 (the RE §4.1
        // per-level table's row 14).
        let rows = [
            //  tier                 spread   disp    accel   damp    decay  floor cap   pause  dodge
            (
                Difficulty::Easy,
                1.632,
                0.632,
                5.028,
                1.895,
                0.784,
                0.778947,
                1.515789,
                0.953,
                0.137,
            ),
            (
                Difficulty::Normal,
                1.316,
                0.316,
                8.683,
                2.147,
                0.942,
                0.589474,
                1.357895,
                0.826,
                0.168,
            ),
            (
                Difficulty::Hard,
                1.0,
                0.0,
                14.0,
                2.4,
                1.1,
                0.4,
                1.2,
                0.7,
                0.2,
            ),
            (
                Difficulty::Elite,
                1.0,
                0.0,
                14.0,
                2.4,
                1.1,
                0.4,
                // The tier's own Elite extra overrides the skill's 1.2 m cap
                // (OURS, issue #86; measure: seeded Good-proxy wins). The PS2
                // curve remains 1.2 m at skill 20, ADDENDUM 4a.
                0.9,
                0.7,
                0.2,
            ),
        ];
        for (d, spread, disp, accel, damp, decay, floor, cap, pause, dodge) in rows {
            let s = d.skill();
            assert!(
                close(s.spread_mult, spread),
                "{d:?} spread {}",
                s.spread_mult
            );
            assert!(
                close(s.dispersion_mult, disp),
                "{d:?} disp {}",
                s.dispersion_mult
            );
            assert!(close(s.aim_accel, accel), "{d:?} accel {}", s.aim_accel);
            assert!(
                close(s.aim_damping, damp),
                "{d:?} damping {}",
                s.aim_damping
            );
            assert!(close(s.err_decay, decay), "{d:?} decay {}", s.err_decay);
            assert!(close(s.err_floor, floor), "{d:?} floor {}", s.err_floor);
            assert!(close(s.err_cap, cap), "{d:?} cap {}", s.err_cap);
            assert!(close(s.pause_mult, pause), "{d:?} pause {}", s.pause_mult);
            assert!(
                close(s.dodge_chance, dodge),
                "{d:?} dodge {}",
                s.dodge_chance
            );
        }
    }

    #[test]
    fn the_curves_match_the_res_per_level_table() {
        // analysis/re-ai-difficulty.md §4.1, the [D] per-level table, with
        // the error floor and cap as `analysis/re-ai-sniper.md` §1 rules them
        // (LEAD ADDENDUM 4a): floats, `max(0, lerp(...))`, so skill 11 is
        // checked on EVERY column now — the truncation that made the RE
        // table's 1 m at skill 11 unreachable is gone.
        let rows = [
            //  s   spread  disp   accel  damp   decay  cap
            (4u8, 2.68, 1.68, 0.85, 1.05, 0.26, 2.042105),
            (11, 1.95, 0.95, 2.72, 1.64, 0.63, 1.673684),
            (14, 1.63, 0.63, 5.03, 1.89, 0.78, 1.515789),
            (17, 1.32, 0.32, 8.68, 2.15, 0.94, 1.357895),
            (20, 1.00, 0.00, 14.0, 2.40, 1.10, 1.2),
        ];
        for (s, spread, disp, accel, damp, decay, cap) in rows {
            let k = Skill::from_level(s);
            // The RE's table is printed to 2-3 significant figures.
            assert!(
                (k.spread_mult - spread).abs() < 5e-3,
                "s{s} spread {}",
                k.spread_mult
            );
            assert!(
                (k.dispersion_mult - disp).abs() < 5e-3,
                "s{s} disp {}",
                k.dispersion_mult
            );
            assert!(
                (k.aim_accel - accel).abs() < 5e-3,
                "s{s} accel {}",
                k.aim_accel
            );
            assert!(
                (k.aim_damping - damp).abs() < 5e-3,
                "s{s} damping {}",
                k.aim_damping
            );
            assert!(
                (k.err_decay - decay).abs() < 5e-3,
                "s{s} decay {}",
                k.err_decay
            );
            assert!((k.err_cap - cap).abs() < 1e-6, "s{s} cap {}", k.err_cap);
        }
        // ADDENDUM 4a's pins, to 1e-6 (IEEE round-to-nearest against the
        // EE's round-toward-zero differ by <= 2.4e-7): the tier levels the
        // PS2 itself uses, from `re-ai-sniper.md` §1's full 0..20 table.
        let pins = [
            (4u8, 1.410_526_3, 2.042_105_2),
            (6, 1.284_210_6, 1.936_842_2),
            (11, 0.968_421, 1.673_684_2),
            (14, 0.778_947_4, 1.515_789_5),
            (17, 0.589_473_6, 1.357_894_8),
            (20, 0.4, 1.2),
        ];
        for (s, floor, cap) in pins {
            let k = Skill::from_level(s);
            assert!(
                (k.err_floor - floor).abs() < 1e-6,
                "s{s} floor {}",
                k.err_floor
            );
            assert!((k.err_cap - cap).abs() < 1e-6, "s{s} cap {}", k.err_cap);
        }
        // Both curves are continuous and monotonically falling in skill, and
        // the floor never reaches 0 inside 1..=20 (the `max(0, ...)` clamp is
        // dead code for a soldier, and is kept because the PS2 has it).
        for s in 2..=20u8 {
            let (a, b) = (Skill::from_level(s - 1), Skill::from_level(s));
            assert!(b.err_floor < a.err_floor && b.err_cap < a.err_cap);
            assert!(b.err_floor > 0.0, "floor at skill {s}");
        }
        assert_eq!(Skill::from_level(1).err_floor, 1.6);
        assert_eq!(Skill::from_level(1).err_cap, 2.2);
    }

    #[test]
    fn the_skill_scale_is_clamped_to_the_originals_range() {
        assert_eq!(Skill::from_level(0).level, 1);
        assert_eq!(Skill::from_level(200).level, 20);
        assert_eq!(Skill::from_level(1).t, 0.0);
        assert_eq!(Skill::from_level(20).t, 1.0);
        // Skill 0 (a player, a dummy) is never a bot's: it clamps to the
        // worst bot rather than dividing by zero anywhere downstream.
        assert!(Skill::from_level(0).spread_mult > 2.9);
    }

    #[test]
    fn the_foot_tracker_reads_a_share_of_the_skill() {
        for (s, t) in [(1, 1), (2, 1), (11, 4), (14, 5), (17, 6), (20, 7)] {
            let k = Skill::from_level(s).tracker_skill();
            assert_eq!(k.level, t, "skill {s}");
        }
        let k = Difficulty::Normal.skill().tracker_skill();
        assert_eq!(k.tracker_for(true), Skill::from_level(6).tracker_for(true));
    }

    #[test]
    fn a_non_cannon_weapon_halves_the_tracker() {
        let s = Difficulty::Elite.skill();
        assert_eq!(s.tracker_for(true), (s.aim_accel, s.aim_damping));
        let (a, b) = s.tracker_for(false);
        assert!(close(a, s.aim_accel * 0.5) && close(b, s.aim_damping * 0.5));
    }

    #[test]
    fn difficulty_parses_labels_case_insensitively_and_round_trips() {
        for d in Difficulty::ALL {
            assert_eq!(Difficulty::parse(d.label()), Some(d));
            assert_eq!(Difficulty::parse(&d.label().to_uppercase()), Some(d));
            assert_eq!(Difficulty::parse(&format!("  {}  ", d.label())), Some(d));
        }
        assert_eq!(Difficulty::parse("medium"), None);
        assert_eq!(Difficulty::parse("legendary"), None, "now Elite");
        assert_eq!(Difficulty::parse(""), None);
        // The default is the skill-17 row, the one every gate has always
        // run, and a brand-new player's.
        assert_eq!(Difficulty::default(), Difficulty::Normal);
        assert_eq!(Difficulty::default().base_skill(), 17);
        // The hash byte is distinct per tier.
        let mut seen: Vec<u8> = Difficulty::ALL.iter().map(|d| d.bytes()[0]).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn the_tier_columns_are_the_designs_table() {
        // (tier, base skill, view, reaction ticks, pressure, bleed,
        // react [strafe, roll, move], grenade, grenade delay, cover)
        type Row = (
            Difficulty,
            u8,
            f32,
            u64,
            f32,
            f32,
            [f32; 3],
            f32,
            (u64, u64),
            bool,
        );
        let want: [Row; 4] = [
            (
                Difficulty::Easy,
                14,
                1.0,
                9,
                0.8,
                1.0,
                [0.30, 0.15, 0.20],
                0.60,
                (18, 39),
                false,
            ),
            (
                Difficulty::Normal,
                17,
                1.0,
                9,
                1.0,
                1.0,
                [0.30, 0.15, 0.20],
                0.60,
                (18, 39),
                false,
            ),
            (
                Difficulty::Hard,
                20,
                1.25,
                6,
                1.3,
                1.0,
                [0.30, 0.20, 0.30],
                0.85,
                (0, 24),
                true,
            ),
            (
                Difficulty::Elite,
                20,
                1.5,
                5,
                1.6,
                1.25,
                [0.30, 0.25, 0.35],
                0.95,
                (0, 24),
                true,
            ),
        ];
        for (d, base, view, react, press, bleed, hit, gren, delay, cover) in want {
            let t = d.tier();
            assert_eq!(d.base_skill(), base, "{d:?}");
            assert_eq!((t.view_mult, t.reaction_ticks), (view, react), "{d:?}");
            assert_eq!((t.pressure, t.bleed_handicap), (press, bleed), "{d:?}");
            assert_eq!((t.react, t.grenade, t.grenade_delay), (hit, gren, delay));
            assert_eq!(t.cover, cover, "{d:?}");
        }
        assert_eq!(
            Difficulty::ALL.map(|d| d.tier().picks),
            [false, false, false, true]
        );
        assert_eq!(
            Difficulty::ALL.map(|d| d.tier().mend),
            [0.0, 0.0, 0.35, 0.35]
        );
        assert_eq!(
            Difficulty::ALL.map(|d| d.tier().hunts),
            [false, false, false, true]
        );
        // The scope's sight (the maintainer, 2026-10-29): Easy off.
        assert_eq!(
            Difficulty::ALL.map(|d| d.tier().scope_sight),
            [0.0, 0.75, 1.0, 1.0]
        );
        assert_eq!(Difficulty::Elite.tier().err_cap, Some(0.9));
        for d in [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard] {
            assert_eq!(d.tier().err_cap, None, "{d:?}: the skill's own cap");
        }
        // Easy and Normal are the PS2's Normal and Elite bases (RE §2.1).
        assert_eq!(Difficulty::Easy.base_skill(), 14);
        assert_eq!(Difficulty::Normal.base_skill(), 17);
    }

    /// The maintainer, 2026-10-28: four tiers, easiest first, named Easy / Normal /
    /// Hard / Elite; each ROW keeps its hash byte through the renames (skill
    /// 14 0, 17 1, 20 2, 20+extras 3), so a gate's hash does not move.
    #[test]
    fn four_tiers_in_order_keep_their_hash_bytes() {
        assert_eq!(
            Difficulty::ALL.map(Difficulty::label),
            ["easy", "normal", "hard", "elite"]
        );
        assert!(Difficulty::ALL.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(Difficulty::ALL.map(|d| d.bytes()[0]), [0, 1, 2, 3]);
        assert_eq!(
            Difficulty::ALL.map(Difficulty::base_skill),
            [14, 17, 20, 20]
        );
        assert_eq!(Difficulty::Elite.tier().err_cap, Some(0.9));
        // Every step up is at least as hard on every lever.
        for w in Difficulty::ALL.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (ta, tb) = (a.tier(), b.tier());
            assert!(a.base_skill() <= b.base_skill(), "{a:?} -> {b:?}");
            assert!(ta.view_mult <= tb.view_mult && ta.reaction_ticks >= tb.reaction_ticks);
            assert!(ta.pressure <= tb.pressure);
            assert!(ta.scope_sight <= tb.scope_sight);
            assert!(a.skill().err_cap >= b.skill().err_cap, "{a:?} -> {b:?}");
        }
    }

    /// Every name a profile may have saved loads as the row it names today:
    /// "easy" the skill-14 row, "normal" the 17 row, "hard" the 20 row,
    /// "elite" the 20+extras row, and "legendary" (that row's name for part
    /// of 2026-10-28) Elite too. An "elite" saved before 2026-10-28 meant the
    /// 20+extras row and still does. The four names round-trip.
    #[test]
    fn every_saved_name_loads_as_its_row() {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct S {
            d: Difficulty,
        }
        let load = |name: &str| -> Difficulty {
            ron::from_str::<S>(&format!("(d: {name})"))
                .map(|s| s.d)
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        };
        assert_eq!(load("easy"), Difficulty::Easy);
        assert_eq!(load("easy").base_skill(), 14);
        assert_eq!(load("normal"), Difficulty::Normal);
        assert_eq!(load("normal").base_skill(), 17);
        assert_eq!(load("hard"), Difficulty::Hard);
        assert_eq!(load("elite"), Difficulty::Elite);
        assert_eq!(load("elite").tier().err_cap, Some(0.9), "20+extras");
        assert_eq!(load("legendary"), Difficulty::Elite);
        for d in Difficulty::ALL {
            let text = ron::to_string(&S { d }).unwrap();
            assert_eq!(text, format!("(d:{})", d.label()));
            assert_eq!(load(d.label()), d);
        }
    }

    /// Card DTEAM (the maintainer, 2026-10-28): the per-team rule as a table, every
    /// tier, every head count the card names. `(humans, [team 0, team 1])`
    /// as (row, skill); the other order mirrors it.
    #[test]
    fn team_skill_is_the_cards_table() {
        use Difficulty::{Easy as Ea, Elite as El, Hard as Ha, Normal as No};
        let ts = |tier, level| TeamSkill { tier, level };
        let counts: [[u8; 2]; 10] = [
            [1, 0],
            [2, 0],
            [3, 0],
            [4, 0],
            [5, 0],
            [1, 1],
            [2, 1],
            [3, 1],
            [2, 2],
            [0, 0],
        ];
        // The surplus team's (row, skill) per tier at surplus 0, 1, 2, 3+.
        let ally = |host: Difficulty, s: u8| match (host, s.min(3)) {
            (h, 0) => ts(h, h.base_skill()),
            (Ea, _) => ts(Ea, 14),
            (No, 1) => ts(Ea, 16),
            (No, 2) => ts(Ea, 15),
            (No, _) => ts(Ea, 14),
            (Ha, 1) => ts(No, 18),
            (Ha, 2) => ts(Ea, 16),
            (Ha, _) => ts(Ea, 14),
            (El, 1) => ts(No, 19),
            (El, 2) => ts(No, 18),
            (El, _) => ts(No, 17),
        };
        // Written out once for Elite, the tier the maintainer plays: 1v0 .. 0v0.
        let elite = [
            [ts(No, 19), ts(El, 20)],
            [ts(No, 18), ts(El, 20)],
            [ts(No, 17), ts(El, 20)],
            [ts(No, 17), ts(El, 20)],
            [ts(No, 17), ts(El, 20)],
            [ts(El, 20), ts(El, 20)],
            [ts(No, 19), ts(El, 20)],
            [ts(No, 18), ts(El, 20)],
            [ts(El, 20), ts(El, 20)],
            [ts(El, 20), ts(El, 20)],
        ];
        for (h, want) in counts.iter().zip(elite) {
            assert_eq!(team_skill(El, *h), want, "elite {h:?}");
        }
        for host in Difficulty::ALL {
            for h in counts {
                let s0 = h[0].saturating_sub(h[1]);
                let want = [ally(host, s0), ts(host, host.base_skill())];
                assert_eq!(team_skill(host, h), want, "{host:?} {h:?}");
                // Mirrored: the same rule from the other side.
                let back = team_skill(host, [h[1], h[0]]);
                assert_eq!(back, [want[1], want[0]], "{host:?} mirrored {h:?}");
            }
        }
        // Normal 1v0, the default a new player meets: allies 16, enemies 17.
        assert_eq!(team_skill(No, [1, 0]).map(|t| t.level), [16, 17]);
    }

    /// The floor and the cap: no surplus team ever plays below skill 11 or
    /// below its tier's floor, a 4th surplus human changes nothing, and the
    /// team with fewer humans never moves.
    #[test]
    fn team_skill_holds_its_floor_and_cap() {
        for host in Difficulty::ALL {
            let (_, floor) = host.ally_offset();
            for a in 0..8u8 {
                for b in 0..8u8 {
                    let r = team_skill(host, [a, b]);
                    for t in r {
                        assert!(t.level >= 11 && t.level >= floor.min(host.base_skill()));
                        assert!(t.level <= host.base_skill() && t.tier <= host);
                        assert_eq!(t.tier, host.row_for(t.level), "{host:?} {a}v{b}");
                    }
                    if a <= b {
                        assert_eq!(r[0], team_skill(host, [0, 0])[0], "{a}v{b}");
                    }
                    if a >= b + 3 {
                        assert_eq!(r, team_skill(host, [b + 3, b]), "{host:?} cap {a}v{b}");
                    }
                }
            }
        }
    }
}
