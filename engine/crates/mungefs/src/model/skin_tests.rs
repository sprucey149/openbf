use super::*;

fn close3(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < tol)
}

fn find(inner: &[u8], name: &str) -> (Model, crate::skel::Skeleton) {
    let chunks = crate::content_chunks(inner).unwrap();
    let modl = chunks
        .iter()
        .filter(|c| &c.tag == b"modl")
        .map(Model::parse)
        .filter_map(Result::ok)
        .find(|m| m.name == name)
        .unwrap();
    let skel = chunks
        .iter()
        .filter(|c| &c.tag == b"skel")
        .map(crate::skel::parse_skel)
        .filter_map(Result::ok)
        .find(|s| s.name == name)
        .unwrap();
    (modl, skel)
}

fn aabb(p: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in p {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }
    (min, max)
}

/// re-softskin.md §10 vectors 6–9: BF1 stormtrooper.
#[test]
fn imp_lvl_stormtrooper_softskin() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL"
    );
    let Ok(buf) = std::fs::read(p) else {
        eprintln!("SKIP: IMP.LVL not present");
        return;
    };
    let inner = crate::open(&buf).unwrap();
    let (m, skel) = find(inner, "imp_inf_stormtrooper");
    assert!(m.bone_space);
    assert!(close3(
        m.vertex_box.min,
        [-0.197186, -0.206013, -0.209799],
        1e-5
    ));
    assert!(close3(m.vis_box.max, [0.929476, 2.08769, 0.215175], 1e-5));
    assert_eq!(m.segments.len(), 1);
    let seg = &m.segments[0];
    assert_eq!((seg.vertex_count, seg.index_count), (1387, 2370));
    assert_eq!(seg.bmap.len(), 21);
    assert_eq!(&seg.bmap[..4], &[1, 2, 3, 4]);
    assert_eq!(&seg.bmap[19..], &[0x15, 0x16]);
    assert_eq!(seg.wght[0], [255, 0, 0, 0]);
    assert!(seg.wght.iter().all(|w| *w == [255, 0, 0, 0]));
    assert_eq!(seg.bind[0], [0x38, 0x34, 0x34, 0x34]);
    assert_eq!(seg.bind[2], [0x48, 0x44, 0x44, 0x44]);
    assert_eq!(seg.vertex_bone(0), Some(15)); // bone_r_toe
    assert_eq!(seg.vertex_bone(2), Some(19)); // bone_l_toe
    assert_eq!(seg.vertex_bone(150), Some(12)); // bone_r_thigh
    assert!(close3(seg.positions[0], [0.0634, 0.0353, 0.0857], 1e-3));
    let (pos, _nrm, bones) = to_model_space(&m, seg, &skel).unwrap();
    assert_eq!(bones[0], 15);
    assert!(
        close3(pos[0], [-0.1770, -0.0012, 0.1237], 1e-3),
        "{:?}",
        pos[0]
    );
    assert!(close3(pos[2], [0.1769, -0.0010, 0.1237], 1e-3));
    assert!(close3(pos[150], [-0.0990, 0.6066, 0.0135], 1e-3));
    let (min, max) = aabb(&pos);
    assert!(
        close3(min, m.vis_box.min, 1e-4),
        "{min:?} vs {:?}",
        m.vis_box.min
    );
    assert!(
        close3(max, m.vis_box.max, 1e-4),
        "{max:?} vs {:?}",
        m.vis_box.max
    );
    assert_eq!(seg.triangles.len(), 1630);
}

/// §10 vector 12: BF2 ep3 trooper (72-B INFO, different bone order).
#[test]
fn rep_lvl_ep3trooper_softskin() {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL"
    );
    let Ok(buf) = std::fs::read(p) else {
        eprintln!("SKIP: REP.LVL not present");
        return;
    };
    let inner = crate::open(&buf).unwrap();
    let (m, skel) = find(inner, "rep_inf_ep3trooper");
    assert!(m.bone_space);
    let seg = &m.segments[0];
    assert_eq!((seg.vertex_count, seg.index_count), (1710, 2698));
    assert_eq!(seg.bmap.len(), 21);
    assert_eq!(seg.vertex_bone(0), Some(17));
    let (pos, _, _) = to_model_space(&m, seg, &skel).unwrap();
    assert!(
        close3(pos[0], [-0.1795, -0.0012, 0.1156], 1e-3),
        "{:?}",
        pos[0]
    );
    assert!(close3(pos[2], [0.1794, -0.0010, 0.1156], 1e-3));
    let (min, max) = aabb(&pos);
    assert!(close3(min, [-0.92948, -0.00120, -0.23314], 1e-3), "{min:?}");
    assert!(close3(max, [0.92948, 2.08267, 0.20353], 1e-3), "{max:?}");
}

/// Real data: the same model on the Xbox and PS2 discs has the same
/// vertices. The discs are separate munges and some models really were
/// reworked (`rep_hover_barcspeederLOWD`), so a segment is a match when
/// every vertex's position is within the rounding of both boxes, and at
/// least 95 % of comparable segments must be. On those, under 1 % of
/// vertices may have a normal off by more than the PS2's i8, or a different
/// uv or top bone.
#[test]
fn xbox_geometry_matches_the_ps2() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../extracted");
    let (mut compared, mut matched, mut skinned, mut soft) = (0usize, 0usize, 0usize, 0usize);
    let (mut vertices, mut normals_off, mut uv_total, mut uv_off) = (0usize, 0, 0usize, 0);
    let (mut bones, mut bones_off) = (0usize, 0);
    // Triangles whose face points along their vertices' normal: the check
    // that the Xbox strip's winding is the PS2's, not a mirror of it.
    let facing = |s: &Segment| {
        let (mut out, mut all) = (0usize, 0usize);
        for t in &s.triangles {
            let [a, b, c] = t.map(|i| s.positions[i as usize]);
            let sub = |p: [f32; 3], q: [f32; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
            let (u, v) = (sub(b, a), sub(c, a));
            let face = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let n = s.normals[t[0] as usize];
            out += usize::from(face[0] * n[0] + face[1] * n[1] + face[2] * n[2] > 0.0);
            all += 1;
        }
        (out, all)
    };
    let (mut xbox_out, mut xbox_all, mut ps2_out, mut ps2_all) = (0, 0, 0, 0);
    for (xbox, ps2) in [
        (
            "xbox_bf2/side/rep.lvl",
            "bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL",
        ),
        (
            "xbox_bf2/side/all.lvl",
            "bf2/lvl/DATA/_LVL_PS2/SIDE/ALL.LVL",
        ),
        (
            "xbox_bf1/side/rep.lvl",
            "bf1/lvl/DATA/_LVL_PS2/SIDE/REP.LVL",
        ),
        (
            "xbox_bf1/side/cis.lvl",
            "bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL",
        ),
        (
            "xbox_bf1/geo/geo1.lvl",
            "bf1/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL",
        ),
        (
            "xbox_bf1/end/end1.lvl",
            "bf1/lvl/DATA/_LVL_PS2/END/END1.LVL",
        ),
    ] {
        let (Ok(xb), Ok(pb)) = (
            std::fs::read(format!("{root}/{xbox}")),
            std::fs::read(format!("{root}/{ps2}")),
        ) else {
            eprintln!("SKIP: {xbox} not present");
            return;
        };
        let models = |buf: &[u8]| -> Vec<Model> {
            crate::content_chunks(crate::open(buf).unwrap())
                .unwrap()
                .into_iter()
                .filter(|c| &c.tag == b"modl")
                .map(|c| Model::parse(&c).unwrap())
                .collect()
        };
        let ps2_models = models(&pb);
        for xm in models(&xb) {
            assert!(
                xm.segments.iter().all(|s| !s.pc_geometry),
                "{}: read as PC",
                xm.name
            );
            let Some(pm) = ps2_models.iter().find(|m| m.name == xm.name) else {
                continue;
            };
            if xm.segments.len() != pm.segments.len() {
                continue;
            }
            let step = |i: usize| {
                let extent = |m: &Model| (m.vertex_box.max[i] - m.vertex_box.min[i]) / 65535.0;
                16.0 * extent(&xm).max(extent(pm)) + 1e-3
            };
            for (xs, ps) in xm.segments.iter().zip(&pm.segments) {
                if xs.vertex_count != ps.vertex_count {
                    continue;
                }
                compared += 1;
                let same_place = (0..xs.positions.len()).all(|v| {
                    (0..3).all(|i| (xs.positions[v][i] - ps.positions[v][i]).abs() <= step(i))
                });
                if !same_place {
                    continue;
                }
                matched += 1;
                let (o, a) = facing(xs);
                (xbox_out, xbox_all) = (xbox_out + o, xbox_all + a);
                let (o, a) = facing(ps);
                (ps2_out, ps2_all) = (ps2_out + o, ps2_all + a);
                // A segment with no real texture ships junk in the uv slot.
                let textured = xs
                    .textures
                    .iter()
                    .any(|(_, t)| !t.is_empty() && t != "noIcon");
                for v in 0..xs.positions.len() {
                    vertices += 1;
                    normals_off += usize::from(
                        (0..3).any(|i| (xs.normals[v][i] - ps.normals[v][i]).abs() >= 0.04),
                    );
                    if textured {
                        uv_total += 1;
                        uv_off += usize::from(xs.uvs[v] != ps.uvs[v]);
                    }
                }
                // A one-bone soft segment on one disc can be hard on the other.
                if xs.bmap == ps.bmap && !xs.bind.is_empty() && !ps.bind.is_empty() {
                    skinned += 1;
                    for v in 0..xs.bind.len() {
                        // Only the top bone: the PS2 keeps slot order among
                        // equal weights differently, and pads unused slots.
                        bones += 1;
                        bones_off += usize::from(xs.bind[v][0] != ps.bind[v][0]);
                        soft += usize::from(xs.wght[v][1] > 0);
                    }
                }
            }
        }
    }
    if compared > 0 {
        eprintln!(
            "compared {compared} segments, {matched} match, {skinned} skinned, {soft} soft \
             vertices; off: normals {normals_off}/{vertices}, uvs {uv_off}/{uv_total}, \
             bones {bones_off}/{bones}"
        );
        assert!(compared > 100, "{compared}");
        assert!(matched * 100 >= compared * 95, "{matched}/{compared}");
        assert!(skinned > 0);
        assert!(normals_off * 100 <= vertices, "{normals_off}/{vertices}");
        assert!(uv_off * 100 <= uv_total, "{uv_off}/{uv_total}");
        assert!(bones_off * 100 <= bones, "{bones_off}/{bones}");
        eprintln!("facing out: xbox {xbox_out}/{xbox_all}, ps2 {ps2_out}/{ps2_all}");
        let share = |o: usize, a: usize| o as f64 / a.max(1) as f64;
        assert!(
            (share(xbox_out, xbox_all) - share(ps2_out, ps2_all)).abs() < 0.02,
            "winding: xbox {xbox_out}/{xbox_all}, ps2 {ps2_out}/{ps2_all}"
        );
    }
}

#[test]
fn normal_packing() {
    assert_eq!(unpack_normal(0x3FF), [1023.0 / 1023.0, 0.0, 0.0]);
    let [x, y, z] = unpack_normal(0x4AD9_A074);
    assert!((x - 0.113).abs() < 0.001, "{x}");
    assert!((y - 0.801).abs() < 0.001, "{y}");
    assert!((z - 0.585).abs() < 0.001, "{z}");
    assert_eq!(unpack_normal(0xFFFF_FFFF)[0], -1.0 / 1023.0);
}

fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    crate::build_chunk(tag, body)
}

/// A one-triangle Xbox `modl` with the given VBUF flags and vertices.
fn xbox_model(flags: u32, stride: u32, verts: &[u8], indices: &[u16], prim: u32) -> Vec<u8> {
    let mut info = prim.to_le_bytes().to_vec();
    info.extend((verts.len() as u32 / stride).to_le_bytes());
    info.extend(1u32.to_le_bytes());
    let mut ibuf = (indices.len() as u32).to_le_bytes().to_vec();
    for i in indices {
        ibuf.extend(i.to_le_bytes());
    }
    let mut vbuf = (verts.len() as u32 / stride).to_le_bytes().to_vec();
    vbuf.extend(stride.to_le_bytes());
    vbuf.extend(flags.to_le_bytes());
    vbuf.extend(verts);
    let mut segm = chunk(b"INFO", &info);
    segm.extend(chunk(b"IBUF", &ibuf));
    segm.extend(chunk(b"VBUF", &vbuf));
    let mut modl_info = vec![0u8; 68];
    for (i, v) in [-1.0f32, -1.0, -1.0, 1.0, 1.0, 1.0].iter().enumerate() {
        modl_info[12 + i * 4..16 + i * 4].copy_from_slice(&v.to_le_bytes());
    }
    let mut modl = chunk(b"NAME", b"x\0");
    modl.extend(chunk(b"INFO", &modl_info));
    modl.extend(chunk(b"segm", &segm));
    chunk(b"modl", &modl)
}

#[test]
fn xbox_segment_with_every_field() {
    // pos, w0 w1, 3 indices, normal, tangents, colour, uv.
    let mut v = Vec::new();
    for i in 0..3i16 {
        for p in [i32::from(i) * 100 + 32767 - 32767, -32768, 32767] {
            v.extend((p as i16).to_le_bytes());
        }
        v.extend([10u8, 200]);
        v.extend([1u8, 2, 3]);
        v.extend(0x3FFu32.to_le_bytes());
        v.extend([0u8; 8]);
        v.extend([9u8, 8, 7, 255]);
        v.extend(2048i16.to_le_bytes());
        v.extend((-1024i16).to_le_bytes());
    }
    let bytes = xbox_model(0xF2EE, 31, &v, &[0, 1, 2], 6);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    let m = Model::parse(&c).unwrap();
    let s = &m.segments[0];
    assert!(!s.pc_geometry);
    assert_eq!(s.vertex_count, 3);
    assert_eq!(s.triangles, [[0, 1, 2]]);
    assert_eq!(s.uvs[1], [1.0, -0.5]);
    assert_eq!(s.normals[0], [1.0, 0.0, 0.0]);
    assert!((s.positions[0][1] + 1.0).abs() < 1e-5);
    assert!((s.positions[0][2] - 1.0).abs() < 1e-5);
    // Weights sort largest first and the third is the rest of 255.
    assert_eq!(s.wght[0], [200, 45, 10, 0]);
    assert_eq!(s.bind[0], [8, 12, 4, 0]);
    // B G R A to R G B A, halved to the PS2's scale.
    assert_eq!(s.colors.as_ref().unwrap()[0], [3, 4, 4, 127]);
}

#[test]
fn xbox_strip_alternates_winding_and_drops_stitches() {
    let v = vec![0u8; 14 * 4];
    let bytes = xbox_model(0xD222, 14, &v, &[0, 1, 2, 3, 3, 3], 6);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    assert_eq!(
        Model::parse(&c).unwrap().segments[0].triangles,
        [[0, 1, 2], [2, 1, 3]]
    );
}

#[test]
fn a_bad_xbox_vbuf_is_an_error_not_a_panic() {
    // Short of its declared vertices, and a stride its flags cannot make.
    let bytes = xbox_model(0xD222, 14, &[0u8; 14], &[0, 0, 0], 6);
    let mut cut = bytes.clone();
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    assert!(Model::parse(&c).is_ok());
    let at = cut.windows(4).position(|w| w == b"VBUF").unwrap();
    cut[at + 12..at + 16].copy_from_slice(&16u32.to_le_bytes());
    let c = crate::chunks(&cut).next().unwrap().unwrap();
    assert!(matches!(Model::parse(&c), Err(UcfbError::Unsupported(_))));
    let bytes = xbox_model(0xD222, 14, &[0u8; 14], &[0, 0, 0], 8);
    let c = crate::chunks(&bytes).next().unwrap().unwrap();
    assert!(matches!(Model::parse(&c), Err(UcfbError::Unsupported(_))));
}
