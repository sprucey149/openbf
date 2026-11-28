//! Class and HUD presentation types projected from game::roster: target
//! boxes, weapon rows, class visuals, cries and per-team templates.
//!
//! Owned here: TargetDraw, WeaponHud, DroidHud, ClassVisual, Cries,
//! TeamTemplate, LOW_HEALTH_THRESHOLD and faction_row. Pure presentation
//! data — nothing here enters core::Sim or its inputs.

use core::unit::{Controller, UnitSpec};
use glam::{Mat4, Vec3};

/// A target box to draw: transform, magnet highlight, health fraction.
#[derive(Debug, Clone, Copy)]
pub struct TargetDraw {
    pub transform: Mat4,
    pub magnet: bool,
    pub health_frac: f32,
    pub team: u8,
    pub droideka: bool,
}

/// What the HUD's weapon slot shows for a class's primary weapon (visual
/// only; `crate::roster::weapon_hud`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeaponHud {
    /// What the PS2's weapon-change event carries (`analysis/re-hud.md` §3
    /// `[V][O]`): the weapon class's `HUDTag`, else its own `GeometryName`,
    /// else its name. `hudtransforms` turns it into a mesh.
    pub key: String,
    /// The weapon class's own name.
    pub class: String,
    /// Uses BF2's HUD key resolution, including BF1 vehicle counterparts
    /// (`ui::hud::HudWeapon::bf2`).
    pub bf2: bool,
    /// The class's `IconTexture` (BF1's HUD sheet), for a weapon with no BF2
    /// mesh to show.
    pub icon: Option<String>,
    /// A remote weapon's droid: what the rows show while it flies.
    pub droid: Option<Box<DroidHud>>,
}

/// The weapon rows while a remote droid flies: the droid's own channel-0
/// and channel-1 weapons, as the PS2 publishes the controlled entity's
/// weapons (`analysis/re-remote-droid.md` §8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DroidHud {
    /// Row 1: the blaster (`WEAPONSECTION 1`, channel 0).
    pub blaster: WeaponHud,
    /// Row 2: the self-destruct (channel 1).
    pub destruct: Option<WeaponHud>,
    /// The destruct's count: `WeaponAmmo` clips x `RoundsPerClip`; `None` =
    /// infinite.
    pub destruct_rounds: Option<u32>,
}

/// One class's VISUAL side, index-aligned with `Sim::classes[team]` (the
/// sim's own copy carries the constants; this carries what to draw and what
/// to call it).
#[derive(Debug, Clone, Default)]
pub struct ClassVisual {
    /// ODF class name (`all_inf_marksman`).
    pub name: String,
    /// Display label for the HUD / spawn screen (`game::roster`'s rule).
    pub label: String,
    /// Soldier model, baked at `side_<game>/soldiers/<geometry>.glb`.
    pub geometry: Option<String>,
    /// Primary weapon model, baked at `side_<game>/weapons/<name>.glb`.
    pub weapon_geometry: Option<String>,
    /// The primary weapon in the HUD's weapon slot.
    pub weapon_hud: WeaponHud,
    /// The class's other slots in the HUD (card GREN): `(slot, what to
    /// show)` for each weapon the unit holds besides the primary.
    pub held_hud: Vec<(u8, WeaponHud)>,
    /// The held slots that are award weapons (`MedalsTypeToUnlock`), which
    /// the class screen leaves out of its equipment list, as BF2 does.
    pub award_slots: Vec<u8>,
    /// A droideka's shield in the secondary row (it is no held weapon).
    pub droideka_shield: Option<WeaponHud>,
    /// The model each of those slots is drawn as in the hand (card WHELD):
    /// `(slot, GeometryName)`, `None` = drawn as nothing. A slot not listed
    /// draws the primary's `weapon_geometry`.
    pub held_models: Vec<(u8, Option<String>)>,
    pub first_person: Option<String>,
    pub first_person_weapon: crate::first_person::FpWeapon,
    pub first_person_held: Vec<(u8, crate::first_person::FpWeapon)>,
    pub flee_like_a_hero: bool,
    /// The hash of the class's `FoleyFXClass` (card FOLY, `crate::foley`):
    /// which row of the map's foley table its steps play; 0 = none.
    pub foley_class: u32,
    /// The class's `MinFootHeight`: a step needs a drawn foot below it
    /// (`crate::foley::MIN_FOOT_HEIGHT` unset).
    pub min_foot_height: Option<f32>,
    /// The class's `HurtSound` / `DeathSound` (card UCRY).
    pub cries: Cries,
    /// What picks the class's animation sets (card ANIML).
    pub anim: crate::animator::ClassAnim,
    /// The class's override textures (card USKN).
    pub skins: crate::roster::ClassSkins,
    /// The jet's effect and engine sounds (card JETP2).
    pub jet_look: crate::roster::JetLook,
    /// A class a BF1 Hunt fields from BF2's side
    /// ([`crate::roster::ClassRoster::from_bf2_side`]).
    pub from_bf2_side: bool,
}

/// A soldier class's two cries (card UCRY, `analysis/re-unit-chatter.md`
/// 2.7, 3.6): the sound property names its `HurtSound` and `DeathSound`
/// give, through its base chain; None for a class without one (the Jedi).
/// And its three BF1 combat calls (card UCALL, 2.1-2.4): `AcquiredTargetSound`,
/// `FleeSound`, `HeardEnemySound`. The class's `HidingSound`,
/// `ApproachingTargetSound` and `PreparingForDamageSound` are never asked
/// on the PS2 (2.6) and are not read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cries {
    pub hero: crate::heroes::HeroAudio,
    pub hurt: Option<String>,
    pub death: Option<String>,
    pub acquired: Option<String>,
    pub flee: Option<String>,
    pub heard: Option<String>,
    /// Card UTALK, the AI's own squad talk (re-squad-orders.md 2, 5): the
    /// class's AI-issuer lines `AISCFieldFollowSound`,
    /// `AISCFieldMoveOutSound`, `AISCDriverGetInSound` and the answer
    /// `AISCResponseYesSirSound`.
    pub follow: Option<String>,
    pub move_out: Option<String>,
    pub need_gunner: Option<String>,
    pub yes: Option<String>,
    /// Card UTALK (re-unit-chatter.md 2.8): `LowHealthSound`, asked whenever
    /// the unit's own line is over while it lives with health at or under
    /// its max x `LowHealthThreshold` (the class's; 0.3 where it sets none).
    pub low_health: Option<String>,
    pub low_threshold: f32,
    /// Issue #173: `ChokeSound`, the voice a Force Choke's hold plays on him
    /// (BF2 `impmaleforcechoke`, `allfemaleforcechoke`); none for a class
    /// that names none.
    pub choke: Option<String>,
    /// Card UVO2: BF2's `VOSound` lines, `VOUnitType` and `VORadius`
    /// (`crate::vo`); empty on BF1 classes.
    pub vo: std::sync::Arc<crate::vo::VoLines>,
}

impl Cries {
    /// The property a combat call asks.
    pub fn call(&self, call: core::sim::Call) -> Option<&str> {
        use core::sim::Call;
        match call {
            Call::Acquired => self.acquired.as_deref(),
            Call::Flee => self.flee.as_deref(),
            Call::Heard => self.heard.as_deref(),
            Call::Follow => self.follow.as_deref(),
            Call::MoveOut => self.move_out.as_deref(),
            Call::NeedGunner => self.need_gunner.as_deref(),
            Call::Yes => self.yes.as_deref(),
            Call::LowHealth => self.low_health.as_deref(),
            Call::Cockpit | Call::Loudspeaker => None,
            Call::HeroSpawned => self.hero.spawned.as_deref(),
            Call::HeroDefeated => self.hero.defeated.as_deref(),
            Call::Vo(_) => None,
        }
    }
}

/// The `LowHealthThreshold` a soldier class that sets none has (B1 class
/// +0x1e0c, the constructor's 0.3 at B1:121978; re-unit-chatter.md 2.8).
pub const LOW_HEALTH_THRESHOLD: f32 = 0.3;

impl ClassVisual {
    /// BF2's hero namespace always uses its own models and animation banks,
    /// including when a BF1 mission selects that playable kit; so does a
    /// class a BF1 Hunt fields from BF2's side ([`Self::from_bf2_side`]).
    pub fn asset_game<'a>(&self, map_game: &'a str) -> &'a str {
        if self.from_bf2_side || self.name.split('_').nth(1) == Some("hero") {
            "bf2"
        } else {
            map_game
        }
    }

    pub fn from_roster_with_held(c: &crate::roster::ClassRoster) -> Self {
        Self {
            held_hud: c.held.iter().map(|h| (h.slot, h.hud.clone())).collect(),
            award_slots: c
                .held
                .iter()
                .filter(|h| h.weapon.unlock_medal().is_some())
                .map(|h| h.slot)
                .collect(),
            held_models: c
                .held
                .iter()
                .map(|h| (h.slot, h.geometry.clone()))
                .collect(),
            first_person_held: c
                .held
                .iter()
                .map(|h| (h.slot, h.first_person.clone()))
                .collect(),
            ..Self::from_roster(c)
        }
    }

    /// Base roster projection. Callers add the held HUD/model entries they use.
    pub fn from_roster(c: &crate::roster::ClassRoster) -> Self {
        Self {
            name: c.class.clone(),
            label: c.label.clone(),
            geometry: c.geometry.clone(),
            weapon_geometry: c.weapon_geometry.clone(),
            weapon_hud: c.weapon_hud.clone(),
            held_hud: Vec::new(),
            award_slots: Vec::new(),
            droideka_shield: c.droideka_shield.clone(),
            held_models: Vec::new(),
            first_person: c.first_person.clone(),
            first_person_weapon: c.first_person_weapon.clone(),
            first_person_held: Vec::new(),
            flee_like_a_hero: c.flee_like_a_hero,
            foley_class: c.foley_class,
            min_foot_height: c.min_foot_height,
            cries: c.cries.clone(),
            anim: c.anim.clone(),
            skins: c.skins.clone(),
            jet_look: c.jet_look.clone(),
            from_bf2_side: c.from_bf2_side,
        }
    }

    /// What the HUD shows for the weapon of class slot `slot`: a held
    /// slot's own entry, else the primary's.
    pub fn hud_of(&self, slot: u8) -> &WeaponHud {
        self.held_hud
            .iter()
            .find(|(s, _)| *s == slot)
            .map_or(&self.weapon_hud, |(_, h)| h)
    }
}

/// Per-team unit template (from game::roster or the built-in defaults).
#[derive(Debug, Clone)]
pub struct TeamTemplate {
    /// Faction key for the HUD ("all" | "imp" | "rep" | "cis" | "team").
    pub faction: String,
    /// Optional custom team display name from v1.3 `SupportsCustomEraTeams` /
    /// `CustomEraTeam1` / `CustomEraTeam2`.
    pub display_name: Option<String>,
    pub name: String,
    pub consts: core::soldier::SoldierConstants,
    pub max_health: f32,
    pub health_type: core::weapon::HealthType,
    /// Index into `Sim::weapon_classes`.
    pub weapon_class: usize,
    pub total_clips: u32,
    /// The fill class's primary slot and its other weapon slots (card GREN),
    /// as its `Sim::classes` entry holds them: the template spawns the
    /// player's first unit.
    pub weapon_slot: u8,
    pub slots: std::sync::Arc<[core::unit::SlotSpec]>,
    /// The fill class's drop table (card PWUP).
    pub drops: core::sim::DropTable,
    /// Soldier model (class `GeometryName`), baked under
    /// `side_<game>/soldiers/<geometry>.glb`; `None` = marker box.
    pub geometry: Option<String>,
    /// Primary weapon model, baked under `side_<game>/weapons/<name>.glb`.
    pub weapon_geometry: Option<String>,
    /// Every class this team fields, index-aligned with `Sim::classes[team]`
    /// — `UnitSpec::class` indexes BOTH. Never empty: a template with no
    /// roster has one entry describing itself.
    pub classes: Vec<ClassVisual>,
}

impl TeamTemplate {
    /// Project roster metadata and its already-registered first class.
    /// Registration order and the visuals included remain the caller's policy.
    pub fn from_roster(
        roster: &crate::roster::FactionRoster,
        registered_fill: &core::unit::UnitClass,
        classes: Vec<ClassVisual>,
    ) -> Self {
        let c = roster.fill_class();
        Self {
            faction: roster.faction.clone(),
            display_name: None,
            name: c.class.clone(),
            consts: c.consts,
            max_health: c.max_health,
            health_type: c.health_type,
            weapon_class: registered_fill.weapon_class,
            weapon_slot: registered_fill.weapon_slot,
            slots: registered_fill.slots.clone(),
            drops: registered_fill.drops,
            total_clips: c.total_clips,
            geometry: c.geometry.clone(),
            weapon_geometry: c.weapon_geometry.clone(),
            classes,
        }
    }

    /// Built-in BF1 trooper with the BF1 rifle (Sim weapon class 0).
    pub fn default_trooper() -> Self {
        Self {
            faction: "team".into(),
            display_name: None,
            name: "trooper".into(),
            consts: core::soldier::SoldierConstants::default(),
            max_health: 300.0,
            health_type: core::weapon::HealthType::Person,
            weapon_class: 0,
            total_clips: 6,
            weapon_slot: 0,
            slots: std::sync::Arc::new([]),
            drops: core::sim::DropTable::NONE,
            geometry: None,
            weapon_geometry: None,
            classes: vec![ClassVisual {
                name: "trooper".into(),
                label: "Trooper".into(),
                ..ClassVisual::default()
            }],
        }
    }

    /// The class at `index`, clamped (the tables are never empty).
    pub fn class(&self, index: u8) -> &ClassVisual {
        let i = usize::from(index).min(self.classes.len().saturating_sub(1));
        &self.classes[i]
    }

    pub fn spec(
        &self,
        team: u8,
        pos: Vec3,
        yaw: f32,
        controller: Controller,
        class: u8,
    ) -> UnitSpec {
        UnitSpec {
            team,
            player_slot: match controller {
                Controller::Player { slot } => Some(slot),
                _ => None,
            },
            controller,
            pos,
            yaw,
            consts: self.consts,
            max_health: self.max_health,
            health_type: self.health_type,
            weapon_class: self.weapon_class,
            total_clips: self.total_clips,
            weapon_slot: self.weapon_slot,
            slots: self.slots.clone(),
            capture_posts: true,
            drops: self.drops,
            class,
            // `Sim::spawn` overwrites this from `Sim::difficulty` for every
            // `Controller::Ai` unit (D2) — the template does not know the tier.
            skill: 0,
            // No class in the game layer builds a jet trooper yet (PLAN
            // "PHASE A.2 DESIGN" decision 4): the flag exists so the planning
            // graph can keep `jet_jump` arcs off a jetless soldier.
            jet: false,
            droideka: None,
        }
    }
}

/// Which of the five per-faction value rows a team's faction key reads
/// (`game::layout::PROP_VALUE_ATK`'s order: Alliance, Empire, Republic, CIS,
/// Locals). The roster's own keys are the three-letter ones (`all`/`imp`/
/// `rep`/`cis`, `game::roster::default_factions`); anything else — the
/// built-in `default_trooper`'s "team", a mod faction, a mission that names
/// something we do not know — is LOCALS, which is what the disc's own
/// "everyone else" row is for.
pub fn faction_row(faction: &str) -> usize {
    match faction.to_ascii_lowercase().as_str() {
        "all" | "alliance" => 0,
        "imp" | "empire" => 1,
        "rep" | "republic" => 2,
        "cis" => 3,
        _ => 4,
    }
}
