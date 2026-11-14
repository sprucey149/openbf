//! What a player can play (LAN §4.1, §4.2, §7): the content manifest each
//! guest sends in its `Hello`, the host's intersection of everyone's, and the
//! build identity peers must share exactly.
//!
//! Rules:
//! - A map is offered only when every player has its key. Nothing about the
//!   map's files is compared here: a baked `manifest.ron` holds presentation
//!   data (textures, sky, lighting, PC texture tiers) that differs between
//!   PS2, PC and Xbox copies, and its line endings differ between a Windows
//!   and a Linux or macOS bake. Whether two copies simulate the same is the
//!   load barrier's `Sim::static_digest` check, which benches only the player
//!   whose copy differs.
//! - The addon heroes are offered only when every player has them.
//! - The build identity is the source commit plus, for a dirty tree, a hash
//!   of the uncommitted changes (`game/build.rs`). The OS and CPU are
//!   reported for diagnostics only, never a reason to refuse.

use serde::{Deserialize, Serialize};

/// `build.rs`'s pure rules, compiled here too so their tests run.
use std::collections::BTreeSet;
use std::path::Path;

/// One player's playable content.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentManifest {
    /// The baked map keys.
    pub maps: BTreeSet<String>,
    /// The games present ("bf1", "bf2").
    pub games: BTreeSet<String>,
    /// The Kit Fisto and Asajj Ventress addon is installed.
    pub addon_heroes: bool,
}

/// FNV-1a 64 over `bytes`.
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

impl ContentManifest {
    /// This install's content under `assets_dir`.
    pub fn scan(assets_dir: &Path) -> Self {
        let mut m = Self::default();
        for (key, game, _dir) in crate::assets::discover_maps(assets_dir) {
            m.maps.insert(key);
            m.games.insert(game.to_ascii_lowercase());
        }
        m.addon_heroes = crate::dlc::installed(assets_dir);
        m
    }

    /// What every one of `all` can play: maps everyone has, games everyone
    /// has, the addon only when all have it.
    pub fn intersect<'a>(all: impl IntoIterator<Item = &'a ContentManifest>) -> Self {
        let mut it = all.into_iter();
        let Some(first) = it.next() else {
            return Self::default();
        };
        let mut out = first.clone();
        for m in it {
            out.maps.retain(|k| m.maps.contains(k));
            out.games.retain(|g| m.games.contains(g));
            out.addon_heroes &= m.addon_heroes;
        }
        out
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("content encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// The sim build identity: the commit, plus `+<diff hash>` when the tree
/// was dirty at build time. Peers must match it exactly.
pub fn build_id() -> String {
    let commit = env!("OPENBF_BUILD_COMMIT");
    let diff = env!("OPENBF_BUILD_DIFF");
    if diff.is_empty() {
        commit.to_owned()
    } else {
        format!("{commit}+{diff}")
    }
}

/// The OS and CPU, for diagnostics.
pub fn target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
