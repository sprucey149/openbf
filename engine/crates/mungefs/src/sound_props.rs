//! `snd_` sound-property scripts (analysis/re-sound-banks.md §snd_): a config
//! DATA/SCOP tree (mungefs::config) where a `soundproperties` record opens a
//! definition whose SCOP holds `name`, `gain`, `pitch`, distances, the
//! `samplelist` (0x4143ACAF → `sample(id, weight[, interval, deviation])`
//! records; the last two are a take's lock), the mixer `bus` and an
//! `inherit` template (0xCA04EFE0). Layered definitions (kind 0xA54A2138 on
//! TAT.LVL, e.g. reload sounds) list `Layer(layer name, sound, delay
//! [, deviation])` entries (analysis/re-weapon-sounds.md §1).
//! A `soundparameterized` record (kind 0x12EBD555, a vehicle's
//! `EngineSound`) names a layered record and, per layer NAME, graphs
//! `ParameterGraph(parameter, graph, target)`; a graph is its own record
//! (kind 0xE657A914) of `ControlPoint(x, y)` entries
//! (analysis/re-vehicle-sounds.md §1).
//! A `mus_` chunk's `Music` records (the stream a battle's music plays and
//! its fade and hold times) are [`find_music`]'s (analysis/re-ambience-music.md
//! 3.7). [`find_sublevels`] / [`find_named_sublevels`] keep a SOUND LVL's
//! records apart by `lvl_` sub-level.
//! Hashes are `fnv1a_lowercase` (byte | 0x20); the game looks properties up
//! by the hash of the class-prop string (FireSound = "all_weap_inf_rifle_fire").
//!
//! The 3-D fields are the six the PS2's level function reads
//! (analysis/re-sound-distance.md): `rollindistance`, `mindistance`,
//! `mutedistance`, `maxdistance`, `rolloff`, and `mode3d` (2-D when 0).

use crate::config::{parse_tree, ConfigNode};
use crate::{fnv1a_lowercase, Chunk, UcfbError};

#[derive(Debug, Clone, PartialEq)]
pub struct SoundProp {
    pub name_hash: u32,
    /// Record kind hash (`soundproperties`, streams, layered...).
    pub kind: u32,
    pub gain: Option<f32>,
    pub pitch: Option<f32>,
    pub min_distance: Option<f32>,
    pub max_distance: Option<f32>,
    /// `mutedistance`: silent from here out (the templates set it equal to
    /// `maxdistance`; 95 definitions on the two discs cut off earlier).
    pub mute_distance: Option<f32>,
    /// `rollindistance`: quieter the nearer inside this (distant layers).
    pub roll_in_distance: Option<f32>,
    pub rolloff: Option<f32>,
    /// `mode3d`: false = a 2-D sound, played at its gain at any distance.
    pub mode_3d: Option<bool>,
    pub priority: Option<f32>,
    pub looping: Option<bool>,
    pub inherit: Option<u32>,
    /// `bus`: the mixer bus the property plays on, by name hash (`soundfx`,
    /// `voiceover`, `ambience`, `battlechatter`...; card MIXH).
    pub bus: Option<u32>,
    /// (sample id hash, weight)
    pub samples: Vec<(u32, f32)>,
    /// `pitchdev` / `gaindev`: each start draws pitch (gain) +- this.
    pub pitch_dev: Option<f32>,
    pub gain_dev: Option<f32>,
    /// `playinterval` (+- `playintervaldev`): seconds before the property
    /// may start again; `playprobability`: the chance a start is taken.
    pub play_interval: Option<f32>,
    pub play_interval_dev: Option<f32>,
    pub play_probability: Option<f32>,
    /// Layered definitions: the record's `Layer` entries, in file order.
    pub layers: Vec<Layer>,
    /// Stream reference (music/VO), if any.
    pub stream: Option<u32>,
    /// `segmentlist` members: (segment id hash, weight) into `stream`'s
    /// segment table (RE: analysis/re-sound-global.md §1.1 — a
    /// `soundstreamproperties` names both, and the segment ids are the only
    /// key GLOBAL.LVL's VO is addressable by).
    pub segments: Vec<(u32, f32)>,
    /// Per `samples` / `segments` entry: its own (PlayInterval,
    /// PlayIntervalDev) - a 4-value entry's third and fourth values, the
    /// take's lock (B1 `FUN_0046af20` 531705-531727: (id, weight, interval,
    /// dev); card UCALL). (0, 0) for a 2-value entry.
    pub sample_locks: Vec<(f32, f32)>,
    pub segment_locks: Vec<(f32, f32)>,
    /// A `soundparameterized` record: the layered record it plays
    /// (`SoundLayered`) and its `Layer { .. }` scopes, in file order.
    pub layered: Option<u32>,
    pub graph_layers: Vec<GraphLayer>,
    /// A `ParameterGraph` record: its `ControlPoint(x, y)` entries, in file
    /// order.
    pub points: Vec<(f32, f32)>,
}

/// One `Layer { Name, ParameterGraph.. }` scope of a `soundparameterized`
/// record (B1 `FUN_004606e0`).
#[derive(Debug, Clone, PartialEq)]
pub struct GraphLayer {
    /// The layered record's layer of this label gets the graphs.
    pub name: u32,
    /// (parameter, graph record, target), name hashes.
    pub graphs: Vec<(u32, u32, u32)>,
}

/// One `Layer` entry of a `soundlayered` record (B1 `FUN_0045b7b0`, B2
/// `FUN_0054e6f8`; re-weapon-sounds.md §1.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    /// The layer's label (`near`, `high`, `null`...): only a
    /// `soundparameterized` record reads it.
    pub name: u32,
    /// The sound property the layer starts.
    pub sound: u32,
    /// Seconds after the record's start, +- `deviation`.
    pub delay: f32,
    pub deviation: f32,
}

fn f(node: &ConfigNode) -> Option<f32> {
    node.f32(0)
}

/// A take entry's lock: the PS2 reads the interval and its deviation only
/// from an entry of exactly four values (B1 `FUN_0046af20`).
fn take_lock(node: &ConfigNode) -> (f32, f32) {
    if node.values.len() == 4 {
        (node.f32(2).unwrap_or(0.0), node.f32(3).unwrap_or(0.0))
    } else {
        (0.0, 0.0)
    }
}

/// Parse every definition in one `snd_` chunk.
pub fn parse_snd(chunk: &Chunk<'_>) -> Result<Vec<SoundProp>, UcfbError> {
    let h = |s: &str| fnv1a_lowercase(s);
    let (h_name, h_gain, h_pitch, h_min, h_max, h_roll, h_prio, h_loop, h_layer, h_stream) = (
        h("name"),
        h("gain"),
        h("pitch"),
        h("mindistance"),
        h("maxdistance"),
        h("rolloff"),
        h("priority"),
        h("looping"),
        h("layer"),
        h("stream"),
    );
    let (h_mute, h_rollin, h_mode3d) = (h("mutedistance"), h("rollindistance"), h("mode3d"));
    let (h_pitch_dev, h_gain_dev) = (h("pitchdev"), h("gaindev"));
    let (h_interval, h_interval_dev, h_prob) = (
        h("playinterval"),
        h("playintervaldev"),
        h("playprobability"),
    );
    let h_samplelist = 0x4143_ACAF;
    let h_segmentlist = h("segmentlist");
    let h_segment = h("segment");
    let h_sample = h("sample");
    let h_inherit = 0xCA04_EFE0;
    let h_bus = h("bus");
    let (h_layered, h_graph, h_point) = (h("soundlayered"), h("parametergraph"), 0x415B_9110);
    let mut out = Vec::new();
    for node in parse_tree(chunk)? {
        if node.children.is_empty() {
            continue;
        }
        let mut p = SoundProp {
            name_hash: 0,
            kind: node.hash,
            gain: None,
            pitch: None,
            min_distance: None,
            max_distance: None,
            mute_distance: None,
            roll_in_distance: None,
            rolloff: None,
            mode_3d: None,
            priority: None,
            looping: None,
            inherit: None,
            bus: None,
            pitch_dev: None,
            gain_dev: None,
            play_interval: None,
            play_interval_dev: None,
            play_probability: None,
            samples: Vec::new(),
            layers: Vec::new(),
            stream: None,
            segments: Vec::new(),
            sample_locks: Vec::new(),
            segment_locks: Vec::new(),
            layered: None,
            graph_layers: Vec::new(),
            points: Vec::new(),
        };
        for c in &node.children {
            match c.hash {
                x if x == h_name => p.name_hash = c.values.first().copied().unwrap_or(0),
                x if x == h_gain => p.gain = f(c),
                x if x == h_pitch => p.pitch = f(c),
                x if x == h_min => p.min_distance = f(c),
                x if x == h_max => p.max_distance = f(c),
                x if x == h_mute => p.mute_distance = f(c),
                x if x == h_rollin => p.roll_in_distance = f(c),
                x if x == h_roll => p.rolloff = f(c),
                x if x == h_mode3d => p.mode_3d = f(c).map(|v| v > 0.0),
                x if x == h_prio => p.priority = f(c),
                x if x == h_loop => p.looping = c.values.first().map(|v| *v != 0),
                x if x == h_inherit => p.inherit = c.values.first().copied(),
                x if x == h_bus => p.bus = c.values.first().copied(),
                x if x == h_stream => p.stream = c.values.first().copied(),
                x if x == h_pitch_dev => p.pitch_dev = f(c),
                x if x == h_gain_dev => p.gain_dev = f(c),
                x if x == h_interval => p.play_interval = f(c),
                x if x == h_interval_dev => p.play_interval_dev = f(c),
                x if x == h_prob => p.play_probability = f(c),
                // Fewer than three values: the PS2 reads a delay that is
                // not there; no record on either disc has one.
                x if x == h_layer && c.values.len() >= 3 => p.layers.push(Layer {
                    name: c.values[0],
                    sound: c.values[1],
                    delay: c.f32(2).unwrap_or(0.0),
                    deviation: c.f32(3).unwrap_or(0.0),
                }),
                x if x == h_layer && !c.children.is_empty() => {
                    let graphs = c.find_all(h_graph).filter(|g| g.values.len() >= 3);
                    p.graph_layers.push(GraphLayer {
                        name: c
                            .find(h_name)
                            .and_then(|n| n.values.first().copied())
                            .unwrap_or(0),
                        graphs: graphs
                            .map(|g| (g.values[0], g.values[1], g.values[2]))
                            .collect(),
                    });
                }
                x if x == h_layered => p.layered = c.values.first().copied(),
                x if x == h_point => {
                    if let (Some(x), Some(y)) = (c.f32(0), c.f32(1)) {
                        p.points.push((x, y));
                    }
                }
                x if x == h_samplelist => {
                    for s in c.find_all(h_sample) {
                        if let Some(id) = s.values.first() {
                            p.samples.push((*id, s.f32(1).unwrap_or(1.0)));
                            p.sample_locks.push(take_lock(s));
                        }
                    }
                }
                x if x == h_segmentlist => {
                    for s in c.find_all(h_segment) {
                        if let Some(id) = s.values.first() {
                            p.segments.push((*id, s.f32(1).unwrap_or(1.0)));
                            p.segment_locks.push(take_lock(s));
                        }
                    }
                }
                _ => {}
            }
        }
        if p.name_hash != 0 {
            out.push(p);
        }
    }
    Ok(out)
}

/// Every `snd_` definition anywhere in a ucfb payload (SOUND/*.LVL nest
/// them inside `lvl_` wrappers and hashed bank wrappers).
pub fn find_all(inner: &[u8]) -> Vec<SoundProp> {
    find_sublevels(inner).into_iter().flatten().collect()
}

/// [`find_all`] kept apart by sub-level: one list per top-level chunk (a
/// SOUND LVL's `lvl_` wrappers - `tat1cw`, `tat1gcw`... - each loaded on its
/// own by the PS2), records in file order.
pub fn find_sublevels(inner: &[u8]) -> Vec<Vec<SoundProp>> {
    let named = find_named_sublevels(inner);
    named.into_iter().map(|(_, sub)| sub).collect()
}

/// [`find_sublevels`] with each sub-level's name hash (`nab1cw`...): a
/// `lvl_`'s payload is `[name hash][size]` + its chunks, which the chunk
/// walker reads as one child tagged with the name hash. 0 for a top-level
/// chunk that is no `lvl_` (CORE.LVL).
pub fn find_named_sublevels(inner: &[u8]) -> Vec<(u32, Vec<SoundProp>)> {
    let mut out = Vec::new();
    for top in crate::chunks(inner).flatten() {
        let name = (&top.tag == b"lvl_")
            .then(|| top.children().flatten().next())
            .flatten()
            .map_or(0, |k| u32::from_le_bytes(k.tag));
        let mut sub = Vec::new();
        crate::soundbank::walk_sound_wrappers_of(&top, 3, &mut |c| {
            if &c.tag != b"snd_" {
                return false;
            }
            if let Ok(v) = parse_snd(c) {
                sub.extend(v);
            }
            true
        });
        if !sub.is_empty() {
            out.push((name, sub));
        }
    }
    out
}

/// A `Music` record of a `mus_` chunk (B1 `FUN_0035ab68`, B2 `FUN_00440788`;
/// re-ambience-music.md 3.7): the stream property it plays and its times.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicRecord {
    pub name_hash: u32,
    /// `SoundStream`: the `soundstreamproperties` record it starts.
    pub stream: u32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub min_playback: f32,
    /// `MinInactiveTime` (key 0x35724887).
    pub min_inactive: f32,
    /// BF2's `MaxPlaybackTime`.
    pub max_playback: Option<f32>,
    pub priority: f32,
}

/// Every `Music` record of one `mus_` chunk, in file order.
pub fn parse_mus(chunk: &Chunk<'_>) -> Result<Vec<MusicRecord>, UcfbError> {
    let h = |s: &str| fnv1a_lowercase(s);
    let h_music = h("music");
    let mut out = Vec::new();
    for node in parse_tree(chunk)? {
        if node.hash != h_music || node.children.is_empty() {
            continue;
        }
        let first = |k: &str| node.find(h(k)).and_then(|c| c.values.first().copied());
        let num = |k: &str| node.find(h(k)).and_then(|c| c.f32(0));
        let (Some(name_hash), Some(stream)) = (first("name"), first("soundstream")) else {
            continue;
        };
        out.push(MusicRecord {
            name_hash,
            stream,
            fade_in: num("fadeintime").unwrap_or(0.0),
            fade_out: num("fadeouttime").unwrap_or(0.0),
            min_playback: num("minplaybacktime").unwrap_or(0.0),
            min_inactive: num("mininactivetime").unwrap_or(0.0),
            max_playback: num("maxplaybacktime"),
            priority: num("priority").unwrap_or(0.0),
        });
    }
    Ok(out)
}

/// Every `Music` record anywhere in a ucfb payload, in file order (a SOUND
/// LVL repeats its records per `lvl_` sub-level).
pub fn find_music(inner: &[u8]) -> Vec<MusicRecord> {
    let mut out = Vec::new();
    crate::soundbank::walk_sound_wrappers(inner, 3, &mut |c| {
        if &c.tag != b"mus_" {
            return false;
        }
        if let Ok(v) = parse_mus(c) {
            out.extend(v);
        }
        true
    });
    out
}

/// What the PS2's parser keeps of each layered record's `Layer` entries: the
/// sound is looked up while the record is parsed (B1 `FUN_0045b7b0` 520164,
/// B2 `FUN_0054e6f8`), so an entry naming a `soundproperties` defined LATER
/// in the sub-level, or in neither the sub-level nor `core`, makes no layer
/// (re-weapon-sounds.md §1.2, §1.4: BF1's `com_weap_mine_land` keeps one of
/// two). `sub` is one sub-level in file order ([`find_sublevels`]); returns
/// how many entries were dropped as (late, absent).
pub fn drop_unresolved_layers(
    sub: &mut [SoundProp],
    core: &std::collections::HashSet<u32>,
) -> (usize, usize) {
    let kind = fnv1a_lowercase("soundproperties");
    let all: std::collections::HashSet<u32> = sub
        .iter()
        .filter(|p| p.kind == kind)
        .map(|p| p.name_hash)
        .collect();
    let mut seen = std::collections::HashSet::new();
    let (mut late, mut absent) = (0, 0);
    for p in sub {
        if p.kind == kind {
            seen.insert(p.name_hash);
        }
        p.layers.retain(|l| {
            if seen.contains(&l.sound) || core.contains(&l.sound) {
                return true;
            }
            if all.contains(&l.sound) {
                late += 1;
            } else {
                absent += 1;
            }
            false
        });
    }
    (late, absent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated: BF1 SOUND/TAT.LVL — the rifle fire property resolves to
    /// one weighted sample with gain 0.6 and a template (re-sound-banks
    /// walk-through reproduced independently in Python before this port).
    #[test]
    fn tat_sound_lvl_rifle_fire_property() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: SOUND/TAT.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let props = find_all(inner);
        assert!(props.len() > 2000, "{} props", props.len());
        let h = fnv1a_lowercase("all_weap_inf_rifle_fire");
        let rifle = props
            .iter()
            .find(|p| p.name_hash == h)
            .expect("rifle fire prop");
        assert_eq!(rifle.gain, Some(0.6));
        assert_eq!(rifle.samples, vec![(0x7265_2AA3, 1.0)]);
        assert_eq!(rifle.inherit, Some(0xB1AD_507C));
        // Its template is not in this LVL: it is in CORE.LVL (card SNDT).
        assert!(props.iter().all(|p| p.name_hash != 0xB1AD_507C));
        let exp = props
            .iter()
            .find(|p| p.name_hash == fnv1a_lowercase("imp_weap_ord_exp"))
            .unwrap();
        assert_eq!(
            (exp.min_distance, exp.max_distance, exp.rolloff),
            (Some(10.0), Some(60.0), Some(3.0))
        );
        assert_eq!((exp.mute_distance, exp.mode_3d), (Some(60.0), Some(true)));
        assert_eq!(exp.samples.len(), 21);
        let banks = crate::soundbank::find_banks_deep(inner);
        assert_eq!(banks.len(), 4);
        assert!(banks
            .iter()
            .any(|b| b.samples.iter().any(|s| s.id_hash == 0x7265_2AA3)));
    }

    /// Data-gated: the `soundstreamproperties` half — BF1 TAT.LVL's tactical
    /// chatter names a STREAM in another file (SOUND/GCW.LVL's `gcw_tac_vo`
    /// — TAT1 is a GCW map)
    /// and three of its segments, and owns no sample of its own
    /// (analysis/re-sound-global.md §3, lead-verified).
    #[test]
    fn tat_sound_lvl_tac_chatter_names_a_stream_and_segments() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/sound/DATA/_LVL_PS2/SOUND/TAT.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: SOUND/TAT.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let props = find_all(inner);
        let h = fnv1a_lowercase("all_inf_com_chatter_tac_follow");
        let follow = props.iter().find(|p| p.name_hash == h).expect("tac follow");
        assert_eq!(follow.stream, Some(fnv1a_lowercase("gcw_tac_vo")));
        assert_eq!(
            follow.segments,
            vec![(0xC045_C279, 1.0), (0xCB45_D3CA, 1.0), (0xCC45_D55D, 1.0)]
        );
        assert!(follow.samples.is_empty());
        // Streamed properties are a real share of a map's sound script.
        let streamed = props.iter().filter(|p| !p.segments.is_empty()).count();
        assert!(streamed > 100, "{streamed} streamed props");
    }

    const EXTRACTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");

    /// One LVL's sub-levels with the parser's drops applied against `core`'s
    /// `soundproperties`, and the (entries, late, absent) counts.
    fn kept(
        path: &str,
        core: &std::collections::HashSet<u32>,
    ) -> (Vec<Vec<SoundProp>>, [usize; 3]) {
        let buf = std::fs::read(path).unwrap();
        let mut subs = find_sublevels(crate::open(&buf).unwrap());
        let mut n = [0; 3];
        for sub in &mut subs {
            n[0] += sub.iter().map(|p| p.layers.len()).sum::<usize>();
            let (late, absent) = drop_unresolved_layers(sub, core);
            n[1] += late;
            n[2] += absent;
        }
        (subs, n)
    }

    fn core_names(game: &str) -> Option<(String, std::collections::HashSet<u32>)> {
        let dir = if game == "bf1" {
            "bf1/sound"
        } else {
            "bf2/lvl"
        };
        let path = format!("{EXTRACTED}/{dir}/DATA/_LVL_PS2/CORE.LVL");
        let buf = std::fs::read(&path).ok()?;
        let kind = fnv1a_lowercase("soundproperties");
        let names = find_all(crate::open(&buf).unwrap())
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| p.name_hash)
            .collect();
        Some((path, names))
    }

    /// Card WSND's layer parse, from the discs (analysis/re-weapon-sounds.md
    /// §1): an entry is (name, sound, delay [, deviation]); a layer naming a
    /// property defined later than its record is dropped. Totals equal
    /// `lead_tmp/wsnd/census_layers.py`.
    #[test]
    fn layered_records_keep_what_the_ps2_parser_keeps() {
        let Some((_, core)) = core_names("bf1") else {
            eprintln!("SKIP: BF1 CORE.LVL not present");
            return;
        };
        let nab = format!("{EXTRACTED}/bf1/sound/DATA/_LVL_PS2/SOUND/NAB.LVL");
        if !std::path::Path::new(&nab).exists() {
            eprintln!("SKIP: SOUND/NAB.LVL not present");
            return;
        }
        let (subs, _) = kept(&nab, &core);
        let h = fnv1a_lowercase;
        let layers = |name: &str| -> Vec<(u32, f32, f32)> {
            let p = subs[0]
                .iter()
                .find(|p| p.name_hash == h(name))
                .expect("record");
            p.layers
                .iter()
                .map(|l| (l.sound, l.delay, l.deviation))
                .collect()
        };
        let (high, low) = (
            h("com_weap_inf_ordnance_exp_high"),
            h("com_weap_inf_ordnance_exp_low"),
        );
        assert_eq!(
            layers("com_weap_inf_ordnance_exp"),
            [
                (high, 0.0, 0.0),
                (high, 0.1, 0.0),
                (high, 0.2, 0.0),
                (low, 0.0, 0.0)
            ]
        );
        let (near, distant) = (
            h("com_weap_obj_med_exp_near"),
            h("com_weap_obj_med_exp_distant"),
        );
        assert_eq!(
            layers("com_weap_obj_med_exp"),
            [
                (near, 0.0, 0.0),
                (near, 0.3, 0.1),
                (distant, 0.3, 0.1),
                (0x8750_2C03, 0.5, 0.1)
            ]
        );
        // Its 0.3 s layer (0x9D66E891) is defined after the record.
        assert_eq!(layers("com_weap_mine_land"), [(0x4E17_6B01, 0.6, 0.0)]);
        // The far layer's own gate, and the explosions' pitch draw.
        let far = subs[0].iter().find(|p| p.name_hash == 0x8750_2C03).unwrap();
        assert_eq!(far.play_interval, Some(0.5));

        // Both discs: every SOUND LVL (GLOBAL holds none) and CORE.
        for (game, want) in [("bf1", [1185, 30, 0]), ("bf2", [1819, 0, 11])] {
            let Some((core_path, core)) = core_names(game) else {
                eprintln!("SKIP: {game} CORE.LVL not present");
                continue;
            };
            let dir = format!("{EXTRACTED}/{game}/sound/DATA/_LVL_PS2/SOUND");
            let mut files: Vec<String> = std::fs::read_dir(&dir)
                .expect("SOUND dir")
                .filter_map(|f| f.ok())
                .map(|f| f.path().to_string_lossy().into_owned())
                .filter(|f| f.ends_with(".LVL") && !f.ends_with("GLOBAL.LVL"))
                .collect();
            files.push(core_path);
            let mut n = [0; 3];
            for f in &files {
                let (subs, k) = kept(f, &core);
                n = [n[0] + k[0], n[1] + k[1], n[2] + k[2]];
                for p in subs.iter().flatten() {
                    // What is kept is a sound, never a label or a float.
                    for l in &p.layers {
                        assert!((0.0..20.0).contains(&l.delay), "{f} {:08x}", p.name_hash);
                    }
                }
            }
            eprintln!(
                "WSND {game}: {} entries, {} late, {} absent",
                n[0], n[1], n[2]
            );
            assert_eq!(n, want, "{game}");
        }
    }

    /// The sub-level whose name hash is four printable bytes: BF1 KAS.LVL's
    /// `kas1gcw` = 0x2A396768 = "hg9*" (the one of 69 on the two discs).
    #[test]
    fn a_printable_sublevel_hash_is_still_a_sublevel() {
        let p = format!("{EXTRACTED}/bf1/sound/DATA/_LVL_PS2/SOUND/KAS.LVL");
        let Ok(buf) = std::fs::read(&p) else {
            eprintln!("SKIP: SOUND/KAS.LVL not present");
            return;
        };
        assert_eq!(fnv1a_lowercase("kas1gcw"), 0x2A39_6768);
        let inner = crate::open(&buf).unwrap();
        let subs = find_sublevels(inner);
        assert_eq!(subs.len(), 4);
        assert!(subs.iter().all(|s| s.len() > 300));
        let banks = crate::soundbank::find_banks_deep(inner);
        let names: Vec<u32> = banks.iter().map(|b| b.name_hash).collect();
        assert_eq!(names.len(), 4, "{names:08x?}");
        assert!(names.contains(&0x2A39_6768), "{names:08x?}");
    }
}
