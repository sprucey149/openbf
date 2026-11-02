use crate::{classdb::ClassDb, roster::ClassRoster, walk::WalkSim};
use core::weapon::{Thrown, WeaponClass};
use std::path::{Path, PathBuf};
mod playable;

pub const TABS: [&str; 9] = [
    "BF1 Rebels",
    "BF1 Empire",
    "BF1 Republic",
    "BF1 CIS",
    "BF2 Rebels",
    "BF2 Empire",
    "BF2 Republic",
    "BF2 CIS",
    "BF2 Heroes",
];

/// The BF1 class whose thermal count a BF2 class takes on a BF1 map: each
/// side's plain thermal carrier. `[ours]`: BF1 has no row for a BF2 class.
const BF1_THERMAL_COUNT: [(&str, &str); 4] = [
    ("all", "all_inf_vanguard"),
    ("imp", "imp_inf_shock_trooper"),
    ("rep", "rep_inf_arc_trooper"),
    ("cis", "cis_inf_assault"),
];

fn is_hand_grenade(weapon: &WeaponClass, channel: u8) -> bool {
    channel == 1 && weapon.thrown.is_some() && !weapon.dispenses()
}

/// A BF2 class on a BF1 map throws its side's BF1 thermal detonator, at BF1's
/// count. Its first-person row stays: the arms are the class's own.
fn throw_bf1_grenades(class: &mut ClassRoster, bf1: &ClassDb) {
    let side = class.class.split('_').next().unwrap_or_default();
    let thermal = format!("{side}_weap_inf_thermaldetonator");
    let Ok(Some(weapon)) = crate::roster::thrown_weapon_from_db(bf1, &thermal) else {
        return;
    };
    let clips = BF1_THERMAL_COUNT
        .iter()
        .find(|(s, _)| *s == side)
        .and_then(|(_, carrier)| {
            bf1.weapon_slots(carrier)
                .into_iter()
                .find(|slot| slot.weapon.eq_ignore_ascii_case(&thermal))
        })
        .and_then(|slot| slot.ammo);
    for held in &mut class.held {
        if is_hand_grenade(&held.weapon, held.channel) {
            held.weapon = weapon.clone();
            held.hud = crate::roster::weapon_hud(bf1, &thermal);
            held.clips = clips.unwrap_or(held.clips);
        }
    }
}

/// A BF1 class on a BF2 map throws as BF2 throws.
fn throw_as_bf2(class: &mut ClassRoster) {
    for held in &mut class.held {
        if is_hand_grenade(&held.weapon, held.channel) {
            Thrown::throw_as_bf2(&mut held.weapon);
        }
    }
}

pub struct Choice {
    pub tab: usize,
    pub game: &'static str,
    pub label: String,
    pub class: ClassRoster,
}

pub struct ClassPicker {
    root: PathBuf,
    pub scene: crate::assets::Scene,
    loaded: bool,
    pub choices: Vec<Choice>,
    registered: Vec<(usize, usize, u8)>,
    pub pending_assets: Vec<crate::walk::ClassAssetRequest>,
}

impl ClassPicker {
    pub fn new(classes_dir: &Path, scene: &crate::assets::Scene) -> Self {
        Self {
            root: classes_dir.to_path_buf(),
            scene: crate::assets::Scene {
                dir: scene.dir.clone(),
                manifest: scene.manifest.clone(),
                meshes: Default::default(),
                terrain: None,
                water: None,
                lowres: None,
                terrain_layers: None,
                textures: Default::default(),
                collision: Default::default(),
                cutters: Vec::new(),
            },
            loaded: false,
            choices: Vec::new(),
            registered: Vec::new(),
            pending_assets: Vec::new(),
        }
    }

    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let map = self.scene.manifest.source_game.to_ascii_lowercase();
        let mut bf1_db = None;
        for game in ["bf1", "bf2"] {
            let db = match crate::roster::load_classdb(
                &self.root,
                game,
                &crate::roster::FACTION_ORDER,
            ) {
                Ok(db) => db,
                Err(error) => {
                    eprintln!("class picker: {error:#}");
                    continue;
                }
            };
            let texts = crate::event_feed::Texts::load(self.root.parent().unwrap(), game);
            let names = if game == "bf1" {
                playable::BF1
            } else {
                playable::BF2
            };
            let mut ready = std::collections::HashMap::new();
            for &name in names {
                match ClassRoster::build(&db, name, 0, Some(0)) {
                    Ok(mut class) => {
                        match (map.as_str(), game, bf1_db.as_ref()) {
                            ("bf1", "bf2", Some(bf1)) => throw_bf1_grenades(&mut class, bf1),
                            ("bf2", "bf1", _) => throw_as_bf2(&mut class),
                            _ => {}
                        }
                        let key = class.geometry.clone().unwrap_or_default();
                        if !*ready
                            .entry(key)
                            .or_insert_with(|| self.has_player_assets(game, &class))
                        {
                            eprintln!("class picker {game}/{name}: player model or animations unavailable");
                            continue;
                        }
                        if let Some(saber) = &class.weapon.saber {
                            class.anim.custom.register(&saber.combo);
                        }
                        let title = texts
                            .class_name(name)
                            .unwrap_or_else(|| class.label.clone());
                        self.choices.push(Choice {
                            tab: if name.contains("_hero_") {
                                TABS.len() - 1
                            } else {
                                usize::from(game == "bf2") * crate::roster::FACTION_ORDER.len()
                                    + crate::roster::FACTION_ORDER
                                        .iter()
                                        .position(|f| name.split('_').next() == Some(*f))
                                        .unwrap()
                            },
                            label: format!("{title} · {name}"),
                            game,
                            class,
                        });
                    }
                    Err(error) => eprintln!("class picker {game}/{name}: {error:#}"),
                }
            }
            if game == "bf1" {
                bf1_db = Some(db);
            }
        }
        self.choices.sort_by(|a, b| a.label.cmp(&b.label));
    }

    fn has_player_assets(&self, game: &str, class: &ClassRoster) -> bool {
        let root = self.root.parent().unwrap();
        if class.droideka.is_some() {
            return crate::vehicle_pads::side_vehicle_files(&self.scene.dir, game)
                .iter()
                .any(|(dir, manifest)| {
                    manifest.droidekas.iter().any(|d| {
                        d.class.eq_ignore_ascii_case(&class.class)
                            && d.clips.iter().any(Option::is_some)
                            && pak::is_file(dir.join("models").join(format!("{}.glb", d.geometry)))
                            && pak::is_file(
                                dir.join("walkers").join(format!(
                                    "{}.glb",
                                    d.skin.as_deref().unwrap_or(&d.geometry)
                                )),
                            )
                    })
                });
        }
        let Some(geometry) = class.geometry.as_deref() else {
            return false;
        };
        let side = root.join(format!("side_{game}"));
        crate::assets::load_skinned_glb(&side.join("soldiers").join(format!("{geometry}.glb")))
            .is_ok_and(|asset| {
                !asset.clips.is_empty()
                    && !asset.skeleton.is_empty()
                    && crate::soldier_vis::SoldierVisuals::load_rig(&side, geometry, &asset)
                        .is_some()
            })
    }

    pub fn model(&self, tab: usize) -> ui::menu::ClassPickerModel {
        let tab = tab.min(TABS.len() - 1);
        ui::menu::ClassPickerModel {
            tabs: TABS.iter().map(|s| (*s).into()).collect(),
            tab,
            choices: self
                .choices
                .iter()
                .enumerate()
                .filter(|(_, c)| c.tab == tab)
                .map(|(id, c)| (id, c.label.clone()))
                .collect(),
        }
    }

    pub fn available(walk: &WalkSim) -> bool {
        walk.sim.outcome.is_none()
            && walk
                .sim
                .unit(walk.player)
                .is_some_and(|u| u.alive && u.mount.is_none())
    }

    pub fn select(&mut self, walk: &mut WalkSim, choice: usize) -> bool {
        if !Self::available(walk) {
            return false;
        }
        let Some(entry) = self.choices.get(choice) else {
            return false;
        };
        let team = usize::from(walk.sim.unit(walk.player).unwrap().team);
        let index = if let Some((_, _, index)) = self
            .registered
            .iter()
            .find(|(t, c, _)| *t == team && *c == choice)
        {
            *index
        } else {
            let Ok(index) = u8::try_from(walk.sim.classes[team].len()) else {
                return false;
            };
            walk.sim.begin_test_classes(team as u8);
            let mut class = entry.class.clone();
            if let Some(deka) = class.droideka.as_deref() {
                if let Some(bake) = crate::droideka::bakes(&self.scene.dir, entry.game)
                    .get(&class.class.to_ascii_lowercase())
                {
                    class.droideka = Some(std::sync::Arc::new(crate::droideka::apply(deka, bake)));
                }
            }
            let uc = class.register(&mut walk.sim, &mut Vec::new());
            let visual = crate::walk::ClassVisual::from_roster_with_held(&class);
            walk.sim.classes[team].push(uc);
            walk.teams[team].classes.push(visual);
            self.registered.push((team, choice, index));
            self.pending_assets.push(crate::walk::ClassAssetRequest {
                team,
                game: entry.game,
                class: usize::from(index),
            });
            index
        };
        let hero = entry.class.class.contains("_hero_");
        if !walk.sim.test_change_class(0, index, hero) {
            return false;
        }
        walk.player = walk.sim.players[0].unwrap();
        true
    }
}

#[cfg(test)]
mod tests;
