//! Resource inventory of one LVL: every named chunk with a payload digest, the
//! `lvl_` sublevel hashes, the class property digests, and the source game and
//! platform the chunk layouts identify. The stock catalog
//! (`extensions::mods::catalog`) and the legacy replacement resolver are built
//! from these facts; nothing here decodes pixels or geometry.
//!
//! Detection rules, each verified by a census over every stock LVL of both
//! games on PC and PS2 and both Xbox discs (2026-11-24, scratch census tools):
//! - **game**: a `modl`'s `INFO` is 72 B in BF2 and 68 B in BF1, on PC and
//!   PS2 alike. An LVL without a `modl` has no game tell and stays `None`.
//! - **platform**: a PC `tex_` carries `FMT_` children ([`crate::texture_pc`]),
//!   an Xbox one is a single 20 B `INFO` + one `BODY`
//!   ([`crate::texture_xbox::is_xbox`]), a PS2 one has `pal_` or per-mip
//!   `BODY`s with a 14 B `INFO`. A PC `segm` has `VBUF` + `VDAT`, an Xbox one
//!   only packed `VBUF`s ([`crate::model::is_xbox_geometry`]), a PS2 one has
//!   `POSI` / `STRP` / `_PS2` children. The majority vote wins.

use extensions::mods::catalog::{LvlFacts, LvlResource, SourceGame, SourcePlatform};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

use crate::{Chunk, CONTENT_TAGS};

/// Chunk tags that are resources even though [`CONTENT_TAGS`] does not list
/// them (it lists only what the bake decodes). Seen at the content level of
/// stock LVLs by the same census.
const EXTRA_RESOURCE_TAGS: &[&[u8; 4]] = &[
    b"sky_", b"hud_", b"plnp", b"anmg", b"zenv", b"SHDW", b"emo_", b"loc_", b"bnk_", b"sanm",
    b"ffx_", b"prp_", b"Wbnk", b"_pad", b"zaa_", b"zaf_", b"mcfg",
];

const CLASS_TAGS: &[&[u8; 4]] = &[b"entc", b"ordc", b"wpnc", b"expc"];

/// 64-bit FNV-1a, the catalog's payload digest.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn fnv1a64_extend(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn is_resource_tag(tag: &[u8; 4]) -> bool {
    CONTENT_TAGS.contains(&tag) || EXTRA_RESOURCE_TAGS.contains(&tag)
}

fn tag_label(tag: &[u8; 4]) -> String {
    if tag.iter().all(|&b| (0x20..0x7f).contains(&b)) {
        String::from_utf8_lossy(tag).into_owned()
    } else {
        format!("{:08x}", u32::from_le_bytes(*tag))
    }
}

fn first_str(payload: &[u8]) -> Option<String> {
    let end = payload.iter().position(|&b| b == 0).unwrap_or(payload.len());
    let s = std::str::from_utf8(&payload[..end]).ok()?;
    (!s.is_empty()).then(|| s.to_ascii_lowercase())
}

fn resource_name(c: &Chunk<'_>) -> String {
    for child in c.children() {
        let Ok(child) = child else { break };
        match &child.tag {
            b"NAME" | b"TYPE" => {
                if let Some(s) = first_str(child.payload) {
                    return s;
                }
            }
            b"INFO" if &c.tag == b"skel" => {
                if let Some(s) = first_str(child.payload) {
                    return s;
                }
            }
            _ => {}
        }
    }
    String::new()
}

#[derive(Default)]
struct Votes {
    game: BTreeMap<SourceGame, u32>,
    platform: BTreeMap<SourcePlatform, u32>,
}

impl Votes {
    fn modl(&mut self, c: &Chunk<'_>) {
        for child in c.children() {
            let Ok(child) = child else { break };
            match &child.tag {
                b"INFO" => match child.payload.len() {
                    72 => *self.game.entry(SourceGame::Bf2).or_default() += 1,
                    68 => *self.game.entry(SourceGame::Bf1).or_default() += 1,
                    _ => {}
                },
                b"segm" => {
                    if crate::model::is_xbox_geometry(&child) {
                        *self.platform.entry(SourcePlatform::Xbox).or_default() += 1;
                    } else if child.child(b"VDAT").is_some() || child.child(b"VBUF").is_some() {
                        *self.platform.entry(SourcePlatform::Pc).or_default() += 1;
                    } else if child.child(b"POSI").is_some()
                        || child.child(b"STRP").is_some()
                        || child.child(b"_PS2").is_some()
                    {
                        *self.platform.entry(SourcePlatform::Ps2).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }

    fn tex(&mut self, c: &Chunk<'_>) {
        let p = if c.child(b"FMT_").is_some() {
            SourcePlatform::Pc
        } else if crate::texture_xbox::is_xbox(c) {
            SourcePlatform::Xbox
        } else if c.child(b"pal_").is_some() || c.child(b"BODY").is_some() {
            SourcePlatform::Ps2
        } else {
            return;
        };
        *self.platform.entry(p).or_default() += 1;
    }

    fn winner<K: Copy + Ord>(m: &BTreeMap<K, u32>) -> Option<K> {
        let mut best: Option<(K, u32)> = None;
        let mut tie = false;
        for (&k, &n) in m {
            match best {
                Some((_, b)) if n == b => tie = true,
                Some((_, b)) if n < b => {}
                _ => {
                    best = Some((k, n));
                    tie = false;
                }
            }
        }
        if tie {
            None
        } else {
            best.map(|(k, _)| k)
        }
    }
}

fn class_props(c: &Chunk<'_>) -> (Vec<(u32, u64)>, Option<String>) {
    let mut by_key: BTreeMap<u32, u64> = BTreeMap::new();
    let mut base = None;
    for child in c.children() {
        let Ok(child) = child else { break };
        match &child.tag {
            b"BASE" => base = first_str(child.payload),
            b"PROP" if child.payload.len() >= 4 => {
                let key = u32::from_le_bytes(child.payload[0..4].try_into().unwrap());
                let h = by_key.entry(key).or_insert(0xcbf2_9ce4_8422_2325);
                *h = fnv1a64_extend(*h, &child.payload[4..]);
                *h = fnv1a64_extend(*h, &[0xff]);
            }
            _ => {}
        }
    }
    (by_key.into_iter().collect(), base)
}

fn visit(buf: &[u8], base: usize, facts: &mut LvlFacts, votes: &mut Votes, depth: u32) {
    if depth > 8 {
        facts.error.get_or_insert_with(|| "sublevels nest too deep".into());
        return;
    }
    let parent = Chunk {
        tag: *b"ucfb",
        payload: buf,
        offset: base.saturating_sub(8),
    };
    for c in parent.children() {
        let c = match c {
            Ok(c) => c,
            Err(e) => {
                facts.error.get_or_insert_with(|| e.to_string());
                return;
            }
        };
        if &c.tag == b"lvl_" {
            for sub in c.children() {
                let sub = match sub {
                    Ok(s) => s,
                    Err(e) => {
                        facts.error.get_or_insert_with(|| e.to_string());
                        break;
                    }
                };
                if is_resource_tag(&sub.tag) {
                    record(&sub, facts, votes);
                } else {
                    facts.sublevels.push(sub.tag_u32());
                    visit(sub.payload, sub.offset + 8, facts, votes, depth + 1);
                }
            }
        } else {
            record(&c, facts, votes);
        }
    }
}

fn record(c: &Chunk<'_>, facts: &mut LvlFacts, votes: &mut Votes) {
    match &c.tag {
        b"modl" => votes.modl(c),
        b"tex_" => votes.tex(c),
        _ => {}
    }
    let (props, base) = if CLASS_TAGS.contains(&&c.tag) {
        class_props(c)
    } else {
        (Vec::new(), None)
    };
    facts.resources.push(LvlResource {
        tag: tag_label(&c.tag),
        name: if is_resource_tag(&c.tag) {
            resource_name(c)
        } else {
            String::new()
        },
        digest: fnv1a64(c.payload),
        size: c.payload.len() as u32,
        props,
        base,
    });
}

/// Inventory an LVL already in memory.
pub fn scan_bytes(bytes: &[u8]) -> LvlFacts {
    let mut facts = LvlFacts {
        size: bytes.len() as u64,
        ..Default::default()
    };
    let payload = match crate::open(bytes) {
        Ok(p) => p,
        Err(e) => {
            facts.sha256 = sha256_hex(bytes);
            facts.error = Some(e.to_string());
            return facts;
        }
    };
    if payload.is_empty() {
        facts.sha256 = sha256_hex(bytes);
        facts.stub = true;
        return facts;
    }
    let mut votes = Votes::default();
    // Large LVLs (e.g. 200 MB–1 GB side LVLs) hash and walk chunks concurrently.
    if bytes.len() >= 4 * 1024 * 1024 {
        let sha = std::thread::scope(|s| {
            let h = s.spawn(|| sha256_hex(bytes));
            visit(payload, 8, &mut facts, &mut votes, 0);
            h.join().unwrap_or_else(|_| sha256_hex(bytes))
        });
        facts.sha256 = sha;
    } else {
        facts.sha256 = sha256_hex(bytes);
        visit(payload, 8, &mut facts, &mut votes, 0);
    }
    facts.game = Votes::winner(&votes.game);
    facts.platform = Votes::winner(&votes.platform);
    facts
}

/// Inventory an LVL on disk.
pub fn scan_file(path: &Path) -> std::io::Result<LvlFacts> {
    let bytes = std::fs::read(path)?;
    Ok(scan_bytes(&bytes))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_chunk;

    fn ucfb(children: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = children.concat();
        build_chunk(b"ucfb", &body)
    }

    fn named(tag: &[u8; 4], name: &str, extra: &[Vec<u8>]) -> Vec<u8> {
        let mut n = name.as_bytes().to_vec();
        n.push(0);
        let mut body = build_chunk(b"NAME", &n);
        for e in extra {
            body.extend(e);
        }
        build_chunk(tag, &body)
    }

    fn class(name: &str, base: &str, props: &[(u32, &str)]) -> Vec<u8> {
        let mut body = build_chunk(b"BASE", format!("{base}\0").as_bytes());
        body.extend(build_chunk(b"TYPE", format!("{name}\0").as_bytes()));
        for (k, v) in props {
            let mut p = k.to_le_bytes().to_vec();
            p.extend(v.as_bytes());
            p.push(0);
            body.extend(build_chunk(b"PROP", &p));
        }
        build_chunk(b"entc", &body)
    }

    #[test]
    fn inventories_sublevels_classes_game_and_platform() {
        let modl = named(
            b"modl",
            "imp_inf_trooper",
            &[
                build_chunk(b"INFO", &[0; 72]),
                build_chunk(
                    b"segm",
                    &[build_chunk(b"VBUF", &[0; 12]), build_chunk(b"VDAT", &[0; 4])].concat(),
                ),
            ],
        );
        let tex = named(b"tex_", "IMP_Trooper", &[build_chunk(b"FMT_", &[])]);
        let cls = class("imp_inf_rifleman", "imp_inf_default", &[(7, "a"), (9, "b"), (7, "c")]);
        let sub_hash = crate::fnv1a_lowercase("imp_inf_rifleman");
        let sub = build_chunk(&sub_hash.to_le_bytes(), &[modl, tex].concat());
        let lvl = build_chunk(b"lvl_", &sub);
        let file = ucfb(&[lvl, cls]);
        let f = scan_bytes(&file);
        assert_eq!(f.error, None);
        assert_eq!(f.sublevels, vec![sub_hash]);
        assert_eq!(f.game, Some(SourceGame::Bf2));
        assert_eq!(f.platform, Some(SourcePlatform::Pc));
        let keys = f.keys();
        assert!(keys.contains(&("tex_".into(), "imp_trooper".into())));
        assert!(keys.contains(&("modl".into(), "imp_inf_trooper".into())));
        let c = f.resources.iter().find(|r| r.tag == "entc").unwrap();
        assert_eq!(c.name, "imp_inf_rifleman");
        assert_eq!(c.base.as_deref(), Some("imp_inf_default"));
        assert_eq!(c.props.len(), 2);

        let cls2 = class("imp_inf_rifleman", "imp_inf_default", &[(7, "c"), (9, "b"), (7, "a")]);
        let f2 = scan_bytes(&ucfb(&[cls2]));
        let c2 = &f2.resources[0];
        assert_eq!(c2.props.iter().find(|p| p.0 == 9), c.props.iter().find(|p| p.0 == 9));
        assert_ne!(c2.props.iter().find(|p| p.0 == 7), c.props.iter().find(|p| p.0 == 7));
    }

    #[test]
    fn stubs_and_garbage_are_reported_not_guessed() {
        let stub = build_chunk(b"ucfb", &[]);
        let f = scan_bytes(&stub);
        assert!(f.stub && f.error.is_none() && f.resources.is_empty());
        let g = scan_bytes(b"PK\x03\x04 not an lvl");
        assert!(g.error.is_some() && !g.stub);
        let bf1 = ucfb(&[named(b"modl", "x", &[build_chunk(b"INFO", &[0; 68])])]);
        let f1 = scan_bytes(&bf1);
        assert_eq!(f1.game, Some(SourceGame::Bf1));
        assert_eq!(f1.platform, None);
    }
}
