//! The WEAPON a bot on foot fights with, and its grenade (card GRAI,
//! `analysis/re-grenades.md` 7, `analysis/re-soldier-ai.md` 4.3): once a
//! think, with a seen target, [`think`] walks every slot the soldier holds as
//! the PS2's choice does ([`choose`]: ammunition, the AI's target-kind mask,
//! the range band, "plenty of ammunition", the score) and keeps the answer in
//! [`ArmState`]; the grenade test ([`grenade_forced`]: a 3 s window in every
//! 30 s per bot, at once against a target in a group of its own side, BF2's
//! difficulty roll on top) forces a grenade in range, one thrower per target
//! and team (the explosive claim) and none with a friend by the target
//! (OURS, [`friend_by`]). The gunner ([`super::gunner_foot`]) asks
//! [`armed`] what that means for the pad this tick and [`lob_aim`] for the
//! aim of a throw: the PS2's low-arc ballistic solution ([`lob`]).
//!
//! The score is BF2's rate under BF2's rules ([`score`]; PROVISIONAL: two of
//! the PS2's five outcomes by ordnance class, `FUN_002aa548` / `FUN_002aa630`,
//! B2:230347-230389) and one shot's damage under BF1's ([`shot`]); BF2 does
//! not choose a weapon that is reloading. PROVISIONAL: the fallback's reach
//! stands in for the unread `FUN_002a9d28`; BF2's second skip when the choice
//! is applied (`FUN_002ab430`) is unread. "Plenty" asks the rounds a weapon was
//! ISSUED, as the PS2's does (7.5; BF2's Galactic Conquest "supplies" bonus,
//! which doubles that load, is not built). Not built: a
//! droideka target, the stand-to-throw mode of an `OffhandWeapon`.
//!
//! Card HVYW (`analysis/re-soldier-ai.md` 22): a target that RIDES is its
//! vehicle to the choice - the hull's kind against the mask, the score, and
//! the PS2's "too weak" rule with its exemption by ordnance
//! ([`against_vehicle`]): the launcher is drawn for a tank and a rifle is not
//! fired at one ([`ArmState::spare`]: the bot does not attack it at all -
//! OURS: past [`SPARE_BEYOND`]; nearer it fires what it holds and walks on).
//! The standing fight ([`ArmState::vehicle`]) is for a weapon with the hull
//! in its band; a grenade is thrown in passing, and (OURS) no bot stands for
//! a throw at a vehicle that cannot be made (`GunReport::no_throw`).
//! OURS (the maintainer, 2026-10-12), named deviations: a HEAVY weapon ([`Cand::heavy`]:
//! the launchers and the wrist rocket) is used on a soldier only in a crowd
//! of three ([`is_crowd`]) and never inside [`HEAVY_NEAR`], where the PS2
//! asks the mask (BF2 exempts a group of five, BF1 none) and fires the wrist
//! rocket as any other weapon; a heavy weapon's mask is not asked against a
//! vehicle; a bot holding one prefers a vehicle it can hurt as its target
//! ([`vehicle_bonus`]); a weapon that reloads is kept against a vehicle; and
//! a missile's line of fire is cleared of friends by what is walked before
//! the rocket gets there (`gunner_foot::friend_in_line`).

use super::behave::Tick;
use super::draw::{unit_unit, SALT_ARM, SALT_NUMBER};
use super::state::{AiState, ArmState};
use crate::autoturret::Turret;
use crate::remote::{Droid, DroidClass};
use crate::sim::{buttons, Sim};
use crate::soldier::GroundRules;
use crate::unit::{Unit, UnitHandle};
use crate::vehicle::VehicleKind;
use crate::weapon::{FireState, OrdKind, WeaponClass};
use crate::TICK_HZ;
use glam::Vec3;

/// The group test's radius round the target, m (7.2).
const GROUP_RADIUS: f32 = 9.0;
/// OURS, PROVISIONAL (card GRAIA): no grenade at a point a friend is this
/// near, m — a little under twice the thermal detonator's outer damage
/// radius (6.5 m: the grenade bounces and rolls, and the friend walks on).
/// MEASURED over 24 seeds x 32 maps with [`FRIEND_LEAD`]: team + self kills
/// 1.96 -> 1.19 % of kills and no map over 3 % (9 m without the lead:
/// 1.49 %, BF2 Tantive IV 4.1 %), for a fifth of the throws.
const FRIEND_RADIUS: f32 = 12.0;
/// ... nor where his walk takes him this long after the grenade lands, s.
const FRIEND_LEAD: f32 = 1.0;
/// OURS (card HVYW; the maintainer, 2026-10-12): a heavy weapon is used on a soldier
/// only when this many of his side stand within [`GROUP_RADIUS`] of him, he
/// among them, and none of the shooter's side (BF1's count of the grenade's
/// group test, in both games).
const CROWD: u32 = 3;
/// OURS (card HVYW): ... and never at a soldier nearer than this, m - past
/// the widest blast of a hand missile on the discs (BF1's wrist rocket, 5.5 m
/// outer; the launchers' 4.0), which the wrist rocket's `MinRange` 0 does not
/// keep the shooter out of.
const HEAVY_NEAR: f32 = 6.0;
/// A target faster than this is spared the grenade, m/s (`re-soldier-ai.md`
/// 22: 144.0 = 12^2 in both games' suitability tests).
const LOB_SPEED: f32 = 12.0;
/// A flyer slower than this may be attacked from the ground, m/s (22: BF2's
/// 256.0 = 16^2 beside its landed, taking-off and landing states, which the
/// port does not have; BF1's test has no flyer at all).
const FLYER_SPEED: f32 = 16.0;
/// OURS (card HVYW): a vehicle no weapon of the bot's is for is left alone
/// only past this, m. Inside it the bot fires what it holds: thirteen
/// riflemen stood round an AT-ST that sat on their post for 150 s with
/// nobody firing (BF1 Endor, seed 3), where the PS2's walker would have
/// fought them off and its soldiers thrown their grenades among their own.
/// The grenade's group radius ([`GROUP_RADIUS`]) and a little.
const SPARE_BEYOND: f32 = 10.0;
/// How long a group stamp forces the grenade, ticks (7.2: 5 s).
const GROUP_STAMP: u64 = 5 * TICK_HZ as u64;
/// How long an explosive claim on a soldier lasts, ticks (4.3: 10 s).
const CLAIM: u64 = 10 * TICK_HZ as u64;

/// How near the solution's elevation the soldier's pitch has to be for the
/// press, rad (OURS: about 1.5 m of range at 16 m).
pub(super) const LOB_PITCH: f32 = 0.01;

/// One slot of the soldier as the choice sees it.
#[derive(Clone, Copy)]
pub(super) struct Cand<'a> {
    pub slot: u8,
    pub class: &'a WeaponClass,
    /// The rounds the weapon was ISSUED, not what is left of them (7.5: the
    /// PS2's "plenty" asks the reserve's cap); `None` = endless.
    pub rounds: Option<u32>,
    /// Nothing left to fire or load.
    pub empty: bool,
    /// The weapon is reloading (BF2's weapon state 4).
    pub busy: bool,
    /// Card HVYW: a HEAVY weapon - missile ordnance whose AI mask refuses a
    /// person (the rocket and torpedo launchers), or one that is not the
    /// soldier's first weapon (the wrist rocket). OURS: used on a soldier
    /// only in a crowd ([`CROWD`]), and on a vehicle whatever its mask.
    pub heavy: bool,
}

impl Cand<'_> {
    /// A weapon the choice never takes: a repair tool, a dispenser, an
    /// ability, a cloak, or one with nothing left.
    fn idle(&self) -> bool {
        self.class.repair.is_some()
            || self.class.dispenses()
            || self.class.ability.is_some()
            || self.class.cloak.is_some()
            || self.empty
    }
}

/// The vehicle a choice is asked for (card HVYW, `re-soldier-ai.md` 22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VehAsk {
    /// Its hull pool when whole (the PS2's "too weak" asks half of it).
    pub max_health: f32,
    /// Its speed, m/s.
    pub speed: f32,
    pub kind: VehicleKind,
}

/// What the choice is asked for.
#[derive(Clone, Copy)]
pub(super) struct Ask {
    /// The target's kind (`Unit::kind`: person 0, animal 1, droid 2 ...).
    pub kind: u32,
    /// The squared distance to it, m^2.
    pub d2: f32,
    pub bf2: bool,
    /// The target stands in a group of its own side (BF2: such a soldier is
    /// exempt from the target-kind mask).
    pub group: bool,
    /// A team mate holds the explosive claim on the target.
    pub barred: bool,
    /// The grenade test forces a grenade in range.
    pub forced: bool,
    /// The coin between two weapons of one score.
    pub coin: bool,
    /// Card HVYW: the target rides this vehicle, and the choice is for IT
    /// (`kind` is its hull's).
    pub vehicle: Option<VehAsk>,
    /// Card HVYW (OURS): the soldier stands in a crowd of [`CROWD`].
    pub crowd: bool,
    /// The target is a remote droid of this MaxHealth (`kind` is its
    /// health type), and the choice is for it ([`against_droid`]).
    pub droid: Option<f32>,
}

/// May weapon `c` be used on a remote droid of `max_health` and health type
/// `kind`, and its score against it? The PS2 asks a droid (B2 kind 1,
/// `EntityDroid`) what it asks every target that is not a soldier
/// (`analysis/re-soldier-ai.md` 22; B2 `FUN_002aaba0`, B1 `FUN_002380a8`):
/// BF2's AI mask must hold the droid's health type (BF1 asks its mask of a
/// soldier against a soldier only); then the weapon must not be too weak -
/// BF2's rate x 10 s, BF1's one shot x min(the rounds issued, 50), over half
/// the droid's MaxHealth. A foot soldier's weak weapon is used only on a
/// flyer, hover, walker or building (22.1 step 5, B1 151700-151716), and a
/// droid is none of them.
pub(super) fn against_droid(c: &Cand, max_health: f32, kind: u32, bf2: bool) -> Option<f32> {
    // `fire_emitter` never hits a droid or a turret: a cone is no answer to one.
    if c.class.force.is_none() && c.class.emitter.as_ref().is_some_and(|e| e.is_cone()) {
        return None;
    }
    if bf2 && c.class.ai_target_kind >> kind.min(31) & 1 == 0 {
        return None;
    }
    let (s, dealt) = scored(c, kind, bf2)?;
    (dealt > 0.5 * max_health).then_some(s)
}

/// Weapon `c`'s score against health type `kind` and what the "too weak"
/// test weighs of it (`re-soldier-ai.md` 22): BF2's score x 10 s, BF1's one
/// shot x min(the rounds issued, 50). `None` when it scores nothing.
fn scored(c: &Cand, kind: u32, bf2: bool) -> Option<(f32, f32)> {
    let s = if bf2 {
        score(c.class, kind)
    } else {
        shot(c.class, kind)
    };
    if s <= 0.0 {
        return None;
    }
    let dealt = if bf2 {
        s * 10.0
    } else {
        s * c.rounds.map_or(50.0, |r| r.min(50) as f32)
    };
    Some((s, dealt))
}

/// Card HVYW: may weapon `c` be used on the vehicle `v` at all, and its score
/// against it (`analysis/re-soldier-ai.md` 4.3 and 22; B2 `FUN_002aaba0`
/// 230557-230784, B1 `FUN_002380a8` 151570-151776 - the same shape in both)?
/// The AI's mask has to allow the hull's kind (BF2; BF1 asks its mask only
/// of a soldier against a soldier) - OURS: a heavy weapon's is not asked,
/// which is the wrist rocket's `TargetVehicle 0` (the maintainer, 2026-10-12). Then
/// "too weak": BF2's rate x 10 s, BF1's one shot x min(the rounds issued, 50),
/// is no more than half the hull's full pool. A weak weapon is used all the
/// same on a hover, a walker, a turret (PROVISIONAL: the PS2's is a building
/// with a flag that was not identified; the port's turrets are all taken)
/// or, in BF2, a slow flyer, when its ordnance is a missile, a shell or a
/// beam, or a grenade or sticky against a hull slower than 12 m/s - and by
/// no other: a rifle is not fired at a tank. A flyer in the air is not
/// attacked from the ground (4.4). Not built: the PS2's melee and remote
/// weapons' fixed 1000, BF1's cannon score of 1000 on a target class with a
/// field that was not identified (22).
pub(super) fn against_vehicle(c: &Cand, v: &VehAsk, kind: u32, bf2: bool) -> Option<f32> {
    if v.kind == VehicleKind::Flyer && v.speed >= FLYER_SPEED {
        return None;
    }
    if bf2 && !c.heavy && c.class.ai_target_kind >> kind.min(31) & 1 == 0 {
        return None;
    }
    let (s, dealt) = scored(c, kind, bf2)?;
    let weak = dealt <= 0.5 * v.max_health;
    let by_ordnance = match c.class.ordnance.kind {
        OrdKind::Missile | OrdKind::Shell | OrdKind::Beam => true,
        OrdKind::Lobbed => v.speed < LOB_SPEED,
        OrdKind::Other => false,
    } && (bf2 || v.kind != VehicleKind::Flyer);
    (!weak || by_ordnance).then_some(s)
}

/// BF2's score: the expected damage a second of weapon `wc` against a target
/// of `kind` (module header: PROVISIONAL). A cannon: a clip's damage over the
/// time to fire and reload it (an endless or instantly reloaded one: a
/// salvo's damage over the shot delay); a grenade weapon: a clip's damage
/// over ShotDelay + ReloadTime.
pub(super) fn score(wc: &WeaponClass, kind: u32) -> f32 {
    // PS2 FUN002aaba0 gives the melee class a fixed desirability.
    if wc.saber.is_some() {
        return 1000.0;
    }
    let dmg = shot(wc, kind);
    let rounds = wc.rounds_per_clip as f32;
    if wc.thrown.is_some() {
        let t = wc.shot_delay[0] + wc.reload_time;
        return rounds.max(1.0) * dmg / if t > 0.0 { t } else { 1.0 };
    }
    let delay = (0.5 * (wc.shot_delay[0] + wc.shot_delay[1])).max(1e-3);
    let salvo = wc.shots_per_salvo.max(1) as f32;
    if wc.reload_time <= 0.0 || wc.rounds_per_clip == 0 {
        salvo * dmg / delay
    } else {
        rounds * dmg / ((rounds / salvo).floor() * delay + wc.reload_time)
    }
}

/// BF1's score (7.5; B1 `FUN_002380a8`): ONE shot's damage against a target
/// of `kind` — the ordnance's plus its explosion's, no rate. PROVISIONAL:
/// the PS2 weighs the explosion by its falloff 2 m out less half the
/// target's size (B1 `FUN_00237ff8` / `FUN_00221530`), taken as 1 here.
pub(super) fn shot(wc: &WeaponClass, kind: u32) -> f32 {
    let o = &wc.ordnance;
    let k = (kind as usize).min(o.scales.len() - 1);
    if let Some(g) = &wc.guided {
        return g
            .class
            .explosion
            .as_ref()
            .map_or(0.0, |e| e.damage * e.scales[k]);
    }
    let mut dmg = o.max_damage * o.scales[k];
    if let Some(x) = o.sticky.as_ref().and_then(|s| s.explosion.as_ref()) {
        dmg += x.damage * x.scales[k];
    }
    dmg
}

/// The PS2's weapon choice (7.1; B2 `FUN_002ab590` 231060-231130): the slots
/// in order — the slot picked and whether the target is inside its band.
/// The first weapon in range is taken; a later one replaces it when it was
/// issued plenty of ammunition (over 20 rounds in BF1, over 2 in BF2: a rifle
/// is "plenty" until it is empty, BF1's 3-4 detonators never are) and the
/// best was not, or on a higher score at equal
/// "plenty"; a forced grenade in range is taken whatever the best is (and a
/// later weapon can still replace it by the same rule). With nothing in
/// range, the same preference among the weapons whose reach holds the target.
/// BF2 only (7.5; B2:231043-231050): a weapon that is reloading is passed
/// over (PROVISIONAL: the PS2 skips its state 5 too, unidentified).
pub(super) fn choose<'a>(cands: impl Iterator<Item = Cand<'a>>, ask: &Ask) -> Option<(u8, bool)> {
    let mut best: Option<u8> = None;
    let (mut best_plenty, mut best_score, mut in_range) = (false, -1.0f32, false);
    for c in cands {
        let grenade = c.class.thrown.is_some();
        // Card HVYW (OURS): against a vehicle a weapon that is reloading
        // is not passed over - the bot with the one weapon that hurts it
        // reloads where it stands instead of walking on for the four seconds.
        let reloads = ask.bf2 && c.busy && ask.vehicle.is_none();
        if c.idle() || (grenade && ask.barred) || reloads {
            continue;
        }
        // Card HVYW: a heavy weapon in a crowd is taken as a forced grenade
        // is; at any other soldier it is passed over (OURS).
        let heavy = c.heavy && ask.vehicle.is_none();
        let s = if ask.bf2 && c.class.saber.is_some() {
            1000.0
        } else if let Some(v) = ask.vehicle {
            match against_vehicle(&c, &v, ask.kind, ask.bf2) {
                Some(s) => s,
                None => continue,
            }
        } else if let Some(max) = ask.droid {
            match against_droid(&c, max, ask.kind, ask.bf2) {
                Some(s) => s,
                None => continue,
            }
        } else {
            let allowed = c.class.ai_target_kind >> ask.kind.min(31) & 1 != 0;
            if heavy {
                if !ask.crowd || ask.d2 <= HEAVY_NEAR * HEAVY_NEAR {
                    continue;
                }
            } else if !(allowed || (ask.bf2 && ask.group)) {
                continue;
            }
            if ask.bf2 {
                score(c.class, ask.kind)
            } else {
                shot(c.class, ask.kind)
            }
        };
        if s <= 0.0 {
            continue;
        }
        let plenty = c.rounds.is_none_or(|r| r > if ask.bf2 { 2 } else { 20 });
        let higher = s > best_score || (s == best_score && ask.coin);
        let by_rule = if best_plenty {
            plenty && higher
        } else {
            plenty || higher
        };
        let [min, _, _] = c.class.ai_range;
        let max = reach(c.class);
        let take = if min * min < ask.d2 && ask.d2 < max * max {
            !in_range || by_rule || (grenade && ask.forced) || heavy
        } else {
            let fallback = if c.class.force.is_some() || c.class.saber_throw.is_some() {
                max
            } else {
                max.max(c.class.shot_range())
            };
            !in_range && ask.d2 < fallback * fallback && by_rule
        };
        if take {
            in_range |= min * min < ask.d2 && ask.d2 < max * max;
            best = Some(c.slot);
            best_plenty = plenty;
            best_score = s;
            // A heavy weapon with its crowd in range is the answer: no
            // later slot takes its place (a forced grenade's can).
            if heavy && in_range {
                break;
            }
        }
    }
    best.map(|s| (s, in_range))
}

/// Card SNPR (OURS): a target inside this and coming makes a sniper draw
/// its pistol, m (the maintainer: "within 25 m and closing")...
pub(super) const PISTOL_NEAR: f32 = 25.0;
/// ... and the rifle comes back only past this, or past the pistol's own
/// reach when that is nearer (a pistol's is 24 m), m.
pub(super) const RIFLE_BACK: f32 = 40.0;
/// "Closing": the target's speed toward the shooter, m/s (a walk is 4.5).
const CLOSING: f32 = 1.0;

/// Card SNPR (OURS, the maintainer 2026-10-12): a sniper's weapon against a soldier
/// at `d` m. Its pistol (the weapon of shortest reach it holds that fires
/// bolts at a person, neither scoped, thrown nor heavy) when the target is
/// inside the rifle's `MinRange`, or within [`PISTOL_NEAR`] and `closing`,
/// or when the last think's pick was the pistol and the target is not past
/// [`RIFLE_BACK`] nor the pistol's reach. `None`: none of these, not a
/// sniper with both, or the choice (`pick`) is another weapon - a forced
/// grenade, a heavy weapon in a crowd: the choice stands (it never loses a
/// pistol `choose` took). BF2 takes no weapon that reloads (7.5).
pub(super) fn sidearm(
    cands: &[Option<Cand>; 8],
    pick: Option<(u8, bool)>,
    last: Option<u8>,
    d: f32,
    closing: bool,
    bf2: bool,
) -> Option<(u8, bool)> {
    let usable = |c: &&Cand| !(c.empty || (bf2 && c.busy));
    let rifle = cands
        .iter()
        .flatten()
        .filter(usable)
        .find(|c| c.class.ai_distance_table())?;
    let pistol = cands
        .iter()
        .flatten()
        .filter(usable)
        .filter(|c| {
            !c.class.ai_distance_table()
                && c.class.thrown.is_none()
                && !c.heavy
                && c.class.ordnance.kind == OrdKind::Other
                && c.class.ai_target_kind & 1 != 0
        })
        .min_by(|a, b| a.class.ai_range[2].total_cmp(&b.class.ai_range[2]))?;
    if pick.is_some_and(|(s, _)| s != rifle.slot && s != pistol.slot) {
        return None;
    }
    let [rmin, _, _] = rifle.class.ai_range;
    // Kept no farther than the pistol fires: the trigger takes no shot past
    // its MaxRange (`gunner_foot`, card CLSE4), so a pistol kept to 40 m
    // would stand silent from 24 m.
    let keep = RIFLE_BACK
        .min(pistol.class.ai_range[2])
        .min(pistol.class.shot_range());
    // Never drawn past that reach either: a BF1 rifle's MinRange (20-32 m)
    // is wider than a pistol's, and the rifle fires there (the trigger asks
    // no MinRange).
    let near =
        d <= keep && (d <= rmin || (d <= PISTOL_NEAR && closing) || last == Some(pistol.slot));
    if !near {
        return None;
    }
    let [min, _, max] = pistol.class.ai_range;
    Some((pistol.slot, min < d && d < max))
}

/// The 3 s window of every 30 s in which bot number `n` considers its
/// grenade (7.2: n mod 10 == floor(t / 3) mod 10).
pub(super) fn in_window(n: u32, tick: u64) -> bool {
    u64::from(n % 10) == (tick / (3 * TICK_HZ as u64)) % 10
}

/// The grenade test (7.2; B1 `FUN_00238678`, B2 `FUN_002ab288`): is a
/// grenade weapon in range forced on this bot? BF1: in its window, or with
/// the target's group stamp fresh. BF2: that, and the difficulty roll
/// (n + floor(10 t)) mod 100 <= floor(100 x `chance`).
pub(super) fn grenade_forced(
    rules: GroundRules,
    n: u32,
    tick: u64,
    group: bool,
    chance: f32,
) -> bool {
    let gate = group || in_window(n, tick);
    match rules {
        GroundRules::Bf1 => gate,
        GroundRules::Bf2 => {
            let tenths = tick * 10 / TICK_HZ as u64;
            gate && (u64::from(n) + tenths) % 100 <= (chance * 100.0) as u64
        }
    }
}

/// The group test's verdict from its two counts round the target (7.2):
/// `own` soldiers of the target's side within 9 m of it, the target included,
/// and `foes` of that side there. BF1: more than 2 and no friend of the
/// attacker; BF2: more than 3 beside the target and none hostile to it
/// (whether the PS2's count holds the target itself was not read).
pub(super) fn is_group(rules: GroundRules, own: u32, foes: u32) -> bool {
    foes == 0
        && match rules {
            GroundRules::Bf1 => own > 2,
            GroundRules::Bf2 => own > 4,
        }
}

/// The group test's two counts round soldier `e` (7.2): the soldiers on foot
/// of his side within [`GROUP_RADIUS`] of him, himself counted, and those of
/// any other side there. One count for the grenade ([`is_group`]) and for a
/// heavy weapon ([`is_crowd`]).
pub(super) fn group_counts(sim: &Sim, e: &Unit) -> (u32, u32) {
    let (mut own, mut foes) = (0, 0);
    for (_, o) in sim.alive_units() {
        if o.mount.is_some()
            || o.soldier.pos.distance_squared(e.soldier.pos) > (GROUP_RADIUS * GROUP_RADIUS)
        {
            continue;
        }
        if o.team == e.team {
            own += 1;
        } else {
            foes += 1;
        }
    }
    (own, foes)
}

/// Card HVYW (OURS): the crowd a heavy weapon is fired into - [`CROWD`] of
/// the target's side or more, and none of any other side by them (the
/// shooter's own: no rocket where a friend stands).
pub(super) fn is_crowd(own: u32, foes: u32) -> bool {
    foes == 0 && own >= CROWD
}

/// OURS (card GRAIA; a named deviation — the PS2 has no such test and a
/// tenth of its bots' kills are of their own side): is a soldier of `u`'s
/// team on foot, other than `u`, within [`FRIEND_RADIUS`] of `at` — where he
/// stands, or where his walk takes him in `lead` seconds? No grenade is
/// chosen for, or thrown at, such a point.
fn friend_by(sim: &Sim, u: &Unit, at: Vec3, lead: f32) -> bool {
    let near = |p: Vec3| p.distance_squared(at) < FRIEND_RADIUS * FRIEND_RADIUS;
    sim.alive_units().any(|(_, o)| {
        o.team == u.team
            && !std::ptr::eq(o, u)
            && o.mount.is_none()
            && (near(o.soldier.pos) || near(o.soldier.pos + o.soldier.vel * lead))
    })
}

/// Card HVYW (OURS): what the target choice adds to a seen contact's score
/// for bot `u` when the contact rides a vehicle - [`VEHICLE_BONUS`] when one
/// of `u`'s HEAVY weapons can be used on it and reaches (a bot with a
/// launcher takes the tank over the nearer rifleman), minus that when no
/// weapon `u` holds can be used on it at all (a rifleman takes the soldier
/// behind the tank he does not attack), else 0.
pub fn vehicle_bonus(sim: &Sim, u: &Unit, o: &Unit) -> f32 {
    if u.mount.is_some() {
        return 0.0;
    }
    match against_ride(sim, u, o) {
        None => 0.0,
        Some((true, _)) => VEHICLE_BONUS,
        Some((false, true)) => 0.0,
        Some((false, false)) => -VEHICLE_BONUS,
    }
}

/// What `u`'s weapons can do to the vehicle `o` rides (`None`: `o` is on
/// foot): a HEAVY weapon usable on it within its reach, and any weapon
/// usable on it at all ([`against_vehicle`]).
pub(super) fn against_ride(sim: &Sim, u: &Unit, o: &Unit) -> Option<(bool, bool)> {
    let v = o.mount.and_then(|m| sim.vehicle(m.vehicle))?;
    let ask = VehAsk {
        max_health: v.health.max,
        speed: v.vel.length(),
        kind: v.kind,
    };
    let (kind, bf2) = (
        v.health.health_type as u32,
        sim.ground_rules == GroundRules::Bf2,
    );
    let d2 = (o.capsule().center() - u.muzzle()).length_squared();
    let (mut heavy, mut any) = (false, false);
    for c in slots(sim, u).iter().flatten().filter(|c| !c.empty) {
        if against_vehicle(c, &ask, kind, bf2).is_some() {
            any = true;
            heavy |= c.heavy && d2 < (c.class.ai_range[2] * c.class.ai_range[2]);
        }
    }
    Some((heavy, any))
}

/// Card DFOCUS: does `u` hold a weapon for a soldier on foot - one with
/// rounds left that is neither heavy ([`Cand::heavy`]) nor a tool (a repair
/// or dispensing weapon, an ability)?
pub(super) fn arms_for_soldier(sim: &Sim, u: &Unit) -> bool {
    slots(sim, u).iter().flatten().any(|c| {
        !c.empty
            && !c.heavy
            && c.class.repair.is_none()
            && !c.class.dispenses()
            && c.class.ability.is_none()
    })
}

/// [`vehicle_bonus`]'s size: the whole span of the choice's distance term
/// (`perception::choose_target`: 200 at the muzzle, 100 at the view
/// distance), so a vehicle in sight outranks any soldier.
const VEHICLE_BONUS: f32 = 100.0;

/// The bot's own number 0..=100, drawn once (the PS2 controller's).
pub(super) fn number(me: UnitHandle) -> u32 {
    (unit_unit(me.index, 0, me.gen, SALT_NUMBER) * 101.0) as u32
}

/// Is the grenade of channel 1 being thrown (its fire state runs)?
fn throwing<'a>(sim: &'a Sim, u: &Unit) -> Option<&'a WeaponClass> {
    let w = &u.held[usize::from(u.cur1?)];
    let c = &sim.weapon_classes[w.class];
    (c.thrown.is_some() && w.state.state == FireState::Firing).then_some(c)
}

/// The soldier's weapons as the choice sees them, by slot.
pub(super) fn slots<'a>(sim: &'a Sim, u: &Unit) -> [Option<Cand<'a>>; 8] {
    // The soldier's first weapon: its lowest slot.
    let first = u.held.iter().map(|w| w.slot).fold(u.weapon_slot, u8::min);
    let cand = |slot, class: usize, w: &crate::weapon::WeaponState| {
        let class = &sim.weapon_classes[class];
        Cand {
            slot,
            class,
            rounds: w.issued(class),
            empty: w.out_of_ammo(class) || class.award_locked(sim.cheats.all_awards),
            busy: w.reloading(),
            heavy: (class.ordnance.kind == OrdKind::Missile || class.guided.is_some())
                && (class.ai_target_kind & 1 == 0 || slot != first),
        }
    };
    let mut cands = [None; 8];
    let hand = cand(u.weapon_slot, u.weapon_class, &u.weapon);
    for c in std::iter::once(hand).chain(u.held.iter().map(|w| cand(w.slot, w.class, &w.state))) {
        if let Some(at) = cands.get_mut(usize::from(c.slot)) {
            *at = Some(c);
        }
    }
    cands
}

/// Once a think: the weapon for the seen target `enemy` (module header).
pub(super) fn think(sim: &Sim, tk: &Tick, st: &mut AiState, enemy: Option<&Unit>) {
    let Tick { me, u, eye, .. } = *tk;
    if u.held.is_empty() || throwing(sim, u).is_some() {
        return;
    }
    let (Some(e), Some(target)) = (enemy, st.target.and_then(|t| t.unit())) else {
        st.arm = ArmState {
            claim: st.arm.claim.filter(|c| sim.tick < c.1),
            ..ArmState::default()
        };
        return;
    };
    // The group test, on its own clock.
    if sim.tick >= st.arm.group_next {
        let bf2 = sim.ground_rules == GroundRules::Bf2;
        st.arm.group_next = sim.tick + if bf2 { TICK_HZ as u64 } else { GROUP_STAMP };
        // BF1's stamp is good for 5 s and written at most once in 5 s;
        // BF2's counts are rebuilt once a second.
        let (own, foes) = group_counts(sim, e);
        st.arm.group_until = if is_group(sim.ground_rules, own, foes) {
            st.arm.group_next
        } else {
            0
        };
    }
    let group = sim.tick < st.arm.group_until;
    // The explosive claim: one bot of a team at a time on a soldier.
    let mine = st.arm.claim.filter(|c| c.0 == target && sim.tick < c.1);
    let barred = sim.alive_units().any(|(h, o)| {
        h != me
            && o.team == u.team
            && o.ai
                .arm
                .claim
                .is_some_and(|c| c.0 == target && sim.tick < c.1)
            && (mine.is_none() || h.index < me.index)
    }) || (u
        .held
        .iter()
        .any(|w| sim.weapon_classes[w.class].thrown.is_some())
        && friend_by(sim, u, e.soldier.pos, 0.0));
    let bf2 = sim.ground_rules == GroundRules::Bf2;
    let cands = slots(sim, u);
    // Card HVYW: a rider's vehicle is the target, and the choice is for it.
    let ridden = e.mount.and_then(|m| sim.vehicle(m.vehicle));
    let vehicle = ridden.map(|v| VehAsk {
        max_health: v.health.max,
        speed: v.vel.length(),
        kind: v.kind,
    });
    // The crowd is counted only for a bot that holds a heavy weapon.
    let crowd = vehicle.is_none() && cands.iter().flatten().any(|c| c.heavy && !c.empty) && {
        let (own, foes) = group_counts(sim, e);
        is_crowd(own, foes)
    };
    let ask = Ask {
        kind: ridden.map_or(e.kind(), |v| v.health.health_type as u32),
        d2: (e.capsule().center() - eye).length_squared(),
        bf2,
        vehicle,
        crowd,
        droid: None,
        group,
        barred,
        forced: grenade_forced(
            sim.ground_rules,
            number(me),
            sim.tick,
            group,
            tk.skill.grenade_chance,
        ),
        coin: unit_unit(me.index, sim.tick, me.gen, SALT_ARM) < 0.5,
    };
    let pick = choose(cands.into_iter().flatten(), &ask);
    // Card SNPR (OURS): a sniper's pistol, by distance and approach.
    let pick = if vehicle.is_none() {
        let to_me = (eye - e.soldier.pos).normalize_or_zero();
        let closing = e.soldier.vel.dot(to_me) >= CLOSING;
        sidearm(&cands, pick, st.arm.pick, ask.d2.sqrt(), closing, bf2).or(pick)
    } else {
        pick
    };
    st.arm.pick = pick.map(|p| p.0);
    st.arm.in_range = pick.is_some_and(|p| p.1);
    let thrown = pick.is_some_and(|(s, _)| {
        cands[usize::from(s)].is_some_and(|c: Cand| c.class.thrown.is_some())
    });
    // The standing fight against a vehicle is for a weapon in the hands
    // with the hull in its band. A grenade is thrown in passing, as at a
    // soldier, and a launcher inside its `MinRange` is fired on the way: a
    // bot that stood for the one grenade it could not throw from there
    // stood for good (BF1 Mos Eisley, seed 3: a scout 3 m from a skiff for
    // 100 s).
    st.arm.vehicle = vehicle.is_some() && st.arm.in_range && !thrown;
    // Nothing it holds is for this target: a vehicle is not attacked at
    // all - OURS: past [`SPARE_BEYOND`]; nearer, the bot fires what is in
    // its hands and walks on, as it did before the rule - and a heavy
    // weapon in the hands is not fired at a soldier for want of another
    // (the launcher at a lone man beyond the pistol's reach).
    st.arm.spare = pick.is_none()
        && (vehicle.is_some() && ask.d2 > SPARE_BEYOND * SPARE_BEYOND
            || vehicle.is_none()
                && cands[usize::from(u.weapon_slot).min(7)].is_some_and(|c: Cand| c.heavy));
    st.arm.claim = if thrown {
        Some((target, mine.map_or(sim.tick + CLAIM, |c| c.1)))
    } else {
        st.arm.claim.filter(|c| sim.tick < c.1)
    };
}

/// The [`Ask`] for the droid `d` of class `c`, seen by `u` from `eye`.
fn item_ask(
    sim: &Sim,
    me: UnitHandle,
    eye: Vec3,
    (pos, max_health, kind): (Vec3, f32, u32),
    forced: bool,
) -> Ask {
    Ask {
        kind,
        d2: (pos - eye).length_squared(),
        bf2: sim.ground_rules == GroundRules::Bf2,
        group: false,
        barred: false,
        forced,
        coin: unit_unit(me.index, sim.tick, me.gen, SALT_ARM) < 0.5,
        vehicle: None,
        crowd: false,
        droid: Some(max_health),
    }
}

/// Whether a weapon `u` holds may be used on a droid of class `c` at all:
/// the PS2's scan admits a candidate only through the weapon choice
/// (B2 `FUN_002e3aa0` -> `FUN_002ac8f8`, `analysis/re-remote-droid.md` 6).
pub fn droid_suited(sim: &Sim, u: &Unit, c: &DroidClass) -> bool {
    item_suited(sim, u, c.max_health, c.health_type as u32)
}

/// [`droid_suited`] for any bot-less item of `max_health` and health type
/// `kind`: a droid, or an auto-turret. The PS2's rule for a building adds a
/// weak weapon's ordnance test (22.1 steps 5-6), which no weapon that clears
/// half of the turret's 50 health in 10 s reaches.
pub fn item_suited(sim: &Sim, u: &Unit, max_health: f32, kind: u32) -> bool {
    let bf2 = sim.ground_rules == GroundRules::Bf2;
    slots(sim, u)
        .into_iter()
        .flatten()
        .any(|w| !w.idle() && against_droid(&w, max_health, kind, bf2).is_some())
}

/// [`think`] for a seen droid: the weapon the PS2's choice takes for it.
pub(super) fn think_droid(sim: &Sim, tk: &Tick, st: &mut AiState, d: &Droid) {
    let Some(c) = sim.droid_class(d) else {
        return;
    };
    think_item(sim, tk, st, (d.pos, c.max_health, c.health_type as u32));
}

/// [`think`] for a seen auto-turret, as for a droid.
pub(super) fn think_turret(sim: &Sim, tk: &Tick, st: &mut AiState, t: &Turret) {
    let Some(c) = sim.turret_class(t) else {
        return;
    };
    let item = (t.centre(), c.body.max_health, c.health_type as u32);
    think_item(sim, tk, st, item);
}

fn think_item(sim: &Sim, tk: &Tick, st: &mut AiState, item: (Vec3, f32, u32)) {
    let Tick { me, u, eye, .. } = *tk;
    if u.held.is_empty() || throwing(sim, u).is_some() {
        return;
    }
    let forced = grenade_forced(
        sim.ground_rules,
        number(me),
        sim.tick,
        false,
        tk.skill.grenade_chance,
    );
    let cands = slots(sim, u);
    let pick = choose(
        cands.into_iter().flatten(),
        &item_ask(sim, me, eye, item, forced),
    );
    st.arm.pick = pick.map(|p| p.0);
    st.arm.in_range = pick.is_some_and(|p| p.1);
    st.arm.vehicle = false;
    st.arm.spare = pick.is_none();
    st.arm.claim = st.arm.claim.filter(|c| sim.tick < c.1);
}

/// What the choice means for the pad this tick.
#[derive(Clone, Copy)]
pub(super) enum Armed<'a> {
    /// The weapon in the hands: the gunner's own rules.
    Hand,
    /// The weapon in the hands launches a SHELL on an arc (the mortar, a
    /// `grenade` class firing `_ord1`): aim it on the PS2's low-arc solution
    /// as a grenade is aimed ([`lob_aim`]), and fire it by the hand's own
    /// gate (issue #29, `analysis/re-ordnance-kinds.md` §6).
    Lobbed(&'a WeaponClass),
    /// The pick is not current yet: this switch button, on a fresh press,
    /// and no trigger (the PS2's switch is at once, by slot; the port's bot
    /// steps its channel as a human does).
    Draw(u32),
    /// Channel 1's grenade is the firing weapon: aim the throw and, when it
    /// is the think's pick (`true`; `false` = only the throw under way), tap
    /// SECONDARY_FIRE.
    Thrown(&'a WeaponClass, bool),
    /// A chosen hero power uses the same offhand channel as human input.
    Secondary(&'a WeaponClass, &'a crate::weapon::WeaponState),
}

/// The weapon in the hands: a shell-firing class is aimed on its arc.
fn in_hand<'a>(sim: &'a Sim, u: &Unit) -> Armed<'a> {
    let class = &sim.weapon_classes[u.weapon_class];
    match class.ordnance.kind {
        OrdKind::Shell => Armed::Lobbed(class),
        _ => Armed::Hand,
    }
}

pub(super) fn reach(class: &WeaponClass) -> f32 {
    let physical = class.force.as_ref().map(|p| p.range).or_else(|| {
        class
            .saber_throw
            .as_ref()
            .map(|p| p.velocity * p.outbound_seconds)
    });
    physical.map_or(class.ai_range[2], |r| r.min(class.ai_range[2]))
}

pub(super) fn armed<'a>(sim: &'a Sim, u: &'a Unit, st: &AiState) -> Armed<'a> {
    // A throw under way keeps the grenade's aim until its fire state ends.
    if let Some(c) = throwing(sim, u) {
        return Armed::Thrown(c, false);
    }
    let Some(k) = st
        .arm
        .pick
        .filter(|s| *s != u.weapon_slot)
        .and_then(|s| u.held.iter().position(|w| w.slot == s))
    else {
        return in_hand(sim, u);
    };
    let class = &sim.weapon_classes[u.held[k].class];
    match u.held[k].channel {
        0 => Armed::Draw(buttons::SWITCH_WEAPON),
        _ if u.cur1 != Some(k as u8) => Armed::Draw(buttons::SWITCH_SECONDARY),
        _ if class.thrown.is_some() => Armed::Thrown(class, true),
        _ if class.force.is_some() || class.saber_throw.is_some() => {
            Armed::Secondary(class, &u.held[k].state)
        }
        _ => Armed::Hand,
    }
}

/// The PS2's ballistic lead (7.3; B1 `FUN_0026d3f0`): for a shell of speed
/// `s` under gravity `g` (< 0) and a target `d` from the launch point, how
/// far the aim point is raised and the time of flight, on the LOW arc —
/// A = s^2 + g d.y; A -= sqrt(A^2 - g^2 |d|^2); t^2 = 2A / g^2; raise =
/// -g t^2 / 2. `false` with no solution (the target is past the range): the
/// un-reduced A is used, as on the PS2.
pub(super) fn lob(s: f32, g: f32, d: Vec3) -> (f32, f32, bool) {
    if g.abs() < 1e-6 {
        return (0.0, d.length() / s.max(1e-3), true);
    }
    let mut a = s * s + g * d.y;
    let disc = a * a - g * g * d.length_squared();
    let ok = disc > 0.0;
    if ok {
        a -= disc.sqrt();
    }
    let t2 = (2.0 * a / (g * g)).max(0.0);
    (-0.5 * g * t2, t2.sqrt(), ok)
}

/// The point a bot looks at to lob `gc` at `target` (moving at `vel`), seen
/// from its `eye`, and whether the round reaches. The arc starts at the hand
/// a grenade leaves ([`crate::weapon::Thrown::hand`], with the soldier turned
/// at the target) or, for a SHELL (the mortar), at the muzzle `eye`; a bot
/// gets no `ShotElevate`, so the whole elevation is in this aim. The fall is
/// the thrown class's own `Gravity`, or a shell's `Gravity` x -9.8
/// (`analysis/re-ordnance-kinds.md` §6).
///
/// OURS: no throw into a wall — the arc's first nine tenths, as four chords,
/// have to be clear of the world (a bot at a door frame threw at its own
/// feet) — none at a target that will be inside `MinRange` when the
/// grenade lands, and none where a friend is or walks to ([`friend_by`]).
pub(super) fn lob_aim(
    sim: &Sim,
    u: &Unit,
    eye: Vec3,
    target: Vec3,
    vel: Vec3,
    gc: &WeaponClass,
) -> (Vec3, bool) {
    let origin = match gc.ordnance.kind {
        // A shell leaves the weapon's muzzle: the bot's fire point (§3.3's
        // "the aimer's fire point"; card FPNT), not a throw's hand.
        OrdKind::Shell => eye,
        _ => match gc.thrown.and_then(|t| t.hand) {
            Some((up, fwd)) => {
                let flat = Vec3::new(target.x - u.soldier.pos.x, 0.0, target.z - u.soldier.pos.z);
                u.soldier.pos + Vec3::Y * up + flat.normalize_or_zero() * fwd
            }
            None => eye,
        },
    };
    let d = target - origin;
    let g = match gc.ordnance.kind {
        OrdKind::Shell => -gc.ordnance.gravity * crate::weapon::SHELL_G,
        _ => gc.ordnance.sticky.as_ref().map_or(0.0, |s| s.gravity),
    };
    let (raise, t, ok) = lob(gc.ordnance.velocity, g, d);
    let lead = d + vel * t;
    let dir = (lead + Vec3::Y * raise).normalize_or_zero();
    let v0 = dir * gc.ordnance.velocity;
    let at = |k: f32| {
        let s = 0.9 * t * k / 4.0;
        origin + v0 * s + Vec3::Y * (0.5 * g * s * s)
    };
    let ok = ok
        && Vec3::new(lead.x, 0.0, lead.z).length() > gc.ai_range[0]
        && (0..4).all(|k| crate::perception::los(sim, at(k as f32), at(k as f32 + 1.0)))
        && !friend_by(sim, u, origin + lead, t + FRIEND_LEAD);
    (eye + dir * d.length().max(1.0), ok)
}

#[cfg(test)]
pub(crate) mod tests;
