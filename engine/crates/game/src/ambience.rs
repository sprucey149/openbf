//! The world's fixed sound emitters (card AMBS, `analysis/re-ambience-music.md`
//! sections 1-2 `[O]`): the battle's ambience streams and the loops on
//! objects and props, listed once from the battle's manifest (the mission's
//! layer merged) and its classes, and each one's level at the listener. The
//! voices are `crate::sounds::SoundLib::ambient_voices`'; the dev referee's
//! rows are `crate::event_sounds::SoundLog`'s. Presentation only.
//!
//! Three sources, as the PS2 makes them:
//! - a world region named `soundstream <property> <divisor>` (or
//!   `soundstatic`, which no disc authors): a looping 3-D sound at the
//!   region's centre with MinDistance = w / divisor, MaxDistance =
//!   MuteDistance = w, w = |SIZE| (B1 `FUN_0035c2f8`, B2 `FUN_00441c40`);
//! - a world object of a `SoundAmbienceStatic` / `SoundAmbienceStreaming`
//!   class (`com_snd_amb_static`): its `Sound` with the object's
//!   `MinDistance`, and its `MaxDistance` as Max AND Mute (class defaults
//!   1.0 / 10.0; flags 7 on every such entity in RAM);
//! - a `prop` / `building` / `animatedprop` whose class has a
//!   `SoundProperty`: that sound with its OWN distances, at the prop (BF1
//!   `FUN_001fa400`) or at the prop's matrix x `SoundOffset` (BF2
//!   `FUN_0022ef70`).
//!
//! All of them start at battle start and are never stopped (BF2 restarts a
//! stopped stream entity every update; a static entity re-asks a sound whose
//! handle went idle, so a one-shot repeats at its own `PlayInterval`).
//! Deviation: a BF2 prop's loop starts with the battle, not the first time
//! the prop is drawn - out of the sound's range it is silent either way.

use crate::classdb::{fnv1a_lowercase, ClassDb};
use glam::Vec3;
use manifest::{Manifest, ManifestInstance, ManifestRegion};
use std::path::Path;

/// Where an emitter comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A `soundstream` region (a streamed property).
    Stream,
    /// A `soundstatic` region (a sample property; none on either disc).
    Static,
    /// A `com_snd_amb_static` / `_streaming` world object.
    Object,
    /// A prop or building whose class has a `SoundProperty`.
    Prop,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Stream => "stream",
            Kind::Static => "static",
            Kind::Object => "object",
            Kind::Prop => "prop",
        }
    }
}

/// One fixed emitter: the property it plays, where, and the entity's
/// distances where the PS2 sets them (`None` = the property's own).
#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    pub kind: Kind,
    /// The property's name as the world or class spells it.
    pub sound: String,
    pub hash: u32,
    pub pos: Vec3,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub mute: Option<f32>,
}

/// The instance keys of a sound entity (B1 `FUN_0035bf08`, B2 `FUN_00441b48`).
const SOUND: &str = "Sound";
const SOUND_STREAM: &str = "SoundStream";
const MIN_DISTANCE: &str = "MinDistance";
const MAX_DISTANCE: &str = "MaxDistance";
/// The prop class keys (B1 `FUN_001fb6e8`, B2 class+0x304 / +0x320).
const SOUND_PROPERTY: &str = "SoundProperty";
const SOUND_OFFSET: &str = "SoundOffset";

/// The sound entity's constructor defaults (B1 `FUN_0035bcd0`).
const DEFAULT_MIN: f32 = 1.0;
const DEFAULT_MAX: f32 = 10.0;

/// The engine labels whose prop setter reads `SoundProperty`: BF1's prop
/// class serves `prop` and `building` (B1 15553-15557); BF2's prop render
/// and `animatedprop` update play it. An `armedbuilding` or
/// `destructablebuilding` never sees the key.
const PROP_LABELS: [&str; 3] = ["prop", "building", "animatedprop"];

/// The engine labels of the two sound object classes.
const OBJECT_LABELS: [&str; 2] = ["soundambiencestatic", "soundambiencestreaming"];

/// A region's emitter, when its name is `soundstream|soundstatic <sound>
/// [<divisor>]` (the PS2 reads `" %s %f"` after the type word; a divisor
/// that is missing or not above 0 leaves MinDistance the property's).
pub fn region_emitter(r: &ManifestRegion) -> Option<Emitter> {
    let mut words = r.name.split_whitespace();
    let kind = match words.next()?.to_ascii_lowercase().as_str() {
        "soundstream" => Kind::Stream,
        "soundstatic" => Kind::Static,
        _ => return None,
    };
    let sound = words.next()?.to_owned();
    let div = words.next().and_then(|w| w.parse::<f32>().ok());
    let w = Vec3::from(r.size).length();
    Some(Emitter {
        kind,
        hash: fnv1a_lowercase(&sound),
        sound,
        pos: Vec3::from(r.translation),
        min: div.filter(|d| *d > 0.0).map(|d| w / d),
        max: Some(w),
        mute: Some(w),
    })
}

/// The engine label at the root of `class`'s chain, lower case.
fn label(db: &ClassDb, class: &str) -> Option<String> {
    db.chain(class).last().map(|d| d.base.to_ascii_lowercase())
}

fn inst_prop<'a>(i: &'a ManifestInstance, key: &str) -> Option<&'a str> {
    let h = fnv1a_lowercase(key);
    i.props
        .iter()
        .find(|(k, _)| *k == h)
        .map(|(_, v)| v.as_str())
}

/// An object's or a prop's emitter (`bf2` picks the prop's sound position).
pub fn instance_emitter(i: &ManifestInstance, db: &ClassDb, bf2: bool) -> Option<Emitter> {
    let label = label(db, &i.class)?;
    let num = |s: Option<&str>| s.and_then(|v| v.trim().parse::<f32>().ok());
    if OBJECT_LABELS.contains(&label.as_str()) {
        let key = |k: &str| inst_prop(i, k).or_else(|| db.prop(&i.class, k));
        let sound = key(SOUND).or_else(|| key(SOUND_STREAM))?.to_owned();
        let min = num(key(MIN_DISTANCE)).unwrap_or(DEFAULT_MIN);
        let max = num(key(MAX_DISTANCE)).unwrap_or(DEFAULT_MAX);
        return Some(Emitter {
            kind: Kind::Object,
            hash: fnv1a_lowercase(&sound),
            sound,
            pos: Vec3::from(i.translation),
            min: Some(min),
            max: Some(max),
            mute: Some(max),
        });
    }
    if !PROP_LABELS.contains(&label.as_str()) {
        return None;
    }
    let sound = db.prop(&i.class, SOUND_PROPERTY)?.trim().to_owned();
    if sound.is_empty() {
        return None;
    }
    let mut pos = Vec3::from(i.translation);
    if bf2 {
        let off = db.prop(&i.class, SOUND_OFFSET).and_then(|s| {
            let v: Vec<f32> = s
                .split_whitespace()
                .filter_map(|w| w.parse().ok())
                .collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        });
        if let Some(off) = off {
            let m = crate::viewer::instance_matrix(i.rotation, i.translation);
            pos = m.transform_point3(off);
        }
    }
    Some(Emitter {
        kind: Kind::Prop,
        hash: fnv1a_lowercase(&sound),
        sound,
        pos,
        min: None,
        max: None,
        mute: None,
    })
}

/// Every emitter of a battle's manifest (its mode layer already merged,
/// `crate::assets::Scene::apply_mode_layer`): regions first, then the
/// instances in world order.
pub fn emitters(m: &Manifest, db: &ClassDb) -> Vec<Emitter> {
    let bf2 = m.source_game.eq_ignore_ascii_case("bf2");
    let mut out: Vec<Emitter> = m.regions.iter().filter_map(region_emitter).collect();
    out.extend(
        m.instances
            .iter()
            .filter_map(|i| instance_emitter(i, db, bf2)),
    );
    out
}

/// The classes a battle's world objects take: the map's own, then the
/// game's shared file (`com_snd_amb_static`, `com_amb_fire`).
pub fn class_db(map_dir: &Path, classes_dir: &Path, game: &str) -> ClassDb {
    let mut db = ClassDb::default();
    if let Err(e) = db.load_map(map_dir) {
        eprintln!("ambience: {e:#}");
    }
    for f in crate::roster::class_files(&game.to_ascii_lowercase(), &[]) {
        let p = classes_dir.join(f);
        if pak::exists(&p) {
            if let Err(e) = db.load(&p) {
                eprintln!("ambience: {e:#}");
            }
        }
    }
    db
}

/// The property `p` with the emitter's distances in place of its own.
pub fn with_distances(e: &Emitter, p: &manifest::SoundProp) -> manifest::SoundProp {
    let mut p = p.clone();
    if let Some(v) = e.min {
        p.min_distance = v;
    }
    if let Some(v) = e.max {
        p.max_distance = v;
    }
    if let Some(v) = e.mute {
        p.mute_distance = Some(v);
    }
    p
}

/// A battle's emitters.
#[derive(Debug, Clone, Default)]
pub struct Ambience {
    pub emitters: Vec<Emitter>,
}

impl Ambience {
    /// The battle of `m` (its layer merged) on the map baked at `map_dir`.
    pub fn load(m: &Manifest, map_dir: &Path, classes_dir: &Path) -> Ambience {
        let db = class_db(map_dir, classes_dir, &m.source_game);
        Ambience {
            emitters: emitters(m, &db),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundTrigger {
    pub name: u32,
    pub region: core::region::Region,
}

pub fn sound_triggers(m: &Manifest) -> Vec<SoundTrigger> {
    m.regions
        .iter()
        .filter_map(|r| {
            let mut words = r.name.split_whitespace();
            if !words.next()?.eq_ignore_ascii_case("soundtrigger") {
                return None;
            }
            let name = crate::classdb::fnv1a_lowercase(words.next()?);
            let region =
                core::region::Region::from_rows(&r.shape, r.rotation, r.translation, r.size)?;
            Some(SoundTrigger { name, region })
        })
        .collect()
}

pub fn trigger_side(faction: &str) -> Option<usize> {
    match faction.to_ascii_lowercase().as_str() {
        "all" => Some(0),
        "imp" => Some(1),
        "rep" => Some(2),
        "cis" => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(name: &str, size: [f32; 3]) -> ManifestRegion {
        ManifestRegion {
            shape: "sphere".into(),
            name: name.into(),
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [1.0, 2.0, 3.0],
            size,
            props: Vec::new(),
        }
    }

    #[test]
    fn the_keys_are_the_ps2s_hashes() {
        // B1 `FUN_0035bf08`: Sound 0x0e0d9594, SoundStream 0x2e93ef4c;
        // B1 `FUN_001fb6e8`: SoundProperty 0xf350b4e5.
        assert_eq!(fnv1a_lowercase(SOUND), 0x0e0d_9594);
        assert_eq!(fnv1a_lowercase(SOUND_STREAM), 0x2e93_ef4c);
        assert_eq!(fnv1a_lowercase(SOUND_PROPERTY), 0xf350_b4e5);
        assert_eq!(fnv1a_lowercase(SOUND_OFFSET), 0xe6de_db8f);
    }

    #[test]
    fn a_region_name_gives_the_distances() {
        // Utapau's wind: SIZE whose length is 471.40, divisor 2.
        let e = region_emitter(&region("soundstream uta_amb_wind 2", [471.4, 0.0, 0.0])).unwrap();
        assert_eq!(e.kind, Kind::Stream);
        assert_eq!(e.sound, "uta_amb_wind");
        assert!((e.min.unwrap() - 235.7).abs() < 1e-3);
        assert_eq!(e.max, Some(471.4));
        assert_eq!(e.mute, Some(471.4));
        assert_eq!(e.pos, Vec3::new(1.0, 2.0, 3.0));
        // No divisor: the property's own minimum.
        let e = region_emitter(&region("soundstream x", [3.0, 4.0, 0.0])).unwrap();
        assert_eq!((e.min, e.max), (None, Some(5.0)));
        assert!(region_emitter(&region("soundspace stonehead", [1.0; 3])).is_none());
        assert!(region_emitter(&region("cp1_capture", [1.0; 3])).is_none());
    }

    /// Utapau's wind with the region's three distances in place of the
    /// property's own (5 / 50 m, RollOff 0): 1 inside 235.7 m, linear to 0
    /// at 471.4 m (re-ambience-music 1.10).
    #[test]
    fn the_region_distances_replace_the_propertys() {
        let p = manifest::SoundProp {
            name_hash: fnv1a_lowercase("uta_amb_wind"),
            kind: 0,
            gain: 0.5,
            pitch: 1.0,
            min_distance: 5.0,
            max_distance: 50.0,
            rolloff: 0.0,
            priority: 0.7,
            looping: true,
            samples: vec![],
            layers: vec![],
            stream: None,
            segments: vec![],
            mute_distance: None,
            roll_in_distance: None,
            flat: false,
            layer_delays: vec![],
            pitch_dev: None,
            gain_dev: None,
            play_interval: None,
            play_probability: None,
            level: 0,
            bus: 0,
            take_locks: vec![],
        };
        let e = region_emitter(&region("soundstream uta_amb_wind 2", [471.4, 0.0, 0.0])).unwrap();
        let p = with_distances(&e, &p);
        let at = |d: f32| crate::sounds::attenuation(d, &p);
        assert_eq!(at(100.0), 1.0);
        assert_eq!(at(235.7), 1.0);
        assert!((at(353.55) - 0.5).abs() < 1e-4, "{}", at(353.55));
        assert!((at(412.475) - 0.25).abs() < 1e-4, "{}", at(412.475));
        assert_eq!(at(471.4), 0.0);
        assert_eq!(at(600.0), 0.0);
    }
}
