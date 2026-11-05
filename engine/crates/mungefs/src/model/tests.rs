use super::*;
use crate::build_chunk as chunk;

#[test]
fn unstitch_two_strips() {
    // strip A: 0,1,2,3 ; strip B: 4,5,6 — starts flagged with 0x8000
    let raw = [
        0x8000, 0x8001, 2, 3, // strip A: quads -> tris (0,1,2),(1->2,1,3)
        0x8004, 0x8005, 6, // strip B: (4,5,6)
    ];
    let tris = unstitch_strips(&raw);
    assert_eq!(tris, vec![[0, 1, 2], [2, 1, 3], [4, 5, 6]]);
}

#[test]
fn degenerates_dropped() {
    let raw = [0x8000, 0x8001, 1, 2]; // (0,1,1) degenerate, (1,1,2) degenerate
    assert_eq!(unstitch_strips(&raw), Vec::<[u32; 3]>::new());
}

/// One `segm` with the given MTRL and RTYP payloads, one vertex, one
/// index — the minimum `parse_segment` accepts.
fn segm(mtrl: &[u8], rtyp: &[u8]) -> Vec<u8> {
    let mut p = chunk(b"INFO", &[1, 0, 0, 0, 1, 0, 0, 0]);
    p.extend(chunk(b"MTRL", mtrl));
    p.extend(chunk(b"RTYP", rtyp));
    p.extend(chunk(b"POSI", &[0u8; 6]));
    p.extend(chunk(b"STRP", &[0u8; 2]));
    chunk(b"segm", &p)
}

fn one_segment(mtrl: &[u8], rtyp: &[u8]) -> Segment {
    let mut p = chunk(b"NAME", b"m\0");
    p.extend(chunk(b"INFO", &[0u8; 68]));
    p.extend(segm(mtrl, rtyp));
    let bytes = chunk(b"modl", &p);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    let mut m = Model::parse(&c).unwrap();
    m.segments.pop().unwrap()
}

/// The MTRL's specular fields in each game's layout
/// (`analysis/re-specular-pass.md` §2): BF2 `{flags, diffuse, spec RGBA,
/// power, ..}`, BF1 `{flags, A = power, B = colour}`. Values from the
/// census (`lead_tmp/re/specular/census_spec.txt`): BF1 `0x31` A 15 B
/// 0xffc9d0 (SIDE/IMP.LVL), BF2 a 0x80 word with power 300.
#[test]
fn the_specular_fields_in_both_layouts() {
    let mut bf1 = Vec::new();
    for w in [0x31u32, 15, 0xffc9d0] {
        bf1.extend(w.to_le_bytes());
    }
    let s = one_segment(&bf1, b"Specmap\0");
    assert!(!s.pc_geometry);
    assert_eq!((s.spec_power, s.spec_color), (15, 0xffc9d0));

    let mut bf2 = vec![0u8; 25];
    for (i, w) in [0x80u32, 0xff80_8080, 0xff40_80ff, 300].iter().enumerate() {
        bf2[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    let s = one_segment(&bf2, &0u32.to_le_bytes());
    assert_eq!((s.spec_power, s.spec_color), (300, 0xff40_80ff));

    // A 4-byte BF1 MTRL carries neither.
    let s = one_segment(&0x31u32.to_le_bytes(), b"Specmap\0");
    assert_eq!((s.spec_power, s.spec_color), (0, 0));
}

#[test]
fn a_bf2_scroll_word_scrolls_by_its_two_bytes() {
    let mut band = vec![0u8; 25];
    band[0..4].copy_from_slice(&0x0101_0010u32.to_le_bytes());
    band[16] = 200;
    band[20] = 255;
    let s = one_segment(&band, &3u32.to_le_bytes());
    assert_eq!(s.uv_anim, [200, 255]);
    assert_eq!(s.uv_scroll(), Some([-200.0 / 255.0, -1.0]));
    band[0..4].copy_from_slice(&0x0001_0010u32.to_le_bytes());
    assert_eq!(one_segment(&band, &3u32.to_le_bytes()).uv_scroll(), None);
    let bf1 = one_segment(&0x0100_0410u32.to_le_bytes(), b"Scroll\0");
    assert_eq!((bf1.uv_anim, bf1.uv_scroll()), ([0, 0], None));
}

#[test]
fn bf1_scroll_material_preserves_its_float_rates() {
    let mut band = Vec::new();
    band.extend(0x414u32.to_le_bytes());
    band.extend(0.0078125f32.to_le_bytes());
    band.extend(0.9921875f32.to_le_bytes());
    assert_eq!(
        one_segment(&band, b"Scroll\0").uv_scroll(),
        Some([-0.0078125, -0.9921875])
    );
    band[0..4].copy_from_slice(&0x30u32.to_le_bytes());
    assert_eq!(one_segment(&band, b"Specmap\0").uv_scroll(), None);
}

/// RTYP's two on-disc encodings, pinned as raw bytes: BF1 writes a
/// NUL-terminated string beside a 4-byte MTRL, BF2 a u32 beside a 25-byte
/// one. Byte layouts taken from the disc (`segm` payloads on KAM1.LVL;
/// see the survey in the `RenderType` doc).
#[test]
fn rtyp_bf1_string_and_bf2_u32() {
    // BF1 kam_prop_crate segment 0: MTRL 0x00000410, RTYP "Scroll\0".
    let bf1 = one_segment(&0x410u32.to_le_bytes(), b"Scroll\0");
    assert_eq!(bf1.material_flags, 0x410);
    assert_eq!(bf1.render_type, RenderType::Bf1("Scroll".into()));
    assert!(bf1.render_type.is_scroll_like());
    assert!(!bf1.bf2_material);

    // BF1 body segment: MTRL 0x00000005, RTYP "Normal\0".
    let body = one_segment(&5u32.to_le_bytes(), b"Normal\0");
    assert_eq!(body.render_type, RenderType::Bf1("Normal".into()));
    assert!(!body.render_type.is_scroll_like());

    // BF1 "Specmap\0" is 8 bytes including the terminator.
    let spec = one_segment(&0x31u32.to_le_bytes(), b"Specmap\0");
    assert_eq!(spec.render_type, RenderType::Bf1("Specmap".into()));
    assert!(spec.render_type.is_scroll_like());

    // BF2 kam_prop_crate segment 0: 25-byte MTRL 0x00010089, RTYP u32 0.
    let mut mtrl = vec![0u8; 25];
    mtrl[0..4].copy_from_slice(&0x1_0089u32.to_le_bytes());
    let bf2 = one_segment(&mtrl, &0u32.to_le_bytes());
    assert_eq!(bf2.material_flags, 0x1_0089);
    assert_eq!(bf2.render_type, RenderType::Bf2(0));
    assert!(!bf2.render_type.is_scroll_like());
    assert!(bf2.bf2_material);

    // BF2's 3 is BF1's Scroll (the census's first evidence for the enum).
    let bf2_scroll = one_segment(&mtrl, &3u32.to_le_bytes());
    assert_eq!(bf2_scroll.render_type, RenderType::Bf2(3));
    assert!(bf2_scroll.render_type.is_scroll_like());
    assert_eq!(bf2_scroll.render_type.label(), "3");

    // No RTYP at all: Absent, never scroll-like.
    let mut p = chunk(b"INFO", &[1, 0, 0, 0, 1, 0, 0, 0]);
    p.extend(chunk(b"MTRL", &7u32.to_le_bytes()));
    p.extend(chunk(b"POSI", &[0u8; 6]));
    p.extend(chunk(b"STRP", &[0u8; 2]));
    let mut modl = chunk(b"NAME", b"m\0");
    modl.extend(chunk(b"INFO", &[0u8; 68]));
    modl.extend(chunk(b"segm", &p));
    let bytes = chunk(b"modl", &modl);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    let seg = &Model::parse(&c).unwrap().segments[0];
    assert_eq!(seg.render_type, RenderType::Absent);
    assert!(!seg.render_type.is_scroll_like());
    assert_eq!(seg.render_type.label(), "");
}

/// Data-gated Tier-1: the RTYP reading agrees with the census's Python
/// walk (`engine/artifacts/texture_census/mtrl_census.py`) on the PLAN
/// entry's known case — `kam_prop_crate`'s four BF1 segments read
/// `Scroll / Normal / Normal / Specmap` with flags `0x410 / 0x05 / 0x01 /
/// 0x31` IN SEGMENT ORDER, and BF2's two read RTYP 0 with flags
/// `0x10089 / 0x10001`.
#[test]
fn rtyp_matches_the_census_on_kam_prop_crate() {
    fn segs(path: &str, want: &str) -> Option<Vec<(u32, RenderType)>> {
        let buf = std::fs::read(path).ok()?;
        let inner = crate::open(&buf).unwrap();
        for c in crate::chunks(inner) {
            let c = c.unwrap();
            if &c.tag != b"modl" {
                continue;
            }
            let m = Model::parse(&c).unwrap();
            if m.name.eq_ignore_ascii_case(want) {
                return Some(
                    m.segments
                        .iter()
                        .map(|s| (s.material_flags, s.render_type.clone()))
                        .collect(),
                );
            }
        }
        None
    }
    let k1 = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/KAM/KAM1.LVL"
    );
    let k2 = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/KAM/KAM1.LVL"
    );
    if !std::path::Path::new(k1).exists() || !std::path::Path::new(k2).exists() {
        eprintln!("SKIP: extracted KAM1 not present");
        return;
    }
    let bf1 = |s: &str| RenderType::Bf1(s.to_owned());
    assert_eq!(
        segs(k1, "kam_prop_crate").expect("BF1 kam_prop_crate"),
        vec![
            (0x410, bf1("Scroll")),
            (0x05, bf1("Normal")),
            (0x01, bf1("Normal")),
            (0x31, bf1("Specmap")),
        ]
    );
    assert_eq!(
        segs(k2, "kam_prop_crate").expect("BF2 kam_prop_crate"),
        vec![
            (0x1_0089, RenderType::Bf2(0)),
            (0x1_0001, RenderType::Bf2(0))
        ]
    );
}

/// Data-gated Tier-1: the BF2 72-byte INFO layout (extra leading u32)
/// dequantizes Mos Eisley models to their real extents; BF1 unchanged.
#[test]
fn modl_info_layout_bf1_68_vs_bf2_72() {
    fn extents(path: &str, want: &str) -> Option<[f32; 3]> {
        let buf = std::fs::read(path).ok()?;
        let inner = crate::open(&buf).unwrap();
        for c in crate::content_chunks(inner).unwrap() {
            if &c.tag == b"modl" {
                let m = Model::parse(&c).unwrap();
                if m.name == want {
                    let b = m.vertex_box;
                    return Some([
                        b.max[0] - b.min[0],
                        b.max[1] - b.min[1],
                        b.max[2] - b.min[2],
                    ]);
                }
            }
        }
        None
    }
    let t2 = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/TAT/TAT2.LVL"
    );
    let t1 = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/TAT/TAT1.LVL"
    );
    if !std::path::Path::new(t2).exists() || !std::path::Path::new(t1).exists() {
        eprintln!("SKIP: extracted TAT1/TAT2 not present");
        return;
    }
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 0.6);
    let cantina = extents(t2, "tat2_bldg_cantina").expect("cantina");
    assert!(near(cantina, [65.0, 13.0, 37.7]), "cantina {cantina:?}");
    let dock = extents(t2, "tat2_bldg_dockingbay_02").expect("dockingbay");
    assert!(near(dock, [39.5, 15.5, 41.1]), "dockingbay {dock:?}");
    let crawler = extents(t1, "tat1_bldg_crawler").expect("crawler");
    assert!(near(crawler, [23.0, 30.7, 59.1]), "crawler {crawler:?}");
}
#[test]
fn pc_model_parses_d3d_buffers() {
    let mut info_payload = Vec::new();
    info_payload.extend(5u32.to_le_bytes());
    info_payload.extend(3u32.to_le_bytes());
    info_payload.extend(1u32.to_le_bytes());

    let mut ibuf_payload = Vec::new();
    ibuf_payload.extend(3u32.to_le_bytes());
    ibuf_payload.extend(0u16.to_le_bytes());
    ibuf_payload.extend(1u16.to_le_bytes());
    ibuf_payload.extend(2u16.to_le_bytes());

    let mut vbuf_payload = Vec::new();
    vbuf_payload.extend(3u32.to_le_bytes());
    vbuf_payload.extend(32u32.to_le_bytes());
    vbuf_payload.extend(0x222u32.to_le_bytes());
    for i in 0..3 {
        vbuf_payload.extend((i as f32).to_le_bytes());
        vbuf_payload.extend(0.0f32.to_le_bytes());
        vbuf_payload.extend(0.0f32.to_le_bytes());
        vbuf_payload.extend(0.0f32.to_le_bytes());
        vbuf_payload.extend(1.0f32.to_le_bytes());
        vbuf_payload.extend(0.0f32.to_le_bytes());
        vbuf_payload.extend(0.5f32.to_le_bytes());
        vbuf_payload.extend(0.5f32.to_le_bytes());
    }

    let mut segm_p = chunk(b"INFO", &info_payload);
    segm_p.extend(chunk(b"IBUF", &ibuf_payload));
    segm_p.extend(chunk(b"VBUF", &vbuf_payload));

    let mut modl_p = chunk(b"NAME", b"test_pc\0");
    modl_p.extend(chunk(b"INFO", &[0u8; 72]));
    modl_p.extend(chunk(b"segm", &segm_p));

    let bytes = chunk(b"modl", &modl_p);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    let m = Model::parse(&c).unwrap();
    assert_eq!(m.name, "test_pc");
    assert!(m.segments[0].pc_geometry);
    assert_eq!(m.segments.len(), 1);
    let s = &m.segments[0];
    assert_eq!(s.vertex_count, 3);
    assert_eq!(s.positions.len(), 3);
    assert_eq!(s.normals.len(), 3);
    assert_eq!(s.uvs.len(), 3);
    assert_eq!(s.triangles.len(), 1);
    assert_eq!(s.triangles[0], [0, 1, 2]);
}

#[test]
fn pc_model_parses_colors_skin_bpv3_and_bf2_rtyp() {
    let mut info_payload = Vec::new();
    info_payload.extend(4u32.to_le_bytes());
    info_payload.extend(3u32.to_le_bytes());
    info_payload.extend(1u32.to_le_bytes());

    let mut ibuf_payload = Vec::new();
    ibuf_payload.extend(3u32.to_le_bytes());
    ibuf_payload.extend(0u16.to_le_bytes());
    ibuf_payload.extend(1u16.to_le_bytes());
    ibuf_payload.extend(2u16.to_le_bytes());

    let mut vbuf_payload = Vec::new();
    vbuf_payload.extend(3u32.to_le_bytes());
    vbuf_payload.extend(60u32.to_le_bytes());
    vbuf_payload.extend(0x362u32.to_le_bytes());
    for i in 0..3 {
        for f in [i as f32, 1.0, 2.0, 0.0, 1.0, 0.0] {
            vbuf_payload.extend(f.to_le_bytes());
        }
        vbuf_payload.extend([0u8; 24]);
        vbuf_payload.extend([77u8, 128, 221, 255]);
        vbuf_payload.extend(0.25f32.to_le_bytes());
        vbuf_payload.extend(0.75f32.to_le_bytes());
    }

    let mut skin_payload = Vec::new();
    skin_payload.extend(3u32.to_le_bytes());
    skin_payload.extend(3u32.to_le_bytes());
    for _ in 0..3 {
        skin_payload.extend([10u8, 45, 1, 2, 3]);
    }

    let mut mtrl = vec![0u8; 25];
    mtrl[0..4].copy_from_slice(&0x0101_0080u32.to_le_bytes());

    let mut segm_p = chunk(b"INFO", &info_payload);
    segm_p.extend(chunk(b"MTRL", &mtrl));
    segm_p.extend(chunk(b"RTYP", b"Normal\0"));
    segm_p.extend(chunk(b"BMAP", &[4, 0, 0, 0, 5, 6, 7, 8]));
    segm_p.extend(chunk(b"SKIN", &skin_payload));
    segm_p.extend(chunk(b"IBUF", &ibuf_payload));
    segm_p.extend(chunk(b"VBUF", &vbuf_payload));

    let mut modl_p = chunk(b"NAME", b"test_pc_full\0");
    modl_p.extend(chunk(b"INFO", &[0u8; 72]));
    modl_p.extend(chunk(b"segm", &segm_p));

    let bytes = chunk(b"modl", &modl_p);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    let m = Model::parse(&c).unwrap();
    let s = &m.segments[0];
    assert_eq!(s.render_type, RenderType::Bf2(3));
    assert_eq!(s.index_count, 3);
    assert_eq!(s.uvs[0], [0.25, 0.75]);
    assert_eq!(s.colors.as_ref().unwrap()[0], [111, 64, 39, 128]);
    assert_eq!(s.wght[0], [200, 45, 10, 0]);
    assert_eq!(s.bind[0], [12, 8, 4, 0]);
    assert_eq!(s.vertex_bone(0), Some(8));
}
