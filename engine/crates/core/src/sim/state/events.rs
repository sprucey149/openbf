//! What the sim tells the game: `Event`, `Call` and `DeathCause`.

use super::*;

/// Things that happened during a tick (drained by the game layer for HUD,
/// audio, traces and Tier-4 artifacts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    SpaceLinkChanged {
        kind: u8,
        index: u16,
        up: bool,
    },
    SpaceObjectiveDown {
        objective: u16,
        owner: u8,
    },
    FlagPickedUp {
        flag: u8,
        unit: UnitHandle,
        team: u8,
    },
    FlagDropped {
        flag: u8,
        unit: UnitHandle,
        team: u8,
        pos: Vec3,
    },
    FlagCaptured {
        flag: u8,
        unit: UnitHandle,
        team: u8,
    },
    FlagReturned {
        flag: u8,
        team: u8,
    },
    FlagReset {
        flag: u8,
    },
    MineBlast {
        pos: Vec3,
        weapon_class: u32,
        cause: crate::mine::Detonation,
    },
    MineTick {
        pos: Vec3,
        weapon_class: u32,
    },
    Cloak {
        unit: UnitHandle,
        weapon_class: u32,
        pos: Vec3,
    },
    DroidCharge {
        pos: Vec3,
        weapon_class: u32,
    },
    GuidedGone {
        pos: Vec3,
        weapon_class: u32,
        end: crate::guided::GuidedEnd,
    },
    DroidGone {
        pos: Vec3,
        weapon_class: u32,
        end: crate::remote::DroidEnd,
    },
    TurretGone {
        pos: Vec3,
        weapon_class: u32,
        end: crate::autoturret::TurretEnd,
    },
    /// An immediate emitter contact, using its actual chain origin.
    ForceHit {
        owner: UnitHandle,
        target: crate::projectile::Target,
        from: Vec3,
        pos: Vec3,
        weapon_class: u32,
        blocked: bool,
        charge: f32,
    },
    /// A weapon-owned explosion; damage has already run through Sim::explode.
    WeaponBlast {
        pos: Vec3,
        weapon_class: u32,
    },
    /// A round or blast hurt an auto-turret: what the attacker's hit flash
    /// reads (B2 `FUN_001e0580`'s `FUN_0035fd50`, any victim of enemy relation).
    TurretHit {
        shooter: UnitHandle,
        turret: u64,
        team: u8,
        damage: f32,
        health_left: f32,
    },
    Ability {
        unit: UnitHandle,
        weapon_class: u32,
        pos: Vec3,
        charge: f32,
    },
    /// A paid cutter pulse for presentation. Slicing has amount=0: it
    /// changes the slice meter, never health or repair-score credit.
    Repair {
        unit: UnitHandle,
        weapon_class: u32,
        target: crate::projectile::Target,
        pos: Vec3,
        /// Health actually restored, zero for a slice pulse.
        amount: f32,
    },
    Shot {
        unit: UnitHandle,
        /// The weapon class that fired: the unit's own on foot, the seat
        /// gun's for a rider (card VSND - its `FireSound` is the gun's).
        weapon_class: u32,
        origin: Vec3,
        dir: Vec3,
    },
    /// An emitter shot's lightning link (card UORDE,
    /// `analysis/re-ordnance-kinds.md` §8): the instant query drew an arc
    /// from `from` to `to`. Render-only; the damage is already in
    /// [`Event::Hit`] / [`Event::VehicleHit`]. `weapon_class` indexes
    /// [`Sim::weapon_classes`] (its emitter's `LightningEffect` names the
    /// arc).
    Emitter {
        unit: UnitHandle,
        weapon_class: u32,
        from: Vec3,
        to: Vec3,
    },
    Hit {
        /// Presentation metadata only: a direct contact that plays the hit
        /// pose and sets the death side: a round (including a zero-damage
        /// shield contact), a saber swing or throw, or Force choke. Splash,
        /// fat rays, emitters (Force push, pull, lightning) and burns are
        /// not. Never hashed or read to drive gameplay.
        direct: bool,
        shooter: UnitHandle,
        victim: UnitHandle,
        /// The health the hit took (after the shield and the scales; 0 for
        /// a hit the shield swallowed).
        damage: f32,
        /// Bolt age at impact (seconds) — distance = age × velocity; 0 for
        /// a bolt that struck in its first update, [`BLAST_AGE`] for a
        /// blast.
        age: f32,
        /// The victim's health after the hit (0 or less: it killed). The hurt
        /// cry's 2 % test reads it (card UCRY, `analysis/re-unit-chatter.md`
        /// 2.7: B1 `FUN_001bc190` 64387).
        health_left: f32,
        /// The way the shot travelled, unit length (card HATK: the HUD's
        /// hit-direction chevrons, `analysis/re-hud-attacker.md` §3): a
        /// bolt's velocity; a blast's victim centre less the blast centre
        /// (PROVISIONAL, §10), zero when they coincide.
        dir: Vec3,
    },
    Kill {
        shooter: UnitHandle,
        victim: UnitHandle,
    },
    /// A bolt's hit on a prim with a HitLocation factor over 1 that took
    /// health: a soldier's head (beside its [`Event::Hit`]) or a vehicle's
    /// weak part, x2 or x4 on a ground map (beside its [`Event::VehicleHit`]).
    /// The PS2's critical hit, which lights BF2's `hitCritical` reticle
    /// flash for its shooter (`analysis/re-target-hud.md` §12: B2
    /// `FUN_001dfb38` hands the factor to the shooter's hit timer
    /// `FUN_00350558`). `victim` is a [`Target::Unit`] or a
    /// [`Target::Vehicle`]; `weapon_class` indexes [`Sim::weapon_classes`].
    /// The HUD keeps the flash to a sniper rifle on a head and a rocket
    /// launcher on a vehicle (hitmark, the maintainer 2026-10-28).
    ///
    /// [`Target::Unit`]: crate::projectile::Target::Unit
    /// [`Target::Vehicle`]: crate::projectile::Target::Vehicle
    CriticalHit {
        shooter: UnitHandle,
        victim: crate::projectile::Target,
        weapon_class: usize,
    },
    /// Card DROIDDEATH (`analysis/re-droid-death.md`): a dead droid took the
    /// chunk death at its death block - after the [`Event::Kill`] (or
    /// [`Event::EnvDeath`]) of the same death, which the PS2 decides at the
    /// damage. The game draws the class's `ExplosionName` effect at `pos`
    /// (the body's sphere centre) and stops drawing the body; the item drop
    /// follows as on any death. `team` / `class` index [`Sim::classes`];
    /// `scale` is the chunks' launch scale ([`crate::unit::chunk_scale`]).
    DroidBurst {
        unit: UnitHandle,
        pos: Vec3,
        team: u8,
        class: u8,
        scale: f32,
    },
    Droideka {
        unit: UnitHandle,
        pos: Vec3,
        cue: crate::droideka::Cue,
    },
    /// Authored ExplosionExpire at the final flight position; effects and sound
    /// resolve this class separately from its impact explosion.
    OrdnanceExpired {
        pos: Vec3,
        weapon_class: u32,
    },
    BoltExpired {
        pos: Vec3,
        /// The bolt STRUCK something at `pos` — the terrain sheet, a world
        /// model or a hull — rather than running out of life in the air. The fx puff and
        /// the impact sound key off this, and neither cares which it was.
        struck: bool,
    },
    /// A bolt struck `surface` at `pos` (card FXP3): the fx layer spawns the
    /// ordnance's impact effect and its explosion's (`analysis/re-fx.md` §7,
    /// B1 FUN_00276810). Alongside [`Event::BoltExpired`] or [`Event::Hit`],
    /// never instead; the sim never reads it. `dir` is the bolt's unit
    /// direction, `weapon_class` indexes [`Sim::weapon_classes`], and a
    /// struck hull names its `vehicle` (the effect rides it).
    Impact {
        pos: Vec3,
        dir: Vec3,
        weapon_class: u32,
        surface: crate::impact::Surface,
        vehicle: Option<crate::vehicle::VehicleHandle>,
    },
    /// AI shed by the no-overspawn rule when a player took the last slots
    /// (or bleed dropped the pool below the live count).
    Despawn {
        unit: UnitHandle,
    },
    Respawn {
        unit: UnitHandle,
    },
    /// A reload started (ReloadSound hook).
    Reload {
        unit: UnitHandle,
        weapon_class: u32,
    },
    /// `unit` selected the weapon `weapon_class` with a switch input (its
    /// `WeaponChangeSound`, re-weapon-sounds.md §7.1).
    WeaponSelect {
        unit: UnitHandle,
        weapon_class: u32,
    },
    /// A human pressed the trigger on a weapon that refused it
    /// ([`crate::weapon::WeaponState::press_clicks`]): `weapon_class`'s
    /// `FireEmptySound` at `pos`, the shooter or the droid it flies (§7.3).
    FireEmpty {
        unit: UnitHandle,
        weapon_class: u32,
        pos: Vec3,
    },
    /// A grenade left `unit`'s hand (card GREN): the weapon's FireSound and
    /// the drawn throw. `weapon_class` indexes [`Sim::weapon_classes`].
    Throw {
        unit: UnitHandle,
        weapon_class: u32,
        origin: Vec3,
    },
    /// A thrown grenade touched something and bounced, stuck or came to rest
    /// (its `CollisionOtherSound`); `water` for the water's surface.
    StickyContact {
        pos: Vec3,
        weapon_class: u32,
        water: bool,
    },
    /// A thrown grenade's fuse ran out at `pos`: its `ExplosionExpire` class
    /// went off there (the damage is already dealt, [`Sim::explode`]).
    StickyBlast {
        pos: Vec3,
        weapon_class: u32,
    },
    /// A player slot gave a squad order (card SQACK). `kind` is
    /// [`crate::squad::Order`]'s byte (0 follow, 1 hold, 2 at ease, 3 move
    /// out, 4 pile in, 5 everyone out); `units` is how many bots are in its reach — pushed even when
    /// that is 0: the player's line plays whether or not anyone hears it.
    Command {
        slot: u8,
        kind: u8,
        units: u8,
    },
    /// `unit` took slot `slot`'s order at its think and answers "yes" —
    /// whatever the order then makes it do. `did`: the order changed what
    /// the unit does (it now follows or holds, or was released by it) — the
    /// HUD's count (card SQHUD).
    OrderTaken {
        unit: crate::unit::UnitHandle,
        slot: u8,
        did: bool,
    },
    /// Card UCALL: a bot asks for one of BF1's combat calls (its class's
    /// `AcquiredTargetSound`, `FleeSound`, `HeardEnemySound`;
    /// re-unit-chatter.md 2.2-2.4). On a BF2 map only the tracker's take
    /// and the grenade warning, which ask BF2's `VOSound` lines (card UVO2).
    Call {
        unit: crate::unit::UnitHandle,
        call: Call,
    },
    /// An authored saber combo, hit or deflect sound at its unit.
    ComboSound {
        unit: UnitHandle,
        weapon_class: u32,
        sound_hash: u32,
    },
    /// A saber blade touched something at `pos`: the fx layer spawns the
    /// saber class's `HitEffect`, or its `DeflectEffect` for a block. The sim
    /// never reads it.
    SaberContact {
        pos: Vec3,
        weapon_class: u32,
        deflect: bool,
    },
    /// The thrown saber's `HitSound`, which the PS2 plays at the first victim
    /// of the update (B2 FUN_0031d688), not at the thrower.
    SaberThrowHit {
        pos: Vec3,
        weapon_class: u32,
    },
    HeroOffered {
        unit: UnitHandle,
        class: u8,
    },
    Captured {
        cp: u32,
        team: u8,
    },
    Neutralized {
        cp: u32,
        by: u8,
    },
    /// Owner units recovered a post being neutralized.
    Saved {
        cp: u32,
        team: u8,
    },
    /// BF2 rule: tickets hit 0 (or domination timer expired) → round over.
    TeamDefeated {
        team: u8,
    },
    /// The round's time limit ran out ([`Sim::time_limit`]): `winner` is the
    /// team with more reinforcements, or [`OUTCOME_DRAW`] when they are equal.
    TimeUp {
        winner: u8,
    },
    /// The world killed a unit: a mission death region, or a lethal landing.
    /// See [`Sim::kill_environment`].
    EnvDeath {
        unit: UnitHandle,
        cause: DeathCause,
    },
    /// Card UNSN: a stalled bot moved on where no human saw it, in place of
    /// its recycle (`crate::ai::unseen`): a defect, listed by the sweep.
    Relocated {
        unit: UnitHandle,
        from: Vec3,
        to: Vec3,
    },
    /// Damage reached a vehicle's hull pool (card VDMG): a bolt (`shooter`
    /// set) or a collision (`None`). `damage` is what left the pool.
    VehicleHit {
        shooter: Option<UnitHandle>,
        vehicle: crate::vehicle::VehicleHandle,
        damage: f32,
        /// The way the bolt or blast travelled, as [`Event::Hit`]'s `dir`;
        /// `None` for a collision.
        dir: Option<Vec3>,
    },
    /// A vehicle's hull pool ran out and it is gone this tick, no wreck
    /// (re-vehicles §6, [`Sim::destroy_vehicle`]). The game layer reads the
    /// class's `ExplosionName` and chunks from `class`; `pos` / `vel` are the
    /// hull's at death (the chunks inherit `vel`).
    VehicleDestroyed {
        vehicle: crate::vehicle::VehicleHandle,
        class: u32,
        pos: Vec3,
        vel: Vec3,
        killer: Option<UnitHandle>,
    },
    /// A flyer met a surface, obstacle or other craft (SWBFRNT2.ELF-decomp.c lines 121808-121845, 121913-121930).
    /// Emitted for impact sound, sparks, camera jolt and controller feedback on non-lethal scrape or lethal crash.
    FlyerImpact {
        vehicle: crate::vehicle::VehicleHandle,
        pos: Vec3,
        normal: Vec3,
        speed: f32,
        damage: f32,
        lethal: bool,
    },
    HitLanded {
        shooter: UnitHandle,
        team: Option<u8>,
    },
    StationDestroyed {
        station: u16,
        class: u8,
        pos: Vec3,
        killer: Option<UnitHandle>,
    },
    /// A dead soldier dropped an item (card PWUP); `item` indexes
    /// [`Sim::item_classes`].
    ItemDropped {
        item: u8,
        pos: Vec3,
    },
    /// An item came to rest (BF1's `CollisionOtherSound`).
    ItemLanded {
        item: u8,
        pos: Vec3,
    },
    /// `unit` took the item at `pos`: its `PowerupSound`, and the taker's
    /// gain effect for what it gave (`re-powerups.md` §5.6).
    ItemTaken {
        unit: UnitHandle,
        item: u8,
        pos: Vec3,
        health: bool,
        ammo: bool,
        energy: bool,
    },
    /// A station served a unit or a vehicle (card PWST): its `PowerupSound`
    /// at `pos` (the station's), and the unit's gain effect for what it
    /// gave. `station` indexes [`Sim::stations`].
    StationServed {
        station: u16,
        unit: Option<UnitHandle>,
        vehicle: Option<crate::vehicle::VehicleHandle>,
        pos: Vec3,
        health: bool,
        ammo: bool,
        /// BF2 energy: no disc station class gives any (§2.2), always
        /// false today.
        energy: bool,
    },
    /// A door's trigger was entered: its `OpenSound` plays (queue DRV,
    /// [`crate::door::Door::edge`]). `door` indexes [`Sim::doors`].
    DoorOpened {
        door: u32,
    },
    /// A door's trigger emptied: its `CloseSound` plays.
    DoorClosed {
        door: u32,
    },
    /// A soldier approached a locked door after its sound had rearmed.
    DoorLocked {
        door: u32,
    },
}

/// One of BF1's combat calls ([`Event::Call`]); on a BF2 map only
/// [`Call::Acquired`] and [`Call::Flee`] are issued, the triggers of BF2's
/// AcquiredTarget and Grenade lines (card UVO2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// The tracker took a target ("there he is").
    Acquired,
    /// The first AI soldier within [`HAZARD_RADIUS`] of a live grenade
    /// ("grenade!").
    Flee,
    /// Hearing turned the unit to a heard enemy.
    Heard,
    /// Card UTALK: a trooper sets out for a post it is not at - "follow me"
    /// (its class's `AISCFieldFollowSound`; re-squad-orders.md 5).
    Follow,
    /// Card UTALK: a trooper has reached its post - "move out" (its class's
    /// `AISCFieldMoveOutSound`).
    MoveOut,
    /// Card UTALK: an AI that took a vehicle's pilot seat with a free gun
    /// seat calls for a gunner (its class's `AISCDriverGetInSound`).
    NeedGunner,
    /// Card UTALK: an AI soldier on foot within [`ORDER_REACH`] of a
    /// [`Call::NeedGunner`] answers "yes" (its class's
    /// `AISCResponseYesSirSound`).
    ///
    /// [`ORDER_REACH`]: crate::squad::ORDER_REACH
    Yes,
    /// Card UTALK: the class's `LowHealthSound`. Never in the sim's events:
    /// its handle is the sound side's (`game::announce::Commander::calls`).
    LowHealth,
    Cockpit,
    Loudspeaker,
    /// Card UVO2: a BF2 `VOSound` line for the event of this index in the
    /// PS2's table (`game::vo::EVENTS`). Never in the sim's events: the
    /// sound side picks it from them (`game::vo::Vo::tick`).
    Vo(u8),
    /// A Hero Assault hero's own `SndHeroSpawned` / `SndHeroDefeated` line,
    /// said where it stands. Never in the sim's events: the sound side asks
    /// it on a respawn or a death (`game::announce::Commander::calls`).
    HeroSpawned,
    HeroDefeated,
}

/// What the world killed a unit with ([`Event::EnvDeath`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    /// Inside one of the mission's `AddDeathRegion` volumes.
    DeathRegion,
    /// Landed at or beyond [`LETHAL_LANDING_SPEED`].
    Fall,
    /// Riding a vehicle destroyed by no attacker (a crash into the world,
    /// [`Sim::destroy_vehicle`]); a destroyer's riders are his kills.
    VehicleDestroyed,
    /// An explosion with no attacker (an empty vehicle's death blast,
    /// [`Sim::explode`]).
    Blast,
    /// Below the map's kill floor ([`Sim::kill_floor`]).
    KillFloor,
    /// A hard landing under the map ([`Sim::lands_under_the_map`]).
    UnderMap,
    /// A bot off the playable level that found no way back
    /// (`crate::ai` `LOST_RECYCLE`): the last resort.
    Lost,
    /// A bot on the level with somewhere to go that made no net progress
    /// for `crate::ai` `RECYCLE_AFTER` (card RCYC): the last resort, and
    /// every use is a defect to close.
    Stalled,
    /// Confirmed pause-menu Respawn (analysis/re-spawn-flow §7).
    Suicide,
    /// A BF2 hero's time drain exhausted its health without a credited attacker.
    HeroExhausted,
    Water,
    /// Worn down inside an authored damage region (Mustafar's lava).
    DamageRegion,
}

impl DeathCause {
    /// Does the `invincible` cheat spare the player from this? The PS2's
    /// death regions and its world floor kill past the cheat (the kill's
    /// third argument is 0 — `analysis/re-kill-floor.md`), and so does a
    /// destroyed vehicle's (re-vehicle-death 10.1 item 4); its landing damage
    /// and blasts are ordinary damage, which the cheat stops.
    pub fn cheat_spares(self) -> bool {
        matches!(
            self,
            DeathCause::Fall | DeathCause::Blast | DeathCause::Water | DeathCause::DamageRegion
        )
    }
}
