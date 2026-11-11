//! What the sim IS, and how it is hashed: the [`Sim`] struct with its
//! counters ([`NavStats`]), cheats ([`Cheats`]) and per-tick output
//! ([`Event`], [`DeathCause`]) and the fall rule ([`LETHAL_LANDING_SPEED`]);
//! the constructors ([`Sim::empty`], [`Sim::new`]); the queries — a unit by
//! handle, the ground under a point ([`Sim::ground_under`]), the terrain ray;
//! [`omni_radius_for`]; and [`Sim::hash`], what every replay gate compares.
//! The child tests preserve the pinned `--sim-hash 100` assertion.

use super::spawn::SpawnWaves;
use crate::aim_assist::AssistState;
use crate::battle::Battle;
use crate::collision::{
    CollisionWorld, FOOT_RADIUS, FOOT_RING, FOOT_RING_DIRS, NO_GROUND, STEP_HEIGHT, TERRAIN_BURY,
};
use crate::cp::CommandPost;
use crate::heightfield::Heightfield;
use crate::lcg::GameLcg;
use crate::projectile::Bolt;
use crate::region::Region;
use crate::rng::SimRng;
use crate::unit::{Unit, UnitHandle, UnitSpec};
use crate::weapon::WeaponClass;
use glam::Vec3;

/// PROVISIONAL (ours): a soldier that lands at or beyond this DOWNWARD speed
/// dies unless its class has zero collision damage — 25 m/s, about a 17 m drop at the soldier's g = 18
/// (`core::soldier::GRAVITY`). Both games deal landing damage by a law the
/// port does not have yet — (approach − CollisionThreshold)² × CollisionScale
/// per class (`analysis/re-soldier-ground.md` §4; cards SLAND, DTHR) — so this
/// is a single lethal-speed cut-off, not a damage model. It is also what makes
/// [`crate::collision::NO_GROUND`] mean what it says — nothing catches you.
pub const LETHAL_LANDING_SPEED: f32 = 25.0;

/// [`Event::Hit`]'s `age` for a blast's hit: no round flew to the victim.
/// A bolt that lands in its first update strikes at age 0 (`Bolt::advance`;
/// a 2000 m/s sniper bolt inside 66 m), so 0 cannot also mean a blast.
///
/// [`Bolt::advance`]: crate::projectile::Bolt::advance
pub const BLAST_AGE: f32 = -1.0;

/// [`Sim::outcome`] when the time limit runs out on equal reinforcements:
/// BF2's `ObjectiveConquest:GameOptionsTimeLimitUp` calls
/// `MissionVictory({1,2})` — BOTH teams win (`analysis/re-time-limit.md` §1
/// hop 9 [V][O]). 2 is no team, so it hashes apart from either win and from
/// "undecided" (255).
pub const OUTCOME_DRAW: u8 = 2;

/// Session cheats (PLAN §Cheats, card CHEAT2): the PS2's `sos` and `ammo`,
/// plus openbf's no reloading, infinite stamina, unlock-all, jet fuel and
/// all-awards. Toggled from the pause menu's rows or its Morse words
/// (`game::cheats`); the game keeps them for the session and copies them
/// into each battle's `Sim`. They apply to PLAYER units only (the PS2 gates
/// on a human controller). Not hashed and not in the input log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats {
    pub invincible: bool,
    /// The reserve never goes down; the clip still empties and reloads
    /// (refilled for free).
    pub infinite_ammo: bool,
    /// `[ours]` The clip never empties: shots draw from the reserve, then
    /// from the clip, with no reload. With `infinite_ammo` nothing is drawn.
    pub no_reload: bool,
    /// Sprint energy never drains, and `[ours]` neither does a jet class's
    /// jet fuel (one cheat for both, 2026-10-28).
    pub infinite_stamina: bool,
    /// `[ours]` Every class costs the player 0 points (`game::unlocks`).
    pub all_classes: bool,
    /// `[ours]` The player holds every BF2 weapon medal on either game's
    /// maps: each class's award weapons replace the ones they stand for
    /// (`Sim::award_bits`; a BF1 class carries BF2's, `game::roster::awards`).
    /// Not in the space modes. Earned medals are issue #130.
    pub all_awards: bool,
    /// `[ours]` The player holds BF2's damage award (medal 8, "War Hero"):
    /// x1.5 on everything dealt (`sim::buffs::WAR_HERO`). Its own row so the
    /// award weapons can be judged at their own damage. Not in space.
    pub damage_award: bool,
    /// `[ours]` The player holds BF2's defense award (medal 7, "Guardian"):
    /// x0.5 on all damage taken (`sim::buffs::GUARDIAN`). Not in space.
    pub defense_award: bool,
    /// `[ours]` The player holds BF2's energy award (medal 6, "Endurance"):
    /// x1.5 energy refill for him and the vehicle he drives
    /// (`sim::awards::ENDURANCE`). Not in space.
    pub energy_award: bool,
    /// `[ours]` The player holds BF2's vehicle award (medal 3,
    /// "Technician"): the vehicle he drives repairs itself
    /// (`Sim::vehicle_regen`). Not in space.
    pub vehicle_award: bool,
}

impl Cheats {
    /// Registry view: (id, label, enabled) in menu order.
    pub fn entries(&self) -> [(&'static str, &'static str, bool); 10] {
        [
            ("invincible", "Invincibility (sos)", self.invincible),
            ("ammo", "Infinite ammo (ammo)", self.infinite_ammo),
            ("clip", "No reloading (clip)", self.no_reload),
            ("stamina", "Infinite stamina (run)", self.infinite_stamina),
            ("classes", "Unlock all classes (all)", self.all_classes),
            ("awards", "Award weapons", self.all_awards),
            (
                "damage",
                "War Hero: damage increase (x1.5)",
                self.damage_award,
            ),
            (
                "guardian",
                "Guardian: damage reduction (x0.5)",
                self.defense_award,
            ),
            (
                "endurance",
                "Endurance: energy regen (x1.5)",
                self.energy_award,
            ),
            (
                "technician",
                "Technician: vehicle regen",
                self.vehicle_award,
            ),
        ]
    }

    pub fn toggle(&mut self, id: &str) {
        match id {
            "invincible" => self.invincible = !self.invincible,
            "ammo" => self.infinite_ammo = !self.infinite_ammo,
            "clip" => self.no_reload = !self.no_reload,
            "stamina" => self.infinite_stamina = !self.infinite_stamina,
            "classes" => self.all_classes = !self.all_classes,
            "awards" => self.all_awards = !self.all_awards,
            "damage" => self.damage_award = !self.damage_award,
            "guardian" => self.defense_award = !self.defense_award,
            "endurance" => self.energy_award = !self.energy_award,
            "technician" => self.vehicle_award = !self.vehicle_award,
            _ => {}
        }
    }

    /// The registry id a pause-menu Morse word toggles: the PS2's `sos` and
    /// `ammo`, and `[ours]` `clip`, `run` and `all`.
    pub fn id_for_word(word: &str) -> Option<&'static str> {
        Some(match word {
            "sos" => "invincible",
            "ammo" => "ammo",
            "clip" => "clip",
            "run" => "stamina",
            "all" => "classes",
            _ => return None,
        })
    }

    /// The five the Morse words toggle one by one.
    pub const FIVE: [&'static str; 5] = ["invincible", "ammo", "clip", "stamina", "classes"];

    /// `[ours]` The `cheat` word: all five on, or all five off when they
    /// already are. Returns whether they are on now.
    pub fn toggle_five(&mut self) -> bool {
        let on = !Self::FIVE.iter().all(|id| self.is_on(id));
        for id in Self::FIVE {
            if self.is_on(id) != on {
                self.toggle(id);
            }
        }
        on
    }

    /// Is the cheat `id` on?
    pub fn is_on(&self, id: &str) -> bool {
        self.entries().iter().any(|e| e.0 == id && e.2)
    }
}

/// Card HOLD: a live explosive lying on the ground — a landed grenade — that
/// bots within its `radius` get warned of and run clear of (`crate::ai`
/// `think`; the PS2's grenade warning, re-grenades.md §7.4: every AI unit
/// within 7.5 m after a difficulty-ranked delay). The grenade contact pass
/// inserts warnings through [`Sim::add_hazard_by`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hazard {
    pub pos: Vec3,
    /// The warning radius (m).
    pub radius: f32,
    /// The tick it landed: the reaction delay counts from here.
    pub since: u64,
    /// The tick it goes off or is gone: dropped from the list then.
    pub until: u64,
    /// Who threw it (card GRAI): its thrower runs from it at once.
    pub owner: Option<UnitHandle>,
    /// Stable link to a thrown grenade; absent for externally supplied warnings.
    pub warning_id: Option<u64>,
}

/// The PS2's grenade-warning radius (m; re-grenades.md §7.4).
pub const HAZARD_RADIUS: f32 = 7.5;

/// Card UTALK: the chance a trooper's "follow me" is issued as it sets out
/// (B1 `FUN_0024b7a0` 165905, `0x3e99999a`; re-squad-orders.md 5), and a
/// seated AI's need-gunner call (`FUN_0024d020` 167039). The AI's "move
/// out" is issued with chance 1 (`FUN_0024baf0` 166190).
pub const FOLLOW_CHANCE: f32 = 0.3;
pub const NEED_GUNNER_CHANCE: f32 = 0.3;

/// The grenade warning's reaction delay at the PS2's default enemy
/// difficulty (s): uniform in (0.6, 1.3) - B1 `FUN_00236be8` 150870-150939,
/// the difficulty word's default 14 (`FUN_0023a540` 153290) takes the
/// "under 17" pair (re-unit-chatter.md 2.3). PROVISIONAL for the port's
/// other difficulties.
pub const GRENADE_WARN_DELAY: (f32, f32) = (0.6, 1.3);

pub struct Sim {
    pub tick: u64,
    /// The round's difficulty tier (PLAN §Saves: host authority). SIM STATE —
    /// it decides every bot's skill at spawn and the two human layers, so it
    /// is HASHED (one byte, right after `tick`). Default `Normal`.
    pub difficulty: crate::skill::Difficulty,
    /// Card ARVS: a tier per team (0 and 1) over [`Sim::difficulty`], for the
    /// arena's head-to-head (`arena ... vs hard:elite`). Every per-unit tier
    /// read goes through [`Sim::difficulty_of`]; `None` (the default, and
    /// every battle the game runs) is `difficulty`. SIM STATE: hashed after
    /// `difficulty`, but only when one is set, so an unset pair keeps every
    /// hash the sim had before.
    pub team_difficulty: [Option<crate::skill::Difficulty>; 2],
    /// Card DTEAM [ours]: humans the team rule counts on each team (0, 1)
    /// BESIDES the human units in `units` - a sweep's stand-in human
    /// (`round_to_outcome`'s `SWEEP_HUMAN_TEAM`). An INPUT: it reaches the
    /// hash only through the per-team rows ([`Sim::team_rows`]).
    pub extra_humans: [u8; 2],
    /// Card MAIS: the AI settings the mission script sets ([`MissionAi`]).
    /// An INPUT like `omni_radius`, set by whoever builds the sim from the
    /// mission; NOT hashed. The default is every knob's engine default.
    pub mission_ai: MissionAi,
    /// Radius inside which a target is inside the AI's view cone whatever the
    /// angle: BF2's 10 m omni (RE §9.1). **BF1 maps set 0.0** — BF1's cone
    /// test has no omni branch. An INPUT like `nav`, set by whoever builds the
    /// sim from `scene.manifest.source_game`; NOT hashed.
    pub omni_radius: f32,
    /// Which game's ground rules the soldiers move by (card STHOP). An INPUT
    /// like `omni_radius`, set from the manifest's `source_game` by the same
    /// sites ([`crate::soldier::GroundRules::for_game`]); BF2's by default.
    pub ground_rules: crate::soldier::GroundRules,
    /// Host option: apply the Elite team handicap (bleed ×1.25 against a team
    /// with a human whose opponent has none, D12). An INPUT — constant for a
    /// round; what it gates reaches the hash through tickets. (Its twin, the
    /// human damage multiplier, was retired by card DRETIRE.)
    pub team_handicap_on: bool,
    pub rng: SimRng,
    pub lcg: GameLcg,
    /// Native DAT_0073d868, separate from weapon-spread DAT_0073d870.
    pub melee_lcg: GameLcg,
    pub melee_lcg_active: bool,
    /// Immutable seat-pose input (both games) for every hit on an exposed
    /// rider. Soldier movement and collision do not read it; vehicle state is
    /// hashed.
    pub hero_rider_frames: Option<std::sync::Arc<dyn super::HeroRiderFrames>>,
    /// Immutable setup for querying animated walker tow collision.
    pub tow_frames: Option<std::sync::Arc<dyn crate::vehicle::tow::TowFrames>>,
    pub tow_cables: Vec<super::TowCable>,
    /// Falling walkers remain in building collision until their clip ends.
    pub tow_falls: Vec<super::TowFall>,
    pub heightfield: Option<Heightfield>,
    /// The map's baked model collision (`coll` meshes, `prim` primitives,
    /// terrain cutters). Empty = heightfield-only, as before. An INPUT like the
    /// heightfield and not part of `hash` — except that [`Sim::doors`] MOVE
    /// their bodies in it, which is why the doors' state is hashed — and the
    /// live vehicles' hulls ride in it, placed from their hashed poses
    /// (`Sim::sync_hulls`, card VHULL). Callers set both after `Scene::apply_mode_layer`
    /// (`game::assets::Scene::install_collision`).
    pub collision: CollisionWorld,
    /// BF2's self-opening doors (queue DR2, `crate::door`), each moving
    /// bodies of `collision`. Their `open` parameters are SIM STATE and
    /// hashed — when there are any, so a map with no door (every BF1 map, the
    /// fixtures) hashes exactly as before the doors existed.
    pub doors: Vec<crate::door::Door>,
    pub weapon_classes: Vec<WeaponClass>,
    /// Per-team soldier CLASS tables, index-aligned with what
    /// `UnitSpec::class` names (`game::roster::FactionRoster` fills them from
    /// the mission's mix, else the faction's default list). SETUP data like
    /// `weapon_classes`: set before anyone spawns, never mutated by `step`,
    /// and NOT hashed — a table is an input, what the units took out of it is
    /// the state. Empty (the `Sim::empty` default) = no class system: every
    /// spawn comes from the caller's own `UnitSpec`, exactly as before.
    pub classes: [Vec<crate::unit::UnitClass>; 2],
    pub test_rosters: [Option<(usize, Option<u8>)>; 2],
    /// BF2 mission hero classes and offer lifecycle; absent on other content.
    pub heroes: Option<super::heroes::Heroes>,
    pub deathmatch: Option<super::deathmatch::Deathmatch>,
    pub space_assault: Option<super::space_assault::SpaceAssault>,
    pub ctf: Option<super::ctf::CtfState>,
    /// Per player SLOT, the class index that slot's NEXT respawn takes
    /// (`buttons::SPAWN_CLASS_NEXT`/`PREV` while dead). Parallel to
    /// `players`, grown by `sync_slot_tables`, and HASHED — a player who has
    /// picked the sniper is in a different state from one who has not.
    pub spawn_choice: Vec<u8>,
    /// Per player slot, last tick's spawn-select bits, for edge detection. A
    /// dead soldier does not step, so `Soldier::prev_buttons` cannot serve.
    /// Hashed beside `spawn_choice`.
    pub prev_spawn_bits: Vec<u32>,
    /// Per player slot, the buttons a prompt took that stay out of play until
    /// released (`Sim::eat_held_pads`). Hashed only while some slot holds
    /// one, so a sim that never answered a prompt keeps its hash.
    pub held_eaten: Vec<super::input::HeldEat>,
    /// Per player slot, the POST the spawn map has selected for that slot's
    /// next respawn (card SPSEL; `buttons::SPAWN_POST_*` while dead, the PS2's
    /// pick `Sim::pick_spawn_post`). `None` = never picked: the respawn draws
    /// a node from every post's, as before the spawn map reached the sim.
    /// Hashed only once some slot holds a pick, so a sim whose players never
    /// touched the spawn map keeps its hash.
    pub spawn_post_choice: Vec<Option<crate::unit::SpawnPost>>,
    /// Card SSELA: the human slots pass through the spawn screens (the PS2's
    /// spawn display, re-spawn-flow §1): a slot is on the field only once it
    /// accepts a class, and a dead one comes back only through the spawn map
    /// and the class accept ([`Sim::open_spawn_screens`] turns it on, the
    /// game binary's battles call it). Off (the default: every headless
    /// referee, the golden scenes, `--spawn`) = auto-accept: a dead human
    /// comes back once its 4.0 s timer has run out. An INPUT for the round;
    /// it reaches the hash through the per-slot block below.
    pub spawn_screens: bool,
    /// Card SSELA: per player slot, which spawn screen is up
    /// ([`super::spawn::SpawnStage`]); `Accepted` while the slot is on the
    /// field or waiting out its respawn timer. Parallel to `players`.
    pub spawn_stage: Vec<super::spawn::SpawnStage>,
    /// Card SSELA: per player slot, the side screen's highlighted row (team
    /// 1, team 2, Auto Assign). Parallel to `players`. This and
    /// `spawn_stage` are hashed together, only while `spawn_screens` is on
    /// or a slot holds a non-default value, so every sim that never opens the
    /// screens keeps its hash.
    pub side_choice: Vec<super::spawn::SideChoice>,
    /// Card SSELB: per player slot, the class carousel's input state (the
    /// PS2's `SD+0x25` arm and `SD+0x33` slide, `FUN_00399dd8`): a held
    /// step fires once, and a step while the line-up slides is dropped.
    /// Parallel to `players`; hashed only while some slot's is not idle.
    pub class_carousel: Vec<super::spawn::Carousel>,
    /// Card SSELB: per player slot, the human's points (the PS2's stats+0x1fc,
    /// `analysis/re-spawn-flow.md` §4.3), which unlock a class on the class
    /// screen ([`Sim::class_refusal`]). An INPUT the game writes from the
    /// slot's event feed before each tick (the feed scores the points); not
    /// hashed, like the pads. Missing = 0.
    pub spawn_points: Vec<i32>,
    /// `[ours]` (card UNLK3): per player slot and team, each class's cost
    /// for this slot as its career has earned it down (the game writes it
    /// from the profile; [`Sim::class_refusal`]'s points test reads it in
    /// place of `unlock_points`). An INPUT like `spawn_points`, not hashed.
    /// Missing = the class's own `unlock_points`.
    pub spawn_costs: Vec<[Vec<u32>; 2]>,
    /// Issue #28: per player slot, the ammo of each class it swapped away
    /// from this life ([`super::spawn::ClassAmmo`]); cleared when the slot's
    /// unit dies. Parallel to `players`; hashed only while some slot holds
    /// one, so a sim with no class change keeps its hash.
    pub class_ammo: Vec<Vec<super::spawn::ClassAmmo>>,
    /// How many AI this team has EVER spawned — the cursor `ai_class` walks
    /// the class table with. Hashed (it decides the next AI's class).
    pub ai_spawned: [u32; 2],
    /// Unit slots; dead slots are reused with a bumped generation.
    pub units: Vec<Unit>,
    /// Vehicle slots (`crate::vehicle`, card VENT), reused like `units`.
    /// Hashed after the units — and NOTHING at all when empty, so a
    /// vehicle-free sim hashes exactly as before.
    pub vehicles: Vec<crate::vehicle::Vehicle>,
    /// The vehicle spawn pads (`crate::vehicle_pad`, card VPAD), ticked after
    /// the posts. Hashed last — and NOTHING at all when empty.
    pub vehicle_pads: Vec<crate::vehicle_pad::VehiclePad>,
    /// The map's building turrets (card VTURB, [`Sim::add_building_turret`]):
    /// how each is spawned and bound, its fall and its rise (card TBLD).
    /// Hashed only while one is down.
    pub building_turrets: Vec<super::building_turrets::BuildingTurret>,
    /// Per vehicle CLASS (`Vehicle::class` indexes it), the hover numbers —
    /// `None` for a class nothing integrates (walkers, flyers, turrets).
    /// SETUP data like `weapon_classes`, NOT hashed. A vehicle whose class
    /// has none, or is past the end, stands still (`Sim::tick_hovers`).
    pub hover_params: Vec<Option<crate::vehicle::hover::HoverParams>>,
    /// Per vehicle CLASS, the walker numbers (card WALK) — `None` for a
    /// class that is not a walker. SETUP data, NOT hashed. A walker whose
    /// class has none stands still (`Sim::tick_walkers`).
    pub walker_params: Vec<Option<crate::vehicle::walker::WalkerParams>>,
    /// PS2 flyer class parameters; absent for other vehicle kinds. Setup only.
    pub flyer_params: Vec<Option<crate::vehicle::flyer::FlyerParams>>,
    /// Mission flight bands, setup only.
    pub flight_limits: super::flyers::FlightLimits,
    /// `AddLandingRegion` volumes, setup only (`MissionApplied::landing_regions`).
    /// Only the space scripts make the call, so a ground map has none
    /// (`analysis/re-flyer.md`).
    pub landing_regions: Vec<Region>,
    /// Do bots board and fly flyers? Setup only. A battle turns it on unless
    /// `OPENBF_FLYER_BOTS=0` (`game::vehicle_pads`); off in a bare `Sim`.
    pub flyer_bots: bool,
    /// BF2 authored `EntityPath`s for the AI follower; setup only.
    pub flyer_paths: Vec<crate::vehicle::flyer::FlyerPath>,
    /// BF2 `SetGroundFlyerMap`: halves the follower's roll wobble.
    pub ground_flyer_map: bool,
    /// Per vehicle CLASS, what the use input and the exit read (card VSEAT,
    /// `crate::vehicle::SeatSetup`); SETUP data, NOT hashed. A class past
    /// the end, or `None`, cannot be entered.
    pub seat_setup: Vec<Option<crate::vehicle::SeatSetup>>,
    /// Per vehicle CLASS, its death explosion and its blast sphere (card
    /// VDTH, `crate::vehicle::BlastSetup`); SETUP data, NOT hashed.
    pub blast_setup: Vec<Option<crate::vehicle::BlastSetup>>,
    pub vehicle_damage_effects: Vec<Vec<super::stations::StationDamageEffect>>,
    /// Per vehicle CLASS, issue #35: the cutter's slice time, s (BF2
    /// `TimeRequiredToEject`, `analysis/re-build-zones.md` §11); 0 or
    /// missing = never sliced. SETUP data, NOT hashed.
    pub slice_time: Vec<f32>,
    /// Per vehicle CLASS, its repair a second as a share of its maximum
    /// health while a Technician drives it (`PilotSkillRepairScale`,
    /// `Sim::vehicle_regen`); missing = `awards::DEFAULT_REPAIR_SCALE`.
    /// SETUP data, NOT hashed.
    pub repair_scale: Vec<f32>,
    /// Per vehicle CLASS, the factor a strike on one of its weak parts deals
    /// on a ground map (`weak_lock::struck_factor`): x4 for the AT-TE, the
    /// AT-AT and the spider walker, x2 for every other class; missing =
    /// [`super::GROUND_WEAK_SPOT_FACTOR`]. Space rounds keep the data's.
    /// SETUP data, NOT hashed.
    pub weak_spot_factor: Vec<f32>,
    /// Per vehicle CLASS, its hull's bodies in model space (card VHULL,
    /// `crate::collision::HullPart`) — `None` for a class with no collision
    /// the port builds, or a building turret, whose building is the world's
    /// own body. SETUP data, NOT hashed: a live vehicle's hull in
    /// [`Sim::collision`] follows its hashed pose (`Sim::sync_hulls`).
    pub hull_parts: Vec<Option<std::sync::Arc<[crate::collision::HullPart]>>>,
    pub bolts: Vec<Bolt>,
    pub beams: Vec<super::Beam>,
    /// The thrown grenades in flight or lying armed (card GREN,
    /// [`crate::sticky`]), in throw order. HASHED when there is one.
    pub stickies: Vec<crate::sticky::Sticky>,
    pub mines: Vec<crate::mine::Mine>,
    pub next_mine_id: u64,
    pub guided: Vec<crate::guided::GuidedRocket>,
    pub next_guided_id: u64,
    pub droids: Vec<crate::remote::Droid>,
    pub next_droid_id: u64,
    pub strikes: Vec<crate::remote::StrikeRun>,
    pub turrets: Vec<crate::autoturret::Turret>,
    pub next_turret_id: u64,
    /// The `powerupitem` classes (card PWUP, `sim/pickups.rs`). SETUP data
    /// like `weapon_classes`, NOT hashed.
    pub item_classes: Vec<super::pickups::ItemClass>,
    /// The `PowerupItem` pool's capacity: a death drops nothing while this
    /// many items lie on the field (25; BF1 missions set 25 to 60). An INPUT.
    pub item_pool: usize,
    /// The items on the field, in drop order. HASHED when there is one.
    pub pickups: Vec<super::pickups::Pickup>,
    /// The `powerupstation` classes (card PWST, `sim/stations.rs`). SETUP
    /// data like `item_classes`, NOT hashed.
    pub station_classes: Vec<super::stations::StationClass>,
    /// The placed health / ammo / vehicle droids, in manifest order. HASHED
    /// when there is one.
    pub stations: Vec<super::stations::Station>,
    /// Body indices and body-local transforms, indexed by placed station.
    pub station_bodies: Vec<Vec<(usize, glam::Mat4)>>,
    /// Authored wreck hull parts, indexed by station class.
    pub station_wreck_collision: Vec<Vec<crate::collision::HullPart>>,
    pub station_live_collision: Vec<Vec<crate::collision::HullPart>>,
    pub battle: Battle,
    pub cheats: Cheats,
    /// BF1 DAT_00517508 / BF2 DAT_005fcbe0: the profile's friendly-fire
    /// percent x 0.01, 100 in every instant-action battle state read (326
    /// states, both games; re-blaster-damage.md section 6.3). A bot's shot
    /// into a human of its own team is dropped before this scale
    /// (`resolve_bolt`, `blast_unit`).
    pub friendly_fire_scale: f32,
    /// PLAN intensity slider; 1.0 = PS2-authentic.
    pub assist_intensity: f32,
    /// The player's controller look sensitivity, which the pad sampler has
    /// already put on the right stick: the aim assist reads the push before
    /// it. A session setting like `assist_intensity`; 1 = none. Neither is
    /// in the input log, so a replay runs both at 1.
    pub pad_look_sensitivity: f32,
    /// Term-3 gate (SP Auto Aim option); default on.
    pub assist_armed: bool,
    /// Aim-assist state per player slot (parallel to `players`).
    pub assist: Vec<AssistState>,
    /// OURS: the aim assist's lead on the magnet (`core::aim_assist::Lead`);
    /// off = the PS2's assist. Default on.
    pub assist_lead_on: bool,
    /// Per player slot (parallel to `players`): the magnet the lead follows
    /// and its state; `None` while the slot has no assisted magnet.
    pub assist_lead: Vec<Option<(super::input::AimTarget, crate::aim_assist::Lead)>>,
    /// Aim magnet per player slot (parallel to `players`): a unit, or a
    /// vehicle / building turret (card HTGT).
    pub magnet: Vec<Option<super::input::AimTarget>>,
    /// Per player slot: ticks the held magnet has been out of sight
    /// (`MAGNET_LOS_GRACE`). Hashed only when one is not 0.
    pub magnet_hidden: Vec<u8>,
    pub lock_on: Vec<super::lock_on::LockOn>,
    /// Per player slot: the on-foot launcher lock on every ground map
    /// (`sim::foot_lock`); unused in space rounds.
    pub foot_lock: Vec<super::foot_lock::FootLock>,
    /// What each player slot's crosshair picks, with its relation (−1
    /// enemy, 0 team 0, 1 friend): the PS2's K+0x30 / K+0x40, which the HUD
    /// reticle colours by (`analysis/re-aim-assist.md` §3.7, card RETC) and
    /// the target readout shows (card HTGT). Written by `acquire_magnet`;
    /// NOT hashed while nothing in the sim reads it.
    pub reticle_pick: Vec<Option<(super::input::AimTarget, i8)>>,
    pub events: Vec<Event>,
    /// Each human's buttons that went down this tick, by unit index, taken
    /// before movement replaces the soldier's previous buttons. Only the
    /// empty click reads it; like `events`, it never enters the hash.
    pub(super) press_edges: Vec<u32>,
    /// Birth-step post-flight endpoints, including rounds already removed.
    /// Item rounds born after flight keep provenance on their live Bolt instead.
    /// Like events/reticle_pick, these never drive physics or enter the hash.
    pub shot_flights: Vec<crate::projectile::ShotFlight>,
    /// The unit driven by each pad passed to [`Sim::step`], indexed by player
    /// slot: slot `s` is driven by `pads[s]`. A slot whose entry is `None` has
    /// no unit; a slot past the end of the `pads` slice idles that tick.
    pub players: Vec<Option<UnitHandle>>,
    pub target_hints: [super::target_hints::TargetHints; 4],
    pub cps: Vec<CommandPost>,
    /// Zero-CP domination timer: (dominating team, seconds).
    pub domination: Option<(u8, f32)>,
    /// Winner once the round is decided (BF2: a team at 0 tickets loses
    /// immediately; domination timer at 20 s; the time limit, where one is
    /// set, gives it on tickets — [`OUTCOME_DRAW`] when they are equal). The
    /// sim keeps stepping.
    pub outcome: Option<u8>,
    /// The conquest time limit in seconds (the Instant Action `con_timer`
    /// option, queue TLO; `analysis/re-time-limit.md` §1 [V][O]). `None` =
    /// Off, the disc's power-on value and the default: no limit ends the
    /// round. Counted on the sim clock (`tick`). An INPUT like
    /// `team_handicap_on` — constant for a round, NOT hashed; what it decides
    /// reaches the hash through `outcome`.
    pub time_limit: Option<u32>,
    /// Death → respawn delay (BF2 FUN_0034f650 / BF1 FUN_00297440: 4.0 s).
    pub respawn_delay: f32,
    /// Per-team spawn waves (teams 0/1).
    pub waves: [SpawnWaves; 2],
    pub(super) native_teams: Vec<super::native_teams::NativeTeam>,
    pub(super) ally_spawns: Vec<super::native_teams::AllySpawn>,
    pub(super) team_relations: std::collections::BTreeMap<(u8, u8), bool>,
    /// Legacy/explicit CLI alive-AI cap per team (NumBots default 16).
    /// Main teams can instead use map-relative combatant limits below.
    pub ai_alive_cap: u32,
    /// Optional combatant limits, including reserved human slots (Instant Action).
    /// Menu input like ai_alive_cap; spawning carries its effects into the hash.
    pub team_combatants: Option<[u32; 2]>,
    /// Volumes that kill any living unit standing in them — the mission's
    /// `AddDeathRegion` list, resolved by
    /// `game::layout::MapLayout::apply_mission`. An INPUT like the
    /// heightfield: set once and not part of [`Sim::hash`].
    pub death_regions: Vec<Region>,
    /// The map's authored damage volumes; set once when the map loads, so
    /// not hashed.
    pub damage_regions: Vec<super::DamageRegion>,
    /// The soldiers' fly-height ceiling, world y (card JETP,
    /// `analysis/re-jetpack.md` §5.3): the mission's `SetMaxPlayerFlyHeight`
    /// (`game::mission::MissionDef::max_player_fly_height`), else
    /// [`crate::soldier::DEFAULT_FLY_CEILING`]. An INPUT like
    /// `death_regions`: set once and not hashed.
    pub fly_ceiling: f32,
    /// The map's water level, when it has water (the manifest's `water`
    /// height). An INPUT like `omni_radius`, set by whoever builds the sim;
    /// NOT hashed. Card HOLD reads it: a bot takes no hold position on a
    /// floor under water.
    pub water_level: Option<f32>,
    pub water_mask: Option<crate::water::WetMask>,
    /// Flat-water oscillation, a load-time input. Ocean mode and BF2's
    /// OscillationEnable(0) keep the query at the authored layer height.
    pub water_oscillates: bool,
    /// The map's boundary shapes; set once when the map loads, so not hashed.
    pub boundary: crate::boundary::Boundary,
    /// Card HOLD: the grenades lying on the ground ([`Hazard`]). SIM STATE —
    /// hashed, when there are any, so a round with none hashes as before.
    pub hazards: Vec<Hazard>,
    /// Last grenade warning identity issued; never reused after compaction.
    pub(super) last_warning_id: u64,
    /// The height under which every soldier dies (`sim/bounds.rs`):
    /// [`crate::sim::KILL_FLOOR_DROP`] under the lowest floor the map's graph
    /// and posts stand on. DERIVED by [`Sim::resolve_nav_floors`] from inputs,
    /// so not hashed. `None` = no graph and no post, no floor.
    pub kill_floor: Option<f32>,
    /// Card LLEG: the posts' spawn nodes that lie inside a solid body, each
    /// with the floor a unit spawned there stands on
    /// (`Sim::resolve_spawn_seats`). DERIVED from inputs, so not hashed.
    pub spawn_seats: Vec<(Vec3, f32)>,
    /// Per hub of `nav`, the floor a soldier walks there (NaN = the walk from
    /// the posts never reached it). DERIVED like `kill_floor`; empty = off.
    pub nav_floors: Vec<f32>,
    /// Per arc of `nav`: bit 0 = a soldier cannot WALK it from `a` to `b`,
    /// bit 1 = from `b` to `a` (`Sim::arc_walkable` — open air on the line,
    /// or a far end the walk does not arrive at). DERIVED with `nav_floors`.
    pub nav_unwalkable: Vec<u8>,
    /// Per hub of `nav`: a soldier there has no walkable way back to the
    /// main body of its part of the graph (`Sim::hub_trapped`, card TGAP).
    /// DERIVED with `nav_floors`; empty = none.
    pub nav_trapped: Vec<bool>,
    /// Card TPOS: the map's position table (`ai::tactic`): the places a
    /// soldier might stand to fight and their cover. DERIVED by
    /// [`Sim::build_positions`] from inputs, so not hashed; empty = none.
    pub positions: crate::ai::tactic::PositionTable,
    /// Card TEAM: the unit grid, live only inside the think loop
    /// (`ai::team`). DERIVED, not hashed.
    pub unit_grid: crate::ai::team::UnitGrid,
    /// Deferred AI write-backs, drained after each think pass. Owned by this
    /// sim and reused between ticks; derived scratch, never hashed.
    pub(super) ai_updates: Vec<(
        usize,
        u32,
        Option<crate::vehicle::Mount>,
        crate::ai::AiState,
        super::input::PadState,
        crate::ai::NavTick,
    )>,
    /// The map's authored planning graph (`core::nav`), when it ships one.
    /// An INPUT like the heightfield and the collision world: immutable during
    /// the round and NOT part of [`Sim::hash`]. `None` = the AI walks straight
    /// at its post, as it did before Phase A. Set by
    /// `game::walk::WalkSim::with_teams` from `game::layout::nav_graph`.
    pub nav: Option<crate::nav::NavGraph>,
    /// Card FLNK: the flank table of `nav` between the posts' hubs
    /// (`nav::flank`), with this match's shares. DERIVED from `nav`, the
    /// posts and the seed ([`Sim::build_flank`]), so not hashed; empty =
    /// none.
    pub flank: crate::nav::FlankTable,
    /// Navigation diagnostics for the round (ROUND_VERBOSE / the sweep). An
    /// OUTPUT counter like `events`: deterministic, but not hashed.
    pub nav_stats: NavStats,
    /// Card HITCH: wall-clock ms the last [`Sim::step`] spent on the AI (the
    /// commander's plan, the turret offers, the team board and every bot's
    /// think) - the windowed game's frame recorder reads it. A measurement
    /// of the machine: never read by the sim, not hashed.
    pub ai_wall_ms: f32,
    /// What the last [`Sim::step`] cost, for the slow-frame log. A measurement
    /// like `ai_wall_ms`: never read by the sim, not hashed.
    pub tick_cost: TickCost,
    /// Bot census (`ai::census`): the round's target and tier-lever counts,
    /// bots and proxies apart. A diagnostic OUTPUT like `nav_stats`:
    /// deterministic, never read by the sim, not hashed. Space rounds are
    /// not counted.
    pub bot_census: crate::ai::census::BotCensus,
    /// Bot census: the round's think wall times. A measurement of the
    /// machine like `tick_cost`: never read by the sim, not hashed.
    pub think_hist: crate::ai::census::ThinkHist,
    /// Phase B: the two teams' objective plans (`core::commander`). SIM STATE
    /// — hashed, replay-exact, re-made one team per tick.
    pub commander: [crate::commander::TeamPlan; 2],
    /// Card ARCM: per team, per arc of `nav`, the round's closings by its
    /// bots (`ai::arcm`): an arc that keeps failing leaves the team's route
    /// draw. SIM STATE — hashed once any is counted; empty until then.
    pub arc_fails: [Vec<crate::ai::arcm::ArcFails>; 2],
    /// Card TOKN: the human players' hit tokens and who holds them
    /// (`ai::tokn`). SIM STATE - hashed once any is dealt; empty with no
    /// human or with the budget off.
    pub tokens: Vec<crate::ai::tokn::Token>,
    /// Card DFOCUS: when each living human player was first seen alive at a
    /// tier that focuses on him (`ai::dfocus`: his spawn grace). SIM STATE -
    /// hashed when not empty; empty with no human or below Hard.
    pub focus_born: Vec<crate::ai::dfocus::Born>,
}

/// What one [`Sim::step`] cost (the slow-frame log). A measurement of the
/// machine: nothing in the sim reads it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TickCost {
    /// The bot whose think took longest.
    pub worst_think: Option<UnitCost>,
    /// Every ray the step cast.
    pub rays: crate::collision::cost::RayCost,
}

/// One bot's think in a tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UnitCost {
    pub unit: u32,
    pub ms: f32,
    /// World ray queries it made.
    pub ray_queries: u32,
}

/// What the follower had to fall back on this round — the numbers the sweep
/// prints per map (PLAN Phase A design, acceptance).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NavStats {
    /// Hops the authored table had no entry for, answered by `route_bfs`.
    pub bfs_fallbacks: u64,
    /// Hops neither the table nor BFS could answer: the unit walked straight
    /// at its post.
    pub straight_fallbacks: u64,
    /// Longest run of ticks any unit spent making no progress toward its
    /// waypoint.
    pub stuck_max: u16,
    /// Card WHY: the failed walks of the round by code, in the order of
    /// `ai::why::FAILS` (a window's failure and what came of it each count).
    pub fails: [u64; crate::ai::why::FAILS.len()],
    /// Card ARCM: arcs closed to a bot (`ai::arcm`), arcs a team shunned,
    /// legs drawn round a closed or shunned arc, and legs that kept one for
    /// want of another way.
    pub arcs_closed: u64,
    pub arcs_shunned: u64,
    pub shun_round: u64,
    pub shun_kept: u64,
    /// Legs whose straight line crossed a barrier and got a corner path
    /// instead (Phase A.1, `core::nav::NavGraph::detour`).
    pub detoured_legs: u64,
    /// Unit-ticks in which a unit wanted to move and the way ahead of it was
    /// NOT clear — the cost, and the size, of the steering layer's work.
    pub blocked_ticks: u64,
    /// Legs whose chosen next hub had NO FLOOR at the unit's own level and
    /// were rejected for an alternative (Phase A.2, `core::ai::hub_has_floor`).
    pub level_rejected: u64,
    /// Of those, the ones where every alternative failed too and the unit fell
    /// back to the straight line — the number that says a map's graph and its
    /// geometry disagree.
    pub level_unrouted: u64,
    /// Unit-ticks in which the forward wall SWEEP found a wall.
    pub sweep_hits: u64,
    /// Phase B: plan passes run (both teams together).
    pub plans: u64,
    /// Assignments a plan CHANGED (a unit given a different objective).
    pub assignment_changes: u64,
    /// Per team, a bit per command-post index the team ever ATTACKED — the
    /// "distinct attack objectives used over the round" the acceptance asks
    /// for. Posts past index 63 do not fit and are not counted (no disc map
    /// has more than 16).
    pub attack_used: [u64; 2],
    /// Ticks a unit was handed an objective the graph could not route to.
    pub unroutable_objectives: u64,
    /// The longest `AiState::unreachable_dwell` any unit reached.
    pub unreachable_dwell_max: u16,
    /// Phase C: shots fired by AI units over the round.
    pub ai_shots: u64,
    /// Of those, shots whose target the shooter's perception did NOT hold —
    /// the fairness rule of design decision 2. It must stay 0: a non-zero
    /// count is a bug, and `debug_assert` catches it in the tests.
    pub unperceived_shots: u64,
    /// The most contacts any unit's memory held at once.
    pub contacts_max: u8,
    /// Records made by HEARING (a shot inside `HEAR_RADIUS`, or being hit).
    pub heard_contacts: u64,
    /// Card TEAM: contact records made or refreshed from a friend's sight.
    pub told_contacts: u64,
    /// Card TEAM: Cover roles the team board gave, and AI ticks spent with
    /// one / on a sniper's spot (Overwatch) - the role counts per map.
    pub cover_given: u64,
    pub cover_ticks: u64,
    pub overwatch_ticks: u64,
    /// Card FLNK: Flank roles the team board gave, and AI ticks spent with
    /// one.
    pub flank_given: u64,
    pub flank_ticks: u64,
    /// ...and how they ended: the last leg into the post, a death, another
    /// objective, anything else (a seat, an order); and the stalled bots
    /// recycled while they held one.
    pub flank_ends: [u64; 4],
    pub flank_stalled: u64,
    /// ...and the legs the thinks planned along a flank route.
    pub flank_legs: u64,
    /// AI shots emitted by a fire cycle that was already running when the
    /// trigger came up — the weapon's, not the AI's (see `unperceived_shots`).
    pub carry_shots: u64,
    /// Card RKSLF: ticks a bot's exploding shot was dropped at its release
    /// because its blast would land on its own side
    /// (`ai::release_blasts_own_side`).
    pub blast_holds: u64,
    /// Card KNOCK: soldiers and corpses a blast's push reached
    /// (`Sim::kick_and_push`).
    pub knocks: u64,
    /// Bolts fired by an AI unit that HIT somebody — the numerator of the
    /// sweep's `hit_rate`, against `ai_shots`.
    pub ai_hits: u64,
    /// Card TOKN: ticks a bot without a token pressed fire at a human (its
    /// aim beside him), its bolts and blasts that met him anyway, and of
    /// those the ones it had aimed at someone else (crossfire).
    pub tokn_aimed_off: u64,
    pub tokn_stray: u64,
    pub tokn_cross: u64,
    /// Card FFLN: kills credited to a unit (a bolt's or a blast's), and of
    /// those the ones whose victim was on the killer's own team (not the
    /// killer himself: `self_kills`). OUTPUT only.
    pub unit_kills: u64,
    pub team_kills: u64,
    pub self_kills: u64,
    /// Card FFLN: bolts that hit a unit, and of those a unit of the
    /// shooter's own team. OUTPUT only.
    pub bolt_hits: u64,
    pub friend_hits: u64,
    /// Card FFLN: bot-ticks a shot was withheld because a friend stood in
    /// the line of fire ([`crate::ai::NavTick::line_held`]). OUTPUT only.
    pub line_held: u64,
    /// Card TPOS: the Tactician's queries, and those in which a candidate
    /// with cover from the target passed its filters (B12's ceiling,
    /// [`crate::ai::NavTick::tpos_query`]). OUTPUT only.
    pub tpos_queries: u64,
    pub tpos_ceiling: u64,
    /// ...each `crate::ai::tactic::EV_*` bit's thinks, and (`[16]`) the
    /// queries with a covered candidate that gave no covered pick (every
    /// confirming ray shut). OUTPUT only.
    pub tpos_ev: [u64; 17],
    /// ...and `NavTick::tpos_funnel` summed. OUTPUT only.
    pub tpos_funnel: [u64; 15],
    /// Card HYST: positions replaced by another on one think (flips), and
    /// queries whose winner did not beat the position held by the margin.
    /// OUTPUT only.
    pub tpos_flips: u64,
    pub tpos_kept: u64,
    /// Card HYST: thinks woken early by a hit, a near miss, a new enemy in
    /// sight (one count per bit), and the behaviour changes those thinks
    /// made, by their `Why` code. OUTPUT only.
    pub wakes: [u64; 3],
    pub wake_why: [u64; 32],
    /// Phase D: squad commands ISSUED (one per accepted edge, whatever the
    /// number of bots that took it — `Event::Command` carries that). OUTPUT
    /// only, never hashed: the sweep's player is idle, so the acceptance reads
    /// `commands 0` on all 32 maps.
    pub commands: u64,
    /// Followers RELEASED by the distance rule (brief D6). OUTPUT only.
    pub releases: u64,
    /// Card VAI: bots that entered a vehicle's seat. OUTPUT only.
    pub ai_boards: u64,
    /// Card VAI: bots that got out of a vehicle. OUTPUT only.
    pub ai_dismounts: u64,
    /// VBAIL: distinct low-health driver rides, drivers ordering bail, AI exits.
    pub ai_bail_exposed: u64,
    pub ai_bail_drivers: u64,
    pub ai_bail_occupants: u64,
    /// Card VAI: bot-ticks spent in a seat. OUTPUT only.
    pub ai_drive_ticks: u64,
    /// Card VTAI: bots that entered a gunner seat (of `ai_boards`). OUTPUT only.
    pub ai_gunners: u64,
    /// Card VTAI: bolts bots fired from seat guns (of `ai_shots`). OUTPUT only.
    pub seat_shots: u64,
    /// Card WAI: bots that entered a WALKER's pilot seat (of `ai_boards`),
    /// and the centimetres walkers covered under a bot (XZ, summed per
    /// tick). OUTPUT only.
    pub ai_walker_rides: u64,
    pub ai_walker_cm: u64,
    /// Card WAI2: boarding claims bots made, those dropped on the give-up
    /// clock (a claimer that never reached its vehicle) and those lost to
    /// another bot (`ai::ride::board_goal`). OUTPUT only.
    pub ai_claims: u64,
    pub ai_claim_giveups: u64,
    pub ai_claim_lost: u64,
    /// Card TURR: turret offers made (`ai::man::offer_pass`), bots that sat
    /// down in a building turret, and per [`crate::ai::NavTick::man`] code
    /// the gunners that left by exit rule 1-5 ([1..=5]) and the claims whose
    /// walk did not reach the turret ([6]). OUTPUT only.
    pub man_offers: u64,
    /// ...of those, made on the defence clause alone. OUTPUT only.
    pub man_defence_offers: u64,
    pub man_boards: u64,
    pub man_codes: [u64; 7],
}

/// BF2's 10 m view-cone omni radius, or 0 for a BF1 map (RE §9.1: BF1's cone
/// test `FUN_002357d0` has NO omni branch; BF2's `FUN_002a7230` does). Every
/// site that builds a `Sim` from a baked scene sets `Sim::omni_radius` from
/// this and the manifest's `source_game`.
pub fn omni_radius_for(source_game: &str) -> f32 {
    if source_game.eq_ignore_ascii_case("bf1") {
        0.0
    } else {
        10.0
    }
}

/// Card MAIS: the AI knobs a mission script sets
/// (`analysis/re-ai-difficulty.md` 3; `engine/artifacts/loop/cards/MAIS.md`).
/// The default of each is the PS2 engine's, so a mission that sets none
/// plays as before.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissionAi {
    /// Stock Kamino disables blind tactical jet hops. The hop must clear
    /// the intervening soldier barriers (BF2 FUN_002dbb68 B2:265077–265083).
    pub allow_blind_jet_jumps: bool,
    /// `SetAIViewMultiplier(m)`: every AI's view distance times this (BF2
    /// clamps it to [0.1, 1.0]; default 1.0). Dagobah 0.35, Felucia 0.65.
    pub view_mult: f32,
    /// `SetTeamAggressiveness(team, v)` per team (default 1.0). OURS: below
    /// 1 the team keeps more defenders on its posts
    /// ([`crate::commander::defend_share`]).
    pub aggressiveness: [f32; 2],
    /// `AISnipeSuitabilityDist(d)`, m (BF2 clamps it to [0, 100]; default
    /// 70): a sniper's lane counts ground out to `2 d`
    /// ([`crate::ai::snipe::lane`]).
    pub snipe_suitability: f32,
}

/// `AISnipeSuitabilityDist`'s default (BF2 `DAT_005fdffc`, 70.0).
pub const SNIPE_SUITABILITY_DEFAULT: f32 = 70.0;

impl Default for MissionAi {
    fn default() -> Self {
        Self {
            allow_blind_jet_jumps: true,
            view_mult: 1.0,
            aggressiveness: [1.0; 2],
            snipe_suitability: SNIPE_SUITABILITY_DEFAULT,
        }
    }
}

impl MissionAi {
    /// `SetAIViewMultiplier`'s clamp (BF2 `FUN_002e37c0`).
    pub fn set_view_mult(&mut self, m: f32) {
        self.view_mult = m.clamp(0.1, 1.0);
    }

    /// `AISnipeSuitabilityDist`'s clamp (BF2: [0, 100]).
    pub fn set_snipe_suitability(&mut self, d: f32) {
        self.snipe_suitability = d.clamp(0.0, 100.0);
    }
}

impl Sim {
    /// A space round: Space Assault, or the space one-flag CTF, which today
    /// is the only CTF the battle installs (a ground CTF, once the battle
    /// installs one, stays a ground round: its flag carries at the ground
    /// offset). A map with landing regions counts too; only the space
    /// scripts add them, in every space mode, so on a launched battle this
    /// decides the same rounds and keeps a space fixture that has only them.
    /// Every space-or-ground rule reads this one test, core and game alike,
    /// so that space rounds keep their own behaviour.
    pub fn space_round(&self) -> bool {
        !self.round_rules().ground()
    }

    /// Card MAIS: team `team`'s AI tier as its bots see with it - the
    /// difficulty tier's columns, its `view_mult` times the mission's
    /// [`MissionAi::view_mult`]. Every view distance a bot uses comes from
    /// here.
    pub fn ai_tier(&self, team: u8) -> crate::skill::Tier {
        let mut t = self.difficulty_of(team).tier();
        t.view_mult *= self.mission_ai.view_mult;
        t
    }

    /// Card MAIS: team `team`'s `SetTeamAggressiveness` (1.0 past team 1).
    pub fn aggressiveness(&self, team: u8) -> f32 {
        self.mission_ai
            .aggressiveness
            .get(usize::from(team))
            .copied()
            .unwrap_or(1.0)
    }

    /// The tier of team `team`'s units: its [`Sim::team_difficulty`] override,
    /// else the row the DTEAM rule gives the team ([`Sim::team_rows`]: the
    /// round's [`Sim::difficulty`] unless the team has a human surplus), else
    /// (a team past 1) the round's.
    pub fn difficulty_of(&self, team: u8) -> crate::skill::Difficulty {
        let t = usize::from(team);
        self.team_difficulty
            .get(t)
            .copied()
            .flatten()
            .or_else(|| self.team_rows().get(t).map(|r| r.tier))
            .unwrap_or(self.difficulty)
    }

    /// Card DTEAM: the humans the team rule counts on teams 0 and 1 - every
    /// occupied player slot's unit, alive or dead (a dead unit keeps its
    /// team; a vacated slot, a human who left, counts no more), plus
    /// [`Sim::extra_humans`].
    pub fn humans_per_team(&self) -> [u8; 2] {
        let mut n = self.extra_humans;
        for h in self.players.iter().flatten() {
            if let Some(u) = self.units.get(h.index as usize).filter(|u| u.team < 2) {
                n[usize::from(u.team)] = n[usize::from(u.team)].saturating_add(1);
            }
        }
        // Card DPRX: a human proxy counts as a human here too.
        for u in self
            .units
            .iter()
            .filter(|u| u.proxy_human.is_some() && u.team < 2)
        {
            n[usize::from(u.team)] = n[usize::from(u.team)].saturating_add(1);
        }
        n
    }

    /// Card DTEAM [ours]: each team's bots under the round's tier and the
    /// humans now on each side ([`crate::skill::team_skill`]). Derived, never
    /// stored, so a human joining, leaving or changing team is in the next
    /// read; the skill reaches a bot at its next spawn ([`Sim::bot_skill`]).
    pub fn team_rows(&self) -> [crate::skill::TeamSkill; 2] {
        crate::skill::team_skill(self.difficulty, self.humans_per_team())
    }

    /// The integer skill a bot of team `team` spawns with: the base skill
    /// of the team's [`Sim::team_difficulty`] override (the arena), else the
    /// team's DTEAM row's skill, else (a team past 1) the round's base.
    pub fn bot_skill(&self, team: u8) -> u8 {
        let t = usize::from(team);
        match self.team_difficulty.get(t).copied().flatten() {
            Some(d) => d.base_skill(),
            None => self
                .team_rows()
                .get(t)
                .map_or(self.difficulty.base_skill(), |r| r.level),
        }
    }

    /// The skill vector of whoever shoots AT a unit of team `team`: the
    /// other side's row at its bot skill for teams 0 and 1, the round's for
    /// any other team (the aim blocks' decay and floor).
    pub fn enemy_skill_of(&self, team: u8) -> crate::skill::Skill {
        match team {
            0 | 1 => {
                let e = 1 - team;
                self.difficulty_of(e).skill_at(self.bot_skill(e))
            }
            _ => self.difficulty.skill(),
        }
    }

    /// The tier of whoever shoots AT a unit of team `team`: the other side's
    /// for teams 0 and 1, the round's for any other team.
    pub fn enemy_difficulty_of(&self, team: u8) -> crate::skill::Difficulty {
        match team {
            0 => self.difficulty_of(1),
            1 => self.difficulty_of(0),
            _ => self.difficulty,
        }
    }

    /// Empty world without a heightfield (the [`NO_GROUND`] fallback): BF1
    /// rifle as class 0, tat1r tickets (250/250). Spawn with [`Sim::spawn`].
    pub fn empty(seed: u32) -> Sim {
        Sim {
            tick: 0,
            difficulty: crate::skill::Difficulty::default(),
            team_difficulty: [None; 2],
            extra_humans: [0; 2],
            mission_ai: MissionAi::default(),
            // BF2's omni; a BF1 map's builder sets 0.0 (RE §9.1).
            omni_radius: 10.0,
            ground_rules: crate::soldier::GroundRules::Bf2,
            team_handicap_on: true,
            rng: SimRng::new(seed),
            // Independent generator; decorrelated seed so the two streams
            // don't start on the same word.
            lcg: GameLcg::new(seed ^ 0xA5A5_5A5A),
            // [ours] Round seed maps the native global to a replay stream.
            melee_lcg: GameLcg::new(seed ^ 0x5341_4252),
            melee_lcg_active: false,
            hero_rider_frames: None,
            tow_frames: None,
            tow_cables: Vec::new(),
            tow_falls: Vec::new(),
            heightfield: None,
            collision: CollisionWorld::default(),
            doors: Vec::new(),
            weapon_classes: vec![WeaponClass::bf1_rifle()],
            classes: [Vec::new(), Vec::new()],
            test_rosters: [None; 2],
            heroes: None,
            deathmatch: None,
            space_assault: None,
            ctf: None,
            spawn_choice: Vec::new(),
            spawn_post_choice: Vec::new(),
            prev_spawn_bits: Vec::new(),
            held_eaten: Vec::new(),
            spawn_screens: false,
            spawn_stage: Vec::new(),
            side_choice: Vec::new(),
            class_carousel: Vec::new(),
            spawn_points: Vec::new(),
            spawn_costs: Vec::new(),
            class_ammo: Vec::new(),
            ai_spawned: [0, 0],
            units: Vec::new(),
            vehicles: Vec::new(),
            vehicle_pads: Vec::new(),
            building_turrets: Vec::new(),
            hover_params: Vec::new(),
            flyer_params: Vec::new(),
            flight_limits: super::flyers::FlightLimits::default(),
            landing_regions: Vec::new(),
            flyer_bots: false,
            flyer_paths: Vec::new(),
            ground_flyer_map: false,
            walker_params: Vec::new(),
            seat_setup: Vec::new(),
            blast_setup: Vec::new(),
            vehicle_damage_effects: Vec::new(),
            slice_time: Vec::new(),
            repair_scale: Vec::new(),
            weak_spot_factor: Vec::new(),
            hull_parts: Vec::new(),
            bolts: Vec::new(),
            beams: Vec::new(),
            stickies: Vec::new(),
            mines: Vec::new(),
            next_mine_id: 0,
            guided: Vec::new(),
            next_guided_id: 0,
            droids: Vec::new(),
            next_droid_id: 0,
            turrets: Vec::new(),
            next_turret_id: 0,
            strikes: Vec::new(),
            item_classes: Vec::new(),
            item_pool: super::pickups::ITEM_POOL_DEFAULT,
            pickups: Vec::new(),
            station_classes: Vec::new(),
            stations: Vec::new(),
            station_bodies: Vec::new(),
            station_wreck_collision: Vec::new(),
            station_live_collision: Vec::new(),
            battle: Battle::new([250, 250]),
            cheats: Cheats::default(),
            friendly_fire_scale: 1.0,
            assist_intensity: 1.0,
            pad_look_sensitivity: 1.0,
            assist_armed: true,
            assist: Vec::new(),
            assist_lead_on: true,
            assist_lead: Vec::new(),
            magnet: Vec::new(),
            magnet_hidden: Vec::new(),
            lock_on: Vec::new(),
            foot_lock: Vec::new(),
            reticle_pick: Vec::new(),
            events: Vec::new(),
            press_edges: Vec::new(),
            shot_flights: Vec::new(),
            players: Vec::new(),
            target_hints: Default::default(),
            cps: Vec::new(),
            domination: None,
            outcome: None,
            time_limit: None,
            respawn_delay: 4.0,
            waves: [SpawnWaves::mission_default(), SpawnWaves::mission_default()],
            native_teams: Vec::new(),
            ally_spawns: Vec::new(),
            team_relations: std::collections::BTreeMap::new(),
            ai_alive_cap: 16,
            team_combatants: None,
            death_regions: Vec::new(),
            damage_regions: Vec::new(),
            fly_ceiling: crate::soldier::DEFAULT_FLY_CEILING,
            water_level: None,
            water_oscillates: false,
            water_mask: None,
            boundary: Default::default(),
            hazards: Vec::new(),
            last_warning_id: 0,
            kill_floor: None,
            nav_floors: Vec::new(),
            spawn_seats: Vec::new(),
            nav_unwalkable: Vec::new(),
            nav_trapped: Vec::new(),
            positions: Default::default(),
            unit_grid: Default::default(),
            ai_updates: Vec::new(),
            nav: None,
            flank: Default::default(),
            nav_stats: NavStats::default(),
            ai_wall_ms: 0.0,
            tick_cost: TickCost::default(),
            bot_census: Default::default(),
            think_hist: Default::default(),
            commander: [
                crate::commander::TeamPlan::new(0),
                crate::commander::TeamPlan::new(1),
            ],
            arc_fails: Default::default(),
            tokens: Vec::new(),
            focus_born: Vec::new(),
        }
    }

    /// The default scene: one player at the origin, no heightfield (what
    /// `--sim-hash` / `--replay` run; ground falls back to [`NO_GROUND`]).
    pub fn new(seed: u32) -> Sim {
        let mut s = Sim::empty(seed);
        s.spawn(&UnitSpec::player(0, Vec3::ZERO));
        s
    }

    /// A grenade has landed at `pos` and goes off `fuse` ticks from now: bots
    /// within [`HAZARD_RADIUS`] are warned of it (card HOLD).
    pub fn add_hazard(&mut self, pos: Vec3, fuse: u64) {
        self.add_hazard_by(pos, fuse, None);
    }

    /// [`Sim::add_hazard`] for a grenade `owner` threw.
    pub fn add_hazard_by(&mut self, pos: Vec3, fuse: u64, owner: Option<UnitHandle>) {
        self.hazards.push(Hazard {
            pos,
            radius: HAZARD_RADIUS,
            since: self.tick,
            until: self.tick + fuse,
            owner,
            warning_id: None,
        });
    }

    pub fn add_weapon_class(&mut self, mut wc: WeaponClass) -> usize {
        if let Some(d) = &wc.dispenser {
            self.add_item_class(d.item.clone());
        }
        if let Some(r) = wc.remote.as_mut() {
            r.blaster_index = self.item_weapon_class(&r.blaster);
            if let Some(s) = r.destruct.as_ref().and_then(|d| d.salvo.as_ref()) {
                r.shell_index = self.item_weapon_class(&s.shell);
            }
        }
        if let Some(t) = wc.turret.as_mut() {
            t.item.gun_index = self.item_weapon_class(&t.item.gun);
        }
        // A secondary ordnance gets a class entry of its own, so
        // its bolts carry its own damage, LaserTexture and LifeSpan
        // (`analysis/re-ordnance-kinds.md` §7).
        let tracer = wc.secondary.as_ref().map(|sec| WeaponClass {
            name: sec.name.clone(),
            ordnance: sec.ordnance.clone(),
            ..WeaponClass::default()
        });
        if let Some(tracer) = tracer {
            let index = self.add_weapon_class(tracer);
            if let Some(sec) = wc.secondary.as_mut() {
                sec.index = index;
            }
        }
        self.weapon_classes.push(wc);
        self.weapon_classes.len() - 1
    }

    /// Unit `i`'s (`infinite_ammo`, `no_reload`) cheats: a player's only,
    /// on foot and in a seat (the PS2 asks for a human controller).
    pub(crate) fn ammo_cheats(&self, i: usize) -> (bool, bool) {
        let player = self.units[i].is_player;
        (
            player && self.cheats.infinite_ammo,
            player && self.cheats.no_reload,
        )
    }

    fn item_weapon_class(&mut self, wc: &WeaponClass) -> usize {
        let item = |i: usize| {
            self.weapon_classes.iter().any(|w| {
                w.remote
                    .as_ref()
                    .is_some_and(|r| r.blaster_index == i || r.shell_index == i)
                    || w.turret.as_ref().is_some_and(|t| t.item.gun_index == i)
            })
        };
        let found =
            (0..self.weapon_classes.len()).find(|&i| self.weapon_classes[i] == *wc && item(i));
        match found {
            Some(i) => i,
            None => self.add_weapon_class(wc.clone()),
        }
    }

    pub fn unit(&self, h: UnitHandle) -> Option<&Unit> {
        self.units
            .get(h.index as usize)
            .filter(|u| u.gen == h.gen && u.alive)
    }

    pub fn unit_mut(&mut self, h: UnitHandle) -> Option<&mut Unit> {
        self.units
            .get_mut(h.index as usize)
            .filter(|u| u.gen == h.gen && u.alive)
    }

    pub fn handle(&self, index: usize) -> UnitHandle {
        UnitHandle {
            index: index as u32,
            gen: self.units[index].gen,
        }
    }

    /// Alive units with their handles.
    pub fn alive_units(&self) -> impl Iterator<Item = (UnitHandle, &Unit)> {
        self.units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.alive)
            .map(|(i, u)| {
                (
                    UnitHandle {
                        index: i as u32,
                        gen: u.gen,
                    },
                    u,
                )
            })
    }

    /// Ground height under `feet`: the highest walkable model surface between
    /// the probe and the terrain, else the terrain itself.
    ///
    /// The probe starts `STEP_HEIGHT` above the feet (the step a soldier climbs
    /// in one tick) and runs down to the terrain height; a model surface is
    /// taken only if it is inside that span, so a platform more than a step
    /// above the feet is ignored and one below the terrain never wins. With
    /// `rising` (the soldier is moving up) the probe starts AT the feet, so a
    /// jump does not mantle onto a ledge it has not cleared.
    ///
    /// A point inside a terrain cutter has NO terrain under it — the PS2's own
    /// height query returns its −300.0 sentinel there
    /// (analysis/re-terrain-heights.md, FUN_00473db0) — so the model floor of a
    /// cave or hangar becomes the only ground, and [`NO_GROUND`] the floor when
    /// there is not one.
    ///
    /// The terrain is also a ONE-SIDED sheet: feet more than [`TERRAIN_BURY`]
    /// below it are underground (a base or tunnel under an UNCUT heightfield —
    /// BF2 Polis Massa's `cp5con`/`cp6con`) and the terrain does not apply at
    /// all, so the model floor under them is the ground.
    ///
    /// PROVISIONAL: `STEP_HEIGHT`, `TERRAIN_BURY`, `WALKABLE_MIN_NY` and using
    /// −300.0 as a no-terrain floor are all ours (`core::collision`).
    pub fn ground_under(&self, feet: Vec3, rising: bool) -> f32 {
        self.ground_under_with(feet, rising, crate::collision::HullFilter::All)
    }

    /// [`Sim::ground_under`] standing on only the vehicle hulls `hulls`
    /// admits (a pad's spawn ray sees none, card VHULL).
    pub fn ground_under_with(
        &self,
        feet: Vec3,
        rising: bool,
        hulls: crate::collision::HullFilter,
    ) -> f32 {
        let probe = feet.y + if rising { 0.0 } else { STEP_HEIGHT };
        self.floor_under(feet, probe, hulls, true).0
    }

    pub fn water_plane(&self) -> Option<crate::water::Plane<'_>> {
        self.water_level.map(|level| crate::water::Plane {
            level: level
                + if self.water_oscillates {
                    crate::water::bob(self.tick as f32 * crate::TICK_DT)
                } else {
                    0.0
                },
            mask: self.water_mask.as_ref(),
        })
    }

    pub fn water_under(&self, p: Vec3) -> Option<f32> {
        self.water_plane().and_then(|w| w.under(p))
    }

    /// The authored layer height over a wet cell, without the bob. The port's
    /// own bot rules read this (hold spots, ring points): they are no PS2 rule,
    /// and a spot should not be refused on one tick and taken on the next.
    pub fn still_water_under(&self, p: Vec3) -> Option<f32> {
        self.water_level.and_then(|level| {
            crate::water::Plane {
                level,
                mask: self.water_mask.as_ref(),
            }
            .under(p)
        })
    }

    /// [`Sim::ground_under`] as ONE ray, with no foot ring: what is under
    /// exactly this point. The AI's ledge probe asks this (`ai::steer`'s
    /// `floor_ahead`): a step is refused when the point ahead has no floor,
    /// though the foot would still hold the edge behind it.
    pub fn ground_point_under(&self, feet: Vec3, rising: bool) -> f32 {
        let probe = feet.y + if rising { 0.0 } else { STEP_HEIGHT };
        self.floor_under(feet, probe, crate::collision::HullFilter::All, false)
            .0
    }

    /// [`Sim::ground_under`] with the floor's normal: the model ray's, or the
    /// heightfield triangle's; up when there is no ground. The soldier's
    /// floor contact (card STHOP, `Soldier::resolve_ground`), asked AFTER the
    /// move: `from_y` is the feet's height before it, and the probe starts no
    /// lower, so a fall of more than a step in one tick (18 m/s, a 9 m drop)
    /// still finds the floor it crossed.
    pub fn ground_under_n(&self, feet: Vec3, rising: bool, from_y: f32) -> (f32, Vec3) {
        let probe = if rising {
            feet.y
        } else {
            (feet.y + STEP_HEIGHT).max(from_y)
        };
        self.floor_under(feet, probe, crate::collision::HullFilter::All, true)
    }

    /// [`Sim::ground_under_n`] as ONE ray, with no foot ring
    /// ([`Sim::ground_point_under`] with the floor's normal).
    pub fn ground_point_under_n(&self, feet: Vec3, from_y: f32) -> (f32, Vec3) {
        let probe = (feet.y + STEP_HEIGHT).max(from_y);
        self.floor_under(feet, probe, crate::collision::HullFilter::All, false)
    }

    /// [`Sim::ground_under_n`] with the probe only
    /// [`crate::collision::AIR_STEP`] over the feet: what a soldier coming
    /// DOWN in the air asks when the whole step found a face he cannot stand
    /// on (card JARC, `Sim::step` 3a').
    pub fn ground_under_falling(&self, feet: Vec3, from_y: f32) -> (f32, Vec3) {
        let probe = (feet.y + crate::collision::AIR_STEP).max(from_y);
        self.floor_under(feet, probe, crate::collision::HullFilter::All, true)
    }

    /// The floor under the point `p` for ONE down ray from no lower than
    /// `from_y`: where a falling item lands (card PWUP).
    pub(super) fn floor_point_under(&self, p: Vec3, from_y: f32) -> f32 {
        let probe = (p.y + STEP_HEIGHT).max(from_y);
        self.floor_under(p, probe, crate::collision::HullFilter::All, false)
            .0
    }

    /// The floor under the point `p` for ONE down ray from `from_y`, blind
    /// to the vehicles' hulls: a pad's spawn ray (card VREST).
    pub(super) fn floor_point_under_static(&self, p: Vec3, from_y: f32) -> f32 {
        self.floor_under(p, from_y, crate::collision::HullFilter::Static, false)
            .0
    }

    /// The floor under `feet` for a down ray starting at height `probe`;
    /// `ring` = the soldier's foot (the ring of rays), not a bare point.
    fn floor_under(
        &self,
        feet: Vec3,
        probe: f32,
        hulls: crate::collision::HullFilter,
        ring: bool,
    ) -> (f32, Vec3) {
        let mut terrain = self
            .heightfield
            .as_ref()
            .map(|h| h.height_at(feet.x, feet.z));
        if terrain.is_some() && self.collision.is_cut(feet) {
            terrain = None;
        }
        // Buried: the sheet is above the feet by more than a bank climb.
        if terrain.is_some_and(|t| t > feet.y + TERRAIN_BURY) {
            terrain = None;
        }
        let floor = terrain.unwrap_or(NO_GROUND);
        if probe > floor {
            // y >= floor by construction of the ray length.
            let down = |x: f32, z: f32| {
                self.collision
                    .ray_down_with(Vec3::new(x, probe, z), probe - floor, hulls)
            };
            let centre = down(feet.x, feet.z);
            // The PS2's foot is a sphere, not a point (`coll_body_lower`,
            // r 0.57 — re-soldier-ground.md §1.2): it bridges the slits
            // between collision floor pieces that a point falls through
            // (KAM1's cloning rooms: the PS2 soldier stands at 69.272 on a
            // slit, the point foot found the hidden disc 7.9 m down — card
            // GRND, re-kamino.md §4.3). When the centre ray finds no floor
            // within a step under the feet, a ring of rays inside the
            // sphere's reach looks for one, each hit lowered by how far the
            // sphere's underside is over the feet at that radius.
            // The terrain within a step is a floor too: no ring on open ground.
            let near = |h: &(f32, Vec3)| h.0 >= feet.y - STEP_HEIGHT;
            let terrain_near = terrain.is_some_and(|t| t >= feet.y - STEP_HEIGHT);
            if ring && !centre.as_ref().is_some_and(near) && !terrain_near {
                let lift = FOOT_RADIUS - (FOOT_RADIUS * FOOT_RADIUS - FOOT_RING * FOOT_RING).sqrt();
                let mut best = centre;
                for (dx, dz) in FOOT_RING_DIRS {
                    if let Some((y, n)) = down(feet.x + dx * FOOT_RING, feet.z + dz * FOOT_RING) {
                        let y = y - lift;
                        if best.is_none_or(|(b, _)| y > b) {
                            best = Some((y, n));
                        }
                    }
                }
                if let Some(hit) = best {
                    return hit;
                }
            } else if let Some(hit) = centre {
                // The same foot on a short STEEP face (card RAIL). The centre
                // ray stands the soldier on the bevel of a kerb or a rail — a
                // floor to the PS2 too (n.y > 0.6), but one the stick has no
                // hold on (under n.y 0.7 / 0.75, `GroundRules::climb_min_ny`),
                // so the point foot had to climb the whole face on momentum:
                // BF2 Kamino's rim rail (0.5 m up a 53° bevel) let a soldier
                // over at 7 m/s head on and held him at 6.5 m/s, at 45°, and
                // from a stand — the maintainer's "invisible wall". The PS2's foot
                // sphere (r 0.57) rides the bevel's top edge instead. So when
                // the floor under the centre gives no hold, a ring ray that
                // finds one that does, higher and within a step of the feet,
                // is the floor: the top of the kerb, less the sphere's sag.
                if ring && hit.1.y < self.ground_rules.climb_min_ny() && near(&hit) {
                    let lift =
                        FOOT_RADIUS - (FOOT_RADIUS * FOOT_RADIUS - FOOT_RING * FOOT_RING).sqrt();
                    // The highest such surface wins; the feet never go
                    // under the face the centre found.
                    let mut best: Option<(f32, Vec3)> = None;
                    for (dx, dz) in FOOT_RING_DIRS {
                        if let Some((y, n)) = down(feet.x + dx * FOOT_RING, feet.z + dz * FOOT_RING)
                        {
                            if n.y >= self.ground_rules.climb_min_ny()
                                && y > hit.0
                                && best.is_none_or(|(b, _)| y > b)
                            {
                                best = Some((y, n));
                            }
                        }
                    }
                    return match best {
                        Some((y, n)) => ((y - lift).max(hit.0), n),
                        None => hit,
                    };
                }
                return hit;
            }
        }
        match (terrain, self.heightfield.as_ref()) {
            (Some(t), Some(h)) => (t, Vec3::from(h.normal_at(feet.x, feet.z))),
            _ => (floor, Vec3::Y),
        }
    }

    /// The map's terrain ray: heightfield + cutters bundled, so every caller
    /// (AI line of sight, bolt spawn rays, the round diagnostics) crosses the
    /// same sheet. `None` when the map has no heightfield, or when the ray
    /// never crosses the surface within `max_t`. See
    /// [`crate::collision::ray_heightfield`].
    pub fn terrain_ray(&self, origin: Vec3, dir: Vec3, max_t: f32, step: f32) -> Option<f32> {
        let hf = self.heightfield.as_ref()?;
        crate::collision::ray_heightfield(hf, &self.collision.cutters, origin, dir, max_t, step)
    }
}

mod events;
pub use events::*;
mod hash;

#[cfg(test)]
mod tests;
