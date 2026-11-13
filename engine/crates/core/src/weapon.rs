//! Blaster weapons: fire cycle, spread, ordnance damage — transcribed from the
//! decomp (spec: analysis/re-blaster-damage.md, lead-verified 2026-08-30 by
//! reading the bodies cited below). Everything here is data-driven from the
//! weapon (`wpnc`) and ordnance (`ordc`) classes; the BF1/BF2 rifle
//! constructors are byte-verified ODF values kept as test fixtures.
//!
//! Sources per item (BF1 SWBFRONT.ELF / BF2 SWBFRNT2.ELF):
//! - weapon state machine: BF1 weapon update (fn @ decomp line 208432, state
//!   at weapon+0xcc): idle + fire-input predicate → vtbl+0x70 start event +
//!   vtbl+0x74 trigger init (`FUN_00288c58`), firing state runs the cycle,
//!   `FUN_002891c0` = "cycle finished, back to idle" query;
//! - trigger init `FUN_00288c58` / BF2 `FUN_0032e758`: charge = accum /
//!   MaxPressedTime (1.0 if none), accum = 0, refire = ShotDelay (BF2:
//!   uniform(min,max)), salvoCounter = SalvoCount, salvoTimer =
//!   InitialSalvoDelay — the FIRST salvo comes from the trigger, not from
//!   the refire accumulator;
//! - fire cycle `FUN_00288d10` / BF2 `FUN_0032e8c0`: dt-chunk loop (eps 2^-15),
//!   refire branch (remainder-carrying accumulator; refuses TriggerSingle;
//!   gates = fire predicate, !reloading, !blocked), salvo branch (shots,
//!   ammo/heat, spread growth once per salvo, back-to-back salvos while the
//!   salvo timer stays ≤ 0, BF2 salvo delay uniform(min,max)); a salvo the
//!   refire branch starts with no dt left fires at the next update — even
//!   after trigger release: the only salvo cancel is weapon+0xc4 == 0, the
//!   weapon's ACTIVE (drawn) flag (set by select `FUN_00284148`, cleared by
//!   deselect `FUN_002841f0`), not the fire button;
//! - fire-input predicate `FUN_00289150`: overheated → false; charge weapon
//!   → fire on release (cmd == −1); automatic → held (cmd > 0); TriggerSingle
//!   → fresh press only (cmd == 1);
//! - heat (BF1 weapon update): overheated when heat ≥ 1.0, `heat -= dt ×
//!   HeatRecoverRate`, clears when heat < HeatThreshold;
//! - spread: ctor `FUN_00288858` (human init −SpreadThreshold, AI 0), per-tick
//!   recovery `FUN_00288930` / BF2 `FUN_0032e200`, getters `FUN_002889e8` /
//!   `FUN_00288c20` (max(s,0) + postureBase + axis spread), posture base
//!   `FUN_002889b0` / BF2 `0x32e438`;
//!   OURS (card SPRD): the floor sits [`WeaponClass::tight_hold`] lower, so
//!   held fire stays tight [`TIGHT_HOLD_SECS`] longer than the PS2's;
//! - shot direction `FUN_002892a8` / BF2 `FUN_0032f208`: triangular pitch then
//!   yaw offsets (4 LCG steps each), local dir (sinY·cosP, −sinP, cosY·cosP);
//! - ODF parsers: BF1 `FUN_002864f0`/`FUN_00289b48` (weapon), `FUN_002738a8`+
//!   `FUN_002774b0` (ordnance); BF2 `FUN_00328fb0`/`FUN_00330810`,
//!   `FUN_00310620`+`FUN_00314b98`;
//! - damage descriptor: BF1 `FUN_00272f88` (flat) / BF2 `FUN_00310178`
//!   (bolt-AGE falloff + team bonus); application: BF1 `FUN_001bc190` /
//!   BF2 `FUN_001dfb38` (HealthType scale, friendly-fire scale, shield phase;
//!   NO locational damage exists in either game).
//!
//! NEGATIVE RESULT carried into the design: the soldier `AimFactor*` /
//! `AimValue` ODF keys are consumed nowhere in either ELF — accuracy is
//! weapon-side only. Open (report §10): reload state machine (provisional
//! auto-reload below), charge-weapon accumulator semantics, BF2 wind-up
//! timers (rifles: 0), magnet-target velocity adjust at spawn.

mod class;

use crate::fmath;
use crate::lcg::GameLcg;
use glam::Vec3;

/// Degrees→radians literal the ODF parsers multiply by (EE float).
pub const DEG: f32 = 0.017_453_294;
pub const TARGET_LOCK_MAX_DEFAULT: f32 = 100.0;
pub const TARGET_LOCK_LOSE_DEFAULT: f32 = 110.0;
/// Fire-cycle loop epsilon, 2^-15 (both games).
pub const FIRE_EPS: f32 = 3.051_757_8e-5;

/// How far into a fire state a press clicks: `1.0 < weapon+0xd0` (B1, 208526)
/// / `+0xbc` (B2, 318218), the refire accumulator a shot's cycle starts at 0.
const FIRE_STATE_CLICK_SECS: f32 = 1.0;

/// Key/value access to a class's PROP list (game layer implements it over
/// the baked ClassDb with BASE-chain inheritance). Uncracked BF2 keys are
/// looked up by hash.
pub trait PropSource {
    fn prop(&self, key: &str) -> Option<&str>;
    fn prop_hash(&self, hash: u32) -> Option<&str>;
    fn prop_hash_all(&self, hash: u32) -> Vec<&str> {
        self.prop_hash(hash).into_iter().collect()
    }
}

pub(crate) fn pf(src: &dyn PropSource, key: &str) -> Option<f32> {
    src.prop(key)?.split_whitespace().next()?.parse().ok()
}

pub(crate) fn pf_hash(src: &dyn PropSource, hash: u32) -> Option<f32> {
    src.prop_hash(hash)?.trim().parse().ok()
}

pub(crate) fn pair(src: &dyn PropSource, key: &str) -> Option<[f32; 2]> {
    let s = src.prop(key)?;
    let mut it = s.split_whitespace().filter_map(|t| t.parse::<f32>().ok());
    let a = it.next()?;
    let b = it.next().unwrap_or(a);
    Some([a, b])
}

pub fn floats<const N: usize>(v: &str) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    let mut it = v.split_whitespace();
    for slot in &mut out {
        *slot = it.next()?.parse().ok()?;
    }
    Some(out)
}

mod ability;
mod beam;
mod dispenser;
mod emitter;
mod fatray;
mod ordnance;
mod repair;
#[cfg(test)]
pub(crate) mod test_props;
pub use ability::Ability;
pub use beam::BeamClass;
pub use dispenser::Dispenser;
pub use emitter::{BurnClass, EmitterOrdnance, BURN_SLOTS};
pub use fatray::FatRayClass;
pub use ordnance::{
    blast_falloff, DamageDesc, ExplosionClass, Falloff, Health, HealthType, OrdBlend, OrdKind,
    OrdKnot, OrdnanceClass, EXPLOSION_OFFSET, HASH_FALLOFF_DURATION, HASH_FALLOFF_END_DAMAGE,
    HASH_FALLOFF_START, HASH_TEAM_BONUS_SCALE, SHELL_G,
};
pub use repair::Repair;

// ── Weapon class ──────────────────────────────────────────────────────────

/// OURS (card SPRD, the maintainer 2026-10-04: "have it stay tight a little while
/// longer"): held fire keeps its tight spread this many seconds longer than
/// the PS2's before the bloom starts, for the player and the bots alike. The
/// PS2's rule is 0.0 (the ODF values are untouched).
pub const TIGHT_HOLD_SECS: f32 = 1.5;
/// The same for a bot's weapon (its bursts are shorter than this, so a
/// burst no longer blooms at all); 0.0 = the PS2's.
pub const TIGHT_HOLD_SECS_AI: f32 = 1.5;
/// The hold's buffer is never more than this, nor than the class's own
/// SpreadLimit: a class with a near-zero ShotDelay (a launcher its reload
/// paces, BF1's commando pistol, the AT-AT's chin gun) would otherwise bank
/// a buffer no fire could use up.
pub const TIGHT_HOLD_CAP: f32 = 6.0 * DEG;

/// The zoom-spread slot keys, the third column of `posture_spread`:
/// `StandAimSpread`, `CrouchAimSpread`, `ProneAimSpread` (`re-zoom.md` §2;
/// no disc class authors one).
pub const HASH_ZOOM_SPREAD: [u32; 3] = [0x5BC0_137F, 0x67BF_C1D7, 0x7D9D_A8BD];

/// One charge stage of a [`WeaponClass`]: from `threshold` (the charge it
/// must exceed) the shot is `class`'s; `index` is its own entry in
/// `Sim::weapon_classes` (`usize::MAX` until registered), which its rounds
/// carry.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeStage {
    pub threshold: f32,
    pub class: Box<WeaponClass>,
    pub index: usize,
}

/// A weapon's secondary ordnance (card UORD, `analysis/re-ordnance-kinds.md`
/// §7): BF2's `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod` (key hashes
/// 0xdddb253a / 0x82e86218, B2:324171-324181), a round spawned BESIDE the
/// primary shot. On the discs the class it names is the TRACER (a 0-damage
/// bolt: `com_weap_inf_shotgun_tracer`, `com_weap_inf_chaingun_tracer`,
/// `com_weap_award_shotgun_tracer`, `com_weap_award_pistol_tracer`), and the
/// port spawns it as [`crate::projectile::Bolt`] of its own class every
/// `period`-th round. The other class a `SecondaryOrdnanceName` names is a
/// real round, BF2's Imperial officer's sonic blaster fat ray, which spawns
/// as its own round beside the primary (`Sim::fire_secondary`).
#[derive(Clone, Debug, PartialEq)]
pub struct Secondary {
    /// The ordnance class's name as the db spells it, for reports and tests.
    pub name: String,
    /// Its ordnance data: velocity, LifeSpan, MaxDamage 0 and the laser look.
    pub ordnance: OrdnanceClass,
    /// `SecondaryOrdnancePeriod`: rounds per tracer; 1 = every round.
    pub period: u32,
    /// Its entry in `Sim::weapon_classes` (`usize::MAX` until
    /// [`crate::sim::Sim::add_weapon_class`] registers it) - the class index
    /// its bolts carry for their damage and their look.
    pub index: usize,
}

impl Secondary {
    /// Does round `n` (1-based: the weapon's rounds fired since it was made)
    /// carry the secondary?
    pub fn on_round(&self, n: u32) -> bool {
        self.period > 0 && n >= 1 && (n - 1).is_multiple_of(self.period)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponClass {
    pub name: String,
    /// 0 = infinite (RoundsPerClip < 1).
    pub rounds_per_clip: u32,
    pub ammo_per_shot: u32,
    pub reload_time: f32,
    pub trigger_single: bool,
    /// BF2 HUD firing-cycle progress. FUN_00327e98 B2:319571 defaults
    /// DisplayRefire on; FUN_00328fb0:320451-320456 loads the override.
    pub display_refire: bool,
    /// `ZoomMax`, the scope's maximum magnification (BF2 class+0xb4 / BF1
    /// +0x7c). Used by human zoom and as the third gate of
    /// [`WeaponClass::ai_distance_table`]
    /// (`analysis/re-ai-sniper.md` §2). Class-ctor default 1.0 in both games
    /// (B2:319499 / B1:209461), and the loaders coerce a written 0 to 1.0.
    pub zoom_max: f32,
    /// `ZoomMin`, the first step of a human's zoom (`analysis/re-zoom.md`
    /// §2; default 1.0, a written 0 reads 1.0).
    pub zoom_min: f32,
    /// `ZoomRate`: non-zero makes the zoom a continuous one on the stick
    /// (turret and vehicle weapons only on the discs; the port has no
    /// continuous zoom yet, and a class with a rate takes no second step).
    pub zoom_rate: f32,
    /// `ZoomTurnDivisorMin` / `Max`: what the turn rates are divided by at
    /// each step. `ZoomMin` / `ZoomMax` are copied here when read (§2); no
    /// disc class authors the divisor keys.
    pub zoom_turn_div: [f32; 2],
    /// `ZoomFirstPerson`: a soldier's zoom on this weapon is the scope view.
    pub zoom_first_person: bool,
    /// `AIBubbleSizeMultiplier` (BF2 class +0x1e8, card CLSE2,
    /// `analysis/re-soldier-ai.md` 18.4): the factor on the error vector of
    /// a bot that fires this weapon - 1.25 the shotgun, 0.6 the sniper
    /// rifles. BF1 reads none of the three keys.
    pub ai_bubble_size: f32,
    /// `AIBubbleScaleDistDivider` (+0x1ec): the error vector grows with the
    /// range d, x (d / this + 0.5).
    pub ai_bubble_dist: f32,
    /// `AIBubbleScaleClamp` (+0x1f0): the most that range factor is.
    pub ai_bubble_clamp: f32,
    /// > 0 = charge weapon (fires on release).
    pub max_pressed_time: f32,
    /// `LockTime` (card UORD §5; B1 launcher `FUN_00290758` B1:217008 writes
    /// W.class+0x250, B2 `FUN_0033acc8` B2:332061): seconds a launcher must
    /// dwell on a target before a missile it fires homes. 0 = never. Read
    /// only on a `launcher` class - the PS2 parses the key in the launcher
    /// setter alone, so a cannon's `LockTime` (the wrist rockets') does
    /// nothing.
    pub lock_time: f32,
    pub lock_on_range: f32,
    pub target_lock_max: f32,
    pub target_lock_lose: f32,
    /// The acquisition cone's slope, radians (C+0xd8: `LockOnAngle` × DEG, or
    /// `AutoAimSize` × 0.072; `analysis/re-aim-assist.md` §3.2). 0 = a cone
    /// only as wide as the target's radius.
    pub lock_on_angle: f32,
    /// The acquisition's reach (C+0xd0, `re-aim-assist.md` §3.2): the query
    /// box's half-side and the cone's length. See [`WeaponClass::from_props`]
    /// for the order-free rule the port uses.
    pub extreme_range: f32,
    /// The AI's range triple, m (card FGHT, `analysis/re-soldier-ai.md` 4.2,
    /// 4.5): `MinRange`, `OptimalRange`, `MaxRange`. A bot fights a target
    /// inside `max + 10`, stands to shoot inside the middle of the band and
    /// circles on a ring about `optimal`. The class defaults 0 / 30 / 60 are
    /// the note's ("defaults 30 / - / 60"); every shipped infantry weapon
    /// writes all three. Only the AI reads it.
    pub ai_range: [f32; 3],
    /// `BarrageMin` / `BarrageMax` (card CLSE4, `analysis/re-soldier-ai.md`
    /// 20.3; BF2 class +0x214 / +0x218, BF1 +0x1bc / +0x1c0): the bounds of
    /// a bot's burst with this weapon. The constructor's 3 / 6 in both
    /// games; on the discs only BF2's flyer weapons and its defence turret
    /// write them. Only the AI reads the three.
    pub barrage: [u8; 2],
    /// `BarrageDelay` (+0x21c / +0x1c4), s: the pause after a burst, before
    /// BF2's skill factor. The constructor's 2.0.
    pub barrage_delay: f32,
    /// Relations the magnet may take (C+0xf4): bit `rel + 1` for rel −1
    /// enemy / 0 neutral / 1 friend — TargetEnemy 1, TargetNeutral 2,
    /// TargetFriendly 4. Class-ctor default 1 (B1:209497).
    pub target_team: u32,
    /// Kinds the magnet may take (C+0xf8): bit `kind` — TargetPerson 1,
    /// Animal 2, Droid 4, Vehicle 8, Building 0x10. The ctor default
    /// 0xffffff9f has every kind the port has on.
    pub target_kind: u32,
    /// Kinds the AI may use this weapon on (card GRAI, BF2 class+0x124,
    /// `analysis/re-soldier-ai.md` 4.3): bit `kind`, as `target_kind`. The
    /// `AITarget*` keys (BF2 only on the discs); a class that writes none
    /// keeps its `Target*` bits — PROVISIONAL: the PS2's constructor sets
    /// both words to 0xffffff9f (B2:307279-307280), so its AI word does not
    /// follow `Target*` (`analysis/re-portable-turret.md` §6.1).
    pub ai_target_kind: u32,
    /// Heat units per shot; 0 = never overheats.
    pub heat_per_shot: f32,
    pub heat_recover_rate: f32,
    pub heat_threshold: f32,
    /// [min, max] seconds; BF1 single value → both equal.
    pub shot_delay: [f32; 2],
    pub salvo_count: u32,
    pub shots_per_salvo: u32,
    pub salvo_delay: [f32; 2],
    pub initial_salvo_delay: f32,
    /// Radians.
    pub pitch_spread: f32,
    pub yaw_spread: f32,
    pub shot_tilts: Vec<(f32, f32)>,
    /// The charge stages after the root (issue #29, `analysis/re-weapon-charge.md`
    /// §1-§3: B1 `FUN_002864f0` / `FUN_00285108`, B2 `FUN_00328fb0` /
    /// `FUN_00327168`): each `NextCharge` clones the class as it stands, and
    /// the keys after it go to the clone. While the trigger is held the
    /// weapon steps to the next stage while pressed / MaxPressedTime is over
    /// that stage's threshold; the shot is built from the stage reached (its
    /// ordnance, ShotsPerSalvo, offsets, HeatPerShot). Empty: no stages.
    pub stages: Vec<ChargeStage>,
    /// [stand, crouch, prone] × [still, move, zoom], radians.
    pub posture_spread: [[f32; 3]; 3],
    pub spread_per_shot: f32,
    pub spread_recover_rate: f32,
    pub spread_threshold: f32,
    pub spread_limit: f32,
    pub ordnance: OrdnanceClass,
    /// BF2's `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod`: the TRACER
    /// round fired beside the primary ([`Secondary`]). `None` on every class
    /// that authors neither key (every BF1 weapon).
    pub secondary: Option<Secondary>,
    /// Sound property names (FireSound / ReloadSound / FireEmptySound /
    /// WeaponChangeSound).
    pub fire_sound: Option<String>,
    pub fire_loop_sound: Option<String>,
    pub muzzle_effect: Option<String>,
    pub reload_sound: Option<String>,
    pub empty_sound: Option<String>,
    pub change_sound: Option<String>,
    /// The muzzle flash's look (card FXMF, `analysis/re-muzzle-flash.md`):
    /// render-only, replayed from the class chain by the class loader.
    /// `None` = none drawn (a sabre, a thrown weapon, a test class).
    pub flash: Option<crate::flash::MuzzleFlashLook>,
    /// The `grenade` weapon class's own keys (card GREN); `None` for every
    /// other weapon.
    pub thrown: Option<Thrown>,
    pub offhand_weapon: bool,
    /// `ScoreForMedalsType` (BF2 wpn+0x1e0, `analysis/re-hitzones.md` §3):
    /// which award the weapon's hits count toward (2 = demolition, the
    /// rocket launchers; 4 = marksman, the sniper rifles). `None` when no
    /// class in the chain authors it (every BF1 weapon).
    pub medals_type: Option<i8>,
    /// `MedalsTypeToUnlock` (BF2 wpn+0x1e2, `analysis/re-grenades.md` §2.4):
    /// the medal an award weapon needs before it joins the switch cycle.
    /// `None` for an ordinary weapon (a written -1, or no key: every BF1 one).
    pub medal_unlock: Option<i8>,
    /// `MedalsTypeToLock` (BF2 wpn+0x1e4): the medal whose award replaces
    /// this weapon (`Sim::weapon_locked`).
    pub medal_lock: Option<i8>,
    pub repair: Option<Repair>,
    pub dispenser: Option<Dispenser>,
    pub mine: Option<Dispenser<crate::mine::MineClass>>,
    pub ability: Option<Ability>,
    pub force: Option<crate::hero_weapon::ForcePower>,
    pub saber_throw: Option<crate::hero_weapon::SaberThrow>,
    pub saber: Option<Box<crate::hero_weapon::SaberClass>>,
    /// An `emitterordnance` class's data (the arc casters, the incinerator,
    /// the flamethrowers; `analysis/re-ordnance-kinds.md` §8). `None` = the
    /// class is not an emitter.
    pub emitter: Option<EmitterOrdnance>,
    pub cloak: Option<crate::cloak::CloakClass>,
    pub guided: Option<Box<crate::guided::GuidedWeapon>>,
    pub remote: Option<Box<crate::remote::RemoteWeapon>>,
    pub turret: Option<Box<Dispenser<crate::autoturret::TurretClass>>>,
}

/// What the `grenade` weapon class adds (`analysis/re-grenades.md` §3.1; B1
/// `FUN_0028f1f0` / setter `FUN_0028f308`, B2 `FUN_00337fd0` /
/// `FUN_00338190`). Its cycle is the cannon's with one salvo: the press, the
/// `InitialSalvoDelay` wind-up, the throw, then `ShotDelay` — and an empty
/// clip reloads at once from the reserve (§3.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrown {
    /// Added to a human's aim.y before the throw (ctor default 0.3).
    pub shot_elevate: f32,
    /// The weapon model is not drawn while its fire state runs (default 1).
    pub hide_on_fire: bool,
    /// BF2 throws from the hand, not the fire point: (up, forward) metres in
    /// the soldier's frame. `None` selects the fire point. The unmeasured
    /// hand paths use this provisional tuple. trace: per-variant native
    /// release vectors and the fire-entry stance (§3.3).
    pub hand: Option<(f32, f32)>,
    /// The four native uncharged BF2 thermals share the measured strong
    /// release vector. BF1 compatibility throws use the ordinary hand tuple.
    pub native_thermal_hand: bool,
}

impl Thrown {
    /// BF2's strong-throw hand offset ([`Thrown::hand`]) from the port's
    /// feet: the PS2's 1.8183 m up is from a soldier position that rests
    /// 0.02 m INTO the floor (the kept penetration, re-soldier-ground §1;
    /// feet y -0.02 on terrain y 0 in all 15 throws of re-grenades §13.1),
    /// the port's feet stand on it. With 1.8183 the grenade flew 2 cm high
    /// and its first contact came an update late in 5 of the 7 throws (card
    /// GRNP: rest 0.05-0.2 m and 1-3 updates off; 14 of 15 exact with this).
    pub const BF2_HAND: (f32, f32) = (1.8183 - Self::BF2_HAND_FLOOR, 0.7433);

    const BF2_HAND_FLOOR: f32 = 0.02;

    /// Scale the stock class vector before applying the port's floor offset.
    /// The calibrated unity path preserves existing launch bits.
    pub(crate) fn release_hand(self, scale: f32) -> Option<(f32, f32)> {
        if self.native_thermal_hand && scale != 1.0 {
            let up = 1.818_271_8 * scale - Self::BF2_HAND_FLOOR;
            let forward = 0.743_278_86 * scale;
            if up.is_finite() && forward.is_finite() && up > 0.0 {
                return Some((up, forward));
            }
        }
        self.hand
    }

    /// The BF2 thermal detonator's `InitialSalvoDelay` and its ordnance's
    /// `LifeSpan` (re-grenades §9; BF1's are 0.3 and 0.9).
    pub const BF2_WIND_UP: f32 = 0.51;
    pub const BF2_FUSE: f32 = 0.6;

    /// Turn a BF1 thrown weapon into one thrown as BF2's thermal detonator,
    /// for a BF1 class fielded on a BF2 map (the pause menu's class picker;
    /// on a BF1 map every grenade throws by BF1's own numbers, the
    /// maintainer's rule of 2026-11-01): released from the hand after BF2's
    /// wind-up, at rest by BF2's slow-bounce count, BF2's fuse from the rest,
    /// sticking to what BF2's thermal sticks to (vehicles, buildings, unbuilt
    /// buildings) besides its own mask. Its speed, gravity and elevation are the discs' own
    /// (equal in both games); its damage, radius and ammo stay BF1's.
    pub fn throw_as_bf2(w: &mut WeaponClass) {
        use crate::sticky::stick;
        let Some(t) = w.thrown.as_mut() else {
            return;
        };
        t.hand = Some(Self::BF2_HAND);
        t.native_thermal_hand = false;
        w.initial_salvo_delay = Self::BF2_WIND_UP;
        w.ordnance.life_span = Self::BF2_FUSE;
        if let Some(s) = w.ordnance.sticky.as_mut() {
            s.bf2 = true;
            s.stick_mask |= stick::VEHICLE | stick::BUILDING | stick::BUILDING_UNBUILT;
        }
    }

    /// The grenade class over a base weapon parsed by
    /// [`WeaponClass::from_props`]: its constructor defaults where the class
    /// authors nothing (`TriggerSingle` 1, `ReloadTime` 0.6, `ShotDelay`
    /// 0.125, `InitialSalvoDelay` 0.3 / BF2 0.5), then its own keys.
    pub fn apply(w: &mut WeaponClass, src: &dyn PropSource, bf2: bool) {
        if pf(src, "TriggerSingle").is_none() {
            w.trigger_single = true;
        }
        if pf(src, "ReloadTime").is_none() {
            w.reload_time = 0.6;
        }
        if pair(src, "ShotDelay").is_none() {
            w.shot_delay = [0.125, 0.125];
        }
        if pf(src, "InitialSalvoDelay").is_none() {
            w.initial_salvo_delay = if bf2 { 0.5 } else { 0.3 };
        }
        w.thrown = Some(Thrown {
            shot_elevate: pf(src, "ShotElevate").unwrap_or(0.3),
            hide_on_fire: pf(src, "HideOnFire").is_none_or(|v| v != 0.0),
            hand: bf2.then_some(Self::BF2_HAND),
            native_thermal_hand: bf2
                && w.ordnance.sticky.is_some()
                && w.max_pressed_time <= 0.0
                && src.prop("ForceFireAnimation").is_none()
                && [
                    "all_weap_inf_thermaldetonator",
                    "cis_weap_inf_thermaldetonator",
                    "imp_weap_inf_thermaldetonator",
                    "rep_weap_inf_thermaldetonator",
                ]
                .iter()
                .any(|name| w.name.eq_ignore_ascii_case(name)),
        });
    }
}

impl Default for WeaponClass {
    /// WeaponCannonClass ctor defaults where traced (ShotsPerSalvo 1,
    /// SalvoCount 1); untraced fields use PROVISIONAL defaults (ours) — every
    /// shipped weapon sets them.
    fn default() -> Self {
        Self {
            name: String::new(),
            rounds_per_clip: 0,
            ammo_per_shot: 1,
            reload_time: 0.01,
            trigger_single: false,
            display_refire: true,
            zoom_max: 1.0,
            zoom_min: 1.0,
            zoom_rate: 0.0,
            zoom_turn_div: [1.0, 1.0],
            zoom_first_person: false,
            ai_bubble_size: 1.0,
            ai_bubble_dist: 20.0,
            ai_bubble_clamp: 10.0,
            max_pressed_time: 0.0,
            lock_time: 0.0,
            lock_on_range: 0.0,
            target_lock_max: TARGET_LOCK_MAX_DEFAULT,
            target_lock_lose: TARGET_LOCK_LOSE_DEFAULT,
            lock_on_angle: 0.0,
            extreme_range: 0.0,
            ai_range: [0.0, 30.0, 60.0],
            barrage: [3, 6],
            barrage_delay: 2.0,
            target_team: 1,
            target_kind: 0x1f,
            ai_target_kind: 0x1f,
            heat_per_shot: 0.0,
            heat_recover_rate: 0.0,
            heat_threshold: 0.0,
            shot_delay: [0.2, 0.2],
            salvo_count: 1,
            shots_per_salvo: 1,
            salvo_delay: [0.0, 0.0],
            initial_salvo_delay: 0.0,
            pitch_spread: 0.0,
            yaw_spread: 0.0,
            shot_tilts: Vec::new(),
            stages: Vec::new(),
            posture_spread: [[0.0; 3]; 3],
            spread_per_shot: 0.0,
            spread_recover_rate: 0.0,
            spread_threshold: 0.0,
            spread_limit: 0.0,
            ordnance: OrdnanceClass::default(),
            secondary: None,
            fire_sound: None,
            fire_loop_sound: None,
            muzzle_effect: None,
            reload_sound: None,
            empty_sound: None,
            change_sound: None,
            flash: None,
            thrown: None,
            offhand_weapon: false,
            medals_type: None,
            medal_unlock: None,
            medal_lock: None,
            repair: None,
            dispenser: None,
            mine: None,
            ability: None,
            force: None,
            saber_throw: None,
            saber: None,
            emitter: None,
            cloak: None,
            guided: None,
            remote: None,
            turret: None,
        }
    }
}

/// [`WeaponClass::give_sniper_assist`]'s reach for a sniper that authors no
/// `MaxRange` (none on the discs): the discs' sniper `MaxRange`.
pub const SNIPER_LOCK_ON_RANGE: f32 = 128.0;
/// [`WeaponClass::give_sniper_assist`]'s cone for a sniper that authors no
/// `LockOnAngle` (none on the discs): the discs' sniper `LockOnAngle`, 1°.
pub const SNIPER_LOCK_ON_ANGLE: f32 = DEG;
/// `TargetPerson | TargetAnimal | TargetDroid`.
pub(crate) const TARGET_SOLDIER_KINDS: u32 = 0b111;

/// The AI sniper distance table: `(d² upper edge, yaw half-angle in radians)`
/// (B2:323373-323398 / B1:211274-211299, byte-identical constants in both
/// games — `analysis/re-ai-sniper.md` §2). It is a STEP function on the
/// SQUARED distance with strict `<` and no interpolation; the edges are
/// 20/40/60/80/100/120 m. Keeping d² is the PS2's own form and costs no sqrt.
pub const AI_SNIPER_DISPERSION: [(f32, f32); 6] = [
    (400.0, 0.0997),
    (1600.0, 0.0499),
    (3600.0, 0.0333),
    (6400.0, 0.025),
    (10000.0, 0.02),
    (14400.0, 0.0167),
];

/// Past the last band (d ≥ 120 m).
pub const AI_SNIPER_DISPERSION_FAR: f32 = 0.0143;

/// The table value (radians) for a squared shooter-to-target distance.
pub fn ai_sniper_yaw(dist_sq: f32) -> f32 {
    for (edge, v) in AI_SNIPER_DISPERSION {
        if dist_sq < edge {
            return v;
        }
    }
    AI_SNIPER_DISPERSION_FAR
}

// ── Weapon instance state ─────────────────────────────────────────────────

/// A shot the cycle asked the world to spawn (direction is rolled by the
/// world with [`shot_direction`] so the LCG order stays pitch-then-yaw per
/// shot, as in FUN_002892a8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotSpec {
    pub tilt: (f32, f32),
    pub charge: f32,
    /// The charge stage that fired it ([`WeaponClass::stage`]).
    pub stage: u8,
    /// Half-angle magnitudes (radians) at spawn time.
    pub pitch_spread: f32,
    pub yaw_spread: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireState {
    Idle,
    Firing,
}

/// Per-tick inputs to [`WeaponState::update`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FireInput {
    /// Fire button held this tick.
    pub held: bool,
    /// Soldier-side veto (rolls, dead, in-menu): no shots.
    pub blocked: bool,
    /// The `ammo` cheat: the reserve never goes down; the clip still empties
    /// and its reload is free.
    pub infinite_ammo: bool,
    /// `[ours]` The `clip` cheat: the clip never empties; shots draw from
    /// the reserve, then the clip, and nothing reloads. Both cheats: nothing
    /// is drawn at all (an endless weapon's rule).
    pub no_reload: bool,
    /// The AI sniper distance table's yaw term for THIS tick, already scaled
    /// by the shooter's skill (`crate::ai::think` — brief D9). `None` for a
    /// human, for a bot with no target and for any class that fails
    /// [`WeaponClass::ai_distance_table`]; then the class `YawSpread` is used
    /// as it always was.
    pub ai_dispersion: Option<f32>,
}

/// [`WeaponState::spare`] of a slot whose `WeaponAmmo` is infinite.
pub const SPARE_INFINITE: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub struct WeaponState {
    pub state: FireState,
    /// Refire accumulator (+0xd0 / BF2 +0xbc) and period (+0xd4 / +0xc0).
    pub refire_accum: f32,
    pub refire_time: f32,
    pub salvo_counter: u32,
    pub salvo_timer: f32,
    pub charge: f32,
    /// Signed spread state, radians (+0x150 / +0x130).
    pub spread: f32,
    pub spread_mult: f32,
    pub posture_base: f32,
    /// Rounds in the clip (BF2 int model; BF1's clip fraction is equivalent).
    pub clip: u32,
    /// Spare rounds outside the clip.
    pub spare: u32,
    /// > 0 while reloading (provisional state machine, report §10.1).
    pub reload_timer: f32,
    /// The running reload is the `ammo` cheat's: it fills the clip without
    /// drawing on the reserve.
    pub free_reload: bool,
    /// Heat in [0, 1]; overheated at ≥ 1.0 until it drops below HeatThreshold.
    pub heat: f32,
    pub overheated: bool,
    /// Per-weapon fire command (soldier-written): >0 held counter, −1 on the
    /// release edge, 0 idle.
    pub fire_cmd: i32,
    /// This update began in [`FireState::Idle`]. Only [`Self::press_clicks`]
    /// reads it, after the update that wrote it, so it stays out of the hash.
    began_idle: bool,
    pub remote_press: u8,
    pub force_channel: Option<crate::hero_weapon::ForceChannel>,
    pub saber_flight: Option<crate::hero_weapon::SaberFlight>,
    pub melee: Option<crate::hero_weapon::MeleeState>,
    /// Human players get the −SpreadThreshold free-shot buffer; AI floor 0
    /// (both less the port's tight hold, `spread_floor`).
    pub human: bool,
    /// Last [`FireInput::ai_dispersion`] seen by [`WeaponState::update`] —
    /// the AI sniper table's contribution to [`WeaponState::yaw_spread`]. It
    /// lives on the instance because that is where the PS2 keeps it
    /// (`weapon+0x138` / `+0x158`, overwritten by the AI every aim tick and
    /// read by `GetYawSpread`; `analysis/re-ai-sniper.md` §2). NOT hashed:
    /// it is written from `AiState.dispersion`, which is, and rewritten
    /// before every use.
    pub ai_dispersion: Option<f32>,
    /// Weapon drawn (weapon+0xc4). Holstering cancels pending salvos and
    /// blocks triggering; held channel slots draw and holster their weapons.
    pub active: bool,
    pub shots_fired: u64,
    /// Rounds fired since the weapon was made, 1-based (the first round is 1),
    /// counted only on a class that carries a [`Secondary`]. Its multiple of
    /// `Secondary::period` is the round that spawns the tracer
    /// ([`Secondary::on_round`]). Hashed only when non-zero, so every weapon
    /// without a secondary keeps the old feed.
    pub secondary_rounds: u32,
    pub tilt_index: u32,
    /// The charge stage this trigger fires ([`WeaponClass::stage_for`]).
    pub stage: u8,
    /// The rounds this weapon was issued (the clip and the reserve at its
    /// making; [`SPARE_INFINITE`] = endless). Never written again: the PS2's
    /// cap of the reserve (B1 weapon+0x94, B2 ammo[2]), which the AI's
    /// "plenty of ammunition" asks (`re-grenades.md` 7.5). NOT hashed: a
    /// constant of the unit's class and slot.
    pub issued: u32,
    /// The dwell lock's candidate (card UORD §5, the PS2's W+0x128 / +0x170)
    /// and the seconds it has been held (an on-foot lock past 100 m counts
    /// them slower, `sim::foot_lock::foot_dwell_scale`). A launcher's
    /// missile homes only when [`WeaponState::locked`] says so at the press.
    pub lock_target: Option<crate::missile::MissileTarget>,
    pub lock_held: f32,
}

impl WeaponState {
    pub fn count_secondary_round(&mut self) -> u32 {
        self.secondary_rounds = self.secondary_rounds.wrapping_add(1);
        self.secondary_rounds
    }

    /// One loaded clip plus `reserve_clips` spare clips. Zero means no reserve;
    /// use [`Self::with_clips`] for authored totals and an unlimited reserve.
    pub fn new(class: &WeaponClass, human: bool, reserve_clips: u32) -> Self {
        Self {
            state: FireState::Idle,
            refire_accum: 0.0,
            refire_time: class.shot_delay[0],
            salvo_counter: 0,
            salvo_timer: 0.0,
            charge: 0.0,
            stage: 0,
            spread: Self::spread_floor(class, human, 1.0),
            spread_mult: 1.0,
            posture_base: 0.0,
            clip: class.rounds_per_clip,
            spare: reserve_clips.saturating_mul(class.rounds_per_clip),
            reload_timer: 0.0,
            free_reload: false,
            heat: 0.0,
            overheated: false,
            fire_cmd: 0,
            began_idle: true,
            remote_press: 0,
            force_channel: None,
            saber_flight: None,
            melee: None,
            human,
            ai_dispersion: None,
            active: true,
            shots_fired: 0,
            secondary_rounds: 0,
            tilt_index: 0,
            issued: class
                .rounds_per_clip
                .saturating_add(reserve_clips.saturating_mul(class.rounds_per_clip)),
            lock_target: None,
            lock_held: 0.0,
        }
    }

    /// Is the launcher's dwell lock up (card UORD §5)? The class must be a
    /// launcher (`LockTime > 0`), the candidate alive, and the clock at the
    /// limit.
    pub fn locked(&self, c: &WeaponClass) -> bool {
        c.lock_time > 0.0 && self.lock_target.is_some() && self.lock_held >= c.lock_time
    }

    /// The PS2's ammo rule for a class slot (`re-grenades.md` §2.5, B1
    /// `FUN_002838d8`): `WeaponAmmo` clips = one loaded + (N − 1) in reserve;
    /// 0 (a written value < 1) = an endless reserve ([`SPARE_INFINITE`]).
    pub fn with_clips(class: &WeaponClass, human: bool, clips: u32) -> Self {
        let mut w = Self::new(class, human, clips.saturating_sub(1));
        if clips == 0 {
            w.spare = SPARE_INFINITE;
            w.issued = SPARE_INFINITE;
        }
        w
    }

    /// The rounds this weapon was issued; `None` = endless.
    pub fn issued(&self, c: &WeaponClass) -> Option<u32> {
        (!c.infinite_ammo() && self.issued != SPARE_INFINITE).then_some(self.issued)
    }

    /// Rounds the HUD counts: the clip plus the reserve (§2.5); `None` when
    /// either is endless.
    pub fn rounds(&self, c: &WeaponClass) -> Option<u32> {
        (!c.infinite_ammo() && self.spare != SPARE_INFINITE).then(|| self.clip + self.spare)
    }

    /// Nothing left to fire or load (the PS2's weapon state 8).
    pub fn out_of_ammo(&self, c: &WeaponClass) -> bool {
        !c.infinite_ammo() && self.clip < c.ammo_per_shot && self.spare == 0
    }

    /// Soldier-side fire command byte: held counter / release edge / idle.
    fn set_fire_input(&mut self, held: bool) {
        self.fire_cmd = if held {
            self.fire_cmd.max(0).saturating_add(1)
        } else if self.fire_cmd > 0 {
            -1
        } else {
            0
        };
    }

    /// The stage the trigger reached (0 the root; [`WeaponClass::stages`]).
    pub fn stage(&self) -> usize {
        usize::from(self.stage)
    }

    /// Whether this update's trigger press is one the weapon refuses, the
    /// moment its `FireEmptySound` starts for a human. Asked after
    /// [`Self::update`], with the shots it made: the PS2 runs the weapon's
    /// state machine with the heat already updated, so the update a weapon
    /// cools or finishes its reload on answers the press with a shot and no
    /// click (B1 `FUN_00284318` 208434-208446, 208523-208526, 208575-208583,
    /// 208598-208604; B2 `FUN_00326170` 318216-318222, 318285-318294;
    /// `analysis/re-weapon-sounds.md` 7.3).
    ///
    /// The press edge is the only rate limit: holding gives one click, and
    /// neither disc's click sound authors a `PlayInterval`. A weapon that has
    /// just fired stays in its fire state until the cycle ends, so the update
    /// after the overheating shot does not click: only the state the cycle
    /// ends in does, or a fire state more than [`FIRE_STATE_CLICK_SECS`] in.
    ///
    /// Two port states have no PS2 fire state behind them. A weapon with
    /// nothing left enters [`FireState::Firing`] here on a press with no shot,
    /// where the PS2's waits in its no-ammunition state and clicks. A reload
    /// started by its button runs inside the cycle here, and clicks as a
    /// reload.
    /// PROVISIONAL [D], measure: a PS2 log of a human's press between the
    /// reload button and the end of the shot's cycle.
    pub fn press_clicks(&self, c: &WeaponClass, input: FireInput, fired: bool) -> bool {
        if self.fire_cmd != 1 || !self.active || input.blocked || fired {
            return false;
        }
        let cannot_pay = !self.can_pay(c, input.infinite_ammo, input.no_reload);
        match self.state {
            FireState::Firing if self.began_idle => cannot_pay,
            FireState::Firing => self.reloading() || self.refire_accum > FIRE_STATE_CLICK_SECS,
            FireState::Idle => self.overheated || self.reloading() || cannot_pay,
        }
    }

    /// FUN_00289150.
    fn fire_predicate(&self, c: &WeaponClass) -> bool {
        if self.overheated {
            return false;
        }
        if c.max_pressed_time > 0.0 {
            self.fire_cmd == -1
        } else if !c.trigger_single {
            self.fire_cmd > 0
        } else {
            self.fire_cmd == 1
        }
    }

    pub fn reloading(&self) -> bool {
        self.reload_timer > 0.0
    }

    /// A charge weapon's charge while the trigger is held, 0..1; `None` when it
    /// is not charging (BF2 weapon state 3, `FUN_0032ee30`; the HUD reads it).
    pub fn charging(&self, c: &WeaponClass) -> Option<f32> {
        let holding = c.max_pressed_time > 0.0
            && self.state == FireState::Idle
            && self.active
            && !self.reloading()
            && self.fire_cmd > 0;
        holding.then(|| (self.refire_accum / c.max_pressed_time).clamp(0.0, 1.0))
    }

    /// Effective half-angles at fire time (vtbl+0x44 / +0x40).
    pub fn pitch_spread(&self, c: &WeaponClass) -> f32 {
        self.spread.max(0.0) + self.posture_base + c.pitch_spread
    }
    /// `max(spreadState, 0) + postureBase + <the YawSpread slot>`
    /// (B2:323416 / B1:211316). The AI sniper table occupies that last slot
    /// when it applies — it REPLACES the class `YawSpread`, it does not
    /// replace `max(s, 0)`, which every branch of the PS2 getter adds
    /// (`analysis/re-ai-sniper.md` §2; the same document's §4.2 corrects
    /// `re-blaster-damage.md` §4.3, which said otherwise).
    pub fn yaw_spread(&self, c: &WeaponClass) -> f32 {
        self.spread.max(0.0) + self.posture_base + self.ai_dispersion.unwrap_or(c.yaw_spread)
    }

    /// SetPostureSpread (BF1 FUN_002889b0 / BF2 0x32e438), called by the
    /// soldier every tick: `stance` 0 stand / 1 crouch / 2 prone (>2 → 0),
    /// `move_mag` = max(|fwd|, |strafe|).
    pub fn set_posture(&mut self, c: &WeaponClass, stance: usize, move_mag: f32, zoomed: bool) {
        let p = if stance > 2 { 0 } else { stance };
        let base = if zoomed {
            c.posture_spread[p][2]
        } else {
            c.posture_spread[p][0]
        };
        self.posture_base = base + (c.posture_spread[p][1] - base) * move_mag;
    }

    /// Start a reload if one is possible (RELOAD button or empty clip).
    pub fn start_reload(&mut self, c: &WeaponClass) -> bool {
        self.start_reload_as(c, false)
    }

    /// Would [`Self::start_reload_as`] start a reload now? Not while one
    /// runs, not on a full clip, not with nothing left to load.
    pub fn can_start_reload(&self, c: &WeaponClass, free: bool) -> bool {
        !(c.infinite_ammo()
            || self.reloading()
            || self.clip >= c.rounds_per_clip
            || (!free && self.spare == 0))
    }

    /// [`Self::start_reload`]; `free` (the `ammo` cheat) fills the clip at
    /// the end without touching the reserve, even an empty one.
    pub fn start_reload_as(&mut self, c: &WeaponClass, free: bool) -> bool {
        if !self.can_start_reload(c, free) {
            return false;
        }
        if !free && self.spare != SPARE_INFINITE {
            self.spare = self.spare.saturating_add(self.clip);
            self.clip = 0;
        }
        self.free_reload = free;
        self.reload_timer = c.reload_time;
        true
    }

    /// Can a shot be paid for under the cheats? (`infinite_ammo`, `no_reload`
    /// as in [`FireInput`].)
    fn can_pay(&self, c: &WeaponClass, infinite_ammo: bool, no_reload: bool) -> bool {
        if c.infinite_ammo() || (infinite_ammo && no_reload) {
            return true;
        }
        if no_reload {
            return self.clip.saturating_add(self.spare) >= c.ammo_per_shot;
        }
        self.clip >= c.ammo_per_shot
    }

    /// Pay for one shot: from the clip, or with `no_reload` from the reserve
    /// first, so the clip stays full until the reserve is gone.
    fn pay(&mut self, c: &WeaponClass, infinite_ammo: bool, no_reload: bool) {
        if c.infinite_ammo() || (infinite_ammo && no_reload) {
            return;
        }
        let mut cost = c.ammo_per_shot;
        if no_reload && self.spare != SPARE_INFINITE {
            let take = cost.min(self.spare);
            self.spare -= take;
            cost -= take;
        } else if no_reload {
            cost = 0;
        }
        self.clip = self.clip.saturating_sub(cost);
    }

    /// Trigger init (BF1 FUN_00288c58 / BF2 FUN_0032e758).
    fn trigger(&mut self, c: &WeaponClass, lcg: &mut GameLcg) {
        self.charge = if c.max_pressed_time > 0.0 {
            self.refire_accum / c.max_pressed_time
        } else {
            1.0
        };
        self.stage = c.stage_for(self.charge).min(usize::from(u8::MAX)) as u8;
        self.refire_accum = 0.0;
        self.refire_time = lcg.uniform(c.shot_delay[0], c.shot_delay[1]);
        self.salvo_counter = c.salvo_count;
        self.salvo_timer = c.initial_salvo_delay;
    }

    /// FUN_002891c0: the firing state may return to idle.
    fn cycle_finished(&self, c: &WeaponClass, blocked: bool, cheats: (bool, bool)) -> bool {
        self.salvo_counter < 1
            && self.refire_time <= self.refire_accum
            && (c.trigger_single
                || !self.fire_predicate(c)
                || self.reloading()
                || !self.can_pay(c, cheats.0, cheats.1)
                || blocked)
    }

    /// One tick of the weapon (state machine + fire cycle + heat + reload +
    /// spread recovery). Spawned shots are appended to `shots`.
    pub fn update(
        &mut self,
        dt: f32,
        c: &WeaponClass,
        input: FireInput,
        lcg: &mut GameLcg,
        shots: &mut Vec<ShotSpec>,
    ) {
        let FireInput {
            held: fire_held,
            blocked,
            infinite_ammo,
            no_reload,
            ai_dispersion,
        } = input;
        let cheats = (infinite_ammo, no_reload);
        // The PS2's AI writes `weapon+0x138` every aim tick, before anything
        // in the fire cycle can ask for a spread; ours arrives with the pad.
        self.ai_dispersion = ai_dispersion;
        self.set_fire_input(fire_held);
        self.began_idle = self.state == FireState::Idle;

        // Heat (BF1 weapon update): overheat at 1.0, recover, clear below threshold.
        if !self.overheated {
            self.overheated = self.heat >= 1.0;
        }
        self.heat = (self.heat - dt * c.heat_recover_rate).max(0.0);
        if self.overheated {
            self.overheated = c.heat_threshold < self.heat;
        }

        // Reload timer (provisional machine): refill from spare on expiry.
        if self.reload_timer > 0.0 {
            self.reload_timer -= dt;
            if self.reload_timer <= 0.0 {
                self.reload_timer = 0.0;
                if std::mem::take(&mut self.free_reload) {
                    // The `ammo` cheat: a full clip, the reserve untouched.
                    self.clip = self.clip.max(c.rounds_per_clip);
                } else {
                    let take =
                        (c.rounds_per_clip - self.clip.min(c.rounds_per_clip)).min(self.spare);
                    self.clip += take;
                    if self.spare != SPARE_INFINITE {
                        self.spare -= take;
                    }
                }
            }
        }
        if !no_reload
            && !c.infinite_ammo()
            && self.active
            && self.state == FireState::Idle
            && self.clip < c.ammo_per_shot
            && !self.reloading()
        {
            self.start_reload_as(c, infinite_ammo);
        }

        // Charge enter (B1 cannon `FUN_00289050` 211479-211486, B2 twin;
        // `analysis/re-weapon-charge.md` §2): a fresh press of a charge weapon
        // starts the accumulator at 0 with MaxPressedTime its limit (in place
        // of the refire period); the idle tick below fills it while the
        // trigger is held, and the trigger reads charge = accum /
        // MaxPressedTime. The port used the refire period as the limit, so a
        // held charge stopped at ShotDelay / MaxPressedTime (issue #29).
        if c.max_pressed_time > 0.0 && self.state == FireState::Idle && self.fire_cmd == 1 {
            self.refire_accum = 0.0;
            self.refire_time = c.max_pressed_time;
        }

        // Idle → trigger (weapon update state 0: active && fire predicate).
        if self.state == FireState::Idle
            && self.active
            && !blocked
            && !self.reloading()
            && self.fire_predicate(c)
            // A grenade weapon with nothing in the hand does not wind up.
            && (c.thrown.is_none() || self.can_pay(c, infinite_ammo, no_reload))
        {
            self.trigger(c, lcg);
            self.state = FireState::Firing;
        }

        if self.state == FireState::Firing {
            // The stage the trigger reached fires: its ShotsPerSalvo, offsets
            // and heat; the ammunition is the weapon's (§3).
            let sc = c.stage(self.stage());
            let first = shots.len();
            self.fire_cycle(dt, sc, blocked, cheats, lcg, shots);
            for s in &mut shots[first..] {
                s.stage = self.stage;
            }
            if self.cycle_finished(sc, blocked, cheats) {
                self.state = FireState::Idle;
                self.stage = 0;
                if !no_reload && !c.infinite_ammo() && self.active && self.clip < c.ammo_per_shot {
                    self.start_reload_as(c, infinite_ammo);
                }
            }
        } else {
            // Idle: the accumulator still saturates toward the period so
            // cycle_finished/trigger semantics stay consistent.
            self.refire_accum = (self.refire_accum + dt).min(self.refire_time);
        }

        // Per-tick spread recovery (FUN_00288930 / FUN_0032e200).
        let floor = Self::spread_floor(c, self.human, self.spread_mult);
        self.spread = (self.spread - dt * c.spread_recover_rate).max(floor);
    }

    /// Where the spread state rests: the PS2's floor (a human's
    /// -SpreadThreshold, an AI's 0; ctor `FUN_00288858`) less the port's
    /// [`WeaponClass::tight_hold`].
    fn spread_floor(c: &WeaponClass, human: bool, mult: f32) -> f32 {
        let ps2 = if human { -c.spread_threshold } else { 0.0 };
        ps2 - c.tight_hold(human, mult)
    }

    /// FUN_00288d10 / FUN_0032e8c0 transcription.
    fn fire_cycle(
        &mut self,
        dt: f32,
        c: &WeaponClass,
        blocked: bool,
        (infinite_ammo, no_reload): (bool, bool),
        lcg: &mut GameLcg,
        shots: &mut Vec<ShotSpec>,
    ) {
        if !self.active {
            self.salvo_counter = 0;
        }
        let mut dt_left = dt;
        if dt_left <= FIRE_EPS {
            return;
        }
        loop {
            let take;
            if self.salvo_counter < 1 {
                // Refire branch: accumulate up to the period, then gate.
                let deficit = self.refire_time - self.refire_accum;
                take = if deficit < dt_left { deficit } else { dt_left };
                self.refire_accum += take;
                if self.refire_accum < self.refire_time
                    || c.trigger_single
                    || !self.fire_predicate(c)
                    || self.reloading()
                    || !self.can_pay(c, infinite_ammo, no_reload)
                    || blocked
                {
                    break;
                }
                self.refire_accum -= self.refire_time;
                self.salvo_counter = c.salvo_count;
                self.salvo_timer = c.initial_salvo_delay;
            } else {
                if self.salvo_timer <= 0.0 {
                    loop {
                        for _ in 0..c.shots_per_salvo {
                            if !self.can_pay(c, infinite_ammo, no_reload) {
                                self.salvo_counter = 0;
                                break;
                            }
                            shots.push(ShotSpec {
                                tilt: c
                                    .shot_tilts
                                    .get(self.tilt_index as usize)
                                    .copied()
                                    .unwrap_or_default(),
                                charge: self.charge,
                                stage: 0,
                                pitch_spread: self.pitch_spread(c),
                                yaw_spread: self.yaw_spread(c),
                            });
                            self.pay(c, infinite_ammo, no_reload);
                            self.heat += c.heat_per_shot;
                            self.shots_fired += 1;
                            self.tilt_index =
                                (self.tilt_index + 1) % c.shot_tilts.len().max(1) as u32;
                        }
                        // Spread grows once per salvo, capped at SpreadLimit.
                        self.spread = (self.spread + self.spread_mult * c.spread_per_shot)
                            .min(c.spread_limit);
                        self.salvo_counter = self.salvo_counter.saturating_sub(1);
                        self.salvo_timer += lcg.uniform(c.salvo_delay[0], c.salvo_delay[1]);
                        if self.salvo_counter < 1 || 0.0 < self.salvo_timer {
                            break;
                        }
                    }
                }
                take = if self.salvo_timer < dt_left {
                    self.salvo_timer
                } else {
                    dt_left
                };
                self.salvo_timer -= take;
            }
            dt_left -= take;
            if dt_left <= FIRE_EPS {
                break;
            }
        }
    }

    /// Hash-feed for the sim state hash.
    pub fn hash_into(&self, eat: &mut impl FnMut(&[u8])) {
        for v in [
            self.refire_accum,
            self.refire_time,
            self.salvo_timer,
            self.charge,
            self.spread,
            self.posture_base,
            self.reload_timer,
            self.heat,
        ] {
            eat(&v.to_bits().to_le_bytes());
        }
        // `stage` follows from `charge` (hashed above) and the class.
        eat(&self.salvo_counter.to_le_bytes());
        eat(&self.clip.to_le_bytes());
        eat(&self.spare.to_le_bytes());
        eat(&self.fire_cmd.to_le_bytes());
        if let Some(melee) = &self.melee {
            melee.hash_into(eat);
        }
        if let Some(flight) = &self.saber_flight {
            flight.hash_into(eat);
        }
        if let Some(channel) = &self.force_channel {
            channel.hash_into(eat);
        }
        if self.remote_press > 0 {
            eat(b"REMOTE");
            eat(&[self.remote_press]);
        }
        eat(&[self.state as u8, self.overheated as u8, self.human as u8]);
        if self.free_reload {
            eat(b"FREE");
        }
        if self.tilt_index != 0 {
            eat(b"tilt");
            eat(&self.tilt_index.to_le_bytes());
        }
        if self.secondary_rounds != 0 {
            eat(b"SEC");
            eat(&self.secondary_rounds.to_le_bytes());
        }
        if let Some(t) = self.lock_target {
            eat(b"LOCK");
            match t {
                crate::missile::MissileTarget::Unit(h) => {
                    eat(&[0]);
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
                crate::missile::MissileTarget::Vehicle(h) => {
                    eat(&[1]);
                    eat(&h.index.to_le_bytes());
                    eat(&h.gen.to_le_bytes());
                }
                crate::missile::MissileTarget::Turret(id) => {
                    eat(&[2]);
                    eat(&id.to_le_bytes());
                }
                crate::missile::MissileTarget::Station(s) => {
                    eat(&[3]);
                    eat(&s.to_le_bytes());
                }
            }
            eat(&self.lock_held.to_bits().to_le_bytes());
        }
    }
}

/// FUN_002892a8 steps 3–4: triangular pitch then yaw offsets (four LCG steps
/// each, pitch first), then the local direction (sinY·cosP, −sinP, cosY·cosP)
/// expressed in the muzzle frame (`right`, `up`, `fwd`). `tilt` = salvo tilt
/// pair (pitch, yaw) in radians, zero for rifles.
pub fn shot_direction(
    lcg: &mut GameLcg,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
    pitch_mag: f32,
    yaw_mag: f32,
    tilt: (f32, f32),
) -> Vec3 {
    let pitch = tilt.0 + lcg.triangular(pitch_mag);
    let yaw = tilt.1 + lcg.triangular(yaw_mag);
    let (sp, cp) = fmath::sin_cos(pitch);
    let (sy, cy) = fmath::sin_cos(yaw);
    (right * (sy * cp) + up * (-sp) + fwd * (cy * cp)).normalize()
}

#[cfg(test)]
pub(crate) mod tests;
