//! The optional DLC adds Kit Fisto and Asajj Ventress to Hero Assault.
//! Each joins the stock roster on their respective side.

use crate::mission::MissionDef;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

pub(crate) const CLASSES: &str = "bf2_dlc.ron";
const MANIFEST: &str = "side_bf2/dlc_heroes.ron";
const HEROES: [(&str, &str); 2] = [("rep", "dlc_hero_fisto"), ("cis", "dlc_hero_ventress")];

pub(crate) fn label(class: &str) -> Option<&'static str> {
    match class {
        "dlc_hero_fisto" => Some("Kit Fisto"),
        "dlc_hero_ventress" => Some("Asajj Ventress"),
        _ => None,
    }
}

/// The selective bake finished: its effect and sound layers may be read too.
pub(crate) fn installed(assets_dir: &Path) -> bool {
    pak::is_file(assets_dir.join(MANIFEST))
}

/// Add the heroes' own classes to a class db a reader built for itself. They
/// are a file of their own, so any db a hero's weapon is looked up in needs
/// this; without the marker it changes nothing, and their `dlch_` names cannot
/// shadow a retail class.
pub(crate) fn load_classes(classes_dir: &Path, db: &mut crate::classdb::ClassDb) -> Result<()> {
    let installed = classes_dir.parent().is_some_and(installed);
    if installed {
        db.load(&classes_dir.join(CLASSES))?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct Heroes {
    heroes: Vec<Hero>,
}

#[derive(Deserialize)]
struct Hero {
    class: String,
    side: String,
}

/// Resolve a Hero Assault addition from the selective bake's completion marker.
pub(crate) fn hero(
    classes_dir: &Path,
    mission: &MissionDef,
    faction: &str,
) -> Result<Option<&'static str>> {
    if mission.source_game != "bf2" || !mission.is_hero_assault() {
        return Ok(None);
    }
    let Some((_, class)) = HEROES.iter().find(|(side, _)| *side == faction) else {
        return Ok(None);
    };
    let Some(root) = classes_dir.parent() else {
        return Ok(None);
    };
    let path = root.join(MANIFEST);
    if !pak::is_file(&path) {
        return Ok(None);
    }
    let manifest: Heroes =
        ron::from_str(&pak::read_to_string(&path)?).with_context(|| path.display().to_string())?;
    ensure!(
        manifest.heroes.len() == HEROES.len()
            && HEROES.iter().all(|(side, class)| manifest
                .heroes
                .iter()
                .any(|h| h.side == *side && h.class == *class)),
        "{}: expected only Kit Fisto and Asajj Ventress",
        path.display()
    );
    ensure!(
        pak::is_file(classes_dir.join(CLASSES)),
        "{}: DLC hero classes are missing; rebake DLC heroes",
        path.display()
    );
    Ok(Some(class))
}

/// The selective bake owns these descriptors; addon mission load order and
/// retail heroes' custom-animation defaults do not apply to either new hero.
pub(crate) fn animations(
    classes_dir: &Path,
    geometry: &str,
) -> Result<crate::animator::custom::Descriptors> {
    let root = classes_dir
        .parent()
        .context("class directory has no asset root")?;
    let path = root
        .join("side_bf2/soldiers")
        .join(format!("{geometry}.combos.ron"));
    let set: manifest::combo::ComboSet =
        ron::from_str(&pak::read_to_string(&path)?).with_context(|| path.display().to_string())?;
    let mut descriptors = crate::animator::custom::Descriptors::default();
    for program in &set.combos {
        descriptors.register(program);
    }
    Ok(descriptors)
}

#[cfg(test)]
mod tests;
