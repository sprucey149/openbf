#[test]
fn kashyyyk_bakes_its_sky_glow_mist_and_water_diffuse_grid() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../extracted/bf2/lvl/DATA/_LVL_PS2/KAS/KAS2.LVL");
    if !source.exists() {
        eprintln!("SKIP: extracted BF2 Kashyyyk is absent");
        return;
    }
    let bytes = std::fs::read(&source).unwrap();
    let chunks = mungefs::content_chunks(mungefs::open(&bytes).unwrap()).unwrap();
    let disc = chunks
        .iter()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| mungefs::texture::Texture::parse(c).ok())
        .find(|t| t.name.eq_ignore_ascii_case("kas2_sky"))
        .unwrap()
        .rgba8(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!(
        "openbf-kashyyyk-atmosphere-{}-{stamp}",
        std::process::id()
    ));
    let baked = bake::map::bake_map(source.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
    assert!(baked.lighting.fog.is_none());
    assert!(baked.sky.as_ref().unwrap().glow_masks_preserved);
    let cloud = baked.fog_cloud.as_ref().unwrap();
    assert_eq!(cloud.texture.as_deref(), Some("fluffy"));
    assert_eq!(cloud.range, Some([40.0, 80.0]));
    assert_eq!(cloud.color, Some([255.0, 255.0, 255.0, 75.0]));
    assert_eq!(cloud.particle_density, Some(20.0));
    let texture = std::fs::read(out.join("textures/kas2_sky.ktx2")).unwrap();
    let reader = ktx2::Reader::new(&texture).unwrap();
    assert_eq!(reader.levels().next().unwrap().data, disc);
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
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../extracted/bf2/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL");
    if !source.exists() {
        eprintln!("SKIP: extracted BF2 Geonosis is absent");
        return;
    }
    let bytes = std::fs::read(&source).unwrap();
    let inner = mungefs::open(&bytes).unwrap();
    let disc = mungefs::chunks(inner)
        .flatten()
        .filter(|c| &c.tag == b"tex_")
        .filter_map(|c| mungefs::texture::Texture::parse(&c).ok())
        .find(|t| t.name.eq_ignore_ascii_case("geo_sky"))
        .unwrap()
        .rgba8(0);
    let range = disc
        .as_chunks::<4>()
        .0
        .iter()
        .fold((255, 0), |(lo, hi), p| (lo.min(p[3]), hi.max(p[3])));
    assert_eq!(range, (0, 115));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!("openbf-sky-glow-{}-{stamp}", std::process::id()));
    let baked = bake::map::bake_map(source.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
    assert!(baked.hdr.is_some());
    assert!(baked.sky.as_ref().unwrap().glow_masks_preserved);
    let bytes = std::fs::read(out.join("textures/geo_sky.ktx2")).unwrap();
    let reader = ktx2::Reader::new(&bytes).unwrap();
    assert_eq!(reader.levels().next().unwrap().data, disc);
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn hoth_bakes_its_flare_and_keeps_both_dust_sheets_transparent() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../extracted/bf2/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL");
    if !source.exists() {
        eprintln!("SKIP: extracted BF2 Hoth absent");
        return;
    }
    let bytes = std::fs::read(&source).unwrap();
    let inner = mungefs::open(&bytes).unwrap();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = std::env::temp_dir().join(format!(
        "openbf-hoth-atmosphere-{}-{stamp}",
        std::process::id()
    ));
    let baked = bake::map::bake_map(source.to_str().unwrap(), out.to_str().unwrap(), None).unwrap();
    let sun = baked.sun_flare.unwrap();
    assert_eq!(sun.angle, [115.0, -20.0]);
    assert_eq!(sun.num_flare_outs, 40);
    for name in ["whitedust", "hoth_mist"] {
        let disc = mungefs::chunks(inner)
            .flatten()
            .filter(|c| &c.tag == b"tex_")
            .filter_map(|c| mungefs::texture::Texture::parse(&c).ok())
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .unwrap()
            .rgba8(0);
        assert!(disc.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        let bytes = std::fs::read(out.join(format!("textures/{name}.ktx2"))).unwrap();
        let reader = ktx2::Reader::new(&bytes).unwrap();
        assert_eq!(reader.levels().next().unwrap().data, disc);
    }
    std::fs::remove_dir_all(out).unwrap();
}
