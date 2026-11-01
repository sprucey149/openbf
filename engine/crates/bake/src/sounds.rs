//! Bake a SOUND/<PLANET>.LVL (per-map sound level: sample banks + `snd_`
//! property scripts + music/VO streams) to WAVs + `sounds.ron`. The game
//! looks properties up by the fnv1a_lowercase hash of the class-prop string
//! (FireSound = "all_weap_inf_rifle_fire"), picks a weighted sample and
//! attenuates by the property's min/max distance + rolloff.
//!
//! Card FOLY: the LVL's `ffx_` foley tables go to `sounds/foley.ron`
//! (`bake::foley`).
//!
//! Since queue item VO it also bakes the LVL's OWN streams into
//! `<mapdir>/sounds/streams/` — the mono `<map>_emt` takes to OGG, the stereo
//! music kept as encoded ADPCM bodies for streaming
//! (`bake::global_sound::bake_map_streams`).

use anyhow::{anyhow, bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod repairs;

/// The RON's two structs live in the `manifest` crate: ONE definition,
/// deriving `Serialize` for this side and `Deserialize` for `game::sounds`,
/// which reads the file back and cannot depend on this crate (game/Cargo.toml
/// has `bake` as a DEV-dependency only). Before this they were hand-kept
/// twins. Field order there is the wire format.
pub use manifest::{
    EngineGraph, EngineLayer, EngineSound, MusicRon, SoundProp as SoundPropRon, SoundsRon,
};

/// Defaults when no link of a 3-D property's template chain sets a field
/// (162 stream properties on the two discs; the 2-D ones never read them).
/// PROVISIONAL (ours): no class-constructor default has been traced for any
/// of the three.
pub const DEFAULT_MIN_DISTANCE: f32 = 5.0;
pub const DEFAULT_MAX_DISTANCE: f32 = 50.0;
pub const DEFAULT_ROLLOFF: f32 = 1.0;

/// Every `snd_` definition of one LVL, for use as `Inherit` templates.
fn read_props(lvl: &str) -> Result<Vec<mungefs::sound_props::SoundProp>> {
    let buf = std::fs::read(lvl).with_context(|| lvl.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    Ok(mungefs::sound_props::find_all(inner))
}

/// A definition beside its sub-level's name hash (0 outside one).
type Leveled = (u32, mungefs::sound_props::SoundProp);

/// Every `snd_` definition of a payload beside its sub-level's name hash (0
/// outside one), each layered record holding the
/// `Layer` entries the PS2's parser keeps (sub-level by sub-level, against
/// that sub-level's own file order and `core`'s properties), how many
/// entries were dropped as (late, absent), and every sub-level's
/// `soundparameterized` records resolved ([`engines`]).
fn read_sublevels(
    inner: &[u8],
    core: &[mungefs::sound_props::SoundProp],
) -> (Vec<Leveled>, (usize, usize), Vec<EngineSound>) {
    let kind = mungefs::fnv1a_lowercase("soundproperties");
    let core: std::collections::HashSet<u32> = core
        .iter()
        .filter(|p| p.kind == kind)
        .map(|p| p.name_hash)
        .collect();
    let (mut out, mut dropped, mut eng) = (Vec::new(), (0, 0), Vec::new());
    for (name, mut sub) in mungefs::sound_props::find_named_sublevels(inner) {
        let (late, absent) = mungefs::sound_props::drop_unresolved_layers(&mut sub, &core);
        dropped = (dropped.0 + late, dropped.1 + absent);
        eng.extend(engines(name, &sub));
        out.extend(sub.into_iter().map(|p| (name, p)));
    }
    (out, dropped, eng)
}

/// The engine component's parameter names (B1 table 0x547e00, BF2's 12th;
/// `time` is the BF2 player's own) and the targets a graph can multiply (B1
/// table 0x6754f8): re-vehicle-sounds.md §1.5, §3.2.
const PARAMETERS: [&str; 13] = [
    "speed",
    "acceleration",
    "health",
    "boost",
    "revs",
    "height",
    "turnontime",
    "turnofftime",
    "strain",
    "winduptime",
    "winddowntime",
    "proximity",
    "time",
];
const TARGETS: [&str; 10] = [
    "Gain",
    "Pitch",
    "ReverbGain",
    "Pan",
    "MinDistance",
    "MuteDistance",
    "MaxDistance",
    "RollOff",
    "RollIn",
    "RollInDistance",
];

fn named(hash: u32, names: &[&str]) -> String {
    let n = names.iter().find(|n| mungefs::fnv1a_lowercase(n) == hash);
    n.map_or_else(|| format!("{hash:08x}"), |n| (*n).to_owned())
}

/// One sub-level's `soundparameterized` records as the PS2 binds them
/// (re-vehicle-sounds.md §1.2-1.3, §2.1): the layered record and every graph
/// are looked up while the record is parsed, so only ones defined EARLIER
/// in the sub-level are found; the voices are the layered record's layers
/// in its order, and a `Layer` scope's graphs go to the first layer of its
/// NAME (a name the layered record does not have reaches no voice). A
/// record whose layered record is not found binds nothing and is left out.
fn engines(level: u32, sub: &[mungefs::sound_props::SoundProp]) -> Vec<EngineSound> {
    let h = mungefs::fnv1a_lowercase;
    let (k_par, k_layered, k_graph) = (
        h("soundparameterized"),
        h("soundlayered"),
        h("parametergraph"),
    );
    let mut out = Vec::new();
    for (i, p) in sub.iter().enumerate().filter(|(_, p)| p.kind == k_par) {
        let earlier = |kind: u32, name: u32| {
            sub[..i]
                .iter()
                .find(|q| q.kind == kind && q.name_hash == name)
        };
        let Some(layered) = p.layered.and_then(|l| earlier(k_layered, l)) else {
            continue;
        };
        let mut layers: Vec<EngineLayer> = layered
            .layers
            .iter()
            .map(|l| EngineLayer {
                sound: l.sound,
                delay: l.delay,
                deviation: l.deviation,
                graphs: Vec::new(),
            })
            .collect();
        for gl in &p.graph_layers {
            let Some(k) = layered.layers.iter().position(|l| l.name == gl.name) else {
                continue;
            };
            for (parameter, graph, target) in &gl.graphs {
                // A graph that is not found multiplies nothing (B1
                // `FUN_004610b8` 524173: the node has no graph).
                let Some(g) = earlier(k_graph, *graph) else {
                    continue;
                };
                layers[k].graphs.push(EngineGraph {
                    parameter: named(*parameter, &PARAMETERS),
                    target: named(*target, &TARGETS),
                    points: g.points.clone(),
                });
            }
        }
        out.push(EngineSound {
            name_hash: p.name_hash,
            level,
            layers,
        });
    }
    out
}

/// The LVL's `Music` records, the first of each name (its `lvl_` sub-levels
/// repeat them, equal on both discs).
fn music(inner: &[u8]) -> Vec<MusicRon> {
    let mut seen = std::collections::HashSet::new();
    mungefs::sound_props::find_music(inner)
        .into_iter()
        .filter(|m| seen.insert(m.name_hash))
        .map(|m| MusicRon {
            name_hash: m.name_hash,
            stream: m.stream,
            fade_in: m.fade_in,
            fade_out: m.fade_out,
            min_playback: m.min_playback,
            min_inactive: m.min_inactive,
            max_playback: m.max_playback,
            priority: m.priority,
        })
        .collect()
}

pub(crate) fn find_common_bnk(lvl_path: &Path) -> Option<std::path::PathBuf> {
    let parent = lvl_path.parent()?;
    ["common.bnk", "sound/common.bnk"]
        .into_iter()
        .map(|rel| crate::join_ci(parent, rel))
        .find(|p| p.is_file())
}

pub(crate) fn common_bnk_samples(bnk_buf: &[u8]) -> HashMap<u32, (u32, u32, u32, &[u8])> {
    let mut out = HashMap::new();
    let Ok(inner) = mungefs::open(bnk_buf) else {
        return out;
    };
    for b in mungefs::soundbank::find_banks_deep(inner) {
        for s in &b.samples {
            if let Some(slice) = b.data.get(s.offset..s.offset.saturating_add(s.size)) {
                out.insert(s.id_hash, (b.format, s.frequency, s.frames, slice));
            }
        }
    }
    out
}

/// `sound_lvl`'s banks and properties under `<outdir>/sounds`. `core_lvl` is
/// the game's resident CORE.LVL: a property's `Inherit` template is looked up
/// in its own LVL, then there. Nearly every template lives in CORE (58,459
/// of the 59,808 `Inherit`s on the two discs name a hash their own LVL does
/// not define; with CORE none is left unresolved - card SNDT), and without
/// it a weapon's fire kept gain 1.0 and the 5 - 50 m default.
pub fn bake_sounds(sound_lvl: &str, outdir: &str, core_lvl: Option<&str>) -> Result<SoundsRon> {
    let buf = std::fs::read(sound_lvl).with_context(|| sound_lvl.to_string())?;
    let inner = mungefs::open(&buf).map_err(|e| anyhow!("{e}"))?;
    let out = Path::new(outdir).join("sounds");
    std::fs::create_dir_all(&out)?;

    let bnk_buf = find_common_bnk(Path::new(sound_lvl)).and_then(|p| std::fs::read(p).ok());
    let common_samples = bnk_buf
        .as_deref()
        .map(common_bnk_samples)
        .unwrap_or_default();

    let banks = mungefs::soundbank::find_banks_deep(inner);
    let mut samples: Vec<(u32, String)> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut bank_list = Vec::new();
    for b in &banks {
        bank_list.push((b.name_hash, b.samples.len()));
        let bank_dir = out.join(format!("{:08x}", b.name_hash));
        std::fs::create_dir_all(&bank_dir)?;
        // Aliases share the target's WAV; real samples are written once per id.
        let mut written: HashMap<u32, String> = HashMap::new();
        for s in b.samples.iter().filter(|s| s.alias_of.is_none()) {
            if seen.contains(&s.id_hash) {
                continue;
            }
            let (format, rate, data_slice) = if !b.data.is_empty() {
                let end = s.offset + s.size;
                if end > b.data.len() {
                    bail!(
                        "sample {:08x} outside bank {:08x} data",
                        s.id_hash,
                        b.name_hash
                    );
                }
                (b.format, s.frequency, &b.data[s.offset..end])
            } else if let Some(&(fmt, freq, _, slice)) = common_samples.get(&s.id_hash) {
                (fmt, freq, slice)
            } else {
                bail!(
                    "sample {:08x} in bank {:08x} has no local data and was not found in common.bnk",
                    s.id_hash,
                    b.name_hash
                );
            };
            let pcm = crate::adpcm::decode_bank_sample(format, data_slice);
            let wav = crate::adpcm::wav_mono_s16(rate, &pcm);
            let rel = format!("{:08x}/{:08x}.wav", b.name_hash, s.id_hash);
            std::fs::write(out.join(&rel), wav)?;
            seen.insert(s.id_hash);
            written.insert(s.id_hash, rel.clone());
            samples.push((s.id_hash, rel));
        }
        for s in b.samples.iter().filter(|s| s.alias_of.is_some()) {
            if seen.contains(&s.id_hash) {
                continue;
            }
            let target = s.alias_of.unwrap();
            let rel = match written.get(&target) {
                Some(r) => r.clone(),
                None => match samples.iter().find(|(id, _)| *id == target) {
                    Some((_, r)) => r.clone(),
                    None => bail!("alias {:08x} -> {:08x} unresolved", s.id_hash, target),
                },
            };
            seen.insert(s.id_hash);
            samples.push((s.id_hash, rel));
        }
    }

    // Properties with inheritance flattened (template chain via `inherit`).
    let core = match core_lvl {
        Some(c) => read_props(c)?,
        None => Vec::new(),
    };
    let (raw, (late, absent), engines) = read_sublevels(inner, &core);
    if late + absent > 0 {
        eprintln!("sounds: {sound_lvl}: {late} late + {absent} absent Layer entries dropped");
    }
    let props = flatten(&raw, &core);
    repairs::tantive_female_aliases(Path::new(sound_lvl), &banks, &props, &mut samples)?;
    let odd_frequency = banks
        .iter()
        .flat_map(|b| b.samples.iter())
        .filter(|s| s.alias_of.is_none() && !(4000..=48000).contains(&s.frequency))
        .count();
    let streamed = props.iter().filter(|p| !p.segments.is_empty()).count();
    let ron_out = SoundsRon {
        odd_frequency,
        streamed,
        source: Path::new(sound_lvl)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        banks: bank_list,
        samples,
        props,
        engines,
        music: music(inner),
    };
    crate::write_ron(&out.join("sounds.ron"), &ron_out)?;
    // The soldier foley table (card FOLY); CORE.LVL has none.
    crate::foley::write(inner, &out)?;

    // The map's OWN streams (queue item VO). The same LVL that holds these
    // banks also holds the map's music and its mono `<map>_emt` stream as
    // StreamList chunks the bank walk treats as leaves, and the `snd_`
    // above names segments of both — ids that are in NO GLOBAL/CW/GCW stream,
    // so they resolved to silence until now. The emptiness check is here
    // rather than inside the bake because this buffer is already open, and
    // because CORE.LVL (also baked by this function) carries no streams at
    // all.
    if !mungefs::stream::find_streams(inner).is_empty() {
        crate::global_sound::bake_map_streams(sound_lvl, &out.join("streams"))?;
    } else {
        crate::remove_optional_file(&out.join("streams").join(crate::global_sound::MAP_INDEX))?;
    }
    Ok(ron_out)
}

/// `raw` with every field resolved down its `Inherit` chain: the first link
/// that sets a field wins, as the PS2 copies the template in and then reads
/// the definition's own keys over it (B1 `FUN_0044d098`). A template is
/// looked up in the record's own sub-level (the one the PS2 has loaded, card
/// SNDE), then anywhere in `raw`, then in `core`; no LVL redefines a CORE
/// name that anything inherits (measured, both discs), so that order cannot
/// matter. Each record keeps its sub-level (`level`).
fn flatten(raw: &[Leveled], core: &[mungefs::sound_props::SoundProp]) -> Vec<SoundPropRon> {
    type Raw = mungefs::sound_props::SoundProp;
    let own: HashMap<(u32, u32), &Raw> = raw.iter().map(|(l, p)| ((*l, p.name_hash), p)).collect();
    let any: HashMap<u32, &Raw> = raw.iter().map(|(_, p)| (p.name_hash, p)).collect();
    let core: HashMap<u32, &Raw> = core.iter().map(|p| (p.name_hash, p)).collect();
    fn resolve<'a, T>(
        p: &'a Raw,
        parent: &dyn Fn(&'a Raw) -> Option<&'a Raw>,
        pick: &dyn Fn(&Raw) -> Option<T>,
    ) -> Option<T> {
        let mut cur = Some(p);
        for _ in 0..8 {
            let c = cur?;
            if let Some(v) = pick(c) {
                return Some(v);
            }
            cur = parent(c);
        }
        None
    }
    raw.iter()
        .map(|(level, p)| {
            let parent = |c: &Raw| -> Option<&Raw> {
                let h = c.inherit?;
                let found = own.get(&(*level, h)).or_else(|| any.get(&h));
                found.or_else(|| core.get(&h)).copied()
            };
            let max_distance =
                resolve(p, &parent, &|c| c.max_distance).unwrap_or(DEFAULT_MAX_DISTANCE);
            SoundPropRon {
                name_hash: p.name_hash,
                kind: p.kind,
                gain: resolve(p, &parent, &|c| c.gain).unwrap_or(1.0),
                pitch: resolve(p, &parent, &|c| c.pitch).unwrap_or(1.0),
                min_distance: resolve(p, &parent, &|c| c.min_distance)
                    .unwrap_or(DEFAULT_MIN_DISTANCE),
                max_distance,
                rolloff: resolve(p, &parent, &|c| c.rolloff).unwrap_or(DEFAULT_ROLLOFF),
                priority: resolve(p, &parent, &|c| c.priority).unwrap_or(0.5),
                looping: resolve(p, &parent, &|c| c.looping).unwrap_or(false),
                samples: p.samples.clone(),
                layers: p.layers.iter().map(|l| l.sound).collect(),
                stream: p.stream,
                segments: p.segments.clone(),
                mute_distance: resolve(p, &parent, &|c| c.mute_distance)
                    .filter(|m| *m != max_distance),
                roll_in_distance: resolve(p, &parent, &|c| c.roll_in_distance).filter(|r| *r > 0.0),
                // No `mode3d` anywhere in the chain reads as 2-D: PROVISIONAL
                // [H] (the menu's 259 BF2 stream properties and the layered /
                // bus records are the only ones without it).
                flat: !resolve(p, &parent, &|c| c.mode_3d).unwrap_or(false),
                layer_delays: if p
                    .layers
                    .iter()
                    .any(|l| (l.delay, l.deviation) != (0.0, 0.0))
                {
                    p.layers.iter().map(|l| (l.delay, l.deviation)).collect()
                } else {
                    Vec::new()
                },
                pitch_dev: resolve(p, &parent, &|c| c.pitch_dev).filter(|v| *v != 0.0),
                gain_dev: resolve(p, &parent, &|c| c.gain_dev).filter(|v| *v != 0.0),
                play_interval: Some((
                    resolve(p, &parent, &|c| c.play_interval).unwrap_or(0.0),
                    resolve(p, &parent, &|c| c.play_interval_dev).unwrap_or(0.0),
                ))
                .filter(|v| *v != (0.0, 0.0)),
                play_probability: resolve(p, &parent, &|c| c.play_probability)
                    .filter(|v| *v != 1.0),
                level: *level,
                bus: resolve(p, &parent, &|c| c.bus).unwrap_or(0),
                take_locks: take_locks(p),
            }
        })
        .collect()
}

/// The locks of the takes a property plays (its samples when it has any,
/// else its segments; card UCALL); empty when none is locked.
fn take_locks(p: &mungefs::sound_props::SoundProp) -> Vec<(f32, f32)> {
    let locks = if p.samples.is_empty() {
        &p.segment_locks
    } else {
        &p.sample_locks
    };
    if locks.iter().any(|l| *l != (0.0, 0.0)) {
        locks.clone()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_direct_rebake_removes_only_owned_optional_audio_indexes() {
        let out = std::env::temp_dir().join(format!("openbf-empty-sounds-{}", std::process::id()));
        std::fs::create_dir_all(out.join("sounds/streams")).unwrap();
        let source = out.join("empty.lvl");
        std::fs::write(&source, b"ucfb\0\0\0\0").unwrap();
        let foley = out.join("sounds/foley.ron");
        let streams = out.join("sounds/streams/streams.ron");
        let body = out.join("sounds/streams/retained.adpcm");
        let unrelated = out.join("sounds/global.ron");
        for path in [&foley, &streams, &body, &unrelated] {
            std::fs::write(path, b"old").unwrap();
        }
        for _ in 0..2 {
            // Both stale and already absent cases succeed.
            let baked = bake_sounds(source.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
            assert!(baked.samples.is_empty());
            assert!(!foley.exists());
            assert!(!streams.exists());
            assert_eq!(std::fs::read(&body).unwrap(), b"old");
            assert_eq!(std::fs::read(&unrelated).unwrap(), b"old");
        }
        // Genuine removal failures must not look like a successful rebake.
        std::fs::create_dir(&foley).unwrap();
        assert!(bake_sounds(source.to_str().unwrap(), out.to_str().unwrap(), None).is_err());
        std::fs::remove_dir(&foley).unwrap();
        std::fs::create_dir(&streams).unwrap();
        assert!(bake_sounds(source.to_str().unwrap(), out.to_str().unwrap(), None).is_err());
        std::fs::remove_dir_all(out).unwrap();
    }

    /// Data-gated Tier-1: BF1 SOUND/TAT.LVL bakes ~1600 samples and the
    /// rifle fire property points at an existing WAV.
    #[test]
    fn tat_sound_lvl_bakes() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        );
        if !Path::new(p).exists() {
            eprintln!("SKIP: SOUND/TAT.LVL not present");
            return;
        }
        // Unique per run: the parallel test runner shared one path with
        // whatever else was running, and its `remove_dir_all` raced them.
        let out = std::env::temp_dir().join(format!(
            "openbf-sounds-tat-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos())
        ));
        let _ = std::fs::remove_dir_all(&out);
        let r = bake_sounds(p, out.to_str().unwrap(), None).expect("bake");
        assert_eq!(r.banks.len(), 4);
        // The four variant banks (tat1/tat2 × era) repeat most ids: ~1620
        // entries dedupe to ~330 unique samples.
        assert!(r.samples.len() >= 300, "{} samples", r.samples.len());
        assert!(r.props.len() >= 2000);
        let h = mungefs::fnv1a_lowercase("all_weap_inf_rifle_fire");
        let rifle = r.props.iter().find(|p| p.name_hash == h).unwrap();
        assert_eq!(rifle.gain, 0.6);
        let (id, _) = rifle.samples[0];
        let rel = &r.samples.iter().find(|(i, _)| *i == id).unwrap().1;
        let wav = std::fs::read(out.join("sounds").join(rel)).unwrap();
        assert!(wav.len() > 1000 && &wav[0..4] == b"RIFF");
        // Baked without CORE.LVL the template is not found and the
        // defaults stand (the with-CORE numbers are pinned below).
        assert_eq!(
            (rifle.min_distance, rifle.max_distance, rifle.flat),
            (DEFAULT_MIN_DISTANCE, DEFAULT_MAX_DISTANCE, true)
        );
        // Streamed properties are carried, with no sample of their own: the
        // tactical chatter plays segments of BF1 GCW.LVL's `gcw_tac_vo`
        // (analysis/re-sound-global.md §3).
        let tac = r
            .props
            .iter()
            .find(|p| p.name_hash == mungefs::fnv1a_lowercase("all_inf_com_chatter_tac_follow"))
            .expect("tac follow");
        assert_eq!(tac.stream, Some(mungefs::fnv1a_lowercase("gcw_tac_vo")));
        assert_eq!(tac.segments.len(), 3);
        assert!(tac.samples.is_empty());
        assert!(r.streamed > 100, "{} streamed props", r.streamed);
        // A property has a stream exactly when it has segments — measured on
        // BF1 TAT and BF2 COR/END/MYG/TAT/YAV: 0 of 16,779 properties name
        // one without the other, so neither field needs the template chain
        // the gains and distances go through.
        assert!(r
            .props
            .iter()
            .all(|p| p.segments.is_empty() == p.stream.is_none()));
        let _ = std::fs::remove_dir_all(&out);
    }

    /// Card SNDT's acceptance, from the real discs: a property's `Inherit`
    /// template resolves through the game's CORE.LVL. Before, every weapon
    /// baked as gain 1.0, 5 - 50 m, rolloff 1.
    #[test]
    fn templates_resolve_through_core_lvl() {
        let e = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
        let find = |r: &SoundsRon, name: &str| -> SoundPropRon {
            let h = mungefs::fnv1a_lowercase(name);
            // The last definition, as `game::sounds::merge_props` keeps it.
            let p = r.props.iter().rev().find(|p| p.name_hash == h);
            p.unwrap_or_else(|| panic!("{name}")).clone()
        };
        let dist = |p: &SoundPropRon| (p.gain, p.min_distance, p.max_distance, p.rolloff);
        for (lvl, core, rolloff) in [
            ("bf1/sound/DATA/_LVL_PS2/SOUND/NAB.LVL", "bf1/lvl", 6.0),
            ("bf2/sound/DATA/_LVL_PS2/SOUND/UTA.LVL", "bf2/lvl", 3.5),
        ] {
            let (lvl, core) = (
                format!("{e}/{lvl}"),
                format!("{e}/{core}/DATA/_LVL_PS2/CORE.LVL"),
            );
            if !Path::new(&lvl).exists() || !Path::new(&core).exists() {
                eprintln!("SKIP: {lvl} or its CORE.LVL not present");
                continue;
            }
            let raw: Vec<_> = read_props(&lvl)
                .expect("lvl")
                .into_iter()
                .map(|p| (0, p))
                .collect();
            let props = flatten(&raw, &read_props(&core).expect("core"));
            let r = SoundsRon {
                source: String::new(),
                banks: vec![],
                samples: vec![],
                props,
                odd_frequency: 0,
                streamed: 0,
                engines: vec![],
                music: vec![],
            };
            let rifle = find(&r, "rep_weap_inf_rifle_fire");
            assert_eq!(dist(&rifle), (0.7, 5.0, 400.0, rolloff), "{lvl}");
            // Card MIXH: the bus through the template chain.
            let bus = |n: &str| mungefs::fnv1a_lowercase(n);
            assert_eq!(rifle.bus, bus("soundfx"), "{lvl}");
            let mut by_bus: std::collections::BTreeMap<String, usize> = Default::default();
            for p in &r.props {
                let n = [
                    "soundfx",
                    "voiceover",
                    "ambience",
                    "battlechatter",
                    "ingamemusic",
                ]
                .into_iter()
                .find(|n| bus(n) == p.bus)
                .map_or_else(|| format!("{:08x}", p.bus), str::to_owned);
                *by_bus.entry(n).or_default() += 1;
            }
            println!("{lvl}: properties by bus {by_bus:?}");
            // Every property with a take names a bus (the ones without are
            // layered / stream / bus records).
            let none = r.props.iter().filter(|p| p.bus == 0);
            assert_eq!(
                none.filter(|p| !(p.samples.is_empty() && p.segments.is_empty()))
                    .count(),
                0,
                "{lvl}"
            );
            assert_eq!(
                (rifle.mute_distance, rifle.roll_in_distance, rifle.flat),
                (None, None, false)
            );
            // Its own `mindistance` over the explosion template's 30 - 450 m.
            let det = find(&r, "com_weap_thermaldetonator_exp");
            assert_eq!((det.min_distance, det.max_distance), (40.0, 450.0), "{lvl}");
        }
        // Every template is found: over every SOUND LVL of both discs nothing
        // inherits a hash that is in neither its own LVL nor its CORE.
        let (mut lvls, mut inherits, mut lost) = (0, 0, 0);
        for game in ["bf1", "bf2"] {
            let core = format!("{e}/{game}/lvl/DATA/_LVL_PS2/CORE.LVL");
            let dir = format!("{e}/{game}/sound/DATA/_LVL_PS2/SOUND");
            let (Ok(core), Ok(dir)) = (read_props(&core), std::fs::read_dir(&dir)) else {
                eprintln!("SKIP: {game} sound LVLs not present");
                continue;
            };
            for f in dir.filter_map(|f| f.ok()).map(|f| f.path()) {
                let raw = read_props(f.to_str().unwrap()).expect("lvl");
                let names: std::collections::HashSet<u32> =
                    raw.iter().chain(&core).map(|p| p.name_hash).collect();
                lvls += 1;
                inherits += raw.iter().filter(|p| p.inherit.is_some()).count();
                lost += raw
                    .iter()
                    .filter(|p| p.inherit.is_some_and(|h| !names.contains(&h)))
                    .count();
            }
        }
        eprintln!("SNDT census: {lvls} SOUND LVLs, {inherits} Inherit, {lost} unresolved");
        assert_eq!(lost, 0, "unresolved Inherit");
    }

    /// Card VSND, from the disc: BF1 NAB.LVL's AAT engine record resolved as
    /// re-vehicle-sounds.md §1 reads it - three layers in the layered
    /// record's order, delay 0, layer 1's six graphs.
    #[test]
    fn engine_records_resolve_with_their_graphs() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/NAB.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: SOUND/NAB.LVL not present");
            return;
        };
        let (_, _, engines) = read_sublevels(mungefs::open(&buf).unwrap(), &[]);
        let h = mungefs::fnv1a_lowercase("cis_hover_aat_engine_parameterized");
        let aat = engines.iter().find(|e| e.name_hash == h).expect("aat");
        assert_eq!(aat.level, mungefs::fnv1a_lowercase("nab1cw"));
        // The fighter tank's graphs differ between NAB's sub-levels.
        let h = mungefs::fnv1a_lowercase("interceptorTank_engine_parameterized");
        let tank: Vec<&EngineSound> = engines.iter().filter(|e| e.name_hash == h).collect();
        let speed = |e: &EngineSound| e.layers[0].graphs[0].points[0];
        let of = |l: &str| tank.iter().find(|e| e.level == mungefs::fnv1a_lowercase(l));
        assert_eq!(of("nab1cw").map(|e| speed(e)), Some((0.0, 0.8)));
        assert_eq!(of("nab2gcw").map(|e| speed(e)), Some((0.0, 0.5)));
        let sounds: Vec<u32> = aat.layers.iter().map(|l| l.sound).collect();
        assert_eq!(sounds, [0x842C_3A54, 0xE053_9C6C, 0x5ACF_7D70]);
        assert!(aat
            .layers
            .iter()
            .all(|l| (l.delay, l.deviation) == (0.0, 0.0)));
        type Row<'a> = (&'a str, &'a str, &'a [(f32, f32)]);
        let g: Vec<Row> = aat.layers[0]
            .graphs
            .iter()
            .map(|g| (g.parameter.as_str(), g.target.as_str(), &g.points[..]))
            .collect();
        let want: [Row; 6] = [
            ("speed", "Pitch", &[(0.0, 0.8), (0.5, 0.9), (1.0, 1.0)]),
            (
                "acceleration",
                "Gain",
                &[(-1.0, 1.0), (0.0, 0.7), (1.0, 1.0)],
            ),
            ("turnontime", "Gain", &[(0.0, 0.0), (2.0, 1.0)]),
            ("turnontime", "Pitch", &[(0.0, 0.5), (3.0, 1.0)]),
            ("turnofftime", "Gain", &[(0.0, 1.0), (3.0, 0.0)]),
            (
                "turnofftime",
                "Pitch",
                &[(0.0, 1.0), (2.0, 0.6), (3.0, 0.3)],
            ),
        ];
        let mut got = g.clone();
        got.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let mut want = want.to_vec();
        want.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        assert_eq!(got, want);
        // Every parameter and target of the disc has a name.
        let hex = |s: &str| s.len() == 8 && s.bytes().all(|b| b.is_ascii_hexdigit());
        let graphs = engines
            .iter()
            .flat_map(|e| &e.layers)
            .flat_map(|l| &l.graphs);
        assert!(graphs.clone().count() > 100);
        assert!(graphs
            .clone()
            .all(|g| !hex(&g.parameter) && !hex(&g.target)));
    }

    /// The whole point of queue item 13: the writer and the reader are ONE
    /// struct now, so the file the bake shipped must survive a parse and a
    /// re-serialize UNCHANGED — byte for byte, through the same
    /// `PrettyConfig` `crate::write_ron` uses. A field renamed, reordered or
    /// dropped on either side fails here on real data, not on a fixture.
    /// Data-gated on `engine/assets_baked`.
    #[test]
    fn every_shipped_sounds_ron_re_serializes_byte_identically() {
        let root = Path::new(crate::BAKED_ROOT);
        if !root.exists() {
            eprintln!("SKIP: assets_baked not present");
            return;
        }
        let mut checked = 0usize;
        let mut dirs: Vec<_> = std::fs::read_dir(root)
            .expect("assets_baked")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for d in dirs {
            let f = d.join("sounds/sounds.ron");
            if !f.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&f).expect("read");
            let parsed: SoundsRon =
                ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            let again =
                ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default()).unwrap();
            assert_eq!(again, text, "{} re-serializes differently", f.display());
            checked += 1;
        }
        assert!(checked >= 30, "only {checked} sounds.ron found");
    }

    /// The RON grew `stream` and `segments`, and something built against the
    /// OLD shape must still load a file this bake writes. (The reader is no
    /// longer a twin — `game::sounds` shares this crate's struct through
    /// `manifest` since queue item 13 — but a stale `assets_baked` consumer,
    /// or a tool of the maintainer's, is still a real reader.)
    #[test]
    fn an_older_reader_still_loads_a_file_this_bake_writes() {
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct OldProp {
            name_hash: u32,
            gain: f32,
            samples: Vec<(u32, f32)>,
        }
        #[derive(serde::Deserialize)]
        struct OldRon {
            props: Vec<OldProp>,
        }
        let new = SoundPropRon {
            name_hash: 1,
            kind: 2,
            gain: 0.5,
            pitch: 1.0,
            min_distance: 1.0,
            max_distance: 2.0,
            rolloff: 1.0,
            priority: 0.5,
            looping: false,
            samples: vec![],
            layers: vec![],
            stream: Some(0xABCD),
            segments: vec![(7, 1.0)],
            mute_distance: Some(1.5),
            roll_in_distance: Some(0.5),
            flat: true,
            layer_delays: vec![],
            pitch_dev: Some(0.15),
            gain_dev: None,
            play_interval: Some((0.5, 0.0)),
            play_probability: None,
            level: 0,
            bus: 0,
            take_locks: vec![],
        };
        let text = ron::ser::to_string_pretty(
            &SoundsRon {
                source: "t".into(),
                banks: vec![],
                samples: vec![],
                props: vec![new.clone()],
                odd_frequency: 0,
                streamed: 1,
                engines: vec![],
                music: vec![],
            },
            ron::ser::PrettyConfig::default(),
        )
        .unwrap();
        let old: OldRon = ron::from_str(&text).expect("old reader on new file");
        assert_eq!(old.props[0].name_hash, 1);
        // The other direction — a reader built now on a file written before
        // the fields existed — is `manifest`'s
        // `a_pre_item_15_sounds_ron_still_loads`, where the struct now lives.
    }

    /// Data-gated Tier-1: the RESIDENT global bank — BF2 `lvl/CORE.LVL`'s
    /// `global` SampleBank + `snd_`, which is where `shell_menu_accept`
    /// (HOLD 9, answered) lives. Numbers from analysis/re-sound-global.md
    /// §0.6/§3, lead-verified: 7 samples, 68 definitions, the accept wave
    /// B75D27CD at 11,060 Hz / 2,704 ADPCM bytes, pitch 1.1.
    #[test]
    fn bf2_core_lvl_global_bank_bakes() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/CORE.LVL"
        );
        if !Path::new(p).exists() {
            eprintln!("SKIP: BF2 lvl/CORE.LVL not present");
            return;
        }
        let out = std::env::temp_dir().join(format!("openbf-sounds-core-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let r = bake_sounds(p, out.to_str().unwrap(), None).expect("bake");
        assert_eq!(r.banks, vec![(mungefs::fnv1a_lowercase("global"), 7)]);
        assert_eq!(r.props.len(), 68);
        assert_eq!(r.streamed, 0, "CORE.LVL is resident, not streamed");
        let accept = r
            .props
            .iter()
            .find(|p| p.name_hash == mungefs::fnv1a_lowercase("shell_menu_accept"))
            .expect("shell_menu_accept");
        assert_eq!(accept.samples, vec![(0xB75D_27CD, 1.0)]);
        assert_eq!(
            (accept.gain, accept.pitch, accept.looping),
            (1.0, 1.1, false)
        );
        // `shell_menu_enter` inherits the same definition.
        let enter = r
            .props
            .iter()
            .find(|p| p.name_hash == mungefs::fnv1a_lowercase("shell_menu_enter"))
            .expect("shell_menu_enter");
        assert_eq!(enter.pitch, 1.1);
        let rel = &r
            .samples
            .iter()
            .find(|(i, _)| *i == 0xB75D_27CD)
            .expect("accept wave")
            .1;
        let wav = std::fs::read(out.join("sounds").join(rel)).unwrap();
        // 2,704 ADPCM bytes -> 2704/16*28 frames of s16 + a 44-byte header.
        assert_eq!(wav.len(), 44 + 2704 / 16 * 28 * 2);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 11_060);
        let _ = std::fs::remove_dir_all(&out);
    }
}
