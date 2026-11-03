//! `PATH` container: spawn paths / AI paths authored in Zero Editor.
//!
//! Layout (byte-inspected on TAT1.LVL, 2026-08-30 — 13 `path` children):
//! - `path` → NAME (string), INFO (u16 nodeCount, u16 kind, u16 word — 3 or 4
//!   on TAT1, meaning open), PROP (repeated `[key NUL][4-byte LE value]`, e.g.
//!   "lanes" 0x40000000 = 2.0f, "num_respect_bridge_flags",
//!   "num_pause_times"), PNTS (nodeCount × 28 B: f32 x, y, z + 4 × f32
//!   rotation quaternion).
//! - Quaternion order: node values like (0, −0, 1, −0) and (0.0999, −0,
//!   0.995, −0) read as (w, x, y, z) pure-yaw rotations (a 180° turn about
//!   Y); the (x, y, z, w) reading would flip nodes upside down. Kept
//!   verbatim in `rot`; consumers derive yaw as 2·atan2(y, w).
//!
//! ERRATUM (2026-08-30, Opus session): INFO's first field was read as a u32
//! node count. It is a u16 count + a u16 `kind`. Every path on TAT1 has
//! kind 0, so the u32 reading worked there and broke on four other BF1 maps
//! (BES2/END1/GEO1/RHN2), whose map-boundary path reads 131078..131082 =
//! 0x2_0006..0x2_000A. Checked across all 213 `path` chunks in BF1's map
//! LVLs: the low u16 equals `PNTS len / 28` in every single one, and kind is
//! 0 everywhere except the five paths literally named "boundary" (kind 2).
//! The kind values beyond that observation are not decoded.
//!
//! CP instances reference these by the SpawnPath PROP ("CP10SpawnPath");
//! the engine spawns units on the path's nodes.
//!
//! BF2 maps instead carry top-level `path` chunks that are whole `.pth`
//! config files (mungefs::config): `Path("name") { ... Nodes(n) { Node() {
//! position(x,y,z); rotation(w,x,y,z); ... } } }`, keyed by hashed names
//! (`position` 0x934F4E0A, `rotation` 0x21AC415F — analysis/re-cp-capture.md
//! §6). The BF2 loader (FUN_0055d060) NEGATES z on load; verified on TAT2:
//! CP1_SpawnPath's first node lands 13 m from cp1 only with z negated. The
//! yaw is mirrored accordingly (quaternion y negated).

use crate::config::parse_tree;
use crate::{fnv1a_lowercase, le, Chunk, UcfbError};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathNode {
    pub pos: [f32; 3],
    /// Quaternion as stored (w, x, y, z — see module doc).
    pub rot: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Path {
    pub name: String,
    pub nodes: Vec<PathNode>,
    /// Raw (key, 4-byte value) pairs from PROP.
    pub props: Vec<(String, u32)>,
    /// BF2 path Properties, preserving strings (including class limits).
    pub flight_props: Vec<(u32, String)>,
    /// INFO's second u16: 0 on ordinary paths, 2 on the map-boundary path
    /// (see the module erratum). Further values undecoded.
    pub kind: u16,
    /// INFO's trailing u16 (meaning open).
    pub info_word: u16,
}

impl Path {
    pub fn parse(c: &Chunk<'_>) -> Result<Path, UcfbError> {
        let bad = || UcfbError::Truncated(c.offset);
        let name = c.child_string(b"NAME").ok_or_else(bad)?;
        let info = c.child(b"INFO").ok_or_else(bad)?;
        if info.payload.len() < 6 {
            return Err(bad());
        }
        let count = u16::from_le_bytes(info.payload[0..2].try_into().unwrap()) as usize;
        let kind = u16::from_le_bytes(info.payload[2..4].try_into().unwrap());
        let info_word = u16::from_le_bytes(info.payload[4..6].try_into().unwrap());
        let mut props = Vec::new();
        if let Some(p) = c.child(b"PROP") {
            let b = p.payload;
            let mut i = 0;
            while i < b.len() {
                let Some(end) = b[i..].iter().position(|&x| x == 0) else {
                    break;
                };
                let key = String::from_utf8_lossy(&b[i..i + end]).into_owned();
                let v0 = i + end + 1;
                if v0 + 4 > b.len() {
                    break;
                }
                let v = u32::from_le_bytes(b[v0..v0 + 4].try_into().unwrap());
                props.push((key, v));
                i = v0 + 4;
            }
        }
        let pnts = c.child(b"PNTS").ok_or_else(bad)?;
        if pnts.payload.len() < count * 28 {
            return Err(bad());
        }
        let f = |o: usize| le::f32_at(pnts.payload, o);
        let nodes = (0..count)
            .map(|n| {
                let o = n * 28;
                PathNode {
                    pos: [f(o), f(o + 4), f(o + 8)],
                    rot: [f(o + 12), f(o + 16), f(o + 20), f(o + 24)],
                }
            })
            .collect();
        Ok(Path {
            name,
            nodes,
            props,
            flight_props: Vec::new(),
            kind,
            info_word,
        })
    }
}

/// True when a `path` chunk is a BF2 config file (DATA children) rather than
/// a BF1 NAME/INFO/PNTS record.
pub fn is_config_path(c: &Chunk<'_>) -> bool {
    c.children().flatten().any(|k| &k.tag == b"DATA")
}

/// Every `Path(...)` block of a BF2 config-format `path` chunk, converted to
/// engine space (z negated, yaw mirrored).
pub fn parse_config_paths(c: &Chunk<'_>) -> Result<Vec<Path>, UcfbError> {
    let h_path = fnv1a_lowercase("Path");
    let h_nodes = fnv1a_lowercase("Nodes");
    let h_node = fnv1a_lowercase("Node");
    let h_pos = fnv1a_lowercase("position");
    let h_rot = fnv1a_lowercase("rotation");
    let tree = parse_tree(c)?;
    let mut out = Vec::new();
    for p in tree.iter().filter(|n| n.hash == h_path) {
        let Some(name) = p.string.clone() else {
            continue;
        };
        let mut nodes = Vec::new();
        for list in p.find_all(h_nodes) {
            for n in list.find_all(h_node) {
                let pos = n.find(h_pos);
                let rot = n.find(h_rot);
                let (Some(pos), Some(rot)) = (pos, rot) else {
                    continue;
                };
                // `f32` is `None` past the values AND on a slot that points
                // into the string blob.
                let (Some([px, py, pz]), Some([r0, r1, r2, r3])) = (pos.f3(), rot.f4()) else {
                    continue;
                };
                nodes.push(PathNode {
                    pos: [px, py, -pz],
                    rot: [r0, r1, -r2, r3],
                });
            }
        }
        out.push(Path {
            name,
            nodes,
            props: Vec::new(),
            flight_props: p
                .find(fnv1a_lowercase("Properties"))
                .map_or_else(Vec::new, |scope| {
                    scope
                        .children
                        .iter()
                        .filter_map(|v| {
                            let value = v
                                .str_arg(0)
                                .map(str::to_owned)
                                .or_else(|| v.f32(0).map(|x| x.to_string()))?;
                            Some((v.hash, value))
                        })
                        .collect()
                }),
            kind: 0,
            info_word: 0,
        });
    }
    Ok(out)
}

/// All `path` children of a `PATH` container chunk.
pub fn parse_all(container: &Chunk<'_>) -> Result<Vec<Path>, UcfbError> {
    let mut out = Vec::new();
    for c in container.children() {
        let c = c?;
        if &c.tag == b"path" {
            out.push(Path::parse(&c)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Data-gated erratum regression: the boundary path on BF1 RHN2 stores
    /// INFO = u16 count 8 + u16 kind 2. Reading that first field as a u32
    /// gives 131080 nodes and the parse fails on a 224-byte PNTS.
    #[test]
    fn rhn2_boundary_path_has_kind_2_and_eight_nodes() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/RHN/RHN2.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: RHN2.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let mut paths = Vec::new();
        for c in crate::chunks(inner).flatten() {
            if &c.tag == b"PATH" {
                paths.extend(parse_all(&c).expect("PATH parses"));
            }
        }
        assert_eq!(paths.len(), 8, "RHN2 PATH holds 8 paths");
        let b = paths
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case("Boundary"))
            .expect("boundary path");
        assert_eq!((b.kind, b.nodes.len()), (2, 8));
        // Every ordinary path is kind 0.
        assert!(paths
            .iter()
            .filter(|p| !p.name.eq_ignore_ascii_case("Boundary"))
            .all(|p| p.kind == 0));
    }

    /// Data-gated Tier-1 on BF2 TAT2.LVL: top-level config paths parse and
    /// CP1_SpawnPath's nodes sit around cp1 (118.36, 0, 39.61) once z is
    /// negated (re-cp-capture.md §6 loader behavior).
    #[test]
    fn tat2_config_paths_parse_in_engine_space() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT2.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: extracted TAT2.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let mut paths = Vec::new();
        for c in crate::chunks(inner) {
            let c = c.unwrap();
            if &c.tag == b"path" && is_config_path(&c) {
                paths.extend(parse_config_paths(&c).unwrap());
            }
        }
        assert!(paths.len() >= 8, "{} paths", paths.len());
        let cp1 = paths
            .iter()
            .find(|p| p.name == "CP1_SpawnPath")
            .expect("CP1_SpawnPath");
        assert_eq!(cp1.nodes.len(), 9);
        for n in &cp1.nodes {
            let dx = n.pos[0] - 118.36;
            let dz = n.pos[2] - 39.61;
            assert!(
                (dx * dx + dz * dz).sqrt() < 40.0,
                "node {:?} far from cp1",
                n.pos
            );
        }
        // First node byte-verified: (131.77, 0.001, -49.32) stored → z flipped.
        let n0 = cp1.nodes[0];
        assert!((n0.pos[0] - 131.77).abs() < 0.01 && (n0.pos[2] - 49.32).abs() < 0.01);
        assert!((n0.rot[0] - 1.0).abs() < 1e-3);
    }

    /// Data-gated Tier-1 on TAT1.LVL: 13 paths; CP3SpawnPath node 0 matches
    /// the raw byte dump.
    #[test]
    fn tat1_paths_parse() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL"
        );
        let Ok(buf) = std::fs::read(p) else {
            eprintln!("SKIP: extracted TAT1.LVL not present");
            return;
        };
        let inner = crate::open(&buf).unwrap();
        let mut paths = Vec::new();
        for c in crate::chunks(inner) {
            let c = c.unwrap();
            if &c.tag == b"PATH" {
                paths.extend(parse_all(&c).unwrap());
            }
        }
        assert_eq!(paths.len(), 13);
        let names: Vec<&str> = paths.iter().map(|p| p.name.as_str()).collect();
        for want in [
            "boundary",
            "CP1SpawnPath",
            "CP2SpawnPath",
            "CP3SpawnPath",
            "CP4SpawnPath",
            "CP7SpawnPath",
            "CP10SpawnPath",
            "TS2Spawn",
            "TS4Spawn",
        ] {
            assert!(names.contains(&want), "missing {want} in {names:?}");
        }
        let cp3 = paths.iter().find(|p| p.name == "CP3SpawnPath").unwrap();
        assert_eq!(cp3.nodes.len(), 13);
        let n0 = cp3.nodes[0];
        assert!((n0.pos[0] + 191.452).abs() < 1e-2 && (n0.pos[2] - 83.784).abs() < 1e-2);
        // (w, x, y, z) pure yaw: |w|² + |y|² = 1.
        assert!((n0.rot[0] * n0.rot[0] + n0.rot[2] * n0.rot[2] - 1.0).abs() < 1e-4);
        assert_eq!(n0.rot[1], -0.0);
        let cp7 = paths.iter().find(|p| p.name == "CP7SpawnPath").unwrap();
        assert_eq!(cp7.props[0], ("lanes".to_string(), 0x4000_0000));
        assert_eq!(cp7.info_word, 4);
        assert_eq!(cp3.info_word, 3);
    }
    fn record(name: &str, text: Option<&str>, numbers: &[f32], scope: &[u8]) -> Vec<u8> {
        let mut payload = fnv1a_lowercase(name).to_le_bytes().to_vec();
        payload.push(if text.is_some() {
            1
        } else {
            numbers.len() as u8
        });
        let blob = text
            .map(|s| format!("{s}\0").into_bytes())
            .unwrap_or_default();
        if text.is_some() {
            payload.extend(4u32.to_le_bytes());
        } else {
            for n in numbers {
                payload.extend(n.to_le_bytes());
            }
        }
        payload.extend((blob.len() as u32).to_le_bytes());
        payload.extend(blob);
        let mut out = crate::build_chunk(b"DATA", &payload);
        if !scope.is_empty() {
            out.extend(crate::build_chunk(b"SCOP", scope));
        }
        out
    }

    #[test]
    fn flight_properties_preserve_class_strings_limits_and_path_scope() {
        let props = [
            record("Class", Some("test_fighter 2"), &[], &[]),
            record("Class1", Some("test_bomber"), &[], &[]),
            record("MaxEntities", None, &[3.0], &[]),
            record("Direction", None, &[-1.0], &[]),
            record("SingleDirection", None, &[1.0], &[]),
            record("Frequency", None, &[0.25], &[]),
            record("EnableObject", Some("test_generator"), &[], &[]),
        ]
        .concat();
        let node = [
            record("position", None, &[1.0, 2.0, 3.0], &[]),
            record("rotation", None, &[1.0, 0.0, 0.0, 0.0], &[]),
            record(
                "Properties",
                None,
                &[1.0],
                &record("Frequency", None, &[99.0], &[]),
            ),
        ]
        .concat();
        let body = [
            record("Properties", None, &[7.0], &props),
            record("Nodes", None, &[1.0], &record("Node", None, &[], &node)),
        ]
        .concat();
        let buf = crate::build_chunk(
            b"path",
            &record("Path", Some("type_EntityPath test"), &[], &body),
        );
        let c = crate::chunks(&buf).next().unwrap().unwrap();
        let paths = parse_config_paths(&c).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].nodes[0].pos, [1.0, 2.0, -3.0]);
        let p = &paths[0].flight_props;
        assert_eq!(
            p.len(),
            7,
            "node properties must not overwrite path admission"
        );
        for (key, value) in [
            ("Class", "test_fighter 2"),
            ("Class1", "test_bomber"),
            ("MaxEntities", "3"),
            ("Direction", "-1"),
            ("SingleDirection", "1"),
            ("Frequency", "0.25"),
            ("EnableObject", "test_generator"),
        ] {
            assert!(p.contains(&(fnv1a_lowercase(key), value.into())), "{key}");
        }
    }

    #[test]
    #[ignore = "requires owned PS2 BF2 map LVLs; no silent missing-data pass"]
    fn ps2_flight_property_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../extracted/bf2/lvl/DATA/_LVL_PS2");
        let mut files = Vec::new();
        for folder in ["HOT", "SPA"] {
            for file in std::fs::read_dir(root.join(folder)).unwrap() {
                let file = file.unwrap().path();
                if file
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("LVL"))
                {
                    files.push(file);
                }
            }
        }
        files.sort();
        let (mut count, mut classes, mut directions, mut objects, mut single) = (0, 0, 0, 0, 0);
        for file in files {
            let bytes = std::fs::read(&file).unwrap();
            for c in crate::chunks(crate::open(&bytes).unwrap())
                .flatten()
                .filter(|c| &c.tag == b"path")
            {
                for p in parse_config_paths(&c)
                    .unwrap()
                    .into_iter()
                    .filter(|p| p.name.to_lowercase().contains("entitypath"))
                {
                    count += 1;
                    for (h, _) in p.flight_props {
                        classes += usize::from(h == fnv1a_lowercase("Class"));
                        directions += usize::from(h == fnv1a_lowercase("Direction"));
                        objects += usize::from(h == fnv1a_lowercase("EnableObject"));
                        single += usize::from(h == fnv1a_lowercase("SingleDirection"));
                    }
                }
            }
        }
        assert_eq!(
            (count, classes, directions, objects, single),
            (448, 429, 165, 65, 2)
        );
    }
}
