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
    let missions = |key: &str, mode: Mode| crate::mission::pairs(missions_dir, key, mode.subst());
    let maps = ui::shell::catalog(&text, &bakes, &missions);
    let texts = IaTexts::new(&|key, english| {
        bf2.bf2_text(key)
            .map_or_else(|| english.to_owned(), |t| t.trim_end().to_owned())
    });
    let options = SessionOptions {
        difficulty: ia_level(difficulty),
        population_table: crate::conquest_defaults::TABLE,
        dlc_heroes_installed: crate::dlc::installed(assets_dir),
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
        assert!(hoth.runnable(Mode::Con, ui::shell::Side::Era(ui::shell::Era::Gcw)));
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
