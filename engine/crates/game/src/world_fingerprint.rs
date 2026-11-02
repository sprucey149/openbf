//! Baked world compatibility identity for a future multiplayer handshake.
//!
//! `scripts/world_fingerprint.py` computes this from baked gameplay inputs.
//! A match is meaningful only for the same fingerprint schema/coverage and
//! engine revision; the simulation build identity remains a separate check.
//! This is a compatibility check, not authentication of another player's data.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorldFingerprint {
    pub schema: u32,
    pub algorithm: String,
    pub map: String,
    pub source_game: String,
    pub sha256: String,
    pub coverage: String,
    #[serde(default)]
    pub complete: bool,
}

impl WorldFingerprint {
    /// Load a fingerprint alongside a baked map; absence is an explicit error.
    pub fn load(map_dir: &Path) -> Result<Self> {
        let path = map_dir.join("world-fingerprint.json");
        let text = pak::read_to_string(&path).with_context(|| path.display().to_string())?;
        let value: Self = serde_json::from_str(&text).context("invalid world fingerprint")?;
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != 1
            || self.algorithm != "sha256"
            || self.coverage != "conservative_baked_gameplay_inputs_v1"
        {
            bail!("unknown world fingerprint schema, algorithm or coverage; regenerate the bake");
        }
        if !self.complete {
            bail!("world fingerprint omits required gameplay inputs; finish the bake");
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.map.is_empty()
            || !matches!(self.source_game.as_str(), "bf1" | "bf2")
        {
            bail!("invalid world fingerprint identity");
        }
        Ok(())
    }

    /// Refuse unknown identities as well as differing worlds.
    pub fn compatible_with(&self, other: &Self) -> Result<bool> {
        self.validate()?;
        other.validate()?;
        Ok(self == other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> WorldFingerprint {
        WorldFingerprint {
            schema: 1,
            algorithm: "sha256".into(),
            map: "bf1_kam1".into(),
            source_game: "bf1".into(),
            sha256: "a".repeat(64),
            coverage: "conservative_baked_gameplay_inputs_v1".into(),
            complete: true,
        }
    }

    #[test]
    fn compatibility_requires_known_schema_map_and_content() {
        let a = example();
        let mut b = a.clone();
        assert!(a.compatible_with(&b).unwrap());
        b.sha256 = "b".repeat(64);
        assert!(!a.compatible_with(&b).unwrap());
        b = a.clone();
        b.map = "bf1_nab1".into();
        assert!(!a.compatible_with(&b).unwrap());
        b = a.clone();
        b.schema = 2;
        assert!(a.compatible_with(&b).is_err());
        b = a.clone();
        b.complete = false;
        assert!(a.compatible_with(&b).is_err());
        b = a.clone();
        b.sha256 = "x".repeat(64);
        assert!(a.compatible_with(&b).is_err());
    }
}
