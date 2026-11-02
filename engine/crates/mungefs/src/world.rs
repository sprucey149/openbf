//! `wrld` chunk parsing: object instances + regions (audit A2 §4).
//!
//! wrld: NAME, TNAM (terrain name), SNAM (sky name), INFO (u32 regnCount,
//! u32 instCount), then regn/inst chunks.
//! regn INFO: TYPE ("box"/"sphere"/"cylinder"), NAME, XFRM, SIZE 3×f32.
//! inst INFO: TYPE (entity class name), NAME, XFRM.
//! XFRM = 48 B: 3×3 f32 rotation (row-major) + f32[3] translation.
//! PROP = u32 fnv1a(key) + NUL-terminated value string.

use crate::{le, Chunk, UcfbError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub rotation: [[f32; 3]; 3],
    pub translation: [f32; 3],
}

impl Transform {
    fn parse(p: &[u8]) -> Option<Transform> {
        if p.len() < 48 {
            return None;
        }
        let f = |o: usize| le::f32_at(p, o);
        Some(Transform {
            rotation: [
                [f(0), f(4), f(8)],
                [f(12), f(16), f(20)],
                [f(24), f(28), f(32)],
            ],
            translation: [f(36), f(40), f(44)],
        })
    }
}

/// A property: FNV-1a hash of the key + raw value string.
#[derive(Debug, Clone)]
pub struct Prop {
    pub key_hash: u32,
    pub value: String,
}

#[derive(Debug)]
pub struct Instance {
    /// Entity class name (e.g. "com_item_healthrecharge").
    pub class: String,
    pub name: String,
    pub transform: Transform,
    pub props: Vec<Prop>,
}

#[derive(Debug)]
pub struct Region {
    /// "box" | "sphere" | "cylinder".
    pub shape: String,
    pub name: String,
    pub transform: Transform,
    pub size: [f32; 3],
    pub props: Vec<Prop>,
}

/// `Hint` PROP keys, brute-forced then confirmed byte-for-byte against the
/// setters both games switch on (BF1 `FUN_0023f648`, BF2 `FUN_002defe0`).
pub const HINT_PRIMARY_STANCE: u32 = 0xbf0f_1cd1;
pub const HINT_SECONDARY_STANCE: u32 = 0x92c0_5a59;
pub const HINT_MODE: u32 = 0xec6e_e012;
pub const HINT_RADIUS: u32 = 0x0dba_4cb3;
pub const HINT_COMMAND_POST: u32 = 0x844c_cdb4;

/// One `Hint` node: an authored spot the AI is told to use (snipe here, take
/// cover here, jet-jump from here).
///
/// The XFRM is kept AS STORED — unlike `BARR`, a hint's translation is NOT
/// z-negated by either loader, and the nearest-instance fit agrees (7.1 m vs
/// 11.7 on DAG1, 9.7 vs 23.9 on POL1).
#[derive(Debug, Clone)]
pub struct Hint {
    pub name: String,
    /// The INFO `TYPE` string parsed as a number: 0 Snipe, 1 Patrol, 2 Cover,
    /// 4 JetJump, 5 Minefield, 6 Land, 7/8 BF2-only (Fortification,
    /// VehicleCover [H]). Neither game handles 3 and no disc authors it.
    pub kind: u8,
    pub transform: Transform,
    /// The five keys both games resolve, parsed from their value strings.
    /// These are the AUTHORED values: BF1 folds Mode 3 to 0 and masks the
    /// stances at load, and BF2 ignores Mode and Radius entirely — that is
    /// consumer behaviour, not data.
    pub primary_stance: Option<u8>,
    pub secondary_stance: Option<u8>,
    pub mode: Option<u8>,
    pub radius: Option<f32>,
    pub command_post: Option<String>,
    /// Every PROP as stored, resolved or not.
    pub props: Vec<Prop>,
}

/// One `BARR` barrier: a 2-D oriented rectangle in XZ that units of the
/// matching size types may not cross.
#[derive(Debug, Clone)]
pub struct Barrier {
    pub name: String,
    /// AS STORED. The loaders NEGATE the translation's z when they build the
    /// world-space barrier (BF1 `FUN_00177828`, BF2 `FUN_00178ae8`
    /// negate its z) and the disc agrees — barriers only fit the
    /// instances with z flipped. Callers that want a world position must do
    /// the flip; `bake::map` does.
    pub transform: Transform,
    pub size: [f32; 3],
    /// The `AISizeType` mask (BF1 keeps it raw; BF2 keeps bits 0..4 and sets
    /// its own bit 5 = enabled, so BF2 drops the FLYER bit at runtime).
    pub flags: u32,
}

#[derive(Debug)]
pub struct World {
    pub name: String,
    pub terrain_name: Option<String>,
    pub sky_name: Option<String>,
    pub regions: Vec<Region>,
    pub instances: Vec<Instance>,
    pub hints: Vec<Hint>,
    pub barriers: Vec<Barrier>,
}

/// Decode the `PROP` children of a chunk: u32 key hash + NUL-terminated
/// UTF-8 value. Shared by `wrld` instances and `entc` classes (entity.rs) so
/// a fix to the raw-record handling lands in one place.
pub(crate) fn parse_props(parent: &Chunk<'_>) -> Vec<Prop> {
    let mut props = Vec::new();
    for c in parent.children_tagged(b"PROP") {
        let p = c.payload;
        if p.len() < 5 {
            continue;
        }
        let key_hash = u32::from_le_bytes(p[0..4].try_into().unwrap());
        if let Some(end) = p[4..].iter().position(|&b| b == 0) {
            if let Ok(v) = std::str::from_utf8(&p[4..4 + end]) {
                props.push(Prop {
                    key_hash,
                    value: v.to_owned(),
                });
            }
        }
    }
    props
}

/// The `INFO` head the four `wrld` record kinds share. `regn`, `inst`, `Hint`
/// and `BARR` all open by pulling `INFO` -> `TYPE` / `NAME` / `XFRM` out of
/// the record, and two of them then read `INFO SIZE`.
struct RecordHead<'a> {
    info: Chunk<'a>,
    /// `INFO TYPE`: a shape for `regn`, a class for `inst`, a hint class
    /// number for `Hint`. Empty when the record has none — `BARR` never has.
    type_str: String,
    /// `INFO NAME`, empty when absent (only `wrld`'s own NAME is required).
    name: String,
    transform: Transform,
}

impl<'a> RecordHead<'a> {
    /// `at` is the parent `wrld`'s offset, so a record missing its `INFO` or
    /// its `XFRM` reports the same `Truncated(wrld.offset)` as before.
    fn parse(rec: &Chunk<'a>, at: usize) -> Result<RecordHead<'a>, UcfbError> {
        let bad = || UcfbError::Truncated(at);
        let info = rec.child(b"INFO").ok_or_else(bad)?;
        let type_str = info.child_string(b"TYPE").unwrap_or_default();
        let name = info.child_string(b"NAME").unwrap_or_default();
        let transform = info
            .child(b"XFRM")
            .and_then(|c| Transform::parse(c.payload))
            .ok_or_else(bad)?;
        Ok(RecordHead {
            info,
            type_str,
            name,
            transform,
        })
    }

    /// `INFO SIZE`'s three floats — the box half-extents a `regn` or a `BARR`
    /// carries; `[0, 0, 0]` when the chunk is absent or short.
    fn size3(&self) -> [f32; 3] {
        self.info
            .child(b"SIZE")
            .filter(|c| c.payload.len() >= 12)
            .map(|c| {
                let f = |o: usize| le::f32_at(c.payload, o);
                [f(0), f(4), f(8)]
            })
            .unwrap_or([0.0; 3])
    }
}

impl World {
    pub fn parse(wrld: &Chunk<'_>) -> Result<World, UcfbError> {
        let bad = || UcfbError::Truncated(wrld.offset);
        let name = wrld.child_string(b"NAME").ok_or_else(bad)?;
        let terrain_name = wrld.child_string(b"TNAM");
        let sky_name = wrld.child_string(b"SNAM");

        let mut regions = Vec::new();
        for r in wrld.children_tagged(b"regn") {
            let head = RecordHead::parse(&r, wrld.offset)?;
            let size = head.size3();
            regions.push(Region {
                shape: head.type_str,
                name: head.name,
                transform: head.transform,
                size,
                props: parse_props(&r),
            });
        }

        let mut instances = Vec::new();
        for ins in wrld.children_tagged(b"inst") {
            let head = RecordHead::parse(&ins, wrld.offset)?;
            instances.push(Instance {
                class: head.type_str,
                name: head.name,
                transform: head.transform,
                props: parse_props(&ins),
            });
        }

        let mut hints = Vec::new();
        for h in wrld.children_tagged(b"Hint") {
            let head = RecordHead::parse(&h, wrld.offset)?;
            let type_str = head.type_str;
            let kind = type_str.parse::<u8>().map_err(|_| {
                UcfbError::Malformed(format!(
                    "wrld {name}: Hint 0x{:X} TYPE {type_str:?} is not a hint class",
                    h.offset
                ))
            })?;
            let props = parse_props(&h);
            let find = |key: u32| props.iter().find(|p| p.key_hash == key);
            hints.push(Hint {
                name: head.name,
                kind,
                transform: head.transform,
                primary_stance: find(HINT_PRIMARY_STANCE).and_then(|p| p.value.parse().ok()),
                secondary_stance: find(HINT_SECONDARY_STANCE).and_then(|p| p.value.parse().ok()),
                mode: find(HINT_MODE).and_then(|p| p.value.parse().ok()),
                radius: find(HINT_RADIUS).and_then(|p| p.value.parse().ok()),
                command_post: find(HINT_COMMAND_POST).map(|p| p.value.clone()),
                props,
            });
        }

        let mut barriers = Vec::new();
        for b in wrld.children_tagged(b"BARR") {
            let head = RecordHead::parse(&b, wrld.offset)?;
            let size = head.size3();
            let flags = head
                .info
                .child(b"FLAG")
                .filter(|c| c.payload.len() >= 4)
                .map(|c| u32::from_le_bytes(c.payload[0..4].try_into().unwrap()))
                .unwrap_or(0);
            // BARR PROP children exist and both games read and DISCARD them
            // (BF1 FUN_00177af8, BF2 FUN_00178d98), so they are not kept.
            barriers.push(Barrier {
                name: head.name,
                transform: head.transform,
                size,
                flags,
            });
        }

        Ok(World {
            name,
            terrain_name,
            sky_name,
            regions,
            instances,
            hints,
            barriers,
        })
    }
}
