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
    pub(super) fn id(&self) -> u64 {
        self.id
    }

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
        let id = self.insert_quiet(profile)?;
        self.last = Some(id);
        Ok(id)
    }

    /// [`Catalog::insert`] without moving `last`.
    fn insert_quiet(&mut self, profile: Profile) -> Result<u64> {
        let id = self.next;
        self.next = id.checked_add(1).context("Profile IDs exhausted")?;
        self.profiles.insert(id, profile);
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

    /// Issue #194: a splitscreen seat's pick (P2-P4). [`Store::select`]
    /// without moving `last`, so the title picker still offers the host's
    /// profile next launch, and without writing the catalog. The profile is
    /// marked as a seat's ([`Profile::into_seat`]). Seat 0 (the host) never
    /// comes through here: its re-pick on the Players screen goes through
    /// [`Store::select`] / [`Store::create`], which move `last` and leave the
    /// profile unmarked. `bind` does not refuse an id another seat already
    /// holds (two holders would overwrite each other's saves); the pickers
    /// grey those out by [`Profile::id`].
    pub fn bind(&self, id: u64) -> Result<Profile> {
        self.access(|c| {
            let profile = c.profiles.get(&id).context("Profile was deleted")?.clone();
            Ok((self.attach(id, profile).into_seat(), false))
        })
    }

    pub fn create(&self, name: &str, difficulty: core::skill::Difficulty) -> Result<Profile> {
        self.create_in(name, difficulty, true)
    }

    /// Issue #194: a seat's new profile. [`Store::create`] without moving
    /// `last`; `difficulty` is the host's (seat pickers have no Difficulty
    /// page). The profile is marked as a seat's. Seats 1-3 only; seat 0 uses
    /// [`Store::create`].
    pub fn create_quiet(&self, name: &str, difficulty: core::skill::Difficulty) -> Result<Profile> {
        self.create_in(name, difficulty, false)
            .map(Profile::into_seat)
    }

    fn create_in(
        &self,
        name: &str,
        difficulty: core::skill::Difficulty,
        move_last: bool,
    ) -> Result<Profile> {
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
            let id = if move_last {
                c.insert(profile.clone())?
            } else {
                c.insert_quiet(profile.clone())?
            };
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

    /// The display mode and last fullscreen kind of the profile the shell
    /// would offer first (the last one picked, else the first): the window
    /// opens in it before any profile is picked. `None` with no profile.
    pub fn last_window_mode(&self) -> Result<Option<(ui::menu::WindowMode, ui::menu::WindowMode)>> {
        self.access(|c| {
            let mode = c
                .last
                .and_then(|id| c.profiles.get(&id))
                .or_else(|| c.profiles.values().next())
                .map(|p| (p.settings.window_mode, p.settings.last_fullscreen));
            Ok((mode, false))
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
