//! The Instant Action screen's content from the bake (`ui::shell`,
//! `analysis/re-bf2-menus.md` §6-§7): both games' maps by their English
//! names, which are baked, the faction pairs of their mission files, the
//! screen's texts; and the profile's AI tier as Session Options' level.

use std::path::Path;
use ui::shell::{Game, IaTexts, InstantAction, Mode, SessionOptions};

/// The profile's tier as Session Options' AI Difficulty level: 1 Easy,
/// 2 Normal, 3 Hard, 4 Elite (`ui::shell::SessionOptions::difficulty`;
/// `[ours]`, the PS2's row offers only its Normal and Elite).
pub fn ia_level(d: core::skill::Difficulty) -> i32 {
    use core::skill::Difficulty::*;
    match d {
        Easy => 1,
        Normal => 2,
        Hard => 3,
        Elite => 4,
    }
}

/// Session Options' level back as the port's tier (out of range: the
/// nearest end).
pub fn port_difficulty(level: i32) -> core::skill::Difficulty {
    use core::skill::Difficulty::*;
    match level {
        ..=1 => Easy,
        2 => Normal,
        3 => Hard,
        _ => Elite,
    }
}

/// The screen at its first Enter: the maps of the bake at `assets_dir`, the
/// missions in `missions_dir`, the AI tier `difficulty`.
pub fn instant_action(
    assets_dir: &Path,
    missions_dir: &Path,
    difficulty: core::skill::Difficulty,
) -> InstantAction {
    let mut ia = catalog_action(assets_dir, missions_dir, difficulty, true);
    if std::env::var_os(extensions::profile::GROUP_MAPS_ENV).is_some() {
        ia.group_by_source();
    }
    ia
}

pub fn lobby_instant_action(
    assets_dir: &Path,
    missions_dir: &Path,
    difficulty: core::skill::Difficulty,
) -> InstantAction {
    catalog_action(assets_dir, missions_dir, difficulty, false)
}

fn catalog_action(
    assets_dir: &Path,
    missions_dir: &Path,
    difficulty: core::skill::Difficulty,
    single_player: bool,
) -> InstantAction {
    let bf2 = crate::event_feed::Texts::load(assets_dir, "bf2");
    let bf1 = crate::event_feed::Texts::load(assets_dir, "bf1");
    let text = |g: Game, key: &str| {
        match g {
            Game::Bf2 => bf2.bf2_text(key),
            Game::Bf1 => bf1.map_text(key),
        }
        .map(str::to_owned)
    };
    let bakes: Vec<(String, String)> = crate::assets::discover_maps(assets_dir)
        .into_iter()
        .map(|(k, g, _)| (k, g))
        .collect();
    let missions = |key: &str, mode: Mode| match mode {
        Mode::Xl if single_player => crate::mission::pairs(missions_dir, key, Mode::Con.subst()),
        Mode::Xl => Vec::new(),
        _ => crate::mission::pairs(missions_dir, key, mode.subst()),
    };
    let maps = ui::shell::catalog_with_mods(
        &text,
        &bakes,
        &missions,
        &crate::mods_runtime::active_mod_maps(),
    );
    let mut texts = IaTexts::new(&|key, english| {
        bf2.bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    });
    let custom_meta = crate::mods_runtime::active_custom_modes();
    let mut icon_dirs = crate::mods_runtime::active_shell_icon_dirs();
    icon_dirs.push(crate::shell_movie::shell_dir(assets_dir).join("textures"));
    let has_icon = |name: &str| {
        let fname = format!("{}.ktx2", name.to_ascii_lowercase());
        icon_dirs.iter().any(|d| pak::is_file(d.join(&fname)))
    };
    let mut custom_seen = std::collections::BTreeSet::new();
    for map in &maps {
        for era_modes in &map.modes {
            for mode in era_modes {
                let Mode::Custom(s) = mode else { continue };
                if !custom_seen.insert(s.clone()) {
                    continue;
                }
                let meta = custom_meta.get(s);
                let locl_name = bf2
                    .bf2_text(&format!("modename.name.{s}"))
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty());
                let locl_desc = bf2
                    .bf2_text(&format!("modename.description.{s}"))
                    .map(|t| t.trim_end().to_owned())
                    .filter(|t| !t.is_empty());
                let name = meta
                    .and_then(|m| m.name.clone())
                    .or(locl_name)
                    .or_else(|| {
                        locl_desc
                            .as_deref()
                            .and_then(|d| d.lines().next())
                            .map(|l| l.trim().to_owned())
                            .filter(|l| !l.is_empty())
                    })
                    .unwrap_or_else(|| s.to_ascii_uppercase());
                let desc = locl_desc
                    .or_else(|| {
                        meta.and_then(|m| m.description.as_deref())
                            .map(|d| format!("{name}\r\n{d}"))
                    })
                    .unwrap_or_else(|| format!("{name}\r\nCustom mod game mode."));
                let mut icon_candidates: Vec<String> = Vec::new();
                if let Some(ic) = meta.and_then(|m| m.icon.as_deref()) {
                    icon_candidates.push(ic.to_ascii_lowercase());
                }
                icon_candidates.push(format!("mode_icon_{s}"));
                let name_slug: String = name
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .map(|c| c.to_ascii_lowercase())
                    .collect();
                if !name_slug.is_empty() {
                    icon_candidates.push(format!("mode_icon_{name_slug}"));
                }
                let icon = icon_candidates
                    .into_iter()
                    .find(|cand| has_icon(cand))
                    .unwrap_or_else(|| "mode_icon_con".to_owned());
                texts.set_custom_mode(s, name, desc, icon);
            }
        }
    }
    let dlc_heroes_installed = crate::dlc::installed(assets_dir);
    let options = SessionOptions {
        difficulty: ia_level(difficulty),
        population_table: crate::conquest_defaults::TABLE,
        dlc_heroes_installed,
        dlc_heroes: dlc_heroes_installed,
        ..Default::default()
    };
    InstantAction::new(maps, texts, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Instant Action row offers exactly the four tiers, one level
    /// each, in order.
    #[test]
    fn the_four_tiers_round_trip_through_the_rows_levels() {
        use core::skill::Difficulty;
        let levels = Difficulty::ALL.map(ia_level);
        assert_eq!(levels, [1, 2, 3, 4]);
        assert_eq!(*levels.last().unwrap(), ui::shell::DIFFICULTY_LEVELS);
        for d in Difficulty::ALL {
            assert_eq!(port_difficulty(ia_level(d)), d);
        }
        assert_eq!(
            ia_level(Difficulty::default()),
            ui::shell::SessionOptions::default().difficulty
        );
    }

    #[test]
    fn the_baked_catalog() {
        let assets = crate::assets::baked_root();
        let missions =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../missions"));
        if crate::assets::discover_maps(&assets).is_empty() {
            eprintln!("skip: no bake");
            return;
        }
        let ia = instant_action(&assets, &missions, Default::default());
        assert_eq!(ia.maps.len(), 40);
        let hoth = ia.maps.iter().find(|m| m.label == "Hoth (bf2)").unwrap();
        assert_eq!(hoth.key, "bf2_hot1");
        assert!(hoth.description.starts_with("Hoth: Echo Base"));
        assert!(hoth.runnable(&Mode::Con, ui::shell::Side::Era(ui::shell::Era::Gcw)));
        let dune = ia
            .maps
            .iter()
            .find(|m| m.label == "Tatooine: Dune Sea (bf1)")
            .unwrap();
        assert_eq!(dune.key, "tat1");
        let rows = ia.map_rows();
        let dimmed: Vec<&str> = rows
            .iter()
            .filter(|r| !r.enabled)
            .map(|r| r.text.as_str())
            .collect();
        assert!(dimmed.iter().all(|t| t.starts_with("Space ")), "{dimmed:?}");
        assert_eq!(ia.options.population_table.len(), 61);
        let help = ia
            .options
            .population_help(ui::shell::Opt::ConNumBots)
            .unwrap();
        assert!(help.starts_with("Average team size: 20 units\n"), "{help}");
    }
}
