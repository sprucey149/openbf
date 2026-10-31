//! Runtime sound library: `sounds.ron` (bake sounds) + lazily decoded clips.
//! Properties are looked up by the fnv1a_lowercase hash of the class-prop
//! string; playback picks a weighted-random take and attenuates by the
//! property's distances and rolloff with the PS2's own level function
//! ([`attenuation`], analysis/re-sound-distance.md), panning by the
//! listener-relative lateral position with the PS2's per-ear law and bus
//! gains ([`ears_3d`], [`bus_table`]; card MIXH, analysis/re-mix-headroom.md).
//! A layered property starts each layer
//! at its own delay ([`SoundLib::start`], [`SoundLib::tick`]; card WSND,
//! analysis/re-weapon-sounds.md §2-3), and every start draws its own pitch
//! and gain and passes the property's `PlayInterval` / `PlayProbability`.
//! A vehicle's engine layers are looping voices whose gain, rate and pan
//! are set every tick ([`SoundLib::loop_voices`]; card VSND,
//! analysis/re-vehicle-sounds.md §2.3).
//!
//! A take is a WAV in the map's own bank, or — for a `soundstreamproperties`
//! (`stream`/`segments`, baked by item 15) — an OGG under
//! `audio/global_bf{1,2}/`, decoded from the game's GLOBAL/CW/GCW stream LVL:
//! almost all of the speech on both discs is there, not in the map banks
//! (`engine/artifacts/global_sound/README.md`; RE `analysis/re-sound-global.md`).
//! A map's SOUND LVL also carries streams of its OWN — the mono `<map>_emt`
//! takes and its music — baked beside its bank into `<map>/sounds/streams/`
//! (queue item VO), and those are searched FIRST.
//! The map's game also brings in its resident CORE.LVL bank
//! (`core_bf{1,2}/sounds/`, the shell/load definitions) behind [`SoundLib::global`].
//! Commander playback keeps one named definition and weighted take through
//! gain, bus, pitch and duration. Fixed positional takes use the same rate
//! product and floor as other positional starts; preparation needs no mixer.

mod hero;
mod playback;
mod walker;

use crate::classdb::fnv1a_lowercase;
use glam::Vec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The RON's two structs live in the `manifest` crate — ONE definition
/// shared with `bake::sounds`, which writes the file. Before this each side
/// carried its own copy and a field added to one and forgotten on the other
/// only showed up as a sound that never played. `SoundProp` is re-exported
/// because the rest of this module (and `SoundLib`'s callers) name it.
pub use manifest::{SoundProp, SoundsRon};

/// The fields the runtime needs out of a stream index — `global.ron` under
/// `audio/global_bf{1,2}/`, or `streams.ron` under a map's own
/// `sounds/streams/`. `bake::global_sound::GlobalSoundRon` writes more (the
/// per-segment offsets a music streamer would seek with); serde ignores the
/// rest.
#[derive(Deserialize, Debug, Default)]
struct GlobalRon {
    /// Segment id → OGG path relative to that directory.
    files: Vec<(u32, String)>,
    /// The stream table, for the segments that have NO file: it says whether
    /// the disc holds that audio at all, which is the difference between a
    /// music cue the bake indexes on purpose and a name whose take is on no
    /// disc (see [`Census`]).
    #[serde(default)]
    streams: Vec<IndexStream>,
}

#[derive(Deserialize, Debug)]
struct IndexStream {
    #[serde(default)]
    name_hash: u32,
    /// The LVL the stream lives in (`nab.lvl`, `global.lvl`).
    #[serde(default)]
    source: String,
    #[serde(default)]
    channels: u32,
    #[serde(default)]
    block_size: u32,
    /// Byte offset of the stream's data in that file.
    #[serde(default)]
    data_offset: u64,
    /// False for the stereo music streams: indexed, never decoded (the maintainer's
    /// decision — music stays streamed).
    baked: bool,
    segments: Vec<IndexSegment>,
    #[serde(default)]
    format: u32,
    #[serde(default)]
    substreams: u32,
    #[serde(default)]
    interleave: u32,
}

#[derive(Deserialize, Debug)]
struct IndexSegment {
    id: u32,
    #[serde(default)]
    frequency: u32,
    /// Offset inside the stream's data.
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    size: usize,
}

/// A `Music` record ready to play (card AMBM): its times, and each take of
/// its stream property as the place of its ADPCM on the disc.
#[derive(Debug, Clone)]
pub struct MusicTrack {
    pub def: core::music::MusicDef,
    /// The stream property's `Gain`.
    pub gain: f32,
    /// One per `def.takes` entry, in the property's list order.
    pub takes: Vec<audio::StreamTake>,
    /// The takes' segment ids, for the log.
    pub ids: Vec<u32>,
}

/// A level 0..1 in decibels for the mixer, floored at -60 dB.
pub fn level_db(level: f32) -> f32 {
    20.0 * level.max(0.001).log10()
}

/// The takes of every stream the bake did not decode, by (stream, segment):
/// each streams from the copy of its ADPCM the bake wrote under the index's
/// own directory `dir` ([`manifest::music_body`]).
fn note_music(g: &GlobalRon, dir: &Path, out: &mut HashMap<(u32, u32), audio::StreamTake>) {
    for s in g
        .streams
        .iter()
        .filter(|s| !s.baked && (s.block_size > 0 || s.format == 5))
    {
        let path = dir.join(manifest::music_body(&s.source, s.data_offset));
        for seg in &s.segments {
            out.entry((s.name_hash, seg.id))
                .or_insert_with(|| audio::StreamTake {
                    path: path.clone(),
                    offset: seg.offset,
                    size: seg.size,
                    format: s.format,
                    channels: s.channels,
                    substreams: s.substreams,
                    interleave: s.interleave as usize,
                    frequency: seg.frequency,
                    block_size: s.block_size as usize,
                });
        }
    }
}

/// A definition that can actually make a noise: it names bank samples, stream
/// segments, or layers that do.
fn audible(p: &SoundProp) -> bool {
    !p.samples.is_empty() || !p.layers.is_empty() || !p.segments.is_empty()
}

/// Why a definition that names stream segments makes no sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    /// Its takes are music segments: indexed, never decoded (the maintainer's decision).
    Music,
    /// The index knows the segment; no OGG was written for it.
    Unbaked,
    /// No stream on the disc carries the take — a cut line.
    Cut,
}

/// What [`SoundLib::census`] counts, over the map bank's distinct properties.
/// The last three partition the definitions that name stream segments and
/// resolve to no file, and they are what queue item VO measured:
/// `music` is by design (the maintainer's decision — the stereo music streams are
/// indexed, never decoded), `unbaked` is a segment the index knows and the
/// bake skipped, and `cut` names a take that is in NO stream on the disc.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    /// Distinct properties in the map's own bank.
    pub props: usize,
    /// …resolving through a bank WAV.
    pub wav: usize,
    /// …resolving through a WAV or a streamed segment's OGG.
    pub any: usize,
    /// Silent: every take is a music segment, which stays streamed.
    pub music: usize,
    /// Silent: the index knows the segment but no OGG was written for it.
    pub unbaked: usize,
    /// Silent: no stream on the disc carries the take at all (cut lines).
    pub cut: usize,
}

pub struct Listener {
    pub pos: Vec3,
    /// Engine yaw (0 = −Z, positive left).
    pub yaw: f32,
}

/// What a definition plays, for the referees: (stream, segment ids, sample
/// ids, layers).
pub type Audio = (Option<u32>, Vec<u32>, Vec<u32>, Vec<u32>);

/// One baked `sounds/` directory — the map's bank, or the resident CORE one.
struct Bank {
    dir: PathBuf,
    /// One definition per name over the whole LVL ([`merge_props`]): what a
    /// name the battle's own sub-levels do not define falls through to.
    props: HashMap<u32, SoundProp>,
    /// One definition per (sub-level, name): [`merge_props`] over each
    /// sub-level's own records.
    by_level: HashMap<(u32, u32), SoundProp>,
    sample_paths: HashMap<u32, String>,
    /// The `soundparameterized` records by name: one per sub-level that
    /// defines it, in file order.
    engines: HashMap<u32, Vec<manifest::EngineSound>>,
    /// The `Music` records by name.
    music: HashMap<u32, manifest::MusicRon>,
}

/// One property per name hash, from records in chunk order: the last
/// definition that can make a noise.
///
/// A planet's SOUND LVL is one `lvl_` sub-level per map and era (BF1
/// KAS.LVL: `kas1gcw`, `kas1cw`, `kas2gcw`, `kas2cw`) and the PS2 loads the
/// ONE its mission script names (`ReadDataFile("sound\\kas.lvl;kas2gcw")`),
/// so a battle's definition of a name is its own sub-level's
/// ([`SoundLib::find`], card SNDE). The disc: the commander's post lines
/// carry the same names in every sub-level, with the `gcw_vo` stream's
/// segments in the gcw ones and `cw_vo`'s in the cw ones; the RAM of 160 of
/// 160 BF1 battle save states holds one era's names beside that era's stream
/// only (`engine/artifacts/lead_tmp/audit-annc2/`: `sublevel_defs_kas.txt`,
/// `ram_defs_all_bf1.txt`). Before the card this merge ran over the whole
/// LVL and every battle heard its last sub-level's definitions
/// (`<planet>2cw`): 256 names over 16 of the 28 sub-levels BF1's missions
/// load, 201 of them streamed voice.
///
/// The merge is still used twice. Over ONE sub-level's records, for a name
/// that sub-level defines more than once (PROVISIONAL, ours: which of two
/// definitions in one sub-level the PS2 keeps is not read). And over the
/// whole LVL, for a name none of the battle's sub-levels defines - an army
/// the mission does not field (the port's era mix, card ANNC2), which the
/// PS2 never has to answer.
/// Counting SEGMENTS as audible cannot change which duplicate wins on
/// this bake: measured over all 34 baked banks, no hash pairs a
/// sample/layer definition with a segment-only one (0 of 34).
fn merge_props(records: Vec<SoundProp>) -> HashMap<u32, SoundProp> {
    let mut props: HashMap<u32, SoundProp> = HashMap::new();
    for p in records {
        let keep = match props.get(&p.name_hash) {
            Some(prev) => audible(&p) || !audible(prev),
            None => true,
        };
        if keep {
            props.insert(p.name_hash, p);
        }
    }
    props
}

/// [`merge_props`] over each sub-level's own records, by (sub-level, name).
fn by_level(records: &[SoundProp]) -> HashMap<(u32, u32), SoundProp> {
    let mut levels: HashMap<u32, Vec<SoundProp>> = HashMap::new();
    for p in records.iter().filter(|p| p.level != 0) {
        levels.entry(p.level).or_default().push(p.clone());
    }
    let mut out = HashMap::new();
    for (level, records) in levels {
        out.extend(
            merge_props(records)
                .into_iter()
                .map(|(name, p)| ((level, name), p)),
        );
    }
    out
}

impl Bank {
    fn import_sbd_sounds(&mut self, root: &Path, game: &str) {
        let names: &[&str] = match game {
            "bf1" => &[
                "cis_weap_inf_blaster_fire",
                "cis_weap_inf_wrist_shrapnel",
                "cis_weap_inf_rocket_fire",
                "cis_weap_inf_equip_med",
                "cis_weap_inf_reload_med",
                "rep_weap_inf_equip_sm",
            ],
            "bf2" => &[
                "cis_weap_inf_wrist_blaster_fire",
                "cis_weap_inf_wrist_trishot_fire",
                "cis_weap_inf_wrist_rocket_fire",
                "com_weap_inf_equip_med",
                "com_weap_inf_equip_lg",
            ],
            _ => return,
        };
        let Some(source) = Bank::load(&root.join(format!("{game}_geo1/sounds"))) else {
            return;
        };
        let mut pending: Vec<_> = names.iter().map(|name| fnv1a_lowercase(name)).collect();
        let mut imported = Vec::new();
        while let Some(hash) = pending.pop() {
            if imported.contains(&hash) {
                continue;
            }
            imported.push(hash);
            let Some(mut prop) = source.props.get(&hash).cloned() else {
                continue;
            };
            pending.extend(prop.layers.iter().copied());
            let Some(paths) = prop
                .samples
                .iter()
                .map(|(id, _)| {
                    source
                        .sample_path(*id)
                        .and_then(|p| std::path::absolute(p).ok())
                })
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            for (sample, path) in prop.samples.iter_mut().zip(paths) {
                let mut id = sample.0;
                while self.sample_paths.contains_key(&id) {
                    id = id.wrapping_sub(1);
                }
                self.sample_paths
                    .insert(id, path.to_string_lossy().into_owned());
                sample.0 = id;
            }
            self.props.insert(hash, prop);
        }
    }

    fn load(dir: &Path) -> Option<Bank> {
        let txt = pak::read_to_string(dir.join("sounds.ron")).ok()?;
        let ron: SoundsRon = ron::from_str(&txt)
            .map_err(|e| eprintln!("sound bank {} unreadable: {e}", dir.display()))
            .ok()?;
        let mut engines: HashMap<u32, Vec<manifest::EngineSound>> = HashMap::new();
        for e in ron.engines.into_iter().filter(|e| !e.layers.is_empty()) {
            engines.entry(e.name_hash).or_default().push(e);
        }
        let mut music = HashMap::new();
        for m in ron.music {
            music.entry(m.name_hash).or_insert(m);
        }
        Some(Bank {
            dir: dir.to_path_buf(),
            engines,
            music,
            by_level: by_level(&ron.props),
            props: merge_props(ron.props),
            sample_paths: ron.samples.into_iter().collect(),
        })
    }

    fn sample_path(&self, id: u32) -> Option<PathBuf> {
        self.sample_paths.get(&id).map(|rel| self.dir.join(rel))
    }
}

/// A start the library decided on: the take and what the mixer is told.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    /// The property that starts (a layered record's layer, not the record).
    pub hash: u32,
    pub path: PathBuf,
    pub gain: f32,
    pub pitch: f32,
    pub pos: Vec3,
    /// The final gain of the bus the property plays on ([`bus_table`]).
    pub bus: f32,
    /// A 2-D property: full gain in both ears, wherever it starts.
    pub flat: bool,
}

impl Start {
    /// What each ear gets (left, right) for `listener`: the gain x the bus,
    /// split by [`ears_3d`] unless the property is 2-D.
    pub fn ears(&self, listener: &Listener) -> (f32, f32) {
        let a = self.gain * self.bus;
        if self.flat {
            (a, a)
        } else {
            ears_3d(a, side(listener, self.pos))
        }
    }
}

/// A one-shot the mixer was handed, for the playtest black box
/// (`crate::blackbox`, card SNDB): kept only while [`SoundLib::handed`] is on.
#[derive(Debug, Clone, PartialEq)]
pub struct Handed {
    /// The property (a 2-D line's too).
    pub hash: u32,
    pub path: PathBuf,
    /// The linear gain: the property's x the level.
    pub gain: f32,
    /// What each ear was handed (left, right): the gain x the bus, split by
    /// [`ears_3d`] for a 3-D sound.
    pub ears: (f32, f32),
    /// The playback rate it was told.
    pub pitch: f32,
    /// Metres from the listener; 0 for a 2-D line.
    pub dist: f32,
    /// The take's length at that rate, seconds.
    pub secs: f32,
}

/// A unit's cry on its voice (card UCRY): sounding until `ends` on the
/// library's clock; `shot` cuts it in the mixer (None headless).
struct Cry {
    ends: f32,
    shot: Option<audio::Shot>,
}

/// A layer waiting for its delay (B1 `FUN_0045bc10` / `FUN_0045be28`).
struct Timer {
    sound: u32,
    /// Where the record was started: the layer plays there.
    pos: Vec3,
    /// Seconds after arming at which it starts.
    fire: f32,
    elapsed: f32,
    /// What a full pool keeps the timer by.
    value: f32,
}

/// An emitter's voice ([`SoundLib::ambient_voices`]).
enum AmbientVoice {
    /// A looping bank sample or baked take; `None` when it has none.
    Loop(Option<audio::Voice>),
    /// A take streamed from the disc (the stereo ambience streams).
    Stream(Option<audio::MusicVoice>),
    /// A one-shot asked again when its take has ended, at [`SoundLib`]'s
    /// clock.
    Shot { until: f32 },
}

/// An emitter's take ([`SoundLib::ambient_take`]).
#[derive(Debug, Clone)]
pub enum AmbientTake {
    Stream(audio::StreamTake),
    File(PathBuf),
}

/// What an emitter sounds like at the listener ([`SoundLib::ambient_heard`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientHeard {
    /// Index into the emitter list.
    pub index: usize,
    pub dist: f32,
    /// The PS2's level by distance with the emitter's distances (0..1).
    pub level: f32,
    /// The property's gain x the level x its bus.
    pub gain: f32,
}

/// The PS2's pool of waiting layers (B1 0x751e40, B2 0x8380a0: 0x4e timers).
pub const LAYER_TIMERS: usize = 78;

/// Why [`SoundLib::ask_line`] queued no line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// No such property, or it names no take.
    Unknown,
    /// The take drawn is inside its own lock (BF1: the request is silent).
    TakeLocked,
    /// The property is inside its interval.
    Interval,
    /// Its probability roll refused it (the interval is spent).
    Probability,
    /// The speaker is beyond the property's mute distance.
    Far,
}

/// A unit line's queue key stays under a commander line's
/// (`core::announcer::COMMANDER_KEY`): on the PS2 a commander line is never
/// queued behind chatter (re-announcer.md 1.5).
pub const CHATTER_KEY_MAX: f32 = 0.999;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum LoopSource {
    Engine(u32, u8),
    /// A jet soldier's engine layer (card JETP2), by unit index.
    Jet(u32, u8),
    Guided(u64, u8),
    /// A round's flight hum (card WHUM), by the round's across-frames key
    /// (`crate::bolt_vis::key`).
    Ordnance(crate::bolt_vis::Key),
    Repair(core::unit::UnitHandle, usize),
    Hero(core::unit::UnitHandle, u32),
    Shield(core::unit::UnitHandle),
    WalkerFire(core::unit::UnitHandle, usize),
}

struct LoopRequest {
    source: LoopSource,
    sound: u32,
    pos: Vec3,
    gain: f32,
    pitch: f32,
}

pub struct SoundLib {
    /// Layers waiting for their delay.
    timers: Vec<Timer>,
    /// Seconds of [`SoundLib::tick`] so far: the `PlayInterval` clock.
    now: f32,
    /// Property -> the time its `PlayInterval` lets it start again.
    next_ok: HashMap<u32, f32>,
    /// (property, take) -> the time the take's own lock ends (card UCALL).
    take_ok: HashMap<(u32, usize), f32>,
    map: Bank,
    /// The battle's sound sub-levels by name hash, best first
    /// (`crate::foley::sublevels`): the one the mission's script loads, then
    /// the map's others. [`SoundLib::find`] takes a name's definition from
    /// the first that has one.
    levels: Vec<u32>,
    /// The battle's armies of another era than its mission's sub-levels
    /// (`crate::foley::foreign_armies`): a name with the prefix is looked up
    /// in that sub-level first ([`SoundLib::find_named`]).
    foreign: Vec<(String, u32)>,
    /// The map game's resident CORE.LVL bank, when it is baked.
    core: Option<Bank>,
    /// `<assets>/audio/global_bf{1,2}` and its segment id → OGG index; empty
    /// when the global VO is not baked (the map still plays its own WAVs).
    global_dir: PathBuf,
    global_files: HashMap<u32, String>,
    /// `<map dir>/sounds/streams` and the same index for the map's OWN
    /// streams (queue item VO); empty on a bake made before that item.
    map_streams_dir: PathBuf,
    map_stream_files: HashMap<u32, String>,
    /// Segment ids either index KNOWS but holds no OGG for → whether the
    /// stream they live in is one the bake never decodes (the stereo music).
    /// Only [`SoundLib::census`] reads it; it is what separates "streamed
    /// music, by design" from "this take is on no disc".
    indexed: HashMap<u32, bool>,
    /// The streamed music's takes on the disc, by (stream, segment id).
    music_takes: HashMap<(u32, u32), audio::StreamTake>,
    /// Decoded clips, keyed by PATH: a bank sample id and a segment id are
    /// hashes of different things and may collide.
    cache: HashMap<PathBuf, audio::SoundData>,
    loops: HashMap<LoopSource, Option<audio::Voice>>,
    /// The world's emitters sounding, by index into
    /// `crate::ambience::Ambience::emitters` ([`SoundLib::ambient_voices`]).
    ambient: HashMap<usize, AmbientVoice>,
    /// Each unit's ONE voice for its hurt and death cries, by unit slot
    /// (card UCRY, [`SoundLib::cry`]).
    cries: HashMap<u32, Cry>,
    /// Each station's ambient handle: sounding until this time on the
    /// library's clock, by station index (card PWST, [`SoundLib::station_ambient`]).
    station_voices: HashMap<u16, Cry>,
    hero_voices: HashMap<(core::unit::UnitHandle, u32), Cry>,
    hero_blades: Vec<hero::BladeVoice>,
    /// Weighted-pick state (audio-only randomness; never touches the sim).
    seed: u32,
    pub played: u64,
    /// Dev: every one-shot handed to the mixer since the list was last
    /// drained (the black box's sound rows); `None` keeps none.
    pub handed: Option<Vec<Handed>>,
    /// Pitch multiplier for every positional sound (PS2 `slow` cheat: 0.3).
    pub rate_mul: f32,
    /// The map's game's bus gains ([`bus_table`]).
    buses: Vec<(u32, f32)>,
}

/// The 3-D listener's rolloff factor: the level function multiplies a
/// property's `rolloff` by it. 10.0 from the listener's constructor (B1
/// `FUN_00449f78`, B2 `FUN_0053da70`) and in the RAM of every one of 329
/// reference states; no writer found (analysis/re-sound-distance.md `[O]`).
pub const LISTENER_ROLLOFF: f32 = 10.0;

/// A 3-D sound's level 0..1 at `dist` metres from the listener: the PS2's
/// function (B1 `FUN_0044cac8`, B2 `FUN_005407e8`, the same on both; `[O]`
/// analysis/re-sound-distance.md). Full between the roll-in and the minimum
/// distance; beyond the minimum `(R - x) / (x k + R)` with `x` the distance
/// past it, `R = max - min` and `k = 10 rolloff` - about `R / (k x)` over
/// most of the range, reaching 0 at the maximum; silent from the mute
/// distance out. Inside the roll-in distance it falls towards the listener
/// the same way. A 2-D (`flat`) property is not attenuated at all.
pub fn attenuation(dist: f32, p: &SoundProp) -> f32 {
    if p.flat {
        return 1.0;
    }
    let k = LISTENER_ROLLOFF * p.rolloff;
    let level = |x: f32, r: f32| {
        let den = x * k + r;
        if den > 0.0 {
            ((r - x) / den).max(0.0)
        } else {
            0.0
        }
    };
    match p.roll_in_distance {
        Some(rin) if dist < rin => level(rin - dist, rin),
        _ if dist <= p.min_distance => 1.0,
        _ if dist >= p.mute_distance.unwrap_or(p.max_distance) => 0.0,
        _ => level(dist - p.min_distance, p.max_distance - p.min_distance),
    }
}

/// Where a sound at `pos` sits for `listener`: the direction to it dotted
/// with the listener's right, -1 (left) .. 1 (right); 0 at the listener.
pub fn side(listener: &Listener, pos: Vec3) -> f32 {
    let (sy, cy) = listener.yaw.sin_cos();
    let right = Vec3::new(cy, 0.0, -sy);
    let d = pos - listener.pos;
    let dist = d.length();
    if dist < 1e-3 {
        return 0.0;
    }
    (d.dot(right) / dist).clamp(-1.0, 1.0)
}

/// The PS2's per-ear split of a 3-D voice of amplitude `a` (gain x level x
/// bus) at side `s` ([`side`]): p = 0.5 + 0.5 s, left = a (1 - p), right =
/// a p - linear, so the ears sum to `a` and a centred sound is half `a` in
/// each (output mode 3, every reference state of both games; B1
/// `FUN_0044cb78` 510409-510411, B2 `FUN_00540b98`; `[O]`
/// analysis/re-mix-headroom.md 1.1 claim 4). A 2-D sound is `(a, a)`.
pub fn ears_3d(a: f32, s: f32) -> (f32, f32) {
    let p = 0.5 + 0.5 * s.clamp(-1.0, 1.0);
    (a * (1.0 - p), a * p)
}

/// What the mixer is told for a mono take that must reach the ears at
/// `(left, right)`: (volume dB, pan -1..1). kira pans by L x sqrt(1 - m)
/// sqrt 2, R x sqrt(m) sqrt 2 with m = (pan + 1) / 2, so amplitude
/// sqrt((l^2 + r^2) / 2) at m = r^2 / (l^2 + r^2) lands on (l, r) exactly.
pub fn mixer_pair(ears: (f32, f32)) -> (f32, f32) {
    let (l, r) = ears;
    let e = l * l + r * r;
    if e <= 0.0 {
        return (level_db(0.0), 0.0);
    }
    let pan = 2.0 * r * r / e - 1.0;
    (level_db((e * 0.5).sqrt()), pan.clamp(-1.0, 1.0))
}

/// The final gain of each mixer bus a property may name, by name hash, for
/// the map's game (`source_game` `bf1` / `bf2`): the bus's disc gain x its
/// parents' x the option slider over it (`[O]` analysis/re-mix-headroom.md
/// 1.4 claims 10-12; the effects voice x 0.63 on BF2 is the RAM's
/// `+0x44 = +0xf4 x bus +0x28`). The sliders are the ones every reference
/// state holds (BF1 effects 10 voice 8, BF2 effects 7 voice 9 chatter 10):
/// PROVISIONAL [H] as the factory defaults (FEEL_NOTES). The shell's two,
/// `shellfx` 0.4 (the menu clicks) and `shellmusic` 0.6, are CORE.LVL's on
/// both discs, parent root (`analysis/re-bf2-menus.md` §4.2, §4.4). A bus
/// not listed (`root`, the battle music's) plays at 1.0. The `voiceover`
/// duck of `main` is not here (card VODK).
pub fn bus_table(game: &str) -> Vec<(u32, f32)> {
    let rows: &[(&str, f32)] = match game {
        "bf2" => &[
            ("soundfx", 0.63),
            ("battlechatter", 0.9),
            ("voiceover", 0.63),
            ("ambience", 0.378),
            ("shellfx", 0.4),
            ("shellmusic", 0.6),
        ],
        _ => &[
            ("soundfx", 1.0),
            ("voiceover", 0.64),
            ("ambience", 0.6),
            ("shellfx", 0.4),
            ("shellmusic", 0.6),
        ],
    };
    let mut t: Vec<(u32, f32)> = rows.iter().map(|(n, g)| (fnv1a_lowercase(n), *g)).collect();
    // The unnamed bus under `root`: 0.4 x the effects slider (1.4 claim 10).
    t.push((UNNAMED_BUS, if game == "bf2" { 0.4 * 0.7 } else { 0.4 }));
    t
}

/// The bus 0x82c7f467 (its name is not on the discs' candidate lists).
const UNNAMED_BUS: u32 = 0x82c7_f467;

fn read_index(path: &Path) -> GlobalRon {
    // A missing index is normal (an unbaked group) and stays silent; one that
    // is there and will not parse is a bake defect that would otherwise
    // surface only as voice lines that never play.
    let Ok(txt) = pak::read_to_string(path) else {
        return GlobalRon::default();
    };
    ron::from_str::<GlobalRon>(&txt).unwrap_or_else(|e| {
        eprintln!("sound index {} unreadable: {e}", path.display());
        GlobalRon::default()
    })
}

/// Record every segment id the index KNOWS: `true` when its stream is one the
/// bake never decodes (music). A file, when there is one, always wins — this
/// map is only consulted for segments that resolved to nothing.
fn note_indexed(g: &GlobalRon, out: &mut HashMap<u32, bool>) {
    for s in &g.streams {
        for seg in &s.segments {
            let e = out.entry(seg.id).or_insert(!s.baked);
            *e &= !s.baked;
        }
    }
}

impl SoundLib {
    /// [`SoundLib::load`] for a battle of `mission` (card SNDE): the
    /// sub-level its script loads first.
    pub fn load_for(map_dir: &Path, mission: &crate::mission::MissionDef) -> Option<SoundLib> {
        let mut lib = SoundLib::load(map_dir)?;
        let f = mission.factions();
        lib.for_battle(
            map_dir,
            [f[0].as_str(), f[1].as_str()],
            &mission.sound_levels(),
        );
        Some(lib)
    }

    /// The battle's sound sub-levels, best first (`crate::foley::sublevels`).
    pub fn set_levels(&mut self, levels: Vec<u32>) {
        self.levels = levels;
    }

    /// The library of a battle of `factions` on the map at `map_dir` whose
    /// mission loads the sub-levels `named` (card SNDE): those first, and
    /// an army of the other era by its own era's sub-level.
    pub fn for_battle(&mut self, map_dir: &Path, factions: [&str; 2], named: &[String]) {
        self.levels = crate::foley::sublevels(map_dir, factions, named);
        self.foreign = crate::foley::foreign_armies(map_dir, factions, named);
    }

    /// [`SoundLib::for_battle`] for `walk`'s teams and mission.
    pub fn for_walk(&mut self, map_dir: &Path, walk: &crate::walk::WalkSim) {
        let f = [0, 1].map(|t| walk.teams[t].faction.as_str());
        self.for_battle(map_dir, f, &walk.sound_levels);
    }

    /// Decode every take the battle can play (card HITCH): each sample of
    /// the map's and the core bank, and each segment a property of either
    /// names, into the cache - at the battle's load, so no first shot,
    /// footstep or line decodes a file inside a frame. Returns the takes,
    /// their decoded bytes and the ms it took.
    pub fn preload(&mut self) -> (usize, u64, f32) {
        let t0 = std::time::Instant::now();
        let mut paths: Vec<PathBuf> = Vec::new();
        for bank in std::iter::once(&self.map).chain(self.core.as_ref()) {
            paths.extend(bank.sample_paths.values().map(|rel| bank.dir.join(rel)));
            let props = bank.props.values().chain(bank.by_level.values());
            let segs: Vec<u32> = props.flat_map(|p| p.segments.iter().map(|s| s.0)).collect();
            paths.extend(segs.into_iter().filter_map(|id| self.segment_path(id)));
        }
        paths.sort();
        paths.dedup();
        let (mut n, mut bytes) = (0, 0u64);
        for path in paths {
            if self.cache.contains_key(&path) {
                continue;
            }
            // A take that will not decode stays out, as at its first play.
            if let Ok(data) = audio::decode(&path) {
                n += 1;
                bytes += data.frames() as u64 * 8;
                self.cache.insert(path, data);
            }
        }
        (n, bytes, t0.elapsed().as_secs_f32() * 1000.0)
    }

    /// `<map dir>/sounds/sounds.ron`, or None when the map has no sounds baked.
    /// Without a battle the sub-levels are the map's own, as
    /// `crate::foley::sublevels` orders them for no faction.
    ///
    /// The map dir's PARENT is the baked root, so the same call also picks up
    /// the streamed VO index and the resident CORE bank of the map's source
    /// game (`manifest.ron`'s `source_game`: BF1 maps and tat1 → `bf1`, BF2
    /// maps and tat2 → `bf2`). A map dir with no readable manifest gets its own
    /// bank only — there is nothing that says which disc's speech it would use.
    pub fn load(map_dir: &Path) -> Option<SoundLib> {
        let map = Bank::load(&map_dir.join("sounds"))?;
        let game = pak::read_to_string(map_dir.join("manifest.ron"))
            .ok()
            .and_then(|txt| ron::from_str::<manifest::ManifestHead>(&txt).ok())
            .map(|h| h.source_game);
        let buses = bus_table(game.as_deref().unwrap_or(""));
        let root = map_dir.parent();
        let mut indexed: HashMap<u32, bool> = HashMap::new();
        let mut music_takes = HashMap::new();
        let (core, global_dir, global_files) = match (root, game) {
            (Some(root), Some(game)) => {
                let mut core = Bank::load(&root.join(format!("core_{game}")).join("sounds"));
                if let Some(core) = &mut core {
                    core.import_sbd_sounds(root, &game);
                }
                let dir = root.join("audio").join(format!("global_{game}"));
                let g = read_index(&dir.join("global.ron"));
                note_indexed(&g, &mut indexed);
                note_music(&g, &dir, &mut music_takes);
                (core, dir, g.files.into_iter().collect())
            }
            _ => (None, PathBuf::new(), HashMap::new()),
        };
        // The map's own streams, baked beside its bank (item VO).
        let map_streams_dir = map_dir.join("sounds").join("streams");
        let m = read_index(&map_streams_dir.join("streams.ron"));
        note_indexed(&m, &mut indexed);
        note_music(&m, &map_streams_dir, &mut music_takes);
        Some(SoundLib {
            map,
            levels: crate::foley::sublevels(map_dir, ["", ""], &[]),
            foreign: Vec::new(),
            core,
            global_dir,
            global_files,
            map_streams_dir,
            map_stream_files: m.files.into_iter().collect(),
            timers: Vec::new(),
            now: 0.0,
            next_ok: HashMap::new(),
            take_ok: HashMap::new(),
            indexed,
            music_takes,
            cache: HashMap::new(),
            loops: HashMap::new(),
            ambient: HashMap::new(),
            cries: HashMap::new(),
            station_voices: HashMap::new(),
            hero_voices: HashMap::new(),
            hero_blades: Vec::new(),
            seed: 0x1234_5678,
            played: 0,
            handed: None,
            rate_mul: 1.0,
            buses,
        })
    }

    /// The BF2 shell's library (`common_bf2/shell`, its music and VO) with
    /// the resident CORE.LVL bank of `core_bf2` behind it, where the menu
    /// clicks are (`analysis/re-bf2-menus.md` §4.2-§4.3), and BF2's buses.
    /// The shell dir has no manifest, so [`SoundLib::load`] finds no core.
    pub fn load_shell(shell_dir: &Path, assets_dir: &Path) -> Option<SoundLib> {
        let mut lib = SoundLib::load(shell_dir)?;
        lib.core = Bank::load(&assets_dir.join("core_bf2").join("sounds"));
        if lib.core.is_none() {
            eprintln!("shell: no core_bf2 sound bank baked; the menus are silent");
        }
        lib.buses = bus_table("bf2");
        Some(lib)
    }

    /// The map bank's definition of a name: the first of the battle's
    /// sub-levels that defines it (the PS2 holds the one its mission loads,
    /// [`merge_props`]), else the whole LVL's.
    fn map_prop(&self, hash: u32) -> Option<&SoundProp> {
        let own = |l: &u32| self.map.by_level.get(&(*l, hash));
        self.levels
            .iter()
            .find_map(own)
            .or_else(|| self.map.props.get(&hash))
    }

    /// The bake keeps each record's sub-level (false before card SNDE).
    pub fn has_levels(&self) -> bool {
        !self.map.by_level.is_empty()
    }

    /// Every name the map's bank defines in any sub-level, for the referees.
    pub fn map_names(&self) -> impl Iterator<Item = u32> + '_ {
        self.map.props.keys().copied()
    }

    /// The sub-level whose definition of `name` this battle plays (0 = the
    /// resident CORE bank's or a bake without sub-levels); `None` for a name
    /// the library does not have. For the referees.
    pub fn level_of(&self, name: &str) -> Option<u32> {
        self.find_named(name).map(|(p, _)| p.level)
    }

    /// [`SoundLib::audio_of`] by name ([`SoundLib::find_named`]).
    pub fn audio_of_name(&self, name: &str) -> Option<Audio> {
        let (p, _) = self.find_named(name)?;
        let ids = |v: &[(u32, f32)]| v.iter().map(|(id, _)| *id).collect();
        Some((p.stream, ids(&p.segments), ids(&p.samples), p.layers))
    }

    /// The sub-level `level` of the map's bank defines `name`.
    pub fn defines(&self, level: &str, name: &str) -> bool {
        let key = (fnv1a_lowercase(level), fnv1a_lowercase(name));
        self.map.by_level.contains_key(&key)
    }

    /// [`SoundLib::find`] by name: a line of an army the mission's
    /// sub-levels are not of the era of is its own era's sub-level's when
    /// the map has one (`crate::foley::foreign_armies`).
    fn find_named(&self, name: &str) -> Option<(SoundProp, bool)> {
        let hash = fnv1a_lowercase(name);
        let lower = name.to_ascii_lowercase();
        let own = self
            .foreign
            .iter()
            .filter(|(prefix, _)| lower.starts_with(prefix.as_str()))
            .find_map(|(_, level)| self.map.by_level.get(&(*level, hash)));
        match own {
            Some(p) => Some((p.clone(), false)),
            None => self.find(hash),
        }
    }

    /// What a name's definition plays, for the referees: (stream, segment
    /// ids, sample ids, layers).
    pub fn audio_of(&self, hash: u32) -> Option<Audio> {
        let (p, _) = self.find(hash)?;
        let ids = |v: &[(u32, f32)]| v.iter().map(|(id, _)| *id).collect();
        Some((p.stream, ids(&p.segments), ids(&p.samples), p.layers))
    }

    /// A property this library can look up: the map's bank first
    /// ([`SoundLib::map_prop`]), then the resident CORE bank. The bool says
    /// which bank held it — a sample id is only meaningful in its own bank.
    fn find(&self, hash: u32) -> Option<(SoundProp, bool)> {
        if let Some(p) = self.map_prop(hash) {
            return Some((p.clone(), false));
        }
        self.core
            .as_ref()
            .and_then(|c| c.props.get(&hash))
            .map(|p| (p.clone(), true))
    }

    /// The `Music` record `name` with its stream property's takes (card
    /// AMBM): None when the map's bank has no such record or property. A
    /// take the indexes do not hold is left out.
    pub fn music(&self, name: &str) -> Option<MusicTrack> {
        let rec = self.map.music.get(&fnv1a_lowercase(name))?;
        let (prop, _) = self.find(rec.stream)?;
        let stream = prop.stream?;
        let (mut takes, mut ids) = (Vec::new(), Vec::new());
        for (id, _) in &prop.segments {
            if let Some(t) = self.music_takes.get(&(stream, *id)) {
                takes.push(t.clone());
                ids.push(*id);
            }
        }
        Some(MusicTrack {
            def: core::music::MusicDef {
                name: name.to_owned(),
                fade_in: rec.fade_in,
                fade_out: rec.fade_out,
                min_playback: rec.min_playback,
                min_inactive: rec.min_inactive,
                max_playback: rec.max_playback,
                priority: rec.priority,
                looping: prop.looping,
                takes: takes.iter().map(audio::StreamTake::secs).collect(),
            },
            gain: prop.gain,
            takes,
            ids,
        })
    }

    /// A streamed property's takes in its list order and its gain x its
    /// bus (the shell's `shell_soundtrack` on `shellmusic`, card BSHLA);
    /// `None` when it names no indexed stream.
    pub fn stream_takes(&self, name: &str) -> Option<(f32, Vec<audio::StreamTake>)> {
        let (p, _) = self.find_named(name)?;
        let stream = p.stream?;
        let takes: Vec<audio::StreamTake> = p
            .segments
            .iter()
            .filter_map(|(id, _)| self.music_takes.get(&(stream, *id)).cloned())
            .collect();
        (!takes.is_empty()).then_some((p.gain * self.bus_of(&p), takes))
    }

    /// A `soundparameterized` record (a vehicle's `EngineSound`) by name
    /// hash: the map bank's definition in the first of `levels` (the
    /// battle's sub-levels, `crate::foley::sublevels`) that has one - NAB's
    /// fighter tank has other graphs in `nab2gcw` than in `nab1cw` - else
    /// its last one (a class the battle's own sub-levels do not define, as
    /// [`merge_props`]), else the resident CORE bank's.
    pub fn engine(&self, hash: u32, levels: &[u32]) -> Option<&manifest::EngineSound> {
        let core = self.core.as_ref().and_then(|c| c.engines.get(&hash));
        let defs = self.map.engines.get(&hash).or(core)?;
        let of = |l: &u32| defs.iter().find(|e| e.level == *l);
        levels.iter().find_map(of).or(defs.last())
    }

    /// What an engine layer's voice is told (B1 `FUN_00461220`): gain =
    /// clamp(property gain x factor, 0, 1) x the level by distance x the
    /// bus, to each ear by [`ears_3d`] (a 2-D layer: both in full), rate =
    /// max(0.1, property pitch x factor); `None` for an unknown property
    /// and from its mute distance out, where the layer is not played (the
    /// PS2's 39-voice limit is not copied).
    pub fn engine_level(
        &self,
        v: &crate::engine_sound::Voice,
        listener: &Listener,
    ) -> Option<((f32, f32), f32)> {
        self.loop_level(v.sound, v.pos, v.gain_factor, v.pitch_factor, listener)
    }

    pub(super) fn loop_level(
        &self,
        sound: u32,
        pos: Vec3,
        gain_factor: f32,
        pitch_factor: f32,
        listener: &Listener,
    ) -> Option<((f32, f32), f32)> {
        let (p, _) = self.find(sound)?;
        let d = (pos - listener.pos).length();
        if !p.flat && d >= p.mute_distance.unwrap_or(p.max_distance) {
            return None;
        }
        let gain = (p.gain * gain_factor).clamp(0.0, 1.0) * attenuation(d, &p) * self.bus_of(&p);
        let ears = if p.flat {
            (gain, gain)
        } else {
            ears_3d(gain, side(listener, pos))
        };
        Some((ears, (p.pitch * pitch_factor).max(0.1)))
    }

    /// The final gain of the bus `p` plays on ([`bus_table`]; 1.0 for one
    /// the table does not list).
    fn bus_of(&self, p: &SoundProp) -> f32 {
        self.buses
            .iter()
            .find(|b| b.0 == p.bus)
            .map_or(1.0, |b| b.1)
    }

    /// Play this tick's engine layers: a voice per (vehicle or jet soldier,
    /// layer), started
    /// when the layer first plays in range, set to its gain, rate and pan
    /// over one tick, stopped when the layer is no longer listed.
    pub fn loop_voices(
        &mut self,
        audio: &mut audio::Audio,
        voices: &[crate::engine_sound::Voice],
        listener: &Listener,
    ) {
        let mut live = std::collections::HashSet::new();
        for v in voices {
            let request = LoopRequest {
                source: if v.soldier {
                    LoopSource::Jet(v.vehicle, v.layer)
                } else {
                    LoopSource::Engine(v.vehicle, v.layer)
                },
                sound: v.sound,
                pos: v.pos,
                gain: v.gain_factor,
                pitch: v.pitch_factor,
            };
            if self.play_loop_voice(audio, &request, listener) {
                live.insert(request.source);
            }
        }
        self.loops.retain(|k, _| {
            !matches!(k, LoopSource::Engine(..) | LoopSource::Jet(..)) || live.contains(k)
        });
    }

    pub fn guided_loops(
        &mut self,
        audio: &mut audio::Audio,
        sim: &core::sim::Sim,
        listener: &Listener,
    ) {
        let mut requests = Vec::new();
        for r in &sim.guided {
            let Some(c) = sim.guided_class(r) else {
                continue;
            };
            let Some(engine) = c
                .engine_sound
                .as_deref()
                .and_then(|n| self.engine(crate::classdb::fnv1a_lowercase(n), &[]))
            else {
                continue;
            };
            let inputs = crate::engine_sound::Inputs {
                speed: r.vel.length() / c.boost_speed.max(c.speeds[2]).max(0.001),
                health: r.health / c.health.max(0.001),
                timer: r.age,
                ..Default::default()
            };
            for (k, layer) in engine.layers.iter().enumerate() {
                if r.age < layer.delay {
                    continue;
                }
                let (gain, pitch) = crate::engine_sound::factors(layer, &inputs);
                requests.push(LoopRequest {
                    source: LoopSource::Guided(r.id, k as u8),
                    sound: layer.sound,
                    pos: r.pos,
                    gain,
                    pitch,
                });
            }
        }
        let mut live = std::collections::HashSet::new();
        for request in requests {
            if self.play_loop_voice(audio, &request, listener) {
                live.insert(request.source);
            }
        }
        self.loops
            .retain(|k, _| !matches!(k, LoopSource::Guided(..)) || live.contains(k));
        // Card WHUM: the round's own flight hum is the other loop it owns.
        self.ordnance_hums(audio, sim, listener);
    }

    /// Card WHUM (`re-weapon-sounds.md` 0.5, 5.2-5.4): every round in
    /// flight whose class authors `OrdnanceSound` hums on a looping voice
    /// that follows it, pitched by the PS2's doppler
    /// ([`crate::event_sounds::ordnance::doppler`]). The voice is asked on
    /// every tick - a start out of the property's range is refused and a
    /// looping sound is retried - and stops the tick its round leaves
    /// `sim.bolts`. Render-only: `sim` is read, never written.
    pub fn ordnance_hums(
        &mut self,
        audio: &mut audio::Audio,
        sim: &core::sim::Sim,
        listener: &Listener,
    ) {
        let (hums, n) = crate::event_sounds::ordnance::hums(sim, listener);
        for h in hums[..n].iter().flatten() {
            let request = LoopRequest {
                source: LoopSource::Ordnance(h.key),
                sound: h.sound,
                pos: h.pos,
                gain: 1.0,
                pitch: h.pitch,
            };
            self.play_loop_voice(audio, &request, listener);
        }
        // A round still in `sim.bolts` keeps its voice; one that left (hit,
        // expired, recycled) stops it, and `play_loop_voice` retries any
        // round the property's range refused.
        self.loops.retain(|k, _| match k {
            LoopSource::Ordnance(key) => hums[..n].iter().flatten().any(|h| h.key == *key),
            _ => true,
        });
    }

    /// Stop every engine layer's voice (the pause menu, the battle's end);
    /// the next [`SoundLib::loop_voices`] starts the live ones again.
    pub fn stop_loops(&mut self) {
        self.loops.clear();
        self.ambient.clear();
    }

    /// Stop the world's emitters (the battle's end); the next
    /// [`SoundLib::ambient_voices`] starts them again.
    pub fn stop_ambient(&mut self) {
        self.ambient.clear();
    }

    /// Each emitter's level at `ears` (none for a property this bank does
    /// not hold): the PS2's level function ([`attenuation`]) with the
    /// emitter's distances in place of the property's where the world set
    /// them (`crate::ambience::with_distances`).
    pub fn ambient_heard(&self, amb: &crate::ambience::Ambience, ears: Vec3) -> Vec<AmbientHeard> {
        let mut out = Vec::new();
        for (index, e) in amb.emitters.iter().enumerate() {
            let Some((p, _)) = self.find(e.hash) else {
                continue;
            };
            let p = crate::ambience::with_distances(e, &p);
            let dist = (e.pos - ears).length();
            let level = attenuation(dist, &p);
            out.push(AmbientHeard {
                index,
                dist,
                level,
                gain: p.gain * level * self.bus_of(&p),
            });
        }
        out
    }

    /// Play the world's emitters for `listener`, once per stepped tick
    /// (card AMBS): a looping property is a voice while the listener is
    /// inside its mute distance (the PS2 retries a looping sound out of
    /// range every update, `re-weapon-sounds.md` 3.4); a streamed one is
    /// started with the battle and kept, its level set every tick; a
    /// one-shot is asked again whenever its take has ended, paced by its own
    /// `PlayInterval` / `PlayProbability` ([`SoundLib::start`]'s gates).
    /// A stereo stream has no pan: it is told the mean of the two ears'
    /// levels the per-ear law gives (PROVISIONAL [H]: how the PS2 places a
    /// stereo stream played 3-D was not read).
    pub fn ambient_voices(
        &mut self,
        audio: &mut audio::Audio,
        amb: &crate::ambience::Ambience,
        listener: &Listener,
    ) {
        for h in self.ambient_heard(amb, listener.pos) {
            let e = &amb.emitters[h.index];
            let Some((p, core)) = self.find(e.hash) else {
                continue;
            };
            let p = crate::ambience::with_distances(e, &p);
            let ears = if p.flat {
                (h.gain, h.gain)
            } else {
                ears_3d(h.gain, side(listener, e.pos))
            };
            let (db, pan) = mixer_pair(ears);
            if let Some(take) = self.stream_take(&p) {
                let v = self.ambient.entry(h.index).or_insert_with(|| {
                    let v = audio.play_stream(take, db, p.looping);
                    AmbientVoice::Stream(v.map_err(|e| eprintln!("ambience: {e:#}")).ok())
                });
                if let AmbientVoice::Stream(Some(v)) = v {
                    v.set_volume(db, core::TICK_DT);
                }
                continue;
            }
            if !p.looping {
                let until = match self.ambient.get(&h.index) {
                    Some(AmbientVoice::Shot { until }) => *until,
                    _ => 0.0,
                };
                if self.now < until || h.level <= 0.0 {
                    continue;
                }
                let mut starts = Vec::new();
                self.start_prop_with(e.hash, e.pos, listener.pos, Some(e), &mut starts);
                self.mix(audio, &starts, listener);
                let secs = starts.first().map_or(0.0, |s| {
                    let len = self.cache.get(&s.path).map_or(0.0, |d| d.duration_secs());
                    len / (s.pitch * self.rate_mul).max(0.05)
                });
                self.ambient.insert(
                    h.index,
                    AmbientVoice::Shot {
                        until: self.now + secs,
                    },
                );
                continue;
            }
            if h.level <= 0.0 {
                self.ambient.remove(&h.index);
                continue;
            }
            let rate = f64::from((p.pitch * self.rate_mul).max(0.05));
            if let Some(AmbientVoice::Loop(v)) = self.ambient.get_mut(&h.index) {
                if let Some(v) = v {
                    v.set(db, rate, pan, core::TICK_DT);
                }
                continue;
            }
            let voice = self
                .file_take(&p, core)
                .filter(|path| self.sound(audio, path).is_ok())
                .and_then(|path| audio.play_voice(&self.cache[&path], db, rate, pan));
            self.played += u64::from(voice.is_some());
            self.ambient.insert(h.index, AmbientVoice::Loop(voice));
        }
    }

    /// The take a property streams from the disc (an unbaked stereo
    /// stream's segment, as the music), or None when its take is a file.
    fn stream_take(&self, p: &SoundProp) -> Option<audio::StreamTake> {
        let s = p.stream?;
        let id = p.segments.first()?.0;
        if self.segment_path(id).is_some() {
            return None;
        }
        self.music_takes.get(&(s, id)).cloned()
    }

    /// A file take of `p` (weighted pick): a bank sample or a baked segment.
    fn file_take(&mut self, p: &SoundProp, core: bool) -> Option<PathBuf> {
        match self.pick_sample(&p.samples) {
            Some(id) => self.bank(core).and_then(|b| b.sample_path(id)),
            None => self
                .pick_sample(&p.segments)
                .and_then(|id| self.segment_path(id)),
        }
    }

    /// What [`SoundLib::ambient_voices`] would play for emitter `e`, for the
    /// referees: a disc stream's place, or a file; None when the bank has no
    /// such property or no take of it.
    pub fn ambient_take(&mut self, e: &crate::ambience::Emitter) -> Option<AmbientTake> {
        let (p, core) = self.find(e.hash)?;
        if let Some(t) = self.stream_take(&p) {
            return Some(AmbientTake::Stream(t));
        }
        self.file_take(&p, core).map(AmbientTake::File)
    }

    /// The world's emitters sounding now (streams, loops; a one-shot
    /// between its takes too).
    pub fn ambient_playing(&self) -> usize {
        self.ambient
            .values()
            .filter(|v| match v {
                AmbientVoice::Loop(v) => v.is_some(),
                AmbientVoice::Stream(v) => v.is_some(),
                AmbientVoice::Shot { .. } => true,
            })
            .count()
    }

    pub fn has(&self, name: &str) -> bool {
        self.find_named(name).is_some()
    }

    /// Card UTALK: is `name` a line on a stream (the voice slot's), not a
    /// sample-bank sound?
    pub fn streamed(&self, name: &str) -> bool {
        self.find(fnv1a_lowercase(name))
            .is_some_and(|(p, _)| p.stream.is_some())
    }

    /// A definition from the resident CORE.LVL bank (the shell/load bank both
    /// discs keep in memory — HOLD 9's `shell_menu_accept`), by name hash.
    pub fn global(&self, id: u32) -> Option<&SoundProp> {
        self.core.as_ref().and_then(|c| c.props.get(&id))
    }

    /// The file take `nth` of a property would play — a bank WAV or a streamed
    /// VO segment's OGG — without the weighted pick's randomness. None when
    /// the property is unknown, has fewer takes, or names a clip this bake
    /// does not hold.
    pub fn take_path(&self, name: &str, nth: usize) -> Option<PathBuf> {
        let (p, core) = self.find_named(name)?;
        if !p.samples.is_empty() {
            return self.bank(core)?.sample_path(p.samples.get(nth)?.0);
        }
        self.segment_path(p.segments.get(nth)?.0)
    }

    /// [`SoundLib::take_path`] for a property whose NAME is not cracked — the
    /// per-map `<map>_emt` properties are all unnamed hashes (0 of them are in
    /// the sound-name dictionary), so the item-VO tests address them the only
    /// way the disc does.
    pub fn take_path_by_hash(&self, hash: u32, nth: usize) -> Option<PathBuf> {
        let (p, core) = self.find(hash)?;
        if !p.samples.is_empty() {
            return self.bank(core)?.sample_path(p.samples.get(nth)?.0);
        }
        self.segment_path(p.segments.get(nth)?.0)
    }

    /// How many of the MAP bank's distinct properties name a clip this bake
    /// holds, and what the rest are waiting on. Layers are not followed —
    /// this counts definitions that name their own audio.
    pub fn census(&self) -> Census {
        let mut c = Census {
            props: self.map.props.len(),
            ..Default::default()
        };
        for p in self.map.props.keys().filter_map(|h| self.map_prop(*h)) {
            let w = p
                .samples
                .iter()
                .any(|(id, _)| self.map.sample_paths.contains_key(id));
            let s = p
                .segments
                .iter()
                .any(|(id, _)| self.segment_path(*id).is_some());
            if w {
                c.wav += 1;
            }
            if w || s {
                c.any += 1;
                continue;
            }
            match self.silence(p) {
                Some(Silence::Music) => c.music += 1,
                Some(Silence::Unbaked) => c.unbaked += 1,
                Some(Silence::Cut) => c.cut += 1,
                None => {}
            }
        }
        c
    }

    /// Why a definition that names stream segments still makes no sound, or
    /// None when it names none (a layer-only or bank definition). Callers
    /// have already established that no take resolves to a file.
    fn silence(&self, p: &SoundProp) -> Option<Silence> {
        if p.segments.is_empty() {
            return None;
        }
        let known = |music: bool| {
            p.segments
                .iter()
                .any(|(id, _)| self.indexed.get(id) == Some(&music))
        };
        Some(if known(true) {
            Silence::Music
        } else if known(false) {
            Silence::Unbaked
        } else {
            Silence::Cut
        })
    }

    /// The name hashes behind one column of [`SoundLib::census`], sorted —
    /// what item VO's report lists for `Silence::Cut`.
    pub fn silent_props(&self, why: Silence) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .map
            .props
            .keys()
            .filter_map(|h| self.map_prop(*h))
            .filter(|p| {
                !p.samples
                    .iter()
                    .any(|(id, _)| self.map.sample_paths.contains_key(id))
                    && !p
                        .segments
                        .iter()
                        .any(|(id, _)| self.segment_path(*id).is_some())
                    && self.silence(p) == Some(why)
            })
            .map(|p| p.name_hash)
            .collect();
        out.sort_unstable();
        out
    }

    fn bank(&self, core: bool) -> Option<&Bank> {
        if core {
            self.core.as_ref()
        } else {
            Some(&self.map)
        }
    }

    /// The OGG a stream segment was baked to. The map's OWN streams are
    /// searched FIRST: a per-map stream is the narrower scope — the PS2's
    /// mission Lua opens `sound\\<planet>.lvl`'s `<map>` / `<map>_emt`
    /// alongside `sound\\global.lvl`'s game-wide streams
    /// (re-sound-global.md §1.2), so where one id existed in both, the map's
    /// own copy is the take that map's script named. On this disc pair the
    /// order is moot and measured to be: 0 of the ids in any map's own
    /// streams is also in its game's global index (all 34 maps,
    /// `map_streams_never_shadow_the_global_index`).
    fn segment_path(&self, id: u32) -> Option<PathBuf> {
        if let Some(rel) = self.map_stream_files.get(&id) {
            return Some(self.map_streams_dir.join(rel));
        }
        self.global_files
            .get(&id)
            .map(|rel| self.global_dir.join(rel))
    }

    /// Every segment id the map's own streams hold an OGG for (item VO's
    /// census and its overlap test).
    pub fn map_stream_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.map_stream_files.keys().copied()
    }

    /// Segment ids the global index holds an OGG for.
    pub fn global_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.global_files.keys().copied()
    }

    fn next_rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(0x0019_660D)
            .wrapping_add(0x3C6E_F35F);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    fn pick_sample(&mut self, samples: &[(u32, f32)]) -> Option<u32> {
        let total: f32 = samples.iter().map(|s| s.1.max(0.0)).sum();
        if samples.is_empty() {
            return None;
        }
        if total <= 0.0 {
            return Some(samples[0].0);
        }
        let mut r = self.next_rand() * total;
        for (id, w) in samples {
            r -= w.max(0.0);
            if r <= 0.0 {
                return Some(*id);
            }
        }
        Some(samples[samples.len() - 1].0)
    }

    fn mix(&mut self, audio: &mut audio::Audio, starts: &[Start], listener: &Listener) -> bool {
        let mut any = false;
        for s in starts {
            if self.sound(audio, &s.path).is_err() {
                continue;
            }
            let pitch = (s.pitch * self.rate_mul).max(0.05);
            let ears = s.ears(listener);
            let (db, pan) = mixer_pair(ears);
            audio.play_oneshot(&self.cache[&s.path], db, f64::from(pitch), pan);
            let dist = (s.pos - listener.pos).length();
            self.note(s.hash, &s.path, s.gain, ears, pitch, dist);
            self.played += 1;
            any = true;
        }
        any
    }

    /// A one-shot handed to the mixer, for [`SoundLib::handed`]. `path` is
    /// decoded (in the cache).
    fn note(&mut self, hash: u32, path: &Path, gain: f32, ears: (f32, f32), pitch: f32, dist: f32) {
        let secs = self.cache.get(path).map_or(0.0, |d| d.duration_secs()) / pitch;
        if let Some(h) = self.handed.as_mut() {
            h.push(Handed {
                hash,
                path: path.to_path_buf(),
                gain,
                ears,
                pitch,
                dist,
                secs,
            });
        }
    }
}

/// Decode a take ([`audio::decode`]) and, for the frame recorder (card
/// HITCH), note it with its bytes and ms (`render::made`: a take decoded in
/// a battle frame is that frame's cost).
fn decode_noted(path: &Path) -> anyhow::Result<audio::SoundData> {
    let t0 = std::time::Instant::now();
    let data = audio::decode(path)?;
    if render::made::on() {
        let name = path.file_name().map(|n| n.to_string_lossy());
        render::made::note(
            "sound",
            name.as_deref(),
            data.frames() as u64 * 8,
            t0.elapsed().as_secs_f32() * 1000.0,
        );
    }
    Ok(data)
}
#[cfg(test)]
mod tests;
