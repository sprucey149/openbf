use std::path::PathBuf;

fn bf2_map(relative: &str) -> Option<(PathBuf, Vec<u8>)> {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../extracted/bf2/lvl/DATA/_LVL_PS2")
        .join(relative);
    if !source.exists() {
        eprintln!("SKIP: extracted BF2 {relative} is absent");
        return None;
    }
    let bytes = std::fs::read(&source).unwrap();
    Some((source, bytes))
}

fn bake(source: &std::path::Path, label: &str) -> (manifest::Manifest, PathBuf) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!("openbf-{label}-{}-{stamp}", std::process::id()));
    let baked = bake::map::bake_map(source.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
    (baked, out)
}

fn disc_texture(bytes: &[u8], name: &str) -> Vec<u8> {
    mungefs::content_chunks(mungefs::open(bytes).unwrap())
        .unwrap()
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| mungefs::texture::Texture::parse(c).ok())
        .find(|t| t.name.eq_ignore_ascii_case(name))
        .unwrap()
        .rgba8(0)
}

fn baked_texture(out: &std::path::Path, name: &str) -> Vec<u8> {
    let bytes = std::fs::read(out.join(format!("textures/{name}.ktx2"))).unwrap();
    let reader = ktx2::Reader::new(&bytes).unwrap();
    let level = reader.levels().next().unwrap().data.to_vec();
    level
}

#[test]
fn kashyyyk_bakes_its_sky_glow_mist_and_water_diffuse_grid() {
    let Some((source, bytes)) = bf2_map("KAS/KAS2.LVL") else {
        return;
    };
    let disc = disc_texture(&bytes, "kas2_sky");
    let (baked, out) = bake(&source, "kashyyyk-atmosphere");
    assert!(baked.lighting.fog.is_none());
    assert!(baked.sky.as_ref().unwrap().glow_masks_preserved);
    let cloud = baked.fog_cloud.as_ref().unwrap();
    assert_eq!(cloud.texture.as_deref(), Some("fluffy"));
    assert_eq!(cloud.range, Some([40.0, 80.0]));
    assert_eq!(cloud.color, Some([255.0, 255.0, 255.0, 75.0]));
    assert_eq!(cloud.particle_density, Some(20.0));
    assert_eq!(baked_texture(&out, "kas2_sky"), disc);
    assert!(disc.as_chunks::<4>().0.iter().any(|c| c[3] > 0));
    let water = baked.water.unwrap();
    assert!(water.vertex_diffuse);
    assert_eq!(water.triangles, 42 * 16 * 16 * 2);
    let (doc, buffers, _) = gltf::import(out.join(&water.glb)).unwrap();
    let primitive = doc.meshes().next().unwrap().primitives().next().unwrap();
    let mesh = primitive.reader(|b| Some(&buffers[b.index()]));
    assert_eq!(
        mesh.read_indices().unwrap().into_u32().count(),
        water.triangles * 3
    );
    let colors: Vec<_> = mesh.read_colors(0).unwrap().into_rgba_f32().collect();
    let values: std::collections::BTreeSet<_> = colors
        .iter()
        .map(|c| (c[0] * 255.0).round() as u32)
        .collect();
    assert_eq!(values, (0..11).map(|i| 30 + i * 8).collect());
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn geonosis_sky_glow_keeps_the_disc_mask_without_changing_rgb() {
    let Some((source, bytes)) = bf2_map("GEO/GEO1.LVL") else {
        return;
    };
    let disc = disc_texture(&bytes, "geo_sky");
    let range = disc
        .as_chunks::<4>()
        .0
        .iter()
        .fold((255, 0), |(lo, hi), p| (lo.min(p[3]), hi.max(p[3])));
    assert_eq!(range, (0, 115));
    let (baked, out) = bake(&source, "sky-glow");
    assert!(baked.hdr.is_some());
    assert!(baked.sky.as_ref().unwrap().glow_masks_preserved);
    assert_eq!(baked_texture(&out, "geo_sky"), disc);
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn hoth_bakes_its_flare_and_keeps_both_dust_sheets_transparent() {
    let Some((source, bytes)) = bf2_map("HOT/HOT1.LVL") else {
        return;
    };
    let (baked, out) = bake(&source, "hoth-atmosphere");
    let sun = baked.sun_flare.unwrap();
    assert_eq!(sun.angle, [115.0, -20.0]);
    assert_eq!(sun.num_flare_outs, 40);
    for name in ["whitedust", "hoth_mist"] {
        let disc = disc_texture(&bytes, name);
        assert!(disc.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        assert_eq!(baked_texture(&out, name), disc);
    }
    std::fs::remove_dir_all(out).unwrap();
}
