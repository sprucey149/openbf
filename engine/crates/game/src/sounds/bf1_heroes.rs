//! BF2 sound dependencies of the playable kits selected by BF1 missions.

use super::{note_indexed, note_music, read_index, Bank, SoundLib};
use crate::{classdb::fnv1a_lowercase as hash, foley::Table, roster};
use manifest::combo::{ComboField, ComboKeyword};
use manifest::FoleyRow;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn combo_sounds(fields: &[ComboField], names: &mut BTreeSet<u32>) {
    for field in fields {
        if field.keyword == ComboKeyword::Sound {
            if let Some(name) = field.text(0) {
                names.insert(hash(name));
            }
        }
        combo_sounds(&field.children, names);
    }
}

impl SoundLib {
    pub(super) fn import_bf1_heroes<'a>(
        &mut self,
        map: &Path,
        heroes: impl Iterator<Item = &'a str>,
    ) -> Option<Table> {
        let root = map.parent()?;
        let bf1 = pak::read_to_string(map.join("manifest.ron"))
            .ok()
            .and_then(|s| ron::from_str::<manifest::ManifestHead>(&s).ok())
            .is_some_and(|m| m.source_game.eq_ignore_ascii_case("bf1"));
        if !bf1 {
            return None;
        }
        let mut classes = BTreeSet::new();
        let mut sources = BTreeSet::new();
        let mut names = BTreeSet::new();
        for hero in heroes {
            // Original BF2 maps carrying these kits and their dependencies.
            let donor = match hero {
                "rep_hero_macewindu" | "cis_hero_countdooku" => "bf2_geo1",
                "all_hero_luke_jedi" => "bf2_tat3",
                "all_hero_luke_pilot" | "imp_hero_darthvader" => "bf2_hot1",
                "rep_hero_obiwan" | "cis_hero_jangofett" => "bf2_kam1",
                "rep_hero_yoda" | "all_hero_chewbacca" => "bf2_kas2",
                "all_hero_hansolo_tat" | "imp_hero_bobafett" => "tat2",
                "cis_hero_darthmaul" => "bf2_nab2",
                _ => continue,
            };
            sources.insert(donor);
            let side = hero.split('_').next().unwrap_or("");
            let Ok(db) = roster::load_classdb(&root.join("classes"), "bf2", &[side]) else {
                eprintln!("hero audio {hero}: BF2 class data unavailable");
                continue;
            };
            if let Some(name) = db.prop(hero, "FoleyFXClass") {
                classes.insert(hash(name));
            }
            // Property values are factual dependency names. Only values which
            // resolve to sound definitions below are imported, never a whole
            // donor infantry bank. Follow weapon/ordnance/base dependencies.
            let mut pending = vec![hero.to_owned()];
            let mut seen = BTreeSet::new();
            while let Some(class) = pending.pop() {
                if !seen.insert(class.clone()) {
                    continue;
                }
                for definition in db.chain(&class) {
                    for (_, value) in &definition.props {
                        for token in value.split_whitespace() {
                            names.insert(hash(token));
                            if let Some(class) = db.resolve(token) {
                                pending.push(class.to_owned());
                            }
                        }
                    }
                }
            }
            if let Ok(class) = roster::ClassRoster::build(&db, hero, 0, Some(1)) {
                for weapon in
                    std::iter::once(&class.weapon).chain(class.held.iter().map(|h| &h.weapon))
                {
                    if let Some(saber) = &weapon.saber {
                        combo_sounds(&saber.combo.fields, &mut names);
                    }
                }
            }
        }
        if sources.is_empty() {
            return None;
        }
        if let Some(core) = &mut self.core {
            for source in sources {
                let Some(bank) = Bank::load(&root.join(source).join("sounds")) else {
                    eprintln!("hero audio: missing {source} sound bank");
                    continue;
                };
                let mut needed = names.clone();
                for name in &names {
                    if let Some(record) = bank.music.get(name) {
                        needed.insert(record.stream);
                        self.map
                            .music
                            .entry(*name)
                            .or_insert_with(|| record.clone());
                    }
                }
                let mut missing = needed
                    .iter()
                    .copied()
                    .filter(|h| !core.props.contains_key(h) && !self.map.props.contains_key(h))
                    .collect();
                core.import_props_with(&bank, &mut missing, true);
                let dir = bank.dir.join("streams");
                let index = read_index(&dir.join("streams.ron"));
                let mut music = std::collections::HashMap::new();
                note_music(&index, &dir, &mut music);
                for (key, value) in music {
                    self.music_takes.entry(key).or_insert(value);
                }
                for prop in needed.iter().filter_map(|h| bank.props.get(h)) {
                    for (id, _) in &prop.segments {
                        if let Some((_, rel)) = index.files.iter().find(|(key, _)| key == id) {
                            if let Ok(path) = std::path::absolute(dir.join(rel)) {
                                self.map_stream_files
                                    .entry(*id)
                                    .or_insert_with(|| path.to_string_lossy().into_owned());
                            }
                        }
                    }
                }
            }
        }
        // Keep BF1's existing stream IDs. BF2-only IDs use their own absolute
        // paths, including packed-archive paths, as Bank::import_props does.
        let dir = root.join("audio/global_bf2");
        let index = read_index(&dir.join("global.ron"));
        note_indexed(&index, &mut self.indexed);
        let mut music = std::collections::HashMap::new();
        note_music(&index, &dir, &mut music);
        for (key, value) in music {
            self.music_takes.entry(key).or_insert(value);
        }
        for (id, rel) in index.files {
            if let Ok(path) = std::path::absolute(dir.join(rel)) {
                self.global_files
                    .entry(id)
                    .or_insert_with(|| path.to_string_lossy().into_owned());
            }
        }
        self.import_hero_foley(map, root, classes)
    }

    fn import_hero_foley(
        &mut self,
        map: &Path,
        root: &Path,
        classes: BTreeSet<u32>,
    ) -> Option<Table> {
        let mut table = Table::load(map, &self.levels)?;
        let clone_trooper = hash("rep_inf_trooper");
        let cw_heroes = ["rep_inf_soldier", "cis_inf_soldier", "npc_inf_small"].map(hash);
        let selected: Vec<_> = classes
            .into_iter()
            .filter(|class| cw_heroes.contains(class))
            .collect();
        if selected.is_empty() {
            return Some(table);
        }
        // The row names encode the material even when the surface group is
        // called terrain_foley or points to a differently named material.
        let names = crate::foley::names();
        let material = |row: &FoleyRow| {
            names
                .get(&row.walk_left)?
                .rsplit_once("_walk_")?
                .1
                .strip_suffix("_left")
        };
        let donors = [
            ("bf2_geo1", "geo1cw"),
            ("bf2_fel1", "fel1cw"),
            ("bf2_kas2", "kas2cw"),
            ("bf2_dag1", "dag1cw"),
            ("bf2_end1", "end1gcw"),
        ];
        let donors: Vec<_> = donors
            .into_iter()
            .filter_map(|(name, level)| {
                let dir = root.join(name);
                let table = Table::load(&dir, &[hash(level)])?;
                Some((dir.join("sounds"), table))
            })
            .collect();
        let mut banks = BTreeMap::new();
        for group in &mut table.groups {
            // Use the BF1 clone's actual surface audio as the local control.
            // It also supplies the same surface if BF2 lacks that family.
            let Some(native) = group
                .rows
                .iter()
                .find(|row| row.class == clone_trooper)
                .cloned()
            else {
                continue;
            };
            for &class in &selected {
                if group
                    .rows
                    .iter()
                    .any(|row| row.class == class || row.class == 0)
                {
                    continue;
                }
                let found = material(&native).and_then(|surface| {
                    donors.iter().find_map(|(dir, donor)| {
                        let row = donor
                            .groups
                            .iter()
                            .flat_map(|g| &g.rows)
                            .find(|r| r.class == class && material(r) == Some(surface))?;
                        let bank = banks.entry(dir).or_insert_with(|| Bank::load(dir));
                        self.import_foley_row(bank.as_ref()?, row).then_some(row)
                    })
                });
                let row = found.cloned().unwrap_or_else(|| FoleyRow {
                    class,
                    ..native.clone()
                });
                group.rows.push(row);
            }
        }
        Some(table)
    }

    fn import_foley_row(&mut self, bank: &Bank, row: &FoleyRow) -> bool {
        let mut needed = Vec::new();
        for sound in [
            row.walk_left,
            row.walk_right,
            row.run_left,
            row.run_right,
            row.jump,
            row.land,
            row.roll,
            row.prone,
            row.squat,
            row.stand,
            row.body_fall,
        ]
        .into_iter()
        .filter(|sound| *sound != 0)
        {
            match self.find(sound) {
                Some((prop, _)) if super::audible(&prop) => {}
                Some(_) => return false,
                None if bank.props.get(&sound).is_some_and(super::audible) => needed.push(sound),
                None => return false,
            }
        }
        let Some(core) = &mut self.core else {
            return needed.is_empty();
        };
        let mut imports = needed.clone();
        core.import_props_with(bank, &mut imports, true);
        needed
            .into_iter()
            .all(|hash| core.props.contains_key(&hash))
    }
}
