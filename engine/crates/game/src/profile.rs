//! Player profile: settings + career stats, autosaved everywhere (PLAN §Saves,
//! sessions & host authority: "each user profile autosaves continuously").
//! v0 = one RON file; awards/unlock progression and per-mode stats grow in
//! here (P8). Cheats are deliberately NOT persisted (session-scoped by design).
//!
//! Location: `$OPENBF_PROFILE` (tests/dev override) else `profile.ron` in
//! [`data_dir`], `openbf_data/` beside the game file (the maintainer, 2026-10-21: on every
//! OS, nothing in %APPDATA% or ~/.config).
//! Saves are atomic (write `.tmp`, rename).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

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
    /// BF1 aim-assist intensity slider (1.0 = PS2-authentic).
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
    /// FOV option: a multiplier on the PS2 chase camera's vertical FOV (1.0 =
    /// the PS2's framing; `game::walk::FOV_SCALE_RANGE`). Hor+ either way.
    #[serde(default = "one")]
    pub fov_scale: f32,
    /// The music volume (card MUSV): a factor on the disc record's own gain,
    /// `game::music::VOLUME_RANGE`; 0 = no music.
    #[serde(default = "one")]
    pub music_volume: f32,
    #[serde(default = "one")]
    pub mouse_sensitivity: f32,
    #[serde(default)]
    pub invert_y: bool,
    #[serde(default)]
    pub viewpoint: crate::viewpoint::Viewpoint,
    #[serde(default, deserialize_with = "keyboard_bindings")]
    pub keyboard_bindings: crate::kbm::Bindings,
    #[serde(default, deserialize_with = "controller_bindings")]
    pub controller_bindings: input::Mapping,
    /// Map-relative population and reinforcement settings. Legacy absolute caps are ignored.
    #[serde(default = "hundred", deserialize_with = "bounded_team_pct")]
    pub team_size_pct: u32,
    #[serde(default = "hundred", deserialize_with = "percent_setting")]
    pub reinforcement_pct: u32,
    pub last_map: Option<String>,
    pub last_mode: Option<String>,
    pub last_teams: Option<[String; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            assist_intensity: 1.0,
            difficulty: core::skill::Difficulty::default(),
            aim_assist: true,
            team_handicap_on: true,
            fov_scale: 1.0,
            music_volume: 1.0,
            mouse_sensitivity: 1.0,
            invert_y: false,
            viewpoint: crate::viewpoint::Viewpoint::default(),
            keyboard_bindings: crate::kbm::Bindings::default(),
            controller_bindings: input::Mapping::default(),
            team_size_pct: 100,
            reinforcement_pct: 100,
            last_map: None,
            last_mode: None,
            last_teams: None,
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
}

impl Profile {
    pub fn path() -> PathBuf {
        if let Some(p) = std::env::var_os("OPENBF_PROFILE") {
            return PathBuf::from(p);
        }
        data_dir().join("profile.ron")
    }

    /// Load the profile, or a fresh default one (also when the file is
    /// unreadable — a corrupt profile must never block play; it is backed up).
    pub fn load_or_default() -> Profile {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(txt) => match ron::from_str::<Profile>(&txt) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "profile {} unreadable ({e}); starting fresh",
                        path.display()
                    );
                    let _ = std::fs::rename(&path, path.with_extension("ron.bad"));
                    Profile::fresh()
                }
            },
            Err(_) => Profile::fresh(),
        }
    }

    fn fresh() -> Profile {
        Profile {
            name: std::env::var("USER").unwrap_or_else(|_| "player".into()),
            ..Default::default()
        }
    }

    /// Atomic save.
    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
        }
        let txt = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())?;
        let tmp = path.with_extension("ron.tmp");
        std::fs::write(&tmp, txt).with_context(|| tmp.display().to_string())?;
        std::fs::rename(&tmp, &path).with_context(|| path.display().to_string())?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
            }
        );
        assert_eq!(profile.settings.assist_intensity, 0.7);
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
        assert_eq!(at("elite").tier().err_cap, Some(0.5), "20+extras");
        let s = Settings {
            difficulty: Difficulty::Elite,
            ..Default::default()
        };
        let back: Settings = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.difficulty, Difficulty::Elite);
    }

    #[test]
    fn an_older_profile_reads_the_mouse_defaults() {
        let s: Settings = ron::from_str("(assist_intensity: 0.5, fov_scale: 1.1)").unwrap();
        assert_eq!((s.mouse_sensitivity, s.invert_y), (1.0, false));
        assert_eq!(s.assist_intensity, 0.5);
        assert_eq!(s.keyboard_bindings, crate::kbm::Bindings::default());
        assert_eq!(s.controller_bindings, input::Mapping::default());
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
        original.settings.last_map = Some("tat2".into());
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
        // SAFETY: test-only env override, single-threaded use of this var.
        unsafe { std::env::set_var("OPENBF_PROFILE", &path) };
        assert_eq!(Profile::path(), path);
        let mut p = Profile::load_or_default();
        assert_eq!(p.career, Career::default());
        p.settings.assist_intensity = 0.7;
        p.settings.last_map = Some("tat2".into());
        p.record_round(Some(true), 12, 3, 240, 90, 2);
        p.career.class_kills.insert("all_inf_officer".into(), 7);
        p.career.class_kills.insert("all_inf_wookiee".into(), 5);
        p.save().expect("save");
        let back = Profile::load_or_default();
        assert_eq!(back, p);
        assert_eq!(back.career.wins, 1);
        // Corrupt file → fresh profile, original kept as .bad.
        std::fs::write(&path, "(not ron").unwrap();
        let fresh = Profile::load_or_default();
        assert_eq!(fresh.career, Career::default());
        assert!(path.with_extension("ron.bad").exists());
        unsafe { std::env::remove_var("OPENBF_PROFILE") };
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn legacy_absolute_cap_does_not_become_a_percent() {
        let serialized = ron::to_string(&super::Settings::default()).unwrap();
        assert!(serialized.contains("team_size_pct:100,"));
        let legacy = serialized
            .replace("team_size_pct:100,", "")
            .replace("reinforcement_pct:100,", "")
            .replace("last_map:None", "ai_alive_cap:Some(16),last_map:None");
        let loaded: super::Settings = ron::from_str(&legacy).unwrap();
        assert_eq!((loaded.team_size_pct, loaded.reinforcement_pct), (100, 100));
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
}
