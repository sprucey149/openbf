//! BF2 sound dependencies of the five playable kits selected by BF1 missions.

use super::{note_indexed, note_music, read_index, Bank, SoundLib};
use crate::{classdb::fnv1a_lowercase as hash, roster};
use manifest::combo::{ComboField, ComboKeyword};
use std::{collections::BTreeSet, path::Path};

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
    ) {
        let Some(root) = map.parent() else { return };
        let bf1 = pak::read_to_string(map.join("manifest.ron"))
            .ok()
            .and_then(|s| ron::from_str::<manifest::ManifestHead>(&s).ok())
            .is_some_and(|m| m.source_game.eq_ignore_ascii_case("bf1"));
        if !bf1 {
            return;
        }
        let mut sources = BTreeSet::new();
        let mut names = BTreeSet::new();
        for hero in heroes {
            // Original BF2 maps carrying these kits and their dependencies.
            let donor = match hero {
                "rep_hero_macewindu" | "cis_hero_countdooku" => "bf2_geo1",
                "all_hero_luke_jedi" => "bf2_tat3",
                "all_hero_luke_pilot" | "imp_hero_darthvader" => "bf2_hot1",
                _ => continue,
            };
            sources.insert(donor);
            let side = hero.split('_').next().unwrap_or("");
            let Ok(db) = roster::load_classdb(&root.join("classes"), "bf2", &[side]) else {
                eprintln!("hero audio {hero}: BF2 class data unavailable");
                continue;
            };
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
            return;
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
    }
}
