//! Player profile: settings + career stats, autosaved everywhere (PLAN §Saves,
//! sessions & host authority: "each user profile autosaves continuously").
//! The profile catalog stores each player separately. Cheats remain session-scoped.
//!
//! Location: `$OPENBF_PROFILE` (single-file tests/dev override) else `profiles.ron` in
//! [`data_dir`], `openbf_data/` beside the game file (the maintainer, 2026-10-21: on every
//! OS, nothing in %APPDATA% or ~/.config).
//! Saves are atomic (write `.tmp`, rename).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod store;
pub use store::{valid_name_char, List, Store, NAME_LIMIT};

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// The settings version this build writes. A profile without the field is
/// version 0. 1 (2026-11-14): the keyboard and mouse defaults the maintainer
/// chose after the first LAN playtest ([`Settings::migrate`]).
pub const SETTINGS_VERSION: u32 = 1;

/// The mouse sensitivity a new profile starts at: 1.0, shown as 100 %. On
/// foot it turns as 2.8 did before 2026-11-14 (the playtester's own setting,
/// `kbm::ON_FOOT_LOOK_SCALE`). The look reads raw mouse counts
/// (`DeviceEvent::MouseMotion`, `kbm::Kbm::motion`), so the display scale and
/// the system pointer speed do not change it on Windows or Linux.
pub const MOUSE_SENSITIVITY_DEFAULT: f32 = 1.0;

// Keep a syntactically valid RON field intact until its typed parse completes:
// one malformed custom layout must not discard the rest of the profile.
fn bindings_or_default<'de, D, T>(
    deserializer: D,
    upgrade: fn(T) -> T,
    valid: fn(&T) -> bool,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let raw = Box::<ron::value::RawValue>::deserialize(deserializer)?;
    match ron::from_str::<T>(raw.get_ron()).map(upgrade) {
        Ok(bindings) if valid(&bindings) => Ok(bindings),
        _ => {
            eprintln!("profile: invalid control bindings; restoring their defaults");
            Ok(T::default())
        }
    }
}

// An icon choice from a newer build falls back to Auto; the rest of the profile stands.
fn input_icons_or_auto<'de, D>(deserializer: D) -> Result<crate::input_icons::IconChoice, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Box::<ron::value::RawValue>::deserialize(deserializer)?;
    Ok(ron::from_str(raw.get_ron()).unwrap_or_default())
}

/// Issue #172: the display mode as its variant name. A profile without one, or
/// with a mode this build does not know, opens fullscreen (Borderless); the rest
/// of the profile stands.
mod window_mode {
    use serde::{Deserialize, Deserializer, Serializer};
    use ui::menu::WindowMode;

    pub fn serialize<S: Serializer>(mode: &WindowMode, s: S) -> Result<S::Ok, S::Error> {
        let i = WindowMode::ALL.iter().position(|m| m == mode).unwrap_or(0);
        s.serialize_unit_variant("WindowMode", i as u32, name(*mode))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<WindowMode, D::Error> {
        let raw = Box::<ron::value::RawValue>::deserialize(d)?;
        let text = raw.get_ron().trim();
        Ok(WindowMode::ALL
            .into_iter()
            .find(|m| name(*m) == text)
            .unwrap_or(WindowMode::Borderless))
    }

    fn name(mode: WindowMode) -> &'static str {
        match mode {
            WindowMode::Windowed => "Windowed",
            WindowMode::Borderless => "Borderless",
            WindowMode::Exclusive => "Exclusive",
        }
    }
}

fn borderless() -> ui::menu::WindowMode {
    ui::menu::WindowMode::Borderless
}

fn keyboard_bindings<'de, D>(deserializer: D) -> Result<crate::kbm::Bindings, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bindings_or_default(
        deserializer,
        crate::kbm::Bindings::with_new_actions,
        crate::kbm::Bindings::is_valid,
    )
}

fn controller_bindings<'de, D>(deserializer: D) -> Result<input::Mapping, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bindings_or_default(
        deserializer,
        std::convert::identity,
        input::Mapping::is_valid,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// [`SETTINGS_VERSION`] when saved; 0 for a profile from before it.
    #[serde(default)]
    pub settings_version: u32,
    /// BF1 aim-assist intensity slider (1.0 = PS2-authentic).
    #[serde(default = "one", deserialize_with = "assist_intensity_setting")]
    pub assist_intensity: f32,
    /// Difficulty tier for the next round (PLAN §Saves: host authority). The
    /// `--difficulty` CLI flag WINS over this for the session it is given in.
    #[serde(default)]
    pub difficulty: core::skill::Difficulty,
    /// Aim assist on/off. **THE MAINTAINER'S RULE (2026-09-03): no difficulty tier ever
    /// turns this off.** The PS2 BF2's difficulty screen clears `bAutoAim`,
    /// `bAimAssist` and `bStickyReticule` when Elite is picked
    /// (`analysis/re-ai-difficulty.md` §9.3) — that is HISTORY here, not
    /// behaviour: the port's assist is the player's own switch at every tier.
    #[serde(default = "yes")]
    pub aim_assist: bool,
    /// Host option: the Elite team handicap (D12).
    #[serde(default = "yes")]
    pub team_handicap_on: bool,
    /// Use earned weapons and stat bonuses in ground battles.
    #[serde(default = "yes")]
    pub career_awards: bool,
    /// FOV option: a multiplier on the PS2 chase camera's vertical FOV (1.0 =
    /// the PS2's framing; `game::walk::FOV_SCALE_RANGE`). Hor+ either way.
    #[serde(default = "one")]
    pub fov_scale: f32,
    /// The music volume (card MUSV): a factor on the disc record's own gain,
    /// `game::music::VOLUME_RANGE`; 0 = no music.
    #[serde(default = "one")]
    pub music_volume: f32,
    #[serde(default = "one")]
    pub effects_volume: f32,
    #[serde(default = "one")]
    pub dialogue_volume: f32,
    /// 0 = off, 1 = on.
    #[serde(default)]
    pub fps_counter: u8,
    /// Issue #168: the LAN net meter overlay (F7 toggles it too).
    #[serde(default)]
    pub network_meter: bool,
    /// Controller feedback strength. Zero turns it off.
    #[serde(default = "one", deserialize_with = "rumble_setting")]
    pub rumble_strength: f32,
    #[serde(default)]
    pub rumble_on_fire: bool,
    /// 1.0 (100 %) is the default ([`MOUSE_SENSITIVITY_DEFAULT`]); the range is
    /// `ui::menu::MOUSE_SENSITIVITY_RANGE`.
    #[serde(default = "one")]
    pub mouse_sensitivity: f32,
    /// The controller's look sensitivity on the right stick
    /// (`ui::menu::PAD_LOOK_SENSITIVITY_RANGE`; 1 = the stick as it was).
    #[serde(default = "one", deserialize_with = "pad_look_setting")]
    pub pad_look_sensitivity: f32,
    #[serde(default)]
    pub invert_y: bool,
    /// Low Power Resolution: the world draws smaller when frames run long
    /// (`game::dyn_res`), the HUD at full size. Off by default.
    #[serde(default)]
    pub low_power_resolution: bool,
    /// Automatic quality (LAN §8b.2, `game::quality_governor`): the world's
    /// render scale follows the frame times. On by default; off keeps
    /// Low Power Resolution's choice.
    #[serde(default = "yes")]
    pub auto_quality: bool,
    #[serde(default)]
    pub terrain_lod: bool,
    /// Hard directional shadows from nearby soldiers, vehicles and scenery.
    #[serde(default = "yes")]
    pub hard_shadows: bool,
    #[serde(default)]
    pub scene_softness: bool,
    #[serde(default)]
    pub viewpoint: crate::viewpoint::Viewpoint,
    /// Issue #172: Windowed (maximized), Borderless or Exclusive fullscreen.
    /// The game launches fullscreen: a new profile starts Borderless, and so
    /// does a saved profile without the field (older than #172). Only a mode the
    /// player chose and saved (Windowed or Exclusive) is kept.
    /// Not part of Reset graphics: that never switches the display.
    #[serde(default = "borderless", with = "window_mode")]
    pub window_mode: ui::menu::WindowMode,
    /// The fullscreen kind the fullscreen key returns to from a window.
    #[serde(default = "borderless", with = "window_mode")]
    pub last_fullscreen: ui::menu::WindowMode,
    #[serde(default, deserialize_with = "keyboard_bindings")]
    pub keyboard_bindings: crate::kbm::Bindings,
    #[serde(default, deserialize_with = "controller_bindings")]
    pub controller_bindings: input::Mapping,
    #[serde(default)]
    pub keyboard_layouts: crate::control_layout::Layouts<crate::kbm::Trigger>,
    #[serde(default)]
    pub gamepad_layouts: crate::control_layout::Layouts<gilrs::Button>,
    #[serde(default)]
    pub flight_controls: input::FlightControls,
    /// Which icons the control prompts draw; Auto follows the device in use.
    #[serde(default, deserialize_with = "input_icons_or_auto")]
    pub input_icons: crate::input_icons::IconChoice,
    /// Map-relative population and reinforcement settings.
    #[serde(default = "hundred", deserialize_with = "bounded_team_pct")]
    pub team_size_pct: u32,
    #[serde(default = "hundred", deserialize_with = "percent_setting")]
    pub reinforcement_pct: u32,
    /// Issue #168: the LAN hosts last joined, newest first
    /// (`netplay::lobby::remember_host`).
    #[serde(default)]
    pub recent_hosts: Vec<String>,
}

impl Settings {
    /// Bring a profile saved by an older build up to [`SETTINGS_VERSION`],
    /// once.
    pub fn migrate(&mut self) {
        if self.settings_version < 1 {
            self.migrate_dev_v0();
        }
        self.settings_version = self.settings_version.max(SETTINGS_VERSION);
    }

    /// DEV-PHASE MIGRATION, slated for removal before v1 (the maintainer
    /// drops every backward-compatibility path at v1). Settings version 0 to
    /// 1 (2026-11-14):
    /// - a keyboard table that is exactly the old stock one, with no infantry
    ///   layout of its own, takes the new stock table (vehicles and
    ///   starfighters keep their controls, `control_layout::fallback`);
    /// - mouse sensitivity moves to the new scale, where on foot it is
    ///   multiplied by `kbm::ON_FOOT_LOOK_SCALE`: the old default 1.0 stays
    ///   1.0 (and takes the new, faster on-foot default), and any value the
    ///   player chose, v, becomes v / 2.8 so it turns on foot as before. In a
    ///   vehicle or starfighter (factor 1.0) a chosen value turns 2.8 times
    ///   slower than it did.
    fn migrate_dev_v0(&mut self) {
        if self.keyboard_bindings == crate::kbm::Bindings::pc_default_v0()
            && self.keyboard_layouts.infantry.is_empty()
        {
            self.keyboard_bindings = crate::kbm::Bindings::pc_default();
        }
        if self.mouse_sensitivity != 1.0 {
            let (lo, hi) = crate::kbm::SENSITIVITY_RANGE;
            self.mouse_sensitivity =
                (self.mouse_sensitivity / crate::kbm::ON_FOOT_LOOK_SCALE as f32).clamp(lo, hi);
        }
    }

    /// Issue #172: F11 switches fullscreen before any binding sees it, so a
    /// profile that bound it would lose that action without a word. The
    /// legacy keyboard layout moves the action to a free default key
    /// ([`crate::kbm::Bindings::free_fullscreen_key`]); a context layout
    /// keeps F11 on disk, its editor shows the action unassigned and asks
    /// for a key before it saves. Both are reported here. Returns the
    /// contexts whose layout binds F11.
    pub fn free_fullscreen_key(&mut self) -> Vec<ui::menu::ControlContext> {
        use crate::kbm::FULLSCREEN_KEY;
        use ui::menu::ControlContext;
        if let Some(to) = self.keyboard_bindings.free_fullscreen_key() {
            eprintln!(
                "profile: {}; its keyboard action moved to {}",
                crate::kbm::f11_note(crate::kbm::MAC),
                crate::control_layout::keyboard_name(to)
            );
        }
        let bound: Vec<_> = [
            ControlContext::Infantry,
            ControlContext::Vehicle,
            ControlContext::Starfighter,
        ]
        .into_iter()
        .filter(|&c| self.keyboard_layouts.get(c).contains(&FULLSCREEN_KEY))
        .collect();
        for c in &bound {
            eprintln!(
                "profile: {}; the {c:?} keyboard layout's F11 action is unassigned \
                 until it gets another key in Controls",
                crate::kbm::f11_note(crate::kbm::MAC)
            );
        }
        bound
    }

    pub fn reset_graphics(&mut self) {
        let defaults = Self::default();
        self.fov_scale = defaults.fov_scale;
        self.viewpoint = defaults.viewpoint;
        self.fps_counter = defaults.fps_counter;
        self.network_meter = defaults.network_meter;
        self.low_power_resolution = defaults.low_power_resolution;
        self.auto_quality = defaults.auto_quality;
        self.hard_shadows = defaults.hard_shadows;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: SETTINGS_VERSION,
            assist_intensity: 1.0,
            difficulty: core::skill::Difficulty::default(),
            aim_assist: true,
            team_handicap_on: true,
            career_awards: true,
            fov_scale: 1.0,
            music_volume: 1.0,
            effects_volume: 1.0,
            dialogue_volume: 1.0,
            fps_counter: 0,
            network_meter: false,
            rumble_strength: 1.0,
            rumble_on_fire: false,
            mouse_sensitivity: MOUSE_SENSITIVITY_DEFAULT,
            pad_look_sensitivity: 1.0,
            invert_y: false,
            low_power_resolution: false,
            terrain_lod: false,
            hard_shadows: true,
            scene_softness: false,
            auto_quality: true,
            viewpoint: crate::viewpoint::Viewpoint::default(),
            // The maintainer, 2026-11-14: a new profile, and a first launch,
            // play fullscreen; the fullscreen key leaves it maximized.
            window_mode: ui::menu::WindowMode::Borderless,
            last_fullscreen: ui::menu::WindowMode::Borderless,
            keyboard_bindings: crate::kbm::Bindings::default(),
            controller_bindings: input::Mapping::default(),
            keyboard_layouts: Default::default(),
            gamepad_layouts: Default::default(),
            flight_controls: input::FlightControls::default(),
            input_icons: crate::input_icons::IconChoice::default(),
            team_size_pct: 100,
            reinforcement_pct: 100,
            recent_hosts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Career {
    pub rounds: u32,
    pub wins: u32,
    pub kills: u32,
    pub deaths: u32,
    pub shots: u64,
    pub hits: u64,
    pub captures: u32,
    #[serde(default)]
    pub class_kills: std::collections::BTreeMap<String, u32>,
    /// BF2's points over every round (the Career Stats page, issue #50); a
    /// profile from before it starts at 0.
    #[serde(default)]
    pub points: i64,
    #[serde(default)]
    pub medals: core::sim::MedalCounts,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleStats {
    pub shots: u64,
    pub hits: u64,
    pub captures: u32,
    /// The round's points (`EventFeed::stats`).
    pub points: i32,
}

/// Where the game keeps what it writes (the profile, the playtest snapshots of
/// a packed build): `openbf_data/` beside the running game file, on every OS.
pub fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("openbf_data")))
        .unwrap_or_else(|| PathBuf::from("openbf_data"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    pub settings: Settings,
    pub career: Career,
    #[serde(skip)]
    slot: Option<store::Slot>,
}

/// A loaded mouse sensitivity, kept on the menu's row: an out-of-range or
/// non-finite value (a hand edit, or a value saved before the 100 % scale)
/// can never turn the view many times too fast.
fn clamp_mouse_sensitivity(value: f32) -> f32 {
    let (lo, hi) = crate::kbm::SENSITIVITY_RANGE;
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        MOUSE_SENSITIVITY_DEFAULT
    }
}

impl Profile {
    /// The single-file dev override (`$OPENBF_PROFILE`), when set. Players'
    /// profiles live in the catalog ([`Store`]); there is no other single
    /// file.
    pub fn path() -> Option<PathBuf> {
        std::env::var_os("OPENBF_PROFILE").map(PathBuf::from)
    }

    /// Load the `$OPENBF_PROFILE` profile, or a fresh default one (also when
    /// the file is unreadable — a corrupt profile must never block play; it
    /// is backed up).
    pub fn load_or_default() -> Profile {
        Self::path().map_or_else(Profile::fresh, |p| Self::load_from(&p))
    }

    /// Load from an explicit file, with the same recovery as the default path.
    pub fn load_from(path: &Path) -> Profile {
        match std::fs::read_to_string(path) {
            Ok(txt) => match ron::from_str::<Profile>(&txt) {
                Ok(mut p) => {
                    p.normalize();
                    p
                }
                Err(e) => {
                    eprintln!(
                        "profile {} unreadable ({e}); starting fresh",
                        path.display()
                    );
                    let _ = std::fs::rename(path, path.with_extension("ron.bad"));
                    Profile::fresh()
                }
            },
            Err(_) => Profile::fresh(),
        }
    }

    fn normalize(&mut self) {
        self.settings.migrate();
        self.settings.mouse_sensitivity = clamp_mouse_sensitivity(self.settings.mouse_sensitivity);
        self.settings.fov_scale = crate::walk::clamp_fov_scale(self.settings.fov_scale);
        self.settings.free_fullscreen_key();
    }

    fn fresh() -> Profile {
        Profile {
            name: std::env::var("USER").unwrap_or_else(|_| "player".into()),
            ..Default::default()
        }
    }

    /// Atomic save: to the profile's catalog slot, else to the
    /// `$OPENBF_PROFILE` dev file. A profile with neither (the shell's
    /// default before a profile is picked) has nowhere to go, so nothing is
    /// written.
    pub fn save(&self) -> Result<()> {
        self.save_with(Self::path().as_deref())
    }

    /// [`Profile::save`] with the dev override passed in.
    fn save_with(&self, dev: Option<&Path>) -> Result<()> {
        match (&self.slot, dev) {
            (Some(slot), _) => slot.save(self),
            (None, Some(path)) => self.save_to(path),
            (None, None) => Ok(()),
        }
    }

    /// The profile saves somewhere ([`Profile::save`] writes a file).
    pub fn persists(&self) -> bool {
        self.slot.is_some() || Self::path().is_some()
    }

    /// Atomically save to an explicit file.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        write_ron(self, path)
    }

    /// Fold a finished (or abandoned) round's player stats into the career.
    pub fn record_round(
        &mut self,
        won: Option<bool>,
        kills: u32,
        deaths: u32,
        shots: u64,
        hits: u64,
        captures: u32,
    ) {
        self.career.rounds += 1;
        if won == Some(true) {
            self.career.wins += 1;
        }
        self.career.kills += kills;
        self.career.deaths += deaths;
        self.career.shots += shots;
        self.career.hits += hits;
        self.career.captures += captures;
    }
}

fn write_ron(value: &impl Serialize, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
    }
    let txt = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())?;
    let tmp = path.with_extension("ron.tmp");
    std::fs::write(&tmp, txt).with_context(|| tmp.display().to_string())?;
    std::fs::rename(&tmp, path).with_context(|| path.display().to_string())?;
    Ok(())
}

fn bounded_team_pct<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    use serde::Deserialize;
    u32::deserialize(d).map(|n| n.clamp(10, ui::population::MAX_TEAM_PCT))
}

fn percent_setting<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    u32::deserialize(d).map(|n| n.clamp(10, 500))
}

fn hundred() -> u32 {
    100
}

fn assist_intensity_setting<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    f32::deserialize(d).map(clamp_assist_intensity)
}

pub fn clamp_assist_intensity(value: f32) -> f32 {
    if value.is_nan() {
        return 1.0;
    }
    value.clamp(0.0, 1.0)
}

fn pad_look_setting<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    f32::deserialize(d).map(clamp_pad_look)
}

fn rumble_setting<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    f32::deserialize(d).map(|v| ui::menu::RUMBLE_LEVELS[ui::menu::rumble_level(v)].0)
}

/// A look sensitivity held to its range; a broken value reads as 1.
pub fn clamp_pad_look(v: f32) -> f32 {
    let (lo, hi) = ui::menu::PAD_LOOK_SENSITIVITY_RANGE;
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A2: a profile with no catalog slot (the shell's default before a
    /// profile is picked, where F11 on the title screen used to save) writes
    /// no single-file `profile.ron` beside the game, and leaves one that is
    /// already there untouched.
    #[test]
    fn an_unslotted_profile_saves_nowhere_and_leaves_profile_ron_alone() {
        // `save_with(None)` is `save()` without the `OPENBF_PROFILE` dev
        // override, so the test holds in a shell that exports it.
        let legacy = data_dir().join("profile.ron");
        let blank = Profile::default();
        assert!(blank.slot.is_none());
        let _ = std::fs::remove_file(&legacy);
        blank.save_with(None).unwrap();
        assert!(
            !legacy.exists(),
            "a blank profile was written to {}",
            legacy.display()
        );
        std::fs::create_dir_all(data_dir()).unwrap();
        std::fs::write(&legacy, "(name: \"Tester\")").unwrap();
        blank.save_with(None).unwrap();
        let kept = std::fs::read_to_string(&legacy).unwrap();
        let _ = std::fs::remove_file(&legacy);
        assert_eq!(kept, "(name: \"Tester\")");
    }

    /// The dev override still saves the single file it names.
    #[test]
    fn the_dev_override_still_saves_its_own_file() {
        let dir = std::env::temp_dir().join(format!("openbf-dev-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dev.ron");
        let p = Profile {
            name: "dev".into(),
            ..Default::default()
        };
        p.save_with(Some(&path)).unwrap();
        let back = Profile::load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back.name, "dev");
    }

    #[test]
    fn render_options_use_legacy_defaults_and_survive_profile_roundtrips() {
        let old: Settings = ron::from_str("(assist_intensity: 0.7)").unwrap();
        assert!(!old.terrain_lod && !old.scene_softness);
        assert!(!Settings::default().terrain_lod && !Settings::default().scene_softness);
        assert!(old.hard_shadows && Settings::default().hard_shadows);
        for terrain_lod in [false, true] {
            for scene_softness in [false, true] {
                let settings = Settings {
                    terrain_lod,
                    scene_softness,
                    ..Default::default()
                };
                let back: Settings = ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
                assert_eq!(
                    (back.terrain_lod, back.scene_softness),
                    (terrain_lod, scene_softness)
                );
            }
        }
        let mut settings = Settings {
            hard_shadows: false,
            ..Default::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
        assert!(!back.hard_shadows);
        settings.reset_graphics();
        assert!(settings.hard_shadows);
    }

    #[test]
    fn loaded_aim_assist_intensity_stays_in_range() {
        for (value, expected) in [("1.5", 1.0), ("-0.1", 0.0), ("0.7", 0.7), ("NaN", 1.0)] {
            let settings: Settings =
                ron::from_str(&format!("(assist_intensity: {value})")).unwrap();
            assert_eq!(settings.assist_intensity, expected);
        }
    }

    #[test]
    fn an_older_profile_keeps_aggregate_stats_and_starts_with_no_class_kills() {
        let profile: Profile = ron::from_str(
            r#"(name:"legacy", settings:(assist_intensity:0.7),
                career:(rounds:3,wins:2,kills:12,deaths:5,shots:200,hits:80,captures:4))"#,
        )
        .unwrap();
        assert_eq!(
            profile.career,
            Career {
                rounds: 3,
                wins: 2,
                kills: 12,
                deaths: 5,
                shots: 200,
                hits: 80,
                captures: 4,
                class_kills: Default::default(),
                points: 0,
                medals: Default::default(),
            }
        );
        assert_eq!(profile.settings.assist_intensity, 0.7);
    }

    /// The input icon choice round-trips; a profile from before it, or one
    /// holding a choice this build does not know, loads as Auto and keeps
    /// its other settings.
    #[test]
    fn the_input_icon_choice_is_kept_and_never_costs_the_profile() {
        use crate::input_icons::IconChoice;
        let s = Settings {
            input_icons: IconChoice::XboxOriginal,
            ..Default::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.input_icons, IconChoice::XboxOriginal);
        let old: Settings = ron::from_str("(assist_intensity: 0.7)").unwrap();
        assert_eq!(old.input_icons, IconChoice::Auto);
        let newer: Settings =
            ron::from_str("(assist_intensity: 0.7, input_icons: SteamDeck)").unwrap();
        assert_eq!(newer.input_icons, IconChoice::Auto);
        assert_eq!(newer.assist_intensity, 0.7);
        // The removed dots set's names, should any profile or tool have saved them: Auto.
        for gone in ["GenericPad", "Generic"] {
            let old: Settings =
                ron::from_str(&format!("(assist_intensity: 0.7, input_icons: {gone})")).unwrap();
            assert_eq!(old.input_icons, IconChoice::Auto, "{gone}");
            assert_eq!(old.assist_intensity, 0.7, "{gone}");
        }
        // Any shape of value, and inside a whole profile as the game loads it.
        for value in ["Pad(1, \"x\")", "3", "[Ds4]", "\"Ds4\""] {
            let whole: Profile = ron::from_str(&format!(
                "(name: \"p\", settings: (assist_intensity: 0.7, input_icons: {value}),
                  career: (rounds: 0, wins: 0, kills: 0, deaths: 0, shots: 0, hits: 0, captures: 0))"
            ))
            .unwrap_or_else(|e| panic!("{value}: {e}"));
            assert_eq!(whole.settings.input_icons, IconChoice::Auto, "{value}");
            assert_eq!(whole.settings.assist_intensity, 0.7, "{value}");
        }
    }

    /// Issue #172: a legacy keyboard layout that bound F11 moves that action
    /// to a free default key on load and stays valid; others are untouched.
    #[test]
    fn a_legacy_f11_binding_moves_to_a_free_key() {
        use crate::kbm::{Bindings, FULLSCREEN_KEY};
        let mut s = Settings::default();
        assert!(s.free_fullscreen_key().is_empty());
        assert_eq!(s.keyboard_bindings, Bindings::default());
        let was = s.keyboard_bindings.table[0].0;
        s.keyboard_bindings.table[0].0 = FULLSCREEN_KEY;
        assert!(s.keyboard_bindings.is_valid());
        assert!(s.free_fullscreen_key().is_empty());
        assert_eq!(
            s.keyboard_bindings.table[0].0, was,
            "its own default was free"
        );
        assert!(s.keyboard_bindings.is_valid());
        assert!(!s
            .keyboard_bindings
            .table
            .iter()
            .any(|(t, _)| *t == FULLSCREEN_KEY));
    }

    /// The maintainer, 2026-11-14: a new profile plays fullscreen
    /// (Borderless), and F11 takes it back to Borderless after a window.
    #[test]
    fn a_new_profile_starts_fullscreen() {
        use ui::menu::WindowMode;
        let s = Settings::default();
        assert_eq!(s.window_mode, WindowMode::Borderless);
        assert_eq!(s.last_fullscreen, WindowMode::Borderless);
        assert_eq!(
            Profile::default().settings.window_mode,
            WindowMode::Borderless
        );
    }

    /// Issue #172: the display mode and the last fullscreen kind round-trip;
    /// a profile from before them launches fullscreen, a saved Windowed stays
    /// windowed, and a mode this build does not know costs nothing else.
    #[test]
    fn the_window_mode_round_trips_and_older_profiles_open_fullscreen() {
        use ui::menu::WindowMode;
        for mode in WindowMode::ALL {
            let s = Settings {
                window_mode: mode,
                last_fullscreen: WindowMode::Exclusive,
                ..Default::default()
            };
            let text = ron::to_string(&s).unwrap();
            let back: Settings = ron::from_str(&text).unwrap();
            assert_eq!(back.window_mode, mode, "{text}");
            assert_eq!(back.last_fullscreen, WindowMode::Exclusive);
        }
        let old: Settings = ron::from_str("(assist_intensity: 0.7)").unwrap();
        assert_eq!(old.window_mode, WindowMode::Borderless);
        assert_eq!(old.last_fullscreen, WindowMode::Borderless);
        assert_eq!(old.assist_intensity, 0.7);
        let named: Settings =
            ron::from_str("(assist_intensity: 0.7, window_mode: Borderless)").unwrap();
        assert_eq!(named.window_mode, WindowMode::Borderless);
        let chosen: Settings =
            ron::from_str("(assist_intensity: 0.7, window_mode: Windowed)").unwrap();
        assert_eq!(chosen.window_mode, WindowMode::Windowed);
        for value in ["Kiosk", "3", "\"Borderless\"", "Exclusive(0)"] {
            let newer: Settings =
                ron::from_str(&format!("(assist_intensity: 0.7, window_mode: {value})"))
                    .unwrap_or_else(|e| panic!("{value}: {e}"));
            assert_eq!(newer.window_mode, WindowMode::Borderless, "{value}");
            assert_eq!(newer.assist_intensity, 0.7, "{value}");
        }
        let mut reset = Settings {
            window_mode: WindowMode::Borderless,
            ..Default::default()
        };
        reset.reset_graphics();
        assert_eq!(reset.window_mode, WindowMode::Borderless);
    }

    /// Every name a profile may hold loads as the row it names today
    /// (the maintainer, 2026-10-28: Easy 14, Normal 17, Hard 20, Elite 20+extras):
    /// "easy" Easy, "normal" Normal, "hard" Hard, "elite" Elite (an "elite"
    /// saved before 2026-10-28 meant the 20+extras row and still does) and
    /// "legendary", that row's name for part of 2026-10-28, Elite too.
    #[test]
    fn every_saved_tier_name_loads_as_its_row() {
        use core::skill::Difficulty;
        let at = |name: &str| {
            ron::from_str::<Settings>(&format!("(assist_intensity: 1.0, difficulty: {name})"))
                .unwrap()
                .difficulty
        };
        assert_eq!(at("easy"), Difficulty::Easy);
        assert_eq!(at("normal"), Difficulty::Normal);
        assert_eq!(at("hard"), Difficulty::Hard);
        assert_eq!(at("elite"), Difficulty::Elite);
        assert_eq!(at("legendary"), Difficulty::Elite);
        assert_eq!(at("normal").base_skill(), 17);
        assert_eq!(at("elite").tier().err_cap, Some(0.9), "20+extras");
        let s = Settings {
            difficulty: Difficulty::Elite,
            ..Default::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.difficulty, Difficulty::Elite);
    }

    /// Issue #142: a profile from before the setting reads 1; a value out of
    /// range is held to it; a saved value comes back.
    #[test]
    fn the_pad_look_sensitivity_loads_in_range() {
        let old: Settings = ron::from_str("(assist_intensity: 0.5)").unwrap();
        assert_eq!(old.pad_look_sensitivity, 1.0);
        let high: Settings =
            ron::from_str("(assist_intensity: 0.5, pad_look_sensitivity: 99.0)").unwrap();
        assert_eq!(high.pad_look_sensitivity, 1.5);
        let low: Settings =
            ron::from_str("(assist_intensity: 0.5, pad_look_sensitivity: 0.0)").unwrap();
        assert_eq!(low.pad_look_sensitivity, 0.5);
        let s = Settings {
            pad_look_sensitivity: 0.7,
            ..Default::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.pad_look_sensitivity, 0.7);
    }

    #[test]
    fn an_older_profile_reads_the_mouse_defaults() {
        let s: Settings = ron::from_str("(assist_intensity: 0.5, fov_scale: 1.1)").unwrap();
        assert_eq!((s.mouse_sensitivity, s.invert_y), (1.0, false));
        assert_eq!(s.assist_intensity, 0.5);
        assert_eq!(s.keyboard_bindings, crate::kbm::Bindings::default());
        assert_eq!(s.controller_bindings, input::Mapping::default());
    }

    /// `settings` as a build from before [`SETTINGS_VERSION`] saved it, read
    /// back as the game loads a profile.
    #[test]
    fn a_loaded_mouse_sensitivity_stays_on_the_slider() {
        let (lo, hi) = crate::kbm::SENSITIVITY_RANGE;
        for (saved, want) in [
            (2.8, hi),
            (9.0, hi),
            (0.0, lo),
            (-1.0, lo),
            (f32::NAN, 1.0),
            (0.65, 0.65),
        ] {
            let mut p = Profile {
                name: "p".into(),
                settings: Settings {
                    mouse_sensitivity: saved,
                    ..Default::default()
                },
                ..Default::default()
            };
            p.normalize();
            assert_eq!(p.settings.mouse_sensitivity, want, "saved {saved}");
        }
    }

    fn saved_by_an_older_build(settings: Settings) -> Profile {
        let profile = Profile {
            name: "p".into(),
            settings,
            ..Default::default()
        };
        let text = ron::to_string(&profile).unwrap();
        let old = text.replacen("settings_version:1,", "", 1);
        assert_ne!(old, text, "the fixture drops the version");
        let mut loaded: Profile = ron::from_str(&old).unwrap();
        assert_eq!(loaded.settings.settings_version, 0);
        loaded.normalize();
        loaded
    }

    fn old_stock() -> Settings {
        Settings {
            keyboard_bindings: crate::kbm::Bindings::pc_default_v0(),
            mouse_sensitivity: 1.0,
            ..Default::default()
        }
    }

    /// 2026-11-14: a profile that kept the old stock keyboard and the old
    /// 1.0 sensitivity takes the new defaults once (1.0 now reads 100 % and
    /// turns on foot as 2.8 did), and keeps them.
    #[test]
    fn an_untouched_older_profile_takes_the_new_keyboard_and_mouse_defaults() {
        use crate::kbm::Bindings;
        let p = saved_by_an_older_build(old_stock());
        assert_eq!(p.settings.keyboard_bindings, Bindings::pc_default());
        assert_eq!(p.settings.mouse_sensitivity, 1.0);
        assert_eq!(p.settings.settings_version, SETTINGS_VERSION);
        assert_eq!(p.settings, Settings::default());
        // Saved now, it round-trips and never migrates again.
        let text = ron::to_string(&p).unwrap();
        let mut again: Profile = ron::from_str(&text).unwrap();
        again.normalize();
        assert_eq!(again, p);
        // A version-1 player's choice is never rescaled again.
        let mut chosen = p.clone();
        chosen.settings.mouse_sensitivity = 0.5;
        let mut back: Profile = ron::from_str(&ron::to_string(&chosen).unwrap()).unwrap();
        back.normalize();
        assert_eq!(back.settings.mouse_sensitivity, 0.5);
    }

    /// 2026-11-14: a remapped keyboard or a layout of his own stays exactly as
    /// saved, the old table's shape stays valid, and a sensitivity he set
    /// keeps its on-foot feel on the new scale (v / 2.8).
    #[test]
    fn a_customised_older_profile_keeps_its_controls() {
        use crate::kbm::{Bindings, Trigger};
        use winit::keyboard::KeyCode;
        let mut remapped = old_stock();
        assert!(remapped
            .keyboard_bindings
            .remap(0, Trigger::Key(KeyCode::KeyP)));
        assert!(remapped.keyboard_bindings.is_valid());
        remapped.mouse_sensitivity = 1.7;
        let p = saved_by_an_older_build(remapped.clone());
        assert_eq!(p.settings.keyboard_bindings, remapped.keyboard_bindings);
        let feel = |v: f32| f64::from(v) * crate::kbm::ON_FOOT_LOOK_SCALE;
        assert!((feel(p.settings.mouse_sensitivity) - 1.7).abs() < 1e-5);
        // The stock table with a vehicle layout of its own still migrates;
        // the vehicle layout is kept.
        let mut vehicle = old_stock();
        let layout: Vec<_> = crate::control_layout::keyboard_defaults(
            ui::menu::ControlContext::Vehicle,
            &Bindings::pc_default_v0(),
        )
        .into_iter()
        .flatten()
        .collect();
        vehicle
            .keyboard_layouts
            .set(ui::menu::ControlContext::Vehicle, layout.clone());
        let p = saved_by_an_older_build(vehicle);
        assert_eq!(p.settings.keyboard_bindings, Bindings::pc_default());
        assert_eq!(p.settings.keyboard_layouts.vehicle, layout);
    }

    /// 2026-11-14: the playtester's own profile (the old stock table, his
    /// infantry layout with the thumb buttons, 2.8): his layout and table load
    /// as saved, his 2.8 reads 1.0 (100 %) with the same on-foot feel, and his
    /// vehicles keep the old table.
    #[test]
    fn the_playtesters_profile_is_untouched() {
        use crate::control_layout as layout;
        use crate::kbm::{Bindings, Trigger};
        use ui::menu::ControlContext;
        use winit::event::MouseButton as M;
        use winit::keyboard::KeyCode as K;
        use Trigger::{Key, Mouse};
        let infantry = vec![
            Key(K::KeyW),
            Key(K::KeyS),
            Key(K::KeyA),
            Key(K::KeyD),
            Mouse(M::Left),
            Mouse(M::Back),
            Mouse(M::Right),
            Key(K::Space),
            Key(K::ControlLeft),
            Key(K::ShiftLeft),
            Key(K::KeyR),
            Key(K::KeyE),
            Trigger::WheelDown,
            Mouse(M::Forward),
            Key(K::ArrowUp),
            Key(K::ArrowDown),
            Key(K::ArrowLeft),
            Key(K::ArrowRight),
            Key(K::KeyV),
        ];
        let mut his = Settings {
            keyboard_bindings: Bindings::pc_default_v0(),
            mouse_sensitivity: 2.8,
            ..Default::default()
        };
        his.keyboard_layouts
            .set(ControlContext::Infantry, infantry.clone());
        let p = saved_by_an_older_build(his.clone());
        his.settings_version = SETTINGS_VERSION;
        his.mouse_sensitivity = 1.0;
        assert_eq!(p.settings, his);
        let s = &p.settings;
        assert!(layout::keyboard(ControlContext::Infantry, &s.keyboard_layouts.infantry).is_some());
        for context in [ControlContext::Vehicle, ControlContext::Starfighter] {
            assert_eq!(
                layout::fallback(Some(context), &s.keyboard_bindings),
                Bindings::pc_default_v0()
            );
        }
        // The new infantry default is his layout with Middle mouse and G in
        // place of the thumb buttons, so it works on any mouse.
        let stock: Vec<_> =
            layout::keyboard_defaults(ControlContext::Infantry, &Bindings::default())
                .into_iter()
                .flatten()
                .collect();
        let mut want = infantry.clone();
        want[5] = Mouse(M::Middle);
        want[13] = Key(K::KeyG);
        assert_eq!(stock, want);
    }

    #[test]
    fn career_awards_default_on_and_an_off_profile_keeps_its_medals() {
        let settings: Settings = ron::from_str("(assist_intensity: 1.0)").unwrap();
        assert!(settings.career_awards);

        let mut profile = Profile::default();
        profile.settings.career_awards = false;
        profile.career.medals[1] = 64;
        let saved = ron::to_string(&profile).unwrap();
        assert_eq!(ron::from_str::<Profile>(&saved).unwrap(), profile);
    }

    #[test]
    fn rc_flight_controls_round_trip_and_old_settings_choose_classic() {
        use input::FlightControls;
        let mut settings: Settings = ron::from_str("(assist_intensity: 0.5)").unwrap();
        assert_eq!(settings.flight_controls, FlightControls::Classic);
        settings.flight_controls = FlightControls::RcMode2;
        let profile = Profile {
            settings,
            ..Default::default()
        };
        let saved = ron::to_string(&profile).unwrap();
        assert_eq!(ron::from_str::<Profile>(&saved).unwrap(), profile);
    }

    #[test]
    fn remapped_controls_survive_serialization() {
        let mut settings = Settings::default();
        settings
            .keyboard_bindings
            .remap(0, crate::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyP));
        settings.controller_bindings.remap(0, gilrs::Button::West);
        let encoded = ron::to_string(&settings).unwrap();
        let decoded: Settings = ron::from_str(&encoded).unwrap();
        assert_eq!(decoded, settings);
        assert_ne!(decoded.keyboard_bindings, crate::kbm::Bindings::default());
        assert_ne!(decoded.controller_bindings, input::Mapping::default());
    }

    #[test]
    fn malformed_binding_fields_preserve_progress_settings_and_the_other_device() {
        let mut original = Profile {
            name: "retained player".into(),
            ..Default::default()
        };
        original.record_round(Some(true), 12, 3, 240, 90, 2);
        original.settings.mouse_sensitivity = 1.7;
        original.settings.invert_y = true;
        original
            .settings
            .keyboard_bindings
            .remap(0, crate::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyP));
        original
            .settings
            .controller_bindings
            .remap(0, gilrs::Button::West);
        let saved = ron::to_string(&original).unwrap();
        for (field, value) in [
            (
                "keyboard_bindings",
                ron::to_string(&original.settings.keyboard_bindings).unwrap(),
            ),
            (
                "controller_bindings",
                ron::to_string(&original.settings.controller_bindings).unwrap(),
            ),
        ] {
            let mut expected = original.clone();
            if field == "keyboard_bindings" {
                expected.settings.keyboard_bindings = Default::default();
            } else {
                expected.settings.controller_bindings = Default::default();
            }
            for bad in [
                "false",
                "(table:false)",
                "(table:[])",
                "(table:[(NoSuchControl,0)])",
            ] {
                let damaged =
                    saved.replacen(&format!("{field}:{value}"), &format!("{field}:{bad}"), 1);
                assert_ne!(damaged, saved, "fixture did not replace {field}");
                let repaired: Profile = ron::from_str(&damaged).unwrap();
                assert_eq!(repaired, expected, "{field}: {bad}");
            }
        }
    }

    #[test]
    fn invalid_saved_controller_actions_and_conflicts_restore_only_that_layout() {
        let valid = input::Mapping::default();
        let mut duplicate = valid.clone();
        duplicate.table[1].0 = duplicate.table[0].0;
        let mut unknown_action = valid.clone();
        unknown_action.table[0].1 |= core::sim::buttons::RESPAWN;
        let mut repeated_action = valid.clone();
        repeated_action.table[1].1 = repeated_action.table[0].1;
        let mut unknown_button = valid.clone();
        unknown_button.table[0].0 = gilrs::Button::Unknown;
        let mut reserved = valid.clone();
        reserved.table[0].0 = gilrs::Button::Start;
        for mapping in [
            duplicate,
            unknown_action,
            repeated_action,
            unknown_button,
            reserved,
        ] {
            let mut settings = Settings::default();
            settings
                .keyboard_bindings
                .remap(0, crate::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyP));
            settings.controller_bindings = mapping;
            let loaded: Settings = ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
            assert_eq!(loaded.controller_bindings, valid);
            assert_eq!(loaded.keyboard_bindings, settings.keyboard_bindings);
        }
    }

    #[test]
    fn reserved_saved_controls_repair_only_their_layout_and_preserve_career() {
        let mut profile = Profile::default();
        profile.record_round(Some(true), 12, 3, 240, 90, 2);
        profile
            .settings
            .keyboard_bindings
            .remap(0, crate::kbm::Trigger::Key(winit::keyboard::KeyCode::KeyP));
        profile
            .settings
            .controller_bindings
            .remap(0, gilrs::Button::West);
        for keyboard in [false, true] {
            let mut bad = profile.clone();
            let mut expected = profile.clone();
            if keyboard {
                bad.settings.keyboard_bindings.table[0].0 =
                    crate::kbm::Trigger::Key(winit::keyboard::KeyCode::Escape);
                expected.settings.keyboard_bindings = Default::default();
            } else {
                bad.settings.controller_bindings.table[0].0 = gilrs::Button::Start;
                expected.settings.controller_bindings = Default::default();
            }
            let repaired: Profile = ron::from_str(&ron::to_string(&bad).unwrap()).unwrap();
            assert_eq!(repaired, expected);
        }
    }

    #[test]
    fn round_trips_and_survives_corruption() {
        let dir = std::env::temp_dir().join(format!("openbf-profile-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("profile.ron");
        let mut p = Profile::load_from(&path);
        assert_eq!(p.career, Career::default());
        p.settings.assist_intensity = 0.7;
        p.record_round(Some(true), 12, 3, 240, 90, 2);
        p.career.class_kills.insert("all_inf_officer".into(), 7);
        p.career.class_kills.insert("all_inf_wookiee".into(), 5);
        p.career.medals[4] = 32;
        p.settings.career_awards = false;
        p.save_to(&path).expect("save");
        let back = Profile::load_from(&path);
        assert_eq!(back, p);
        assert_eq!(back.career.wins, 1);
        // Corrupt file → fresh profile, original kept as .bad.
        std::fs::write(&path, "(not ron").unwrap();
        let fresh = Profile::load_from(&path);
        assert_eq!(fresh.career, Career::default());
        assert!(path.with_extension("ron.bad").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn a_profile_saved_with_the_removed_last_battle_fields_still_loads() {
        let serialized = ron::to_string(&super::Settings::default()).unwrap();
        assert!(serialized.ends_with(')'));
        let old = format!(
            "{},last_map:Some(\"tat2\"),last_mode:Some(\"con\"),last_teams:Some((\"cis\",\"rep\")))",
            serialized[..serialized.len() - 1].trim_end_matches(',')
        );
        let loaded: super::Settings = ron::from_str(&old).unwrap();
        assert_eq!(loaded, super::Settings::default());
        assert!(!ron::to_string(&loaded).unwrap().contains("last_map"));
    }
    #[test]
    fn saved_percentages_are_bounded_before_the_menu_reads_them() {
        let serialized = ron::to_string(&super::Settings::default()).unwrap();
        assert!(serialized.contains("team_size_pct:100,"));
        assert!(serialized.contains("reinforcement_pct:100,"));
        let extreme = serialized
            .replace("team_size_pct:100", "team_size_pct:0")
            .replace("reinforcement_pct:100", "reinforcement_pct:4294967295");
        let loaded: super::Settings = ron::from_str(&extreme).unwrap();
        assert_eq!((loaded.team_size_pct, loaded.reinforcement_pct), (10, 500));
        let high = serialized.replace("team_size_pct:100", "team_size_pct:500");
        let loaded: super::Settings = ron::from_str(&high).unwrap();
        assert_eq!(loaded.team_size_pct, ui::population::MAX_TEAM_PCT);
    }

    /// Issue #168: the recent LAN hosts and the player name survive a save
    /// and a load; a profile from before them loads with none.
    #[test]
    fn recent_hosts_and_the_player_name_persist() {
        let dir = std::env::temp_dir().join(format!("openbf_recent_{}", std::process::id()));
        let path = dir.join("profile.ron");
        let mut p = Profile {
            name: "Ann".into(),
            ..Default::default()
        };
        crate::netplay::lobby::remember_host(&mut p.settings.recent_hosts, "10.0.0.2");
        crate::netplay::lobby::remember_host(&mut p.settings.recent_hosts, "studio-pc:40000");
        p.save_to(&path).unwrap();
        let back = Profile::load_from(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(back.name, "Ann");
        assert_eq!(back.settings.recent_hosts, ["studio-pc:40000", "10.0.0.2"]);
        let old: Settings = ron::from_str("(assist_intensity: 0.5)").unwrap();
        assert!(old.recent_hosts.is_empty());
    }
}
