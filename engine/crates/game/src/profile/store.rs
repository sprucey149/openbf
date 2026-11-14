//! Local profile catalog. The lock covers each read/modify/write transaction
//! so separate game processes can autosave different profiles safely.

use super::Profile;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

/// BF2's console name entry accepts up to ten characters.
pub const NAME_LIMIT: usize = 10;

pub fn valid_name_char(c: char) -> bool {
    c == ' ' || c.is_ascii_graphic()
}

#[derive(Debug)]
pub struct List {
    pub names: Vec<(u64, String)>,
    pub last: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Slot {
    store: Store,
    id: u64,
}

impl Slot {
    pub(super) fn save(&self, profile: &Profile) -> Result<()> {
        self.store.update(|catalog| {
            *catalog
                .profiles
                .get_mut(&self.id)
                .context("Profile was deleted")? = profile.clone();
            Ok(())
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Catalog {
    profiles: BTreeMap<u64, Profile>,
    last: Option<u64>,
    next: u64,
}

impl Catalog {
    fn insert(&mut self, profile: Profile) -> Result<u64> {
        let id = self.next;
        self.next = id.checked_add(1).context("Profile IDs exhausted")?;
        self.profiles.insert(id, profile);
        self.last = Some(id);
        Ok(id)
    }
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn access<T>(&self, f: impl FnOnce(&mut Catalog) -> Result<(T, bool)>) -> Result<T> {
        fs::create_dir_all(&self.dir).with_context(|| self.dir.display().to_string())?;
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.dir.join("profiles.lock"))?;
        lock.lock()?;
        let path = self.dir.join("profiles.ron");
        let (mut catalog, imported) = match fs::read_to_string(&path) {
            Ok(text) => (
                ron::from_str(&text).with_context(|| format!("Cannot read {}", path.display()))?,
                false,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.import_legacy()?,
            Err(e) => return Err(e).with_context(|| path.display().to_string()),
        };
        let (result, changed) = f(&mut catalog)?;
        if changed || imported {
            super::write_ron(&catalog, &path)?;
        }
        Ok(result)
    }

    fn update<T>(&self, f: impl FnOnce(&mut Catalog) -> Result<T>) -> Result<T> {
        self.access(|catalog| Ok((f(catalog)?, true)))
    }

    fn import_legacy(&self) -> Result<(Catalog, bool)> {
        let path = self.dir.join("profile.ron");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Catalog::default(), false));
            }
            Err(e) => return Err(e).with_context(|| path.display().to_string()),
        };
        let mut catalog = Catalog::default();
        match ron::from_str::<Profile>(&text) {
            Ok(mut profile) => {
                if profile.name.trim().is_empty() {
                    profile.name = "Player 1".into();
                }
                catalog.insert(profile)?;
            }
            Err(e) => {
                eprintln!("Could not import {}: {e}", path.display());
                return Ok((catalog, false));
            }
        }
        // Keep the original file as a backup. An existing catalog, even an
        // empty one, records that the import was already attempted.
        Ok((catalog, true))
    }

    pub fn list(&self) -> Result<List> {
        self.access(|c| {
            let names = c
                .profiles
                .iter()
                .map(|(&id, p)| (id, p.name.clone()))
                .collect();
            Ok((
                List {
                    names,
                    last: c.last,
                },
                false,
            ))
        })
    }

    fn attach(&self, id: u64, mut profile: Profile) -> Profile {
        profile.normalize();
        profile.slot = Some(Slot {
            store: self.clone(),
            id,
        });
        profile
    }

    pub fn select(&self, id: u64) -> Result<Profile> {
        self.update(|c| {
            let profile = c.profiles.get(&id).context("Profile was deleted")?.clone();
            c.last = Some(id);
            Ok(self.attach(id, profile))
        })
    }

    pub fn create(&self, name: &str, difficulty: core::skill::Difficulty) -> Result<Profile> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Enter at least one character");
        }
        if name.chars().count() > NAME_LIMIT || !name.chars().all(valid_name_char) {
            bail!("Use up to {NAME_LIMIT} letters, numbers or symbols");
        }
        self.update(|c| {
            if c.profiles
                .values()
                .any(|p| p.name.trim().eq_ignore_ascii_case(name))
            {
                bail!("That profile name already exists");
            }
            let mut profile = Profile {
                name: name.into(),
                ..Profile::default()
            };
            profile.settings.difficulty = difficulty;
            let id = c.insert(profile.clone())?;
            Ok(self.attach(id, profile))
        })
    }

    pub fn delete(&self, id: u64) -> Result<()> {
        self.update(|c| {
            c.profiles
                .remove(&id)
                .context("Profile was already deleted")?;
            if c.last == Some(id) {
                c.last = c.profiles.first_key_value().map(|(&id, _)| id);
            }
            Ok(())
        })
    }

    /// Direct map launches have no picker. Use the last profile, or make
    /// the default named profile if there is no save yet.
    pub fn for_direct_launch(&self) -> Result<Profile> {
        self.access(|c| {
            let selected = c
                .last
                .and_then(|id| c.profiles.get_key_value(&id))
                .or_else(|| c.profiles.first_key_value());
            if let Some((&id, profile)) = selected {
                return Ok((self.attach(id, profile.clone()), false));
            }
            let profile = Profile {
                name: "Player 1".into(),
                ..Profile::default()
            };
            let id = c.insert(profile.clone())?;
            Ok((self.attach(id, profile), true))
        })
    }
}

#[cfg(test)]
mod tests;
