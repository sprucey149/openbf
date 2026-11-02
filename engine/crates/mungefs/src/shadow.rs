//! BF1/BF2 `shdw` closed half-edge meshes (B1 0x4acb20).
//!
//! The PS2 holds `NAME`, `INFO`, `SHDV` and `SHDI` directly in the chunk. The
//! Xbox and PC name the piece in `BNAM` and keep the same three inside
//! `CSHD/DATA`, beside a `GSHD` drawing mesh this reader does not need. A piece
//! with a `BMAP` instead of a `BNAM` is skinned across bones; see [`is_skinned`].
use crate::{Chunk, UcfbError};

#[derive(Debug, Clone)]
pub struct ShadowPiece {
    pub node: String,
    pub positions: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u16>>,
}

/// An Xbox or PC piece bound to several bones through a `BMAP`. Its closed mesh
/// carries no per-vertex bone, so it cannot follow a walker's legs from here.
pub fn is_skinned(c: &Chunk<'_>) -> bool {
    c.child(b"BMAP").is_some()
}

impl ShadowPiece {
    pub fn parse(c: &Chunk<'_>) -> Result<Self, UcfbError> {
        let bad = || UcfbError::Malformed(format!("shadow mesh at 0x{:x}", c.offset));
        let node = c
            .child_string(b"NAME")
            .or_else(|| c.child_string(b"BNAM"))
            .ok_or_else(bad)?;
        let nested = match c.child(b"CSHD") {
            Some(closed) => Some(closed.child(b"DATA").ok_or_else(bad)?),
            None => None,
        };
        let body = nested.as_ref().unwrap_or(c);
        let info = body.child(b"INFO").ok_or_else(bad)?;
        if info.payload.len() != 8 {
            return Err(bad());
        }
        let nv = u32::from_le_bytes(info.payload[0..4].try_into().unwrap()) as usize;
        let ni = u32::from_le_bytes(info.payload[4..8].try_into().unwrap()) as usize;
        let vs = body.child(b"SHDV").ok_or_else(bad)?;
        let es = body.child(b"SHDI").ok_or_else(bad)?;
        if nv > u16::MAX as usize
            || nv < 4
            || !ni.is_multiple_of(4)
            || ni == 0
            || vs.payload.len() != nv * 12
            || es.payload.len() != ni.saturating_mul(2)
        {
            return Err(bad());
        }
        let positions: Vec<[f32; 3]> = vs
            .payload
            .chunks_exact(12)
            .map(|v| {
                std::array::from_fn(|i| f32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap()))
            })
            .collect();
        if positions.iter().flatten().any(|f| !f.is_finite()) {
            return Err(bad());
        }
        let edges: Vec<[usize; 4]> = es
            .payload
            .chunks_exact(8)
            .map(|e| {
                std::array::from_fn(|i| {
                    u16::from_le_bytes(e[i * 2..i * 2 + 2].try_into().unwrap()) as usize
                })
            })
            .collect();
        for (h, e) in edges.iter().enumerate() {
            if e[0] >= nv || e[1] >= edges.len() || e[2] >= edges.len() || e[3] != 65535 {
                return Err(bad());
            }
            let twin = edges[e[2]];
            if twin[2] != h || edges[e[1]][0] != twin[0] {
                return Err(bad());
            }
        }
        let mut visited = vec![false; edges.len()];
        let mut faces = Vec::new();
        for start in 0..edges.len() {
            if visited[start] {
                continue;
            }
            let mut face = Vec::new();
            let mut h = start;
            loop {
                if visited[h] {
                    return Err(bad());
                }
                visited[h] = true;
                face.push(edges[h][0] as u16);
                h = edges[h][1];
                if h == start {
                    break;
                }
            }
            if face.len() < 3 {
                return Err(bad());
            }
            faces.push(face);
        }
        Ok(Self {
            node,
            positions,
            faces,
        })
    }
}

#[cfg(test)]
mod tests {
    fn fixture() -> Vec<u8> {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = tag.to_vec();
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            while !bytes.len().is_multiple_of(4) {
                bytes.push(0);
            }
            bytes
        }
        let faces = [[0u16, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let mut edges = Vec::new();
        for f in faces {
            for i in 0..3 {
                edges.push((f[i], f[(i + 1) % 3]));
            }
        }
        let mut indices = Vec::new();
        for (i, &(a, b)) in edges.iter().enumerate() {
            let twin = edges.iter().position(|&(x, y)| x == b && y == a).unwrap() as u16;
            for n in [a, (i / 3 * 3 + (i + 1) % 3) as u16, twin, 65535] {
                indices.extend(n.to_le_bytes());
            }
        }
        let mut payload = chunk(b"NAME", b"dummyroot\0");
        payload.extend(chunk(
            b"INFO",
            &[4u32.to_le_bytes(), 48u32.to_le_bytes()].concat(),
        ));
        let positions: Vec<u8> = [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        payload.extend(chunk(b"SHDV", &positions));
        payload.extend(chunk(b"SHDI", &indices));
        payload
    }

    #[test]
    fn rejects_truncation_bad_twins_cycles_and_nonfinite_vertices() {
        let valid = fixture();
        let parse = |p: &[u8]| {
            super::ShadowPiece::parse(&crate::Chunk {
                tag: *b"shdw",
                payload: p,
                offset: 0,
            })
        };
        assert_eq!(parse(&valid).unwrap().faces.len(), 4);
        for n in 0..valid.len() {
            assert!(parse(&valid[..n]).is_err(), "accepted truncated length {n}");
        }
        let offset = |tag: &[u8; 4]| valid.windows(4).position(|p| p == tag).unwrap() + 8;
        for (at, bytes) in [
            (offset(b"INFO"), u32::MAX.to_le_bytes().to_vec()),
            (offset(b"SHDI"), 65534u16.to_le_bytes().to_vec()),
            (offset(b"SHDI") + 2, 0u16.to_le_bytes().to_vec()),
            (offset(b"SHDI") + 4, 0u16.to_le_bytes().to_vec()),
            (offset(b"SHDV"), f32::NAN.to_le_bytes().to_vec()),
        ] {
            let mut bad = valid.clone();
            bad[at..at + bytes.len()].copy_from_slice(&bytes);
            assert!(parse(&bad).is_err(), "accepted malformed field at {at}");
        }
    }

    /// The Xbox and PC layout: the same mesh, one level down, named by `BNAM`.
    fn nested(tag: &[u8; 4]) -> Vec<u8> {
        fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut bytes = tag.to_vec();
            bytes.extend((payload.len() as u32).to_le_bytes());
            bytes.extend(payload);
            while !bytes.len().is_multiple_of(4) {
                bytes.push(0);
            }
            bytes
        }
        let flat = fixture();
        let name_end = flat.windows(4).position(|p| p == b"INFO").unwrap();
        let mut closed = chunk(b"INFO", &0u32.to_le_bytes());
        closed.extend(chunk(b"DATA", &flat[name_end..]));
        let mut piece = chunk(b"INFO", &[]);
        piece.extend(chunk(tag, b"dummyroot\0"));
        piece.extend(chunk(b"GSHD", &[0; 8]));
        piece.extend(chunk(b"CSHD", &closed));
        piece
    }

    #[test]
    fn the_xbox_and_pc_layout_reads_the_same_mesh() {
        let shdw = |p: &'static [u8]| crate::Chunk {
            tag: *b"shdw",
            payload: p,
            offset: 0,
        };
        let flat = super::ShadowPiece::parse(&shdw(Vec::leak(fixture()))).unwrap();
        let bnam = shdw(Vec::leak(nested(b"BNAM")));
        assert!(!super::is_skinned(&bnam));
        let read = super::ShadowPiece::parse(&bnam).unwrap();
        assert_eq!(read.node, flat.node);
        assert_eq!(read.positions, flat.positions);
        assert_eq!(read.faces, flat.faces);
        assert!(super::is_skinned(&shdw(Vec::leak(nested(b"BMAP")))));
    }

    /// Xbox and PC BF1, every rigid piece of every LVL: all closed, or the
    /// bake would silently drop a vehicle's shadow on those inputs.
    #[test]
    #[ignore = "requires the extracted Xbox BF1 disc and a PC BF1 install"]
    fn every_rigid_xbox_and_pc_bf1_piece_is_closed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        fn walk(dir: &std::path::Path, counts: &mut [usize; 2]) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, counts);
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lvl")) {
                    let bytes = std::fs::read(&p).unwrap();
                    let Ok(inner) = crate::open(&bytes) else {
                        continue;
                    };
                    let Ok(chunks) = crate::content_chunks(inner) else {
                        continue;
                    };
                    for c in chunks.iter().filter(|c| &c.tag == b"modl") {
                        for s in c.children_tagged(b"shdw") {
                            if super::is_skinned(&s) {
                                counts[1] += 1;
                                continue;
                            }
                            super::ShadowPiece::parse(&s).unwrap_or_else(|e| {
                                panic!("{}: {e}", p.display());
                            });
                            counts[0] += 1;
                        }
                    }
                }
            }
        }
        for tree in ["extracted/xbox_bf1/side", "PC_swbf1_exe"] {
            let mut counts = [0; 2];
            walk(&root.join(tree), &mut counts);
            eprintln!("{tree}: {} rigid, {} skinned", counts[0], counts[1]);
            assert!(counts[0] > 0, "{tree}: read nothing");
        }
    }

    #[test]
    #[ignore = "requires both extracted PS2 discs"]
    fn all_disc_shadow_meshes_are_closed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../extracted");
        fn walk(dir: &std::path::Path, count: &mut usize) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, count);
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lvl")) {
                    let bytes = std::fs::read(&p).unwrap();
                    let Ok(inner) = crate::open(&bytes) else {
                        continue;
                    };
                    for c in crate::content_chunks(inner).unwrap() {
                        if &c.tag == b"modl" {
                            for s in c.children_tagged(b"shdw") {
                                super::ShadowPiece::parse(&s)
                                    .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                                *count += 1;
                            }
                        }
                    }
                }
            }
        }
        let mut count = 0;
        walk(&root.join("bf1"), &mut count);
        walk(&root.join("bf2"), &mut count);
        assert_eq!(count, 404);
    }
}
