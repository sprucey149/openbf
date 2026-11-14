//! The byte feeder behind [`crate::sim::Sim::static_digest`] (issue #168,
//! `analysis/plan/LAN_MULTIPLAYER.md` §4.3): FNV-1a-style 64-bit hashing
//! with the basis and multiplier [`crate::sim::Sim::hash`] uses (its
//! multiplier `0x1_0000_01B3` is not the published FNV prime; the two
//! feeders stay alike on purpose), fed raw bytes (`eat`, the hash's
//! convention: little-endian integers, `f32::to_bits`) or a value's `Debug`
//! text ([`Fnv64::debug`]).
//!
//! The `Debug` feed is what lets a class table be digested without a hand
//! list of its hundreds of fields: every derived field is printed, so a new
//! field joins the digest by itself. It is deterministic on every platform:
//! Rust prints a float as its shortest round-trip decimal (two different
//! bit patterns print differently, NaN payloads aside), and the digested
//! types hold no `HashMap` (BTreeMap prints in key order). A type whose
//! `Debug` prints something not of the data (an address, a wall-clock
//! measurement) must be fed by hand instead.

use crate::fmath::FloatExt;
use std::fmt::{self, Write as _};

/// The 64-bit FNV-1a-style hash `Sim::hash` computes.
#[derive(Clone, Copy, Debug)]
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv64 {
    pub const fn new() -> Self {
        Fnv64(0xCBF2_9CE4_8422_2325)
    }

    pub fn eat(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x1_0000_01B3);
        }
    }

    pub fn u32(&mut self, v: u32) {
        self.eat(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.eat(&v.to_le_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.u32(v.hash_bits());
    }

    pub fn f32s(&mut self, vs: &[f32]) {
        for &v in vs {
            self.f32(v);
        }
    }

    /// A length or index, as the hash feeds one (u64).
    pub fn count(&mut self, n: usize) {
        self.u64(n as u64);
    }

    /// `v`'s `{:?}` text, streamed (nothing is allocated).
    pub fn debug(&mut self, v: &impl fmt::Debug) {
        // Writing into the hasher cannot fail.
        let _ = write!(self, "{v:?}");
    }

    pub fn finish(&self) -> u64 {
        self.0
    }

    /// `v`'s pretty `{:#?}` text without its [`PRESENTATION_FIELDS`]: the
    /// class tables as the simulation reads them. Pretty `Debug` puts every
    /// field on its own line and closes a multi-line value at the field's
    /// own indentation, so a named field is dropped whole, at any depth.
    pub fn debug_sim(&mut self, v: &impl fmt::Debug) {
        let mut w = SimView {
            f: self,
            line: String::new(),
            skip: None,
        };
        let _ = write!(w, "{v:#?}");
        w.end_line();
    }
}

/// Fields of the class tables the simulation never reads: sound, effect
/// and texture names, and the damage effects drawn on vehicles, turrets and
/// stations. Copies of the game (PS2, PC, Xbox) name these differently while
/// simulating alike (issue #168, LAN §7), so the static digest leaves them
/// out. A field joins this list only when no `core` code reads it (the
/// game's presentation does): `effects` and `sound` of
/// `impact::ExplosionLook`, `texture` of `laser::LaserLook`, `effect` of a
/// force power and of `autoturret::DamageEffect`, `damage_effects`, the
/// weapon, ordnance, mine and pickup sound names and the droideka shield's.
pub const PRESENTATION_FIELDS: [&str; 15] = [
    "fire_sound",
    "reload_sound",
    "empty_sound",
    "change_sound",
    "land_sound",
    "contact_sound",
    "tick_sound",
    "collision_sound",
    "ordnance_sound",
    "sound",
    "effects",
    "water_effect",
    "effect",
    "texture",
    "damage_effects",
];

/// The line filter under [`Fnv64::debug_sim`].
struct SimView<'a> {
    f: &'a mut Fnv64,
    line: String,
    /// Dropping a multi-line field opened at this indentation.
    skip: Option<usize>,
}

impl SimView<'_> {
    fn end_line(&mut self) {
        let line = std::mem::take(&mut self.line);
        let body = line.trim_start_matches(' ');
        let indent = line.len() - body.len();
        if let Some(at) = self.skip {
            if indent == at && body.starts_with([')', '}', ']']) {
                self.skip = None;
            }
            if indent >= at {
                return;
            }
            self.skip = None;
        }
        if let Some((name, rest)) = body.split_once(": ") {
            if PRESENTATION_FIELDS.contains(&name) {
                if rest.ends_with(['(', '{', '[']) {
                    self.skip = Some(indent);
                }
                return;
            }
        }
        self.f.eat(line.as_bytes());
        self.f.eat(b"\n");
    }
}

impl fmt::Write for SimView<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut rest = s;
        while let Some(i) = rest.find('\n') {
            self.line.push_str(&rest[..i]);
            self.end_line();
            rest = &rest[i + 1..];
        }
        self.line.push_str(rest);
        Ok(())
    }
}

impl fmt::Write for Fnv64 {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.eat(s.as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv64_steps_as_the_sim_hash_does() {
        // `Sim::hash`'s basis and multiplier (0x1_0000_01B3, not the
        // published FNV prime): one byte is one xor and one multiply.
        assert_eq!(Fnv64::new().finish(), 0xcbf2_9ce4_8422_2325);
        let mut h = Fnv64::new();
        h.eat(b"a");
        let want = (0xcbf2_9ce4_8422_2325u64 ^ 0x61).wrapping_mul(0x1_0000_01B3);
        assert_eq!(h.finish(), want);
        let mut d = Fnv64::new();
        d.debug(&"a");
        let mut e = Fnv64::new();
        e.eat(b"\"a\"");
        assert_eq!(d.finish(), e.finish());
    }

    #[derive(Debug)]
    #[allow(dead_code)] // read through Debug only
    struct Look {
        effects: Vec<String>,
        sound: Option<String>,
        radius: f32,
    }

    #[derive(Debug)]
    #[allow(dead_code)] // read through Debug only
    struct Gun {
        name: String,
        fire_sound: Option<String>,
        look: Look,
        damage: f32,
    }

    fn gun(sound: &str, effects: &[&str], damage: f32) -> Gun {
        Gun {
            name: "rifle".into(),
            fire_sound: (!sound.is_empty()).then(|| sound.into()),
            look: Look {
                effects: effects.iter().map(|e| (*e).into()).collect(),
                sound: Some(sound.into()),
                radius: 2.0,
            },
            damage,
        }
    }

    /// The simulation's view of a table drops sound and effect names at any
    /// depth, and keeps every other field.
    #[test]
    fn debug_sim_ignores_presentation_names_only() {
        let digest = |g: &Gun| {
            let mut f = Fnv64::new();
            f.debug_sim(g);
            f.finish()
        };
        let a = digest(&gun("a_fire", &["boom", "smoke"], 10.0));
        assert_eq!(a, digest(&gun("", &[], 10.0)));
        assert_eq!(a, digest(&gun("b_fire", &["flash"], 10.0)));
        assert_ne!(a, digest(&gun("a_fire", &["boom", "smoke"], 11.0)));
        let mut wider = gun("a_fire", &[], 10.0);
        wider.look.radius = 3.0;
        assert_ne!(
            a,
            digest(&wider),
            "a field after a dropped one still counts"
        );
    }
}
